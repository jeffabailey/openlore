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
    let github = GithubServer::start(FakeGithub::for_public_repo_with_all_signals("rust-lang/cargo"));

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
        outcome
            .stdout
            .contains("nothing is a claim until you sign it"),
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
    // GIVEN an initialized env + a public repo serving public signals.
    let env = TestEnv::initialized();
    let github = GithubServer::start(FakeGithub::for_public_repo_with_all_signals("rust-lang/cargo"));

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

    // THEN the banner names BOTH the "only public data" affordance AND the
    // "nothing is published" affordance — the two halves of the WD-51
    // reassurance contract (skeptical -> reassured emotional arc).
    let only_public_idx = outcome
        .stdout
        .find("PUBLIC GitHub data is read")
        .expect("the banner must state that ONLY public GitHub data is read");
    assert!(
        outcome.stdout.contains("Nothing published"),
        "the banner must also reassure that nothing is published; \n--- stdout ---\n{}",
        outcome.stdout
    );

    // AND the banner precedes the first harvest line (ordering, not just
    // presence — the user is reassured BEFORE any network beat begins).
    let harvest_idx = outcome
        .stdout
        .find("Harvesting public signals")
        .expect("the harvest line must be present in stdout");
    assert!(
        only_public_idx < harvest_idx,
        "the public-data-only banner must precede the harvest line; \n--- stdout ---\n{}",
        outcome.stdout
    );
}

/// SG-3 (US-SCR-001 Ex 2; WD-64): a USER/contributor target resolves to a
/// User (not a Repo) and harvests cleanly, but DERIVES NO candidates — the
/// bounded cross-repo USER aggregate is DEFERRED to slice-04 (WD-64). A real
/// user scrape today reads zero repo-level signals, so the honest slice-02
/// outcome is "resolves as a user, harvests, proposes nothing" — never a
/// synthetic aggregate. This pins the deferral as the observed behavior.
///
/// Given the GitHub user torvalds is public; When `scrape github torvalds`;
/// Then the target resolves as a user, the harvest completes, and NO candidate
/// claims are derived (user aggregation deferred to slice-04) — exit 0.
///
/// @us-scr-001 @real-io @driving_port @j-004a @wd-64 @edge
#[test]
fn scrape_github_resolves_user_target_and_derives_no_candidates_aggregation_deferred() {
    // GIVEN an initialized env + a PUBLIC USER target. The USER-aggregate harvest
    // is deferred to slice-04 (WD-64), so a real user scrape yields ZERO signals
    // today — no synthetic aggregate is injected.
    let env = TestEnv::initialized();
    let github = GithubServer::start(FakeGithub::for_public_user("torvalds"));

    // WHEN Maria scrapes the bare-user target (no --sign).
    let outcome = run_openlore_scrape(&env, &["scrape", "github", "torvalds"], github.base_url());

    // THEN the run exits ZERO — a user target that resolves but yields no usable
    // signals is a clean no-op, NOT an error (contrast SG-4's non-zero 404).
    assert_eq!(
        outcome.status, 0,
        "scrape of a public user must exit 0; \n--- stdout ---\n{}\n--- stderr ---\n{}",
        outcome.stdout, outcome.stderr
    );

    // AND the public-data-only banner precedes the first harvest line
    // (the user is reassured BEFORE any network beat begins).
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

    // AND the target resolves as a USER (not a repo) — `torvalds` has no
    // `owner/repo` slash, so it is disambiguated to TargetKind::User and the
    // resolution line names it "(user)".
    assert!(
        outcome
            .stdout
            .contains("Resolving target torvalds ... ok (user)"),
        "a bare-user target must resolve as a USER (not a repo); \n--- stdout ---\n{}",
        outcome.stdout
    );

    // AND the no-candidates message is printed (US-SCR-002 Ex 2 shape): the
    // USER-aggregate derivation is deferred to slice-04, so nothing is proposed.
    assert!(
        outcome
            .stdout
            .contains("No candidate claims could be derived"),
        "a user scrape must state that no candidate claims could be derived \
         (aggregation deferred to slice-04); \n--- stdout ---\n{}",
        outcome.stdout
    );

    // AND ZERO candidates are rendered — no numbered list and no footer.
    assert!(
        !outcome.stdout.contains("[1]"),
        "a deferred user aggregate must render NO numbered candidate list; \n--- stdout ---\n{}",
        outcome.stdout
    );
    assert!(
        !outcome
            .stdout
            .contains("nothing is a claim until you sign it"),
        "a deferred user aggregate must render NO candidate-list footer; \n--- stdout ---\n{}",
        outcome.stdout
    );

    // AND the human-gate held at the storage layer: zero `claims` rows, zero
    // PDS writes, zero claim artifact files (scraper_never_persists_unsigned).
    assert_no_claim_persisted(&env);
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
    // GIVEN an initialized env + a target that does not exist on GitHub
    // (FakeGithub::for_not_found serves a 404; the adapter classifies that as
    // GithubError::NotFound, 03-01).
    let env = TestEnv::initialized();
    let github = GithubServer::start(FakeGithub::for_not_found("ghost-org/ghost-repo"));

    // WHEN Maria scrapes the non-existent target (no --sign).
    let outcome = run_openlore_scrape(
        &env,
        &["scrape", "github", "ghost-org/ghost-repo"],
        github.base_url(),
    );

    // THEN the run exits NON-ZERO (a 404 is an error, not an empty harvest).
    assert_ne!(
        outcome.status, 0,
        "a non-existent target must exit non-zero; \n--- stdout ---\n{}\n--- stderr ---\n{}",
        outcome.stdout, outcome.stderr
    );

    // AND the error on stderr NAMES the target so zero-candidates is
    // explainable (US-SCR-001 Ex 3 — the user must know WHICH target failed).
    assert!(
        outcome.stderr.contains("ghost-org/ghost-repo"),
        "stderr must name the failing target ghost-org/ghost-repo; \n--- stderr ---\n{}",
        outcome.stderr
    );

    // AND the error names the NOT-FOUND cause (the railway-oriented
    // GithubError::NotFound Display: "github target not found: {target}").
    assert!(
        outcome.stderr.contains("not found"),
        "stderr must name the not-found cause; \n--- stderr ---\n{}",
        outcome.stderr
    );

    // AND ZERO candidates are produced — no numbered candidate list is
    // rendered (the resolve refusal short-circuits BEFORE any harvest /
    // derivation, so no `[1]` line and no candidate-list footer can appear).
    assert!(
        !outcome.stdout.contains("[1]"),
        "a refused target must render NO numbered candidate list; \n--- stdout ---\n{}",
        outcome.stdout
    );
    assert!(
        !outcome
            .stdout
            .contains("nothing is a claim until you sign it"),
        "a refused target must render NO candidate-list footer; \n--- stdout ---\n{}",
        outcome.stdout
    );

    // AND nothing was persisted: zero `claims` rows, zero PDS writes, zero
    // claim artifact files (scraper_never_persists_unsigned holds on the
    // error path too — a refused scrape is never a mutation).
    assert_no_claim_persisted(&env);
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
    // GIVEN an initialized env + a PRIVATE/inaccessible target. The double has
    // NO private surface (public-data-only is structural): the public-only API
    // refuses with a 404 that carries the private signal, which the adapter
    // classifies as GithubError::NotPublic (03-05).
    let env = TestEnv::initialized();
    let github = GithubServer::start(FakeGithub::for_private_target("acme-corp/secret-repo"));

    // WHEN Maria scrapes the private target (no --sign).
    let outcome = run_openlore_scrape(
        &env,
        &["scrape", "github", "acme-corp/secret-repo"],
        github.base_url(),
    );

    // THEN the run exits NON-ZERO — a private target is refused, not an empty
    // harvest (KPI-SCR-4 release-gate).
    assert_ne!(
        outcome.status, 0,
        "a private target must be refused with a non-zero exit; \
         \n--- stdout ---\n{}\n--- stderr ---\n{}",
        outcome.stdout, outcome.stderr
    );

    // AND the refusal message states the scraper only reads public data — the
    // load-bearing no-surveillance reassurance (WD-51 / I-SCR-2). This is the
    // `GithubError::NotPublic` Display, distinct from the `NotFound` cause.
    assert!(
        outcome.stderr.contains("only reads public data"),
        "stderr must state the scraper only reads public data; \n--- stderr ---\n{}",
        outcome.stderr
    );

    // AND the error names the refused target so the refusal is explainable.
    assert!(
        outcome.stderr.contains("acme-corp/secret-repo"),
        "stderr must name the refused target acme-corp/secret-repo; \n--- stderr ---\n{}",
        outcome.stderr
    );

    // AND — the load-bearing structural guarantee (gate
    // `scraper_only_reads_public_data`, KPI-SCR-4): EVERY request path the
    // production code hit is on the PUBLIC endpoint allowlist (`/repos/...` or
    // `/users/...`). NO private/authenticated-private endpoint was ever called.
    // The refusal short-circuits at resolve, so no deeper private fetch follows.
    let seen_paths = github.fake().seen_paths();
    assert!(
        seen_paths
            .iter()
            .all(|path| path.starts_with("/repos/") || path.starts_with("/users/")),
        "scraper_only_reads_public_data (KPI-SCR-4): every requested path must be on the \
         public allowlist (/repos/... or /users/...); no private endpoint may be called; \
         got {seen_paths:?}"
    );

    // AND ZERO candidates are produced — no numbered candidate list is rendered
    // (the resolve refusal short-circuits BEFORE any harvest / derivation).
    assert!(
        !outcome.stdout.contains("[1]"),
        "a refused private target must render NO numbered candidate list; \
         \n--- stdout ---\n{}",
        outcome.stdout
    );
    assert!(
        !outcome
            .stdout
            .contains("nothing is a claim until you sign it"),
        "a refused private target must render NO candidate-list footer; \
         \n--- stdout ---\n{}",
        outcome.stdout
    );

    // AND nothing was persisted: zero `claims` rows, zero PDS writes, zero
    // claim artifact files (scraper_never_persists_unsigned holds on the
    // refusal path too — a refused scrape is never a mutation).
    assert_no_claim_persisted(&env);
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
    // GIVEN an initialized env + an OFFLINE posture: FakeGithub::offline()
    // serves a server that DROPS every connection without responding, so the
    // production code's reqwest GET sees a transport error (the adapter lifts
    // that into GithubError::Network — US-SCR-001 offline UAT).
    let env = TestEnv::initialized();
    let github = GithubServer::start(FakeGithub::offline());

    // WHEN Maria scrapes a public repo with no reachable network (no --sign).
    let outcome = run_openlore_scrape(
        &env,
        &["scrape", "github", "rust-lang/cargo"],
        github.base_url(),
    );

    // THEN the run exits NON-ZERO — an offline harvest is a failure, not an
    // empty harvest (the only network step refuses; nothing is fabricated).
    assert_ne!(
        outcome.status, 0,
        "an offline scrape must exit non-zero; \n--- stdout ---\n{}\n--- stderr ---\n{}",
        outcome.stdout, outcome.stderr
    );

    // AND the error on stderr states the scrape REQUIRES NETWORK access — the
    // honest, actionable cause (GithubError::Network Display: "... — scrape
    // requires network access"), so the offline user knows WHY it failed.
    assert!(
        outcome.stderr.contains("requires network"),
        "stderr must state the scrape requires network access; \n--- stderr ---\n{}",
        outcome.stderr
    );

    // AND NO partial candidate list is rendered — the transport failure
    // short-circuits at the harvest (the ONLY network step) BEFORE any
    // derivation, so no `[1]` line and no candidate-list footer can appear
    // (no half-built, misleading output).
    assert!(
        !outcome.stdout.contains("[1]"),
        "an offline scrape must render NO numbered candidate list; \n--- stdout ---\n{}",
        outcome.stdout
    );
    assert!(
        !outcome
            .stdout
            .contains("nothing is a claim until you sign it"),
        "an offline scrape must render NO candidate-list footer; \n--- stdout ---\n{}",
        outcome.stdout
    );

    // AND nothing was persisted: zero `claims` rows, zero PDS writes, zero
    // claim artifact files (scraper_never_persists_unsigned holds on the
    // offline path too — a failed scrape is never a mutation).
    assert_no_claim_persisted(&env);
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
    // GIVEN an initialized env + a REACHABLE public target whose harvest
    // yields ZERO signals the mapping can use (resolve succeeds; the signal
    // set maps to no predicate — the SD-5 empty-derive path). This is the
    // edge case distinct from SG-4: nothing-to-propose is NOT an error.
    let env = TestEnv::initialized();
    let github = GithubServer::start(FakeGithub::with_no_matching_signals(
        "some-user/empty-experiment",
    ));

    // WHEN Maria scrapes the empty-experiment target (no --sign).
    let outcome = run_openlore_scrape(
        &env,
        &["scrape", "github", "some-user/empty-experiment"],
        github.base_url(),
    );

    // THEN the run exits ZERO — a target that resolves but yields no usable
    // signals is a clean no-op, NOT an error (contrast SG-4's non-zero 404).
    assert_eq!(
        outcome.status, 0,
        "no-matching-signals must exit 0 (nothing to propose is NOT an error); \
         \n--- stdout ---\n{}\n--- stderr ---\n{}",
        outcome.stdout, outcome.stderr
    );

    // AND the public-data-only banner still precedes the harvest line — the
    // reassurance arc is unchanged by the empty result (the resolve succeeded,
    // so the full happy-path preamble runs).
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

    // AND the no-candidates message is printed verbatim (US-SCR-002 Ex 2):
    // the user is told nothing could be derived rather than seeing an empty
    // numbered list.
    assert!(
        outcome
            .stdout
            .contains("No candidate claims could be derived"),
        "stdout must state that no candidate claims could be derived; \n--- stdout ---\n{}",
        outcome.stdout
    );

    // AND ZERO candidates are rendered — no numbered candidate list and no
    // candidate-list footer (the empty branch short-circuits the list render).
    assert!(
        !outcome.stdout.contains("[1]"),
        "an empty harvest must render NO numbered candidate list; \n--- stdout ---\n{}",
        outcome.stdout
    );
    assert!(
        !outcome
            .stdout
            .contains("nothing is a claim until you sign it"),
        "an empty harvest must render NO candidate-list footer; \n--- stdout ---\n{}",
        outcome.stdout
    );

    // AND nothing was persisted: zero `claims` rows, zero PDS writes, zero
    // claim artifact files (scraper_never_persists_unsigned holds on the
    // empty path too — a scrape that proposes nothing is never a mutation).
    assert_no_claim_persisted(&env);
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
    // GIVEN an initialized env + a public repo serving 5 public signals (so
    // candidates ARE derived — the gate must hold regardless of how many
    // candidates are proposed, not only on the empty path).
    let env = TestEnv::initialized();
    let github = GithubServer::start(FakeGithub::for_public_repo_with_all_signals("rust-lang/cargo"));

    // WHEN Maria scrapes the public repo WITHOUT --sign.
    let outcome = run_openlore_scrape(
        &env,
        &["scrape", "github", "rust-lang/cargo"],
        github.base_url(),
    );

    // THEN the run succeeds on the happy path (a successful harvest+propose;
    // the human-gate guarantee is about a SUCCESSFUL scrape persisting nothing,
    // not about an error path).
    assert_eq!(
        outcome.status, 0,
        "scrape without --sign must exit 0 on the happy path; \
         \n--- stdout ---\n{}\n--- stderr ---\n{}",
        outcome.stdout, outcome.stderr
    );

    // AND the harvest+propose actually happened (candidates were derived) — so
    // the zero-writes assertion below is load-bearing, not vacuous on an empty
    // harvest. The numbered candidate list is rendered.
    assert!(
        outcome.stdout.contains("[1]"),
        "the scrape must have derived candidates (so zero-writes is non-vacuous); \
         \n--- stdout ---\n{}",
        outcome.stdout
    );

    // AND the human-gate held at the PUBLISH layer: ZERO records exist on the
    // user's own PDS and ZERO `create_record` (publish) calls were made
    // (scraper_never_persists_unsigned from the PDS side; WD-55 / KPI-SCR-2).
    assert_no_pds_call_was_made(&env);
    assert!(
        env.pds.records().is_empty(),
        "the human-gate must hold at the publish layer: ZERO PDS records after a \
         scrape with no --sign; got {} records: {:?}",
        env.pds.records().len(),
        env.pds.records()
    );

    // AND nothing was persisted anywhere else either: zero `claims` rows, zero
    // claim artifact files (the storage-layer half of the same gate).
    assert_no_claim_persisted(&env);
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
    // GIVEN an initialized env + a public repo serving 5 public signals. The
    // SAME server (idempotent target) backs every invocation — a pure read of
    // an unchanged target must yield an unchanged candidate list.
    let env = TestEnv::initialized();
    let github = GithubServer::start(FakeGithub::for_public_repo_with_all_signals("rust-lang/cargo"));

    // count_candidates :: the rendered candidate count is the observable
    // proxy for "the same candidate list" — every `[n]` line is one candidate.
    // (Port-exposed observable: stdout numbered list, not an internal field.)
    let count_candidates = |stdout: &str| -> usize {
        (1..)
            .take_while(|index| stdout.contains(&format!("[{index}]")))
            .count()
    };

    // WHEN Maria scrapes the SAME public target REPEATEDLY (no --sign). Three
    // runs make the "no accumulation across repeated runs" contract
    // load-bearing — a leak would compound run-over-run, not just appear once.
    let runs: Vec<CliOutcome> = (0..3)
        .map(|_| {
            run_openlore_scrape(
                &env,
                &["scrape", "github", "rust-lang/cargo"],
                github.base_url(),
            )
        })
        .collect();

    // THEN every run exits 0 and renders a non-empty candidate list (so the
    // pure-read invariant below is non-vacuous — each run actually proposed).
    for (attempt, outcome) in runs.iter().enumerate() {
        assert_eq!(
            outcome.status,
            0,
            "repeated scrape run #{} must exit 0; \n--- stdout ---\n{}\n--- stderr ---\n{}",
            attempt + 1,
            outcome.stdout,
            outcome.stderr
        );
        assert!(
            outcome.stdout.contains("[1]"),
            "repeated scrape run #{} must render candidates (so pure-read is non-vacuous); \
             \n--- stdout ---\n{}",
            attempt + 1,
            outcome.stdout
        );
    }

    // AND every run renders the SAME candidate count — a pure read of an
    // unchanged target is deterministic: same input -> same output, no drift,
    // no accumulation in the rendered list run-over-run.
    let first_count = count_candidates(&runs[0].stdout);
    assert_eq!(
        first_count, 5,
        "the first run must render the 5 candidates of the unchanged target; \
         \n--- stdout ---\n{}",
        runs[0].stdout
    );
    for (attempt, outcome) in runs.iter().enumerate() {
        let count = count_candidates(&outcome.stdout);
        assert_eq!(
            count,
            first_count,
            "every repeated scrape run must render the SAME candidate count ({} expected); \
             run #{} rendered {}; a pure read must not drift or accumulate; \
             \n--- stdout ---\n{}",
            first_count,
            attempt + 1,
            count,
            outcome.stdout
        );
    }

    // AND after ALL runs the human-gate STILL holds at the storage layer: zero
    // `claims` rows, zero PDS writes, zero claim artifact files. Nothing
    // accumulated across the repeated reads — a scrape is never a mutation no
    // matter how many times it runs (scraper_never_persists_unsigned, KPI-SCR-2).
    assert_no_claim_persisted(&env);
}
