//! Wire → probe → use (ADR-009 / ADR-072). The root builds every adapter
//! from the validated configuration, runs every hard probe arm, and only
//! then serves. Any failure is one `health.startup.refused` event naming the
//! failing probe, then a non-zero exit — never a half-wired app.
//!
//! Log events are a closed catalogue ([`LogEvent`]): the allowlist of event
//! names and fields is the type, so no token, key or user data can be logged.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use adapter_atproto_did::IdentityLookup;
use adapter_atproto_ingest::AtProtoIngestAdapter;
use adapter_atproto_oauth::{ClientKey, OAuthClientAdapter, Upstreams};
use adapter_github::client::{
    token_days_left, token_expiry_needs_warning, TOKEN_EXPIRATION_HEADER,
};
use adapter_github::GithubAdapter;
use adapter_review_store::{DataKey, ReviewStore};
use ports::{
    OAuthPort, ProbeOutcome, ReviewStorePort, RevokeOutcome, ScanRunPort, ScanStatus,
    SecretStorePort,
};
use review_domain::kpi::{
    approval_events, rollup_day, seconds_until_next_rollup, sign_in_refusal_event, sum_counters,
    utc_day, KpiEvent,
};
use review_domain::signin::{permission_mode, SignInFailure};
use scraper_domain::{load_mapping, SignalPredicateMapping, EMBEDDED_MAPPING_YAML};
use serde_json::{json, Map, Value};

use crate::config::{
    parse_config, parse_data_key, parse_secrets, AppConfig, BuildProfile, DataKeyMaterial,
    GithubToken, RawSecrets, DEFAULT_OAUTH_SCOPES,
};
use crate::http::{self, App, Surface};
use crate::limiter::ScanLimiter;
use crate::routes::github::VerifyAttempts;

/// Exit code when the app refuses to start.
const EXIT_REFUSED: u8 = 2;

/// The startup probes, by the name the operator sees.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Probe {
    Config,
    Secrets,
    OAuthClient,
    ReviewStore,
    GithubToken,
    Listeners,
}

impl Probe {
    fn name(self) -> &'static str {
        match self {
            Self::Config => "config",
            Self::Secrets => "secrets",
            Self::OAuthClient => "oauth-client",
            Self::ReviewStore => "review-store",
            Self::GithubToken => "github-token",
            Self::Listeners => "listeners",
        }
    }
}

/// Why startup stopped: the probe and an operator-safe explanation.
#[derive(Debug)]
pub(crate) struct Refusal {
    probe: Probe,
    detail: String,
}

fn refusal(probe: Probe) -> impl FnOnce(String) -> Refusal {
    move |detail| Refusal { probe, detail }
}

/// The closed catalogue of events the app emits.
pub(crate) enum LogEvent<'a> {
    AppReady,
    StartupRefused(&'a Refusal),
    ProbePassed,
    SignInStarted,
    SignInCompleted,
    SignInRefused(SignInFailure),
    /// SPIKE finding 3: the OAuth library panicked in a code exchange and the
    /// panic was contained to that exchange.
    CallbackPanicIsolated,
    GithubVerified,
    /// A GitHub ownership check did not pass (the refusal's label only).
    GithubVerifyRefused(&'static str),
    ScanFinished(ScanStatus),
    /// The server GitHub token expires within the warning window (A-8).
    GithubTokenExpiring(i64),
    /// An owner declined a suggestion (KPI count; never which one or whose).
    SuggestionDeclined,
    /// An owner confirmed a share post and it landed (KPI count; never the
    /// text, whose or where).
    SharePosted,
    /// An owner confirmed a retraction and it landed (KPI count; never which
    /// claim or whose).
    RetractPosted,
    /// An owner's approval landed in her repo (KPI count; `edited` when she
    /// changed it first; never which or whose).
    SuggestionApproved {
        edited: bool,
    },
    /// A person was forgotten (by herself or on request) and how the
    /// revocation at her PDS ended.
    Disconnect(RevokeOutcome),
    /// The daily anonymous counters of `day`.
    KpiRollup {
        day: String,
        counters: BTreeMap<&'static str, i64>,
    },
    /// A data-key rotation re-encrypted `rows` sealed blobs at startup.
    SecretsRekeyed(usize),
}

/// The counters one event adds (pure). Only the closed catalogue counts.
fn counters_of(event: &LogEvent<'_>) -> Vec<KpiEvent> {
    match event {
        LogEvent::SignInStarted => vec![KpiEvent::SignInStarted],
        LogEvent::SignInCompleted => vec![KpiEvent::SignInCompleted],
        LogEvent::SignInRefused(failure) => vec![sign_in_refusal_event(*failure)],
        LogEvent::GithubVerified => vec![KpiEvent::GithubVerifyOk],
        LogEvent::GithubVerifyRefused(_) => vec![KpiEvent::GithubVerifyFail],
        LogEvent::ScanFinished(ScanStatus::Completed) => vec![KpiEvent::ScanCompleted],
        LogEvent::SuggestionDeclined => vec![KpiEvent::SuggestionDeclined],
        LogEvent::SuggestionApproved { edited } => approval_events(*edited),
        LogEvent::SharePosted => vec![KpiEvent::SharePosted],
        LogEvent::RetractPosted => vec![KpiEvent::RetractPosted],
        LogEvent::Disconnect(_) => vec![KpiEvent::Disconnect],
        _ => Vec::new(),
    }
}

/// Log `event` and add its anonymous counters for today (UTC). A counter
/// that cannot be stored never fails the request it describes.
pub(crate) fn observe(app: &App, event: LogEvent<'_>) {
    let counted = counters_of(&event);
    emit(event);
    let today = utc_day(crate::limiter::unix_now_secs());
    for counter in counted {
        let _ = app.kpi.count(&today, counter.name());
    }
}

/// The operator-facing name of why a sign-in did not complete.
fn refusal_reason(failure: SignInFailure) -> &'static str {
    match failure {
        SignInFailure::HandleNotFound => "handle_not_found",
        SignInFailure::TemporarilyUnavailable => "unavailable",
        SignInFailure::Cancelled => "cancelled",
        SignInFailure::ExchangeFailed => "exchange_failed",
        SignInFailure::AccountMismatch => "account_mismatch",
        SignInFailure::ReturnNotRecognised => "return_not_recognised",
    }
}

/// How loud an event is.
#[derive(Debug, Clone, Copy)]
enum Level {
    Info,
    Warn,
    Error,
}

impl Level {
    fn as_str(self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Warn => "warn",
            Self::Error => "error",
        }
    }
}

/// An event's level, catalogue name and extra fields (pure). The fields are
/// the whole allowlist: nothing outside this table is ever logged.
fn describe(event: LogEvent<'_>) -> (Level, &'static str, Vec<(&'static str, Value)>) {
    use Level::{Error, Info, Warn};
    match event {
        LogEvent::AppReady => (Info, "app.ready", vec![]),
        LogEvent::ProbePassed => (Info, "health.probe.passed", vec![]),
        LogEvent::SignInStarted => (Info, "signin.started", vec![]),
        LogEvent::SignInCompleted => (Info, "signin.completed", vec![]),
        LogEvent::SignInRefused(failure) => (
            Info,
            "signin.refused",
            vec![("reason", json!(refusal_reason(failure)))],
        ),
        LogEvent::CallbackPanicIsolated => (Warn, "signin.callback_panic_isolated", vec![]),
        LogEvent::SuggestionDeclined => (Info, "suggestion.declined", vec![]),
        LogEvent::SharePosted => (Info, "share.posted", vec![]),
        LogEvent::RetractPosted => (Info, "retract.posted", vec![]),
        LogEvent::SuggestionApproved { edited } => {
            (Info, "suggestion.approved", vec![("edited", json!(edited))])
        }
        LogEvent::Disconnect(outcome) => (
            Info,
            "disconnect",
            vec![("revoke_outcome", json!(outcome.as_str()))],
        ),
        LogEvent::KpiRollup { day, counters } => (
            Info,
            "kpi.rollup",
            vec![("day", json!(day)), ("counters", json!(counters))],
        ),
        LogEvent::SecretsRekeyed(rows) => (Info, "secrets.rekeyed", vec![("rows", json!(rows))]),
        LogEvent::GithubVerified => (Info, "github.verified", vec![]),
        LogEvent::GithubVerifyRefused(reason) => (
            Info,
            "github.verify_refused",
            vec![("reason", json!(reason))],
        ),
        LogEvent::ScanFinished(status) => (
            Info,
            "scan.finished",
            vec![("status", json!(status.as_str()))],
        ),
        LogEvent::GithubTokenExpiring(days_left) => (
            Warn,
            "github.token.expiring",
            vec![("days_left", json!(days_left))],
        ),
        LogEvent::StartupRefused(r) => (
            Error,
            "health.startup.refused",
            vec![
                ("probe", json!(r.probe.name())),
                ("reason", json!(r.detail)),
            ],
        ),
    }
}

/// The JSON line of `event` at Unix second `ts` (pure).
fn event_line(ts: u64, event: LogEvent<'_>) -> Value {
    let (level, name, fields) = describe(event);
    let mut line = Map::new();
    line.insert("ts".into(), json!(ts));
    line.insert("level".into(), json!(level.as_str()));
    line.insert("event".into(), json!(name));
    line.extend(
        fields
            .into_iter()
            .map(|(key, value)| (key.to_string(), value)),
    );
    Value::Object(line)
}

/// Emit one structured JSON event on stdout.
pub(crate) fn emit(event: LogEvent<'_>) {
    println!("{}", event_line(crate::limiter::unix_now(), event));
}

/// Everything the app runs on, wired but not yet trusted.
struct Wired {
    config: AppConfig,
    oauth: Arc<OAuthClientAdapter>,
    store: Arc<ReviewStore>,
    identity: Arc<IdentityLookup>,
    github_token: GithubToken,
    mapping: SignalPredicateMapping,
}

/// Run a future on a single-threaded runtime.
pub(crate) fn block_on(work: impl std::future::Future<Output = u8>) -> u8 {
    match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime.block_on(work),
        Err(e) => refuse(&Refusal {
            probe: Probe::Listeners,
            detail: format!("cannot start the async runtime: {e}"),
        }),
    }
}

fn refuse(r: &Refusal) -> u8 {
    emit(LogEvent::StartupRefused(r));
    EXIT_REFUSED
}

/// `serve`: wire, probe, then serve until the listeners fail.
pub(crate) async fn serve(env: &BTreeMap<String, String>) -> u8 {
    let ready = match wire(env) {
        Ok(wired) => probe(&wired).await.map(|()| wired),
        Err(r) => Err(r),
    };
    let wired = match ready {
        Ok(wired) => wired,
        Err(r) => return refuse(&r),
    };
    let listeners = bind(&wired.config).await;
    let (public, admin) = match listeners {
        Ok(pair) => pair,
        Err(r) => return refuse(&r),
    };
    // Nothing survives a restart still running (data-models `scan_runs`).
    let _ = wired.store.interrupt_running_scans();
    let token_check = (
        wired.config.github_api_base.clone(),
        wired.github_token.clone(),
    );
    let app = Arc::new(app(wired));
    tokio::spawn(daily_signals(app.clone(), token_check));
    emit(LogEvent::AppReady);
    match http::serve(public, admin, app).await {
        Ok(()) => 0,
        Err(e) => refuse(&Refusal {
            probe: Probe::Listeners,
            detail: format!("a listener failed: {e}"),
        }),
    }
}

/// `probe`: wire and probe the real configuration, then exit.
pub(crate) async fn probe_only(env: &BTreeMap<String, String>) -> u8 {
    let outcome = match wire(env) {
        Ok(wired) => probe(&wired).await,
        Err(r) => Err(r),
    };
    match outcome {
        Ok(()) => {
            emit(LogEvent::ProbePassed);
            0
        }
        Err(r) => refuse(&r),
    }
}

/// `probe --self-test`: a throwaway client key and data key, an in-memory
/// store, no network. Proves the image can sign, seal and isolate owners.
pub(crate) fn self_test() -> u8 {
    let offline = Upstreams {
        plc_url: "https://self-test.invalid",
        handle_resolver_url: "https://self-test.invalid",
    };
    let checked = ReviewStore::open_in_memory(DataKey::generate())
        .map_err(|e| refusal(Probe::ReviewStore)(e.to_string()))
        .and_then(|store| {
            let store = Arc::new(store);
            let oauth = OAuthClientAdapter::new(
                ClientKey::generate("self-test"),
                "https://self-test.invalid",
                DEFAULT_OAUTH_SCOPES,
                offline,
                store.clone(),
            )
            .map_err(|e| refusal(Probe::OAuthClient)(e.to_string()))?;
            arm(Probe::OAuthClient, oauth.probe())?;
            arm(Probe::ReviewStore, store.probe())
        });
    match checked {
        Ok(()) => {
            emit(LogEvent::ProbePassed);
            0
        }
        Err(r) => refuse(&r),
    }
}

/// `gen-client-jwk`: print a fresh private client JWK.
pub(crate) fn gen_client_jwk() -> u8 {
    let issued = crate::limiter::unix_now();
    println!(
        "{}",
        ClientKey::generate(&format!("openlore-review-{issued}")).to_private_json()
    );
    0
}

/// Build every adapter from the environment and the secrets directory.
fn wire(env: &BTreeMap<String, String>) -> Result<Wired, Refusal> {
    let config = parse_config(env, BuildProfile::of_this_build())
        .map_err(|e| refusal(Probe::Config)(e.to_string()))?;
    let mapping = load_mapping(EMBEDDED_MAPPING_YAML)
        .map_err(|e| refusal(Probe::Config)(format!("signal mapping: {e}")))?;
    let secrets = parse_secrets(read_secrets(&config.secrets_dir))
        .map_err(|e| refusal(Probe::Secrets)(e.to_string()))?;
    let key = ClientKey::parse(&secrets.client_jwk)
        .map_err(|e| refusal(Probe::OAuthClient)(e.to_string()))?;
    let previous = read_secret(&config.secrets_dir, "data-key-previous")
        .map(|text| parse_data_key("data-key-previous", &text))
        .transpose()
        .map_err(|e| refusal(Probe::Secrets)(e.to_string()))?;
    let store = ReviewStore::open(&config.review_db, data_key(secrets.data_key)?)
        .map(Arc::new)
        .map_err(|e| refusal(Probe::ReviewStore)(e.to_string()))?;
    if let Some(previous) = previous {
        let rows = store
            .rekey(&data_key(previous)?)
            .map_err(|e| refusal(Probe::ReviewStore)(format!("data-key rotation: {e}")))?;
        emit(LogEvent::SecretsRekeyed(rows));
    }
    let upstreams = Upstreams {
        plc_url: &config.plc_url,
        handle_resolver_url: &config.handle_resolver_url,
    };
    let secret_store: Arc<dyn SecretStorePort> = store.clone();
    let oauth = OAuthClientAdapter::new(
        key,
        &config.origin,
        &config.oauth_scopes,
        upstreams,
        secret_store,
    )
    .map(Arc::new)
    .map_err(|e| refusal(Probe::OAuthClient)(e.to_string()))?;
    let identity = Arc::new(IdentityLookup::new(
        &config.handle_resolver_url,
        &config.plc_url,
    ));
    Ok(Wired {
        config,
        oauth,
        store,
        identity,
        github_token: secrets.github_token,
        mapping,
    })
}

/// Every hard arm; the first refusal wins.
async fn probe(wired: &Wired) -> Result<(), Refusal> {
    arm(Probe::OAuthClient, wired.oauth.probe())?;
    arm(Probe::ReviewStore, wired.store.probe())?;
    github_token_arm(&wired.config.github_api_base, &wired.github_token).await
}

/// Once a day at 00:05 UTC: yesterday's anonymous counters as one
/// `kpi.rollup` line, and the server GitHub token's expiry re-checked so
/// `github.token.expiring` (alarm A-8, 14 days ahead) fires daily, not only
/// at startup.
async fn daily_signals(app: Arc<App>, (api_base, token): (String, GithubToken)) {
    loop {
        let now = crate::limiter::unix_now_secs();
        let wait = u64::try_from(seconds_until_next_rollup(now)).unwrap_or(1);
        tokio::time::sleep(Duration::from_secs(wait)).await;
        let at = crate::limiter::unix_now_secs();
        let day = rollup_day(at);
        if let Ok(rows) = app.kpi.counters_between(&day, &day) {
            emit(LogEvent::KpiRollup {
                counters: sum_counters(&rows),
                day,
            });
        }
        let _ = github_token_arm(&api_base, &token).await;
    }
}

fn arm(probe: Probe, outcome: ProbeOutcome) -> Result<(), Refusal> {
    match outcome {
        ProbeOutcome::Ok => Ok(()),
        ProbeOutcome::Refused { detail, .. } => Err(Refusal { probe, detail }),
    }
}

/// The server GitHub token must still be accepted (an expired or revoked
/// PAT would fail every scan). Unreachable GitHub is not a refusal: scans
/// report it to the user and retry.
async fn github_token_arm(api_base: &str, token: &GithubToken) -> Result<(), Refusal> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .user_agent("openlore-review-app")
        .build()
        .map_err(|e| refusal(Probe::GithubToken)(e.to_string()))?;
    let response = client
        .get(format!("{api_base}/rate_limit"))
        .bearer_auth(token.expose())
        .send()
        .await;
    match response {
        Ok(r) if r.status() == reqwest::StatusCode::UNAUTHORIZED => Err(Refusal {
            probe: Probe::GithubToken,
            detail: "GitHub rejected the server token (HTTP 401): renew the PAT".into(),
        }),
        Ok(r) => {
            warn_if_token_expiring(r.headers());
            Ok(())
        }
        Err(_) => Ok(()),
    }
}

/// A token close to expiry still serves; the operator is warned (A-8).
fn warn_if_token_expiring(headers: &reqwest::header::HeaderMap) {
    let now = crate::limiter::unix_now_secs();
    let days_left = headers
        .get(TOKEN_EXPIRATION_HEADER)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| token_days_left(value, now))
        .filter(|days| token_expiry_needs_warning(*days));
    if let Some(days_left) = days_left {
        emit(LogEvent::GithubTokenExpiring(days_left));
    }
}

/// The store key of a configured data key (a legacy key has no kid).
fn data_key(material: DataKeyMaterial) -> Result<DataKey, Refusal> {
    match material.kid {
        Some(kid) => DataKey::new(&kid, material.key)
            .ok_or_else(|| refusal(Probe::Secrets)("data-key: unusable key id".into())),
        None => Ok(DataKey::from_bytes(material.key)),
    }
}

/// One optional secrets file; absent or empty is `None`.
fn read_secret(dir: &Path, name: &str) -> Option<String> {
    std::fs::read_to_string(dir.join(name))
        .ok()
        .filter(|text| !text.trim().is_empty())
}

/// Read each secrets file; an absent file is `None`.
fn read_secrets(dir: &Path) -> RawSecrets {
    let read = |name: &str| std::fs::read_to_string(dir.join(name)).ok();
    RawSecrets {
        client_jwk: read("client-jwk"),
        data_key: read("data-key"),
        github_token: read("github-token"),
        log_salt: read("log-salt"),
    }
}

async fn bind(
    config: &AppConfig,
) -> Result<(tokio::net::TcpListener, tokio::net::TcpListener), Refusal> {
    let bind = |addr| async move {
        tokio::net::TcpListener::bind(addr)
            .await
            .map_err(|e| refusal(Probe::Listeners)(format!("cannot listen on {addr}: {e}")))
    };
    Ok((bind(config.listen).await?, bind(config.admin_listen).await?))
}

/// The app the listeners serve: the static surface, rendered once, and the
/// driven ports the pages use.
fn app(wired: Wired) -> App {
    let mode = permission_mode(&wired.config.oauth_scopes);
    let github = GithubAdapter::with_token(
        wired.config.github_api_base.as_str(),
        wired.github_token.expose(),
    );
    App {
        surface: Surface {
            landing: review_domain::views::landing_page(mode, None),
            client_metadata: wired.oauth.client_metadata().to_string(),
            jwks: wired.oauth.public_jwks().to_string(),
        },
        origin: wired.config.origin,
        permission_mode: mode,
        identity: wired.identity,
        repo_write: wired.oauth.clone(),
        repo_read: wired.oauth.clone(),
        repo_listing: Arc::new(AtProtoIngestAdapter::new("")),
        oauth: wired.oauth,
        sessions: wired.store.clone(),
        github: Arc::new(github),
        links: wired.store.clone(),
        scans: wired.store.clone(),
        review_read: wired.store.clone(),
        plans: wired.store.clone(),
        forget: wired.store.clone(),
        kpi: wired.store.clone(),
        review_write: wired.store,
        mapping: wired.mapping,
        verify_attempts: VerifyAttempts::default(),
        scan_limiter: ScanLimiter::default(),
    }
}
