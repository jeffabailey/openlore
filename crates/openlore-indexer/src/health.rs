//! `/healthz` (ADR-083 §2, B8) — the effect shell around the pure
//! `appview_domain::health` projection: it reads when the index was last
//! published and whether the store is usable, and projects them. It holds a
//! [`LastPublishedAt`] reader and a [`StoreUsability`] reader only, so it can
//! neither start a pass nor change what they report.

use std::sync::Arc;

use adapter_index_store::IndexStoreAdapter;
use adapter_xrpc_query_server::HealthHandler;
use appview_domain::health::{health_of, StoreHealth};
use chrono::{DateTime, Utc};
use ports::ObjectVersion;

/// Reads whether the process's one store handle is still usable (ADR-080 §7).
pub type StoreUsability = Arc<dyn Fn() -> StoreHealth + Send + Sync>;

/// Reads when the index was last published (`None` before the first time). The
/// self-host's reader is the pass runner's last exit-0 pass; the Lambda root's
/// is [`published_at_of_fetched`].
pub type LastPublishedAt = Arc<dyn Fn() -> Option<DateTime<Utc>> + Send + Sync>;

/// Whether `store` is usable, as `/healthz` reports it.
pub fn store_usability(store: Arc<IndexStoreAdapter>) -> StoreUsability {
    Arc::new(move || match store.unusable_reason() {
        None => StoreHealth::Usable,
        Some(_) => StoreHealth::Unusable,
    })
}

/// The `published-at` metadata of the index object an instance fetched
/// (D-STZ-12); `None` when no object exists.
pub fn published_at_of_fetched(version: Option<&ObjectVersion>) -> LastPublishedAt {
    let published_at = version
        .and_then(|fetched| fetched.published_at.as_deref())
        .and_then(|stamp| stamp.parse().ok());
    Arc::new(move || published_at)
}

/// The `/healthz` handler over the last publication and the store's usability:
/// an unusable store is a 503, whatever was last published.
pub fn health_handler(last_published_at: LastPublishedAt, store: StoreUsability) -> HealthHandler {
    Arc::new(move || health_of(store(), last_published_at()))
}
