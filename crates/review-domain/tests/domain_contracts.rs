//! Pure-core contracts asserted at the crate boundary (DELIVER Phase 5:
//! each of these behaviours had a surviving mutant under the in-crate tests).

use std::collections::BTreeMap;

use ports::claim_domain::RecordOrigin;
use ports::{KpiCounterRow, RepoRecord, ScanCounts, Suggestion, SuggestionKey};
use proptest::prelude::*;
use review_domain::budget::{
    admit_scan, budget_allows, release_scan, ScanAdmission, ScanBudget, DAY_SECS,
};
use review_domain::edits::{parse_confidence, philosophy_choices, vocabulary, ClaimEdit};
use review_domain::kpi::{
    approval_was_edited, rollup_day, seconds_until_next_rollup, sign_in_refusal_event,
    sum_counters, utc_day, KpiEvent,
};
use review_domain::ownership::{did_tokens, OwnershipRefusal};
use review_domain::plans::{
    at_uri, plan_expires_at, plan_freshness, publish_edited_plan, publish_plan, read_back_matches,
    retract_plan, rfc3339_utc, ClaimDraft, PlanFreshness, PublishPlan,
};
use review_domain::published::{live_published_claim, profile_subject, ProfileSubject};
use review_domain::reconcile::Reconciled;
use review_domain::share::{compose_post, share_post_plan, ShareRefusal, MAX_POST_GRAPHEMES};
use review_domain::signin::SignInFailure;

const OWNER: &str = "did:plc:owner";
const COMPOSED_AT: &str = "2026-10-04T15:02:11Z";
const MEMORY_SAFETY: &str = "org.openlore.philosophy.memory-safety";

fn key() -> SuggestionKey {
    SuggestionKey {
        subject: "github:priyaraman/tidepool".to_string(),
        predicate: "embodiesPhilosophy".to_string(),
        object: MEMORY_SAFETY.to_string(),
    }
}

fn draft() -> ClaimDraft {
    ClaimDraft {
        key: key(),
        evidence: vec!["https://github.com/priyaraman/tidepool/".to_string()],
        confidence_bp: 2500,
    }
}

fn plan() -> PublishPlan {
    publish_plan(OWNER, &draft(), COMPOSED_AT).expect("plan")
}

fn suggestion() -> Suggestion {
    Suggestion {
        key: key(),
        confidence_bp: 2500,
        evidence: draft().evidence,
        why: vec!["Cargo.lock pins every dependency".to_string()],
        source_repo: "priyaraman/tidepool".to_string(),
    }
}

// ---------------------------------------------------------------- budget

#[test]
fn an_event_exactly_one_window_old_no_longer_counts() {
    assert!(budget_allows(&[3600], 1, 3600));
    assert!(!budget_allows(&[3599], 1, 3600));
}

#[test]
fn a_scan_a_full_day_old_is_forgotten() {
    let (aged, first) = admit_scan(ScanBudget::default(), OWNER, 0, 2);
    assert_eq!(first, ScanAdmission::Admitted);
    let (after_a_day, again) = admit_scan(release_scan(aged, OWNER), OWNER, DAY_SECS, 2);
    let (fresh, _) = admit_scan(ScanBudget::default(), OWNER, DAY_SECS, 2);
    assert_eq!(again, ScanAdmission::Admitted);
    assert_eq!(after_a_day, fresh);
}

// ---------------------------------------------------------------- edits

#[test]
fn a_confidence_may_omit_the_leading_zero() {
    assert_eq!(parse_confidence(".5"), Ok(5000));
    assert_eq!(parse_confidence(".05"), Ok(500));
}

#[test]
fn a_signed_fraction_is_refused() {
    for typed in ["0.+5", "0.-5", "0.+", "0. 5"] {
        assert!(parse_confidence(typed).is_err(), "{typed}");
    }
}

#[test]
fn the_vocabulary_is_the_distinct_philosophy_objects() {
    let known = vocabulary();
    assert!(known.len() > 1);
    assert!(known.contains(&MEMORY_SAFETY.to_string()));
    assert!(known
        .iter()
        .all(|object| object.starts_with("org.openlore.philosophy.")));
    let distinct: std::collections::BTreeSet<&String> = known.iter().collect();
    assert_eq!(distinct.len(), known.len());
    assert_eq!(philosophy_choices(MEMORY_SAFETY), known);
}

// ---------------------------------------------------------------- kpi

#[test]
fn every_catalogue_event_counts_under_its_own_distinct_name() {
    let rows: Vec<KpiCounterRow> = KpiEvent::ALL
        .iter()
        .zip(1i64..)
        .map(|(event, count)| KpiCounterRow {
            day: "2026-10-04".to_string(),
            event: event.name().to_string(),
            count,
        })
        .collect();
    let expected: BTreeMap<&'static str, i64> = KpiEvent::ALL
        .iter()
        .zip(1i64..)
        .map(|(event, count)| (event.name(), count))
        .collect();
    assert_eq!(expected.len(), KpiEvent::ALL.len(), "names are distinct");
    assert!(expected
        .keys()
        .all(|name| name.contains('.') || *name == "disconnect"));
    assert_eq!(sum_counters(&rows), expected);
    assert_eq!(KpiEvent::SignInStarted.name(), "signin.started");
}

#[test]
fn only_a_cancelled_sign_in_counts_as_denied() {
    assert_eq!(
        sign_in_refusal_event(SignInFailure::Cancelled),
        KpiEvent::SignInDenied
    );
    assert_eq!(
        sign_in_refusal_event(SignInFailure::HandleNotFound),
        KpiEvent::SignInFailed
    );
}

#[test]
fn an_approval_is_edited_only_when_its_philosophy_or_confidence_changed() {
    let offered = suggestion();
    let as_offered = plan();
    assert!(!approval_was_edited(as_offered.claim(), Some(&offered)));
    assert!(!approval_was_edited(as_offered.claim(), None));
    let other_object = ClaimEdit {
        object: "org.openlore.philosophy.test-driven".to_string(),
        confidence_bp: 2500,
    };
    let other_confidence = ClaimEdit {
        object: MEMORY_SAFETY.to_string(),
        confidence_bp: 7000,
    };
    for edit in [other_object, other_confidence] {
        let edited = publish_edited_plan(OWNER, &draft(), &edit, COMPOSED_AT).expect("plan");
        assert!(approval_was_edited(edited.claim(), Some(&offered)));
    }
}

#[test]
fn the_rollup_fires_daily_at_five_past_midnight_utc_for_the_day_that_ended() {
    assert_eq!(seconds_until_next_rollup(0), 5 * 60);
    assert_eq!(seconds_until_next_rollup(5 * 60), 24 * 60 * 60);
    assert_eq!(seconds_until_next_rollup(6 * 60), 24 * 60 * 60 - 60);
    assert_eq!(utc_day(0), "1970-01-01");
    assert_eq!(rollup_day(24 * 60 * 60 + 5 * 60), "1970-01-01");
}

// ---------------------------------------------------------------- ownership

#[test]
fn only_did_shaped_tokens_are_dids() {
    let cases: [(&str, &[&str]); 9] = [
        ("did:plc:abc", &["did:plc:abc"]),
        ("see did:web:example.com.", &["did:web:example.com"]),
        ("hello world", &[]),
        ("dad:plc:abc", &[]),
        ("did::abc", &[]),
        ("did:PLC:abc", &[]),
        ("did:plc:", &[]),
        ("did:plc", &[]),
        ("did:plc:a did:web:b", &["did:plc:a", "did:web:b"]),
    ];
    for (bio, expected) in cases {
        assert_eq!(did_tokens(bio), expected.to_vec(), "{bio}");
    }
}

#[test]
fn only_a_bio_or_account_finding_disproves_ownership_and_each_refusal_has_its_label() {
    let table = [
        (OwnershipRefusal::DidMissing, true, "did_missing"),
        (
            OwnershipRefusal::DifferentDid("did:plc:x".into()),
            true,
            "different_did",
        ),
        (OwnershipRefusal::NoBio, true, "no_bio"),
        (OwnershipRefusal::IdentityChanged, true, "identity_changed"),
        (OwnershipRefusal::AccountNotFound, true, "account_not_found"),
        (OwnershipRefusal::RateLimited, false, "rate_limited"),
        (
            OwnershipRefusal::GithubUnavailable,
            false,
            "github_unavailable",
        ),
        (
            OwnershipRefusal::LinkedToAnotherAccount,
            false,
            "linked_to_another_account",
        ),
        (
            OwnershipRefusal::TooManyAttempts,
            false,
            "too_many_attempts",
        ),
    ];
    for (refusal, disproves, label) in table {
        assert_eq!(refusal.disproves_ownership(), disproves, "{refusal:?}");
        assert_eq!(refusal.label(), label);
    }
}

// ---------------------------------------------------------------- plans

#[test]
fn a_plan_names_its_owner_and_its_record_address() {
    let publish = plan();
    assert_eq!(publish.owner_did(), OWNER);
    let expected = format!("at://{OWNER}/org.openlore.claim/{}", publish.rkey());
    assert_eq!(publish.at_uri(), expected);
    assert_eq!(at_uri(OWNER, publish.rkey()), expected);

    let retract =
        retract_plan(OWNER, publish.rkey(), publish.claim(), COMPOSED_AT).expect("retract");
    assert_eq!(retract.owner_did(), OWNER);
    assert_eq!(
        retract.at_uri(),
        format!("at://{OWNER}/org.openlore.claim/{}", retract.rkey())
    );
    assert_ne!(retract.rkey(), publish.rkey());
}

#[test]
fn a_read_back_matches_only_the_record_that_hashes_to_its_key() {
    let publish = plan();
    assert!(read_back_matches(&publish.record(), publish.rkey()));
    assert!(!read_back_matches(&publish.record(), "bafynotit"));
    let mut tampered = publish.record();
    tampered["confidence"] = serde_json::json!(9000);
    assert!(!read_back_matches(&tampered, publish.rkey()));
}

#[test]
fn a_preview_stays_confirmable_for_thirty_minutes() {
    let previewed_at = 1_000_000;
    let expires_at = plan_expires_at(previewed_at);
    assert_eq!(expires_at, previewed_at + 30 * 60);
    assert_eq!(
        plan_freshness(expires_at, expires_at - 1),
        PlanFreshness::Fresh
    );
    assert_eq!(
        plan_freshness(expires_at, expires_at),
        PlanFreshness::Expired
    );
}

#[test]
fn unix_times_render_as_rfc3339_utc_on_the_civil_calendar() {
    for (unix, expected) in [
        (0, "1970-01-01T00:00:00Z"),
        (-1, "1969-12-31T23:59:59Z"),
        (951_782_400, "2000-02-29T00:00:00Z"),
        (951_868_800, "2000-03-01T00:00:00Z"),
        (1_000_000_000, "2001-09-09T01:46:40Z"),
        (1_704_067_200, "2024-01-01T00:00:00Z"),
        (1_709_164_800, "2024-02-29T00:00:00Z"),
        (1_735_689_599, "2024-12-31T23:59:59Z"),
        (4_102_444_800, "2100-01-01T00:00:00Z"),
    ] {
        assert_eq!(rfc3339_utc(unix), expected, "{unix}");
    }
}

// ---------------------------------------------------------------- published

#[test]
fn a_profile_segment_with_a_did_method_and_a_body_names_that_did() {
    assert_eq!(
        profile_subject("did:plc:abc123"),
        Some(ProfileSubject::Did("did:plc:abc123".into()))
    );
    assert_eq!(
        profile_subject("did:web:example.com"),
        Some(ProfileSubject::Did("did:web:example.com".into()))
    );
    assert!(!matches!(
        profile_subject("did:plc:"),
        Some(ProfileSubject::Did(_))
    ));
    assert!(!matches!(
        profile_subject("did:key:abc"),
        Some(ProfileSubject::Did(_))
    ));
}

#[test]
fn the_live_claim_under_a_key_is_the_owners_record_there() {
    let publish = plan();
    let records = [RepoRecord {
        repo_did: OWNER.to_string(),
        rkey: publish.rkey().to_string(),
        value: publish.record(),
    }];
    assert_eq!(
        live_published_claim(OWNER, &records, RecordOrigin::AuthorPds, publish.rkey()),
        Some(publish.claim().clone())
    );
    assert_eq!(
        live_published_claim(OWNER, &records, RecordOrigin::AuthorPds, "bafyother"),
        None
    );
}

// ---------------------------------------------------------------- reconcile

#[test]
fn a_reconcile_counts_what_it_adds_and_what_it_found_settled() {
    let reconciled = Reconciled {
        new: vec![suggestion()],
        already_published: 2,
        declined_hidden: 3,
    };
    assert_eq!(
        reconciled.counts(),
        ScanCounts {
            new: 1,
            already_published: 2,
            declined_hidden: 3
        }
    );
}

// ---------------------------------------------------------------- share

proptest! {
    /// Universe: post bodies around the limit. A post of exactly the limit
    /// is sent; one grapheme more is refused with its length.
    #[test]
    fn a_post_fits_up_to_and_including_the_limit(extra in 0usize..3) {
        let url = "https://openlore.example/@priyaraman.bsky.social";
        let length = MAX_POST_GRAPHEMES - url.len() - 1 + extra;
        let text = "a".repeat(length);
        let result = compose_post(&text, url);
        if extra == 0 {
            prop_assert_eq!(result.expect("fits").text().chars().count(), MAX_POST_GRAPHEMES);
        } else {
            prop_assert_eq!(result, Err(ShareRefusal::TooLong { graphemes: MAX_POST_GRAPHEMES + extra }));
        }
    }
}

#[test]
fn the_draft_names_each_published_philosophy_by_its_slug() {
    let url = "https://openlore.example/@priyaraman.bsky.social";
    let published = [review_domain::published::PublishedClaim {
        subject: "github:priyaraman/tidepool".into(),
        object: MEMORY_SAFETY.into(),
        confidence_bp: 2500,
        rkey: "bafy1".into(),
    }];
    let share = share_post_plan(url, &published).expect("plan");
    assert_eq!(
        share.draft().text(),
        format!("How I build, in claims I've published on OpenLore: memory-safety. {url}")
    );
}

#[test]
fn every_share_refusal_explains_itself_distinctly() {
    let messages = [
        ShareRefusal::NothingPublished.message(),
        ShareRefusal::Empty.message(),
        ShareRefusal::TooLong { graphemes: 412 }.message(),
    ];
    assert!(messages[0].contains("publish a claim first"));
    assert!(messages[1].contains("Nothing was posted"));
    assert!(messages[2].contains("412"));
}
