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
//! ## Flow (architecture-design §5.1 VerbPeerAdd)
//!
//! 1. Reject self-subscribe (`did == identity.author_did()`) BEFORE any
//!    network or storage call (PS-4).
//! 2. `resolve_peer(did)` → `PeerInfo` (handle + PDS endpoint). A
//!    resolution failure exits non-zero, persists nothing (PS-3).
//! 3. Persist via `add_subscription`. `Added` → fresh-subscribe output;
//!    `AlreadyExisted` → idempotent "already subscribed since <ts>" (PS-2).
//! 4. Render the resolve confirmation + the next-pull hint + the
//!    unsubscribe hint (journey step 1; ADR-013).
//!
//! Per WD-D14 the per-peer PDS *probe* (does the peer actually expose
//! `org.openlore.claim`?) is DEFERRED to first `peer pull`; `peer add`
//! confirms the collection the subscription targets without a speculative
//! list call (which would couple subscribe latency to the peer's record
//! count). The successful DID resolution is the reachability proof at
//! subscribe time.
//!
//! ## Pure-vs-effect split (ADR-009)
//!
//! Output rendering is a pure function of the resolved `PeerInfo` + the
//! `add_subscription` outcome (`render_added` / `render_already`). The
//! effects — resolve, persist, clock read — live in `run`.

use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use ports::{AddSubscriptionOutcome, PeerInfo, PeerSubscription};

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
pub fn run(wiring: &Wiring, args: &PeerAddArgs) -> Result<PeerAddOutcome> {
    let peer_did = claim_domain::Did(args.did.clone());

    // Step 1: self-subscribe short-circuit (PS-4) — before any I/O.
    if peer_did.0 == wiring.identity.author_did().0 {
        return Err(anyhow!(
            "you are already your own author; cannot subscribe to yourself"
        ));
    }

    // Step 2: resolve the peer DID document. Failure → non-zero exit,
    // nothing persisted (PS-3).
    let peer_info = wiring
        .identity
        .resolve_peer(&peer_did)
        .map_err(|err| anyhow!("could not resolve peer DID {}: {err}", peer_did.0))?;

    // Step 3: persist the subscription (idempotent).
    let now = wiring.clock.now_utc();
    let subscription = PeerSubscription {
        peer_did: peer_info.did.clone(),
        peer_handle: peer_info.handle.clone(),
        peer_pds_endpoint: peer_info.pds_endpoint.clone(),
        subscribed_at: now,
        removed_at: None,
    };
    let outcome = wiring
        .peer_storage
        .add_subscription(subscription)
        .map_err(|err| anyhow!("could not persist subscription for {}: {err}", peer_did.0))?;

    // Step 4: render.
    let stdout = match outcome {
        AddSubscriptionOutcome::Added { subscribed_at } => render_added(&peer_info, subscribed_at),
        AddSubscriptionOutcome::AlreadyExisted { since } => render_already(&peer_info, since),
    };

    Ok(PeerAddOutcome {
        exit_code: 0,
        stdout,
    })
}

/// Pure render for a fresh subscription (journey step 1 / ADR-013).
fn render_added(peer: &PeerInfo, subscribed_at: DateTime<Utc>) -> String {
    let mut out = String::new();
    out.push_str(&format!("Resolving DID {} ... ok\n", peer.did.0));
    out.push_str(&format!("  handle           : {}\n", peer.handle));
    out.push_str(&format!("  PDS              : {}\n", peer.pds_endpoint));
    out.push_str(&format!(
        "  claim collection : {}  (lexicon ok)\n",
        lexicon::CLAIM_NSID
    ));
    out.push('\n');
    out.push_str("Adding peer to subscription list ... ok\n");
    out.push_str(&format!(
        "  subscribed_at    : {}\n",
        subscribed_at.to_rfc3339()
    ));
    out.push_str("  next pull        : on-demand (`openlore peer pull`)\n");
    out.push_str("  local layer      : peer_claims (separate from your own claims)\n");
    out.push('\n');
    out.push_str(&format!(
        "Tip: peer claims will appear in `openlore graph query --federated <subject>`.\n     \
         To unsubscribe later: `openlore peer remove {}`.\n",
        peer.did.0
    ));
    out
}

/// Pure render for an idempotent re-subscribe (PS-2 / US-FED-001 AC #3).
/// Still emits the unsubscribe hint so the user always sees the exit path.
fn render_already(peer: &PeerInfo, since: DateTime<Utc>) -> String {
    let mut out = String::new();
    out.push_str(&format!("Resolving DID {} ... ok\n", peer.did.0));
    out.push_str(&format!("  handle           : {}\n", peer.handle));
    out.push_str(&format!("  PDS              : {}\n", peer.pds_endpoint));
    out.push_str(&format!(
        "  claim collection : {}  (lexicon ok)\n",
        lexicon::CLAIM_NSID
    ));
    out.push('\n');
    out.push_str(&format!(
        "already subscribed since {}\n",
        since.to_rfc3339()
    ));
    out.push_str(&format!(
        "To unsubscribe: `openlore peer remove {}`.\n",
        peer.did.0
    ));
    out
}
