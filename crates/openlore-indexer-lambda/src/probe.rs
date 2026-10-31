//! Probe mode (D-STZ-23, U-STZ-3, U-STZ-6, U-STZ-7): the canned search and then
//! `GET /healthz` through the public edge (`adapter-index-query`, D-STZ-37), the
//! age of `last-pass.json`, then exactly one `indexer.host.health` line (A3).
//! When the served index lags the record it also writes
//! `indexer.lambda.served_index_stale` (A10), which never changes `not_live`.
//! Read-only: it is composed with the read port alone (D-STZ-38).

use std::sync::Arc;
use std::time::Duration;

use adapter_index_query::HttpIndexQueryAdapter;
use chrono::{DateTime, Utc};
use openlore_indexer::events::Events;
use ports::{HealthBody, IndexQueryPort, LastPass, SearchDimension};
use serde_json::{json, Value};
use tokio::time::Instant;

use crate::decide::{
    not_live, served_index_stale, ProbeFacts, ServedIndex, LAST_PASS_STALE_AFTER,
    SERVED_INDEX_MARGIN,
};
use crate::search::run_read_probe;
use crate::{events_through, Environment, FunctionError, InitRefused, ProbeDriven};

/// The public edge the probe calls, as the function is configured with it.
const PROBE_BASE_URL_VAR: &str = "OPENLORE_INDEXER_LAMBDA_PROBE_BASE_URL";

/// The canned search health-timer.sh sends: no user data, a value matching nothing.
const CANNED_SEARCH_DIMENSION: SearchDimension = SearchDimension::Subject;
const CANNED_SEARCH_VALUE: &str = "https://openlore.invalid/health-probe";

/// The status a line reports for a search nothing answered (health-timer.sh's 0).
const NO_ANSWER_STATUS: u16 = 0;

/// One probe instance.
pub struct ProbeFunction {
    driven: ProbeDriven,
    edge: HttpIndexQueryAdapter,
    events: Events,
}

impl ProbeFunction {
    /// Lambda init. Refused on a key naming the write probe's marker and on a
    /// refused read probe (D-STZ-39).
    pub async fn init(env: &Environment, driven: ProbeDriven) -> Result<Self, InitRefused> {
        env.refuse_a_key_naming_the_marker(&driven.events)?;
        run_read_probe(&driven.index_object, &driven.events).await?;
        Ok(Self {
            edge: HttpIndexQueryAdapter::for_url(env.text_of(PROBE_BASE_URL_VAR)),
            events: events_through(Arc::clone(&driven.events)),
            driven,
        })
    }

    /// Run one probe for one event. An event that is not a JSON object is
    /// refused before anything else happens (U-STZ-7). The answer is the line.
    pub async fn handle(&self, event: Value) -> Result<Value, FunctionError> {
        refuse_a_non_object_event(&event)?;
        let search = self.canned_search().await;
        let health = passed_health(self.edge.health().await);
        let last_pass = self.last_pass().await;
        let now = self.driven.clock.now_utc();
        let line = health_line(&search, health.is_some(), last_pass.as_ref(), now);
        self.events.emit(&line);
        if let Some(stale) = stale_index_line(health.as_ref(), last_pass.as_ref(), now) {
            self.events.emit(&stale);
        }
        Ok(line)
    }

    async fn canned_search(&self) -> CannedSearch {
        let sent = Instant::now();
        let (status, result) = self
            .edge
            .search_with_status(CANNED_SEARCH_DIMENSION, CANNED_SEARCH_VALUE, None)
            .await;
        CannedSearch {
            ok: result.is_ok(),
            status,
            took: sent.elapsed(),
        }
    }

    /// The record, when there is one that could be read (UI-12).
    async fn last_pass(&self) -> Option<LastPass> {
        self.driven
            .index_object
            .get_last_pass()
            .await
            .ok()
            .flatten()
    }
}

/// U-STZ-7: a function error, with no health line and no calls. `{}` runs a
/// normal probe. `lambda test-alarm errors` relies on this for a real, harmless
/// `Errors` datapoint.
fn refuse_a_non_object_event(event: &Value) -> Result<(), FunctionError> {
    if event.is_object() {
        return Ok(());
    }
    Err(FunctionError {
        message: format!("the probe takes a JSON object, not {event}"),
    })
}

/// As health-timer.sh: `"status":"ok"` in a 200 (`health` makes a 503 an error).
fn says_ok(health: &HealthBody) -> bool {
    health.status == "ok"
}

/// The health answer, when the check passed; a failed one says nothing about
/// what is served (D-STZ-48).
fn passed_health<E>(answer: Result<HealthBody, E>) -> Option<HealthBody> {
    answer.ok().filter(says_ok)
}

/// How the canned search went.
struct CannedSearch {
    ok: bool,
    status: Option<u16>,
    took: Duration,
}

/// The `indexer.host.health` line (DEVOPS § Monitoring contracts) for what one
/// run saw at `now`.
fn health_line(
    search: &CannedSearch,
    healthz_ok: bool,
    last_pass: Option<&LastPass>,
    now: DateTime<Utc>,
) -> Value {
    let last_pass_age = last_pass.and_then(|record| age_of(record, now));
    let facts = ProbeFacts {
        healthz_ok,
        search_ok: search.ok,
        last_pass_age,
    };
    let liveness = not_live(&facts, LAST_PASS_STALE_AFTER);
    json!({
        "event": "indexer.host.health",
        "source": "lambda-probe",
        "healthz_ok": healthz_ok,
        "search_ok": search.ok,
        "search_ms": u64::try_from(search.took.as_millis()).unwrap_or(u64::MAX),
        "search_status": search.status.unwrap_or(NO_ANSWER_STATUS),
        "last_pass_age_s": last_pass_age.map(|age| age.as_secs()),
        "last_pass_exit_code": last_pass.map(|record| record.exit_code),
        "not_live": u8::from(!liveness.causes.is_empty()),
    })
}

/// What a passed health check says is served.
fn served_index(health: Option<&HealthBody>) -> ServedIndex<'_> {
    match health.map(|answer| answer.last_successful_pass_at.as_deref()) {
        None => ServedIndex::Unknown,
        Some(None) => ServedIndex::NothingPublished,
        Some(Some(published_at)) => ServedIndex::PublishedAt(published_at),
    }
}

/// The `indexer.lambda.served_index_stale` line (D-STZ-48, alarm A10), when the
/// served index lags the record at `now`. `lag_s` is null when nothing is served.
fn stale_index_line(
    health: Option<&HealthBody>,
    last_pass: Option<&LastPass>,
    now: DateTime<Utc>,
) -> Option<Value> {
    let recorded = last_pass.and_then(|record| record.index_published_at.as_deref());
    let record_age = last_pass.and_then(|record| age_of(record, now));
    let stale = served_index_stale(
        served_index(health),
        recorded,
        record_age,
        SERVED_INDEX_MARGIN,
    )?;
    Some(json!({
        "event": "indexer.lambda.served_index_stale",
        "served": health.and_then(|answer| answer.last_successful_pass_at.as_deref()),
        "recorded": recorded,
        "lag_s": stale.lag.map(|lag| lag.as_secs()),
    }))
}

/// How long ago the recorded pass finished; `None` for a time nobody can read,
/// or one still to come, which counts as no recent pass (UI-12).
fn age_of(record: &LastPass, now: DateTime<Utc>) -> Option<Duration> {
    let finished_at = DateTime::parse_from_rfc3339(&record.finished_at).ok()?;
    now.signed_duration_since(finished_at).to_std().ok()
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::sync::Mutex;

    use async_trait::async_trait;
    use ports::{
        ClockPort, HeadOutcome, IndexObjectError, IndexObjectReadPort, ObjectVersion, ProbeOutcome,
        ProbeRefusalReason,
    };
    use proptest::prelude::*;

    use super::*;
    use crate::EventSink;

    /// A bucket whose read probe refuses, and which nothing may ask anything.
    struct RefusingBucket;

    #[async_trait]
    impl IndexObjectReadPort for RefusingBucket {
        fn probe(&self) -> ProbeOutcome {
            ProbeOutcome::Refused {
                reason: ProbeRefusalReason::IndexObjectUnusable,
                detail: "the role cannot HEAD the object".to_string(),
                structured: json!({}),
            }
        }

        async fn head(&self) -> Result<HeadOutcome, IndexObjectError> {
            panic!("init asked the store after a refused probe")
        }

        async fn fetch_to(&self, _dest: &Path) -> Result<Option<ObjectVersion>, IndexObjectError> {
            panic!("init asked the store after a refused probe")
        }

        async fn get_last_pass(&self) -> Result<Option<LastPass>, IndexObjectError> {
            panic!("init asked the store after a refused probe")
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

    // bypass: the probe's answer is a fixed case, and the contract is that the
    // instance never starts; it has no input space.
    #[tokio::test]
    async fn a_refused_read_probe_means_the_probe_never_starts() {
        let lines = Arc::new(RecordedLines::default());
        let driven = ProbeDriven {
            index_object: Arc::new(RefusingBucket),
            clock: Arc::new(StoppedClock),
            events: Arc::clone(&lines) as Arc<dyn EventSink>,
        };

        let started = ProbeFunction::init(&Environment::default(), driven).await;

        assert!(started.is_err(), "init must be refused");
        let lines = lines.0.lock().expect("the lines");
        assert_eq!(lines.len(), 1, "{lines:?}");
        assert!(lines[0].contains("health.startup.refused"), "{lines:?}");
    }

    // bypass: the two sides of one string comparison, both fixed by the wire.
    #[test]
    fn only_a_status_of_ok_is_a_passed_health_check() {
        let answering = |status: &str| HealthBody {
            status: status.to_string(),
            last_successful_pass_at: None,
        };
        assert!(says_ok(&answering("ok")));
        assert!(!says_ok(&answering("store_unusable")));
    }

    // bypass: the three shapes of one answer; the stale line itself is a property below.
    #[test]
    fn only_a_passed_health_check_says_what_is_served() {
        let answering = |status: &str| HealthBody {
            status: status.to_string(),
            last_successful_pass_at: Some("2026-10-27T13:45:01Z".to_string()),
        };
        assert_eq!(
            passed_health(Ok::<_, ()>(answering("ok"))),
            Some(answering("ok"))
        );
        assert_eq!(
            passed_health(Ok::<_, ()>(answering("store_unusable"))),
            None
        );
        assert_eq!(passed_health(Err::<HealthBody, _>(())), None);
    }

    fn record_finished(seconds_before: i64, now: DateTime<Utc>, exit_code: i32) -> LastPass {
        LastPass {
            pass_id: "a-pass".to_string(),
            exit_code,
            finished_at: (now - chrono::Duration::seconds(seconds_before)).to_rfc3339(),
            published: false,
            index_published_at: None,
        }
    }

    proptest! {
        // The whole line, compared whole, so no field can move on its own.
        #[test]
        fn the_line_says_what_the_run_saw_and_is_live_only_when_all_of_it_is_good(
            healthz_ok in any::<bool>(),
            search_ok in any::<bool>(),
            status in proptest::option::of(100_u16..600),
            search_ms in 0_u64..30_000,
            age_s in 0_i64..10_000,
            exit_code in 0_i32..4,
        ) {
            let now = DateTime::UNIX_EPOCH + chrono::Duration::days(20_000);
            let search = CannedSearch {
                ok: search_ok,
                status,
                took: Duration::from_millis(search_ms),
            };
            let record = record_finished(age_s, now, exit_code);

            let line = health_line(&search, healthz_ok, Some(&record), now);

            let live = healthz_ok && search_ok && age_s <= 45 * 60;
            prop_assert_eq!(line, json!({
                "event": "indexer.host.health",
                "source": "lambda-probe",
                "healthz_ok": healthz_ok,
                "search_ok": search_ok,
                "search_ms": search_ms,
                "search_status": status.unwrap_or(0),
                "last_pass_age_s": age_s,
                "last_pass_exit_code": exit_code,
                "not_live": i32::from(!live),
            }));
        }
    }

    proptest! {
        // The whole line, compared whole: a settled record and a served index
        // behind it, by any lag, or none served at all.
        #[test]
        fn the_stale_line_names_both_times_and_the_lag_between_them(
            lag_s in proptest::option::of(1_i64..100_000),
            record_age_s in 121_i64..10_000,
        ) {
            let now = DateTime::UNIX_EPOCH + chrono::Duration::days(20_000);
            let recorded_at = now - chrono::Duration::days(1);
            let recorded = recorded_at.to_rfc3339();
            let served = lag_s.map(|lag| (recorded_at - chrono::Duration::seconds(lag)).to_rfc3339());
            let health = HealthBody {
                status: "ok".to_string(),
                last_successful_pass_at: served.clone(),
            };
            let record = LastPass {
                index_published_at: Some(recorded.clone()),
                ..record_finished(record_age_s, now, 0)
            };

            let line = stale_index_line(Some(&health), Some(&record), now);

            prop_assert_eq!(line, Some(json!({
                "event": "indexer.lambda.served_index_stale",
                "served": served,
                "recorded": recorded,
                "lag_s": lag_s,
            })));
        }
    }

    // bypass: clock skew is one fixed case; a future time is not an age.
    #[test]
    fn a_record_from_the_future_counts_as_no_recent_pass() {
        let now = DateTime::UNIX_EPOCH + chrono::Duration::days(20_000);
        let search = CannedSearch {
            ok: true,
            status: Some(200),
            took: Duration::ZERO,
        };
        let record = record_finished(-1, now, 0);

        let line = health_line(&search, true, Some(&record), now);

        assert_eq!(line["last_pass_age_s"], Value::Null, "{line}");
        assert_eq!(line["not_live"], 1, "{line}");
    }
}
