//! `suggest` — the PURE near-match suggestion for an empty dimension result.
//!
//! [`near_match_suggestion`] computes a near-match (edit distance over the known
//! dimension values) so an empty `--object`/`--contributor`/`--subject` result
//! can offer "did you mean …?" (US-AV-002 Ex 4) instead of a bare zero-result.
//! Pure; deterministic; no I/O.
//!
//! The body is intentionally `todo!()` at the 01-01 bootstrap; the suggestion
//! behavior is driven by the Phase 02+ empty-result scenario. Split into its own
//! module for mutation-test clarity (D-D40).
//
// SCAFFOLD: true

/// Near-match suggestion for an empty dimension result (edit distance over
/// `known` values). Returns `Some(suggestion)` when a close-enough known value
/// exists, else `None`. Deterministic; no I/O.
pub fn near_match_suggestion(_query: &str, _known: &[String]) -> Option<String> {
    // SCAFFOLD: true — behavior driven by the Phase 02+ empty-result scenario.
    todo!("near_match_suggestion — driven by the empty-result near-match scenario (US-AV-002 Ex 4)")
}
