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

    /// `confidence` is outside the Lexicon-defined range `[0.0, 1.0]`
    /// (inclusive bounds, per JSON-Schema `minimum`/`maximum`).
    /// Step 02-02 wires this variant up to LC-5; the error display names
    /// both the field name and the valid range literally so peer-side
    /// diagnostics can pattern-match without parsing serde strings.
    #[error("confidence {value} is outside [0.0, 1.0]")]
    OutOfRangeConfidence { value: f64 },

    /// A `references[].type` value is not one of the four ADR-008 enums.
    #[error(
        "reference type `{value}` is not one of \
         {{retracts, corrects, counters, supersedes}}"
    )]
    InvalidReferenceType { value: String },

    /// `reason` is present but its character length falls outside the
    /// Lexicon-defined inclusive range `1..=1000` (ADR-015 `minLength: 1`,
    /// `maxLength: 1000`). Length is measured in Unicode scalar values
    /// (`chars().count()`), matching ATProto Lexicon string-length
    /// semantics (codepoints, not bytes or grapheme clusters). This is the
    /// Lexicon-layer defense-in-depth gate; the `claim counter` CLI verb
    /// (step 05-02) enforces the same bound at a different layer.
    #[error("reason length {length} is outside 1..=1000 (ADR-015 minLength 1 / maxLength 1000)")]
    ReasonLengthOutOfRange { length: usize },

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
    /// (slice-03; ADR-015) Optional free-text explanation. REQUIRED by the
    /// `claim counter` verb at the CLI level; permitted but semantically
    /// unused on other claim types. UTF-8 NFC-normalized at compose time
    /// (`claim-domain::normalize_reason`). OPTIONAL at the wire level (NOT
    /// in `required[]`) per ADR-005 forward-compat.
    ///
    /// `#[serde(default, skip_serializing_if = "Option::is_none")]` is
    /// load-bearing: a `reason: None` claim serializes byte-identically to
    /// a slice-01-era claim (the key is dropped entirely), preserving CID
    /// stability across the slice-01 -> slice-03 upgrade (I-FED-7). In CBOR
    /// canonical lex order (ADR-006) `reason` falls between `references`
    /// and `signature` — relevant only for claims that CARRY the field.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
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

    // Gate 4: reason length in 1..=1000 (ADR-015). Optional field —
    // the gate fires ONLY when `reason` is present as a JSON string.
    if let Some(reason) = object.get("reason") {
        if let Some(text) = reason.as_str() {
            validate_reason_length(text)?;
        }
        // A present-but-non-string `reason` (e.g. a number or null) is
        // left to the serde gate below to reject with a schema error;
        // the length gate only governs string-valued reasons.
    }

    // Gate 5: serde-level deserialization (final shape check).
    serde_json::from_value::<Claim>(value.clone()).map_err(|err| LexiconError::SchemaMismatch {
        message: err.to_string(),
    })
}

/// Enforce the ADR-015 `reason` length bound: `1..=1000` Unicode scalar
/// values (`chars().count()` — codepoints, matching ATProto Lexicon
/// string-length semantics; NOT bytes, NOT grapheme clusters).
///
/// Pure: takes the already-extracted reason text and returns
/// `Ok(())` when the length is in range, or
/// `ReasonLengthOutOfRange { length }` naming the offending char count.
/// Bounds are inclusive on both ends (`minLength: 1`, `maxLength: 1000`).
fn validate_reason_length(text: &str) -> Result<(), LexiconError> {
    const MIN_LENGTH: usize = 1;
    const MAX_LENGTH: usize = 1000;
    let length = text.chars().count();
    if (MIN_LENGTH..=MAX_LENGTH).contains(&length) {
        Ok(())
    } else {
        Err(LexiconError::ReasonLengthOutOfRange { length })
    }
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

    // -------------------------------------------------------------------------
    // Step 02-02: confidence range — defense-in-depth at the wire boundary.
    //
    // The Lexicon JSON declares `confidence: { type: "number", minimum: 0.0,
    // maximum: 1.0 }`. Per JSON-Schema convention `minimum`/`maximum` are
    // INCLUSIVE; ATProto inherits this. Boundary values (0.0 / 1.0) MUST
    // validate; anything strictly outside MUST reject. These unit tests
    // pin the contract; LC-5 is the acceptance-level counterpart.
    // -------------------------------------------------------------------------

    fn claim_with_confidence(confidence: f64) -> serde_json::Value {
        let mut value = well_formed_claim_value();
        value
            .as_object_mut()
            .expect("object")
            .insert("confidence".to_string(), json!(confidence));
        value
    }

    #[test]
    fn rejects_confidence_above_max_with_named_field_and_range() {
        let value = claim_with_confidence(1.4);
        let err = validate_claim_json(&value).expect_err("confidence=1.4 must reject");
        assert_eq!(err, LexiconError::OutOfRangeConfidence { value: 1.4 });
        let msg = err.to_string();
        assert!(msg.contains("confidence"), "msg must name field: {msg}");
        assert!(msg.contains("[0.0, 1.0]"), "msg must name range: {msg}");
    }

    #[test]
    fn rejects_confidence_below_min_with_named_field_and_range() {
        let value = claim_with_confidence(-0.1);
        let err = validate_claim_json(&value).expect_err("confidence=-0.1 must reject");
        assert_eq!(err, LexiconError::OutOfRangeConfidence { value: -0.1 });
        let msg = err.to_string();
        assert!(msg.contains("confidence"), "msg must name field: {msg}");
        assert!(msg.contains("[0.0, 1.0]"), "msg must name range: {msg}");
    }

    #[test]
    fn accepts_confidence_at_inclusive_lower_bound() {
        let value = claim_with_confidence(0.0);
        let claim =
            validate_claim_json(&value).expect("confidence=0.0 must validate (inclusive)");
        assert_eq!(claim.confidence, 0.0);
    }

    #[test]
    fn accepts_confidence_at_inclusive_upper_bound() {
        let value = claim_with_confidence(1.0);
        let claim =
            validate_claim_json(&value).expect("confidence=1.0 must validate (inclusive)");
        assert_eq!(claim.confidence, 1.0);
    }

    // -------------------------------------------------------------------------
    // Step 02-04: `reason` length — Lexicon-layer defense-in-depth.
    //
    // ADR-015 declares `reason: { type: "string", minLength: 1,
    // maxLength: 1000 }`. minLength/maxLength are INCLUSIVE; length is
    // measured in Unicode scalar values (`chars().count()`), matching
    // ATProto Lexicon string-length semantics (codepoints — NOT bytes,
    // NOT grapheme clusters). Slice-01 introduced no string-length gate
    // (it validates only presence, confidence range, and the reference
    // enum), so there is no prior length-unit convention to match; this
    // step establishes chars().count() per the ATProto default.
    //
    // `reason` is OPTIONAL at the wire level — the gate fires ONLY when
    // `reason` is present (a non-null JSON string). Absence / null defers
    // to the existing forward-compat behavior (reason -> None).
    //
    // Boundary-pinning example tests (no proptest at this layer: the
    // contract IS the four boundary points 0/1/1000/1001 plus the
    // chars-vs-bytes semantic; per nw-test-optimization Mandate 11 a
    // boundary contract is correctly pinned by its boundaries).
    // -------------------------------------------------------------------------

    fn claim_with_reason(reason: serde_json::Value) -> serde_json::Value {
        let mut value = well_formed_claim_value();
        value
            .as_object_mut()
            .expect("object")
            .insert("reason".to_string(), reason);
        value
    }

    #[test]
    fn rejects_empty_reason_at_inclusive_lower_bound_minus_one() {
        // length 0 < minLength 1 -> reject.
        let value = claim_with_reason(json!(""));
        let err = validate_claim_json(&value).expect_err("reason=\"\" (length 0) must reject");
        assert_eq!(err, LexiconError::ReasonLengthOutOfRange { length: 0 });
        let msg = err.to_string();
        assert!(msg.contains("1..=1000"), "msg must name the range: {msg}");
    }

    #[test]
    fn rejects_reason_one_over_inclusive_upper_bound() {
        // length 1001 > maxLength 1000 -> reject.
        let value = claim_with_reason(json!("a".repeat(1001)));
        let err =
            validate_claim_json(&value).expect_err("reason length 1001 must reject");
        assert_eq!(err, LexiconError::ReasonLengthOutOfRange { length: 1001 });
    }

    #[test]
    fn accepts_reason_at_inclusive_lower_bound() {
        // length 1 == minLength -> accept (inclusive).
        let value = claim_with_reason(json!("x"));
        let claim =
            validate_claim_json(&value).expect("reason length 1 must validate (inclusive)");
        assert_eq!(claim.reason.as_deref(), Some("x"));
    }

    #[test]
    fn accepts_reason_at_inclusive_upper_bound() {
        // length 1000 == maxLength -> accept (inclusive).
        let value = claim_with_reason(json!("a".repeat(1000)));
        let claim =
            validate_claim_json(&value).expect("reason length 1000 must validate (inclusive)");
        assert_eq!(claim.reason.as_deref().map(str::len), Some(1000));
    }

    #[test]
    fn measures_reason_length_in_chars_not_bytes() {
        // The load-bearing length-unit decision: "é" (U+00E9) is ONE
        // Unicode scalar value but TWO UTF-8 bytes. A 1000-`é` reason is
        // 1000 chars (== maxLength, ACCEPT) but 2000 bytes (would REJECT
        // under a byte gate). This test fails loudly if the validator ever
        // switches to byte-length, which would diverge from the ATProto
        // Lexicon codepoint semantics and break valid multi-byte reasons.
        let one_thousand_accented = "é".repeat(1000);
        assert_eq!(one_thousand_accented.chars().count(), 1000);
        assert_eq!(one_thousand_accented.len(), 2000, "precondition: 2 bytes/char");
        let value = claim_with_reason(json!(one_thousand_accented));
        let claim = validate_claim_json(&value)
            .expect("a 1000-CHAR (2000-byte) reason must validate: length is measured in chars");
        assert_eq!(claim.reason.as_deref().map(|s| s.chars().count()), Some(1000));

        // And one char over the limit (1001 chars / 2002 bytes) rejects
        // with the CHAR count, not the byte count.
        let value_over = claim_with_reason(json!("é".repeat(1001)));
        let err = validate_claim_json(&value_over)
            .expect_err("1001 chars must reject regardless of byte count");
        assert_eq!(
            err,
            LexiconError::ReasonLengthOutOfRange { length: 1001 },
            "the rejected length must be the CHAR count (1001), not the byte count (2002)"
        );
    }

    #[test]
    fn accepts_absent_reason_unchanged_forward_compat() {
        // The gate must NOT fire when `reason` is absent — slice-01
        // forward-compat (reason -> None) is preserved.
        let value = well_formed_claim_value(); // no `reason` key
        let claim =
            validate_claim_json(&value).expect("an absent reason must validate (-> None)");
        assert_eq!(claim.reason, None);
    }
}
