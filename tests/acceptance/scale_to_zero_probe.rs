//! scale-to-zero-hosting — the 5-minute probe that feeds alarms A3 and A10
//! (D-STZ-23, D-STZ-48, U-STZ-3, U-STZ-6, U-STZ-7).
//!
//! Driving port: the probe function's scheduler entry
//! ([`openlore_indexer_lambda::ProbeFunction::handle`]). The public edge is a
//! [`CannedEdge`] on loopback (or a refusing address); `last-pass.json` is in
//! the fake bucket. Layer 2, example-only; `not_live` and `served_index_stale`
//! themselves are property-tested in `scale_to_zero_core.rs`. Read-only: the
//! probe is composed with no write port, so a run changes the bucket in no way.
//!
//! The line contract (DEVOPS § Monitoring contracts): event
//! `indexer.host.health` (the existing A3 filter), `source: "lambda-probe"`,
//! `healthz_ok`, `search_ok`, `search_ms`, `search_status`, `last_pass_age_s`,
//! `last_pass_exit_code`, `not_live` (0 or 1). Exactly one such line per run. A
//! run can also write `indexer.lambda.served_index_stale {served, recorded,
//! lag_s}`, which A10 counts and which never changes `not_live`.
//!
//! The schedules' event is `{}`, so every scenario that fires the schedule also
//! proves `{}` runs a normal probe. An event that is not a JSON object is a
//! function error (PR-E3); otherwise a function error means a crash.
#![cfg(unix)]

#[path = "../common/state_delta.rs"]
mod state_delta;

#[path = "support/indexer_lambda.rs"]
mod indexer_lambda;

use std::sync::mpsc;
use std::time::Duration;

use indexer_lambda::*;
use serde_json::json;

const MINUTE: Duration = Duration::from_secs(60);

/// When the seeded record's index was published, in the scenarios that compare.
const RECORDED_AT: &str = "2026-10-27T14:00:00Z";

/// PR-1
/// ```gherkin
/// @driving_port @real-io @D-STZ-23 @U-STZ-6 @U-STZ-7 @R-STZ-7 @KPI-IXD-5 @kpi @contract-shape:unbounded-preservation
/// Scenario: The probe reports a live index
///   Given the public index answers its health check and the canned search
///   And the last pass completed 10 minutes ago
///   When the 5-minute schedule fires the probe with an empty event
///   Then it runs the canned search first and the health check after it
///   And it writes one health line saying the index is live
///   And the line names the probe as its source, the search time and status, and the last pass's age and exit code
/// ```
#[test]
#[ignore = "RED scaffold: DELIVER (probe mode)"]
fn the_probe_reports_a_live_index() {
    let world = LambdaWorld::listing(&[Author::Priya]);
    let edge = CannedEdge::answering(200, 200);
    world
        .bucket
        .records_last_pass(Exit::Completed, world.clock.now());
    world.time_passes(10 * MINUTE);
    let probe = world.cold_probe_instance(edge.url());

    let line = probe.schedule_fires().health_line();

    assert_eq!(edge.asked(), [SEARCH_PATH, HEALTH_PATH]);
    assert_eq!(line["not_live"], 0, "{line}");
    assert_eq!(line["source"], "lambda-probe", "{line}");
    assert_eq!(line["healthz_ok"], true, "{line}");
    assert_eq!(line["search_ok"], true, "{line}");
    assert_eq!(line["search_status"], 200, "{line}");
    assert!(line["search_ms"].is_u64(), "{line}");
    assert_eq!(line["last_pass_age_s"], 600, "{line}");
    assert_eq!(line["last_pass_exit_code"], 0, "{line}");
}

/// PR-2
/// ```gherkin
/// @driving_port @real-io @U-STZ-3 @KPI-IXD-5 @kpi @contract-shape:unbounded-preservation
/// Scenario: A failed pass that just ran does not also make the index look dead
///   Given the public index answers its health check and the canned search
///   And the last pass ended with exit 2 five minutes ago
///   When the 5-minute schedule fires the probe
///   Then the health line says the index is live, with the last exit code 2
/// ```
#[test]
#[ignore = "RED scaffold: DELIVER (probe mode; one cause pages once)"]
fn a_failed_pass_that_just_ran_does_not_also_make_the_index_look_dead() {
    let world = LambdaWorld::listing(&[Author::Priya]);
    let edge = CannedEdge::answering(200, 200);
    world
        .bucket
        .records_last_pass(Exit::Failed, world.clock.now());
    world.time_passes(5 * MINUTE);
    let probe = world.cold_probe_instance(edge.url());

    let line = probe.schedule_fires().health_line();

    assert_eq!(line["not_live"], 0, "{line}");
    assert_eq!(line["last_pass_exit_code"], 2, "{line}");
}

/// PR-3 (D-STZ-48: a stale copy is still a correct index, so it alarms through
/// A10 and the index stays live)
/// ```gherkin
/// @driving_port @real-io @D-STZ-48 @U-STZ-6 @R-STZ-10 @error @contract-shape:unbounded-preservation
/// Scenario Outline: The probe reports a search stuck on an old copy, without calling the index dead
///   Given the last pass recorded the index published at 14:00, 10 minutes ago
///   And the public index says it serves <what it serves>
///   When the 5-minute schedule fires the probe
///   Then it writes a stale-index line naming both times and a lag of <lag>
///   And its one health line says the index is live
///   Examples:
///     | what it serves          | lag          |
///     | a copy from 13:45:01    | 899 seconds  |
///     | nothing published       | no figure    |
/// ```
#[test]
#[ignore = "RED scaffold: DELIVER (probe mode, served_index_stale)"]
fn the_probe_reports_a_search_stuck_on_an_old_copy_without_calling_the_index_dead() {
    each_row(
        [(Some(SERVED_AT), json!(899)), (None, json!(null))],
        |(served, lag_s)| {
            let world = LambdaWorld::listing(&[Author::Priya]);
            let edge = CannedEdge::serving_an_index_published_at(*served);
            world.bucket.records_a_pass_of_the_index(
                Exit::Completed,
                world.clock.now(),
                Some(RECORDED_AT),
            );
            world.time_passes(10 * MINUTE);
            let probe = world.cold_probe_instance(edge.url());

            let run = probe.schedule_fires();

            let line = run.health_line();
            let readings = run.stale_readings();
            assert_eq!(readings.len(), 1, "{:?}", run.events);
            assert_eq!(readings[0]["served"], json!(served), "{}", readings[0]);
            assert_eq!(readings[0]["recorded"], RECORDED_AT, "{}", readings[0]);
            assert_eq!(readings[0]["lag_s"], *lag_s, "{}", readings[0]);
            assert_eq!(line["not_live"], 0, "{line}");
        },
    );
}

/// One PR-4 row.
#[derive(Debug, Clone, Copy)]
enum NoReading {
    NothingRecorded,
    HealthCheckFails,
    RecordUnreadable,
    ServedTheRecordedCopy,
    ServedTheSameInstantWrittenDifferently,
    ServedANewerCopy,
    RecordOnlyJustWritten,
}

/// PR-4 (`decide::served_index_stale` holds the full table; these rows prove the
/// probe hands it what it read)
/// ```gherkin
/// @driving_port @real-io @D-STZ-48 @U-STZ-6 @C1b @edge @contract-shape:unbounded-preservation
/// Scenario Outline: The probe writes no stale-index line when it has nothing to compare or nothing lags
///   Given <situation>
///   When the 5-minute schedule fires the probe
///   Then it writes no stale-index line
///   Examples:
///     | situation                                                       |
///     | the pass record says no index was published                     |
///     | the health check fails                                          |
///     | the pass record cannot be read                                  |
///     | the public index serves the recorded copy                       |
///     | it serves that copy, with the time written in another offset    |
///     | it serves a newer copy than the record names                    |
///     | the record is exactly 2 minutes old, so a reload may be running |
/// ```
#[test]
#[ignore = "RED scaffold: DELIVER (probe mode, served_index_stale)"]
fn the_probe_writes_no_stale_index_line_when_it_has_nothing_to_compare_or_nothing_lags() {
    use NoReading::*;
    each_row(
        [
            NothingRecorded,
            HealthCheckFails,
            RecordUnreadable,
            ServedTheRecordedCopy,
            ServedTheSameInstantWrittenDifferently,
            ServedANewerCopy,
            RecordOnlyJustWritten,
        ],
        |row| {
            let world = LambdaWorld::listing(&[Author::Priya]);
            let recorded = match row {
                NothingRecorded => None,
                _ => Some(RECORDED_AT),
            };
            let edge = match row {
                HealthCheckFails => CannedEdge::answering(503, 200),
                NothingRecorded => CannedEdge::answering(200, 200),
                ServedTheRecordedCopy => {
                    CannedEdge::serving_an_index_published_at(Some(RECORDED_AT))
                }
                ServedTheSameInstantWrittenDifferently => {
                    CannedEdge::serving_an_index_published_at(Some("2026-10-27T16:00:00+02:00"))
                }
                ServedANewerCopy => {
                    CannedEdge::serving_an_index_published_at(Some("2026-10-27T14:00:01Z"))
                }
                RecordUnreadable | RecordOnlyJustWritten => {
                    CannedEdge::serving_an_index_published_at(Some(SERVED_AT))
                }
            };
            world
                .bucket
                .records_a_pass_of_the_index(Exit::Completed, world.clock.now(), recorded);
            if matches!(row, RecordUnreadable) {
                world.bucket.last_pass_cannot_be_read();
            }
            world.time_passes(match row {
                RecordOnlyJustWritten => 2 * MINUTE,
                _ => 10 * MINUTE,
            });
            let probe = world.cold_probe_instance(edge.url());

            let run = probe.schedule_fires();

            run.health_line();
            assert!(run.stale_readings().is_empty(), "{:?}", run.events);
        },
    );
}

/// PR-B1
/// ```gherkin
/// @driving_port @real-io @D-STZ-23 @C1b @edge @contract-shape:unbounded-preservation
/// Scenario: A pass exactly 45 minutes old still counts as recent
///   Given the public index answers its health check and the canned search
///   And the last pass completed exactly 45 minutes ago
///   When the 5-minute schedule fires the probe
///   Then the health line says the index is live
/// ```
#[test]
#[ignore = "RED scaffold: DELIVER (probe mode)"]
fn a_pass_exactly_45_minutes_old_still_counts_as_recent() {
    let world = LambdaWorld::listing(&[Author::Priya]);
    let edge = CannedEdge::answering(200, 200);
    world
        .bucket
        .records_last_pass(Exit::Completed, world.clock.now());
    world.time_passes(45 * MINUTE);
    let probe = world.cold_probe_instance(edge.url());

    let line = probe.schedule_fires().health_line();

    assert_eq!(line["not_live"], 0, "{line}");
    assert_eq!(line["last_pass_age_s"], 2700, "{line}");
}

/// One PR-E1 row.
#[derive(Debug, Clone, Copy)]
enum Failure {
    EdgeUnreachable,
    StoreUnusable,
    SearchFails,
    PassTooOld,
    NoPassRecord,
    RecordUnreadable,
    RecordGarbled,
}

/// PR-E1
/// ```gherkin
/// @driving_port @real-io @D-STZ-23 @R-STZ-7 @UI-12 @KPI-IXD-3 @kpi @C1b @C6b @C7a @error
/// @contract-shape:unbounded-preservation
/// Scenario Outline: The probe says the index is not live, and why, for each failure
///   Given <situation>
///   When the 5-minute schedule fires the probe
///   Then the invocation succeeds with one health line saying the index is not live
///   And the line shows <evidence>
///   Examples:
///     | situation                                                   | evidence                     |
///     | the public index cannot be reached                          | the health check failed      |
///     | the public index says its store is unusable                 | the health check failed      |
///     | the canned search fails on the index's side                 | the search check failed      |
///     | the last pass finished 45 minutes and 1 second ago          | the age of that last pass    |
///     | no pass has ever recorded itself                            | no age for a last pass       |
///     | the pass record cannot be read                              | no age for a last pass       |
///     | the pass record holds a time nobody can read                | no age for a last pass       |
/// ```
#[test]
#[ignore = "RED scaffold: DELIVER (probe mode)"]
fn the_probe_says_the_index_is_not_live_and_why_for_each_failure() {
    use Failure::*;
    let refusing = openlore_test_support::refusing_address();
    each_row(
        [
            EdgeUnreachable,
            StoreUnusable,
            SearchFails,
            PassTooOld,
            NoPassRecord,
            RecordUnreadable,
            RecordGarbled,
        ],
        |failure| {
            let world = LambdaWorld::listing(&[Author::Priya]);
            let (healthz, search) = match failure {
                StoreUnusable => (503, 200),
                SearchFails => (200, 500),
                _ => (200, 200),
            };
            let edge = CannedEdge::answering(healthz, search);
            let edge_url = match failure {
                EdgeUnreachable => refusing.http_url(),
                _ => edge.url().to_string(),
            };
            let now = world.clock.now();
            match failure {
                NoPassRecord => {}
                RecordGarbled => world.bucket.holds_a_garbled_pass_record(),
                RecordUnreadable => {
                    world.bucket.records_last_pass(Exit::Completed, now);
                    world.bucket.last_pass_cannot_be_read();
                }
                PassTooOld => {
                    world.bucket.records_last_pass(Exit::Completed, now);
                    world.time_passes(45 * MINUTE + Duration::from_secs(1));
                }
                EdgeUnreachable | StoreUnusable | SearchFails => {
                    world.bucket.records_last_pass(Exit::Completed, now)
                }
            }
            let probe = world.cold_probe_instance(&edge_url);

            let line = probe.schedule_fires().health_line();

            assert_eq!(line["not_live"], 1, "{line}");
            match failure {
                EdgeUnreachable | StoreUnusable => {
                    assert_eq!(line["healthz_ok"], false, "{line}")
                }
                SearchFails => {
                    assert_eq!(line["search_ok"], false, "{line}");
                    assert_eq!(line["search_status"], 500, "{line}");
                }
                PassTooOld => assert_eq!(line["last_pass_age_s"], 2701, "{line}"),
                NoPassRecord | RecordUnreadable | RecordGarbled => {
                    assert!(line["last_pass_age_s"].is_null(), "{line}")
                }
            }
        },
    );
}

/// PR-E2
/// ```gherkin
/// @driving_port @real-io @D-STZ-23 @D-STZ-30 @D-STZ-37 @KPI-IXD-5 @kpi @C7b @error
/// @contract-shape:unbounded-preservation
/// Scenario: A public index that hangs is not live, and the probe still reports it in time
///   Given the public index accepts connections and never answers
///   And the last pass completed 10 minutes ago
///   When the 5-minute schedule fires the probe
///   Then the invocation succeeds with one health line saying the index is not live
///   And the line is written inside the probe's 30-second limit
/// ```
#[test]
#[ignore = "RED scaffold: DELIVER (probe mode, bounded client timeouts)"]
fn a_public_index_that_hangs_is_not_live_and_the_probe_still_reports_it_in_time() {
    // On its own thread, so a probe that really hangs fails this test at the
    // limit instead of hanging the suite.
    let (answer, reported) = mpsc::channel();
    std::thread::spawn(move || {
        let world = LambdaWorld::listing(&[Author::Priya]);
        let edge = CannedEdge::hanging();
        world
            .bucket
            .records_last_pass(Exit::Completed, world.clock.now());
        world.time_passes(10 * MINUTE);
        let probe = world.cold_probe_instance(edge.url());

        let _ = answer.send(probe.schedule_fires().health_line());
    });

    let line = reported
        .recv_timeout(30 * Duration::from_secs(1))
        .expect("a health line inside the probe's 30-second limit");

    assert_eq!(line["not_live"], 1, "{line}");
    assert_eq!(line["healthz_ok"], false, "{line}");
}

/// PR-E3 (U-STZ-7: `lambda test-alarm errors` relies on this for a real, harmless
/// `Errors` datapoint)
/// ```gherkin
/// @driving_port @real-io @U-STZ-7 @D-STZ-23 @KPI-STZ-G5 @kpi @C6a @error @contract-shape:unbounded-preservation
/// Scenario Outline: The probe refuses an event that is not an object and does nothing else
///   Given the public index answers its health check and the canned search
///   When the probe is invoked with <event>
///   Then the invocation is a function error
///   And there is no health line, and after the instance started, neither the public index nor the store was asked anything
///   Examples:
///     | event     |
///     | a list    |
///     | some text |
///     | nothing   |
/// ```
#[test]
#[ignore = "RED scaffold: DELIVER (probe event check, U-STZ-7)"]
fn the_probe_refuses_an_event_that_is_not_an_object_and_does_nothing_else() {
    each_row([json!([]), json!("a string"), json!(null)], |event| {
        let world = LambdaWorld::listing(&[Author::Priya]);
        let edge = CannedEdge::answering(200, 200);
        let probe = world.cold_probe_instance(edge.url());
        let requests_at_init = world.bucket.requests().len();

        let run = probe.invoked_with(event.clone());

        assert!(run.answer.is_err(), "{:?}", run.answer);
        assert!(
            named(&run.events, "indexer.host.health").is_empty(),
            "{:?}",
            run.events
        );
        assert!(edge.asked().is_empty(), "{:?}", edge.asked());
        assert_eq!(world.bucket.requests().len(), requests_at_init);
    });
}

/// PR-E4
/// ```gherkin
/// @driving_port @real-io @D-STZ-23 @D-STZ-37 @C6a @error @contract-shape:unbounded-preservation
/// Scenario: A health answer the probe cannot read counts as a failed health check
///   Given the public index answers its health check with something that is not a health answer
///   And the last pass completed 10 minutes ago
///   When the 5-minute schedule fires the probe
///   Then the invocation succeeds with one health line saying the index is not live
/// ```
#[test]
#[ignore = "RED scaffold: DELIVER (probe mode, adapter-index-query health)"]
fn a_health_answer_the_probe_cannot_read_counts_as_a_failed_health_check() {
    let world = LambdaWorld::listing(&[Author::Priya]);
    let edge = CannedEdge::with_health_body(200, 200, "not a health answer".to_string());
    world
        .bucket
        .records_last_pass(Exit::Completed, world.clock.now());
    world.time_passes(10 * MINUTE);
    let probe = world.cold_probe_instance(edge.url());

    let line = probe.schedule_fires().health_line();

    assert_eq!(line["not_live"], 1, "{line}");
    assert_eq!(line["healthz_ok"], false, "{line}");
}
