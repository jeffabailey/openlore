//! What the public listener answers, status AND body, over a real socket
//! (ADR-083 §2/§3): `/healthz` carries the health projection, a body over the
//! limit is 413, a body cut short is 400 (not 413), and a search the index
//! could not read is a 500 naming `index_unavailable`. Plus the admission
//! bounds at their exact edges.

use std::io::{Read, Write};
use std::net::{Shutdown, SocketAddr, TcpStream};
use std::sync::Arc;
use std::time::Duration;

use adapter_xrpc_query_server::{
    admit, Admission, IndexUnavailable, QueryHandler, ServeLimits, XrpcQueryServer,
    MAX_REQUEST_BODY_BYTES, MAX_SEARCH_VALUE_BYTES,
};
use appview_domain::health::{health_of, StoreHealth};
use lexicon::{SearchQueryRequest, SearchQueryResponse};

const SEARCH_PATH: &str = "/xrpc/org.openlore.appview.searchClaims";
const SEARCH_BODY: &str = r#"{"dimension":"object","value":"org.openlore.philosophy.x"}"#;

fn serve(handler: QueryHandler) -> SocketAddr {
    let (bound, address) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime");
        runtime.block_on(async move {
            let addr: SocketAddr = "127.0.0.1:0".parse().expect("addr");
            let server = XrpcQueryServer::bind(addr, handler)
                .expect("bind")
                .with_health(Arc::new(|| health_of(StoreHealth::Unusable, None)))
                .with_limits(ServeLimits::default());
            bound.send(server.local_addr()).expect("report address");
            let _ = server.serve().await;
        });
    });
    address.recv().expect("server address")
}

fn empty_answer_handler() -> QueryHandler {
    Arc::new(|_: SearchQueryRequest| {
        Ok(SearchQueryResponse {
            results: Vec::new(),
            distinct_author_count: 0,
            total_claims: 0,
            suggestion: None,
        })
    })
}

/// Write `head` then `body`, optionally half-close, and read the whole answer.
fn exchange(addr: SocketAddr, head: &str, body: &[u8], half_close: bool) -> (u16, String) {
    let mut stream = TcpStream::connect(addr).expect("connect");
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .expect("read timeout");
    let mut request = head.as_bytes().to_vec();
    request.extend_from_slice(body);
    stream.write_all(&request).expect("write");
    if half_close {
        stream.shutdown(Shutdown::Write).expect("half-close");
    }
    let mut response = Vec::new();
    let _ = stream.read_to_end(&mut response);
    let text = String::from_utf8_lossy(&response).to_string();
    let status = text
        .split_whitespace()
        .nth(1)
        .and_then(|code| code.parse().ok())
        .unwrap_or(0);
    (status, text)
}

fn search_head(content_length: usize) -> String {
    format!(
        "POST {SEARCH_PATH} HTTP/1.1\r\nHost: index\r\nConnection: close\r\n\
         Content-Type: application/json\r\nContent-Length: {content_length}\r\n\r\n"
    )
}

#[test]
fn healthz_answers_with_the_health_projection_status_and_body() {
    let addr = serve(empty_answer_handler());
    let (status, text) = exchange(
        addr,
        "GET /healthz HTTP/1.1\r\nHost: index\r\nConnection: close\r\n\r\n",
        b"",
        false,
    );
    assert_eq!(status, 503, "{text}");
    assert!(text.ends_with(r#"{"status":"store_unusable"}"#), "{text}");
}

#[test]
fn a_body_over_the_limit_is_413() {
    let addr = serve(empty_answer_handler());
    let body = vec![b' '; MAX_REQUEST_BODY_BYTES + 1];
    let (status, text) = exchange(addr, &search_head(body.len()), &body, false);
    assert_eq!(status, 413, "{text}");
    assert!(text.contains("request body exceeds 8192 bytes"), "{text}");
}

#[test]
fn a_body_cut_short_is_400_not_413() {
    let addr = serve(empty_answer_handler());
    let (status, text) = exchange(addr, &search_head(100), b"{\"dim", true);
    assert_eq!(status, 400, "{text}");
}

#[test]
fn a_search_the_index_cannot_read_is_500_index_unavailable() {
    let addr = serve(Arc::new(|_: SearchQueryRequest| Err(IndexUnavailable)));
    let (status, text) = exchange(
        addr,
        &search_head(SEARCH_BODY.len()),
        SEARCH_BODY.as_bytes(),
        false,
    );
    assert_eq!(status, 500, "{text}");
    assert!(text.ends_with(r#"{"error":"index_unavailable"}"#), "{text}");
}

#[test]
fn admission_holds_both_bounds_at_their_exact_edges() {
    assert_eq!(MAX_REQUEST_BODY_BYTES, 8192);
    assert_eq!(MAX_SEARCH_VALUE_BYTES, 512);
    assert_eq!(admit(8192, 512), Admission::Admitted);
    assert_eq!(admit(8193, 0), Admission::TooLarge);
    assert_eq!(
        admit(8193, 513),
        Admission::TooLarge,
        "size is checked first"
    );
    assert_eq!(admit(100, 513), Admission::BadRequest);
}
