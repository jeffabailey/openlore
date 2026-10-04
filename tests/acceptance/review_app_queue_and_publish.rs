//! bluesky-claim-review-app — the private queue and approve-to-publish
//! (US-BRA-003, US-BRA-004; walking-skeleton release).
//!
//! Focused scenarios behind WS-3/WS-4: scan outcomes (ownership re-check,
//! empty result, GitHub busy with partial results kept), the exact-record
//! preview (Plan-value pattern: preview == write), and every publish failure
//! (PDS unreachable / erroring / session expired) leaving the suggestion
//! pending with no partial record.
//!
//! Driving port: the REAL `openlore-review-app` over HTTP. Fakes: ATProto
//! network + GitHub. Layer 4: example-only (Mandate 9/11). `#[ignore]`d for
//! one-at-a-time DELIVER.

#[path = "support/review_app/mod.rs"]
mod review_app;

use openlore_test_support::WritePosture;
use review_app::state_delta::{assert_state_delta, Delta};
use review_app::*;

// =============================================================================
// US-BRA-003 — the private suggestion queue
// =============================================================================

/// QP-1
/// ```gherkin
/// @US-BRA-003 @AC-003.2 @BR-3 @contract-shape:pure-function
/// Scenario: Every suggestion card carries its evidence
///   Given Priya has verified github.com/priyaraman
///   When the scan finishes
///   Then each of her 5 cards shows the subject, the philosophy, 0.25 with its "speculative" label,
///        the signal that produced it and at least one evidence link into her repo
///   And her fork of serde produced no card
/// ```
#[test]
#[ignore = "DELIVER WS: unskip one-at-a-time (US-BRA-003 evidence-backed cards)"]
fn every_suggestion_card_carries_its_signal_and_evidence() {
    let world = ReviewWorld::new();
    let browser = given_pending_suggestions(&world, Persona::Priya);
    for s in priya::FIRST_SCAN {
        let card =
            html::cards_containing(&browser.page.html, &[&s.repo_path(), s.philosophy.slug()])
                .first()
                .map(|c| c.to_string())
                .unwrap_or_else(|| panic!("a card for {}", s.phrase()));
        let text = html::visible_text(&card);
        assert!(
            text.contains("0.25") && text.contains("speculative"),
            "{}: {text}",
            s.phrase()
        );
        assert!(
            text.contains("Why:"),
            "the producing signal is explained: {text}"
        );
        assert!(
            card.contains(&format!("https://github.com/{}/", s.repo_path())),
            "an evidence link into {}: {card}",
            s.repo_path()
        );
    }
    assert!(
        !browser.page.text().contains("priyaraman/serde"),
        "forks are skipped (BR-3)"
    );
}

/// QP-2
/// ```gherkin
/// @US-BRA-003 @AC-003.1 @I-BRA-4 @error @contract-shape:unbounded-preservation
/// Scenario: Ownership is re-checked right before the first scan
///   Given Priya verified github.com/priyaraman and then removed her DID from the bio
///   When the scan is about to start
///   Then no repo is read and no suggestion is created
///   And she sees how to restore her DID and verify again
/// ```
#[test]
#[ignore = "DELIVER WS: unskip one-at-a-time (US-BRA-003 re-check before the first scan)"]
fn ownership_is_re_checked_right_before_the_first_scan() {
    let world = ReviewWorld::new();
    let mut browser = given_github_verified(&world, Persona::Priya);
    removes_did_from_bio(&world, Persona::Priya);
    let status = when_scan_finishes(&mut browser);
    assert_eq!(status, "ownership_failed");
    assert_eq!(world.github.scrape_reads_of("priyaraman"), 0);
    assert!(browser.page.pending_cards().is_empty());
    let text = browser.page.text();
    assert!(
        text.contains("no longer in github.com/priyaraman's bio") && text.contains("verify"),
        "{text}"
    );
}

/// QP-3
/// ```gherkin
/// @US-BRA-003 @AC-003.6 @edge @contract-shape:unbounded-preservation
/// Scenario: An empty result guides Aisha
///   Given Aisha's GitHub has only forked repos
///   When her scan finishes
///   Then she sees that no suggestions were found and that forks and archived repos are skipped
/// ```
#[test]
#[ignore = "DELIVER WS: unskip one-at-a-time (US-BRA-003 empty result guidance)"]
fn an_empty_scan_explains_that_forks_and_archived_repos_are_skipped() {
    let world = ReviewWorld::new();
    let mut aisha = given_github_verified(&world, Persona::Aisha);
    assert_eq!(when_scan_finishes(&mut aisha), "completed");
    assert!(aisha.page.pending_cards().is_empty());
    assert!(
        aisha.page.shows("We didn't find suggestions in your owned, public repos. Forks and archived repos are skipped"),
        "{}",
        aisha.page.text()
    );
}

/// QP-4
/// ```gherkin
/// @US-BRA-003 @AC-003.7 @infrastructure-failure @C7a @contract-shape:bounded-change
/// Scenario: When GitHub is busy, partial results are kept and the scan resumes
///   Given Priya has verified github.com/priyaraman
///   And GitHub's remaining request budget is below the app's floor after her first repo
///   When the scan runs
///   Then it stops with "GitHub is busy, resuming at …" and the suggestions found so far are kept
///   And when the budget recovers and she resumes, the rest arrive without duplicates
/// ```
#[test]
#[ignore = "DELIVER WS: unskip one-at-a-time (US-BRA-003 rate-limit floor keeps partial results)"]
fn when_github_is_busy_partial_results_are_kept_and_the_scan_resumes() {
    let world = ReviewWorld::new();
    let mut browser = given_github_verified(&world, Persona::Priya);
    world.github.set_remaining(120);
    let status = when_scan_finishes(&mut browser);
    assert_eq!(status, "rate_limited");
    assert!(
        browser.page.shows("GitHub is busy, resuming at"),
        "{}",
        browser.page.text()
    );
    let partial = browser.page.pending_cards();
    world.github.set_remaining(4_990);
    assert_eq!(when_scan_finishes(&mut browser), "completed");
    let all = browser.page.pending_cards();
    assert_eq!(
        all,
        phrases(&priya::FIRST_SCAN),
        "no duplicates, nothing lost"
    );
    assert!(
        partial.iter().all(|p| all.contains(p)),
        "partial results were kept"
    );
}

/// QP-5
/// ```gherkin
/// @US-BRA-003 @AC-003.8 @NFR-BRA-6 @accessibility @contract-shape:pure-function
/// Scenario: Every review page is navigable by keyboard with labelled controls
///   Given Priya has pending suggestions
///   When she opens the start page, the GitHub step, her queue and a preview
///   Then every page has a descriptive title and a language
///   And every input has a label and no action is removed from the keyboard order
///   And the queue documents the A/E/N/J/K triage keys
/// ```
#[test]
#[ignore = "DELIVER WS: unskip one-at-a-time (NFR-BRA-6 keyboard + labels)"]
fn every_review_page_is_navigable_by_keyboard_with_labelled_controls() {
    let world = ReviewWorld::new();
    let mut browser = given_pending_suggestions(&world, Persona::Priya);
    let mut pages = Vec::new();
    for path in ["/", "/github", "/review"] {
        pages.push(browser.open(path).clone());
    }
    pages.push(when_previews_approval(&mut browser, priya::TIDEPOOL_DEPENDENCY_PINNING).clone());
    for page in &pages {
        let title = html::elements(&page.html, "title")
            .first()
            .map(|t| html::visible_text(t))
            .unwrap_or_default();
        assert!(title.len() > 3, "a descriptive title on {}", page.url);
        assert!(
            page.html.contains("<html lang="),
            "a page language on {}",
            page.url
        );
        for (_, input) in html::opening_tags(&page.html, "input") {
            if html::attr(input, "type").as_deref() == Some("hidden") {
                continue;
            }
            let id = html::attr(input, "id").unwrap_or_default();
            let labelled = html::attr(input, "aria-label").is_some()
                || (!id.is_empty() && page.html.contains(&format!("for=\"{id}\"")));
            assert!(labelled, "an unlabelled input on {}: {input}", page.url);
        }
        assert!(
            !page.html.contains("tabindex=\"-1\""),
            "no action is removed from tab order on {}",
            page.url
        );
    }
    assert!(
        pages[2].html.contains("data-triage-keys"),
        "the queue documents its A/E/N/J/K triage keys"
    );
}

// =============================================================================
// US-BRA-004 — approve into my own PDS
// =============================================================================

/// QP-6
/// ```gherkin
/// @US-BRA-004 @AC-004.1 @AC-004.4 @AC-004.5 @NFR-BRA-7 @I-BRA-3 @contract-shape:unbounded-preservation
/// Scenario: The preview shows exactly what will be written, and writes nothing
///   Given Priya has the pending suggestion "priyaraman/tidepool embodies dependency-pinning"
///   When she presses Approve
///   Then she sees the destination (her repo on her PDS), every field to be written,
///        confidence as 0.25 and as stored 2500, provenance "self-attested", and the words "not as truth"
///   And nothing is written until she confirms
/// ```
#[test]
#[ignore = "DELIVER WS: unskip one-at-a-time (US-BRA-004 exact-record preview)"]
fn the_preview_shows_exactly_what_will_be_written_and_writes_nothing() {
    let world = ReviewWorld::new();
    let mut browser = given_pending_suggestions(&world, Persona::Priya);
    let before = capture(&world, &mut browser, Persona::Priya);
    when_previews_approval(&mut browser, priya::TIDEPOOL_DEPENDENCY_PINNING);
    let text = browser.page.text();
    for expected in [
        Persona::Priya.did(),
        "github:priyaraman/tidepool",
        "embodiesPhilosophy",
        "org.openlore.philosophy.dependency-pinning",
        "https://github.com/priyaraman/tidepool/blob/master/Cargo.lock",
        "0.25",
        "2500",
        "self-attested",
        "not as truth",
    ] {
        assert!(
            text.contains(expected),
            "preview shows {expected:?}:\n{text}"
        );
    }
    let after = capture(&world, &mut browser, Persona::Priya);
    assert_state_delta(&before, &after, &universe(), &Delta::new());
}

/// QP-7
/// ```gherkin
/// @US-BRA-004 @AC-004.4 @BR-4 @property @contract-shape:bounded-change
/// Scenario: The published record is the previewed record, field for field
///   Given Priya previewed "priyaraman/tidepool embodies test-driven"
///   When she confirms "Publish to my repo"
///   Then the record in her PDS carries exactly the subject, predicate, object, evidence and confidence the preview showed
///   And no display label is stored
/// ```
#[test]
#[ignore = "DELIVER WS: unskip one-at-a-time (US-BRA-004 preview == record)"]
fn the_published_record_is_the_previewed_record_field_for_field() {
    let world = ReviewWorld::new();
    let mut browser = given_pending_suggestions(&world, Persona::Priya);
    let preview = when_previews_approval(&mut browser, priya::TIDEPOOL_TEST_DRIVEN).text();
    when_confirms_publish(&mut browser);
    let record = then_pds_holds_self_attested_claim(
        &world,
        Persona::Priya,
        &priya::TIDEPOOL_TEST_DRIVEN.subject(),
        Philosophy::TestDriven,
        2500,
    );
    for field in ["subject", "predicate", "object", "composedAt"] {
        let value = record.value[field].as_str().unwrap_or_default().to_string();
        assert!(
            preview.contains(&value),
            "the preview showed the written {field} {value:?}"
        );
    }
    for evidence in record.value["evidence"].as_array().expect("evidence list") {
        assert!(
            preview.contains(evidence.as_str().unwrap_or_default()),
            "evidence {evidence} was previewed"
        );
    }
    let keys: Vec<&String> = record.value.as_object().unwrap().keys().collect();
    for key in &keys {
        assert!(
            [
                "$type",
                "subject",
                "predicate",
                "object",
                "evidence",
                "confidence",
                "author",
                "composedAt",
                "references"
            ]
            .contains(&key.as_str()),
            "only lexicon keys are written; found {key}"
        );
    }
}

/// QP-8
/// ```gherkin
/// @US-BRA-004 @AC-004.5 @AC-004.7 @kpi-bra-1 @contract-shape:bounded-change
/// Scenario: The confirmation names the record and the way back, and the queue moves on
///   Given Priya has 5 pending suggestions
///   When she publishes "priyaraman/quill-docs embodies documentation-first"
///   Then she sees its at:// address and how to retract it
///   And her queue shows the 4 remaining suggestions and 1 published
/// ```
#[test]
#[ignore = "DELIVER WS: unskip one-at-a-time (US-BRA-004 confirmation + queue counts)"]
fn the_confirmation_names_the_record_and_the_way_back_and_the_queue_moves_on() {
    let world = ReviewWorld::new();
    let mut browser = given_pending_suggestions(&world, Persona::Priya);
    when_previews_approval(&mut browser, priya::QUILL_DOCS_DOCUMENTATION_FIRST);
    let confirmation = when_confirms_publish(&mut browser).text();
    let record = claims_in_pds(&world, Persona::Priya)
        .pop()
        .expect("a record");
    assert!(confirmation.contains(&record.uri()));
    assert!(confirmation.to_lowercase().contains("retract"));
    let remaining = pending_on_queue(&mut browser);
    assert_eq!(remaining.len(), 4);
    assert!(!remaining.contains(&priya::QUILL_DOCS_DOCUMENTATION_FIRST.phrase()));
    assert!(browser.page.shows("1 published"), "{}", browser.page.text());
}

/// QP-9
/// ```gherkin
/// @US-BRA-004 @AC-004.6 @contract-shape:unbounded-preservation
/// Scenario: Backing out of the preview writes nothing
///   Given Priya is on the preview of a suggestion
///   When she presses Back
///   Then nothing is written and the suggestion is still pending
/// ```
#[test]
#[ignore = "DELIVER WS: unskip one-at-a-time (US-BRA-004 Back writes nothing)"]
fn backing_out_of_the_preview_writes_nothing() {
    let world = ReviewWorld::new();
    let mut browser = given_pending_suggestions(&world, Persona::Priya);
    let before = capture(&world, &mut browser, Persona::Priya);
    when_previews_approval(&mut browser, priya::TIDEPOOL_DEPENDENCY_PINNING);
    browser.press("Back");
    let after = capture(&world, &mut browser, Persona::Priya);
    assert_state_delta(&before, &after, &universe(), &Delta::new());
}

/// QP-10 — parametrised over the ways a publish can fail at the PDS.
/// ```gherkin
/// @US-BRA-004 @AC-004.6 @NFR-BRA-5 @error @infrastructure-failure @C6b @contract-shape:unbounded-preservation
/// Scenario Outline: A failed publish leaves the suggestion pending with no partial record
///   Given Priya's PDS <fails>
///   When she confirms publishing "priyaraman/tidepool embodies dependency-pinning"
///   Then she sees "We couldn't reach your PDS. Nothing was published." (or the matching reason) with a retry
///   And the suggestion is still pending and her repo holds no record
///   Examples: | is unreachable | answers with a server error | has expired her session |
/// ```
#[test]
#[ignore = "DELIVER WS: unskip one-at-a-time (US-BRA-004 failed publish stays pending)"]
fn a_failed_publish_leaves_the_suggestion_pending_with_no_partial_record() {
    enum Failure {
        Unreachable,
        ServerError,
        ExpiredSession,
    }
    for failure in [
        Failure::Unreachable,
        Failure::ServerError,
        Failure::ExpiredSession,
    ] {
        let world = ReviewWorld::new();
        let mut browser = given_pending_suggestions(&world, Persona::Priya);
        when_previews_approval(&mut browser, priya::TIDEPOOL_DEPENDENCY_PINNING);
        let expected_message = match failure {
            Failure::Unreachable => {
                world.atproto.set_host_reachable(PdsHost::BSKY, false);
                "We couldn't reach your PDS. Nothing was published."
            }
            Failure::ServerError => {
                world
                    .atproto
                    .set_write_posture(Persona::Priya.did(), WritePosture::ServerError);
                "Nothing was published."
            }
            Failure::ExpiredSession => {
                world
                    .atproto
                    .set_write_posture(Persona::Priya.did(), WritePosture::ExpiredSession);
                "Sign in again"
            }
        };
        when_confirms_publish(&mut browser);
        assert!(
            browser.page.shows(expected_message),
            "{}",
            browser.page.text()
        );
        assert!(browser.page.offers("Retry") || browser.page.offers("Sign in with Bluesky"));
        world.atproto.set_host_reachable(PdsHost::BSKY, true);
        assert!(
            claims_in_pds(&world, Persona::Priya).is_empty(),
            "no partial record"
        );
        assert!(
            pending_on_queue(&mut browser).contains(&priya::TIDEPOOL_DEPENDENCY_PINNING.phrase())
        );
    }
}

/// QP-11
/// ```gherkin
/// @US-BRA-004 @AC-004.6 @C4a @contract-shape:bounded-change
/// Scenario: Retrying after a failure publishes exactly once
///   Given Priya's publish failed because her PDS was unreachable
///   When her PDS is back and she presses Retry
///   Then her repo holds exactly one record for the suggestion
/// ```
#[test]
#[ignore = "DELIVER WS: unskip one-at-a-time (US-BRA-004 retry is exactly-once)"]
fn retrying_after_a_failure_publishes_exactly_once() {
    let world = ReviewWorld::new();
    let mut browser = given_pending_suggestions(&world, Persona::Priya);
    when_previews_approval(&mut browser, priya::TIDEPOOL_DEPENDENCY_PINNING);
    world.atproto.set_host_reachable(PdsHost::BSKY, false);
    when_confirms_publish(&mut browser);
    world.atproto.set_host_reachable(PdsHost::BSKY, true);
    browser.press("Retry");
    then_pds_holds_self_attested_claim(
        &world,
        Persona::Priya,
        &priya::TIDEPOOL_DEPENDENCY_PINNING.subject(),
        Philosophy::DependencyPinning,
        2500,
    );
}

/// QP-12
/// ```gherkin
/// @US-BRA-004 @I-BRA-3 @C4a @adversarial @contract-shape:bounded-change
/// Scenario: Confirming the same preview twice writes one record
///   Given Priya confirmed "priyaraman/tidepool embodies dependency-pinning"
///   When the same confirmation is submitted again (double click, back-and-resubmit)
///   Then her repo still holds exactly one record for it
/// ```
#[test]
#[ignore = "DELIVER WS: unskip one-at-a-time (plan taken exactly once)"]
fn confirming_the_same_preview_twice_writes_one_record() {
    let world = ReviewWorld::new();
    let mut browser = given_pending_suggestions(&world, Persona::Priya);
    when_previews_approval(&mut browser, priya::TIDEPOOL_DEPENDENCY_PINNING);
    let confirm_form = browser
        .forms()
        .into_iter()
        .find(|f| {
            f.buttons
                .iter()
                .any(|(l, _, _)| html::label_matches(l, "Publish to my repo"))
        })
        .expect("the confirm form");
    when_confirms_publish(&mut browser);
    browser.submit(&confirm_form, Some("Publish to my repo"));
    assert_eq!(claims_in_pds(&world, Persona::Priya).len(), 1);
}

/// QP-13
/// ```gherkin
/// @US-BRA-004 @I-BRA-3 @adversarial @C6b @contract-shape:unbounded-preservation
/// Scenario: A confirmation for something never previewed is refused
///   Given Priya is signed in with pending suggestions
///   When a confirmation arrives for a record she never previewed
///   Then it is refused and nothing is written
/// ```
#[test]
#[ignore = "DELIVER WS: unskip one-at-a-time (no write without a stored plan)"]
fn a_confirmation_for_something_never_previewed_is_refused() {
    let world = ReviewWorld::new();
    let mut browser = given_pending_suggestions(&world, Persona::Priya);
    let token = csrf_token(&browser);
    browser.post(
        "/plans/bafyreinever0previewed0000000000000000000000000000000000/confirm",
        &[("csrf", token.as_str())],
    );
    assert!(
        browser.page.status == 404 || browser.page.status == 400,
        "status {}",
        browser.page.status
    );
    assert!(world.atproto.write_attempts().is_empty());
}

/// QP-14
/// ```gherkin
/// @US-BRA-004 @AC-004.2 @I-BRA-7 @contract-shape:bounded-change
/// Scenario: Dmitri's approval lands in his self-hosted PDS
///   Given Dmitri has verified github.com/dvolkov and has a pending suggestion
///   When he publishes "dvolkov/ferrite embodies dependency-pinning"
///   Then the claim is in his repo on his own PDS
///   And no write reached any other PDS
/// ```
#[test]
#[ignore = "DELIVER WS: unskip one-at-a-time (US-BRA-004 self-hosted destination)"]
fn dmitris_approval_lands_in_his_self_hosted_pds() {
    let world = ReviewWorld::new();
    let mut browser = given_pending_suggestions(&world, Persona::Dmitri);
    when_previews_approval(&mut browser, dmitri::FERRITE_DEPENDENCY_PINNING);
    when_confirms_publish(&mut browser);
    then_pds_holds_self_attested_claim(
        &world,
        Persona::Dmitri,
        &dmitri::FERRITE_DEPENDENCY_PINNING.subject(),
        Philosophy::DependencyPinning,
        2500,
    );
    assert!(world
        .atproto
        .write_attempts()
        .iter()
        .all(|w| w.host == PdsHost::VOLKOV));
}

/// QP-15
/// ```gherkin
/// @US-BRA-004 @NFR-BRA-5 @SPIKE-2-finding-4 @contract-shape:bounded-change
/// Scenario: Publishing still works after the PDS access has quietly expired
///   Given Priya has pending suggestions
///   And the access her PDS granted has expired since she signed in
///   When she publishes a suggestion
///   Then it lands in her PDS without asking her to sign in again
/// ```
#[test]
#[ignore = "DELIVER WS: unskip one-at-a-time (session restore/refresh path)"]
fn publishing_still_works_after_the_pds_access_has_quietly_expired() {
    let world = ReviewWorld::new();
    let mut browser = given_pending_suggestions(&world, Persona::Priya);
    world.atproto.expire_access_tokens(Persona::Priya.did());
    when_previews_approval(&mut browser, priya::TIDEPOOL_MEMORY_SAFETY);
    when_confirms_publish(&mut browser);
    then_pds_holds_self_attested_claim(
        &world,
        Persona::Priya,
        &priya::TIDEPOOL_MEMORY_SAFETY.subject(),
        Philosophy::MemorySafety,
        2500,
    );
}

/// QP-16
/// ```gherkin
/// @US-BRA-003 @NFR-BRA-5 @C7b @contract-shape:unbounded-preservation
/// Scenario: Her queue survives signing out and the app restarting
///   Given Priya has 5 pending suggestions and declined one
///   When she signs out, the app restarts, and she signs in again
///   Then she sees the same 4 pending suggestions and the decline still holds
/// ```
#[test]
#[ignore = "DELIVER WS: unskip one-at-a-time (pending + declined survive sign-out and restart)"]
fn her_queue_survives_signing_out_and_the_app_restarting() {
    let world = ReviewWorld::new();
    let mut browser = given_pending_suggestions(&world, Persona::Priya);
    when_declines(&mut browser, priya::QUILL_DOCS_DOCUMENTATION_FIRST);
    let before = pending_on_queue(&mut browser);
    when_signs_out(&mut browser);
    let world = world.restart_app();
    let mut again = given_signed_in(&world, Persona::Priya);
    assert_eq!(pending_on_queue(&mut again), before);
    assert_eq!(before.len(), 4);
}

/// QP-17
/// ```gherkin
/// @US-BRA-003 @ADR-076 @C1b @error @contract-shape:unbounded-preservation
/// Scenario: The seventh scan in a day is politely refused
///   Given Priya has scanned 6 times today
///   When she starts another scan
///   Then she is told to come back tomorrow, and no repo is read
/// ```
#[test]
#[ignore = "DELIVER WS: unskip one-at-a-time (ADR-076 per-DID daily scan budget boundary)"]
fn the_seventh_scan_in_a_day_is_politely_refused() {
    let world = ReviewWorld::new();
    let mut browser = given_github_verified(&world, Persona::Priya);
    for _ in 0..6 {
        assert_eq!(when_scan_finishes(&mut browser), "completed");
    }
    let reads = world.github.scrape_reads_of("priyaraman");
    browser.open("/review");
    let _ = browser.try_press("Scan again");
    assert!(browser.page.shows("tomorrow"), "{}", browser.page.text());
    assert_eq!(world.github.scrape_reads_of("priyaraman"), reads);
}

/// QP-18
/// ```gherkin
/// @US-BRA-003 @C7c @concurrency @contract-shape:bounded-change
/// Scenario: Two people scanning at the same time each get only their own suggestions
///   Given Priya and Dmitri have both verified their GitHub accounts
///   When both scans run at the same time
///   Then Priya sees only her repos' suggestions and Dmitri only his
/// ```
#[test]
#[ignore = "DELIVER WS: unskip one-at-a-time (concurrent scans stay owner-scoped)"]
fn two_people_scanning_at_the_same_time_each_get_only_their_own_suggestions() {
    let world = ReviewWorld::new();
    let mut priya_browser = given_github_verified(&world, Persona::Priya);
    let mut dmitri_browser = given_github_verified(&world, Persona::Dmitri);
    let (p, d) = std::thread::scope(|scope| {
        let p = scope.spawn(|| when_scan_finishes(&mut priya_browser));
        let d = scope.spawn(|| when_scan_finishes(&mut dmitri_browser));
        (p.join().unwrap(), d.join().unwrap())
    });
    assert_eq!((p.as_str(), d.as_str()), ("completed", "completed"));
    assert_eq!(
        priya_browser.page.pending_cards(),
        phrases(&priya::FIRST_SCAN)
    );
    assert_eq!(
        dmitri_browser.page.pending_cards(),
        phrases(&[dmitri::FERRITE_DEPENDENCY_PINNING])
    );
}
