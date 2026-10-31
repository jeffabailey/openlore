//! `dispatch` (D-STZ-22) — the public surface with no transport: the route
//! table, `admit`, the time limit, and every answer they lead to. A caller
//! brings a method, a path, a body still to be read and the wired handlers; it
//! gets a plain [`PublicAnswer`] back. No socket, no HTTP server, no connection
//! cap and no rate limiter (D-STZ-19): those stay with the caller that needs
//! them, in front of this function.
//!
//! The body is a future so each transport keeps its own way of reading one: the
//! hyper server hands in a read capped at [`MAX_REQUEST_BODY_BYTES`], and only a
//! search ever awaits it. A body that arrived whole is checked against the same
//! bound here, so the bounds hold however it was read.

use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use appview_domain::health::HealthResponse;
use bytes::Bytes;
use lexicon::{SearchQueryRequest, SearchQueryResponse};
use tokio::time::{timeout_at, Instant};

use crate::{
    admit, public_route, Admission, HealthHandler, IndexUnavailable, PublicRoute, QueryHandler,
    MAX_REQUEST_BODY_BYTES, MAX_SEARCH_VALUE_BYTES,
};

/// The 500 body of a search the index could not answer (a short reason only).
const INDEX_UNAVAILABLE_BODY: &str = r#"{"error":"index_unavailable"}"#;

/// The 503 body of a search the index did not answer in time.
const INDEX_BUSY_BODY: &str = r#"{"error":"index_busy"}"#;

const JSON_CONTENT: (&str, &str) = ("content-type", "application/json");

/// What the public surface answers with once wired.
#[derive(Clone)]
pub struct Handlers {
    pub query: QueryHandler,
    /// `None` leaves `GET /healthz` not found.
    pub health: Option<HealthHandler>,
}

/// One request, as any transport can state it.
pub struct PublicRequest<'request, Body> {
    pub method: &'request str,
    pub path: &'request str,
    /// Awaited only by a search.
    pub body: Body,
    /// How long a search may take: its body read plus its query. A caller that
    /// spent part of its limit before this call passes what is left (D-STZ-10).
    pub time_allowed: Duration,
}

/// How reading a request body ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BodyRead {
    Complete(Bytes),
    /// The reader stopped at its bound.
    TooLarge,
    /// The transport failed; the reason is safe to answer with.
    Unreadable(String),
}

/// A transport-free answer: the status, the headers and the body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublicAnswer {
    pub status: u16,
    pub headers: Vec<(&'static str, &'static str)>,
    pub body: Bytes,
}

/// Answer one request through the public surface.
pub async fn dispatch<Body>(request: PublicRequest<'_, Body>, handlers: &Handlers) -> PublicAnswer
where
    Body: Future<Output = BodyRead>,
{
    match public_route(request.method, request.path) {
        PublicRoute::Search => search(request.body, request.time_allowed, &handlers.query).await,
        PublicRoute::Health => handlers
            .health
            .as_ref()
            .map_or_else(not_found, |health| health_answer(&health())),
        PublicRoute::NotFound => not_found(),
    }
}

/// Answer one search within `time_allowed`: read the body (408 when it is too
/// slow), admit it, and run the query off the executor (a blocking store call
/// never stalls other requests; 503 when it is too slow).
async fn search<Body>(body: Body, time_allowed: Duration, query: &QueryHandler) -> PublicAnswer
where
    Body: Future<Output = BodyRead>,
{
    let deadline = Instant::now() + time_allowed;
    let body = match timeout_at(deadline, body).await {
        Ok(BodyRead::Complete(body)) => body,
        Ok(BodyRead::TooLarge) => return too_large(),
        Ok(BodyRead::Unreadable(reason)) => return bad_request(format!("read body: {reason}")),
        Err(_too_slow) => return request_timeout(),
    };
    let request = match admitted_search(&body) {
        Ok(request) => request,
        Err(refusal) => return refusal,
    };
    let query = Arc::clone(query);
    let answer = tokio::task::spawn_blocking(move || query(request));
    match timeout_at(deadline, answer).await {
        Ok(Ok(Ok(results))) => results_answer(&results),
        Ok(Ok(Err(IndexUnavailable))) => index_unavailable(),
        Ok(Err(join)) => internal_error(format!("search task: {join}")),
        Err(_too_slow) => index_busy(),
    }
}

/// The search a body holds, once it is within both bounds. The size is judged
/// before the content, so an oversized body is never parsed.
fn admitted_search(body: &[u8]) -> Result<SearchQueryRequest, PublicAnswer> {
    if admit(body.len(), 0) == Admission::TooLarge {
        return Err(too_large());
    }
    let request: SearchQueryRequest =
        serde_json::from_slice(body).map_err(|err| bad_request(format!("parse request: {err}")))?;
    match admit(body.len(), request.value.len()) {
        Admission::Admitted => Ok(request),
        Admission::TooLarge => Err(too_large()),
        Admission::BadRequest => Err(bad_request(format!(
            "value exceeds {MAX_SEARCH_VALUE_BYTES} bytes"
        ))),
    }
}

fn results_answer(results: &SearchQueryResponse) -> PublicAnswer {
    match serde_json::to_vec(results) {
        Ok(json) => json_answer(200, json.into()),
        Err(err) => internal_error(format!("serialize response: {err}")),
    }
}

/// The health projection as an answer.
fn health_answer(health: &HealthResponse) -> PublicAnswer {
    json_answer(health.status_code(), health.body().to_string().into())
}

fn json_answer(status: u16, body: Bytes) -> PublicAnswer {
    PublicAnswer {
        status,
        headers: vec![JSON_CONTENT],
        body,
    }
}

fn text_answer(status: u16, body: impl Into<Bytes>) -> PublicAnswer {
    PublicAnswer {
        status,
        headers: Vec::new(),
        body: body.into(),
    }
}

fn not_found() -> PublicAnswer {
    text_answer(404, "not found")
}

fn too_large() -> PublicAnswer {
    text_answer(
        413,
        format!("request body exceeds {MAX_REQUEST_BODY_BYTES} bytes"),
    )
}

fn bad_request(message: String) -> PublicAnswer {
    text_answer(400, message)
}

/// A search the index could not answer: 500 with a short reason, no query value.
fn index_unavailable() -> PublicAnswer {
    json_answer(500, Bytes::from_static(INDEX_UNAVAILABLE_BODY.as_bytes()))
}

/// A body that did not arrive in time: 408, and the connection is closed.
fn request_timeout() -> PublicAnswer {
    PublicAnswer {
        headers: vec![("connection", "close")],
        ..text_answer(408, "request body too slow")
    }
}

/// A search the index did not answer in time: 503, retry shortly.
fn index_busy() -> PublicAnswer {
    PublicAnswer {
        status: 503,
        headers: vec![JSON_CONTENT, ("retry-after", "1")],
        body: Bytes::from_static(INDEX_BUSY_BODY.as_bytes()),
    }
}

fn internal_error(message: String) -> PublicAnswer {
    text_answer(500, message)
}

#[cfg(test)]
mod tests {
    //! The surface with no socket. Each observation is the whole universe a
    //! caller can see: {answer, searches run, health reads, body reads}, so a
    //! property pins what did NOT happen as well as what did.

    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Mutex;

    use appview_domain::health::{health_of, StoreHealth};
    use lexicon::SearchDimensionDto;
    use proptest::prelude::*;

    use super::*;
    use crate::rate_limit::DEFAULT_RATE_BURST;
    use crate::{ServeTimeouts, HEALTH_PATH, REQUEST_TIMEOUT};

    const SEARCH_PATH: &str = "/xrpc/org.openlore.appview.searchClaims";
    const JSON: (&str, &str) = ("content-type", "application/json");

    #[derive(Debug, PartialEq, Eq)]
    struct Observed {
        answer: PublicAnswer,
        searches: Vec<SearchQueryRequest>,
        health_reads: usize,
        body_reads: usize,
    }

    struct Wired {
        handlers: Handlers,
        searches: Arc<Mutex<Vec<SearchQueryRequest>>>,
        health_reads: Arc<AtomicUsize>,
    }

    fn empty_results() -> SearchQueryResponse {
        SearchQueryResponse {
            results: Vec::new(),
            distinct_author_count: 0,
            total_claims: 0,
            suggestion: None,
        }
    }

    fn wired(
        answer: Result<SearchQueryResponse, IndexUnavailable>,
        health: Option<HealthResponse>,
    ) -> Wired {
        let searches = Arc::new(Mutex::new(Vec::new()));
        let health_reads = Arc::new(AtomicUsize::new(0));
        let seen = Arc::clone(&searches);
        let query: QueryHandler = Arc::new(move |request| {
            seen.lock().expect("search spy").push(request);
            answer.clone()
        });
        let reads = Arc::clone(&health_reads);
        let health = health.map(|health| -> HealthHandler {
            Arc::new(move || {
                reads.fetch_add(1, Ordering::SeqCst);
                health
            })
        });
        Wired {
            handlers: Handlers { query, health },
            searches,
            health_reads,
        }
    }

    fn serving() -> Wired {
        wired(Ok(empty_results()), None)
    }

    fn block_on<Answer>(future: impl Future<Output = Answer>) -> Answer {
        tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .expect("runtime")
            .block_on(future)
    }

    fn observe(wired: &Wired, method: &str, path: &str, body: BodyRead) -> Observed {
        let body_reads = AtomicUsize::new(0);
        let request = PublicRequest {
            method,
            path,
            body: async {
                body_reads.fetch_add(1, Ordering::SeqCst);
                body
            },
            time_allowed: REQUEST_TIMEOUT,
        };
        let answer = block_on(dispatch(request, &wired.handlers));
        Observed {
            answer,
            searches: wired.searches.lock().expect("search spy").clone(),
            health_reads: wired.health_reads.load(Ordering::SeqCst),
            body_reads: body_reads.load(Ordering::SeqCst),
        }
    }

    fn post_search(wired: &Wired, body: Vec<u8>) -> Observed {
        observe(wired, "POST", SEARCH_PATH, BodyRead::Complete(body.into()))
    }

    fn answer(status: u16, headers: &[(&'static str, &'static str)], body: &str) -> PublicAnswer {
        PublicAnswer {
            status,
            headers: headers.to_vec(),
            body: Bytes::from(body.to_string()),
        }
    }

    fn results_answer() -> PublicAnswer {
        let json = serde_json::to_string(&empty_results()).expect("results serialize");
        answer(200, &[JSON], &json)
    }

    fn body_too_large() -> PublicAnswer {
        answer(413, &[], "request body exceeds 8192 bytes")
    }

    fn refused(answer: PublicAnswer, body_reads: usize) -> Observed {
        Observed {
            answer,
            searches: Vec::new(),
            health_reads: 0,
            body_reads,
        }
    }

    fn search_of(value: &str) -> SearchQueryRequest {
        SearchQueryRequest {
            dimension: SearchDimensionDto::Object,
            value: value.to_string(),
            cid: None,
        }
    }

    /// A valid search, JSON whitespace-padded to exactly `len` bytes.
    fn search_body_of_len(len: usize) -> Vec<u8> {
        let mut body = serde_json::to_vec(&search_of("x")).expect("request serializes");
        body.resize(len, b' ');
        body
    }

    fn any_search() -> impl Strategy<Value = SearchQueryRequest> {
        let dimension = prop_oneof![
            Just(SearchDimensionDto::Object),
            Just(SearchDimensionDto::Subject),
            Just(SearchDimensionDto::Contributor),
        ];
        (
            dimension,
            "[ -~]{0,512}",
            proptest::option::of("[a-z0-9]{1,16}"),
        )
            .prop_map(|(dimension, value, cid)| SearchQueryRequest {
                dimension,
                value,
                cid,
            })
    }

    fn unrouted() -> impl Strategy<Value = (String, String)> {
        let near_misses = prop_oneof![
            Just(("GET".to_string(), SEARCH_PATH.to_string())),
            Just(("POST".to_string(), HEALTH_PATH.to_string())),
            Just(("HEAD".to_string(), HEALTH_PATH.to_string())),
            Just(("POST".to_string(), format!("{SEARCH_PATH}/"))),
            Just(("GET".to_string(), format!("{HEALTH_PATH}/"))),
            Just(("post".to_string(), SEARCH_PATH.to_string())),
        ];
        let anything = ("[A-Z]{1,7}", "/[a-zA-Z./]{0,24}");
        prop_oneof![near_misses, anything].prop_filter("not a public route", |(method, path)| {
            public_route(method, path) == PublicRoute::NotFound
        })
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(64))]

        /// An admitted search runs once with the request as sent, and its
        /// results come back as JSON, with nothing but this function and its
        /// handlers involved.
        #[test]
        fn dispatch_answers_with_no_socket(search in any_search()) {
            let wired = serving();
            let body = serde_json::to_vec(&search).expect("request serializes");
            prop_assert_eq!(post_search(&wired, body), Observed {
                answer: results_answer(),
                searches: vec![search],
                health_reads: 0,
                body_reads: 1,
            });
        }

        /// Every other method and path is 404, before any body is read.
        #[test]
        fn anything_else_is_404_and_reads_no_body((method, path) in unrouted()) {
            let wired = serving();
            let observed = observe(&wired, &method, &path, BodyRead::Complete(Bytes::new()));
            prop_assert_eq!(observed, refused(answer(404, &[], "not found"), 0));
        }

        /// 8 KiB is admitted; one byte more is 413 and no search runs.
        #[test]
        fn the_body_bound_is_exact(
            len in (MAX_REQUEST_BODY_BYTES - 16)..=(MAX_REQUEST_BODY_BYTES + 16),
        ) {
            let wired = serving();
            let observed = post_search(&wired, search_body_of_len(len));
            let expected = if len <= MAX_REQUEST_BODY_BYTES {
                Observed {
                    answer: results_answer(),
                    searches: vec![search_of("x")],
                    health_reads: 0,
                    body_reads: 1,
                }
            } else {
                refused(body_too_large(), 1)
            };
            prop_assert_eq!(observed, expected);
        }

        /// The size is judged before the content: a body over the bound is 413
        /// whatever it holds, one within it that is not a search is 400.
        #[test]
        fn a_body_that_is_no_search_is_judged_by_its_size_first(
            len in (MAX_REQUEST_BODY_BYTES - 8)..=(MAX_REQUEST_BODY_BYTES + 8),
        ) {
            let wired = serving();
            let observed = post_search(&wired, vec![b'x'; len]);
            let status = if len <= MAX_REQUEST_BODY_BYTES { 400 } else { 413 };
            prop_assert_eq!(observed.answer.status, status);
            prop_assert_eq!(observed.searches, Vec::new());
        }

        /// 512 bytes of value are admitted; one byte more is 400, the value is
        /// not echoed, and no search runs.
        #[test]
        fn the_value_bound_is_exact(
            len in (MAX_SEARCH_VALUE_BYTES - 8)..=(MAX_SEARCH_VALUE_BYTES + 8),
        ) {
            let wired = serving();
            let search = search_of(&"v".repeat(len));
            let observed = post_search(&wired, serde_json::to_vec(&search).expect("serializes"));
            let expected = if len <= MAX_SEARCH_VALUE_BYTES {
                Observed {
                    answer: results_answer(),
                    searches: vec![search],
                    health_reads: 0,
                    body_reads: 1,
                }
            } else {
                refused(answer(400, &[], "value exceeds 512 bytes"), 1)
            };
            prop_assert_eq!(observed, expected);
        }

        /// A store that cannot be read is a 500 naming `index_unavailable`,
        /// never a 200 that would read as "no results".
        #[test]
        fn an_unreadable_index_is_500_never_200(search in any_search()) {
            let wired = wired(Err(IndexUnavailable), None);
            let body = serde_json::to_vec(&search).expect("request serializes");
            prop_assert_eq!(post_search(&wired, body), Observed {
                answer: answer(500, &[JSON], r#"{"error":"index_unavailable"}"#),
                searches: vec![search],
                health_reads: 0,
                body_reads: 1,
            });
        }

        /// `/healthz` answers the wired reader's projection, reading it once
        /// and running no search; it reads no body.
        #[test]
        fn health_answers_the_wired_reader(
            usable in any::<bool>(),
            published_day in proptest::option::of(1_u8..=28),
        ) {
            let store = if usable { StoreHealth::Usable } else { StoreHealth::Unusable };
            let published = published_day
                .and_then(|day| format!("2026-10-{day:02}T12:30:05Z").parse().ok());
            let health = health_of(store, published);
            let wired = wired(Ok(empty_results()), Some(health));
            let observed = observe(&wired, "GET", HEALTH_PATH, BodyRead::Complete(Bytes::new()));
            prop_assert_eq!(observed, Observed {
                answer: answer(health.status_code(), &[JSON], &health.body().to_string()),
                searches: Vec::new(),
                health_reads: 1,
                body_reads: 0,
            });
        }
    }

    // bypass: the rest are single states of a closed set or clock races; the
    // inputs carry no dimension worth generating.

    #[test]
    fn with_no_health_reader_healthz_is_404() {
        let observed = observe(
            &serving(),
            "GET",
            HEALTH_PATH,
            BodyRead::Complete(Bytes::new()),
        );
        assert_eq!(observed, refused(answer(404, &[], "not found"), 0));
    }

    #[test]
    fn a_reader_that_stopped_at_its_bound_is_413() {
        let observed = observe(&serving(), "POST", SEARCH_PATH, BodyRead::TooLarge);
        assert_eq!(observed, refused(body_too_large(), 1));
    }

    #[test]
    fn a_body_the_transport_lost_is_400() {
        let lost = BodyRead::Unreadable("connection reset".to_string());
        let observed = observe(&serving(), "POST", SEARCH_PATH, lost);
        assert_eq!(
            observed,
            refused(answer(400, &[], "read body: connection reset"), 1)
        );
    }

    #[test]
    fn a_query_that_panics_is_500_and_not_a_lost_connection() {
        let query: QueryHandler = Arc::new(|_| panic!("the store blew up"));
        let handlers = Handlers {
            query,
            health: None,
        };
        let request = PublicRequest {
            method: "POST",
            path: SEARCH_PATH,
            body: std::future::ready(BodyRead::Complete(search_body_of_len(64).into())),
            time_allowed: REQUEST_TIMEOUT,
        };
        let answered = block_on(dispatch(request, &handlers));
        assert_eq!(answered.status, 500);
        assert!(answered.body.starts_with(b"search task:"));
    }

    fn timed_search(
        query_time: Duration,
        body: impl Future<Output = BodyRead>,
        time_allowed: Duration,
    ) -> PublicAnswer {
        let query: QueryHandler = Arc::new(move |_| {
            std::thread::sleep(query_time);
            Ok(empty_results())
        });
        let handlers = Handlers {
            query,
            health: None,
        };
        let request = PublicRequest {
            method: "POST",
            path: SEARCH_PATH,
            body,
            time_allowed,
        };
        block_on(dispatch(request, &handlers))
    }

    fn whole_body() -> BodyRead {
        BodyRead::Complete(search_body_of_len(64).into())
    }

    #[test]
    fn a_query_past_the_time_allowed_is_503_index_busy() {
        let answered = timed_search(
            Duration::from_millis(400),
            std::future::ready(whole_body()),
            Duration::from_millis(40),
        );
        assert_eq!(
            answered,
            answer(
                503,
                &[JSON, ("retry-after", "1")],
                r#"{"error":"index_busy"}"#
            )
        );
    }

    /// A caller can arrive with its limit spent (D-STZ-10). A body already here
    /// is still read, so the answer is the usual retry and not a slow-body 408.
    #[test]
    fn a_search_with_no_time_left_is_503_index_busy() {
        let answered = timed_search(
            Duration::from_millis(400),
            std::future::ready(whole_body()),
            Duration::ZERO,
        );
        assert_eq!(
            answered,
            answer(
                503,
                &[JSON, ("retry-after", "1")],
                r#"{"error":"index_busy"}"#
            )
        );
    }

    #[test]
    fn a_body_past_the_time_allowed_is_408_and_closes_the_connection() {
        let answered = timed_search(
            Duration::ZERO,
            std::future::pending(),
            Duration::from_millis(40),
        );
        assert_eq!(
            answered,
            answer(408, &[("connection", "close")], "request body too slow")
        );
    }

    /// The body read and the query spend one allowance: each alone fits it,
    /// together they do not.
    #[test]
    fn the_body_read_and_the_query_share_the_time_allowed() {
        let slow_body = async {
            tokio::time::sleep(Duration::from_millis(250)).await;
            whole_body()
        };
        let answered = timed_search(
            Duration::from_millis(250),
            slow_body,
            Duration::from_millis(350),
        );
        assert_eq!(answered.status, 503);
    }

    #[test]
    fn the_time_allowed_by_default_is_five_seconds() {
        assert_eq!(REQUEST_TIMEOUT, Duration::from_secs(5));
        assert_eq!(ServeTimeouts::default().request, REQUEST_TIMEOUT);
    }

    /// The per-client limiter stays outside (D-STZ-19): well past any burst, on
    /// one set of handlers, every search is still answered.
    #[test]
    fn dispatch_itself_limits_no_rate() {
        let wired = serving();
        let past_the_burst = DEFAULT_RATE_BURST as usize * 2 + 1;
        let answers: Vec<u16> = (0..past_the_burst)
            .map(|_| post_search(&wired, search_body_of_len(64)).answer.status)
            .collect();
        assert_eq!(answers, vec![200; past_the_burst]);
    }
}
