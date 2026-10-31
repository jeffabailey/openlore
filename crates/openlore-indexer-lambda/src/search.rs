//! Search mode (ADR-090 §3, §5): fetch the index object at init, then answer
//! through the shared transport-free `dispatch` (D-STZ-22). Read-only: it is
//! composed with the read port alone (D-STZ-38), so it cannot name a write
//! (check-arch `indexer_search_handler_read_only`, extended).

use std::sync::Arc;

use adapter_xrpc_query_server::{
    dispatch, BodyRead, Handlers, PublicAnswer, PublicRequest, REQUEST_TIMEOUT,
};
use openlore_indexer::events::Events;
use openlore_indexer::run::search_answers;
use ports::IndexReadPort;
use serde_json::{json, Map, Value};

use crate::index_holder::HeldIndex;
use crate::{events_through, Environment, InitRefused, SearchDriven};

/// One warm search instance.
pub struct SearchFunction {
    handlers: Handlers,
}

impl SearchFunction {
    /// Lambda init: fetch the index (or open an empty one when there is no
    /// object) and get ready. Refused on any other store failure.
    pub async fn init(env: &Environment, driven: SearchDriven) -> Result<Self, InitRefused> {
        let config = env.indexer_config()?;
        let index = HeldIndex::fetched(driven.index_object.as_ref(), &config).await?;
        Ok(Self {
            handlers: handlers_over(&index, &events_through(driven.events)),
        })
    }

    /// Answer one Function URL event (payload format 2.0) with a Function URL
    /// response.
    pub async fn handle(&self, event: Value) -> Value {
        let request = PublicRequest {
            method: text_at(&event, "/requestContext/http/method"),
            path: text_at(&event, "/rawPath"),
            body: std::future::ready(body_of(&event)),
            time_allowed: REQUEST_TIMEOUT,
        };
        function_url_response(&dispatch(request, &self.handlers).await)
    }
}

/// The public surface over one copy. A search holds `IndexReadPort` and nothing
/// that writes.
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
