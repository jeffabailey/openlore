//! `render` — pure-function renderers for verb output blocks.
//!
//! Step 05-11 introduces `render_graph_query_result`: turn a list of
//! `SignedClaim` values (as returned by `StoragePort::query_by_subject`)
//! into a human-friendly stdout block. The renderer is pure (no I/O,
//! no clock, no storage access) so it can be unit-tested without spinning
//! up a wiring.
//!
//! ## Byte-for-byte invariant (KPI-4)
//!
//! The graph-query output prints every field VERBATIM from the SignedClaim
//! serde model — the same model the write path canonicalizes through. No
//! normalization happens here:
//!
//! - `confidence` is rendered as the original `f64` (e.g. `0.86`), NEVER
//!   as a bucket label like `well-evidenced`. Bucket labels are
//!   compose-time display only and MUST NOT appear here (WD-10 / D-12).
//! - `evidence` URLs are printed verbatim, one per line, no
//!   reformatting or scheme normalization.
//! - `composedAt` keeps the exact RFC3339 string the author composed
//!   with, including timezone marker (no conversion to local time).
//! - `author` is the full DID string with verification-method fragment.
//!
//! These invariants make round-trip identity (compose → sign → publish →
//! query) verifiable byte-for-byte at the CLI boundary, which is KPI-4's
//! zero-silent-normalization promise.
//!
//! ## Field label format
//!
//! Each claim renders as a labeled block:
//!
//! ```text
//! subject:     github:rust-lang/rust
//! predicate:   embodiesPhilosophy
//! object:      org.openlore.philosophy.memory-safety
//! evidence:    https://www.rust-lang.org/
//! confidence:  0.86
//! author:      did:plc:test-jeff#org.openlore.application
//! composedAt:  2026-05-25T12:00:00Z
//! cid:         <signed_cid>
//! ```
//!
//! When multiple claims match, blocks are separated by a blank line so
//! awk/grep/cut-style downstream tooling can split on `\n\n`.
//!
//! ## WS-15: retraction annotation (ADR-008 Behavioral rule 3 + WD-11)
//!
//! Per WD-11 "no hard-delete", a retracted claim is preserved verbatim
//! in both the local store and the PDS. The retraction is published as
//! a NEW counter-claim referencing the original. To make the retract
//! VISIBLE without mutating immutable history, the render layer
//! annotates the original claim with the literal string
//! `retracted by author` on its own line at the end of the block.
//!
//! The annotation is content-frozen UX (WD-11) — do NOT paraphrase. The
//! annotation list is computed by the verb via
//! `StoragePort::query_referencing` and passed alongside each claim so
//! the renderer stays pure (no I/O, no storage access).

pub(crate) use adapter_github::AuthReport;
pub(crate) use claim_domain::{Cid, SignedClaim};
pub(crate) use ports::{
    AttributedClaim, AuthorRelationship, CandidateClaim, FederatedRow, GraphEdge, GraphNode,
    NetworkResultRowRaw, NetworkSearchResultRaw, SearchDimension, SourceTable, TraversalResult,
};

// ---- verb-output render modules (split from the former 3117-line monolith) ----
mod claim;
mod common;
mod contributor;
mod counter;
mod federated;
mod graph;
mod object_query;
mod philosophy;
mod score;
mod scrape;
mod search;
mod traversal;

// A `pub use m::*` glob re-exports each module's `pub` items as the external
// render API (consumed as `cli::render::render_*` by the verbs + acceptance
// tests) AND its `pub(crate)` helpers at pub(crate) visibility — enough for the
// heavily-coupled `tests` submodule to reach them via `use super::*`. `common`
// holds only pub(crate) helpers (no external API), so it re-exports at
// pub(crate).
pub use claim::*;
pub(crate) use common::*;
pub use contributor::*;
pub use counter::*;
pub use federated::*;
pub use graph::*;
pub use object_query::*;
pub use philosophy::*;
pub use score::*;
pub use scrape::*;
pub use search::*;
pub use traversal::*;

#[cfg(test)]
mod tests;
