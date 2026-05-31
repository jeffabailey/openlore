//! `store_read` — the slice-06 READ-ONLY `StoreReadPort` impl (ADR-030).
//!
//! The `openlore ui` viewer reads the operator's OWN `claims` table over a port
//! that exposes NO write/sign surface (I-VIEW-1). This adapter shares the EXACT
//! `Arc<Mutex<Connection>>` the CLI's `StoragePort` adapter writes through
//! (BR-VIEW-4) — there is NO second connection, NO second file. Read-only SQL
//! only: `list_claims` is a paginated ordered SELECT; `count_claims` is a
//! `COUNT(*)`.
//!
//! ## Functional discipline
//!
//! Pure-shaped railway: each read returns `Result<_, StoreReadError>`, never
//! panics. The `claims` table is projected into the FLAT [`ports::ClaimRow`] DTO
//! (subject/predicate/object/confidence/author_did/composed_at/cid) the pure
//! `viewer-domain` core renders — no `SignedClaim`/artifact read needed for the
//! list view.

use std::sync::{Arc, Mutex};

use chrono::{DateTime, Utc};
use duckdb::Connection;
use ports::{
    ClaimDetail, ClaimRow, Page, PageRequest, PeerClaimRow, PeerOrigin, StoreReadError,
    StoreReadPort,
};

/// Read-only view over the SAME shared DuckDB connection the CLI writes through.
/// Constructed via [`crate::DuckDbStorageAdapter::read_adapter`] so no second
/// handle to the DB file is ever opened (BR-VIEW-4).
pub struct DuckDbStoreReadAdapter {
    conn: Arc<Mutex<Connection>>,
}

impl DuckDbStoreReadAdapter {
    /// Construct from a shared connection handle (cloned `Arc`). Private to the
    /// crate — only [`crate::DuckDbStorageAdapter::read_adapter`] builds it.
    pub(crate) fn from_shared(conn: Arc<Mutex<Connection>>) -> Self {
        Self { conn }
    }

    /// Lock the shared connection, mapping a poisoned mutex to a plain-language
    /// [`StoreReadError::Unreadable`]. The single site for the poison-recovery
    /// rule — every read method acquires the connection through here so a
    /// poisoned lock surfaces as a clean refusal (NFR-VIEW-6), never a panic.
    fn lock_conn(&self) -> Result<std::sync::MutexGuard<'_, Connection>, StoreReadError> {
        self.conn.lock().map_err(|_| StoreReadError::Unreadable {
            detail: "connection mutex poisoned".to_string(),
        })
    }
}

impl StoreReadPort for DuckDbStoreReadAdapter {
    fn list_claims(&self, request: PageRequest) -> Result<Page<ClaimRow>, StoreReadError> {
        let conn = self.lock_conn()?;

        // Ordered, paginated, read-only SELECT. composed_at DESC per ADR-030
        // (most-recent first). The `request` offset/limit selects one page; the
        // total COUNT(*) below feeds the renderer's position indicator (FR-VIEW-6).
        let mut stmt = conn
            .prepare(
                "SELECT cid, subject, predicate, object, confidence, author_did, composed_at \
                 FROM claims ORDER BY composed_at DESC, cid LIMIT ? OFFSET ?",
            )
            .map_err(|err| StoreReadError::QueryFailed {
                detail: format!("prepare list_claims: {err}"),
            })?;

        let row_iter = stmt
            .query_map(
                duckdb::params![request.limit as i64, request.offset as i64],
                |row| {
                    Ok(ClaimRow {
                        cid: row.get::<_, String>(0)?,
                        subject: row.get::<_, String>(1)?,
                        predicate: row.get::<_, String>(2)?,
                        object: row.get::<_, String>(3)?,
                        confidence: row.get::<_, f64>(4)?,
                        author_did: row.get::<_, String>(5)?,
                        composed_at: row.get::<_, DateTime<Utc>>(6)?,
                    })
                },
            )
            .map_err(|err| StoreReadError::QueryFailed {
                detail: format!("query_map list_claims: {err}"),
            })?;

        let mut rows = Vec::new();
        for row in row_iter {
            rows.push(row.map_err(|err| StoreReadError::QueryFailed {
                detail: format!("row decode list_claims: {err}"),
            })?);
        }
        drop(stmt);

        let total: i64 = conn
            .query_row("SELECT COUNT(*) FROM claims", [], |row| row.get(0))
            .map_err(|err| StoreReadError::QueryFailed {
                detail: format!("count for list_claims total: {err}"),
            })?;

        Ok(Page {
            rows,
            total: total as u64,
        })
    }

    fn count_claims(&self) -> Result<usize, StoreReadError> {
        let conn = self.lock_conn()?;
        let total: i64 = conn
            .query_row("SELECT COUNT(*) FROM claims", [], |row| row.get(0))
            .map_err(|err| StoreReadError::Unreadable {
                detail: format!("count_claims sentinel read failed: {err}"),
            })?;
        Ok(total as usize)
    }

    fn get_claim(&self, cid: &str) -> Result<Option<ClaimDetail>, StoreReadError> {
        let conn = self.lock_conn()?;

        // Read the scalar claim row (read-only). Missing CID -> Ok(None): a
        // guided not-found is the viewer's job, not an error (step 02-03).
        let mut stmt = conn
            .prepare(
                "SELECT cid, subject, predicate, object, confidence, author_did, composed_at \
                 FROM claims WHERE cid = ?",
            )
            .map_err(|err| StoreReadError::QueryFailed {
                detail: format!("prepare get_claim: {err}"),
            })?;
        let mut claim_iter = stmt
            .query_map(duckdb::params![cid], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, f64>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, DateTime<Utc>>(6)?,
                ))
            })
            .map_err(|err| StoreReadError::QueryFailed {
                detail: format!("query_map get_claim: {err}"),
            })?;
        let claim = match claim_iter.next() {
            None => {
                drop(claim_iter);
                drop(stmt);
                return Ok(None);
            }
            Some(row) => row.map_err(|err| StoreReadError::QueryFailed {
                detail: format!("row decode get_claim: {err}"),
            })?,
        };
        drop(claim_iter);
        drop(stmt);

        // Read the evidence URLs ordered by ordinal ascending (the order they
        // were attached, FR-VIEW-3).
        let mut ev_stmt = conn
            .prepare("SELECT evidence FROM claim_evidence WHERE cid = ? ORDER BY ordinal ASC")
            .map_err(|err| StoreReadError::QueryFailed {
                detail: format!("prepare get_claim evidence: {err}"),
            })?;
        let ev_iter = ev_stmt
            .query_map(duckdb::params![cid], |row| row.get::<_, String>(0))
            .map_err(|err| StoreReadError::QueryFailed {
                detail: format!("query_map get_claim evidence: {err}"),
            })?;
        let mut evidence = Vec::new();
        for url in ev_iter {
            evidence.push(url.map_err(|err| StoreReadError::QueryFailed {
                detail: format!("row decode get_claim evidence: {err}"),
            })?);
        }
        drop(ev_stmt);

        let (cid, subject, predicate, object, confidence, author_did, composed_at) = claim;
        Ok(Some(ClaimDetail {
            cid,
            subject,
            predicate,
            object,
            confidence,
            author_did,
            composed_at,
            evidence,
        }))
    }

    fn list_peer_claims(&self, request: PageRequest) -> Result<Page<PeerClaimRow>, StoreReadError> {
        let conn = self.lock_conn()?;

        // Ordered, paginated, read-only SELECT over the SAME shared connection's
        // slice-03 `peer_claims` table (BR-VIEW-4). composed_at DESC mirrors
        // `list_claims` (most-recent first). The peer ORIGIN is projected from
        // `author_did` + `fetched_from_pds` — there is no `peer_origin` column.
        let mut stmt = conn
            .prepare(
                "SELECT cid, subject, predicate, object, confidence, author_did, \
                 fetched_from_pds, composed_at \
                 FROM peer_claims ORDER BY composed_at DESC, cid LIMIT ? OFFSET ?",
            )
            .map_err(|err| StoreReadError::QueryFailed {
                detail: format!("prepare list_peer_claims: {err}"),
            })?;

        let row_iter = stmt
            .query_map(
                duckdb::params![request.limit as i64, request.offset as i64],
                |row| {
                    let author_did = row.get::<_, String>(5)?;
                    let fetched_from_pds = row.get::<_, String>(6)?;
                    // The origin IS (author_did, fetched_from_pds). A blank
                    // author_did (defensive data bypassing the slice-03 CHECK)
                    // projects to `Unknown` so the viewer labels it rather than
                    // dropping the row (step 03-03 / V-10). The production
                    // `peer pull` path always yields a non-empty author_did, so
                    // step 03-01 produces only `Known`.
                    let origin = if author_did.is_empty() {
                        PeerOrigin::Unknown
                    } else {
                        PeerOrigin::Known {
                            author_did,
                            fetched_from_pds,
                        }
                    };
                    Ok(PeerClaimRow {
                        cid: row.get::<_, String>(0)?,
                        subject: row.get::<_, String>(1)?,
                        predicate: row.get::<_, String>(2)?,
                        object: row.get::<_, String>(3)?,
                        confidence: row.get::<_, f64>(4)?,
                        origin,
                        composed_at: row.get::<_, DateTime<Utc>>(7)?,
                    })
                },
            )
            .map_err(|err| StoreReadError::QueryFailed {
                detail: format!("query_map list_peer_claims: {err}"),
            })?;

        let mut rows = Vec::new();
        for row in row_iter {
            rows.push(row.map_err(|err| StoreReadError::QueryFailed {
                detail: format!("row decode list_peer_claims: {err}"),
            })?);
        }
        drop(stmt);

        let total: i64 = conn
            .query_row("SELECT COUNT(*) FROM peer_claims", [], |row| row.get(0))
            .map_err(|err| StoreReadError::QueryFailed {
                detail: format!("count for list_peer_claims total: {err}"),
            })?;

        Ok(Page {
            rows,
            total: total as u64,
        })
    }

    fn count_peer_claims(&self) -> Result<usize, StoreReadError> {
        let conn = self.lock_conn()?;
        let total: i64 = conn
            .query_row("SELECT COUNT(*) FROM peer_claims", [], |row| row.get(0))
            .map_err(|err| StoreReadError::QueryFailed {
                detail: format!("count_peer_claims read failed: {err}"),
            })?;
        Ok(total as usize)
    }
}
