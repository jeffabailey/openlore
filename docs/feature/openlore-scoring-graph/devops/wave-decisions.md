# Wave Decisions — DEVOPS — openlore-scoring-graph (slice-04)

- **Wave**: DEVOPS
- **Date**: 2026-05-28
- **Architect**: Apex (nw-platform-architect)
- **Feature**: openlore-scoring-graph (sibling slice-04; explore-the-graph)
- **Inherits**: openlore-foundation DEVOPS D-D1..D-D13 (all LOCKED, carry forward unchanged), ADR-010..ADR-012 (all in force unchanged); openlore-federated-read DEVOPS D-D14..D-D21 (all LOCKED, carry forward unchanged); openlore-github-scraper DEVOPS D-D22..D-D29 (all LOCKED, carry forward unchanged)

This file is the DEVOPS-wave decision log for slice-04. Decisions are numbered
**D-D30 onward** to continue the sequence after slice-02's D-D29. None of the
foundation (D-D1..D-D13), slice-03 (D-D14..D-D21), or slice-02 (D-D22..D-D29)
decisions are re-opened or amended.

> **Sequencing note**: slice numbers reflect the carpaccio split (WD-13); the
> DEVOPS decision numbers follow ship/authoring order. Slice-04 is authored
> after slice-02 (D-D22..D-D29), so slice-04's decisions are D-D30+.

## Inheritance

All foundation (D-D1..D-D13), slice-03 (D-D14..D-D21), and slice-02
(D-D22..D-D29) DEVOPS decisions carry forward verbatim. The load-bearing ones
for slice-04:

**Foundation (D-D1..D-D13):**

- D-D1 (GitHub Actions + GitHub Flow + `v*` tag releases) → unchanged
- D-D2 (tracing + tracing-subscriber + tracing-appender + JSON Lines local) → unchanged; slice-04 emits additional `graph.*` events into the SAME pipeline (no new endpoint)
- D-D3 (distributed tracing skipped) → unchanged; slice-04 is still single-binary; an explorer query is one LOCAL DuckDB read — nothing distributed, and per WD-92 nothing networked
- D-D4 (telemetry opt-in OFF by default, no endpoint) → unchanged; slice-04 events are privacy-preserving structural counts only (no claim contents)
- D-D5 (`openlore stats` verb is DELIVER's call; jq fallback) → unchanged; slice-04 adds a `--explorer` flag to the verb-or-fallback design
- D-D6 (no capacity/perf/stress/chaos) → unchanged; an explorer query is a bounded local read; KPI-GRAPH-6's 5 s budget is the only latency concern and it is informational, not a gate (D-D32)
- D-D7 (GitHub Flow branching) → unchanged
- D-D8 (mutation testing nightly-only, release-tag blocking, pure-core scope) → unchanged in POLICY; SCOPE widens to add `crates/scoring` — see D-D31
- D-D9 (4-cell PR substrate, 8-cell release substrate, +tmpfs/overlayfs nightly) → unchanged in shape; per-cell body extended with a `graph query --weighted --explain` happy path AND a cycle-safe `--traverse` against a seeded local fixture
- D-D10 (cargo install primary + 4-platform binaries; Windows out) → unchanged
- D-D11 (cosign + CycloneDX SBOM + SLSA L2 minimum / L3 target) → unchanged
- D-D12 (Pact mocked in PR/nightly; real provider in release with manual approval) → unchanged in POLICY; slice-04 adds NO external contract surface (WD-92), so no new Pact suite — see D-D34
- D-D13 (no KPI marked RED; YELLOW for cohort) → carried forward as the SAME policy applied to KPI-GRAPH-1..6; see D-D32

**Slice-03 (D-D14..D-D21):**

- D-D14 (peer-DID resolution at startup probe = user's OWN DID only) → unchanged; orthogonal to slice-04 (no DID resolution in the explorer path — the contributor dimension reads existing `author_did` values)
- D-D15 (adversarial-peer fixture, `xtask`-regenerated) → unchanged; slice-04 needs NO adversarial external fixture (read-only LOCAL slice); its "adversarial" surface is the cyclic-graph traversal fixture, handled by the adapter probe (D-D30)
- D-D16 (KPI post-hoc jq aggregation, no state file) → the SAME post-hoc default informs KPI-GRAPH-1 and KPI-GRAPH-6 measurement — see D-D32
- D-D17 (KPI feasibility GREEN/YELLOW policy) → the SAME policy framework applied to KPI-GRAPH-1..6 — see D-D32
- D-D18 (one-shot Likert survey after first event, file-presence pattern) → unchanged; reused unmodified for the KPI-GRAPH-1 day-30 think-aloud prompt (delivered after the first `graph.connection.surfaced` event) — see D-D32
- D-D19 (renderer-review checklist at release time) → unchanged; slice-04 adds ONE line (weighted/sparse renderer never collapses authors AND never suppresses the `[SPARSE]` honesty line) — see D-D33
- D-D20 (CI test-only escape hatch via build-time guard) → unchanged; slice-04 needs no interactive-confirmation hatch (the explorer verbs are read-only, no destructive `--purge`-style gesture)
- D-D21 (no new ADR at DEVOPS layer) → the SAME outcome holds for slice-04 — see D-D34

**Slice-02 (D-D22..D-D29):**

- D-D22 (GitHub contract sub-job with public-endpoint allowlist) → unchanged; orthogonal (slice-04 reads no external API)
- D-D23 (mutation scope widened to add `scraper-domain`; first widening since slice-01; ≥95% kill rate) → the DIRECT precedent for D-D31; slice-04 is the SECOND widening, adding `crates/scoring`
- D-D24 / D-D25 (FakeGithub fixtures; PR/nightly mock vs release real) → unchanged; orthogonal
- D-D26 (KPI-SCR feasibility GREEN/YELLOW; guardrails GREEN+release-blocking) → the SAME framework applied to KPI-GRAPH; the three KPI-GRAPH guardrails are GREEN + release-blocking — see D-D32
- D-D27 (KPI-SCR-1 post-hoc jq, no state file; reuses D-D18 survey) → the SAME post-hoc + survey reuse for KPI-GRAPH-1/6 — see D-D32
- D-D28 (KPI-SCR-3 auditability: CI gate + runtime counter + checklist line) → the DIRECT precedent for slice-04's two runtime guardrail counters (`scoring_attribution_missing_total`, `scoring_weight_irreproducible_total`) backing CI gates — see D-D30, D-D33
- D-D29 (no new DEVOPS ADR; DESIGN raised the architectural ADRs) → the SAME outcome; slice-04's DESIGN raised ADR-020/021/022 — see D-D34

## Locked slice-04 decisions

| # | Decision | Rationale | Status | ADR / Document |
|---|----------|-----------|--------|----------------|
| D-D30 | **The three release-blocking GUARDRAIL acceptance jobs land as new acceptance-stage jobs in the existing `ci.yml`: `at-scoring-aggregate-preserves-attribution` (Gate 1, KPI-GRAPH-2), `at-weight-equals-formula` (Gate 2, KPI-GRAPH-3), `at-sparse-renders-sparse` (Gate 3, KPI-GRAPH-4).** Each is a required status check and release-blocking. Two runtime guardrail counters back them: `scoring_attribution_missing_total` and `scoring_weight_irreproducible_total` (both target 0 forever). The remaining three gate ATs (`at-weight-and-bucket-never-persisted` Gate 4, `at-traversal-invents-no-edges` Gate 5, `at-scoring-uses-numeric-confidence` Gate 6) + four explorer-scenario ATs (`at-query-by-object`, `at-query-by-contributor`, `at-traverse-bounded`, `at-explain-reproduces`) are blocking-on-PR. | KPI-GRAPH-2/3/4 are explicit DISCUSS disprovers (`outcome-kpis.md` §Disprovers — any failure is unshippable). The weight is the FIRST aggregate VIEW in the product — the first place two authors' claims combine into one row — so the anti-merging invariant (slice-03 I-FED-1) needs a NEW behavioral gate at the aggregate boundary; the slice-03 row-level test does not exercise it. The runtime counters mirror D-D28's CI-gate-plus-runtime-counter pattern. The cycle-safety substrate-lie concern is met by the adapter probe (extended `adapter-duckdb::probe`), not a CI job, since it is an Earned-Trust probe concern. | LOCKED | `ci-cd-pipeline.md` delta §3; `platform-design.md` delta §3, §8; `observability.md` delta §2.4, §3, §10 |
| D-D31 | **Mutation scope widens: `crates/scoring` (the new pure-core crate, WD-82) is added to the nightly `cargo mutants --package` list.** Kill-rate target **≥95%** (matches `claim-domain` + `scraper-domain` per ADR-006 Earned Trust). `adapter-duckdb` is NOT mutated (effect shell; covered by the extended probe + integration tests, per D-D8). Release-tag mutation re-run inherits the D-D8 blocking-on-regression gate; `crates/scoring` is now in scope. | D-D8 scopes mutation to pure-core. Slice-04 adds a GENUINELY NEW pure-core crate (the closed-form scoring formula + bucket function — the load-bearing transparency primitive, KPI-GRAPH-3 / WD-71). It MUST enter the mutation list or its correctness is unguarded: a surviving formula/bucket mutant means the by-hand reproduction (`--explain`) could silently drift from the displayed weight, or a sparse pairing could slip to `[STRONG]`, without any test failing. This is the SECOND mutation-scope widening, exactly mirroring slice-02's D-D23 (`scraper-domain`). | LOCKED | `ci-cd-pipeline.md` delta §4; `platform-design.md` delta §5 |
| D-D32 | **KPI-GRAPH feasibility: no KPI-GRAPH marked RED.** KPI-GRAPH-2, KPI-GRAPH-3, KPI-GRAPH-4 (the three guardrails) = **GREEN + RELEASE-BLOCKING** (each is a CI gate per D-D30). KPI-GRAPH-1 (north star) = **GREEN per-user / YELLOW cohort** (`graph.connection.surfaced` event + post-hoc `scripts/kpi-graph-1.jq` + D-D18 survey reuse; cohort % needs future endpoint OR PO day-30 outreach). KPI-GRAPH-6 (latency) = **GREEN per-user / YELLOW cohort percentile** (`graph.query.duration_seconds` histogram + post-hoc jq; P95 > 5 s for ≤200 bucket is INFORMATIONAL, NOT release-blocking — escalate to PO). KPI-GRAPH-5 (referenced justification) = **YELLOW** (qualitative, PO-owned 30-day survey; no honest per-user telemetry). | All 6 KPI-GRAPH have designed capture in slice-04. The YELLOW items mirror the foundation KPI-3/KPI-6, slice-03 KPI-FED-3/KPI-FED-5, slice-02 KPI-SCR-1/KPI-SCR-5 deferral — a deferred-endpoint / PO-outreach constraint, NOT a slice-04 capture gap. The three GUARDRAILS are fully GREEN AND release-blocking (the D-D26 guardrail posture applied to KPI-GRAPH). The KPI-GRAPH-6 latency budget is informational (per `outcome-kpis.md` Handoff item 3) — a sustained breach is the ADR-021 graph-store revisit trigger, not a release gate. | LOCKED | `kpi-instrumentation.md` delta §1, §9; `observability.md` delta §2.5, §10 |
| D-D33 | **The renderer-review checklist (D-D19/D-D28) gains ONE slice-04 line: "weighted/sparse renderer never collapses authors into a consensus row AND never suppresses the `[SPARSE]` honesty line."** Recorded in the release CHANGELOG ("Renderer review: passed YYYY-MM-DD"). Solo dev = self-review. | KPI-GRAPH-2 (anti-merging) and KPI-GRAPH-4 (sparse honesty) are guardrails whose CI gates (Gates 1, 3) cover CURRENT renderers, but a FUTURE renderer added in a later slice could regress without test coverage — exactly the D-D19 (KPI-FED-2) and D-D28 (KPI-SCR-3) reasoning. The checklist is the human-in-the-loop backstop for the weighted/sparse renderers specifically. | LOCKED | `kpi-instrumentation.md` delta §5; `ci-cd-pipeline.md` delta §5.5 |
| D-D34 | **No new ADR at the DEVOPS layer.** ADR-010, ADR-011, ADR-012 carry forward unchanged. Slice-04's DESIGN wave raised ADR-020 (verb amendment), ADR-021 (recursive-CTE traversal — the WD-8 store revisit resolution), ADR-022 (pure scoring core + anti-merging-in-aggregates) — those are DESIGN ADRs. Slice-04's DEVOPS decisions (six gate ATs + four explorer-scenario ATs, the mutation-scope widening to `crates/scoring`, the KPI-GRAPH instrumentation events) are CI/observability tactical extensions of D-D8, D-D17, D-D19, D-D23, D-D28; none crosses the DEVOPS-ADR threshold. **No external contract test is needed** — slice-04 is a read-only LOCAL slice (WD-92 / DESIGN §6.4); it consumes no external API. | ADR convention: cross-slice or cross-component architectural decisions. The WD-8 store revisit IS an ADR — but it is ADR-021 (DESIGN), not a DEVOPS ADR. The mutation-scope widening and the new gate jobs are tactical applications of D-D8 and D-D30, not new axes. Same outcome as slice-03's D-D21 and slice-02's D-D29. | LOCKED | `platform-design.md` delta §9; `ci-cd-pipeline.md` delta §3.8, §5.3 |

## Proposed (awaiting user confirmation)

None. All slice-04 DEVOPS decisions are LOCKED. (In auto-mode the recommended
verdicts are taken per the auto-mode product-defaults instruction; the user
may override any D-D30..D-D34 on review.)

## Open questions (handed to DESIGN — already answered in parallel; recorded for traceability)

DESIGN ran in parallel and resolved every cross-wave question DEVOPS would
otherwise hand back. Recorded so the trace is complete:

1. **Store revisit (swap vs augment)** — RESOLVED by WD-81 / ADR-021 (AUGMENT
   DuckDB with recursive CTEs; no graph store). The DEVOPS consequence: no
   second backup target, no store-sync probe, no new substrate cell (zero new
   external dependency; `platform-design.md` §5–§6).
2. **New pure-core crate** (`crates/scoring`) — RESOLVED by WD-82 / ADR-022
   (a NEW pure crate, not a `claim-domain` module). The DEVOPS consequence:
   the mutation `--package` list widens (D-D31), the `arch-check` pure-core
   allowlist gains `crates/scoring`.
3. **`StoragePort` extension vs new port** — RESOLVED by WD-83 (EXTENSION). The
   probe extends the existing `adapter-duckdb` probe; no new probe surface.
4. **Explorer verbs imply federated scope** — RESOLVED by WD-87 (implied
   federated; bare `--subject` unchanged). DEVOPS instruments `federated: bool`
   on `graph.query.executed`; the default-scope assertion is DISTILL's.
5. **Countered claims in scoring** — RESOLVED by WD-85 (contribute normally;
   counter shown in `--explain`/traversal, not silently applied). DEVOPS needs
   no special instrumentation; the counter relationship is a render concern.
6. **Recursive-CTE cycle safety** — RESOLVED by WD-91 / ADR-021 (depth-bounded
   + visited-set guard). The DEVOPS consequence: the adapter probe scenario (b)
   (the substrate-lie check) + the `RecursiveCteCycleUnsafe` startup refusal.

## Open questions (handed to DELIVER)

These are deliberately deferred to DELIVER. DEVOPS has defaults; DELIVER decides.

1. **`scripts/kpi-graph-{1,2,3,4,6}.jq` snippets** — DELIVER lands these
   alongside the foundation `kpi-{1,2,4,5}.jq`, slice-03 `kpi-fed-*.jq`, and
   slice-02 `kpi-scr-*.jq` snippets (per `observability.md` delta §4.1).
2. **`openlore stats --explorer` flag implementation** — concrete only if D-D5
   verb landed; otherwise the `scripts/kpi-graph-*.jq` snippets are the
   fallback.
3. **`graph.query.duration_seconds` emission shape** — DELIVER threads the
   wall-clock from dispatch to last-row-rendered onto `graph.query.executed` as
   a histogram field (per `observability.md` delta §2.1); no correlation field
   needed (single-invocation timing, no cross-event join).
4. **`graph.connection.surfaced` dedup key** — DELIVER dedups by
   `(object, spanning_author_did)` within an invocation (per `observability.md`
   delta §2.3) so a span is counted once per query.
5. **Cyclic-graph traversal fixture** for the adapter probe scenario (b) and
   the `at-traversal-invents-no-edges` cycle case — DELIVER writes the A→B→A
   two-claim fixture; it is a LOCAL DuckDB seed, NOT an external/wiremock
   fixture (slice-04 has no external surface).
6. **`crates/scoring` module split** (single `lib.rs` vs `formula.rs` +
   `bucket.rs` + `types.rs`) — DELIVER's call (Q-DELIVER-3); affects mutation
   granularity but not scope (the whole crate is in `--package`).
7. **Renderer-review checklist content** — DELIVER drafts the slice-04 line
   (D-D33); DEVOPS reviews at release-tag time.

## Out of scope for DEVOPS slice-04 (explicit deferrals)

All foundation + slice-03 + slice-02 deferrals (SLOs/SLAs, runbooks,
dashboards, telemetry endpoint, auto-updater, multi-tenancy, DR, capacity,
chaos, Windows, push-based federation, multi-source scrapers) carry forward
unchanged. Slice-04 adds these explicit deferrals:

- **Persisted / federated scores**: NOT designed (WD-72 / WD-89; Gate 4
  forbids any write path). A future slice needing persistence requires a WD +
  ADR. There is no caching layer for weights — they are recomputed at query
  time.
- **ML / learned weighting infrastructure**: NOT designed (WD-71 forbids). The
  formula is a compile-time `const` SSOT (WD-86); any tuning is a code + test
  change, never config or a learned weight. No model registry, no training
  pipeline, no feature store.
- **Cohort dashboards for KPI-GRAPH-1 / KPI-GRAPH-6**: NOT designed (no central
  aggregation; per ADR-010). Per-user `openlore stats --explorer` + jq fallback
  is the slice-04 surface. Cohort is a future-telemetry-endpoint problem.
- **Graph-store swap (Kùzu / petgraph / dedicated graph DB)**: NOT designed for
  slice-04 (WD-81 / ADR-021 resolved to AUGMENT). The revisit trigger is
  documented in ADR-021 (dogfood ≤200-claim P95 breach, or `peer_claims` > ~100k
  rows, or unbounded deep traversal becoming a JTBD); a swap requires a new ADR
  and a sync-consistency probe — NOT a slice-04 concern.
- **Unbounded / deep traversal**: NOT designed (WD-76 bounded default depth 2).
  Deep traversal would be the graph-store revisit trigger, not a CTE tuning.
- **External contract tests for slice-04**: NONE (WD-92; DESIGN §6.4).
  Slice-04 consumes no external API; the existing Pact suites
  (`contract-pact-pds`, `-peer`, `-github`) are unchanged and gain nothing from
  slice-04.

## Handoff summary

| Recipient | Reads | Produces |
|---|---|---|
| DELIVER (software-crafter — functional, per ADR-007) | every slice-04 DEVOPS doc + every slice-04 DESIGN doc (WD-80..93, ADR-020..022) + slice-01/02/03 carryover + the Q-DELIVER + open-questions lists | Additions to `.github/workflows/ci.yml` (ten new acceptance jobs §3: six gate ATs + four explorer-scenario ATs) and `.github/workflows/nightly.yml` (mutation `--package` += `crates/scoring`); the cyclic-graph traversal LOCAL fixture (A→B→A two-claim seed); `scripts/kpi-graph-{1,2,3,4,6}.jq` snippets; `tracing` event emission code at the explorer/traversal/scoring boundaries (`graph.query.executed`, `graph.query.duration_seconds`, `graph.traverse.{started,completed}`, `graph.connection.surfaced`, `graph.score.{computed,explained,attribution_missing}`); the two runtime guardrail counters; the extended `adapter-duckdb::probe` (scoring-feed attribution + recursive-CTE cycle safety + depth bound); the `arch-check` rule extension + pure-core allowlist entry; one line added to the renderer-review checklist (D-D33). |
| DISTILL (nw-acceptance-designer) | the six integration gates (`scoring_aggregate_preserves_attribution`, `weight_equals_formula`, `sparse_renders_sparse`, `weight_and_bucket_never_persisted`, `traversal_invents_no_edges`, `scoring_uses_numeric_confidence`) + the four explorer scenarios + the Q-DELIVER-SCORE-1 bucket rule + the WD-87 default-federated-scope assertion | Executable acceptance tests for the explorer scenarios + the six gates, consuming the DEVOPS-defined event shapes and the local fixtures |
| Operations team (POST-DELIVER) | not applicable — still local-first CLI, no operations team for slice-04 | not applicable |
| Future DEVOPS wave (slice-05 AppView or whichever sibling stands up the telemetry endpoint) | this doc + foundation/slice-03/slice-02 DEVOPS docs + ADR-010 + `kpi-instrumentation.md` event-shape definitions | cohort aggregation for KPI-GRAPH-1 (% surfacing a connection) and KPI-GRAPH-6 (latency percentiles) — the two slice-04 cohort YELLOWs; KPI-GRAPH-5 remains PO-survey-owned |

## Changelog

- 2026-05-28 — Apex — initial DEVOPS-wave decisions for slice-04 (openlore-scoring-graph). All decisions D-D30..D-D34 LOCKED. No new DEVOPS ADRs proposed (D-D34). Foundation D-D1..D-D13 + ADR-010..012, slice-03 D-D14..D-D21, and slice-02 D-D22..D-D29 carry forward unchanged. Mutation scope widened to add `crates/scoring` (D-D31) — SECOND scope widening since slice-01 (after slice-02's `scraper-domain`, D-D23); `CLAUDE.md` Mutation Testing Strategy section unchanged in POLICY (nightly-only per D-D8), only the `--package` list grows. Three release-blocking GUARDRAIL gates added (D-D30): anti-merging-in-aggregates (Gate 1 / KPI-GRAPH-2), scoring-transparency (Gate 2 / KPI-GRAPH-3), sparse-honesty (Gate 3 / KPI-GRAPH-4). Zero new external contract surface (WD-92).
