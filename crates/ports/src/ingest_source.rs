//! `ingest_source` — the indexer-side bounded-PULL ingest port (ADR-024) +
//! its railway error + the fetched-but-unverified `RawRecord` value.
//!
//! `IngestSourcePort` is ASYNC (network I/O) and READ-ONLY by construction
//! (capability boundary I-AV-5): there is NO write / sign / publish method on
//! this trait. The indexer is signing-incapable and holds no local store; the
//! absence of those methods is the type-level half of that boundary (the
//! structural `xtask check-arch` rule + the composition-root probe are the
//! other layers).
//!
//! `RawRecord` is hoisted here from `appview-domain` (step 01-02): it is the
//! shared shape between the ingest adapter (which produces it) and the pure
//! `appview-domain::ingest_decision` gate (which consumes it). `ports` is the
//! non-cyclic home. See data-models.md §"In-memory value types".
//
// SCAFFOLD: true  (trait surface only; the adapter impl lands in step 01-03/04)

use async_trait::async_trait;
use claim_domain::{Cid, SignedClaim};

use crate::ProbeOutcome;

// -----------------------------------------------------------------------------
// RawRecord — the fetched-but-unverified network record
// -----------------------------------------------------------------------------

/// A fetched-but-not-yet-verified network record — the `IngestSourcePort::enumerate`
/// output the verify-before-index gate consumes. Transient; NEVER persisted as-is.
///
/// `PartialEq` (not `Eq`) because `SignedClaim` carries an `f64` confidence.
#[derive(Debug, Clone, PartialEq)]
pub struct RawRecord {
    /// The network-published rkey/CID (recomputed + verified at ingest).
    pub published_cid: Cid,
    /// The signed-claim value (author/subject/object/confidence/signature/...).
    pub raw_payload: SignedClaim,
    /// The PDS/relay URL it was pulled from (provenance; NOT signed).
    pub source_pds: String,
}

// -----------------------------------------------------------------------------
// IngestError — the railway-oriented failure surface
// -----------------------------------------------------------------------------

/// Why a bounded ingest PULL failed. The ingest source is an external network
/// dependency; these are the transport/shape failures the indexer's pull loop
/// classifies. (Verification rejection is NOT here — that is the pure
/// `appview-domain::ingest_decision` gate's `RejectReason`, applied AFTER a
/// `RawRecord` is fetched.)
#[derive(Debug, thiserror::Error)]
pub enum IngestError {
    #[error("ingest source probe refused: {detail}")]
    ProbeRefused { detail: String },
    #[error("ingest source unreachable: {message}")]
    Unreachable { message: String },
    #[error("ingest source returned a malformed response: {message}")]
    BadResponse { message: String },
    /// The transport policy (DD-IPF-5) left no admissible address for the
    /// source: a refused scheme or IP literal, or every address DNS returned
    /// was in a refused range. Nothing was contacted.
    #[error("ingest source address refused: {message}")]
    AddressRefused { message: String },
}

// -----------------------------------------------------------------------------
// IngestSourcePort — read-only, bounded PULL (ADR-024 / I-AV-5)
// -----------------------------------------------------------------------------

/// The indexer-side ingest port: bounded PULL of public `org.openlore.claim`
/// records from a network source (seed DIDs → PDS `listRecords`; an optional
/// configured relay). ASYNC (network I/O) so `#[async_trait]` is permitted
/// exactly as for `PdsPort`/`GithubPort` (ADR-004).
///
/// READ-ONLY by construction (I-AV-5): there is intentionally NO
/// `write`/`sign`/`publish`/`put_record` method on this trait. The indexer
/// cannot author or mutate a claim — the capability boundary is encoded as the
/// ABSENCE of those methods.
#[async_trait]
pub trait IngestSourcePort: Send + Sync {
    /// Earned-Trust probe — see ADR-009 + `probe.rs`. The adapter impl exercises
    /// source reachability + enumeration shape, and the network-lies check (a
    /// fixture tampered/CID-mismatch record is rejected by the gate before the
    /// index). REQUIRED trait method per I-4.
    fn probe(&self) -> ProbeOutcome;

    /// Enumerate the bounded set of fetched-but-unverified records from `source`.
    /// Returns already-fetched [`RawRecord`]s ready for the pure
    /// `appview_domain::ingest_decision` gate — NO verification happens in the
    /// adapter.
    async fn enumerate(&self, source: &str) -> Result<Vec<RawRecord>, IngestError>;
}

// -----------------------------------------------------------------------------
// RepoListingPort — one repo's records, read live from a PDS (ADR-071 §4)
// -----------------------------------------------------------------------------

/// One record of a single repo as fetched: the repo DID and rkey come from
/// the record's own `at://` URI (the rkey is what the claim CID is checked
/// against, never the view's `cid`). The body is undecoded; no trust
/// decision has been made.
#[derive(Debug, Clone, PartialEq)]
pub struct RepoRecord {
    pub repo_did: String,
    pub rkey: String,
    pub value: serde_json::Value,
}

/// Every claim record of one repo, with the base URL it was actually read
/// from. The reader's root compares `fetched_from` with the PDS it freshly
/// resolved for the DID to compute the record origin; the adapter never does.
#[derive(Debug, Clone, PartialEq)]
pub struct RepoListing {
    pub fetched_from: String,
    pub records: Vec<RepoRecord>,
}

/// Read-only, bounded enumeration of ONE repo's `org.openlore.claim`
/// records (`listRecords` with `repo=<DID>`, cursor paging). Like
/// [`IngestSourcePort`] it has no write, sign or publish method.
#[async_trait]
pub trait RepoListingPort: Send + Sync {
    async fn list_repo_claims(
        &self,
        pds_base: &str,
        repo_did: &str,
    ) -> Result<RepoListing, IngestError>;
}
