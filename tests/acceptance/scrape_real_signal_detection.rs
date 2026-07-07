//! RGSD-1 acceptance — real-GitHub signal detection (walking skeleton).
//!
//! Feature `real-github-signal-detection`, slice RGSD-1 (design §5). The
//! GitHub scraper must detect its `MemorySafetyLanguage` signal from the
//! REAL `/repos/{owner}/{repo}` `language` field — the metadata the live
//! GitHub API actually provides — rather than the synthetic `signals[]`
//! array only the legacy `FakeGithub` postures inject (design §1/§4).
//!
//! These are layer-3 subprocess acceptance scenarios (example-only per
//! Mandate 11) driven through the real `openlore` binary via the
//! `OPENLORE_GITHUB_API_BASE` seam. They assert ONLY the observable CLI
//! surface (exit code + stdout) — no scraper-domain struct field is named.
//!
//! RED cause (design §1/§2): `adapter-github::harvest_repo` today reads the
//! (absent) `signals[]` array from the realistic body, yielding ZERO signals
//! -> zero candidates. The language-based detection
//! (`parse_repo_facts` + `detect_signals`) does not exist yet; those pure
//! functions are DELIVER's RED_UNIT (design §8). So:
//!
//!   * the happy scenario is RED today — the `memory-safety` candidate is
//!     absent (MISSING_FUNCTIONALITY), and turns GREEN once DELIVER lands the
//!     language arm of `detect_signals`;
//!   * the negative scenario is a GREEN-today guardrail — no candidate is
//!     produced regardless today; it becomes load-bearing once detection
//!     exists, pinning that detection is LANGUAGE-gated (a non-memory-safe
//!     language must NOT fire the signal).
//!
//! Covers:
//! - RGSD-1 (design §5 walking skeleton): detect MemorySafetyLanguage from
//!   the real `language` field; a Rust repo yields the memory-safety
//!   candidate end-to-end, a C++ repo does not.

mod support;

#[allow(unused_imports)]
use support::*;

/// The candidate object the `MemorySafetyLanguage` signal maps to
/// (`signal_predicate_mapping.yaml` — mapping SSOT, unchanged by this slice).
const MEMORY_SAFETY_OBJECT: &str = "org.openlore.philosophy.memory-safety";

// =============================================================================
// RGSD-1 — language-derived MemorySafetyLanguage detection (design §5)
// =============================================================================

/// RGSD-1 happy (WALKING SKELETON — RED today):
/// a real public repo whose live `/repos` body reports `language: "Rust"`
/// (a memory-safety language) — with NO synthetic `signals[]` — must, when
/// scraped, derive the `org.openlore.philosophy.memory-safety` candidate and
/// name the language as its source. This closes the RGSD-1 loop end-to-end
/// through the real CLI (design §1/§5).
///
/// Given a public repo whose real repository metadata reports its primary
/// language is Rust; When Maria scrapes the repo; Then the CLI exits 0 and
/// the derived candidate list proposes the memory-safety philosophy, naming
/// the primary language as the source of that proposal.
///
/// RED today: harvest reads the absent `signals[]` -> 0 signals -> the
/// memory-safety candidate is absent (MISSING_FUNCTIONALITY). GREEN once
/// DELIVER lands `detect_signals`'s language arm.
///
/// @rgsd-1 @walking_skeleton @real-io @driving_port @happy
#[test]
fn scrape_repo_whose_language_is_rust_derives_the_memory_safety_candidate() {
    // GIVEN an initialized env + a public repo serving a REALISTIC `/repos`
    // body: `language: "Rust"`, an `html_url`, and NO synthetic `signals[]`
    // (the shape the live GitHub API returns — design §5 walking skeleton).
    let env = TestEnv::initialized();
    let github = GithubServer::start(FakeGithub::for_public_repo_with_language(
        "rust-lang/cargo",
        "Rust",
    ));

    // WHEN Maria scrapes the public repo (no --sign — this is a pure read).
    let outcome = run_openlore_scrape(
        &env,
        &["scrape", "github", "rust-lang/cargo"],
        github.base_url(),
    );

    // THEN the scrape exits 0 (a well-formed public repo is never an error) ...
    assert_eq!(
        outcome.status, 0,
        "scrape of a resolvable public repo must exit 0; \n--- stdout ---\n{}\n--- stderr ---\n{}",
        outcome.stdout, outcome.stderr
    );

    let stdout = &outcome.stdout;

    // ... AND the derived candidate list proposes the memory-safety philosophy
    // (design §5): detection reads the real `language` field, fires the
    // `MemorySafetyLanguage` signal, and the mapping derives this object.
    assert!(
        stdout.contains(MEMORY_SAFETY_OBJECT),
        "a repo whose primary language is Rust must derive the memory-safety \
         candidate ({MEMORY_SAFETY_OBJECT}) from its real `language` field \
         (RGSD-1 design §5); \n--- stdout ---\n{stdout}\n--- stderr ---\n{}",
        outcome.stderr
    );

    // ... AND the proposal names the PRIMARY LANGUAGE as its source (design §3:
    // the emitted signal is honest about what was measured — the language, not
    // "no unsafe blocks"). The user must be able to see WHY the candidate was
    // proposed (auditability, KPI-SCR-3). We pin only that the language string
    // surfaces, not the exact source-signal copy (DELIVER owns the wording).
    assert!(
        stdout.contains("Rust"),
        "the memory-safety proposal must name its source — the primary language \
         \"Rust\" — so the user can audit why it was derived (design §3, \
         KPI-SCR-3); \n--- stdout ---\n{stdout}\n--- stderr ---\n{}",
        outcome.stderr
    );
}

/// RGSD-1 negative (edge — GREEN-today guardrail):
/// a real public repo whose live `/repos` body reports `language: "C++"` (a
/// NON-memory-safe language) — with NO synthetic `signals[]` — must NOT
/// derive the memory-safety candidate. Detection is LANGUAGE-gated: the
/// signal fires only for languages in the curated memory-safe set (design
/// §2 `MEMORY_SAFE_LANGUAGES`, which EXCLUDES C/C++), never unconditionally.
///
/// Given a public repo whose real repository metadata reports its primary
/// language is C++; When Maria scrapes the repo; Then the CLI exits 0 and no
/// memory-safety philosophy is proposed.
///
/// GREEN today: no candidate is produced regardless (harvest reads the absent
/// `signals[]`). This becomes the load-bearing over-firing guard once
/// detection exists — it must stay GREEN when the happy scenario turns GREEN.
///
/// @rgsd-1 @real-io @driving_port @edge @guardrail
#[test]
fn scrape_repo_whose_language_is_cpp_proposes_no_memory_safety_candidate() {
    // GIVEN an initialized env + a public repo whose REALISTIC `/repos` body
    // reports a NON-memory-safe primary language (`C++`), NO synthetic
    // `signals[]`. C++ is deliberately EXCLUDED from `MEMORY_SAFE_LANGUAGES`
    // (design §2).
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

    // ... AND NO memory-safety candidate is proposed: detection is
    // language-gated, so a non-memory-safe language must NEVER fire the signal
    // (design §2 — the over-firing guard).
    let stdout = &outcome.stdout;
    assert!(
        !stdout.contains(MEMORY_SAFETY_OBJECT),
        "a repo whose primary language is C++ must NOT derive the memory-safety \
         candidate ({MEMORY_SAFETY_OBJECT}) — detection is language-gated, never \
         unconditional (design §2); \n--- stdout ---\n{stdout}\n--- stderr ---\n{}",
        outcome.stderr
    );
}
