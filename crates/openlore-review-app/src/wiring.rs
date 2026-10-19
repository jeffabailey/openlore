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
use adapter_review_store::{DataKey, DuckDbCaps, MigrationFault, ReviewStore};
use ports::claim_domain::authority::SigningRefusal;
use ports::{
    AccountCheckOutcome, KpiTally, OAuthPort, OutsideDefault, ProbeOutcome, ReviewStorePort,
    RevokeOutcome, ScanRunPort, ScanStatus, SecretStorePort,
};
use review_domain::kpi::{
    account_check_event, approval_events, outside_default_events, publish_refusal_events,
    publish_refusal_label, rollup_day, seconds_until_next_rollup, sign_in_refusal_event,
    sum_counters, utc_day, AccountAddRefused, KpiEvent,
};
use review_domain::signin::{permission_mode, SignInFailure};
use review_domain::views::ScanRefused;
use scraper_domain::{load_mapping, SignalPredicateMapping, EMBEDDED_MAPPING_YAML};
use serde_json::{json, Map, Value};

use crate::config::{
    parse_config, parse_data_key, parse_secrets, AppConfig, BuildProfile, DataKeyMaterial,
    GithubToken, RawSecrets, TestFault, DEFAULT_OAUTH_SCOPES,
};
use crate::http::{self, App, Surface};
use crate::limiter::{ScanLimiter, ShareAllowance};
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
    /// An owner who lists an account pressed Verify on another (KPI-RRA-2).
    AccountAddStarted,
    /// That account was added, verified (counted with the add).
    AccountAdded,
    /// That account was not added (the reason only; never whose or which).
    AccountAddRefused(AccountAddRefused),
    /// An owner removed one of her accounts (counted with the removal;
    /// never whose or which, nor how many suggestions went with it).
    AccountRemoved,
    /// A GitHub ownership check did not pass (the refusal's label only).
    GithubVerifyRefused(&'static str),
    /// An owner's confirmed identity link landed (never whose or which).
    GithubLinkPublished,
    /// A link confirm wrote nothing (the reason's label only, U-29).
    GithubLinkRefused(&'static str),
    ScanFinished(ScanStatus),
    /// A scan's bio check of one listed account ended (the outcome only;
    /// never whose or which account).
    ScanAccountChecked(AccountCheckOutcome),
    /// A scan read one repo, of an account whose check passed in that scan
    /// (KPI-RRA-5 audit; never which repo, account or whose).
    ScanRepoRead(AccountCheckOutcome),
    /// A scan was refused before it started (the reason only; never whose).
    ScanRefused(ScanRefused),
    /// The server GitHub token expires within the warning window (A-8).
    GithubTokenExpiring(i64),
    /// An owner declined a suggestion (KPI count; never which one or whose).
    SuggestionDeclined,
    /// A preview or confirm minted no signing permit (the reason only;
    /// never which repo or whose, KPI-RRA-4).
    PublishRefused(SigningRefusal),
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
        /// Counted when today's top 5 would never have produced it
        /// (KPI-RRA-1); never logged.
        outside_default: Option<OutsideDefault>,
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
        LogEvent::AccountAddStarted => vec![KpiEvent::AccountAddStarted],
        LogEvent::AccountAdded => vec![KpiEvent::AccountAddVerified],
        LogEvent::AccountRemoved => vec![KpiEvent::AccountRemoved],
        LogEvent::AccountAddRefused(refused) => vec![refused.event()],
        LogEvent::ScanFinished(ScanStatus::Completed) => vec![KpiEvent::ScanCompleted],
        LogEvent::ScanAccountChecked(outcome) => {
            account_check_event(*outcome).into_iter().collect()
        }
        LogEvent::SuggestionDeclined => vec![KpiEvent::SuggestionDeclined],
        LogEvent::PublishRefused(refusal) => publish_refusal_events(*refusal),
        LogEvent::SuggestionApproved {
            edited,
            outside_default,
        } => approval_events(*edited)
            .into_iter()
            .chain(outside_default_events(*outside_default))
            .collect(),
        LogEvent::SharePosted => vec![KpiEvent::SharePosted],
        LogEvent::RetractPosted => vec![KpiEvent::RetractPosted],
        LogEvent::Disconnect(_) => vec![KpiEvent::Disconnect],
        _ => Vec::new(),
    }
}

/// The anonymous counters an event adds, for today (UTC), handed to the
/// store write the event describes so both land in one transaction.
pub(crate) struct Counted {
    day: String,
    events: Vec<&'static str>,
}

impl Counted {
    pub(crate) fn of(event: &LogEvent<'_>) -> Self {
        Self {
            day: utc_day(crate::limiter::unix_now_secs()),
            events: counters_of(event).iter().map(|c| c.name()).collect(),
        }
    }

    pub(crate) fn tally(&self) -> KpiTally<'_> {
        KpiTally {
            day: &self.day,
            events: &self.events,
        }
    }
}

/// Log `event` and add its anonymous counters for today (UTC), for an event
/// with no state change of its own (a state change hands [`Counted`] to its
/// store write instead). A counter that cannot be stored never fails the
/// request it describes.
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

/// The operator-facing name of why a scan was refused.
fn scan_refusal_reason(refused: ScanRefused) -> &'static str {
    match refused {
        ScanRefused::AlreadyScanning => "already_scanning",
        ScanRefused::DailyLimitReached => "daily_limit",
        ScanRefused::AppBusy => "busy",
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
        LogEvent::PublishRefused(refusal) => (
            Info,
            "publish.refused",
            vec![("reason", json!(publish_refusal_label(refusal)))],
        ),
        LogEvent::SharePosted => (Info, "share.posted", vec![]),
        LogEvent::RetractPosted => (Info, "retract.posted", vec![]),
        LogEvent::SuggestionApproved { edited, .. } => {
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
        LogEvent::AccountAddStarted => (Info, "account.add.started", vec![]),
        LogEvent::AccountAdded => (Info, "account.add.verified", vec![]),
        LogEvent::AccountRemoved => (Info, "account.removed", vec![]),
        LogEvent::AccountAddRefused(refused) => (
            Info,
            "account.add.refused",
            vec![("reason", json!(refused.label()))],
        ),
        LogEvent::GithubVerifyRefused(reason) => (
            Info,
            "github.verify_refused",
            vec![("reason", json!(reason))],
        ),
        LogEvent::GithubLinkPublished => (Info, "github.link.published", vec![]),
        LogEvent::GithubLinkRefused(reason) => {
            (Info, "github.link.refused", vec![("reason", json!(reason))])
        }
        LogEvent::ScanFinished(status) => (
            Info,
            "scan.finished",
            vec![("status", json!(status.as_str()))],
        ),
        LogEvent::ScanAccountChecked(outcome) => (
            Info,
            "scan.account.checked",
            vec![("outcome", json!(outcome.as_str()))],
        ),
        LogEvent::ScanRepoRead(account_check) => (
            Info,
            "scan.repo.read",
            vec![("account_check", json!(account_check.as_str()))],
        ),
        LogEvent::ScanRefused(refused) => (
            Info,
            "scan.refused",
            vec![("reason", json!(scan_refusal_reason(refused)))],
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
    // Nothing survives a restart still running (data-models `scan_runs`);
    // those scans, and any paused for GitHub's budget, resume on their own.
    let interrupted = wired.store.interrupt_running_scans().unwrap_or_default();
    let paused = wired.store.owners_with_paused_scans().unwrap_or_default();
    let token_check = (
        wired.config.github_api_base.clone(),
        wired.github_token.clone(),
    );
    let app = Arc::new(app(wired));
    for owner_did in interrupted {
        crate::scan::resume_on_its_own(app.clone(), owner_did, true);
    }
    for owner_did in paused {
        crate::scan::resume_on_its_own(app.clone(), owner_did, false);
    }
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
    crate::limiter::set_clock_skew(config.clock_skew_secs);
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
    let caps = DuckDbCaps {
        memory_limit_mb: Some(config.db_memory_limit_mb),
        threads: Some(config.db_threads),
    };
    let fault = config.test_fault.map(migration_fault);
    let store =
        ReviewStore::open_capped_with(&config.review_db, data_key(secrets.data_key)?, caps, fault)
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
    let identity = Arc::new(IdentityLookup::guarded_for_sign_in(
        &config.handle_resolver_url,
        &config.plc_url,
        config.atproto_address_policy,
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

/// The store fault a development build's test fault names (DWD-RRA-16).
#[cfg(debug_assertions)]
fn migration_fault(fault: TestFault) -> MigrationFault {
    match fault {
        TestFault::MigrateBeforeCommit => MigrationFault::StopBeforeCommit,
    }
}

/// A release build holds no test fault (the type has no values).
#[cfg(not(debug_assertions))]
fn migration_fault(fault: TestFault) -> MigrationFault {
    match fault {}
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

/// The store key of a configured data key.
fn data_key(material: DataKeyMaterial) -> Result<DataKey, Refusal> {
    DataKey::new(&material.kid, material.key)
        .ok_or_else(|| refusal(Probe::Secrets)("data-key: unusable key id".into()))
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
    let scan_limiter = scan_limiter(&wired.config);
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
        repo_listing: Arc::new(AtProtoIngestAdapter::guarded_for_page_loads(
            "",
            wired.config.atproto_address_policy,
        )),
        oauth: wired.oauth,
        sessions: wired.store.clone(),
        github: Arc::new(github),
        links: wired.store.clone(),
        accounts_write: wired.store.clone(),
        account_status: wired.store.clone(),
        scope_removal: wired.store.clone(),
        listing_note: wired.store.clone(),
        scans: wired.store.clone(),
        scan_plans: wired.store.clone(),
        review_read: wired.store.clone(),
        choices_read: wired.store.clone(),
        choices_write: wired.store.clone(),
        plans: wired.store.clone(),
        forget: wired.store.clone(),
        kpi: wired.store.clone(),
        review_write: wired.store,
        mapping: wired.mapping,
        verify_attempts: VerifyAttempts::default(),
        share_allowance: ShareAllowance::default(),
        scan_limiter,
    }
}

/// The scan limiter, admitting as many concurrent scans as the config allows.
fn scan_limiter(config: &AppConfig) -> ScanLimiter {
    ScanLimiter::new(config.scan_concurrency)
}

#[cfg(test)]
mod tests {
    use super::*;
    use review_domain::budget::ScanAdmission;

    /// The limiter the app is wired with, from this environment.
    fn wired_limiter(scan_concurrency: Option<&str>) -> ScanLimiter {
        let mut env =
            BTreeMap::from([("APP_ORIGIN".to_string(), "https://app.example".to_string())]);
        if let Some(value) = scan_concurrency {
            env.insert(
                "OPENLORE_REVIEW_SCAN_CONCURRENCY".to_string(),
                value.to_string(),
            );
        }
        let config = parse_config(&env, BuildProfile::Release).expect("a valid config");
        scan_limiter(&config)
    }

    #[test]
    fn the_configured_scan_concurrency_reaches_the_limiter() {
        let one = wired_limiter(Some("1"));
        assert_eq!(one.admit("did:plc:first"), ScanAdmission::Admitted);
        assert_eq!(one.admit("did:plc:second"), ScanAdmission::AppBusy);
        one.release("did:plc:first");
        assert_eq!(one.admit("did:plc:second"), ScanAdmission::Admitted);

        let default = wired_limiter(None);
        assert_eq!(default.admit("did:plc:first"), ScanAdmission::Admitted);
        assert_eq!(default.admit("did:plc:second"), ScanAdmission::Admitted);
        assert_eq!(default.admit("did:plc:third"), ScanAdmission::AppBusy);
    }

    #[test]
    fn a_refused_scan_logs_its_reason_and_nothing_else() {
        for (refused, reason) in [
            (ScanRefused::AlreadyScanning, "already_scanning"),
            (ScanRefused::DailyLimitReached, "daily_limit"),
            (ScanRefused::AppBusy, "busy"),
        ] {
            let line = event_line(7, LogEvent::ScanRefused(refused));
            assert_eq!(
                line,
                json!({"ts": 7, "level": "info", "event": "scan.refused", "reason": reason})
            );
            assert!(counters_of(&LogEvent::ScanRefused(refused)).is_empty());
        }
    }
}
