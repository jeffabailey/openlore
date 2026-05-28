# Observability Extension — openlore-scoring-graph (slice-04)

- **Wave**: DEVOPS
- **Date**: 2026-05-28
- **Architect**: Apex

This is the slice-04 **delta** to `observability.md` (foundation). Operating
model (developer-as-operator; local-first; no remote sink; telemetry opt-in
OFF by default; no dashboards; no alerting) is **UNCHANGED**. Slice-04 is a
read-only LOCAL slice (WD-79 / WD-92) — there is NO new endpoint, NO daemon,
NO network surface. All new events flow into the SAME JSON Lines pipeline
established in D-D2 (slice-01) and reused unchanged by slice-02/03. This doc
adds:

1. New `tracing` event names emitted by the explorer code paths.
2. New per-event metric rows (derived on-demand from the log).
3. New probe events for the AUGMENTED `adapter-duckdb` (recursive-CTE paths).
4. New `openlore stats` rendering rows (assumes D-D5 verb landed; otherwise
   the same fallback applies — read events directly via `jq`).

## 1. Pillars (UNCHANGED)

Logs YES (same JSON Lines sink — D-D2; reused, no new endpoint). Metrics YES
(same on-demand aggregation from the log). Traces still DEFERRED with the same
revisit trigger (slice-04 is still a single-binary CLI; an explorer query is a
single LOCAL DuckDB read from the user's machine — nothing distributed; no
network at all per WD-92). The `tracing` span per `graph query` invocation is
in-process scoping for log enrichment, NOT distributed tracing (D-D3 carries
forward).

## 2. Logging — new events emitted

All events below are author-side, privacy-preserving STRUCTURAL counts only.
Per `outcome-kpis.md` §Handoff item 1: **NO telemetry on the contents of
claims**; only structural counts (subject/object identifiers and DIDs are
local identifiers the user already sees on screen; claim BODIES are never in
any event). The five new event families back KPI-GRAPH-1..4 + KPI-GRAPH-6.

### 2.1 Explorer-query boundary events (mandatory)

- `graph.query.executed` — payload `{ dimension: "subject"|"object"|"contributor", federated: bool, traverse: bool, weighted: bool, explain: bool, claim_count_bucket: "le50"|"51to200"|"201to1000"|"gt1000", row_count: u32 }`. Emitted once per `graph query` invocation, on completion. The load-bearing event for understanding which explorer paths users exercise; `federated` is `true` by default for explorer flags (WD-87).
- `graph.query.duration_seconds` — a histogram observation (NOT a discrete event in the discrete-event sense; emitted as a structured field on the `graph.query.executed` event AND as a histogram bucket the on-demand metric aggregation reads) — payload tags `{ dimension, traverse_bool, weighted_bool, claim_count_bucket }`, value = wall-clock seconds from dispatch to last row rendered. This is the KPI-GRAPH-6 source (the local-first read-budget latency). Per `outcome-kpis.md` §Handoff item 1.

### 2.2 Traversal boundary events (mandatory)

- `graph.traverse.started` — payload `{ dimension, root, depth_requested: u8 }`. Emitted when `--traverse` begins the recursive-CTE walk.
- `graph.traverse.completed` — payload `{ depth: u8, edges: u32, edges_omitted_at_bound: u32, cycle_guard_hits: u32, distinct_subjects_spanned: u32 }`. Emitted once per traversal. `edges_omitted_at_bound` is the depth-bound report (Gate 5 / WD-91); `cycle_guard_hits` is the visited-set guard counter (non-zero is normal on a cyclic graph; it proves the substrate-lie guard fired rather than looping). `distinct_subjects_spanned` feeds the cross-project-span detection (KPI-GRAPH-1).

### 2.3 Connection-discovery event (mandatory — KPI-GRAPH-1 NORTH STAR)

- `graph.connection.surfaced` — payload `{ object: String, distinct_subjects_spanned: u32, spanning_author_did: Did }`. Emitted when a `--traverse` or `--weighted` run yields a CROSS-PROJECT span — a contributor spanning ≥2 of the candidate subjects/projects, or a philosophy clustering across ≥2 subjects. Per `outcome-kpis.md` §Handoff item 1, this is the load-bearing measurement for KPI-GRAPH-1 (the north star). Emitted at most once per surfaced connection per query (deduped by `(object, spanning_author_did)` within the invocation). NO claim content; only the structural span (the object identifier + the spanning DID + the count, all of which the user sees on screen).

### 2.4 Scoring boundary events (mandatory)

- `graph.score.computed` — payload `{ pairings: u32, contributions_total: u32, sparse_pairings: u32, strong_pairings: u32, moderate_pairings: u32, distinct_authors: u32 }`. Emitted once per `--weighted` invocation, AFTER the pure `scoring` core returns the `WeightedView`. `sparse_pairings` is the count of `[SPARSE]`-bucketed pairings (a sanity signal for KPI-GRAPH-4 — a session that NEVER produces a sparse pairing on a thin graph is suspect). `contributions_total >= pairings` always (every pairing decomposes to ≥1 contribution; the anti-merging structural sanity per WD-88).
- `graph.score.explained` — payload `{ subject: String, contributions: u32, weight_reproduced: bool }`. Emitted on a `--explain <subject>` invocation. `weight_reproduced` MUST be `true` (the displayed weight equals the sum of the rendered contributions; the runtime mirror of Gate 2). A `false` emission is a P0 bug-worthy event (the runtime guardrail counter `scoring_weight_irreproducible_total` aggregates it; target = 0 forever).
- `graph.score.attribution_missing` — payload `{ subject: String, object: String, pairing_index: u32 }`. Emitted ONLY if a `WeightedPairing` or a traversal node is about to render with a missing/empty `author_did` (which the type system should make impossible — non-`Option<Did>`; WD-88 layer 1). This event firing AT ALL is a P0 bug; the counter `scoring_attribution_missing_total` (target = 0 forever) is the runtime guardrail mirror of Gate 1, analogous to slice-03's `peer_render_attribution_missing_total`.

### 2.5 KPI-GRAPH-1 / KPI-GRAPH-6 measurement mechanism (mandatory)

Mirrors the slice-03 D-D16 / slice-02 D-D27 POST-HOC default: KPI-GRAPH-1 and
KPI-GRAPH-6 are computed POST-HOC by `jq` aggregation over the events above —
NO state file in the binary.

- **KPI-GRAPH-1** (% of explorer sessions surfacing ≥1 non-obvious connection):
  count distinct SESSIONS (a session = a run of `graph query` invocations
  within a window; grouped by process-start correlation or a timestamp window)
  that emitted ≥1 `graph.connection.surfaced` event, divided by total explorer
  sessions (sessions that emitted ≥1 `graph.query.executed` with `traverse` or
  `weighted` true). The `jq` snippet `scripts/kpi-graph-1.jq` does the join.
- **KPI-GRAPH-6** (latency): read the `graph.query.duration_seconds` histogram
  field off `graph.query.executed`; compute P50/P95 per `claim_count_bucket`.
  The `jq` snippet `scripts/kpi-graph-6.jq` does the bucketing.

No `flow_id` / state file is persisted (the events carry the timestamps and
buckets; post-hoc aggregation is exact). This matches the D-D16 / D-D27
no-state-file reasoning.

### 2.6 Probe extension events (mandatory)

- `probe.scoring_feed` — payload `{ adapter: "adapter-duckdb", outcome: ok|refused, distinct_author_dids: u32, reason, detail }`. Emitted by the AUGMENTED `adapter-duckdb` probe scenario (a) — the scoring-feed attribution round-trip (3 claims, 3 distinct non-empty author_dids). On failure (an author dropped in the projection): `health.startup.refused{ reason: ScoringFeedAttributionLost, detail }`.
- `probe.traverse_cycle_safe` — payload `{ adapter: "adapter-duckdb", outcome: ok|refused, terminated_within_ms: u64, edges_emitted: u32, duplicate_edges: u32, reason, detail }`. Emitted by probe scenario (b) — the cyclic-fixture termination check (the substrate-lie check; A→B→A at depth 3). On non-termination within the 250 ms budget OR a duplicate edge: `health.startup.refused{ reason: RecursiveCteCycleUnsafe, detail }`. This is the load-bearing slice-04 probe — DuckDB CTEs do NOT auto-detect cycles, so the probe refuses to trust the substrate.
- `probe.traverse_depth_bound` — payload `{ adapter: "adapter-duckdb", outcome: ok|refused, requested_depth: u8, max_observed_depth: u8, omitted_count: u32, reason, detail }`. Emitted by probe scenario (c) — the depth-bound-honored check. On a depth-2 request returning a depth-3 edge: `health.startup.refused{ reason: TraversalDepthBoundViolated, detail }`.

The probe-result event-name pattern matches foundation §3.2 ("Every `probe()`
invocation emits a result event"). These extend the EXISTING `adapter-duckdb`
probe — no new adapter, no new port, so no new probe surface beyond this
extension (DESIGN §6.3, §10).

### 2.7 What did NOT change

All foundation, slice-03, and slice-02 events (verb.invoked, port.call,
port.return, compose.*, sign.success, publish.*, query.executed, health.*,
peer.pull.*, peer.claim.rendered, claim.counter.*, scrape.*,
claim.signed.from_scraper) remain emitted unchanged. The new code paths emit
ADDITIONAL events; they do not suppress or rename any existing ones. Note the
slice-04 `graph.query.executed` is a NEW richer event distinct from the
slice-01 `query.executed{kind}`; both coexist (slice-04's explorer dispatch
emits the richer one for the new flags).

## 3. Probes (extension)

Foundation `observability.md` §7.2 lists per-adapter probe responsibilities.
Slice-04 extends ONLY `adapter-duckdb` (the only adapter touched; DESIGN §10):

| Adapter | New probe responsibility for slice-04 |
|---|---|
| `adapter-duckdb` (extended `StoragePort::probe`) | (a) **scoring-feed attribution round-trip**: write 1 own + 2 peer claims on the same `(subject, object)` by distinct authors; `query_attributed_for_scoring` returns exactly 3 `AttributedClaim`s with 3 distinct non-empty `author_did`s (anti-merging substrate check). (b) **recursive-CTE termination**: `traverse_graph` on a cyclic fixture (A→B→A) at depth 3 terminates within 250 ms, each edge once (the substrate-lie check). (c) **depth-bound honored**: `traverse_graph` at `depth=2` on a depth-4 fixture returns only ≤depth-2 edges + correct omitted count. |

Slice-04 introduces NO new adapter and NO network probe (WD-92). The
`--offline` flag (foundation §7.3) is a no-op refinement for slice-04 — the
explorer verbs are local-only by construction, so there is no network probe to
skip; the existing offline-skip logic remains correct unchanged.

## 4. Metrics — new rows for `openlore stats`

Append to foundation §4.3 table (and the slice-03 / slice-02 additions):

| Metric | Source events | Render shape | Used by |
|---|---|---|---|
| `graph_queries_total{dimension, traverse, weighted}` | count of `graph.query.executed` grouped by the flags | counter, per-dimension breakdown | operational visibility; KPI-GRAPH usage context |
| `graph_query_duration_seconds{claim_count_bucket}` | `graph.query.duration_seconds` field | histogram (p50/p95) per bucket | KPI-GRAPH-6 (local-first read budget) |
| `graph_connections_surfaced_total` | count of `graph.connection.surfaced` | counter, breakdown by `object` available | KPI-GRAPH-1 (the north-star measurement) |
| `graph_traversals_total` | count of `graph.traverse.completed` | counter | operational visibility |
| `graph_traverse_edges_omitted_total` | sum of `graph.traverse.completed.edges_omitted_at_bound` | counter | context (how often users hit the depth bound — a tuning signal) |
| `graph_cycle_guard_hits_total` | sum of `graph.traverse.completed.cycle_guard_hits` | counter | sanity (non-zero proves the guard works; the substrate-lie guard is firing, not looping) |
| `graph_weighted_pairings_total{bucket}` | sum of `graph.score.computed.{sparse,moderate,strong}_pairings` | counter, per bucket | KPI-GRAPH-4 context (the sparse/strong distribution) |
| `scoring_weight_irreproducible_total` | count of `graph.score.explained` where `weight_reproduced == false` | counter; target = 0 forever | KPI-GRAPH-3 (runtime guardrail; non-zero is a P0 bug) |
| `scoring_attribution_missing_total` | count of `graph.score.attribution_missing` | counter; target = 0 forever | KPI-GRAPH-2 (runtime guardrail; non-zero is a P0 bug — mirror of slice-03 `peer_render_attribution_missing_total`) |

### 4.1 `openlore stats` rendering additions

Append to foundation §4.2 commands (and slice-03 `--federation`, slice-02
`--scraper`):

| Command | Renders |
|---|---|
| `openlore stats --explorer` | Summary card: total explorer queries (by dimension), traversals run, connections surfaced, query-duration p50/p95 per claim-count bucket, weighted-pairing bucket distribution (strong/moderate/sparse), and the two guardrail counters (weight-irreproducible: 0; attribution-missing: 0). |
| `openlore stats --explorer --since <date>` | Same, filtered. |
| `openlore stats --json` | Already exists; now includes the new metric rows. |

If D-D5 (the `openlore stats` verb) is deferred and the `scripts/kpi-*.jq`
snippets are the fallback, DELIVER ships:

- `scripts/kpi-graph-1.jq` — % of explorer sessions with ≥1 `graph.connection.surfaced` event (the north-star post-hoc aggregation, §2.5).
- `scripts/kpi-graph-2.jq` — asserts `scoring_attribution_missing_total == 0` AND every `graph.score.computed` has `contributions_total >= pairings` (the runtime anti-merging sanity).
- `scripts/kpi-graph-3.jq` — asserts `scoring_weight_irreproducible_total == 0` (every `graph.score.explained.weight_reproduced == true`).
- `scripts/kpi-graph-4.jq` — distribution of weighted-pairing buckets; sanity that a thin-graph fixture produces ≥1 `[SPARSE]` (else a renderer test is silently passing for the wrong reason).
- `scripts/kpi-graph-6.jq` — P50/P95 of `graph.query.duration_seconds` per claim-count bucket (§2.5).

## 5. Where logs go (UNCHANGED)

Same file: `$XDG_DATA_HOME/openlore/logs/openlore.log`. Same rotation policy.
Same stderr verbose mode. The new `graph.*` event names append to the same
JSON Lines stream (D-D2; reused, no new endpoint).

## 6. Verbosity controls (UNCHANGED)

Foundation §3.5 table carries forward unchanged. The new events emit at:

- INFO: `graph.query.executed`, `graph.traverse.started`, `graph.traverse.completed`, `graph.connection.surfaced`, `graph.score.computed`, `graph.score.explained`.
- DEBUG: the per-edge / per-contribution detail inside a traversal or scoring
  computation (if DELIVER chooses to emit per-edge debug events; not mandated).
- WARN: `graph.score.attribution_missing` and a `graph.score.explained` with
  `weight_reproduced == false`. These are guardrail-violation events — a
  rendering about to drop an author or display an irreproducible weight is a
  security/trust-relevant event; WARN surfaces it in default stderr (matching
  the slice-03 `peer.pull.rejected` WARN reasoning). They should be impossible
  (type-level + Gate-tested) — WARN ensures that if the impossible happens, the
  operator sees it immediately rather than it being silently swallowed.

## 7. Telemetry (UNCHANGED policy)

Per ADR-010 (D-D4): telemetry remains opt-in, OFF by default, no endpoint
operated. The slice-04 events are designed to be privacy-preserving STRUCTURAL
counts (per `outcome-kpis.md` §Handoff item 1 — NO claim contents, ever). If/
when the future endpoint exists, the slice-04 events eligible for telemetry
rollup are:

- `graph_queries_total` (counter only; the dimension/flag tags, no subject/object/DID).
- `graph_query_duration_seconds` histogram (bucketed; no per-query detail).
- `graph_connections_surfaced_total` (counter only; NO `object`, NO `spanning_author_did`).
- `graph_weighted_pairings_total{bucket}` (the bucket distribution, counter only).
- `scoring_weight_irreproducible_total` and `scoring_attribution_missing_total` (the two guardrail counters; both should be 0 — a non-zero rollup across the cohort is a strong P0 signal).

EXPLICITLY NEVER sent over telemetry (even when opted in):

- `graph.connection.surfaced.object` and `.spanning_author_did` (a stream would reveal the user's exploration targets and their candidate-project graph).
- `subject` / `object` identifiers from any event (reveal the user's research interests).
- Any `author_did` value (would reveal the user's subscription / contributor graph — same reasoning as slice-03's peer_did rule).
- Any claim content (slice-04 events carry NONE by design; this is belt-and-suspenders).

The existing `[telemetry]` on/off semantics suffice; no new subsection needed
for slice-04.

## 8. Health checks (extension)

Per §3 above. Startup gate behavior UNCHANGED — the binary still emits
`health.startup.refused` and exits 2 on probe refusal. The three new
`adapter-duckdb` probe scenarios (scoring-feed attribution, recursive-CTE
cycle safety, depth bound) widen what `probe_all` covers; the exit-code
semantics carry. The `RecursiveCteCycleUnsafe` refusal is the load-bearing new
refusal reason — a binary whose recursive-CTE could loop forever MUST refuse
to start the explorer path rather than risk a hang.

## 9. Dashboards (UNCHANGED — none)

Slice-04 ships NO dashboards. Same reasoning as foundation §8 + slice-03 §9:
solo dev, single user per binary, no central aggregation (per ADR-010). The
KPI-GRAPH-1 "dashboard" and KPI-GRAPH-6 "dashboard" called out in
`outcome-kpis.md` §Handoff item 2 are — for slice-04 — the `openlore stats
--explorer` CLI render against the local log, plus the `scripts/kpi-graph-1.jq`
and `scripts/kpi-graph-6.jq` fallbacks. The DEVOPS-handoff dashboard items are
satisfied by the CLI surface + the jq snippets.

When a future telemetry endpoint exists (post-slice-05, the AppView wave's
problem), the same privacy-preserving aggregable events feed cohort-level
dashboards (KPI-GRAPH-1 cohort %, KPI-GRAPH-6 cohort percentiles). The
instrumentation contract is forward-compatible.

## 10. Alerting (UNCHANGED policy; CI-time alerts)

Slice-04 ships NO continuous alerting (no on-call). The CI-time alerts shipped
(per `outcome-kpis.md` §Handoff item 3):

| Alert | Trigger | Surface | Release-blocking? |
|---|---|---|---|
| KPI-GRAPH-2 != 100% in CI | `at-scoring-aggregate-preserves-attribution` failure | GitHub Actions check | YES (guardrail; disprover) |
| KPI-GRAPH-3 != 100% in CI | `at-weight-equals-formula` failure | GitHub Actions check | YES (guardrail; disprover) |
| KPI-GRAPH-4 != 100% in CI | `at-sparse-renders-sparse` failure | GitHub Actions check | YES (guardrail; disprover) |
| KPI-GRAPH-6 P95 > 5 s for ≤200 bucket | `openlore stats --explorer` / `scripts/kpi-graph-6.jq` exceeds 5 s | developer-noticed signal | NO (informational; escalate to PO; do NOT block release alone — per `outcome-kpis.md` Handoff item 3) |

The three guardrail alerts are wired via the existing branch-protection
required-status-checks (the CI test IS the alert; no separate metric store).
The KPI-GRAPH-6 latency alert is NOT automatically wired — there is no central
metric store to alert against; in slice-04 the "alert" is the operator noticing
on their own `openlore stats --explorer` output. The `at-traverse-bounded` and
`at-explain-reproduces` CI tests run against ≤200-claim fixtures, so test
wall-clock implicitly bounds the P95 for those fixtures (a > 30 s test run is a
CI flakiness signal, not an SLO alert).

## 11. KPI-to-instrumentation mapping (cross-link)

See `kpi-instrumentation.md` delta in this dir for the per-KPI-GRAPH
traceability table.

## 12. References

- `platform-design.md` (sibling, this dir)
- `ci-cd-pipeline.md` (sibling, this dir) — new acceptance jobs that consume these events
- `kpi-instrumentation.md` (sibling, this dir)
- Foundation `observability.md` + slice-03/slice-02 `observability.md` deltas
- `docs/feature/openlore-scoring-graph/discuss/outcome-kpis.md` (KPI-GRAPH-1..6; §Handoff items 1–4)
- `docs/feature/openlore-scoring-graph/design/architecture-design.md` (§6.3 probe; §5.1 scoring core invariants)
- ADR-010 (telemetry-opt-in) — still in force
