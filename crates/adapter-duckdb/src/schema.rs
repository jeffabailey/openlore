//! DuckDB schema definitions + forward-only migration runner.
//!
//! The schema is copied verbatim from
//! `docs/feature/openlore-foundation/design/data-models.md §"DuckDB schema"` —
//! that doc IS the spec. Any divergence here is a bug.
//!
//! ## Migration policy (data-models.md §"DuckDB schema")
//!
//! - Forward-only. There is no `down` direction in slice-01.
//! - Idempotent: `CREATE TABLE IF NOT EXISTS` + a guard on
//!   `schema_version` so reopening the DB twice is a no-op.
//! - The probe refuses to open a DB whose `schema_version` is HIGHER
//!   than `LATEST_VERSION` (the binary is older than the file).
//!
//! ## Functional discipline
//!
//! Pure data + total functions. The migration runner takes a
//! `&mut duckdb::Connection` (the only "effect") and either applies
//! all pending migrations within a single transaction or rolls them
//! back. No mid-migration partial state.

use duckdb::Connection;
use ports::StorageError;

/// The schema version this binary knows how to read. A file recording
/// a HIGHER version is a "future file" — the probe refuses to start.
pub const LATEST_VERSION: i32 = 1;

/// One migration step. Each step is a SQL script applied within a
/// transaction; `description` lands in the `schema_version` table for
/// audit. Forward-only by construction.
struct Migration {
    version: i32,
    description: &'static str,
    sql: &'static str,
}

/// The full migration list. To add a new schema version, append a
/// `Migration` here; never edit an existing entry.
const MIGRATIONS: &[Migration] = &[Migration {
    version: 1,
    description: "initial schema: claims, claim_evidence, claim_references",
    sql: r"
        -- claims: core index. The on-disk JSON files are the
        -- authoritative artifact; this table is a derived index for
        -- query speed (data-models.md §'DuckDB schema').
        CREATE TABLE IF NOT EXISTS claims (
            cid           VARCHAR PRIMARY KEY,
            subject       VARCHAR NOT NULL,
            predicate     VARCHAR NOT NULL,
            object        VARCHAR NOT NULL,
            confidence    DOUBLE  NOT NULL CHECK (confidence >= 0.0 AND confidence <= 1.0),
            author_did    VARCHAR NOT NULL,
            composed_at   TIMESTAMP NOT NULL,
            published_at  TIMESTAMP,
            at_uri        VARCHAR,
            artifact_path VARCHAR NOT NULL,
            inserted_at   TIMESTAMP NOT NULL DEFAULT now()
        );

        CREATE INDEX IF NOT EXISTS idx_claims_subject     ON claims (subject);
        CREATE INDEX IF NOT EXISTS idx_claims_author      ON claims (author_did);
        CREATE INDEX IF NOT EXISTS idx_claims_composed_at ON claims (composed_at);

        CREATE TABLE IF NOT EXISTS claim_evidence (
            cid         VARCHAR NOT NULL,
            evidence    VARCHAR NOT NULL,
            ordinal     INTEGER NOT NULL,
            PRIMARY KEY (cid, ordinal),
            FOREIGN KEY (cid) REFERENCES claims (cid)
        );

        CREATE TABLE IF NOT EXISTS claim_references (
            referencing_cid VARCHAR NOT NULL,
            referenced_cid  VARCHAR NOT NULL,
            ref_type        VARCHAR NOT NULL CHECK (
                ref_type IN ('retracts','corrects','counters','supersedes')
            ),
            PRIMARY KEY (referencing_cid, referenced_cid, ref_type),
            FOREIGN KEY (referencing_cid) REFERENCES claims (cid)
        );

        CREATE INDEX IF NOT EXISTS idx_claim_references_referenced
            ON claim_references (referenced_cid);
    ",
}];

/// Ensure `schema_version` exists and return the current applied
/// version, or `0` if no migrations have been applied yet.
fn current_version(conn: &Connection) -> Result<i32, StorageError> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_version (
            version     INTEGER PRIMARY KEY,
            applied_at  TIMESTAMP NOT NULL,
            description VARCHAR  NOT NULL
        );",
    )
    .map_err(|err| StorageError::SchemaMigrationFailed {
        message: format!("create schema_version: {err}"),
    })?;

    let version: i32 = conn
        .query_row(
            "SELECT COALESCE(MAX(version), 0) FROM schema_version",
            [],
            |row| row.get(0),
        )
        .map_err(|err| StorageError::SchemaMigrationFailed {
            message: format!("read schema_version: {err}"),
        })?;

    Ok(version)
}

/// Apply every migration with `version > current` within a single
/// transaction. Returns the post-migration version. Idempotent: if
/// `current >= LATEST_VERSION`, returns immediately without touching
/// the DB.
///
/// Returns `Err(StorageError::SchemaMigrationFailed)` on any SQL error;
/// the transaction is rolled back so the DB stays at `current`.
pub fn run_migrations(conn: &mut Connection) -> Result<i32, StorageError> {
    let current = current_version(conn)?;

    if current >= LATEST_VERSION {
        return Ok(current);
    }

    let tx = conn
        .transaction()
        .map_err(|err| StorageError::SchemaMigrationFailed {
            message: format!("begin migration tx: {err}"),
        })?;

    for migration in MIGRATIONS.iter().filter(|m| m.version > current) {
        tx.execute_batch(migration.sql)
            .map_err(|err| StorageError::SchemaMigrationFailed {
                message: format!("apply migration v{}: {err}", migration.version),
            })?;

        tx.execute(
            "INSERT INTO schema_version (version, applied_at, description) \
             VALUES (?, now(), ?)",
            duckdb::params![migration.version, migration.description],
        )
        .map_err(|err| StorageError::SchemaMigrationFailed {
            message: format!("record migration v{}: {err}", migration.version),
        })?;
    }

    tx.commit()
        .map_err(|err| StorageError::SchemaMigrationFailed {
            message: format!("commit migration tx: {err}"),
        })?;

    Ok(LATEST_VERSION)
}

/// Read the current `schema_version` for probe purposes. Does NOT run
/// any migrations.
pub fn read_version(conn: &Connection) -> Result<i32, StorageError> {
    current_version(conn)
}
