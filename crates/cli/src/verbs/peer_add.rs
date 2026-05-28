//! `peer add <did>` — subscribe to a peer's claim stream (slice-03;
//! US-FED-001).
//!
//! Resolves the peer DID document (via `IdentityPort::resolve_peer`),
//! confirms the peer to the user, and records a `peer_subscriptions` row
//! (via `PeerStoragePort::add_subscription`). Idempotent: re-running
//! against an already-subscribed DID reports "already subscribed since
//! <ts>" rather than erroring (PS-2). Rejects `peer add <self_did>` —
//! the user cannot subscribe to themselves (PS-4 / anti-merging).
//!
//! SCAFFOLD: true (slice-03)

use anyhow::Result;

use crate::wiring::Wiring;

/// Argument struct for the `peer add` verb (mirrors the clap subcommand).
#[derive(Debug, Clone)]
pub struct PeerAddArgs {
    /// The peer DID to subscribe to (e.g. `did:plc:rachel-test`).
    pub did: String,
}

/// Outcome of one `peer add` invocation — exit code + stdout chunk.
pub struct PeerAddOutcome {
    pub exit_code: i32,
    pub stdout: String,
}

/// Run the `peer add` verb.
///
/// SCAFFOLD: true (slice-03) — `todo!()` stub at this bootstrap step.
/// The PS-* acceptance scenarios drive the real implementation
/// (resolve_peer → confirm → add_subscription, with self-subscribe
/// rejection + idempotent re-subscribe) in a later slice-03 phase.
pub fn run(_wiring: &Wiring, _args: &PeerAddArgs) -> Result<PeerAddOutcome> {
    // SCAFFOLD: true (slice-03)
    todo!(
        "VerbPeerAdd — wire IdentityPort::resolve_peer → confirm peer → \
         PeerStoragePort::add_subscription (reject self-subscribe PS-4; \
         idempotent re-subscribe PS-2). Driven by PS-* scenarios."
    )
}
