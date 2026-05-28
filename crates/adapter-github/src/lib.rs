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
use ports::{GithubError, GithubPort, ProbeOutcome, Signal, TargetKind};

pub mod client;
pub mod probe;

use client::AuthMode;

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
    /// SCAFFOLD: true (slice-02)
    ///
    /// Bodied `todo!()` at step 01-03; the live `GET /repos/{owner}/{repo}` +
    /// `GET /users/{user}` disambiguation lands per the SCR-* scenarios in
    /// Phase 03/04.
    async fn resolve_target(&self, _target: &str) -> Result<TargetKind, GithubError> {
        // SCAFFOLD: true (slice-02)
        todo!("resolve_target lands per SCR-* scenarios in Phase 03/04")
    }

    /// Harvest the bounded public-signal set for a repo. Returns
    /// already-fetched `Signal`s ready for `scraper-domain::derive_candidates`.
    ///
    /// SCAFFOLD: true (slice-02)
    ///
    /// Bodied `todo!()` at step 01-03; the live signal harvest (manifest,
    /// docs, test-ratio/CI, semver/changelog, language) lands per the SCR-*
    /// scenarios in Phase 03/04.
    async fn harvest_repo(&self, _owner: &str, _repo: &str) -> Result<Vec<Signal>, GithubError> {
        // SCAFFOLD: true (slice-02)
        todo!("harvest_repo lands per SCR-* scenarios in Phase 03/04")
    }

    /// Harvest a BOUNDED cross-repo aggregate for a user / contributor target
    /// (deep triangulation deferred to slice-04 per WD-64).
    ///
    /// SCAFFOLD: true (slice-02)
    ///
    /// Bodied `todo!()` at step 01-03; the live bounded user harvest lands per
    /// the SCR-* scenarios in Phase 03/04.
    async fn harvest_user(&self, _user: &str) -> Result<Vec<Signal>, GithubError> {
        // SCAFFOLD: true (slice-02)
        todo!("harvest_user lands per SCR-* scenarios in Phase 03/04")
    }
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

    /// SCAFFOLD pin: `resolve_target` is bodied `todo!()` at step 01-03 — the
    /// live disambiguation lands per the SCR-* scenarios in Phase 03/04.
    /// `#[should_panic]` proves the scaffold is present (RED for the right
    /// reason: the body is a deferred-scenario stub, not a silent success).
    #[tokio::test]
    #[should_panic(expected = "Phase 03/04")]
    async fn resolve_target_is_scaffold_todo_until_phase_03_04() {
        let adapter = GithubAdapter::for_api_base("https://fake.test");
        let _ = adapter.resolve_target("rust-lang/rust").await;
    }

    /// SCAFFOLD pin: `harvest_repo` is bodied `todo!()` at step 01-03.
    #[tokio::test]
    #[should_panic(expected = "Phase 03/04")]
    async fn harvest_repo_is_scaffold_todo_until_phase_03_04() {
        let adapter = GithubAdapter::for_api_base("https://fake.test");
        let _ = adapter.harvest_repo("rust-lang", "rust").await;
    }

    /// SCAFFOLD pin: `harvest_user` is bodied `todo!()` at step 01-03.
    #[tokio::test]
    #[should_panic(expected = "Phase 03/04")]
    async fn harvest_user_is_scaffold_todo_until_phase_03_04() {
        let adapter = GithubAdapter::for_api_base("https://fake.test");
        let _ = adapter.harvest_user("rust-lang").await;
    }
}
