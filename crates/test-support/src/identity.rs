//! `FakeIdentity` — deterministic `IdentityPort` test double.
//!
//! Step 04-03 (DD-6): the canonical identity test double used across
//! slice-01 acceptance tests and the WS-6 / FR-2 signing scenarios.
//! Real OS-keychain integration lands in step 04-04
//! (`adapter-atproto-did`); this fake is the test seam that lets
//! pure-core + cli composition be exercised without touching a real
//! DID resolver or keystore.
//!
//! Functional-paradigm note (ADR-007): one value per identity, no
//! shared mutable state. Each `jeff()` / `maria()` call returns a
//! fresh, immutable `FakeIdentity`. The deterministic Ed25519 keypair
//! is derived from a fixed 32-byte seed so signatures are byte-stable
//! across runs / platforms — load-bearing for FR-2 federation roundtrip
//! tests that pin the published signature against a known public key.
//!
//! ## Layout (per nw-fp-domain-modeling §2)
//!
//! - `did` is the bare author DID (no `#fragment`). The `IdentityPort`
//!   contract — `author_did() -> &Did` — exposes this directly.
//! - `signing_key` / `verifying_key` are claim-domain `SigningKey` /
//!   `VerifyingKey` newtypes; signing delegates to `claim_domain::sign`
//!   and verification to `claim_domain::verify`. Pure delegation keeps
//!   the test double honest: anything the production adapter could
//!   verify, the fake also verifies (and vice versa), because both
//!   share the same pure primitive.

use claim_domain::{Cid, Did, SignedClaim, SigningKey, VerifyingKey};
use ed25519_dalek::SigningKey as DalekSigningKey;
use ports::{IdentityError, IdentityPort, PeerInfo, ProbeOutcome};

/// Deterministic `IdentityPort` test double.
///
/// Holds a fixed Ed25519 keypair derived from a known seed. Constructed
/// via [`FakeIdentity::jeff`] or [`FakeIdentity::maria`] — these are the
/// canonical identities slice-01 tests refer to.
pub struct FakeIdentity {
    did: Did,
    signing_key: SigningKey,
    verifying_key: VerifyingKey,
}

impl FakeIdentity {
    /// Build a `FakeIdentity` from a bare DID string (no fragment) and a
    /// 32-byte Ed25519 seed. Internal helper — call sites use
    /// [`jeff`](Self::jeff) / [`maria`](Self::maria) for known fixtures.
    fn from_seed(did: &str, seed: [u8; 32]) -> Self {
        let dalek_sk = DalekSigningKey::from_bytes(&seed);
        let dalek_vk = dalek_sk.verifying_key();
        Self {
            did: Did(did.to_string()),
            signing_key: SigningKey(dalek_sk.to_bytes().to_vec()),
            verifying_key: VerifyingKey(dalek_vk.to_bytes().to_vec()),
        }
    }

    /// Canonical `did:plc:test-jeff` identity used across slice-01 tests.
    ///
    /// Seed: 32 zero bytes. Yields a stable, well-known public key whose
    /// bytes can be pinned in golden fixtures if a future test needs to
    /// verify the published record against a constant.
    pub fn jeff() -> Self {
        Self::from_seed("did:plc:test-jeff", [0u8; 32])
    }

    /// Secondary `did:plc:test-maria` identity used by multi-author
    /// scenarios (US-002 Example 3, US-003 Example 2, WS-10).
    ///
    /// Seed: 32 bytes of `0x01`. Distinct from `jeff` so the two
    /// identities produce different public keys, which acceptance tests
    /// rely on for "this claim was signed by Maria, not Jeff" checks.
    pub fn maria() -> Self {
        Self::from_seed("did:plc:test-maria", [1u8; 32])
    }

    /// Read access to the verifying (public) key. Acceptance tests pass
    /// this into `claim_domain::verify` directly when they want to assert
    /// a published signature verifies against this identity's public key.
    pub fn verifying_key(&self) -> &VerifyingKey {
        &self.verifying_key
    }
}

impl IdentityPort for FakeIdentity {
    /// Test doubles always probe `Ok`. The real `adapter-atproto-did`
    /// probes by reading a token from the OS keychain; there is no
    /// keychain in tests, so reporting `Ok` unconditionally keeps
    /// downstream wiring tests (WS-6, FR-2) unblocked. Refusal paths
    /// live in the real adapter's integration suite.
    fn probe(&self) -> ProbeOutcome {
        ProbeOutcome::Ok
    }

    fn author_did(&self) -> &Did {
        &self.did
    }

    /// Sign by delegating to the pure `claim_domain::sign` primitive.
    /// `IdentityError::SignatureFailed` wraps any error from the pure
    /// core so the port contract stays clean.
    fn sign(
        &self,
        unsigned_cid: &Cid,
    ) -> Result<claim_domain::SignatureBlock, IdentityError> {
        claim_domain::sign(unsigned_cid, &self.signing_key).map_err(|e| {
            IdentityError::SignatureFailed {
                message: format!("{e}"),
            }
        })
    }

    /// Verify by delegating to `claim_domain::verify` with this
    /// identity's verifying key. Failure surfaces as
    /// `IdentityError::VerificationFailed` regardless of the underlying
    /// pure-core error class — verify is a yes/no contract at the port.
    fn verify(&self, signed: &SignedClaim) -> Result<(), IdentityError> {
        claim_domain::verify(signed, &self.verifying_key)
            .map_err(|_| IdentityError::VerificationFailed)
    }

    /// Resolve a peer DID document (slice-03 extension).
    ///
    /// SCAFFOLD: true (slice-03) — `todo!()` stub. The PS-* / PP-*
    /// acceptance scenarios drive a real fixture-backed `resolve_peer`
    /// here in a later slice-03 phase (returning a deterministic
    /// `PeerInfo` for known fixture peers like `did:plc:rachel-test`).
    fn resolve_peer(&self, _peer_did: &Did) -> Result<PeerInfo, IdentityError> {
        // SCAFFOLD: true (slice-03)
        todo!("FakeIdentity::resolve_peer — fixture peer resolution lands with PS-* / PP-* scenarios")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use claim_domain::{
        canonicalize, compute_cid, ClaimReference, Confidence, SignatureBlock, UnsignedClaim,
    };

    /// Hand-built unsigned claim — the `Confidence` wrapper's smart
    /// constructor is RED-scaffolded, so we deserialize a JSON number
    /// directly (same trick `fixtures.rs` uses).
    fn sample_unsigned(author: &Did) -> UnsignedClaim {
        let confidence: Confidence = serde_json::from_value(serde_json::json!(0.8))
            .expect("test confidence value is well-formed");
        UnsignedClaim {
            subject: "github:rust-lang/rust".into(),
            predicate: "embodiesPhilosophy".into(),
            object: "org.openlore.philosophy.memory-safety".into(),
            evidence: vec!["https://www.rust-lang.org/".into()],
            confidence,
            author_did: author.clone(),
            composed_at: "2026-05-26T12:00:00Z".into(),
            references: Vec::<ClaimReference>::new(),
            reason: None,
        }
    }

    fn assemble_signed(unsigned: UnsignedClaim, sig: SignatureBlock) -> SignedClaim {
        SignedClaim {
            unsigned,
            signature: sig,
        }
    }

    /// Sign-then-verify roundtrip through the port methods. Asserts the
    /// load-bearing FakeIdentity contract: anything `jeff` signs,
    /// `jeff` also verifies. Without this, WS-6 / FR-2 signing
    /// scenarios cannot be wired against the fake at all.
    #[test]
    fn jeff_signs_and_verifies_with_own_pubkey() {
        let jeff = FakeIdentity::jeff();
        let unsigned = sample_unsigned(jeff.author_did());
        let canonical = canonicalize(&unsigned).expect("canonicalize succeeds");
        let cid = compute_cid(&canonical);

        let signature = jeff.sign(&cid).expect("jeff.sign succeeds");
        let signed = assemble_signed(unsigned, signature);

        let result = jeff.verify(&signed);
        assert!(
            result.is_ok(),
            "jeff must verify a signature it just produced, got {:?}",
            result
        );
    }

    /// The DID stored in `FakeIdentity::jeff()` is the bare DID — no
    /// `#fragment` suffix. `author_did()` returns it verbatim so the
    /// "author DID + key fragment" convention in the design is honored:
    /// the FAKE owns only the DID; the key fragment lives at the
    /// verification-method layer (filled by the adapter, not the
    /// identity port).
    #[test]
    fn jeff_did_strips_fragment() {
        let jeff = FakeIdentity::jeff();
        let did_str = &jeff.author_did().0;
        assert_eq!(did_str, "did:plc:test-jeff", "jeff DID must be bare");
        assert!(
            !did_str.contains('#'),
            "jeff DID must not carry a fragment, got {did_str:?}"
        );
    }

    /// `jeff` and `maria` must be distinct: different DIDs AND different
    /// public keys. Multi-author acceptance scenarios pin on this — a
    /// claim signed by Maria must not verify under Jeff's key.
    #[test]
    fn jeff_and_maria_are_distinct() {
        let jeff = FakeIdentity::jeff();
        let maria = FakeIdentity::maria();

        assert_ne!(
            jeff.author_did(),
            maria.author_did(),
            "jeff and maria must have distinct DIDs"
        );
        assert_ne!(
            jeff.verifying_key().0,
            maria.verifying_key().0,
            "jeff and maria must have distinct public keys"
        );

        // Cross-verification rejection: a claim Maria signed must not
        // verify under Jeff's key. Reinforces the "different keypair"
        // contract at the port boundary.
        let unsigned = sample_unsigned(maria.author_did());
        let canonical = canonicalize(&unsigned).expect("canonicalize succeeds");
        let cid = compute_cid(&canonical);
        let signature = maria.sign(&cid).expect("maria.sign succeeds");
        let signed = assemble_signed(unsigned, signature);

        let cross = jeff.verify(&signed);
        assert!(
            matches!(cross, Err(IdentityError::VerificationFailed)),
            "jeff must NOT verify a signature produced by maria, got {:?}",
            cross
        );
    }
}
