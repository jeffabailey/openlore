//! Slice-03 schema migration v3 — peer-storage tables.
//!
//! The DDL is copied VERBATIM from
//! `docs/feature/openlore-federated-read/design/data-models.md
//! §"DuckDB schema — slice-03 additions (migration v3)"` — that doc IS
//! the spec. Any divergence here is a bug.
//!
//! ## Migration policy (data-models.md §"Migration policy")
//!
//! - Forward-only; idempotent (`CREATE TABLE IF NOT EXISTS` +
//!   `CREATE INDEX IF NOT EXISTS`).
//! - Slice-01 data (`claims`, `claim_evidence`, `claim_references`) is
//!   BIT-PRESERVED — this migration ADDS tables only, never alters
//!   existing ones.
//! - The migration registers `schema_version(version=3, applied_at=now(),
//!   description='slice-03 peer storage')`.
//! - On a slice-01 DB (version=1), the migration jumps to version=3.
//!   Version=2 is reserved for slice-02 (if installed separately); it is
//!   safe to skip if absent (each migration step is independent and
//!   gated only by `version > current`).
//!
//! ## Functional discipline
//!
//! Pure data (the SQL script + two named constants) plus ONE effectful
//! runner (`run_migration`) that takes the sole effect — a
//! `&mut Connection` — and either applies migration v3 within a single
//! transaction or rolls it back. No mid-migration partial state. The
//! slice-01 `schema.rs` migration runner is left untouched; v3 is
//! applied as an independent, idempotent follow-on step from
//! `DuckDbStorageAdapter::open` (mirrors the data-models.md policy
//! "slices may install in any order; each migration is independent").

use duckdb::Connection;
use ports::StorageError;

/// The schema version this slice introduces. Appended to the migration
/// list in `lib.rs`; never edited once shipped (forward-only contract).
pub const PEER_STORAGE_VERSION: i32 = 3;

/// Human-readable description recorded in the `schema_version` table for
/// audit. Matches the acceptance-criterion string exactly.
pub const PEER_STORAGE_DESCRIPTION: &str = "slice-03 peer storage";

/// Migration v3 DDL — the four peer-storage tables + their indexes.
///
/// Copied verbatim from data-models.md. The CHECK constraints,
/// partial index (`WHERE removed_at IS NULL`), and foreign keys are
/// load-bearing for the anti-merging invariants (I-FED-1 / I-FED-2) and
/// the soft-remove / hard-purge separation (WD-25 / ADR-014).
///
/// NOTE: there is intentionally NO foreign key on `peer_claims.author_did`
/// — soft-remove leaves dangling attribution per WD-25.
pub const PEER_STORAGE_SQL: &str = r"
    -- Subscriptions: one row per peer DID the user has chosen to follow.
    -- `removed_at` distinguishes 'active' from
    -- 'soft-removed-but-cache-retained' per WD-25. Hard-purge DELETEs the
    -- row entirely.
    CREATE TABLE IF NOT EXISTS peer_subscriptions (
        peer_did            VARCHAR PRIMARY KEY,
        peer_handle         VARCHAR NOT NULL,
        peer_pds_endpoint   VARCHAR NOT NULL,
        subscribed_at       TIMESTAMP NOT NULL,
        removed_at          TIMESTAMP
    );

    -- data-models.md specifies a PARTIAL index `WHERE removed_at IS NULL`,
    -- but DuckDB does not support partial indexes ('Creating partial
    -- indexes is not supported currently'). We create a plain index on
    -- the same column instead: it still accelerates the active-subscription
    -- lookup (the `removed_at IS NULL` predicate is applied at query time
    -- in `list_active_subscriptions`). Index name + covered column are
    -- preserved so the design intent and any future DuckDB upgrade path
    -- remain legible.
    CREATE INDEX IF NOT EXISTS idx_peer_subs_active
        ON peer_subscriptions (peer_did);

    -- Peer claims: signed claims authored by peers, NOT by the current
    -- user. LOAD-BEARING: author_did is NEVER NULL, NEVER empty. The
    -- anti-merging invariant (I-FED-1) makes every cross-store query
    -- carry this column.
    CREATE TABLE IF NOT EXISTS peer_claims (
        cid                 VARCHAR PRIMARY KEY,
        author_did          VARCHAR NOT NULL,
        subject             VARCHAR NOT NULL,
        predicate           VARCHAR NOT NULL,
        object              VARCHAR NOT NULL,
        confidence          DOUBLE  NOT NULL CHECK (confidence >= 0.0 AND confidence <= 1.0),
        composed_at         TIMESTAMP NOT NULL,
        fetched_at          TIMESTAMP NOT NULL,
        fetched_from_pds    VARCHAR NOT NULL,
        signed_record_path  VARCHAR NOT NULL,
        CHECK (author_did <> ''),
        CHECK (cid <> '')
    );

    CREATE INDEX IF NOT EXISTS idx_peer_claims_author       ON peer_claims (author_did);
    CREATE INDEX IF NOT EXISTS idx_peer_claims_subject      ON peer_claims (subject);
    CREATE INDEX IF NOT EXISTS idx_peer_claims_composed_at  ON peer_claims (composed_at);

    -- Reference graph for peer claims (denormalized from references[]
    -- field). Same shape as the slice-01 claim_references table; SEPARATE
    -- table preserves the author-store / peer-store separation invariant
    -- per ADR-014.
    CREATE TABLE IF NOT EXISTS peer_claim_references (
        referencing_cid     VARCHAR NOT NULL,
        referenced_cid      VARCHAR NOT NULL,
        ref_type            VARCHAR NOT NULL CHECK (ref_type IN ('retracts','corrects','counters','supersedes')),
        PRIMARY KEY (referencing_cid, referenced_cid, ref_type),
        FOREIGN KEY (referencing_cid) REFERENCES peer_claims (cid)
    );

    CREATE INDEX IF NOT EXISTS idx_peer_claim_refs_referenced ON peer_claim_references (referenced_cid);

    -- Evidence URIs for peer claims (denormalized; same shape as
    -- claim_evidence).
    CREATE TABLE IF NOT EXISTS peer_claim_evidence (
        cid         VARCHAR NOT NULL,
        evidence    VARCHAR NOT NULL,
        ordinal     INTEGER NOT NULL,
        PRIMARY KEY (cid, ordinal),
        FOREIGN KEY (cid) REFERENCES peer_claims (cid)
    );
";

/// Whether migration v3 has already been registered in `schema_version`.
///
/// Pure-ish query: reads a single COUNT. Returns `Ok(true)` if a row
/// with `version = PEER_STORAGE_VERSION` exists. Assumes `schema_version`
/// already exists (slice-01's `run_migrations` creates it; we only run
/// AFTER that).
fn already_applied(conn: &Connection) -> Result<bool, StorageError> {
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM schema_version WHERE version = ?",
            duckdb::params![PEER_STORAGE_VERSION],
            |row| row.get(0),
        )
        .map_err(|err| StorageError::SchemaMigrationFailed {
            message: format!("read schema_version v3 presence: {err}"),
        })?;
    Ok(count > 0)
}

/// Apply migration v3 idempotently within a single transaction.
///
/// Forward-only; safe to call on every `open`. If the `schema_version`
/// table already carries a `version=3` row, this is a no-op (returns
/// immediately without touching the DB). Otherwise it applies the v3 DDL
/// and records the `schema_version(version=3, ..., 'slice-03 peer
/// storage')` row, committing both atomically.
///
/// Returns `Err(StorageError::SchemaMigrationFailed)` on any SQL error;
/// the transaction is rolled back so the DB stays at its prior version.
pub fn run_migration(conn: &mut Connection) -> Result<(), StorageError> {
    if already_applied(conn)? {
        return Ok(());
    }

    let tx = conn
        .transaction()
        .map_err(|err| StorageError::SchemaMigrationFailed {
            message: format!("begin v3 migration tx: {err}"),
        })?;

    tx.execute_batch(PEER_STORAGE_SQL)
        .map_err(|err| StorageError::SchemaMigrationFailed {
            message: format!("apply migration v3: {err}"),
        })?;

    tx.execute(
        "INSERT INTO schema_version (version, applied_at, description) \
         VALUES (?, now(), ?)",
        duckdb::params![PEER_STORAGE_VERSION, PEER_STORAGE_DESCRIPTION],
    )
    .map_err(|err| StorageError::SchemaMigrationFailed {
        message: format!("record migration v3: {err}"),
    })?;

    tx.commit()
        .map_err(|err| StorageError::SchemaMigrationFailed {
            message: format!("commit v3 migration tx: {err}"),
        })?;

    Ok(())
}
