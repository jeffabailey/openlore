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
use url::Url;

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
// Slice-03 (federated read) — peer storage port + cross-store row type
// -----------------------------------------------------------------------------
//
// `federated_row` declares the cross-store row type returned by
// `StoragePort::query_federated_by_subject` + the supporting peer
// identity / record types. The non-Option `author_did` field is the
// layer-1 anti-merging defense per WD-30.
//
// `peer_storage` declares the new `PeerStoragePort` trait (sync,
// local-DB only) plus its outcomes + `PeerStorageError`.

mod federated_row;
mod peer_storage;

pub use federated_row::{
    AuthorRelationship, FederatedRow, PeerInfo, PeerRecordPage, PeerSubscription, SignedRecord,
    SourceTable, VerificationMethod,
};
pub use peer_storage::{
    AddSubscriptionOutcome, HardPurgeOutcome, PeerStorageError, PeerStoragePort,
    SoftRemoveOutcome, WritePeerClaimOutcome,
};

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

    // -------- slice-03 (federated read) --------
    /// Federated subject query: returns every row across BOTH the
    /// author table (`claims`) and the peer table (`peer_claims`)
    /// matching `subject`, each carrying its `author_did` attribution.
    ///
    /// Per WD-30 (layered anti-merging), the implementation MUST use
    /// SQL `UNION ALL` with explicit `author_did` projection — NOT a
    /// `JOIN` that could elide the column. `xtask check-arch`
    /// enforces this structurally.
    fn query_federated_by_subject(
        &self,
        subject: &str,
    ) -> Result<Vec<FederatedRow>, StorageError>;
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

    // -------- slice-03 (federated read) --------
    /// The requested peer record does not exist on the peer's PDS
    /// (HTTP 404 from `com.atproto.repo.getRecord`).
    #[error("peer record not found: collection={collection} rkey={rkey}")]
    PeerRecordNotFound { collection: String, rkey: String },
    /// The fetched record could not be parsed against the
    /// `org.openlore.claim` lexicon. Wraps the underlying
    /// lexicon/serde error verbatim for diagnostics.
    #[error("peer record schema invalid: {detail}")]
    PeerRecordSchemaInvalid { detail: String },
    /// CID round-trip check failed: the record fetched from the peer's
    /// PDS does not recompute byte-equal to its declared CID locally.
    /// Either a canonicalization regression or a PDS-side mutation.
    #[error("peer record CID round-trip failed: expected={expected:?} actual={actual:?}")]
    PeerCidRoundTripFailed { expected: Cid, actual: Cid },
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

    // -------- slice-03 (federated read) --------
    /// Page through `org.openlore.claim` records on a peer's PDS.
    ///
    /// Per ADR-016, `peer_pds_endpoint` is re-resolved fresh from the
    /// peer's DID document on each pull (callers MUST NOT cache it);
    /// the cached `PeerSubscription.peer_pds_endpoint` is advisory only.
    /// `cursor = None` requests the first page; the returned
    /// `next_cursor` is opaque (echoed back verbatim on the next call).
    async fn list_peer_records(
        &self,
        peer_did: &Did,
        peer_pds_endpoint: &Url,
        cursor: Option<String>,
    ) -> Result<PeerRecordPage, PdsError>;

    /// Fetch one specific peer record by rkey. Used by re-pull paths
    /// where the cli already has the rkey from a previous list and
    /// wants to refresh just one record.
    async fn get_peer_record(
        &self,
        peer_did: &Did,
        peer_pds_endpoint: &Url,
        rkey: &str,
    ) -> Result<SignedRecord, PdsError>;
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

    // -------- slice-03 (federated read) --------
    /// `resolve_peer(did)` failed: the PLC directory or `did:web`
    /// endpoint is unreachable, the DID does not exist, or the
    /// returned DID document failed schema validation. `detail`
    /// carries the underlying transport / parse error for diagnostics.
    ///
    /// (Field is named `detail` rather than `source` so thiserror does
    /// not treat it as a wrapped `std::error::Error` — we carry a
    /// pre-formatted String to keep the pure-core crate dependency-free
    /// of the adapter's transport error types.)
    #[error("peer DID resolution failed for {did:?}: {detail}")]
    PeerResolutionFailed { did: Did, detail: String },
}

pub trait IdentityPort {
    fn probe(&self) -> ProbeOutcome;
    fn author_did(&self) -> &Did;
    fn sign(&self, unsigned_cid: &Cid) -> Result<SignatureBlock, IdentityError>;
    fn verify(&self, signed: &SignedClaim) -> Result<(), IdentityError>;

    // -------- slice-03 (federated read) --------
    /// Resolve a peer's DID into the information needed to subscribe
    /// to and pull from them: handle, PDS endpoint, and verification
    /// methods. Used at `peer add` (validate the DID is resolvable
    /// before persisting a subscription) AND at every `peer pull`
    /// (re-resolve fresh per ADR-016).
    fn resolve_peer(&self, peer_did: &Did) -> Result<PeerInfo, IdentityError>;
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
