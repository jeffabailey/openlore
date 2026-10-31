//! scale-to-zero-hosting — layer-1 properties of the PURE `decide` module
//! (ADR-007 functional core; Mandate 9: PBT full at layers 1-2).
//!
//! | ID | Contract | Source |
//! |---|---|---|
//! | CORE-1 | `freshness_due`: due before the first check; due iff at least the TTL has passed since the last check; does not panic when the clock goes backwards | D-STZ-10/11 |
//! | CORE-2 | `reload_needed`: same ETag → keep; other ETag → fetch it; no object → empty (or keep when already empty); failed check → keep and report | D-STZ-10/11/13 |
//! | CORE-3 | `publish_decision`: publish iff the fingerprint changed AND the store is usable, for every exit code 0/2/3 | D-STZ-7/9 |
//! | CORE-4 | `not_live`: live iff health ok AND search ok AND a pass recorded within 45 min (2700 s still recent); each failing fact names its own cause | D-STZ-23, U-STZ-3 |
//! | CORE-5 | `dids_copy_usable`: usable iff the copy is at most 2 h old | D-STZ-16 |
//! | CORE-6 | `final_exit`: a failed pass keeps its cause, then a publish failure, then a record failure, then the pass exit | D-STZ-40, D-STZ-41 |
//! | CORE-7 | `pass_mode`: `{}` is a normal pass, `{"mode":"no-publish"}` is no-publish, everything else is refused | D-STZ-44 |
//! | CORE-8 | `served_index_stale`: the D-STZ-48 truth table, over parsed instants | D-STZ-48 |
//!
//! Oracles are written from the decision text, never from an implementation.
//! Closed finite tables (3 exit codes × 2 × 2; 3 head outcomes; 3 × 3 × 2 for
//! `final_exit`) are exhaustive examples beside the generators (falsifier gate).
//!
//! Binding seam (Mandate 7): each `sut_*` makes ONE call into the production
//! function. RED at hand-off: the scaffold bodies panic.

use std::time::Duration;

use appview_domain::ingest_pass::{PassExit, PassFailure};
use chrono::{DateTime, FixedOffset, TimeZone, Utc};
use openlore_indexer_lambda::decide::{
    self, EventRefused, HeadSeen, Liveness, NotLiveCause, PassMode, ProbeFacts, Publish,
    PublishStep, RecordStep, Reload, ServedIndex, StaleServedIndex,
};
use ports::IndexFingerprint;
use proptest::prelude::*;
use serde_json::{json, Value};

// =============================================================================
// Binding seam
// =============================================================================

fn sut_freshness_due(now: DateTime<Utc>, last: Option<DateTime<Utc>>, ttl: Duration) -> bool {
    decide::freshness_due(now, last, ttl)
}

fn sut_reload_needed(current: Option<&str>, head: &HeadSeen) -> Reload {
    decide::reload_needed(current, head)
}

fn sut_publish(
    exit: PassExit,
    before: IndexFingerprint,
    after: IndexFingerprint,
    usable: bool,
) -> Publish {
    decide::publish_decision(exit, before, after, usable)
}

fn sut_not_live(facts: &ProbeFacts) -> Liveness {
    decide::not_live(facts, decide::LAST_PASS_STALE_AFTER)
}

fn sut_dids_copy_usable(age: Duration) -> bool {
    decide::dids_copy_usable(age, decide::DIDS_COPY_MAX_AGE)
}

fn sut_final_exit(pass: PassExit, publish: PublishStep, record: RecordStep) -> PassExit {
    decide::final_exit(pass, publish, record)
}

fn sut_pass_mode(event: &Value) -> Result<PassMode, EventRefused> {
    decide::pass_mode(event)
}

fn sut_served_index_stale(
    served: ServedIndex<'_>,
    recorded: Option<&str>,
    record_age: Option<Duration>,
) -> Option<StaleServedIndex> {
    decide::served_index_stale(served, recorded, record_age, decide::SERVED_INDEX_MARGIN)
}

// =============================================================================
// Generators
// =============================================================================

fn instant() -> impl Strategy<Value = DateTime<Utc>> {
    // 2026-01-01 .. 2027-01-01, second precision.
    (1_767_225_600_i64..1_798_761_600_i64).prop_map(|s| Utc.timestamp_opt(s, 0).unwrap())
}

fn etag() -> impl Strategy<Value = String> {
    "\"[0-9a-f]{8}\"".prop_map(|s| s)
}

fn fingerprint() -> impl Strategy<Value = IndexFingerprint> {
    (0u64..5, 0u64..3, any::<u64>()).prop_map(|(claims, authors, cid_hash_xor)| IndexFingerprint {
        claims,
        authors,
        cid_hash_xor,
    })
}

fn failure() -> impl Strategy<Value = PassFailure> {
    prop_oneof![
        Just(PassFailure::ListMalformed),
        Just(PassFailure::ListUnreadable),
        Just(PassFailure::UpsertFailed),
        Just(PassFailure::PurgeFailed),
        Just(PassFailure::PassPanicked),
        Just(PassFailure::PassDeadlineExceeded),
        Just(PassFailure::StartingIndexUnusable),
    ]
}

fn exit() -> impl Strategy<Value = PassExit> {
    prop_oneof![
        Just(PassExit::Completed),
        Just(PassExit::TotalOutage),
        failure().prop_map(PassExit::Failed),
    ]
}

const EXITS: [PassExit; 3] = [
    PassExit::Completed,
    PassExit::TotalOutage,
    PassExit::Failed(PassFailure::UpsertFailed),
];

// =============================================================================
// CORE-1 freshness_due
// =============================================================================

/// CORE-1
/// ```gherkin
/// @property @D-STZ-10 @D-STZ-11 @C1b @contract-shape:pure-function
/// Property: A warm instance checks the index at most once per TTL, and always before its first answer
///   Given any time of the last check and any TTL
///   When a request arrives at any later time
///   Then a check is due exactly when at least the TTL has passed
/// ```
mod core_1_freshness_due {
    use super::*;

    proptest! {
        #[test]
        fn due_iff_at_least_the_ttl_has_passed(last in instant(), elapsed in 0u64..600, ttl in 1u64..300) {
            let now = last + chrono::Duration::seconds(elapsed as i64);
            prop_assert_eq!(
                sut_freshness_due(now, Some(last), Duration::from_secs(ttl)),
                elapsed >= ttl
            );
        }

        #[test]
        fn due_before_the_first_check(now in instant(), ttl in 1u64..300) {
            prop_assert!(sut_freshness_due(now, None, Duration::from_secs(ttl)));
        }

        /// What a backwards clock should answer is not specified, so this pins
        /// only that it answers. Subtracting into a `std::time::Duration` is the
        /// mistake it catches.
        #[test]
        fn does_not_panic_when_the_clock_goes_backwards(last in instant(), back in 1u64..600, ttl in 1u64..300) {
            let now = last - chrono::Duration::seconds(back as i64);
            let _ = sut_freshness_due(now, Some(last), Duration::from_secs(ttl));
        }
    }

    #[test]
    fn the_60_second_boundary() {
        let last = Utc.with_ymd_and_hms(2026, 10, 27, 14, 0, 0).unwrap();
        let ttl = decide::FRESHNESS_TTL;
        assert!(!sut_freshness_due(
            last + chrono::Duration::seconds(59),
            Some(last),
            ttl
        ));
        assert!(sut_freshness_due(
            last + chrono::Duration::seconds(60),
            Some(last),
            ttl
        ));
        assert!(sut_freshness_due(
            last + chrono::Duration::seconds(61),
            Some(last),
            ttl
        ));
    }
}

// =============================================================================
// CORE-2 reload_needed
// =============================================================================

/// CORE-2
/// ```gherkin
/// @property @D-STZ-10 @D-STZ-11 @D-STZ-13 @C2b @contract-shape:pure-function
/// Property: A warm instance downloads only a different index, empties only when the index is gone, and never fails on a failed check
///   Given any index version the instance serves (or none)
///   When the check finds the same version, another version, no index, or fails
///   Then it keeps, fetches that version, opens empty, or keeps and reports
/// ```
mod core_2_reload_needed {
    use super::*;

    fn oracle(current: Option<&str>, head: &HeadSeen) -> Reload {
        match (current, head) {
            (_, HeadSeen::Failed) => Reload::KeepAndReport,
            (Some(c), HeadSeen::Present(e)) if c == e => Reload::Keep,
            (_, HeadSeen::Present(e)) => Reload::Fetch(e.clone()),
            (Some(_), HeadSeen::Absent) => Reload::OpenEmpty,
            (None, HeadSeen::Absent) => Reload::Keep,
        }
    }

    fn head() -> impl Strategy<Value = HeadSeen> {
        prop_oneof![
            etag().prop_map(HeadSeen::Present),
            Just(HeadSeen::Absent),
            Just(HeadSeen::Failed)
        ]
    }

    proptest! {
        #[test]
        fn matches_the_decision_table(current in proptest::option::of(etag()), head in head()) {
            prop_assert_eq!(sut_reload_needed(current.as_deref(), &head), oracle(current.as_deref(), &head));
        }

        #[test]
        fn the_same_version_is_never_downloaded_again(e in etag()) {
            prop_assert_eq!(sut_reload_needed(Some(&e), &HeadSeen::Present(e.clone())), Reload::Keep);
        }
    }

    #[test]
    fn every_row_of_the_table() {
        let rows: [(Option<&str>, HeadSeen, Reload); 7] = [
            (
                Some("\"a\""),
                HeadSeen::Present("\"a\"".into()),
                Reload::Keep,
            ),
            (
                Some("\"a\""),
                HeadSeen::Present("\"b\"".into()),
                Reload::Fetch("\"b\"".into()),
            ),
            (
                None,
                HeadSeen::Present("\"b\"".into()),
                Reload::Fetch("\"b\"".into()),
            ),
            (Some("\"a\""), HeadSeen::Absent, Reload::OpenEmpty),
            (None, HeadSeen::Absent, Reload::Keep),
            (Some("\"a\""), HeadSeen::Failed, Reload::KeepAndReport),
            (None, HeadSeen::Failed, Reload::KeepAndReport),
        ];
        for (current, head, expected) in rows {
            assert_eq!(
                sut_reload_needed(current, &head),
                expected,
                "{current:?} {head:?}"
            );
        }
    }
}

// =============================================================================
// CORE-3 publish_decision
// =============================================================================

/// CORE-3
/// ```gherkin
/// @property @D-STZ-7 @D-STZ-9 @C5a @contract-shape:pure-function
/// Property: A pass publishes exactly when it changed the index and the index is still usable
///   Given any exit code 0, 2 or 3, any fingerprints before and after, and a usable or unusable store
///   When the pass decides whether to publish
///   Then it publishes iff the fingerprints differ and the store is usable
/// ```
mod core_3_publish_decision {
    use super::*;

    proptest! {
        #[test]
        fn publish_iff_changed_and_usable(
            exit in exit(), before in fingerprint(), after in fingerprint(), usable in any::<bool>()
        ) {
            let expected = if before != after && usable { Publish::Publish } else { Publish::Skip };
            prop_assert_eq!(sut_publish(exit, before, after, usable), expected);
        }

        #[test]
        fn an_unchanged_index_is_never_published(exit in exit(), same in fingerprint(), usable in any::<bool>()) {
            prop_assert_eq!(sut_publish(exit, same, same, usable), Publish::Skip);
        }

        #[test]
        fn an_unusable_store_is_never_published(exit in exit(), before in fingerprint(), after in fingerprint()) {
            prop_assert_eq!(sut_publish(exit, before, after, false), Publish::Skip);
        }
    }

    #[test]
    fn every_exit_code_by_change_by_usability() {
        let a = IndexFingerprint {
            claims: 1,
            authors: 1,
            cid_hash_xor: 7,
        };
        let b = IndexFingerprint {
            claims: 2,
            authors: 1,
            cid_hash_xor: 9,
        };
        for exit in EXITS {
            for (after, changed) in [(a, false), (b, true)] {
                for usable in [true, false] {
                    let expected = if changed && usable {
                        Publish::Publish
                    } else {
                        Publish::Skip
                    };
                    assert_eq!(
                        sut_publish(exit, a, after, usable),
                        expected,
                        "{exit:?} {changed} {usable}"
                    );
                }
            }
        }
    }
}

// =============================================================================
// CORE-4 not_live
// =============================================================================

/// CORE-4
/// ```gherkin
/// @property @D-STZ-23 @U-STZ-3 @C1b @C6c @contract-shape:pure-function
/// Property: The probe calls the index live only when every fact is good, and names each bad one
///   Given any health result, search result and last-pass age (or none)
///   When the probe judges liveness
///   Then the causes are exactly the failing facts, and live means no cause
/// ```
mod core_4_not_live {
    use super::*;

    fn oracle(facts: &ProbeFacts) -> Vec<NotLiveCause> {
        let mut causes = Vec::new();
        if !facts.healthz_ok {
            causes.push(NotLiveCause::HealthzFailed);
        }
        if !facts.search_ok {
            causes.push(NotLiveCause::SearchFailed);
        }
        match facts.last_pass_age {
            Some(age) if age <= Duration::from_secs(2700) => {}
            _ => causes.push(NotLiveCause::NoRecentPass),
        }
        causes
    }

    proptest! {
        #[test]
        fn causes_are_exactly_the_failing_facts(
            healthz_ok in any::<bool>(), search_ok in any::<bool>(),
            age in proptest::option::of(0u64..10_000)
        ) {
            let facts = ProbeFacts { healthz_ok, search_ok, last_pass_age: age.map(Duration::from_secs) };
            let mut causes = sut_not_live(&facts).causes;
            causes.sort();
            prop_assert_eq!(causes, oracle(&facts));
        }
    }

    #[test]
    fn the_45_minute_boundary() {
        let at = |s| ProbeFacts {
            healthz_ok: true,
            search_ok: true,
            last_pass_age: Some(Duration::from_secs(s)),
        };
        assert!(sut_not_live(&at(2699)).causes.is_empty());
        assert!(sut_not_live(&at(2700)).causes.is_empty());
        assert_eq!(
            sut_not_live(&at(2701)).causes,
            vec![NotLiveCause::NoRecentPass]
        );
    }
}

// =============================================================================
// CORE-5 dids_copy_usable
// =============================================================================

/// CORE-5
/// ```gherkin
/// @property @D-STZ-16 @C1b @contract-shape:pure-function
/// Property: A warm pass reuses its last good DID list for at most 2 hours
///   Given a last good list of any age
///   When the parameter store fails
///   Then the copy is usable iff it is at most 2 hours old
/// ```
mod core_5_dids_copy_usable {
    use super::*;

    proptest! {
        #[test]
        fn usable_iff_at_most_two_hours_old(age in 0u64..20_000) {
            prop_assert_eq!(sut_dids_copy_usable(Duration::from_secs(age)), age <= 7200);
        }
    }

    #[test]
    fn the_two_hour_boundary() {
        assert!(sut_dids_copy_usable(Duration::from_secs(7199)));
        assert!(sut_dids_copy_usable(Duration::from_secs(7200)));
        assert!(!sut_dids_copy_usable(Duration::from_secs(7201)));
    }
}

// =============================================================================
// CORE-6 final_exit
// =============================================================================

/// CORE-6
/// ```gherkin
/// @property @D-STZ-40 @D-STZ-41 @UI-14 @C5a @C6c @contract-shape:pure-function
/// Property: The summary carries the first failure, and a pass that did not fail keeps its exit
///   Given any pass exit, any publish outcome and any record outcome
///   When the pass decides its final exit
///   Then a failed pass keeps its cause
///   And otherwise a publish failure wins, then a record failure, then the pass exit
/// ```
mod core_6_final_exit {
    use super::*;

    const PUBLISHES: [PublishStep; 3] = [
        PublishStep::NoFailure,
        PublishStep::Conflict,
        PublishStep::Failed,
    ];
    const RECORDS: [RecordStep; 2] = [RecordStep::NoFailure, RecordStep::Failed];

    fn oracle(pass: PassExit, publish: PublishStep, record: RecordStep) -> PassExit {
        match (pass, publish, record) {
            (PassExit::Failed(cause), _, _) => PassExit::Failed(cause),
            (_, PublishStep::Conflict, _) => PassExit::Failed(PassFailure::PublishConflict),
            (_, PublishStep::Failed, _) => PassExit::Failed(PassFailure::PublishFailed),
            (_, PublishStep::NoFailure, RecordStep::Failed) => {
                PassExit::Failed(PassFailure::PassRecordFailed)
            }
            (exit, PublishStep::NoFailure, RecordStep::NoFailure) => exit,
        }
    }

    fn publish() -> impl Strategy<Value = PublishStep> {
        proptest::sample::select(PUBLISHES.to_vec())
    }

    fn record() -> impl Strategy<Value = RecordStep> {
        proptest::sample::select(RECORDS.to_vec())
    }

    proptest! {
        #[test]
        fn matches_the_decision_table(pass in exit(), publish in publish(), record in record()) {
            prop_assert_eq!(sut_final_exit(pass, publish, record), oracle(pass, publish, record));
        }

        #[test]
        fn a_failed_pass_keeps_its_cause(cause in failure(), publish in publish(), record in record()) {
            prop_assert_eq!(
                sut_final_exit(PassExit::Failed(cause), publish, record),
                PassExit::Failed(cause)
            );
        }
    }

    #[test]
    fn every_row_of_the_table() {
        for pass in EXITS {
            for publish in PUBLISHES {
                for record in RECORDS {
                    assert_eq!(
                        sut_final_exit(pass, publish, record),
                        oracle(pass, publish, record),
                        "{pass:?} {publish:?} {record:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn a_412_on_an_exit_3_pass_is_exit_2_with_its_cause() {
        let exit = sut_final_exit(
            PassExit::TotalOutage,
            PublishStep::Conflict,
            RecordStep::NoFailure,
        );
        assert_eq!(exit.code(), 2);
        assert_eq!(
            exit.cause().map(PassFailure::token),
            Some("publish_conflict")
        );
    }
}

// =============================================================================
// CORE-7 pass_mode
// =============================================================================

/// CORE-7
/// ```gherkin
/// @property @D-STZ-44 @C5a @C6a @contract-shape:pure-function
/// Property: A pass accepts exactly two events and refuses every other
///   Given any event
///   When the pass reads it
///   Then an empty object is a normal pass
///   And an object holding only the no-publish mode is no-publish
///   And everything else is refused
/// ```
mod core_7_pass_mode {
    use super::*;

    const REFUSED: Result<PassMode, EventRefused> = Err(EventRefused);

    fn json_value() -> impl Strategy<Value = Value> {
        prop_oneof![
            Just(Value::Null),
            any::<bool>().prop_map(Value::from),
            any::<i64>().prop_map(Value::from),
            "[a-z-]{0,12}".prop_map(Value::from),
        ]
    }

    /// Any key but `mode`, so at least one is there.
    fn other_keys() -> impl Strategy<Value = serde_json::Map<String, Value>> {
        proptest::collection::btree_map(
            "[a-z]{1,8}".prop_filter("not the mode key", |k| k != "mode"),
            json_value(),
            1..4,
        )
        .prop_map(|keys| keys.into_iter().collect())
    }

    proptest! {
        #[test]
        fn a_richer_object_is_refused(event in other_keys()) {
            prop_assert_eq!(sut_pass_mode(&Value::Object(event)), REFUSED);
        }

        #[test]
        fn a_key_beside_a_good_mode_is_refused(mut event in other_keys()) {
            event.insert("mode".to_string(), json!("no-publish"));
            prop_assert_eq!(sut_pass_mode(&Value::Object(event)), REFUSED);
        }

        #[test]
        fn any_other_mode_value_is_refused(
            mode in json_value().prop_filter("not the known mode", |m| m != "no-publish")
        ) {
            prop_assert_eq!(sut_pass_mode(&json!({"mode": mode})), REFUSED);
        }
    }

    #[test]
    fn the_events_that_matter() {
        let rows = [
            (json!({}), Ok(PassMode::Normal)),
            (json!({"mode": "no-publish"}), Ok(PassMode::NoPublish)),
            (json!({"mode": "rehearsal"}), REFUSED),
            (json!({"mode": ""}), REFUSED),
            (json!({"mode": null}), REFUSED),
            (json!({"mode": false}), REFUSED),
            (json!({"mood": "no-publish"}), REFUSED),
            (json!({"Mode": "no-publish"}), REFUSED),
            (json!({"mode": "no-publish", "extra": 1}), REFUSED),
            (json!({"detail-type": "Scheduled Event"}), REFUSED),
            (json!([]), REFUSED),
            (json!("no-publish"), REFUSED),
            (json!(null), REFUSED),
        ];
        for (event, expected) in rows {
            assert_eq!(sut_pass_mode(&event), expected, "{event}");
        }
    }
}

// =============================================================================
// CORE-8 served_index_stale
// =============================================================================

/// CORE-8
/// ```gherkin
/// @property @D-STZ-48 @U-STZ-6 @C1b @C5a @contract-shape:pure-function
/// Property: A served index is stale only when it lags a record old enough to trust
///   Given any served time or none, any recorded time or none, and a record of any age
///   When the probe compares them
///   Then it finds nothing to compare without a recorded value, a health answer or a readable record
///   And it finds nothing stale when the served index is as new, or the record is 120 seconds old or less
///   And otherwise it finds the served index stale, by the difference, or by no figure when it serves nothing
/// ```
mod core_8_served_index_stale {
    use super::*;

    const RECORDED: &str = "2026-10-27T14:00:00Z";
    const LATE: Option<Duration> = Some(Duration::from_secs(600));

    fn stale_by(secs: u64) -> Option<StaleServedIndex> {
        Some(StaleServedIndex {
            lag: Some(Duration::from_secs(secs)),
        })
    }

    /// The same instant in another offset: its text sorts differently.
    fn written_in(instant: DateTime<Utc>, offset_hours: i32) -> String {
        let offset = FixedOffset::east_opt(offset_hours * 3600).expect("a small offset");
        instant.with_timezone(&offset).to_rfc3339()
    }

    proptest! {
        #[test]
        fn compares_instants_never_their_text(
            served in instant(), recorded in instant(),
            served_offset in -11i32..=11, recorded_offset in -11i32..=11,
            age in 0u64..1_000
        ) {
            let lag = (recorded - served).num_seconds();
            let expected = (age > 120 && lag > 0).then(|| StaleServedIndex {
                lag: Some(Duration::from_secs(lag as u64)),
            });
            let (served, recorded) =
                (written_in(served, served_offset), written_in(recorded, recorded_offset));
            prop_assert_eq!(
                sut_served_index_stale(
                    ServedIndex::PublishedAt(&served),
                    Some(&recorded),
                    Some(Duration::from_secs(age))
                ),
                expected
            );
        }

        #[test]
        fn nothing_to_compare_means_no_line(
            served in proptest::option::of(instant()), age in proptest::option::of(0u64..1_000)
        ) {
            let text = served.map(|s| s.to_rfc3339());
            let reading = text.as_deref().map_or(ServedIndex::NothingPublished, ServedIndex::PublishedAt);
            let age = age.map(Duration::from_secs);
            prop_assert_eq!(sut_served_index_stale(reading, None, age), None);
            prop_assert_eq!(sut_served_index_stale(ServedIndex::Unknown, Some(RECORDED), age), None);
            prop_assert_eq!(sut_served_index_stale(reading, Some(RECORDED), None), None);
        }
    }

    #[test]
    fn every_row_of_the_truth_table() {
        let older = ServedIndex::PublishedAt("2026-10-27T13:45:00Z");
        let same = ServedIndex::PublishedAt(RECORDED);
        let same_elsewhere = ServedIndex::PublishedAt("2026-10-27T16:00:00+02:00");
        let newer = ServedIndex::PublishedAt("2026-10-27T14:00:01Z");
        let earlier_text_later_instant = ServedIndex::PublishedAt("2026-10-27T10:00:00-05:00");
        let nothing = ServedIndex::NothingPublished;
        let gibberish = ServedIndex::PublishedAt("not a time");
        type Row<'a> = (
            ServedIndex<'a>,
            Option<&'a str>,
            Option<Duration>,
            Option<StaleServedIndex>,
        );
        let rows: [Row<'_>; 11] = [
            (older, Some(RECORDED), LATE, stale_by(900)),
            (same, Some(RECORDED), LATE, None),
            (same_elsewhere, Some(RECORDED), LATE, None),
            (newer, Some(RECORDED), LATE, None),
            (earlier_text_later_instant, Some(RECORDED), LATE, None),
            (
                nothing,
                Some(RECORDED),
                LATE,
                Some(StaleServedIndex { lag: None }),
            ),
            (older, None, LATE, None),
            (nothing, None, LATE, None),
            (ServedIndex::Unknown, Some(RECORDED), LATE, None),
            (older, Some(RECORDED), None, None),
            (gibberish, Some(RECORDED), LATE, None),
        ];
        for (served, recorded, age, expected) in rows {
            assert_eq!(
                sut_served_index_stale(served, recorded, age),
                expected,
                "{served:?} {recorded:?} {age:?}"
            );
        }
    }

    #[test]
    fn the_120_second_boundary() {
        let older = ServedIndex::PublishedAt("2026-10-27T13:45:00Z");
        let aged = |s| sut_served_index_stale(older, Some(RECORDED), Some(Duration::from_secs(s)));
        assert_eq!(aged(119), None);
        assert_eq!(aged(120), None);
        assert_eq!(aged(121), stale_by(900));
    }
}
