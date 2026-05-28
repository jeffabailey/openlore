# Outcome KPIs — openlore-scoring-graph (slice-04)

- **Wave**: DISCUSS
- **Date**: 2026-05-28
- **Owner**: Luna (nw-product-owner)

## Feature: openlore-scoring-graph

### Objective

A graph-explorer walks away from a single session able to make a defensible
tech/community decision — knowing WHICH projects embody a philosophy, WHO backs
that up, and HOW well-supported each signal is — with every weight transparent
and reproducible, and with sparse evidence honestly shown as sparse rather than
dressed up as confident truth.

### Outcome KPIs

| # | Who | Does What | By How Much | Baseline | Measured By | Type |
|---|-----|-----------|-------------|----------|-------------|------|
| KPI-GRAPH-1 | Graph-explorer users (P-002 + P-001 in explorer hat) | Surface a non-obvious connection (a contributor spanning >=2 of their candidate projects, or a philosophy clustering they had not noticed) in a single query session | >=60% of dogfood explorer sessions surface >=1 such connection within 30 days | 0 (no traversal/weighted view exists today) | Author-side telemetry: `graph.connection.surfaced` event on traversal/weighted runs that yield a cross-project span; 30-day think-aloud study | Leading (Outcome) |
| KPI-GRAPH-2 | Graph-explorer users | Correctly identify the author DID(s) behind any weighted/scored or traversed result they are shown | 100% (zero attribution loss in any aggregate) | n/a (new behavior) | Acceptance test `scoring_aggregate_preserves_attribution` + 5-user think-aloud at day-30 | Leading (Guardrail) |
| KPI-GRAPH-3 | Graph-explorer users | Reproduce a displayed adherence weight by hand from the formula + `--explain` output (transparency check) | 100% of weights are reproducible (weight == documented formula applied to displayed claims) | n/a (new behavior) | Property test `weight_equals_formula` + `--explain` arithmetic test; day-30 "could you explain why this ranked first?" interview | Leading (Outcome / Guardrail) |
| KPI-GRAPH-4 | Graph-explorer users encountering a thin subgraph | Recognize a sparse subgraph AS sparse and treat it as a lead, not a conclusion | 100% of single-claim/single-author subgraphs render with [SPARSE] + honesty line; 0 cases of confidence manufactured from thin evidence | n/a (new behavior) | Acceptance test `sparse_renders_sparse` + adversarial review of every weighted renderer | Leading (Guardrail) |
| KPI-GRAPH-5 | Graph-explorer users | Refer to a query/weighted result when justifying a stack or community choice to a teammate | >=1 referenced justification per dogfood explorer within 30 days | 0 (no such artifact exists today) | 30-day survey ("did you cite an openlore query when justifying a choice?") + qualitative session notes | Leading (Outcome) |
| KPI-GRAPH-6 | Graph-explorer users | Complete query -> traverse -> weighted/explain end-to-end on the LOCAL graph | In under 5 seconds for a subgraph of <=200 claims (local read; no network) | n/a (new; comparable to slice-01 KPI-1 local-first speed) | Author-side timing telemetry `graph.query.duration_seconds` histogram | Leading (Outcome) |

### Metric Hierarchy

- **North Star**: **KPI-GRAPH-1** — % of dogfood explorer sessions that surface a
  non-obvious connection in a single query. This is the behavioral validation of
  J-002: the whole point of the graph is to surface alignment a developer could
  not get from `gh search` + skimming READMEs. The slice's value evaporates if
  users CAN traverse/weight but never discover a non-obvious connection.
- **Leading Indicators**: KPI-GRAPH-5 (referenced justification — proves the
  connection was decision-relevant, not just curious), KPI-GRAPH-6 (latency —
  friction kills exploration behavior).
- **Guardrail Metrics**: KPI-GRAPH-2 (anti-merging in aggregates — zero
  attribution loss), KPI-GRAPH-3 (scoring transparency — every weight
  reproducible), KPI-GRAPH-4 (sparse renders sparse — zero manufactured
  confidence). All three MUST hold; any failure is unshippable.

### Mapping to inherited KPIs

| Inherited KPI | Status in slice-04 |
|---|---|
| KPI-4 (slice-01: zero silent normalization, 100% round-trip identity) | Inherited UNCHANGED — scoring reads stored claims as-is; the numeric confidence shown == the numeric confidence scored (no silent rounding); KPI-GRAPH-3 Gate 6 enforces. |
| KPI-5 (slice-01: local-first, network-disabled correctness) | Inherited and REINFORCED — scoring/traversal read LOCAL stores only; the entire slice-04 surface works with the network disabled. |
| KPI-FED-1 (slice-03: 100% attribution fidelity) | Inherited and EXTENDED — KPI-GRAPH-2 carries the anti-merging guarantee into aggregates: a weight is an aggregate VIEW that never loses per-claim attribution. |
| KPI-FED-2 (slice-03: zero merged-consensus rows) | Inherited and EXTENDED — no "consensus weight" is shown without the contributing authors visible; `--explain` always decomposes. |
| WD-10 / I-6 (display-only buckets; numeric-only persistence) | Inherited and EXTENDED — `weight_bucket` ([STRONG]/[MODERATE]/[SPARSE]) is display-only exactly like confidence buckets; `adherence_weight` is derived + never persisted (WD-72). |

### Measurement Plan

| KPI | Data Source | Collection Method | Frequency | Owner |
|-----|------------|-------------------|-----------|-------|
| KPI-GRAPH-1 | author-side telemetry + day-30 think-aloud | tracing event `graph.connection.surfaced` + manual session | continuous + once | DEVOPS (telemetry), nw-product-owner (session) |
| KPI-GRAPH-2 | acceptance test + dogfood think-aloud | automated test (CI) + manual session (day-30) | continuous + once | DEVOPS (CI), nw-product-owner (session) |
| KPI-GRAPH-3 | property test + `--explain` test + day-30 interview | automated test (CI) + manual interview | continuous + once | DEVOPS (CI), nw-product-owner (interview) |
| KPI-GRAPH-4 | acceptance test `sparse_renders_sparse` + adversarial renderer review | automated test (CI) + manual review | continuous + once per release | DEVOPS (CI), nw-product-owner (review) |
| KPI-GRAPH-5 | 30-day survey + qualitative session notes | survey form + notes | once at day-30 | nw-product-owner |
| KPI-GRAPH-6 | author-side timing telemetry (`graph.query.duration_seconds` histogram) | tracing histogram on query completion | continuous | DEVOPS |

### Hypothesis

We believe that **a CLI that lets a developer query the local federated graph by
subject/object/contributor, traverse its edges, and see a SMALL transparent
adherence weight (count x confidence x triangulation, no ML) with sparse data
rendered honestly as sparse** will achieve **a 30-day dogfood explorer cohort
that surfaces a non-obvious connection in >=60% of sessions and cites a query
result when justifying a real stack/community choice.**

We will know this is true when **dogfood explorers surface >=1 non-obvious
connection per session in 60% of sessions within 30 days, AND at least one
explorer per cohort cites an openlore query when justifying a decision to a
teammate, AND every weight they were shown was reproducible by hand from the
formula.**

### Disprovers (kill criteria for the scoring-graph hypothesis)

These outcomes would kill the slice-04 hypothesis and force a re-design before
slice-05:

1. **KPI-GRAPH-2 < 100%**: any attribution loss in a weighted/traversed
   aggregate is a fatal failure of the trust model carried from slice-03. The
   query/scoring shape would need to change.
2. **KPI-GRAPH-3 < 100%**: any non-reproducible / opaque weight breaks the J-002
   transparency promise. The formula or its display would need a redesign (and
   any drift toward an opaque/ML model is forbidden by WD-71).
3. **KPI-GRAPH-4 < 100%**: any case where thin evidence is rendered as a
   confident score directly causes the J-002 "bad call on sparse data" anxiety to
   materialize. Halts release.
4. **KPI-GRAPH-1 < 20%**: a near-zero non-obvious-connection rate suggests the
   traversal/weighting does not actually surface insight users could not get
   elsewhere — the J-002 value thesis is weakened. Re-investigate the traversal
   UX and the default formula before slice-05.

### Handoff to DEVOPS

The platform-architect needs these from this document to plan instrumentation:

1. **Data collection requirements**:
   - Author-side tracing event `graph.connection.surfaced{object, distinct_subjects_spanned, spanning_author_did}` emitted when a traversal/weighted run yields a cross-project span (KPI-GRAPH-1).
   - Author-side tracing histogram `graph.query.duration_seconds{dimension, traverse_bool, weighted_bool, claim_count_bucket}` on every query completion (KPI-GRAPH-6).
   - NO telemetry on the contents of claims (local-first, privacy-preserving); only structural counts.

2. **Dashboard/monitoring needs**:
   - KPI-GRAPH-1 dashboard: % of explorer sessions with >=1 `graph.connection.surfaced` event per 30-day window (dogfood-only initially).
   - KPI-GRAPH-6 dashboard: P50/P95 of `graph.query.duration_seconds` per claim-count bucket (<=50, 51-200, 201-1000, >1000).

3. **Alerting thresholds**:
   - Alert if any CI run reports KPI-GRAPH-2 != 100% (release-blocking).
   - Alert if any CI run reports KPI-GRAPH-3 != 100% (release-blocking).
   - Alert if any CI run reports KPI-GRAPH-4 != 100% (release-blocking).
   - Alert if P95 of KPI-GRAPH-6 exceeds 5 seconds for the <=200 bucket (informational; escalate to PO; do NOT block release alone).

4. **Baseline measurement**: no baselines needed; all KPIs are for new behavior.
   KPI-GRAPH-1 and KPI-GRAPH-5 baselines are implicitly 0 (no traversal/weighted
   view or shareable query result exists today).
