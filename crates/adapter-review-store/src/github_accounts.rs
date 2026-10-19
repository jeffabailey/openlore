//! `GithubAccountsRead` / `GithubAccountsWrite` over `github_accounts`
//! (ADR-088 §2), keyed by GitHub's numeric id, with today's one-account
//! behaviour: a newly verified account takes the place of the DID's account.
//! A verified GitHub id belongs to one DID only.

use duckdb::params;
use ports::{
    GithubAccountsRead, GithubAccountsWrite, GithubLink, KpiTally, LinkRecorded, ReviewStoreError,
};

use crate::secrets::port_error;
use crate::{db_error, ReviewStore, StoreError};

const READ_ACCOUNTS: &str = "SELECT github_login_seen, github_user_id, status
    FROM github_accounts WHERE owner_did = ? ORDER BY added_at, github_user_id";

const COUNT_VERIFIED_BY_OTHER_OWNERS: &str = "SELECT count(*) FROM github_accounts
    WHERE github_user_id = ? AND status = 'verified' AND owner_did <> ?";

/// One account per DID today: verifying replaces the DID's account in place.
const REPLACE_WITH_VERIFIED: &str = "UPDATE github_accounts
    SET github_user_id = ?, github_login_seen = ?, status = 'verified', verified_at = now(),
        last_checked_at = now(), last_verdict = 'verified'
    WHERE owner_did = ?";

const INSERT_VERIFIED: &str = "INSERT INTO github_accounts
    (owner_did, github_user_id, github_login_seen, status, verified_at, last_checked_at,
     last_verdict, added_at)
    VALUES (?, ?, ?, 'verified', now(), now(), 'verified', now())";

const MARK_UNVERIFIED: &str = "UPDATE github_accounts
    SET status = 'unverified', last_checked_at = now(), last_verdict = ?
    WHERE owner_did = ? AND github_user_id = ?";

fn to_db_id(github_user_id: u64) -> Result<i64, StoreError> {
    i64::try_from(github_user_id)
        .map_err(|_| StoreError::Database("GitHub user id out of range".into()))
}

impl GithubAccountsRead for ReviewStore {
    fn github_accounts(&self, owner_did: &str) -> Result<Vec<GithubLink>, ReviewStoreError> {
        self.with_connection(|conn| {
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
        })
        .map_err(port_error)
    }
}

impl GithubAccountsWrite for ReviewStore {
    fn record_verified_account(
        &self,
        owner_did: &str,
        github_login: &str,
        github_user_id: u64,
        tally: KpiTally<'_>,
    ) -> Result<LinkRecorded, ReviewStoreError> {
        let linked = |recorded: &LinkRecorded| *recorded == LinkRecorded::Linked;
        self.with_tally(tally, linked, |conn| {
            let id = to_db_id(github_user_id)?;
            let held_elsewhere: i64 = conn
                .query_row(
                    COUNT_VERIFIED_BY_OTHER_OWNERS,
                    params![id, owner_did],
                    |row| row.get(0),
                )
                .map_err(db_error)?;
            if held_elsewhere > 0 {
                return Ok(LinkRecorded::HeldByAnotherOwner);
            }
            let replaced = conn
                .execute(REPLACE_WITH_VERIFIED, params![id, github_login, owner_did])
                .map_err(db_error)?;
            if replaced == 0 {
                conn.execute(INSERT_VERIFIED, params![owner_did, id, github_login])
                    .map_err(db_error)?;
            }
            Ok(LinkRecorded::Linked)
        })
        .map_err(port_error)
    }

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DataKey;
    use ports::GithubLinkPort;
    use proptest::prelude::*;

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(32))]

        /// Universe: the github_accounts rows of two DIDs. A GitHub id
        /// verified for one DID cannot be linked by the other (its row is
        /// untouched), until the first is marked unverified; each DID only
        /// ever reads its own account.
        #[test]
        fn a_verified_github_id_belongs_to_one_did_only(
            id in 1u64..10_000_000, login in "[a-z][a-z0-9-]{0,12}",
        ) {
            let store = ReviewStore::open_in_memory(DataKey::generate()).unwrap();
            let (priya, sam) = ("did:plc:priya", "did:plc:sam");
            let link = |verified| Some(GithubLink { github_login: login.clone(), github_user_id: id, verified });

            prop_assert_eq!(store.record_verified_account(priya, &login, id, KpiTally::NONE).unwrap(), LinkRecorded::Linked);
            prop_assert_eq!(store.record_verified_account(sam, &login, id, KpiTally::NONE).unwrap(), LinkRecorded::HeldByAnotherOwner);
            prop_assert_eq!(store.github_link(priya).unwrap(), link(true));
            prop_assert_eq!(store.github_link(sam).unwrap(), None);

            store.mark_account_unverified(priya, id, "did_missing").unwrap();
            prop_assert_eq!(store.github_link(priya).unwrap(), link(false));
            prop_assert_eq!(store.record_verified_account(sam, &login, id, KpiTally::NONE).unwrap(), LinkRecorded::Linked);
            prop_assert_eq!(store.github_link(sam).unwrap(), link(true));
            prop_assert_eq!(store.github_link(priya).unwrap(), link(false));
        }

        /// Today's one-account behaviour: verifying another GitHub account
        /// replaces the DID's account; the DID still holds exactly one.
        #[test]
        fn verifying_another_account_replaces_the_one_account(
            first in 1u64..5_000_000, second in 5_000_000u64..10_000_000,
        ) {
            let store = ReviewStore::open_in_memory(DataKey::generate()).unwrap();
            let priya = "did:plc:priya";
            store.record_verified_account(priya, "first", first, KpiTally::NONE).unwrap();
            store.record_verified_account(priya, "second", second, KpiTally::NONE).unwrap();
            prop_assert_eq!(
                store.github_accounts(priya).unwrap(),
                vec![GithubLink { github_login: "second".into(), github_user_id: second, verified: true }]
            );
        }
    }
}
