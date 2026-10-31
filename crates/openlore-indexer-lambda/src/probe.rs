//! Probe mode (D-STZ-23, U-STZ-3, U-STZ-6, U-STZ-7): the canned search and then
//! `GET /healthz` through the public edge (`adapter-index-query`, D-STZ-37), the
//! age of `last-pass.json`, then exactly one `indexer.host.health` line (A3).
//! When the served index lags the record it also writes
//! `indexer.lambda.served_index_stale` (A10), which never changes `not_live`.
//! Read-only: it is composed with the read port alone (D-STZ-38).
//
// SCAFFOLD: true  (DISTILL 2026-10-27)

use crate::{Environment, FunctionError, InitRefused, ProbeDriven};

/// One probe instance.
pub struct ProbeFunction {
    _driven: ProbeDriven,
}

impl ProbeFunction {
    /// Lambda init. Refused on a key naming the write probe's marker (D-STZ-39).
    pub async fn init(env: &Environment, driven: ProbeDriven) -> Result<Self, InitRefused> {
        env.refuse_a_key_naming_the_marker(&driven.events)?;
        panic!("Not yet implemented -- RED scaffold")
    }

    /// Run one probe for one event. An event that is not a JSON object is
    /// refused before anything else happens (U-STZ-7).
    pub async fn handle(
        &self,
        event: serde_json::Value,
    ) -> Result<serde_json::Value, FunctionError> {
        refuse_a_non_object_event(&event)?;
        panic!("Not yet implemented -- RED scaffold")
    }
}

/// U-STZ-7: a function error, with no health line and no calls. `{}` runs a
/// normal probe. `lambda test-alarm errors` relies on this for a real, harmless
/// `Errors` datapoint.
fn refuse_a_non_object_event(event: &serde_json::Value) -> Result<(), FunctionError> {
    let _ = event;
    panic!("Not yet implemented -- RED scaffold")
}
