//! Slice-04 layer-2 acceptance — the pure `scoring` core's transparency +
//! anti-merging-in-aggregates contracts.
//!
//! Layer 2 (in-memory acceptance — pure-core direct invocation, NO CLI
//! subprocess) per nw-tdd-methodology Layered Test Discipline matrix +
//! DD-GRAPH-3. Sibling to slice-02's `scraper_domain.rs` and slice-03's
//! `lexicon_counter_claim.rs`; same shape, same file role. The driving port
//! here is the PURE function signature (`scoring::score` /
//! `scoring::weight_bucket`) — calling it directly IS port-to-port testing
//! at the domain layer (the function signature IS the public interface).
//!
//! Per Mandate 9 (layer-dependent PBT mode): layers 1-2 may use PBT full.
//! The four load-bearing scoring INVARIANTS are `@property` scenarios
//! runnable via proptest:
//!   - SC-1 weight reproducibility: weight == sum(contributions.subtotal)
//!     (Gate 2 `weight_equals_formula`; WD-71/KPI-GRAPH-3 transparency).
//!   - SC-2 weight determinism: same input -> byte-identical WeightedView
//!     (the by-hand reproducibility precondition).
//!   - SC-3 sparse-stays-sparse: a single-author single-claim pairing with
//!     NO cross-project triangulation buckets [SPARSE] regardless of
//!     confidence magnitude (Gate 3 `sparse_renders_sparse`; WD-74/WD-90).
//!   - SC-4 triangulation-increases-weight: a 2-distinct-author pairing
//!     outscores a 1-author pairing at equal max confidence (the monotonic
//!     triangulation property; WD-77 formula).
//! Plus example-pinned scenarios for the anti-merging-in-aggregates type
//! contract (SC-5) and the cross-project-triangulation-counts-as-breadth
//! bucket rule (SC-6; resolves Q-DELIVER-SCORE-1).
//!
//! These are the LOAD-BEARING transparency + anti-merging properties the
//! whole slice-04 thesis rests on (KPI-GRAPH-2 + KPI-GRAPH-3 + KPI-GRAPH-4);
//! pinning them as generative properties at the cheap layer-2 boundary means
//! the example-only layer-3 subprocess tests (`graph_query_explore.rs`) only
//! need to verify the user-visible RENDERING of these already-proven
//! invariants.
//!
//! The EXHAUSTIVE per-arm unit coverage (each bucket threshold boundary, the
//! exact per-claim author-distinct multiplier apportionment, mutation testing
//! of formula.rs + bucket.rs) is DELIVER's inner TDD loop in
//! `crates/scoring/src/`'s `#[cfg(test)] mod tests` block (out of DISTILL
//! scope per DD-GRAPH-7, symmetric with slice-03 DD-FED-7 + slice-02 DD-SCR-7).
//!
//! Covers:
//! - US-GRAPH-003: transparent weighted view (reproducibility + sparse honesty)
//! - US-GRAPH-005: `--explain` per-claim decomposition (anti-merging type contract)
//! - US-GRAPH-006: pure `scoring` core (determinism + reproducibility)
//! - WD-71 / I-GRAPH-1: scoring transparent / no ML (property)
//! - WD-73 / I-GRAPH-2: anti-merging in aggregates (type-level decomposition)
//! - WD-74 / WD-90 / I-GRAPH-4: sparse renders sparse (property)
//! - WD-77 / WD-86: the closed-form formula constants SSOT
//! - Q-DELIVER-SCORE-1: cross-project triangulation counts as breadth (bucket rule)
//
// SCAFFOLD: true

#![allow(dead_code)]
#![allow(unused_imports)]

use chrono::{TimeZone, Utc};
use claim_domain::{Cid, Did};
use ports::{AttributedClaim, AuthorRelationship};
use proptest::prelude::*;
use proptest::test_runner::TestRunner;
use scoring::ScoringConfig;

// NOTE — unlike the subprocess-driven graph_query_explore tests, this file
// invokes `scoring` directly (layer 2). It does NOT use `support/mod.rs`'s
// TestEnv (no subprocess). Same pattern as slice-02's `scraper_domain.rs` and
// slice-03's `lexicon_counter_claim.rs`.
//
// The `scoring` crate + its public ADTs (AttributedClaim, Contribution,
// WeightedPairing, WeightedView, WeightBucket, ScoringConfig) and the pure
// entry points (`score`, `weight_bucket`) are scaffolded by DELIVER's first
// slice-04 step (step-07-01 bootstrap) per component-boundaries.md
// §`crates/scoring`. Until then this file does not compile — that is the
// intended RED-ready state (DD-GRAPH-13): once the crate's types exist with
// `todo!()` bodies, every `#[test]` here reaches its own `todo!()` (RED, not
// BROKEN).

// =============================================================================
// US-GRAPH-003 / US-GRAPH-006 — weight reproducibility (PROPERTY; Gate 2)
// =============================================================================

/// SC-1 / Property (Mandate 9 layer 2 PBT full): for EVERY (subject, object)
/// pairing in the WeightedView, the displayed `weight` EQUALS the sum of its
/// `Contribution.subtotal` values. This IS the by-hand reproduction contract
/// `--explain` renders (Gate 2 `weight_equals_formula`; WD-71/KPI-GRAPH-3).
/// No opaque, ML, or non-reproducible weight is permitted.
///
///     forall claims:
///         score(claims, cfg).ranked.all(|p|
///             p.weight == p.contributions.iter().map(|c| c.subtotal).sum())
///
/// @property @us-graph-003 @us-graph-006 @j-002 @i-graph-1 @kpi-graph-3 @gate-2
#[test]
fn scoring_weight_equals_sum_of_contributions_property() {
    // Layer-2 @property (Mandate 9; DD-GRAPH): pure-core direct invocation, NO
    // CLI subprocess. The driving port IS the pure `scoring::score` signature;
    // we drive it over an arbitrary attributed-claim set and assert the
    // reproducibility invariant from data-models.md §"The scoring formula"
    // (Gate 2 / WD-71 / KPI-GRAPH-3):
    //
    //     forall claims:
    //         score(claims, ScoringConfig::DEFAULT).ranked.all(|p|
    //             (p.weight - p.contributions().iter().map(|c| c.subtotal).sum::<f64>()).abs() < EPS)
    //
    // The contributions list IS the auditable decomposition `--explain` prints;
    // if the displayed weight ever drifts from the sum of the per-claim
    // subtotals, the transparency promise breaks (the user cannot reproduce the
    // number by hand). A future formula refactor that computes the weight by any
    // path OTHER than summing the visible contributions fails LOUDLY here at the
    // cheap layer-2 boundary.
    //
    // Generator: an arbitrary NON-EMPTY Vec<AttributedClaim> over a small
    // bounded universe of {subject in 3, object in 2, author in 3} with
    // confidence in [0.0, 1.0], so the generated sets exercise single-author,
    // multi-author, AND cross-project-triangulation pairings. Forcing >=1 claim
    // makes the WeightedView never empty (an empty ranked list would pass the
    // all(..) vacuously — the non-vacuity guard below asserts ranked non-empty).
    const EPS: f64 = 1e-9;

    let mut runner = TestRunner::default();
    runner
        .run(&arbitrary_attributed_claims(), |claims| {
            let view = scoring::score(&claims, &ScoringConfig::DEFAULT);

            // Non-vacuity: a non-empty claim set yields >=1 ranked pairing, so
            // the per-pairing invariant below is never asserted vacuously.
            prop_assert!(
                !view.ranked.is_empty(),
                "a non-empty attributed-claim set must produce at least one ranked pairing"
            );

            // Gate 2: every displayed weight reproduces by hand as the sum of
            // its per-claim Contribution subtotals (the `--explain` running sum).
            for pairing in &view.ranked {
                let recomputed: f64 = pairing.contributions().iter().map(|c| c.subtotal).sum();
                prop_assert!(
                    (pairing.weight - recomputed).abs() < EPS,
                    "weight {} != sum(subtotals) {} for ({}, {})",
                    pairing.weight,
                    recomputed,
                    pairing.subject,
                    pairing.object
                );
            }
            Ok(())
        })
        .unwrap();
}

/// Generator for a NON-EMPTY `Vec<AttributedClaim>` over a small bounded
/// universe (3 subjects x 2 objects x 3 authors, confidence in `[0.0, 1.0]`)
/// so generated sets exercise single-author, multi-author, and
/// cross-project-triangulation pairings. Used by the SC-1 reproducibility
/// property; sibling properties (SC-2/SC-3/SC-4) reuse it as they activate.
fn arbitrary_attributed_claims() -> impl Strategy<Value = Vec<AttributedClaim>> {
    let subject = prop_oneof![Just("deno"), Just("cargo"), Just("nixpkgs")];
    let object = prop_oneof![Just("dependency-pinning"), Just("immutability")];
    let author = prop_oneof![
        Just("did:plc:tobias"),
        Just("did:plc:maria"),
        Just("did:plc:rachel")
    ];

    let one_claim = (subject, object, author, 0.0_f64..=1.0).prop_map(
        |(subject, object, author, confidence)| AttributedClaim {
            author_did: Did(author.to_string()),
            cid: Cid(format!("bafy-{subject}-{object}-{author}")),
            subject: subject.to_string(),
            predicate: "adheres-to".to_string(),
            object: object.to_string(),
            confidence,
            composed_at: Utc.timestamp_opt(1_700_000_000, 0).unwrap(),
            relationship: AuthorRelationship::You,
        },
    );

    prop::collection::vec(one_claim, 1..12)
}

// =============================================================================
// US-GRAPH-006 — determinism (PROPERTY)
// =============================================================================

/// SC-2 / Property: `scoring::score` is DETERMINISTIC — the same attributed
/// claims + config produce a byte-identical WeightedView (same ranking, same
/// weights, same buckets, same contribution lists). Determinism is the
/// precondition of reproducibility: a weight a user reproduces by hand must
/// be the SAME weight a re-run displays (US-GRAPH-006 UAT scenario 1).
///
///     forall claims:
///         score(claims, cfg) == score(claims, cfg)
///
/// @property @us-graph-006 @j-002 @i-graph-1
#[test]
fn scoring_score_is_deterministic_property() {
    // Layer-2 @property (Mandate 9; DD-GRAPH): pure-core direct invocation. The
    // driving port IS the pure `scoring::score` signature. Determinism is
    // structural in a pure core (no clock, no I/O, no HashMap iteration-order
    // leak in the ranking — stable tiebreak by subject per data-models.md
    // §WeightedView). This property PINS it so a future refactor that
    // introduces a HashMap-ordered ranking (or any nondeterministic tiebreak)
    // fails LOUDLY here, mirroring slice-02's
    // `scraper_domain_derive_candidates_is_deterministic_property`.
    //
    //     forall claims:
    //         score(claims, ScoringConfig::DEFAULT) == score(claims, ScoringConfig::DEFAULT)
    //
    // `WeightedView` derives `PartialEq` (over its ranked `WeightedPairing`s,
    // their `f64` weights, buckets, and contribution lists), so this byte-for-
    // byte equality covers ranking ORDER, weights, buckets, AND the per-claim
    // decomposition. Symmetric-property style (Hebert ch.3 Tier 1): applying the
    // same pure transformation twice yields the same value. The generator
    // `arbitrary_attributed_claims()` (reused from SC-1) draws over the bounded
    // {3 subjects x 2 objects x 3 authors} universe, so the generated sets
    // exercise single-author, multi-author, AND cross-project-triangulation
    // pairings — the determinism invariant must hold across every grouping
    // shape. 02-01 grouped via `BTreeMap` (stable iteration) with a stable
    // weight-desc / subject / object tiebreak sort; this property pins that
    // there is no nondeterministic ordering or NaN-driven tiebreak left.
    let mut runner = TestRunner::default();
    runner
        .run(&arbitrary_attributed_claims(), |claims| {
            let first = scoring::score(&claims, &ScoringConfig::DEFAULT);
            let second = scoring::score(&claims, &ScoringConfig::DEFAULT);
            prop_assert_eq!(
                first,
                second,
                "scoring::score must be DETERMINISTIC: the same attributed claims + config must \
                 yield a byte-identical WeightedView (same ranking ORDER, same weights, same \
                 buckets, same contribution lists) — the by-hand reproducibility precondition \
                 (US-GRAPH-006). A weight a user reproduces by hand must be the SAME weight a \
                 re-run displays."
            );
            Ok(())
        })
        .expect(
            "determinism invariant: score(claims, ScoringConfig::DEFAULT) must equal a second \
             call with the SAME inputs for all generated attributed-claim sets",
        );
}

// =============================================================================
// US-GRAPH-003 — sparse-stays-sparse (PROPERTY; Gate 3 / WD-74 / WD-90)
// =============================================================================

/// SC-3 / Property: a (subject, object) pairing backed by a SINGLE claim from
/// a SINGLE author with NO cross-project triangulation buckets [SPARSE]
/// REGARDLESS of the claim's confidence magnitude — even at confidence 0.99.
/// The breadth guard (WD-90) makes thin evidence look thin; a single
/// high-confidence opinion must never be dressed up as [STRONG]. This is the
/// direct mitigation of the J-002 sparse-data anxiety (Gate 3
/// `sparse_renders_sparse`; KPI-GRAPH-4 guardrail).
///
///     forall confidence in [0.0, 1.0]:
///         weight_bucket(score(single_author_single_claim(confidence))) == Sparse
///
/// @property @us-graph-003 @j-002 @i-graph-4 @kpi-graph-4 @gate-3 @wd-90
#[test]
fn scoring_single_author_single_claim_is_sparse_at_any_confidence_property() {
    // Layer-2 @property (Mandate 9; DD-GRAPH): pure-core direct invocation. The
    // driving port IS `scoring::score` (+ the `weight_bucket` it annotates each
    // pairing with). The breadth-guard invariant (WD-74 / WD-90 / I-GRAPH-4):
    //
    //     forall confidence in [0.0, 1.0]:
    //         let view = score(&[one_claim(subject, object, author, confidence)], cfg);
    //         view.ranked[0].bucket == WeightBucket::Sparse
    //
    // claim_count == 1 AND distinct_author_count == 1 AND no cross-project span
    // => Sparse, independent of `weight` (which scales with confidence). The
    // negative is LOAD-BEARING: a single 0.99-confidence claim bucketed
    // [STRONG] would manufacture confidence from thin evidence (the exact J-002
    // failure). Generator: confidence drawn from the full [0.0, 1.0] range so
    // the property covers the boundary where a naive weight-only threshold
    // would flip the bucket. The single-claim fixture has ONE author on ONE
    // subject (no second subject for the same author => no triangulation
    // breadth, per the Q-DELIVER-SCORE-1 rule pinned in SC-6).
    let mut runner = TestRunner::default();
    runner
        .run(&(0.0_f64..=1.0), |confidence| {
            // A SINGLE claim: one author, one subject, one object — no co-author,
            // no second subject for the same author (no cross-project span). The
            // ONLY varying input is the confidence magnitude.
            let claims = vec![AttributedClaim {
                author_did: Did("did:plc:tobias".to_string()),
                cid: Cid("bafy-sparse-single".to_string()),
                subject: "deno".to_string(),
                predicate: "adheres-to".to_string(),
                object: "dependency-pinning".to_string(),
                confidence,
                composed_at: Utc.timestamp_opt(1_700_000_000, 0).unwrap(),
                relationship: AuthorRelationship::You,
            }];

            let view = scoring::score(&claims, &ScoringConfig::DEFAULT);

            // Non-vacuity: the single claim yields exactly one ranked pairing, so
            // the bucket assertion below is never asserted vacuously.
            prop_assert_eq!(
                view.ranked.len(),
                1,
                "a single attributed claim must produce exactly one ranked pairing"
            );

            let pairing = &view.ranked[0];

            // The single-author single-claim no-span pairing has NO evidence
            // breadth, so it MUST bucket Sparse regardless of how high the
            // confidence (hence the weight) climbs — even at 0.99 the breadth
            // guard keeps thin evidence thin (Gate 3 sparse_renders_sparse;
            // WD-74 / WD-90). Manufacturing [STRONG] from a lone high-confidence
            // opinion is the exact J-002 failure this guard prevents.
            prop_assert_eq!(
                pairing.bucket,
                scoring::WeightBucket::Sparse,
                "a single-author single-claim pairing with no cross-project span must bucket \
                 Sparse at confidence {} (weight {}); breadth — not raw confidence — lifts a \
                 pairing out of Sparse (WD-74/WD-90; Gate 3)",
                confidence,
                pairing.weight
            );
            Ok(())
        })
        .expect(
            "sparse-renders-sparse invariant: a single-author single-claim pairing with no \
             cross-project span must bucket WeightBucket::Sparse for every confidence in \
             [0.0, 1.0] (Gate 3 sparse_renders_sparse; WD-74/WD-90; KPI-GRAPH-4)",
        );
}

// =============================================================================
// US-GRAPH-003 — triangulation-increases-weight (PROPERTY; WD-77 formula)
// =============================================================================

/// SC-4 / Property: a (subject, object) pairing supported by TWO distinct
/// authors scores STRICTLY HIGHER than an otherwise-identical pairing
/// supported by ONE author at the same max confidence. Triangulation
/// (multi-author support) monotonically raises the adherence weight per the
/// WD-77 formula's `+author_distinct_bonus per additional distinct author`.
/// This is the monotonicity property that makes the ranking meaningful — more
/// independent support => higher rank.
///
///     forall confidence in [0.0, 1.0]:
///         weight(two_distinct_authors(confidence)) > weight(one_author(confidence))
///
/// @property @us-graph-003 @j-002 @kpi-graph-1 @wd-77
#[test]
fn scoring_multi_author_outweighs_single_author_at_equal_confidence_property() {
    // SCAFFOLD: true
    //
    // Layer-2 @property (Mandate 9; DD-GRAPH): pure-core direct invocation. The
    // driving port IS `scoring::score`. The triangulation-monotonicity
    // invariant (WD-77 author_distinct_bonus):
    //
    //     forall confidence in [0.0, 1.0]:
    //         let one = score(&[claim(S, O, author_a, confidence)], cfg);
    //         let two = score(&[claim(S, O, author_a, confidence),
    //                          claim(S, O, author_b, confidence)], cfg);
    //         two.ranked[0].weight > one.ranked[0].weight
    //
    // Two distinct authors on the SAME (subject, object) apply the
    // +author_distinct_bonus multiplier, so the multi-author weight strictly
    // exceeds the single-author weight at equal confidence. This is the formula
    // property that gives triangulation its meaning (KPI-GRAPH-1 connection
    // discovery: better-triangulated support ranks higher). Generator:
    // confidence in (0.0, 1.0] (a strictly-positive base so the bonus is
    // observable; at confidence 0 both weights are 0 and the strict inequality
    // does not hold — the generator excludes the degenerate 0.0 case or the
    // assertion uses >= with a documented note).
    todo!(
        "DELIVER (slice-04): proptest confidence in (0.0,1.0]; assert a 2-distinct-author \
         pairing scores strictly higher than a 1-author pairing at equal max confidence \
         (triangulation monotonicity; WD-77; KPI-GRAPH-1)"
    )
}

// =============================================================================
// US-GRAPH-005 / US-GRAPH-006 — anti-merging-in-aggregates type contract (example)
// =============================================================================

/// SC-5 (Gate 1 type-level `scoring_aggregate_preserves_attribution`): a
/// WeightedPairing produced from claims by two distinct authors decomposes to
/// EXACTLY two Contributions, each carrying its own non-`Option` `author_did`
/// + `cid`. There is NO API that returns a bare weight without its
/// contributions; the aggregate decomposes BY CONSTRUCTION. This is the
/// type-level layer of the three-layer anti-merging-in-aggregates enforcement
/// (WD-73 / WD-88 / I-GRAPH-2; extends slice-03 I-FED-1 to aggregates).
///
/// Given two attributed claims on github:denoland/deno (Tobias 0.55, Maria
/// 0.40) for object dependency-pinning; When score runs; Then the deno
/// WeightedPairing has exactly 2 contributions, one attributed to Tobias's
/// DID+CID and one to Maria's DID+CID, and weight == sum of their subtotals.
///
/// @us-graph-005 @us-graph-006 @j-002 @i-graph-2 @kpi-graph-2 @gate-1 @anti-merging
#[test]
fn scoring_two_author_pairing_decomposes_to_two_attributed_contributions() {
    // SCAFFOLD: true
    //
    // Layer-2 example (Mandate 9; DD-GRAPH): pure-core direct invocation. The
    // driving port IS `scoring::score`. Example-pinned (not a property) per the
    // file header: the anti-merging decomposition is documented by the
    // worked-arithmetic fixture from US-GRAPH-005 Example 1 / data-models.md
    // §"Worked example (deno)":
    //
    //   deno / dependency-pinning has 2 contributing claims:
    //     Tobias (bafy...d3no, conf 0.55) -> subtotal 0.55
    //     Maria  (bafy...mz01, conf 0.40) -> +0.25 second-author bonus -> subtotal 0.50
    //     weight = 0.55 + 0.50 = 1.05; distinct_authors = 2 -> bucket Moderate
    //
    // The assertion pins: contributions.len() == 2; each Contribution.author_did
    // is the right non-empty DID; each carries its own cid; and the pairing's
    // weight equals the sum of the two subtotals (the decomposition is exact).
    // This is the type-level + behavioral layer-2 proof that the aggregate can
    // ALWAYS enumerate its individually-attributed claims (no faceless
    // consensus weight). The structural xtask SQL-string layer + the layer-3
    // subprocess --explain rendering complete the three-layer enforcement.
    todo!(
        "DELIVER (slice-04): score the two-author deno/dependency-pinning fixture; assert the \
         WeightedPairing has exactly 2 contributions each with its own non-empty author_did + \
         cid, and weight == sum(subtotals) (Gate 1 type-level; WD-73/WD-88/I-GRAPH-2)"
    )
}

// =============================================================================
// US-GRAPH-003 — cross-project-triangulation-counts-as-breadth (example; SCORE-1)
// =============================================================================

/// SC-6 (resolves Q-DELIVER-SCORE-1 / WD-90 bucket-rule lock): a SINGLE-claim
/// pairing whose author asserts the SAME object on >=2 distinct subjects
/// (cross-project triangulation) is NOT [SPARSE] — the cross-project span
/// counts toward evidence breadth for the bucket. Contrast SC-3: a single
/// claim with NO triangulation and NO co-author STAYS [SPARSE] regardless of
/// confidence. This pins the one consistent bucket rule against the
/// data-models.md worked examples (cargo [STRONG] via Rachel's cargo+nixpkgs
/// span; tokio [SPARSE] single-claim-no-span).
///
/// Given cargo has 1 dependency-pinning claim by Rachel (conf 0.91) AND Rachel
/// also asserts dependency-pinning on nixpkgs (a 2nd distinct subject); When
/// score runs; Then cargo's pairing is NOT bucketed Sparse (the cross-project
/// span is breadth), while a tokio pairing with 1 claim by 1 author and no
/// span IS bucketed Sparse.
///
/// @us-graph-003 @j-002 @i-graph-4 @wd-90 @score-1 @bucket-rule
#[test]
fn scoring_cross_project_triangulation_counts_as_breadth_lifts_out_of_sparse() {
    // SCAFFOLD: true
    //
    // Layer-2 example (Mandate 9; DD-GRAPH): pure-core direct invocation. The
    // driving port IS `scoring::score`. This is the DISTILL-confirmed resolution
    // of Q-DELIVER-SCORE-1 (the `# DISTILL: confirm` flag in DESIGN
    // wave-decisions.md): the worked examples narrate cargo as [STRONG] "boosted
    // by Rachel spanning cargo+nixpkgs" (US-GRAPH-003 Example 1) AND require a
    // single-claim no-span pairing to stay [SPARSE] (US-GRAPH-003 Example 2 /
    // SC-3 above). The ONE consistent rule (WD-90):
    //
    //   cross-project triangulation by the SAME author counts toward evidence
    //   breadth for the bucket => a triangulated single-claim pairing is NOT
    //   Sparse; a single claim with NO triangulation AND NO co-author STAYS
    //   Sparse regardless of confidence magnitude.
    //
    // Fixture (data-models.md §"Worked example — triangulation, cargo"):
    //   cargo / dependency-pinning: 1 claim by Rachel (conf 0.91)
    //     Rachel ALSO asserts dependency-pinning on nixpkgs (2nd subject)
    //     -> cross_project_triangulation_bonus applies -> NOT Sparse
    //   tokio / actor-model: 1 claim by 1 author, no span -> Sparse (the SC-3 leg)
    //
    // The assertion pins: cargo.bucket != Sparse AND tokio.bucket == Sparse,
    // proving the bucket function takes cross-project span as a breadth input
    // (not just `weight` magnitude). DELIVER picks the exact [STRONG]-vs-
    // [MODERATE] threshold for cargo within WD-86's tunable constants; DISTILL
    // asserts only the NOT-Sparse half (the load-bearing SCORE-1 contract).
    todo!(
        "DELIVER (slice-04): score a cross-project-triangulated single-claim cargo pairing \
         (Rachel spans cargo+nixpkgs) AND a no-span single-claim tokio pairing; assert cargo \
         is NOT Sparse while tokio IS Sparse (Q-DELIVER-SCORE-1 / WD-90 bucket rule)"
    )
}
