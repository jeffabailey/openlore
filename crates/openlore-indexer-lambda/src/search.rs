//! Search mode (ADR-090 §3, §5): fetch the index object at init, then answer
//! through the shared transport-free `dispatch` (D-STZ-22), first bringing the
//! copy up to date when a freshness check is due (D-STZ-10). Read-only: it is
//! composed with the read port alone (D-STZ-38), so it cannot name a write
//! (check-arch `indexer_search_handler_read_only`, extended).

use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use adapter_xrpc_query_server::{
    dispatch, BodyRead, Handlers, PublicAnswer, PublicRequest, QueryHandler, REQUEST_TIMEOUT,
};
use base64::prelude::{Engine, BASE64_STANDARD};
use chrono::{DateTime, Utc};
use lambda_http::http::StatusCode;
use lambda_http::{Body, Request, Response};
use openlore_indexer::config::{IndexerConfig, TestFault};
use openlore_indexer::events::Events;
use openlore_indexer::run::search_answers;
use openlore_indexer::search_handler::{slow_index, unreadable_index, SharedIndexReads};
use ports::{HeadOutcome, IndexObjectError, IndexObjectReadPort, ProbeOutcome, ProbeRefused};
use serde_json::{json, Map, Value};
use tokio::task::JoinError;
use tokio::time::{timeout, Instant};

use crate::config::SearchSettings;
use crate::decide::{freshness_due, reload_needed, HeadSeen, Reload};
use crate::index_holder::{HeldIndex, IndexHolder};
use crate::{
    events_through, refuse_config, refuse_start, Environment, EventSink, InitRefused, SearchDriven,
};

/// The bound on one reload, the download and the open together (D-STZ-10). It is
/// the root's: no adapter sees both.
const RELOAD_BOUND: Duration = Duration::from_secs(3);

/// The port a refused start names (`health.startup.refused.adapter`).
const READ_PORT: &str = "index_object_read";

/// One warm search instance.
pub struct SearchFunction {
    driven: SearchDriven,
    config: IndexerConfig,
    events: Events,
    index: IndexHolder,
    last_checked: Mutex<Option<DateTime<Utc>>>,
    freshness_ttl: Duration,
}

impl SearchFunction {
    /// Lambda init: pass the read probe, fetch the index (or open an empty one
    /// when there is no object) and get ready. Refused on a key naming the write
    /// probe's marker and on a refused read probe (D-STZ-39), on a store failure,
    /// and on an object that downloads but cannot be used (D-STZ-14). A missing
    /// object is not a refusal (D-STZ-13).
    pub async fn init(env: &Environment, driven: SearchDriven) -> Result<Self, InitRefused> {
        env.refuse_a_key_naming_the_marker(&driven.events)?;
        let config = env.indexer_config(&driven.events)?;
        let settings =
            SearchSettings::read(env).map_err(|refused| refuse_config(&driven.events, refused))?;
        run_read_probe(&driven.index_object, &driven.events).await?;
        let looked_at = driven.clock.now_utc();
        let index = HeldIndex::fetched(driven.index_object.as_ref(), &config)
            .await
            .map_err(|unusable| {
                let refused = ProbeRefused::index_object_unusable(unusable.reason);
                refuse_start(&driven.events, READ_PORT, refused)
            })?;
        Ok(Self {
            events: events_through(Arc::clone(&driven.events)),
            driven,
            config,
            index: IndexHolder::holding(index),
            last_checked: Mutex::new(Some(looked_at)),
            freshness_ttl: settings.freshness_ttl,
        })
    }

    /// Answer one Function URL event (payload format 2.0) with a Function URL
    /// response.
    pub async fn handle(&self, event: Value) -> Value {
        let arrived = Instant::now();
        let method = text_at(&event, "/requestContext/http/method");
        let path = text_at(&event, "/rawPath");
        let answer = self.answer(method, path, body_of(&event), arrived).await;
        function_url_response(&answer)
    }

    /// Answer the same event as `lambda_http` hands it over, its body decoded.
    pub async fn handle_request(&self, request: Request) -> Response<Body> {
        let arrived = Instant::now();
        let body = BodyRead::Complete(request.body().to_vec().into());
        let (method, path) = (request.method().as_str(), request.uri().path());
        http_response(&self.answer(method, path, body, arrived).await)
    }

    /// The 5 s cover the freshness gate and the query together (D-STZ-10), so
    /// the search gets what the gate left.
    async fn answer(
        &self,
        method: &str,
        path: &str,
        body: BodyRead,
        arrived: Instant,
    ) -> PublicAnswer {
        self.freshen().await;
        let handlers = handlers_over(&self.index.current(), &self.events, self.config.test_fault);
        let request = PublicRequest {
            method,
            path,
            body: std::future::ready(body),
            time_allowed: REQUEST_TIMEOUT.saturating_sub(arrived.elapsed()),
        };
        dispatch(request, &handlers).await
    }

    /// The freshness gate (D-STZ-10): when a check is due, look at the object and
    /// act on what was seen. It runs inline, because Lambda gives an instance one
    /// request at a time. A failure never reaches the search (D-STZ-11).
    async fn freshen(&self) {
        if !self.check_is_due() {
            return;
        }
        let served = self.index.current();
        let seen = head_seen(self.driven.index_object.head().await);
        match reload_needed(served.etag(), &seen) {
            Reload::Keep => {}
            Reload::KeepAndReport => self.report_failed_check(),
            Reload::Fetch(_) | Reload::OpenEmpty => self.reload().await,
        }
    }

    /// Whether this request makes the check. Asking starts the next TTL, so a
    /// failed check waits for it like any other.
    fn check_is_due(&self) -> bool {
        let now = self.driven.clock.now_utc();
        let mut last_checked = self
            .last_checked
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let due = freshness_due(now, *last_checked, self.freshness_ttl);
        if due {
            *last_checked = Some(now);
        }
        due
    }

    /// Bring in the object as it is now, which is empty when it has gone
    /// (D-STZ-13), and swap. On failure or at the bound the served copy stays,
    /// and dropping the reload gives up what it had fetched (`index_holder`).
    async fn reload(&self) {
        let index_object = self.driven.index_object.as_ref();
        let reload = self.index.next_copy(index_object, &self.config);
        match timeout(RELOAD_BOUND, reload).await {
            Ok(Ok(next)) => self.index.swap(next),
            Ok(Err(_)) | Err(_) => self.report_failed_check(),
        }
    }

    fn report_failed_check(&self) {
        self.events
            .emit(&json!({"event": "indexer.lambda.freshness_check_failed"}));
    }
}

/// The read probe, off the async runtime: it is sync and blocks, and blocking on
/// the runtime from async init panics (D-STZ-39, UI-11).
pub(crate) async fn run_read_probe(
    index_object: &Arc<dyn IndexObjectReadPort>,
    events: &Arc<dyn EventSink>,
) -> Result<(), InitRefused> {
    let index_object = Arc::clone(index_object);
    let answer = tokio::task::spawn_blocking(move || index_object.probe()).await;
    read_probe_verdict(answer).map_err(|refused| refuse_start(events, READ_PORT, refused))
}

/// D-STZ-39: search and probe init gate on this probe, so an instance starts
/// only on its `Ok`. A probe that never answered proved nothing.
fn read_probe_verdict(answer: Result<ProbeOutcome, JoinError>) -> Result<(), ProbeRefused> {
    match answer {
        Ok(outcome) => outcome.into_result(),
        Err(crashed) => Err(ProbeRefused::index_object_unusable(format!(
            "read probe gave no answer: {crashed}"
        ))),
    }
}

fn head_seen(head: Result<HeadOutcome, IndexObjectError>) -> HeadSeen {
    match head {
        Ok(HeadOutcome::Present(version)) => HeadSeen::Present(version.etag),
        Ok(HeadOutcome::Absent) => HeadSeen::Absent,
        Err(_) => HeadSeen::Failed,
    }
}

/// The public surface over one copy, composed per request so the next answer
/// comes from the copy a swap left in place, and a retired copy is held by
/// nothing. A search holds `IndexReadPort` and nothing that writes.
fn handlers_over(index: &Arc<HeldIndex>, events: &Events, fault: Option<TestFault>) -> Handlers {
    let answers = search_answers(reads_under(fault, index.reads()), events);
    Handlers {
        query: answers_under(fault, answers, index),
        health: Some(index.health()),
    }
}

/// The TEST-ONLY read faults (debug builds only; config refuses the seam in a
/// release build). They act here, per request, so init reads a healthy index.
fn reads_under(fault: Option<TestFault>, reads: SharedIndexReads) -> SharedIndexReads {
    match fault {
        Some(TestFault::SearchStoreReadFails) => unreadable_index(),
        Some(TestFault::SearchStoreSlow) => slow_index(reads),
        _ => reads,
    }
}

/// The TEST-ONLY running-store fault: a health read leaves the store alone, and
/// the search that answers poisons it.
fn answers_under(
    fault: Option<TestFault>,
    answers: QueryHandler,
    index: &Arc<HeldIndex>,
) -> QueryHandler {
    if fault != Some(TestFault::StorePoisonedAfterFirstSearch) {
        return answers;
    }
    let index = Arc::clone(index);
    Arc::new(move |request| {
        let answer = answers(request);
        index.poison_for_test_fault();
        answer
    })
}

fn text_at<'event>(event: &'event Value, pointer: &str) -> &'event str {
    event
        .pointer(pointer)
        .and_then(Value::as_str)
        .unwrap_or_default()
}

/// A Function URL event brings the whole body with it, as base64 when the URL
/// says so. Text that does not decode is a request nobody can read.
fn body_of(event: &Value) -> BodyRead {
    let body = text_at(event, "/body");
    if event.pointer("/isBase64Encoded") != Some(&Value::Bool(true)) {
        return BodyRead::Complete(body.to_owned().into());
    }
    BASE64_STANDARD.decode(body).map_or_else(
        |_| BodyRead::Unreadable("the body is not valid base64".to_string()),
        |decoded| BodyRead::Complete(decoded.into()),
    )
}

fn function_url_response(answer: &PublicAnswer) -> Value {
    let headers: Map<String, Value> = answer
        .headers
        .iter()
        .map(|(name, value)| ((*name).to_string(), json!(value)))
        .collect();
    json!({
        "statusCode": answer.status,
        "headers": headers,
        "body": String::from_utf8_lossy(&answer.body),
        "isBase64Encoded": false,
    })
}

/// The same answer as a response `lambda_http` can turn into the Function URL's.
fn http_response(answer: &PublicAnswer) -> Response<Body> {
    let answered = answer.headers.iter().fold(
        Response::builder().status(answer.status),
        |response, (name, value)| response.header(*name, *value),
    );
    let body = String::from_utf8_lossy(&answer.body).into_owned();
    answered
        .body(Body::Text(body))
        .unwrap_or_else(|_| unbuilt_response())
}

/// Dispatch's status and headers are fixed text, so this is never reached by
/// them, and it must still answer rather than panic.
fn unbuilt_response() -> Response<Body> {
    let mut response = Response::new(Body::Empty);
    *response.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;
    response
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::path::Path;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use async_trait::async_trait;
    use ports::{ClockPort, LastPass, ObjectVersion, ProbeRefusalReason};
    use proptest::prelude::*;
    use tempfile::TempDir;

    use super::*;
    use crate::config::FRESHNESS_TTL_VAR;

    /// A bucket whose read probe answers as told, and which holds `object`.
    struct ProbedBucket {
        probe: fn() -> ProbeOutcome,
        object: Option<Vec<u8>>,
        fetches: AtomicUsize,
        heads: AtomicUsize,
    }

    impl ProbedBucket {
        fn answering(probe: fn() -> ProbeOutcome) -> Self {
            Self {
                probe,
                object: None,
                fetches: AtomicUsize::new(0),
                heads: AtomicUsize::new(0),
            }
        }

        fn holding(object: &[u8]) -> Self {
            Self {
                object: Some(object.to_vec()),
                ..Self::answering(|| ProbeOutcome::Ok)
            }
        }
    }

    #[async_trait]
    impl IndexObjectReadPort for ProbedBucket {
        fn probe(&self) -> ProbeOutcome {
            (self.probe)()
        }

        async fn head(&self) -> Result<HeadOutcome, IndexObjectError> {
            self.heads.fetch_add(1, Ordering::Relaxed);
            Ok(HeadOutcome::Absent)
        }

        async fn fetch_to(&self, dest: &Path) -> Result<Option<ObjectVersion>, IndexObjectError> {
            self.fetches.fetch_add(1, Ordering::Relaxed);
            let Some(object) = &self.object else {
                return Ok(None);
            };
            std::fs::write(dest, object).expect("a scratch file");
            Ok(Some(ObjectVersion {
                etag: "etag".to_string(),
                published_at: None,
                pass_id: None,
                size_bytes: object.len() as u64,
            }))
        }

        async fn get_last_pass(&self) -> Result<Option<LastPass>, IndexObjectError> {
            Ok(None)
        }
    }

    struct StoppedClock;

    impl ClockPort for StoppedClock {
        fn probe(&self) -> ProbeOutcome {
            ProbeOutcome::Ok
        }

        fn now_utc(&self) -> DateTime<Utc> {
            DateTime::UNIX_EPOCH
        }
    }

    #[derive(Default)]
    struct RecordedLines(Mutex<Vec<String>>);

    impl EventSink for RecordedLines {
        fn line(&self, line: &str) {
            self.0.lock().expect("the lines").push(line.to_string());
        }
    }

    impl RecordedLines {
        fn refusals(&self) -> usize {
            let lines = self.0.lock().expect("the lines");
            lines
                .iter()
                .filter(|line| line.contains("health.startup.refused"))
                .count()
        }
    }

    fn environment(scratch: &TempDir) -> Environment {
        let path = |name: &str| scratch.path().join(name).display().to_string();
        Environment(BTreeMap::from(
            [
                ("OPENLORE_INDEXER_INDEX_PATH", path("index.duckdb")),
                ("OPENLORE_INDEXER_REPO_DIDS_FILE", path("repo-dids")),
                ("OPENLORE_INDEXER_DUCKDB_MEMORY_LIMIT_MB", "48".to_string()),
                ("OPENLORE_INDEXER_DUCKDB_THREADS", "1".to_string()),
                (FRESHNESS_TTL_VAR, "60".to_string()),
            ]
            .map(|(name, value)| (name.to_string(), value)),
        ))
    }

    /// A clock a test moves by hand.
    struct MovingClock(Mutex<DateTime<Utc>>);

    impl MovingClock {
        fn move_on(&self, seconds: i64) {
            let mut now = self.0.lock().expect("the time");
            *now += chrono::Duration::seconds(seconds);
        }
    }

    impl ClockPort for MovingClock {
        fn probe(&self) -> ProbeOutcome {
            ProbeOutcome::Ok
        }

        fn now_utc(&self) -> DateTime<Utc> {
            *self.0.lock().expect("the time")
        }
    }

    const SEARCH_PATH: &str = "/xrpc/org.openlore.appview.searchClaims";
    const A_SEARCH: &str = r#"{"dimension":"subject","value":"https://example.invalid/a"}"#;

    /// A Function URL event, as the URL sends it (payload format 2.0).
    fn function_url_event(method: &str, path: &str, body: &str, encoded: bool) -> Value {
        json!({
            "version": "2.0",
            "routeKey": "$default",
            "rawPath": path,
            "rawQueryString": "",
            "headers": {"host": "abc.lambda-url.us-east-1.on.aws"},
            "requestContext": {
                "accountId": "anonymous",
                "apiId": "abc",
                "domainName": "abc.lambda-url.us-east-1.on.aws",
                "domainPrefix": "abc",
                "http": {
                    "method": method,
                    "path": path,
                    "protocol": "HTTP/1.1",
                    "sourceIp": "203.0.113.1",
                    "userAgent": "test",
                },
                "requestId": "id",
                "routeKey": "$default",
                "stage": "$default",
                "time": "27/Oct/2026:14:00:00 +0000",
                "timeEpoch": 0,
            },
            "body": body,
            "isBase64Encoded": encoded,
        })
    }

    fn lambda_http_request(event: &Value) -> Request {
        lambda_http::request::from_str(&event.to_string()).expect("a Function URL event")
    }

    async fn started_on_an_empty_bucket(scratch: &TempDir) -> SearchFunction {
        let driven = SearchDriven {
            index_object: Arc::new(ProbedBucket::answering(|| ProbeOutcome::Ok)),
            clock: Arc::new(StoppedClock),
            events: Arc::new(RecordedLines::default()),
        };
        SearchFunction::init(&environment(scratch), driven)
            .await
            .unwrap_or_else(|refused| panic!("init: {refused:?}"))
    }

    proptest! {
        // Any text, plain or encoded, is the same body to the search.
        #[test]
        fn an_encoded_body_reads_as_the_text_it_encodes(text in "\\PC*") {
            let plain = json!({"body": text});
            let encoded = json!({
                "body": BASE64_STANDARD.encode(&text),
                "isBase64Encoded": true,
            });

            prop_assert_eq!(body_of(&encoded), body_of(&plain));
        }

        // Not base64: the search must hear "unreadable", never read the text.
        #[test]
        fn a_body_that_is_not_base64_is_unreadable(text in "[a-z]{1,8}[*!]") {
            let event = json!({"body": text, "isBase64Encoded": true});

            prop_assert!(matches!(body_of(&event), BodyRead::Unreadable(_)));
        }
    }

    // bypass: one fixed event shape on both roads into the function; the body's
    // space is the property above. This pins that lambda_http decodes for us.
    #[tokio::test]
    async fn an_encoded_search_is_answered_alike_on_both_roads_in() {
        let scratch = tempfile::tempdir().expect("a scratch directory");
        let function = started_on_an_empty_bucket(&scratch).await;
        let event =
            function_url_event("POST", SEARCH_PATH, &BASE64_STANDARD.encode(A_SEARCH), true);
        let request = lambda_http_request(&event);
        assert_eq!(request.body().as_ref(), A_SEARCH.as_bytes());

        let through_the_harness = function.handle(event).await;
        let through_lambda_http = function.handle_request(request).await;

        assert_eq!(through_the_harness["statusCode"], 200);
        assert_eq!(through_lambda_http.status().as_u16(), 200);
        assert_eq!(
            through_the_harness["body"]
                .as_str()
                .expect("a text body")
                .as_bytes(),
            through_lambda_http.body().as_ref(),
        );
    }

    // bypass: the path and status mapping of one fixed route, next to the one
    // above; every route's own answer is dispatch's and has its own suite.
    #[tokio::test]
    async fn a_route_nobody_serves_is_a_404_through_lambda_http_too() {
        let scratch = tempfile::tempdir().expect("a scratch directory");
        let function = started_on_an_empty_bucket(&scratch).await;
        let event = function_url_event("GET", "/xrpc/x", "", false);

        let answer = function.handle_request(lambda_http_request(&event)).await;

        assert_eq!(answer.status().as_u16(), 404);
    }

    // bypass: the rule over every time and TTL is `freshness_due`'s own property
    // in the core suite; this pins that the deployed TTL is the one that reaches it.
    #[tokio::test]
    async fn the_deployed_ttl_decides_when_a_warm_search_looks_at_the_object() {
        let scratch = tempfile::tempdir().expect("a scratch directory");
        let bucket = Arc::new(ProbedBucket::answering(|| ProbeOutcome::Ok));
        let clock = Arc::new(MovingClock(Mutex::new(DateTime::UNIX_EPOCH)));
        let mut env = environment(&scratch);
        env.0
            .insert(FRESHNESS_TTL_VAR.to_string(), "300".to_string());
        let driven = SearchDriven {
            index_object: Arc::clone(&bucket) as Arc<dyn IndexObjectReadPort>,
            clock: Arc::clone(&clock) as Arc<dyn ClockPort + Send + Sync>,
            events: Arc::new(RecordedLines::default()),
        };
        let function = SearchFunction::init(&env, driven)
            .await
            .unwrap_or_else(|refused| panic!("init: {refused:?}"));

        clock.move_on(299);
        function.handle(json!({})).await;
        assert_eq!(bucket.heads.load(Ordering::Relaxed), 0, "inside the TTL");

        clock.move_on(1);
        function.handle(json!({})).await;
        assert_eq!(bucket.heads.load(Ordering::Relaxed), 1, "at the TTL");
    }

    /// One start against `bucket`, and how many refusals it logged.
    async fn start(bucket: &Arc<ProbedBucket>) -> (Result<SearchFunction, InitRefused>, usize) {
        let scratch = tempfile::tempdir().expect("a scratch directory");
        let lines = Arc::new(RecordedLines::default());
        let driven = SearchDriven {
            index_object: Arc::clone(bucket) as Arc<dyn IndexObjectReadPort>,
            clock: Arc::new(StoppedClock),
            events: Arc::clone(&lines) as Arc<dyn EventSink>,
        };
        let started = SearchFunction::init(&environment(&scratch), driven).await;
        (started, lines.refusals())
    }

    fn refused_probe() -> ProbeOutcome {
        ProbeOutcome::Refused {
            reason: ProbeRefusalReason::IndexObjectUnusable,
            detail: "the role cannot HEAD the object".to_string(),
            structured: json!({}),
        }
    }

    /// Blocks on the runtime the way a sync adapter's probe does, which panics
    /// on a thread that is running async work.
    fn probe_that_blocks_on_the_runtime() -> ProbeOutcome {
        tokio::runtime::Handle::current().block_on(async { ProbeOutcome::Ok })
    }

    // bypass: the probe's answer is a fixed case, and the contract is that init
    // goes no further; it has no input space.
    #[tokio::test]
    async fn a_refused_read_probe_stops_init_before_it_fetches_anything() {
        let bucket = Arc::new(ProbedBucket::answering(refused_probe));

        let (started, refusals) = start(&bucket).await;

        assert!(started.is_err(), "init must be refused");
        assert_eq!(refusals, 1);
        assert_eq!(bucket.fetches.load(Ordering::Relaxed), 0);
    }

    // bypass: one crash is one fixed case.
    #[tokio::test]
    async fn a_read_probe_that_crashes_proves_nothing_so_init_is_refused() {
        let bucket = Arc::new(ProbedBucket::answering(|| panic!("the probe crashed")));

        let (started, refusals) = start(&bucket).await;

        assert!(started.is_err(), "init must be refused");
        assert_eq!(refusals, 1);
    }

    // bypass: UI-11 is about which thread the probe runs on; it has no input space.
    #[tokio::test]
    async fn a_read_probe_that_blocks_runs_off_the_runtime_and_init_goes_on() {
        let bucket = Arc::new(ProbedBucket::answering(probe_that_blocks_on_the_runtime));

        let (started, refusals) = start(&bucket).await;

        assert!(started.is_ok(), "the probe passed");
        assert_eq!(refusals, 0);
    }

    // bypass: no bytes is the one object a store could take for a new file, so
    // it is the fixed case that would serve a damaged object as empty.
    #[tokio::test]
    async fn an_object_of_no_bytes_is_refused_and_not_served_as_empty() {
        let bucket = Arc::new(ProbedBucket::holding(b""));

        let (started, refusals) = start(&bucket).await;

        assert!(started.is_err(), "init must be refused");
        assert_eq!(refusals, 1);
    }
}
