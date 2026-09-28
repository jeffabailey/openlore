//! One `infer people` run as pure data (DDD-8 / DDD-13 / DDD-14): filters
//! applied before numbering, each candidate placed against my standing
//! adherence claims, the unused-repos footer, and weakened support.

use std::collections::{BTreeMap, BTreeSet};

use claim_domain::{
    is_self_retracted, is_superseded_by_author, ClaimLineage, ClaimReference, ReferenceType,
};
use ports::ContributionLink;

use super::candidate::{PersonCandidate, RepoClaim};
use super::classify::{classify, Classification};
use super::inference::{infer_person_candidates, repos_without_signed_claims};
use super::subject::PersonSubject;
use super::weakened::{weakened_claims, WeakenedClaim};
use super::{pair_key, subject_key, PairKey, ADHERES_TO_PHILOSOPHY};

/// The DDD-13 filters of one `infer people` run, applied BEFORE numbering
/// (UC-8) so the list and `--sign` number the same candidates.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InferenceFilter {
    /// Scope to one person (compared case-insensitively).
    pub person: Option<PersonSubject>,
    /// Keep only candidates supported by at least this many repos.
    pub min_repos: usize,
}

/// My standing adherence claims, grouped by the pair they are about.
pub(super) type StandingByPair<'a> = BTreeMap<PairKey, Vec<&'a OwnClaim>>;

/// One of MY signed claims, as the already-signed / STRONGER classification
/// (DDD-8) sees it: its evidence carries the cited AT-URIs (ADR-064 §5) and
/// its `composed_at` orders several current claims for one pair (Q-CPI-D4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnClaim {
    pub subject: String,
    pub predicate: String,
    pub object: String,
    pub author_did: String,
    pub cid: String,
    pub evidence: Vec<String>,
    /// RFC3339 UTC as the one clock port writes it, so lexical order is
    /// chronological (the invariant `latest_composed` relies on).
    pub composed_at: String,
    pub references: Vec<ClaimReference>,
}

/// An inferred (person, philosophy) I already signed: shown, never numbered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AlreadySigned {
    pub candidate: PersonCandidate,
    /// The CID of my standing adherence claim for the pair.
    pub cid: String,
}

/// Why a numbered candidate is proposed (DDD-8).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CandidateStatus {
    /// No standing adherence of mine for the pair.
    New,
    /// My latest standing INFERRED claim for the pair leaves a currently
    /// supporting repo uncited; signing adds `supersedes <cid>` to a NEW claim
    /// and leaves that claim unchanged (D-5).
    Stronger { supersedes: String },
}

/// A numbered candidate and why it is proposed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NumberedCandidate {
    pub candidate: PersonCandidate,
    pub status: CandidateStatus,
}

/// One `infer people` run as pure data: the numbered candidates, the
/// inferred pairs I already signed (unnumbered, DDD-8), plus the linked repos
/// that contributed nothing because no signed philosophy claim is about them
/// (D-2 / KPI-CPI-3 footer) — so the render needs no second query.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InferenceReport {
    pub candidates: Vec<NumberedCandidate>,
    pub already_signed: Vec<AlreadySigned>,
    pub repos_without_signed_claims: Vec<String>,
    /// My standing INFERRED claims (in scope) some of whose cited support no
    /// longer holds (DDD-8 / UC-5) — flagged only, never acted on (D-5).
    pub weakened: Vec<WeakenedClaim>,
}

/// Infer the report under `filter`, classifying each candidate against my
/// own standing adherence claims (DDD-8). Person scoping narrows the links
/// first, so the footer names only the scoped person's repos.
pub fn infer_people_report(
    links: &[ContributionLink],
    repo_claims: &[RepoClaim],
    own_claims: &[OwnClaim],
    filter: &InferenceFilter,
) -> InferenceReport {
    let person = filter.person.as_ref().map(PersonSubject::as_str);
    let scoped_links: Vec<ContributionLink> = links
        .iter()
        .filter(|link| is_in_scope(link, person))
        .cloned()
        .collect();
    let signed = standing_adherences(own_claims);
    let no_standing: Vec<&OwnClaim> = Vec::new();
    let (already_signed, candidates) = infer_person_candidates(&scoped_links, repo_claims)
        .into_iter()
        .filter(|candidate| candidate.support().len() >= filter.min_repos)
        .fold(
            (Vec::new(), Vec::new()),
            |(mut already, mut numbered), candidate| {
                let standing = signed
                    .get(&pair_key(
                        candidate.person_subject(),
                        candidate.philosophy(),
                    ))
                    .unwrap_or(&no_standing);
                let (signed_lines, numbered_entry) = place_candidate(candidate, standing);
                already.extend(signed_lines);
                numbered.extend(numbered_entry);
                (already, numbered)
            },
        );
    InferenceReport {
        candidates,
        already_signed,
        repos_without_signed_claims: repos_without_signed_claims(&scoped_links, repo_claims),
        weakened: weakened_claims(&signed, repo_claims, person),
    }
}

/// Where one candidate lands in the report, given my standing claims for its
/// pair (DDD-8): the already-signed lines it produces and, unless it is
/// fully signed, its numbered entry.
fn place_candidate(
    candidate: PersonCandidate,
    standing: &[&OwnClaim],
) -> (Vec<AlreadySigned>, Option<NumberedCandidate>) {
    match classify(&candidate, standing) {
        Classification::New => (
            Vec::new(),
            Some(NumberedCandidate {
                candidate,
                status: CandidateStatus::New,
            }),
        ),
        Classification::AlreadySigned(cid) => (
            vec![AlreadySigned {
                cid: cid.to_string(),
                candidate,
            }],
            None,
        ),
        Classification::Stronger {
            supersedes,
            still_signed,
        } => {
            let still_signed_lines = still_signed
                .iter()
                .map(|cid| AlreadySigned {
                    cid: (*cid).to_string(),
                    candidate: candidate.clone(),
                })
                .collect();
            let numbered = NumberedCandidate {
                candidate,
                status: CandidateStatus::Stronger {
                    supersedes: supersedes.to_string(),
                },
            };
            (still_signed_lines, Some(numbered))
        }
    }
}

/// DDD-14 change summary: how many numbered (unsigned) inferred candidates
/// the `after` report proposes for a (person, philosophy) pair the `before`
/// report did not — the scrape hint's count. Candidates present in both runs,
/// and candidates a run dropped, count nothing; identical reports give 0.
pub fn new_inferred_candidate_count(before: &InferenceReport, after: &InferenceReport) -> usize {
    let proposed_before = numbered_pair_keys(before);
    numbered_pair_keys(after)
        .difference(&proposed_before)
        .count()
}

/// The join keys of a report's numbered candidates.
fn numbered_pair_keys(report: &InferenceReport) -> BTreeSet<PairKey> {
    report
        .candidates
        .iter()
        .map(|numbered| {
            pair_key(
                numbered.candidate.person_subject(),
                numbered.candidate.philosophy(),
            )
        })
        .collect()
}

/// DDD-8: my STANDING adherence claims by pair — any adherence I signed
/// (from an inference or by hand) that is no retraction marker and that I have
/// neither retracted nor superseded (the shared claim-domain rules).
fn standing_adherences(own_claims: &[OwnClaim]) -> StandingByPair<'_> {
    let lineages: Vec<ClaimLineage<'_>> = own_claims.iter().map(OwnClaim::lineage).collect();
    own_claims
        .iter()
        .filter(|claim| claim.predicate == ADHERES_TO_PHILOSOPHY)
        .filter(|claim| !is_retraction_marker(&claim.references))
        .filter(|claim| {
            let lineage = claim.lineage();
            !is_self_retracted(&lineage, &lineages) && !is_superseded_by_author(&lineage, &lineages)
        })
        .fold(BTreeMap::new(), |mut by_pair, claim| {
            by_pair
                .entry(pair_key(&claim.subject, &claim.object))
                .or_insert_with(Vec::new)
                .push(claim);
            by_pair
        })
}

fn is_retraction_marker(references: &[ClaimReference]) -> bool {
    references
        .iter()
        .any(|reference| reference.ref_type == ReferenceType::Retracts)
}

impl OwnClaim {
    fn lineage(&self) -> ClaimLineage<'_> {
        ClaimLineage {
            author_did: &self.author_did,
            cid: &self.cid,
            references: &self.references,
        }
    }
}

/// A link is in scope when no person is named, or it names that person.
fn is_in_scope(link: &ContributionLink, person: Option<&str>) -> bool {
    person.is_none_or(|p| subject_key(&link.person_subject) == subject_key(p))
}
