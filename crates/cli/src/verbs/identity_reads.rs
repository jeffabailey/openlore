//! The reads the identity verbs share (ADR-085 §4/§7): the stored link
//! lineage — the link candidates and every stored claim referencing one (the
//! retraction / supersession markers), through the candidate and lineage READ
//! ports only — and a GitHub profile as the observation a check consumes.
//! On top of that lineage: the links a listing shows (`identity list`, the
//! person views) and the links a re-check covers (`identity verify`, the
//! person-scrape refresh), each within a `RecheckScope`.
//!
//! The decisions (which candidates are active links, what a read proves) stay
//! in `claim_domain::identity_link`; this module only reads and adapts.

use anyhow::Result;
use chrono::{DateTime, Utc};
use claim_domain::identity_link::{
    active_link_records, active_links, is_in_scope, link_predicate, links_to_recheck,
    listing_in_scope, login_to_read, ClaimFields, GithubObservation, IdentityLink, LatestCheck,
    LinkRecord, ListedLink, ProofCheck, RecheckScope, StoredClaim,
};
use claim_domain::ClaimLineage;
use ports::{
    CheckFilter, ClaimReferrer, GithubAccountProfile, IdentityLinkCheckReadPort, LinkCandidate,
    LinkCandidateReadPort, LinkCheck, LinkLineageReadPort,
};

/// The link candidates in the store and every stored claim — own or a
/// peer's — referencing one.
pub struct StoredLinkLineage {
    candidates: Vec<LinkCandidate>,
    referrers: Vec<ClaimReferrer>,
}

impl StoredLinkLineage {
    /// Read the candidates carrying the link predicate and their referrers.
    pub fn read(
        candidates: &dyn LinkCandidateReadPort,
        lineage: &dyn LinkLineageReadPort,
    ) -> Result<Self> {
        let candidates = candidates.link_candidates(link_predicate())?;
        let referrers = candidates
            .iter()
            .map(|candidate| lineage.referrers_of(&candidate.cid))
            .collect::<std::result::Result<Vec<_>, _>>()?
            .into_iter()
            .flatten()
            .collect();
        Ok(Self {
            candidates,
            referrers,
        })
    }

    /// The active links (every active link claim).
    pub fn active_links(&self) -> Vec<IdentityLink> {
        self.decide(active_links)
    }

    /// The active links, once per (DID, account id).
    pub fn links_to_recheck(&self) -> Vec<IdentityLink> {
        self.decide(links_to_recheck)
    }

    /// The records stating an active link, each named by its record ID.
    pub fn active_link_records(&self) -> Vec<LinkRecord> {
        self.decide(active_link_records)
    }

    fn decide<T>(&self, decision: fn(&[StoredClaim<'_>], &[ClaimLineage<'_>]) -> Vec<T>) -> Vec<T> {
        let stored: Vec<StoredClaim<'_>> = self.candidates.iter().map(stored_claim).collect();
        let lineages: Vec<ClaimLineage<'_>> = self.referrers.iter().map(referrer_lineage).collect();
        decision(&stored, &lineages)
    }
}

/// One recorded check as the pure link view reads it.
fn latest_of(check: LinkCheck) -> LatestCheck {
    LatestCheck {
        subject_did: check.subject_did,
        user_id: check.github_user_id,
        login_seen: check.login_seen,
        verdict: check.verdict,
        checked_at: check.checked_at,
    }
}

/// Every link within `scope` as `identity list` shows it as of `now`,
/// offline: the stored lineage, ALL of this reader's own checks and
/// `listing_in_scope`, with the records stating each. Standings are decided
/// over every active link before the scope selects lines, so a scope never
/// changes a standing. A retracted or superseded link is not among them
/// (ADR-085 §5, ADR-060). Shared by `identity list` and the person views.
/// Read-only — no check is written, purged or refreshed; no check is read
/// when no link is stored.
pub(crate) fn read_listed_links(
    checks: &dyn IdentityLinkCheckReadPort,
    claims: &dyn LinkCandidateReadPort,
    lineage: &dyn LinkLineageReadPort,
    scope: &RecheckScope,
    now: DateTime<Utc>,
) -> Result<Vec<ListedLink>> {
    let records = StoredLinkLineage::read(claims, lineage)?.active_link_records();
    if records.is_empty() {
        return Ok(Vec::new());
    }
    let latest: Vec<LatestCheck> = checks
        .latest_checks(&CheckFilter::All, now)?
        .into_iter()
        .map(latest_of)
        .collect();
    Ok(listing_in_scope(scope, &records, &latest, now))
}

/// One candidate as the pure projection reads it.
fn stored_claim(candidate: &LinkCandidate) -> StoredClaim<'_> {
    StoredClaim {
        fields: ClaimFields {
            author: &candidate.author_did,
            subject: &candidate.subject,
            predicate: &candidate.predicate,
            object: &candidate.object,
            evidence: &candidate.evidence,
        },
        provenance: &candidate.provenance,
        lineage: ClaimLineage {
            author_did: &candidate.author_did,
            cid: &candidate.cid,
            references: &[],
        },
    }
}

/// One referrer as a lineage marker (its author and what it references).
fn referrer_lineage(referrer: &ClaimReferrer) -> ClaimLineage<'_> {
    ClaimLineage {
        author_did: &referrer.author_did,
        cid: &referrer.cid,
        references: &referrer.references,
    }
}

/// The observation a fresh strict account read makes — only a profile
/// naming an account (`read_account` / `read_person_by_id`) is observed.
pub(crate) fn observation_of(account: GithubAccountProfile) -> GithubObservation {
    let profile = account.into_profile();
    GithubObservation {
        login: profile.login,
        user_id: profile.id,
        bio: profile.bio,
    }
}

/// One active link a re-check covers, with the login to read for it.
pub(crate) struct LinkToRecheck {
    pub link: IdentityLink,
    pub login: String,
}

/// The active links within `scope` as of `now`, once per (DID, account id),
/// each with the login to read for it (`identity verify`, the person scrape).
pub(crate) fn links_in_scope(
    checks: &dyn IdentityLinkCheckReadPort,
    claims: &dyn LinkCandidateReadPort,
    lineage: &dyn LinkLineageReadPort,
    scope: &RecheckScope,
    now: DateTime<Utc>,
) -> Result<Vec<LinkToRecheck>> {
    let latest: Vec<LatestCheck> = checks
        .latest_checks(&CheckFilter::All, now)?
        .into_iter()
        .map(latest_of)
        .collect();
    Ok(StoredLinkLineage::read(claims, lineage)?
        .links_to_recheck()
        .into_iter()
        .filter_map(|link| {
            let login = login_to_read(&link, &latest).to_string();
            is_in_scope(scope, &link, &login).then_some(LinkToRecheck { link, login })
        })
        .collect())
}

/// The one check a decided re-check appends.
pub(crate) fn check_of(link: &IdentityLink, decided: &ProofCheck, now: DateTime<Utc>) -> LinkCheck {
    LinkCheck {
        subject_did: link.did.clone(),
        github_user_id: link.user_id,
        login_seen: decided.login_seen().to_string(),
        verdict: decided.verdict(),
        checked_at: now,
    }
}
