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
    /// The REAL `/repos/{owner}/{repo}` `language` field (RGSD-1). `Some`
    /// only for the realistic-body posture `for_public_repo_with_language`,
    /// which serves a live-shaped repo body (a `language` string, NO
    /// synthetic `signals[]`) mirroring what the real GitHub API returns.
    /// `None` for every legacy `signals[]`-driven posture (so those bodies
    /// serialize `"language": null` and stay untouched).
    language: Option<String>,
    /// Whether this repo has a committed `Cargo.lock` (RGSD-2). `true` only
    /// for the `for_public_repo_with_cargo_lock` posture: the HTTP handler
    /// then serves `GET /repos/{o}/{r}/contents/Cargo.lock` → **200** with a
    /// realistic file body carrying the file `html_url` (200 = present, the
    /// real GitHub `contents` API shape). `false` for every OTHER posture (so
    /// the same probe → **404** = absent). Combined with the default-404 rule
    /// for ALL unconfigured `contents/*` paths, this makes every existing
    /// posture read as "no Cargo.lock" for the new probe (no regression).
    has_cargo_lock: bool,
    /// The repo's tag names, as `GET /repos/{o}/{r}/tags` returns them (RGSD-3).
    /// The HTTP handler serves this as a JSON array of `{"name": <tag>}` objects
    /// (the real GitHub `tags` shape). **Empty by default** for EVERY posture
    /// that does not set it — so the `/tags` endpoint serves `[]` (no tags) and
    /// no `SemverAndChangelog` signal can ever fire on an unconfigured posture
    /// (no regression). Set (with a clearly-semver name like `v1.2.3`) only by
    /// the `for_public_repo_with_tags_and_changelog` family (RGSD-3 postures).
    tags: Vec<String>,
    /// Whether this repo has a committed `CHANGELOG.md` (RGSD-3). `true` serves
    /// `GET /repos/{o}/{r}/contents/CHANGELOG.md` → **200** with a realistic
    /// file body carrying the file `html_url` (200 = present); `false` (the
    /// default for EVERY posture that does not set it) → **404** = absent, via
    /// the same default-404 `contents/*` rule as `Cargo.lock`. The
    /// `SemverAndChangelog` signal is the CONJUNCTION of a semver `tags` entry
    /// AND a present CHANGELOG — neither half alone fires it.
    has_changelog: bool,
    /// The README byte size `GET /repos/{o}/{r}/readme` reports (RGSD-4).
    /// `Some(size)` only for a posture that declares a README: the HTTP handler
    /// then serves `/readme` → **200** `{"name":"README.md","size":<size>,
    /// "html_url":".../blob/master/README.md"}` (the real GitHub `readme` shape,
    /// which carries the file `size` in bytes). `None` (the default for EVERY
    /// posture that does not set it) → **404** = no README. The
    /// `DocsPresentAndSubstantial` signal fires when the README is SUBSTANTIAL
    /// (`size >= README_SUBSTANTIAL_BYTES`, a DELIVER threshold ~3000) OR a
    /// `docs/` dir is present — a DISJUNCTION (design §5).
    readme_bytes: Option<u64>,
    /// Whether this repo has a `docs/` directory (RGSD-4). `true` serves
    /// `GET /repos/{o}/{r}/contents/docs` → **200** with a realistic dir-listing
    /// body (a JSON ARRAY of entries — the real GitHub `contents` shape for a
    /// directory) carrying an `html_url`; `false` (the default for EVERY posture
    /// that does not set it) → **404** = absent, via the same default-404
    /// `contents/*` rule as `Cargo.lock` / `CHANGELOG.md`. The `docs/` dir is
    /// the SECOND disjunct of `DocsPresentAndSubstantial` — a `docs/` dir alone
    /// fires the signal even when the README is tiny/absent.
    has_docs_dir: bool,
    /// Whether this repo has a `.github/workflows` CI directory (RGSD-5). `true`
    /// serves `GET /repos/{o}/{r}/contents/.github/workflows` → **200** with a
    /// realistic dir-listing body (a JSON ARRAY of entries — the real GitHub
    /// `contents` shape for a directory) carrying an `html_url`; `false` (the
    /// default for EVERY posture that does not set it) → **404** = absent, via the
    /// same default-404 `contents/*` rule as `docs`. NOTE the probed path
    /// (`.github/workflows`) carries a dot AND a slash: the `contents/*` router
    /// matches on the FULL suffix after `contents/`, not the last segment. CI
    /// workflows are the FIRST disjunct of `TestRatioOrCiMatrix` — CI workflows
    /// present ALONE fires the signal even when there is no `tests/` dir.
    has_ci_workflows: bool,
    /// Whether this repo has a `tests/` directory (RGSD-5). `true` serves `GET
    /// /repos/{o}/{r}/contents/tests` → **200** with a realistic dir-listing body
    /// (a JSON ARRAY — the real GitHub `contents` directory shape) carrying an
    /// `html_url`; `false` (the default for EVERY posture that does not set it) →
    /// **404** = absent, via the same default-404 `contents/*` rule. A `tests/`
    /// dir is the SECOND disjunct of `TestRatioOrCiMatrix` — a `tests/` dir ALONE
    /// fires the signal even when there are no CI workflows.
    has_tests_dir: bool,
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
/// `for_public_repo_with_*` real-fact builders, `for_not_found`,
/// `for_private_target`, `offline`, `rate_limited_anon`, `with_rejected_token`,
/// `authenticated`, `with_no_matching_signals`), then `serve_http()` to obtain
/// a base URL the `adapter-github` resolves against via the
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
        auth: FakeAuthMode,
    ) -> Self {
        Self {
            state: Arc::new(State {
                target: target.to_string(),
                resolution,
                auth,
                language: None,
                has_cargo_lock: false,
                tags: Vec::new(),
                has_changelog: false,
                readme_bytes: None,
                has_docs_dir: false,
                has_ci_workflows: false,
                has_tests_dir: false,
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

    /// A public repo target that resolves to `Repo` (with NO synthetic
    /// signals — a scrape drives REAL detection over the served body). Serves
    /// `"language": null` + no configured facts, so detection fires nothing:
    /// use the `for_public_repo_with_*` postures to configure the real facts a
    /// detector reads. `auth` defaults to anonymous; chain `.authenticated(...)`.
    pub fn for_public_repo(target: &str) -> Self {
        Self::from_state(
            target,
            Ok(Self::resolve_kind(target)),
            FakeAuthMode::Anonymous,
        )
    }

    /// A public user/contributor target that resolves to `User` (US-SCR-001
    /// Ex 2; WD-64). The bounded cross-repo USER aggregate is DEFERRED to
    /// slice-04, so a user scrape derives no signals — the target resolves and
    /// the auth posture is reported, but no candidates are proposed.
    pub fn for_public_user(user: &str) -> Self {
        Self::from_state(
            user,
            Ok(FakeTargetKind::User {
                user: user.to_string(),
            }),
            FakeAuthMode::Anonymous,
        )
    }

    /// A public repo that resolves but yields ZERO detectable signals
    /// (US-SCR-002 Ex 2 — "no candidates derived", exit 0, not an error). No
    /// facts are configured, so `detect_signals` fires no arm.
    pub fn with_no_matching_signals(target: &str) -> Self {
        Self::from_state(
            target,
            Ok(Self::resolve_kind(target)),
            FakeAuthMode::Anonymous,
        )
    }

    /// A public repo that serves a REALISTIC `/repos/{owner}/{repo}` body
    /// (RGSD-1 walking skeleton): a top-level `language` string (e.g.
    /// `"Rust"`, `"C++"`) plus `html_url`, and NO synthetic `signals[]`
    /// array — exactly the shape the LIVE GitHub API returns.
    ///
    /// This is the posture the language-based `MemorySafetyLanguage`
    /// detection is exercised against: the real API never provides the
    /// synthetic `signals[]` field the legacy `for_public_repo` postures
    /// inject, so a scrape of this body yields ZERO signals today (harvest
    /// reads the absent `signals[]`) — the RGSD-1 RED. Once detection lands
    /// in DELIVER, `parse_repo_facts` + `detect_signals` read the `language`
    /// field and fire the `MemorySafetyLanguage` signal for a memory-safe
    /// language, deriving the `org.openlore.philosophy.memory-safety`
    /// candidate.
    ///
    /// Additive: every existing `signals[]`-driven posture is untouched
    /// (their bodies serialize `"language": null`), so the legacy scrape
    /// acceptance suite stays green through the §4 union bridge.
    pub fn for_public_repo_with_language(target: &str, language: &str) -> Self {
        Self {
            state: Arc::new(State {
                target: target.to_string(),
                resolution: Ok(Self::resolve_kind(target)),
                language: Some(language.to_string()),
                has_cargo_lock: false,
                tags: Vec::new(),
                has_changelog: false,
                readme_bytes: None,
                has_docs_dir: false,
                has_ci_workflows: false,
                has_tests_dir: false,
                auth: FakeAuthMode::Anonymous,
                seen_token: Mutex::new(None),
                seen_paths: Mutex::new(Vec::new()),
                offline: AtomicBool::new(false),
            }),
        }
    }

    /// A public repo that has a committed `Cargo.lock` at its root (RGSD-2):
    /// the HTTP handler serves `GET /repos/{owner}/{repo}/contents/Cargo.lock`
    /// → **200** with a realistic `contents` file body carrying the file
    /// `html_url` (`https://github.com/{owner}/{repo}/blob/master/Cargo.lock`),
    /// exactly the shape the LIVE GitHub `contents` API returns for a present
    /// file (SPIKE-verified: ripgrep → 200; torvalds/linux → 404).
    ///
    /// This is the posture the `DependencyManifestPinned` detection is
    /// exercised against: RGSD-2's `harvest_repo` will issue a SECOND request
    /// (`content_exists(owner, repo, "Cargo.lock")`), reading **200 = present**
    /// / **404 = absent**, and fire the `DependencyManifestPinned` signal when
    /// the file is present → deriving the
    /// `org.openlore.philosophy.dependency-pinning` candidate. The repo carries
    /// NO `language` (so ONLY dependency-pinning fires, isolating the signal
    /// from the RGSD-1 memory-safety detection) and NO synthetic `signals[]`.
    ///
    /// Additive: every existing posture keeps `has_cargo_lock == false`, so its
    /// `contents/Cargo.lock` probe → **404** (absent) and the legacy scrape
    /// suite is untouched. Any UNCONFIGURED `contents/*` path also → **404** by
    /// construction (see [`contents_response`]).
    pub fn for_public_repo_with_cargo_lock(target: &str) -> Self {
        Self {
            state: Arc::new(State {
                target: target.to_string(),
                resolution: Ok(Self::resolve_kind(target)),
                language: None,
                has_cargo_lock: true,
                tags: Vec::new(),
                has_changelog: false,
                readme_bytes: None,
                has_docs_dir: false,
                has_ci_workflows: false,
                has_tests_dir: false,
                auth: FakeAuthMode::Anonymous,
                seen_token: Mutex::new(None),
                seen_paths: Mutex::new(Vec::new()),
                offline: AtomicBool::new(false),
            }),
        }
    }

    /// A public repo whose `GET /repos/{o}/{r}/tags` lists the supplied tag
    /// names AND whose `contents/CHANGELOG.md` probe reflects `has_changelog`
    /// (RGSD-3). This is the posture family the `SemverAndChangelog` detection
    /// is exercised against — the signal is the CONJUNCTION of (a) a semver
    /// tag among the listed tags AND (b) a present CHANGELOG.
    ///
    /// The HTTP handler serves:
    /// - `GET /repos/{o}/{r}/tags` → **200** with a JSON array of
    ///   `{"name": <tag>}` objects from `tags` (the real GitHub `tags` shape);
    /// - `GET /repos/{o}/{r}/contents/CHANGELOG.md` → **200** with a realistic
    ///   `contents` file body carrying the file `html_url` when `has_changelog`,
    ///   else **404** (absent) via the same default-404 `contents/*` rule.
    ///
    /// The repo carries NO `language` and NO committed `Cargo.lock` (so ONLY
    /// semver-and-changelog can fire, isolating it from the RGSD-1 memory-safety
    /// and RGSD-2 dependency-pinning detections) and NO synthetic `signals[]`.
    ///
    /// Additive: every existing posture keeps `tags == []` (so `/tags` serves
    /// `[]`) and `has_changelog == false` (so `contents/CHANGELOG.md` → 404), so
    /// no `SemverAndChangelog` signal can fire on any existing posture — the
    /// legacy scrape suite is untouched.
    pub fn for_public_repo_with_tags_and_changelog(
        target: &str,
        tags: Vec<&str>,
        has_changelog: bool,
    ) -> Self {
        Self {
            state: Arc::new(State {
                target: target.to_string(),
                resolution: Ok(Self::resolve_kind(target)),
                language: None,
                has_cargo_lock: false,
                tags: tags.into_iter().map(str::to_string).collect(),
                has_changelog,
                readme_bytes: None,
                has_docs_dir: false,
                has_ci_workflows: false,
                has_tests_dir: false,
                auth: FakeAuthMode::Anonymous,
                seen_token: Mutex::new(None),
                seen_paths: Mutex::new(Vec::new()),
                offline: AtomicBool::new(false),
            }),
        }
    }

    /// A public repo that BOTH follows semver in its tags (`v1.2.3`, …) AND
    /// commits a `CHANGELOG.md` (RGSD-3 happy posture) — the CONJUNCTION that
    /// fires the `SemverAndChangelog` signal → deriving the
    /// `org.openlore.philosophy.semantic-versioning` candidate. SPIKE-verified
    /// against real GitHub (ripgrep: semver-style `tags` + `contents/CHANGELOG.md`
    /// → 200). Convenience over `for_public_repo_with_tags_and_changelog`.
    pub fn for_public_repo_with_semver_and_changelog(target: &str) -> Self {
        Self::for_public_repo_with_tags_and_changelog(
            target,
            vec!["v1.2.3", "v1.2.2", "v1.0.0"],
            true,
        )
    }

    /// A public repo whose documentation evidence is declared directly (RGSD-4):
    /// an optional README byte size (`readme_bytes`) served by `GET
    /// /repos/{o}/{r}/readme`, and whether a `docs/` directory is present
    /// (`has_docs_dir`) served by `GET /repos/{o}/{r}/contents/docs`. This is the
    /// general builder the `DocsPresentAndSubstantial` detection is exercised
    /// against — the signal is the DISJUNCTION of (a) a SUBSTANTIAL README
    /// (`size >= README_SUBSTANTIAL_BYTES`, a DELIVER threshold ~3000) OR (b) a
    /// present `docs/` dir (design §5). Either disjunct alone fires it.
    ///
    /// The HTTP handler serves:
    /// - `GET /repos/{o}/{r}/readme` → **200** `{"name":"README.md","size":
    ///   <readme_bytes>,"html_url":".../blob/master/README.md"}` when
    ///   `readme_bytes` is `Some` (the real GitHub `readme` shape, carrying the
    ///   file `size` in bytes), else **404** (no README);
    /// - `GET /repos/{o}/{r}/contents/docs` → **200** with a realistic dir-listing
    ///   JSON array carrying an `html_url` when `has_docs_dir`, else **404**
    ///   (absent) via the same default-404 `contents/*` rule.
    ///
    /// The repo carries NO `language`, NO `Cargo.lock`, NO `tags`, NO CHANGELOG
    /// (so ONLY documentation-first can fire, isolating it from RGSD-1/2/3) and
    /// NO synthetic `signals[]`.
    ///
    /// Additive: every existing posture keeps `readme_bytes == None` (so
    /// `/readme` → **404**) and `has_docs_dir == false` (so `contents/docs` →
    /// **404**), so no `DocsPresentAndSubstantial` signal can fire on any existing
    /// posture — the legacy scrape suite is untouched.
    pub fn for_public_repo_with_docs_evidence(
        target: &str,
        readme_bytes: Option<u64>,
        has_docs_dir: bool,
    ) -> Self {
        Self {
            state: Arc::new(State {
                target: target.to_string(),
                resolution: Ok(Self::resolve_kind(target)),
                language: None,
                has_cargo_lock: false,
                tags: Vec::new(),
                has_changelog: false,
                readme_bytes,
                has_docs_dir,
                has_ci_workflows: false,
                has_tests_dir: false,
                auth: FakeAuthMode::Anonymous,
                seen_token: Mutex::new(None),
                seen_paths: Mutex::new(Vec::new()),
                offline: AtomicBool::new(false),
            }),
        }
    }

    /// A public repo with a SUBSTANTIAL README (`size_bytes` large, e.g. 20000)
    /// and NO `docs/` dir (RGSD-4 happy A) — the README-half disjunct of
    /// `DocsPresentAndSubstantial`. SPIKE-verified against real GitHub (ripgrep:
    /// `/readme` `size` 21615 + `html_url`, `contents/docs` → 404). Fires the
    /// `documentation-first` candidate via a substantial README ALONE.
    pub fn for_public_repo_with_readme(target: &str, size_bytes: u64) -> Self {
        Self::for_public_repo_with_docs_evidence(target, Some(size_bytes), false)
    }

    /// A public repo with a `docs/` directory present but a tiny/absent README
    /// (RGSD-4 happy B) — the docs-dir disjunct of `DocsPresentAndSubstantial`.
    /// Pins the OR: a `docs/` dir ALONE fires the `documentation-first` candidate
    /// even when the README is absent (`/readme` → 404).
    pub fn for_public_repo_with_docs_dir(target: &str) -> Self {
        Self::for_public_repo_with_docs_evidence(target, None, true)
    }

    /// A public repo whose test evidence is declared directly (RGSD-5): whether a
    /// `.github/workflows` CI directory is present (`has_ci_workflows`, served by
    /// `GET /repos/{o}/{r}/contents/.github/workflows`) and whether a `tests/`
    /// directory is present (`has_tests_dir`, served by `GET
    /// /repos/{o}/{r}/contents/tests`). This is the general builder the
    /// `TestRatioOrCiMatrix` detection is exercised against — the signal is the
    /// DISJUNCTION of (a) CI workflows present OR (b) a `tests/` dir present
    /// (design §5). Either disjunct alone fires it. Both probes reuse RGSD-2's
    /// `contents/*` fork (200 = present as a dir-listing JSON ARRAY, 404 = absent)
    /// — NO new endpoint type. The precise "test/source ratio > 0.5" precision
    /// (needs a full recursive tree walk) is DEFERRED; the walking skeleton uses
    /// the cheap directory-presence proxy (design §3 — honest semantics).
    ///
    /// The HTTP handler serves:
    /// - `GET /repos/{o}/{r}/contents/.github/workflows` → **200** with a realistic
    ///   dir-listing JSON array carrying an `html_url` when `has_ci_workflows`, else
    ///   **404** (absent) via the same default-404 `contents/*` rule;
    /// - `GET /repos/{o}/{r}/contents/tests` → **200** with a realistic dir-listing
    ///   JSON array when `has_tests_dir`, else **404** (absent).
    ///
    /// The repo carries NO `language`, NO `Cargo.lock`, NO `tags`, NO CHANGELOG,
    /// NO README, NO `docs/` dir (so ONLY test-driven can fire, isolating it from
    /// RGSD-1/2/3/4) and NO synthetic `signals[]`.
    ///
    /// Additive: every existing posture keeps `has_ci_workflows == false` (so
    /// `contents/.github/workflows` → **404**) and `has_tests_dir == false` (so
    /// `contents/tests` → **404**), so no `TestRatioOrCiMatrix` signal can fire on
    /// any existing posture — the legacy scrape suite is untouched.
    pub fn for_public_repo_with_test_evidence(
        target: &str,
        has_ci_workflows: bool,
        has_tests_dir: bool,
    ) -> Self {
        Self {
            state: Arc::new(State {
                target: target.to_string(),
                resolution: Ok(Self::resolve_kind(target)),
                language: None,
                has_cargo_lock: false,
                tags: Vec::new(),
                has_changelog: false,
                readme_bytes: None,
                has_docs_dir: false,
                has_ci_workflows,
                has_tests_dir,
                auth: FakeAuthMode::Anonymous,
                seen_token: Mutex::new(None),
                seen_paths: Mutex::new(Vec::new()),
                offline: AtomicBool::new(false),
            }),
        }
    }

    /// A public repo with a `.github/workflows` CI directory present but NO
    /// `tests/` dir (RGSD-5 happy A) — the CI-workflows disjunct of
    /// `TestRatioOrCiMatrix`. SPIKE-verified against real GitHub (ripgrep:
    /// `contents/.github/workflows` → 200). Fires the `test-driven` candidate via
    /// CI workflows ALONE.
    pub fn for_public_repo_with_ci_workflows(target: &str) -> Self {
        Self::for_public_repo_with_test_evidence(target, true, false)
    }

    /// A public repo with a `tests/` directory present but NO CI workflows
    /// (RGSD-5 happy B) — the tests-dir disjunct of `TestRatioOrCiMatrix`. Pins
    /// the OR: a `tests/` dir ALONE fires the `test-driven` candidate even when
    /// there are no CI workflows. SPIKE-verified against real GitHub (ripgrep:
    /// `contents/tests` → 200).
    pub fn for_public_repo_with_tests_dir(target: &str) -> Self {
        Self::for_public_repo_with_test_evidence(target, false, true)
    }

    /// A public repo whose REAL metadata fires ALL FIVE bounded signals via
    /// genuine detection — the realistic replacement for the synthetic
    /// `for_public_repo(target, signals)` scaffold (RGSD-6). Every fact the
    /// five detectors read is configured on ONE posture so a scrape of this
    /// body yields five candidates through `detect_signals` alone (NO synthetic
    /// `signals[]`):
    ///
    /// - `language = "Rust"` → `MemorySafetyLanguage` (a memory-safety language);
    /// - a committed `Cargo.lock` (`has_cargo_lock`) → `DependencyManifestPinned`;
    /// - semver `tags` (`v1.2.3`, …) AND a present `CHANGELOG.md`
    ///   (`has_changelog`) → `SemverAndChangelog` (the conjunction);
    /// - a SUBSTANTIAL README (`readme_bytes >= README_SUBSTANTIAL_BYTES`, here
    ///   20000) → `DocsPresentAndSubstantial` (the README disjunct);
    /// - a `.github/workflows` CI directory (`has_ci_workflows`) →
    ///   `TestRatioOrCiMatrix` (the CI disjunct).
    ///
    /// `has_docs_dir` / `has_tests_dir` stay `false` — the README and CI
    /// disjuncts alone fire docs / test-driven, so the two second disjuncts are
    /// deliberately left off to keep the posture minimal. This posture drives
    /// REAL detection end-to-end and is what STEP 2 migrates the multi-signal
    /// acceptance tests onto (retiring the `signals[]` scaffold).
    pub fn for_public_repo_with_all_signals(target: &str) -> Self {
        Self {
            state: Arc::new(State {
                target: target.to_string(),
                resolution: Ok(Self::resolve_kind(target)),
                language: Some("Rust".to_string()),
                has_cargo_lock: true,
                tags: vec![
                    "v1.2.3".to_string(),
                    "v1.2.2".to_string(),
                    "v1.0.0".to_string(),
                ],
                has_changelog: true,
                readme_bytes: Some(20000),
                has_docs_dir: false,
                has_ci_workflows: true,
                has_tests_dir: false,
                auth: FakeAuthMode::Anonymous,
                seen_token: Mutex::new(None),
                seen_paths: Mutex::new(Vec::new()),
                offline: AtomicBool::new(false),
            }),
        }
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
                language: prev.language.clone(),
                has_cargo_lock: prev.has_cargo_lock,
                tags: prev.tags.clone(),
                has_changelog: prev.has_changelog,
                readme_bytes: prev.readme_bytes,
                has_docs_dir: prev.has_docs_dir,
                has_ci_workflows: prev.has_ci_workflows,
                has_tests_dir: prev.has_tests_dir,
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
        Ok(kind) => {
            // RGSD-2: the `contents/{path}` probe is a SEPARATE endpoint from
            // the repo resolve/harvest body — route it explicitly. Without this
            // fork every path on a resolvable target falls through to the 200
            // repo body, so a `content_exists` probe would read EVERY repo as
            // "file present". Instead: 200 iff the file is configured-present
            // (only Cargo.lock, only for the cargo-lock posture); 404 (absent)
            // for every other `contents/*` path AND for Cargo.lock when the
            // posture has none. That default-404 is what keeps all existing
            // postures reading as "no Cargo.lock" (no regression).
            if path.contains("/contents/") {
                Ok(contents_response(&fake, kind, &path))
            } else if path.ends_with("/tags") {
                // RGSD-3: `GET /repos/{o}/{r}/tags` is a SEPARATE endpoint from
                // the repo resolve/harvest body — route it explicitly so a
                // `list_tags` probe reads the posture's tag list. Served for
                // EVERY resolvable posture; unconfigured postures carry `tags ==
                // []` → `[]` (no tags), so `SemverAndChangelog` can never fire on
                // them (no regression).
                Ok(tags_response(&fake))
            } else if path.ends_with("/readme") {
                // RGSD-4: `GET /repos/{o}/{r}/readme` is a SEPARATE endpoint from
                // the repo resolve/harvest body — route it explicitly so a
                // `fetch_readme` probe reads the posture's README size. **200**
                // with the file `size` when the posture declares a README
                // (`readme_bytes` is `Some`), else **404** (no README). Every
                // unconfigured posture keeps `readme_bytes == None` → 404, so no
                // `DocsPresentAndSubstantial` signal can fire on them (no
                // regression).
                Ok(readme_response(&fake, kind))
            } else {
                Ok(resolve_or_harvest_response(&fake, kind, &path))
            }
        }
        Err(posture) => Ok(error_response(posture)),
    }
}

/// Serve the `GET /repos/{owner}/{repo}/contents/{file_path}` probe (RGSD-2/3).
///
/// **200** with a realistic `contents` file body (carrying the file `html_url`)
/// iff the probed file is a CONFIGURED present file — `Cargo.lock` when the
/// posture declared a committed Cargo.lock (`has_cargo_lock`, RGSD-2), or
/// `CHANGELOG.md` when the posture declared a committed CHANGELOG
/// (`has_changelog`, RGSD-3). Every OTHER `contents/*` path, and a configured
/// file on a posture without it, → **404** (absent), the real GitHub `contents`
/// "no such file" shape. This default-404 is structural: an unconfigured
/// content path can never accidentally read as present.
fn contents_response(fake: &FakeGithub, kind: &FakeTargetKind, path: &str) -> HttpResponse {
    // Extract the file path after `/contents/` (e.g. `Cargo.lock`, `CHANGELOG.md`).
    let file_path = path
        .split_once("/contents/")
        .map(|(_, rest)| rest)
        .unwrap_or("");

    // A file/dir is present iff it is a configured-present path for this posture.
    let present = match file_path {
        "Cargo.lock" => fake.state.has_cargo_lock,
        "CHANGELOG.md" => fake.state.has_changelog,
        // RGSD-4: `docs` is a DIRECTORY (not a file) — present iff the posture
        // declares a `docs/` dir. GitHub returns a JSON ARRAY for a directory.
        "docs" => fake.state.has_docs_dir,
        // RGSD-5: `.github/workflows` + `tests` are DIRECTORIES — present iff the
        // posture declares CI workflows / a `tests/` dir. NOTE `.github/workflows`
        // carries a dot AND a slash: matching on the FULL suffix after
        // `contents/` (this `file_path`, not the last segment) resolves it.
        ".github/workflows" => fake.state.has_ci_workflows,
        "tests" => fake.state.has_tests_dir,
        _ => false,
    };
    if present {
        let (owner, repo) = match kind {
            FakeTargetKind::Repo { owner, repo } => (owner.clone(), repo.clone()),
            // `contents` is a repo endpoint; a user target has no such path,
            // but stay total and mirror the login as owner/repo.
            FakeTargetKind::User { user } => (user.clone(), user.clone()),
        };
        // RGSD-4: a DIRECTORY (`docs`) resolves to a JSON ARRAY of entries — the
        // real GitHub `contents` shape for a directory. `content_exists` treats
        // any 200 as "present"; the entries carry an `html_url` the adapter can
        // capture as the `docs_url` evidence. A FILE resolves to a single object.
        // RGSD-4/5: `docs`, `.github/workflows`, and `tests` are DIRECTORIES —
        // GitHub returns a JSON ARRAY of entries for a directory (a FILE returns a
        // single object). `content_exists` treats any 200 as "present"; the
        // entries carry an `html_url` the adapter can capture as evidence.
        let is_directory = matches!(file_path, "docs" | ".github/workflows" | "tests");
        if is_directory {
            let dir_url = format!("https://github.com/{owner}/{repo}/tree/master/{file_path}");
            let entry_path = format!("{file_path}/index.md");
            let entry_url = format!("https://github.com/{owner}/{repo}/blob/master/{entry_path}");
            return json_response(
                200,
                serde_json::json!([
                    {
                        "name": "index.md",
                        "path": entry_path,
                        "type": "file",
                        "html_url": entry_url,
                        "_dir_html_url": dir_url,
                    }
                ]),
            );
        }
        let html_url = format!("https://github.com/{owner}/{repo}/blob/master/{file_path}");
        return json_response(
            200,
            serde_json::json!({
                "name": file_path,
                "path": file_path,
                "type": "file",
                "html_url": html_url,
            }),
        );
    }

    // Unconfigured content path (or a configured file absent) → 404 = absent.
    json_response(404, serde_json::json!({ "message": "Not Found" }))
}

/// Serve the `GET /repos/{owner}/{repo}/tags` list probe (RGSD-3).
///
/// **200** with a JSON array of `{"name": <tag>}` objects from the posture's
/// `tags` list — the real GitHub `tags` API shape. **Empty (`[]`)** for every
/// posture that does not set `tags` (the default), so an unconfigured posture
/// lists no tags and the `SemverAndChangelog` signal can never fire on it (no
/// regression). Always a 200 (a public repo's tags endpoint never 404s — an
/// untagged repo simply returns `[]`).
fn tags_response(fake: &FakeGithub) -> HttpResponse {
    let tags: Vec<serde_json::Value> = fake
        .state
        .tags
        .iter()
        .map(|name| serde_json::json!({ "name": name }))
        .collect();
    json_response(200, serde_json::Value::Array(tags))
}

/// Serve the `GET /repos/{owner}/{repo}/readme` probe (RGSD-4).
///
/// **200** with `{"name":"README.md","size":<readme_bytes>,"html_url":
/// ".../blob/master/README.md"}` — the real GitHub `readme` API shape, which
/// carries the README `size` in bytes — when the posture declares a README
/// (`readme_bytes` is `Some`). **404** (no README) for every posture that does
/// not set it (the default). The `DocsPresentAndSubstantial` detection reads
/// the `size` to decide whether the README is SUBSTANTIAL (design §5).
fn readme_response(fake: &FakeGithub, kind: &FakeTargetKind) -> HttpResponse {
    match fake.state.readme_bytes {
        Some(size) => {
            let (owner, repo) = match kind {
                FakeTargetKind::Repo { owner, repo } => (owner.clone(), repo.clone()),
                FakeTargetKind::User { user } => (user.clone(), user.clone()),
            };
            let html_url = format!("https://github.com/{owner}/{repo}/blob/master/README.md");
            json_response(
                200,
                serde_json::json!({
                    "name": "README.md",
                    "path": "README.md",
                    "type": "file",
                    "size": size,
                    "html_url": html_url,
                }),
            )
        }
        // No README declared → 404, exactly as the real GitHub `readme` endpoint
        // returns for a repo with no README at all.
        None => json_response(404, serde_json::json!({ "message": "Not Found" })),
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

    // RGSD-1: the REAL `/repos` body carries a top-level `language` string +
    // `html_url`. `language` is `Some` only for the realistic-body postures
    // (`for_public_repo_with_language` / `_with_all_signals`); an unconfigured
    // posture serves `"language": null`. `html_url` mirrors the public
    // repo/user URL (the source_url a language-derived signal names as evidence).
    // NO synthetic `signals[]` is ever served — a scrape drives REAL detection
    // over the served facts (RGSD-6).
    let html_url = match kind {
        FakeTargetKind::Repo { owner, repo } => format!("https://github.com/{owner}/{repo}"),
        FakeTargetKind::User { user } => format!("https://github.com/{user}"),
    };
    let language_json = match &fake.state.language {
        Some(language) => serde_json::Value::String(language.clone()),
        None => serde_json::Value::Null,
    };

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
            "language": language_json,
            "html_url": html_url,
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
        // A private/inaccessible target serves the PUBLIC-API 404 shape (the
        // public API deliberately 404s a private repo to avoid leaking its
        // existence) PLUS a `private: true` discriminator on the body — the
        // SG-5 seam (step 03-05) the adapter keys on to classify
        // `GithubError::NotPublic` (vs a bare 404 => `NotFound`). There is
        // still NO private surface served (no signals); public-data-only stays
        // structural. The discriminator only distinguishes the REFUSAL cause so
        // the CLI can reassure "the scraper only reads public data" (WD-51 /
        // I-SCR-2) rather than the generic not-found message.
        FakeGithubErrorPosture::NotPublic => serde_json::json!({
            "message": posture.message(),
            "private": true,
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

    /// `for_public_repo` resolves to a Repo at a 200 and serves NO synthetic
    /// `signals[]` (RGSD-6): the body carries only the real repo shape
    /// (`target` + `language: null`), so a scrape drives REAL detection.
    #[tokio::test]
    async fn for_public_repo_resolves_at_200_with_no_synthetic_signals() {
        let fake = FakeGithub::for_public_repo("rust-lang/cargo");
        let handle = fake.serve_http().await;

        let (status, body) =
            get_json(&format!("{}/repos/rust-lang/cargo", handle.base_url())).await;

        assert_eq!(status, 200, "public repo must resolve at 200");
        assert_eq!(body["target"]["kind"], "repo");
        assert_eq!(body["target"]["full_name"], "rust-lang/cargo");
        assert!(
            body.get("signals").is_none(),
            "the body carries NO synthetic signals[] — a scrape drives REAL detection"
        );
        assert_eq!(body["auth"]["authenticated"], false);
    }

    /// RGSD-1: `for_public_repo_with_language` serves a REALISTIC `/repos`
    /// body — a top-level `language` string + `html_url`, and NO synthetic
    /// `signals[]` (the real API never provides synthetic signals). This is the
    /// load-bearing shape the language-based detection reads; an unconfigured
    /// posture keeps `"language": null`.
    #[tokio::test]
    async fn for_public_repo_with_language_serves_language_and_no_signals() {
        let fake = FakeGithub::for_public_repo_with_language("rust-lang/cargo", "Rust");
        let handle = fake.serve_http().await;

        let (status, body) =
            get_json(&format!("{}/repos/rust-lang/cargo", handle.base_url())).await;

        assert_eq!(status, 200, "a public repo must resolve at 200");
        assert_eq!(body["target"]["kind"], "repo");
        assert_eq!(
            body["language"], "Rust",
            "the realistic body must carry the real `language` field"
        );
        assert_eq!(
            body["html_url"], "https://github.com/rust-lang/cargo",
            "the realistic body must carry the repo `html_url` (signal source_url)"
        );
        assert!(
            body.get("signals").is_none(),
            "the realistic body carries NO synthetic signals (real API shape)"
        );
    }

    /// An unconfigured posture serves `"language": null` — the `language`
    /// field is `Some` only for the realistic-body postures.
    #[tokio::test]
    async fn unconfigured_posture_serves_null_language() {
        let fake = FakeGithub::for_public_repo("rust-lang/cargo");
        let handle = fake.serve_http().await;
        let (status, body) =
            get_json(&format!("{}/repos/rust-lang/cargo", handle.base_url())).await;
        assert_eq!(status, 200);
        assert!(
            body["language"].is_null(),
            "an unconfigured posture keeps `language` null"
        );
        assert!(body.get("signals").is_none(), "no synthetic signals[] is ever served");
    }

    /// RGSD-2: `for_public_repo_with_cargo_lock` serves the `contents/Cargo.lock`
    /// probe at **200** with a realistic file body carrying the file `html_url`
    /// (200 = present, the real GitHub `contents` shape). This is the
    /// load-bearing shape the `DependencyManifestPinned` detection reads.
    #[tokio::test]
    async fn for_public_repo_with_cargo_lock_serves_200_on_contents_cargo_lock() {
        let fake = FakeGithub::for_public_repo_with_cargo_lock("BurntSushi/ripgrep");
        let handle = fake.serve_http().await;

        let (status, body) = get_json(&format!(
            "{}/repos/BurntSushi/ripgrep/contents/Cargo.lock",
            handle.base_url()
        ))
        .await;

        assert_eq!(status, 200, "a committed Cargo.lock must resolve at 200 (present)");
        assert_eq!(body["type"], "file");
        assert_eq!(body["path"], "Cargo.lock");
        assert_eq!(
            body["html_url"], "https://github.com/BurntSushi/ripgrep/blob/master/Cargo.lock",
            "the contents body must carry the file html_url (the signal source_url)"
        );
    }

    /// RGSD-2 no-regression: a posture WITHOUT a Cargo.lock serves the same
    /// probe at **404** (absent). Every existing posture keeps
    /// `has_cargo_lock == false`, so the new `content_exists` probe reads them
    /// all as "no Cargo.lock" — the additive-change / no-regression guarantee.
    #[tokio::test]
    async fn posture_without_cargo_lock_serves_404_on_contents_cargo_lock() {
        let fake = FakeGithub::for_public_repo_with_language("some-org/cpp-project", "C++");
        let handle = fake.serve_http().await;

        let (status, _body) = get_json(&format!(
            "{}/repos/some-org/cpp-project/contents/Cargo.lock",
            handle.base_url()
        ))
        .await;

        assert_eq!(
            status, 404,
            "a repo with no committed Cargo.lock must 404 the contents probe (absent)"
        );
    }

    /// RGSD-2 default-404 guarantee: even the cargo-lock posture serves **404**
    /// for any UNCONFIGURED `contents/*` path (only Cargo.lock is present). An
    /// unconfigured content path can never accidentally read as present.
    #[tokio::test]
    async fn unconfigured_contents_path_serves_404_even_with_cargo_lock() {
        let fake = FakeGithub::for_public_repo_with_cargo_lock("BurntSushi/ripgrep");
        let handle = fake.serve_http().await;

        let (status, _body) = get_json(&format!(
            "{}/repos/BurntSushi/ripgrep/contents/some-other-file.toml",
            handle.base_url()
        ))
        .await;

        assert_eq!(
            status, 404,
            "an unconfigured contents path must 404 (only Cargo.lock is configured present)"
        );
    }

    /// RGSD-2: the repo resolve/harvest body is UNCHANGED by the contents fork
    /// — `GET /repos/{o}/{r}` (no `/contents/`) still serves the 200 repo body.
    #[tokio::test]
    async fn cargo_lock_posture_still_serves_the_repo_resolve_body_at_200() {
        let fake = FakeGithub::for_public_repo_with_cargo_lock("BurntSushi/ripgrep");
        let handle = fake.serve_http().await;

        let (status, body) =
            get_json(&format!("{}/repos/BurntSushi/ripgrep", handle.base_url())).await;

        assert_eq!(status, 200, "the repo resolve path is untouched by the contents fork");
        assert_eq!(body["target"]["kind"], "repo");
        assert_eq!(body["target"]["full_name"], "BurntSushi/ripgrep");
    }

    /// `for_public_user` resolves to a User (not a repo) — drives SG-3 /
    /// SA-1's user-target path (WD-64 bounded aggregate).
    #[tokio::test]
    async fn for_public_user_resolves_as_user() {
        let fake = FakeGithub::for_public_user("torvalds");
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
        let fake = FakeGithub::for_public_user("torvalds")
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
        let fake = FakeGithub::for_public_repo("rust-lang/cargo");
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

    /// RGSD-3: `for_public_repo_with_semver_and_changelog` serves the `/tags`
    /// list as a JSON array of `{"name": <tag>}` objects carrying a
    /// clearly-semver name (`v1.2.3`) — the real GitHub `tags` shape. This is
    /// the load-bearing shape the `list_tags` + `is_semver_tag` detection reads.
    #[tokio::test]
    async fn semver_and_changelog_posture_serves_semver_tags_array() {
        let fake = FakeGithub::for_public_repo_with_semver_and_changelog("BurntSushi/ripgrep");
        let handle = fake.serve_http().await;

        let (status, body) = get_json(&format!(
            "{}/repos/BurntSushi/ripgrep/tags",
            handle.base_url()
        ))
        .await;

        assert_eq!(status, 200, "the tags endpoint always resolves at 200");
        let tags = body.as_array().expect("tags must be a JSON array");
        assert!(!tags.is_empty(), "the semver posture lists tags");
        assert!(
            tags.iter().any(|t| t["name"] == "v1.2.3"),
            "the semver posture must list a clearly-semver tag (v1.2.3)"
        );
    }

    /// RGSD-3: the semver-and-changelog posture serves `contents/CHANGELOG.md`
    /// → **200** with a realistic file body carrying the file `html_url` (200 =
    /// present, the real GitHub `contents` shape) — the CHANGELOG half of the
    /// conjunction, reusing RGSD-2's `contents/*` fork.
    #[tokio::test]
    async fn semver_and_changelog_posture_serves_200_on_contents_changelog() {
        let fake = FakeGithub::for_public_repo_with_semver_and_changelog("BurntSushi/ripgrep");
        let handle = fake.serve_http().await;

        let (status, body) = get_json(&format!(
            "{}/repos/BurntSushi/ripgrep/contents/CHANGELOG.md",
            handle.base_url()
        ))
        .await;

        assert_eq!(status, 200, "a committed CHANGELOG must resolve at 200 (present)");
        assert_eq!(body["type"], "file");
        assert_eq!(body["path"], "CHANGELOG.md");
        assert_eq!(
            body["html_url"],
            "https://github.com/BurntSushi/ripgrep/blob/master/CHANGELOG.md",
            "the contents body must carry the file html_url (the signal source_url)"
        );
    }

    /// RGSD-3 conjunction-guard: a posture with semver tags but `has_changelog
    /// == false` serves `contents/CHANGELOG.md` → **404** (absent). The
    /// `/tags` list still carries the semver names — proving the two halves are
    /// served INDEPENDENTLY so the production conjunction can be exercised.
    #[tokio::test]
    async fn semver_tags_without_changelog_serves_tags_but_404s_changelog() {
        let fake = FakeGithub::for_public_repo_with_tags_and_changelog(
            "torvalds/linux",
            vec!["v6.9", "v1.0.0"],
            false,
        );
        let handle = fake.serve_http().await;

        let (tags_status, tags_body) =
            get_json(&format!("{}/repos/torvalds/linux/tags", handle.base_url())).await;
        assert_eq!(tags_status, 200);
        assert!(
            tags_body
                .as_array()
                .expect("tags array")
                .iter()
                .any(|t| t["name"] == "v6.9"),
            "the semver tags are still served even without a CHANGELOG"
        );

        let (changelog_status, _) = get_json(&format!(
            "{}/repos/torvalds/linux/contents/CHANGELOG.md",
            handle.base_url()
        ))
        .await;
        assert_eq!(
            changelog_status, 404,
            "a repo without a committed CHANGELOG must 404 the contents probe (absent)"
        );
    }

    /// RGSD-3 no-regression: every UNCONFIGURED posture lists NO tags — the
    /// `/tags` endpoint serves an EMPTY array `[]`. This is what keeps every
    /// existing posture reading as "no semver tags" for the new `list_tags`
    /// probe, so no `SemverAndChangelog` signal can ever fire on them.
    #[tokio::test]
    async fn unconfigured_posture_serves_empty_tags_array() {
        let fake = FakeGithub::for_public_repo("rust-lang/cargo");
        let handle = fake.serve_http().await;

        let (status, body) =
            get_json(&format!("{}/repos/rust-lang/cargo/tags", handle.base_url())).await;

        assert_eq!(status, 200, "the tags endpoint never 404s (an untagged repo returns [])");
        assert!(
            body.as_array().expect("tags array").is_empty(),
            "an unconfigured posture must list NO tags (empty array — no regression)"
        );
    }

    /// RGSD-4: `for_public_repo_with_readme` serves `GET /repos/{o}/{r}/readme`
    /// → **200** with the README `size` in bytes + the file `html_url` — the
    /// real GitHub `readme` shape. This is the load-bearing shape the
    /// substantial-README half of `DocsPresentAndSubstantial` reads.
    #[tokio::test]
    async fn readme_posture_serves_200_with_size_on_readme() {
        let fake = FakeGithub::for_public_repo_with_readme("BurntSushi/ripgrep", 20000);
        let handle = fake.serve_http().await;

        let (status, body) = get_json(&format!(
            "{}/repos/BurntSushi/ripgrep/readme",
            handle.base_url()
        ))
        .await;

        assert_eq!(status, 200, "a repo with a README must resolve /readme at 200");
        assert_eq!(body["name"], "README.md");
        assert_eq!(body["size"], 20000, "the readme body must carry the size in bytes");
        assert_eq!(
            body["html_url"],
            "https://github.com/BurntSushi/ripgrep/blob/master/README.md",
            "the readme body must carry the file html_url (the signal source_url)"
        );
    }

    /// RGSD-4: a `readme` posture that declares NO docs dir 404s
    /// `contents/docs` — the README-only disjunct (docs/ absent). Proves the two
    /// disjuncts are served INDEPENDENTLY so the production OR can be exercised.
    #[tokio::test]
    async fn readme_posture_404s_contents_docs() {
        let fake = FakeGithub::for_public_repo_with_readme("BurntSushi/ripgrep", 20000);
        let handle = fake.serve_http().await;

        let (status, _) = get_json(&format!(
            "{}/repos/BurntSushi/ripgrep/contents/docs",
            handle.base_url()
        ))
        .await;

        assert_eq!(status, 404, "a README-only posture must 404 the contents/docs probe (absent)");
    }

    /// RGSD-4: `for_public_repo_with_docs_dir` serves `contents/docs` → **200**
    /// with a JSON ARRAY (the real GitHub `contents` shape for a directory) and
    /// 404s `/readme` (a tiny/absent README) — the docs-dir disjunct alone.
    #[tokio::test]
    async fn docs_dir_posture_serves_200_array_on_contents_docs_and_404s_readme() {
        let fake = FakeGithub::for_public_repo_with_docs_dir("some-org/documented");
        let handle = fake.serve_http().await;

        let (docs_status, docs_body) = get_json(&format!(
            "{}/repos/some-org/documented/contents/docs",
            handle.base_url()
        ))
        .await;
        assert_eq!(docs_status, 200, "a present docs/ dir must resolve at 200");
        assert!(
            docs_body.is_array(),
            "a directory resolves to a JSON array (the real GitHub contents shape)"
        );
        assert!(
            !docs_body.as_array().expect("array").is_empty(),
            "the docs/ dir listing carries at least one entry"
        );

        let (readme_status, _) = get_json(&format!(
            "{}/repos/some-org/documented/readme",
            handle.base_url()
        ))
        .await;
        assert_eq!(
            readme_status, 404,
            "the docs-dir-only posture has no README (docs/ alone must fire the signal)"
        );
    }

    /// RGSD-4 no-regression: every UNCONFIGURED posture 404s BOTH `/readme` and
    /// `contents/docs` — the default `readme_bytes == None` + `has_docs_dir ==
    /// false` guarantee. This is what keeps every existing posture reading as "no
    /// substantial README AND no docs dir" for the new probes, so no
    /// `DocsPresentAndSubstantial` signal can ever fire on them.
    #[tokio::test]
    async fn unconfigured_posture_404s_both_readme_and_contents_docs() {
        let fake = FakeGithub::for_public_repo("rust-lang/cargo");
        let handle = fake.serve_http().await;

        let (readme_status, _) =
            get_json(&format!("{}/repos/rust-lang/cargo/readme", handle.base_url())).await;
        assert_eq!(readme_status, 404, "an unconfigured posture has no README (404 — no regression)");

        let (docs_status, _) = get_json(&format!(
            "{}/repos/rust-lang/cargo/contents/docs",
            handle.base_url()
        ))
        .await;
        assert_eq!(docs_status, 404, "an unconfigured posture has no docs/ dir (404 — no regression)");
    }

    /// RGSD-5: `for_public_repo_with_ci_workflows` serves
    /// `contents/.github/workflows` → **200** with a JSON ARRAY (the real GitHub
    /// `contents` shape for a directory) and 404s `contents/tests` (no tests dir)
    /// — the CI-workflows disjunct alone. Proves the dot+slash path is routed
    /// correctly (match on the full suffix after `contents/`).
    #[tokio::test]
    async fn ci_workflows_posture_serves_200_array_on_github_workflows_and_404s_tests() {
        let fake = FakeGithub::for_public_repo_with_ci_workflows("BurntSushi/ripgrep");
        let handle = fake.serve_http().await;

        let (ci_status, ci_body) = get_json(&format!(
            "{}/repos/BurntSushi/ripgrep/contents/.github/workflows",
            handle.base_url()
        ))
        .await;
        assert_eq!(ci_status, 200, "a present .github/workflows dir must resolve at 200");
        assert!(
            ci_body.is_array(),
            "a directory resolves to a JSON array (the real GitHub contents shape)"
        );
        assert!(
            !ci_body.as_array().expect("array").is_empty(),
            "the .github/workflows dir listing carries at least one entry"
        );

        let (tests_status, _) = get_json(&format!(
            "{}/repos/BurntSushi/ripgrep/contents/tests",
            handle.base_url()
        ))
        .await;
        assert_eq!(
            tests_status, 404,
            "the CI-workflows-only posture has no tests/ dir (CI workflows alone must fire)"
        );
    }

    /// RGSD-5: `for_public_repo_with_tests_dir` serves `contents/tests` → **200**
    /// with a JSON ARRAY and 404s `contents/.github/workflows` (no CI) — the
    /// tests-dir disjunct alone. Proves the two disjuncts are served
    /// INDEPENDENTLY so the production OR can be exercised.
    #[tokio::test]
    async fn tests_dir_posture_serves_200_array_on_tests_and_404s_github_workflows() {
        let fake = FakeGithub::for_public_repo_with_tests_dir("some-org/tested");
        let handle = fake.serve_http().await;

        let (tests_status, tests_body) = get_json(&format!(
            "{}/repos/some-org/tested/contents/tests",
            handle.base_url()
        ))
        .await;
        assert_eq!(tests_status, 200, "a present tests/ dir must resolve at 200");
        assert!(
            tests_body.is_array(),
            "a directory resolves to a JSON array (the real GitHub contents shape)"
        );

        let (ci_status, _) = get_json(&format!(
            "{}/repos/some-org/tested/contents/.github/workflows",
            handle.base_url()
        ))
        .await;
        assert_eq!(
            ci_status, 404,
            "the tests-dir-only posture has no CI workflows (tests/ alone must fire the signal)"
        );
    }

    /// RGSD-5 no-regression: every UNCONFIGURED posture 404s BOTH
    /// `contents/.github/workflows` and `contents/tests` — the default
    /// `has_ci_workflows == false` + `has_tests_dir == false` guarantee. This is
    /// what keeps every existing posture reading as "no CI workflows AND no tests
    /// dir" for the new probes, so no `TestRatioOrCiMatrix` signal can ever fire
    /// on them. Mirrors real octocat/Hello-World (404/404).
    #[tokio::test]
    async fn unconfigured_posture_404s_both_ci_workflows_and_tests() {
        let fake = FakeGithub::for_public_repo("rust-lang/cargo");
        let handle = fake.serve_http().await;

        let (ci_status, _) = get_json(&format!(
            "{}/repos/rust-lang/cargo/contents/.github/workflows",
            handle.base_url()
        ))
        .await;
        assert_eq!(
            ci_status, 404,
            "an unconfigured posture has no CI workflows (404 — no regression)"
        );

        let (tests_status, _) = get_json(&format!(
            "{}/repos/rust-lang/cargo/contents/tests",
            handle.base_url()
        ))
        .await;
        assert_eq!(
            tests_status, 404,
            "an unconfigured posture has no tests/ dir (404 — no regression)"
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

    /// RGSD-6: `for_public_repo_with_all_signals` serves a `/repos` body whose
    /// `language` is `"Rust"` (the `MemorySafetyLanguage` fact) with NO
    /// synthetic `signals[]` — the realistic all-signals posture drives REAL
    /// detection, never the legacy scaffold.
    #[tokio::test]
    async fn for_public_repo_with_all_signals_serves_language_and_no_synthetic_signals() {
        let fake = FakeGithub::for_public_repo_with_all_signals("rust-lang/cargo");
        let handle = fake.serve_http().await;

        let (status, body) =
            get_json(&format!("{}/repos/rust-lang/cargo", handle.base_url())).await;

        assert_eq!(status, 200, "a public repo must resolve at 200");
        assert_eq!(
            body["language"], "Rust",
            "the all-signals body must carry the real `language` (MemorySafetyLanguage fact)"
        );
        assert!(
            body.get("signals").is_none(),
            "the all-signals posture drives REAL detection — NO synthetic signals[]"
        );
    }

    /// RGSD-6: the all-signals posture reflects EVERY remaining detector fact on
    /// its dedicated route — `contents/Cargo.lock` (dependency-pinning), `/tags`
    /// + `contents/CHANGELOG.md` (semver conjunction), a substantial `/readme`
    /// (docs), and `contents/.github/workflows` (CI) — so a scrape fires all
    /// five signals through genuine detection.
    #[tokio::test]
    async fn for_public_repo_with_all_signals_reflects_every_detector_fact() {
        let fake = FakeGithub::for_public_repo_with_all_signals("rust-lang/cargo");
        let handle = fake.serve_http().await;
        let base = handle.base_url();

        let (cargo_lock_status, _) =
            get_json(&format!("{base}/repos/rust-lang/cargo/contents/Cargo.lock")).await;
        assert_eq!(cargo_lock_status, 200, "committed Cargo.lock → 200 (DependencyManifestPinned)");

        let (_, tags_body) = get_json(&format!("{base}/repos/rust-lang/cargo/tags")).await;
        let tags = tags_body.as_array().expect("tags array");
        assert!(
            tags.iter().any(|t| t["name"] == "v1.2.3"),
            "the /tags route must list a semver tag (SemverAndChangelog half)"
        );

        let (changelog_status, _) =
            get_json(&format!("{base}/repos/rust-lang/cargo/contents/CHANGELOG.md")).await;
        assert_eq!(changelog_status, 200, "committed CHANGELOG → 200 (SemverAndChangelog half)");

        let (_, readme_body) = get_json(&format!("{base}/repos/rust-lang/cargo/readme")).await;
        assert_eq!(
            readme_body["size"], 20000,
            "the /readme route must report a substantial size (DocsPresentAndSubstantial)"
        );
        assert!(
            readme_body["size"].as_u64().unwrap() >= 3000,
            "README size must clear README_SUBSTANTIAL_BYTES"
        );

        let (ci_status, _) = get_json(&format!(
            "{base}/repos/rust-lang/cargo/contents/.github/workflows"
        ))
        .await;
        assert_eq!(ci_status, 200, "CI workflows dir → 200 (TestRatioOrCiMatrix)");
    }
}
