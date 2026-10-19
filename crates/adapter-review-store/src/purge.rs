//! Forget a person (US-BRA-012, ADR-074 operator forget-by-DID): the only
//! multi-table DELETE (check-arch `review_store_owner_scoped_sql`). Every
//! statement is bounded by the one owner, all run in ONE transaction, and
//! `kpi_counters` (anonymous, nobody's) is never deleted from; the forget's
//! own tally is added in the same transaction.

use duckdb::Connection;
use ports::{ForgetPort, KpiTally, ReviewStoreError};

use crate::secrets::port_error;
use crate::{db_error, in_transaction, kpi, ReviewStore, StoreError};

/// Every row the app holds about one owner, children before the account.
const PURGE_OWNER: [&str; 12] = [
    "DELETE FROM plans WHERE owner_did = ?",
    "DELETE FROM suggestions WHERE owner_did = ?",
    "DELETE FROM scan_run_repos WHERE owner_did = ?",
    "DELETE FROM scan_runs WHERE owner_did = ?",
    "DELETE FROM repo_choices WHERE owner_did = ?",
    "DELETE FROM imported_repos WHERE owner_did = ?",
    "DELETE FROM github_accounts WHERE owner_did = ?",
    "DELETE FROM github_links WHERE owner_did = ?",
    "DELETE FROM oauth_auth_requests WHERE owner_did_expected = ?",
    "DELETE FROM oauth_sessions WHERE owner_did = ?",
    "DELETE FROM web_sessions WHERE owner_did = ?",
    "DELETE FROM accounts WHERE owner_did = ?",
];

fn purge_in_transaction(
    conn: &Connection,
    owner_did: &str,
    tally: KpiTally<'_>,
) -> Result<(), StoreError> {
    in_transaction(conn, |conn| {
        PURGE_OWNER
            .iter()
            .try_for_each(|sql| conn.execute(sql, [owner_did]).map(drop))
            .map_err(db_error)?;
        kpi::add_tally(conn, tally)
    })
}

impl ForgetPort for ReviewStore {
    fn purge_owner(&self, owner_did: &str, tally: KpiTally<'_>) -> Result<(), ReviewStoreError> {
        self.with_connection(|conn| purge_in_transaction(conn, owner_did, tally))
            .map_err(port_error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DataKey;
    use ports::{
        GithubAccountsWrite, GithubLinkPort, KpiCounterPort, NewWebSession, PlannedScanRepo,
        RepoChoiceWrite, ReviewStateRead, ReviewStateWrite, SavedRepoChoice, ScanRunPort,
        SecretStorePort, SessionPort, Suggestion, SuggestionKey,
    };
    use proptest::prelude::*;

    /// Every owner table, dumped whole (byte-for-byte rows, sorted).
    const UNIVERSE: [&str; 13] = [
        "accounts",
        "github_links",
        "github_accounts",
        "repo_choices",
        "imported_repos",
        "scan_run_repos",
        "suggestions",
        "scan_runs",
        "plans",
        "oauth_auth_requests",
        "oauth_sessions",
        "web_sessions",
        "kpi_counters",
    ];

    fn snapshot(store: &ReviewStore) -> Vec<(String, Vec<String>)> {
        store
            .with_connection(|conn| {
                UNIVERSE
                    .iter()
                    .map(|table| {
                        let sql = format!("SELECT * FROM {table}");
                        let mut statement = conn.prepare(&sql).map_err(db_error)?;
                        let mut rows: Vec<String> = statement
                            .query_map([], |row| {
                                let columns = row.as_ref().column_count();
                                Ok((0..columns)
                                    .map(|i| format!("{:?}", row.get::<_, duckdb::types::Value>(i)))
                                    .collect::<Vec<_>>()
                                    .join("|"))
                            })
                            .map_err(db_error)?
                            .collect::<Result<_, _>>()
                            .map_err(db_error)?;
                        rows.sort();
                        Ok((table.to_string(), rows))
                    })
                    .collect()
            })
            .expect("snapshot")
    }

    fn rows_naming(snapshot: &[(String, Vec<String>)], owner: &str) -> usize {
        snapshot
            .iter()
            .flat_map(|(_, rows)| rows)
            .filter(|row| row.contains(owner))
            .count()
    }

    fn seed(store: &ReviewStore, owner: &str, github_id: u64, n: usize) {
        store
            .start_session(
                &NewWebSession {
                    session_hash: format!("hash-{owner}"),
                    csrf_hash: "csrf".into(),
                    owner_did: owner.into(),
                    handle: format!("{}.test", &owner[8..]),
                    pds_endpoint: "https://pds.test".into(),
                },
                1_700_000_000,
                KpiTally::NONE,
            )
            .unwrap();
        store
            .put_oauth_session(owner, "https://pds.test", "atproto", b"tokens")
            .unwrap();
        store
            .put_auth_request(&format!("state-{owner}"), "iss", Some(owner), b"req")
            .unwrap();
        store
            .add_or_reverify(
                owner,
                &format!("gh-{}", &owner[8..]),
                github_id,
                3,
                KpiTally::NONE,
            )
            .unwrap();
        let suggestions: Vec<Suggestion> = (0..n)
            .map(|i| Suggestion {
                key: SuggestionKey {
                    subject: format!("https://github.com/{owner}/repo{i}"),
                    predicate: "embodies".into(),
                    object: "test-driven".into(),
                },
                confidence_bp: 7000,
                evidence: vec!["e".into()],
                why: vec!["w".into()],
                source_repo: format!("{owner}/repo{i}"),
            })
            .collect();
        store.add_pending(owner, &suggestions).unwrap();
        let chosen: Vec<SavedRepoChoice> = (0..n)
            .map(|i| SavedRepoChoice {
                repo_id: i as u64 + 1,
                full_name: format!("{owner}/repo{i}"),
                ticked: i % 2 == 0,
            })
            .collect();
        store
            .save_choice(owner, github_id, &chosen, KpiTally::NONE)
            .unwrap();
        store.start_scan(owner, &format!("run-{owner}")).unwrap();
        let plan: Vec<PlannedScanRepo> = chosen
            .iter()
            .map(|c| PlannedScanRepo {
                repo_id: c.repo_id,
                full_name: c.full_name.clone(),
                source_account_id: github_id,
                outside_default: None,
            })
            .collect();
        store
            .record_plan(owner, &format!("run-{owner}"), &plan)
            .unwrap();
        store.count("2026-10-04", "signin.completed").unwrap();
    }

    proptest! {
        /// Universe: every table of the store, two owners with arbitrary
        /// amounts of state. purge(A) removes every row naming A and leaves
        /// every row of B — and the anonymous counters — byte-identical.
        #[test]
        fn purge_touches_only_the_owners_rows(a_count in 0usize..4, b_count in 0usize..4) {
            let (a, b) = ("did:plc:aaaaaaaaaaaa", "did:plc:bbbbbbbbbbbb");
            let store = ReviewStore::open_in_memory(DataKey::generate()).unwrap();
            seed(&store, a, 1, a_count);
            seed(&store, b, 2, b_count);
            let before = snapshot(&store);
            store.purge_owner(a, KpiTally::NONE).unwrap();
            let after = snapshot(&store);
            prop_assert_eq!(rows_naming(&after, a), 0, "nothing about A remains");
            let without_a = |s: &[(String, Vec<String>)]| -> Vec<(String, Vec<String>)> {
                s.iter()
                    .map(|(t, rows)| (t.clone(), rows.iter().filter(|r| !r.contains(a)).cloned().collect()))
                    .collect()
            };
            prop_assert_eq!(without_a(&after), without_a(&before), "B and the counters untouched");
            prop_assert!(store.pending_suggestions(b).unwrap().len() == b_count);
            prop_assert!(store.github_link(b).unwrap().is_some());
        }
    }
}
