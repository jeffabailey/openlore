//! `claim-domain` — the pure claim model.
//!
//! Defines the unsigned + signed claim ADTs and the pure transformations
//! `canonicalize → compute_cid → sign → verify → reference_rules_validate
//! → confidence_bucket`. NO I/O. NO async. NO adapters.
//!
//! Hexagonal pure core (ADR-009 + ADR-007). The composition root
//! (`crates/cli`) wires this into the effect shell.
//!
//! RED-baseline scaffold (step 01-01): every public item panics with
//! `panic!("Not yet implemented -- RED scaffold")`. DELIVER fills bodies
//! one acceptance scenario at a time.
//
// SCAFFOLD: true

#![allow(dead_code)] // scaffolds; usage lands in subsequent DELIVER steps
#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};

// -----------------------------------------------------------------------------
// Submodules (step 02-03: canonical CBOR + CID computation;
//             step 03-01: Ed25519 sign + verify primitives)
// -----------------------------------------------------------------------------
mod canonicalize;
mod cid;
mod confidence;
mod references;
mod sign;
mod verify;

pub use canonicalize::canonicalize;
pub use cid::compute_cid;
pub use confidence::confidence_bucket;
pub use references::reference_rules_validate;
pub use sign::sign;
pub use verify::verify;

// Step 02-04: proptest strategies for the one @property scenario in
// slice-01 (LC-3). `pub` so test-support and acceptance tests can
// reach `arb_unsigned_claim` directly. proptest is a regular dep of
// this crate (see Cargo.toml comment); a later cleanup may
// feature-gate it.
pub mod proptest_strategies;

// -----------------------------------------------------------------------------
// Domain wrappers (per nw-fp-domain-modeling §2 — never use primitives directly)
// -----------------------------------------------------------------------------

/// A claim's content-derived identifier (CIDv1 dag-cbor sha2-256 base32-lower,
/// per ADR-006). Wraps the upstream `cid::Cid` so the domain owns the type.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Cid(pub String);

/// A decentralized identifier (ATProto DID per ADR-002). Always carries the
/// fragment selecting the OpenLore application verification method.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Did(pub String);

/// Numeric confidence in `[0.0, 1.0]` (validated by smart constructor).
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct Confidence(f64);

impl Confidence {
    /// Smart constructor: returns `Err(OutOfRangeConfidence)` outside `[0.0, 1.0]`.
    pub fn try_new(_value: f64) -> Result<Self, ClaimError> {
        panic!("Not yet implemented -- RED scaffold");
    }

    /// Inner value accessor (read-only — domain remains immutable).
    pub fn value(&self) -> f64 {
        panic!("Not yet implemented -- RED scaffold");
    }
}

/// One typed reference from this claim to another (ADR-008 §Lexicon design).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClaimReference {
    pub ref_type: ReferenceType,
    pub cid: Cid,
}

/// Kind of inter-claim relationship (ADR-008).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReferenceType {
    Retracts,
    Corrects,
    Counters,
    Supersedes,
}

/// Display-only bucket label for confidence; NEVER persisted (WD-10 / D-12).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfidenceBucket {
    Speculative,
    Weighted,
    WellEvidenced,
    Triangulated,
}

// -----------------------------------------------------------------------------
// Core claim types
// -----------------------------------------------------------------------------

/// An UNSIGNED claim — everything the author composed before signing.
/// Serializes to canonical CBOR via `canonicalize`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UnsignedClaim {
    pub subject: String,
    pub predicate: String,
    pub object: String,
    pub evidence: Vec<String>,
    pub confidence: Confidence,
    pub author_did: Did,
    /// RFC3339 UTC. Pinned via test env var for determinism in tests.
    pub composed_at: String,
    pub references: Vec<ClaimReference>,
}

/// The signature block attached during `sign`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignatureBlock {
    pub signed_cid: Cid,
    pub signature_bytes: Vec<u8>,
    pub verification_method: String,
}

/// A SIGNED claim — unsigned + signature. Ready for storage + publish.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SignedClaim {
    pub unsigned: UnsignedClaim,
    pub signature: SignatureBlock,
}

// -----------------------------------------------------------------------------
// Error type — railway-oriented per nw-fp-domain-modeling §8
// -----------------------------------------------------------------------------

#[derive(Debug, thiserror::Error)]
pub enum ClaimError {
    #[error("confidence {value} is outside [0.0, 1.0]")]
    OutOfRangeConfidence { value: f64 },
    #[error("claim references its own CID (self-reference rejected)")]
    SelfReference,
    #[error("reference cycle detected at CID {cid:?}")]
    CycleDetected { cid: Cid },
    #[error("canonicalization failed: {message}")]
    CanonicalizationFailed { message: String },
    #[error("invalid Lexicon shape: {message}")]
    InvalidLexiconShape { message: String },
    #[error("signature operation failed: {message}")]
    SignatureFailed { message: String },
    #[error("signature verification failed")]
    VerificationFailed,
}

// -----------------------------------------------------------------------------
// Ports the pure core needs FROM adapters (kept here, NOT in crates/ports,
// because claim-domain is the consumer and the trait is pure-shaped)
// -----------------------------------------------------------------------------

/// A pure-shaped lookup the storage adapter can satisfy. Unit tests pass
/// `None`; integration tests pass a small in-memory implementation.
pub trait ClaimLookup {
    fn signed_by_cid(&self, cid: &Cid) -> Option<SignedClaim>;
}

// -----------------------------------------------------------------------------
// Pure pipeline functions
// -----------------------------------------------------------------------------
//
// `canonicalize` and `compute_cid` were promoted to dedicated submodules
// (`mod canonicalize`, `mod cid`) at step 02-03; their `pub use`
// re-exports above preserve the `claim_domain::canonicalize` /
// `claim_domain::compute_cid` import paths the rest of the workspace
// uses.

/// Newtype over the raw signing key bytes. The adapter holds the real key
/// material; this wrapper is what `sign` consumes so the pure core stays
/// key-format-agnostic.
#[derive(Debug, Clone)]
pub struct SigningKey(pub Vec<u8>);

/// Newtype over the public-key bytes used by `verify`.
#[derive(Debug, Clone)]
pub struct VerifyingKey(pub Vec<u8>);

// `sign` and `verify` were promoted to dedicated submodules
// (`mod sign`, `mod verify`) at step 03-01. The `pub use` re-exports
// above preserve `claim_domain::sign` / `claim_domain::verify` import
// paths for the rest of the workspace (`ports::SigningPort`, adapter
// composition, acceptance tests).

// `reference_rules_validate` was promoted to a dedicated submodule
// (`mod references`) at step 03-03. The `pub use` re-export above
// preserves `claim_domain::reference_rules_validate` as the import
// path for the sign pipeline and acceptance tests. Step 03-04
// extends this module with two-hop cycle detection via the
// `ClaimLookup` trait.

// `confidence_bucket` was promoted to a dedicated submodule
// (`mod confidence`) at step 03-02. The `pub use` re-export above
// preserves `claim_domain::confidence_bucket` as the import path for
// the cli render layer (WS-3 / WS-5 in phase 05). Keeping the function
// in its own module enforces WD-10: the lexicon crate MUST NOT depend
// on `confidence_bucket` (architectural rule checked by
// `cargo xtask check-arch` in phase 06).
