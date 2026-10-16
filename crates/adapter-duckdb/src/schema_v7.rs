//! Migration v7 (github-did-identity-link, ADR-085 §7): the insert-only
//! `identity_link_checks` table — the local history of GitHub proof checks.
//! One row per check, keyed by a sequence-assigned `check_id` so equal
//! timestamps never collide; "latest" is a read query. Additive, idempotent,
//! forward-only, gated on its own `schema_version` row (same shape as v6).

use duckdb::Connection;
use ports::StorageError;

pub const IDENTITY_LINK_CHECKS_VERSION: i32 = 7;

pub const IDENTITY_LINK_CHECKS_DESCRIPTION: &str =
    "github-did-identity-link identity_link_checks (ADR-085 §7)";

pub const IDENTITY_LINK_CHECKS_SQL: &str = r"
    CREATE SEQUENCE IF NOT EXISTS identity_link_check_ids START 1;
    CREATE TABLE IF NOT EXISTS identity_link_checks (
        check_id       BIGINT PRIMARY KEY DEFAULT nextval('identity_link_check_ids'),
        subject_did    VARCHAR NOT NULL,
        github_user_id BIGINT  NOT NULL,
        login_seen     VARCHAR NOT NULL,
        verdict        VARCHAR NOT NULL CHECK (verdict IN (
            'verified', 'did_missing', 'different_did', 'no_bio', 'account_not_found'
        )),
        checked_at     TIMESTAMP NOT NULL
    );
    CREATE INDEX IF NOT EXISTS idx_identity_link_checks_link
        ON identity_link_checks (subject_did, github_user_id);
";

fn already_applied(conn: &Connection) -> Result<bool, StorageError> {
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM schema_version WHERE version = ?",
            duckdb::params![IDENTITY_LINK_CHECKS_VERSION],
            |row| row.get(0),
        )
        .map_err(|err| StorageError::SchemaMigrationFailed {
            message: format!("read schema_version v7 presence: {err}"),
        })?;
    Ok(count > 0)
}

pub fn run_migration(conn: &mut Connection) -> Result<(), StorageError> {
    if already_applied(conn)? {
        return Ok(());
    }
    let migration_failed = |step: &str, err: duckdb::Error| StorageError::SchemaMigrationFailed {
        message: format!("{step} v7 migration: {err}"),
    };
    let tx = conn
        .transaction()
        .map_err(|err| migration_failed("begin", err))?;
    tx.execute_batch(IDENTITY_LINK_CHECKS_SQL)
        .map_err(|err| migration_failed("apply", err))?;
    tx.execute(
        "INSERT INTO schema_version (version, applied_at, description) VALUES (?, now(), ?)",
        duckdb::params![
            IDENTITY_LINK_CHECKS_VERSION,
            IDENTITY_LINK_CHECKS_DESCRIPTION
        ],
    )
    .map_err(|err| migration_failed("record", err))?;
    tx.commit().map_err(|err| migration_failed("commit", err))
}
