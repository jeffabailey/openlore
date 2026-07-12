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
// Name/object/alias resolver (slice-30) — pure + total
// =============================================================================

/// Resolve a philosophy seed by its bare name, its derived object id, OR any of
/// its aliases — the key `philosophy show <name|object|alias>` drives on.
///
/// Pure + total. First tries [`find`] (canonical name or object id). Failing
/// that, an ALIAS lookup: the key is treated as an alias segment — prefixed into
/// the philosophy namespace if bare — and classified by [`resolve_object_advisory`];
/// an `Alias { canonical }` verdict resolves to that canonical seed. A key that is
/// neither a name, an object id, nor a known alias resolves to `None` (never a
/// panic). Both `resolve("xp")` and `resolve("org.openlore.philosophy.xp")` return
/// the `extreme-programming` seed, case-insensitively. REUSES
/// `find`/`resolve_object_advisory`/`object_id` — never a second copy of the
/// resolution logic or the NSID prefix. Unlike [`find`] (whose `Some` is always a
/// name/object-id match), a `resolve` hit may be reached via an alias.
pub fn resolve(key: &str) -> Option<Philosophy> {
    // Exact name / object-id resolution first (find's contract).
    if let Some(record) = find(key) {
        return Some(record);
    }
    // Else treat the key as an alias segment. `resolve_object_advisory` matches
    // aliases only in the philosophy namespace, so prefix a bare key with the
    // NSID (via `object_id`) before classifying — never a second prefix copy.
    let prefix = format!("{NSID}.");
    let prefixed = if key.starts_with(&prefix) {
        key.to_string()
    } else {
        object_id(key)
    };
    match resolve_object_advisory(&prefixed) {
        ObjectAdvisory::Alias { canonical } => find(&canonical),
        ObjectAdvisory::Canonical { .. }
        | ObjectAdvisory::UnknownInNamespace
        | ObjectAdvisory::NotPhilosophy => None,
    }
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
    // Parse the embedded seed set ONCE and share it across both passes.
    let all_seeds = seeds();

    // Canonical takes precedence: a direct name match over every seed.
    if let Some(seed) = all_seeds
        .iter()
        .find(|seed| normalize(&seed.name) == segment_normalized)
    {
        return ObjectAdvisory::Canonical {
            name: seed.name.clone(),
        };
    }

    // Else an alias match reports the seed's canonical name.
    if let Some(seed) = all_seeds.iter().find(|seed| {
        seed.aliases
            .iter()
            .any(|alias| normalize(alias) == segment_normalized)
    }) {
        return ObjectAdvisory::Alias {
            canonical: seed.name.clone(),
        };
    }

    // In the namespace but matches no seed name or alias.
    ObjectAdvisory::UnknownInNamespace
}

// =============================================================================
// Equivalence-class resolver (slice-26 01-01; ADR-059 §5 row 26) — pure + total
// =============================================================================

/// The set of philosophy object-ids that triangulate together with `object`.
///
/// Pure + total, offline (embedded seeds only). Given ANY object in a seed
/// philosophy's family — the canonical object-id `object_id(name)` OR one of its
/// alias object-ids `object_id(alias)` — returns EVERY object-id in that family:
/// the canonical FIRST, then one per alias in seed order (deduped, stable). Given
/// an object that is not a known philosophy (unknown-in-namespace, or a
/// non-philosophy object) returns the SINGLETON `[object]`, so a query by a
/// no-alias philosophy or a non-philosophy object stays byte-identical to an
/// exact match (no over-widening — AT-5).
///
/// REUSES `resolve_object_advisory`/`find`/`object_id` — never a second copy of
/// resolution or the NSID prefix. This is the pure seam the `adapter-duckdb`
/// object-dimension read widens on (slice-26 02-01): resolution is a read-time
/// derivation; it NEVER rewrites the stored claim object (AC-005.2).
pub fn equivalence_class(object: &str) -> Vec<String> {
    // Resolve to the seed's canonical NAME; a non-philosophy / unknown object has
    // no family, so it is its own singleton class.
    let canonical_name = match resolve_object_advisory(object) {
        ObjectAdvisory::Canonical { name } => name,
        ObjectAdvisory::Alias { canonical } => canonical,
        ObjectAdvisory::UnknownInNamespace | ObjectAdvisory::NotPhilosophy => {
            return vec![object.to_string()];
        }
    };

    // `resolve_object_advisory` only reports Canonical/Alias for a seed match, so
    // `find` succeeds; the singleton fallback keeps the function total regardless.
    let Some(seed) = find(&canonical_name) else {
        return vec![object.to_string()];
    };

    // Canonical object-id first, then each alias object-id (deduped, stable).
    let mut class = vec![object_id(&seed.name)];
    for alias in &seed.aliases {
        let alias_id = object_id(alias);
        if !class.contains(&alias_id) {
            class.push(alias_id);
        }
    }
    class
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
        /// 01-01 (AC-005.1 triangulation + AT-5 singleton no-over-widening):
        /// `equivalence_class` maps EVERY object in a seed's family — the seed's
        /// canonical object-id AND each of its alias object-ids — to the SAME
        /// class `{ canonical } ∪ { alias ids }`, with the canonical FIRST; and
        /// an arbitrary object that is NOT a known philosophy (out-of-namespace or
        /// unknown-in-namespace) maps to its own SINGLETON `[object]` (so a query
        /// by a no-alias philosophy or a non-philosophy object stays byte-identical
        /// to today — no cross-class leak). Generalizing (Hebert ch.3) over the
        /// whole embedded seed set + generated outsiders.
        #[test]
        fn equivalence_class_maps_seed_family_together_else_singleton(
            seed in prop::sample::select(seeds()),
            outsider in ".*",
        ) {
            // The expected class: canonical object-id FIRST, then each alias id in
            // seed order (deduped, stable).
            let mut expected = vec![object_id(&seed.name)];
            for alias in &seed.aliases {
                let alias_id = object_id(alias);
                if !expected.contains(&alias_id) {
                    expected.push(alias_id);
                }
            }

            // The canonical object-id resolves to the whole class, canonical first.
            let canonical_id = object_id(&seed.name);
            let from_canonical = equivalence_class(&canonical_id);
            prop_assert_eq!(&from_canonical, &expected);
            prop_assert_eq!(from_canonical.first(), Some(&canonical_id));

            // EVERY alias object-id resolves to the SAME class (triangulation is
            // symmetric — an alias query sees the canonical + its siblings).
            for alias in &seed.aliases {
                prop_assert_eq!(equivalence_class(&object_id(alias)), expected.clone());
            }

            // A non-philosophy / unknown-in-namespace object → SINGLETON [object]
            // (no over-widening; AT-5 no-regression guardrail).
            prop_assume!(matches!(
                resolve_object_advisory(&outsider),
                ObjectAdvisory::NotPhilosophy | ObjectAdvisory::UnknownInNamespace
            ));
            prop_assert_eq!(equivalence_class(&outsider), vec![outsider.clone()]);
        }
    }

    /// slice-29 (seed-vocabulary expansion): EVERY normalized name AND alias
    /// across the whole seed set is globally unique. This is the invariant the
    /// resolvers rely on — `find`/`resolve_object_advisory` return the FIRST
    /// match, so a duplicated token would silently mis-resolve one seed to
    /// another (and break `find_resolves_every_seed_by_name_and_object_id`).
    /// Pinned explicitly so a future seed collision fails with a named token.
    #[test]
    fn every_seed_name_and_alias_normalizes_uniquely() {
        use std::collections::HashMap;
        let mut owner: HashMap<String, String> = HashMap::new();
        for seed in seeds() {
            for token in std::iter::once(&seed.name).chain(seed.aliases.iter()) {
                let normalized = normalize(token);
                if let Some(previous) = owner.insert(normalized.clone(), seed.name.clone()) {
                    panic!(
                        "seed token `{token}` (normalized `{normalized}`) collides: \
                         owned by both `{previous}` and `{}`",
                        seed.name
                    );
                }
            }
        }
    }

    /// slice-29: the vocabulary is a SUBSTANTIAL, curated set spanning the three
    /// categories a reader expects in most software projects — process
    /// methodologies, design principles, and architecture/ops patterns. Anchored
    /// on canonical names (resolvable by name AND derived object-id) so the
    /// expansion cannot silently regress below a useful floor.
    #[test]
    fn seed_vocabulary_covers_common_software_philosophies() {
        let all = seeds();
        assert!(
            all.len() >= 60,
            "expected a substantial seeded vocabulary, found {}",
            all.len()
        );
        for anchor in [
            "agile",                  // process methodology
            "devops",                 // process methodology
            "solid",                  // design principle
            "dry",                    // design principle
            "clean-architecture",     // architecture pattern
            "infrastructure-as-code", // ops pattern
        ] {
            let record = find(anchor)
                .unwrap_or_else(|| panic!("canonical philosophy `{anchor}` must be seeded"));
            assert_eq!(find(&object_id(anchor)).as_ref(), Some(&record));
        }
    }

    proptest! {
        /// slice-30 (`philosophy show <name|object|alias>`): `resolve` finds every
        /// seed by its bare name, its derived object id, AND every one of its
        /// aliases — in both bare and object-id form, and case-insensitively. This
        /// is the name-or-object-OR-alias contract the `show` verb drives on, a
        /// strict superset of `find`. Generalizing (Hebert ch.3) over the whole
        /// embedded seed set; the global name/alias uniqueness pin guarantees each
        /// alias resolves to exactly ONE seed.
        #[test]
        fn resolve_finds_every_seed_by_name_object_id_and_alias(
            seed in prop::sample::select(seeds()),
        ) {
            // Superset of `find`: canonical name and derived object id still hit.
            let by_name = resolve(&seed.name);
            prop_assert_eq!(by_name.as_ref(), Some(&seed));
            let by_object = resolve(&object_id(&seed.name));
            prop_assert_eq!(by_object.as_ref(), Some(&seed));

            // NEW: every alias resolves to its canonical seed — bare, in object-id
            // form, and upper-cased (case-insensitive via `normalize`).
            for alias in &seed.aliases {
                let by_alias = resolve(alias);
                prop_assert_eq!(by_alias.as_ref(), Some(&seed));
                let by_alias_object = resolve(&object_id(alias));
                prop_assert_eq!(by_alias_object.as_ref(), Some(&seed));
                let by_alias_upper = resolve(&alias.to_uppercase());
                prop_assert_eq!(by_alias_upper.as_ref(), Some(&seed));
            }
        }
    }

    proptest! {
        /// slice-30 totality: `resolve` never panics on arbitrary input, and a
        /// key that resolves to NO seed by name, object id, or alias returns
        /// `None` (a linear-scan reference over the seed set confirms the miss).
        #[test]
        fn resolve_is_total_and_sound_over_arbitrary_input(key in ".*") {
            match resolve(&key) {
                Some(record) => {
                    let key_norm = normalize(&key);
                    let matches = object_id(&record.name) == key
                        || normalize(&record.name) == key_norm
                        || record.aliases.iter().any(|a| normalize(a) == key_norm)
                        || record
                            .aliases
                            .iter()
                            .any(|a| object_id(a) == key || object_id(a) == object_id(&key));
                    prop_assert!(matches, "resolve returned a seed the key does not name");
                }
                None => {
                    for seed in seeds() {
                        prop_assert_ne!(normalize(&seed.name), normalize(&key));
                        for alias in &seed.aliases {
                            prop_assert_ne!(normalize(alias), normalize(&key));
                        }
                    }
                }
            }
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
