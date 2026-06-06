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
mod github;
mod graph;
mod peer_storage;

pub use federated_row::{
    AuthorRelationship, FederatedRow, PeerInfo, PeerRecordPage, PeerSubscription, SignedRecord,
    SourceTable, VerificationMethod,
};

// -----------------------------------------------------------------------------
// Slice-04 (scoring + graph) — attributed-claim feed + bounded-traversal ADTs
// -----------------------------------------------------------------------------
//
// `graph` declares the slice-04 in-memory value types returned by the four new
// `StoragePort` read methods (`query_by_object`, `query_by_contributor`,
// `query_attributed_for_scoring`, `traverse_graph`). Defined HERE (not in
// `scoring`) because BOTH the pure `scoring` core AND the `cli` composition
// root consume them, and `scoring -> ports` (never the reverse) — the
// non-cyclic home (component-boundaries.md §`crates/ports`; data-models.md
// §"In-memory value types"). `AttributedClaim` is hoisted from `scoring` (step
// 01-01) to here; `scoring` re-exports it. The non-`Option` `author_did` on
// `AttributedClaim` + `GraphEdge`, and the non-`Option` `claim_cid` on
// `GraphEdge`, are the I-GRAPH-2 / I-GRAPH-5 type-level anti-merging defenses.

pub use graph::{
    AttributedClaim, GraphEdge, GraphNode, ScoringFilter, TraversalBound, TraversalResult,
};
pub use peer_storage::{
    AddSubscriptionOutcome, HardPurgeOutcome, PeerStorageError, PeerStoragePort, SoftRemoveOutcome,
    WritePeerClaimOutcome,
};

// -----------------------------------------------------------------------------
// Slice-02 (github scraper) — GitHub port surface + harvested signal /
// candidate value types
// -----------------------------------------------------------------------------
//
// `github` owns the slice-02 value types that flow across `GithubPort`:
// `TargetKind` (repo vs user), `Signal` + `SignalKind` (one harvested public
// artifact), `CandidateClaim` (a purely-derived proposal; non-empty
// source_signals is the I-SCR-4 type-level auditability invariant), and
// `GithubError`. Placed in `ports` (not `scraper-domain`) per Q-DELIVER-3 so
// the trait signatures reference them with zero new `ports` dependency; the
// pure `scraper-domain` derivation crate (step 01-02) consumes these shapes.

pub use github::{
    CandidateClaim, CandidateClaimError, GithubError, Signal, SignalKind, TargetKind,
};

// -----------------------------------------------------------------------------
// Slice-05 (appview search) — the indexer subsystem ports + boundary ADTs
// -----------------------------------------------------------------------------
//
// FOUR new ports + the indexed-claim/raw-record boundary value types, each in
// its own submodule (mirroring the federated_row/graph/github/peer_storage
// pattern). The async ports (`IndexQueryPort`/`IngestSourcePort`/
// `IdentityResolvePort`) follow the existing `#[async_trait] pub trait X: Send +
// Sync` pattern (PdsPort/GithubPort); `IndexStorePort` is SYNC (like
// `StoragePort`). `indexed_claim` is the single home for `IndexedClaim` +
// `SearchDimension` + `CounterRef`, hoisted from `appview-domain` (step 01-02)
// so `ports` owns the boundary shapes (`appview-domain -> ports`, never the
// reverse). `RawRecord` lives with its producing port (`ingest_source`).
//
// LOAD-BEARING (WD-120 / I-AV-2): `IndexedClaim.author_did` + every transport
// row in `NetworkSearchResultRaw` carry `author_did: Did` as NON-`Option`;
// `IndexStorePort` exposes NO aggregate-across-authors method (anti-merging at
// the type + surface level). NO new external dependency added to `ports`.

mod identity_resolve;
mod index_query;
mod index_store;
mod indexed_claim;
mod ingest_source;

pub use identity_resolve::{IdentityResolvePort, ResolveError};

// -----------------------------------------------------------------------------
// Slice-06 (htmx viewer) — the READ-ONLY store port + boundary ADTs (ADR-030)
// -----------------------------------------------------------------------------
//
// `store_read` declares the `StoreReadPort` trait the `openlore ui` viewer reads
// through — it exposes NO write/sign method, so a `Box<dyn StoreReadPort>` is
// structurally incapable of mutating the store (I-VIEW-1). The adapter
// (`adapter-duckdb`) implements it over the SAME shared connection the CLI
// writes through (BR-VIEW-4). The boundary ADTs `ClaimRow`/`PageRequest`/
// `Page<T>`/`StoreReadError` are FLAT DTOs the pure `viewer-domain` core
// projects its view-model from.

mod store_read;

pub use index_query::{
    IndexQueryError, IndexQueryPort, NetworkResultRowRaw, NetworkSearchResultRaw,
};
pub use index_store::{IndexStoreError, IndexStorePort};
pub use indexed_claim::{CounterRef, IndexedClaim, SearchDimension};
pub use ingest_source::{IngestError, IngestSourcePort, RawRecord};
pub use store_read::{
    ClaimDetail, ClaimRow, Page, PageRequest, PeerClaimRow, PeerOrigin, StoreReadError,
    StoreReadPort, SurveyRow,
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
    fn record_publication(
        &self,
        cid: &Cid,
        at_uri: &str,
        published_at: DateTime<Utc>,
    ) -> Result<(), StorageError>;

    // -------- slice-03 (federated read) --------
    /// Federated subject query: returns every row across BOTH the
    /// author table (`claims`) and the peer table (`peer_claims`)
    /// matching `subject`, each carrying its `author_did` attribution.
    ///
    /// Per WD-30 (layered anti-merging), the implementation MUST use
    /// SQL `UNION ALL` with explicit `author_did` projection — NOT a
    /// `JOIN` that could elide the column. `xtask check-arch`
    /// enforces this structurally.
    fn query_federated_by_subject(&self, subject: &str) -> Result<Vec<FederatedRow>, StorageError>;

    // -------- slice-04 (scoring + graph) --------
    //
    // Four read methods, all SYNC local reads over the SAME single-file
    // DuckDB store (NO new table; NO store swap — `adapter-duckdb` AUGMENT,
    // WD-8). Each returns PER-CLAIM / PER-EDGE rows carrying a non-`Option`
    // `author_did`: aggregation (the weight) happens later in the pure
    // `scoring` core in Rust, NEVER in SQL (WD-73 anti-merging-in-aggregates).
    // The cross-store SQL uses `UNION ALL` with explicit `author_did`, NEVER a
    // merging `JOIN`/`GROUP BY` — `xtask check-arch`'s extended
    // `no_cross_table_join_elides_author` enforces it structurally. Adapter
    // impls land in step 01-03; these are declarations only.

    /// Which claims assert this `object` (philosophy), across own + peer
    /// stores. Every row carries its non-`Option` `author_did`; the renderer
    /// groups by subject. Two identical-content claims from different authors
    /// stay TWO rows (never merged — I-GRAPH-2).
    fn query_by_object(&self, object: &str) -> Result<Vec<AttributedClaim>, StorageError>;

    /// Every claim authored by this DID, across all subjects, own + peer
    /// stores. Drives `--contributor`: "one developer's reasoning trail, not a
    /// community consensus".
    fn query_by_contributor(&self, author_did: &Did) -> Result<Vec<AttributedClaim>, StorageError>;

    /// The attributed-claim feed for the pure `scoring::score` core. Returns
    /// per-claim rows (the `UNION ALL` of [`Self::query_by_object`]'s shape),
    /// NEVER a SQL aggregate — so the weight the pure core computes always
    /// decomposes into these rows (Gate 1 / I-GRAPH-2).
    fn query_attributed_for_scoring(
        &self,
        filter: &ScoringFilter,
    ) -> Result<Vec<AttributedClaim>, StorageError>;

    /// Bounded, cycle-safe traversal of contributor↔project↔philosophy edges
    /// from `start`, capped at `bound.max_depth` (WD-76). Each [`GraphEdge`]
    /// maps to exactly ONE signed claim (`claim_cid` non-`Option`, Gate 5 /
    /// I-GRAPH-5); the recursive CTE selects FROM existing rows only and is
    /// depth-bounded + visited-set-guarded (ADR-021) — it never fabricates an
    /// edge nor loops on a cyclic graph.
    fn traverse_graph(
        &self,
        start: &GraphNode,
        bound: &TraversalBound,
    ) -> Result<TraversalResult, StorageError>;
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
// Slice-02 (github scraper) — GithubPort (WD-61 / ADR-019)
// -----------------------------------------------------------------------------

/// The GitHub-scraper driving port.
///
/// A NEW port (NOT a `PdsPort` extension): GitHub is a wholly different
/// external system from ATProto — no method shape, auth model, rate-limit
/// semantic, or failure surface is shared (WD-61). `adapter-github`
/// (step 01-03/04) implements it over the GitHub PUBLIC REST/GraphQL API and
/// holds the optional `GITHUB_TOKEN` PAT as an effect-shell credential. By
/// construction the adapter holds NO `StoragePort` / `IdentityPort` /
/// `PdsPort` reference — it CANNOT sign or publish (the human-gate at the
/// architecture layer, I-SCR-1).
///
/// `async fn` (network I/O) so `#[async_trait]` is permitted exactly as for
/// `PdsPort` (ADR-004 / component-boundaries.md §`crates/ports`). Trait
/// bodies are declarations only here; the probe + harvest logic lives in the
/// adapter.
///
/// Harvest returns already-fetched [`Signal`]s ready for the pure
/// `scraper-domain::derive_candidates` (step 01-02) — no derivation happens
/// in the adapter.
#[async_trait]
pub trait GithubPort: Send + Sync {
    /// Earned-Trust probe — see ADR-009 + `probe.rs`. The `adapter-github`
    /// implementation exercises (within the 250ms budget, I-5): public
    /// reachability; private-target refusal (the load-bearing KPI-SCR-4
    /// check); optional-PAT auth mode + rate-budget reporting; rate-limit
    /// header parsing; and the no-token-leak invariant.
    fn probe(&self) -> ProbeOutcome;

    /// Disambiguate `owner/repo` ([`TargetKind::Repo`]) vs `user`
    /// ([`TargetKind::User`]); REFUSE private / non-existent targets with
    /// [`GithubError::NotPublic`] / [`GithubError::NotFound`].
    /// Public-data-only (WD-51 / I-SCR-2).
    async fn resolve_target(&self, target: &str) -> Result<TargetKind, GithubError>;

    /// Harvest the bounded public-signal set for a repo. Returns
    /// already-fetched [`Signal`]s ready for `derive_candidates`.
    async fn harvest_repo(&self, owner: &str, repo: &str) -> Result<Vec<Signal>, GithubError>;

    /// Harvest a BOUNDED cross-repo aggregate for a user / contributor
    /// target (deep triangulation deferred to slice-04 per WD-64).
    async fn harvest_user(&self, user: &str) -> Result<Vec<Signal>, GithubError>;
}

// -----------------------------------------------------------------------------
// Re-exports for adapter ergonomics
// -----------------------------------------------------------------------------

pub use claim_domain;
pub use lexicon;
