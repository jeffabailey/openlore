//! `peer_read` — slice-03 peer-PDS read scaffold.
//!
//! Step 01-03 (bootstrap stub): declares the shape of the peer-read
//! pipeline backing `PdsPort::list_peer_records` + `PdsPort::get_peer_record`.
//! The live bodies land per the PP-* (peer-pull) scenarios in Phase 04.
//! Until then every entry point is `todo!()` so the crate compiles and
//! the trait-method gap that `cargo build` flags at this step is closed
//! without smuggling in behavior no test yet requires.
//!
//! ## Why this lives in its own module (Extension Justification)
//!
//! WHY-NEW-FILE: crates/adapter-atproto-pds/src/peer_read.rs
//!   CLOSEST-EXISTING: crates/adapter-atproto-pds/src/probe.rs
//!   EXTENSION-COST: `probe.rs` holds pure probe ARMS that consume the
//!     outcome of an XRPC step and emit structured refusals; folding the
//!     peer-read pipeline into it would couple the probe's pure-arm
//!     contract to the live `listRecords` / `getRecord` paging + CID
//!     round-trip orchestration.
//!   PARALLEL-RATIONALE: peer read owns the `com.atproto.repo.listRecords`
//!     cursor walk and per-record parse into `SignedRecord`; the design's
//!     §6.3 probe table treats `list_peer_records` as the thing the probe
//!     DRIVES (it re-computes CIDs against the listed records), so the
//!     read path and the probe arm have different call directions and must
//!     not share a module.
//!
//! ## Architectural posture (ADR-016; effect shell)
//!
//! Per ADR-016 the `peer_pds_endpoint` is an INPUT to every call (NOT
//! cached on the adapter): each pull re-resolves the peer's DID document
//! to get a fresh endpoint. Records come back as raw JSON parsed into the
//! `SignedRecord` ADT; signature verification + CID recomputation are NOT
//! this adapter's job — they happen in `claim_domain` (pure) called from
//! `VerbPeerPull` (cli). Network / parse failures surface as the
//! slice-03 `PdsError` variants (`Unreachable`, `PeerRecordNotFound`,
//! `PeerRecordSchemaInvalid`, `PeerRecordCidRoundTripFailed`); never a
//! panic, never a silently-empty page.

use ports::claim_domain::Did;
use ports::{PdsError, PeerRecordPage, SignedRecord};
use url::Url;

/// Page through a peer's `org.openlore.claim` records via
/// `com.atproto.repo.listRecords`.
///
/// SCAFFOLD: true (slice-03)
///
/// `cursor = None` requests the first page; the returned `next_cursor` is
/// the opaque ATProto cursor echoed back verbatim on the next call. The
/// `peer_pds_endpoint` is taken fresh per ADR-016 (never cached on the
/// adapter). Live body lands per the PP-* scenarios in Phase 04.
pub(crate) async fn list_peer_records_xrpc(
    _peer_did: &Did,
    _peer_pds_endpoint: &Url,
    _cursor: Option<String>,
) -> Result<PeerRecordPage, PdsError> {
    // SCAFFOLD: true (slice-03)
    todo!("list_peer_records_xrpc: listRecords cursor walk lands per PP-* scenarios (Phase 04)")
}

/// Fetch one specific peer record by `rkey` via
/// `com.atproto.repo.getRecord`.
///
/// SCAFFOLD: true (slice-03)
///
/// Used by re-pull paths that already hold the `rkey` from a prior list
/// and want to refresh just that record. A missing record surfaces as
/// `PdsError::PeerRecordNotFound`. Live body lands per the PP-* scenarios
/// in Phase 04.
pub(crate) async fn get_peer_record_xrpc(
    _peer_did: &Did,
    _peer_pds_endpoint: &Url,
    _rkey: &str,
) -> Result<SignedRecord, PdsError> {
    // SCAFFOLD: true (slice-03)
    todo!("get_peer_record_xrpc: single-record getRecord lands per PP-* scenarios (Phase 04)")
}
