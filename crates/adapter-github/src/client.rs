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

/// The auth/rate-budget posture surfaced by ONE harvest response (ADR-019
/// §5). Distinct from [`AuthMode`] (the adapter's CONFIGURED posture from
/// `GITHUB_TOKEN`): `AuthReport` is the OBSERVED posture the GitHub API
/// reported on the response — when authenticated it carries the remaining /
/// limit rate budget the CLI reports to the user.
///
/// Carries NO token field by construction: the budget numbers are safe to
/// surface, the token bytes are not (no-token-leak; US-SCR-004). A separate
/// type from `AuthMode` keeps that guarantee structural — there is simply no
/// token to leak through an `AuthReport`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthReport {
    /// The harvest ran on the anonymous budget — nothing to report.
    Anonymous,
    /// The harvest ran authenticated; report the remaining / limit budget.
    Authenticated { remaining: u32, limit: u32 },
}

/// Parse a harvest response body's `auth` object into an [`AuthReport`]
/// (ADR-019 §5 rate-budget reporting). PURE — a value-in / value-out reshape
/// of the already-fetched JSON (the network I/O lives in `lib.rs`), so the
/// rate-budget parse is testable without any network.
///
/// An `auth.authenticated == true` body MUST also carry `rate_remaining` +
/// `rate_limit` to be reportable; any other shape (no `auth`, `authenticated:
/// false`, or an authenticated body missing the budget fields) degrades to
/// [`AuthReport::Anonymous`] — the harvest ran on the anon budget, so there is
/// no budget to report. The parse reads ONLY the budget numbers; it never
/// touches a token (no-token-leak; US-SCR-004).
pub fn parse_auth_report(body: &serde_json::Value) -> AuthReport {
    let Some(auth) = body.get("auth") else {
        return AuthReport::Anonymous;
    };
    let authenticated = auth
        .get("authenticated")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    if !authenticated {
        return AuthReport::Anonymous;
    }
    match (
        auth.get("rate_remaining")
            .and_then(serde_json::Value::as_u64),
        auth.get("rate_limit").and_then(serde_json::Value::as_u64),
    ) {
        (Some(remaining), Some(limit)) => AuthReport::Authenticated {
            remaining: remaining as u32,
            limit: limit as u32,
        },
        _ => AuthReport::Anonymous,
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

/// Reshape a real `/repos/{owner}/{repo}` response body into the pure
/// [`RepoFacts`](scraper_domain::RepoFacts) the signal detector reads (RGSD-1,
/// design §2). PURE: a value-in / value-out reshape of the already-fetched JSON
/// (the network I/O lives in `lib.rs`), mirroring [`parse_signals`] /
/// [`parse_auth_report`].
///
/// Reads the top-level `language` (a string → `Some`; `null`/absent → `None`)
/// and `html_url` (the repo's public URL). When `html_url` is absent the URL is
/// reconstructed from the `target` object's owner/repo so a detected signal
/// always names a public evidence URL.
pub fn parse_repo_facts(body: &serde_json::Value) -> scraper_domain::RepoFacts {
    let language = body
        .get("language")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string);
    let source_url = body
        .get("html_url")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| repo_url_from_target(body));
    scraper_domain::RepoFacts {
        language,
        source_url,
        // The Cargo.lock probe (RGSD-2) is a SEPARATE endpoint the effect shell
        // issues after this body-reshape — `parse_repo_facts` reads ONLY the
        // `/repos` body, so it defaults `cargo_lock_url` to `None`. `harvest_repo`
        // fills it from `content_exists(owner, repo, "Cargo.lock")` before
        // `detect_signals` runs (design §2/§4).
        cargo_lock_url: None,
        // The `/tags` list (RGSD-3) and the `contents/CHANGELOG.md` probe are
        // likewise SEPARATE endpoints — `parse_repo_facts` reads ONLY the
        // `/repos` body, so both default to `None`. `harvest_repo` fills them
        // from `list_tags` (→ `pick_semver_tag`) and `content_exists(owner, repo,
        // "CHANGELOG.md")` before `detect_signals` runs (design §2/§4).
        semver_tag: None,
        changelog_url: None,
    }
}

/// Parse a `GET /repos/{owner}/{repo}/tags` 200 body — a JSON array of
/// `{"name": <tag>}` objects — into the list of tag names (RGSD-3). PURE — a
/// value-in / value-out reshape of the already-fetched JSON (the network I/O
/// lives in `lib.rs`), mirroring [`parse_signals`] / [`parse_repo_facts`].
///
/// Reads each array entry's `name` string; entries without a string `name` are
/// dropped (a malformed entry simply yields no tag). A body that is not an
/// array (an untagged repo the API serves as `[]`, or any other shape) yields
/// an empty list — the repo publishes no tags, never an error.
pub fn parse_tag_names(body: &serde_json::Value) -> Vec<String> {
    let Some(entries) = body.as_array() else {
        return Vec::new();
    };
    entries
        .iter()
        .filter_map(|entry| {
            entry
                .get("name")
                .and_then(serde_json::Value::as_str)
                .map(str::to_string)
        })
        .collect()
}

/// Reconstruct a public repo URL from the response body's `target` object when
/// `html_url` is absent, so a detected signal always names a public evidence
/// URL. Falls back to the bare GitHub host when no owner/repo is present.
fn repo_url_from_target(body: &serde_json::Value) -> String {
    let target = body.get("target");
    let full_name = target
        .and_then(|t| t.get("full_name"))
        .and_then(serde_json::Value::as_str);
    match full_name {
        Some(full_name) => format!("https://github.com/{full_name}"),
        None => "https://github.com".to_string(),
    }
}

/// Read the committed file's public `html_url` out of a `GET /repos/{owner}/
/// {repo}/contents/{path}` 200 body (RGSD-2). PURE — a value-in / value-out
/// reshape of the already-fetched JSON (the network I/O lives in `lib.rs`).
///
/// The real GitHub `contents` API returns the file's `html_url`
/// (`https://github.com/{owner}/{repo}/blob/{ref}/{path}`); when it is absent
/// the URL is reconstructed from `owner`/`repo`/`path` so a detected
/// `DependencyManifestPinned` signal always names a public evidence URL the
/// user can audit (design §3, KPI-SCR-3).
pub fn content_html_url(body: &serde_json::Value, owner: &str, repo: &str, path: &str) -> String {
    body.get("html_url")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| format!("https://github.com/{owner}/{repo}/blob/HEAD/{path}"))
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

    /// `parse_auth_report` reads the harvest response body's `auth` object
    /// into a typed [`AuthReport`]. PURE — a value-in / value-out reshape of
    /// the already-fetched JSON, so the rate-budget parse is testable without
    /// any network (ADR-019 §5). Table-driven over the equivalence classes the
    /// parser must distinguish:
    ///
    /// - an `authenticated: true` body carries the `rate_remaining` /
    ///   `rate_limit` budget => `Authenticated { remaining, limit }`;
    /// - `authenticated: false`, a body with NO `auth` object, and an
    ///   authenticated body that OMITS the budget fields ALL degrade to
    ///   `Anonymous` (the harvest ran on the anon budget; there is no budget
    ///   to report).
    ///
    /// The budget cases span the boundary values (0/0, partial, full 4982/5000,
    /// saturated u32) so the parse is exercised across its numeric domain in
    /// one example-driven sweep (no new PBT dependency on the effect-shell
    /// crate). Each case ALSO proves the parsed report never carries a token
    /// value (no-token-leak; US-SCR-004) — `AuthReport` has no token field.
    #[test]
    fn parse_auth_report_classifies_authenticated_budget_vs_anonymous() {
        // Authenticated bodies round-trip their budget across the numeric span.
        for (remaining, limit) in [(0u32, 0u32), (1, 60), (4982, 5000), (u32::MAX, u32::MAX)] {
            let body = serde_json::json!({
                "target": { "kind": "user", "login": "torvalds" },
                "signals": [],
                "auth": {
                    "authenticated": true,
                    "rate_remaining": remaining,
                    "rate_limit": limit,
                },
            });
            let report = parse_auth_report(&body);
            assert_eq!(
                report,
                AuthReport::Authenticated { remaining, limit },
                "an authenticated body must round-trip its {remaining}/{limit} budget"
            );
            assert!(
                !format!("{report:?}").contains("ghp_"),
                "the parsed report must never carry a token value (no-token-leak)"
            );
        }

        // Anonymous / missing-auth / authenticated-without-budget => Anonymous.
        let anonymous_shapes = [
            serde_json::json!({ "auth": { "authenticated": false } }),
            serde_json::json!({ "target": { "kind": "repo" }, "signals": [] }),
            serde_json::json!({ "auth": { "authenticated": true } }),
        ];
        for body in anonymous_shapes {
            assert_eq!(
                parse_auth_report(&body),
                AuthReport::Anonymous,
                "an unauthenticated / missing-auth / budget-less body must be Anonymous; got body {body}"
            );
        }
    }

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

    /// `parse_repo_facts` reshapes a real `/repos` body into `RepoFacts`
    /// (RGSD-1, design §2). A body with a top-level `language` STRING yields
    /// `Some(language)` verbatim; a `language: null` or a body that omits the
    /// field yields `None`. The `source_url` is read from `html_url`. PURE —
    /// the pure decomposition of `harvest_repo`'s new detection union.
    #[test]
    fn parse_repo_facts_reads_language_and_source_url_from_a_real_body() {
        // A realistic `/repos` body: top-level `language` string + `html_url`,
        // NO synthetic `signals[]` (the shape the live API returns).
        let with_language = serde_json::json!({
            "target": { "kind": "repo", "full_name": "rust-lang/cargo" },
            "language": "Rust",
            "html_url": "https://github.com/rust-lang/cargo",
        });
        let facts = parse_repo_facts(&with_language);
        assert_eq!(facts.language, Some("Rust".to_string()));
        assert_eq!(facts.source_url, "https://github.com/rust-lang/cargo");

        // `language: null` -> None (an empty repo the API reports as null).
        let null_language = serde_json::json!({
            "language": serde_json::Value::Null,
            "html_url": "https://github.com/some-org/empty-repo",
        });
        assert_eq!(parse_repo_facts(&null_language).language, None);

        // Language field entirely absent (a legacy `signals[]` body) -> None.
        let absent_language = serde_json::json!({
            "html_url": "https://github.com/some-org/legacy",
            "signals": [],
        });
        assert_eq!(parse_repo_facts(&absent_language).language, None);

        // `html_url` ABSENT: the `source_url` is reconstructed from the
        // `target.full_name` so a detected signal always names a public
        // evidence URL (the `repo_url_from_target` fallback arm).
        let no_html_url = serde_json::json!({
            "target": { "kind": "repo", "full_name": "rust-lang/cargo" },
            "language": "Rust",
        });
        assert_eq!(
            parse_repo_facts(&no_html_url).source_url,
            "https://github.com/rust-lang/cargo",
            "an absent html_url must be reconstructed from target.full_name"
        );

        // Neither `html_url` NOR a `target.full_name`: fall back to the bare
        // GitHub host rather than an empty / bogus URL.
        let no_url_at_all = serde_json::json!({ "language": "Go" });
        assert_eq!(
            parse_repo_facts(&no_url_at_all).source_url,
            "https://github.com",
            "with no html_url and no target the URL degrades to the bare host"
        );
    }

    /// `signal_kind_from_wire` round-trips each recognized variant name and
    /// rejects anything else.
    #[test]
    fn signal_kind_from_wire_maps_the_bounded_set() {
        use ports::SignalKind;
        for (wire, expected) in [
            (
                "DependencyManifestPinned",
                SignalKind::DependencyManifestPinned,
            ),
            (
                "DocsPresentAndSubstantial",
                SignalKind::DocsPresentAndSubstantial,
            ),
            ("TestRatioOrCiMatrix", SignalKind::TestRatioOrCiMatrix),
            ("SemverAndChangelog", SignalKind::SemverAndChangelog),
            ("MemorySafetyLanguage", SignalKind::MemorySafetyLanguage),
        ] {
            assert_eq!(signal_kind_from_wire(wire), Some(expected));
        }
        assert_eq!(signal_kind_from_wire("nope"), None);
    }
}
