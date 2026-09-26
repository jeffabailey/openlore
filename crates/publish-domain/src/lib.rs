//! `publish-domain` — the PURE core of the opaque-instance publish transport
//! (ADR-062; serverless-philosophy-federation).
//!
//! Four small pure transformations, composed by the `openlore publish`
//! composition root around the effectful `adapter-publish-http`:
//!
//! 1. [`classify_instance`] — Q-SF-D5: is this `/manifest` JSON an openlore
//!    opaque instance (the discriminator envelope) or an arbitrary URL? Total.
//! 2. [`read_manifest`] / [`classify_manifest_observation`] — parse a
//!    classified manifest into its display entries (manifest v1), with the
//!    probe refusal ordering unreachable → not-openlore → registered;
//!    [`card_url`] derives the instance's public card URL.
//! 3. [`record_bytes_of`] / [`display_projection`] — what a push sends: the
//!    verbatim lexicon-JSON record blob and its manifest display projection.
//! 4. [`verify_round_trip`] / [`round_trip_verdict`] — KPI-SF-1: recompute
//!    the CID (via `claim-domain`, the SOLE canonicalizer) from the bytes the
//!    instance returned and compare it with the CID they were pushed under;
//!    [`judge_readback`] — the push-side verify-before-commit gate (CID
//!    recompute AND verbatim bytes).
//! 5. [`plan_push`] — Q-SF-D3: the bulk-push plan, a pure diff of the local
//!    CID set against the instance manifest's CID set (no resume marker; the
//!    manifest IS the commit log).
//!
//! NO I/O, NO async, NO clock. The instance never computes a CID; this crate
//! never talks to the instance.

#![forbid(unsafe_code)]

use std::collections::HashSet;

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

/// What one `GET /manifest` attempt observed at the transport edge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManifestObservation {
    /// No HTTP response at all (connection refused, DNS failure, timeout).
    Unreachable { url: String, detail: String },
    /// An HTTP response arrived: its status and raw body bytes.
    Responded { status: u16, body: Vec<u8> },
}

/// Classify one probe observation, in refusal order (Q-SF-D5):
///
/// 1. no HTTP response → [`InstanceError::Unreachable`];
/// 2. a non-2xx status, a non-JSON body, or JSON lacking the openlore
///    envelope → [`InstanceError::NotAnOpenloreInstance`];
/// 3. otherwise the parsed manifest — the only outcome that may register.
///
/// Total: never panics.
pub fn classify_manifest_observation(
    observation: &ManifestObservation,
) -> Result<InstanceManifest, InstanceError> {
    match observation {
        ManifestObservation::Unreachable { url, detail } => Err(InstanceError::Unreachable {
            url: url.clone(),
            detail: detail.clone(),
        }),
        ManifestObservation::Responded { status, body } => {
            let manifest = manifest_json_of(*status, body)?;
            read_manifest(&manifest)
        }
    }
}

/// The JSON body of a successful `/manifest` response, or why it cannot be one.
fn manifest_json_of(status: u16, body: &[u8]) -> Result<serde_json::Value, InstanceError> {
    if !(200..300).contains(&status) {
        return Err(not_an_instance(&format!(
            "GET /manifest returned HTTP {status}"
        )));
    }
    serde_json::from_slice(body).map_err(|_| not_an_instance("GET /manifest did not return JSON"))
}

/// The instance's public card URL: the instance root (`<instance_url>/`).
/// Trailing slashes on the input are normalized to exactly one.
pub fn card_url(instance_url: &str) -> String {
    format!("{}/", instance_url.trim_end_matches('/'))
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

/// Why a record staged on the instance must NOT be committed: its read-back
/// did not survive the boundary.
#[derive(Debug, Clone, PartialEq)]
pub enum ReadbackMismatch {
    /// The returned bytes are not a signed lexicon claim at all.
    Unreadable { detail: String },
    /// The returned bytes recompute (in Rust) to a different CID.
    CidDrift { recomputed: Cid },
    /// Same recomputed CID, but the bytes were not stored verbatim (e.g. the
    /// signature block was altered) — the instance must hold exactly what
    /// was sent (ADR-062 §3).
    BytesAltered,
}

impl ReadbackMismatch {
    /// A one-line, human-readable reason for the push report.
    pub fn describe(&self) -> String {
        match self {
            ReadbackMismatch::Unreadable { detail } => format!("read back unreadable: {detail}"),
            ReadbackMismatch::CidDrift { recomputed } => format!("read back as {}", recomputed.0),
            ReadbackMismatch::BytesAltered => "read back with altered bytes".to_string(),
        }
    }
}

/// Push-side judgement (verify BEFORE commit): the staged record may be
/// committed under `pushed` only if the bytes the instance returned
/// recompute to `pushed` AND are exactly the bytes that were `sent`.
pub fn judge_readback(
    pushed: &Cid,
    sent: &RecordBytes,
    returned: &RecordBytes,
) -> Result<Cid, ReadbackMismatch> {
    match verify_round_trip(pushed, returned) {
        Err(UnreadableRecord { detail }) => Err(ReadbackMismatch::Unreadable { detail }),
        Ok(RoundTripVerdict::CidMismatch { recomputed, .. }) => {
            Err(ReadbackMismatch::CidDrift { recomputed })
        }
        Ok(RoundTripVerdict::Verified { .. }) if returned != sent => {
            Err(ReadbackMismatch::BytesAltered)
        }
        Ok(RoundTripVerdict::Verified { cid }) => Ok(cid),
    }
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

// -----------------------------------------------------------------------------
// 5. Bulk push plan (Q-SF-D3)
// -----------------------------------------------------------------------------

/// What a bulk `publish push` will do: the local claims to send, and those
/// the instance's manifest already lists (skipped). Both keep local order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PushPlan {
    pub to_push: Vec<Cid>,
    pub skipped: Vec<Cid>,
}

/// Diff the user's local CIDs against the CIDs the instance's manifest
/// already lists (Q-SF-D3): push only what is missing, skip what is there.
pub fn plan_push(local: &[Cid], remote: &[Cid]) -> PushPlan {
    let already_present: HashSet<&Cid> = remote.iter().collect();
    let (skipped, to_push) = local
        .iter()
        .cloned()
        .partition(|cid| already_present.contains(cid));
    PushPlan { to_push, skipped }
}

/// The CIDs an instance's manifest lists — its committed record set.
pub fn manifest_cids(manifest: &InstanceManifest) -> Vec<Cid> {
    manifest
        .entries
        .iter()
        .map(|entry| Cid(entry.cid.clone()))
        .collect()
}

#[cfg(test)]
mod tests;
