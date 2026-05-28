//! Counter-claim validation for `UnsignedClaim` (ADR-015 / WD-34).
//!
//! Pure function. NO I/O. NO async.
//!
//! ## What a counter-claim is
//!
//! A counter-claim is the author's OWN claim that carries a
//! `references[]` entry of type [`ReferenceType::Counters`] pointing at
//! the CID of the claim being countered (`target_cid`), plus a mandatory
//! free-text `reason`. It NEVER overwrites the target — counter-claims
//! coexist with the claims they counter.
//!
//! ## Validation contract (component-boundaries §`crates/claim-domain`)
//!
//! [`validate_counter_claim`] REJECTS in two cases beyond the existing
//! reference-rules:
//!
//! 1. **Missing reason** — if `references[]` contains a `Counters` entry
//!    AND the (NFC-normalized) `reason` is `None` or empty, return
//!    [`ClaimError::CounterReasonMissing`]. A counter MUST explain
//!    itself (ADR-015). The emptiness test uses [`normalize_reason`] so
//!    a reason of only-combining-or-whitespace that normalizes away is
//!    treated as empty too.
//!
//! 2. **Self-counter** — if the `target_cid` resolves (via the
//!    [`ClaimLookup`], which spans the author's own store AND the peer
//!    store) to a claim whose `author_did == current_user_did`, return
//!    [`ClaimError::SelfCounter`] (with a hint to use `claim retract`).
//!    You cannot counter your own claim (WD-34).
//!
//! Cycle / self-reference detection is DELEGATED to the existing
//! [`reference_rules_validate`] — this function does not reimplement it.
//!
//! ## Why this runs in the pure core (WD-34, belt-and-braces)
//!
//! The CLI verb invokes this BEFORE rendering the compose preview so the
//! user sees a clear error instead of a useless preview; the domain
//! layer rejecting at sign-time is the safety net. Both surfaces share
//! THIS one pure function, so there is no parallel validation path.

use crate::{
    normalize_reason, reference_rules_validate, ClaimError, ClaimLookup, Did, ReferenceType,
    UnsignedClaim,
};

/// Validate a (possibly-counter) unsigned claim before signing.
///
/// Runs three checks in order:
///
/// 1. Delegates to [`reference_rules_validate`] for self-reference and
///    two-hop cycle detection (ADR-008). The `lookup` is forwarded so
///    the cycle arm is active.
/// 2. If the claim is a counter-claim (its `references[]` carry a
///    [`ReferenceType::Counters`] entry), requires a non-empty
///    NFC-normalized `reason` — else [`ClaimError::CounterReasonMissing`].
/// 3. For every `Counters` target, resolves it via `lookup`; if the
///    resolved claim's `author_did` equals `current_user_did`, the user
///    is countering their own claim — [`ClaimError::SelfCounter`].
///
/// A NON-counter claim (no `Counters` reference) never needs a reason
/// and never triggers the self-counter check: this function then reduces
/// to `reference_rules_validate`.
///
/// ## Errors
///
/// - [`ClaimError::CounterReasonMissing`] — counter-claim with no/empty
///   reason.
/// - [`ClaimError::SelfCounter`] — counter target authored by the
///   current user.
/// - [`ClaimError::SelfReference`] / [`ClaimError::CycleDetected`] —
///   propagated from [`reference_rules_validate`].
/// - [`ClaimError::CanonicalizationFailed`] — propagated from
///   canonicalization inside the reference-rules check.
pub fn validate_counter_claim(
    claim: &UnsignedClaim,
    lookup: &dyn ClaimLookup,
    current_user_did: &Did,
) -> Result<(), ClaimError> {
    // 1. Reuse the slice-01 reference-rules (self-reference + 2-hop
    //    cycle). We pass the lookup so the cycle arm is live.
    reference_rules_validate(claim, Some(lookup))?;

    // A claim is a counter-claim iff it carries at least one `Counters`
    // reference. Non-counter claims need neither a reason nor the
    // self-counter check, so we short-circuit.
    if !is_counter_claim(claim) {
        return Ok(());
    }

    // 2. A counter-claim MUST carry a non-empty reason.
    require_non_empty_reason(claim.reason.as_deref())?;

    // 3. Reject countering one's own claim.
    reject_self_counter(claim, lookup, current_user_did)
}

/// True when the claim carries at least one [`ReferenceType::Counters`]
/// reference — the structural marker of a counter-claim.
fn is_counter_claim(claim: &UnsignedClaim) -> bool {
    claim
        .references
        .iter()
        .any(|r| r.ref_type == ReferenceType::Counters)
}

/// Reject a counter-claim whose reason is absent or normalizes to empty.
///
/// The emptiness test runs over the NFC-normalized reason so that a
/// reason consisting only of characters that normalize away (or pure
/// whitespace) is treated as empty — the same normalization the compose
/// preview and signing apply (WD-35).
fn require_non_empty_reason(reason: Option<&str>) -> Result<(), ClaimError> {
    match reason {
        Some(raw) if !normalize_reason(raw).trim().is_empty() => Ok(()),
        _ => Err(ClaimError::CounterReasonMissing),
    }
}

/// Reject the case where any `Counters` target resolves (via `lookup`)
/// to a claim authored by `current_user_did`.
///
/// A target the lookup does not know is simply skipped here — resolving
/// a missing target is a CLI-layer concern (it surfaces a different
/// "unknown target" error pre-compose); this pure check only adjudicates
/// the self-counter rule for targets the store CAN resolve.
fn reject_self_counter(
    claim: &UnsignedClaim,
    lookup: &dyn ClaimLookup,
    current_user_did: &Did,
) -> Result<(), ClaimError> {
    for target in claim
        .references
        .iter()
        .filter(|r| r.ref_type == ReferenceType::Counters)
    {
        if let Some(resolved) = lookup.signed_by_cid(&target.cid) {
            if &resolved.unsigned.author_did == current_user_did {
                return Err(ClaimError::SelfCounter);
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        canonicalize, compute_cid, sign, Cid, ClaimReference, Confidence, SignatureBlock,
        SignedClaim, SigningKey, VerifyingKey,
    };
    use proptest::prelude::*;
    use std::collections::HashMap;

    // -------------------------------------------------------------------
    // Fake ClaimLookup — a pure in-memory map (Meszaros "fake"), NOT a
    // mock library. Satisfies the port (trait) contract; unit tests pass
    // it as `&dyn ClaimLookup`. An EMPTY lookup resolves nothing, which
    // models "target not in any store".
    // -------------------------------------------------------------------
    #[derive(Default)]
    struct FakeLookup {
        by_cid: HashMap<String, SignedClaim>,
    }

    impl FakeLookup {
        fn with(cid: &Cid, claim: SignedClaim) -> Self {
            let mut by_cid = HashMap::new();
            by_cid.insert(cid.0.clone(), claim);
            Self { by_cid }
        }
    }

    impl ClaimLookup for FakeLookup {
        fn signed_by_cid(&self, cid: &Cid) -> Option<SignedClaim> {
            self.by_cid.get(&cid.0).cloned()
        }
    }

    fn me() -> Did {
        Did("did:plc:jeff#org.openlore.application".into())
    }

    fn someone_else() -> Did {
        Did("did:plc:peer-alice#org.openlore.application".into())
    }

    /// A minimal unsigned claim authored by `author`, with NO references
    /// and NO reason (the slice-01 shape).
    fn unsigned_by(author: &Did) -> UnsignedClaim {
        UnsignedClaim {
            subject: "github:openlore/openlore".into(),
            predicate: "embodiesPhilosophy".into(),
            object: "org.openlore.philosophy.memory-safety".into(),
            evidence: vec!["https://example.org/evidence/1".into()],
            confidence: Confidence(0.75),
            author_did: author.clone(),
            composed_at: "2026-05-26T12:00:00Z".into(),
            references: Vec::new(),
            reason: None,
        }
    }

    /// Wrap an unsigned claim in a signed envelope. The signature bytes
    /// are real (sign over the claim's CID) so the fixture is faithful,
    /// but only `unsigned.author_did` is load-bearing for these tests.
    fn signed_of(unsigned: UnsignedClaim) -> SignedClaim {
        // Deterministic 32-byte seed → Ed25519 signing key.
        let signing = SigningKey(vec![7u8; 32]);
        let canonical = canonicalize(&unsigned).expect("canonicalize");
        let cid = compute_cid(&canonical);
        let signature: SignatureBlock = sign(&cid, &signing).expect("sign");
        SignedClaim {
            unsigned,
            signature,
        }
    }

    /// Build a counter-claim authored by `me` that counters `target_cid`,
    /// with the given `reason`.
    fn counter_claim(target_cid: &Cid, reason: Option<&str>) -> UnsignedClaim {
        let mut claim = unsigned_by(&me());
        claim.references.push(ClaimReference {
            ref_type: ReferenceType::Counters,
            cid: target_cid.clone(),
        });
        claim.reason = reason.map(str::to_string);
        claim
    }

    fn target_cid() -> Cid {
        Cid("bafytargetcidxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx".into())
    }

    // -------------------------------------------------------------------
    // Behavior 1: missing / empty reason on a Counters claim → reject.
    // -------------------------------------------------------------------

    /// A counter-claim with `reason: None` is rejected with
    /// `CounterReasonMissing`, even though the target is a stranger's
    /// claim (so the self-counter arm would otherwise pass). Mirrors the
    /// reason arm of the named scenario.
    #[test]
    fn rejects_counter_claim_with_no_reason() {
        let target = target_cid();
        // Lookup resolves the target to SOMEONE ELSE — not a self-counter.
        let lookup = FakeLookup::with(&target, signed_of(unsigned_by(&someone_else())));
        let claim = counter_claim(&target, None);

        let result = validate_counter_claim(&claim, &lookup, &me());
        assert!(
            matches!(result, Err(ClaimError::CounterReasonMissing)),
            "expected Err(CounterReasonMissing), got {result:?}"
        );
    }

    /// Property: for ANY whitespace-only reason on a counter-claim, the
    /// validator rejects with `CounterReasonMissing`. Generalizes the
    /// single empty-string example over the equivalence class of
    /// "reason that normalizes to empty".
    proptest! {
        #[test]
        fn rejects_counter_claim_with_blank_reason(ws in "[ \t\n\r]{0,12}") {
            let target = target_cid();
            let lookup = FakeLookup::with(&target, signed_of(unsigned_by(&someone_else())));
            let claim = counter_claim(&target, Some(&ws));

            let result = validate_counter_claim(&claim, &lookup, &me());
            prop_assert!(
                matches!(result, Err(ClaimError::CounterReasonMissing)),
                "blank reason {ws:?} must be rejected, got {result:?}"
            );
        }
    }

    // -------------------------------------------------------------------
    // Behavior 2: target resolves to current user → self-counter reject.
    // -------------------------------------------------------------------

    /// A counter-claim whose target resolves (via lookup) to a claim the
    /// CURRENT USER authored is rejected with `SelfCounter` — even with a
    /// perfectly good reason. Mirrors the self-counter arm of the named
    /// scenario.
    #[test]
    fn rejects_self_counter_when_target_authored_by_current_user() {
        let target = target_cid();
        // Lookup resolves the target to a claim authored by ME.
        let lookup = FakeLookup::with(&target, signed_of(unsigned_by(&me())));
        let claim = counter_claim(&target, Some("I changed my mind, this is wrong"));

        let result = validate_counter_claim(&claim, &lookup, &me());
        assert!(
            matches!(result, Err(ClaimError::SelfCounter)),
            "expected Err(SelfCounter), got {result:?}"
        );
    }

    /// Self-counter check fires regardless of which store backs the
    /// lookup: the cross-store span is the lookup's concern, so any
    /// resolution to the current user is rejected. Property over an
    /// arbitrary current-user DID.
    proptest! {
        #[test]
        fn self_counter_rejected_for_any_user_did(
            user_suffix in "[a-z0-9-]{4,16}"
        ) {
            let user = Did(format!("did:plc:{user_suffix}#org.openlore.application"));
            let target = target_cid();
            let lookup = FakeLookup::with(&target, signed_of(unsigned_by(&user)));
            let mut claim = unsigned_by(&user);
            claim.references.push(ClaimReference {
                ref_type: ReferenceType::Counters,
                cid: target.clone(),
            });
            claim.reason = Some("solid reasoning here".into());

            let result = validate_counter_claim(&claim, &lookup, &user);
            prop_assert!(
                matches!(result, Err(ClaimError::SelfCounter)),
                "countering own claim must be rejected for {user:?}, got {result:?}"
            );
        }
    }

    // -------------------------------------------------------------------
    // Behavior 3: valid counter-claim against a peer → Ok.
    // -------------------------------------------------------------------

    /// A counter-claim with a real reason, targeting a claim authored by
    /// SOMEONE ELSE, passes. This is the happy path.
    #[test]
    fn accepts_valid_counter_against_other_authors_claim() {
        let target = target_cid();
        let lookup = FakeLookup::with(&target, signed_of(unsigned_by(&someone_else())));
        let claim = counter_claim(&target, Some("The cited benchmark was retracted"));

        let result = validate_counter_claim(&claim, &lookup, &me());
        assert!(result.is_ok(), "valid counter must pass, got {result:?}");
    }

    /// Unknown target (lookup resolves nothing): the self-counter check
    /// cannot fire, so a counter with a good reason passes here — the
    /// "unknown target" error is a CLI-layer concern, not this pure
    /// function's. With an EMPTY lookup the result is Ok.
    #[test]
    fn accepts_counter_when_target_not_resolvable() {
        let lookup = FakeLookup::default();
        let claim = counter_claim(&target_cid(), Some("Disagree, here is why"));

        let result = validate_counter_claim(&claim, &lookup, &me());
        assert!(
            result.is_ok(),
            "unresolvable target is not a self-counter, got {result:?}"
        );
    }

    // -------------------------------------------------------------------
    // Behavior 4: NON-counter claim needs no reason → Ok.
    // -------------------------------------------------------------------

    /// A plain claim (no `Counters` reference) with `reason: None` is
    /// fine — reason is required ONLY for counter-claims. Property over
    /// arbitrary non-counter reference sets (Retracts / Corrects /
    /// Supersedes) proves the reason rule keys on `Counters` alone.
    proptest! {
        #[test]
        fn non_counter_claim_needs_no_reason(
            kinds in proptest::collection::vec(
                prop_oneof![
                    Just(ReferenceType::Retracts),
                    Just(ReferenceType::Corrects),
                    Just(ReferenceType::Supersedes),
                ],
                0..4,
            )
        ) {
            let lookup = FakeLookup::default();
            let mut claim = unsigned_by(&me());
            // Distinct, non-self CIDs so reference_rules stays happy.
            for (i, kind) in kinds.into_iter().enumerate() {
                claim.references.push(ClaimReference {
                    ref_type: kind,
                    cid: Cid(format!("bafyother{i:040}")),
                });
            }
            claim.reason = None; // no reason, no Counters → must be Ok

            let result = validate_counter_claim(&claim, &lookup, &me());
            prop_assert!(
                result.is_ok(),
                "non-counter claim must not require a reason, got {result:?}"
            );
        }
    }

    // -------------------------------------------------------------------
    // The named scenario: combines the two rejection arms in one test so
    // the step's `scenario_name` maps to an executable assertion.
    // -------------------------------------------------------------------

    /// `claim_domain_validate_counter_claim_rejects_self_counter_and_missing_reason`
    /// — the layer-1 unit covering BOTH rejection arms the step names.
    #[test]
    fn claim_domain_validate_counter_claim_rejects_self_counter_and_missing_reason() {
        let target = target_cid();

        // Arm A — missing reason (target is a stranger so only the reason
        // rule can fire).
        let stranger_lookup =
            FakeLookup::with(&target, signed_of(unsigned_by(&someone_else())));
        let no_reason = counter_claim(&target, None);
        assert!(
            matches!(
                validate_counter_claim(&no_reason, &stranger_lookup, &me()),
                Err(ClaimError::CounterReasonMissing)
            ),
            "missing-reason counter must be rejected"
        );

        // Arm B — self-counter (good reason, but target is MY claim).
        let self_lookup = FakeLookup::with(&target, signed_of(unsigned_by(&me())));
        let self_counter = counter_claim(&target, Some("good reason"));
        assert!(
            matches!(
                validate_counter_claim(&self_counter, &self_lookup, &me()),
                Err(ClaimError::SelfCounter)
            ),
            "self-counter must be rejected"
        );
    }

    // Silence unused-import warnings for the VerifyingKey alias when the
    // crate's `verify` path is not exercised here.
    #[allow(dead_code)]
    fn _assert_verifying_key_imported(_: VerifyingKey) {}
}
