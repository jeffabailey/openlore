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

use openlore_test_support::WritePosture;
use review_app::*;

/// The confirm form on the current link preview.
fn link_confirm_form(browser: &Browser) -> html::Form {
    browser
        .forms()
        .into_iter()
        .find(|f| {
            f.buttons
                .iter()
                .any(|(l, _, _)| html::label_matches(l, "Publish to my repo"))
        })
        .expect("the link preview's confirm form")
}

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
#[ignore = "DISTILL github-did-identity-link: pending DELIVER (one at a time)"]
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
#[ignore = "DISTILL github-did-identity-link: pending DELIVER (one at a time)"]
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
#[ignore = "DISTILL github-did-identity-link: pending DELIVER (one at a time)"]
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
#[ignore = "DISTILL github-did-identity-link: pending DELIVER (one at a time)"]
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

/// RA-5 (error, infrastructure-failure, table)
/// ```gherkin
/// @driving_port @real-io @error @infrastructure-failure @adr-085-4 @contract-shape:unbounded-preservation
/// Scenario Outline: When GitHub cannot be read while publishing a link, nothing is published
///   Given Priya is on her link preview
///   And GitHub <trouble>
///   When she confirms
///   Then she sees "<message>" and "Nothing was published."
///   And the app's GitHub token is not on the page
///   And nothing is written to her published repo
///   Examples:
///     | trouble                    | message                                    |
///     | is rate-limiting the app   | GitHub is rate-limiting us                 |
///     | rejects the app's token    | GitHub did not accept the app's access     |
/// ```
#[test]
#[ignore = "DISTILL github-did-identity-link: pending DELIVER (one at a time)"]
fn when_github_cannot_be_read_while_publishing_a_link_nothing_is_published() {
    let rows = [
        ("rate limited", "GitHub is rate-limiting us"),
        ("token rejected", "GitHub did not accept the app's access"),
    ];
    for (row, message) in rows {
        let world = ReviewWorld::new();
        let mut browser = given_github_verified(&world, Persona::Priya);
        when_opens_link_preview(&mut browser);
        if row == "rate limited" {
            world.github.rate_limit_for(240);
        } else {
            world.github.reject_token(true);
        }

        when_confirms_link(&mut browser);

        assert!(
            browser.page.shows(message),
            "[{row}] {}",
            browser.page.text()
        );
        assert!(
            browser.page.shows("Nothing was published."),
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
#[ignore = "DISTILL github-did-identity-link: pending DELIVER (one at a time)"]
fn a_link_confirmation_that_did_not_come_from_priyas_own_page_is_refused() {
    let world = ReviewWorld::new();
    let mut browser = given_github_verified(&world, Persona::Priya);
    when_opens_link_preview(&mut browser);
    let confirm = link_confirm_form(&browser);

    for token in ["", "forged-token"] {
        let forged = Browser::with_field(&confirm, "csrf", token);
        browser.submit(&forged, Some("Publish to my repo"));
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
#[ignore = "DISTILL github-did-identity-link: pending DELIVER (one at a time)"]
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
#[ignore = "DISTILL github-did-identity-link: pending DELIVER (one at a time)"]
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
                browser.submit(&confirm, Some("Publish to my repo"));
                assert!(
                    browser.page.offers("Sign in with Bluesky"),
                    "[{row}] {}",
                    browser.page.text()
                );
                world
            }
            "cookie cleared" => {
                browser.clear_cookies();
                browser.submit(&confirm, Some("Publish to my repo"));
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
                browser.submit(&confirm, Some("Publish to my repo"));
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
