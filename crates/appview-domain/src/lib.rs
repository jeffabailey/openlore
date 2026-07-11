//! `appview-domain` — the pure ingest-gate + anti-merging search core.
//!
//! Slice-05's pure core: the symmetric counterpart to slice-02's
//! `scraper-domain` and slice-04's `scoring`. It holds two pure concerns and
//! NOTHING else:
//!
//! 1. **The verify-before-index gate** ([`ingest_decision`]): the pure
//!    `RawRecord -> IngestOutcome` decision that reuses `claim_domain::verify`
//!    and `compute_cid` (the SAME pure core — NO second verification path,
//!    WD-104). A record enters the index ONLY when its signature verifies
//!    against the resolved key AND its recomputed CID matches the published
//!    CID; otherwise it is `Reject`ed.
//!
//! 2. **The anti-merging search composition** ([`compose_results`]): groups
//!    attributed rows by author, NEVER merges authors, and computes
//!    `distinct_author_count` from the rows. Two identical-content claims by
//!    different authors stay as two rows (WD-103). Plus [`near_match_suggestion`]
//!    for an empty dimension result.
//!
//! NO I/O. NO async. NO persistence. NO knowledge of DuckDB/HTTP/the network
//! (ADR-007 + ADR-009 hexagonal pure core). The composition root
//! (`crates/openlore-indexer`) wires the effect shell around this.
//!
//! ## LOAD-BEARING anti-merging at the type level (WD-120 layer 1)
//!
//! Every result-bearing type ([`IndexedClaim`], [`NetworkResultRow`]) carries
//! `author_did: Did` as a NON-`Option` field. Dropping attribution is a compile
//! error, not a runtime check. `compose_results` returns a per-author structure
//! ([`NetworkSearchResult::by_author`]) with NO merged-row API — there is no way
//! to construct a faceless "network consensus" row (WD-103 / I-AV-2 layer 1).
//!
//! Bootstrap (step 01-01): the ADTs land here with `todo!()`-bodied entry
//! points marked `// SCAFFOLD: true`. The behavior is driven fully by the
//! Phase 02+ AVC-*/AV-* acceptance scenarios (ingest gate, anti-merging
//! grouping, near-match suggestion). The 01-02 step reconciles the boundary
//! value types (`IndexedClaim`, `SearchDimension`, `KeyId`,
//! `AuthorRelationship`) with their `ports` home.
//
// SCAFFOLD: true

#![allow(dead_code)] // scaffolds; usage lands in subsequent DELIVER steps
#![forbid(unsafe_code)]

mod compose;
mod ingest;
mod retraction;
mod suggest;

// Step 02-01 (AVC-1): proptest strategies for the verify-before-index gate's
// `@property` scenarios. `pub` so the layer-2 acceptance test reaches
// `arbitrary_raw_records()` directly via the pure-core import path (mirrors
// slice-01's `claim_domain::proptest_strategies`). proptest is a regular dep
// (pure-CPU; not on the check-arch banned-I/O list) — the pure core stays pure.
pub mod proptest_strategies;

pub use compose::compose_results;
pub use ingest::ingest_decision;
pub use retraction::{partition_retracted, RetractionPartition};
pub use suggest::near_match_suggestion;

use claim_domain::{Cid, Did, KeyId};
use serde::{Deserialize, Serialize};

// -----------------------------------------------------------------------------
// Hoisted boundary types — single home is `ports` (step 01-02)
// -----------------------------------------------------------------------------
//
// `RawRecord`, `IndexedClaim`, `SearchDimension`, `CounterRef`, and
// `AuthorRelationship` were temporarily declared here at the 01-01 bootstrap so
// `appview-domain` compiled self-contained. Step 01-02 reconciles them: their
// single home is now `ports` (the boundary shapes the ingest adapter, the
// index-store adapter, and this pure core all share — `appview-domain -> ports`,
// never the reverse). `ReferenceType` was a DUPLICATE of `claim_domain::ReferenceType`
// — deleted; `ports::CounterRef.ref_type` reuses the claim-domain SSOT. These
// re-exports preserve the existing `appview_domain::{...}` public surface +
// every `crate::`-path reference in `ingest.rs`/`compose.rs`/`suggest.rs`.
pub use claim_domain::ReferenceType;
pub use ports::{AuthorRelationship, CounterRef, IndexedClaim, RawRecord, SearchDimension};

// -----------------------------------------------------------------------------
// Ingest ADTs (the verify-before-index decision — WD-104)
// -----------------------------------------------------------------------------

/// The PURE verify-before-index gate decision (WD-104). `Index` carries the
/// verified, attributed claim whose `author_did` is taken from the SIGNED
/// payload; `Reject` carries the structured reason. There is no third state —
/// a record is either admitted with full attribution or rejected.
// reason: large_enum_variant — `IngestOutcome` is short-lived (produced per-record
// by `ingest_decision`, immediately matched, NEVER stored in bulk), so the
// `Index(IndexedClaim)` vs `Reject(RejectReason)` size gap costs nothing. Boxing
// the large variant would ripple a `Box<IndexedClaim>` deref through ~10 match
// arms across `openlore-indexer` + the AVC-* acceptance tests (several of which
// bind `Index(claim) => claim` as a value); not worth churning a stable contract.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, PartialEq)]
pub enum IngestOutcome {
    /// Verified + CID-matched; ready to enter the index.
    Index(IndexedClaim),
    /// Rejected at the gate; never enters the index, never appears in a search.
    Reject(RejectReason),
}

/// Why the verify-before-index gate rejected a record. Each variant is a
/// distinct failure of the WD-104 contract; modeled as a choice type so the
/// renderer and the KPI-AV-3 telemetry can distinguish them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RejectReason {
    /// The record carries no usable signature block.
    Unsigned,
    /// The Ed25519 signature did not verify against the resolved key.
    BadSignature,
    /// The recomputed CID does not match the published CID (tamper/mismatch).
    CidMismatch,
    /// The record does not match the `org.openlore.claim` Lexicon shape.
    SchemaUnknown,
}

// -----------------------------------------------------------------------------
// Search ADTs (computed per query; NEVER persisted)
// -----------------------------------------------------------------------------

/// One search result row. NON-`Option` `author_did` (LOAD-BEARING) — the unit
/// the renderer emits. `verified_against` drives the `[verified]` marker (never
/// empty). `counter_annotation` is shown, never applied (OD-AV-7).
///
/// `PartialEq` (not `Eq`) because of the `f64` confidence.
#[derive(Debug, Clone, PartialEq)]
pub struct NetworkResultRow {
    /// NON-`Option`; LOAD-BEARING.
    pub author_did: Did,
    pub cid: Cid,
    pub subject: String,
    pub predicate: String,
    pub object: String,
    pub confidence: f64,
    /// Drives the `[verified]` marker (never empty).
    pub verified_against: KeyId,
    pub relationship: AuthorRelationship,
    /// OD-AV-7: shown, never applied.
    pub counter_annotation: Option<CounterRef>,
}

/// The per-author-grouped search result. `by_author` carries each author's rows
/// under their DID — there is NO merged-author row, by construction (WD-103).
/// `distinct_author_count` is a COUNT over attributed rows, NEVER a merge.
///
/// `PartialEq` (not `Eq`) because `NetworkResultRow` carries an `f64`.
#[derive(Debug, Clone, PartialEq)]
pub struct NetworkSearchResult {
    /// Per-author; NO merged-author row exists.
    pub by_author: Vec<(Did, Vec<NetworkResultRow>)>,
    /// COUNT over attributed rows; never a merge.
    pub distinct_author_count: u32,
    pub total_claims: u32,
    /// Near-match suggestion for an empty result (US-AV-002 Ex 4).
    pub suggestion: Option<String>,
}
