# Data Models — openlore-scoring-graph (slice-04) — DELTA from slice-03

- **Wave**: DESIGN
- **Date**: 2026-05-28
- **Architect**: Morgan
- **Authoritative for**: the slice-04 DERIVED, DISPLAY-ONLY scoring/weight types (`AttributedClaim`, `Contribution`, `WeightedPairing`, `WeightedView`, `WeightBucket`); the read-side query SHAPES (dimension `UNION ALL`, scoring-feed projection, the recursive-CTE traversal); the worked formula arithmetic. **No new persisted table** (WD-72 — weights are never stored). **No Lexicon change** (nothing new is signed).
- **Extends**: `docs/feature/openlore-federated-read/design/data-models.md` (slice-03 peer schema, inherited unchanged)

## The single most important data fact of slice-04

**Slice-04 adds NO persisted data.** No new DuckDB table. No new on-disk
artifact. No Lexicon field. No record. The `adherence_weight` and
`weight_bucket` are DERIVED at query time and exist ONLY in the render layer
(WD-72, extending the WD-10 / I-6 display-only-bucket discipline). The slice
is a READ slice: it computes views over the slice-01 `claims` + slice-03
`peer_claims` tables (which include slice-02 scraper-signed claims, stored as
normal author claims in `claims`).

Everything below is therefore either (a) an in-memory value type that lives
only during a query, or (b) a query SHAPE (SQL read) over the EXISTING
schema. The persisted schema is exactly slice-03's, unchanged.

## In-memory value types (live only during a query; NEVER persisted)

These are the slice-04 ADTs. The persistence column for each is "NONE —
display-only" by design (WD-72). They mirror the slice-03 `FederatedRow`
non-`Option<Did>` discipline that makes attribution unviolatable.

| Type | Where defined | Persisted? | Purpose |
|---|---|---|---|
| `AttributedClaim` | `ports` | NO — read into memory from `claims`/`peer_claims`, never written back | The fully-attributed claim feed; the boundary value the pure `scoring` core consumes. `author_did` non-`Option` (Gate 1). |
| `Contribution` | `scoring` | NO — computed per query | One claim's contribution to a weight; the auditable unit `--explain` renders. Carries `author_did` + `cid` + `subtotal`. |
| `WeightedPairing` | `scoring` | NO — computed per query | One ranked (subject, object) with its `weight`, display `bucket`, and NON-EMPTY `contributions` (decomposes by construction — anti-merging in aggregates). |
| `WeightedView` | `scoring` | NO — computed per query | The ranked list returned by `score()`. |
| `WeightBucket` | `scoring` | NO — display-only label | `Strong | Moderate | Sparse`; driven by `(weight, claim_count, distinct_author_count)`. |
| `GraphEdge` / `TraversalResult` | `ports` | NO — computed per query | A traversal edge backed by exactly one `claim_cid` (Gate 5); the bounded traversal result. |

### The display-only invariant, made concrete

```
property: after `graph query --weighted [--explain] [--traverse]`:
    no row in any DuckDB table contains a column named adherence_weight / weight_bucket
    no <cid>.json (claims/ or peer_claims/) contains a weight or bucket field
    no PDS record is written (read-only slice; WD-79)
    re-running the SAME query after `openlore peer pull` MAY produce a DIFFERENT weight
        (proves the weight is computed at query time, not stored)
```

Enforced by Gate 4 (`weight_and_bucket_never_persisted`) + the slice-01/03
confidence-bucket no-persist unit test extended to scan for
`STRONG|MODERATE|SPARSE` and `adherence_weight` substrings in every table and
every on-disk artifact.

## The scoring formula — worked arithmetic (the transparency contract)

The formula (WD-77; constants in `scoring::ScoringConfig::DEFAULT`, the SSOT):

```
Constants (WD-77 defaults; DESIGN-tunable; small/closed-form/no-ML):
    author_distinct_bonus              = 0.25   (per additional distinct author on the SAME (subject, object))
    cross_project_triangulation_bonus  = 0.50   (when the SAME author asserts the object on >=2 distinct subjects)

Per (subject, object) pairing, for each contributing claim c authored by A:
    base(c)            = c.confidence                                   # numeric [0.0,1.0] (Gate 6)
    distinct_authors   = count of distinct author_did on this (subject, object)
    author_multiplier  = 1.0 + author_distinct_bonus * (distinct_authors - 1)
    triangulation(c)   = cross_project_triangulation_bonus  if A asserts `object` on >=2 distinct subjects else 0.0
    subtotal(c)        = base(c) * author_multiplier_share(c) + triangulation(c)

    weight(subject, object) = sum over contributing claims c of subtotal(c)   # == sum of subtotals (Gate 2)
```

DELIVER fixes `author_multiplier_share(c)` so the per-claim subtotals sum
EXACTLY to the displayed weight (the `--explain` running-sum must reproduce it
by hand). The example below is the worked target from US-GRAPH-005 Example 1.

### Worked example (US-GRAPH-005 Example 1 — deno, object = dependency-pinning)

```
deno has 2 contributing claims for dependency-pinning:
    Tobias  (bafy...d3no, conf 0.55)  -> base 0.55, author bonus 1.0 (first author), no triangulation -> subtotal 0.55
    Maria   (bafy...mz01, conf 0.40)  -> base 0.40, +0.25 second-author bonus (x1.25), no triangulation -> subtotal 0.50
    weight(deno, dependency-pinning) = 0.55 + 0.50 = 1.05
    distinct_authors = 2, claim_count = 2  -> bucket = Moderate (not Sparse: breadth >1)
```

### Worked example (triangulation — cargo, US-GRAPH-005 Example 4)

```
cargo has 1 contributing claim for dependency-pinning, by Rachel (conf 0.91):
    Rachel also asserts dependency-pinning on nixpkgs (a 2nd distinct subject)
        -> cross_project_triangulation_bonus +0.50 applies to Rachel's cargo contribution
    base 0.91, author bonus 1.0 (single author on cargo), +0.50 triangulation -> subtotal 1.41
    weight(cargo, dependency-pinning) = 1.41
    distinct_authors = 1, claim_count = 1
    -> BUT the breadth guard: claim_count <= 1 OR distinct_authors <= 1 -> Sparse?
```

**Bucket nuance (DELIVER + DISTILL note)**: the breadth guard (WD-74) buckets
a single-author-single-claim pairing as `[SPARSE]` even when triangulation
raised its numeric weight. US-GRAPH-003 Example 1 narrates cargo as `[STRONG]`
"boosted by Rachel spanning cargo+nixpkgs" — that narrative treats Rachel's
cross-project span as the breadth that lifts cargo out of sparse. DELIVER MUST
pick ONE consistent rule and DISTILL asserts it; the DESIGN constraint is:
**cross-project triangulation by the same author counts toward evidence
breadth for the bucket** (so cargo+nixpkgs span -> not sparse), while a single
claim with NO triangulation and NO co-author stays `[SPARSE]` regardless of
confidence magnitude. Flagged as Q-DELIVER-SCORE-1 (and a `# DISTILL: confirm`
in the expanded gherkin) so the bucket rule is locked against the worked
examples before tests are written.

## Read-side query shapes (over the EXISTING slice-03 schema)

All slice-04 queries read the slice-01 `claims` + slice-03 `peer_claims`
tables. Every cross-store read uses `UNION ALL` with explicit `author_did`
projection (NEVER a `JOIN` that elides it) — exactly the slice-03 anti-merging
SQL discipline (I-FED-1), now extended to scoring/traversal (I-GRAPH-2).

### Dimension query — by object (philosophy)

```sql
-- query_by_object(?object): SAFE pattern (UNION ALL, explicit author_did)
SELECT c.author_did, c.cid, c.subject, c.predicate, c.object,
       c.confidence, c.composed_at, c.artifact_path, 'Own' AS source_table
FROM claims c
WHERE c.object = ?object
UNION ALL
SELECT pc.author_did, pc.cid, pc.subject, pc.predicate, pc.object,
       pc.confidence, pc.composed_at, pc.signed_record_path, 'Peer' AS source_table
FROM peer_claims pc
WHERE pc.object = ?object;
-- Renderer groups by subject; each row -> one AttributedClaim (author_did non-Option).
```

### Dimension query — by contributor (DID)

```sql
-- query_by_contributor(?author_did): SAFE pattern
SELECT c.author_did, c.cid, c.subject, c.predicate, c.object,
       c.confidence, c.composed_at, c.artifact_path, 'Own' AS source_table
FROM claims c
WHERE c.author_did = ?author_did
UNION ALL
SELECT pc.author_did, pc.cid, pc.subject, pc.predicate, pc.object,
       pc.confidence, pc.composed_at, pc.signed_record_path, 'Peer' AS source_table
FROM peer_claims pc
WHERE pc.author_did = ?author_did;
-- Relationship label (you/subscribed-peer/unsubscribed-cache) is resolved by joining
-- the Peer-sourced rows to peer_subscriptions WHERE removed_at IS NULL (slice-03 reuse).
```

### Scoring-feed query — attributed claims for the pure core

```sql
-- query_attributed_for_scoring(ByObject{?object}): the same UNION ALL as query_by_object.
-- The result Vec<AttributedClaim> is passed to scoring::score(claims, cfg).
-- CRITICAL (I-GRAPH-2): aggregation (the weight) happens in the PURE scoring core in Rust,
-- NOT in SQL. The SQL returns per-claim attributed rows; it NEVER SUM()s or GROUP BYs across
-- authors. This is what keeps the aggregate decomposable — the contributions exist as rows.
```

**FORBIDDEN pattern** (would merge in SQL, hiding attribution — caught by
`xtask check-arch`):

```sql
-- DO NOT: aggregate the weight in SQL across authors. Flagged because it touches both
-- claims and peer_claims (via a UNION subquery) and the outer GROUP BY drops author_did.
SELECT subject, object, SUM(confidence) AS faux_weight
FROM (SELECT subject, object, confidence FROM claims
      UNION ALL SELECT subject, object, confidence FROM peer_claims)
GROUP BY subject, object;   -- author_did eliminated -> anti-merging violation
```

### Traversal query — bounded, cycle-safe recursive CTE (ADR-021)

The traversal walks contributor↔project↔philosophy edges, where each edge is
ONE signed claim. The recursive CTE is the WD-8-resolution mechanism (AUGMENT
DuckDB; no graph store). It MUST be **depth-bounded** (WD-76) and
**cycle-safe** (DuckDB recursive CTEs do NOT auto-detect cycles — the design
refuses to trust the substrate; ADR-021).

```sql
-- traverse_graph(start, max_depth): illustrative shape; DELIVER fixes exact SQL (Q-DELIVER #1).
-- Cycle safety via a visited-path string; depth bound via a depth column.
WITH RECURSIVE edges_base AS (
    -- one row per signed claim = one edge between (author_did, subject, object)
    SELECT author_did, cid AS claim_cid, subject, object FROM claims
    UNION ALL
    SELECT author_did, cid AS claim_cid, subject, object FROM peer_claims
),
walk AS (
    -- seed: edges incident to the start node (philosophy / project / contributor)
    SELECT e.author_did, e.claim_cid, e.subject, e.object,
           1 AS depth,
           '|' || e.claim_cid || '|' AS visited      -- visited-set as a delimited path
    FROM edges_base e
    WHERE /* e matches the start GraphNode */ TRUE

    UNION ALL

    -- step: expand to adjacent edges, BOUNDED by depth and GUARDED against revisiting a claim
    SELECT e.author_did, e.claim_cid, e.subject, e.object,
           w.depth + 1,
           w.visited || e.claim_cid || '|'
    FROM walk w
    JOIN edges_base e
      ON (e.author_did = w.author_did OR e.subject = w.subject OR e.object = w.object)
    WHERE w.depth < ?max_depth                        -- DEPTH BOUND (WD-76)
      AND w.visited NOT LIKE '%|' || e.claim_cid || '|%'  -- CYCLE GUARD (each claim once)
)
SELECT DISTINCT author_did, claim_cid, subject, object, depth FROM walk;
-- Every output row -> one GraphEdge with claim_cid (Gate 5) + author_did (anti-merging).
-- omitted_edge_count is computed by a parallel COUNT at depth = max_depth+1 boundary.
```

Cycle-safety + termination is exercised by `adapter-duckdb` probe #2 (a cyclic
A↔B fixture at depth 3 MUST terminate within the 250ms budget and emit each
edge once). This is the slice-04 "what happens if the substrate lies" check:
the SQL engine will loop forever on a cyclic graph without the visited guard;
the design bounds + dedupes explicitly rather than trusting the engine.

## Why no new persisted table (the WD-72 contract restated)

| Candidate to persist | Decision | Rationale |
|---|---|---|
| `adherence_weight` per (subject, object) | NOT persisted | Would go stale relative to the claims it summarizes; would tempt federation of a derived value (the aggregator failure J-002 distrusts). Computed at query time. (WD-72) |
| `weight_bucket` label | NOT persisted | Extends the WD-10 confidence-bucket display-only discipline. A bucket string in any store is a CI-failable invariant (Gate 4). |
| traversal edges / adjacency | NOT persisted | The edges ARE the signed claims (`claims`/`peer_claims` rows); a separate adjacency table would duplicate them and risk drift (an edge with no backing claim = invented edge, Gate 5 violation). The recursive CTE derives edges on demand. |
| scoring config constants | Compile-time `const` in `scoring` | A constant change is a code change, never a learned/config weight (WD-71). A config-file path would require a WD + ADR. |

## Shared artifact ↔ data model mapping (slice-04)

Per `shared-artifacts-registry.md`, the slice-04 artifacts resolve to:

| Shared artifact | Source of truth |
|---|---|
| `subject` | `claims.subject` / `peer_claims.subject` (slice-01/03); byte-equal across query/traversal/weight/explain (`graph_subject_round_trip`). |
| `object` (philosophy) | `claims.object` / `peer_claims.object`; the `--object` query key + traversal root + weight key. Near-miss suggestion engine for typos (US-GRAPH-001 Example 4). |
| `author_did` | `claims.author_did` / `peer_claims.author_did` (derived from the signed payload `author`); carried into EVERY `AttributedClaim`, `Contribution`, and `GraphEdge` as non-`Option<Did>` (Gate 1). |
| `claim_cid` | `claims.cid` / `peer_claims.cid` PK; the auditable unit — every traversal edge AND every weight contribution maps to exactly one cid (Gate 5). |
| `confidence` (numeric) | `claims.confidence` / `peer_claims.confidence` (`DOUBLE`, WD-10); the load-bearing scoring input; the value shown == the value scored (Gate 6). |
| `confidence_bucket` | DERIVED display-only (`claim-domain::confidence_bucket`); never persisted (inherits WD-10 / I-6). |
| `adherence_weight` | DERIVED at query time by `scoring::score`; **NO persisted source** (WD-72); equals the formula (Gate 2). |
| `weight_bucket` | DERIVED display-only label by `scoring::weight_bucket`; **NO persisted source** (Gate 4). |
| `traversal_depth` | `--depth` CLI flag (default 2, WD-76); not persisted; bounds the recursive CTE. |

## Validation rules — translated to data assertions

| Registry rule / Gate | Data-model assertion |
|---|---|
| Gate 1 `scoring_aggregate_preserves_attribution` | Every `WeightedPairing.contributions` is non-empty; every `Contribution.author_did` is `Did` (non-Option); the scoring-feed SQL returns per-claim rows (no SQL aggregation across authors); `xtask check-arch` extends `no_cross_table_join_elides_author` to the scoring/traversal queries. |
| Gate 2 `weight_equals_formula` | `WeightedPairing.weight == sum(contributions.subtotal)` (property test); the `--explain` running sum reproduces it by hand. |
| Gate 3 `sparse_renders_sparse` | `weight_bucket(weight, claim_count<=1 OR distinct_authors<=1)` returns `Sparse` regardless of `weight`; renderer emits the "based on N claims by M authors" line. |
| Gate 4 `weight_and_bucket_never_persisted` | No table column, no on-disk artifact field, no record carries a weight/bucket; the no-persist unit test scans for the forbidden substrings. |
| Gate 5 `traversal_invents_no_edges` | Every `GraphEdge.claim_cid` resolves to an existing `claims.cid` OR `peer_claims.cid`; the recursive CTE selects FROM existing rows only. |
| Gate 6 `scoring_uses_numeric_confidence` | `AttributedClaim.confidence: f64` carries the raw `[0.0,1.0]` numeric; the value displayed in per-claim rows equals the value the formula consumes. |

## Confidence + weight buckets stay UNPERSISTED (inherits + extends WD-10 / OD-2)

Slice-04 does NOT change the confidence-bucket discipline and EXTENDS it to
weight buckets. Neither `confidence_bucket` nor `weight_bucket` is persisted
anywhere — they exist only in the render layer. The slice-01/03 no-persist
unit test extends to scan for `STRONG|MODERATE|SPARSE` and `adherence_weight`
substrings across all tables and on-disk artifacts, in addition to the
existing confidence-bucket substrings.

## identity.toml — optional slice-04 extension (deferred-tunable)

Slice-04 MAY add ONE optional key for a once-per-user first-explorer-query
orientation, mirroring the slice-03 `[federation]` orientation keys. Whether
it ships is deferred to DELIVER against DISTILL scenarios (Q-DELIVER #7); it
is local-only, no telemetry, and the user can delete `identity.toml` to reset.

```toml
[explorer]
first_weighted_query_completed_at = "2026-05-28T10:14:32Z"   # optional; gates a one-time orientation
```
