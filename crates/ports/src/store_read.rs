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
//! - [`PageRequest`] — the offset/limit pagination request. For the walking
//!   skeleton (step 01-01) a simple ordered read suffices; full pagination
//!   lands in step 04-01.
//! - [`Page<T>`] — a page of rows plus the total count (so the renderer can
//!   show the "N–M of TOTAL" position indicator later).
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

/// An offset/limit pagination request over the own-claim store. The viewer
/// translates a `?page=N` query (page size 50, ADR-030) into one of these. For
/// the walking skeleton a single ordered read is enough; full pagination
/// (bounds, position indicator) lands in step 04-01.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PageRequest {
    /// Zero-based row offset into the ordered result set.
    pub offset: u64,
    /// Maximum number of rows to return.
    pub limit: u64,
}

/// A page of rows plus the total matching count. `total` lets the renderer show
/// the "N–M of TOTAL" position indicator + decide whether pagination controls
/// are needed (step 04-01); the walking skeleton reads it but renders a single
/// page.
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
    /// paginated by `request`. Read-only SQL only. The walking skeleton uses a
    /// simple ordered read; full pagination lands in step 04-01.
    fn list_claims(&self, request: PageRequest) -> Result<Page<ClaimRow>, StoreReadError>;

    /// Total number of own claims in the store. Used by the store-readability
    /// startup probe (a `COUNT(*)` sentinel read) AND the position indicator.
    fn count_claims(&self) -> Result<usize, StoreReadError>;
}
