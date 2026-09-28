//! contributor-philosophy-inference — slice-03 acceptance + the feature's
//! WALKING SKELETON: sign an inferred adherence I agree with; its provenance
//! travels inside the signed claim (US-CPI-003; D-1, D-9; ADR-064).
//!
//! Drives the real `openlore` CLI as a subprocess (the production composition
//! root). Real: claim-domain canonicalize/CID/sign, the shared sign batch
//! (DDD-12 — the SAME path `claim add` / scraper `--sign` use), DuckDB,
//! filesystem. Fake (driven-external only): `FakeGithub` (`/contributors`),
//! `PeerPds`, `FakePds`, `FakeIdentity`.
//!
//! Layer 3/5 subprocess — example-only (Mandate 9); sad paths are named
//! examples (Mandate 11). The WS is NOT `#[ignore]`: it is the first scenario
//! DELIVER turns green (thin thread through slices 01-03); every other
//! scenario is `#[ignore]`d with the slice/step that activates it.
//!
//! Covers: US-CPI-003 (all ACs), UC-7 (evidence wording), UC-8 (numbering
//! tied to filters), Q-CPI-D3 (bare author DID in at-uris), KPI-CPI-2
//! (provenance in the signed payload), KPI-CPI-3 (nothing written without
//! `--sign`).

mod support;

#[allow(unused_imports)]
use support::people::*;
#[allow(unused_imports)]
use support::*;

// =============================================================================
// WALKING SKELETON — NOT #[ignore]: RED at handoff, first GREEN in DELIVER
// =============================================================================

/// WS-CPI-1: Maria scraped `BurntSushi/ripgrep` and `rust-lang/regex` (both
/// record BurntSushi as their #1 contributor) and signed "memory-safety" for
/// both repos. She runs `openlore infer people --sign 1`, raises the proposed
/// confidence to 0.45 and signs. A claim "github:BurntSushi adheresToPhilosophy
/// memory-safety" now exists, signed with HER identity, at 0.45, and it
/// carries its evidence: each supporting repo claim (by AT-URI: author + CID)
/// and BurntSushi's commits page for each repo — so anyone can audit why.
///
/// Demo value (stakeholder litmus): "the claims I signed about repos now let
/// me sign an evidence-backed statement about the PERSON who builds them."
///
/// @walking_skeleton @driving_port @driving_adapter @real-io @us-cpi-001
/// @us-cpi-002 @us-cpi-003 @j-004 @j-004d @j-004e @kpi-cpi-1 @kpi-cpi-2
#[test]
fn maria_signs_an_evidence_backed_adherence_for_the_person_who_builds_her_signed_repos() {
    // GIVEN Maria scraped two repos whose #1 contributor is BurntSushi …
    let env = TestEnv::initialized();
    given_repo_scraped(&env, "BurntSushi/ripgrep", ripgrep_contributors(), None);
    given_repo_scraped(&env, "rust-lang/regex", regex_contributors(), None);
    // … AND signed memory-safety for both repos.
    let ripgrep_claim =
        given_i_signed_repo_claim(&env, "BurntSushi/ripgrep", MEMORY_SAFETY, 0.55, None);
    let regex_claim = given_i_signed_repo_claim(&env, "rust-lang/regex", MEMORY_SAFETY, 0.60, None);
    let files_before = claim_files(&env);
    let before = capture_store_universe(&env);

    // WHEN she signs inferred candidate 1 at 0.45 (declining publish).
    let signed = infer_people_with(&env, &["--sign", "1"], &sign_stdin("0.45", false), None);

    // THEN the run succeeds and proposed exactly that person/philosophy.
    assert_eq!(
        signed.status, 0,
        "`openlore infer people --sign 1` must sign the selected candidate;\n--- stdout ---\n{}\n--- stderr ---\n{}",
        signed.stdout, signed.stderr
    );
    assert_candidate(&signed, 1, "github:BurntSushi", MEMORY_SAFETY);

    // AND exactly one new claim was signed — nothing else in her store moved
    // (links untouched, nothing published).
    let new = new_claim_cids(&files_before, &env);
    assert_eq!(
        new.len(),
        1,
        "exactly one claim must be signed; new claim files: {new:?}"
    );
    let cid = new[0].clone();
    assert_store_delta(
        &before,
        &env,
        &[
            (
                "local.claims.row_count",
                (before["local.claims.row_count"].parse::<i64>().unwrap() + 1).to_string(),
            ),
            (
                "local.claim_files",
                format!("{:?}", claim_files(&env).keys().collect::<Vec<_>>()),
            ),
        ],
    );

    // AND the signed claim says what she agreed to, in her name, at 0.45.
    let claim = read_signed(&env, &cid);
    assert_eq!(claim.unsigned.subject, "github:BurntSushi");
    assert_eq!(claim.unsigned.predicate, ADHERES);
    assert_eq!(claim.unsigned.object, MEMORY_SAFETY);
    assert_eq!(bare(&claim.unsigned.author_did.0), my_did(&env));
    assert!(
        (confidence_of(&claim) - 0.45).abs() < 1e-9,
        "recorded confidence must be 0.45"
    );
    assert!(
        claim.unsigned.references.is_empty(),
        "a NEW inference supersedes nothing"
    );

    // AND its evidence IS the provenance, in ADR-064 order (supporting repos
    // in case-folded subject order; per repo: claim AT-URI, then commits URL).
    assert_eq!(
        claim.unsigned.evidence,
        vec![
            at_uri(&my_did(&env), &ripgrep_claim),
            commits_url("BurntSushi/ripgrep", "BurntSushi"),
            at_uri(&my_did(&env), &regex_claim),
            commits_url("rust-lang/regex", "BurntSushi"),
        ],
        "the signed evidence must carry the full provenance (D-9 / ADR-064 §3)"
    );
}

// =============================================================================
// US-CPI-003 — focused scenarios (activated one at a time in DELIVER)
// =============================================================================

/// IS-2: With a supporting claim from subscribed peer Rachel, the signed
/// adherence names EACH supporting claim with its OWN author (D-7), the
/// compose preview shows the derived-from summary and the AT-URI evidence
/// (UC-7), the CID re-verifies identically, and the published record carries
/// the same evidence a peer will read (D-9, KPI-CPI-2).
///
/// @us-cpi-003 @driving_port @real-io @kpi-cpi-2 @d-7 @d-9 @uc-7 @q-cpi-d3 @happy
#[test]
#[ignore = "DELIVER slice-03: unskip one-at-a-time (US-CPI-003 peer-attributed provenance survives signing + publishing)"]
fn signing_an_inference_keeps_each_supporting_claims_own_author_and_re_verifies() {
    // GIVEN BurntSushi builds ripgrep (Maria signed memory-safety 0.55) and
    // regex (subscribed peer Rachel signed memory-safety 0.60).
    let env = TestEnv::initialized();
    let (mine, rachel) = given_burntsushi_builds_two_signed_memory_safe_repos(&env);
    let files_before = claim_files(&env);

    // WHEN Maria signs candidate 1 at 0.45 and publishes it.
    let signed = infer_people_with(&env, &["--sign", "1"], &sign_stdin("0.45", true), None);

    // THEN the preview showed the derived-from summary + both AT-URIs.
    assert_eq!(
        signed.status, 0,
        "sign+publish must succeed;\n{}\n{}",
        signed.stdout, signed.stderr
    );
    assert!(
        signed.stdout.contains("derived-from: 2 signed repo claims"),
        "the compose preview must summarise its provenance (UC-7);\n{}",
        signed.stdout
    );
    let rachel_uri = at_uri(RACHEL_DID, &rachel.cids[0]);
    assert!(
        signed.stdout.contains(&rachel_uri)
            && signed.stdout.contains(&at_uri(&my_did(&env), &mine)),
        "the preview must list each supporting claim by AT-URI;\n{}",
        signed.stdout
    );

    // AND the signed evidence attributes Rachel's claim to Rachel (bare DID —
    // the same form `claim publish` mints; Q-CPI-D3), never to Maria.
    let cid = new_claim_cids(&files_before, &env)
        .pop()
        .expect("one signed claim");
    let claim = read_signed(&env, &cid);
    assert!(
        claim.unsigned.evidence.contains(&rachel_uri),
        "Rachel's claim must be cited as hers: {:?}",
        claim.unsigned.evidence
    );
    assert!(
        !claim.unsigned.evidence.iter().any(|e| e.contains(&format!(
            "{}/org.openlore.claim/{}",
            my_did(&env),
            rachel.cids[0]
        ))),
        "a peer's claim must never be presented as Maria's own (D-7)"
    );

    // AND re-verification recomputes the identical CID.
    assert_eq!(
        recomputed_cid(&claim),
        cid,
        "the signed claim's CID must be stable on re-verify"
    );

    // AND the published record (what a peer pulls) carries the same evidence.
    let published = published_cid_from_stdout(&signed.stdout);
    assert_eq!(published, cid);
    let record = env
        .pds
        .record_at(&at_uri(&my_did(&env), &cid))
        .expect("the adherence claim is on Maria's PDS");
    let wire = &record.body;
    assert!(
        wire.to_string().contains(&rachel_uri),
        "the federated record must carry the provenance a peer audits;\n{wire}"
    );
}

/// IS-3: Listing without `--sign` signs and writes nothing (KPI-CPI-3).
///
/// @us-cpi-003 @driving_port @real-io @kpi-cpi-3 @guardrail
#[test]
#[ignore = "DELIVER slice-03: unskip one-at-a-time (US-CPI-003 listing without --sign writes nothing)"]
fn listing_inferred_candidates_without_sign_writes_nothing() {
    // GIVEN three inferred candidates exist.
    let env = TestEnv::initialized();
    given_burntsushi_builds_two_signed_memory_safe_repos(&env);
    given_i_signed_repo_claim(&env, "rust-lang/regex", SEMANTIC_VERSIONING, 0.50, None);
    given_i_signed_repo_claim(&env, "BurntSushi/ripgrep", TEST_DRIVEN, 0.40, None);
    let before = capture_store_universe(&env);

    // WHEN Maria lists them.
    let listed = infer_people(&env, &[]);

    // THEN three are listed and nothing was signed, written or published.
    assert_eq!(listed.status, 0, "{}\n{}", listed.stdout, listed.stderr);
    assert!(
        numbered_count(&listed.stdout) >= 3,
        "three candidates expected;\n{}",
        listed.stdout
    );
    assert_store_unchanged(&before, &env);
}

/// IS-4 (error): `--sign 7` with three candidates is rejected BEFORE any
/// compose; nothing is signed.
///
/// @us-cpi-003 @driving_port @real-io @error
#[test]
#[ignore = "DELIVER slice-03: unskip one-at-a-time (US-CPI-003 out-of-range selection rejected before compose)"]
fn selecting_a_candidate_that_does_not_exist_is_rejected_before_composing() {
    // GIVEN three inferred candidates exist.
    let env = TestEnv::initialized();
    given_burntsushi_builds_two_signed_memory_safe_repos(&env);
    given_i_signed_repo_claim(&env, "rust-lang/regex", SEMANTIC_VERSIONING, 0.50, None);
    given_i_signed_repo_claim(&env, "BurntSushi/ripgrep", TEST_DRIVEN, 0.40, None);
    let before = capture_store_universe(&env);

    // WHEN Tobias selects candidate 7.
    let outcome = infer_people_with(&env, &["--sign", "7"], &sign_stdin("", false), None);

    // THEN the CLI exits non-zero naming the valid range, composes nothing.
    assert_ne!(
        outcome.status, 0,
        "an out-of-range selection must fail;\n{}",
        outcome.stdout
    );
    assert!(
        outcome
            .stderr
            .contains("candidate 7 does not exist; valid range 1..3"),
        "stderr must name the valid range;\n{}",
        outcome.stderr
    );
    assert!(
        !outcome.stdout.contains("Computing claim CID"),
        "nothing may be composed"
    );
    assert_store_unchanged(&before, &env);
}

/// IS-5 (edge): numbering is stable between the list and `--sign` — candidate
/// 2 signed is the (person, philosophy) listed as 2.
///
/// @us-cpi-003 @driving_port @real-io @edge
#[test]
#[ignore = "DELIVER slice-03: unskip one-at-a-time (US-CPI-003 deterministic numbering list == sign)"]
fn the_candidate_signed_is_the_one_listed_under_that_number() {
    // GIVEN several candidates exist and Maria has listed them.
    let env = TestEnv::initialized();
    given_burntsushi_builds_two_signed_memory_safe_repos(&env);
    given_i_signed_repo_claim(&env, "rust-lang/regex", SEMANTIC_VERSIONING, 0.50, None);
    let listed = infer_people(&env, &[]);
    let listed_two = candidate_line(&listed.stdout, 2).expect("candidate [2] listed");
    let files_before = claim_files(&env);

    // WHEN she signs candidate 2.
    let signed = infer_people_with(&env, &["--sign", "2"], &sign_stdin("", false), None);

    // THEN the signed claim is exactly listed candidate 2.
    assert_eq!(signed.status, 0, "{}\n{}", signed.stdout, signed.stderr);
    let cid = new_claim_cids(&files_before, &env)
        .pop()
        .expect("one signed claim");
    let claim = read_signed(&env, &cid);
    let short = claim
        .unsigned
        .object
        .rsplit('.')
        .next()
        .unwrap()
        .to_string();
    assert!(
        listed_two.contains(&claim.unsigned.subject) && listed_two.contains(&short),
        "signed ({} {}) must be the listed [2]: {listed_two:?}",
        claim.unsigned.subject,
        claim.unsigned.object
    );
}

/// IS-6 (edge, UC-8): `--sign N` numbers candidates identically to the list
/// run with the SAME filters — `--person github:dtolnay --sign 1` signs
/// dtolnay's candidate, not the unfiltered #1 (BurntSushi).
///
/// @us-cpi-003 @driving_port @real-io @uc-8 @edge
#[test]
#[ignore = "DELIVER slice-03: unskip one-at-a-time (UC-8 --sign numbering follows the same filters)"]
fn sign_numbers_candidates_within_the_same_person_filter_as_the_list() {
    // GIVEN BurntSushi AND dtolnay each have one candidate.
    let env = TestEnv::initialized();
    given_burntsushi_builds_two_signed_memory_safe_repos(&env);
    given_repo_scraped(&env, "dtolnay/serde", serde_contributors(), None);
    given_i_signed_repo_claim(&env, "dtolnay/serde", DEPENDENCY_PINNING, 0.50, None);
    let files_before = claim_files(&env);

    // WHEN Maria signs candidate 1 of the dtolnay-filtered list.
    let signed = infer_people_with(
        &env,
        &["--person", "github:dtolnay", "--sign", "1"],
        &sign_stdin("", false),
        None,
    );

    // THEN she signed dtolnay's candidate.
    assert_eq!(signed.status, 0, "{}\n{}", signed.stdout, signed.stderr);
    let cid = new_claim_cids(&files_before, &env)
        .pop()
        .expect("one signed claim");
    let claim = read_signed(&env, &cid);
    assert_eq!(claim.unsigned.subject, "github:dtolnay");
    assert_eq!(claim.unsigned.object, DEPENDENCY_PINNING);
}

/// IS-7 (error): a confidence outside [0.0, 1.0] is refused and re-asked;
/// the claim is signed only with a valid value.
///
/// @us-cpi-003 @driving_port @real-io @error
#[test]
#[ignore = "DELIVER slice-03: unskip one-at-a-time (US-CPI-003 confidence editable only within [0.0, 1.0])"]
fn an_out_of_range_confidence_is_refused_and_re_asked_before_signing() {
    // GIVEN one inferred candidate.
    let env = TestEnv::initialized();
    given_burntsushi_builds_two_signed_memory_safe_repos(&env);
    let files_before = claim_files(&env);

    // WHEN Maria types 1.5, then 0.45.
    let signed = infer_people_with(&env, &["--sign", "1"], "\n\n\n\n1.5\n0.45\n\nn\n", None);

    // THEN 1.5 is refused and the claim records 0.45.
    assert_eq!(signed.status, 0, "{}\n{}", signed.stdout, signed.stderr);
    assert!(
        format!("{}{}", signed.stdout, signed.stderr).contains("1.5"),
        "the refusal must name the rejected value"
    );
    let cid = new_claim_cids(&files_before, &env)
        .pop()
        .expect("one signed claim");
    assert!((confidence_of(&read_signed(&env, &cid)) - 0.45).abs() < 1e-9);
}

/// IS-8 (edge): accepting the default signs the PROPOSED speculative
/// confidence (0.20 for 2 supporting repos, max 0.60) — never auto-raised.
///
/// @us-cpi-003 @driving_port @real-io @edge @kpi-cpi-3
#[test]
#[ignore = "DELIVER slice-03: unskip one-at-a-time (US-CPI-003 default confidence never auto-raised)"]
fn accepting_the_proposed_confidence_signs_the_speculative_value_unchanged() {
    // GIVEN BurntSushi's candidate is proposed at 0.20.
    let env = TestEnv::initialized();
    given_burntsushi_builds_two_signed_memory_safe_repos(&env);
    let files_before = claim_files(&env);

    // WHEN Maria accepts every pre-filled field.
    let signed = infer_people_with(&env, &["--sign", "1"], &sign_stdin("", false), None);

    // THEN the signed confidence is exactly the proposal.
    assert_eq!(signed.status, 0, "{}\n{}", signed.stdout, signed.stderr);
    let cid = new_claim_cids(&files_before, &env)
        .pop()
        .expect("one signed claim");
    assert!((confidence_of(&read_signed(&env, &cid)) - 0.20).abs() < 1e-9);
}

/// IS-9 (happy): `--sign 1,2` signs two candidates in one pass, one claim
/// each, through the same sign batch.
///
/// @us-cpi-003 @driving_port @real-io @happy
#[test]
#[ignore = "DELIVER slice-03: unskip one-at-a-time (US-CPI-003 batch --sign N,N)"]
fn signing_two_candidates_in_one_pass_produces_one_claim_each() {
    // GIVEN two candidates for BurntSushi.
    let env = TestEnv::initialized();
    given_burntsushi_builds_two_signed_memory_safe_repos(&env);
    given_i_signed_repo_claim(&env, "rust-lang/regex", SEMANTIC_VERSIONING, 0.50, None);
    let files_before = claim_files(&env);

    // WHEN Maria signs both.
    let one = sign_stdin("", false);
    let signed = infer_people_with(&env, &["--sign", "1,2"], &format!("{one}{one}"), None);

    // THEN two adherence claims about BurntSushi exist.
    assert_eq!(signed.status, 0, "{}\n{}", signed.stdout, signed.stderr);
    let new = new_claim_cids(&files_before, &env);
    assert_eq!(new.len(), 2, "one claim per selected candidate: {new:?}");
    for cid in new {
        let claim = read_signed(&env, &cid);
        assert_eq!(claim.unsigned.subject, "github:BurntSushi");
        assert_eq!(claim.unsigned.predicate, ADHERES);
    }
}

/// IS-10 (production data, live GitHub — slice-03 brief): on real scrapes of
/// ripgrep + regex, signing the BurntSushi memory-safety inference yields a
/// claim whose evidence names both supporting claims and both commits pages.
///
/// @us-cpi-003 @real-io @requires_external @live-github
#[test]
#[ignore = "live-github: network-dependent production-data check; run with OPENLORE_LIVE_GITHUB=1 cargo test -p cli --test infer_people_sign -- --ignored live"]
fn live_signing_burntsushi_memory_safety_carries_both_real_supporting_claims() {
    if std::env::var("OPENLORE_LIVE_GITHUB").is_err() {
        eprintln!("skipped: set OPENLORE_LIVE_GITHUB=1 to run against api.github.com");
        return;
    }
    let env = TestEnv::initialized();
    for repo in ["BurntSushi/ripgrep", "rust-lang/regex"] {
        let scraped = openlore_with(&env, &["scrape", "github", repo], Invocation::default());
        assert_eq!(
            scraped.status, 0,
            "live scrape {repo};\n{}\n{}",
            scraped.stdout, scraped.stderr
        );
    }
    let rg = given_i_signed_repo_claim(&env, "BurntSushi/ripgrep", MEMORY_SAFETY, 0.55, None);
    let rx = given_i_signed_repo_claim(&env, "rust-lang/regex", MEMORY_SAFETY, 0.60, None);
    let files_before = claim_files(&env);
    let signed = infer_people_with(
        &env,
        &["--person", "github:BurntSushi", "--sign", "1"],
        &sign_stdin("0.45", false),
        None,
    );
    assert_eq!(signed.status, 0, "{}\n{}", signed.stdout, signed.stderr);
    let cid = new_claim_cids(&files_before, &env)
        .pop()
        .expect("one signed claim");
    let evidence = read_signed(&env, &cid).unsigned.evidence;
    for expected in [
        at_uri(&my_did(&env), &rg),
        at_uri(&my_did(&env), &rx),
        commits_url("BurntSushi/ripgrep", "BurntSushi"),
        commits_url("rust-lang/regex", "BurntSushi"),
    ] {
        assert!(
            evidence.contains(&expected),
            "live evidence must contain {expected}: {evidence:?}"
        );
    }
}
