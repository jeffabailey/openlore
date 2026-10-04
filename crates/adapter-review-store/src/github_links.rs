//! `GithubLinkPort`: one GitHub link per DID (ADR-076, data-models §3).
//! A verified GitHub id belongs to one DID only.

use duckdb::{params, OptionalExt};
use ports::{GithubLink, GithubLinkPort, LinkRecorded, ReviewStoreError};

use crate::secrets::port_error;
use crate::{db_error, ReviewStore, StoreError};

const READ_LINK: &str = "SELECT github_login, github_user_id, status
    FROM github_links WHERE owner_did = ?";

const COUNT_VERIFIED_BY_OTHER_OWNERS: &str = "SELECT count(*) FROM github_links
    WHERE github_user_id = ? AND status = 'verified' AND owner_did <> ?";

const UPSERT_VERIFIED_LINK: &str = "INSERT INTO github_links
    (owner_did, github_user_id, github_login, status, verified_at, last_checked_at, last_verdict)
    VALUES (?, ?, ?, 'verified', now(), now(), 'verified')
    ON CONFLICT (owner_did) DO UPDATE SET github_user_id = excluded.github_user_id,
        github_login = excluded.github_login, status = 'verified',
        verified_at = excluded.verified_at, last_checked_at = excluded.last_checked_at,
        last_verdict = 'verified'";

const MARK_UNVERIFIED: &str = "UPDATE github_links
    SET status = 'unverified', last_checked_at = now(), last_verdict = ?
    WHERE owner_did = ?";

fn to_db_id(github_user_id: u64) -> Result<i64, StoreError> {
    i64::try_from(github_user_id)
        .map_err(|_| StoreError::Database("GitHub user id out of range".into()))
}

impl GithubLinkPort for ReviewStore {
    fn github_link(&self, owner_did: &str) -> Result<Option<GithubLink>, ReviewStoreError> {
        self.with_connection(|conn| {
            conn.query_row(READ_LINK, [owner_did], |row| {
                let id: i64 = row.get(1)?;
                let status: String = row.get(2)?;
                Ok(GithubLink {
                    github_login: row.get(0)?,
                    github_user_id: u64::try_from(id).unwrap_or_default(),
                    verified: status == "verified",
                })
            })
            .optional()
            .map_err(db_error)
        })
        .map_err(port_error)
    }

    fn record_verified_link(
        &self,
        owner_did: &str,
        github_login: &str,
        github_user_id: u64,
    ) -> Result<LinkRecorded, ReviewStoreError> {
        self.with_connection(|conn| {
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
            conn.execute(UPSERT_VERIFIED_LINK, params![owner_did, id, github_login])
                .map_err(db_error)?;
            Ok(LinkRecorded::Linked)
        })
        .map_err(port_error)
    }

    fn mark_link_unverified(&self, owner_did: &str, verdict: &str) -> Result<(), ReviewStoreError> {
        self.with_connection(|conn| {
            conn.execute(MARK_UNVERIFIED, params![verdict, owner_did])
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
    use proptest::prelude::*;

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(32))]

        /// Universe: the github_links rows of two DIDs. A GitHub id verified
        /// for one DID cannot be linked by the other (its row is untouched),
        /// until the first link is marked unverified; each DID only ever
        /// reads its own link.
        #[test]
        fn a_verified_github_id_belongs_to_one_did_only(
            id in 1u64..10_000_000, login in "[a-z][a-z0-9-]{0,12}",
        ) {
            let store = ReviewStore::open_in_memory(DataKey::generate()).unwrap();
            let (priya, sam) = ("did:plc:priya", "did:plc:sam");
            let link = |verified| Some(GithubLink { github_login: login.clone(), github_user_id: id, verified });

            prop_assert_eq!(store.record_verified_link(priya, &login, id).unwrap(), LinkRecorded::Linked);
            prop_assert_eq!(store.record_verified_link(sam, &login, id).unwrap(), LinkRecorded::HeldByAnotherOwner);
            prop_assert_eq!(store.github_link(priya).unwrap(), link(true));
            prop_assert_eq!(store.github_link(sam).unwrap(), None);

            store.mark_link_unverified(priya, "did_missing").unwrap();
            prop_assert_eq!(store.github_link(priya).unwrap(), link(false));
            prop_assert_eq!(store.record_verified_link(sam, &login, id).unwrap(), LinkRecorded::Linked);
            prop_assert_eq!(store.github_link(sam).unwrap(), link(true));
            prop_assert_eq!(store.github_link(priya).unwrap(), link(false));
        }
    }
}
