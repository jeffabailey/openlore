//! Crafter-owned guards (steps 01-04, 01-10): the D-STZ-40 order, the warm copy
//! and the closed file. WS-1 and PA-1 read only the end state, so nothing else
//! pins the order until PA-11 (02-03).
#![cfg(unix)]

#[path = "../../../tests/common/state_delta.rs"]
mod state_delta;

#[path = "../../../tests/acceptance/support/indexer_lambda.rs"]
mod indexer_lambda;

use indexer_lambda::claims::*;
use indexer_lambda::*;

#[test]
fn the_pass_publishes_then_records_itself_then_writes_its_summary() {
    let world = LambdaWorld::listing(&[Author::Priya]);
    world.publishes(Author::Priya, &[CARGO_PIN_REPRODUCIBLE_BUILDS]);
    let mark = world.timeline.mark();

    world.cold_pass_instance().schedule_fires();

    let at = |step| world.timeline.position_since(mark, step);
    let (publish, record, summary) = (at(step::PUBLISH), at(step::RECORD), at(step::SUMMARY));
    assert!(
        publish.is_some() && publish < record && record < summary,
        "{:?}",
        world.timeline.since(mark)
    );
}

/// P-7, on the real store: what a pass uploads is its closed file, with nothing
/// left in the log. `ready_to_upload` holds the rule for a log that is not empty.
#[test]
fn a_pass_uploads_its_closed_file_with_nothing_left_in_the_log() {
    let world = LambdaWorld::listing(&[Author::Priya]);
    world.publishes(Author::Priya, &[CARGO_PIN_REPRODUCIBLE_BUILDS]);
    let pass = world.cold_pass_instance();

    pass.schedule_fires().assert_exit(Exit::Completed);

    let log = std::fs::metadata(pass.scratch.join("index.duckdb.wal"));
    assert!(log.map_or(true, |found| found.len() == 0));
    let local = std::fs::read(pass.scratch.join("index.duckdb")).expect("the closed file");
    assert_eq!(world.bucket.current().expect("an object").bytes, local);
}

/// PA-2 activates later; until then this pins the publish on change, and the
/// record naming the object the pass saw at its start (D-STZ-48).
#[test]
fn a_pass_that_changes_nothing_names_the_index_it_saw_and_publishes_nothing() {
    let world = LambdaWorld::listing(&[Author::Priya]);
    world.publishes(Author::Priya, &[CARGO_PIN_REPRODUCIBLE_BUILDS]);
    world.given_a_pass_published_the_index();
    let stamped = world.bucket.current().expect("an object").published_at;
    world.time_passes(std::time::Duration::from_secs(15 * 60));

    let run = world.cold_pass_instance().schedule_fires();

    run.assert_exit(Exit::Completed);
    assert!(!run.published(), "{}", run.dump());
    assert_eq!(world.bucket.versions().len(), 1);
    let record = world.bucket.last_pass().expect("a record");
    assert!(!record.published);
    assert_eq!(record.index_published_at, stamped);
}

/// A warm instance's copy does not outlive `reset-index`: the pass starts from
/// nothing, so it finds its claims new and publishes them.
#[test]
fn a_warm_pass_after_the_index_was_reset_starts_from_nothing() {
    let world = LambdaWorld::listing(&[Author::Priya]);
    world.publishes(Author::Priya, &[CARGO_PIN_REPRODUCIBLE_BUILDS]);
    let pass = world.cold_pass_instance();
    pass.schedule_fires().assert_exit(Exit::Completed);
    world.bucket.operator_resets_the_index();

    let run = pass.schedule_fires();

    run.assert_exit(Exit::Completed);
    assert!(run.published(), "{}", run.dump());
    assert!(world.bucket.current().is_some());
}
