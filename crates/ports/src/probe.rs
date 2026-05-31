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

    // -------- slice-03 additions (federated read) --------
    /// `peer_subscriptions` / `peer_claims` schema does not match
    /// migration v3. Per ADR-014.
    StoragePeerSchemaMismatch,
    /// Probe-time write of a peer_claim attributed to the local user's
    /// own DID was accepted by the adapter (the adapter is REQUIRED to
    /// reject it). Anti-merging layer-2 enforcement.
    StoragePeerSelfAttribution,
    /// `soft_remove` deleted peer_claims rows (it MUST only set
    /// `removed_at` on the subscription, retaining the cached claims).
    StoragePeerSoftRemoveBleed,
    /// `hard_purge` left orphan peer_claims or peer_claim_references
    /// rows (cascade incomplete) — see ADR-014 transaction shape.
    StoragePeerPurgeIncomplete,
    /// A peer record fetched from the peer's PDS did not recompute to
    /// the same CID locally. Indicates either a canonicalization
    /// regression (claim_domain) or a PDS-side mutation; either way the
    /// adapter MUST refuse to serve.
    PdsPeerCidRoundTripFailed,
    /// A fixture peer DID failed to resolve through the identity
    /// adapter. Either the PLC directory is unreachable or the resolver
    /// is misconfigured.
    IdentityPeerResolutionFailed,

    // -------- slice-02 additions (github scraper) --------
    /// `adapter-github` probe step 1 failed — the public GitHub API was
    /// unreachable (or did not respond within the 250ms budget) for a
    /// stable public fixture target. Emitted as `github.public_api_unreachable`.
    GithubPublicApiUnreachable,
    /// `adapter-github` probe step 2 failed — a known-private / inaccessible
    /// target was NOT refused (the public-data-only guarantee broke). This is
    /// the load-bearing trust event (KPI-SCR-4); emitted as
    /// `github.private_not_refused`.
    GithubPrivateNotRefused,
    /// `adapter-github` probe step 3 failed — a configured `GITHUB_TOKEN` was
    /// rejected (HTTP 401). The adapter refuses to start rather than silently
    /// fall back to the anon budget. Emitted as `github.token_rejected`.
    GithubTokenRejected,
    /// `adapter-github` probe step 4 failed — the rate-limit headers the
    /// budget-reporting path depends on were absent from the response.
    /// Emitted as `github.rate_limit_headers_missing`.
    GithubRateLimitHeadersMissing,

    // -------- slice-06 additions (htmx viewer) --------
    /// `adapter-http-viewer` store-readability probe failed — the read-only
    /// store could not be read (locked by another process, missing). Per ADR-030
    /// §Earned-Trust step 1; surfaced as a plain-language startup refusal naming
    /// the store path, never a per-request crash (NFR-VIEW-6).
    ViewerStoreUnreadable,
    /// `adapter-http-viewer` loopback probe failed — the server bound a
    /// non-loopback address (the viewer is localhost-only, I-VIEW-4).
    ViewerNotLoopback,
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

        assert_eq!(
            value,
            json!("Ok"),
            "Ok variant must serialize to JSON string \"Ok\""
        );

        let parsed: ProbeOutcome = serde_json::from_value(value).expect("deserialize ok");
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

        let parsed: ProbeOutcome = serde_json::from_value(value).expect("deserialize refused");
        match parsed {
            ProbeOutcome::Refused {
                reason,
                detail,
                structured,
            } => {
                assert_eq!(reason, ProbeRefusalReason::StorageFsyncUnreliable);
                assert_eq!(detail, "tmpfs detected; fsync would silently no-op");
                assert_eq!(
                    structured,
                    json!({"medium": "tmpfs", "path": "/dev/shm/openlore"})
                );
            }
            ProbeOutcome::Ok => panic!("roundtripped value must be Refused"),
        }
    }
}
