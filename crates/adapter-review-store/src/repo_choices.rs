//! `RepoChoiceRead` / `RepoChoiceWrite` over `repo_choices` (ADR-088 §3):
//! an account's saved choice, keyed by GitHub's numeric repo id. Every
//! statement is scoped by `owner_did` and the account.

use duckdb::params;
use ports::{KpiTally, RepoChoiceRead, RepoChoiceWrite, ReviewStoreError, SavedRepoChoice};

use crate::secrets::port_error;
use crate::{db_error, db_id, ReviewStore};

const READ_CHOICE: &str = "SELECT repo_id, full_name_seen, ticked FROM repo_choices
    WHERE owner_did = ? AND github_user_id = ? ORDER BY repo_id";

/// Whether the owner still lists the account a Save is for (review R1).
const ACCOUNT_LISTED: &str = "SELECT EXISTS (SELECT 1 FROM github_accounts
    WHERE owner_did = ? AND github_user_id = ?)";

/// A Save replaces the account's ticks: what it does not show is unticked.
const UNTICK_ACCOUNT: &str = "UPDATE repo_choices SET ticked = false
    WHERE owner_did = ? AND github_user_id = ?";

pub(crate) const SAVE_ROW: &str = "INSERT INTO repo_choices
    (owner_did, github_user_id, repo_id, full_name_seen, ticked, saved_at)
    VALUES (?, ?, ?, ?, ?, now())
    ON CONFLICT (owner_did, github_user_id, repo_id) DO UPDATE
    SET full_name_seen = excluded.full_name_seen, ticked = excluded.ticked,
        saved_at = excluded.saved_at";

impl RepoChoiceRead for ReviewStore {
    fn saved_choice(
        &self,
        owner_did: &str,
        github_user_id: u64,
    ) -> Result<Vec<SavedRepoChoice>, ReviewStoreError> {
        self.with_connection(|conn| {
            let account = db_id(github_user_id)?;
            let mut statement = conn.prepare(READ_CHOICE).map_err(db_error)?;
            let rows = statement
                .query_map(params![owner_did, account], |row| {
                    Ok(SavedRepoChoice {
                        repo_id: u64::try_from(row.get::<_, i64>(0)?).unwrap_or_default(),
                        full_name: row.get(1)?,
                        ticked: row.get(2)?,
                    })
                })
                .map_err(db_error)?;
            rows.collect::<Result<_, _>>().map_err(db_error)
        })
        .map_err(port_error)
    }
}

impl RepoChoiceWrite for ReviewStore {
    fn save_choice(
        &self,
        owner_did: &str,
        github_user_id: u64,
        rows: &[SavedRepoChoice],
        tally: KpiTally<'_>,
    ) -> Result<bool, ReviewStoreError> {
        self.with_tally(
            tally,
            |saved| *saved,
            |conn| {
                let account = db_id(github_user_id)?;
                let listed: bool = conn
                    .query_row(ACCOUNT_LISTED, params![owner_did, account], |row| {
                        row.get(0)
                    })
                    .map_err(db_error)?;
                if !listed {
                    return Ok(false);
                }
                conn.execute(UNTICK_ACCOUNT, params![owner_did, account])
                    .map_err(db_error)?;
                rows.iter().try_for_each(|row| {
                    conn.execute(
                        SAVE_ROW,
                        params![
                            owner_did,
                            account,
                            db_id(row.repo_id)?,
                            row.full_name,
                            row.ticked
                        ],
                    )
                    .map(drop)
                    .map_err(db_error)
                })?;
                Ok(true)
            },
        )
        .map_err(port_error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DataKey;
    use ports::{GithubAccountsWrite, KpiCounterPort, ScopeRemovalPort};
    use proptest::prelude::*;

    const DAY: &str = "2026-10-19";

    fn row(repo_id: u64, ticked: bool) -> SavedRepoChoice {
        SavedRepoChoice {
            repo_id,
            full_name: format!("me/repo-{repo_id}"),
            ticked,
        }
    }

    /// A store where each (owner, account) pair lists that account.
    fn store_listing(accounts: &[(&str, u64)]) -> ReviewStore {
        let store = ReviewStore::open_in_memory(DataKey::generate()).unwrap();
        for (owner, account) in accounts {
            store
                .add_or_reverify(
                    owner,
                    &format!("login-{account}"),
                    *account,
                    3,
                    KpiTally::NONE,
                )
                .unwrap();
        }
        store
    }

    fn saved_count(store: &ReviewStore) -> i64 {
        store
            .counters_between(DAY, DAY)
            .unwrap()
            .iter()
            .filter(|row| row.event == "repos.choice.saved")
            .map(|row| row.count)
            .sum()
    }

    proptest! {
        /// Universe: `repo_choices` of two owners, two accounts each. A Save
        /// leaves exactly its rows' ticks for (owner, account), unticks what
        /// it did not show, and changes no other owner's or account's choice.
        #[test]
        fn a_save_replaces_only_that_accounts_ticks(
            first in prop::collection::btree_map(1u64..20, any::<bool>(), 0..8),
            second in prop::collection::btree_map(1u64..20, any::<bool>(), 0..8),
        ) {
            let (a, b) = ("did:plc:aaaaaaaaaaaa", "did:plc:bbbbbbbbbbbb");
            let store = store_listing(&[(a, 1), (a, 2), (b, 1)]);
            let rows = |m: &std::collections::BTreeMap<u64, bool>| -> Vec<SavedRepoChoice> {
                m.iter().map(|(id, t)| row(*id, *t)).collect()
            };
            store.save_choice(a, 1, &rows(&first), KpiTally::NONE).unwrap();
            store.save_choice(b, 1, &rows(&first), KpiTally::NONE).unwrap();
            store.save_choice(a, 2, &rows(&first), KpiTally::NONE).unwrap();
            let others_before = (store.saved_choice(b, 1).unwrap(), store.saved_choice(a, 2).unwrap());

            prop_assert!(store.save_choice(a, 1, &rows(&second), KpiTally::NONE).unwrap());

            let ticked: Vec<u64> = store.saved_choice(a, 1).unwrap()
                .into_iter().filter(|r| r.ticked).map(|r| r.repo_id).collect();
            let expected: Vec<u64> = second.iter().filter(|(_, t)| **t).map(|(id, _)| *id).collect();
            prop_assert_eq!(ticked, expected);
            prop_assert_eq!(
                (store.saved_choice(b, 1).unwrap(), store.saved_choice(a, 2).unwrap()),
                others_before
            );
        }

        /// Review R1. Universe: `repo_choices` and the `repos.choice.saved`
        /// counter. A Save for an account the owner no longer lists (removed,
        /// or never hers) writes no row and counts nothing, whatever it
        /// carries; her listed account's choice is untouched.
        #[test]
        fn a_save_for_an_account_she_no_longer_lists_writes_nothing(
            rows in prop::collection::btree_map(1u64..20, any::<bool>(), 1..8),
            removed in any::<bool>(),
        ) {
            let owner = "did:plc:aaaaaaaaaaaa";
            let store = store_listing(&[(owner, 1), (owner, 2)]);
            let rows: Vec<SavedRepoChoice> = rows.iter().map(|(id, t)| row(*id, *t)).collect();
            store.save_choice(owner, 1, &rows, KpiTally::NONE).unwrap();
            let gone = if removed {
                store.remove_account(owner, 2, KpiTally::NONE).unwrap();
                2
            } else {
                7
            };
            let kept = store.saved_choice(owner, 1).unwrap();
            let tally = KpiTally { day: DAY, events: &["repos.choice.saved"] };

            prop_assert!(!store.save_choice(owner, gone, &rows, tally).unwrap());

            prop_assert!(store.saved_choice(owner, gone).unwrap().is_empty());
            prop_assert_eq!(store.saved_choice(owner, 1).unwrap(), kept);
            prop_assert_eq!(saved_count(&store), 0);
        }
    }
}
