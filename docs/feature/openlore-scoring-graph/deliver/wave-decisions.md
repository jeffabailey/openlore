# Wave Decisions — DELIVER — openlore-scoring-graph (slice-04)

- **Wave**: DELIVER
- **Date**: 2026-05-28
- **Orchestrator**: Main Claude instance (nw-deliver)
- **Crafter**: @nw-functional-software-crafter (ADR-007)
- **Roadmap**: `deliver/roadmap.json` — 35 steps, 5 phases, all COMMIT/PASS
- **Rigor**: legacy 5-phase TDD; review + L1-L6 refactor + per-feature mutation enabled; models inherit.

## Execution summary

All 35 roadmap steps executed via DES-monitored crafter dispatches. All 35 slice-04
acceptance scenarios GREEN (GQE-1..27 graph_query_explore + SC-1..6 scoring_core;
the 27 GQE scenarios resolve to 29 test fns incl. state-delta bootstraps). slice-01,
slice-02, slice-03 suites show zero regression. One NEW PURE crate shipped (`scoring`)
plus read-side StoragePort extensions; crate count 12 → 13.

| Phase | Scope | Result |
|---|---|---|
| 01 Bootstrap (01-01..04) | ports graph.rs (GraphEdge/AttributedClaim/ScoringFilter/TraversalBound) + scoring crate skeleton + cli explorer flags + 2 test targets | fail-for-right-reason gate; all 35 ATs compile RED |
| 02 scoring pure core (02-01..06) | SC-1..6 (score formula SSOT, weight_bucket, contributions, proptest) | green |
| 03 object/contributor dimensions (03-01..09) | GQE-1..9 (query_by_object UNION-ALL, query_by_contributor, grouped/trail renderers) | green |
| 04 traverse (04-01..06) | GQE-20..25 (WITH RECURSIVE depth-bounded cycle-safe traversal, Connections-found callout, network-disabled) | green |
| 05 weighted + explain (05-01..10) | GQE-10..19 + GQE-26/27 (weighted ranking, sparse honesty, conflict-both-contribute, never-persisted, --explain per-claim arithmetic, Gate 1/2/4/6, e2e wiring) | green |

## DELIVER-wave decisions

| # | Decision | Rationale |
|---|---|---|
| DV-1 | DES `project_id` header added to execution-log right after `des-init-log` (same hook-defect workaround as slice-03/02 DV-1). | Stop-hook reads `project_id`; des-init-log writes `feature_id`. |
| DV-2 | Mutation = per-feature ≥80% on the new PURE `scoring` crate (Phase 6), matching slice-02/03 DV-2. | Per-feature gate at deliver-time + nightly delta sweep backstop. |
| DV-3 | Workspace rustfmt normalization committed as housekeeping at the Phase-05 boundary (per-file-staging crafters accumulate fmt drift: `crates/ports/src/{graph.rs,lib.rs}`, `tests/acceptance/graph_query_explore.rs:2095`). | Keeps CI fmt gate green; matches slice-02/03 DV-3. |
| DV-4 | Anti-merging extended to AGGREGATES (ADR-022 / WD-73): the scoring feed (`query_attributed_for_scoring`) returns per-claim rows with explicit `author_did` + `claim_cid` (UNION ALL, NO SQL GROUP BY); the pure `scoring` core does the aggregation in Rust, decomposing every weight to per-author `Contribution`s. The xtask anti-merging rule passes the feed SQL (`safe_scoring_feed_union_all_with_author_did`). | I-FED-1 / Gate 1: an aggregate weight must never become a faceless consensus row; conflicting claims both contribute per their own confidence, never averaged. |
| DV-5 | Weights are DISPLAY-ONLY (WD-72): never persisted to DuckDB or any `<cid>.json`; recomputed at query time. Proven by `assert_weight_not_persisted` (scans every table/column/cell + every on-disk json) + a recompute leg (adding a contributing peer claim changes the weight 1.05 → 1.71). | Gate 4 release gate. The scoring path is a read-only feed → pure score → render String; no write seam exists. |
| DV-6 | `--explain` reuses the SAME pure `scoring` core output as `--weighted` (single source of truth for the arithmetic); it renders the intermediate per-claim `Contribution`s rather than re-deriving. Derived weights print `{:.2}` (f64 sum precision); base confidence prints verbatim (KPI-4, no rounding / no bucket-midpoint substitution — Gate 6). | WD-71 transparency: the displayed number is the consumed number; the running sum reproduces the displayed weight by hand. |

## Demo Evidence — 2026-05-28

Built `target/release/openlore`. The slice-04 explorer surface
(`graph query --object|--contributor|--traverse|--depth|--weighted|--explain`,
all OPT-IN on top of the slice-01/03 `--subject`/`--federated` surface) is visible
via `--help`. Runtime orientation/edge paths executed standalone in a tempdir
(slice-01 stub env: OPENLORE_HOME, OPENLORE_DID, OPENLORE_KEY_SEED_HEX):

| Story | Command | stdout (captured) | exit |
|---|---|---|---|
| US-GRAPH-001 | `graph query --object org.openlore.philosophy.dependency-pinning` (empty) | `Claims embodying org.openlore.philosophy.dependency-pinning (grouped by subject):` + `No claims found for object …` | 0 |
| US-GRAPH-002 | `graph query --contributor did:plc:rachel-test` (empty) | `No local claims authored by did:plc:rachel-test. Subscribe and pull with …` | 0 |
| US-GRAPH-003 | `graph query --object … --weighted` (empty) | weighted-view header + `How weight is computed (auditable, no ML):` + the full formula (confidence x author_distinct_bonus x cross_project_triangulation_bonus) | 0 |
| US-GRAPH-005 | `graph query --object … --weighted --explain github:foo/bar` (absent) | `openlore graph query: Subject github:foo/bar is not in this result set.` (stderr) | non-zero |

Live seeded-graph happy paths (ranked weights with per-claim inputs, the `[SPARSE]`
"(!) based on 1 claim by 1 author — treat as a lead, not a conclusion" honesty line,
the `--traverse` "Connections found: did:plc:rachel-test spans 2 of these projects"
callout, the `--explain` per-claim running sum `0.55 + 0.50 = 1.05 = displayed weight`)
are captured by the GREEN acceptance subprocess tests that drive the real `openlore`
binary against a seeded multi-author DuckDB (GQE-10, GQE-11, GQE-16, GQE-19, GQE-22).
These ARE the captured demo evidence per story (slice-02/03 model):

| Story | Demo coverage (green acceptance scenario, real binary + seeded DuckDB) |
|---|---|
| US-GRAPH-001 (object/subject) | GQE-1..5 (grouped-by-subject, per-claim attribution, "No claims are merged" footer, local-first) |
| US-GRAPH-002 (contributor) | GQE-6..9 (one-DID reasoning trail, "one developer's reasoning trail, not a community consensus") |
| US-GRAPH-003 (weighted) | GQE-10 (ranked transparent no-ML), GQE-11 (sparse honesty line), GQE-12/13 (triangulation + conflict-both-contribute), GQE-14 (never persisted), GQE-15 (network-disabled) |
| US-GRAPH-004 (traverse) | GQE-20..25 (depth-bounded cycle-safe tree + Connections-found callout, network-disabled) |
| US-GRAPH-005 (--explain) | GQE-16 (running sum == weight), GQE-17 (sparse repeat), GQE-18 (absent = usage error), GQE-19 (bonus attributed to earner), GQE-26 (numeric confidence match) |
| US-GRAPH-006 (@infrastructure) | the new `scoring` crate + ports graph.rs + scoring-feed StoragePort method bootstrapped (Phase 01); GQE-27 e2e wiring proof |

Transparency + anti-merging invariants end-to-end verified: weights are display-only
(never persisted; recompute at query time — GQE-14), every aggregate decomposes to
per-author contributions (no faceless consensus row — GQE-27 / Gate 1), the displayed
confidence is the consumed confidence (GQE-26 / Gate 6), and the formula is "no ML"
and reproducible by hand (GQE-16 / Gate 2).

## Post-Merge Integration Gate — PASS

- Full slice-04 acceptance suite GREEN single-threaded (graph_query_explore 29 +
  scoring_core 6); slice-01 (walking_skeleton 19, lexicon_conformance 10 [1 pre-existing
  ignored], lexicon_counter_claim 5, integration 12, state_delta 2), slice-02
  (scrape_auth 7, scrape_candidates 7, scrape_github 11, scrape_sign 11, scraper_domain 6),
  slice-03 (counter_claim 8, federated_query 10, federation_roundtrip 6, peer_pull 10,
  peer_subscribe 10) — all green, zero regression. xtask guards: anti_merging 7,
  autoconfirm_guard 4 — green. — 2026-05-28.
- Environment matrix: slice-04 acceptance is hermetic (subprocess + seeded `DuckDB` +
  `tempfile` HOME) and does NOT depend on a per-environment cross-product; the default
  matrix (clean | with-pre-commit | with-stale-config) is satisfied by the hermetic
  design (DEVOPS graceful-degrade default; same rationale as slice-02/03).
- Known harness flake (NOT a slice-04 regression): `adapter-system-clock` `now_utc_*`
  env-var contention under full-workspace PARALLEL lib-test runs; the acceptance
  targets pass single-threaded / in isolation.

## Quality gates

- `cargo xtask check-arch`: OK (13 workspace members) — scoring pure-core allowlist +
  anti-merging-in-aggregates SQL rule (scoring-feed UNION ALL + recursive-CTE base)
  active.
- `cargo xtask check-probes`: OK (the 1 allowlisted-stub warning is the pre-existing
  slice-03 peer-storage probe, out of slice-04 scope).
- Per-phase L1-L6 refactor / adversarial review / mutation outcomes recorded below
  (Phases 4–7).
