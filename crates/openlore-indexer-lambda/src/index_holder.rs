//! The index a search instance answers from: the object fetched into its `/tmp`
//! (or an empty index when no object exists) and the version it came from, so
//! `/healthz` reports what the object's metadata says (D-STZ-12). A reload brings
//! the next copy into a new file, the holder swaps it in, and the old file goes
//! with the old copy (D-STZ-10).
//!
//! A copy is taken only after one read through it (D-STZ-14). The download checks
//! the length and an open reads little of the file, so this read is what stops a
//! whole but damaged object: init refuses it, and a reload keeps the served copy.
//!
//! The 3 s reload bound is a tokio timeout over the whole reload (the root's, in
//! `search`), and a timeout acts only when the reload yields. So the download is
//! dropped where it waits, and the open and that read, which are synchronous, run
//! on a blocking thread for the search to wait on. That thread cannot be stopped:
//! at the bound the search goes on, and the open runs out alone. The copy it is building owns
//! the file, so the file goes when the download is dropped or when the abandoned
//! open ends, and each attempt has its own file, so a retry never meets it.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, PoisonError, RwLock};

use adapter_index_store::{write_ahead_log_path, DuckDbCaps, IndexStoreAdapter};
use adapter_xrpc_query_server::HealthHandler;
use openlore_indexer::config::IndexerConfig;
use openlore_indexer::health::{health_handler, published_at_of_fetched, store_usability};
use openlore_indexer::search_handler::SharedIndexReads;
use ports::{IndexObjectReadPort, ObjectVersion};

use crate::InitRefused;

/// The file of one copy in `/tmp` and its log, removed when the copy is dropped.
/// So a failed download or open leaves nothing behind, and a retired copy gives
/// its space back (D-STZ-30, open question 2). Either file may never have been
/// written, and a drop has nobody to tell, so the removals' results are not read.
struct ScratchFile(PathBuf);

impl Drop for ScratchFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
        let _ = std::fs::remove_file(write_ahead_log_path(&self.0));
    }
}

/// Where the copy of this generation lives: the configured path first, then a
/// new file for each reload attempt.
fn copy_path(index_path: &Path, generation: u64) -> PathBuf {
    if generation == 0 {
        return index_path.to_path_buf();
    }
    let mut path = index_path.as_os_str().to_owned();
    path.push(format!(".{generation}"));
    path.into()
}

/// Hands each reload attempt a generation of its own, past the first copy's 0.
#[derive(Default)]
struct ReloadAttempts(AtomicU64);

impl ReloadAttempts {
    fn next_generation(&self) -> u64 {
        self.0.fetch_add(1, Ordering::Relaxed) + 1
    }
}

/// One open copy of the index and the object version it holds.
pub(crate) struct HeldIndex {
    store: Arc<IndexStoreAdapter>,
    version: Option<ObjectVersion>,
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

    async fn fetched_as(
        index_object: &dyn IndexObjectReadPort,
        config: &IndexerConfig,
        generation: u64,
    ) -> Result<Self, InitRefused> {
        let file = ScratchFile(copy_path(&config.index_path, generation));
        let version = index_object.fetch_to(&file.0).await.map_err(refused)?;
        let caps = DuckDbCaps {
            memory_limit_mb: config.duckdb_memory_limit_mb,
            threads: config.duckdb_threads,
        };
        off_the_runtime(move || {
            let store = IndexStoreAdapter::open_capped(&file.0, caps).map_err(refused)?;
            store.read_through().map_err(refused)?;
            Ok(Self {
                store: Arc::new(store),
                version,
                _file: file,
            })
        })
        .await
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
pub(crate) struct IndexHolder {
    served: RwLock<Arc<HeldIndex>>,
    reload_attempts: ReloadAttempts,
}

impl IndexHolder {
    pub(crate) fn holding(index: HeldIndex) -> Self {
        Self {
            served: RwLock::new(Arc::new(index)),
            reload_attempts: ReloadAttempts::default(),
        }
    }

    pub(crate) fn current(&self) -> Arc<HeldIndex> {
        let served = self.served.read().unwrap_or_else(PoisonError::into_inner);
        Arc::clone(&served)
    }

    /// The copy to replace the served one: the current object in a file of its
    /// own, which an open abandoned by an earlier attempt cannot be holding.
    pub(crate) async fn next_copy(
        &self,
        index_object: &dyn IndexObjectReadPort,
        config: &IndexerConfig,
    ) -> Result<HeldIndex, InitRefused> {
        let generation = self.reload_attempts.next_generation();
        HeldIndex::fetched_as(index_object, config, generation).await
    }

    pub(crate) fn swap(&self, next: HeldIndex) {
        *self.served.write().unwrap_or_else(PoisonError::into_inner) = Arc::new(next);
    }
}

/// Run a synchronous open where a timeout on the caller can still fire. Whatever
/// `open` owns, its result included, is dropped when it ends, waited for or not.
async fn off_the_runtime<Opened: Send + 'static>(
    open: impl FnOnce() -> Result<Opened, InitRefused> + Send + 'static,
) -> Result<Opened, InitRefused> {
    tokio::task::spawn_blocking(open).await.map_err(refused)?
}

fn refused(error: impl std::fmt::Display) -> InitRefused {
    InitRefused {
        reason: error.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use std::sync::mpsc;
    use std::time::Duration;

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

        #[test]
        fn no_reload_attempt_shares_a_generation_with_another_or_with_the_first_copy(
            attempts in 1..64_usize,
        ) {
            let reload_attempts = ReloadAttempts::default();
            let generations: HashSet<u64> = (0..attempts)
                .map(|_| reload_attempts.next_generation())
                .collect();
            prop_assert_eq!(generations.len(), attempts);
            prop_assert!(!generations.contains(&0));
        }
    }

    // bypass: one fixed pair of files, and dropping the copy has no input space.
    #[test]
    fn a_dropped_copy_takes_its_log_with_it() {
        let scratch = tempfile::tempdir().expect("a scratch directory");
        let path = scratch.path().join("index.duckdb.1");
        let log = scratch.path().join("index.duckdb.1.wal");
        std::fs::write(&path, b"a copy").expect("a copy");
        std::fs::write(&log, b"rows").expect("its log");

        drop(ScratchFile(path.clone()));

        assert!(!path.exists(), "the copy");
        assert!(!log.exists(), "its log");
    }

    // bypass: the contract is how one stuck open meets a timer; it has no input space.
    #[tokio::test]
    async fn an_open_that_outlasts_the_bound_is_left_behind_and_its_file_goes_when_it_ends() {
        let scratch = tempfile::tempdir().expect("a scratch directory");
        let path = scratch.path().join("index.duckdb.1");
        std::fs::write(&path, b"half an index").expect("a partial file");
        let file = ScratchFile(path.clone());
        let (end_the_open, open_may_end) = mpsc::channel::<()>();

        let open = off_the_runtime(move || {
            let _ = open_may_end.recv_timeout(Duration::from_secs(5));
            Ok(file)
        });
        let bounded = tokio::time::timeout(Duration::from_millis(10), open).await;

        assert!(bounded.is_err(), "the bound fires during the open");
        end_the_open.send(()).expect("the open is still running");
        let file_gone = async {
            while path.exists() {
                tokio::time::sleep(Duration::from_millis(1)).await;
            }
        };
        tokio::time::timeout(Duration::from_secs(5), file_gone)
            .await
            .expect("the file goes when the abandoned open ends");
    }
}
