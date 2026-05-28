# Platform Design Delta — openlore-scoring-graph (slice-04)

- **Wave**: DEVOPS (design portion; the sibling-feature extension of slice-01/02/03 DEVOPS)
- **Date**: 2026-05-28
- **Architect**: Apex (nw-platform-architect)
- **Feature**: openlore-scoring-graph (sibling slice-04; explore-the-graph)
- **Inherits**: openlore-foundation DEVOPS (D-D1..D-D13, ADR-010..ADR-012) — UNCHANGED; openlore-federated-read DEVOPS (D-D14..D-D21) — UNCHANGED; openlore-github-scraper DEVOPS (D-D22..D-D29) — UNCHANGED
- **Paradigm context**: functional Rust (ADR-007, Accepted)

This is the DEVOPS platform-design **delta** for slice-04. The slice-01
platform layer (operating model, branching, gate inventory, distribution,
substrate matrix) is **unchanged**, as are the slice-02 and slice-03 deltas.
This document records only the new extensions. Read in conjunction with
`docs/feature/openlore-foundation/devops/platform-design.md`,
`docs/feature/openlore-federated-read/devops/platform-design.md`, and
`docs/feature/openlore-github-scraper/devops/platform-design.md`.

## 1. What did NOT change

| Concern | Status | Reference |
|---|---|---|
| Operating model (local-first, solo-dev, no SLOs, no on-call) | UNCHANGED | foundation §1 |
| DORA framing (per-release tag, no fleet) | UNCHANGED | foundation §1 |
| CI tool (GitHub Actions) + branching (GitHub Flow) | UNCHANGED | foundation §7, D-D1, D-D7 |
| Distribution (`cargo install` + 4-platform binaries; Windows out) | UNCHANGED | foundation §3 row 4, ADR-011, D-D10 |
| Substrate gold matrix (8 cells for release; 4 for PR) | UNCHANGED in shape; one row extended | foundation §3 row 5; this doc §6 |
| Release artifact security (cosign + SBOM + SLSA) | UNCHANGED | ADR-012, D-D11 |
| Telemetry opt-in policy (off by default; no endpoint operated) | UNCHANGED | ADR-010, D-D4 |
| Local quality gates (lefthook/pre-commit + pre-push) | UNCHANGED | foundation §5 |
| Quality-gate inventory taxonomy | UNCHANGED in shape; entries added | foundation §6; this doc §3 |
| Mutation testing POLICY (nightly-only, release-tag blocking, pure-core scope) | UNCHANGED in policy; SCOPE widens (+`scoring`) | D-D8, D-D23; this doc §5, D-D31 |
| External contract surface (Pact suite shape) | UNCHANGED — slice-04 adds ZERO external integration | this doc §7, D-D34 |

## 2. What DID change (the delta)

Slice-04 is a **read-only LOCAL** analysis slice (WD-79 / WD-92). It is the
**meatiest read slice** so far: it resolves the WD-8 store revisit (AUGMENT,
not swap; WD-81 / ADR-021), introduces the first genuinely new pure-domain
crate since slice-02 (`crates/scoring`; WD-82 / ADR-022), and carries the
slice-03 anti-merging invariant into a NEW failure surface — **aggregates**.

Slice-04 introduces three new platform-layer concerns; CRUCIALLY, none is a
new EXTERNAL surface (no daemon, no network, no new service):

| New concern | Where it lives | Why slice-04 introduces it |
|---|---|---|
| Two release-blocking aggregate guardrails (anti-merging-in-aggregates + scoring-transparency) | `at-scoring-aggregate-preserves-attribution` + `at-weight-equals-formula` CI jobs (new) | KPI-GRAPH-2 (zero attribution loss in any aggregate) and KPI-GRAPH-3 (every weight reproducible) are release-blocking guardrails; the weight is the FIRST aggregate view in the product and the first place an author could be merged away |
| New pure-core mutation target (`crates/scoring`) | nightly `cargo mutants --package` list extension | The closed-form scoring formula (WD-86 constants SSOT) is the load-bearing transparency primitive; un-mutated, its correctness is unguarded — second mutation-scope widening, after slice-02's `scraper-domain` (D-D23) |
| Five new KPI-GRAPH instrumentation surfaces (KPI-GRAPH-1..4 + KPI-GRAPH-6) | `tracing` events emitted at the explorer-query, traversal, and scoring boundaries | The slice has its own outcome KPIs (`discuss/outcome-kpis.md`); foundation/slice-03 KPIs do not subsume the explorer behaviors |

Everything else is additive within existing structures — new CI jobs in the
existing `ci.yml`, the mutation `--package` extension in `nightly.yml`, new
`tracing` event names emitted from new code paths, and one extended
`adapter-duckdb` probe (the recursive-CTE cycle-safety "substrate-lie" check).

## 3. Quality-gate inventory delta

Net additions to foundation §6 (no rows removed; no semantics changed). The
three release-blocking GUARDRAILS map directly to the KPI-GRAPH guardrail
metrics (KPI-GRAPH-2, KPI-GRAPH-3, KPI-GRAPH-4).

| Category | Where | Type | What it gates | Origin |
|---|---|---|---|---|
| CI | acceptance-stage job `at-scoring-aggregate-preserves-attribution` (Gate 1) | blocking (GUARDRAIL — KPI-GRAPH-2) | every weighted/scored/traversed aggregate decomposes to its `(author_did, claim_cid)` contributions; ZERO merged-into-aggregate rows; no SQL aggregation across authors | this slice §4 |
| CI | acceptance-stage job `at-weight-equals-formula` (Gate 2) | blocking (GUARDRAIL — KPI-GRAPH-3) | the displayed `adherence_weight` equals the documented formula applied to exactly the displayed claims; `--explain` reproduces the arithmetic by hand; no opaque/ML weight | this slice §4 |
| CI | acceptance-stage job `at-sparse-renders-sparse` (Gate 3) | blocking (GUARDRAIL — KPI-GRAPH-4) | a thin subgraph (1 claim / 1 author, no triangulation breadth) renders `[SPARSE]` + the "based on N claims by M authors" honesty line, regardless of confidence magnitude | this slice §4 |
| CI | acceptance-stage job `at-weight-and-bucket-never-persisted` (Gate 4) | blocking | neither `adherence_weight` nor `weight_bucket` is written to any DuckDB table, on-disk artifact, signed payload, or PDS record; scan for `STRONG\|MODERATE\|SPARSE` + `adherence_weight` finds nothing persisted | this slice §4 |
| CI | acceptance-stage job `at-traversal-invents-no-edges` (Gate 5) | blocking | every traversed edge maps to exactly one backing `claim_cid`; the CTE walks existing rows only; no interpolated/inferred edge | this slice §4 |
| CI | acceptance-stage job `at-scoring-uses-numeric-confidence` (Gate 6) | blocking | the numeric `confidence` fed to the formula is byte-equal to the value shown in per-claim rows; no silent rounding (extends slice-01 KPI-4) | this slice §4 |
| CI | acceptance-stage jobs `at-query-by-object`, `at-query-by-contributor`, `at-traverse-bounded`, `at-explain-reproduces` (US-GRAPH scenarios) | blocking | the explorer dimensions, bounded/cycle-safe traversal, and `--explain` behaviors function end-to-end on the local graph | this slice §4 |
| CI (nightly) | mutation-stage scope expansion | advisory (nightly) / blocking-on-regression (release-tag) | `cargo mutants` extended to include `crates/scoring` (the new pure-core crate); kill-rate target ≥95% | this slice §5; D-D31 |

All other foundation/slice-02/slice-03 gates (`fmt`, `lint`, `supply-chain`,
`arch-check`, `probe-check`, `test-unit`, `test-property`, `kpi-4-roundtrip`,
`kpi-5-offline`, `test-integration-pds`, `contract-pact-pds`,
`contract-pact-pds-peer`, `contract-pact-github`, and the slice-03/02 ATs)
remain unchanged in command and gating semantics.

## 4. Constraint Impact Analysis (delta)

Four new constraints surface in slice-04; one inherited constraint gains
weight (the anti-merging invariant now also applies to aggregates).

| Constraint | Source | % delivery affected | Priority | New / changed? |
|---|---|---|---|---|
| Anti-merging-in-AGGREGATES invariant (KPI-GRAPH-2 guardrail) | DISCUSS KPI-GRAPH-2; WD-73 / WD-88; Gate 1; extends slice-03 I-FED-1 | 100% of weighted/traversed paths | HIGH | NEW (extends slice-03) |
| Scoring transparency / reproducible weight (KPI-GRAPH-3 guardrail) | DISCUSS KPI-GRAPH-3; WD-71 / WD-86; Gate 2 | 100% of weighted paths | HIGH | NEW |
| Sparse honesty driven by evidence breadth (KPI-GRAPH-4 guardrail) | DISCUSS KPI-GRAPH-4; WD-74 / WD-90; Gate 3 | 100% of weighted paths over thin subgraphs | HIGH | NEW |
| Recursive-CTE cycle safety + depth bound (substrate-lie check) | DESIGN WD-91 / ADR-021; adapter probe #2/#3; Gate 5 | 100% of `--traverse` paths | HIGH | NEW |
| Weights/buckets never persisted (display-only discipline) | WD-72 / WD-89 / I-6; Gate 4 | 100% of weighted paths | MEDIUM | NEW (extends WD-10 confidence-bucket discipline) |
| Pure-core has zero I/O imports (now applies to the new `scoring` crate) | ADR-009, WD-82; I-1/I-2 | every PR (CI `arch-check` gate) | HIGH | UNCHANGED (rule applies to the new crate too) |

**Decision Rule applied (per platform-engineering-foundations skill)**: the
three guardrails (anti-merging-in-aggregates, scoring-transparency, sparse
honesty) each affect 100% of the slice's weighted/traversed user-visible
behavior AND are explicit DISCUSS disprovers (any failure is unshippable per
`outcome-kpis.md` §Disprovers). All three warrant first-class **release-blocking**
CI gates — landed as the GUARDRAIL AT entries in §3.

**Constraint-Free Baseline (delta)**: nothing about slice-04 introduces an
operational gating ceremony that wasn't already there. The release cadence is
still "ship when green" with the same set of gates plus the new acceptance-test
gates listed in §3. Wall-clock impact is small (each new AT is < 30 s;
aggregate < 4 min added to the acceptance stage; jobs parallelize).

## 5. Mutation scope (delta) — second widening since slice-01

Per Apex Core Principle 9 + D-D8 (nightly-only, scoped to pure-core) + the
D-D23 precedent (slice-02 added `scraper-domain`):

- **`crates/scoring` is added to the nightly `cargo mutants --package` list.**
  This is the SECOND mutation-scope widening, mirroring D-D23's reasoning:
  slice-04 adds a GENUINELY NEW pure-core crate (WD-82 / ADR-022) — it MUST
  enter the `--package` list or the closed-form scoring formula's correctness
  is unguarded by mutation.
- **Kill-rate target ≥95%** (matches `claim-domain` and `scraper-domain` per
  ADR-006 Earned Trust). The scoring formula is the load-bearing transparency
  primitive (KPI-GRAPH-3 / WD-71); a surviving mutant in the formula or the
  bucket function would mean the by-hand reproduction (`--explain`) could
  silently drift from the displayed weight — precisely the J-002 trust
  failure. Pure-core mutation hardness is the price.
- **`adapter-duckdb` is NOT mutated** (effect shell; the new recursive-CTE /
  scoring-feed reads are covered by the extended probe + integration tests,
  per the D-D8 pure-core-only policy). The traversal cycle-safety is an
  Earned-Trust PROBE concern, not a mutation concern.
- Release-tag mutation re-run inherits the D-D8 blocking-on-regression gate;
  the `scoring` crate is now in that re-run's scope.
- The `CLAUDE.md` `## Mutation Testing Strategy` section is unchanged in
  POLICY (nightly-only per D-D8); only the `--package` list grows — a
  workflow-file edit, not a strategy change. Mirrors the D-D23 note exactly.

Production crate count: 10 → 11 (the new `crates/scoring`; per WD-82). External
dependency count: **unchanged (zero new)** — recursive CTEs are built into
DuckDB (already pinned, ADR-001); the scoring core is pure arithmetic
(`std`-only). No `cargo deny` change (I-11).

## 6. Substrate matrix (delta)

No new axes, no new cells. The existing 8-cell release matrix and 4-cell PR
subset are extended only in the "per-cell exercised path": each cell now also
exercises a `graph query --weighted --explain` happy-path AND a `graph query
--traverse` cycle-safe traversal against a seeded local fixture (the same
single DuckDB file the cell already provisions). No new substrate is
introduced — slice-04 reads the SAME single-file store across the SAME
filesystem/allocator cells. The recursive-CTE cycle-safety concern is
exercised per-cell via the extended `adapter-duckdb` probe (§8 risk row +
`observability.md` delta §3).

## 7. Simplest Solution Check (per cicd-and-deployment skill)

Before extending CI/observability for slice-04, three simpler alternatives
were considered (and the meatiest decision — the store — is in DESIGN §9 /
ADR-021, AUGMENT-not-swap, which is itself the simplest-solution outcome):

### Alternative 1: "Skip the separate aggregate-attribution AT; rely on the slice-03 `at-federation-attribution-preserved` test"
- **What**: trust that the slice-03 attribution test already covers author
  fidelity, so no new `at-scoring-aggregate-preserves-attribution` job.
- **Expected Impact**: meets ~40% of KPI-GRAPH-2. The slice-03 test asserts
  attribution at the per-ROW federated-listing surface; it does NOT exercise
  the AGGREGATE surface (the weight), which is a NEW failure mode — a weight
  is the first place two authors' claims are combined into one view.
- **Why insufficient**: KPI-GRAPH-2 is a release-blocking disprover. The
  aggregate is precisely where merging could happen silently (a SQL `SUM/GROUP
  BY` that drops `author_did`). WD-88's three-layer enforcement requires a
  BEHAVIORAL gate at the aggregate boundary; the slice-03 row-level test does
  not exercise it. Skipping it would leave the slice's headline guardrail
  unguarded end-to-end.

### Alternative 2: "Don't add `crates/scoring` to mutation scope; unit tests on the formula suffice"
- **What**: write thorough unit + property tests for the scoring formula; do
  not extend the nightly `cargo mutants --package` list.
- **Expected Impact**: meets ~70% of the transparency guarantee (the formula
  is tested) but leaves the TESTS unguarded — a weak assertion (e.g.,
  `assert!(weight > 0.0)` instead of `assert_eq!(weight, expected)`) would let
  a formula regression through.
- **Why insufficient**: per the methodology, mutation testing is Earned Trust
  applied to the TESTS. KPI-GRAPH-3 demands the weight be reproducible by hand;
  if a formula mutant survives, the by-hand reproduction (`--explain`) can drift
  from the displayed weight without any test failing. The formula is the
  load-bearing trust primitive — exactly the D-D23 reasoning for `scraper-domain`.

### Alternative 3: "Add a separate workflow file `scoring.yml` for the explorer tests"
- **What**: a dedicated workflow trigger for slice-04 explorer/scoring tests;
  keep `ci.yml` for the prior slices.
- **Expected Impact**: meets ~100% of functional requirements but duplicates
  triggers, caches, toolchain setup, and branch-protection required-checks
  ceremony.
- **Why rejected**: identical to slice-03 Alternative 3. The slice ships as
  part of the same binary; the CI is monorepo; splitting workflows multiplies
  maintenance for zero isolation benefit. `ci.yml` and `nightly.yml` extend
  cleanly. DELIVER adds jobs to the EXISTING workflow files (see §3 of
  `ci-cd-pipeline.md` delta).

The chosen shape (extend `ci.yml` with the new acceptance jobs — six gate ATs
plus the four explorer-scenario ATs; extend `nightly.yml`'s mutation
`--package` list with `crates/scoring`; extend the `adapter-duckdb` probe with
the recursive-CTE cycle-safety scenario; add the privacy-preserving
`graph.*` tracing events into the SAME log pipeline) is the minimum that
satisfies the KPI-GRAPH-2/3/4 guardrails + the auditability/no-persist gates
without duplicating prior-slice infrastructure and without introducing any
external surface (WD-92).

## 8. Risk register (delta)

New risks introduced by slice-04:

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| Recursive-CTE infinite loop on a cyclic claim graph (DuckDB CTEs do NOT auto-detect cycles — the "substrate lies" risk) | MEDIUM | `graph query --traverse` hangs; user blocked | Extended `adapter-duckdb` probe #2 runs a deliberately cyclic fixture (A→B→A via two claims) at depth 3, asserts termination within 250 ms and each edge emitted exactly once (WD-91 / ADR-021); depth-bound (default 2, WD-76) + visited-set guard in the CTE; `at-traversal-invents-no-edges` (Gate 5) backs it behaviorally |
| Silent author merge in an aggregate (a future scoring query uses SQL `SUM/GROUP BY` and drops `author_did`) | LOW-MEDIUM | KPI-GRAPH-2 collapses; the slice-03 trust model breaks at the aggregate | Three-layer enforcement (WD-88): type-level (non-`Option` `author_did` on `Contribution`/`GraphEdge`; aggregation in pure Rust, never SQL), structural (`no_cross_table_join_elides_author` xtask rule extended to scoring/traversal SQL), behavioral (`at-scoring-aggregate-preserves-attribution` CI gate) |
| Weight non-reproducible / drifts from `--explain` (a renderer recomputes weight on a second code path) | LOW | KPI-GRAPH-3 collapses; the J-002 transparency promise broken | Single scoring path (DESIGN 6.1 #1 — `scoring` core is the only weight arithmetic); `at-weight-equals-formula` (Gate 2) asserts `weight == sum(contributions)`; mutation testing on the formula (D-D31) hardens the test; ML/learned weighting forbidden (WD-71) |
| Thin evidence dressed as confident (a single high-confidence claim renders `[STRONG]`) | LOW-MEDIUM | KPI-GRAPH-4 collapses; directly materializes the J-002 "bad call on sparse data" anxiety | `weight_bucket` takes evidence breadth (`claim_count`, `distinct_author_count`), not just magnitude (WD-90); `single_claim_is_sparse_even_at_high_confidence` unit test + `at-sparse-renders-sparse` (Gate 3); adversarial renderer review at release (renderer-review checklist gains one line — D-D33) |
| Derived weight/bucket accidentally persisted (a future "cache the score for speed" optimization) | LOW | KPI-GRAPH-2/3 trust erodes (a stale persisted score tempts federation of a derived value) | `at-weight-and-bucket-never-persisted` (Gate 4) scans all tables + artifacts for `STRONG\|MODERATE\|SPARSE` + `adherence_weight`; WD-89 forbids any write path; revisit requires a WD + ADR |
| Local-first latency regression on a dense graph (recursive-CTE cost grows with density) | LOW | KPI-GRAPH-6 budget (< 5 s for ≤200 claims) breached; exploration friction | `graph.query.duration_seconds` histogram per claim-count bucket (DEVOPS instrumentation); informational alert if P95 > 5 s for the ≤200 bucket (escalate to PO, do NOT block release alone — per `outcome-kpis.md` Handoff item 3); ADR-021 revisit trigger toward a graph store if dogfood shows a sustained breach |

All foundation + slice-03 + slice-02 risks (atrium pre-1.0 churn, PDS drift,
substrate-lies, mutation slowness, supply-chain, Windows, GitHub API drift)
remain in force and unchanged in mitigation.

## 9. Proposed ADRs

**No new ADRs at the DEVOPS layer.** ADR-010..ADR-012 carry forward unchanged
(per D-D34). Slice-04's DESIGN wave already raised ADR-020/021/022 (verb
amendment, recursive-CTE traversal, pure-scoring core + anti-merging-in-aggregates)
— those are DESIGN ADRs. Slice-04's DEVOPS decisions (the six new gate ATs,
the four explorer-scenario ATs, the mutation-scope widening to `crates/scoring`,
the KPI-GRAPH instrumentation events) are CI/observability tactical extensions
of existing decisions (D-D8, D-D17, D-D19, D-D23) — none crosses the DEVOPS-ADR
threshold. Same outcome as slice-03 (D-D21) and slice-02 (D-D29).

The store revisit — the one decision that DID meet the ADR threshold — was
resolved in DESIGN as ADR-021 (AUGMENT DuckDB, not swap), NOT a DEVOPS ADR.
The DEVOPS consequence (no new store = no second backup target, no sync
probe, no new substrate cell) is recorded in §5–§6 above.

## 10. References

- `docs/feature/openlore-scoring-graph/feature-delta.md` (WD-69..WD-79)
- `docs/feature/openlore-scoring-graph/discuss/outcome-kpis.md` (KPI-GRAPH-1..6)
- `docs/feature/openlore-scoring-graph/discuss/shared-artifacts-registry.md` (Gate 1..6)
- `docs/feature/openlore-scoring-graph/design/wave-decisions.md` (WD-80..WD-93; ADR-020..022)
- `docs/feature/openlore-scoring-graph/design/architecture-design.md` (§5 scoring core, §6.3 probe, §9 store revisit, §10 three-layer enforcement)
- Prior-slice DEVOPS docs (`docs/feature/openlore-{foundation,federated-read,github-scraper}/devops/*.md`)
- ADR-010 (telemetry-opt-in), ADR-011 (release-matrix), ADR-012 (supply-chain) — still in force
- ADR-020/021/022 (DESIGN-wave, slice-04) — the architectural axes; this DEVOPS doc adds no ADR
- Sibling files in this dir: `ci-cd-pipeline.md`, `observability.md`, `kpi-instrumentation.md`, `wave-decisions.md`
