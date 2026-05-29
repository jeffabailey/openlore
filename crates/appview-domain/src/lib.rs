//! `appview-domain` — the pure ingest-gate + anti-merging search core.
//!
//! Slice-05's pure core: the symmetric counterpart to slice-02's
//! `scraper-domain` and slice-04's `scoring`. It holds two pure concerns and
//! NOTHING else:
//!
//! 1. **The verify-before-index gate** ([`ingest_decision`]): the pure
//!    `RawRecord -> IngestOutcome` decision that reuses `claim_domain::verify`
//!    + `compute_cid` (the SAME pure core — NO second verification path,
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
mod suggest;

pub use compose::compose_results;
pub use ingest::ingest_decision;
pub use suggest::near_match_suggestion;

use chrono::{DateTime, Utc};
use claim_domain::{Cid, ClaimReference, Did, KeyId, SignedClaim};
use serde::{Deserialize, Serialize};

// -----------------------------------------------------------------------------
// Ingest ADTs (the verify-before-index decision — WD-104)
// -----------------------------------------------------------------------------

/// A fetched-but-not-yet-verified network record — the `IngestSourcePort::enumerate`
/// output the ingest gate consumes. Transient; NEVER persisted as-is.
///
/// The 01-02 step hoists this to `ports` (the home shared by the ingest adapter
/// and the pure gate); declared here at bootstrap so `appview-domain` compiles
/// self-contained before that reconciliation.
///
/// `PartialEq` (not `Eq`) because `SignedClaim` carries an `f64` confidence.
#[derive(Debug, Clone, PartialEq)]
pub struct RawRecord {
    /// The network-published rkey/CID (recomputed + verified at ingest).
    pub published_cid: Cid,
    /// The signed-claim value (author/subject/object/confidence/signature/...).
    pub raw_payload: SignedClaim,
    /// The PDS/relay URL it was pulled from (provenance; NOT signed).
    pub source_pds: String,
}

/// The PURE verify-before-index gate decision (WD-104). `Index` carries the
/// verified, attributed claim whose `author_did` is taken from the SIGNED
/// payload; `Reject` carries the structured reason. There is no third state —
/// a record is either admitted with full attribution or rejected.
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
// Indexed-claim boundary value (an indexed_claims row + a JSON artifact)
// -----------------------------------------------------------------------------

/// The verified, attributed indexed claim — the boundary value `appview-domain`,
/// the index-store adapter, and the query handler share.
///
/// `author_did` is NON-`Option` and LOAD-BEARING (anti-merging, WD-103): it is
/// taken byte-equal from the signed payload's author, never asserted separately.
/// `verified_against` is NEVER empty (verified-before-index, WD-104). Declared
/// here at bootstrap; hoisted to `ports` in 01-02.
///
/// `PartialEq` (not `Eq`) because of the `f64` confidence.
#[derive(Debug, Clone, PartialEq)]
pub struct IndexedClaim {
    /// NON-`Option`; LOAD-BEARING (anti-merging, WD-103); == signed payload author.
    pub author_did: Did,
    /// Verified `== compute_cid(payload)` (WD-104).
    pub cid: Cid,
    pub subject: String,
    pub predicate: String,
    pub object: String,
    /// Numeric `[0.0, 1.0]` (WD-10 / I-6) — the display bucket is render-only.
    pub confidence: f64,
    pub composed_at: DateTime<Utc>,
    /// The DID-doc key id the signature verified against (ADR-026); NEVER empty (WD-104).
    pub verified_against: KeyId,
    pub evidence: Vec<String>,
    /// For the OD-AV-7 counter annotation.
    pub references: Vec<ClaimReference>,
    /// Resolved CLI-side (you/subscribed-peer/unsubscribed-cache/network-unfollowed).
    pub relationship: AuthorRelationship,
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

/// The search dimension a query addresses. Declared here at bootstrap; hoisted
/// to `ports` in 01-02 (the home shared by `IndexQueryPort`/`IndexStorePort`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SearchDimension {
    /// By philosophy URI — the headline dimension.
    Object,
    /// By author DID — one developer's whole network trail.
    Contributor,
    /// By project URI.
    Subject,
}

/// A counter/retract relationship annotation on a result row (OD-AV-7). The
/// annotation is SHOWN, never applied — the countered row is never removed from
/// the result set. Carries the countering claim's CID + its author (the
/// attribution of the counter is itself preserved).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CounterRef {
    /// The CID of the claim that counters/retracts this row.
    pub referencing_cid: Cid,
    /// The author of the countering claim (attribution preserved).
    pub counter_author: Did,
    /// The kind of relationship (`Counters` | `Retracts`).
    pub ref_type: ReferenceType,
}

/// The subset of inter-claim relationships the counter annotation surfaces.
/// Mirrors `claim_domain::ReferenceType` (`Counters`/`Retracts`); declared
/// locally so the annotation stays self-contained at the appview boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReferenceType {
    Counters,
    Retracts,
}

// -----------------------------------------------------------------------------
// Author relationship (slice-03 enum + the slice-05 NetworkUnfollowed variant)
// -----------------------------------------------------------------------------

/// How a result's author relates to the local user.
///
/// `NetworkUnfollowed` is the slice-05 addition: an author present in the
/// network index whom the user does NOT subscribe to → drives the
/// `(not subscribed)` label + the `peer add` follow affordance (US-AV-005). The
/// relationship is resolved CLI-side by checking the result's `author_did`
/// against the user's `peer_subscriptions`; the index itself is per-user-neutral.
///
/// Declared here at bootstrap as the 4-variant slice-05 set; the 01-02 step
/// reconciles this with `ports::AuthorRelationship` (which gains the
/// `NetworkUnfollowed` variant there).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuthorRelationship {
    You,
    SubscribedPeer,
    UnsubscribedCache,
    NetworkUnfollowed,
}
