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
use std::sync::{Arc, Mutex};

use chrono::{DateTime, NaiveDateTime, Utc};
use claim_domain::{Cid, Did, ReferenceType, SignedClaim};
use duckdb::Connection;
use ports::{
    AttributedClaim, AuthorRelationship, FederatedRow, GraphNode, ProbeOutcome, ScoringFilter,
    SourceTable, StorageError, StoragePort, TraversalBound, TraversalResult,
};

mod graph_query;
mod peer_storage;
mod probe;
mod schema;
mod schema_v3;

pub use peer_storage::DuckDbPeerStorageAdapter;

/// Embedded-DuckDB `StoragePort` adapter.
///
/// Holds the open DB handle (behind an `Arc<Mutex<_>>` because
/// `Connection` is `!Sync` AND the slice-03 `DuckDbPeerStorageAdapter`
/// SHARES this exact handle to honor DuckDB's single-writer constraint —
/// Q-DELIVER-3) plus the path to the colocated `claims/` directory and
/// the `peer_claims/` root.
pub struct DuckDbStorageAdapter {
    conn: Arc<Mutex<Connection>>,
    claims_dir: PathBuf,
    peer_claims_root: PathBuf,
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
        // Slice-03 migration v3: idempotent forward-only follow-on. On a
        // slice-01 DB (version=1) this jumps to version=3 (version=2 is
        // reserved for slice-02 if installed separately; safe to skip).
        // See `schema_v3` + data-models.md §"Migration policy".
        schema_v3::run_migration(&mut conn)?;

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

        // Colocate `peer_claims/` next to the DB file (data-models.md
        // §"On-disk artifact format — slice-03 additions"). Per-peer
        // subtrees `<peer_did>/<cid>.json` land at write time; we only
        // ensure the root exists here.
        let peer_claims_root = db_path
            .parent()
            .map(|p| p.join("peer_claims"))
            .unwrap_or_else(|| PathBuf::from("peer_claims"));
        fs::create_dir_all(&peer_claims_root).map_err(|err| {
            StorageError::SchemaMigrationFailed {
                message: format!(
                    "create peer_claims root {}: {err}",
                    peer_claims_root.display()
                ),
            }
        })?;

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
            claims_dir,
            peer_claims_root,
        })
    }

    /// Construct a `DuckDbPeerStorageAdapter` SHARING this adapter's
    /// connection handle (Q-DELIVER-3 single-writer constraint). Both
    /// adapters then serialize all writes through one mutex; no second
    /// DuckDB handle to the file is ever opened.
    ///
    /// `local_did` is the composition root's `IdentityPort::author_did()` —
    /// the peer adapter holds it so `write_peer_claim` can reject a record
    /// self-attributed to the local user (WD-40 layer-2 storage guard).
    pub fn peer_adapter(&self, local_did: &claim_domain::Did) -> DuckDbPeerStorageAdapter {
        DuckDbPeerStorageAdapter::from_shared(
            Arc::clone(&self.conn),
            self.peer_claims_root.clone(),
            local_did,
        )
    }

    /// Construct the artifact path for a CID: `<claims_dir>/<cid>.json`.
    fn artifact_path(&self, cid: &Cid) -> PathBuf {
        self.claims_dir.join(format!("{}.json", cid.0))
    }

    /// The set of DIDs with a currently-ACTIVE subscription
    /// (`removed_at IS NULL`). Used by `query_federated_by_subject` to
    /// classify each `Peer`-sourced row as `SubscribedPeer` (DID present)
    /// vs `UnsubscribedCache` (soft-removed residue, DID absent). Reads the
    /// shared connection once per federated query.
    fn active_subscription_dids(&self) -> Result<std::collections::HashSet<String>, StorageError> {
        let conn = self.conn.lock().map_err(|_| StorageError::QueryFailed {
            message: "connection mutex poisoned".to_string(),
        })?;
        let mut stmt = conn
            .prepare("SELECT peer_did FROM peer_subscriptions WHERE removed_at IS NULL")
            .map_err(|err| StorageError::QueryFailed {
                message: format!("prepare active_subscription_dids: {err}"),
            })?;
        let rows = stmt
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(|err| StorageError::QueryFailed {
                message: format!("query active_subscription_dids: {err}"),
            })?;
        let mut dids = std::collections::HashSet::new();
        for row in rows {
            dids.insert(row.map_err(|err| StorageError::QueryFailed {
                message: format!("row decode active_subscription_dids: {err}"),
            })?);
        }
        Ok(dids)
    }

    /// Read the authoritative `SignedClaim` from an on-disk artifact path.
    /// Own-claim rows store an ABSOLUTE path under `claims/`; peer-claim
    /// rows store a path RELATIVE to the storage root
    /// (`peer_claims/<encoded_did>/<cid>.json`). A relative `peer_claims/`
    /// path is resolved under `peer_claims_root` (which already points at
    /// `<root>/peer_claims`), matching `DuckDbPeerStorageAdapter`'s own
    /// `get_peer_claim_by_cid` resolution.
    fn read_artifact_at(&self, artifact_path: &str) -> Result<SignedClaim, StorageError> {
        let resolved: PathBuf = match artifact_path.strip_prefix("peer_claims/") {
            Some(relative) => self.peer_claims_root.join(relative),
            None => PathBuf::from(artifact_path),
        };
        let bytes = fs::read(&resolved).map_err(|err| StorageError::QueryFailed {
            message: format!("read federated artifact {}: {err}", resolved.display()),
        })?;
        serde_json::from_slice(&bytes).map_err(|err| StorageError::QueryFailed {
            message: format!(
                "deserialize federated artifact {}: {err}",
                resolved.display()
            ),
        })
    }
}

/// Lightweight projection from the federated UNION-ALL query — just the
/// attribution + the artifact pointer. The full `SignedClaim` is read from
/// the artifact afterward so the returned row is byte-faithful.
struct FederatedProjection {
    author_did: String,
    source_table: String,
    artifact_path: String,
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

    fn query_federated_by_subject(&self, subject: &str) -> Result<Vec<FederatedRow>, StorageError> {
        // SAFE cross-store read (data-models.md §"Cross-store query
        // examples"): a SQL `UNION ALL` that projects `author_did`
        // EXPLICITLY from BOTH `claims` and `peer_claims`. This is the
        // structural anti-merging defense (I-FED-1 layer 2): because the
        // literal names both tables AND projects `author_did`, it passes
        // `xtask check-arch::no_cross_table_join_elides_author`. It is
        // NEVER a JOIN (which would risk eliding attribution).
        //
        // The query yields only the lightweight projection
        // `(author_did, cid, source_table, signed_record_path)`; the
        // authoritative `SignedClaim` is then read from the on-disk
        // artifact per row so the returned claim is byte-faithful
        // (its CID + signature still round-trip), matching the slice-01
        // `query_by_subject` strategy.
        let projections: Vec<FederatedProjection> = {
            let conn = self.conn.lock().map_err(|_| StorageError::QueryFailed {
                message: "connection mutex poisoned".to_string(),
            })?;

            let mut stmt = conn
                .prepare(
                    "SELECT author_did, cid, source_table, artifact_path FROM ( \
                       SELECT c.author_did AS author_did, \
                              c.cid AS cid, \
                              'Own' AS source_table, \
                              c.artifact_path AS artifact_path \
                       FROM claims c \
                       WHERE c.subject = ? \
                       UNION ALL \
                       SELECT pc.author_did AS author_did, \
                              pc.cid AS cid, \
                              'Peer' AS source_table, \
                              pc.signed_record_path AS artifact_path \
                       FROM peer_claims pc \
                       WHERE pc.subject = ? \
                     ) ORDER BY source_table, cid",
                )
                .map_err(|err| StorageError::QueryFailed {
                    message: format!("prepare query_federated_by_subject: {err}"),
                })?;

            let rows = stmt
                .query_map(duckdb::params![subject, subject], |row| {
                    Ok(FederatedProjection {
                        author_did: row.get::<_, String>(0)?,
                        source_table: row.get::<_, String>(2)?,
                        artifact_path: row.get::<_, String>(3)?,
                    })
                })
                .map_err(|err| StorageError::QueryFailed {
                    message: format!("query_map federated: {err}"),
                })?;

            let mut collected = Vec::new();
            for row in rows {
                collected.push(row.map_err(|err| StorageError::QueryFailed {
                    message: format!("row decode federated: {err}"),
                })?);
            }
            collected
        };

        // The set of currently-ACTIVE peer subscriptions (removed_at IS
        // NULL). A `Peer`-sourced row whose author is in this set is a
        // `SubscribedPeer`; one whose subscription was soft-removed (or
        // never present) is an `UnsubscribedCache` (ADR-014 soft-remove
        // residue). Read once, up front, so the per-row classification is
        // an O(1) set membership test.
        let active_peers = self.active_subscription_dids()?;

        let mut results = Vec::with_capacity(projections.len());
        for projection in projections {
            let source_table = match projection.source_table.as_str() {
                "Own" => SourceTable::Own,
                "Peer" => SourceTable::Peer,
                other => {
                    return Err(StorageError::QueryFailed {
                        message: format!("unknown source_table {other:?} in federated read"),
                    })
                }
            };

            // Normalize attribution to the BARE DID (fragment stripped) so
            // own rows (`claims.author_did` carries the `#org.openlore.application`
            // signing-key fragment) and peer rows (already bare) group under
            // ONE header per author. The signing-key fragment is a locator,
            // not an attribution distinction.
            let author_did = bare_did(&projection.author_did);

            let author_relationship = match source_table {
                SourceTable::Own => AuthorRelationship::You,
                SourceTable::Peer => {
                    if active_peers.contains(&author_did) {
                        AuthorRelationship::SubscribedPeer
                    } else {
                        AuthorRelationship::UnsubscribedCache
                    }
                }
            };

            let signed_claim = self.read_artifact_at(&projection.artifact_path)?;

            results.push(FederatedRow {
                author_did: claim_domain::Did(author_did),
                author_relationship,
                signed_claim,
                source_table,
            });
        }

        Ok(results)
    }

    // -------- slice-04 (scoring + graph) --------
    //
    // The four new read methods AUGMENT the SAME single-file DuckDB store
    // (WD-8 — NO new table, NO store swap). Each delegates to the
    // `graph_query` effect-shell helpers, which carry the recursive-CTE +
    // attributed `UNION ALL` SQL (cross-store reads project `author_did`
    // explicitly — `xtask check-arch`'s extended
    // `no_cross_table_join_elides_author` enforces it; aggregation is the pure
    // `scoring` core's job, NEVER SQL). Bodies are SCAFFOLD: true (slice-04) —
    // the live SQL lands per-scenario in Phase 03/04/05.

    fn query_by_object(&self, object: &str) -> Result<Vec<AttributedClaim>, StorageError> {
        graph_query::query_by_object(&self.conn, object)
    }

    fn query_by_contributor(
        &self,
        author_did: &Did,
    ) -> Result<Vec<AttributedClaim>, StorageError> {
        graph_query::query_by_contributor(&self.conn, author_did)
    }

    fn query_attributed_for_scoring(
        &self,
        filter: &ScoringFilter,
    ) -> Result<Vec<AttributedClaim>, StorageError> {
        graph_query::query_attributed_for_scoring(&self.conn, filter)
    }

    fn traverse_graph(
        &self,
        start: &GraphNode,
        bound: &TraversalBound,
    ) -> Result<TraversalResult, StorageError> {
        graph_query::traverse_graph(&self.conn, start, bound)
    }
}

// -----------------------------------------------------------------------------
// Small named helpers (nw-fp-usable-design — never inline a one-liner that
// has a domain name)
// -----------------------------------------------------------------------------

/// Strip the `#fragment` from a DID, returning the bare DID. The author
/// `claims` table stores the full `did:...#org.openlore.application` signing
/// locator; the federated read normalizes to the bare form so own + peer
/// rows attribute under one author header (matches the peer adapter's
/// `bare_did`).
fn bare_did(did: &str) -> String {
    did.split('#').next().unwrap_or(did).to_string()
}

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

// -----------------------------------------------------------------------------
// Migration-v3 integration tests (Mandate 6: adapter tests use REAL DuckDB).
//
// These enter through the public driving surface (`DuckDbStorageAdapter::open`,
// which runs migration v3) and assert at the DB boundary — the observable
// schema state. They are RED before `schema_v3` lands and the v3 runner is
// wired into `open`; GREEN after.
//
// Behavior budget: 3 distinct behaviors (table creation; version
// registration; CHECK enforcement) → 3 tests, within the 2×3=6 budget.
// -----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use claim_domain::{Confidence, Did, SignatureBlock, UnsignedClaim};
    use ports::{AuthorRelationship, PeerStoragePort, SourceTable};
    use tempfile::tempdir;
    use url::Url;

    /// Open a fresh adapter on a tmp DB file and hand back both the
    /// adapter (to keep the shared connection alive) and a clone of the
    /// Arc<Mutex<Connection>> for direct schema introspection.
    fn open_tmp() -> (tempfile::TempDir, DuckDbStorageAdapter) {
        let dir = tempdir().expect("tempdir");
        let db_path = dir.path().join("openlore.duckdb");
        let adapter = DuckDbStorageAdapter::open(&db_path).expect("open adapter");
        (dir, adapter)
    }

    /// Build a `SignedClaim` authored by `author_did` (bare DID; the
    /// `#fragment` is appended) about `subject`. `cid` is both the
    /// `signed_cid` and the row primary key. Confidence routes through
    /// serde (the wrapper is crate-private).
    fn signed_claim(author_did: &str, subject: &str, object: &str, cid: &str) -> SignedClaim {
        let confidence: Confidence = serde_json::from_value(serde_json::json!(0.5))
            .expect("0.5 is a well-formed confidence");
        SignedClaim {
            unsigned: UnsignedClaim {
                subject: subject.to_string(),
                predicate: "embodiesPhilosophy".to_string(),
                object: object.to_string(),
                evidence: vec!["https://github.com/rust-lang/cargo".to_string()],
                confidence,
                author_did: Did(format!("{author_did}#org.openlore.application")),
                composed_at: "2026-05-22T09:18:44Z".to_string(),
                references: Vec::new(),
                reason: None,
            },
            signature: SignatureBlock {
                signed_cid: Cid(cid.to_string()),
                signature_bytes: vec![0u8; 64],
                verification_method: format!("{author_did}#org.openlore.application"),
            },
        }
    }

    /// Behavior: migration v3 creates all four peer-storage tables.
    ///
    /// Observable check: `SELECT * FROM <table> LIMIT 0` succeeds for
    /// each table (it errors with "table does not exist" if the table is
    /// absent). LIMIT 0 returns no rows but still validates the table +
    /// every column reference resolves.
    #[test]
    fn migration_v3_creates_all_four_peer_tables() {
        let (_dir, adapter) = open_tmp();
        let conn = adapter.conn.lock().expect("lock conn");

        for table in [
            "peer_subscriptions",
            "peer_claims",
            "peer_claim_references",
            "peer_claim_evidence",
        ] {
            let sql = format!("SELECT * FROM {table} LIMIT 0");
            conn.execute_batch(&sql)
                .unwrap_or_else(|err| panic!("table {table} must exist after migration v3: {err}"));
        }
    }

    /// Behavior: migration v3 registers schema_version (version=3,
    /// description='slice-03 peer storage'), and re-opening is idempotent
    /// (still exactly one v3 row).
    #[test]
    fn migration_v3_registers_schema_version_three_idempotently() {
        let dir = tempdir().expect("tempdir");
        let db_path = dir.path().join("openlore.duckdb");

        // First open applies migration v3.
        {
            let adapter = DuckDbStorageAdapter::open(&db_path).expect("first open");
            let conn = adapter.conn.lock().expect("lock conn");
            let description: String = conn
                .query_row(
                    "SELECT description FROM schema_version WHERE version = 3",
                    [],
                    |row| row.get(0),
                )
                .expect("schema_version v3 row must exist");
            assert_eq!(
                description, "slice-03 peer storage",
                "schema_version v3 description must match the acceptance criterion"
            );
        }

        // Second open must be a no-op for v3 (idempotent forward-only).
        {
            let adapter = DuckDbStorageAdapter::open(&db_path).expect("re-open");
            let conn = adapter.conn.lock().expect("lock conn");
            let v3_count: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM schema_version WHERE version = 3",
                    [],
                    |row| row.get(0),
                )
                .expect("count v3 rows");
            assert_eq!(
                v3_count, 1,
                "re-opening must NOT insert a duplicate schema_version v3 row (idempotent migration)"
            );
        }
    }

    /// Behavior: the `CHECK (author_did <> '')` constraint on
    /// `peer_claims` rejects an empty-author_did insert (I-FED-2
    /// defense-in-depth). The insert MUST error; we assert the adapter's
    /// shared connection refuses it.
    #[test]
    fn peer_claims_check_rejects_empty_author_did() {
        let (_dir, adapter) = open_tmp();
        let conn = adapter.conn.lock().expect("lock conn");

        let result = conn.execute(
            "INSERT INTO peer_claims \
             (cid, author_did, subject, predicate, object, confidence, \
              composed_at, fetched_at, fetched_from_pds, signed_record_path) \
             VALUES (?, ?, ?, ?, ?, ?, now(), now(), ?, ?)",
            duckdb::params![
                "bafytestcid",
                "", // empty author_did — MUST be rejected by CHECK
                "github:rust-lang/cargo",
                "embodiesPhilosophy",
                "org.openlore.philosophy.x",
                0.5_f64,
                "https://pds.example.test",
                "/tmp/peer_claims/x/bafytestcid.json",
            ],
        );

        assert!(
            result.is_err(),
            "INSERT with empty author_did must be rejected by the CHECK (author_did <> '') constraint"
        );
    }

    /// FQ-1 (step 05-06): `query_federated_by_subject` returns rows from
    /// both the user's own author store AND the cached peer store for the
    /// subject — each row carrying its OWN author_did (anti-merging,
    /// I-FED-1), the correct `source_table`, and the correct
    /// `author_relationship` (You for own; SubscribedPeer for an active
    /// peer subscription). The UNION-ALL never merges: distinct authors
    /// stay distinct rows.
    #[test]
    fn query_federated_by_subject_returns_own_and_peer_rows_attributed_per_row() {
        let local_did = "did:plc:test-jeff";
        let peer_did = "did:plc:rachel-test";
        let subject = "github:rust-lang/cargo";

        let (_dir, storage) = open_tmp();
        let peer = storage.peer_adapter(&Did(local_did.to_string()));

        // Own claim → `claims` table via the slice-01 write path.
        let own = signed_claim(
            local_did,
            subject,
            "org.openlore.philosophy.local-first",
            "bafyowncid0000000000000000000000000000000000",
        );
        storage.write_signed_claim(&own).expect("write own claim");

        // An ACTIVE subscription to Rachel so the row is annotated
        // SubscribedPeer (not UnsubscribedCache).
        let endpoint = Url::parse("https://pds.example.test").expect("valid url");
        peer.add_subscription(ports::PeerSubscription {
            peer_did: Did(peer_did.to_string()),
            peer_handle: "rachel.test".to_string(),
            peer_pds_endpoint: endpoint.clone(),
            subscribed_at: Utc::now(),
            removed_at: None,
        })
        .expect("add subscription");

        // Two peer claims → `peer_claims` table via the pull write path.
        for (object, cid) in [
            (
                "org.openlore.philosophy.dependency-pinning",
                "bafypeercid001000000000000000000000000000000",
            ),
            (
                "org.openlore.philosophy.reproducible-builds",
                "bafypeercid002000000000000000000000000000000",
            ),
        ] {
            let peer_claim = signed_claim(peer_did, subject, object, cid);
            peer.write_peer_claim(
                &Did(peer_did.to_string()),
                &peer_claim,
                &endpoint,
                Utc::now(),
            )
            .expect("write peer claim");
        }

        let rows = storage
            .query_federated_by_subject(subject)
            .expect("query_federated_by_subject");

        assert_eq!(
            rows.len(),
            3,
            "expected 1 own + 2 peer rows from the UNION ALL; got {}",
            rows.len()
        );

        // Anti-merging: exactly two DISTINCT author DIDs, each row carrying
        // a non-empty author_did (the field is non-Option at the type level;
        // this asserts the adapter populated it from the right column).
        let own_rows: Vec<&FederatedRow> = rows
            .iter()
            .filter(|r| r.author_did.0 == local_did)
            .collect();
        let peer_rows: Vec<&FederatedRow> =
            rows.iter().filter(|r| r.author_did.0 == peer_did).collect();
        assert_eq!(own_rows.len(), 1, "exactly one own-authored row");
        assert_eq!(peer_rows.len(), 2, "exactly two peer-authored rows");

        // source_table + author_relationship are correct per provenance.
        assert_eq!(own_rows[0].source_table, SourceTable::Own);
        assert_eq!(own_rows[0].author_relationship, AuthorRelationship::You);
        for row in &peer_rows {
            assert_eq!(row.source_table, SourceTable::Peer);
            assert_eq!(row.author_relationship, AuthorRelationship::SubscribedPeer);
            assert!(
                !row.author_did.0.is_empty(),
                "every federated row carries a non-empty author_did (I-FED-1)"
            );
        }

        // The signed claims round-trip: the own row's CID matches, and each
        // peer row's CID is one of the two we wrote (no CID drift).
        assert_eq!(
            own_rows[0].signed_claim.signature.signed_cid.0,
            own.signature.signed_cid.0
        );
        let peer_cids: std::collections::HashSet<String> = peer_rows
            .iter()
            .map(|r| r.signed_claim.signature.signed_cid.0.clone())
            .collect();
        assert_eq!(peer_cids.len(), 2, "two distinct peer CIDs preserved");
    }

    /// A subject with no matching rows in EITHER table returns an empty
    /// vec (not an error). Pins the empty-federation degenerate case the
    /// renderer's footer-only path relies on.
    #[test]
    fn query_federated_by_subject_empty_when_no_rows_match() {
        let (_dir, storage) = open_tmp();
        let rows = storage
            .query_federated_by_subject("github:does/not-exist")
            .expect("query_federated_by_subject");
        assert!(rows.is_empty(), "expected no rows; got {}", rows.len());
    }
}
