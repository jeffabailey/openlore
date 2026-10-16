//! The self-attested `org.openlore.claim` record JSON (SPIKE-1 wire shape,
//! ADR-071): `$type`, the claim fields, confidence as integer basis points
//! (ADR-070), the bare-DID author and `references`.
//!
//! Moved here from `review-domain::plans` (github-did-identity-link Q-RM-1)
//! so the identity-link codec can emit the same record; `review-domain`
//! re-exports it, nothing is copied.

use serde_json::{json, Value};

use crate::{ReferenceType, UnsignedClaim};

/// The ATProto collection claims are written to.
pub const CLAIM_COLLECTION: &str = "org.openlore.claim";

/// The self-attested wire shape of a claim (SPIKE-1): `$type`, the claim
/// fields, confidence as basis points, the bare-DID author, `references`.
pub fn claim_record_json(claim: &UnsignedClaim) -> Value {
    let references: Vec<Value> = claim
        .references
        .iter()
        .map(|r| json!({"type": reference_type(r.ref_type), "cid": r.cid.0}))
        .collect();
    json!({
        "$type": CLAIM_COLLECTION,
        "subject": claim.subject,
        "predicate": claim.predicate,
        "object": claim.object,
        "evidence": claim.evidence,
        "confidence": claim.confidence.basis_points(),
        "author": claim.author_did.0,
        "composedAt": claim.composed_at,
        "references": references,
    })
}

/// The lexicon token of a reference type.
pub fn reference_type(ref_type: ReferenceType) -> &'static str {
    match ref_type {
        ReferenceType::Retracts => "retracts",
        ReferenceType::Corrects => "corrects",
        ReferenceType::Counters => "counters",
        ReferenceType::Supersedes => "supersedes",
    }
}
