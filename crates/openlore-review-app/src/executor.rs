//! Executing a publish plan (US-BRA-004) or a confirmed share post
//! (US-BRA-008): the bounded change — ONE record,
//! claim collection, the owner's own repo — then the read-back that proves
//! it landed as planned. The PDS-returned CID is never trusted: the record
//! read back is recomputed with claim-domain's canonicalizer (DWD-5).

use ports::{CreatedRecord, RepoWriteError, UserRepoReadPort, UserRepoWritePort};
use review_domain::plans::{read_back_matches, PublishPlan};
use review_domain::share::SharePost;

/// Why a publish did not complete.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PublishFailure {
    /// The PDS did not take the record (nothing was written).
    NotWritten(RepoWriteError),
    /// The record was created but what reads back is not the plan's record.
    ReadBackMismatch,
}

impl PublishFailure {
    /// A failure only a fresh sign-in can cure (a retry would not help).
    pub(crate) fn needs_sign_in(&self) -> bool {
        matches!(
            self,
            Self::NotWritten(RepoWriteError::SessionExpired | RepoWriteError::NoSession)
        )
    }

    /// What the owner is told (AC-004.6).
    pub(crate) fn message(&self) -> &'static str {
        match self {
            Self::NotWritten(RepoWriteError::Unreachable { .. }) => {
                "We couldn't reach your PDS. Nothing was published."
            }
            Self::NotWritten(RepoWriteError::SessionExpired | RepoWriteError::NoSession) => {
                "Your Bluesky session has expired. Sign in again; nothing was published."
            }
            Self::NotWritten(RepoWriteError::Refused { .. }) => {
                "Your PDS refused the record. Nothing was published."
            }
            Self::ReadBackMismatch => {
                "Your PDS did not return the record exactly as previewed, so it is not counted \
                 as published."
            }
        }
    }
}

/// Create the plan's record in the owner's own repo and read it back.
/// Returns the record's `at://` address.
pub(crate) async fn execute_publish(
    write: &dyn UserRepoWritePort,
    read: &dyn UserRepoReadPort,
    plan: &PublishPlan,
) -> Result<String, PublishFailure> {
    let created = write
        .create_claim_record(plan.owner_did(), plan.rkey(), &plan.record())
        .await
        .map_err(PublishFailure::NotWritten)?;
    let read_back = read
        .read_claim_record(plan.owner_did(), plan.rkey())
        .await
        .map_err(|_| PublishFailure::ReadBackMismatch)?;
    if read_back_matches(&read_back, plan.rkey()) {
        Ok(created.uri)
    } else {
        Err(PublishFailure::ReadBackMismatch)
    }
}

/// Create the owner's confirmed share post (US-BRA-008) in their own repo,
/// exactly as composed, created at `created_at`. Returns its `at://` address.
pub(crate) async fn execute_share(
    write: &dyn UserRepoWritePort,
    owner_did: &str,
    post: &SharePost,
    created_at: &str,
) -> Result<CreatedRecord, PublishFailure> {
    write
        .create_post_record(owner_did, &post.record(created_at))
        .await
        .map_err(PublishFailure::NotWritten)
}
