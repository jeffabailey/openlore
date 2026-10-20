//! ADR-088 Earned Trust: the v1 → v2 store update against a real DuckDB file
//! seeded with the v1 schema by SQL (U-RRA-4: a DID with suggestions and no
//! `github_links` row cannot be produced through the v1 app's pages).
//!
//! Universe: every table of the file, dumped whole, plus the backups beside
//! it. The update keeps every v1 row, copies each link to `github_accounts`,
//! backfills each suggestion's source (NULL account for a DID without a
//! link), records version 2 and leaves one dated v1 copy; a second start is
//! a no-op; a start cut off before COMMIT leaves v1 and the next start
//! migrates cleanly without touching the first copy.

use std::path::{Path, PathBuf};

use adapter_review_store::{DataKey, ReviewStore};
use duckdb::Connection;
use ports::{GithubLink, GithubLinkPort};

const PRIYA: &str = "did:plc:priyaraman7777777777777777";
const SAM: &str = "did:plc:samnolink88888888888888888";
const PRIYA_GITHUB_ID: i64 = 4_210_001;

/// The v1 schema as the v1 app wrote it (frozen: never follow the code).
const V1_SCHEMA: &str = "
CREATE TABLE schema_version (version BIGINT NOT NULL);
CREATE TABLE accounts (owner_did VARCHAR PRIMARY KEY, handle_last_seen VARCHAR NOT NULL,
    pds_endpoint_last_seen VARCHAR NOT NULL, created_at TIMESTAMPTZ NOT NULL,
    last_seen_at TIMESTAMPTZ NOT NULL);
CREATE TABLE github_links (owner_did VARCHAR PRIMARY KEY, github_user_id BIGINT NOT NULL,
    github_login VARCHAR NOT NULL,
    status VARCHAR NOT NULL CHECK (status IN ('verified', 'unverified')),
    verified_at TIMESTAMPTZ, last_checked_at TIMESTAMPTZ NOT NULL, last_verdict VARCHAR NOT NULL);
CREATE TABLE suggestions (owner_did VARCHAR NOT NULL, subject VARCHAR NOT NULL,
    predicate VARCHAR NOT NULL, object VARCHAR NOT NULL, confidence_bp INTEGER NOT NULL,
    evidence VARCHAR NOT NULL, signals VARCHAR NOT NULL, source_repo VARCHAR NOT NULL,
    state VARCHAR NOT NULL CHECK (state IN ('pending', 'declined', 'published', 'retracted')),
    first_suggested_at TIMESTAMPTZ NOT NULL, state_changed_at TIMESTAMPTZ NOT NULL,
    published_uri VARCHAR, published_cid VARCHAR, published_object VARCHAR,
    published_confidence_bp INTEGER, edited BOOLEAN NOT NULL DEFAULT false,
    retraction_cid VARCHAR, PRIMARY KEY (owner_did, subject, predicate, object));
CREATE TABLE scan_runs (owner_did VARCHAR NOT NULL, run_id VARCHAR NOT NULL,
    status VARCHAR NOT NULL CHECK (status IN
        ('running', 'completed', 'rate_limited', 'interrupted', 'ownership_failed')),
    repos_total INTEGER NOT NULL DEFAULT 0, repos_done INTEGER NOT NULL DEFAULT 0,
    new_count INTEGER NOT NULL DEFAULT 0, published_count INTEGER NOT NULL DEFAULT 0,
    declined_count INTEGER NOT NULL DEFAULT 0, resume_after TIMESTAMPTZ,
    started_at TIMESTAMPTZ NOT NULL, finished_at TIMESTAMPTZ, PRIMARY KEY (owner_did, run_id));
CREATE TABLE plans (owner_did VARCHAR NOT NULL, plan_id VARCHAR NOT NULL,
    kind VARCHAR NOT NULL CHECK (kind IN ('publish', 'retract', 'share')),
    suggestion_key VARCHAR, plan VARCHAR NOT NULL, created_at TIMESTAMPTZ NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL, PRIMARY KEY (owner_did, plan_id));
CREATE TABLE oauth_auth_requests (state VARCHAR PRIMARY KEY, owner_did_expected VARCHAR,
    issuer VARCHAR NOT NULL, enc_blob BLOB NOT NULL, created_at TIMESTAMPTZ NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL);
CREATE TABLE oauth_sessions (owner_did VARCHAR PRIMARY KEY, issuer VARCHAR NOT NULL,
    granted_scopes VARCHAR NOT NULL, enc_blob BLOB NOT NULL, updated_at TIMESTAMPTZ NOT NULL);
CREATE TABLE web_sessions (session_hash VARCHAR PRIMARY KEY, owner_did VARCHAR NOT NULL,
    csrf_token_hash VARCHAR NOT NULL, created_at TIMESTAMPTZ NOT NULL,
    last_used_at TIMESTAMPTZ NOT NULL, expires_at TIMESTAMPTZ NOT NULL);
CREATE TABLE kpi_counters (day DATE NOT NULL, event VARCHAR NOT NULL,
    count BIGINT NOT NULL DEFAULT 0, PRIMARY KEY (day, event));
INSERT INTO schema_version VALUES (1);
";

/// Priya: a link, three suggestions (pending, declined, published), a scan.
/// Sam: two suggestions and no link.
const V1_ROWS: &str = "
INSERT INTO accounts VALUES
    ('did:plc:priyaraman7777777777777777', 'priya.test', 'https://pds.test', now(), now()),
    ('did:plc:samnolink88888888888888888', 'sam.test', 'https://pds.test', now(), now());
INSERT INTO github_links VALUES ('did:plc:priyaraman7777777777777777', 4210001, 'priyaraman',
    'verified', TIMESTAMPTZ '2026-10-01 10:00:00+00', TIMESTAMPTZ '2026-10-02 10:00:00+00',
    'verified');
INSERT INTO suggestions (owner_did, subject, predicate, object, confidence_bp, evidence, signals,
    source_repo, state, first_suggested_at, state_changed_at) VALUES
    ('did:plc:priyaraman7777777777777777', 'github:priyaraman/tidepool', 'embodies', 'test-driven',
        7000, '[]', '[]', 'priyaraman/tidepool', 'pending', now(), now()),
    ('did:plc:priyaraman7777777777777777', 'github:priyaraman/driftwood', 'embodies',
        'dependency-pinning', 6000, '[]', '[]', 'priyaraman/driftwood', 'declined', now(), now()),
    ('did:plc:priyaraman7777777777777777', 'github:priyaraman/kelp', 'embodies',
        'dependency-pinning', 6500, '[]', '[]', 'priyaraman/kelp', 'published', now(), now()),
    ('did:plc:samnolink88888888888888888', 'github:sam/one', 'embodies', 'test-driven',
        5000, '[]', '[]', 'sam/one', 'pending', now(), now()),
    ('did:plc:samnolink88888888888888888', 'github:sam/two', 'embodies', 'memory-safety',
        5000, '[]', '[]', 'sam/two', 'declined', now(), now());
INSERT INTO scan_runs (owner_did, run_id, status, started_at) VALUES
    ('did:plc:priyaraman7777777777777777', 'run-1', 'completed', now());
INSERT INTO kpi_counters VALUES (DATE '2026-10-02', 'signin.completed', 2);
";

/// Every v1 table, dumped by the universe.
const V1_TABLES: [&str; 10] = [
    "schema_version",
    "accounts",
    "github_links",
    "suggestions",
    "scan_runs",
    "plans",
    "oauth_auth_requests",
    "oauth_sessions",
    "web_sessions",
    "kpi_counters",
];

/// Every v2 table (ADR-088 §2-§6).
const V2_TABLES: [&str; 4] = [
    "github_accounts",
    "repo_choices",
    "imported_repos",
    "scan_run_repos",
];

fn key() -> DataKey {
    DataKey::new("d1", [7; 32]).expect("key")
}

/// A v1 store file written by SQL, as the v1 app left it.
fn v1_store() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("review-app.duckdb");
    let conn = Connection::open(&path).expect("open v1");
    conn.execute_batch(V1_SCHEMA).expect("v1 schema");
    conn.execute_batch(V1_ROWS).expect("v1 rows");
    drop(conn);
    (dir, path)
}

fn query_strings(conn: &Connection, sql: &str) -> Vec<String> {
    let mut statement = conn.prepare(sql).expect(sql);
    let mut rows: Vec<String> = statement
        .query_map([], |row| {
            let columns = row.as_ref().column_count();
            (0..columns)
                .map(|i| Ok(format!("{:?}", row.get::<_, duckdb::types::Value>(i)?)))
                .collect::<Result<Vec<_>, _>>()
                .map(|values| values.join("|"))
        })
        .expect(sql)
        .collect::<Result<_, _>>()
        .expect(sql);
    rows.sort();
    rows
}

/// The whole file: each named table's rows, sorted.
fn dump(path: &Path, tables: &[&str]) -> Vec<(String, Vec<String>)> {
    let conn = Connection::open(path).expect("open for dump");
    tables
        .iter()
        .map(|t| {
            (
                t.to_string(),
                query_strings(&conn, &format!("SELECT * FROM {t}")),
            )
        })
        .collect()
}

fn single(path: &Path, sql: &str) -> Vec<String> {
    query_strings(&Connection::open(path).expect("open"), sql)
}

fn backups(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .expect("dir")
        .filter_map(Result::ok)
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| {
            n.strip_prefix("review-app.v1.")
                .and_then(|rest| rest.strip_suffix(".bak"))
                .is_some_and(|secs| !secs.is_empty() && secs.chars().all(|c| c.is_ascii_digit()))
        })
        .collect();
    names.sort();
    names
}

/// The v1 rows of the v1 tables, with v2's added suggestion columns dropped.
fn v1_rows(path: &Path) -> Vec<(String, Vec<String>)> {
    let conn = Connection::open(path).expect("open");
    V1_TABLES
        .iter()
        .filter(|t| **t != "schema_version")
        .map(|t| {
            let sql = match *t {
                "suggestions" => "SELECT * EXCLUDE (source_kind, source_account_id, \
                                  source_repo_id, outside_default) FROM suggestions"
                    .to_string(),
                "scan_runs" => "SELECT * EXCLUDE (summary) FROM scan_runs".to_string(),
                other => format!("SELECT * FROM {other}"),
            };
            (t.to_string(), query_strings(&conn, &sql))
        })
        .collect()
}

#[test]
fn the_update_keeps_every_row_copies_links_and_backfills_sources() {
    let (dir, path) = v1_store();
    let before = dump(&path, &V1_TABLES[1..]);

    let store = ReviewStore::open(&path, key()).expect("v1 migrates");
    assert_eq!(
        store.github_link(PRIYA).expect("read"),
        Some(GithubLink {
            github_login: "priyaraman".into(),
            github_user_id: 4_210_001,
            verified: true
        })
    );
    assert_eq!(store.github_link(SAM).expect("read"), None);
    drop(store);

    assert_eq!(v1_rows(&path), before, "every v1 row kept as it was");
    assert_eq!(
        single(&path, "SELECT max(version) FROM schema_version"),
        ["BigInt(2)"]
    );
    for table in V2_TABLES {
        single(&path, &format!("SELECT count(*) FROM {table}"));
    }
    assert_eq!(
        single(
            &path,
            "SELECT owner_did, github_user_id, github_login_seen, status, verified_at, \
             last_checked_at, last_verdict, added_at, eligible_count_seen FROM github_accounts"
        ),
        single(
            &path,
            "SELECT owner_did, github_user_id, github_login, status, verified_at, \
             last_checked_at, last_verdict, verified_at, NULL::INTEGER FROM github_links"
        ),
        "github_links copied 1:1"
    );
    assert_eq!(
        single(
            &path,
            "SELECT owner_did, source_kind, source_account_id, source_repo_id, outside_default \
             FROM suggestions"
        ),
        [
            format!("Text(\"{PRIYA}\")|Text(\"account\")|BigInt({PRIYA_GITHUB_ID})|Null|Null"),
            format!("Text(\"{PRIYA}\")|Text(\"account\")|BigInt({PRIYA_GITHUB_ID})|Null|Null"),
            format!("Text(\"{PRIYA}\")|Text(\"account\")|BigInt({PRIYA_GITHUB_ID})|Null|Null"),
            format!("Text(\"{SAM}\")|Text(\"account\")|Null|Null|Null"),
            format!("Text(\"{SAM}\")|Text(\"account\")|Null|Null|Null"),
        ],
        "each suggestion names its DID's account; no link, no account"
    );
    assert_eq!(single(&path, "SELECT summary FROM scan_runs"), ["Null"]);

    let copies = backups(dir.path());
    assert_eq!(copies.len(), 1, "one dated v1 copy: {copies:?}");
    let copy = dir.path().join(&copies[0]);
    assert_eq!(
        single(&copy, "SELECT max(version) FROM schema_version"),
        ["BigInt(1)"]
    );
    assert_eq!(
        dump(&copy, &V1_TABLES[1..]),
        before,
        "the copy is the v1 data"
    );
}

#[test]
fn a_second_start_changes_nothing() {
    let (dir, path) = v1_store();
    drop(ReviewStore::open(&path, key()).expect("first start migrates"));
    let all: Vec<&str> = V1_TABLES.iter().chain(V2_TABLES.iter()).copied().collect();
    let before = dump(&path, &all);
    let copies = backups(dir.path());
    let copy_bytes = std::fs::read(dir.path().join(&copies[0])).expect("copy");

    drop(ReviewStore::open(&path, key()).expect("second start"));

    assert_eq!(dump(&path, &all), before);
    assert_eq!(backups(dir.path()), copies, "no second copy");
    assert_eq!(
        std::fs::read(dir.path().join(&copies[0])).expect("copy"),
        copy_bytes,
        "the copy is never overwritten"
    );
}

#[test]
#[cfg(debug_assertions)]
fn a_start_cut_off_before_commit_leaves_v1_and_the_next_start_migrates() {
    let (dir, path) = v1_store();
    let before = dump(&path, &V1_TABLES);

    let cut_off = ReviewStore::open_capped_with(
        &path,
        key(),
        Default::default(),
        Some(adapter_review_store::MigrationFault::StopBeforeCommit),
    )
    .err();

    assert_eq!(
        cut_off,
        Some(adapter_review_store::StoreError::MigrationStopped)
    );
    assert_eq!(dump(&path, &V1_TABLES), before, "still v1, every row");
    assert!(
        single(
            &path,
            "SELECT table_name FROM information_schema.tables WHERE table_name = 'github_accounts'"
        )
        .is_empty(),
        "no v2 table survived the cut-off"
    );
    let first = backups(dir.path());
    assert_eq!(first.len(), 1, "the cut-off start copied the v1 data first");
    let first_bytes = std::fs::read(dir.path().join(&first[0])).expect("copy");

    let store = ReviewStore::open(&path, key()).expect("the next start migrates");
    assert_eq!(
        store
            .github_link(PRIYA)
            .expect("read")
            .map(|l| l.github_user_id),
        Some(4_210_001)
    );
    drop(store);
    assert_eq!(
        single(&path, "SELECT max(version) FROM schema_version"),
        ["BigInt(2)"]
    );
    assert_eq!(
        backups(dir.path()),
        first,
        "R5: the cut-off start's copy is reused, no second copy"
    );
    assert_eq!(
        std::fs::read(dir.path().join(&first[0])).expect("copy"),
        first_bytes,
        "the first copy is never overwritten"
    );
}

/// R5: a crash-looping start (every one cut off before COMMIT) keeps
/// exactly one copy, the v1 data, and the store stays v1.
#[test]
#[cfg(debug_assertions)]
fn crash_looping_starts_keep_exactly_one_copy() {
    let (dir, path) = v1_store();
    let before = dump(&path, &V1_TABLES);
    for _ in 0..3 {
        let cut_off = ReviewStore::open_capped_with(
            &path,
            key(),
            Default::default(),
            Some(adapter_review_store::MigrationFault::StopBeforeCommit),
        );
        assert!(cut_off.is_err());
    }
    let copies = backups(dir.path());
    assert_eq!(copies.len(), 1, "{copies:?}");
    let copy = dir.path().join(&copies[0]);
    assert_eq!(
        single(&copy, "SELECT max(version) FROM schema_version"),
        ["BigInt(1)"]
    );
    assert_eq!(dump(&copy, &V1_TABLES), before, "the copy is the v1 data");
    assert_eq!(dump(&path, &V1_TABLES), before, "the store is still v1");
}

/// R5 (user decision 2026-10-19): once the upgraded store is open, its v1
/// copies are deleted, and another file beside it is left alone; a second
/// start of the v2 store makes no copy and deletes nothing.
#[test]
fn the_v1_copy_is_deleted_after_the_upgrade() {
    let (dir, path) = v1_store();
    let neighbour = dir.path().join("review-app.v1.notes.bak");
    std::fs::write(&neighbour, b"not a copy").expect("write");
    let store = ReviewStore::open(&path, key()).expect("the start migrates");
    assert_eq!(backups(dir.path()).len(), 1, "copied before migrating");

    assert_eq!(store.discard_v1_backups(&path).expect("discard"), 1);
    drop(store);

    assert!(backups(dir.path()).is_empty());
    assert!(neighbour.exists(), "only the store's dated copies go");
    let again = ReviewStore::open(&path, key()).expect("second start");
    assert_eq!(again.discard_v1_backups(&path).expect("discard"), 0);
    assert!(backups(dir.path()).is_empty());
}

/// A v1 copy the folder refuses fails the start with the folder's own
/// reason, not as if every backup name were taken; nothing is migrated.
#[test]
#[cfg(unix)]
fn a_copy_the_folder_refuses_fails_with_the_folders_reason() {
    use std::os::unix::fs::PermissionsExt;
    let (dir, path) = v1_store();
    let before = dump(&path, &V1_TABLES);
    let mode = |m| std::fs::Permissions::from_mode(m);
    std::fs::set_permissions(dir.path(), mode(0o555)).expect("read-only folder");
    let folder_refuses = std::fs::write(dir.path().join("probe"), b"").is_err();
    let opened = ReviewStore::open(&path, key()).err();
    std::fs::set_permissions(dir.path(), mode(0o755)).expect("writable again");
    if !folder_refuses {
        return; // root ignores folder permissions: nothing to observe
    }
    match opened {
        Some(adapter_review_store::StoreError::Backup(reason)) => {
            assert_ne!(reason, "every backup name is taken")
        }
        other => panic!("expected the folder's refusal, got {other:?}"),
    }
    assert!(backups(dir.path()).is_empty());
    assert_eq!(dump(&path, &V1_TABLES), before, "the store is still v1");
}
