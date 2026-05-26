//! CIDv1 dag-cbor sha2-256 base32-lower computation (ADR-006).
//!
//! Pure function. NO I/O.
//!
//! ## Wire shape
//!
//! Encoded form: `bafyrei…` — the `b` prefix is `multibase` base32-lower,
//! the rest decodes to: CIDv1 marker (`0x01`) + codec `dag-cbor` (`0x71`)
//! + multihash `{ code: sha2-256 (0x12), size: 32, digest: <32 bytes> }`.

use cid::Cid as IpldCid;
use multihash::Multihash;
use sha2::{Digest, Sha256};

use crate::Cid;

/// dag-cbor codec identifier per IPLD multicodec table.
const DAG_CBOR_CODEC: u64 = 0x71;
/// sha2-256 hash code per multihash code table.
const SHA2_256_CODE: u64 = 0x12;

/// Compute the CIDv1 dag-cbor sha2-256 base32-lower CID over canonical
/// CBOR bytes. Total — input is a byte slice, output is always a `Cid`.
///
/// Use ONLY with bytes produced by [`crate::canonicalize`]; any other
/// byte source yields a CID that no other implementation will reproduce.
pub fn compute_cid(canonical_bytes: &[u8]) -> Cid {
    // 1. sha2-256 digest of the canonical bytes.
    let mut hasher = Sha256::new();
    hasher.update(canonical_bytes);
    let digest = hasher.finalize(); // 32-byte GenericArray

    // 2. Wrap the 32-byte digest in a Multihash sized for the cid
    //    crate's default `Cid = CidGeneric<64>` (S=64 is the *buffer*
    //    size, not the digest size — the actual digest stays 32 bytes
    //    for sha2-256). Wrap cannot fail: 32 ≤ 64.
    let mh: Multihash<64> = Multihash::wrap(SHA2_256_CODE, digest.as_slice())
        .expect("sha2-256 digest is 32 bytes; fits in a 64-byte multihash buffer");

    // 3. Construct the CIDv1 with dag-cbor codec.
    let ipld_cid = IpldCid::new_v1(DAG_CBOR_CODEC, mh);

    // 4. `Display` for CIDv1 defaults to multibase base32-lower (the
    //    `b…` prefix). That is exactly the ADR-006 wire form.
    Cid(ipld_cid.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Determinism: same bytes → same CID, twice in a row.
    /// Companion to `canonicalize_is_deterministic_for_equal_inputs`
    /// — together they pin the full `claim → CID` invariant.
    #[test]
    fn compute_cid_is_deterministic_for_equal_bytes() {
        let bytes = b"hello openlore";
        let first = compute_cid(bytes);
        let second = compute_cid(bytes);
        assert_eq!(
            first, second,
            "compute_cid must be a pure function of its input bytes"
        );
        // Sanity: CIDv1 base32-lower is always `b…` per multibase.
        assert!(
            first.0.starts_with('b'),
            "CIDv1 base32-lower must begin with multibase prefix 'b', got {:?}",
            first.0
        );
    }

    /// Different bytes → different CIDs (catches a hypothetical bug
    /// where `compute_cid` returns a constant).
    #[test]
    fn compute_cid_distinguishes_distinct_inputs() {
        let a = compute_cid(b"input-a");
        let b = compute_cid(b"input-b");
        assert_ne!(a, b, "distinct inputs must yield distinct CIDs");
    }
}
