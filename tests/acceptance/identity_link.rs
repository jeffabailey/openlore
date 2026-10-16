//! github-did-identity-link — slice 1 acceptance + the feature's WALKING
//! SKELETON: I prove my GitHub account is mine, sign and publish the link,
//! re-check it, and erase my local proof history (ADR-084, ADR-085 §2/§4/§7).
//!
//! No DISCUSS wave: acceptance criteria are derived from the accepted DESIGN
//! (GDL-D1..D14, C-GDL-1..10, A-1..A-10) and the brief's "For Acceptance
//! Designer" list, plus the decisions D-a..D-g recorded in
//! `docs/feature/github-did-identity-link/distill/wave-decisions.md`.
//!
//! Walking-skeleton strategy C: REAL `openlore` binary, `adapter-github` over
//! HTTP, `adapter-duckdb`, filesystem; FAKE (driven-external only)
//! `FakeGithubAccounts`, `FakePds`, `FakeIdentity`, `PeerPds`.
//!
//! Layer 3/4 subprocess — example-only (Mandate 9); sad paths are named
//! examples, table-driven where the shape is shared (Mandate 11). PRIMARY Thens
//! observe through driving ports (CLI output/exit, `identity list`, the PDS wire
//! JSON); the store-universe delta is a secondary guardrail (D-e). Every
//! scenario is `#[ignore]`d at hand-off, the walking skeleton included (D-d).
//!
//! Lifecycle of one link, as seen by me (C2a):
//!
//! ```text
//!   (no link) --link, bio names my DID--> Active+Verified --verify, bio lost--> Active+Disproved
//!   (no link) --link, bio lacks DID-----> (no link)          Active+Disproved --verify, bio back--> Active+Verified
//!   Active+*  --link the same account---> refused "already linked" (D-a)
//!   Active+*  --claim retract-----------> Retracted (never shown, never checked)
//!   Retracted --link the same account---> a NEW Active link, fresh record ID (D-g)
//!   Active+Verified --GitHub rename-----> Active+Verified (login seen = new login)
//!   any --GitHub rate limit/outage/401--> unchanged (nothing recorded)
//!   any --identity forget --did---------> Active+NotChecked (local history erased)
//! ```

mod support;

#[allow(unused_imports)]
use support::identity_links::*;
#[allow(unused_imports)]
use support::people::*;
#[allow(unused_imports)]
use support::*;

use openlore_test_support::OldLogin;

// =============================================================================
// WALKING SKELETON
// =============================================================================

/// WS-GDL-1
/// ```gherkin
/// @walking_skeleton @driving_port @driving_adapter @real-io @adapter-integration
/// @gdl-d1 @gdl-d2 @gdl-d4 @gdl-d6 @gdl-d10 @kpi-gdl-1 @contract-shape:bounded-change
/// Scenario: Jeff's GitHub account is publicly linked to his DID and shows as verified
///   Given Jeff's GitHub bio for "jeffbailey" names his DID
///   When Jeff links github "jeffbailey" and confirms
///   Then his published repo holds the link "did:plc:test-jeff sameAs github:jeffbailey" under the record ID he was shown
///   And it carries his profile page and his account number as evidence
///   And the publish message names the command that would retract it
///   And his linked identities show him as github:jeffbailey, verified today
/// ```
#[test]
fn jeffs_github_account_is_publicly_linked_to_his_did_and_shows_as_verified() {
    let world = LinkWorld::with(&[JEFF_GH]);
    bio_names(&world, JEFF_GH, MY_DID);
    let before = capture_link_universe(&world.env);

    let linked = when_i_link(&world, JEFF_GH);

    // PRIMARY (ports): the message, the wire record, my linked identities.
    let cid = assert_link_published(&linked);
    assert_link_on_pds(&world.env, &cid, MY_DID, JEFF_GH);
    let listed = when_i_list(&world, None);
    assert_eq!(listed.status, 0, "{}\n{}", listed.stdout, listed.stderr);
    assert_shown(&listed, MY_DID, JEFF_GH, Shown::Associated);
    assert_check_age(&listed, MY_DID, JEFF_GH, "today", false);

    // GUARDRAIL: one claim, one file, one PDS record, one check — nothing else.
    assert_link_store_delta(
        &before,
        &world.env,
        &[
            ("local.claims.row_count", claims_plus(&before, 1)),
            ("local.claim_files", claim_file_slot(&world.env)),
            ("pds.records.len", pds_plus(&before, 1)),
            (
                "local.identity_link_checks",
                checks_with(&[], &[CheckRow::of(MY_DID, JEFF_GH, Verdict::Verified)]),
            ),
        ],
    );
}

// =============================================================================
// Linking
// =============================================================================

/// IL-2
/// ```gherkin
/// @driving_port @real-io @gdl-d12 @kpi-gdl-1 @guardrail @contract-shape:unbounded-preservation
/// Scenario: Jeff sees exactly what a link would publish before anything is written
///   Given Jeff's GitHub bio names his DID
///   When Jeff starts linking github "jeffbailey" but does not confirm
///   Then he sees the exact record, that it is public, and that retracting withdraws it but does not erase it
///   And his published repo and his linked identities are unchanged
/// ```
#[test]
fn jeff_sees_exactly_what_a_link_would_publish_before_anything_is_written() {
    let world = LinkWorld::with(&[JEFF_GH]);
    bio_names(&world, JEFF_GH, MY_DID);
    let before = capture_link_universe(&world.env);

    let previewed = when_i_preview_a_link(&world, JEFF_GH);

    assert_link_preview(&previewed, MY_DID, JEFF_GH);
    assert!(world.env.pds.records().is_empty(), "nothing published");
    assert_shown(&when_i_list(&world, None), MY_DID, JEFF_GH, Shown::Nothing);
    assert_link_store_unchanged(&before, &world.env);
}

/// IL-3 (error, table)
/// ```gherkin
/// @driving_port @real-io @error @gdl-d4 @adr-076 @kpi-gdl-1 @contract-shape:unbounded-preservation
/// Scenario Outline: No link is made unless the bio names Jeff's exact DID
///   Given the GitHub account "jeffbailey" <bio state>
///   When Jeff links github "jeffbailey" and confirms
///   Then he is told it is not linked because <reason>
///   And nothing is published and his linked identities are unchanged
///   Examples:
///     | bio state                                 | reason                               |
///     | has no bio                                | the account has no bio               |
///     | has a bio of only spaces                  | the account has no bio               |
///     | names no DID                              | the bio names no DID                 |
///     | names someone else's DID                  | the bio names someone else's DID     |
///     | names a DID that merely starts with his   | the bio names someone else's DID     |
///     | names his DID in capital letters          | the bio names no DID                 |
///     | does not exist on GitHub                  | the account no longer exists         |
/// ```
#[test]
fn no_link_is_made_unless_the_bio_names_jeffs_exact_did() {
    let upper = MY_DID.to_ascii_uppercase();
    let rows: [(&str, Option<&str>, Verdict); 6] = [
        ("no bio", None, Verdict::NoBio),
        ("blank bio", Some("   "), Verdict::NoBio),
        (
            "no DID",
            Some("Builder of small tools."),
            Verdict::DidMissing,
        ),
        (
            "someone else's DID",
            Some(STRANGER_DID),
            Verdict::DifferentDid,
        ),
        (
            "prefix collider",
            Some(MY_DID_PREFIX_COLLIDER),
            Verdict::DifferentDid,
        ),
        ("capitalised DID", Some(upper.as_str()), Verdict::DidMissing),
    ];
    for (row, bio, verdict) in rows {
        let world = LinkWorld::with(&[JEFF_GH]);
        bio_is(&world, JEFF_GH, bio);
        let before = capture_link_universe(&world.env);

        let attempt = when_i_link(&world, JEFF_GH);

        assert_link_refused(&attempt, verdict, row);
        assert_shown_in(
            row,
            &when_i_list(&world, None),
            MY_DID,
            JEFF_GH,
            Shown::Nothing,
        );
        assert_link_store_unchanged_in(row, &before, &world.env);
    }

    let world = LinkWorld::with(&[]);
    let before = capture_link_universe(&world.env);
    let attempt = when_i_link(&world, JEFF_GH);
    assert_link_refused(&attempt, Verdict::AccountNotFound, "account does not exist");
    assert_link_store_unchanged_in("account does not exist", &before, &world.env);
}

/// IL-4 (edge, table)
/// ```gherkin
/// @driving_port @real-io @edge @adr-076 @contract-shape:bounded-change
/// Scenario Outline: Jeff's DID proves the account wherever it stands in his bio
///   Given the GitHub bio of "jeffbailey" is "<bio>"
///   When Jeff links github "jeffbailey" and confirms
///   Then the link is in his published repo
///   Examples:
///     | bio                                              |
///     | (did:plc:test-jeff).                             |
///     | did:plc:stranger-test did:plc:test-jeff          |
///     | did:plc:test-jeff                                |
///     | Rust, tools; my DID: did:plc:test-jeff, say hi!  |
/// ```
#[test]
fn jeffs_did_proves_the_account_wherever_it_stands_in_his_bio() {
    let bios = [
        ("in parentheses", format!("({MY_DID}).")),
        ("after another DID", format!("{STRANGER_DID} {MY_DID}")),
        ("alone", MY_DID.to_string()),
        (
            "in a sentence",
            format!("Rust, tools; my DID: {MY_DID}, say hi!"),
        ),
    ];
    for (row, bio) in bios {
        let world = LinkWorld::with(&[JEFF_GH]);
        bio_is(&world, JEFF_GH, Some(&bio));

        let linked = when_i_link(&world, JEFF_GH);

        assert_eq!(
            linked.status, 0,
            "[{row}] {}\n{}",
            linked.stdout, linked.stderr
        );
        let cid = assert_link_published(&linked);
        assert_link_on_pds(&world.env, &cid, MY_DID, JEFF_GH);
    }
}

/// IL-5 (error, decision D-a)
/// ```gherkin
/// @driving_port @real-io @error @d-a @c2b @c4a @contract-shape:unbounded-preservation
/// Scenario: Linking an account Jeff has already linked is refused and points to re-checking
///   Given Jeff linked github "jeffbailey"
///   When Jeff links github "jeffbailey" again and confirms
///   Then he is told it is already linked and to use "openlore identity verify"
///   And nothing new is signed or published
/// ```
#[test]
#[ignore = "DISTILL github-did-identity-link: pending DELIVER (one at a time)"]
fn linking_an_account_jeff_has_already_linked_is_refused_and_points_to_rechecking() {
    let world = LinkWorld::with(&[JEFF_GH]);
    given_i_linked(&world, JEFF_GH, None);
    let pds_before = world.env.pds.records().len();
    let before = capture_link_universe(&world.env);

    let again = when_i_link(&world, JEFF_GH);

    assert_already_linked(&again);
    assert_eq!(
        world.env.pds.records().len(),
        pds_before,
        "nothing published"
    );
    assert_link_store_unchanged(&before, &world.env);
}

/// IL-6 (error, infrastructure-failure, table)
/// ```gherkin
/// @driving_port @real-io @error @infrastructure-failure @adr-085-earned-trust @contract-shape:unbounded-preservation
/// Scenario Outline: When GitHub cannot be read, nothing is linked and nothing is recorded
///   Given Jeff's GitHub bio names his DID
///   And GitHub <trouble>
///   When Jeff links github "jeffbailey" and confirms
///   Then he is told GitHub could not be read and nothing was recorded, and his token is never shown
///   And nothing is published and his linked identities are unchanged
///   Examples:
///     | trouble                                         |
///     | is rate-limiting anonymous requests             |
///     | is unreachable                                  |
///     | rejects his GITHUB_TOKEN                        |
/// ```
#[test]
fn when_github_cannot_be_read_nothing_is_linked_and_nothing_is_recorded() {
    for trouble in GithubTrouble::ALL {
        let row = format!("{trouble:?}");
        let world = LinkWorld::with(&[JEFF_GH]);
        bio_names(&world, JEFF_GH, MY_DID);
        github_has(&world, trouble);
        let before = capture_link_universe(&world.env);

        let attempt = when_i_link(&world, JEFF_GH);

        assert_github_unavailable(&attempt, &row);
        assert!(
            world.env.pds.records().is_empty(),
            "[{row}] nothing published"
        );
        assert_shown_in(
            &row,
            &when_i_list(&world, None),
            MY_DID,
            JEFF_GH,
            Shown::Nothing,
        );
        assert_link_store_unchanged_in(&row, &before, &world.env);
    }
}

/// IL-7 (error, table)
/// ```gherkin
/// @driving_port @real-io @error @adr-076 @contract-shape:unbounded-preservation
/// Scenario Outline: A malformed GitHub username is refused before GitHub is asked
///   Given GitHub serves Jeff's account "jeffbailey" and Jeff has no links
///   When Jeff links github "<username>"
///   Then he is told "<username>" is not a GitHub username
///   And GitHub was never asked and nothing changed
///   Examples:
///     | username                                  |
///     | -jeff                                     |
///     | jeff/../admin                             |
///     | jeff bailey                               |
///     | a-forty-character-username-is-too-long-x  |
/// ```
#[test]
fn a_malformed_github_username_is_refused_before_github_is_asked() {
    let too_long = "a".repeat(40);
    for username in ["-jeff", "jeff/../admin", "jeff bailey", too_long.as_str()] {
        let world = LinkWorld::with(&[JEFF_GH]);
        let before = capture_link_universe(&world.env);

        let attempt = identity(&world, &["link", "github", username, "--yes"], "", None);

        assert_eq!(attempt.status, 1, "[{username:?}] refused with exit 1");
        assert!(
            format!("{}{}", attempt.stdout, attempt.stderr).contains("not a GitHub username"),
            "[{username:?}] must say `not a GitHub username`;\n{}\n{}",
            attempt.stdout,
            attempt.stderr
        );
        assert!(
            world.github_paths().is_empty(),
            "[{username:?}] GitHub must not be asked: {:?}",
            world.github_paths()
        );
        assert_link_store_unchanged_in(username, &before, &world.env);
    }
}

/// IL-8a (error, guardrail)
/// ```gherkin
/// @driving_port @real-io @error @guardrail @n-1 @c-gdl-2 @kpi-gdl-1 @contract-shape:unbounded-preservation
/// Scenario: A hand-written "sameAs" claim is refused and Jeff is pointed to the link command
///   Given Jeff's GitHub bio for "jeffbailey" names his DID
///   When Jeff adds a claim "did:plc:test-jeff sameAs github:jeffbailey" by hand and confirms publishing
///   Then it is refused and he is told to use "openlore identity link"
///   And nothing that readers would treat as a link is in his published repo
/// ```
#[test]
#[ignore = "DISTILL github-did-identity-link: pending DELIVER (one at a time)"]
fn a_hand_written_same_as_claim_is_refused_and_jeff_is_pointed_to_the_link_command() {
    let world = LinkWorld::with(&[JEFF_GH]);
    bio_names(&world, JEFF_GH, MY_DID);

    let attempt = when_i_add_a_same_as_claim(&world);

    assert_eq!(
        attempt.status, 1,
        "refused with exit 1;\n{}",
        attempt.stderr
    );
    assert_points_to_identity_link(&attempt, "claim add");
    assert_no_link_on_pds(&world.env, "claim add");
    assert_shown(&when_i_list(&world, None), MY_DID, JEFF_GH, Shown::Nothing);
}

/// IL-8b (error, guardrail; split from IL-8, L-1)
/// ```gherkin
/// @driving_port @real-io @error @guardrail @n-1 @c-gdl-2 @kpi-gdl-1 @contract-shape:unbounded-preservation
/// Scenario: Editing an inferred claim into "sameAs" is refused at signing
///   Given Jeff's store infers that github "BurntSushi" adheres to memory-safety
///   When Jeff signs that inference with the predicate changed to "sameAs"
///   Then it is refused and he is told to use "openlore identity link"
///   And his linked identities show no link, and nothing that readers would treat as a link is in his published repo
/// ```
#[test]
#[ignore = "DISTILL github-did-identity-link: pending DELIVER (one at a time)"]
fn editing_an_inferred_claim_into_same_as_is_refused_at_signing() {
    let world = LinkWorld::with(&[JEFF_GH]);
    given_burntsushi_builds_two_signed_memory_safe_repos(&world.env);

    let attempt = when_i_sign_an_inference_as_same_as(&world);

    assert_eq!(
        attempt.status, 1,
        "[infer people --sign] refused with exit 1;\n{}",
        attempt.stderr
    );
    assert_points_to_identity_link(&attempt, "infer people --sign");
    let listed = when_i_list(&world, None);
    assert!(
        link_lines(&listed.stdout).is_empty(),
        "[infer people --sign] no link is shown;\n{}",
        listed.stdout
    );
    assert_no_link_on_pds(&world.env, "infer people --sign");
}

/// IL-8c (edge, decision D-b — user-ratified 2026-10-14)
/// ```gherkin
/// @driving_port @real-io @edge @d-b @oos-d1 @gdl-d3 @contract-shape:bounded-change
/// Scenario: Jeff can publicly dispute Rachel's link, and his dispute is never read as a link
///   Given Jeff follows Rachel, who published a link to github "rachelk"
///   When Jeff counters Rachel's link with a reason and publishes the counter
///   Then his published repo holds the counter, authored by Jeff about Rachel's DID, pointing at her link
///   And his linked identities still show Rachel's claim as not checked, and no link for Jeff
/// ```
#[test]
#[ignore = "DISTILL github-did-identity-link: pending DELIVER (one at a time)"]
fn jeff_can_publicly_dispute_rachels_link_and_his_dispute_is_never_read_as_a_link() {
    let mut world = LinkWorld::with(&[RACHEL_GH]);
    let cids = given_peer_published(
        &mut world,
        Peer::Rachel,
        vec![link_spec(RACHEL_DID, RACHEL_GH)],
    );

    let countered = when_i_counter_and_publish(&world, &cids[0]);

    assert_eq!(
        countered.status, 0,
        "{}\n{}",
        countered.stdout, countered.stderr
    );
    let bodies = pds_bodies(&world.env);
    assert_eq!(
        bodies.len(),
        1,
        "exactly the counter is published: {bodies:?}"
    );
    let (_, counter) = &bodies[0];
    assert_eq!(
        counter["subject"], RACHEL_DID,
        "the counter mirrors Rachel's subject"
    );
    let author = bare(counter["author"].as_str().unwrap_or_default());
    assert_eq!(author, MY_DID, "the counter is Jeff's");
    assert_ne!(
        author, RACHEL_DID,
        "author != subject, so it is never a link (GDL-D3)"
    );
    assert!(
        counter["references"].as_array().is_some_and(|r| r
            .iter()
            .any(|x| x["type"] == "counters" && x["cid"] == cids[0].as_str())),
        "the counter points at Rachel's link: {counter}"
    );
    assert_no_link_on_pds(&world.env, "counter");
    let listed = when_i_list(&world, None);
    assert_shown(&listed, RACHEL_DID, RACHEL_GH, Shown::NotChecked);
    assert_shown(&listed, MY_DID, RACHEL_GH, Shown::Nothing);
}

/// IL-9a (error, interruption)
/// ```gherkin
/// @driving_port @real-io @error @infrastructure-failure @c7b @adr-085-7 @contract-shape:bounded-change
/// Scenario: When the host of Jeff's published repo is unreachable, his link is kept signed and verified locally
///   Given Jeff's GitHub bio names his DID
///   And the host of his published repo is unreachable
///   When Jeff links github "jeffbailey" and confirms
///   Then he is told his repo host could not be reached and to retry with "openlore claim publish"
///   And his linked identities show him as github:jeffbailey, verified today
///   And nothing is in his published repo
/// ```
#[test]
#[ignore = "DISTILL github-did-identity-link: pending DELIVER (one at a time)"]
fn when_the_host_of_jeffs_published_repo_is_unreachable_his_link_is_kept_signed_and_verified_locally(
) {
    let mut world = LinkWorld::with(&[JEFF_GH]);
    bio_names(&world, JEFF_GH, MY_DID);
    world.env.pds.simulate_unreachable();
    let before = capture_link_universe(&world.env);

    let attempt = when_i_link(&world, JEFF_GH);

    // Q-GDL-D1 pin: the identity verb words the shared publish failure as
    // "repo host" (WS-10's own `claim add` wording is unaffected).
    assert_exit_nonzero_and_stderr_contains(&attempt, "repo host");
    assert_exit_nonzero_and_stderr_contains(&attempt, "retry with `openlore claim publish");
    let listed = when_i_list(&world, None);
    assert_shown(&listed, MY_DID, JEFF_GH, Shown::Associated);
    assert_check_age(&listed, MY_DID, JEFF_GH, "today", false);
    assert!(
        world.env.pds.records().is_empty(),
        "nothing in his published repo"
    );
    assert_link_store_delta(
        &before,
        &world.env,
        &[
            ("local.claims.row_count", claims_plus(&before, 1)),
            ("local.claim_files", claim_file_slot(&world.env)),
            (
                "local.identity_link_checks",
                checks_with(&[], &[CheckRow::of(MY_DID, JEFF_GH, Verdict::Verified)]),
            ),
        ],
    );
}

/// IL-9b (happy, chained on IL-9a)
/// ```gherkin
/// @driving_port @real-io @happy @c7b @contract-shape:bounded-change
/// Scenario: A link kept locally reaches Jeff's published repo when he publishes it later
///   Given Jeff's link was kept locally because his repo host was unreachable
///   And his repo host is reachable again
///   When Jeff publishes the link's record ID
///   Then his published repo holds the link under that record ID
/// ```
#[test]
#[ignore = "DISTILL github-did-identity-link: pending DELIVER (one at a time)"]
fn a_link_kept_locally_reaches_jeffs_published_repo_when_he_publishes_it_later() {
    let mut world = LinkWorld::with(&[JEFF_GH]);
    bio_names(&world, JEFF_GH, MY_DID);
    world.env.pds.simulate_unreachable();
    let files_before = claim_files(&world.env);
    when_i_link(&world, JEFF_GH);
    let cid = new_claim_cids(&files_before, &world.env)
        .pop()
        .expect("precondition: the link is signed locally");
    world.env.pds.restore();

    let published = openlore_with(
        &world.env,
        &["claim", "publish", &cid],
        Invocation::default(),
    );

    assert_eq!(
        published.status, 0,
        "{}\n{}",
        published.stdout, published.stderr
    );
    assert_link_on_pds(&world.env, &cid, MY_DID, JEFF_GH);
}

/// IL-10 (happy, F1)
/// ```gherkin
/// @driving_port @real-io @happy @a-4 @f1 @contract-shape:bounded-change
/// Scenario: Jeff links a second GitHub account and both show as his
///   Given Jeff linked github "jeffbailey"
///   And the bio of his work account "jeff-at-work" names his DID
///   When Jeff links github "jeff-at-work" and confirms
///   Then his published repo holds the second link
///   And his linked identities show him as github:jeffbailey and as github:jeff-at-work, both verified
/// ```
#[test]
#[ignore = "DISTILL github-did-identity-link: pending DELIVER (one at a time)"]
fn jeff_links_a_second_github_account_and_both_show_as_his() {
    let world = LinkWorld::with(&[JEFF_GH, JEFF_WORK_GH]);
    given_i_linked(&world, JEFF_GH, None);
    bio_names(&world, JEFF_WORK_GH, MY_DID);

    let linked = when_i_link(&world, JEFF_WORK_GH);

    let cid = assert_link_published(&linked);
    assert_link_on_pds(&world.env, &cid, MY_DID, JEFF_WORK_GH);
    let listed = when_i_list(&world, None);
    assert_shown(&listed, MY_DID, JEFF_GH, Shown::Associated);
    assert_shown(&listed, MY_DID, JEFF_WORK_GH, Shown::Associated);
}

/// IL-11 (guardrail, platform)
/// ```gherkin
/// @driving_port @real-io @guardrail @od-cpi-2 @uc-7 @contract-shape:unbounded-preservation
/// Scenario: Linking and re-checking leave the contributors Jeff recorded untouched
///   Given Jeff scraped "BurntSushi/ripgrep", and BurntSushi's person view lists ripgrep with his rank
///   And Jeff linked github "jeffbailey"
///   When Jeff re-checks his links
///   Then the re-check succeeds
///   And BurntSushi's person view lists exactly the same repos and ranks as before
/// ```
#[test]
#[ignore = "DISTILL github-did-identity-link: pending DELIVER (one at a time)"]
fn linking_and_rechecking_leave_the_contributors_jeff_recorded_untouched() {
    let world = LinkWorld::with(&[JEFF_GH, BURNTSUSHI_GH]);
    given_repo_scraped(
        &world.env,
        "BurntSushi/ripgrep",
        ripgrep_contributors(),
        None,
    );
    let repo_lines = |w: &LinkWorld| {
        let view = when_i_read_the_person(w, BURNTSUSHI_GH);
        assert_eq!(
            view.status, 0,
            "person view;\n{}\n{}",
            view.stdout, view.stderr
        );
        view.stdout
            .lines()
            .filter(|l| l.contains("ripgrep"))
            .map(str::to_string)
            .collect::<Vec<_>>()
    };
    let before_lines = repo_lines(&world);
    assert!(
        before_lines.iter().any(|l| l.contains("#1")),
        "precondition: the person view lists ripgrep with rank #1: {before_lines:?}"
    );
    given_i_linked(&world, JEFF_GH, None);
    let contributors_before = contribution_links(&world.env);

    let checked = when_i_verify(&world, &[]);

    assert_eq!(checked.status, 0, "{}\n{}", checked.stdout, checked.stderr);
    assert_eq!(
        repo_lines(&world),
        before_lines,
        "the person view is unchanged"
    );
    // GUARDRAIL: the recorded rows themselves.
    assert_eq!(contribution_links(&world.env), contributors_before);
}

/// IL-12 (guardrail, F2, table)
/// ```gherkin
/// @driving_port @real-io @guardrail @gdl-d12 @f2 @contract-shape:pure-function
/// Scenario Outline: The identity verbs never repeat Jeff's bio or log his identity
///   Given Jeff's bio names his DID beside a private phrase
///   And <given>
///   When Jeff runs <verb>
///   Then the private phrase appears neither in what the command prints nor in its log output
///   And its log output (stderr, the CLI's log channel) names neither his DID nor his login
///   And no log event is printed among its results (stdout)
///   Examples:
///     | given                     | verb            |
///     | nothing else              | identity link   |
///     | he linked the account     | identity verify |
///     | he linked the account     | identity list   |
/// ```
#[test]
#[ignore = "DISTILL github-did-identity-link: pending DELIVER (one at a time)"]
fn the_identity_verbs_never_repeat_jeffs_bio_or_log_his_identity() {
    for verb in ["link", "verify", "list"] {
        let world = LinkWorld::with(&[JEFF_GH]);
        if verb != "link" {
            given_i_linked(&world, JEFF_GH, None);
        }
        bio_names_with_private_phrase(&world, JEFF_GH, MY_DID);

        let ran = match verb {
            "link" => when_i_link(&world, JEFF_GH),
            "verify" => when_i_verify(&world, &[]),
            _ => when_i_list(&world, None),
        };

        assert_eq!(ran.status, 0, "[{verb}] {}\n{}", ran.stdout, ran.stderr);
        assert_no_link_data_leaked(&ran, MY_DID, JEFF_GH, verb);
    }
}

/// IL-13a (guardrail, platform: v6 → v7 upgrade)
/// ```gherkin
/// @driving_port @real-io @guardrail @adr-085-7 @migration-v7 @contract-shape:bounded-change
/// Scenario: Opening a store made before identity links keeps everything already in it
///   Given Jeff's store is the frozen copy made before identity links existed (schema v6),
///         holding his own claim, Rachel's pulled claim and recorded contributors
///   When Jeff lists linked identities for the first time
///   Then he is told there are no linked identities
///   And his claims and Rachel's claims read exactly as they did before identity links existed
///   And the store records the identity-link schema step exactly once, with every earlier row unchanged
/// ```
#[test]
fn opening_a_store_made_before_identity_links_keeps_everything_already_in_it() {
    let world = LinkWorld::with_frozen_pre_v7_store(&[JEFF_GH]);
    let versions_before = schema_versions(&world.env);
    assert_eq!(
        versions_before.last(),
        Some(&6),
        "precondition: a frozen v6 store"
    );
    let rows_before = prior_rows(&world.env);

    let first = when_i_list(&world, None);

    assert_eq!(first.status, 0, "{}\n{}", first.stdout, first.stderr);
    assert!(
        first.stdout.contains("No linked identities"),
        "{}",
        first.stdout
    );
    let read_back = run_openlore(
        &world.env,
        &["graph", "query", "--object", MEMORY_SAFETY, "--federated"],
    );
    assert_eq!(
        read_back.stdout,
        pre_v7_golden_claims(),
        "prior claims read as before"
    );
    let mut expected_versions = versions_before.clone();
    expected_versions.push(7);
    assert_eq!(
        schema_versions(&world.env),
        expected_versions,
        "v7 applied exactly once"
    );
    assert_eq!(
        prior_rows(&world.env),
        rows_before,
        "earlier rows unchanged"
    );
}

/// IL-13b (guardrail, idempotent reopen; chained on IL-13a)
/// ```gherkin
/// @driving_port @real-io @guardrail @migration-v7 @c4a @contract-shape:unbounded-preservation
/// Scenario: Reopening an upgraded store changes nothing
///   Given Jeff's frozen pre-identity-link store was opened once for linked identities
///   When Jeff lists linked identities again
///   Then he gets the same answer
///   And the identity-link schema step is still recorded once and every row is unchanged
/// ```
#[test]
fn reopening_an_upgraded_store_changes_nothing() {
    let world = LinkWorld::with_frozen_pre_v7_store(&[JEFF_GH]);
    let first = when_i_list(&world, None);
    assert_eq!(
        first.status, 0,
        "precondition: first open;\n{}",
        first.stderr
    );
    let versions = schema_versions(&world.env);
    let rows = prior_rows(&world.env);
    let before = capture_link_universe(&world.env);

    let second = when_i_list(&world, None);

    assert_eq!(second.status, 0, "{}\n{}", second.stdout, second.stderr);
    assert_eq!(second.stdout, first.stdout, "same answer");
    assert_eq!(schema_versions(&world.env), versions, "no second v7 step");
    assert_eq!(prior_rows(&world.env), rows, "rows unchanged");
    assert_link_store_unchanged(&before, &world.env);
}

/// Fixture regenerator for IL-13a/b — NOT a scenario. Writes
/// `tests/fixtures/identity-link-pre-v7-store/` from the CURRENT binary and
/// refuses unless the store's schema head is v6, so it can only be re-run on
/// a commit before DELIVER adds v7. Run:
/// `OPENLORE_REGEN_PRE_V7=1 cargo test -p cli --test identity_link -- --ignored regenerate`.
#[test]
#[ignore = "fixture regenerator: OPENLORE_REGEN_PRE_V7=1, only on a binary whose schema head is v6"]
fn regenerate_the_frozen_pre_v7_store_fixture() {
    if std::env::var("OPENLORE_REGEN_PRE_V7").is_err() {
        eprintln!("skipped: set OPENLORE_REGEN_PRE_V7=1 on a pre-v7 commit");
        return;
    }
    let mut world = LinkWorld::with(&[]);
    given_repo_scraped(
        &world.env,
        "BurntSushi/ripgrep",
        ripgrep_contributors(),
        None,
    );
    given_i_signed_repo_claim(&world.env, "BurntSushi/ripgrep", MEMORY_SAFETY, 0.55, None);
    given_peer_published(
        &mut world,
        Peer::Rachel,
        vec![PeerClaimSpec::repo_claim(
            "rust-lang/regex",
            MEMORY_SAFETY,
            0.60,
        )],
    );
    assert_eq!(
        schema_versions(&world.env).last(),
        Some(&6),
        "the fixture must be made by a binary whose schema head is v6"
    );
    let golden = run_openlore(
        &world.env,
        &["graph", "query", "--object", MEMORY_SAFETY, "--federated"],
    );
    assert_eq!(golden.status, 0, "{}", golden.stderr);
    freeze_pre_v7_store(&world.env, &golden.stdout);

    // Self-check: the frozen copy rebuilds into the same v6 store.
    let rebuilt = LinkWorld::with_frozen_pre_v7_store(&[]);
    assert_eq!(schema_versions(&rebuilt.env), schema_versions(&world.env));
    assert_eq!(prior_rows(&rebuilt.env), prior_rows(&world.env));
    let reread = run_openlore(
        &rebuilt.env,
        &["graph", "query", "--object", MEMORY_SAFETY, "--federated"],
    );
    assert_eq!(
        reread.stdout, golden.stdout,
        "the rebuilt store reads back the same"
    );
}

/// IL-14 (happy, decision D-g — user-ratified 2026-10-14)
/// ```gherkin
/// @driving_port @real-io @happy @d-g @gdl-d5 @contract-shape:bounded-change
/// Scenario: After retracting his link, Jeff can link the same account again
///   Given Jeff linked github "jeffbailey" and then retracted the link
///   When Jeff links github "jeffbailey" again and confirms
///   Then his published repo holds a fresh link with a new record ID
///   And his linked identities show him as github:jeffbailey, verified
/// ```
#[test]
#[ignore = "DISTILL github-did-identity-link: pending DELIVER (one at a time)"]
fn after_retracting_his_link_jeff_can_link_the_same_account_again() {
    let world = LinkWorld::with(&[JEFF_GH]);
    let first = given_i_linked(&world, JEFF_GH, Some(LINKED_AT));
    given_i_retracted(&world.env, &first);

    let again = when_i_link_at(&world, JEFF_GH, Some(TWELVE_DAYS_LATER));

    let fresh = assert_link_published(&again);
    assert_ne!(fresh, first, "a fresh link, not the retracted one");
    assert_link_on_pds(&world.env, &fresh, MY_DID, JEFF_GH);
    assert_shown(
        &when_i_list(&world, Some(TWELVE_DAYS_LATER)),
        MY_DID,
        JEFF_GH,
        Shown::Associated,
    );
}

// =============================================================================
// Re-checking (`identity verify`)
// =============================================================================

/// IV-1 (error, KPI-GDL-3)
/// ```gherkin
/// @driving_port @real-io @error @gdl-d5 @kpi-gdl-3 @c-gdl-6 @contract-shape:bounded-change
/// Scenario: Once Jeff's bio stops naming his DID, his link shows the proof is gone and stays published
///   Given Jeff linked github "jeffbailey"
///   And he then removed his DID from his GitHub bio
///   When Jeff re-checks his links
///   Then he is told the bio names no DID
///   And his linked identities show "did:plc:test-jeff claims github:jeffbailey — proof gone"
///   And the link is still in his published repo, unretracted
/// ```
#[test]
#[ignore = "DISTILL github-did-identity-link: pending DELIVER (one at a time)"]
fn once_jeffs_bio_stops_naming_his_did_his_link_shows_the_proof_gone_and_stays_published() {
    let world = LinkWorld::with(&[JEFF_GH]);
    given_i_linked(&world, JEFF_GH, None);
    bio_drops_the_did(&world, JEFF_GH);
    let pds_before = pds_bodies(&world.env);
    let checks_before = link_checks(&world.env);
    let before = capture_link_universe(&world.env);

    let checked = when_i_verify(&world, &[]);

    assert_eq!(checked.status, 0, "{}\n{}", checked.stdout, checked.stderr);
    assert_verify_reported(&checked, JEFF_GH, Verdict::DidMissing);
    assert_shown(
        &when_i_list(&world, None),
        MY_DID,
        JEFF_GH,
        Shown::ProofGone(Verdict::DidMissing),
    );
    assert_eq!(pds_bodies(&world.env), pds_before, "never auto-retracted");
    assert_link_store_delta(
        &before,
        &world.env,
        &[(
            "local.identity_link_checks",
            checks_with(
                &checks_before,
                &[CheckRow::of(MY_DID, JEFF_GH, Verdict::DidMissing)],
            ),
        )],
    );
}

/// IV-2 (happy, idempotency; chained on IV-1)
/// ```gherkin
/// @driving_port @real-io @happy @gdl-d10 @c4a @contract-shape:bounded-change
/// Scenario: When Jeff's DID is back in his bio, one re-check from one GitHub request restores the link
///   Given Jeff's link showed "proof gone" after a re-check twelve days after linking
///   And he put his DID back in his bio
///   When Jeff re-checks his links thirty days after linking
///   Then GitHub was asked once
///   And his linked identities show him as github:jeffbailey, verified today
/// ```
#[test]
#[ignore = "DISTILL github-did-identity-link: pending DELIVER (one at a time)"]
fn when_jeffs_did_is_back_one_recheck_from_one_github_request_restores_the_link() {
    let world = LinkWorld::with(&[JEFF_GH]);
    given_i_linked(&world, JEFF_GH, Some(LINKED_AT));
    bio_drops_the_did(&world, JEFF_GH);
    given_i_verified(&world, Some(TWELVE_DAYS_LATER));
    bio_names(&world, JEFF_GH, MY_DID);
    let checks_before = link_checks(&world.env);
    let asked_before = world.github_paths().len();
    let before = capture_link_universe(&world.env);

    let checked = when_i_verify_at(&world, &[], Some(THIRTY_DAYS_LATER));

    assert_verify_reported(&checked, JEFF_GH, Verdict::Verified);
    assert_eq!(
        world.github_paths_since(asked_before).len(),
        1,
        "one request"
    );
    let listed = when_i_list(&world, Some(THIRTY_DAYS_LATER));
    assert_shown(&listed, MY_DID, JEFF_GH, Shown::Associated);
    assert_check_age(&listed, MY_DID, JEFF_GH, "today", false);
    // GUARDRAIL: insert-only — the earlier observations are kept.
    assert_link_store_delta(
        &before,
        &world.env,
        &[(
            "local.identity_link_checks",
            checks_with(
                &checks_before,
                &[CheckRow::of(MY_DID, JEFF_GH, Verdict::Verified)],
            ),
        )],
    );
}

/// IV-3a (edge, rename)
/// ```gherkin
/// @driving_port @real-io @adapter-integration @edge @gdl-d9 @a-10 @contract-shape:bounded-change
/// Scenario: A renamed account stays verified under its new name
///   Given Jeff linked github "jeffbailey"
///   And he renamed his GitHub account to "jeff-b", and the old name now answers "not found"
///   When Jeff re-checks his links
///   Then GitHub was asked for "jeffbailey" and then by account number
///   And his linked identities show him as github:jeff-b, verified
/// ```
#[test]
#[ignore = "DISTILL github-did-identity-link: pending DELIVER (one at a time)"]
fn a_renamed_account_stays_verified_under_its_new_name() {
    let world = LinkWorld::with(&[JEFF_GH]);
    given_i_linked(&world, JEFF_GH, None);
    i_renamed_my_account(&world, JEFF_GH, JEFF_RENAMED, OldLogin::NotFound);
    let asked_before = world.github_paths().len();

    let checked = when_i_verify(&world, &[]);

    assert_eq!(checked.status, 0, "{}\n{}", checked.stdout, checked.stderr);
    assert_eq!(
        world.github_paths_since(asked_before),
        [JEFF_GH.login_path(), JEFF_GH.id_path()],
        "the two-request rename case: login read, then the id read"
    );
    assert_shown(
        &when_i_list(&world, None),
        MY_DID,
        JEFF_RENAMED,
        Shown::Associated,
    );
}

/// IV-3b (edge, chained on IV-3a)
/// ```gherkin
/// @driving_port @real-io @edge @gdl-d9 @contract-shape:bounded-change
/// Scenario: After a rename is recognised, the next re-check reads the new name with one request
///   Given Jeff's renamed account was re-checked as "jeff-b"
///   When Jeff re-checks his links again
///   Then GitHub was asked once, for "jeff-b"
/// ```
#[test]
#[ignore = "DISTILL github-did-identity-link: pending DELIVER (one at a time)"]
fn after_a_rename_is_recognised_the_next_recheck_reads_the_new_name_with_one_request() {
    let world = LinkWorld::with(&[JEFF_GH]);
    given_i_linked(&world, JEFF_GH, None);
    i_renamed_my_account(&world, JEFF_GH, JEFF_RENAMED, OldLogin::NotFound);
    given_i_verified(&world, None);
    let asked_before = world.github_paths().len();

    let checked = when_i_verify(&world, &[]);

    assert_verify_reported(&checked, JEFF_RENAMED, Verdict::Verified);
    assert_eq!(
        world.github_paths_since(asked_before),
        [JEFF_RENAMED.login_path()],
        "the last-seen login is read first"
    );
}

/// IV-4 (edge, rename by redirect)
/// ```gherkin
/// @driving_port @real-io @adapter-integration @edge @gdl-d9 @n-2 @contract-shape:bounded-change
/// Scenario: Jeff stays verified under his new GitHub name, found from his old name alone
///   Given Jeff linked github "jeffbailey"
///   And he renamed his account to "jeff-b", and GitHub forwards the old name to the new one
///   When Jeff re-checks his links
///   Then his linked identities show him as github:jeff-b, verified
///   And GitHub was asked at most twice and never by account number
/// ```
#[test]
#[ignore = "DISTILL github-did-identity-link: pending DELIVER (one at a time)"]
fn jeff_stays_verified_under_his_new_github_name_found_from_his_old_name_alone() {
    let world = LinkWorld::with(&[JEFF_GH]);
    given_i_linked(&world, JEFF_GH, None);
    i_renamed_my_account(&world, JEFF_GH, JEFF_RENAMED, OldLogin::Redirects);
    let asked_before = world.github_paths().len();

    let checked = when_i_verify(&world, &[]);

    assert_eq!(checked.status, 0, "{}\n{}", checked.stdout, checked.stderr);
    assert_shown(
        &when_i_list(&world, None),
        MY_DID,
        JEFF_RENAMED,
        Shown::Associated,
    );
    let asked = world.github_paths_since(asked_before);
    assert!(asked.len() <= 2, "at most two requests: {asked:?}");
    assert!(!asked.contains(&JEFF_GH.id_path()), "no id read: {asked:?}");
}

/// IV-5 (error/edge, table)
/// ```gherkin
/// @driving_port @real-io @adapter-integration @error @gdl-d9 @a-10 @uc-10 @contract-shape:bounded-change
/// Scenario Outline: The account number tells a re-registered login and a deleted account apart
///   Given Jeff linked github "jeffbailey"
///   And on GitHub, <what happened>
///   When Jeff re-checks his links
///   Then he is told <outcome>
///   And his linked identities show <shown>
///   Examples:
///     | what happened                                           | outcome                         | shown                                   |
///     | he renamed to "jeff-b" and a stranger took "jeffbailey" | github:jeff-b is verified       | him as github:jeff-b, verified          |
///     | he deleted his account and a stranger took "jeffbailey" | the account no longer exists    | his claim on jeffbailey, proof gone     |
///     | he deleted his account                                  | the account no longer exists    | his claim on jeffbailey, proof gone     |
/// ```
#[test]
#[ignore = "DISTILL github-did-identity-link: pending DELIVER (one at a time)"]
fn the_account_number_tells_a_re_registered_login_and_a_deleted_account_apart() {
    let rows = [
        (
            AccountFate::RenamedAndLoginReRegistered,
            JEFF_RENAMED,
            Verdict::Verified,
            Shown::Associated,
        ),
        (
            AccountFate::DeletedAndLoginReRegistered,
            JEFF_GH,
            Verdict::AccountNotFound,
            Shown::ProofGone(Verdict::AccountNotFound),
        ),
        (
            AccountFate::Deleted,
            JEFF_GH,
            Verdict::AccountNotFound,
            Shown::ProofGone(Verdict::AccountNotFound),
        ),
    ];
    for (fate, account, verdict, shown) in rows {
        let world = LinkWorld::with(&[JEFF_GH]);
        given_i_linked(&world, JEFF_GH, None);
        my_account_meets(&world, fate);

        let checked = when_i_verify(&world, &[]);

        assert_eq!(
            checked.status, 0,
            "[{fate:?}] {}\n{}",
            checked.stdout, checked.stderr
        );
        assert_verify_reported(&checked, account, verdict);
        assert_shown_in(
            &format!("{fate:?}"),
            &when_i_list(&world, None),
            MY_DID,
            account,
            shown,
        );
    }
}

/// IV-6 (error, infrastructure-failure, table)
/// ```gherkin
/// @driving_port @real-io @error @infrastructure-failure @adr-085-4 @contract-shape:unbounded-preservation
/// Scenario Outline: A GitHub failure during a re-check changes nothing
///   Given Jeff linked github "jeffbailey" and it is verified
///   And GitHub <trouble>
///   When Jeff re-checks his links
///   Then he is told GitHub could not be read and nothing was recorded, and his token is never shown
///   And his linked identities still show him as github:jeffbailey, verified
///   Examples:
///     | trouble                              |
///     | is rate-limiting anonymous requests  |
///     | is unreachable                       |
///     | rejects his GITHUB_TOKEN             |
/// ```
#[test]
#[ignore = "DISTILL github-did-identity-link: pending DELIVER (one at a time)"]
fn a_github_failure_during_a_recheck_changes_nothing() {
    for trouble in GithubTrouble::ALL {
        let row = format!("{trouble:?}");
        let world = LinkWorld::with(&[JEFF_GH]);
        given_i_linked(&world, JEFF_GH, None);
        github_has(&world, trouble);
        let before = capture_link_universe(&world.env);

        let checked = when_i_verify(&world, &[]);

        assert_github_unavailable(&checked, &row);
        assert_shown_in(
            &row,
            &when_i_list(&world, None),
            MY_DID,
            JEFF_GH,
            Shown::Associated,
        );
        assert_link_store_unchanged_in(&row, &before, &world.env);
    }
}

/// IV-7 (mode flags, table)
/// ```gherkin
/// @driving_port @real-io @edge @c5a @c5b @contract-shape:bounded-change
/// Scenario Outline: Re-checking can be narrowed to one person or one account
///   Given Jeff linked github "jeffbailey" and he follows Rachel, who linked github "rachelk"
///   When Jeff re-checks with <filter>
///   Then GitHub is asked only about <accounts asked>, and every answer is still "verified"
///   Examples:
///     | filter                       | accounts asked                       |
///     | no filter                    | jeffbailey, rachelk                  |
///     | --did did:plc:rachel-test    | rachelk                              |
///     | --github jeffbailey          | jeffbailey                           |
///     | --did and --github together  | nothing (usage error naming --did)   |
/// ```
#[test]
#[ignore = "DISTILL github-did-identity-link: pending DELIVER (one at a time)"]
fn rechecking_can_be_narrowed_to_one_person_or_one_account() {
    let rows: [(&str, &[&str], Vec<Account>); 4] = [
        ("no filter", &[], vec![JEFF_GH, RACHEL_GH]),
        ("--did", &["--did", RACHEL_DID], vec![RACHEL_GH]),
        ("--github", &["--github", JEFF_GH.login], vec![JEFF_GH]),
        (
            "both flags",
            &["--did", RACHEL_DID, "--github", JEFF_GH.login],
            Vec::new(),
        ),
    ];
    for (row, filters, accounts) in rows {
        let mut world = LinkWorld::with(&[JEFF_GH, RACHEL_GH]);
        given_i_linked(&world, JEFF_GH, None);
        bio_names(&world, RACHEL_GH, RACHEL_DID);
        given_peer_published(
            &mut world,
            Peer::Rachel,
            vec![link_spec(RACHEL_DID, RACHEL_GH)],
        );
        let asked_before = world.github_paths().len();

        let checked = when_i_verify(&world, filters);

        if accounts.is_empty() {
            assert_usage_error(&checked, "--did", row);
        } else {
            assert_eq!(
                checked.status, 0,
                "[{row}] {}\n{}",
                checked.stdout, checked.stderr
            );
            for account in &accounts {
                // C5b: a filter narrows the scope; the verdict stays "verified".
                assert_verify_reported(&checked, *account, Verdict::Verified);
            }
        }
        let mut asked = world.github_paths_since(asked_before);
        asked.sort();
        let mut expected: Vec<String> = accounts.iter().map(|a| a.login_path()).collect();
        expected.sort();
        assert_eq!(asked, expected, "[{row}]");
    }
}

/// IV-8 (edge, C3 zero)
/// ```gherkin
/// @driving_port @real-io @edge @c3 @contract-shape:unbounded-preservation
/// Scenario: With no links in the store, a re-check says so and asks GitHub nothing
///   Given Jeff's store holds no identity links
///   When Jeff re-checks his links
///   Then he is told there are no identity links to verify
///   And GitHub was never asked
/// ```
#[test]
#[ignore = "DISTILL github-did-identity-link: pending DELIVER (one at a time)"]
fn with_no_links_a_recheck_says_so_and_asks_github_nothing() {
    let world = LinkWorld::with(&[JEFF_GH]);

    let checked = when_i_verify(&world, &[]);

    assert_eq!(checked.status, 0, "{}\n{}", checked.stdout, checked.stderr);
    assert!(
        checked.stdout.contains("No identity links to verify"),
        "{}",
        checked.stdout
    );
    assert!(
        world.github_paths().is_empty(),
        "{:?}",
        world.github_paths()
    );
}

/// IV-9 (happy, F1, chained on IL-10)
/// ```gherkin
/// @driving_port @real-io @happy @a-4 @f1 @contract-shape:bounded-change
/// Scenario: One re-check covers both of Jeff's GitHub accounts
///   Given Jeff linked github "jeffbailey" and github "jeff-at-work"
///   When Jeff re-checks his links
///   Then GitHub was asked about both accounts and both are reported verified
/// ```
#[test]
#[ignore = "DISTILL github-did-identity-link: pending DELIVER (one at a time)"]
fn one_recheck_covers_both_of_jeffs_github_accounts() {
    let world = LinkWorld::with(&[JEFF_GH, JEFF_WORK_GH]);
    given_i_linked(&world, JEFF_GH, None);
    given_i_linked(&world, JEFF_WORK_GH, None);
    let asked_before = world.github_paths().len();

    let checked = when_i_verify(&world, &[]);

    assert_verify_reported(&checked, JEFF_GH, Verdict::Verified);
    assert_verify_reported(&checked, JEFF_WORK_GH, Verdict::Verified);
    let mut asked = world.github_paths_since(asked_before);
    asked.sort();
    let mut expected = vec![JEFF_GH.login_path(), JEFF_WORK_GH.login_path()];
    expected.sort();
    assert_eq!(asked, expected);
}

/// IV-10 (error, F8, table)
/// ```gherkin
/// @driving_port @real-io @adapter-integration @error @n-2 @f8 @contract-shape:unbounded-preservation
/// Scenario Outline: An answer from GitHub that OpenLore will not follow leaves Jeff's link exactly as it was
///   Given Jeff linked github "jeffbailey"
///   And GitHub answers for "jeffbailey" by forwarding <where>
///   When Jeff re-checks his links
///   Then he is told GitHub could not be read and nothing was recorded
///   And his linked identities still show him as github:jeffbailey, verified
///   Examples:
///     | where                                          |
///     | to a name that forwards again                  |
///     | to another site                                |
/// ```
#[test]
#[ignore = "DISTILL github-did-identity-link: pending DELIVER (one at a time)"]
fn an_answer_from_github_that_openlore_will_not_follow_leaves_jeffs_link_as_it_was() {
    for row in ["chained redirect", "cross-host redirect"] {
        let world = LinkWorld::with(&[JEFF_GH]);
        given_i_linked(&world, JEFF_GH, None);
        if row == "chained redirect" {
            i_renamed_my_account(&world, JEFF_GH, JEFF_RENAMED, OldLogin::Redirects);
            i_renamed_my_account(
                &world,
                JEFF_RENAMED,
                JEFF_RENAMED_TWICE,
                OldLogin::Redirects,
            );
        } else {
            i_renamed_my_account(
                &world,
                JEFF_GH,
                JEFF_RENAMED,
                OldLogin::RedirectsToAnotherHost,
            );
        }
        let before = capture_link_universe(&world.env);

        let checked = when_i_verify(&world, &[]);

        assert_github_unavailable(&checked, row);
        assert_shown_in(
            row,
            &when_i_list(&world, None),
            MY_DID,
            JEFF_GH,
            Shown::Associated,
        );
        assert_link_store_unchanged_in(row, &before, &world.env);
    }
}

// =============================================================================
// Local erasure (`identity forget`)
// =============================================================================

/// IF-1a (happy)
/// ```gherkin
/// @driving_port @real-io @happy @c-gdl-5 @m-4 @contract-shape:bounded-change
/// Scenario: Forgetting Rachel erases only her proof history
///   Given Jeff verified his own link and Rachel's link
///   When Jeff forgets did:plc:rachel-test
///   Then Rachel's link is shown as not checked, and his own is still verified
///   And no claim changed
/// ```
#[test]
#[ignore = "DISTILL github-did-identity-link: pending DELIVER (one at a time)"]
fn forgetting_rachel_erases_only_her_proof_history() {
    let mut world = LinkWorld::with(&[JEFF_GH, RACHEL_GH]);
    given_i_linked(&world, JEFF_GH, None);
    bio_names(&world, RACHEL_GH, RACHEL_DID);
    given_peer_published(
        &mut world,
        Peer::Rachel,
        vec![link_spec(RACHEL_DID, RACHEL_GH)],
    );
    given_i_verified(&world, None);
    let mine: Vec<CheckRow> = link_checks(&world.env)
        .into_iter()
        .filter(|c| c.subject_did == MY_DID)
        .collect();
    let before = capture_link_universe(&world.env);

    let forgot = when_i_forget(&world, RACHEL_DID);

    assert_eq!(forgot.status, 0, "{}\n{}", forgot.stdout, forgot.stderr);
    let listed = when_i_list(&world, None);
    assert_shown(&listed, RACHEL_DID, RACHEL_GH, Shown::NotChecked);
    assert_shown(&listed, MY_DID, JEFF_GH, Shown::Associated);
    assert_link_store_delta(
        &before,
        &world.env,
        &[("local.identity_link_checks", checks_with(&mine, &[]))],
    );
}

/// IF-1b (edge, idempotency; chained on IF-1a)
/// ```gherkin
/// @driving_port @real-io @edge @c4a @contract-shape:unbounded-preservation
/// Scenario: Forgetting Rachel a second time forgets nothing more
///   Given Jeff forgot did:plc:rachel-test
///   When Jeff forgets did:plc:rachel-test again
///   Then he is told 0 proof checks were forgotten and nothing changed
/// ```
#[test]
#[ignore = "DISTILL github-did-identity-link: pending DELIVER (one at a time)"]
fn forgetting_rachel_a_second_time_forgets_nothing_more() {
    let mut world = LinkWorld::with(&[RACHEL_GH]);
    bio_names(&world, RACHEL_GH, RACHEL_DID);
    given_peer_published(
        &mut world,
        Peer::Rachel,
        vec![link_spec(RACHEL_DID, RACHEL_GH)],
    );
    given_i_verified(&world, None);
    assert_eq!(
        when_i_forget(&world, RACHEL_DID).status,
        0,
        "precondition: forgot"
    );
    let before = capture_link_universe(&world.env);

    let again = when_i_forget(&world, RACHEL_DID);

    assert_eq!(again.status, 0, "{}\n{}", again.stdout, again.stderr);
    assert!(again.stdout.contains("Forgot 0"), "{}", again.stdout);
    assert_link_store_unchanged(&before, &world.env);
}

/// IF-2 (error/edge, table)
/// ```gherkin
/// @driving_port @real-io @edge @c4b @c6a @contract-shape:unbounded-preservation
/// Scenario Outline: Forgetting without a history, or without naming a valid DID, changes nothing
///   Given Jeff linked github "jeffbailey"
///   When Jeff runs forget <arguments>
///   Then <outcome> and nothing changed
///   Examples:
///     | arguments                    | outcome                                     |
///     | --did did:plc:stranger-test  | he is told 0 proof checks were forgotten    |
///     | (none)                       | a usage error names --did                   |
///     | --did not-a-did              | a usage error names --did                   |
/// ```
#[test]
#[ignore = "DISTILL github-did-identity-link: pending DELIVER (one at a time)"]
fn forgetting_without_a_history_or_a_valid_did_changes_nothing() {
    let rows: [(&str, &[&str]); 3] = [
        ("no history", &["forget", "--did", STRANGER_DID]),
        ("no --did", &["forget"]),
        ("malformed --did", &["forget", "--did", "not-a-did"]),
    ];
    for (row, args) in rows {
        let world = LinkWorld::with(&[JEFF_GH]);
        given_i_linked(&world, JEFF_GH, None);
        let before = capture_link_universe(&world.env);

        let forgot = identity(&world, args, "", None);

        if row == "no history" {
            assert_eq!(
                forgot.status, 0,
                "[{row}] {}\n{}",
                forgot.stdout, forgot.stderr
            );
            assert!(
                forgot.stdout.contains("Forgot 0"),
                "[{row}] {}",
                forgot.stdout
            );
        } else {
            assert_usage_error(&forgot, "--did", row);
        }
        assert_link_store_unchanged_in(row, &before, &world.env);
    }
}
