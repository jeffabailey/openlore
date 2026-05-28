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
//! - `fixture_adversarial_peer_*` — the HONEST record set each adversarial
//!   posture is built on top of. The single offending record is appended
//!   by the matching `FakePeerPds::with_*` constructor (DD-FED-3
//!   constructor-time-pinned posture), so each fixture here returns ONLY
//!   the honest records — pair it with the constructor at the call site:
//!
//!   ```ignore
//!   let peer = FakePeerPds::with_tampered_signature(
//!       "did:plc:rachel-test",
//!       fixture_adversarial_peer_tampered_signature(),
//!   );
//!   ```
//!
//! Wire shape: each record `body` is the ATProto JSON the peer published
//! (`author`, `composedAt`, `signature: {kid, alg, sig}`) per
//! data-models.md §"Example peer counter-claim file". The signatures here
//! are deterministic placeholders — the real Ed25519 bytes are materialized
//! per-scenario in DELIVER phases 03-05 once `claim_domain::verify` is
//! wired into the pull pipeline. The shape is load-bearing now; the
//! cryptographic validity becomes load-bearing later.

#![allow(dead_code)]

use crate::fake_peer_pds::FakePeerRecord;

/// The subscribed peer DID used across the happy-path peer fixtures.
pub const RACHEL_DID: &str = "did:plc:rachel-test";

/// Build one well-formed peer claim record body attributed to `author_did`.
/// Helper so the fixture functions stay declarative.
fn peer_claim_body(
    subject: &str,
    object: &str,
    confidence: f64,
    author_did: &str,
    sig_tag: &str,
) -> serde_json::Value {
    serde_json::json!({
        "subject": subject,
        "predicate": "embodiesPhilosophy",
        "object": object,
        "evidence": [format!("https://example.test/evidence/{sig_tag}")],
        "confidence": confidence,
        "author": format!("{author_did}#org.openlore.application"),
        "composedAt": "2026-05-22T09:18:44Z",
        "references": [],
        "signature": {
            "kid": format!("{author_did}#org.openlore.application"),
            "alg": "EdDSA",
            "sig": format!("MEUCIQDz{sig_tag}HonestSignatureBytesBase64Url00000000")
        }
    })
}

/// The canonical Rachel-on-Cargo fixture trio.
///
/// Used by US-FED-002 (peer pull happy path), US-FED-003 (federated query
/// happy path), US-FED-004 (counter-claim authoring — Maria counters one
/// of Rachel's three claims).
///
/// All three claims share `subject = "github:rust-lang/cargo"` so a single
/// federated query returns three rows under Rachel's author header (and
/// the same subject can host Maria's own claim too, for the "3 found across
/// 2 authors" assertion in US-FED-003 Example 1). Each uses a distinct
/// `object` so the three CIDs cannot collapse via content aliasing.
///
/// Author DID: `did:plc:rachel-test` (distinct from `did:plc:test-jeff` +
/// `did:plc:test-maria` used by slice-01 fixtures). Every record is
/// well-formed and attributed to Rachel.
pub fn fixture_other_developer_three_claims() -> Vec<FakePeerRecord> {
    vec![
        FakePeerRecord::claim(
            "bafyrachelclaim0001000000000000000000000000000000000000",
            peer_claim_body(
                "github:rust-lang/cargo",
                "org.openlore.philosophy.dependency-pinning",
                0.42,
                RACHEL_DID,
                "rachel01",
            ),
        ),
        FakePeerRecord::claim(
            "bafyrachelclaim0002000000000000000000000000000000000000",
            peer_claim_body(
                "github:rust-lang/cargo",
                "org.openlore.philosophy.reproducible-builds",
                0.71,
                RACHEL_DID,
                "rachel02",
            ),
        ),
        FakePeerRecord::claim(
            "bafyrachelclaim0003000000000000000000000000000000000000",
            peer_claim_body(
                "github:rust-lang/cargo",
                "org.openlore.philosophy.workspace-cohesion",
                0.88,
                RACHEL_DID,
                "rachel03",
            ),
        ),
    ]
}

/// Honest record set for the self-attribution posture (WD-40).
///
/// Returns two well-formed records attributed to Rachel; pair with
/// `FakePeerPds::with_self_attribution(RACHEL_DID, victim_did, …)` which
/// appends the one record whose `author` is the LOCAL USER's DID. Pull MUST
/// store the 2 honest records, reject the 1 self-attributing record, and
/// exit non-zero (WD-37 fault isolation + ADR-013 exit-code table).
pub fn fixture_adversarial_peer_self_attribution() -> Vec<FakePeerRecord> {
    two_honest_rachel_records("selfattr")
}

/// Honest record set for the cross-attribution posture (WD-41).
///
/// Returns two well-formed records attributed to Rachel; pair with
/// `FakePeerPds::with_cross_attribution(RACHEL_DID, third_party_did, …)`
/// which appends the one record whose `author` is a DID OTHER than the
/// subscribed peer. Pull MUST store the 2, reject the 1, and exit non-zero
/// — slice-03's "subscribing to a peer means accepting THEIR claims" trust
/// model.
pub fn fixture_adversarial_peer_cross_attribution() -> Vec<FakePeerRecord> {
    two_honest_rachel_records("crossattr")
}

/// Honest record set for the tampered-signature posture (KPI-FED-6).
///
/// Returns FOUR well-formed records attributed to Rachel; pair with
/// `FakePeerPds::with_tampered_signature(RACHEL_DID, …)` which appends the
/// one record whose `signature.sig` last byte is flipped. Pull MUST store
/// the 4 honest records, reject the 1 tampered record (KPI-FED-6: zero
/// invalid signatures stored), and exit non-zero. This is US-FED-002
/// Example 2 ("4 stored / 1 rejected").
pub fn fixture_adversarial_peer_tampered_signature() -> Vec<FakePeerRecord> {
    (1..=4)
        .map(|i| {
            FakePeerRecord::claim(
                format!("bafyrachelhonest{i:04}0000000000000000000000000000000000000"),
                peer_claim_body(
                    "github:rust-lang/cargo",
                    &format!("org.openlore.philosophy.honest-claim-{i}"),
                    0.50 + (i as f64) * 0.05,
                    RACHEL_DID,
                    &format!("honest{i}"),
                ),
            )
        })
        .collect()
}

/// Honest record set for the CID-mismatch posture.
///
/// Returns two well-formed records attributed to Rachel; pair with
/// `FakePeerPds::with_cid_mismatch(RACHEL_DID, …)` which appends the one
/// record whose published rkey does NOT recompute byte-equal to its body
/// CID ("possible adversarial input"). Drives integration gate
/// `peer_cid_round_trip` + US-FED-002 UAT scenario "Peer claim with CID
/// mismatch is rejected at ingest."
pub fn fixture_adversarial_peer_cid_mismatch() -> Vec<FakePeerRecord> {
    two_honest_rachel_records("cidmismatch")
}

/// Two well-formed Rachel records with distinct objects so their CIDs do
/// not alias. `tag` keeps signatures/evidence distinct across postures.
fn two_honest_rachel_records(tag: &str) -> Vec<FakePeerRecord> {
    vec![
        FakePeerRecord::claim(
            format!("bafy{tag}honest0001000000000000000000000000000000000000"),
            peer_claim_body(
                "github:rust-lang/cargo",
                "org.openlore.philosophy.dependency-pinning",
                0.42,
                RACHEL_DID,
                &format!("{tag}1"),
            ),
        ),
        FakePeerRecord::claim(
            format!("bafy{tag}honest0002000000000000000000000000000000000000"),
            peer_claim_body(
                "github:rust-lang/cargo",
                "org.openlore.philosophy.reproducible-builds",
                0.71,
                RACHEL_DID,
                &format!("{tag}2"),
            ),
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The happy-path trio is three distinct, well-formed, Rachel-attributed
    /// records sharing the same subject. Distinct objects guarantee distinct
    /// CIDs (no content aliasing).
    #[test]
    fn three_claims_are_distinct_and_rachel_attributed() {
        let records = fixture_other_developer_three_claims();
        assert_eq!(records.len(), 3, "fixture publishes exactly three claims");

        for r in &records {
            assert_eq!(r.collection, "org.openlore.claim");
            assert_eq!(r.body["subject"], "github:rust-lang/cargo");
            assert_eq!(
                r.author(),
                Some("did:plc:rachel-test#org.openlore.application"),
                "every happy-path record is attributed to Rachel"
            );
        }

        let objects: std::collections::HashSet<_> =
            records.iter().map(|r| r.body["object"].to_string()).collect();
        assert_eq!(objects.len(), 3, "objects must be distinct (no CID aliasing)");

        let rkeys: std::collections::HashSet<_> =
            records.iter().map(|r| r.rkey.clone()).collect();
        assert_eq!(rkeys.len(), 3, "rkeys must be distinct");
    }

    /// The tampered-signature honest set has four records (so the posture
    /// yields the "4 stored / 1 rejected" US-FED-002 Example 2 shape).
    #[test]
    fn tampered_signature_fixture_has_four_honest_records() {
        let records = fixture_adversarial_peer_tampered_signature();
        assert_eq!(records.len(), 4, "US-FED-002 Example 2: 4 honest records");
        for r in &records {
            assert_eq!(
                r.author(),
                Some("did:plc:rachel-test#org.openlore.application"),
                "honest records are Rachel-attributed"
            );
        }
    }

    /// The remaining adversarial fixtures return two honest records each.
    #[test]
    fn other_adversarial_fixtures_have_two_honest_records() {
        assert_eq!(fixture_adversarial_peer_self_attribution().len(), 2);
        assert_eq!(fixture_adversarial_peer_cross_attribution().len(), 2);
        assert_eq!(fixture_adversarial_peer_cid_mismatch().len(), 2);
    }
}
