//! `ports` — port traits the cli wires adapters into.
//!
//! Ports are function-shaped contracts (nw-fp-hexagonal-architecture):
//! each trait method is a port operation; each adapter is an
//! implementation. The pure core (claim-domain, lexicon) does NOT
//! import this crate — adapters and cli do.
//!
//! Async exception: `PdsPort` carries `async fn` methods via
//! `async-trait` because network I/O is inherently async per ADR-004.
//! All other port traits are sync.
//!
//! RED-baseline scaffold (step 01-01).
//
// SCAFFOLD: true

#![allow(dead_code)]
#![forbid(unsafe_code)]

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use claim_domain::{Cid, Did, ReferenceType, SignatureBlock, SignedClaim};
use serde::{Deserialize, Serialize};

// -----------------------------------------------------------------------------
// Earned-trust probe contract (every adapter exposes one)
// -----------------------------------------------------------------------------
//
// `ProbeOutcome` + `ProbeRefusalReason` live in the dedicated `probe`
// submodule so the JSON contract (consumed by the tracing layer's
// `health.startup.refused` event) lives next to its tests.

mod probe;
pub use probe::{ProbeOutcome, ProbeRefusalReason};

// -----------------------------------------------------------------------------
// Driven ports — adapters implement these
// -----------------------------------------------------------------------------

#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("storage probe refused: {detail}")]
    ProbeRefused { detail: String },
    #[error("schema migration failed: {message}")]
    SchemaMigrationFailed { message: String },
    #[error("write failed for cid {cid:?}: {message}")]
    WriteFailed { cid: Cid, message: String },
    #[error("read failed for cid {cid:?}: {message}")]
    ReadFailed { cid: Cid, message: String },
    #[error("query failed: {message}")]
    QueryFailed { message: String },
}

pub trait StoragePort {
    fn probe(&self) -> ProbeOutcome;
    fn write_signed_claim(&self, signed: &SignedClaim) -> Result<(), StorageError>;
    fn read_signed_claim(&self, cid: &Cid) -> Result<Option<SignedClaim>, StorageError>;
    fn query_by_subject(&self, subject: &str) -> Result<Vec<SignedClaim>, StorageError>;
    fn query_referencing(
        &self,
        target_cid: &Cid,
    ) -> Result<Vec<(Cid, ReferenceType)>, StorageError>;
    fn record_publication(&self, cid: &Cid, at_uri: &str, published_at: DateTime<Utc>)
        -> Result<(), StorageError>;
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AtUri(pub String);

#[derive(Debug, thiserror::Error)]
pub enum PdsError {
    #[error("PDS probe refused: {detail}")]
    ProbeRefused { detail: String },
    #[error("PDS unreachable: {message}")]
    Unreachable { message: String },
    #[error("TLS handshake failed: {message}")]
    TlsHandshakeFailed { message: String },
    #[error("PDS rejected record: {message}")]
    RecordRejected { message: String },
    #[error("PDS idempotency violation: {message}")]
    IdempotencyViolation { message: String },
}

/// Result of one successful `create_record` call.
///
/// `at_uri` is the canonical AT URI of the record (whether freshly
/// inserted or pre-existing). `was_idempotent` distinguishes:
///
/// - `false` — this invocation actually inserted the record (HTTP 2xx
///   from `com.atproto.repo.createRecord`).
/// - `true`  — the rkey already existed on the PDS; the adapter
///   classified the 409/`RecordAlreadyExists` response as success per
///   architecture §6.2 (WS-9 idempotent-republish contract).
///
/// The `claim publish` verb branches its rendered success message on
/// this bit so users re-publishing a CID see "already published"
/// instead of the fresh-publish wording. Keeping idempotency as a
/// caller-observable bit (rather than a sentinel error) preserves
/// railway-oriented composition: the success arm carries everything the
/// caller needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateRecordOutcome {
    pub at_uri: AtUri,
    pub was_idempotent: bool,
}

#[async_trait]
pub trait PdsPort: Send + Sync {
    fn probe(&self) -> ProbeOutcome;
    async fn create_record(
        &self,
        collection: &str,
        rkey: &str,
        body: serde_json::Value,
    ) -> Result<CreateRecordOutcome, PdsError>;
    async fn get_record(
        &self,
        collection: &str,
        rkey: &str,
    ) -> Result<Option<serde_json::Value>, PdsError>;
    async fn list_records(&self, collection: &str) -> Result<Vec<serde_json::Value>, PdsError>;
}

#[derive(Debug, thiserror::Error)]
pub enum IdentityError {
    #[error("identity probe refused: {detail}")]
    ProbeRefused { detail: String },
    #[error("keychain unreachable: {message}")]
    KeychainUnreachable { message: String },
    #[error("DID document mismatch: {message}")]
    DidDocumentMismatch { message: String },
    #[error("signature operation failed: {message}")]
    SignatureFailed { message: String },
    #[error("signature verification failed")]
    VerificationFailed,
}

pub trait IdentityPort {
    fn probe(&self) -> ProbeOutcome;
    fn author_did(&self) -> &Did;
    fn sign(&self, unsigned_cid: &Cid) -> Result<SignatureBlock, IdentityError>;
    fn verify(&self, signed: &SignedClaim) -> Result<(), IdentityError>;
}

pub trait ClockPort {
    fn probe(&self) -> ProbeOutcome;
    fn now_utc(&self) -> DateTime<Utc>;
}

// -----------------------------------------------------------------------------
// Re-exports for adapter ergonomics
// -----------------------------------------------------------------------------

pub use claim_domain;
pub use lexicon;
