//! `lexicon` — the `org.openlore.*` Lexicon schemas (federation contract).
//!
//! Holds the JSON schemas + serde-derived Rust models. Validates inbound
//! JSON against the schemas. Pure module: no I/O.
//!
//! ADR-005 (namespace), ADR-006 (CID derivation), ADR-008 (retraction
//! semantics). The Lexicon shape is what peers in slice-03 will
//! deserialize against — any drift breaks federation.
//!
//! RED-baseline scaffold (step 01-01): public items panic.
//
// SCAFFOLD: true

#![allow(dead_code)]
#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};

#[derive(Debug, thiserror::Error)]
pub enum LexiconError {
    #[error("JSON value does not match the {nsid} schema: {message}")]
    SchemaMismatch { nsid: String, message: String },
    #[error("required field `{field}` missing")]
    MissingField { field: String },
    #[error("field `{field}` outside valid range: {message}")]
    OutOfRange { field: String, message: String },
    #[error("serde round-trip not byte-equal (loadable but not stable)")]
    SerdeRoundTripFailed,
}

// =============================================================================
// org.openlore.claim
// =============================================================================

pub mod claim {
    use super::*;

    pub const NSID: &str = "org.openlore.claim";

    /// Serde-modeled mirror of the `org.openlore.claim` Lexicon record.
    /// Field names track the Lexicon JSON keys verbatim.
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    pub struct Claim {
        pub subject: String,
        pub predicate: String,
        pub object: String,
        pub evidence: Vec<String>,
        pub confidence: f64,
        #[serde(rename = "authorDid")]
        pub author_did: String,
        #[serde(rename = "composedAt")]
        pub composed_at: String,
        #[serde(default)]
        pub references: Vec<ClaimReference>,
        #[serde(default)]
        pub signature: Option<SignatureBlock>,
    }

    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    pub struct ClaimReference {
        #[serde(rename = "type")]
        pub ref_type: String,
        pub cid: String,
    }

    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    pub struct SignatureBlock {
        #[serde(rename = "signedCid")]
        pub signed_cid: String,
        #[serde(rename = "signatureBytes")]
        pub signature_bytes: String, // base64
        #[serde(rename = "verificationMethod")]
        pub verification_method: String,
    }
}

// =============================================================================
// org.openlore.philosophy
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

// =============================================================================
// Validators
// =============================================================================

/// Validate a JSON value against `org.openlore.claim`. Returns the
/// parsed `Claim` or a `LexiconError` naming the violation.
pub fn validate_claim_json(_value: &serde_json::Value) -> Result<claim::Claim, LexiconError> {
    panic!("Not yet implemented -- RED scaffold");
}

/// Validate a JSON value against `org.openlore.philosophy`.
pub fn validate_philosophy_json(
    _value: &serde_json::Value,
) -> Result<philosophy::Philosophy, LexiconError> {
    panic!("Not yet implemented -- RED scaffold");
}
