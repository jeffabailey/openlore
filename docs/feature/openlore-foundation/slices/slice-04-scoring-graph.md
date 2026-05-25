# Slice 04 — Scoring Graph (sibling feature seed)

**Status**: deferred — births sibling feature `openlore-scoring-graph` after slices 01-03 land.
**Slice priority**: P3
**Effort estimate**: ~1-2 weeks
**Primary persona**: P-001 + P-002
**Primary job**: J-002 Explore the philosophy graph + J-004 Evaluate contributor adherence

## Hypothesis

> Triangulating a contributor's claims across multiple projects produces an
> adherence weighting for (contributor, philosophy) tuples that a senior engineer
> finds non-obvious and defensible — i.e. they would cite it when justifying a
> stack choice or a collaboration recommendation.

## Disproves if it fails

- Triangulation produces weights that feel arbitrary or that re-rank claims in
  ways the user finds noisy.
- The graph traversal cost on DuckDB at realistic data scale forces a switch to
  Kùzu / SurrealDB / a graph-native store.

## In scope (when this slice runs)

- Scoring engine: contributor↔project, project↔philosophy, contributor↔philosophy edges.
- Triangulation weighting (multi-project agreement increases adherence weight).
- User-configurable trust weights per author per predicate.
- `openlore graph query --weight-by adherence` etc.

## Out of scope

- ML-derived weights; this slice is purely deterministic triangulation.
- Real-time recomputation; on-demand recompute is fine.

## Why deferred

Scoring is only meaningful at data scale. Until slices 01-03 produce a corpus of
authored + scraped + federated claims, scoring algorithms will tune against noise.

## Hand-off

Sibling feature directory at planning time: `docs/feature/openlore-scoring-graph/`.
Database choice (DuckDB vs Kùzu vs SurrealDB) revisited HERE, not earlier.
