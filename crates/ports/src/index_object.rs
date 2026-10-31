//! `index_object` — the two ports over the ONE published index object (ADR-090
//! §1, §2): `index/index.duckdb` plus `index/last-pass.json`, in a versioned
//! bucket. ASYNC (network object store), so `#[async_trait]` is permitted as for
//! `IdentityResolvePort`.
//!
//! Read and write are separate ports (D-STZ-38). [`IndexObjectReadPort`] has no
//! write method; search and probe hold only that one, so a read-only mode cannot
//! name a publish, and search calls `head` and `fetch_to` only.
//! [`IndexObjectWritePort`] is held by the pass, and never by its no-publish arm,
//! which is a function of the read port alone (D-STZ-44).
//!
//! The ETag is the object's version. The pass is the only writer and publishes
//! conditionally (`If-Match: <etag read at the start>`, or `If-None-Match: *`
//! when no object exists yet); a lost update comes back as
//! [`PublishOutcome::Conflict`], never as a silent overwrite (D-STZ-6).
//!
//! Also here: [`IndexFingerprint`], what the pass compares before and after a
//! run to decide whether anything changed (D-STZ-8).

use std::path::Path;
use std::time::Duration;

use async_trait::async_trait;

use crate::ProbeOutcome;

/// The longest a publish may take (D-STZ-40a). The pass narrows it by the time it
/// has left, and the adapter's call table reads the same figure.
pub const PUBLISH_OPERATION_TIMEOUT: Duration = Duration::from_secs(35);

/// The version of the index object as the store reports it (`HEAD` / `GET`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectVersion {
    /// The store's entity tag; the object's version identity.
    pub etag: String,
    /// Object metadata `published-at` (RFC 3339), set by the pass that wrote it.
    pub published_at: Option<String>,
    /// Object metadata `pass-id`, set by the pass that wrote it.
    pub pass_id: Option<String>,
    /// The object's size in bytes.
    pub size_bytes: u64,
}

/// What a `HEAD` of the index object found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HeadOutcome {
    Present(ObjectVersion),
    /// No object (a fresh bucket, or after `reset-index`); never an error.
    Absent,
}

/// The condition a publish is made under (D-STZ-6).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WriteCondition {
    /// Replace only the version read at the start of the pass.
    IfMatch(String),
    /// Create only when no object exists.
    IfNoneMatch,
}

/// The metadata a publish stamps on the new object version (D-STZ-12).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublishMetadata {
    pub published_at: String,
    pub pass_id: String,
}

/// What a conditional publish did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PublishOutcome {
    Published(ObjectVersion),
    /// The condition failed (HTTP 412): another version is current.
    Conflict,
}

/// `index/last-pass.json`: written on every pass that writes a `pass_summary`
/// (U-STZ-3, D-STZ-7), read by the probe. `exit_code` is the exit decided before
/// this record's own write (D-STZ-40).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LastPass {
    pub pass_id: String,
    pub exit_code: i32,
    /// RFC 3339.
    pub finished_at: String,
    pub published: bool,
    /// RFC 3339 (U-STZ-6, D-STZ-48): the `published-at` of this pass's publish,
    /// otherwise of the object seen at the start of the pass, otherwise `None`.
    pub index_published_at: Option<String>,
}

/// Why an index-object operation failed. A missing object is NOT an error
/// ([`HeadOutcome::Absent`], `fetch_to` → `Ok(None)`).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum IndexObjectError {
    #[error("index object probe refused: {detail}")]
    ProbeRefused { detail: String },
    /// The store refused the request (for example access denied).
    #[error("index object store refused the request: {detail}")]
    Refused { detail: String },
    /// The store could not be reached or answered with a server error.
    #[error("index object store unavailable: {detail}")]
    Unavailable { detail: String },
    /// The download could not be written locally (for example no space left).
    #[error("index object could not be written locally: {detail}")]
    LocalWrite { detail: String },
}

/// Reading the published index object (ADR-090 §1, D-STZ-3, D-STZ-38). It
/// exposes no write method: search and probe are composed with this port only.
#[async_trait]
pub trait IndexObjectReadPort: Send + Sync {
    /// Earned-Trust probe — see ADR-009 + `probe.rs`. Proves a `HEAD` through
    /// the role (D-STZ-39).
    fn probe(&self) -> ProbeOutcome;

    /// The current version of the index object, or `Absent`.
    async fn head(&self) -> Result<HeadOutcome, IndexObjectError>;

    /// Download the current index object to `dest` (a new file). `Ok(None)`
    /// when no object exists. A fetch that fails, runs out of time, or is dropped
    /// by its caller leaves no file, and a body shorter than the object is a
    /// failed fetch.
    async fn fetch_to(&self, dest: &Path) -> Result<Option<ObjectVersion>, IndexObjectError>;

    /// Read `index/last-pass.json`; `Ok(None)` when no pass recorded one yet,
    /// an error when one exists and cannot be read (UI-12).
    async fn get_last_pass(&self) -> Result<Option<LastPass>, IndexObjectError>;
}

/// Writing the published index object (ADR-090 §2, D-STZ-6, D-STZ-38). Held by
/// the pass only, and never by its no-publish arm (D-STZ-44).
#[async_trait]
pub trait IndexObjectWritePort: Send + Sync {
    /// The write probe (D-STZ-39), run at pass init: a zero-byte conditional
    /// write to its own key, a constant in the adapter and not configuration, so
    /// it can never touch the index. A 412 means the condition is honoured, the usual case. A 2xx is a
    /// first run or a store that ignores conditions, so the same PUT is sent
    /// again, and the repeat must get 412. `Refused` means exactly two 2xx: the
    /// store ignored the condition, on a first run or any later one, and the
    /// pass then refuses to start. Anything else on either PUT is not a refusal:
    /// that pass runs and reports through D-STZ-41. It proves `If-None-Match`
    /// only. Sync, like every port probe (ADR-009): async init calls it through
    /// `spawn_blocking`.
    fn probe(&self) -> ProbeOutcome;

    /// Publish the file at `src` as the new index object, under `condition`,
    /// within `limit` (D-STZ-40a). A zero `limit` fails at once and sends nothing.
    async fn publish_if_match(
        &self,
        src: &Path,
        condition: WriteCondition,
        metadata: PublishMetadata,
        limit: Duration,
    ) -> Result<PublishOutcome, IndexObjectError>;

    /// Write `index/last-pass.json` (unconditional).
    async fn put_last_pass(&self, last: &LastPass) -> Result<(), IndexObjectError>;
}

/// What the pass compares before and after a run (D-STZ-8): the claim count,
/// the distinct author count, and the XOR of a 64-bit hash of every CID. A
/// re-upsert of the same claim leaves it unchanged. It carries no claim data.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct IndexFingerprint {
    pub claims: u64,
    pub authors: u64,
    pub cid_hash_xor: u64,
}
