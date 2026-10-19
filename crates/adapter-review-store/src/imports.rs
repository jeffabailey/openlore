//! `ImportedReposRead` / `ImportedReposWrite` over `imported_repos`
//! (ADR-088 §4): the public repos an owner named that none of her accounts
//! owns. Every statement is scoped by `owner_did`.

use duckdb::params;
use ports::{ImportedRepo, ImportedReposRead, ImportedReposWrite, KpiTally, ReviewStoreError};

use crate::secrets::port_error;
use crate::{db_error, db_id, ReviewStore};

const READ_IMPORTS: &str = "SELECT repo_id, full_name_seen FROM imported_repos
    WHERE owner_did = ? ORDER BY added_at, repo_id";

const ADD_IMPORT: &str = "INSERT INTO imported_repos (owner_did, repo_id, full_name_seen, added_at)
    VALUES (?, ?, ?, now())
    ON CONFLICT (owner_did, repo_id) DO NOTHING";

impl ImportedReposRead for ReviewStore {
    fn imports(&self, owner_did: &str) -> Result<Vec<ImportedRepo>, ReviewStoreError> {
        self.with_connection(|conn| {
            let mut statement = conn.prepare(READ_IMPORTS).map_err(db_error)?;
            let rows = statement
                .query_map([owner_did], |row| {
                    Ok(ImportedRepo {
                        repo_id: u64::try_from(row.get::<_, i64>(0)?).unwrap_or_default(),
                        full_name: row.get(1)?,
                    })
                })
                .map_err(db_error)?;
            rows.collect::<Result<_, _>>().map_err(db_error)
        })
        .map_err(port_error)
    }
}

impl ImportedReposWrite for ReviewStore {
    fn add_import(
        &self,
        owner_did: &str,
        repo: &ImportedRepo,
        tally: KpiTally<'_>,
    ) -> Result<(), ReviewStoreError> {
        self.with_tally(
            tally,
            |()| true,
            |conn| {
                conn.execute(
                    ADD_IMPORT,
                    params![owner_did, db_id(repo.repo_id)?, repo.full_name],
                )
                .map(drop)
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

    fn cargo() -> ImportedRepo {
        ImportedRepo {
            repo_id: 2_001,
            full_name: "rust-lang/cargo".into(),
        }
    }

    /// An import is listed for its owner only, once however often it is added.
    #[test]
    fn an_import_is_listed_once_and_only_for_its_owner() {
        let store = ReviewStore::open_in_memory(DataKey::generate()).unwrap();
        store
            .add_import("did:plc:priya", &cargo(), KpiTally::NONE)
            .unwrap();
        store
            .add_import("did:plc:priya", &cargo(), KpiTally::NONE)
            .unwrap();
        assert_eq!(store.imports("did:plc:priya").unwrap(), [cargo()]);
        assert!(store.imports("did:plc:dmitri").unwrap().is_empty());
    }
}
