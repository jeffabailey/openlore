//! `index_store` — the indexer-side index-store port (ADR-025) + its railway
//! error. SYNC (local DB), like `StoragePort` — NO `async_trait`.
//!
//! `IndexStorePort` is the index store over the SEPARATE `index.duckdb`
//! (ADR-023: the indexer never touches the user's `openlore.duckdb`). Every
//! query method returns `Vec<IndexedClaim>` / `Option<IndexedClaim>` whose rows
//! carry a NON-`Option` `author_did` — the type-level anti-merging defense
//! (WD-120 / I-AV-2). There is intentionally NO method that aggregates across
//! authors (NO `GROUP BY` / `COUNT` / `SUM`-across-authors surface): the
//! `distinct_author_count` aggregation happens in the PURE `appview-domain`
//! core (Rust), NEVER in SQL (the structural `xtask check-arch`
//! `no_cross_table_join_elides_author` rule is the other half).
//!
//! See data-models.md §"Read-side query shapes" + component-boundaries.md
//! §`crates/ports`.
//
// SCAFFOLD: true  (trait surface only; the adapter impl lands in step 01-03/04)

use std::collections::BTreeSet;

use claim_domain::{Cid, Did};

use crate::{IndexedClaim, ProbeOutcome};

// -----------------------------------------------------------------------------
// IndexStoreError — the railway-oriented failure surface
// -----------------------------------------------------------------------------

/// Why an index-store operation failed. Mirrors `StorageError`'s shape for the
/// separate `index.duckdb` store (ADR-025): schema/migration, write, read, and
/// query failures, plus the probe refusal.
#[derive(Debug, thiserror::Error)]
pub enum IndexStoreError {
    #[error("index store probe refused: {detail}")]
    ProbeRefused { detail: String },
    #[error("index schema migration failed: {message}")]
    SchemaMigrationFailed { message: String },
    #[error("index write failed for cid {cid:?}: {message}")]
    WriteFailed { cid: Cid, message: String },
    #[error("index read failed for cid {cid:?}: {message}")]
    ReadFailed { cid: Cid, message: String },
    #[error("index query failed: {message}")]
    QueryFailed { message: String },
}

// -----------------------------------------------------------------------------
// IndexReadPort — the read side of index.duckdb (ADR-082/083, B7)
// -----------------------------------------------------------------------------

/// The READ side of the index store over the SEPARATE `index.duckdb`
/// (ADR-023/025). SYNC (local DB) — like `StoragePort`, NO `async_trait`.
///
/// Split from the write side (B7, DD-IXD-9) so the indexer's search handler
/// can hold reads ONLY: by type it cannot reach an upsert, a purge or a probe
/// (principle 12 — "no public request can change the index" is structural).
/// Contract shape: unbounded-preservation — every method leaves the whole store
/// exactly as it found it.
///
/// Every query returns rows carrying a NON-`Option` `author_did` (type-level
/// anti-merging, I-AV-2). There is intentionally NO aggregate-across-authors
/// method (NO `GROUP BY`/`COUNT`/`SUM`-across-authors surface): aggregation is
/// composed in the PURE `appview-domain` core from individually-attributed
/// rows, NEVER as a stored merged row or an author-eliding SQL aggregate
/// (WD-103).
pub trait IndexReadPort {
    /// Which claims assert this `object` (philosophy). Every row carries its
    /// non-`Option` `author_did`; the pure core groups by author. Two
    /// identical-content claims from different authors stay TWO rows (I-AV-2).
    fn query_by_object(&self, object: &str) -> Result<Vec<IndexedClaim>, IndexStoreError>;

    /// Every claim authored by this DID, across all subjects. One developer's
    /// whole network trail — "one developer's reasoning trail, not a community
    /// consensus" (a render-side framing; the rows are attributed here).
    fn query_by_contributor(&self, did: &Did) -> Result<Vec<IndexedClaim>, IndexStoreError>;

    /// Which claims address this `subject` (project). Grouped by author in the
    /// pure core; NO "the network thinks X about this project" merged row.
    fn query_by_subject(&self, subject: &str) -> Result<Vec<IndexedClaim>, IndexStoreError>;

    /// Fetch one indexed claim by its (verified) CID PK — the `--show` key.
    fn get_by_cid(&self, cid: &Cid) -> Result<Option<IndexedClaim>, IndexStoreError>;

    /// The distinct indexed objects within `max_distance` edits of `object`,
    /// closest first, at most [`NEAR_OBJECTS_CAP`]: the candidates a "did you
    /// mean" suggestion for an empty object search is picked from (US-AV-002
    /// Ex 4). Object values only — no author, no claim, nothing merged.
    fn objects_near(
        &self,
        object: &str,
        max_distance: usize,
    ) -> Result<Vec<String>, IndexStoreError>;
}

/// The most near-match candidates [`IndexReadPort::objects_near`] returns.
pub const NEAR_OBJECTS_CAP: usize = 64;

// -----------------------------------------------------------------------------
// IndexStorePort — the write side (probe + upsert) over index.duckdb
// -----------------------------------------------------------------------------

/// The indexer-side index store over the SEPARATE `index.duckdb` (ADR-023/025):
/// the read side ([`IndexReadPort`], a supertrait) plus the write side — the
/// Earned-Trust probe and the de-dup-by-CID upsert (ADR-025). SYNC (local DB).
///
/// Only the composition root and the ingest pass hold this port; the search
/// handler holds [`IndexReadPort`] alone (B7).
pub trait IndexStorePort: IndexReadPort {
    /// Earned-Trust probe — see ADR-009 + `probe.rs`. The adapter impl asserts
    /// schema version + fsync honored on the substrate, attribution round-trip
    /// (distinct non-empty `author_did`s read back byte-equal), and the
    /// no-merge-schema assertion (NO consensus/merged table). REQUIRED per I-4.
    fn probe(&self) -> ProbeOutcome;

    /// Insert (or de-dup-by-CID upsert) one verified, attributed indexed claim.
    fn upsert(&self, claim: &IndexedClaim) -> Result<(), IndexStoreError>;

    /// Fold the write-ahead log into the database file, explicitly, at the end
    /// of a pass (ADR-080 §6): automatic checkpoints during the pass's writes
    /// stay rare and small, so a search never waits on a large one.
    fn checkpoint(&self) -> Result<(), IndexStoreError>;
}

// -----------------------------------------------------------------------------
// IndexPurgePort — the delete side, held only by the pass runner (ADR-082)
// -----------------------------------------------------------------------------

/// What purging one author removed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PurgeReport {
    /// How many of the author's claims (rows) were removed.
    pub claims_removed: u64,
}

/// The ONLY delete capability over `index.duckdb` (ADR-082, B6), separate
/// from the read and upsert surfaces so that it can be handed to the pass
/// runner alone (check-arch `index_purge_only_in_pass_runner`).
///
/// `purge_author` is a bounded change: it removes the author's claim rows
/// (stored under the bare DID or any `bare#fragment` key), their evidence and
/// outgoing references, and each removed row's artifact file at its stored
/// `signed_record_path` — nothing else. It is resumable and idempotent: an
/// author stays in [`IndexPurgePort::indexed_authors`] until their rows are
/// gone, and purging an absent author removes nothing.
pub trait IndexPurgePort {
    /// Earned-Trust probe: the schema this binary purges, and the author
    /// listing the purge plan is computed from, both answer.
    fn probe(&self) -> ProbeOutcome;

    /// Every author the index holds a claim of, as BARE DIDs (read-only).
    fn indexed_authors(&self) -> Result<BTreeSet<String>, IndexStoreError>;

    /// Remove one author (a BARE DID) from the index.
    fn purge_author(&self, bare_did: &str) -> Result<PurgeReport, IndexStoreError>;
}
