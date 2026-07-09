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

    // Gate 1b (slice-24, AC-003.4): a PRESENT-but-blank required string
    // (empty or whitespace-only) carries no usable value, so it is rejected
    // exactly like an absent one — reusing `MissingField` (no parallel error
    // type, ADR-059) so the error names the offending field. Placing this in
    // the PURE validator means the scraper mint path inherits it too, not just
    // the CLI. Non-string required values fall through to the serde gate below.
    for field in REQUIRED_FIELDS {
        if let Some(serde_json::Value::String(text)) = object.get(*field) {
            if text.trim().is_empty() {
                return Err(LexiconError::MissingField {
                    field: (*field).to_string(),
                });
            }
        }
    }

    // Gate 2: optional `aliases` / `seeAlso` must be arrays of strings when
    // present — gated before serde so the error names the field.
    for field in OPTIONAL_STRING_ARRAY_FIELDS {
        if let Some(present) = object.get(*field) {
            let array = present
                .as_array()
                .ok_or_else(|| LexiconError::InvalidType {
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
    let value: serde_json::Value = serde_json::from_str(PHILOSOPHY_SEEDS_JSON)
        .expect("embedded seeds.json must be valid JSON");
    value
        .as_array()
        .expect("embedded seeds.json must be a JSON array")
        .iter()
        .map(|record| {
            validate_philosophy_json(record).expect("every embedded philosophy seed must validate")
        })
        .collect()
}

// =============================================================================
// Vocabulary resolution (slice-23; ADR-059 §5) — pure + total
// =============================================================================

/// Resolve a philosophy seed by EITHER its bare name OR its derived object id
/// (ADR-059 §5 slice-23 — `philosophy show` accepts name-OR-object).
///
/// Pure + total: returns the seed whose derived `object_id(&name) == key`, OR
/// (falling back) whose `normalize(&name) == normalize(key)` — so both
/// `memory-safety` and `org.openlore.philosophy.memory-safety` resolve to the
/// SAME record, and resolution is case/separator-insensitive (both sides pass
/// through `normalize`). An unknown key resolves to `None` (never a panic).
pub fn find(key: &str) -> Option<Philosophy> {
    let normalized_key = normalize(key);
    seeds()
        .into_iter()
        .find(|seed| object_id(&seed.name) == key || normalize(&seed.name) == normalized_key)
}

// =============================================================================
// Alias-aware compose-advisory resolver (slice-25 01-01; ADR-059 §5 row 25)
// =============================================================================

/// The display-only classification of a claim `--object` against the embedded
/// philosophy vocabulary, for the `claim add` compose advisory (US-PV-004).
///
/// Pure verdict, NEVER a gate and NEVER a payload rewrite: the CLI turns this
/// into ONE preview line, but the object the user typed is what gets signed
/// (AC-004.3). `find()` matches name/object-id only; the `Alias` arm — matching
/// a seed's `aliases` entry and reporting the canonical name — is new here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ObjectAdvisory {
    /// The object is the derived object-id of a known philosophy `name`.
    Canonical { name: String },
    /// The object matched a seed's ALIAS; `canonical` is that seed's `name`.
    Alias { canonical: String },
    /// In the `org.openlore.philosophy.*` namespace but matches no seed.
    UnknownInNamespace,
    /// Outside the philosophy namespace — not a philosophy claim, no advisory.
    NotPhilosophy,
}

/// Classify a claim `--object` string for the compose advisory (pure + total,
/// offline — embedded seeds only, no store/network).
///
/// Algorithm (ADR-059 §5 row 25): if `object` is not prefixed by the philosophy
/// object-id prefix (`org.openlore.philosophy.`, derived from `object_id`) →
/// `NotPhilosophy`. Else take the segment after the prefix and, over `seeds()`:
/// if `normalize(segment) == normalize(seed.name)` for some seed → `Canonical`;
/// else if `normalize(alias) == normalize(segment)` for some seed's alias →
/// `Alias { canonical = seed.name }`; else → `UnknownInNamespace`. Reuses
/// `seeds()`, `normalize()`, and the `object_id` prefix (`{NSID}.`) — never a
/// second hardcoded prefix copy.
pub fn resolve_object_advisory(object: &str) -> ObjectAdvisory {
    let prefix = format!("{NSID}.");
    let Some(segment) = object.strip_prefix(&prefix) else {
        return ObjectAdvisory::NotPhilosophy;
    };
    let segment_normalized = normalize(segment);

    // Canonical takes precedence: a direct name match over every seed.
    if let Some(seed) = seeds()
        .into_iter()
        .find(|seed| normalize(&seed.name) == segment_normalized)
    {
        return ObjectAdvisory::Canonical { name: seed.name };
    }

    // Else an alias match reports the seed's canonical name.
    if let Some(seed) = seeds().into_iter().find(|seed| {
        seed.aliases
            .iter()
            .any(|alias| normalize(alias) == segment_normalized)
    }) {
        return ObjectAdvisory::Alias {
            canonical: seed.name,
        };
    }

    // In the namespace but matches no seed name or alias.
    ObjectAdvisory::UnknownInNamespace
}

#[cfg(test)]
mod tests {
    //! Port-to-port unit tests at the pure-resolver scope: the driving port is
    //! `find`'s signature; the observable outcome is the returned `Option`.
    //! Property-based (Hebert ch.3 Generalizing + Invariant/Oracle) over the
    //! WHOLE embedded seed set, not a hand-built fixture.

    use super::*;
    use proptest::prelude::*;
    use serde_json::json;

    proptest! {
        /// AC-003.4 / PA-4 (slice-24): a PRESENT-but-blank required string
        /// (empty or whitespace-only) is rejected with a NAMED-field error —
        /// naming the offending field, reusing `LexiconError::MissingField` (no
        /// parallel error type, ADR-059). Property over both required string
        /// fields (name, description) and the whole blank equivalence class
        /// (empty + arbitrary whitespace runs of spaces/tabs/newlines/CRs).
        #[test]
        fn blank_required_string_rejects_naming_the_field(
            blank in "[ \\t\\n\\r]{0,8}",
        ) {
            let description_blank =
                json!({ "name": "capability-security", "description": blank.clone() });
            prop_assert_eq!(
                validate_philosophy_json(&description_blank)
                    .expect_err("a blank `description` must reject"),
                LexiconError::MissingField { field: "description".to_string() }
            );

            let name_blank =
                json!({ "name": blank.clone(), "description": "a real, non-blank description" });
            prop_assert_eq!(
                validate_philosophy_json(&name_blank)
                    .expect_err("a blank `name` must reject"),
                LexiconError::MissingField { field: "name".to_string() }
            );
        }
    }

    proptest! {
        /// No regression (slice-24): a well-formed record — non-blank `name`
        /// AND non-blank `description` — still validates and round-trips both
        /// fields verbatim. The blank gate must not reject genuine content.
        #[test]
        fn well_formed_record_still_validates(
            name in "[a-z][a-z0-9 _-]{0,20}",
            description in "[A-Za-z][A-Za-z0-9 ._-]{0,60}",
        ) {
            let value = json!({ "name": name.clone(), "description": description.clone() });
            let parsed = validate_philosophy_json(&value)
                .expect("a well-formed philosophy record must validate");
            prop_assert_eq!(parsed.name, name);
            prop_assert_eq!(parsed.description, description);
        }
    }

    proptest! {
        /// Generalizing / round-trip (PS-1/PS-2 name-OR-object contract): every
        /// seed resolves back to ITSELF from both its bare name AND its derived
        /// object id, and the match is case-insensitive (both sides normalize).
        #[test]
        fn find_resolves_every_seed_by_name_and_object_id(
            seed in prop::sample::select(seeds())
        ) {
            let by_name = find(&seed.name);
            prop_assert_eq!(by_name.as_ref(), Some(&seed));
            let by_id = find(&object_id(&seed.name));
            prop_assert_eq!(by_id.as_ref(), Some(&seed));
            let by_upper = find(&seed.name.to_uppercase());
            prop_assert_eq!(by_upper.as_ref(), Some(&seed));
        }
    }

    proptest! {
        /// 01-01 (AC-004.1/.2 + out-of-namespace): the compose-advisory resolver
        /// classifies EVERY seed's derived object-id as `Canonical { name }`,
        /// each of its aliases (in-namespace) as `Alias { canonical = name }`, an
        /// in-namespace segment that matches no seed as `UnknownInNamespace`, and
        /// an arbitrary out-of-namespace string as `NotPhilosophy`. Generalizing
        /// (Hebert ch.3) over the whole embedded seed set + generated segments.
        #[test]
        fn resolve_object_advisory_classifies_known_alias_unknown_and_non_philosophy(
            seed in prop::sample::select(seeds()),
            unknown_seg in "[a-z][a-z0-9-]{0,20}",
            outsider in ".*",
        ) {
            // Canonical: a seed's derived object-id resolves to Canonical{name}.
            prop_assert_eq!(
                resolve_object_advisory(&object_id(&seed.name)),
                ObjectAdvisory::Canonical { name: seed.name.clone() }
            );

            // Alias: each in-namespace alias resolves to Alias{canonical=name},
            // reporting the CANONICAL — not the typed alias segment.
            for alias in &seed.aliases {
                prop_assert_eq!(
                    resolve_object_advisory(&format!("{NSID}.{alias}")),
                    ObjectAdvisory::Alias { canonical: seed.name.clone() }
                );
            }

            // Unknown-in-namespace: a prefixed segment matching NO seed name and
            // NO seed alias resolves to UnknownInNamespace.
            let seg_norm = normalize(&unknown_seg);
            let matches_a_seed = seeds().iter().any(|s| {
                normalize(&s.name) == seg_norm
                    || s.aliases.iter().any(|a| normalize(a) == seg_norm)
            });
            prop_assume!(!matches_a_seed);
            prop_assert_eq!(
                resolve_object_advisory(&format!("{NSID}.{unknown_seg}")),
                ObjectAdvisory::UnknownInNamespace
            );

            // Not-philosophy: any string NOT prefixed by `{NSID}.` resolves to
            // NotPhilosophy (no advisory, no nagging).
            prop_assume!(!outsider.starts_with(&format!("{NSID}.")));
            prop_assert_eq!(
                resolve_object_advisory(&outsider),
                ObjectAdvisory::NotPhilosophy
            );
        }
    }

    proptest! {
        /// Totality + soundness/completeness (Oracle): `find` never panics on
        /// arbitrary input; any `Some(record)` genuinely matches the key by
        /// object id OR normalized name; and a `None` means NO seed matched
        /// (a linear-scan reference confirms the miss).
        #[test]
        fn find_is_total_and_sound_over_arbitrary_input(key in ".*") {
            match find(&key) {
                Some(record) => prop_assert!(
                    object_id(&record.name) == key
                        || normalize(&record.name) == normalize(&key)
                ),
                None => {
                    for seed in seeds() {
                        prop_assert_ne!(object_id(&seed.name), key.clone());
                        prop_assert_ne!(normalize(&seed.name), normalize(&key));
                    }
                }
            }
        }
    }
}
