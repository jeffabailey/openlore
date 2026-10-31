//! `repo_did_list` — where the pass reads the operator's DID list from on every
//! run (ADR-090 §4, D-STZ-16, KPI-IXD-4). ASYNC (network parameter store).
//!
//! It returns the RAW text. Validation stays in the existing ADR-081 loader
//! (`appview_domain::did_list::read_did_list`), so a malformed list is refused
//! exactly as on the host.
//
// SCAFFOLD: true  (trait surface only, DISTILL-proposed names; the SSM adapter
// lands in DELIVER in `adapter-s3-index-object`)

use async_trait::async_trait;

use crate::ProbeOutcome;

/// Why the DID list could not be read.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RepoDidListError {
    #[error("DID list source probe refused: {detail}")]
    ProbeRefused { detail: String },
    /// The parameter store could not be reached, refused, or has no such list.
    #[error("DID list source unavailable: {detail}")]
    Unavailable { detail: String },
}

/// The source of the operator's DID list.
#[async_trait]
pub trait RepoDidListSourcePort: Send + Sync {
    /// Earned-Trust probe — see ADR-009 + `probe.rs`.
    fn probe(&self) -> ProbeOutcome;

    /// The list as the operator saved it (raw text, one DID per line or comma
    /// separated, exactly what the ADR-081 loader reads).
    async fn current(&self) -> Result<String, RepoDidListError>;
}
