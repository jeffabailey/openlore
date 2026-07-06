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

use serde::{Deserialize, Serialize};

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
// org.openlore.philosophy — RED scaffold (later step turns this GREEN)
// =============================================================================

pub mod philosophy {
    use super::*;

    pub const NSID: &str = "org.openlore.philosophy";

    /// Serde-modeled mirror of the `org.openlore.philosophy` Lexicon record.
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    pub struct Philosophy {
        pub id: String,
        pub label: String,
        pub description: String,
    }
}

/// Validate a JSON value against `org.openlore.philosophy`.
/// RED scaffold — a later step in slice-01 turns this GREEN.
pub fn validate_philosophy_json(
    _value: &serde_json::Value,
) -> Result<philosophy::Philosophy, LexiconError> {
    panic!("Not yet implemented -- RED scaffold");
}

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
