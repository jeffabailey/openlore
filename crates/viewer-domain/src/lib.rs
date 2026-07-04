//! `viewer-domain` — the PURE read-only viewer core (slice-06; ADR-029).
//!
//! Holds the view-model ADTs the `openlore ui` `/claims` route projects from the
//! read-only `StoreReadPort`, plus the `render_claims_page` server-rendered-HTML
//! function (maud, compile-time HTML macro). PURE: zero I/O, no knowledge of
//! DuckDB/HTTP/hyper/the network. The effect shell (`adapter-http-viewer`) reads
//! the store, builds a [`PageView`], and calls [`render_claims_page`] — this
//! crate never touches a socket or a file.
//!
//! ## View-model (nw-fp-domain-modeling §10 — persistence ignorance)
//!
//! The viewer has two type hierarchies: the boundary DTOs in `ports`
//! ([`ports::ClaimRow`], flat, from the store) and the VIEW-model here
//! ([`ClaimRowView`], shaped for rendering). The effect shell converts
//! `ClaimRow -> ClaimRowView` (always succeeds — [`ClaimRowView::from_row`]), so
//! the renderer stays a total pure function over an already-shaped view-model.
//!
//! ## Confidence renders VERBATIM (FR-VIEW-8 — the prime mutation target)
//!
//! The stored confidence is a DOUBLE (`f64`). The operator sees it rendered
//! VERBATIM as `0.90` (two decimal places) — NEVER `0.9`, NEVER `90%`. This is
//! [`render_confidence`]; it is the load-bearing UX contract the V-1 walking
//! skeleton + the in-crate property tests below pin.

#![forbid(unsafe_code)]

pub(crate) use maud::{html, Markup, DOCTYPE};
pub(crate) use ports::{
    AuthorRelationship, CandidateClaim, ClaimDetail, ClaimRow, CounterClaimRow, PeerClaimRow,
    PeerOrigin, PeerSubscriptionSummary, SurveyRow,
};
// The PURE slice-04 `scoring` core is REUSED for the `/score` view-model
// projection (ADR-039): the renderer projects the `WeightedView` (ranked
// `WeightedPairing`s + their per-claim `Contribution` decomposition) + the
// display-only `WeightBucket`, referenced via the `scoring::` path. The scoring
// math is the pure core's job — never reimplemented here.

// ---- feature modules (split from the former monolith; see each module's //! doc) ----
mod claims;
mod common;
mod detail;
mod landing;
mod peer_claims;
mod peers;
mod score;
mod scrape;
mod search;
mod traversal;

pub use claims::*;
pub use common::*;
pub use detail::*;
pub use landing::*;
pub use peer_claims::*;
pub use peers::*;
pub use score::*;
pub use scrape::*;
pub use search::*;
pub use traversal::*;

#[cfg(test)]
mod tests;
