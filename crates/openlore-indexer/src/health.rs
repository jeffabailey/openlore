//! `/healthz` (ADR-083 §2, B8) — the effect shell around the pure
//! `appview_domain::health` projection: it reads the pass runner's published
//! status and projects it. It holds a [`StatusReader`] only, so it can neither
//! start a pass nor change the status.

use std::sync::Arc;

use adapter_xrpc_query_server::HealthHandler;
use appview_domain::health::{health_of, StoreHealth};
use chrono::{DateTime, Utc};

use crate::pass_runner::StatusReader;

/// The `/healthz` handler over the runner's status. The store is usable for as
/// long as `serve` answers: an unusable store (the 503 arm) is wired with the
/// poisoned-store exit (02-03).
pub fn health_handler(status: StatusReader) -> HealthHandler {
    Arc::new(move || {
        let last_success = status
            .status()
            .last_successful_pass_at()
            .map(DateTime::<Utc>::from);
        health_of(StoreHealth::Usable, last_success)
    })
}
