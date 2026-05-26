//! Canonical claim fixtures shared across acceptance + integration tests.
//!
//! Functional-paradigm note (ADR-007): every fixture is a free function
//! returning a fresh, immutable value. No setup hooks; no shared mutable
//! state. Tests compose by passing the returned value through pipelines.
//!
//! Step 02-06 (LC-1 roundtrip) introduces the first two fixtures here:
//!   * `fixture_jeff_rust_memory_safety()` — the canonical UnsignedClaim
//!     from US-001 Example 1 and data-models.md §"On-disk artifact format".
//!   * `fixture_jeff_rust_memory_safety_signed()` — the SignedClaim wrapping
//!     the above with a deterministic placeholder signature block. The
//!     signature bytes are NOT cryptographically valid; the real `sign`
//!     primitive lands in phase 03. LC-1 only asserts roundtrip equality,
//!     which is signature-content-agnostic.

use claim_domain::{
    Cid, ClaimReference, Confidence, Did, ReferenceType, SignatureBlock, SignedClaim,
    UnsignedClaim,
};

// -----------------------------------------------------------------------------
// Internal helpers
// -----------------------------------------------------------------------------

/// Construct a `Confidence` value without calling the RED-scaffolded
/// smart constructor. Goes through serde because `Confidence` is a tuple
/// struct around `f64` with a private inner field — the derived
/// `Deserialize` impl maps a JSON number straight into the wrapper.
///
/// Panics if the caller passes a value serde-json refuses (e.g. NaN);
/// fixtures only ever pass constants in `[0.0, 1.0]`, so this is fine.
fn confidence(value: f64) -> Confidence {
    serde_json::from_value(serde_json::json!(value))
        .expect("fixture confidence values are well-formed finite numbers")
}

// -----------------------------------------------------------------------------
// fixture_jeff_rust_memory_safety — US-001 Example 1
// -----------------------------------------------------------------------------

/// The canonical Jeff-on-Rust unsigned claim from US-001 Example 1.
///
/// Matches the on-disk JSON example in
/// `docs/feature/openlore-foundation/design/data-models.md`
/// §"On-disk artifact format", minus the signature block (this is the
/// pre-sign shape).
pub fn fixture_jeff_rust_memory_safety() -> UnsignedClaim {
    UnsignedClaim {
        subject: "github:rust-lang/rust".to_string(),
        predicate: "embodiesPhilosophy".to_string(),
        object: "org.openlore.philosophy.memory-safety".to_string(),
        evidence: vec!["https://www.rust-lang.org/".to_string()],
        confidence: confidence(0.86),
        author_did: Did("did:plc:jeff#org.openlore.application".to_string()),
        composed_at: "2026-05-25T12:00:00Z".to_string(),
        references: Vec::<ClaimReference>::new(),
    }
}

/// The Jeff-on-Rust claim with a deterministic placeholder signature
/// block. NOT cryptographically valid — used only by tests that need a
/// `SignedClaim` shape (LC-1 roundtrip equality, etc.). The real
/// signing primitive is exercised in phase 03 once `claim_domain::sign`
/// is GREEN.
pub fn fixture_jeff_rust_memory_safety_signed() -> SignedClaim {
    let unsigned = fixture_jeff_rust_memory_safety();
    SignedClaim {
        unsigned,
        signature: SignatureBlock {
            signed_cid: Cid(
                "bafyreigh2akiscaildc7tge3pp7tezt5ihtdmf6wzgnrjexampletest".to_string(),
            ),
            // Deterministic 64-byte filler pattern; would be a real Ed25519
            // signature in production. Roundtrip equality does not depend
            // on signature validity, only on byte-stability.
            signature_bytes: (0u8..64).collect(),
            verification_method: "did:plc:jeff#org.openlore.application".to_string(),
        },
    }
}

// -----------------------------------------------------------------------------
// Convenience: typed-reference helper used by later step fixtures
// -----------------------------------------------------------------------------

/// Build a `ClaimReference` from a ref-type and CID string. Hand-rolled
/// helper because `ClaimReference` is a public POD struct but keeps the
/// fixture builder bodies concise.
pub fn reference(ref_type: ReferenceType, cid: impl Into<String>) -> ClaimReference {
    ClaimReference {
        ref_type,
        cid: Cid(cid.into()),
    }
}
