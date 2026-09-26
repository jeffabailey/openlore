//! Integration tests: `HttpPublishAdapter` against a REAL local TCP listener
//! (no mocks). A tiny scripted HTTP/1.1 server stands in for the instance so
//! each test pins one wire behaviour of the opaque transport (ADR-062 §1/§6).

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};

use adapter_publish_http::{HttpPublishAdapter, MANIFEST_ENTRY_HEADER};
use claim_domain::Cid;
use ports::{
    InstanceReadPort, ManifestEntry, ProbeOutcome, ProbeRefusalReason, PublishPort, RecordBytes,
};

/// One request as the scripted server saw it.
#[derive(Debug, Clone)]
struct SeenRequest {
    method: String,
    path: String,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

impl SeenRequest {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }
}

type Responder = dyn Fn(&SeenRequest) -> (u16, Vec<u8>) + Send + Sync;

/// Serve `respond` on 127.0.0.1:<ephemeral> in a background thread; every
/// request is recorded. Each response closes the connection.
fn serve(respond: Box<Responder>) -> (String, Arc<Mutex<Vec<SeenRequest>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let base_url = format!("http://{}", listener.local_addr().expect("addr"));
    let seen = Arc::new(Mutex::new(Vec::new()));
    let seen_by_server = Arc::clone(&seen);
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let mut reader = BufReader::new(stream.try_clone().expect("clone stream"));
            let Some(request) = read_request(&mut reader) else {
                continue;
            };
            let (status, body) = respond(&request);
            seen_by_server.lock().expect("seen").push(request);
            let mut stream = stream;
            let head = format!(
                "HTTP/1.1 {status} X\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
                body.len()
            );
            let _ = stream.write_all(head.as_bytes());
            let _ = stream.write_all(&body);
        }
    });
    (base_url, seen)
}

fn read_request(reader: &mut BufReader<std::net::TcpStream>) -> Option<SeenRequest> {
    let mut request_line = String::new();
    reader.read_line(&mut request_line).ok()?;
    let mut parts = request_line.split_whitespace();
    let method = parts.next()?.to_string();
    let path = parts.next()?.to_string();
    let mut headers = Vec::new();
    loop {
        let mut line = String::new();
        reader.read_line(&mut line).ok()?;
        let line = line.trim_end();
        if line.is_empty() {
            break;
        }
        let (name, value) = line.split_once(':')?;
        headers.push((name.trim().to_string(), value.trim().to_string()));
    }
    let length = headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("content-length"))
        .and_then(|(_, v)| v.parse::<usize>().ok())
        .unwrap_or(0);
    let mut body = vec![0u8; length];
    reader.read_exact(&mut body).ok()?;
    Some(SeenRequest {
        method,
        path,
        headers,
        body,
    })
}

fn marked_manifest() -> Vec<u8> {
    serde_json::json!({
        "openlore": { "kind": "opaque-instance", "contract_version": 1 },
        "records": []
    })
    .to_string()
    .into_bytes()
}

fn refusal_reason(outcome: ProbeOutcome) -> Option<(ProbeRefusalReason, serde_json::Value)> {
    match outcome {
        ProbeOutcome::Ok => None,
        ProbeOutcome::Refused {
            reason, structured, ..
        } => Some((reason, structured)),
    }
}

#[test]
fn probe_refuses_with_instance_unreachable_when_the_connection_is_refused() {
    // Bind then drop: nothing listens on the port any more.
    let port = TcpListener::bind("127.0.0.1:0")
        .expect("bind")
        .local_addr()
        .expect("addr")
        .port();
    let adapter = HttpPublishAdapter::for_instance(&format!("http://127.0.0.1:{port}"));

    let (reason, structured) = refusal_reason(adapter.probe()).expect("probe must refuse");

    assert_eq!(reason, ProbeRefusalReason::PublishInstanceUnreachable);
    assert_eq!(structured["reason_code"], "publish.instance_unreachable");
}

#[test]
fn probe_refuses_a_reachable_url_whose_manifest_lacks_the_openlore_marker() {
    let (base_url, _seen) = serve(Box::new(|_req| {
        (200, br#"{"hello":"an ordinary site"}"#.to_vec())
    }));
    let adapter = HttpPublishAdapter::for_instance(&base_url);

    let (reason, structured) = refusal_reason(adapter.probe()).expect("probe must refuse");

    assert_eq!(reason, ProbeRefusalReason::PublishNotAnOpenloreInstance);
    assert_eq!(
        structured["reason_code"],
        "publish.not_an_openlore_instance"
    );
}

#[test]
fn probe_refuses_an_ordinary_html_site_as_not_an_openlore_instance_not_unreachable() {
    let (base_url, _seen) = serve(Box::new(|_req| {
        (
            200,
            b"<!doctype html><title>my blog</title><p>hello</p>".to_vec(),
        )
    }));
    let adapter = HttpPublishAdapter::for_instance(&base_url);

    let (reason, structured) = refusal_reason(adapter.probe()).expect("probe must refuse");

    assert_eq!(reason, ProbeRefusalReason::PublishNotAnOpenloreInstance);
    assert_eq!(
        structured["reason_code"],
        "publish.not_an_openlore_instance"
    );
}

#[test]
fn stage_and_commit_send_the_record_bytes_verbatim_and_only_the_commit_carries_the_projection() {
    let (base_url, seen) = serve(Box::new(|req| match req.path.as_str() {
        "/manifest" => (200, marked_manifest()),
        _ => (201, Vec::new()),
    }));
    let adapter = HttpPublishAdapter::for_instance(&base_url);
    let cid = Cid("bafyexample".to_string());
    let record = RecordBytes(br#"{"subject":"github:a/b","confidence":0.5}"#.to_vec());
    let entry = ManifestEntry {
        cid: cid.0.clone(),
        author_did: "did:plc:maria-test".to_string(),
        subject: "github:caf\u{e9}/\u{1f980}".to_string(),
        predicate: "embodiesPhilosophy".to_string(),
        object: "org.openlore.philosophy.x".to_string(),
        confidence: 0.5,
        composed_at: "2026-05-25T12:00:00Z".to_string(),
    };

    assert!(matches!(adapter.probe(), ProbeOutcome::Ok));
    adapter.put_record(&cid, &record).expect("stage");
    adapter
        .commit_manifest_entry(&cid, &record, &entry)
        .expect("commit");

    let seen = seen.lock().expect("seen").clone();
    let puts: Vec<&SeenRequest> = seen.iter().filter(|r| r.method == "PUT").collect();
    assert_eq!(puts.len(), 2);
    assert!(puts.iter().all(|r| r.path == "/records/bafyexample"));
    assert!(
        puts.iter().all(|r| r.body == record.0),
        "bytes must go out verbatim"
    );
    assert_eq!(puts[0].header(MANIFEST_ENTRY_HEADER), None);
    let header = puts[1]
        .header(MANIFEST_ENTRY_HEADER)
        .expect("commit carries the projection");
    let projected: ManifestEntry = serde_json::from_str(header).expect("header is JSON");
    assert_eq!(
        projected, entry,
        "non-ASCII survives the ASCII-escaped header"
    );
}

#[test]
fn read_back_returns_divergent_bytes_verbatim_so_the_core_can_reject_them() {
    let divergent = br#"{"subject":"tampered"}"#.to_vec();
    let served = divergent.clone();
    let (base_url, _seen) = serve(Box::new(move |req| match req.path.as_str() {
        "/records/bafyexample" => (200, served.clone()),
        _ => (404, Vec::new()),
    }));
    let adapter = HttpPublishAdapter::for_instance(&base_url);

    let returned = adapter
        .get_record(&Cid("bafyexample".to_string()))
        .expect("read back");
    let missing = adapter.get_record(&Cid("bafyabsent".to_string()));

    assert_eq!(returned.0, divergent, "the adapter never masks divergence");
    assert!(matches!(
        missing,
        Err(ports::InstanceError::RecordNotFound { .. })
    ));
}
