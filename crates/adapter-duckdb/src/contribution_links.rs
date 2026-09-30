//! `DuckDbContributionLinkAdapter` — the append-only `ContributionLinkPort`
//! over the SHARED DuckDB connection (contributor-philosophy-inference
//! DDD-4 / DDD-5 / DDD-15; `DuckDbPeerStorageAdapter` precedent).
//!
//! `record_snapshot` = ONE transaction of `INSERT … ON CONFLICT (repo_key,
//! person_key) DO UPDATE` refreshing user id / rank / contributions /
//! `last_observed_at` and NEVER touching `first_observed_at`. No statement in
//! this module deletes a link.
//!
//! The LIVE probe runs the same upsert twice on a sentinel inside a
//! transaction that is always rolled back, and refuses startup if the store
//! lies (rank / last-observed not refreshed, or first-observed altered).

use crate::conn::{ConnGuard, SharedConn};

use chrono::{DateTime, TimeZone, Utc};
use duckdb::{Connection, Transaction};
use ports::{
    ContributionLink, ContributionLinkError, ContributionLinkPort, LinkFilter, ProbeOutcome,
    ProbeRefusalReason, RankedContributor, RecordSnapshotOutcome,
};

/// The upsert every snapshot row (and the probe sentinel) goes through.
const UPSERT_LINK_SQL: &str = "
    INSERT INTO contribution_links (
        repo_key, person_key, repo_subject, person_subject, github_user_id,
        rank, contributions, first_observed_at, last_observed_at
    ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
    ON CONFLICT (repo_key, person_key) DO UPDATE SET
        repo_subject     = excluded.repo_subject,
        person_subject   = excluded.person_subject,
        github_user_id   = excluded.github_user_id,
        rank             = excluded.rank,
        contributions    = excluded.contributions,
        last_observed_at = excluded.last_observed_at";

const SELECT_LINKS_SQL: &str = "
    SELECT repo_subject, person_subject, github_user_id, rank, contributions,
           first_observed_at, last_observed_at
    FROM contribution_links";

/// Append-only link storage sharing the one DuckDB connection.
pub struct DuckDbContributionLinkAdapter {
    conn: SharedConn,
}

impl DuckDbContributionLinkAdapter {
    /// Construct from the SHARED handle (see
    /// `DuckDbStorageAdapter::contribution_link_adapter`). Opens no second
    /// handle and runs no migration (v5 ran at `open`).
    pub(crate) fn from_shared(conn: SharedConn) -> Self {
        Self { conn }
    }

    fn lock(&self) -> Result<ConnGuard<'_>, ContributionLinkError> {
        self.conn
            .lock()
            .map_err(|lock_err| ContributionLinkError::Store(lock_err.to_string()))
    }
}

/// The case-folded comparison key of a `github:` subject.
fn subject_key(subject: &str) -> String {
    subject.to_ascii_lowercase()
}

fn store_error(step: &str) -> impl Fn(duckdb::Error) -> ContributionLinkError + '_ {
    move |err| ContributionLinkError::Store(format!("{step}: {err}"))
}

fn upsert_link(
    tx: &Transaction<'_>,
    repo_subject: &str,
    observed_at: DateTime<Utc>,
    person: &RankedContributor,
) -> Result<(), duckdb::Error> {
    let person_subject = person.person_subject();
    tx.execute(
        UPSERT_LINK_SQL,
        duckdb::params![
            subject_key(repo_subject),
            subject_key(&person_subject),
            repo_subject,
            person_subject,
            i64::try_from(person.github_user_id).unwrap_or(i64::MAX),
            i64::from(person.rank),
            i64::try_from(person.contributions).unwrap_or(i64::MAX),
            observed_at,
            observed_at,
        ],
    )
    .map(|_| ())
}

fn decode_link(row: &duckdb::Row<'_>) -> Result<ContributionLink, duckdb::Error> {
    let user_id: i64 = row.get(2)?;
    let rank: i64 = row.get(3)?;
    let contributions: i64 = row.get(4)?;
    Ok(ContributionLink {
        repo_subject: row.get(0)?,
        person_subject: row.get(1)?,
        github_user_id: u64::try_from(user_id).unwrap_or_default(),
        rank: u32::try_from(rank).unwrap_or_default(),
        contributions: u64::try_from(contributions).unwrap_or_default(),
        first_observed_at: row.get(5)?,
        last_observed_at: row.get(6)?,
    })
}

impl ContributionLinkPort for DuckDbContributionLinkAdapter {
    fn probe(&self) -> ProbeOutcome {
        match self.lock() {
            Ok(mut conn) => probe_upsert_twice(&mut conn),
            Err(err) => refuse(err.to_string()),
        }
    }

    fn record_snapshot(
        &self,
        repo_subject: &str,
        observed_at: DateTime<Utc>,
        people: &[RankedContributor],
    ) -> Result<RecordSnapshotOutcome, ContributionLinkError> {
        let mut conn = self.lock()?;
        let tx = conn
            .transaction()
            .map_err(store_error("begin snapshot tx"))?;
        for person in people {
            upsert_link(&tx, repo_subject, observed_at, person)
                .map_err(store_error("upsert contribution link"))?;
        }
        tx.commit().map_err(store_error("commit snapshot tx"))?;
        Ok(RecordSnapshotOutcome {
            recorded: people.len(),
        })
    }

    fn list_links(
        &self,
        filter: &LinkFilter,
    ) -> Result<Vec<ContributionLink>, ContributionLinkError> {
        let conn = self.lock()?;
        let (sql, params): (String, Vec<String>) = match filter {
            LinkFilter::All => (
                format!("{SELECT_LINKS_SQL} ORDER BY repo_key, rank, person_key"),
                Vec::new(),
            ),
            LinkFilter::Person(subject) => (
                format!("{SELECT_LINKS_SQL} WHERE person_key = ? ORDER BY repo_key, rank"),
                vec![subject_key(subject)],
            ),
        };
        let mut stmt = conn
            .prepare(&sql)
            .map_err(store_error("prepare list_links"))?;
        let rows = stmt
            .query_map(duckdb::params_from_iter(params), decode_link)
            .map_err(store_error("query list_links"))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(store_error("decode contribution link"))
    }
}

// -----------------------------------------------------------------------------
// Live probe — upsert-twice sentinel inside an always-rolled-back tx (DDD-15)
// -----------------------------------------------------------------------------

const SENTINEL_REPO: &str = "github:openlore-probe/sentinel";

/// What the store reports for the sentinel after the second upsert.
#[derive(Debug, PartialEq, Eq)]
struct SentinelReadback {
    rows: i64,
    rank: i64,
    first_observed_at: DateTime<Utc>,
    last_observed_at: DateTime<Utc>,
}

fn probe_upsert_twice(conn: &mut Connection) -> ProbeOutcome {
    match upsert_sentinel_twice(conn) {
        Ok(readback) => judge_sentinel(&readback),
        Err(err) => refuse(format!("contribution_links sentinel upsert failed: {err}")),
    }
}

fn sentinel_instants() -> (DateTime<Utc>, DateTime<Utc>) {
    let first = Utc.with_ymd_and_hms(2000, 1, 1, 0, 0, 0).single();
    let second = Utc.with_ymd_and_hms(2000, 1, 2, 0, 0, 0).single();
    (first.unwrap_or_default(), second.unwrap_or_default())
}

fn sentinel_person(rank: u32) -> RankedContributor {
    RankedContributor {
        login: "openlore-probe".to_string(),
        github_user_id: 1,
        rank,
        contributions: u64::from(rank),
    }
}

/// Upsert the sentinel at rank 2 then rank 1 inside a transaction that is
/// dropped WITHOUT commit (DuckDB rolls it back) — the store is untouched.
fn upsert_sentinel_twice(conn: &mut Connection) -> Result<SentinelReadback, duckdb::Error> {
    let (first, second) = sentinel_instants();
    let tx = conn.transaction()?;
    upsert_link(&tx, SENTINEL_REPO, first, &sentinel_person(2))?;
    upsert_link(&tx, SENTINEL_REPO, second, &sentinel_person(1))?;
    let readback = tx.query_row(
        "SELECT COUNT(*), MAX(rank), MIN(first_observed_at), MAX(last_observed_at) \
         FROM contribution_links WHERE repo_key = ?",
        duckdb::params![subject_key(SENTINEL_REPO)],
        |row| {
            Ok(SentinelReadback {
                rows: row.get(0)?,
                rank: row.get(1)?,
                first_observed_at: row.get(2)?,
                last_observed_at: row.get(3)?,
            })
        },
    )?;
    tx.rollback()?;
    Ok(readback)
}

/// Pure verdict over the sentinel readback.
fn judge_sentinel(readback: &SentinelReadback) -> ProbeOutcome {
    let (first, second) = sentinel_instants();
    let expected = SentinelReadback {
        rows: 1,
        rank: 1,
        first_observed_at: first,
        last_observed_at: second,
    };
    if *readback == expected {
        ProbeOutcome::Ok
    } else {
        refuse(format!(
            "contribution_links upsert-twice sentinel lied: expected {expected:?}, observed {readback:?}"
        ))
    }
}

fn refuse(detail: String) -> ProbeOutcome {
    ProbeOutcome::Refused {
        reason: ProbeRefusalReason::StorageContributionLinkUpsertUnreliable,
        structured: serde_json::json!({"adapter": "contribution_links", "detail": detail}),
        detail,
    }
}

#[cfg(test)]
mod tests {
    //! Real-infrastructure integration tests: a REAL DuckDB file per test,
    //! driven through `ContributionLinkPort` (never mocked).

    use super::*;
    use crate::{schema, DuckDbStorageAdapter};
    use std::path::Path;

    const RIPGREP: &str = "github:BurntSushi/ripgrep";

    fn at(day: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 9, day, 9, 0, 0).unwrap()
    }

    fn person(login: &str, id: u64, rank: u32) -> RankedContributor {
        RankedContributor {
            login: login.to_string(),
            github_user_id: id,
            rank,
            contributions: 1_000 - u64::from(rank),
        }
    }

    fn raw_sql(db: &Path, sql: &str) {
        let conn = Connection::open(db).unwrap();
        conn.execute_batch(sql).unwrap();
    }

    fn schema_version(db: &Path) -> i32 {
        schema::read_version(&Connection::open(db).unwrap()).unwrap()
    }

    #[test]
    fn a_rescrape_refreshes_rank_but_never_deletes_nor_touches_first_observed() {
        let dir = tempfile::tempdir().unwrap();
        let storage = DuckDbStorageAdapter::open(&dir.path().join("openlore.duckdb")).unwrap();
        let links = storage.contribution_link_adapter();

        links
            .record_snapshot(
                RIPGREP,
                at(1),
                &[person("BurntSushi", 456_674, 1), person("dev-a", 2, 2)],
            )
            .unwrap();
        // Second snapshot: dev-a dropped out, BurntSushi re-ranked, new dev-b.
        let outcome = links
            .record_snapshot(
                "github:burntsushi/RIPGREP",
                at(20),
                &[person("dev-b", 3, 1), person("BurntSushi", 456_674, 2)],
            )
            .unwrap();
        assert_eq!(outcome.recorded, 2);

        let all = links.list_links(&LinkFilter::All).unwrap();
        let by_login = |login: &str| {
            all.iter()
                .find(|l| l.person_subject == format!("github:{login}"))
                .unwrap_or_else(|| panic!("{login} must still be linked: {all:#?}"))
        };
        assert_eq!(all.len(), 3, "never lowers the row set: {all:#?}");
        assert_eq!(by_login("BurntSushi").rank, 2);
        assert_eq!(by_login("BurntSushi").first_observed_at, at(1));
        assert_eq!(by_login("BurntSushi").last_observed_at, at(20));
        assert_eq!(
            by_login("dev-a").last_observed_at,
            at(1),
            "stale, not deleted"
        );
        assert_eq!(by_login("dev-b").first_observed_at, at(20));
        assert_eq!(
            links
                .list_links(&LinkFilter::Person("github:burntsushi".to_string()))
                .unwrap()
                .len(),
            1
        );
    }

    #[test]
    fn migration_v5_upgrades_a_v4_store_once_and_the_probe_passes() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("openlore.duckdb");
        drop(DuckDbStorageAdapter::open(&db).unwrap());
        // Turn it back into a v4 store.
        raw_sql(
            &db,
            "DROP TABLE contribution_links; DELETE FROM schema_version WHERE version = 5;",
        );
        assert_eq!(schema_version(&db), 4);

        drop(DuckDbStorageAdapter::open(&db).unwrap());
        let storage = DuckDbStorageAdapter::open(&db).unwrap();
        assert!(matches!(
            ports::StoragePort::probe(&storage),
            ProbeOutcome::Ok
        ));
        assert!(matches!(
            storage.contribution_link_adapter().probe(),
            ProbeOutcome::Ok
        ));
        assert!(
            storage
                .contribution_link_adapter()
                .list_links(&LinkFilter::All)
                .unwrap()
                .is_empty(),
            "the probe sentinel is rolled back"
        );
        drop(storage);
        let v5_rows: i64 = Connection::open(&db)
            .unwrap()
            .query_row(
                "SELECT COUNT(*) FROM schema_version WHERE version = 5",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!((schema_version(&db), v5_rows), (5, 1));
    }

    #[test]
    fn a_store_whose_upsert_lies_is_refused_by_the_probe() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("openlore.duckdb");
        drop(DuckDbStorageAdapter::open(&db).unwrap());
        // The lie: the table lost its conflict key, so the upsert cannot refresh.
        raw_sql(
            &db,
            "DROP TABLE contribution_links;
             CREATE TABLE contribution_links (
                repo_key VARCHAR, person_key VARCHAR, repo_subject VARCHAR,
                person_subject VARCHAR, github_user_id BIGINT, rank INTEGER,
                contributions BIGINT, first_observed_at TIMESTAMP,
                last_observed_at TIMESTAMP);",
        );
        let storage = DuckDbStorageAdapter::open(&db).unwrap();
        match storage.contribution_link_adapter().probe() {
            ProbeOutcome::Refused { reason, .. } => assert_eq!(
                reason,
                ProbeRefusalReason::StorageContributionLinkUpsertUnreliable
            ),
            ProbeOutcome::Ok => panic!("a lying upsert must be refused"),
        }
    }

    #[test]
    fn the_sentinel_verdict_refuses_an_unrefreshed_rank_or_altered_first_observed() {
        let (first, second) = sentinel_instants();
        let honest = SentinelReadback {
            rows: 1,
            rank: 1,
            first_observed_at: first,
            last_observed_at: second,
        };
        assert!(matches!(judge_sentinel(&honest), ProbeOutcome::Ok));
        for lie in [
            SentinelReadback { rank: 2, ..honest },
            SentinelReadback {
                first_observed_at: second,
                ..honest
            },
            SentinelReadback {
                last_observed_at: first,
                ..honest
            },
        ] {
            assert!(
                matches!(judge_sentinel(&lie), ProbeOutcome::Refused { .. }),
                "{lie:?}"
            );
        }
    }
}
