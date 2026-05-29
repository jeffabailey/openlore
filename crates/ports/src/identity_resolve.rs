//! `identity_resolve` — the shared verify-only identity-resolution port
//! (ADR-026) + its railway error. ASYNC (network: PLC DID-document resolution).
//!
//! `IdentityResolvePort` resolves an author's DID into the Ed25519
//! `VerificationKey` the PURE `claim_domain::verify` consumes (decoded from the
//! PLC DID-doc `z6Mk...` `publicKeyMultibase` via `claim_domain::decode_ed25519_multibase`).
//! READ/VERIFY-ONLY by construction (I-AV-5): there is intentionally NO sign /
//! publish / put_record method on this trait. The indexer wires ONLY this
//! resolve-only variant (it cannot sign — the capability boundary, ADR-023);
//! the CLI's signing `IdentityPort` is a separate trait. The absence of any
//! signing method is the type-level half of the boundary.
//
// SCAFFOLD: true  (trait surface only; the adapter impl lands in step 01-03/04)

use async_trait::async_trait;
use claim_domain::{Did, VerificationKey};

use crate::ProbeOutcome;

// -----------------------------------------------------------------------------
// ResolveError — the railway-oriented failure surface
// -----------------------------------------------------------------------------

/// Why a DID → verification-key resolution failed. The resolver consults the
/// PLC directory / `did:web` endpoint (network) then decodes the
/// `publicKeyMultibase` via the PURE `claim_domain::decode_ed25519_multibase`
/// (ADR-026).
///
/// (`detail` is a pre-formatted String rather than a wrapped
/// `std::error::Error` source, so this pure-core port stays free of the
/// adapter's transport error types — mirrors `IdentityError::PeerResolutionFailed`.)
#[derive(Debug, thiserror::Error)]
pub enum ResolveError {
    #[error("identity-resolve probe refused: {detail}")]
    ProbeRefused { detail: String },
    /// The PLC directory / `did:web` endpoint is unreachable, the DID does not
    /// exist, or the returned DID document failed schema validation.
    #[error("DID resolution failed for {did:?}: {detail}")]
    ResolutionFailed { did: Did, detail: String },
    /// The DID document resolved but its `publicKeyMultibase` could not be
    /// decoded into an Ed25519 verification key (the ADR-026 decode failed).
    #[error("pubkey decode failed for {did:?}: {detail}")]
    PubkeyDecodeFailed { did: Did, detail: String },
}

// -----------------------------------------------------------------------------
// IdentityResolvePort — verify-only key resolution (ASYNC; I-AV-5 / ADR-026)
// -----------------------------------------------------------------------------

/// The shared verify-only identity-resolution port (ADR-026). ASYNC (network:
/// PLC DID-document resolution) so `#[async_trait]` is permitted exactly as for
/// `PdsPort`/`GithubPort` (ADR-004).
///
/// READ/VERIFY-ONLY by construction (I-AV-5): there is intentionally NO
/// `sign`/`publish`/`put_record` method. The indexer is signing-incapable; the
/// capability boundary is encoded as the ABSENCE of those methods.
#[async_trait]
pub trait IdentityResolvePort: Send + Sync {
    /// Earned-Trust probe — see ADR-009 + `probe.rs`. The adapter impl resolves
    /// a FIXTURE DID document with a real `z6Mk...` value, runs the REAL decode,
    /// and asserts the key VERIFIES a known-good signature AND REJECTS a
    /// tampered one (a seam-only pass is a CI failure). REQUIRED per I-4.
    fn probe(&self) -> ProbeOutcome;

    /// Resolve `did` into the Ed25519 [`VerificationKey`] the pure `verify`
    /// consumes (decoded from the PLC DID-doc `z6Mk...`, ADR-026). Read-only;
    /// no signing capability is implied or exposed.
    async fn resolve_verification_key(&self, did: &Did) -> Result<VerificationKey, ResolveError>;
}
