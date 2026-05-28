//! `scoring` — the pure, transparent, NO-ML adherence-weight core.
//!
//! Computes the DERIVED, DISPLAY-ONLY adherence weight for `(subject, object)`
//! pairings from attributed claims via a small closed-form, reproducible
//! formula. Holds the formula constants as the SSOT ([`ScoringConfig::DEFAULT`],
//! WD-71/WD-77). Exposes the per-claim [`Contribution`] decomposition so
//! `--explain` reproduces the arithmetic by hand (Gate 2). NO I/O, NO
//! persistence, NO knowledge of DuckDB (ADR-022; xtask pure-core allowlist).
//!
//! Hexagonal pure core (ADR-009 + ADR-007), the symmetric counterpart to
//! slice-02's `scraper-domain`. The composition root (`crates/cli`) wires the
//! attributed-claim feed from the extended `StoragePort` through [`score`].
//!
//! Bootstrap (step 01-01): the ADTs + const-SSOT config + [`score`] signature
//! land here; the formula body is `todo!()`, driven fully by the Phase 02 SC
//! acceptance scenarios (Gate 2/3/6).
//
// SCAFFOLD: true

#![forbid(unsafe_code)]

mod config;
mod explain;
mod score;

pub use config::ScoringConfig;
pub use explain::Contribution;
pub use score::{
    score, weight_bucket, EmptyContributions, WeightBucket, WeightedPairing, WeightedView,
};

// `AttributedClaim` is hoisted to `ports` (step 01-02) so the single
// definition is shared by the `cli` composition root, the extended
// `StoragePort` read methods, and this pure core. Re-exported here so existing
// `scoring::AttributedClaim` call-sites keep working (`scoring -> ports`, the
// non-cyclic placement).
pub use ports::AttributedClaim;

#[cfg(test)]
mod tests {
    use super::*;
    use claim_domain::{Cid, Did};
    use proptest::prelude::*;

    // -- Behavior 1: the const-SSOT config carries the WD-77 constants -------
    //
    // Example-based (the const is a single fixed value; a property over "the
    // one constant" would be vacuous). This pins the SSOT so a silent edit to
    // the formula weights is caught.
    #[test]
    fn default_config_carries_wd77_constants() {
        let cfg = ScoringConfig::DEFAULT;
        assert_eq!(
            cfg.author_distinct_bonus, 0.25,
            "WD-77 author-distinct bonus"
        );
        assert_eq!(
            cfg.cross_project_triangulation_bonus, 0.50,
            "WD-77 cross-project triangulation bonus"
        );
        // `Default` agrees with the named SSOT const.
        assert_eq!(ScoringConfig::default(), cfg);
    }

    // -- Behavior 2: Gate 2 — weight == sum(contributions.subtotal) ----------
    //
    // Property over arbitrary contribution subtotals: a `WeightedPairing`
    // built with `weight = sum(subtotals)` satisfies the Gate 2 invariant for
    // ANY non-empty contribution set, AND the smart constructor enforces the
    // non-empty decomposition (Gate 1 type-level). The empty case proves the
    // constructor rejects an undecomposable weight.
    proptest! {
        #[test]
        fn weight_equals_sum_of_subtotals_and_contributions_non_empty(
            subtotals in prop::collection::vec(-10.0_f64..10.0, 1..12)
        ) {
            let contributions: Vec<Contribution> = subtotals
                .iter()
                .enumerate()
                .map(|(i, &subtotal)| Contribution {
                    author_did: Did(format!("did:plc:author{i}")),
                    cid: Cid(format!("bafyclaim{i}")),
                    base: subtotal,
                    author_distinct_bonus: 1.0,
                    cross_project_triangulation_bonus: 0.0,
                    subtotal,
                })
                .collect();

            let weight: f64 = subtotals.iter().sum();

            let pairing = WeightedPairing::new(
                "deno".to_string(),
                "dependency-pinning".to_string(),
                weight,
                WeightBucket::Moderate,
                contributions.len() as u32,
                contributions.len() as u32,
                1.0,
                1,
                contributions.clone(),
            )
            .expect("a non-empty contribution set must construct a WeightedPairing");

            // Gate 2: the displayed weight reproduces by hand from the
            // per-claim subtotals.
            let recomputed: f64 = pairing.contributions().iter().map(|c| c.subtotal).sum();
            prop_assert!((pairing.weight - recomputed).abs() < 1e-9);

            // Gate 1 (type-level): the decomposition is non-empty and every
            // contribution carries its non-Option author_did.
            prop_assert!(!pairing.contributions().is_empty());
            for c in pairing.contributions() {
                prop_assert!(!c.author_did().0.is_empty());
            }

            // The smart constructor refuses an undecomposable weight.
            prop_assert_eq!(
                WeightedPairing::new(
                    "deno".to_string(),
                    "dependency-pinning".to_string(),
                    weight,
                    WeightBucket::Moderate,
                    0,
                    0,
                    1.0,
                    1,
                    Vec::new(),
                ),
                Err(EmptyContributions)
            );
        }
    }

    // -- Behavior 3: WD-74/WD-90 breadth guard — each dimension lifts ---------
    //
    // The breadth guard (`weight_bucket`, score.rs:310-312) is LOAD-BEARING:
    // a single thin opinion stays Sparse REGARDLESS of how high its weight is.
    // These boundary tests pin the guard at a weight HIGH enough (>= the SSOT
    // strong_threshold) that ANY breach of the guard is observable as a
    // mis-bucket to Strong instead of the correct Sparse — the existing
    // fixtures used a below-moderate weight, which let a flipped guard fall
    // through to the same Sparse via the weight else-branch and hid the
    // mutants. Thresholds are pulled from `ScoringConfig::DEFAULT` (the WD-77
    // SSOT), never hardcoded, so the tests track the SSOT.
    //
    // Mutation coverage (the 9 score.rs:310/314 survivors):
    //   - claim_count `> 1` -> `== 1` / `< 1`: case A (2,1,1) is non-Sparse;
    //     a `==1`/`<1` mutant computes claim_count breadth = false -> Sparse.
    //   - claim_count `> 1` -> `>= 1`: case D (1,1,1) is Sparse; a `>=1` mutant
    //     treats claim_count=1 as breadth -> buckets by weight -> Strong.
    //   - distinct_author_count `> 1` -> `== 1` / `< 1`: case B (1,2,1) is
    //     non-Sparse; the mutant -> author breadth false -> Sparse.
    //   - distinct_author_count `> 1` -> `>= 1`: case D (1,1,1) -> mutant
    //     treats author=1 as breadth -> Strong, correct is Sparse.
    //   - cross_project_span `> 1` -> `>= 1`: case D (1,1,1) -> mutant treats
    //     span=1 as breadth -> Strong, correct is Sparse. (The key gap.)
    //   - `||` (author/span, col 39) -> `&&`: case C (1,1,2) lifts via span
    //     ALONE; `&&` mutant needs BOTH author>1 AND span>1 -> false ->
    //     Sparse, correct is Strong.
    //   - `weight >= strong_threshold` -> `weight < strong_threshold`
    //     (score.rs:314): case A (2,1,1) at exactly strong_threshold is Strong;
    //     the flipped mutant falls through to Moderate.
    #[test]
    fn breadth_guard_each_dimension_lifts_out_of_sparse_at_high_weight() {
        let cfg = ScoringConfig::DEFAULT;
        // High weight: at the strong cut, so a correctly-bredth pairing is
        // Strong and any guard breach is observable (Strong/Moderate != Sparse).
        let w_strong = cfg.strong_threshold;

        // Case A: claim_count alone is the breadth (2,1,1) -> bucket by weight.
        assert_eq!(
            weight_bucket(w_strong, 2, 1, 1, &cfg),
            WeightBucket::Strong,
            "claim_count=2 alone clears the breadth guard; at the strong cut -> Strong"
        );
        // Case B: distinct_author_count alone is the breadth (1,2,1).
        assert_eq!(
            weight_bucket(w_strong, 1, 2, 1, &cfg),
            WeightBucket::Strong,
            "distinct_author_count=2 alone clears the breadth guard -> Strong"
        );
        // Case C: cross_project_span alone is the breadth (1,1,2) — the OR-term
        // that must NOT degrade to AND.
        assert_eq!(
            weight_bucket(w_strong, 1, 1, 2, &cfg),
            WeightBucket::Strong,
            "cross_project_span=2 alone clears the breadth guard -> Strong"
        );
        // Case D: NO breadth (1,1,1) -> Sparse DESPITE a strong-cut weight. The
        // guard overrides weight magnitude (Gate 3; mitigates J-002).
        assert_eq!(
            weight_bucket(w_strong, 1, 1, 1, &cfg),
            WeightBucket::Sparse,
            "thin evidence (1,1,1) stays Sparse regardless of a high weight"
        );
    }

    // -- Behavior 4: threshold boundaries WITH breadth present ----------------
    //
    // Once the breadth guard clears, the bucket is decided purely by weight
    // against the SSOT cuts. Pin all three cuts at the boundary so the
    // `>=`/`<` mutants on score.rs:314/316 are observable. Breadth is held
    // constant (claim_count=2) so the guard never confounds the threshold
    // assertions. Thresholds come from the SSOT const.
    #[test]
    fn threshold_boundaries_with_breadth_bucket_by_weight() {
        let cfg = ScoringConfig::DEFAULT;
        // Exactly at strong_threshold -> Strong (kills `>= -> <` at line 314).
        assert_eq!(
            weight_bucket(cfg.strong_threshold, 2, 1, 1, &cfg),
            WeightBucket::Strong,
            "weight == strong_threshold buckets Strong"
        );
        // Just below strong, but >= moderate -> Moderate.
        let just_below_strong = (cfg.strong_threshold + cfg.moderate_threshold) / 2.0;
        assert!(
            just_below_strong < cfg.strong_threshold && just_below_strong >= cfg.moderate_threshold,
            "fixture midpoint must sit in [moderate, strong)"
        );
        assert_eq!(
            weight_bucket(just_below_strong, 2, 1, 1, &cfg),
            WeightBucket::Moderate,
            "weight in [moderate_threshold, strong_threshold) buckets Moderate"
        );
        // Exactly at moderate_threshold -> Moderate (kills `>= -> <` at 316).
        assert_eq!(
            weight_bucket(cfg.moderate_threshold, 2, 1, 1, &cfg),
            WeightBucket::Moderate,
            "weight == moderate_threshold buckets Moderate"
        );
        // Below moderate_threshold -> Sparse, even WITH breadth present.
        let below_moderate = cfg.moderate_threshold - 0.5;
        assert!(
            below_moderate < cfg.moderate_threshold,
            "fixture must sit below the moderate cut"
        );
        assert_eq!(
            weight_bucket(below_moderate, 2, 1, 1, &cfg),
            WeightBucket::Sparse,
            "weight below moderate_threshold buckets Sparse even with breadth"
        );
    }
}
