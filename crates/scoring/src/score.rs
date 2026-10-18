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
    let scorable = without_identity_links(claims);
    let triangulated = triangulated_author_objects(&scorable);

    let mut ranked: Vec<WeightedPairing> = group_by_pairing(&scorable)
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

/// Drop every identity-link row before grouping AND before triangulation
/// (ADR-085 §6, GDL-D8): a link is display-only, so it never forms a pairing
/// and never counts toward an author's cross-project span. The predicate comes
/// from `claim_domain::identity_link` (Q-RM-5); the match is exact (CORE-7).
/// A link's retraction is reference-only (U-39, ADR-008: empty subject,
/// predicate and object), so it names no pairing and is dropped too.
fn without_identity_links(claims: &[AttributedClaim]) -> Vec<&AttributedClaim> {
    claims
        .iter()
        .filter(|claim| !claim_domain::identity_link::is_reserved_predicate(&claim.predicate))
        .filter(|claim| !is_reference_only(claim))
        .collect()
}

/// A row stating no full triple: a reference-only retraction (U-39). An
/// ordinary retraction restates its claim's triple and is scored as before.
fn is_reference_only(claim: &AttributedClaim) -> bool {
    claim.subject.is_empty() || claim.predicate.is_empty() || claim.object.is_empty()
}

/// Group the flat attributed-claim feed into one bucket per `(subject, object)`
/// pairing. `BTreeMap` keeps the grouping deterministic (no HashMap order leak).
fn group_by_pairing<'a>(
    claims: &[&'a AttributedClaim],
) -> BTreeMap<(String, String), Vec<&'a AttributedClaim>> {
    let mut grouped: BTreeMap<(String, String), Vec<&'a AttributedClaim>> = BTreeMap::new();
    for &claim in claims {
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
fn triangulated_author_objects(claims: &[&AttributedClaim]) -> BTreeSet<(String, String)> {
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

#[cfg(test)]
mod identity_link_drop_tests {
    use chrono::{TimeZone, Utc};
    use claim_domain::identity_link::link_predicate;
    use claim_domain::{Cid, Did};
    use ports::AuthorRelationship;

    use super::*;

    fn claim(cid: &str, subject: &str, predicate: &str, object: &str) -> AttributedClaim {
        AttributedClaim {
            author_did: Did("did:plc:rachel".to_string()),
            cid: Cid(cid.to_string()),
            subject: subject.to_string(),
            predicate: predicate.to_string(),
            object: object.to_string(),
            confidence: 0.6,
            composed_at: Utc.timestamp_opt(1_700_000_000, 0).unwrap(),
            relationship: AuthorRelationship::SubscribedPeer,
        }
    }

    /// A link row sharing an author and object with a real claim must not
    /// lift that claim into cross-project triangulation (drop before
    /// triangulation, ADR-085 §6).
    #[test]
    fn a_link_never_counts_toward_cross_project_triangulation() {
        let real = claim(
            "bafy-claim",
            "github:rust-lang/regex",
            "embodiesPhilosophy",
            "x",
        );
        let link = claim("bafy-link", "did:plc:rachel", link_predicate(), "x");
        let view = score(&[real.clone(), link], &ScoringConfig::DEFAULT);
        assert_eq!(view, score(&[real], &ScoringConfig::DEFAULT));
        assert_eq!(view.ranked[0].cross_project_span, 1);
    }

    /// A link's retraction (U-39) carries only its `retracts` reference: its
    /// subject, predicate and object are empty. It never forms a pairing, so
    /// it never reaches the /score feed, and it changes no other pairing.
    #[test]
    fn a_reference_only_link_retraction_is_never_scored() {
        let real = claim(
            "bafy-claim",
            "github:rust-lang/regex",
            "embodiesPhilosophy",
            "x",
        );
        let retraction = claim("bafy-retraction", "", "", "");
        assert_eq!(
            score(std::slice::from_ref(&retraction), &ScoringConfig::DEFAULT),
            WeightedView::default()
        );
        assert_eq!(
            score(&[real.clone(), retraction], &ScoringConfig::DEFAULT),
            score(&[real], &ScoringConfig::DEFAULT)
        );
        for (subject, predicate, object) in [
            ("", "embodiesPhilosophy", "x"),
            ("github:rust-lang/regex", "", "x"),
            ("github:rust-lang/regex", "embodiesPhilosophy", ""),
        ] {
            let partial = claim("bafy-partial", subject, predicate, object);
            assert!(
                score(&[partial], &ScoringConfig::DEFAULT).ranked.is_empty(),
                "{subject:?} {predicate:?} {object:?}"
            );
        }
    }

    /// Only the exact link predicate is dropped; a near-miss is scored (CORE-7).
    #[test]
    fn a_near_miss_predicate_is_still_scored() {
        let near_misses = [
            link_predicate().to_lowercase(),
            link_predicate().to_uppercase(),
            format!(" {}", link_predicate()),
        ];
        for predicate in near_misses {
            let row = claim("bafy-near", "did:plc:rachel", &predicate, "github:rachelk");
            let view = score(&[row], &ScoringConfig::DEFAULT);
            assert_eq!(view.ranked.len(), 1, "{predicate:?} must be scored");
        }
    }
}
