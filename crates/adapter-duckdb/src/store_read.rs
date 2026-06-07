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
    PeerClaimRow, PeerOrigin, StoreReadError, StoreReadPort, SurveyRow,
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
    /// `fs::read`, LOCAL only, no network. A missing/undeserializable artifact
    /// surfaces as [`StoreReadError::QueryFailed`] (never a panic).
    fn read_reason_at(&self, artifact_path: &str) -> Result<Option<String>, StoreReadError> {
        let resolved: PathBuf = match artifact_path.strip_prefix("peer_claims/") {
            Some(relative) => self.peer_claims_root.join(relative),
            None => PathBuf::from(artifact_path),
        };
        let bytes = fs::read(&resolved).map_err(|err| StoreReadError::QueryFailed {
            detail: format!("read counter artifact {}: {err}", resolved.display()),
        })?;
        let signed: SignedClaim =
            serde_json::from_slice(&bytes).map_err(|err| StoreReadError::QueryFailed {
                detail: format!("deserialize counter artifact {}: {err}", resolved.display()),
            })?;
        Ok(signed.unsigned.reason)
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
        // column — ADR-015). LOCAL `fs::read`, no network. A blank `author_did`
        // (defensive, bypassing the slice-03 CHECK) projects to `Unknown` so the
        // viewer labels rather than drops it.
        let mut counters = Vec::with_capacity(staged.len());
        for (author_did, cid, confidence, composed_at, fetched_from_pds, artifact_path) in staged {
            let reason = self.read_reason_at(&artifact_path)?;
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
