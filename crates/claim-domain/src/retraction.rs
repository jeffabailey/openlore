//! `retraction` — the PURE author-withdrawal rules over the reference graph,
//! shared by every surface that must honour a claim author's own withdrawal
//! (ADR-060 D-RF-D3; contributor-philosophy-inference DDD-7 / Q-CPI-D2).
//!
//! Self-retraction rule (D-RF-D3, literal): a claim C is author-self-retracted
//! ⟺ some claim K in the set has `K.author_did == C.author_did` and carries a
//! reference `{ ref_type == Retracts, cid == C.cid }`. A `Counters` reference,
//! or a `Retracts` by a DIFFERENT author, never withdraws C (no heckler's veto —
//! ADR-060 I-RF-4).
//!
//! Callers project their own row type into a borrowed [`ClaimLineage`] (who
//! signed it, its CID, what it references); author DIDs are compared exactly, so
//! callers pass them in one normal form. NO I/O.

use crate::{ClaimReference, ReferenceType};

/// The reference-graph facts of one claim that the withdrawal rules read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClaimLineage<'a> {
    pub author_did: &'a str,
    pub cid: &'a str,
    pub references: &'a [ClaimReference],
}

/// True when `marker` is `target`'s OWN retraction marker (D-RF-D3): signed by
/// the same author and carrying a `Retracts` reference to `target`'s CID.
pub fn is_own_retraction_marker(marker: &ClaimLineage<'_>, target: &ClaimLineage<'_>) -> bool {
    names_by_same_author(marker, target, ReferenceType::Retracts)
}

/// True when `target` is author-self-retracted: some claim in `claims` is its
/// own retraction marker (D-RF-D3).
pub fn is_self_retracted(target: &ClaimLineage<'_>, claims: &[ClaimLineage<'_>]) -> bool {
    claims
        .iter()
        .any(|marker| is_own_retraction_marker(marker, target))
}

/// True when `target` is superseded by its own author (DDD-7): some claim in
/// `claims` is by `target`'s author and carries a `Supersedes` reference to it.
/// A different author's `Supersedes` never replaces someone else's claim.
pub fn is_superseded_by_author(target: &ClaimLineage<'_>, claims: &[ClaimLineage<'_>]) -> bool {
    claims
        .iter()
        .any(|successor| names_by_same_author(successor, target, ReferenceType::Supersedes))
}

/// `referrer` is by `target`'s author and references `target` with `ref_type`.
fn names_by_same_author(
    referrer: &ClaimLineage<'_>,
    target: &ClaimLineage<'_>,
    ref_type: ReferenceType,
) -> bool {
    referrer.author_did == target.author_did
        && referrer
            .references
            .iter()
            .any(|reference| reference.ref_type == ref_type && reference.cid.0 == target.cid)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Cid;
    use proptest::prelude::*;

    /// One claim in a small, colliding universe: (author index, own cid
    /// index, optional (reference type, referenced cid index)).
    type Spec = (usize, usize, Option<(ReferenceType, usize)>);

    const AUTHORS: [&str; 3] = ["did:plc:maria", "did:plc:rachel", "did:plc:sven"];
    const CIDS: [&str; 4] = ["bafya", "bafyb", "bafyc", "bafyd"];

    fn arb_specs() -> impl Strategy<Value = Vec<Spec>> {
        let ref_type = prop_oneof![
            Just(ReferenceType::Retracts),
            Just(ReferenceType::Counters),
            Just(ReferenceType::Supersedes),
            Just(ReferenceType::Corrects),
        ];
        proptest::collection::vec(
            (
                0..AUTHORS.len(),
                0..CIDS.len(),
                proptest::option::of((ref_type, 0..CIDS.len())),
            ),
            0..8,
        )
    }

    fn references_of(spec: &Spec) -> Vec<ClaimReference> {
        spec.2
            .iter()
            .map(|(ref_type, target)| ClaimReference {
                ref_type: *ref_type,
                cid: Cid(CIDS[*target].to_string()),
            })
            .collect()
    }

    /// Oracle straight from the rule's text: ∃ a same-author claim carrying
    /// `{ ref_type, target cid }`.
    fn withdrawn_by_author(target: &Spec, specs: &[Spec], ref_type: ReferenceType) -> bool {
        specs
            .iter()
            .any(|s| s.0 == target.0 && s.2 == Some((ref_type, target.1)))
    }

    proptest! {
        /// D-RF-D3 + DDD-7: a claim is self-retracted (resp. superseded) exactly
        /// when its OWN author retracts (resp. supersedes) its CID — a
        /// different author's `Retracts`/`Supersedes`, or any `Counters`/
        /// `Corrects`, never withdraws it (no heckler's veto, I-RF-4).
        #[test]
        fn only_the_claims_own_author_can_withdraw_it(specs in arb_specs()) {
            let references: Vec<Vec<ClaimReference>> = specs.iter().map(references_of).collect();
            let lineages: Vec<ClaimLineage<'_>> = specs
                .iter()
                .zip(&references)
                .map(|(spec, refs)| ClaimLineage {
                    author_did: AUTHORS[spec.0],
                    cid: CIDS[spec.1],
                    references: refs,
                })
                .collect();
            for (spec, lineage) in specs.iter().zip(&lineages) {
                prop_assert_eq!(
                    is_self_retracted(lineage, &lineages),
                    withdrawn_by_author(spec, &specs, ReferenceType::Retracts)
                );
                prop_assert_eq!(
                    is_superseded_by_author(lineage, &lineages),
                    withdrawn_by_author(spec, &specs, ReferenceType::Supersedes)
                );
            }
        }
    }
}
