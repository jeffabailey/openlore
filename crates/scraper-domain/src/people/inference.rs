//! Inference (slices 02 + 03; DDD-7 / DDD-10; ADR-064): from recorded
//! contribution links and signed repo claims to the numbered person
//! candidates.
//!
//! `collapse_links |> join eligible repo claims |> group by (person,
//! philosophy) |> PersonCandidate::new |> sort (numbering)`.

use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet};

use claim_domain::{is_self_retracted, is_superseded_by_author, ClaimLineage, ReferenceType};
use ports::{AuthorRelationship, ContributionLink};

use super::candidate::{PersonCandidate, RepoClaim, SupportingClaim, SupportingRepo};
use super::provenance::CitedClaim;
use super::{pair_key, subject_key, PairKey};

/// Infer the numbered person candidates from the recorded contribution links
/// and the signed repo claims. Output order IS the numbering (person key,
/// then philosophy) and is independent of input order.
pub fn infer_person_candidates(
    links: &[ContributionLink],
    repo_claims: &[RepoClaim],
) -> Vec<PersonCandidate> {
    let links = collapse_links(links);
    let eligible = supporting_claims(repo_claims);
    group_support(&links, &eligible)
        .into_values()
        .filter_map(|group| {
            PersonCandidate::new(
                &group.person_subject,
                &group.philosophy,
                group.repos.into_values().collect(),
            )
            .ok()
        })
        .collect()
}

/// Linked repos (once per case-folded subject, smallest spelling shown) about
/// which no eligible signed philosophy claim exists.
pub(super) fn repos_without_signed_claims(
    links: &[ContributionLink],
    repo_claims: &[RepoClaim],
) -> Vec<String> {
    let supported: BTreeSet<String> = supporting_claims(repo_claims)
        .into_iter()
        .map(|claim| subject_key(&claim.repo_subject))
        .collect();
    links
        .iter()
        .filter(|link| !supported.contains(&subject_key(&link.repo_subject)))
        .fold(BTreeMap::<String, &str>::new(), |mut by_key, link| {
            by_key
                .entry(subject_key(&link.repo_subject))
                .and_modify(|shown| *shown = (*shown).min(link.repo_subject.as_str()))
                .or_insert(&link.repo_subject);
            by_key
        })
        .into_values()
        .map(str::to_string)
        .collect()
}

/// UC-6 / DDD-6 read side: every subject spelling the effect shell must read
/// with the exact-match federated query so the case-insensitive join sees all
/// claims about each linked repo — the links' own spellings plus every stored
/// subject that case-folds to a linked repo.
pub fn repo_subjects_to_read<'a>(
    links: &[ContributionLink],
    stored_subjects: impl IntoIterator<Item = &'a str>,
) -> BTreeSet<String> {
    let linked: BTreeSet<String> = links
        .iter()
        .map(|link| subject_key(&link.repo_subject))
        .collect();
    let stored_spellings = stored_subjects
        .into_iter()
        .filter(|subject| linked.contains(&subject_key(subject)))
        .map(str::to_string);
    links
        .iter()
        .map(|link| link.repo_subject.clone())
        .chain(stored_spellings)
        .collect()
}

/// DDD-7: the claims that may support an inference — each judged against the
/// whole read (retraction markers and successors arrive in the same subject
/// read, DDD-6).
fn supporting_claims(repo_claims: &[RepoClaim]) -> Vec<&RepoClaim> {
    let lineages: Vec<ClaimLineage<'_>> = repo_claims.iter().map(RepoClaim::lineage).collect();
    repo_claims
        .iter()
        .filter(|claim| supports_inference(claim, &lineages))
        .collect()
}

/// DDD-7: an `embodiesPhilosophy` claim by me or an ACTIVE peer, that is no
/// retracts/counters marker, and that its own author has neither retracted
/// (ADR-060 D-RF-D3, shared rule) nor superseded.
fn supports_inference(claim: &RepoClaim, lineages: &[ClaimLineage<'_>]) -> bool {
    let lineage = claim.lineage();
    is_philosophy_claim(claim)
        && is_by_active_author(claim)
        && !is_retraction_or_counter_marker(claim)
        && !is_self_retracted(&lineage, lineages)
        && !is_superseded_by_author(&lineage, lineages)
}

fn is_philosophy_claim(claim: &RepoClaim) -> bool {
    claim.predicate == crate::EMBODIES_PHILOSOPHY
}

/// Me, or a peer I am subscribed to NOW (D-2) — never a former peer's cache.
pub(super) fn is_by_active_author(claim: &RepoClaim) -> bool {
    matches!(
        claim.relationship,
        AuthorRelationship::You | AuthorRelationship::SubscribedPeer
    )
}

/// A marker copies its target's subject/predicate/object but asserts nothing
/// new — it is never itself support.
fn is_retraction_or_counter_marker(claim: &RepoClaim) -> bool {
    claim.references.iter().any(|reference| {
        matches!(
            reference.ref_type,
            ReferenceType::Retracts | ReferenceType::Counters
        )
    })
}

/// One link per (repo, person) key: the most recently observed, then the best
/// rank, then the smallest original spelling — a total, order-free choice.
fn collapse_links(links: &[ContributionLink]) -> Vec<&ContributionLink> {
    links
        .iter()
        .fold(
            BTreeMap::<(String, String), &ContributionLink>::new(),
            |mut by_key, link| {
                let key = (
                    subject_key(&link.repo_subject),
                    subject_key(&link.person_subject),
                );
                by_key
                    .entry(key)
                    .and_modify(|kept| {
                        if link_preference(link) < link_preference(kept) {
                            *kept = link;
                        }
                    })
                    .or_insert(link);
                by_key
            },
        )
        .into_values()
        .collect()
}

/// Preference key: latest observation (micros since epoch) first.
fn link_preference(link: &ContributionLink) -> (Reverse<i64>, u32, &str, &str) {
    (
        Reverse(link.last_observed_at.timestamp_micros()),
        link.rank,
        &link.repo_subject,
        &link.person_subject,
    )
}

/// The support gathered for one (person, philosophy) before construction.
struct SupportGroup {
    person_subject: String,
    philosophy: String,
    repos: BTreeMap<String, SupportingRepo>,
}

/// Join each collapsed link with the eligible claims on its repo, grouped by
/// (person key, philosophy) — the BTreeMap key order IS the numbering.
fn group_support(
    links: &[&ContributionLink],
    eligible: &[&RepoClaim],
) -> BTreeMap<PairKey, SupportGroup> {
    let mut groups = BTreeMap::<PairKey, SupportGroup>::new();
    for link in links {
        let repo_key = subject_key(&link.repo_subject);
        let on_repo = eligible
            .iter()
            .filter(|claim| subject_key(&claim.repo_subject) == repo_key);
        for claim in on_repo {
            let group = groups
                .entry(pair_key(&link.person_subject, &claim.philosophy))
                .or_insert_with(|| SupportGroup {
                    person_subject: link.person_subject.clone(),
                    philosophy: claim.philosophy.clone(),
                    repos: BTreeMap::new(),
                });
            if link.person_subject < group.person_subject {
                group.person_subject = link.person_subject.clone();
            }
            group
                .repos
                .entry(repo_key.clone())
                .or_insert_with(|| SupportingRepo {
                    repo_subject: link.repo_subject.clone(),
                    rank: link.rank,
                    claims: Vec::new(),
                })
                .claims
                .push(SupportingClaim {
                    cited: CitedClaim::new(&claim.author_did, &claim.cid),
                    confidence: claim.confidence,
                });
        }
    }
    groups
}
