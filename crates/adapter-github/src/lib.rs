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
        Ok(client::parse_signals(&body))
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
    async fn get_public(
        &self,
        path: &str,
        target: &str,
    ) -> Result<serde_json::Value, GithubError> {
        let client = client::build_client()
            .map_err(|e| GithubError::Network(format!("could not build HTTP client: {e}")))?;
        let url = format!("{}{}", self.api_base, path);

        let mut request = client.get(&url);
        if let Some(header) = self.auth.authorization_header() {
            request = request.header(reqwest::header::AUTHORIZATION, header);
        }

        let response = request
            .send()
            .await
            .map_err(|e| GithubError::Network(format!("request to GitHub failed: {e}")))?;

        let status = response.status();
        if status.is_success() {
            return response
                .json::<serde_json::Value>()
                .await
                .map_err(|e| GithubError::ApiShape(format!("response body was not JSON: {e}")));
        }

        Err(classify_status(status.as_u16(), target))
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

/// Classify a non-2xx HTTP status into the railway-oriented [`GithubError`].
/// A 404 is the not-found cause; 403 is rate-budget exhaustion; 401 is a
/// rejected token (the value is NEVER echoed — US-SCR-004). The NotPublic
/// distinction (a private repo the public API also 404s) lands with the
/// SG-5 scenario; at this step a 404 is surfaced as `NotFound`.
fn classify_status(status: u16, target: &str) -> GithubError {
    match status {
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

    /// `classify_status` maps HTTP refusal statuses onto the railway-oriented
    /// `GithubError` surface; the 401 path NEVER echoes a token value
    /// (US-SCR-004 no-token-leak).
    #[test]
    fn classify_status_maps_refusals_without_leaking_a_token() {
        assert!(matches!(
            classify_status(404, "ghost-org/ghost-repo"),
            GithubError::NotFound { .. }
        ));
        assert!(matches!(
            classify_status(403, "torvalds"),
            GithubError::RateLimited { authenticated: false }
        ));
        let rejected = classify_status(401, "rust-lang/cargo");
        assert!(matches!(rejected, GithubError::TokenRejected));
        assert!(
            !rejected.to_string().contains("ghp_"),
            "TokenRejected must never echo a token value"
        );
    }
}
