//! Aggregate KPI counters (OD-BRA-11, kpi-instrumentation §2): one row per
//! `(day, event)`, no owner column and no owner in any statement (check-arch
//! rule 6). Event names are `&'static str` from the closed catalogue.

use duckdb::{params, Connection};
use ports::{KpiCounterPort, KpiCounterRow, KpiTally, ReviewStoreError};

use crate::secrets::port_error;
use crate::{db_error, ReviewStore, StoreError};

const COUNT_EVENT: &str = "INSERT INTO kpi_counters (day, event, count)
    VALUES (CAST(? AS DATE), ?, 1)
    ON CONFLICT (day, event) DO UPDATE SET count = kpi_counters.count + 1";

const COUNTERS_BETWEEN: &str = "SELECT CAST(day AS VARCHAR), event, count FROM kpi_counters
    WHERE day BETWEEN CAST(? AS DATE) AND CAST(? AS DATE)
    ORDER BY day, event";

/// Add one to each of `tally`'s events on its day, on `conn` (inside the
/// caller's transaction).
pub(crate) fn add_tally(conn: &Connection, tally: KpiTally<'_>) -> Result<(), StoreError> {
    tally.events.iter().try_for_each(|event| {
        conn.execute(COUNT_EVENT, params![tally.day, event])
            .map(drop)
            .map_err(db_error)
    })
}

impl KpiCounterPort for ReviewStore {
    fn count(&self, day: &str, event: &'static str) -> Result<(), ReviewStoreError> {
        self.with_connection(|conn| {
            add_tally(
                conn,
                KpiTally {
                    day,
                    events: &[event],
                },
            )
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
    use ports::{
        AccountAdmission, AccountAdmitted, AccountRefusal, ForgetPort, GithubAccountsWrite,
        GithubLinkPort, NewWebSession, ReviewStateRead, ReviewStateWrite, ScanCounts, ScanRunPort,
        ScanStatus, SessionPort, Suggestion, SuggestionKey, SuggestionState,
    };
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

        /// Universe: one suggestion's state and the kpi_counters rows. A
        /// state change adds its tally exactly when it moves, so the count
        /// equals the moves however the attempts interleave.
        #[test]
        fn a_state_change_counts_exactly_when_it_moves(
            attempts in prop::collection::vec(any::<bool>(), 0..8)
        ) {
            let store = ReviewStore::open_in_memory(DataKey::generate()).unwrap();
            store.add_pending(PRIYA, &[suggestion()]).unwrap();
            let mut moves = 0;
            for decline in attempts {
                let (from, to) = if decline {
                    (SuggestionState::Pending, SuggestionState::Declined)
                } else {
                    (SuggestionState::Declined, SuggestionState::Pending)
                };
                let tally = KpiTally { day: DAY, events: &["suggestion.declined"] };
                if store.change_state(PRIYA, &suggestion().key, from, to, tally).unwrap() {
                    moves += 1;
                }
            }
            prop_assert_eq!(count_of(&store, "suggestion.declined"), moves);
        }
    }

    const PRIYA: &str = "did:plc:priya";
    const DAY: &str = "2026-10-13";

    fn suggestion() -> Suggestion {
        Suggestion {
            key: SuggestionKey {
                subject: "github:priyaraman/tidepool".into(),
                predicate: "embodiesPhilosophy".into(),
                object: "org.openlore.philosophy.memory-safety".into(),
            },
            confidence_bp: 2500,
            evidence: vec![],
            why: vec![],
            source_repo: "priyaraman/tidepool".into(),
        }
    }

    fn count_of(store: &ReviewStore, event: &str) -> i64 {
        store
            .counters_between(DAY, DAY)
            .unwrap()
            .iter()
            .filter(|row| row.event == event)
            .map(|row| row.count)
            .sum()
    }

    /// A tally that cannot be stored takes its state change down with it:
    /// the suggestion stays pending and nothing is counted.
    #[test]
    fn a_failed_tally_rolls_back_its_state_change() {
        let store = ReviewStore::open_in_memory(DataKey::generate()).unwrap();
        store.add_pending(PRIYA, &[suggestion()]).unwrap();
        let unstorable = KpiTally {
            day: "not-a-day",
            events: &["suggestion.declined"],
        };
        let declined = store.change_state(
            PRIYA,
            &suggestion().key,
            SuggestionState::Pending,
            SuggestionState::Declined,
            unstorable,
        );
        assert!(declined.is_err());
        assert_eq!(
            store.pending_suggestions(PRIYA).unwrap(),
            vec![suggestion()]
        );
        assert!(store
            .counters_between("2000-01-01", "2100-01-01")
            .unwrap()
            .is_empty());
    }

    /// Each counted write adds its tally only when it takes effect: a link
    /// held by another owner, and a session, scan and forget that land.
    #[test]
    fn each_counted_write_adds_its_tally_when_it_lands() {
        let store = ReviewStore::open_in_memory(DataKey::generate()).unwrap();
        let tally = |events| KpiTally { day: DAY, events };
        store
            .start_session(
                &NewWebSession {
                    session_hash: "hash".into(),
                    csrf_hash: "csrf".into(),
                    owner_did: PRIYA.into(),
                    handle: "priya.test".into(),
                    pds_endpoint: "https://pds.test".into(),
                },
                1_700_000_000,
                tally(&["signin.completed"]),
            )
            .unwrap();
        let linked = store
            .add_or_reverify(PRIYA, "priyaraman", 7, 3, tally(&["github.verify.ok"]))
            .unwrap();
        let held = store
            .add_or_reverify(
                "did:plc:sam",
                "priyaraman",
                7,
                3,
                tally(&["github.verify.ok"]),
            )
            .unwrap();
        assert_eq!(
            (linked, held),
            (
                AccountAdmission::Admitted(AccountAdmitted::Added),
                AccountAdmission::Refused(AccountRefusal::HeldByAnotherOwner)
            )
        );
        store.start_scan(PRIYA, "run").unwrap();
        store
            .finish_scan(
                PRIYA,
                "run",
                ScanStatus::Completed,
                None,
                ScanCounts {
                    new: 0,
                    already_published: 0,
                    declined_hidden: 0,
                },
                tally(&["scan.completed"]),
            )
            .unwrap();
        store.purge_owner(PRIYA, tally(&["disconnect"])).unwrap();
        for (event, expected) in [
            ("signin.completed", 1),
            ("github.verify.ok", 1),
            ("scan.completed", 1),
            ("disconnect", 1),
        ] {
            assert_eq!(count_of(&store, event), expected, "{event}");
        }
        assert_eq!(store.github_link(PRIYA).unwrap(), None, "the forget landed");
    }
}
