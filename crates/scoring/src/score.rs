//! `score` — the pure scoring entry point + its input/output ADTs.
//!
//! `score(claims, cfg) -> WeightedView` is the single, deterministic entry
//! point. The aggregation (the weight) happens HERE, in pure Rust, NOT in SQL
//! — that is what keeps the aggregate decomposable into its `Contribution`
//! rows (I-GRAPH-2). The body is intentionally minimal for the 01-01
//! bootstrap; the Phase 02 SC scenarios drive the formula fully (Gate 2/3/6).

use chrono::{DateTime, Utc};
use claim_domain::{Cid, Did};
use ports::AuthorRelationship;

use crate::config::ScoringConfig;
use crate::explain::Contribution;

/// A fully-attributed claim — the boundary value the pure scoring core
/// consumes. Mirrors the slice-03 `FederatedRow` non-`Option<Did>` discipline
/// that makes attribution unviolatable (Gate 1).
///
/// `PartialEq` (not `Eq`) because `confidence: f64` cannot derive `Eq` (NaN).
#[derive(Debug, Clone, PartialEq)]
pub struct AttributedClaim {
    /// LOAD-BEARING: non-`Option` (anti-merging, I-GRAPH-2).
    pub author_did: Did,
    pub cid: Cid,
    pub subject: String,
    pub predicate: String,
    pub object: String,
    /// Numeric `[0.0, 1.0]` — the scoring input; the value shown equals the
    /// value scored (Gate 6).
    pub confidence: f64,
    pub composed_at: DateTime<Utc>,
    /// `You | SubscribedPeer | UnsubscribedCache` (slice-03 reuse).
    pub relationship: AuthorRelationship,
}

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
/// Bootstrap skeleton (01-01): the formula, ranking, bucketing, and bonus
/// apportionment are driven by the Phase 02 SC acceptance scenarios
/// (Gate 2/3/6). This step lands the signature + the ADTs only.
pub fn score(_claims: &[AttributedClaim], _cfg: &ScoringConfig) -> WeightedView {
    todo!("scoring formula lands in Phase 02 (Gate 2/3/6 SC scenarios)")
}
