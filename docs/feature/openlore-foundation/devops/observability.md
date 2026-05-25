# Observability — openlore-foundation (slice-01)

- **Wave**: DEVOPS
- **Date**: 2026-05-25
- **Architect**: Apex

This document specifies what the OpenLore binary instruments and where the
signals go. Audience: the **developer-as-operator** (the user themselves)
debugging their own crash reports — no central aggregation, no SRE team.

## 1. Operating model recap

- No remote logging endpoint.
- No metrics endpoint.
- No distributed traces (single-binary CLI).
- Telemetry is **opt-in via a config flag**, off by default, and even when
  on it only buffers locally for now (no exfiltration endpoint operated in
  slice-01).
- Logs are the user's local files; they read them when something breaks.

## 2. Three Pillars (per `infrastructure-and-observability` skill §Three Pillars)

| Pillar | Slice-01 status |
|---|---|
| Logs | YES — structured JSON via `tracing` + `tracing-subscriber`, written to a local rolling file + stderr in verbose mode |
| Metrics | YES — emitted as structured `tracing` events at boundaries; rendered locally on demand (no Prometheus endpoint) |
| Traces | DEFERRED with revisit trigger (single-process; no distributed system) |

## 3. Logging

### 3.1 Library choice

- **`tracing`** crate (already in `technology-stack.md`) for emission.
- **`tracing-subscriber`** for sinks and formatting.
- No vendor-specific crate (no `tracing-honeycomb`, `datadog-tracing`,
  `tracing-opentelemetry-otlp-exporter`). Adding `opentelemetry` as an
  optional dependency behind a `--features otel` cargo feature is acceptable
  for a future slice; slice-01 does not enable it.

### 3.2 What gets instrumented

**Boundary instrumentation** (mandatory):
- Every CLI verb invocation: `tracing::info!("verb.invoked", verb = name, args_hash = ...)`. `args_hash` is a non-secret hash of normalized args for grouping; raw arg values are NOT logged (potential PII in user-typed claim text).
- Every adapter port-method entry/exit: `tracing::debug!("port.call", port = "StoragePort", method = "write_signed_claim", cid = ...)` on entry; matching `tracing::debug!("port.return", ..., result = ok|err, latency_us = ...)` on exit.
- Every `probe()` invocation: `tracing::info!("probe.result", adapter = ..., outcome = ok|refused, reason = ..., detail = ...)`.

**Domain-event instrumentation** (mandatory):
- `compose.started` — emitted when user begins composing a claim.
- `compose.preview_rendered` — emitted just before the first prompt; payload includes `contains_not_as_truth = true|false` (the boolean is a CI-watched invariant — should ALWAYS be true; if logged false, that's a bug).
- `sign.success` — payload includes `unsigned_cid`, `signed_cid`, `latency_ms`.
- `publish.attempt` — payload includes `cid`, `target_pds_endpoint`.
- `publish.success` — payload includes `cid`, `at_uri`, `latency_ms`.
- `publish.refused` — payload includes `cid`, `reason`, `pds_error_body` (if available).
- `retract.composed` — emitted before publishing a counter-claim; references original cid.
- `query.executed` — payload includes `query_kind`, `result_count`, `latency_ms`.

**Health events** (mandatory, per ADR-009 D-9):
- `health.startup.refused` — payload `{ adapter, reason: ProbeRefusalReason, detail, structured }`. Emitted to stderr + log file BEFORE the process exits 2.

**Error/failure instrumentation**:
- Every `Result::Err` returned at a port boundary: `tracing::warn!("port.error", port = ..., method = ..., kind = ..., chain = ?err)`. The `chain` field walks `std::error::Error::source()`.

### 3.3 Where logs go

Destinations, in priority order:

1. **Local rolling file**: `$XDG_DATA_HOME/openlore/logs/openlore.log`
   (resolved via `directories` crate; on macOS, this is
   `~/Library/Application Support/openlore/logs/openlore.log`; on Linux,
   `~/.local/share/openlore/logs/openlore.log`). Rotation policy: 10 MiB
   per file, keep last 5. Implemented via the `tracing-appender` crate (a
   sibling of `tracing-subscriber`).
2. **stderr** — only when `--verbose` flag is set OR `OPENLORE_LOG=debug` env var. Default behavior: stderr emits the human prompts/preview/success messages only; structured logs do NOT pollute the TTY.
3. **NO remote sink by default**. See §6 telemetry opt-in for the future endpoint design.

### 3.4 Format

- File sink: **JSON Lines** (one JSON object per line, `tracing_subscriber::fmt::format::Json`).
- stderr sink: **human-readable text** (`tracing_subscriber::fmt::format::Format::default().compact()`) UNLESS `--log-json` flag is set, in which case stderr also gets JSON.

JSON line schema (illustrative; DELIVER finalizes via the `tracing` API):

```
{
  "ts": "2026-05-25T12:34:56.789Z",
  "level": "INFO",
  "target": "openlore::cli::verb_claim_add",
  "event": "sign.success",
  "fields": {
    "unsigned_cid": "bafy...",
    "signed_cid": "bafy...",
    "latency_ms": 4
  },
  "span": { "name": "verb.claim_add", "verb": "claim add", "args_hash": "9af..." }
}
```

### 3.5 Verbosity controls

| Flag / env | Effect |
|---|---|
| (default) | stderr: prompts + status messages only. File: INFO and above. |
| `--verbose` / `-v` | stderr also receives INFO+ structured logs (human format). |
| `OPENLORE_LOG=debug` | both sinks receive DEBUG (per-port-method entry/exit). |
| `OPENLORE_LOG=trace` | both sinks receive TRACE. Reserved for crafter use; never expected during normal user-debugging. |
| `--log-json` | stderr emits JSON instead of human format. Useful for piping to `jq`. |

## 4. Metrics

### 4.1 Approach

Slice-01 does NOT run a Prometheus scrape target or a push-gateway client.
Metrics are derived **on demand** from the local JSON log by counting/aggregating
the structured event records.

### 4.2 `openlore stats` (proposed CLI verb — flagged for DELIVER)

> **OPEN QUESTION FOR DELIVER**: this verb is NOT in Morgan's
> architecture-design.md §5.2 cli component diagram. Apex proposes adding it
> as an additional VerbStats component because there must be a user-facing
> way to read the metrics this design specifies. If DELIVER cannot land
> `openlore stats` in slice-01 due to scope, the user can still read the
> metrics directly with `jq` against the JSON log (the schema in §3.4 is
> structured enough). Flagged in `wave-decisions.md` D-D3 as an "Open
> question handed to DELIVER" matching Morgan's own pattern.

If `openlore stats` lands:

| Command | Renders |
|---|---|
| `openlore stats` | Summary card: total claims authored, total published, total retracted, average time-to-publish, confidence histogram (4 buckets), session-count, last-session-at. |
| `openlore stats --since '2026-05-01'` | Same, filtered. |
| `openlore stats --json` | Machine-readable. |

### 4.3 Per-event metrics tracked

Derived from the structured log events in §3.2:

| Metric | Source events | Render shape |
|---|---|---|
| `claims_authored_total` | count of `sign.success` | counter |
| `claims_published_total` | count of `publish.success` | counter |
| `claims_retracted_total` | count of `retract.composed` AND subsequent `publish.success` for that counter-claim cid | counter |
| `compose_started_total` | count of `compose.started` | counter (denominator for KPI-2 non-abandon rate) |
| `time_to_publish_seconds` (KPI-1) | `publish.success.ts - compose.started.ts` (matched by session id) | histogram (rendered as text-art percentiles or JSON) |
| `confidence_value` | the `confidence` field on every authored claim | distribution / histogram by display bucket |
| `field_mismatch_total` (KPI-4 telemetry counter) | count of any `query.executed` event where the result differs from the stored claim | counter; target = 0 |
| `network_calls_total_debug` | count of every outbound HTTP call when built with `--features network-audit` (debug build flag) | counter; per KPI-5 "zero unsolicited network calls before publish confirmation" guardrail |

### 4.4 Method alignment

Per the skill's metrics-method guidance (RED / USE / Golden Signals):

- **RED** is not a perfect fit — there's no request stream. The analogue is
  **per-verb invocation rate / error rate / duration**, which the event log
  captures naturally.
- **USE** is irrelevant for a CLI (no CPU/memory pressure to monitor at the
  application level beyond what the OS gives you).
- **Golden Signals** is partially relevant:
  - Latency: yes (per-verb wall-clock).
  - Traffic: invocation count per verb per day.
  - Errors: per-verb error rate.
  - Saturation: not applicable.

## 5. Traces

**Decision**: skip distributed tracing for slice-01.

- **Rationale**: single-binary, single-thread of execution per invocation; there's nothing distributed to trace. `tracing` spans suffice for in-process structure.
- **`tracing` spans WILL be used** for per-verb-invocation scoping and per-port-call scoping (these are what shows up as `span` in the JSON line schema in §3.4). They are NOT exported anywhere; they exist purely for log enrichment.
- **Revisit trigger**: slice-05 AppView introduces a separate process the
  CLI talks to. At that point, add `tracing-opentelemetry` and an OTLP
  exporter behind `--features otel`, with a docs note on bring-your-own
  collector (no vendor lock-in).

## 6. Telemetry opt-in (future-ready, off by default)

Per task spec: design the config flag mechanism; do NOT design the endpoint.

### 6.1 Config surface

The opt-in lives in `$XDG_CONFIG_HOME/openlore/config.toml` (resolved via
the `directories` crate). DELIVER adds a section:

```toml
[telemetry]
# Telemetry is OFF by default. Set to true to enable anonymized aggregate
# event counts to be SENT to a future OpenLore-operated endpoint.
# As of slice-01 there is no endpoint; setting this true makes openlore
# BUFFER the events locally in $XDG_DATA_HOME/openlore/telemetry-buffer/
# pending a future enabled endpoint.
enabled = false

# If set, overrides the default endpoint. Reserved for self-hosted aggregators.
# endpoint = "https://telemetry.openlore.example/events"
```

### 6.2 What gets sent (if enabled)

ONLY the following anonymized counters, on a daily rollup:
- per-KPI counters (KPI-1 publish-times histogram bucketed, KPI-2 abandon counts, KPI-4 field-mismatch count, KPI-5 offline-success count).
- The user's DID is NOT sent. A random per-installation UUID is sent (generated at first telemetry-buffer write; can be reset by deleting the buffer dir).
- No claim contents, no subjects, no philosophy URIs.

### 6.3 What DOES NOT get sent (ever, regardless of opt-in)

- `compose.preview_rendered` payload contents.
- `sign.success` CIDs (CIDs are public but a per-DID stream of CIDs reveals authorship patterns — exclude on principle).
- File system paths.
- Network errors with PDS endpoint hostnames (only categorical error class is sent).

### 6.4 Implementation note (for DELIVER)

The telemetry-buffer writer is a thin layer over the same `tracing`
subscriber pipeline — it's a separate `Layer` that ONLY activates if
`config.telemetry.enabled = true`. The buffer is append-only JSON Lines;
even if the future endpoint never materializes, the user can `cat` the file
and self-inspect what would have been sent.

## 7. Health checks

Per task spec: surface Morgan's `health.startup.refused` hook.

### 7.1 Startup health gate

The composition root's "wire-probe-use" sequence (ADR-009 §Composition root
invariant) IS the startup health check. The mechanics:

1. `cli::main` wires adapters.
2. `cli::main` invokes `probe()` on every adapter.
3. On ANY `ProbeOutcome::Refused`, the binary:
   - Emits a `tracing::error!("health.startup.refused", adapter, reason, detail, structured)` event to BOTH the log file AND stderr (unconditionally, regardless of `--verbose`).
   - Prints a human-readable summary to stderr explaining what failed and what the user should do (per `ProbeRefusalReason` variant — DELIVER writes the mapping).
   - Exits with code **2** (per ADR-009).

### 7.2 What `probe_all` checks (per Morgan's component-boundaries.md probe responsibilities)

| Adapter | Probe verifies |
|---|---|
| `adapter-duckdb` | DuckDB file opens; schema version matches binary; sentinel write+read+byte-equal succeeds; `fsync` honored on the storage medium |
| `adapter-atproto-did` | User's DID document resolves; OpenLore verification method present in the DID doc; sentinel sign+verify with the local key produces a sig the DID-doc-published key verifies; keychain readable; WSL2-fallback file perms = `0600` |
| `adapter-atproto-pds` | TLS handshake against `cfg.pds_endpoint` succeeds; `com.atproto.server.describeServer` returns a DID matching the user's PDS DID; rkey-collision idempotency probe (write sentinel twice, assert no overwrite) |
| `adapter-system-clock` | trivial, always Ok |
| `lexicon` module probe | every Lexicon JSON validates against the Lexicon schema-of-schemas; serde round-trip is byte-equal |

### 7.3 The `--offline` flag

Per ADR-009: `pds.probe()` is SKIPPED when `--offline` is set (so KPI-5
offline-compose-and-sign succeeds even though the PDS is unreachable).
The remaining probes (storage, identity, lexicon, clock) MUST still pass —
they are local and unaffected by network.

### 7.4 Exit codes

| Code | Meaning |
|---|---|
| 0 | success |
| 1 | user-visible error (validation, user-input rejection) — exists, not a probe failure |
| 2 | `health.startup.refused` — adapter probe failed at startup |
| 3 | unrecoverable I/O after probe passed (rare; bug-likely) |
| 64+ | reserved for future |

### 7.5 Runtime "is everything OK" — there is no `/health` endpoint

A CLI doesn't have a long-running process to ping. The user invokes the CLI,
either it works or it doesn't. The startup probe IS the entire health-check
surface.

## 8. Dashboards

**Slice-01 ships NO dashboards.** Per Morgan's architecture-design.md §7
and `outcome-kpis.md` final paragraph, aggregated dashboards become
meaningful only after slice-05 (AppView).

- For slice-01 the user reads metrics via `openlore stats` (if it lands;
  see §4.2) or via `jq` over the JSON log.
- For the developer-as-operator persona, this is the right level. A
  Grafana/Honeycomb dashboard for a CLI used by 1 person is overkill.

**Future-revisit**: post-slice-05, when there's an AppView that aggregates
across many users, the AppView itself becomes the surface for cross-user
KPI dashboards. That's a sibling-feature DEVOPS wave's problem.

## 9. Alerting

**Slice-01 ships NO alerting.** Per the constraint (solo dev, no on-call,
no SLA contracts).

- The user IS the operator. Their feedback loop is "the CLI exited non-zero
  with a message I should read."
- The `health.startup.refused` event IS the alert — it's printed to stderr
  in the user's face, in human-readable form, on the spot.

**Future-revisit**: slice-05 AppView IS an operated service. That wave's
DEVOPS will design the SLOs, error budgets, and alerting tiers (per
`production-readiness` skill §Alerting Tiers). Not slice-01's concern.

## 10. KPI-to-instrumentation mapping (cross-link)

See `kpi-instrumentation.md` for the per-KPI traceability table.

## 11. References

- `platform-design.md` (sibling)
- `ci-cd-pipeline.md` (sibling) — `kpi-4-roundtrip` and `kpi-5-offline` test specs
- `kpi-instrumentation.md` (sibling)
- Morgan: `architecture-design.md` §7, §8, §9 (Earned Trust); `component-boundaries.md` per-adapter probe responsibilities; ADR-009 §Composition root invariant
- Proposed ADR-010 (telemetry opt-in policy)
