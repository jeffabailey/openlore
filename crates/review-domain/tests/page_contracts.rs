//! Page contracts asserted at the crate boundary (DELIVER Phase 5): every
//! page is a whole document that shows what it was given — the anti-forgery
//! token and plan id on each state-changing form, the reason of each
//! failure, the address of each written record, the way back.

use ports::{ScanCounts, ScanRun, ScanStatus, Suggestion, SuggestionKey};
use review_domain::lifecycle::Tally;
use review_domain::ownership::OwnershipRefusal;
use review_domain::plans::{publish_plan, retract_plan, ClaimDraft, PublishPlan};
use review_domain::signin::{PermissionMode, SignInFailure};
use review_domain::views::*;

const OWNER: &str = "did:plc:owner";
const CSRF: &str = "csrf-token-7";
const PROFILE_PATH: &str = "/@priyaraman.bsky.social";
const AT_URI: &str = "at://did:plc:owner/org.openlore.claim/bafywritten";
const BACK_TO_QUEUE: &str = r#"<a href="/review">Back to your review queue</a>"#;
const MEMORY_SAFETY: &str = "org.openlore.philosophy.memory-safety";

fn key() -> SuggestionKey {
    SuggestionKey {
        subject: "github:priyaraman/tidepool".to_string(),
        predicate: "embodiesPhilosophy".to_string(),
        object: MEMORY_SAFETY.to_string(),
    }
}

fn plan() -> PublishPlan {
    let draft = ClaimDraft {
        key: key(),
        evidence: vec!["https://github.com/priyaraman/tidepool/".to_string()],
        confidence_bp: 2500,
    };
    publish_plan(OWNER, &draft, "2026-10-04T15:02:11Z").expect("plan")
}

fn suggestion() -> Suggestion {
    Suggestion {
        key: key(),
        confidence_bp: 2500,
        evidence: vec!["https://github.com/priyaraman/tidepool/".to_string()],
        why: vec!["Cargo.lock pins every dependency".to_string()],
        source_repo: "priyaraman/tidepool".to_string(),
    }
}

fn csrf_input() -> String {
    format!(r#"<input type="hidden" name="csrf" value="{CSRF}">"#)
}

fn plan_input(plan_id: &str) -> String {
    format!(r#"<input type="hidden" name="plan" value="{plan_id}">"#)
}

fn back_to_profile() -> String {
    format!(r#"<a href="{PROFILE_PATH}">Back to your profile</a>"#)
}

fn button(label: &str) -> String {
    format!(r#"<button type="submit">{label}</button>"#)
}

/// A whole document whose `<h1>` (or first heading text) is `heading`.
fn assert_page(html: &str, heading: &str, shows: &[&str]) {
    assert!(html.starts_with("<!DOCTYPE html>"), "{html}");
    assert!(html.contains(heading), "missing heading {heading}: {html}");
    for shown in shows {
        assert!(html.contains(shown), "missing {shown}: {html}");
    }
}

fn headline_of_tidepool() -> [&'static str; 2] {
    [
        r#"<a href="https://github.com/priyaraman/tidepool/">priyaraman/tidepool</a>"#,
        " embodies memory-safety",
    ]
}

// ------------------------------------------------------------ helpers' text

#[test]
fn a_confidence_shows_its_display_bucket() {
    for (basis_points, bucket) in [
        (0, "speculative"),
        (2500, "speculative"),
        (5000, "weighted"),
        (8000, "well-evidenced"),
        (10_000, "triangulated"),
    ] {
        assert_eq!(bucket_label(basis_points), bucket, "{basis_points}");
    }
}

#[test]
fn a_clock_shows_hours_and_minutes_utc() {
    assert_eq!(utc_clock(13 * 3600 + 7 * 60 + 59), "13:07");
    assert_eq!(utc_clock(86_400 + 3600 + 60), "01:01");
    assert_eq!(utc_clock(-60), "23:59");
}

// ------------------------------------------------------------ simple pages

#[test]
fn the_not_found_pages_say_so_and_lead_back() {
    assert_page(&not_found_page(), "<h1>Not found</h1>", &[BACK_TO_QUEUE]);
    assert_page(
        &profile_not_found_page(),
        "<h1>Profile not found</h1>",
        &[r#"<a href="/">"#],
    );
    assert_page(
        &scan_status_page("running"),
        r#"data-scan-status="running""#,
        &[BACK_TO_QUEUE],
    );
}

#[test]
fn the_settings_pages_name_the_account_and_carry_the_token() {
    assert_page(
        &settings_page("priyaraman.bsky.social"),
        "<h1>Settings</h1>",
        &[
            "Signed in as @priyaraman.bsky.social",
            FORGET_ME_LABEL,
            BACK_TO_QUEUE,
        ],
    );
    assert_page(
        &forget_me_page(CSRF),
        "<h1>Disconnect and forget me?</h1>",
        &[
            &csrf_input(),
            &button(CONFIRM_FORGET_ME_LABEL),
            FORGET_ME_NOTICE,
        ],
    );
    assert_page(
        &forget_me_failed_page(),
        "<h1>We could not finish forgetting you</h1>",
        &[FORGET_ME_LABEL],
    );
}

#[test]
fn the_landing_page_explains_and_shows_the_sign_in_failure() {
    let failures = [
        SignInFailure::HandleNotFound,
        SignInFailure::TemporarilyUnavailable,
        SignInFailure::Cancelled,
        SignInFailure::ExchangeFailed,
        SignInFailure::AccountMismatch,
        SignInFailure::ReturnNotRecognised,
    ];
    let messages: std::collections::BTreeSet<&str> = failures
        .iter()
        .map(|f| sign_in_failure_message(*f))
        .collect();
    assert_eq!(
        messages.len(),
        failures.len(),
        "each failure is explained distinctly"
    );
    for failure in failures {
        let message = sign_in_failure_message(failure);
        assert!(
            message.contains("Bluesky") || message.contains("Nothing changed"),
            "{message}"
        );
        assert_page(
            &landing_page(PermissionMode::BroadFallback, Some(failure)),
            "<h1>OpenLore review</h1>",
            &[message, BROAD_PERMISSION_NOTICE, &button(SIGN_IN_LABEL)],
        );
    }
}

// ------------------------------------------------------------ github

#[test]
fn the_github_page_shows_the_did_the_token_and_the_message() {
    assert_page(
        &github_page(OWNER, CSRF, Some("Verified: yes")),
        "<h1>Prove your GitHub account is yours</h1>",
        &[
            OWNER,
            &csrf_input(),
            "Verified: yes",
            &button(VERIFY_LABEL),
            BACK_TO_QUEUE,
        ],
    );
}

#[test]
fn each_ownership_outcome_is_explained_with_the_account_it_concerns() {
    let verified = ownership_message(
        OwnershipOutcome::Verified,
        "priyaraman",
        OWNER,
        "priyaraman.bsky.social",
    );
    assert!(
        verified.contains("github.com/priyaraman") && verified.contains("@priyaraman.bsky.social")
    );
    let different = OwnershipRefusal::DifferentDid("did:plc:someone".into());
    let message = ownership_message(
        OwnershipOutcome::Refused(&different),
        "priyaraman",
        OWNER,
        "h",
    );
    assert!(
        message.contains("did:plc:someone") && message.contains(OWNER),
        "{message}"
    );
    let missing = ownership_message(
        OwnershipOutcome::Refused(&OwnershipRefusal::DidMissing),
        "priyaraman",
        OWNER,
        "h",
    );
    assert!(
        missing.contains("github.com/priyaraman") && missing.contains("Verify"),
        "{missing}"
    );
}

// ------------------------------------------------------------ publish

#[test]
fn the_approval_preview_shows_the_exact_record_and_confirms_its_plan() {
    let publish = plan();
    let [repo_link, embodies] = headline_of_tidepool();
    assert_page(
        &approval_preview_page(&publish, CSRF),
        "<h1>Preview before publishing</h1>",
        &[
            OWNER,
            &publish.at_uri(),
            &plan_input(publish.rkey()),
            &csrf_input(),
            &button(PUBLISH_LABEL),
            "0.25 (speculative)",
            repo_link,
            embodies,
            NOT_AS_TRUTH_NOTICE,
        ],
    );
}

#[test]
fn the_publish_outcome_pages_show_the_record_or_the_reason_and_next_step() {
    assert_page(
        &published_page(AT_URI),
        "<h1>Published to your repo</h1>",
        &[AT_URI, BACK_TO_QUEUE],
    );
    let retry = PublishRetry {
        plan_id: "plan-9",
        csrf_token: CSRF,
    };
    assert_page(
        &publish_failed_page("PDS refused", PublishNextStep::Retry(retry)),
        "<h1>Nothing was published</h1>",
        &[
            "PDS refused",
            &plan_input("plan-9"),
            &csrf_input(),
            &button(RETRY_LABEL),
            BACK_TO_QUEUE,
        ],
    );
    assert_page(
        &publish_failed_page("session ended", PublishNextStep::SignInAgain),
        "<h1>Nothing was published</h1>",
        &["session ended", SIGN_IN_LABEL],
    );
    assert_page(
        &plan_expired_page(),
        "<h1>This preview has expired</h1>",
        &["30 minutes", BACK_TO_QUEUE],
    );
}

// ------------------------------------------------------------ retract

#[test]
fn the_retract_pages_show_the_retraction_and_lead_back_to_the_profile() {
    let publish = plan();
    let retraction = retract_plan(
        OWNER,
        publish.rkey(),
        publish.claim(),
        "2026-10-05T09:00:00Z",
    )
    .expect("retract");
    let [repo_link, embodies] = headline_of_tidepool();
    assert_page(
        &retract_preview_page(&RetractPreview {
            plan: &retraction,
            csrf_token: CSRF,
            profile_path: PROFILE_PATH,
        }),
        "<h1>Retract this claim?</h1>",
        &[
            &retraction.at_uri(),
            retraction.retracted_cid(),
            &plan_input(retraction.rkey()),
            &csrf_input(),
            &button(CONFIRM_RETRACTION_LABEL),
            &format!(r#"<a href="{PROFILE_PATH}">"#),
            repo_link,
            embodies,
        ],
    );
    assert_page(
        &retracted_page(Some(AT_URI), PROFILE_PATH),
        "<h1>Retracted</h1>",
        &[AT_URI, &back_to_profile()],
    );
    assert_page(
        &retracted_page(None, PROFILE_PATH),
        "<h1>Retracted</h1>",
        &["already retracted", &back_to_profile()],
    );
    let retry = PublishRetry {
        plan_id: "plan-r",
        csrf_token: CSRF,
    };
    assert_page(
        &retract_failed_page("PDS down", Some(retry), PROFILE_PATH),
        "<h1>Nothing was retracted</h1>",
        &[
            "PDS down",
            &plan_input("plan-r"),
            &csrf_input(),
            &back_to_profile(),
        ],
    );
}

// ------------------------------------------------------------ share

#[test]
fn the_share_pages_show_the_draft_the_link_and_the_outcome() {
    let preview = SharePreview {
        plan_id: "share-1",
        csrf_token: CSRF,
        text: "How I build",
        profile_url: "https://openlore.example/@p",
        profile_path: PROFILE_PATH,
        notice: Some("try again"),
    };
    assert_page(
        &share_preview_page(&preview),
        "<h1>Share on Bluesky</h1>",
        &[
            &plan_input("share-1"),
            &csrf_input(),
            "How I build",
            "https://openlore.example/@p",
            PROFILE_PATH,
            "try again",
        ],
    );
    assert_page(
        &share_posted_page(AT_URI, PROFILE_PATH),
        "<h1>Posted.</h1>",
        &[AT_URI, &back_to_profile()],
    );
    assert_page(
        &share_unavailable_page("nothing yet"),
        "<h1>Nothing to share</h1>",
        &["nothing yet"],
    );
    let notice = share_failed_notice("PDS refused.");
    assert!(
        notice.contains("PDS refused.") && notice.contains(POST_LABEL),
        "{notice}"
    );
}

// ------------------------------------------------------------ queue

fn verified_queue(
    latest_scan: Option<ScanRun>,
    pending: &[Suggestion],
    tally: Tally,
    notice: Option<QueueNotice<'_>>,
) -> String {
    review_page(&QueueView {
        handle: "priyaraman.bsky.social",
        csrf_token: CSRF,
        github: GithubStep::Verified {
            login: "priyaraman",
        },
        latest_scan,
        pending,
        tally,
        notice,
    })
}

fn run(status: ScanStatus) -> ScanRun {
    ScanRun {
        status,
        resume_after: Some(13 * 3600 + 7 * 60),
        counts: ScanCounts {
            new: 3,
            already_published: 0,
            declined_hidden: 0,
        },
    }
}

#[test]
fn a_pending_suggestion_is_a_card_with_its_evidence_and_three_actions() {
    let pending = [suggestion()];
    let html = verified_queue(
        Some(run(ScanStatus::Completed)),
        &pending,
        Tally::default(),
        None,
    );
    let [repo_link, embodies] = headline_of_tidepool();
    for shown in [
        repo_link,
        embodies,
        "Confidence 0.25 (speculative)",
        "Why: Cargo.lock pins every dependency",
        r#"<input type="hidden" name="subject" value="github:priyaraman/tidepool">"#,
        r#"<input type="hidden" name="object" value="org.openlore.philosophy.memory-safety">"#,
        r#"<form method="post" action="/review/approve">"#,
        r#"<button type="submit" data-key="a">Approve</button>"#,
        r#"<button type="submit" data-key="e">Edit</button>"#,
        r#"<button type="submit" data-key="n">Not me</button>"#,
    ] {
        assert!(html.contains(shown), "missing {shown}: {html}");
    }
    assert_eq!(
        html.matches(&csrf_input()).count(),
        5,
        "three card forms, scan, sign out"
    );
}

#[test]
fn the_scan_action_follows_the_latest_scan() {
    let label_shown = |latest: Option<ScanRun>| {
        [SCAN_LABEL, SCAN_AGAIN_LABEL, RESUME_SCAN_LABEL]
            .into_iter()
            .filter(|label| {
                verified_queue(latest, &[], Tally::default(), None).contains(&button(label))
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(label_shown(None), vec![SCAN_LABEL]);
    assert_eq!(
        label_shown(Some(run(ScanStatus::Running))),
        Vec::<&str>::new()
    );
    assert_eq!(
        label_shown(Some(run(ScanStatus::RateLimited))),
        vec![RESUME_SCAN_LABEL]
    );
    assert_eq!(
        label_shown(Some(run(ScanStatus::Interrupted))),
        vec![RESUME_SCAN_LABEL]
    );
    assert_eq!(
        label_shown(Some(run(ScanStatus::Completed))),
        vec![SCAN_AGAIN_LABEL]
    );
    assert_eq!(
        label_shown(Some(run(ScanStatus::OwnershipFailed))),
        vec![SCAN_AGAIN_LABEL]
    );
}

#[test]
fn only_a_completed_scan_shows_its_summary() {
    let summary = scan_summary(run(ScanStatus::Completed).counts);
    assert!(verified_queue(
        Some(run(ScanStatus::Completed)),
        &[],
        Tally::default(),
        None
    )
    .contains(&summary));
    for status in [
        ScanStatus::Running,
        ScanStatus::RateLimited,
        ScanStatus::Interrupted,
        ScanStatus::OwnershipFailed,
    ] {
        assert!(
            !verified_queue(Some(run(status)), &[], Tally::default(), None).contains(&summary),
            "{status:?}"
        );
    }
}

#[test]
fn a_paused_scan_says_when_it_resumes() {
    let html = verified_queue(
        Some(run(ScanStatus::RateLimited)),
        &[],
        Tally::default(),
        None,
    );
    assert!(html.contains("resuming at 13:07 UTC"), "{html}");
}

#[test]
fn a_queue_with_cards_on_screen_never_says_all_were_reviewed() {
    let pending = [suggestion()];
    let tally = Tally {
        pending: 0,
        declined: 1,
        published: 1,
        retracted: 0,
    };
    let html = verified_queue(Some(run(ScanStatus::Completed)), &pending, tally, None);
    assert!(!html.contains(&all_reviewed_notice(&tally)), "{html}");
    let unsettled = verified_queue(
        Some(run(ScanStatus::Completed)),
        &pending,
        Tally::default(),
        None,
    );
    assert!(!unsettled.contains(EMPTY_SCAN_NOTICE), "{unsettled}");
}

#[test]
fn a_refused_scan_and_a_decline_are_reported_on_the_queue() {
    let refusals = [
        ScanRefused::AlreadyScanning,
        ScanRefused::DailyLimitReached,
        ScanRefused::AppBusy,
    ];
    let messages: std::collections::BTreeSet<&str> =
        refusals.iter().map(|r| scan_refused_message(*r)).collect();
    assert_eq!(messages.len(), refusals.len());
    for refused in refusals {
        let html = verified_queue(
            None,
            &[],
            Tally::default(),
            Some(QueueNotice::ScanRefused(refused)),
        );
        assert!(
            html.contains(scan_refused_message(refused))
                && scan_refused_message(refused).contains("scan")
        );
    }
    let declined = key();
    let html = verified_queue(
        None,
        &[],
        Tally::default(),
        Some(QueueNotice::Declined(&declined)),
    );
    for shown in [
        DECLINED_NOTICE,
        r#"<form method="post" action="/review/undo">"#,
        r#"<input type="hidden" name="subject" value="github:priyaraman/tidepool">"#,
        &button(UNDO_LABEL),
    ] {
        assert!(html.contains(shown), "missing {shown}: {html}");
    }
}

#[test]
fn the_edit_page_offers_each_philosophy_by_slug_and_shows_the_guidance() {
    let suggestion = suggestion();
    let choices = vec![
        MEMORY_SAFETY.to_string(),
        "org.openlore.philosophy.test-driven".to_string(),
    ];
    let html = edit_page(&EditView {
        suggestion: &suggestion,
        csrf_token: CSRF,
        choices: &choices,
        chosen: MEMORY_SAFETY,
        typed_confidence: "0.4",
        guidance: Some("Use 0.00 to 1.00"),
    });
    let [repo_link, embodies] = headline_of_tidepool();
    assert_page(
        &html,
        "<h1>Edit before approving</h1>",
        &[
            &csrf_input(),
            r#"<option value="org.openlore.philosophy.memory-safety" selected>memory-safety</option>"#,
            r#"<option value="org.openlore.philosophy.test-driven">test-driven</option>"#,
            r#"value="0.4""#,
            "Use 0.00 to 1.00",
            repo_link,
            embodies,
            &button(PREVIEW_LABEL),
        ],
    );
}
