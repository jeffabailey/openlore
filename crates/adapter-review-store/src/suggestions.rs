//! `ReviewStateRead` / `ReviewStateWrite`: the owner's private suggestion
//! queue (`suggestions`, ADR-074). Every statement is scoped by `owner_did`;
//! a pending suggestion is never readable by another DID (I-BRA-1).

use duckdb::params;
use ports::{
    ReviewStateRead, ReviewStateWrite, ReviewStoreError, Suggestion, SuggestionKey, SuggestionState,
};

use crate::secrets::port_error;
use crate::{db_error, ReviewStore, StoreError};

const READ_PENDING: &str = "SELECT subject, predicate, object, confidence_bp, evidence, signals,
    source_repo FROM suggestions WHERE owner_did = ? AND state = 'pending'
    ORDER BY subject, object, predicate";

const READ_STATES: &str = "SELECT subject, predicate, object, state
    FROM suggestions WHERE owner_did = ?";

const ADD_PENDING: &str = "INSERT INTO suggestions
    (owner_did, subject, predicate, object, confidence_bp, evidence, signals, source_repo,
     state, first_suggested_at, state_changed_at)
    VALUES (?, ?, ?, ?, ?, ?, ?, ?, 'pending', now(), now())
    ON CONFLICT DO NOTHING";

const CHANGE_STATE: &str = "UPDATE suggestions SET state = ?, state_changed_at = now()
    WHERE owner_did = ? AND subject = ? AND predicate = ? AND object = ? AND state = ?";

fn to_json(values: &[String]) -> String {
    serde_json::Value::from(values.to_vec()).to_string()
}

fn from_json(stored: &str) -> Vec<String> {
    serde_json::from_str(stored).unwrap_or_default()
}

fn not_a_state(stored: &str) -> StoreError {
    StoreError::Database(format!("unknown suggestion state {stored:?}"))
}

impl ReviewStateRead for ReviewStore {
    fn pending_suggestions(&self, owner_did: &str) -> Result<Vec<Suggestion>, ReviewStoreError> {
        self.with_connection(|conn| {
            let mut statement = conn.prepare(READ_PENDING).map_err(db_error)?;
            let rows = statement
                .query_map([owner_did], |row| {
                    Ok(Suggestion {
                        key: SuggestionKey {
                            subject: row.get(0)?,
                            predicate: row.get(1)?,
                            object: row.get(2)?,
                        },
                        confidence_bp: row.get(3)?,
                        evidence: from_json(&row.get::<_, String>(4)?),
                        why: from_json(&row.get::<_, String>(5)?),
                        source_repo: row.get(6)?,
                    })
                })
                .map_err(db_error)?;
            rows.collect::<Result<Vec<_>, _>>().map_err(db_error)
        })
        .map_err(port_error)
    }

    fn suggestion_states(
        &self,
        owner_did: &str,
    ) -> Result<Vec<(SuggestionKey, SuggestionState)>, ReviewStoreError> {
        self.with_connection(|conn| {
            let mut statement = conn.prepare(READ_STATES).map_err(db_error)?;
            let rows = statement
                .query_map([owner_did], |row| {
                    let key = SuggestionKey {
                        subject: row.get(0)?,
                        predicate: row.get(1)?,
                        object: row.get(2)?,
                    };
                    Ok((key, row.get::<_, String>(3)?))
                })
                .map_err(db_error)?;
            rows.map(|row| {
                let (key, stored) = row.map_err(db_error)?;
                let state = SuggestionState::parse(&stored).ok_or_else(|| not_a_state(&stored))?;
                Ok((key, state))
            })
            .collect()
        })
        .map_err(port_error)
    }
}

impl ReviewStateWrite for ReviewStore {
    fn add_pending(
        &self,
        owner_did: &str,
        suggestions: &[Suggestion],
    ) -> Result<(), ReviewStoreError> {
        self.with_connection(|conn| {
            suggestions.iter().try_for_each(|s| {
                conn.execute(
                    ADD_PENDING,
                    params![
                        owner_did,
                        s.key.subject,
                        s.key.predicate,
                        s.key.object,
                        s.confidence_bp,
                        to_json(&s.evidence),
                        to_json(&s.why),
                        s.source_repo,
                    ],
                )
                .map(drop)
                .map_err(db_error)
            })
        })
        .map_err(port_error)
    }

    fn change_state(
        &self,
        owner_did: &str,
        key: &SuggestionKey,
        from: SuggestionState,
        to: SuggestionState,
    ) -> Result<bool, ReviewStoreError> {
        self.with_connection(|conn| {
            conn.execute(
                CHANGE_STATE,
                params![
                    to.as_str(),
                    owner_did,
                    key.subject,
                    key.predicate,
                    key.object,
                    from.as_str()
                ],
            )
            .map(|changed| changed == 1)
            .map_err(db_error)
        })
        .map_err(port_error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DataKey;
    use proptest::prelude::*;
    use std::collections::{BTreeMap, BTreeSet};

    const OWNERS: [&str; 2] = ["did:plc:priya", "did:plc:dmitri"];

    fn suggestion() -> impl Strategy<Value = Suggestion> {
        (
            prop::sample::select(vec![
                "github:p/tidepool",
                "github:p/quill",
                "github:d/ferrite",
            ]),
            prop::sample::select(vec!["dependency-pinning", "memory-safety", "test-driven"]),
            0u16..=10_000,
            prop::collection::vec("https://github.com/[a-z]{1,6}/[a-z]{1,6}", 1..3),
            prop::collection::vec("[A-Za-z .]{1,16}", 1..3),
        )
            .prop_map(|(subject, slug, confidence_bp, evidence, why)| Suggestion {
                source_repo: subject.trim_start_matches("github:").to_string(),
                key: SuggestionKey {
                    subject: subject.into(),
                    predicate: "embodiesPhilosophy".into(),
                    object: format!("org.openlore.philosophy.{slug}"),
                },
                confidence_bp,
                evidence,
                why,
            })
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(24))]

        /// Universe: the suggestions rows of two owners, after adds by both
        /// and a decline by the first. Each owner reads back exactly their
        /// own keys (first add of a key wins, field for field); a decline
        /// moves only that owner's key, and only out of pending; the other
        /// owner's queue is untouched.
        #[test]
        fn each_owner_reads_only_their_own_queue_and_declines_only_their_own(
            adds in prop::collection::vec((0usize..2, suggestion()), 0..10),
            decline_pick in any::<prop::sample::Index>(),
        ) {
            let store = ReviewStore::open_in_memory(DataKey::generate()).unwrap();
            let mut expected: [BTreeMap<SuggestionKey, Suggestion>; 2] = Default::default();
            for (owner, s) in &adds {
                store.add_pending(OWNERS[*owner], std::slice::from_ref(s)).unwrap();
                expected[*owner].entry(s.key.clone()).or_insert_with(|| s.clone());
            }
            for (owner, did) in OWNERS.iter().enumerate() {
                let pending: BTreeMap<SuggestionKey, Suggestion> = store
                    .pending_suggestions(did).unwrap().into_iter().map(|s| (s.key.clone(), s)).collect();
                prop_assert_eq!(&pending, &expected[owner]);
            }
            let other_before = store.pending_suggestions(OWNERS[1]).unwrap();
            if let Some(key) = expected[0].keys().nth(decline_pick.index(expected[0].len().max(1))).cloned() {
                prop_assert!(store.change_state(OWNERS[0], &key, SuggestionState::Pending, SuggestionState::Declined).unwrap());
                prop_assert!(!store.change_state(OWNERS[0], &key, SuggestionState::Pending, SuggestionState::Declined).unwrap());
                let pending: BTreeSet<SuggestionKey> = store
                    .pending_suggestions(OWNERS[0]).unwrap().into_iter().map(|s| s.key).collect();
                let mut remaining: BTreeSet<SuggestionKey> = expected[0].keys().cloned().collect();
                remaining.remove(&key);
                prop_assert_eq!(pending, remaining);
                let states: BTreeMap<SuggestionKey, SuggestionState> =
                    store.suggestion_states(OWNERS[0]).unwrap().into_iter().collect();
                prop_assert_eq!(states.get(&key), Some(&SuggestionState::Declined));
            }
            prop_assert_eq!(store.pending_suggestions(OWNERS[1]).unwrap(), other_before);
        }
    }
}
