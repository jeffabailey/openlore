//! `identity_link_listing` — the one lineage read of active links (D-VL-3),
//! composed over the identity-link READ ports only: shared by `identity
//! list`, the person views and the viewer, so the retraction, removed-peer and
//! supersession rules cannot drift between them. The decisions stay in
//! `claim_domain::identity_link`; this module only reads and adapts.

use chrono::{DateTime, Utc};
use claim_domain::identity_link::{
    active_link_records, active_links, link_predicate, links_to_recheck, listing_in_scope,
    ClaimFields, IdentityLink, LatestCheck, LinkRecord, ListedLink, RecheckScope, StoredClaim,
};
use claim_domain::ClaimLineage;

use crate::identity_link_check::{
    CheckFilter, ClaimReferrer, IdentityLinkCheckError, IdentityLinkCheckReadPort, LinkCandidate,
    LinkCandidateReadPort, LinkCheck, LinkLineageReadPort,
};
use crate::StorageError;

/// Why the stored links could not be read.
#[derive(Debug, thiserror::Error)]
pub enum LinkReadError {
    /// The check history or the link candidates could not be read.
    #[error(transparent)]
    Checks(#[from] IdentityLinkCheckError),
    /// A candidate's lineage (its retraction / supersession markers) could not be read.
    #[error(transparent)]
    Lineage(#[from] StorageError),
}

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
    ) -> Result<Self, LinkReadError> {
        let candidates = candidates.link_candidates(link_predicate())?;
        let referrers = candidates
            .iter()
            .map(|candidate| lineage.referrers_of(&candidate.cid))
            .collect::<Result<Vec<_>, _>>()?
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

/// This reader's latest check per link, not dated after `now`, as the pure
/// link view reads them.
pub fn read_latest_checks(
    checks: &dyn IdentityLinkCheckReadPort,
    now: DateTime<Utc>,
) -> Result<Vec<LatestCheck>, IdentityLinkCheckError> {
    Ok(checks
        .latest_checks(&CheckFilter::All, now)?
        .into_iter()
        .map(latest_of)
        .collect())
}

/// Every link within `scope` as `identity list` shows it as of `now`,
/// offline: the stored lineage, ALL of this reader's own checks and
/// `listing_in_scope`, with the records stating each. Standings are decided
/// over every active link before the scope selects lines, so a scope never
/// changes a standing. A retracted or superseded link is not among them
/// (ADR-085 §5, ADR-060). Shared by `identity list`, the person views and the
/// viewer. Read-only — no check is written, purged or refreshed; no check is
/// read when no link is stored.
pub fn read_listed_links(
    checks: &dyn IdentityLinkCheckReadPort,
    candidates: &dyn LinkCandidateReadPort,
    lineage: &dyn LinkLineageReadPort,
    scope: &RecheckScope,
    now: DateTime<Utc>,
) -> Result<Vec<ListedLink>, LinkReadError> {
    let records = StoredLinkLineage::read(candidates, lineage)?.active_link_records();
    if records.is_empty() {
        return Ok(Vec::new());
    }
    let latest = read_latest_checks(checks, now)?;
    Ok(listing_in_scope(scope, &records, &latest, now))
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
            references: &candidate.references,
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
