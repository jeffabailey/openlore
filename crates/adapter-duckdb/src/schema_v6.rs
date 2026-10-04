//! Migration v6 (bluesky-claim-review-app, ADR-071): the additive
//! `peer_claims.provenance` column. Existing rows are app-signed (the only
//! mode before v6); a self-attested peer claim is stored with
//! `'self-attested'` and its artifact holds the unsigned claim. Idempotent,
//! forward-only, gated on its own `schema_version` row (same shape as v5).

use duckdb::Connection;
use ports::StorageError;

pub const PEER_PROVENANCE_VERSION: i32 = 6;

pub const PEER_PROVENANCE_DESCRIPTION: &str =
    "bluesky-claim-review-app peer_claims.provenance (ADR-071)";

pub const PEER_PROVENANCE_SQL: &str = r"
    ALTER TABLE peer_claims
        ADD COLUMN IF NOT EXISTS provenance VARCHAR DEFAULT 'app-signed';
    UPDATE peer_claims SET provenance = 'app-signed' WHERE provenance IS NULL;
";

fn already_applied(conn: &Connection) -> Result<bool, StorageError> {
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM schema_version WHERE version = ?",
            duckdb::params![PEER_PROVENANCE_VERSION],
            |row| row.get(0),
        )
        .map_err(|err| StorageError::SchemaMigrationFailed {
            message: format!("read schema_version v6 presence: {err}"),
        })?;
    Ok(count > 0)
}

pub fn run_migration(conn: &mut Connection) -> Result<(), StorageError> {
    if already_applied(conn)? {
        return Ok(());
    }
    let migration_failed = |step: &str, err: duckdb::Error| StorageError::SchemaMigrationFailed {
        message: format!("{step} v6 migration: {err}"),
    };
    let tx = conn
        .transaction()
        .map_err(|err| migration_failed("begin", err))?;
    tx.execute_batch(PEER_PROVENANCE_SQL)
        .map_err(|err| migration_failed("apply", err))?;
    tx.execute(
        "INSERT INTO schema_version (version, applied_at, description) VALUES (?, now(), ?)",
        duckdb::params![PEER_PROVENANCE_VERSION, PEER_PROVENANCE_DESCRIPTION],
    )
    .map_err(|err| migration_failed("record", err))?;
    tx.commit().map_err(|err| migration_failed("commit", err))
}
