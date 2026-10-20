//! `DuckDbRepoAuthorityCheckAdapter` — the insert-only local history of repo
//! authority observations (`repo_authority_checks`, migration v8, ADR-087 §5)
//! over the SHARED DuckDB connection.
//!
//! No statement in this module rewrites an observation: `record` is one
//! `INSERT`, and "latest" is a window query over the history (max
//! `checked_at`, then max `check_id`).
//!
//! The LIVE probe records two observations of one sentinel repo at two
//! instants inside a transaction that is always rolled back, and refuses
//! startup unless both are kept and the later one reads back as latest
//! (ADR-087 Earned Trust).

use crate::conn::{ConnGuard, SharedConn};

use chrono::{DateTime, TimeZone, Utc};
use claim_domain::authority::RepoOwnerKind;
use duckdb::Connection;
use ports::{
    AuthorityFile, FoundRepo, ProbeOutcome, ProbeRefusalReason, RepoAuthorityCheck,
    RepoAuthorityCheckError, RepoAuthorityCheckReadPort, RepoAuthorityCheckWritePort,
    RepoObservation, RepoSubjectKey,
};

/// The listed DIDs travel as one space-joined parameter (a DID holds no
/// space); an empty join stores an empty list, a NULL stores none.
const INSERT_CHECK_SQL: &str = "
    INSERT INTO repo_authority_checks (
        subject_key, outcome, repo_id, full_name_seen, owner_id, owner_type,
        authority_file, authority_dids, checked_at
    ) VALUES (
        ?, ?, ?, ?, ?, ?, ?,
        CASE WHEN CAST(? AS VARCHAR) = '' THEN []::VARCHAR[] ELSE string_split(?, ' ') END,
        ?
    )";

/// The latest observation of one subject not dated after `now`.
const LATEST_CHECK_SQL: &str = "
    SELECT subject_key, outcome, repo_id, full_name_seen, owner_id, owner_type,
           authority_file, array_to_string(COALESCE(authority_dids, []::VARCHAR[]), ' '),
           checked_at
    FROM repo_authority_checks
    WHERE subject_key = ? AND checked_at <= ?
    ORDER BY checked_at DESC, check_id DESC
    LIMIT 1";

/// Repo observation history sharing the one DuckDB connection.
pub struct DuckDbRepoAuthorityCheckAdapter {
    conn: SharedConn,
}

impl DuckDbRepoAuthorityCheckAdapter {
    /// Construct from the SHARED handle (see
    /// `DuckDbStorageAdapter::repo_authority_check_adapter`). Opens no second
    /// handle and runs no migration (v8 ran at `open`).
    pub(crate) fn from_shared(conn: SharedConn) -> Self {
        Self { conn }
    }

    fn lock(&self) -> Result<ConnGuard<'_>, RepoAuthorityCheckError> {
        self.conn
            .lock()
            .map_err(|lock_err| RepoAuthorityCheckError::Store(lock_err.to_string()))
    }
}

fn store_error(step: &str) -> impl Fn(duckdb::Error) -> RepoAuthorityCheckError + '_ {
    move |err| RepoAuthorityCheckError::Store(format!("{step}: {err}"))
}

// -----------------------------------------------------------------------------
// Column tokens
// -----------------------------------------------------------------------------

const FOUND: &str = "found";
const REPO_NOT_FOUND: &str = "repo_not_found";

fn owner_type_token(kind: RepoOwnerKind) -> &'static str {
    match kind {
        RepoOwnerKind::User => "user",
        RepoOwnerKind::Organization => "organization",
    }
}

fn owner_type_from_token(token: &str) -> Option<RepoOwnerKind> {
    match token {
        "user" => Some(RepoOwnerKind::User),
        "organization" => Some(RepoOwnerKind::Organization),
        _ => None,
    }
}

fn authority_file_token(file: &AuthorityFile) -> &'static str {
    match file {
        AuthorityFile::Absent => "absent",
        AuthorityFile::Present(_) => "present",
        AuthorityFile::Invalid => "invalid",
    }
}

fn authority_file_from_columns(token: &str, joined_dids: &str) -> Option<AuthorityFile> {
    match token {
        "absent" => Some(AuthorityFile::Absent),
        "present" => Some(AuthorityFile::Present(
            joined_dids.split_whitespace().map(str::to_string).collect(),
        )),
        "invalid" => Some(AuthorityFile::Invalid),
        _ => None,
    }
}

fn listed_dids(file: &AuthorityFile) -> String {
    match file {
        AuthorityFile::Present(dids) => dids.join(" "),
        AuthorityFile::Absent | AuthorityFile::Invalid => String::new(),
    }
}

// -----------------------------------------------------------------------------
// Insert and decode
// -----------------------------------------------------------------------------

/// The flat column values of one observation, `None` where the row is NULL.
struct ObservationColumns<'a> {
    outcome: &'static str,
    repo_id: Option<u64>,
    full_name_seen: Option<&'a str>,
    owner_id: Option<u64>,
    owner_type: Option<&'static str>,
    authority_file: Option<&'static str>,
    authority_dids: Option<String>,
}

fn columns_of(observation: &RepoObservation) -> ObservationColumns<'_> {
    match observation {
        RepoObservation::Found(repo) => ObservationColumns {
            outcome: FOUND,
            repo_id: Some(repo.repo_id),
            full_name_seen: Some(&repo.full_name_seen),
            owner_id: Some(repo.owner_id),
            owner_type: Some(owner_type_token(repo.owner_type)),
            authority_file: Some(authority_file_token(&repo.authority_file)),
            authority_dids: Some(listed_dids(&repo.authority_file)),
        },
        RepoObservation::RepoNotFound => ObservationColumns {
            outcome: REPO_NOT_FOUND,
            repo_id: None,
            full_name_seen: None,
            owner_id: None,
            owner_type: None,
            authority_file: None,
            authority_dids: None,
        },
    }
}

fn insert_check(conn: &Connection, check: &RepoAuthorityCheck) -> Result<(), duckdb::Error> {
    let columns = columns_of(&check.observation);
    conn.execute(
        INSERT_CHECK_SQL,
        duckdb::params![
            check.subject_key.as_str(),
            columns.outcome,
            columns.repo_id,
            columns.full_name_seen,
            columns.owner_id,
            columns.owner_type,
            columns.authority_file,
            columns.authority_dids,
            columns.authority_dids,
            check.checked_at,
        ],
    )
    .map(|_| ())
}

fn conversion_failure(column: usize, message: String) -> duckdb::Error {
    duckdb::Error::FromSqlConversionFailure(column, duckdb::types::Type::Text, message.into())
}

fn decode_found(row: &duckdb::Row<'_>) -> Result<FoundRepo, duckdb::Error> {
    let owner_token: String = row.get(5)?;
    let owner_type = owner_type_from_token(&owner_token)
        .ok_or_else(|| conversion_failure(5, format!("unknown owner type `{owner_token}`")))?;
    let file_token: String = row.get(6)?;
    let authority_file = authority_file_from_columns(&file_token, &row.get::<_, String>(7)?)
        .ok_or_else(|| conversion_failure(6, format!("unknown authority file `{file_token}`")))?;
    Ok(FoundRepo {
        repo_id: row.get(2)?,
        full_name_seen: row.get(3)?,
        owner_id: row.get(4)?,
        owner_type,
        authority_file,
    })
}

fn decode_check(row: &duckdb::Row<'_>) -> Result<RepoAuthorityCheck, duckdb::Error> {
    let subject: String = row.get(0)?;
    let outcome: String = row.get(1)?;
    let observation = match outcome.as_str() {
        FOUND => RepoObservation::Found(decode_found(row)?),
        REPO_NOT_FOUND => RepoObservation::RepoNotFound,
        other => return Err(conversion_failure(1, format!("unknown outcome `{other}`"))),
    };
    Ok(RepoAuthorityCheck {
        subject_key: RepoSubjectKey::of(&subject),
        observation,
        checked_at: row.get(8)?,
    })
}

fn query_latest(
    conn: &Connection,
    subjects: &[RepoSubjectKey],
    now: DateTime<Utc>,
) -> Result<Vec<RepoAuthorityCheck>, duckdb::Error> {
    let mut ordered = subjects.to_vec();
    ordered.sort();
    ordered.dedup();
    let mut stmt = conn.prepare(LATEST_CHECK_SQL)?;
    let mut latest = Vec::new();
    for subject in &ordered {
        let rows = stmt.query_map(duckdb::params![subject.as_str(), now], decode_check)?;
        for row in rows {
            latest.push(row?);
        }
    }
    Ok(latest)
}

impl RepoAuthorityCheckReadPort for DuckDbRepoAuthorityCheckAdapter {
    fn probe(&self) -> ProbeOutcome {
        match self.lock() {
            Ok(mut conn) => probe_two_observations_kept(&mut conn),
            Err(err) => refuse(err.to_string()),
        }
    }

    fn latest(
        &self,
        subjects: &[RepoSubjectKey],
        now: DateTime<Utc>,
    ) -> Result<Vec<RepoAuthorityCheck>, RepoAuthorityCheckError> {
        let conn = self.lock()?;
        query_latest(&conn, subjects, now).map_err(store_error("query latest repo checks"))
    }
}

impl RepoAuthorityCheckWritePort for DuckDbRepoAuthorityCheckAdapter {
    fn record(&self, check: &RepoAuthorityCheck) -> Result<(), RepoAuthorityCheckError> {
        let conn = self.lock()?;
        insert_check(&conn, check).map_err(store_error("record repo check"))
    }
}

// -----------------------------------------------------------------------------
// Live probe — two observations of one sentinel repo, always rolled back
// -----------------------------------------------------------------------------

const SENTINEL_SUBJECT: &str = "github:openlore-probe/sentinel";
const SENTINEL_DID: &str = "did:plc:openlore-probe-sentinel";

fn sentinel_check(day: u32, observation: RepoObservation) -> RepoAuthorityCheck {
    let checked_at = Utc
        .with_ymd_and_hms(2000, 1, day, 0, 0, 0)
        .single()
        .unwrap_or_default();
    RepoAuthorityCheck {
        subject_key: RepoSubjectKey::of(SENTINEL_SUBJECT),
        observation,
        checked_at,
    }
}

/// The earlier observation finds no repo; the later one finds it listing the
/// sentinel DID — both row shapes the table CHECK admits.
fn sentinel_checks() -> (RepoAuthorityCheck, RepoAuthorityCheck) {
    (
        sentinel_check(1, RepoObservation::RepoNotFound),
        sentinel_check(
            2,
            RepoObservation::Found(FoundRepo {
                repo_id: 1,
                full_name_seen: "openlore-probe/sentinel".to_string(),
                owner_id: 1,
                owner_type: RepoOwnerKind::Organization,
                authority_file: AuthorityFile::Present(vec![SENTINEL_DID.to_string()]),
            }),
        ),
    )
}

/// What the store reports for the sentinel repo after both inserts.
#[derive(Debug, PartialEq, Eq)]
struct SentinelReadback {
    kept: i64,
    latest: Vec<RepoAuthorityCheck>,
}

fn probe_two_observations_kept(conn: &mut Connection) -> ProbeOutcome {
    match record_sentinel_twice(conn) {
        Ok(readback) => judge_sentinel(&readback),
        Err(err) => refuse(format!(
            "repo_authority_checks could not keep two observations of one repo: {err}"
        )),
    }
}

/// Insert both sentinel observations inside a transaction that is rolled
/// back — the store is untouched.
fn record_sentinel_twice(conn: &mut Connection) -> Result<SentinelReadback, duckdb::Error> {
    let (earlier, later) = sentinel_checks();
    let tx = conn.transaction()?;
    insert_check(&tx, &earlier)?;
    insert_check(&tx, &later)?;
    let kept = tx.query_row(
        "SELECT COUNT(*) FROM repo_authority_checks WHERE subject_key = ?",
        duckdb::params![earlier.subject_key.as_str()],
        |row| row.get(0),
    )?;
    let latest = query_latest(
        &tx,
        std::slice::from_ref(&later.subject_key),
        later.checked_at,
    )?;
    tx.rollback()?;
    Ok(SentinelReadback { kept, latest })
}

/// Pure verdict over the sentinel readback.
fn judge_sentinel(readback: &SentinelReadback) -> ProbeOutcome {
    let (_, later) = sentinel_checks();
    let expected = SentinelReadback {
        kept: 2,
        latest: vec![later],
    };
    if *readback == expected {
        ProbeOutcome::Ok
    } else {
        refuse(format!(
            "repo_authority_checks sentinel lied: expected {expected:?}, observed {readback:?}"
        ))
    }
}

fn refuse(detail: String) -> ProbeOutcome {
    ProbeOutcome::Refused {
        reason: ProbeRefusalReason::StorageRepoAuthorityCheckHistoryUnreliable,
        structured: serde_json::json!({"adapter": "repo_authority_checks", "detail": detail}),
        detail,
    }
}
