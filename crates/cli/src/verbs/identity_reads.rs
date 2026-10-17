//! The reads the identity verbs share (ADR-085 §4/§7): the stored link
//! lineage — the link candidates and every stored claim referencing one (the
//! retraction / supersession markers), through the candidate and lineage READ
//! ports only — and a GitHub profile as the observation a check consumes.
//!
//! The decisions (which candidates are active links, what a read proves) stay
//! in `claim_domain::identity_link`; this module only reads and adapts.

use anyhow::Result;
use claim_domain::identity_link::{
    active_link_records, active_links, link_predicate, links_to_recheck, ClaimFields,
    GithubObservation, IdentityLink, LinkLifecycle, LinkRecord, ReaderLink, StoredClaim,
};
use claim_domain::{ClaimLineage, SignedClaim};
use ports::{GithubAccountProfile, LinkCandidate, LinkCandidateReadPort, LinkLineageReadPort};

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
