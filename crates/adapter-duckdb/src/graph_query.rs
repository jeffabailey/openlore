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

use std::sync::{Arc, Mutex};

use claim_domain::Did;
use duckdb::Connection;
use ports::{
    AttributedClaim, GraphNode, ScoringFilter, StorageError, TraversalBound, TraversalResult,
};

/// Which claims assert this `object` (philosophy), across own + peer stores.
/// SAFE cross-store `UNION ALL` projecting `author_did` (I-GRAPH-2): two
/// identical-content claims by different authors stay TWO rows (never merged).
///
/// SCAFFOLD: true (slice-04) — live SQL lands with the `--object` dimension
/// acceptance scenario (Phase 03).
pub(crate) fn query_by_object(
    _conn: &Arc<Mutex<Connection>>,
    _object: &str,
) -> Result<Vec<AttributedClaim>, StorageError> {
    todo!("slice-04 Phase 03: UNION ALL claims+peer_claims WHERE object=?, project author_did")
}

/// Every claim authored by this DID, across all subjects, own + peer stores.
/// Drives `--contributor` (one developer's reasoning trail).
///
/// SCAFFOLD: true (slice-04) — live SQL lands with the `--contributor`
/// dimension acceptance scenario (Phase 03).
pub(crate) fn query_by_contributor(
    _conn: &Arc<Mutex<Connection>>,
    _author_did: &Did,
) -> Result<Vec<AttributedClaim>, StorageError> {
    todo!(
        "slice-04 Phase 03: UNION ALL claims+peer_claims WHERE author_did=?, project author_did"
    )
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
