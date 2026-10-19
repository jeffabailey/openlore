//! `identity_link_check` — local GitHub proof checks of identity links and the
//! link-candidate claim read (ADR-085 §7).
//!
//! Checks are a local, never-federated history of observations: one row per
//! check, **insert only**. "Latest" is a read, never an overwrite. The ports
//! are split so each verb holds the narrowest handle (ADR-085 §7 table):
//! readers hold [`IdentityLinkCheckReadPort`] only; writers also hold
//! [`IdentityLinkCheckWritePort`], whose single method inserts. The one
//! exception to insert-only is [`IdentityLinkCheckPurgePort`], held by
//! `identity forget` alone (C-GDL-5, ADR-082 precedent). A link's lineage
//! (the stored claims referencing it) is read through
//! [`LinkLineageReadPort`], which can never write a claim.

use chrono::{DateTime, Utc};
use claim_domain::identity_link::{
    active_link_records, active_links, link_predicate, links_to_recheck, listing_in_scope,
    CheckVerdict, ClaimFields, IdentityLink, LatestCheck, LinkRecord, ListedLink, RecheckScope,
    StoredClaim,
};
use claim_domain::{ClaimLineage, ClaimReference, Provenance};

use crate::probe::ProbeOutcome;
use crate::StorageError;

/// One proof check of one link (subject DID, numeric GitHub id).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkCheck {
    /// The link's bare subject DID.
    pub subject_did: String,
    /// The linked account's stable numeric GitHub id.
    pub github_user_id: u64,
    /// The login GitHub reported in the profile the check read.
    pub login_seen: String,
    pub verdict: CheckVerdict,
    pub checked_at: DateTime<Utc>,
}

/// Which links' checks a read returns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckFilter {
    /// Every link.
    All,
    /// The links of one bare subject DID.
    Subject(String),
}

/// Failure surface of the check ports.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum IdentityLinkCheckError {
    #[error("identity link check store failure: {0}")]
    Store(String),
}

/// Read side of the local check history.
pub trait IdentityLinkCheckReadPort {
    /// Earned-Trust probe (ADR-085 Earned Trust): inside a rolled-back
    /// transaction, two checks of one sentinel link at two instants must
    /// both be kept and the later one read back as latest.
    fn probe(&self) -> ProbeOutcome;

    /// The latest check per link (max `checked_at`, then insertion order)
    /// among the checks not dated after `now`, ordered by link.
    fn latest_checks(
        &self,
        filter: &CheckFilter,
        now: DateTime<Utc>,
    ) -> Result<Vec<LinkCheck>, IdentityLinkCheckError>;
}

/// Write side: insert one check. There is no update and no delete here.
pub trait IdentityLinkCheckWritePort {
    fn record_check(&self, check: &LinkCheck) -> Result<(), IdentityLinkCheckError>;
}

/// Purge side: the only removal of checks, held by `identity forget` alone.
/// It erases the local history of exactly one subject DID and never touches a
/// claim (C-GDL-5, ADR-085 §7).
pub trait IdentityLinkCheckPurgePort {
    /// Delete every check of one bare subject DID; returns how many went.
    fn purge_subject(&self, subject_did: &str) -> Result<u64, IdentityLinkCheckError>;
}

/// A stored claim whose predicate is the identity-link predicate — a
/// CANDIDATE only: whether it is a link is decided by the pure projection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkCandidate {
    pub cid: String,
    pub author_did: String,
    pub subject: String,
    pub predicate: String,
    pub object: String,
    /// The evidence entries, in signed order.
    pub evidence: Vec<String>,
    /// The record's OWN references (e.g. a `Retracts` marker), so the pure
    /// projection decides whether the record is a retraction (U-51, U-39).
    pub references: Vec<ClaimReference>,
    /// The provenance the row was stored under: own rows are app-signed;
    /// peer rows carry `peer_claims.provenance` (ADR-071).
    pub provenance: Provenance,
}

/// Read the stored claims carrying one predicate (callers pass
/// `claim_domain::identity_link::link_predicate()`).
pub trait LinkCandidateReadPort {
    fn link_candidates(
        &self,
        predicate: &str,
    ) -> Result<Vec<LinkCandidate>, IdentityLinkCheckError>;
}

/// One stored claim referencing another — own or a peer's — as its lineage
/// reads it: its CID, its author as stored, and its references to the claim
/// read. Only its author can retract or supersede their own claim, so the
/// author is never elided (I-FED-1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaimReferrer {
    pub cid: String,
    pub author_did: String,
    pub references: Vec<ClaimReference>,
}

/// Read a link's lineage: the stored claims — own and pulled from peers —
/// referencing one claim (its retraction / supersession markers).
/// Read-only — the narrow handle a verb holds instead of the whole
/// `StoragePort`.
pub trait LinkLineageReadPort {
    /// The stored claims (own and peer) referencing the claim at `cid`.
    fn referrers_of(&self, cid: &str) -> Result<Vec<ClaimReferrer>, StorageError>;
}

// =============================================================================
// The one lineage read of active links (D-VL-3): shared by `identity list`,
// the person views and the viewer, so the retraction, removed-peer and
// supersession rules cannot drift between them.
// =============================================================================

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
    claims: &dyn LinkCandidateReadPort,
    lineage: &dyn LinkLineageReadPort,
    scope: &RecheckScope,
    now: DateTime<Utc>,
) -> Result<Vec<ListedLink>, LinkReadError> {
    let records = StoredLinkLineage::read(claims, lineage)?.active_link_records();
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
