# Shared Artifacts Registry — openlore-scoring-graph (slice-04)

- **Wave**: DISCUSS
- **Date**: 2026-05-28
- **Owner**: Luna (nw-product-owner)

This registry tracks every `${variable}` that flows across the explore-the-graph
journey steps, its single source of truth, and the integration gate that
verifies consistency. Slice-04 introduces NO new write surface — it reads the
slice-01/02/03 stores. The new artifacts (`adherence_weight`, `weight_bucket`)
are DERIVED + DISPLAY-ONLY and have NO persisted source of truth by design.

## Artifact table

| Artifact | Source of truth | Consumers | Risk | Validation |
|---|---|---|---|---|
| `subject` | `claims.subject` / `peer_claims.subject` (slice-01/03 DuckDB) | step 1 query arg, step 3 traversal node, step 4 weight key, `--explain` | HIGH — drift breaks edge/weight identity | byte-equal across all touchpoints; `graph_subject_round_trip` |
| `object` (philosophy) | `claims.object` / `peer_claims.object` | step 2 `--object` query, step 3 traversal root, step 4 weight key | HIGH — drift breaks philosophy grouping | byte-equal; suggestion engine for near-misses |
| `author_did` | `claims.author_did` / `peer_claims.author_did` (derived from signed payload `author`) | every row header (steps 1-4), step 4 weight breakdown, traversal leaves | HIGH — drift = attribution loss (anti-merging) | `scoring_aggregate_preserves_attribution` + `xtask check-arch` no-elide-author rule |
| `claim_cid` | `claims.cid` / `peer_claims.cid` PK | step 1 row, step 3 edge identity (1 edge = 1 cid), step 4 `--explain` arithmetic | HIGH — the auditable unit of a weight | every traversal edge AND every weight contribution maps to exactly one cid |
| `confidence` (numeric) | `claims.confidence` / `peer_claims.confidence` (DOUBLE, WD-10) | step 1 row, step 4 scoring formula input | HIGH — the load-bearing scoring input | numeric-only persisted (I-6); the value shown == the value scored (`scoring_uses_numeric_confidence`) |
| `confidence_bucket` | DERIVED display-only (`claim-domain::confidence_bucket`) | every confidence display | MEDIUM — must never be persisted | inherited WD-10 / I-6: bucket strings never serialized |
| `adherence_weight` | DERIVED at query time by the pure `scoring` core; **NO persisted source** (WD-72) | step 4 ranking, `--explain` | HIGH (correctness) — must equal the documented formula | `weight_equals_formula` property test; never written to any store |
| `weight_bucket` ([STRONG]/[MODERATE]/[SPARSE]) | DERIVED display-only label; **NO persisted source** | step 4 ranking annotation, sparse-honesty line | MEDIUM — must never be persisted | extends WD-10 display-only invariant to scoring; `weight_bucket_never_persisted` |
| `traversal_depth` | `--depth` CLI flag (default 2); not persisted | step 3 traversal bound | LOW | bounded default prevents fan-out explosion |

## Integration gates (handed to DISTILL as acceptance tests)

These are the cross-step consistency checks DESIGN must preserve and DISTILL must
turn into executable acceptance tests.

### Gate 1 — `scoring_aggregate_preserves_attribution` (LOAD-BEARING)

Every weighted/scored row MUST decompose to its contributing
`(author_did, claim_cid)` tuples via `--explain`. No aggregate row may exist
that cannot enumerate the individually-attributed claims that produced it. This
is the anti-merging-in-aggregates invariant (extends slice-03 I-FED-1). The
`xtask check-arch` `no_cross_table_join_elides_author` rule extends to cover any
scoring query that touches both `claims` and `peer_claims`.

### Gate 2 — `weight_equals_formula` (scoring transparency)

For any weighted result, the displayed `adherence_weight` MUST equal the
documented formula `sum(confidence x author_distinct_bonus x cross_project_triangulation_bonus)`
applied to exactly the displayed contributing claims. The weight must be
reproducible by hand from the `--explain` output. No opaque, ML, or
non-reproducible weight is permitted.

### Gate 3 — `sparse_renders_sparse` (J-002 anxiety mitigation)

A subgraph with thin evidence (e.g., 1 claim by 1 author on 1 project) MUST
render with the `[SPARSE]` bucket AND a "based on N claims by M authors" honesty
line. The system MUST NOT manufacture a confident-looking score from thin
evidence. A single-claim philosophy must visibly look thin.

### Gate 4 — `weight_and_bucket_never_persisted` (display-only discipline)

Neither `adherence_weight` nor `weight_bucket` may be written to any DuckDB
table, any on-disk artifact, any signed payload, or any PDS record. They are
computed at query time and exist only in the render layer. Extends the WD-10 /
I-6 confidence-bucket display-only invariant.

### Gate 5 — `traversal_invents_no_edges` (auditability)

Every edge displayed by `--traverse` MUST correspond to exactly one signed claim
(`claim_cid`). Traversal walks existing claims only; it never interpolates,
infers, or fabricates a contributor<->project<->philosophy edge that no author
signed.

### Gate 6 — `scoring_uses_numeric_confidence` (no silent rounding)

The numeric `confidence` value used as a scoring input MUST be the same numeric
value shown in the per-claim rows (steps 1-3). The display bucket is for humans;
the formula operates on the raw `[0.0, 1.0]` numeric per WD-10.

## Validation questions (checklist)

- Does every `${variable}` in the step mockups have a documented source above? YES.
- Could any weighted aggregate row hide an author? NO — Gate 1 forbids it; `--explain` always decomposes.
- Is any derived value (weight, bucket) at risk of being persisted? NO — Gates 4 forbids it; treated exactly like confidence buckets (WD-10).
- Does any traversal edge lack a backing signed claim? NO — Gate 5 forbids it.
- Is the scoring formula reproducible by a human from the output? YES — `--explain` + Gate 2.
