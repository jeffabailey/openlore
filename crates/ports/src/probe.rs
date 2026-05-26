//! `probe` — the Earned-Trust startup contract every adapter must answer.
//!
//! Every adapter exposes a `probe(&self) -> ProbeOutcome` method. The
//! composition root walks the gauntlet at startup and refuses to serve
//! traffic if any adapter returns `Refused`. The structured JSON shape
//! emitted by `serde_json::to_value(&outcome)` is the contract that
//! `tracing` consumes as the `health.startup.refused` event payload —
//! tests in this module pin that contract.
//!
//! See ADR-009 (hexagonal composition root) and
//! `docs/feature/openlore-foundation/design/component-boundaries.md`
//! §"crates/ports public surface".

use serde::{Deserialize, Serialize};

/// What a `probe()` call answers: "are you safe to start serving traffic?".
///
/// Adapters return `Ok` once their probe gauntlet (per-adapter; see ADRs
/// 001/002/004) all-greens. `Refused` carries enough structured detail
/// for the DevOps observability layer to emit a machine-parsable
/// `health.startup.refused` tracing event without further enrichment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProbeOutcome {
    Ok,
    Refused {
        reason: ProbeRefusalReason,
        detail: String,
        structured: serde_json::Value,
    },
}

/// Why an adapter refused to start. `#[non_exhaustive]` so new adapters
/// can extend the enum without a SemVer break for downstream consumers
/// that pattern-match on it.
///
/// Variants are PascalCase identifiers; serde serializes them verbatim
/// (e.g. `"StorageFsyncUnreliable"`) — this is the on-the-wire shape
/// the tracing layer emits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum ProbeRefusalReason {
    StorageFsyncUnreliable,
    StorageSchemaMismatch,
    IdentityKeyPermsUnsafe,
    IdentityKeychainUnreachable,
    IdentityDidDocumentMismatch,
    PdsTlsHandshakeFailed,
    PdsDidMismatch,
    PdsIdempotencyViolation,
    LexiconInvalid,
    LexiconSerdeRoundTripFailed,
}

// -----------------------------------------------------------------------------
// JSON contract tests — pin the shape the DevOps tracing layer consumes.
// -----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    //! These tests pin the **observable JSON contract** of `ProbeOutcome`,
    //! NOT internal field names. The tracing layer's
    //! `health.startup.refused` event consumes this exact shape; changing
    //! it is a contract break and must red these tests first.

    use super::*;
    use serde_json::json;

    /// Property: `ProbeOutcome::Ok` serializes to the string `"Ok"` and
    /// roundtrips losslessly. This is the externally-tagged unit-variant
    /// shape serde produces by default — pinning it documents the
    /// contract.
    #[test]
    fn probe_outcome_ok_serializes_to_string_ok_and_roundtrips() {
        let outcome = ProbeOutcome::Ok;
        let value = serde_json::to_value(&outcome).expect("serialize ok");

        assert_eq!(value, json!("Ok"), "Ok variant must serialize to JSON string \"Ok\"");

        let parsed: ProbeOutcome =
            serde_json::from_value(value).expect("deserialize ok");
        assert!(
            matches!(parsed, ProbeOutcome::Ok),
            "roundtripped value must equal ProbeOutcome::Ok"
        );
    }

    /// Property: `ProbeOutcome::Refused` serializes to the
    /// externally-tagged shape `{"Refused":{"reason":"<Variant>","detail":"...","structured":<json>}}`.
    /// The `reason` field carries a PascalCase variant name; the
    /// `structured` field is arbitrary JSON the adapter may use to
    /// surface diagnostic detail.
    #[test]
    fn probe_outcome_refused_carries_reason_detail_and_structured_payload() {
        let outcome = ProbeOutcome::Refused {
            reason: ProbeRefusalReason::StorageFsyncUnreliable,
            detail: "tmpfs detected; fsync would silently no-op".to_string(),
            structured: json!({"medium": "tmpfs", "path": "/dev/shm/openlore"}),
        };

        let value = serde_json::to_value(&outcome).expect("serialize refused");

        let expected = json!({
            "Refused": {
                "reason": "StorageFsyncUnreliable",
                "detail": "tmpfs detected; fsync would silently no-op",
                "structured": {"medium": "tmpfs", "path": "/dev/shm/openlore"},
            }
        });
        assert_eq!(
            value, expected,
            "Refused must serialize as externally-tagged object with reason/detail/structured fields"
        );

        let parsed: ProbeOutcome =
            serde_json::from_value(value).expect("deserialize refused");
        match parsed {
            ProbeOutcome::Refused {
                reason,
                detail,
                structured,
            } => {
                assert_eq!(reason, ProbeRefusalReason::StorageFsyncUnreliable);
                assert_eq!(detail, "tmpfs detected; fsync would silently no-op");
                assert_eq!(structured, json!({"medium": "tmpfs", "path": "/dev/shm/openlore"}));
            }
            ProbeOutcome::Ok => panic!("roundtripped value must be Refused"),
        }
    }
}
