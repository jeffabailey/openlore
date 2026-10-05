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

// -----------------------------------------------------------------------------
// IdentityLookupPort — read-only identity resolution (bluesky-claim-review-app)
// -----------------------------------------------------------------------------

/// A handle resolved to its account: the DID, the handle the DID document
/// confirms, and the PDS that hosts the repo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedIdentity {
    pub did: String,
    pub verified_handle: String,
    pub pds_endpoint: String,
}

/// Why a handle could not be resolved.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum IdentityLookupError {
    /// No account answers to the handle (or its DID document disowns it).
    #[error("no account answers to that handle")]
    NotFound,
    /// The resolver or the DID directory could not be reached.
    #[error("identity resolution is unavailable: {detail}")]
    Unavailable { detail: String },
}

/// The read-only extension of identity resolution the review app's sign-in
/// needs (component-boundaries §2): handle → `{did, verified_handle,
/// pds_endpoint}`. Like [`IdentityResolvePort`] it has no signing or write
/// capability.
#[async_trait]
pub trait IdentityLookupPort: Send + Sync {
    async fn resolve_identity(&self, handle: &str)
        -> Result<ResolvedIdentity, IdentityLookupError>;

    /// DID → its DID document's PDS and handle. The handle is the
    /// document's `at://` alias only when that handle resolves back to the
    /// DID; otherwise `verified_handle` is the DID itself.
    async fn resolve_did(&self, did: &str) -> Result<ResolvedIdentity, IdentityLookupError>;

    /// DID → the PDS its document names (`#atproto_pds`), read afresh. The
    /// indexer needs only the PDS, so an implementation may skip the handle
    /// round-trip [`Self::resolve_did`] makes.
    async fn resolve_pds(&self, did: &str) -> Result<String, IdentityLookupError> {
        self.resolve_did(did)
            .await
            .map(|identity| identity.pds_endpoint)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    /// A lookup that knows only `resolve_did`, answering one fixed result.
    struct DidOnlyLookup(Result<ResolvedIdentity, IdentityLookupError>);

    #[async_trait]
    impl IdentityLookupPort for DidOnlyLookup {
        async fn resolve_identity(
            &self,
            _handle: &str,
        ) -> Result<ResolvedIdentity, IdentityLookupError> {
            unreachable!("resolve_pds never resolves a handle")
        }

        async fn resolve_did(&self, _did: &str) -> Result<ResolvedIdentity, IdentityLookupError> {
            self.0.clone()
        }
    }

    /// Drive a future that never waits (the fake answers immediately).
    fn ready<T>(future: impl std::future::Future<Output = T>) -> T {
        let mut future = std::pin::pin!(future);
        let mut cx = std::task::Context::from_waker(std::task::Waker::noop());
        match future.as_mut().poll(&mut cx) {
            std::task::Poll::Ready(value) => value,
            std::task::Poll::Pending => unreachable!("the fake lookup never waits"),
        }
    }

    proptest! {
        /// The default `resolve_pds` is exactly `resolve_did`'s PDS, and its
        /// failure unchanged.
        #[test]
        fn the_default_pds_is_the_resolved_identitys_pds(
            pds in "https://[a-z]{1,10}\\.[a-z]{2,4}", found in any::<bool>()
        ) {
            let answer = if found {
                Ok(ResolvedIdentity {
                    did: "did:plc:abc".to_string(),
                    verified_handle: "did:plc:abc".to_string(),
                    pds_endpoint: pds.clone(),
                })
            } else {
                Err(IdentityLookupError::NotFound)
            };
            let expected = answer.clone().map(|identity| identity.pds_endpoint);
            prop_assert_eq!(ready(DidOnlyLookup(answer).resolve_pds("did:plc:abc")), expected);
        }
    }
}
