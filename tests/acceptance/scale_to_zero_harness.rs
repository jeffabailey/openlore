//! scale-to-zero-hosting — checks of the acceptance harness itself.
//!
//! The doubles and fixture builders whose failure no scenario would show, and the
//! positive controls of the oracles the scenarios lean on. A double the scenarios
//! drive has no test here: the scenarios fail when it stops doing its job, and
//! RIO-1..3 hold the real S3 adapter to the conditional writes `FakeIndexBucket`
//! answers with. These are not scenarios.
#![cfg(unix)]

#[path = "../common/state_delta.rs"]
mod state_delta;

#[path = "support/indexer_lambda.rs"]
mod indexer_lambda;

use indexer_lambda::*;
use ports::{
    IndexObjectError, IndexObjectReadPort, IndexObjectWritePort, LastPass, ProbeOutcome,
    PublishMetadata, WriteCondition,
};
use serde_json::json;

/// What a publish is given, when the time it has is not what is under test.
const LIMIT: std::time::Duration = std::time::Duration::from_secs(35);

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("a test runtime")
}

fn metadata(pass_id: &str) -> PublishMetadata {
    PublishMetadata {
        published_at: START.to_string(),
        pass_id: pass_id.to_string(),
    }
}

fn record() -> LastPass {
    LastPass {
        pass_id: "pass-1".to_string(),
        exit_code: 2,
        finished_at: START.to_string(),
        published: false,
        index_published_at: None,
    }
}

#[test]
fn each_narrow_trouble_fails_only_its_own_request() {
    let rt = runtime();
    let dir = tempfile::tempdir().expect("scratch");
    let src = dir.path().join("index.duckdb");
    std::fs::write(&src, b"v1").expect("v1");
    let dest = dir.path().join("copy.duckdb");
    let seeded = || {
        let bucket = FakeIndexBucket::default();
        rt.block_on(bucket.publish_if_match(
            &src,
            WriteCondition::IfNoneMatch,
            metadata("p1"),
            LIMIT,
        ))
        .expect("publish");
        bucket
    };

    for trouble in [
        ObjectTrouble::DownloadsFail,
        ObjectTrouble::DownloadsRefused,
    ] {
        let bucket = seeded();
        bucket.has_trouble(trouble);
        assert!(rt.block_on(bucket.fetch_to(&dest)).is_err(), "{trouble:?}");
        assert!(rt.block_on(bucket.head()).is_ok(), "{trouble:?}");
        assert!(
            rt.block_on(bucket.put_last_pass(&record())).is_ok(),
            "{trouble:?}"
        );
    }

    let bucket = seeded();
    bucket.has_trouble(ObjectTrouble::PublishFails);
    let etag = bucket.current().expect("an object").etag;
    assert!(matches!(
        rt.block_on(bucket.publish_if_match(
            &src,
            WriteCondition::IfMatch(etag),
            metadata("p2"),
            LIMIT
        )),
        Err(IndexObjectError::Unavailable { .. })
    ));
    assert!(rt.block_on(bucket.put_last_pass(&record())).is_ok());

    let bucket = seeded();
    bucket.has_trouble(ObjectTrouble::PassRecordCannotBeWritten);
    assert!(rt.block_on(bucket.put_last_pass(&record())).is_err());
    assert!(rt.block_on(bucket.fetch_to(&dest)).is_ok());

    let bucket = seeded();
    bucket.has_trouble(ObjectTrouble::ScratchSpaceFull);
    assert!(matches!(
        rt.block_on(bucket.fetch_to(&dest)),
        Err(IndexObjectError::LocalWrite { .. })
    ));
    assert!(dest.exists(), "half the bytes landed");

    let bucket = seeded();
    bucket.has_trouble(ObjectTrouble::Unavailable);
    assert!(rt.block_on(bucket.put_last_pass(&record())).is_err());
    bucket.recovers();
    assert!(rt.block_on(bucket.put_last_pass(&record())).is_ok());
    assert_eq!(bucket.last_pass(), Some(record()));
}

#[test]
fn the_body_builders_land_on_the_exact_size_boundaries() {
    assert_eq!(search_body_of_size(8192).len(), 8192);
    assert_eq!(search_body_of_size(8193).len(), 8193);
    let largest: serde_json::Value =
        serde_json::from_slice(&search_body_of_size(8192)).expect("JSON");
    assert_eq!(largest["dimension"], "object");
    assert_eq!(
        largest["value"], "org.openlore.philosophy.reproducible-builds",
        "a search that can match something"
    );
    let body: serde_json::Value =
        serde_json::from_slice(&search_body_with_value_of(512)).expect("JSON");
    assert_eq!(body["value"].as_str().map(str::len), Some(512));
}

#[test]
fn the_write_check_creates_its_marker_once_and_catches_a_store_that_ignores_conditions() {
    let bucket = FakeIndexBucket::default();
    let check = |bucket: &FakeIndexBucket| IndexObjectWritePort::probe(bucket);

    assert!(matches!(check(&bucket), ProbeOutcome::Ok));
    assert!(matches!(check(&bucket), ProbeOutcome::Ok));
    assert_eq!(bucket.write_check_marker_versions(), 1);

    bucket.ignores_write_conditions();
    assert!(matches!(check(&bucket), ProbeOutcome::Refused { .. }));
    assert_eq!(bucket.write_check_marker_versions(), 3, "both PUTs landed");

    let on_a_first_run = FakeIndexBucket::default();
    on_a_first_run.ignores_write_conditions();
    assert!(matches!(
        check(&on_a_first_run),
        ProbeOutcome::Refused { .. }
    ));
    assert_eq!(on_a_first_run.write_check_marker_versions(), 2);
}

#[test]
fn an_outage_does_not_read_as_a_store_that_ignores_conditions() {
    for trouble in [ObjectTrouble::Unavailable, ObjectTrouble::Refuses] {
        let bucket = FakeIndexBucket::default();
        bucket.holds_the_write_check_marker();
        bucket.ignores_write_conditions();
        bucket.has_trouble(trouble);
        assert!(
            matches!(IndexObjectWritePort::probe(&bucket), ProbeOutcome::Ok),
            "{trouble:?}"
        );
    }
}

#[test]
fn forbidden_writes_count_every_object_write_and_leave_the_write_check_alone() {
    let rt = runtime();
    let bucket = std::sync::Arc::new(FakeIndexBucket::default());
    let writes = ForbiddenWrites::around(bucket.clone());
    let dir = tempfile::tempdir().expect("scratch");
    let src = dir.path().join("index.duckdb");
    std::fs::write(&src, b"v1").expect("v1");

    assert!(matches!(
        IndexObjectWritePort::probe(&writes),
        ProbeOutcome::Ok
    ));
    assert_eq!(bucket.write_check_marker_versions(), 1);
    assert_eq!(writes.attempts(), 0);

    assert!(rt
        .block_on(writes.publish_if_match(&src, WriteCondition::IfNoneMatch, metadata("p"), LIMIT))
        .is_err());
    assert!(rt.block_on(writes.put_last_pass(&record())).is_err());
    assert_eq!(writes.attempts(), 2);
    assert!(bucket.current().is_none());
    assert!(bucket.last_pass().is_none());
}

#[test]
fn the_timeline_orders_bucket_writes_and_event_lines_together() {
    let world = LambdaWorld::listing(&[Author::Priya]);
    let rt = runtime();
    let dir = tempfile::tempdir().expect("scratch");
    let src = dir.path().join("index.duckdb");
    std::fs::write(&src, b"v1").expect("v1");
    let mark = world.timeline.mark();

    rt.block_on(world.bucket.publish_if_match(
        &src,
        WriteCondition::IfNoneMatch,
        metadata("p"),
        LIMIT,
    ))
    .expect("publish");
    rt.block_on(world.bucket.put_last_pass(&record()))
        .expect("record");
    openlore_indexer_lambda::EventSink::line(
        world.events.as_ref(),
        r#"{"event":"indexer.ingest.pass_summary"}"#,
    );

    assert_eq!(
        world.timeline.since(mark),
        [step::PUBLISH, step::RECORD, step::SUMMARY]
    );
    assert_eq!(world.timeline.position_since(mark, step::RECORD), Some(1));
}

#[test]
fn a_stalled_check_fails_after_the_operation_timeout_and_a_stalled_download_outlasts_any_bound() {
    let rt = runtime();
    let bucket = FakeIndexBucket::default();
    let dir = tempfile::tempdir().expect("scratch");
    let src = dir.path().join("index.duckdb");
    std::fs::write(&src, b"v1 and some more").expect("v1");
    rt.block_on(bucket.publish_if_match(&src, WriteCondition::IfNoneMatch, metadata("p1"), LIMIT))
        .expect("publish");

    bucket.has_trouble(ObjectTrouble::HeadStalls);
    assert!(matches!(
        rt.block_on(bucket.head()),
        Err(IndexObjectError::Unavailable { .. })
    ));

    bucket.has_trouble(ObjectTrouble::DownloadStalls);
    let dest = dir.path().join("copy.duckdb");
    let stalled = rt.block_on(async {
        tokio::time::timeout(std::time::Duration::from_millis(50), bucket.fetch_to(&dest)).await
    });
    assert!(stalled.is_err(), "still stalled at the limit");
    assert_eq!(std::fs::read(&dest).expect("half").len(), 8, "half landed");
}

#[test]
fn each_row_reports_every_failing_row_not_only_the_first() {
    let report = std::panic::catch_unwind(|| {
        each_row([1, 2, 3], |row| assert_eq!(*row, 2, "row {row}"));
    })
    .expect_err("two rows fail");
    let text = report.downcast_ref::<String>().expect("a message");
    assert!(text.contains("2 row(s) failed"), "{text}");
    assert!(text.contains("row 1") && text.contains("row 3"), "{text}");

    each_row([2, 2], |row| assert_eq!(*row, 2));
}

#[test]
fn a_function_url_response_reads_back_as_a_reply() {
    let plain = Reply::from_function_url_response(&json!({
        "statusCode": 503,
        "headers": {"Retry-After": "1"},
        "body": r#"{"results":[{"author_did":"did:plc:priyaraman7x2k#org.openlore.application","subject":"s"}]}"#,
    }));
    assert_eq!(plain.status, 503);
    assert_eq!(
        plain.headers.get("retry-after").map(String::as_str),
        Some("1")
    );
    assert_eq!(plain.subjects_by(Author::Priya), vec!["s".to_string()]);

    let encoded = Reply::from_function_url_response(&json!({
        "statusCode": 200, "isBase64Encoded": true, "body": "e30=",
    }));
    assert_eq!(encoded.text(), "{}");
}

/// The CID of Priya's self-attested cargo-pin claim (L-13). A pass recomputes the
/// CID and rejects a record keyed by anything else, so a harness that drifted
/// here would look like a pass that indexes nothing.
const KNOWN_CID: &str = "bafyreidvsg2jhgnuaar25mbs7pnnko3jxjkyyt4eimltn6mxgdto2oomua";

#[test]
fn the_world_seeds_records_keyed_by_their_cid() {
    let world = LambdaWorld::listing(&[Author::Priya]);
    let keys = world.publishes(Author::Priya, &[claims::CARGO_PIN_REPRODUCIBLE_BUILDS]);
    assert_eq!(keys, [KNOWN_CID], "the claim's CID has not moved");
}

/// Not a scenario: the positive control of every "PDS down" scenario, on each host the
/// cast lives on. A real pass reaches the author, then cannot. `Author::Ghost` has no PDS
/// to take down: `pds_goes_down` leaves nothing to reach there, and a pass of that author
/// is skipped with or without it.
#[test]
fn a_pds_that_goes_down_stops_a_real_pass_reaching_its_author() {
    each_row([Author::Priya, Author::Dmitri, Author::Tomas], |author| {
        let world = LambdaWorld::listing(&[*author]);
        world.publishes(*author, &[claims::CARGO_PIN_REPRODUCIBLE_BUILDS]);

        let reached = world.cold_pass_instance().schedule_fires();

        reached.assert_exit(Exit::Completed);
        assert!(reached.published(), "{}", reached.dump());
        world.pds_goes_down(*author);

        let cut_off = world.cold_pass_instance().schedule_fires();

        cut_off.assert_exit(Exit::AllSkipped);
    });
}

/// Not a scenario: the positive control of SE-E6's `/tmp` bound. A failed reload is only
/// seen by that bound if what it leaves is as large as the index, which is why a damaged
/// index is. So the copies that three failed reloads would leak, if one outlived its
/// failure, are fetched from the fake beside the running instance's own copy.
#[test]
fn a_leaked_copy_of_a_damaged_index_breaks_the_scratch_bound() {
    let world = LambdaWorld::listing(&[Author::Priya]);
    world.publishes(Author::Priya, &[claims::CARGO_PIN_REPRODUCIBLE_BUILDS]);
    world.given_a_pass_published_the_index();
    let search = world.cold_search_instance();
    let served = world.bucket.current().map_or(0, |v| v.bytes.len() as u64);
    world.bucket.holds_a_damaged_index();
    search.assert_scratch_within_two_sizes_of(served);

    let rt = runtime();
    for attempt in 1..=3 {
        let leaked = search.scratch.join(format!("index.duckdb.{attempt}"));
        rt.block_on(world.bucket.fetch_to(&leaked)).expect("a copy");
    }

    let bound = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        search.assert_scratch_within_two_sizes_of(served);
    }));
    assert!(bound.is_err(), "three leaked copies fit the bound");
}

#[test]
fn the_pass_universe_names_only_what_the_ports_expose() {
    let world = LambdaWorld::listing(&[Author::Priya]);
    let universe = world.pass_universe();
    assert_eq!(universe.len(), 11);
    assert_eq!(universe["index.version_count"], 0);
    assert!(universe["last_pass.exit_code"].is_null());
    assert!(universe["last_pass.index_published_at"].is_null());
    assert_eq!(universe["write_check_marker.versions"], 0);
    assert_eq!(world.pass_universe_names().len(), 11);
    assert_eq!(world.bucket_universe_names().len(), 8);
}
