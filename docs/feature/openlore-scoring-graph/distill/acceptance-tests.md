# Acceptance Test Design — openlore-scoring-graph (slice-04)

- **Wave**: DISTILL
- **Date**: 2026-05-28
- **Acceptance Designer**: Quinn (nw-acceptance-designer)
- **Feature**: openlore-scoring-graph
- **Slice**: slice-04-scoring-graph (sibling feature; sibling-feature pattern per WD-9/WD-69)
- **Crafter target (DELIVER)**: `@nw-functional-software-crafter` (per ADR-007)
- **Inherits**: slice-01/02/03 DISTILL artifacts + slice-04 DISCUSS WD-69..WD-79 + slice-04 DESIGN WD-80..WD-93 + ADR-020..ADR-022
- **Language**: Rust (per ADR-009)
- **Test framework**: same as slice-01/02/03 — Rust std `#[test]` (per DD-GRAPH-1)

This document is the human-readable map over the executable test skeletons in
`tests/acceptance/graph_query_explore.rs` (layer 3, subprocess) +
`tests/acceptance/scoring_core.rs` (layer 2, pure-core). The `.rs` files are the
SSOT for executable scenarios.

---

## 1. Wave-Decision Reconciliation result

**Reconciliation passed — 0 contradictions** between DISCUSS WD-69..WD-79
(+ OD-GRAPH-1..4 accepted at default) and DESIGN WD-80..WD-93. The full
reconciliation matrix is in `wave-decisions.md §Wave-Decision Reconciliation
result`. DESIGN WD-81/85/86/87 RESOLVE the four DISCUSS Open Decisions; WD-90 +
Q-DELIVER-SCORE-1 RESOLVE the single `# DISTILL: confirm` flag (see §2).

DEVOPS-wave artifacts are absent (`docs/feature/openlore-scoring-graph/devops/`
does not exist as of this writing). Per `nw-distill` Graceful Degradation
matrix: WARN + apply default environment matrix; DO NOT block. Slice-04's
acceptance scenarios do not depend on a per-environment fixture cross-product
(the test seam is the subprocess + REAL DuckDB seeded with own + peer claims).

---

## 2. Resolved DISTILL flag (Q-DELIVER-SCORE-1 / WD-90)

DISCUSS + DESIGN carried ONE `# DISTILL: confirm` flag into this wave: the
bucket rule when cross-project triangulation raises a single-claim pairing's
weight (data-models.md §"Bucket nuance"; DESIGN Q-DELIVER-SCORE-1). DISTILL
inherits the WD-90 resolution verbatim and binds the scenarios.

| `# DISTILL: confirm` flag | Resolution (WD-90 / Q-DELIVER-SCORE-1) | Bound scenario(s) |
|---|---|---|
| Does Rachel's cargo+nixpkgs span lift cargo out of `[SPARSE]` despite cargo being a single claim? | **Cross-project triangulation by the SAME author counts toward evidence breadth for the bucket** — a triangulated single-claim pairing is NOT sparse (cargo); a single claim with NO triangulation AND NO co-author STAYS `[SPARSE]` regardless of confidence magnitude (tokio at 0.50; a 0.99 single claim too) | `scoring_core.rs` SC-6 (cargo NOT sparse via span) + SC-3 (single-claim no-span sparse at any confidence) + `graph_query_explore.rs` GQE-11 (tokio sparse) + GQE-19 (cargo triangulation explained) |

This is the ONE consistent rule reconciling US-GRAPH-003 Example 1 (cargo
narrated `[STRONG]` "boosted by Rachel spanning cargo+nixpkgs") with US-GRAPH-003
Example 2 (tokio single-claim `[SPARSE]`) + data-models.md §"Bucket nuance". The
`weight_bucket` function takes cross-project span as a breadth INPUT, not just
`weight` magnitude. DELIVER picks the exact STRONG-vs-MODERATE cut within WD-86's
tunable constants; DISTILL asserts only the load-bearing NOT-Sparse half.

Zero open ambiguity at scenario-write time.

---

## 3. Scope and shape

Same hexagonal port-to-port discipline as slice-01/02/03: every subprocess
acceptance test enters through the CLI driving adapter via the real `openlore`
binary (`assert_cmd::cargo_bin`), exercises the real `claim-domain` + the NEW
pure `scoring` core + `adapter-duckdb` (AUGMENTED with recursive CTEs) pure-core
/ local-effect stack over a REAL DuckDB, and the pure scoring-formula PROPERTIES
are exercised by direct pure-core invocation at layer 2.

### The single most important shape fact of slice-04

**Slice-04 introduces NO new external fake.** It is a READ slice over the LOCAL
federated graph. The graph the scenarios need is SEEDED into the REAL DuckDB by
REUSING the slice-03 seam: own claims via the real `claim add` verb, peer claims
via the real `peer add` + `peer pull` verbs against the slice-03 `PeerPds`
double (built with `build_verifiable_peer_records*`). Scoring + traversal +
dimension queries are local read-only analysis over that real store. A new
external fake would model a surface that does not exist (WD-79/WD-92: no network,
no write). The slice-04 test-support additions are a graph-SEEDING orchestrator
(`seed_federated_graph`) + scoring/traversal assertion helpers — not a new fake.

### Layer placement (per nw-test-design-mandates Mandate 9 + DD-GRAPH-3)

| Layer | Test file(s) | Real components | Test mode |
|---|---|---|---|
| Subprocess / FS acceptance (layer 3) | `graph_query_explore.rs` (27) | CLI binary + REAL DuckDB (seeded own + peer claims) + pure `scoring` core + `claim-domain`; slice-03 `PeerPds` double only as the SEED mechanism | example-only (Mandate 11) |
| In-memory acceptance (layer 2) | `scoring_core.rs` (6; 4 `@property`) | None — pure `scoring` core directly | example + `@property` proptest (Mandate 9 layer-2 PBT full) |

Layer 1 (pure-core unit tests for the exact per-claim apportionment, bucket
thresholds, mutation testing of `formula.rs`/`bucket.rs`) is OUT OF DISTILL
SCOPE — DELIVER's inner TDD loop (DD-GRAPH-7). The `adapter-duckdb` recursive-CTE
cycle-safety + termination + depth-bound PROBE (ADR-021 probe #2/#3) is also
DELIVER's (below the driving-port boundary; DESIGN §6.3).

### What is real, what is seeded (slice-04 additions to the slice-03 table)

| Component | Treatment | Why |
|---|---|---|
| Pure `scoring` core (`score`, `weight_bucket`, ADTs) | REAL — invoked directly at layer 2 (`scoring_core.rs`), and through the cli at layer 3 | The transparency contract IS the formula; the pure core is trivially testable with no fixtures (CM-D) |
| `adapter-duckdb` (AUGMENTED: `query_by_object`, `query_by_contributor`, `query_attributed_for_scoring`, `traverse_graph`) | REAL DuckDB (recursive CTEs + UNION ALL projections over the EXISTING schema; no new tables) | Mandate 6 — every driven adapter has a real-I/O scenario; the graph is seeded into the real store |
| `claims` (own) + `peer_claims` (peer) tables | REAL DuckDB, SEEDED via `claim add` + `peer pull` | The graph the scenarios read is the union of slice-01 own + slice-03 peer + slice-02 scraper-signed claims |
| Peer claim source (the SEED mechanism) | slice-03 `PeerPds` double + `build_verifiable_peer_records*` | Used ONLY to populate `peer_claims` as a precondition; scoring/traversal then read the local store. NO new fake. |
| `adherence_weight` / `weight_bucket` | DERIVED at query time; NEVER persisted (WD-72/WD-89) | Gate 4 scans every table + artifact for the forbidden substrings |

### Test file rationale + placement (DD-GRAPH-4)

FLAT layout under `tests/acceptance/` matching slice-01/02/03. Two new files
alongside the existing ones:

```
tests/acceptance/
  walking_skeleton.rs            # slice-01, unchanged
  lexicon_conformance.rs         # slice-01, unchanged
  federation_roundtrip.rs        # slice-01, unchanged
  peer_subscribe.rs              # slice-03, unchanged
  peer_pull.rs                   # slice-03, unchanged
  counter_claim.rs               # slice-03, unchanged
  federated_query.rs             # slice-03, unchanged
  lexicon_counter_claim.rs       # slice-03, unchanged
  scrape_*.rs / scraper_domain.rs # slice-02, unchanged
  graph_query_explore.rs         # slice-04 NEW (27 scenarios; layer 3 subprocess; US-GRAPH-001..006)
  scoring_core.rs                # slice-04 NEW (6 scenarios; layer 2 pure-core; 4 @property)
  support/
    mod.rs                       # EXTENDED — adds the slice-04 seeder + scoring/traversal helpers
```

Rationale: preserves `cargo test --test <file>` ergonomics; the two new files
are clearly labeled by concern (the explorer verb surface vs the pure scoring
core). Symmetric with slice-02 (`scrape_*` + `scraper_domain.rs`) and slice-03
(`peer_*` + `lexicon_counter_claim.rs`). Nested layout rejected for the same
reasons as DD-FED-14.

---

## 4. Acceptance test inventory

Per Mandate 3 (User Journey Completeness) every test exercises a complete user
journey from observable trigger through observable outcome. Full per-scenario
docstrings + Given-When-Then are in the `.rs` files.

### `tests/acceptance/graph_query_explore.rs` — 27 scenarios (layer 3)

Stories: US-GRAPH-001 (object/subject dimension), US-GRAPH-002 (contributor),
US-GRAPH-003 (weighted/sparse), US-GRAPH-004 (traverse), US-GRAPH-005 (explain),
US-GRAPH-006 (infra wiring).

| # | Test name | Story | Type | Tag(s) |
|---|---|---|---|---|
| GQE-1 | `graph_query_by_object_groups_by_subject_with_per_author_attribution` | US-GRAPH-001 | happy / WS | `@walking_skeleton @real-io @driving_port @j-002 @kpi-graph-2` |
| GQE-2 | `graph_query_by_object_identical_content_different_authors_renders_two_rows` | US-GRAPH-001 | anti-merging | `@anti-merging @kpi-graph-2 @edge` |
| GQE-3 | `graph_query_bare_subject_is_unchanged_from_prior_slice_behavior` | US-GRAPH-001 | regression | `@regression @default-off` |
| GQE-4 | `graph_query_by_object_unknown_philosophy_returns_empty_with_suggestion_exit_zero` | US-GRAPH-001 | error | `@error` |
| GQE-5 | `graph_query_by_object_succeeds_with_network_disabled` | US-GRAPH-001 | local-first | `@local-first @i-graph-7` |
| GQE-6 | `graph_query_by_contributor_lists_full_reasoning_trail_with_honest_framing` | US-GRAPH-002 | happy | `@kpi-graph-2 @happy` |
| GQE-7 | `graph_query_by_contributor_own_did_is_a_valid_self_review_annotated_you` | US-GRAPH-002 | edge | `@edge` |
| GQE-8 | `graph_query_by_contributor_absent_did_degrades_with_subscribe_pull_hint_exit_zero` | US-GRAPH-002 | error | `@error` |
| GQE-9 | `graph_query_by_contributor_soft_removed_peer_labels_unsubscribed_cache` | US-GRAPH-002 | edge | `@edge` |
| GQE-10 | `graph_query_weighted_ranks_projects_with_transparent_no_ml_formula` | US-GRAPH-003 | happy / WS | `@walking_skeleton @kpi-graph-1 @kpi-graph-3 @gate-2` |
| GQE-11 | `graph_query_weighted_single_claim_single_author_renders_sparse_with_honesty_line` | US-GRAPH-003 | boundary | `@kpi-graph-4 @gate-3 @sparse @release-gate` |
| GQE-12 | `graph_query_weighted_multi_author_support_raises_triangulation_weight` | US-GRAPH-003 | happy | `@kpi-graph-1 @kpi-graph-2` |
| GQE-13 | `graph_query_weighted_conflicting_claims_both_contribute_nothing_dropped` | US-GRAPH-003 | anti-merging | `@kpi-graph-2 @anti-merging @edge` |
| GQE-14 | `graph_query_weighted_outputs_are_never_persisted_and_recompute_at_query_time` | US-GRAPH-003 | guardrail | `@gate-4 @display-only @release-gate` |
| GQE-15 | `graph_query_weighted_succeeds_with_network_disabled` | US-GRAPH-003 | local-first | `@local-first @i-graph-7` |
| GQE-16 | `graph_query_explain_reproduces_weight_from_per_claim_arithmetic` | US-GRAPH-005 | happy | `@kpi-graph-2 @kpi-graph-3 @gate-1 @gate-2` |
| GQE-17 | `graph_query_explain_on_sparse_subject_repeats_the_honesty_line` | US-GRAPH-005 | edge | `@gate-3 @sparse @edge` |
| GQE-18 | `graph_query_explain_for_subject_absent_from_result_set_is_a_usage_error` | US-GRAPH-005 | error | `@error` |
| GQE-19 | `graph_query_explain_attributes_triangulation_bonus_to_the_contributor_who_earned_it` | US-GRAPH-005 | edge | `@kpi-graph-2 @kpi-graph-3 @gate-1 @edge` |
| GQE-20 | `graph_query_traverse_surfaces_a_non_obvious_cross_project_contributor_connection` | US-GRAPH-004 | happy / WS | `@walking_skeleton @kpi-graph-1 @gate-5 @happy` |
| GQE-21 | `graph_query_traverse_single_node_no_edges_renders_without_fabrication` | US-GRAPH-004 | edge | `@gate-5 @edge` |
| GQE-22 | `graph_query_traverse_is_bounded_to_default_depth_two_and_reports_omitted_edges` | US-GRAPH-004 | edge | `@wd-76 @bounded @edge` |
| GQE-23 | `graph_query_traverse_depth_override_reveals_previously_omitted_real_edges` | US-GRAPH-004 | edge | `@wd-76 @gate-5 @edge` |
| GQE-24 | `graph_query_traverse_every_edge_maps_to_a_verifiable_signed_claim` | US-GRAPH-004 | happy | `@gate-5 @anti-merging @happy` |
| GQE-25 | `graph_query_traverse_succeeds_with_network_disabled` | US-GRAPH-004 | local-first | `@local-first @i-graph-7` |
| GQE-26 | `graph_query_scoring_uses_the_same_numeric_confidence_shown_in_per_claim_rows` | US-GRAPH-003 | guardrail | `@gate-6 @kpi-graph-3` |
| GQE-27 | `graph_query_weighted_end_to_end_wires_scoring_feed_without_persisting_outputs` | US-GRAPH-006 | infra | `@infrastructure @gate-1 @gate-4` |

`@error`-tagged: GQE-4, GQE-8, GQE-18 = 3/27 = 11.1%. See §4-rationale below for
the read-surface justification.

### `tests/acceptance/scoring_core.rs` — 6 scenarios (layer 2; 4 `@property`)

Stories: US-GRAPH-003 (transparency/sparse), US-GRAPH-005 (decomposition),
US-GRAPH-006 (pure core); Q-DELIVER-SCORE-1 (bucket rule).

| # | Test name | Source | Type | Tag(s) |
|---|---|---|---|---|
| SC-1 | `scoring_weight_equals_sum_of_contributions_property` | Gate 2 / WD-71 | `@property` | `@property @gate-2 @kpi-graph-3` |
| SC-2 | `scoring_score_is_deterministic_property` | DESIGN §5.1 inv 2 | `@property` | `@property @i-graph-1` |
| SC-3 | `scoring_single_author_single_claim_is_sparse_at_any_confidence_property` | Gate 3 / WD-74/WD-90 | `@property` | `@property @gate-3 @kpi-graph-4 @wd-90` |
| SC-4 | `scoring_multi_author_outweighs_single_author_at_equal_confidence_property` | WD-77 | `@property` | `@property @kpi-graph-1 @wd-77` |
| SC-5 | `scoring_two_author_pairing_decomposes_to_two_attributed_contributions` | Gate 1 type-level / WD-73 | example | `@gate-1 @anti-merging` |
| SC-6 | `scoring_cross_project_triangulation_counts_as_breadth_lifts_out_of_sparse` | Q-DELIVER-SCORE-1 / WD-90 | example | `@score-1 @bucket-rule @wd-90` |

### Total slice-04 scenarios

27 (GQE) + 6 (SC) = **33 scenarios** authored, all RED-ready as `todo!()`
scaffolds with `// SCAFFOLD: true` module markers. Within the ~25-35 target band
for 6 stories.

### §4-rationale — error-path ratio + read-surface justification

Explicit `@error` ratio is 3/33 = 9.1% (GQE-4 unknown object, GQE-8 absent
contributor, GQE-18 explain-absent-subject). This is BELOW the 40%
nw-test-design-mandates target — by the same read-surface logic as slice-03's
`federated_query.rs` (0/8 `@error`) + slice-01 DD-8:

- Slice-04 is a READ/VIEW slice. The load-bearing slice-04 RISK is NOT
  input-validation sad paths — it is the GUARDRAIL surface: anti-merging in
  aggregates (Gate 1; GQE-2/12/13/16/19/24/26 + SC-5), sparse-honesty (Gate 3;
  GQE-11/17 + SC-3, release-blocking), never-persisted (Gate 4; GQE-14/27), and
  no-invented-edges (Gate 5; GQE-20/21/24). Counting guardrail + boundary +
  anti-merging scenarios as the "non-happy" surface, the ratio is 14/33 = 42% —
  above target. The 3 pure `@error` exits (empty result / usage error) are the
  only true validation sad paths the read surface admits.
- The SUBSTRATE sad paths (recursive-CTE non-termination on a cyclic graph,
  depth-bound violation) live at the `adapter-duckdb` PROBE layer per DESIGN
  §6.3 / ADR-021 (probe #2/#3) — DELIVER's adapter-integration concern, below the
  driving-port boundary, backed by `scoring_fixture_cyclic_two_claim_graph`.
- No infrastructure-failure scenarios at the acceptance layer (disk full, fsync
  lies) — continues slice-01 DD-8 / slice-03 DD-FED deferral to DELIVER's
  adapter-level integration tests.

---

## 5. Driving Adapter coverage (Mandate 1 + RCA P1)

Every NEW or EXTENDED CLI flag in ADR-020 is covered by at least one subprocess
scenario:

| Flag (ADR-020) | Scenario coverage |
|---|---|
| `graph query --object <philosophy>` (NEW) | GQE-1 (happy/WS), GQE-2 (anti-merging), GQE-4 (error), GQE-5 (local-first) |
| `graph query --contributor <did>` (NEW) | GQE-6 (happy), GQE-7 (own DID), GQE-8 (absent), GQE-9 (unsubscribed cache) |
| `graph query --weighted` (NEW; combinable) | GQE-10 (happy/WS), GQE-11 (sparse), GQE-12 (triangulation), GQE-13 (conflict), GQE-14 (never-persisted), GQE-15 (local-first), GQE-26 (numeric conf), GQE-27 (e2e) |
| `graph query --explain <subject>` (NEW; combinable with --weighted) | GQE-16 (reproduce), GQE-17 (sparse), GQE-18 (usage error), GQE-19 (triangulation attributed) |
| `graph query --traverse` (NEW; combinable) | GQE-20 (connection/WS), GQE-21 (no fabrication), GQE-22 (bounded), GQE-24 (edge=signed claim), GQE-25 (local-first) |
| `graph query --depth K` (NEW; modifies --traverse) | GQE-22 (default 2), GQE-23 (--depth 3 override) |
| `graph query --subject <S>` (UNCHANGED) | GQE-3 (bare-subject regression — byte-identical to slice-01/03) |

Zero uncovered NEW flags. Every flag is exercised via subprocess (the real
`openlore` binary) — pipeline/service-level tests do NOT replace driving-adapter
tests.

---

## 6. Driven adapter coverage (Mandate 6)

| Driven adapter | Real-I/O scenario? | Tag |
|---|---|---|
| `adapter-duckdb` (StoragePort `query_by_object`) | YES — GQE-1, GQE-2, GQE-4, GQE-5 exercise it via `graph query --object` over real DuckDB | `@real-io` |
| `adapter-duckdb` (StoragePort `query_by_contributor`) | YES — GQE-6, GQE-7, GQE-8, GQE-9 | `@real-io` |
| `adapter-duckdb` (StoragePort `query_attributed_for_scoring`) | YES — GQE-10..GQE-19, GQE-26, GQE-27 (the scoring-feed) | `@real-io` |
| `adapter-duckdb` (StoragePort `traverse_graph` recursive CTE) | YES — GQE-20..GQE-25 via `graph query --traverse` over real DuckDB | `@real-io` |
| Pure `scoring` core (not an adapter — no probe) | YES — SC-1..SC-6 invoke it directly (layer 2); GQE-10..GQE-19 through the cli | `@property` / `@real-io` |
| Filesystem `claims/` + `peer_claims/<did>/` (read) | YES — GQE-14/27 scan the on-disk artifacts for the never-persisted gate | `@real-io` |

The recursive-CTE cycle-safety + termination + depth-bound PROBE
(`adapter-duckdb` probe #2/#3 per ADR-021) is DELIVER's adapter-integration
deliverable (DESIGN §6.3), backed by `scoring_fixture_cyclic_two_claim_graph` —
NOT a DISTILL acceptance scenario (it is below the driving-port boundary, a
substrate-lie check). The pure `scoring` core has NO `probe()` (it touches no
substrate); its Earned-Trust analog is the layer-2 property suite + DELIVER's
mutation testing (DESIGN §10).

---

## 7. Integration gates coverage (shared-artifacts-registry.md, Gates 1-6)

| Gate | Where asserted | Mandatory for KPI |
|---|---|---|
| 1. `scoring_aggregate_preserves_attribution` (anti-merging in aggregates) | GQE-16 (--explain decomposes) + GQE-1/26 + SC-5 (type-level) + GQE-27 (e2e) | **KPI-GRAPH-2 (release-blocking)** |
| 2. `weight_equals_formula` (scoring transparency) | GQE-16 (--explain running sum == weight) + SC-1 (weight == sum) | **KPI-GRAPH-3 (release-blocking)** |
| 3. `sparse_renders_sparse` (J-002 anxiety mitigation) | GQE-11 (load-bearing) + SC-3 (any confidence) + GQE-17 | **KPI-GRAPH-4 (release-blocking)** |
| 4. `weight_and_bucket_never_persisted` (display-only) | GQE-14 (never persisted + recompute) + GQE-27 (e2e) | KPI-GRAPH-3 |
| 5. `traversal_invents_no_edges` (auditability) | GQE-20 (no invented edges) + GQE-24 (every edge = signed claim) + GQE-21 (no fabrication) | KPI-GRAPH-1 |
| 6. `scoring_uses_numeric_confidence` (no silent rounding) | GQE-26 (dimension confidence == --explain base) | KPI-GRAPH-3 |

All six gates have >= 1 acceptance test. Gates 1, 2, 3 are the release-blocking
guardrails (anti-merging-in-aggregates + scoring-transparency are the
load-bearing carries of slice-03 I-FED-1 + WD-71 into the aggregate surface).

---

## 8. KPI coverage

| KPI | Description | Acceptance coverage |
|---|---|---|
| KPI-GRAPH-1 | Non-obvious connection (north star) | GQE-20 (connection SURFACEABLE), GQE-12 (triangulation lift), SC-4 (monotonicity); the RATE is DEVOPS telemetry, not an acceptance assertion |
| KPI-GRAPH-2 | Zero attribution loss in aggregates (release-blocking) | GQE-1/2/12/13/16/19/24/26/27 + SC-5 (Gate 1) |
| KPI-GRAPH-3 | Every weight reproducible (release-blocking) | GQE-16 + SC-1 (Gate 2) + GQE-10 (formula printed) + GQE-14 (display-only) + GQE-26 (numeric) |
| KPI-GRAPH-4 | Sparse renders sparse (release-blocking) | GQE-11 (load-bearing) + SC-3 + GQE-17 (Gate 3) |
| KPI-GRAPH-5 | Referenced justification | No acceptance assertion — 30-day survey; the suite proves the result EXISTS to cite |
| KPI-GRAPH-6 | Local-read latency | No hard acceptance assertion — telemetry; GQE-22 asserts the depth bound that keeps traversal responsive |

KPI-GRAPH-1/5/6 are telemetry-measured (production), not asserted at the
acceptance boundary — by design, per outcome-kpis.md Measurement Plan.

---

## 9. Three Pillars compliance

| Pillar | How DISTILL satisfied it |
|---|---|
| 1 — Domain language | Scenario titles use `query`, `object`, `philosophy`, `subject`, `project`, `contributor`, `reasoning trail`, `weighted`, `adherence weight`, `sparse`, `traverse`, `connection`, `explain`, `attribution`, `triangulation`, `author DID`, `cid`. Zero technical jargon in titles/step-names: NO `SQL`, `recursive CTE`, `JSON`, `HTTP`, `endpoint`, `schema`, `DuckDB`. (`DuckDB` / `recursive CTE` appear in test-support + docstring comments because that IS the substrate the seeder/adapter speak; they do NOT appear in any scenario title or step-method name.) |
| 2 — Chained narrative | Multi-scenario journeys read in order: the explore-the-graph arc query (GQE-1) -> contributor (GQE-6) -> weighted (GQE-10) -> sparse (GQE-11) -> traverse (GQE-20) -> explain (GQE-16) reuses the same seeded-graph Given via `seed_federated_graph(FederatedGraphFixture::*)`. GQE-14 chains query -> add-peer-claim -> re-query (the recompute-at-query-time narrative). The layer-2 SC-1..SC-6 chain the same `scoring::score` Given across reproducibility -> determinism -> sparse -> triangulation. No copy-pasted fixture setup — the named `FederatedGraphFixture` variants are the shared step composition. |
| 3 — App as in production | Every GQE-* scenario spawns the REAL `openlore` binary via `assert_cmd::Command::cargo_bin` (the production composition root); the graph is seeded through the REAL `claim add` + `peer add` + `peer pull` verbs; scoring/traversal read the REAL DuckDB. No hand-rebuilt wiring. The slice-03 `PeerPds` double substitutes ONLY the external/non-deterministic peer-PDS boundary as the seed mechanism. SC-* (layer 2) invoke the pure `scoring` core directly (the function signature IS the port). |

---

## 10. Mandate compliance evidence (CM-A through CM-H)

| Mandate | Compliance evidence |
|---|---|
| CM-A (Mandate 1, hexagonal boundary) | All `graph_query_explore.rs` scenarios invoke `openlore` via subprocess; ZERO direct imports of `adapter_duckdb::*` etc. from the test bodies. The `scoring_core.rs` layer-2 tests directly invoke pure-core `scoring::score` / `scoring::weight_bucket` — appropriate at layer 2 per Mandate 9 (the pure function signature IS the driving port at domain scope) |
| CM-B (Mandate 2, business language) | Grep of test names: zero `HTTP`, `endpoint`, `database`, `schema`, `JSON`, `SQL`, `CTE`. Domain terms only (`query`, `object`, `contributor`, `weighted`, `sparse`, `traverse`, `explain`, `attribution`, `triangulation`) |
| CM-C (Mandate 3, complete journeys) | Every test traces to a user story → see traceability.md §4. The chained narratives (the query→weighted→traverse→explain arc; the GQE-14 query→add→re-query recompute pair; the SC reproducibility→determinism→sparse→triangulation chain) satisfy Pillar 2 |
| CM-D (Mandate 4, pure function extraction) | The `scoring` formula is a PURE function exercised DIRECTLY in `scoring_core.rs` SC-1..SC-6 — no fixtures, no adapters, no environment cross-product. The CLI parameterization is just `tempfile::TempDir` for HOME (inherited slice-01 seam). Impure storage is behind the `StoragePort` real adapter, seeded once per scenario |
| CM-E (Mandate 8, state-delta + Universe) | **DEFERRED to DELIVER** (DD-GRAPH-10) — same status as slice-01 DD-3 / slice-03 DD-FED-10. The Rust `state_delta` port at `tests/common/state_delta.rs` was bootstrapped by slice-01; slice-04 INHERITS it. Slice-04 scenarios use named assertion helpers in `support/mod.rs` (`assert_weight_not_persisted`, `assert_weight_decomposes_to_per_author`, `assert_sparse_rendered_as_sparse`, `assert_explain_sums_to_weight`, `assert_every_edge_has_backing_claim`) as the Rust idiomatic mirror; DELIVER migrates the load-bearing scenarios (GQE-14, GQE-16, GQE-11, GQE-20, GQE-1/26) to `assert_state_delta(before, after, universe, expected)` form. Universe entries are port-exposed (`cli.graph_query.bucket[subject]`, `cli.graph_query.explain.running_sum[subject]`, `storage.duckdb.no_weight_or_bucket_column`, `cli.graph_query.traverse.edge_cids`) — NEVER internal scoring/StoragePort struct fields |
| CM-F (Mandate 9, layered PBT mode) | SC-1..SC-4 are `@property` at layer 2 (proptest); SC-5/SC-6 are example-pinned; ALL 27 GQE-* subprocess scenarios at layer 3 are example-only. ZERO proptest at layer 3+ |
| CM-G (Mandate 10, two-tier acceptance) | Tier A only. Per Mandate 10 + the state-machine-PBT trigger: the explore-the-graph journey IS >=3 chained scenarios with a domain-rich input space (qualifying on BOTH Tier-B triggers), BUT the load-bearing aggregate invariants are FORALL properties over a PURE function whose model is NOT a state machine (Hebert ch.11: the model shape, not user-perceived states) — scoring is a pure read with no mutate-then-observe command protocol. The layer-2 `@property` suite (`scoring_core.rs`) already explores the generative input space. A `RuleBasedStateMachine` + `InMemoryComposition` would be ceremony with no detection gain. **Revisit at slice-05** (AppView) where multi-user/cohort aggregation introduces a genuine indexer state machine. See DD-GRAPH-9 |
| CM-H (Mandate 11, sad-paths example-based) | Every layer-3 sad path is a named example scenario: GQE-4 (unknown object), GQE-8 (absent contributor), GQE-18 (explain absent subject), GQE-21 (no-fabrication edge). ZERO proptest at layer 3+ for sad paths. The substrate sad paths (cycle/termination/depth-bound) are DELIVER's adapter probe (ADR-021), not the acceptance suite |

---

## 11. Project Infrastructure Policy — slice-04 additions

Slice-01 DD-11 / slice-03 DD-FED-11 deferred writing
`docs/architecture/atdd-infrastructure-policy.md` until the orchestrator scope
permits. Slice-04's orchestrator brief also limits writes to the slice-04
directories + `tests/acceptance/` + `crates/test-support/src/`. The policy file
is STILL not created in this DISTILL wave (DD-GRAPH-11). The new entries that
SHOULD land when the surface opens:

```markdown
# Slice-04 additions to ATDD Infrastructure Policy

## Driving (extends slice-01/02/03)
| Port | Mechanism | Note |
|---|---|---|
| CLI (`openlore graph query --object/--contributor/--traverse/--depth/--weighted/--explain`) | subprocess from `tempfile::TempDir` via `assert_cmd` | inherits the slice-01 cli mechanism; explorer flags imply federated scope (WD-87) |

## Driven internal (real) — extends slice-01/03
| Port | Mechanism | Note |
|---|---|---|
| StoragePort (extended: query_by_object, query_by_contributor, query_attributed_for_scoring, traverse_graph) | real DuckDB file (recursive CTEs + UNION ALL projections over the EXISTING schema; NO new tables); seeded via `seed_federated_graph` (claim add + peer add + peer pull) | xtask check-arch extends no_cross_table_join_elides_author to scoring/traversal SQL (DELIVER) |
| scoring (pure core) | direct in-process invocation (no adapter; no probe) | layer-2 `@property` + DELIVER mutation testing is its Earned-Trust analog |

## Driven external / non-deterministic (fake) — extends slice-03
| Port | Fake | Note |
|---|---|---|
| (NONE NEW) | slice-03 `PeerPds` reused ONLY as the SEED mechanism for `peer_claims` | slice-04 adds no new external surface (WD-79/WD-92); scoring/traversal is local read-only |
```

---

## 12. Pre-requisites for compilation (DELIVER wiring expectations)

The slice-04 skeletons use `use scoring::...` (the NEW pure crate), the extended
`use ports::...` (the four new `StoragePort` methods + the new ADTs), the
extended `cli` explorer-flag dispatch, AND `use support::*` for the new seeder +
helpers + `use openlore_test_support::fixtures_scoring::*`. The intentional
consequence:

1. **`cargo build --tests` will fail until DELIVER's first slice-04 step
   (bootstrap)** lands:
   - The pure `crates/scoring` crate (ADTs `AttributedClaim`, `Contribution`,
     `WeightedPairing`, `WeightedView`, `WeightBucket`, `ScoringConfig` +
     `score`/`weight_bucket`/`ScoringConfig::DEFAULT` stubs) per
     component-boundaries.md §`crates/scoring`.
   - `crates/ports/` extended with `query_by_object`, `query_by_contributor`,
     `query_attributed_for_scoring`, `traverse_graph` +
     `AttributedClaim`/`GraphEdge`/`TraversalBound`/`ScoringFilter`/`GraphNode`/
     `TraversalResult`.
   - `crates/adapter-duckdb/` recursive-CTE + scoring-feed read-method stubs.
   - `crates/cli/` `graph query` dispatch for the 6 explorer flags (bodies
     `todo!()`).
   - `crates/test-support/src/fixtures_scoring.rs` fixture bodies + the
     `support/mod.rs` `seed_federated_graph` + `SeededGraph::add_peer_claim` +
     `run_openlore_network_disabled` + the scoring/traversal assertion-helper
     bodies.

2. **Once those land**, the slice-04 tests compile to "all `#[test]` functions
   panic with `todo!()`" → tests RED per Mandate 7. DELIVER then unskips one at
   a time (Release 1: GQE-1/10/26 + SC-1/3/5; Release 2: GQE-6/20 + SC-4/6;
   Release 3: GQE-16).

3. **Rust scaffold marker** per Mandate 7 + slice-01/02/03 precedent: every
   `#[test]` body panics via `todo!("DELIVER (slice-04): ...")` with a
   `// SCAFFOLD: true` comment-marker on the surrounding module. Detection via
   `grep -r "SCAFFOLD: true" tests/ crates/test-support/`. The new
   `fixtures_scoring.rs` module + the `support/mod.rs` slice-04 additions carry
   the same marker.

4. **Pre-DELIVER fail-for-right-reason gate (slice-04)** is deferred per the same
   logic as slice-01 DD-2 / slice-03 DD-FED-13 (DD-GRAPH-13): the scoring crate +
   StoragePort extensions + cli flags + test-support bodies land in DELIVER's
   first slice-04 step; only after that step do the tests compile and reach the
   `todo!()` panic that classifies as RED.

5. **`[[test]]` registration** for `graph_query_explore` + `scoring_core` is
   DEFERRED to that same bootstrap step (DD-GRAPH-14) — registering targets whose
   imports do not yet exist would fail `cargo build --tests` for a reason OTHER
   than the scaffold `todo!()` (BROKEN, not RED).

---

## 13. Definition of Done (DISTILL handoff to DELIVER)

- [x] All 33 slice-04 scenarios written as RED-ready Rust skeletons (27 GQE
      layer-3 + 6 SC layer-2).
- [x] Every NEW or EXTENDED CLI flag in ADR-020 covered by at least one
      subprocess scenario (§5).
- [x] Every NEW or EXTENDED driven adapter mapped (real, or the scoring pure
      core's layer-2 analog explicitly justified) — see §6.
- [x] Three Pillars verified (domain language, chained narrative, production
      composition — §9).
- [x] The single `# DISTILL: confirm` flag (Q-DELIVER-SCORE-1
      cross-project-triangulation-counts-as-breadth) resolved per WD-90 and
      bound to SC-3/SC-6/GQE-11/GQE-19 (§2).
- [x] Wave-decision reconciliation passed (0 contradictions DISCUSS WD-69..79 ↔
      DESIGN WD-80..93 — §1 + wave-decisions.md).
- [x] `traceability.md` written: every scenario → story → J-002 (+ sub-jobs) →
      WD lock → ADR → journey step → integration gate → KPI.
- [x] `wave-decisions.md` written: DD-GRAPH-1..DD-GRAPH-14.
- [x] All 6 integration gates from shared-artifacts-registry.md covered (§7).
- [x] All 6 KPIs mapped to acceptance scenarios (or to telemetry —
      KPI-GRAPH-1/5/6 — §8).
- [ ] **Pre-DELIVER fail-for-right-reason gate**: DEFERRED until DELIVER
      bootstraps the slice-04 scoring crate + StoragePort extensions + cli flags
      + test-support bodies. See §12 + DD-GRAPH-13.

Handoff-ready: **YES**, conditional on DELIVER's first slice-04 step landing the
`crates/scoring` crate + the four `StoragePort` methods + the cli explorer flags
+ the test-support seeder/fixture/helper bodies + the two `[[test]]`
registrations before running the suite the first time.

---

## 14. Open items for DELIVER

1. **Bootstrap the slice-04 surface** (DD-GRAPH-13): the pure `crates/scoring`
   crate + ADTs; the `StoragePort` extensions + ADTs in `crates/ports/`; the
   `adapter-duckdb` recursive-CTE + scoring-feed read-method stubs; the `cli`
   explorer-flag dispatch; the test-support seeder + helper + fixture bodies; the
   two `[[test]]` registrations (DD-GRAPH-14). All `probe()` per ADR-009 (the
   AUGMENTED adapter's extended probe per DESIGN §6.3 — recursive-CTE
   termination/cycle-safety/depth-bound). At that point the suite classifies RED.
2. **Materialize `seed_federated_graph`** (the 12 `FederatedGraphFixture`
   variants) + `SeededGraph::add_peer_claim` + the scoring/traversal assertion
   helpers in `support/mod.rs` (scaffolded by THIS DISTILL wave; bodies
   `todo!()`). See per-helper docstrings for the universe + contract.
3. **Materialize `crates/test-support/src/fixtures_scoring.rs`** fixture bodies
   (the `ScoringClaimSpec` recipes for each named fixture). Cross-check
   `EXPECTED_*` against `crates/scoring::ScoringConfig::DEFAULT`.
4. **Implement the pure `scoring` core** + the per-claim apportionment so
   `weight == sum(contributions.subtotal)` holds exactly (Gate 2 / SC-1); pin the
   exhaustive per-arm + bucket-threshold + mutation tests in
   `crates/scoring/src/`'s `#[cfg(test)] mod tests` (layer 1 — out of DISTILL
   scope, DELIVER's call). Lock the SCORE-1 bucket rule against SC-3 + SC-6.
5. **Implement the `cli` `VerbGraphQuery` extension**: the dimension router, the
   `WeightedRenderer` (formula + inputs + bucket + never-stored footer +
   `--explain` Contribution breakdown), the `TraversalRenderer` (tree +
   "Connections found" + omitted-edge line), the `SparseHonestyRenderer`. Fill
   the exact line formats GQE-10/11/16/19/20/22 assert (Q-DELIVER-6).
6. **Extend `adapter-duckdb`** with the cycle-safe depth-bounded recursive CTE +
   the attributed-projection scoring-feed/dimension queries (Q-DELIVER-1); every
   cross-store SQL string projects `author_did` (anti-merging structural layer).
7. **xtask check-arch slice-04 extension**: `no_cross_table_join_elides_author`
   extends to the scoring-feed + traversal SQL literals; `scoring` added to the
   pure-core allowlist (structural layer of the three-layer anti-merging
   enforcement; the behavioral layer is GQE-1/16/26 + SC-5, the type layer is the
   non-`Option` author_did ADTs).
8. **State-delta migration** (DD-GRAPH-10): migrate the load-bearing scenarios
   (GQE-14, GQE-16, GQE-11, GQE-20, GQE-1/26) to explicit
   `assert_state_delta(before, after, universe, expected)` form once the helper
   bodies are real. The universe entries are listed in wave-decisions.md
   §"Open questions handed to DELIVER" item 2.
9. **Tier B (state-machine PBT)** revisit decision per CM-G §10 / DD-GRAPH-9 —
   slice-05 (AppView) is the right surface once multi-user/cohort aggregation
   introduces a genuine indexer state machine.
10. **DEVOPS KPI instrumentation** (`graph.connection.surfaced` for KPI-GRAPH-1,
    `graph.query.duration_seconds` for KPI-GRAPH-6) — coordination point per
    outcome-kpis.md DEVOPS handoff; not a DELIVER deliverable, but the
    acceptance suite proves the connection is SURFACEABLE (GQE-20) so the rate
    telemetry has something to count.

---

## 15. References

- `docs/feature/openlore-scoring-graph/distill/wave-decisions.md`
- `docs/feature/openlore-scoring-graph/distill/traceability.md`
- `tests/acceptance/graph_query_explore.rs`
- `tests/acceptance/scoring_core.rs`
- `tests/acceptance/support/mod.rs` (slice-04 seeder + assertion helpers)
- `crates/test-support/src/fixtures_scoring.rs`
- DISCUSS: `docs/feature/openlore-scoring-graph/feature-delta.md` +
  `discuss/{user-stories,story-map,outcome-kpis,shared-artifacts-registry,journey-explore-the-graph-visual}.md`
- DESIGN: `docs/feature/openlore-scoring-graph/design/{architecture-design,component-boundaries,data-models,wave-decisions}.md`
- ADRs: ADR-020 (graph-query verb amendment), ADR-021 (DuckDB recursive-CTE
  traversal — WD-8 store revisit), ADR-022 (pure scoring core +
  anti-merging-in-aggregates)
- SSOT: `docs/product/journeys/explore-the-graph.yaml` +
  `docs/product/jobs.yaml` (J-002 + sub-jobs J-002a/b/c)
- Inherited from prior slices:
  - `docs/feature/openlore-federated-read/distill/{acceptance-tests,wave-decisions,traceability}.md` (STRUCTURAL TEMPLATE)
  - `tests/acceptance/{walking_skeleton,lexicon_conformance,federation_roundtrip,peer_*,counter_claim,federated_query,lexicon_counter_claim,scrape_*,scraper_domain}.rs`
  - `crates/test-support/src/{lib,fake_pds,fake_peer_pds,fixtures,fixtures_peer,fixtures_github,identity}.rs`
