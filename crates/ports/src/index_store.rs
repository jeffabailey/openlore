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
}

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
