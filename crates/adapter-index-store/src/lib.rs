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
use claim_domain::{Cid, ClaimReference, Did, KeyId, ReferenceType};
use duckdb::Connection;
use ports::{
    AuthorRelationship, IndexStoreError, IndexStorePort, IndexedClaim, ProbeOutcome,
    ProbeRefusalReason,
};

mod schema;

/// Embedded-DuckDB `IndexStorePort` adapter over the SEPARATE `index.duckdb`
/// (ADR-023/025).
///
/// Holds the open DB handle (behind an `Arc<Mutex<_>>` because DuckDB's
/// `Connection` is `!Sync`) + the path to the colocated `indexed_claims/`
/// artifact directory (`<index dir>/indexed_claims/<author_did>/<cid>.json`).
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
        write_atomic(&tmp_path, &final_path, &payload).map_err(|err| {
            IndexStoreError::WriteFailed {
                cid: claim.cid.clone(),
                message: err,
            }
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
    fn select_rows(
        &self,
        where_clause: &str,
        bind: &str,
    ) -> Result<Vec<IndexedClaim>, IndexStoreError> {
        let conn = self.lock()?;
        let sql = format!(
            "SELECT author_did, cid, subject, predicate, object, confidence, \
                    composed_at, verified_against \
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
        Ok(out)
    }
}

impl IndexStorePort for IndexStoreAdapter {
    fn probe(&self) -> ProbeOutcome {
        // Earned-Trust probe (happy-path arm for the AV-1 walking skeleton): the
        // schema version must match what this binary knows how to read. The full
        // fsync + attribution-round-trip + no-merge-schema substrate-lie arms are
        // AV-6/03-06; here we assert the schema is present + current so the
        // gauntlet has a REAL readiness check (not a trivial `Ok`).
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
        match schema::read_version(&conn) {
            Ok(v) if v == schema::LATEST_VERSION => ProbeOutcome::Ok,
            Ok(v) => ProbeOutcome::Refused {
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
            },
            Err(err) => ProbeOutcome::Refused {
                reason: ProbeRefusalReason::StorageSchemaMismatch,
                detail: format!("could not read index schema version: {err}"),
                structured: serde_json::json!({"adapter": "index_store"}),
            },
        }
    }

    fn upsert(&self, claim: &IndexedClaim) -> Result<(), IndexStoreError> {
        // Write the JSON artifact first (the authoritative on-disk record), then
        // index the row. De-dup by CID only (ADR-025): an INSERT OR REPLACE on
        // the CID PK leaves exactly one row per CID.
        self.write_artifact(claim)?;
        let signed_record_path = Self::artifact_rel_path(claim);

        let conn = self.lock()?;
        // Replace the row + its children idempotently (de-dup by CID PK).
        conn.execute(
            "DELETE FROM indexed_claim_evidence WHERE cid = ?",
            duckdb::params![claim.cid.0],
        )
        .map_err(|err| write_failed(claim, format!("clear evidence: {err}")))?;
        conn.execute(
            "DELETE FROM indexed_claim_references WHERE referencing_cid = ?",
            duckdb::params![claim.cid.0],
        )
        .map_err(|err| write_failed(claim, format!("clear references: {err}")))?;
        conn.execute(
            "DELETE FROM indexed_claims WHERE cid = ?",
            duckdb::params![claim.cid.0],
        )
        .map_err(|err| write_failed(claim, format!("clear row: {err}")))?;

        conn.execute(
            "INSERT INTO indexed_claims (\
                cid, author_did, subject, predicate, object, confidence, \
                composed_at, indexed_at, source_pds, signed_record_path, verified_against\
             ) VALUES (?, ?, ?, ?, ?, ?, ?, now(), ?, ?, ?)",
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
            ],
        )
        .map_err(|err| write_failed(claim, format!("insert row: {err}")))?;

        for (ordinal, evidence) in claim.evidence.iter().enumerate() {
            conn.execute(
                "INSERT INTO indexed_claim_evidence (cid, evidence, ordinal) VALUES (?, ?, ?)",
                duckdb::params![claim.cid.0, evidence, ordinal as i32],
            )
            .map_err(|err| write_failed(claim, format!("insert evidence: {err}")))?;
        }

        for reference in &claim.references {
            conn.execute(
                "INSERT INTO indexed_claim_references \
                    (referencing_cid, referenced_cid, ref_type) VALUES (?, ?, ?)",
                duckdb::params![
                    claim.cid.0,
                    reference.cid.0,
                    reference_type_wire(reference.ref_type)
                ],
            )
            .map_err(|err| write_failed(claim, format!("insert reference: {err}")))?;
        }

        Ok(())
    }

    fn query_by_object(&self, object: &str) -> Result<Vec<IndexedClaim>, IndexStoreError> {
        self.select_rows("object = ?", object)
    }

    fn query_by_contributor(&self, did: &Did) -> Result<Vec<IndexedClaim>, IndexStoreError> {
        self.select_rows("author_did = ?", &did.0)
    }

    fn query_by_subject(&self, subject: &str) -> Result<Vec<IndexedClaim>, IndexStoreError> {
        self.select_rows("subject = ?", subject)
    }

    fn get_by_cid(&self, cid: &Cid) -> Result<Option<IndexedClaim>, IndexStoreError> {
        Ok(self.select_rows("cid = ?", &cid.0)?.into_iter().next())
    }
}

// -----------------------------------------------------------------------------
// Pure helpers (free functions; no I/O except where wired through the adapter)
// -----------------------------------------------------------------------------

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
    })
}

/// The JSON artifact body for an indexed claim (the verified network record, as
/// stored at `indexed_claims/<did>/<cid>.json`).
fn artifact_json(claim: &IndexedClaim) -> serde_json::Value {
    serde_json::json!({
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
    })
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
    fn sample_claim() -> IndexedClaim {
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
            verified_against: KeyId(
                "did:plc:priya-test#org.openlore.application".to_string(),
            ),
            evidence: vec!["https://example.test/evidence/bazel".to_string()],
            references: Vec::new(),
            relationship: AuthorRelationship::NetworkUnfollowed,
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
}
