//! `FakeGithub` — deterministic test double for the PUBLIC GitHub API.
//!
//! Distinct from [`crate::FakePds`] (the user's-own-PDS double) and
//! [`crate::FakePeerPds`] (a peer's read-only PDS double): GitHub is a
//! WHOLLY different external system (WD-61 / ADR-019). No method shape,
//! auth model, rate-limit semantic, or failure surface is shared with the
//! ATProto doubles, so `FakeGithub` is a SEPARATE module that backs the new
//! `GithubPort` (DD-SCR-2), never a posture on `FakePds`.
//!
//! `FakeGithub` implements ONLY the public, read-only paths slice-02
//! consumes per component-boundaries §`crates/adapter-github` + ADR-019:
//!
//! - resolve a target (`owner/repo` => Repo, `user` => User; REFUSE
//!   private / non-existent — WD-51 / I-SCR-2)
//! - harvest a repo's bounded public signal set (`harvest_repo`)
//! - harvest a user's BOUNDED cross-repo aggregate (`harvest_user`; deep
//!   triangulation deferred to slice-04 per WD-64)
//! - report the optional-PAT auth mode + remaining rate budget (US-SCR-004)
//!
//! ## Public-data-only by construction (WD-51 / I-SCR-2)
//!
//! There is NO private-repo handler and NO authenticated-private endpoint.
//! A `for_private_target` posture returns `GithubError::NotPublic` exactly
//! as the real public GitHub API would (404/403 on a private repo seen by a
//! non-member token), so the `scraper_only_reads_public_data` gate is
//! enforced STRUCTURALLY — the double cannot serve private data because it
//! has no private surface.
//!
//! ## Human-gate by construction (WD-49 / I-SCR-1)
//!
//! `FakeGithub` holds NO `StoragePort`, `IdentityPort`, or `PdsPort`
//! reference — by construction it CANNOT sign or persist or publish. The
//! ONLY path from a harvested signal to a signed claim is the human's
//! signing gesture through the slice-01 pipeline. This mirrors the
//! production `adapter-github`, which holds no storage/identity/pds ref
//! (the human-gate at the architecture layer).
//!
//! ## Posture is constructor-time-pinned (DD-SCR-3)
//!
//! Every posture (happy repo, happy user, not-found, private, offline,
//! rate-limited-anon, token-rejected, authenticated, no-matching-signals,
//! multi-signal-one-predicate) is fixed at construction. The
//! system-under-test sees a deterministic GitHub for the whole scenario
//! lifetime — no "did the test arm the rate-limit before or after resolve?"
//! race-condition test-bugs.
//!
//! ## Token-never-leaks (US-SCR-004 / I-SCR-?)
//!
//! When a posture carries an authenticated token, the token VALUE is held
//! only inside the fake's auth state and is NEVER echoed in any structured
//! event, captured output, or `Debug` impl. `saw_token` lets a scenario
//! assert the production code passed the token (so auth genuinely happened)
//! WITHOUT the double ever surfacing the value to stdout.
//!
//! ## Runtime model
//!
//! `serve_http` spins up an in-process HTTP server bound to a random
//! `127.0.0.1` port via `tokio::spawn`, returning a
//! [`FakeGithubHttpHandle`] whose `AbortOnDrop` guard stops the server when
//! the handle drops — RAII per-scenario isolation, byte-for-byte the same
//! pattern as [`crate::FakePeerPds::serve_http`]. The base URL is injected
//! into `adapter-github` via the `OPENLORE_GITHUB_API_BASE` env-var seam
//! (mirrors the slice-03 `OPENLORE_PEER_PDS_ENDPOINT_<did>` seam); the
//! optional PAT is injected via `GITHUB_TOKEN` (WD-63).

#![allow(dead_code)]

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

/// The well-known fixture repo target used by the happy-path scenarios.
pub const FIXTURE_REPO_TARGET: &str = "rust-lang/cargo";

/// The well-known fixture user/contributor target.
pub const FIXTURE_USER_TARGET: &str = "torvalds";

/// A sentinel token value the authenticated postures carry. Scenarios
/// assert this NEVER appears in any captured output / claim / log line
/// (US-SCR-004 no-token-leak), and that the production code DID send it
/// (so auth genuinely happened) via [`FakeGithub::saw_token`].
pub const FIXTURE_VALID_PAT: &str = "ghp_FAKEvalidtoken000000000000000000000000";

/// A sentinel token value the rejected-token posture carries (GitHub
/// returns 401 for it). Asserted to never leak, same as the valid PAT.
pub const FIXTURE_REJECTED_PAT: &str = "ghp_FAKErejectedtoken00000000000000000000000";

/// Which class of GitHub identifier the target resolves to.
///
/// Mirrors `ports::TargetKind` (the production type DELIVER wires); kept as
/// a fixture-local mirror so the double is self-describing in scenarios.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FakeTargetKind {
    /// `owner/repo` resolved to a public repository.
    Repo { owner: String, repo: String },
    /// `user` resolved to a public user / contributor.
    User { user: String },
}

/// A harvested public signal, as the fake returns it before the pure
/// `scraper-domain::derive_candidates` maps it to a candidate.
///
/// Fixture-local mirror of `scraper_domain::Signal` (component-boundaries
/// §`crates/scraper-domain`): `kind` matches a `signal_predicate_mapping`
/// entry; `value` is the human-readable detail a candidate names in its
/// source-signal line; `source_url` is the public GitHub URL that becomes
/// the candidate's evidence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FakeSignal {
    /// Stable identifier matching a `jobs.yaml` mapping entry's signal kind
    /// (e.g. `"DependencyManifestPinned"`, `"DocsPresentAndSubstantial"`).
    pub kind: String,
    /// Human-readable detail ("Cargo.lock committed (exact pins)",
    /// "test ratio 0.61"). This is what a candidate names as its source.
    pub value: String,
    /// Public GitHub URL evidencing the signal (becomes candidate evidence).
    pub source_url: String,
}

impl FakeSignal {
    /// Convenience constructor for a fixture signal.
    pub fn new(
        kind: impl Into<String>,
        value: impl Into<String>,
        source_url: impl Into<String>,
    ) -> Self {
        Self {
            kind: kind.into(),
            value: value.into(),
            source_url: source_url.into(),
        }
    }

    /// The fake's JSON view of one harvested signal, served under the
    /// `signals` array of the resolve/harvest endpoints. The adapter
    /// re-shapes this into `scraper_domain::Signal`; the fake just serves
    /// the raw harvested EFFECT data (WD-56 pure/effect split).
    fn as_json(&self) -> serde_json::Value {
        serde_json::json!({
            "kind": self.kind,
            "value": self.value,
            "source_url": self.source_url,
        })
    }
}

/// The auth posture a `FakeGithub` was constructed with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FakeAuthMode {
    /// No `GITHUB_TOKEN`; harvest runs against the anonymous rate budget.
    Anonymous,
    /// A valid PAT is present; harvest uses the authenticated budget. The
    /// `remaining`/`limit` pair is reported back ("4982/5000 rate budget").
    Authenticated { remaining: u32, limit: u32 },
}

/// Internal shared state. Held inside an `Arc` so the HTTP server task and
/// in-process assertion calls observe one source of truth. Records are NOT
/// mutated after construction (the posture is fixed); only the
/// `seen_token`/`seen_paths` observation slots toggle at runtime so a
/// scenario can assert what the production code sent.
struct State {
    /// The single target this fake serves and how it resolves (or the
    /// error posture if it must refuse).
    target: String,
    /// `Ok(kind)` for a resolvable public target; `Err(reason)` for the
    /// not-found / private / offline / rate-limited / token-rejected
    /// postures. Constructor-pinned (DD-SCR-3).
    resolution: Result<FakeTargetKind, FakeGithubErrorPosture>,
    /// The signals `harvest_repo` / `harvest_user` returns on the happy
    /// path. Empty for the no-matching-signals posture (US-SCR-002 Ex 2).
    signals: Vec<FakeSignal>,
    /// The auth posture (anonymous vs authenticated + budget).
    auth: FakeAuthMode,
    /// Observation slot: the token value the production code actually sent
    /// (or `None` if it sent none). Lets a scenario assert auth happened
    /// WITHOUT the value ever reaching captured output.
    seen_token: Mutex<Option<String>>,
    /// Observation slot: every request path the production code hit. Lets
    /// the `scraper_only_reads_public_data` gate assert ONLY allowlisted
    /// public paths were called (no private endpoint).
    seen_paths: Mutex<Vec<String>>,
    /// `true` for the offline posture: the HTTP server drops every
    /// connection without responding (reqwest classifies that as a network
    /// error the adapter lifts into `GithubError::Network`).
    offline: AtomicBool,
}

/// The deterministic GitHub error posture a refusing fixture serves.
///
/// Fixture-local mirror of `ports::GithubError` so adversarial /
/// degradation scenarios read self-describingly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FakeGithubErrorPosture {
    /// 404 — target named in the error; zero candidates (US-SCR-001 Ex 3).
    NotFound,
    /// private / inaccessible — "scraper only reads public data"
    /// (US-SCR-001 Ex 4; WD-51 / I-SCR-2).
    NotPublic,
    /// 403 rate budget exhausted — "set GITHUB_TOKEN" remediation
    /// (US-SCR-004 Ex 3). `authenticated` distinguishes anon vs PAT.
    RateLimited { authenticated: bool },
    /// 401 — stale/invalid PAT; the token value is NEVER echoed
    /// (US-SCR-004 Ex 4).
    TokenRejected,
    /// offline / transport — "scrape requires network" (US-SCR-001 Ex 5 /
    /// the offline UAT scenario).
    Network,
}

impl FakeGithubErrorPosture {
    /// HTTP status the public GitHub API returns for this posture. Used by
    /// the resolve/harvest handlers to serve a deterministic refusal.
    fn http_status(&self) -> u16 {
        match self {
            FakeGithubErrorPosture::NotFound => 404,
            // A private repo seen by a non-member token is indistinguishable
            // from a 404 on the public API (GitHub deliberately returns 404
            // to avoid leaking the repo's existence). Public-data-only is
            // structural: the fake has no private surface to serve.
            FakeGithubErrorPosture::NotPublic => 404,
            FakeGithubErrorPosture::RateLimited { .. } => 403,
            FakeGithubErrorPosture::TokenRejected => 401,
            // Network never reaches a handler — the offline posture drops the
            // connection. This status is only used if a handler is somehow
            // reached; in practice the offline flag short-circuits first.
            FakeGithubErrorPosture::Network => 503,
        }
    }

    /// The error-body `message` the refusing handler serves (mirrors the
    /// real GitHub API's JSON error shape).
    fn message(&self) -> &'static str {
        match self {
            FakeGithubErrorPosture::NotFound => "Not Found",
            FakeGithubErrorPosture::NotPublic => "Not Found",
            FakeGithubErrorPosture::RateLimited { .. } => "API rate limit exceeded",
            FakeGithubErrorPosture::TokenRejected => "Bad credentials",
            FakeGithubErrorPosture::Network => "Service Unavailable",
        }
    }
}

/// Deterministic read-only test double for the public GitHub API.
///
/// Construct with a posture (`for_public_repo`, `for_public_user`,
/// `for_not_found`, `for_private_target`, `offline`, `rate_limited_anon`,
/// `with_rejected_token`, `authenticated`, `with_no_matching_signals`,
/// `with_multi_signal_single_predicate`), then `serve_http()` to obtain a
/// base URL the `adapter-github` resolves against via the
/// `OPENLORE_GITHUB_API_BASE` env-var seam.
#[derive(Clone)]
pub struct FakeGithub {
    state: Arc<State>,
}

impl std::fmt::Debug for FakeGithub {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // The Debug impl MUST NOT print `seen_token` (no-token-leak
        // invariant, US-SCR-004). Render only the target + resolution +
        // auth-mode DISCRIMINANT (never the value): an `Authenticated`
        // budget is fine to surface (it is not the token), but the token
        // bytes the production code sent live in `seen_token` and are
        // deliberately omitted here.
        let auth = match &self.state.auth {
            FakeAuthMode::Anonymous => "Anonymous".to_string(),
            FakeAuthMode::Authenticated { remaining, limit } => {
                format!("Authenticated {{ remaining: {remaining}, limit: {limit} }}")
            }
        };
        f.debug_struct("FakeGithub")
            .field("target", &self.state.target)
            .field("resolution", &self.state.resolution)
            .field("auth", &auth)
            .finish_non_exhaustive()
    }
}

impl FakeGithub {
    // -------------------------------------------------------------------------
    // Internal constructor
    // -------------------------------------------------------------------------

    fn from_state(
        target: &str,
        resolution: Result<FakeTargetKind, FakeGithubErrorPosture>,
        signals: Vec<FakeSignal>,
        auth: FakeAuthMode,
    ) -> Self {
        Self {
            state: Arc::new(State {
                target: target.to_string(),
                resolution,
                signals,
                auth,
                seen_token: Mutex::new(None),
                seen_paths: Mutex::new(Vec::new()),
                offline: AtomicBool::new(false),
            }),
        }
    }

    /// Parse an `owner/repo` (or bare `user`) target into a resolved kind.
    fn resolve_kind(target: &str) -> FakeTargetKind {
        match target.split_once('/') {
            Some((owner, repo)) => FakeTargetKind::Repo {
                owner: owner.to_string(),
                repo: repo.to_string(),
            },
            None => FakeTargetKind::User {
                user: target.to_string(),
            },
        }
    }

    // -------------------------------------------------------------------------
    // Happy-path postures
    // -------------------------------------------------------------------------

    /// A public repo target that resolves to `Repo` and harvests the
    /// supplied signal set. The canonical happy-path constructor for
    /// US-SCR-001 / US-SCR-002 (SG-1 walking skeleton). `auth` defaults to
    /// anonymous; chain `.authenticated(...)` to flip it.
    pub fn for_public_repo(target: &str, signals: Vec<FakeSignal>) -> Self {
        Self::from_state(
            target,
            Ok(Self::resolve_kind(target)),
            signals,
            FakeAuthMode::Anonymous,
        )
    }

    /// A public user/contributor target that resolves to `User` and
    /// harvests a BOUNDED cross-repo aggregate (US-SCR-001 Ex 2; WD-64).
    pub fn for_public_user(user: &str, signals: Vec<FakeSignal>) -> Self {
        Self::from_state(
            user,
            Ok(FakeTargetKind::User {
                user: user.to_string(),
            }),
            signals,
            FakeAuthMode::Anonymous,
        )
    }

    /// A public repo whose harvest yields ZERO signals the mapping can use
    /// (US-SCR-002 Ex 2 — "no candidates derived", exit 0, not an error).
    pub fn with_no_matching_signals(target: &str) -> Self {
        Self::from_state(
            target,
            Ok(Self::resolve_kind(target)),
            Vec::new(),
            FakeAuthMode::Anonymous,
        )
    }

    /// A public repo whose harvest yields THREE distinct signals that all
    /// map to the SAME predicate (docs/ + long README + high doc-comment
    /// density => `documentation-first`). Used by SC-3 to assert the
    /// collapse-into-one-candidate behavior (US-SCR-002 Ex 4 / I-SCR-4).
    pub fn with_multi_signal_single_predicate(target: &str) -> Self {
        Self::from_state(
            target,
            Ok(Self::resolve_kind(target)),
            crate::fixtures_github::fixture_three_docs_signals_one_predicate(),
            FakeAuthMode::Anonymous,
        )
    }

    /// Mark this fixture authenticated with the supplied remaining/limit
    /// rate budget (US-SCR-004 Ex 1). The token the production code sends is
    /// captured in the `seen_token` observation slot but NEVER echoed.
    pub fn authenticated(self, remaining: u32, limit: u32) -> Self {
        // Posture is constructor-pinned: build a fresh state carrying the
        // authenticated auth mode while preserving the resolution + signals.
        let prev = self.state;
        Self {
            state: Arc::new(State {
                target: prev.target.clone(),
                resolution: prev.resolution.clone(),
                signals: prev.signals.clone(),
                auth: FakeAuthMode::Authenticated { remaining, limit },
                seen_token: Mutex::new(None),
                seen_paths: Mutex::new(Vec::new()),
                offline: AtomicBool::new(prev.offline.load(Ordering::SeqCst)),
            }),
        }
    }

    // -------------------------------------------------------------------------
    // Refusal / degradation postures (sad paths; example-only per Mandate 11)
    // -------------------------------------------------------------------------

    /// A target that does not exist (HTTP 404). Resolve fails with
    /// `NotFound`; zero candidates; non-zero exit (US-SCR-001 Ex 3).
    pub fn for_not_found(target: &str) -> Self {
        Self::from_state(
            target,
            Err(FakeGithubErrorPosture::NotFound),
            Vec::new(),
            FakeAuthMode::Anonymous,
        )
    }

    /// A PRIVATE / inaccessible target. The public-only API returns
    /// 404/403; resolve fails with `NotPublic`; "scraper only reads public
    /// data" (US-SCR-001 Ex 4; WD-51 / I-SCR-2). By construction the double
    /// has NO private surface — public-data-only is structural.
    pub fn for_private_target(target: &str) -> Self {
        Self::from_state(
            target,
            Err(FakeGithubErrorPosture::NotPublic),
            Vec::new(),
            FakeAuthMode::Anonymous,
        )
    }

    /// No reachable network (the offline posture). Resolve fails with
    /// `Network`; "scrape requires network"; no partial list (US-SCR-001 Ex
    /// 5 / offline UAT). Implemented by serving a server that drops every
    /// connection without responding (reqwest sees a transport error).
    pub fn offline() -> Self {
        let fake = Self::from_state(
            "",
            Err(FakeGithubErrorPosture::Network),
            Vec::new(),
            FakeAuthMode::Anonymous,
        );
        fake.state.offline.store(true, Ordering::SeqCst);
        fake
    }

    /// An UNAUTHENTICATED fixture whose harvest exhausts the anonymous rate
    /// budget mid-way (HTTP 403 + rate-limit headers). "set GITHUB_TOKEN for
    /// higher limits"; no partial candidate list (US-SCR-004 Ex 3). `target`
    /// resolves fine; the rate limit trips during harvest.
    pub fn rate_limited_anon(target: &str) -> Self {
        Self::from_state(
            target,
            Err(FakeGithubErrorPosture::RateLimited {
                authenticated: false,
            }),
            Vec::new(),
            FakeAuthMode::Anonymous,
        )
    }

    /// A fixture that REJECTS the supplied token (HTTP 401). The CLI exits
    /// non-zero with the 401 explanation; the token value is NEVER echoed
    /// (US-SCR-004 Ex 4). `target` is irrelevant — the auth check fails
    /// first.
    pub fn with_rejected_token(target: &str) -> Self {
        Self::from_state(
            target,
            Err(FakeGithubErrorPosture::TokenRejected),
            Vec::new(),
            FakeAuthMode::Anonymous,
        )
    }

    // -------------------------------------------------------------------------
    // Runtime + observation
    // -------------------------------------------------------------------------

    /// Spin up an in-process HTTP server on a random `127.0.0.1` port and
    /// return its base URL + an `AbortOnDrop` handle. The base URL is fed to
    /// `adapter-github` via `OPENLORE_GITHUB_API_BASE` (the test-only seam,
    /// mirroring the slice-03 peer-endpoint seam). The server stops when the
    /// returned handle drops (RAII per-scenario isolation).
    ///
    /// Async to mirror [`crate::FakePeerPds::serve_http`] exactly — the
    /// acceptance `support::FakeGithub` wrapper owns its own tokio runtime
    /// and `block_on`s this, byte-for-byte the same way the `PeerPds` /
    /// `FakePds` wrappers do.
    pub async fn serve_http(&self) -> FakeGithubHttpHandle {
        use hyper::server::conn::http1;
        use hyper_util::rt::TokioIo;
        use tokio::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("FakeGithub::serve_http: bind 127.0.0.1:0");
        let local_addr = listener
            .local_addr()
            .expect("FakeGithub::serve_http: local_addr");
        let base_url = format!("http://{local_addr}");

        let fake = self.clone();
        let handle = tokio::spawn(async move {
            loop {
                let (stream, _peer) = match listener.accept().await {
                    Ok(io) => io,
                    Err(_) => return, // listener died — task shuts down
                };

                // Offline mode: drop the connection immediately. reqwest
                // sees this as a network error which the adapter lifts into
                // `GithubError::Network` (US-SCR-001 Ex 5).
                if fake.state.offline.load(Ordering::SeqCst) {
                    drop(stream);
                    continue;
                }

                let fake_for_conn = fake.clone();
                tokio::spawn(async move {
                    let io = TokioIo::new(stream);
                    let svc = hyper::service::service_fn(move |req| {
                        let fake = fake_for_conn.clone();
                        async move { github_http_route(fake, req).await }
                    });
                    let _ = http1::Builder::new().serve_connection(io, svc).await;
                });
            }
        });

        FakeGithubHttpHandle {
            base_url,
            _task: AbortOnDrop(handle),
        }
    }

    /// The auth posture this fixture was constructed with (anonymous vs
    /// authenticated + budget). Used by SA-1/SA-2 to assert the reported
    /// auth-line matches the posture.
    pub fn auth_mode(&self) -> FakeAuthMode {
        self.state.auth.clone()
    }

    /// `true` iff the production code sent the given token value to the fake
    /// (so a scenario can assert auth genuinely happened) WITHOUT the value
    /// ever reaching captured output. Pairs with the no-token-leak assertion
    /// that the same value is ABSENT from stdout/stderr (US-SCR-004).
    pub fn saw_token(&self, token: &str) -> bool {
        self.state
            .seen_token
            .lock()
            .map(|seen| seen.as_deref() == Some(token))
            .unwrap_or(false)
    }

    /// Every request PATH the production code hit, in order. The
    /// `scraper_only_reads_public_data` gate asserts every entry is on the
    /// public-endpoint allowlist (no private path; KPI-SCR-4 / I-SCR-2).
    pub fn seen_paths(&self) -> Vec<String> {
        self.state
            .seen_paths
            .lock()
            .map(|paths| paths.clone())
            .unwrap_or_default()
    }

    // -------------------------------------------------------------------------
    // Observation recording (used by the HTTP route handler)
    // -------------------------------------------------------------------------

    fn record_seen_path(&self, path: &str) {
        if let Ok(mut paths) = self.state.seen_paths.lock() {
            paths.push(path.to_string());
        }
    }

    fn record_seen_token(&self, token: &str) {
        if let Ok(mut seen) = self.state.seen_token.lock() {
            *seen = Some(token.to_string());
        }
    }
}

// -----------------------------------------------------------------------------
// HTTP routing — read-only PUBLIC GitHub REST subset
// -----------------------------------------------------------------------------
//
// The fake serves ONLY the public, read-only paths slice-02 consumes. There
// is NO private surface (public-data-only is structural). Every request path
// is recorded into `seen_paths` so the `scraper_only_reads_public_data` gate
// can assert the allowlist was honored. An `Authorization: token <PAT>`
// header is recorded into `seen_token` (the value the production code sent)
// so `saw_token` can confirm auth happened — but the value is NEVER echoed
// in any response body.

type HttpRequest = hyper::Request<hyper::body::Incoming>;
type HttpResponse = hyper::Response<http_body_util::Full<bytes::Bytes>>;

/// Read-only route handler over the public GitHub REST subset. Records the
/// request path + any bearer token, then dispatches to the posture-pinned
/// resolution / harvest responses.
async fn github_http_route(
    fake: FakeGithub,
    req: HttpRequest,
) -> Result<HttpResponse, std::convert::Infallible> {
    let path = req.uri().path().to_string();
    let method = req.method().clone();

    // Record the request path (allowlist observability) + any token the
    // production code sent in the `Authorization` header (never echoed).
    fake.record_seen_path(&path);
    if let Some(token) = extract_bearer_token(&req) {
        fake.record_seen_token(&token);
    }

    // A token-rejected posture fails the auth check FIRST (before any
    // resolution), exactly as the real API returns 401 for bad credentials.
    if let Err(posture @ FakeGithubErrorPosture::TokenRejected) = &fake.state.resolution {
        return Ok(error_response(posture));
    }

    // Only GETs are served — the public read paths are all GETs. Any other
    // method is a slice-02 invariant violation (the scraper never writes).
    if method != hyper::Method::GET {
        return Ok(text_response(
            405,
            format!("FakeGithub is read-only; refusing {method} {path}"),
        ));
    }

    match &fake.state.resolution {
        Ok(kind) => Ok(resolve_or_harvest_response(&fake, kind, &path)),
        Err(posture) => Ok(error_response(posture)),
    }
}

/// Serve the resolution + harvest happy-path response for a resolvable
/// target. The fake collapses resolve + harvest into one self-describing
/// JSON document (the adapter re-shapes it); a real adapter would make
/// several GETs, all of which land on this handler and are recorded.
fn resolve_or_harvest_response(
    fake: &FakeGithub,
    kind: &FakeTargetKind,
    _path: &str,
) -> HttpResponse {
    let target_json = match kind {
        FakeTargetKind::Repo { owner, repo } => serde_json::json!({
            "kind": "repo",
            "owner": owner,
            "repo": repo,
            "full_name": format!("{owner}/{repo}"),
            "private": false,
        }),
        FakeTargetKind::User { user } => serde_json::json!({
            "kind": "user",
            "login": user,
        }),
    };

    let signals: Vec<serde_json::Value> =
        fake.state.signals.iter().map(FakeSignal::as_json).collect();

    let auth_json = match &fake.state.auth {
        FakeAuthMode::Anonymous => serde_json::json!({ "authenticated": false }),
        FakeAuthMode::Authenticated { remaining, limit } => serde_json::json!({
            "authenticated": true,
            "rate_remaining": remaining,
            "rate_limit": limit,
        }),
    };

    json_response(
        200,
        serde_json::json!({
            "target": target_json,
            "signals": signals,
            "auth": auth_json,
        }),
    )
}

/// Serve a deterministic refusal for an error posture (mirrors the public
/// GitHub API's JSON error shape). The token value is NEVER included.
fn error_response(posture: &FakeGithubErrorPosture) -> HttpResponse {
    let body = match posture {
        FakeGithubErrorPosture::RateLimited { authenticated } => serde_json::json!({
            "message": posture.message(),
            "authenticated": authenticated,
            "documentation_url": "https://docs.github.com/rest/overview/rate-limits",
        }),
        _ => serde_json::json!({ "message": posture.message() }),
    };
    json_response(posture.http_status(), body)
}

/// Extract the PAT from an `Authorization: token <PAT>` or
/// `Authorization: Bearer <PAT>` header (the two shapes the real GitHub API
/// accepts). Returns the bare token value (never logged).
fn extract_bearer_token(req: &HttpRequest) -> Option<String> {
    let value = req.headers().get(hyper::header::AUTHORIZATION)?;
    let raw = value.to_str().ok()?;
    for prefix in ["token ", "Bearer ", "bearer "] {
        if let Some(rest) = raw.strip_prefix(prefix) {
            return Some(rest.to_string());
        }
    }
    None
}

fn json_response(status: u16, body: serde_json::Value) -> HttpResponse {
    let bytes = bytes::Bytes::from(body.to_string());
    hyper::Response::builder()
        .status(status)
        .header("content-type", "application/json")
        .body(http_body_util::Full::new(bytes))
        .expect("build JSON response")
}

fn text_response(status: u16, body: String) -> HttpResponse {
    let bytes = bytes::Bytes::from(body);
    hyper::Response::builder()
        .status(status)
        .header("content-type", "text/plain")
        .body(http_body_util::Full::new(bytes))
        .expect("build text response")
}

/// RAII handle for the [`FakeGithub::serve_http`] background server.
///
/// Holds the bound base URL (injected via `OPENLORE_GITHUB_API_BASE`) and a
/// guard that aborts the server task on drop — byte-for-byte the same shape
/// as [`crate::FakePeerPdsHttpHandle`] (DD-SCR-2 reuse-the-pattern).
#[derive(Debug)]
pub struct FakeGithubHttpHandle {
    /// The `http://127.0.0.1:<port>` base URL the adapter resolves against.
    pub base_url: String,
    /// Abort guard; aborting stops the background server on drop.
    _task: AbortOnDrop<()>,
}

impl FakeGithubHttpHandle {
    /// The base URL to feed `adapter-github` via `OPENLORE_GITHUB_API_BASE`.
    pub fn base_url(&self) -> &str {
        &self.base_url
    }
}

/// Aborts a spawned tokio task when dropped (RAII server shutdown). Mirrors
/// the slice-03 `FakePeerPds` AbortOnDrop guard exactly.
#[derive(Debug)]
struct AbortOnDrop<T>(tokio::task::JoinHandle<T>);

impl<T> Drop for AbortOnDrop<T> {
    fn drop(&mut self) {
        self.0.abort();
    }
}

// -----------------------------------------------------------------------------
// Unit tests — the FakeGithub contract is load-bearing for the slice-02
// scrape_* acceptance scenarios, so we pin its shape with real
// async-runtime + real-HTTP tests here (RED_UNIT for step 01-05). These
// drive the double as an in-process HTTP CLIENT, exactly as the production
// `adapter-github` reqwest client will, proving the served record set +
// the structural public-data-only / no-token-leak guarantees.
// -----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    async fn get_with_optional_token(
        url: &str,
        token: Option<&str>,
    ) -> Result<(u16, serde_json::Value), ()> {
        use http_body_util::BodyExt;
        use hyper::Request;
        use hyper_util::rt::TokioIo;

        let uri: hyper::Uri = url.parse().map_err(|_| ())?;
        let host = uri.host().ok_or(())?.to_string();
        let port = uri.port_u16().ok_or(())?;
        let stream = tokio::net::TcpStream::connect((host.as_str(), port))
            .await
            .map_err(|_| ())?;
        let io = TokioIo::new(stream);
        let (mut sender, conn) = hyper::client::conn::http1::handshake(io)
            .await
            .map_err(|_| ())?;
        tokio::spawn(async move {
            let _ = conn.await;
        });
        let authority = uri.authority().ok_or(())?.clone();
        let mut builder = Request::builder()
            .uri(&uri)
            .header(hyper::header::HOST, authority.as_str());
        if let Some(token) = token {
            builder = builder.header(hyper::header::AUTHORIZATION, format!("token {token}"));
        }
        let req = builder
            .body(http_body_util::Empty::<bytes::Bytes>::new())
            .map_err(|_| ())?;
        let resp = sender.send_request(req).await.map_err(|_| ())?;
        let status = resp.status().as_u16();
        let body = resp.into_body().collect().await.map_err(|_| ())?.to_bytes();
        let json: serde_json::Value =
            serde_json::from_slice(&body).unwrap_or(serde_json::Value::Null);
        Ok((status, json))
    }

    async fn get_json(url: &str) -> (u16, serde_json::Value) {
        get_with_optional_token(url, None)
            .await
            .expect("GET must succeed")
    }

    /// `for_public_repo` resolves to a Repo and serves the supplied signal
    /// set. The load-bearing happy-path contract: SG-1 cannot wire without
    /// this returning every fixture signal at a 200.
    #[tokio::test]
    async fn for_public_repo_resolves_and_serves_signals() {
        let signals = vec![
            FakeSignal::new(
                "DependencyManifestPinned",
                "Cargo.lock committed",
                "https://x.test/1",
            ),
            FakeSignal::new(
                "DocsPresentAndSubstantial",
                "docs/ present",
                "https://x.test/2",
            ),
        ];
        let fake = FakeGithub::for_public_repo("rust-lang/cargo", signals);
        let handle = fake.serve_http().await;

        let (status, body) =
            get_json(&format!("{}/repos/rust-lang/cargo", handle.base_url())).await;

        assert_eq!(status, 200, "public repo must resolve at 200");
        assert_eq!(body["target"]["kind"], "repo");
        assert_eq!(body["target"]["full_name"], "rust-lang/cargo");
        let served = body["signals"].as_array().expect("signals array");
        assert_eq!(served.len(), 2, "every fixture signal must be served");
        assert_eq!(served[0]["kind"], "DependencyManifestPinned");
        assert_eq!(body["auth"]["authenticated"], false);
    }

    /// `for_public_user` resolves to a User (not a repo) — drives SG-3 /
    /// SA-1's user-target path (WD-64 bounded aggregate).
    #[tokio::test]
    async fn for_public_user_resolves_as_user() {
        let fake = FakeGithub::for_public_user(
            "torvalds",
            vec![FakeSignal::new(
                "MemorySafetyLanguage",
                "C kernel",
                "https://x.test/k",
            )],
        );
        let handle = fake.serve_http().await;

        let (status, body) = get_json(&format!("{}/users/torvalds", handle.base_url())).await;

        assert_eq!(status, 200);
        assert_eq!(body["target"]["kind"], "user");
        assert_eq!(body["target"]["login"], "torvalds");
    }

    /// A PRIVATE target refuses with a non-200 and serves NO signals — the
    /// structural public-data-only guarantee (WD-51 / I-SCR-2): the double
    /// has no private surface to serve.
    #[tokio::test]
    async fn private_target_refuses_and_serves_no_signals() {
        let fake = FakeGithub::for_private_target("acme-corp/secret-repo");
        let handle = fake.serve_http().await;

        let (status, body) = get_json(&format!(
            "{}/repos/acme-corp/secret-repo",
            handle.base_url()
        ))
        .await;

        assert_ne!(status, 200, "a private target must NOT resolve at 200");
        assert!(
            body.get("signals").is_none(),
            "a refusing posture must serve NO signals (no private surface)"
        );
    }

    /// A not-found target serves a 404.
    #[tokio::test]
    async fn not_found_target_serves_404() {
        let fake = FakeGithub::for_not_found("ghost-org/ghost-repo");
        let handle = fake.serve_http().await;
        let (status, _) =
            get_json(&format!("{}/repos/ghost-org/ghost-repo", handle.base_url())).await;
        assert_eq!(status, 404, "a non-existent target must 404");
    }

    /// The rate-limited-anonymous posture serves a 403 with the rate-limit
    /// message — drives SA-3's "set GITHUB_TOKEN" remediation path.
    #[tokio::test]
    async fn rate_limited_anon_serves_403() {
        let fake = FakeGithub::rate_limited_anon("torvalds");
        let handle = fake.serve_http().await;
        let (status, body) = get_json(&format!("{}/users/torvalds", handle.base_url())).await;
        assert_eq!(status, 403, "rate-limit exhaustion must 403");
        assert_eq!(body["authenticated"], false);
    }

    /// The token-rejected posture serves a 401 and the token VALUE never
    /// appears in the response body — but `saw_token` confirms the
    /// production code DID send it (auth happened). The no-token-leak
    /// structural guarantee (US-SCR-004), proven both ways.
    #[tokio::test]
    async fn rejected_token_serves_401_records_token_but_never_echoes_it() {
        let fake = FakeGithub::with_rejected_token("rust-lang/cargo");
        let handle = fake.serve_http().await;

        let (status, body) = get_with_optional_token(
            &format!("{}/repos/rust-lang/cargo", handle.base_url()),
            Some(FIXTURE_REJECTED_PAT),
        )
        .await
        .expect("GET must complete");

        assert_eq!(status, 401, "a rejected token must 401");
        // saw_token confirms the production code sent it (auth happened) ...
        assert!(
            fake.saw_token(FIXTURE_REJECTED_PAT),
            "the fake must observe the token the client sent"
        );
        // ... but the value MUST NOT appear in the response body.
        assert!(
            !body.to_string().contains(FIXTURE_REJECTED_PAT),
            "the token value must NEVER be echoed in any response body"
        );
        // ... nor in the Debug rendering (no-token-leak via {:?}).
        assert!(
            !format!("{fake:?}").contains(FIXTURE_REJECTED_PAT),
            "the token value must NEVER appear in the Debug impl"
        );
    }

    /// An authenticated posture reports its rate budget; the token the
    /// client sends is observed via `saw_token` but the auth-mode discriminant
    /// is the only thing surfaced. Drives SA-1.
    #[tokio::test]
    async fn authenticated_posture_reports_budget_and_observes_token() {
        let fake = FakeGithub::for_public_user(
            "torvalds",
            vec![FakeSignal::new(
                "MemorySafetyLanguage",
                "kernel",
                "https://x.test/k",
            )],
        )
        .authenticated(4982, 5000);
        let handle = fake.serve_http().await;

        let (status, body) = get_with_optional_token(
            &format!("{}/users/torvalds", handle.base_url()),
            Some(FIXTURE_VALID_PAT),
        )
        .await
        .expect("GET must complete");

        assert_eq!(status, 200);
        assert_eq!(body["auth"]["authenticated"], true);
        assert_eq!(body["auth"]["rate_remaining"], 4982);
        assert_eq!(body["auth"]["rate_limit"], 5000);
        assert!(fake.saw_token(FIXTURE_VALID_PAT));
        assert_eq!(
            fake.auth_mode(),
            FakeAuthMode::Authenticated {
                remaining: 4982,
                limit: 5000
            }
        );
        assert!(
            !body.to_string().contains(FIXTURE_VALID_PAT),
            "the token must never be echoed even on the happy authenticated path"
        );
    }

    /// `seen_paths` records every path the client hit, in order — drives the
    /// `scraper_only_reads_public_data` allowlist gate (KPI-SCR-4).
    #[tokio::test]
    async fn seen_paths_records_every_requested_path() {
        let fake = FakeGithub::for_public_repo(
            "rust-lang/cargo",
            vec![FakeSignal::new("X", "y", "https://x.test/z")],
        );
        let handle = fake.serve_http().await;

        let _ = get_json(&format!("{}/repos/rust-lang/cargo", handle.base_url())).await;
        let _ = get_json(&format!(
            "{}/repos/rust-lang/cargo/contents",
            handle.base_url()
        ))
        .await;

        let paths = fake.seen_paths();
        assert_eq!(paths.len(), 2, "every request path must be recorded");
        assert_eq!(paths[0], "/repos/rust-lang/cargo");
        assert_eq!(paths[1], "/repos/rust-lang/cargo/contents");
        assert!(
            paths
                .iter()
                .all(|p| p.starts_with("/repos/") || p.starts_with("/users/")),
            "all recorded paths are on the public allowlist (no private surface)"
        );
    }

    /// The offline posture drops the connection — a client GET errors out
    /// rather than receiving a response (US-SCR-001 Ex 5 / offline UAT).
    #[tokio::test]
    async fn offline_posture_drops_the_connection() {
        let fake = FakeGithub::offline();
        let handle = fake.serve_http().await;

        let result = tokio::time::timeout(
            std::time::Duration::from_secs(2),
            get_with_optional_token(&format!("{}/repos/x/y", handle.base_url()), None),
        )
        .await
        .expect("offline probe must not hang");
        assert!(result.is_err(), "while offline the request must fail");
    }
}
