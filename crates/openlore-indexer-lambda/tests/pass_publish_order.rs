//! Crafter-owned guards (steps 01-04, 01-10, 04-03, 06-01): the warm copy, the
//! closed file, the publish's time limit and the no-publish arm with no usable
//! starting index.
//! The D-STZ-40 order is PA-11's.
#![cfg(unix)]

#[path = "../../../tests/common/state_delta.rs"]
mod state_delta;

#[path = "../../../tests/acceptance/support/indexer_lambda.rs"]
mod indexer_lambda;

use indexer_lambda::claims::*;
use indexer_lambda::*;

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

/// D-STZ-40a: the limit reaches the port. A pass this quick still has the whole
/// budget; `publish_limit` holds how it shrinks, and `publish_time_limit` the rule.
#[test]
fn a_publish_is_handed_its_time_limit() {
    let world = LambdaWorld::listing(&[Author::Priya]);
    world.publishes(Author::Priya, &[CARGO_PIN_REPRODUCIBLE_BUILDS]);

    world.cold_pass_instance().schedule_fires();

    assert_eq!(
        world.bucket.publish_limits(),
        [std::time::Duration::from_secs(35)]
    );
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

/// D-STZ-41, D-STZ-44: no usable starting index fails a no-publish run too, with
/// its one summary, and that arm still writes nothing.
#[test]
fn a_no_publish_run_with_no_usable_starting_index_fails_and_writes_nothing() {
    for trouble in ["download fails", "damaged"] {
        let world = LambdaWorld::listing(&[Author::Priya]);
        world.publishes(Author::Priya, &[CARGO_PIN_REPRODUCIBLE_BUILDS]);
        world.given_a_pass_published_the_index();
        world.publishes(Author::Priya, &[CARGO_PIN_TEST_DRIVEN]);
        match trouble {
            "damaged" => world.bucket.holds_a_damaged_index(),
            _ => world.bucket.has_trouble(ObjectTrouble::DownloadsFail),
        }
        let before = world.bucket_universe();
        let (pass, writes) = world.cold_pass_instance_forbidden_to_write();

        let run = pass.invoked_with(no_publish_event());

        let summary = run.assert_exit(Exit::Failed);
        assert_eq!(
            summary["cause"],
            "starting_index_unusable",
            "{trouble}: {}",
            run.dump()
        );
        assert_eq!(summary["no_publish"], true, "{trouble}: {}", run.dump());
        assert_eq!(run.payload()["would_publish"], false, "{trouble}");
        assert_eq!(
            writes.attempts(),
            0,
            "{trouble}: no publish or record write"
        );
        assert_eq!(before, world.bucket_universe(), "{trouble}");
    }
}
