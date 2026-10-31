//! `IndexStoreAdapter::read_through` over real, really damaged files: flipping
//! bytes leaves the length as it was, so only a read that reaches a block can tell.
//! DuckDB keeps a checksum per block, and only checks it when the block is read.

use std::fs;
use std::path::Path;

use adapter_index_store::IndexStoreAdapter;
use duckdb::Connection;

/// DuckDB's three header pages, then its 256 KiB blocks.
const HEADER_BYTES: usize = 12_288;
const BLOCK_BYTES: usize = 262_144;

/// A table too big for one block, so each column's data has blocks of its own.
const ROWS: &str = "\
    INSERT INTO indexed_claims SELECT 'cid' || i, 'did:plc:a' || (i % 9), \
        'github:x/' || md5(i::VARCHAR), md5((i + 1)::VARCHAR), 'object' || (i % 500), i / 40000.0, \
        TIMESTAMP '2026-05-26 12:00:00' + INTERVAL (i) MINUTE, now(), 'network', 'path', \
        md5((i + 2)::VARCHAR), 'app-signed' \
    FROM range(40000) AS rows(i); \
    INSERT INTO indexed_claim_references SELECT 'cid' || i, 'cid' || ((i * 7) % 40000), 'counters' \
    FROM range(1, 8000) AS rows(i); \
    CHECKPOINT;";

/// Every row of every column a search reads, by a road that shares nothing with
/// `read_through`, so a block it should reach and does not shows.
const ROWS_A_SEARCH_READS: [&str; 2] = [
    "SELECT author_did, cid, subject, predicate, object, confidence, composed_at, \
         verified_against, provenance FROM indexed_claims",
    "SELECT referencing_cid, referenced_cid, ref_type FROM indexed_claim_references",
];

fn a_published_index(path: &Path) -> Vec<u8> {
    drop(IndexStoreAdapter::open(path).expect("a new store"));
    let connection = Connection::open(path).expect("the file");
    connection.execute_batch(ROWS).expect("the rows");
    drop(connection);
    fs::read(path).expect("the file")
}

fn with_block_damaged(index: &[u8], block: usize) -> Vec<u8> {
    let mut damaged = index.to_vec();
    let start = HEADER_BYTES + block * BLOCK_BYTES + 5_000;
    damaged[start..start + 32]
        .iter_mut()
        .for_each(|byte| *byte ^= 0xA5);
    damaged
}

fn every_row_reads(path: &Path) -> bool {
    let connection = Connection::open(path).expect("the file opens");
    ROWS_A_SEARCH_READS.iter().all(|sql| {
        let mut statement = connection.prepare(sql).expect("a statement");
        statement
            .query_map([], |_| Ok(()))
            .is_ok_and(|rows| rows.collect::<Result<Vec<()>, _>>().is_ok())
    })
}

// bypass: real DuckDB over one fixed file; the fixed fact is that nothing is wrong.
#[test]
fn a_whole_index_and_a_new_one_read_through() {
    let scratch = tempfile::tempdir().expect("a scratch directory");
    let index = scratch.path().join("index.duckdb");
    a_published_index(&index);

    let whole = IndexStoreAdapter::open(&index).expect("the store");
    let new = IndexStoreAdapter::open(&scratch.path().join("sub").join("new.duckdb"))
        .expect("a new store");

    assert!(whole.read_through().is_ok(), "a whole index");
    assert!(new.read_through().is_ok(), "a new, empty index");
}

// bypass: a sweep of every block of one real file, the input space of this
// contract; a generated file would only move the blocks around.
#[test]
fn a_damaged_block_of_any_row_a_search_reads_fails_the_read_through() {
    let scratch = tempfile::tempdir().expect("a scratch directory");
    let published = a_published_index(&scratch.path().join("published.duckdb"));
    let blocks = (published.len() - HEADER_BYTES) / BLOCK_BYTES;
    let copy = scratch.path().join("copy.duckdb");
    let mut unread_rows_seen = 0;

    for block in 0..blocks {
        fs::write(&copy, with_block_damaged(&published, block)).expect("a damaged copy");
        let Ok(store) = IndexStoreAdapter::open(&copy) else {
            continue;
        };
        let read_through = store.read_through().is_ok();
        drop(store);

        if !every_row_reads(&copy) {
            unread_rows_seen += 1;
            assert!(!read_through, "block {block} holds rows a search reads");
        }
    }

    assert!(unread_rows_seen > 1, "the sweep reached the tables' blocks");
}
