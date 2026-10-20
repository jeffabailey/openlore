//! Real-infrastructure tests of the insert-only repo authority observation
//! store (ADR-087 §5 + Earned Trust): a REAL DuckDB file per test, driven
//! through the repo-authority check ports (never mocked).

use adapter_duckdb::DuckDbStorageAdapter;
use chrono::{DateTime, TimeZone, Utc};
use claim_domain::authority::RepoOwnerKind;
use duckdb::Connection;
use ports::{
    AuthorityFile, FoundRepo, ProbeOutcome, ProbeRefusalReason, RepoAuthorityCheck,
    RepoAuthorityCheckReadPort, RepoAuthorityCheckWritePort, RepoObservation, RepoSubjectKey,
};
use std::path::Path;

const JEFF: &str = "did:plc:test-jeff";

fn at(day: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 10, day, 9, 0, 0).unwrap()
}

fn found(owner_type: RepoOwnerKind, authority_file: AuthorityFile) -> RepoObservation {
    RepoObservation::Found(FoundRepo {
        repo_id: 42,
        full_name_seen: "acme/Tool".to_string(),
        owner_id: 7,
        owner_type,
        authority_file,
    })
}

fn observed(subject: &str, day: u32, observation: RepoObservation) -> RepoAuthorityCheck {
    RepoAuthorityCheck {
        subject_key: RepoSubjectKey::of(subject),
        observation,
        checked_at: at(day),
    }
}

fn raw_sql(db: &Path, sql: &str) -> Result<(), duckdb::Error> {
    Connection::open(db).unwrap().execute_batch(sql)
}

fn count(db: &Path, sql: &str) -> i64 {
    Connection::open(db)
        .unwrap()
        .query_row(sql, [], |r| r.get(0))
        .unwrap()
}

fn stored_checks(db: &Path) -> i64 {
    count(db, "SELECT COUNT(*) FROM repo_authority_checks")
}

#[test]
fn a_fresh_store_passes_the_probe_and_the_sentinel_leaves_no_observation_behind() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("openlore.duckdb");
    let storage = DuckDbStorageAdapter::open(&db).unwrap();

    assert!(matches!(
        storage.repo_authority_check_adapter().probe(),
        ProbeOutcome::Ok
    ));
    drop(storage);
    assert_eq!(stored_checks(&db), 0, "the probe sentinel is rolled back");
}

#[test]
fn every_observation_is_kept_and_the_latest_is_the_newest_then_the_last_recorded() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("openlore.duckdb");
    let storage = DuckDbStorageAdapter::open(&db).unwrap();
    let checks = storage.repo_authority_check_adapter();
    let listed = AuthorityFile::Present(vec![JEFF.to_string(), "did:plc:rachel".to_string()]);

    // Recorded out of time order: the last recorded is NOT the newest.
    for check in [
        observed(
            "github:acme/tool",
            5,
            found(RepoOwnerKind::User, AuthorityFile::Absent),
        ),
        observed(
            "github:ACME/Tool",
            5,
            found(RepoOwnerKind::Organization, listed.clone()),
        ),
        observed(
            "github:acme/tool",
            2,
            found(RepoOwnerKind::User, AuthorityFile::Invalid),
        ),
        observed("github:gone/repo", 3, RepoObservation::RepoNotFound),
    ] {
        checks.record(&check).unwrap();
    }

    assert_eq!(
        checks
            .latest(
                &[
                    RepoSubjectKey::of("github:gone/repo"),
                    RepoSubjectKey::of("github:Acme/TOOL"),
                    RepoSubjectKey::of("github:never/seen"),
                ],
                at(9),
            )
            .unwrap(),
        vec![
            observed(
                "github:acme/tool",
                5,
                found(RepoOwnerKind::Organization, listed)
            ),
            observed("github:gone/repo", 3, RepoObservation::RepoNotFound),
        ],
        "newest first, the later check_id breaks a tie; subjects fold case; never-seen is absent"
    );
    assert_eq!(
        checks
            .latest(&[RepoSubjectKey::of("github:acme/tool")], at(4))
            .unwrap(),
        vec![observed(
            "github:acme/tool",
            2,
            found(RepoOwnerKind::User, AuthorityFile::Invalid)
        )],
        "an observation dated after `now` is skipped"
    );
    drop(checks);
    drop(storage);
    assert_eq!(stored_checks(&db), 4, "every observation is kept");
    assert_eq!(
        count(
            &db,
            "SELECT COUNT(*) FROM repo_authority_checks WHERE subject_key = 'github:acme/tool'"
        ),
        3,
        "the stored key is case-folded"
    );
}

#[test]
fn a_repo_not_found_row_carries_no_file_state_and_a_found_row_always_does() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("openlore.duckdb");
    drop(DuckDbStorageAdapter::open(&db).unwrap());

    for (label, sql) in [
        (
            "repo_not_found with a file state",
            "INSERT INTO repo_authority_checks (subject_key, outcome, authority_file, checked_at) \
             VALUES ('github:gone/repo', 'repo_not_found', 'absent', now())",
        ),
        (
            "found with no file state",
            "INSERT INTO repo_authority_checks \
             (subject_key, outcome, repo_id, full_name_seen, owner_id, owner_type, checked_at) \
             VALUES ('github:acme/tool', 'found', 1, 'acme/tool', 1, 'user', now())",
        ),
    ] {
        assert!(raw_sql(&db, sql).is_err(), "the table refuses {label}");
    }
    assert_eq!(stored_checks(&db), 0);
}

#[test]
fn a_store_whose_observation_table_cannot_hold_two_observations_of_one_repo_is_refused_by_the_probe(
) {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("openlore.duckdb");
    drop(DuckDbStorageAdapter::open(&db).unwrap());
    // The lie: the table is keyed by the subject, so a second observation
    // cannot be kept and the later one never reads back as latest.
    raw_sql(
        &db,
        "DROP TABLE repo_authority_checks;
         CREATE TABLE repo_authority_checks (
            check_id BIGINT DEFAULT nextval('repo_authority_check_ids'),
            subject_key VARCHAR PRIMARY KEY, outcome VARCHAR NOT NULL,
            repo_id UBIGINT, full_name_seen VARCHAR, owner_id UBIGINT,
            owner_type VARCHAR, authority_file VARCHAR, authority_dids VARCHAR[],
            checked_at TIMESTAMP NOT NULL);",
    )
    .unwrap();
    let storage = DuckDbStorageAdapter::open(&db).unwrap();
    match storage.repo_authority_check_adapter().probe() {
        ProbeOutcome::Refused { reason, .. } => assert_eq!(
            reason,
            ProbeRefusalReason::StorageRepoAuthorityCheckHistoryUnreliable
        ),
        ProbeOutcome::Ok => panic!("an observation table that cannot keep history must be refused"),
    }
}

#[test]
fn migration_v8_upgrades_a_v7_store_once_and_keeps_its_rows() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("openlore.duckdb");
    drop(DuckDbStorageAdapter::open(&db).unwrap());
    raw_sql(
        &db,
        "DROP TABLE repo_authority_checks; DROP SEQUENCE repo_authority_check_ids; \
         DELETE FROM schema_version WHERE version >= 8;",
    )
    .unwrap();

    let storage = DuckDbStorageAdapter::open(&db).unwrap();
    storage
        .repo_authority_check_adapter()
        .record(&observed(
            "github:acme/tool",
            1,
            RepoObservation::RepoNotFound,
        ))
        .unwrap();
    drop(storage);
    drop(DuckDbStorageAdapter::open(&db).unwrap());

    assert_eq!(
        count(&db, "SELECT COUNT(*) FROM schema_version WHERE version = 8"),
        1,
        "v8 recorded once"
    );
    assert_eq!(
        count(&db, "SELECT COUNT(*) FROM schema_version WHERE version = 7"),
        1,
        "v7 recorded once"
    );
    assert_eq!(stored_checks(&db), 1, "reopening keeps the observation");
}
