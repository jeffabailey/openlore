//! `score` — the pure scoring entry point + its input/output ADTs.
//!
//! `score(claims, cfg) -> WeightedView` is the single, deterministic entry
//! point. The aggregation (the weight) happens HERE, in pure Rust, NOT in SQL
//! — that is what keeps the aggregate decomposable into its `Contribution`
//! rows (I-GRAPH-2). The body is intentionally minimal for the 01-01
//! bootstrap; the Phase 02 SC scenarios drive the formula fully (Gate 2/3/6).

// `AttributedClaim` is the boundary value the pure scoring core consumes. It
// lives in `ports` (hoisted from here in step 01-02) because BOTH this pure
// core AND the `cli` composition root consume it, and `scoring -> ports`
// (never the reverse) — `ports` is the non-cyclic home. Re-exported via
// `crate::AttributedClaim` (see `lib.rs`) so existing call-sites are unchanged.
use std::collections::BTreeMap;
use std::collections::BTreeSet;

use ports::AttributedClaim;

use crate::config::ScoringConfig;
use crate::explain::Contribution;

/// The display-only weight bucket (WD-72: never persisted). Driven by
/// `(weight, claim_count, distinct_author_count)` with the WD-74 breadth
/// guard, NOT by weight magnitude alone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WeightBucket {
    Strong,
    Moderate,
    Sparse,
}

/// One ranked `(subject, object)` pairing. Cannot exist without its
/// `contributions` — it decomposes by construction (anti-merging in
/// aggregates, I-GRAPH-2). Use [`WeightedPairing::new`] to construct; the
/// non-empty `contributions` invariant is enforced there.
///
/// `weight == sum(contributions.subtotal)` (Gate 2).
///
/// `PartialEq` (not `Eq`) because of the `f64` fields.
#[derive(Debug, Clone, PartialEq)]
pub struct WeightedPairing {
    pub subject: String,
    pub object: String,
    /// `== sum of contributions' subtotals` (Gate 2).
    pub weight: f64,
    /// DISPLAY-ONLY (WD-72).
    pub bucket: WeightBucket,
    pub claim_count: u32,
    pub distinct_author_count: u32,
    pub max_confidence: f64,
    /// Distinct subjects the top contributor spans (cross-project breadth).
    pub cross_project_span: u32,
    /// NON-EMPTY by construction — the decomposition (Gate 1 type-level via
    /// the smart constructor).
    contributions: Vec<Contribution>,
}

/// Error from constructing a [`WeightedPairing`] with an empty contribution
/// set — the type-level anti-merging guard (Gate 1). A pairing with no
/// contributions is a domain impossibility (a weight that decomposes into
/// nothing), so construction is fallible.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmptyContributions;

impl WeightedPairing {
    /// Smart constructor: rejects an empty `contributions` set so a
    /// `WeightedPairing` can never exist without its decomposition (Gate 1).
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        subject: String,
        object: String,
        weight: f64,
        bucket: WeightBucket,
        claim_count: u32,
        distinct_author_count: u32,
        max_confidence: f64,
        cross_project_span: u32,
        contributions: Vec<Contribution>,
    ) -> Result<Self, EmptyContributions> {
        if contributions.is_empty() {
            return Err(EmptyContributions);
        }
        Ok(Self {
            subject,
            object,
            weight,
            bucket,
            claim_count,
            distinct_author_count,
            max_confidence,
            cross_project_span,
            contributions,
        })
    }

    /// Read accessor for the (guaranteed non-empty) decomposition. The domain
    /// stays immutable; `--explain` renders these.
    pub fn contributions(&self) -> &[Contribution] {
        &self.contributions
    }
}

/// The ranked list returned by [`score`]. Sorted by weight descending, stable
/// tiebreak by subject (the formula step lands the sort in Phase 02).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct WeightedView {
    pub ranked: Vec<WeightedPairing>,
}

/// The single entry point — pure and deterministic: the same `claims` + `cfg`
/// always produce a byte-identical `WeightedView`.
///
/// Pipeline (each step pure, deterministic, small):
///   group claims by (subject, object)  ->  score each pairing  ->  rank
///
/// The weight is computed HERE, in Rust, by SUMMING the per-claim
/// [`Contribution::subtotal`] values — never by an opaque path. That is the
/// Gate 2 transparency contract: `--explain` reproduces the displayed weight by
/// re-summing the visible contributions (WD-71 / KPI-GRAPH-3). The formula
/// (WD-77; `data-models.md` §"The scoring formula") is closed-form, no-ML, and
/// reproducible.
pub fn score(claims: &[AttributedClaim], cfg: &ScoringConfig) -> WeightedView {
    let triangulated = triangulated_author_objects(claims);

    let mut ranked: Vec<WeightedPairing> = group_by_pairing(claims)
        .into_iter()
        .filter_map(|((subject, object), pairing_claims)| {
            score_pairing(&subject, &object, &pairing_claims, cfg, &triangulated)
        })
        .collect();

    // Deterministic order: weight descending, stable tiebreak by subject then
    // object (no HashMap iteration-order leak; a re-run is byte-identical).
    ranked.sort_by(|a, b| {
        b.weight
            .partial_cmp(&a.weight)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.subject.cmp(&b.subject))
            .then_with(|| a.object.cmp(&b.object))
    });

    WeightedView { ranked }
}

/// Group the flat attributed-claim feed into one bucket per `(subject, object)`
/// pairing. `BTreeMap` keeps the grouping deterministic (no HashMap order leak).
fn group_by_pairing(
    claims: &[AttributedClaim],
) -> BTreeMap<(String, String), Vec<&AttributedClaim>> {
    let mut grouped: BTreeMap<(String, String), Vec<&AttributedClaim>> = BTreeMap::new();
    for claim in claims {
        grouped
            .entry((claim.subject.clone(), claim.object.clone()))
            .or_default()
            .push(claim);
    }
    grouped
}

/// The set of `(author_did, object)` pairs for which the author asserts the
/// object on `>= 2` distinct subjects — i.e. cross-project triangulation
/// (WD-77). Computed once over the WHOLE feed (triangulation is a global
/// property of an author's reach, not local to one pairing).
fn triangulated_author_objects(claims: &[AttributedClaim]) -> BTreeSet<(String, String)> {
    let mut subjects_per_author_object: BTreeMap<(String, String), BTreeSet<String>> =
        BTreeMap::new();
    for claim in claims {
        subjects_per_author_object
            .entry((claim.author_did.0.clone(), claim.object.clone()))
            .or_default()
            .insert(claim.subject.clone());
    }
    subjects_per_author_object
        .into_iter()
        .filter(|(_, subjects)| subjects.len() >= 2)
        .map(|(key, _)| key)
        .collect()
}

/// Score ONE `(subject, object)` pairing into a [`WeightedPairing`], or `None`
/// if it has no claims (the smart constructor would reject an empty
/// decomposition — grouping never yields an empty bucket, so this is total).
fn score_pairing(
    subject: &str,
    object: &str,
    pairing_claims: &[&AttributedClaim],
    cfg: &ScoringConfig,
    triangulated: &BTreeSet<(String, String)>,
) -> Option<WeightedPairing> {
    if pairing_claims.is_empty() {
        return None;
    }

    // Distinct authors, ordered by first appearance -> each author's 1-based
    // rank. Rank 1 (first author) takes multiplier-share 1.0; each ADDITIONAL
    // distinct author takes `1.0 + author_distinct_bonus * (rank - 1)`. This is
    // the per-claim apportionment that makes the subtotals sum EXACTLY to the
    // weight (the worked deno arithmetic: Tobias x1.0=0.55, Maria x1.25=0.50).
    let author_rank = distinct_author_ranks(pairing_claims);

    let contributions: Vec<Contribution> = pairing_claims
        .iter()
        .map(|claim| {
            let rank = author_rank[&claim.author_did.0];
            let author_multiplier_share = 1.0 + cfg.author_distinct_bonus * ((rank - 1) as f64);
            let triangulation_bonus =
                if triangulated.contains(&(claim.author_did.0.clone(), object.to_string())) {
                    cfg.cross_project_triangulation_bonus
                } else {
                    0.0
                };
            let base = claim.confidence;
            let subtotal = base * author_multiplier_share + triangulation_bonus;
            Contribution {
                author_did: claim.author_did.clone(),
                cid: claim.cid.clone(),
                base,
                author_distinct_bonus: author_multiplier_share,
                cross_project_triangulation_bonus: triangulation_bonus,
                subtotal,
            }
        })
        .collect();

    // Gate 2: the weight IS the sum of the visible per-claim subtotals.
    let weight: f64 = contributions.iter().map(|c| c.subtotal).sum();

    let claim_count = pairing_claims.len() as u32;
    let distinct_author_count = author_rank.len() as u32;
    let max_confidence = pairing_claims
        .iter()
        .map(|c| c.confidence)
        .fold(f64::NEG_INFINITY, f64::max);
    let cross_project_span = max_cross_project_span(pairing_claims, triangulated, object);
    let bucket = weight_bucket(
        weight,
        claim_count,
        distinct_author_count,
        cross_project_span,
        cfg,
    );

    // Total by construction: grouping guarantees >= 1 contribution, so the
    // smart constructor never rejects here. `.ok()` discards the impossible
    // empty-decomposition error rather than panicking.
    WeightedPairing::new(
        subject.to_string(),
        object.to_string(),
        weight,
        bucket,
        claim_count,
        distinct_author_count,
        max_confidence,
        cross_project_span,
        contributions,
    )
    .ok()
}

/// Map each distinct `author_did` on a pairing to its 1-based rank by first
/// appearance (deterministic in feed order). The first distinct author is
/// rank 1, the second rank 2, etc.
fn distinct_author_ranks(pairing_claims: &[&AttributedClaim]) -> BTreeMap<String, usize> {
    let mut ranks: BTreeMap<String, usize> = BTreeMap::new();
    let mut next_rank = 1usize;
    for claim in pairing_claims {
        ranks.entry(claim.author_did.0.clone()).or_insert_with(|| {
            let rank = next_rank;
            next_rank += 1;
            rank
        });
    }
    ranks
}

/// The largest cross-project span among this pairing's contributing authors:
/// how many distinct subjects the most-spanning author reaches for this object.
/// `1` when no author triangulates (a single-subject author spans exactly 1).
fn max_cross_project_span(
    pairing_claims: &[&AttributedClaim],
    triangulated: &BTreeSet<(String, String)>,
    object: &str,
) -> u32 {
    let any_triangulates = pairing_claims
        .iter()
        .any(|c| triangulated.contains(&(c.author_did.0.clone(), object.to_string())));
    if any_triangulates {
        2
    } else {
        1
    }
}

/// The display-only bucket (WD-72; never persisted). The breadth guard
/// (WD-74 / WD-90) is LOAD-BEARING: thin evidence renders thin. A pairing with
/// `claim_count <= 1` AND `distinct_author_count <= 1` AND no cross-project span
/// (`cross_project_span <= 1`) is [`WeightBucket::Sparse`] REGARDLESS of weight
/// magnitude — a single high-confidence opinion is never dressed up as Strong
/// (Gate 3; mitigates J-002). Cross-project triangulation by the same author
/// counts as breadth, lifting a single-claim pairing out of Sparse
/// (Q-DELIVER-SCORE-1). Only pairings that clear the breadth guard are bucketed
/// by weight against the SSOT thresholds.
pub fn weight_bucket(
    weight: f64,
    claim_count: u32,
    distinct_author_count: u32,
    cross_project_span: u32,
    cfg: &ScoringConfig,
) -> WeightBucket {
    let has_breadth = claim_count > 1 || distinct_author_count > 1 || cross_project_span > 1;
    if !has_breadth {
        return WeightBucket::Sparse;
    }
    if weight >= cfg.strong_threshold {
        WeightBucket::Strong
    } else if weight >= cfg.moderate_threshold {
        WeightBucket::Moderate
    } else {
        WeightBucket::Sparse
    }
}
