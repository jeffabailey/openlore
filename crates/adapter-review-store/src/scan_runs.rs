//! `ScanRunPort`: the owner's scan runs (`scan_runs`, ADR-076 §3).

use duckdb::types::{TimeUnit, Value};
use duckdb::{params, OptionalExt};
use ports::{ReviewStoreError, ScanRun, ScanRunPort, ScanStatus};

use crate::secrets::port_error;
use crate::{db_error, ReviewStore};

const START_SCAN: &str = "INSERT INTO scan_runs
    (owner_did, run_id, status, started_at) VALUES (?, ?, 'running', now())";

const FINISH_SCAN: &str = "UPDATE scan_runs
    SET status = ?, resume_after = to_timestamp(?), finished_at = now()
    WHERE owner_did = ? AND run_id = ?";

// Restart sweep, one owner at a time (every statement owner-scoped).
const OWNERS_WITH_RUNNING_SCANS: &str =
    "SELECT DISTINCT owner_did FROM scan_runs WHERE status = 'running'";
const INTERRUPT_OWNERS_RUNNING_SCANS: &str = "UPDATE scan_runs
    SET status = ?, finished_at = now()
    WHERE owner_did = ? AND status = 'running'";

// `resume_after` is read raw: the TIMESTAMPTZ overloads of `epoch` & co.
// live in the ICU extension, which the app never loads.
const LATEST_SCAN: &str = "SELECT status, resume_after
    FROM scan_runs WHERE owner_did = ?
    ORDER BY started_at DESC, rowid DESC LIMIT 1";

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
    ) -> Result<(), ReviewStoreError> {
        self.with_connection(|conn| {
            conn.execute(
                FINISH_SCAN,
                params![status.as_str(), resume_after, owner_did, run_id],
            )
            .map(drop)
            .map_err(db_error)
        })
        .map_err(port_error)
    }

    fn interrupt_running_scans(&self) -> Result<usize, ReviewStoreError> {
        let after_restart = ScanStatus::Running.after_restart().as_str();
        self.with_connection(|conn| {
            let owners: Vec<String> = conn
                .prepare(OWNERS_WITH_RUNNING_SCANS)
                .and_then(|mut statement| {
                    statement
                        .query_map([], |row| row.get::<_, String>(0))?
                        .collect()
                })
                .map_err(db_error)?;
            owners.iter().try_fold(0usize, |changed, owner_did| {
                conn.execute(
                    INTERRUPT_OWNERS_RUNNING_SCANS,
                    params![after_restart, owner_did],
                )
                .map(|rows| changed + rows)
                .map_err(db_error)
            })
        })
        .map_err(port_error)
    }

    fn latest_scan(&self, owner_did: &str) -> Result<Option<ScanRun>, ReviewStoreError> {
        self.with_connection(|conn| {
            conn.query_row(LATEST_SCAN, [owner_did], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    unix_secs_of(&row.get::<_, Value>(1)?),
                ))
            })
            .optional()
            .map(|row| {
                row.and_then(|(status, resume_after)| {
                    ScanStatus::parse(&status).map(|status| ScanRun {
                        status,
                        resume_after,
                    })
                })
            })
            .map_err(db_error)
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

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(32))]

        /// Universe: the scan_runs rows of two DIDs. Each DID reads its own
        /// most recent run — running until finished, then how it ended and
        /// when it may resume; the other's runs never show.
        #[test]
        fn each_did_reads_its_own_latest_run(
            first in status(), last in status(), other in status(),
            resume_after in proptest::option::of(1_700_000_000i64..1_900_000_000),
        ) {
            let store = ReviewStore::open_in_memory(DataKey::generate()).unwrap();
            prop_assert_eq!(store.latest_scan("did:plc:priya").unwrap(), None);
            store.start_scan("did:plc:priya", "run-1").unwrap();
            store.finish_scan("did:plc:priya", "run-1", first, None).unwrap();
            store.start_scan("did:plc:priya", "run-2").unwrap();
            prop_assert_eq!(
                store.latest_scan("did:plc:priya").unwrap().map(|r| r.status),
                Some(ScanStatus::Running)
            );
            store.start_scan("did:plc:sam", "run-3").unwrap();
            store.finish_scan("did:plc:sam", "run-3", other, None).unwrap();
            store.finish_scan("did:plc:priya", "run-2", last, resume_after).unwrap();
            prop_assert_eq!(
                store.latest_scan("did:plc:priya").unwrap(),
                Some(ScanRun { status: last, resume_after })
            );
            prop_assert_eq!(
                store.latest_scan("did:plc:sam").unwrap(),
                Some(ScanRun { status: other, resume_after: None })
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
                    store.finish_scan(&did(i), "run", *status, None).unwrap();
                }
            }
            let running = latest.iter().filter(|s| **s == ScanStatus::Running).count();
            prop_assert_eq!(store.interrupt_running_scans().unwrap(), running);
            for (i, status) in latest.iter().enumerate() {
                prop_assert_eq!(
                    store.latest_scan(&did(i)).unwrap().map(|r| r.status),
                    Some(status.after_restart())
                );
            }
            prop_assert_eq!(store.interrupt_running_scans().unwrap(), 0, "a second sweep changes nothing");
        }
    }
}
