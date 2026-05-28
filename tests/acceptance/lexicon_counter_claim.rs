//! Slice-03 lexicon-conformance acceptance — counter-claim Lexicon
//! extension + NFC normalization invariant + CID stability across the
//! slice-01 → slice-03 upgrade.
//!
//! Layer 2 (in-memory acceptance — pure-core direct invocation, no CLI
//! subprocess) per nw-tdd-methodology Layered Test Discipline matrix.
//! Sibling to slice-01's `lexicon_conformance.rs`; same shape, same
//! file role.
//!
//! Per Mandate 9 (layer-dependent PBT mode): layers 1-2 may use PBT
//! full. The NFC idempotency property + the slice-01→slice-03 CID
//! stability property are `@property` scenarios runnable via proptest.
//! The example-only Lexicon validation scenarios are example-pinned
//! (single fixture each).
//!
//! Covers:
//! - ADR-015 `reason` field forward-compat with slice-01 readers
//! - WD-35 NFC normalization idempotency + NFC-unification properties
//! - I-FED-6 + I-FED-7 (data-models.md + component-boundaries.md
//!   §`crates/claim-domain` probe responsibilities slice-03 additions)
//
// SCAFFOLD: true

#![allow(dead_code)]
#![allow(unused_imports)]

// NOTE — unlike the subprocess-driven peer_* tests above, this file
// invokes claim_domain + lexicon directly (layer 2). It does NOT use
// `support/mod.rs`'s TestEnv (no subprocess). This is the same pattern
// as slice-01's `lexicon_conformance.rs`.

// =============================================================================
// ADR-015 forward-compat — slice-01 claims still load under slice-03
// =============================================================================

/// LCC-1: A slice-01-era signed claim (one without the `reason` field
/// at all) deserializes cleanly through the slice-03 Lexicon and
/// claim_domain types. The `reason` field defaults to `None`; serde
/// roundtrip is byte-equal modulo the `reason` key being omitted
/// entirely (per `#[serde(default, skip_serializing_if = "Option::is_none")]`
/// per ADR-015 + data-models.md). This is the forward-compat
/// regression gate — proves slice-03 readers do NOT break when reading
/// the slice-01 claim shape.
///
/// @us-fed-006 @real-io @j-003 @forward-compat @adr-015
#[test]
fn lexicon_counter_claim_slice_01_era_claim_loads_without_reason_field() {
    use lexicon::Claim;

    // GIVEN: a slice-01-era `org.openlore.claim` JSON value — note the
    // object has NO `reason` key at all (slice-01 binaries never emitted
    // one; ADR-005 forward-compat requires slice-03 readers tolerate its
    // absence). This is the exact wire shape a slice-01 peer publishes.
    let slice_01_era_json = serde_json::json!({
        "subject": "github:rust-lang/rust",
        "predicate": "embodiesPhilosophy",
        "object": "org.openlore.philosophy.memory-safety",
        "evidence": ["https://www.rust-lang.org/"],
        "confidence": 0.86,
        "author": "did:plc:test-jeff#org.openlore.application",
        "composedAt": "2026-05-25T12:00:00Z",
        "references": [],
        "signature": {
            "kid": "did:plc:test-jeff#org.openlore.application",
            "alg": "EdDSA",
            "sig": "AAAA"
        }
    });
    assert!(
        !slice_01_era_json
            .as_object()
            .expect("fixture is an object")
            .contains_key("reason"),
        "precondition: the slice-01-era fixture MUST NOT carry a `reason` key"
    );

    // WHEN: a slice-03 reader deserializes it through the lexicon `Claim`
    // serde shape (layer-2 pure-core direct invocation — no subprocess).
    let claim: Claim = serde_json::from_value(slice_01_era_json)
        .expect("slice-01-era claim (no `reason` key) MUST deserialize under slice-03 (LCC-1 forward-compat gate)");

    // THEN (criterion 1): the missing `reason` key defaults to `None`
    // (`#[serde(default, ...)]` on `Claim::reason`, per ADR-015).
    assert_eq!(
        claim.reason, None,
        "an absent `reason` key must deserialize to None, never an empty string or a panic"
    );

    // THEN (criterion 2): re-serializing the `reason: None` claim drops
    // the key entirely (`skip_serializing_if = \"Option::is_none\"`), so
    // the re-emitted JSON is byte-equal to a slice-01 claim modulo the
    // `reason` key being omitted — this is what preserves CID stability
    // across the slice-01 -> slice-03 upgrade (I-FED-7).
    let reserialized =
        serde_json::to_value(&claim).expect("a `reason: None` Claim must re-serialize");
    assert!(
        !reserialized
            .as_object()
            .expect("re-serialized claim is an object")
            .contains_key("reason"),
        "a `reason: None` claim must NOT re-emit the `reason` key (forward-compat / CID stability); got: {reserialized}"
    );
}

/// LCC-2 (I-FED-7): A slice-03 claim with `reason: None` produces the
/// SAME canonical CID as a slice-01-era binary would produce for the
/// same content. CID stability is required across the upgrade so
/// previously-published author claims continue to resolve at the same
/// at-uri after the user updates to slice-03. (data-models.md
/// "CID stability across slice-01 → slice-03 upgrade" + ADR-015 +
/// claim_domain property test 1.)
///
/// @us-fed-006 @real-io @j-003 @cid-stability @adr-006 @adr-015
#[test]
fn lexicon_counter_claim_reason_none_preserves_cid_stability_with_slice_01() {
    use claim_domain::{canonicalize, compute_cid, UnsignedClaim};
    use lexicon::Claim;
    use std::fs;
    use std::path::PathBuf;

    // GIVEN: a frozen slice-01-era gold fixture. `claim_001.json` is the
    // exact `UnsignedClaim` body a slice-01 binary published (it carries
    // NO `reason` key — slice-01 never emitted one), and `claim_001.cid`
    // is the base32-lower CID that the slice-01 pipeline FROZE for it
    // (pinned by slice-01 LC-4). That frozen CID is our byte-identical
    // slice-01 reference: re-using LC-4's gold pair means LCC-2 asserts
    // against the SAME slice-01-era CID, not a freshly-recomputed one.
    let fixtures_dir: PathBuf = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..") // crates/
        .join("..") // workspace root
        .join("tests")
        .join("fixtures")
        .join("gold_cids");

    let json_path = fixtures_dir.join("claim_001.json");
    let cid_path = fixtures_dir.join("claim_001.cid");

    let json_bytes = fs::read(&json_path).unwrap_or_else(|e| {
        panic!(
            "slice-01 gold fixture {} missing: {}",
            json_path.display(),
            e
        )
    });
    let slice_01_cid = fs::read_to_string(&cid_path)
        .unwrap_or_else(|e| panic!("frozen slice-01 CID {} missing: {}", cid_path.display(), e))
        .trim()
        .to_string();

    // Precondition: the slice-01 gold fixture MUST NOT carry a `reason`
    // key — it is the wire shape a slice-01 binary actually published.
    let raw: serde_json::Value =
        serde_json::from_slice(&json_bytes).expect("gold fixture is valid JSON");
    assert!(
        !raw.as_object()
            .expect("gold fixture is an object")
            .contains_key("reason"),
        "precondition: the slice-01 gold fixture MUST NOT carry a `reason` key"
    );

    // AND: the semantic premise of the guarantee — a slice-01-era body
    // (no `reason` key) reads as `reason: None` under the slice-03
    // lexicon wire `Claim` (the `#[serde(default, ...)]` on
    // `Claim::reason`, per 01-07 / ADR-015). The gold fixture is the
    // on-disk `claim_domain` shape (`author_did` / `composed_at`), so we
    // pin this `reason: None` reading against the federation-WIRE shape
    // (`author` / `composedAt`) of the SAME slice-01-era claim.
    let slice_01_wire_body = serde_json::json!({
        "subject": raw["subject"],
        "predicate": raw["predicate"],
        "object": raw["object"],
        "evidence": raw["evidence"],
        "confidence": raw["confidence"],
        "author": raw["author_did"],
        "composedAt": raw["composed_at"],
        "references": raw["references"],
    });
    let wire_claim: Claim = serde_json::from_value(slice_01_wire_body).expect(
        "slice-01 wire body (no `reason` key) must deserialize under the slice-03 lexicon `Claim`",
    );
    assert_eq!(
        wire_claim.reason, None,
        "a slice-01 body (no `reason` key) MUST read as `reason: None` under slice-03"
    );

    // WHEN: the slice-03 pure core canonicalizes + CIDs the SAME content
    // as a `claim_domain::UnsignedClaim`. `UnsignedClaim` has no `reason`
    // field at all, so canonicalize emits the slice-01-era CBOR map
    // verbatim — `reason: None` contributes ZERO bytes (this is exactly
    // what `skip_serializing_if = "Option::is_none"` guarantees at the
    // lexicon wire layer, mirrored here at the canonical-CBOR layer).
    let unsigned: UnsignedClaim = serde_json::from_slice(&json_bytes)
        .expect("slice-01 gold body must deserialize as a claim_domain UnsignedClaim");
    let canonical = canonicalize(&unsigned)
        .expect("canonicalize MUST succeed for a slice-01-era reason=None claim");
    let slice_03_cid = compute_cid(&canonical);

    // THEN (I-FED-7 / KPI-FED-1): the slice-03 CID is byte-identical to
    // the FROZEN slice-01 CID. This is the LOAD-BEARING cross-slice
    // attribution-fidelity guarantee: a claim published under slice-01
    // continues to resolve at the SAME at-uri after the author upgrades
    // to slice-03, because `reason: None` adds nothing to the canonical
    // bytes and therefore nothing to the sha2-256 digest / CIDv1.
    assert_eq!(
        slice_03_cid.0.as_str(),
        slice_01_cid.as_str(),
        "CID drift across the slice-01 -> slice-03 upgrade: a reason=None claim \
         computed `{}` but the frozen slice-01 CID is `{}`. The forward-compat \
         contract (I-FED-7) requires byte-identical canonical CBOR -> identical CID, \
         so previously-published author claims keep resolving at the same at-uri.",
        slice_03_cid.0,
        slice_01_cid,
    );
}

// =============================================================================
// WD-35 — NFC normalization (claim_domain::normalize_reason) — PROPERTIES
// =============================================================================

/// LCC-3 / Property (Mandate 9 layer 2 PBT full): `normalize_reason`
/// is IDEMPOTENT. For every UTF-8 string `s`,
/// `normalize_reason(normalize_reason(s)) == normalize_reason(s)`.
/// (data-models.md property 2; WD-35.)
///
/// @property @us-fed-004 @j-003b @wd-35
#[test]
fn lexicon_counter_claim_normalize_reason_is_idempotent_property() {
    use claim_domain::normalize_reason;
    use claim_domain::proptest_strategies::arb_reason_text;
    use proptest::prelude::*;
    use proptest::test_runner::TestRunner;

    // Layer-2 @property (DD-FED-12): pure-core direct invocation, NO
    // subprocess. The harness drives `normalize_reason` (the driving
    // port IS the pure function signature) over a domain-realistic
    // generator — ASCII + Latin-with-combining-marks + CJK — and asserts
    // the idempotency invariant from data-models.md property 2 / WD-35:
    //
    //     forall reason text R:
    //         normalize_reason(R) == normalize_reason(normalize_reason(R))
    //
    // Idempotency is load-bearing because NFC normalization happens once
    // at compose time; if a second pass over already-normalized text
    // changed the bytes, re-normalizing a stored reason (e.g. on display
    // or re-sign) would silently drift the signed CID.
    let mut runner = TestRunner::default();
    runner
        .run(&arb_reason_text(), |reason| {
            let once = normalize_reason(&reason);
            let twice = normalize_reason(&once);
            prop_assert_eq!(
                &twice,
                &once,
                "normalize_reason must be idempotent: applying it to already-NFC text \
                 must be a no-op, else re-normalizing a stored reason drifts the signed CID"
            );
            Ok(())
        })
        .expect("normalize_reason idempotency property must hold for all generated reason texts");
}

/// LCC-4 / Property: `normalize_reason` UNIFIES strings with identical
/// NFC form. For every pair `(r, s)` where `r != s` byte-wise but
/// `NFC(r) == NFC(s)`, `normalize_reason(r) == normalize_reason(s)`.
/// (data-models.md property 3; WD-35.)
///
/// @property @us-fed-004 @j-003b @wd-35
#[test]
fn lexicon_counter_claim_normalize_reason_unifies_canonically_equivalent_strings_property() {
    use claim_domain::normalize_reason;
    use claim_domain::proptest_strategies::arb_nfc_equivalent_pair;
    use proptest::prelude::*;
    use proptest::test_runner::TestRunner;

    // Layer-2 @property (DD-FED-12): the NFC-unification invariant from
    // data-models.md property 3 / WD-35:
    //
    //     forall R, S where R != S byte-wise and NFC(R) == NFC(S):
    //         normalize_reason(R) == normalize_reason(S)
    //
    // The generator produces byte-DISTINCT but canonically-equivalent
    // pairs — e.g. precomposed "é" (U+00E9) vs decomposed "e" + combining
    // acute (U+0065 U+0301). This is the load-bearing property that makes
    // copy-paste workflows deterministic: two users who paste the
    // "same" accented reason from different editors sign byte-identical
    // canonical CBOR and therefore land on the SAME CID.
    let mut runner = TestRunner::default();
    runner
        .run(&arb_nfc_equivalent_pair(), |(r, s)| {
            // Precondition the generator guarantees, asserted so a
            // future generator regression that stops producing distinct
            // pairs fails LOUDLY instead of trivially passing.
            prop_assert_ne!(
                &r,
                &s,
                "generator must yield byte-DISTINCT pairs (else the property is vacuous)"
            );
            prop_assert_eq!(
                normalize_reason(&r),
                normalize_reason(&s),
                "byte-distinct but NFC-equivalent reasons must normalize to the SAME string \
                 so copy-paste workflows produce a stable signed CID"
            );
            Ok(())
        })
        .expect("normalize_reason NFC-unification property must hold for all generated pairs");
}

// =============================================================================
// ADR-015 — `reason` length validation at the wire boundary
// =============================================================================

/// LCC-5: A Lexicon validator (slice-03 layer) rejects a `reason`
/// string of length 0 (`minLength: 1` per ADR-015 schema) AND a
/// `reason` string of length 1001 (`maxLength: 1000`). Pre-CLI
/// defense-in-depth: even if the CLI argument validator is bypassed,
/// the Lexicon-level check holds. (data-models.md §reason field +
/// component-boundaries §lexicon probe slice-03 additions.)
///
/// @us-fed-004 @j-003b @adr-015 @error
#[test]
fn lexicon_counter_claim_rejects_reason_length_outside_one_to_one_thousand() {
    use lexicon::{validate_claim_json, LexiconError};

    // GIVEN: a well-formed `org.openlore.claim` JSON value, parameterized
    // by the `reason` field under test. Every other field is a fixed,
    // valid placeholder so the ONLY thing the validator can reject on is
    // the `reason`-length gate (boundary-pinning). The reason string is
    // built from a single ASCII char repeated `len` times, so character
    // count == byte count here — the boundary assertions hold regardless
    // of whether the validator measures chars or bytes for ASCII input.
    // (The chars-vs-bytes distinction is exercised by the in-crate unit
    // tests in `claim.rs`; this layer-2 test pins the boundary contract.)
    fn claim_value_with_reason(reason: &str) -> serde_json::Value {
        serde_json::json!({
            "subject": "github:rust-lang/cargo",
            "predicate": "embodiesPhilosophy",
            "object": "org.openlore.philosophy.dependency-pinning",
            "evidence": ["https://github.com/rust-lang/cargo/issues/5359"],
            "confidence": 0.42,
            "author": "did:plc:rachel-test#org.openlore.application",
            "composedAt": "2026-05-22T09:18:44Z",
            "references": [
                { "type": "counters", "cid": "bafy-target" }
            ],
            "reason": reason,
            "signature": {
                "kid": "did:plc:rachel-test#org.openlore.application",
                "alg": "EdDSA",
                "sig": "AAAA"
            }
        })
    }

    // WHEN/THEN (criterion 1 — minLength: 1): an empty `reason` ("") is
    // REJECTED. ADR-015 declares `minLength: 1`; the Lexicon-layer gate
    // is defense-in-depth even if the `claim counter` CLI verb (step
    // 05-02, a different layer) is bypassed.
    let empty = claim_value_with_reason("");
    let err = validate_claim_json(&empty)
        .expect_err("an empty `reason` must be REJECTED (minLength: 1, ADR-015)");
    assert_eq!(
        err,
        LexiconError::ReasonLengthOutOfRange { length: 0 },
        "an empty reason must reject with a length-out-of-range error naming length 0"
    );

    // WHEN/THEN (criterion 2 — maxLength: 1000): a 1001-char `reason` is
    // REJECTED.
    let too_long_text = "a".repeat(1001);
    let too_long = claim_value_with_reason(&too_long_text);
    let err = validate_claim_json(&too_long)
        .expect_err("a 1001-char `reason` must be REJECTED (maxLength: 1000, ADR-015)");
    assert_eq!(
        err,
        LexiconError::ReasonLengthOutOfRange { length: 1001 },
        "a 1001-char reason must reject with a length-out-of-range error naming length 1001"
    );

    // WHEN/THEN (criterion 3 — inclusive lower bound): a 1-char `reason`
    // is ACCEPTED (minLength is inclusive).
    let at_min = claim_value_with_reason("x");
    let claim = validate_claim_json(&at_min)
        .expect("a 1-char `reason` must be ACCEPTED (minLength 1 is inclusive)");
    assert_eq!(
        claim.reason.as_deref(),
        Some("x"),
        "the accepted 1-char reason must survive into the parsed Claim"
    );

    // WHEN/THEN (criterion 4 — inclusive upper bound): a 1000-char
    // `reason` is ACCEPTED (maxLength is inclusive).
    let at_max_text = "a".repeat(1000);
    let at_max = claim_value_with_reason(&at_max_text);
    let claim = validate_claim_json(&at_max)
        .expect("a 1000-char `reason` must be ACCEPTED (maxLength 1000 is inclusive)");
    assert_eq!(
        claim.reason.as_deref().map(str::len),
        Some(1000),
        "the accepted 1000-char reason must survive into the parsed Claim"
    );
}
