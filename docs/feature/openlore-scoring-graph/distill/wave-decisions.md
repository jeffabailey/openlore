# Wave Decisions — DISTILL — openlore-scoring-graph (slice-04)

- **Wave**: DISTILL
- **Date**: 2026-05-28
- **Acceptance Designer**: Quinn (nw-acceptance-designer)
- **Feature**: openlore-scoring-graph (slice-04)
- **Inherits**: DISCUSS WD-69..WD-79 (feature-delta.md) + DESIGN WD-80..WD-93 +
  ADR-001..ADR-022; slice-01 DISTILL DD-1..DD-13 + slice-03 DISTILL
  DD-FED-1..DD-FED-14 + slice-02 DISTILL DD-SCR-* apply where the slice-04
  surface is symmetric.

This file records DISTILL-wave decisions (DD-GRAPH-N prefix to keep the
namespace distinct from slice-01 DD-N / slice-03 DD-FED-N / slice-02 DD-SCR-N).
Decisions that point at a test artifact (a file under `tests/acceptance/` or
`crates/test-support/src/`) are binding for DELIVER unless re-opened.

---

## Wave-Decision Reconciliation result

**Reconciliation passed — 0 contradictions** between DISCUSS WD-69..WD-79
(+ OD-GRAPH-1..4 accepted at default) and DESIGN WD-80..WD-93. DESIGN's
WD-81/85/86/87 RESOLVE the four DISCUSS Open Decisions; WD-90 + Q-DELIVER-SCORE-1
RESOLVE the single `# DISTILL: confirm` flag (the
cross-project-triangulation-counts-as-breadth bucket rule). See the
reconciliation matrix below; this DISTILL wave inherits the resolutions as
binding.

| DISCUSS lock | DESIGN counterpart | Verdict |
|---|---|---|
| WD-71 scoring transparent / no ML | WD-86 (const SSOT) + WD-88 (scoring in pure Rust core, not SQL) | CONSISTENT |
| WD-72 weights derived + display-only, never persisted | WD-89 (no persistence code path) | CONSISTENT |
| WD-73 anti-merging extends to aggregates | WD-88 (three-layer: type / structural / behavioral) | CONSISTENT |
| WD-74 sparse renders sparse | WD-90 (breadth guard drives the bucket, not weight magnitude) | CONSISTENT |
| WD-75 dimensions by subject/object/contributor | WD-84 (explorer flags on `graph query`) | CONSISTENT |
| WD-76 traverse, default depth 2, no invented edges | WD-91 (cycle-safe, depth-bounded, GraphEdge.claim_cid non-Option) | CONSISTENT |
| WD-77 closed-form formula shape | WD-86 (WD-77 defaults as compile-time const) | CONSISTENT |
| WD-78 store revisit is DESIGN-internal | WD-81 (AUGMENT DuckDB; DESIGN exercised its call) | CONSISTENT |
| WD-79 local scope only | WD-92 (no external integration, network-disabled success) | CONSISTENT |

**Q-DELIVER-SCORE-1 resolution (the `# DISTILL: confirm` flag)**: per WD-90 +
Q-DELIVER-SCORE-1 + the data-models.md worked examples, the ONE consistent
bucket rule DISTILL asserts is: **cross-project triangulation by the SAME author
counts toward evidence breadth for the bucket** — so a single-claim pairing
whose author spans >=2 distinct subjects on the object is NOT `[SPARSE]`
(cargo, lifted by Rachel's cargo+nixpkgs span), while a single claim with NO
triangulation AND NO co-author STAYS `[SPARSE]` regardless of confidence
magnitude (tokio at 0.50; a 0.99 single claim too). This is pinned at layer 2 by
`scoring_core.rs::scoring_cross_project_triangulation_counts_as_breadth_lifts_out_of_sparse`
(SC-6) + `..._is_sparse_at_any_confidence_property` (SC-3), and at layer 3 by
`graph_query_explore.rs` GQE-11 (sparse) + GQE-19 (cargo triangulation explained).

DEVOPS-wave artifacts are absent (`docs/feature/openlore-scoring-graph/devops/`
does not exist as of this writing). Per `nw-distill` Graceful Degradation
matrix: WARN + apply default environment matrix; DO NOT block. Slice-04's
acceptance scenarios do not depend on a per-environment fixture cross-product
(the test seam is the subprocess + REAL DuckDB seeded with own + peer claims via
the slice-03 `PeerPds` + `claim add`/`peer pull` pattern; scoring/traversal is
local read-only over that real store).

---

## Locked decisions

| # | Decision | Rationale | Status |
|---|---|---|---|
| DD-GRAPH-1 | Slice-04 acceptance tests inherit the slice-01/02/03 framework: Rust std `#[test]` with snake_case function names encoding the scenario (`graph_query_*`, `scoring_*`). No new test-framework dependency; no `.feature` files. | Symmetric with slice-01 DD-1 / slice-03 DD-FED-1 / slice-02 DD-SCR-1; preserves `cargo test --test <file>` ergonomics. | LOCKED |
| DD-GRAPH-2 | Slice-04 is a READ slice — NO scenario writes or signs a claim as the behavior under test. Claims are SEEDED as preconditions (own via `claim add`, peer via `peer add` + `peer pull`); scoring/traversal/dimension queries READ them. The graph is seeded into the REAL DuckDB; NO new external fake is introduced. | WD-79/WD-92: scoring/traversal is local read-only analysis over the existing slice-01/03 stores. A new fake would model an external surface that does not exist; the slice-03 `PeerPds` double + the real `claim add`/`peer pull` verbs already populate the real store. | LOCKED |
| DD-GRAPH-3 | Layered placement (Mandate 9): the pure scoring-formula PROPERTIES live at LAYER 2 in `scoring_core.rs` (PBT full — `@property` via proptest); the user-visible explorer scenarios live at LAYER 3 in `graph_query_explore.rs` (subprocess, example-only — sad paths enumerated, never PBT-generated per Mandate 11). | The transparency/sparse/triangulation invariants are pure-function properties best expressed generatively at the cheap layer-2 boundary; the verb-orchestration + rendering behavior is example-pinned at layer 3 because each example is a real subprocess invocation with real DuckDB I/O. Symmetric with slice-02 `scraper_domain.rs` (layer 2) + `scrape_*.rs` (layer 3) and slice-03 `lexicon_counter_claim.rs` + `peer_*.rs`. | LOCKED |
| DD-GRAPH-4 | FLAT test placement under `tests/acceptance/` matching slice-01/02/03. Two new files alongside the existing ones: `graph_query_explore.rs` (layer-3 subprocess explorer scenarios) + `scoring_core.rs` (layer-2 pure-formula properties). NOT nested. | Preserves `cargo test --test <file>` ergonomics; the two new files are clearly labeled by concern (the explorer verb surface vs the pure scoring core). Symmetric with slice-03 DD-FED-4 + slice-02; rejected nested layout for the same reasons (DD-FED-14). | LOCKED |
| DD-GRAPH-5 | The shared `tests/acceptance/support/mod.rs` is EXTENDED, not duplicated. Slice-04 adds the `FederatedGraphFixture` enum + `seed_federated_graph` orchestrator + `SeededGraph`/`AddedPeerClaim` handles + `run_openlore_network_disabled` + the scoring/traversal assertion helpers (`assert_weight_not_persisted`, `assert_weight_decomposes_to_per_author`, `assert_sparse_rendered_as_sparse`, `assert_explain_sums_to_weight`, `assert_every_edge_has_backing_claim`). | One source of truth for `TestEnv`, the subprocess seam, and the seeding/assertion helpers across all acceptance tests. The seeder reuses the slice-03 `build_verifiable_peer_records*` + `PeerPds` machinery already in `support/mod.rs` — no duplication. | LOCKED |
| DD-GRAPH-6 | Slice-04 adds `crates/test-support/src/fixtures_scoring.rs` (canonical GRAPH-SHAPE fixtures: `ScoringClaimSpec` + the named worked-example/sparse/span/multi-author/conflict/dense fixtures), re-exported flat from `lib.rs`. Symmetric with `fixtures_peer.rs` / `fixtures_github.rs`. The seeder consumes these specs; the layer-2 `scoring_core.rs` may consume the worked-arithmetic targets directly. | Keeps the slice-04 graph fixtures declarative + in one place; the worked-arithmetic constants (author bonus 0.25, triangulation bonus 0.5) are re-stated as EXPECTED targets (the SSOT stays `crates/scoring::ScoringConfig::DEFAULT`). | LOCKED |
| DD-GRAPH-7 | Pure-core unit tests for `scoring::score` / `scoring::weight_bucket` / the per-claim apportionment + the exact bucket thresholds + mutation testing of `formula.rs`/`bucket.rs` are DELIVER's responsibility (inner TDD loop), NOT DISTILL's. The slice-04 layer-2 `scoring_core.rs` exposes the four LOAD-BEARING properties (reproducibility / determinism / sparse / triangulation) + the anti-merging type contract + the SCORE-1 bucket rule at the in-memory acceptance layer; the exhaustive per-arm coverage lives in `crates/scoring/src/`'s `#[cfg(test)] mod tests`. | Symmetric with slice-03 DD-FED-7 + slice-02 DD-SCR-7. Pure functions live in DELIVER's inner loop; the outer-loop acceptance suite asserts the CONTRACT (the load-bearing invariants), not exhaustive unit coverage. | LOCKED |
| DD-GRAPH-8 | Layer-2 scoring properties (SC-1..SC-6) live in their own file `scoring_core.rs` rather than being appended to slice-02's `scraper_domain.rs`. Each is the slice-04 scoring concern; slice-02's file is "the slice-02 derivation core". | Keeps the slice-02 file pristine (locked + committed + green); slice-04's scoring concerns are a focused surface that deserves its own role-prefixed file + test-binary boundary (faster `cargo test --test scoring_core`). Symmetric with slice-03 DD-FED-8. | LOCKED |
| DD-GRAPH-9 | Tier B (state-machine PBT) is NOT added for slice-04 per Mandate 10 evaluation. The explore-the-graph journey IS >=3 chained scenarios (query -> traverse -> weight -> audit) AND the scoring input space (authors x subjects x objects x confidences) is domain-rich — so it QUALIFIES on both Tier-B triggers. BUT the load-bearing aggregate invariants (reproducibility, anti-merging-in-aggregates, sparse-honesty, triangulation-monotonicity) are over a PURE function whose model is NOT a state machine (Hebert ch.11: the model shape, not user-perceived states) — they are FORALL properties best expressed as the layer-2 `@property` suite in `scoring_core.rs`, which already explores the generative input space. A `RuleBasedStateMachine` would add an `InMemoryComposition` + a command/postcondition model for a system that has no meaningful command sequence (scoring is a pure read; there is no mutate-then-observe state transition to model). | Mandate 10 + the `nw-distill` state-machine-PBT trigger (the MODEL must be a state machine, not merely a multi-step journey). Slice-04's richness is in the INPUT space (covered by layer-2 PBT full), not in a stateful command protocol. The two-tier shared-vocabulary contract would be ceremony with no detection gain here. **Re-evaluate at slice-05 (AppView)** where multi-user/cohort aggregation introduces a genuine indexer state machine (subscribe/index/evict transitions). | LOCKED — revisit at slice-05 |
| DD-GRAPH-10 | State-delta + Universe assertions (Mandate 8) at layer 3 (subprocess acceptance) are written via NAMED assertion helper functions in `support/mod.rs` (e.g. `assert_weight_not_persisted`, `assert_weight_decomposes_to_per_author`, `assert_sparse_rendered_as_sparse`, `assert_explain_sums_to_weight`, `assert_every_edge_has_backing_claim`), NOT via `assert_state_delta(before, after, universe, expected)` directly. The Rust `state_delta` port at `tests/common/state_delta.rs` was bootstrapped by slice-01; slice-04 INHERITS it. DELIVER MAY migrate the load-bearing scenarios (GQE-14 never-persisted, GQE-16 explain-sums, GQE-11 sparse, GQE-20 traversal, GQE-1/GQE-26 attribution) to explicit `assert_state_delta` form once the helper bodies are real. The universe entries are PORT-EXPOSED names (`cli.graph_query.bucket[subject]`, `cli.graph_query.explain.running_sum[subject]`, `storage.duckdb.no_weight_or_bucket_column`, `cli.graph_query.traverse.edge_cids`) — NEVER internal scoring/StoragePort struct fields per Mandate 8. | Two-stage bootstrap symmetric with slice-01 DD-3 / slice-03 DD-FED-10: DISTILL declares the contract via named helper signatures (the helper names already encode the universe shape); DELIVER materializes the universe wiring as each scenario goes green. The migration is mechanical. | LOCKED |
| DD-GRAPH-11 | The Project Infrastructure Policy file at `docs/architecture/atdd-infrastructure-policy.md` is STILL NOT written by this DISTILL wave. The orchestrator brief limits writes to `docs/feature/openlore-scoring-graph/distill/` + `tests/acceptance/` + `crates/test-support/src/`. The slice-04 additions to the inherited inline policy are documented in `acceptance-tests.md §11`; the slice-01/02/03 + slice-04 policy entries should land at the project-local file on a future wave whose orchestrator scope permits. | Continues slice-01 DD-11 + slice-03 DD-FED-11 deferral; cross-wave write-surface convention unchanged. | LOCKED |
| DD-GRAPH-12 | Layer-2 scoring properties (SC-1..SC-4) use proptest per slice-02 precedent (the `@property` PBT-full layer-2 mode, Mandate 9). SC-5 (anti-merging type contract) + SC-6 (SCORE-1 bucket rule) are example-pinned (a single worked-arithmetic fixture each). The layer-3 explorer scenarios (GQE-*) are ALL example-only per Mandate 11 — ZERO proptest at layer 3+. | Layered test discipline: the scoring INVARIANTS are pure-data properties best expressed at layer 2 with generators; the verb-orchestration + rendering behavior is example-pinned at layer 3 because each example is a real subprocess invocation with real DuckDB I/O. Symmetric with slice-02 DD-SCR-* + slice-03 DD-FED-12. | LOCKED |
| DD-GRAPH-13 | Pre-DELIVER fail-for-right-reason gate (slice-04) runs in DELIVER's first slice-04 step (the bootstrap step), AFTER (a) the pure `crates/scoring` crate + its ADTs (`AttributedClaim`, `Contribution`, `WeightedPairing`, `WeightedView`, `WeightBucket`, `ScoringConfig`) + `score`/`weight_bucket` entry points are scaffolded, AND (b) the `StoragePort` extensions (`query_by_object`, `query_by_contributor`, `query_attributed_for_scoring`, `traverse_graph`) + the `AttributedClaim`/`GraphEdge`/`TraversalBound`/`ScoringFilter`/`GraphNode`/`TraversalResult` ADTs are scaffolded in `crates/ports/`, AND (c) `adapter-duckdb` gains the four read-method stubs, AND (d) the `cli` `graph query` dispatch parses the 6 explorer flags (bodies `todo!()`), AND (e) the `seed_federated_graph` orchestrator + the scoring/traversal assertion-helper bodies + the `fixtures_scoring.rs` fixture bodies are materialized in test-support. At that point every slice-04 acceptance test MUST classify as RED (panic at `todo!()`), not BROKEN (import error, missing trait method, missing fixture, missing crate). | Same logic as slice-01 DD-2 / slice-03 DD-FED-13: the source tree changes shape under DELIVER's hand before the suite can compile (the `scoring` crate does not exist yet; `StoragePort` lacks the four methods; the cli lacks the flags). The gate runs at the first moment the suite compiles. The gate is still HARD — any scenario in BROKEN state at that moment blocks the start of the outside-in TDD loop. | LOCKED |
| DD-GRAPH-14 | `Cargo.toml` `[[test]]` registration for the two new test targets (`graph_query_explore`, `scoring_core`) is DEFERRED to DELIVER's bootstrap step (NOT written by this DISTILL wave), continuing the slice-03 DD-FED convention. The new test-support module (`fixtures_scoring`) is wired into `crates/test-support/src/lib.rs` NOW (a re-export edit), but the `cli/Cargo.toml` `[[test]]` blocks land when DELIVER bootstraps the scoring crate + StoragePort + cli flags. | Registering a `[[test]]` target whose imports (`scoring::*`, the extended `ports::*`, the cli explorer flags) do not yet exist would fail `cargo build --tests` for a reason OTHER than the scaffold `todo!()` — a BROKEN classification. DELIVER registers the targets in the SAME bootstrap step that lands the crate + ports + flags, so the first compile reaches the `todo!()` panic (RED). | LOCKED |

---

## Inheritance from prior-slice DISTILL (still binding)

| Prior-slice DD | Status in slice-04 |
|---|---|
| Slice-01 DD-1 (Rust `#[test]` framework, no `.feature`) | Inherited verbatim (DD-GRAPH-1) |
| Slice-01 DD-2 / DD-FED-13 (fail-for-right-reason gate deferred to first step) | Inherited + re-scoped (DD-GRAPH-13) |
| Slice-01 DD-3 / DD-FED-10 (state-delta + Universe lazy bootstrap; Rust port at `tests/common/state_delta.rs`) | Inherited; slice-04 consumes the port via named helpers (DD-GRAPH-10) |
| Slice-01 DD-4 / DD-FED-9 (Tier B not added) | Re-evaluated, same conclusion for slice-04 with a sharpened rationale (DD-GRAPH-9) |
| Slice-01 DD-5 (subprocess invocation = driving-adapter coverage) | Inherited verbatim |
| Slice-01 DD-6 / DD-FED-2 (fake doubles in `test-support`) | Inherited; slice-04 adds NO new fake (DD-GRAPH-2) — it reuses the slice-03 `PeerPds` + `claim add`/`peer pull` seam |
| Slice-01 DD-7 / DD-FED-4 (test directory = `tests/acceptance/` flat) | Inherited verbatim (DD-GRAPH-4) |
| Slice-01 DD-8 (error-path ratio; infra-failure deferred to adapter tests) | Slice-04's per-file ratios: `graph_query_explore.rs` 4/27 = 14.8% explicit `@error` (read surface — the bulk of slice-04 risk is guardrail/anti-merging/sparse, not input-validation sad paths); the recursive-CTE cycle/termination + depth-bound substrate sad paths live at the `adapter-duckdb` probe layer (DELIVER) per DESIGN §6.3 / ADR-021, not the acceptance suite. See acceptance-tests.md §4 for the read-surface rationale. |
| Slice-02 DD-SCR-* (layer-2 pure-core properties via proptest) | Inherited shape — `scoring_core.rs` mirrors `scraper_domain.rs` (DD-GRAPH-3 / DD-GRAPH-12) |
| Slice-03 DD-FED-* (peer-PDS double + seeding helpers) | EXTENDED — slice-04 reuses `PeerPds` + `build_verifiable_peer_records*` for graph seeding (DD-GRAPH-5) |
| Slice-01 DD-11 / DD-FED-11 (Project Infrastructure Policy file deferral) | Continued (DD-GRAPH-11) |
| Slice-01 DD-13 (WS scenario count) | Slice-04 has NO new full-stack walking skeleton in the slice-01 sense — slice-01's WS is the umbrella e2e wiring proof. Slice-04 tags its thinnest user-value demo scenarios (`graph query --object` GQE-1, `--object --weighted` GQE-10, `--object --traverse` GQE-20) `@walking_skeleton` per the story-map's Release-1 + Release-2 demo gates; they exercise the established slice-01/03 e2e path + the NEW scoring/traversal read path. |

---

## Open questions handed to DELIVER (slice-04)

These are deliberately deferred to the DELIVER wave (most are the DESIGN
Q-DELIVER-1..7 set; DISTILL adds the test-side universe + bootstrap items):

1. **Bootstrap order (DD-GRAPH-13)**: the first slice-04 step lands the
   `crates/scoring` crate (ADTs + `score`/`weight_bucket` stubs per
   component-boundaries.md §`crates/scoring`), the `StoragePort` extensions +
   ADTs in `crates/ports/`, the `adapter-duckdb` read-method stubs, the `cli`
   explorer-flag dispatch (todo bodies), AND the test-support seeder +
   assertion-helper + `fixtures_scoring` bodies. Only then does the suite
   compile and reach `todo!()` (RED). Register the two `[[test]]` targets in the
   same step (DD-GRAPH-14).
2. **State-delta universe naming (DD-GRAPH-10)**: which port-exposed names go
   into the universe when migrating GQE-14 (never-persisted: scan
   `storage.duckdb.no_weight_or_bucket_column` +
   `storage.local_claim_store.no_weight_or_bucket_string`), GQE-16
   (`cli.graph_query.explain.running_sum[subject]` ==
   `cli.graph_query.weighted.displayed_weight[subject]`), GQE-11
   (`cli.graph_query.bucket[subject]` +
   `cli.graph_query.sparse_honesty_line_present`), GQE-20
   (`cli.graph_query.traverse.edge_cids` +
   `cli.graph_query.traverse.edge_cid_resolvable[cid]`). DD-GRAPH-10 names the
   helpers; DELIVER fills the explicit `universe = {...}` set.
3. **Exact recursive-CTE SQL + cycle-safety (DESIGN Q-DELIVER-1)**: the
   visited-set representation + depth-column type + omitted-edge counting for
   `traverse_graph`. ADR-021 fixes the cycle-safety + depth-bound CONTRACT; the
   exact SQL is DELIVER's, subject to `adapter-duckdb` probe #2/#3 + Gate 5
   (GQE-24). The cyclic-graph fixture
   (`scoring_fixture_cyclic_two_claim_graph`) backs the probe, not an acceptance
   scenario (the substrate-lie check is an adapter-level concern per DESIGN §6.3).
4. **One `query_attributed_for_scoring(ScoringFilter)` vs three thin methods
   (DESIGN Q-DELIVER-2)**: recommended one filtered method; crafter confirms.
5. **`crates/scoring` module split (DESIGN Q-DELIVER-3)**: single `lib.rs` vs
   `formula.rs`+`bucket.rs`+`types.rs`. Crafter's call; SC-1..SC-6 + the
   per-arm unit tests (DELIVER inner loop) drive the split.
6. **`ScoringConfig` compile-time `const` vs config file (DESIGN Q-DELIVER-4)**:
   compile-time `const` (WD-86); `fixtures_scoring.rs::EXPECTED_*` mirrors the
   defaults and updates in lockstep if DELIVER tunes them.
7. **Exact `[STRONG]/[MODERATE]/[SPARSE]` thresholds (DESIGN Q-DELIVER-5)**: the
   WD-77/ADR-022 defaults. SC-6 + GQE-19 assert only the load-bearing NOT-Sparse
   half for the triangulated cargo pairing; DELIVER picks the STRONG-vs-MODERATE
   cut within the small/closed-form constraint.
8. **Exact "Connections found" callout + omitted-edge + sparse-honesty line
   formats (DESIGN Q-DELIVER-6)**: GQE-20/GQE-22/GQE-11 assert the line CONTENT
   (the connection naming, the omitted count + `--depth` hint, the "based on N
   claims by M authors" line); DELIVER fills the exact format that satisfies
   them. The `assert_sparse_rendered_as_sparse` / traversal helpers pin the
   content contract.
9. **Whether the once-per-user explorer orientation message ships (DESIGN
   Q-DELIVER-7)**: optional; not load-bearing; no slice-04 acceptance scenario
   REQUIRES it (unlike slice-03's FQ-6/CC-5 which asserted the orientation
   gating). DELIVER's call; if shipped, an `[explorer]` key in identity.toml +
   a once-per-user gate, but DISTILL does not assert it.

---

## Out of scope for this DISTILL (explicit deferrals)

- **Pure-core unit tests + mutation testing of `crates/scoring`** — DELIVER's
  inner TDD loop (DD-GRAPH-7); `scoring_core.rs` exposes only the load-bearing
  layer-2 properties.
- **The `adapter-duckdb` recursive-CTE cycle-safety + termination + depth-bound
  PROBE** (ADR-021 probe #2/#3) — DELIVER's adapter-integration concern per
  DESIGN §6.3; the `scoring_fixture_cyclic_two_claim_graph` fixture backs it but
  no acceptance scenario asserts the substrate-lie check (it is below the
  driving-port boundary).
- **`xtask check-arch` extension** (the `no_cross_table_join_elides_author` rule
  extending to scoring/traversal SQL + the `scoring` pure-core allowlist entry)
  — DELIVER's xtask concern per component-boundaries.md §`xtask`; the structural
  layer of the three-layer anti-merging enforcement (the behavioral layer is
  GQE-1/16/26 + SC-5; the type layer is the non-`Option` `author_did` ADTs).
- **Persisted/federated scores, ML/learned weighting, multi-user aggregation,
  retraction-aware down-weighting, graph-store swap, unbounded depth** — all
  LOCKED REJECTED/deferred by DESIGN §12 (WD-72/71/79/85/81/76). No slice-04
  scenario asserts any of them.
- **DEVOPS KPI instrumentation** (`graph.connection.surfaced`,
  `graph.query.duration_seconds`) — DEVOPS's deliverable per outcome-kpis.md;
  KPI-GRAPH-1 (north-star connection rate) + KPI-GRAPH-6 (latency) are
  telemetry-measured, not asserted at the acceptance boundary (the acceptance
  suite proves the connection is SURFACEABLE via GQE-20; the RATE is a
  production-telemetry measurement).
- **Bootstrapping `docs/architecture/atdd-infrastructure-policy.md`** —
  continues slice-01 DD-11 + DD-FED-11 deferral (DD-GRAPH-11).

---

## Handoff summary

| Recipient | Reads | Produces |
|---|---|---|
| DELIVER (`@nw-functional-software-crafter`, per ADR-007) | `acceptance-tests.md`; `traceability.md`; the two slice-04 test skeletons (`tests/acceptance/graph_query_explore.rs` + `scoring_core.rs`); this file; the open-questions list above; DESIGN's `component-boundaries.md` for the `crates/scoring` surface + the `StoragePort` extensions; DESIGN's `data-models.md` for the worked formula arithmetic + the recursive-CTE shape | The first slice-04 bootstrap step lands: (a) the pure `crates/scoring` crate (ADTs + `score`/`weight_bucket`/`ScoringConfig::DEFAULT` stubs); (b) `crates/ports/` extended with the four `StoragePort` read methods + `AttributedClaim`/`GraphEdge`/`TraversalBound`/`ScoringFilter`/`GraphNode`/`TraversalResult`; (c) `crates/adapter-duckdb/` recursive-CTE + scoring-feed read-method stubs; (d) `crates/cli/` `graph query` dispatch for the 6 explorer flags (bodies `todo!()`); (e) `crates/test-support/src/{fixtures_scoring}.rs` fixture bodies + the `support/mod.rs` seeder + assertion-helper bodies; (f) the two `[[test]]` registrations. After this step all slice-04 acceptance tests classify as RED — DD-GRAPH-13 fail-for-right-reason gate runs. Then one-at-a-time scenario implementation per outside-in TDD (Release 1: GQE-1/10/26 + SC-1/3/5; Release 2: GQE-6/20 + SC-4/6; Release 3: GQE-16). |

---

## Changelog

- 2026-05-28 — Quinn — initial DISTILL-wave decisions for slice-04. All
  decisions DD-GRAPH-1..DD-GRAPH-14 LOCKED. Reconciliation against DISCUSS
  WD-69..WD-79 + DESIGN WD-80..WD-93 passed with 0 contradictions. WD-90 +
  Q-DELIVER-SCORE-1 resolution of the `# DISTILL: confirm`
  cross-project-triangulation-counts-as-breadth bucket rule inherited verbatim
  and pinned in SC-3 + SC-6 + GQE-11 + GQE-19.
