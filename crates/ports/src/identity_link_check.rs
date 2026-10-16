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
use claim_domain::identity_link::CheckVerdict;
use claim_domain::{Provenance, SignedClaim};

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

/// Read a link's lineage: the stored claims referencing one claim (its
/// retraction / supersession markers). Read-only — the narrow handle a verb
/// holds instead of the whole `StoragePort`.
pub trait LinkLineageReadPort {
    /// The stored signed claims referencing the claim at `cid`.
    fn referrers_of(&self, cid: &str) -> Result<Vec<SignedClaim>, StorageError>;
}
