//! Canonical peer-claim fixtures for slice-03 acceptance + integration.
//!
//! Symmetric with `fixtures.rs` (slice-01 author-claim fixtures): each
//! fixture is a free function returning a fresh, immutable value. No
//! shared mutable state. Tests compose by passing values through.
//!
//! Naming convention (matches DISCUSS/journey YAML examples):
//! - `fixture_other_developer_three_claims` — Rachel publishes three
//!   claims about `github:rust-lang/cargo` that Maria can pull, query,
//!   and counter (US-FED-002 + US-FED-003 + US-FED-004 happy path).
//! - `fixture_adversarial_peer_*` — preconfigured adversarial postures
//!   for WD-40 / WD-41 / KPI-FED-6 pull-time rejection tests.
//!
//! RED-baseline scaffold per Mandate 7 (slice-03 first scaffold):
//! signatures and CIDs are placeholders until DELIVER's first scenario
//! activates these — at which point the body materializes the real
//! deterministic CBOR + Ed25519 bytes against `FakeIdentity::rachel`.
//
// SCAFFOLD: true

#![allow(dead_code)]
#![allow(unused_variables)]

use crate::fake_peer_pds::FakePeerRecord;

/// The canonical Rachel-on-Cargo fixture trio.
///
/// Used by US-FED-002 (peer pull happy path), US-FED-003 (federated
/// query happy path), US-FED-004 (counter-claim authoring — Maria
/// counters one of Rachel's three claims).
///
/// All three claims share `subject = "github:rust-lang/cargo"` so a
/// single federated query returns three rows under Rachel's author
/// header (and the same subject can host Maria's own claim too, for the
/// "3 found across 2 authors" assertion in US-FED-003 Example 1).
///
/// Author DID: `did:plc:rachel-test` (distinct from `did:plc:test-jeff`
/// + `did:plc:test-maria` used by slice-01 fixtures).
pub fn fixture_other_developer_three_claims() -> Vec<FakePeerRecord> {
    panic!("Not yet implemented -- RED scaffold")
}

/// Adversarial fixture: Rachel publishes ONE record whose `author` field
/// is `did:plc:test-maria` (the local user when this fixture is wired
/// against a Maria-initialized test env). Per WD-40 this MUST be
/// rejected at pull time with `PeerStorageError::SelfAttribution`.
///
/// Returns a populated peer-PDS posture honoring two well-formed records
/// alongside the one self-attributing record; pull MUST store the 2,
/// reject the 1, and exit non-zero (per WD-37 fault-isolation +
/// ADR-013 exit-code table for `peer pull`).
pub fn fixture_adversarial_peer_self_attribution() -> Vec<FakePeerRecord> {
    panic!("Not yet implemented -- RED scaffold")
}

/// Adversarial fixture: Rachel's PDS hosts ONE record whose `author`
/// field is `did:plc:trusted-third-party-test` (a DID OTHER than the
/// subscribed peer `did:plc:rachel-test`). Per WD-41 this MUST be
/// rejected at pull time with `PeerStorageError::CrossAttribution` —
/// slice-03's "subscribing to a peer means accepting THEIR claims"
/// trust model.
///
/// Returns a populated peer-PDS posture honoring two well-formed records
/// alongside the one cross-attributing record; pull MUST store the 2,
/// reject the 1, and exit non-zero.
pub fn fixture_adversarial_peer_cross_attribution() -> Vec<FakePeerRecord> {
    panic!("Not yet implemented -- RED scaffold")
}

/// Adversarial fixture: Rachel's PDS hosts ONE record whose signature
/// has been deliberately tampered (last byte flipped) after the peer's
/// nominal sign step. Drives KPI-FED-6 (zero invalid signatures stored)
/// and US-FED-002 Example 2 (4 stored / 1 rejected). Returns the honest
/// records; pair with `FakePeerPds::with_tampered_signature` at the
/// call site so the fixture knows which to corrupt.
pub fn fixture_adversarial_peer_tampered_signature() -> Vec<FakePeerRecord> {
    panic!("Not yet implemented -- RED scaffold")
}

/// Adversarial fixture: Rachel's PDS hosts ONE record whose rkey does
/// NOT match its locally-recomputed CID (canonicalization disagreement;
/// "possible adversarial input"). Drives integration gate
/// `peer_cid_round_trip` + US-FED-002 UAT scenario "Peer claim with CID
/// mismatch is rejected at ingest." Returns the honest records; pair
/// with `FakePeerPds::with_cid_mismatch`.
pub fn fixture_adversarial_peer_cid_mismatch() -> Vec<FakePeerRecord> {
    panic!("Not yet implemented -- RED scaffold")
}
