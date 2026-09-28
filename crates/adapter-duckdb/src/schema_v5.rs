//! Migration v5 — the append-only `contribution_links` table
//! (contributor-philosophy-inference DDD-5 / ADR-063 §4).
//!
//! Forward-only + idempotent exactly like v4: applied once (recorded in
//! `schema_version`), a no-op on every later open. Key = case-folded
//! (`repo_key`, `person_key`); display-form subjects kept alongside; the
//! numeric `github_user_id` is indexed for rename detection. Nothing ever
//! deletes from this table; `first_observed_at` is written once.

use duckdb::Connection;
use ports::StorageError;

/// The schema version this migration introduces.
pub const CONTRIBUTION_LINKS_VERSION: i32 = 5;

/// Human-readable description recorded in `schema_version`.
pub const CONTRIBUTION_LINKS_DESCRIPTION: &str =
    "contributor-philosophy-inference contribution_links (append-only)";

/// The v5 DDL.
pub const CONTRIBUTION_LINKS_SQL: &str = r"
    CREATE TABLE IF NOT EXISTS contribution_links (
        repo_key          VARCHAR   NOT NULL,
        person_key        VARCHAR   NOT NULL,
        repo_subject      VARCHAR   NOT NULL,
        person_subject    VARCHAR   NOT NULL,
        github_user_id    BIGINT    NOT NULL,
        rank              INTEGER   NOT NULL CHECK (rank >= 1),
        contributions     BIGINT    NOT NULL CHECK (contributions >= 0),
        first_observed_at TIMESTAMP NOT NULL,
        last_observed_at  TIMESTAMP NOT NULL,
        PRIMARY KEY (repo_key, person_key)
    );

    CREATE INDEX IF NOT EXISTS idx_contribution_links_user_id
        ON contribution_links (github_user_id);
";

fn already_applied(conn: &Connection) -> Result<bool, StorageError> {
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM schema_version WHERE version = ?",
            duckdb::params![CONTRIBUTION_LINKS_VERSION],
            |row| row.get(0),
        )
        .map_err(|err| StorageError::SchemaMigrationFailed {
            message: format!("read schema_version v5 presence: {err}"),
        })?;
    Ok(count > 0)
}

/// Apply migration v5 if it has not been applied yet (one transaction).
pub fn run_migration(conn: &mut Connection) -> Result<(), StorageError> {
    if already_applied(conn)? {
        return Ok(());
    }
    let migration_failed = |step: &str, err: duckdb::Error| StorageError::SchemaMigrationFailed {
        message: format!("{step} v5 migration: {err}"),
    };
    let tx = conn
        .transaction()
        .map_err(|err| migration_failed("begin", err))?;
    tx.execute_batch(CONTRIBUTION_LINKS_SQL)
        .map_err(|err| migration_failed("apply", err))?;
    tx.execute(
        "INSERT INTO schema_version (version, applied_at, description) VALUES (?, now(), ?)",
        duckdb::params![CONTRIBUTION_LINKS_VERSION, CONTRIBUTION_LINKS_DESCRIPTION],
    )
    .map_err(|err| migration_failed("record", err))?;
    tx.commit().map_err(|err| migration_failed("commit", err))
}
