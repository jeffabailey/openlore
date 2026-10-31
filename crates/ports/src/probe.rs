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

/// The payload of a refused probe — [`ProbeOutcome::Refused`] as a
/// standalone value, for probes that hand back what they observed on
/// success (e.g. the opaque-instance probe returns the manifest it read).
#[derive(Debug, Clone)]
pub struct ProbeRefused {
    pub reason: ProbeRefusalReason,
    pub detail: String,
    pub structured: serde_json::Value,
}

impl From<ProbeRefused> for ProbeOutcome {
    fn from(refused: ProbeRefused) -> Self {
        ProbeOutcome::Refused {
            reason: refused.reason,
            detail: refused.detail,
            structured: refused.structured,
        }
    }
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

    // -------- serverless-philosophy-federation additions (ADR-062 §6) --------
    /// `adapter-publish-http` probe: the configured instance could not be
    /// reached. Reason code `publish.instance_unreachable`.
    PublishInstanceUnreachable,
    /// A pushed record did not recompute (in Rust) to the CID it was pushed
    /// under. Reason code `publish.cid_roundtrip_failed` (KPI-SF-1).
    PublishCidRoundtripFailed,
    /// `adapter-publish-http` probe: the URL is reachable but its
    /// `/manifest` lacks the openlore opaque-instance marker (Q-SF-D5).
    /// Reason code `publish.not_an_openlore_instance`.
    PublishNotAnOpenloreInstance,

    // -------- contributor-philosophy-inference additions (DDD-15) --------
    /// The `contribution_links` upsert-twice sentinel did not refresh the
    /// rank / last-observed, or altered `first_observed_at` (the DuckDB
    /// `ON CONFLICT` lie), or the table is unusable.
    StorageContributionLinkUpsertUnreliable,

    // -------- github-did-identity-link additions (ADR-085 Earned Trust) --------
    /// The `identity_link_checks` sentinel did not keep two checks of one link
    /// (the store overwrote or refused the second), did not read the later one
    /// back as latest, or the table is unusable.
    StorageIdentityLinkCheckHistoryUnreliable,

    // -------- owner-only-signing additions (ADR-087 Earned Trust) --------
    /// The `repo_authority_checks` sentinel did not keep two observations of
    /// one repo, did not read the later one back as latest, or the table is
    /// unusable.
    StorageRepoAuthorityCheckHistoryUnreliable,
    /// The `imported_candidates` sentinel import was not kept or not read
    /// back by its subject, or the table is unusable.
    StorageImportedCandidatesUnreliable,

    // -------- bluesky-claim-review-app additions (ADR-073/074) --------
    /// The private store was written by a newer app (schema_version ahead).
    ReviewStoreSchemaMismatch,
    /// The data key failed the seal/open canary (or a tampered blob opened).
    ReviewStoreAeadCanaryFailed,
    /// An owner-scoped read saw another owner's canary row (I-BRA-1).
    ReviewStoreCrossOwnerBleed,
    /// DuckDB does not report the `memory_limit` / `threads` the review app
    /// configured (B11).
    ReviewStoreCapsNotHonoured,
    /// A rolled-back removal deleted a declined or published suggestion, or
    /// left a pending one of the removed account (ADR-088 §8).
    ReviewStoreRemovalUnsafe,
    /// The OAuth client key cannot sign, or its published half does not verify.
    OAuthClientKeyUnusable,
    /// The published JWKS is not ES256-only or carries a private part.
    OAuthJwksMalformed,

    // -------- indexer-per-did-pds-fetch additions (ADR-077) --------
    /// The indexer's origin classifier misbehaved on its in-process fixtures:
    /// a fallback read was not relay origin, or the resolved PDS was not
    /// author-PDS origin exactly when fetched from it.
    IndexerOriginClassificationUnsound,
    /// The indexer's configuration was refused at startup (a malformed repo
    /// DID or fallback URL, an out-of-range bound, or the loopback test seam
    /// in a release build); `structured` names the variable and its value.
    IndexerConfigInvalid,
    /// The object store accepted a conditional write it should have refused, so
    /// the single-writer guarantee cannot be trusted (scale-to-zero-hosting
    /// D-STZ-39).
    IndexObjectWriteConditionIgnored,
}

// -----------------------------------------------------------------------------
// The write probe's pure rules (scale-to-zero-hosting D-STZ-39). They live here
// so the adapter and the Lambda root share one reading of the store's answers.
// -----------------------------------------------------------------------------

/// The write probe's own key (D-STZ-39). A constant and not a configured value,
/// so configuration can never point it at the index.
pub const WRITE_PROBE_KEY: &str = "index/.write-probe";

/// D-STZ-39: does this location name the write probe's key, as its index or as
/// its pass record? Pure, so it can be tested without building an adapter.
pub fn location_names_write_probe_key(index_key: &str, last_pass_key: &str) -> bool {
    index_key == WRITE_PROBE_KEY || last_pass_key == WRITE_PROBE_KEY
}

/// How the repeat PUT of the write probe ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepeatAnswer {
    /// 412: the first write was a first run, and the store honours the condition.
    Honoured,
    /// 2xx: the store took the same conditional write twice.
    Accepted,
    /// Anything else: unreachable, access denied, a timeout.
    Unknown,
}

/// What the write probe saw (D-STZ-39). "Accepted, with no repeat" cannot be
/// expressed, because a 2xx is always followed by the repeat.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WriteProbeSeen {
    /// The first PUT got 412.
    Honoured,
    /// The first PUT got anything but 2xx or 412.
    Unknown,
    /// The first PUT got 2xx, so it was repeated.
    Repeated(RepeatAnswer),
}

/// D-STZ-39: what the write probe's PUTs say. `Repeated(Accepted)` is the only
/// refusal, with reason `IndexObjectWriteConditionIgnored`; an unknown answer
/// says nothing about conditions, so it is not one. Pure, so that a store that
/// lies can be tested: moto honours conditions.
pub fn write_probe_outcome(seen: WriteProbeSeen) -> ProbeOutcome {
    match seen {
        WriteProbeSeen::Repeated(RepeatAnswer::Accepted) => write_condition_ignored().into(),
        WriteProbeSeen::Honoured
        | WriteProbeSeen::Unknown
        | WriteProbeSeen::Repeated(RepeatAnswer::Honoured | RepeatAnswer::Unknown) => {
            ProbeOutcome::Ok
        }
    }
}

fn write_condition_ignored() -> ProbeRefused {
    ProbeRefused {
        reason: ProbeRefusalReason::IndexObjectWriteConditionIgnored,
        detail: format!("the store accepted the same conditional write to {WRITE_PROBE_KEY} twice"),
        structured: serde_json::json!({ "key": WRITE_PROBE_KEY }),
    }
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

    /// The indexer's refusals serialize to their PascalCase variant names and
    /// roundtrip (ADR-077) — the closed table of indexer-per-did additions.
    #[test]
    fn indexer_refusals_serialize_verbatim_and_roundtrip() {
        for (reason, name) in [
            (
                ProbeRefusalReason::IndexerOriginClassificationUnsound,
                "IndexerOriginClassificationUnsound",
            ),
            (
                ProbeRefusalReason::IndexerConfigInvalid,
                "IndexerConfigInvalid",
            ),
        ] {
            let value = serde_json::to_value(reason).expect("serialize reason");
            assert_eq!(value, json!(name));
            let parsed: ProbeRefusalReason = serde_json::from_value(value).expect("deserialize");
            assert_eq!(parsed, reason);
        }
    }
}
