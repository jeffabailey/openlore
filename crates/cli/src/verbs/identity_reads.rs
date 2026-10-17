//! The reads the identity verbs share (ADR-085 §4/§7): the stored link
//! lineage — the link candidates and every stored claim referencing one (the
//! retraction / supersession markers), through the candidate and lineage READ
//! ports only — and a GitHub profile as the observation a check consumes.
//!
//! The decisions (which candidates are active links, what a read proves) stay
//! in `claim_domain::identity_link`; this module only reads and adapts.

use anyhow::Result;
use chrono::{DateTime, Utc};
use claim_domain::identity_link::{
    active_link_records, active_links, is_in_scope, link_predicate, link_view, links_to_recheck,
    listed_links, ClaimFields, GithubObservation, IdentityLink, LatestCheck, LinkLifecycle,
    LinkRecord, ListedLink, ProofCheck, ReaderLink, RecheckScope, StoredClaim,
};
use claim_domain::{ClaimLineage, SignedClaim};
use ports::{
    CheckFilter, GithubAccountProfile, IdentityLinkCheckReadPort, LinkCandidate,
    LinkCandidateReadPort, LinkCheck, LinkLineageReadPort,
};

/// The link candidates in the store and every stored claim referencing one.
pub struct StoredLinkLineage {
    candidates: Vec<LinkCandidate>,
    referrers: Vec<SignedClaim>,
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

    /// The active links as a reader holds them: a retracted or superseded
    /// link is not among them (ADR-085 §5, ADR-060).
    pub fn reader_links(&self) -> Vec<ReaderLink> {
        self.active_links()
            .into_iter()
            .map(|link| ReaderLink {
                link,
                lifecycle: LinkLifecycle::Active,
            })
            .collect()
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
pub(crate) fn latest_of(check: LinkCheck) -> LatestCheck {
    LatestCheck {
        subject_did: check.subject_did,
        user_id: check.github_user_id,
        login_seen: check.login_seen,
        verdict: check.verdict,
        checked_at: check.checked_at,
    }
}

/// Every link as `identity list` shows it as of `now`, offline: the stored
/// lineage, this reader's own checks and `link_view`, with the records
/// stating each. Read-only — no check is written, purged or refreshed.
pub(crate) fn read_shown_links(
    checks: &dyn IdentityLinkCheckReadPort,
    claims: &dyn LinkCandidateReadPort,
    lineage: &dyn LinkLineageReadPort,
    now: DateTime<Utc>,
) -> Result<Vec<ListedLink>> {
    let stored = StoredLinkLineage::read(claims, lineage)?;
    let links = stored.reader_links();
    if links.is_empty() {
        return Ok(Vec::new());
    }
    let latest: Vec<LatestCheck> = checks
        .latest_checks(&CheckFilter::All, now)?
        .into_iter()
        .map(latest_of)
        .collect();
    Ok(listed_links(
        link_view(&links, &latest, now),
        &stored.active_link_records(),
    ))
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
fn referrer_lineage(referrer: &SignedClaim) -> ClaimLineage<'_> {
    ClaimLineage {
        author_did: &referrer.unsigned.author_did.0,
        cid: "",
        references: &referrer.unsigned.references,
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

/// The active links in the store, once per (DID, account id).
fn active_links_in_store(
    claims: &dyn LinkCandidateReadPort,
    lineage: &dyn LinkLineageReadPort,
) -> Result<Vec<IdentityLink>> {
    Ok(StoredLinkLineage::read(claims, lineage)?.links_to_recheck())
}

/// The login to read for `link`: the last one this reader saw, else the
/// signed one (ADR-085 §4).
fn login_to_read<'a>(link: &'a IdentityLink, latest: &'a [LinkCheck]) -> &'a str {
    latest
        .iter()
        .find(|check| check.subject_did == link.did && check.github_user_id == link.user_id)
        .map_or(link.login.as_str(), |check| check.login_seen.as_str())
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
    let latest = checks.latest_checks(&CheckFilter::All, now)?;
    Ok(active_links_in_store(claims, lineage)?
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
