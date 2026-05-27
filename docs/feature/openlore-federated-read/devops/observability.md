# Observability Extension — openlore-federated-read (slice-03)

- **Wave**: DEVOPS
- **Date**: 2026-05-27
- **Architect**: Apex

This is the slice-03 **delta** to `observability.md` (foundation). Operating
model (developer-as-operator; local-first; no remote sink; telemetry
opt-in OFF by default; no dashboards; no alerting) is **UNCHANGED**. This
doc adds:

1. New `tracing` event names emitted by new code paths.
2. New per-event metric rows.
3. New probe events for peer-pull paths.
4. New `openlore stats` rendering rows (assumes D-D5 verb landed; otherwise
   the same fallback applies — read events directly via `jq`).

## 1. Pillars (UNCHANGED)

Logs YES (same JSON Lines sink). Metrics YES (same on-demand aggregation
from log). Traces still DEFERRED with same revisit trigger (slice-03 is
still a single-binary CLI; peer-pull is a single HTTP request from the
user's machine — nothing distributed). The `tracing` span per peer-pull
operation is in-process scoping for log enrichment, NOT distributed
tracing.

## 2. Logging — new events emitted

### 2.1 Peer-pull boundary events (mandatory)

- `peer.pull.started` — payload `{ peer_did, expected_record_count: Option<u32>, cursor_in: Option<String> }`. Emitted when `peer pull <did>` begins network work after probe.
- `peer.pull.record.received` — payload `{ peer_did, record_cid_received: String, record_size_bytes: u64 }`. One event per record fetched, BEFORE verification.
- `peer.pull.record.verified` — payload `{ peer_did, record_cid: String, verify_outcome: ok|sig_failed|cid_failed|both_failed, sig_latency_us: u64, cid_recompute_latency_us: u64 }`. One event per record verified. The `verify_outcome` field is the load-bearing per-record assertion; it MUST be exactly one of the listed variants.
- `peer.pull.record.stored` — payload `{ peer_did, record_cid: String, table: "peer_claims" }`. Emitted ONLY for records that passed verification and were inserted.
- `peer.pull.rejected` — payload `{ peer_did, record_cid: String, reason: SigInvalid|CidMismatch|LexiconValidationFailed|DuplicateCid, detail: String }`. Emitted instead of `peer.pull.record.stored` for records that did not pass.
- `peer.pull.completed` — payload `{ peer_did, total_received: u32, total_stored: u32, total_rejected: u32, duration_ms: u64, cursor_out: Option<String> }`. One event per `peer pull` invocation.

Invariant: for every `peer.pull.record.received` there is exactly ONE matching
`peer.pull.record.verified` AND exactly ONE matching `peer.pull.record.stored`
or `peer.pull.rejected`. This invariant is testable via a `jq` script (see
§5.6 of `kpi-instrumentation.md` delta).

### 2.2 Peer-render boundary events (mandatory)

- `peer.claim.rendered` — payload `{ peer_did, claim_cid, has_author_did_field: bool, render_surface: "graph_query_federated"|"peer_pull_summary" }`. Emitted EVERY TIME a peer claim is rendered to ANY user-visible surface. The `has_author_did_field` MUST be `true` for every emission; the field is logged so CI tests can assert the invariant rather than only relying on adversarial review.

### 2.3 Counter-claim authoring events (mandatory)

- `claim.counter.started` — payload `{ target_cid, target_author_did, reason_len: usize }`. Emitted when `claim counter` begins composition.
- `claim.counter.composed` — payload `{ target_cid, target_author_did, reason_len: usize, contains_not_as_truth: bool }`. Emitted before signing. The `contains_not_as_truth` MUST be `true` (per the foundation US-001 AC "not as truth" framing rule extended to counter-claims per `feature-delta.md` Locks Inherited row 9); CI-watched invariant.
- `claim.counter.published` — payload `{ counter_cid, target_cid, target_author_did, reason_len: usize }`. Emitted on successful publish via the same `VerbClaimPublish` internals (WD-22). This event is the load-bearing measurement for KPI-FED-3 (north star).

### 2.4 Subscription lifecycle events (mandatory)

- `peer.added` — payload `{ peer_did, source: "cli"|"initial-config" }`. Emitted on `peer add`.
- `peer.removed` — payload `{ peer_did, purge: bool, residue_rows_before_purge: u32 }`. Emitted on `peer remove`; if `--purge`, `residue_rows_before_purge` records how many `peer_claims` rows existed for the DID at the moment of removal (so post-purge tests can assert 0 and operators have a delta).
- `peer.removed.purge_complete` — payload `{ peer_did, residue_rows_after_purge: u32 }`. Emitted AFTER the purge step; `residue_rows_after_purge` MUST be 0 (CI-watched invariant; KPI-FED-4).

### 2.5 Federation E2E timing event (mandatory for KPI-FED-5)

- `federation.e2e.timing` — payload `{ flow_id: String, peer_did, peer_claim_count_bucket: "le5"|"6to20"|"21to100"|"gt100", t_peer_add_ms: u64, t_peer_pull_ms: u64, t_first_query_render_ms: u64, total_ms: u64 }`. Emitted by the first `graph query --federated` invocation AFTER a `peer pull` that itself followed a `peer add` within the same `flow_id` (the flow_id is a session-spanning UUID stored in `$XDG_DATA_HOME/openlore/state/federation-flow.json` between invocations; cleared when total_ms is emitted). This event is the KPI-FED-5 source.
  - **Open question to DESIGN**: whether to persist the flow_id state file. The alternative is "compute KPI-FED-5 post-hoc from the log by joining `peer.added`, `peer.pull.completed`, `query.executed{kind: federated}` events grouped by peer_did". Both are viable; persistence is simpler for the user (one timing event, exact value); post-hoc keeps state out of the binary. Default: **post-hoc** (no state file; the `jq` aggregation in §3.3 of `kpi-instrumentation.md` delta does the join). DESIGN may override.

### 2.6 Probe extension events (mandatory)

- `probe.peer_pull_path` — payload `{ adapter: "atproto-pds"|"peer-port", outcome: ok|refused, reason, detail }`. Emitted by the EXTENSION to the existing probe of whichever adapter ends up owning peer-read. The probe-result event name pattern matches foundation §3.2 ("Every `probe()` invocation").

### 2.7 What did NOT change

All foundation events (verb.invoked, port.call, port.return, compose.*,
sign.success, publish.*, retract.composed, query.executed, health.*) remain
emitted unchanged. The new code paths emit ADDITIONAL events; they do not
suppress or rename any existing ones.

## 3. Probes (extension)

Foundation `observability.md` §7.2 lists per-adapter probe responsibilities.
Slice-03 extends them; the table additions:

| Adapter | New probe responsibility for slice-03 |
|---|---|
| `adapter-atproto-pds` (or new `peer-port` adapter, DESIGN's call) | `com.atproto.repo.listRecords` against the user's OWN DID succeeds and returns a parseable response (proves the read XRPC works against the user's own PDS as a self-test; does NOT touch any peer at probe time). On failure: `health.startup.refused{ reason: PdsListRecordsUnavailable, detail: <XRPC error> }`. |
| `adapter-duckdb` | `peer_subscriptions` and `peer_claims` tables exist at the expected schema version (forward-compat with foundation `schema_version` table). On mismatch: existing `StorageSchemaMismatch` refusal (foundation §7.2 row 1) covers this — no new refusal reason. |
| `lexicon` module | `org.openlore.claim` Lexicon's `reason` field validates against the schema-of-schemas as OPTIONAL (per WD-23 forward-compat invariant). On failure (someone made it required): existing `LexiconInvalid` refusal covers it. |

Slice-03 does NOT introduce per-peer-DID probes at startup. Resolving every
subscribed peer's DID at startup would (a) couple binary startup time to
the number of peers, (b) fail-fast for transient resolver outages making
the binary unusable, (c) burn DID-resolver request budget. The user's DID
resolves at startup (proves the resolver works); per-peer DID resolution
is deferred to first `peer pull` of that peer. Documented as DEVOPS choice
in `wave-decisions.md` delta (D-D14).

## 4. Metrics — new rows for `openlore stats`

Append to foundation §4.3 table:

| Metric | Source events | Render shape | Used by |
|---|---|---|---|
| `peer_subscriptions_active` | `peer.added` minus `peer.removed` (count, where `peer.removed.purge_complete` strengthens to "purged") | gauge (current state) | KPI-FED-5 (subscription longevity context) |
| `peer_pulls_total` | count of `peer.pull.completed` | counter, per-peer breakdown available | operational visibility |
| `peer_pull_latency_seconds` | `peer.pull.completed.duration_ms` | histogram (p50/p95/p99) | KPI-FED-1 surface (per-pull latency) |
| `peer_records_rejected_total{reason}` | count of `peer.pull.rejected` grouped by `reason` field | counter, per-reason; per-peer-DID drilldown available | KPI-FED-6 (this counter MUST be reachable via `openlore stats --rejections`; non-zero is investigative, not unshippable — rejection is the system working correctly when an adversarial record is encountered) |
| `peer_records_stored_total` | count of `peer.pull.record.stored` | counter | sanity / context |
| `peer_render_attribution_missing_total` | count of `peer.claim.rendered` events where `has_author_did_field == false` | counter; target = 0 forever | KPI-FED-1, KPI-FED-2 (runtime guardrail counter — analogous to foundation `field_mismatch_total` for KPI-4); non-zero is a P0 bug |
| `counter_claims_published_total` | count of `claim.counter.published` | counter, can break down by `target_author_did` | KPI-FED-3 (the north-star measurement) |
| `peer_removes_total{purge: bool}` | count of `peer.removed` grouped by `purge` field | counter | KPI-FED-5 context (subscription churn) + KPI-FED-4 context (purge frequency) |
| `peer_purge_residue_after_purge_total` | sum of `peer.removed.purge_complete.residue_rows_after_purge` | counter; target = 0 forever | KPI-FED-4 (runtime guardrail; non-zero is a P0 bug) |
| `federation_e2e_seconds` | `federation.e2e.timing.total_ms` (or post-hoc jq aggregation per §2.5) | histogram (p50/p95/p99), per peer-cardinality bucket | KPI-FED-5 |

### 4.1 `openlore stats` rendering additions

Append to foundation §4.2 commands:

| Command | Renders |
|---|---|
| `openlore stats --federation` | Summary card: active subscriptions, total peer pulls, total records stored, total rejected (with reason breakdown), counter-claims published, federation e2e p50/p95. |
| `openlore stats --federation --since <date>` | Same, filtered. |
| `openlore stats --rejections` | Per-`peer.pull.rejected` event detail (peer_did, record_cid, reason, detail, timestamp). For investigating "is this peer adversarial?". |
| `openlore stats --json` | Already exists; now includes the new metric rows. |

If D-D5 (the `openlore stats` verb) deferred and the `scripts/kpi-*.jq`
snippets are the fallback, DELIVER ships:

- `scripts/kpi-fed-1.jq` — attribution-fidelity check (asserts all `peer.claim.rendered` have `has_author_did_field == true`).
- `scripts/kpi-fed-3.jq` — count of `claim.counter.published` per 30-day window.
- `scripts/kpi-fed-5.jq` — post-hoc e2e timing aggregation per §2.5 (post-hoc default).
- `scripts/kpi-fed-6.jq` — count of `peer.pull.rejected` events, grouped by reason; sanity-check that the count is non-zero in tests that exercise the adversarial fixture (else the test is silently passing for the wrong reason).

## 5. Where logs go (UNCHANGED)

Same file: `$XDG_DATA_HOME/openlore/logs/openlore.log`. Same rotation
policy. Same stderr verbose mode. The new event names append to the same
JSON Lines stream.

## 6. Verbosity controls (UNCHANGED)

Foundation §3.5 table carries forward unchanged. The new events emit at:

- INFO: `peer.added`, `peer.removed`, `peer.removed.purge_complete`, `peer.pull.started`, `peer.pull.completed`, `claim.counter.started`, `claim.counter.composed`, `claim.counter.published`, `federation.e2e.timing`.
- DEBUG: `peer.pull.record.received`, `peer.pull.record.verified`, `peer.pull.record.stored`, `peer.claim.rendered`.
- WARN: `peer.pull.rejected` (rejection of a peer record is a security-relevant event; INFO is too quiet, ERROR is too loud — WARN is the correct level so it surfaces in default stderr the user sees, signalling "your peer published something I rejected" without being treated as a bug).

The WARN level for `peer.pull.rejected` matches foundation's `port.error`
WARN level — port-boundary rejections are operator-visible by default.

## 7. Telemetry (UNCHANGED policy)

Per ADR-010: telemetry remains opt-in, OFF by default, no endpoint
operated. If/when the future endpoint exists, the slice-03 events that
would be eligible for telemetry rollup are:

- `peer_pulls_total` (counter only; no peer_did sent — privacy-by-default).
- `peer_records_rejected_total{reason}` (counter only, grouped by reason class; no peer_did, no record_cid).
- `counter_claims_published_total` (counter only; no target_author_did, no reason text).
- `federation_e2e_seconds` histogram bucketed (no per-event detail).

EXPLICITLY NEVER sent over telemetry (even when opted in):

- `peer.pull.rejected.detail` (could contain peer-published content fragments).
- `peer_did` values (a stream of peer DIDs would reveal the user's subscription graph).
- `target_cid` / `target_author_did` from counter-claim events (would reveal the user's disagreement targets).
- `reason` text from counter-claims (free-form user-authored content; never sent).
- `claim.counter.composed.contains_not_as_truth` boolean (sounds harmless but its emission pattern correlates with user behavior; not sent).

The `[telemetry]` section of `config.toml` (per foundation §6.1) does NOT
need a new subsection for slice-03 — the existing on/off semantics suffice.
If a user later wants federation-level granularity (e.g., "send aggregate
peer counts but not counter-claim publish counts"), THAT is a future
extension to ADR-010 — not slice-03's concern.

## 8. Health checks (extension)

Per §3 above. Startup gate behavior UNCHANGED — the binary still emits
`health.startup.refused` and exits 2 on probe refusal. The new probe
extensions widen what `probe_all` covers; the exit-code semantics carry.

The `--offline` flag (foundation §7.3) now ALSO skips the
`peer.pull_path` probe (which is a network probe in disguise). The
existing PDS-skip logic generalizes.

## 9. Dashboards (UNCHANGED — none)

Slice-03 ships NO dashboards. Same reasoning as foundation §8: solo dev,
single user per binary, no central aggregation. The KPI-FED-3 "dashboard"
called out in `outcome-kpis.md` §3 Handoff to DEVOPS item 2 is — for
slice-03 — the `openlore stats --federation` CLI render against the local
log, NOT a Grafana/Honeycomb instance. The DEVOPS-handoff item is
satisfied by the CLI surface plus the `scripts/kpi-fed-3.jq` fallback.

When a future telemetry endpoint exists (post-slice-05, the AppView wave's
problem), the same events feed cohort-level dashboards. The instrumentation
contract is forward-compatible.

## 10. Alerting (UNCHANGED policy; one delta)

Slice-03 ships NO continuous alerting (no on-call). However, three CI-time
alerts that ARE shipped (per `outcome-kpis.md` Handoff to DEVOPS item 3):

| Alert | Trigger | Surface |
|---|---|---|
| KPI-FED-1 != 100% in CI | `at-federation-attribution-preserved` test failure | GitHub Actions check; release-blocking via existing branch protection |
| KPI-FED-2 > 0 in CI | `at-federation-attribution-preserved` test failure (covers both KPI-FED-1 and KPI-FED-2 by checking author_did field non-null) | same |
| KPI-FED-6 != 100% in CI | `at-peer-tampered-signature-rejected` test failure | same |

The KPI-FED-5 informational alert ("P95 exceeds 180s") is NOT
automatically wired because there is no central metric store to alert
against. The CI test `at-peer-cid-round-trip` runs against a fixture with
≤20 records, so it implicitly bounds the P95 for that fixture. If the test
takes > 30s wall-clock for the standard fixture, that's a regression
surfaced by CI flakiness — not an SLO alert but a developer signal.

## 11. KPI-to-instrumentation mapping (cross-link)

See `kpi-instrumentation.md` delta in this dir for the per-KPI-FED traceability table.

## 12. References

- `platform-design.md` (sibling, this dir)
- `ci-cd-pipeline.md` (sibling, this dir) — new acceptance jobs that consume these events
- `kpi-instrumentation.md` (sibling, this dir)
- Foundation `observability.md`
- Foundation `kpi-instrumentation.md`
- `docs/feature/openlore-federated-read/feature-delta.md` (WD-14..WD-25)
- `docs/feature/openlore-federated-read/discuss/outcome-kpis.md`
- ADR-010 (telemetry-opt-in) — still in force
