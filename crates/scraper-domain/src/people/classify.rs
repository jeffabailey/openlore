//! DDD-8 / UC-4 / Q-CPI-D4: how one candidate stands against my standing
//! adherence claims for its pair — NEW, already signed, or STRONGER.

use super::candidate::PersonCandidate;
use super::provenance::{cited_cids, parse_provenance};
use super::report::OwnClaim;

/// How one candidate stands against my standing claims for its pair.
pub(super) enum Classification<'a> {
    New,
    AlreadySigned(&'a str),
    Stronger {
        supersedes: &'a str,
        still_signed: Vec<&'a str>,
    },
}

/// DDD-8 / UC-4 / Q-CPI-D4: no standing claim → NEW; any hand-authored one
/// (cites no claim AT-URI) → already signed, never STRONGER; otherwise the
/// latest `composed_at` inferred claim is STRONGER-superseded when a repo
/// supporting the candidate now is cited by none of its claim AT-URIs — the
/// others stay listed as already signed.
pub(super) fn classify<'a>(
    candidate: &PersonCandidate,
    standing: &[&'a OwnClaim],
) -> Classification<'a> {
    let Some(smallest) = standing.iter().map(|claim| claim.cid.as_str()).min() else {
        return Classification::New;
    };
    if standing.iter().any(|claim| is_hand_authored(claim)) {
        return Classification::AlreadySigned(smallest);
    }
    let Some(latest) = latest_composed(standing) else {
        return Classification::New;
    };
    if !has_uncited_supporting_repo(candidate, latest) {
        return Classification::AlreadySigned(smallest);
    }
    let mut still_signed: Vec<&str> = standing
        .iter()
        .filter(|claim| claim.cid != latest.cid)
        .map(|claim| claim.cid.as_str())
        .collect();
    still_signed.sort_unstable();
    Classification::Stronger {
        supersedes: latest.cid.as_str(),
        still_signed,
    }
}

/// Q-CPI-D4: the most recently composed of my standing claims for a pair,
/// ties broken by the larger CID so the choice is total and order-free.
///
/// INVARIANT: every `composed_at` compared here is an RFC3339 UTC timestamp
/// written by the ONE `ClockPort` (`now_utc().to_rfc3339()`) — same offset,
/// same precision — so its lexical order IS its chronological order. These
/// are MY claims only (`OwnClaim`); a peer's clock never reaches this
/// comparison.
fn latest_composed<'a>(standing: &[&'a OwnClaim]) -> Option<&'a OwnClaim> {
    standing
        .iter()
        .copied()
        .max_by(|a, b| (&a.composed_at, &a.cid).cmp(&(&b.composed_at, &b.cid)))
}

/// UC-4: an adherence citing no claim AT-URI was authored by hand.
fn is_hand_authored(claim: &OwnClaim) -> bool {
    parse_provenance(&claim.evidence).is_empty()
}

/// Some repo supporting the candidate NOW has none of its supporting claims
/// cited by `signed` (claims matched by content address).
fn has_uncited_supporting_repo(candidate: &PersonCandidate, signed: &OwnClaim) -> bool {
    let cited = cited_cids(&signed.evidence);
    candidate.support().iter().any(|repo| {
        !repo
            .claims
            .iter()
            .any(|claim| cited.contains(&claim.cited.cid))
    })
}
