//! Crafter-owned (step 06-01): what moto cannot play. A loopback server gives the
//! adapter a trickling body, a short body, a refusal, an outage and a silent store,
//! so these checks need no moto.
//!
//! Layer 3 (real HTTP), example-only: they verify the wiring between the SDK's
//! answers and the pure mapping in `s3.rs`, which holds the properties.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use adapter_s3_index_object::{
    S3IndexObjectAdapter, S3Location, SsmLocation, SsmRepoDidListAdapter, StoreCall,
    WRITE_PROBE_KEY,
};
use ports::{
    HeadOutcome, IndexObjectError, IndexObjectReadPort, IndexObjectWritePort, LastPass,
    ProbeOutcome, ProbeRefusalReason, PublishMetadata, PublishOutcome, RepoDidListSourcePort,
    WriteCondition,
};

const LIMIT: Duration = Duration::from_secs(35);
const TWO_SECONDS: Duration = Duration::from_secs(2);
const THREE_SECONDS: Duration = Duration::from_secs(3);
const FIVE_SECONDS: Duration = Duration::from_secs(5);
/// A budget's overrun: scheduling, and the connection closing.
const MARGIN: Duration = Duration::from_secs(2);

const NO_SUCH_KEY: &str = "<Error><Code>NoSuchKey</Code></Error>";
const NO_SUCH_BUCKET: &str = "<Error><Code>NoSuchBucket</Code></Error>";

/// One request as the store saw it, with the header names lowercased.
struct Seen {
    head: String,
}

impl Seen {
    fn method(&self) -> &str {
        self.head.split(' ').next().unwrap_or_default()
    }

    fn header(&self, name: &str) -> Option<&str> {
        self.head
            .lines()
            .find_map(|line| line.strip_prefix(&format!("{name}: ")))
    }
}

/// Read one request, whatever its framing asks for.
fn read_request(stream: &mut TcpStream) -> Option<Seen> {
    let mut bytes = Vec::new();
    let mut chunk = [0u8; 4096];
    let end = loop {
        if let Some(end) = bytes.windows(4).position(|window| window == b"\r\n\r\n") {
            break end + 4;
        }
        let read = stream.read(&mut chunk).ok()?;
        if read == 0 {
            return None;
        }
        bytes.extend_from_slice(&chunk[..read]);
    };
    let raw = String::from_utf8_lossy(&bytes[..end]).into_owned();
    let head = raw
        .lines()
        .map(|line| match line.split_once(": ") {
            Some((name, value)) => format!("{}: {value}", name.to_lowercase()),
            None => line.to_string(),
        })
        .collect::<Vec<_>>()
        .join("\n");
    let seen = Seen { head };
    if seen.header("expect").is_some() {
        stream.write_all(b"HTTP/1.1 100 Continue\r\n\r\n").ok()?;
    }
    let length: usize = seen
        .header("content-length")
        .and_then(|length| length.parse().ok())
        .unwrap_or(0);
    let mut left = (end + length).saturating_sub(bytes.len());
    while left > 0 {
        let read = stream.read(&mut chunk).ok()?;
        if read == 0 {
            return None;
        }
        left = left.saturating_sub(read);
    }
    Some(seen)
}

fn answer_with(stream: &mut TcpStream, status: &str, headers: &[&str], body: &str) {
    let headers: String = headers.iter().map(|line| format!("{line}\r\n")).collect();
    let answer = format!(
        "HTTP/1.1 {status}\r\n{headers}Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.write_all(answer.as_bytes());
}

/// A store that hands each request to `respond`, one per connection.
struct LoopbackStore {
    endpoint: String,
    requests: Arc<Mutex<Vec<String>>>,
}

impl LoopbackStore {
    fn answering(respond: impl Fn(&Seen, &mut TcpStream) + Send + Sync + 'static) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind the store");
        let endpoint = format!("http://{}", listener.local_addr().expect("address"));
        let requests = Arc::new(Mutex::new(Vec::new()));
        let recorded = requests.clone();
        let respond = Arc::new(respond);
        thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let (respond, recorded) = (respond.clone(), recorded.clone());
                thread::spawn(move || {
                    let mut stream = stream;
                    if let Some(seen) = read_request(&mut stream) {
                        recorded.lock().expect("log").push(seen.head.clone());
                        respond(&seen, &mut stream);
                    }
                });
            }
        });
        Self { endpoint, requests }
    }

    fn requests(&self) -> Vec<Seen> {
        let log = self.requests.lock().expect("log");
        log.iter().map(|head| Seen { head: head.clone() }).collect()
    }

    fn set_credentials() {
        for (key, value) in [
            ("AWS_ACCESS_KEY_ID", "testing"),
            ("AWS_SECRET_ACCESS_KEY", "testing"),
        ] {
            std::env::set_var(key, value);
        }
    }

    async fn did_list(&self) -> SsmRepoDidListAdapter {
        Self::set_credentials();
        SsmRepoDidListAdapter::connect(SsmLocation {
            region: "us-east-1".to_string(),
            endpoint_url: Some(self.endpoint.clone()),
            parameter: "/openlore/test/repo-dids".to_string(),
        })
        .await
        .expect("connect")
    }

    async fn adapter(&self) -> S3IndexObjectAdapter {
        Self::set_credentials();
        S3IndexObjectAdapter::connect(S3Location {
            region: "us-east-1".to_string(),
            endpoint_url: Some(self.endpoint.clone()),
            bucket: "openlore-index-test".to_string(),
            index_key: "index/index.duckdb".to_string(),
            last_pass_key: "index/last-pass.json".to_string(),
        })
        .await
        .expect("connect")
    }
}

/// Headers, then a byte every 200 ms for as long as the client stays.
fn trickle(stream: &mut TcpStream) {
    let head = "HTTP/1.1 200 OK\r\nETag: \"e\"\r\nContent-Length: 100000\r\n\r\n";
    let _ = stream.write_all(head.as_bytes());
    while stream.write_all(b"x").is_ok() {
        let _ = stream.flush();
        thread::sleep(Duration::from_millis(200));
    }
}

fn metadata() -> PublishMetadata {
    PublishMetadata {
        published_at: "2026-10-27T14:00:02Z".to_string(),
        pass_id: "pass-1".to_string(),
    }
}

fn file_holding(dir: &Path, bytes: &[u8]) -> std::path::PathBuf {
    let file = dir.join("index.duckdb");
    std::fs::write(&file, bytes).expect("a file to publish");
    file
}

/// FROM 01-07: 5 bytes a second outlasts neither the operation timeout, which
/// ended at the headers, nor the stall check, so only the fetch's own bound stops
/// this.
#[tokio::test]
async fn a_body_that_trickles_past_the_budget_fails_and_leaves_no_file() {
    let store = LoopbackStore::answering(|_, stream| trickle(stream));
    let adapter = store.adapter().await.fetching_as(StoreCall::Reload);
    let scratch = tempfile::tempdir().expect("scratch");
    let dest = scratch.path().join("copy.duckdb");
    let started = Instant::now();

    let fetched = adapter.fetch_to(&dest).await;

    let took = started.elapsed();
    assert!(
        matches!(fetched, Err(IndexObjectError::Unavailable { .. })),
        "{fetched:?}"
    );
    assert!(took < THREE_SECONDS + MARGIN, "{took:?}");
    assert!(took >= THREE_SECONDS - MARGIN, "{took:?}");
    assert!(!dest.exists());
}

/// FROM 05-02: the probe's 23 s worst case depends on this.
#[tokio::test]
async fn a_pass_record_that_trickles_past_three_seconds_is_an_error() {
    let store = LoopbackStore::answering(|_, stream| trickle(stream));
    let adapter = store.adapter().await;
    let started = Instant::now();

    let read = adapter.get_last_pass().await;

    assert!(read.is_err(), "{read:?}");
    assert!(
        started.elapsed() < THREE_SECONDS + MARGIN,
        "{:?}",
        started.elapsed()
    );
}

/// FROM 04-08: chunked framing ends cleanly, so HTTP sees nothing wrong with a body
/// shorter than the Content-Length that described the object. The SDK refuses this
/// one before `whole_body` sees it; a property in `s3.rs` holds that check itself.
#[tokio::test]
async fn a_body_shorter_than_the_object_is_a_failed_fetch_and_leaves_no_file() {
    let store = LoopbackStore::answering(|_, stream| {
        let _ = stream.write_all(
            b"HTTP/1.1 200 OK\r\nETag: \"e\"\r\nContent-Length: 10\r\n\
              Transfer-Encoding: chunked\r\nConnection: close\r\n\r\n5\r\nhello\r\n0\r\n\r\n",
        );
    });
    let adapter = store.adapter().await;
    let scratch = tempfile::tempdir().expect("scratch");
    let dest = scratch.path().join("copy.duckdb");

    let fetched = adapter.fetch_to(&dest).await;

    assert!(
        matches!(fetched, Err(IndexObjectError::Unavailable { .. })),
        "{fetched:?}"
    );
    assert!(!dest.exists());
}

/// The connection dropping part way is the other way a body comes up short.
#[tokio::test]
async fn a_body_cut_off_part_way_is_a_failed_fetch_and_leaves_no_file() {
    let store = LoopbackStore::answering(|_, stream| {
        let _ =
            stream.write_all(b"HTTP/1.1 200 OK\r\nETag: \"e\"\r\nContent-Length: 10\r\n\r\nhello");
    });
    let adapter = store.adapter().await;
    let scratch = tempfile::tempdir().expect("scratch");
    let dest = scratch.path().join("copy.duckdb");

    let fetched = adapter.fetch_to(&dest).await;

    assert!(fetched.is_err(), "{fetched:?}");
    assert!(!dest.exists());
}

/// The download goes to `dest` and nowhere else, in pieces of any size.
#[tokio::test]
async fn a_whole_body_lands_at_dest_alone() {
    let store = LoopbackStore::answering(|_, stream| {
        answer_with(
            stream,
            "200 OK",
            &[
                "ETag: \"e\"",
                "x-amz-meta-published-at: stamp",
                "x-amz-meta-pass-id: pass",
            ],
            "whole index",
        );
    });
    let adapter = store.adapter().await;
    let scratch = tempfile::tempdir().expect("scratch");
    let dest = scratch.path().join("copy.duckdb");

    let fetched = adapter.fetch_to(&dest).await.expect("a fetch");

    let version = fetched.expect("an object");
    assert_eq!(version.size_bytes, 11);
    assert_eq!(version.published_at.as_deref(), Some("stamp"));
    assert_eq!(version.pass_id.as_deref(), Some("pass"));
    assert_eq!(std::fs::read(&dest).expect("the copy"), b"whole index");
    let entries = std::fs::read_dir(scratch.path()).expect("scratch").count();
    assert_eq!(entries, 1);
}

#[tokio::test]
async fn a_file_that_cannot_be_written_is_a_local_failure() {
    let store = LoopbackStore::answering(|_, stream| {
        answer_with(stream, "200 OK", &["ETag: \"e\""], "bytes");
    });
    let adapter = store.adapter().await;
    let scratch = tempfile::tempdir().expect("scratch");

    let fetched = adapter
        .fetch_to(&scratch.path().join("no-such-directory").join("copy"))
        .await;

    assert!(
        matches!(fetched, Err(IndexObjectError::LocalWrite { .. })),
        "{fetched:?}"
    );
}

/// `Absent` only when the key is what is missing: a `HEAD` 404 then asks a `GET`.
#[tokio::test]
async fn a_check_finds_no_index_only_when_the_bucket_is_there() {
    for (body, absent) in [(NO_SUCH_KEY, true), (NO_SUCH_BUCKET, false)] {
        let store = LoopbackStore::answering(move |seen, stream| {
            let body = if seen.method() == "HEAD" { "" } else { body };
            answer_with(stream, "404 Not Found", &[], body);
        });
        let adapter = store.adapter().await;

        let checked = adapter.head().await;

        assert_eq!(
            matches!(checked, Ok(HeadOutcome::Absent)),
            absent,
            "{checked:?}"
        );
        let sent: Vec<String> = store
            .requests()
            .iter()
            .map(|r| r.method().to_string())
            .collect();
        assert_eq!(sent, ["HEAD", "GET"]);
    }
}

/// The mutation note's two: neither reads as an empty start, on any read, and
/// one attempt reaches the store.
#[tokio::test]
async fn a_refusal_and_an_outage_are_never_absent() {
    let cases = [
        ("403 Forbidden", "AccessDenied", true),
        ("500 Internal Server Error", "InternalError", false),
    ];
    for (status, code, refused) in cases {
        let body = format!("<Error><Code>{code}</Code></Error>");
        let store = LoopbackStore::answering(move |_, stream| {
            answer_with(stream, status, &[], &body);
        });
        let adapter = store.adapter().await;
        let scratch = tempfile::tempdir().expect("scratch");

        let checked = adapter.head().await;
        let fetched = adapter.fetch_to(&scratch.path().join("copy")).await;
        let record = adapter.get_last_pass().await;

        for error in [checked.err(), fetched.err(), record.err()] {
            let error = error.expect("a failure");
            assert_eq!(
                matches!(error, IndexObjectError::Refused { .. }),
                refused,
                "{error:?}"
            );
            assert!(error.to_string().contains("service error"), "{error:?}");
        }
        assert_eq!(store.requests().len(), 3, "{status}: one attempt each");
    }
}

/// The publish sends its condition and stamps, and reports the size it published,
/// which the pass's `index_bytes` comes from.
#[tokio::test]
async fn a_publish_reports_the_size_it_sent_under_the_condition_it_was_given() {
    let store = LoopbackStore::answering(|_, stream| {
        answer_with(stream, "200 OK", &["ETag: \"new\""], "");
    });
    let adapter = store.adapter().await;
    let scratch = tempfile::tempdir().expect("scratch");
    let src = file_holding(scratch.path(), b"index bytes");

    let first = adapter
        .publish_if_match(&src, WriteCondition::IfNoneMatch, metadata(), LIMIT)
        .await
        .expect("a publish");
    let second = adapter
        .publish_if_match(
            &src,
            WriteCondition::IfMatch("\"old\"".to_string()),
            metadata(),
            LIMIT,
        )
        .await
        .expect("a publish");

    let PublishOutcome::Published(version) = first else {
        panic!("{first:?}")
    };
    assert_eq!(version.size_bytes, 11);
    assert_eq!(version.etag, "\"new\"");
    assert!(matches!(second, PublishOutcome::Published(_)), "{second:?}");
    let sent = store.requests();
    assert_eq!(sent.len(), 2, "one attempt each");
    assert_eq!(sent[0].header("if-none-match"), Some("*"));
    assert_eq!(sent[0].header("if-match"), None);
    assert_eq!(sent[1].header("if-match"), Some("\"old\""));
    assert_eq!(sent[1].header("if-none-match"), None);
    for request in &sent {
        assert_eq!(
            request.header("x-amz-meta-published-at"),
            Some("2026-10-27T14:00:02Z")
        );
        assert_eq!(request.header("x-amz-meta-pass-id"), Some("pass-1"));
    }
}

#[tokio::test]
async fn a_412_is_a_conflict_and_every_other_refusal_a_failure() {
    let cases = [
        ("412 Precondition Failed", "PreconditionFailed", true),
        ("409 Conflict", "ConditionalRequestConflict", false),
        ("403 Forbidden", "AccessDenied", false),
        ("500 Internal Server Error", "InternalError", false),
    ];
    for (status, code, conflict) in cases {
        let body = format!("<Error><Code>{code}</Code></Error>");
        let store = LoopbackStore::answering(move |_, stream| {
            answer_with(stream, status, &[], &body);
        });
        let adapter = store.adapter().await;
        let scratch = tempfile::tempdir().expect("scratch");
        let src = file_holding(scratch.path(), b"index bytes");

        let published = adapter
            .publish_if_match(&src, WriteCondition::IfNoneMatch, metadata(), LIMIT)
            .await;

        assert_eq!(
            published == Ok(PublishOutcome::Conflict),
            conflict,
            "{status}: {published:?}"
        );
        assert_eq!(published.is_err(), !conflict, "{status}: {published:?}");
        assert_eq!(store.requests().len(), 1, "{status}: one attempt");
    }
}

/// D-STZ-40a: the narrowed limit is what holds a silent store, not the 35 s.
#[tokio::test]
async fn a_publish_stops_at_the_limit_it_is_given() {
    let store = LoopbackStore::answering(|_, _| thread::sleep(Duration::from_secs(60)));
    let adapter = store.adapter().await;
    let scratch = tempfile::tempdir().expect("scratch");
    let src = file_holding(scratch.path(), b"index bytes");
    let started = Instant::now();

    let published = adapter
        .publish_if_match(
            &src,
            WriteCondition::IfNoneMatch,
            metadata(),
            Duration::from_secs(1),
        )
        .await;

    assert!(
        matches!(published, Err(IndexObjectError::Unavailable { .. })),
        "{published:?}"
    );
    assert!(started.elapsed() < MARGIN * 2, "{:?}", started.elapsed());
}

/// D-STZ-40a: no time left is `publish_failed` at once, with nothing sent.
#[tokio::test]
async fn a_publish_with_no_time_left_fails_and_sends_nothing() {
    let store = LoopbackStore::answering(|_, stream| {
        answer_with(stream, "200 OK", &["ETag: \"new\""], "");
    });
    let adapter = store.adapter().await;
    let scratch = tempfile::tempdir().expect("scratch");
    let src = file_holding(scratch.path(), b"index bytes");

    let published = adapter
        .publish_if_match(
            &src,
            WriteCondition::IfNoneMatch,
            metadata(),
            Duration::ZERO,
        )
        .await;

    assert!(
        matches!(published, Err(IndexObjectError::Unavailable { .. })),
        "{published:?}"
    );
    assert!(store.requests().is_empty());
}

#[tokio::test]
async fn a_file_that_will_not_read_is_a_local_failure_and_sends_nothing() {
    let store = LoopbackStore::answering(|_, stream| {
        answer_with(stream, "200 OK", &["ETag: \"new\""], "");
    });
    let adapter = store.adapter().await;
    let scratch = tempfile::tempdir().expect("scratch");

    let published = adapter
        .publish_if_match(
            &scratch.path().join("never-written"),
            WriteCondition::IfNoneMatch,
            metadata(),
            LIMIT,
        )
        .await;

    assert!(
        matches!(published, Err(IndexObjectError::LocalWrite { .. })),
        "{published:?}"
    );
    assert!(store.requests().is_empty());
}

#[tokio::test]
async fn a_pass_record_that_is_refused_is_a_failure_and_goes_to_its_own_key() {
    for (status, refused) in [
        ("200 OK", None),
        ("403 Forbidden", Some(true)),
        ("500 Oops", Some(false)),
    ] {
        let store = LoopbackStore::answering(move |_, stream| {
            answer_with(stream, status, &[], "");
        });
        let adapter = store.adapter().await;

        let recorded = adapter
            .put_last_pass(&LastPass {
                pass_id: "pass-1".to_string(),
                exit_code: 0,
                finished_at: "2026-10-27T14:15:01Z".to_string(),
                published: false,
                index_published_at: None,
            })
            .await;

        match refused {
            None => assert_eq!(recorded, Ok(())),
            Some(true) => assert!(matches!(recorded, Err(IndexObjectError::Refused { .. }))),
            Some(false) => assert!(matches!(
                recorded,
                Err(IndexObjectError::Unavailable { .. })
            )),
        }
        let sent = store.requests();
        assert_eq!(sent.len(), 1, "{status}: one attempt");
        assert!(
            sent[0].head.contains("/index/last-pass.json"),
            "{}",
            sent[0].head
        );
    }
}

/// RIO-2 against moto proves the record's round trip, but CI's `test` job has no
/// moto: this holds the key each way, and what the record is sent as.
#[tokio::test]
async fn a_pass_record_is_json_at_its_own_key_both_ways() {
    const STORED: &str = r#"{"pass_id":"pass-1","exit_code":0,"finished_at":"2026-10-27T14:15:01Z","published":false,"index_published_at":null}"#;
    let store = LoopbackStore::answering(|seen, stream| {
        let body = if seen.method() == "GET" { STORED } else { "" };
        answer_with(stream, "200 OK", &[], body);
    });
    let adapter = store.adapter().await;
    let record = LastPass {
        pass_id: "pass-1".to_string(),
        exit_code: 0,
        finished_at: "2026-10-27T14:15:01Z".to_string(),
        published: false,
        index_published_at: None,
    };

    let written = adapter.put_last_pass(&record).await;
    let read = adapter.get_last_pass().await;

    assert_eq!(written, Ok(()));
    assert_eq!(read, Ok(Some(record)));
    let sent = store.requests();
    assert_eq!(sent[0].method(), "PUT");
    assert_eq!(sent[0].header("content-type"), Some("application/json"));
    assert_eq!(sent[1].method(), "GET");
    for request in &sent {
        assert!(
            request.head.contains("/index/last-pass.json"),
            "{}",
            request.head
        );
    }
}

/// A connection that is taken and never answered.
fn hold_silently(stream: &mut TcpStream) {
    let mut held = [0u8; 1];
    let _ = stream.read(&mut held);
}

/// Init's way in: sync, on a blocking thread of the running runtime.
async fn read_probe_as_init_runs_it(adapter: &Arc<S3IndexObjectAdapter>) -> ProbeOutcome {
    let adapter = Arc::clone(adapter);
    tokio::task::spawn_blocking(move || IndexObjectReadPort::probe(adapter.as_ref()))
        .await
        .expect("the probe answered")
}

/// The same HEAD and GET as `head`: an empty store passes, a missing one refuses.
#[tokio::test]
async fn a_read_probe_passes_on_a_reachable_store_and_refuses_a_missing_one() {
    for (body, reachable) in [(NO_SUCH_KEY, true), (NO_SUCH_BUCKET, false)] {
        let store = LoopbackStore::answering(move |seen, stream| {
            let body = if seen.method() == "HEAD" { "" } else { body };
            answer_with(stream, "404 Not Found", &[], body);
        });
        let adapter = Arc::new(store.adapter().await);

        let outcome = read_probe_as_init_runs_it(&adapter).await;

        let refused = matches!(
            outcome,
            ProbeOutcome::Refused {
                reason: ProbeRefusalReason::IndexObjectUnusable,
                ..
            }
        );
        assert_eq!(refused, !reachable, "{outcome:?}");
        assert_eq!(matches!(outcome, ProbeOutcome::Ok), reachable);
        let sent: Vec<String> = store
            .requests()
            .iter()
            .map(|r| r.method().to_string())
            .collect();
        assert_eq!(sent, ["HEAD", "GET"]);
    }
}

#[tokio::test]
async fn a_read_probe_refuses_a_store_that_refuses_it_after_one_attempt() {
    let store = LoopbackStore::answering(|_, stream| {
        answer_with(stream, "403 Forbidden", &[], "");
    });
    let adapter = Arc::new(store.adapter().await);

    let outcome = read_probe_as_init_runs_it(&adapter).await;

    assert!(
        matches!(outcome, ProbeOutcome::Refused { .. }),
        "{outcome:?}"
    );
    assert_eq!(store.requests().len(), 1);
}

/// § Timeouts: the probe has 2 s of its own, not the freshness check's 1 s.
#[tokio::test]
async fn a_silent_store_refuses_a_read_probe_at_its_two_seconds() {
    let store = LoopbackStore::answering(|_, stream| hold_silently(stream));
    let adapter = Arc::new(store.adapter().await);
    let started = Instant::now();

    let outcome = read_probe_as_init_runs_it(&adapter).await;

    let took = started.elapsed();
    assert!(
        matches!(outcome, ProbeOutcome::Refused { .. }),
        "{outcome:?}"
    );
    assert!(took >= TWO_SECONDS - Duration::from_millis(200), "{took:?}");
    assert!(took < TWO_SECONDS + MARGIN, "{took:?}");
}

/// § Timeouts: a warm search waits 1 s for this, so a reload's 3 s is too long. The
/// bound sits between the two, as `MARGIN` would let the 3 s through.
#[tokio::test]
async fn a_silent_store_fails_a_freshness_check_before_a_reloads_budget() {
    let store = LoopbackStore::answering(|_, stream| hold_silently(stream));
    let adapter = store.adapter().await;
    let started = Instant::now();

    let checked = adapter.head().await;

    let took = started.elapsed();
    assert!(
        matches!(checked, Err(IndexObjectError::Unavailable { .. })),
        "{checked:?}"
    );
    assert!(
        took < THREE_SECONDS - Duration::from_millis(500),
        "{took:?}"
    );
}

/// § Timeouts: the record's 5 s is part of what a publish leaves over (D-STZ-40a).
#[tokio::test]
async fn a_silent_store_fails_a_pass_record_write_at_its_five_seconds() {
    let store = LoopbackStore::answering(|_, stream| hold_silently(stream));
    let adapter = store.adapter().await;
    let started = Instant::now();

    let written = adapter
        .put_last_pass(&LastPass {
            pass_id: "pass-1".to_string(),
            exit_code: 0,
            finished_at: "2026-10-27T14:15:01Z".to_string(),
            published: false,
            index_published_at: None,
        })
        .await;

    let took = started.elapsed();
    assert!(
        matches!(written, Err(IndexObjectError::Unavailable { .. })),
        "{written:?}"
    );
    assert!(took < FIVE_SECONDS + MARGIN, "{took:?}");
}

/// § Timeouts: unless composed otherwise a fetch is a pass's, so a store that is
/// slower than search init's 8 s still delivers.
#[tokio::test]
async fn a_fetch_outlasts_search_inits_budget_unless_it_was_composed_for_search() {
    let store = LoopbackStore::answering(|_, stream| {
        thread::sleep(Duration::from_secs(9));
        answer_with(stream, "200 OK", &["ETag: \"e\""], "index");
    });
    let adapter = store.adapter().await;
    let scratch = tempfile::tempdir().expect("scratch");
    let dest = scratch.path().join("copy.duckdb");

    let fetched = adapter.fetch_to(&dest).await;

    assert!(matches!(fetched, Ok(Some(_))), "{fetched:?}");
}

/// A store that gives each request the next of `answers`, and the last for good.
fn answering_in_turn(answers: &'static [&'static str]) -> LoopbackStore {
    let turn = AtomicUsize::new(0);
    LoopbackStore::answering(move |_, stream| {
        let this_turn = turn.fetch_add(1, Ordering::SeqCst);
        let status = answers[this_turn.min(answers.len() - 1)];
        answer_with(stream, status, &[], "");
    })
}

/// Init's way in: sync, on a blocking thread of the running runtime.
async fn write_probe_as_init_runs_it(adapter: &Arc<S3IndexObjectAdapter>) -> ProbeOutcome {
    let adapter = Arc::clone(adapter);
    tokio::task::spawn_blocking(move || IndexObjectWritePort::probe(adapter.as_ref()))
        .await
        .expect("the probe answered")
}

/// D-STZ-39, on what moto cannot play: a store that ignores the condition, and every
/// answer that says nothing about it. Each row is how a run ends and how many PUTs it
/// took, so a retry or a repeat skipped or added shows here.
#[tokio::test]
async fn the_write_probe_refuses_only_two_accepted_puts_after_at_most_two_puts() {
    const TAKEN: &str = "200 OK";
    const HONOURED: &str = "412 Precondition Failed";
    const DENIED: &str = "403 Forbidden";
    const DOWN: &str = "500 Internal Server Error";
    let rows: [(&[&str], usize, bool); 7] = [
        (&[HONOURED], 1, false),
        (&[DENIED], 1, false),
        (&[DOWN], 1, false),
        (&[TAKEN, HONOURED], 2, false),
        (&[TAKEN, DENIED], 2, false),
        (&[TAKEN, DOWN], 2, false),
        (&[TAKEN, TAKEN], 2, true),
    ];
    for (answers, puts, refused) in rows {
        let store = answering_in_turn(answers);
        let adapter = Arc::new(store.adapter().await);

        let outcome = write_probe_as_init_runs_it(&adapter).await;

        let ignored = matches!(
            outcome,
            ProbeOutcome::Refused {
                reason: ProbeRefusalReason::IndexObjectWriteConditionIgnored,
                ..
            }
        );
        assert_eq!(ignored, refused, "{answers:?}: {outcome:?}");
        assert_eq!(matches!(outcome, ProbeOutcome::Ok), !refused, "{answers:?}");
        assert_eq!(store.requests().len(), puts, "{answers:?}");
    }
}

/// Every PUT is the same empty create-only write, on the marker's key alone.
#[tokio::test]
async fn each_write_probe_put_is_an_empty_create_only_write_to_the_marker() {
    let store = answering_in_turn(&["200 OK", "412 Precondition Failed"]);
    let adapter = Arc::new(store.adapter().await);

    write_probe_as_init_runs_it(&adapter).await;

    let sent = store.requests();
    assert_eq!(sent.len(), 2);
    for request in &sent {
        assert_eq!(request.method(), "PUT");
        assert!(
            request
                .head
                .lines()
                .next()
                .is_some_and(|line| { line.contains(&format!("/{WRITE_PROBE_KEY}")) }),
            "{}",
            request.head
        );
        assert_eq!(request.header("if-none-match"), Some("*"));
        assert_eq!(request.header("if-match"), None);
        assert_eq!(request.header("content-length"), Some("0"));
    }
}

/// § Timeouts: a silent first PUT is unknown, at its 2 s and not the 1 s below it, and
/// the one PUT sent means no repeat and no retry.
#[tokio::test]
async fn a_silent_store_passes_a_write_probe_at_its_two_seconds_after_one_put() {
    let store = LoopbackStore::answering(|_, stream| hold_silently(stream));
    let adapter = Arc::new(store.adapter().await);
    let started = Instant::now();

    let outcome = write_probe_as_init_runs_it(&adapter).await;

    let took = started.elapsed();
    assert!(matches!(outcome, ProbeOutcome::Ok), "{outcome:?}");
    assert!(took >= TWO_SECONDS - Duration::from_millis(200), "{took:?}");
    assert!(took < TWO_SECONDS + MARGIN, "{took:?}");
    assert_eq!(store.requests().len(), 1);
}

/// RIO-4's whitespace, which moto's seed does not send: the kill for a trimmed list.
#[tokio::test]
async fn the_list_keeps_the_whitespace_it_was_saved_with() {
    let store = LoopbackStore::answering(|_, stream| {
        let body = r#"{"Parameter":{"Value":"  did:plc:a\ndid:plc:b\n"}}"#;
        answer_with(stream, "200 OK", &[], body);
    });
    let list = store.did_list().await;

    let read = list.current().await;

    assert_eq!(read.as_deref(), Ok("  did:plc:a\ndid:plc:b\n"));
    let sent = store.requests();
    assert_eq!(sent.len(), 1);
    assert_eq!(
        sent[0].header("x-amz-target"),
        Some("AmazonSSM.GetParameter")
    );
}

/// Every failure is a failure and a refused probe, after one attempt each.
#[tokio::test]
async fn a_list_the_store_will_not_give_fails_and_refuses_the_probe() {
    let missing = r#"{"__type":"ParameterNotFound"}"#;
    for status in ["400 Bad Request", "500 Internal Server Error"] {
        let store = LoopbackStore::answering(move |_, stream| {
            answer_with(stream, status, &[], missing);
        });
        let list = Arc::new(store.did_list().await);

        let read = list.current().await;
        let probed = Arc::clone(&list);
        let outcome = tokio::task::spawn_blocking(move || probed.probe())
            .await
            .expect("the probe answered");

        assert!(read.is_err(), "{status}: {read:?}");
        let ProbeOutcome::Refused { reason, .. } = outcome else {
            panic!("{status}: {outcome:?}")
        };
        assert_eq!(reason, ProbeRefusalReason::IndexerConfigInvalid);
        assert_eq!(store.requests().len(), 2, "{status}: one attempt each");
    }
}

/// § Timeouts: 5 s, whichever way the parameter is read.
#[tokio::test]
async fn a_silent_parameter_store_fails_at_its_five_seconds() {
    let store = LoopbackStore::answering(|_, stream| hold_silently(stream));
    let list = store.did_list().await;
    let started = Instant::now();

    let read = list.current().await;

    let took = started.elapsed();
    assert!(read.is_err(), "{read:?}");
    assert!(
        took >= FIVE_SECONDS - Duration::from_millis(200),
        "{took:?}"
    );
    assert!(took < FIVE_SECONDS + MARGIN, "{took:?}");
}
