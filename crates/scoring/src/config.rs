//! `config` — the scoring constants SSOT (WD-71 / WD-77).
//!
//! The adherence-weight formula is a small, closed-form, reproducible,
//! NO-ML function. Its constants live here as compile-time `const` values,
//! NOT in a runtime config file: a constant change is a code change, never a
//! learned/tuned weight (WD-71; Q-DELIVER config-vs-const resolves to const).

use serde::{Deserialize, Serialize};

/// The formula constants SSOT (WD-77 defaults; DESIGN-tunable by editing the
/// `DEFAULT` const; small / closed-form / no-ML).
///
/// `Copy` because it is a handful of `f64`s threaded by value into the pure
/// `score` core — no ownership semantics, no allocation.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ScoringConfig {
    /// `+0.25` per ADDITIONAL distinct author on the SAME `(subject, object)`
    /// pairing (the first author contributes the base multiplier of 1.0).
    pub author_distinct_bonus: f64,

    /// `+0.50` when the SAME author asserts the `object` on `>= 2` distinct
    /// subjects (cross-project triangulation).
    pub cross_project_triangulation_bonus: f64,

    /// Bucket cut: a pairing that clears the breadth guard and reaches this
    /// weight buckets `Strong`.
    pub strong_threshold: f64,

    /// Bucket cut: a pairing that clears the breadth guard and reaches this
    /// weight (but not `strong_threshold`) buckets `Moderate`.
    pub moderate_threshold: f64,
}

impl ScoringConfig {
    /// The WD-77 default constants — the SSOT. A change here is a reviewed
    /// code change, never a learned weight (WD-71).
    pub const DEFAULT: ScoringConfig = ScoringConfig {
        author_distinct_bonus: 0.25,
        cross_project_triangulation_bonus: 0.50,
        strong_threshold: 2.0,
        moderate_threshold: 1.0,
    };
}

impl Default for ScoringConfig {
    fn default() -> Self {
        Self::DEFAULT
    }
}
