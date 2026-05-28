//! Explicit serde helpers + `Eq`-friendly confidence wrapper for the
//! `org.openlore.claim` Lexicon wire shape.
//!
//! Step 02-06 (LC-1 roundtrip) added this module to consolidate the
//! pieces of the federation-wire contract that the derive macros alone
//! cannot express:
//!
//!   * `ConfidenceField` — newtype around `f64` with a manual `Eq` impl
//!     based on bit-equality. The Lexicon constrains `confidence` to
//!     `[0.0, 1.0]` (no NaN), so bit-equality is total there. Useful for
//!     downstream types that want to derive `Eq` on a record containing
//!     a confidence number.
//!
//!   * `claim_to_canonical_json` / `claim_from_json` — explicit
//!     roundtrip helpers that go through `serde_json` while documenting
//!     the federation contract: the produced JSON object uses the
//!     Lexicon's camelCase field names (`composedAt`, `author`) verbatim.
//!     The derived serde impls on `Claim` already enforce this; these
//!     helpers exist so call sites read as `lexicon::claim_to_canonical_json(...)`
//!     instead of a bare `serde_json::to_value` that obscures the contract.
//!
//! No mutation; no I/O. Pure module.

use crate::claim::{Claim, LexiconError};
use serde::{Deserialize, Serialize};

// -----------------------------------------------------------------------------
// ConfidenceField — `f64` newtype with bit-equality `Eq`
// -----------------------------------------------------------------------------

/// `f64` wrapper that satisfies `Eq` via bit-equality.
///
/// Standard `f64` does NOT implement `Eq` because of NaN's reflexivity
/// hole. The Lexicon's confidence range is `[0.0, 1.0]` (per the JSON
/// schema's `minimum` / `maximum` keywords), so a validated
/// `ConfidenceField` value cannot be NaN — making bit-equality both
/// total and consistent with `PartialEq` over the legal domain.
///
/// `Hash` is provided via the same bit pattern so a `ConfidenceField`
/// can sit inside a `HashMap` key.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ConfidenceField(pub f64);

impl PartialEq for ConfidenceField {
    fn eq(&self, other: &Self) -> bool {
        self.0.to_bits() == other.0.to_bits()
    }
}

impl Eq for ConfidenceField {}

impl std::hash::Hash for ConfidenceField {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.to_bits().hash(state);
    }
}

impl From<f64> for ConfidenceField {
    fn from(value: f64) -> Self {
        ConfidenceField(value)
    }
}

impl From<ConfidenceField> for f64 {
    fn from(value: ConfidenceField) -> f64 {
        value.0
    }
}

// -----------------------------------------------------------------------------
// Canonical roundtrip helpers for `lexicon::Claim`
// -----------------------------------------------------------------------------

/// Serialize a `Claim` to its Lexicon-shaped JSON value.
///
/// The output is field-for-field equivalent to what a peer in slice-03
/// will see on the wire: camelCase keys (`composedAt`), `signature`
/// nested as `{kid, alg, sig}`, omitted-when-empty defaults preserved.
/// Federation drift in this shape breaks the contract — keep this
/// function as the single canonical path from `Claim` to JSON.
pub fn claim_to_canonical_json(claim: &Claim) -> serde_json::Value {
    // `Claim` already derives `Serialize` with the right `#[serde(rename)]`
    // attributes; this wrapper exists so the federation-contract intent
    // is visible at the call site (rather than a bare `to_value`).
    serde_json::to_value(claim).expect("Claim Serialize impl is infallible for the typed shape")
}

/// Parse a Lexicon-shaped JSON value into a `Claim`.
///
/// This is a thin wrapper around the derived `Deserialize` impl. For
/// inbound JSON from an untrusted peer, prefer `validate_claim_json` —
/// it runs the per-field gates (presence, range, enum) that produce
/// targeted `LexiconError` variants instead of an opaque serde error.
pub fn claim_from_json(value: &serde_json::Value) -> Result<Claim, LexiconError> {
    serde_json::from_value::<Claim>(value.clone()).map_err(|err| LexiconError::SchemaMismatch {
        message: err.to_string(),
    })
}

// -----------------------------------------------------------------------------
// In-crate unit tests — roundtrip contract for the lexicon-wire format
// -----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::claim::{ClaimReference, SignatureBlock};
    use serde_json::json;

    fn well_formed_claim() -> Claim {
        Claim {
            subject: "github:rust-lang/rust".to_string(),
            predicate: "embodiesPhilosophy".to_string(),
            object: "org.openlore.philosophy.memory-safety".to_string(),
            evidence: vec!["https://www.rust-lang.org/".to_string()],
            confidence: 0.86,
            author: "did:plc:jeff#org.openlore.application".to_string(),
            composed_at: "2026-05-25T12:00:00Z".to_string(),
            references: Vec::<ClaimReference>::new(),
            // slice-03 (step 01-07): the struct gained an optional `reason`.
            // None here keeps this slice-01-era fixture byte-stable.
            reason: None,
            signature: Some(SignatureBlock {
                kid: "did:plc:jeff#org.openlore.application".to_string(),
                alg: "EdDSA".to_string(),
                sig: "MEUCIQDz".to_string(),
            }),
        }
    }

    #[test]
    fn claim_serializes_with_lexicon_camelcase_field_names() {
        let claim = well_formed_claim();
        let value = claim_to_canonical_json(&claim);
        let obj = value.as_object().expect("top-level object");
        // Federation contract: the wire key is `composedAt`, not `composed_at`.
        assert!(obj.contains_key("composedAt"), "must emit `composedAt`");
        assert!(!obj.contains_key("composed_at"), "must NOT emit `composed_at`");
        // Author key is `author`, signature block uses `kid`/`alg`/`sig`.
        assert_eq!(obj["author"].as_str(), Some("did:plc:jeff#org.openlore.application"));
        let sig = obj["signature"].as_object().expect("signature object");
        assert!(sig.contains_key("kid") && sig.contains_key("alg") && sig.contains_key("sig"));
    }

    #[test]
    fn claim_roundtrips_through_canonical_json() {
        let original = well_formed_claim();
        let json_value = claim_to_canonical_json(&original);
        let recovered = claim_from_json(&json_value).expect("roundtrip parse");
        assert_eq!(original, recovered, "roundtrip must preserve all fields");
    }

    #[test]
    fn confidence_field_eq_is_bit_exact() {
        let a = ConfidenceField(0.86);
        let b = ConfidenceField(0.86);
        assert_eq!(a, b, "same bit pattern must compare equal");

        let c = ConfidenceField(0.86_f64 + 0.0); // identical bit pattern
        assert_eq!(a, c);

        let d = ConfidenceField(0.85);
        assert_ne!(a, d, "different bit pattern must compare unequal");
    }

    #[test]
    fn confidence_field_roundtrips_through_json_as_a_number() {
        let original = ConfidenceField(0.86);
        let json_value = serde_json::to_value(original).expect("serialize");
        assert_eq!(json_value, json!(0.86), "must serialize as a bare number");
        let recovered: ConfidenceField =
            serde_json::from_value(json_value).expect("deserialize");
        assert_eq!(original, recovered);
    }
}
