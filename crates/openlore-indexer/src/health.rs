//! `/healthz` (ADR-083 §2, B8) — the effect shell around the pure
//! `appview_domain::health` projection: it reads the pass runner's published
//! status and whether the store is usable, and projects them. It holds a
//! [`StatusReader`] and a [`StoreUsability`] reader only, so it can neither
//! start a pass nor change the status.

use std::sync::Arc;

use adapter_xrpc_query_server::HealthHandler;
use appview_domain::health::{health_of, StoreHealth};
use chrono::{DateTime, Utc};

use crate::pass_runner::StatusReader;

/// Reads whether the process's one store handle is still usable (ADR-080 §7).
pub type StoreUsability = Arc<dyn Fn() -> StoreHealth + Send + Sync>;

/// The `/healthz` handler over the runner's status and the store's usability:
/// an unusable store is a 503, whatever the last pass did.
pub fn health_handler(status: StatusReader, store: StoreUsability) -> HealthHandler {
    Arc::new(move || {
        let last_success = status
            .status()
            .last_successful_pass_at()
            .map(DateTime::<Utc>::from);
        health_of(store(), last_success)
    })
}
