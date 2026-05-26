//! Lexicon-conformance acceptance tests for openlore-foundation
//! slice-01.
//!
//! The `org.openlore.claim` Lexicon is a **federation contract**: any
//! peer in slice-03 will deserialize OpenLore claims against this
//! schema, and any drift in canonical-CID computation breaks
//! round-trip identity (KPI-4). These tests validate the Lexicon shape
//! and the CID stability properties from ADR-006 §Earned Trust.
//!
//! Layer placement (per nw-test-design-mandates Mandate 9): these are
//! LAYER-2 acceptance tests — they exercise the pure core
//! (`claim-domain` + `lexicon`) DIRECTLY, without going through the
//! CLI subprocess. This is appropriate for the canonicalization
//! contract: the CID-computing pipeline IS the contract, and exercising
//! it through the CLI would only add noise.
//!
//! LC-3 is the one property-based test in slice-01 (per DD-12 +
//! Mandate 9). It uses `proptest`, the Rust idiomatic PBT crate from
//! the nw-distill polyglot matrix.
//!
//! Per Mandate 7: every test panics via `todo!()` at DISTILL handoff.
//
// SCAFFOLD: true

mod support;

#[allow(unused_imports)] // proptest is used by LC-3 only; DELIVER may swap to direct use
use support::*;

// =============================================================================
// LC-1: Compose → sign → serialize → deserialize → equality
// =============================================================================

/// LC-1: Round-trip identity at the value level. Compose a claim, sign
/// it, serialize to canonical JSON, deserialize back, and assert the
/// recovered value equals the original.
///
/// @lexicon @US-002 @US-004 @J-001 @real-io @in-memory
#[test]
fn lexicon_roundtrip_compose_sign_serialize_deserialize_yields_equal_value() {
    use claim_domain::SignedClaim;
    use openlore_test_support::{
        fixture_jeff_rust_memory_safety, fixture_jeff_rust_memory_safety_signed,
    };

    // Compose: the canonical Jeff-on-Rust UnsignedClaim per US-001 Ex 1.
    let original_unsigned = fixture_jeff_rust_memory_safety();

    // Sign: phase-03 will exercise the real `claim_domain::sign`
    // primitive end-to-end via FakeIdentity::jeff; LC-1's contract is
    // shape-roundtrip equality, which is signature-content-agnostic, so
    // the deterministic placeholder signature in the fixture is the
    // right level of detail for this scenario.
    let original_signed = fixture_jeff_rust_memory_safety_signed();
    assert_eq!(
        original_signed.unsigned, original_unsigned,
        "the signed fixture must wrap the canonical unsigned fixture (sanity)"
    );

    // Serialize → JSON: claim_domain types own the on-disk shape; the
    // lexicon module owns the federation-wire shape (see
    // lexicon::serde_impls). LC-1's roundtrip is on the on-disk shape
    // because that is what a peer reading back its own published claim
    // will compare against.
    let serialized = serde_json::to_value(&original_signed)
        .expect("SignedClaim Serialize derive must succeed for fixture");

    // Deserialize → SignedClaim.
    let recovered: SignedClaim = serde_json::from_value(serialized.clone())
        .expect("SignedClaim Deserialize derive must round-trip for fixture");

    // Criterion 1: full-value equality.
    assert_eq!(
        original_signed, recovered,
        "compose → serialize → deserialize must yield an equal SignedClaim (LC-1)"
    );

    // Criterion 2: signature block survives the roundtrip unchanged.
    assert_eq!(
        original_signed.signature, recovered.signature,
        "signature block must be byte-identical across roundtrip"
    );

    // Criterion 3: unsigned fields survive the roundtrip unchanged.
    assert_eq!(
        original_signed.unsigned, recovered.unsigned,
        "every unsigned field (subject, predicate, object, evidence, confidence, author_did, composed_at, references) must roundtrip verbatim"
    );

    // Also sanity-check the unsigned-only roundtrip path — the pre-sign
    // value used by the compose preview must itself roundtrip cleanly.
    let unsigned_json = serde_json::to_value(&original_unsigned)
        .expect("UnsignedClaim Serialize derive must succeed");
    let unsigned_recovered: claim_domain::UnsignedClaim =
        serde_json::from_value(unsigned_json).expect("UnsignedClaim Deserialize must roundtrip");
    assert_eq!(
        original_unsigned, unsigned_recovered,
        "UnsignedClaim must roundtrip independently of the signature block"
    );
}

// =============================================================================
// LC-2: Schema conformance — produced JSON validates against the Lexicon
// =============================================================================

/// LC-2: Every claim emitted by the production pipeline validates
/// against the `org.openlore.claim` Lexicon schema. (ADR-005 + US-005
/// "Lexicon loadable" AC.)
///
/// Step 02-01 note: `claim_domain::canonicalize` + `sign` are still RED
/// scaffolds, so this scenario hand-rolls a Lexicon-shaped JSON value
/// matching the US-001 Example 1 fixture (jeff / rust / memory-safety)
/// and exercises `lexicon::validate_claim_json` directly. The later
/// step 03-02 (sign pipeline) will rewire this scenario to compose via
/// `claim_domain` once that crate is GREEN — the assertion stays the
/// same: a well-formed signed claim MUST validate against the Lexicon.
///
/// @lexicon @US-005 @infra @in-memory
#[test]
fn lexicon_validates_signed_claim_against_org_openlore_claim_schema() {
    // Confirm the embedded Lexicon JSON loads (US-005 AC: "Lexicon loadable").
    let lexicon_schema: serde_json::Value =
        serde_json::from_str(lexicon::CLAIM_LEXICON_JSON)
            .expect("embedded org.openlore.claim Lexicon JSON must parse");
    assert_eq!(
        lexicon_schema["id"].as_str(),
        Some(lexicon::CLAIM_NSID),
        "embedded Lexicon NSID must match the public CLAIM_NSID constant"
    );

    // Compose a Lexicon-shaped signed-claim JSON value (US-001 Example 1).
    let signed_claim_json = serde_json::json!({
        "subject": "github:rust-lang/rust",
        "predicate": "embodiesPhilosophy",
        "object": "org.openlore.philosophy.memory-safety",
        "evidence": ["https://github.com/rust-lang/rust"],
        "confidence": 0.85,
        "author": "did:plc:test-jeff#org.openlore.application",
        "composedAt": "2026-05-25T12:00:00Z",
        "references": [],
        "signature": {
            "kid": "did:plc:test-jeff#org.openlore.application",
            "alg": "EdDSA",
            "sig": "Zm9vYmFy"
        }
    });

    let claim = lexicon::validate_claim_json(&signed_claim_json)
        .expect("a well-formed signed claim MUST validate against the Lexicon");

    // Spot-check the parsed shape — federation contract: field names
    // round-trip verbatim per the Lexicon JSON keys.
    assert_eq!(claim.subject, "github:rust-lang/rust");
    assert_eq!(claim.confidence, 0.85);
    assert_eq!(claim.composed_at, "2026-05-25T12:00:00Z");
    assert!(
        claim.signature.is_some(),
        "signed claim must carry a signature block"
    );
}

// =============================================================================
// LC-3: CID byte stability across N re-canonicalizations (PROPERTY)
// =============================================================================

/// LC-3: Property test — for any randomly generated valid claim,
/// encoding to canonical CBOR, decoding, re-encoding MUST yield
/// byte-identical CBOR (so the CID is stable). (ADR-006 §Earned Trust
/// property test 1; the load-bearing canonicalization invariant.)
///
/// This is the ONE `@property`-tagged scenario in slice-01 (per DD-12).
/// Runs at layer 2 (Mandate 9 permits PBT here).
///
/// Step 02-04 implementation: the actual property body is wrapped in
/// `proptest!` and consumes `arb_unsigned_claim` from `claim_domain`.
/// Case count (256) is pinned via the workspace-root `proptest.toml`,
/// satisfying criterion 1; CI determinism rides on `PROPTEST_SEED`
/// (DEVOPS scope) per DESIGN open-question #4.
///
/// @lexicon @property @US-002 @J-001 @in-memory
#[test]
fn lexicon_cid_is_byte_stable_across_n_re_canonicalizations() {
    use claim_domain::proptest_strategies::arb_unsigned_claim;
    use proptest::prelude::*;

    proptest!(|(claim in arb_unsigned_claim())| {
        // Criterion 1: canonicalize twice → byte-equal CBOR.
        let cbor_1 = claim_domain::canonicalize(&claim)
            .expect("canonicalize MUST succeed for any well-formed UnsignedClaim");
        let cbor_2 = claim_domain::canonicalize(&claim)
            .expect("canonicalize MUST succeed on the second call too");
        prop_assert_eq!(
            &cbor_1, &cbor_2,
            "canonical CBOR must be byte-identical across re-canonicalizations \
             (ADR-006 §Earned Trust 1)"
        );

        // Criterion 2: same CID across re-runs for the same input.
        let cid_1 = claim_domain::compute_cid(&cbor_1);
        let cid_2 = claim_domain::compute_cid(&cbor_2);
        prop_assert_eq!(
            cid_1, cid_2,
            "compute_cid of byte-equal CBOR must produce equal CIDs \
             (federation round-trip identity, KPI-4)"
        );
    });
}

// =============================================================================
// LC-4: CID stability against gold-fixture suite
// =============================================================================

/// LC-4: For a frozen suite of known JSON claims, the CID computed by
/// the production pipeline equals the CID stored in the fixture file.
/// Catches cross-version drift in the CBOR encoder. (ADR-006 §Earned
/// Trust property test 2 — gold fixtures.)
///
/// ## Fixture layout
///
/// `tests/fixtures/gold_cids/claim_NNN.json` — an `UnsignedClaim` JSON
/// body (serde-default shape; field-for-field).
/// `tests/fixtures/gold_cids/claim_NNN.cid` — the frozen base32-lower
/// CID string, one line, no trailing newline.
///
/// ## Diversity coverage (the 5 gold fixtures)
///
/// 1. `claim_001` — basic happy-path (jeff / rust / memory-safety,
///    matches data-models.md §"On-disk artifact format" example).
/// 2. `claim_002` — unicode in subject (Japanese mastodon handle) and
///    object (Japanese philosophy name) plus emoji in evidence URL.
/// 3. `claim_003` — `references` array with 2 entries (retracts +
///    corrects), exercising the inner-map canonicalization.
/// 4. `claim_004` — empty `evidence`, confidence at the inclusive
///    lower bound (0.0).
/// 5. `claim_005` — confidence near the upper bound (0.99) AND a
///    4-URL evidence array (longest-array branch).
///
/// ## CID drift detection
///
/// When this test fires, the assertion message NAMES the failing
/// fixture so a CBOR-encoder drift is immediately attributable. See
/// ADR-006 §Earned Trust point 2.
///
/// @lexicon @US-002 @J-001 @real-io @in-memory
#[test]
fn lexicon_cid_is_byte_stable_for_fixture_suite_of_known_claims() {
    use claim_domain::{canonicalize, compute_cid, UnsignedClaim};
    use std::fs;
    use std::path::PathBuf;

    // Fixtures live OUTSIDE tests/acceptance/ so they're test-data, not
    // test code. Path is resolved relative to the workspace root, which
    // is where `cargo test` runs.
    let fixtures_dir: PathBuf = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..") // crates/
        .join("..") // workspace root
        .join("tests")
        .join("fixtures")
        .join("gold_cids");
    assert!(
        fixtures_dir.is_dir(),
        "gold-fixture directory MUST exist at {}",
        fixtures_dir.display()
    );

    // Diversity contract: at least 5 fixtures covering distinct shapes.
    let fixture_ids = ["001", "002", "003", "004", "005"];
    assert!(
        fixture_ids.len() >= 5,
        "LC-4 requires >= 5 gold fixtures for diversity coverage; have {}",
        fixture_ids.len()
    );

    for id in fixture_ids {
        let json_path = fixtures_dir.join(format!("claim_{id}.json"));
        let cid_path = fixtures_dir.join(format!("claim_{id}.cid"));

        let json_bytes = fs::read(&json_path)
            .unwrap_or_else(|e| panic!("gold fixture {} missing: {}", json_path.display(), e));
        let claim: UnsignedClaim = serde_json::from_slice(&json_bytes)
            .unwrap_or_else(|e| panic!("gold fixture {} not valid UnsignedClaim JSON: {}", json_path.display(), e));

        let canonical = canonicalize(&claim).unwrap_or_else(|e| {
            panic!("canonicalize MUST succeed on gold fixture {}: {:?}", json_path.display(), e)
        });
        let computed_cid = compute_cid(&canonical);

        let expected_cid_raw = fs::read_to_string(&cid_path)
            .unwrap_or_else(|e| panic!("frozen CID file {} missing: {}", cid_path.display(), e));
        let expected_cid = expected_cid_raw.trim();

        assert_eq!(
            computed_cid.0.as_str(),
            expected_cid,
            "CID drift detected for gold fixture claim_{id}: \
             canonicalize+compute_cid yielded `{}` but frozen `.cid` file says `{}`. \
             Either the CBOR encoder drifted (federation-breaking — see ADR-006 §Earned \
             Trust point 2) or the fixture/CID pair needs regenerating after an \
             intentional spec change.",
            computed_cid.0,
            expected_cid,
        );
    }
}

/// Snapshot-bootstrap helper for LC-4. Computes the gold CIDs for every
/// `claim_NNN.json` fixture and prints them. Run once after authoring or
/// intentionally-changing a fixture; capture the printed lines and write
/// each to the corresponding `claim_NNN.cid` file. NOT part of the
/// regular suite — `#[ignore]` keeps it out of CI.
///
/// Invocation:
///   cargo test -p cli --test lexicon_conformance \
///     lc4_recompute_gold_cids -- --ignored --nocapture
#[test]
#[ignore]
fn lc4_recompute_gold_cids() {
    use claim_domain::{canonicalize, compute_cid, UnsignedClaim};
    use std::fs;
    use std::path::PathBuf;

    let fixtures_dir: PathBuf = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("tests")
        .join("fixtures")
        .join("gold_cids");

    let fixture_ids = ["001", "002", "003", "004", "005"];
    for id in fixture_ids {
        let json_path = fixtures_dir.join(format!("claim_{id}.json"));
        let json_bytes = fs::read(&json_path).expect("fixture JSON readable");
        let claim: UnsignedClaim = serde_json::from_slice(&json_bytes).expect("JSON parses");
        let canonical = canonicalize(&claim).expect("canonicalize succeeds");
        let cid = compute_cid(&canonical);
        println!("claim_{id}.cid={}", cid.0);
    }
}

// =============================================================================
// LC-5: Lexicon rejects out-of-range confidence at the wire boundary
// =============================================================================

/// LC-5: The Lexicon's confidence field carries `minimum: 0.0` and
/// `maximum: 1.0`. A JSON payload with confidence outside that range
/// MUST be rejected at the Lexicon validation boundary, regardless of
/// what the CLI's pre-sign validator does. (data-models.md confidence
/// min/max; defense-in-depth on top of WS-4.)
///
/// Per ATProto/JSON-Schema convention, `minimum`/`maximum` are
/// inclusive — boundary values 0.0 and 1.0 MUST validate.
///
/// @lexicon @US-001 @J-001 @in-memory
#[test]
fn lexicon_rejects_out_of_range_confidence_at_wire_boundary() {
    // Build a Lexicon-shaped claim with the confidence value parameterized.
    let claim_with_confidence = |confidence: f64| -> serde_json::Value {
        serde_json::json!({
            "subject": "github:rust-lang/rust",
            "predicate": "embodiesPhilosophy",
            "object": "org.openlore.philosophy.memory-safety",
            "evidence": ["https://github.com/rust-lang/rust"],
            "confidence": confidence,
            "author": "did:plc:test-jeff#org.openlore.application",
            "composedAt": "2026-05-25T12:00:00Z",
            "references": [],
        })
    };

    // Criterion 1: confidence = 1.4 (above max) rejected; error names
    // BOTH the field "confidence" AND the range "[0.0, 1.0]".
    let above_err = lexicon::validate_claim_json(&claim_with_confidence(1.4))
        .expect_err("confidence=1.4 MUST be rejected (above max)");
    let above_msg = above_err.to_string();
    assert!(
        above_msg.contains("confidence"),
        "error MUST name the `confidence` field; got: {above_msg}"
    );
    assert!(
        above_msg.contains("[0.0, 1.0]"),
        "error MUST name the valid range `[0.0, 1.0]`; got: {above_msg}"
    );

    // Criterion 2: confidence = -0.1 (below min) rejected; same shape.
    let below_err = lexicon::validate_claim_json(&claim_with_confidence(-0.1))
        .expect_err("confidence=-0.1 MUST be rejected (below min)");
    let below_msg = below_err.to_string();
    assert!(
        below_msg.contains("confidence"),
        "error MUST name the `confidence` field; got: {below_msg}"
    );
    assert!(
        below_msg.contains("[0.0, 1.0]"),
        "error MUST name the valid range `[0.0, 1.0]`; got: {below_msg}"
    );

    // Criterion 3: boundary values 0.0 and 1.0 ACCEPTED (inclusive bounds).
    let at_zero = lexicon::validate_claim_json(&claim_with_confidence(0.0))
        .expect("confidence=0.0 MUST validate (inclusive lower bound)");
    assert_eq!(at_zero.confidence, 0.0);
    let at_one = lexicon::validate_claim_json(&claim_with_confidence(1.0))
        .expect("confidence=1.0 MUST validate (inclusive upper bound)");
    assert_eq!(at_one.confidence, 1.0);
}

// =============================================================================
// LC-6: Self-reference rejected at sign time
// =============================================================================

/// LC-6: A claim whose `references` array contains an entry pointing
/// at the claim's own CID MUST be rejected at sign time. (ADR-008
/// §Behavioral rule 4 + Earned Trust 2.)
///
/// Note: this is a chicken-and-egg situation — the claim's CID is
/// derived from the claim including its references field. The
/// production code MUST detect the would-be cycle by computing the
/// unsigned CID once, checking if any reference targets that CID, and
/// rejecting BEFORE signing.
///
/// @lexicon @US-003 @J-001 @in-memory
#[test]
fn lexicon_rejects_self_reference_in_references_array() {
    todo!("DELIVER: construct an UnsignedClaim whose references array contains [{{type: retracts, cid: <its_own_unsigned_cid>}}]; call claim_domain::reference_rules_validate(claim, None); assert Err(ClaimError::SelfReference)")
}

// =============================================================================
// LC-7: Two-hop reference cycle rejected at sign time
// =============================================================================

/// LC-7: Claim A references claim B; if claim B (already in the local
/// store) references claim A, the sign step MUST reject claim A with
/// `CycleDetected`. (ADR-008 §Earned Trust 3.)
///
/// @lexicon @US-003 @J-001 @real-io @in-memory
#[test]
fn lexicon_rejects_two_hop_reference_cycle() {
    todo!("DELIVER: build a tiny in-memory ClaimLookup implementing the claim_domain trait; seed it with claim B (which references claim A's would-be CID); attempt to sign claim A which references claim B's CID; assert Err(ClaimError::CycleDetected); requires reference_rules_validate(claim, Some(lookup))")
}

// =============================================================================
// LC-8: Persisted payload never contains a bucket label string
// =============================================================================

/// LC-8: The on-disk JSON of a signed claim with `confidence: 0.55`
/// (display bucket: "weighted") MUST NOT contain any of the strings
/// `speculative | weighted | well-evidenced | triangulated` anywhere.
/// (WD-10 / D-12 — CI-failable invariant per data-models.md
/// §Confidence buckets are NOT persisted.)
///
/// @lexicon @US-001 @US-002 @J-001 @in-memory
#[test]
fn lexicon_persisted_payload_never_contains_bucket_label_string() {
    // Compose a claim for each of the 4 buckets, serialize to canonical
    // JSON, and grep for the bucket labels. None must appear.
    let bucket_test_confidences = [0.1_f64, 0.5, 0.8, 0.95]; // speculative, weighted, well-evidenced, triangulated
    let bucket_labels = ["speculative", "weighted", "well-evidenced", "triangulated"];

    for _conf in bucket_test_confidences {
        // Build claim with this confidence
        // Serialize via lexicon::claim
        // For each bucket_label, assert !serialized.contains(label)
    }

    let _ = bucket_labels; // silence unused warning until DELIVER fills in
    todo!("DELIVER: iterate the 4 confidences, serialize each signed claim to JSON, assert none of the 4 bucket-label strings appears anywhere in the serialized payload")
}
