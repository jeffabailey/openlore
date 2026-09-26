//! `publish` — the opaque-instance transport ports (ADR-062;
//! serverless-philosophy-federation).
//!
//! A user's serverless instance is an OPAQUE, content-addressed HTTP blob
//! store: it stores/returns lexicon-JSON signed records VERBATIM keyed by the
//! Rust-minted CID and computes no CID itself. Two ports model it, split by
//! CAPABILITY (DDD-8 / ADR-062 §6 Earned Trust):
//!
//! - [`InstanceReadPort`] — READ-ONLY (`probe`, `fetch_manifest`,
//!   `get_record`). Exposes NO write method, so the pull / card /
//!   cross-instance paths that hold only this port can never write to an
//!   instance — the type system makes it un-callable.
//! - [`PublishPort`] — the write-capable extension (`put_record`,
//!   `commit_manifest_entry`). Wired ONLY by the `openlore publish`
//!   composition root.
//!
//! Both ports are SYNC (like `IdentityPort`): the publish verbs are
//! sequential and the adapter uses a blocking HTTP client.

use claim_domain::Cid;
use serde::{Deserialize, Serialize};

// -----------------------------------------------------------------------------
// Boundary ADTs
// -----------------------------------------------------------------------------

/// The exact bytes of one record blob — a verbatim lexicon-JSON signed
/// record. Opaque to the instance; only the Rust core gives it meaning.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordBytes(pub Vec<u8>);

/// One manifest entry: the per-CID DISPLAY projection the CLI writes on push
/// (manifest v1). Advisory for display only — trust is always re-established
/// by the puller recomputing the CID in Rust.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ManifestEntry {
    pub cid: String,
    pub author_did: String,
    pub subject: String,
    pub predicate: String,
    pub object: String,
    pub confidence: f64,
    pub composed_at: String,
}

/// What a `/manifest` says the remote is (Q-SF-D5 detection marker).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstanceKind {
    /// The manifest carries the openlore discriminator envelope
    /// `{openlore:{kind:"opaque-instance", contract_version:N}}`.
    OpaqueInstance { contract_version: u64 },
    /// Anything else — an arbitrary URL, not an openlore instance.
    NotAnOpenloreInstance,
}

/// A parsed openlore instance manifest: its contract version + the committed
/// display entries in instance order.
#[derive(Debug, Clone, PartialEq)]
pub struct InstanceManifest {
    pub contract_version: u64,
    pub entries: Vec<ManifestEntry>,
}

/// The round-trip verdict for one record: does the CID the CLI pushed under
/// equal the CID Rust recomputes from the bytes that came back (KPI-SF-1)?
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RoundTripVerdict {
    Verified { cid: Cid },
    CidMismatch { pushed: Cid, recomputed: Cid },
}

// -----------------------------------------------------------------------------
// Probe-refusal / failure reason codes (the dotted `publish.*` vocabulary)
// -----------------------------------------------------------------------------

/// The instance could not be reached (connection refused, DNS, timeout).
pub const REASON_INSTANCE_UNREACHABLE: &str = "publish.instance_unreachable";
/// A pushed record did not recompute to the CID it was pushed under.
pub const REASON_CID_ROUNDTRIP_FAILED: &str = "publish.cid_roundtrip_failed";
/// The URL is reachable but its `/manifest` lacks the openlore marker.
pub const REASON_NOT_AN_OPENLORE_INSTANCE: &str = "publish.not_an_openlore_instance";

/// Why an instance operation failed. Railway-style: every adapter failure is
/// a value, never a panic.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum InstanceError {
    #[error("cannot reach instance at {url} (instance unreachable): {detail}")]
    Unreachable { url: String, detail: String },
    #[error("not an openlore instance: {detail}")]
    NotAnOpenloreInstance { detail: String },
    #[error("record {cid} not found on the instance")]
    RecordNotFound { cid: String },
    #[error("instance rejected the request (HTTP {status}): {detail}")]
    Rejected { status: u16, detail: String },
}

impl InstanceError {
    /// The dotted `publish.*` reason code for this failure, when it maps to
    /// one of the probe-refusal reasons.
    pub fn reason_code(&self) -> Option<&'static str> {
        match self {
            Self::Unreachable { .. } => Some(REASON_INSTANCE_UNREACHABLE),
            Self::NotAnOpenloreInstance { .. } => Some(REASON_NOT_AN_OPENLORE_INSTANCE),
            Self::RecordNotFound { .. } | Self::Rejected { .. } => None,
        }
    }
}

// -----------------------------------------------------------------------------
// Ports
// -----------------------------------------------------------------------------

/// READ-ONLY instance surface. Deliberately has NO write method (DDD-8).
pub trait InstanceReadPort {
    /// Earned-Trust probe (ADR-062 §6, amended 2026-09-25): reachability +
    /// the `/manifest` openlore marker ONLY — no canary write.
    fn probe(&self) -> crate::ProbeOutcome;

    /// `GET /manifest`, parsed; a manifest lacking the openlore marker is
    /// [`InstanceError::NotAnOpenloreInstance`].
    fn fetch_manifest(&self) -> Result<InstanceManifest, InstanceError>;

    /// `GET /records/:cid` — the exact stored bytes.
    fn get_record(&self, cid: &Cid) -> Result<RecordBytes, InstanceError>;
}

/// WRITE-capable instance surface — wired ONLY by the `openlore publish`
/// composition root (ADR-062 §6).
pub trait PublishPort: InstanceReadPort {
    /// Stage: `PUT /records/:cid` with the verbatim record bytes (idempotent;
    /// the blob is NOT yet listed in the manifest).
    fn put_record(&self, cid: &Cid, record: &RecordBytes) -> Result<(), InstanceError>;

    /// Commit: re-`PUT /records/:cid` (an idempotent no-op on the blob)
    /// carrying the display projection, which appends the entry to the
    /// manifest. Called only AFTER the read-back CID verified (RT-2 style:
    /// verify before manifest append; manifest append = commit).
    fn commit_manifest_entry(
        &self,
        cid: &Cid,
        record: &RecordBytes,
        entry: &ManifestEntry,
    ) -> Result<(), InstanceError>;
}
