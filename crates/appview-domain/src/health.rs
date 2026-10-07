//! `health` — the pure projection behind the public `GET /healthz` (ADR-083 §2,
//! data-models §6).
//!
//! The response is honest and minimal: an unusable store is always `503
//! store_unusable`; a usable one is `200` with exactly `status` and the end
//! time of the last successful pass (or `null`). Nothing else is ever exposed
//! — no counts, no DIDs, no version.

use chrono::{DateTime, SecondsFormat, Utc};

/// Whether the index store can still answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StoreHealth {
    Usable,
    Unusable,
}

/// What `/healthz` answers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HealthResponse {
    /// The store answers; when the last exit-0 pass ended (`None` before one).
    Healthy {
        last_successful_pass_at: Option<DateTime<Utc>>,
    },
    StoreUnusable,
}

/// The health of a `serve` process: the store's state decides the status;
/// only a usable store reports the last successful pass.
pub const fn health_of(
    store: StoreHealth,
    last_successful_pass_at: Option<DateTime<Utc>>,
) -> HealthResponse {
    match store {
        StoreHealth::Usable => HealthResponse::Healthy {
            last_successful_pass_at,
        },
        StoreHealth::Unusable => HealthResponse::StoreUnusable,
    }
}

impl HealthResponse {
    /// The HTTP status.
    pub const fn status_code(&self) -> u16 {
        match self {
            Self::Healthy { .. } => 200,
            Self::StoreUnusable => 503,
        }
    }

    /// The JSON body: exactly the fields data-models §6 names.
    pub fn body(&self) -> serde_json::Value {
        match self {
            Self::Healthy {
                last_successful_pass_at,
            } => serde_json::json!({
                "status": "ok",
                "last_successful_pass_at": last_successful_pass_at
                    .map(|at| at.to_rfc3339_opts(SecondsFormat::Secs, true)),
            }),
            Self::StoreUnusable => serde_json::json!({"status": "store_unusable"}),
        }
    }
}
