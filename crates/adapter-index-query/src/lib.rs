//! `adapter-index-query` — the CLI-side index-query adapter (over HTTP/XRPC).
//!
//! EFFECT shell for the `IndexQueryPort` trait (`crates/ports`). Queries the
//! self-hosted indexer at a CONFIGURED URL over HTTP/XRPC
//! (`org.openlore.appview.searchClaims`, ADR-027) and returns the raw FLAT
//! attributed transport result ([`NetworkSearchResultRaw`]); the CLI re-composes
//! the per-author view via the pure `appview-domain` core.
//!
//! ## Unreachable is SOFT and NON-FATAL (KPI-AV-5 / KPI-5 / WD-116)
//!
//! The headline contract: a connection failure maps to
//! [`IndexQueryError::Unreachable`] — a SOFT, NON-FATAL outcome. An unreachable
//! indexer NEVER blocks local CLI startup nor any local-first verb; `search`
//! degrades to a clear local-only message and exits 0. This adapter classifies
//! a transport connection failure as `Unreachable`, NEVER a panic or a startup
//! refusal. READ-ONLY by construction (no sign/write method).
//!
//! ## Architecture (nw-fp-hexagonal-architecture)
//!
//! The pure core never imports this crate; the CLI composition root wires a
//! [`HttpIndexQueryAdapter`] behind the `IndexQueryPort` interface.
//!
//! Bootstrap SCAFFOLD (step 01-03): the port impl exists so the workspace
//! compiles and the wiring seam is present, but every body is `todo!()`. The
//! HTTP query + the graceful-degradation `Unreachable` mapping are driven by the
//! Phase 03/04 acceptance scenarios (AV-* / the indexer-down non-fatal scenario).
//
// SCAFFOLD: true  (adapter skeleton; HTTP query + degradation land in Phase 03/04)

#![allow(dead_code)] // scaffold; real wiring lands in subsequent DELIVER steps
#![forbid(unsafe_code)]

use async_trait::async_trait;
use claim_domain::Cid;
use ports::{
    IndexQueryError, IndexQueryPort, NetworkSearchResultRaw, ProbeOutcome, SearchDimension,
};

/// CLI-side `IndexQueryPort` adapter over HTTP/XRPC to the configured indexer
/// URL (ADR-027).
///
/// Bootstrap SCAFFOLD — the `reqwest` client + the configured indexer URL land
/// with the real wiring in Phase 03/04. A connection failure will map to the
/// SOFT `IndexQueryError::Unreachable` (WD-116), never a panic.
pub struct HttpIndexQueryAdapter {
    // SCAFFOLD: true — the HTTPS client + the configured indexer base URL land
    // in Phase 03/04. No signing/identity field exists (read-only by construction).
    _scaffold: (),
}

impl HttpIndexQueryAdapter {
    /// Construct the CLI-side query adapter for a configured indexer URL.
    /// Bootstrap SCAFFOLD: the real constructor (reqwest client + URL) lands in
    /// Phase 03/04.
    pub fn new() -> Self {
        // SCAFFOLD: true
        todo!("HttpIndexQueryAdapter::new — wired in Phase 03/04 (ADR-027)")
    }
}

#[async_trait]
impl IndexQueryPort for HttpIndexQueryAdapter {
    fn probe(&self) -> ProbeOutcome {
        // SCAFFOLD: true — the SOFT-at-startup probe (an unreachable indexer is
        // informational, NOT a refusal — KPI-5) lands in Phase 03/04.
        todo!("HttpIndexQueryAdapter::probe — SOFT index-query probe (Phase 03/04)")
    }

    async fn search(
        &self,
        _dim: SearchDimension,
        _value: &str,
        _cid: Option<&Cid>,
    ) -> Result<NetworkSearchResultRaw, IndexQueryError> {
        // SCAFFOLD: true — the HTTP/XRPC searchClaims query lands in Phase 03/04.
        // A connection failure MUST map to the SOFT IndexQueryError::Unreachable
        // (WD-116), never a panic/fatal.
        todo!("HttpIndexQueryAdapter::search — XRPC searchClaims query, Unreachable=soft (Phase 03/04)")
    }
}
