//! `DuckDbIdentityLinkCheckAdapter` — the insert-only local proof-check
//! history (`identity_link_checks`, migration v7) and the link-candidate claim
//! read, over the SHARED DuckDB connection (ADR-085 §7).
//!
//! No statement in this module updates or deletes a check: `record_check` is
//! one `INSERT`, and "latest" is a window query over the history. The only
//! DELETE of the table belongs to the purge adapter (`identity_link_purge.rs`).
//!
//! The LIVE probe records two checks of one sentinel link at two instants
//! inside a transaction that is always rolled back, and refuses startup unless
//! both are kept and the later one reads back as latest (ADR-085 Earned Trust).

use crate::conn::{ConnGuard, SharedConn};

use chrono::{DateTime, TimeZone, Utc};
use claim_domain::identity_link::CheckVerdict;
use claim_domain::{bare_did, Did, Provenance};
use duckdb::Connection;
use ports::{
    CheckFilter, IdentityLinkCheckError, IdentityLinkCheckReadPort, IdentityLinkCheckWritePort,
    LinkCandidate, LinkCandidateReadPort, LinkCheck, PeerClaimProvenance, ProbeOutcome,
    ProbeRefusalReason,
};

const INSERT_CHECK_SQL: &str = "
    INSERT INTO identity_link_checks (
        subject_did, github_user_id, login_seen, verdict, checked_at
    ) VALUES (?, ?, ?, ?, ?)";

/// The latest non-future check per link; the subject filter is optional
/// (`?` = NULL selects every subject).
const LATEST_CHECKS_SQL: &str = "
    SELECT subject_did, github_user_id, login_seen, verdict, checked_at
    FROM (
        SELECT subject_did, github_user_id, login_seen, verdict, checked_at,
               row_number() OVER (
                   PARTITION BY subject_did, github_user_id
                   ORDER BY checked_at DESC, check_id DESC
               ) AS recency
        FROM identity_link_checks
        WHERE checked_at <= ? AND (CAST(? AS VARCHAR) IS NULL OR subject_did = ?)
    )
    WHERE recency = 1
    ORDER BY subject_did, github_user_id";

/// Every link candidate any reader sees (list, verify, person views): Jeff's
/// own links plus the links cached from peers he has not removed (U-19). A
/// soft-removed peer keeps its cached rows (ADR-057) but contributes no
/// candidate; a re-added peer's links are candidates again.
const LINK_CANDIDATES_SQL: &str = "
    SELECT cid, author_did, subject, predicate, object, 'app-signed' AS provenance
    FROM claims WHERE predicate = ?
    UNION ALL
    SELECT cid, author_did, subject, predicate, object, COALESCE(provenance, 'app-signed')
    FROM peer_claims
    WHERE predicate = ?
      AND NOT EXISTS (
          SELECT 1 FROM peer_subscriptions
          WHERE peer_subscriptions.peer_did = peer_claims.author_did
            AND peer_subscriptions.removed_at IS NOT NULL
      )
    ORDER BY cid";

/// The evidence of one candidate, in signed order (own or peer claim).
const CANDIDATE_EVIDENCE_SQL: &str = "
    SELECT evidence, ordinal FROM claim_evidence WHERE cid = ?
    UNION ALL
    SELECT evidence, ordinal FROM peer_claim_evidence WHERE cid = ?
    ORDER BY ordinal";

/// Check history + link-candidate reads sharing the one DuckDB connection.
pub struct DuckDbIdentityLinkCheckAdapter {
    conn: SharedConn,
}

impl DuckDbIdentityLinkCheckAdapter {
    /// Construct from the SHARED handle (see
    /// `DuckDbStorageAdapter::identity_link_check_adapter`). Opens no second
    /// handle and runs no migration (v7 ran at `open`).
    pub(crate) fn from_shared(conn: SharedConn) -> Self {
        Self { conn }
    }

    fn lock(&self) -> Result<ConnGuard<'_>, IdentityLinkCheckError> {
        self.conn
            .lock()
            .map_err(|lock_err| IdentityLinkCheckError::Store(lock_err.to_string()))
    }
}

fn store_error(step: &str) -> impl Fn(duckdb::Error) -> IdentityLinkCheckError + '_ {
    move |err| IdentityLinkCheckError::Store(format!("{step}: {err}"))
}

fn insert_check(conn: &Connection, check: &LinkCheck) -> Result<(), duckdb::Error> {
    conn.execute(
        INSERT_CHECK_SQL,
        duckdb::params![
            check.subject_did,
            i64::try_from(check.github_user_id).unwrap_or(i64::MAX),
            check.login_seen,
            check.verdict.token(),
            check.checked_at,
        ],
    )
    .map(|_| ())
}

fn decode_check(row: &duckdb::Row<'_>) -> Result<LinkCheck, duckdb::Error> {
    let user_id: i64 = row.get(1)?;
    let token: String = row.get(3)?;
    let verdict = CheckVerdict::from_token(&token).ok_or_else(|| {
        duckdb::Error::FromSqlConversionFailure(
            3,
            duckdb::types::Type::Text,
            format!("unknown check verdict `{token}`").into(),
        )
    })?;
    Ok(LinkCheck {
        subject_did: row.get(0)?,
        github_user_id: u64::try_from(user_id).unwrap_or_default(),
        login_seen: row.get(2)?,
        verdict,
        checked_at: row.get(4)?,
    })
}

fn query_latest_checks(
    conn: &Connection,
    filter: &CheckFilter,
    now: DateTime<Utc>,
) -> Result<Vec<LinkCheck>, duckdb::Error> {
    let subject = match filter {
        CheckFilter::All => None,
        CheckFilter::Subject(did) => Some(did.as_str()),
    };
    let mut stmt = conn.prepare(LATEST_CHECKS_SQL)?;
    let rows = stmt.query_map(duckdb::params![now, subject, subject], decode_check)?;
    rows.collect()
}

/// The provenance a stored row passed under: app-signed rows were verified
/// against their author's key when written or pulled; self-attested rows
/// were fetched from their author's own repo (ADR-071).
fn stored_provenance(author_did: &str, column: &str) -> Provenance {
    match PeerClaimProvenance::from_column(column) {
        PeerClaimProvenance::AppSigned => Provenance::AppSigned {
            key_id: author_did.to_string(),
        },
        PeerClaimProvenance::SelfAttested => Provenance::SelfAttested {
            repo_did: Did(bare_did(author_did).to_string()),
        },
    }
}

fn decode_candidate(row: &duckdb::Row<'_>) -> Result<LinkCandidate, duckdb::Error> {
    let author_did: String = row.get(1)?;
    let provenance = stored_provenance(&author_did, &row.get::<_, String>(5)?);
    Ok(LinkCandidate {
        cid: row.get(0)?,
        author_did,
        subject: row.get(2)?,
        predicate: row.get(3)?,
        object: row.get(4)?,
        evidence: Vec::new(),
        provenance,
    })
}

fn candidate_evidence(conn: &Connection, cid: &str) -> Result<Vec<String>, duckdb::Error> {
    let mut stmt = conn.prepare(CANDIDATE_EVIDENCE_SQL)?;
    let rows = stmt.query_map(duckdb::params![cid, cid], |row| row.get(0))?;
    rows.collect()
}

fn with_evidence(
    conn: &Connection,
    candidate: LinkCandidate,
) -> Result<LinkCandidate, duckdb::Error> {
    Ok(LinkCandidate {
        evidence: candidate_evidence(conn, &candidate.cid)?,
        ..candidate
    })
}

impl IdentityLinkCheckReadPort for DuckDbIdentityLinkCheckAdapter {
    fn probe(&self) -> ProbeOutcome {
        match self.lock() {
            Ok(mut conn) => probe_two_checks_kept(&mut conn),
            Err(err) => refuse(err.to_string()),
        }
    }

    fn latest_checks(
        &self,
        filter: &CheckFilter,
        now: DateTime<Utc>,
    ) -> Result<Vec<LinkCheck>, IdentityLinkCheckError> {
        let conn = self.lock()?;
        query_latest_checks(&conn, filter, now).map_err(store_error("query latest checks"))
    }
}

impl IdentityLinkCheckWritePort for DuckDbIdentityLinkCheckAdapter {
    fn record_check(&self, check: &LinkCheck) -> Result<(), IdentityLinkCheckError> {
        let conn = self.lock()?;
        insert_check(&conn, check).map_err(store_error("record check"))
    }
}

impl LinkCandidateReadPort for DuckDbIdentityLinkCheckAdapter {
    fn link_candidates(
        &self,
        predicate: &str,
    ) -> Result<Vec<LinkCandidate>, IdentityLinkCheckError> {
        let conn = self.lock()?;
        let mut stmt = conn
            .prepare(LINK_CANDIDATES_SQL)
            .map_err(store_error("prepare link candidates"))?;
        let rows = stmt
            .query_map(duckdb::params![predicate, predicate], decode_candidate)
            .map_err(store_error("query link candidates"))?;
        let candidates = rows
            .collect::<Result<Vec<_>, _>>()
            .map_err(store_error("decode link candidate"))?;
        candidates
            .into_iter()
            .map(|candidate| with_evidence(&conn, candidate))
            .collect::<Result<Vec<_>, _>>()
            .map_err(store_error("read link candidate evidence"))
    }
}

// -----------------------------------------------------------------------------
// Live probe — two checks of one sentinel link, always rolled back
// -----------------------------------------------------------------------------

const SENTINEL_DID: &str = "did:plc:openlore-probe-sentinel";

fn sentinel_check(day: u32, verdict: CheckVerdict) -> LinkCheck {
    let checked_at = Utc
        .with_ymd_and_hms(2000, 1, day, 0, 0, 0)
        .single()
        .unwrap_or_default();
    LinkCheck {
        subject_did: SENTINEL_DID.to_string(),
        github_user_id: 1,
        login_seen: "openlore-probe".to_string(),
        verdict,
        checked_at,
    }
}

fn sentinel_checks() -> (LinkCheck, LinkCheck) {
    (
        sentinel_check(1, CheckVerdict::DidMissing),
        sentinel_check(2, CheckVerdict::Verified),
    )
}

/// What the store reports for the sentinel link after both inserts.
#[derive(Debug, PartialEq, Eq)]
struct SentinelReadback {
    kept: i64,
    latest: Vec<LinkCheck>,
}

fn probe_two_checks_kept(conn: &mut Connection) -> ProbeOutcome {
    match record_sentinel_twice(conn) {
        Ok(readback) => judge_sentinel(&readback),
        Err(err) => refuse(format!(
            "identity_link_checks could not keep two checks of one link: {err}"
        )),
    }
}

/// Insert both sentinel checks inside a transaction that is rolled back —
/// the store is untouched.
fn record_sentinel_twice(conn: &mut Connection) -> Result<SentinelReadback, duckdb::Error> {
    let (earlier, later) = sentinel_checks();
    let tx = conn.transaction()?;
    insert_check(&tx, &earlier)?;
    insert_check(&tx, &later)?;
    let kept = tx.query_row(
        "SELECT COUNT(*) FROM identity_link_checks WHERE subject_did = ?",
        duckdb::params![SENTINEL_DID],
        |row| row.get(0),
    )?;
    let latest = query_latest_checks(
        &tx,
        &CheckFilter::Subject(SENTINEL_DID.to_string()),
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
            "identity_link_checks sentinel lied: expected {expected:?}, observed {readback:?}"
        ))
    }
}

fn refuse(detail: String) -> ProbeOutcome {
    ProbeOutcome::Refused {
        reason: ProbeRefusalReason::StorageIdentityLinkCheckHistoryUnreliable,
        structured: serde_json::json!({"adapter": "identity_link_checks", "detail": detail}),
        detail,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sentinel_verdict_refuses_an_overwrite_or_a_wrong_latest() {
        let (earlier, later) = sentinel_checks();
        let honest = SentinelReadback {
            kept: 2,
            latest: vec![later.clone()],
        };
        assert!(matches!(judge_sentinel(&honest), ProbeOutcome::Ok));
        for lie in [
            SentinelReadback {
                kept: 1,
                latest: vec![later],
            },
            SentinelReadback {
                kept: 2,
                latest: vec![earlier],
            },
            SentinelReadback {
                kept: 2,
                latest: Vec::new(),
            },
        ] {
            assert!(
                matches!(judge_sentinel(&lie), ProbeOutcome::Refused { .. }),
                "{lie:?}"
            );
        }
    }
}
