//! `adapter-duckdb` — embedded DuckDB-backed `StoragePort` implementation.
//!
//! Writes signed-claim JSON files alongside the DB so the on-disk layout
//! matches `data-models.md`. Manages schema migrations. Probe verifies
//! schema-version match + fsync honored + write-read-equal round-trip
//! per ADR-001.
//!
//! RED-baseline scaffold (step 01-01).
//
// SCAFFOLD: true

#![allow(dead_code)]
#![forbid(unsafe_code)]

use chrono::{DateTime, Utc};
use claim_domain::{Cid, ReferenceType, SignedClaim};
use ports::{ProbeOutcome, StorageError, StoragePort};

/// Embedded-DuckDB `StoragePort` adapter. Holds the open DB handle + the
/// path to the colocated `claims/` JSON directory.
pub struct DuckDbStorageAdapter {
    _scaffold: (),
}

impl DuckDbStorageAdapter {
    /// Open the DB at the given path; run pending migrations; prepare the
    /// colocated `claims/` directory.
    pub fn open(_db_path: &std::path::Path) -> Result<Self, StorageError> {
        panic!("Not yet implemented -- RED scaffold");
    }
}

impl StoragePort for DuckDbStorageAdapter {
    fn probe(&self) -> ProbeOutcome {
        panic!("Not yet implemented -- RED scaffold");
    }

    fn write_signed_claim(&self, _signed: &SignedClaim) -> Result<(), StorageError> {
        panic!("Not yet implemented -- RED scaffold");
    }

    fn read_signed_claim(&self, _cid: &Cid) -> Result<Option<SignedClaim>, StorageError> {
        panic!("Not yet implemented -- RED scaffold");
    }

    fn query_by_subject(&self, _subject: &str) -> Result<Vec<SignedClaim>, StorageError> {
        panic!("Not yet implemented -- RED scaffold");
    }

    fn query_referencing(
        &self,
        _target_cid: &Cid,
    ) -> Result<Vec<(Cid, ReferenceType)>, StorageError> {
        panic!("Not yet implemented -- RED scaffold");
    }

    fn record_publication(
        &self,
        _cid: &Cid,
        _at_uri: &str,
        _published_at: DateTime<Utc>,
    ) -> Result<(), StorageError> {
        panic!("Not yet implemented -- RED scaffold");
    }
}
