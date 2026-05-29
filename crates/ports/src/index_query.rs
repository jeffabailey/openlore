//! `index_query` — the CLI-side index-query port (ADR-027) + its railway error
//! + the raw transport result the CLI re-composes into a per-author view.
//!
//! `IndexQueryPort` is ASYNC (network I/O over HTTP/XRPC to the indexer) and
//! READ-ONLY by construction: there is NO sign / write method (the CLI's
//! signing identity is a separate port). The headline contract is that
//! `IndexQueryError::Unreachable` is a SOFT, NON-FATAL outcome — an unreachable
//! indexer MUST NOT block local CLI startup or any local-first verb
//! (KPI-AV-5 / KPI-5 / WD-116). Graceful degradation is the design.
//!
//! ## Anti-merging across the transport (WD-120 / I-AV-2)
//!
//! Every row of [`NetworkSearchResultRaw`] ([`NetworkResultRowRaw`]) carries
//! `author_did: Did` as a NON-`Option` field — dropping attribution over the
//! wire is a COMPILE error, not a runtime check. There is NO merged / consensus
//! object in the transport shape; grouping by author is the CLI renderer's job
//! (it re-composes via the pure `appview-domain` core). The wire carries flat
//! attributed rows. See data-models.md §"The XRPC query DTOs".
//
// SCAFFOLD: true  (trait surface only; the adapter impl lands in step 01-03/04)

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use claim_domain::{Cid, ClaimReference, Did, KeyId};

use crate::{ProbeOutcome, SearchDimension};

// -----------------------------------------------------------------------------
// NetworkSearchResultRaw — the raw transport result (CLI ← indexer)
// -----------------------------------------------------------------------------

/// One raw result row as carried over the wire (CLI ← indexer; ADR-027).
///
/// `author_did` is NON-`Option` and LOAD-BEARING — the anti-merging-across-the-
/// transport contract (I-AV-2). `verified_against` is never empty (drives the
/// `[verified]` marker; every result was verified at ingest, WD-104). This is
/// the flat attributed shape; the CLI groups it by author in the pure core
/// (there is NO per-author grouping nor merged row over the wire).
///
/// `PartialEq` (not `Eq`) because of the `f64` confidence.
#[derive(Debug, Clone, PartialEq)]
pub struct NetworkResultRowRaw {
    /// NON-`Option`; LOAD-BEARING (anti-merging across the transport, I-AV-2).
    pub author_did: Did,
    pub cid: Cid,
    pub subject: String,
    pub predicate: String,
    pub object: String,
    pub confidence: f64,
    pub composed_at: DateTime<Utc>,
    /// Drives the `[verified]` marker (never empty, WD-104).
    pub verified_against: KeyId,
    pub evidence: Vec<String>,
    pub references: Vec<ClaimReference>,
}

/// The raw transport result of one `IndexQueryPort::search` call.
///
/// `results` is a FLAT list of attributed rows (NO per-author grouping, NO
/// merged/consensus object — I-AV-2); the CLI re-composes the per-author view
/// via the pure `appview-domain` core. `distinct_author_count` is reported by
/// the indexer (itself a COUNT over attributed rows; never a merge).
/// `suggestion` carries the near-match for an empty dimension result
/// (US-AV-002 Ex 4); the CLI exits 0 on an empty result.
///
/// `PartialEq` (not `Eq`) because rows carry an `f64`.
#[derive(Debug, Clone, PartialEq)]
pub struct NetworkSearchResultRaw {
    /// FLAT attributed rows; NO merged-author row exists over the wire.
    pub results: Vec<NetworkResultRowRaw>,
    /// COUNT over attributed rows; never a merge.
    pub distinct_author_count: u32,
    pub total_claims: u32,
    /// Near-match suggestion for an empty result (US-AV-002 Ex 4).
    pub suggestion: Option<String>,
}

// -----------------------------------------------------------------------------
// IndexQueryError — Unreachable is NON-FATAL (KPI-AV-5 / KPI-5 / WD-116)
// -----------------------------------------------------------------------------

/// Why an index query failed. The query targets the self-hosted indexer at a
/// CONFIGURED URL over HTTP/XRPC (ADR-027).
///
/// `Unreachable` is documented + modeled as a SOFT, NON-FATAL outcome
/// (KPI-AV-5 / KPI-5 / WD-116): an unreachable indexer NEVER blocks the local
/// CLI — `search` degrades to a clear local-only message and the CLI starts
/// without a reachable indexer. The CLI-side adapter classifies a connection
/// failure as `Unreachable` (graceful degradation), NOT a startup refusal.
#[derive(Debug, thiserror::Error)]
pub enum IndexQueryError {
    /// SOFT, NON-FATAL: the configured indexer is not reachable. The CLI MUST
    /// degrade gracefully (local-only message, exit 0) — NEVER refuse to start
    /// nor treat this as a fatal error (KPI-AV-5 / KPI-5 / WD-116).
    #[error("indexer unreachable (non-fatal; search degrades to local-only): {message}")]
    Unreachable { message: String },
    /// The indexer responded but the payload did not match the XRPC contract
    /// (e.g. a result row dropping `author_did` — an I-AV-2 contract violation).
    #[error("indexer returned a malformed response: {message}")]
    BadResponse { message: String },
    /// A `--show <cid>` query for a CID not present in any result (distinct from
    /// an empty dimension search, which is `Ok` with a `suggestion`).
    #[error("queried record not found: {message}")]
    NotFound { message: String },
}

// -----------------------------------------------------------------------------
// IndexQueryPort — CLI → indexer transport (ASYNC; graceful-degrading)
// -----------------------------------------------------------------------------

/// The CLI-side index-query port: query the self-hosted indexer over HTTP/XRPC
/// (ADR-027). ASYNC (network I/O) so `#[async_trait]` is permitted exactly as
/// for `PdsPort`/`GithubPort` (ADR-004).
///
/// There is intentionally NO sign / write / publish method: the CLI's signing
/// identity is a separate port; the index query is read-only. An unreachable
/// indexer is the SOFT `IndexQueryError::Unreachable` outcome (KPI-AV-5).
#[async_trait]
pub trait IndexQueryPort: Send + Sync {
    /// Earned-Trust probe — see ADR-009 + `probe.rs`. SOFT at CLI startup: an
    /// unreachable indexer is informational, NOT a startup refusal (KPI-5).
    /// REQUIRED trait method per I-4.
    fn probe(&self) -> ProbeOutcome;

    /// Query the indexer along `dim` for `value`. `cid = Some(..)` selects one
    /// result for `--show`; `cid = None` is a dimension search. Returns the raw
    /// FLAT attributed transport result (the CLI re-composes per-author), or a
    /// railway error — `Unreachable` being SOFT/non-fatal (KPI-AV-5 / WD-116).
    async fn search(
        &self,
        dim: SearchDimension,
        value: &str,
        cid: Option<&Cid>,
    ) -> Result<NetworkSearchResultRaw, IndexQueryError>;
}
