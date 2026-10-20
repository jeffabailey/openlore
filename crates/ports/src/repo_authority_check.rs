//! `repo_authority_check` — this reader's local observations of repo
//! authority (`github:<owner>/<repo>` subjects, ADR-087 §4-§5).
//!
//! Observations are a local, never-federated history: one row per
//! observation, **insert only**. "Latest" is a read (max `checked_at`, then
//! max `check_id`), never an overwrite. The ports are split so each verb holds
//! the narrowest handle (ADR-087 §5, as ADR-085 §7): readers hold
//! [`RepoAuthorityCheckReadPort`] only; recorders also hold
//! [`RepoAuthorityCheckWritePort`], whose single method inserts.

use chrono::{DateTime, Utc};
use claim_domain::authority::RepoOwnerKind;

use crate::probe::ProbeOutcome;

/// The case-folded repo subject an observation is keyed by
/// (`github:o/r` as asked, lower-cased), so `github:Jeff/Tool` and
/// `github:jeff/tool` share one history.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RepoSubjectKey(String);

impl RepoSubjectKey {
    /// Fold a repo subject as asked into its key.
    pub fn of(subject: &str) -> Self {
        Self(subject.to_lowercase())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// The `.github/openlore-authority` file on the default branch, as read
/// alongside the repo (always read, so an observation is signer-independent).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthorityFile {
    Absent,
    /// The exact DID tokens the file lists.
    Present(Vec<String>),
    /// Any other shape (symlink, submodule, directory, oversize, non-UTF-8):
    /// grants nothing.
    Invalid,
}

/// The repo facts and authority file one observation found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FoundRepo {
    pub repo_id: u64,
    /// `owner/name` as GitHub served it.
    pub full_name_seen: String,
    pub owner_id: u64,
    pub owner_type: RepoOwnerKind,
    pub authority_file: AuthorityFile,
}

/// What one observation found. A repo GitHub does not have carries no file
/// state: none was read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RepoObservation {
    Found(FoundRepo),
    RepoNotFound,
}

/// One recorded observation of one repo subject.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoAuthorityCheck {
    pub subject_key: RepoSubjectKey,
    pub observation: RepoObservation,
    pub checked_at: DateTime<Utc>,
}

/// Failure surface of the repo-authority check ports.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RepoAuthorityCheckError {
    #[error("repo authority check store failure: {0}")]
    Store(String),
}

/// Read side of the local observation history.
pub trait RepoAuthorityCheckReadPort {
    /// Earned-Trust probe (ADR-087 Earned Trust): inside a rolled-back
    /// transaction, one sentinel observed at two instants must keep both and
    /// read the later one back as latest.
    fn probe(&self) -> ProbeOutcome;

    /// The latest observation per subject among `subjects` (max
    /// `checked_at`, then max `check_id`), skipping observations dated after
    /// `now`, ordered by subject key. A subject never observed is absent.
    fn latest(
        &self,
        subjects: &[RepoSubjectKey],
        now: DateTime<Utc>,
    ) -> Result<Vec<RepoAuthorityCheck>, RepoAuthorityCheckError>;
}

/// Write side: insert one observation. There is no update and no delete.
pub trait RepoAuthorityCheckWritePort {
    fn record(&self, check: &RepoAuthorityCheck) -> Result<(), RepoAuthorityCheckError>;
}
