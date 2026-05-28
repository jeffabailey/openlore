# ADR-021: DuckDB Recursive-CTE Graph Traversal — the WD-8 Store Revisit Resolution (AUGMENT, not swap)

- **Status**: Proposed
- **Date**: 2026-05-28
- **Deciders**: Morgan (nw-solution-architect), resolving OD-GRAPH-1 / WD-8 / WD-78 for openlore-scoring-graph
- **Feature**: openlore-scoring-graph (slice-04)
- **Revisits / affirms**: ADR-001 (DuckDB single-file local storage). The slice-01/02/03 schema is UNCHANGED. This ADR ADDS recursive-CTE READ queries over the existing tables; it adds NO table and NO store.

## Context

The cross-feature brief locked WD-8: "DuckDB revisit at slice-04 when graph
traversal becomes the dominant workload." Slice-04's traversal
(contributor↔project↔philosophy) triggers the revisit. DISCUSS stayed
solution-neutral (WD-78) and framed the swap-vs-augment choice as DESIGN's
call (OD-GRAPH-1).

The choice: **SWAP/ADD a dedicated graph store** (e.g., an embedded Kùzu/KuzuDB,
or an in-memory `petgraph` layer) **vs AUGMENT** the existing `adapter-duckdb`
with recursive CTEs (`WITH RECURSIVE`).

DESIGN owns:

1. The swap-vs-augment decision (the headline slice-04 architectural call).
2. If augment: how the recursive CTE stays cycle-safe + depth-bounded.
3. The revisit trigger that would justify a future graph store.
4. The probe responsibilities for the AUGMENTED adapter.

## Decision

**AUGMENT `adapter-duckdb` with recursive-CTE graph traversal + attributed
scoring-feed queries over the SAME single-file store. Do NOT swap or add a
graph store.** Recorded as WD-81.

### Trade-off analysis

| Factor | AUGMENT DuckDB (recursive CTE) — **CHOSEN** | SWAP/ADD a graph store |
|---|---|---|
| Traversal workload (slice-04) | Low-thousands of claims/install; bounded default depth 2 (WD-76). Recursive CTE handles this in a columnar scan. | Over-provisioned; a graph store's advantages appear at 100k+ edges / deep unbounded walks that WD-76 explicitly avoids. |
| Query complexity | A bounded bipartite-ish walk over claim rows; a `WITH RECURSIVE` over `claims ∪ peer_claims` with a visited-set + depth column. Moderate, well-understood SQL. | Cypher is more ergonomic for DEEP traversal, but slice-04's traversal is shallow + bounded; the ergonomics gain is marginal. |
| Dependency cost | ZERO new production dependency. Recursive CTEs are built into DuckDB (already pinned). No `cargo deny` change (I-11). | New embedded-graph crate = new dependency + license review + `cargo deny` entry + new adapter crate + a SECOND store file + a claims↔graph sync problem. |
| Probe / Earned Trust cost | Extend the existing `adapter-duckdb` probe (one new scenario: CTE cycle/termination + depth bound). I-4/I-5 already satisfied. | A new adapter needs its own `probe()` (I-4), its own substrate gold-tests, AND a sync-consistency probe between the two stores. |
| Local-first simplicity | One embedded file unchanged (one backup target; the P-002 operational model). | Two stores break single-file simplicity; a sync layer is a new failure mode. |
| Anti-merging enforcement | The slice-03 `no_cross_table_join_elides_author` xtask SQL-string rule EXTENDS naturally to the new CTE queries (same crate, same string-literal pass). | A non-SQL store needs a NEW structural enforcement mechanism for anti-merging — re-inventing the slice-03 guarantee on a new substrate. |
| Future scale | Revisit trigger documented (below); reversible with data. | Premature now; the right call only when evidence shows CTEs are the bottleneck. |

### Cycle safety (the substrate-lie this ADR confronts head-on)

DuckDB recursive CTEs do **NOT** auto-detect cycles. An unbounded
`WITH RECURSIVE` over a cyclic claim graph (A→B→A via two signed claims) loops
forever. The design REFUSES to trust the substrate's good behavior and bounds
+ dedupes explicitly:

1. **Depth bound** (WD-76): a `depth` column; the recursive step's `WHERE
   depth < ?max_depth` (default 2; `--depth K` override).
2. **Visited-set cycle guard**: a delimited visited-path string (or equivalent)
   carried through the recursion; the recursive step excludes any
   already-visited `claim_cid` (each claim edge expands once).
3. **Probe**: `adapter-duckdb` probe #2 builds a cyclic A↔B fixture, runs
   `traverse_graph` at depth 3, and asserts (a) termination within the 250ms
   budget (I-5), (b) each edge emitted exactly once.

This is the slice-04 manifestation of the Earned Trust principle: *every
dependency you don't probe is an act of faith*. The SQL engine will loop
forever on a cyclic graph; the design proves empirically that the bounded,
deduped CTE terminates in the real DuckDB it runs on.

### What is AUGMENTED (read-only; no schema change)

- `query_by_object(object)` / `query_by_contributor(author_did)` — `UNION ALL`
  attributed projections over `claims` + `peer_claims` (the slice-03
  anti-merging SQL discipline, new dimensions).
- `query_attributed_for_scoring(filter)` — the per-claim attributed feed for
  the pure `scoring` core. **Aggregation happens in Rust, NEVER in SQL** (the
  SQL returns per-claim rows; it never `SUM`/`GROUP BY`s across authors — this
  is what keeps the aggregate decomposable; I-GRAPH-2).
- `traverse_graph(start, bound)` — the cycle-safe, depth-bounded recursive CTE;
  every output row is a `GraphEdge` carrying its backing `claim_cid` (Gate 5)
  and `author_did` (anti-merging).

No new table. No new on-disk artifact. No store. The schema is exactly
slice-03's (`claims`, `peer_claims`, the slice-03 peer tables, unchanged).

## Alternatives Considered

| Option | Rejection rationale |
|---|---|
| **Embedded graph DB (Kùzu / KuzuDB) as a second store** | The "right long-term home" for deep unbounded traversal, but premature at slice-04 scale (bounded depth 2, low-thousands of claims). New dependency + adapter + probe + second backup target + claims↔graph sync + a new anti-merging enforcement substrate. Large cost for marginal benefit. Documented as the revisit target. |
| **In-memory `petgraph` adjacency built per query** | The edges ARE the claim rows; materializing a separate in-memory graph duplicates them and risks the invented-edge failure mode (an edge with no backing claim, Gate 5 violation). The recursive CTE derives edges from the authoritative rows on demand. |
| **Application-layer recursion in Rust (iterative `query_by_*` calls)** | Pushes the traversal into the `cli` driver, multiplying round-trips and moving the cycle/depth logic out of the storage boundary where the anti-merging SQL rule can see it. The CTE keeps traversal in the adapter where `xtask check-arch` enforces attribution. |
| **Unbounded recursive CTE (trust DuckDB to terminate)** | Loops forever on a cyclic graph (DuckDB does not auto-detect cycles). Locked rejected; bounded + visited-guarded + probed. |
| **Materialized adjacency table refreshed on write** | Adds a persisted table (slice-04 adds none, WD-72-adjacent simplicity) + a refresh-on-write hook (slice-04 is read-only, WD-79). The on-demand CTE has no staleness. |

## Consequences

### Positive

- Zero new dependency; single-file local-first simplicity preserved; one
  backup target (the P-002 operational model).
- The slice-03 anti-merging SQL-string xtask rule extends onto the same SQL
  substrate for free — the new CTE/UNION-ALL queries are checked by the same
  `no_cross_table_join_elides_author` pass.
- The decision is reversible with data: the revisit trigger is explicit.
- The cycle-safety probe makes the substrate's "will loop forever" behavior a
  caught, tested property rather than a production surprise.

### Negative

- Recursive-CTE cost grows with graph density; a pathologically dense local
  graph at depth >2 could approach the KPI-GRAPH-6 latency budget.
  **Mitigation**: bounded default depth 2; omitted-edge reporting; the revisit
  trigger toward a graph store if the ≤200-claim P95 is breached in dogfeed.
- The visited-set-as-string cycle guard is a SQL-level workaround for the
  absence of native cycle detection; a future DuckDB version with native
  cycle handling could simplify it. **Mitigation**: the contract (terminate +
  dedupe) is probe-enforced; the exact SQL is DELIVER's and can evolve.
- A reader of the SQL must understand the visited-path encoding. **Mitigation**:
  a comment in the migration/query file citing this ADR; the probe documents
  the expected behavior.

### Earned Trust

The `adapter-duckdb::probe()` (extended for slice-04) MUST exercise:

1. **Scoring-feed attribution round-trip**: write 1 own + 2 peer claims on the
   same (subject, object) by three distinct authors; call
   `query_attributed_for_scoring(ByObject)`; assert exactly 3 `AttributedClaim`s
   with three distinct, non-empty `author_did`s.
2. **Recursive-CTE termination on a cyclic fixture**: A↔B cyclic graph; depth 3;
   assert termination within 250ms (I-5) + each edge once.
3. **Depth-bound honored**: depth-4 fixture; `max_depth=2`; assert only
   ≤depth-2 edges + `omitted_edge_count > 0`.
4. fsync + schema-version probes inherited from ADR-001/014 (unchanged).

These extend the existing `StoragePort` probe; no new port, no new adapter, so
no new probe surface beyond this extension (I-4 already satisfied for the
adapter).

## Revisit Trigger

Re-evaluate a dedicated graph store (Kùzu/KuzuDB or similar) when ANY of:

- Dogfeed shows the `graph.query.duration_seconds` P95 breaches the
  KPI-GRAPH-6 budget for the ≤200-claim bucket (the recursive CTE is the
  bottleneck, with evidence).
- `peer_claims` exceeds ~100k rows in a single install (matches ADR-014's own
  revisit trigger; the columnar scan degrades).
- A JTBD emerges for UNBOUNDED deep traversal (the bounded-depth-2 model no
  longer serves the exploration need).
- A second federated source / non-claim edge type emerges that does not map to
  a signed-claim row (the "edges = claims" model breaks).

The decision is deliberately reversible: AUGMENT now keeps the door open to a
graph store later, with data, without committing to the migration prematurely.
