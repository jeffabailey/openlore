//! Real-infrastructure tests of the link-candidate read (U-19): a REAL DuckDB
//! file per test, written through the storage ports and read through
//! `LinkCandidateReadPort` (never mocked).
//!
//! A peer Jeff removed keeps their cached claims (soft remove, ADR-057) but
//! contributes no link candidate to any reader; a peer he follows does, and
//! a peer he re-adds does again.

use adapter_duckdb::DuckDbStorageAdapter;
use chrono::{TimeZone, Utc};
use claim_domain::{Cid, Confidence, Did, SignatureBlock, SignedClaim, UnsignedClaim};
use ports::{LinkCandidateReadPort, PeerStoragePort, PeerSubscription};
use tempfile::TempDir;
use url::Url;

const RACHEL: &str = "did:plc:rachel-test";
const OMAR: &str = "did:plc:omar-test";
const LOCAL_USER: &str = "did:plc:test-jeff";
const SAME_AS: &str = "sameAs";

fn confidence(value: f64) -> Confidence {
    serde_json::from_value(serde_json::json!(value)).expect("confidence round-trips")
}

/// `author`'s link from their own DID to `github:<login>`, with CID `cid`.
fn link(author: &str, login: &str, cid: &str) -> SignedClaim {
    SignedClaim {
        unsigned: UnsignedClaim {
            subject: author.to_string(),
            predicate: SAME_AS.to_string(),
            object: format!("github:{login}"),
            evidence: Vec::new(),
            confidence: confidence(1.0),
            author_did: Did(author.to_string()),
            composed_at: "2026-10-01T09:00:00Z".to_string(),
            references: Vec::new(),
            reason: None,
        },
        signature: SignatureBlock {
            signed_cid: Cid(cid.to_string()),
            signature_bytes: vec![0x01, 0x02],
            verification_method: format!("{author}#org.openlore.application"),
        },
    }
}

struct Store {
    own: DuckDbStorageAdapter,
    _tmp: TempDir,
}

impl Store {
    fn open() -> Self {
        let tmp = TempDir::new().expect("create tempdir");
        let own = DuckDbStorageAdapter::open(&tmp.path().join("openlore.duckdb"))
            .expect("open adapter on tempdir");
        Self { own, _tmp: tmp }
    }

    fn peers(&self) -> impl PeerStoragePort {
        self.own.peer_adapter(&Did(LOCAL_USER.to_string()))
    }

    /// Jeff follows `author` and pulls their link.
    fn follow_and_pull(&self, author: &str, login: &str, cid: &str) {
        let pds = Url::parse("https://peer.example/pds").expect("a url");
        let peers = self.peers();
        peers
            .add_subscription(PeerSubscription {
                peer_did: Did(author.to_string()),
                peer_handle: format!("{login}.test"),
                peer_pds_endpoint: pds.clone(),
                subscribed_at: Utc.with_ymd_and_hms(2026, 10, 1, 8, 0, 0).unwrap(),
                removed_at: None,
            })
            .expect("follow the peer");
        let fetched_at = Utc.with_ymd_and_hms(2026, 10, 2, 9, 0, 0).unwrap();
        let written = peers
            .write_peer_claim(
                &Did(author.to_string()),
                &link(author, login, cid),
                &pds,
                fetched_at,
            )
            .expect("cache the peer's link");
        assert!(written.written);
    }

    fn candidate_cids(&self) -> Vec<String> {
        self.own
            .identity_link_check_adapter()
            .link_candidates(SAME_AS)
            .expect("read link candidates")
            .into_iter()
            .map(|candidate| candidate.cid)
            .collect()
    }
}

#[test]
fn a_removed_peers_link_is_no_candidate_while_a_followed_peers_link_is() {
    let store = Store::open();
    store.follow_and_pull(RACHEL, "rachelk", "bafylinkrachel");
    store.follow_and_pull(OMAR, "omarq", "bafylinkomar");
    assert_eq!(
        store.candidate_cids(),
        vec!["bafylinkomar", "bafylinkrachel"]
    );

    let removed = store
        .peers()
        .soft_remove(&Did(RACHEL.to_string()))
        .expect("remove Rachel");
    assert_eq!(removed.cached_claim_count, 1, "her cached link is retained");

    assert_eq!(store.candidate_cids(), vec!["bafylinkomar"]);
}

#[test]
fn a_re_added_peers_link_is_a_candidate_again() {
    let store = Store::open();
    store.follow_and_pull(RACHEL, "rachelk", "bafylinkrachel");
    store
        .peers()
        .soft_remove(&Did(RACHEL.to_string()))
        .expect("remove Rachel");
    assert!(store.candidate_cids().is_empty());

    store
        .peers()
        .hard_purge(&Did(RACHEL.to_string()))
        .expect("forget Rachel");
    store.follow_and_pull(RACHEL, "rachelk", "bafylinkrachel");

    assert_eq!(store.candidate_cids(), vec!["bafylinkrachel"]);
}
