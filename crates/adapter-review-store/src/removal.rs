//! `ScopeRemovalPort` (ADR-088 §8, D-RRA-17): removing an account deletes
//! its `github_accounts` row, its `repo_choices` rows (re-adding starts
//! fresh, US-RRA-008) and its `pending` suggestions, in ONE transaction.
//! The `DELETE` on `suggestions` names `state = 'pending'`: declined,
//! published and retracted keys stay (BR-1, BR-2). The only `DELETE` outside
//! purge and expiry (check-arch `review_store_owner_scoped_sql`).

use duckdb::{params, Connection};
use ports::{AccountRemoval, KpiTally, ReviewStoreError, ScopeRemovalPort};

use crate::secrets::port_error;
use crate::{db_error, db_id, ReviewStore, StoreError};

const DELETE_ACCOUNT: &str = "DELETE FROM github_accounts
    WHERE owner_did = ? AND github_user_id = ?";

const DELETE_ACCOUNT_CHOICE: &str = "DELETE FROM repo_choices
    WHERE owner_did = ? AND github_user_id = ?";

const DELETE_ACCOUNT_PENDING: &str = "DELETE FROM suggestions
    WHERE owner_did = ? AND state = 'pending' AND source_kind = 'account'
      AND source_account_id = ?";

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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DataKey;
    use ports::{
        GithubAccountsRead, GithubAccountsWrite, KpiCounterPort, ReviewStateRead, ReviewStateWrite,
        Suggestion, SuggestionKey, SuggestionSource, SuggestionState,
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
}
