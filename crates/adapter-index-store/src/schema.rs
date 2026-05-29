//! `schema` — the `index.duckdb` migration v1 DDL + forward-only runner (ADR-025).
//!
//! The schema is copied verbatim from ADR-025 / data-models.md §"Network index
//! schema". That doc IS the spec; any divergence here is a bug.
//!
//! ## The single load-bearing schema decision (WD-103)
//!
//! ONE `indexed_claims` table with a NON-`Option` (`NOT NULL` + `CHECK <> ''`)
//! `author_did`, plus two child tables (`indexed_claim_evidence`,
//! `indexed_claim_references`). There is NO `consensus` / `merged` / `aggregate`
//! table of ANY kind — the absence is the design. Aggregates
//! (`distinct_author_count`) are composed at QUERY time in the PURE
//! `appview-domain` core, NEVER stored as a merged row nor produced by an
//! author-eliding SQL aggregate.
//!
//! ## Migration policy (mirrors `adapter-duckdb::schema`)
//!
//! - Forward-only. Idempotent: `CREATE TABLE IF NOT EXISTS` + an
//!   `index_schema_version` guard so reopening the DB twice is a no-op.
//! - The migration runner takes a `&mut duckdb::Connection` (the only effect)
//!   and applies all pending migrations within a single transaction or rolls
//!   them back. No mid-migration partial state.

use duckdb::Connection;
use ports::IndexStoreError;

/// The index-store schema version this binary knows how to read.
pub const LATEST_VERSION: i32 = 1;

/// One migration step. Forward-only by construction.
struct Migration {
    version: i32,
    description: &'static str,
    sql: &'static str,
}

/// The full migration list. To add a new schema version, append a `Migration`
/// here; never edit an existing entry.
const MIGRATIONS: &[Migration] = &[Migration {
    version: 1,
    description: "slice-05 network index: indexed_claims + evidence + references",
    sql: r"
        -- The SEPARATE index store (ADR-023): the indexer NEVER touches the
        -- user's openlore.duckdb. ONE indexed_claims table, non-Option
        -- author_did, NO merged/consensus/aggregate table (WD-103).
        CREATE TABLE IF NOT EXISTS indexed_claims (
            cid                 VARCHAR PRIMARY KEY,
            author_did          VARCHAR NOT NULL,
            subject             VARCHAR NOT NULL,
            predicate           VARCHAR NOT NULL,
            object              VARCHAR NOT NULL,
            confidence          DOUBLE  NOT NULL CHECK (confidence >= 0.0 AND confidence <= 1.0),
            composed_at         TIMESTAMP NOT NULL,
            indexed_at          TIMESTAMP NOT NULL,
            source_pds          VARCHAR NOT NULL,
            signed_record_path  VARCHAR NOT NULL,
            verified_against    VARCHAR NOT NULL,
            CHECK (author_did <> ''),
            CHECK (cid <> ''),
            CHECK (verified_against <> '')
        );
        CREATE INDEX IF NOT EXISTS idx_indexed_object       ON indexed_claims (object);
        CREATE INDEX IF NOT EXISTS idx_indexed_author       ON indexed_claims (author_did);
        CREATE INDEX IF NOT EXISTS idx_indexed_subject      ON indexed_claims (subject);
        CREATE INDEX IF NOT EXISTS idx_indexed_composed_at  ON indexed_claims (composed_at);

        CREATE TABLE IF NOT EXISTS indexed_claim_evidence (
            cid VARCHAR NOT NULL, evidence VARCHAR NOT NULL, ordinal INTEGER NOT NULL,
            PRIMARY KEY (cid, ordinal), FOREIGN KEY (cid) REFERENCES indexed_claims (cid)
        );

        CREATE TABLE IF NOT EXISTS indexed_claim_references (
            referencing_cid VARCHAR NOT NULL, referenced_cid VARCHAR NOT NULL,
            ref_type VARCHAR NOT NULL CHECK (ref_type IN ('retracts','corrects','counters','supersedes')),
            PRIMARY KEY (referencing_cid, referenced_cid, ref_type),
            FOREIGN KEY (referencing_cid) REFERENCES indexed_claims (cid)
        );
        CREATE INDEX IF NOT EXISTS idx_indexed_refs_referenced ON indexed_claim_references (referenced_cid);
    ",
}];

/// Ensure `index_schema_version` exists and return the current applied version,
/// or `0` if no migrations have been applied yet.
fn current_version(conn: &Connection) -> Result<i32, IndexStoreError> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS index_schema_version (
            version     INTEGER PRIMARY KEY,
            applied_at  TIMESTAMP NOT NULL,
            description VARCHAR  NOT NULL
        );",
    )
    .map_err(|err| IndexStoreError::SchemaMigrationFailed {
        message: format!("create index_schema_version: {err}"),
    })?;

    let version: i32 = conn
        .query_row(
            "SELECT COALESCE(MAX(version), 0) FROM index_schema_version",
            [],
            |row| row.get(0),
        )
        .map_err(|err| IndexStoreError::SchemaMigrationFailed {
            message: format!("read index_schema_version: {err}"),
        })?;

    Ok(version)
}

/// Apply every migration with `version > current` within a single transaction.
/// Idempotent: if `current >= LATEST_VERSION`, returns immediately.
pub fn run_migrations(conn: &mut Connection) -> Result<i32, IndexStoreError> {
    let current = current_version(conn)?;
    if current >= LATEST_VERSION {
        return Ok(current);
    }

    let tx = conn
        .transaction()
        .map_err(|err| IndexStoreError::SchemaMigrationFailed {
            message: format!("begin index migration tx: {err}"),
        })?;

    for migration in MIGRATIONS.iter().filter(|m| m.version > current) {
        tx.execute_batch(migration.sql)
            .map_err(|err| IndexStoreError::SchemaMigrationFailed {
                message: format!("apply index migration v{}: {err}", migration.version),
            })?;
        tx.execute(
            "INSERT INTO index_schema_version (version, applied_at, description) \
             VALUES (?, now(), ?)",
            duckdb::params![migration.version, migration.description],
        )
        .map_err(|err| IndexStoreError::SchemaMigrationFailed {
            message: format!("record index migration v{}: {err}", migration.version),
        })?;
    }

    tx.commit()
        .map_err(|err| IndexStoreError::SchemaMigrationFailed {
            message: format!("commit index migration tx: {err}"),
        })?;

    Ok(LATEST_VERSION)
}

/// Read the current `index_schema_version` for probe purposes. Does NOT run any
/// migrations.
pub fn read_version(conn: &Connection) -> Result<i32, IndexStoreError> {
    current_version(conn)
}
