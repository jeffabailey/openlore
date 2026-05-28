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
    score, AttributedClaim, EmptyContributions, WeightBucket, WeightedPairing, WeightedView,
};

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
}
