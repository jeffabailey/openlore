//! Review R2: one GitHub request is bounded as a whole. A GitHub that
//! accepts the connection and never answers fails the read within
//! [`REQUEST_TIMEOUT`] (plus slack), never hangs the caller.

use std::io::Read;
use std::net::TcpListener;
use std::time::{Duration, Instant};

use adapter_github::client::REQUEST_TIMEOUT;
use adapter_github::GithubAdapter;
use ports::{GithubError, GithubPort};

/// A localhost GitHub that accepts each connection, reads the request and
/// never writes an answer.
fn stalled_github() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let base = format!("http://{}", listener.local_addr().expect("addr"));
    std::thread::spawn(move || {
        let mut held = Vec::new();
        for mut stream in listener.incoming().flatten() {
            let mut sink = [0u8; 1024];
            let _ = stream.read(&mut sink);
            held.push(stream);
        }
    });
    base
}

#[tokio::test]
async fn a_github_that_never_answers_fails_the_read_within_the_request_bound() {
    let adapter = GithubAdapter::for_api_base(stalled_github());

    let started = Instant::now();
    let read = adapter.read_account("octocat").await;
    let took = started.elapsed();

    assert!(
        matches!(read, Err(GithubError::Network(_))),
        "a stalled GitHub is a network failure: {read:?}"
    );
    assert!(
        took >= REQUEST_TIMEOUT - Duration::from_millis(500)
            && took < REQUEST_TIMEOUT + Duration::from_secs(5),
        "bounded by the {REQUEST_TIMEOUT:?} request timeout, took {took:?}"
    );
}
