//! bluesky-claim-review-app — RELEASE 2 "Trust that lasts", app-side lifecycle
//! (US-BRA-010 rescan with ownership re-check, US-BRA-011 retract,
//! US-BRA-012 disconnect and forget me — including SPIKE-2 findings 1 and 2:
//! a revocation answered 200 is success and local tokens are ALWAYS deleted;
//! the already-issued access token's residual window is documented).
//!
//! Driving port: the REAL `openlore-review-app` over HTTP (+ its loopback
//! admin listener for the operator purge). Fakes: ATProto network + GitHub.
//! Layer 4: example-only. `#[ignore]`d for one-at-a-time DELIVER (Release 2).

#[path = "support/review_app/mod.rs"]
mod review_app;

use openlore_test_support::WritePosture;
use review_app::state_delta::{assert_state_delta, Delta};
use review_app::*;

// =============================================================================
// US-BRA-010 — rescan for only new suggestions, ownership re-checked
// =============================================================================

/// GIVEN Priya has 3 published claims and 1 declined suggestion (chained).
fn given_priya_returns_with_3_published_and_1_declined(world: &ReviewWorld) -> Browser {
    let mut browser = given_published(
        world,
        Persona::Priya,
        &[
            priya::TIDEPOOL_DEPENDENCY_PINNING,
            priya::TIDEPOOL_MEMORY_SAFETY,
            priya::TIDEPOOL_TEST_DRIVEN,
        ],
    );
    when_declines(&mut browser, priya::QUILL_DOCS_DOCUMENTATION_FIRST);
    browser
}

/// RS-1
/// ```gherkin
/// @US-BRA-010 @AC-010.5 @BR-1 @BR-2 @kpi-bra-8 @contract-shape:bounded-change
/// Scenario: A rescan offers only new suggestions, with a summary
///   Given Priya has 3 published claims, 1 pending and 1 declined suggestion
///   And she added a CHANGELOG with semver tags to priyaraman/estuary
///   When she scans again
///   Then she sees "Ownership re-checked ✓ · 1 new suggestion · 3 already published · 1 declined (hidden)"
///   And her queue holds the old pending one plus "priyaraman/estuary embodies semantic-versioning", nothing published or declined
/// ```
#[test]
fn a_rescan_offers_only_new_suggestions_with_a_summary() {
    let world = ReviewWorld::new();
    let mut browser = given_priya_returns_with_3_published_and_1_declined(&world);
    priya_adds_changelog_to_estuary(&world);
    assert_eq!(when_scan_finishes(&mut browser), "completed");
    let text = browser.page.text();
    assert!(text.contains("Ownership re-checked"), "{text}");
    assert!(
        text.contains("1 new")
            && text.contains("3 already published")
            && text.contains("1 declined (hidden)"),
        "{text}"
    );
    assert_eq!(
        browser.page.pending_cards(),
        phrases(&[
            priya::TIDEPOOL_SEMANTIC_VERSIONING,
            priya::ESTUARY_SEMANTIC_VERSIONING
        ])
    );
}

/// RS-2
/// ```gherkin
/// @US-BRA-010 @AC-010.5 @C3 @edge @contract-shape:unbounded-preservation
/// Scenario: A rescan with nothing new says so
///   Given Priya has 3 published claims and 1 declined suggestion and nothing changed on GitHub
///   When she scans again
///   Then she sees "0 new" and her queue is unchanged
/// ```
#[test]
fn a_rescan_with_nothing_new_says_so() {
    let world = ReviewWorld::new();
    let mut browser = given_priya_returns_with_3_published_and_1_declined(&world);
    let before = pending_on_queue(&mut browser);
    assert_eq!(when_scan_finishes(&mut browser), "completed");
    assert!(browser.page.shows("0 new"), "{}", browser.page.text());
    assert_eq!(browser.page.pending_cards(), before);
}

/// RS-3
/// ```gherkin
/// @US-BRA-010 @AC-010.1 @I-BRA-4 @kpi-bra-5 @contract-shape:pure-function
/// Scenario: Ownership is re-checked before every scrape
///   Given Priya's link was verified earlier and she has scanned before
///   When she scans again, twice
///   Then before each scan reads any repo, the app read her GitHub bio and found her DID
/// ```
#[test]
fn ownership_is_re_checked_before_every_scrape() {
    let world = ReviewWorld::new();
    let mut browser = given_pending_suggestions(&world, Persona::Priya);
    assert_eq!(when_scan_finishes(&mut browser), "completed");
    assert_eq!(when_scan_finishes(&mut browser), "completed");
    assert!(world
        .github
        .scrapes_without_preceding_bio_check("priyaraman")
        .is_empty());
    let bio_reads = world
        .github
        .requests()
        .iter()
        .filter(|r| r.is_profile_read_of("priyaraman"))
        .count();
    assert!(
        bio_reads >= 4,
        "verify + three scans each read the bio; saw {bio_reads}"
    );
}

/// RS-4
/// ```gherkin
/// @US-BRA-010 @AC-010.1 @AC-010.2 @AC-010.3 @I-BRA-4 @I-BRA-8 @D-12 @error @contract-shape:unbounded-preservation
/// Scenario: A failed re-check blocks the scan and keeps published claims untouched
///   Given Priya has 3 published claims and 2 pending suggestions
///   And she removed her DID from her GitHub bio
///   When she scans again
///   Then no repo is read and no new suggestions are created
///   And her link shows as unverified with instructions to restore the DID and re-verify
///   And her 3 published claims in her PDS are unchanged
///   And her 2 pending suggestions are hidden, not deleted
/// ```
#[test]
fn a_failed_re_check_blocks_the_scan_and_keeps_published_claims_untouched() {
    let world = ReviewWorld::new();
    let mut browser = given_priya_returns_with_3_published_and_1_declined(&world);
    let before = capture(&world, &mut browser, Persona::Priya);
    let reads_before = world.github.scrape_reads_of("priyaraman");
    removes_did_from_bio(&world, Persona::Priya);
    assert_eq!(when_scan_finishes(&mut browser), "ownership_failed");
    let text = browser.page.text();
    assert!(
        text.contains("Your DID is no longer in github.com/priyaraman's bio, so we didn't scan."),
        "{text}"
    );
    assert!(
        text.contains("Your published claims are untouched")
            && text.contains("hidden until you re-verify"),
        "{text}"
    );
    assert_eq!(
        world.github.scrape_reads_of("priyaraman"),
        reads_before,
        "no repo read"
    );
    let after = capture(&world, &mut browser, Persona::Priya);
    let expected = Delta::new().with_slot(
        "queue.pending",
        Box::new(|_b: &String, a: &String| {
            a.is_empty().then_some(()).ok_or_else(|| {
                format!("pending suggestions are hidden while unverified; still visible: {a}")
            })
        }),
    );
    assert_state_delta(&before, &after, &universe(), &expected);
}

/// RS-5
/// ```gherkin
/// @US-BRA-010 @AC-010.2 @I-BRA-4 @adversarial @contract-shape:unbounded-preservation
/// Scenario: A hidden suggestion cannot be approved while ownership is unproven
///   Given Priya's link became unverified while "priyaraman/tidepool embodies semantic-versioning" was pending
///   When its approval is requested anyway (an old preview form is resubmitted)
///   Then nothing is written to her PDS
/// ```
#[test]
fn a_hidden_suggestion_cannot_be_approved_while_ownership_is_unproven() {
    let world = ReviewWorld::new();
    let mut browser = given_pending_suggestions(&world, Persona::Priya);
    browser.open("/review");
    let approve_form = html::cards_containing(
        &browser.page.html,
        &["priyaraman/tidepool", "semantic-versioning"],
    )
    .first()
    .map(|c| html::forms(c))
    .unwrap_or_default()
    .into_iter()
    .find(|f| {
        f.buttons
            .iter()
            .any(|(l, _, _)| html::label_matches(l, "Approve"))
    })
    .expect("an Approve form");
    removes_did_from_bio(&world, Persona::Priya);
    assert_eq!(when_scan_finishes(&mut browser), "ownership_failed");
    browser.submit(&approve_form, Some("Approve"));
    let _ = browser.try_press("Publish to my repo");
    assert!(claims_in_pds(&world, Persona::Priya).is_empty());
    assert!(world.atproto.write_attempts().is_empty());
}

/// RS-6
/// ```gherkin
/// @US-BRA-010 @AC-010.4 @C2 @contract-shape:bounded-change
/// Scenario: Re-verifying restores scanning and pending suggestions
///   Given Priya's link is unverified and her pending suggestions are hidden
///   When she restores her DID in the bio and verifies
///   Then her pending suggestions are visible again
///   And scanning works again
/// ```
#[test]
fn re_verifying_restores_scanning_and_pending_suggestions() {
    let world = ReviewWorld::new();
    let mut browser = given_pending_suggestions(&world, Persona::Priya);
    let visible = pending_on_queue(&mut browser);
    removes_did_from_bio(&world, Persona::Priya);
    assert_eq!(when_scan_finishes(&mut browser), "ownership_failed");
    assert!(pending_on_queue(&mut browser).is_empty());
    puts_did_in_bio(&world, Persona::Priya);
    when_verifies_github(&mut browser, "priyaraman");
    then_sees_verified(&browser.page, Persona::Priya);
    assert_eq!(pending_on_queue(&mut browser), visible);
    assert_eq!(when_scan_finishes(&mut browser), "completed");
}

// =============================================================================
// US-BRA-011 — retract a claim I published through the app
// =============================================================================

/// RT-1
/// ```gherkin
/// @US-BRA-011 @AC-011.1 @AC-011.2 @AC-011.3 @I-BRA-3 @I-BRA-8 @contract-shape:bounded-change
/// Scenario: Priya retracts a published claim
///   Given Priya published "priyaraman/tidepool embodies test-driven"
///   When she opens Retract on it, reads the preview and confirms
///   Then the preview said "A public retraction referencing this claim will be added to your repo. The original stays readable, marked retracted."
///   And a self-attested retraction referencing the original claim's CID is added to her PDS
///   And the original record is still there, unmodified
///   And her profile no longer lists test-driven
/// ```
#[test]
#[ignore = "DELIVER R2: unskip one-at-a-time (US-BRA-011 retract)"]
fn priya_retracts_a_published_claim() {
    let world = ReviewWorld::new();
    let mut browser = given_published(
        &world,
        Persona::Priya,
        &[priya::TIDEPOOL_TEST_DRIVEN, priya::TIDEPOOL_MEMORY_SAFETY],
    );
    let original = claims_in_pds(&world, Persona::Priya)
        .into_iter()
        .find(|r| r.value["object"] == Philosophy::TestDriven.object().as_str())
        .expect("the published test-driven claim");
    when_opens_retract_preview(&mut browser, Persona::Priya, priya::TIDEPOOL_TEST_DRIVEN);
    assert!(browser.page.shows(
        "A public retraction referencing this claim will be added to your repo. The original stays readable, marked retracted."
    ));
    assert_eq!(
        claims_in_pds(&world, Persona::Priya).len(),
        2,
        "the preview writes nothing"
    );
    browser.press("Confirm retraction");
    let claims = claims_in_pds(&world, Persona::Priya);
    assert_eq!(claims.len(), 3);
    assert!(
        claims.contains(&original),
        "the original is never deleted or modified"
    );
    let retraction = claims.last().expect("the retraction");
    assert_eq!(retraction.value["references"][0]["type"], "retracts");
    assert_eq!(
        retraction.value["references"][0]["cid"],
        original.rkey.as_str()
    );
    assert_eq!(retraction.value["author"], Persona::Priya.did());
    assert_eq!(recomputed_cid(&retraction.value), retraction.rkey);
    let profile = when_opens_profile(&mut browser, Persona::Priya).embodies_phrases();
    assert_eq!(profile, phrases(&[priya::TIDEPOOL_MEMORY_SAFETY]));
}

/// RT-2
/// ```gherkin
/// @US-BRA-011 @AC-011.1 @contract-shape:unbounded-preservation
/// Scenario: Cancelling a retraction writes nothing
///   Given Priya opened the retract preview
///   When she cancels
///   Then nothing is written and the claim is still on her profile
/// ```
#[test]
#[ignore = "DELIVER R2: unskip one-at-a-time (US-BRA-011 cancel retraction)"]
fn cancelling_a_retraction_writes_nothing() {
    let world = ReviewWorld::new();
    let mut browser = given_published(&world, Persona::Priya, &[priya::TIDEPOOL_TEST_DRIVEN]);
    let before = capture(&world, &mut browser, Persona::Priya);
    when_opens_retract_preview(&mut browser, Persona::Priya, priya::TIDEPOOL_TEST_DRIVEN);
    browser.press("Cancel");
    let after = capture(&world, &mut browser, Persona::Priya);
    assert_state_delta(&before, &after, &universe(), &Delta::new());
}

/// RT-3
/// ```gherkin
/// @US-BRA-011 @AC-011.4 @error @infrastructure-failure @contract-shape:unbounded-preservation
/// Scenario: A failed retraction leaves the claim active, with a retry
///   Given Priya's PDS is unreachable
///   When she confirms a retraction
///   Then she sees that nothing was retracted and can retry
///   And her claim is still active and still on her profile once her PDS is back
/// ```
#[test]
#[ignore = "DELIVER R2: unskip one-at-a-time (US-BRA-011 failed retraction)"]
fn a_failed_retraction_leaves_the_claim_active_with_a_retry() {
    let world = ReviewWorld::new();
    let mut browser = given_published(&world, Persona::Priya, &[priya::TIDEPOOL_TEST_DRIVEN]);
    when_opens_retract_preview(&mut browser, Persona::Priya, priya::TIDEPOOL_TEST_DRIVEN);
    world
        .atproto
        .set_write_posture(Persona::Priya.did(), WritePosture::ServerError);
    browser.press("Confirm retraction");
    assert!(browser.page.offers("Retry"), "{}", browser.page.text());
    world
        .atproto
        .set_write_posture(Persona::Priya.did(), WritePosture::Accept);
    assert_eq!(claims_in_pds(&world, Persona::Priya).len(), 1);
    let profile = when_opens_profile(&mut browser, Persona::Priya).embodies_phrases();
    assert_eq!(profile, phrases(&[priya::TIDEPOOL_TEST_DRIVEN]));
}

/// RT-4
/// ```gherkin
/// @US-BRA-011 @AC-011.3 @I-BRA-6 @contract-shape:pure-function
/// Scenario: A retracted claim never appears in the share post
///   Given Priya published memory-safety and test-driven and retracted test-driven
///   When she opens the share preview
///   Then the text names memory-safety only
/// ```
#[test]
#[ignore = "DELIVER R2: unskip one-at-a-time (US-BRA-011 retracted excluded from share)"]
fn a_retracted_claim_never_appears_in_the_share_post() {
    let world = ReviewWorld::new();
    let mut browser = given_published(
        &world,
        Persona::Priya,
        &[priya::TIDEPOOL_TEST_DRIVEN, priya::TIDEPOOL_MEMORY_SAFETY],
    );
    when_opens_retract_preview(&mut browser, Persona::Priya, priya::TIDEPOOL_TEST_DRIVEN);
    browser.press("Confirm retraction");
    when_opens_share_preview(&mut browser, Persona::Priya);
    let text = browser.page.text();
    assert!(
        text.contains("memory-safety") && !text.contains("test-driven"),
        "{text}"
    );
}

/// RT-5
/// ```gherkin
/// @US-BRA-011 @C4a @idempotency @contract-shape:bounded-change
/// Scenario: Retracting the same claim twice adds one retraction
///   Given Priya retracted a claim and the confirmation is submitted again
///   Then her repo holds exactly one retraction for it
/// ```
#[test]
#[ignore = "DELIVER R2: unskip one-at-a-time (retraction exactly once)"]
fn retracting_the_same_claim_twice_adds_one_retraction() {
    let world = ReviewWorld::new();
    let mut browser = given_published(&world, Persona::Priya, &[priya::TIDEPOOL_TEST_DRIVEN]);
    when_opens_retract_preview(&mut browser, Persona::Priya, priya::TIDEPOOL_TEST_DRIVEN);
    let confirm = browser
        .forms()
        .into_iter()
        .find(|f| {
            f.buttons
                .iter()
                .any(|(l, _, _)| html::label_matches(l, "Confirm retraction"))
        })
        .expect("the confirm form");
    browser.press("Confirm retraction");
    browser.submit(&confirm, Some("Confirm retraction"));
    assert_eq!(
        claims_in_pds(&world, Persona::Priya).len(),
        2,
        "one original + one retraction"
    );
}

// =============================================================================
// US-BRA-012 — disconnect and have the app forget me
// =============================================================================

/// FG-1
/// ```gherkin
/// @US-BRA-012 @AC-012.1 @AC-012.2 @AC-012.3 @I-BRA-8 @contract-shape:bounded-change
/// Scenario: Forget me removes everything the app holds, and nothing in her PDS
///   Given Priya has 1 published claim, pending suggestions, 1 decline and a verified GitHub link
///   When she opens "Disconnect and forget me", reads what is deleted and what is kept, and confirms
///   Then she is signed out
///   And her PDS was not written to and still holds her published claim
///   And on her next sign-in she has no pending suggestions, no declines and no GitHub link
/// ```
#[test]
#[ignore = "DELIVER R2: unskip one-at-a-time (US-BRA-012 forget me)"]
fn forget_me_removes_everything_the_app_holds_and_nothing_in_her_pds() {
    let world = ReviewWorld::new();
    let mut browser = given_published(&world, Persona::Priya, &[priya::TIDEPOOL_TEST_DRIVEN]);
    when_declines(&mut browser, priya::QUILL_DOCS_DOCUMENTATION_FIRST);
    let claims_before = claims_in_pds(&world, Persona::Priya);
    let writes_before = world.atproto.write_attempts().len();
    when_opens_forget_me(&mut browser);
    assert!(browser.page.shows(
        "This deletes your pending suggestions, declines and GitHub link from OpenLore review. Claims you published stay in your own repo."
    ));
    browser.press("Yes, forget me");
    assert!(!has_session(&mut browser), "signed out");
    assert_eq!(
        world.atproto.write_attempts().len(),
        writes_before,
        "no PDS call"
    );
    assert_eq!(claims_in_pds(&world, Persona::Priya), claims_before);
    let mut again = given_signed_in(&world, Persona::Priya);
    assert!(pending_on_queue(&mut again).is_empty());
    let github_step = again.open("/github").text();
    assert!(
        !github_step.contains("Verified:"),
        "no GitHub link survives: {github_step}"
    );
    assert!(world.atproto.forbidden_write_attempts().is_empty());
}

/// FG-2
/// ```gherkin
/// @US-BRA-012 @AC-012.4 @contract-shape:unbounded-preservation
/// Scenario: Cancel keeps everything
///   Given Priya opened the forget-me confirmation
///   When she cancels
///   Then nothing is removed and she is still signed in
/// ```
#[test]
#[ignore = "DELIVER R2: unskip one-at-a-time (US-BRA-012 cancel)"]
fn cancelling_forget_me_keeps_everything() {
    let world = ReviewWorld::new();
    let mut browser = given_pending_suggestions(&world, Persona::Priya);
    let before = capture(&world, &mut browser, Persona::Priya);
    when_opens_forget_me(&mut browser);
    browser.press("Cancel");
    assert!(has_session(&mut browser));
    let after = capture(&world, &mut browser, Persona::Priya);
    assert_state_delta(&before, &after, &universe(), &Delta::new());
}

/// FG-3
/// ```gherkin
/// @US-BRA-012 @AC-012.2 @SPIKE-2-finding-1 @contract-shape:bounded-change
/// Scenario: Disconnecting revokes her grant, and an answer of "200 OK" counts as done
///   Given Priya is signed in and her PDS answers revocation with 200 (as the standard says)
///   When she confirms "Disconnect and forget me"
///   Then her grant was revoked at her PDS and her refresh token no longer works
///   And the app keeps no tokens: signing in again needs a fresh authorization
/// ```
#[test]
#[ignore = "DELIVER R2: unskip one-at-a-time (SPIKE-2 finding 1: revoke 200 tolerated, tokens deleted)"]
fn disconnecting_revokes_her_grant_and_an_answer_of_200_counts_as_done() {
    let world = ReviewWorld::new();
    let mut browser = given_signed_in(&world, Persona::Priya);
    when_opens_forget_me(&mut browser);
    browser.press("Yes, forget me");
    let revocations = world.atproto.revocations();
    assert!(!revocations.is_empty(), "the grant was revoked");
    assert!(revocations.iter().all(|r| r.status == 200));
    assert_eq!(world.atproto.live_refresh_tokens(Persona::Priya.did()), 0);
    assert!(
        browser.page.status < 500,
        "disconnect succeeded: {}",
        browser.page.text()
    );
    let exchanges = world.atproto.token_exchanges();
    let _again = given_signed_in(&world, Persona::Priya);
    assert_eq!(
        world.atproto.token_exchanges(),
        exchanges + 1,
        "a fresh authorization was needed"
    );
}

/// FG-4
/// ```gherkin
/// @US-BRA-012 @SPIKE-2-finding-1 @error @infrastructure-failure @contract-shape:bounded-change
/// Scenario: Forget me still forgets when her PDS cannot be reached to revoke
///   Given Priya has pending suggestions and her PDS is unreachable
///   When she confirms "Disconnect and forget me"
///   Then she is signed out and, on her next sign-in, the app holds nothing about her
/// ```
#[test]
#[ignore = "DELIVER R2: unskip one-at-a-time (revoke is best effort; purge always happens)"]
fn forget_me_still_forgets_when_her_pds_cannot_be_reached_to_revoke() {
    let world = ReviewWorld::new();
    let mut browser = given_pending_suggestions(&world, Persona::Priya);
    when_opens_forget_me(&mut browser);
    world.atproto.set_host_reachable(PdsHost::BSKY, false);
    browser.press("Yes, forget me");
    world.atproto.set_host_reachable(PdsHost::BSKY, true);
    assert!(!has_session(&mut browser));
    let mut again = given_signed_in(&world, Persona::Priya);
    assert!(pending_on_queue(&mut again).is_empty());
}

/// FG-5
/// ```gherkin
/// @US-BRA-012 @SPIKE-2-finding-2 @documentation @contract-shape:unbounded-preservation
/// Scenario: After forgetting, the app never uses the access her PDS already issued
///   Given Priya disconnected and the access token her PDS issued has not yet expired
///   Then her PDS would still accept that access token for its remaining lifetime (the documented residual window)
///   And the app makes no further call to her PDS of any kind
/// ```
#[test]
#[ignore = "DELIVER R2: unskip one-at-a-time (SPIKE-2 finding 2: residual access-token window)"]
fn after_forgetting_the_app_never_uses_the_access_her_pds_already_issued() {
    let world = ReviewWorld::new();
    let mut browser = given_published(&world, Persona::Priya, &[priya::TIDEPOOL_TEST_DRIVEN]);
    when_opens_forget_me(&mut browser);
    browser.press("Yes, forget me");
    assert!(
        world
            .atproto
            .access_token_still_accepted(Persona::Priya.did()),
        "the residual window exists (SPIKE-2 finding 2) — only discarding tokens protects her"
    );
    let writes = world.atproto.write_attempts().len();
    browser.open("/review");
    browser.open(&format!("/@{}", Persona::Priya.handle()));
    assert_eq!(
        world.atproto.write_attempts().len(),
        writes,
        "no PDS write after forgetting"
    );
}

/// FG-6
/// ```gherkin
/// @US-BRA-012 @DV-BRA-10 @operator @contract-shape:bounded-change
/// Scenario: The operator can forget a person on request, without their session
///   Given Priya has pending suggestions and asked the operator to remove her
///   When the operator purges her DID through the loopback admin listener
///   Then on her next sign-in the app holds nothing about her and her PDS is untouched
///   And the public listener does not offer the admin routes
/// ```
#[test]
#[ignore = "DELIVER R2: unskip one-at-a-time (operator purge via admin listener)"]
fn the_operator_can_forget_a_person_on_request_without_their_session() {
    let world = ReviewWorld::new();
    let _browser = given_pending_suggestions(&world, Persona::Priya);
    assert_eq!(
        world.app.get("/admin/purge").0,
        404,
        "admin routes are loopback-admin only"
    );
    let (status, _) = world.app.admin_post("/admin/purge", Persona::Priya.did());
    assert_eq!(status, 200);
    let mut again = given_signed_in(&world, Persona::Priya);
    assert!(pending_on_queue(&mut again).is_empty());
    assert!(world.atproto.write_attempts().is_empty());
}
