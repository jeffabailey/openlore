//! `ReviewStateRead` / `ReviewStateWrite`: the owner's private suggestion
//! queue (`suggestions`, ADR-074). Every statement is scoped by `owner_did`;
//! a pending suggestion is never readable by another DID (I-BRA-1).

use duckdb::OptionalExt;
use duckdb::{params, Connection};
use ports::{
    KpiTally, OutsideDefault, RepoSource, ReviewStateRead, ReviewStateWrite, ReviewStoreError,
    Suggestion, SuggestionKey, SuggestionSource, SuggestionState,
};

use crate::secrets::port_error;
use crate::{db_error, db_id, in_transaction, ReviewStore, StoreError};

const READ_PENDING: &str = "SELECT subject, predicate, object, confidence_bp, evidence, signals,
    source_repo FROM suggestions WHERE owner_did = ? AND state = 'pending'
    ORDER BY subject, object, predicate";

const READ_PENDING_SOURCES: &str = "SELECT subject, predicate, object, source_kind,
    source_account_id, source_repo_id FROM suggestions
    WHERE owner_did = ? AND state = 'pending' AND source_kind IS NOT NULL";

const READ_STATES: &str = "SELECT subject, predicate, object, state
    FROM suggestions WHERE owner_did = ?";

/// The conditional insert (ADR-088 §7): the row is added only while its
/// source account is still listed for the owner, in the same statement, so a
/// removal committed first leaves nothing to insert.
const ADD_PENDING_FROM_LISTED_ACCOUNT: &str = "INSERT INTO suggestions
    (owner_did, subject, predicate, object, confidence_bp, evidence, signals, source_repo,
     state, first_suggested_at, state_changed_at, source_kind, source_account_id,
     source_repo_id, outside_default)
    SELECT ?, ?, ?, ?, ?, ?, ?, ?, 'pending', now(), now(), 'account', ?, ?, ?
    WHERE EXISTS (SELECT 1 FROM github_accounts WHERE owner_did = ? AND github_user_id = ?)
    ON CONFLICT DO NOTHING";

/// The conditional insert of an imported repo's suggestion (ADR-088 §7,
/// U-RRA-D9): added only while the import is still listed for the owner.
const ADD_PENDING_FROM_LISTED_IMPORT: &str = "INSERT INTO suggestions
    (owner_did, subject, predicate, object, confidence_bp, evidence, signals, source_repo,
     state, first_suggested_at, state_changed_at, source_kind, source_account_id,
     source_repo_id, outside_default)
    SELECT ?, ?, ?, ?, ?, ?, ?, ?, 'pending', now(), now(), 'imported', NULL, ?, NULL
    WHERE EXISTS (SELECT 1 FROM imported_repos WHERE owner_did = ? AND repo_id = ?)
    ON CONFLICT DO NOTHING";

const READ_OUTSIDE_DEFAULT: &str = "SELECT outside_default FROM suggestions
    WHERE owner_did = ? AND subject = ? AND predicate = ? AND object = ?";

const CHANGE_STATE: &str = "UPDATE suggestions SET state = ?, state_changed_at = now()
    WHERE owner_did = ? AND subject = ? AND predicate = ? AND object = ? AND state = ?";

fn to_json(values: &[String]) -> String {
    serde_json::Value::from(values.to_vec()).to_string()
}

fn from_json(stored: &str) -> Vec<String> {
    serde_json::from_str(stored).unwrap_or_default()
}

/// A stored source back as where the suggestion came from.
fn source_of(kind: &str, account: Option<i64>, repo: Option<i64>) -> Option<RepoSource> {
    let id = |stored: Option<i64>| stored.and_then(|id| u64::try_from(id).ok());
    match kind {
        "account" => id(account).map(RepoSource::Account),
        "imported" => id(repo).map(RepoSource::Imported),
        _ => None,
    }
}

fn not_a_state(stored: &str) -> StoreError {
    StoreError::Database(format!("unknown suggestion state {stored:?}"))
}

/// Insert one pending suggestion only while `source`'s account is listed
/// for the owner, in the caller's transaction (the port's committed one, or
/// the probe's rolled-back one). The count of rows it added: 0 or 1.
pub(crate) fn insert_if_listed(
    conn: &Connection,
    owner_did: &str,
    source: &SuggestionSource,
    s: &Suggestion,
) -> Result<usize, StoreError> {
    let account_id = db_id(source.account_id)?;
    let repo_id = db_id(source.repo_id)?;
    conn.execute(
        ADD_PENDING_FROM_LISTED_ACCOUNT,
        params![
            owner_did,
            s.key.subject,
            s.key.predicate,
            s.key.object,
            s.confidence_bp,
            to_json(&s.evidence),
            to_json(&s.why),
            s.source_repo,
            account_id,
            repo_id,
            source.outside_default.map(OutsideDefault::as_str),
            owner_did,
            account_id,
        ],
    )
    .map_err(db_error)
}

/// The conditional insert of one suggestion from the owner's import
/// `repo_id`: inserted only while the import is still listed (U-RRA-D9).
fn insert_if_imported(
    conn: &Connection,
    owner_did: &str,
    repo_id: u64,
    s: &Suggestion,
) -> Result<usize, StoreError> {
    let repo_id = db_id(repo_id)?;
    conn.execute(
        ADD_PENDING_FROM_LISTED_IMPORT,
        params![
            owner_did,
            s.key.subject,
            s.key.predicate,
            s.key.object,
            s.confidence_bp,
            to_json(&s.evidence),
            to_json(&s.why),
            s.source_repo,
            repo_id,
            owner_did,
            repo_id,
        ],
    )
    .map_err(db_error)
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

    fn pending_sources(
        &self,
        owner_did: &str,
    ) -> Result<Vec<(SuggestionKey, RepoSource)>, ReviewStoreError> {
        self.with_connection(|conn| {
            let mut statement = conn.prepare(READ_PENDING_SOURCES).map_err(db_error)?;
            let rows = statement
                .query_map([owner_did], |row| {
                    let key = SuggestionKey {
                        subject: row.get(0)?,
                        predicate: row.get(1)?,
                        object: row.get(2)?,
                    };
                    let kind: String = row.get(3)?;
                    let account: Option<i64> = row.get(4)?;
                    let repo: Option<i64> = row.get(5)?;
                    Ok((key, source_of(&kind, account, repo)))
                })
                .map_err(db_error)?;
            rows.filter_map(|row| match row {
                Ok((key, source)) => source.map(|source| Ok((key, source))),
                Err(error) => Some(Err(db_error(error))),
            })
            .collect()
        })
        .map_err(port_error)
    }

    fn outside_default(
        &self,
        owner_did: &str,
        key: &SuggestionKey,
    ) -> Result<Option<OutsideDefault>, ReviewStoreError> {
        self.with_connection(|conn| {
            conn.query_row(
                READ_OUTSIDE_DEFAULT,
                params![owner_did, key.subject, key.predicate, key.object],
                |row| row.get::<_, Option<String>>(0),
            )
            .optional()
            .map(|stored| stored.flatten().as_deref().and_then(OutsideDefault::parse))
            .map_err(db_error)
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
    fn add_pending_from_listed_source(
        &self,
        owner_did: &str,
        source: &SuggestionSource,
        suggestions: &[Suggestion],
    ) -> Result<(), ReviewStoreError> {
        self.with_connection(|conn| {
            in_transaction(conn, |conn| {
                suggestions
                    .iter()
                    .try_for_each(|s| insert_if_listed(conn, owner_did, source, s).map(drop))
            })
        })
        .map_err(port_error)
    }

    fn add_pending_from_import(
        &self,
        owner_did: &str,
        repo_id: u64,
        suggestions: &[Suggestion],
    ) -> Result<(), ReviewStoreError> {
        self.with_connection(|conn| {
            in_transaction(conn, |conn| {
                suggestions
                    .iter()
                    .try_for_each(|s| insert_if_imported(conn, owner_did, repo_id, s).map(drop))
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
        tally: KpiTally<'_>,
    ) -> Result<bool, ReviewStoreError> {
        self.with_tally(
            tally,
            |moved| *moved,
            |conn| {
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
            },
        )
        .map_err(port_error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DataKey;
    use ports::{GithubAccountsWrite, ScopeRemovalPort};
    use proptest::prelude::*;
    use std::collections::{BTreeMap, BTreeSet};

    const OWNERS: [&str; 2] = ["did:plc:priya", "did:plc:dmitri"];
    const SOURCES: [SuggestionSource; 2] = [source(7), source(8)];

    const fn source(account_id: u64) -> SuggestionSource {
        SuggestionSource {
            account_id,
            repo_id: 1,
            outside_default: None,
        }
    }

    fn cargo_suggestion() -> Suggestion {
        Suggestion {
            key: SuggestionKey {
                subject: "github:rust-lang/cargo".into(),
                predicate: "embodies".into(),
                object: "test-driven".into(),
            },
            confidence_bp: 7_000,
            evidence: vec!["https://github.com/rust-lang/cargo".into()],
            why: vec!["tests".into()],
            source_repo: "rust-lang/cargo".into(),
        }
    }

    /// An imported repo's suggestion is stored as imported, and only while
    /// the import is listed (U-RRA-D9): never for another owner's import.
    #[test]
    fn an_imported_suggestion_is_stored_as_imported_only_while_listed() {
        use ports::{ImportedRepo, ImportedReposWrite};
        let store = ReviewStore::open_in_memory(DataKey::generate()).unwrap();
        let cargo = ImportedRepo {
            repo_id: 2_001,
            full_name: "rust-lang/cargo".into(),
        };
        store.add_import(OWNERS[0], &cargo, KpiTally::NONE).unwrap();
        for owner in OWNERS {
            store
                .add_pending_from_import(owner, cargo.repo_id, &[cargo_suggestion()])
                .unwrap();
        }
        assert_eq!(
            store.pending_sources(OWNERS[0]).unwrap(),
            [(cargo_suggestion().key, RepoSource::Imported(2_001))]
        );
        assert!(store.pending_suggestions(OWNERS[1]).unwrap().is_empty());
    }

    /// A store where each owner lists their source account.
    fn store_listing_both() -> ReviewStore {
        let store = ReviewStore::open_in_memory(DataKey::generate()).unwrap();
        for (owner, source) in OWNERS.iter().zip(SOURCES) {
            store
                .add_or_reverify(owner, "some-login", source.account_id, 3, KpiTally::NONE)
                .unwrap();
        }
        store
    }

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
        fn each_owner_reads_only_their_own_queue_and_declines_and_undoes_only_their_own(
            adds in prop::collection::vec((0usize..2, suggestion()), 0..10),
            decline_pick in any::<prop::sample::Index>(),
        ) {
            let store = store_listing_both();
            let mut expected: [BTreeMap<SuggestionKey, Suggestion>; 2] = Default::default();
            for (owner, s) in &adds {
                store.add_pending_from_listed_source(OWNERS[*owner], &SOURCES[*owner], std::slice::from_ref(s)).unwrap();
                expected[*owner].entry(s.key.clone()).or_insert_with(|| s.clone());
            }
            for (owner, did) in OWNERS.iter().enumerate() {
                let pending: BTreeMap<SuggestionKey, Suggestion> = store
                    .pending_suggestions(did).unwrap().into_iter().map(|s| (s.key.clone(), s)).collect();
                prop_assert_eq!(&pending, &expected[owner]);
            }
            let other_before = store.pending_suggestions(OWNERS[1]).unwrap();
            if let Some(key) = expected[0].keys().nth(decline_pick.index(expected[0].len().max(1))).cloned() {
                prop_assert!(store.change_state(OWNERS[0], &key, SuggestionState::Pending, SuggestionState::Declined, KpiTally::NONE).unwrap());
                prop_assert!(!store.change_state(OWNERS[0], &key, SuggestionState::Pending, SuggestionState::Declined, KpiTally::NONE).unwrap());
                let pending: BTreeSet<SuggestionKey> = store
                    .pending_suggestions(OWNERS[0]).unwrap().into_iter().map(|s| s.key).collect();
                let mut remaining: BTreeSet<SuggestionKey> = expected[0].keys().cloned().collect();
                remaining.remove(&key);
                prop_assert_eq!(pending, remaining);
                let states: BTreeMap<SuggestionKey, SuggestionState> =
                    store.suggestion_states(OWNERS[0]).unwrap().into_iter().collect();
                prop_assert_eq!(states.get(&key), Some(&SuggestionState::Declined));
                prop_assert!(store.change_state(OWNERS[0], &key, SuggestionState::Declined, SuggestionState::Pending, KpiTally::NONE).unwrap());
                let restored: BTreeSet<SuggestionKey> = store
                    .pending_suggestions(OWNERS[0]).unwrap().into_iter().map(|s| s.key).collect();
                prop_assert_eq!(restored, expected[0].keys().cloned().collect::<BTreeSet<_>>());
            }
            prop_assert_eq!(store.pending_suggestions(OWNERS[1]).unwrap(), other_before);
        }
    }

    // bypass: one adapter example over a real store for the race ADR-088 §7
    // closes, in both commit orders — a removal committed before the insert
    // leaves nothing to insert, one committed after deletes what it added;
    // either way no pending row of the removed account remains, and another
    // owner's insert from a listed account still lands.
    #[test]
    fn a_removed_accounts_suggestions_are_never_left_pending_whichever_commits_first() {
        let fixture = Suggestion {
            key: SuggestionKey {
                subject: "github:p/tidepool".into(),
                predicate: "embodiesPhilosophy".into(),
                object: "org.openlore.philosophy.test-driven".into(),
            },
            confidence_bp: 7000,
            evidence: vec!["e".into()],
            why: vec!["w".into()],
            source_repo: "p/tidepool".into(),
        };
        let batch = std::slice::from_ref(&fixture);
        for removal_first in [true, false] {
            let store = store_listing_both();
            let [priya, dmitri] = OWNERS;
            let remove = || {
                store
                    .remove_account(priya, SOURCES[0].account_id, KpiTally::NONE)
                    .unwrap()
            };
            if removal_first {
                remove();
            }
            store
                .add_pending_from_listed_source(priya, &SOURCES[0], batch)
                .unwrap();
            if !removal_first {
                remove();
            }
            store
                .add_pending_from_listed_source(dmitri, &SOURCES[1], batch)
                .unwrap();

            assert!(
                store.pending_suggestions(priya).unwrap().is_empty(),
                "removal first: {removal_first}"
            );
            assert_eq!(
                store.pending_suggestions(dmitri).unwrap(),
                vec![fixture.clone()]
            );
            assert!(store
                .add_pending_from_listed_source(priya, &SOURCES[0], batch)
                .is_ok());
            assert!(
                store.pending_suggestions(priya).unwrap().is_empty(),
                "unlisted inserts nothing"
            );
        }
    }
}
