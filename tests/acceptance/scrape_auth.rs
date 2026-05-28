//! Slice-02 acceptance — optional Personal Access Token for higher rate
//! limits (`GITHUB_TOKEN`).
//!
//! `adapter-github` reads an OPTIONAL PAT from the `GITHUB_TOKEN` env var
//! ONLY in slice-02 (WD-54 / WD-63 / ADR-019; config-file support deferred).
//! When present, harvest uses the authenticated rate budget and reports the
//! remaining budget; when absent, harvest runs unauthenticated and degrades
//! gracefully (clear remediation) when the anonymous budget is exhausted.
//! The token is an EFFECT-shell credential held only in `adapter-github`: it
//! is NEVER logged, echoed, written to a claim, or published. `scraper-domain`
//! (pure) never sees it.
//!
//! Layer placement: layer 3 / layer 5 subprocess, example-only (Mandate 11).
//! The token VALUE is asserted ABSENT from all captured output while
//! `FakeGithub::saw_token` confirms the production code actually SENT it (so
//! auth genuinely happened) — the double observes the token without ever
//! surfacing it.
//!
//! Covers:
//! - US-SCR-004: optional PAT for higher rate limits (happy + 3 sad/edge)
//! - WD-54 / WD-63: `GITHUB_TOKEN` env-var only; token never leaks
//
// SCAFFOLD: true

mod support;

#[allow(unused_imports)]
use support::*;

// =============================================================================
// US-SCR-004 — authenticated + unauthenticated harvest
// =============================================================================

/// SA-1 (US-SCR-004 Ex 1 — happy + load-bearing no-token-leak): with a valid
/// `GITHUB_TOKEN`, harvest reports authenticated status + the remaining rate
/// budget and completes for a target that would exhaust the anonymous
/// budget. The token VALUE never appears in any output line, claim, or log —
/// asserted both ways: ABSENT from stdout/stderr AND `FakeGithub::saw_token`
/// confirms the production code DID send it (so auth really happened).
///
/// Given GITHUB_TOKEN holds a valid PAT; When Maria runs `scrape github
/// torvalds`; Then the CLI reports it is authenticated and shows the
/// remaining rate budget, the harvest completes, and the token value never
/// appears in any output line, claim, or log.
///
/// @us-scr-004 @driving_port @real-io @j-004a @wd-63 @happy
#[test]
fn scrape_auth_authenticated_harvest_reports_budget_and_never_leaks_token() {
    let env = TestEnv::initialized();

    // GIVEN an authenticated GitHub posture for the `torvalds` USER target
    // that would exhaust the anonymous budget, carrying a 4982/5000 rate
    // budget; and a valid PAT in the child's `GITHUB_TOKEN`.
    let github = FakeGithub::for_public_user("torvalds", fixture_torvalds_user_aggregate_signals())
        .authenticated(4982, 5000);
    let server = GithubServer::start(github);

    // WHEN Maria runs `scrape github torvalds` with the PAT set.
    let outcome = run_openlore_scrape_with_token(
        &env,
        &["scrape", "github", "torvalds"],
        server.base_url(),
        FIXTURE_VALID_PAT,
    );

    // THEN the run completes (exit 0) and reports the authenticated status +
    // the remaining rate budget verbatim ("authenticated (4982/5000 rate
    // budget)") — the harvest ran on the authenticated budget.
    assert_exit_zero_and_stdout_contains(&outcome, "authenticated (4982/5000 rate budget)");

    // AND the production code DID send the token to GitHub — auth genuinely
    // happened (the PAT only ever leaves the adapter as an Authorization
    // header).
    assert!(
        server.fake().saw_token(FIXTURE_VALID_PAT),
        "the production code must send the PAT so authentication genuinely happens \
         (the only place the token leaves the adapter is the Authorization header)"
    );

    // AND the token VALUE never appears in any captured output line — the
    // load-bearing no-token-leak invariant (US-SCR-004 / WD-63), asserted the
    // other way from `saw_token`: sent to GitHub, never echoed to the user.
    assert_token_value_absent(&outcome, FIXTURE_VALID_PAT);
}

/// SA-2 (US-SCR-004 Ex 2): an UNAUTHENTICATED harvest of a small target
/// succeeds within the anonymous budget and reports unauthenticated status;
/// candidates render normally. No `GITHUB_TOKEN` is set.
///
/// Given no GITHUB_TOKEN is set; When Tobias runs `scrape github
/// small-org/tiny-lib`; Then the CLI reports it is unauthenticated, the
/// harvest completes within the anonymous budget, and candidates render
/// normally.
///
/// @us-scr-004 @driving_port @real-io @j-004a @wd-63 @edge
#[test]
fn scrape_auth_unauthenticated_small_target_succeeds_within_anonymous_budget() {
    // SCAFFOLD: true
    todo!(
        "DELIVER (slice-02): SA-2. GIVEN FakeGithub::for_public_repo(\"small-org/tiny-lib\", \
         fixture_cargo_five_signals()) (anonymous posture) + NO GITHUB_TOKEN in env; WHEN \
         scrape github small-org/tiny-lib; THEN exit 0, stdout reports 'unauthenticated', the \
         harvest completes, and a candidate list renders."
    )
}

// =============================================================================
// US-SCR-004 — rate-limit + token-rejected sad paths (example-only; Mandate 11)
// =============================================================================

/// SA-3 / Sad (US-SCR-004 Ex 3): an UNAUTHENTICATED harvest that exhausts
/// the anonymous budget exits non-zero with a "set GITHUB_TOKEN for higher
/// limits" remediation and renders NO partial candidate list (avoids a
/// misleadingly incomplete proposal set).
///
/// Given no GITHUB_TOKEN is set and the target requires more requests than
/// the anonymous budget allows; When Aanya runs `scrape github torvalds`;
/// Then the CLI exits non-zero, the error suggests setting GITHUB_TOKEN for
/// higher limits, and no partial candidate list is rendered.
///
/// @us-scr-004 @driving_port @real-io @j-004a @error
#[test]
fn scrape_auth_anonymous_rate_limit_exhausted_suggests_token_no_partial_list() {
    // SCAFFOLD: true
    todo!(
        "DELIVER (slice-02): SA-3. GIVEN FakeGithub::rate_limited_anon(\"torvalds\") + NO \
         GITHUB_TOKEN; WHEN scrape github torvalds; THEN exit non-zero, stderr suggests \
         setting GITHUB_TOKEN for higher limits (5000/hour), and stdout renders NO partial \
         numbered candidate list, assert_no_claim_persisted(&env)."
    )
}

/// SA-4 / Sad (US-SCR-004 Ex 4): a stale/invalid `GITHUB_TOKEN` is rejected
/// by GitHub (401); the CLI exits non-zero with an HTTP-401 explanation and
/// a remediation hint (unset the token or provide a valid one) — WITHOUT
/// echoing the token value anywhere.
///
/// Given GITHUB_TOKEN holds a stale or invalid PAT; When Maria runs `scrape
/// github rust-lang/cargo`; Then the CLI exits non-zero with an HTTP 401
/// explanation, the error suggests unsetting the token or providing a valid
/// one, and the token value is not echoed anywhere.
///
/// @us-scr-004 @driving_port @real-io @j-004a @wd-63 @error
#[test]
fn scrape_auth_rejected_token_exits_with_401_without_echoing_value() {
    // SCAFFOLD: true
    todo!(
        "DELIVER (slice-02): SA-4. GIVEN FakeGithub::with_rejected_token(\"rust-lang/cargo\") \
         + GITHUB_TOKEN=FIXTURE_REJECTED_PAT; WHEN scrape github rust-lang/cargo; THEN exit \
         non-zero, stderr explains the HTTP 401 AND hints unset-or-replace the token, \
         assert_token_value_absent(&outcome, FIXTURE_REJECTED_PAT) (value never echoed), \
         assert_no_claim_persisted(&env)."
    )
}

/// SA-5: the token never reaches the PURE `scraper-domain` derivation and is
/// never written into a signed claim even on the AUTHENTICATED happy path
/// where a candidate is subsequently signed. (Defense-in-depth: the token is
/// an effect-shell credential only; a signed-from-authenticated-scrape claim
/// is byte-identical in shape to any other — no token field, no token in the
/// payload.)
///
/// Given GITHUB_TOKEN holds a valid PAT; When Maria runs `scrape github
/// rust-lang/cargo --sign 1` and signs; Then the signed claim's on-disk
/// payload contains NO token value and the captured output never echoes it.
///
/// @us-scr-004 @driving_port @real-io @j-004a @j-004c @wd-63 @edge
#[test]
fn scrape_auth_token_never_reaches_signed_claim_or_output_on_authenticated_sign() {
    // SCAFFOLD: true
    todo!(
        "DELIVER (slice-02): SA-5. GIVEN authenticated posture + GITHUB_TOKEN=FIXTURE_VALID_PAT; \
         WHEN scrape github rust-lang/cargo --sign 1 (sign + publish); THEN exit 0, the \
         on-disk claims/<cid>.json contains NO occurrence of FIXTURE_VALID_PAT, and \
         assert_token_value_absent(&outcome, FIXTURE_VALID_PAT) — the token is an effect-shell \
         credential that never reaches the pure derivation or the signed payload."
    )
}
