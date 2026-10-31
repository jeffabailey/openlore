//! The index a search instance answers from: the object fetched into its `/tmp`
//! (or an empty index when no object exists) and the version it came from, so
//! `/healthz` reports what the object's metadata says (D-STZ-12).

use std::sync::Arc;

use adapter_index_store::{DuckDbCaps, IndexStoreAdapter};
use adapter_xrpc_query_server::HealthHandler;
use openlore_indexer::config::IndexerConfig;
use openlore_indexer::health::{health_handler, published_at_of_fetched, store_usability};
use openlore_indexer::search_handler::SharedIndexReads;
use ports::{IndexObjectReadPort, ObjectVersion};

use crate::InitRefused;

/// One open copy of the index and the object version it holds.
pub(crate) struct HeldIndex {
    store: Arc<IndexStoreAdapter>,
    version: Option<ObjectVersion>,
}

impl HeldIndex {
    /// Fetch the current object to the configured path and open it. No object
    /// leaves no file there, so the store opens empty.
    pub(crate) async fn fetched(
        index_object: &dyn IndexObjectReadPort,
        config: &IndexerConfig,
    ) -> Result<Self, InitRefused> {
        let version = index_object
            .fetch_to(&config.index_path)
            .await
            .map_err(refused)?;
        let caps = DuckDbCaps {
            memory_limit_mb: config.duckdb_memory_limit_mb,
            threads: config.duckdb_threads,
        };
        let store = IndexStoreAdapter::open_capped(&config.index_path, caps).map_err(refused)?;
        Ok(Self {
            store: Arc::new(store),
            version,
        })
    }

    /// This copy as a search sees it: through its read port alone.
    pub(crate) fn reads(&self) -> SharedIndexReads {
        Arc::clone(&self.store) as SharedIndexReads
    }

    /// `/healthz` over this copy: its usability and the version's stamp.
    pub(crate) fn health(&self) -> HealthHandler {
        health_handler(
            published_at_of_fetched(self.version.as_ref()),
            store_usability(Arc::clone(&self.store)),
        )
    }
}

fn refused(error: impl std::fmt::Display) -> InitRefused {
    InitRefused {
        reason: error.to_string(),
    }
}
