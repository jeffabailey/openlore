//! scale-to-zero-hosting — checks of the acceptance harness itself.
//!
//! Every RED scenario stops at the first scaffold panic, so nothing after that
//! panic has ever run. These tests run the doubles and the fixture builders the
//! scenarios lean on, so a harness bug cannot wait for DELIVER and look like a
//! product bug. `FakeIndexBucket`'s contract is the one RIO-1..3 hold the real
//! S3 adapter to. They run green at hand-off; they are not scenarios.
#![cfg(unix)]

#[path = "../common/state_delta.rs"]
mod state_delta;

#[path = "support/indexer_lambda.rs"]
mod indexer_lambda;

use indexer_lambda::*;
use ports::{
    HeadOutcome, IndexObjectError, IndexObjectReadPort, IndexObjectWritePort, LastPass,
    ProbeOutcome, PublishMetadata, PublishOutcome, RepoDidListSourcePort, WriteCondition,
};
use serde_json::json;

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
fn the_fake_bucket_keeps_the_conditional_write_contract() {
    let rt = runtime();
    let bucket = FakeIndexBucket::default();
    let dir = tempfile::tempdir().expect("scratch");
    let src = dir.path().join("index.duckdb");
    std::fs::write(&src, b"v1").expect("v1");

    assert_eq!(
        rt.block_on(bucket.head()).expect("head"),
        HeadOutcome::Absent
    );
    assert_eq!(
        rt.block_on(bucket.fetch_to(&dir.path().join("x")))
            .expect("get"),
        None
    );

    let first = rt
        .block_on(bucket.publish_if_match(&src, WriteCondition::IfNoneMatch, metadata("p1")))
        .expect("publish");
    let PublishOutcome::Published(v1) = first else {
        panic!("creates the object: {first:?}")
    };
    let created_again =
        rt.block_on(bucket.publish_if_match(&src, WriteCondition::IfNoneMatch, metadata("p2")));
    assert_eq!(created_again, Ok(PublishOutcome::Conflict));
    let stale = rt.block_on(bucket.publish_if_match(
        &src,
        WriteCondition::IfMatch("\"stale\"".to_string()),
        metadata("p3"),
    ));
    assert_eq!(stale, Ok(PublishOutcome::Conflict));

    bucket.another_writer_publishes_during_the_next_pass();
    let lost = rt.block_on(bucket.publish_if_match(
        &src,
        WriteCondition::IfMatch(v1.etag.clone()),
        metadata("p4"),
    ));
    assert_eq!(lost, Ok(PublishOutcome::Conflict));
    assert_eq!(
        bucket.current().and_then(|v| v.pass_id).as_deref(),
        Some("another-writer")
    );

    bucket.operator_resets_the_index();
    assert_eq!(
        rt.block_on(bucket.head()).expect("head"),
        HeadOutcome::Absent
    );
    assert_eq!(bucket.versions().len(), 2, "a reset leaves the versions");
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
        rt.block_on(bucket.publish_if_match(&src, WriteCondition::IfNoneMatch, metadata("p1")))
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
        rt.block_on(bucket.publish_if_match(&src, WriteCondition::IfMatch(etag), metadata("p2"))),
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
fn the_fake_did_list_answers_what_the_operator_saved_and_counts_reads() {
    let rt = runtime();
    let list = FakeDidList::default();
    list.saves("did:plc:a");
    assert_eq!(rt.block_on(list.current()).as_deref(), Ok("did:plc:a"));
    list.fails(true);
    assert!(rt.block_on(list.current()).is_err());
    assert_eq!(list.reads(), 2);
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
        .block_on(writes.publish_if_match(&src, WriteCondition::IfNoneMatch, metadata("p")))
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

    rt.block_on(
        world
            .bucket
            .publish_if_match(&src, WriteCondition::IfNoneMatch, metadata("p")),
    )
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
    rt.block_on(bucket.publish_if_match(&src, WriteCondition::IfNoneMatch, metadata("p1")))
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
fn a_damaged_index_is_as_large_as_the_one_it_replaced() {
    let rt = runtime();
    let bucket = FakeIndexBucket::default();
    let dir = tempfile::tempdir().expect("scratch");
    let src = dir.path().join("index.duckdb");
    std::fs::write(&src, vec![b'i'; 4096]).expect("v1");
    rt.block_on(bucket.publish_if_match(&src, WriteCondition::IfNoneMatch, metadata("p1")))
        .expect("publish");

    bucket.holds_a_damaged_index();

    assert_eq!(bucket.current().map(|v| v.bytes.len()), Some(4096));
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

#[test]
fn the_canned_edge_answers_health_and_search_with_its_statuses() {
    let edge = CannedEdge::answering(503, 200);
    let get = |path: &str, method: &str| {
        let mut stream =
            std::net::TcpStream::connect(edge.url().trim_start_matches("http://")).expect("edge");
        let request = format!("{method} {path} HTTP/1.1\r\nhost: x\r\ncontent-length: 0\r\n\r\n");
        std::io::Write::write_all(&mut stream, request.as_bytes()).expect("send");
        let mut answer = String::new();
        std::io::Read::read_to_string(&mut stream, &mut answer).expect("read");
        answer
    };
    assert!(get(SEARCH_PATH, "POST").starts_with("HTTP/1.1 200"));
    assert!(get(HEALTH_PATH, "GET").starts_with("HTTP/1.1 503"));
    assert_eq!(edge.asked(), [SEARCH_PATH, HEALTH_PATH], "in order");

    let serving_nothing = CannedEdge::serving_an_index_published_at(None);
    let mut stream =
        std::net::TcpStream::connect(serving_nothing.url().trim_start_matches("http://"))
            .expect("edge");
    std::io::Write::write_all(
        &mut stream,
        b"GET /healthz HTTP/1.1\r\nhost: x\r\ncontent-length: 0\r\n\r\n",
    )
    .expect("send");
    let mut answer = String::new();
    std::io::Read::read_to_string(&mut stream, &mut answer).expect("read");
    assert!(
        answer.contains(r#""last_successful_pass_at":null"#),
        "{answer}"
    );
}

/// The CID of Priya's self-attested cargo-pin claim (L-13). A pass recomputes the
/// CID and rejects a record keyed by anything else, so a harness that drifted
/// here would look like a pass that indexes nothing.
const KNOWN_CID: &str = "bafyreidvsg2jhgnuaar25mbs7pnnko3jxjkyyt4eimltn6mxgdto2oomua";

#[test]
fn the_world_seeds_records_keyed_by_their_cid_and_reaches_every_pds() {
    let world = LambdaWorld::listing(&[Author::Priya]);
    let keys = world.publishes(Author::Priya, &[claims::CARGO_PIN_REPRODUCIBLE_BUILDS]);
    assert_eq!(keys, [KNOWN_CID], "the claim's CID has not moved");
    world.pds_goes_down(Author::Priya);
    world.pds_goes_down(Author::Dmitri);
    world.pds_goes_down(Author::Ghost);
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
