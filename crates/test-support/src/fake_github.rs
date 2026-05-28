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
//! event, captured output, or `Debug` impl. `record_seen_token` lets a
//! scenario assert the production code passed the token (so auth genuinely
//! happened) WITHOUT the double ever surfacing the value to stdout.
//!
//! ## Runtime model
//!
//! `serve_http` spins up an in-process HTTP server bound to a random
//! `127.0.0.1` port via `tokio::spawn`, returning a
//! [`FakeGithubHttpHandle`] whose `AbortOnDrop` guard stops the server when
//! the handle drops — RAII per-scenario isolation, byte-for-byte the same
//! pattern as [`crate::FakePds::serve_http`] + [`crate::FakePeerPds::serve_http`].
//! The base URL is injected into `adapter-github` via the
//! `OPENLORE_GITHUB_API_BASE` env-var seam (mirrors the slice-03
//! `OPENLORE_PEER_PDS_ENDPOINT_<did>` seam); the optional PAT is injected
//! via `GITHUB_TOKEN` (WD-63).
//!
//! ## RED scaffold (DISTILL slice-02)
//!
//! Per Mandate 7 + DD-SCR precedent: the types + method shapes exist; every
//! body is `todo!("DELIVER (slice-02): ...")`. DELIVER's first slice-02 step
//! (step-07-01) materializes the bodies. Until then `cargo build --tests`
//! fails to compile against this crate; once the bodies + the `GithubPort`
//! trait + `scraper-domain` ADTs land, the slice-02 acceptance suite
//! classifies as RED (panic at `todo!()`).
//
// SCAFFOLD: true

#![allow(dead_code)]

use std::sync::atomic::AtomicBool;
use std::sync::Arc;

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
        // SCAFFOLD: true
        let _ = (kind, value, source_url);
        todo!("DELIVER (slice-02): construct a FakeSignal fixture value")
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
#[derive(Debug)]
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
    seen_token: std::sync::Mutex<Option<String>>,
    /// Observation slot: every request path the production code hit. Lets
    /// the `scraper_only_reads_public_data` gate assert ONLY allowlisted
    /// public paths were called (no private endpoint).
    seen_paths: std::sync::Mutex<Vec<String>>,
    /// Toggled to abort the background HTTP server on handle drop.
    aborted: AtomicBool,
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
    fn fmt(&self, _f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // SCAFFOLD: true — the Debug impl MUST NOT print `seen_token`
        // (no-token-leak invariant, US-SCR-004). DELIVER renders only the
        // target + resolution + auth-mode discriminant (never the value).
        todo!("DELIVER (slice-02): Debug FakeGithub WITHOUT echoing any token value")
    }
}

impl FakeGithub {
    // -------------------------------------------------------------------------
    // Happy-path postures
    // -------------------------------------------------------------------------

    /// A public repo target that resolves to `Repo` and harvests the
    /// supplied signal set. The canonical happy-path constructor for
    /// US-SCR-001 / US-SCR-002 (SG-1 walking skeleton). `auth` defaults to
    /// anonymous; chain `.authenticated(...)` to flip it.
    pub fn for_public_repo(target: &str, signals: Vec<FakeSignal>) -> Self {
        // SCAFFOLD: true
        let _ = (target, signals);
        todo!("DELIVER (slice-02): construct a public-repo FakeGithub posture")
    }

    /// A public user/contributor target that resolves to `User` and
    /// harvests a BOUNDED cross-repo aggregate (US-SCR-001 Ex 2; WD-64).
    pub fn for_public_user(user: &str, signals: Vec<FakeSignal>) -> Self {
        // SCAFFOLD: true
        let _ = (user, signals);
        todo!("DELIVER (slice-02): construct a public-user FakeGithub posture")
    }

    /// A public repo whose harvest yields ZERO signals the mapping can use
    /// (US-SCR-002 Ex 2 — "no candidates derived", exit 0, not an error).
    pub fn with_no_matching_signals(target: &str) -> Self {
        // SCAFFOLD: true
        let _ = target;
        todo!("DELIVER (slice-02): construct a no-matching-signals FakeGithub posture")
    }

    /// A public repo whose harvest yields THREE distinct signals that all
    /// map to the SAME predicate (docs/ + long README + high doc-comment
    /// density => `documentation-first`). Used by SC-3 to assert the
    /// collapse-into-one-candidate behavior (US-SCR-002 Ex 4 / I-SCR-4).
    pub fn with_multi_signal_single_predicate(target: &str) -> Self {
        // SCAFFOLD: true
        let _ = target;
        todo!("DELIVER (slice-02): construct a multi-signal-one-predicate FakeGithub posture")
    }

    /// Mark this fixture authenticated with the supplied remaining/limit
    /// rate budget (US-SCR-004 Ex 1). The token the production code sends is
    /// captured in the `seen_token` observation slot but NEVER echoed.
    pub fn authenticated(self, remaining: u32, limit: u32) -> Self {
        // SCAFFOLD: true
        let _ = (remaining, limit);
        todo!("DELIVER (slice-02): mark FakeGithub authenticated with a rate budget")
    }

    // -------------------------------------------------------------------------
    // Refusal / degradation postures (sad paths; example-only per Mandate 11)
    // -------------------------------------------------------------------------

    /// A target that does not exist (HTTP 404). Resolve fails with
    /// `NotFound`; zero candidates; non-zero exit (US-SCR-001 Ex 3).
    pub fn for_not_found(target: &str) -> Self {
        // SCAFFOLD: true
        let _ = target;
        todo!("DELIVER (slice-02): construct a not-found FakeGithub posture")
    }

    /// A PRIVATE / inaccessible target. The public-only API returns
    /// 404/403; resolve fails with `NotPublic`; "scraper only reads public
    /// data" (US-SCR-001 Ex 4; WD-51 / I-SCR-2). By construction the double
    /// has NO private surface — public-data-only is structural.
    pub fn for_private_target(target: &str) -> Self {
        // SCAFFOLD: true
        let _ = target;
        todo!("DELIVER (slice-02): construct a private-target FakeGithub posture (no private surface)")
    }

    /// No reachable network (the offline posture). Resolve fails with
    /// `Network`; "scrape requires network"; no partial list (US-SCR-001 Ex
    /// 5 / offline UAT). Implemented by binding NO server and pointing the
    /// base URL at a dead port.
    pub fn offline() -> Self {
        // SCAFFOLD: true
        todo!("DELIVER (slice-02): construct an offline FakeGithub posture")
    }

    /// An UNAUTHENTICATED fixture whose harvest exhausts the anonymous rate
    /// budget mid-way (HTTP 403 + rate-limit headers). "set GITHUB_TOKEN for
    /// higher limits"; no partial candidate list (US-SCR-004 Ex 3). `target`
    /// resolves fine; the rate limit trips during harvest.
    pub fn rate_limited_anon(target: &str) -> Self {
        // SCAFFOLD: true
        let _ = target;
        todo!("DELIVER (slice-02): construct a rate-limited-anonymous FakeGithub posture")
    }

    /// A fixture that REJECTS the supplied token (HTTP 401). The CLI exits
    /// non-zero with the 401 explanation; the token value is NEVER echoed
    /// (US-SCR-004 Ex 4). `target` is irrelevant — the auth check fails
    /// first.
    pub fn with_rejected_token(target: &str) -> Self {
        // SCAFFOLD: true
        let _ = target;
        todo!("DELIVER (slice-02): construct a rejected-token FakeGithub posture")
    }

    // -------------------------------------------------------------------------
    // Runtime + observation
    // -------------------------------------------------------------------------

    /// Spin up an in-process HTTP server on a random `127.0.0.1` port and
    /// return its base URL + an `AbortOnDrop` handle. The base URL is fed to
    /// `adapter-github` via `OPENLORE_GITHUB_API_BASE` (the test-only seam,
    /// mirroring the slice-03 peer-endpoint seam). The server stops when the
    /// returned handle drops (RAII per-scenario isolation).
    pub fn serve_http(&self) -> FakeGithubHttpHandle {
        // SCAFFOLD: true
        todo!("DELIVER (slice-02): serve_http on a random 127.0.0.1 port (AbortOnDrop guard)")
    }

    /// The auth posture this fixture was constructed with (anonymous vs
    /// authenticated + budget). Used by SA-1/SA-2 to assert the reported
    /// auth-line matches the posture.
    pub fn auth_mode(&self) -> FakeAuthMode {
        // SCAFFOLD: true
        todo!("DELIVER (slice-02): expose the constructed auth posture")
    }

    /// `true` iff the production code sent the given token value to the fake
    /// (so a scenario can assert auth genuinely happened) WITHOUT the value
    /// ever reaching captured output. Pairs with the no-token-leak assertion
    /// that the same value is ABSENT from stdout/stderr (US-SCR-004).
    pub fn saw_token(&self, token: &str) -> bool {
        // SCAFFOLD: true
        let _ = token;
        todo!("DELIVER (slice-02): report whether the production code sent the token")
    }

    /// Every request PATH the production code hit, in order. The
    /// `scraper_only_reads_public_data` gate asserts every entry is on the
    /// public-endpoint allowlist (no private path; KPI-SCR-4 / I-SCR-2).
    pub fn seen_paths(&self) -> Vec<String> {
        // SCAFFOLD: true
        todo!("DELIVER (slice-02): report the request paths the production code hit")
    }
}

/// RAII handle for the [`FakeGithub::serve_http`] background server.
///
/// Holds the bound base URL (injected via `OPENLORE_GITHUB_API_BASE`) and a
/// guard that aborts the server task on drop — byte-for-byte the same shape
/// as [`crate::FakePds::serve_http`]'s handle (DD-SCR-2 reuse-the-pattern).
pub struct FakeGithubHttpHandle {
    /// The `http://127.0.0.1:<port>` base URL the adapter resolves against.
    base_url: String,
    /// Abort guard; aborting stops the background server on drop.
    _abort: AbortOnDrop,
}

impl FakeGithubHttpHandle {
    /// The base URL to feed `adapter-github` via `OPENLORE_GITHUB_API_BASE`.
    pub fn base_url(&self) -> &str {
        // SCAFFOLD: true
        todo!("DELIVER (slice-02): return the bound base URL")
    }
}

/// Aborts a spawned tokio task when dropped (RAII server shutdown). Mirrors
/// the slice-01/03 `FakePds`/`FakePeerPds` AbortOnDrop guard exactly.
struct AbortOnDrop {
    _scaffold: (),
}

impl Drop for AbortOnDrop {
    fn drop(&mut self) {
        // SCAFFOLD: true — no-op until DELIVER wires the real abort handle.
    }
}
