//! The account ports over `github_accounts` (ADR-088 §2), keyed by GitHub's
//! numeric id: several accounts per DID in `added_at` order, at most
//! `max_accounts` of them, and a verified GitHub id belongs to one DID only.
//! Both limits are checked again inside the add's own transaction.

use duckdb::{params, Connection};
use ports::{
    AccountAdmission, AccountAdmitted, AccountListingNote, AccountRefusal, AccountStatusWrite,
    GithubAccountsRead, GithubAccountsWrite, GithubLink, KpiTally, ReviewStoreError,
};

use crate::secrets::port_error;
use crate::{db_error, ReviewStore, StoreError};

const READ_ACCOUNTS: &str = "SELECT github_login_seen, github_user_id, status
    FROM github_accounts WHERE owner_did = ? ORDER BY added_at, github_user_id";

const COUNT_VERIFIED_BY_OTHER_OWNERS: &str = "SELECT count(*) FROM github_accounts
    WHERE github_user_id = ? AND status = 'verified' AND owner_did <> ?";

const REVERIFY: &str = "UPDATE github_accounts
    SET github_login_seen = ?, status = 'verified', verified_at = now(),
        last_checked_at = now(), last_verdict = 'verified'
    WHERE owner_did = ? AND github_user_id = ?";

const INSERT_VERIFIED: &str = "INSERT INTO github_accounts
    (owner_did, github_user_id, github_login_seen, status, verified_at, last_checked_at,
     last_verdict, added_at)
    VALUES (?, ?, ?, 'verified', now(), now(), 'verified', now())";

const MARK_UNVERIFIED: &str = "UPDATE github_accounts
    SET status = 'unverified', last_checked_at = now(), last_verdict = ?
    WHERE owner_did = ? AND github_user_id = ?";

const RECORD_ELIGIBLE_COUNT: &str = "UPDATE github_accounts
    SET eligible_count_seen = ? WHERE owner_did = ? AND github_user_id = ?";

fn to_db_id(github_user_id: u64) -> Result<i64, StoreError> {
    i64::try_from(github_user_id)
        .map_err(|_| StoreError::Database("GitHub user id out of range".into()))
}

fn listed_accounts(conn: &Connection, owner_did: &str) -> Result<Vec<GithubLink>, StoreError> {
    let mut statement = conn.prepare(READ_ACCOUNTS).map_err(db_error)?;
    let rows = statement
        .query_map([owner_did], |row| {
            let id: i64 = row.get(1)?;
            let status: String = row.get(2)?;
            Ok(GithubLink {
                github_login: row.get(0)?,
                github_user_id: u64::try_from(id).unwrap_or_default(),
                verified: status == "verified",
            })
        })
        .map_err(db_error)?;
    rows.collect::<Result<_, _>>().map_err(db_error)
}

fn held_by_another_owner(
    conn: &Connection,
    owner_did: &str,
    github_user_id: u64,
) -> Result<bool, StoreError> {
    let held: i64 = conn
        .query_row(
            COUNT_VERIFIED_BY_OTHER_OWNERS,
            params![to_db_id(github_user_id)?, owner_did],
            |row| row.get(0),
        )
        .map_err(db_error)?;
    Ok(held > 0)
}

/// The add's decision on the rows as they are inside its transaction. A
/// listed unverified account is verified again only while no other DID
/// holds it verified, so a GitHub id is never verified for two DIDs.
fn admission(
    listed: &[GithubLink],
    github_user_id: u64,
    held_elsewhere: bool,
    max_accounts: usize,
) -> AccountAdmission {
    let listed_as = listed
        .iter()
        .find(|account| account.github_user_id == github_user_id);
    match (listed_as, held_elsewhere) {
        (Some(account), _) if account.verified => {
            AccountAdmission::Refused(AccountRefusal::AlreadyListed)
        }
        (_, true) => AccountAdmission::Refused(AccountRefusal::HeldByAnotherOwner),
        (Some(_), false) => AccountAdmission::Admitted(AccountAdmitted::Reverified),
        (None, false) if listed.len() >= max_accounts => {
            AccountAdmission::Refused(AccountRefusal::AtLimit)
        }
        (None, false) => AccountAdmission::Admitted(AccountAdmitted::Added),
    }
}

impl GithubAccountsRead for ReviewStore {
    fn github_accounts(&self, owner_did: &str) -> Result<Vec<GithubLink>, ReviewStoreError> {
        self.with_connection(|conn| listed_accounts(conn, owner_did))
            .map_err(port_error)
    }

    fn verified_for_another_owner(
        &self,
        owner_did: &str,
        github_user_id: u64,
    ) -> Result<bool, ReviewStoreError> {
        self.with_connection(|conn| held_by_another_owner(conn, owner_did, github_user_id))
            .map_err(port_error)
    }
}

impl GithubAccountsWrite for ReviewStore {
    fn add_or_reverify(
        &self,
        owner_did: &str,
        github_login: &str,
        github_user_id: u64,
        max_accounts: usize,
        tally: KpiTally<'_>,
    ) -> Result<AccountAdmission, ReviewStoreError> {
        let admitted =
            |admission: &AccountAdmission| matches!(admission, AccountAdmission::Admitted(_));
        self.with_tally(tally, admitted, |conn| {
            let id = to_db_id(github_user_id)?;
            let decided = admission(
                &listed_accounts(conn, owner_did)?,
                github_user_id,
                held_by_another_owner(conn, owner_did, github_user_id)?,
                max_accounts,
            );
            match decided {
                AccountAdmission::Admitted(AccountAdmitted::Added) => conn
                    .execute(INSERT_VERIFIED, params![owner_did, id, github_login])
                    .map_err(db_error)?,
                AccountAdmission::Admitted(AccountAdmitted::Reverified) => conn
                    .execute(REVERIFY, params![github_login, owner_did, id])
                    .map_err(db_error)?,
                AccountAdmission::Refused(_) => 0,
            };
            Ok(decided)
        })
        .map_err(port_error)
    }
}

impl AccountStatusWrite for ReviewStore {
    fn mark_account_unverified(
        &self,
        owner_did: &str,
        github_user_id: u64,
        verdict: &str,
    ) -> Result<(), ReviewStoreError> {
        self.with_connection(|conn| {
            let id = to_db_id(github_user_id)?;
            conn.execute(MARK_UNVERIFIED, params![verdict, owner_did, id])
                .map(drop)
                .map_err(db_error)
        })
        .map_err(port_error)
    }
}

impl AccountListingNote for ReviewStore {
    fn record_eligible_count(
        &self,
        owner_did: &str,
        github_user_id: u64,
        eligible: usize,
    ) -> Result<(), ReviewStoreError> {
        self.with_connection(|conn| {
            let id = to_db_id(github_user_id)?;
            let count = i64::try_from(eligible)
                .map_err(|_| StoreError::Database("eligible count out of range".into()))?;
            conn.execute(RECORD_ELIGIBLE_COUNT, params![count, owner_did, id])
                .map(drop)
                .map_err(db_error)
        })
        .map_err(port_error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DataKey;
    use ports::GithubLinkPort;
    use proptest::prelude::*;

    const MAX: usize = 3;
    const ADDED: AccountAdmission = AccountAdmission::Admitted(AccountAdmitted::Added);

    fn add(store: &ReviewStore, owner: &str, login: &str, id: u64) -> AccountAdmission {
        store
            .add_or_reverify(owner, login, id, MAX, KpiTally::NONE)
            .unwrap()
    }

    fn eligible_count_seen(store: &ReviewStore, owner: &str, id: u64) -> Option<i64> {
        store
            .with_connection(|conn| {
                conn.query_row(
                    "SELECT eligible_count_seen FROM github_accounts
                     WHERE owner_did = ? AND github_user_id = ?",
                    params![owner, to_db_id(id)?],
                    |row| row.get(0),
                )
                .map_err(db_error)
            })
            .unwrap()
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(32))]

        /// Universe: the github_accounts rows of two DIDs. A GitHub id
        /// verified for one DID cannot be added by the other (its row is
        /// untouched), until the first is marked unverified; the first then
        /// cannot verify it again while the other holds it; each DID only
        /// ever reads its own accounts.
        #[test]
        fn a_verified_github_id_belongs_to_one_did_only(
            id in 1u64..10_000_000, login in "[a-z][a-z0-9-]{0,12}",
        ) {
            let store = ReviewStore::open_in_memory(DataKey::generate()).unwrap();
            let (priya, sam) = ("did:plc:priya", "did:plc:sam");
            let link = |verified| Some(GithubLink { github_login: login.clone(), github_user_id: id, verified });
            let held = AccountAdmission::Refused(AccountRefusal::HeldByAnotherOwner);

            prop_assert_eq!(add(&store, priya, &login, id), ADDED);
            prop_assert!(store.verified_for_another_owner(sam, id).unwrap());
            prop_assert!(!store.verified_for_another_owner(priya, id).unwrap());
            prop_assert_eq!(add(&store, sam, &login, id), held);
            prop_assert_eq!(store.github_link(priya).unwrap(), link(true));
            prop_assert_eq!(store.github_link(sam).unwrap(), None);

            store.mark_account_unverified(priya, id, "did_missing").unwrap();
            prop_assert_eq!(store.github_link(priya).unwrap(), link(false));
            prop_assert_eq!(add(&store, sam, &login, id), ADDED);
            prop_assert_eq!(add(&store, priya, &login, id), held);
            prop_assert_eq!(store.github_link(sam).unwrap(), link(true));
            prop_assert_eq!(store.github_link(priya).unwrap(), link(false));
        }

        /// Universe: one DID's github_accounts rows. Up to three distinct
        /// accounts are added in order, each verified, none replacing
        /// another; a fourth changes nothing; a listed verified account is
        /// already listed; a listed unverified one is verified in its place.
        #[test]
        fn a_did_lists_up_to_three_accounts_in_the_order_they_were_added(
            ids in proptest::collection::btree_set(1u64..10_000_000, 4),
            reverified in 0usize..3,
        ) {
            let store = ReviewStore::open_in_memory(DataKey::generate()).unwrap();
            let priya = "did:plc:priya";
            let ids: Vec<u64> = ids.into_iter().collect();
            let listed = |unverified: Option<usize>| -> Vec<GithubLink> {
                ids[..MAX].iter().enumerate().map(|(i, id)| GithubLink {
                    github_login: format!("gh{i}"),
                    github_user_id: *id,
                    verified: unverified != Some(i),
                }).collect()
            };
            for (i, id) in ids[..MAX].iter().enumerate() {
                prop_assert_eq!(add(&store, priya, &format!("gh{i}"), *id), ADDED);
            }
            prop_assert_eq!(store.github_accounts(priya).unwrap(), listed(None));

            prop_assert_eq!(
                add(&store, priya, "gh-fourth", ids[MAX]),
                AccountAdmission::Refused(AccountRefusal::AtLimit)
            );
            let again = ids[reverified];
            prop_assert_eq!(
                add(&store, priya, &format!("gh{reverified}"), again),
                AccountAdmission::Refused(AccountRefusal::AlreadyListed)
            );
            prop_assert_eq!(store.github_accounts(priya).unwrap(), listed(None));

            store.mark_account_unverified(priya, again, "did_missing").unwrap();
            prop_assert_eq!(store.github_accounts(priya).unwrap(), listed(Some(reverified)));
            prop_assert_eq!(
                add(&store, priya, &format!("gh{reverified}"), again),
                AccountAdmission::Admitted(AccountAdmitted::Reverified)
            );
            prop_assert_eq!(store.github_accounts(priya).unwrap(), listed(None));
        }

        /// Universe: the eligible_count_seen of each row of two DIDs. Noting
        /// one account's count changes that row's count only.
        #[test]
        fn noting_an_eligible_count_changes_that_account_only(
            count in 0usize..300, first in 1u64..5_000_000, second in 5_000_000u64..10_000_000,
        ) {
            let store = ReviewStore::open_in_memory(DataKey::generate()).unwrap();
            let (priya, sam) = ("did:plc:priya", "did:plc:sam");
            add(&store, priya, "first", first);
            add(&store, priya, "second", second);
            add(&store, sam, "sams", second + 1);

            store.record_eligible_count(priya, first, count).unwrap();

            prop_assert_eq!(eligible_count_seen(&store, priya, first), Some(count as i64));
            prop_assert_eq!(eligible_count_seen(&store, priya, second), None);
            prop_assert_eq!(eligible_count_seen(&store, sam, second + 1), None);
        }
    }
}
