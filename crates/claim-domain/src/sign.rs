//! Ed25519 signing primitive over an unsigned-CID (ADR-006 step 6).
//!
//! Pure function. NO I/O. NO async. NO key loading from disk — the
//! adapter (crates/adapter-atproto-did) reads keys from the keychain
//! and passes them in as `SigningKey` bytes.
//!
//! ## Determinism
//!
//! Ed25519 signatures are deterministic per RFC 8032 §5.1.6: the nonce
//! `r` is derived from `SHA-512(secret_key_prefix || message)`, not
//! from a runtime RNG. The same `(unsigned_cid, key)` pair therefore
//! produces byte-identical signature output across runs / platforms /
//! language implementations. This is the load-bearing invariant for
//! federation: a peer recomputing the CID and re-signing with the
//! same key produces the same bytes — but more importantly, a peer
//! VERIFYING with the public key gets the same `Ok(())` regardless of
//! which language signed first.
//!
//! ## Wire shape (ADR-006)
//!
//! `SignatureBlock` carries:
//!   - `signed_cid`: the CID over which we signed (the unsigned claim's
//!     canonical-CBOR CID — see `crate::compute_cid`)
//!   - `signature_bytes`: the raw 64-byte Ed25519 signature. This crate
//!     holds the raw bytes; the JSON wire layer in `crates/lexicon`
//!     base64url-no-pad-encodes them on serialise.
//!   - `verification_method`: the DID fragment (`did:plc:…#org.openlore.application`)
//!     pointing at the public key in the author's DID document.
//!
//! NB: `signature_bytes` is `Vec<u8>` in domain because the pure core
//! is byte-agnostic — encoding belongs at the serialization boundary
//! (`crates/lexicon`). Step 03-01 keeps the boundary clean by NOT
//! base64-encoding inside `claim-domain::sign`.

use ed25519_dalek::{Signer, SigningKey as DalekSigningKey, SECRET_KEY_LENGTH};

use crate::{Cid, ClaimError, SignatureBlock, SigningKey};

/// Sign the canonical bytes of an unsigned claim's CID with an Ed25519
/// secret key. Returns a `SignatureBlock` ready to attach to the
/// unsigned claim to form a `SignedClaim`.
///
/// ## Inputs
/// - `unsigned_cid`: the CID computed by `crate::compute_cid` over
///   the canonical CBOR of the unsigned claim. Signed AS-IS (the
///   string form is the message).
/// - `key`: 32-byte Ed25519 secret key, wrapped in our domain
///   `SigningKey` newtype. The adapter loads this from the keychain.
///
/// ## Output
/// - `Ok(SignatureBlock { signed_cid, signature_bytes, verification_method })`
///   on success. `verification_method` is left empty — the adapter
///   fills it in with the DID fragment from the author's DID doc.
///   The pure core has no knowledge of DIDs at the verification-method
///   level (only of the author's `Did` on the claim itself).
/// - `Err(ClaimError::SignatureFailed { message })` if the key bytes
///   are not 32 bytes long (the only failure mode for Ed25519 signing
///   given the input is bytes — the signing operation itself cannot
///   fail in `ed25519-dalek` once the key is well-formed).
///
/// ## Why this returns `Result` despite Ed25519 signing being infallible
/// The `SigningKey(Vec<u8>)` newtype carries arbitrary bytes so the
/// pure core stays key-format-agnostic. Length validation is the only
/// thing that can go wrong, and we surface it as an error rather than
/// a panic to keep the pure pipeline railway-compatible (§8 of
/// `nw-fp-domain-modeling`).
pub fn sign(unsigned_cid: &Cid, key: &SigningKey) -> Result<SignatureBlock, ClaimError> {
    // 1. Validate key length. Ed25519 secret keys are 32 bytes; longer
    //    or shorter inputs are caller error.
    let key_bytes: [u8; SECRET_KEY_LENGTH] =
        key.0
            .as_slice()
            .try_into()
            .map_err(|_| ClaimError::SignatureFailed {
                message: format!(
                    "Ed25519 secret key must be exactly {} bytes, got {}",
                    SECRET_KEY_LENGTH,
                    key.0.len()
                ),
            })?;

    // 2. Build the dalek signing key from the validated bytes. This is
    //    infallible once the length is correct.
    let signing_key = DalekSigningKey::from_bytes(&key_bytes);

    // 3. Sign the CID's UTF-8 bytes (the wire form: `bafy…`). This is
    //    what ADR-006 step 6 specifies: sign over the CID, not over
    //    the canonical CBOR directly. Verifiers re-derive the same
    //    CID from the unsigned portion and verify against this sig.
    let signature = signing_key.sign(unsigned_cid.0.as_bytes());

    Ok(SignatureBlock {
        signed_cid: unsigned_cid.clone(),
        signature_bytes: signature.to_bytes().to_vec(),
        // `verification_method` is filled by the DID adapter at the
        // composition root — pure-core has no DID-doc knowledge.
        verification_method: String::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Deterministic test key. NOT a real key; safe to embed in source
    /// because it's published publicly here and used only for tests.
    /// 32 bytes of distinct values so any byte-truncation bug would
    /// flip a digit.
    fn test_signing_key() -> SigningKey {
        SigningKey(vec![
            0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e,
            0x0f, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1a, 0x1b, 0x1c,
            0x1d, 0x1e, 0x1f, 0x20,
        ])
    }

    fn sample_cid() -> Cid {
        Cid("bafyreigexampleexampleexampleexampleexampleexampleexample".into())
    }

    /// Determinism: same input → same signature, twice in a row.
    /// Load-bearing invariant for ADR-006 (CID stability + sig stability
    /// together let any peer reproduce the signed bytes from the source).
    #[test]
    fn sign_is_deterministic_for_equal_inputs() {
        let cid = sample_cid();
        let key = test_signing_key();
        let first = sign(&cid, &key).expect("first sign succeeds");
        let second = sign(&cid, &key).expect("second sign succeeds");
        assert_eq!(
            first.signature_bytes, second.signature_bytes,
            "Ed25519 sign must be deterministic per RFC 8032"
        );
        assert_eq!(
            first.signed_cid, second.signed_cid,
            "signed_cid must echo the input CID verbatim"
        );
    }

    /// Sanity: signature bytes are 64 (Ed25519 fixed-size). Catches a
    /// hypothetical bug where we accidentally truncated or padded.
    #[test]
    fn sign_produces_ed25519_sized_signature() {
        let block = sign(&sample_cid(), &test_signing_key()).expect("sign succeeds");
        assert_eq!(
            block.signature_bytes.len(),
            64,
            "Ed25519 signatures are exactly 64 bytes"
        );
    }

    /// Negative path: malformed key length surfaces as a domain error,
    /// NOT a panic. Railway-oriented (`nw-fp-domain-modeling` §8).
    #[test]
    fn sign_rejects_wrong_length_key() {
        let too_short = SigningKey(vec![0x01, 0x02, 0x03]);
        let result = sign(&sample_cid(), &too_short);
        assert!(
            matches!(result, Err(ClaimError::SignatureFailed { .. })),
            "wrong-length key must yield SignatureFailed, got {:?}",
            result
        );
    }
}
