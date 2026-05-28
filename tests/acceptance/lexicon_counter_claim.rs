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
    let reserialized = serde_json::to_value(&claim)
        .expect("a `reason: None` Claim must re-serialize");
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
    todo!("DELIVER (slice-03): canonicalize + compute_cid an UnsignedClaim with reason=None; assert byte-equal to a gold-fixture CID generated by a slice-01 binary for the same compose-time inputs. Companion to slice-01 LC-3 property test; this is the FORWARD-compat sibling assertion.")
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
    todo!("DELIVER (slice-03): proptest harness over arbitrary UTF-8 strings (bounded to a domain-realistic generator: ASCII + Latin + CJK with combining marks); assert normalize_reason(normalize_reason(s)) == normalize_reason(s) for N=100+ generated examples. Pin proptest seed in proptest.toml per slice-01 DD-3 convention. Layer 2 — pure-core direct invocation, NO subprocess.")
}

/// LCC-4 / Property: `normalize_reason` UNIFIES strings with identical
/// NFC form. For every pair `(r, s)` where `r != s` byte-wise but
/// `NFC(r) == NFC(s)`, `normalize_reason(r) == normalize_reason(s)`.
/// (data-models.md property 3; WD-35.)
///
/// @property @us-fed-004 @j-003b @wd-35
#[test]
fn lexicon_counter_claim_normalize_reason_unifies_canonically_equivalent_strings_property() {
    todo!("DELIVER (slice-03): proptest generates pairs (r, s) where r and s are byte-distinct but NFC-equivalent (use unicode-normalization to construct them) ; assert normalize_reason(r) == normalize_reason(s) for N=100+ pairs. This is the load-bearing property that copy-paste workflows behave deterministically.")
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
    todo!("DELIVER (slice-03): use lexicon::validate to assert empty string ('' for reason) is REJECTED with a minLength error AND a 1001-char string is REJECTED with a maxLength error AND a 1-char + a 1000-char string are ACCEPTED. Boundary-pinning example test; no proptest needed at this layer (Mandate 11 applies even though we're at layer 2 because the contract IS the boundary).")
}
