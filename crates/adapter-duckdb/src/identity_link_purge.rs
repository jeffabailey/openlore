//! `DuckDbIdentityLinkPurgeAdapter` — the one DELETE of the local proof-check
//! history, held only by `identity forget` (C-GDL-5, ADR-085 §7; the ADR-082
//! separate-purge precedent).
//!
//! One statement, scoped to exactly one subject DID: no other subject's checks
//! and no claim are touched. The rest of `identity_link_checks` stays
//! insert-only (check-arch `identity_link_checks_insert_only` exempts this
//! file alone).

use crate::conn::SharedConn;

use ports::{IdentityLinkCheckError, IdentityLinkCheckPurgePort};

const PURGE_SUBJECT_SQL: &str = "DELETE FROM identity_link_checks WHERE subject_did = ?";

/// Purge of one subject's checks over the SHARED DuckDB connection.
pub struct DuckDbIdentityLinkPurgeAdapter {
    conn: SharedConn,
}

impl DuckDbIdentityLinkPurgeAdapter {
    /// Construct from the SHARED handle (see
    /// `DuckDbStorageAdapter::identity_link_purge_adapter`).
    pub(crate) fn from_shared(conn: SharedConn) -> Self {
        Self { conn }
    }
}

impl IdentityLinkCheckPurgePort for DuckDbIdentityLinkPurgeAdapter {
    fn purge_subject(&self, subject_did: &str) -> Result<u64, IdentityLinkCheckError> {
        let conn = self
            .conn
            .lock()
            .map_err(|lock_err| IdentityLinkCheckError::Store(lock_err.to_string()))?;
        conn.execute(PURGE_SUBJECT_SQL, duckdb::params![subject_did])
            .map(|deleted| u64::try_from(deleted).unwrap_or(u64::MAX))
            .map_err(|err| IdentityLinkCheckError::Store(format!("purge checks: {err}")))
    }
}
