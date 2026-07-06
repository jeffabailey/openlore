//! `lexicon` — the `org.openlore.*` Lexicon schemas (federation contract).
//!
//! Holds the JSON schemas + serde-derived Rust models. Validates inbound
//! JSON against the schemas. Pure module: no I/O.
//!
//! ADR-005 (namespace), ADR-006 (CID derivation), ADR-008 (retraction
//! semantics). The Lexicon shape is what peers in slice-03 will
//! deserialize against — any drift breaks federation.
//!
//! Step 02-01: `claim::validate_claim_json` is now real (LC-2 GREEN).
//! Other validators remain RED scaffolds; later steps turn them GREEN.
//
// SCAFFOLD: true

#![allow(dead_code)]
#![forbid(unsafe_code)]

// =============================================================================
// Embedded Lexicon JSON resources (step 01-02)
// =============================================================================
//
// The two `org.openlore.*` Lexicon schemas are authored verbatim per
// `docs/feature/openlore-foundation/design/data-models.md` and embedded at
// compile time. Validation logic (step 02-01) consumes these constants.

/// The `org.openlore.claim` Lexicon JSON schema (embedded at compile time).
pub const CLAIM_LEXICON_JSON: &str = include_str!("../../../lexicons/org/openlore/claim.json");

/// The `org.openlore.philosophy` Lexicon JSON schema (embedded at compile time).
pub const PHILOSOPHY_LEXICON_JSON: &str =
    include_str!("../../../lexicons/org/openlore/philosophy.json");

/// NSID for the `org.openlore.claim` Lexicon.
pub const CLAIM_NSID: &str = "org.openlore.claim";

/// NSID for the `org.openlore.philosophy` Lexicon.
pub const PHILOSOPHY_NSID: &str = "org.openlore.philosophy";

// =============================================================================
// org.openlore.claim — real implementation in `claim.rs` (step 02-01)
// =============================================================================

pub mod claim;

/// Re-export of the claim validator for ergonomic call sites.
pub use claim::{validate_claim_json, Claim, ClaimReference, LexiconError, SignatureBlock};

// Step 02-06: explicit serde helpers + Eq-friendly confidence wrapper
// for the lexicon wire shape. Consolidates the federation-contract
// roundtrip path that LC-1 exercises.
pub mod serde_impls;
pub use serde_impls::{claim_from_json, claim_to_canonical_json, ConfidenceField};

// Step 01-07: lexicon module-level startup probe extended for the
// slice-03 `reason`-field federation-contract invariants (ADR-015 /
// WD-32 / I-FED-7). Pure: no I/O.
pub mod probe;
pub use probe::{probe, ProbeError};

// =============================================================================
// org.openlore.appview.searchClaims — the slice-05 READ query lexicon (ADR-027)
// =============================================================================
//
// Step 01-04: the XRPC query lexicon + the shared CLI↔indexer request/response
// DTOs (per-result `author_did` ALWAYS present; anti-merging across the
// transport, I-AV-2). A `query` (READ) type — no signed payload, no CID concern.
// Also recognizes the `[appview] indexer_url` config key + the indexer's own
// config shape (pure serde; the binaries do the I/O). Pure module: no I/O.

pub mod appview_query;
pub use appview_query::{
    AppviewConfig, ClaimReferenceDto, IndexerConfig, IndexerSources, SearchDimensionDto,
    SearchQueryRequest, SearchQueryResponse, SearchResultDto, SEARCH_CLAIMS_NSID,
};

// =============================================================================
// org.openlore.philosophy — real implementation in `philosophy.rs` (slice-01)
// =============================================================================
//
// ADR-059: the pure vocabulary core. `Philosophy` mirrors the shipped
// Lexicon (`required: [name, description]`, optional `aliases`/`seeAlso`);
// `validate_philosophy_json` mirrors `validate_claim_json` (per-field gates
// before serde, reusing `LexiconError`); `object_id`/`normalize` derive the
// deterministic claim<->vocabulary join key; `seeds()` returns the embedded
// slice-01 seed vocabulary (compile-time `include_str!`).

pub mod philosophy;

/// Re-export of the philosophy validator + derivation helpers for ergonomic
/// call sites (`lexicon::validate_philosophy_json`, `lexicon::Philosophy`).
pub use philosophy::{normalize, object_id, seeds, validate_philosophy_json, Philosophy};

// =============================================================================
// In-crate unit tests — `validate_philosophy_json` accept + reject arms
// =============================================================================
//
// Slice-22 (US-PV-001 / AC-003.4 / DoD item 2): completes the
// `validate_philosophy_json` RED scaffold's TEST surface, mirroring the
// `claim.rs` per-field-gated validator tests. Layer 2 (pure core, no I/O) per
// nw-tdd-methodology Layered Test Discipline — example-pinned (the accept
// example + the named-field reject arm; the full-shape PBT lattice belongs in
// DELIVER's unit layer, not here).
//
// RED TODAY: `validate_philosophy_json` panics ("Not yet implemented -- RED
// scaffold"), so both tests fail via that panic BEFORE reaching their
// assertions — MISSING_FUNCTIONALITY, never a compile/import error. They pin
// the behavior ADR-059 D2 mandates (mirror `validate_claim_json`: required-field
// presence → `LexiconError::MissingField` naming the offending key). Both
// assertions reference only the EXISTING scaffold signature (`Result<Philosophy,
// LexiconError>`) and the `description` field the Lexicon schema freezes, so
// they compile against the current struct AND the ADR-059-reconciled struct.

#[cfg(test)]
mod philosophy_validator_tests {
    use super::*;
    use serde_json::json;

    /// A well-formed `org.openlore.philosophy` record per the shipped Lexicon
    /// schema (`required: [name, description]`, optional `aliases`, `seeAlso`).
    fn well_formed_philosophy_value() -> serde_json::Value {
        json!({
            "name": "memory-safety",
            "description": "Programs cannot corrupt memory: no use-after-free, no buffer overrun.",
            "aliases": ["mem-safety", "memory-safe"],
            "seeAlso": ["https://en.wikipedia.org/wiki/Memory_safety"]
        })
    }

    #[test]
    fn validates_well_formed_philosophy_record() {
        // ACCEPT arm: a record carrying name + description (+ optional aliases /
        // seeAlso) validates and round-trips its description verbatim.
        let value = well_formed_philosophy_value();
        let philosophy =
            validate_philosophy_json(&value).expect("a well-formed philosophy must validate");
        assert_eq!(
            philosophy.description,
            "Programs cannot corrupt memory: no use-after-free, no buffer overrun.",
            "the validated record must round-trip its description verbatim"
        );
    }

    #[test]
    fn rejects_missing_description_with_named_field_error() {
        // REJECT arm (AC-003.4): a record MISSING the required `description`
        // rejects with a NAMED-field error (no panic — completes the scaffold),
        // reusing `LexiconError::MissingField` per ADR-059 D2 (no parallel error
        // type).
        let value = json!({ "name": "memory-safety" });
        let err = validate_philosophy_json(&value)
            .expect_err("a philosophy record missing `description` must reject");
        assert_eq!(
            err,
            LexiconError::MissingField {
                field: "description".to_string()
            },
            "the reject arm must name the missing `description` field (not an opaque serde string)"
        );
    }
}

// =============================================================================
// In-crate unit tests — embedded seeds + `normalize`/`object_id` invariants
// =============================================================================
//
// Layer 2 (pure core, no I/O) per nw-tdd-methodology Layered Test Discipline.
//
// `normalize`/`object_id` are pure + total; the natural test shape is a
// property (idempotence, output-charset, NSID-prefix). This crate is PURE by
// contract (serde + thiserror only — see Cargo.toml) and step 01-01's
// files_to_modify does NOT include Cargo.toml, so a proptest dev-dependency
// cannot be added here. Per the ADR-059 directive ("prefer proptest where
// PRACTICAL; example-based is a documented FALLBACK"), these use hand-rolled
// property loops over a curated input corpus — the fallback that stays within
// the pure-crate dependency envelope while still asserting the invariant over
// many inputs rather than a single pinned example.

#[cfg(test)]
mod seeds_tests {
    use super::*;
    use std::collections::HashSet;

    /// The six names ADR-059 hard-pins into the slice-01 seed vocabulary.
    const HARD_PINNED_NAMES: &[&str] = &[
        "memory-safety",
        "type-safety",
        "test-driven",
        "documentation-first",
        "dependency-pinning",
        "semantic-versioning",
    ];

    #[test]
    fn every_seed_validates_through_the_validator() {
        // Each embedded seed record must pass the SAME per-field-gated
        // validator that guards inbound federation JSON.
        let value: serde_json::Value = serde_json::from_str(philosophy::PHILOSOPHY_SEEDS_JSON)
            .expect("seeds.json must be valid JSON");
        let records = value.as_array().expect("seeds.json must be a JSON array");
        for record in records {
            validate_philosophy_json(record)
                .expect("every embedded philosophy seed must validate");
        }
    }

    #[test]
    fn ships_at_least_ten_seeds() {
        assert!(
            seeds().len() >= 10,
            "slice-01 must ship >=10 philosophy seeds, found {}",
            seeds().len()
        );
    }

    #[test]
    fn seed_names_have_distinct_object_ids() {
        // No two seed names may collide under `normalize` — each seed must
        // occupy a distinct object-id slot in the vocabulary namespace.
        let mut ids = HashSet::new();
        for seed in seeds() {
            let id = object_id(&seed.name);
            assert!(
                ids.insert(id.clone()),
                "seed object id collision under normalize: {id} (name {})",
                seed.name
            );
        }
    }

    #[test]
    fn hard_pinned_names_are_all_present() {
        // ADR-059 freezes these six names into the slice-01 vocabulary.
        let names: HashSet<String> = seeds().into_iter().map(|s| s.name).collect();
        for pinned in HARD_PINNED_NAMES {
            assert!(
                names.contains(*pinned),
                "hard-pinned seed name `{pinned}` missing from the vocabulary"
            );
        }
    }

    /// A corpus spanning the transformations `normalize` must handle:
    /// already-kebab, spaces, underscores, mixed case, punctuation, and
    /// leading/trailing/duplicate separators.
    fn normalize_corpus() -> Vec<&'static str> {
        vec![
            "memory-safety",
            "Memory Safety",
            "memory_safety",
            "  Memory   Safety  ",
            "Type-Safety!",
            "test.driven",
            "SEMANTIC__VERSIONING",
            "--leading-and-trailing--",
            "reproducible builds",
            "local-first",
            "C++ style",
            "already-kebab-case",
        ]
    }

    #[test]
    fn normalize_is_idempotent_over_corpus() {
        // Property: normalize(normalize(x)) == normalize(x).
        for input in normalize_corpus() {
            let once = normalize(input);
            let twice = normalize(&once);
            assert_eq!(twice, once, "normalize must be idempotent for {input:?}");
        }
    }

    #[test]
    fn normalize_output_charset_is_kebab_only() {
        // Property: output uses only [a-z0-9-], with no leading/trailing '-'
        // and no doubled '-'.
        for input in normalize_corpus() {
            let out = normalize(input);
            assert!(
                out.chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'),
                "normalize({input:?}) = {out:?} escaped the [a-z0-9-] charset"
            );
            assert!(
                !out.starts_with('-') && !out.ends_with('-'),
                "normalize({input:?}) = {out:?} has a boundary dash"
            );
            assert!(
                !out.contains("--"),
                "normalize({input:?}) = {out:?} has a doubled dash"
            );
        }
    }

    #[test]
    fn normalize_maps_separators_and_punctuation_to_exact_kebab() {
        // Modeling property (reference mapping): the STRUCTURAL invariants
        // (charset / idempotence / prefix) pin the *shape* of normalize's
        // output but not its *value*, so a mutant that mis-classifies which
        // characters are separators (e.g. dropping '_' / whitespace instead of
        // mapping them to '-', or treating punctuation as a separator instead
        // of dropping it) still yields kebab-shaped output and survives. This
        // pins the exact value mapping: whitespace and '_' collapse to a single
        // '-'; any other punctuation is dropped WITHOUT emitting a separator.
        let cases: &[(&str, &str)] = &[
            ("Memory Safety", "memory-safety"),   // whitespace -> '-'
            ("memory_safety", "memory-safety"),   // underscore -> '-'
            ("  Memory   Safety  ", "memory-safety"), // runs collapse + trim
            ("test.driven", "testdriven"),        // punctuation dropped, no '-'
            ("C++ style", "c-style"),             // '+' dropped, space -> '-'
        ];
        for (input, expected) in cases {
            assert_eq!(
                normalize(input),
                *expected,
                "normalize({input:?}) must map to the exact kebab value {expected:?}"
            );
        }
    }

    #[test]
    fn object_id_always_carries_the_nsid_prefix() {
        // Property: every derived id is `org.openlore.philosophy.<segment>`,
        // and the join key matches the slice-01 claim `object` literal for
        // the hard-pinned `memory-safety` seed exactly.
        for input in normalize_corpus() {
            let id = object_id(input);
            assert!(
                id.starts_with("org.openlore.philosophy."),
                "object_id({input:?}) = {id:?} lost the NSID prefix"
            );
        }
        assert_eq!(
            object_id("memory-safety"),
            "org.openlore.philosophy.memory-safety",
            "object_id must be byte-identical to the slice-01 claim `object` literal"
        );
    }
}
