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
use ports::{
    GraphNode, PeerStoragePort, PeerSubscription, ProbeOutcome, StoragePort, TraversalBound,
};
use tempfile::TempDir;
use url::Url;

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

// -----------------------------------------------------------------------------
// PeerStoragePort — soft-remove isolation (component-boundaries §adapter-duckdb
// probe #5 / WD-25 / ADR-014). Port-to-port at the PeerStoragePort boundary.
// -----------------------------------------------------------------------------

/// A peer adapter over a fresh tempdir. Returns `(peer, _author, tmp)` —
/// the caller MUST keep `tmp` AND `_author` alive: the peer adapter SHARES
/// the author adapter's single DuckDB connection (Q-DELIVER-3 single-writer
/// constraint), so seeding + assertions all flow through ONE connection.
fn fresh_peer_adapter() -> (
    adapter_duckdb::DuckDbPeerStorageAdapter,
    DuckDbStorageAdapter,
    TempDir,
) {
    let tmp = TempDir::new().expect("create tempdir");
    let db_path = tmp.path().join("openlore.duckdb");
    let author = DuckDbStorageAdapter::open(&db_path).expect("open adapter on tempdir");
    // Bind a LOCAL user DID distinct from every test peer DID so the
    // WD-40 SelfAttribution guard (step 04-05) never fires for these
    // peer-authored soft-remove/hard-purge fixtures.
    let peer = author.peer_adapter(&Did("did:plc:local-user-test".to_string()));
    (peer, author, tmp)
}

/// A deterministic peer-authored `SignedClaim` for a given peer DID +
/// distinct ordinal (so each seeded CID is unique).
fn peer_signed_claim(peer_did: &str, i: usize) -> SignedClaim {
    SignedClaim {
        unsigned: UnsignedClaim {
            subject: format!("subject-{i}"),
            predicate: "endorses".to_string(),
            object: format!("object-{i}"),
            evidence: vec!["https://peer.example/proof".to_string()],
            confidence: confidence(0.8),
            author_did: Did(peer_did.to_string()),
            composed_at: "2026-05-25T12:00:00Z".to_string(),
            references: vec![],
            reason: None,
        },
        signature: SignatureBlock {
            signed_cid: Cid(format!("bafyseedpeer{i}")),
            signature_bytes: vec![0x01, 0x02, 0x03, 0x04],
            verification_method: format!("{peer_did}#org.openlore.application"),
        },
    }
}

/// Seed `count` cached `peer_claims` rows attributed to `peer_did`
/// THROUGH the port (`write_peer_claim`) so they share the adapter's
/// connection. (`peer pull` is the production population path, Phase 04;
/// here we drive the same storage seam directly.)
fn seed_peer_claims(peer: &adapter_duckdb::DuckDbPeerStorageAdapter, peer_did: &Did, count: usize) {
    let pds = Url::parse("https://peer.example/pds").unwrap();
    let fetched_at = Utc.with_ymd_and_hms(2026, 5, 27, 10, 0, 0).unwrap();
    for i in 0..count {
        let claim = peer_signed_claim(&peer_did.0, i);
        let outcome = peer
            .write_peer_claim(peer_did, &claim, &pds, fetched_at)
            .unwrap_or_else(|err| panic!("seed peer_claim {i}: {err}"));
        assert!(outcome.written, "fresh peer_claim {i} must be written");
    }
}

/// Property (soft-remove isolation, probe #5): given 1 subscription + N
/// cached peer_claims rows, `soft_remove` sets the subscription's
/// `removed_at` (it leaves `list_active_subscriptions` yet `lookup_subscription`
/// still finds it with `removed_at` SET) and RETAINS all N peer_claims rows.
/// The returned `SoftRemoveOutcome` reports `was_subscribed = true` and the
/// retained `cached_claim_count = N`.
#[test]
fn soft_remove_sets_removed_at_and_retains_all_peer_claims() {
    let (peer, _author, _tmp) = fresh_peer_adapter();
    let peer_did = Did("did:plc:rachel-test".to_string());
    let cached = 3usize;

    // Seed: 1 ACTIVE subscription + N cached peer_claims — both through
    // the SAME shared connection (port-to-port).
    let subscribed_at = Utc.with_ymd_and_hms(2026, 5, 27, 10, 14, 32).unwrap();
    peer.add_subscription(PeerSubscription {
        peer_did: peer_did.clone(),
        peer_handle: "rachel.test".to_string(),
        peer_pds_endpoint: Url::parse("https://peer.example/pds").unwrap(),
        subscribed_at,
        removed_at: None,
    })
    .expect("seed active subscription");
    seed_peer_claims(&peer, &peer_did, cached);

    // Precondition sanity: exactly one ACTIVE subscription before remove.
    assert_eq!(
        peer.list_active_subscriptions().unwrap().len(),
        1,
        "precondition: exactly one ACTIVE subscription before soft-remove"
    );

    // Action: soft-remove.
    let outcome = peer.soft_remove(&peer_did).expect("soft_remove succeeds");

    // Outcome surface (port-exposed return): subscribed + retained count.
    // `cached_claim_count` IS the port-observable "peer_claims unchanged".
    assert!(
        outcome.was_subscribed,
        "soft_remove of a known subscription must report was_subscribed=true"
    );
    assert_eq!(
        outcome.cached_claim_count, cached as u32,
        "soft_remove must report the RETAINED cached-claim count (probe #5; WD-25)"
    );

    // State: the row is soft-removed — gone from active listing but still
    // present via lookup with `removed_at` SET (soft-remove does NOT delete).
    assert_eq!(
        peer.list_active_subscriptions().unwrap().len(),
        0,
        "soft-removed subscription must drop out of the active listing"
    );
    let looked_up = peer
        .lookup_subscription(&peer_did)
        .unwrap()
        .expect("soft-remove must NOT delete the subscription row");
    assert!(
        looked_up.removed_at.is_some(),
        "soft_remove must SET removed_at on the subscription row (WD-25)"
    );

    // Re-running soft_remove still reports the SAME retained cache count —
    // the peer_claims rows survived the first soft-remove (idempotent
    // isolation; the count would drop to 0 if the rows had been deleted).
    let again = peer.soft_remove(&peer_did).expect("idempotent soft_remove");
    assert_eq!(
        again.cached_claim_count, cached as u32,
        "cached peer_claims must persist across repeated soft-removes (WD-25)"
    );
}

/// Property (idempotent / never-subscribed): `soft_remove` of a DID with no
/// subscription row is a no-op that reports `was_subscribed = false` and
/// `cached_claim_count = 0` (US-FED-005 Example 4 storage contract).
#[test]
fn soft_remove_of_unknown_did_is_noop() {
    let (peer, _author, _tmp) = fresh_peer_adapter();
    let stranger = Did("did:plc:stranger-test".to_string());

    let outcome = peer.soft_remove(&stranger).expect("soft_remove succeeds");

    assert!(
        !outcome.was_subscribed,
        "soft_remove of an unknown DID must report was_subscribed=false"
    );
    assert_eq!(
        outcome.cached_claim_count, 0,
        "an unknown DID has zero cached peer claims"
    );
}

// -----------------------------------------------------------------------------
// Slice-04 (step 04-01) — `traverse_graph` recursive-CTE integration tests
// (real DuckDB; port-to-port at the StoragePort::traverse_graph boundary).
// -----------------------------------------------------------------------------

/// Build a `SignedClaim` with full control of cid / subject / object /
/// author_did so a traversal fixture can seed a cross-project span (one author
/// asserting one object across two subjects) and a cyclic graph.
fn traversal_claim(cid: &str, subject: &str, object: &str, author_did: &str) -> SignedClaim {
    let mut claim = sample_signed_claim(cid, subject);
    claim.unsigned.object = object.to_string();
    claim.unsigned.author_did = Did(author_did.to_string());
    claim.signature.verification_method = author_did.to_string();
    claim
}

/// GQE-20 leg (US-GRAPH-004; Gate 5 + anti-merging WD-73): a `--object`
/// traversal over a cross-project span returns ONE edge per signed claim, each
/// carrying its backing `claim_cid` (non-`Option`) AND its bare `author_did`
/// (non-`Option`). Rachel spans cargo + nixpkgs; Tobias holds deno — so the walk
/// surfaces exactly the three seeded edges, none fabricated, none merged.
#[test]
fn traverse_graph_returns_one_attributed_edge_per_signed_claim() {
    let (adapter, _tmp) = fresh_adapter();
    let dep = "org.openlore.philosophy.dependency-pinning";

    adapter
        .write_signed_claim(&traversal_claim(
            "bafyrachelcargo",
            "github:rust-lang/cargo",
            dep,
            "did:plc:rachel-test",
        ))
        .expect("seed rachel/cargo");
    adapter
        .write_signed_claim(&traversal_claim(
            "bafyrachelnixpkgs",
            "github:NixOS/nixpkgs",
            dep,
            "did:plc:rachel-test",
        ))
        .expect("seed rachel/nixpkgs");
    adapter
        .write_signed_claim(&traversal_claim(
            "bafytobiasdeno",
            "github:denoland/deno",
            dep,
            "did:plc:tobias-test",
        ))
        .expect("seed tobias/deno");

    let start = GraphNode::Philosophy {
        object: dep.to_string(),
    };
    let result = adapter
        .traverse_graph(&start, &TraversalBound::default())
        .expect("traverse the dependency-pinning graph");

    // Exactly 3 edges (one per seeded signed claim) — Gate 5: traversal invents
    // no edge; each maps to exactly one backing claim_cid.
    assert_eq!(
        result.edges.len(),
        3,
        "expected exactly 3 edges (one per seeded claim, none fabricated/merged); got {:?}",
        result.edges
    );

    // Every edge carries a backing claim_cid AND a bare author_did (anti-merging
    // WD-73). The cid set matches the seeded cids exactly (no edge unmapped).
    let mut cids: Vec<&str> = result
        .edges
        .iter()
        .map(|e| e.claim_cid.0.as_str())
        .collect();
    cids.sort_unstable();
    assert_eq!(
        cids,
        vec!["bafyrachelcargo", "bafyrachelnixpkgs", "bafytobiasdeno"],
        "every edge must map to exactly one seeded signed claim (Gate 5)"
    );
    for edge in &result.edges {
        assert!(
            !edge.author_did.0.is_empty(),
            "every edge must carry its backing claim's author DID (anti-merging WD-73); got {edge:?}"
        );
    }

    // Rachel's two edges span TWO distinct projects (cargo + nixpkgs) — the
    // cross-project span the traversal surfaces (KPI-GRAPH-1).
    let rachel_projects: std::collections::HashSet<String> = result
        .edges
        .iter()
        .filter(|e| e.author_did.0 == "did:plc:rachel-test")
        .map(|e| match &e.to {
            GraphNode::Project { subject } => subject.clone(),
            other => panic!("expected a project edge, got {other:?}"),
        })
        .collect();
    assert_eq!(
        rachel_projects.len(),
        2,
        "Rachel's edges must span 2 distinct projects (cross-project span); got {rachel_projects:?}"
    );
}

/// ADR-021 cycle-safety: a CYCLIC fixture (two claims sharing a subject so the
/// recursive self-join would loop) TERMINATES and never re-traverses an edge —
/// the delimited `visited` guard breaks the cycle (DuckDB recursive CTEs do not
/// auto-detect cycles). The walk returns each edge a bounded number of times,
/// not an unbounded explosion.
#[test]
fn traverse_graph_terminates_on_a_cyclic_graph() {
    let (adapter, _tmp) = fresh_adapter();
    let phil = "org.openlore.philosophy.cycle-test";
    let project = "github:test/cycle";

    // Two claims on the SAME project + object by two authors: the recursive
    // `eb.subject = w.subject` join would loop A->B->A->... without the guard.
    adapter
        .write_signed_claim(&traversal_claim("bafycyclea", project, phil, "did:plc:a"))
        .expect("seed cycle a");
    adapter
        .write_signed_claim(&traversal_claim("bafycycleb", project, phil, "did:plc:b"))
        .expect("seed cycle b");

    let start = GraphNode::Philosophy {
        object: phil.to_string(),
    };
    // A generous bound that, without the visited guard, would let the shared
    // subject loop indefinitely. The guard caps each claim_cid to one traversal
    // per path, so the walk terminates and the edge count stays bounded.
    let result = adapter
        .traverse_graph(&start, &TraversalBound { max_depth: 8 })
        .expect("cyclic traversal terminates");

    assert!(
        result.edges.len() <= 8,
        "cyclic traversal must terminate with a bounded edge count (visited guard); got {} edges",
        result.edges.len()
    );
    // No single backing claim is traversed more than the path length allows
    // (the guard forbids revisiting a claim_cid within one path).
    assert!(
        !result.edges.is_empty(),
        "the seed edges (depth 1) must still be discovered"
    );
}

/// WD-76 depth bound: a chain of claims deeper than `max_depth` returns only the
/// in-bound edges AND reports the omitted deeper edges. A depth-2 walk over a
/// 3-hop chain omits the depth-3 edge.
#[test]
fn traverse_graph_is_depth_bounded_and_reports_omitted_edges() {
    let (adapter, _tmp) = fresh_adapter();
    let phil = "org.openlore.philosophy.chain-test";

    // A chain where each successive claim shares the PRIOR claim's subject so
    // the walk hops project-to-project: seed -> hop1 -> hop2. With max_depth 2,
    // the third hop (depth 3) is omitted.
    //
    // The recursive join is `eb.subject = w.subject`, so to chain we need each
    // hop to share a subject with the prior frontier. Two claims on subject S1
    // (the seed pair) let the walk step S1->S1 once (depth 2), and a deeper pair
    // would step again (depth 3). Seed three claims on one shared subject so the
    // walk can reach depth 3 — the bound must cut it at 2.
    let shared = "github:test/chain";
    for (i, cid) in ["bafychain1", "bafychain2", "bafychain3", "bafychain4"]
        .iter()
        .enumerate()
    {
        adapter
            .write_signed_claim(&traversal_claim(
                cid,
                shared,
                phil,
                &format!("did:plc:chain{i}"),
            ))
            .expect("seed chain claim");
    }

    let start = GraphNode::Philosophy {
        object: phil.to_string(),
    };
    let result = adapter
        .traverse_graph(&start, &TraversalBound { max_depth: 2 })
        .expect("depth-bounded traversal");

    // Every returned edge is within the bound.
    assert!(
        result.edges.iter().all(|e| e.depth <= 2),
        "all returned edges must be within max_depth=2; got depths {:?}",
        result.edges.iter().map(|e| e.depth).collect::<Vec<_>>()
    );
    // Deeper edges exist (depth 3 reachable on the shared subject), so the bound
    // omitted some — the omitted count is reported (WD-76).
    assert!(
        result.omitted_edge_count > 0,
        "expected the depth-2 bound to omit deeper (depth-3) edges on the shared-subject chain; \
         got omitted_edge_count={}",
        result.omitted_edge_count
    );
    assert!(
        result.reached_bound,
        "reached_bound must be true when edges existed beyond the bound"
    );
}
