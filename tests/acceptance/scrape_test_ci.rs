//! RGSD-5 acceptance — real-GitHub signal detection (CI workflows / tests dir).
//!
//! Feature `real-github-signal-detection`, slice RGSD-5 (design §5) — the FIFTH
//! and last detector. The GitHub scraper must detect its
//! `SignalKind::TestRatioOrCiMatrix` signal from test-infrastructure evidence —
//! the DISJUNCTION of EITHER (a) the repo has CI workflows (`GET
//! /repos/{o}/{r}/contents/.github/workflows` → 200) OR (b) a `tests/` directory
//! (`GET /repos/{o}/{r}/contents/tests` → 200) — rather than the synthetic
//! `signals[]` array only the legacy `FakeGithub` postures inject (design §1/§4).
//! This is the "test-driven" heuristic. Both probes REUSE RGSD-2's `contents/*`
//! fork (200 = present, 404 = absent) — NO new endpoint type.
//!
//! SPIKE-verified against real GitHub: ripgrep's `contents/.github/workflows` →
//! 200 AND `contents/tests` → 200 (fires via EITHER disjunct); octocat/Hello-World
//! → 404/404 (a clean negative — neither disjunct fires).
//!
//! The "test/source ratio > 0.5" precision (which needs a full recursive tree
//! walk) is DEFERRED — the walking skeleton uses the cheap directory-presence
//! proxy; the emitted signal is honest about what was measured (design §3: "CI
//! workflows present" / "tests/ directory present"), never claiming a ratio was
//! computed.
//!
//! These are layer-3 subprocess acceptance scenarios (example-only per Mandate
//! 11) driven through the real `openlore` binary via the
//! `OPENLORE_GITHUB_API_BASE` seam. They assert ONLY the observable CLI surface
//! (exit code + stdout) — no scraper-domain / adapter-github struct field is
//! named (Mandate 8 universe = port-exposed CLI output).
//!
//! RED cause (design §1/§2): `adapter-github::harvest_repo` today fetches `GET
//! /repos/{o}/{r}` (RGSD-1 language), probes `contents/Cargo.lock` (RGSD-2), lists
//! `/tags`, probes `contents/CHANGELOG.md` (RGSD-3), fetches `/readme`, and probes
//! `contents/docs` (RGSD-4). It NEVER probes `contents/.github/workflows` nor
//! `contents/tests`, so no `TestRatioOrCiMatrix` signal is produced → no
//! `org.openlore.philosophy.test-driven` candidate. The `RepoFacts.{ci_workflows_url,
//! tests_dir_url}` fields and the `TestRatioOrCiMatrix` arm of `detect_signals`
//! do not exist yet; those pure/effect functions are DELIVER's RED_UNIT (design
//! §8). So:
//!
//!   * happy A (CI workflows) + happy B (tests/ dir) are RED today — the
//!     `test-driven` candidate is absent (MISSING_FUNCTIONALITY), and each turns
//!     GREEN once DELIVER lands the two `content_exists` probes + the
//!     `TestRatioOrCiMatrix` arm;
//!   * the negative (neither CI workflows nor tests dir) is a GREEN-today
//!     guardrail — no candidate is produced regardless today; it becomes
//!     load-bearing once detection exists, pinning that NEITHER disjunct fires
//!     when both are absent (mirrors real octocat/Hello-World).
//!
//! Covers:
//! - RGSD-5 (design §5): detect TestRatioOrCiMatrix from CI workflows OR a tests/
//!   dir; a repo with either yields the test-driven candidate end-to-end, a repo
//!   with neither does not.

mod support;

#[allow(unused_imports)]
use support::*;

/// The candidate object the `TestRatioOrCiMatrix` signal maps to
/// (`signal_predicate_mapping.yaml` — mapping SSOT, unchanged by this slice).
const TEST_DRIVEN_OBJECT: &str = "org.openlore.philosophy.test-driven";

// =============================================================================
// RGSD-5 — CI-workflows / tests-dir-derived TestRatioOrCiMatrix detection (§5)
// =============================================================================

/// RGSD-5 happy A — CI workflows (RED today):
/// a real public repo whose `contents/.github/workflows` probe returns 200 (CI
/// workflows present) and whose `contents/tests` probe returns 404 (NO tests
/// dir) — with NO synthetic `signals[]`, NO memory-safe `language`, NO
/// `Cargo.lock`, NO semver tags / CHANGELOG, NO README / docs (so ONLY
/// test-driven can fire) — must, when scraped, derive the
/// `org.openlore.philosophy.test-driven` candidate and name its CI/workflows
/// source. This closes the RGSD-5 loop end-to-end through the real CLI, pinning
/// that CI workflows ALONE fire the signal (the first disjunct). Mirrors real
/// ripgrep (`contents/.github/workflows` → 200).
///
/// Given a public repo with CI workflows and no tests/ dir (it drives its
/// quality through continuous integration); When Maria scrapes the repo; Then
/// the CLI exits 0 and the derived candidate list proposes the test-driven
/// philosophy, naming its CI/workflows evidence.
///
/// RED today: harvest never probes `contents/.github/workflows` nor
/// `contents/tests` → 0 test-infrastructure signals → the test-driven candidate
/// is absent (MISSING_FUNCTIONALITY). GREEN once DELIVER lands the two
/// `content_exists` probes + `RepoFacts.{ci_workflows_url,tests_dir_url}` +
/// `detect_signals`'s `TestRatioOrCiMatrix` arm.
///
/// @rgsd-5 @real-io @driving_port @happy
#[test]
fn scrape_repo_with_ci_workflows_derives_the_test_driven_candidate() {
    // GIVEN an initialized env + a public repo whose `contents/.github/workflows`
    // probe returns 200 (CI workflows present) and whose `contents/tests` probe
    // returns 404 (NO tests dir). NO `language`, NO Cargo.lock, NO tags/CHANGELOG,
    // NO README/docs — so test-driven is the ONLY signal that can fire (isolating
    // it from the RGSD-1/2/3/4 detections).
    let env = TestEnv::initialized();
    let github = GithubServer::start(FakeGithub::for_public_repo_with_ci_workflows(
        "BurntSushi/ripgrep",
    ));

    // WHEN Maria scrapes the public repo (no --sign — this is a pure read).
    let outcome = run_openlore_scrape(
        &env,
        &["scrape", "github", "BurntSushi/ripgrep"],
        github.base_url(),
    );

    // THEN the scrape exits 0 (a well-formed public repo is never an error) ...
    assert_eq!(
        outcome.status, 0,
        "scrape of a resolvable public repo must exit 0; \n--- stdout ---\n{}\n--- stderr ---\n{}",
        outcome.stdout, outcome.stderr
    );

    let stdout = &outcome.stdout;

    // ... AND the derived candidate list proposes the test-driven philosophy
    // (design §5): detection probes `contents/.github/workflows`, sees the 200,
    // fires the `TestRatioOrCiMatrix` signal, and the mapping derives this object.
    assert!(
        stdout.contains(TEST_DRIVEN_OBJECT),
        "a repo with CI workflows must derive the test-driven candidate \
         ({TEST_DRIVEN_OBJECT}) from its `contents/.github/workflows` evidence \
         (RGSD-5 design §5); \n--- stdout ---\n{stdout}\n--- stderr ---\n{}",
        outcome.stderr
    );

    // ... AND the proposal names its source — the CI workflows the harvest probed
    // (design §3: the emitted signal is honest about what was measured). The
    // workflows source_url (the dir html_url, which contains "workflows")
    // surfaces so the user can audit WHY the candidate was proposed (auditability,
    // KPI-SCR-3). We pin only that the `workflows` token surfaces, not the exact
    // source-signal copy (DELIVER owns the wording).
    assert!(
        stdout.contains("workflows"),
        "the test-driven proposal must name its source — the CI \"workflows\" — \
         so the user can audit why it was derived (design §3, KPI-SCR-3); \n\
         --- stdout ---\n{stdout}\n--- stderr ---\n{}",
        outcome.stderr
    );
}

/// RGSD-5 happy B — tests/ directory (RED today):
/// a real public repo with a `tests/` directory present (`contents/tests` → 200)
/// but NO CI workflows (`contents/.github/workflows` → 404) — with NO synthetic
/// `signals[]`, NO `language`, NO Cargo.lock, NO semver tags / CHANGELOG, NO
/// README / docs — must, when scraped, derive the `test-driven` candidate. This
/// pins the OR: a `tests/` dir ALONE fires the signal even when there are no CI
/// workflows (the second disjunct), so detection is a DISJUNCTION, not a
/// CI-only rule.
///
/// Given a public repo with a tests/ directory but no CI workflows (it invests
/// in a test suite); When Maria scrapes the repo; Then the CLI exits 0 and the
/// test-driven philosophy is proposed.
///
/// RED today: harvest never probes `contents/.github/workflows` nor
/// `contents/tests` → 0 test-infrastructure signals → the test-driven candidate
/// is absent (MISSING_FUNCTIONALITY). GREEN once DELIVER lands the `contents/tests`
/// probe + the `TestRatioOrCiMatrix` arm.
///
/// @rgsd-5 @real-io @driving_port @happy
#[test]
fn scrape_repo_with_a_tests_directory_derives_the_test_driven_candidate() {
    // GIVEN an initialized env + a public repo with a `tests/` dir present
    // (`contents/tests` → 200) but NO CI workflows (`contents/.github/workflows` →
    // 404). NO `language`, NO Cargo.lock, NO tags/CHANGELOG, NO README/docs — so
    // test-driven is the ONLY signal that can fire, and the tests-dir disjunct is
    // the thing under test.
    let env = TestEnv::initialized();
    let github = GithubServer::start(FakeGithub::for_public_repo_with_tests_dir(
        "some-org/tested",
    ));

    // WHEN Maria scrapes the public repo (no --sign — a pure read).
    let outcome = run_openlore_scrape(
        &env,
        &["scrape", "github", "some-org/tested"],
        github.base_url(),
    );

    // THEN the scrape exits 0 (a well-formed public repo is never an error) ...
    assert_eq!(
        outcome.status, 0,
        "scrape of a resolvable public repo must exit 0; \n--- stdout ---\n{}\n--- stderr ---\n{}",
        outcome.stdout, outcome.stderr
    );

    // ... AND the test-driven candidate IS proposed: detection is a DISJUNCTION,
    // so a `tests/` dir ALONE (without CI workflows) must fire the signal (design
    // §5 — the tests-dir disjunct).
    let stdout = &outcome.stdout;
    assert!(
        stdout.contains(TEST_DRIVEN_OBJECT),
        "a repo with a tests/ directory must derive the test-driven candidate \
         ({TEST_DRIVEN_OBJECT}) from its `contents/tests` evidence — detection is \
         the DISJUNCTION of CI workflows OR a tests/ dir, so tests/ alone fires \
         (design §5); \n--- stdout ---\n{stdout}\n--- stderr ---\n{}",
        outcome.stderr
    );
}

/// RGSD-5 negative (edge — GREEN-today guardrail):
/// a real public repo whose `contents/.github/workflows` probe returns 404 (NO
/// CI workflows) AND whose `contents/tests` probe returns 404 (NO tests dir) must
/// NOT derive the test-driven candidate. Neither disjunct holds: no CI workflows
/// AND no tests dir (design §5). This mirrors the real octocat/Hello-World case
/// (`contents/.github/workflows` → 404, `contents/tests` → 404).
///
/// Given a public repo with neither CI workflows nor a tests/ dir; When Maria
/// scrapes the repo; Then the CLI exits 0 and no test-driven philosophy is
/// proposed.
///
/// GREEN today: no candidate is produced regardless (harvest never probes either
/// path). This becomes the load-bearing disjunction-guard once detection exists —
/// it must stay GREEN when the happy scenarios turn GREEN (a repo with neither
/// disjunct MUST NOT fire the signal).
///
/// @rgsd-5 @real-io @driving_port @edge @guardrail
#[test]
fn scrape_repo_with_neither_ci_nor_tests_proposes_no_test_driven_candidate() {
    // GIVEN an initialized env + a public repo whose `contents/.github/workflows`
    // AND `contents/tests` probes BOTH return 404 (neither disjunct present). NO
    // `language`, NO Cargo.lock, NO tags/CHANGELOG, NO README/docs — so RGSD-1/2/3/4
    // stay quiet and the test-driven ABSENCE is the thing under test.
    let env = TestEnv::initialized();
    let github = GithubServer::start(FakeGithub::for_public_repo_with_test_evidence(
        "octocat/Hello-World",
        false, // no CI workflows
        false, // no tests/ dir
    ));

    // WHEN Maria scrapes the public repo (no --sign — a pure read).
    let outcome = run_openlore_scrape(
        &env,
        &["scrape", "github", "octocat/Hello-World"],
        github.base_url(),
    );

    // THEN the scrape exits 0 (a resolvable public repo with nothing to propose
    // is not an error — US-SCR-002 Ex 2) ...
    assert_eq!(
        outcome.status, 0,
        "scrape of a resolvable public repo must exit 0 even when nothing is \
         proposed; \n--- stdout ---\n{}\n--- stderr ---\n{}",
        outcome.stdout, outcome.stderr
    );

    // ... AND NO test-driven candidate is proposed: neither disjunct holds, so a
    // repo with no CI workflows AND no tests/ dir must NEVER fire the signal
    // (design §5 — the disjunction guard).
    let stdout = &outcome.stdout;
    assert!(
        !stdout.contains(TEST_DRIVEN_OBJECT),
        "a repo with neither CI workflows nor a tests/ dir must NOT derive the \
         test-driven candidate ({TEST_DRIVEN_OBJECT}) — neither disjunct holds, so \
         the signal must never fire (design §5); \n--- stdout ---\n{stdout}\n--- \
         stderr ---\n{}",
        outcome.stderr
    );
}
