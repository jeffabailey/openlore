//! Search mode (ADR-090 §3, §5): fetch the index object at init, re-check it at
//! most every TTL before answering, then answer through the shared transport-free
//! `dispatch` (D-STZ-22). Read-only: it is composed with the read port alone
//! (D-STZ-38), so it cannot name a write (check-arch
//! `indexer_search_handler_read_only`, extended).
//
// SCAFFOLD: true  (DISTILL 2026-10-27)

use crate::{Environment, InitRefused, SearchDriven};

/// One warm search instance.
pub struct SearchFunction {
    _driven: SearchDriven,
}

impl SearchFunction {
    /// Lambda init: fetch the index (or open an empty one when there is no
    /// object), probe, and get ready. Refused on any other store failure.
    pub async fn init(env: &Environment, driven: SearchDriven) -> Result<Self, InitRefused> {
        let _ = (env, driven);
        panic!("Not yet implemented -- RED scaffold")
    }

    /// Answer one Function URL event (payload format 2.0) with a Function URL
    /// response.
    pub async fn handle(&self, event: serde_json::Value) -> serde_json::Value {
        let _ = event;
        panic!("Not yet implemented -- RED scaffold")
    }
}
