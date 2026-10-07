//! indexer-deployment CORE-11 — moved VERBATIM (view type, generators, oracle,
//! binding, property) from `tests/acceptance/indexer_deployment_core.rs`
//! (roadmap review F2): the `cli` test target may not link this crate
//! (check-arch CLI_FORBIDDEN_INDEXER_DEPS), and the property binds to the REAL
//! adapter over a temp DuckDB (the shape of `atomic_upsert_properties`).
//!
//! RED scaffold: DELIVER 02-02 replaces the `sut_purge_author` body: seed a temp
//! index.duckdb + artifact dir from `store` through this crate, call
//! `IndexPurgePort::purge_author(bare)`, read it back (ADR-082, data-models §3).
//
// SCAFFOLD: true

#[path = "../../../tests/common/state_delta.rs"]
mod state_delta;

use std::collections::{BTreeSet, HashMap, HashSet};

use proptest::prelude::*;
use state_delta::{assert_state_delta, set_to, Delta};

/// One store as the purge universe sees it (data-models §3).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct StoreView {
    /// (author_did, cid, signed_record_path)
    claims: BTreeSet<(String, String, String)>,
    /// (cid, evidence url)
    evidence: BTreeSet<(String, String)>,
    /// (referencing_cid, referenced_cid)
    references: BTreeSet<(String, String)>,
    /// artifact file paths present on disk
    artifacts: BTreeSet<String>,
}

fn sut_purge_author(store: &StoreView, bare: &str) -> (StoreView, u64) {
    let dir = tempfile::tempdir().expect("temp index dir");
    let db_path = dir.path().join("index.duckdb");
    // The real adapter creates the schema; the rows and files are then laid
    // down exactly as the view states them (stored paths included).
    drop(adapter_index_store::IndexStoreAdapter::open(&db_path).expect("open index store"));
    seed(&db_path, dir.path(), store);
    let adapter = adapter_index_store::IndexStoreAdapter::open(&db_path).expect("reopen");
    let report = ports::IndexPurgePort::purge_author(&adapter, bare).expect("purge_author");
    drop(adapter);
    (read_back(&db_path, dir.path()), report.claims_removed)
}

fn seed(db_path: &std::path::Path, index_dir: &std::path::Path, store: &StoreView) {
    let conn = duckdb::Connection::open(db_path).expect("open seed connection");
    for (author, cid, path) in &store.claims {
        conn.execute(
            "INSERT INTO indexed_claims (cid, author_did, subject, predicate, object, confidence, \
             composed_at, indexed_at, source_pds, signed_record_path, verified_against) \
             VALUES (?, ?, 'subject', 'predicate', 'object', 0.5, now(), now(), 'network', ?, 'key')",
            duckdb::params![cid, author, path],
        )
        .expect("seed claim");
        let file = index_dir.join(path);
        std::fs::create_dir_all(file.parent().expect("partition")).expect("partition dir");
        std::fs::write(&file, cid).expect("seed artifact");
    }
    for (ordinal, (cid, evidence)) in store.evidence.iter().enumerate() {
        conn.execute(
            "INSERT INTO indexed_claim_evidence (cid, evidence, ordinal) VALUES (?, ?, ?)",
            duckdb::params![cid, evidence, ordinal as i32],
        )
        .expect("seed evidence");
    }
    for (referencing, referenced) in &store.references {
        conn.execute(
            "INSERT INTO indexed_claim_references (referencing_cid, referenced_cid, ref_type) \
             VALUES (?, ?, 'counters')",
            duckdb::params![referencing, referenced],
        )
        .expect("seed reference");
    }
}

fn read_back(db_path: &std::path::Path, index_dir: &std::path::Path) -> StoreView {
    let conn = duckdb::Connection::open(db_path).expect("open read-back connection");
    let claims = rows(
        &conn,
        "SELECT author_did, cid, signed_record_path FROM indexed_claims",
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    );
    let evidence = rows(
        &conn,
        "SELECT cid, evidence FROM indexed_claim_evidence",
        |r| Ok((r.get(0)?, r.get(1)?)),
    );
    let references = rows(
        &conn,
        "SELECT referencing_cid, referenced_cid FROM indexed_claim_references",
        |r| Ok((r.get(0)?, r.get(1)?)),
    );
    let artifacts = walkdir(&index_dir.join("indexed_claims"))
        .into_iter()
        .map(|file| {
            file.strip_prefix(index_dir)
                .expect("under the index dir")
                .display()
                .to_string()
        })
        .collect();
    StoreView {
        claims,
        evidence,
        references,
        artifacts,
    }
}

fn rows<T: Ord>(
    conn: &duckdb::Connection,
    sql: &str,
    map: impl FnMut(&duckdb::Row<'_>) -> duckdb::Result<T>,
) -> BTreeSet<T> {
    let mut stmt = conn.prepare(sql).expect("prepare read-back");
    stmt.query_map([], map)
        .expect("read back")
        .map(|row| row.expect("decode"))
        .collect()
}

fn walkdir(dir: &std::path::Path) -> Vec<std::path::PathBuf> {
    std::fs::read_dir(dir)
        .map(|entries| {
            entries
                .flatten()
                .flat_map(|entry| {
                    let path = entry.path();
                    if path.is_dir() {
                        walkdir(&path)
                    } else {
                        vec![path]
                    }
                })
                .collect()
        })
        .unwrap_or_default()
}

/// BareDid: the text of an `author_did` before the first `#`.
fn bare(author_did: &str) -> String {
    author_did.split('#').next().unwrap_or("").to_string()
}

fn purge_oracle(store: &StoreView, target: &str) -> (StoreView, u64) {
    let purged: BTreeSet<(String, String, String)> = store
        .claims
        .iter()
        .filter(|(author, _, _)| bare(author) == target)
        .cloned()
        .collect();
    let cids: BTreeSet<&String> = purged.iter().map(|(_, cid, _)| cid).collect();
    let paths: BTreeSet<&String> = purged.iter().map(|(_, _, p)| p).collect();
    let after = StoreView {
        claims: store.claims.difference(&purged).cloned().collect(),
        evidence: store
            .evidence
            .iter()
            .filter(|(cid, _)| !cids.contains(cid))
            .cloned()
            .collect(),
        references: store
            .references
            .iter()
            .filter(|(referencing, _)| !cids.contains(referencing))
            .cloned()
            .collect(),
        artifacts: store
            .artifacts
            .iter()
            .filter(|p| !paths.contains(p))
            .cloned()
            .collect(),
    };
    (after, purged.len() as u64)
}

/// A small DID pool with deliberate look-alikes: a prefix pair and a did:web
/// pair whose filesystem segments collide (`did_to_fs_segment` is not injective).
const DID_POOL: [&str; 8] = [
    "did:plc:priyaraman7x2k",
    "did:plc:priyaraman7x2k_x",
    "did:plc:dvolkov3m9q",
    "did:plc:jeffbailey5n2p",
    "did:plc:therrera2v6w",
    "did:web:a:b",
    "did:web:a_b",
    "did:web:example.com",
];

fn did() -> impl Strategy<Value = String> {
    prop::sample::select(DID_POOL.to_vec()).prop_map(str::to_string)
}

fn indexed_author_id() -> impl Strategy<Value = String> {
    (did(), any::<bool>()).prop_map(|(d, app)| {
        if app {
            format!("{d}#org.openlore.application")
        } else {
            d
        }
    })
}

fn segment(author_did: &str) -> String {
    author_did.split('#').next().unwrap_or("").replace(':', "_")
}

/// A store of 0..12 claims by pool authors, with evidence, cross-author
/// references (including references TO claims that will be purged) and one
/// artifact file per claim at its stored `signed_record_path`.
fn store() -> impl Strategy<Value = StoreView> {
    prop::collection::vec(
        (
            indexed_author_id(),
            0u8..3,
            prop::collection::vec(0usize..12, 0..3),
        ),
        0..12,
    )
    .prop_map(|rows| {
        let mut s = StoreView::default();
        for (i, (author, evidence, refs)) in rows.iter().enumerate() {
            let cid = format!("bafyclaim{i:03}");
            let path = format!("indexed_claims/{}/{cid}.json", segment(author));
            s.claims.insert((author.clone(), cid.clone(), path.clone()));
            s.artifacts.insert(path);
            for e in 0..*evidence {
                s.evidence
                    .insert((cid.clone(), format!("https://example.org/evidence/{i}/{e}")));
            }
            for r in refs {
                if *r < rows.len() && *r != i {
                    s.references
                        .insert((cid.clone(), format!("bafyclaim{r:03}")));
                }
            }
        }
        s
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    /// CORE-11 @US-IXD-003 @AC-003.2 @ADR-082 @property @contract-shape:bounded-change
    /// Purging one bare DID changes exactly: its claims (bare and `#fragment`
    /// forms), their evidence and outgoing references, and the artifact files
    /// at THEIR stored paths. Look-alike DIDs (prefix pair, colliding did:web
    /// segments) and references other authors hold TO purged claims are
    /// untouched. Purging again removes nothing (idempotent, C4a).
    #[test]
    fn purging_an_author_changes_only_that_author_s_claims_and_files(
        before in store(),
        target in did(),
    ) {
        let (after, removed) = sut_purge_author(&before, &target);
        let (expected, expected_removed) = purge_oracle(&before, &target);
        prop_assert_eq!(removed, expected_removed);
        let snap = |s: &StoreView| -> HashMap<String, Vec<String>> {
            HashMap::from([
                ("index.claims".to_string(), s.claims.iter().map(|c| format!("{c:?}")).collect()),
                ("index.evidence".to_string(), s.evidence.iter().map(|c| format!("{c:?}")).collect()),
                ("index.references".to_string(), s.references.iter().map(|c| format!("{c:?}")).collect()),
                ("index.artifact_files".to_string(), s.artifacts.iter().cloned().collect()),
            ])
        };
        let (b, a, e) = (snap(&before), snap(&after), snap(&expected));
        let universe: HashSet<String> = b.keys().cloned().collect();
        let delta = universe.iter().fold(Delta::new(), |d, slot| d.with_slot(slot.clone(), set_to(e[slot].clone())));
        assert_state_delta(&b, &a, &universe, &delta);

        let (again, removed_again) = sut_purge_author(&after, &target);
        prop_assert_eq!(removed_again, 0);
        prop_assert_eq!(again, after);
    }
}
