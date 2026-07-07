//! RGSD-2 acceptance — real-GitHub signal detection (dependency-pinning).
//!
//! Feature `real-github-signal-detection`, slice RGSD-2 (design §5). The
//! GitHub scraper must detect its `DependencyManifestPinned` signal from a
//! committed `Cargo.lock` — the presence of `GET /repos/{owner}/{repo}/
//! contents/Cargo.lock` (200 = present, SPIKE-verified against real GitHub:
//! ripgrep → 200, torvalds/linux → 404) — rather than the synthetic
//! `signals[]` array only the legacy `FakeGithub` postures inject (design
//! §1/§4). This adds exactly ONE endpoint probe to the RGSD-1 harvest.
//!
//! These are layer-3 subprocess acceptance scenarios (example-only per
//! Mandate 11) driven through the real `openlore` binary via the
//! `OPENLORE_GITHUB_API_BASE` seam. They assert ONLY the observable CLI
//! surface (exit code + stdout) — no scraper-domain / adapter-github struct
//! field is named (Mandate 8 universe = port-exposed CLI output).
//!
//! RED cause (design §1/§2): `adapter-github::harvest_repo` today fetches only
//! `GET /repos/{o}/{r}` and detects the RGSD-1 language signal — it NEVER
//! probes `contents/Cargo.lock`, so no `DependencyManifestPinned` signal is
//! produced → no `org.openlore.philosophy.dependency-pinning` candidate. The
//! `content_exists` effect, the `RepoFacts.cargo_lock_url` field, and the
//! `DependencyManifestPinned` arm of `detect_signals` do not exist yet; those
//! pure/effect functions are DELIVER's RED_UNIT (design §8). So:
//!
//!   * the happy scenario is RED today — the `dependency-pinning` candidate is
//!     absent (MISSING_FUNCTIONALITY), and turns GREEN once DELIVER lands the
//!     Cargo.lock probe + the `DependencyManifestPinned` arm;
//!   * the negative scenario is a GREEN-today guardrail — no candidate is
//!     produced regardless today; it becomes load-bearing once detection
//!     exists, pinning that detection is CARGO-LOCK-gated (a repo without a
//!     committed Cargo.lock must NOT fire the signal).
//!
//! Covers:
//! - RGSD-2 (design §5): detect DependencyManifestPinned from a committed
//!   Cargo.lock; a repo whose `contents/Cargo.lock` → 200 yields the
//!   dependency-pinning candidate end-to-end, a repo whose probe → 404 does not.

mod support;

#[allow(unused_imports)]
use support::*;

/// The candidate object the `DependencyManifestPinned` signal maps to
/// (`signal_predicate_mapping.yaml` — mapping SSOT, unchanged by this slice).
const DEPENDENCY_PINNING_OBJECT: &str = "org.openlore.philosophy.dependency-pinning";

// =============================================================================
// RGSD-2 — Cargo.lock-derived DependencyManifestPinned detection (design §5)
// =============================================================================

/// RGSD-2 happy (RED today):
/// a real public repo whose `contents/Cargo.lock` probe returns 200 (a
/// committed Cargo.lock) — with NO synthetic `signals[]` and NO memory-safe
/// `language` (so ONLY dependency-pinning can fire) — must, when scraped,
/// derive the `org.openlore.philosophy.dependency-pinning` candidate and name
/// its Cargo.lock source. This closes the RGSD-2 loop end-to-end through the
/// real CLI (design §1/§5).
///
/// Given a public repo that commits a Cargo.lock (its dependency manifest is
/// pinned); When Maria scrapes the repo; Then the CLI exits 0 and the derived
/// candidate list proposes the dependency-pinning philosophy, naming the
/// committed Cargo.lock as the source of that proposal.
///
/// RED today: harvest never probes `contents/Cargo.lock` → 0 dependency
/// signals → the dependency-pinning candidate is absent
/// (MISSING_FUNCTIONALITY). GREEN once DELIVER lands `content_exists` +
/// `detect_signals`'s `DependencyManifestPinned` arm.
///
/// @rgsd-2 @real-io @driving_port @happy
#[test]
fn scrape_repo_with_a_committed_cargo_lock_derives_the_dependency_pinning_candidate() {
    // GIVEN an initialized env + a public repo whose `contents/Cargo.lock`
    // probe returns 200 (a committed Cargo.lock), NO synthetic `signals[]`, NO
    // `language` — so dependency-pinning is the ONLY signal that can fire
    // (isolating it from the RGSD-1 memory-safety detection).
    let env = TestEnv::initialized();
    let github = GithubServer::start(FakeGithub::for_public_repo_with_cargo_lock(
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

    // ... AND the derived candidate list proposes the dependency-pinning
    // philosophy (design §5): detection probes `contents/Cargo.lock`, fires the
    // `DependencyManifestPinned` signal, and the mapping derives this object.
    assert!(
        stdout.contains(DEPENDENCY_PINNING_OBJECT),
        "a repo that commits a Cargo.lock must derive the dependency-pinning \
         candidate ({DEPENDENCY_PINNING_OBJECT}) from its `contents/Cargo.lock` \
         probe (RGSD-2 design §5); \n--- stdout ---\n{stdout}\n--- stderr ---\n{}",
        outcome.stderr
    );

    // ... AND the proposal names the committed Cargo.lock as its source (design
    // §3: the emitted signal is honest about what was measured). The Cargo.lock
    // source_url (the file html_url the harvest reads) surfaces so the user can
    // audit WHY the candidate was proposed (auditability, KPI-SCR-3). We pin
    // only that the `Cargo.lock` token surfaces, not the exact source-signal
    // copy (DELIVER owns the wording).
    assert!(
        stdout.contains("Cargo.lock"),
        "the dependency-pinning proposal must name its source — the committed \
         \"Cargo.lock\" — so the user can audit why it was derived (design §3, \
         KPI-SCR-3); \n--- stdout ---\n{stdout}\n--- stderr ---\n{}",
        outcome.stderr
    );
}

/// RGSD-2 negative (edge — GREEN-today guardrail):
/// a real public repo whose `contents/Cargo.lock` probe returns 404 (no
/// committed Cargo.lock) must NOT derive the dependency-pinning candidate.
/// Detection is CARGO-LOCK-gated: the signal fires only when the manifest is
/// actually pinned (design §2), never unconditionally.
///
/// Given a public repo that commits NO Cargo.lock; When Maria scrapes the
/// repo; Then the CLI exits 0 and no dependency-pinning philosophy is proposed.
///
/// GREEN today: no candidate is produced regardless (harvest never probes
/// `contents/Cargo.lock`). This becomes the load-bearing over-firing guard
/// once detection exists — it must stay GREEN when the happy scenario turns
/// GREEN.
///
/// This posture also carries a non-memory-safe `language` ("C++"), so the
/// RGSD-1 memory-safety detection does not fire either — isolating the
/// dependency-pinning absence as the thing under test.
///
/// @rgsd-2 @real-io @driving_port @edge @guardrail
#[test]
fn scrape_repo_without_a_cargo_lock_proposes_no_dependency_pinning_candidate() {
    // GIVEN an initialized env + a public repo whose `contents/Cargo.lock`
    // probe returns 404 (NO committed Cargo.lock). The `for_public_repo_with_
    // language` posture keeps `has_cargo_lock == false`, so the new probe 404s;
    // its `C++` language is deliberately non-memory-safe so RGSD-1 stays quiet.
    let env = TestEnv::initialized();
    let github = GithubServer::start(FakeGithub::for_public_repo_with_language(
        "some-org/cpp-project",
        "C++",
    ));

    // WHEN Maria scrapes the public repo (no --sign — a pure read).
    let outcome = run_openlore_scrape(
        &env,
        &["scrape", "github", "some-org/cpp-project"],
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

    // ... AND NO dependency-pinning candidate is proposed: detection is
    // Cargo.lock-gated, so a repo without a committed Cargo.lock must NEVER fire
    // the signal (design §2 — the over-firing guard).
    let stdout = &outcome.stdout;
    assert!(
        !stdout.contains(DEPENDENCY_PINNING_OBJECT),
        "a repo without a committed Cargo.lock must NOT derive the \
         dependency-pinning candidate ({DEPENDENCY_PINNING_OBJECT}) — detection \
         is Cargo.lock-gated, never unconditional (design §2); \n--- stdout \
         ---\n{stdout}\n--- stderr ---\n{}",
        outcome.stderr
    );
}
