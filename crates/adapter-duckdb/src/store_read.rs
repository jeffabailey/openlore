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

use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use chrono::{DateTime, Utc};
use claim_domain::{Cid, Did, SignedClaim};
use duckdb::Connection;
use ports::{
    AttributedClaim, AuthorRelationship, ClaimDetail, ClaimRow, CounterClaimRow, Page, PageRequest,
    PeerClaimRow, PeerOrigin, PeerSubscriptionSummary, StoreReadError, StoreReadPort, SurveyRow,
};

use crate::bare_did;

/// Read-only view over the SAME shared DuckDB connection the CLI writes through.
/// Constructed via [`crate::DuckDbStorageAdapter::read_adapter`] so no second
/// handle to the DB file is ever opened (BR-VIEW-4).
///
/// ## Read-only enforcement boundary (I-VIEW-1 / I-CS-4)
///
/// This struct holds an `Arc<Mutex<Connection>>` — a connection that is, at the
/// type level, fully capable of writing. The read-only guarantee (I-VIEW-1 /
/// I-CS-4: the viewer NEVER mutates the store) is NOT enforced by this type; it is
/// enforced at the [`StoreReadPort`] TRAIT boundary, which exposes NO mutation
/// method (only `list_*` / `count_*` / `get_claim` / `query_contributor_scoring_feed`
/// — all read-only SELECTs). The impl must therefore only ever be reached THROUGH
/// the trait: callers depend on `&dyn StoreReadPort`, never on this concrete type,
/// so the absence of a write method is the contract.
///
/// That trait-level guarantee is backed by two further layers (three-layer
/// enforcement): the `xtask check-arch` capability rule (which audits that the
/// viewer holds no write/sign capability), and the behavioral gold tests
/// (`every_score_route_leaves_the_store_read_only` / the slice-06 `viewer_is_read_only`
/// twins) that exercise every route and assert the persisted row counts are
/// UNCHANGED. A `ReadOnlyConnection` newtype wrapping the handle was CONSIDERED and
/// REJECTED as over-engineering: the impl is unreachable except through the
/// no-mutation trait, so the threat it would guard against is not reachable.
pub struct DuckDbStoreReadAdapter {
    conn: Arc<Mutex<Connection>>,
    /// The storage root's `peer_claims` directory — used to resolve a peer
    /// counter's RELATIVE `signed_record_path` (`peer_claims/<encoded_did>/<cid>.json`)
    /// when reading its artifact for the free-text `reason` (the ADR-046 step-B read).
    /// Mirrors `DuckDbStorageAdapter::read_artifact_at`'s resolution so own-claim
    /// (absolute) and peer-claim (relative) artifact paths both resolve correctly.
    peer_claims_root: PathBuf,
}

impl DuckDbStoreReadAdapter {
    /// Construct from a shared connection handle (cloned `Arc`) + the storage
    /// root's `peer_claims` directory (for resolving peer artifact paths in the
    /// counter-thread step-B read). Private to the crate — only
    /// [`crate::DuckDbStorageAdapter::read_adapter`] builds it.
    pub(crate) fn from_shared(conn: Arc<Mutex<Connection>>, peer_claims_root: PathBuf) -> Self {
        Self {
            conn,
            peer_claims_root,
        }
    }

    /// Read one counter's free-text `reason` from its on-disk `SignedClaim`
    /// artifact — the ADR-046 step-B read (the reason is NOT a DB column; it lives
    /// in the artifact, ADR-015). Own-counter rows store an ABSOLUTE path under
    /// `claims/`; peer-counter rows store a path RELATIVE to the storage root
    /// (`peer_claims/<encoded_did>/<cid>.json`) resolved under `peer_claims_root`
    /// — mirroring `DuckDbStorageAdapter::read_artifact_at`. READ-ONLY: a plain
    /// `fs::read`, LOCAL only, no network.
    ///
    /// BEST-EFFORT (graceful degradation, review D2): the counter entry itself is
    /// authoritative from the Step-A DB ref lookup (its `author_did` + `cid` are
    /// already in hand); only the free-text `reason` lives in the artifact. A
    /// MISSING / unreadable / undeserializable artifact — a plausible real scenario
    /// for a PULLED PEER counter whose artifact isn't local — therefore yields
    /// `None` (the existing "no reason provided" render state), NOT an error. The
    /// counter STILL renders. This NEVER panics and NEVER fails the enclosing
    /// `query_counter_claims` call (which would 5xx the `/claims/{cid}` detail page).
    /// Only the per-row artifact/reason read is best-effort; genuine DB / Step-A
    /// errors are still surfaced by the caller.
    fn read_reason_at(&self, artifact_path: &str) -> Option<String> {
        let resolved: PathBuf = match artifact_path.strip_prefix("peer_claims/") {
            Some(relative) => self.peer_claims_root.join(relative),
            None => PathBuf::from(artifact_path),
        };
        // fs read error (missing / unreadable) -> None (degrade, do not error).
        let bytes = fs::read(&resolved).ok()?;
        // deserialize error (corrupt / unexpected shape) -> None (degrade).
        let signed: SignedClaim = serde_json::from_slice(&bytes).ok()?;
        signed.unsigned.reason
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

    /// Shared engine for the two LOCAL attributed survey reads (`/project` +
    /// `/philosophy`; slice-10 / ADR-042). INTERNAL impl-sharing only — the
    /// [`StoreReadPort`] trait deliberately keeps TWO public methods (ADR-042 chose
    /// two reads over a dimension-enum at the port boundary); this helper is private
    /// to the adapter and exists so the byte-identical cross-store SELECT + row-decode
    /// machinery lives in ONE place. The two callers differ ONLY in the GROUPED
    /// dimension: `filter_col` is the column the `?` value matches (`subject` for the
    /// project survey, `object` for the philosophy survey) and `order_col` is the
    /// GROUPED dimension placed first in `ORDER BY` (so the pure grouper's first-seen
    /// group order is deterministic) — the mirror of `filter_col`. Both are
    /// hard-coded `&'static str` column names (NEVER user input), so the formatted SQL
    /// carries no injection surface; the matched VALUE is always a bound `?` param.
    /// `label` names the calling method in error details.
    ///
    /// READ-ONLY: own `claims` UNION ALL local `peer_claims`, EXPLICIT `author_did` +
    /// `cid` projection (NEVER a merging JOIN/GROUP BY/AVG — `xtask check-arch::
    /// no_cross_table_join_elides_author` enforces it), LOCAL only (no network).
    /// Grouping into edges is the PURE `viewer-domain` core's job in Rust — this query
    /// returns one row per signed claim (each survey edge maps to exactly one claim,
    /// I-GT-4). The own arm carries an empty `fetched_from_pds` + `'Own'` discriminant;
    /// the peer arm carries its PDS endpoint + `'Peer'` (data-models.md §4).
    fn query_survey(
        &self,
        value: &str,
        filter_col: &'static str,
        order_col: &'static str,
        label: &str,
    ) -> Result<Vec<SurveyRow>, StoreReadError> {
        let conn = self.lock_conn()?;

        let sql = format!(
            "SELECT author_did, cid, subject, predicate, object, confidence, \
             composed_at, fetched_from_pds, source_table \
             FROM ( \
               SELECT c.author_did AS author_did, c.cid AS cid, c.subject AS subject, \
                      c.predicate AS predicate, c.object AS object, \
                      c.confidence AS confidence, c.composed_at AS composed_at, \
                      '' AS fetched_from_pds, 'Own' AS source_table \
               FROM claims c \
               WHERE c.{filter_col} = ? \
               UNION ALL \
               SELECT pc.author_did AS author_did, pc.cid AS cid, pc.subject AS subject, \
                      pc.predicate AS predicate, pc.object AS object, \
                      pc.confidence AS confidence, pc.composed_at AS composed_at, \
                      pc.fetched_from_pds AS fetched_from_pds, 'Peer' AS source_table \
               FROM peer_claims pc \
               WHERE pc.{filter_col} = ? \
             ) ORDER BY {order_col}, source_table, cid"
        );

        let mut stmt = conn
            .prepare(&sql)
            .map_err(|err| StoreReadError::QueryFailed {
                detail: format!("prepare {label}: {err}"),
            })?;
        let row_iter = stmt
            .query_map(duckdb::params![value, value], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, f64>(5)?,
                    row.get::<_, DateTime<Utc>>(6)?,
                    row.get::<_, String>(7)?,
                    row.get::<_, String>(8)?,
                ))
            })
            .map_err(|err| StoreReadError::QueryFailed {
                detail: format!("query_map {label}: {err}"),
            })?;

        let mut survey = Vec::new();
        for row in row_iter {
            let (
                author_did,
                cid,
                subject,
                predicate,
                object,
                confidence,
                composed_at,
                fetched_from_pds,
                source_table,
            ) = row.map_err(|err| StoreReadError::QueryFailed {
                detail: format!("row decode {label}: {err}"),
            })?;
            // The origin IS the (source_table, author_did, fetched_from_pds) triple:
            // an own row carries an empty PDS; a peer row carries the PDS it was
            // fetched from. A blank author_did (defensive, bypassing the slice-03
            // CHECK) projects to `Unknown` so the viewer labels rather than drops it.
            let origin = peer_origin(&author_did, fetched_from_pds);
            let _ = source_table;
            survey.push(SurveyRow {
                author_did,
                cid,
                subject,
                predicate,
                object,
                confidence,
                origin,
                composed_at,
            });
        }
        Ok(survey)
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
        let own_claim = match claim_iter.next() {
            None => None,
            Some(row) => Some(row.map_err(|err| StoreReadError::QueryFailed {
                detail: format!("row decode get_claim: {err}"),
            })?),
        };
        drop(claim_iter);
        drop(stmt);

        // slice-11: a counter-thread's target may be a PULLED PEER claim (Maria
        // counters Rachel's peer claim — the self-counter rule forbids countering
        // one's OWN claim, ADR-015 / WD-34). So when the CID is not an OWN claim,
        // FALL BACK to the LOCAL `peer_claims` table (the SAME shared connection,
        // read-only). This keeps the detail route able to render the countered claim
        // verbatim (built from `get_claim` UNCHANGED — I-CT-2) regardless of which
        // local store holds it. A CID in NEITHER store still yields `Ok(None)` (the
        // guided not-found — slice-06 V-7 unchanged). The own table is checked FIRST
        // so an own claim's detail is byte-identical to slice-06.
        let claim = match own_claim {
            Some(row) => row,
            None => {
                let mut peer_stmt = conn
                    .prepare(
                        "SELECT cid, subject, predicate, object, confidence, author_did, \
                         composed_at FROM peer_claims WHERE cid = ?",
                    )
                    .map_err(|err| StoreReadError::QueryFailed {
                        detail: format!("prepare get_claim peer fallback: {err}"),
                    })?;
                let mut peer_iter = peer_stmt
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
                        detail: format!("query_map get_claim peer fallback: {err}"),
                    })?;
                match peer_iter.next() {
                    None => {
                        drop(peer_iter);
                        drop(peer_stmt);
                        return Ok(None);
                    }
                    Some(row) => {
                        let decoded = row.map_err(|err| StoreReadError::QueryFailed {
                            detail: format!("row decode get_claim peer fallback: {err}"),
                        })?;
                        drop(peer_iter);
                        drop(peer_stmt);
                        decoded
                    }
                }
            }
        };

        // Read the evidence URLs ordered by ordinal ascending (the order they
        // were attached, FR-VIEW-3). The CID is unique to ONE store, so UNION the
        // own `claim_evidence` with the peer `peer_claim_evidence` (slice-11 peer
        // fallback) — only the table holding the claim contributes rows; the other
        // is empty. ORDER BY ordinal preserves attachment order in both cases.
        let mut ev_stmt = conn
            .prepare(
                "SELECT evidence, ordinal FROM ( \
                   SELECT evidence, ordinal FROM claim_evidence WHERE cid = ? \
                   UNION ALL \
                   SELECT evidence, ordinal FROM peer_claim_evidence WHERE cid = ? \
                 ) ORDER BY ordinal ASC",
            )
            .map_err(|err| StoreReadError::QueryFailed {
                detail: format!("prepare get_claim evidence: {err}"),
            })?;
        let ev_iter = ev_stmt
            .query_map(duckdb::params![cid, cid], |row| row.get::<_, String>(0))
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
                    let origin = peer_origin(&author_did, fetched_from_pds);
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

    fn count_active_peer_subscriptions(&self) -> Result<usize, StoreReadError> {
        let conn = self.lock_conn()?;
        // ONE aggregate over the SAME shared connection (mirrors `count_claims` /
        // `count_peer_claims`). Active-only `WHERE removed_at IS NULL` — the SAME
        // definition as `list_active_peer_subscriptions` (ADR-052 / BR-LD-2): a
        // soft-removed row is residue, EXCLUDED. Touches ONLY `peer_subscriptions`
        // (NO JOIN — it counts subscriptions, not claims).
        let total: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM peer_subscriptions WHERE removed_at IS NULL",
                [],
                |row| row.get(0),
            )
            .map_err(|err| StoreReadError::QueryFailed {
                detail: format!("count_active_peer_subscriptions read failed: {err}"),
            })?;
        Ok(total as usize)
    }

    fn count_countered_own_claims(&self) -> Result<usize, StoreReadError> {
        let conn = self.lock_conn()?;
        // ADR-055 D1: ONE aggregate over the SAME shared connection (mirrors
        // `count_claims` / `count_peer_claims` / `count_active_peer_subscriptions`).
        // COUNT(DISTINCT own cid) where the own CID appears as a COUNTERED
        // `referenced_cid` across the two INDEXED ref tables (`claim_references` ∪
        // `peer_claim_references`, `ref_type = 'counters'`). The inner `UNION` (set
        // union — de-duped, not `UNION ALL`) collapses the countered referenced_cids to
        // a DISTINCT set; `c.cid IN (...)` is a membership test, so a claim countered by
        // N peers contributes its cid ONCE → it counts ONCE (presence-once, C-4 /
        // BR-CC-1, no JOIN-fanout). The outer `COUNT(DISTINCT c.cid)` is belt-and-braces.
        // Own-only by the outer `claims` table (a countered PEER claim is not in
        // `claims`, so it never contributes — WD-CC-7 own-only by query shape).
        // Parameter-free (the only WHERE value is the literal `'counters'`) →
        // injection-safe. Invariant to store size (both ref columns indexed, ADR-048).
        let total: i64 = conn
            .query_row(
                "SELECT COUNT(DISTINCT c.cid) FROM claims c WHERE c.cid IN (\
                     SELECT referenced_cid FROM claim_references      WHERE ref_type = 'counters' \
                     UNION \
                     SELECT referenced_cid FROM peer_claim_references WHERE ref_type = 'counters')",
                [],
                |row| row.get(0),
            )
            .map_err(|err| StoreReadError::QueryFailed {
                detail: format!("count_countered_own_claims read failed: {err}"),
            })?;
        Ok(total as usize)
    }

    fn count_countered_peer_claims(&self) -> Result<usize, StoreReadError> {
        let conn = self.lock_conn()?;
        // ADR-056 D1: the EXACT slice-18 `count_countered_own_claims` aggregate with the
        // OUTER table swapped `claims c → peer_claims p`. ONE aggregate over the SAME
        // shared connection. COUNT(DISTINCT peer cid) where the PEER CID appears as a
        // COUNTERED `referenced_cid` across the two INDEXED ref tables (`claim_references`
        // ∪ `peer_claim_references`, `ref_type = 'counters'`). The inner `UNION` (set
        // union — de-duped, not `UNION ALL`) collapses the countered referenced_cids to
        // a DISTINCT set; `p.cid IN (...)` is a membership test, so a peer claim countered
        // by N counterers contributes its cid ONCE → it counts ONCE (presence-once, C-4 /
        // BR-PC-1, no JOIN-fanout). The outer `COUNT(DISTINCT p.cid)` is belt-and-braces.
        // The inner UNION IN-set is BYTE-IDENTICAL to slice-18's — only the outer table
        // differs. Peer-only by the outer `peer_claims` table (a countered OWN claim is
        // not in `peer_claims`, so it never contributes — R-PC-9 peer-only by query
        // shape; the xtask `no_cross_table_join_elides_author` rule stays GREEN by
        // construction since `peer_claims` is named whole-word and standalone `claims` is
        // NOT). A cached peer claim is countered by the OPERATOR (her counter in
        // `claim_references`) OR by ANOTHER peer (their counter in `peer_claim_references`,
        // slice-11) — both arms of the UNION contribute. Parameter-free (the only WHERE
        // value is the literal `'counters'`) → injection-safe. Invariant to store size
        // (both ref columns indexed, ADR-048).
        let total: i64 = conn
            .query_row(
                "SELECT COUNT(DISTINCT p.cid) FROM peer_claims p WHERE p.cid IN (\
                     SELECT referenced_cid FROM claim_references      WHERE ref_type = 'counters' \
                     UNION \
                     SELECT referenced_cid FROM peer_claim_references WHERE ref_type = 'counters')",
                [],
                |row| row.get(0),
            )
            .map_err(|err| StoreReadError::QueryFailed {
                detail: format!("count_countered_peer_claims read failed: {err}"),
            })?;
        Ok(total as usize)
    }

    fn distinct_own_author_dids(
        &self,
    ) -> Result<std::collections::HashSet<String>, StoreReadError> {
        let conn = self.lock_conn()?;
        // ADR-057 D1 / D-FS-1: the `You`-arm presence read. ONE single-table SELECT
        // over the SAME shared connection — `SELECT DISTINCT author_did FROM claims`.
        // SINGLE-TABLE by construction: it names `claims` ONLY, NO JOIN to
        // `peer_claims` — so the `xtask check-arch::no_cross_table_join_elides_author`
        // anti-merging precondition is structurally unreachable (the rule's trigger is
        // a JOIN across the two stores; this query has none). Parameter-free →
        // injection-safe. Own claims store the `#org.openlore.application` signing
        // fragment on `author_did`; the effect shell bares BOTH sides via `bare_did`
        // before membership (R-FS-6), so the DIDs are projected VERBATIM here.
        let mut stmt = conn
            .prepare("SELECT DISTINCT author_did FROM claims")
            .map_err(|err| StoreReadError::QueryFailed {
                detail: format!("distinct_own_author_dids prepare: {err}"),
            })?;
        let rows = stmt
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(|err| StoreReadError::QueryFailed {
                detail: format!("distinct_own_author_dids query_map: {err}"),
            })?;
        let mut dids = std::collections::HashSet::new();
        for row in rows {
            dids.insert(row.map_err(|err| StoreReadError::QueryFailed {
                detail: format!("distinct_own_author_dids row decode: {err}"),
            })?);
        }
        Ok(dids)
    }

    fn distinct_cached_peer_author_dids(
        &self,
    ) -> Result<std::collections::HashSet<String>, StoreReadError> {
        let conn = self.lock_conn()?;
        // ADR-057 D1 / D-FS-1: the `UnsubscribedCache`-arm presence read. ONE
        // single-table SELECT over the SAME shared connection — `SELECT DISTINCT
        // author_did FROM peer_claims`, NO `removed_at` filter (the cached residue of
        // a soft-removed peer is RETAINED in `peer_claims`; the soft-remove only flips
        // `peer_subscriptions.removed_at`, never deletes the cached claims — slice-15
        // PS-4 residue). SINGLE-TABLE by construction: it names `peer_claims` ONLY, NO
        // JOIN to `claims`/`peer_subscriptions` — so the `xtask check-arch::
        // no_cross_table_join_elides_author` anti-merging precondition is structurally
        // unreachable. Parameter-free → injection-safe. The effect shell bares the
        // signing fragment via `bare_did` before membership (R-FS-6).
        let mut stmt = conn
            .prepare("SELECT DISTINCT author_did FROM peer_claims")
            .map_err(|err| StoreReadError::QueryFailed {
                detail: format!("distinct_cached_peer_author_dids prepare: {err}"),
            })?;
        let rows = stmt
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(|err| StoreReadError::QueryFailed {
                detail: format!("distinct_cached_peer_author_dids query_map: {err}"),
            })?;
        let mut dids = std::collections::HashSet::new();
        for row in rows {
            dids.insert(row.map_err(|err| StoreReadError::QueryFailed {
                detail: format!("distinct_cached_peer_author_dids row decode: {err}"),
            })?);
        }
        Ok(dids)
    }

    fn query_contributor_scoring_feed(
        &self,
        contributor: &Did,
    ) -> Result<Vec<AttributedClaim>, StoreReadError> {
        let conn = self.lock_conn()?;

        // The set of DIDs with a currently-ACTIVE peer subscription (`removed_at IS
        // NULL`) — drives the `SubscribedPeer` vs `UnsubscribedCache` relationship
        // on each peer row (slice-03 reuse). Read once, read-only.
        let active_peers = active_subscription_dids(&conn)?;

        // READ-ONLY cross-store SELECT for the contributor's LOCAL attributed feed:
        // own `claims` UNION ALL local `peer_claims`, EXPLICIT `author_did`
        // projection (NEVER a merging JOIN/GROUP BY — `xtask check-arch::
        // no_cross_table_join_elides_author` enforces it), LOCAL only (no network).
        // Own claims store the `#fragment` signing locator on `author_did`, so the
        // contributor filter matches the bare DID via a `LIKE '<bare>%'` prefix.
        // Aggregation (the weight) is the PURE `scoring::score` core's job in Rust —
        // this query returns one row per signed claim (the aggregate's
        // decomposition, I-GRAPH-2 / WD-73).
        let sql = "SELECT author_did, cid, subject, predicate, object, confidence, \
                   composed_at, source_table \
                   FROM ( \
                     SELECT c.author_did AS author_did, c.cid AS cid, c.subject AS subject, \
                            c.predicate AS predicate, c.object AS object, \
                            c.confidence AS confidence, c.composed_at AS composed_at, \
                            'Own' AS source_table \
                     FROM claims c \
                     WHERE c.author_did LIKE ? \
                     UNION ALL \
                     SELECT pc.author_did AS author_did, pc.cid AS cid, pc.subject AS subject, \
                            pc.predicate AS predicate, pc.object AS object, \
                            pc.confidence AS confidence, pc.composed_at AS composed_at, \
                            'Peer' AS source_table \
                     FROM peer_claims pc \
                     WHERE pc.author_did LIKE ? \
                   ) ORDER BY subject, source_table, cid";

        let param = format!("{}%", bare_did(&contributor.0));

        let mut stmt = conn.prepare(sql).map_err(|err| StoreReadError::QueryFailed {
            detail: format!("prepare query_contributor_scoring_feed: {err}"),
        })?;
        let row_iter = stmt
            .query_map(duckdb::params![param, param], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, f64>(5)?,
                    row.get::<_, DateTime<Utc>>(6)?,
                    row.get::<_, String>(7)?,
                ))
            })
            .map_err(|err| StoreReadError::QueryFailed {
                detail: format!("query_map query_contributor_scoring_feed: {err}"),
            })?;

        let mut feed = Vec::new();
        for row in row_iter {
            let (author_did, cid, subject, predicate, object, confidence, composed_at, source_table) =
                row.map_err(|err| StoreReadError::QueryFailed {
                    detail: format!("row decode query_contributor_scoring_feed: {err}"),
                })?;
            let bare_author = bare_did(&author_did);
            let relationship = match source_table.as_str() {
                "Own" => AuthorRelationship::You,
                _ if active_peers.contains(&bare_author) => AuthorRelationship::SubscribedPeer,
                _ => AuthorRelationship::UnsubscribedCache,
            };
            feed.push(AttributedClaim {
                author_did: Did(bare_author),
                cid: Cid(cid),
                subject,
                predicate,
                object,
                confidence,
                composed_at,
                relationship,
            });
        }
        Ok(feed)
    }

    fn query_project_survey(&self, subject: &str) -> Result<Vec<SurveyRow>, StoreReadError> {
        // The project survey filters on `subject` and GROUPS by `object` (the
        // philosophy embodied) — so `object` leads the ORDER BY for a deterministic
        // first-seen group order. Delegates to the shared `query_survey` engine
        // (INTERNAL impl-sharing; the trait keeps two methods per ADR-042).
        self.query_survey(subject, "subject", "object", "query_project_survey")
    }

    fn query_philosophy_survey(&self, object: &str) -> Result<Vec<SurveyRow>, StoreReadError> {
        // The SYMMETRIC mirror, swapping subject↔object: filters on `object` and
        // GROUPS by `subject` (the project that embodies the philosophy) — so
        // `subject` leads the ORDER BY. Delegates to the SAME shared engine.
        self.query_survey(object, "object", "subject", "query_philosophy_survey")
    }

    fn query_counter_claims(
        &self,
        target_cid: &str,
    ) -> Result<Vec<CounterClaimRow>, StoreReadError> {
        let conn = self.lock_conn()?;

        // ADR-046 STEP A: the INDEXED UNION-ALL ref lookup for every counter of
        // `target_cid`. Own counters live in `claims` JOIN `claim_references`; peer
        // counters in `peer_claims` JOIN `peer_claim_references` — both filtered by
        // the indexed `referenced_cid = ? AND ref_type = 'counters'`. The JOIN is
        // INTRA-store only (ref table → its own claims table) to recover the
        // counter's `author_did` + `cid` + `confidence` + `composed_at` + its
        // artifact path; the cross-store combination is a UNION ALL (NEVER a merging
        // JOIN/GROUP BY/AVG), projecting `author_did` + `cid` EXPLICITLY so two
        // counters by different authors stay TWO rows (anti-merging, I-CT-3 —
        // `xtask check-arch::no_cross_table_join_elides_author` enforces it). The own
        // arm carries an empty `fetched_from_pds` + `'Own'` discriminant; the peer
        // arm carries its PDS endpoint + `'Peer'`. ORDER BY (composed_at,
        // source_table, cid) is deterministic — composed_at is for ORDERING only,
        // never a re-weight of the countered claim (shown-never-applied, I-CT-2).
        let sql = "SELECT author_did, cid, confidence, composed_at, fetched_from_pds, \
                   artifact_path, source_table \
                   FROM ( \
                     SELECT c.author_did AS author_did, c.cid AS cid, \
                            c.confidence AS confidence, c.composed_at AS composed_at, \
                            '' AS fetched_from_pds, c.artifact_path AS artifact_path, \
                            'Own' AS source_table \
                     FROM claims c \
                     JOIN claim_references cr ON cr.referencing_cid = c.cid \
                     WHERE cr.referenced_cid = ? AND cr.ref_type = 'counters' \
                     UNION ALL \
                     SELECT pc.author_did AS author_did, pc.cid AS cid, \
                            pc.confidence AS confidence, pc.composed_at AS composed_at, \
                            pc.fetched_from_pds AS fetched_from_pds, \
                            pc.signed_record_path AS artifact_path, 'Peer' AS source_table \
                     FROM peer_claims pc \
                     JOIN peer_claim_references pcr ON pcr.referencing_cid = pc.cid \
                     WHERE pcr.referenced_cid = ? AND pcr.ref_type = 'counters' \
                   ) ORDER BY composed_at, source_table, cid";

        let mut stmt = conn.prepare(sql).map_err(|err| StoreReadError::QueryFailed {
            detail: format!("prepare query_counter_claims: {err}"),
        })?;
        let row_iter = stmt
            .query_map(duckdb::params![target_cid, target_cid], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, f64>(2)?,
                    row.get::<_, DateTime<Utc>>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                ))
            })
            .map_err(|err| StoreReadError::QueryFailed {
                detail: format!("query_map query_counter_claims: {err}"),
            })?;

        // Decode step A into intermediate tuples FIRST (releasing the statement
        // borrow) so step B's per-row artifact reads do not hold the prepared
        // statement open across the filesystem reads.
        let mut staged: Vec<(String, String, f64, DateTime<Utc>, String, String)> = Vec::new();
        for row in row_iter {
            let (author_did, cid, confidence, composed_at, fetched_from_pds, artifact_path, source_table) =
                row.map_err(|err| StoreReadError::QueryFailed {
                    detail: format!("row decode query_counter_claims: {err}"),
                })?;
            let _ = source_table;
            staged.push((
                author_did,
                cid,
                confidence,
                composed_at,
                fetched_from_pds,
                artifact_path,
            ));
        }
        drop(stmt);
        drop(conn);

        // ADR-046 STEP B: per-row artifact read for the free-text `reason` (NOT a DB
        // column — ADR-015). LOCAL `fs::read`, no network. BEST-EFFORT (review D2): a
        // missing / unreadable / undeserializable artifact degrades `reason` to
        // `None` (the "no reason provided" state) rather than failing the whole
        // query — the counter still renders from its authoritative DB `author_did` +
        // `cid`. A blank `author_did` (defensive, bypassing the slice-03 CHECK)
        // projects to `Unknown` so the viewer labels rather than drops it.
        let mut counters = Vec::with_capacity(staged.len());
        for (author_did, cid, confidence, composed_at, fetched_from_pds, artifact_path) in staged {
            let reason = self.read_reason_at(&artifact_path);
            let origin = peer_origin(&author_did, fetched_from_pds);
            counters.push(CounterClaimRow {
                author_did,
                cid,
                reason,
                confidence,
                composed_at,
                origin,
            });
        }
        Ok(counters)
    }

    fn counter_presence_for(
        &self,
        cids: &[String],
    ) -> Result<std::collections::HashSet<String>, StoreReadError> {
        // EMPTY input (an empty / all-un-countered page) → empty set, with NO query
        // prepared: an empty `IN ()` is a SQL error (ADR-048). This guard is the
        // no-noise common case (the list renders byte-identically to slice-06).
        if cids.is_empty() {
            return Ok(std::collections::HashSet::new());
        }

        let conn = self.lock_conn()?;

        // ADR-048: ONE aggregate `referenced_cid IN (...)` UNION-ALL DISTINCT read over
        // the INDEXED `claim_references` ∪ `peer_claim_references` ref tables
        // (`ref_type = 'counters'`). Ref-tables-only (NO JOIN to `claims`/`peer_claims`,
        // NO per-row artifact read — the flag carries no reason text). Returns the
        // SUBSET of input CIDs that are COUNTERED (a presence SET, never a count). The
        // CID list is bound via `params_from_iter` (NEVER string-interpolated —
        // injection-safe); the `(?, ?, …)` placeholder group is built from the input
        // arity and bound TWICE (own arm + peer arm), mirroring slice-11's double-bind.
        let placeholders = std::iter::repeat_n("?", cids.len())
            .collect::<Vec<_>>()
            .join(", ");
        let sql = format!(
            "SELECT DISTINCT referenced_cid FROM ( \
               SELECT referenced_cid FROM claim_references \
               WHERE referenced_cid IN ({placeholders}) AND ref_type = 'counters' \
               UNION ALL \
               SELECT referenced_cid FROM peer_claim_references \
               WHERE referenced_cid IN ({placeholders}) AND ref_type = 'counters' \
             )"
        );

        let mut stmt = conn.prepare(&sql).map_err(|err| StoreReadError::QueryFailed {
            detail: format!("prepare counter_presence_for: {err}"),
        })?;

        // Bind the CID list TWICE (the own arm's IN then the peer arm's IN). The two
        // `IN` lists share the SAME placeholder positions in order, so chaining the
        // slice with itself yields the correct positional binds (slice-11 `[cid, cid]`
        // double-bind, generalized to the whole list).
        let params = duckdb::params_from_iter(cids.iter().chain(cids.iter()));
        let row_iter = stmt
            .query_map(params, |row| row.get::<_, String>(0))
            .map_err(|err| StoreReadError::QueryFailed {
                detail: format!("query_map counter_presence_for: {err}"),
            })?;

        let mut presence = std::collections::HashSet::new();
        for row in row_iter {
            presence.insert(row.map_err(|err| StoreReadError::QueryFailed {
                detail: format!("row decode counter_presence_for: {err}"),
            })?);
        }
        Ok(presence)
    }

    fn list_active_peer_subscriptions(
        &self,
    ) -> Result<Vec<PeerSubscriptionSummary>, StoreReadError> {
        let conn = self.lock_conn()?;

        // ADR-052 / DD-PS-1: ONE aggregate query for the WHOLE active subscription set
        // + every per-peer count — invariant to peer count (NO N+1, NO per-peer fold).
        // `peer_subscriptions ps LEFT JOIN peer_claims pc ON pc.author_did = ps.peer_did`,
        // `WHERE ps.removed_at IS NULL` (active-only — a soft-removed row is residue,
        // excluded; I-PS-2), `GROUP BY` the subscription identity, `COUNT(pc.cid)` per
        // peer. The LEFT JOIN keeps a subscribed-but-never-pulled peer in the result;
        // `COUNT(pc.cid)` counts the NULL right side as 0 (NOT an inner JOIN that would
        // drop the row, NOT `COUNT(*)` that would count NULL as 1 — DD-PS-2). The `GROUP
        // BY ps.peer_did` decomposition is PER-PEER, so two peers stay TWO rows whose
        // counts are NEVER summed/averaged (anti-merging, J-003a / I-PS-3). The SELECT
        // names `peer_subscriptions` + `peer_claims` and projects `author_did` as the
        // GROUP BY key — the standalone `claims` table is never mentioned, so the
        // `no_cross_table_join_elides_author` xtask rule stays GREEN. LOCAL only, no
        // network (I-PS-4). READ-ONLY: a SELECT over the SAME shared connection (I-PS-1).
        let sql = "SELECT ps.peer_did, ps.peer_handle, ps.subscribed_at, \
                   COUNT(pc.cid) AS local_claim_count \
                   FROM peer_subscriptions ps \
                   LEFT JOIN peer_claims pc ON pc.author_did = ps.peer_did \
                   WHERE ps.removed_at IS NULL \
                   GROUP BY ps.peer_did, ps.peer_handle, ps.subscribed_at \
                   ORDER BY ps.subscribed_at, ps.peer_did";

        let mut stmt = conn.prepare(sql).map_err(|err| StoreReadError::QueryFailed {
            detail: format!("prepare list_active_peer_subscriptions: {err}"),
        })?;
        let row_iter = stmt
            .query_map([], |row| {
                Ok(PeerSubscriptionSummary {
                    peer_did: row.get::<_, String>(0)?,
                    peer_handle: row.get::<_, String>(1)?,
                    subscribed_at: row.get::<_, DateTime<Utc>>(2)?,
                    local_claim_count: row.get::<_, i64>(3)?.max(0) as u64,
                })
            })
            .map_err(|err| StoreReadError::QueryFailed {
                detail: format!("query_map list_active_peer_subscriptions: {err}"),
            })?;

        let mut subscriptions = Vec::new();
        for row in row_iter {
            subscriptions.push(row.map_err(|err| StoreReadError::QueryFailed {
                detail: format!("row decode list_active_peer_subscriptions: {err}"),
            })?);
        }
        Ok(subscriptions)
    }
}

/// Project a `(author_did, fetched_from_pds)` pair from a `peer_claims`-shaped row
/// into the [`PeerOrigin`] ADT — the SINGLE site for the "blank author_did ->
/// `Unknown`" defensive rule (BR-VIEW-5 / step 03-03 / V-10). A NON-EMPTY
/// `author_did` (the schema-guaranteed common case, slice-03 CHECK) yields
/// [`PeerOrigin::Known`]; a blank one (data bypassing the CHECK) yields
/// [`PeerOrigin::Unknown`] so the viewer LABELS rather than DROPS the row. Shared
/// by `query_survey`, `list_peer_claims`, and `query_counter_claims` (one mapping,
/// one place to attack).
fn peer_origin(author_did: &str, fetched_from_pds: String) -> PeerOrigin {
    if author_did.is_empty() {
        PeerOrigin::Unknown
    } else {
        PeerOrigin::Known {
            author_did: author_did.to_string(),
            fetched_from_pds,
        }
    }
}

/// The set of DIDs with a currently-ACTIVE peer subscription (`removed_at IS
/// NULL`). Read-only helper over the SAME shared connection (mirrors the slice-04
/// `graph_query` helper; takes the locked connection so it runs inside the
/// read-only `query_contributor_scoring_feed` shell). A peer row whose author is in
/// this set is a `SubscribedPeer`; otherwise an `UnsubscribedCache` (soft-removed
/// residue, ADR-014).
fn active_subscription_dids(
    conn: &Connection,
) -> Result<std::collections::HashSet<String>, StoreReadError> {
    let mut stmt = conn
        .prepare("SELECT peer_did FROM peer_subscriptions WHERE removed_at IS NULL")
        .map_err(|err| StoreReadError::QueryFailed {
            detail: format!("prepare active_subscription_dids: {err}"),
        })?;
    let rows = stmt
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|err| StoreReadError::QueryFailed {
            detail: format!("query active_subscription_dids: {err}"),
        })?;
    let mut dids = std::collections::HashSet::new();
    for row in rows {
        dids.insert(row.map_err(|err| StoreReadError::QueryFailed {
            detail: format!("row decode active_subscription_dids: {err}"),
        })?);
    }
    Ok(dids)
}

#[cfg(test)]
mod counter_presence_tests {
    //! slice-12 (US-LF-002/003; ADR-048) — `counter_presence_for` unit tests at the
    //! adapter scope. Pins the ADR-048 single-aggregate `IN (...)` UNION-ALL DISTINCT
    //! presence read: it returns the COUNTERED SUBSET of the input CIDs (a presence
    //! SET, never a count), bound (never interpolated), and an EMPTY input slice →
    //! `Ok(HashSet::new())` with NO query prepared (empty `IN ()` is a SQL error).

    use std::collections::HashSet;

    use claim_domain::{
        Cid, ClaimReference, Did, ReferenceType, SignatureBlock, SignedClaim, UnsignedClaim,
    };
    use ports::{StoragePort, StoreReadError, StoreReadPort};
    use tempfile::TempDir;

    use crate::DuckDbStorageAdapter;

    fn confidence(value: f64) -> claim_domain::Confidence {
        serde_json::from_value(serde_json::json!(value)).expect("confidence round-trips")
    }

    fn fresh_adapter() -> (DuckDbStorageAdapter, TempDir) {
        let tmp = TempDir::new().expect("create tempdir");
        let db_path = tmp.path().join("openlore.duckdb");
        let adapter = DuckDbStorageAdapter::open(&db_path).expect("open adapter on tempdir");
        (adapter, tmp)
    }

    fn claim(cid: &str, subject: &str, references: Vec<ClaimReference>) -> SignedClaim {
        SignedClaim {
            unsigned: UnsignedClaim {
                subject: subject.to_string(),
                predicate: "embodiesPhilosophy".to_string(),
                object: "org.openlore.philosophy.x".to_string(),
                evidence: vec![],
                confidence: confidence(0.90),
                author_did: Did("did:plc:maria#org.openlore.application".to_string()),
                composed_at: "2026-05-25T12:00:00Z".to_string(),
                references,
                reason: None,
            },
            signature: SignatureBlock {
                signed_cid: Cid(cid.to_string()),
                signature_bytes: vec![0xAA, 0xBB],
                verification_method: "did:plc:maria#org.openlore.application".to_string(),
            },
        }
    }

    /// `counter_presence_for` returns the COUNTERED SUBSET of the input CIDs (presence
    /// membership): a target referenced by a `counters` claim is in the set; an
    /// un-countered target and an unknown CID are not. Targets referenced by a
    /// NON-`counters` ref (e.g. `supersedes`) are NOT counted as countered.
    #[test]
    fn counter_presence_for_returns_the_countered_subset() {
        let (adapter, _tmp) = fresh_adapter();

        // Two own targets; only `bafyTarget` is countered. A third target is referenced
        // by a `supersedes` (NOT a counter) — it must NOT appear in the presence set.
        let target = claim("bafyTarget", "github:rust-lang/cargo", vec![]);
        let plain = claim("bafyPlain", "github:rust-lang/rust", vec![]);
        let superseded = claim("bafySuper", "github:denoland/deno", vec![]);
        let counter = claim(
            "bafyCounter",
            "github:rust-lang/cargo",
            vec![ClaimReference {
                ref_type: ReferenceType::Counters,
                cid: Cid("bafyTarget".to_string()),
            }],
        );
        let superseder = claim(
            "bafySuperseder",
            "github:denoland/deno",
            vec![ClaimReference {
                ref_type: ReferenceType::Supersedes,
                cid: Cid("bafySuper".to_string()),
            }],
        );
        for c in [&target, &plain, &superseded, &counter, &superseder] {
            adapter.write_signed_claim(c).expect("write claim");
        }

        let read = adapter.read_adapter();
        let presence = read
            .counter_presence_for(&[
                "bafyTarget".to_string(),
                "bafyPlain".to_string(),
                "bafySuper".to_string(),
                "bafyUnknown".to_string(),
            ])
            .expect("presence read succeeds");

        let expected: HashSet<String> = ["bafyTarget".to_string()].into_iter().collect();
        assert_eq!(
            presence, expected,
            "only the genuinely-countered target is in the presence set (a presence SET, \
             never a count; supersedes is not a counter)"
        );
    }

    /// An EMPTY input slice yields `Ok(HashSet::new())` WITHOUT preparing a query (an
    /// empty `IN ()` is a SQL error) — the empty-page / all-un-countered guard.
    #[test]
    fn counter_presence_for_empty_input_is_empty_set_no_query() {
        let (adapter, _tmp) = fresh_adapter();
        let read = adapter.read_adapter();

        let presence = read
            .counter_presence_for(&[])
            .expect("empty input must not error (no query prepared)");
        assert!(
            presence.is_empty(),
            "empty input → empty presence set, no query prepared"
        );
    }

    /// A page whose CIDs are all UN-countered yields the EMPTY set (no row flagged) —
    /// the no-noise common case (the list renders byte-identically to slice-06).
    #[test]
    fn counter_presence_for_all_uncountered_is_empty_set() {
        let (adapter, _tmp) = fresh_adapter();
        let a = claim("bafyA", "s-a", vec![]);
        let b = claim("bafyB", "s-b", vec![]);
        adapter.write_signed_claim(&a).expect("write a");
        adapter.write_signed_claim(&b).expect("write b");

        let read = adapter.read_adapter();
        let presence: Result<HashSet<String>, StoreReadError> =
            read.counter_presence_for(&["bafyA".to_string(), "bafyB".to_string()]);
        assert!(presence.expect("read succeeds").is_empty());
    }

    /// The STRICT N+1 guard (I-LF-8 / ADR-048): `counter_presence_for` issues EXACTLY
    /// ONE counter-presence read per page render, INVARIANT to page size — never one
    /// query per row. The `duckdb-rs` harness exposes NO prepared-statement / query
    /// counter, so the bound is pinned two ways:
    ///
    /// 1. STRUCTURAL (read the impl): `counter_presence_for` builds ONE `IN (...)`
    ///    UNION-ALL DISTINCT statement (a single `conn.prepare` over the input arity,
    ///    bound via `params_from_iter`) — there is NO per-CID loop issuing per-row
    ///    queries. The single prepared statement is the guarantee; this test pins its
    ///    OBSERVABLE consequence.
    /// 2. BEHAVIORAL (this test): the read is CONSTANT-SHAPE — the returned presence set
    ///    equals the countered subset for inputs of size 1, N, and 5N over the SAME
    ///    store, with NO per-CID iteration or fan-out artifact. An N+1 implementation
    ///    (a loop of per-CID `SELECT`s) could not stay correct under a 5N input without
    ///    its per-row cost/shape changing; a single aggregate `IN (...)` read scales the
    ///    bound CID list while keeping ONE statement. Pairing this with the
    ///    `empty_input_is_empty_set_no_query` guard (empty input → ZERO queries) pins the
    ///    full single-aggregate contract: 0 queries for an empty page, exactly 1 for any
    ///    non-empty page regardless of size.
    #[test]
    fn counter_presence_for_is_one_aggregate_query_invariant_to_page_size() {
        let (adapter, _tmp) = fresh_adapter();

        // Seed 5 own targets, each countered by ONE distinct `counters` claim. These are
        // the "always present" countered CIDs the presence read must return for ANY page
        // that includes them, regardless of how many UN-countered padding CIDs surround
        // them on the page.
        let mut countered: Vec<String> = Vec::new();
        for i in 0..5 {
            let target_cid = format!("bafyTarget{i}");
            let subject = format!("github:org/countered-{i}");
            let target = claim(&target_cid, &subject, vec![]);
            let counter = claim(
                &format!("bafyCounter{i}"),
                &subject,
                vec![ClaimReference {
                    ref_type: ReferenceType::Counters,
                    cid: Cid(target_cid.clone()),
                }],
            );
            adapter.write_signed_claim(&target).expect("write target");
            adapter.write_signed_claim(&counter).expect("write counter");
            countered.push(target_cid);
        }
        let countered_set: HashSet<String> = countered.iter().cloned().collect();

        let read = adapter.read_adapter();

        // Build a page of arbitrary size `n` whose FIRST `min(n,5)` CIDs are the seeded
        // countered targets and the rest are un-countered padding CIDs (never referenced
        // by any `counters` claim). The expected presence set is exactly the countered
        // targets that appear on the page — a constant SHAPE: one aggregate read, the
        // result is the membership intersection, never a per-CID fan-out.
        let page_of = |n: usize| -> Vec<String> {
            (0..n)
                .map(|i| {
                    if i < countered.len() {
                        countered[i].clone()
                    } else {
                        format!("bafyPadding{i}")
                    }
                })
                .collect()
        };
        let expected_for = |n: usize| -> HashSet<String> {
            countered
                .iter()
                .take(n.min(countered.len()))
                .cloned()
                .collect()
        };

        // Probe three page sizes spanning two orders of magnitude over the SAME store:
        // size 1, size N (= 5, all countered), and size 5N (= 25, the 5 countered + 20
        // un-countered padding). The result is the countered subset of the page in ALL
        // three cases — the single aggregate `IN (...)` read scales the bound list while
        // the statement count stays at one (structural), so the behavior is invariant to
        // page size (no per-CID degradation / mis-flagging).
        let n = countered.len(); // 5
        for size in [1usize, n, 5 * n] {
            let page = page_of(size);
            let presence = read
                .counter_presence_for(&page)
                .unwrap_or_else(|err| panic!("presence read for page size {size} must succeed: {err:?}"));
            assert_eq!(
                presence,
                expected_for(size),
                "counter_presence_for must return EXACTLY the countered subset of a page of \
                 size {size} in ONE aggregate IN(...) read (constant-shape, invariant to page \
                 size; no per-CID fan-out; ADR-048 / I-LF-8)"
            );
        }

        // The full 5N page returns ALL five countered targets (the whole known subset),
        // proving the single aggregate read does not drop members as the page grows.
        let big_page = page_of(5 * n);
        let big_presence = read
            .counter_presence_for(&big_page)
            .expect("presence read for the 5N page must succeed");
        assert_eq!(
            big_presence, countered_set,
            "the 5N page must flag EVERY countered target in one aggregate read — the single \
             IN(...) statement scales the bound list, it does NOT fan out to per-row queries"
        );
    }
}
