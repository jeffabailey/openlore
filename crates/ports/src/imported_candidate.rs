//! `imported_candidate` — the candidates an import derived, kept locally
//! (owner-only-signing slice 5, ADR-087 imports, OOS-D12 B, C-OOS-7).
//!
//! Anyone may import any public repo; what the import derived is kept as a
//! local, never-federated record so it can be reviewed offline later. The
//! record is **insert only** and it is never a claim: nothing here signs,
//! publishes or weighs, and scoring and inference never read it (check-arch
//! `imported_candidates_not_in_scoring_or_inference`). Signing from an import
//! always re-derives through the authority gate, so a kept candidate carries
//! no signing state.
//!
//! The ports are split so each verb holds the narrowest handle: `candidates
//! list` holds [`ImportedCandidateReadPort`] only; the import verb also holds
//! [`ImportedCandidateWritePort`], whose single method inserts one import.

use chrono::{DateTime, Utc};

use crate::github::CandidateClaim;
use crate::probe::ProbeOutcome;
use crate::repo_authority_check::RepoSubjectKey;

/// One candidate claim an import derived, as kept.
#[derive(Debug, Clone, PartialEq)]
pub struct ImportedCandidate {
    /// `github:<owner>/<repo>` as imported.
    pub subject: String,
    pub predicate: String,
    pub object: String,
    /// The mapping default the candidate was derived with.
    pub confidence: f64,
    /// Public URL(s) of the signals the candidate came from.
    pub evidence: Vec<String>,
}

impl ImportedCandidate {
    /// The case-folded subject the record is read by.
    pub fn subject_key(&self) -> RepoSubjectKey {
        RepoSubjectKey::of(&self.subject)
    }
}

impl From<&CandidateClaim> for ImportedCandidate {
    fn from(candidate: &CandidateClaim) -> Self {
        Self {
            subject: candidate.subject.clone(),
            predicate: candidate.predicate.clone(),
            object: candidate.object.clone(),
            confidence: candidate.confidence,
            evidence: candidate.evidence.clone(),
        }
    }
}

/// One import: every candidate it derived, at one instant.
#[derive(Debug, Clone, PartialEq)]
pub struct CandidateImport {
    pub imported_at: DateTime<Utc>,
    pub candidates: Vec<ImportedCandidate>,
}

/// A kept candidate with the import it came from. `import_id` is assigned
/// by the store, one per import, ascending.
#[derive(Debug, Clone, PartialEq)]
pub struct KeptCandidate {
    pub import_id: i64,
    pub imported_at: DateTime<Utc>,
    pub candidate: ImportedCandidate,
}

/// Failure surface of the imported-candidate ports.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ImportedCandidateError {
    #[error("imported candidate store failure: {0}")]
    Store(String),
}

/// Read side of the kept candidates.
pub trait ImportedCandidateReadPort {
    /// Earned-Trust probe: inside a rolled-back transaction, one sentinel
    /// import must be kept and read back by its subject.
    fn probe(&self) -> ProbeOutcome;

    /// Every kept candidate, or only those about `subject`, ordered by
    /// subject key, predicate, object, then import.
    fn list(
        &self,
        subject: Option<&RepoSubjectKey>,
    ) -> Result<Vec<KeptCandidate>, ImportedCandidateError>;
}

/// Write side: keep one import. There is no update and no delete.
pub trait ImportedCandidateWritePort {
    fn record_import(&self, import: &CandidateImport) -> Result<(), ImportedCandidateError>;
}
