//! bluesky-claim-review-app — RELEASE 1 "Consent with control, then show it off"
//! (US-BRA-005 edit, US-BRA-006 decline privately, US-BRA-007 public profile,
//! US-BRA-008 opt-in share post).
//!
//! Driving port: the REAL `openlore-review-app` over HTTP (owner pages, the
//! anonymous `/@handle` profile). Fakes: ATProto network + GitHub. Layer 4:
//! example-only; enumerated sad paths (Mandate 11). `#[ignore]`d for
//! one-at-a-time DELIVER (Release 1 milestone).

#[path = "support/review_app/mod.rs"]
mod review_app;

use openlore_test_support::WritePosture;
use review_app::state_delta::{assert_state_delta, set_to, Delta};
use review_app::*;

// =============================================================================
// US-BRA-005 — edit confidence or swap the philosophy before approving
// =============================================================================

/// ED-1 — parametrised over (philosophy swap, typed confidence) key examples.
/// ```gherkin
/// @US-BRA-005 @AC-005.1 @AC-005.2 @AC-005.3 @BR-4 @C1b @contract-shape:bounded-change
/// Scenario Outline: Priya publishes her edited claim
///   Given Priya edits the tidepool dependency-pinning suggestion to <philosophy> at <typed>
///   When she previews and confirms
///   Then the preview showed "<bucket>" and <stored>
///   And her PDS holds the claim with object <philosophy> and confidence <stored>, and no bucket
///   Examples:
///     | philosophy    | typed | stored | bucket         |
///     | memory-safety | 0.70  | 7000   | well-evidenced |
///     | test-driven   | 1.00  | 10000  | triangulated   |
///     | (unchanged)   | 0.00  | 0      | speculative    |
/// ```
#[test]
fn priya_publishes_her_edited_claim() {
    let cases = [
        (
            Some(Philosophy::MemorySafety),
            ConfidenceEntry::new("0.70", 7000, "well-evidenced"),
        ),
        (
            Some(Philosophy::TestDriven),
            ConfidenceEntry::new("1.00", 10_000, "triangulated"),
        ),
        (None, ConfidenceEntry::new("0.00", 0, "speculative")),
    ];
    for (philosophy, confidence) in cases {
        let world = ReviewWorld::new();
        let mut browser = given_pending_suggestions(&world, Persona::Priya);
        let preview = when_edits_and_previews(
            &mut browser,
            priya::TIDEPOOL_DEPENDENCY_PINNING,
            philosophy,
            Some(confidence.typed),
        )
        .text();
        assert!(
            preview.contains(confidence.bucket),
            "the bucket label is shown: {preview}"
        );
        assert!(preview.contains(&confidence.basis_points.to_string()));
        when_confirms_publish(&mut browser);
        then_pds_holds_self_attested_claim(
            &world,
            Persona::Priya,
            &priya::TIDEPOOL_DEPENDENCY_PINNING.subject(),
            philosophy.unwrap_or(Philosophy::DependencyPinning),
            confidence.basis_points,
        );
    }
}

/// ED-2 — parametrised over invalid confidence inputs.
/// ```gherkin
/// @US-BRA-005 @AC-005.4 @NFR-BRA-6 @error @C6a @contract-shape:unbounded-preservation
/// Scenario Outline: An invalid confidence is caught with guidance and blocks approval
///   Given Priya is editing a suggestion
///   When she enters "<typed>" and leaves the field
///   Then she sees "Enter a number from 0.00 to 1.00" and cannot approve until it is fixed
///   And nothing is written
///   Examples: | 1.5 | -0.1 | abc | 0.555 | (empty) | 1,0 |
/// ```
#[test]
fn an_invalid_confidence_is_caught_with_guidance_and_blocks_approval() {
    let world = ReviewWorld::new();
    let mut browser = given_pending_suggestions(&world, Persona::Priya);
    for typed in ["1.5", "-0.1", "abc", "0.555", "", "1,0"] {
        when_edits_and_previews(
            &mut browser,
            priya::TIDEPOOL_DEPENDENCY_PINNING,
            None,
            Some(typed),
        );
        assert!(
            browser.page.shows("Enter a number from 0.00 to 1.00"),
            "{typed:?} must be refused with guidance:\n{}",
            browser.page.text()
        );
        assert!(
            !browser.page.offers("Publish to my repo"),
            "approval is blocked for {typed:?}"
        );
    }
    assert!(world.atproto.write_attempts().is_empty());
}

/// ED-3
/// ```gherkin
/// @US-BRA-005 @AC-005.5 @contract-shape:unbounded-preservation
/// Scenario: Cancelling an edit restores the suggestion
///   Given Priya changed the philosophy in Edit
///   When she presses Cancel
///   Then the card shows the original suggestion unchanged and nothing is written
/// ```
#[test]
fn cancelling_an_edit_restores_the_suggestion() {
    let world = ReviewWorld::new();
    let mut browser = given_pending_suggestions(&world, Persona::Priya);
    let before = capture(&world, &mut browser, Persona::Priya);
    browser.open("/review");
    browser.press_in_card(&["priyaraman/tidepool", "dependency-pinning"], "Edit");
    browser.fill("object", &Philosophy::MemorySafety.object());
    browser.press("Cancel");
    let after = capture(&world, &mut browser, Persona::Priya);
    assert_state_delta(&before, &after, &universe(), &Delta::new());
}

/// ED-4
/// ```gherkin
/// @US-BRA-005 @AC-005.1 @BR-5 @contract-shape:pure-function
/// Scenario: Every vocabulary philosophy is offered when swapping
///   Given Priya is editing a suggestion
///   Then she can choose any philosophy of the OpenLore vocabulary
/// ```
#[test]
fn every_vocabulary_philosophy_is_offered_when_swapping() {
    let world = ReviewWorld::new();
    let mut browser = given_pending_suggestions(&world, Persona::Priya);
    browser.open("/review");
    browser.press_in_card(&["priyaraman/tidepool", "dependency-pinning"], "Edit");
    let options: Vec<String> = html::opening_tags(&browser.page.html, "option")
        .into_iter()
        .filter_map(|(_, o)| html::attr(o, "value"))
        .collect();
    for philosophy in Philosophy::ALL {
        assert!(
            options.contains(&philosophy.object()),
            "{} offered: {options:?}",
            philosophy.slug()
        );
    }
    // The whole seeded vocabulary (J-002e / ADR-059) is offered, not just the five.
    for seeded in lexicon::philosophy::seeds() {
        let object = lexicon::philosophy::object_id(&seeded.name);
        assert!(options.contains(&object), "{object} offered");
    }
}

// =============================================================================
// US-BRA-006 — decline privately so it never comes back
// =============================================================================

/// DC-1
/// ```gherkin
/// @US-BRA-006 @AC-006.1 @AC-006.2 @I-BRA-2 @contract-shape:bounded-change
/// Scenario: Declining writes nothing public
///   Given Priya has the pending suggestion "priyaraman/quill-docs embodies documentation-first"
///   When she chooses "Not me"
///   Then she sees "Declined. Private: never published, won't be suggested again." with Undo
///   And no record of any kind is written to any PDS
///   And only that card left her queue
/// ```
#[test]
#[ignore = "DELIVER R1: unskip one-at-a-time (US-BRA-006 decline writes nothing)"]
fn declining_writes_nothing_public() {
    let world = ReviewWorld::new();
    let mut browser = given_pending_suggestions(&world, Persona::Priya);
    let before = capture(&world, &mut browser, Persona::Priya);
    when_declines(&mut browser, priya::QUILL_DOCS_DOCUMENTATION_FIRST);
    assert!(browser
        .page
        .shows("Declined. Private: never published, won't be suggested again."));
    assert!(browser.page.offers("Undo"));
    let after = capture(&world, &mut browser, Persona::Priya);
    let remaining = phrases(&priya::FIRST_SCAN[..4]);
    let expected = Delta::new().with_slot("queue.pending", set_to(remaining.join("\n")));
    assert_state_delta(&before, &after, &universe(), &expected);
}

/// DC-2
/// ```gherkin
/// @US-BRA-006 @AC-006.4 @BR-1 @contract-shape:unbounded-preservation
/// Scenario: A declined suggestion is not offered again
///   Given Priya declined "priyaraman/quill-docs embodies documentation-first"
///   When her repos are scanned again and the same signal is found
///   Then that suggestion is not offered
/// ```
#[test]
#[ignore = "DELIVER R1: unskip one-at-a-time (US-BRA-006 no re-offer)"]
fn a_declined_suggestion_is_not_offered_again() {
    let world = ReviewWorld::new();
    let mut browser = given_pending_suggestions(&world, Persona::Priya);
    when_declines(&mut browser, priya::QUILL_DOCS_DOCUMENTATION_FIRST);
    assert_eq!(when_scan_finishes(&mut browser), "completed");
    assert!(!browser
        .page
        .pending_cards()
        .contains(&priya::QUILL_DOCS_DOCUMENTATION_FIRST.phrase()));
}

/// DC-3
/// ```gherkin
/// @US-BRA-006 @AC-006.3 @I-BRA-2 @I-BRA-1 @adversarial @contract-shape:unbounded-preservation
/// Scenario: Nobody else can see Priya's declines
///   Given Priya declined 1 suggestion
///   When Dmitri or an anonymous visitor looks at Priya's profile or her PDS records
///   Then there is no trace of the decline
/// ```
#[test]
#[ignore = "DELIVER R1: unskip one-at-a-time (US-BRA-006 declines invisible to others)"]
fn nobody_else_can_see_priyas_declines() {
    let world = ReviewWorld::new();
    let mut priya_browser = given_pending_suggestions(&world, Persona::Priya);
    when_declines(&mut priya_browser, priya::QUILL_DOCS_DOCUMENTATION_FIRST);
    let mut dmitri_browser = given_signed_in(&world, Persona::Dmitri);
    let mut anonymous = world.browser();
    for browser in [&mut dmitri_browser, &mut anonymous] {
        let profile = when_opens_profile(browser, Persona::Priya).text();
        assert!(
            !profile.contains("documentation-first") && !profile.contains("Declined"),
            "{profile}"
        );
        let queue = browser.open("/review").text();
        assert!(!queue.contains("quill-docs"), "{queue}");
    }
    assert!(world
        .atproto
        .all_records()
        .iter()
        .all(|r| !r.value.to_string().contains("quill-docs")));
}

/// DC-4
/// ```gherkin
/// @US-BRA-006 @AC-006.5 @C4b @contract-shape:bounded-change
/// Scenario: Undo restores the suggestion
///   Given Priya just declined a suggestion
///   When she presses Undo
///   Then the suggestion is pending again and nothing was written
/// ```
#[test]
#[ignore = "DELIVER R1: unskip one-at-a-time (US-BRA-006 undo)"]
fn undo_restores_a_declined_suggestion() {
    let world = ReviewWorld::new();
    let mut browser = given_pending_suggestions(&world, Persona::Priya);
    let before = capture(&world, &mut browser, Persona::Priya);
    when_declines(&mut browser, priya::QUILL_DOCS_DOCUMENTATION_FIRST);
    browser.press("Undo");
    let after = capture(&world, &mut browser, Persona::Priya);
    assert_state_delta(&before, &after, &universe(), &Delta::new());
}

/// DC-5
/// ```gherkin
/// @US-BRA-006 @C4a @idempotency @contract-shape:bounded-change
/// Scenario: Declining twice is the same as declining once
///   Given Priya declined a suggestion and the decline form is submitted a second time
///   Then exactly that one suggestion is declined and nothing is written
/// ```
#[test]
#[ignore = "DELIVER R1: unskip one-at-a-time (decline idempotent)"]
fn declining_twice_is_the_same_as_declining_once() {
    let world = ReviewWorld::new();
    let mut browser = given_pending_suggestions(&world, Persona::Priya);
    browser.open("/review");
    let form = html::cards_containing(
        &browser.page.html,
        &["priyaraman/quill-docs", "documentation-first"],
    )
    .first()
    .map(|card| html::forms(card))
    .unwrap_or_default()
    .into_iter()
    .find(|f| {
        f.buttons
            .iter()
            .any(|(l, _, _)| html::label_matches(l, "Not me"))
    })
    .expect("the Not me form");
    browser.submit(&form, Some("Not me"));
    browser.submit(&form, Some("Not me"));
    assert_eq!(
        pending_on_queue(&mut browser),
        phrases(&priya::FIRST_SCAN[..4])
    );
    assert!(world.atproto.write_attempts().is_empty());
}

// =============================================================================
// US-BRA-007 — my public profile shows only claims I approved
// =============================================================================

/// GIVEN Priya has 2 published claims, 3 pending suggestions and 1 declined
/// (the canonical R1 state; chains given_published + when_declines).
fn given_priya_with_2_published_3_pending_1_declined(world: &ReviewWorld) -> Browser {
    let mut browser = given_published(
        world,
        Persona::Priya,
        &[priya::TIDEPOOL_MEMORY_SAFETY, priya::TIDEPOOL_TEST_DRIVEN],
    );
    when_declines(&mut browser, priya::QUILL_DOCS_DOCUMENTATION_FIRST);
    browser
}

/// PR-1
/// ```gherkin
/// @US-BRA-007 @AC-007.1 @AC-007.2 @AC-007.5 @I-BRA-1 @I-BRA-5 @contract-shape:pure-function
/// Scenario: The profile shows only published claims, each labelled self-attested
///   Given Priya has 2 published claims, 3 pending suggestions and 1 declined suggestion
///   When anyone opens Priya's profile page
///   Then they see exactly the 2 published claims, each labelled self-attested with 0.25 (speculative)
///   And nothing pending or declined is mentioned
/// ```
#[test]
#[ignore = "DELIVER R1: unskip one-at-a-time (US-BRA-007 published only)"]
fn the_profile_shows_only_published_claims_each_labelled_self_attested() {
    let world = ReviewWorld::new();
    let _priya = given_priya_with_2_published_3_pending_1_declined(&world);
    let mut visitor = world.browser();
    let page = when_opens_profile(&mut visitor, Persona::Priya).clone();
    assert_eq!(
        page.embodies_phrases(),
        phrases(&[priya::TIDEPOOL_MEMORY_SAFETY, priya::TIDEPOOL_TEST_DRIVEN])
    );
    let cards = html::elements(&page.html, "article");
    assert_eq!(cards.len(), 2);
    for card in cards {
        let text = html::visible_text(card);
        assert!(
            text.contains("self-attested") && text.contains("0.25") && text.contains("speculative"),
            "{text}"
        );
        assert!(!text.contains("unverified"));
    }
    for hidden in [
        "dependency-pinning",
        "semantic-versioning",
        "documentation-first",
        "Declined",
    ] {
        assert!(
            !page.text().contains(hidden),
            "{hidden} never on the profile"
        );
    }
}

/// PR-2
/// ```gherkin
/// @US-BRA-007 @AC-007.3 @edge @C3 @contract-shape:pure-function
/// Scenario: An empty profile is honest, with a call to action for its owner
///   Given Aisha has published nothing
///   When a visitor opens her profile
///   Then they see "Aisha hasn't published any claims yet." (or her handle in place of the name)
///   And when Aisha herself opens it she also sees "Review suggestions →"
/// ```
#[test]
#[ignore = "DELIVER R1: unskip one-at-a-time (US-BRA-007 empty profile)"]
fn an_empty_profile_is_honest_with_a_call_to_action_for_its_owner() {
    let world = ReviewWorld::new();
    let mut visitor = world.browser();
    let text = when_opens_profile(&mut visitor, Persona::Aisha).text();
    assert!(text.contains("hasn't published any claims yet"), "{text}");
    assert!(!text.contains("Review suggestions"));
    let mut aisha = given_signed_in(&world, Persona::Aisha);
    when_opens_profile(&mut aisha, Persona::Aisha);
    assert!(aisha.page.offers("Review suggestions →"));
}

/// PR-3
/// ```gherkin
/// @US-BRA-007 @AC-007.4 @error @infrastructure-failure @C7a @contract-shape:pure-function
/// Scenario: An unreachable PDS is stated, never replaced by stale claims
///   Given Priya published a claim and her PDS is now unreachable
///   When a visitor opens her profile
///   Then they see "We can't reach this person's PDS right now" and no claims
/// ```
#[test]
#[ignore = "DELIVER R1: unskip one-at-a-time (US-BRA-007 unreachable PDS stated)"]
fn an_unreachable_pds_is_stated_never_replaced_by_stale_claims() {
    let world = ReviewWorld::new();
    let _priya = given_published(&world, Persona::Priya, &[priya::TIDEPOOL_MEMORY_SAFETY]);
    world.atproto.set_host_reachable(PdsHost::BSKY, false);
    let mut visitor = world.browser();
    let page = when_opens_profile(&mut visitor, Persona::Priya).clone();
    assert!(
        page.shows("We can't reach this person's PDS right now"),
        "{}",
        page.text()
    );
    assert!(page.embodies_phrases().is_empty(), "no stale claims");
}

/// PR-4
/// ```gherkin
/// @US-BRA-007 @OD-BRA-8 @error @C6a @contract-shape:pure-function
/// Scenario: A profile is found by handle or DID, and an unknown handle is plainly not found
///   Given Priya published a claim
///   Then her profile opens at /@priyaraman.bsky.social and at /@did:plc:7x3kq2mzv5rj4w6hbn2tqclp alike
///   And /@nobody.invalid answers "not found" without an error page
/// ```
#[test]
#[ignore = "DELIVER R1: unskip one-at-a-time (profile by handle or DID)"]
fn a_profile_is_found_by_handle_or_did_and_an_unknown_handle_is_plainly_not_found() {
    let world = ReviewWorld::new();
    let _priya = given_published(&world, Persona::Priya, &[priya::TIDEPOOL_MEMORY_SAFETY]);
    let mut visitor = world.browser();
    let by_handle = when_opens_profile(&mut visitor, Persona::Priya).embodies_phrases();
    let by_did = visitor
        .open(&format!("/@{}", Persona::Priya.did()))
        .embodies_phrases();
    assert_eq!(by_handle, by_did);
    let unknown = visitor.open("/@nobody.invalid").clone();
    assert_eq!(unknown.status, 404);
    assert!(unknown.text().to_lowercase().contains("not found"));
}

// =============================================================================
// US-BRA-008 — share my profile on Bluesky, only if I choose to
// =============================================================================

/// SH-1
/// ```gherkin
/// @US-BRA-008 @AC-008.1 @AC-008.2 @AC-008.3 @I-BRA-3 @I-BRA-6 @kpi-bra-7 @contract-shape:bounded-change
/// Scenario: Priya previews, edits and confirms her post
///   Given Priya has published memory-safety and test-driven
///   When she opens "Share on Bluesky…"
///   Then she sees an editable post that names memory-safety and test-driven and links to her profile
///   And nothing has been posted yet
///   And when she adds "Feedback welcome!" and presses "Post to Bluesky", exactly that post appears in her repo,
///       with a link to her profile
/// ```
#[test]
#[ignore = "DELIVER R1: unskip one-at-a-time (US-BRA-008 preview, edit, post)"]
fn priya_previews_edits_and_confirms_her_post() {
    let world = ReviewWorld::new();
    let mut browser = given_priya_with_2_published_3_pending_1_declined(&world);
    when_opens_share_preview(&mut browser, Persona::Priya);
    let draft = browser
        .forms()
        .iter()
        .flat_map(|f| f.fields.clone())
        .find(|(k, _)| k == "text")
        .map(|(_, v)| v)
        .expect("an editable post text");
    assert!(
        draft.contains("memory-safety") && draft.contains("test-driven"),
        "{draft}"
    );
    let profile_url = format!("{}/@{}", world.app.origin(), Persona::Priya.handle());
    assert!(
        browser.page.html.contains(&profile_url),
        "the preview shows the profile link"
    );
    assert!(
        posts_in_pds(&world, Persona::Priya).is_empty(),
        "nothing posted before confirm"
    );
    browser.fill("text", &format!("{draft} Feedback welcome!"));
    browser.press("Post to Bluesky");
    let posts = posts_in_pds(&world, Persona::Priya);
    assert_eq!(posts.len(), 1);
    let post = &posts[0].value;
    assert!(post["text"]
        .as_str()
        .unwrap_or_default()
        .ends_with("Feedback welcome!"));
    let facet_uri = post["facets"][0]["features"][0]["uri"]
        .as_str()
        .unwrap_or_default();
    assert_eq!(facet_uri, profile_url, "the post links to her profile");
    assert!(browser.page.shows("Posted."), "{}", browser.page.text());
}

/// SH-2 — parametrised over the two ways of not posting.
/// ```gherkin
/// @US-BRA-008 @AC-008.4 @I-BRA-6 @contract-shape:unbounded-preservation
/// Scenario Outline: Nothing is posted without consent
///   Given Priya opens the share preview
///   When she <declines>
///   Then nothing is posted to Bluesky and nothing else changes
///   Examples: | presses "Don't post" | leaves the page |
/// ```
#[test]
#[ignore = "DELIVER R1: unskip one-at-a-time (US-BRA-008 no consent, no post)"]
fn nothing_is_posted_without_consent() {
    for leaves_instead in [false, true] {
        let world = ReviewWorld::new();
        let mut browser = given_published(&world, Persona::Priya, &[priya::TIDEPOOL_MEMORY_SAFETY]);
        let before = capture(&world, &mut browser, Persona::Priya);
        when_opens_share_preview(&mut browser, Persona::Priya);
        if leaves_instead {
            browser.open("/review");
        } else {
            browser.press("Don't post");
        }
        let after = capture(&world, &mut browser, Persona::Priya);
        assert_state_delta(&before, &after, &universe(), &Delta::new());
    }
}

/// SH-3
/// ```gherkin
/// @US-BRA-008 @AC-008.3 @I-BRA-1b @I-BRA-6 @contract-shape:pure-function
/// Scenario: The post never mentions unapproved items
///   Given Priya has 2 published claims, 3 pending suggestions and 1 declined suggestion
///   When she opens the share preview
///   Then the generated text names only the 2 published philosophies
/// ```
#[test]
#[ignore = "DELIVER R1: unskip one-at-a-time (US-BRA-008 approved-only text)"]
fn the_post_never_mentions_unapproved_items() {
    let world = ReviewWorld::new();
    let mut browser = given_priya_with_2_published_3_pending_1_declined(&world);
    when_opens_share_preview(&mut browser, Persona::Priya);
    let text = browser.page.text();
    for unapproved in [
        "dependency-pinning",
        "semantic-versioning",
        "documentation-first",
    ] {
        assert!(
            !text.contains(unapproved),
            "{unapproved} must not be in the share preview:\n{text}"
        );
    }
}

/// SH-4
/// ```gherkin
/// @US-BRA-008 @AC-008.5 @edge @C3 @contract-shape:unbounded-preservation
/// Scenario: Sharing is unavailable with nothing published
///   Given Aisha has no published claims
///   When she views her profile
///   Then no share action is offered, with a hint to review suggestions first
///   And a share request sent anyway posts nothing
/// ```
#[test]
#[ignore = "DELIVER R1: unskip one-at-a-time (US-BRA-008 nothing to share)"]
fn sharing_is_unavailable_with_nothing_published() {
    let world = ReviewWorld::new();
    let mut aisha = given_signed_in(&world, Persona::Aisha);
    when_opens_profile(&mut aisha, Persona::Aisha);
    assert!(!aisha.page.offers("Share on Bluesky…"));
    assert!(aisha.page.offers("Review suggestions →"));
    let token = csrf_token(&aisha);
    aisha.post(
        "/share",
        &[("csrf", token.as_str()), ("text", "How I build")],
    );
    assert!(posts_in_pds(&world, Persona::Aisha).is_empty());
}

/// SH-5
/// ```gherkin
/// @US-BRA-008 @AC-008.6 @error @infrastructure-failure @contract-shape:unbounded-preservation
/// Scenario: A failed post is explained and retryable, and the profile is unaffected
///   Given Priya's post is refused by her PDS
///   When she presses "Post to Bluesky"
///   Then she sees that the post wasn't published and how to retry
///   And her profile and published claims are unchanged
/// ```
#[test]
#[ignore = "DELIVER R1: unskip one-at-a-time (US-BRA-008 failed post)"]
fn a_failed_post_is_explained_and_retryable_and_the_profile_is_unaffected() {
    let world = ReviewWorld::new();
    let mut browser = given_published(&world, Persona::Priya, &[priya::TIDEPOOL_MEMORY_SAFETY]);
    let profile_before = when_opens_profile(&mut browser, Persona::Priya).embodies_phrases();
    let claims_before = claims_in_pds(&world, Persona::Priya);
    world.atproto.set_write_posture(
        Persona::Priya.did(),
        WritePosture::RefuseCollection(openlore_test_support::POST_COLLECTION.to_string()),
    );
    when_opens_share_preview(&mut browser, Persona::Priya);
    browser.press("Post to Bluesky");
    assert!(
        browser.page.shows("wasn't published"),
        "{}",
        browser.page.text()
    );
    assert!(browser.page.offers("Retry") || browser.page.offers("Post to Bluesky"));
    assert!(posts_in_pds(&world, Persona::Priya).is_empty());
    assert_eq!(claims_in_pds(&world, Persona::Priya), claims_before);
    assert_eq!(
        when_opens_profile(&mut browser, Persona::Priya).embodies_phrases(),
        profile_before
    );
}

/// SH-6
/// ```gherkin
/// @US-BRA-008 @AC-008.1 @C1b @boundary @contract-shape:unbounded-preservation
/// Scenario: A post longer than Bluesky allows is caught before anything is sent
///   Given Priya is on the share preview
///   When she makes the text 301 characters long and presses "Post to Bluesky"
///   Then she is told the post is too long and nothing is posted
/// ```
#[test]
#[ignore = "DELIVER R1: unskip one-at-a-time (300-grapheme limit)"]
fn a_post_longer_than_bluesky_allows_is_caught_before_anything_is_sent() {
    let world = ReviewWorld::new();
    let mut browser = given_published(&world, Persona::Priya, &[priya::TIDEPOOL_MEMORY_SAFETY]);
    when_opens_share_preview(&mut browser, Persona::Priya);
    browser.fill("text", &"é".repeat(301));
    browser.press("Post to Bluesky");
    assert!(
        browser.page.shows("300"),
        "names the limit:\n{}",
        browser.page.text()
    );
    assert!(posts_in_pds(&world, Persona::Priya).is_empty());
}
