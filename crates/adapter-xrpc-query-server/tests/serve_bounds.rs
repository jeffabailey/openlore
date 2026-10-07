//! The public listener's bounds over a real socket (review H2, H3b, M2): a
//! slow body is cut off with 408, an idle kept-alive connection is closed, a
//! blocked store call never stalls `/healthz`, and a client over its rate is
//! refused with 429 while other clients are not.
//!
//! Each test serves on its own CURRENT-THREAD runtime thread, as the indexer
//! does, so a handler run on the HTTP executor would stall every other
//! connection; the clients are plain blocking sockets on the test thread.

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::sync::Arc;
use std::time::{Duration, Instant};

use adapter_xrpc_query_server::{
    QueryHandler, RateLimit, ServeLimits, ServeTimeouts, TrustedProxies, XrpcQueryServer,
};
use appview_domain::health::{health_of, StoreHealth};
use lexicon::{SearchQueryRequest, SearchQueryResponse};

const SEARCH_PATH: &str = "/xrpc/org.openlore.appview.searchClaims";
const SEARCH_BODY: &str = r#"{"dimension":"object","value":"org.openlore.philosophy.x"}"#;

fn empty_answer() -> SearchQueryResponse {
    SearchQueryResponse {
        results: Vec::new(),
        distinct_author_count: 0,
        total_claims: 0,
        suggestion: None,
    }
}

/// Serve `handler` (and a healthy `/healthz`) under `limits` on a
/// current-thread runtime of its own; the address.
fn serve(handler: QueryHandler, limits: ServeLimits) -> SocketAddr {
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
                .with_health(Arc::new(|| health_of(StoreHealth::Usable, None)))
                .with_limits(limits);
            bound.send(server.local_addr()).expect("report address");
            let _ = server.serve().await;
        });
    });
    address.recv().expect("server address")
}

fn instant_handler() -> QueryHandler {
    Arc::new(|_: SearchQueryRequest| Ok(empty_answer()))
}

fn limits_with(timeouts: ServeTimeouts, rate: RateLimit) -> ServeLimits {
    ServeLimits {
        timeouts,
        rate,
        trusted_proxies: TrustedProxies::loopback_only(),
    }
}

/// One whole request on a fresh connection (closed after); the status line's
/// code and the raw response.
fn exchange(addr: SocketAddr, request: &str) -> (u16, String) {
    let mut stream = TcpStream::connect(addr).expect("connect");
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .expect("read timeout");
    stream.write_all(request.as_bytes()).expect("write");
    let mut response = Vec::new();
    let _ = stream.read_to_end(&mut response);
    let text = String::from_utf8_lossy(&response).to_string();
    (status_of(&text), text)
}

fn status_of(response: &str) -> u16 {
    response
        .split_whitespace()
        .nth(1)
        .and_then(|code| code.parse().ok())
        .unwrap_or(0)
}

fn search_request(forwarded_for: Option<&str>) -> String {
    let forwarded = forwarded_for
        .map(|ip| format!("X-Forwarded-For: {ip}\r\n"))
        .unwrap_or_default();
    format!(
        "POST {SEARCH_PATH} HTTP/1.1\r\nHost: index\r\nConnection: close\r\n{forwarded}\
         Content-Type: application/json\r\nContent-Length: {}\r\n\r\n{SEARCH_BODY}",
        SEARCH_BODY.len()
    )
}

fn health_request(forwarded_for: Option<&str>) -> String {
    let forwarded = forwarded_for
        .map(|ip| format!("X-Forwarded-For: {ip}\r\n"))
        .unwrap_or_default();
    format!("GET /healthz HTTP/1.1\r\nHost: index\r\nConnection: close\r\n{forwarded}\r\n")
}

#[test]
fn a_slow_trickling_body_gets_408_within_the_bound_while_other_searches_are_answered() {
    let request_bound = Duration::from_millis(500);
    let addr = serve(
        instant_handler(),
        limits_with(
            ServeTimeouts {
                header_read: Duration::from_secs(10),
                request: request_bound,
            },
            RateLimit::DEFAULT,
        ),
    );

    let started = Instant::now();
    let slow = std::thread::spawn(move || {
        let mut stream = TcpStream::connect(addr).expect("connect");
        let head = format!(
            "POST {SEARCH_PATH} HTTP/1.1\r\nHost: index\r\n\
             Content-Type: application/json\r\nContent-Length: 4000\r\n\r\n"
        );
        stream.write_all(head.as_bytes()).expect("head");
        let mut writer = stream.try_clone().expect("clone");
        std::thread::spawn(move || {
            for _ in 0..80 {
                if writer.write_all(b" ").is_err() {
                    break;
                }
                std::thread::sleep(Duration::from_millis(50));
            }
        });
        stream
            .set_read_timeout(Some(Duration::from_secs(10)))
            .expect("read timeout");
        let mut response = Vec::new();
        let _ = stream.read_to_end(&mut response);
        (
            status_of(&String::from_utf8_lossy(&response)),
            started.elapsed(),
        )
    });

    std::thread::sleep(Duration::from_millis(100));
    let (normal, _) = exchange(addr, &search_request(None));
    assert_eq!(normal, 200, "a normal search beside the slow one");

    let (status, took) = slow.join().expect("slow client");
    assert_eq!(status, 408, "the slow body is refused as too slow");
    assert!(
        took < request_bound + Duration::from_millis(1_000),
        "408 within the bound, not after the trickle ends: {took:?}"
    );
}

#[test]
fn an_idle_kept_alive_connection_is_closed_within_the_header_bound() {
    let header_bound = Duration::from_millis(300);
    let addr = serve(
        instant_handler(),
        limits_with(
            ServeTimeouts {
                header_read: header_bound,
                request: Duration::from_secs(5),
            },
            RateLimit::DEFAULT,
        ),
    );
    let mut stream = TcpStream::connect(addr).expect("connect");
    stream
        .write_all(b"GET /healthz HTTP/1.1\r\nHost: index\r\n\r\n")
        .expect("write");
    let mut first = [0u8; 1024];
    let read = stream.read(&mut first).expect("first response");
    assert_eq!(status_of(&String::from_utf8_lossy(&first[..read])), 200);

    let idle_since = Instant::now();
    stream
        .set_read_timeout(Some(Duration::from_secs(3)))
        .expect("read timeout");
    let mut rest = Vec::new();
    let closed = stream.read_to_end(&mut rest);
    assert!(
        closed.is_ok(),
        "the idle connection is still open after 3 s: {closed:?}"
    );
    assert!(
        idle_since.elapsed() < header_bound + Duration::from_millis(1_000),
        "closed after {:?}",
        idle_since.elapsed()
    );
}

#[test]
fn healthz_answers_while_a_search_is_blocked_in_the_store() {
    let store_held = Duration::from_millis(2_000);
    let blocking: QueryHandler = Arc::new(move |_: SearchQueryRequest| {
        std::thread::sleep(store_held);
        Ok(empty_answer())
    });
    let addr = serve(blocking, ServeLimits::default());

    let search = std::thread::spawn(move || exchange(addr, &search_request(None)));
    std::thread::sleep(Duration::from_millis(200));

    let asked = Instant::now();
    let (health, _) = exchange(addr, &health_request(None));
    let took = asked.elapsed();
    assert_eq!(health, 200);
    assert!(
        took < Duration::from_millis(500),
        "/healthz waited {took:?} behind a search blocked in the store"
    );
    assert_eq!(search.join().expect("search").0, 200);
}

#[test]
fn a_search_the_store_holds_past_the_bound_is_503_not_a_stall() {
    let blocking: QueryHandler = Arc::new(|_: SearchQueryRequest| {
        std::thread::sleep(Duration::from_millis(1_500));
        Ok(empty_answer())
    });
    let addr = serve(
        blocking,
        limits_with(
            ServeTimeouts {
                header_read: Duration::from_secs(10),
                request: Duration::from_millis(300),
            },
            RateLimit::DEFAULT,
        ),
    );
    let (status, response) = exchange(addr, &search_request(None));
    assert_eq!(status, 503, "{response}");
    assert!(response.to_ascii_lowercase().contains("retry-after: 1"));
}

#[test]
fn a_client_over_its_burst_gets_429_and_others_are_unaffected() {
    let burst = 5;
    let addr = serve(
        instant_handler(),
        limits_with(
            ServeTimeouts::default(),
            RateLimit::new(1, burst).expect("limit"),
        ),
    );
    let flooder = "203.0.113.7";

    let mut statuses = Vec::new();
    for _ in 0..=burst {
        statuses.push(exchange(addr, &search_request(Some(flooder))).0);
    }
    assert_eq!(statuses, vec![200, 200, 200, 200, 200, 429], "{statuses:?}");

    let (refused, response) = exchange(addr, &search_request(Some(flooder)));
    assert_eq!(refused, 429);
    assert!(
        response.to_ascii_lowercase().contains("retry-after: 1"),
        "{response}"
    );

    let (other, _) = exchange(addr, &search_request(Some("198.51.100.9")));
    assert_eq!(other, 200, "another client keeps its own bucket");

    let (health, _) = exchange(addr, &health_request(Some(flooder)));
    assert_eq!(health, 200, "/healthz is not rate limited");
}
