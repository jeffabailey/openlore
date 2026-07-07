//! `adapter-github` — `GithubPort` over the workspace `reqwest`/`rustls`.
//!
//! Step 01-03 BOOTSTRAP (effect shell). This crate implements the slice-02
//! `GithubPort` (a NEW port; ADR-019 §1) against the public GitHub REST/GraphQL
//! API. At this bootstrap step the three harvest methods are bodied `todo!()`
//! (`// SCAFFOLD: true (slice-02)`) — the live HTTP harvest lands per-scenario
//! in Phase 03/04 (SCR-*). The probe skeleton already exercises the five-step
//! Earned-Trust contract shape (ADR-019 §6) via the pure arms in `probe.rs`,
//! with the live network driver filling in around the pinned arm contracts.
//!
//! ## Why a new port (not a `PdsPort` extension) — ADR-019 §1
//!
//! GitHub is a wholly different external system from ATProto: no method shape,
//! auth model, rate-limit semantic, or failure surface is shared. Folding
//! GitHub harvest into `PdsPort` would conflate two unrelated trust boundaries
//! and leave `adapter-github` no place to own its distinct probe.
//!
//! ## HTTP client — ADR-019 §2
//!
//! The adapter uses the SAME workspace `reqwest 0.12` (rustls-tls-webpki-roots,
//! json) that `adapter-atproto-pds` pins (ADR-004) — ZERO new transport dep,
//! ZERO new `cargo deny` surface. `octocrab` was rejected for footprint + a new
//! supply-chain surface (technology-stack.md). REST vs GraphQL per signal is a
//! DELIVER call (Q-DELIVER-2); the client skeleton defaults to REST.
//!
//! ## Public-data-only + the human-gate — ADR-019 §3, I-SCR-1/I-SCR-2
//!
//! `GithubAdapter` holds NO `StoragePort`/`IdentityPort`/`PdsPort` reference —
//! by construction it CANNOT sign or publish. It calls ONLY public GitHub
//! endpoints; private/non-existent targets are refused (`GithubError::NotPublic`
//! / `NotFound`), never silently treated as an empty harvest.
//!
//! ## Optional PAT + test seam — ADR-019 §4, WD-54/WD-63
//!
//! The optional PAT is read from `GITHUB_TOKEN` (env-only) by `client.rs`; it
//! is held ONLY in the effect shell and NEVER logged/echoed/published
//! (US-SCR-004 no-token-leak). The `OPENLORE_GITHUB_API_BASE` test seam lets a
//! FakeGithub in-process server thread its base URL in (mirrors slice-01's
//! `OPENLORE_PDS_ENDPOINT` and slice-03's `OPENLORE_PEER_PDS_ENDPOINT`).

#![allow(dead_code)] // probe arms + client helpers used via probe()/harvest; live paths land Phase 03/04
#![forbid(unsafe_code)]

use async_trait::async_trait;
use ports::{GithubError, GithubPort, ProbeOutcome, Signal, SignalKind, TargetKind};

pub mod client;
pub mod probe;

use client::AuthMode;

/// The observed auth/rate-budget posture of a harvest (ADR-019 §5). Re-exported
/// at the crate root so the composition root can name it alongside
/// [`take_last_auth_report`] without reaching into the `client` module. Carries
/// only the budget numbers — never a token (no-token-leak; US-SCR-004).
pub use client::AuthReport;

// ACCEPTED SLICE-02 TECH-DEBT (auth-report side channel; revisit in a future
// slice). The `GithubPort` trait (a LOCKED contract; ADR-019 §1) returns
// `Vec<Signal>` from its harvest methods — by design the pure derivation never
// sees auth state. The observed rate budget lives ONLY on the harvest RESPONSE
// (not on the configured `AuthMode`), so it must reach the CLI verb through a
// side channel rather than by widening the port. The verb sees the adapter only
// as `Box<dyn GithubPort>` (cli `Wiring.github`), so an adapter instance method
// is not reachable without concrete-typing the field (which would break the
// port abstraction AND the FakeGithub substitution). The smallest correct
// option that keeps the contract is therefore a side channel; removing it
// entirely is a future-slice concern that needs a contract change.
//
// CONTAINMENT (refactor-02): the slot is THREAD-LOCAL, not process-global. The
// scrape verb drives its harvest on a `new_current_thread` tokio runtime
// (`build_tokio_runtime`), so `record_auth_report` (inside the async harvest)
// and `take_last_auth_report` (after `block_on` returns) run on the SAME OS
// thread. A thread-local slot is strictly safer than the former
// `OnceLock<Mutex<..>>`: no lock (no poisoning, no contention) and two scrapes
// on different threads can never alias one slot. Single-shot take semantics and
// observable behavior are byte-identical to the global version.
//
// This is the effect boundary: the parse (`client::parse_auth_report`) and the
// render (cli `render_auth_report`) are PURE and unit-tested; only the
// record/read of this slot is the side effect, and it lives in the effect shell
// where credentials and I/O already live (pure-core/effect-shell; ADR-007 /
// ADR-009).
thread_local! {
    static LAST_AUTH_REPORT: std::cell::Cell<Option<AuthReport>> = const { std::cell::Cell::new(None) };
}

/// Record the auth/rate-budget posture observed on a harvest response into the
/// thread-local side channel (see [`LAST_AUTH_REPORT`]). Called from the
/// harvest paths once the response body is in hand. The token bytes are NEVER
/// stored here — an [`AuthReport`] carries only the budget numbers
/// (no-token-leak; US-SCR-004).
fn record_auth_report(report: AuthReport) {
    LAST_AUTH_REPORT.with(|slot| slot.set(Some(report)));
}

/// Take the auth/rate-budget posture the most recent harvest observed on THIS
/// thread (ADR-019 §5), clearing the slot. Returns [`AuthReport::Anonymous`]
/// when no harvest has run (e.g. a resolve-only refusal). The CLI verb calls
/// this AFTER the harvest to render the "authenticated (N/M rate budget)" line.
/// Never surfaces a token value — an [`AuthReport`] has no token field.
pub fn take_last_auth_report() -> AuthReport {
    LAST_AUTH_REPORT
        .with(|slot| slot.take())
        .unwrap_or(AuthReport::Anonymous)
}

/// `GithubPort` adapter over the public GitHub API. One value per process;
/// immutable after construction. Binds the resolved API base (real public API,
/// or the `OPENLORE_GITHUB_API_BASE` test seam) and the auth posture derived
/// from the optional `GITHUB_TOKEN`.
///
/// Holds NO storage/identity/pds reference (I-SCR-1) — it cannot sign or
/// publish. The `auth` field's secret never leaves the adapter except as a
/// reqwest `Authorization` header (US-SCR-004).
pub struct GithubAdapter {
    /// The GitHub API base URL the adapter targets. Either the real public
    /// `https://api.github.com` or the `OPENLORE_GITHUB_API_BASE` test seam.
    /// Stored without a trailing slash so path joins do not double-slash.
    api_base: String,
    /// Auth posture from the optional `GITHUB_TOKEN`. The token bytes (if any)
    /// live here and leave ONLY as a request header — never logged or echoed.
    auth: AuthMode,
}

impl GithubAdapter {
    /// Build the adapter from the environment: resolve the API base (test seam
    /// or real public API) and read the optional `GITHUB_TOKEN` PAT. This is
    /// the composition-root construction path (cli wires it in Phase 03/04+).
    pub fn from_env() -> Self {
        Self {
            api_base: client::resolve_api_base(),
            auth: client::read_auth_mode(),
        }
    }

    /// Build the adapter pointed at an explicit API base (used by tests that
    /// stand up a FakeGithub without mutating the process-global env var).
    /// Auth posture is still read from `GITHUB_TOKEN` so the no-leak path is
    /// exercised uniformly.
    pub fn for_api_base(api_base: impl Into<String>) -> Self {
        Self {
            api_base: client::strip_trailing_slashes(api_base.into()),
            auth: client::read_auth_mode(),
        }
    }

    /// The API base the adapter is bound to. Exposed for tests + the
    /// composition root's startup banner.
    pub fn api_base(&self) -> &str {
        &self.api_base
    }

    /// Whether a PAT was configured. Safe to surface (a bool, not the token).
    pub fn is_authenticated(&self) -> bool {
        self.auth.is_authenticated()
    }
}

#[async_trait]
impl GithubPort for GithubAdapter {
    /// Walk the five-step Earned-Trust gauntlet (ADR-019 §6) within the 250ms
    /// budget (I-5). The first arm that refuses is surfaced via
    /// `ProbeOutcome::Refused`; all-green returns `ProbeOutcome::Ok`.
    ///
    /// ### Step 01-03 BOOTSTRAP wiring
    ///
    /// The arms in `probe.rs` are pure — they consume the *outcome* of an I/O
    /// step and produce structured refusals. At this bootstrap step the live
    /// network arms (public reachability, private refusal, header presence)
    /// have not landed (Phase 03/04, SCR-*), so this `probe()` does the
    /// non-network work it CAN do safely at startup:
    ///
    /// - a pre-flight guard that an empty API base refuses (no host to probe);
    /// - the auth-mode arm, which does not require a live request to assert the
    ///   no-token-leak invariant on the structured event it would emit.
    ///
    /// The arm contracts are already pinned by `probe.rs` unit tests; the live
    /// I/O driver fills in around them without changing the arm signatures.
    /// This body performs real work (the guard + arm dispatch + the no-leak
    /// assertion) so `cargo xtask check-probes` classifies it `Accept`.
    fn probe(&self) -> ProbeOutcome {
        // Pre-flight: an empty API base cannot be probed. Surface as
        // GithubPublicApiUnreachable so the composition root refuses startup
        // with a clear reason (ADR-019 §6 step 1 class).
        if self.api_base.is_empty() {
            return refuse(
                ports::ProbeRefusalReason::GithubPublicApiUnreachable,
                "GitHub API base is empty; the public API cannot be reached".to_string(),
                serde_json::json!({"api_base": ""}),
            );
        }

        // Auth-mode arm (ADR-019 §6 step 3): at bootstrap we have not yet
        // validated a token against the live API, so `token_rejected` is
        // false (a set token is assumed valid until the live arm lands). The
        // structured event carries ONLY `is_authenticated` (a bool) — never
        // the token bytes (US-SCR-004 no-token-leak). The live driver in
        // Phase 03/04 replaces the `false` with the real 401-validation
        // outcome without changing this lift.
        let token_rejected = false;
        if let probe::ArmOutcome::Refused(r) = probe::check_auth_mode(token_rejected) {
            return ProbeOutcome::Refused {
                reason: r.reason,
                detail: r.detail,
                structured: r.structured,
            };
        }

        // Live network arms (public reachability, private refusal, rate-limit
        // header presence) wire in Phase 03/04. The pure arm contracts in
        // probe.rs are already pinned by unit tests; only the reqwest I/O glue
        // that feeds them moves. Bootstrap returns Ok once the non-network
        // checks pass.
        ProbeOutcome::Ok
    }

    /// Disambiguate `owner/repo` vs `user`; REFUSE private / non-existent
    /// targets with `GithubError::NotPublic` / `GithubError::NotFound`
    /// (public-data-only; WD-51 / I-SCR-2).
    ///
    /// `GET {base}/repos/{owner}/{repo}` for an `owner/repo` target, else
    /// `GET {base}/users/{user}`. Only PUBLIC read paths are ever hit
    /// (allowlist: `/repos/...`, `/users/...`) — there is no private
    /// surface (I-SCR-2). The HTTP status classifies the refusal.
    async fn resolve_target(&self, target: &str) -> Result<TargetKind, GithubError> {
        match split_target(target) {
            ResolvedTarget::Repo { owner, repo } => {
                let path = format!("/repos/{owner}/{repo}");
                self.get_public(&path, target).await?;
                Ok(TargetKind::Repo { owner, repo })
            }
            ResolvedTarget::User { user } => {
                let path = format!("/users/{user}");
                self.get_public(&path, target).await?;
                Ok(TargetKind::User { user })
            }
        }
    }

    /// Harvest the bounded public-signal set for a repo. Returns
    /// already-fetched `Signal`s ready for `scraper-domain::derive_candidates`.
    ///
    /// `GET {base}/repos/{owner}/{repo}` and reshape the response's
    /// `signals[]` into typed `Signal`s (the derivation is the pure
    /// `scraper-domain`'s job — the adapter only fetches + reshapes).
    async fn harvest_repo(&self, owner: &str, repo: &str) -> Result<Vec<Signal>, GithubError> {
        let target = format!("{owner}/{repo}");
        let path = format!("/repos/{owner}/{repo}");
        let body = self.get_public(&path, &target).await?;
        // Surface the observed auth/rate-budget posture (ADR-019 §5) through
        // the effect-shell side channel so the verb can report it. The token
        // is NEVER recorded — an AuthReport carries only the budget numbers.
        record_auth_report(client::parse_auth_report(&body));
        // RGSD-1 union bridge (design §4): the returned signals are the UNION
        // of the NEW pure detection over real repo facts
        // (`detect_signals(parse_repo_facts(body))` — reads the live `language`
        // field) AND the legacy synthetic `signals[]` path (`parse_signals`,
        // which the real API never populates but the existing FakeGithub
        // postures still inject). Detection results lead so a real repo's
        // language-derived signal is present; the legacy path fills the rest,
        // deduped by `SignalKind` so a (currently impossible) both-present kind
        // can never double-count. As detectors 2–5 land and the fake fixtures
        // migrate to realistic bodies, the `parse_signals` path is removed
        // (RGSD-6 cleanup).
        // RGSD-2: probe `contents/Cargo.lock` (a SECOND public endpoint) and set
        // `cargo_lock_url` before detection. 200 => Some(file html_url) (a
        // committed Cargo.lock — the manifest is pinned); 404 => None (absent, a
        // total result, not an error). The pure `detect_signals` then fires the
        // `DependencyManifestPinned` arm iff the probe found the file (design
        // §2/§4). A rate-limit / auth failure on the probe propagates via `?`,
        // never silently read as "absent".
        let mut facts = client::parse_repo_facts(&body);
        facts.cargo_lock_url = self.content_exists(owner, repo, "Cargo.lock").await?;
        // RGSD-3: list `/tags` (a THIRD public endpoint) and probe
        // `contents/CHANGELOG.md`, then set the two halves of the
        // `SemverAndChangelog` CONJUNCTION before detection. `list_tags` →
        // `pick_semver_tag` finds a semver-shaped tag name (`Some` only when the
        // repo follows semver); the CHANGELOG probe reads 200 => Some(file
        // html_url) / 404 => None exactly as the Cargo.lock probe does. The pure
        // `detect_signals` then fires the `SemverAndChangelog` arm iff BOTH are
        // `Some` (design §2/§4). A rate-limit / auth failure on either probe
        // propagates via `?`, never silently read as "absent".
        let tags = self.list_tags(owner, repo).await?;
        facts.semver_tag = scraper_domain::pick_semver_tag(&tags);
        facts.changelog_url = self.content_exists(owner, repo, "CHANGELOG.md").await?;
        // RGSD-4: fetch `/readme` (a FOURTH public endpoint) and probe
        // `contents/docs` (reusing the RGSD-2 `contents/*` probe), then set the
        // two disjuncts of the `DocsPresentAndSubstantial` DISJUNCTION before
        // detection. `fetch_readme` → Some((size, url)) when a README exists (200)
        // / None when absent (404); `content_exists(.., "docs")` → Some(url) when
        // a `docs/` directory 200s / None when absent (404). The pure
        // `detect_signals` then fires the `DocsPresentAndSubstantial` arm iff the
        // README is SUBSTANTIAL OR a docs dir is present (design section 2/5). A
        // rate-limit / auth failure on either probe propagates via `?`, never
        // silently read as "absent".
        if let Some((readme_bytes, readme_url)) = self.fetch_readme(owner, repo).await? {
            facts.readme_bytes = Some(readme_bytes);
            facts.readme_url = Some(readme_url);
        }
        facts.docs_url = self.content_exists(owner, repo, "docs").await?;
        let detected = scraper_domain::detect_signals(&facts);
        Ok(union_signals_by_kind(
            detected,
            client::parse_signals(&body),
        ))
    }

    /// Harvest a BOUNDED cross-repo aggregate for a user / contributor target
    /// (deep triangulation deferred to slice-04 per WD-64).
    ///
    /// `GET {base}/users/{user}` and reshape the response's `signals[]`, then
    /// CAP the aggregate to [`USER_AGGREGATE_SIGNAL_CAP`] so a user-target can
    /// never fan out unboundedly (WD-64 / Q-DELIVER-4). The cap is the
    /// slice-02 bound; deep cross-repo triangulation (a larger, scored walk)
    /// is slice-04's concern.
    async fn harvest_user(&self, user: &str) -> Result<Vec<Signal>, GithubError> {
        let path = format!("/users/{user}");
        let body = self.get_public(&path, user).await?;
        // Surface the observed auth/rate-budget posture (ADR-019 §5); see
        // `harvest_repo`. The token is NEVER recorded here.
        record_auth_report(client::parse_auth_report(&body));
        Ok(bound_user_aggregate(client::parse_signals(&body)))
    }
}

/// The slice-02 bound on a USER-target's cross-repo aggregate (WD-64 /
/// Q-DELIVER-4). A user with many public repos must NOT fan out unboundedly;
/// the aggregate harvest is capped at this many signals (a small fixed cap —
/// deep, scored cross-repo triangulation is deferred to slice-04). The cap is
/// surfaced as a constant so the bound is explicit + auditable rather than an
/// incidental side effect of how many signals a single response happened to
/// carry.
pub const USER_AGGREGATE_SIGNAL_CAP: usize = 25;

/// Apply the slice-02 user-aggregate bound (WD-64): truncate the harvested
/// aggregate to at most [`USER_AGGREGATE_SIGNAL_CAP`] signals. PURE — a
/// value-in / value-out transform of the already-fetched signal set, so the
/// bound is testable without any network. Repo harvests are NOT capped here:
/// a repo's signal set is intrinsically bounded by the SSOT mapping, whereas a
/// user aggregates across an unbounded number of public repos.
fn bound_user_aggregate(mut signals: Vec<Signal>) -> Vec<Signal> {
    signals.truncate(USER_AGGREGATE_SIGNAL_CAP);
    signals
}

/// Union the NEW pure detection with the legacy synthetic `signals[]` set,
/// deduped by [`SignalKind`] against the DETECTED kinds only (RGSD-1 union
/// bridge, design §4). PURE — a value-in / value-out merge so the bridge is
/// testable without any network.
///
/// `detected` (the NEW pure detection) leads; each `legacy` signal is appended
/// UNLESS its kind was already produced by detection — that is the "both
/// present" case design §4 dedups so a kind can never double-count. Dedup is
/// against the detected kinds ONLY, never within `legacy`: the legacy path
/// legitimately carries multiple signals of the SAME kind (e.g. three
/// `DocsPresentAndSubstantial` signals that collapse into one candidate
/// downstream in `derive_candidates`), and those must all pass through.
fn union_signals_by_kind(detected: Vec<Signal>, legacy: Vec<Signal>) -> Vec<Signal> {
    let detected_kinds: Vec<SignalKind> = detected.iter().map(|s| s.kind).collect();
    let mut out = detected;
    for signal in legacy {
        if !detected_kinds.contains(&signal.kind) {
            out.push(signal);
        }
    }
    out
}

impl GithubAdapter {
    /// Issue one PUBLIC GET against `{api_base}{path}`, attaching the
    /// optional `Authorization: token <PAT>` header (the ONLY path the PAT
    /// bytes leave the adapter — never logged/echoed; US-SCR-004). The HTTP
    /// status classifies refusals into the railway-oriented [`GithubError`]
    /// surface; a 2xx body is returned as parsed JSON for the caller to
    /// reshape.
    ///
    /// `path` is always on the public allowlist (`/repos/...` or
    /// `/users/...`); there is no private surface (WD-51 / I-SCR-2).
    async fn get_public(&self, path: &str, target: &str) -> Result<serde_json::Value, GithubError> {
        let url = format!("{}{}", self.api_base, path);
        let response = self.send_get(&url).await?;

        let status = response.status();
        if status.is_success() {
            return response
                .json::<serde_json::Value>()
                .await
                .map_err(|e| GithubError::ApiShape(format!("response body was not JSON: {e}")));
        }

        // A non-2xx status is a refusal: read its body and classify it. This
        // is still a PUBLIC endpoint (`/repos/...` or `/users/...`); no private
        // surface is ever reached.
        Err(classify_refusal(response, target).await)
    }

    /// Issue one PUBLIC GET against `{api_base}{url_path}`, attaching the
    /// optional `Authorization: token <PAT>` header (the ONLY path the PAT
    /// bytes leave the adapter — never logged/echoed; US-SCR-004). Shared by
    /// [`get_public`](Self::get_public) and
    /// [`content_exists`](Self::content_exists) so the client-build + auth-header
    /// wiring lives in ONE place; each caller classifies the RESPONSE per its own
    /// railway rules (a 404 is a refusal for `get_public`, but "absent" for
    /// `content_exists`).
    async fn send_get(&self, url: &str) -> Result<reqwest::Response, GithubError> {
        let client = client::build_client()
            .map_err(|e| GithubError::Network(format!("could not build HTTP client: {e}")))?;
        let mut request = client.get(url);
        if let Some(header) = self.auth.authorization_header() {
            request = request.header(reqwest::header::AUTHORIZATION, header);
        }
        request
            .send()
            .await
            .map_err(|e| GithubError::Network(format!("request to GitHub failed: {e}")))
    }

    /// Probe whether a public repo commits a file at `path` (RGSD-2). Issues
    /// `GET {api_base}/repos/{owner}/{repo}/contents/{path}` and reads the
    /// SPIKE-verified presence contract:
    ///
    /// - **2xx** => `Ok(Some(html_url))` — the file is present; the committed
    ///   file's public `html_url` (read from the body, or reconstructed) becomes
    ///   the `DependencyManifestPinned` signal's `source_url` (design §3);
    /// - **404** => `Ok(None)` — the file is ABSENT. Absent is NOT an error: a
    ///   repo without the file is a perfectly valid public repo, so the probe is
    ///   a TOTAL, railway-oriented result (the negative guardrail relies on this);
    /// - any OTHER status => the SAME [`GithubError`] classification
    ///   `get_public` uses (403 => `RateLimited`, 401 => `TokenRejected`,
    ///   transport => `Network`) so a rate-limit / auth failure mid-harvest is
    ///   surfaced, never silently read as "absent".
    ///
    /// The path is on the public allowlist (`/repos/...`); no private surface is
    /// reached (WD-51 / I-SCR-2). The token is NEVER logged (US-SCR-004).
    async fn content_exists(
        &self,
        owner: &str,
        repo: &str,
        path: &str,
    ) -> Result<Option<String>, GithubError> {
        let url = format!("{}/repos/{owner}/{repo}/contents/{path}", self.api_base);
        let response = self.send_get(&url).await?;

        let status = response.status();
        if status.is_success() {
            let body = response
                .json::<serde_json::Value>()
                .await
                .map_err(|e| GithubError::ApiShape(format!("contents body was not JSON: {e}")))?;
            return Ok(Some(client::content_html_url(&body, owner, repo, path)));
        }

        // 404 = absent (a total result, NOT an error) — the whole point of the
        // probe. Every other non-2xx is a real failure classified exactly as
        // get_public does (rate-limit / auth / shape), never read as "absent".
        if status.as_u16() == 404 {
            return Ok(None);
        }
        let target = format!("{owner}/{repo}");
        Err(classify_refusal(response, &target).await)
    }

    /// List a public repo's tag names (RGSD-3). Issues
    /// `GET {api_base}/repos/{owner}/{repo}/tags` and reads the SPIKE-verified
    /// tags contract:
    ///
    /// - **2xx** => `Ok(tag_names)` — the JSON array of `{"name": <tag>}`
    ///   objects reshaped into the tag-name list via
    ///   [`client::parse_tag_names`]. An untagged repo the API serves as `[]`
    ///   yields an empty list (no tags), never an error;
    /// - any non-2xx => the SAME [`GithubError`] classification `get_public`
    ///   uses (403 => `RateLimited`, 401 => `TokenRejected`, transport =>
    ///   `Network`) so a rate-limit / auth failure mid-harvest is surfaced,
    ///   never silently read as "no tags".
    ///
    /// The path is on the public allowlist (`/repos/...`); no private surface is
    /// reached (WD-51 / I-SCR-2). The token is NEVER logged (US-SCR-004). The
    /// pure `pick_semver_tag` then decides whether any listed tag is
    /// semver-shaped (design §2/§4).
    async fn list_tags(&self, owner: &str, repo: &str) -> Result<Vec<String>, GithubError> {
        let url = format!("{}/repos/{owner}/{repo}/tags", self.api_base);
        let response = self.send_get(&url).await?;

        let status = response.status();
        if status.is_success() {
            let body = response
                .json::<serde_json::Value>()
                .await
                .map_err(|e| GithubError::ApiShape(format!("tags body was not JSON: {e}")))?;
            return Ok(client::parse_tag_names(&body));
        }

        let target = format!("{owner}/{repo}");
        Err(classify_refusal(response, &target).await)
    }

    /// Fetch a public repo's README size + URL (RGSD-4). Issues
    /// `GET {api_base}/repos/{owner}/{repo}/readme` and reads the SPIKE-verified
    /// README contract:
    ///
    /// - **2xx** => `Ok(Some((size, html_url)))` — the README's `size` in bytes
    ///   (the real GitHub `readme` API carries it) + its public `html_url` (or a
    ///   reconstructed blob URL when absent). The pure `detect_signals` then
    ///   decides whether the `size` is SUBSTANTIAL (design section 5);
    /// - **404** => `Ok(None)` — the repo has NO README. Absent is NOT an error:
    ///   a repo without a README is a perfectly valid public repo, so the fetch
    ///   is a TOTAL, railway-oriented result (the negative guardrail relies on
    ///   this);
    /// - any OTHER status => the SAME [`GithubError`] classification `get_public`
    ///   uses (403 => `RateLimited`, 401 => `TokenRejected`, transport =>
    ///   `Network`) so a rate-limit / auth failure mid-harvest is surfaced, never
    ///   silently read as "no README".
    ///
    /// The path is on the public allowlist (`/repos/...`); no private surface is
    /// reached (WD-51 / I-SCR-2). The token is NEVER logged (US-SCR-004).
    async fn fetch_readme(
        &self,
        owner: &str,
        repo: &str,
    ) -> Result<Option<(u64, String)>, GithubError> {
        let url = format!("{}/repos/{owner}/{repo}/readme", self.api_base);
        let response = self.send_get(&url).await?;

        let status = response.status();
        if status.is_success() {
            let body = response
                .json::<serde_json::Value>()
                .await
                .map_err(|e| GithubError::ApiShape(format!("readme body was not JSON: {e}")))?;
            let size = body
                .get("size")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0);
            return Ok(Some((size, client::readme_html_url(&body, owner, repo))));
        }

        // 404 = no README (a total result, NOT an error). Every other non-2xx is
        // a real failure classified exactly as get_public does (rate-limit / auth
        // / shape), never read as "no README".
        if status.as_u16() == 404 {
            return Ok(None);
        }
        let target = format!("{owner}/{repo}");
        Err(classify_refusal(response, &target).await)
    }
}

/// A `<target>` split into the kind the resolve/harvest paths need.
enum ResolvedTarget {
    Repo { owner: String, repo: String },
    User { user: String },
}

/// Split an `owner/repo` target into [`ResolvedTarget::Repo`]; a bare token
/// (no `/`) into [`ResolvedTarget::User`]. Mirrors `FakeGithub::resolve_kind`
/// so the adapter + the double agree on the disambiguation rule.
fn split_target(target: &str) -> ResolvedTarget {
    match target.split_once('/') {
        Some((owner, repo)) => ResolvedTarget::Repo {
            owner: owner.to_string(),
            repo: repo.to_string(),
        },
        None => ResolvedTarget::User {
            user: target.to_string(),
        },
    }
}

/// Classify a non-2xx HTTP status + its refusal body into the railway-oriented
/// [`GithubError`]. PURE — a value-in / value-out transform of the
/// already-fetched status + body, so the classification is testable without any
/// network.
///
/// A 404 is split by the body discriminator (WD-51 / I-SCR-2; SG-5):
///
/// - 404 + `private: true` => [`GithubError::NotPublic`] — the public API 404s
///   a private repo to avoid leaking its existence; the `private` marker is the
///   only honest signal that the refusal is a private/inaccessible cause, so the
///   CLI can reassure "the scraper only reads public data" rather than the
///   generic not-found message. Public-data-only stays STRUCTURAL: this is a
///   classification of a PUBLIC-endpoint refusal, NOT a private endpoint call.
/// - any other 404 (no `private` marker, or `private: false`) =>
///   [`GithubError::NotFound`] — the conservative not-found cause.
///
/// 403 is rate-budget exhaustion; 401 is a rejected token (the value is NEVER
/// echoed — US-SCR-004).
fn classify_status(status: u16, body: &serde_json::Value, target: &str) -> GithubError {
    match status {
        404 if is_private_refusal(body) => GithubError::NotPublic {
            target: target.to_string(),
        },
        404 => GithubError::NotFound {
            target: target.to_string(),
        },
        403 => GithubError::RateLimited {
            authenticated: false,
        },
        401 => GithubError::TokenRejected,
        other => GithubError::ApiShape(format!("unexpected HTTP status {other} for {target}")),
    }
}

/// Read a non-2xx refusal body, then classify it into the railway-oriented
/// [`GithubError`]. Shared by [`GithubAdapter::get_public`] and
/// [`GithubAdapter::content_exists`] so the "read the refusal body, then
/// classify" tail lives in ONE place (each caller already peeled off its own
/// success / 404-absent arm before delegating here).
///
/// The body carries the 404 discriminator [`classify_status`] uses to tell a
/// private target (404 + `private: true`) from a plain not-found — the public
/// API serves the SAME 404 status for both, so the body is the only honest
/// signal (WD-51 / I-SCR-2). A body that fails to parse degrades to an empty
/// object, so an unrecognized refusal stays on the conservative `NotFound` arm
/// (never silently treated as a successful empty harvest). The token is NEVER
/// echoed (US-SCR-004).
async fn classify_refusal(response: reqwest::Response, target: &str) -> GithubError {
    let status_code = response.status().as_u16();
    let body = response
        .json::<serde_json::Value>()
        .await
        .unwrap_or_else(|_| serde_json::json!({}));
    classify_status(status_code, &body, target)
}

/// Whether a refusal body marks a PRIVATE/inaccessible target — the `private`
/// discriminator the public API surfaces alongside its existence-hiding 404
/// (WD-51 / I-SCR-2). `true` only when the body carries `private: true`; a
/// missing marker or `private: false` is NOT a private refusal (it stays on the
/// conservative `NotFound` arm). Reading a body field is a pure inspection — no
/// private endpoint is ever called to obtain it.
fn is_private_refusal(body: &serde_json::Value) -> bool {
    body.get("private").and_then(serde_json::Value::as_bool) == Some(true)
}

/// Lift a refusal-arm shape into a `ProbeOutcome::Refused`. Pulled out so the
/// `probe()` body's refusal paths are a single intention-revealing call.
fn refuse(
    reason: ports::ProbeRefusalReason,
    detail: String,
    structured: serde_json::Value,
) -> ProbeOutcome {
    ProbeOutcome::Refused {
        reason,
        detail,
        structured,
    }
}

// -----------------------------------------------------------------------------
// Inner-TDD unit tests — constructor + seam wiring + probe lift + scaffold pins.
//
// The live harvest paths (real resolve_target / harvest over reqwest) are
// integration territory and land per the SCR-* scenarios in Phase 03/04. The
// tests below cover the adapter's bootstrap-observable surface: API-base seam
// resolution, the probe()'s non-network lift behavior + no-token-leak, and the
// `// SCAFFOLD: true` panic pins that prove the harvest bodies are deferred.
// -----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// `for_api_base` strips trailing slashes so path joins do not
    /// double-slash (mirrors the PDS adapter's `normalize_endpoint`).
    #[test]
    fn for_api_base_strips_trailing_slashes() {
        let adapter = GithubAdapter::for_api_base("https://fake.test/");
        assert_eq!(adapter.api_base(), "https://fake.test");
    }

    /// A non-empty API base + no rejected token returns `ProbeOutcome::Ok` at
    /// the bootstrap step (live network arms land Phase 03/04). The arm
    /// contracts themselves are pinned by probe.rs unit tests.
    #[test]
    fn probe_returns_ok_when_api_base_present_bootstrap() {
        let adapter = GithubAdapter::for_api_base("https://fake.test");
        assert!(matches!(adapter.probe(), ProbeOutcome::Ok));
    }

    /// Pre-flight: an empty API base refuses the probe with
    /// `GithubPublicApiUnreachable` (no host to reach).
    #[test]
    fn probe_refuses_when_api_base_is_empty() {
        let adapter = GithubAdapter::for_api_base("");
        match adapter.probe() {
            ProbeOutcome::Refused {
                reason, structured, ..
            } => {
                assert_eq!(
                    reason,
                    ports::ProbeRefusalReason::GithubPublicApiUnreachable
                );
                // No-token-leak: the refusal payload never carries a token.
                let rendered = structured.to_string();
                assert!(
                    !rendered.contains("ghp_"),
                    "probe event must not leak a token: {rendered}"
                );
            }
            ProbeOutcome::Ok => panic!("expected refusal for empty API base"),
        }
    }

    /// `split_target` disambiguates `owner/repo` (Repo) from a bare token
    /// (User), mirroring `FakeGithub::resolve_kind` so the adapter + double
    /// agree on the resolve path. Pure decomposition of `resolve_target`'s
    /// GREEN body — the live HTTP disambiguation in SG-1 / SG-3 relies on it.
    #[test]
    fn split_target_disambiguates_repo_from_user() {
        match split_target("rust-lang/cargo") {
            ResolvedTarget::Repo { owner, repo } => {
                assert_eq!(owner, "rust-lang");
                assert_eq!(repo, "cargo");
            }
            ResolvedTarget::User { .. } => panic!("owner/repo must resolve as Repo"),
        }
        match split_target("torvalds") {
            ResolvedTarget::User { user } => assert_eq!(user, "torvalds"),
            ResolvedTarget::Repo { .. } => panic!("a bare token must resolve as User"),
        }
    }

    /// `bound_user_aggregate` enforces the WD-64 / Q-DELIVER-4 slice-02 cap:
    /// an over-cap aggregate is truncated to EXACTLY `USER_AGGREGATE_SIGNAL_CAP`
    /// (preserving the leading prefix order — a user-target can never fan out
    /// unboundedly), while an at-or-under-cap aggregate passes through
    /// unchanged. Pure decomposition of `harvest_user`'s GREEN body — the bound
    /// is testable without any network.
    #[test]
    fn bound_user_aggregate_caps_an_oversized_aggregate_and_preserves_small_ones() {
        use ports::SignalKind;

        let signal = |n: usize| Signal {
            kind: SignalKind::TestRatioOrCiMatrix,
            value: format!("aggregate signal {n}"),
            source_url: format!("https://github.com/torvalds#{n}"),
        };

        // Over the cap: truncate to exactly the cap, leading prefix preserved.
        let oversized: Vec<Signal> = (0..USER_AGGREGATE_SIGNAL_CAP + 7).map(signal).collect();
        let bounded = bound_user_aggregate(oversized.clone());
        assert_eq!(
            bounded.len(),
            USER_AGGREGATE_SIGNAL_CAP,
            "an over-cap user aggregate must be bounded to the slice-02 cap (WD-64)"
        );
        assert_eq!(
            bounded.as_slice(),
            &oversized[..USER_AGGREGATE_SIGNAL_CAP],
            "the bound must preserve the leading prefix (no fan-out reordering)"
        );

        // At or under the cap: pass through unchanged.
        let small: Vec<Signal> = (0..2).map(signal).collect();
        assert_eq!(
            bound_user_aggregate(small.clone()),
            small,
            "an at-or-under-cap aggregate must pass through unchanged"
        );
    }

    /// `union_signals_by_kind` merges the NEW pure detection with the legacy
    /// synthetic `signals[]` path, deduped by `SignalKind` (RGSD-1 union
    /// bridge, design §4). Detection leads; a legacy signal whose kind is
    /// already present is dropped so no predicate double-counts; a legacy
    /// signal of a fresh kind is appended. Pure decomposition of
    /// `harvest_repo`'s union — testable without any network.
    #[test]
    fn union_signals_by_kind_dedupes_with_detection_leading() {
        let detected = Signal {
            kind: SignalKind::MemorySafetyLanguage,
            value: "primary language: Rust".to_string(),
            source_url: "https://github.com/rust-lang/cargo".to_string(),
        };
        // A legacy signal of the SAME kind must be dropped (detection wins).
        let legacy_same_kind = Signal {
            kind: SignalKind::MemorySafetyLanguage,
            value: "Rust + no unsafe blocks".to_string(),
            source_url: "https://github.com/rust-lang/cargo".to_string(),
        };
        // A legacy signal of a FRESH kind must be appended.
        let legacy_fresh_kind = Signal {
            kind: SignalKind::DependencyManifestPinned,
            value: "Cargo.lock committed".to_string(),
            source_url: "https://github.com/rust-lang/cargo/blob/master/Cargo.lock".to_string(),
        };

        let unioned = union_signals_by_kind(
            vec![detected.clone()],
            vec![legacy_same_kind, legacy_fresh_kind.clone()],
        );

        assert_eq!(
            unioned,
            vec![detected, legacy_fresh_kind],
            "detection leads, the duplicate-kind legacy signal is dropped, the \
             fresh-kind legacy signal is appended"
        );

        // The empty-detection case passes the legacy set through verbatim so
        // every existing FakeGithub `signals[]` posture stays untouched —
        // INCLUDING multiple legacy signals of the SAME kind (they collapse
        // downstream in `derive_candidates`, NOT here; dedup is against the
        // detected kinds only, never within legacy).
        let docs_a = Signal {
            kind: SignalKind::DocsPresentAndSubstantial,
            value: "docs/ directory present".to_string(),
            source_url: "https://github.com/x/y/tree/master/docs".to_string(),
        };
        let docs_b = Signal {
            kind: SignalKind::DocsPresentAndSubstantial,
            value: "README 412 lines (> 200)".to_string(),
            source_url: "https://github.com/x/y/blob/master/README.md".to_string(),
        };
        assert_eq!(
            union_signals_by_kind(Vec::new(), vec![docs_a.clone(), docs_b.clone()]),
            vec![docs_a, docs_b],
            "with no detection ALL legacy signals pass through unchanged — even \
             same-kind ones (they collapse downstream, not in the bridge)"
        );
    }

    /// `classify_status` maps HTTP refusal statuses onto the railway-oriented
    /// `GithubError` surface; the 401 path NEVER echoes a token value
    /// (US-SCR-004 no-token-leak).
    #[test]
    fn classify_status_maps_refusals_without_leaking_a_token() {
        let empty = serde_json::json!({});
        assert!(matches!(
            classify_status(404, &empty, "ghost-org/ghost-repo"),
            GithubError::NotFound { .. }
        ));
        assert!(matches!(
            classify_status(403, &empty, "torvalds"),
            GithubError::RateLimited {
                authenticated: false
            }
        ));
        let rejected = classify_status(401, &empty, "rust-lang/cargo");
        assert!(matches!(rejected, GithubError::TokenRejected));
        assert!(
            !rejected.to_string().contains("ghp_"),
            "TokenRejected must never echo a token value"
        );
    }

    /// SG-5 / 03-05: a 404 whose body carries the `private: true`
    /// discriminator is the private/inaccessible cause — classified
    /// `GithubError::NotPublic` (not `NotFound`) so the CLI reassures "the
    /// scraper only reads public data" (WD-51 / I-SCR-2). A BARE 404 (no
    /// `private` marker) stays `NotFound`: the two refusals are distinguished
    /// ONLY by the body discriminator, never by a private endpoint call
    /// (public-data-only is structural; KPI-SCR-4).
    #[test]
    fn classify_status_distinguishes_private_target_from_not_found() {
        // A private repo: 404 + `private: true` body => NotPublic, target named.
        let private_body = serde_json::json!({ "message": "Not Found", "private": true });
        match classify_status(404, &private_body, "acme-corp/secret-repo") {
            GithubError::NotPublic { target } => {
                assert_eq!(
                    target, "acme-corp/secret-repo",
                    "NotPublic must name the refused target (WD-51)"
                );
            }
            other => panic!("a 404 with private:true must classify NotPublic, got {other:?}"),
        }
        assert!(
            classify_status(404, &private_body, "acme-corp/secret-repo")
                .to_string()
                .contains("only reads public data"),
            "the NotPublic Display must carry the public-data-only reassurance"
        );

        // A bare 404 (no `private` marker) is the not-found cause => NotFound.
        let plain_404 = serde_json::json!({ "message": "Not Found" });
        assert!(
            matches!(
                classify_status(404, &plain_404, "ghost-org/ghost-repo"),
                GithubError::NotFound { .. }
            ),
            "a bare 404 (no private discriminator) must stay NotFound"
        );

        // A `private: false` marker is NOT a private refusal => NotFound.
        let not_private = serde_json::json!({ "message": "Not Found", "private": false });
        assert!(
            matches!(
                classify_status(404, &not_private, "ghost-org/ghost-repo"),
                GithubError::NotFound { .. }
            ),
            "private:false must not be misread as a private refusal"
        );
    }
}
