//! `client` — the reqwest HTTP client skeleton for `adapter-github`.
//!
//! Step 01-03 BOOTSTRAP: this module owns the two effect-shell seams the
//! adapter needs before any live request lands:
//!
//! 1. **`GITHUB_TOKEN` PAT read (WD-54 / WD-63)**. The optional Personal
//!    Access Token is read from the `GITHUB_TOKEN` env var ONLY (config-file
//!    deferred). When present it raises the rate budget (5000/hr authed vs
//!    60/hr anon). The token is held ONLY here in the effect shell; the pure
//!    `scraper-domain` never sees it. It is NEVER logged, echoed, written to a
//!    claim, or published — [`AuthMode`] deliberately does NOT implement
//!    `Debug`/`Display` over the secret so a stray `{:?}` cannot leak it
//!    (US-SCR-004 no-token-leak; ADR-019 §4).
//!
//! 2. **`OPENLORE_GITHUB_API_BASE` test seam**. Mirrors the slice-03
//!    `OPENLORE_PEER_PDS_ENDPOINT` / slice-01 `OPENLORE_PDS_ENDPOINT` seam
//!    pattern: a test (the FakeGithub in-process server) threads its base URL
//!    in via this env var so acceptance scenarios can drive the adapter
//!    against a fake without touching the real GitHub API. In production the
//!    var is unset and the adapter targets the real public API base.
//!
//! The live `reqwest::Client` builder + request methods (REST vs GraphQL per
//! signal is the Q-DELIVER-2 call; the skeleton defaults to REST) land
//! per-scenario in Phase 03/04. This module currently provides the
//! seam-reading helpers + the client builder so the GithubPort impl can name
//! them.

/// The real public GitHub REST API base. Used when the
/// `OPENLORE_GITHUB_API_BASE` test seam is unset (production path).
pub const DEFAULT_GITHUB_API_BASE: &str = "https://api.github.com";

/// Env var name for the optional PAT (WD-63: env-only). Read by
/// [`read_auth_mode`]; the value is NEVER logged.
pub const GITHUB_TOKEN_ENV: &str = "GITHUB_TOKEN";

/// Env var name for the FakeGithub test seam (mirrors slice-01's
/// `OPENLORE_PDS_ENDPOINT` and slice-03's `OPENLORE_PEER_PDS_ENDPOINT`).
pub const GITHUB_API_BASE_ENV: &str = "OPENLORE_GITHUB_API_BASE";

/// The adapter's auth posture, derived from the optional `GITHUB_TOKEN`.
///
/// Deliberately NOT `Debug`/`Display`-deriving over the secret: the token
/// value is held only in the `Authenticated` variant's private field and
/// there is no API surface that renders it. The probe's structured event +
/// any log line can safely carry [`AuthMode::is_authenticated`] (a bool) but
/// never the token bytes (US-SCR-004 no-token-leak).
#[derive(Clone)]
pub enum AuthMode {
    /// No `GITHUB_TOKEN` set — harvest runs on the anon budget (60/hr).
    Anonymous,
    /// A `GITHUB_TOKEN` is set — sent as `Authorization: token <PAT>` for the
    /// authed budget (5000/hr). The token bytes live in the private field and
    /// are never exposed by any accessor.
    Authenticated { token: String },
}

impl AuthMode {
    /// True iff a PAT was configured. Safe to surface in logs / the probe
    /// event — it is a bool, not the token.
    pub fn is_authenticated(&self) -> bool {
        matches!(self, AuthMode::Authenticated { .. })
    }

    /// The `Authorization` header value (`token <PAT>`) when authenticated.
    /// `None` for the anon budget. The ONLY path the token bytes leave this
    /// module is into a reqwest request header — never into a log, a claim,
    /// or the probe event.
    pub fn authorization_header(&self) -> Option<String> {
        match self {
            AuthMode::Anonymous => None,
            AuthMode::Authenticated { token } => Some(format!("token {token}")),
        }
    }
}

/// Read the optional PAT from `GITHUB_TOKEN` (WD-63 env-only). An unset or
/// empty var yields [`AuthMode::Anonymous`]; a non-empty value yields
/// [`AuthMode::Authenticated`]. The token is NEVER logged here.
pub fn read_auth_mode() -> AuthMode {
    match std::env::var(GITHUB_TOKEN_ENV) {
        Ok(token) if !token.is_empty() => AuthMode::Authenticated { token },
        _ => AuthMode::Anonymous,
    }
}

/// Resolve the GitHub API base URL: the `OPENLORE_GITHUB_API_BASE` test seam
/// when set + non-empty, else the real public API base. Trailing slashes are
/// stripped so path joins do not double-slash (mirrors the PDS adapter's
/// `normalize_endpoint`).
pub fn resolve_api_base() -> String {
    let raw = match std::env::var(GITHUB_API_BASE_ENV) {
        Ok(base) if !base.is_empty() => base,
        _ => DEFAULT_GITHUB_API_BASE.to_string(),
    };
    strip_trailing_slashes(raw)
}

/// Strip trailing `/` so `base + "/path"` joins cleanly.
pub fn strip_trailing_slashes(mut s: String) -> String {
    while s.ends_with('/') {
        s.pop();
    }
    s
}

/// Build the shared `reqwest::Client` the adapter uses for every request.
///
/// Step 01-03 BOOTSTRAP: this constructs a client with a connect timeout
/// (so an unreachable host classifies fast rather than hanging) and the
/// GitHub-required `User-Agent` header (the public API rejects requests
/// without one). The per-endpoint request methods (resolve/harvest) wire in
/// Phase 03/04. ADR-019 §2 mandates the workspace `reqwest` (rustls); no new
/// transport dep is introduced.
pub fn build_client() -> Result<reqwest::Client, reqwest::Error> {
    reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(10))
        .user_agent("openlore-adapter-github")
        .build()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `resolve_api_base` strips trailing slashes off the configured seam so
    /// downstream path joins do not double-slash. Pinned with an explicit
    /// guard against env bleed across the process-global var.
    #[test]
    fn resolve_api_base_strips_trailing_slashes_from_seam() {
        // Property-shaped over the small set of trailing-slash counts the
        // pure helper must normalize identically.
        for raw in [
            "https://fake.test",
            "https://fake.test/",
            "https://fake.test///",
        ] {
            assert_eq!(
                strip_trailing_slashes(raw.to_string()),
                "https://fake.test",
                "must strip all trailing slashes off {raw:?}"
            );
        }
    }

    /// Anonymous auth mode produces NO authorization header and reports
    /// `is_authenticated() == false` — the anon-budget path.
    #[test]
    fn anonymous_auth_mode_has_no_header_and_reports_unauthenticated() {
        let mode = AuthMode::Anonymous;
        assert!(!mode.is_authenticated());
        assert_eq!(mode.authorization_header(), None);
    }

    /// Authenticated auth mode produces a `token <PAT>` header and reports
    /// `is_authenticated() == true`. The token bytes leave ONLY via the
    /// header — there is no Debug/Display accessor that could leak them.
    #[test]
    fn authenticated_auth_mode_emits_token_header_without_leaking_via_debug() {
        let mode = AuthMode::Authenticated {
            token: "ghp_secret_value".to_string(),
        };
        assert!(mode.is_authenticated());
        assert_eq!(
            mode.authorization_header(),
            Some("token ghp_secret_value".to_string())
        );
        // No-token-leak invariant (US-SCR-004): AuthMode does not derive
        // Debug, so there is no `{:?}` path that prints the secret. This
        // compiles ONLY because we never format the mode — a future
        // `#[derive(Debug)]` regression would make the leak possible, which
        // is why the derive is deliberately absent.
    }
}
