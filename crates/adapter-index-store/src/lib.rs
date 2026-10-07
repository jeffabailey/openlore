//! `adapter-index-store` — embedded-DuckDB `IndexStorePort` over `index.duckdb`.
//!
//! EFFECT shell for the `IndexStorePort` trait (`crates/ports`). SYNC (local DB)
//! like `adapter-duckdb`'s `StoragePort`. Operates over a SEPARATE
//! `index.duckdb` file (ADR-023/025) — the indexer NEVER touches the user's
//! `openlore.duckdb`. The index is a re-buildable cache of signature-verified,
//! per-author-attributed PUBLIC claims.
//!
//! ## The load-bearing anti-merging contract (WD-103 / I-AV-2)
//!
//! Every query method returns PER-CLAIM [`IndexedClaim`] rows whose
//! `author_did` is NON-`Option` — two identical-content claims by different
//! authors stay TWO rows. There is NO method that aggregates across authors and
//! NO `consensus` / `merged` table: `distinct_author_count` is composed in the
//! PURE `appview-domain` core, NEVER in SQL. The extended
//! `no_cross_table_join_elides_author` `xtask check-arch` rule scans THIS
//! crate's SQL literals and fails CI on any `indexed_claims` aggregate that
//! drops `author_did`. De-dup at `upsert` is by CID only (ADR-025).
//!
//! ## Architecture (nw-fp-hexagonal-architecture)
//!
//! Pure core (claim-domain, appview-domain) never imports this crate; the
//! indexer composition root wires an [`IndexStoreAdapter`] behind the
//! `IndexStorePort` interface. `appview_domain::compose_results` consumes the
//! attributed rows this adapter returns (the dependency on `appview-domain` is
//! the shared boundary-value home, not a behavioral coupling).
//
// SCAFFOLD: false  (step 03-01: live DDL + query bodies for the AV-1 walking
// skeleton; the broader query surface is exercised by AV-2..7 in 03-02..03-07).

#![allow(dead_code)]
#![forbid(unsafe_code)]

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use chrono::{DateTime, Utc};
use claim_domain::{distinct_references, Cid, ClaimReference, Did, KeyId, ReferenceType};
use duckdb::Connection;
use ports::{
    AuthorRelationship, IndexReadPort, IndexStoreError, IndexStorePort, IndexedClaim,
    PeerClaimProvenance, ProbeOutcome, ProbeRefusalReason,
};

mod schema;

/// Embedded-DuckDB `IndexStorePort` adapter over the SEPARATE `index.duckdb`
/// (ADR-023/025).
///
/// Holds the open DB handle (behind an `Arc<Mutex<_>>` because DuckDB's
/// `Connection` is `!Sync`) + the path to the colocated `indexed_claims/`
/// artifact directory (`<index dir>/indexed_claims/<author_did>/<cid>.json`).
///
/// ONE instance per process (ADR-080, B1): the composition root opens the file
/// once and shares that handle (`Arc`) between the probe, the search reads and
/// the writes; it never opens the same `index.duckdb` a second time.
pub struct IndexStoreAdapter {
    conn: Arc<Mutex<Connection>>,
    /// `<index dir>/indexed_claims/` — per-author artifact partition root.
    artifacts_root: PathBuf,
}

impl IndexStoreAdapter {
    /// Open the index store at `db_path`; run pending migrations; prepare the
    /// colocated `indexed_claims/` artifact directory. Idempotent across reopens.
    pub fn open(db_path: &Path) -> Result<Self, IndexStoreError> {
        if let Some(parent) = db_path.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent).map_err(|err| {
                    IndexStoreError::SchemaMigrationFailed {
                        message: format!("create index db parent dir: {err}"),
                    }
                })?;
            }
        }

        let mut conn =
            Connection::open(db_path).map_err(|err| IndexStoreError::SchemaMigrationFailed {
                message: format!("open index.duckdb at {}: {err}", db_path.display()),
            })?;

        schema::run_migrations(&mut conn)?;

        // Colocate `indexed_claims/` next to the DB file (data-models.md
        // §"On-disk artifact format"): `<index dir>/indexed_claims/<did>/<cid>.json`.
        let artifacts_root = db_path
            .parent()
            .map(|p| p.join("indexed_claims"))
            .unwrap_or_else(|| PathBuf::from("indexed_claims"));
        fs::create_dir_all(&artifacts_root).map_err(|err| {
            IndexStoreError::SchemaMigrationFailed {
                message: format!(
                    "create indexed_claims dir {}: {err}",
                    artifacts_root.display()
                ),
            }
        })?;

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
            artifacts_root,
        })
    }

    /// The colocated artifact path for one indexed claim:
    /// `indexed_claims/<did_to_fs_segment>/<cid>.json` (RELATIVE to the index
    /// dir; stored verbatim in `signed_record_path`). Mirrors the slice-03
    /// `peer_claims/<did>/<cid>.json` partition.
    fn artifact_rel_path(claim: &IndexedClaim) -> String {
        format!(
            "indexed_claims/{}/{}.json",
            did_to_fs_segment(&claim.author_did.0),
            claim.cid.0
        )
    }

    /// Write the verified claim's JSON artifact under the per-author partition
    /// (`<cid>.json.tmp` → fsync → rename; the slice-01 POSIX-atomic pattern).
    fn write_artifact(&self, claim: &IndexedClaim) -> Result<(), IndexStoreError> {
        let dir = self
            .artifacts_root
            .join(did_to_fs_segment(&claim.author_did.0));
        fs::create_dir_all(&dir).map_err(|err| IndexStoreError::WriteFailed {
            cid: claim.cid.clone(),
            message: format!("create artifact dir {}: {err}", dir.display()),
        })?;

        let payload = serde_json::to_vec_pretty(&artifact_json(claim)).map_err(|err| {
            IndexStoreError::WriteFailed {
                cid: claim.cid.clone(),
                message: format!("serialize artifact: {err}"),
            }
        })?;

        let final_path = dir.join(format!("{}.json", claim.cid.0));
        let tmp_path = dir.join(format!("{}.json.tmp", claim.cid.0));
        write_atomic(&tmp_path, &final_path, &payload).map_err(|err| IndexStoreError::WriteFailed {
            cid: claim.cid.clone(),
            message: err,
        })
    }

    /// Lock the shared connection, mapping a poisoned mutex to a query error.
    fn lock(&self) -> Result<std::sync::MutexGuard<'_, Connection>, IndexStoreError> {
        self.conn.lock().map_err(|_| IndexStoreError::QueryFailed {
            message: "index connection mutex poisoned".to_string(),
        })
    }

    /// Run a per-claim attributed SELECT with an EXPLICIT `author_did` projection
    /// (anti-merging; NEVER an author-eliding aggregate) bound by one parameter.
    ///
    /// Each returned row carries its typed `references` populated from the
    /// `indexed_claim_references` child table (a SAME-store lookup keyed by the
    /// row's own `referencing_cid`) so the PURE `appview_domain::compose_results`
    /// counter/retract annotation (OD-AV-7 / AVC-6) can fire. This is NOT a
    /// cross-store join and it NEVER elides `author_did`: each row keeps its own
    /// attribution; the reference rows only carry `(referenced_cid, ref_type)`.
    fn select_rows(
        &self,
        where_clause: &str,
        bind: &str,
    ) -> Result<Vec<IndexedClaim>, IndexStoreError> {
        let conn = self.lock()?;
        let sql = format!(
            "SELECT author_did, cid, subject, predicate, object, confidence, \
                    composed_at, verified_against, COALESCE(provenance, 'app-signed') \
             FROM indexed_claims WHERE {where_clause}"
        );
        let mut stmt = conn
            .prepare(&sql)
            .map_err(|err| IndexStoreError::QueryFailed {
                message: format!("prepare index query: {err}"),
            })?;
        let rows = stmt
            .query_map(duckdb::params![bind], row_to_indexed_claim)
            .map_err(|err| IndexStoreError::QueryFailed {
                message: format!("run index query: {err}"),
            })?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row.map_err(|err| IndexStoreError::QueryFailed {
                message: format!("decode index row: {err}"),
            })?);
        }
        // Populate each row's typed references (OD-AV-7 same-store lookup). Done in
        // a second pass so the base SELECT's borrow of `stmt` is released first.
        for claim in &mut out {
            claim.references = references_for(&conn, &claim.cid)?;
        }
        Ok(out)
    }
}

/// The typed `references` a claim carries, read from the `indexed_claim_references`
/// child table by the claim's OWN `referencing_cid` (a SAME-store lookup — NOT a
/// cross-store join). Each reference is `ClaimReference { ref_type, cid:
/// referenced_cid }`; the countering claim K's row therefore carries a `Counters`
/// reference to the countered claim C's CID, which the pure compose core reads to
/// annotate C (OD-AV-7 / AVC-6). Anti-merging is preserved: this lookup touches
/// only the reference rows; the claim's `author_did` is untouched.
fn references_for(conn: &Connection, cid: &Cid) -> Result<Vec<ClaimReference>, IndexStoreError> {
    let mut stmt = conn
        .prepare(
            "SELECT referenced_cid, ref_type FROM indexed_claim_references \
             WHERE referencing_cid = ? ORDER BY referenced_cid, ref_type",
        )
        .map_err(|err| IndexStoreError::QueryFailed {
            message: format!("prepare references query: {err}"),
        })?;
    let rows = stmt
        .query_map(duckdb::params![cid.0], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|err| IndexStoreError::QueryFailed {
            message: format!("run references query: {err}"),
        })?;
    let mut out = Vec::new();
    for row in rows {
        let (referenced_cid, ref_type) = row.map_err(|err| IndexStoreError::QueryFailed {
            message: format!("decode reference row: {err}"),
        })?;
        if let Some(ref_type) = reference_type_from_wire(&ref_type) {
            out.push(ClaimReference {
                ref_type,
                cid: Cid(referenced_cid),
            });
        }
    }
    Ok(out)
}

/// Parse a `ref_type` wire token (the `indexed_claim_references` CHECK domain) back
/// into a [`ReferenceType`]. The inverse of [`reference_type_wire`]; an unknown
/// token (impossible under the CHECK constraint) is skipped, not panicked.
fn reference_type_from_wire(token: &str) -> Option<ReferenceType> {
    match token {
        "retracts" => Some(ReferenceType::Retracts),
        "corrects" => Some(ReferenceType::Corrects),
        "counters" => Some(ReferenceType::Counters),
        "supersedes" => Some(ReferenceType::Supersedes),
        _ => None,
    }
}

impl IndexStorePort for IndexStoreAdapter {
    fn probe(&self) -> ProbeOutcome {
        // Earned-Trust probe (ADR-009 / ADR-025): two arms run BEFORE the gauntlet
        // trusts this store. (1) the schema version must match what this binary
        // knows how to read; (2) the substrate must HONOR fsync — a tmpfs/
        // overlayfs/DrvFs durability no-op is a substrate LIE the indexer must
        // refuse (DESIGN §6.3, AV-6). Mirrors the slice-01 `adapter-duckdb` probe.
        let conn = match self.conn.lock() {
            Ok(c) => c,
            Err(_) => {
                return ProbeOutcome::Refused {
                    reason: ProbeRefusalReason::StorageFsyncUnreliable,
                    detail: "index connection mutex poisoned".to_string(),
                    structured: serde_json::json!({"adapter": "index_store"}),
                };
            }
        };
        // Arm 1: schema version.
        match schema::read_version(&conn) {
            Ok(v) if v == schema::LATEST_VERSION => {}
            Ok(v) => {
                return ProbeOutcome::Refused {
                    reason: ProbeRefusalReason::StorageSchemaMismatch,
                    detail: format!(
                        "index.duckdb schema version {v} != expected {}",
                        schema::LATEST_VERSION
                    ),
                    structured: serde_json::json!({
                        "adapter": "index_store",
                        "found": v,
                        "expected": schema::LATEST_VERSION,
                    }),
                };
            }
            Err(err) => {
                return ProbeOutcome::Refused {
                    reason: ProbeRefusalReason::StorageSchemaMismatch,
                    detail: format!("could not read index schema version: {err}"),
                    structured: serde_json::json!({"adapter": "index_store"}),
                };
            }
        }
        drop(conn);

        // Arm 2: fsync honored on the durability medium (the substrate-lie check).
        // The indexer writes its `<cid>.json` artifacts under `artifacts_root`, so
        // that is the medium whose durability the probe must trust.
        probe_fsync_honored(
            &self.artifacts_root,
            detect_fsync_honesty(&self.artifacts_root),
        )
    }

    fn upsert(&self, claim: &IndexedClaim) -> Result<(), IndexStoreError> {
        // Write the JSON artifact first (the authoritative on-disk record), then
        // index the row. De-dup by CID only (ADR-025): an INSERT OR REPLACE on
        // the CID PK leaves exactly one row per CID.
        self.write_artifact(claim)?;
        let signed_record_path = Self::artifact_rel_path(claim);

        // One claim's rows are replaced in ONE transaction (fix-indexer-follow-ups
        // D3): any failing statement rolls back the whole upsert, so the prior
        // version stays whole. No cross-claim transaction: earlier claims of the
        // pass stay committed (ADR-078 §2). The artifact above cannot join it; a
        // stray artifact for a rolled-back CID is overwritten on retry.
        let mut conn = self.lock()?;
        let transaction = conn
            .transaction()
            .map_err(|err| write_failed(claim, format!("begin transaction: {err}")))?;
        replace_claim_rows(&transaction, claim, &signed_record_path)?;
        transaction
            .commit()
            .map_err(|err| write_failed(claim, format!("commit: {err}")))
    }
}

/// The read side (B7): every method only SELECTs, so the whole store is left
/// exactly as it was found (unbounded-preservation).
impl IndexReadPort for IndexStoreAdapter {
    fn query_by_object(&self, object: &str) -> Result<Vec<IndexedClaim>, IndexStoreError> {
        self.select_rows("object = ?", object)
    }

    /// A contributor is matched on the BARE DID (ADR-079 / AC-005.3): rows stored
    /// under the bare DID (self-attested) and under any `bare#fragment` key
    /// (app-signed) both belong to them. `starts_with`, not `LIKE`, so `%` / `_`
    /// in a DID are never wildcards.
    fn query_by_contributor(&self, did: &Did) -> Result<Vec<IndexedClaim>, IndexStoreError> {
        self.select_rows(
            "author_did = $1 OR starts_with(author_did, $1 || '#')",
            bare_did(&did.0),
        )
    }

    fn query_by_subject(&self, subject: &str) -> Result<Vec<IndexedClaim>, IndexStoreError> {
        self.select_rows("subject = ?", subject)
    }

    fn get_by_cid(&self, cid: &Cid) -> Result<Option<IndexedClaim>, IndexStoreError> {
        Ok(self.select_rows("cid = ?", &cid.0)?.into_iter().next())
    }
}

/// Replace `claim`'s row and its evidence/reference children idempotently
/// (de-dup by CID PK). The row is updated in place, never deleted: DuckDB's
/// foreign-key check still sees children deleted earlier in the same
/// transaction, and it rewrites an update of an INDEXED column as delete +
/// insert. The indexed columns (author, subject, object, composed_at) are part
/// of the content the CID addresses, so for one CID they never change. Each distinct reference is inserted once: a remote
/// record may list one twice, and the references PK forbids a repeat.
fn replace_claim_rows(
    conn: &Connection,
    claim: &IndexedClaim,
    signed_record_path: &str,
) -> Result<(), IndexStoreError> {
    let execute = |sql: &str, params: &[&dyn duckdb::ToSql], what: &str| {
        conn.execute(sql, params)
            .map(drop)
            .map_err(|err| write_failed(claim, format!("{what}: {err}")))
    };
    execute(
        "DELETE FROM indexed_claim_evidence WHERE cid = ?",
        duckdb::params![claim.cid.0],
        "clear evidence",
    )?;
    execute(
        "DELETE FROM indexed_claim_references WHERE referencing_cid = ?",
        duckdb::params![claim.cid.0],
        "clear references",
    )?;
    execute(
        "INSERT INTO indexed_claims (\
            cid, author_did, subject, predicate, object, confidence, \
            composed_at, indexed_at, source_pds, signed_record_path, verified_against, \
            provenance\
         ) VALUES (?, ?, ?, ?, ?, ?, ?, now(), ?, ?, ?, ?) \
         ON CONFLICT (cid) DO UPDATE SET \
            predicate = excluded.predicate, confidence = excluded.confidence, \
            indexed_at = excluded.indexed_at, source_pds = excluded.source_pds, \
            signed_record_path = excluded.signed_record_path, \
            verified_against = excluded.verified_against, provenance = excluded.provenance",
        duckdb::params![
            claim.cid.0,
            claim.author_did.0,
            claim.subject,
            claim.predicate,
            claim.object,
            claim.confidence,
            claim.composed_at,
            // source_pds is pull provenance; the IndexedClaim does not carry
            // it (it is on RawRecord). The verified marker stands as proof;
            // record an empty-safe provenance sentinel here for the NOT NULL.
            "network",
            signed_record_path,
            claim.verified_against.0,
            provenance_column(claim.provenance),
        ],
        "upsert row",
    )?;
    for (ordinal, evidence) in claim.evidence.iter().enumerate() {
        execute(
            "INSERT INTO indexed_claim_evidence (cid, evidence, ordinal) VALUES (?, ?, ?)",
            duckdb::params![claim.cid.0, evidence, ordinal as i32],
            "insert evidence",
        )?;
    }
    for reference in distinct_references(&claim.references) {
        execute(
            "INSERT INTO indexed_claim_references \
                (referencing_cid, referenced_cid, ref_type) VALUES (?, ?, ?)",
            duckdb::params![
                claim.cid.0,
                reference.cid.0,
                reference_type_wire(reference.ref_type)
            ],
            "insert reference",
        )?;
    }
    Ok(())
}

// -----------------------------------------------------------------------------
// Pure helpers (free functions; no I/O except where wired through the adapter)
// -----------------------------------------------------------------------------

/// The bare DID of an author key id: the text before the first `#`
/// (`did:plc:x#org.openlore.application` ⇒ `did:plc:x`; a bare DID is itself).
fn bare_did(author_did: &str) -> &str {
    author_did.split('#').next().unwrap_or(author_did)
}

/// Map one `indexed_claims` row (the SAFE attributed projection) into an
/// `IndexedClaim`. `author_did` is NON-`Option` (the column is `NOT NULL`).
/// `evidence`/`references` are not rehydrated for the walking-skeleton search
/// path (the artifact carries them); they are empty here and filled by the
/// broader query surface in later steps.
fn row_to_indexed_claim(row: &duckdb::Row<'_>) -> duckdb::Result<IndexedClaim> {
    Ok(IndexedClaim {
        author_did: Did(row.get::<_, String>(0)?),
        cid: Cid(row.get::<_, String>(1)?),
        subject: row.get::<_, String>(2)?,
        predicate: row.get::<_, String>(3)?,
        object: row.get::<_, String>(4)?,
        confidence: row.get::<_, f64>(5)?,
        composed_at: row.get::<_, DateTime<Utc>>(6)?,
        verified_against: KeyId(row.get::<_, String>(7)?),
        evidence: Vec::new(),
        references: Vec::new(),
        relationship: AuthorRelationship::NetworkUnfollowed,
        provenance: PeerClaimProvenance::from_column(&row.get::<_, String>(8)?),
    })
}

/// The `provenance` column value (the same domain as `peer_claims.provenance`).
fn provenance_column(provenance: PeerClaimProvenance) -> &'static str {
    match provenance {
        PeerClaimProvenance::AppSigned => "app-signed",
        PeerClaimProvenance::SelfAttested => "self-attested",
    }
}

/// The JSON artifact body for an indexed claim (the verified network record, as
/// stored at `indexed_claims/<did>/<cid>.json`).
fn artifact_json(claim: &IndexedClaim) -> serde_json::Value {
    let mut artifact = serde_json::json!({
        "cid": claim.cid.0,
        "author_did": claim.author_did.0,
        "subject": claim.subject,
        "predicate": claim.predicate,
        "object": claim.object,
        "confidence": claim.confidence,
        "composedAt": claim.composed_at.to_rfc3339(),
        "verified_against": claim.verified_against.0,
        "evidence": claim.evidence,
        "references": claim
            .references
            .iter()
            .map(reference_json)
            .collect::<Vec<_>>(),
    });
    // App-signed artifacts stay byte-identical; only a self-attested one says so.
    if claim.provenance == PeerClaimProvenance::SelfAttested {
        artifact["provenance"] = serde_json::json!(provenance_column(claim.provenance));
    }
    artifact
}

fn reference_json(reference: &ClaimReference) -> serde_json::Value {
    serde_json::json!({
        "type": reference_type_wire(reference.ref_type),
        "cid": reference.cid.0,
    })
}

/// The lexicon wire token for a reference type (the `ref_type` CHECK domain).
fn reference_type_wire(ref_type: ReferenceType) -> &'static str {
    match ref_type {
        ReferenceType::Retracts => "retracts",
        ReferenceType::Corrects => "corrects",
        ReferenceType::Counters => "counters",
        ReferenceType::Supersedes => "supersedes",
    }
}

/// DID → filesystem-safe partition segment (colons → underscores). The single
/// source of truth the acceptance harness + this adapter agree on (mirrors the
/// slice-03 `peer_claims/<encoded_did>/` encoding).
fn did_to_fs_segment(did: &str) -> String {
    did.replace(':', "_")
}

/// Build a `WriteFailed` error for `claim` with `message`.
fn write_failed(claim: &IndexedClaim, message: String) -> IndexStoreError {
    IndexStoreError::WriteFailed {
        cid: claim.cid.clone(),
        message,
    }
}

/// The substrate-durability verdict the fsync-honesty probe acts on: does the
/// medium HONOR fsync, or is it a silent no-op (tmpfs/overlayfs/DrvFs)?
///
/// Extracted as an explicit ADT so the load-bearing refusal decision is a PURE
/// function of the verdict — unit-testable without process-global env state
/// (mirrors the slice-01 probe's railway shape, made data-explicit).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FsyncHonesty {
    /// The medium persists fsynced writes durably (a real, durable filesystem).
    Honored,
    /// The medium SILENTLY no-ops fsync (the container substrate lie, DESIGN §6.3)
    /// — a tmpfs/overlayfs/WSL2-DrvFs durability no-op. The indexer must refuse.
    SilentNoop,
}

/// Detect whether `dir`'s substrate honors fsync.
///
/// ## Limitation (mirrors the slice-01 `adapter-duckdb` probe inline note)
///
/// Detecting that the kernel SILENTLY no-ops fsync on tmpfs/overlayfs/WSL2-DrvFs
/// requires platform-specific kernel cooperation we don't have at the userspace
/// boundary. The pragmatic check is the fsync round-trip in [`probe_fsync_honored`]
/// (the file persists across an explicit `sync_all`); BEYOND that, the
/// `OPENLORE_INDEXER_FORCE_FSYNC_NOOP` seam lets an operator (and the AV-6
/// acceptance harness) assert the no-op verdict the probe would reach on a known
/// lying substrate, so the refusal path is exercised deterministically across CI
/// + macOS + Linux where a real tmpfs mount is not portable.
fn detect_fsync_honesty(_dir: &Path) -> FsyncHonesty {
    if std::env::var_os("OPENLORE_INDEXER_FORCE_FSYNC_NOOP").is_some() {
        return FsyncHonesty::SilentNoop;
    }
    FsyncHonesty::Honored
}

/// The fsync-honesty probe arm: do a REAL fsync round-trip on `dir` (write a
/// sentinel, `sync_all`, sync the dir handle, re-read) AND act on the substrate
/// `honesty` verdict. Refuses with `StorageFsyncUnreliable` + the
/// `storage.fsync_unhonored` structured event on a `SilentNoop` lie or any
/// round-trip failure; returns `Ok` only when the medium is durable.
///
/// The decision is a pure function of (the round-trip result, `honesty`) so AV-6's
/// load-bearing refusal is unit-testable by passing `FsyncHonesty::SilentNoop`.
fn probe_fsync_honored(dir: &Path, honesty: FsyncHonesty) -> ProbeOutcome {
    use std::io::{Read, Write};

    let refuse = |detail: String| ProbeOutcome::Refused {
        reason: ProbeRefusalReason::StorageFsyncUnreliable,
        detail,
        structured: serde_json::json!({
            "event": "storage.fsync_unhonored",
            "adapter": "index_store",
            "path": dir.display().to_string(),
        }),
    };

    // The substrate-lie verdict short-circuits: a fsync no-op medium is unsafe to
    // index into regardless of whether THIS round-trip happened to persist.
    if honesty == FsyncHonesty::SilentNoop {
        return refuse(format!(
            "index store substrate at {} silently no-ops fsync (tmpfs/overlayfs/DrvFs \
             durability lie) — refusing to index into a store that cannot persist \
             (DESIGN §6.3)",
            dir.display()
        ));
    }

    // The REAL round-trip: write → sync_all → re-read → byte-equal. Catches gross
    // medium failures the verdict cannot (permissions, wrong mount, corruption).
    if let Err(err) = fs::create_dir_all(dir) {
        return refuse(format!("could not create index artifacts dir: {err}"));
    }
    let path = dir.join(".index-probe-fsync");
    let payload = b"openlore-index-fsync-probe-v1";
    {
        let mut file = match fs::File::create(&path) {
            Ok(f) => f,
            Err(err) => return refuse(format!("could not create fsync sentinel: {err}")),
        };
        if let Err(err) = file.write_all(payload) {
            return refuse(format!("could not write fsync sentinel: {err}"));
        }
        if let Err(err) = file.sync_all() {
            return refuse(format!("sync_all on index fsync sentinel failed: {err}"));
        }
    }
    if let Ok(dir_handle) = fs::File::open(dir) {
        let _ = dir_handle.sync_all();
    }
    let mut observed = Vec::new();
    let reread = fs::File::open(&path).and_then(|mut f| f.read_to_end(&mut observed));
    if let Err(err) = reread {
        return refuse(format!("could not re-read fsync sentinel: {err}"));
    }
    if observed != payload {
        return refuse("index fsync sentinel round-trip mismatch".to_string());
    }
    let _ = fs::remove_file(&path);
    ProbeOutcome::Ok
}

/// Write `payload` to `final_path` atomically: write to `tmp_path`, fsync, then
/// rename over the destination (the slice-01 POSIX-atomic artifact pattern).
fn write_atomic(tmp_path: &Path, final_path: &Path, payload: &[u8]) -> Result<(), String> {
    use std::io::Write;
    let mut file = fs::File::create(tmp_path)
        .map_err(|err| format!("create {}: {err}", tmp_path.display()))?;
    file.write_all(payload)
        .map_err(|err| format!("write {}: {err}", tmp_path.display()))?;
    file.sync_all()
        .map_err(|err| format!("fsync {}: {err}", tmp_path.display()))?;
    fs::rename(tmp_path, final_path).map_err(|err| {
        format!(
            "rename {} -> {}: {err}",
            tmp_path.display(),
            final_path.display()
        )
    })
}

#[cfg(test)]
mod tests {
    //! DELIVER inner loop (step 03-01): the load-bearing index-store round-trip —
    //! `upsert` then `query_by_object`/`get_by_cid` returns the verified attributed
    //! row with its NON-`Option` `author_did` + non-empty `verified_against`
    //! preserved (the walking-skeleton beat-1 store contract). The adapter IS the
    //! I/O boundary, so this exercises a REAL `index.duckdb` on `tmp_path` (Mandate
    //! 6: adapter integration tests are real I/O — no mock substitutes for DuckDB).

    use super::*;
    use chrono::{DateTime, Utc};
    use claim_domain::{Cid, Did, KeyId};
    use ports::AuthorRelationship;

    /// Build a verified, attributed `IndexedClaim` for the round-trip test.
    pub(super) fn sample_claim() -> IndexedClaim {
        IndexedClaim {
            author_did: Did("did:plc:priya-test#org.openlore.application".to_string()),
            cid: Cid("bafytestpriyaclaim001".to_string()),
            subject: "github:bazelbuild/bazel".to_string(),
            predicate: "embodiesPhilosophy".to_string(),
            object: "org.openlore.philosophy.reproducible-builds".to_string(),
            confidence: 0.82,
            composed_at: "2026-05-26T12:00:00Z"
                .parse::<DateTime<Utc>>()
                .expect("fixed RFC3339 timestamp parses"),
            verified_against: KeyId("did:plc:priya-test#org.openlore.application".to_string()),
            evidence: vec!["https://example.test/evidence/bazel".to_string()],
            references: Vec::new(),
            relationship: AuthorRelationship::NetworkUnfollowed,
            provenance: PeerClaimProvenance::AppSigned,
        }
    }

    /// The load-bearing store contract: a row written via `upsert` reads back via
    /// `query_by_object` with its attribution (`author_did`) + verified marker
    /// (`verified_against`) byte-equal. This is the inner-loop decomposition of
    /// AV-1's "the index exists + is trustworthy + is searchable" proof.
    #[test]
    fn upsert_then_query_by_object_roundtrips_attributed_row() {
        let dir = tempfile::tempdir().expect("tempdir");
        let db_path = dir.path().join("index.duckdb");
        let store = IndexStoreAdapter::open(&db_path).expect("open index store");

        let claim = sample_claim();
        store.upsert(&claim).expect("upsert verified claim");

        let rows = store
            .query_by_object("org.openlore.philosophy.reproducible-builds")
            .expect("query by object");
        assert_eq!(rows.len(), 1, "exactly one indexed row for the object");
        let read = &rows[0];
        assert_eq!(
            read.author_did, claim.author_did,
            "author_did must round-trip byte-equal (anti-merging attribution, WD-103)"
        );
        assert_eq!(read.cid, claim.cid, "cid PK must round-trip");
        assert_eq!(read.subject, claim.subject);
        assert_eq!(read.object, claim.object);
        assert!(
            !read.verified_against.0.is_empty(),
            "verified_against must never be empty (WD-104)"
        );
        assert_eq!(read.verified_against, claim.verified_against);
    }

    /// `get_by_cid` returns the single attributed row by its CID PK (the `--show`
    /// lookup), and `None` for an absent CID.
    #[test]
    fn get_by_cid_returns_attributed_row_or_none() {
        let dir = tempfile::tempdir().expect("tempdir");
        let db_path = dir.path().join("index.duckdb");
        let store = IndexStoreAdapter::open(&db_path).expect("open index store");

        let claim = sample_claim();
        store.upsert(&claim).expect("upsert");

        let found = store.get_by_cid(&claim.cid).expect("get_by_cid");
        assert_eq!(
            found.as_ref().map(|c| c.author_did.clone()),
            Some(claim.author_did.clone()),
            "get_by_cid returns the attributed row"
        );

        let absent = store
            .get_by_cid(&Cid("bafy-does-not-exist".to_string()))
            .expect("get_by_cid absent");
        assert!(absent.is_none(), "absent CID returns None");
    }

    /// Anti-merging at the store boundary (WD-103 / I-AV-2; the inner-loop
    /// decomposition of AV-2): two DISTINCT-author claims on the SAME
    /// (subject,object) but with distinct CIDs stay TWO individually-attributed
    /// rows — `query_by_object` returns both, attributed to {priya, sven}, and
    /// there is no cross-author collapse. De-dup is by CID only (ADR-025), so a
    /// merge would require collapsing on (subject,object) — which the store MUST
    /// NOT do.
    #[test]
    fn upsert_two_distinct_authors_same_subject_object_stays_two_attributed_rows() {
        let dir = tempfile::tempdir().expect("tempdir");
        let db_path = dir.path().join("index.duckdb");
        let store = IndexStoreAdapter::open(&db_path).expect("open index store");

        let object = "org.openlore.philosophy.dependency-pinning";
        let subject = "github:denoland/deno";
        let priya = IndexedClaim {
            author_did: Did("did:plc:priya-test#org.openlore.application".to_string()),
            cid: Cid("bafytestpriyadeno".to_string()),
            subject: subject.to_string(),
            object: object.to_string(),
            confidence: 0.70,
            verified_against: KeyId("did:plc:priya-test#org.openlore.application".to_string()),
            ..sample_claim()
        };
        let sven = IndexedClaim {
            author_did: Did("did:plc:sven-test#org.openlore.application".to_string()),
            cid: Cid("bafytestsvendeno".to_string()),
            subject: subject.to_string(),
            object: object.to_string(),
            confidence: 0.65,
            verified_against: KeyId("did:plc:sven-test#org.openlore.application".to_string()),
            ..sample_claim()
        };

        store.upsert(&priya).expect("upsert priya");
        store.upsert(&sven).expect("upsert sven");

        let rows = store.query_by_object(object).expect("query by object");
        assert_eq!(
            rows.len(),
            2,
            "two distinct-author claims on the same (subject,object) must stay TWO rows"
        );
        let mut authors: Vec<String> = rows.iter().map(|r| r.author_did.0.clone()).collect();
        authors.sort();
        assert_eq!(
            authors,
            vec![
                "did:plc:priya-test#org.openlore.application".to_string(),
                "did:plc:sven-test#org.openlore.application".to_string(),
            ],
            "both authors must be individually attributed — never merged onto one row"
        );
    }

    /// Earned-Trust fsync-honesty probe — the honored-substrate arm (the
    /// inner-loop decomposition of AV-6's "wire → probe → use" refusal gate). On
    /// a real, durable tmp filesystem the fsync round-trip succeeds, so the probe
    /// must return `Ok` (the gauntlet proceeds to ingest/serve).
    #[test]
    fn probe_returns_ok_on_a_durable_substrate() {
        let dir = tempfile::tempdir().expect("tempdir");
        let db_path = dir.path().join("index.duckdb");
        let store = IndexStoreAdapter::open(&db_path).expect("open index store");

        match store.probe() {
            ProbeOutcome::Ok => {}
            other => {
                panic!("a durable substrate with a current schema must probe Ok; got {other:?}")
            }
        }
    }

    /// Earned-Trust fsync-honesty probe — the substrate-LIE arm (the load-bearing
    /// AV-6 decomposition). A tmpfs/overlayfs/DrvFs fsync no-op (forced via the
    /// `OPENLORE_INDEXER_FORCE_FSYNC_NOOP` seam — the same pragmatic limitation
    /// the slice-01 duckdb probe documents) MUST refuse with
    /// `StorageFsyncUnreliable` and the `storage.fsync_unhonored` structured event
    /// the DevOps `health.startup.refused` layer routes on. The probe REFUSES so
    /// the indexer never indexes into a store it cannot trust to persist.
    #[test]
    fn probe_refuses_when_fsync_is_a_silent_noop() {
        let dir = tempfile::tempdir().expect("tempdir");

        // The load-bearing decision is a PURE function of the substrate verdict:
        // the fsync-honesty arm refuses when the medium's fsync is a no-op. Tested
        // at the pure-function boundary (no process-global env, parallel-safe). The
        // env-seam → adapter-`probe()` → gauntlet → exit-2 wiring is proven
        // end-to-end by AV-6 (`indexer_refuses_to_start_when_a_driven_adapter_probe_fails`).
        let outcome = probe_fsync_honored(dir.path(), FsyncHonesty::SilentNoop);
        match outcome {
            ProbeOutcome::Refused {
                reason, structured, ..
            } => {
                assert_eq!(
                    reason,
                    ProbeRefusalReason::StorageFsyncUnreliable,
                    "a fsync no-op must refuse with the storage-durability reason"
                );
                assert_eq!(
                    structured["event"], "storage.fsync_unhonored",
                    "the structured payload must carry the storage.fsync_unhonored event"
                );
            }
            ProbeOutcome::Ok => panic!("a fsync no-op substrate must REFUSE, not Ok"),
        }
    }

    /// OD-AV-7 (AV-25 decomposition): the query path POPULATES each row's typed
    /// `references` from the `indexed_claim_references` child table (the same-store
    /// lookup the pure counter annotation reads). A countering claim K upserted with
    /// a `Counters` reference to claim C's CID reads back via `query_by_object` with
    /// that reference INTACT — so `appview_domain::compose_results` can annotate C.
    /// The countered claim C carries NO references (it counters nothing). Anti-
    /// merging is preserved: each row keeps its own `author_did`.
    #[test]
    fn query_populates_typed_references_for_the_counter_annotation() {
        use claim_domain::{ClaimReference, ReferenceType};

        let dir = tempfile::tempdir().expect("tempdir");
        let db_path = dir.path().join("index.duckdb");
        let store = IndexStoreAdapter::open(&db_path).expect("open index store");

        // C — the countered claim (Priya), no references of its own.
        let c = IndexedClaim {
            author_did: Did("did:plc:priya-test#org.openlore.application".to_string()),
            cid: Cid("bafyclaimc".to_string()),
            ..sample_claim()
        };
        // K — the countering claim (Sven), same object so it co-appears, carrying a
        // typed Counters reference to C's CID.
        let k = IndexedClaim {
            author_did: Did("did:plc:sven-test#org.openlore.application".to_string()),
            cid: Cid("bafyclaimk".to_string()),
            verified_against: KeyId("did:plc:sven-test#org.openlore.application".to_string()),
            references: vec![ClaimReference {
                ref_type: ReferenceType::Counters,
                cid: Cid("bafyclaimc".to_string()),
            }],
            ..sample_claim()
        };

        store.upsert(&c).expect("upsert C");
        store.upsert(&k).expect("upsert K");

        let rows = store
            .query_by_object("org.openlore.philosophy.reproducible-builds")
            .expect("query by object");
        assert_eq!(
            rows.len(),
            2,
            "both C and K are returned (counter never drops a row)"
        );

        let k_row = rows
            .iter()
            .find(|r| r.cid.0 == "bafyclaimk")
            .expect("K is in the result");
        assert_eq!(
            k_row.references,
            vec![ClaimReference {
                ref_type: ReferenceType::Counters,
                cid: Cid("bafyclaimc".to_string()),
            }],
            "K's typed Counters reference to C must be populated from the same-store \
             child table (the OD-AV-7 annotation input)"
        );

        let c_row = rows
            .iter()
            .find(|r| r.cid.0 == "bafyclaimc")
            .expect("C is in the result");
        assert!(
            c_row.references.is_empty(),
            "C counters nothing — it carries no references of its own"
        );
        // Anti-merging: each row keeps its OWN author_did.
        assert_eq!(
            c_row.author_did.0,
            "did:plc:priya-test#org.openlore.application"
        );
        assert_eq!(
            k_row.author_did.0,
            "did:plc:sven-test#org.openlore.application"
        );
    }

    /// De-dup at upsert is by CID only (ADR-025): upserting the same CID twice
    /// leaves exactly one row.
    #[test]
    fn upsert_is_idempotent_by_cid() {
        let dir = tempfile::tempdir().expect("tempdir");
        let db_path = dir.path().join("index.duckdb");
        let store = IndexStoreAdapter::open(&db_path).expect("open index store");

        let claim = sample_claim();
        store.upsert(&claim).expect("first upsert");
        store.upsert(&claim).expect("second upsert (same CID)");

        let rows = store
            .query_by_object("org.openlore.philosophy.reproducible-builds")
            .expect("query by object");
        assert_eq!(rows.len(), 1, "de-dup by CID: a re-upsert leaves one row");
    }

    /// Each provenance reads back as written; an app-signed row is unchanged.
    // bypass: real-DuckDB integration over the two-value provenance domain.
    #[test]
    fn provenance_reads_back_as_written() {
        let dir = tempfile::tempdir().expect("tempdir");
        let store = IndexStoreAdapter::open(&dir.path().join("index.duckdb")).expect("open");
        for (cid, provenance) in [
            ("bafyappsigned", PeerClaimProvenance::AppSigned),
            ("bafyselfattested", PeerClaimProvenance::SelfAttested),
        ] {
            let claim = IndexedClaim {
                cid: Cid(cid.to_string()),
                provenance,
                ..sample_claim()
            };
            store.upsert(&claim).expect("upsert");
            let read = store
                .get_by_cid(&claim.cid)
                .expect("read")
                .expect("row present");
            assert_eq!(read.provenance, provenance);
            assert_eq!(read.author_did, claim.author_did);
            assert_eq!(read.verified_against, claim.verified_against);
        }
    }

    /// A v1 index (no provenance column) migrates additively: its rows read as
    /// app-signed, and reopening is a no-op.
    // bypass: real-DuckDB migration over one pre-existing v1 file.
    #[test]
    fn a_v1_index_gains_provenance_additively_and_idempotently() {
        let dir = tempfile::tempdir().expect("tempdir");
        let db_path = dir.path().join("index.duckdb");
        {
            let conn = Connection::open(&db_path).expect("open raw v1");
            conn.execute_batch(
                "CREATE TABLE index_schema_version (version INTEGER PRIMARY KEY, \
                     applied_at TIMESTAMP NOT NULL, description VARCHAR NOT NULL);
                 INSERT INTO index_schema_version VALUES (1, now(), 'v1');
                 CREATE TABLE indexed_claims (cid VARCHAR PRIMARY KEY, author_did VARCHAR NOT NULL,
                     subject VARCHAR NOT NULL, predicate VARCHAR NOT NULL, object VARCHAR NOT NULL,
                     confidence DOUBLE NOT NULL, composed_at TIMESTAMP NOT NULL,
                     indexed_at TIMESTAMP NOT NULL, source_pds VARCHAR NOT NULL,
                     signed_record_path VARCHAR NOT NULL, verified_against VARCHAR NOT NULL);
                 CREATE TABLE indexed_claim_evidence (cid VARCHAR NOT NULL, evidence VARCHAR NOT NULL,
                     ordinal INTEGER NOT NULL);
                 CREATE TABLE indexed_claim_references (referencing_cid VARCHAR NOT NULL,
                     referenced_cid VARCHAR NOT NULL, ref_type VARCHAR NOT NULL);
                 INSERT INTO indexed_claims VALUES ('bafyv1', 'did:plc:priya-test', 's', 'p', 'o',
                     0.5, TIMESTAMP '2026-05-26 12:00:00', now(), 'network', 'x.json', 'did:plc:priya-test');",
            )
            .expect("seed v1 index");
        }
        for _reopen in 0..2 {
            let store = IndexStoreAdapter::open(&db_path).expect("open migrates");
            let row = store
                .get_by_cid(&Cid("bafyv1".to_string()))
                .expect("read")
                .expect("v1 row survives");
            assert_eq!(row.provenance, PeerClaimProvenance::AppSigned);
            assert_eq!(row.author_did, Did("did:plc:priya-test".to_string()));
        }
    }
}

#[cfg(test)]
mod contributor_match_properties {
    //! ADR-079 / AC-005.3: contributor search matches on the bare DID. State-delta
    //! over a declared row universe — authors drawn from a few bare DIDs (some
    //! carrying DuckDB `LIKE` metacharacters) each stored bare or with a key
    //! fragment: the match set is exactly the rows whose bare DID equals the
    //! query's bare DID, whichever form (bare or fragmented) is queried.

    use super::*;
    use proptest::prelude::*;

    const BARE_DIDS: [&str; 4] = [
        "did:plc:priya",
        "did:plc:priya_x",
        "did:plc:pri%a",
        "did:plc:priyaraman",
    ];
    const FRAGMENTS: [Option<&str>; 3] = [None, Some("org.openlore.application"), Some("k2")];

    fn author(bare: usize, fragment: usize) -> String {
        match FRAGMENTS[fragment] {
            None => BARE_DIDS[bare].to_string(),
            Some(f) => format!("{}#{f}", BARE_DIDS[bare]),
        }
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(24))]
        #[test]
        fn contributor_matches_exactly_the_rows_sharing_the_bare_did(
            rows in proptest::collection::vec((0..BARE_DIDS.len(), 0..FRAGMENTS.len()), 0..10),
            query in (0..BARE_DIDS.len(), 0..FRAGMENTS.len()),
        ) {
            let dir = tempfile::tempdir().expect("tempdir");
            let store = IndexStoreAdapter::open(&dir.path().join("index.duckdb")).expect("open");
            for (index, (bare, fragment)) in rows.iter().enumerate() {
                let did = author(*bare, *fragment);
                store
                    .upsert(&IndexedClaim {
                        author_did: Did(did.clone()),
                        verified_against: KeyId(did),
                        cid: Cid(format!("bafyrow{index}")),
                        ..super::tests::sample_claim()
                    })
                    .expect("upsert");
            }

            let found = store
                .query_by_contributor(&Did(author(query.0, query.1)))
                .expect("query");

            let mut found_cids: Vec<String> = found.into_iter().map(|c| c.cid.0).collect();
            found_cids.sort();
            let mut expected: Vec<String> = rows
                .iter()
                .enumerate()
                .filter(|(_, (bare, _))| *bare == query.0)
                .map(|(index, _)| format!("bafyrow{index}"))
                .collect();
            expected.sort();
            prop_assert_eq!(found_cids, expected);
        }
    }
}

#[cfg(test)]
mod atomic_upsert_properties {
    //! fix-indexer-follow-ups D3 @contract-shape:bounded-change. Universe = every
    //! row the store holds for the TARGET claim and for an unrelated NEIGHBOUR
    //! claim across `indexed_claims`, `indexed_claim_evidence` and
    //! `indexed_claim_references`. An upsert may change only the TARGET's rows,
    //! and it changes them all or none: a remote record listing one reference
    //! twice is indexed whole (each distinct reference once), and a statement
    //! failing inside an upsert leaves the prior version (or its absence) as it was.

    use std::collections::BTreeSet;

    use super::*;
    use proptest::prelude::*;

    const TARGET: &str = "bafytarget";
    const NEIGHBOUR: &str = "bafyneighbour";
    const REFERENCED: [&str; 3] = ["bafyref0", "bafyref1", "bafyref2"];
    const REF_TYPES: [ReferenceType; 4] = [
        ReferenceType::Retracts,
        ReferenceType::Corrects,
        ReferenceType::Counters,
        ReferenceType::Supersedes,
    ];

    /// Everything the store holds for one CID, in a comparable form.
    #[derive(Debug, Clone, PartialEq)]
    struct StoredClaim {
        row: Option<(String, String, String)>,
        evidence: Vec<(i32, String)>,
        references: Vec<(String, String)>,
    }

    fn stored(store: &IndexStoreAdapter, cid: &str) -> StoredClaim {
        let conn = store.lock().expect("lock");
        let row = conn
            .query_row(
                "SELECT subject, object, verified_against FROM indexed_claims WHERE cid = ?",
                duckdb::params![cid],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .ok();
        let evidence = collect_pairs(
            &conn,
            "SELECT ordinal, evidence FROM indexed_claim_evidence WHERE cid = ? ORDER BY ordinal",
            cid,
        );
        let references = collect_pairs(
            &conn,
            "SELECT referenced_cid, ref_type FROM indexed_claim_references \
             WHERE referencing_cid = ? ORDER BY referenced_cid, ref_type",
            cid,
        );
        StoredClaim {
            row,
            evidence,
            references,
        }
    }

    fn collect_pairs<A: duckdb::types::FromSql, B: duckdb::types::FromSql>(
        conn: &Connection,
        sql: &str,
        cid: &str,
    ) -> Vec<(A, B)> {
        let mut stmt = conn.prepare(sql).expect("prepare");
        stmt.query_map(duckdb::params![cid], |r| Ok((r.get(0)?, r.get(1)?)))
            .expect("query")
            .map(|row| row.expect("row"))
            .collect()
    }

    fn reference((cid, ref_type): (usize, usize)) -> ClaimReference {
        ClaimReference {
            ref_type: REF_TYPES[ref_type],
            cid: Cid(REFERENCED[cid].to_string()),
        }
    }

    /// One version of a claim. Versions of one CID share its content (the CID
    /// addresses it) and differ in what a later pass may legitimately change:
    /// the key it verified against, its evidence and references as listed.
    fn claim(cid: &str, version: &str, picks: &[(usize, usize)], evidence: usize) -> IndexedClaim {
        IndexedClaim {
            cid: Cid(cid.to_string()),
            verified_against: KeyId(format!("did:plc:priya-test#{version}")),
            evidence: (0..evidence)
                .map(|i| format!("https://example.test/{version}/{i}"))
                .collect(),
            references: picks.iter().copied().map(reference).collect(),
            ..super::tests::sample_claim()
        }
    }

    /// What the store must hold for `claim` once indexed: each distinct
    /// reference exactly once.
    fn whole(claim: &IndexedClaim) -> StoredClaim {
        let distinct: BTreeSet<(String, String)> = claim
            .references
            .iter()
            .map(|r| (r.cid.0.clone(), reference_type_wire(r.ref_type).to_string()))
            .collect();
        StoredClaim {
            row: Some((
                claim.subject.clone(),
                claim.object.clone(),
                claim.verified_against.0.clone(),
            )),
            evidence: claim
                .evidence
                .iter()
                .enumerate()
                .map(|(i, e)| (i32::try_from(i).expect("small"), e.clone()))
                .collect(),
            references: distinct.into_iter().collect(),
        }
    }

    fn arb_picks() -> impl Strategy<Value = Vec<(usize, usize)>> {
        proptest::collection::vec((0..REFERENCED.len(), 0..REF_TYPES.len()), 0..5)
    }

    /// References with at least one exact duplicate inserted at any position.
    fn arb_picks_with_a_duplicate() -> impl Strategy<Value = Vec<(usize, usize)>> {
        (
            proptest::collection::vec((0..REFERENCED.len(), 0..REF_TYPES.len()), 1..5),
            any::<proptest::sample::Index>(),
            any::<proptest::sample::Index>(),
        )
            .prop_map(|(mut picks, which, at)| {
                let duplicate = picks[which.index(picks.len())];
                picks.insert(at.index(picks.len() + 1), duplicate);
                picks
            })
    }

    /// A prior version of TARGET (distinct references) or none.
    fn arb_prior() -> impl Strategy<Value = Option<(Vec<(usize, usize)>, usize)>> {
        proptest::option::of((
            proptest::collection::btree_set((0..REFERENCED.len(), 0..REF_TYPES.len()), 0..4)
                .prop_map(|set| set.into_iter().collect()),
            0..3usize,
        ))
    }

    /// GIVEN a store holding NEIGHBOUR and, optionally, a prior TARGET.
    fn given_a_store(
        neighbour_picks: &[(usize, usize)],
        prior: Option<&(Vec<(usize, usize)>, usize)>,
    ) -> (tempfile::TempDir, IndexStoreAdapter) {
        let dir = tempfile::tempdir().expect("tempdir");
        let store = IndexStoreAdapter::open(&dir.path().join("index.duckdb")).expect("open");
        let neighbour = claim(NEIGHBOUR, "neighbour", &dedupe(neighbour_picks), 1);
        store.upsert(&neighbour).expect("upsert neighbour");
        if let Some((picks, evidence)) = prior {
            store
                .upsert(&claim(TARGET, "prior", picks, *evidence))
                .expect("upsert prior");
        }
        (dir, store)
    }

    fn dedupe(picks: &[(usize, usize)]) -> Vec<(usize, usize)> {
        picks
            .iter()
            .copied()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(24))]

        /// A claim listing a reference twice is indexed whole — on a first
        /// upsert and on a re-upsert over a prior version — and nothing else changes.
        #[test]
        fn upsert_with_duplicate_references_keeps_the_claim_whole(
            neighbour_picks in arb_picks(),
            prior in arb_prior(),
            picks in arb_picks_with_a_duplicate(),
            evidence in 0..3usize,
        ) {
            let (_dir, store) = given_a_store(&neighbour_picks, prior.as_ref());
            let neighbour_before = stored(&store, NEIGHBOUR);
            let incoming = claim(TARGET, "current", &picks, evidence);

            let outcome = store.upsert(&incoming);

            prop_assert!(outcome.is_ok(), "{:?}", outcome);
            prop_assert_eq!(stored(&store, TARGET), whole(&incoming));
            prop_assert_eq!(stored(&store, NEIGHBOUR), neighbour_before);
        }

        /// When a statement inside one upsert fails (here the `verified_against`
        /// CHECK on the row insert, after the old rows were cleared), none of that
        /// upsert's changes are visible: the prior version (or its absence) and
        /// every other claim are exactly as they were.
        #[test]
        fn a_failing_upsert_leaves_the_prior_version_unchanged(
            neighbour_picks in arb_picks(),
            prior in arb_prior(),
            picks in arb_picks(),
            evidence in 0..3usize,
        ) {
            let (_dir, store) = given_a_store(&neighbour_picks, prior.as_ref());
            let target_before = stored(&store, TARGET);
            let neighbour_before = stored(&store, NEIGHBOUR);
            let unstorable = IndexedClaim {
                verified_against: KeyId(String::new()),
                ..claim(TARGET, "current", &dedupe(&picks), evidence)
            };

            let outcome = store.upsert(&unstorable);

            prop_assert!(outcome.is_err(), "an empty verified_against must not be stored");
            prop_assert_eq!(stored(&store, TARGET), target_before);
            prop_assert_eq!(stored(&store, NEIGHBOUR), neighbour_before);
        }
    }
}

#[cfg(test)]
mod read_port_preservation_properties {
    //! B7 / ADR-082 @contract-shape:unbounded-preservation. Universe = EVERY row
    //! the store holds across `indexed_claims`, `indexed_claim_evidence` and
    //! `indexed_claim_references`. Any sequence of `IndexReadPort` reads, issued
    //! through the read port alone, leaves that whole universe unchanged.

    use super::*;
    use proptest::prelude::*;

    const OBJECTS: [&str; 2] = ["org.openlore.philosophy.a", "org.openlore.philosophy.b"];
    const SUBJECTS: [&str; 2] = ["github:o/one", "github:o/two"];
    const AUTHORS: [&str; 3] = ["did:plc:priya", "did:plc:sven#k", "did:plc:ana"];

    #[derive(Debug, Clone)]
    enum Read {
        Object(usize),
        Subject(usize),
        Contributor(usize),
        Cid(usize),
    }

    fn read_strategy() -> impl Strategy<Value = Read> {
        prop_oneof![
            (0..OBJECTS.len()).prop_map(Read::Object),
            (0..SUBJECTS.len()).prop_map(Read::Subject),
            (0..AUTHORS.len()).prop_map(Read::Contributor),
            (0..8usize).prop_map(Read::Cid),
        ]
    }

    /// Perform one read through the READ port only (the search handler's view).
    fn perform(reads: &dyn IndexReadPort, read: &Read) {
        let _ = match read {
            Read::Object(i) => reads.query_by_object(OBJECTS[*i]).map(|_| ()),
            Read::Subject(i) => reads.query_by_subject(SUBJECTS[*i]).map(|_| ()),
            Read::Contributor(i) => reads
                .query_by_contributor(&Did(AUTHORS[*i].to_string()))
                .map(|_| ()),
            Read::Cid(i) => reads.get_by_cid(&Cid(format!("bafyrow{i}"))).map(|_| ()),
        };
    }

    /// The whole store, in a comparable form.
    fn snapshot(store: &IndexStoreAdapter) -> Vec<String> {
        let conn = store.lock().expect("lock");
        [
            "SELECT cid || '|' || author_did || '|' || subject || '|' || object FROM indexed_claims ORDER BY cid",
            "SELECT cid || '|' || CAST(ordinal AS VARCHAR) || '|' || evidence FROM indexed_claim_evidence ORDER BY cid, ordinal",
            "SELECT referencing_cid || '|' || referenced_cid || '|' || ref_type FROM indexed_claim_references ORDER BY 1",
        ]
        .iter()
        .flat_map(|sql| {
            let mut stmt = conn.prepare(sql).expect("prepare");
            stmt.query_map([], |r| r.get::<_, String>(0))
                .expect("query")
                .map(|row| row.expect("row"))
                .collect::<Vec<_>>()
        })
        .collect()
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(16))]
        #[test]
        fn reads_through_the_read_port_leave_the_whole_store_unchanged(
            rows in proptest::collection::vec(
                (0..AUTHORS.len(), 0..SUBJECTS.len(), 0..OBJECTS.len(), 0..3usize),
                0..8,
            ),
            reads in proptest::collection::vec(read_strategy(), 0..12),
        ) {
            let dir = tempfile::tempdir().expect("tempdir");
            let store = IndexStoreAdapter::open(&dir.path().join("index.duckdb")).expect("open");
            for (index, (author, subject, object, evidence)) in rows.iter().enumerate() {
                let did = AUTHORS[*author].to_string();
                store
                    .upsert(&IndexedClaim {
                        author_did: Did(did.clone()),
                        verified_against: KeyId(did),
                        cid: Cid(format!("bafyrow{index}")),
                        subject: SUBJECTS[*subject].to_string(),
                        object: OBJECTS[*object].to_string(),
                        evidence: (0..*evidence).map(|e| format!("https://e.test/{e}")).collect(),
                        references: if index > 0 {
                            vec![ClaimReference {
                                ref_type: ReferenceType::Counters,
                                cid: Cid(format!("bafyrow{}", index - 1)),
                            }]
                        } else {
                            Vec::new()
                        },
                        ..super::tests::sample_claim()
                    })
                    .expect("upsert");
            }

            let before = snapshot(&store);
            reads.iter().for_each(|read| perform(&store, read));
            prop_assert_eq!(snapshot(&store), before);
        }
    }
}
