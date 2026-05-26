//! `adapter-duckdb` — embedded DuckDB-backed `StoragePort` implementation.
//!
//! Writes signed-claim JSON files alongside the DB so the on-disk layout
//! matches `data-models.md`. Manages schema migrations. Probe verifies
//! schema-version match + sentinel round-trip + fsync honored
//! per ADR-001 / `component-boundaries.md §"crates/adapter-duckdb"`.
//!
//! ## Architecture (nw-fp-hexagonal-architecture)
//!
//! This crate is the EFFECT shell for the `StoragePort` trait defined
//! in `crates/ports`. The pure core never imports this; the
//! composition root (`crates/cli`) wires a `DuckDbStorageAdapter`
//! behind the `StoragePort` interface.
//!
//! ## On-disk layout (data-models.md)
//!
//! ```text
//! <root>/openlore.duckdb              # the DB file
//! <root>/claims/<cid>.json            # signed-claim artifact files
//! ```
//!
//! ## Write strategy (data-models.md §"Write strategy")
//!
//! `write_signed_claim` is the only multi-slot mutation. The contract
//! is "DB row + artifact file in one transaction-equivalent":
//!
//! 1. Begin DuckDB transaction.
//! 2. Write `<cid>.json.tmp`, `sync_all`, rename to `<cid>.json`
//!    (atomic per POSIX `rename(2)`).
//! 3. INSERT into `claims` + `claim_evidence` + `claim_references`.
//! 4. Commit the transaction.
//!
//! If step 4 fails, the artifact file is left in place — it is
//! reconcilable on restart (DuckDB row absent = artifact orphan,
//! observable). The reverse failure (artifact write fails, DB row
//! never inserted) is the dominant safe-failure direction.

#![allow(dead_code)]
#![forbid(unsafe_code)]

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use chrono::{DateTime, NaiveDateTime, Utc};
use claim_domain::{Cid, ReferenceType, SignedClaim};
use duckdb::Connection;
use ports::{ProbeOutcome, StorageError, StoragePort};

mod probe;
mod schema;

/// Embedded-DuckDB `StoragePort` adapter.
///
/// Holds the open DB handle (behind a `Mutex` because `Connection` is
/// `!Sync`) plus the path to the colocated `claims/` directory.
pub struct DuckDbStorageAdapter {
    conn: Mutex<Connection>,
    claims_dir: PathBuf,
}

impl DuckDbStorageAdapter {
    /// Open the DB at the given path; run pending migrations; prepare
    /// the colocated `claims/` directory. Idempotent across reopens.
    pub fn open(db_path: &Path) -> Result<Self, StorageError> {
        // Ensure the parent directory exists (DuckDB will create the
        // DB file but not its parent).
        if let Some(parent) = db_path.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent).map_err(|err| StorageError::SchemaMigrationFailed {
                    message: format!("create db parent dir: {err}"),
                })?;
            }
        }

        let mut conn =
            Connection::open(db_path).map_err(|err| StorageError::SchemaMigrationFailed {
                message: format!("open duckdb at {}: {err}", db_path.display()),
            })?;

        schema::run_migrations(&mut conn)?;

        // Colocate `claims/` next to the DB file. data-models.md
        // §"DuckDB schema" defines the canonical layout
        // `~/.local/share/openlore/openlore.duckdb` +
        // `~/.local/share/openlore/claims/<cid>.json`.
        let claims_dir = db_path
            .parent()
            .map(|p| p.join("claims"))
            .unwrap_or_else(|| PathBuf::from("claims"));
        fs::create_dir_all(&claims_dir).map_err(|err| StorageError::SchemaMigrationFailed {
            message: format!("create claims dir {}: {err}", claims_dir.display()),
        })?;

        Ok(Self {
            conn: Mutex::new(conn),
            claims_dir,
        })
    }

    /// Construct the artifact path for a CID: `<claims_dir>/<cid>.json`.
    fn artifact_path(&self, cid: &Cid) -> PathBuf {
        self.claims_dir.join(format!("{}.json", cid.0))
    }
}

// -----------------------------------------------------------------------------
// `StoragePort` impl — port-shaped, railway-oriented (nw-fp-domain-modeling §8)
// -----------------------------------------------------------------------------

impl StoragePort for DuckDbStorageAdapter {
    fn probe(&self) -> ProbeOutcome {
        let conn = match self.conn.lock() {
            Ok(c) => c,
            Err(_) => {
                return ProbeOutcome::Refused {
                    reason: ports::ProbeRefusalReason::StorageFsyncUnreliable,
                    detail: "connection mutex poisoned".to_string(),
                    structured: serde_json::json!({}),
                };
            }
        };
        probe::run_probe(&conn, &self.claims_dir)
    }

    fn write_signed_claim(&self, signed: &SignedClaim) -> Result<(), StorageError> {
        let cid = &signed.signature.signed_cid;
        let artifact = self.artifact_path(cid);
        let artifact_tmp = artifact.with_extension("json.tmp");

        // Step 1: serialize the SignedClaim once. Both the file write
        // and the eventual at_uri reconstruction use the same bytes.
        let json_bytes =
            serde_json::to_vec_pretty(signed).map_err(|err| StorageError::WriteFailed {
                cid: cid.clone(),
                message: format!("serialize signed claim: {err}"),
            })?;

        // Step 2: atomic file write — tmp + fsync + rename. POSIX
        // guarantees rename(2) is atomic on the same filesystem.
        {
            let mut f =
                fs::File::create(&artifact_tmp).map_err(|err| StorageError::WriteFailed {
                    cid: cid.clone(),
                    message: format!("create {}: {err}", artifact_tmp.display()),
                })?;
            f.write_all(&json_bytes)
                .map_err(|err| StorageError::WriteFailed {
                    cid: cid.clone(),
                    message: format!("write {}: {err}", artifact_tmp.display()),
                })?;
            f.sync_all().map_err(|err| StorageError::WriteFailed {
                cid: cid.clone(),
                message: format!("sync_all {}: {err}", artifact_tmp.display()),
            })?;
        }
        fs::rename(&artifact_tmp, &artifact).map_err(|err| StorageError::WriteFailed {
            cid: cid.clone(),
            message: format!(
                "rename {} -> {}: {err}",
                artifact_tmp.display(),
                artifact.display()
            ),
        })?;

        // Step 3: DB transaction — insert claim + evidence + references.
        let mut conn = self.conn.lock().map_err(|_| StorageError::WriteFailed {
            cid: cid.clone(),
            message: "connection mutex poisoned".to_string(),
        })?;

        let tx = conn
            .transaction()
            .map_err(|err| StorageError::WriteFailed {
                cid: cid.clone(),
                message: format!("begin tx: {err}"),
            })?;

        let composed_at_naive = parse_composed_at(&signed.unsigned.composed_at).map_err(|err| {
            StorageError::WriteFailed {
                cid: cid.clone(),
                message: err,
            }
        })?;
        let confidence_f64 = confidence_to_f64(&signed.unsigned.confidence).map_err(|_| {
            StorageError::WriteFailed {
                cid: cid.clone(),
                message: "confidence value extraction failed".to_string(),
            }
        })?;

        tx.execute(
            "INSERT INTO claims (cid, subject, predicate, object, confidence, \
             author_did, composed_at, artifact_path) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            duckdb::params![
                cid.0,
                signed.unsigned.subject,
                signed.unsigned.predicate,
                signed.unsigned.object,
                confidence_f64,
                signed.unsigned.author_did.0,
                composed_at_naive,
                artifact.display().to_string(),
            ],
        )
        .map_err(|err| StorageError::WriteFailed {
            cid: cid.clone(),
            message: format!("insert into claims: {err}"),
        })?;

        for (ordinal, evidence) in signed.unsigned.evidence.iter().enumerate() {
            tx.execute(
                "INSERT INTO claim_evidence (cid, evidence, ordinal) VALUES (?, ?, ?)",
                duckdb::params![cid.0, evidence, ordinal as i32],
            )
            .map_err(|err| StorageError::WriteFailed {
                cid: cid.clone(),
                message: format!("insert into claim_evidence: {err}"),
            })?;
        }

        for r in &signed.unsigned.references {
            tx.execute(
                "INSERT INTO claim_references (referencing_cid, referenced_cid, ref_type) \
                 VALUES (?, ?, ?)",
                duckdb::params![cid.0, r.cid.0, reference_type_to_sql(r.ref_type)],
            )
            .map_err(|err| StorageError::WriteFailed {
                cid: cid.clone(),
                message: format!("insert into claim_references: {err}"),
            })?;
        }

        tx.commit().map_err(|err| StorageError::WriteFailed {
            cid: cid.clone(),
            message: format!("commit tx: {err}"),
        })?;

        Ok(())
    }

    fn read_signed_claim(&self, cid: &Cid) -> Result<Option<SignedClaim>, StorageError> {
        // The on-disk JSON file is the authoritative artifact
        // (data-models.md). Read from it, not from the derived DB
        // index, so byte-equality is guaranteed.
        let conn = self.conn.lock().map_err(|_| StorageError::ReadFailed {
            cid: cid.clone(),
            message: "connection mutex poisoned".to_string(),
        })?;

        let artifact_path: Option<String> = conn
            .query_row(
                "SELECT artifact_path FROM claims WHERE cid = ?",
                duckdb::params![cid.0],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(|err| StorageError::ReadFailed {
                cid: cid.clone(),
                message: format!("query claims: {err}"),
            })?;

        drop(conn);

        let Some(artifact_path) = artifact_path else {
            return Ok(None);
        };

        let bytes = fs::read(&artifact_path).map_err(|err| StorageError::ReadFailed {
            cid: cid.clone(),
            message: format!("read artifact {artifact_path}: {err}"),
        })?;

        let claim: SignedClaim =
            serde_json::from_slice(&bytes).map_err(|err| StorageError::ReadFailed {
                cid: cid.clone(),
                message: format!("deserialize artifact: {err}"),
            })?;

        Ok(Some(claim))
    }

    fn query_by_subject(&self, subject: &str) -> Result<Vec<SignedClaim>, StorageError> {
        let conn = self.conn.lock().map_err(|_| StorageError::QueryFailed {
            message: "connection mutex poisoned".to_string(),
        })?;

        let mut stmt = conn
            .prepare("SELECT cid FROM claims WHERE subject = ? ORDER BY cid")
            .map_err(|err| StorageError::QueryFailed {
                message: format!("prepare query_by_subject: {err}"),
            })?;

        let cid_iter = stmt
            .query_map(duckdb::params![subject], |row| {
                row.get::<_, String>(0).map(Cid)
            })
            .map_err(|err| StorageError::QueryFailed {
                message: format!("query_map: {err}"),
            })?;

        let mut cids: Vec<Cid> = Vec::new();
        for cid in cid_iter {
            cids.push(cid.map_err(|err| StorageError::QueryFailed {
                message: format!("row decode: {err}"),
            })?);
        }
        drop(stmt);
        drop(conn);

        let mut results = Vec::with_capacity(cids.len());
        for cid in cids {
            if let Some(claim) = self.read_signed_claim(&cid)? {
                results.push(claim);
            }
        }
        Ok(results)
    }

    fn query_referencing(
        &self,
        target_cid: &Cid,
    ) -> Result<Vec<(Cid, ReferenceType)>, StorageError> {
        let conn = self.conn.lock().map_err(|_| StorageError::QueryFailed {
            message: "connection mutex poisoned".to_string(),
        })?;

        let mut stmt = conn
            .prepare(
                "SELECT referencing_cid, ref_type FROM claim_references \
                 WHERE referenced_cid = ?",
            )
            .map_err(|err| StorageError::QueryFailed {
                message: format!("prepare query_referencing: {err}"),
            })?;

        let rows = stmt
            .query_map(duckdb::params![target_cid.0], |row| {
                let cid_str: String = row.get(0)?;
                let ref_type_str: String = row.get(1)?;
                Ok((cid_str, ref_type_str))
            })
            .map_err(|err| StorageError::QueryFailed {
                message: format!("query_map: {err}"),
            })?;

        let mut results = Vec::new();
        for row in rows {
            let (cid_str, ref_type_str) = row.map_err(|err| StorageError::QueryFailed {
                message: format!("row decode: {err}"),
            })?;
            let ref_type = reference_type_from_sql(&ref_type_str)
                .map_err(|err| StorageError::QueryFailed { message: err })?;
            results.push((Cid(cid_str), ref_type));
        }

        Ok(results)
    }

    fn record_publication(
        &self,
        cid: &Cid,
        at_uri: &str,
        published_at: DateTime<Utc>,
    ) -> Result<(), StorageError> {
        let conn = self.conn.lock().map_err(|_| StorageError::WriteFailed {
            cid: cid.clone(),
            message: "connection mutex poisoned".to_string(),
        })?;

        let published_naive = published_at.naive_utc();

        conn.execute(
            "UPDATE claims SET at_uri = ?, published_at = ? WHERE cid = ?",
            duckdb::params![at_uri, published_naive, cid.0],
        )
        .map_err(|err| StorageError::WriteFailed {
            cid: cid.clone(),
            message: format!("update record_publication: {err}"),
        })?;

        Ok(())
    }
}

// -----------------------------------------------------------------------------
// Small named helpers (nw-fp-usable-design — never inline a one-liner that
// has a domain name)
// -----------------------------------------------------------------------------

/// `ReferenceType` → wire string used in the SQL `CHECK` constraint.
fn reference_type_to_sql(rt: ReferenceType) -> &'static str {
    match rt {
        ReferenceType::Retracts => "retracts",
        ReferenceType::Corrects => "corrects",
        ReferenceType::Counters => "counters",
        ReferenceType::Supersedes => "supersedes",
    }
}

/// Inverse of `reference_type_to_sql`. Returns `Err(msg)` on unknown
/// strings — defends against direct DB tampering producing a string
/// outside the four known variants.
fn reference_type_from_sql(s: &str) -> Result<ReferenceType, String> {
    match s {
        "retracts" => Ok(ReferenceType::Retracts),
        "corrects" => Ok(ReferenceType::Corrects),
        "counters" => Ok(ReferenceType::Counters),
        "supersedes" => Ok(ReferenceType::Supersedes),
        other => Err(format!("unknown ref_type in DB: {other:?}")),
    }
}

/// Extract the inner `f64` from a `Confidence` wrapper. We route
/// through serde because the wrapper's `value()` accessor is still a
/// RED-scaffold panic at this slice; serde access goes through the
/// derived `Serialize` impl (`#[derive(Serialize)]` on a tuple struct
/// emits the inner primitive directly). Mirrors the pattern used in
/// `test_support::fixtures::confidence`.
fn confidence_to_f64(c: &claim_domain::Confidence) -> Result<f64, String> {
    serde_json::to_value(c)
        .ok()
        .and_then(|v| v.as_f64())
        .ok_or_else(|| "confidence serde returned non-number".to_string())
}

/// Parse the RFC3339 `composedAt` string into a `NaiveDateTime` (UTC)
/// suitable for the `TIMESTAMP` column. The signed-claim wire format
/// uses RFC3339 with `Z` suffix; we accept the broader RFC3339 shape
/// and re-normalize to UTC.
fn parse_composed_at(s: &str) -> Result<NaiveDateTime, String> {
    DateTime::parse_from_rfc3339(s)
        .map(|dt| dt.with_timezone(&Utc).naive_utc())
        .map_err(|err| format!("parse composed_at {s:?}: {err}"))
}

// -----------------------------------------------------------------------------
// `OptionalExtension` shim — duckdb 1.x doesn't ship `Option<T>` ext for
// `query_row` so we mirror the rusqlite trick locally.
// -----------------------------------------------------------------------------

trait OptionalExtension<T> {
    fn optional(self) -> Result<Option<T>, duckdb::Error>;
}

impl<T> OptionalExtension<T> for Result<T, duckdb::Error> {
    fn optional(self) -> Result<Option<T>, duckdb::Error> {
        match self {
            Ok(v) => Ok(Some(v)),
            Err(duckdb::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }
}
