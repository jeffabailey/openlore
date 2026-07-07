//! RGSD-3 acceptance — real-GitHub signal detection (semver + CHANGELOG).
//!
//! Feature `real-github-signal-detection`, slice RGSD-3 (design §5). The
//! GitHub scraper must detect its `SignalKind::SemverAndChangelog` signal from
//! the CONJUNCTION of (a) the repo's tags following semver AND (b) a committed
//! CHANGELOG — `GET /repos/{owner}/{repo}/tags` carrying a semver-style tag
//! name PLUS `GET /repos/{o}/{r}/contents/CHANGELOG.md` → 200 — rather than the
//! synthetic `signals[]` array only the legacy `FakeGithub` postures inject
//! (design §1/§4). This adds exactly ONE new endpoint (`/tags`) to the harvest
//! and reuses RGSD-2's `contents/*` probe for the CHANGELOG.
//!
//! SPIKE-verified against real GitHub: ripgrep's `tags` include semver-style
//! names (`13.0.0`, `wincolor-0.1.6`) AND `contents/CHANGELOG.md` → 200 (a
//! clean positive); torvalds/linux has semver-ish tags but
//! `contents/CHANGELOG.md` → 404 (a clean negative — the conjunction must NOT
//! fire on semver tags alone).
//!
//! These are layer-3 subprocess acceptance scenarios (example-only per
//! Mandate 11) driven through the real `openlore` binary via the
//! `OPENLORE_GITHUB_API_BASE` seam. They assert ONLY the observable CLI
//! surface (exit code + stdout) — no scraper-domain / adapter-github struct
//! field is named (Mandate 8 universe = port-exposed CLI output).
//!
//! RED cause (design §1/§2): `adapter-github::harvest_repo` today fetches only
//! `GET /repos/{o}/{r}` (RGSD-1 language) and probes `contents/Cargo.lock`
//! (RGSD-2). It NEVER lists `/tags` and NEVER checks a CHANGELOG, so no
//! `SemverAndChangelog` signal is produced → no
//! `org.openlore.philosophy.semantic-versioning` candidate. The `list_tags`
//! effect, the pure `is_semver_tag` predicate, the `RepoFacts.{semver_tag,
//! changelog_url}` fields, and the `SemverAndChangelog` arm of `detect_signals`
//! do not exist yet; those pure/effect functions are DELIVER's RED_UNIT
//! (design §8). So:
//!
//!   * the happy scenario is RED today — the `semantic-versioning` candidate is
//!     absent (MISSING_FUNCTIONALITY), and turns GREEN once DELIVER lands the
//!     `/tags` list + CHANGELOG probe + the `SemverAndChangelog` arm;
//!   * both negative scenarios are GREEN-today guardrails — no candidate is
//!     produced regardless today; each becomes load-bearing once detection
//!     exists, pinning that detection is a CONJUNCTION (semver tags alone must
//!     NOT fire; a CHANGELOG alone must NOT fire).
//!
//! Covers:
//! - RGSD-3 (design §5): detect SemverAndChangelog from semver tags + a
//!   committed CHANGELOG; a repo with both yields the semantic-versioning
//!   candidate end-to-end, a repo missing EITHER half does not.

mod support;

#[allow(unused_imports)]
use support::*;

/// The candidate object the `SemverAndChangelog` signal maps to
/// (`signal_predicate_mapping.yaml` — mapping SSOT, unchanged by this slice).
const SEMANTIC_VERSIONING_OBJECT: &str = "org.openlore.philosophy.semantic-versioning";

// =============================================================================
// RGSD-3 — semver-tags + CHANGELOG-derived SemverAndChangelog detection (§5)
// =============================================================================

/// RGSD-3 happy (RED today):
/// a real public repo whose `/tags` include a clearly-semver tag (e.g.
/// `v1.2.3`) AND whose `contents/CHANGELOG.md` probe returns 200 (a committed
/// CHANGELOG) — with NO synthetic `signals[]`, NO memory-safe `language`, and
/// NO committed `Cargo.lock` (so ONLY semver-and-changelog can fire) — must,
/// when scraped, derive the `org.openlore.philosophy.semantic-versioning`
/// candidate and name its semver/CHANGELOG source. This closes the RGSD-3 loop
/// end-to-end through the real CLI (design §1/§5).
///
/// Given a public repo that follows semver in its tags AND commits a CHANGELOG
/// (its release process is disciplined); When Maria scrapes the repo; Then the
/// CLI exits 0 and the derived candidate list proposes the semantic-versioning
/// philosophy, naming its semver/CHANGELOG evidence.
///
/// RED today: harvest never lists `/tags` nor checks a CHANGELOG → 0
/// semver-and-changelog signals → the semantic-versioning candidate is absent
/// (MISSING_FUNCTIONALITY). GREEN once DELIVER lands `list_tags` +
/// `is_semver_tag` + the CHANGELOG probe + `detect_signals`'s
/// `SemverAndChangelog` arm.
///
/// @rgsd-3 @real-io @driving_port @happy
#[test]
fn scrape_repo_with_semver_tags_and_a_changelog_derives_the_semantic_versioning_candidate() {
    // GIVEN an initialized env + a public repo whose `/tags` list a semver tag
    // (`v1.2.3`) AND whose `contents/CHANGELOG.md` probe returns 200 (a
    // committed CHANGELOG), NO synthetic `signals[]`, NO `language`, NO
    // Cargo.lock — so semver-and-changelog is the ONLY signal that can fire
    // (isolating it from the RGSD-1 memory-safety + RGSD-2 dependency-pinning
    // detections).
    let env = TestEnv::initialized();
    let github = GithubServer::start(FakeGithub::for_public_repo_with_semver_and_changelog(
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

    // ... AND the derived candidate list proposes the semantic-versioning
    // philosophy (design §5): detection lists `/tags`, recognizes a semver tag,
    // probes `contents/CHANGELOG.md`, fires the `SemverAndChangelog` signal on
    // the CONJUNCTION, and the mapping derives this object.
    assert!(
        stdout.contains(SEMANTIC_VERSIONING_OBJECT),
        "a repo that follows semver in its tags AND commits a CHANGELOG must \
         derive the semantic-versioning candidate ({SEMANTIC_VERSIONING_OBJECT}) \
         from its `/tags` + `contents/CHANGELOG.md` evidence (RGSD-3 design §5); \
         \n--- stdout ---\n{stdout}\n--- stderr ---\n{}",
        outcome.stderr
    );

    // ... AND the proposal names its source — the CHANGELOG the harvest read
    // (design §3: the emitted signal is honest about what was measured). The
    // CHANGELOG source_url (the file html_url) surfaces so the user can audit
    // WHY the candidate was proposed (auditability, KPI-SCR-3). We pin only
    // that the `CHANGELOG` token surfaces, not the exact source-signal copy
    // (DELIVER owns the wording).
    assert!(
        stdout.contains("CHANGELOG"),
        "the semantic-versioning proposal must name its source — the committed \
         \"CHANGELOG\" — so the user can audit why it was derived (design §3, \
         KPI-SCR-3); \n--- stdout ---\n{stdout}\n--- stderr ---\n{}",
        outcome.stderr
    );
}

/// RGSD-3 negative A (edge — GREEN-today guardrail):
/// a real public repo whose `/tags` follow semver (`v1.2.3`) BUT whose
/// `contents/CHANGELOG.md` probe returns 404 (NO committed CHANGELOG) must NOT
/// derive the semantic-versioning candidate. Detection is a CONJUNCTION: the
/// signal fires only when BOTH halves hold (design §2), never on semver tags
/// alone. This mirrors the real torvalds/linux case (semver-ish tags, no
/// CHANGELOG at the contents root).
///
/// Given a public repo with semver tags but NO CHANGELOG; When Maria scrapes
/// the repo; Then the CLI exits 0 and no semantic-versioning philosophy is
/// proposed.
///
/// GREEN today: no candidate is produced regardless (harvest never lists
/// `/tags` nor checks a CHANGELOG). This becomes the load-bearing
/// conjunction-guard once detection exists — it must stay GREEN when the happy
/// scenario turns GREEN (semver tags alone MUST NOT fire the signal).
///
/// @rgsd-3 @real-io @driving_port @edge @guardrail
#[test]
fn scrape_repo_with_semver_tags_but_no_changelog_proposes_no_semantic_versioning_candidate() {
    // GIVEN an initialized env + a public repo whose `/tags` follow semver
    // (`v1.2.3`) but whose `contents/CHANGELOG.md` probe returns 404 (NO
    // committed CHANGELOG — `has_changelog == false`). NO `language`, NO
    // Cargo.lock — so RGSD-1/RGSD-2 stay quiet and the semantic-versioning
    // ABSENCE is the thing under test.
    let env = TestEnv::initialized();
    let github = GithubServer::start(FakeGithub::for_public_repo_with_tags_and_changelog(
        "torvalds/linux",
        vec!["v6.9", "v6.8.1", "v1.0.0"],
        false, // no CHANGELOG at the contents root
    ));

    // WHEN Maria scrapes the public repo (no --sign — a pure read).
    let outcome = run_openlore_scrape(
        &env,
        &["scrape", "github", "torvalds/linux"],
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

    // ... AND NO semantic-versioning candidate is proposed: detection is a
    // CONJUNCTION, so semver tags WITHOUT a CHANGELOG must NEVER fire the
    // signal (design §2 — the under-firing / conjunction guard).
    let stdout = &outcome.stdout;
    assert!(
        !stdout.contains(SEMANTIC_VERSIONING_OBJECT),
        "a repo with semver tags but no committed CHANGELOG must NOT derive the \
         semantic-versioning candidate ({SEMANTIC_VERSIONING_OBJECT}) — detection \
         is the CONJUNCTION of semver tags AND a CHANGELOG, never tags alone \
         (design §2); \n--- stdout ---\n{stdout}\n--- stderr ---\n{}",
        outcome.stderr
    );
}

/// RGSD-3 negative B (edge — GREEN-today guardrail):
/// a real public repo that commits a CHANGELOG (`contents/CHANGELOG.md` → 200)
/// BUT whose `/tags` are ALL non-semver (`nightly`, `latest`) must NOT derive
/// the semantic-versioning candidate. Detection is a CONJUNCTION: a CHANGELOG
/// alone (without semver-shaped tags) must NEVER fire the signal (design §2).
///
/// Given a public repo with a CHANGELOG but only non-semver tags; When Maria
/// scrapes the repo; Then the CLI exits 0 and no semantic-versioning philosophy
/// is proposed.
///
/// GREEN today: no candidate is produced regardless (harvest never lists
/// `/tags` nor checks a CHANGELOG). This becomes the load-bearing
/// conjunction-guard once detection exists — it must stay GREEN when the happy
/// scenario turns GREEN (a CHANGELOG with only non-semver tags MUST NOT fire).
///
/// @rgsd-3 @real-io @driving_port @edge @guardrail
#[test]
fn scrape_repo_with_a_changelog_but_only_non_semver_tags_proposes_no_semantic_versioning_candidate()
{
    // GIVEN an initialized env + a public repo whose `contents/CHANGELOG.md`
    // probe returns 200 (a committed CHANGELOG — `has_changelog == true`) but
    // whose `/tags` are ALL non-semver (`nightly`, `latest`). NO `language`, NO
    // Cargo.lock — so RGSD-1/RGSD-2 stay quiet and the semantic-versioning
    // ABSENCE is the thing under test.
    let env = TestEnv::initialized();
    let github = GithubServer::start(FakeGithub::for_public_repo_with_tags_and_changelog(
        "some-org/rolling-release",
        vec!["nightly", "latest"],
        true, // CHANGELOG present, but the tags are not semver
    ));

    // WHEN Maria scrapes the public repo (no --sign — a pure read).
    let outcome = run_openlore_scrape(
        &env,
        &["scrape", "github", "some-org/rolling-release"],
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

    // ... AND NO semantic-versioning candidate is proposed: detection is a
    // CONJUNCTION, so a CHANGELOG WITHOUT semver-shaped tags must NEVER fire
    // the signal (design §2 — the over-firing / conjunction guard).
    let stdout = &outcome.stdout;
    assert!(
        !stdout.contains(SEMANTIC_VERSIONING_OBJECT),
        "a repo that commits a CHANGELOG but tags only non-semver names must NOT \
         derive the semantic-versioning candidate ({SEMANTIC_VERSIONING_OBJECT}) \
         — detection is the CONJUNCTION of semver tags AND a CHANGELOG, never a \
         CHANGELOG alone (design §2); \n--- stdout ---\n{stdout}\n--- stderr \
         ---\n{}",
        outcome.stderr
    );
}
