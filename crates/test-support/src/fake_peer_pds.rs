//! `FakePeerPds` — deterministic read-only test double for a PEER's ATProto PDS.
//!
//! Distinct from [`crate::FakePds`] (the user's-own-PDS double): peer PDSes
//! are HONESTLY a different actor. Peer pulls are UNAUTHENTICATED reads —
//! the user's CLI cannot publish to a peer's PDS and the peer's PDS does
//! NOT see the user's identity. This double therefore implements ONLY the
//! read paths slice-03 consumes per ADR-013 + DESIGN §6.2:
//!
//! - `com.atproto.repo.listRecords` (with optional cursor)
//! - `com.atproto.repo.getRecord`
//! - `com.atproto.identity.resolveDid` (resolves the peer's DID document so
//!   `IdentityPort::resolve_peer` can return a `PeerInfo`)
//!
//! Slice-03 explicitly REFUSES write paths against peer PDSes — there is
//! no `createRecord` handler. If the production code accidentally tries to
//! write to a peer endpoint the request returns 405 / surfaces a routing
//! refusal that DELIVER's wiring tests catch immediately.
//!
//! Functional-paradigm note (ADR-007): like `FakePds`, the fake owns a
//! small `Arc<Mutex<...>>` for record storage because port methods take
//! `&self`. The state is preconfigured by the test author at construction
//! (via `with_records`, `with_tampered_signature`, `with_cross_attribution`)
//! and is read-only thereafter from the system-under-test's perspective.
//!
//! ## Adversarial fixtures (KPI-FED-6 + WD-40 + WD-41)
//!
//! Three preconfigured adversarial postures are exposed as constructors so
//! DELIVER does not have to re-invent them per scenario:
//!
//! - `with_tampered_signature(peer_did, fixture)`: the peer's PDS returns
//!   N records, K of which carry a signature byte that has been flipped
//!   AFTER the peer's nominal sign step. Pulling MUST verify, MUST reject
//!   the K tampered records, MUST store the (N-K) honest records, MUST
//!   exit non-zero (WD-24). Drives `Sad_pull_signature_invalid_rejected`.
//! - `with_cid_mismatch(peer_did, fixture)`: the peer's PDS publishes a
//!   record whose rkey does NOT match the locally-recomputed CID for its
//!   canonical CBOR. Drives `Sad_pull_cid_mismatch_rejected`.
//! - `with_self_attribution(peer_did, victim_did, fixture)`: the peer
//!   publishes a record whose `author` field is `victim_did` (the local
//!   user). Per WD-40 this MUST be rejected with `SelfAttribution`.
//! - `with_cross_attribution(peer_did, claimed_author_did, fixture)`: the
//!   peer publishes a record whose `author` field is a third-party DID
//!   that is NOT the subscribed peer's DID. Per WD-41 this MUST be
//!   rejected with `CrossAttribution`.
//!
//! ## RED-baseline scaffold (DISTILL — slice-03 first scenario)
//!
//! Step 06-01 (DELIVER's first slice-03 scaffold) materializes the types
//! and the HTTP server in earnest; until then every body panics via
//! `panic!("Not yet implemented -- RED scaffold")`. The shape exists so
//! the slice-03 acceptance skeletons can name the right types at
//! compile-time once the workspace re-builds.
//
// SCAFFOLD: true

#![allow(dead_code)]
#![allow(unused_variables)]

/// A peer's published claim record as the peer PDS would return it.
///
/// Slice-03 reads peer records as raw JSON values (per
/// `PdsPort::list_peer_records` returning `PeerRecordPage` of
/// `SignedRecord` per component-boundaries §`crates/ports`). This
/// fixture-shape mirrors that — `body` is the canonical JSON the peer
/// published; `rkey` is the published key (which the production code
/// MUST verify against `compute_cid(body)` per WD-24).
#[derive(Debug, Clone)]
pub struct FakePeerRecord {
    pub collection: String, // always "org.openlore.claim" for slice-03
    pub rkey: String,       // peer-published; may or may not match recomputed CID
    pub body: serde_json::Value,
}

/// Read-only test double for a peer's ATProto PDS.
///
/// Constructed via `for_peer` (well-behaved) or the adversarial
/// constructors (`with_tampered_signature`, `with_cid_mismatch`,
/// `with_self_attribution`, `with_cross_attribution`). Once constructed,
/// the record set is fixed for the lifetime of the scenario.
///
/// `serve_http` spins up an in-process HTTP server bound to a random
/// `127.0.0.1` port that responds to the peer-read XRPC subset. The
/// returned `FakePeerPdsHttpHandle` aborts the server when dropped —
/// RAII per-scenario isolation, same shape as `FakePds`.
#[derive(Debug)]
pub struct FakePeerPds {
    _scaffold: (),
}

impl FakePeerPds {
    /// Construct a well-behaved peer PDS hosting the given records under
    /// `peer_did`. Every record's signature verifies against `peer_did`'s
    /// fixture key AND every record's rkey equals its recomputed CID.
    /// This is the baseline happy-path fixture used by US-FED-002
    /// Example 1 (Maria pulls Rachel's 5 claims, all verified, all
    /// stored).
    pub fn for_peer(peer_did: &str, records: Vec<FakePeerRecord>) -> Self {
        let _ = (peer_did, records);
        panic!("Not yet implemented -- RED scaffold")
    }

    /// Construct an adversarial peer PDS where exactly ONE record's
    /// signature has been tampered with after the peer's nominal sign
    /// step. The other records verify cleanly. Drives KPI-FED-6 and
    /// US-FED-002 Example 2 (rejected 1, stored 4).
    pub fn with_tampered_signature(peer_did: &str, honest: Vec<FakePeerRecord>) -> Self {
        let _ = (peer_did, honest);
        panic!("Not yet implemented -- RED scaffold")
    }

    /// Construct an adversarial peer PDS where exactly ONE record's rkey
    /// does NOT match its recomputed CID (canonicalization disagreement).
    /// Drives US-FED-002 UAT scenario "Peer claim with CID mismatch is
    /// rejected at ingest" + integration gate `peer_cid_round_trip`.
    pub fn with_cid_mismatch(peer_did: &str, honest: Vec<FakePeerRecord>) -> Self {
        let _ = (peer_did, honest);
        panic!("Not yet implemented -- RED scaffold")
    }

    /// Construct an adversarial peer PDS where exactly ONE record's
    /// `author` field is the LOCAL USER's DID (`victim_did`). Per WD-40
    /// this MUST be rejected at write time with
    /// `PeerStorageError::SelfAttribution`, even if the signature were
    /// valid against the victim's key (which would indicate key
    /// compromise — orthogonal failure mode).
    pub fn with_self_attribution(
        peer_did: &str,
        victim_did: &str,
        honest: Vec<FakePeerRecord>,
    ) -> Self {
        let _ = (peer_did, victim_did, honest);
        panic!("Not yet implemented -- RED scaffold")
    }

    /// Construct an adversarial peer PDS where exactly ONE record's
    /// `author` field is `claimed_author_did` (a third party that is NOT
    /// the subscribed `peer_did`). Per WD-41 this MUST be rejected with
    /// `PeerStorageError::CrossAttribution` — slice-03's trust model is
    /// "subscribing to a peer means accepting THEIR claims; cross-
    /// attributed records are out of scope for slice-03."
    pub fn with_cross_attribution(
        peer_did: &str,
        claimed_author_did: &str,
        honest: Vec<FakePeerRecord>,
    ) -> Self {
        let _ = (peer_did, claimed_author_did, honest);
        panic!("Not yet implemented -- RED scaffold")
    }

    /// Toggle "unreachable" mode on. Subsequent HTTP calls drop the
    /// connection without sending bytes; reqwest classifies this as a
    /// network error. Used by US-FED-002 Example 3 (PDS unreachable;
    /// skip this peer, proceed with others).
    pub fn simulate_unreachable(&self) {
        panic!("Not yet implemented -- RED scaffold")
    }

    /// Inverse of `simulate_unreachable`.
    pub fn restore(&self) {
        panic!("Not yet implemented -- RED scaffold")
    }

    /// Read access for assertions: all records the fake would return on a
    /// list_peer_records call. Lets tests cross-check the production
    /// code stored only the verified subset.
    pub fn records(&self) -> Vec<FakePeerRecord> {
        panic!("Not yet implemented -- RED scaffold")
    }

    /// Spin up an in-process HTTP XRPC server bound to `127.0.0.1` on an
    /// OS-assigned port. Endpoints served (read-only):
    ///
    /// - `GET /xrpc/com.atproto.repo.listRecords?repo=&collection=&cursor=`
    /// - `GET /xrpc/com.atproto.repo.getRecord?repo=&collection=&rkey=`
    /// - `GET /xrpc/com.atproto.identity.resolveDid?did=` (returns the
    ///   peer DID document so `IdentityPort::resolve_peer` can return a
    ///   valid `PeerInfo`).
    ///
    /// Returns the handle; dropping it aborts the server task.
    pub async fn serve_http(&self) -> FakePeerPdsHttpHandle {
        panic!("Not yet implemented -- RED scaffold")
    }
}

/// Owning handle to a running [`FakePeerPds::serve_http`] task. Same
/// shape as `FakePdsHttpHandle`; the server task is aborted on drop.
#[derive(Debug)]
pub struct FakePeerPdsHttpHandle {
    pub base_url: String,
    _task: (),
}
