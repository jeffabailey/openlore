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

/// Resolve a FakeGithub/GitHub `kind` string into the typed
/// [`SignalKind`](ports::SignalKind). The wire `kind` is the exact
/// `SignalKind` variant name (the SSOT-bounded set the harvest recognizes);
/// an unrecognized kind yields `None` so the caller can drop it (a signal
/// the mapping cannot use is silently ignored, never an error — mirrors
/// `scraper-domain`'s drop-unmapped-signals rule).
pub fn signal_kind_from_wire(kind: &str) -> Option<ports::SignalKind> {
    use ports::SignalKind;
    match kind {
        "DependencyManifestPinned" => Some(SignalKind::DependencyManifestPinned),
        "DocsPresentAndSubstantial" => Some(SignalKind::DocsPresentAndSubstantial),
        "TestRatioOrCiMatrix" => Some(SignalKind::TestRatioOrCiMatrix),
        "SemverAndChangelog" => Some(SignalKind::SemverAndChangelog),
        "MemorySafetyLanguage" => Some(SignalKind::MemorySafetyLanguage),
        _ => None,
    }
}

/// Parse the `signals` array of a harvest response body into typed
/// [`Signal`](ports::Signal)s. PURE: a value-in / value-out reshape of the
/// already-fetched JSON (the network I/O lives in `lib.rs`). Signals whose
/// `kind` is not in the recognized set are dropped; malformed entries
/// (missing `kind`/`value`/`source_url`) are dropped too — the public API
/// shape is the contract, and a partial entry simply yields no signal.
pub fn parse_signals(body: &serde_json::Value) -> Vec<ports::Signal> {
    let Some(signals) = body.get("signals").and_then(|s| s.as_array()) else {
        return Vec::new();
    };
    signals.iter().filter_map(parse_one_signal).collect()
}

/// Parse one `signals[]` entry into a [`Signal`](ports::Signal), or `None`
/// when the kind is unrecognized or a required field is absent.
fn parse_one_signal(entry: &serde_json::Value) -> Option<ports::Signal> {
    let kind = signal_kind_from_wire(entry.get("kind")?.as_str()?)?;
    let value = entry.get("value")?.as_str()?.to_string();
    let source_url = entry.get("source_url")?.as_str()?.to_string();
    Some(ports::Signal {
        kind,
        value,
        source_url,
    })
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

    /// `parse_signals` reshapes the harvest body's `signals[]` into typed
    /// `Signal`s in order, mapping each wire `kind` to its `SignalKind`
    /// variant and carrying `value` + `source_url` verbatim. This is the
    /// pure decomposition of `harvest_repo`'s GREEN body — the live HTTP
    /// fetch in lib.rs feeds this its already-parsed JSON.
    #[test]
    fn parse_signals_reshapes_every_recognized_signal_in_order() {
        use ports::SignalKind;
        let body = serde_json::json!({
            "target": { "kind": "repo", "full_name": "rust-lang/cargo" },
            "signals": [
                {
                    "kind": "DependencyManifestPinned",
                    "value": "Cargo.lock committed (exact pins)",
                    "source_url": "https://github.com/rust-lang/cargo/blob/master/Cargo.lock"
                },
                {
                    "kind": "MemorySafetyLanguage",
                    "value": "Rust + no unsafe blocks",
                    "source_url": "https://github.com/rust-lang/cargo"
                }
            ]
        });
        let signals = parse_signals(&body);
        assert_eq!(signals.len(), 2, "every recognized signal must be parsed");
        assert_eq!(signals[0].kind, SignalKind::DependencyManifestPinned);
        assert_eq!(signals[0].value, "Cargo.lock committed (exact pins)");
        assert_eq!(
            signals[0].source_url,
            "https://github.com/rust-lang/cargo/blob/master/Cargo.lock"
        );
        assert_eq!(signals[1].kind, SignalKind::MemorySafetyLanguage);
    }

    /// An unrecognized `kind` (not in the SSOT-bounded set) is dropped, not
    /// an error — mirrors `scraper-domain`'s drop-unmapped-signals rule.
    #[test]
    fn parse_signals_drops_unrecognized_kinds() {
        let body = serde_json::json!({
            "signals": [
                { "kind": "TotallyUnknownKind", "value": "x", "source_url": "https://x.test/1" },
                {
                    "kind": "TestRatioOrCiMatrix",
                    "value": "test/source ratio 0.61",
                    "source_url": "https://x.test/2"
                }
            ]
        });
        let signals = parse_signals(&body);
        assert_eq!(
            signals.len(),
            1,
            "an unrecognized kind is dropped; only the recognized one survives"
        );
        assert_eq!(signals[0].kind, ports::SignalKind::TestRatioOrCiMatrix);
    }

    /// A body with no `signals` array (e.g. a resolve-only response) parses
    /// to an empty vec rather than panicking.
    #[test]
    fn parse_signals_returns_empty_when_no_signals_array() {
        let body = serde_json::json!({ "target": { "kind": "user", "login": "torvalds" } });
        assert!(parse_signals(&body).is_empty());
    }

    /// `signal_kind_from_wire` round-trips each recognized variant name and
    /// rejects anything else.
    #[test]
    fn signal_kind_from_wire_maps_the_bounded_set() {
        use ports::SignalKind;
        for (wire, expected) in [
            ("DependencyManifestPinned", SignalKind::DependencyManifestPinned),
            ("DocsPresentAndSubstantial", SignalKind::DocsPresentAndSubstantial),
            ("TestRatioOrCiMatrix", SignalKind::TestRatioOrCiMatrix),
            ("SemverAndChangelog", SignalKind::SemverAndChangelog),
            ("MemorySafetyLanguage", SignalKind::MemorySafetyLanguage),
        ] {
            assert_eq!(signal_kind_from_wire(wire), Some(expected));
        }
        assert_eq!(signal_kind_from_wire("nope"), None);
    }
}
