//! Slice-24 schema migration v4 — minted-philosophy storage table.
//!
//! ADR-059 §4.5 (minted storage layout). The `philosophies` table is the
//! queryable index for locally-minted `org.openlore.philosophy` records;
//! the authoritative artifact is the signed `<cid>.json` file under
//! `<root>/philosophies/` (written atomically by
//! `DuckDbStorageAdapter::write_signed_philosophy`).
//!
//! ## Migration policy (mirrors `schema_v3::run_migration`)
//!
//! - Forward-only; idempotent (`CREATE TABLE IF NOT EXISTS` +
//!   `CREATE INDEX IF NOT EXISTS`) guarded by the `schema_version` table.
//! - All existing slice-01/slice-03 tables are BIT-PRESERVED — this
//!   migration ADDS the one new table only, never alters existing ones.
//! - Registers `schema_version(version=4, applied_at=now(),
//!   description='slice-24 philosophy mint storage')`.
//! - On a slice-03 DB (version=3) this jumps to version=4; run AFTER
//!   `schema_v3::run_migration` from `DuckDbStorageAdapter::open`.
//!
//! ## Functional discipline
//!
//! Pure data (the SQL script + two named constants) plus ONE effectful
//! runner (`run_migration`) taking the sole effect — a `&mut Connection` —
//! that applies migration v4 within a single transaction or rolls it back.

use duckdb::Connection;
use ports::StorageError;

/// The schema version this slice introduces. Appended after `schema_v3`'s
/// head; never edited once shipped (forward-only contract).
pub const PHILOSOPHY_STORAGE_VERSION: i32 = 4;

/// Human-readable description recorded in the `schema_version` table.
pub const PHILOSOPHY_STORAGE_DESCRIPTION: &str = "slice-24 philosophy mint storage";

/// Migration v4 DDL — the single `philosophies` table + its indexes.
///
/// `object_id` carries a `UNIQUE` constraint: it is the deterministic
/// join key (ADR-059 D1) and no two minted records may occupy the same
/// vocabulary slot (AC-003.3 defense-in-depth — the CLI seed pre-check is
/// the primary refusal, this constraint catches minted-vs-minted). `cid`
/// is the content-address primary key; the on-disk `<cid>.json` artifact
/// is authoritative and `artifact_path` points at it. The `CHECK`
/// constraints mirror the peer-claims table's non-empty guards.
pub const PHILOSOPHY_STORAGE_SQL: &str = r"
    CREATE TABLE IF NOT EXISTS philosophies (
        cid           VARCHAR PRIMARY KEY,
        object_id     VARCHAR NOT NULL UNIQUE,
        name          VARCHAR NOT NULL,
        description   VARCHAR NOT NULL,
        author_did    VARCHAR NOT NULL,
        composed_at   TIMESTAMP NOT NULL,
        artifact_path VARCHAR NOT NULL,
        inserted_at   TIMESTAMP NOT NULL DEFAULT now(),
        CHECK (cid <> ''),
        CHECK (object_id <> ''),
        CHECK (author_did <> '')
    );

    CREATE INDEX IF NOT EXISTS idx_philosophies_object_id ON philosophies (object_id);
    CREATE INDEX IF NOT EXISTS idx_philosophies_author    ON philosophies (author_did);
";

/// Whether migration v4 has already been registered in `schema_version`.
///
/// Assumes `schema_version` exists (slice-01's `run_migrations` creates it;
/// we run AFTER that). Returns `Ok(true)` if a `version = 4` row exists.
fn already_applied(conn: &Connection) -> Result<bool, StorageError> {
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM schema_version WHERE version = ?",
            duckdb::params![PHILOSOPHY_STORAGE_VERSION],
            |row| row.get(0),
        )
        .map_err(|err| StorageError::SchemaMigrationFailed {
            message: format!("read schema_version v4 presence: {err}"),
        })?;
    Ok(count > 0)
}

/// Apply migration v4 idempotently within a single transaction.
///
/// Forward-only; safe to call on every `open`. If `schema_version` already
/// carries a `version=4` row this is a no-op. Otherwise it applies the v4
/// DDL and records the `schema_version(version=4, …)` row, committing both
/// atomically. Returns `Err(StorageError::SchemaMigrationFailed)` on any
/// SQL error; the transaction is rolled back so the DB stays at its prior
/// version.
pub fn run_migration(conn: &mut Connection) -> Result<(), StorageError> {
    if already_applied(conn)? {
        return Ok(());
    }

    let tx = conn
        .transaction()
        .map_err(|err| StorageError::SchemaMigrationFailed {
            message: format!("begin v4 migration tx: {err}"),
        })?;

    tx.execute_batch(PHILOSOPHY_STORAGE_SQL).map_err(|err| {
        StorageError::SchemaMigrationFailed {
            message: format!("apply migration v4: {err}"),
        }
    })?;

    tx.execute(
        "INSERT INTO schema_version (version, applied_at, description) \
         VALUES (?, now(), ?)",
        duckdb::params![PHILOSOPHY_STORAGE_VERSION, PHILOSOPHY_STORAGE_DESCRIPTION],
    )
    .map_err(|err| StorageError::SchemaMigrationFailed {
        message: format!("record migration v4: {err}"),
    })?;

    tx.commit()
        .map_err(|err| StorageError::SchemaMigrationFailed {
            message: format!("commit v4 migration tx: {err}"),
        })?;

    Ok(())
}
