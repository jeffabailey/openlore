//! Golden pin of the self-attested `org.openlore.claim` record bytes
//! (github-did-identity-link Q-RM-1).
//!
//! Characterization test guarding the move of `claim_record_json` from
//! `review-domain::plans` into `claim-domain`: the bytes a publish record and
//! a retract record serialize to today are frozen in
//! `fixtures/record_json_golden.json` and must stay byte-identical through
//! the `review_domain::plans` re-export after the move.

use ports::claim_domain::{Cid, ClaimReference, Confidence, Did, ReferenceType, UnsignedClaim};
use review_domain::plans::claim_record_json;

const GOLDEN: &str = include_str!("fixtures/record_json_golden.json");

fn published_claim() -> UnsignedClaim {
    UnsignedClaim {
        subject: "github:BurntSushi/ripgrep".to_string(),
        predicate: "adheres_to".to_string(),
        object: "org.openlore.philosophy.semver".to_string(),
        evidence: vec![
            "https://github.com/BurntSushi/ripgrep/blob/master/CHANGELOG.md".to_string(),
            "https://github.com/BurntSushi/ripgrep/releases".to_string(),
        ],
        confidence: Confidence::from_basis_points(8_500),
        author_did: Did("did:plc:test-maria".to_string()),
        composed_at: "2026-10-01T09:00:00Z".to_string(),
        references: Vec::new(),
        reason: None,
    }
}

fn retraction_claim() -> UnsignedClaim {
    UnsignedClaim {
        composed_at: "2026-10-02T09:00:00Z".to_string(),
        references: vec![ClaimReference {
            ref_type: ReferenceType::Retracts,
            cid: Cid("bafyreigoldenpublishedclaim".to_string()),
        }],
        ..published_claim()
    }
}

fn golden(kind: &str) -> String {
    let fixture: serde_json::Value = serde_json::from_str(GOLDEN).expect("golden fixture is JSON");
    fixture[kind]
        .as_str()
        .unwrap_or_else(|| panic!("golden fixture holds a `{kind}` string"))
        .to_string()
}

#[test]
fn a_publish_record_serializes_to_the_pinned_bytes() {
    assert_eq!(
        claim_record_json(&published_claim()).to_string(),
        golden("publish")
    );
}

#[test]
fn a_retract_record_serializes_to_the_pinned_bytes() {
    assert_eq!(
        claim_record_json(&retraction_claim()).to_string(),
        golden("retract")
    );
}
