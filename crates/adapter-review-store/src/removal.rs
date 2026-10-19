//! `ScopeRemovalPort` (ADR-088 §8, D-RRA-17): removing an account deletes
//! its `github_accounts` row, its `repo_choices` rows (re-adding starts
//! fresh, US-RRA-008) and its `pending` suggestions, in ONE transaction;
//! removing an import deletes its `imported_repos` row and its `pending`
//! suggestions the same way (US-RRA-009).
//! The `DELETE` on `suggestions` names `state = 'pending'`: declined,
//! published and retracted keys stay (BR-1, BR-2). The only `DELETE` outside
//! purge and expiry (check-arch `review_store_owner_scoped_sql`).

use duckdb::OptionalExt;
use duckdb::{params, Connection};
use ports::{AccountRemoval, ImportRemoval, KpiTally, ReviewStoreError, ScopeRemovalPort};

use crate::secrets::port_error;
use crate::{db_error, db_id, ReviewStore, StoreError};

const DELETE_ACCOUNT: &str = "DELETE FROM github_accounts
    WHERE owner_did = ? AND github_user_id = ?";

const DELETE_ACCOUNT_CHOICE: &str = "DELETE FROM repo_choices
    WHERE owner_did = ? AND github_user_id = ?";

/// An import leaves "Repos you don't own" (a BR-RRA-5 move, `imports.rs`).
pub(crate) const DELETE_IMPORT: &str = "DELETE FROM imported_repos
    WHERE owner_did = ? AND repo_id = ?";

const DELETE_ACCOUNT_PENDING: &str = "DELETE FROM suggestions
    WHERE owner_did = ? AND state = 'pending' AND source_kind = 'account'
      AND source_account_id = ?";

/// The owner's import as removal names it: as seen when added, and how
/// long ago that was.
const READ_IMPORT_HELD: &str = "SELECT full_name_seen,
        greatest(CAST(epoch(now()::TIMESTAMP - added_at::TIMESTAMP) AS BIGINT), 0)
    FROM imported_repos WHERE owner_did = ? AND repo_id = ?";

const DELETE_IMPORT_PENDING: &str = "DELETE FROM suggestions
    WHERE owner_did = ? AND state = 'pending' AND source_kind = 'imported'
      AND source_repo_id = ?";

/// The import removal's statements, run in the caller's transaction. A
/// repo the owner does not import removes nothing.
fn remove_import_rows(
    conn: &Connection,
    owner_did: &str,
    repo_id: u64,
) -> Result<ImportRemoval, StoreError> {
    let repo = db_id(repo_id)?;
    let held: Option<(String, i64)> = conn
        .query_row(READ_IMPORT_HELD, params![owner_did, repo], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })
        .optional()
        .map_err(db_error)?;
    let Some((full_name, held_secs)) = held else {
        return Ok(ImportRemoval::NotImported);
    };
    let delete = |sql: &str| {
        conn.execute(sql, params![owner_did, repo])
            .map_err(db_error)
    };
    delete(DELETE_IMPORT)?;
    let deleted_pending = delete(DELETE_IMPORT_PENDING)?;
    Ok(ImportRemoval::Removed {
        full_name,
        deleted_pending,
        held_secs: u64::try_from(held_secs).unwrap_or_default(),
    })
}

/// The removal's statements, run in the caller's transaction (the port's
/// committed one, or the probe's rolled-back one). An account the owner
/// does not list removes nothing.
pub(crate) fn remove_account_rows(
    conn: &Connection,
    owner_did: &str,
    github_user_id: u64,
) -> Result<AccountRemoval, StoreError> {
    let account = db_id(github_user_id)?;
    let delete = |sql: &str| {
        conn.execute(sql, params![owner_did, account])
            .map_err(db_error)
    };
    if delete(DELETE_ACCOUNT)? == 0 {
        return Ok(AccountRemoval::NotListed);
    }
    delete(DELETE_ACCOUNT_CHOICE)?;
    let deleted_pending = delete(DELETE_ACCOUNT_PENDING)?;
    Ok(AccountRemoval::Removed { deleted_pending })
}

impl ScopeRemovalPort for ReviewStore {
    fn remove_account(
        &self,
        owner_did: &str,
        github_user_id: u64,
        tally: KpiTally<'_>,
    ) -> Result<AccountRemoval, ReviewStoreError> {
        self.with_tally(
            tally,
            |removal| matches!(removal, AccountRemoval::Removed { .. }),
            |conn| remove_account_rows(conn, owner_did, github_user_id),
        )
        .map_err(port_error)
    }

    fn remove_import(
        &self,
        owner_did: &str,
        repo_id: u64,
        tally: KpiTally<'_>,
    ) -> Result<ImportRemoval, ReviewStoreError> {
        self.with_tally(
            tally,
            |removal| matches!(removal, ImportRemoval::Removed { .. }),
            |conn| remove_import_rows(conn, owner_did, repo_id),
        )
        .map_err(port_error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DataKey;
    use ports::{
        GithubAccountsRead, GithubAccountsWrite, ImportedRepo, ImportedReposRead,
        ImportedReposWrite, KpiCounterPort, ReviewStateRead, ReviewStateWrite, Suggestion,
        SuggestionKey, SuggestionSource, SuggestionState,
    };

    const PRIYA: &str = "did:plc:priyapriyapriya";
    const SAM: &str = "did:plc:samsamsamsamsam";
    const TIDAL: u64 = 7;

    fn suggestion(subject: &str) -> Suggestion {
        Suggestion {
            key: SuggestionKey {
                subject: subject.into(),
                predicate: "embodies".into(),
                object: "test-driven".into(),
            },
            confidence_bp: 7000,
            evidence: vec!["e".into()],
            why: vec!["w".into()],
            source_repo: subject.into(),
        }
    }

    /// Seed `owner`'s account `account` with four suggestions; two are then
    /// declined and published.
    fn seed(store: &ReviewStore, owner: &str, account: u64) {
        store
            .add_or_reverify(owner, "some-login", account, 3, KpiTally::NONE)
            .unwrap();
        let source = SuggestionSource {
            account_id: account,
            repo_id: 1,
            outside_default: None,
        };
        let batch: Vec<Suggestion> = ["a", "b", "declined", "published"]
            .into_iter()
            .map(suggestion)
            .collect();
        store
            .add_pending_from_listed_source(owner, &source, &batch)
            .unwrap();
        for (subject, to) in [
            ("declined", SuggestionState::Declined),
            ("published", SuggestionState::Published),
        ] {
            store
                .change_state(
                    owner,
                    &suggestion(subject).key,
                    SuggestionState::Pending,
                    to,
                    KpiTally::NONE,
                )
                .unwrap();
        }
    }

    // bypass: one adapter example over a real store — the port deletes the
    // owner's account and its pending suggestions at once, keeps declined
    // and published keys, counts once, and leaves another owner untouched.
    #[test]
    fn removing_an_account_deletes_only_its_owners_pending_suggestions() {
        let store = ReviewStore::open_in_memory(DataKey::generate()).unwrap();
        seed(&store, PRIYA, TIDAL);
        seed(&store, SAM, TIDAL + 1);
        let sams_before = store.suggestion_states(SAM).unwrap();
        let tally = KpiTally {
            day: "2026-10-19",
            events: &["account.removed"],
        };

        let refused = store.remove_account(SAM, TIDAL, tally).unwrap();
        let removal = store.remove_account(PRIYA, TIDAL, tally).unwrap();
        let again = store.remove_account(PRIYA, TIDAL, tally).unwrap();

        assert_eq!(removal, AccountRemoval::Removed { deleted_pending: 2 });
        assert_eq!(again, AccountRemoval::NotListed);
        assert_eq!(refused, AccountRemoval::NotListed, "not Sam's account");
        assert!(store.github_accounts(PRIYA).unwrap().is_empty());
        let mut kept: Vec<(String, SuggestionState)> = store
            .suggestion_states(PRIYA)
            .unwrap()
            .into_iter()
            .map(|(key, state)| (key.subject, state))
            .collect();
        kept.sort_by(|l, r| l.0.cmp(&r.0));
        assert_eq!(
            kept,
            vec![
                ("declined".to_string(), SuggestionState::Declined),
                ("published".to_string(), SuggestionState::Published),
            ]
        );
        assert_eq!(store.suggestion_states(SAM).unwrap(), sams_before);
        assert_eq!(store.github_accounts(SAM).unwrap().len(), 1);
        let counted = store.counters_between("2026-10-19", "2026-10-19").unwrap();
        assert_eq!(counted.iter().map(|row| row.count).sum::<i64>(), 1);
    }

    // bypass: one adapter example over a real store — removing an import
    // deletes it and its pending suggestions at once, keeps the declined
    // key, counts once, and leaves another owner's same import untouched.
    #[test]
    fn removing_an_import_deletes_only_its_owners_pending_suggestions() {
        let store = ReviewStore::open_in_memory(DataKey::generate()).unwrap();
        let cargo = ImportedRepo {
            repo_id: 2_001,
            full_name: "rust-lang/cargo".into(),
        };
        for owner in [PRIYA, SAM] {
            store.add_import(owner, &cargo, KpiTally::NONE).unwrap();
            let batch: Vec<Suggestion> = ["a", "declined"].into_iter().map(suggestion).collect();
            store
                .add_pending_from_import(owner, cargo.repo_id, &batch)
                .unwrap();
        }
        store
            .change_state(
                PRIYA,
                &suggestion("declined").key,
                SuggestionState::Pending,
                SuggestionState::Declined,
                KpiTally::NONE,
            )
            .unwrap();
        let sams_before = store.suggestion_states(SAM).unwrap();
        let tally = KpiTally {
            day: "2026-10-19",
            events: &["import.removed"],
        };

        let removal = store.remove_import(PRIYA, cargo.repo_id, tally).unwrap();
        let again = store.remove_import(PRIYA, cargo.repo_id, tally).unwrap();

        assert!(
            matches!(
                removal,
                ImportRemoval::Removed { ref full_name, deleted_pending: 1, held_secs: 0..=60 }
                    if full_name == "rust-lang/cargo"
            ),
            "{removal:?}"
        );
        assert_eq!(again, ImportRemoval::NotImported);
        assert!(store.imports(PRIYA).unwrap().is_empty());
        let kept: Vec<(String, SuggestionState)> = store
            .suggestion_states(PRIYA)
            .unwrap()
            .into_iter()
            .map(|(key, state)| (key.subject, state))
            .collect();
        assert_eq!(
            kept,
            vec![("declined".to_string(), SuggestionState::Declined)]
        );
        assert_eq!(store.suggestion_states(SAM).unwrap(), sams_before);
        assert_eq!(store.imports(SAM).unwrap(), vec![cargo]);
        let counted = store.counters_between("2026-10-19", "2026-10-19").unwrap();
        assert_eq!(counted.iter().map(|row| row.count).sum::<i64>(), 1);
    }
}
