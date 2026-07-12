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
pub fn decode_ed25519_multibase(s: &str) -> Result<VerificationKey, DecodeError> {
    // 1. Strip the `z` multibase (base58btc) prefix.
    let base58_body = s.strip_prefix('z').ok_or(DecodeError::NotMultibase)?;

    // 2. base58btc-decode the remainder to raw multicodec-prefixed bytes.
    let bytes = base58btc_decode(base58_body).ok_or(DecodeError::BadBase58)?;

    // 3. The bytes MUST start with a multicodec prefix. The first byte
    //    discriminates the key type; only Ed25519 (`0xed`) is supported. A
    //    different (but well-formed varint) key-type prefix is an explicit
    //    UnsupportedKeyType — never a panic, never a mis-decode.
    let rest = match bytes.split_first() {
        Some((&ED25519_MULTICODEC_LOW, rest)) => rest,
        Some((&first, _)) if KNOWN_NON_ED25519_MULTICODEC_LOWS.contains(&first) => {
            return Err(DecodeError::UnsupportedKeyType)
        }
        // No prefix byte at all, or an unrecognised first byte: the multicodec
        // prefix is malformed.
        _ => return Err(DecodeError::BadMulticodecPrefix),
    };

    // 4. The Ed25519 multicodec is the two-byte varint `0xed 0x01`. The low
    //    byte matched above; the continuation byte MUST be `0x01`.
    let key_bytes = match rest.split_first() {
        Some((&ED25519_MULTICODEC_HIGH, key_bytes)) => key_bytes,
        _ => return Err(DecodeError::BadMulticodecPrefix),
    };

    // 5. The remaining bytes MUST be exactly 32 (an Ed25519 public key).
    if key_bytes.len() != ED25519_PUBLIC_KEY_LEN {
        return Err(DecodeError::WrongKeyLength);
    }

    Ok(VerificationKey(key_bytes.to_vec()))
}

/// The Ed25519 multicodec prefix is the unsigned-varint `0xed 0x01` (code `0xed`).
const ED25519_MULTICODEC_LOW: u8 = 0xed;
const ED25519_MULTICODEC_HIGH: u8 = 0x01;

/// An Ed25519 public key is exactly 32 bytes (RFC 8032).
const ED25519_PUBLIC_KEY_LEN: usize = 32;

/// A small set of well-known NON-Ed25519 multicodec low bytes we recognise
/// explicitly so we can surface [`DecodeError::UnsupportedKeyType`] (rather than
/// the generic [`DecodeError::BadMulticodecPrefix`]) when a caller hands us a
/// valid-but-unsupported key type. `0xe7` = secp256k1-pub, `0x80 0x24` = P-256-pub.
const KNOWN_NON_ED25519_MULTICODEC_LOWS: &[u8] = &[0xe7, 0x80, 0x12, 0x13];

/// PURE base58btc (Bitcoin alphabet) decoder — the inverse of the encoder in
/// `openlore_test_support::fixtures_ingest::base58btc_encode`. Returns `None` on
/// any character outside the alphabet. No external dependency (the algorithm is
/// short + well-known, and the pure core must stay dependency-light).
fn base58btc_decode(input: &str) -> Option<Vec<u8>> {
    const ALPHABET: &[u8; 58] = b"123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";

    // Leading '1' chars encode leading zero bytes.
    let leading_ones = input.bytes().take_while(|&b| b == b'1').count();

    // Convert each base58 digit into the running big-endian byte string via
    // repeated multiply-by-58 + add (the inverse of the encoder's divide loop).
    let mut bytes: Vec<u8> = Vec::new();
    for ch in input.bytes() {
        let value = ALPHABET.iter().position(|&a| a == ch)? as u32;
        let mut carry = value;
        for byte in bytes.iter_mut() {
            carry += (*byte as u32) * 58;
            *byte = (carry & 0xff) as u8;
            carry >>= 8;
        }
        while carry > 0 {
            bytes.push((carry & 0xff) as u8);
            carry >>= 8;
        }
    }

    // `bytes` is little-endian (least-significant first); reverse to big-endian
    // and prepend the leading zero bytes the '1' chars encoded.
    let mut out = vec![0u8; leading_ones];
    out.extend(bytes.into_iter().rev());
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Reference encoder (the inverse of [`decode_ed25519_multibase`]) used to
    /// build round-trip + boundary fixtures. Byte-for-byte the same layout as
    /// `openlore_test_support::fixtures_ingest::encode_ed25519_z6mk` — `z` ++
    /// base58btc(`0xed 0x01` ++ key) — so the decode is proven against the exact
    /// shape the production resolver receives from a PLC DID document.
    fn encode_ed25519_z6mk(pubkey_bytes: &[u8]) -> String {
        let mut payload = Vec::with_capacity(2 + pubkey_bytes.len());
        payload.push(ED25519_MULTICODEC_LOW);
        payload.push(ED25519_MULTICODEC_HIGH);
        payload.extend_from_slice(pubkey_bytes);
        format!("z{}", base58btc_encode(&payload))
    }

    /// Mirror encoder with an arbitrary multicodec prefix (for the
    /// unsupported-key-type + bad-prefix boundary fixtures).
    fn encode_with_prefix(prefix: &[u8], body: &[u8]) -> String {
        let mut payload = Vec::with_capacity(prefix.len() + body.len());
        payload.extend_from_slice(prefix);
        payload.extend_from_slice(body);
        format!("z{}", base58btc_encode(&payload))
    }

    /// PURE base58btc encoder (the encoder side of the round-trip; identical to
    /// the test-support fixture encoder).
    fn base58btc_encode(input: &[u8]) -> String {
        const ALPHABET: &[u8; 58] = b"123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";
        let leading_zeros = input.iter().take_while(|&&b| b == 0).count();
        let mut digits: Vec<u8> = Vec::new();
        for &byte in input {
            let mut carry = byte as u32;
            for digit in digits.iter_mut() {
                carry += (*digit as u32) << 8;
                *digit = (carry % 58) as u8;
                carry /= 58;
            }
            while carry > 0 {
                digits.push((carry % 58) as u8);
                carry /= 58;
            }
        }
        let mut out = String::with_capacity(leading_zeros + digits.len());
        for _ in 0..leading_zeros {
            out.push('1');
        }
        for &d in digits.iter().rev() {
            out.push(ALPHABET[d as usize] as char);
        }
        out
    }

    /// A deterministic 32-byte key fixture (distinct, non-trivial bytes so the
    /// round-trip exercises the full byte range, not an all-zero degenerate).
    fn sample_key() -> Vec<u8> {
        (0u8..32)
            .map(|i| i.wrapping_mul(7).wrapping_add(3))
            .collect()
    }

    /// decode ∘ encode == identity: a real `z6Mk...` value encoded from a known
    /// 32-byte key decodes byte-for-byte back to that key (the Earned-Trust
    /// round-trip property for valid keys, ADR-026 §Earned Trust).
    #[test]
    fn decode_encode_identity_round_trips_a_known_key() {
        let key = sample_key();
        let encoded = encode_ed25519_z6mk(&key);
        assert!(
            encoded.starts_with("z6Mk"),
            "the encoder must produce a z6Mk value; got {encoded}"
        );
        let decoded = decode_ed25519_multibase(&encoded).expect("valid z6Mk decodes");
        assert_eq!(
            decoded,
            VerificationKey(key),
            "decode∘encode must be the identity for a valid Ed25519 key"
        );
    }

    /// All-zero key edge: the encoder emits leading '1' chars for the key's zero
    /// bytes; the decoder must restore them (the leading-zero base58 boundary).
    #[test]
    fn decode_round_trips_all_zero_key() {
        let key = vec![0u8; 32];
        let encoded = encode_ed25519_z6mk(&key);
        let decoded = decode_ed25519_multibase(&encoded).expect("all-zero key decodes");
        assert_eq!(decoded, VerificationKey(key));
    }

    /// Boundary — NotMultibase: a string lacking the `z` base58btc prefix.
    #[test]
    fn rejects_missing_multibase_prefix() {
        // A valid body, but without the leading `z`.
        let body = {
            let encoded = encode_ed25519_z6mk(&sample_key());
            encoded.trim_start_matches('z').to_string()
        };
        assert_eq!(
            decode_ed25519_multibase(&body),
            Err(DecodeError::NotMultibase)
        );
        // The empty string also lacks the prefix.
        assert_eq!(decode_ed25519_multibase(""), Err(DecodeError::NotMultibase));
    }

    /// Boundary — BadBase58: a `z`-prefixed body carrying a non-alphabet char
    /// (`0`, `O`, `I`, `l` are excluded from the Bitcoin base58 alphabet).
    #[test]
    fn rejects_bad_base58_body() {
        assert_eq!(
            decode_ed25519_multibase("z6Mk0OIl"),
            Err(DecodeError::BadBase58)
        );
    }

    /// Boundary — BadMulticodecPrefix: well-formed base58 whose decoded bytes do
    /// NOT carry a recognised multicodec prefix (here the Ed25519 low byte but a
    /// WRONG continuation byte).
    #[test]
    fn rejects_bad_multicodec_continuation_byte() {
        // 0xed (Ed25519 low) followed by 0x02 (not the 0x01 varint continuation).
        let encoded = encode_with_prefix(&[ED25519_MULTICODEC_LOW, 0x02], &sample_key());
        assert_eq!(
            decode_ed25519_multibase(&encoded),
            Err(DecodeError::BadMulticodecPrefix)
        );
    }

    /// Boundary — BadMulticodecPrefix: an entirely unrecognised first byte (not
    /// Ed25519, not one of the known non-Ed25519 codecs) is a malformed prefix.
    #[test]
    fn rejects_unrecognised_first_prefix_byte() {
        let encoded = encode_with_prefix(&[0x99, 0x01], &sample_key());
        assert_eq!(
            decode_ed25519_multibase(&encoded),
            Err(DecodeError::BadMulticodecPrefix)
        );
    }

    /// Boundary — WrongKeyLength: a valid Ed25519 prefix but a key body that is
    /// not 32 bytes (here 31). Must NEVER silently accept a short/long key.
    #[test]
    fn rejects_wrong_key_length() {
        let short = vec![7u8; 31];
        let encoded = encode_ed25519_z6mk(&short);
        assert_eq!(
            decode_ed25519_multibase(&encoded),
            Err(DecodeError::WrongKeyLength)
        );
        let long = vec![7u8; 33];
        let encoded_long = encode_ed25519_z6mk(&long);
        assert_eq!(
            decode_ed25519_multibase(&encoded_long),
            Err(DecodeError::WrongKeyLength)
        );
    }

    /// Boundary — UnsupportedKeyType: a well-formed multibase carrying a KNOWN
    /// non-Ed25519 multicodec (secp256k1-pub `0xe7`) is an explicit, documented
    /// rejection — NEVER a panic, NEVER a mis-decode (ADR-026 §Negative).
    #[test]
    fn rejects_unsupported_key_type() {
        // secp256k1-pub multicodec is `0xe7 0x01` + 33 compressed key bytes.
        let encoded = encode_with_prefix(&[0xe7, 0x01], &[5u8; 33]);
        assert_eq!(
            decode_ed25519_multibase(&encoded),
            Err(DecodeError::UnsupportedKeyType)
        );
    }
}
