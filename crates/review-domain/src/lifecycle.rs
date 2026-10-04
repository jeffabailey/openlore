//! The suggestion lifecycle (data-models §5): a suggestion is born pending
//! from a derived candidate, at the candidate's confidence (SPIKE finding 5:
//! 0.25), and moves only by the owner's own actions.

use ports::{CandidateClaim, Suggestion, SuggestionKey, SuggestionState};

/// Basis points in 1.0 (BR-4: confidence is stored as basis points).
pub const BASIS_POINTS: u16 = 10_000;

/// A candidate's confidence in basis points, clamped to `0..=10000`.
pub fn basis_points_of(confidence: f64) -> u16 {
    let clamped = confidence.clamp(0.0, 1.0);
    // In range by the clamp: 0.0..=10000.0 always fits a u16.
    (clamped * f64::from(BASIS_POINTS)).round() as u16
}

/// The pending suggestion a derived candidate becomes, from `source_repo`.
pub fn suggestion_from_candidate(candidate: &CandidateClaim, source_repo: &str) -> Suggestion {
    Suggestion {
        key: SuggestionKey {
            subject: candidate.subject.clone(),
            predicate: candidate.predicate.clone(),
            object: candidate.object.clone(),
        },
        confidence_bp: basis_points_of(candidate.confidence),
        evidence: candidate.evidence.clone(),
        why: candidate
            .source_signals()
            .iter()
            .map(|signal| signal.value.clone())
            .collect(),
        source_repo: source_repo.to_string(),
    }
}

/// "Not me": only a pending suggestion can be declined.
pub fn decline(state: SuggestionState) -> Option<SuggestionState> {
    match state {
        SuggestionState::Pending => Some(SuggestionState::Declined),
        SuggestionState::Declined | SuggestionState::Published | SuggestionState::Retracted => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ports::{Signal, SignalKind};
    use proptest::prelude::*;

    fn state() -> impl Strategy<Value = SuggestionState> {
        proptest::sample::select(SuggestionState::ALL.to_vec())
    }

    proptest! {
        /// Universe: every state. Declining moves pending → declined and
        /// refuses every other state (nothing else changes).
        #[test]
        fn only_a_pending_suggestion_can_be_declined(from in state()) {
            let expected = (from == SuggestionState::Pending).then_some(SuggestionState::Declined);
            prop_assert_eq!(decline(from), expected);
        }

        /// Universe: every confidence (in and out of range). Basis points
        /// stay within 0..=10000 and every hundredth maps exactly.
        #[test]
        fn basis_points_stay_in_range_and_hundredths_are_exact(
            confidence in proptest::num::f64::ANY, hundredths in 0u16..=100,
        ) {
            prop_assert!(basis_points_of(confidence) <= BASIS_POINTS);
            prop_assert_eq!(basis_points_of(f64::from(hundredths) / 100.0), hundredths * 100);
        }

        /// Universe: candidates. The suggestion keeps the candidate's key and
        /// evidence, names every producing signal, and records its repo.
        #[test]
        fn a_suggestion_keeps_its_candidates_key_evidence_and_signals(
            repo in "[a-z]{1,8}/[a-z]{1,8}", whys in prop::collection::vec("[a-z .]{1,20}", 1..4),
        ) {
            let signals: Vec<Signal> = whys.iter().map(|why| Signal {
                kind: SignalKind::DependencyManifestPinned,
                value: why.clone(),
                source_url: format!("https://github.com/{repo}/blob/main/Cargo.lock"),
            }).collect();
            let candidate = CandidateClaim::try_new(
                format!("github:{repo}"),
                "embodiesPhilosophy".into(),
                "org.openlore.philosophy.dependency-pinning".into(),
                signals.iter().map(|s| s.source_url.clone()).collect(),
                0.25,
                signals.clone(),
            ).unwrap();
            let suggestion = suggestion_from_candidate(&candidate, &repo);
            prop_assert_eq!(&suggestion.key.subject, &candidate.subject);
            prop_assert_eq!(&suggestion.key.object, &candidate.object);
            prop_assert_eq!(&suggestion.evidence, &candidate.evidence);
            prop_assert_eq!(&suggestion.why, &whys);
            prop_assert_eq!(suggestion.confidence_bp, 2500);
            prop_assert_eq!(&suggestion.source_repo, &repo);
        }
    }
}
