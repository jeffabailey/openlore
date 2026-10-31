//! `openlore-indexer-lambda` — the FOURTH composition root (ADR-090, D-STZ-1):
//! the indexer's search, pass and probe as three AWS Lambda functions from one
//! signed image. `main` reads the Lambda environment and the AWS SDK config,
//! builds the driven adapters, and hands one Lambda event at a time to the
//! entry points below. Acceptance tests call the same entry points with the
//! external ports faked (Pillar 3: only the object store, the parameter store,
//! the clock and the event sink are substituted).
//!
//! | Function | Entry | Event in | Answer out |
//! |---|---|---|---|
//! | search | [`SearchFunction::handle`] | Function URL event, payload 2.0 | Function URL response (`statusCode`, `headers`, `body`) |
//! | pass | [`PassFunction::handle`] | `{}` from the schedule (`input = "{}"`), or exactly `{"mode":"no-publish"}` from the deploy gate; any other event is refused (D-STZ-44) | `{pass_id, exit_code, published, index_bytes}` for exit 0/2/3, plus `mode` and `would_publish` on a no-publish run; a function error only for a crash or a refused event (U-STZ-5, D-STZ-44) |
//! | probe | [`ProbeFunction::handle`] | `{}` from the schedule; an event that is not a JSON object is refused with a function error (U-STZ-7) | one `indexer.host.health` line, and `indexer.lambda.served_index_stale` when the served index lags the record (D-STZ-48); a function error only for a crash or a refused event |
//!
//! Every function writes its JSON event lines through [`EventSink`] (stdout in
//! production, so the existing log metric filters match unchanged).

#![forbid(unsafe_code)]

pub mod decide;
mod index_holder;
pub mod no_publish;
mod pass;
mod probe;
mod search;

use std::collections::BTreeMap;
use std::sync::Arc;

use adapter_atproto_did::AtProtoDidAdapter;
use openlore_indexer::config::{parse_config, BuildProfile, IndexerConfig};
use openlore_indexer::events::{EventWriter, Events};
use openlore_indexer::run::{IndexerWiring, Wired};
use ports::{
    ClockPort, IdentityResolvePort, IndexObjectReadPort, IndexObjectWritePort,
    RepoDidListSourcePort,
};

pub use pass::PassFunction;
pub use probe::ProbeFunction;
pub use search::SearchFunction;

/// The function's environment as Lambda hands it over (`std::env::vars()` in
/// `main`). The same variable names as the self-host indexer, plus the Lambda
/// ones (bucket, keys, DID parameter, probe base URL, freshness TTL).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Environment(pub BTreeMap<String, String>);

impl Environment {
    /// The indexer's own configuration, read from this environment.
    fn indexer_config(&self) -> Result<IndexerConfig, InitRefused> {
        parse_config(
            |name| self.0.get(name).cloned(),
            BuildProfile::of_this_build(),
        )
        .map_err(|refused| InitRefused {
            reason: refused.to_string(),
        })
    }
}

/// Where the function's JSON event lines go: stdout in production.
pub trait EventSink: Send + Sync {
    fn line(&self, line: &str);
}

/// The indexer's event writer over the root's sink (UI-8).
struct SinkWriter(Arc<dyn EventSink>);

impl EventWriter for SinkWriter {
    fn write_line(&self, line: &str) -> std::io::Result<()> {
        self.0.line(line);
        Ok(())
    }

    fn flush(&self) -> std::io::Result<()> {
        Ok(())
    }
}

fn events_through(sink: Arc<dyn EventSink>) -> Events {
    Events::new(Arc::new(SinkWriter(sink)))
}

/// The verify-only resolve path (D-STZ-50): the root's one site, held behind
/// `IdentityResolvePort`.
fn verify_only_resolver() -> Box<dyn IdentityResolvePort> {
    Box::new(AtProtoDidAdapter::resolve_only())
}

/// The pass's store, ingest and DID-resolve clients, composed from the
/// environment (D-STZ-38). Opens the store, so it is called off the async runtime.
fn pass_wiring(config: IndexerConfig, sink: Arc<dyn EventSink>) -> Result<Wired, FunctionError> {
    IndexerWiring::production(config, events_through(sink), verify_only_resolver()).map_err(
        |refused| FunctionError {
            message: format!("{refused:#}"),
        },
    )
}

/// What the search function is composed with (D-STZ-38): the read port only, so
/// its constructor cannot be handed a write capability. In production `main`
/// builds these from the S3 adapter, the system clock and stdout.
#[derive(Clone)]
pub struct SearchDriven {
    pub index_object: Arc<dyn IndexObjectReadPort>,
    pub clock: Arc<dyn ClockPort + Send + Sync>,
    pub events: Arc<dyn EventSink>,
}

/// What the pass function is composed with: both object ports and the DID list.
#[derive(Clone)]
pub struct PassDriven {
    pub index_read: Arc<dyn IndexObjectReadPort>,
    pub index_write: Arc<dyn IndexObjectWritePort>,
    pub repo_dids: Arc<dyn RepoDidListSourcePort>,
    pub clock: Arc<dyn ClockPort + Send + Sync>,
    pub events: Arc<dyn EventSink>,
}

/// What the probe function is composed with: the read port only (D-STZ-38). Its
/// HTTP calls go through the public edge, on a client it builds from the
/// environment (D-STZ-37).
#[derive(Clone)]
pub struct ProbeDriven {
    pub index_object: Arc<dyn IndexObjectReadPort>,
    pub clock: Arc<dyn ClockPort + Send + Sync>,
    pub events: Arc<dyn EventSink>,
}

/// Init refused (D-STZ-14, D-STZ-39): the function never answers;
/// `health.startup.refused` was emitted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InitRefused {
    pub reason: String,
}

/// A function error (a crash, a timeout, or a refused event): Lambda counts it
/// in `Errors` (A6).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionError {
    pub message: String,
}
