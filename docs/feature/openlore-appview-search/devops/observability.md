# Observability Extension — openlore-appview-search (slice-05)

- **Wave**: DEVOPS
- **Date**: 2026-05-28
- **Architect**: Apex

This is the slice-05 **delta** to `observability.md` (foundation + slice-02/03/04).
The CLI's operating model (developer-as-operator; local-first; no remote sink;
author-side telemetry opt-in OFF by default; no dashboards; no continuous alerting)
is **UNCHANGED**. Slice-05 introduces the FIRST genuine network service, so it adds
ONE genuinely new observability surface absent from slices 01-04: the
**indexer-OPERATOR** surface (a long-running service the operator runs and whose
log the operator reads). All CLI-side events flow into the SAME author-side JSON
Lines pipeline (D-D2); the indexer's events flow into the INDEXER's OWN log (it is
a separate binary with a separate process; §2.6, §7). This doc adds:

1. New CLI-side `tracing` events emitted by the `search` verb (KPI-AV-1/4/6).
2. New indexer-side `tracing` events emitted by the ingest loop + query handler (KPI-AV-3 + coverage).
3. New per-event metric rows (derived on-demand from each log).
4. New probe events for the FOUR new adapters.
5. The indexer-operator observability surface + the index-coverage/freshness dashboard (KPI-AV-1 sparsity diagnosis).

## 1. Pillars

- **Logs**: YES. CLI side — the SAME author-side JSON Lines sink (D-D2; reused, no
  new endpoint). Indexer side — the indexer's OWN JSON Lines log (a SEPARATE file in
  the indexer's data dir; §2.6, §5). Both reuse the foundation `tracing` +
  `tracing-subscriber` + `tracing-appender` stack (D-D2; no new logging dependency).
- **Metrics**: YES. Same on-demand aggregation from each log (no metric store). The
  indexer adds RED-method-shaped counters/histograms for the ingest + query paths
  (§4) — the FIRST request-driven service in the product, so the FIRST place
  RED-method observability (Rate / Errors / Duration) genuinely applies.
- **Traces**: still DEFERRED (D-D3). Slice-05 adds a cross-PROCESS boundary
  (CLI→indexer), so distributed tracing becomes THEORETICALLY applicable for the
  first time — but the walking skeleton is a single CLI talking to a single
  localhost indexer; a per-`search` correlation (one request, one response) is
  in-process log enrichment on each side, NOT distributed tracing. The revisit
  trigger updates: distributed tracing becomes worth it when (a) a HOSTED
  multi-tenant indexer serves many CLIs (ADR-023 revisit) OR (b) the ingest pipeline
  fans out across multiple sources needing cross-source correlation. Until then,
  per-process `tracing` spans + a `request_id` echoed on the XRPC response suffice.

## 2. Logging — new events emitted

All events are privacy-preserving STRUCTURAL counts only. Per `outcome-kpis.md`
§Handoff item 1 + the DESIGN §10 telemetry-hooks annotation: **NO telemetry on the
CONTENTS of discovered claims** — only structural counts + identifiers the user
ALREADY SAW on screen (the object/subject/DID they searched and the results they
inspected); claim BODIES are NEVER in any event. The public-data framing (WD-105)
does NOT extend to user-behavior surveillance.

### 2.1 CLI-side `search` boundary events (mandatory — author-side)

- `search.executed` — payload `{ dimension: "object"|"contributor"|"subject", indexer_reachable: bool, result_count: u32, distinct_author_count: u32, unfollowed_author_count: u32, degraded_local_only: bool }`. Emitted once per `openlore search` invocation, on completion. `degraded_local_only=true` when the indexer was unreachable and `search` printed the local-only message (the KPI-5 graceful-degradation signal; `observability` of I-AV-3). NO subject/object VALUE in this event (only the dimension + counts).
- `search.latency_seconds` — a histogram observation emitted as a structured field on `search.executed` AND read as a histogram bucket by the on-demand aggregation — payload tags `{ dimension, indexer_reachable }`, value = wall-clock seconds from `search` dispatch to last result rendered. The KPI-AV latency source (the discovery read budget). Distinct from the indexer-side serve latency (§2.6) — this is the END-TO-END CLI-observed latency including transport.

### 2.2 Discovery north-star event (mandatory — KPI-AV-1 NORTH STAR)

- `search.discovery.unfollowed_author_hit` — payload `{ dimension: "object"|"contributor"|"subject", unfollowed_author_count: u32 }`. Emitted when a `search` result includes ≥1 author the user does NOT subscribe to (relationship `NetworkUnfollowed`) AND the user INSPECTS it (renders the result / runs `--show` on an unfollowed-author result). Per `outcome-kpis.md` §Handoff item 1, this is the load-bearing measurement for KPI-AV-1 (the north star). Emitted at most once per surfaced unfollowed-author set per `search` invocation (deduped by `dimension` within the invocation). NO `author_did` VALUE, NO `object`/`subject` VALUE — only the dimension + the count (the unfollowed-author DIDs are local relationship facts the user already saw, but the EVENT carries only the structural count, per the slice-03/04 §7 telemetry rule that a DID stream reveals the subscription graph). This is STRICTER than the slice-04 `graph.connection.surfaced` (which carried the spanning DID into the local event) — because slice-05 events touch a NETWORK-discovery surface, the DID is omitted from the event entirely.

### 2.3 Discovery→federation funnel event (mandatory — KPI-AV-4)

- `search.discovery.follow_funnel` — payload `{ time_from_search_to_add_seconds: u64, was_previously_unfollowed: bool }`. Emitted when an `openlore peer add <did>` follows a `search` that surfaced that previously-unfollowed author within a session window (the discovery→federation funnel; the affordance reuses `peer add` verbatim, WD-122/I-AV-7). Per `outcome-kpis.md` §Handoff item 1. The event LINKS a `search.discovery.unfollowed_author_hit` to a subsequent `peer add` — the join is POST-HOC (§2.7) by session window + the fact the `peer add` DID matches an unfollowed-author DID surfaced earlier in the session; the event carries only the elapsed time + the boolean, NEVER the DID (same DID-omission rule as §2.2).

### 2.4 Shareable-link events (mandatory — KPI-AV-6)

- `search.share.link_emitted` — payload `{ dimension: "object"|"contributor"|"subject" }`. Emitted when `openlore search ... --share` emits a query-encoding link (WD-122/I-AV-8). NO value (only the dimension); the link encodes the query (dimension+value), but the EVENT carries only the dimension.
- `search.share.link_opened` — payload `{ dimension: "object"|"contributor"|"subject", re_resolved_result_count: u32 }`. Emitted when a shared `openlore://search?...` link is opened (CLI re-run resolver, ADR-027). `re_resolved_result_count` is the CURRENT result count (the link re-composes current per-author-attributed verified results, never a stale snapshot; I-AV-8). NO value.

### 2.5 Public-data framing comprehension hook (KPI-AV-5)

- `search.public_data_banner_shown` — payload `{ first_session: bool }`. Emitted when the up-front public-data banner is printed before results (WD-105/I-AV-4). `first_session=true` on the user's first `search` (drives the one-shot opt-in comprehension prompt, §8). The banner itself is a render-string acceptance concern (`public_data_banner_shown` AT; DISTILL); this event is the telemetry hook for the KPI-AV-5 comprehension-prompt delivery (reusing the D-D18 one-shot file-presence survey mechanism, §8).

### 2.6 Indexer-side ingest + serve events (mandatory — KPI-AV-3 + coverage) — OPERATOR SURFACE

These events flow into the INDEXER's OWN log (`<indexer data dir>/logs/openlore-indexer.log`),
NOT the CLI's author-side log. They are operator-side service telemetry (the operator
runs the indexer and reads its log); they are NOT author-side opt-in telemetry (§7).

- `indexer.ingest.verified` — payload `{ source_pds_class: "seed"|"relay", batch_size: u32 }`. Emitted per ingested record that PASSED the verify-before-index gate (`ingest_decision` returned `Index`). `source_pds_class` is a CATEGORICAL class (NOT the PDS hostname — same categorical-error-class rule as ADR-010's PDS-hostname omission). The verified counter (KPI-AV-3 numerator-of-trust: every indexed row was verified).
- `indexer.ingest.rejected` — payload `{ reason: "bad_signature"|"cid_mismatch"|"unsigned"|"schema_unknown"|"did_unresolvable", source_pds_class }`. Emitted per record REJECTED at the verify-before-index gate (`ingest_decision` returned `Reject`, OR the pubkey decode/resolution failed → `did_unresolvable`). **The cardinal KPI-AV-3 signal**: in production this counter is >0 ONLY for genuinely-bad records; a LEGITIMATELY-signed claim being rejected (a false-positive reject) is a P0 bug. The `did_unresolvable` reason covers the ADR-026 production-decode failure path (a network author whose PLC DID-doc could not be resolved/decoded — the claim is NOT indexed; the verify-before-index gate holds by refusing to index an unverifiable record).
- `indexer.ingest.pass_completed` — payload `{ verified: u32, rejected: u32, sources_enumerated: u32, sources_failed: u32, duration_seconds: u64, ingest_lag_seconds: u64 }`. Emitted once per bounded pull pass (ADR-024). `sources_failed` is the per-source fault-isolation count (a bad source is skipped, the pass continues; ADR-024 reuses ADR-016). `ingest_lag_seconds` = now − the freshest record's `composed_at` ingested this pass — the KPI-AV-1 freshness/staleness signal (feeds the coverage dashboard, §9).
- `indexer.query.served` — payload `{ dimension, result_count: u32, distinct_author_count: u32, serve_latency_seconds: u64, request_id }`. Emitted per `org.openlore.appview.searchClaims` query served (the RED-method serve-side observation; the indexer is the first request-driven service). `request_id` is echoed on the response (the §1 in-process correlation seed). NO subject/object value.
- `indexer.query.attribution_missing` — payload `{ dimension, result_index: u32 }`. Emitted ONLY if a query result is about to be SERVED with a missing/empty `author_did` (which the type system makes impossible — non-`Option<Did>`; WD-120 layer 1). This event firing AT ALL is a P0 bug; the counter `indexer_query_attribution_missing_total` (target = 0 forever) is the runtime guardrail mirror of the `at-network-result-preserves-attribution` gate (KPI-AV-2), analogous to slice-03's `peer_render_attribution_missing_total` + slice-04's `scoring_attribution_missing_total`.

### 2.7 KPI-AV-1 / KPI-AV-4 / KPI-AV-6 measurement mechanism (mandatory)

Mirrors the slice-03 D-D16 / slice-04 D-D32 POST-HOC default: the KPI-AV outcome
metrics are computed POST-HOC by `jq` aggregation over the events above — NO state
file in either binary.

- **KPI-AV-1** (% of discovery sessions surfacing ≥1 unfollowed-author hit): count
  distinct SESSIONS (a `search`-invocation window) that emitted ≥1
  `search.discovery.unfollowed_author_hit`, divided by total discovery sessions
  (sessions that emitted ≥1 `search.executed` with `indexer_reachable=true`). The
  `jq` snippet `scripts/kpi-av-1.jq` does the join.
- **KPI-AV-4** (discovery→follow funnel conversion): count discovery sessions that
  emitted a `search.discovery.follow_funnel` (a `peer add` of a previously-unfollowed
  surfaced author), divided by total discovery sessions. `scripts/kpi-av-4.jq`.
- **KPI-AV-6** (shared-link usage): count of `search.share.link_emitted` (and, if
  observable, `link_opened`) per cohort window. `scripts/kpi-av-6.jq`.

No `flow_id` / state file is persisted (the events carry the timestamps + the
session windowing; post-hoc aggregation is exact). This matches the D-D16 / D-D27 /
D-D32 no-state-file reasoning.

### 2.8 Probe extension events (mandatory) — the FOUR new adapters

Per foundation §3.2 ("Every `probe()` invocation emits a result event") + DESIGN
§6.3. Each new adapter's probe emits a result event; on refusal,
`health.startup.refused{reason}` + exit 2 (the indexer is FAIL-FAST at startup,
ADR-009/023; the CLI's index-query probe is SOFT — an unreachable indexer must NOT
refuse CLI startup, the inverted-probe case, ADR-027).

- `probe.index_store` — payload `{ adapter: "adapter-index-store", outcome: ok|refused, fsync_honored: bool, attribution_round_trip_ok: bool, no_merge_schema_ok: bool, reason, detail }`. On a durability lie: `health.startup.refused{reason: storage.fsync_unhonored}` (ADR-025; the container-substrate-lie check — the load-bearing slice-05 indexer probe). On an attribution-round-trip failure (two distinct-author rows on the same (subject,object) not read back byte-equal): `health.startup.refused{reason: index.attribution_round_trip_failed}`. On finding a `consensus`/`merged` table: `health.startup.refused{reason: index.merge_schema_present}`.
- `probe.ingest_source` — payload `{ adapter: "adapter-atproto-ingest", outcome: ok|refused, enumeration_shape_ok: bool, rejects_tampered: bool, rejects_cid_mismatch: bool, reason, detail }`. **The network-lies check** (ADR-024): the probe asserts a fixture source's tampered-signature + CID-mismatch records are REJECTED by the ingest path before the index. On failure (a tampered record would be indexed): `health.startup.refused{reason: indexer.ingest_gate_unproven}`. On the required source being unreachable: `health.startup.refused{reason: indexer.ingest_source_unreachable}`.
- `probe.identity_resolve` — payload `{ adapter: "adapter-atproto-did", outcome: ok|refused, real_decode_path: bool, verifies_good_sig: bool, rejects_tampered_sig: bool, reason, detail }`. **The network-lies-about-a-key check** (ADR-026): the probe decodes a real `z6Mk...` fixture DID-doc, asserts it VERIFIES a known-good signature AND REJECTS a tampered one, AND asserts the REAL decode path ran (`real_decode_path=true`) — a probe passing only against the slice-03 test seam is a CI failure. On decode failure: `health.startup.refused{reason: identity.pubkey_decode_failed}`.
- `probe.xrpc_query_server` — payload `{ adapter: "adapter-xrpc-query-server", outcome: ok|refused, binds_ok: bool, response_carries_author_did: bool, reason, detail }`. On a response shape that dropped per-result `author_did`: `health.startup.refused{reason: indexer.transport_drops_attribution}` (the anti-merging-across-the-transport check, I-AV-2).
- `probe.index_query` (CLI side) — payload `{ adapter: "adapter-index-query", outcome: ok|soft_unreachable, response_carries_author_did: bool, unreachable_is_soft: bool, reason, detail }`. **The inverted/degradation check** (ADR-027): against a reachable fixture indexer the probe confirms the response carries `author_did`; against an UNREACHABLE indexer it asserts `IndexQueryError::Unreachable` is a SOFT, NON-FATAL outcome — NOT a startup refusal (the CLI MUST start without a reachable indexer; KPI-5). A CLI that hard-refused on an unreachable indexer is the KPI-5 regression the probe catches.

### 2.9 The capability-boundary probe (mandatory — I-AV-5 / ADR-023)

- `indexer.startup.capability_boundary` — payload `{ index_store_is_index_duckdb: bool, identity_is_resolve_only: bool, outcome: ok|refused }`. Emitted by the `openlore-indexer` composition-root `capability_boundary_probe` BEFORE any ingest/serve. On a violation (a signing identity or the user's `openlore.duckdb` was wired): `health.startup.refused{reason: indexer.capability_boundary_violated}` + exit 2 (ADR-023 Earned Trust behavioral layer; mirrors the slice-02 I-SCR-1 human-gate).

### 2.10 What did NOT change

All foundation, slice-02, slice-03, slice-04 events remain emitted unchanged
(verb.invoked, port.call/return, compose.*, sign.success, publish.*, query.executed,
health.*, peer.pull.*, peer.claim.rendered, claim.counter.*, scrape.*,
claim.signed.from_scraper, graph.query.executed, graph.traverse.*,
graph.connection.surfaced, graph.score.*). The new code paths emit ADDITIONAL events;
they suppress or rename none. The CLI's `search.*` events are author-side (D-D2 log);
the indexer's `indexer.*` events are operator-side (the indexer's own log) — two
separate streams from two separate binaries.

## 3. Probes (extension) — the FOUR new adapters + the capability boundary

Foundation `observability.md` §7.2 lists per-adapter probe responsibilities.
Slice-05 adds FIVE probe surfaces (the four new adapters + the composition-root
capability boundary). All within the 250ms budget (ADR-009 I-5).

| Adapter / surface | New probe responsibility for slice-05 |
|---|---|
| `adapter-index-store` (`IndexStorePort`) | (a) schema-version + **fsync honored on the container substrate** (overlayfs/DrvFs/tmpfs lie → refuse `storage.fsync_unhonored`); (b) attribution round-trip (two rows, same (subject,object), distinct non-empty `author_did`s read back byte-equal — the anti-merging substrate check); (c) no-merge-schema assertion (NO `consensus`/`merged` table). (ADR-025) |
| `adapter-atproto-ingest` (`IngestSourcePort`) | (a) source reachability + enumeration shape against a fixture source; (b) **the network-lies check** — a fixture source's tampered-signature + CID-mismatch records are REJECTED before the index (the verify-before-index gate proven, not trusted). (ADR-024) |
| `adapter-atproto-did` (`IdentityResolvePort`, extended) | (a) resolve a FIXTURE DID-doc with a real `z6Mk...` value, decode it, assert it VERIFIES a good signature AND REJECTS a tampered one; (b) the gold test runs the REAL decode path (a seam-only pass is a CI failure). (ADR-026) |
| `adapter-xrpc-query-server` (HTTP query surface) | (a) bind + serve a fixture query; (b) the response carries per-result `author_did` (anti-merging across the transport; a response dropping it is a contract violation caught at probe time). (ADR-027) |
| `adapter-index-query` (`IndexQueryPort`, CLI side) | (a) a reachable fixture indexer returns the expected XRPC shape with `author_did` present; (b) **the inverted/degradation check** — an UNREACHABLE indexer yields a SOFT `Unreachable`, NOT a startup refusal (the CLI MUST start without a reachable indexer; KPI-5). (ADR-027) |
| `openlore-indexer` composition root | the `capability_boundary_probe`: asserts the store is `index.duckdb` (NOT `openlore.duckdb`) + the identity adapter is resolve-only; refuses on violation (`indexer.capability_boundary_violated`). (ADR-023) |

`check-probes` (the foundation AST walker over every `impl <Port> for <Adapter>`)
picks up the four new adapters' probes by construction — no CI change beyond the
new trait set (`ci-cd-pipeline.md` delta §2). `appview-domain` has NO `probe()` (a
pure crate touches no substrate); `check-probes` correctly does not require one.

## 4. Metrics — new rows for `openlore stats` (CLI) + `openlore-indexer stats` (indexer)

### 4.1 CLI-side rows (append to foundation §4.3 + slice-02/03/04 additions)

| Metric | Source events | Render shape | Used by |
|---|---|---|---|
| `search_total{dimension, indexer_reachable}` | count of `search.executed` | counter, per-dimension breakdown | operational visibility; KPI-AV usage context |
| `search_latency_seconds{dimension, indexer_reachable}` | `search.latency_seconds` field | histogram (p50/p95) | KPI-AV latency (the end-to-end discovery read budget) |
| `search_degraded_local_only_total` | count of `search.executed` where `degraded_local_only=true` | counter | KPI-5 graceful-degradation sanity (the I-AV-3 signal — how often the indexer was unreachable) |
| `search_discovery_unfollowed_hit_total` | count of `search.discovery.unfollowed_author_hit` | counter, per-dimension | KPI-AV-1 (the north-star measurement) |
| `search_discovery_follow_funnel_total` | count of `search.discovery.follow_funnel` | counter | KPI-AV-4 (the funnel conversion) |
| `search_share_link_emitted_total` / `_opened_total` | count of `search.share.link_*` | counter | KPI-AV-6 |

### 4.2 Indexer-side rows (NEW — the operator surface; `openlore-indexer stats` or jq-fallback)

| Metric | Source events | Render shape | Used by |
|---|---|---|---|
| `indexer_ingest_verified_total` | count of `indexer.ingest.verified` | counter | KPI-AV-3 (the verified count) + coverage (claims indexed) |
| `indexer_ingest_rejected_total{reason}` | count of `indexer.ingest.rejected` per reason | counter, per reason | KPI-AV-3 (the rejection-by-reason breakdown; the `did_unresolvable` + `bad_signature` + `cid_mismatch` counts) |
| `indexer_ingest_lag_seconds` | `indexer.ingest.pass_completed.ingest_lag_seconds` | gauge (last pass) + histogram | KPI-AV-1 freshness/staleness (the coverage-dashboard freshness signal) |
| `indexer_distinct_authors_indexed` | DISTINCT `author_did` count over `indexed_claims` (queried, not event-derived) | gauge | KPI-AV-1 coverage (the sparsity diagnosis — "is the index too biased toward already-followed authors?") |
| `indexer_claims_indexed_total` | `COUNT(*)` over `indexed_claims` (queried) | gauge | KPI-AV-1 coverage (claims indexed) |
| `indexer_query_served_total{dimension}` | count of `indexer.query.served` | counter (RED: Rate) | operational visibility |
| `indexer_query_serve_latency_seconds{dimension}` | `indexer.query.served.serve_latency_seconds` | histogram (RED: Duration) | the indexer serve budget (distinct from the CLI end-to-end latency) |
| `indexer_source_failures_total` | sum of `indexer.ingest.pass_completed.sources_failed` | counter (RED: Errors, ingest side) | ingest health (per-source fault isolation; a non-zero is normal on a flaky source) |
| `indexer_query_attribution_missing_total` | count of `indexer.query.attribution_missing` | counter; target = 0 forever | KPI-AV-2 (runtime guardrail; non-zero is a P0 bug — mirror of slice-03/04 attribution-missing counters) |

### 4.3 `openlore stats` / `openlore-indexer stats` rendering additions

Append to the foundation §4.2 commands (+ slice-02 `--scraper`, slice-03
`--federation`, slice-04 `--explorer`):

| Command | Renders |
|---|---|
| `openlore stats --discovery` (CLI; assumes D-D5 verb landed) | Summary card: total searches (by dimension), indexer-reachable rate, unfollowed-author hits, follow-funnel conversions, shared-links emitted/opened, search latency p50/p95, degraded-local-only count. |
| `openlore-indexer stats` (the indexer's OWN stats, over its OWN log + index) | Summary card: claims indexed, distinct authors indexed, ingest verified/rejected (by reason), ingest lag (last pass + p95), queries served + serve latency p50/p95, source failures, and the guardrail counter (query-attribution-missing: 0). This is the OPERATOR's index-coverage/freshness dashboard (§9). |

If D-D5 (the `openlore stats` verb) is deferred and the `scripts/kpi-av-*.jq`
snippets are the fallback, DELIVER ships (mirroring the slice-03 `kpi-fed-*.jq` +
slice-04 `kpi-graph-*.jq` snippets):

- `scripts/kpi-av-1.jq` — % of discovery sessions with ≥1 `search.discovery.unfollowed_author_hit` (the north-star post-hoc aggregation, §2.7).
- `scripts/kpi-av-3.jq` — over the INDEXER log: asserts `indexer_query_attribution_missing_total == 0` AND breaks down `indexer.ingest.rejected` by reason AND asserts no `indexer.ingest.verified` followed an `ingest_decision` Reject (the runtime verified-before-index sanity).
- `scripts/kpi-av-4.jq` — discovery→follow funnel conversion (§2.7).
- `scripts/kpi-av-6.jq` — shared-link emitted/opened counts (§2.7).
- `scripts/indexer-coverage.jq` — over the INDEXER log + a `SELECT COUNT(*), COUNT(DISTINCT author_did)`: claims indexed, distinct authors indexed, ingest lag p95 (the coverage/freshness dashboard, §9).

## 5. Where logs go

- **CLI side (UNCHANGED)**: `$XDG_DATA_HOME/openlore/logs/openlore.log` — the new
  `search.*` events append to the same author-side JSON Lines stream (D-D2; reused,
  no new endpoint).
- **Indexer side (NEW)**: `<indexer data dir>/logs/openlore-indexer.log` (e.g.
  `~/.local/share/openlore-indexer/logs/openlore-indexer.log`) — the `indexer.*`
  events. Same `tracing-appender` JSON Lines stack (D-D2), same rotation policy. A
  SEPARATE file because it is a SEPARATE binary/process with a SEPARATE lifecycle.
  The two logs are never commingled (the binaries are config- and data-disjoint,
  ADR-023).

## 6. Verbosity controls

Foundation §3.5 table carries forward. The new events emit at:

- INFO: `search.executed`, `search.discovery.unfollowed_author_hit`,
  `search.discovery.follow_funnel`, `search.share.link_*`,
  `search.public_data_banner_shown`; `indexer.ingest.verified` (sampled/aggregated to
  avoid log spam on large passes — emit a per-record DEBUG + the per-pass INFO
  `indexer.ingest.pass_completed`), `indexer.query.served`.
- DEBUG: per-record ingest detail; per-result query detail (if DELIVER chooses).
- WARN: `indexer.ingest.rejected` (a rejected record is a security-relevant ingest
  event — surfaces in default stderr, matching slice-03 `peer.pull.rejected`),
  `indexer.query.attribution_missing` and `indexer.startup.capability_boundary`
  refusal (guardrail-violation events that should be impossible — WARN ensures the
  operator sees them immediately rather than them being silently swallowed).

## 7. Telemetry — the CLI author-side (UNCHANGED policy) + the indexer-operator surface (NEW, distinct)

This is the load-bearing slice-05 observability distinction.

### 7.1 CLI author-side telemetry (UNCHANGED — ADR-010 / D-D4)

Telemetry remains opt-in, OFF by default, no endpoint operated. The CLI `search.*`
events are designed to be privacy-preserving STRUCTURAL counts (per `outcome-kpis.md`
§Handoff item 1 — NO claim contents, ever; STRICTER than slice-04: NO `author_did`
and NO subject/object VALUE in any `search.*` event). If/when the future endpoint
exists, the slice-05 CLI events eligible for telemetry rollup are:

- `search_total{dimension, indexer_reachable}` (counter only; the dimension/flag tags).
- `search_latency_seconds` histogram (bucketed).
- `search_discovery_unfollowed_hit_total` (counter only; NO dimension VALUE, NO DID).
- `search_discovery_follow_funnel_total` (counter only; NO DID).
- `search_share_link_emitted_total` / `_opened_total` (counters only).
- `search_degraded_local_only_total` (counter only; the KPI-5 sanity).

EXPLICITLY NEVER sent over CLI telemetry (even when opted in): any `author_did`
value; any subject/object/predicate VALUE; any claim content (the `search.*` events
carry NONE by design); the indexer URL/hostname (only the reachable boolean). Same
reasoning as slice-03's peer_did rule + slice-04's connection-target rule.

### 7.2 Indexer-OPERATOR observability (NEW — NOT author-side telemetry)

The indexer's `indexer.*` events are NOT covered by ADR-010's author-side opt-in
policy — they are SERVICE-OPERATOR logs (the person who runs `openlore-indexer serve`
reads their own service's log to operate it; the walking-skeleton indexer is a single
self-hosted dogfood instance, ADR-023). This is the FIRST operator-side surface in
the product (slices 01-04 were single-user CLIs with no service to operate).

- The indexer log is LOCAL to the operator (the indexer's data dir; §5). It is NOT
  exfiltrated; the indexer operates NO telemetry endpoint (the same no-endpoint
  posture as ADR-010, applied to the operator surface).
- The indexer log contains operational identifiers the OPERATOR needs to operate the
  service (categorical source classes, reject reasons, ingest lag, query counts) but
  NEVER claim CONTENTS and NEVER per-user-search behavior (the indexer is per-user-
  neutral; it does not know WHO searched — the CLI is the only place a user identity
  exists, and the CLI's `search.*` events stay author-side).
- A FUTURE hosted/multi-tenant indexer (ADR-023 revisit) would need a DEVOPS ADR for
  operator telemetry, auth, and rate-limiting — OUT of scope for the walking skeleton
  (§9 + `platform-design.md` §9 caveat). Recorded as the forward note.

**Upstream clarification (per `platform-design.md` §11 Upstream Issue 2)**: slice-04's
handoff expected slice-05 to "stand up the telemetry endpoint". Slice-05 does NOT —
the indexer-operator surface is a SERVICE LOG, not the author-side cohort telemetry
endpoint. The cohort-aggregation YELLOWs (KPI-3/6, KPI-FED-3/5, KPI-SCR-1/5,
KPI-GRAPH-1/5/6 cohort, and now the KPI-AV-1/4/6 cohort) REMAIN deferred to a future
endpoint.

## 8. Public-data comprehension prompt (KPI-AV-5) — one-shot, reusing D-D18

The KPI-AV-5 comprehension prompt ("what data does discovery index?") is delivered
as a one-shot, dismissible, opt-in CLI prompt after the FIRST `search` (the
`search.public_data_banner_shown{first_session: true}` hook, §2.5), reusing the
foundation/slice-03 D-D18 one-shot file-presence survey mechanism. Response stored at
`$XDG_DATA_HOME/openlore/surveys/post-search-comprehension.response.json`. Same
dismiss-on-Enter + `--no-tty` skip semantics; free-text optional, never telemetry-sent
(D-D38). The up-front public-data banner (every search) is the always-on framing; the
one-shot prompt is the comprehension measurement.

## 9. Index-coverage / freshness dashboard (the KPI-AV-1 sparsity diagnosis)

The load-bearing slice-05 dashboard, handed to DEVOPS per `outcome-kpis.md` §Handoff
item 2 + DISCUSS Risks (the "is the index too sparse to discover anything?"
question that gates the KPI-AV-1 north star). For the walking skeleton it is the
`openlore-indexer stats` CLI render over the indexer's OWN log + a coverage query,
plus the `scripts/indexer-coverage.jq` fallback — NOT a hosted dashboard (no central
aggregation; ADR-010 / ADR-023 single self-hosted instance). It surfaces:

- **Claims indexed** (`indexer_claims_indexed_total`) + **distinct authors indexed**
  (`indexer_distinct_authors_indexed`) — the coverage breadth. A high claim count but
  low distinct-author count is the "biased toward already-followed authors" sparsity
  signature (the KPI-AV-1 < 20% disprover diagnosis).
- **Ingest lag** (`indexer_ingest_lag_seconds`, last pass + p95) — the freshness
  window (how stale the index is vs the network). Informational alert if it exceeds a
  DELIVER-tuned freshness budget (Q-DELIVER-AV-4; `outcome-kpis.md` §Handoff item 3).
- **Verified vs rejected ratio** (`indexer_ingest_verified_total` vs
  `indexer_ingest_rejected_total{reason}`) — the KPI-AV-3 health (the rejected-for-
  real-claims count must be 0; the `did_unresolvable` count flags PLC-resolution
  trouble against real network authors).

This dashboard is the operator's tool to DIAGNOSE a low KPI-AV-1 (is it a coverage
problem or a UX problem?) — the disprover-triage instrument DISCUSS asked for.

## 10. Alerting (CLI: UNCHANGED policy; indexer: operator-noticed; CI-time: release-blocking)

Slice-05 ships NO continuous paging (no on-call; the indexer is a single self-hosted
dogfood instance). The alerts shipped (per `outcome-kpis.md` §Handoff item 3):

| Alert | Trigger | Surface | Release-blocking? |
|---|---|---|---|
| KPI-AV-2 != 100% in CI | `at-network-result-preserves-attribution` failure | GitHub Actions check | YES (cardinal guardrail; disprover) |
| KPI-AV-3 != 100% in CI | `at-indexer-rejects-unverified-claim` failure (a tampered/CID-mismatch/unsigned fixture indexed, OR a legitimately-signed fixture rejected) | GitHub Actions check | YES (cardinal guardrail; disprover) |
| KPI-5 regression in CI | `at-local-first-preserved` failure (offline compose/sign breaks, or `search` hard-fails with the indexer down) | GitHub Actions check | YES (cardinal guardrail; disprover) |
| `indexer_query_attribution_missing_total` > 0 (runtime) | the runtime KPI-AV-2 guardrail counter | `openlore-indexer stats` / `scripts/kpi-av-3.jq` | P0 bug signal (not a release gate — should be type-impossible; the CI gate is the release block) |
| `did_unresolvable` reject rate elevated (runtime) | `indexer.ingest.rejected{reason: did_unresolvable}` spiking | `openlore-indexer stats` | NO (operator-noticed; signals PLC-resolution trouble vs real network authors — investigate the PLC endpoint, not a release block) |
| KPI-AV-1 < 30% at day-30 | post-hoc `scripts/kpi-av-1.jq` / PO outreach | developer/PO-noticed signal | NO (informational; escalate to PO; the < 20% disprover triggers a coverage/UX re-investigation per `outcome-kpis.md` §Handoff item 3) |
| Index ingest lag > freshness budget | `indexer_ingest_lag_seconds` exceeds the DELIVER-tuned budget | `openlore-indexer stats` / `scripts/indexer-coverage.jq` | NO (informational; the coverage-dashboard freshness signal) |

The three cardinal CI alerts (KPI-AV-2/3 + KPI-5) are wired via the existing
branch-protection required-status-checks (the CI test IS the alert; no separate
metric store). The runtime/operator alerts are NOT automatically wired — there is no
central metric store; the "alert" is the operator noticing on their own
`openlore-indexer stats` output. This mirrors the slice-04 §10 alerting posture,
extended with the operator-surface signals.

## 11. Dashboards (CLI: UNCHANGED — none; indexer: the operator coverage/freshness render)

The CLI ships NO dashboards (foundation §8; solo dev, single user per binary). The
indexer's "dashboard" is the `openlore-indexer stats` CLI render + the
`scripts/indexer-coverage.jq` fallback (§9) — the index-coverage/freshness diagnosis,
read by the single self-hosted operator against the local indexer log. When a future
HOSTED indexer exists (ADR-023 revisit), the same privacy-preserving aggregable
operator events feed a real dashboard; the instrumentation contract is
forward-compatible. The KPI-AV-1/4/6 "dashboards" called out in `outcome-kpis.md`
§Handoff item 2 are — for slice-05 — the `openlore stats --discovery` CLI render +
the `scripts/kpi-av-*.jq` fallbacks (per-user; cohort deferred to the future endpoint).

## 12. KPI-to-instrumentation mapping (cross-link)

See `kpi-instrumentation.md` delta in this dir for the per-KPI-AV traceability table.

## 13. References

- `platform-design.md` (sibling, this dir) — gate-inventory delta, the deployable, the environment matrix, the `deny.toml` change
- `ci-cd-pipeline.md` (sibling, this dir) — new acceptance + contract jobs that consume these events
- `kpi-instrumentation.md` (sibling, this dir) — KPI-AV gate mapping
- `contract-test-ownership.md` (sibling, this dir) — the two boundaries the contract tests pin
- Foundation `observability.md` + slice-02/03/04 `observability.md` deltas
- `docs/feature/openlore-appview-search/discuss/outcome-kpis.md` (KPI-AV-1..6; §Handoff items 1-4)
- `docs/feature/openlore-appview-search/design/architecture-design.md` (§6.3 probes; §10 Earned Trust telemetry hooks)
- `docs/feature/openlore-appview-search/design/component-boundaries.md` (the DEVOPS annotation — the event names + probe failure reasons)
- ADR-010 (telemetry-opt-in — CLI author-side; the indexer-operator surface is distinct), ADR-023 (the self-hosted indexer), ADR-024/025/026/027 — in force
