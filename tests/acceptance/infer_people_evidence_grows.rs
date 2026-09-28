//! contributor-philosophy-inference — slice-04 acceptance: new repos grow the
//! inference WITHOUT rewriting what Maria signed (US-CPI-004; D-5; DDD-8/12/14;
//! ADR-064 §4-5; UC-4, UC-5; Q-CPI-D4).
//!
//! Drives the real `openlore scrape github` / `infer people` verbs as
//! subprocesses over a REAL store. Fake (driven-external only): `FakeGithub`,
//! `PeerPds`, `FakePds`, `FakeIdentity`.
//!
//! The append-only guarantee (KPI-CPI-4) is asserted as a byte-compare of the
//! signed-claim artifacts Maria already holds, plus "the system authored no
//! marker": the only new claim files are the ones she explicitly signed.
//!
//! Layer 3 subprocess — example-only (Mandate 9/11). All scenarios
//! `#[ignore]`d for one-at-a-time unskip.

mod support;

#[allow(unused_imports)]
use support::people::*;
#[allow(unused_imports)]
use support::*;

use claim_domain::ReferenceType;

/// GIVEN (chained, Pillar 2): BurntSushi builds ripgrep + regex; Maria signed
/// ripgrep memory-safety and then SIGNED the inference from that ONE repo
/// (claim `p7`). Returns `(p7, ripgrep_claim)`.
fn given_i_signed_the_inference_from_ripgrep_alone(env: &TestEnv) -> (String, String) {
    given_repo_scraped(env, "BurntSushi/ripgrep", burntsushi_only(), None);
    given_repo_scraped(env, "rust-lang/regex", burntsushi_only(), None);
    let ripgrep_claim =
        given_i_signed_repo_claim(env, "BurntSushi/ripgrep", MEMORY_SAFETY, 0.55, None);
    let before = claim_files(env);
    let signed = infer_people_with(env, &["--sign", "1"], &sign_stdin("0.15", false), None);
    assert_eq!(
        signed.status, 0,
        "precondition: sign p7;\n{}\n{}",
        signed.stdout, signed.stderr
    );
    let p7 = new_claim_cids(&before, env).pop().expect("p7 signed");
    (p7, ripgrep_claim)
}

/// GIVEN (chained): the canonical two-repo store (Maria ripgrep + Rachel
/// regex) and Maria signed the inference citing BOTH (claim `p9`).
fn given_i_signed_the_inference_citing_rachel(env: &TestEnv) -> (String, SubscribedPeer) {
    let (_, rachel) = given_burntsushi_builds_two_signed_memory_safe_repos(env);
    let before = claim_files(env);
    let signed = infer_people_with(env, &["--sign", "1"], &sign_stdin("", false), None);
    assert_eq!(
        signed.status, 0,
        "precondition: sign p9;\n{}\n{}",
        signed.stdout, signed.stderr
    );
    let p9 = new_claim_cids(&before, env).pop().expect("p9 signed");
    assert!(
        read_signed(env, &p9)
            .unsigned
            .evidence
            .contains(&at_uri(RACHEL_DID, &rachel.cids[0])),
        "precondition: p9 cites Rachel's regex claim"
    );
    (p9, rachel)
}

/// EG-1 (happy, NEW): BurntSushi is already linked to `rust-lang/regex`.
/// Maria scrapes regex and signs its semantic-versioning candidate in the same
/// run; the scrape ends with the "new inferred candidates" hint, and `infer
/// people` lists BurntSushi semantic-versioning marked NEW.
///
/// @us-cpi-004 @driving_port @real-io @kpi-cpi-4 @happy
#[test]
fn a_newly_signed_repo_philosophy_proposes_a_new_person_inference() {
    // GIVEN BurntSushi is linked to rust-lang/regex.
    let env = TestEnv::initialized();
    given_repo_scraped(&env, "rust-lang/regex", burntsushi_only(), None);

    // WHEN Maria scrapes regex and signs candidate 3 (semantic-versioning).
    let scrape = scrape_github(
        &env,
        &["rust-lang/regex", "--sign", "3"],
        repo_with_all_signals("rust-lang/regex", burntsushi_only()),
        None,
        &sign_stdin("", false),
    );

    // THEN the run ends with the hint …
    let out = &scrape.outcome;
    assert_eq!(out.status, 0, "{}\n{}", out.stdout, out.stderr);
    assert!(
        out.stdout.contains("new inferred candidate"),
        "{}",
        out.stdout
    );
    assert!(
        out.stdout.contains("openlore infer people"),
        "{}",
        out.stdout
    );
    // … AND infer people marks BurntSushi semantic-versioning NEW.
    let listed = infer_people(&env, &[]);
    let line = listed
        .stdout
        .lines()
        .find(|l| l.contains("github:BurntSushi") && l.contains("semantic-versioning"))
        .unwrap_or_else(|| {
            panic!(
                "BurntSushi semantic-versioning expected;\n{}",
                listed.stdout
            )
        })
        .to_string();
    assert!(line.contains("NEW"), "{line}");
}

/// EG-2 (happy, STRONGER listing): Maria's p7 rests on ripgrep alone; she
/// then signs regex memory-safety. `infer people` labels BurntSushi
/// memory-safety STRONGER — naming p7, now 2 repos — and says her existing
/// claim is unchanged.
///
/// @us-cpi-004 @driving_port @real-io @ddd-8 @happy
#[test]
#[ignore = "DELIVER slice-04: unskip one-at-a-time (US-CPI-004 STRONGER label names the earlier claim)"]
fn stronger_evidence_is_offered_as_a_superseding_candidate() {
    // GIVEN p7 rests on one repo AND regex now has Maria's memory-safety claim.
    let env = TestEnv::initialized();
    let (p7, _) = given_i_signed_the_inference_from_ripgrep_alone(&env);
    given_i_signed_repo_claim(&env, "rust-lang/regex", MEMORY_SAFETY, 0.60, None);

    // WHEN she runs infer people.
    let out = infer_people(&env, &[]);

    // THEN candidate 1 is STRONGER, naming p7 and two supporting repos.
    assert_eq!(out.status, 0, "{}\n{}", out.stdout, out.stderr);
    assert_candidate(&out, 1, "github:BurntSushi", MEMORY_SAFETY);
    let block = candidate_block(&out.stdout, 1).expect("block");
    assert!(block.contains("STRONGER"), "{block}");
    assert!(block.contains(&p7), "the earlier claim is named;\n{block}");
    assert!(block.contains("2 repos"), "{block}");
    assert!(
        block.contains("unchanged"),
        "must reassure the existing claim is unchanged;\n{block}"
    );
}

/// EG-3 (happy, supersede): continuing EG-2, Maria signs the STRONGER
/// candidate. A NEW claim is signed referencing p7 as superseded; p7 is still
/// stored byte-identical and was neither retracted nor countered.
///
/// @us-cpi-004 @driving_port @real-io @kpi-cpi-4 @d-5 @ddd-12 @happy
#[test]
#[ignore = "DELIVER slice-04: unskip one-at-a-time (US-CPI-004 STRONGER sign supersedes, old claim byte-identical)"]
fn signing_stronger_evidence_supersedes_and_never_edits_the_earlier_claim() {
    // GIVEN the EG-2 state.
    let env = TestEnv::initialized();
    let (p7, _) = given_i_signed_the_inference_from_ripgrep_alone(&env);
    given_i_signed_repo_claim(&env, "rust-lang/regex", MEMORY_SAFETY, 0.60, None);
    let files_before = claim_files(&env);

    // WHEN Maria signs the STRONGER candidate.
    let signed = infer_people_with(&env, &["--sign", "1"], &sign_stdin("", false), None);

    // THEN one new claim, superseding p7 …
    assert_eq!(signed.status, 0, "{}\n{}", signed.stdout, signed.stderr);
    let new = new_claim_cids(&files_before, &env);
    assert_eq!(new.len(), 1, "exactly the claim she signed: {new:?}");
    let successor = read_signed(&env, &new[0]);
    assert_eq!(successor.unsigned.references.len(), 1);
    assert_eq!(
        successor.unsigned.references[0].ref_type,
        ReferenceType::Supersedes
    );
    assert_eq!(successor.unsigned.references[0].cid.0, p7);
    assert_eq!(
        successor
            .unsigned
            .evidence
            .iter()
            .filter(|e| e.starts_with("at://"))
            .count(),
        2
    );
    // … AND every claim she already held is byte-identical (KPI-CPI-4).
    let after = claim_files(&env);
    for (cid, bytes) in &files_before {
        assert_eq!(
            after.get(cid),
            Some(bytes),
            "signed claim {cid} must be byte-identical"
        );
    }
}

/// EG-4 (error-ish, retracted support): Maria's p9 cites Rachel's regex
/// claim; Rachel soft-retracts it. `infer people --person github:BurntSushi`
/// flags p9 "support weakened: 1 of 2 supporting claims retracted" — and p9
/// is not retracted, countered or modified; nothing is written.
///
/// @us-cpi-004 @driving_port @real-io @kpi-cpi-4 @d-5 @uc-5 @error
#[test]
#[ignore = "DELIVER slice-04: unskip one-at-a-time (US-CPI-004 retracted support flagged, never acted on)"]
fn retracted_support_is_flagged_not_acted_on() {
    // GIVEN p9 cites Rachel's claim, which Rachel then retracts.
    let env = TestEnv::initialized();
    let (p9, mut rachel) = given_i_signed_the_inference_citing_rachel(&env);
    let original = PeerClaimSpec::repo_claim("rust-lang/regex", MEMORY_SAFETY, 0.60);
    let target = rachel.cids[0].clone();
    rachel.publish_and_pull(
        &env,
        vec![original.marking(ReferenceType::Retracts, &target)],
    );
    let files_before = claim_files(&env);
    let before = capture_store_universe(&env);

    // WHEN Maria reviews BurntSushi.
    let out = infer_people(&env, &["--person", "github:BurntSushi"]);

    // THEN p9 is flagged with the reason; nothing changed.
    assert_eq!(out.status, 0, "{}\n{}", out.stdout, out.stderr);
    assert!(out.stdout.contains(&p9), "p9 is shown;\n{}", out.stdout);
    assert!(
        out.stdout.to_lowercase().contains("support weakened"),
        "{}",
        out.stdout
    );
    assert!(
        out.stdout.contains("1 of 2 supporting claims retracted"),
        "{}",
        out.stdout
    );
    assert_store_unchanged(&before, &env);
    assert_eq!(
        claim_files(&env),
        files_before,
        "no signed claim modified, no marker authored"
    );
}

/// EG-5 (error-ish, UC-5): Maria unsubscribes from Rachel; p9 is flagged
/// support weakened with the reason "peer no longer subscribed", unchanged.
///
/// @us-cpi-004 @driving_port @real-io @kpi-cpi-4 @uc-5 @error
#[test]
#[ignore = "DELIVER slice-04: unskip one-at-a-time (UC-5 weakened: peer no longer subscribed)"]
fn support_from_an_unsubscribed_peer_is_flagged_with_its_reason() {
    // GIVEN p9 cites Rachel's claim and Maria soft-removes Rachel.
    let env = TestEnv::initialized();
    let (p9, rachel) = given_i_signed_the_inference_citing_rachel(&env);
    rachel.unsubscribe(&env);
    let files_before = claim_files(&env);

    // WHEN Maria reviews BurntSushi.
    let out = infer_people(&env, &["--person", "github:BurntSushi"]);

    // THEN p9 is weakened because the peer is no longer subscribed.
    assert_eq!(out.status, 0, "{}\n{}", out.stdout, out.stderr);
    assert!(out.stdout.contains(&p9), "{}", out.stdout);
    assert!(
        out.stdout.to_lowercase().contains("support weakened"),
        "{}",
        out.stdout
    );
    assert!(
        out.stdout.contains("no longer subscribed"),
        "{}",
        out.stdout
    );
    assert_eq!(claim_files(&env), files_before);
}

/// EG-6 (error-ish, UC-5): Maria purges Rachel's cached claims; p9's cited
/// support is no longer in her local store — flagged with that reason.
///
/// @us-cpi-004 @driving_port @real-io @kpi-cpi-4 @uc-5 @error
#[test]
#[ignore = "DELIVER slice-04: unskip one-at-a-time (UC-5 weakened: support not in local store)"]
fn support_missing_from_the_local_store_is_flagged_with_its_reason() {
    // GIVEN p9 cites Rachel's claim and Maria purges Rachel.
    let env = TestEnv::initialized();
    let (p9, rachel) = given_i_signed_the_inference_citing_rachel(&env);
    rachel.unsubscribe_and_purge(&env);
    let files_before = claim_files(&env);

    // WHEN Maria reviews BurntSushi.
    let out = infer_people(&env, &["--person", "github:BurntSushi"]);

    // THEN p9 is weakened because its support is not in the local store.
    assert_eq!(out.status, 0, "{}\n{}", out.stdout, out.stderr);
    assert!(out.stdout.contains(&p9), "{}", out.stdout);
    assert!(out.stdout.contains("not in local store"), "{}", out.stdout);
    assert_eq!(claim_files(&env), files_before);
}

/// EG-7 (edge): a scrape that changes no inference input prints NO hint (no
/// noise).
///
/// @us-cpi-004 @driving_port @real-io @edge
#[test]
fn a_scrape_that_changes_nothing_prints_no_hint() {
    // GIVEN the canonical store (one candidate already exists).
    let env = TestEnv::initialized();
    given_burntsushi_builds_two_signed_memory_safe_repos(&env);

    // WHEN Maria re-scrapes ripgrep with the same contributors.
    let scrape = scrape_github(
        &env,
        &["BurntSushi/ripgrep"],
        repo_with("BurntSushi/ripgrep", burntsushi_only()),
        None,
        "",
    );

    // THEN the contributors were (re)recorded — so the scrape genuinely ran
    // the people step and the silence below is not vacuous …
    assert_eq!(
        scrape.outcome.status, 0,
        "{}\n{}",
        scrape.outcome.stdout, scrape.outcome.stderr
    );
    assert!(
        scrape.outcome.stdout.contains("Contributors recorded: 1"),
        "{}",
        scrape.outcome.stdout
    );
    // … AND no inference hint.
    assert!(
        !scrape.outcome.stdout.contains("inferred candidate"),
        "{}",
        scrape.outcome.stdout
    );
}

/// EG-8 (edge): new LINKS alone change the inference — Maria signed regex
/// semantic-versioning before ever scraping regex; scraping it now links
/// BurntSushi and the run reports 1 new inferred candidate.
///
/// @us-cpi-004 @driving_port @real-io @ddd-14 @edge
#[test]
fn newly_recorded_people_alone_can_create_new_inferences() {
    // GIVEN BurntSushi known from ripgrep; regex semver signed but regex unscraped.
    let env = TestEnv::initialized();
    given_repo_scraped(&env, "BurntSushi/ripgrep", burntsushi_only(), None);
    given_i_signed_repo_claim(&env, "rust-lang/regex", SEMANTIC_VERSIONING, 0.50, None);

    // WHEN Maria scrapes regex.
    let scrape = scrape_github(
        &env,
        &["rust-lang/regex"],
        repo_with("rust-lang/regex", burntsushi_only()),
        None,
        "",
    );

    // THEN the hint counts exactly one new inferred candidate.
    assert_eq!(
        scrape.outcome.status, 0,
        "{}\n{}",
        scrape.outcome.stdout, scrape.outcome.stderr
    );
    assert!(
        scrape.outcome.stdout.contains("1 new inferred candidate"),
        "{}",
        scrape.outcome.stdout
    );
}

/// EG-9 (edge, UC-4): a hand-authored adherence claim (no inferred
/// provenance) counts as already signed and is NEVER offered for supersession,
/// however much support grows.
///
/// @us-cpi-004 @driving_port @real-io @uc-4 @edge
#[test]
#[ignore = "DELIVER slice-04: unskip one-at-a-time (UC-4 hand-authored adherence never STRONGER)"]
fn a_hand_authored_adherence_is_already_signed_and_never_superseded() {
    // GIVEN Maria hand-signed BurntSushi memory-safety citing a blog post, and
    // two signed repos now support it.
    let env = TestEnv::initialized();
    given_burntsushi_builds_two_signed_memory_safe_repos(&env);
    let hand = given_i_signed(
        &env,
        "github:BurntSushi",
        ADHERES,
        MEMORY_SAFETY,
        0.70,
        &["https://blog.burntsushi.net/".to_string()],
        None,
    );

    // WHEN she runs infer people.
    let out = infer_people(&env, &[]);

    // THEN nothing is numbered, the hand claim is "already signed", no STRONGER.
    assert_eq!(out.status, 0, "{}\n{}", out.stdout, out.stderr);
    assert_eq!(numbered_count(&out.stdout), 0, "{}", out.stdout);
    assert!(
        out.stdout.contains("already signed") && out.stdout.contains(&hand),
        "{}",
        out.stdout
    );
    assert!(!out.stdout.contains("STRONGER"), "{}", out.stdout);
}

/// EG-10 (edge, Q-CPI-D4): Maria holds TWO current inferred claims for
/// BurntSushi memory-safety (composed T1 and T2, both citing ripgrep only).
/// When regex support arrives, the STRONGER candidate supersedes the LATEST
/// (T2); the T1 claim stays as already signed.
///
/// @us-cpi-004 @driving_port @real-io @q-cpi-d4 @edge
#[test]
#[ignore = "DELIVER slice-04: unskip one-at-a-time (Q-CPI-D4 supersede the latest of several current claims)"]
fn with_two_current_inferred_claims_the_latest_is_the_one_superseded() {
    // GIVEN two inferred-shape claims for the pair, at T1 and T2.
    let env = TestEnv::initialized();
    given_repo_scraped(&env, "BurntSushi/ripgrep", burntsushi_only(), None);
    given_repo_scraped(&env, "rust-lang/regex", burntsushi_only(), None);
    let rg = given_i_signed_repo_claim(&env, "BurntSushi/ripgrep", MEMORY_SAFETY, 0.55, None);
    let evidence = vec![
        at_uri(&my_did(&env), &rg),
        commits_url("BurntSushi/ripgrep", "BurntSushi"),
    ];
    let _older = given_i_signed(
        &env,
        "github:BurntSushi",
        ADHERES,
        MEMORY_SAFETY,
        0.15,
        &evidence,
        Some(T1),
    );
    let latest = given_i_signed(
        &env,
        "github:BurntSushi",
        ADHERES,
        MEMORY_SAFETY,
        0.15,
        &evidence,
        Some(T2),
    );
    given_i_signed_repo_claim(&env, "rust-lang/regex", MEMORY_SAFETY, 0.60, None);
    let files_before = claim_files(&env);

    // WHEN Maria signs the STRONGER candidate.
    let signed = infer_people_with(&env, &["--sign", "1"], &sign_stdin("", false), Some(T3));

    // THEN it supersedes the T2 claim.
    assert_eq!(signed.status, 0, "{}\n{}", signed.stdout, signed.stderr);
    let cid = new_claim_cids(&files_before, &env)
        .pop()
        .expect("successor");
    let refs = read_signed(&env, &cid).unsigned.references;
    assert_eq!(refs.len(), 1);
    assert_eq!(refs[0].ref_type, ReferenceType::Supersedes);
    assert_eq!(refs[0].cid.0, latest);
}

/// EG-11 (production data, live GitHub — slice-04 brief): with Maria's
/// inference signed from real ripgrep alone, scraping the real regex and
/// signing its memory-safety claim offers BurntSushi as STRONGER.
///
/// @us-cpi-004 @real-io @requires_external @live-github
#[test]
#[ignore = "live-github: network-dependent production-data check; run with OPENLORE_LIVE_GITHUB=1 cargo test -p cli --test infer_people_evidence_grows -- --ignored live"]
fn live_scraping_regex_offers_burntsushi_memory_safety_as_stronger() {
    if std::env::var("OPENLORE_LIVE_GITHUB").is_err() {
        eprintln!("skipped: set OPENLORE_LIVE_GITHUB=1 to run against api.github.com");
        return;
    }
    let env = TestEnv::initialized();
    let rg = openlore_with(
        &env,
        &["scrape", "github", "BurntSushi/ripgrep"],
        Invocation::default(),
    );
    assert_eq!(rg.status, 0, "{}\n{}", rg.stdout, rg.stderr);
    given_i_signed_repo_claim(&env, "BurntSushi/ripgrep", MEMORY_SAFETY, 0.55, None);
    let signed = infer_people_with(
        &env,
        &["--person", "github:BurntSushi", "--sign", "1"],
        &sign_stdin("", false),
        None,
    );
    assert_eq!(signed.status, 0, "{}\n{}", signed.stdout, signed.stderr);
    let rx = openlore_with(
        &env,
        &["scrape", "github", "rust-lang/regex"],
        Invocation::default(),
    );
    assert_eq!(rx.status, 0, "{}\n{}", rx.stdout, rx.stderr);
    given_i_signed_repo_claim(&env, "rust-lang/regex", MEMORY_SAFETY, 0.60, None);
    let out = infer_people(&env, &["--person", "github:BurntSushi"]);
    assert!(out.stdout.contains("STRONGER"), "{}", out.stdout);
}
