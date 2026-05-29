//! `compose` — the PURE anti-merging search composition (WD-103 / I-AV-2).
//!
//! [`compose_results`] groups verified, attributed [`IndexedClaim`] rows into a
//! [`NetworkSearchResult`]. It groups BY AUTHOR (or by subject under an author)
//! and NEVER merges authors: two identical-content claims by different authors
//! produce two rows under two authors. `distinct_author_count` is a COUNT over
//! the rows, NEVER a stored or computed merge. There is no API that yields a
//! faceless "network consensus" row — the absence is the design (WD-103).
//!
//! The body is intentionally `todo!()` at the 01-01 bootstrap; the grouping +
//! attribution-preservation behavior is driven by the Phase 02+ search
//! scenarios (KPI-AV-2 `network_result_preserves_attribution`) + the OD-AV-7
//! counter-annotation scenario. Split into its own module for mutation-test
//! clarity (D-D40).
//
// SCAFFOLD: true

use crate::{IndexedClaim, NetworkSearchResult, SearchDimension};

/// The PURE anti-merging-preserving search composition. Groups by author (or by
/// subject under an author per `dimension`); NEVER merges authors; computes
/// `distinct_author_count` from the rows. Deterministic; no I/O.
pub fn compose_results(
    _rows: Vec<IndexedClaim>,
    _dimension: SearchDimension,
) -> NetworkSearchResult {
    // SCAFFOLD: true — behavior driven by the Phase 02+ anti-merging search scenarios.
    todo!("compose_results — driven by the anti-merging search acceptance scenarios (WD-103)")
}
