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
pub const CLAIM_LEXICON_JSON: &str =
    include_str!("../../../lexicons/org/openlore/claim.json");

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
