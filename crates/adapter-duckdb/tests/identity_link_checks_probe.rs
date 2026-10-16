//! Real-infrastructure tests of the insert-only identity-link check store
//! (ADR-085 §7 + Earned Trust): a REAL DuckDB file per test, driven through
//! the check ports (never mocked).

use adapter_duckdb::DuckDbStorageAdapter;
use chrono::{DateTime, TimeZone, Utc};
use claim_domain::identity_link::CheckVerdict;
use duckdb::Connection;
use ports::{
    CheckFilter, IdentityLinkCheckReadPort, IdentityLinkCheckWritePort, LinkCheck, ProbeOutcome,
    ProbeRefusalReason,
};
use std::path::Path;

const JEFF: &str = "did:plc:test-jeff";

fn at(day: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 10, day, 9, 0, 0).unwrap()
}

fn check(day: u32, verdict: CheckVerdict) -> LinkCheck {
    LinkCheck {
        subject_did: JEFF.to_string(),
        github_user_id: 1_001,
        login_seen: "jeffbailey".to_string(),
        verdict,
        checked_at: at(day),
    }
}

fn raw_sql(db: &Path, sql: &str) {
    Connection::open(db).unwrap().execute_batch(sql).unwrap();
}

fn stored_checks(db: &Path) -> i64 {
    Connection::open(db)
        .unwrap()
        .query_row("SELECT COUNT(*) FROM identity_link_checks", [], |r| {
            r.get(0)
        })
        .unwrap()
}

#[test]
fn a_fresh_store_passes_the_probe_and_the_sentinel_leaves_no_check_behind() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("openlore.duckdb");
    let storage = DuckDbStorageAdapter::open(&db).unwrap();

    assert!(matches!(
        storage.identity_link_check_adapter().probe(),
        ProbeOutcome::Ok
    ));
    drop(storage);
    assert_eq!(stored_checks(&db), 0, "the probe sentinel is rolled back");
}

#[test]
fn two_checks_of_one_link_are_both_kept_and_the_later_one_reads_as_latest() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("openlore.duckdb");
    let storage = DuckDbStorageAdapter::open(&db).unwrap();
    let checks = storage.identity_link_check_adapter();

    checks
        .record_check(&check(1, CheckVerdict::Verified))
        .unwrap();
    checks.record_check(&check(5, CheckVerdict::NoBio)).unwrap();
    // Same instant as the second: insertion order breaks the tie.
    checks
        .record_check(&check(5, CheckVerdict::DidMissing))
        .unwrap();

    assert_eq!(
        checks.latest_checks(&CheckFilter::All, at(9)).unwrap(),
        vec![check(5, CheckVerdict::DidMissing)]
    );
    assert_eq!(
        checks
            .latest_checks(&CheckFilter::Subject(JEFF.to_string()), at(3))
            .unwrap(),
        vec![check(1, CheckVerdict::Verified)],
        "a check dated after `now` is skipped"
    );
    assert!(checks
        .latest_checks(&CheckFilter::Subject("did:plc:other".to_string()), at(9))
        .unwrap()
        .is_empty());
    drop(checks);
    drop(storage);
    assert_eq!(stored_checks(&db), 3, "every check is kept");
}

#[test]
fn a_store_whose_check_table_cannot_hold_two_checks_for_one_link_is_refused_by_the_probe() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("openlore.duckdb");
    drop(DuckDbStorageAdapter::open(&db).unwrap());
    // The lie: the table is keyed by the link, so a second check cannot be kept.
    raw_sql(
        &db,
        "DROP TABLE identity_link_checks;
         CREATE TABLE identity_link_checks (
            check_id BIGINT DEFAULT nextval('identity_link_check_ids'),
            subject_did VARCHAR NOT NULL, github_user_id BIGINT NOT NULL,
            login_seen VARCHAR NOT NULL, verdict VARCHAR NOT NULL,
            checked_at TIMESTAMP NOT NULL,
            PRIMARY KEY (subject_did, github_user_id));",
    );
    let storage = DuckDbStorageAdapter::open(&db).unwrap();
    match storage.identity_link_check_adapter().probe() {
        ProbeOutcome::Refused { reason, .. } => assert_eq!(
            reason,
            ProbeRefusalReason::StorageIdentityLinkCheckHistoryUnreliable
        ),
        ProbeOutcome::Ok => panic!("a check table that cannot keep history must be refused"),
    }
}

#[test]
fn migration_v7_upgrades_a_v6_store_once() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("openlore.duckdb");
    drop(DuckDbStorageAdapter::open(&db).unwrap());
    raw_sql(
        &db,
        "DROP TABLE identity_link_checks; DROP SEQUENCE identity_link_check_ids; \
         DELETE FROM schema_version WHERE version >= 7;",
    );

    drop(DuckDbStorageAdapter::open(&db).unwrap());
    drop(DuckDbStorageAdapter::open(&db).unwrap());

    let v7_rows: i64 = Connection::open(&db)
        .unwrap()
        .query_row(
            "SELECT COUNT(*) FROM schema_version WHERE version = 7",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(v7_rows, 1);
    assert_eq!(stored_checks(&db), 0);
}
