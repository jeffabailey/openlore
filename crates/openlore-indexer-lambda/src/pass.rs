//! Pass mode (ADR-090 §4): in the D-STZ-40 order, read the DID list from the
//! parameter store, fetch the index object, run the reused ADR-077/078 pass
//! (D-STZ-42) and take its exit, publish conditionally when the fingerprint
//! changed, write `last-pass.json`, then write the ONE `pass_summary`, carrying
//! the final exit from `decide::final_exit`. An event `decide::pass_mode` does
//! not accept is a function error, before any work and any call. A no-publish
//! run stops after the pass work: that arm is `no_publish::run_no_publish`,
//! which takes the read port only, so it has no path to a write (D-STZ-44). Init
//! runs the write probe whatever the event (D-STZ-39, U-STZ-8).
//
// SCAFFOLD: true  (DISTILL 2026-10-27)

use crate::{Environment, FunctionError, InitRefused, PassDriven};

/// One pass instance (warm across scheduled events).
pub struct PassFunction {
    _driven: PassDriven,
}

impl PassFunction {
    /// Lambda init: the write probe, through `spawn_blocking`, because it is sync
    /// and init is async (D-STZ-39). Refused only on two accepted writes; a store
    /// outage is a failed pass (D-STZ-41).
    pub async fn init(env: &Environment, driven: PassDriven) -> Result<Self, InitRefused> {
        let _ = (env, driven);
        panic!("Not yet implemented -- RED scaffold")
    }

    /// Run one pass for one event. `Ok` carries
    /// `{pass_id, exit_code, published, index_bytes}` for exit 0, 2 and 3, and a
    /// no-publish run adds `mode` and `would_publish`. A refused event is `Err`.
    pub async fn handle(
        &self,
        event: serde_json::Value,
    ) -> Result<serde_json::Value, FunctionError> {
        let _ = event;
        panic!("Not yet implemented -- RED scaffold")
    }
}
