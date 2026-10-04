//! Reconcile (BR-1 / BR-2): combine the keys the owner already holds with
//! the suggestions a scan just derived. Only keys never seen before (in any
//! state) become new pending suggestions; already-published and declined
//! keys are counted, never re-offered.

use std::collections::{BTreeMap, BTreeSet};

use ports::{ScanCounts, Suggestion, SuggestionKey, SuggestionState};

/// What a scan adds, and what it found already settled.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reconciled {
    /// New pending suggestions, one per never-seen key.
    pub new: Vec<Suggestion>,
    /// Derived keys already published (or published then retracted).
    pub already_published: usize,
    /// Derived keys the owner declined; they stay hidden.
    pub declined_hidden: usize,
}

impl Reconciled {
    /// This reconcile's contribution to its scan's summary (DWD-11).
    pub fn counts(&self) -> ScanCounts {
        ScanCounts {
            new: self.new.len(),
            already_published: self.already_published,
            declined_hidden: self.declined_hidden,
        }
    }
}

/// Two parts of one scan, counted together. A scan reconciles repo by repo;
/// each repo derives keys of its own subject, so the parts never overlap.
pub fn add_counts(left: ScanCounts, right: ScanCounts) -> ScanCounts {
    ScanCounts {
        new: left.new + right.new,
        already_published: left.already_published + right.already_published,
        declined_hidden: left.declined_hidden + right.declined_hidden,
    }
}

/// Reconcile `derived` against the owner's `existing` keys.
pub fn reconcile(
    existing: &BTreeMap<SuggestionKey, SuggestionState>,
    derived: Vec<Suggestion>,
) -> Reconciled {
    let derived_keys: BTreeSet<&SuggestionKey> = derived.iter().map(|s| &s.key).collect();
    let count_in = |wanted: &[SuggestionState]| {
        derived_keys
            .iter()
            .filter(|key| {
                existing
                    .get(**key)
                    .is_some_and(|state| wanted.contains(state))
            })
            .count()
    };
    let already_published = count_in(&[SuggestionState::Published, SuggestionState::Retracted]);
    let declined_hidden = count_in(&[SuggestionState::Declined]);
    Reconciled {
        new: first_of_each_unseen_key(existing, derived),
        already_published,
        declined_hidden,
    }
}

/// The first suggestion for each key absent from `existing`, in order.
fn first_of_each_unseen_key(
    existing: &BTreeMap<SuggestionKey, SuggestionState>,
    derived: Vec<Suggestion>,
) -> Vec<Suggestion> {
    let mut seen: BTreeSet<SuggestionKey> = existing.keys().cloned().collect();
    derived
        .into_iter()
        .filter(|suggestion| seen.insert(suggestion.key.clone()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn key() -> impl Strategy<Value = SuggestionKey> {
        (
            prop::sample::select(vec![
                "github:p/tidepool",
                "github:p/quill",
                "github:d/ferrite",
            ]),
            prop::sample::select(vec![
                "org.openlore.philosophy.dependency-pinning",
                "org.openlore.philosophy.memory-safety",
                "org.openlore.philosophy.test-driven",
            ]),
        )
            .prop_map(|(subject, object)| SuggestionKey {
                subject: subject.into(),
                predicate: "embodiesPhilosophy".into(),
                object: object.into(),
            })
    }

    fn state() -> impl Strategy<Value = SuggestionState> {
        prop::sample::select(SuggestionState::ALL.to_vec())
    }

    fn suggestion(key: SuggestionKey) -> Suggestion {
        Suggestion {
            source_repo: key.subject.trim_start_matches("github:").to_string(),
            key,
            confidence_bp: 2500,
            evidence: vec!["https://github.com/p/tidepool/blob/main/Cargo.lock".into()],
            why: vec!["Cargo.lock committed".into()],
        }
    }

    proptest! {
        /// Universe: (existing keys + states, derived suggestions). New =
        /// exactly the derived keys never seen, once each; the counts are
        /// the derived keys already published/retracted and declined; and
        /// reconciling again after storing the new ones adds nothing.
        #[test]
        fn reconcile_offers_only_unseen_keys_once_and_is_idempotent(
            existing in prop::collection::btree_map(key(), state(), 0..9),
            derived_keys in prop::collection::vec(key(), 0..12),
        ) {
            let derived: Vec<Suggestion> = derived_keys.iter().cloned().map(suggestion).collect();
            let out = reconcile(&existing, derived.clone());
            let new_keys: Vec<&SuggestionKey> = out.new.iter().map(|s| &s.key).collect();
            let distinct: BTreeSet<&SuggestionKey> = derived_keys.iter().collect();
            let expected: BTreeSet<&SuggestionKey> =
                distinct.iter().copied().filter(|k| !existing.contains_key(*k)).collect();
            prop_assert_eq!(new_keys.len(), expected.len(), "each new key once");
            prop_assert_eq!(new_keys.iter().copied().collect::<BTreeSet<_>>(), expected);
            let published = distinct.iter().filter(|k| matches!(
                existing.get(**k), Some(SuggestionState::Published | SuggestionState::Retracted))).count();
            let declined = distinct.iter().filter(|k| existing.get(**k) == Some(&SuggestionState::Declined)).count();
            prop_assert_eq!(out.already_published, published);
            prop_assert_eq!(out.declined_hidden, declined);
            let mut after = existing.clone();
            after.extend(out.new.iter().map(|s| (s.key.clone(), SuggestionState::Pending)));
            prop_assert!(reconcile(&after, derived).new.is_empty());
        }

        /// Universe: (existing keys + states, derived suggestions split at
        /// an arbitrary subject boundary). Reconciling repo by repo — each
        /// part stored before the next — counts exactly what reconciling
        /// the whole scan at once counts, so a scan's summary covers only
        /// the keys that scan derived, each once.
        #[test]
        fn a_scan_counted_repo_by_repo_counts_the_whole_scan(
            existing in prop::collection::btree_map(key(), state(), 0..9),
            derived_keys in prop::collection::vec(key(), 0..12),
        ) {
            let (first, second): (Vec<SuggestionKey>, Vec<SuggestionKey>) =
                derived_keys.iter().cloned().partition(|k| k.subject == "github:p/tidepool");
            let whole = reconcile(&existing, derived_keys.iter().cloned().map(suggestion).collect());
            let first_part = reconcile(&existing, first.into_iter().map(suggestion).collect());
            let mut stored = existing.clone();
            stored.extend(first_part.new.iter().map(|s| (s.key.clone(), SuggestionState::Pending)));
            let second_part = reconcile(&stored, second.into_iter().map(suggestion).collect());
            prop_assert_eq!(add_counts(first_part.counts(), second_part.counts()), whole.counts());
            prop_assert_eq!(add_counts(ScanCounts::default(), whole.counts()), whole.counts());
        }
    }
}
