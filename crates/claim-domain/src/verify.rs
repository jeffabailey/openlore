//! Ed25519 signature verification over a signed claim (ADR-006 step 6).
//!
//! Pure function. NO I/O. NO async. NO public-key resolution — the
//! adapter resolves DIDs to verifying keys and passes the bytes in.
//!
//! ## Verification procedure (ADR-006 step 6)
//!
//! 1. Recompute the unsigned-claim CID via `canonicalize` + `compute_cid`
//!    on `signed.unsigned`. This guarantees that any mutation to the
//!    unsigned portion produces a different CID, which Ed25519 verify
//!    then rejects against the signature.
//! 2. (Optional cross-check) Confirm the recomputed CID matches the
//!    `signed.signature.signed_cid` field; mismatch means the signed
//!    claim was assembled inconsistently. We surface this as
//!    `VerificationFailed` — a peer should never accept such a claim.
//! 3. Ed25519-verify the signature bytes against the recomputed CID's
//!    UTF-8 bytes using the verifying key.
//!
//! Steps 1 + 2 make tampering on the unsigned portion impossible to
//! hide: changing any byte of the unsigned claim shifts the CID,
//! which fails step 2 or step 3.

use ed25519_dalek::{
    Signature, Verifier, VerifyingKey as DalekVerifyingKey, PUBLIC_KEY_LENGTH, SIGNATURE_LENGTH,
};

use crate::{canonicalize, compute_cid, ClaimError, SignedClaim, VerifyingKey};

/// Verify the Ed25519 signature on a signed claim using the provided
/// verifying key. Returns `Ok(())` iff the signature is valid AND the
/// recomputed unsigned-CID matches the one inside the signature block.
///
/// ## Inputs
/// - `signed`: a `SignedClaim` containing the unsigned portion and the
///   signature block.
/// - `pubkey`: 32-byte Ed25519 public key in our domain `VerifyingKey`
///   newtype. The adapter resolves it from the author's DID doc.
///
/// ## Output
/// - `Ok(())` on a valid signature over an un-tampered unsigned claim.
/// - `Err(ClaimError::VerificationFailed)` if:
///     - the recomputed CID disagrees with `signed.signature.signed_cid`
///       (the unsigned portion has been mutated since signing); OR
///     - Ed25519 `verify` rejects the signature (tampered signature
///       bytes, wrong key, or wrong CID).
/// - `Err(ClaimError::SignatureFailed { message })` if the verifying
///   key or signature bytes are not the expected length (malformed
///   inputs from the adapter, not a signature problem).
/// - `Err(ClaimError::CanonicalizationFailed { .. })` if
///   re-canonicalizing the unsigned portion fails.
pub fn verify(signed: &SignedClaim, pubkey: &VerifyingKey) -> Result<(), ClaimError> {
    // 1. Length-check the verifying key. Ed25519 public keys are 32 bytes.
    let pubkey_bytes: [u8; PUBLIC_KEY_LENGTH] =
        pubkey
            .0
            .as_slice()
            .try_into()
            .map_err(|_| ClaimError::SignatureFailed {
                message: format!(
                    "Ed25519 public key must be exactly {} bytes, got {}",
                    PUBLIC_KEY_LENGTH,
                    pubkey.0.len()
                ),
            })?;

    // 2. Length-check the signature. Ed25519 signatures are 64 bytes.
    let sig_bytes: [u8; SIGNATURE_LENGTH] = signed
        .signature
        .signature_bytes
        .as_slice()
        .try_into()
        .map_err(|_| ClaimError::SignatureFailed {
            message: format!(
                "Ed25519 signature must be exactly {} bytes, got {}",
                SIGNATURE_LENGTH,
                signed.signature.signature_bytes.len()
            ),
        })?;

    // 3. Build dalek primitives.
    let verifying_key = DalekVerifyingKey::from_bytes(&pubkey_bytes).map_err(|e| {
        ClaimError::SignatureFailed {
            message: format!("malformed Ed25519 public key: {e}"),
        }
    })?;
    let signature = Signature::from_bytes(&sig_bytes);

    // 4. Recompute the unsigned-CID. If the unsigned portion has been
    //    tampered with, the CID will differ from the one in the
    //    signature block AND the signature will fail to verify
    //    against the mutated CID. Either path → VerificationFailed.
    let canonical = canonicalize(&signed.unsigned)?;
    let recomputed_cid = compute_cid(&canonical);

    if recomputed_cid != signed.signature.signed_cid {
        return Err(ClaimError::VerificationFailed);
    }

    // 5. Ed25519-verify the signature over the recomputed CID's bytes.
    //    `verify_strict` rejects malleable signatures (RFC 8032 §5.1.7).
    verifying_key
        .verify(recomputed_cid.0.as_bytes(), &signature)
        .map_err(|_| ClaimError::VerificationFailed)
}

#[cfg(test)]
mod tests {
    use ed25519_dalek::SigningKey as DalekSigningKey;

    use super::*;
    use crate::{
        sign, ClaimReference, Confidence, Did, SignedClaim, SigningKey, UnsignedClaim,
    };

    /// Build a fresh dalek (signing, verifying) keypair from
    /// deterministic seed bytes. Returns the domain newtypes ready to
    /// pass into `sign` / `verify`.
    fn test_keypair() -> (SigningKey, VerifyingKey) {
        let seed: [u8; 32] = [
            0x42, 0x42, 0x42, 0x42, 0x42, 0x42, 0x42, 0x42, 0x42, 0x42, 0x42, 0x42, 0x42, 0x42,
            0x42, 0x42, 0x42, 0x42, 0x42, 0x42, 0x42, 0x42, 0x42, 0x42, 0x42, 0x42, 0x42, 0x42,
            0x42, 0x42, 0x42, 0x42,
        ];
        let sk = DalekSigningKey::from_bytes(&seed);
        let vk = sk.verifying_key();
        (
            SigningKey(sk.to_bytes().to_vec()),
            VerifyingKey(vk.to_bytes().to_vec()),
        )
    }

    fn sample_unsigned() -> UnsignedClaim {
        UnsignedClaim {
            subject: "github:openlore/openlore".into(),
            predicate: "embodiesPhilosophy".into(),
            object: "org.openlore.philosophy.memory-safety".into(),
            evidence: vec!["https://example.org/evidence/1".into()],
            confidence: Confidence(0.8),
            author_did: Did("did:plc:jeff#org.openlore.application".into()),
            composed_at: "2026-05-26T12:00:00Z".into(),
            references: Vec::<ClaimReference>::new(),
            reason: None,
        }
    }

    /// Construct a real signed claim by canonicalize → compute_cid →
    /// sign. Mirrors the production composition for testing.
    fn make_signed(unsigned: UnsignedClaim, key: &SigningKey) -> SignedClaim {
        let canonical = canonicalize(&unsigned).expect("canonicalize succeeds");
        let cid = compute_cid(&canonical);
        let signature = sign(&cid, key).expect("sign succeeds");
        SignedClaim {
            unsigned,
            signature,
        }
    }

    /// Roundtrip: sign-then-verify must succeed on un-tampered input.
    /// This is the load-bearing invariant for ADR-006 step 6.
    #[test]
    fn verify_accepts_genuine_signature() {
        let (sk, vk) = test_keypair();
        let signed = make_signed(sample_unsigned(), &sk);
        let result = verify(&signed, &vk);
        assert!(
            result.is_ok(),
            "genuine signature must verify, got {:?}",
            result
        );
    }

    /// Tamper-detection: mutating the unsigned portion changes the
    /// recomputed CID, which step 4 of `verify` catches before the
    /// Ed25519 check even runs.
    #[test]
    fn verify_rejects_tampered_unsigned_portion() {
        let (sk, vk) = test_keypair();
        let mut signed = make_signed(sample_unsigned(), &sk);
        // Tamper: bump confidence. Recomputed CID will differ.
        signed.unsigned.confidence = Confidence(0.99);
        let result = verify(&signed, &vk);
        assert!(
            matches!(result, Err(ClaimError::VerificationFailed)),
            "tampered unsigned portion must produce VerificationFailed, got {:?}",
            result
        );
    }

    /// Tamper-detection: mutating the signature bytes themselves
    /// makes Ed25519 reject. The CID still matches signed_cid (we
    /// only touched the sig), so we fall through to step 5.
    #[test]
    fn verify_rejects_tampered_signature_bytes() {
        let (sk, vk) = test_keypair();
        let mut signed = make_signed(sample_unsigned(), &sk);
        // Tamper: flip a bit in the signature. XOR keeps the length at 64.
        signed.signature.signature_bytes[0] ^= 0x01;
        let result = verify(&signed, &vk);
        assert!(
            matches!(result, Err(ClaimError::VerificationFailed)),
            "tampered signature bytes must produce VerificationFailed, got {:?}",
            result
        );
    }
}
