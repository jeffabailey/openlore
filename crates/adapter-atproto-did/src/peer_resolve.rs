//! `peer_resolve` — slice-03 peer DID-document resolution scaffold.
//!
//! Step 01-03 (bootstrap stub): declares the shape of the peer-resolution
//! pipeline that backs `IdentityPort::resolve_peer`. The live body lands
//! per the PP-* (peer-pull) scenarios in Phase 04. Until then every entry
//! point is `todo!()` so the crate compiles and the trait-method gap that
//! `cargo build` flags at this step is closed without smuggling in
//! behavior no test yet requires.
//!
//! ## Why this lives in its own module (Extension Justification)
//!
//! WHY-NEW-FILE: crates/adapter-atproto-did/src/peer_resolve.rs
//!   CLOSEST-EXISTING: crates/adapter-atproto-did/src/probe.rs
//!   EXTENSION-COST: `probe.rs` holds pure probe ARMS (consume an I/O
//!     outcome, emit a structured refusal); folding the resolve pipeline
//!     into it would mix the probe's pure-arm contract with live PLC /
//!     did:web transport orchestration — two different lifecycles.
//!   PARALLEL-RATIONALE: peer resolution owns network transport
//!     (PLC HTTP GET for `did:plc:`, `.well-known/did.json` for
//!     `did:web:`) plus DID-document parsing; that is a distinct
//!     dependency surface (HTTP client + DID-doc parser) from probe.rs's
//!     pure-arm signature, and the design's §6.3 probe table treats
//!     `resolve_peer` as the thing the probe DRIVES, not part of it.
//!
//! ## Architectural posture (ADR-009 effect shell; WD-29)
//!
//! `resolve_peer` reuses the SAME atrium/PLC client the slice-01 own-DID
//! resolution path uses — no new dependency (WD-29). The resolution is
//! pure modulo the network read: fetch the DID document, parse it into
//! the `PeerInfo` shape, surface transport / parse failures as
//! `IdentityError::PeerResolutionFailed { did, detail }`. Per ADR-016 the
//! result is NOT cached on the adapter; every `peer pull` re-resolves to
//! pick up the peer's CURRENT PDS endpoint.

use claim_domain::Did;
use ports::{IdentityError, PeerInfo};

/// Resolve a peer's DID document into a `PeerInfo` (handle, current PDS
/// endpoint, verification methods).
///
/// SCAFFOLD: true (slice-03)
///
/// Dispatches on the DID method:
/// - `did:plc:…`  → PLC directory HTTP GET (reuses the slice-01 PLC
///   client per WD-29).
/// - `did:web:…`  → `https://<host>/.well-known/did.json` HTTP GET.
///
/// On any transport / parse failure returns
/// `IdentityError::PeerResolutionFailed { did, detail }` carrying the
/// underlying error verbatim for diagnostics (never panics, never returns
/// a silently-empty `PeerInfo`). Live body lands per the PP-* scenarios
/// in Phase 04.
pub(crate) fn resolve_peer_did(_peer_did: &Did) -> Result<PeerInfo, IdentityError> {
    // SCAFFOLD: true (slice-03)
    todo!("resolve_peer_did: PLC / did:web resolution lands per PP-* scenarios (Phase 04)")
}
