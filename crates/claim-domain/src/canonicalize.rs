//! Canonical CBOR encoding of an `UnsignedClaim` (ADR-006).
//!
//! Pure function. NO I/O. NO mutation of inputs.
//!
//! ## Wire shape
//!
//! The on-the-wire CBOR map uses the Lexicon key names from
//! `org.openlore.claim` (`subject`, `predicate`, `object`, `evidence`,
//! `confidence`, `author`, `composedAt`, `references`) — NOT the Rust
//! struct field names. This is what locks CID stability across language
//! implementations: a verifier in another language reading the same
//! Lexicon and the same canonical-CBOR rule produces the same bytes.
//!
//! ## Canonicalization rule (RFC 8949 §4.2.1, Core Deterministic Encoding)
//!
//! - Shortest-form integer encoding (`ciborium` does this by default).
//! - Length-first lexicographic key sorting for maps. For our short
//!   ASCII string keys this collapses to "shorter key first, then
//!   lexicographic byte order within the same length".
//! - No indefinite-length items (`ciborium`'s default).

use crate::{ClaimError, UnsignedClaim};

/// RFC 8949 canonical CBOR over an `UnsignedClaim`. Stable across runs
/// / platforms / language implementations sharing the same Lexicon.
///
/// Returns `Err(ClaimError::CanonicalizationFailed { .. })` only if
/// `ciborium` itself fails to serialise; the error path is preserved
/// for total-function discipline (in practice `Vec<u8>` writes cannot
/// fail).
pub fn canonicalize(claim: &UnsignedClaim) -> Result<Vec<u8>, ClaimError> {
    use ciborium::Value;

    // 1. Build (Lexicon-key, CBOR-value) pairs. Lexicon names — NOT
    //    Rust field names — are what other implementations will read.
    let pairs: Vec<(&'static str, Value)> = vec![
        ("subject", Value::Text(claim.subject.clone())),
        ("predicate", Value::Text(claim.predicate.clone())),
        ("object", Value::Text(claim.object.clone())),
        (
            "evidence",
            Value::Array(
                claim
                    .evidence
                    .iter()
                    .map(|e| Value::Text(e.clone()))
                    .collect(),
            ),
        ),
        // Read the inner `f64` directly (crate-private field). We do
        // NOT call `Confidence::value()` because that smart-accessor
        // is still a RED-scaffold panic at step 02-03 — canonicalize
        // is responsible for `f64 → CBOR float`, not for the wrapper's
        // ergonomics. When `value()` lands later this stays byte-stable.
        ("confidence", Value::Float(claim.confidence.0)),
        ("author", Value::Text(claim.author_did.0.clone())),
        ("composedAt", Value::Text(claim.composed_at.clone())),
        (
            "references",
            Value::Array(
                claim
                    .references
                    .iter()
                    .map(|r| {
                        let type_str = match r.ref_type {
                            crate::ReferenceType::Retracts => "retracts",
                            crate::ReferenceType::Corrects => "corrects",
                            crate::ReferenceType::Counters => "counters",
                            crate::ReferenceType::Supersedes => "supersedes",
                        };
                        // Inner reference map: keys "cid" + "type", same
                        // length-first lex rule. Both keys are length 3
                        // and 4; "cid" sorts before "type".
                        Value::Map(vec![
                            (Value::Text("cid".into()), Value::Text(r.cid.0.clone())),
                            (Value::Text("type".into()), Value::Text(type_str.into())),
                        ])
                    })
                    .collect(),
            ),
        ),
    ];

    // 2. RFC 8949 §4.2.1: length-first, then lexicographic on UTF-8
    //    bytes. Equivalent to lex on encoded CBOR key bytes for short
    //    ASCII keys, which all our keys are.
    let mut sorted = pairs;
    sorted.sort_by(|(a, _), (b, _)| {
        a.len()
            .cmp(&b.len())
            .then_with(|| a.as_bytes().cmp(b.as_bytes()))
    });

    let map_value = Value::Map(
        sorted
            .into_iter()
            .map(|(k, v)| (Value::Text(k.to_string()), v))
            .collect(),
    );

    // 3. Serialise via ciborium — shortest-form ints + no
    //    indefinite-length items are ciborium defaults.
    let mut buf = Vec::new();
    ciborium::ser::into_writer(&map_value, &mut buf).map_err(|e| {
        ClaimError::CanonicalizationFailed {
            message: e.to_string(),
        }
    })?;
    Ok(buf)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ClaimReference, Confidence, Did, ReferenceType};

    fn sample_claim() -> UnsignedClaim {
        // NOTE: `Confidence`'s inner `f64` is private to the crate; we
        // construct directly here rather than via `try_new` (still a
        // RED-scaffold panic at step 02-03). Tests inside the crate
        // legitimately bypass the smart constructor because they own
        // the invariant being constructed.
        UnsignedClaim {
            subject: "github:openlore/openlore".into(),
            predicate: "embodiesPhilosophy".into(),
            object: "org.openlore.philosophy.memory-safety".into(),
            evidence: vec![
                "https://example.org/evidence/1".into(),
                "https://example.org/evidence/2".into(),
            ],
            confidence: Confidence(0.75),
            author_did: Did("did:plc:jeff#org.openlore.application".into()),
            composed_at: "2026-05-25T12:00:00Z".into(),
            references: vec![ClaimReference {
                ref_type: ReferenceType::Supersedes,
                cid: crate::Cid("bafyreiexamplecidxxxxxxxxxxxxxxxxxxxx".into()),
            }],
        }
    }

    /// Determinism: same input → same canonical bytes, twice in a row.
    /// This is the load-bearing invariant for CID stability (ADR-006
    /// §"Earned Trust" point 1).
    #[test]
    fn canonicalize_is_deterministic_for_equal_inputs() {
        let claim = sample_claim();
        let first = canonicalize(&claim).expect("first canonicalize succeeds");
        let second = canonicalize(&claim).expect("second canonicalize succeeds");
        assert_eq!(
            first, second,
            "canonical CBOR must be byte-equal across runs"
        );
    }
}
