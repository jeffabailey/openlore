//! Real-infrastructure tests of the read-only link lineage (ADR-085 §5/§7,
//! ADR-060, slice-2 review B2): a REAL DuckDB file per test, written through
//! the storage ports and read through `LinkLineageReadPort` (never mocked).
//!
//! A link's referrers span BOTH stores — own `claim_references` and pulled
//! `peer_claim_references` — and each keeps the author it was stored under,
//! so the shared claim-domain lineage rule sees a peer's own withdrawal and
//! ignores a stranger's.

use adapter_duckdb::DuckDbStorageAdapter;
use chrono::{TimeZone, Utc};
use claim_domain::{
    is_self_retracted, is_superseded_by_author, Cid, ClaimLineage, ClaimReference, Confidence, Did,
    ReferenceType, SignatureBlock, SignedClaim, UnsignedClaim,
};
use ports::{ClaimReferrer, LinkLineageReadPort, PeerStoragePort, StoragePort};
use tempfile::TempDir;
use url::Url;

const RACHEL: &str = "did:plc:rachel-test";
const STRANGER: &str = "did:plc:stranger-test";
const LOCAL_USER: &str = "did:plc:test-jeff";
const LINK_CID: &str = "bafylinkrachel";

fn confidence(value: f64) -> Confidence {
    serde_json::from_value(serde_json::json!(value)).expect("confidence round-trips")
}

/// A claim by `author` with CID `cid`, referencing `references`.
fn claim(author: &str, cid: &str, references: &[(ReferenceType, &str)]) -> SignedClaim {
    SignedClaim {
        unsigned: UnsignedClaim {
            subject: RACHEL.to_string(),
            predicate: "sameAs".to_string(),
            object: "github:rachelk".to_string(),
            evidence: Vec::new(),
            confidence: confidence(1.0),
            author_did: Did(author.to_string()),
            composed_at: "2026-10-01T09:00:00Z".to_string(),
            references: references
                .iter()
                .map(|(ref_type, cid)| ClaimReference {
                    ref_type: *ref_type,
                    cid: Cid(cid.to_string()),
                })
                .collect(),
            reason: None,
        },
        signature: SignatureBlock {
            signed_cid: Cid(cid.to_string()),
            signature_bytes: vec![0x01, 0x02],
            verification_method: format!("{author}#org.openlore.application"),
        },
    }
}

/// A store holding Rachel's link, pulled from her as a peer.
struct Store {
    own: DuckDbStorageAdapter,
    _tmp: TempDir,
}

impl Store {
    fn with_rachels_link() -> Self {
        let tmp = TempDir::new().expect("create tempdir");
        let own = DuckDbStorageAdapter::open(&tmp.path().join("openlore.duckdb"))
            .expect("open adapter on tempdir");
        let store = Self { own, _tmp: tmp };
        store.pull(RACHEL, claim(RACHEL, LINK_CID, &[]));
        store
    }

    /// `author`'s claim, cached as pulled from `author`'s own repo.
    fn pull(&self, author: &str, signed: SignedClaim) {
        let pds = Url::parse("https://peer.example/pds").expect("a url");
        let fetched_at = Utc.with_ymd_and_hms(2026, 10, 2, 9, 0, 0).unwrap();
        let written = self
            .own
            .peer_adapter(&Did(LOCAL_USER.to_string()))
            .write_peer_claim(&Did(author.to_string()), &signed, &pds, fetched_at)
            .expect("cache the peer claim");
        assert!(written.written);
    }

    fn referrers_of_link(&self) -> Vec<ClaimReferrer> {
        self.own
            .link_lineage_adapter()
            .referrers_of(LINK_CID)
            .expect("read the link's lineage")
    }
}

/// Rachel's link as the lineage rule reads it.
fn rachels_link() -> ClaimLineage<'static> {
    ClaimLineage {
        author_did: RACHEL,
        cid: LINK_CID,
        references: &[],
    }
}

fn markers(referrers: &[ClaimReferrer]) -> Vec<ClaimLineage<'_>> {
    referrers
        .iter()
        .map(|referrer| ClaimLineage {
            author_did: &referrer.author_did,
            cid: &referrer.cid,
            references: &referrer.references,
        })
        .collect()
}

/// A peer's own retraction of their link is read, attributed to them, and
/// withdraws the link.
#[test]
fn a_peers_own_retraction_of_their_link_is_read_and_withdraws_it() {
    let store = Store::with_rachels_link();
    store.pull(
        RACHEL,
        claim(
            RACHEL,
            "bafyretract",
            &[(ReferenceType::Retracts, LINK_CID)],
        ),
    );

    let referrers = store.referrers_of_link();

    assert_eq!(
        referrers,
        vec![ClaimReferrer {
            cid: "bafyretract".to_string(),
            author_did: RACHEL.to_string(),
            references: vec![ClaimReference {
                ref_type: ReferenceType::Retracts,
                cid: Cid(LINK_CID.to_string()),
            }],
        }]
    );
    assert!(is_self_retracted(&rachels_link(), &markers(&referrers)));
}

/// A peer's own supersession of their link is read and replaces the link.
#[test]
fn a_peers_own_supersession_of_their_link_replaces_it() {
    let store = Store::with_rachels_link();
    store.pull(
        RACHEL,
        claim(
            RACHEL,
            "bafysuccessor",
            &[(ReferenceType::Supersedes, LINK_CID)],
        ),
    );

    let referrers = store.referrers_of_link();

    assert!(is_superseded_by_author(
        &rachels_link(),
        &markers(&referrers)
    ));
}

/// A stranger's peer retraction or supersession of Rachel's link is read
/// under the stranger's own name, so it never withdraws her link.
#[test]
fn a_strangers_peer_retraction_or_supersession_is_ignored() {
    let store = Store::with_rachels_link();
    store.pull(
        STRANGER,
        claim(
            STRANGER,
            "bafystranger",
            &[
                (ReferenceType::Retracts, LINK_CID),
                (ReferenceType::Supersedes, LINK_CID),
            ],
        ),
    );

    let referrers = store.referrers_of_link();

    assert_eq!(referrers.len(), 1, "{referrers:?}");
    assert_eq!(referrers[0].author_did, STRANGER);
    assert_eq!(referrers[0].references.len(), 2, "{referrers:?}");
    let markers = markers(&referrers);
    assert!(!is_self_retracted(&rachels_link(), &markers));
    assert!(!is_superseded_by_author(&rachels_link(), &markers));
}

/// An own claim referencing a claim is still read, under its own author.
#[test]
fn an_own_retraction_is_read_under_its_own_author() {
    let tmp = TempDir::new().expect("create tempdir");
    let own = DuckDbStorageAdapter::open(&tmp.path().join("openlore.duckdb"))
        .expect("open adapter on tempdir");
    let jeff = format!("{LOCAL_USER}#org.openlore.application");
    own.write_signed_claim(&claim(&jeff, "bafyownlink", &[]))
        .expect("write own link");
    own.write_signed_claim(&claim(
        &jeff,
        "bafyownretract",
        &[(ReferenceType::Retracts, "bafyownlink")],
    ))
    .expect("write own retraction");

    let referrers = own
        .link_lineage_adapter()
        .referrers_of("bafyownlink")
        .expect("read the own link's lineage");

    assert_eq!(referrers.len(), 1, "{referrers:?}");
    assert_eq!(referrers[0].cid, "bafyownretract");
    assert_eq!(referrers[0].author_did, jeff);
}
