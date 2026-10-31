//! Crafter-owned (step 06-01): what moto cannot play. A loopback server gives the
//! adapter a trickling body, a short body, a refusal, an outage and a silent store,
//! so these checks need no moto.
//!
//! Layer 3 (real HTTP), example-only: they verify the wiring between the SDK's
//! answers and the pure mapping in `s3.rs`, which holds the properties.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use adapter_s3_index_object::{S3IndexObjectAdapter, S3Location, StoreCall};
use ports::{
    HeadOutcome, IndexObjectError, IndexObjectReadPort, IndexObjectWritePort, LastPass,
    PublishMetadata, PublishOutcome, WriteCondition,
};

const LIMIT: Duration = Duration::from_secs(35);
const THREE_SECONDS: Duration = Duration::from_secs(3);
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

    async fn adapter(&self) -> S3IndexObjectAdapter {
        for (key, value) in [
            ("AWS_ACCESS_KEY_ID", "testing"),
            ("AWS_SECRET_ACCESS_KEY", "testing"),
        ] {
            std::env::set_var(key, value);
        }
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
