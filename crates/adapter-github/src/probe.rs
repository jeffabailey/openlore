//! `probe` — Earned-Trust startup gauntlet for the GitHub adapter.
//!
//! Implements the pure arms of the five-step probe contract ADR-019 §6
//! requires of `adapter-github`. GitHub is the highest-risk boundary in the
//! slice: it is external, rate-limited, and can MISLEAD about access (a
//! private repo and a missing repo both 404; a rate-limited response can look
//! like a transport error). The probe therefore exercises the SPECIFIC GitHub
//! lies, not just a happy-path fetch:
//!
//! 1. **Public reachability** — `resolve_target` against a stable PUBLIC
//!    fixture returns `TargetKind::Repo` within the 250ms budget (I-5).
//!    Refusal reason: [`ProbeRefusalReason::GithubPublicApiUnreachable`].
//! 2. **Private refusal** — `resolve_target` against a known-private /
//!    inaccessible fixture returns `GithubError::NotPublic` (NOT a silent
//!    empty harvest). This is the load-bearing KPI-SCR-4 check: it catches the
//!    "private repo 404s like a missing one, so we harvested nothing and
//!    called it success" lie. Refusal reason:
//!    [`ProbeRefusalReason::GithubPrivateNotRefused`].
//! 3. **Auth-mode** — a set-but-rejected `GITHUB_TOKEN` (401) refuses to start
//!    rather than silently fall back to the anon budget. Refusal reason:
//!    [`ProbeRefusalReason::GithubTokenRejected`].
//! 4. **Rate-limit-header presence** — assert the budget-reporting path parses
//!    the `X-RateLimit-*` headers (catches a GitHub response-shape change).
//!    Refusal reason: [`ProbeRefusalReason::GithubRateLimitHeadersMissing`].
//! 5. **No-token-leak** — assert the token value never appears in any
//!    structured probe event or log line (US-SCR-004).
//!
//! ## Pure-arm design (nw-fp-domain-modeling §8)
//!
//! Each arm is a pure function over a small input shape so the gauntlet
//! composes cleanly as a railway. The arms consume the *result* of the I/O
//! step (resolve outcome, token-validation outcome, header-presence flag) and
//! produce structured refusals. The live `reqwest` calls live in `lib.rs` so
//! the arms here stay unit-testable without hitting the real GitHub API.
//!
//! Step 01-03 BOOTSTRAP: the arm contracts below are pinned by unit tests; the
//! `lib.rs::probe()` boundary drives them with placeholder inputs until the
//! live network driver lands per the SCR-* scenarios in Phase 03/04. The arm
//! signatures stay identical when the I/O glue fills in around them.

use serde_json::json;

use ports::ProbeRefusalReason;

/// Outcome of one arm of the probe. Aggregated by `lib.rs::probe()` into a
/// `ports::ProbeOutcome`.
#[derive(Debug, Clone)]
pub enum ArmOutcome {
    Ok,
    Refused(ProbeRefusal),
}

/// Structured refusal an arm emits when it trips. Mirrors the public
/// `ProbeOutcome::Refused` shape so the lift at `lib.rs::probe()` is a 1:1
/// field copy.
///
/// CRITICAL (US-SCR-004): no arm ever places the `GITHUB_TOKEN` value into
/// `detail` or `structured`. The no-token-leak invariant is the reason the
/// auth-mode arm takes a `token_rejected: bool` rather than the token bytes.
#[derive(Debug, Clone)]
pub struct ProbeRefusal {
    pub reason: ProbeRefusalReason,
    pub detail: String,
    pub structured: serde_json::Value,
}

// -----------------------------------------------------------------------------
// Arm 1 — public reachability
// -----------------------------------------------------------------------------

/// Inspect the outcome of resolving a stable PUBLIC fixture target. The caller
/// (lib.rs) runs the live `resolve_target` over reqwest; this arm decides
/// whether the outcome is acceptable.
///
/// `resolve_error` is `None` when the public fixture resolved within budget
/// and `Some(detail)` on any failure — unreachable, timeout, unexpected shape.
pub fn check_public_reachability(fixture_target: &str, resolve_error: Option<&str>) -> ArmOutcome {
    match resolve_error {
        None => ArmOutcome::Ok,
        Some(detail) => ArmOutcome::Refused(ProbeRefusal {
            reason: ProbeRefusalReason::GithubPublicApiUnreachable,
            detail: format!(
                "public GitHub API unreachable resolving fixture {fixture_target}: {detail}"
            ),
            structured: json!({
                "fixture_target": fixture_target,
                "resolve_error": detail,
            }),
        }),
    }
}

// -----------------------------------------------------------------------------
// Arm 2 — private refusal (load-bearing KPI-SCR-4)
// -----------------------------------------------------------------------------

/// Inspect whether a known-private / inaccessible fixture was REFUSED. The
/// caller resolves a private fixture; `was_refused` is `true` iff the resolve
/// returned `GithubError::NotPublic` (or `NotFound`) rather than a successful
/// `TargetKind` or a silently-empty harvest.
///
/// This is the load-bearing trust check: a `false` here means the public-only
/// guarantee broke — the adapter treated an inaccessible target as harvestable.
pub fn check_private_refusal(private_fixture: &str, was_refused: bool) -> ArmOutcome {
    if was_refused {
        return ArmOutcome::Ok;
    }
    ArmOutcome::Refused(ProbeRefusal {
        reason: ProbeRefusalReason::GithubPrivateNotRefused,
        detail: format!(
            "private fixture {private_fixture} was NOT refused — the public-data-only \
             guarantee broke (KPI-SCR-4)"
        ),
        structured: json!({
            "private_fixture": private_fixture,
            "was_refused": was_refused,
        }),
    })
}

// -----------------------------------------------------------------------------
// Arm 3 — auth-mode (set-but-rejected token fast-fails)
// -----------------------------------------------------------------------------

/// Inspect the auth-mode validation outcome. The caller validates a configured
/// `GITHUB_TOKEN` against the API; `token_rejected` is `true` iff a token WAS
/// set and the API returned 401.
///
/// CRITICAL: this arm takes a bool, NEVER the token bytes — the no-token-leak
/// invariant (US-SCR-004) forbids the rejected token from appearing in the
/// refusal `detail` / `structured` payload.
pub fn check_auth_mode(token_rejected: bool) -> ArmOutcome {
    if !token_rejected {
        return ArmOutcome::Ok;
    }
    ArmOutcome::Refused(ProbeRefusal {
        reason: ProbeRefusalReason::GithubTokenRejected,
        detail: "configured GITHUB_TOKEN was rejected (401) — refusing to start rather than \
                 silently falling back to the anon budget. The token value is NOT echoed."
            .to_string(),
        structured: json!({
            // Deliberately NO token field here — only the fact of rejection.
            "token_rejected": true,
        }),
    })
}

// -----------------------------------------------------------------------------
// Arm 4 — rate-limit-header presence
// -----------------------------------------------------------------------------

/// Inspect whether the `X-RateLimit-*` headers the budget-reporting path
/// depends on were present in the probe response. `headers_present` is `true`
/// iff both `X-RateLimit-Remaining` and `X-RateLimit-Limit` were parsed.
///
/// A `false` here means a GitHub response-shape change broke the
/// budget-reporting path — refuse so the rate-limit remediation does not
/// silently stop working.
pub fn check_rate_limit_headers(headers_present: bool) -> ArmOutcome {
    if headers_present {
        return ArmOutcome::Ok;
    }
    ArmOutcome::Refused(ProbeRefusal {
        reason: ProbeRefusalReason::GithubRateLimitHeadersMissing,
        detail: "X-RateLimit-Remaining / X-RateLimit-Limit headers absent — the budget-reporting \
                 path cannot function (GitHub response-shape change?)"
            .to_string(),
        structured: json!({
            "rate_limit_headers_present": false,
        }),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const PUBLIC_FIXTURE: &str = "rust-lang/rust";
    const PRIVATE_FIXTURE: &str = "openlore/known-private-probe-fixture";

    // -------- Arm 1: public reachability --------

    #[test]
    fn public_reachability_arm_passes_when_no_error() {
        let outcome = check_public_reachability(PUBLIC_FIXTURE, None);
        assert!(matches!(outcome, ArmOutcome::Ok));
    }

    #[test]
    fn public_reachability_arm_refuses_on_resolve_error() {
        let outcome = check_public_reachability(PUBLIC_FIXTURE, Some("connect timeout"));
        match outcome {
            ArmOutcome::Refused(r) => {
                assert_eq!(r.reason, ProbeRefusalReason::GithubPublicApiUnreachable);
                assert!(r.detail.contains("connect timeout"));
                assert!(r.detail.contains(PUBLIC_FIXTURE));
            }
            ArmOutcome::Ok => panic!("expected refusal for unreachable public API"),
        }
    }

    // -------- Arm 2: private refusal (KPI-SCR-4) --------

    #[test]
    fn private_refusal_arm_passes_when_private_target_refused() {
        let outcome = check_private_refusal(PRIVATE_FIXTURE, /* was_refused */ true);
        assert!(matches!(outcome, ArmOutcome::Ok));
    }

    #[test]
    fn private_refusal_arm_refuses_when_private_target_not_refused() {
        // The load-bearing lie: a private/inaccessible target was treated as
        // harvestable instead of refused.
        let outcome = check_private_refusal(PRIVATE_FIXTURE, /* was_refused */ false);
        match outcome {
            ArmOutcome::Refused(r) => {
                assert_eq!(r.reason, ProbeRefusalReason::GithubPrivateNotRefused);
                assert!(r.detail.contains("public-data-only"));
                assert!(r.detail.contains(PRIVATE_FIXTURE));
            }
            ArmOutcome::Ok => panic!("expected refusal when a private target was not refused"),
        }
    }

    // -------- Arm 3: auth-mode + no-token-leak --------

    #[test]
    fn auth_mode_arm_passes_when_token_not_rejected() {
        let outcome = check_auth_mode(/* token_rejected */ false);
        assert!(matches!(outcome, ArmOutcome::Ok));
    }

    #[test]
    fn auth_mode_arm_refuses_on_rejected_token_without_leaking_it() {
        let outcome = check_auth_mode(/* token_rejected */ true);
        match outcome {
            ArmOutcome::Refused(r) => {
                assert_eq!(r.reason, ProbeRefusalReason::GithubTokenRejected);
                // No-token-leak: the refusal must NOT contain a token VALUE.
                // The arm takes a bool, so by construction it cannot — pin it
                // by asserting a representative GitHub PAT prefix never
                // appears, so a future refactor that threads the token bytes
                // into detail/structured reds here. (We assert on a token
                // *value* sentinel, not the word "token" — the detail string
                // legitimately says "The token value is NOT echoed".)
                let rendered = format!("{}{}", r.detail, r.structured);
                assert!(
                    !rendered.contains("ghp_") && !rendered.contains("github_pat_"),
                    "refusal must never echo a token value: {rendered}"
                );
            }
            ArmOutcome::Ok => panic!("expected refusal for a rejected token"),
        }
    }

    // -------- Arm 4: rate-limit-header presence --------

    #[test]
    fn rate_limit_arm_passes_when_headers_present() {
        let outcome = check_rate_limit_headers(/* headers_present */ true);
        assert!(matches!(outcome, ArmOutcome::Ok));
    }

    #[test]
    fn rate_limit_arm_refuses_when_headers_absent() {
        let outcome = check_rate_limit_headers(/* headers_present */ false);
        match outcome {
            ArmOutcome::Refused(r) => {
                assert_eq!(r.reason, ProbeRefusalReason::GithubRateLimitHeadersMissing);
            }
            ArmOutcome::Ok => panic!("expected refusal when rate-limit headers are absent"),
        }
    }
}
