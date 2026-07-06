//! `org.openlore.philosophy` Lexicon model + validator (slice-01 pure core).
//!
//! Pure module: no I/O, no async, no adapters. Validates a JSON value
//! against the `org.openlore.philosophy` Lexicon record shape (see
//! `lexicons/org/openlore/philosophy.json`) and hosts the slice-01
//! embedded seed vocabulary (`seeds.json`, compile-time `include_str!`).
//!
//! Strategy MIRRORS `claim::validate_claim_json` (per ADR-059 D2 +
//! nw-fp-domain-modeling §8): per-field gates run BEFORE serde so errors
//! name the offending key rather than carrying an opaque serde string.
//! The SAME `LexiconError` enum is reused — no parallel error type.
//!
//! `object_id` is the deterministic join between the claim graph and the
//! vocabulary (ADR-059 D1): a claim's `object` string equals
//! `object_id(philosophy.name)`. It is DERIVED — never stored on the
//! record — and must stay byte-identical to the slice-01 claim `object`
//! literals (e.g. `org.openlore.philosophy.memory-safety`), so `normalize`
//! is total and idempotent on already-kebab input.

use crate::LexiconError;
use serde::{Deserialize, Serialize};

// =============================================================================
// Public NSID constant
// =============================================================================

pub const NSID: &str = "org.openlore.philosophy";

// =============================================================================
// Lexicon-shaped record type (serde mirror of the JSON schema)
// =============================================================================

/// Serde-modeled mirror of the `org.openlore.philosophy` Lexicon record.
///
/// Field names track the Lexicon JSON keys verbatim (`seeAlso` camelCase).
/// `required: [name, description]`; `aliases` / `seeAlso` are optional and
/// default to empty. The `object_id` is DERIVED from `name`, never stored.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Philosophy {
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub aliases: Vec<String>,
    #[serde(default, rename = "seeAlso")]
    pub see_also: Vec<String>,
}

// =============================================================================
// Required-field manifest (mirrors `lexicons/org/openlore/philosophy.json`)
// =============================================================================

/// Required top-level fields per the Lexicon JSON's `required` array.
const REQUIRED_FIELDS: &[&str] = &["name", "description"];

/// Optional array-of-string fields; gated to arrays-of-strings BEFORE serde
/// so a mis-typed value names the field rather than yielding a serde string.
const OPTIONAL_STRING_ARRAY_FIELDS: &[&str] = &["aliases", "seeAlso"];

// =============================================================================
// Validator
// =============================================================================

/// Validate a JSON value against the `org.openlore.philosophy` Lexicon.
///
/// Returns the parsed `Philosophy` on success, or a `LexiconError` naming
/// the violating field. Per-field gates run BEFORE serde deserialization
/// (mirroring `validate_claim_json`) so errors carry the field name.
pub fn validate_philosophy_json(value: &serde_json::Value) -> Result<Philosophy, LexiconError> {
    let object = value.as_object().ok_or_else(|| LexiconError::InvalidType {
        field: "(root)".to_string(),
        expected: "object".to_string(),
    })?;

    // Gate 1: required-field presence (names the offending key).
    for field in REQUIRED_FIELDS {
        if !object.contains_key(*field) {
            return Err(LexiconError::MissingField {
                field: (*field).to_string(),
            });
        }
    }

    // Gate 2: optional `aliases` / `seeAlso` must be arrays of strings when
    // present — gated before serde so the error names the field.
    for field in OPTIONAL_STRING_ARRAY_FIELDS {
        if let Some(present) = object.get(*field) {
            let array = present.as_array().ok_or_else(|| LexiconError::InvalidType {
                field: (*field).to_string(),
                expected: "array of string".to_string(),
            })?;
            if array.iter().any(|item| !item.is_string()) {
                return Err(LexiconError::InvalidType {
                    field: (*field).to_string(),
                    expected: "array of string".to_string(),
                });
            }
        }
    }

    // Gate 3: serde-level deserialization (final shape check).
    serde_json::from_value::<Philosophy>(value.clone()).map_err(|err| {
        LexiconError::SchemaMismatch {
            message: err.to_string(),
        }
    })
}

// =============================================================================
// Deterministic object-id derivation (ADR-059 D1) — pure + total
// =============================================================================

/// Normalize a philosophy `name` into its kebab-case identifier segment.
///
/// Pure + total: lowercase -> trim -> map runs of whitespace/underscore/dash
/// to a single `-` -> drop any character outside `[a-z0-9-]` -> collapse
/// duplicate `-` and trim leading/trailing `-`. Idempotent on already-kebab
/// input, so `normalize("memory-safety") == "memory-safety"` and the seed
/// object ids stay byte-identical to the slice-01 claim `object` literals.
pub fn normalize(name: &str) -> String {
    let lowered = name.to_lowercase();
    let mut result = String::with_capacity(lowered.len());
    // `prev_dash` starts true so any leading separators are trimmed.
    let mut prev_dash = true;
    for ch in lowered.trim().chars() {
        if ch.is_ascii_alphanumeric() {
            result.push(ch);
            prev_dash = false;
        } else if ch.is_whitespace() || ch == '_' || ch == '-' {
            // Separator: collapse runs into a single '-'.
            if !prev_dash {
                result.push('-');
                prev_dash = true;
            }
        }
        // Any other punctuation is dropped without emitting a separator.
    }
    // Trim a trailing separator.
    while result.ends_with('-') {
        result.pop();
    }
    result
}

/// Derive the deterministic object id for a philosophy `name` (ADR-059 D1).
///
/// `object_id(name) == "org.openlore.philosophy.{normalize(name)}"` — i.e. the
/// `NSID`-prefixed, normalized segment. This is the join key a claim's `object`
/// string must equal; it is DERIVED, never stored on the `Philosophy` record.
pub fn object_id(name: &str) -> String {
    format!("{NSID}.{}", normalize(name))
}

// =============================================================================
// Embedded seed vocabulary (slice-01) — compile-time `include_str!`
// =============================================================================

/// The slice-01 philosophy seed vocabulary, embedded at compile time.
/// A JSON array of `{ name, description, aliases?, seeAlso? }` records.
pub const PHILOSOPHY_SEEDS_JSON: &str = include_str!("seeds.json");

/// Parse and return the embedded slice-01 philosophy seeds.
///
/// Each record is validated through `validate_philosophy_json`; a malformed
/// embedded seed is a compile-time-authored bug and panics loudly (this is
/// static data baked into the binary, not runtime input).
pub fn seeds() -> Vec<Philosophy> {
    let value: serde_json::Value =
        serde_json::from_str(PHILOSOPHY_SEEDS_JSON).expect("embedded seeds.json must be valid JSON");
    value
        .as_array()
        .expect("embedded seeds.json must be a JSON array")
        .iter()
        .map(|record| {
            validate_philosophy_json(record).expect("every embedded philosophy seed must validate")
        })
        .collect()
}
