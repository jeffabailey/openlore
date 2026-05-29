//! `indexed_claim` — the slice-05 verified-indexed-claim boundary value +
//! the search dimension + the counter-relationship annotation.
//!
//! These value types are the single home (`ports`) for the boundary shapes the
//! pure `appview-domain` core, the index-store adapter, and the query handler
//! all share. Hoisted here from `appview-domain` (step 01-02) so `ports` is the
//! non-cyclic owner: `appview-domain -> ports -> {claim-domain, lexicon}` (NEVER
//! the reverse). See `docs/feature/openlore-appview-search/design/data-models.md`
//! §"In-memory value types" + component-boundaries.md §`crates/ports`.
//!
//! ## LOAD-BEARING anti-merging at the type level (WD-120 / I-AV-2)
//!
//! [`IndexedClaim::author_did`] is `Did`, NOT `Option<Did>`: dropping attribution
//! is a COMPILE error, not a runtime check. `verified_against: KeyId` is
//! never-empty (verified-before-index, WD-104 / I-AV-1). There is NO merged /
//! consensus row type anywhere — the absence is the design (WD-103).
//
// SCAFFOLD: false  (data types are real; behavior arrives via the appview-domain core)

use chrono::{DateTime, Utc};
use claim_domain::{Cid, ClaimReference, Did, KeyId, ReferenceType};
use serde::{Deserialize, Serialize};

use crate::AuthorRelationship;

// -----------------------------------------------------------------------------
// Search dimension
// -----------------------------------------------------------------------------

/// The search dimension a query addresses. The single home shared by
/// `IndexQueryPort`/`IndexStorePort` (which key on it) and the pure
/// `appview-domain::compose_results` (which groups by it).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SearchDimension {
    /// By philosophy URI — the headline dimension.
    Object,
    /// By author DID — one developer's whole network trail.
    Contributor,
    /// By project URI.
    Subject,
}

// -----------------------------------------------------------------------------
// Indexed-claim boundary value (an indexed_claims row + a JSON artifact)
// -----------------------------------------------------------------------------

/// The verified, attributed indexed claim — the boundary value `appview-domain`,
/// the index-store adapter, and the query handler share.
///
/// `author_did` is NON-`Option` and LOAD-BEARING (anti-merging, WD-103): it is
/// taken byte-equal from the signed payload's author, never asserted separately.
/// `verified_against` is NEVER empty (verified-before-index, WD-104).
///
/// `PartialEq` (not `Eq`) because of the `f64` confidence (NaN).
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
// Counter-relationship annotation (OD-AV-7 — shown, never applied)
// -----------------------------------------------------------------------------

/// A counter/retract relationship annotation on a result row (OD-AV-7). The
/// annotation is SHOWN, never applied — the countered row is never removed from
/// the result set. Carries the countering claim's CID + its author (the
/// attribution of the counter is itself preserved).
///
/// `ref_type` reuses `claim_domain::ReferenceType` (the SSOT for inter-claim
/// relationship kinds) — no duplicate enum at the appview boundary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CounterRef {
    /// The CID of the claim that counters/retracts this row.
    pub referencing_cid: Cid,
    /// The author of the countering claim (attribution preserved).
    pub counter_author: Did,
    /// The kind of relationship (`Counters` | `Retracts` | ...).
    pub ref_type: ReferenceType,
}
