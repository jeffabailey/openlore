//! contributor-philosophy-inference — slice-05 acceptance: `openlore scrape
//! github <user>` reads a PERSON, not just a repo (US-CPI-005; D-4; DDD-13;
//! Q-CPI-D7).
//!
//! Drives the real `openlore scrape github <user>` verb as a subprocess. The
//! person view is rendered from the LOCAL store (links, signed adherence,
//! inferred candidates); GitHub is asked only for the public profile. Fake
//! (driven-external only): `FakeGithub`, `FakePds`, `FakeIdentity`.
//!
//! Layer 3 subprocess — example-only (Mandate 9/11). All scenarios
//! `#[ignore]`d for one-at-a-time unskip.
//!
//! NOTE for DELIVER: this slice REPLACES the shipped user-target outcome
//! pinned by `scrape_github.rs` SG-3
//! (`scrape_github_resolves_user_target_and_derives_no_candidates_aggregation_deferred`,
//! which asserts "No candidate claims could be derived"); narrow SG-3 when
//! SP-3 is activated (see `distill/acceptance-review.md`).

mod support;

#[allow(unused_imports)]
use support::people::*;
#[allow(unused_imports)]
use support::*;

/// GIVEN (chained, Pillar 2): BurntSushi is #1 on ripgrep, regex and jiff;
/// Maria signed memory-safety (ripgrep, regex), semantic-versioning (regex,
/// jiff) and test-driven (ripgrep), all at `now`, then signed the inferred
/// BurntSushi memory-safety adherence (claim `p7`, at `sign_at`). Leaves two
/// inferred candidates: semantic-versioning (2 repos), test-driven (1 repo).
fn given_burntsushi_person_picture(
    env: &TestEnv,
    now: Option<&str>,
    sign_at: Option<&str>,
) -> String {
    for repo in ["BurntSushi/ripgrep", "rust-lang/regex", "BurntSushi/jiff"] {
        given_repo_scraped(env, repo, burntsushi_only(), now);
    }
    given_i_signed_repo_claim(env, "BurntSushi/ripgrep", MEMORY_SAFETY, 0.55, now);
    given_i_signed_repo_claim(env, "rust-lang/regex", MEMORY_SAFETY, 0.60, now);
    given_i_signed_repo_claim(env, "rust-lang/regex", SEMANTIC_VERSIONING, 0.50, now);
    given_i_signed_repo_claim(env, "BurntSushi/jiff", SEMANTIC_VERSIONING, 0.45, now);
    given_i_signed_repo_claim(env, "BurntSushi/ripgrep", TEST_DRIVEN, 0.40, now);
    let before = claim_files(env);
    let signed = infer_people_with(
        env,
        &["--person", "github:BurntSushi", "--sign", "1"],
        &sign_stdin("0.45", false),
        sign_at,
    );
    assert_eq!(
        signed.status, 0,
        "precondition: sign p7;\n{}\n{}",
        signed.stdout, signed.stderr
    );
    let p7 = new_claim_cids(&before, env).pop().expect("p7");
    assert_eq!(
        read_signed(env, &p7).unsigned.object,
        MEMORY_SAFETY,
        "precondition: [1] was memory-safety"
    );
    p7
}

/// SP-1 (happy): `scrape github BurntSushi` shows his accumulated picture —
/// the three linked repos with his rank in each, his signed adherence claim
/// with its CID, and his inferred candidates numbered for `--sign`.
///
/// @us-cpi-005 @driving_port @real-io @j-004a @happy
#[test]
#[ignore = "DELIVER slice-05: unskip one-at-a-time (US-CPI-005 person view: links + signed + candidates)"]
fn a_user_scrape_shows_the_persons_accumulated_picture() {
    // GIVEN BurntSushi's picture in Maria's store.
    let env = TestEnv::initialized();
    let p7 = given_burntsushi_person_picture(&env, None, None);

    // WHEN Maria scrapes the person.
    let scrape = scrape_github(
        &env,
        &["BurntSushi"],
        FakeGithub::for_public_user("BurntSushi"),
        None,
        "",
    );

    // THEN links with rank, the signed claim, and two numbered candidates.
    let out = &scrape.outcome;
    assert_eq!(out.status, 0, "{}\n{}", out.stdout, out.stderr);
    assert!(out.stdout.contains("github:BurntSushi"), "{}", out.stdout);
    assert!(
        out.stdout.contains("linked to 3 scraped repos"),
        "{}",
        out.stdout
    );
    for repo in ["ripgrep", "regex", "jiff"] {
        assert!(
            out.stdout
                .lines()
                .any(|l| l.contains(repo) && l.contains("#1")),
            "{repo} with his rank;\n{}",
            out.stdout
        );
    }
    assert!(
        out.stdout.contains(&p7),
        "signed adherence shown by CID;\n{}",
        out.stdout
    );
    assert_eq!(numbered_count(&out.stdout), 2, "{}", out.stdout);
    assert_candidate(out, 1, "github:BurntSushi", SEMANTIC_VERSIONING);
    assert_candidate(out, 2, "github:BurntSushi", TEST_DRIVEN);
}

/// SP-2 (guardrail, DDD-13): `scrape github BurntSushi --sign 1` signs EXACTLY
/// what `infer people --person github:BurntSushi --sign 1` signs — proven by
/// identical CIDs from two identically seeded stores at a pinned instant.
///
/// @us-cpi-005 @us-cpi-003 @driving_port @real-io @ddd-13 @guardrail
#[test]
#[ignore = "DELIVER slice-05: unskip one-at-a-time (US-CPI-005 person-view --sign == infer people --person --sign)"]
fn signing_from_the_person_view_is_the_same_as_signing_from_infer_people() {
    // GIVEN two identically seeded stores (all instants pinned).
    let via_scrape = TestEnv::initialized();
    let via_infer = TestEnv::initialized();
    given_burntsushi_person_picture(&via_scrape, Some(T1), Some(T1));
    given_burntsushi_person_picture(&via_infer, Some(T1), Some(T1));
    let before_scrape = claim_files(&via_scrape);
    let before_infer = claim_files(&via_infer);

    // WHEN Maria signs candidate 1 from each surface at T2.
    let scraped = scrape_github(
        &via_scrape,
        &["BurntSushi", "--sign", "1"],
        FakeGithub::for_public_user("BurntSushi"),
        Some(T2),
        &sign_stdin("", false),
    );
    let inferred = infer_people_with(
        &via_infer,
        &["--person", "github:BurntSushi", "--sign", "1"],
        &sign_stdin("", false),
        Some(T2),
    );

    // THEN both produced the same claim (same CID).
    assert_eq!(
        scraped.outcome.status, 0,
        "{}\n{}",
        scraped.outcome.stdout, scraped.outcome.stderr
    );
    assert_eq!(
        inferred.status, 0,
        "{}\n{}",
        inferred.stdout, inferred.stderr
    );
    let a = new_claim_cids(&before_scrape, &via_scrape);
    let b = new_claim_cids(&before_infer, &via_infer);
    assert_eq!(a.len(), 1, "{a:?}");
    assert_eq!(
        a, b,
        "the person view must sign exactly what infer people signs"
    );
}

/// SP-3 (edge): a person with no links gets guidance, not an error — exit 0.
///
/// @us-cpi-005 @driving_port @real-io @edge
#[test]
#[ignore = "DELIVER slice-05: unskip one-at-a-time (US-CPI-005 unknown person -> guidance, exit 0; narrow SG-3)"]
fn a_person_with_no_links_gets_guidance_not_an_error() {
    // GIVEN no scraped repo is linked to octocat.
    let env = TestEnv::initialized();
    given_repo_scraped(&env, "BurntSushi/ripgrep", burntsushi_only(), None);

    // WHEN Maria scrapes octocat.
    let scrape = scrape_github(
        &env,
        &["octocat"],
        FakeGithub::for_public_user("octocat"),
        None,
        "",
    );

    // THEN guidance to scrape their repos first; exit 0.
    let out = &scrape.outcome;
    assert_eq!(out.status, 0, "{}\n{}", out.stdout, out.stderr);
    assert!(
        out.stdout
            .contains("github:octocat is not linked to any repo you've scraped"),
        "{}",
        out.stdout
    );
    assert!(out.stdout.contains("scrape"), "{}", out.stdout);
}

/// SP-4 (error): a non-existent user still fails clearly — non-zero, the
/// target and the not-found cause named, nothing written. (GREEN-today
/// regression guard of the shipped behavior; kept ignored until slice-05 so
/// the person-view refactor is proven not to regress it.)
///
/// @us-cpi-005 @driving_port @real-io @error
#[test]
#[ignore = "DELIVER slice-05: unskip one-at-a-time (US-CPI-005 non-existent user still fails clearly)"]
fn a_non_existent_user_still_fails_clearly() {
    // GIVEN ghost-user-zz9 does not exist on GitHub.
    let env = TestEnv::initialized();
    let before = capture_store_universe(&env);

    // WHEN Maria scrapes it.
    let scrape = scrape_github(
        &env,
        &["ghost-user-zz9"],
        FakeGithub::for_not_found("ghost-user-zz9"),
        None,
        "",
    );

    // THEN non-zero, target + cause named, nothing written.
    assert_ne!(scrape.outcome.status, 0, "{}", scrape.outcome.stdout);
    assert!(
        scrape.outcome.stderr.contains("ghost-user-zz9"),
        "{}",
        scrape.outcome.stderr
    );
    assert!(
        scrape.outcome.stderr.contains("not found"),
        "{}",
        scrape.outcome.stderr
    );
    assert_store_unchanged(&before, &env);
}

/// SP-5 (guardrail, D-4 / D-6; slice-05 AC "exactly one GitHub request"):
/// reading a person asks GitHub for the person's public profile EXACTLY ONCE —
/// none of their repositories is crawled. (The shipped user scrape fetches
/// `/users/{user}` twice — resolve + harvest; DELIVER collapses that to one
/// read, taking the auth/rate report from the single response.)
///
/// @us-cpi-005 @driving_port @real-io @d-4 @d-6 @guardrail
#[test]
#[ignore = "DELIVER slice-05: unskip one-at-a-time (D-4 person view reads only the public profile, exactly once)"]
fn reading_a_person_asks_github_only_for_their_profile_once() {
    // GIVEN BurntSushi's picture in Maria's store.
    let env = TestEnv::initialized();
    given_burntsushi_person_picture(&env, None, None);

    // WHEN Maria scrapes the person.
    let scrape = scrape_github(
        &env,
        &["BurntSushi"],
        FakeGithub::for_public_user("BurntSushi"),
        None,
        "",
    );

    // THEN exactly one request — the public profile — and no repo read.
    assert_eq!(
        scrape.outcome.status, 0,
        "{}\n{}",
        scrape.outcome.stdout, scrape.outcome.stderr
    );
    assert_eq!(
        scrape.seen_paths,
        vec!["/users/BurntSushi".to_string()],
        "exactly one GitHub request, for the public profile only"
    );
}

/// SP-6 (edge, Q-CPI-D7): two logins sharing one GitHub user id (a rename)
/// are shown as "possible rename" — never merged into one person.
///
/// @us-cpi-005 @driving_port @real-io @q-cpi-d7 @od-cpi-1 @edge
#[test]
#[ignore = "DELIVER slice-05: unskip one-at-a-time (Q-CPI-D7 possible rename shown, never merged)"]
fn a_renamed_login_is_flagged_as_a_possible_rename_never_merged() {
    // GIVEN old-login (id 42) recorded on acme/old, new-login (id 42) on acme/new.
    let env = TestEnv::initialized();
    given_repo_scraped(
        &env,
        "acme/old",
        vec![FakeContributor::human("old-login", 42, 10)],
        None,
    );
    given_repo_scraped(
        &env,
        "acme/new",
        vec![FakeContributor::human("new-login", 42, 12)],
        None,
    );

    // WHEN Maria reads new-login.
    let scrape = scrape_github(
        &env,
        &["new-login"],
        FakeGithub::for_public_user("new-login"),
        None,
        "",
    );

    // THEN possible rename to github:old-login is shown; both stay distinct.
    let out = &scrape.outcome;
    assert_eq!(out.status, 0, "{}\n{}", out.stdout, out.stderr);
    assert!(out.stdout.contains("possible rename"), "{}", out.stdout);
    assert!(out.stdout.contains("github:old-login"), "{}", out.stdout);
    assert_eq!(
        links_for(&env, "github:acme/old")[0].person,
        "github:old-login",
        "never merged"
    );
    assert_eq!(
        links_for(&env, "github:acme/new")[0].person,
        "github:new-login",
        "never merged"
    );
}

/// SP-7 (edge): a linked person whose repos carry no signed philosophy
/// claims shows the links, "No inferred candidates", and how to enable
/// inference.
///
/// @us-cpi-005 @driving_port @real-io @edge
#[test]
#[ignore = "DELIVER slice-05: unskip one-at-a-time (US-CPI-005 linked person without signed repo claims)"]
fn a_linked_person_without_signed_repo_claims_shows_links_and_how_to_enable_inference() {
    // GIVEN dtolnay is linked to serde, which carries no signed claims.
    let env = TestEnv::initialized();
    given_repo_scraped(&env, "dtolnay/serde", serde_contributors(), None);

    // WHEN Maria reads dtolnay.
    let scrape = scrape_github(
        &env,
        &["dtolnay"],
        FakeGithub::for_public_user("dtolnay"),
        None,
        "",
    );

    // THEN serde with rank, no candidates, a pointer to signing repo claims.
    let out = &scrape.outcome;
    assert_eq!(out.status, 0, "{}\n{}", out.stdout, out.stderr);
    assert!(
        out.stdout
            .lines()
            .any(|l| l.contains("dtolnay/serde") && l.contains("#1")),
        "{}",
        out.stdout
    );
    assert!(
        out.stdout.contains("No inferred candidates"),
        "{}",
        out.stdout
    );
    assert!(
        out.stdout.contains("no signed philosophy claims"),
        "{}",
        out.stdout
    );
}

/// SP-8 (error): `--sign 7` on a person view with two candidates is rejected
/// before composing, exactly like `infer people`.
///
/// @us-cpi-005 @driving_port @real-io @error
#[test]
#[ignore = "DELIVER slice-05: unskip one-at-a-time (US-CPI-005 out-of-range --sign on the person view)"]
fn selecting_a_missing_candidate_from_the_person_view_is_rejected() {
    // GIVEN BurntSushi has two inferred candidates.
    let env = TestEnv::initialized();
    given_burntsushi_person_picture(&env, None, None);
    let before = capture_store_universe(&env);

    // WHEN Maria selects candidate 7.
    let scrape = scrape_github(
        &env,
        &["BurntSushi", "--sign", "7"],
        FakeGithub::for_public_user("BurntSushi"),
        None,
        &sign_stdin("", false),
    );

    // THEN rejected naming the valid range; nothing composed or written.
    assert_ne!(scrape.outcome.status, 0, "{}", scrape.outcome.stdout);
    assert!(
        scrape
            .outcome
            .stderr
            .contains("candidate 7 does not exist; valid range 1..2"),
        "{}",
        scrape.outcome.stderr
    );
    assert_store_unchanged(&before, &env);
}

/// SP-9 (production data, live GitHub — slice-05 brief): after scraping the
/// real ripgrep and regex, `scrape github BurntSushi` lists both with his
/// rank, answering "which philosophies, based on what" from one command.
///
/// @us-cpi-005 @real-io @requires_external @live-github
#[test]
#[ignore = "live-github: network-dependent production-data check; run with OPENLORE_LIVE_GITHUB=1 cargo test -p cli --test scrape_person -- --ignored live"]
fn live_reading_burntsushi_lists_his_real_scraped_repos() {
    if std::env::var("OPENLORE_LIVE_GITHUB").is_err() {
        eprintln!("skipped: set OPENLORE_LIVE_GITHUB=1 to run against api.github.com");
        return;
    }
    let env = TestEnv::initialized();
    for repo in ["BurntSushi/ripgrep", "rust-lang/regex"] {
        let scraped = openlore_with(&env, &["scrape", "github", repo], Invocation::default());
        assert_eq!(scraped.status, 0, "{}\n{}", scraped.stdout, scraped.stderr);
    }
    let person = openlore_with(
        &env,
        &["scrape", "github", "BurntSushi"],
        Invocation::default(),
    );
    assert_eq!(person.status, 0, "{}\n{}", person.stdout, person.stderr);
    for repo in ["ripgrep", "regex"] {
        assert!(
            person
                .stdout
                .lines()
                .any(|l| l.contains(repo) && l.contains('#')),
            "{}",
            person.stdout
        );
    }
}
