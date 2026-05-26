//! Display-only confidence bucket helper (WD-10 / D-12).
//!
//! Pure function. NO I/O. NO serde. Maps a numeric confidence in
//! `[0.0, 1.0]` to one of four labelled buckets used ONLY for UI /
//! CLI preview text (WS-3, WS-5 in phase 05). Bucket labels are
//! NEVER persisted into the canonical CBOR claim payload — that
//! invariant is the reason this module lives separately from
//! `crates/lexicon`. A future `cargo xtask check-arch` (phase 06)
//! enforces the architectural rule: the lexicon crate MUST NOT
//! depend on (or import) this function.
//!
//! ## Thresholds (per WD-10 / feature-delta)
//!
//! - `[0.0, 0.3)` → `Speculative`
//! - `[0.3, 0.7)` → `Weighted`
//! - `[0.7, 0.9)` → `WellEvidenced`
//! - `[0.9, 1.0]` → `Triangulated`
//!
//! The upper boundary at `1.0` is closed so an exact `1.0` confidence
//! still maps to `Triangulated`. The lower boundaries are open so the
//! threshold values themselves (`0.3`, `0.7`, `0.9`) belong to the
//! HIGHER bucket — the bucket whose interval STARTS at that value.

use crate::ConfidenceBucket;

/// Map a numeric confidence value to its display-only bucket per WD-10.
///
/// Pure, total: every `f64` produces a bucket. Values outside `[0.0,
/// 1.0]` are clamped by the comparison ordering — `< 0.3` covers
/// negatives, `>= 0.9` covers values above `1.0`. The smart constructor
/// `Confidence::try_new` is the place that REJECTS out-of-range inputs;
/// this helper is intentionally total so display code paths never
/// panic when handed a raw `f64`.
pub fn confidence_bucket(numeric: f64) -> ConfidenceBucket {
    if numeric < 0.3 {
        ConfidenceBucket::Speculative
    } else if numeric < 0.7 {
        ConfidenceBucket::Weighted
    } else if numeric < 0.9 {
        ConfidenceBucket::WellEvidenced
    } else {
        ConfidenceBucket::Triangulated
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Criterion 1: 0.10 → Speculative (well inside the lowest bucket).
    #[test]
    fn confidence_bucket_maps_0_10_to_speculative() {
        assert_eq!(confidence_bucket(0.10), ConfidenceBucket::Speculative);
    }

    /// Criterion 2: 0.55 → Weighted (middle of the second bucket).
    #[test]
    fn confidence_bucket_maps_0_55_to_weighted() {
        assert_eq!(confidence_bucket(0.55), ConfidenceBucket::Weighted);
    }

    /// Criterion 3: 0.86 → WellEvidenced (well inside the third bucket).
    #[test]
    fn confidence_bucket_maps_0_86_to_well_evidenced() {
        assert_eq!(confidence_bucket(0.86), ConfidenceBucket::WellEvidenced);
    }

    /// Criterion 4: 0.95 → Triangulated (well inside the top bucket).
    #[test]
    fn confidence_bucket_maps_0_95_to_triangulated() {
        assert_eq!(confidence_bucket(0.95), ConfidenceBucket::Triangulated);
    }

    /// Boundary 0.3 belongs to `Weighted` (lower edge is closed).
    #[test]
    fn confidence_bucket_at_lower_weighted_boundary_is_weighted() {
        assert_eq!(confidence_bucket(0.3), ConfidenceBucket::Weighted);
    }

    /// Boundary 0.7 belongs to `WellEvidenced` (lower edge is closed).
    #[test]
    fn confidence_bucket_at_lower_well_evidenced_boundary_is_well_evidenced() {
        assert_eq!(confidence_bucket(0.7), ConfidenceBucket::WellEvidenced);
    }

    /// Boundary 0.9 belongs to `Triangulated` (lower edge is closed).
    #[test]
    fn confidence_bucket_at_lower_triangulated_boundary_is_triangulated() {
        assert_eq!(confidence_bucket(0.9), ConfidenceBucket::Triangulated);
    }

    /// Upper edge `1.0` stays inside `Triangulated` (closed upper
    /// boundary). Documents the asymmetric-interval choice.
    #[test]
    fn confidence_bucket_at_upper_boundary_is_triangulated() {
        assert_eq!(confidence_bucket(1.0), ConfidenceBucket::Triangulated);
    }
}
