//! Search mode (ADR-090 §3, §5): fetch the index object at init, then answer
//! through the shared transport-free `dispatch` (D-STZ-22), first bringing the
//! copy up to date when a freshness check is due (D-STZ-10). Read-only: it is
//! composed with the read port alone (D-STZ-38), so it cannot name a write
//! (check-arch `indexer_search_handler_read_only`, extended).

use std::sync::{Arc, Mutex, PoisonError};

use adapter_xrpc_query_server::{
    dispatch, BodyRead, Handlers, PublicAnswer, PublicRequest, REQUEST_TIMEOUT,
};
use chrono::{DateTime, Utc};
use openlore_indexer::config::IndexerConfig;
use openlore_indexer::events::Events;
use openlore_indexer::run::search_answers;
use ports::{HeadOutcome, IndexObjectError, IndexReadPort};
use serde_json::{json, Map, Value};

use crate::decide::{freshness_due, reload_needed, HeadSeen, Reload, FRESHNESS_TTL};
use crate::index_holder::{HeldIndex, IndexHolder};
use crate::{events_through, Environment, InitRefused, SearchDriven};

/// One warm search instance.
pub struct SearchFunction {
    driven: SearchDriven,
    config: IndexerConfig,
    events: Events,
    index: IndexHolder,
    last_checked: Mutex<Option<DateTime<Utc>>>,
}

impl SearchFunction {
    /// Lambda init: fetch the index (or open an empty one when there is no
    /// object) and get ready. Refused on a key naming the write probe's marker
    /// (D-STZ-39), and on any other store failure.
    pub async fn init(env: &Environment, driven: SearchDriven) -> Result<Self, InitRefused> {
        env.refuse_a_key_naming_the_marker(&driven.events)?;
        let config = env.indexer_config()?;
        let looked_at = driven.clock.now_utc();
        let index = HeldIndex::fetched(driven.index_object.as_ref(), &config).await?;
        Ok(Self {
            events: events_through(Arc::clone(&driven.events)),
            driven,
            config,
            index: IndexHolder::holding(index),
            last_checked: Mutex::new(Some(looked_at)),
        })
    }

    /// Answer one Function URL event (payload format 2.0) with a Function URL
    /// response.
    pub async fn handle(&self, event: Value) -> Value {
        self.freshen().await;
        let handlers = handlers_over(&self.index.current(), &self.events);
        let request = PublicRequest {
            method: text_at(&event, "/requestContext/http/method"),
            path: text_at(&event, "/rawPath"),
            body: std::future::ready(body_of(&event)),
            time_allowed: REQUEST_TIMEOUT,
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
            Reload::Fetch(_) | Reload::OpenEmpty => self.reload(&served).await,
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
    /// (D-STZ-13), and swap. On failure the served copy stays.
    async fn reload(&self, served: &HeldIndex) {
        let index_object = self.driven.index_object.as_ref();
        match served.successor(index_object, &self.config).await {
            Ok(next) => self.index.swap(next),
            Err(_) => self.report_failed_check(),
        }
    }

    fn report_failed_check(&self) {
        self.events
            .emit(&json!({"event": "indexer.lambda.freshness_check_failed"}));
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
fn handlers_over(index: &HeldIndex, events: &Events) -> Handlers {
    let reads: Arc<dyn IndexReadPort + Send + Sync> = index.reads();
    Handlers {
        query: search_answers(reads, events),
        health: Some(index.health()),
    }
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
