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
    let mut pairs: Vec<(&'static str, Value)> = vec![
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

    // 1b. OPTIONAL `reason` (ADR-015 / WD-34): present ONLY on
    //     counter-claims. When absent it contributes ZERO bytes — that is
    //     what preserves CID stability across the slice-01 → slice-03
    //     upgrade (I-FED-7 / LCC-2): a slice-01 reason=None claim
    //     canonicalizes to the SAME bytes a slice-01 binary produced.
    //     When present it is folded into the canonical bytes so the CID +
    //     signature cover it. Under the length-first lex sort below,
    //     "reason" (length 6) lands among the other length-6 keys,
    //     ordering as `author` < `object` < `reason`.
    if let Some(reason) = &claim.reason {
        pairs.push(("reason", Value::Text(reason.clone())));
    }

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
            reason: None,
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

    /// CID stability (LCC-2 / I-FED-7): a `reason: None` claim canonicalizes
    /// to EXACTLY the slice-01-era byte sequence — adding the optional
    /// `reason` field to the model contributes ZERO bytes when absent. This
    /// is the unit-level guard mirroring the LCC-2 acceptance gate: a
    /// regression that unconditionally emitted a `reason` key (even as CBOR
    /// null) would drift every previously-published author claim's CID.
    #[test]
    fn canonicalize_omits_reason_entirely_when_none() {
        // A counter-style claim (carries a Counters reference) but with NO
        // reason. The canonical bytes MUST NOT contain the UTF-8 key
        // "reason" anywhere.
        let mut claim = sample_claim();
        claim.references = vec![ClaimReference {
            ref_type: ReferenceType::Counters,
            cid: crate::Cid("bafytargetcidxxxxxxxxxxxxxxxxxxxx".into()),
        }];
        claim.reason = None;

        let bytes = canonicalize(&claim).expect("canonicalize reason=None claim");
        assert!(
            !contains_subslice(&bytes, b"reason"),
            "a reason=None claim must contribute ZERO bytes for the `reason` key \
             (CID stability across slice-01 → slice-03; I-FED-7)"
        );
    }

    /// The signed payload (and thus the CID) MUST cover the reason when
    /// present (ADR-006 lex order). Property: for any non-empty reason, the
    /// reason=Some canonical bytes (a) DIFFER from the reason=None bytes —
    /// proving the reason is included — and (b) contain the UTF-8 key
    /// "reason" placed in length-first lexicographic order among the other
    /// length-6 keys (`author`, `object`, `reason`), i.e. AFTER `object`.
    #[test]
    fn canonicalize_includes_reason_in_canonical_bytes_when_present() {
        use proptest::prelude::*;
        use proptest::test_runner::TestRunner;

        let base = {
            let mut c = sample_claim();
            c.references = vec![ClaimReference {
                ref_type: ReferenceType::Counters,
                cid: crate::Cid("bafytargetcidxxxxxxxxxxxxxxxxxxxx".into()),
            }];
            c.reason = None;
            c
        };
        let none_bytes = canonicalize(&base).expect("canonicalize reason=None");

        let mut runner = TestRunner::default();
        runner
            .run(&"[a-zA-Z0-9 .,!?]{1,80}", |reason_text| {
                let mut with_reason = base.clone();
                with_reason.reason = Some(reason_text.clone());
                let some_bytes = canonicalize(&with_reason).expect("canonicalize reason=Some");

                // (a) The reason is actually folded into the canonical bytes,
                //     so the CID + signature cover it.
                prop_assert_ne!(
                    &some_bytes,
                    &none_bytes,
                    "a present reason must change the canonical bytes (else the CID \
                     would not cover the reason — ADR-006)"
                );
                // (b) The "reason" key appears, AND it sorts AFTER "object"
                //     under the length-first lex rule (both are length 6;
                //     "object" < "reason" lexicographically).
                let reason_at = find_subslice(&some_bytes, b"reason")
                    .expect("canonical bytes must contain the `reason` key");
                let object_at = find_subslice(&some_bytes, b"object")
                    .expect("canonical bytes must contain the `object` key");
                prop_assert!(
                    object_at < reason_at,
                    "`reason` must sort after `object` (length-first lex; ADR-006)"
                );
                Ok(())
            })
            .expect("reason-inclusion property must hold for all generated reasons");
    }

    fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
        haystack
            .windows(needle.len())
            .position(|window| window == needle)
    }

    fn contains_subslice(haystack: &[u8], needle: &[u8]) -> bool {
        find_subslice(haystack, needle).is_some()
    }
}
