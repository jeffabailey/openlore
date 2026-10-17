//! `DuckDbLinkLineageAdapter` — the read-only lineage of a stored claim: the
//! stored claims, own AND pulled from peers, referencing it (ADR-085 §4/§7,
//! ADR-060).
//!
//! It holds the SHARED DuckDB connection but exposes only
//! [`LinkLineageReadPort`]: the verb holding it can read a link's
//! retraction / supersession markers and can never write a claim. A peer's
//! own retraction or supersession of their link lands in
//! `peer_claim_references` / `peer_claims`, so the read spans both stores;
//! each referrer keeps the author it was stored under, so the same-author
//! rule (only a link's own author withdraws it) is decided downstream, in
//! `claim_domain`, never merged away here (I-FED-1).

use std::collections::BTreeMap;

use claim_domain::{Cid, ClaimReference};
use ports::{ClaimReferrer, LinkLineageReadPort, StorageError};

use crate::conn::{ConnGuard, SharedConn};

/// Every reference to one claim, own and peer, with the referencing claim's
/// stored author projected from BOTH stores (no_cross_table_join_elides_author).
const REFERRERS_SQL: &str = "
    SELECT r.referencing_cid, c.author_did, r.ref_type
    FROM claim_references r JOIN claims c ON c.cid = r.referencing_cid
    WHERE r.referenced_cid = ?
    UNION ALL
    SELECT r.referencing_cid, pc.author_did, r.ref_type
    FROM peer_claim_references r JOIN peer_claims pc ON pc.cid = r.referencing_cid
    WHERE r.referenced_cid = ?
    ORDER BY 1, 3";

/// Read-only lineage reads over the SHARED DuckDB connection.
pub struct DuckDbLinkLineageAdapter {
    conn: SharedConn,
}

impl DuckDbLinkLineageAdapter {
    /// Construct from the SHARED handle (see
    /// `DuckDbStorageAdapter::link_lineage_adapter`).
    pub(crate) fn from_shared(conn: SharedConn) -> Self {
        Self { conn }
    }

    fn lock(&self) -> Result<ConnGuard<'_>, StorageError> {
        self.conn
            .lock()
            .map_err(|lock_err| StorageError::QueryFailed {
                message: lock_err.to_string(),
            })
    }
}

fn query_failed(step: &str) -> impl Fn(duckdb::Error) -> StorageError + '_ {
    move |err| StorageError::QueryFailed {
        message: format!("{step}: {err}"),
    }
}

/// One stored reference to the claim read: (referencing CID, its stored
/// author, the reference type token).
type ReferenceRow = (String, String, String);

/// The referencing rows grouped into one referrer per referencing claim.
fn referrers_of_rows(cid: &str, rows: Vec<ReferenceRow>) -> Result<Vec<ClaimReferrer>, String> {
    rows.into_iter()
        .try_fold(
            BTreeMap::<String, ClaimReferrer>::new(),
            |mut referrers, (referencing, author_did, ref_type)| {
                let reference = ClaimReference {
                    ref_type: crate::reference_type_from_sql(&ref_type)?,
                    cid: Cid(cid.to_string()),
                };
                referrers
                    .entry(referencing.clone())
                    .or_insert_with(|| ClaimReferrer {
                        cid: referencing,
                        author_did,
                        references: Vec::new(),
                    })
                    .references
                    .push(reference);
                Ok(referrers)
            },
        )
        .map(|referrers| referrers.into_values().collect())
}

impl LinkLineageReadPort for DuckDbLinkLineageAdapter {
    fn referrers_of(&self, cid: &str) -> Result<Vec<ClaimReferrer>, StorageError> {
        let conn = self.lock()?;
        let mut stmt = conn
            .prepare(REFERRERS_SQL)
            .map_err(query_failed("prepare referrers_of"))?;
        let rows = stmt
            .query_map(duckdb::params![cid, cid], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?))
            })
            .map_err(query_failed("query referrers_of"))?
            .collect::<Result<Vec<ReferenceRow>, _>>()
            .map_err(query_failed("decode referrer"))?;
        referrers_of_rows(cid, rows).map_err(|message| StorageError::QueryFailed { message })
    }
}
