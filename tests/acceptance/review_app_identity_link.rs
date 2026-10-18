//! github-did-identity-link — slice 3: the review app publishes and retracts
//! a verified GitHub link as a self-attested `sameAs` claim (GDL-D6,
//! ADR-084 §2/§5, ADR-085 §2 freshness rule C-GDL-10, ADR-073 create-only).
//!
//! Driving port: the REAL `openlore-review-app` over HTTP (session + CSRF,
//! existing harness). Fakes: ATProto network + `FakeGithubAccounts` only. The
//! stored private `github_links` row is never the proof: the publish handler
//! (the confirm request) reads the profile itself (Q-GDL-D8, pinned). Layer 4
//! (real HTTP): example-only; sad paths are named examples (Mandate 11).
//! PRIMARY Thens observe pages and the wire records at the user's PDS. All
//! `#[ignore]`d for one-at-a-time DELIVER except RA-3, a regression guard that
//! holds today (D-d).

#[path = "support/review_app/mod.rs"]
mod review_app;

use openlore_test_support::{WritePosture, CLAIM_COLLECTION};
use review_app::state_delta::{assert_state_delta, Delta};
use review_app::*;

/// RA-1a
/// ```gherkin
/// @driving_port @real-io @gdl-d6 @gdl-d12 @contract-shape:unbounded-preservation
/// Scenario: Priya sees exactly what her link will publish before anything is written
///   Given Priya verified github "priyaraman" in the app
///   When she presses "Publish this link"
///   Then she sees the exact record, that it is public, and that retracting withdraws it but does not erase it
///   And nothing has been written to her repo
/// ```
#[test]
fn priya_sees_exactly_what_her_link_will_publish_before_anything_is_written() {
    let world = ReviewWorld::new();
    let mut browser = given_github_verified(&world, Persona::Priya);

    when_opens_link_preview(&mut browser);

    then_sees_link_preview(&browser.page, Persona::Priya);
    assert!(
        world.atproto.write_attempts().is_empty(),
        "the preview writes nothing"
    );
}

/// RA-1b (chained on RA-1a)
/// ```gherkin
/// @driving_port @real-io @adapter-integration @gdl-d6 @gdl-d2 @c-gdl-10 @adr-073 @f2 @contract-shape:bounded-change
/// Scenario: Priya's verified GitHub account is publicly linked to her DID from the app
///   Given Priya is on her link preview
///   When she confirms
///   Then her repo holds one self-attested link "did:plc:… sameAs github:priyaraman" with her profile page and account number
///   And the app read her GitHub profile again while publishing
///   And the app's logs name no DID, no login and no bio
/// ```
#[test]
fn priyas_verified_github_account_is_publicly_linked_to_her_did_from_the_app() {
    let world = ReviewWorld::new();
    let mut browser = given_github_verified(&world, Persona::Priya);
    when_opens_link_preview(&mut browser);
    let reads_before_confirm = profile_reads_of(&world, Persona::Priya);

    when_confirms_link(&mut browser);

    let link = then_pds_holds_self_attested_link(&world, Persona::Priya);
    then_sees_record_address(&browser.page, &link);
    assert!(
        profile_reads_of(&world, Persona::Priya) > reads_before_confirm,
        "the publish request reads the profile itself (fresh proof, C-GDL-10)"
    );
    assert!(
        world.atproto.forbidden_write_attempts().is_empty(),
        "create-only"
    );
    then_logs_are_closed_and_leak_nothing(
        &world.app.logs(),
        &[
            Persona::Priya.did().to_string(),
            "Rust, tide models".to_string(),
            github_subject(Persona::Priya),
        ],
    );
}

/// RA-2 (error, freshness)
/// ```gherkin
/// @driving_port @real-io @error @c-gdl-10 @adr-085-2 @kpi-gdl-1 @contract-shape:unbounded-preservation
/// Scenario: A link is never published on the strength of an old verification
///   Given Priya verified github "priyaraman" in the app and opened her link preview
///   And she then removed her DID from her GitHub bio
///   When she confirms
///   Then she is told the proof is gone and nothing is written to her repo
/// ```
#[test]
fn a_link_is_never_published_on_the_strength_of_an_old_verification() {
    let world = ReviewWorld::new();
    let mut browser = given_github_verified(&world, Persona::Priya);
    when_opens_link_preview(&mut browser);
    removes_did_from_bio(&world, Persona::Priya);

    when_confirms_link(&mut browser);

    assert!(
        browser.page.shows("proof is gone"),
        "the refusal says the proof is gone:\n{}",
        browser.page.text()
    );
    assert!(world.atproto.write_attempts().is_empty(), "nothing written");
}

/// RA-3 (guardrail, opt-in — ACTIVE regression guard, D-d)
///
/// Verify-then-scan is ONE user flow in the app (the scan is only offered
/// after a verification, and the user does both in one sitting), so the When
/// is that flow, not two independent actions.
/// ```gherkin
/// @driving_port @real-io @guardrail @a-2 @gdl-d12 @contract-shape:unbounded-preservation
/// Scenario: Verifying GitHub and scanning never publish a link on their own
///   Given Priya signed in
///   When she verifies github "priyaraman" and lets her scan finish
///   Then nothing at all was written to her published repo
/// ```
#[test]
fn verifying_github_and_scanning_never_publish_a_link_on_their_own() {
    let world = ReviewWorld::new();
    let mut browser = given_signed_in(&world, Persona::Priya);
    let claims_before = claims_in_pds(&world, Persona::Priya).len();

    when_verifies_github(&mut browser, Persona::Priya.github_login());
    then_sees_verified(&browser.page, Persona::Priya);
    when_scan_finishes(&mut browser);

    assert!(
        world.atproto.write_attempts().is_empty(),
        "no write attempt of any kind: {:?}",
        world.atproto.write_attempts()
    );
    assert_eq!(claims_in_pds(&world, Persona::Priya).len(), claims_before);
    assert!(links_in_pds(&world, Persona::Priya).is_empty());
}

/// RA-4 (lifecycle)
/// ```gherkin
/// @driving_port @real-io @gdl-d5 @adr-008 @c-gdl-6 @contract-shape:bounded-change
/// Scenario: Priya withdraws her link without erasing it
///   Given Priya published her GitHub link from the app
///   When she presses "Retract this link" and confirms the retraction
///   Then her published repo holds one more claim, by her, that retracts the link
///   And the link record itself is still there, unchanged
/// ```
#[test]
fn priya_withdraws_her_link_without_erasing_it() {
    let world = ReviewWorld::new();
    let mut browser = given_link_published(&world, Persona::Priya);
    let link = then_pds_holds_self_attested_link(&world, Persona::Priya);
    let claims_before = claims_in_pds(&world, Persona::Priya).len();
    when_opens_link_retraction(&mut browser);

    browser.press("Confirm retraction");

    let claims = claims_in_pds(&world, Persona::Priya);
    assert_eq!(claims.len(), claims_before + 1, "one retraction added");
    assert!(
        claims
            .iter()
            .any(|r| r.value["author"] == Persona::Priya.did()
                && r.value["references"].as_array().is_some_and(|refs| refs
                    .iter()
                    .any(|x| x["type"] == "retracts" && x["cid"] == link.rkey.as_str()))),
        "a retraction by Priya referencing the link: {claims:#?}"
    );
    assert_eq!(
        then_pds_holds_self_attested_link(&world, Persona::Priya),
        link
    );
    assert!(
        world.atproto.forbidden_write_attempts().is_empty(),
        "create-only"
    );
}

/// RA-4b (error, regression guard; slice-3 re-review N2, F-4 fixed in adbc648)
/// ```gherkin
/// @driving_port @real-io @error @f-4 @n-2 @contract-shape:unbounded-preservation
/// Scenario Outline: A refused GitHub request keeps the offer to retract Priya's link
///   Given Priya published her GitHub link from the app
///   When <she asks for something GitHub refuses>
///   Then the page that refuses her still offers "Retract this link"
///   And her link is still there, unchanged
///   Examples:
///     | she asks for something GitHub refuses                        |
///     | she verifies github "priyaramn", which has no account          |
///     | she asks for a link preview naming the malformed username "-priyaraman-" |
/// ```
#[test]
fn a_refused_github_request_keeps_the_offer_to_retract_priyas_link() {
    for row in ["verify unknown account", "malformed link preview"] {
        let world = ReviewWorld::new();
        let mut browser = given_link_published(&world, Persona::Priya);
        let link = then_pds_holds_self_attested_link(&world, Persona::Priya);
        match row {
            "verify unknown account" => {
                when_verifies_github(&mut browser, UNKNOWN_GITHUB_LOGIN);
            }
            _ => {
                let offer = link_offer_form(&mut browser);
                when_submits_link_offer(
                    &mut browser,
                    &Browser::with_field(&offer, LINK_LOGIN_FIELD, MALFORMED_GITHUB_LOGIN),
                );
            }
        }

        then_refused_page_offers_retraction(&browser.page, row);
        assert_eq!(
            then_pds_holds_self_attested_link(&world, Persona::Priya),
            link,
            "[{row}] her link is unchanged"
        );
    }
}

/// RA-5 (error, infrastructure-failure, table; rows 3-4 from the slice-3
/// roadmap review F-10)
///
/// The confirm form is captured on the preview and submitted after the
/// trouble starts, so the "used up" row can spend the budget on the GitHub
/// step first.
/// ```gherkin
/// @driving_port @real-io @error @infrastructure-failure @adr-085-4 @f-10 @contract-shape:unbounded-preservation
/// Scenario Outline: When GitHub cannot be read while publishing a link, nothing is published
///   Given Priya is on her link preview
///   And <trouble>
///   When she confirms
///   Then she sees "<message>" and "Nothing was published."
///   And the app's GitHub token is not on the page
///   And nothing is written to her published repo
///   Examples:
///     | trouble                                                  | message                                                  |
///     | GitHub is rate-limiting the app                          | GitHub is rate-limiting us                               |
///     | GitHub rejects the app's token                           | GitHub did not accept the app's access                   |
///     | she has used up this hour's GitHub checks                | Too many verification attempts in the last hour          |
///     | GitHub no longer has an account "priyaraman"             | We couldn't find a GitHub account at github.com/priyaraman |
/// ```
#[test]
fn when_github_cannot_be_read_while_publishing_a_link_nothing_is_published() {
    let rows = [
        ("rate limited", "GitHub is rate-limiting us".to_string()),
        (
            "token rejected",
            "GitHub did not accept the app's access".to_string(),
        ),
        ("budget used up", TOO_MANY_ATTEMPTS.to_string()),
        (
            "account not found",
            account_not_found_message(Persona::Priya.github_login()),
        ),
    ];
    for (row, message) in rows {
        let world = ReviewWorld::new();
        let mut browser = given_github_verified(&world, Persona::Priya);
        when_opens_link_preview(&mut browser);
        let confirm = link_confirm_form(&browser);
        match row {
            "rate limited" => world.github.rate_limit_for(240),
            "token rejected" => world.github.reject_token(true),
            "budget used up" => uses_up_the_hours_verify_attempts(&mut browser, Persona::Priya),
            _ => world.github.delete_account(Persona::Priya.github_login()),
        }

        browser.submit(&confirm, Some(CONFIRM_LINK));

        assert!(
            browser.page.shows(&message),
            "[{row}] {}",
            browser.page.text()
        );
        assert!(
            browser.page.shows(NOTHING_PUBLISHED),
            "[{row}] the publish itself was refused:\n{}",
            browser.page.text()
        );
        assert!(
            !browser.page.html.contains(SERVER_GITHUB_TOKEN),
            "[{row}] the app's GitHub token is never shown"
        );
        assert!(
            world.atproto.write_attempts().is_empty(),
            "[{row}] nothing written"
        );
    }
}

/// RA-6 (error, adversarial, table)
/// ```gherkin
/// @driving_port @real-io @error @adversarial @contract-shape:unbounded-preservation
/// Scenario Outline: A link confirmation that did not come from Priya's own page is refused
///   Given Priya is on her link preview
///   When a confirmation is submitted carrying <token>
///   Then it is refused as forbidden and nothing is written
///   Examples:
///     | token                          |
///     | no page token                  |
///     | a page token Priya never saw   |
/// ```
#[test]
fn a_link_confirmation_that_did_not_come_from_priyas_own_page_is_refused() {
    let world = ReviewWorld::new();
    let mut browser = given_github_verified(&world, Persona::Priya);
    when_opens_link_preview(&mut browser);
    let confirm = link_confirm_form(&browser);

    for token in ["", "forged-token"] {
        let forged = Browser::with_field(&confirm, "csrf", token);
        browser.submit(&forged, Some(CONFIRM_LINK));
        assert_eq!(browser.page.status, 403, "[{token:?}] refused as forbidden");
    }
    assert!(world.atproto.write_attempts().is_empty());
}

/// RA-7 (edge)
/// ```gherkin
/// @driving_port @real-io @edge @gdl-d12 @contract-shape:unbounded-preservation
/// Scenario: Leaving the link preview publishes nothing and keeps the offer open
///   Given Priya is on her link preview
///   When she presses "Cancel"
///   Then nothing is written and "Publish this link" is offered again
/// ```
#[test]
fn leaving_the_link_preview_publishes_nothing_and_keeps_the_offer_open() {
    let world = ReviewWorld::new();
    let mut browser = given_github_verified(&world, Persona::Priya);
    when_opens_link_preview(&mut browser);

    browser.press("Cancel");

    assert!(world.atproto.write_attempts().is_empty());
    let offered = browser.open("/github").offers("Publish this link");
    assert!(offered, "{}", browser.page.text());
}

/// RA-8 (error, platform, table)
/// ```gherkin
/// @driving_port @real-io @error @infrastructure-failure @session @contract-shape:unbounded-preservation
/// Scenario Outline: An ended session at confirmation publishes nothing and asks Priya to sign in again
///   Given Priya is on her link preview
///   And <session ended>
///   When she confirms
///   Then she is asked to sign in again and nothing is in her published repo
///   Examples:
///     | session ended                                                    |
///     | 31 days pass, so the app's 30-day session expires (cookie kept)  |
///     | her browser no longer sends the session cookie                   |
///     | her repo host session expired                                    |
/// ```
#[test]
fn an_ended_session_at_confirmation_publishes_nothing_and_asks_priya_to_sign_in_again() {
    const THIRTY_ONE_DAYS: u64 = 31 * 24 * 60 * 60;
    for row in [
        "session expired on the server",
        "cookie cleared",
        "repo host session expired",
    ] {
        let world = ReviewWorld::new();
        let mut browser = given_github_verified(&world, Persona::Priya);
        when_opens_link_preview(&mut browser);
        let confirm = link_confirm_form(&browser);

        let world = match row {
            "session expired on the server" => {
                // The REVIEW_APP_CLOCK_SKEW_SECS seam: the server's clock runs
                // past the 30-day session; the browser keeps its cookie.
                let world = world.restart_app_later(THIRTY_ONE_DAYS);
                browser.submit(&confirm, Some(CONFIRM_LINK));
                assert!(
                    browser.page.offers("Sign in with Bluesky"),
                    "[{row}] {}",
                    browser.page.text()
                );
                world
            }
            "cookie cleared" => {
                browser.clear_cookies();
                browser.submit(&confirm, Some(CONFIRM_LINK));
                assert!(
                    browser.page.offers("Sign in with Bluesky"),
                    "[{row}] {}",
                    browser.page.text()
                );
                world
            }
            _ => {
                world
                    .atproto
                    .set_write_posture(Persona::Priya.did(), WritePosture::ExpiredSession);
                browser.submit(&confirm, Some(CONFIRM_LINK));
                assert!(
                    browser.page.shows("Sign in again"),
                    "[{row}] {}",
                    browser.page.text()
                );
                world
            }
        };
        assert!(
            links_in_pds(&world, Persona::Priya).is_empty(),
            "[{row}] nothing published"
        );
    }
}

// =============================================================================
// Slice-3 roadmap review (2026-10-17): RA-9..RA-14. Each fails today at the
// missing "Publish this link" offer (MISSING_FUNCTIONALITY).
// =============================================================================

/// RA-9 (guardrail, table; roadmap review F-08)
/// ```gherkin
/// @driving_port @real-io @guardrail @f-08 @gdl-d12 @contract-shape:pure-function
/// Scenario Outline: Only an owner the app has verified is offered "Publish this link"
///   Given <owner state>
///   When the GitHub step is opened
///   Then "Publish this link" is <offered>
///   Examples:
///     | owner state                                                           | offered     |
///     | Priya verified github "priyaraman"                                    | offered     |
///     | Priya signed in and never verified                                    | not offered |
///     | Sam's verify of "samortega" was refused (no DID in his bio)           | not offered |
///     | Priya verified, removed her DID from her bio and pressed Verify again | not offered |
///     | Priya verified and then signed out                                    | not offered |
/// ```
#[test]
fn only_an_owner_the_app_has_verified_is_offered_publish_this_link() {
    let rows = [
        ("verified", true),
        ("never verified", false),
        ("verify refused", false),
        ("verification lapsed", false),
        ("signed out", false),
    ];
    for (row, expected) in rows {
        let world = ReviewWorld::new();
        let mut browser = match row {
            "verified" => given_github_verified(&world, Persona::Priya),
            "never verified" => given_signed_in(&world, Persona::Priya),
            "verify refused" => {
                let mut sam = given_signed_in(&world, Persona::Sam);
                when_verifies_github(&mut sam, Persona::Sam.github_login());
                sam
            }
            "verification lapsed" => {
                let mut priya = given_github_verified(&world, Persona::Priya);
                removes_did_from_bio(&world, Persona::Priya);
                when_verifies_github(&mut priya, Persona::Priya.github_login());
                priya
            }
            _ => {
                let mut priya = given_github_verified(&world, Persona::Priya);
                when_signs_out(&mut priya);
                priya
            }
        };

        let offered = browser.open("/github").offers(PUBLISH_LINK);

        assert_eq!(
            offered,
            expected,
            "[{row}] {PUBLISH_LINK:?} offered = {offered}, expected {expected}:\n{}",
            browser.page.text()
        );
    }
}

/// RA-10 (error, adversarial, table; roadmap review F-01)
///
/// The offer form is captured on the GitHub step and replayed with one
/// thing wrong. "No link plan kept" is observed at the port: the reply
/// offers nothing to confirm.
/// ```gherkin
/// @driving_port @real-io @error @adversarial @f-01 @adr-076-2 @contract-shape:unbounded-preservation
/// Scenario Outline: A link preview request that fails a guard asks GitHub nothing and keeps nothing
///   Given Priya verified github "priyaraman" and sees "Publish this link"
///   And <guard failure>
///   When the link preview is requested
///   Then it is refused <how>
///   And GitHub was asked nothing, no link is offered for confirmation and nothing is written
///   Examples:
///     | guard failure                                       | how                                               |
///     | the request carries no page token                   | as forbidden                                      |
///     | the request carries a page token Priya never saw    | as forbidden                                      |
///     | she has used up this hour's GitHub checks           | with "Too many verification attempts in the last hour" |
///     | the request names the malformed username "-priyaraman-" | with "We couldn't find a GitHub account"      |
/// ```
#[test]
fn a_link_preview_request_that_fails_a_guard_asks_github_nothing_and_keeps_nothing() {
    let rows = [
        "no page token",
        "forged page token",
        "budget used up",
        "malformed username",
    ];
    for row in rows {
        let world = ReviewWorld::new();
        let mut browser = given_github_verified(&world, Persona::Priya);
        let offer = link_offer_form(&mut browser);
        let request = match row {
            "no page token" => Browser::with_field(&offer, "csrf", ""),
            "forged page token" => Browser::with_field(&offer, "csrf", "forged-token"),
            "budget used up" => {
                uses_up_the_hours_verify_attempts(&mut browser, Persona::Priya);
                offer.clone()
            }
            _ => Browser::with_field(&offer, LINK_LOGIN_FIELD, "-priyaraman-"),
        };
        let asked_before = github_requests(&world);

        when_submits_link_offer(&mut browser, &request);

        match row {
            "no page token" | "forged page token" => assert_eq!(
                browser.page.status,
                403,
                "[{row}] refused as forbidden:\n{}",
                browser.page.text()
            ),
            "budget used up" => assert!(
                browser.page.shows(TOO_MANY_ATTEMPTS),
                "[{row}] {}",
                browser.page.text()
            ),
            _ => assert!(
                browser.page.shows("We couldn't find a GitHub account"),
                "[{row}] {}",
                browser.page.text()
            ),
        }
        assert_eq!(
            github_requests(&world),
            asked_before,
            "[{row}] GitHub is asked nothing: {:?}",
            world.github.request_paths()
        );
        then_no_link_confirmation_is_offered(&browser.page);
        assert!(
            world.atproto.write_attempts().is_empty(),
            "[{row}] nothing written"
        );
    }
}

/// RA-10d (error, infrastructure-failure, table; roadmap review F-01)
/// ```gherkin
/// @driving_port @real-io @error @infrastructure-failure @f-01 @adr-085-4 @contract-shape:unbounded-preservation
/// Scenario Outline: When GitHub cannot be read for a link preview, Priya is refused without seeing the app's token
///   Given Priya verified github "priyaraman"
///   And GitHub <trouble>
///   When she presses "Publish this link"
///   Then she is refused, the app's GitHub token is not on the page
///   And no link is offered for confirmation and nothing is written
///   Examples:
///     | trouble                    |
///     | is rate-limiting the app   |
///     | rejects the app's token    |
///     | cannot be reached          |
/// ```
#[test]
fn when_github_cannot_be_read_for_a_link_preview_priya_is_refused_without_seeing_the_apps_token() {
    for row in ["rate limited", "token rejected", "offline"] {
        let world = ReviewWorld::new();
        let mut browser = given_github_verified(&world, Persona::Priya);
        let offer = link_offer_form(&mut browser);
        match row {
            "rate limited" => world.github.rate_limit_for(240),
            "token rejected" => world.github.reject_token(true),
            _ => world.github.set_offline(true),
        }

        when_submits_link_offer(&mut browser, &offer);

        assert!(
            !browser.page.html.contains(SERVER_GITHUB_TOKEN),
            "[{row}] the app's GitHub token is never shown"
        );
        then_no_link_confirmation_is_offered(&browser.page);
        assert!(
            world.atproto.write_attempts().is_empty(),
            "[{row}] nothing written"
        );
    }
}

/// RA-11a (error, adversarial; roadmap review F-11)
/// ```gherkin
/// @driving_port @real-io @error @adversarial @f-11 @gdl-d3 @c-gdl-1 @contract-shape:unbounded-preservation
/// Scenario: Sam cannot publish Priya's GitHub account as his own
///   Given Sam verified his own github "samortega" in the app
///   When he asks for a link preview naming github "priyaraman"
///   Then he is refused and no link from his DID to github:priyaraman is shown or offered for confirmation
///   And nothing is written to anyone's repo
/// ```
#[test]
fn sam_cannot_publish_priyas_github_account_as_his_own() {
    let world = ReviewWorld::new();
    puts_did_in_bio(&world, Persona::Sam);
    let mut sam = given_github_verified(&world, Persona::Sam);
    let offer = link_offer_form(&mut sam);
    let naming_priya = Browser::with_field(&offer, LINK_LOGIN_FIELD, Persona::Priya.github_login());

    when_submits_link_offer(&mut sam, &naming_priya);

    let stolen = format!(
        "{} {SAME_AS} {}",
        Persona::Sam.did(),
        github_subject(Persona::Priya)
    );
    assert!(
        !sam.page.shows(&stolen),
        "Sam is never shown a link to Priya's account:\n{}",
        sam.page.text()
    );
    then_no_link_confirmation_is_offered(&sam.page);
    assert!(
        world.atproto.write_attempts().is_empty(),
        "nothing written: {:?}",
        world.atproto.write_attempts()
    );
}

/// RA-11b (error; roadmap review F-11, U-25, AC-002.3)
/// ```gherkin
/// @driving_port @real-io @error @f-11 @u-25 @ac-002-3 @contract-shape:unbounded-preservation
/// Scenario: A bio naming someone else's DID is explained with that DID in full, and nothing is offered
///   Given Priya verified github "priyaraman" in the app
///   And her GitHub bio now names Aisha's DID instead of hers
///   When she presses "Publish this link"
///   Then she sees that the bio holds a different DID, shown in full
///   And no link is offered for confirmation and nothing is written
/// ```
#[test]
fn a_bio_naming_someone_elses_did_is_explained_with_that_did_in_full_and_nothing_is_offered() {
    let world = ReviewWorld::new();
    let mut browser = given_github_verified(&world, Persona::Priya);
    world.github.set_bio(
        Persona::Priya.github_login(),
        Some(&format!("Builder. {}", Persona::Aisha.did())),
    );

    when_opens_link_preview(&mut browser);

    let text = browser.page.text();
    assert!(
        text.contains("different DID") && text.contains(Persona::Aisha.did()),
        "the refusal names the bio's DID in full ({}):\n{text}",
        Persona::Aisha.did()
    );
    then_no_link_confirmation_is_offered(&browser.page);
    assert!(world.atproto.write_attempts().is_empty(), "nothing written");
}

/// RA-12 (error, infrastructure-failure → recovery, table; roadmap review F-08;
/// wording per user ruling U-42)
///
/// After a server error the app cannot tell "not stored" from "stored, answer
/// lost", so both rows say the link may have been saved and that Retry will
/// confirm — never "Nothing was published." (U-42). Row 2 uses `WritePosture::StoreThenServerError`: the repo host stores the
/// link and then answers 500, so the app cannot tell the write landed. Retry
/// re-creates the same record key (the link's CID, ADR-071), which the host
/// answers "already exists" — that must read as success, not a duplicate or
/// an error page.
/// ```gherkin
/// @driving_port @real-io @error @infrastructure-failure @f-08 @adr-071 @u-42 @contract-shape:bounded-change
/// Scenario Outline: After a failed link write, Retry leaves exactly one link
///   Given Priya is on her link preview
///   And her repo host <fails>
///   When she confirms
///   Then she sees that her link may have been saved and that Retry will confirm
///   And she does not see "Nothing was published."
///   When her repo host recovers and she presses Retry
///   Then her repo holds exactly one link and she sees its address, not an error page
///   Examples:
///     | fails                                                         |
///     | answers every write with a server error                       |
///     | stores the link but its answer is lost (answers a server error) |
/// ```
#[test]
fn after_a_failed_link_write_retry_publishes_the_link_exactly_once() {
    let rows = [
        ("server error", WritePosture::ServerError),
        ("answer lost", WritePosture::StoreThenServerError),
    ];
    for (row, failing) in rows {
        let world = ReviewWorld::new();
        let mut browser = given_github_verified(&world, Persona::Priya);
        when_opens_link_preview(&mut browser);
        world
            .atproto
            .set_write_posture(Persona::Priya.did(), failing);

        when_confirms_link(&mut browser);

        assert!(
            browser.page.shows(MAY_HAVE_BEEN_SAVED),
            "[{row}] the page says the link may have been saved (U-42):\n{}",
            browser.page.text()
        );
        assert!(
            !browser.page.shows(NOTHING_PUBLISHED),
            "[{row}] the page never claims nothing was published (U-42):\n{}",
            browser.page.text()
        );
        assert!(
            browser.page.offers("Retry"),
            "[{row}] Retry is offered to confirm:\n{}",
            browser.page.text()
        );

        world
            .atproto
            .set_write_posture(Persona::Priya.did(), WritePosture::Accept);
        browser.press("Retry");

        assert_eq!(
            browser.page.status,
            200,
            "[{row}] Retry ends on the published page, not an error page:\n{}",
            browser.page.text()
        );
        let link = then_pds_holds_self_attested_link(&world, Persona::Priya);
        then_sees_record_address(&browser.page, &link);
    }
}

/// RA-12b (error, infrastructure-failure, table; slice-3 re-review N1, U-42)
///
/// U-42 allows "may have been saved" only where the record may really have
/// been stored (RA-12). A repo host that refuses the record, or that cannot
/// be reached at all, stored nothing, so the page keeps "Nothing was
/// published." Whether Retry is offered here is not pinned. The unreachable
/// row makes the host unreachable on the preview; the confirm reads GitHub
/// (a separate double) first and the write is its first call to the repo host.
/// ```gherkin
/// @driving_port @real-io @error @infrastructure-failure @u-42 @n-1 @contract-shape:unbounded-preservation
/// Scenario Outline: When her repo host definitely did not take the link, Priya is told nothing was published
///   Given Priya is on her link preview
///   And her repo host <refuses>
///   When she confirms
///   Then she sees "Nothing was published."
///   And she is not told her link may have been saved
///   And her repo holds no link
///   Examples:
///     | refuses                       |
///     | refuses the link record       |
///     | cannot be reached             |
/// ```
#[test]
fn when_her_repo_host_definitely_did_not_take_the_link_priya_is_told_nothing_was_published() {
    for row in ["record refused", "host unreachable"] {
        let world = ReviewWorld::new();
        let mut browser = given_github_verified(&world, Persona::Priya);
        when_opens_link_preview(&mut browser);
        match row {
            "record refused" => world.atproto.set_write_posture(
                Persona::Priya.did(),
                WritePosture::RefuseCollection(CLAIM_COLLECTION.to_string()),
            ),
            _ => world
                .atproto
                .set_host_reachable(Persona::Priya.pds_host(), false),
        }

        when_confirms_link(&mut browser);

        then_told_nothing_was_published(&browser.page, row);
        assert!(
            links_in_pds(&world, Persona::Priya).is_empty(),
            "[{row}] her repo holds no link: {:#?}",
            claims_in_pds(&world, Persona::Priya)
        );
    }
}

/// RA-13 (edge; user ruling U-27 "hide for now")
/// ```gherkin
/// @driving_port @real-io @edge @u-27 @contract-shape:bounded-change
/// Scenario: Priya's published link stays off her public profile and her share draft
///   Given Priya published "priyaraman/tidepool embodies dependency-pinning" from the app
///   And she published her GitHub link from the app
///   When a visitor opens her profile and she opens "Share on Bluesky…"
///   Then the profile lists only her dependency-pinning claim and no link
///   And the share draft quotes her dependency-pinning claim and not her link
/// ```
#[test]
fn priyas_published_link_stays_off_her_public_profile_and_her_share_draft() {
    let world = ReviewWorld::new();
    let mut browser = given_published(
        &world,
        Persona::Priya,
        &[priya::TIDEPOOL_DEPENDENCY_PINNING],
    );
    when_opens_link_preview(&mut browser);
    when_confirms_link(&mut browser);
    then_pds_holds_self_attested_link(&world, Persona::Priya);
    let link_words = [github_subject(Persona::Priya), SAME_AS.to_string()];

    let mut visitor = world.browser();
    let profile = when_opens_profile(&mut visitor, Persona::Priya).clone();
    when_opens_share_preview(&mut browser, Persona::Priya);

    assert_eq!(
        profile.embodies_phrases(),
        phrases(&[priya::TIDEPOOL_DEPENDENCY_PINNING]),
        "her other published claim still shows"
    );
    assert_eq!(
        html::elements(&profile.html, "article").len(),
        1,
        "one claim card, no link card:\n{}",
        profile.text()
    );
    for word in &link_words {
        assert!(
            !profile.text().contains(word.as_str()),
            "the profile never lists the link ({word}):\n{}",
            profile.text()
        );
    }
    let draft = browser
        .forms()
        .iter()
        .flat_map(|f| f.fields.clone())
        .find(|(k, _)| k == "text")
        .map(|(_, v)| v)
        .expect("an editable post text");
    assert!(draft.contains("dependency-pinning"), "{draft}");
    for word in &link_words {
        assert!(
            !draft.contains(word.as_str()),
            "the share draft never quotes the link ({word}): {draft}"
        );
    }
}

/// RA-14 (error, adversarial; roadmap review F-03, user ruling U-21 B)
/// ```gherkin
/// @driving_port @real-io @error @adversarial @f-03 @u-21 @adr-084-4 @contract-shape:unbounded-preservation
/// Scenario: A link preview confirmed as an ordinary claim publish writes nothing and is used up
///   Given Priya has pending suggestions and is on her link preview
///   When its confirmation is sent to the ordinary claim-publish confirmation instead
///   Then nothing is written, no suggestion leaves her queue and her repo is unchanged
///   And confirming the link preview afterwards publishes nothing either
/// ```
#[test]
fn a_link_preview_confirmed_as_an_ordinary_claim_publish_writes_nothing_and_is_used_up() {
    let world = ReviewWorld::new();
    let mut browser = given_pending_suggestions(&world, Persona::Priya);
    when_opens_link_preview(&mut browser);
    let confirm = link_confirm_form(&browser);
    let mut as_claim_publish = confirm.clone();
    as_claim_publish.action = "/review/publish".to_string();
    let before = capture(&world, &mut browser, Persona::Priya);

    browser.submit(&as_claim_publish, Some(CONFIRM_LINK));

    let after = capture(&world, &mut browser, Persona::Priya);
    assert_state_delta(&before, &after, &universe(), &Delta::new());
    browser.submit(&confirm, Some(CONFIRM_LINK));
    assert!(
        links_in_pds(&world, Persona::Priya).is_empty(),
        "the link plan was used up: nothing published:\n{}",
        browser.page.text()
    );
    assert!(
        world.atproto.write_attempts().is_empty(),
        "nothing written: {:?}",
        world.atproto.write_attempts()
    );
}
