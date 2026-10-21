//! Migration v9 (owner-only-signing slice 5, OOS-D12 B, C-OOS-7): the
//! insert-only `imported_candidates` table — the candidates each import
//! derived, kept locally for offline review. One row per derived candidate;
//! the rows of one import share an `import_id` (one per import, ascending)
//! and its `imported_at`, so a later read can prefer the latest import of a
//! candidate (max `imported_at`, then max `import_id`). There is no signing
//! column: signing re-derives through the authority gate. Never federated,
//! never read by scoring or inference. Additive, idempotent, forward-only,
//! gated on its own `schema_version` row (same shape as v8).

use duckdb::Connection;
use ports::StorageError;

pub const IMPORTED_CANDIDATES_VERSION: i32 = 9;

pub const IMPORTED_CANDIDATES_DESCRIPTION: &str =
    "owner-only-signing imported_candidates (OOS-D12 B, C-OOS-7)";

pub const IMPORTED_CANDIDATES_SQL: &str = r"
    CREATE SEQUENCE IF NOT EXISTS imported_candidate_ids START 1;
    CREATE SEQUENCE IF NOT EXISTS candidate_import_ids START 1;
    CREATE TABLE IF NOT EXISTS imported_candidates (
        candidate_id BIGINT PRIMARY KEY DEFAULT nextval('imported_candidate_ids'),
        import_id    BIGINT NOT NULL,
        subject      VARCHAR NOT NULL,
        subject_key  VARCHAR NOT NULL,
        predicate    VARCHAR NOT NULL,
        object       VARCHAR NOT NULL,
        confidence   DOUBLE NOT NULL,
        evidence     VARCHAR[] NOT NULL,
        imported_at  TIMESTAMP NOT NULL
    );
    CREATE INDEX IF NOT EXISTS idx_imported_candidates_subject
        ON imported_candidates (subject_key);
";

fn already_applied(conn: &Connection) -> Result<bool, StorageError> {
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM schema_version WHERE version = ?",
            duckdb::params![IMPORTED_CANDIDATES_VERSION],
            |row| row.get(0),
        )
        .map_err(|err| StorageError::SchemaMigrationFailed {
            message: format!("read schema_version v9 presence: {err}"),
        })?;
    Ok(count > 0)
}

pub fn run_migration(conn: &mut Connection) -> Result<(), StorageError> {
    if already_applied(conn)? {
        return Ok(());
    }
    let migration_failed = |step: &str, err: duckdb::Error| StorageError::SchemaMigrationFailed {
        message: format!("{step} v9 migration: {err}"),
    };
    let tx = conn
        .transaction()
        .map_err(|err| migration_failed("begin", err))?;
    tx.execute_batch(IMPORTED_CANDIDATES_SQL)
        .map_err(|err| migration_failed("apply", err))?;
    tx.execute(
        "INSERT INTO schema_version (version, applied_at, description) VALUES (?, now(), ?)",
        duckdb::params![IMPORTED_CANDIDATES_VERSION, IMPORTED_CANDIDATES_DESCRIPTION],
    )
    .map_err(|err| migration_failed("record", err))?;
    tx.commit().map_err(|err| migration_failed("commit", err))
}
