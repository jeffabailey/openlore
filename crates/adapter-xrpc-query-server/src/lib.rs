//! `adapter-xrpc-query-server` — the indexer's HTTP/XRPC query surface.
//!
//! EFFECT shell that will serve `org.openlore.appview.searchClaims` (ADR-027):
//! it binds an HTTP listener, reads the index via [`IndexStorePort`], calls the
//! pure `appview_domain::compose_results` to group the attributed rows
//! per-author, and returns a response in which EVERY result carries
//! `author_did` (the anti-merging-across-the-transport contract, I-AV-2). There
//! is NO `consensus` / `merged` object in the response shape.
//!
//! ## HTTP framework: NONE (bootstrap), `axum` BANNED
//!
//! This bootstrap skeleton is FRAMEWORK-AGNOSTIC: `axum` is banned
//! (`deny.toml`; the narrowing is a 01-04 task) and the concrete HTTP-serving
//! framework is DEFERRED to step 04-06. Signatures use abstract std types
//! (`std::net::SocketAddr`) so the crate compiles with zero HTTP dependency.
//! When real serving lands, the preferred dependency is `hyper` (transitive via
//! reqwest, NOT banned) — NOT axum.
//!
//! ## Architecture (nw-fp-hexagonal-architecture)
//!
//! The pure core (claim-domain, appview-domain) never imports this crate; the
//! indexer composition root wires a [`XrpcQueryServer`] over an `IndexStorePort`
//! implementation. The server is the impure shell; the per-author grouping it
//! returns is computed by the PURE `compose_results` it calls.
//!
//! Bootstrap SCAFFOLD (step 01-03): `bind` / `serve` / `probe` are `todo!()`;
//! real HTTP serving is driven by the Phase 04 wiring scenarios (step 04-06).
//
// SCAFFOLD: true  (server skeleton; real HTTP serving lands in step 04-06)

#![allow(dead_code)] // scaffold; real wiring lands in step 04-06
#![forbid(unsafe_code)]

use std::net::SocketAddr;

use ports::ProbeOutcome;

/// Why the XRPC query server failed to bind / serve. Bootstrap SCAFFOLD — the
/// real variants land with the serving wiring in step 04-06.
#[derive(Debug, thiserror::Error)]
pub enum QueryServerError {
    /// The configured listen address could not be bound (port in use, etc.).
    #[error("query server bind failed: {message}")]
    BindFailed { message: String },
    /// The server loop terminated abnormally while serving.
    #[error("query server serve loop failed: {message}")]
    ServeFailed { message: String },
}

/// The indexer's HTTP/XRPC query server (ADR-027). Framework-agnostic bootstrap
/// skeleton — it will hold the `IndexStorePort` read side it serves once the
/// concrete HTTP framework lands (step 04-06).
pub struct XrpcQueryServer {
    // SCAFFOLD: true — the bound `IndexStorePort` read side + the concrete HTTP
    // listener/runtime land in step 04-06 (preferring `hyper`, NOT axum).
    _scaffold: (),
}

impl XrpcQueryServer {
    /// Earned-Trust probe — see ADR-009. Bootstrap SCAFFOLD: the readiness probe
    /// (bind preflight + index reachability) lands in step 04-06.
    pub fn probe(&self) -> ProbeOutcome {
        // SCAFFOLD: true
        todo!("XrpcQueryServer::probe — Earned-Trust serving probe (step 04-06)")
    }

    /// Bind the HTTP listener at `addr`. Bootstrap SCAFFOLD: the real bind +
    /// framework wiring lands in step 04-06. The abstract `SocketAddr` keeps the
    /// signature framework-agnostic (no axum/hyper dependency at bootstrap).
    pub fn bind(_addr: SocketAddr) -> Result<Self, QueryServerError> {
        // SCAFFOLD: true
        todo!("XrpcQueryServer::bind — HTTP listener bind (step 04-06; hyper, NOT axum)")
    }

    /// Serve `org.openlore.appview.searchClaims` until shutdown. Bootstrap
    /// SCAFFOLD: the handler (read IndexStorePort → compose_results → respond
    /// with per-result `author_did`) lands in step 04-06.
    pub fn serve(&self) -> Result<(), QueryServerError> {
        // SCAFFOLD: true
        todo!("XrpcQueryServer::serve — searchClaims handler (step 04-06, ADR-027)")
    }
}
