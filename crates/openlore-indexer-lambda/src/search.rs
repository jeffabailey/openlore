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
use chrono::{DateTime, Utc};
use openlore_indexer::config::{IndexerConfig, TestFault};
use openlore_indexer::events::Events;
use openlore_indexer::run::search_answers;
use openlore_indexer::search_handler::{slow_index, unreadable_index, SharedIndexReads};
use ports::{
    HeadOutcome, IndexObjectError, IndexObjectReadPort, ProbeOutcome, ProbeRefusalReason,
    ProbeRefused,
};
use serde_json::{json, Map, Value};
use tokio::task::JoinError;
use tokio::time::{timeout, Instant};

use crate::decide::{freshness_due, reload_needed, HeadSeen, Reload, FRESHNESS_TTL};
use crate::index_holder::{HeldIndex, IndexHolder};
use crate::{events_through, refuse_start, Environment, EventSink, InitRefused, SearchDriven};

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
}

impl SearchFunction {
    /// Lambda init: pass the read probe, fetch the index (or open an empty one
    /// when there is no object) and get ready. Refused on a key naming the write
    /// probe's marker and on a refused read probe (D-STZ-39), on a store failure,
    /// and on an object that downloads but cannot be used (D-STZ-14). A missing
    /// object is not a refusal (D-STZ-13).
    pub async fn init(env: &Environment, driven: SearchDriven) -> Result<Self, InitRefused> {
        env.refuse_a_key_naming_the_marker(&driven.events)?;
        let config = env.indexer_config()?;
        run_read_probe(&driven.index_object, &driven.events).await?;
        let looked_at = driven.clock.now_utc();
        let index = HeldIndex::fetched(driven.index_object.as_ref(), &config)
            .await
            .map_err(|unusable| {
                refuse_start(&driven.events, READ_PORT, index_unusable(unusable.reason))
            })?;
        Ok(Self {
            events: events_through(Arc::clone(&driven.events)),
            driven,
            config,
            index: IndexHolder::holding(index),
            last_checked: Mutex::new(Some(looked_at)),
        })
    }

    /// Answer one Function URL event (payload format 2.0) with a Function URL
    /// response. The 5 s cover the freshness gate and the query together
    /// (D-STZ-10), so the search gets what the gate left.
    pub async fn handle(&self, event: Value) -> Value {
        let arrived = Instant::now();
        self.freshen().await;
        let handlers = handlers_over(&self.index.current(), &self.events, self.config.test_fault);
        let request = PublicRequest {
            method: text_at(&event, "/requestContext/http/method"),
            path: text_at(&event, "/rawPath"),
            body: std::future::ready(body_of(&event)),
            time_allowed: REQUEST_TIMEOUT.saturating_sub(arrived.elapsed()),
        };
        function_url_response(&dispatch(request, &handlers).await)
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
        let due = freshness_due(now, *last_checked, FRESHNESS_TTL);
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
        Ok(ProbeOutcome::Ok) => Ok(()),
        Ok(ProbeOutcome::Refused {
            reason,
            detail,
            structured,
        }) => Err(ProbeRefused {
            reason,
            detail,
            structured,
        }),
        Err(crashed) => Err(index_unusable(format!(
            "read probe gave no answer: {crashed}"
        ))),
    }
}

fn index_unusable(detail: String) -> ProbeRefused {
    ProbeRefused {
        reason: ProbeRefusalReason::IndexObjectUnusable,
        detail,
        structured: json!({}),
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

/// A Function URL event brings the whole body with it.
fn body_of(event: &Value) -> BodyRead {
    BodyRead::Complete(text_at(event, "/body").to_owned().into())
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

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::path::Path;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use async_trait::async_trait;
    use ports::{ClockPort, LastPass, ObjectVersion};
    use tempfile::TempDir;

    use super::*;

    /// A bucket whose read probe answers as told, and which holds `object`.
    struct ProbedBucket {
        probe: fn() -> ProbeOutcome,
        object: Option<Vec<u8>>,
        fetches: AtomicUsize,
    }

    impl ProbedBucket {
        fn answering(probe: fn() -> ProbeOutcome) -> Self {
            Self {
                probe,
                object: None,
                fetches: AtomicUsize::new(0),
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
            ]
            .map(|(name, value)| (name.to_string(), value)),
        ))
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
