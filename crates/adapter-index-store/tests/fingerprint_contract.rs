//! scale-to-zero-hosting FP-1..5 — the index fingerprint the Lambda pass
//! compares before and after a run (D-STZ-8): (claim count, distinct author
//! count, XOR of a 64-bit hash of every CID), read by one query from the REAL
//! adapter over a temp DuckDB.
//!
//! Lives in this crate (not `tests/acceptance`), like CORE-11's
//! `purge_properties.rs`: the `cli` test target may not link the index store.
//! Layer 3 (real DuckDB) → example-based (Mandate 9); the XOR's order
//! independence is covered by permutations of a fixed set.

use adapter_index_store::IndexStoreAdapter;
use chrono::{DateTime, Utc};
use claim_domain::{Cid, Did, KeyId};
use ports::{
    AuthorRelationship, IndexFingerprint, IndexPurgePort, IndexStorePort, IndexedClaim,
    PeerClaimProvenance,
};

fn claim(author: &str, cid: &str, subject: &str) -> IndexedClaim {
    IndexedClaim {
        author_did: Did(author.to_string()),
        cid: Cid(cid.to_string()),
        subject: subject.to_string(),
        predicate: "embodiesPhilosophy".to_string(),
        object: "org.openlore.philosophy.reproducible-builds".to_string(),
        confidence: 0.82,
        composed_at: "2026-10-27T13:02:11Z"
            .parse::<DateTime<Utc>>()
            .expect("fixed RFC 3339"),
        verified_against: KeyId(author.to_string()),
        evidence: vec![format!(
            "https://{}",
            subject.replace("github:", "github.com/")
        )],
        references: Vec::new(),
        relationship: AuthorRelationship::NetworkUnfollowed,
        provenance: PeerClaimProvenance::SelfAttested,
    }
}

fn priya_cargo_pin() -> IndexedClaim {
    claim(
        "did:plc:priyaraman7x2k",
        "bafypriyacargopin0001",
        "github:priyaraman/cargo-pin",
    )
}

fn priya_tidepool() -> IndexedClaim {
    claim(
        "did:plc:priyaraman7x2k",
        "bafypriyatidepool0002",
        "github:priyaraman/tidepool",
    )
}

fn dmitri_ferrite() -> IndexedClaim {
    claim(
        "did:plc:dvolkov3m9q",
        "bafydmitriferrite0003",
        "github:dvolkov/ferrite",
    )
}

/// The fingerprint of a fresh store after upserting `claims` in this order.
fn fingerprint_after(claims: &[IndexedClaim]) -> IndexFingerprint {
    let dir = tempfile::tempdir().expect("temp index dir");
    let store = IndexStoreAdapter::open(&dir.path().join("index.duckdb")).expect("open");
    for c in claims {
        store.upsert(c).expect("upsert");
    }
    store.fingerprint().expect("fingerprint")
}

/// FP-1
/// ```gherkin
/// @real-io @D-STZ-8 @C1a @C3 @contract-shape:pure-function
/// Scenario: An empty index has the zero fingerprint, and counts claims and authors
///   Given an index with 0, 1 and then 3 claims by 2 authors
///   Then the fingerprint counts 0/0, 1/1 and 3/2
/// ```
#[test]
fn an_empty_index_has_the_zero_fingerprint_and_counts_claims_and_authors() {
    let empty = fingerprint_after(&[]);
    let one = fingerprint_after(&[priya_cargo_pin()]);
    let three = fingerprint_after(&[priya_cargo_pin(), priya_tidepool(), dmitri_ferrite()]);

    assert_eq!(empty, IndexFingerprint::default());
    assert_eq!((one.claims, one.authors), (1, 1));
    assert_ne!(one.cid_hash_xor, 0, "a CID contributes its hash");
    assert_eq!((three.claims, three.authors), (3, 2));
}

/// FP-2
/// ```gherkin
/// @real-io @D-STZ-8 @C4a @contract-shape:unbounded-preservation
/// Scenario: Indexing the same claim again does not change the fingerprint
///   Given an index holding Priya's cargo-pin claim
///   When the pass upserts the same claim again
///   Then the fingerprint is unchanged
/// ```
#[test]
fn indexing_the_same_claim_again_does_not_change_the_fingerprint() {
    let dir = tempfile::tempdir().expect("temp index dir");
    let store = IndexStoreAdapter::open(&dir.path().join("index.duckdb")).expect("open");
    store.upsert(&priya_cargo_pin()).expect("upsert");
    let before = store.fingerprint().expect("fingerprint");

    store.upsert(&priya_cargo_pin()).expect("re-upsert");

    assert_eq!(store.fingerprint().expect("fingerprint"), before);
}

/// FP-3
/// ```gherkin
/// @real-io @D-STZ-8 @contract-shape:pure-function
/// Scenario: The fingerprint does not depend on the order claims were indexed in
///   Given the same three claims indexed in every order
///   Then every order gives the same fingerprint
/// ```
#[test]
fn the_fingerprint_does_not_depend_on_the_order_claims_were_indexed_in() {
    let (a, b, c) = (priya_cargo_pin(), priya_tidepool(), dmitri_ferrite());
    let orders = [
        vec![a.clone(), b.clone(), c.clone()],
        vec![a.clone(), c.clone(), b.clone()],
        vec![b.clone(), a.clone(), c.clone()],
        vec![b.clone(), c.clone(), a.clone()],
        vec![c.clone(), a.clone(), b.clone()],
        vec![c, b, a],
    ];
    let prints: Vec<IndexFingerprint> = orders.iter().map(|o| fingerprint_after(o)).collect();
    assert!(prints.windows(2).all(|w| w[0] == w[1]), "{prints:?}");
}

/// FP-4
/// ```gherkin
/// @real-io @D-STZ-8 @ADR-082 @C4b @contract-shape:bounded-change
/// Scenario: Adding a claim changes the fingerprint, and purging its author restores it
///   Given an index holding Priya's cargo-pin claim
///   When Dmitri's ferrite claim is indexed, and then Dmitri is purged
///   Then the fingerprint changed after the add and is back to the original after the purge
/// ```
#[test]
fn adding_a_claim_changes_the_fingerprint_and_purging_its_author_restores_it() {
    let dir = tempfile::tempdir().expect("temp index dir");
    let store = IndexStoreAdapter::open(&dir.path().join("index.duckdb")).expect("open");
    store.upsert(&priya_cargo_pin()).expect("upsert");
    let original = store.fingerprint().expect("fingerprint");

    store.upsert(&dmitri_ferrite()).expect("upsert");
    let added = store.fingerprint().expect("fingerprint");
    store.purge_author("did:plc:dvolkov3m9q").expect("purge");
    let purged = store.fingerprint().expect("fingerprint");

    assert_ne!(added, original);
    assert_eq!(purged, original);
}

/// FP-5
/// ```gherkin
/// @real-io @D-STZ-8 @C1b @contract-shape:pure-function
/// Scenario: Two indexes with the same counts but different claims have different fingerprints
///   Given one index holding Priya's cargo-pin claim and another holding her tidepool claim
///   Then both count 1 claim by 1 author, and their fingerprints differ
/// ```
#[test]
fn two_indexes_with_the_same_counts_but_different_claims_have_different_fingerprints() {
    let one = fingerprint_after(&[priya_cargo_pin()]);
    let other = fingerprint_after(&[priya_tidepool()]);

    assert_eq!((one.claims, one.authors), (other.claims, other.authors));
    assert_ne!(one, other);
}
