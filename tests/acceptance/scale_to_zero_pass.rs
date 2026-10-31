//! scale-to-zero-hosting — the scheduled pass on Lambda (ADR-090 §2, §4).
//!
//! Driving port: the pass function's scheduler entry
//! ([`openlore_indexer_lambda::PassFunction::handle`]), the entry the
//! EventBridge schedule and `deploy.sh lambda trigger` reach. Layer 2
//! (in-process root, fakes of the AWS ports, the REAL pass and DuckDB),
//! example-only; `publish_decision` and `dids_copy_usable` are
//! property-tested in `scale_to_zero_core.rs`. Every state-mutating scenario
//! asserts over the pass universe (`LambdaWorld::pass_universe`, Mandate 8).
//!
//! What one pass does (C2a), in the D-STZ-40 order:
//!
//! ```text
//! init: write probe ─┬─ store honours the condition, or cannot be reached ─> go on
//!                    └─ store ignored the condition ─> startup refused (D-STZ-39)
//! read DID list ─┬─ ok ─────────────────────────────┐
//!                ├─ fails, warm copy ≤ 2 h ─────────┤
//!                └─ fails, no usable copy ─> pass exit 2 (list unreadable)
//! fetch object ──┬─ present / absent ───────────────┐
//!                └─ fails / damaged / no room ─> pass exit 2 starting_index_unusable
//! fingerprint before ─> reused ADR-077/078 pass ─> fingerprint after ─> pass exit
//! mode ─┬─ no-publish ─> no write of any kind ─────────────────┐ (D-STZ-44)
//!       └─ normal ─> publish iff changed and store usable (If-Match / If-None-Match)
//!                     ├─ 412 ─> exit 2 publish_conflict, logged
//!                     └─ store error ─> exit 2 publish_failed
//!                    write last-pass.json, carrying the exit decided so far
//!                     └─ fails ─> exit 2 pass_record_failed; the index stays published
//!                                                              │
//! the ONE pass_summary, carrying the final exit <──────────────┘ (UI-14, D-STZ-40)
//! answer: {pass_id, exit_code, published, index_bytes}; function error only on a crash
//! ```
//!
//! `index_published_at` in the record (D-STZ-48): this pass's publish, otherwise
//! the object seen at the start of the pass, otherwise null. A fetch that failed
//! saw no object.
//!
//! Story line (Pillar 2): PA-1 first publish → PA-2 nothing changed → PA-3 a
//! new claim → PA-4 a DID-list edit → PA-8 a removal → PA-9 an emptied list.
//! Then the record, the order, no-publish and the write probe. Exit 2/3 and the
//! error paths follow.
#![cfg(unix)]

#[path = "../common/state_delta.rs"]
mod state_delta;

#[path = "support/indexer_lambda.rs"]
mod indexer_lambda;

use std::time::Duration;

use indexer_lambda::claims::*;
use indexer_lambda::*;
use serde_json::{json, Value};
use state_delta::{assert_state_delta, set_to, Delta};

/// A universe snapshot.
type Snapshot = std::collections::HashMap<String, Value>;

const MINUTE: Duration = Duration::from_secs(60);

/// GIVEN the 14:00 pass published Priya's cargo-pin claim (PA-1's Given+When).
fn given_the_first_pass_published_priyas_claim(world: &LambdaWorld) {
    world.publishes(Author::Priya, &[CARGO_PIN_REPRODUCIBLE_BUILDS]);
    world.given_a_pass_published_the_index();
}

/// WHEN the next quarter-hour comes and the schedule fires a pass.
fn when_the_next_schedule_fires(world: &LambdaWorld) -> PassRun {
    world.time_passes(15 * MINUTE);
    world.cold_pass_instance().schedule_fires()
}

// =============================================================================
// Publish rules (D-STZ-6..9)
// =============================================================================

/// PA-1 (the schedule's event is `{}`, so this is also the proof that `{}` runs a
/// normal pass, D-STZ-44)
/// ```gherkin
/// @driving_port @real-io @D-STZ-6 @D-STZ-7 @D-STZ-33 @D-STZ-44 @KPI-STZ-G4 @kpi @contract-shape:bounded-change
/// Scenario: The first pass creates the index and records itself
///   Given Priya published a claim and the operator's list names her
///   And no index has been published yet
///   When the 14:00 schedule fires the pass
///   Then the pass completes and the index is published, stamped 14:00 with the pass's id
///   And the pass reports the published index's size, and logs it for the size alarm
///   And the pass record says it completed and published that index
///   And nothing about the run says it was a no-publish run
/// ```
#[test]
#[ignore = "RED scaffold: DELIVER (pass mode)"]
fn the_first_pass_creates_the_index_and_records_itself() {
    let world = LambdaWorld::listing(&[Author::Priya]);
    world.publishes(Author::Priya, &[CARGO_PIN_REPRODUCIBLE_BUILDS]);
    let before = world.pass_universe();

    let run = world.cold_pass_instance().schedule_fires();

    let summary = run.assert_exit(Exit::Completed);
    let payload = run.payload();
    let pass_id = summary["pass_id"].clone();
    assert_state_delta(
        &before,
        &world.pass_universe(),
        &world.pass_universe_names(),
        &published_and_recorded(&world, &summary, Exit::Completed)
            .with_slot("write_check_marker.versions", set_to(json!(1))),
    );
    let current = world.bucket.current().expect("an index object");
    assert_eq!(current.pass_id.as_deref(), Some(id_text(&pass_id).as_str()));
    assert_eq!(payload["published"], true);
    assert_eq!(
        payload["index_bytes"].as_u64(),
        Some(current.bytes.len() as u64)
    );
    let published = named(&run.events, "indexer.lambda.published");
    assert_eq!(published[0]["pass_id"], pass_id, "{}", run.dump());
    assert_eq!(
        published[0]["index_bytes"],
        payload["index_bytes"],
        "{}",
        run.dump()
    );
    assert!(payload.get("mode").is_none(), "{}", run.dump());
    assert_ne!(summary["no_publish"], true, "{}", run.dump());
}

/// PA-2
/// ```gherkin
/// @driving_port @real-io @D-STZ-7 @D-STZ-8 @D-STZ-48 @U-STZ-3 @C4a @contract-shape:bounded-change
/// Scenario: A pass that changes nothing publishes nothing but still records itself
///   Given the 14:00 pass published Priya's claim
///   And nobody published anything since
///   When the 14:15 schedule fires the pass
///   Then the pass completes without publishing
///   And the index object is exactly the 14:00 one
///   And the pass record names the 14:15 pass, completed, not published, seeing the 14:00 index
/// ```
#[test]
#[ignore = "RED scaffold: DELIVER (publish only on change)"]
fn a_pass_that_changes_nothing_publishes_nothing_but_still_records_itself() {
    let world = LambdaWorld::listing(&[Author::Priya]);
    given_the_first_pass_published_priyas_claim(&world);
    let before = world.pass_universe();

    let run = when_the_next_schedule_fires(&world);

    let summary = run.assert_exit(Exit::Completed);
    assert_eq!(run.payload()["published"], false, "{}", run.dump());
    assert_state_delta(
        &before,
        &world.pass_universe(),
        &world.pass_universe_names(),
        &recorded_only(&summary, Exit::Completed, seen_at_start(&before)),
    );
}

/// PA-3
/// ```gherkin
/// @driving_port @real-io @D-STZ-6 @D-STZ-7 @KPI-IXD-2 @kpi @contract-shape:bounded-change
/// Scenario: A pass that indexes a new claim replaces exactly the version it started from
///   Given the 14:00 pass published Priya's claim
///   And Priya approved a test-driven claim on cargo-pin at 14:05
///   When the 14:15 schedule fires the pass
///   Then the pass completes and publishes a new version of the index, stamped 14:15
///   And a search instance that starts afterwards finds both of Priya's claims
/// ```
#[test]
#[ignore = "RED scaffold: DELIVER (conditional publish on change)"]
fn a_pass_that_indexes_a_new_claim_replaces_exactly_the_version_it_started_from() {
    let world = LambdaWorld::listing(&[Author::Priya]);
    given_the_first_pass_published_priyas_claim(&world);
    world.publishes(Author::Priya, &[CARGO_PIN_TEST_DRIVEN]);
    let before = world.pass_universe();

    let run = when_the_next_schedule_fires(&world);

    let summary = run.assert_exit(Exit::Completed);
    assert_state_delta(
        &before,
        &world.pass_universe(),
        &world.pass_universe_names(),
        &published_and_recorded(&world, &summary, Exit::Completed),
    );
    assert_search_finds_both_of_priyas_claims(&world);
}

/// PA-4 (the counted check is the requirement itself: KPI-IXD-4 wants the list
/// read on every pass, and a warm copy would also load 2 DIDs on a later pass)
/// ```gherkin
/// @driving_port @real-io @D-STZ-16 @R-STZ-6 @KPI-IXD-4 @kpi @contract-shape:bounded-change
/// Scenario: An edit to the DID list takes effect on the next pass with no redeploy
///   Given the 14:00 pass published Priya's claim
///   And Tomás published a claim that loom embodies reproducible builds
///   And at 14:07 the operator added Tomás to the DID list
///   When the 14:15 schedule fires the pass on the same warm instance
///   Then the pass read the list afresh and loaded 2 DIDs
///   And a search instance that starts afterwards finds Tomás's loom claim
/// ```
#[test]
#[ignore = "RED scaffold: DELIVER (DID list read from the parameter store each pass)"]
fn an_edit_to_the_did_list_takes_effect_on_the_next_pass_with_no_redeploy() {
    let world = LambdaWorld::listing(&[Author::Priya]);
    world.publishes(Author::Priya, &[CARGO_PIN_REPRODUCIBLE_BUILDS]);
    world.publishes(Author::Tomas, &[LOOM_REPRODUCIBLE_BUILDS]);
    let pass = world.cold_pass_instance();
    pass.schedule_fires().assert_exit(Exit::Completed);
    let reads = world.did_list.reads();
    world.time_passes(7 * MINUTE);
    world.operator_lists(&[Author::Priya, Author::Tomas]);
    world.time_passes(8 * MINUTE);
    let before = world.pass_universe();

    let run = pass.schedule_fires();

    let summary = run.assert_exit(Exit::Completed);
    assert_state_delta(
        &before,
        &world.pass_universe(),
        &world.pass_universe_names(),
        &published_and_recorded(&world, &summary, Exit::Completed),
    );
    assert_eq!(world.did_list.reads(), reads + 1, "read once per pass");
    let loaded = named(&run.events, "indexer.config.loaded");
    assert_eq!(loaded.len(), 1, "{}", run.dump());
    assert_eq!(
        loaded[0]["repo_did_count"].as_u64(),
        Some(2),
        "{}",
        run.dump()
    );
    let reply = world
        .cold_search_instance()
        .maria_searches("reproducible-builds");
    assert_eq!(
        reply.subjects_by(Author::Tomas),
        vec![LOOM_REPRODUCIBLE_BUILDS.subject.to_string()],
        "{}",
        reply.text()
    );
}

/// PA-5
/// ```gherkin
/// @driving_port @real-io @D-STZ-9 @U-STZ-5 @KPI-IXD-5 @kpi @error @contract-shape:bounded-change
/// Scenario: A pass that fails part way still publishes the claims it committed
///   Given the 14:00 pass published Priya's claim
///   And Priya approved a test-driven claim and Dmitri published a ferrite claim
///   And the index cannot store Dmitri's claims
///   When the 14:15 schedule fires the pass
///   Then the pass reports exit 2 for the store failure, and the invocation itself succeeds
///   And the index is published with both of Priya's claims
///   And the pass record says exit 2, published
/// ```
#[test]
#[ignore = "RED scaffold: DELIVER (publish on exit 2 with a usable store)"]
fn a_pass_that_fails_part_way_still_publishes_the_claims_it_committed() {
    let world = LambdaWorld::listing(&[Author::Priya, Author::Dmitri]);
    given_the_first_pass_published_priyas_claim(&world);
    world.publishes(Author::Priya, &[CARGO_PIN_TEST_DRIVEN]);
    world.publishes(Author::Dmitri, &[FERRITE_REPRODUCIBLE_BUILDS]);
    world.time_passes(15 * MINUTE);
    let pass = world.cold_pass_instance();
    world.index_cannot_store_claims_of(&pass, Author::Dmitri);
    let before = world.pass_universe();

    let run = pass.schedule_fires();

    let summary = run.assert_exit(Exit::Failed);
    assert_cause(&summary, "upsert_failed", &run);
    assert_state_delta(
        &before,
        &world.pass_universe(),
        &world.pass_universe_names(),
        &published_and_recorded(&world, &summary, Exit::Failed),
    );
    assert_search_finds_both_of_priyas_claims(&world);
}

/// PA-6
/// ```gherkin
/// @driving_port @real-io @D-STZ-9 @U-STZ-3 @U-STZ-5 @KPI-IXD-5 @kpi @error @contract-shape:bounded-change
/// Scenario: A pass that reaches nobody publishes nothing and still records itself
///   Given the 14:00 pass published Priya's claim
///   And Priya's PDS stopped answering
///   When the 14:15 schedule fires the pass
///   Then the pass reports exit 3, and the invocation itself succeeds
///   And the index object is exactly the 14:00 one
///   And the pass record says exit 3, not published
/// ```
#[test]
#[ignore = "RED scaffold: DELIVER (exit 3 is an answer, not a function error)"]
fn a_pass_that_reaches_nobody_publishes_nothing_and_still_records_itself() {
    let world = LambdaWorld::listing(&[Author::Priya]);
    given_the_first_pass_published_priyas_claim(&world);
    world.pds_goes_down(Author::Priya);
    let before = world.pass_universe();

    let run = when_the_next_schedule_fires(&world);

    let summary = run.assert_exit(Exit::AllSkipped);
    assert_state_delta(
        &before,
        &world.pass_universe(),
        &world.pass_universe_names(),
        &recorded_only(&summary, Exit::AllSkipped, seen_at_start(&before)),
    );
}

/// PA-7
/// ```gherkin
/// @driving_port @real-io @D-STZ-16 @C7a @contract-shape:bounded-change
/// Scenario: A warm pass rides out a parameter store outage on its last good list
///   Given the 14:00 pass on a warm instance read the list naming Priya
///   And Priya approved a test-driven claim
///   And the parameter store is unavailable
///   When the 14:15 schedule fires the same instance
///   Then the pass completes on the list it read at 14:00 and publishes Priya's new claim
/// ```
#[test]
#[ignore = "RED scaffold: DELIVER (warm DID-list copy, D-STZ-16)"]
fn a_warm_pass_rides_out_a_parameter_store_outage_on_its_last_good_list() {
    let world = LambdaWorld::listing(&[Author::Priya]);
    world.publishes(Author::Priya, &[CARGO_PIN_REPRODUCIBLE_BUILDS]);
    let pass = world.cold_pass_instance();
    pass.schedule_fires().assert_exit(Exit::Completed);
    world.publishes(Author::Priya, &[CARGO_PIN_TEST_DRIVEN]);
    world.parameter_store_fails(true);
    world.time_passes(15 * MINUTE);
    let before = world.pass_universe();

    let run = pass.schedule_fires();

    let summary = run.assert_exit(Exit::Completed);
    assert_state_delta(
        &before,
        &world.pass_universe(),
        &world.pass_universe_names(),
        &published_and_recorded(&world, &summary, Exit::Completed),
    );
}

/// PA-8
/// ```gherkin
/// @driving_port @real-io @D-STZ-7 @ADR-082 @KPI-IXD-4 @kpi @contract-shape:bounded-change
/// Scenario: An author removed from the DID list leaves search on the next pass
///   Given the 14:00 pass published claims by Priya and Dmitri
///   And at 14:07 the operator removed Dmitri from the DID list
///   When the 14:15 schedule fires the pass
///   Then the pass completes, reports 1 author purged, and publishes a new version of the index
///   And a search instance that starts afterwards finds Priya's claim and none of Dmitri's
/// ```
#[test]
#[ignore = "RED scaffold: DELIVER (the reused pass purges and the purge changes the fingerprint)"]
fn an_author_removed_from_the_did_list_leaves_search_on_the_next_pass() {
    let world = LambdaWorld::listing(&[Author::Priya, Author::Dmitri]);
    world.publishes(Author::Priya, &[CARGO_PIN_REPRODUCIBLE_BUILDS]);
    world.publishes(Author::Dmitri, &[FERRITE_REPRODUCIBLE_BUILDS]);
    world.given_a_pass_published_the_index();
    world.operator_lists(&[Author::Priya]);
    let before = world.pass_universe();

    let run = when_the_next_schedule_fires(&world);

    let summary = run.assert_exit(Exit::Completed);
    assert_eq!(summary["purged_authors"], 1, "{}", run.dump());
    assert_state_delta(
        &before,
        &world.pass_universe(),
        &world.pass_universe_names(),
        &published_and_recorded(&world, &summary, Exit::Completed),
    );
    let reply = world
        .cold_search_instance()
        .maria_searches("reproducible-builds");
    assert_eq!(
        reply.subjects_by(Author::Priya).len(),
        1,
        "{}",
        reply.text()
    );
    assert!(
        reply.subjects_by(Author::Dmitri).is_empty(),
        "{}",
        reply.text()
    );
}

/// PA-9
/// ```gherkin
/// @driving_port @real-io @ADR-082 @C1a @C3 @contract-shape:bounded-change
/// Scenario: An emptied DID list purges nobody and publishes nothing
///   Given the 14:00 pass published Priya's claim
///   And the operator emptied the DID list by mistake
///   When the 14:15 schedule fires the pass
///   Then the pass completes with no authors to read and logs that it held the purge back
///   And the index object is exactly the 14:00 one
/// ```
#[test]
#[ignore = "RED scaffold: DELIVER (ADR-082's empty-list rule on the Lambda pass)"]
fn an_emptied_did_list_purges_nobody_and_publishes_nothing() {
    let world = LambdaWorld::listing(&[Author::Priya]);
    given_the_first_pass_published_priyas_claim(&world);
    world.operator_saves_list_text("\n");
    let before = world.pass_universe();

    let run = when_the_next_schedule_fires(&world);

    let summary = run.assert_exit(Exit::Completed);
    assert_eq!(summary["configured"], 0, "{}", run.dump());
    assert_eq!(summary["purged_authors"], 0, "{}", run.dump());
    assert_eq!(
        named(&run.events, "indexer.ingest.purge_suppressed").len(),
        1,
        "{}",
        run.dump()
    );
    assert_state_delta(
        &before,
        &world.pass_universe(),
        &world.pass_universe_names(),
        &recorded_only(&summary, Exit::Completed, seen_at_start(&before)),
    );
}

/// PA-10
/// ```gherkin
/// @driving_port @real-io @D-STZ-48 @U-STZ-6 @C1a @contract-shape:bounded-change
/// Scenario Outline: A pass on an empty bucket with nothing to publish records that no index exists
///   Given no index has been published yet
///   And <situation>
///   When the 14:00 schedule fires the pass
///   Then the pass reports exit <exit> and publishes nothing
///   And the pass record says no index was published
///   Examples:
///     | situation                       | exit |
///     | the operator's list is empty    | 0    |
///     | Priya's PDS stopped answering   | 3    |
/// ```
#[test]
#[ignore = "RED scaffold: DELIVER (record with no object, D-STZ-48)"]
fn a_pass_on_an_empty_bucket_with_nothing_to_publish_records_that_no_index_exists() {
    each_row(
        [
            ("empty list", Exit::Completed),
            ("PDS down", Exit::AllSkipped),
        ],
        |(situation, exit)| {
            let world = LambdaWorld::listing(&[Author::Priya]);
            world.publishes(Author::Priya, &[CARGO_PIN_REPRODUCIBLE_BUILDS]);
            match *situation {
                "empty list" => world.operator_saves_list_text("\n"),
                _ => world.pds_goes_down(Author::Priya),
            }
            let before = world.pass_universe();

            let run = world.cold_pass_instance().schedule_fires();

            let summary = run.assert_exit(*exit);
            assert_state_delta(
                &before,
                &world.pass_universe(),
                &world.pass_universe_names(),
                &recorded_only(&summary, *exit, Value::Null)
                    .with_slot("write_check_marker.versions", set_to(json!(1))),
            );
        },
    );
}

/// PA-11 (D-STZ-40: A2 counts exit-2 summaries, and a 412 is known only at the
/// publish, so the one summary has to come after it)
/// ```gherkin
/// @driving_port @real-io @D-STZ-40 @UI-14 @KPI-IXD-5 @kpi @contract-shape:bounded-change
/// Scenario Outline: A pass reports itself last, after publishing and recording
///   Given the 14:00 pass published Priya's claim
///   And Priya approved a test-driven claim
///   And <publish outcome>
///   When the 14:15 schedule fires the pass
///   Then the pass tried to publish, then wrote its record, then wrote one summary
///   And the record and the summary carry the same exit, <exit>
///   Examples:
///     | publish outcome                       | exit |
///     | the store takes the publish           | 0    |
///     | another writer publishes first        | 2    |
///     | the store fails the publish           | 2    |
/// ```
#[test]
#[ignore = "RED scaffold: DELIVER (publish, record, summary, D-STZ-40)"]
fn a_pass_reports_itself_last_after_publishing_and_recording() {
    each_row(
        [
            ("published", Exit::Completed),
            ("lost to another writer", Exit::Failed),
            ("store error", Exit::Failed),
        ],
        |(outcome, exit)| {
            let world = LambdaWorld::listing(&[Author::Priya]);
            given_the_first_pass_published_priyas_claim(&world);
            world.publishes(Author::Priya, &[CARGO_PIN_TEST_DRIVEN]);
            match *outcome {
                "published" => {}
                "lost to another writer" => {
                    world.bucket.another_writer_publishes_during_the_next_pass()
                }
                _ => world.bucket.has_trouble(ObjectTrouble::PublishFails),
            }
            let mark = world.timeline.mark();

            let run = when_the_next_schedule_fires(&world);

            let summary = run.assert_exit(*exit);
            let at = |step| world.timeline.position_since(mark, step);
            let (publish, record, reported) =
                (at(step::PUBLISH), at(step::RECORD), at(step::SUMMARY));
            assert!(
                publish.is_some() && publish < record && record < reported,
                "{:?}",
                world.timeline.since(mark)
            );
            assert_eq!(
                world.bucket.last_pass().map(|l| i64::from(l.exit_code)),
                summary["exit_code"].as_i64()
            );
        },
    );
}

// =============================================================================
// No-publish (D-STZ-44, U-STZ-8) and the write probe (D-STZ-39)
// =============================================================================

/// PA-12 (the write check is init's and runs whatever the event, so it is not
/// counted here: it is seeded, and `forbidden` sees only object writes)
/// ```gherkin
/// @driving_port @real-io @D-STZ-44 @U-STZ-8 @C5a @contract-shape:unbounded-preservation
/// Scenario Outline: A no-publish run does the whole pass and writes nothing to the bucket
///   Given the 14:00 pass published Priya's claim
///   And <what is waiting>
///   And the bucket already holds its write-check marker
///   When the deploy gate invokes a pass asking for no-publish
///   Then the pass completes, and its summary says no-publish
///   And the reply says no-publish, not published, and that it <would publish>
///   And nothing under the index changed, and no publish or record write was attempted
///   Examples:
///     | what is waiting                | would publish  |
///     | Priya approved a new claim     | would publish  |
///     | nothing new                    | would not      |
/// ```
#[test]
#[ignore = "RED scaffold: DELIVER (no-publish arm, D-STZ-44)"]
fn a_no_publish_run_does_the_whole_pass_and_writes_nothing_to_the_bucket() {
    each_row(
        [("a new claim", true), ("nothing new", false)],
        |(waiting, would_publish)| {
            let world = LambdaWorld::listing(&[Author::Priya]);
            given_the_first_pass_published_priyas_claim(&world);
            if *waiting == "a new claim" {
                world.publishes(Author::Priya, &[CARGO_PIN_TEST_DRIVEN]);
            }
            world.bucket.holds_the_write_check_marker();
            world.time_passes(15 * MINUTE);
            let before = world.bucket_universe();
            let (pass, writes) = world.cold_pass_instance_forbidden_to_write();

            let run = pass.invoked_with(no_publish_event());

            let summary = run.assert_exit(Exit::Completed);
            assert_eq!(summary["no_publish"], true, "{}", run.dump());
            let payload = run.payload();
            assert_eq!(payload["mode"], "no-publish", "{}", run.dump());
            assert_eq!(payload["published"], false, "{}", run.dump());
            assert_eq!(payload["would_publish"], *would_publish, "{}", run.dump());
            assert_state_delta(
                &before,
                &world.bucket_universe(),
                &world.bucket_universe_names(),
                &Delta::new(),
            );
            assert_eq!(writes.attempts(), 0, "no publish or record write");
        },
    );
}

/// PA-13 (U-STZ-8: "writes nothing" is about the arm; init runs whatever the event)
/// ```gherkin
/// @driving_port @real-io @D-STZ-44 @D-STZ-39 @U-STZ-8 @contract-shape:bounded-change
/// Scenario: On a bucket's very first run, a no-publish run leaves only the write-check marker
///   Given no pass has ever run against this bucket
///   And Priya published a claim and the operator's list names her
///   When the deploy gate invokes a pass asking for no-publish
///   Then the reply says it would publish
///   And the only thing in the bucket is the write-check marker, which init wrote
///   And no publish or record write was attempted
/// ```
#[test]
#[ignore = "RED scaffold: DELIVER (init write probe plus the no-publish arm)"]
fn on_a_buckets_very_first_run_a_no_publish_run_leaves_only_the_write_check_marker() {
    let world = LambdaWorld::listing(&[Author::Priya]);
    world.publishes(Author::Priya, &[CARGO_PIN_REPRODUCIBLE_BUILDS]);
    let before = world.bucket_universe();
    let (pass, writes) = world.cold_pass_instance_forbidden_to_write();

    let run = pass.invoked_with(no_publish_event());

    run.assert_exit(Exit::Completed);
    assert_eq!(run.payload()["would_publish"], true, "{}", run.dump());
    assert_state_delta(
        &before,
        &world.bucket_universe(),
        &world.bucket_universe_names(),
        &Delta::new().with_slot("write_check_marker.versions", set_to(json!(1))),
    );
    assert_eq!(writes.attempts(), 0, "no publish or record write");
}

/// PA-14
/// ```gherkin
/// @driving_port @real-io @D-STZ-39 @C4a @contract-shape:bounded-change
/// Scenario: The write check leaves one marker in a bucket's life, however often passes start
///   Given no pass has ever run against this bucket
///   When a pass instance starts cold three times
///   Then every instance starts, and the bucket holds one write-check marker
/// ```
#[test]
#[ignore = "RED scaffold: DELIVER (write probe at pass init)"]
fn the_write_check_leaves_one_marker_in_a_buckets_life() {
    let world = LambdaWorld::listing(&[Author::Priya]);

    for _ in 0..3 {
        world.cold_pass_instance();
    }

    assert_eq!(world.bucket.write_check_marker_versions(), 1);
}

// =============================================================================
// Error paths
// =============================================================================

/// PA-E1
/// ```gherkin
/// @driving_port @real-io @D-STZ-6 @D-STZ-41 @KPI-IXD-5 @kpi @C7c @error @contract-shape:bounded-change
/// Scenario: A pass never overwrites an index somebody else published while it ran
///   Given the 14:00 pass published Priya's claim
///   And Priya approved a test-driven claim
///   And another writer publishes the index while the 14:15 pass runs
///   When the 14:15 schedule fires the pass
///   Then the pass reports exit 2 for a publish conflict and logs it, and the invocation itself succeeds
///   And the other writer's version is still the current index
///   And the pass record says exit 2, not published, seeing the 14:00 index
/// ```
#[test]
#[ignore = "RED scaffold: DELIVER (If-Match, 412 → publish_conflict)"]
fn a_pass_never_overwrites_an_index_somebody_else_published_while_it_ran() {
    let world = LambdaWorld::listing(&[Author::Priya]);
    given_the_first_pass_published_priyas_claim(&world);
    world.publishes(Author::Priya, &[CARGO_PIN_TEST_DRIVEN]);
    world.bucket.another_writer_publishes_during_the_next_pass();
    let before = world.pass_universe();

    let run = when_the_next_schedule_fires(&world);

    let summary = run.assert_exit(Exit::Failed);
    assert_cause(&summary, "publish_conflict", &run);
    assert_eq!(run.payload()["published"], false, "{}", run.dump());
    assert_state_delta(
        &before,
        &world.pass_universe(),
        &world.pass_universe_names(),
        &recorded_only(&summary, Exit::Failed, seen_at_start(&before))
            .with_slot("index.etag", changed())
            .with_slot("index.published_at", changed())
            .with_slot("index.version_count", grew_by(1))
            .with_slot("events.lambda.publish_conflict.count", grew_by(1)),
    );
    let current = world.bucket.current().expect("an index object");
    assert_eq!(
        current.pass_id.as_deref(),
        Some("another-writer"),
        "no lost update"
    );
}

/// PA-E2
/// ```gherkin
/// @driving_port @real-io @D-STZ-9 @ADR-080-7 @C6b @error @contract-shape:bounded-change
/// Scenario: A pass whose store became unusable never publishes it
///   Given the 14:00 pass published Priya's claim
///   And Priya approved a test-driven claim
///   And the store is unusable by the time the 14:15 pass goes to write
///   When the 14:15 schedule fires the pass
///   Then the pass reports exit 2 and logs that the store is unusable
///   And the index object is exactly the 14:00 one
///   And the pass record says exit 2, not published, seeing the 14:00 index
/// ```
#[test]
#[ignore = "RED scaffold: DELIVER (never publish an unusable store)"]
fn a_pass_whose_store_became_unusable_never_publishes_it() {
    let world = LambdaWorld::listing(&[Author::Priya]);
    given_the_first_pass_published_priyas_claim(&world);
    world.publishes(Author::Priya, &[CARGO_PIN_TEST_DRIVEN]);
    world.with_fault(Fault::StorePoisoned);
    let before = world.pass_universe();

    let run = when_the_next_schedule_fires(&world);

    let summary = run.assert_exit(Exit::Failed);
    assert_eq!(
        named(&run.events, "indexer.store.unusable").len(),
        1,
        "{}",
        run.dump()
    );
    assert_state_delta(
        &before,
        &world.pass_universe(),
        &world.pass_universe_names(),
        &recorded_only(&summary, Exit::Failed, seen_at_start(&before)),
    );
}

/// PA-E3
/// ```gherkin
/// @driving_port @real-io @D-STZ-16 @ADR-081 @KPI-IXD-5 @kpi @C7a @error @contract-shape:bounded-change
/// Scenario Outline: A pass with no usable DID list refuses, publishes nothing, and pages
///   Given the 14:00 pass published Priya's claim
///   And <situation>
///   When the schedule fires a pass
///   Then the pass reports exit 2 for an unreadable list, and the invocation itself succeeds
///   And the index object is exactly the 14:00 one
///   And the pass record says exit 2, not published
///   Examples:
///     | situation                                                              |
///     | the parameter store is unavailable and the instance is starting cold   |
///     | the parameter store has been unavailable for more than 2 hours         |
/// ```
#[test]
#[ignore = "RED scaffold: DELIVER (no usable list → ADR-081 refusal)"]
fn a_pass_with_no_usable_did_list_refuses_publishes_nothing_and_pages() {
    each_row(["cold", "copy too old"], |situation| {
        let world = LambdaWorld::listing(&[Author::Priya]);
        world.publishes(Author::Priya, &[CARGO_PIN_REPRODUCIBLE_BUILDS]);
        let warm = world.cold_pass_instance();
        warm.schedule_fires().assert_exit(Exit::Completed);
        world.publishes(Author::Priya, &[CARGO_PIN_TEST_DRIVEN]);
        world.parameter_store_fails(true);
        let before = world.pass_universe();

        let run = match *situation {
            "cold" => when_the_next_schedule_fires(&world),
            _ => {
                world.time_passes(2 * 60 * MINUTE + MINUTE);
                warm.schedule_fires()
            }
        };

        let summary = run.assert_exit(Exit::Failed);
        assert_cause(&summary, "repo_dids_unreadable", &run);
        assert_state_delta(
            &before,
            &world.pass_universe(),
            &world.pass_universe_names(),
            &recorded_only(&summary, Exit::Failed, seen_at_start(&before)),
        );
    });
}

/// PA-E4
/// ```gherkin
/// @driving_port @real-io @ADR-081 @C6a @error @contract-shape:bounded-change
/// Scenario: A pass refuses a DID list with a typo and keeps the index as it was
///   Given the 14:00 pass published Priya's claim
///   And the operator saved a list whose second entry is "did:plc:" with nothing after it
///   When the 14:15 schedule fires the pass
///   Then the pass reports exit 2 for a malformed list and names the bad entry
///   And nobody is purged and the index object is exactly the 14:00 one
/// ```
#[test]
#[ignore = "RED scaffold: DELIVER (list from the parameter store through the ADR-081 loader)"]
fn a_pass_refuses_a_did_list_with_a_typo_and_keeps_the_index_as_it_was() {
    let world = LambdaWorld::listing(&[Author::Priya]);
    given_the_first_pass_published_priyas_claim(&world);
    world.operator_saves_list_text(&format!("{}\ndid:plc:\n", Author::Priya.did()));
    let before = world.pass_universe();

    let run = when_the_next_schedule_fires(&world);

    let summary = run.assert_exit(Exit::Failed);
    assert_cause(&summary, "repo_dids_malformed", &run);
    assert_eq!(summary["purged_authors"], 0, "{}", run.dump());
    assert!(
        run.events.iter().any(|e| e["value"] == "did:plc:"),
        "the bad entry is named\n{}",
        run.dump()
    );
    assert_state_delta(
        &before,
        &world.pass_universe(),
        &world.pass_universe_names(),
        &recorded_only(&summary, Exit::Failed, seen_at_start(&before)),
    );
}

/// PA-E5 (user decision 2026-10-30, UI-2: a failed pass, not a startup refusal.
/// A download that failed saw no object, so the record says null; a damaged
/// object was seen, and its `published-at` is what the probe must compare, or a
/// published but unusable index would hide from A10.)
/// ```gherkin
/// @driving_port @real-io @D-STZ-41 @D-STZ-48 @UI-2 @KPI-IXD-5 @kpi @C6b @C7a @error @contract-shape:bounded-change
/// Scenario Outline: A pass that cannot get a usable starting index is a failed pass
///   Given the 14:00 pass published Priya's claim
///   And Priya approved a test-driven claim
///   And <trouble> when the 14:15 pass starts
///   When the 14:15 schedule fires the pass
///   Then the pass reports exit 2 because it has no usable starting index, and the invocation itself succeeds
///   And nothing is published and the index object is exactly what it was
///   And the pass record says exit 2, not published, and <seen>
///   Examples:
///     | trouble                                         | seen                          |
///     | downloading the index fails                     | that it saw no index          |
///     | the store refuses to hand over the index        | that it saw no index          |
///     | the instance has no room left to download it    | that it saw no index          |
///     | the published index is damaged                  | that it saw the damaged index |
/// ```
#[test]
#[ignore = "RED scaffold: DELIVER (pass start failures, UI-2)"]
fn a_pass_that_cannot_get_a_usable_starting_index_is_a_failed_pass() {
    each_row(
        ["download fails", "refused", "no room", "damaged"],
        |trouble| {
            let world = LambdaWorld::listing(&[Author::Priya]);
            given_the_first_pass_published_priyas_claim(&world);
            world.publishes(Author::Priya, &[CARGO_PIN_TEST_DRIVEN]);
            match *trouble {
                "download fails" => world.bucket.has_trouble(ObjectTrouble::DownloadsFail),
                "refused" => world.bucket.has_trouble(ObjectTrouble::DownloadsRefused),
                "damaged" => world.bucket.holds_a_damaged_index(),
                _ => world.bucket.has_trouble(ObjectTrouble::ScratchSpaceFull),
            }
            let before = world.pass_universe();
            let seen = match *trouble {
                "damaged" => seen_at_start(&before),
                _ => Value::Null,
            };

            let run = when_the_next_schedule_fires(&world);

            let summary = run.assert_exit(Exit::Failed);
            assert_cause(&summary, "starting_index_unusable", &run);
            assert_eq!(run.payload()["published"], false, "{}", run.dump());
            assert_state_delta(
                &before,
                &world.pass_universe(),
                &world.pass_universe_names(),
                &recorded_only(&summary, Exit::Failed, seen),
            );
        },
    );
}

/// PA-E6
/// ```gherkin
/// @driving_port @real-io @U-STZ-5 @D-STZ-43 @ADR-080-7 @KPI-IXD-5 @kpi @C7b @error @contract-shape:bounded-change
/// Scenario: A pass that panics part way is reported as a failed pass and publishes nothing
///   Given the 14:00 pass published Priya's claim
///   And Priya approved a test-driven claim
///   And the 14:15 pass panics part way
///   When the 14:15 schedule fires the pass
///   Then the pass reports exit 2 because it panicked, and the invocation itself succeeds
///   And the index object is exactly the 14:00 one
///   And the pass record says exit 2, not published
/// ```
#[test]
#[ignore = "RED scaffold: DELIVER (the reused in_serve_pass path turns a panic into exit 2, D-STZ-42)"]
fn a_pass_that_panics_part_way_is_reported_as_a_failed_pass_and_publishes_nothing() {
    let world = LambdaWorld::listing(&[Author::Priya]);
    given_the_first_pass_published_priyas_claim(&world);
    world.publishes(Author::Priya, &[CARGO_PIN_TEST_DRIVEN]);
    world.with_fault(Fault::FirstPassPanics);
    let before = world.pass_universe();

    let run = when_the_next_schedule_fires(&world);

    let summary = run.assert_exit(Exit::Failed);
    assert_cause(&summary, "pass_panicked", &run);
    assert_state_delta(
        &before,
        &world.pass_universe(),
        &world.pass_universe_names(),
        &recorded_only(&summary, Exit::Failed, seen_at_start(&before)),
    );
}

/// PA-E7 (D-STZ-41: the pass record cannot be written when the store is down.
/// The instance starts cold in the outage: the write probe does not refuse it,
/// UI-17.)
/// ```gherkin
/// @driving_port @real-io @D-STZ-41 @KPI-IXD-5 @kpi @C7a @error @contract-shape:bounded-change
/// Scenario: A pass during a full store outage still reports a failed pass
///   Given the 14:00 pass published Priya's claim
///   And every request to the store fails from 14:10
///   When the 14:15 schedule fires the pass on a cold instance
///   Then the pass reports exit 2 because it has no usable starting index, and the invocation itself succeeds
///   And nothing is published, and the pass record still names the 14:00 pass
/// ```
#[test]
#[ignore = "RED scaffold: DELIVER (a failed pass survives a store that refuses the record too)"]
fn a_pass_during_a_full_store_outage_still_reports_a_failed_pass() {
    let world = LambdaWorld::listing(&[Author::Priya]);
    given_the_first_pass_published_priyas_claim(&world);
    world.bucket.has_trouble(ObjectTrouble::Unavailable);
    let before = world.pass_universe();

    let run = when_the_next_schedule_fires(&world);

    let summary = run.assert_exit(Exit::Failed);
    assert_cause(&summary, "starting_index_unusable", &run);
    assert_eq!(run.payload()["published"], false, "{}", run.dump());
    assert_state_delta(
        &before,
        &world.pass_universe(),
        &world.pass_universe_names(),
        &Delta::new().with_slot("events.pass_summary.count", grew_by(1)),
    );
}

/// PA-E8
/// ```gherkin
/// @driving_port @real-io @D-STZ-41 @R-STZ-7 @KPI-IXD-5 @kpi @C6b @error @contract-shape:bounded-change
/// Scenario: A pass that cannot publish its changes fails and keeps the index as it was
///   Given the 14:00 pass published Priya's claim
///   And Priya approved a test-driven claim
///   And the store fails the publish of the 14:15 pass
///   When the 14:15 schedule fires the pass
///   Then the pass reports exit 2 because the publish failed, and the invocation itself succeeds
///   And the index object is exactly the 14:00 one
///   And the pass record says exit 2, not published, seeing the 14:00 index
/// ```
#[test]
#[ignore = "RED scaffold: DELIVER (publish store error → exit 2 publish_failed)"]
fn a_pass_that_cannot_publish_its_changes_fails_and_keeps_the_index_as_it_was() {
    let world = LambdaWorld::listing(&[Author::Priya]);
    given_the_first_pass_published_priyas_claim(&world);
    world.publishes(Author::Priya, &[CARGO_PIN_TEST_DRIVEN]);
    world.bucket.has_trouble(ObjectTrouble::PublishFails);
    let before = world.pass_universe();

    let run = when_the_next_schedule_fires(&world);

    let summary = run.assert_exit(Exit::Failed);
    assert_cause(&summary, "publish_failed", &run);
    assert_eq!(run.payload()["published"], false, "{}", run.dump());
    assert_state_delta(
        &before,
        &world.pass_universe(),
        &world.pass_universe_names(),
        &recorded_only(&summary, Exit::Failed, seen_at_start(&before)),
    );
}

/// PA-E9 (D-STZ-40, D-STZ-41: exit 2 pages through A2, so a lost record cannot go
/// unnoticed until A3)
/// ```gherkin
/// @driving_port @real-io @U-STZ-3 @U-STZ-5 @D-STZ-40 @D-STZ-41 @KPI-IXD-5 @kpi @C7a @error @contract-shape:bounded-change
/// Scenario: A pass whose record cannot be written reports exit 2 and its index stays published
///   Given the 14:00 pass published Priya's claim
///   And Priya approved a test-driven claim
///   And the store will not take the pass record
///   When the 14:15 schedule fires the pass
///   Then the pass reports exit 2 because the record failed, and the invocation itself succeeds
///   And the index is published with both of Priya's claims
///   And the pass record still names the 14:00 pass
/// ```
#[test]
#[ignore = "RED scaffold: DELIVER (a record write failure is exit 2 pass_record_failed)"]
fn a_pass_whose_record_cannot_be_written_reports_exit_2_and_its_index_stays_published() {
    let world = LambdaWorld::listing(&[Author::Priya]);
    given_the_first_pass_published_priyas_claim(&world);
    world.publishes(Author::Priya, &[CARGO_PIN_TEST_DRIVEN]);
    world
        .bucket
        .has_trouble(ObjectTrouble::PassRecordCannotBeWritten);
    let before = world.pass_universe();

    let run = when_the_next_schedule_fires(&world);

    let summary = run.assert_exit(Exit::Failed);
    assert_cause(&summary, "pass_record_failed", &run);
    assert!(run.published(), "{}", run.dump());
    assert_state_delta(
        &before,
        &world.pass_universe(),
        &world.pass_universe_names(),
        &Delta::new()
            .with_slot("index.etag", changed())
            .with_slot("index.published_at", published_at_is(&world))
            .with_slot("index.version_count", grew_by(1))
            .with_slot("events.pass_summary.count", grew_by(1))
            .with_slot("events.lambda.published.count", grew_by(1)),
    );
    assert_search_finds_both_of_priyas_claims(&world);
}

/// PA-E10 (D-STZ-39: the single-writer guarantee rests on the store honouring
/// conditions. Two 2xx on the write check's two PUTs is the only refusal.)
/// ```gherkin
/// @driving_port @real-io @D-STZ-39 @KPI-IXD-5 @kpi @C6b @error @contract-shape:bounded-change
/// Scenario Outline: A store that ignores the write condition is refused before a pass can trust it
///   Given <history>
///   And the store now accepts writes it should refuse
///   When a pass instance starts
///   Then it refuses to start and logs that its startup was refused
///   And both writes of the write check landed on its own marker
///   And no pass ran, and the index and the pass record are exactly what they were
///   Examples:
///     | history                                      |
///     | the 14:00 pass published Priya's claim       |
///     | no pass has ever run against this bucket     |
/// ```
#[test]
#[ignore = "RED scaffold: DELIVER (write probe at pass init)"]
fn a_store_that_ignores_the_write_condition_is_refused_before_a_pass_can_trust_it() {
    each_row(["a later run", "the very first run"], |history| {
        let world = LambdaWorld::listing(&[Author::Priya]);
        if *history == "a later run" {
            given_the_first_pass_published_priyas_claim(&world);
        }
        world.bucket.ignores_write_conditions();
        let before = world.pass_universe();
        let mark = world.events.mark();

        let started = world.pass_instance_starts();

        assert!(started.is_err(), "init must be refused");
        assert_eq!(
            named(&world.events.since(mark), "health.startup.refused").len(),
            1,
            "{:?}",
            world.events.since(mark)
        );
        assert_state_delta(
            &before,
            &world.pass_universe(),
            &world.pass_universe_names(),
            &Delta::new().with_slot("write_check_marker.versions", grew_by(2)),
        );
    });
}

/// PA-E11 (D-STZ-44, strict: a mistyped gate invoke stops at once. The write check
/// ran at init, before the event was read, so the calls are counted after it.)
/// ```gherkin
/// @driving_port @real-io @D-STZ-44 @KPI-STZ-G5 @kpi @C6a @error @contract-shape:unbounded-preservation
/// Scenario Outline: A pass refuses any event but an empty one or a no-publish request
///   Given the 14:00 pass published Priya's claim
///   And Priya approved a test-driven claim
///   When a pass instance is invoked with <event>
///   Then the invocation is a function error and no pass ran, so there is no summary
///   And after the instance started, neither the store nor the parameter store was asked anything
///   And nothing under the index changed
///   Examples:
///     | event                               |
///     | a mode nobody knows                 |
///     | an empty mode                       |
///     | a mode with no value                |
///     | a mode that is not text             |
///     | no-publish with another key beside it |
///     | a misspelled mode key               |
///     | a list                              |
///     | some text                           |
///     | nothing                             |
/// ```
#[test]
#[ignore = "RED scaffold: DELIVER (decide::pass_mode, D-STZ-44)"]
fn a_pass_refuses_any_event_but_an_empty_one_or_a_no_publish_request() {
    each_row(
        [
            json!({"mode": "rehearsal"}),
            json!({"mode": ""}),
            json!({"mode": null}),
            json!({"mode": false}),
            json!({"mode": "no-publish", "reason": "gate"}),
            json!({"mood": "no-publish"}),
            json!([]),
            json!("no-publish"),
            json!(null),
        ],
        |event| {
            let world = LambdaWorld::listing(&[Author::Priya]);
            given_the_first_pass_published_priyas_claim(&world);
            world.publishes(Author::Priya, &[CARGO_PIN_TEST_DRIVEN]);
            world.time_passes(15 * MINUTE);
            let pass = world.cold_pass_instance();
            let before = world.pass_universe();
            let (requests, reads) = (world.bucket.requests().len(), world.did_list.reads());

            let run = pass.invoked_with(event.clone());

            assert!(run.answer.is_err(), "{}", run.dump());
            assert!(run.summaries().is_empty(), "{}", run.dump());
            assert_eq!(world.bucket.requests().len(), requests, "{}", run.dump());
            assert_eq!(world.did_list.reads(), reads, "{}", run.dump());
            assert_state_delta(
                &before,
                &world.pass_universe(),
                &world.pass_universe_names(),
                &Delta::new(),
            );
        },
    );
}

/// PA-E12 (D-STZ-39, UI-21: the check is at connect, before any PUT. It is in all
/// three functions because each one reads the keys. The variable names are
/// DISTILL-proposed.)
/// ```gherkin
/// @driving_port @real-io @D-STZ-39 @UI-21 @KPI-IXD-5 @kpi @C6a @error @contract-shape:unbounded-preservation
/// Scenario Outline: A function configured to keep its data under the write check's marker never starts
///   Given <function> is configured to keep <what> under the write check's marker
///   When an instance starts
///   Then it refuses to start and logs that its startup was refused, naming the marker
///   And nothing under the index changed
///   Examples:
///     | function | what            |
///     | search   | the index       |
///     | the pass | the index       |
///     | the pass | the pass record |
///     | the probe | the pass record |
/// ```
#[test]
#[ignore = "RED scaffold: DELIVER (config refusal at init, D-STZ-39)"]
fn a_function_configured_to_keep_its_data_under_the_write_checks_marker_never_starts() {
    each_row(
        [
            ("search", var::INDEX_KEY),
            ("pass", var::INDEX_KEY),
            ("pass", var::LAST_PASS_KEY),
            ("probe", var::LAST_PASS_KEY),
        ],
        |(function, variable)| {
            let world = LambdaWorld::listing(&[Author::Priya]);
            let edge = CannedEdge::answering(200, 200);
            world.setting(variable, WRITE_CHECK_MARKER_KEY);
            let before = world.bucket_universe();
            let mark = world.events.mark();

            let refused = match *function {
                "search" => world.search_instance_starts().is_err(),
                "pass" => world.pass_instance_starts().is_err(),
                _ => world.probe_instance_starts(edge.url()).is_err(),
            };

            assert!(refused, "init must be refused");
            let refusals = named(&world.events.since(mark), "health.startup.refused");
            assert_eq!(refusals.len(), 1, "{:?}", world.events.since(mark));
            assert!(
                refusals[0].to_string().contains(WRITE_CHECK_MARKER_KEY),
                "{}",
                refusals[0]
            );
            assert_state_delta(
                &before,
                &world.bucket_universe(),
                &world.bucket_universe_names(),
                &Delta::new(),
            );
        },
    );
}

// =============================================================================
// Helpers (observables only)
// =============================================================================

/// The delta of a pass that published one new version and recorded it (Mandate 8).
fn published_and_recorded(world: &LambdaWorld, summary: &Value, exit: Exit) -> Delta<Value> {
    Delta::new()
        .with_slot("index.etag", changed())
        .with_slot("index.published_at", published_at_is(world))
        .with_slot("index.version_count", grew_by(1))
        .with_slot(
            "last_pass.pass_id",
            set_to(json!(id_text(&summary["pass_id"]))),
        )
        .with_slot("last_pass.exit_code", set_to(json!(exit.code())))
        .with_slot("last_pass.published", set_to(json!(true)))
        .with_slot("last_pass.index_published_at", published_at_is(world))
        .with_slot("events.pass_summary.count", grew_by(1))
        .with_slot("events.lambda.published.count", grew_by(1))
}

/// The delta of a pass that published nothing and recorded `index_published_at`.
fn recorded_only(summary: &Value, exit: Exit, index_published_at: Value) -> Delta<Value> {
    Delta::new()
        .with_slot(
            "last_pass.pass_id",
            set_to(json!(id_text(&summary["pass_id"]))),
        )
        .with_slot("last_pass.exit_code", set_to(json!(exit.code())))
        .with_slot("last_pass.published", set_to(json!(false)))
        .with_slot("last_pass.index_published_at", set_to(index_published_at))
        .with_slot("events.pass_summary.count", grew_by(1))
}

/// The `published-at` of the object the pass saw at its start (D-STZ-48).
fn seen_at_start(before: &Snapshot) -> Value {
    before["index.published_at"].clone()
}

/// A search instance that starts afterwards finds both of Priya's claims.
fn assert_search_finds_both_of_priyas_claims(world: &LambdaWorld) {
    let search = world.cold_search_instance();
    for philosophy in ["reproducible-builds", "test-driven"] {
        let reply = search.maria_searches(philosophy);
        assert_eq!(
            reply.subjects_by(Author::Priya),
            vec![CARGO_PIN_TEST_DRIVEN.subject.to_string()],
            "{philosophy}: {}",
            reply.text()
        );
    }
}

/// An exit-2 summary names exactly this cause (ADR-078, D-STZ-41).
fn assert_cause(summary: &Value, cause: &str, run: &PassRun) {
    assert_eq!(summary["cause"], cause, "{}", run.dump());
}

/// A pass id as the pass record stores it (text).
fn id_text(pass_id: &Value) -> String {
    pass_id
        .as_str()
        .map(str::to_string)
        .unwrap_or_else(|| pass_id.to_string())
}

/// Slot predicate: the index's `published-at` is the clock's time now.
fn published_at_is(world: &LambdaWorld) -> state_delta::SlotPredicate<Value> {
    let now = world.clock.now();
    Box::new(move |_before: &Value, after: &Value| {
        let at: Option<chrono::DateTime<chrono::Utc>> = after.as_str().and_then(|s| s.parse().ok());
        if at == Some(now) {
            Ok(())
        } else {
            Err(format!("expected published at {now}, got {after}"))
        }
    })
}
