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

/// What the owner can do to a suggestion (data-models §5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LifecycleEvent {
    /// "Not me".
    Decline,
    /// Undo a decline.
    Undo,
    /// Publish to the owner's repo.
    Publish,
    /// Retract a published claim.
    Retract,
}

impl LifecycleEvent {
    pub const ALL: [LifecycleEvent; 4] = [Self::Decline, Self::Undo, Self::Publish, Self::Retract];

    /// The state this event leaves a suggestion in.
    pub fn settles_in(self) -> SuggestionState {
        match self {
            Self::Decline => SuggestionState::Declined,
            Self::Undo => SuggestionState::Pending,
            Self::Publish => SuggestionState::Published,
            Self::Retract => SuggestionState::Retracted,
        }
    }

    /// The only state this event may start from.
    fn starts_from(self) -> SuggestionState {
        match self {
            Self::Decline | Self::Publish => SuggestionState::Pending,
            Self::Undo => SuggestionState::Declined,
            Self::Retract => SuggestionState::Published,
        }
    }
}

/// The lifecycle state machine: pending ⇄ declined, pending → published →
/// retracted. Every other event is refused in every state.
pub fn transition(state: SuggestionState, event: LifecycleEvent) -> Option<SuggestionState> {
    (state == event.starts_from()).then(|| event.settles_in())
}

/// "Not me": only a pending suggestion can be declined.
pub fn decline(state: SuggestionState) -> Option<SuggestionState> {
    transition(state, LifecycleEvent::Decline)
}

/// What the owner's action does to a stored suggestion: move it, or nothing
/// because it is already where the action leaves it (a repeated "Not me" or
/// Undo is the same as one, C4a). `None`: the action is refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OwnerStep {
    Move {
        from: SuggestionState,
        to: SuggestionState,
    },
    AlreadyDone,
}

/// The step `event` takes from `current`.
pub fn owner_step(current: SuggestionState, event: LifecycleEvent) -> Option<OwnerStep> {
    match transition(current, event) {
        Some(to) => Some(OwnerStep::Move { from: current, to }),
        None => (current == event.settles_in()).then_some(OwnerStep::AlreadyDone),
    }
}

/// May the owner see and approve a suggestion in `state` right now? Only
/// while it is pending AND their GitHub link is currently verified (D-12,
/// CORE-9). Visibility is derived, never stored: an unproven link hides a
/// pending suggestion without changing it, and proof restores it.
pub fn visible_and_approvable(state: SuggestionState, link_verified: bool) -> bool {
    state == SuggestionState::Pending && link_verified
}

/// How many of the owner's suggestions stand in each state.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Tally {
    pub pending: usize,
    pub declined: usize,
    pub published: usize,
    pub retracted: usize,
}

impl Tally {
    /// Has the owner settled suggestions, and none is left pending?
    pub fn all_reviewed(&self) -> bool {
        self.pending == 0 && self.declined + self.published + self.retracted > 0
    }
}

/// Count `states` by state.
pub fn tally(states: impl IntoIterator<Item = SuggestionState>) -> Tally {
    states
        .into_iter()
        .fold(Tally::default(), |mut tally, state| {
            match state {
                SuggestionState::Pending => tally.pending += 1,
                SuggestionState::Declined => tally.declined += 1,
                SuggestionState::Published => tally.published += 1,
                SuggestionState::Retracted => tally.retracted += 1,
            }
            tally
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use ports::{Signal, SignalKind};
    use proptest::prelude::*;

    fn state() -> impl Strategy<Value = SuggestionState> {
        proptest::sample::select(SuggestionState::ALL.to_vec())
    }

    fn event() -> impl Strategy<Value = LifecycleEvent> {
        proptest::sample::select(LifecycleEvent::ALL.to_vec())
    }

    proptest! {
        /// Universe: (state, event) pairs. Exactly the four legal moves
        /// (pending ⇄ declined, pending → published → retracted) succeed;
        /// in particular declined → published is refused.
        #[test]
        fn only_the_legal_moves_succeed(from in state(), on in event()) {
            use LifecycleEvent::*;
            use SuggestionState::*;
            let expected = match (from, on) {
                (Pending, Decline) => Some(Declined),
                (Declined, Undo) => Some(Pending),
                (Pending, Publish) => Some(Published),
                (Published, Retract) => Some(Retracted),
                _ => None,
            };
            prop_assert_eq!(transition(from, on), expected);
        }

        /// Universe: (state, event). Taking the owner's step twice leaves the
        /// suggestion where taking it once did; a refused step moves nothing.
        #[test]
        fn an_owner_step_taken_twice_equals_once(from in state(), on in event()) {
            let apply = |current: SuggestionState| match owner_step(current, on) {
                Some(OwnerStep::Move { from: was, to }) => {
                    prop_assert_eq!(was, current);
                    Ok(Some(to))
                }
                Some(OwnerStep::AlreadyDone) => Ok(Some(current)),
                None => Ok(None),
            };
            match apply(from)? {
                Some(once) => {
                    prop_assert_eq!(once, on.settles_in());
                    prop_assert_eq!(apply(once)?, Some(once));
                }
                None => prop_assert!(transition(from, on).is_none() && from != on.settles_in()),
            }
        }

        /// Universe: lists of states. The tally counts each state exactly,
        /// and "all reviewed" holds iff nothing is pending and something is
        /// settled.
        #[test]
        fn the_tally_counts_every_state(states in prop::collection::vec(state(), 0..12)) {
            let counted = tally(states.iter().copied());
            let count = |wanted| states.iter().filter(|s| **s == wanted).count();
            prop_assert_eq!(counted.pending, count(SuggestionState::Pending));
            prop_assert_eq!(counted.declined, count(SuggestionState::Declined));
            prop_assert_eq!(counted.published, count(SuggestionState::Published));
            prop_assert_eq!(counted.retracted, count(SuggestionState::Retracted));
            prop_assert_eq!(
                counted.all_reviewed(),
                counted.pending == 0 && !states.is_empty()
            );
        }

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

        /// Universe: a queue of suggestion states × the link verdict. An
        /// unproven link hides every suggestion without changing any state;
        /// proving it again shows exactly the pending ones — nothing
        /// declined, published or retracted is ever approvable.
        #[test]
        fn only_pending_suggestions_of_a_verified_link_are_approvable(
            queue in prop::collection::vec(state(), 0..12),
        ) {
            prop_assert!(queue.iter().all(|s| !visible_and_approvable(*s, false)));
            let shown: Vec<SuggestionState> =
                queue.iter().copied().filter(|s| visible_and_approvable(*s, true)).collect();
            let pending: Vec<SuggestionState> =
                queue.iter().copied().filter(|s| *s == SuggestionState::Pending).collect();
            prop_assert_eq!(shown, pending);
        }
    }
}
