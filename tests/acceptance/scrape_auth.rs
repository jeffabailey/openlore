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
    let env = TestEnv::initialized();

    // GIVEN an UNAUTHENTICATED (anonymous) GitHub posture for a SMALL public
    // repo target whose five signals stay well within the anonymous rate
    // budget. `for_public_repo` defaults to `FakeAuthMode::Anonymous`, so no
    // `GITHUB_TOKEN` is implied — and `run_openlore_scrape` (below) sets none.
    let github =
        FakeGithub::for_public_repo("small-org/tiny-lib", fixture_cargo_five_signals());
    let server = GithubServer::start(github);

    // WHEN Tobias runs `scrape github small-org/tiny-lib` with NO GITHUB_TOKEN
    // set (the token-less harvest helper, mirroring SA-1's token helper).
    let outcome = run_openlore_scrape(
        &env,
        &["scrape", "github", "small-org/tiny-lib"],
        server.base_url(),
    );

    // THEN the run completes within the anonymous budget (exit 0) and reports
    // the unauthenticated status verbatim (the `render_auth_report` →
    // "unauthenticated" line wired in 04-06).
    assert_exit_zero_and_stdout_contains(&outcome, "unauthenticated");

    // AND the candidate list renders normally — the harvested signals map to
    // at least one numbered candidate under the resolved subject header.
    assert!(
        outcome.stdout.contains("Candidate claims for subject")
            && outcome.stdout.contains(" [1] "),
        "expected an unauthenticated small-target harvest to render a numbered \
         candidate list (US-SCR-004 Ex 2); \n--- stdout ---\n{}\n--- stderr ---\n{}",
        outcome.stdout,
        outcome.stderr
    );
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
    let env = TestEnv::initialized();

    // GIVEN an UNAUTHENTICATED posture whose harvest exhausts the anonymous
    // rate budget: `rate_limited_anon` serves a 403 + rate-limit body (no
    // `GITHUB_TOKEN` is set by `run_openlore_scrape`). The adapter classifies
    // the 403 as `GithubError::RateLimited { authenticated: false }` (03-05).
    let github = GithubServer::start(FakeGithub::rate_limited_anon("torvalds"));

    // WHEN Aanya scrapes the rate-limited target with NO GITHUB_TOKEN.
    let outcome = run_openlore_scrape(&env, &["scrape", "github", "torvalds"], github.base_url());

    // THEN the run exits NON-ZERO (an exhausted budget is an error, not a
    // partial harvest — US-SCR-004 Ex 3).
    assert_ne!(
        outcome.status, 0,
        "an exhausted anonymous rate budget must exit non-zero; \n--- stdout ---\n{}\n--- stderr ---\n{}",
        outcome.stdout, outcome.stderr
    );

    // AND the error on stderr NAMES the rate-limit cause (the railway-oriented
    // `GithubError::RateLimited` Display: "github rate limit exhausted ...").
    assert!(
        outcome.stderr.contains("rate limit"),
        "stderr must name the rate-limit cause; \n--- stderr ---\n{}",
        outcome.stderr
    );

    // AND it SUGGESTS setting GITHUB_TOKEN for a higher limit — the remediation
    // an unauthenticated user needs (US-SCR-004 Ex 3). The token env-var name is
    // surfaced verbatim so the remediation is actionable.
    assert!(
        outcome.stderr.contains("GITHUB_TOKEN"),
        "stderr must suggest setting GITHUB_TOKEN for a higher limit; \n--- stderr ---\n{}",
        outcome.stderr
    );

    // AND NO partial candidate list is rendered: a rate-limited harvest could
    // produce a MISLEADINGLY incomplete proposal set, so the refusal
    // short-circuits BEFORE any candidate-list output (no `[1]` line, no
    // candidate-list footer).
    assert!(
        !outcome.stdout.contains("[1]"),
        "a rate-limited scrape must render NO partial numbered candidate list; \n--- stdout ---\n{}",
        outcome.stdout
    );
    assert!(
        !outcome
            .stdout
            .contains("nothing is a claim until you sign it"),
        "a rate-limited scrape must render NO candidate-list footer; \n--- stdout ---\n{}",
        outcome.stdout
    );

    // AND nothing was persisted: zero `claims` rows, zero PDS writes, zero
    // claim artifact files (scraper_never_persists_unsigned holds on the
    // error path too — a refused scrape is never a mutation).
    assert_no_claim_persisted(&env);
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
    let env = TestEnv::initialized();

    // GIVEN a GitHub posture that REJECTS the configured token with HTTP 401
    // ("Bad credentials") — the stale/invalid-PAT case. The fake fails the
    // auth check FIRST (before any resolution), exactly as the real API does;
    // the adapter classifies the 401 as `GithubError::TokenRejected` (03-05),
    // whose Display carries NO token value (01-01 ports invariant).
    let server = GithubServer::start(FakeGithub::with_rejected_token("rust-lang/cargo"));

    // WHEN Maria runs `scrape github rust-lang/cargo` with a stale/invalid PAT
    // in the child's `GITHUB_TOKEN` (the token-carrying harvest helper).
    let outcome = run_openlore_scrape_with_token(
        &env,
        &["scrape", "github", "rust-lang/cargo"],
        server.base_url(),
        FIXTURE_REJECTED_PAT,
    );

    // THEN the run exits NON-ZERO (a rejected credential is an error, not a
    // partial harvest — US-SCR-004 Ex 4).
    assert_ne!(
        outcome.status, 0,
        "a rejected token must exit non-zero; \n--- stdout ---\n{}\n--- stderr ---\n{}",
        outcome.stdout, outcome.stderr
    );

    // AND the production code DID send the rejected token to GitHub — auth was
    // genuinely attempted (the PAT only ever leaves the adapter as an
    // Authorization header; that attempt is what earned the 401).
    assert!(
        server.fake().saw_token(FIXTURE_REJECTED_PAT),
        "the production code must send the PAT so the 401 genuinely came from an \
         auth attempt (the only place the token leaves the adapter is the \
         Authorization header)"
    );

    // AND stderr EXPLAINS the HTTP 401 (the railway-oriented
    // `GithubError::TokenRejected` Display: "github token rejected (401) ...").
    assert!(
        outcome.stderr.contains("401"),
        "stderr must explain the HTTP 401 cause; \n--- stderr ---\n{}",
        outcome.stderr
    );

    // AND it HINTS the remediation — unset the stale token or provide a valid
    // one. The Display names the `GITHUB_TOKEN` env-var as stale/invalid so the
    // remediation is actionable (US-SCR-004 Ex 4).
    assert!(
        outcome.stderr.contains("GITHUB_TOKEN")
            && (outcome.stderr.contains("stale") || outcome.stderr.contains("invalid")),
        "stderr must hint unsetting/replacing the stale-or-invalid GITHUB_TOKEN; \n--- stderr ---\n{}",
        outcome.stderr
    );

    // AND the rejected token VALUE never appears in any captured output line —
    // the load-bearing no-token-leak invariant (US-SCR-004 / WD-63): the value
    // was sent to GitHub (saw_token above) but is NEVER echoed back to the user
    // in the 401 explanation or anywhere else.
    assert_token_value_absent(&outcome, FIXTURE_REJECTED_PAT);

    // AND nothing was persisted: a refused scrape is never a mutation
    // (`scraper_never_persists_unsigned` holds on the error path too).
    assert_no_claim_persisted(&env);
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
