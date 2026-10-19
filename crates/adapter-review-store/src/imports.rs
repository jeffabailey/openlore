//! `ImportedReposRead` / `ImportedReposWrite` over `imported_repos`
//! (ADR-088 §4): the public repos an owner named that none of her accounts
//! owns; and `ScopeMovePort`, the BR-RRA-5 move of one to the owned list.
//! Every statement is scoped by `owner_did`.

use duckdb::{params, Connection};
use ports::{
    ImportedRepo, ImportedReposRead, ImportedReposWrite, KpiTally, ReviewStoreError,
    SavedRepoChoice, ScopeMovePort,
};

use crate::removal::DELETE_IMPORT;
use crate::repo_choices::SAVE_ROW;
use crate::secrets::port_error;
use crate::{db_error, db_id, ReviewStore, StoreError};

const READ_IMPORTS: &str = "SELECT repo_id, full_name_seen FROM imported_repos
    WHERE owner_did = ? ORDER BY added_at, repo_id";

const CHOICE_ROWS: &str = "SELECT count(*) FROM repo_choices
    WHERE owner_did = ? AND github_user_id = ?";

/// A moved repo's suggestions now come from the account that owns it.
const SOURCE_FROM_ACCOUNT: &str = "UPDATE suggestions
    SET source_kind = 'account', source_account_id = ?
    WHERE owner_did = ? AND source_kind = 'imported' AND source_repo_id = ?";

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

impl ScopeMovePort for ReviewStore {
    fn move_import_to_account(
        &self,
        owner_did: &str,
        moved: &ImportedRepo,
        github_user_id: u64,
        choice_if_absent: &[SavedRepoChoice],
        tally: KpiTally<'_>,
    ) -> Result<bool, ReviewStoreError> {
        self.with_tally(
            tally,
            |moved| *moved,
            |conn| move_rows(conn, owner_did, moved, github_user_id, choice_if_absent),
        )
        .map_err(port_error)
    }
}

/// The move's statements, in the caller's transaction.
fn move_rows(
    conn: &Connection,
    owner_did: &str,
    moved: &ImportedRepo,
    github_user_id: u64,
    choice_if_absent: &[SavedRepoChoice],
) -> Result<bool, StoreError> {
    let repo = db_id(moved.repo_id)?;
    let account = db_id(github_user_id)?;
    if conn
        .execute(DELETE_IMPORT, params![owner_did, repo])
        .map_err(db_error)?
        == 0
    {
        return Ok(false);
    }
    let saved: i64 = conn
        .query_row(CHOICE_ROWS, params![owner_did, account], |row| row.get(0))
        .map_err(db_error)?;
    let ticked_move = SavedRepoChoice {
        repo_id: moved.repo_id,
        full_name: moved.full_name.clone(),
        ticked: true,
    };
    let rows: &[SavedRepoChoice] = if saved == 0 { choice_if_absent } else { &[] };
    rows.iter()
        .chain(std::iter::once(&ticked_move))
        .try_for_each(|row| {
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
    conn.execute(SOURCE_FROM_ACCOUNT, params![account, owner_did, repo])
        .map_err(db_error)?;
    Ok(true)
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
