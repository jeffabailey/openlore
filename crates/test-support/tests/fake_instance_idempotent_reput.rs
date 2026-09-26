//! Integration test (real sockets, no mocks): the `FakeInstance` honours the
//! ADR-062 §1 idempotency contract on the wire — a re-PUT of an already
//! committed CID, with or without the `x-openlore-manifest-entry` commit
//! header, is a no-op success: the stored bytes stay the first write and the
//! manifest never gains a second entry for that CID (step 02-02, AC-3). This
//! is what keeps re-push / interrupted-resume duplicate-free when the CLI
//! re-sends a record the instance already holds.
//!
//! Driven through a raw HTTP/1.1 client over `std::net::TcpStream` and
//! observed only through the public `GET /manifest` and `GET /records/:cid`
//! routes — the same port the `atproto/` Worker's contract test exercises.

use std::io::{Read, Write};
use std::net::TcpStream;

use openlore_test_support::{FakeInstance, MANIFEST_ENTRY_HEADER};

const CID: &str = "bafyreigh2akiscaildcqabsyg3dfr6chu3fgpregiymsck7e7aqa4s52zy";

/// One request/response over a fresh connection; returns (status, body).
fn http(
    base_url: &str,
    method: &str,
    path: &str,
    headers: &[(&str, &str)],
    body: &[u8],
) -> (u16, Vec<u8>) {
    let authority = base_url.trim_start_matches("http://");
    let mut stream = TcpStream::connect(authority).expect("connect to FakeInstance");
    let extra: String = headers
        .iter()
        .map(|(name, value)| format!("{name}: {value}\r\n"))
        .collect();
    let head = format!(
        "{method} {path} HTTP/1.1\r\nhost: {authority}\r\ncontent-length: {}\r\n{extra}connection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(head.as_bytes()).expect("write head");
    stream.write_all(body).expect("write body");
    let mut raw = Vec::new();
    stream.read_to_end(&mut raw).expect("read response");
    parse_response(&raw)
}

fn parse_response(raw: &[u8]) -> (u16, Vec<u8>) {
    let split = raw
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .expect("response head terminator");
    let head = String::from_utf8_lossy(&raw[..split]);
    let status = head
        .split_whitespace()
        .nth(1)
        .and_then(|code| code.parse().ok())
        .expect("status code");
    (status, raw[split + 4..].to_vec())
}

fn put_record(base_url: &str, body: &[u8], manifest_entry: Option<&str>) -> u16 {
    let headers: Vec<(&str, &str)> = manifest_entry
        .map(|entry| vec![(MANIFEST_ENTRY_HEADER, entry)])
        .unwrap_or_default();
    http(base_url, "PUT", &format!("/records/{CID}"), &headers, body).0
}

/// The CIDs `GET /manifest` lists, in append order.
fn manifest_cids(base_url: &str) -> Vec<String> {
    let (status, body) = http(base_url, "GET", "/manifest", &[], b"");
    assert_eq!(status, 200, "GET /manifest must succeed");
    let manifest: serde_json::Value = serde_json::from_slice(&body).expect("manifest is JSON");
    manifest["records"]
        .as_array()
        .expect("manifest.records is an array")
        .iter()
        .map(|entry| entry["cid"].as_str().expect("entry.cid").to_string())
        .collect()
}

fn entry_json() -> String {
    serde_json::json!({ "cid": CID, "subject": "github:maria/project-000" }).to_string()
}

#[test]
fn re_put_of_a_committed_cid_is_a_no_op_success_with_or_without_the_commit_header() {
    let instance = FakeInstance::fresh();
    let url = instance.endpoint_url().to_string();
    let entry = entry_json();

    // Given the CID is committed once (blob + manifest entry).
    assert!((200..300).contains(&put_record(&url, b"first-bytes", Some(&entry))));
    assert_eq!(manifest_cids(&url), vec![CID.to_string()]);

    // When it is re-PUT carrying the commit header, and again without it.
    let reput_committing = put_record(&url, b"different-bytes", Some(&entry));
    let reput_bare = put_record(&url, b"other-bytes", None);

    // Then both are successes, the manifest still lists the CID exactly once,
    // and the stored bytes are the first write (content-addressed, first wins).
    assert!(
        (200..300).contains(&reput_committing),
        "committing re-PUT: {reput_committing}"
    );
    assert!(
        (200..300).contains(&reput_bare),
        "bare re-PUT: {reput_bare}"
    );
    assert_eq!(manifest_cids(&url), vec![CID.to_string()]);
    let (status, stored) = http(&url, "GET", &format!("/records/{CID}"), &[], b"");
    assert_eq!((status, stored), (200, b"first-bytes".to_vec()));
}

#[test]
fn concurrent_identical_committing_puts_append_the_manifest_exactly_once() {
    let instance = FakeInstance::fresh();
    let url = instance.endpoint_url().to_string();
    let entry = entry_json();

    // When eight identical committing PUTs of one CID race on separate sockets.
    let statuses: Vec<u16> = std::thread::scope(|scope| {
        let racers: Vec<_> = (0..8)
            .map(|_| scope.spawn(|| put_record(&url, b"same-bytes", Some(&entry))))
            .collect();
        racers
            .into_iter()
            .map(|racer| racer.join().expect("racer"))
            .collect()
    });

    // Then all succeed and the manifest holds a single entry for the CID.
    assert!(
        statuses.iter().all(|status| (200..300).contains(status)),
        "{statuses:?}"
    );
    assert_eq!(manifest_cids(&url), vec![CID.to_string()]);
}
