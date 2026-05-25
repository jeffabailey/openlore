//! `adapter-atproto-did` — `IdentityPort` over ATProto DID + OS keychain.
//!
//! Holds the per-app derived Ed25519 keypair (ADR-002). Exposes
//! `sign()`/`verify()`. Resolves the user's DID document for
//! verification-method discovery. Probe verifies DID-document
//! resolvability + keychain accessibility + WSL2 fallback key file
//! perms = `0600`.
//!
//! RED-baseline scaffold (step 01-01).
//
// SCAFFOLD: true

#![allow(dead_code)]
#![forbid(unsafe_code)]

use claim_domain::{Cid, Did, SignatureBlock, SignedClaim};
use ports::{IdentityError, IdentityPort, ProbeOutcome};

pub struct AtProtoDidAdapter {
    did: Did,
}

impl AtProtoDidAdapter {
    /// Initialize from a resolved handle (e.g. `jeff.test` → did:plc:test-jeff).
    /// Reads / creates the per-app Ed25519 key in the OS keychain.
    pub fn for_handle(_handle: &str, _app_password: &str) -> Result<Self, IdentityError> {
        panic!("Not yet implemented -- RED scaffold");
    }
}

impl IdentityPort for AtProtoDidAdapter {
    fn probe(&self) -> ProbeOutcome {
        panic!("Not yet implemented -- RED scaffold");
    }

    fn author_did(&self) -> &Did {
        &self.did
    }

    fn sign(&self, _unsigned_cid: &Cid) -> Result<SignatureBlock, IdentityError> {
        panic!("Not yet implemented -- RED scaffold");
    }

    fn verify(&self, _signed: &SignedClaim) -> Result<(), IdentityError> {
        panic!("Not yet implemented -- RED scaffold");
    }
}
