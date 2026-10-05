//! Aggregate KPI counters (OD-BRA-11, kpi-instrumentation §2): one row per
//! `(day, event)`, no owner column and no owner in any statement (check-arch
//! rule 6). Event names are `&'static str` from the closed catalogue.

use duckdb::params;
use ports::{KpiCounterPort, KpiCounterRow, ReviewStoreError};

use crate::secrets::port_error;
use crate::{db_error, ReviewStore};

const COUNT_EVENT: &str = "INSERT INTO kpi_counters (day, event, count)
    VALUES (CAST(? AS DATE), ?, 1)
    ON CONFLICT (day, event) DO UPDATE SET count = kpi_counters.count + 1";

const COUNTERS_BETWEEN: &str = "SELECT CAST(day AS VARCHAR), event, count FROM kpi_counters
    WHERE day BETWEEN CAST(? AS DATE) AND CAST(? AS DATE)
    ORDER BY day, event";

impl KpiCounterPort for ReviewStore {
    fn count(&self, day: &str, event: &'static str) -> Result<(), ReviewStoreError> {
        self.with_connection(|conn| {
            conn.execute(COUNT_EVENT, params![day, event])
                .map(drop)
                .map_err(db_error)
        })
        .map_err(port_error)
    }

    fn counters_between(
        &self,
        from: &str,
        to: &str,
    ) -> Result<Vec<KpiCounterRow>, ReviewStoreError> {
        self.with_connection(|conn| {
            let mut statement = conn.prepare(COUNTERS_BETWEEN).map_err(db_error)?;
            let rows = statement
                .query_map([from, to], |row| {
                    Ok(KpiCounterRow {
                        day: row.get(0)?,
                        event: row.get(1)?,
                        count: row.get(2)?,
                    })
                })
                .map_err(db_error)?;
            rows.collect::<Result<_, _>>().map_err(db_error)
        })
        .map_err(port_error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DataKey;
    use proptest::prelude::*;

    const EVENTS: [&str; 3] = ["signin.completed", "scan.completed", "share.posted"];

    proptest! {
        /// Universe: the kpi_counters rows. Counting n events across days
        /// stores exactly n per (day, event), and a window reads only its
        /// days.
        #[test]
        fn counts_add_up_per_day_and_event(
            counted in prop::collection::vec((1u8..4, 0usize..3), 0..12)
        ) {
            let store = ReviewStore::open_in_memory(DataKey::generate()).unwrap();
            for (day, event) in &counted {
                store.count(&format!("2026-10-0{day}"), EVENTS[*event]).unwrap();
            }
            let window = store.counters_between("2026-10-02", "2026-10-03").unwrap();
            let total: i64 = window.iter().map(|r| r.count).sum();
            let expected = counted.iter().filter(|(day, _)| (2..=3).contains(day)).count();
            prop_assert_eq!(total, expected as i64);
            prop_assert!(window.iter().all(|r| r.day.as_str() >= "2026-10-02" && r.day.as_str() <= "2026-10-03"));
        }
    }
}
