//! Slice-03 acceptance — `openlore peer add` + `openlore peer remove` verbs.
//!
//! Drives the real `openlore` CLI as a subprocess via `assert_cmd` per
//! Mandate 1 (Hexagonal Boundary) + slice-01 DD-5. PDS + Identity +
//! Peer-PDS are faked; everything else is real (real DuckDB, real
//! filesystem, real clap). Pattern inherited verbatim from
//! `walking_skeleton.rs` — the shared `support/mod.rs` + the new
//! `openlore_test_support::FakePeerPds` provide the test seam.
//!
//! Covers:
//! - US-FED-001: subscribe to a peer's claim stream (add + idempotent
//!   re-add + self-DID rejection + unresolvable-DID rejection)
//! - US-FED-005: remove a peer subscription with optional purge
//!   (soft-remove + hard-purge + `--no-tty` refusal per WD-36)
//!
//! Per Mandate 7 (RED-ready scaffolding) + slice-03 DD-FED-5
//! (inherits slice-01 DD-2 fail-for-right-reason gate deferred until
//! slice-03 production code lands): every test body panics via
//! `todo!()`. DELIVER's first slice-03 step bootstraps the new types
//! (`PeerStoragePort`, `IdentityPort::resolve_peer`, `VerbPeerAdd`,
//! `VerbPeerRemove`) so the panic-at-`todo!()` classifies as RED, not
//! BROKEN (import error). DELIVER then unskips one at a time per the
//! standard outside-in TDD loop.
//
// SCAFFOLD: true

mod support;

#[allow(unused_imports)]
use support::*;

// =============================================================================
// US-FED-001 — `openlore peer add <did>`
// =============================================================================

/// PS-1: `openlore peer add did:plc:rachel-test` resolves the peer DID
/// via the identity adapter, probes that the peer's PDS exposes
/// `org.openlore.claim`, persists a subscription row in
/// `peer_subscriptions`, and prints the next-step hint + the
/// unsubscribe hint. (US-FED-001 AC 1-2-6; UAT scenario #1.)
///
/// @us-fed-001 @real-io @driving_port @j-003
#[test]
fn peer_subscribe_add_resolves_did_and_persists_subscription() {
    todo!("DELIVER (slice-03): wire VerbPeerAdd → PeerStoragePort.add_subscription + IdentityPort.resolve_peer + the next-step + unsubscribe hint output lines per ADR-013")
}

/// PS-2: Re-running `openlore peer add` against an already-subscribed
/// peer prints "already subscribed since <ts>" with the original
/// subscription timestamp, does NOT duplicate the row, and exits 0.
/// (US-FED-001 AC 3; UAT scenario #2; Example 2.)
///
/// @us-fed-001 @real-io @driving_port @j-003 @edge
#[test]
fn peer_subscribe_add_is_idempotent_on_re_subscribe() {
    todo!("DELIVER (slice-03): assert AddSubscriptionOutcome::AlreadyExisted dispatch path emits the original subscribed_at timestamp; assert exactly one peer_subscriptions row remains after two add invocations")
}

/// PS-3: `openlore peer add did:plc:not-a-real-did` exits non-zero with
/// stderr naming the DID and the resolution failure cause; no
/// peer_subscriptions row is written. (US-FED-001 AC 4 + Example 3.)
///
/// @us-fed-001 @real-io @driving_port @j-003 @error
#[test]
fn peer_subscribe_add_rejects_unresolvable_did_and_writes_no_subscription() {
    todo!("DELIVER (slice-03): wire IdentityPort.resolve_peer failure path → VerbPeerAdd exit-1 + stderr containing the DID + resolution-failure message per ADR-013 exit-code table; assert zero peer_subscriptions rows")
}

/// PS-4: `openlore peer add <self_did>` (where <self_did> is the local
/// user's own DID from identity.toml) exits non-zero with the error
/// message "you are already your own author; cannot subscribe to
/// yourself"; no peer_subscriptions row is written. (US-FED-001 AC 5 +
/// UAT scenario #4.)
///
/// @us-fed-001 @real-io @driving_port @j-003 @error
#[test]
fn peer_subscribe_add_rejects_self_did_subscription() {
    todo!("DELIVER (slice-03): wire self-DID short-circuit in VerbPeerAdd BEFORE PeerStoragePort.add_subscription is called; assert stderr literal 'cannot subscribe to yourself' + zero peer_subscriptions rows")
}

// =============================================================================
// US-FED-005 — `openlore peer remove <did> [--purge]`
// =============================================================================

/// PS-5: `openlore peer remove <did>` (no --purge) sets
/// `peer_subscriptions.removed_at` for that peer but leaves the
/// `peer_claims` row count unchanged; subsequent
/// `graph query --federated` annotates those rows as "(unsubscribed
/// cache)". (US-FED-005 AC 1-2; UAT scenario #1; WD-25 soft-remove
/// retains cache.)
///
/// @us-fed-005 @real-io @driving_port @j-003 @j-003c @happy
#[test]
fn peer_subscribe_remove_soft_keeps_cached_peer_claims() {
    todo!("DELIVER (slice-03): wire VerbPeerRemove default branch → PeerStoragePort.soft_remove; assert peer_subscriptions.removed_at IS NOT NULL AND peer_claims row count unchanged; assert subsequent graph query --federated annotates rows '(unsubscribed cache)'")
}

/// PS-6: `openlore peer remove <did> --purge` shows the cached-record
/// count, prompts `Proceed? [y/N]`, and on confirmation DELETES the
/// subscription AND ALL of that peer's cached claims from
/// `peer_claims`, but PRESERVES user-authored counter-claims in
/// `author_claims` referencing those (now-deleted) CIDs (WD-25
/// invariant). (US-FED-005 AC 3 + 5; UAT scenarios #2 + #5; integration
/// gate `peer_remove_purge_separation`.)
///
/// @us-fed-005 @real-io @driving_port @j-003 @j-003c @happy
#[test]
fn peer_subscribe_remove_purge_with_confirmation_deletes_peer_claims_and_preserves_user_counters() {
    todo!("DELIVER (slice-03): wire VerbPeerRemove --purge branch → TtyIO prompt → PeerStoragePort.hard_purge; assert (a) peer_subscriptions row gone, (b) peer_claims rows for that peer gone, (c) peer_claims/<did>/ directory removed, (d) author_claims (including counter-claims) untouched. Drives integration gate 4 (peer_remove_purge_separation).")
}

/// PS-7: `openlore peer remove <did> --purge` answered "n" to the
/// confirmation prompt leaves BOTH the subscription AND the cached
/// peer claims unchanged; CLI prints "Cancelled. Subscription and
/// cached peer claims unchanged." and exits 0. (US-FED-005 AC 4; UAT
/// scenario #3.)
///
/// @us-fed-005 @real-io @driving_port @j-003 @j-003c @edge
#[test]
fn peer_subscribe_remove_purge_declined_leaves_state_unchanged() {
    todo!("DELIVER (slice-03): feed 'n\\n' to the --purge prompt via run_openlore_with_stdin; assert peer_subscriptions row STILL present AND peer_claims rows unchanged AND stdout literal 'Cancelled. Subscription and cached peer claims unchanged.'")
}

/// PS-8: `openlore peer remove --purge --no-tty <did>` REFUSES to run
/// the destructive branch — exits non-zero with a directing error
/// message naming the missing TTY and pointing at slice-04's future
/// `--yes` flag. NO purge happens. (US-FED-005 + WD-21 + WD-36 lock;
/// ADR-013 exit-code table for `peer remove`.)
///
/// @us-fed-005 @real-io @driving_port @j-003c @error
#[test]
fn peer_subscribe_remove_purge_refuses_no_tty_mode() {
    todo!("DELIVER (slice-03): wire VerbPeerRemove --purge + --no-tty detection BEFORE TtyIO.confirm is called; assert exit-nonzero + stderr containing the directing error per WD-36; assert peer_subscriptions row STILL present AND peer_claims rows unchanged (no silent purge)")
}
