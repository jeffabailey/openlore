//! DDD-8 / UC-5: my standing inferred claims whose cited support no longer
//! holds — flagged only, never acted on (D-5).

use std::collections::BTreeMap;

use claim_domain::{is_self_retracted, ClaimLineage};

use super::candidate::RepoClaim;
use super::inference::is_by_active_author;
use super::provenance::cited_cids;
use super::report::{OwnClaim, StandingByPair};
use super::subject_key;

/// Why one supporting claim my signed inference cites no longer supports it
/// (DDD-8 / UC-5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum WeakenedSupport {
    /// Its own author retracted it (the shared claim-domain rule).
    Retracted,
    /// It is still cached but its author is no longer a subscribed peer (D-2).
    NoLongerEligible,
    /// No claim with its CID is in my local store any more.
    MissingLocally,
}

/// One of my standing inferred claims whose cited support weakened: the
/// count of distinct supporting claims it cites and, per reason, how many of
/// them no longer support it. The claim itself is untouched (D-5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WeakenedClaim {
    pub cid: String,
    pub person_subject: String,
    pub philosophy: String,
    pub cited: usize,
    pub weakened: BTreeMap<WeakenedSupport, usize>,
}

/// DDD-8 / UC-5: my standing inferred claims in scope whose cited support
/// weakened, by pair then CID. Read-only — nothing is retracted, countered or
/// re-signed (D-5).
pub(super) fn weakened_claims(
    standing: &StandingByPair<'_>,
    repo_claims: &[RepoClaim],
    person: Option<&str>,
) -> Vec<WeakenedClaim> {
    let lineages: Vec<ClaimLineage<'_>> = repo_claims.iter().map(RepoClaim::lineage).collect();
    let person_key = person.map(subject_key);
    standing
        .iter()
        .filter(|((person_subject, _), _)| person_key.as_ref().is_none_or(|p| p == person_subject))
        .flat_map(|(_, claims)| {
            let mut claims = claims.clone();
            claims.sort_by(|a, b| a.cid.cmp(&b.cid));
            claims
        })
        .filter_map(|claim| weakened_claim(claim, repo_claims, &lineages))
        .collect()
}

/// `claim`'s weakened support, counted per reason over its DISTINCT cited
/// claims — `None` when every cited claim still supports it (or it cites
/// none, i.e. it was authored by hand).
fn weakened_claim(
    claim: &OwnClaim,
    repo_claims: &[RepoClaim],
    lineages: &[ClaimLineage<'_>],
) -> Option<WeakenedClaim> {
    let cited = cited_cids(&claim.evidence);
    let weakened = cited
        .iter()
        .filter_map(|cid| weakening_of(cid, repo_claims, lineages))
        .fold(BTreeMap::new(), |mut by_reason, reason| {
            *by_reason.entry(reason).or_insert(0) += 1;
            by_reason
        });
    (!weakened.is_empty()).then(|| WeakenedClaim {
        cid: claim.cid.clone(),
        person_subject: claim.subject.clone(),
        philosophy: claim.object.clone(),
        cited: cited.len(),
        weakened,
    })
}

/// Why the cited claim `cid` no longer supports an inference, resolved by
/// content address against the read: absent → missing locally; retracted by
/// its own author (the shared claim-domain rule) → retracted; held only from
/// authors who are no longer active (DDD-7 eligibility, D-2) → no longer
/// eligible; otherwise it still supports.
fn weakening_of(
    cid: &str,
    repo_claims: &[RepoClaim],
    lineages: &[ClaimLineage<'_>],
) -> Option<WeakenedSupport> {
    let rows: Vec<&RepoClaim> = repo_claims
        .iter()
        .filter(|claim| claim.cid == cid)
        .collect();
    if rows.is_empty() {
        Some(WeakenedSupport::MissingLocally)
    } else if rows
        .iter()
        .any(|row| is_self_retracted(&row.lineage(), lineages))
    {
        Some(WeakenedSupport::Retracted)
    } else if !rows.iter().any(|row| is_by_active_author(row)) {
        Some(WeakenedSupport::NoLongerEligible)
    } else {
        None
    }
}
