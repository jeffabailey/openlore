//! Reference-rules validation for `UnsignedClaim` (ADR-008 §Behavioral
//! rule 4 + Earned Trust 2/3).
//!
//! Pure function. NO I/O. NO async.
//!
//! ## Scope (step 03-04, full pure-core arm)
//!
//! This module implements both arms of the sign-time reference-rules
//! contract:
//!
//! 1. **Self-reference rejection** (step 03-03, ADR-008 Earned Trust 2):
//!    a claim whose `references[]` points at its own body CID is
//!    rejected before signing.
//! 2. **Two-hop cycle rejection** (step 03-04, ADR-008 Earned Trust 3):
//!    given a `ClaimLookup` over already-stored claims, if any claim B
//!    referenced by A is itself a claim whose `references[]` points
//!    back at A's body CID, the round-trip A→B→A forms a 2-hop cycle
//!    that the validator rejects before signing.
//!
//! Slice-01 detects cycles up to depth two. Longer cycles are
//! improbable in slice-01's single-author scope; deeper traversal is a
//! slice-04 scoring-graph concern.
//!
//! ## Chicken-and-egg resolution
//!
//! A claim's full canonical-CBOR CID depends on its `references` array,
//! so the literal reading "this reference equals the claim's own CID"
//! is unsatisfiable without a hash collision — the canonical CBOR
//! changes the moment you insert the reference. The semantically
//! useful identity for cycle detection is the claim's **body CID**:
//! the CID of the canonical CBOR computed with `references` cleared to
//! the empty array. That CID is stable regardless of how many
//! retraction / correction annotations the author later attaches, and
//! it captures the rule's intent: "you may not retract / correct /
//! counter / supersede the claim you'd have published without this
//! annotation, even across one hop".
//!
//! Algorithm:
//!
//! 1. Clone the unsigned claim and clear its `references` to `[]`.
//! 2. Canonicalize that body-only clone and compute its CID — the
//!    "body CID" of `claim` (call it `A_body`).
//! 3. Iterate the ORIGINAL claim's `references[]`:
//!    a. If any entry's `cid` equals `A_body`, return
//!    `Err(SelfReference)` (the 03-03 arm).
//!    b. If a `lookup` was supplied, fetch the referenced signed claim
//!    B. Compute B's body CID the same way (clear B's references,
//!    canonicalize, hash). If any of B's references targets
//!    `A_body`, return `Err(CycleDetected { cid: A_body })` (the
//!    03-04 arm).
//!
//! The detection runs at **sign time** (before signature bytes are
//! produced), making the rejection a domain-validation outcome rather
//! than a verify-time signature failure. ADR-008 §Behavioral rule 4 +
//! Earned Trust 2/3.

use crate::{canonicalize, compute_cid, Cid, ClaimError, ClaimLookup, UnsignedClaim};

/// Compute the body CID of an unsigned claim — its canonical-CBOR CID
/// with `references` cleared to `[]`. This is the claim's stable
/// identity used by reference-rules checks (see module docstring).
fn body_cid_of(claim: &UnsignedClaim) -> Result<Cid, ClaimError> {
    let mut body_only = claim.clone();
    body_only.references = Vec::new();
    let body_bytes = canonicalize(&body_only)?;
    Ok(compute_cid(&body_bytes))
}

/// Validate the reference-rules invariants on an unsigned claim.
///
/// Rejects:
/// 1. Self-references — a reference to the claim's own body CID
///    (ADR-008 Earned Trust 2).
/// 2. Two-hop cycles — a reference to a claim B (looked up via
///    `lookup`) whose own references include `claim`'s body CID
///    (ADR-008 Earned Trust 3). The check is skipped when
///    `lookup = None` so unit-test callers without a store can still
///    exercise the self-reference arm.
///
/// ## Errors
///
/// - `ClaimError::SelfReference` if any entry in `claim.references[]`
///   has a `cid` equal to the claim's own body CID.
/// - `ClaimError::CycleDetected { cid }` if a referenced claim B
///   itself references `claim`'s body CID. The reported `cid` is the
///   body CID of `claim` (the apex of the closed loop).
/// - `ClaimError::CanonicalizationFailed { .. }` propagated from
///   [`canonicalize`] (in practice infallible for `Vec<u8>` writes).
pub fn reference_rules_validate(
    claim: &UnsignedClaim,
    lookup: Option<&dyn ClaimLookup>,
) -> Result<(), ClaimError> {
    // Fast path: no references means no rules to check.
    if claim.references.is_empty() {
        return Ok(());
    }

    // Compute `A_body`: the stable identity used by both arms.
    let a_body_cid = body_cid_of(claim)?;

    // Arm 1: self-reference. Any direct reference to A_body fails before
    // we even consult the lookup.
    if claim.references.iter().any(|r| r.cid == a_body_cid) {
        return Err(ClaimError::SelfReference);
    }

    // Arm 2: two-hop cycle. Only checked when a lookup is supplied;
    // unit tests calling with `None` opt out (e.g. when verifying
    // self-reference detection in isolation).
    if let Some(lookup) = lookup {
        for outgoing in &claim.references {
            // Look up the signed claim B that this reference targets.
            // If the store doesn't know it, we can't form a cycle
            // verdict — silently skip (slice-04 may upgrade to a
            // stricter "dangling reference" check).
            let Some(signed_b) = lookup.signed_by_cid(&outgoing.cid) else {
                continue;
            };

            // B's references closing back on A_body is the 2-hop
            // cycle. We compare against A_body, not B's body CID,
            // because the cycle apex we report is A — the claim we
            // were asked to validate.
            if signed_b
                .unsigned
                .references
                .iter()
                .any(|r| r.cid == a_body_cid)
            {
                return Err(ClaimError::CycleDetected { cid: a_body_cid });
            }
        }
    }

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
            reason: None,
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
