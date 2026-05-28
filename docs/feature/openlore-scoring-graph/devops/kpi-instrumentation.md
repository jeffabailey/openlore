# KPI-GRAPH Instrumentation — openlore-scoring-graph (slice-04)

- **Wave**: DEVOPS
- **Date**: 2026-05-28
- **Architect**: Apex
- **Source-of-truth KPIs**: `docs/feature/openlore-scoring-graph/discuss/outcome-kpis.md`
- **Foundation cross-link**: `docs/feature/openlore-foundation/devops/kpi-instrumentation.md` (KPI-1..6 — still in force); slice-03 KPI-FED-1..6; slice-02 KPI-SCR-1..5 — all in force, unchanged

This document traces each of the 6 outcome KPI-GRAPH targets to **what**
measures it, **where** the data lives, **how** the developer-as-operator reads
it, and a **feasibility tag** (GREEN/YELLOW/RED) for slice-04. The three
GUARDRAILS (KPI-GRAPH-2, KPI-GRAPH-3, KPI-GRAPH-4) get **release-blocking**
treatment — each is a DISCUSS disprover (any failure is unshippable).

## 1. Summary table

| KPI | Type | Instrumentation | Read mechanism | Feasibility |
|---|---|---|---|---|
| KPI-GRAPH-1 (≥60% of explorer sessions surface a non-obvious connection in 30d) | Leading / **North Star** | `tracing` event `graph.connection.surfaced` + post-hoc `scripts/kpi-graph-1.jq` + 30-day think-aloud (reuse D-D18 survey) | per-user: `openlore stats --explorer`. Cohort: requires future telemetry endpoint OR PO day-30 outreach | **GREEN per-user / YELLOW cohort** |
| KPI-GRAPH-2 (100% — zero attribution loss in any aggregate) | Leading (**Guardrail**) | AT `at-scoring-aggregate-preserves-attribution` (Gate 1) + runtime counter `scoring_attribution_missing_total` + 5-user day-30 think-aloud | CI status (RELEASE-BLOCKING) + `openlore stats --explorer` | **GREEN** |
| KPI-GRAPH-3 (100% — every weight reproducible from formula + `--explain`) | Leading (**Outcome / Guardrail**) | property test `weight_equals_formula` + AT `at-weight-equals-formula` (Gate 2) + `--explain` arithmetic test + runtime counter `scoring_weight_irreproducible_total` + day-30 interview | CI status (RELEASE-BLOCKING) + `openlore stats --explorer` | **GREEN** |
| KPI-GRAPH-4 (100% — sparse renders sparse; zero manufactured confidence) | Leading (**Guardrail**) | AT `at-sparse-renders-sparse` (Gate 3) + `single_claim_is_sparse_even_at_high_confidence` unit test + adversarial renderer review (D-D33) | CI status (RELEASE-BLOCKING) + release-time checklist | **GREEN** |
| KPI-GRAPH-5 (≥1 referenced justification per dogfood explorer in 30d) | Leading (Outcome) | 30-day survey + qualitative session notes (PO-owned; no CI hook) | PO out-of-band day-30 outreach (no per-user telemetry signal in slice-04) | **YELLOW** (qualitative; PO-owned — same as foundation KPI-3/KPI-6) |
| KPI-GRAPH-6 (query→traverse→weighted/explain < 5 s for ≤200 claims, local) | Leading (Outcome) | `tracing` histogram `graph.query.duration_seconds` + post-hoc `scripts/kpi-graph-6.jq` | per-user: `openlore stats --explorer`. Cohort: deferred | **GREEN per-user / YELLOW cohort percentile** |

**No KPI-GRAPH is RED.** Every one has a designed capture mechanism that ships
in slice-04. The three GUARDRAILS are fully GREEN and release-blocking. The
YELLOW items (KPI-GRAPH-1 cohort %, KPI-GRAPH-5 qualitative, KPI-GRAPH-6 cohort
percentile) reflect the SAME deferred-cohort-endpoint constraint that
foundation KPI-3/KPI-6, slice-03 KPI-FED-3/KPI-FED-5, and slice-02
KPI-SCR-1/KPI-SCR-5 carry — NOT a slice-04 capture gap. This is the D-D17 /
D-D26 GREEN/YELLOW policy applied to KPI-GRAPH.

## 2. KPI-GRAPH-1 — Non-obvious connection in a single session (NORTH STAR)

### What
Per `outcome-kpis.md` §North Star: ≥60% of dogfood explorer sessions surface
≥1 non-obvious connection (a contributor spanning ≥2 candidate projects, or a
philosophy clustering they had not noticed) within 30 days. This is the
behavioral validation of J-002 — the whole point of the graph is to surface
alignment a developer could not get from `gh search` + skimming READMEs.

### Where the data lives
- **Per-user behavioral signal**: JSON log event `graph.connection.surfaced{object, distinct_subjects_spanned, spanning_author_did}` (per `observability.md` delta §2.3), emitted when a `--traverse`/`--weighted` run yields a cross-project span. Counter `graph_connections_surfaced_total` (per `observability.md` delta §4).
- **Session denominator**: count of explorer sessions (`graph.query.executed` with `traverse` or `weighted` true) per 30-day window.
- **Per-user qualitative signal**: 30-day think-aloud study (PO-owned), plus the one-shot Likert survey reusing the D-D18 / D-D27 file-presence mechanism, delivered after the FIRST `graph.connection.surfaced` event.

### How the developer reads it
- **Per-user**: `openlore stats --explorer` shows `Connections surfaced: N`; the % needs the session join (`scripts/kpi-graph-1.jq`, post-hoc per `observability.md` §2.5).
- **Per-user fallback**: `scripts/kpi-graph-1.jq` (sessions-with-≥1-connection / total-explorer-sessions; works offline against the JSON log).
- **Cohort**: NOT in slice-04. Out-of-band: Luna (PO) coordinates direct outreach to dogfood explorers at day-30 for the think-aloud + the connection-rate, AS the slice-01/02/03 models did.

### Aggregation across cohort
- **Per-user signal**: GREEN, fully captured in slice-04.
- **Cohort %**: requires the future opt-in telemetry endpoint OR PO day-30 outreach. **Telemetry rule**: ONLY the boolean "surfaced-≥1-connection-in-session" and the session count can ever be sent; `object` and `spanning_author_did` are NEVER sent (per `observability.md` delta §7).

### Feasibility: GREEN per-user / YELLOW cohort
Event + survey delivery ships in slice-04. Cohort % deferred (same constraint
as foundation KPI-3). The per-user signal suffices for the dogfood-cohort PO
outreach within 30 days of release.

## 3. KPI-GRAPH-2 — Zero attribution loss in any aggregate (GUARDRAIL, RELEASE-BLOCKING)

### What
100% attribution fidelity in any weighted/scored/traversed AGGREGATE. The user
can correctly identify the author DID(s) behind any aggregate result they are
shown. ZERO merged-into-aggregate rows; no "consensus weight" without the
contributing authors visible. This extends slice-03 KPI-FED-1/KPI-FED-2 into
the NEW aggregate surface (the weight).

### Where the data lives
- **CI signal (PRIMARY)**: `at-scoring-aggregate-preserves-attribution` test result (Gate 1; `ci-cd-pipeline.md` delta §3.1). It seeds 3 distinct-author claims on one `(subject, object)` and asserts the weight decomposes via `--explain` to 3 attributed contributions, each with a non-null `author_did`; asserts ZERO synthesized-author rows; asserts the aggregation happened in pure Rust (the per-claim `Contribution`s exist as rows), NOT in SQL `SUM/GROUP BY` (WD-88).
- **Structural signal**: the `xtask check-arch` rule `no_cross_table_join_elides_author` EXTENDS to the scoring-feed + traversal SQL (WD-88 layer 2; runs in the existing `arch-check` stage).
- **Type-level signal**: `Contribution.author_did` and `GraphEdge.author_did` are non-`Option<Did>`; `WeightedPairing.contributions` is non-empty by construction (WD-88 layer 1; compile error if dropped).
- **Runtime signal**: counter `scoring_attribution_missing_total` aggregates `graph.score.attribution_missing` events. Target = 0 forever; non-zero is a P0 bug (mirror of slice-03 `peer_render_attribution_missing_total`).

### How the developer reads it
- **CI**: GitHub Actions check; RELEASE-BLOCKING via branch protection. A failing `at-scoring-aggregate-preserves-attribution` is the KPI-GRAPH-2 disprover alert.
- **Runtime**: `openlore stats --explorer` shows `Attribution-missing in aggregates: 0` (anything else is P0).
- **Qualitative**: 5-user day-30 think-aloud ("which author DID is behind this weight?") per `outcome-kpis.md` Measured-By column.

### Aggregation across cohort
CI signal IS the cohort signal — if the test passes, EVERY user's binary has
the property (same logic as slice-03 KPI-FED-1 + foundation KPI-4). The
three-layer enforcement (type / structural / behavioral) means a single-layer
bypass is caught by ≥1 other.

### Feasibility: GREEN
CI gate (Gate 1) + structural rule + type-level guarantee + runtime counter all
designed. Release-blocking.

## 4. KPI-GRAPH-3 — Every weight reproducible (GUARDRAIL, RELEASE-BLOCKING)

### What
100% of weights are reproducible: the displayed `adherence_weight` equals the
documented formula `sum(confidence x author_distinct_bonus x cross_project_triangulation_bonus)`
(WD-86 default constants) applied to exactly the displayed claims, reproducible
by hand from the `--explain` output. No opaque, ML, or non-reproducible weight
(WD-71 forbids any drift toward an opaque/learned model).

### Where the data lives
- **CI signal (PRIMARY)**: `at-weight-equals-formula` test result (Gate 2; `ci-cd-pipeline.md` delta §3.2) + the pure-core property test `weight_equals_sum_of_contributions` (DESIGN §5.1 #3, in the existing `test-property` stage). The AT parses `--explain` output and asserts the running per-claim sum equals the displayed weight byte-for-byte, and that the formula text is printed.
- **Mutation signal**: `crates/scoring` in the nightly mutation `--package` list (D-D31; ≥95% kill rate). Mutation testing is Earned Trust applied to the TEST — a surviving formula mutant means the by-hand reproduction could silently drift from the displayed weight without the test failing. This is why scoring MUST be mutated.
- **Runtime signal**: counter `scoring_weight_irreproducible_total` aggregates `graph.score.explained` events where `weight_reproduced == false`. Target = 0 forever; non-zero is a P0 bug.

### How the developer reads it
- **CI**: GitHub Actions check; RELEASE-BLOCKING. A failing `at-weight-equals-formula` (or a mutation regression in `crates/scoring`) is the KPI-GRAPH-3 disprover alert.
- **Runtime**: `openlore stats --explorer` shows `Weight-irreproducible: 0`.
- **Qualitative**: day-30 "could you explain why this ranked first?" interview (PO-owned) per `outcome-kpis.md` Measured-By.

### Aggregation across cohort
CI signal IS the cohort signal. The formula is a compile-time `const` SSOT
(WD-86); every binary computes the same weight from the same constants.

### Feasibility: GREEN
CI gate (Gate 2) + property test + mutation scope (`crates/scoring`) + runtime
counter all designed. Release-blocking.

## 5. KPI-GRAPH-4 — Sparse renders sparse (GUARDRAIL, RELEASE-BLOCKING)

### What
100% of single-claim / single-author subgraphs render with `[SPARSE]` + a
"based on N claims by M authors" honesty line; 0 cases of confidence
manufactured from thin evidence. A single high-confidence claim must look thin,
never `[STRONG]`. The bucket is driven by evidence BREADTH (`claim_count`,
`distinct_author_count`), not weight magnitude (WD-90).

### Where the data lives
- **CI signal (PRIMARY)**: `at-sparse-renders-sparse` test result (Gate 3; `ci-cd-pipeline.md` delta §3.3) + the pure-core unit test `single_claim_is_sparse_even_at_high_confidence` (DESIGN §5.1 #5). The AT seeds a 1-claim/1-author/conf-0.95 pairing and asserts `[SPARSE]` + the honesty line, NOT `[STRONG]`. It also asserts the Q-DELIVER-SCORE-1 boundary (same-author cross-project triangulation counts toward breadth, so a triangulated single-claim pairing is NOT sparse — the one consistent rule DISTILL fixes).
- **Renderer-review signal**: the adversarial renderer review at release (D-D33) gains one line — "weighted/sparse renderer never suppresses the `[SPARSE]` honesty line and never collapses authors". Solo dev = self-review; the checklist backstops future renderers (the D-D19 / D-D28 reasoning: ATs cover current renderers, the checklist prevents a future renderer regressing without test coverage).
- **Runtime signal**: `graph.score.computed.sparse_pairings` count (per `observability.md` delta §2.4) — a sanity signal; a session over a thin graph that NEVER produces a sparse pairing is suspect.

### How the developer reads it
- **CI**: GitHub Actions check; RELEASE-BLOCKING. A failing `at-sparse-renders-sparse` is the KPI-GRAPH-4 disprover alert.
- **Release**: renderer-review checklist completion (a line in the release CHANGELOG: "Renderer review: passed YYYY-MM-DD").
- **Runtime/sanity**: `openlore stats --explorer` shows the weighted-pairing bucket distribution (strong/moderate/sparse).

### Aggregation across cohort
CI gate IS the cohort signal. The bucket function is in the pure `scoring`
core; every binary buckets identically.

### Feasibility: GREEN
CI gate (Gate 3) + unit test + renderer-review checklist line all designed.
Release-blocking.

## 6. KPI-GRAPH-5 — Referenced justification in a real decision

### What
≥1 referenced justification per dogfood explorer within 30 days — the explorer
cites a query/weighted result when justifying a stack or community choice to a
teammate. This proves the connection was decision-relevant, not just curious
(it is the KPI-GRAPH-1 leading indicator).

### Where the data lives
- **Qualitative only**: 30-day survey ("did you cite an openlore query when
  justifying a choice?") + qualitative session notes. PO-owned. There is NO
  per-user telemetry signal for "cited a result to a teammate" — that is an
  out-of-band human behavior, not observable from the local log.

### How the developer reads it
- **Cohort/per-user**: PO day-30 outreach + survey form. No CI hook, no `openlore stats` row.

### Aggregation across cohort
PO-owned out-of-band, exactly like foundation KPI-3 / slice-03 KPI-FED-3
qualitative half / slice-02 KPI-SCR-5 cohort.

### Feasibility: YELLOW
Qualitative, PO-owned. Not a slice-04 capture gap — there is no honest way to
instrument "cited to a teammate" without surveillance; the survey is the right
mechanism. Same posture as foundation KPI-3.

## 7. KPI-GRAPH-6 — End-to-end query→traverse→weighted/explain < 5 s (≤200 claims, local)

### What
Wall-clock for a query → traverse → weighted/explain end-to-end run on the
LOCAL graph, under 5 seconds for a subgraph of ≤200 claims (local read; no
network). Friction kills exploration (the KPI-GRAPH-1 leading indicator).

### Where the data lives
- **Per-user**: `tracing` histogram `graph.query.duration_seconds{dimension, traverse_bool, weighted_bool, claim_count_bucket}` (per `observability.md` delta §2.1), emitted on every query completion. Post-hoc aggregation via `scripts/kpi-graph-6.jq` (no state file; the events carry the durations + buckets — the D-D16 / D-D27 post-hoc default).
- **CI sanity**: `at-traverse-bounded` and `at-explain-reproduces` run against ≤200-claim fixtures; test wall-clock implicitly bounds the implementation for those fixtures (a regression signal, not a strict KPI test).

### How the developer reads it
- **Per-user (post-hoc default)**:
```
$ jq -sf scripts/kpi-graph-6.jq $XDG_DATA_HOME/openlore/logs/openlore.log
{
  "le50":     { "p50_s": 0.4, "p95_s": 0.9, "samples": 22 },
  "51to200":  { "p50_s": 1.1, "p95_s": 3.8, "samples": 14 },
  "201to1000":{ "p50_s": 4.2, "p95_s": 9.1, "samples":  5 },
  "gt1000":   { "p50_s": 0,   "p95_s": 0,   "samples":  0 }
}
```
- **Per-user (verb)**: `openlore stats --explorer` renders the bucketed P50/P95.
- **Alert**: per `outcome-kpis.md` Handoff item 3, P95 > 5 s for the ≤200
  (`51to200`/`le50`) bucket is INFORMATIONAL (do NOT block release alone;
  escalate to PO). In slice-04 the "alert" is the operator noticing on their
  own `openlore stats --explorer` output; no automated paging (no central
  metric store). A sustained breach is the ADR-021 revisit trigger toward a
  graph store.

### Aggregation across cohort
- **Per-user**: GREEN.
- **Cohort percentiles**: requires the future telemetry endpoint. The histogram
  is shaped to be aggregable (bucketed claim-count + bucketed duration); rollup
  is straightforward once an endpoint exists.

### Feasibility: GREEN per-user / YELLOW cohort percentile
Per-user full capture, readable today. Cohort deferred (same constraint as
slice-03 KPI-FED-5).

## 8. Mapping to inherited KPIs (per `outcome-kpis.md` §Mapping)

| Inherited KPI | Status in slice-04 instrumentation |
|---|---|
| KPI-4 (slice-01: zero silent normalization, 100% round-trip identity) | Inherited and EXTENDED. The CI test `at-scoring-uses-numeric-confidence` (Gate 6) extends KPI-4's no-silent-rounding into the scoring path — the numeric confidence the formula consumes is byte-equal to the value displayed. The foundation `field_mismatch_total` counter scope is unchanged; Gate 6 is the slice-04-specific extension. |
| KPI-5 (slice-01: local-first, network-disabled correctness) | Inherited and REINFORCED. The entire slice-04 explorer surface works network-disabled (WD-92). The `kpi-5-offline` integration test extends to seed a scoring fixture before the `unshare -n` step (per `ci-cd-pipeline.md` delta §3.8, §6). No external contract test needed (DESIGN §6.4). |
| KPI-FED-1 (slice-03: 100% attribution fidelity) | Inherited and EXTENDED into aggregates. KPI-GRAPH-2 carries the anti-merging guarantee into the weight (an aggregate VIEW that never loses per-claim attribution). The slice-03 runtime counter pattern is mirrored by `scoring_attribution_missing_total`. |
| KPI-FED-2 (slice-03: zero merged-consensus rows) | Inherited and EXTENDED. No "consensus weight" is shown without the contributing authors visible; `--explain` always decomposes (Gate 1 + Gate 2). The renderer-review checklist gains the slice-04 line (D-D33). |
| WD-10 / I-6 (display-only buckets; numeric-only persistence) | Inherited and EXTENDED. `weight_bucket` ([STRONG]/[MODERATE]/[SPARSE]) is display-only exactly like confidence buckets; `adherence_weight` is derived + never persisted (Gate 4 / WD-89). |

## 9. KPI-GRAPH sign-off readiness

| KPI-GRAPH | Per-user instrumented in slice-04? | Cohort aggregated? | Status |
|---|---|---|---|
| KPI-GRAPH-1 | YES (`graph.connection.surfaced` event + survey) | NO (future endpoint OR PO day-30 outreach) | **GREEN per-user / YELLOW cohort** |
| KPI-GRAPH-2 | YES (CI gate + structural rule + type guarantee + runtime counter) | YES (CI = cohort property) | **GREEN** (release-blocking) |
| KPI-GRAPH-3 | YES (CI gate + property test + mutation scope + runtime counter) | YES (CI = cohort property) | **GREEN** (release-blocking) |
| KPI-GRAPH-4 | YES (CI gate + unit test + renderer-review line) | YES (CI = cohort property) | **GREEN** (release-blocking) |
| KPI-GRAPH-5 | NO per-user telemetry (qualitative behavior) | NO (PO day-30 survey) | **YELLOW** (PO-owned qualitative) |
| KPI-GRAPH-6 | YES (post-hoc jq over `graph.query.duration_seconds`) | NO (future endpoint for cohort percentiles) | **GREEN per-user / YELLOW cohort** |

**No KPI-GRAPH is RED.** All six are at minimum per-user-readable or
PO-surveyable from the slice-04 release. The three GUARDRAILS (KPI-GRAPH-2/3/4)
are fully GREEN AND release-blocking. The YELLOW items (KPI-GRAPH-1 cohort,
KPI-GRAPH-5 qualitative, KPI-GRAPH-6 cohort percentile) reflect the same
deferred-cohort-endpoint / PO-outreach constraint that every prior slice
carries — the D-D17 / D-D26 policy applied to KPI-GRAPH. The slice IS shippable
with these YELLOWs: the per-user signal suffices for dogfood PO outreach within
30 days; cohort aggregation is the future telemetry endpoint's job (slice-05+).

## 10. Cross-references

- `observability.md` delta §2 — event names that back KPI-GRAPH-1..6 measurement
- `observability.md` delta §4 — metric rows derived from those events
- `ci-cd-pipeline.md` delta §3.1 (KPI-GRAPH-2 / Gate 1), §3.2 (KPI-GRAPH-3 / Gate 2), §3.3 (KPI-GRAPH-4 / Gate 3), §3.4–3.6 (Gates 4–6), §4 (mutation scope)
- `wave-decisions.md` delta — D-D30..D-D34
- Foundation/slice-03/slice-02 `kpi-instrumentation.md` — KPI-1..6, KPI-FED-1..6, KPI-SCR-1..5 unchanged
- `discuss/outcome-kpis.md` — authoritative KPI-GRAPH definitions + §Disprovers + §Handoff to DEVOPS
- ADR-010 — telemetry-opt-in (governs the YELLOW path for KPI-GRAPH-1/5/6 cohort)
