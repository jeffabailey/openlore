//! RGSD-4 acceptance — real-GitHub signal detection (substantial README / docs dir).
//!
//! Feature `real-github-signal-detection`, slice RGSD-4 (design §5). The GitHub
//! scraper must detect its `SignalKind::DocsPresentAndSubstantial` signal from
//! documentation evidence — the DISJUNCTION of EITHER (a) a SUBSTANTIAL README
//! (`GET /repos/{owner}/{repo}/readme` → the `size` in bytes ≥ a threshold) OR
//! (b) a present `docs/` directory (`GET /repos/{o}/{r}/contents/docs` → 200) —
//! rather than the synthetic `signals[]` array only the legacy `FakeGithub`
//! postures inject (design §1/§4). This is the "documentation-first" heuristic.
//! It adds exactly ONE new endpoint (`/readme`) to the harvest and reuses
//! RGSD-2's `contents/*` probe for the `docs/` directory.
//!
//! SPIKE-verified against real GitHub: ripgrep's `/readme` reports `size` 21615
//! bytes + `html_url` (a substantial README; `contents/docs` → 404 — fires via
//! the README alone); octocat/Hello-World's `/readme` reports `size` 13 bytes +
//! `contents/docs` → 404 (a clean negative — a tiny README, no docs dir, neither
//! disjunct fires).
//!
//! These are layer-3 subprocess acceptance scenarios (example-only per Mandate
//! 11) driven through the real `openlore` binary via the
//! `OPENLORE_GITHUB_API_BASE` seam. They assert ONLY the observable CLI surface
//! (exit code + stdout) — no scraper-domain / adapter-github struct field is
//! named (Mandate 8 universe = port-exposed CLI output).
//!
//! RED cause (design §1/§2): `adapter-github::harvest_repo` today fetches only
//! `GET /repos/{o}/{r}` (RGSD-1 language), probes `contents/Cargo.lock` (RGSD-2),
//! lists `/tags`, and probes `contents/CHANGELOG.md` (RGSD-3). It NEVER fetches
//! `/readme` and NEVER probes `contents/docs`, so no `DocsPresentAndSubstantial`
//! signal is produced → no `org.openlore.philosophy.documentation-first`
//! candidate. The `fetch_readme` effect, the `RepoFacts.{readme_bytes,
//! readme_url, docs_url}` fields, the `README_SUBSTANTIAL_BYTES` threshold, and
//! the `DocsPresentAndSubstantial` arm of `detect_signals` do not exist yet;
//! those pure/effect functions are DELIVER's RED_UNIT (design §8). So:
//!
//!   * happy A (substantial README) + happy B (docs/ dir) are RED today — the
//!     `documentation-first` candidate is absent (MISSING_FUNCTIONALITY), and
//!     each turns GREEN once DELIVER lands the `/readme` fetch + `contents/docs`
//!     probe + the `DocsPresentAndSubstantial` arm;
//!   * the negative (tiny README, no docs) is a GREEN-today guardrail — no
//!     candidate is produced regardless today; it becomes load-bearing once
//!     detection exists, pinning that NEITHER disjunct fires below the threshold
//!     without a docs dir (a tiny README alone must NOT fire).
//!
//! Covers:
//! - RGSD-4 (design §5): detect DocsPresentAndSubstantial from a substantial
//!   README OR a docs/ dir; a repo with either yields the documentation-first
//!   candidate end-to-end, a repo with neither does not.

mod support;

#[allow(unused_imports)]
use support::*;

/// The candidate object the `DocsPresentAndSubstantial` signal maps to
/// (`signal_predicate_mapping.yaml` — mapping SSOT, unchanged by this slice).
const DOCUMENTATION_FIRST_OBJECT: &str = "org.openlore.philosophy.documentation-first";

// =============================================================================
// RGSD-4 — README/docs-derived DocsPresentAndSubstantial detection (§5)
// =============================================================================

/// RGSD-4 happy A — substantial README (RED today):
/// a real public repo whose `/readme` reports a clearly-large `size` (e.g.
/// 20000 bytes, well above the substantiality threshold) and whose
/// `contents/docs` probe returns 404 (NO docs dir) — with NO synthetic
/// `signals[]`, NO memory-safe `language`, NO `Cargo.lock`, NO semver tags /
/// CHANGELOG (so ONLY documentation-first can fire) — must, when scraped, derive
/// the `org.openlore.philosophy.documentation-first` candidate and name its
/// README source. This closes the RGSD-4 loop end-to-end through the real CLI,
/// pinning that a SUBSTANTIAL README ALONE fires the signal (the first disjunct).
/// Mirrors real ripgrep (`/readme` size 21615, `contents/docs` → 404).
///
/// Given a public repo with a substantial README and no docs/ dir (its
/// documentation is thorough); When Maria scrapes the repo; Then the CLI exits 0
/// and the derived candidate list proposes the documentation-first philosophy,
/// naming its README evidence.
///
/// RED today: harvest never fetches `/readme` nor probes `contents/docs` → 0
/// documentation signals → the documentation-first candidate is absent
/// (MISSING_FUNCTIONALITY). GREEN once DELIVER lands `fetch_readme` +
/// `README_SUBSTANTIAL_BYTES` + the `contents/docs` probe + `detect_signals`'s
/// `DocsPresentAndSubstantial` arm.
///
/// @rgsd-4 @real-io @driving_port @happy
#[test]
fn scrape_repo_with_a_substantial_readme_derives_the_documentation_first_candidate() {
    // GIVEN an initialized env + a public repo whose `/readme` reports a large
    // `size` (20000 bytes — substantial) and whose `contents/docs` probe returns
    // 404 (NO docs dir). NO `language`, NO Cargo.lock, NO tags/CHANGELOG — so
    // documentation-first is the ONLY signal that can fire (isolating it from the
    // RGSD-1/2/3 detections).
    let env = TestEnv::initialized();
    let github = GithubServer::start(FakeGithub::for_public_repo_with_readme(
        "BurntSushi/ripgrep",
        20000,
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

    // ... AND the derived candidate list proposes the documentation-first
    // philosophy (design §5): detection fetches `/readme`, reads the `size`,
    // recognizes it as substantial, fires the `DocsPresentAndSubstantial` signal,
    // and the mapping derives this object.
    assert!(
        stdout.contains(DOCUMENTATION_FIRST_OBJECT),
        "a repo with a substantial README must derive the documentation-first \
         candidate ({DOCUMENTATION_FIRST_OBJECT}) from its `/readme` size evidence \
         (RGSD-4 design §5); \n--- stdout ---\n{stdout}\n--- stderr ---\n{}",
        outcome.stderr
    );

    // ... AND the proposal names its source — the README the harvest read
    // (design §3: the emitted signal is honest about what was measured). The
    // README source_url (the file html_url) surfaces so the user can audit WHY
    // the candidate was proposed (auditability, KPI-SCR-3). We pin only that the
    // `README` token surfaces, not the exact source-signal copy (DELIVER owns the
    // wording).
    assert!(
        stdout.contains("README"),
        "the documentation-first proposal must name its source — the substantial \
         \"README\" — so the user can audit why it was derived (design §3, \
         KPI-SCR-3); \n--- stdout ---\n{stdout}\n--- stderr ---\n{}",
        outcome.stderr
    );
}

/// RGSD-4 happy B — docs/ directory (RED today):
/// a real public repo with a `docs/` directory present (`contents/docs` → 200)
/// but a tiny/absent README (`/readme` → 404) — with NO synthetic `signals[]`,
/// NO `language`, NO Cargo.lock, NO semver tags / CHANGELOG — must, when
/// scraped, derive the `documentation-first` candidate. This pins the OR: a
/// `docs/` dir ALONE fires the signal even when the README is absent (the second
/// disjunct), so detection is a DISJUNCTION, not a README-only rule.
///
/// Given a public repo with a docs/ directory but no substantial README (it
/// documents in a docs/ tree); When Maria scrapes the repo; Then the CLI exits 0
/// and the documentation-first philosophy is proposed.
///
/// RED today: harvest never fetches `/readme` nor probes `contents/docs` → 0
/// documentation signals → the documentation-first candidate is absent
/// (MISSING_FUNCTIONALITY). GREEN once DELIVER lands the `contents/docs` probe +
/// the `DocsPresentAndSubstantial` arm.
///
/// @rgsd-4 @real-io @driving_port @happy
#[test]
fn scrape_repo_with_a_docs_directory_derives_the_documentation_first_candidate() {
    // GIVEN an initialized env + a public repo with a `docs/` dir present
    // (`contents/docs` → 200) but a tiny/absent README (`/readme` → 404). NO
    // `language`, NO Cargo.lock, NO tags/CHANGELOG — so documentation-first is
    // the ONLY signal that can fire, and the docs-dir disjunct is the thing under
    // test.
    let env = TestEnv::initialized();
    let github = GithubServer::start(FakeGithub::for_public_repo_with_docs_dir(
        "some-org/documented",
    ));

    // WHEN Maria scrapes the public repo (no --sign — a pure read).
    let outcome = run_openlore_scrape(
        &env,
        &["scrape", "github", "some-org/documented"],
        github.base_url(),
    );

    // THEN the scrape exits 0 (a well-formed public repo is never an error) ...
    assert_eq!(
        outcome.status, 0,
        "scrape of a resolvable public repo must exit 0; \n--- stdout ---\n{}\n--- stderr ---\n{}",
        outcome.stdout, outcome.stderr
    );

    // ... AND the documentation-first candidate IS proposed: detection is a
    // DISJUNCTION, so a `docs/` dir ALONE (without a substantial README) must
    // fire the signal (design §5 — the docs-dir disjunct).
    let stdout = &outcome.stdout;
    assert!(
        stdout.contains(DOCUMENTATION_FIRST_OBJECT),
        "a repo with a docs/ directory must derive the documentation-first \
         candidate ({DOCUMENTATION_FIRST_OBJECT}) from its `contents/docs` \
         evidence — detection is the DISJUNCTION of a substantial README OR a \
         docs/ dir, so docs/ alone fires (design §5); \n--- stdout ---\n{stdout}\n\
         --- stderr ---\n{}",
        outcome.stderr
    );
}

/// RGSD-4 negative (edge — GREEN-today guardrail):
/// a real public repo whose `/readme` reports a TINY `size` (e.g. 20 bytes, well
/// below the substantiality threshold) AND whose `contents/docs` probe returns
/// 404 (NO docs dir) must NOT derive the documentation-first candidate. Neither
/// disjunct holds: the README is not substantial AND there is no docs dir
/// (design §5). This mirrors the real octocat/Hello-World case (`/readme` size
/// 13, `contents/docs` → 404).
///
/// Given a public repo with a tiny README and no docs/ dir; When Maria scrapes
/// the repo; Then the CLI exits 0 and no documentation-first philosophy is
/// proposed.
///
/// GREEN today: no candidate is produced regardless (harvest never fetches
/// `/readme` nor probes `contents/docs`). This becomes the load-bearing
/// disjunction-guard once detection exists — it must stay GREEN when the happy
/// scenarios turn GREEN (a tiny README with no docs dir MUST NOT fire the
/// signal: below-threshold README alone does not count).
///
/// @rgsd-4 @real-io @driving_port @edge @guardrail
#[test]
fn scrape_repo_with_a_tiny_readme_and_no_docs_proposes_no_documentation_first_candidate() {
    // GIVEN an initialized env + a public repo whose `/readme` reports a tiny
    // `size` (20 bytes — below the substantiality threshold) AND whose
    // `contents/docs` probe returns 404 (NO docs dir). NO `language`, NO
    // Cargo.lock, NO tags/CHANGELOG — so RGSD-1/2/3 stay quiet and the
    // documentation-first ABSENCE is the thing under test.
    let env = TestEnv::initialized();
    let github = GithubServer::start(FakeGithub::for_public_repo_with_docs_evidence(
        "octocat/Hello-World",
        Some(20), // tiny README, below the substantiality threshold
        false,    // no docs/ dir
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

    // ... AND NO documentation-first candidate is proposed: neither disjunct
    // holds, so a tiny (below-threshold) README with no docs/ dir must NEVER fire
    // the signal (design §5 — the under-firing / disjunction guard).
    let stdout = &outcome.stdout;
    assert!(
        !stdout.contains(DOCUMENTATION_FIRST_OBJECT),
        "a repo with a tiny README and no docs/ dir must NOT derive the \
         documentation-first candidate ({DOCUMENTATION_FIRST_OBJECT}) — neither \
         disjunct holds (README not substantial AND no docs dir), so the signal \
         must never fire (design §5); \n--- stdout ---\n{stdout}\n--- stderr \
         ---\n{}",
        outcome.stderr
    );
}
