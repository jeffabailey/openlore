//! The index a search instance answers from: the object fetched into its `/tmp`
//! (or an empty index when no object exists) and the version it came from, so
//! `/healthz` reports what the object's metadata says (D-STZ-12). A reload brings
//! the next copy into a new file, the holder swaps it in, and the old file goes
//! with the old copy (D-STZ-10).

use std::path::{Path, PathBuf};
use std::sync::{Arc, PoisonError, RwLock};

use adapter_index_store::{DuckDbCaps, IndexStoreAdapter};
use adapter_xrpc_query_server::HealthHandler;
use openlore_indexer::config::IndexerConfig;
use openlore_indexer::health::{health_handler, published_at_of_fetched, store_usability};
use openlore_indexer::search_handler::SharedIndexReads;
use ports::{IndexObjectReadPort, ObjectVersion};

use crate::InitRefused;

/// The file of one copy in `/tmp`, removed when the copy is dropped. So a failed
/// download or open leaves nothing behind, and a retired copy gives its space
/// back (D-STZ-30, open question 2). The file may never have been written, and a
/// drop has nobody to tell, so the removal's result is not read.
struct ScratchFile(PathBuf);

impl Drop for ScratchFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

/// Where the copy of this generation lives: the configured path first, then a
/// new file for each reload.
fn copy_path(index_path: &Path, generation: u64) -> PathBuf {
    if generation == 0 {
        return index_path.to_path_buf();
    }
    let mut path = index_path.as_os_str().to_owned();
    path.push(format!(".{generation}"));
    path.into()
}

/// One open copy of the index and the object version it holds.
pub(crate) struct HeldIndex {
    store: Arc<IndexStoreAdapter>,
    version: Option<ObjectVersion>,
    generation: u64,
    /// After `store`: fields drop in order, so the store closes before its file goes.
    _file: ScratchFile,
}

impl HeldIndex {
    /// Fetch the current object to the configured path and open it. No object
    /// leaves no file there, so the store opens empty.
    pub(crate) async fn fetched(
        index_object: &dyn IndexObjectReadPort,
        config: &IndexerConfig,
    ) -> Result<Self, InitRefused> {
        Self::fetched_as(index_object, config, 0).await
    }

    /// The copy to replace this one: the current object in a new file.
    pub(crate) async fn successor(
        &self,
        index_object: &dyn IndexObjectReadPort,
        config: &IndexerConfig,
    ) -> Result<Self, InitRefused> {
        Self::fetched_as(index_object, config, self.generation + 1).await
    }

    async fn fetched_as(
        index_object: &dyn IndexObjectReadPort,
        config: &IndexerConfig,
        generation: u64,
    ) -> Result<Self, InitRefused> {
        let path = copy_path(&config.index_path, generation);
        let file = ScratchFile(path.clone());
        let version = index_object.fetch_to(&path).await.map_err(refused)?;
        let caps = DuckDbCaps {
            memory_limit_mb: config.duckdb_memory_limit_mb,
            threads: config.duckdb_threads,
        };
        let store = IndexStoreAdapter::open_capped(&path, caps).map_err(refused)?;
        Ok(Self {
            store: Arc::new(store),
            version,
            generation,
            _file: file,
        })
    }

    /// The ETag of the object this copy came from; `None` for an empty index.
    pub(crate) fn etag(&self) -> Option<&str> {
        self.version.as_ref().map(|version| version.etag.as_str())
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

    /// TEST-FAULT seam (`store_poisoned_after_first_search`, debug builds only).
    pub(crate) fn poison_for_test_fault(&self) {
        self.store.poison_for_test_fault();
    }
}

/// The copy being served. A swap takes effect for the next answer, and the last
/// reference to the old copy closes it and deletes its file.
pub(crate) struct IndexHolder(RwLock<Arc<HeldIndex>>);

impl IndexHolder {
    pub(crate) fn holding(index: HeldIndex) -> Self {
        Self(RwLock::new(Arc::new(index)))
    }

    pub(crate) fn current(&self) -> Arc<HeldIndex> {
        let served = self.0.read().unwrap_or_else(PoisonError::into_inner);
        Arc::clone(&served)
    }

    pub(crate) fn swap(&self, next: HeldIndex) {
        *self.0.write().unwrap_or_else(PoisonError::into_inner) = Arc::new(next);
    }
}

fn refused(error: impl std::fmt::Display) -> InitRefused {
    InitRefused {
        reason: error.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    const CONFIGURED: &str = "/tmp/index.duckdb";

    proptest! {
        #[test]
        fn the_first_copy_is_at_the_configured_path_and_no_two_generations_share_a_file(
            first in any::<u64>(),
            second in any::<u64>(),
        ) {
            let configured = Path::new(CONFIGURED);
            prop_assert_eq!(copy_path(configured, 0), configured);
            prop_assert_eq!(
                copy_path(configured, first) == copy_path(configured, second),
                first == second
            );
        }
    }
}
