//! `store_read` — the slice-06 READ-ONLY store port (ADR-030).
//!
//! The `openlore ui` viewer reads the operator's OWN node store as
//! server-rendered HTML over a port that exposes NO write/sign surface. This is
//! the structural read-only guarantee (I-VIEW-1 / ADR-030): a `StoreReadPort`
//! trait object cannot mutate the store because the trait declares no mutation
//! method. The adapter (`adapter-duckdb`) implements it over the SAME shared
//! connection the CLI writes through — there is no second handle, no second
//! file (BR-VIEW-4).
//!
//! ## Boundary ADTs (data-models.md §"Read-side query shapes")
//!
//! - [`ClaimRow`] — one own-claim row projected from the `claims` table:
//!   subject/predicate/object/confidence (the DOUBLE numeric, rendered VERBATIM
//!   by the pure viewer core, FR-VIEW-8)/author_did/composed_at/cid. A FLAT,
//!   serialization-friendly shape (DTO, not the rich `SignedClaim`).
//! - [`PageRequest`] — the offset/limit pagination request the viewer derives
//!   from the `?page=N` query (page size 50, ADR-030).
//! - [`Page<T>`] — a page of rows plus the total count, so the renderer can
//!   show the "N–M of TOTAL" position indicator (FR-VIEW-6).
//! - [`StoreReadError`] — read failures, surfaced as a plain-language error by
//!   the viewer (NFR-VIEW-6), never a raw stack trace.

use chrono::{DateTime, Utc};

/// One own-claim row from the `claims` table, projected for the read-only
/// viewer. A FLAT DTO (not the rich `claim_domain::SignedClaim`): the viewer
/// renders these fields verbatim and never needs the signature/canonical-CBOR
/// shape. `confidence` is the stored DOUBLE — the pure viewer core renders it
/// VERBATIM as `0.90` (FR-VIEW-8), never `0.9` nor `90%`.
#[derive(Debug, Clone, PartialEq)]
pub struct ClaimRow {
    pub cid: String,
    pub subject: String,
    pub predicate: String,
    pub object: String,
    pub confidence: f64,
    pub author_did: String,
    pub composed_at: DateTime<Utc>,
}

/// One own-claim's FULL detail, projected for the read-only detail view
/// (`/claims/{cid}`, US-VIEW-002). The flat claim fields PLUS the COMPLETE
/// `evidence[]` array the list view summarizes away (FR-VIEW-3) — ordered by
/// the `claim_evidence.ordinal` column so the operator sees the evidence in the
/// order it was attached. A FLAT DTO (not the rich `SignedClaim`): the detail
/// renderer reads these fields verbatim. `confidence` is the stored DOUBLE,
/// rendered VERBATIM (FR-VIEW-8).
#[derive(Debug, Clone, PartialEq)]
pub struct ClaimDetail {
    pub cid: String,
    pub subject: String,
    pub predicate: String,
    pub object: String,
    pub confidence: f64,
    pub author_did: String,
    pub composed_at: DateTime<Utc>,
    /// The claim's evidence URLs, ordered by `claim_evidence.ordinal` ascending
    /// (the order they were attached). Empty when the claim was signed without
    /// evidence (the detail view then shows an explicit "no evidence attached"
    /// state — step 02-02).
    pub evidence: Vec<String>,
}

/// Where a federated peer claim came from — its peer ORIGIN (US-VIEW-003 /
/// FR-VIEW-4). There is no `peer_origin` column in the slice-03 schema; the
/// origin IS the pair (`author_did`, `fetched_from_pds`) per data-models.md.
///
/// Modeled as an ADT so the "mine vs federated never ambiguous" contract
/// (BR-VIEW-5) and the future unknown-origin path (step 03-03 / V-10) are both
/// total at the type level:
///
/// - [`PeerOrigin::Known`] — the schema-guaranteed common case: `author_did` is
///   NON-EMPTY (the slice-03 CHECK enforces `author_did <> ''`). Carries the
///   peer's DID + the PDS it was fetched from.
/// - [`PeerOrigin::Unknown`] — the DEFENSIVE path: a row whose `author_did` is
///   blank/absent (data that predates/bypasses the CHECK). Step 03-01 only
///   PRODUCES `Known` (the production `peer pull` path always sets a non-empty
///   `author_did`); the `Unknown` variant is here so step 03-03 (V-10) is a
///   clean, total extension — the renderer matches both arms, never drops a row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PeerOrigin {
    /// A peer claim with a known origin: the peer's `author_did` (NON-EMPTY) and
    /// the PDS endpoint it was `fetched_from`.
    Known {
        /// The peer's DID — the bare `did:plc:...` stored in
        /// `peer_claims.author_did`. Rendered VERBATIM (attribution discipline,
        /// I-FED-1 / FR-VIEW-4): the operator sees exactly who authored it.
        author_did: String,
        /// The PDS endpoint the claim was fetched from
        /// (`peer_claims.fetched_from_pds`).
        fetched_from_pds: String,
    },
    /// A peer claim whose origin is absent/blank (defensive). Renders labeled
    /// "unknown" rather than being dropped (step 03-03 / V-10).
    Unknown,
}

/// One federated PEER-claim row from the `peer_claims` table (slice-03),
/// projected for the read-only Peer Claims view (`/peer-claims`, US-VIEW-003).
/// A FLAT DTO (not the rich `SignedClaim`). DISTINCT from [`ClaimRow`] (own
/// claims) so the viewer can render peers on a SEPARATE surface where
/// "mine vs federated" is never ambiguous (BR-VIEW-5).
///
/// `confidence` is the stored DOUBLE, rendered VERBATIM (FR-VIEW-8). `origin`
/// carries the peer ORIGIN ([`PeerOrigin`]) — the peer's `author_did` +
/// `fetched_from_pds`, projected verbatim (there is no `peer_origin` column).
#[derive(Debug, Clone, PartialEq)]
pub struct PeerClaimRow {
    pub cid: String,
    pub subject: String,
    pub predicate: String,
    pub object: String,
    pub confidence: f64,
    /// The peer ORIGIN — who authored this federated claim + the PDS it came
    /// from. The "distinct from own" / attribution surface (BR-VIEW-5).
    pub origin: PeerOrigin,
    pub composed_at: DateTime<Utc>,
}

/// An offset/limit pagination request over the own-claim store. The viewer
/// translates a `?page=N` query (page size 50, ADR-030) into one of these: the
/// offset/limit selects one page, the bounds + position indicator are projected
/// by the pure `viewer-domain` `PageView` over the returned [`Page::total`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PageRequest {
    /// Zero-based row offset into the ordered result set.
    pub offset: u64,
    /// Maximum number of rows to return.
    pub limit: u64,
}

/// A page of rows plus the total matching count. `total` lets the renderer show
/// the "N–M of TOTAL" position indicator + decide whether pagination controls
/// are needed (FR-VIEW-6) — it is the whole-set `COUNT(*)`, not `rows.len()`.
#[derive(Debug, Clone, PartialEq)]
pub struct Page<T> {
    pub rows: Vec<T>,
    pub total: u64,
}

/// Why a read-only store read failed. The viewer surfaces these as
/// plain-language messages (NFR-VIEW-6), never a raw stack trace. `Unreadable`
/// is the store-readability probe failure (another process holds the file;
/// ADR-030 §Earned-Trust step 1).
#[derive(Debug, thiserror::Error)]
pub enum StoreReadError {
    /// The store could not be opened/read (locked by another process, missing,
    /// permissions). Carries a plain-language detail for the operator.
    #[error("store unreadable: {detail}")]
    Unreadable { detail: String },
    /// A read query failed for a reason other than store-unreadability.
    #[error("store read query failed: {detail}")]
    QueryFailed { detail: String },
}

/// The READ-ONLY store port the `openlore ui` viewer reads (ADR-030). Exposes
/// ONLY read methods — there is NO `write_*` / `sign` method on this trait, so a
/// `Box<dyn StoreReadPort>` is structurally incapable of mutating the store
/// (I-VIEW-1). The adapter shares the CLI's connection (BR-VIEW-4).
pub trait StoreReadPort: Send + Sync {
    /// List own claims ordered for display (composed_at DESC per ADR-030),
    /// paginated by `request` (the `?page=N` offset/limit). Read-only SQL only;
    /// returns the page rows plus the whole-set `total` for the indicator.
    fn list_claims(&self, request: PageRequest) -> Result<Page<ClaimRow>, StoreReadError>;

    /// Total number of own claims in the store. Used by the store-readability
    /// startup probe (a `COUNT(*)` sentinel read) AND the position indicator.
    fn count_claims(&self) -> Result<usize, StoreReadError>;

    /// Fetch ONE own claim by CID together with its COMPLETE, ordinal-ordered
    /// `evidence[]` (US-VIEW-002 / FR-VIEW-3). Returns `Ok(None)` when no claim
    /// with that CID exists (the viewer renders a guided not-found — step 02-03),
    /// `Ok(Some(detail))` for a known CID. Read-only SQL only: a SELECT over
    /// `claims` joined to `claim_evidence` by `cid`, ordered by `ordinal`.
    fn get_claim(&self, cid: &str) -> Result<Option<ClaimDetail>, StoreReadError>;

    /// List federated PEER claims ordered for display (composed_at DESC, mirroring
    /// `list_claims`), paginated by `request` (US-VIEW-003 / FR-VIEW-4). Read-only
    /// SQL only — a SELECT over the SAME shared connection's `peer_claims` table
    /// (slice-03). Each row carries its peer ORIGIN ([`PeerOrigin`]: the peer's
    /// `author_did` + `fetched_from_pds`) so the viewer renders peers on a SEPARATE
    /// surface, "mine vs federated" never ambiguous (BR-VIEW-5).
    fn list_peer_claims(&self, request: PageRequest) -> Result<Page<PeerClaimRow>, StoreReadError>;

    /// Total number of federated peer claims in the store. The Peer Claims
    /// position indicator + empty-state decision (US-VIEW-003) read this
    /// `COUNT(*)` over `peer_claims`.
    fn count_peer_claims(&self) -> Result<usize, StoreReadError>;
}
