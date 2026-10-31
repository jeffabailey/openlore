//! scale-to-zero-hosting — the REAL S3/SSM adapter against moto (Mandate 6:
//! every driven adapter has one real-I/O scenario; DEVOPS UD-5 chose moto).
//!
//! Registered as a `[[test]]` of `crates/adapter-s3-index-object`. Layer 3
//! (real HTTP to an S3/SSM stand-in), example-only (Mandate 9/11).
//!
//! `@requires_external`: needs a moto server with S3 conditional writes
//! (`If-Match` / `If-None-Match` on PutObject), which 5.2.5 has. Run it with
//!
//! ```text
//! docker run --rm -p 5055:5000 motoserver/moto:5.2.5
//! OPENLORE_TEST_MOTO_ENDPOINT=http://127.0.0.1:5055 \
//!   cargo nextest run -p adapter-s3-index-object --run-ignored only
//! ```
//!
//! Without the variable every test here panics, naming it, so a run that never
//! reached the store cannot pass. CI runs this suite in the blocking
//! `rio-object-store` job (D-STZ-47). RIO-9 is the exception: its store is a
//! silent socket, so it needs no moto.
//!
//! moto routes a request by the signing service in its `Authorization` header.
//! Moto ignores the signature but needs the credential scope: an unsigned POST
//! goes to its S3 backend, which answers 200 with ListBuckets and creates
//! nothing. So the seeds below send a scope, and each one reads back what it set.
#![cfg(unix)]

use std::net::TcpListener;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use adapter_s3_index_object::{
    S3IndexObjectAdapter, S3Location, SsmLocation, SsmRepoDidListAdapter, WRITE_PROBE_KEY,
};
use ports::{
    HeadOutcome, IndexObjectError, IndexObjectReadPort, IndexObjectWritePort, LastPass,
    ProbeOutcome, PublishMetadata, PublishOutcome, RepoDidListSourcePort, WriteCondition,
};

const ENDPOINT_VAR: &str = "OPENLORE_TEST_MOTO_ENDPOINT";

/// What a publish is given: moto answers at once, so this is never the limit.
const LIMIT: Duration = Duration::from_secs(35);

fn moto() -> String {
    match std::env::var(ENDPOINT_VAR) {
        Ok(url) if !url.trim().is_empty() => url.trim_end_matches('/').to_string(),
        _ => panic!("not run: set {ENDPOINT_VAR} to a moto server URL (moto==5.2.5)"),
    }
}

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("a test runtime")
}

fn unique(prefix: &str) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    format!("{prefix}-{nanos}")
}

/// Static test credentials for moto (it does not check signatures).
fn moto_credentials() {
    for (k, v) in [
        ("AWS_ACCESS_KEY_ID", "testing"),
        ("AWS_SECRET_ACCESS_KEY", "testing"),
        ("AWS_REGION", "us-east-1"),
    ] {
        if std::env::var(k).is_err() {
            std::env::set_var(k, v);
        }
    }
}

/// The credential scope moto routes by (see the module docs).
fn signed_for(service: &str) -> String {
    format!(
        "AWS4-HMAC-SHA256 Credential=testing/20261030/us-east-1/{service}/aws4_request, \
         SignedHeaders=host, Signature=0"
    )
}

fn location_of(endpoint: &str, bucket: String) -> S3Location {
    S3Location {
        region: "us-east-1".to_string(),
        endpoint_url: Some(endpoint.to_string()),
        bucket,
        index_key: "index/index.duckdb".to_string(),
        last_pass_key: "index/last-pass.json".to_string(),
    }
}

/// GIVEN an empty versioned bucket on moto (created over plain REST).
fn given_an_empty_versioned_bucket(endpoint: &str) -> S3Location {
    let bucket = unique("openlore-index-test");
    let http = reqwest::blocking::Client::new();
    let created = http
        .put(format!("{endpoint}/{bucket}"))
        .header("Authorization", signed_for("s3"))
        .send()
        .expect("create bucket");
    assert!(
        created.status().is_success(),
        "create bucket: {}",
        created.status()
    );
    let versioning = r#"<VersioningConfiguration xmlns="http://s3.amazonaws.com/doc/2006-03-01/"><Status>Enabled</Status></VersioningConfiguration>"#;
    let enabled = http
        .put(format!("{endpoint}/{bucket}?versioning"))
        .header("Authorization", signed_for("s3"))
        .body(versioning)
        .send()
        .expect("enable versioning");
    assert!(
        enabled.status().is_success(),
        "versioning: {}",
        enabled.status()
    );
    let status = http
        .get(format!("{endpoint}/{bucket}?versioning"))
        .header("Authorization", signed_for("s3"))
        .send()
        .and_then(|r| r.text())
        .expect("read versioning");
    assert!(status.contains("Enabled"), "versioning: {status}");
    location_of(endpoint, bucket)
}

/// GIVEN the DID list parameter holds `text` on moto (SSM JSON protocol).
fn given_the_did_list_parameter(endpoint: &str, text: &str) -> SsmLocation {
    let parameter = format!("/openlore/test/{}/repo-dids", unique("p"));
    let body = serde_json_text(&parameter, text);
    let saved = reqwest::blocking::Client::new()
        .post(format!("{endpoint}/"))
        .header("X-Amz-Target", "AmazonSSM.PutParameter")
        .header("Content-Type", "application/x-amz-json-1.1")
        .header("Authorization", signed_for("ssm"))
        .body(body)
        .send()
        .expect("put parameter");
    assert!(
        saved.status().is_success(),
        "put parameter: {}",
        saved.status()
    );
    let answer = saved.text().expect("put parameter answer");
    assert!(
        answer.contains("\"Version\""),
        "the parameter store took it: {answer}"
    );
    SsmLocation {
        region: "us-east-1".to_string(),
        endpoint_url: Some(endpoint.to_string()),
        parameter,
    }
}

/// How many versions of `key` the bucket holds, and whether the newest is empty.
fn versions_of(endpoint: &str, location: &S3Location, key: &str) -> (usize, bool) {
    let listing = reqwest::blocking::Client::new()
        .get(format!(
            "{endpoint}/{}?versions&prefix={key}",
            location.bucket
        ))
        .header("Authorization", signed_for("s3"))
        .send()
        .and_then(|r| r.text())
        .expect("list versions");
    (
        listing.matches("<Version>").count(),
        listing.contains("<Size>0</Size>"),
    )
}

fn serde_json_text(name: &str, value: &str) -> String {
    let escape = |s: &str| {
        s.replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\n', "\\n")
    };
    format!(
        r#"{{"Name":"{}","Value":"{}","Type":"String","Overwrite":true}}"#,
        escape(name),
        escape(value)
    )
}

fn metadata(pass_id: &str) -> PublishMetadata {
    PublishMetadata {
        published_at: "2026-10-27T14:00:02Z".to_string(),
        pass_id: pass_id.to_string(),
    }
}

/// RIO-1
/// ```gherkin
/// @real-io @adapter-integration @requires_external @D-STZ-5 @D-STZ-6 @D-STZ-12 @D-STZ-13
/// @contract-shape:bounded-change
/// Scenario: The index object round-trips through a real object store, and a lost update is refused
///   Given an empty versioned bucket, where a check finds no index and a download finds nothing
///   When the first pass publishes an index created only if none exists
///   Then a check finds that version, stamped with its publish time and pass id
///   And a download writes exactly the published bytes
///   When a second publish names a version that is no longer current
///   Then it is refused as a conflict and the first version stays current
///   And a second create-only publish is refused as a conflict too
/// ```
#[test]
#[ignore = "requires_external: moto"]
fn the_index_object_round_trips_through_a_real_object_store_and_a_lost_update_is_refused() {
    let endpoint = moto();
    moto_credentials();
    let location = given_an_empty_versioned_bucket(&endpoint);
    let rt = runtime();
    let store = rt
        .block_on(S3IndexObjectAdapter::connect(location))
        .expect("connect");
    let dir = tempfile::tempdir().expect("scratch");
    let published = dir.path().join("index.duckdb");
    std::fs::write(&published, b"index bytes v1").expect("a file to publish");

    assert_eq!(
        rt.block_on(store.head()).expect("head"),
        HeadOutcome::Absent
    );
    assert_eq!(
        rt.block_on(store.fetch_to(&dir.path().join("none.duckdb")))
            .expect("get"),
        None
    );

    let first = rt
        .block_on(store.publish_if_match(
            &published,
            WriteCondition::IfNoneMatch,
            metadata("pass-1"),
            LIMIT,
        ))
        .expect("publish");
    let PublishOutcome::Published(v1) = first else {
        panic!("the first publish creates the object: {first:?}")
    };
    let HeadOutcome::Present(seen) = rt.block_on(store.head()).expect("head") else {
        panic!("the object exists")
    };
    assert_eq!(seen.etag, v1.etag);
    assert_eq!(seen.published_at.as_deref(), Some("2026-10-27T14:00:02Z"));
    assert_eq!(seen.pass_id.as_deref(), Some("pass-1"));
    assert_eq!(seen.size_bytes, 14);
    let copy = dir.path().join("copy.duckdb");
    let fetched = rt
        .block_on(store.fetch_to(&copy))
        .expect("get")
        .expect("an object");
    assert_eq!(fetched.etag, v1.etag);
    assert_eq!(std::fs::read(&copy).expect("downloaded"), b"index bytes v1");

    std::fs::write(&published, b"index bytes v2").expect("v2");
    let stale = rt
        .block_on(store.publish_if_match(
            &published,
            WriteCondition::IfMatch("\"not-the-current-etag\"".to_string()),
            metadata("pass-2"),
            LIMIT,
        ))
        .expect("conditional publish answers");
    assert_eq!(stale, PublishOutcome::Conflict);
    let again = rt
        .block_on(store.publish_if_match(
            &published,
            WriteCondition::IfNoneMatch,
            metadata("pass-3"),
            LIMIT,
        ))
        .expect("create-only publish answers");
    assert_eq!(again, PublishOutcome::Conflict);
    let HeadOutcome::Present(still) = rt.block_on(store.head()).expect("head") else {
        panic!("the object exists")
    };
    assert_eq!(still.etag, v1.etag, "no lost update");

    let replaced = rt
        .block_on(store.publish_if_match(
            &published,
            WriteCondition::IfMatch(v1.etag.clone()),
            metadata("pass-4"),
            LIMIT,
        ))
        .expect("publish");
    assert!(
        matches!(replaced, PublishOutcome::Published(ref v) if v.etag != v1.etag),
        "{replaced:?}"
    );
}

/// RIO-2
/// ```gherkin
/// @real-io @adapter-integration @requires_external @U-STZ-3 @contract-shape:bounded-change
/// Scenario: The pass record round-trips through a real object store
///   Given an empty versioned bucket with no pass record yet
///   When a pass records exit 2, not published, naming the index it saw
///   Then reading it back gives exactly that record
///   And a record naming no index reads back the same way
/// ```
#[test]
#[ignore = "requires_external: moto"]
fn the_pass_record_round_trips_through_a_real_object_store() {
    let endpoint = moto();
    moto_credentials();
    let location = given_an_empty_versioned_bucket(&endpoint);
    let rt = runtime();
    let store = rt
        .block_on(S3IndexObjectAdapter::connect(location))
        .expect("connect");
    let record = LastPass {
        pass_id: "pass-7".to_string(),
        exit_code: 2,
        finished_at: "2026-10-27T14:15:01Z".to_string(),
        published: false,
        index_published_at: Some("2026-10-27T14:00:02Z".to_string()),
    };
    let no_index = LastPass {
        pass_id: "pass-8".to_string(),
        index_published_at: None,
        ..record.clone()
    };

    assert_eq!(rt.block_on(store.get_last_pass()).expect("get"), None);
    for written in [record, no_index] {
        rt.block_on(store.put_last_pass(&written)).expect("put");

        assert_eq!(
            rt.block_on(store.get_last_pass()).expect("get"),
            Some(written)
        );
    }
}

/// RIO-3
/// ```gherkin
/// @real-io @adapter-integration @requires_external @error @D-STZ-14 @C6b @contract-shape:unbounded-preservation
/// Scenario: A bucket that does not exist is a store failure, never "no index yet"
///   Given the configured bucket does not exist
///   When the search instance checks the index
///   Then the check fails, rather than reporting that no index was published
/// ```
#[test]
#[ignore = "requires_external: moto"]
fn a_bucket_that_does_not_exist_is_a_store_failure_never_no_index_yet() {
    let endpoint = moto();
    moto_credentials();
    let rt = runtime();
    let missing = location_of(&endpoint, unique("openlore-index-missing"));
    let store = rt
        .block_on(S3IndexObjectAdapter::connect(missing))
        .expect("connect");

    let checked = rt.block_on(store.head());

    assert!(
        matches!(
            checked,
            Err(IndexObjectError::Refused { .. } | IndexObjectError::Unavailable { .. })
        ),
        "{checked:?}"
    );
}

/// RIO-4
/// ```gherkin
/// @real-io @adapter-integration @requires_external @D-STZ-16 @KPI-IXD-4 @kpi @contract-shape:unbounded-preservation
/// Scenario: The DID list is read verbatim from a real parameter store, and a missing list is a failure
///   Given the DID list parameter names Priya and Dmitri on two lines
///   When the pass reads it
///   Then it gets exactly the saved text
///   And reading a parameter that does not exist fails
/// ```
#[test]
#[ignore = "requires_external: moto"]
fn the_did_list_is_read_verbatim_from_a_real_parameter_store() {
    let endpoint = moto();
    moto_credentials();
    let text = "did:plc:priyaraman7x2k\ndid:plc:dvolkov3m9q";
    let location = given_the_did_list_parameter(&endpoint, text);
    let rt = runtime();
    let source = rt
        .block_on(SsmRepoDidListAdapter::connect(location.clone()))
        .expect("connect");

    assert_eq!(rt.block_on(source.current()).expect("read"), text);

    let missing = SsmLocation {
        parameter: format!("{}-missing", location.parameter),
        ..location
    };
    let absent = rt
        .block_on(SsmRepoDidListAdapter::connect(missing))
        .expect("connect");
    assert!(rt.block_on(absent.current()).is_err());
}

/// RIO-5
/// ```gherkin
/// @real-io @adapter-integration @requires_external @error @ADR-009 @D-STZ-14 @C6b
/// @contract-shape:unbounded-preservation
/// Scenario: The start-up probes tell a reachable store from a missing one
///   Given a real bucket and a real DID list parameter
///   Then both start-up probes succeed
///   And the probes of a missing bucket and a missing parameter are refused
/// ```
#[test]
#[ignore = "requires_external: moto"]
fn the_start_up_probes_tell_a_reachable_store_from_a_missing_one() {
    let endpoint = moto();
    moto_credentials();
    let rt = runtime();
    let bucket = given_an_empty_versioned_bucket(&endpoint);
    let list = given_the_did_list_parameter(&endpoint, "did:plc:priyaraman7x2k");
    let missing_bucket = S3Location {
        bucket: unique("openlore-index-missing"),
        ..bucket.clone()
    };
    let missing_list = SsmLocation {
        parameter: format!("{}-missing", list.parameter),
        ..list.clone()
    };

    let store = rt
        .block_on(S3IndexObjectAdapter::connect(bucket))
        .expect("connect");
    let source = rt
        .block_on(SsmRepoDidListAdapter::connect(list))
        .expect("connect");
    let no_store = rt
        .block_on(S3IndexObjectAdapter::connect(missing_bucket))
        .expect("connect");
    let no_source = rt
        .block_on(SsmRepoDidListAdapter::connect(missing_list))
        .expect("connect");

    assert!(matches!(
        IndexObjectReadPort::probe(&store),
        ProbeOutcome::Ok
    ));
    assert!(matches!(source.probe(), ProbeOutcome::Ok));
    assert!(matches!(
        IndexObjectReadPort::probe(&no_store),
        ProbeOutcome::Refused { .. }
    ));
    assert!(matches!(no_source.probe(), ProbeOutcome::Refused { .. }));
}

/// RIO-6
/// ```gherkin
/// @real-io @adapter-integration @requires_external @D-STZ-39 @C4a @contract-shape:bounded-change
/// Scenario: The write check creates one empty marker and never touches the index
///   Given an empty versioned bucket
///   When the write check runs twice
///   Then both runs pass
///   And the bucket holds one empty version of the marker
///   And there is still no index
/// ```
#[test]
#[ignore = "requires_external: moto"]
fn the_write_check_creates_one_empty_marker_and_never_touches_the_index() {
    let endpoint = moto();
    moto_credentials();
    let location = given_an_empty_versioned_bucket(&endpoint);
    let rt = runtime();
    let store = rt
        .block_on(S3IndexObjectAdapter::connect(location.clone()))
        .expect("connect");

    let first = IndexObjectWritePort::probe(&store);
    let second = IndexObjectWritePort::probe(&store);

    assert!(matches!(first, ProbeOutcome::Ok), "{first:?}");
    assert!(matches!(second, ProbeOutcome::Ok), "{second:?}");
    assert_eq!(
        versions_of(&endpoint, &location, WRITE_PROBE_KEY),
        (1, true)
    );
    assert_eq!(
        rt.block_on(store.head()).expect("head"),
        HeadOutcome::Absent
    );
}

/// RIO-7
/// ```gherkin
/// @real-io @adapter-integration @requires_external @error @D-STZ-41 @C6b @contract-shape:unbounded-preservation
/// Scenario: A write the store cannot take is a failure, never a lost update
///   Given the configured bucket does not exist
///   When the pass publishes, and then records itself
///   Then both fail, and the publish is not reported as a conflict
/// ```
#[test]
#[ignore = "requires_external: moto"]
fn a_write_the_store_cannot_take_is_a_failure_never_a_lost_update() {
    let endpoint = moto();
    moto_credentials();
    let rt = runtime();
    let missing = location_of(&endpoint, unique("openlore-index-missing"));
    let store = rt
        .block_on(S3IndexObjectAdapter::connect(missing))
        .expect("connect");
    let dir = tempfile::tempdir().expect("scratch");
    let published = dir.path().join("index.duckdb");
    std::fs::write(&published, b"index bytes").expect("a file to publish");
    let record = LastPass {
        pass_id: "pass-9".to_string(),
        exit_code: 0,
        finished_at: "2026-10-27T14:15:01Z".to_string(),
        published: false,
        index_published_at: None,
    };

    let publish = rt.block_on(store.publish_if_match(
        &published,
        WriteCondition::IfNoneMatch,
        metadata("p"),
        LIMIT,
    ));
    let recorded = rt.block_on(store.put_last_pass(&record));

    assert!(publish.is_err(), "{publish:?}");
    assert!(recorded.is_err(), "{recorded:?}");
}

/// RIO-8
/// ```gherkin
/// @real-io @adapter-integration @requires_external @error @UI-12 @D-STZ-23 @C6a @contract-shape:unbounded-preservation
/// Scenario: A pass record nobody can read is an error, never "no pass yet"
///   Given the pass record holds something that is not a record
///   When the probe reads it
///   Then the read fails, rather than reporting that no pass ran
/// ```
#[test]
#[ignore = "requires_external: moto"]
fn a_pass_record_nobody_can_read_is_an_error_never_no_pass_yet() {
    let endpoint = moto();
    moto_credentials();
    let location = given_an_empty_versioned_bucket(&endpoint);
    let written = reqwest::blocking::Client::new()
        .put(format!(
            "{endpoint}/{}/{}",
            location.bucket, location.last_pass_key
        ))
        .header("Authorization", signed_for("s3"))
        .body("not a pass record")
        .send()
        .expect("a garbled record");
    assert!(written.status().is_success(), "{}", written.status());
    let rt = runtime();
    let store = rt
        .block_on(S3IndexObjectAdapter::connect(location))
        .expect("connect");

    let read = rt.block_on(store.get_last_pass());

    assert!(read.is_err(), "{read:?}");
}

/// RIO-9 (D-STZ-39: 2 s for each PUT, so an outage cannot use up Lambda's 10 s init
/// limit. A silent first PUT is unknown, so no second PUT follows.)
/// ```gherkin
/// @real-io @adapter-integration @error @D-STZ-39 @D-STZ-41 @C7b @contract-shape:unbounded-preservation
/// Scenario: A store that accepts the connection and never answers does not stop a pass starting
///   Given a store that accepts every connection and never answers
///   When the write check runs
///   Then it reached the store
///   And it passes, because an outage says nothing about conditions
///   And it comes back inside its 2 seconds, plus a margin
/// ```
#[test]
#[ignore = "real-I/O: runs with RIO-1..8 in the rio-object-store job"]
fn a_store_that_accepts_the_connection_and_never_answers_does_not_stop_a_pass_starting() {
    moto_credentials();
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind the silent store");
    let endpoint = format!("http://{}", listener.local_addr().expect("address"));
    let accepted = Arc::new(AtomicUsize::new(0));
    let counted = accepted.clone();
    std::thread::spawn(move || {
        let mut held = Vec::new();
        for stream in listener.incoming().flatten() {
            counted.fetch_add(1, Ordering::SeqCst);
            held.push(stream);
        }
    });
    let rt = runtime();
    let store = rt
        .block_on(S3IndexObjectAdapter::connect(location_of(
            &endpoint,
            unique("openlore-index-silent"),
        )))
        .expect("connect");
    let started = Instant::now();
    // On its own thread, so a probe that really hangs fails at the limit.
    let (answer, answered) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = answer.send(IndexObjectWritePort::probe(&store));
    });

    let outcome = answered
        .recv_timeout(Duration::from_secs(10))
        .expect("the write check came back");

    let took = started.elapsed();
    assert!(accepted.load(Ordering::SeqCst) >= 1, "it reached the store");
    assert!(matches!(outcome, ProbeOutcome::Ok), "{outcome:?}");
    assert!(took < Duration::from_secs(3), "{took:?}");
}

/// Not a scenario: the seeds themselves, against moto. Green once moto runs, with
/// the adapter still a scaffold, so a broken seed cannot look like a broken
/// adapter in DELIVER.
#[test]
#[ignore = "requires_external: moto"]
fn the_seeds_really_reach_the_moto_store() {
    let endpoint = moto();
    let list = given_the_did_list_parameter(&endpoint, "did:plc:priyaraman7x2k");
    let read = reqwest::blocking::Client::new()
        .post(format!("{endpoint}/"))
        .header("X-Amz-Target", "AmazonSSM.GetParameter")
        .header("Content-Type", "application/x-amz-json-1.1")
        .header("Authorization", signed_for("ssm"))
        .body(format!(r#"{{"Name":"{}"}}"#, list.parameter))
        .send()
        .and_then(|r| r.text())
        .expect("get parameter");

    assert!(read.contains("did:plc:priyaraman7x2k"), "{read}");
    given_an_empty_versioned_bucket(&endpoint);
}
