//! Migration v8 (owner-only-signing, ADR-087 §5): the insert-only
//! `repo_authority_checks` table — this reader's local history of repo
//! authority observations. One row per observation, keyed by a
//! sequence-assigned `check_id`; reads go by the case-folded `subject_key`.
//! An observation of a repo GitHub does not have carries no file state (the
//! table CHECK ties `authority_file IS NULL` to `outcome = 'repo_not_found'`).
//! Additive, idempotent, forward-only, gated on its own `schema_version` row
//! (same shape as v7).

use duckdb::Connection;
use ports::StorageError;

pub const REPO_AUTHORITY_CHECKS_VERSION: i32 = 8;

pub const REPO_AUTHORITY_CHECKS_DESCRIPTION: &str =
    "owner-only-signing repo_authority_checks (ADR-087 §5)";

pub const REPO_AUTHORITY_CHECKS_SQL: &str = r"
    CREATE SEQUENCE IF NOT EXISTS repo_authority_check_ids START 1;
    CREATE TABLE IF NOT EXISTS repo_authority_checks (
        check_id       BIGINT PRIMARY KEY DEFAULT nextval('repo_authority_check_ids'),
        subject_key    VARCHAR NOT NULL,
        outcome        VARCHAR NOT NULL CHECK (outcome IN ('found', 'repo_not_found')),
        repo_id        UBIGINT,
        full_name_seen VARCHAR,
        owner_id       UBIGINT,
        owner_type     VARCHAR CHECK (owner_type IN ('user', 'organization')),
        authority_file VARCHAR CHECK (authority_file IN ('absent', 'present', 'invalid')),
        authority_dids VARCHAR[],
        checked_at     TIMESTAMP NOT NULL,
        CHECK ((outcome = 'repo_not_found') = (authority_file IS NULL)),
        CHECK (outcome = 'repo_not_found' OR (
            repo_id IS NOT NULL AND full_name_seen IS NOT NULL
            AND owner_id IS NOT NULL AND owner_type IS NOT NULL
        ))
    );
    CREATE INDEX IF NOT EXISTS idx_repo_authority_checks_subject
        ON repo_authority_checks (subject_key);
";

fn already_applied(conn: &Connection) -> Result<bool, StorageError> {
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM schema_version WHERE version = ?",
            duckdb::params![REPO_AUTHORITY_CHECKS_VERSION],
            |row| row.get(0),
        )
        .map_err(|err| StorageError::SchemaMigrationFailed {
            message: format!("read schema_version v8 presence: {err}"),
        })?;
    Ok(count > 0)
}

pub fn run_migration(conn: &mut Connection) -> Result<(), StorageError> {
    if already_applied(conn)? {
        return Ok(());
    }
    let migration_failed = |step: &str, err: duckdb::Error| StorageError::SchemaMigrationFailed {
        message: format!("{step} v8 migration: {err}"),
    };
    let tx = conn
        .transaction()
        .map_err(|err| migration_failed("begin", err))?;
    tx.execute_batch(REPO_AUTHORITY_CHECKS_SQL)
        .map_err(|err| migration_failed("apply", err))?;
    tx.execute(
        "INSERT INTO schema_version (version, applied_at, description) VALUES (?, now(), ?)",
        duckdb::params![
            REPO_AUTHORITY_CHECKS_VERSION,
            REPO_AUTHORITY_CHECKS_DESCRIPTION
        ],
    )
    .map_err(|err| migration_failed("record", err))?;
    tx.commit().map_err(|err| migration_failed("commit", err))
}
