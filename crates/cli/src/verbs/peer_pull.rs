//! `peer pull` — fetch + verify + cache claims from every subscribed
//! peer (slice-03; US-FED-002 / PP-*).
//!
//! For each active subscription: re-resolve the peer PDS endpoint, list
//! the peer's `org.openlore.claim` records, recompute each record's CID
//! locally and verify its signature against the peer's DID-doc key, and
//! cache the verified records (via `PeerStoragePort::write_peer_claim` +
//! the `peer_claims/<did>/<cid>.json` artifact tree). Fault-isolated: a
//! failed peer or a rejected record never aborts the other pulls; the
//! overall exit code is non-zero if ANY peer was skipped or ANY record
//! rejected.
//!
//! First-pull orientation (data-models.md §OrientationState): the FIRST
//! EVER successful `peer pull` emits the orientation message exactly once
//! (gated via `crate::orientation`), then records
//! `federation.first_pull_completed_at`.
//!
//! SCAFFOLD: true (slice-03)

use anyhow::Result;

use crate::wiring::Wiring;

/// Argument struct for the `peer pull` verb. It takes no arguments today
/// (it pulls ALL active subscriptions); the struct exists for uniformity
/// with the other verbs and as the seam for a future `--peer <did>`
/// targeted-pull flag.
#[derive(Debug, Clone, Default)]
pub struct PeerPullArgs {}

/// Outcome of one `peer pull` invocation — exit code + stdout chunk.
pub struct PeerPullOutcome {
    pub exit_code: i32,
    pub stdout: String,
}

/// Run the `peer pull` verb.
///
/// SCAFFOLD: true (slice-03) — `todo!()` stub at this bootstrap step.
/// The PP-* acceptance scenarios drive the real implementation
/// (list_peer_records → recompute CID + verify signature → write_peer_claim,
/// fault-isolated per peer/record, with first-pull orientation) in a
/// later slice-03 phase.
pub fn run(_wiring: &Wiring, _args: &PeerPullArgs) -> Result<PeerPullOutcome> {
    // SCAFFOLD: true (slice-03)
    todo!(
        "VerbPeerPull — for each active subscription: list_peer_records → \
         recompute CID + verify signature (claim_domain) → write_peer_claim; \
         fault-isolated; first-pull orientation via crate::orientation. \
         Driven by PP-* scenarios."
    )
}
