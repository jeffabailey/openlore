//! `seed_index <path>`: writes the one-claim `index.duckdb` the CI lambda-smoke seeds
//! its bucket with, so a changed DID list can change the fingerprint with no network
//! (DEVOPS lambda-smoke). The author is on no list the smoke sets, so a purge drops
//! the claim and the fingerprint goes from (1, 1, its CID's hash) to (0, 0, 0).
//!
//! It goes through the real adapter, so the file has the schema the image opens. It
//! is built in `indexer-build` and never reaches the image.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use adapter_index_store::IndexStoreAdapter;
use chrono::{DateTime, Utc};
use claim_domain::{Cid, Did, KeyId};
use ports::{
    AuthorRelationship, IndexStoreError, IndexStorePort, IndexedClaim, PeerClaimProvenance,
};

const SEEDED_AUTHOR: &str = "did:web:seeded-author.smoke.invalid";
const SEEDED_CID: &str = "bafysmokeseededclaim0001";
const COMPOSED_AT: &str = "2026-01-01T00:00:00Z";

fn seeded_claim() -> IndexedClaim {
    IndexedClaim {
        author_did: Did(SEEDED_AUTHOR.to_string()),
        cid: Cid(SEEDED_CID.to_string()),
        subject: "https://smoke.invalid/seeded".to_string(),
        predicate: "embodiesPhilosophy".to_string(),
        object: "org.openlore.philosophy.reproducible-builds".to_string(),
        confidence: 0.5,
        composed_at: COMPOSED_AT
            .parse::<DateTime<Utc>>()
            .expect("a fixed RFC 3339 time"),
        verified_against: KeyId(SEEDED_AUTHOR.to_string()),
        evidence: vec!["https://smoke.invalid/evidence".to_string()],
        references: Vec::new(),
        relationship: AuthorRelationship::NetworkUnfollowed,
        provenance: PeerClaimProvenance::SelfAttested,
    }
}

/// The checkpoint folds the claim into the file: a copy holds only the file, so a
/// row left in the log would be lost.
fn write_seed(path: &Path) -> Result<(), IndexStoreError> {
    let store = IndexStoreAdapter::open(path)?;
    store.upsert(&seeded_claim())?;
    store.checkpoint()
}

fn main() -> ExitCode {
    let Some(path) = std::env::args_os().nth(1).map(PathBuf::from) else {
        eprintln!("usage: seed_index <path>");
        return ExitCode::from(2);
    };
    match write_seed(&path) {
        Ok(()) => ExitCode::SUCCESS,
        Err(failed) => {
            eprintln!("seed_index: {failed}");
            ExitCode::FAILURE
        }
    }
}
