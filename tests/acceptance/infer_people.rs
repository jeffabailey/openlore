//! contributor-philosophy-inference — slice-02 acceptance: `openlore infer
//! people` proposes which philosophies a person likely holds, and WHY
//! (US-CPI-002; D-2, D-7, D-9; DDD-6/7/8/9; UC-6).
//!
//! Drives the real `openlore infer people` verb as a subprocess over a REAL
//! DuckDB store seeded through the real verbs (`scrape github`, `claim add`,
//! `claim retract`, `peer add`/`pull`/`remove`). Fake (driven-external only):
//! `FakeGithub` (seeding scrapes), `PeerPds`.
//!
//! Layer 3 subprocess — example-only (Mandate 9). The `@property`-shaped
//! guardrails (KPI-CPI-2 complete provenance, confidence ≤ 0.29 and ≤ the
//! strongest support) are example-PINNED here; their proptest forms belong to
//! DELIVER's pure `scraper-domain::people` unit layer (listed in
//! `distill/test-scenarios.md`). All scenarios `#[ignore]`d for one-at-a-time
//! unskip.
//!
//! Covers: every US-CPI-002 AC; KPI-CPI-2, KPI-CPI-3 (signed-only,
//! non-retracted, non-countered, non-superseded, active-peer support);
//! local-first KPI-5; D-8 vocabulary; UC-6.

mod support;

#[allow(unused_imports)]
use support::people::*;
#[allow(unused_imports)]
use support::*;

/// IP-1 (happy): BurntSushi builds ripgrep (Maria signed memory-safety 0.55)
/// and regex (subscribed peer Rachel signed memory-safety 0.60). One
/// candidate: BurntSushi adheres to memory-safety; its provenance lists BOTH
/// repo claims by CID, each with ITS OWN author and his rank there;
/// confidence 0.20 speculative with its arithmetic; nothing written.
///
/// @us-cpi-002 @driving_port @real-io @kpi-cpi-2 @d-7 @d-9 @happy
#[test]
fn a_person_is_proposed_for_the_philosophies_of_signed_repos_they_build() {
    // GIVEN BurntSushi is linked to two repos with signed memory-safety claims.
    let env = TestEnv::initialized();
    let (mine, rachel) = given_burntsushi_builds_two_signed_memory_safe_repos(&env);
    let before = capture_store_universe(&env);

    // WHEN Maria runs `openlore infer people`.
    let out = infer_people(&env, &[]);

    // THEN exactly one candidate, attributed per supporting claim.
    assert_eq!(out.status, 0, "{}\n{}", out.stdout, out.stderr);
    assert_eq!(numbered_count(&out.stdout), 1, "{}", out.stdout);
    assert_candidate(&out, 1, "github:BurntSushi", MEMORY_SAFETY);
    assert_provenance(
        &out,
        1,
        "github:BurntSushi/ripgrep",
        1,
        &mine,
        &my_did(&env),
    );
    assert_provenance(
        &out,
        1,
        "github:rust-lang/regex",
        1,
        &rachel.cids[0],
        RACHEL_DID,
    );
    assert_speculative_confidence(&out, 1, "0.20");
    // AND no merged / consensus wording (D-7 anti-merging).
    assert!(
        !out.stdout.to_lowercase().contains("consensus"),
        "{}",
        out.stdout
    );
    // AND nothing was signed or written.
    assert_store_unchanged(&before, &env);
}

/// IP-2 (guardrail, D-2 / KPI-CPI-3): unsigned scraper candidates never feed
/// inference — `dtolnay/serde` was scraped (five repo candidates proposed)
/// but nothing was signed, so dtolnay gets NO candidate and the output names
/// the repo as having no signed philosophy claims.
///
/// @us-cpi-002 @driving_port @real-io @kpi-cpi-3 @d-2 @guardrail
#[test]
fn unsigned_scraper_candidates_never_feed_inference() {
    // GIVEN Maria scraped dtolnay/serde (candidates proposed) and signed none.
    let env = TestEnv::initialized();
    let scraped = scrape_github(
        &env,
        &["dtolnay/serde"],
        repo_with_all_signals("dtolnay/serde", serde_contributors()),
        None,
        "",
    );
    assert!(
        scraped.outcome.stdout.contains("[1]"),
        "precondition: serde proposed candidates;\n{}",
        scraped.outcome.stdout
    );

    // WHEN she asks about dtolnay.
    let out = infer_people(&env, &["--person", "github:dtolnay"]);

    // THEN nothing is inferred and serde is named as lacking signed claims.
    assert_no_inferred_candidates(&out);
    assert!(
        out.stdout.contains("no signed philosophy claims"),
        "{}",
        out.stdout
    );
    assert!(out.stdout.contains("dtolnay/serde"), "{}", out.stdout);
}

/// IP-3 (edge): Rachel soft-retracts her regex claim; it stops supporting the
/// inference — the candidate now cites only ripgrep, confidence 0.15.
///
/// @us-cpi-002 @driving_port @real-io @kpi-cpi-3 @rc-02 @edge
#[test]
fn a_soft_retracted_repo_claim_stops_supporting_inferences() {
    // GIVEN the canonical two-repo store, then Rachel retracts her regex claim.
    let env = TestEnv::initialized();
    let (mine, mut rachel) = given_burntsushi_builds_two_signed_memory_safe_repos(&env);
    let original = PeerClaimSpec::repo_claim("rust-lang/regex", MEMORY_SAFETY, 0.60);
    let retracted_cid = rachel.cids[0].clone();
    rachel.publish_and_pull(
        &env,
        vec![original.marking(claim_domain::ReferenceType::Retracts, &retracted_cid)],
    );

    // WHEN Maria runs infer people.
    let out = infer_people(&env, &[]);

    // THEN the candidate cites only ripgrep (Maria's claim).
    assert_eq!(out.status, 0, "{}\n{}", out.stdout, out.stderr);
    assert_candidate(&out, 1, "github:BurntSushi", MEMORY_SAFETY);
    assert_provenance(
        &out,
        1,
        "github:BurntSushi/ripgrep",
        1,
        &mine,
        &my_did(&env),
    );
    assert_provenance_omits(&out, 1, &retracted_cid);
    assert_provenance_omits(&out, 1, "github:rust-lang/regex");
    assert_speculative_confidence(&out, 1, "0.15");
}

/// IP-4 (edge): no candidates is a normal outcome — Björn scraped one repo
/// and signed nothing: "No inferred candidates", exit 0.
///
/// @us-cpi-002 @driving_port @real-io @edge
#[test]
fn no_candidates_is_a_normal_outcome() {
    // GIVEN one scraped repo, no signed claims.
    let env = TestEnv::initialized();
    given_repo_scraped(&env, "BurntSushi/ripgrep", ripgrep_contributors(), None);

    // WHEN Björn runs infer people.
    let out = infer_people(&env, &[]);

    // THEN the empty-result message, exit 0.
    assert_no_inferred_candidates(&out);
}

/// IP-5 (edge, D-2 active peers only): after Maria unsubscribes from Rachel
/// (cache kept), Rachel's regex claim no longer supports anything.
///
/// @us-cpi-002 @driving_port @real-io @kpi-cpi-3 @d-2 @edge
#[test]
fn an_unsubscribed_peers_claims_no_longer_support_inferences() {
    // GIVEN the canonical store, then Maria soft-removes Rachel.
    let env = TestEnv::initialized();
    let (mine, rachel) = given_burntsushi_builds_two_signed_memory_safe_repos(&env);
    rachel.unsubscribe(&env);

    // WHEN she runs infer people.
    let out = infer_people(&env, &[]);

    // THEN only her own ripgrep claim supports the candidate.
    assert_eq!(out.status, 0, "{}\n{}", out.stdout, out.stderr);
    assert_provenance(
        &out,
        1,
        "github:BurntSushi/ripgrep",
        1,
        &mine,
        &my_did(&env),
    );
    assert_provenance_omits(&out, 1, &rachel.cids[0]);
}

/// IP-6 (edge, DDD-7): Rachel COUNTERS Maria's ripgrep claim. The counter —
/// which carries the same repo and philosophy — is never itself counted as
/// support, and a third-party counter does not hide Maria's claim: still two
/// supporting claims (Maria's ripgrep, Rachel's regex), never three.
///
/// @us-cpi-002 @driving_port @real-io @kpi-cpi-3 @ddd-7 @edge
#[test]
fn a_counter_claim_is_never_counted_as_support() {
    // GIVEN the canonical store, then Rachel counters Maria's ripgrep claim.
    let env = TestEnv::initialized();
    let (mine, mut rachel) = given_burntsushi_builds_two_signed_memory_safe_repos(&env);
    let counter = rachel.publish_and_pull(
        &env,
        vec![
            PeerClaimSpec::repo_claim("BurntSushi/ripgrep", MEMORY_SAFETY, 0.30)
                .marking(claim_domain::ReferenceType::Counters, &mine),
        ],
    );

    // WHEN Maria runs infer people.
    let out = infer_people(&env, &[]);

    // THEN the counter is not cited; the two genuine supports remain.
    assert_eq!(out.status, 0, "{}\n{}", out.stdout, out.stderr);
    assert_provenance_omits(&out, 1, &counter[0]);
    assert_provenance(
        &out,
        1,
        "github:BurntSushi/ripgrep",
        1,
        &mine,
        &my_did(&env),
    );
    assert_provenance(
        &out,
        1,
        "github:rust-lang/regex",
        1,
        &rachel.cids[0],
        RACHEL_DID,
    );
}

/// IP-7 (edge, DDD-7): Rachel supersedes her regex claim with a revised one;
/// only the CURRENT claim supports the inference, never the superseded one.
///
/// @us-cpi-002 @driving_port @real-io @kpi-cpi-3 @ddd-7 @edge
#[test]
fn a_superseded_repo_claim_is_replaced_by_its_successor_as_support() {
    // GIVEN the canonical store, then Rachel supersedes her regex claim.
    let env = TestEnv::initialized();
    let (_, mut rachel) = given_burntsushi_builds_two_signed_memory_safe_repos(&env);
    let old = rachel.cids[0].clone();
    let revised = rachel.publish_and_pull(
        &env,
        vec![
            PeerClaimSpec::repo_claim("rust-lang/regex", MEMORY_SAFETY, 0.70)
                .marking(claim_domain::ReferenceType::Supersedes, &old),
        ],
    );

    // WHEN Maria runs infer people.
    let out = infer_people(&env, &[]);

    // THEN the successor is cited, the superseded claim is not.
    assert_eq!(out.status, 0, "{}\n{}", out.stdout, out.stderr);
    assert_provenance(
        &out,
        1,
        "github:rust-lang/regex",
        1,
        &revised[0],
        RACHEL_DID,
    );
    assert_provenance_omits(&out, 1, &old);
}

/// IP-8 (edge): a (person, philosophy) pair Maria already signed is shown as
/// already signed (with its CID) and is NOT re-proposed as a numbered
/// candidate.
///
/// @us-cpi-002 @driving_port @real-io @edge
#[test]
fn an_already_signed_person_philosophy_is_not_proposed_again() {
    // GIVEN Maria already signed BurntSushi memory-safety from the inference.
    let env = TestEnv::initialized();
    given_burntsushi_builds_two_signed_memory_safe_repos(&env);
    let files_before = claim_files(&env);
    let signed = infer_people_with(&env, &["--sign", "1"], &sign_stdin("", false), None);
    assert_eq!(
        signed.status, 0,
        "precondition sign;\n{}\n{}",
        signed.stdout, signed.stderr
    );
    let adherence = new_claim_cids(&files_before, &env)
        .pop()
        .expect("signed adherence");

    // WHEN she runs infer people again.
    let out = infer_people(&env, &[]);

    // THEN nothing is numbered; the pair is listed as already signed.
    assert_eq!(out.status, 0, "{}\n{}", out.stdout, out.stderr);
    assert_eq!(numbered_count(&out.stdout), 0, "{}", out.stdout);
    assert!(out.stdout.contains("already signed"), "{}", out.stdout);
    assert!(
        out.stdout.contains(&adherence),
        "the existing claim is shown by CID;\n{}",
        out.stdout
    );
}

/// IP-9 (edge, UC-6): `github:` subjects join case-insensitively — ripgrep
/// scraped as `burntsushi/ripgrep` still meets Maria's claim on
/// `github:BurntSushi/ripgrep`.
///
/// @us-cpi-002 @driving_port @real-io @uc-6 @edge
#[test]
fn repos_differing_only_in_letter_case_are_the_same_repo() {
    // GIVEN the repo was scraped in lower case, the claim signed in GitHub's casing.
    let env = TestEnv::initialized();
    given_repo_scraped(&env, "burntsushi/ripgrep", ripgrep_contributors(), None);
    let mine = given_i_signed_repo_claim(&env, "BurntSushi/ripgrep", MEMORY_SAFETY, 0.55, None);

    // WHEN Maria runs infer people.
    let out = infer_people(&env, &[]);

    // THEN BurntSushi is proposed, supported by that claim.
    assert_eq!(out.status, 0, "{}\n{}", out.stdout, out.stderr);
    assert_candidate(&out, 1, "github:BurntSushi", MEMORY_SAFETY);
    assert_provenance(&out, 1, "ripgrep", 1, &mine, &my_did(&env));
}

/// IP-10 (guardrail, KPI-CPI-2 — `@property`, example-pinned at layer 3):
/// EVERY numbered candidate in a mixed store carries complete provenance — at
/// least one line naming a supporting claim CID, its author DID and a rank.
///
/// @us-cpi-002 @driving_port @real-io @kpi-cpi-2 @property @guardrail
#[test]
fn every_inferred_candidate_carries_complete_provenance() {
    // GIVEN several people, philosophies, and authors.
    let env = TestEnv::initialized();
    given_burntsushi_builds_two_signed_memory_safe_repos(&env);
    given_i_signed_repo_claim(&env, "rust-lang/regex", SEMANTIC_VERSIONING, 0.50, None);
    given_repo_scraped(&env, "dtolnay/serde", serde_contributors(), None);
    given_i_signed_repo_claim(&env, "dtolnay/serde", DEPENDENCY_PINNING, 0.70, None);

    // WHEN Maria runs infer people.
    let out = infer_people(&env, &[]);

    // THEN every candidate block has a complete provenance line.
    assert_eq!(out.status, 0, "{}\n{}", out.stdout, out.stderr);
    let n = numbered_count(&out.stdout);
    assert!(
        n >= 4,
        "several candidates expected (BurntSushi x2 + rx/serde devs);\n{}",
        out.stdout
    );
    for i in 1..=n {
        let block = candidate_block(&out.stdout, i).expect("block");
        assert!(
            block
                .lines()
                .skip(1)
                .any(|l| l.contains("bafy") && l.contains("did:") && l.contains('#')),
            "candidate [{i}] lacks a complete provenance line (CID + author + rank);\n{block}"
        );
    }
}

/// IP-11 (boundary, DDD-9 / OD-CPI-3): the proposed confidence never leaves
/// the speculative bucket and never exceeds the strongest support — four
/// supporting repos at 0.90 cap at 0.29; two at ≤ 0.179 floor to 0.17.
///
/// @us-cpi-002 @driving_port @real-io @ddd-9 @property @boundary
#[test]
fn proposed_confidence_is_capped_and_never_exceeds_the_strongest_support() {
    // GIVEN cap-person builds four repos signed at 0.90 and floor-person two
    // repos signed at 0.179 and 0.10.
    let env = TestEnv::initialized();
    for i in 1..=4 {
        let repo = format!("acme/cap-{i}");
        given_repo_scraped(
            &env,
            &repo,
            vec![FakeContributor::human("cap-person", 70_001, 100)],
            None,
        );
        given_i_signed_repo_claim(&env, &repo, MEMORY_SAFETY, 0.90, None);
    }
    for (i, confidence) in [(1, 0.179), (2, 0.10)] {
        let repo = format!("acme/floor-{i}");
        given_repo_scraped(
            &env,
            &repo,
            vec![FakeContributor::human("floor-person", 70_002, 100)],
            None,
        );
        given_i_signed_repo_claim(&env, &repo, MEMORY_SAFETY, confidence, None);
    }

    // WHEN Maria runs infer people.
    let out = infer_people(&env, &[]);

    // THEN [1] cap-person 0.29, [2] floor-person 0.17 (floor, not round).
    assert_eq!(out.status, 0, "{}\n{}", out.stdout, out.stderr);
    assert_candidate(&out, 1, "github:cap-person", MEMORY_SAFETY);
    assert_speculative_confidence(&out, 1, "0.29");
    assert_candidate(&out, 2, "github:floor-person", MEMORY_SAFETY);
    assert_speculative_confidence(&out, 2, "0.17");
}

/// IP-12 (guardrail, KPI-5 local-first): `infer people` works with no network
/// endpoint at all and writes nothing.
///
/// @us-cpi-002 @driving_port @real-io @kpi-5 @kpi-cpi-3 @guardrail
#[test]
fn inference_runs_offline_and_writes_nothing() {
    // GIVEN the canonical store.
    let env = TestEnv::initialized();
    given_burntsushi_builds_two_signed_memory_safe_repos(&env);
    let before = capture_store_universe(&env);

    // WHEN Maria runs infer people with no network.
    let out = infer_people_offline(&env, &[]);

    // THEN it lists the candidate and changes nothing.
    assert_eq!(out.status, 0, "{}\n{}", out.stdout, out.stderr);
    assert_candidate(&out, 1, "github:BurntSushi", MEMORY_SAFETY);
    assert_store_unchanged(&before, &env);
    assert_no_pds_call_was_made(&env);
}

/// IP-13 (boundary, DDD-13): `--min-repos 2` keeps only inferences supported
/// by at least two repos.
///
/// @us-cpi-002 @driving_port @real-io @ddd-13 @boundary
#[test]
fn a_minimum_number_of_supporting_repos_can_be_required() {
    // GIVEN memory-safety has 2 supporting repos, test-driven only 1.
    let env = TestEnv::initialized();
    given_burntsushi_builds_two_signed_memory_safe_repos(&env);
    given_i_signed_repo_claim(&env, "BurntSushi/ripgrep", TEST_DRIVEN, 0.40, None);

    // WHEN Maria requires two supporting repos.
    let out = infer_people(&env, &["--person", "github:BurntSushi", "--min-repos", "2"]);

    // THEN only memory-safety remains.
    assert_eq!(out.status, 0, "{}\n{}", out.stdout, out.stderr);
    assert_eq!(numbered_count(&out.stdout), 1, "{}", out.stdout);
    assert_candidate(&out, 1, "github:BurntSushi", MEMORY_SAFETY);
}

/// IP-14 (error, D-8): a person must be named as `github:<login>` — a bare
/// login (or a claim-author DID) is refused with the expected form.
///
/// @us-cpi-002 @driving_port @real-io @d-8 @error
#[test]
fn a_person_named_without_the_github_prefix_is_refused_with_the_expected_form() {
    // GIVEN any store.
    let env = TestEnv::initialized();

    // WHEN Maria names a person by bare login.
    let out = infer_people(&env, &["--person", "BurntSushi"]);

    // THEN refused, naming the github:<login> form.
    assert_ne!(out.status, 0, "{}", out.stdout);
    assert!(out.stderr.contains("github:<login>"), "{}", out.stderr);
}

/// IP-15 (guardrail, D-8 vocabulary): the new surface speaks of "person";
/// the shipped `--contributor <did>` keeps meaning CLAIM AUTHOR and says so.
///
/// @us-cpi-002 @driving_port @real-io @d-8 @guardrail
#[test]
fn the_new_surface_says_person_and_contributor_still_means_claim_author() {
    // GIVEN an initialized store.
    let env = TestEnv::initialized();

    // WHEN Maria reads the help of both commands.
    let infer_help = run_openlore(&env, &["infer", "people", "--help"]);
    let graph_help = run_openlore(&env, &["graph", "query", "--help"]);

    // THEN infer people says "person" and never "contributor" …
    assert_eq!(infer_help.status, 0, "{}", infer_help.stderr);
    assert!(
        infer_help.stdout.contains("--person"),
        "{}",
        infer_help.stdout
    );
    assert!(
        !infer_help.stdout.to_lowercase().contains("contributor"),
        "{}",
        infer_help.stdout
    );
    // … and graph query's --contributor is a claim author.
    assert!(
        graph_help.stdout.contains("--contributor"),
        "{}",
        graph_help.stdout
    );
    assert!(
        graph_help.stdout.contains("claim author"),
        "{}",
        graph_help.stdout
    );
}

/// IP-16 (production data, live GitHub — slice-02 brief): after real scrapes
/// of ripgrep + regex, Maria's signed ripgrep claim and peer Rachel's regex
/// claim yield one BurntSushi memory-safety candidate citing both.
///
/// @us-cpi-002 @real-io @requires_external @live-github
#[test]
#[ignore = "live-github: network-dependent production-data check; run with OPENLORE_LIVE_GITHUB=1 cargo test -p cli --test infer_people -- --ignored live"]
fn live_burntsushi_memory_safety_is_inferred_from_real_repos() {
    if std::env::var("OPENLORE_LIVE_GITHUB").is_err() {
        eprintln!("skipped: set OPENLORE_LIVE_GITHUB=1 to run against api.github.com");
        return;
    }
    let env = TestEnv::initialized();
    for repo in ["BurntSushi/ripgrep", "rust-lang/regex"] {
        let scraped = openlore_with(&env, &["scrape", "github", repo], Invocation::default());
        assert_eq!(scraped.status, 0, "{}\n{}", scraped.stdout, scraped.stderr);
    }
    let mine = given_i_signed_repo_claim(&env, "BurntSushi/ripgrep", MEMORY_SAFETY, 0.55, None);
    let rachel = SubscribedPeer::subscribe_and_pull(
        &env,
        RACHEL_DID,
        RACHEL_SEED,
        vec![PeerClaimSpec::repo_claim(
            "rust-lang/regex",
            MEMORY_SAFETY,
            0.60,
        )],
    );
    let out = infer_people(&env, &["--person", "github:BurntSushi"]);
    assert_eq!(out.status, 0, "{}\n{}", out.stdout, out.stderr);
    assert_candidate(&out, 1, "github:BurntSushi", MEMORY_SAFETY);
    let block = candidate_block(&out.stdout, 1).expect("block");
    assert!(
        block.contains(&mine) && block.contains(&rachel.cids[0]),
        "{block}"
    );
}
