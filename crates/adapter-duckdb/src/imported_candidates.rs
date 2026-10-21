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

const KEPT_ORDER: &str = "ORDER BY subject_key, predicate, object, imported_at, import_id";

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
            let sql = format!("{KEPT_COLUMNS} WHERE subject_key = ? {KEPT_ORDER}");
            let mut stmt = conn.prepare(&sql)?;
            let rows = stmt.query_map(duckdb::params![key.as_str()], decode_kept)?;
            rows.collect()
        }
        None => {
            let sql = format!("{KEPT_COLUMNS} {KEPT_ORDER}");
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
