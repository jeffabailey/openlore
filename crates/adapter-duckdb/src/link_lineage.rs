//! `DuckDbLinkLineageAdapter` — the read-only lineage of a stored claim: the
//! stored claims referencing it (ADR-085 §4/§7).
//!
//! It wraps a storage adapter SHARING the one DuckDB connection, but exposes
//! only [`LinkLineageReadPort`]: the verb holding it can read a link's
//! retraction / supersession markers and can never write a claim.

use claim_domain::{Cid, SignedClaim};
use ports::{LinkLineageReadPort, StorageError, StoragePort};

use crate::DuckDbStorageAdapter;

/// Read-only lineage reads over the SHARED DuckDB connection.
pub struct DuckDbLinkLineageAdapter {
    storage: DuckDbStorageAdapter,
}

impl DuckDbLinkLineageAdapter {
    /// Construct from a storage adapter sharing the open handle (see
    /// `DuckDbStorageAdapter::link_lineage_adapter`).
    pub(crate) fn from_shared(storage: DuckDbStorageAdapter) -> Self {
        Self { storage }
    }
}

impl LinkLineageReadPort for DuckDbLinkLineageAdapter {
    fn referrers_of(&self, cid: &str) -> Result<Vec<SignedClaim>, StorageError> {
        let referencing = self.storage.query_referencing(&Cid(cid.to_string()))?;
        let read = referencing
            .iter()
            .map(|(referrer, _)| self.storage.read_signed_claim(referrer))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(read.into_iter().flatten().collect())
    }
}
