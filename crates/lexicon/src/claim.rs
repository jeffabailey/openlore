//! `org.openlore.claim` Lexicon model + validator (step 02-01).
//!
//! Pure module: no I/O, no async, no adapters. Validates a JSON value
//! against the `org.openlore.claim` Lexicon record shape (see
//! `lexicons/org/openlore/claim.json` and `data-models.md`).
//!
//! Strategy (per nw-fp-domain-modeling §3 + §8):
//!   1. Walk the input JSON value as `serde_json::Value`.
//!   2. Enforce required-field presence — return `MissingField { field }`
//!      naming the violating key (NOT a serde error string).
//!   3. Enforce confidence range `[0.0, 1.0]` (data-models.md / WD-10) —
//!      `OutOfRangeConfidence` names the bad value.
//!   4. Enforce reference `type` enum (ADR-008) — `InvalidReferenceType`.
//!   5. Only THEN deserialize into the typed `Claim` via serde.
//!
//! This step (02-01) turns LC-2 GREEN. LC-5/LC-6/LC-7/LC-8 remain RED
//! because their test bodies are still `todo!()` scaffolds — they will
//! be filled in by steps 02-02, 03-03, 03-04, 02-07 respectively.
//!
//! Field-naming follows the Lexicon JSON keys verbatim (camelCase
//! `composedAt`) per the federation contract — any drift breaks
//! cross-peer deserialization in slice-03.

use serde::{Deserialize, Serialize};

// =============================================================================
// Public NSID constant
// =============================================================================

pub const NSID: &str = "org.openlore.claim";

// =============================================================================
// Typed error — railway-oriented per nw-fp-domain-modeling §8
// =============================================================================

/// Errors emitted by `validate_claim_json`. Each variant names the
/// offending field so callers can map errors to user-facing messages
/// without parsing strings.
#[derive(Debug, thiserror::Error, PartialEq)]
pub enum LexiconError {
    /// A required field is missing from the input JSON object.
    #[error("required field `{field}` missing")]
    MissingField { field: String },

    /// A field is present but has the wrong JSON type.
    #[error("field `{field}` has invalid type: expected {expected}")]
    InvalidType { field: String, expected: String },

    /// `confidence` is outside the Lexicon-defined range `[0.0, 1.0]`.
    #[error("confidence {value} is outside [0.0, 1.0]")]
    OutOfRangeConfidence { value: f64 },

    /// A `references[].type` value is not one of the four ADR-008 enums.
    #[error(
        "reference type `{value}` is not one of \
         {{retracts, corrects, counters, supersedes}}"
    )]
    InvalidReferenceType { value: String },

    /// Catch-all for serde-level deserialization failures after the
    /// per-field gates above have passed. Should be rare in practice.
    #[error("schema mismatch: {message}")]
    SchemaMismatch { message: String },
}

// =============================================================================
// Lexicon-shaped record types (serde mirrors of the JSON schema)
// =============================================================================

/// Serde-modeled mirror of the `org.openlore.claim` Lexicon record.
///
/// Field names track the Lexicon JSON keys verbatim (camelCase). Any
/// drift here breaks the federation contract — peers in slice-03
/// deserialize incoming claims against THIS struct's serde shape.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Claim {
    pub subject: String,
    pub predicate: String,
    pub object: String,
    #[serde(default)]
    pub evidence: Vec<String>,
    pub confidence: f64,
    pub author: String,
    #[serde(rename = "composedAt")]
    pub composed_at: String,
    #[serde(default)]
    pub references: Vec<ClaimReference>,
    #[serde(default)]
    pub signature: Option<SignatureBlock>,
}

/// One typed reference from this claim to another (ADR-008).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClaimReference {
    #[serde(rename = "type")]
    pub ref_type: String,
    pub cid: String,
}

/// Signature block attached when the claim is signed (ADR-006).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SignatureBlock {
    pub kid: String,
    pub alg: String,
    pub sig: String,
}

// =============================================================================
// Required-field manifest (mirrors `lexicons/org/openlore/claim.json`)
// =============================================================================

/// Required top-level fields per the Lexicon JSON's `required` array.
/// Kept as a pure constant — no JSON re-parsing at call time.
const REQUIRED_FIELDS: &[&str] = &[
    "subject",
    "predicate",
    "object",
    "confidence",
    "author",
    "composedAt",
];

/// Required fields on each entry in the `references` array.
const REQUIRED_REFERENCE_FIELDS: &[&str] = &["type", "cid"];

/// Allowed `references[].type` values per ADR-008.
const ALLOWED_REFERENCE_TYPES: &[&str] =
    &["retracts", "corrects", "counters", "supersedes"];

// =============================================================================
// Validator
// =============================================================================

/// Validate a JSON value against the `org.openlore.claim` Lexicon.
///
/// Returns the parsed `Claim` on success, or a `LexiconError` naming
/// the violating field. Per-field gates run BEFORE serde deserialization
/// so errors carry the field name rather than an opaque serde string.
pub fn validate_claim_json(value: &serde_json::Value) -> Result<Claim, LexiconError> {
    let object = value
        .as_object()
        .ok_or_else(|| LexiconError::InvalidType {
            field: "(root)".to_string(),
            expected: "object".to_string(),
        })?;

    // Gate 1: required-field presence.
    for field in REQUIRED_FIELDS {
        if !object.contains_key(*field) {
            return Err(LexiconError::MissingField {
                field: (*field).to_string(),
            });
        }
    }

    // Gate 2: confidence in [0.0, 1.0] (data-models.md / WD-10).
    let confidence_value = object
        .get("confidence")
        .and_then(serde_json::Value::as_f64)
        .ok_or_else(|| LexiconError::InvalidType {
            field: "confidence".to_string(),
            expected: "number".to_string(),
        })?;
    if !(0.0..=1.0).contains(&confidence_value) {
        return Err(LexiconError::OutOfRangeConfidence {
            value: confidence_value,
        });
    }

    // Gate 3: references[].type enum (ADR-008). Optional array.
    if let Some(refs) = object.get("references") {
        let array = refs.as_array().ok_or_else(|| LexiconError::InvalidType {
            field: "references".to_string(),
            expected: "array".to_string(),
        })?;
        for (index, entry) in array.iter().enumerate() {
            let entry_obj = entry
                .as_object()
                .ok_or_else(|| LexiconError::InvalidType {
                    field: format!("references[{index}]"),
                    expected: "object".to_string(),
                })?;
            for field in REQUIRED_REFERENCE_FIELDS {
                if !entry_obj.contains_key(*field) {
                    return Err(LexiconError::MissingField {
                        field: format!("references[{index}].{field}"),
                    });
                }
            }
            let ref_type = entry_obj
                .get("type")
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| LexiconError::InvalidType {
                    field: format!("references[{index}].type"),
                    expected: "string".to_string(),
                })?;
            if !ALLOWED_REFERENCE_TYPES.contains(&ref_type) {
                return Err(LexiconError::InvalidReferenceType {
                    value: ref_type.to_string(),
                });
            }
        }
    }

    // Gate 4: serde-level deserialization (final shape check).
    serde_json::from_value::<Claim>(value.clone()).map_err(|err| LexiconError::SchemaMismatch {
        message: err.to_string(),
    })
}

// =============================================================================
// In-crate unit tests
// =============================================================================
//
// Functional/PBT note (nw-tdd-methodology §RED_UNIT): a single happy-path
// example + targeted missing-field/range/enum error cases. Property-based
// tests over the full claim-shape lattice belong in LC-3 (handled in a
// later step), not here — `validate_claim_json` is exercised primarily
// via the LC-2 acceptance test.

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn well_formed_claim_value() -> serde_json::Value {
        json!({
            "subject": "github:rust-lang/rust",
            "predicate": "embodiesPhilosophy",
            "object": "org.openlore.philosophy.memory-safety",
            "evidence": ["https://github.com/rust-lang/rust"],
            "confidence": 0.85,
            "author": "did:plc:test-jeff#org.openlore.application",
            "composedAt": "2026-05-25T12:00:00Z",
            "references": [],
            "signature": {
                "kid": "did:plc:test-jeff#org.openlore.application",
                "alg": "EdDSA",
                "sig": "AAAA"
            }
        })
    }

    #[test]
    fn validates_well_formed_signed_claim() {
        let value = well_formed_claim_value();
        let claim = validate_claim_json(&value).expect("well-formed claim must validate");
        assert_eq!(claim.subject, "github:rust-lang/rust");
        assert_eq!(claim.predicate, "embodiesPhilosophy");
        assert_eq!(claim.confidence, 0.85);
        assert_eq!(
            claim.composed_at, "2026-05-25T12:00:00Z",
            "composedAt must round-trip verbatim"
        );
        assert!(claim.signature.is_some());
    }

    #[test]
    fn rejects_missing_subject_with_named_field_error() {
        let mut value = well_formed_claim_value();
        value
            .as_object_mut()
            .expect("object")
            .remove("subject");
        let err = validate_claim_json(&value).expect_err("missing subject must reject");
        assert_eq!(
            err,
            LexiconError::MissingField {
                field: "subject".to_string()
            }
        );
    }

    #[test]
    fn rejects_missing_composed_at_with_camelcase_field_name() {
        let mut value = well_formed_claim_value();
        value
            .as_object_mut()
            .expect("object")
            .remove("composedAt");
        let err = validate_claim_json(&value).expect_err("missing composedAt must reject");
        assert_eq!(
            err,
            LexiconError::MissingField {
                field: "composedAt".to_string()
            }
        );
    }

    #[test]
    fn rejects_non_object_root() {
        let value = serde_json::json!("not an object");
        let err = validate_claim_json(&value).expect_err("non-object root must reject");
        assert!(matches!(err, LexiconError::InvalidType { .. }));
    }

    #[test]
    fn validates_claim_with_no_signature() {
        // signature is OPTIONAL per the Lexicon JSON (not in `required`).
        let mut value = well_formed_claim_value();
        value
            .as_object_mut()
            .expect("object")
            .remove("signature");
        let claim = validate_claim_json(&value)
            .expect("unsigned-but-otherwise-valid claim must validate");
        assert!(claim.signature.is_none());
    }
}
