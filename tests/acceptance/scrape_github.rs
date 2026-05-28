//! Slice-02 acceptance — `openlore scrape github <target>` harvest +
//! the scrape->propose->sign walking skeleton.
//!
//! The `scrape github` sugar verb (WD-50 + WD-60 / ADR-017): resolves a
//! PUBLIC GitHub target, prints the public-data-only banner BEFORE any
//! harvest, harvests the bounded public signal set via the new `GithubPort`
//! (`adapter-github`), then derives a candidate list via the PURE
//! `scraper-domain`. Running WITHOUT `--sign` persists NOTHING (the
//! human-gate at the storage layer — WD-49 / WD-55 / I-SCR-1).
//!
//! Layer placement (per nw-tdd-methodology Layered Test Discipline matrix +
//! DD-SCR-6): every test here is a layer-3 / layer-5 subprocess test —
//! example-only (Mandate 11), driven by `assert_cmd` against the real
//! `openlore` binary with a `FakeGithub` HTTP double for the external
//! GitHub boundary and the slice-01 `FakePds` / `FakeIdentity` for the sign
//! path. Sad paths are enumerated explicitly (Mandate 11), never
//! PBT-generated.
//!
//! Covers:
//! - US-SCR-001: harvest a public GitHub target's signals (happy + 4 sad)
//! - US-SCR-002: derive auditable candidate claims (happy render here;
//!   derivation specifics live in `scrape_candidates.rs`)
//! - WD-50 / WD-60: the `scrape github` sugar verb shape
//! - WD-51 / I-SCR-2: public-data-only (gate `scraper_only_reads_public_data`)
//! - WD-55 / I-SCR-1: nothing persisted unsigned (gate
//!   `scraper_never_persists_unsigned`)
//!
//! Walking skeleton: SG-1 is the slice-02 walking skeleton — the thinnest
//! end-to-end proof of scrape -> propose (the sign half is exercised by
//! `scrape_sign.rs` SS-1). It exercises the REAL CLI driving adapter +
//! REAL DuckDB (to prove zero rows are written) + REAL filesystem (to prove
//! zero claim artifacts) + the `FakeGithub` external boundary.
//
// SCAFFOLD: true

mod support;

#[allow(unused_imports)]
use support::*;

// =============================================================================
// US-SCR-001 / US-SCR-002 — walking skeleton: scrape -> propose (no persist)
// =============================================================================

/// SG-1 (WALKING SKELETON): `openlore scrape github rust-lang/cargo`
/// (no `--sign`) prints the public-data-only banner BEFORE any harvest,
/// resolves the public repo, reports the harvested signal count, renders a
/// numbered candidate list whose footer states nothing is a claim until the
/// user signs it — AND persists NOTHING: zero `claims` rows, zero PDS
/// writes, zero `claims/<cid>.json` files. This is the load-bearing
/// human-gate-at-the-storage-layer proof (gate `scraper_never_persists_unsigned`,
/// KPI-SCR-2) bundled with the harvest happy path.
///
/// Given the GitHub repo rust-lang/cargo is public (FakeGithub serves 5
/// public signals);
/// When Maria runs `openlore scrape github rust-lang/cargo`;
/// Then the public-data-only banner is printed before any harvest, the
/// harvested signal count is reported, a numbered candidate list with the
/// "nothing is a claim until you sign it" footer is rendered, AND zero
/// rows / zero PDS writes / zero claim files exist.
///
/// @us-scr-001 @us-scr-002 @walking_skeleton @driving_port @driving_adapter
/// @real-io @j-004 @j-004a @kpi-scr-2 @happy
#[test]
fn scrape_github_harvests_public_repo_proposes_candidates_and_persists_nothing() {
    // GIVEN an initialized env + a public repo serving 5 public signals.
    let env = TestEnv::initialized();
    let github = GithubServer::start(FakeGithub::for_public_repo(
        "rust-lang/cargo",
        fixture_cargo_five_signals(),
    ));

    // WHEN Maria scrapes the public repo (no --sign).
    let outcome = run_openlore_scrape(
        &env,
        &["scrape", "github", "rust-lang/cargo"],
        github.base_url(),
    );

    assert_eq!(
        outcome.status, 0,
        "scrape must exit 0 on the happy path; \n--- stdout ---\n{}\n--- stderr ---\n{}",
        outcome.stdout, outcome.stderr
    );

    // THEN the public-data-only banner is printed BEFORE any harvest line
    // (ordering, not just presence — the user is reassured before any network
    // beat begins).
    let banner_idx = outcome
        .stdout
        .find("PUBLIC")
        .expect("public-data-only banner must be present in stdout");
    let harvest_idx = outcome
        .stdout
        .find("Harvesting public signals")
        .expect("the harvest line must be present in stdout");
    assert!(
        banner_idx < harvest_idx,
        "the public-data-only banner must precede the harvest line; \n--- stdout ---\n{}",
        outcome.stdout
    );

    // AND the harvested signal count is reported (5 signals).
    assert!(
        outcome.stdout.contains("5 signals"),
        "the harvested signal count (5 signals) must be reported; \n--- stdout ---\n{}",
        outcome.stdout
    );

    // AND a numbered candidate list (1..5) is rendered (one per mapping entry).
    for index in 1..=5 {
        assert!(
            outcome.stdout.contains(&format!("[{index}]")),
            "expected a numbered candidate [{index}] in the list; \n--- stdout ---\n{}",
            outcome.stdout
        );
    }

    // AND the subject of every candidate is the resolved github_target.
    assert!(
        outcome.stdout.contains("github:rust-lang/cargo"),
        "the candidate list must name the resolved subject github:rust-lang/cargo; \
         \n--- stdout ---\n{}",
        outcome.stdout
    );

    // AND the "nothing is a claim until you sign it" footer is rendered.
    assert!(
        outcome.stdout.contains("nothing is a claim until you sign it"),
        "the candidate-list footer must reassure that nothing is a claim until signed; \
         \n--- stdout ---\n{}",
        outcome.stdout
    );

    // AND every candidate is the conservative speculative default (0.25).
    assert!(
        outcome.stdout.contains("0.25"),
        "every candidate must display the conservative default confidence 0.25; \
         \n--- stdout ---\n{}",
        outcome.stdout
    );

    // AND the human-gate held at the storage layer: zero `claims` rows, zero
    // PDS writes, zero claim artifact files (scraper_never_persists_unsigned).
    assert_no_claim_persisted(&env);
}

/// SG-2: the public-data-only banner appears BEFORE the first harvest line
/// (ordering, not just presence) — the user is reassured no private data is
/// read before any network beat begins (emotional arc: skeptical ->
/// reassured; WD-51).
///
/// Given a public repo target; When `scrape github <repo>`; Then the
/// banner's "only PUBLIC GitHub data is read ... Nothing published" text
/// precedes the "Harvesting public signals" line in stdout.
///
/// @us-scr-001 @real-io @driving_port @j-004a @happy
#[test]
fn scrape_github_prints_public_data_banner_before_any_harvest() {
    // SCAFFOLD: true
    todo!(
        "DELIVER (slice-02): SG-2. Assert the banner substring index < the harvest-line \
         substring index in stdout (ordering), AND the banner names BOTH 'only public data' \
         and 'nothing is published'."
    )
}

/// SG-3 (US-SCR-001 Ex 2; WD-64): a USER/contributor target resolves to a
/// User (not a Repo) and harvests a BOUNDED cross-repo aggregate, reporting
/// the signal count. (Deep cross-repo triangulation is deferred to slice-04;
/// slice-02 harvests a bounded aggregate.)
///
/// Given the GitHub user torvalds is public; When `scrape github torvalds`;
/// Then the target resolves as a user, a bounded aggregate signal count is
/// reported, and candidates render normally.
///
/// @us-scr-001 @real-io @driving_port @j-004a @wd-64 @edge
#[test]
fn scrape_github_resolves_user_target_and_harvests_bounded_aggregate() {
    // SCAFFOLD: true
    todo!(
        "DELIVER (slice-02): SG-3. GIVEN FakeGithub::for_public_user(\"torvalds\", \
         fixture_torvalds_user_aggregate_signals()); WHEN scrape github torvalds; THEN exit \
         0, stdout reports the user resolved as a USER (not a repo) and a bounded aggregate \
         signal count; candidates render normally."
    )
}

// =============================================================================
// US-SCR-001 — public-data-only + harvest sad paths (example-only; Mandate 11)
// =============================================================================

/// SG-4 / Sad (US-SCR-001 Ex 3): a non-existent target (HTTP 404) exits
/// non-zero, names the target + the not-found cause, and produces ZERO
/// candidates. Nothing persisted.
///
/// Given ghost-org/ghost-repo does not exist on GitHub; When `scrape github
/// ghost-org/ghost-repo`; Then exit non-zero, the error names the target and
/// the not-found cause, zero candidate claims are produced, nothing persisted.
///
/// @us-scr-001 @real-io @driving_port @j-004a @error
#[test]
fn scrape_github_rejects_nonexistent_target_with_zero_candidates() {
    // SCAFFOLD: true
    todo!(
        "DELIVER (slice-02): SG-4. GIVEN FakeGithub::for_not_found(\"ghost-org/ghost-repo\"); \
         WHEN scrape github ghost-org/ghost-repo; THEN exit non-zero, stderr names the target \
         AND the not-found cause, no numbered candidate list is rendered, \
         assert_no_claim_persisted(&env)."
    )
}

/// SG-5 / Sad (US-SCR-001 Ex 4; WD-51 / I-SCR-2 — gate
/// `scraper_only_reads_public_data`, KPI-SCR-4): a PRIVATE/inaccessible
/// target is refused with the "scraper only reads public data" message, and
/// NO private endpoint is ever called. This is the load-bearing
/// no-surveillance guardrail — release-blocking. Public-data-only is
/// structural: FakeGithub::for_private_target has no private surface to
/// serve, and the production code's request paths (asserted via
/// FakeGithub::seen_paths) MUST all be on the public allowlist.
///
/// Given acme-corp/secret-repo is a private repository; When `scrape github
/// acme-corp/secret-repo`; Then exit non-zero, the message states the
/// scraper only reads public data, NO private endpoint is called, zero
/// candidates, nothing persisted.
///
/// @us-scr-001 @real-io @driving_port @j-004a @kpi-scr-4 @error @release-gate
#[test]
fn scrape_github_refuses_private_target_and_calls_no_private_endpoint() {
    // SCAFFOLD: true
    todo!(
        "DELIVER (slice-02): SG-5 — scraper_only_reads_public_data gate (KPI-SCR-4 \
         release-blocking). GIVEN FakeGithub::for_private_target(\"acme-corp/secret-repo\"); \
         WHEN scrape github acme-corp/secret-repo; THEN exit non-zero, stderr contains \
         'scraper only reads public data', assert_only_public_endpoints_called(&github) \
         (every FakeGithub::seen_paths entry is on the public allowlist; no private path), \
         no candidates, assert_no_claim_persisted(&env)."
    )
}

/// SG-6 / Sad (US-SCR-001 offline UAT): with no network connectivity the
/// harvest (the ONLY network step) exits non-zero with a "scrape requires
/// network access" message and renders NO partial candidate list.
///
/// Given the machine has no network connectivity; When `scrape github
/// rust-lang/cargo`; Then exit non-zero, the message states scrape requires
/// network, no partial candidate list is rendered.
///
/// @us-scr-001 @real-io @driving_port @j-004a @error
#[test]
fn scrape_github_offline_exits_with_requires_network_and_no_partial_list() {
    // SCAFFOLD: true
    todo!(
        "DELIVER (slice-02): SG-6. GIVEN FakeGithub::offline() (base URL points at a dead \
         port); WHEN scrape github rust-lang/cargo; THEN exit non-zero, stderr contains \
         'requires network', stdout renders NO numbered candidate list, \
         assert_no_claim_persisted(&env)."
    )
}

/// SG-7 (US-SCR-002 Ex 2; gate-adjacent): a public target whose harvest
/// yields ZERO signals the mapping can use prints "No candidate claims
/// could be derived ..." and exits 0 (nothing to propose is NOT an error).
/// Zero rows are written to `author_claims`.
///
/// Given some-user/empty-experiment has no signals matching the mapping;
/// When the CLI finishes harvesting; Then it prints "No candidate claims
/// could be derived", exit code is 0, and zero rows are written.
///
/// @us-scr-002 @real-io @driving_port @j-004b @edge
#[test]
fn scrape_github_with_no_matching_signals_proposes_nothing_and_exits_zero() {
    // SCAFFOLD: true
    todo!(
        "DELIVER (slice-02): SG-7. GIVEN \
         FakeGithub::with_no_matching_signals(\"some-user/empty-experiment\"); WHEN scrape \
         github some-user/empty-experiment; THEN exit 0, stdout contains 'No candidate \
         claims could be derived', assert_no_claim_persisted(&env)."
    )
}

/// SG-8: running `scrape github <target>` makes ZERO PDS writes regardless
/// of how many candidates are derived — the human-gate is enforced at the
/// network-publish layer too, not only at the storage layer. (Reinforces
/// gate `scraper_never_persists_unsigned` from the PDS side; WD-55.)
///
/// Given a public repo with 5 signals; When `scrape github <repo>` without
/// `--sign`; Then exactly zero records exist on the user's own PDS and zero
/// publish calls were made.
///
/// @us-scr-001 @us-scr-002 @real-io @driving_port @j-004 @kpi-scr-2 @edge
#[test]
fn scrape_github_without_sign_makes_zero_pds_writes() {
    // SCAFFOLD: true
    todo!(
        "DELIVER (slice-02): SG-8. WHEN scrape github rust-lang/cargo (no --sign); THEN \
         assert_no_pds_call_was_made(&env) AND env.pds.records().is_empty() — the human-gate \
         holds at the publish layer."
    )
}

/// SG-9: a second identical `scrape github <target>` invocation is a pure
/// read — it derives the same candidate list and STILL persists nothing
/// (idempotent on the no-side-effect contract; a scrape is never a mutation
/// no matter how many times it runs).
///
/// Given a public repo; When `scrape github <repo>` runs twice; Then both
/// runs exit 0, render the same candidate count, and after both runs zero
/// rows / zero PDS writes / zero claim files exist.
///
/// @us-scr-001 @real-io @driving_port @j-004 @kpi-scr-2 @edge
#[test]
fn scrape_github_is_a_pure_read_persisting_nothing_across_repeated_runs() {
    // SCAFFOLD: true
    todo!(
        "DELIVER (slice-02): SG-9. WHEN scrape github rust-lang/cargo runs TWICE; THEN both \
         exit 0 with the same candidate count, and assert_no_claim_persisted(&env) holds \
         after both runs (a scrape never mutates state)."
    )
}
