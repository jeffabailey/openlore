//! The one place an OpenLore claim is signed (ADR-086 §4, OOS-D8/D15).
//!
//! `sign_permitted` signs a draft only under a [`SigningPermit`] minted in
//! `claim_domain::authority` for that draft's subject and signer, so no verb
//! can sign a claim its authority gate did not pass. `xtask check-arch`
//! (`sign_only_with_permit`) holds every `IdentityPort::sign` call to here;
//! philosophy records, which are not claims, are the one allowlisted site.

use anyhow::{anyhow, bail, Result};
use claim_domain::authority::{ClaimShape, SigningPermit};
use claim_domain::{canonicalize, compute_cid, ReferenceType, SignedClaim, UnsignedClaim};
use ports::IdentityPort;

/// Sign `unsigned` under `permit`: canonicalize, compute its CID and sign it
/// with `identity`. Refuses, before anything is signed, a permit for another
/// subject or another signer than the draft's.
pub(crate) fn sign_permitted(
    identity: &dyn IdentityPort,
    permit: &SigningPermit,
    unsigned: UnsignedClaim,
) -> Result<SignedClaim> {
    if permit.subject() != unsigned.subject {
        bail!(
            "the signing permit is for {}, not for {}",
            permit.subject(),
            unsigned.subject
        );
    }
    let signer = &identity.author_did().0;
    if permit.signer() != signer || unsigned.author_did.0 != *signer {
        bail!(
            "the signing permit is for {}, the draft is by {} and the key is {}",
            permit.signer(),
            unsigned.author_did.0,
            signer
        );
    }
    let canonical = canonicalize(&unsigned).map_err(|e| anyhow!("canonicalizing: {e}"))?;
    let signature = identity.sign(&compute_cid(&canonical))?;
    Ok(SignedClaim {
        unsigned,
        signature,
    })
}

/// The reference types a draft carries: what its kind is judged from.
pub(crate) fn reference_types(unsigned: &UnsignedClaim) -> Vec<ReferenceType> {
    unsigned.references.iter().map(|r| r.ref_type).collect()
}

/// The shape the authority gate judges `unsigned` by.
pub(crate) fn shape_of<'a>(
    unsigned: &'a UnsignedClaim,
    reference_types: &'a [ReferenceType],
) -> ClaimShape<'a> {
    ClaimShape {
        subject: &unsigned.subject,
        predicate: &unsigned.predicate,
        reference_types,
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use claim_domain::authority::exempt_permit;
    use claim_domain::{Cid, Confidence, Did, SignatureBlock};
    use openlore_test_support::identity::FakeIdentity;
    use ports::{IdentityError, PeerInfo, ProbeOutcome};
    use proptest::prelude::*;

    use super::*;

    /// The test identity, counting the signatures it makes.
    struct CountingIdentity {
        inner: FakeIdentity,
        signed: Cell<usize>,
    }

    impl IdentityPort for CountingIdentity {
        fn probe(&self) -> ProbeOutcome {
            self.inner.probe()
        }
        fn author_did(&self) -> &Did {
            self.inner.author_did()
        }
        fn sign(&self, unsigned_cid: &Cid) -> Result<SignatureBlock, IdentityError> {
            self.signed.set(self.signed.get() + 1);
            self.inner.sign(unsigned_cid)
        }
        fn verify(&self, signed: &SignedClaim) -> Result<(), IdentityError> {
            self.inner.verify(signed)
        }
        fn resolve_peer(&self, peer_did: &Did) -> Result<PeerInfo, IdentityError> {
            self.inner.resolve_peer(peer_did)
        }
    }

    fn draft(subject: &str, author: &str) -> UnsignedClaim {
        UnsignedClaim {
            subject: subject.to_string(),
            predicate: "adheresToPhilosophy".to_string(),
            object: "unix".to_string(),
            evidence: Vec::new(),
            confidence: Confidence::from_basis_points(7000),
            author_did: Did(author.to_string()),
            composed_at: "2026-10-20T00:00:00Z".to_string(),
            references: Vec::new(),
            reason: None,
        }
    }

    fn exempt(signer: &str, subject: &str) -> SigningPermit {
        exempt_permit(signer, shape_of(&draft(subject, signer), &[])).expect("exempt subject")
    }

    proptest! {
        /// A permit signs only the draft it was minted for: another subject
        /// or another signer is refused and nothing is signed; its own draft
        /// is signed once, under the draft's CID.
        #[test]
        fn a_permit_signs_only_its_own_subject_and_signer(
            permit_subject in "ex:[a-z]{1,6}",
            draft_subject in "ex:[a-z]{1,6}",
            other_signer in any::<bool>(),
        ) {
            let identity = CountingIdentity { inner: FakeIdentity::jeff(), signed: Cell::new(0) };
            let jeff = identity.author_did().0.clone();
            let signer = if other_signer { "did:plc:test-maria".to_string() } else { jeff.clone() };
            let permit = exempt(&signer, &permit_subject);

            let signed = sign_permitted(&identity, &permit, draft(&draft_subject, &jeff));

            let own_draft = permit_subject == draft_subject && !other_signer;
            prop_assert_eq!(signed.is_ok(), own_draft);
            prop_assert_eq!(identity.signed.get(), usize::from(own_draft));
            if let Ok(signed) = signed {
                let cid = compute_cid(&canonicalize(&signed.unsigned).expect("canonical"));
                prop_assert_eq!(signed.signature.signed_cid, cid);
            }
        }
    }
}
