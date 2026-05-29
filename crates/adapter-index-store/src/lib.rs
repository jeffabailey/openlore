//! `adapter-index-store` — embedded-DuckDB `IndexStorePort` over `index.duckdb`.
//!
//! EFFECT shell for the `IndexStorePort` trait (`crates/ports`). SYNC (local DB)
//! like `adapter-duckdb`'s `StoragePort`. Operates over a SEPARATE
//! `index.duckdb` file (ADR-023/025) — the indexer NEVER touches the user's
//! `openlore.duckdb`. The index is a re-buildable cache of signature-verified,
//! per-author-attributed PUBLIC claims.
//!
//! ## The load-bearing anti-merging contract (WD-103 / I-AV-2)
//!
//! Every query method returns PER-CLAIM [`IndexedClaim`] rows whose
//! `author_did` is NON-`Option` — two identical-content claims by different
//! authors stay TWO rows. There is NO method that aggregates across authors and
//! NO `consensus` / `merged` table: `distinct_author_count` is composed in the
//! PURE `appview-domain` core, NEVER in SQL. The extended
//! `no_cross_table_join_elides_author` `xtask check-arch` rule scans THIS
//! crate's SQL literals and fails CI on any `indexed_claims` aggregate that
//! drops `author_did`. De-dup at `upsert` is by CID only (ADR-025).
//!
//! ## Architecture (nw-fp-hexagonal-architecture)
//!
//! Pure core (claim-domain, appview-domain) never imports this crate; the
//! indexer composition root wires an [`IndexStoreAdapter`] behind the
//! `IndexStorePort` interface. `appview_domain::compose_results` consumes the
//! attributed rows this adapter returns (the dependency on `appview-domain` is
//! the shared boundary-value home, not a behavioral coupling).
//!
//! Bootstrap SCAFFOLD (step 01-03): the port impl exists so the workspace
//! compiles and the wiring seam is present, but every body is `todo!()`. The
//! `src/schema.rs` DDL is a COMMENT sketch only; the live DDL + query bodies
//! land in step 03-01 (driven by the AV-* search + ingest acceptance scenarios).
//
// SCAFFOLD: true  (adapter skeleton; DDL + query bodies land in step 03-01)

#![allow(dead_code)] // scaffold; real wiring lands in step 03-01
#![forbid(unsafe_code)]

use claim_domain::{Cid, Did};
use ports::{IndexStoreError, IndexStorePort, IndexedClaim, ProbeOutcome};

mod schema;

/// Embedded-DuckDB `IndexStorePort` adapter over the SEPARATE `index.duckdb`
/// (ADR-023/025).
///
/// Bootstrap SCAFFOLD — the open DB handle + colocated `indexed_claims/`
/// artifact-dir fields land with the real wiring in step 03-01.
pub struct IndexStoreAdapter {
    // SCAFFOLD: true — the `index.duckdb` connection handle + the colocated
    // `indexed_claims/<author_did>/` artifact directory root land in step 03-01.
    _scaffold: (),
}

impl IndexStoreAdapter {
    /// Open the index store at the given path; run pending migrations. Bootstrap
    /// SCAFFOLD: the real constructor (DuckDB open + migration v1 from
    /// `schema.rs` + artifact-dir prep) lands in step 03-01.
    pub fn open() -> Result<Self, IndexStoreError> {
        // SCAFFOLD: true
        todo!("IndexStoreAdapter::open — DuckDB open + migration v1 (step 03-01, ADR-025)")
    }
}

impl IndexStorePort for IndexStoreAdapter {
    fn probe(&self) -> ProbeOutcome {
        // SCAFFOLD: true — the Earned-Trust probe (schema version + fsync +
        // attribution round-trip + the no-merge-schema assertion) lands in 03-01.
        todo!("IndexStoreAdapter::probe — Earned-Trust index-store probe (step 03-01)")
    }

    fn upsert(&self, _claim: &IndexedClaim) -> Result<(), IndexStoreError> {
        // SCAFFOLD: true — de-dup-by-CID insert of one verified attributed row
        // (+ its evidence/references children + the JSON artifact) lands in 03-01.
        todo!("IndexStoreAdapter::upsert — de-dup-by-CID attributed insert (step 03-01)")
    }

    fn query_by_object(&self, _object: &str) -> Result<Vec<IndexedClaim>, IndexStoreError> {
        // SCAFFOLD: true — the SAFE per-claim attributed SELECT (explicit
        // author_did; NO aggregation across authors) lands in step 03-01.
        todo!("IndexStoreAdapter::query_by_object — attributed per-claim SELECT (step 03-01)")
    }

    fn query_by_contributor(&self, _did: &Did) -> Result<Vec<IndexedClaim>, IndexStoreError> {
        // SCAFFOLD: true — one DID's whole network trail (attributed rows) — 03-01.
        todo!("IndexStoreAdapter::query_by_contributor — attributed per-claim SELECT (step 03-01)")
    }

    fn query_by_subject(&self, _subject: &str) -> Result<Vec<IndexedClaim>, IndexStoreError> {
        // SCAFFOLD: true — attributed per-claim SELECT by subject — step 03-01.
        todo!("IndexStoreAdapter::query_by_subject — attributed per-claim SELECT (step 03-01)")
    }

    fn get_by_cid(&self, _cid: &Cid) -> Result<Option<IndexedClaim>, IndexStoreError> {
        // SCAFFOLD: true — the `--show` single-row lookup by CID PK — step 03-01.
        todo!("IndexStoreAdapter::get_by_cid — single attributed row by CID PK (step 03-01)")
    }
}
