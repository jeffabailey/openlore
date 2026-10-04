//! `ScanRunPort`: the owner's scan runs (`scan_runs`, ADR-076 §3).

use duckdb::{params, OptionalExt};
use ports::{ReviewStoreError, ScanRunPort, ScanStatus};

use crate::secrets::port_error;
use crate::{db_error, ReviewStore};

const RECORD_FINISHED_SCAN: &str = "INSERT INTO scan_runs
    (owner_did, run_id, status, started_at, finished_at) VALUES (?, ?, ?, now(), now())";

const LATEST_SCAN_STATUS: &str = "SELECT status FROM scan_runs WHERE owner_did = ?
    ORDER BY started_at DESC, rowid DESC LIMIT 1";

impl ScanRunPort for ReviewStore {
    fn record_finished_scan(
        &self,
        owner_did: &str,
        run_id: &str,
        status: ScanStatus,
    ) -> Result<(), ReviewStoreError> {
        self.with_connection(|conn| {
            conn.execute(
                RECORD_FINISHED_SCAN,
                params![owner_did, run_id, status.as_str()],
            )
            .map(drop)
            .map_err(db_error)
        })
        .map_err(port_error)
    }

    fn latest_scan_status(&self, owner_did: &str) -> Result<Option<ScanStatus>, ReviewStoreError> {
        self.with_connection(|conn| {
            conn.query_row(LATEST_SCAN_STATUS, [owner_did], |row| {
                row.get::<_, String>(0)
            })
            .optional()
            .map(|stored| stored.as_deref().and_then(ScanStatus::parse))
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

        /// Universe: the scan_runs rows of two DIDs. Each DID reads the status
        /// of its own most recent run; the other's runs never show.
        #[test]
        fn each_did_reads_the_status_of_its_own_latest_run(
            first in status(), last in status(), other in status(),
        ) {
            let store = ReviewStore::open_in_memory(DataKey::generate()).unwrap();
            prop_assert_eq!(store.latest_scan_status("did:plc:priya").unwrap(), None);
            store.record_finished_scan("did:plc:priya", "run-1", first).unwrap();
            store.record_finished_scan("did:plc:priya", "run-2", last).unwrap();
            store.record_finished_scan("did:plc:sam", "run-3", other).unwrap();
            prop_assert_eq!(store.latest_scan_status("did:plc:priya").unwrap(), Some(last));
            prop_assert_eq!(store.latest_scan_status("did:plc:sam").unwrap(), Some(other));
        }
    }
}
