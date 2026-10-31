//! Real index files for this crate's tests.

use std::path::Path;

use adapter_index_store::IndexStoreAdapter;
use claim_domain::{Cid, Did, KeyId};
use ports::{AuthorRelationship, IndexReadPort, IndexStorePort, IndexedClaim, PeerClaimProvenance};

/// The subject of every claim here.
pub(crate) const SUBJECT: &str = "https://example.invalid/a";

/// DuckDB's three header pages, then its 256 KiB blocks, each under a checksum.
const HEADER_BYTES: usize = 12_288;
const BLOCK_BYTES: usize = 262_144;

/// Text long enough to need blocks of its own, beyond the one a store's open reads.
fn a_long_predicate() -> String {
    (0..60_000).map(|word| format!("{word:x}.")).collect()
}

pub(crate) fn a_claim(cid: &str) -> IndexedClaim {
    IndexedClaim {
        author_did: Did("did:plc:priya".to_string()),
        cid: Cid(cid.to_string()),
        subject: SUBJECT.to_string(),
        predicate: a_long_predicate(),
        object: "org.openlore.philosophy.reproducible-builds".to_string(),
        confidence: 0.8,
        composed_at: "2026-10-27T14:00:00Z".parse().expect("a fixed instant"),
        verified_against: KeyId("did:plc:priya".to_string()),
        evidence: Vec::new(),
        references: Vec::new(),
        relationship: AuthorRelationship::NetworkUnfollowed,
        provenance: PeerClaimProvenance::AppSigned,
    }
}

/// The bytes of an index holding one claim, as a pass would publish it.
pub(crate) fn a_published_index(cid: &str) -> Vec<u8> {
    let scratch = tempfile::tempdir().expect("a scratch directory");
    let index = scratch.path().join("index.duckdb");
    let store = IndexStoreAdapter::open(&index).expect("a store");
    store.upsert(&a_claim(cid)).expect("a claim");
    drop(store);
    std::fs::read(&index).expect("the file")
}

/// The object a length check cannot tell from a whole one: a real index, the same
/// length, with bytes flipped in one block, which still opens and fails a search.
pub(crate) fn with_a_damaged_block(published: &[u8]) -> Vec<u8> {
    let blocks = (published.len() - HEADER_BYTES) / BLOCK_BYTES;
    (0..blocks)
        .map(|block| damaged_at(published, block))
        .find(|damaged| opens_and_fails_a_search(damaged))
        .expect("a block that a search reads")
}

fn damaged_at(published: &[u8], block: usize) -> Vec<u8> {
    let mut damaged = published.to_vec();
    let start = HEADER_BYTES + block * BLOCK_BYTES + 5_000;
    damaged[start..start + 32]
        .iter_mut()
        .for_each(|byte| *byte ^= 0xA5);
    damaged
}

fn opens_and_fails_a_search(index: &[u8]) -> bool {
    let scratch = tempfile::tempdir().expect("a scratch directory");
    let path = scratch.path().join("index.duckdb");
    std::fs::write(&path, index).expect("a copy");
    store_opens_and_fails_a_search(&path)
}

fn store_opens_and_fails_a_search(path: &Path) -> bool {
    IndexStoreAdapter::open(path).is_ok_and(|store| store.query_by_subject(SUBJECT).is_err())
}
