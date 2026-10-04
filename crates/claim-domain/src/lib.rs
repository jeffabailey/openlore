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
mod decode;
mod normalize;
mod provenance;
mod references;
mod retraction;
mod sign;
mod validate_counter_claim;
mod verify;

pub use canonicalize::canonicalize;
pub use cid::compute_cid;
pub use confidence::confidence_bucket;
// Slice-05 (ADR-026): the PURE z6Mk publicKeyMultibase decode helper + its
// value types. `verify`/`compute_cid` are UNCHANGED and reused (no second path).
pub use decode::{
    decode_claim_record, decode_ed25519_multibase, encode_ed25519_multibase, DecodeError, KeyId,
    VerificationKey,
};
pub use normalize::normalize_reason;
// ADR-071: self-attested provenance — the record ADT + the pure verdict.
pub use provenance::{
    provenance_mode, provenance_verdict, ClaimRecord, Provenance, ProvenanceMode,
    ProvenanceRejection, RecordOrigin, SelfAttestedClaim,
};
pub use references::reference_rules_validate;
// ADR-060 D-RF-D3 self-retraction rule, hoisted here so appview search and
// person inference share ONE rule (contributor-philosophy-inference DDD-7).
pub use retraction::{
    is_own_retraction_marker, is_self_retracted, is_superseded_by_author, ClaimLineage,
};
pub use sign::sign;
pub use validate_counter_claim::validate_counter_claim;
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

/// The DID a DID URL names: `<did>#<fragment>` → `<did>`; a bare DID is
/// returned as-is. The ONE normalisation rule for naming an author in an
/// at-uri — `claim publish` mints record at-uris with it and person-inference
/// provenance cites supporting claims with it (Q-CPI-D3).
pub fn bare_did(author_did: &str) -> &str {
    author_did
        .split_once('#')
        .map_or(author_did, |(did, _fragment)| did)
}

/// Numeric confidence in `[0.0, 1.0]` (validated by smart constructor),
/// held at basis-point precision (4 decimal places).
///
/// ATProto records cannot carry floats, so the wire form is an integer
/// count of basis points (`0.85` -> `8500`). Every value is rounded to that
/// grid when it enters the domain (deserialization is the one entry
/// point), so `from_basis_points(c.basis_points()) == c` exactly and a
/// claim's CID survives the publish -> pull round trip. Values that were
/// already on the grid (0.25, 0.85, ...) are unchanged, and so are their
/// CIDs.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize)]
pub struct Confidence(f64);

/// Basis points per 1.0 of confidence.
pub const CONFIDENCE_BASIS_POINTS: i64 = 10_000;

impl<'de> Deserialize<'de> for Confidence {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        f64::deserialize(deserializer).map(|value| Confidence(on_basis_point_grid(value)))
    }
}

/// Round to the nearest basis point. Idempotent.
fn on_basis_point_grid(value: f64) -> f64 {
    (value * CONFIDENCE_BASIS_POINTS as f64).round() / CONFIDENCE_BASIS_POINTS as f64
}

impl Confidence {
    /// The wire form: whole basis points (`0.85` -> `8500`).
    pub fn basis_points(&self) -> i64 {
        (self.0 * CONFIDENCE_BASIS_POINTS as f64).round() as i64
    }

    /// From the wire form. Range is not checked here (see `try_new`).
    pub fn from_basis_points(basis_points: i64) -> Self {
        Confidence(basis_points as f64 / CONFIDENCE_BASIS_POINTS as f64)
    }

    /// Read a record's `confidence` JSON value: an integer is basis points
    /// (the ATProto-safe form); a float is the legacy `[0.0, 1.0]` form,
    /// still accepted from records written before basis points.
    pub fn from_wire(value: &serde_json::Value) -> Option<Self> {
        if let Some(basis_points) = value.as_i64() {
            Some(Self::from_basis_points(basis_points))
        } else {
            value
                .as_f64()
                .map(|legacy| Confidence(on_basis_point_grid(legacy)))
        }
    }

    /// Smart constructor: returns `Err(OutOfRangeConfidence)` outside `[0.0, 1.0]`.
    pub fn try_new(_value: f64) -> Result<Self, ClaimError> {
        panic!("Not yet implemented -- RED scaffold");
    }

    /// Inner value accessor (read-only — domain remains immutable).
    pub fn value(&self) -> f64 {
        self.0
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
    /// Free-text explanation, present only on counter-claims (ADR-015 /
    /// WD-34). NFC-normalized at compose time via [`normalize_reason`].
    /// OPTIONAL at the wire level (mirrors `lexicon::Claim::reason`):
    /// `#[serde(default, skip_serializing_if = "Option::is_none")]` keeps
    /// a `reason: None` claim byte-identical to a slice-01 claim, so the
    /// CID is stable across the slice-01 → slice-03 upgrade (I-FED-7).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
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
    /// A counter-claim (a claim whose `references[]` carries a `Counters`
    /// entry) was composed without a reason — the normalized reason is
    /// `None` or empty. ADR-015 / WD-34: a counter MUST explain itself.
    #[error("a counter-claim requires a non-empty reason")]
    CounterReasonMissing,
    /// The target of a counter-claim resolves (via [`ClaimLookup`]) to a
    /// claim authored by the current user. You cannot counter your own
    /// claim — use `openlore claim retract` instead. WD-34.
    #[error(
        "cannot counter your own claim (target authored by you); \
         use `openlore claim retract` instead"
    )]
    SelfCounter,
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

#[cfg(test)]
mod confidence_wire_tests {
    use super::*;
    use proptest::prelude::*;

    fn confidence(value: f64) -> Confidence {
        serde_json::from_value(serde_json::json!(value)).expect("a number deserializes")
    }

    #[test]
    fn values_already_on_the_grid_are_unchanged() {
        for value in [0.0, 0.25, 0.5, 0.7, 0.75, 0.85, 0.42, 1.0] {
            assert_eq!(confidence(value).value(), value, "{value}");
        }
    }

    #[test]
    fn the_wire_form_is_whole_basis_points() {
        assert_eq!(confidence(0.85).basis_points(), 8500);
        assert_eq!(confidence(0.123456).basis_points(), 1235);
        assert_eq!(
            Confidence::from_wire(&serde_json::json!(8500)),
            Some(confidence(0.85))
        );
        assert_eq!(
            Confidence::from_wire(&serde_json::json!(0.85)),
            Some(confidence(0.85)),
            "a legacy float record still reads"
        );
        assert_eq!(Confidence::from_wire(&serde_json::json!("0.85")), None);
    }

    proptest! {
        /// The CID-preserving property: wire out, wire in, same value bit for bit.
        #[test]
        fn basis_points_round_trip_is_exact(value in 0.0_f64..=1.0) {
            let c = confidence(value);
            prop_assert_eq!(Confidence::from_basis_points(c.basis_points()), c);
            prop_assert_eq!(confidence(c.value()), c, "the grid is idempotent");
        }
    }
}
