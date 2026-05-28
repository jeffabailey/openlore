# CI/CD Pipeline Delta — openlore-scoring-graph (slice-04)

- **Wave**: DEVOPS
- **Date**: 2026-05-28
- **Architect**: Apex
- **Tool**: GitHub Actions (UNCHANGED from D-D1)
- **Branching**: GitHub Flow (UNCHANGED from D-D7)

This is the slice-04 **delta** to `ci-cd-pipeline.md` (foundation). Read that
file first, plus the slice-03 and slice-02 deltas. This document describes
only the additions and the single-line modifications. No YAML is written
here. DELIVER lands the YAML into the EXISTING `ci.yml` and `nightly.yml`
workflow files — no new workflow file is created (Simplest-Solution
Alternative 3, rejected in `platform-design.md` §7).

## 1. Workflow files (no new files)

| File | Triggers (UNCHANGED) | Slice-04 additions |
|---|---|---|
| `.github/workflows/ci.yml` | `pull_request: [main]`, `push: [main]` | Ten new acceptance-stage jobs (§3): six gate ATs (Gates 1–6) + four explorer-scenario ATs |
| `.github/workflows/nightly.yml` | `schedule: cron 02:00 UTC daily`, `workflow_dispatch` | Mutation `--package` list += `crates/scoring` (§4) |
| `.github/workflows/release.yml` | `push: tags: ['v*']` | Re-runs the new acceptance jobs as part of the existing acceptance re-run; release-tag mutation re-run now covers `crates/scoring` (§5). NO new external/Pact step — slice-04 adds no external integration (WD-92) |

## 2. Commit-stage (UNCHANGED + one expanded rule + one new allowlist entry)

`fmt`, `lint`, `supply-chain`, `arch-check`, `probe-check`, `test-unit`,
`test-property` all run unchanged in command and gating semantics. Two
single-line, no-new-job extensions land in existing stages:

- **`arch-check` rule extension (anti-merging in aggregates)**: the slice-03
  `xtask check-arch` rule `no_cross_table_join_elides_author` EXTENDS to cover
  the new scoring-feed and traversal SQL string literals in `adapter-duckdb`
  (per WD-88 layer 2 + DESIGN §10). Same command, expanded rule set; DELIVER
  lands the rule code in the `xtask` crate; the CI job invocation is unchanged.
- **`arch-check` pure-core allowlist extension**: `crates/scoring` is added to
  the `xtask check-arch` pure-core allowlist (WD-82). The rule asserts
  `crates/scoring` imports only `std` + pure value types and NEVER touches
  `duckdb`, `tokio`, `reqwest`, `std::fs`, or `std::time::SystemTime` (DESIGN
  §5.1 invariant 1). Compile-and-check; no new CI job.

`probe-check` (foundation §3.5) covers the extended `adapter-duckdb` probe
automatically — it's an AST walker over every `impl <Port> for <Adapter>`, so
the new recursive-CTE / scoring-feed probe scenarios on the AUGMENTED
`StoragePort` impl are in scope by construction. No CI change needed.

`test-property` covers the new pure-core property tests (`weight_is_deterministic`,
`weight_equals_sum_of_contributions`, `single_claim_is_sparse_even_at_high_confidence`)
in the existing stage — they are property tests in the workspace, discovered
unchanged. No new job.

## 3. Acceptance-stage additions

All ten new jobs run in parallel within the existing acceptance stage, after
the commit-stage gates pass. Each is **blocking on PR** and **gates release**.
The first three are release-blocking GUARDRAILS (the KPI-GRAPH disprovers);
the next three are blocking gates (auditability / no-persist / no-rounding);
the final four are the explorer-scenario functional ATs.

### 3.1 `at-scoring-aggregate-preserves-attribution` (Gate 1 — GUARDRAIL)
- **Command**: `cargo nextest run --test scoring_aggregate_preserves_attribution`
- **What it does**: seeds 1 own + 2 peer claims on the SAME `(subject, object)`
  authored by three DISTINCT DIDs; runs `graph query --weighted` then
  `--weighted --explain <subject>`; asserts the single `WeightedPairing`
  decomposes via `--explain` to exactly THREE attributed `Contribution` rows,
  each carrying a non-null `author_did` traceable to `claims` or `peer_claims`;
  asserts ZERO "consensus"/synthesized-author rows; asserts the same property
  holds for a `--traverse` result over the same fixture. Cross-checks that the
  weight aggregation happened in Rust (the per-claim `Contribution`s exist as
  rows), NOT a SQL `SUM/GROUP BY` (WD-88).
- **Maps to**: KPI-GRAPH-2 (anti-merging in aggregates = 100%); WD-73 / WD-88;
  extends slice-03 KPI-FED-1; Gate 1
- **Type**: blocking GUARDRAIL (release-blocking; a disprover)
- **Wall-clock target**: < 30 s

### 3.2 `at-weight-equals-formula` (Gate 2 — GUARDRAIL)
- **Command**: `cargo nextest run --test weight_equals_formula`
- **What it does**: for a fixture of N claims on a `(subject, object)`, runs
  `graph query --weighted --explain <subject>`; parses the `--explain` output;
  asserts the displayed `adherence_weight` equals the documented formula
  `sum(confidence x author_distinct_bonus x cross_project_triangulation_bonus)`
  (WD-86 default constants) applied to EXACTLY the displayed contributing
  claims; asserts the running sum of the per-claim `Contribution.subtotal`
  values equals the displayed weight byte-for-byte; asserts the formula text
  is printed in the output (reproducible by hand). Includes an adversarial set
  (author-distinct bonus boundary, triangulation bonus boundary) so a formula
  off-by-one is caught.
- **Maps to**: KPI-GRAPH-3 (every weight reproducible = 100%); WD-71 / WD-86;
  Gate 2
- **Type**: blocking GUARDRAIL (release-blocking; a disprover)
- **Wall-clock target**: < 20 s
- **Pairs with**: the `crates/scoring` mutation target (§4) — the AT asserts
  the formula is reproducible; mutation testing asserts the TEST would catch a
  formula regression.

### 3.3 `at-sparse-renders-sparse` (Gate 3 — GUARDRAIL)
- **Command**: `cargo nextest run --test sparse_renders_sparse`
- **What it does**: seeds a `(subject, object)` with exactly ONE claim by ONE
  author at confidence 0.95 and no cross-project triangulation; runs `graph
  query --weighted`; asserts the pairing renders the `[SPARSE]` bucket (NOT
  `[STRONG]`) regardless of the high confidence; asserts the "based on N claims
  by M authors" honesty line is present verbatim and NOT suppressed for
  brevity. Includes the Q-DELIVER-SCORE-1 boundary case: a single-claim pairing
  that IS lifted by same-author cross-project triangulation (DESIGN default:
  triangulation counts toward breadth, so it is NOT `[SPARSE]`) — asserts the
  one consistent rule the DISTILL acceptance test fixes.
- **Maps to**: KPI-GRAPH-4 (zero manufactured confidence = 100%); WD-74 / WD-90;
  Gate 3
- **Type**: blocking GUARDRAIL (release-blocking; a disprover)
- **Wall-clock target**: < 20 s

### 3.4 `at-weight-and-bucket-never-persisted` (Gate 4)
- **Command**: `cargo nextest run --test weight_and_bucket_never_persisted`
- **What it does**: runs a `graph query --weighted --explain` over a seeded
  graph; then scans EVERY DuckDB table, EVERY on-disk artifact under
  `claims/`, `peer_claims/`, and any config/state file for the strings
  `STRONG`, `MODERATE`, `SPARSE`, and `adherence_weight`; asserts none is
  persisted; asserts no PDS write occurred (no network in slice-04); re-runs
  the same query after a simulated `peer pull` and asserts the weight CHANGES
  (proving it is recomputed at query time, never cached). Extends the slice-01
  confidence-bucket no-persist unit test.
- **Maps to**: WD-72 / WD-89 / I-6 (extends WD-10 display-only discipline);
  Gate 4
- **Type**: blocking
- **Wall-clock target**: < 20 s

### 3.5 `at-traversal-invents-no-edges` (Gate 5)
- **Command**: `cargo nextest run --test traversal_invents_no_edges`
- **What it does**: seeds a contributor↔project↔philosophy fixture; runs `graph
  query --traverse --depth 2`; asserts EVERY displayed edge carries a backing
  `claim_cid` that is independently lookuppable via `graph query --subject
  <project>`; asserts no edge exists that no author signed (no interpolated /
  inferred edge); asserts the depth-2 bound is honored on a depth-4 fixture and
  the omitted-edge count is reported. Runs the cycle-safety case (A→B→A via two
  claims) and asserts the traversal terminates and emits each edge exactly once
  (the behavioral backstop to the adapter probe #2 substrate-lie check).
- **Maps to**: WD-76 / WD-91 / ADR-021; Gate 5
- **Type**: blocking
- **Wall-clock target**: < 30 s

### 3.6 `at-scoring-uses-numeric-confidence` (Gate 6)
- **Command**: `cargo nextest run --test scoring_uses_numeric_confidence`
- **What it does**: seeds claims with adversarial `f64` confidence values
  (float-boundary, many-decimal, 0.0, 1.0 — mirroring the `kpi-4-roundtrip`
  adversarial set); runs `graph query --weighted --explain`; asserts the
  numeric `confidence` the formula consumes is byte-equal to the value shown in
  the per-claim rows (no silent rounding); asserts the display bucket
  (`confidence_bucket`) is a SEPARATE display concern and never feeds the
  formula. Inherits and extends slice-01 KPI-4 round-trip into the scoring path.
- **Maps to**: WD-10 / I-6; inherited slice-01 KPI-4; Gate 6
- **Type**: blocking
- **Wall-clock target**: < 20 s

### 3.7 Explorer-scenario functional ATs (US-GRAPH-001..006)
Four functional ATs cover the explorer surface beyond the six invariants
above. Each is blocking on PR and gates release.

- **`at-query-by-object`** — `cargo nextest run --test query_by_object`. Runs
  `graph query --object <philosophy>`; asserts the result groups projects
  embodying the philosophy by subject; asserts federated scope is implied by
  default (own + peer + scraper-signed; WD-87) and each row carries
  `author_did`. < 20 s.
- **`at-query-by-contributor`** — `cargo nextest run --test query_by_contributor`.
  Runs `graph query --contributor <did>`; asserts one developer's reasoning
  trail is returned with attribution preserved; asserts bare `--subject` (no
  new flags) stays byte-identical to slice-01 own-claims-only behavior
  (WD-87 backward-compat). < 20 s.
- **`at-traverse-bounded`** — `cargo nextest run --test traverse_bounded`. Runs
  `graph query --traverse --depth 2`; asserts the contributor↔project↔philosophy
  tree renders; asserts a contributor spanning ≥2 projects produces a
  "Connections found" callout (the KPI-GRAPH-1 surface); asserts the
  default-depth-2 bound and omitted-edge reporting. < 30 s.
- **`at-explain-reproduces`** — `cargo nextest run --test explain_reproduces`.
  Runs `graph query --weighted --explain <subject>`; asserts the per-claim
  arithmetic decomposition is present and sums to the weight; asserts
  `--explain` for a subject ABSENT from the result set is a usage error
  (non-zero exit), distinct from an empty dimension query (exit 0) — per
  US-GRAPH-005 Example 3. < 20 s.

### 3.8 Acceptance-stage summary (delta)

Net additions to foundation §4.6 (and the slice-03 / slice-02 additions):

| Stage | Wall-clock target | Type | Conditional? |
|---|---|---|---|
| at-scoring-aggregate-preserves-attribution (Gate 1) | < 30 s | blocking GUARDRAIL (release-blocking) | no |
| at-weight-equals-formula (Gate 2) | < 20 s | blocking GUARDRAIL (release-blocking) | no |
| at-sparse-renders-sparse (Gate 3) | < 20 s | blocking GUARDRAIL (release-blocking) | no |
| at-weight-and-bucket-never-persisted (Gate 4) | < 20 s | blocking | no |
| at-traversal-invents-no-edges (Gate 5) | < 30 s | blocking | no |
| at-scoring-uses-numeric-confidence (Gate 6) | < 20 s | blocking | no |
| at-query-by-object | < 20 s | blocking | no |
| at-query-by-contributor | < 20 s | blocking | no |
| at-traverse-bounded | < 30 s | blocking | no |
| at-explain-reproduces | < 20 s | blocking | no |

Aggregate added wall-clock: **< 4 min per PR** (jobs parallelize within the
acceptance stage). No release-tag external overhead (no Pact-real step —
slice-04 adds no external integration). Foundation's target (< 30 min
acceptance) is comfortably preserved.

**No new external contract job.** Unlike slice-03 (`contract-pact-pds-peer`)
and slice-02 (`contract-pact-github`), slice-04 consumes NO external API
(WD-92 / DESIGN §6.2, §6.4). The existing Pact suites are unchanged; no
release-tag real-provider step is added. The local-first guardrail is
asserted by the `kpi-5-offline` integration test (extended to seed a scoring
fixture before the `unshare -n` step; see §6).

## 4. Mutation testing (delta) — `crates/scoring` added to scope

Per Apex Core Principle 9 + D-D8 (nightly-only, pure-core) + D-D23 precedent:

- **`crates/scoring` is added to the `--package` list** of the nightly
  `cargo mutants` invocation. This is the SECOND mutation-scope widening
  (after slice-02's `scraper-domain`, D-D23).
- **Kill-rate target: ≥95%** (matches `claim-domain` + `scraper-domain` per
  ADR-006 Earned Trust — the closed-form scoring formula is the load-bearing
  transparency primitive; KPI-GRAPH-3 demands reproducibility, so the tests
  must be mutation-hard).
- The formula functions (`score`, `contributions_for`, `weight_bucket`) and
  the constants in `ScoringConfig::DEFAULT` are the mutation surface; a
  surviving mutant in `weight_bucket`'s breadth guard would mean a sparse
  pairing could slip to `[STRONG]` without test failure — exactly the
  KPI-GRAPH-4 failure the gate guards against.
- `adapter-duckdb` is NOT mutated (effect shell; recursive-CTE reads covered
  by the extended probe + integration tests; D-D8 pure-core-only policy).
- Release-tag mutation re-run inherits the D-D8 blocking-on-regression gate;
  `crates/scoring` is now in scope.
- DELIVER updates the nightly workflow's `--package` list (a one-line edit).
  No new gate semantics; just a wider scope. The `CLAUDE.md` Mutation Testing
  Strategy section is unchanged in POLICY (D-D31).

## 5. Release workflow (delta)

Per `ci-cd-pipeline.md` (foundation) §7. Slice-04 inserts:

- 5.1 The ten new acceptance-stage jobs (§3.1–3.7) are re-run on the tagged
  ref as part of the existing acceptance re-run. No new step needed; they are
  already in the workflow.
- 5.2 The release-tag mutation re-run now covers `crates/scoring` under the
  same blocking-on-regression rule (§4).
- 5.3 **No new external Pact step.** Slice-04 adds no external integration
  (WD-92), so there is no real-provider release variant (contrast slice-03's
  real-bsky and slice-02's real-GitHub steps, which carry forward unchanged
  but gain nothing from slice-04).
- 5.4 Substrate matrix: NO new cells; each cell now also exercises a
  `graph query --weighted --explain` happy path AND a cycle-safe `--traverse`
  against a seeded local fixture (per `platform-design.md` delta §6). Same
  jobs, expanded body.
- 5.5 Adversarial renderer review (the slice-03 D-D19 / slice-02 D-D28
  checklist) gains one slice-04 line: "weighted/sparse renderer never collapses
  authors into a consensus row AND never suppresses the `[SPARSE]` honesty
  line" (D-D33). Recorded in the release CHANGELOG ("Renderer review: passed
  YYYY-MM-DD").

Estimated release wall-clock (delta): **+3 to +4 min** (the ten new acceptance
ATs, parallelized, plus the slightly longer mutation re-run covering
`crates/scoring`). No external-provider overhead added. Prior-slice estimate
18–35 min; new estimate 21–39 min. Acceptable.

## 6. Quality-gate enforcement summary (delta rows only)

Insert these rows into the foundation table at §9:

| Gate | Pre-PR (local) | PR | Nightly | Release-tag |
|---|---|---|---|---|
| at-scoring-aggregate-preserves-attribution (Gate 1) | – | ✓ GUARDRAIL | – | ✓ GUARDRAIL |
| at-weight-equals-formula (Gate 2) | – | ✓ GUARDRAIL | – | ✓ GUARDRAIL |
| at-sparse-renders-sparse (Gate 3) | – | ✓ GUARDRAIL | – | ✓ GUARDRAIL |
| at-weight-and-bucket-never-persisted (Gate 4) | – | ✓ blocking | – | ✓ blocking |
| at-traversal-invents-no-edges (Gate 5) | – | ✓ blocking | – | ✓ blocking |
| at-scoring-uses-numeric-confidence (Gate 6) | – | ✓ blocking | – | ✓ blocking |
| at-query-by-object / -by-contributor / -traverse-bounded / -explain-reproduces | – | ✓ blocking | – | ✓ blocking |
| mutation testing (`crates/scoring`) | – | – | ✓ advisory | ✓ blocking on regression |
| kpi-5-offline (extended: seed scoring fixture before `unshare -n`) | – | ✓ blocking | – | ✓ blocking |
| arch-check `no_cross_table_join_elides_author` (extended to scoring/traversal SQL) | (lint subset, pre-push) | ✓ blocking | – | ✓ blocking |
| arch-check pure-core allowlist (`crates/scoring`) | (lint subset, pre-push) | ✓ blocking | – | ✓ blocking |

The "Pre-PR (local)" column is intentionally empty for the new acceptance ATs
— they are too slow for pre-push (foundation pre-push runs only unit +
property + arch). The `arch-check` rule extensions run in the lint/arch
subset that pre-push already invokes. The pre-commit and pre-push hook designs
from foundation §5 are unchanged.

## 7. Adapter probe extension (no new top-level job)

The recursive-CTE cycle-safety substrate-lie check is an Earned-Trust PROBE
concern, exercised by the EXISTING `probe-check` AST walker + the per-cell
substrate run, NOT a separate CI job. Per DESIGN §6.3, the AUGMENTED
`adapter-duckdb` probe gains three scenarios:

- **(a) scoring-feed attribution round-trip**: write 1 own + 2 peer claims on
  the same `(subject, object)` by distinct authors; call
  `query_attributed_for_scoring`; assert exactly 3 `AttributedClaim`s with
  three distinct non-empty `author_did`s (the anti-merging substrate check).
- **(b) recursive-CTE termination**: run `traverse_graph` on a cyclic fixture
  (A→B→A via two claims) at depth 3; assert termination within 250 ms and each
  edge emitted exactly once (the substrate-lie check — DuckDB CTEs do not
  auto-detect cycles).
- **(c) depth-bound honored**: `traverse_graph` at `depth=2` on a depth-4
  fixture returns only ≤depth-2 edges and reports the omitted count.

These run wherever `probe_all` runs (startup self-test + the substrate gold
matrix per-cell). DELIVER writes the probe scenarios; no CI YAML change beyond
the matrix-body extension in §5.4.

## 8. Branch protection rules (UNCHANGED)

Foundation §10 rules carry forward unchanged. The ten new acceptance jobs are
added to the "required status checks" list at the same level as the existing
acceptance jobs. The three GUARDRAIL jobs (Gates 1–3) are required-checks like
any other blocking AT; their release-blocking nature is the DISCUSS disprover
policy, enforced by being required checks.

## 9. `deny.toml` (UNCHANGED)

Foundation §11 content unchanged. Slice-04 introduces ZERO new production
dependencies (recursive CTEs are built into DuckDB, already pinned per ADR-001;
`crates/scoring` is pure `std`-only arithmetic). No `cargo deny` additions
(I-11). DELIVER does NOT amend `deny.toml`.

## 10. References

- `platform-design.md` (sibling, this dir) — gate-inventory delta, mutation scope, risk register
- `observability.md` (sibling, this dir) — what the new tests' events look like
- `kpi-instrumentation.md` (sibling, this dir) — KPI-GRAPH gate mapping
- Foundation `ci-cd-pipeline.md` + slice-03/slice-02 `ci-cd-pipeline.md` deltas — the base to extend
- `docs/feature/openlore-scoring-graph/discuss/shared-artifacts-registry.md` (Gate 1..6 definitions)
- `docs/feature/openlore-scoring-graph/design/architecture-design.md` (§6.3 probe; §10 three-layer enforcement)
- `docs/feature/openlore-scoring-graph/design/wave-decisions.md` (WD-80..WD-93)
- `docs/feature/openlore-scoring-graph/discuss/outcome-kpis.md`
