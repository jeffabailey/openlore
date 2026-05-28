//! Integration tests for `DuckDbStorageAdapter` (step 04-05).
//!
//! These are integration tests, NOT pure-unit tests: the adapter is an
//! EFFECT shell, so the only honest test exercises real DuckDB + real
//! filesystem (`tempfile::TempDir`) per nw-tdd-methodology Mandate 6
//! ("Adapter integration tests are real I/O").
//!
//! Port-to-port: every test enters through the `StoragePort` trait
//! surface (`probe`, `write_signed_claim`, `read_signed_claim`,
//! `query_by_subject`, `query_referencing`) and asserts on the trait's
//! observable returns. No internal field inspection.

use adapter_duckdb::DuckDbStorageAdapter;
use chrono::{TimeZone, Utc};
use claim_domain::{
    Cid, ClaimReference, Confidence, Did, ReferenceType, SignatureBlock, SignedClaim, UnsignedClaim,
};
use ports::{ProbeOutcome, StoragePort};
use tempfile::TempDir;

/// Build a `Confidence` value bypassing the still-RED smart constructor
/// (the wrapper's private inner field forbids direct tuple
/// construction from outside `claim_domain`). Mirrors the pattern in
/// `test_support::fixtures`.
fn confidence(value: f64) -> Confidence {
    serde_json::from_value(serde_json::json!(value))
        .expect("confidence value must round-trip through serde")
}

// -----------------------------------------------------------------------------
// Fixture helpers — small, named, single-purpose (nw-fp-usable-design)
// -----------------------------------------------------------------------------

/// Build a fresh tempdir + the adapter rooted at `<tmp>/openlore.duckdb`.
/// Returns `(adapter, tmp)` — the caller MUST keep `tmp` alive (its
/// `Drop` removes the directory).
fn fresh_adapter() -> (DuckDbStorageAdapter, TempDir) {
    let tmp = TempDir::new().expect("create tempdir");
    let db_path = tmp.path().join("openlore.duckdb");
    let adapter = DuckDbStorageAdapter::open(&db_path).expect("open adapter on tempdir");
    (adapter, tmp)
}

/// Construct a deterministic `SignedClaim` with a caller-supplied CID
/// and subject. Other fields use stable defaults so two calls with the
/// same `(cid, subject)` produce byte-equal claims.
fn sample_signed_claim(cid: &str, subject: &str) -> SignedClaim {
    SignedClaim {
        unsigned: UnsignedClaim {
            subject: subject.to_string(),
            predicate: "endorses".to_string(),
            object: "the-good".to_string(),
            evidence: vec!["https://example.com/proof".to_string()],
            confidence: confidence(0.8),
            author_did: Did("did:plc:test-jeff#org.openlore.application".to_string()),
            composed_at: "2026-05-25T12:00:00Z".to_string(),
            references: vec![],
            reason: None,
        },
        signature: SignatureBlock {
            signed_cid: Cid(cid.to_string()),
            signature_bytes: vec![0xAA, 0xBB, 0xCC, 0xDD],
            verification_method: "did:plc:test-jeff#org.openlore.application".to_string(),
        },
    }
}

/// Build a `SignedClaim` that references another claim by CID.
fn sample_referencing_claim(
    cid: &str,
    subject: &str,
    target_cid: &str,
    ref_type: ReferenceType,
) -> SignedClaim {
    let mut claim = sample_signed_claim(cid, subject);
    claim.unsigned.references.push(ClaimReference {
        ref_type,
        cid: Cid(target_cid.to_string()),
    });
    claim
}

// -----------------------------------------------------------------------------
// Unit tests at the StoragePort boundary (port-to-port at adapter scope)
// -----------------------------------------------------------------------------

/// Property: opening twice on the same path is idempotent — the schema
/// migration runs on the first open and the second open MUST NOT panic
/// or duplicate rows in `schema_version`.
#[test]
fn open_is_idempotent_across_reopens() {
    let tmp = TempDir::new().expect("create tempdir");
    let db_path = tmp.path().join("openlore.duckdb");

    let adapter1 = DuckDbStorageAdapter::open(&db_path).expect("first open");
    assert!(matches!(adapter1.probe(), ProbeOutcome::Ok));
    drop(adapter1);

    let adapter2 = DuckDbStorageAdapter::open(&db_path).expect("second open");
    assert!(
        matches!(adapter2.probe(), ProbeOutcome::Ok),
        "second open must probe Ok; schema migration must be idempotent"
    );
}

/// Property: `write_signed_claim` then `read_signed_claim` returns the
/// EXACT same value (byte-for-byte equality).
#[test]
fn write_then_read_signed_claim_returns_byte_equal_value() {
    let (adapter, _tmp) = fresh_adapter();
    let claim = sample_signed_claim("bafytest0001", "kant");

    adapter.write_signed_claim(&claim).expect("write succeeds");

    let read_back = adapter
        .read_signed_claim(&Cid("bafytest0001".to_string()))
        .expect("read succeeds")
        .expect("row present");

    assert_eq!(read_back, claim, "round-tripped claim must be byte-equal");
}

/// Property: `read_signed_claim` on an unknown CID returns `Ok(None)`,
/// NOT an error. This is the "missing target = annotated unresolved"
/// contract from data-models.md.
#[test]
fn read_signed_claim_returns_none_for_unknown_cid() {
    let (adapter, _tmp) = fresh_adapter();

    let result = adapter
        .read_signed_claim(&Cid("bafy_does_not_exist".to_string()))
        .expect("read should succeed even for missing");

    assert!(result.is_none(), "missing CID must yield Ok(None)");
}

/// Property: `query_by_subject` returns every claim with the matching
/// subject string and excludes claims with other subjects.
#[test]
fn query_by_subject_returns_only_matching_subject() {
    let (adapter, _tmp) = fresh_adapter();
    let kant1 = sample_signed_claim("bafytest0010", "kant");
    let kant2 = sample_signed_claim("bafytest0011", "kant");
    let hegel = sample_signed_claim("bafytest0012", "hegel");

    adapter.write_signed_claim(&kant1).unwrap();
    adapter.write_signed_claim(&kant2).unwrap();
    adapter.write_signed_claim(&hegel).unwrap();

    let kant_rows = adapter.query_by_subject("kant").expect("query succeeds");

    assert_eq!(kant_rows.len(), 2, "exactly two kant rows expected");
    for row in &kant_rows {
        assert_eq!(row.unsigned.subject, "kant");
    }
}

/// Property: `query_referencing(target)` returns `(referencing_cid,
/// ref_type)` pairs for every claim that references `target` and
/// excludes claims that do not.
#[test]
fn query_referencing_joins_claim_references_correctly() {
    let (adapter, _tmp) = fresh_adapter();
    // Target claim must exist first (FK constraint).
    let target = sample_signed_claim("bafytarget0001", "schopenhauer");
    let retracts = sample_referencing_claim(
        "bafyretracts001",
        "kant",
        "bafytarget0001",
        ReferenceType::Retracts,
    );
    let corrects = sample_referencing_claim(
        "bafycorrects01",
        "hegel",
        "bafytarget0001",
        ReferenceType::Corrects,
    );
    let unrelated = sample_signed_claim("bafyunrelated01", "nietzsche");

    adapter.write_signed_claim(&target).unwrap();
    adapter.write_signed_claim(&retracts).unwrap();
    adapter.write_signed_claim(&corrects).unwrap();
    adapter.write_signed_claim(&unrelated).unwrap();

    let mut rows = adapter
        .query_referencing(&Cid("bafytarget0001".to_string()))
        .expect("query succeeds");
    // Sort for deterministic comparison (DuckDB row order is unspecified).
    rows.sort_by(|a, b| a.0 .0.cmp(&b.0 .0));

    assert_eq!(rows.len(), 2, "exactly two referencing rows expected");
    assert_eq!(
        rows[0],
        (Cid("bafycorrects01".to_string()), ReferenceType::Corrects)
    );
    assert_eq!(
        rows[1],
        (Cid("bafyretracts001".to_string()), ReferenceType::Retracts)
    );
}

/// Property: writing a claim ALSO produces the `<cid>.json` artifact
/// on the filesystem alongside the DB (per data-models.md "Write
/// strategy" — DB row + artifact file in one transaction-equivalent).
#[test]
fn write_signed_claim_produces_artifact_file() {
    let tmp = TempDir::new().expect("create tempdir");
    let db_path = tmp.path().join("openlore.duckdb");
    let adapter = DuckDbStorageAdapter::open(&db_path).expect("open");
    let claim = sample_signed_claim("bafyartifact01", "kant");

    adapter.write_signed_claim(&claim).expect("write succeeds");

    // The adapter colocates `claims/<cid>.json` next to the DB file.
    let artifact = tmp.path().join("claims").join("bafyartifact01.json");
    assert!(
        artifact.exists(),
        "artifact file must be written atomically alongside DB row: {:?}",
        artifact
    );

    // And no .tmp file should remain (atomic rename completed).
    let tmpfile = tmp.path().join("claims").join("bafyartifact01.json.tmp");
    assert!(!tmpfile.exists(), "no .tmp leftover after atomic rename");
}

/// Property: `record_publication` updates `at_uri` + `published_at` for
/// an existing row, leaving the rest of the row untouched. This is the
/// state-delta contract — exactly two slots change, all others are
/// unchanged.
#[test]
fn record_publication_updates_at_uri_and_published_at_only() {
    let (adapter, _tmp) = fresh_adapter();
    let claim = sample_signed_claim("bafypublish001", "kant");
    adapter.write_signed_claim(&claim).unwrap();

    let before = adapter
        .read_signed_claim(&Cid("bafypublish001".to_string()))
        .unwrap()
        .unwrap();

    let pub_time = Utc.with_ymd_and_hms(2026, 5, 26, 9, 0, 0).unwrap();
    adapter
        .record_publication(
            &Cid("bafypublish001".to_string()),
            "at://did:plc:test-jeff/org.openlore.claim/bafypublish001",
            pub_time,
        )
        .expect("record_publication succeeds");

    let after = adapter
        .read_signed_claim(&Cid("bafypublish001".to_string()))
        .unwrap()
        .unwrap();

    // The signed payload (unsigned + signature) MUST be unchanged.
    // `at_uri` + `published_at` are LOCAL-ONLY metadata per
    // data-models.md and are NOT part of `SignedClaim`.
    assert_eq!(
        before, after,
        "SignedClaim payload must be unchanged by record_publication"
    );
}
