//! What a failed create may have left in the owner's repo (U-43, U-45): the
//! one decision every write page (link, claim publish, retraction, share
//! post) tells the truth by.

use ports::RepoWriteError;

/// Whether the repo host may have stored a record whose create failed
/// (U-43). A server error or an answer lost after the send may hide a
/// stored record; a 4xx refusal, an ended session or a connection never
/// made stored nothing. ("Already exists" is success, never a failure.)
pub fn may_have_stored(failure: &RepoWriteError) -> bool {
    match failure {
        RepoWriteError::Refused { status, .. } => !(400..500).contains(status),
        RepoWriteError::AnswerLost { .. } => true,
        RepoWriteError::Unreachable { .. }
        | RepoWriteError::NoSession
        | RepoWriteError::SessionExpired => false,
    }
}

/// What the app knows after a write that did not succeed: a page says
/// nothing was published, retracted or posted only when that is known (U-45).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailedWrite {
    /// The app knows what the repo holds; the page's reason says it.
    Definite,
    /// The repo host may have stored the record (a server error or a lost
    /// answer); only a Retry, or a look at the repo, can tell.
    MayHaveStored,
}

impl FailedWrite {
    /// What a create that failed with `failure` leaves the app knowing.
    pub fn of(failure: &RepoWriteError) -> Self {
        if may_have_stored(failure) {
            Self::MayHaveStored
        } else {
            Self::Definite
        }
    }
}

#[cfg(test)]
mod tests {
    //! U-43, U-45. Universe: every way a create can fail — refusals with
    //! literal 4xx and 5xx statuses (and their details), an answer lost after
    //! the send, a connection never made, and an ended session.
    use super::*;
    use proptest::prelude::*;

    const DEFINITE_STATUSES: [u16; 3] = [400, 403, 404];
    const AMBIGUOUS_STATUSES: [u16; 3] = [500, 502, 503];

    fn arb_failure() -> impl Strategy<Value = (RepoWriteError, bool)> {
        let detail = "[ -~]{0,24}";
        prop_oneof![
            (prop::sample::select(DEFINITE_STATUSES.to_vec()), detail)
                .prop_map(|(status, detail)| (RepoWriteError::Refused { status, detail }, false)),
            (prop::sample::select(AMBIGUOUS_STATUSES.to_vec()), detail)
                .prop_map(|(status, detail)| (RepoWriteError::Refused { status, detail }, true)),
            detail.prop_map(|detail| (RepoWriteError::AnswerLost { detail }, true)),
            detail.prop_map(|detail| (RepoWriteError::Unreachable { detail }, false)),
            Just((RepoWriteError::NoSession, false)),
            Just((RepoWriteError::SessionExpired, false)),
        ]
    }

    proptest! {
        /// Only a server error or a lost answer may have stored the record,
        /// and the app's knowledge after the failure follows that decision.
        #[test]
        fn only_a_server_error_or_a_lost_answer_may_have_stored_the_record(
            (failure, ambiguous) in arb_failure(),
        ) {
            prop_assert_eq!(may_have_stored(&failure), ambiguous, "{:?}", failure);
            let known = if ambiguous { FailedWrite::MayHaveStored } else { FailedWrite::Definite };
            prop_assert_eq!(FailedWrite::of(&failure), known, "{:?}", failure);
        }
    }
}
