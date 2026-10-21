//! `DuckDbImportedCandidateAdapter` — the insert-only local record of what
//! imports derived (`imported_candidates`, migration v9, OOS-D12 B, C-OOS-7)
//! over the SHARED DuckDB connection.
//!
//! No statement in this module rewrites a kept candidate: `record_import`
//! takes one `import_id` from its sequence and inserts one row per
//! candidate, all inside one transaction. This table is the only home of
//! imported candidates; scoring and inference never read it.
//!
//! The LIVE probe records one sentinel import inside a transaction that is
//! always rolled back, and refuses startup unless it reads back by its
//! subject.

use crate::conn::{ConnGuard, SharedConn};

use chrono::{TimeZone, Utc};
use duckdb::Connection;
use ports::{
    CandidateImport, ImportedCandidate, ImportedCandidateError, ImportedCandidateReadPort,
    ImportedCandidateWritePort, KeptCandidate, ProbeOutcome, ProbeRefusalReason, RepoSubjectKey,
};

const NEXT_IMPORT_ID_SQL: &str = "SELECT nextval('candidate_import_ids')";

/// The evidence URLs travel as one space-joined parameter (a URL holds no
/// space); an empty join stores an empty list.
const INSERT_CANDIDATE_SQL: &str = "
    INSERT INTO imported_candidates (
        import_id, subject, subject_key, predicate, object, confidence,
        evidence, imported_at
    ) VALUES (
        ?, ?, ?, ?, ?, ?,
        CASE WHEN CAST(? AS VARCHAR) = '' THEN []::VARCHAR[] ELSE string_split(?, ' ') END,
        ?
    )";

const KEPT_COLUMNS: &str = "
    SELECT import_id, imported_at, subject, predicate, object, confidence,
           array_to_string(evidence, ' ')
    FROM imported_candidates";

/// Each (subject, predicate, object) once, as last imported (ruling P-OOS-3):
/// max `imported_at`, then max `import_id`. The de-duplication lives in the
/// read only — every import's rows stay stored.
const LATEST_IMPORT_ONLY: &str = "
    QUALIFY row_number() OVER (
        PARTITION BY subject_key, predicate, object
        ORDER BY imported_at DESC, import_id DESC
    ) = 1";

const KEPT_ORDER: &str = "ORDER BY subject_key, predicate, object";

/// The candidates imports kept, sharing the one DuckDB connection.
pub struct DuckDbImportedCandidateAdapter {
    conn: SharedConn,
}

impl DuckDbImportedCandidateAdapter {
    /// Construct from the SHARED handle (see
    /// `DuckDbStorageAdapter::imported_candidate_adapter`). Opens no second
    /// handle and runs no migration (v9 ran at `open`).
    pub(crate) fn from_shared(conn: SharedConn) -> Self {
        Self { conn }
    }

    fn lock(&self) -> Result<ConnGuard<'_>, ImportedCandidateError> {
        self.conn
            .lock()
            .map_err(|lock_err| ImportedCandidateError::Store(lock_err.to_string()))
    }
}

fn store_error(step: &str) -> impl Fn(duckdb::Error) -> ImportedCandidateError + '_ {
    move |err| ImportedCandidateError::Store(format!("{step}: {err}"))
}

// -----------------------------------------------------------------------------
// Insert and decode
// -----------------------------------------------------------------------------

fn insert_candidate(
    conn: &Connection,
    import_id: i64,
    imported_at: chrono::DateTime<Utc>,
    candidate: &ImportedCandidate,
) -> Result<(), duckdb::Error> {
    let evidence = candidate.evidence.join(" ");
    conn.execute(
        INSERT_CANDIDATE_SQL,
        duckdb::params![
            import_id,
            candidate.subject,
            candidate.subject_key().as_str(),
            candidate.predicate,
            candidate.object,
            candidate.confidence,
            evidence,
            evidence,
            imported_at,
        ],
    )
    .map(|_| ())
}

/// Insert one import: one fresh `import_id`, one row per candidate.
fn insert_import(conn: &Connection, import: &CandidateImport) -> Result<(), duckdb::Error> {
    let import_id: i64 = conn.query_row(NEXT_IMPORT_ID_SQL, [], |row| row.get(0))?;
    import
        .candidates
        .iter()
        .try_for_each(|candidate| insert_candidate(conn, import_id, import.imported_at, candidate))
}

fn decode_kept(row: &duckdb::Row<'_>) -> Result<KeptCandidate, duckdb::Error> {
    let joined_evidence: String = row.get(6)?;
    Ok(KeptCandidate {
        import_id: row.get(0)?,
        imported_at: row.get(1)?,
        candidate: ImportedCandidate {
            subject: row.get(2)?,
            predicate: row.get(3)?,
            object: row.get(4)?,
            confidence: row.get(5)?,
            evidence: joined_evidence
                .split_whitespace()
                .map(str::to_string)
                .collect(),
        },
    })
}

fn query_kept(
    conn: &Connection,
    subject: Option<&RepoSubjectKey>,
) -> Result<Vec<KeptCandidate>, duckdb::Error> {
    match subject {
        Some(key) => {
            let sql =
                format!("{KEPT_COLUMNS} WHERE subject_key = ? {LATEST_IMPORT_ONLY} {KEPT_ORDER}");
            let mut stmt = conn.prepare(&sql)?;
            let rows = stmt.query_map(duckdb::params![key.as_str()], decode_kept)?;
            rows.collect()
        }
        None => {
            let sql = format!("{KEPT_COLUMNS} {LATEST_IMPORT_ONLY} {KEPT_ORDER}");
            let mut stmt = conn.prepare(&sql)?;
            let rows = stmt.query_map([], decode_kept)?;
            rows.collect()
        }
    }
}

impl ImportedCandidateReadPort for DuckDbImportedCandidateAdapter {
    fn probe(&self) -> ProbeOutcome {
        match self.lock() {
            Ok(mut conn) => probe_sentinel_import_kept(&mut conn),
            Err(err) => refuse(err.to_string()),
        }
    }

    fn list(
        &self,
        subject: Option<&RepoSubjectKey>,
    ) -> Result<Vec<KeptCandidate>, ImportedCandidateError> {
        let conn = self.lock()?;
        query_kept(&conn, subject).map_err(store_error("list imported candidates"))
    }
}

impl ImportedCandidateWritePort for DuckDbImportedCandidateAdapter {
    fn record_import(&self, import: &CandidateImport) -> Result<(), ImportedCandidateError> {
        let mut conn = self.lock()?;
        let tx = conn
            .transaction()
            .map_err(store_error("begin keeping an import"))?;
        insert_import(&tx, import).map_err(store_error("keep imported candidates"))?;
        tx.commit().map_err(store_error("commit kept import"))
    }
}

// -----------------------------------------------------------------------------
// Live probe — one sentinel import, always rolled back
// -----------------------------------------------------------------------------

const SENTINEL_SUBJECT: &str = "github:openlore-probe/sentinel";

fn sentinel_candidate() -> ImportedCandidate {
    ImportedCandidate {
        subject: SENTINEL_SUBJECT.to_string(),
        predicate: "embodiesPhilosophy".to_string(),
        object: "org.openlore.philosophy.openlore-probe".to_string(),
        confidence: 0.25,
        evidence: vec!["https://github.com/openlore-probe/sentinel".to_string()],
    }
}

fn sentinel_import() -> CandidateImport {
    CandidateImport {
        imported_at: Utc
            .with_ymd_and_hms(2000, 1, 1, 0, 0, 0)
            .single()
            .unwrap_or_default(),
        candidates: vec![sentinel_candidate()],
    }
}

fn probe_sentinel_import_kept(conn: &mut Connection) -> ProbeOutcome {
    match record_sentinel_import(conn) {
        Ok(readback) => judge_sentinel(&readback),
        Err(err) => refuse(format!(
            "imported_candidates could not keep an import: {err}"
        )),
    }
}

/// Keep the sentinel import inside a transaction that is rolled back — the
/// store is untouched.
fn record_sentinel_import(conn: &mut Connection) -> Result<Vec<ImportedCandidate>, duckdb::Error> {
    let tx = conn.transaction()?;
    insert_import(&tx, &sentinel_import())?;
    let kept = query_kept(&tx, Some(&RepoSubjectKey::of(SENTINEL_SUBJECT)))?;
    tx.rollback()?;
    Ok(kept.into_iter().map(|kept| kept.candidate).collect())
}

/// Pure verdict over the sentinel readback.
fn judge_sentinel(readback: &[ImportedCandidate]) -> ProbeOutcome {
    let expected = vec![sentinel_candidate()];
    if readback == expected.as_slice() {
        ProbeOutcome::Ok
    } else {
        refuse(format!(
            "imported_candidates sentinel lied: expected {expected:?}, observed {readback:?}"
        ))
    }
}

fn refuse(detail: String) -> ProbeOutcome {
    ProbeOutcome::Refused {
        reason: ProbeRefusalReason::StorageImportedCandidatesUnreliable,
        structured: serde_json::json!({"adapter": "imported_candidates", "detail": detail}),
        detail,
    }
}

#[cfg(test)]
mod tests {
    //! Real-infrastructure integration tests: a REAL DuckDB file per test,
    //! driven through the imported-candidate ports (never mocked).

    use super::*;
    use crate::DuckDbStorageAdapter;
    use std::path::Path;

    const ANVIL: &str = "github:acme-tools/anvil";
    const PINNING: &str = "org.openlore.philosophy.dependency-pinning";

    fn on(day: u32) -> chrono::DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 10, day, 9, 0, 0).unwrap()
    }

    fn pinning(subject: &str, confidence: f64, evidence: &str) -> ImportedCandidate {
        ImportedCandidate {
            subject: subject.to_string(),
            predicate: "embodiesPhilosophy".to_string(),
            object: PINNING.to_string(),
            confidence,
            evidence: vec![evidence.to_string()],
        }
    }

    fn import_of(
        imported_at: chrono::DateTime<Utc>,
        candidate: ImportedCandidate,
    ) -> CandidateImport {
        CandidateImport {
            imported_at,
            candidates: vec![candidate],
        }
    }

    fn stored_rows(db: &Path) -> i64 {
        Connection::open(db)
            .unwrap()
            .query_row("SELECT count(*) FROM imported_candidates", [], |row| {
                row.get(0)
            })
            .unwrap()
    }

    #[test]
    fn a_reimport_keeps_both_rows_but_lists_the_candidate_once_from_the_later_import() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("openlore.duckdb");
        let storage = DuckDbStorageAdapter::open(&db).unwrap();
        let kept = storage.imported_candidate_adapter();

        // The later import is recorded FIRST, so neither insertion order nor
        // import_id alone can pick the winner: only imported_at can.
        let later = pinning(ANVIL, 0.9, "https://github.com/acme-tools/anvil/later");
        let earlier = pinning(
            "github:Acme-Tools/Anvil",
            0.4,
            "https://example.org/earlier",
        );
        kept.record_import(&import_of(on(3), later.clone()))
            .unwrap();
        kept.record_import(&import_of(on(1), earlier)).unwrap();

        let listed = kept.list(None).unwrap();
        let narrowed = kept.list(Some(&RepoSubjectKey::of(ANVIL))).unwrap();
        drop(kept);
        drop(storage);

        assert_eq!(stored_rows(&db), 2, "both imports stay stored");
        assert_eq!(
            narrowed, listed,
            "--subject narrows the same de-duplicated list"
        );
        assert_eq!(listed.len(), 1, "listed once: {listed:?}");
        assert_eq!(listed[0].imported_at, on(3));
        assert_eq!(listed[0].candidate, later);
    }

    #[test]
    fn on_equal_import_time_the_larger_import_id_wins() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("openlore.duckdb");
        let storage = DuckDbStorageAdapter::open(&db).unwrap();
        let kept = storage.imported_candidate_adapter();

        let first = pinning(ANVIL, 0.4, "https://example.org/first");
        let second = pinning(ANVIL, 0.9, "https://example.org/second");
        kept.record_import(&import_of(on(3), first)).unwrap();
        kept.record_import(&import_of(on(3), second.clone()))
            .unwrap();

        let listed = kept.list(None).unwrap();
        drop(kept);
        drop(storage);

        assert_eq!(stored_rows(&db), 2, "both imports stay stored");
        assert_eq!(listed.len(), 1, "listed once: {listed:?}");
        assert_eq!(
            listed[0].candidate, second,
            "the larger import_id wins the tie"
        );
    }

    fn schema_versions(db: &Path) -> Vec<i64> {
        let conn = Connection::open(db).unwrap();
        let mut stmt = conn
            .prepare("SELECT version FROM schema_version ORDER BY version")
            .unwrap();
        let versions = stmt
            .query_map([], |row| row.get(0))
            .unwrap()
            .map(Result::unwrap)
            .collect();
        versions
    }

    fn assert_refused_as_unreliable(outcome: ProbeOutcome) {
        match outcome {
            ProbeOutcome::Refused { reason, .. } => assert_eq!(
                reason,
                ProbeRefusalReason::StorageImportedCandidatesUnreliable
            ),
            other => panic!("expected a refusal, got {other:?}"),
        }
    }

    /// D29: a healthy store passes the probe and keeps nothing from it; a
    /// store that cannot keep an import, or reads back something else than
    /// the sentinel, is refused as StorageImportedCandidatesUnreliable.
    #[test]
    fn the_probe_passes_a_healthy_store_untouched_and_refuses_one_that_cannot_keep_an_import() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("openlore.duckdb");
        let storage = DuckDbStorageAdapter::open(&db).unwrap();
        let kept = storage.imported_candidate_adapter();

        assert!(matches!(kept.probe(), ProbeOutcome::Ok));
        assert_eq!(
            kept.list(None).unwrap(),
            Vec::new(),
            "the sentinel rolled back"
        );

        {
            let conn = storage.conn.lock().unwrap();
            conn.execute_batch("DROP TABLE imported_candidates")
                .unwrap();
        }
        assert_refused_as_unreliable(kept.probe());
    }

    #[test]
    fn a_sentinel_read_back_altered_is_refused() {
        let altered = ImportedCandidate {
            confidence: 0.5,
            ..sentinel_candidate()
        };
        assert!(matches!(
            judge_sentinel(&[sentinel_candidate()]),
            ProbeOutcome::Ok
        ));
        for readback in [
            vec![],
            vec![altered],
            vec![sentinel_candidate(), sentinel_candidate()],
        ] {
            assert_refused_as_unreliable(judge_sentinel(&readback));
        }
    }

    /// D29: an import is kept whole or not at all — a candidate that cannot
    /// be inserted rolls back the rows already inserted for that import, and
    /// earlier imports are untouched.
    #[test]
    fn a_failed_insert_rolls_the_whole_import_back() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("openlore.duckdb");
        let storage = DuckDbStorageAdapter::open(&db).unwrap();
        let kept = storage.imported_candidate_adapter();
        let earlier = pinning(ANVIL, 0.4, "https://example.org/earlier");
        kept.record_import(&import_of(on(1), earlier.clone()))
            .unwrap();
        {
            // Make a second row for one subject in one import impossible.
            let conn = storage.conn.lock().unwrap();
            conn.execute_batch(
                "CREATE UNIQUE INDEX one_row_per_import_subject \
                 ON imported_candidates (import_id, subject_key)",
            )
            .unwrap();
        }
        let failing = CandidateImport {
            imported_at: on(3),
            candidates: vec![
                pinning("github:jeffbailey/tidepool", 0.9, "https://example.org/a"),
                pinning("github:jeffbailey/tidepool", 0.8, "https://example.org/b"),
            ],
        };

        let refused = kept.record_import(&failing);

        assert!(
            matches!(refused, Err(ImportedCandidateError::Store(_))),
            "{refused:?}"
        );
        let listed = kept.list(None).unwrap();
        drop(kept);
        drop(storage);
        assert_eq!(stored_rows(&db), 1, "nothing of the failed import stays");
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].candidate, earlier);
    }

    /// D29: a store at v8 (no imported_candidates) gains the table on open,
    /// records v9 once, keeps what it held, and keeps imports.
    #[test]
    fn opening_a_v8_store_migrates_it_to_v9_and_keeps_what_it_held() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("openlore.duckdb");
        {
            let storage = DuckDbStorageAdapter::open(&db).unwrap();
            let conn = storage.conn.lock().unwrap();
            // Wind the fresh store back to v8.
            conn.execute_batch(
                "DROP TABLE imported_candidates; \
                 DROP SEQUENCE imported_candidate_ids; \
                 DROP SEQUENCE candidate_import_ids; \
                 DELETE FROM schema_version WHERE version = 9;",
            )
            .unwrap();
        }
        let held_before = |db: &Path| -> i64 {
            Connection::open(db)
                .unwrap()
                .query_row("SELECT count(*) FROM schema_version", [], |row| row.get(0))
                .unwrap()
        };
        assert!(!schema_versions(&db).contains(&9), "wound back to v8");
        let versions_at_v8 = held_before(&db);

        let storage = DuckDbStorageAdapter::open(&db).unwrap();
        let kept = storage.imported_candidate_adapter();
        let candidate = pinning(ANVIL, 0.9, "https://example.org/after");
        kept.record_import(&import_of(on(2), candidate.clone()))
            .unwrap();
        let listed = kept.list(None).unwrap();
        drop(kept);
        drop(storage);
        DuckDbStorageAdapter::open(&db).unwrap();

        let versions = schema_versions(&db);
        assert_eq!(
            versions.iter().filter(|v| **v == 9).count(),
            1,
            "v9 recorded once, also on reopen: {versions:?}"
        );
        assert_eq!(held_before(&db), versions_at_v8 + 1, "older versions kept");
        assert!(versions.contains(&8), "{versions:?}");
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].candidate, candidate);
    }
}
