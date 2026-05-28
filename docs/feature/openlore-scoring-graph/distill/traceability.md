# Traceability Matrix — openlore-scoring-graph (slice-04)

- **Wave**: DISTILL
- **Date**: 2026-05-28
- **Acceptance Designer**: Quinn

Every slice-04 acceptance scenario maps to (a) a user story, (b) the
Job-To-Be-Done / sub-job from `docs/product/jobs.yaml` (J-002 + sub-jobs
J-002a query-by-dimension / J-002b traverse-edges / J-002c weighted-scoring),
(c) the originating wave-decision lock (DISCUSS WD-N OR DESIGN WD-N), (d) the
DESIGN ADR(s) (020-022) constraining the observable contract, (e) the
`explore-the-graph.yaml` journey step, (f) the integration gate (Gate 1-6 from
shared-artifacts-registry.md), and (g) the KPI link.

Scenario IDs: `GQE-N` (layer-3 subprocess, `graph_query_explore.rs`); `SC-N`
(layer-2 pure-core property/example, `scoring_core.rs`).

---

## 1. Coverage matrix — `graph_query_explore.rs` (layer 3; 27 scenarios)

| # | Test name | Story | Job / sub-job | Wave-decision lock | ADR(s) | Journey step | Integration gate | KPI link |
|---|---|---|---|---|---|---|---|---|
| GQE-1 | `graph_query_by_object_groups_by_subject_with_per_author_attribution` | US-GRAPH-001 | J-002a | WD-75 (object dimension), WD-87 (federated default), WD-73 (anti-merging) | ADR-020 | step 2 (query by object) | **Gate 1 (dimension baseline)** | KPI-GRAPH-2 |
| GQE-2 | `graph_query_by_object_identical_content_different_authors_renders_two_rows` | US-GRAPH-001 | J-002a | WD-73 (anti-merging in aggregates) | ADR-020, ADR-022 | step 2 (Example 3) | drives Gate 1 | KPI-GRAPH-2 |
| GQE-3 | `graph_query_bare_subject_is_unchanged_from_prior_slice_behavior` | US-GRAPH-001 | J-002 | WD-87 (bare --subject unchanged) | ADR-020 | step 1 (subject regression) | n/a | n/a (regression) |
| GQE-4 | `graph_query_by_object_unknown_philosophy_returns_empty_with_suggestion_exit_zero` | US-GRAPH-001 | J-002a | US-GRAPH-001 AC (unknown URI exit 0) | ADR-020 | step 2 (failure_modes — unknown object) | n/a | n/a (UX) |
| GQE-5 | `graph_query_by_object_succeeds_with_network_disabled` | US-GRAPH-001 | J-002a | WD-79/WD-92 (local-first), I-GRAPH-7 | ADR-020 | step 2 (integration_checkpoint) | n/a | KPI-5 inherited |
| GQE-6 | `graph_query_by_contributor_lists_full_reasoning_trail_with_honest_framing` | US-GRAPH-002 | J-002a (+J-004 contributor lens) | WD-75 (contributor dimension) | ADR-020 | step 2b (query by contributor) | n/a (Gate 1 covered at weighted layer) | KPI-GRAPH-2 |
| GQE-7 | `graph_query_by_contributor_own_did_is_a_valid_self_review_annotated_you` | US-GRAPH-002 | J-002a | US-GRAPH-002 AC (relationship labels: you) | ADR-020 | step 2b (Example 2) | n/a | n/a (UX) |
| GQE-8 | `graph_query_by_contributor_absent_did_degrades_with_subscribe_pull_hint_exit_zero` | US-GRAPH-002 | J-002a | US-GRAPH-002 AC (absent DID exit 0 + hint) | ADR-020 | step 2b (failure_modes — DID not in graph) | n/a | n/a (UX) |
| GQE-9 | `graph_query_by_contributor_soft_removed_peer_labels_unsubscribed_cache` | US-GRAPH-002 | J-002a | WD-11 inherited (slice-03 relationship labels), US-GRAPH-002 AC | ADR-020 | step 2b (Example 4) | n/a | KPI-GRAPH-2 (attribution) |
| GQE-10 | `graph_query_weighted_ranks_projects_with_transparent_no_ml_formula` | US-GRAPH-003 | J-002c | WD-71 (no ML), WD-72 (display-only), WD-77/WD-86 (formula), WD-84 (--weighted) | ADR-020, ADR-022 | step 4 (weighted view) | **Gate 2 `weight_equals_formula`** | KPI-GRAPH-1, KPI-GRAPH-3 |
| GQE-11 | `graph_query_weighted_single_claim_single_author_renders_sparse_with_honesty_line` | US-GRAPH-003 | J-002c | WD-74 (sparse renders sparse), WD-90 (breadth guard) | ADR-022 | step 4 (Example 2 — sparse) | **Gate 3 `sparse_renders_sparse`** | **KPI-GRAPH-4 (release-blocking)** |
| GQE-12 | `graph_query_weighted_multi_author_support_raises_triangulation_weight` | US-GRAPH-003 | J-002c | WD-77 (author_distinct_bonus), WD-73 (attribution) | ADR-022 | step 4 (Example 3) | drives Gate 1 | KPI-GRAPH-1, KPI-GRAPH-2 |
| GQE-13 | `graph_query_weighted_conflicting_claims_both_contribute_nothing_dropped` | US-GRAPH-003 | J-002c | WD-73 (nothing merged/dropped), WD-85 (counter shown not applied) | ADR-022 | step 4 (Example 4 / failure_modes — conflicting) | drives Gate 1 | KPI-GRAPH-2 |
| GQE-14 | `graph_query_weighted_outputs_are_never_persisted_and_recompute_at_query_time` | US-GRAPH-003 | J-002c | WD-72 (never persisted), WD-89 (no persist path) | ADR-022 | step 4 (integration_checkpoint — re-run changes) | **Gate 4 `weight_and_bucket_never_persisted`** | KPI-GRAPH-3 (display-only) |
| GQE-15 | `graph_query_weighted_succeeds_with_network_disabled` | US-GRAPH-003 | J-002c | WD-79/WD-92 (local-first), I-GRAPH-7 | ADR-020 | step 4 (integration_checkpoint) | n/a | KPI-5 inherited |
| GQE-16 | `graph_query_explain_reproduces_weight_from_per_claim_arithmetic` | US-GRAPH-005 | J-002c | WD-71 (reproducible), WD-73 (decomposes), WD-88 (single scoring path) | ADR-022 | step 4 (--explain) | **Gate 1 `scoring_aggregate_preserves_attribution` + Gate 2** | KPI-GRAPH-2, **KPI-GRAPH-3 (load-bearing)** |
| GQE-17 | `graph_query_explain_on_sparse_subject_repeats_the_honesty_line` | US-GRAPH-005 | J-002c | WD-74 (sparse), WD-90 (breadth) | ADR-022 | step 4 (Example 2) | drives Gate 3 | KPI-GRAPH-4 |
| GQE-18 | `graph_query_explain_for_subject_absent_from_result_set_is_a_usage_error` | US-GRAPH-005 | J-002c | US-GRAPH-005 AC (usage error non-zero), DESIGN §5.2 invariant 5 | ADR-020 | step 4 (failure_modes — explain absent subject) | n/a | n/a (defense) |
| GQE-19 | `graph_query_explain_attributes_triangulation_bonus_to_the_contributor_who_earned_it` | US-GRAPH-005 | J-002c | WD-90/Q-DELIVER-SCORE-1 (triangulation breadth), WD-73 (attribution) | ADR-022 | step 4 (Example 4) | drives Gate 1 | KPI-GRAPH-2, KPI-GRAPH-3 |
| GQE-20 | `graph_query_traverse_surfaces_a_non_obvious_cross_project_contributor_connection` | US-GRAPH-004 | J-002b | WD-76 (traverse, no invented edges), WD-91 (cycle-safe/bounded) | ADR-020, ADR-021 | step 3 (traverse) | **Gate 5 `traversal_invents_no_edges`** | **KPI-GRAPH-1 (north star)** |
| GQE-21 | `graph_query_traverse_single_node_no_edges_renders_without_fabrication` | US-GRAPH-004 | J-002b | WD-76 (no fabrication) | ADR-021 | step 3 (Example 2 / failure_modes — single node) | drives Gate 5 | KPI-GRAPH-1 |
| GQE-22 | `graph_query_traverse_is_bounded_to_default_depth_two_and_reports_omitted_edges` | US-GRAPH-004 | J-002b | WD-76 (depth 2 default), WD-91 (bounded) | ADR-021 | step 3 (Example 3 / failure_modes — fan-out) | n/a (drives I-GRAPH-6) | KPI-GRAPH-6 (latency) |
| GQE-23 | `graph_query_traverse_depth_override_reveals_previously_omitted_real_edges` | US-GRAPH-004 | J-002b | WD-76 (--depth K override) | ADR-021 | step 3 (Example 3 — depth override) | drives Gate 5 | KPI-GRAPH-1 |
| GQE-24 | `graph_query_traverse_every_edge_maps_to_a_verifiable_signed_claim` | US-GRAPH-004 | J-002b | WD-76 (edge = 1 signed claim), WD-91 (GraphEdge.claim_cid non-Option), WD-73 (edge attribution) | ADR-021, ADR-022 | step 3 (Example 4 / UAT scenario 4) | **Gate 5 `traversal_invents_no_edges`** | KPI-GRAPH-1, KPI-GRAPH-2 |
| GQE-25 | `graph_query_traverse_succeeds_with_network_disabled` | US-GRAPH-004 | J-002b | WD-79/WD-92 (local-first), I-GRAPH-7 | ADR-020 | step 3 (integration_checkpoint) | n/a | KPI-5 inherited |
| GQE-26 | `graph_query_scoring_uses_the_same_numeric_confidence_shown_in_per_claim_rows` | US-GRAPH-003 | J-002c | WD-10/I-6 inherited (numeric-only), Gate 6 | ADR-022 | steps 1+4 (confidence consistency) | **Gate 6 `scoring_uses_numeric_confidence`** | KPI-GRAPH-3 |
| GQE-27 | `graph_query_weighted_end_to_end_wires_scoring_feed_without_persisting_outputs` | US-GRAPH-006 | infrastructure (→J-002) | WD-83 (StoragePort extension), WD-88 (anti-merging), WD-89 (no persist) | ADR-020, ADR-022 | step 4 (infra wiring) | **Gate 1 + Gate 4** | supports KPI-GRAPH-1..4 |

Layer-3 error-path scenarios (explicit `@error`): GQE-4, GQE-8, GQE-18 (+ GQE-21
no-fabrication edge). Ratio 3/27 = 11.1% (`@error`-tagged); read-surface
rationale in acceptance-tests.md §4 (the load-bearing slice-04 risk is the
guardrail surface — anti-merging / sparse-honesty / never-persisted /
no-invented-edges — not input-validation sad paths; the substrate sad paths
live at the `adapter-duckdb` probe layer per ADR-021).

---

## 2. Coverage matrix — `scoring_core.rs` (layer 2; 6 scenarios; 4 `@property`)

| # | Test name | Story | Job | Wave-decision lock | ADR(s) | Integration gate | KPI link | Mode |
|---|---|---|---|---|---|---|---|---|
| SC-1 | `scoring_weight_equals_sum_of_contributions_property` | US-GRAPH-003, US-GRAPH-006 | J-002c | WD-71 (reproducible), WD-77 (formula) | ADR-022 | **Gate 2 `weight_equals_formula`** | KPI-GRAPH-3 | `@property` |
| SC-2 | `scoring_score_is_deterministic_property` | US-GRAPH-006 | J-002c | WD-71 (reproducible precondition), DESIGN §5.1 invariant 2 | ADR-022 | supports Gate 2 | KPI-GRAPH-3 | `@property` |
| SC-3 | `scoring_single_author_single_claim_is_sparse_at_any_confidence_property` | US-GRAPH-003 | J-002c | WD-74 (sparse), WD-90 (breadth guard) | ADR-022 | **Gate 3 `sparse_renders_sparse`** | **KPI-GRAPH-4** | `@property` |
| SC-4 | `scoring_multi_author_outweighs_single_author_at_equal_confidence_property` | US-GRAPH-003 | J-002c | WD-77 (author_distinct_bonus monotonicity) | ADR-022 | supports Gate 1/2 | KPI-GRAPH-1 | `@property` |
| SC-5 | `scoring_two_author_pairing_decomposes_to_two_attributed_contributions` | US-GRAPH-005, US-GRAPH-006 | J-002c | WD-73 (anti-merging in aggregates), WD-88 (three-layer type level) | ADR-022 | **Gate 1 (type-level layer)** | KPI-GRAPH-2 | example |
| SC-6 | `scoring_cross_project_triangulation_counts_as_breadth_lifts_out_of_sparse` | US-GRAPH-003 | J-002c | WD-90 + **Q-DELIVER-SCORE-1** (breadth-counts bucket rule) | ADR-022 | drives Gate 3 | KPI-GRAPH-4 | example |

Per Mandate 9: SC-1..SC-4 are `@property` (PBT full, layer 2); SC-5..SC-6 are
example-pinned (worked-arithmetic fixtures). ALL layer-3 GQE-* are example-only
(Mandate 11). ZERO proptest at layer 3+.

---

## 3. Total slice-04 scenarios

27 (GQE, layer 3) + 6 (SC, layer 2) = **33 scenarios** authored, all RED-ready
as `todo!()` scaffolds with `// SCAFFOLD: true` module markers. Within the
~25-35 target band for 6 stories.

Per-file:
- `graph_query_explore.rs` — 27 (subprocess; example-only per Mandate 11)
- `scoring_core.rs` — 6 (pure-core; 4 `@property` + 2 example per Mandate 9 layer 2)

---

## 4. Story coverage (every story has >= 1 acceptance test)

| Story | Title | Test count | Test IDs |
|---|---|---|---|
| US-GRAPH-001 | Query by object (philosophy) + subject, attribution preserved | 5 | GQE-1, GQE-2, GQE-3, GQE-4, GQE-5 |
| US-GRAPH-002 | Query by contributor (DID) — one developer's reasoning trail | 4 | GQE-6, GQE-7, GQE-8, GQE-9 |
| US-GRAPH-003 | Transparent weighted/scored view; sparse renders sparse | 9 | GQE-10, GQE-11, GQE-12, GQE-13, GQE-14, GQE-15, GQE-26 + SC-1, SC-3, SC-4, SC-6 (layer-2 properties) |
| US-GRAPH-004 | Traverse contributor<->project<->philosophy edges | 6 | GQE-20, GQE-21, GQE-22, GQE-23, GQE-24, GQE-25 |
| US-GRAPH-005 | Audit a weight with `--explain` per-claim arithmetic | 5 | GQE-16, GQE-17, GQE-18, GQE-19 + SC-5 (layer-2 type contract) |
| US-GRAPH-006 | Bootstrap pure `scoring` core + read-side extensions (`@infrastructure`) | 4 | GQE-27 + SC-1, SC-2, SC-5 (the pure-core wiring + determinism + reproducibility + type contract) |

Every story has >= 4 scenarios. The most-tested is US-GRAPH-003 (the
load-bearing transparent-weighting story; 2.5 days / largest story in DISCUSS)
because it carries Gate 2 (transparency) + Gate 3 (sparse-honesty) + Gate 6
(numeric confidence) + the layer-2 formula properties.

---

## 5. Job coverage (J-002 + sub-jobs)

| Job / sub-job | In slice-04? | Test count | Scenarios |
|---|---|---|---|
| J-002 Explore the philosophy graph to inform a decision | YES — primary (walking-skeleton job for this feature, WD-69) | 33 (all slice-04 scenarios) | every GQE / SC scenario |
| J-002a Query the graph by dimension (subject/object/contributor) | YES — LOAD-BEARING | 9 | GQE-1..GQE-9 |
| J-002b Traverse contributor<->project<->philosophy edges | YES — LOAD-BEARING | 6 | GQE-20..GQE-25 |
| J-002c See transparent, auditable adherence weighting | YES — LOAD-BEARING | 16 | GQE-10..GQE-19, GQE-26, SC-1..SC-6 |
| J-003 Read another developer's federated claims with weighting | PARTIAL — built-on | 9 | the contributor lens (GQE-6..9) reads the slice-03 `peer_claims`; the weighted view extends `graph query` |
| J-004 Evaluate a contributor's body of work through a philosophy lens | PARTIAL — realized | 4 | GQE-6 (contributor lens) + GQE-10/12/16 (adherence weighting over the local graph) |

---

## 6. Wave-decision coverage (DISCUSS WD-69..79 + DESIGN WD-80..93)

Every locked WD-N + OD-GRAPH-N + Q-DELIVER-SCORE-1 that touches user-observable
behavior maps to >= 1 acceptance scenario:

### DISCUSS

| Wave decision | Coverage |
|---|---|
| WD-69 (slice-04 = SIBLING feature; J-002 walking-skeleton job) | This entire DISTILL wave's directory + all scenarios trace to J-002 (§5) |
| WD-70 (P-002 primary, P-001 secondary in explorer hat) | All scenarios reference P-002 personas (Maria/Rachel/Tobias/Aanya) per the user-stories fixtures |
| WD-71 (scoring transparent / no ML; formula displayed; --explain reproduces) | GQE-10 (formula printed + "no ML"), GQE-16 (reproduce by hand), SC-1 (weight==sum), SC-2 (deterministic) |
| WD-72 (weights derived + display-only, never persisted) | GQE-14 (never persisted + recompute), GQE-27 (no persist e2e), SC-1 (display-only by construction) |
| WD-73 (anti-merging extends to aggregates) | GQE-2, GQE-12, GQE-13, GQE-16, GQE-19, GQE-24, SC-5 (type-level decomposition) |
| WD-74 (sparse renders sparse) | GQE-11 (sparse + honesty line), GQE-17 (explain repeats it), SC-3 (sparse at any confidence) |
| WD-75 (dimensions by subject/object/contributor) | GQE-1/2/4 (object), GQE-3 (subject), GQE-6/7/8/9 (contributor) |
| WD-76 (traverse, default depth 2, no invented edges) | GQE-20 (no invented edges), GQE-21 (no fabrication), GQE-22 (bounded depth 2), GQE-23 (--depth override), GQE-24 (edge=signed claim) |
| WD-77 (closed-form formula constants) | GQE-10 (formula printed), GQE-16/19 (worked arithmetic), SC-1/SC-4 (formula properties) |
| WD-78 (store revisit DESIGN-internal) | INVISIBLE to scenarios by design (storage-neutral contract); the AUGMENT decision (WD-81) is asserted indirectly via the seeded-real-DuckDB read path |
| WD-79 (local scope only; network-disabled) | GQE-5, GQE-15, GQE-25 (network-disabled success) |
| OD-GRAPH-1 (store: augment) | WD-81 LOCKED; seeded-real-DuckDB read path (no graph-store crate) — invisible to scenarios |
| OD-GRAPH-2 (countered claims contribute normally) | WD-85; GQE-13 (conflicting/countered both contribute, nothing dropped) |
| OD-GRAPH-3 (formula constants WD-77 defaults) | WD-86; SC-1/SC-4 + `fixtures_scoring::EXPECTED_*` |
| OD-GRAPH-4 (explorer verbs imply federated) | WD-87; GQE-1 (object includes peers), GQE-3 (bare --subject does NOT) |

### DESIGN

| Wave decision | Coverage |
|---|---|
| WD-80 (port-method + pure-crate extension, not re-architecture) | DISTILL constraint — two test files extend the existing flat layout; `crates/scoring` is the one new pure crate |
| WD-81 (AUGMENT DuckDB; no graph store) | The seeded-real-DuckDB read path (recursive CTEs over the existing schema); the cyclic-graph fixture backs the adapter probe (DELIVER), not an acceptance scenario |
| WD-82 (NEW pure `scoring` crate) | SC-1..SC-6 invoke `scoring::*` directly (layer-2 pure-core direct invocation) |
| WD-83 (StoragePort extension, not new port) | GQE-27 (the scoring-feed wiring proof); all GQE-* exercise the four new read methods via the CLI |
| WD-84 (explorer flags on `graph query`, not new verbs) | every GQE-* exercises a flag on the existing `graph query` verb |
| WD-85 (countered claims contribute normally, counter shown) | GQE-13 (conflicting contribute per confidence; nothing silently subtracted) |
| WD-86 (formula constants compile-time const SSOT) | SC-1/SC-4 (the formula properties); `fixtures_scoring::EXPECTED_*` mirrors the defaults |
| WD-87 (explorer verbs imply federated; bare --subject unchanged) | GQE-1 (object includes peers by default), GQE-3 (bare --subject byte-identical to slice-01) |
| WD-88 (anti-merging three-layer: type/structural/behavioral) | type: SC-5 + GQE-24 (non-Option author_did); structural: xtask (DELIVER); behavioral: GQE-1/16/26 + SC-5 |
| WD-89 (no persistence code path for weights/buckets) | GQE-14, GQE-27 (`assert_weight_not_persisted` scans tables + artifacts) |
| WD-90 (sparse driven by evidence breadth, not weight magnitude) | SC-3 (sparse at any confidence), SC-6 (triangulation counts as breadth), GQE-11/17 |
| WD-91 (traversal invents no edges + bounded + cycle-safe) | GQE-20/21/22/23/24 (no invented edges, bounded, depth override); cycle-safety = adapter probe (DELIVER) |
| WD-92 (no external integration; network-disabled success) | GQE-5/15/25 (network-disabled) |
| WD-93 (ADR-020-022 accepted) | INTENTIONALLY UNTESTED — process decision |
| Q-DELIVER-SCORE-1 (cross-project triangulation counts as breadth) | SC-6 (cargo NOT sparse via span; tokio stays sparse) + SC-3 (single-claim no-span sparse at any confidence) + GQE-11 (sparse) + GQE-19 (cargo triangulation explained) |

---

## 7. Integration gate coverage (shared-artifacts-registry.md, Gates 1-6)

| Gate | Description | Where asserted (load-bearing) | Mandatory for KPI |
|---|---|---|---|
| Gate 1 | `scoring_aggregate_preserves_attribution` (anti-merging in aggregates) | GQE-16 (--explain decomposes to per-author per-cid) + GQE-1/26 (dimension/feed attribution) + SC-5 (type-level decomposition) + GQE-27 (e2e) | **KPI-GRAPH-2 (release-blocking)** |
| Gate 2 | `weight_equals_formula` (scoring transparency) | GQE-16 (--explain running sum == displayed weight) + SC-1 (weight == sum(contributions)) | **KPI-GRAPH-3 (release-blocking)** |
| Gate 3 | `sparse_renders_sparse` (J-002 anxiety mitigation) | GQE-11 (sparse + honesty line) + SC-3 (sparse at any confidence) + GQE-17 (--explain repeats it) | **KPI-GRAPH-4 (release-blocking)** |
| Gate 4 | `weight_and_bucket_never_persisted` (display-only discipline) | GQE-14 (never persisted + recompute) + GQE-27 (e2e no persist) | KPI-GRAPH-3 (display-only) |
| Gate 5 | `traversal_invents_no_edges` (auditability) | GQE-20 ("Traversal does not invent edges" + Connections found) + GQE-24 (every edge = signed claim) + GQE-21 (no fabrication) | KPI-GRAPH-1 |
| Gate 6 | `scoring_uses_numeric_confidence` (no silent rounding) | GQE-26 (dimension-row confidence == --explain base value) | KPI-GRAPH-3 |

All six gates have >= 1 acceptance test. Gates 1, 2, 3 are the release-blocking
guardrails (KPI-GRAPH-2/3/4 != 100% halts release per outcome-kpis.md
Disprovers). The anti-merging-in-aggregates (Gate 1) + scoring-transparency
(Gate 2) are the load-bearing carries of slice-03 I-FED-1 + WD-71 into the
aggregate surface.

---

## 8. KPI coverage

| KPI | Description | Acceptance coverage | Type |
|---|---|---|---|
| KPI-GRAPH-1 | Surface a non-obvious connection in a single session (>=60%) | GQE-20 (the connection is SURFACEABLE — "Connections found" callout); GQE-12 (triangulation lift); SC-4 (triangulation monotonicity). The RATE is DEVOPS telemetry (`graph.connection.surfaced`), not asserted at the acceptance boundary | North Star (Outcome) |
| KPI-GRAPH-2 | Zero attribution loss in any aggregate (100%) | GQE-1/2/12/13/16/19/24/26/27 + SC-5 (Gate 1, the load-bearing anti-merging assertions) | Guardrail (release-blocking) |
| KPI-GRAPH-3 | Every weight reproducible by hand (100%) | GQE-16 (--explain reproduce) + SC-1 (weight==sum) + GQE-10 (formula printed) + GQE-14 (display-only) + GQE-26 (numeric confidence) | Guardrail (release-blocking) |
| KPI-GRAPH-4 | Sparse renders sparse, zero manufactured confidence (100%) | GQE-11 (load-bearing) + SC-3 (sparse at any confidence) + GQE-17 | Guardrail (release-blocking) |
| KPI-GRAPH-5 | Referenced justification per dogfood explorer (>=1 / 30 days) | NO acceptance assertion — measured via 30-day survey per outcome-kpis.md; the acceptance suite proves the query result EXISTS to cite (GQE-10/16/20) | Leading (Outcome) |
| KPI-GRAPH-6 | Local-read latency < 5s for <=200 claims | NO hard acceptance assertion — telemetry (`graph.query.duration_seconds`) per outcome-kpis.md; GQE-22 asserts the depth bound that keeps traversal responsive | Leading (Outcome) |

KPI-GRAPH-1, KPI-GRAPH-5, KPI-GRAPH-6 are telemetry-measured (production), not
asserted at the acceptance boundary — by design, per outcome-kpis.md
Measurement Plan. The acceptance suite proves the BEHAVIOR is correct +
surfaceable; the RATES/LATENCIES are DEVOPS dashboards.

---

## 9. Cross-feature inheritance from prior slices

Slice-04 INHERITS without modification:

| Inherited | Status in slice-04 |
|---|---|
| Slice-01 walking-skeleton + lexicon + federation-roundtrip scenarios | UNCHANGED (slice-01 is the umbrella WS) |
| Slice-02 scraper scenarios + `scraper_domain.rs` layer-2 file | UNCHANGED; scraper-signed claims participate in scoring as normal author claims (WD-58, no special weight) |
| Slice-03 peer scenarios + `PeerPds` + `build_verifiable_peer_records*` | EXTENDED — slice-04 reuses the peer seam for graph seeding (DD-GRAPH-5); `peer_claims` is read by the contributor/object/scoring queries |
| `tests/acceptance/support/mod.rs` | EXTENDED with the slice-04 seeder + assertion helpers (DD-GRAPH-5) |
| `crates/test-support/src/lib.rs` | EXTENDED with the `fixtures_scoring` re-export (DD-GRAPH-6) |
| `tests/common/state_delta.rs` (Rust port) | INHERITED; consumed via named helpers (DD-GRAPH-10) |
| WD-10 / I-6 (numeric-only persistence; display-only buckets) | INHERITED + EXTENDED to weight buckets (Gate 4 / GQE-14/27) |
| Slice-01/03 ADR-001..016 + slice-02 ADR-017..019 | All still binding; slice-04 adds ADR-020..022 |

---

## 10. Changelog

- 2026-05-28 — Quinn — initial traceability matrix for slice-04. All 33
  scenarios (27 GQE layer-3 + 6 SC layer-2) mapped to story / job / sub-job /
  wave-decision / ADR / journey step / integration gate / KPI. Zero un-traced
  scenarios. The Q-DELIVER-SCORE-1 `# DISTILL: confirm` flag mapped to its WD-90
  resolution + the SC-3/SC-6/GQE-11/GQE-19 scenario bindings.
