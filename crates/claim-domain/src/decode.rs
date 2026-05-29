//! `decode` — the PURE Ed25519 `publicKeyMultibase` decode helper (ADR-026).
//!
//! [`decode_ed25519_multibase`] decodes a `z6Mk...` base58btc multibase
//! `publicKeyMultibase` (the form the PLC DID document records, ADR-026) into the
//! [`VerificationKey`] the pure [`crate::verify`] consumes. NO I/O. NO async. The
//! adapter (`adapter-atproto-did`) resolves the DID document over the network and
//! passes the multibase string in; the byte-level decode happens HERE, in the
//! pure core, so the verify-before-index gate (WD-104) uses one verification path.
//!
//! ## The z6Mk decode procedure (ADR-026)
//!
//! 1. The string MUST start with the `z` multibase prefix (base58btc).
//! 2. base58btc-decode the remainder to raw bytes.
//! 3. The bytes MUST start with the Ed25519 multicodec prefix `0xed 0x01`.
//! 4. The remaining bytes MUST be exactly 32 (an Ed25519 public key).
//!
//! Each failure maps to a distinct [`DecodeError`] variant so the renderer and
//! the `identity.pubkey_decode_failed` telemetry can distinguish them. The
//! function NEVER panics and NEVER mis-decodes (Earned Trust:
//! `decode∘encode == identity` for valid keys; malformed input errors).
//!
//! Bootstrap (step 01-01): the value types ([`VerificationKey`], [`KeyId`]) +
//! the error type + the signature land here; the decode BODY is `todo!()`. The
//! real z6Mk path is driven by AV-4 in step 03-04 (a genuine TDD cycle on the
//! pure decode at that point). `verify`/`compute_cid` are UNCHANGED.
//
// SCAFFOLD: true

use serde::{Deserialize, Serialize};

/// The Ed25519 verification key decoded from a `publicKeyMultibase` value
/// (ADR-026). Wraps the 32 raw public-key bytes the pure `verify` consumes.
///
/// Distinct from the lower-level [`crate::VerifyingKey`] newtype (which `verify`
/// takes directly): `VerificationKey` is the DECODE output the resolver yields;
/// the bridge into `verify` is wired at the call site so `verify`'s signature
/// stays UNCHANGED.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerificationKey(pub Vec<u8>);

/// The DID-document verification-method id the signature verified against
/// (e.g. `did:plc:priya-test#org.openlore.application`), recorded in
/// `IndexedClaim::verified_against`. NEVER empty (WD-104).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct KeyId(pub String);

/// Why decoding a `publicKeyMultibase` value failed. Each variant is a distinct
/// step of the ADR-026 procedure; modeled as a choice type so callers and
/// telemetry distinguish them. The decode NEVER panics — every failure is one of
/// these values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DecodeError {
    /// The string does not start with the `z` (base58btc) multibase prefix.
    NotMultibase,
    /// The base58btc body failed to decode (invalid alphabet / checksum).
    BadBase58,
    /// The decoded bytes do not start with the Ed25519 multicodec prefix `0xed 0x01`.
    BadMulticodecPrefix,
    /// The key body is not exactly 32 bytes after stripping the prefix.
    WrongKeyLength,
    /// The multicodec prefix names a key type other than Ed25519.
    UnsupportedKeyType,
}

/// PURE: decode a `z6Mk...` base58btc multibase `publicKeyMultibase` into the
/// Ed25519 [`VerificationKey`] the pure `verify` consumes. NO I/O (ADR-026).
///
/// Returns `Err(DecodeError::…)` on any malformed input (never panics, never
/// mis-decodes). Bootstrap: `todo!()` — the real z6Mk path is driven by AV-4 in
/// step 03-04.
pub fn decode_ed25519_multibase(_s: &str) -> Result<VerificationKey, DecodeError> {
    // SCAFFOLD: true — real z6Mk decode driven by AV-4 (step 03-04).
    todo!("decode_ed25519_multibase — driven by the ADR-026 z6Mk decode scenario (AV-4)")
}
