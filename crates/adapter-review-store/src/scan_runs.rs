//! `ScanRunPort`: the owner's scan runs (`scan_runs`, ADR-076 §3).

use duckdb::types::{TimeUnit, Value};
use duckdb::{params, OptionalExt};
use ports::{
    AccountCheckOutcome, AccountCheckRecord, KpiTally, OutsideDefault, PlanRowStatus,
    PlannedScanRepo, RepoSkipReason, RepoSource, ReviewStoreError, ScanCounts, ScanPlanRead,
    ScanPlanRow, ScanRun, ScanRunPort, ScanStatus, SkippedScanRepo,
};

use crate::secrets::port_error;
use crate::{db_error, db_id, in_transaction, ReviewStore, StoreError};

const START_SCAN: &str = "INSERT INTO scan_runs
    (owner_did, run_id, status, started_at) VALUES (?, ?, 'running', now())";

const FINISH_SCAN: &str = "UPDATE scan_runs
    SET status = ?, resume_after = to_timestamp(?), finished_at = now(),
        new_count = ?, published_count = ?, declined_count = ?
    WHERE owner_did = ? AND run_id = ?";

// ADR-088 §6: the frozen plan, one row per repo, written before the first read.
// An imported row has no account (`source_account_id` NULL).
const RECORD_PLAN_ROW: &str = "INSERT INTO scan_run_repos
    (owner_did, run_id, position, repo_id, full_name, source_kind, source_account_id, status,
     outside_default)
    VALUES (?, ?, ?, ?, ?, ?, ?, 'planned', ?)";

// Review R4: a run's rows are written only while the run itself is stored,
// so a forget-me purge mid-scan (which deletes the run) leaves none behind.
const RUN_STORED: &str = "SELECT EXISTS (SELECT 1 FROM scan_runs
    WHERE owner_did = ? AND run_id = ?)";

// A skipped chosen repo, after every row already recorded for the run.
const RECORD_SKIPPED_ROW: &str = "INSERT INTO scan_run_repos
    (owner_did, run_id, position, repo_id, full_name, source_kind, source_account_id, status,
     skip_reason)
    SELECT ?, ?, coalesce(max(position) + 1, 0), ?, ?, 'account', ?, 'skipped', ?
    FROM scan_run_repos WHERE owner_did = ? AND run_id = ?";

// The skipped rows of the owner's most recent run (the run `LATEST_SCAN` reads).
const LATEST_SKIPPED: &str = "SELECT repo_id, full_name, source_account_id, skip_reason
    FROM scan_run_repos
    WHERE owner_did = ? AND status = 'skipped' AND run_id = (
        SELECT run_id FROM scan_runs WHERE owner_did = ?
        ORDER BY started_at DESC, rowid DESC LIMIT 1)
    ORDER BY position";

// Every plan row of the owner's most recent run (the run `LATEST_SCAN` reads).
const LATEST_PLAN: &str = "SELECT repo_id, full_name, source_kind, source_account_id, status,
    outside_default
    FROM scan_run_repos
    WHERE owner_did = ? AND run_id = (
        SELECT run_id FROM scan_runs WHERE owner_did = ?
        ORDER BY started_at DESC, rowid DESC LIMIT 1)
    ORDER BY position";

// Each account's bio check in a run: `[{"account": id, "outcome": "passed"}]`
// (ids and outcomes only, design §6.2 step 7).
const RECORD_ACCOUNT_CHECKS: &str = "UPDATE scan_runs SET summary = ?
    WHERE owner_did = ? AND run_id = ?";

const LATEST_ACCOUNT_CHECKS: &str = "SELECT summary FROM scan_runs WHERE owner_did = ?
    ORDER BY started_at DESC, rowid DESC LIMIT 1";

const MARK_REPO_READ: &str = "UPDATE scan_run_repos SET status = 'read'
    WHERE owner_did = ? AND run_id = ? AND position = ?";

// Restart sweep, one owner at a time (every statement owner-scoped).
const OWNERS_WITH_RUNNING_SCANS: &str =
    "SELECT DISTINCT owner_did FROM scan_runs WHERE status = 'running'";
const OWNERS_WITH_PAUSED_SCANS: &str =
    "SELECT DISTINCT owner_did FROM scan_runs WHERE status = 'rate_limited' ORDER BY owner_did";
const INTERRUPT_OWNERS_RUNNING_SCANS: &str = "UPDATE scan_runs
    SET status = ?, finished_at = now()
    WHERE owner_did = ? AND status = 'running'";

// `resume_after` is read raw: the TIMESTAMPTZ overloads of `epoch` & co.
// live in the ICU extension, which the app never loads.
const LATEST_SCAN: &str = "SELECT status, resume_after, new_count, published_count, declined_count,
        (SELECT count(*) FROM scan_run_repos AS planned
         WHERE planned.owner_did = scan_runs.owner_did AND planned.run_id = scan_runs.run_id
           AND planned.status = 'read')
    FROM scan_runs WHERE owner_did = ?
    ORDER BY started_at DESC, rowid DESC LIMIT 1";

/// Whether the owner's run `run_id` is stored (R4: a purged one is not).
fn run_stored(
    conn: &duckdb::Connection,
    owner_did: &str,
    run_id: &str,
) -> Result<bool, StoreError> {
    conn.query_row(RUN_STORED, params![owner_did, run_id], |row| row.get(0))
        .map_err(db_error)
}

/// The owners a `SELECT DISTINCT owner_did …` names.
fn owners_where(conn: &duckdb::Connection, sql: &str) -> Result<Vec<String>, StoreError> {
    conn.prepare(sql)
        .and_then(|mut statement| {
            statement
                .query_map([], |row| row.get::<_, String>(0))?
                .collect()
        })
        .map_err(db_error)
}

/// Unix seconds of a stored timestamp (`NULL` → `None`).
pub(crate) fn unix_secs_of(stored: &Value) -> Option<i64> {
    let Value::Timestamp(unit, value) = stored else {
        return None;
    };
    let per_second = match unit {
        TimeUnit::Second => 1,
        TimeUnit::Millisecond => 1_000,
        TimeUnit::Microsecond => 1_000_000,
        TimeUnit::Nanosecond => 1_000_000_000,
    };
    Some(value.div_euclid(per_second))
}

/// A count as its `INTEGER` column holds it (saturating; never negative).
fn stored_count(count: usize) -> i32 {
    i32::try_from(count).unwrap_or(i32::MAX)
}

/// The stored summary of a run's account checks.
fn checks_json(checks: &[AccountCheckRecord]) -> String {
    serde_json::Value::from(
        checks
            .iter()
            .map(|check| {
                serde_json::json!({
                    "account": check.github_user_id,
                    "outcome": check.outcome.as_str(),
                })
            })
            .collect::<Vec<_>>(),
    )
    .to_string()
}

/// A run's account checks back from its stored summary (unreadable entries
/// are dropped).
fn checks_of_json(stored: &str) -> Vec<AccountCheckRecord> {
    let entries: Vec<serde_json::Value> = serde_json::from_str(stored).unwrap_or_default();
    entries
        .iter()
        .filter_map(|entry| {
            Some(AccountCheckRecord {
                github_user_id: entry["account"].as_u64()?,
                outcome: AccountCheckOutcome::parse(entry["outcome"].as_str()?)?,
            })
        })
        .collect()
}

/// A stored count back as a count.
fn read_count(stored: i32) -> usize {
    usize::try_from(stored).unwrap_or_default()
}

impl ScanRunPort for ReviewStore {
    fn start_scan(&self, owner_did: &str, run_id: &str) -> Result<(), ReviewStoreError> {
        self.with_connection(|conn| {
            conn.execute(START_SCAN, params![owner_did, run_id])
                .map(drop)
                .map_err(db_error)
        })
        .map_err(port_error)
    }

    fn finish_scan(
        &self,
        owner_did: &str,
        run_id: &str,
        status: ScanStatus,
        resume_after: Option<i64>,
        counts: ScanCounts,
        tally: KpiTally<'_>,
    ) -> Result<(), ReviewStoreError> {
        self.with_tally(
            tally,
            |_| true,
            |conn| {
                conn.execute(
                    FINISH_SCAN,
                    params![
                        status.as_str(),
                        resume_after,
                        stored_count(counts.new),
                        stored_count(counts.already_published),
                        stored_count(counts.declined_hidden),
                        owner_did,
                        run_id
                    ],
                )
                .map(drop)
                .map_err(db_error)
            },
        )
        .map_err(port_error)
    }

    fn interrupt_running_scans(&self) -> Result<Vec<String>, ReviewStoreError> {
        let after_restart = ScanStatus::Running.after_restart().as_str();
        self.with_connection(|conn| {
            let mut owners = owners_where(conn, OWNERS_WITH_RUNNING_SCANS)?;
            owners.sort();
            owners.iter().try_for_each(|owner_did| {
                conn.execute(
                    INTERRUPT_OWNERS_RUNNING_SCANS,
                    params![after_restart, owner_did],
                )
                .map(drop)
                .map_err(db_error)
            })?;
            Ok(owners)
        })
        .map_err(port_error)
    }

    fn owners_with_paused_scans(&self) -> Result<Vec<String>, ReviewStoreError> {
        self.with_connection(|conn| owners_where(conn, OWNERS_WITH_PAUSED_SCANS))
            .map_err(port_error)
    }

    fn record_plan(
        &self,
        owner_did: &str,
        run_id: &str,
        rows: &[PlannedScanRepo],
    ) -> Result<(), ReviewStoreError> {
        self.with_connection(|conn| {
            in_transaction(conn, |conn| {
                if !run_stored(conn, owner_did, run_id)? {
                    return Ok(());
                }
                rows.iter().enumerate().try_for_each(|(position, row)| {
                    conn.execute(
                        RECORD_PLAN_ROW,
                        params![
                            owner_did,
                            run_id,
                            stored_count(position),
                            db_id(row.repo_id)?,
                            row.full_name,
                            source_kind(row.source),
                            source_account(row.source)?,
                            row.outside_default.map(OutsideDefault::as_str),
                        ],
                    )
                    .map(drop)
                    .map_err(db_error)
                })
            })
        })
        .map_err(port_error)
    }

    fn mark_repo_read(
        &self,
        owner_did: &str,
        run_id: &str,
        position: usize,
    ) -> Result<(), ReviewStoreError> {
        self.with_connection(|conn| {
            conn.execute(
                MARK_REPO_READ,
                params![owner_did, run_id, stored_count(position)],
            )
            .map(drop)
            .map_err(db_error)
        })
        .map_err(port_error)
    }

    fn record_skipped(
        &self,
        owner_did: &str,
        run_id: &str,
        skipped: &[SkippedScanRepo],
    ) -> Result<(), ReviewStoreError> {
        self.with_connection(|conn| {
            in_transaction(conn, |conn| {
                if !run_stored(conn, owner_did, run_id)? {
                    return Ok(());
                }
                skipped.iter().try_for_each(|row| {
                    conn.execute(
                        RECORD_SKIPPED_ROW,
                        params![
                            owner_did,
                            run_id,
                            db_id(row.repo_id)?,
                            row.full_name,
                            db_id(row.source_account_id)?,
                            row.reason.as_str(),
                            owner_did,
                            run_id,
                        ],
                    )
                    .map(drop)
                    .map_err(db_error)
                })
            })
        })
        .map_err(port_error)
    }

    fn record_account_checks(
        &self,
        owner_did: &str,
        run_id: &str,
        checks: &[AccountCheckRecord],
    ) -> Result<(), ReviewStoreError> {
        self.with_connection(|conn| {
            conn.execute(
                RECORD_ACCOUNT_CHECKS,
                params![checks_json(checks), owner_did, run_id],
            )
            .map(drop)
            .map_err(db_error)
        })
        .map_err(port_error)
    }

    fn latest_scan(&self, owner_did: &str) -> Result<Option<ScanRun>, ReviewStoreError> {
        self.with_connection(|conn| {
            conn.query_row(LATEST_SCAN, [owner_did], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    unix_secs_of(&row.get::<_, Value>(1)?),
                    ScanCounts {
                        new: read_count(row.get(2)?),
                        already_published: read_count(row.get(3)?),
                        declined_hidden: read_count(row.get(4)?),
                    },
                    usize::try_from(row.get::<_, i64>(5)?).unwrap_or_default(),
                ))
            })
            .optional()
            .map(|row| {
                row.and_then(|(status, resume_after, counts, repos_read)| {
                    ScanStatus::parse(&status).map(|status| ScanRun {
                        status,
                        resume_after,
                        counts,
                        repos_read,
                    })
                })
            })
            .map_err(db_error)
        })
        .map_err(port_error)
    }
}

/// A plan row's `source_kind`.
fn source_kind(source: RepoSource) -> &'static str {
    match source {
        RepoSource::Account(_) => "account",
        RepoSource::Imported(_) => "imported",
    }
}

/// A plan row's `source_account_id`: an imported row has none.
fn source_account(source: RepoSource) -> Result<Option<i64>, StoreError> {
    match source {
        RepoSource::Account(account_id) => db_id(account_id).map(Some),
        RepoSource::Imported(_) => Ok(None),
    }
}

/// A stored plan row's source back: an imported row is its own repo.
fn stored_source(kind: &str, account_id: Option<i64>, repo_id: u64) -> Option<RepoSource> {
    match kind {
        "account" => u64::try_from(account_id?).ok().map(RepoSource::Account),
        "imported" => Some(RepoSource::Imported(repo_id)),
        _ => None,
    }
}

impl ScanPlanRead for ReviewStore {
    fn latest_account_checks(
        &self,
        owner_did: &str,
    ) -> Result<Vec<AccountCheckRecord>, ReviewStoreError> {
        self.with_connection(|conn| {
            conn.query_row(LATEST_ACCOUNT_CHECKS, [owner_did], |row| {
                row.get::<_, Option<String>>(0)
            })
            .optional()
            .map(|stored| stored.flatten().as_deref().map(checks_of_json))
            .map(Option::unwrap_or_default)
            .map_err(db_error)
        })
        .map_err(port_error)
    }

    fn latest_plan(&self, owner_did: &str) -> Result<Vec<ScanPlanRow>, ReviewStoreError> {
        self.with_connection(|conn| {
            let mut statement = conn.prepare(LATEST_PLAN).map_err(db_error)?;
            let rows = statement
                .query_map(params![owner_did, owner_did], |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, Option<i64>>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, Option<String>>(5)?,
                    ))
                })
                .map_err(db_error)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(db_error)?;
            Ok(rows
                .into_iter()
                .filter_map(|(repo_id, full_name, kind, account_id, status, tag)| {
                    let repo_id = u64::try_from(repo_id).ok()?;
                    Some(ScanPlanRow {
                        repo: PlannedScanRepo {
                            repo_id,
                            full_name,
                            source: stored_source(&kind, account_id, repo_id)?,
                            outside_default: tag.as_deref().and_then(OutsideDefault::parse),
                        },
                        status: PlanRowStatus::parse(&status)?,
                    })
                })
                .collect())
        })
        .map_err(port_error)
    }

    fn latest_skipped(&self, owner_did: &str) -> Result<Vec<SkippedScanRepo>, ReviewStoreError> {
        self.with_connection(|conn| {
            let mut statement = conn.prepare(LATEST_SKIPPED).map_err(db_error)?;
            let rows = statement
                .query_map(params![owner_did, owner_did], |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, Option<i64>>(2)?,
                        row.get::<_, Option<String>>(3)?,
                    ))
                })
                .map_err(db_error)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(db_error)?;
            Ok(rows
                .into_iter()
                .filter_map(|(repo_id, full_name, account_id, reason)| {
                    Some(SkippedScanRepo {
                        repo_id: u64::try_from(repo_id).ok()?,
                        full_name,
                        source_account_id: u64::try_from(account_id?).ok()?,
                        reason: RepoSkipReason::parse(reason.as_deref()?)?,
                    })
                })
                .collect())
        })
        .map_err(port_error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DataKey;
    use proptest::prelude::*;

    fn status() -> impl Strategy<Value = ScanStatus> {
        proptest::sample::select(ScanStatus::ALL.to_vec())
    }

    fn counts() -> impl Strategy<Value = ScanCounts> {
        (0usize..50, 0usize..50, 0usize..50).prop_map(
            |(new, already_published, declined_hidden)| ScanCounts {
                new,
                already_published,
                declined_hidden,
            },
        )
    }

    const NONE_FOUND: ScanCounts = ScanCounts {
        new: 0,
        already_published: 0,
        declined_hidden: 0,
    };

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(32))]

        /// Universe: the scan_runs rows of two DIDs. Each DID reads its own
        /// most recent run — running (nothing found yet) until finished,
        /// then how it ended, when it may resume and what it found; the
        /// other's runs never show.
        #[test]
        fn each_did_reads_its_own_latest_run(
            first in status(), last in status(), other in status(),
            resume_after in proptest::option::of(1_700_000_000i64..1_900_000_000),
            found in counts(), other_found in counts(),
        ) {
            let store = ReviewStore::open_in_memory(DataKey::generate()).unwrap();
            prop_assert_eq!(store.latest_scan("did:plc:priya").unwrap(), None);
            store.start_scan("did:plc:priya", "run-1").unwrap();
            store.finish_scan("did:plc:priya", "run-1", first, None, found, KpiTally::NONE).unwrap();
            store.start_scan("did:plc:priya", "run-2").unwrap();
            prop_assert_eq!(
                store.latest_scan("did:plc:priya").unwrap().map(|r| (r.status, r.counts)),
                Some((ScanStatus::Running, NONE_FOUND))
            );
            store.start_scan("did:plc:sam", "run-3").unwrap();
            store.finish_scan("did:plc:sam", "run-3", other, None, other_found, KpiTally::NONE).unwrap();
            store.finish_scan("did:plc:priya", "run-2", last, resume_after, found, KpiTally::NONE).unwrap();
            prop_assert_eq!(
                store.latest_scan("did:plc:priya").unwrap(),
                Some(ScanRun { status: last, resume_after, counts: found, repos_read: 0 })
            );
            prop_assert_eq!(
                store.latest_scan("did:plc:sam").unwrap(),
                Some(ScanRun { status: other, resume_after: None, counts: other_found, repos_read: 0 })
            );
        }

        /// Universe: the latest runs of up to five DIDs, any status. The
        /// startup sweep moves exactly the running ones to interrupted (so
        /// the one-running-scan limit frees up) and leaves every other run.
        #[test]
        fn a_restart_interrupts_exactly_the_runs_left_running(
            latest in proptest::collection::vec(status(), 1..5),
        ) {
            let store = ReviewStore::open_in_memory(DataKey::generate()).unwrap();
            let did = |i: usize| format!("did:plc:owner{i}");
            for (i, status) in latest.iter().enumerate() {
                store.start_scan(&did(i), "run").unwrap();
                if *status != ScanStatus::Running {
                    store.finish_scan(&did(i), "run", *status, None, NONE_FOUND, KpiTally::NONE).unwrap();
                }
            }
            let running: Vec<String> = (0..latest.len())
                .filter(|i| latest[*i] == ScanStatus::Running)
                .map(did)
                .collect();
            let paused: Vec<String> = (0..latest.len())
                .filter(|i| latest[*i] == ScanStatus::RateLimited)
                .map(did)
                .collect();
            prop_assert_eq!(store.owners_with_paused_scans().unwrap(), paused);
            prop_assert_eq!(store.interrupt_running_scans().unwrap(), running);
            for (i, status) in latest.iter().enumerate() {
                prop_assert_eq!(
                    store.latest_scan(&did(i)).unwrap().map(|r| r.status),
                    Some(status.after_restart())
                );
            }
            prop_assert_eq!(store.interrupt_running_scans().unwrap(), Vec::<String>::new(), "a second sweep changes nothing");
        }

        /// Universe: a paused run's frozen plan, each row with any
        /// outside-default tag, and the resuming run that records the rows
        /// it carries (U-RRA-D2). Each resumed row keeps its tag, so its
        /// suggestions still count for KPI-RRA-1.
        #[test]
        fn a_resumed_plan_row_keeps_its_outside_default_tag(
            tags in proptest::collection::vec(
                proptest::option::of(proptest::sample::select(OutsideDefault::ALL.to_vec())), 1..10),
        ) {
            let store = ReviewStore::open_in_memory(DataKey::generate()).unwrap();
            let priya = "did:plc:priya";
            let plan: Vec<PlannedScanRepo> = tags
                .iter()
                .enumerate()
                .map(|(i, tag)| PlannedScanRepo {
                    repo_id: 500 + i as u64,
                    full_name: format!("priya/repo-{i}"),
                    source: RepoSource::Account(1 + i as u64 % 2),
                    outside_default: *tag,
                })
                .collect();
            store.start_scan(priya, "run-1").unwrap();
            store.record_plan(priya, "run-1", &plan).unwrap();
            store.mark_repo_read(priya, "run-1", 0).unwrap();
            store.finish_scan(priya, "run-1", ScanStatus::RateLimited, None, NONE_FOUND, KpiTally::NONE).unwrap();
            let carried: Vec<PlannedScanRepo> = store
                .latest_plan(priya)
                .unwrap()
                .into_iter()
                .filter(|row| row.status == PlanRowStatus::Planned)
                .map(|row| row.repo)
                .collect();
            prop_assert_eq!(&carried[..], &plan[1..]);
            store.start_scan(priya, "run-2").unwrap();
            store.record_plan(priya, "run-2", &carried).unwrap();
            let resumed: Vec<PlannedScanRepo> =
                store.latest_plan(priya).unwrap().into_iter().map(|row| row.repo).collect();
            prop_assert_eq!(resumed, carried);
        }
    }

    /// Review R4. Universe: `scan_run_repos` (counted directly) and
    /// `scan_runs` of two DIDs. A purge of one DID between its scan's start
    /// and the plan write leaves no row of that DID; the other DID's run
    /// records its plan as before.
    #[test]
    fn a_purge_between_scan_start_and_plan_write_leaves_no_rows() {
        use ports::ForgetPort;
        let store = ReviewStore::open_in_memory(DataKey::generate()).unwrap();
        let (priya, sam) = ("did:plc:aaaaaaaaaaaa", "did:plc:bbbbbbbbbbbb");
        let plan = vec![PlannedScanRepo {
            repo_id: 7,
            full_name: "priyaraman/tidepool".into(),
            source: RepoSource::Account(1),
            outside_default: None,
        }];
        let skipped = vec![SkippedScanRepo {
            repo_id: 8,
            full_name: "priyaraman/kelp".into(),
            source_account_id: 1,
            reason: RepoSkipReason::AccountUnverified,
        }];
        store.start_scan(priya, "run-1").unwrap();
        store.start_scan(sam, "run-2").unwrap();

        store.purge_owner(priya, KpiTally::NONE).unwrap();
        store.record_plan(priya, "run-1", &plan).unwrap();
        store.record_skipped(priya, "run-1", &skipped).unwrap();
        store.record_plan(sam, "run-2", &plan).unwrap();

        let rows_of = |did: &str| -> i64 {
            store
                .with_connection(|conn| {
                    conn.query_row(
                        "SELECT count(*) FROM scan_run_repos WHERE owner_did = ?",
                        params![did],
                        |row| row.get(0),
                    )
                    .map_err(db_error)
                })
                .unwrap()
        };
        assert_eq!(rows_of(priya), 0, "nothing of the purged DID");
        assert_eq!(store.latest_scan(priya).unwrap(), None);
        assert_eq!(rows_of(sam), 1, "another DID's plan is recorded");
    }
}
