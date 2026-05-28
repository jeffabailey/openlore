//! `graph_query` — slice-04 recursive-CTE traversal + attributed scoring-feed
//! reads for `DuckDbStorageAdapter` (component-boundaries.md §`crates/adapter-duckdb`).
//!
//! These are the EFFECT-shell bodies for the four slice-04 `StoragePort` read
//! methods (`query_by_object`, `query_by_contributor`,
//! `query_attributed_for_scoring`, `traverse_graph`). They AUGMENT the existing
//! single-file DuckDB store (WD-8): NO new table, NO store swap, NO new
//! dependency. The cross-store reads use `UNION ALL` with explicit `author_did`
//! (NEVER a merging `JOIN`/`GROUP BY` — `xtask check-arch`'s extended
//! `no_cross_table_join_elides_author` enforces it on every SQL literal in this
//! crate). Aggregation (the weight) happens later in the pure `scoring` core in
//! Rust, NEVER in SQL (WD-73 / I-GRAPH-2).
//!
//! ## Query shapes (data-models.md — live SQL lands per-scenario in Phase 03/04/05)
//!
//! - dimension / scoring-feed: `SELECT author_did, cid, subject, predicate,
//!   object, confidence, composed_at FROM claims WHERE … UNION ALL SELECT …
//!   FROM peer_claims WHERE …` — one attributed row per claim, `author_did`
//!   projected (the renderer / pure scoring core aggregates).
//! - traversal: a `WITH RECURSIVE` CTE whose `edges_base` UNION-ALLs both
//!   stores projecting `author_did` + `cid AS claim_cid`, bounded by a `depth`
//!   column (WD-76) AND guarded by a delimited `visited` path string
//!   (`NOT LIKE '%|' || claim_cid || '|%'`) so it terminates on a cyclic graph
//!   (ADR-021 — DuckDB recursive CTEs do NOT auto-detect cycles). Every output
//!   row maps to exactly ONE signed claim (`claim_cid` non-`Option`, Gate 5).
//!
//! ## SCAFFOLD status
//!
//! SCAFFOLD: true (slice-04) — the bodies are `todo!()`; the live recursive-CTE
//! / scoring-feed SQL + per-row → `AttributedClaim` / `GraphEdge` projection
//! lands per-scenario in Phase 03/04/05, driven by the dimension / weighted /
//! traversal acceptance tests. The signatures + the SAFE query shapes above are
//! the contract these bodies will satisfy.

use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use chrono::{DateTime, Utc};
use claim_domain::{Cid, Did};
use duckdb::Connection;
use ports::{
    AttributedClaim, AuthorRelationship, GraphNode, ScoringFilter, StorageError, TraversalBound,
    TraversalResult,
};

use crate::bare_did;

/// Which claims assert this `object` (philosophy), across own + peer stores.
/// SAFE cross-store `UNION ALL` projecting `author_did` (I-GRAPH-2): two
/// identical-content claims by different authors stay TWO rows (never merged).
///
/// The query mirrors `query_federated_by_subject` (slice-03) but filters on
/// `object` and returns the lighter [`AttributedClaim`] projection the pure
/// scoring core + dimension renderer consume (no on-disk artifact re-read —
/// every field the renderer shows is already a column). The literal names BOTH
/// `claims` AND `peer_claims` AND projects `author_did`, so it passes
/// `xtask check-arch::no_cross_table_join_elides_author` (I-FED-1 / WD-73);
/// aggregation (the weight) is the pure `scoring` core's job, NEVER SQL.
pub(crate) fn query_by_object(
    conn: &Arc<Mutex<Connection>>,
    object: &str,
) -> Result<Vec<AttributedClaim>, StorageError> {
    query_attributed_dimension(conn, &DimensionFilter::Object(object.to_string()))
}

/// The dimension on which a per-claim attributed read filters. Both the
/// `--object` and `--contributor` dimension reads share the SAME UNION-ALL
/// projection (only the `WHERE` column differs), so they decompose to one
/// parameterized query rather than two near-identical SQL literals.
enum DimensionFilter {
    Object(String),
    Contributor(Did),
}

impl DimensionFilter {
    /// The fully-qualified `WHERE`-column for each store side. Own claims store
    /// the `#fragment` signing locator on `author_did`; the contributor filter
    /// therefore matches the bare DID via a `LIKE '<bare>%'` prefix so a query
    /// by the bare contributor DID still finds the locator-suffixed own rows.
    fn where_clause(&self) -> (&'static str, &'static str) {
        match self {
            DimensionFilter::Object(_) => ("c.object = ?", "pc.object = ?"),
            DimensionFilter::Contributor(_) => ("c.author_did LIKE ?", "pc.author_did LIKE ?"),
        }
    }

    /// The bound parameter value for the `WHERE` clause (same on both sides).
    fn param(&self) -> String {
        match self {
            DimensionFilter::Object(object) => object.clone(),
            DimensionFilter::Contributor(did) => format!("{}%", bare_did(&did.0)),
        }
    }
}

/// One raw per-claim row of the cross-store UNION-ALL projection, before the
/// `You | SubscribedPeer | UnsubscribedCache` relationship is resolved.
struct DimensionProjection {
    author_did: String,
    cid: String,
    subject: String,
    predicate: String,
    object: String,
    confidence: f64,
    composed_at: DateTime<Utc>,
    source_table: String,
}

/// Read per-claim attributed rows for a dimension filter from BOTH stores via a
/// SAFE `UNION ALL` (explicit `author_did` projection — anti-merging), then
/// resolve each Peer row's subscription relationship. Returns one
/// [`AttributedClaim`] per signed claim (never a SQL aggregate).
fn query_attributed_dimension(
    conn: &Arc<Mutex<Connection>>,
    filter: &DimensionFilter,
) -> Result<Vec<AttributedClaim>, StorageError> {
    let (own_where, peer_where) = filter.where_clause();
    let param = filter.param();

    // `peer_subscriptions.removed_at IS NULL` ⇒ SubscribedPeer; else
    // UnsubscribedCache (soft-remove residue, ADR-014). Read once.
    let active_peers = active_subscription_dids(conn)?;

    let sql = format!(
        "SELECT author_did, cid, subject, predicate, object, confidence, composed_at, source_table \
         FROM ( \
           SELECT c.author_did AS author_did, c.cid AS cid, c.subject AS subject, \
                  c.predicate AS predicate, c.object AS object, c.confidence AS confidence, \
                  c.composed_at AS composed_at, 'Own' AS source_table \
           FROM claims c \
           WHERE {own_where} \
           UNION ALL \
           SELECT pc.author_did AS author_did, pc.cid AS cid, pc.subject AS subject, \
                  pc.predicate AS predicate, pc.object AS object, pc.confidence AS confidence, \
                  pc.composed_at AS composed_at, 'Peer' AS source_table \
           FROM peer_claims pc \
           WHERE {peer_where} \
         ) ORDER BY subject, source_table, cid"
    );

    let projections: Vec<DimensionProjection> = {
        let conn = conn.lock().map_err(|_| StorageError::QueryFailed {
            message: "connection mutex poisoned".to_string(),
        })?;
        let mut stmt = conn
            .prepare(&sql)
            .map_err(|err| StorageError::QueryFailed {
                message: format!("prepare query_attributed_dimension: {err}"),
            })?;
        let rows = stmt
            .query_map(duckdb::params![param, param], |row| {
                Ok(DimensionProjection {
                    author_did: row.get::<_, String>(0)?,
                    cid: row.get::<_, String>(1)?,
                    subject: row.get::<_, String>(2)?,
                    predicate: row.get::<_, String>(3)?,
                    object: row.get::<_, String>(4)?,
                    confidence: row.get::<_, f64>(5)?,
                    composed_at: row.get::<_, DateTime<Utc>>(6)?,
                    source_table: row.get::<_, String>(7)?,
                })
            })
            .map_err(|err| StorageError::QueryFailed {
                message: format!("query_map dimension: {err}"),
            })?;
        let mut collected = Vec::new();
        for row in rows {
            collected.push(row.map_err(|err| StorageError::QueryFailed {
                message: format!("row decode dimension: {err}"),
            })?);
        }
        collected
    };

    projections
        .into_iter()
        .map(|projection| attributed_claim_from(projection, &active_peers))
        .collect()
}

/// Resolve one raw projection into an [`AttributedClaim`], normalizing the
/// author DID to its bare form and classifying the relationship.
fn attributed_claim_from(
    projection: DimensionProjection,
    active_peers: &HashSet<String>,
) -> Result<AttributedClaim, StorageError> {
    let author_did = bare_did(&projection.author_did);
    let relationship = match projection.source_table.as_str() {
        "Own" => AuthorRelationship::You,
        "Peer" => {
            if active_peers.contains(&author_did) {
                AuthorRelationship::SubscribedPeer
            } else {
                AuthorRelationship::UnsubscribedCache
            }
        }
        other => {
            return Err(StorageError::QueryFailed {
                message: format!("unknown source_table {other:?} in dimension read"),
            })
        }
    };

    Ok(AttributedClaim {
        author_did: Did(author_did),
        cid: Cid(projection.cid),
        subject: projection.subject,
        predicate: projection.predicate,
        object: projection.object,
        confidence: projection.confidence,
        composed_at: projection.composed_at,
        relationship,
    })
}

/// The set of DIDs with a currently-ACTIVE peer subscription
/// (`removed_at IS NULL`). Mirrors the adapter's own `active_subscription_dids`
/// (the helper takes the shared connection directly so it can run inside the
/// `graph_query` effect shell without a `&self`).
fn active_subscription_dids(
    conn: &Arc<Mutex<Connection>>,
) -> Result<HashSet<String>, StorageError> {
    let conn = conn.lock().map_err(|_| StorageError::QueryFailed {
        message: "connection mutex poisoned".to_string(),
    })?;
    let mut stmt = conn
        .prepare("SELECT peer_did FROM peer_subscriptions WHERE removed_at IS NULL")
        .map_err(|err| StorageError::QueryFailed {
            message: format!("prepare active_subscription_dids: {err}"),
        })?;
    let rows = stmt
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|err| StorageError::QueryFailed {
            message: format!("query active_subscription_dids: {err}"),
        })?;
    let mut dids = HashSet::new();
    for row in rows {
        dids.insert(row.map_err(|err| StorageError::QueryFailed {
            message: format!("row decode active_subscription_dids: {err}"),
        })?);
    }
    Ok(dids)
}

/// Every claim authored by this DID, across all subjects, own + peer stores.
/// Drives `--contributor` (one developer's reasoning trail).
///
/// Decomposes to the SAME SAFE cross-store `UNION ALL` projection
/// [`query_by_object`] uses (via [`query_attributed_dimension`]) — only the
/// `WHERE` column differs ([`DimensionFilter::Contributor`] matches
/// `author_did LIKE '<bare>%'` so a query by the bare contributor DID also
/// finds the `#fragment`-suffixed own rows). `author_did` is projected per
/// claim (NEVER a merging `JOIN`/`GROUP BY`): two claims by this author about
/// the same `(subject, object)` stay TWO rows, and no row leaks under any
/// OTHER DID — `xtask check-arch::no_cross_table_join_elides_author` enforces
/// it (I-GRAPH-2 / WD-73). Aggregation (the weight) is the pure `scoring`
/// core's job later, NEVER SQL.
pub(crate) fn query_by_contributor(
    conn: &Arc<Mutex<Connection>>,
    author_did: &Did,
) -> Result<Vec<AttributedClaim>, StorageError> {
    query_attributed_dimension(conn, &DimensionFilter::Contributor(author_did.clone()))
}

/// The attributed-claim feed for the pure `scoring::score` core. Returns
/// per-claim rows (the same `UNION ALL` shape as [`query_by_object`]), NEVER a
/// SQL aggregate — so the weight the pure core computes always decomposes into
/// these rows (Gate 1 / I-GRAPH-2 / WD-73).
///
/// SCAFFOLD: true (slice-04) — live SQL lands with the `--weighted` scoring
/// acceptance scenario (Phase 04).
pub(crate) fn query_attributed_for_scoring(
    _conn: &Arc<Mutex<Connection>>,
    _filter: &ScoringFilter,
) -> Result<Vec<AttributedClaim>, StorageError> {
    todo!(
        "slice-04 Phase 04: per-claim UNION ALL by filter (Object/Subject/Contributor), \
         NO SQL aggregation — the pure scoring core aggregates"
    )
}

/// Bounded, cycle-safe traversal of contributor↔project↔philosophy edges from
/// `start`, capped at `bound.max_depth` (WD-76). The recursive CTE selects FROM
/// existing rows only (Gate 5 — invents no edge), is depth-bounded by a `depth`
/// column AND visited-set-guarded by a delimited `visited` path string so it
/// terminates on a cyclic graph (ADR-021).
///
/// SCAFFOLD: true (slice-04) — live recursive-CTE SQL + omitted-edge count
/// lands with the `--traverse` acceptance scenario + the cycle-safety probe
/// (Phase 05).
pub(crate) fn traverse_graph(
    _conn: &Arc<Mutex<Connection>>,
    _start: &GraphNode,
    _bound: &TraversalBound,
) -> Result<TraversalResult, StorageError> {
    todo!(
        "slice-04 Phase 05: WITH RECURSIVE edges_base (UNION ALL claims+peer_claims, \
         project author_did + cid AS claim_cid) + depth-bounded visited-guarded walk"
    )
}
