//! `purge` — the index store's ONLY delete path (ADR-082): `IndexPurgePort`
//! over the same single DuckDB connection and artifact directory.
//!
//! check-arch `index_store_delete_only_in_purge` confines every parent-table
//! DELETE of this crate to this file; `index_purge_only_in_pass_runner`
//! confines its caller to the indexer's pass runner.
//!
//! ## One author's purge — resumable, not atomic (ADR-082 §6)
//!
//! 1. Read the author's rows: their CIDs and stored `signed_record_path`s.
//! 2. Delete each row's artifact file BY ITS STORED PATH ("already gone" is
//!    success). Never by a recomputed partition: `did_to_fs_segment` is not
//!    injective (`did:web:a:b` and `did:web:a_b` share one). A partition
//!    directory is removed only when the purge left it empty.
//! 3. Delete the child rows (evidence; OUTGOING references), committed.
//! 4. Delete the parent rows, committed.
//!
//! The author stays in `indexed_authors()` until step 4, so a crash at any
//! point leaves a state the next pass's plan finishes. Steps 3 and 4 are two
//! transactions: DuckDB's foreign-key check still sees child rows deleted
//! earlier in the SAME transaction, so a parent delete there is refused.

use std::collections::BTreeSet;
use std::fs;
use std::io::ErrorKind;
use std::path::{Component, Path, PathBuf};

use duckdb::Connection;
use ports::{IndexPurgePort, IndexStoreError, ProbeOutcome, ProbeRefusalReason, PurgeReport};

use crate::{bare_did, schema, IndexStoreAdapter};

/// The author match (ADR-079 contributor predicate): the bare DID itself or
/// any `bare#fragment` key — `starts_with`, never `LIKE`, so `%`/`_` in a DID
/// are never wildcards.
const AUTHOR_MATCH: &str = "author_did = $1 OR starts_with(author_did, $1 || '#')";

/// One claim row being purged.
struct PurgedRow {
    signed_record_path: String,
}

impl IndexPurgePort for IndexStoreAdapter {
    fn probe(&self) -> ProbeOutcome {
        let version = self.lock().and_then(|conn| schema::read_version(&conn));
        match version {
            Ok(v) if v == schema::LATEST_VERSION => {}
            Ok(v) => {
                return purge_probe_refused(
                    ProbeRefusalReason::StorageSchemaMismatch,
                    format!(
                        "index.duckdb schema version {v} != expected {} for purging",
                        schema::LATEST_VERSION
                    ),
                )
            }
            Err(err) => {
                return purge_probe_refused(
                    ProbeRefusalReason::StorageSchemaMismatch,
                    format!("could not read index schema version: {err}"),
                )
            }
        }
        match self.indexed_authors() {
            Ok(_) => ProbeOutcome::Ok,
            Err(err) => purge_probe_refused(
                ProbeRefusalReason::StorageSchemaMismatch,
                format!("could not list indexed authors: {err}"),
            ),
        }
    }

    fn indexed_authors(&self) -> Result<BTreeSet<String>, IndexStoreError> {
        let conn = self.lock()?;
        let mut stmt = conn
            .prepare("SELECT DISTINCT author_did FROM indexed_claims")
            .map_err(query_failed("prepare indexed authors"))?;
        let rows = stmt
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(query_failed("list indexed authors"))?;
        rows.map(|row| {
            row.map(|author_did| bare_did(&author_did).to_string())
                .map_err(query_failed("decode indexed author"))
        })
        .collect()
    }

    fn purge_author(&self, bare: &str) -> Result<PurgeReport, IndexStoreError> {
        let bare = bare_did(bare);
        let mut conn = self.lock()?;
        let rows = author_rows(&conn, bare)?;
        rows.iter()
            .try_for_each(|row| remove_artifact(&self.artifacts_root, &row.signed_record_path))?;
        delete_in_transaction(
            &mut conn,
            bare,
            &[
                &format!(
                    "DELETE FROM indexed_claim_evidence WHERE cid IN \
                     (SELECT cid FROM indexed_claims WHERE {AUTHOR_MATCH})"
                ),
                &format!(
                    "DELETE FROM indexed_claim_references WHERE referencing_cid IN \
                     (SELECT cid FROM indexed_claims WHERE {AUTHOR_MATCH})"
                ),
            ],
        )?;
        delete_in_transaction(
            &mut conn,
            bare,
            &[&format!("DELETE FROM indexed_claims WHERE {AUTHOR_MATCH}")],
        )?;
        Ok(PurgeReport {
            claims_removed: rows.len() as u64,
        })
    }
}

/// The author's claim rows, by the bare-DID match.
fn author_rows(conn: &Connection, bare: &str) -> Result<Vec<PurgedRow>, IndexStoreError> {
    let mut stmt = conn
        .prepare(&format!(
            "SELECT signed_record_path FROM indexed_claims WHERE {AUTHOR_MATCH}"
        ))
        .map_err(query_failed("prepare author rows"))?;
    let rows = stmt
        .query_map(duckdb::params![bare], |row| {
            Ok(PurgedRow {
                signed_record_path: row.get(0)?,
            })
        })
        .map_err(query_failed("read author rows"))?;
    rows.map(|row| row.map_err(query_failed("decode author row")))
        .collect()
}

/// Run `statements` (each bound to `bare`) in ONE committed transaction.
fn delete_in_transaction(
    conn: &mut Connection,
    bare: &str,
    statements: &[&String],
) -> Result<(), IndexStoreError> {
    let transaction = conn
        .transaction()
        .map_err(query_failed("begin purge transaction"))?;
    for sql in statements {
        transaction
            .execute(sql, duckdb::params![bare])
            .map_err(query_failed("purge rows"))?;
    }
    transaction
        .commit()
        .map_err(query_failed("commit purge transaction"))
}

/// Delete one artifact file at its stored path (relative to the index
/// directory, under `indexed_claims/`); a missing file is already purged. Its
/// partition directory goes too when that left it empty.
fn remove_artifact(artifacts_root: &Path, signed_record_path: &str) -> Result<(), IndexStoreError> {
    let Some(file) = artifact_file(artifacts_root, signed_record_path) else {
        return Ok(());
    };
    match fs::remove_file(&file) {
        Ok(()) => {}
        Err(err) if err.kind() == ErrorKind::NotFound => {}
        Err(err) => {
            return Err(IndexStoreError::QueryFailed {
                message: format!("remove artifact {}: {err}", file.display()),
            })
        }
    }
    if let Some(partition) = file.parent().filter(|dir| *dir != artifacts_root) {
        // Fails (and is ignored) unless the partition is now empty.
        let _ = fs::remove_dir(partition);
    }
    Ok(())
}

/// The file a stored `signed_record_path` names, only when it stays inside
/// the artifact root (`indexed_claims/<partition>/<file>`, plain components).
fn artifact_file(artifacts_root: &Path, signed_record_path: &str) -> Option<PathBuf> {
    let relative = Path::new(signed_record_path)
        .strip_prefix("indexed_claims")
        .ok()?;
    let plain = relative
        .components()
        .all(|component| matches!(component, Component::Normal(_)));
    (plain && relative.components().count() > 0).then(|| artifacts_root.join(relative))
}

fn query_failed(what: &'static str) -> impl Fn(duckdb::Error) -> IndexStoreError {
    move |err| IndexStoreError::QueryFailed {
        message: format!("{what}: {err}"),
    }
}

fn purge_probe_refused(reason: ProbeRefusalReason, detail: String) -> ProbeOutcome {
    ProbeOutcome::Refused {
        reason,
        detail,
        structured: serde_json::json!({"adapter": "index_purge"}),
    }
}
