//! `explain` — the per-claim decomposition types (the `--explain` contract).
//!
//! Every aggregate weight decomposes, by construction, into the
//! `Contribution` rows that produced it (I-GRAPH-2 anti-merging in
//! aggregates; the `--explain` running sum reproduces the weight by hand,
//! Gate 2). `author_did` is non-`Option` at the type level: a contribution
//! without attribution is a compile error, not a runtime check.

use claim_domain::authority::{requirement, Authority, Requirement};
use claim_domain::{Cid, Did};
use ports::ScoringFeedRow;

/// One claim's contribution to a pairing's weight — the auditable unit that
/// `--explain` renders. `subtotal` is the value the contribution adds to the
/// pairing weight; the pairing weight is the sum of its contributions'
/// subtotals (Gate 2).
///
/// `PartialEq` (not `Eq`) because the `f64` fields cannot derive `Eq` (NaN).
#[derive(Debug, Clone, PartialEq)]
pub struct Contribution {
    /// LOAD-BEARING: non-`Option` per I-GRAPH-2. The author this contribution
    /// is attributed to; never merged away.
    pub author_did: Did,

    /// The signed claim this contribution maps to (every contribution is one
    /// claim; Gate 5 analog for weights).
    pub cid: Cid,

    /// `= confidence` — the raw numeric `[0.0, 1.0]` scoring input (Gate 6).
    pub base: f64,

    /// `1.0` for the first author on the pairing; raised by
    /// `cfg.author_distinct_bonus` per additional distinct author.
    pub author_distinct_bonus: f64,

    /// `cfg.cross_project_triangulation_bonus` when this author asserts the
    /// object on `>= 2` distinct subjects, else `0.0`.
    pub cross_project_triangulation_bonus: f64,

    /// `= base * <author-distinct multiplier share> + triangulation`. The
    /// pairing weight is the sum of these (Gate 2).
    pub subtotal: f64,
}

impl Contribution {
    /// Read accessor for the load-bearing attribution (the domain stays
    /// immutable; this is the `--explain` renderer's entry point).
    pub fn author_did(&self) -> &Did {
        &self.author_did
    }
}

/// A `github:` assertion this reader does not weigh, with whose word it is
/// (ADR-086 §8): shown apart, never merged into a weighted pairing (C-OOS-6).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LeftOut<'a> {
    pub row: &'a ScoringFeedRow,
    pub authority: Authority,
}

/// Every `github:` assertion in `feed` whose author is not attested for it,
/// in feed order. The complement of the attested rows `score_attested`
/// weighs; claims that need no authority are never left out.
pub fn left_out<'a>(
    feed: &'a [ScoringFeedRow],
    authority_of: impl Fn(&ScoringFeedRow) -> Option<Authority>,
) -> Vec<LeftOut<'a>> {
    feed.iter()
        .filter(|row| needs_authority(row))
        .filter_map(|row| match authority_of(row)? {
            Authority::Attested(_) => None,
            authority => Some(LeftOut { row, authority }),
        })
        .collect()
}

/// Only an assertion about a GitHub account or repo needs authority.
fn needs_authority(row: &ScoringFeedRow) -> bool {
    matches!(
        requirement(row.shape()),
        Requirement::Person(_) | Requirement::Repo { .. }
    )
}

/// The reason an assertion about a malformed `github:` subject (one
/// naming no account or repo) is not counted: no one can be attested for it.
pub const MALFORMED_SUBJECT_REASON: &str = "malformed GitHub subject";

/// How many claims the weighted view left out, and why: one count per reader
/// label, in a fixed order (`third-party`, `authority not checked`, then
/// [`MALFORMED_SUBJECT_REASON`]), reasons with no claim omitted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotCounted {
    pub total: usize,
    pub by_reason: Vec<(&'static str, usize)>,
}

/// Count `left_out` per reason (the reader label), and the assertions of
/// `feed` about a malformed `github:` subject.
pub fn not_counted(left_out: &[LeftOut<'_>], feed: &[ScoringFeedRow]) -> NotCounted {
    let labelled = [
        claim_domain::authority::THIRD_PARTY_LABEL,
        claim_domain::authority::NOT_CHECKED_LABEL,
    ]
    .map(|reason| {
        let count = left_out
            .iter()
            .filter(|claim| claim.authority.label() == reason)
            .count();
        (reason, count)
    });
    let malformed = feed
        .iter()
        .filter(|row| requirement(row.shape()) == Requirement::Malformed)
        .count();
    let by_reason: Vec<(&'static str, usize)> = labelled
        .into_iter()
        .chain([(MALFORMED_SUBJECT_REASON, malformed)])
        .filter(|(_, count)| *count > 0)
        .collect();
    NotCounted {
        total: left_out.len() + malformed,
        by_reason,
    }
}

#[cfg(test)]
mod not_counted_tests {
    use chrono::{TimeZone, Utc};
    use claim_domain::authority::{AttestedTier, Unattested};
    use claim_domain::{Cid, Did};
    use ports::{AttributedClaim, AuthorRelationship};

    use super::*;

    fn row(cid: &str, subject: &str) -> ScoringFeedRow {
        ScoringFeedRow {
            claim: AttributedClaim {
                author_did: Did("did:plc:rachel".to_string()),
                cid: Cid(cid.to_string()),
                subject: subject.to_string(),
                predicate: "embodiesPhilosophy".to_string(),
                object: "org.openlore.philosophy.memory-safety".to_string(),
                confidence: 0.6,
                composed_at: Utc.timestamp_opt(1_700_000_000, 0).unwrap(),
                relationship: AuthorRelationship::SubscribedPeer,
            },
            reference_types: vec![],
        }
    }

    /// Rachel is third-party to every `github:` subject except `attested`.
    fn rachels_standing(row: &ScoringFeedRow) -> Option<Authority> {
        match row.claim.cid.0.as_str() {
            "attested" => Some(Authority::Attested(AttestedTier::RepoListed)),
            "malformed" | "malformed-empty" => None,
            _ => Some(Authority::Unattested(Unattested::NotRepoOwner)),
        }
    }

    fn feed() -> Vec<ScoringFeedRow> {
        vec![
            row("attested", "github:acme-tools/anvil"),
            row("third-party", "github:jeffbailey/tidepool"),
            row("malformed", "github:acme-tools/anvil/extra"),
            row("malformed-empty", "github:"),
        ]
    }

    #[test]
    fn a_claim_about_a_malformed_github_subject_is_disclosed_as_not_counted() {
        let feed = feed();
        assert_eq!(
            not_counted(&left_out(&feed, rachels_standing), &feed),
            NotCounted {
                total: 3,
                by_reason: vec![
                    (claim_domain::authority::THIRD_PARTY_LABEL, 1),
                    (MALFORMED_SUBJECT_REASON, 2),
                ],
            }
        );
    }

    #[test]
    fn with_no_malformed_subject_the_reasons_are_the_reader_labels_alone() {
        let feed: Vec<ScoringFeedRow> = feed()
            .into_iter()
            .filter(|row| !row.claim.cid.0.starts_with("malformed"))
            .collect();
        assert_eq!(
            not_counted(&left_out(&feed, rachels_standing), &feed),
            NotCounted {
                total: 1,
                by_reason: vec![(claim_domain::authority::THIRD_PARTY_LABEL, 1)],
            }
        );
    }

    /// Claims needing no authority are never left out, whatever the
    /// reader's view says of them.
    #[test]
    fn a_claim_needing_no_authority_is_never_left_out() {
        let feed = vec![
            row("exempt", "https:example.com"),
            row("third-party", "github:jeffbailey/tidepool"),
        ];
        let left: Vec<&str> = left_out(&feed, rachels_standing)
            .iter()
            .map(|claim| claim.row.claim.cid.0.as_str())
            .collect();
        assert_eq!(left, vec!["third-party"]);
    }
}
