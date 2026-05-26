//! Reference-rules validation for `UnsignedClaim` (ADR-008 §Behavioral
//! rule 4 + Earned Trust 2).
//!
//! Pure function. NO I/O. NO async.
//!
//! ## Scope (step 03-03)
//!
//! This module implements the **self-reference** half of the
//! reference-rules contract: a claim whose `references[]` array points
//! at its own would-be CID must be rejected BEFORE the signature is
//! computed. The two-hop cycle detection arm (step 03-04) extends this
//! module to consume the `lookup` argument; step 03-03 only honours
//! `lookup = None`.
//!
//! ## Chicken-and-egg resolution
//!
//! A claim's full canonical-CBOR CID depends on its `references` array,
//! so the literal reading "this reference equals the claim's own CID"
//! is unsatisfiable without a hash collision — the canonical CBOR
//! changes the moment you insert the reference. The semantically
//! useful self-reference check is over the claim's **body identity**:
//! the CID of the canonical CBOR computed with `references` cleared to
//! the empty array. That CID is stable regardless of how many
//! retraction / correction annotations the author later attaches, and
//! it captures the rule's intent: "you may not retract / correct /
//! counter / supersede the claim you'd have published without this
//! annotation".
//!
//! Algorithm:
//!
//! 1. Clone the unsigned claim and clear its `references` to `[]`.
//! 2. Canonicalize that body-only clone and compute its CID — the
//!    "body CID".
//! 3. Iterate the ORIGINAL claim's `references[]`; if any entry's
//!    `cid` equals the body CID, return `Err(SelfReference)`.
//!
//! The detection runs at **sign time** (before signature bytes are
//! produced), making the rejection a domain-validation outcome rather
//! than a verify-time signature failure. ADR-008 §Behavioral rule 4 +
//! Earned Trust 2.

use crate::{canonicalize, compute_cid, ClaimError, ClaimLookup, UnsignedClaim};

/// Validate the reference-rules invariants on an unsigned claim.
///
/// **Step 03-03 scope**: rejects self-references. The `lookup`
/// parameter is reserved for the two-hop cycle detection arm landing
/// in step 03-04 and is currently unused.
///
/// ## Errors
///
/// - `ClaimError::SelfReference` if any entry in `claim.references[]`
///   has a `cid` equal to the claim's own unsigned CID.
/// - `ClaimError::CanonicalizationFailed { .. }` propagated from
///   [`canonicalize`] (in practice infallible for `Vec<u8>` writes).
pub fn reference_rules_validate(
    claim: &UnsignedClaim,
    _lookup: Option<&dyn ClaimLookup>,
) -> Result<(), ClaimError> {
    // Fast path: no references means no rules to check.
    if claim.references.is_empty() {
        return Ok(());
    }

    // 1. Build a body-only clone with `references` cleared. The CID of
    //    this body-only clone is what we use as the claim's stable
    //    identity for self-reference detection — see module docstring.
    let mut body_only = claim.clone();
    body_only.references = Vec::new();

    // 2. Canonicalize + CID over the body-only clone. This "body CID"
    //    is stable regardless of how many references the author
    //    attaches, which is what makes the rule testable and
    //    mathematically meaningful.
    let body_bytes = canonicalize(&body_only)?;
    let body_cid = compute_cid(&body_bytes);

    // 3. Scan the ORIGINAL claim's references for any entry pointing
    //    at the body CID. Equality is exact-string match on the CID
    //    wire form (multibase base32-lower).
    if claim.references.iter().any(|r| r.cid == body_cid) {
        return Err(ClaimError::SelfReference);
    }

    // Two-hop cycle detection (consumer of `_lookup`) lands in step 03-04.
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Cid, ClaimReference, Confidence, Did, ReferenceType};

    fn sample_unsigned() -> UnsignedClaim {
        UnsignedClaim {
            subject: "github:openlore/openlore".into(),
            predicate: "embodiesPhilosophy".into(),
            object: "org.openlore.philosophy.memory-safety".into(),
            evidence: vec!["https://example.org/evidence/1".into()],
            confidence: Confidence(0.75),
            author_did: Did("did:plc:jeff#org.openlore.application".into()),
            composed_at: "2026-05-25T12:00:00Z".into(),
            references: Vec::new(),
        }
    }

    /// Happy path: no references → Ok.
    #[test]
    fn validate_returns_ok_when_no_references() {
        let claim = sample_unsigned();
        assert!(reference_rules_validate(&claim, None).is_ok());
    }

    /// Happy path: references point at OTHER CIDs → Ok.
    #[test]
    fn validate_returns_ok_when_references_target_other_cids() {
        let mut claim = sample_unsigned();
        claim.references.push(ClaimReference {
            ref_type: ReferenceType::Supersedes,
            cid: Cid("bafyreidifferentcidxxxxxxxxxxxxxxxxxxxx".into()),
        });
        assert!(reference_rules_validate(&claim, None).is_ok());
    }

    /// Rejection: a reference whose CID equals the claim's body CID
    /// (CID of the claim with `references = []`) → `Err(SelfReference)`.
    /// Mirrors LC-6.
    #[test]
    fn validate_rejects_self_reference() {
        // 1. Start with the body — references = [].
        let body = sample_unsigned();
        // 2. Compute the stable body CID (refs cleared).
        let body_bytes = canonicalize(&body).expect("canonicalize body");
        let body_cid = compute_cid(&body_bytes);
        // 3. Build the attacker's claim: body + a reference pointing
        //    at the body CID.
        let mut attack = body;
        attack.references.push(ClaimReference {
            ref_type: ReferenceType::Retracts,
            cid: body_cid,
        });

        let result = reference_rules_validate(&attack, None);
        assert!(
            matches!(result, Err(ClaimError::SelfReference)),
            "expected Err(SelfReference), got {:?}",
            result
        );
    }
}
