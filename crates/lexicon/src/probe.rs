//! Lexicon module-level startup probe — slice-03 `reason`-field extension
//! (step 01-07, per ADR-015 / WD-23 / WD-32 / component-boundaries
//! §`crates/lexicon` Probe responsibilities).
//!
//! Pure module: no I/O, no async. The probe is the lexicon crate's
//! "Earned Trust" self-check (ADR-005 probe model): at startup the host
//! binary calls [`probe`] once and refuses to run if any invariant of the
//! federation contract is violated. Per slice-03 the probe is extended
//! with four `reason`-field checks:
//!
//!   1. `reason` field DECLARATION in the embedded Lexicon JSON: type
//!      string, `minLength` 1, `maxLength` 1000, NOT in `required[]`.
//!   2. Serde round-trip of a sentinel `Claim` with `reason: Some(..)` —
//!      the value survives serialize→deserialize byte-equal.
//!   3. Serde round-trip of a sentinel `Claim` with `reason: None` — the
//!      serialized JSON does NOT contain the `"reason"` key (this is what
//!      `#[serde(skip_serializing_if)]` buys us).
//!   4. CID stability (I-FED-7): a slice-01-era claim (no `reason`)
//!      serializes byte-IDENTICAL under the slice-03 struct, so the CID a
//!      slice-01 binary would compute is preserved. The CID itself is
//!      computed in `claim-domain` (which owns the CBOR/multihash deps);
//!      the lexicon crate is PURE, so the lexicon-level guarantee is the
//!      *byte-stability of the serialized payload* — a deterministic CID
//!      is then a pure function of those bytes.
//!
//! Why byte-stability is the right lexicon-level check: `compute_cid`
//! lives in `claim-domain` and pulls in `multihash` + CBOR, which the
//! pure-core ban list (`xtask check-arch` invariant 2) forbids inside
//! `lexicon`. Equal serialized bytes ⟹ equal canonical CBOR ⟹ equal CID.
//! The acceptance-level CID-against-gold-fixture check (LC-4) lives in the
//! `cli` test crate, which CAN depend on `claim-domain`.

use crate::claim::{Claim, ClaimReference, SignatureBlock};
use crate::CLAIM_LEXICON_JSON;

// =============================================================================
// Probe error — names the violated invariant so a failing host startup is
// immediately attributable (no string-parsing of a panic message).
// =============================================================================

/// A violation of the lexicon federation contract detected at startup.
///
/// Each variant names the specific invariant that failed so the host
/// binary can log an attributable refusal-to-start message.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ProbeError {
    /// The embedded Lexicon JSON failed to parse — should be impossible
    /// (it is `include_str!`-embedded and parsed at build-adjacent time),
    /// but the probe refuses to assume.
    #[error("embedded org.openlore.claim Lexicon JSON failed to parse: {message}")]
    LexiconJsonUnparseable { message: String },

    /// The `reason` field declaration in the Lexicon JSON does not match
    /// the ADR-015 contract (type/minLength/maxLength/required-membership).
    #[error("`reason` field declaration invalid: {detail}")]
    ReasonFieldDeclaration { detail: String },

    /// A sentinel `Claim` did not survive a serialize→deserialize round
    /// trip (the federation-wire contract is broken).
    #[error("serde round-trip failed for sentinel `{sentinel}`: {detail}")]
    SerdeRoundTrip { sentinel: String, detail: String },

    /// A `reason: None` claim leaked a `"reason"` key into its serialized
    /// JSON (would break CID stability with slice-01 binaries).
    #[error("`reason: None` claim serialized with a `\"reason\"` key (CID-stability / I-FED-7 violation)")]
    ReasonKeyLeaked,

    /// A slice-01-shaped claim did not serialize byte-identically under
    /// the slice-03 struct (CID stability / I-FED-7 violation).
    #[error("slice-01 claim is not byte-stable under the slice-03 struct (CID-stability / I-FED-7 violation): {detail}")]
    CidStability { detail: String },
}

// =============================================================================
// Probe entry point
// =============================================================================

/// Run the lexicon federation-contract startup probe.
///
/// Pure: parses the embedded Lexicon JSON and round-trips in-memory
/// sentinel claims. No I/O. Returns `Ok(())` if every invariant holds,
/// or the first [`ProbeError`] encountered.
pub fn probe() -> Result<(), ProbeError> {
    check_reason_field_declaration()?;
    check_reason_some_roundtrips()?;
    check_reason_none_omits_key()?;
    check_slice_01_claim_is_byte_stable()?;
    Ok(())
}

// =============================================================================
// Probe check 1 — `reason` field declaration in the Lexicon JSON
// =============================================================================

/// Validate the `reason` field declaration in the embedded Lexicon JSON:
/// it MUST be a `string` with `minLength: 1`, `maxLength: 1000`, and it
/// MUST NOT appear in the record's `required[]` array (ADR-015 / WD-32).
fn check_reason_field_declaration() -> Result<(), ProbeError> {
    let schema: serde_json::Value = serde_json::from_str(CLAIM_LEXICON_JSON)
        .map_err(|err| ProbeError::LexiconJsonUnparseable {
            message: err.to_string(),
        })?;

    let record = &schema["defs"]["main"]["record"];

    // Sub-check (a): `reason` is declared in `properties`.
    let reason = record["properties"]
        .get("reason")
        .ok_or_else(|| ProbeError::ReasonFieldDeclaration {
            detail: "`reason` missing from defs.main.record.properties".to_string(),
        })?;

    // Sub-check (b): type is "string".
    if reason["type"].as_str() != Some("string") {
        return Err(ProbeError::ReasonFieldDeclaration {
            detail: format!("expected type \"string\", got {:?}", reason["type"]),
        });
    }

    // Sub-check (c): minLength == 1.
    if reason["minLength"].as_u64() != Some(1) {
        return Err(ProbeError::ReasonFieldDeclaration {
            detail: format!("expected minLength 1, got {:?}", reason["minLength"]),
        });
    }

    // Sub-check (d): maxLength == 1000.
    if reason["maxLength"].as_u64() != Some(1000) {
        return Err(ProbeError::ReasonFieldDeclaration {
            detail: format!("expected maxLength 1000, got {:?}", reason["maxLength"]),
        });
    }

    // Sub-check (e): `reason` is NOT in required[] (forward-compat per ADR-005).
    let required_has_reason = record["required"]
        .as_array()
        .map(|arr| arr.iter().any(|v| v.as_str() == Some("reason")))
        .unwrap_or(false);
    if required_has_reason {
        return Err(ProbeError::ReasonFieldDeclaration {
            detail: "`reason` MUST NOT be listed in required[] (ADR-005 forward-compat)"
                .to_string(),
        });
    }

    Ok(())
}

// =============================================================================
// Probe check 2 — `reason: Some(..)` round-trips byte-equal
// =============================================================================

/// A `Claim` carrying `reason: Some("test")` MUST survive a
/// serialize→deserialize round trip with full value equality.
fn check_reason_some_roundtrips() -> Result<(), ProbeError> {
    let claim = sentinel_claim(Some("test".to_string()));
    let value = serde_json::to_value(&claim).map_err(|err| ProbeError::SerdeRoundTrip {
        sentinel: "reason=Some(\"test\")".to_string(),
        detail: err.to_string(),
    })?;
    let recovered: Claim =
        serde_json::from_value(value).map_err(|err| ProbeError::SerdeRoundTrip {
            sentinel: "reason=Some(\"test\")".to_string(),
            detail: err.to_string(),
        })?;
    if recovered != claim {
        return Err(ProbeError::SerdeRoundTrip {
            sentinel: "reason=Some(\"test\")".to_string(),
            detail: "recovered value differs from original".to_string(),
        });
    }
    Ok(())
}

// =============================================================================
// Probe check 3 — `reason: None` omits the `"reason"` key
// =============================================================================

/// A `Claim` with `reason: None` MUST serialize WITHOUT a `"reason"` key
/// (`skip_serializing_if` drops it). This is the structural guarantee
/// behind CID stability for unextended claims.
fn check_reason_none_omits_key() -> Result<(), ProbeError> {
    let claim = sentinel_claim(None);
    let value = serde_json::to_value(&claim).map_err(|err| ProbeError::SerdeRoundTrip {
        sentinel: "reason=None".to_string(),
        detail: err.to_string(),
    })?;
    let obj = value.as_object().ok_or_else(|| ProbeError::SerdeRoundTrip {
        sentinel: "reason=None".to_string(),
        detail: "serialized claim is not a JSON object".to_string(),
    })?;
    if obj.contains_key("reason") {
        return Err(ProbeError::ReasonKeyLeaked);
    }
    Ok(())
}

// =============================================================================
// Probe check 4 — slice-01 claim is byte-stable under the slice-03 struct
// =============================================================================

/// A slice-01-era claim (which never carried `reason`) MUST serialize to
/// byte-IDENTICAL JSON under the slice-03 struct. Because the CID is a
/// deterministic function of the canonical bytes (computed downstream in
/// `claim-domain`), byte-stable serialization preserves the CID a
/// slice-01 binary would have computed (I-FED-7).
///
/// The reference bytes are the canonical slice-01 federation-wire shape:
/// the field set a slice-01 binary emitted, with NO `reason` key. We
/// re-serialize that exact JSON through `serde_json` to obtain a
/// formatting-normalized reference, then compare against the slice-03
/// struct's serialization of the same content.
fn check_slice_01_claim_is_byte_stable() -> Result<(), ProbeError> {
    // The exact field set a slice-01 binary emits for a signed claim
    // (no `reason` key — slice-01 had no such field). Field ORDER here is
    // irrelevant: both sides are normalized through `serde_json::Value`'s
    // ordered map before byte comparison.
    let slice_01_json = serde_json::json!({
        "subject": "github:rust-lang/rust",
        "predicate": "embodiesPhilosophy",
        "object": "org.openlore.philosophy.memory-safety",
        "evidence": ["https://www.rust-lang.org/"],
        "confidence": 0.86,
        "author": "did:plc:jeff#org.openlore.application",
        "composedAt": "2026-05-25T12:00:00Z",
        "references": [],
        "signature": {
            "kid": "did:plc:jeff#org.openlore.application",
            "alg": "EdDSA",
            "sig": "Zm9vYmFy"
        }
    });

    // Parse the slice-01 JSON into the slice-03 struct, then re-serialize.
    // `reason` defaults to None (via `#[serde(default)]`), so it must NOT
    // reappear in the output (`skip_serializing_if`).
    let claim: Claim =
        serde_json::from_value(slice_01_json.clone()).map_err(|err| ProbeError::CidStability {
            detail: format!("slice-01 JSON failed to deserialize into slice-03 Claim: {err}"),
        })?;

    if claim.reason.is_some() {
        return Err(ProbeError::CidStability {
            detail: "deserializing a slice-01 claim produced reason=Some (must default to None)"
                .to_string(),
        });
    }

    let reserialized = serde_json::to_value(&claim).map_err(|err| ProbeError::CidStability {
        detail: format!("slice-03 Claim failed to re-serialize: {err}"),
    })?;

    // Byte-stability: the slice-03 re-serialization MUST equal the
    // slice-01 input (no `reason` key added, no field dropped). Compared
    // as canonical JSON text so the assertion is on bytes, not Value
    // identity.
    let expected = serde_json::to_string(&slice_01_json).map_err(|err| ProbeError::CidStability {
        detail: format!("reference slice-01 JSON failed to serialize: {err}"),
    })?;
    let actual = serde_json::to_string(&reserialized).map_err(|err| ProbeError::CidStability {
        detail: format!("slice-03 re-serialization failed to stringify: {err}"),
    })?;

    if actual != expected {
        return Err(ProbeError::CidStability {
            detail: format!("byte mismatch: slice-01 `{expected}` vs slice-03 `{actual}`"),
        });
    }

    Ok(())
}

// =============================================================================
// Sentinel builder — a minimal valid Claim parameterized by `reason`
// =============================================================================

/// Build a sentinel `Claim` for round-trip probing, parameterized by the
/// optional `reason`. All other fields are fixed, valid placeholders.
fn sentinel_claim(reason: Option<String>) -> Claim {
    Claim {
        subject: "github:rust-lang/rust".to_string(),
        predicate: "embodiesPhilosophy".to_string(),
        object: "org.openlore.philosophy.memory-safety".to_string(),
        evidence: vec!["https://www.rust-lang.org/".to_string()],
        confidence: 0.86,
        author: "did:plc:jeff#org.openlore.application".to_string(),
        composed_at: "2026-05-25T12:00:00Z".to_string(),
        references: Vec::<ClaimReference>::new(),
        reason,
        signature: Some(SignatureBlock {
            kid: "did:plc:jeff#org.openlore.application".to_string(),
            alg: "EdDSA".to_string(),
            sig: "Zm9vYmFy".to_string(),
        }),
    }
}

// =============================================================================
// In-crate unit tests (RED_UNIT, step 01-07)
// =============================================================================
//
// Each probe sub-check has a focused unit test asserting the invariant it
// guards, plus one happy-path test that the whole probe passes. Per the
// nw-fp-domain-modeling §3 + nw-tdd-methodology layered discipline, these
// are pure-function tests (single deterministic output) — the property
// over the full claim lattice (`forall reason text: idempotent NFC`) lives
// in `claim-domain::normalize_reason`, not in the lexicon wire layer.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probe_passes_for_the_slice_03_lexicon() {
        probe().expect("the slice-03 lexicon must pass its own startup probe");
    }

    #[test]
    fn reason_field_is_declared_string_with_length_bounds_and_not_required() {
        // Check 1: the embedded Lexicon JSON declares `reason` per ADR-015.
        check_reason_field_declaration()
            .expect("`reason` must be declared as optional string[1..=1000]");

        // Spot-assert the raw JSON directly too, so a failure points at the
        // schema file rather than only the helper.
        let schema: serde_json::Value =
            serde_json::from_str(CLAIM_LEXICON_JSON).expect("embedded Lexicon JSON parses");
        let record = &schema["defs"]["main"]["record"];
        let reason = &record["properties"]["reason"];
        assert_eq!(reason["type"].as_str(), Some("string"));
        assert_eq!(reason["minLength"].as_u64(), Some(1));
        assert_eq!(reason["maxLength"].as_u64(), Some(1000));
        let required = record["required"].as_array().expect("required[] is an array");
        assert!(
            !required.iter().any(|v| v.as_str() == Some("reason")),
            "`reason` MUST NOT be in required[] (ADR-005 forward-compat)"
        );
    }

    #[test]
    fn reason_some_round_trips_byte_equal() {
        // Check 2: reason=Some("test") survives serialize→deserialize.
        check_reason_some_roundtrips().expect("reason=Some must round-trip");

        let original = sentinel_claim(Some("test".to_string()));
        let value = serde_json::to_value(&original).expect("serialize");
        assert_eq!(
            value.get("reason").and_then(|r| r.as_str()),
            Some("test"),
            "a present reason MUST serialize under the `reason` key"
        );
        let recovered: Claim = serde_json::from_value(value).expect("deserialize");
        assert_eq!(original, recovered, "reason=Some must round-trip verbatim");
        assert_eq!(recovered.reason.as_deref(), Some("test"));
    }

    #[test]
    fn reason_none_serializes_without_the_reason_key() {
        // Check 3: reason=None drops the key entirely (skip_serializing_if).
        check_reason_none_omits_key().expect("reason=None must omit the key");

        let claim = sentinel_claim(None);
        let value = serde_json::to_value(&claim).expect("serialize");
        let obj = value.as_object().expect("top-level object");
        assert!(
            !obj.contains_key("reason"),
            "reason=None MUST NOT emit a `reason` key (CID stability / I-FED-7); got keys {:?}",
            obj.keys().collect::<Vec<_>>()
        );
    }

    #[test]
    fn slice_01_claim_is_byte_stable_under_slice_03_struct() {
        // Check 4: a slice-01-era claim serializes byte-identical under the
        // slice-03 struct, preserving the CID a slice-01 binary computes
        // (I-FED-7). This is the lexicon-level (pure, no multihash)
        // guarantee that underwrites the gold-fixture CID test in `cli`.
        check_slice_01_claim_is_byte_stable()
            .expect("a slice-01 claim must be byte-stable under the slice-03 struct");
    }

    #[test]
    fn deserializing_a_claim_without_reason_defaults_to_none() {
        // Forward-compat: slice-03 binary reading a slice-01 payload (no
        // `reason` key) MUST default `reason` to None, not error.
        let slice_01_json = serde_json::json!({
            "subject": "s",
            "predicate": "p",
            "object": "o",
            "confidence": 0.5,
            "author": "did:plc:x#org.openlore.application",
            "composedAt": "2026-05-25T12:00:00Z",
        });
        let claim: Claim =
            serde_json::from_value(slice_01_json).expect("slice-01 payload must deserialize");
        assert!(
            claim.reason.is_none(),
            "absent `reason` key MUST deserialize to None (#[serde(default)])"
        );
    }
}
