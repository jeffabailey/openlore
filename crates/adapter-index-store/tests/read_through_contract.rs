//! `IndexStoreAdapter::read_through` over real, really damaged files: flipping
//! bytes leaves the length as it was, so only a read that reaches a block can tell.
//! DuckDB keeps a checksum per block, and only checks it when the block is read.

use std::fs;
use std::path::{Path, PathBuf};

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

/// What each column of a read through holds, varied enough to need blocks of its
/// own: `ROWS` gives `ref_type` and `provenance` one value each, and a column of one
/// value is stored with no block at all. `ref_type` has a CHECK, so it takes its four
/// values in no pattern, over enough rows to outgrow a small block.
const ROWS_IN_EVERY_COLUMN: &str = "\
    INSERT INTO indexed_claims SELECT 'cid' || i, 'did:plc:a' || (i % 9), \
        'github:x/' || md5(i::VARCHAR), md5((i + 1)::VARCHAR), 'object' || (i % 500), i / 40000.0, \
        TIMESTAMP '2026-05-26 12:00:00' + INTERVAL (i) MINUTE, now(), 'network', 'path', \
        md5((i + 2)::VARCHAR), md5((i + 3)::VARCHAR) \
    FROM range(40000) AS rows(i); \
    INSERT INTO indexed_claim_references SELECT 'cid' || (i % 40000), 'cid' || i, \
        ['retracts', 'corrects', 'counters', 'supersedes'][(1 + hash(i) % 4)::BIGINT] \
    FROM range(1, 200000) AS rows(i); \
    CHECKPOINT;";

/// DuckDB's smallest block. At the 256 KiB default, a column of four values would
/// take millions of rows to fill one.
const SMALL_BLOCK_BYTES: usize = 16_384;

/// Each column's data, by name, across the tables a search reads.
const COLUMN_OF_EACH_BLOCK: &str = "\
    SELECT DISTINCT 'indexed_claims.' || column_name, block_id \
    FROM pragma_storage_info('indexed_claims') WHERE block_id >= 0 \
    UNION \
    SELECT DISTINCT 'indexed_claim_references.' || column_name, block_id \
    FROM pragma_storage_info('indexed_claim_references') WHERE block_id >= 0";

/// Every column a search reads (`ROWS_A_SEARCH_READS`), as `COLUMN_OF_EACH_BLOCK`
/// names it.
const COLUMNS_A_SEARCH_READS: [&str; 12] = [
    "indexed_claims.author_did",
    "indexed_claims.cid",
    "indexed_claims.subject",
    "indexed_claims.predicate",
    "indexed_claims.object",
    "indexed_claims.confidence",
    "indexed_claims.composed_at",
    "indexed_claims.verified_against",
    "indexed_claims.provenance",
    "indexed_claim_references.referencing_cid",
    "indexed_claim_references.referenced_cid",
    "indexed_claim_references.ref_type",
];

/// An index in small blocks, with the store's own schema.
fn an_index_in_small_blocks(scratch: &Path) -> PathBuf {
    let schema = scratch.join("schema.duckdb");
    let index = scratch.join("small-blocks.duckdb");
    drop(IndexStoreAdapter::open(&schema).expect("a new store"));
    Connection::open_in_memory()
        .expect("a connection")
        .execute_batch(&format!(
            "ATTACH '{}' AS schema_only; \
             ATTACH '{}' AS small (BLOCK_SIZE {SMALL_BLOCK_BYTES}); \
             COPY FROM DATABASE schema_only TO small; \
             USE small; {ROWS_IN_EVERY_COLUMN}",
            schema.display(),
            index.display(),
        ))
        .expect("the rows");
    index
}

/// One block that holds `column` and nothing else, so only a read of that column
/// can find it damaged. `None` when every block it uses is shared.
fn a_block_of_its_own(blocks_of_columns: &[(String, i64)], column: &str) -> Option<i64> {
    let users = |block: i64| {
        blocks_of_columns
            .iter()
            .filter(|(_, used)| *used == block)
            .count()
    };
    blocks_of_columns
        .iter()
        .find(|(name, block)| name == column && users(*block) == 1)
        .map(|(_, block)| *block)
}

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

// bypass: the twelve columns are the input space, each against its own block of one
// real file. The sweep above cannot tell a column skipped from a column read: a
// shared block fails through its neighbour, and a one-value column has no block.
#[test]
fn a_damaged_block_only_one_column_uses_fails_the_read_through() {
    let scratch = tempfile::tempdir().expect("a scratch directory");
    let index = an_index_in_small_blocks(scratch.path());
    let whole = fs::read(&index).expect("the file");
    let blocks_of_columns: Vec<(String, i64)> = {
        let connection = Connection::open(&index).expect("the file");
        let mut statement = connection
            .prepare(COLUMN_OF_EACH_BLOCK)
            .expect("a statement");
        statement
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .expect("the blocks")
            .collect::<Result<_, _>>()
            .expect("the blocks")
    };
    let copy = scratch.path().join("copy.duckdb");

    for column in COLUMNS_A_SEARCH_READS {
        let block = a_block_of_its_own(&blocks_of_columns, column)
            .unwrap_or_else(|| panic!("{column} has no block of its own"));
        let start = HEADER_BYTES + block as usize * SMALL_BLOCK_BYTES + SMALL_BLOCK_BYTES / 2;
        let mut damaged = whole.clone();
        damaged[start..start + 32]
            .iter_mut()
            .for_each(|byte| *byte ^= 0xA5);
        fs::write(&copy, damaged).expect("a damaged copy");

        let store = IndexStoreAdapter::open(&copy).expect("an open reads no column's data");

        assert!(store.read_through().is_err(), "{column}, block {block}");
    }
}
