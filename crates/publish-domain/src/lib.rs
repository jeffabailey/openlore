//! `publish-domain` — the PURE core of the opaque-instance publish transport
//! (ADR-062; serverless-philosophy-federation).
//!
//! Four small pure transformations, composed by the `openlore publish`
//! composition root around the effectful `adapter-publish-http`:
//!
//! 1. [`classify_instance`] — Q-SF-D5: is this `/manifest` JSON an openlore
//!    opaque instance (the discriminator envelope) or an arbitrary URL? Total.
//! 2. [`read_manifest`] — parse a classified manifest into its display
//!    entries (manifest v1).
//! 3. [`record_bytes_of`] / [`display_projection`] — what a push sends: the
//!    verbatim lexicon-JSON record blob and its manifest display projection.
//! 4. [`verify_round_trip`] / [`round_trip_verdict`] — KPI-SF-1: recompute
//!    the CID (via `claim-domain`, the SOLE canonicalizer) from the bytes the
//!    instance returned and compare it with the CID they were pushed under.
//!
//! NO I/O, NO async, NO clock. The instance never computes a CID; this crate
//! never talks to the instance.

#![forbid(unsafe_code)]

use claim_domain::{Cid, SignedClaim};
use ports::{
    InstanceError, InstanceKind, InstanceManifest, ManifestEntry, RecordBytes, RoundTripVerdict,
};

/// The manifest discriminator `kind` value marking an openlore opaque instance.
pub const OPAQUE_INSTANCE_KIND: &str = "opaque-instance";

// -----------------------------------------------------------------------------
// 1. Manifest-marker classification (Q-SF-D5)
// -----------------------------------------------------------------------------

/// Classify a `/manifest` response body. An openlore opaque instance carries
/// `{"openlore": {"kind": "opaque-instance", "contract_version": <u64>}}`;
/// anything else (HTML, other JSON, a missing or malformed envelope) is
/// [`InstanceKind::NotAnOpenloreInstance`]. Total: never panics.
pub fn classify_instance(manifest: &serde_json::Value) -> InstanceKind {
    let envelope = manifest.get("openlore");
    let kind = envelope
        .and_then(|e| e.get("kind"))
        .and_then(|k| k.as_str());
    let contract_version = envelope
        .and_then(|e| e.get("contract_version"))
        .and_then(serde_json::Value::as_u64);
    match (kind, contract_version) {
        (Some(OPAQUE_INSTANCE_KIND), Some(contract_version)) => {
            InstanceKind::OpaqueInstance { contract_version }
        }
        _ => InstanceKind::NotAnOpenloreInstance,
    }
}

// -----------------------------------------------------------------------------
// 2. Manifest v1 parse
// -----------------------------------------------------------------------------

/// Parse a manifest body: classify it, then read its `records[]` display
/// entries. A missing marker or malformed entries mean the remote does not
/// honour the openlore contract.
pub fn read_manifest(manifest: &serde_json::Value) -> Result<InstanceManifest, InstanceError> {
    let contract_version = match classify_instance(manifest) {
        InstanceKind::OpaqueInstance { contract_version } => contract_version,
        InstanceKind::NotAnOpenloreInstance => {
            return Err(not_an_instance(
                "the /manifest lacks the openlore marker envelope",
            ))
        }
    };
    let records = manifest
        .get("records")
        .cloned()
        .unwrap_or_else(|| serde_json::Value::Array(Vec::new()));
    let entries: Vec<ManifestEntry> = serde_json::from_value(records)
        .map_err(|err| not_an_instance(&format!("manifest records are malformed: {err}")))?;
    Ok(InstanceManifest {
        contract_version,
        entries,
    })
}

fn not_an_instance(detail: &str) -> InstanceError {
    InstanceError::NotAnOpenloreInstance {
        detail: detail.to_string(),
    }
}

// -----------------------------------------------------------------------------
// 3. What a push sends
// -----------------------------------------------------------------------------

/// The record blob for a signed claim: its lexicon-JSON wire form as bytes.
/// The instance stores exactly these bytes, verbatim (ADR-062 §3).
pub fn record_bytes_of(signed: &SignedClaim) -> RecordBytes {
    let wire = lexicon::encode_signed_claim(signed);
    RecordBytes(serde_json::to_vec(&wire).expect("a serde_json::Value always serializes"))
}

/// The manifest v1 display projection of a signed claim. Every field is
/// copied VERBATIM — display only; trust is re-established on pull.
pub fn display_projection(signed: &SignedClaim) -> ManifestEntry {
    let claim = &signed.unsigned;
    ManifestEntry {
        cid: signed.signature.signed_cid.0.clone(),
        author_did: claim.author_did.0.clone(),
        subject: claim.subject.clone(),
        predicate: claim.predicate.clone(),
        object: claim.object.clone(),
        confidence: claim.confidence.value(),
        composed_at: claim.composed_at.clone(),
    }
}

// -----------------------------------------------------------------------------
// 4. Round-trip CID verdict (KPI-SF-1)
// -----------------------------------------------------------------------------

/// Compare the CID a record was pushed under with the CID Rust recomputed.
pub fn round_trip_verdict(pushed: &Cid, recomputed: &Cid) -> RoundTripVerdict {
    if pushed == recomputed {
        RoundTripVerdict::Verified {
            cid: pushed.clone(),
        }
    } else {
        RoundTripVerdict::CidMismatch {
            pushed: pushed.clone(),
            recomputed: recomputed.clone(),
        }
    }
}

/// Why returned bytes could not even be re-parsed into a signed claim.
#[derive(Debug, Clone, PartialEq)]
pub struct UnreadableRecord {
    pub detail: String,
}

/// Re-parse the bytes an instance returned (lexicon JSON), recompute their
/// CID through `claim-domain`, and judge the round trip against `pushed`.
pub fn verify_round_trip(
    pushed: &Cid,
    returned: &RecordBytes,
) -> Result<RoundTripVerdict, UnreadableRecord> {
    let recomputed = recompute_cid(returned)?;
    Ok(round_trip_verdict(pushed, &recomputed))
}

fn recompute_cid(returned: &RecordBytes) -> Result<Cid, UnreadableRecord> {
    let unreadable = |detail: String| UnreadableRecord { detail };
    let wire: serde_json::Value = serde_json::from_slice(&returned.0)
        .map_err(|err| unreadable(format!("record is not JSON: {err}")))?;
    // The lexicon decode re-canonicalizes the unsigned claim through
    // `claim-domain` and carries the RECOMPUTED CID as `signed_cid` — it never
    // trusts a CID from the wire.
    lexicon::decode_signed_claim(&wire)
        .map(|signed| signed.signature.signed_cid)
        .map_err(|err| unreadable(format!("record is not a signed lexicon claim: {err}")))
}

#[cfg(test)]
mod tests;
