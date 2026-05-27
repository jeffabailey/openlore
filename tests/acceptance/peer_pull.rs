//! Slice-03 acceptance — `openlore peer pull` verb.
//!
//! Drives the real `openlore` CLI as a subprocess via `assert_cmd`.
//! Reuses `support/mod.rs` for TestEnv + assertion helpers; adds the
//! peer-PDS double via `openlore_test_support::FakePeerPds`. The
//! adversarial-peer fixtures (`fixture_adversarial_peer_*`) drive the
//! KPI-FED-6 + WD-40 + WD-41 rejection scenarios.
//!
//! Covers:
//! - US-FED-002: pull peer claims with per-claim signature verification
//!   + per-claim CID recomputation (WD-24)
//! - WD-37 fault isolation (per-peer + per-record)
//! - WD-40 self-attribution rejection at write time
//! - WD-41 cross-attribution rejection at write time
//! - WD-18 / ADR-016 pull-on-demand only (no daemon, no auto-pull)
//!
//! Per Mandate 11 (sad-path coverage at layer 3 is example-based, NEVER
//! PBT-generated) every adversarial scenario is a named example with an
//! explicit fixture. No proptest at this layer; the property-shaped
//! invariants (CID byte-stability across N pulls) live in
//! `lexicon_conformance.rs` at layer 2.
//
// SCAFFOLD: true

mod support;

#[allow(unused_imports)]
use support::*;

// =============================================================================
// US-FED-002 — happy path
// =============================================================================

/// PP-1: `openlore peer pull` against one subscribed peer (Rachel)
/// publishing N records fetches all N via XRPC, verifies each
/// signature against Rachel's DID-doc key, recomputes each CID locally
/// and byte-matches against the peer's published rkey, and stores all
/// N rows in `peer_claims` attributed to `did:plc:rachel-test`. The
/// pull summary reports "N stored, 0 rejected" and stdout contains
/// the content-frozen line "None merged with your own claims."
/// (US-FED-002 AC 1-2-3-5-8 + UAT scenario #1 + ADR-013 output
/// convention.)
///
/// @us-fed-002 @real-io @driving_port @j-003 @j-003a @happy
#[test]
fn peer_pull_fetches_verifies_and_stores_peer_claims_attributed_per_record() {
    todo!("DELIVER (slice-03): wire VerbPeerPull → PdsPort.list_peer_records → claim_domain::verify + compute_cid per record → PeerStoragePort.write_peer_claim. Use fixture_other_developer_three_claims via FakePeerPds::for_peer('did:plc:rachel-test', ...). Assert per-row author_did = did:plc:rachel-test (anti-merging: NEVER under any other DID) + author_claims row count UNCHANGED + stdout literal 'None merged with your own claims.'")
}

/// PP-2: Re-running `openlore peer pull` with no new records on the
/// peer's PDS skips already-stored claims by CID and reports
/// "0 new, N already in peer_claims, skipped" with exit 0.
/// (US-FED-002 AC 6 + UAT scenario #4 — pull is idempotent.)
///
/// @us-fed-002 @real-io @driving_port @j-003 @edge
#[test]
fn peer_pull_is_idempotent_skipping_already_stored_claims_by_cid() {
    todo!("DELIVER (slice-03): assert second-invocation peer_claims row count UNCHANGED + stdout contains 'already in peer_claims' + exit 0; assert WritePeerClaimOutcome.written == false on second pull per component-boundaries §PeerStoragePort.write_peer_claim")
}

// =============================================================================
// US-FED-002 — sad paths (Mandate 11: example-based, never PBT-generated)
// =============================================================================

/// PP-3 / Sad: A peer publishes 5 records, one of which has a tampered
/// signature (last byte flipped after the peer's nominal sign step).
/// `openlore peer pull` rejects ONLY that record, stores the other 4,
/// reports "4 stored, 1 rejected (signature invalid)", and exits
/// non-zero overall (WD-37 fault isolation + ADR-013 exit-code table).
/// Drives KPI-FED-6 (zero invalid signatures stored) + integration
/// gate `peer_tampered_signature_rejected`.
///
/// @us-fed-002 @real-io @driving_port @j-003a @error @kpi-fed-6
#[test]
fn peer_pull_rejects_tampered_signature_per_record_and_stores_honest_records() {
    todo!("DELIVER (slice-03): wire VerbPeerPull → claim_domain::verify failure path → reject this record only, continue with others. Use FakePeerPds::with_tampered_signature + fixture_adversarial_peer_tampered_signature. Assert: peer_claims row count = 4 (NOT 5), stderr/stdout contains 'signature invalid' + per-claim reject line, exit code != 0 per WD-37, no row attributed to ANY DID for the tampered record (anti-merging holds even at the reject path)")
}

/// PP-4 / Sad: A peer publishes a record whose published rkey does NOT
/// equal the locally-recomputed CID (canonicalization disagreement —
/// "possible adversarial input"). `openlore peer pull` rejects only
/// that record, stores the others, reports the rejection reason
/// verbatim, and exits non-zero. Drives integration gate
/// `peer_cid_round_trip`.
///
/// @us-fed-002 @real-io @driving_port @j-003a @error
#[test]
fn peer_pull_rejects_cid_mismatch_per_record_and_stores_honest_records() {
    todo!("DELIVER (slice-03): wire VerbPeerPull → claim_domain::compute_cid recompute → byte-match against peer rkey → reject on mismatch. Use FakePeerPds::with_cid_mismatch + fixture_adversarial_peer_cid_mismatch. Assert rejection-reason text 'CID mismatch (possible adversarial input)' per US-FED-002 UAT scenario #3 + exit nonzero + zero peer_claims row written for the mismatched CID")
}

/// PP-5 / Sad (WD-40): A peer publishes a record whose `author` field
/// is the LOCAL USER's DID (`did:plc:test-maria`). Even though the
/// signature might verify against the user's own key (which would
/// indicate key compromise — orthogonal failure), the pull-time write
/// MUST reject with `PeerStorageError::SelfAttribution`. Probe #4 of
/// `PeerStoragePort.probe` mirrors this gate.
///
/// @us-fed-002 @real-io @driving_port @j-003a @error @wd-40
#[test]
fn peer_pull_rejects_self_attribution_at_write_time() {
    todo!("DELIVER (slice-03): wire DuckDbPeerStorageAdapter::write_peer_claim → if author_did == identity.author_did() → reject with PeerStorageError::SelfAttribution. Use FakePeerPds::with_self_attribution + fixture_adversarial_peer_self_attribution. Assert: stderr contains 'self attribution' or equivalent message, peer_claims row count for the offending CID == 0, the OTHER honest records ARE stored (fault isolation per WD-37), exit nonzero")
}

/// PP-6 / Sad (WD-41 — RESOLVES `# DISTILL: confirm` anxiety scenario
/// 1.2): A subscribed peer (Rachel) publishes a record whose `author`
/// field references `did:plc:trusted-third-party-test` (a DID OTHER
/// than Rachel). Per WD-41 this MUST be rejected with
/// `PeerStorageError::CrossAttribution`; slice-03 does NOT silently
/// follow cross-attributed records. The user's trust model is
/// "subscribing to a peer means accepting THEIR claims."
///
/// @us-fed-002 @real-io @driving_port @j-003a @error @wd-41
#[test]
fn peer_pull_rejects_cross_attribution_to_third_party_did_at_write_time() {
    todo!("DELIVER (slice-03): wire DuckDbPeerStorageAdapter::write_peer_claim → if signed.author != subscribed_peer.did → reject with PeerStorageError::CrossAttribution. Use FakePeerPds::with_cross_attribution + fixture_adversarial_peer_cross_attribution. Assert: NO peer_claims row attributed to ANY DID for the cross-attributed record (anti-merging holds at reject path too), other honest records stored, exit nonzero")
}

/// PP-7 / Sad: One of three subscribed peers' PDSes is currently
/// unreachable. `openlore peer pull` reports
/// "peer did:plc:down-test: PDS unreachable (connection refused);
/// skipping", proceeds with the other two peers (storing their records
/// normally), and exits non-zero overall to flag the partial failure
/// (WD-37 fault isolation + ADR-013 exit-code table).
///
/// @us-fed-002 @real-io @driving_port @j-003 @error
#[test]
fn peer_pull_skips_unreachable_peer_and_proceeds_with_others() {
    todo!("DELIVER (slice-03): wire VerbPeerPull → iterate subscriptions sequentially per WD-37 → each peer's PdsError::Unreachable becomes a skip-with-message, not a process abort. Use three FakePeerPds doubles, one with simulate_unreachable(). Assert per-peer summary lines + total peer_claims row count = sum(non-skipped peers' records) + exit code != 0")
}

/// PP-8: `openlore peer pull` against ZERO subscribed peers exits 0
/// with a "no peers subscribed" stdout line and writes nothing. (cli
/// probe #4 of ADR-013 §Earned Trust; ADR-016 pull-on-demand-only.)
///
/// @us-fed-002 @real-io @driving_port @j-003 @edge
#[test]
fn peer_pull_with_zero_subscriptions_prints_no_peers_subscribed_and_exits_zero() {
    todo!("DELIVER (slice-03): wire VerbPeerPull early-return on empty list_active_subscriptions; assert stdout contains 'no peers subscribed' (case-insensitive) + exit 0 + zero peer_claims rows + zero filesystem writes under peer_claims/ directory tree")
}
