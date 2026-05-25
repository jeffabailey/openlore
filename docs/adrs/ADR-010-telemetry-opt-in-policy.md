# ADR-010: Telemetry is Opt-In, Off by Default, Locally Buffered

- **Status**: Accepted (locked by user 2026-05-25)
- **Date**: 2026-05-25
- **Deciders**: Apex (nw-platform-architect)
- **Feature**: openlore-foundation (slice-01 walking skeleton)

## Context

OpenLore is a local-first tool for a privacy-conscious senior-engineer
persona (P-001). The DISCUSS-wave KPIs include:

- **Per-user signals** (KPI-1 time-to-publish, KPI-2 non-abandon, KPI-3
  felt-framing, KPI-4 field-mismatch counter, KPI-5 offline guardrail).
- **Cohort signals** (KPI-3 averaged across the first 50 publishes; KPI-6
  the day-30 north-star) that only become meaningful when aggregated across
  multiple users.

The platform layer needs to capture both kinds of signal without
compromising the privacy posture or the local-first invariant. Per
`outcome-kpis.md` §Guardrails: "zero unsolicited network calls before the
user's explicit publish confirmation."

## Decision

**Telemetry is OPT-IN, OFF by default, locally BUFFERED, and never
exfiltrated until both (a) the user enables it AND (b) a future
OpenLore-operated endpoint exists.**

### Surface

A `[telemetry]` section in `$XDG_CONFIG_HOME/openlore/config.toml`:

```toml
[telemetry]
enabled = false                                  # default
# endpoint = "https://telemetry.openlore.example/events"   # reserved for self-host
```

### Behavior

| User state | Result |
|---|---|
| `enabled = false` (default) | NO telemetry-buffer file ever created. No outbound traffic. The `tracing` Layer for telemetry is not even installed. |
| `enabled = true`, no endpoint operated (slice-01 case) | Anonymized event JSON Lines APPEND to `$XDG_DATA_HOME/openlore/telemetry-buffer/events.jsonl`. NO outbound traffic. User can `cat` the file to self-inspect. |
| `enabled = true`, endpoint operated (future) | Daily rollup of the buffer is POSTed to the endpoint; on success, the buffer is rotated; on failure, the buffer is retained and retried next day. |

### What can be sent (when endpoint exists)

ONLY anonymized aggregate counts:
- KPI-1: time-to-publish histogram (bucketed seconds, not raw timestamps).
- KPI-2: counts of compose-started / publish-success / cancel / abort.
- KPI-4: count of field_mismatch_total events (target = 0).
- KPI-5: count of offline-success / network-call-before-publish events.
- KPI-3, KPI-6: numeric survey scores (never the free-text fields).
- An anonymous random installation UUID (regenerable by deleting the
  buffer dir).

### What is NEVER sent

- The user's DID.
- Claim CIDs (per-DID stream reveals authorship patterns).
- Claim subjects, predicates, objects, evidence URIs, free-text survey
  responses.
- Filesystem paths.
- PDS endpoint hostnames (only categorical error class is recorded).
- IP / hostname / OS-build-id / hardware ids.

## Alternatives Considered

| Option | Rejection rationale |
|---|---|
| **Opt-out (telemetry on by default)** | Violates the privacy posture; would surprise the persona; arguably violates KPI-5 spirit even though publish IS the explicit consent. Strong reject. |
| **No telemetry at all, ever** | KPI-3 and KPI-6 require cohort aggregation to validate the product hypothesis. Without ANY path to that, the north-star KPI is forever unmeasurable. Reject as unmeasurement-by-design. |
| **Immediate POST-on-event (no buffer)** | Violates KPI-5-adjacent invariant "no surprise network traffic"; couples the CLI to an endpoint we don't operate yet; brittle. Reject. |
| **Build a backend in slice-01** | Scope creep; not in slice-01 brief; the buffer-only design captures every signal locally so the future endpoint just consumes what's already there. |
| **Use a third-party (Sentry, PostHog, Mixpanel)** | Vendor lock-in; sends data to a third party which violates the local-first ethos; introduces a hosted dependency we don't operate. Reject. |

## Consequences

### Positive

- Default behavior is "no surprise traffic, ever" — preserves KPI-5
  invariant and persona trust.
- Per-user signal is captured locally in slice-01 (the user reads their
  own metrics via `openlore stats` or `jq`).
- The future endpoint is straightforward to add — it consumes JSON Lines
  that already exist. No client-side rework.
- The buffer file is self-inspectable; a privacy-conscious user can
  enable telemetry, run for a week, `cat` the buffer, and decide whether
  the data is acceptable to send.

### Negative

- KPI-3 cohort aggregation (avg score across first 50 publishes) and
  KPI-6 cohort aggregation (>=60% with >=3 unblogged) require the
  endpoint that doesn't exist in slice-01. Mitigation: PO (Luna) can
  coordinate out-of-band surveys to known users for the first cohort.
- The buffer can grow unbounded if the user enables telemetry but the
  endpoint never materializes. **Mitigation**: cap the buffer at 10 MiB
  (~tens of thousands of events for a CLI used a few times a day);
  rotate when full; oldest entries are dropped silently. Documented in
  the config.toml comments.

## Architecture Enforcement

- The telemetry-buffer write path MUST be a distinct `tracing` Layer that
  is only added to the subscriber when `config.telemetry.enabled` is true
  at startup. Verified by a unit test in `crates/cli` that constructs the
  subscriber from a `config.telemetry.enabled = false` config and asserts
  the resulting subscriber stack has no telemetry layer.
- An integration test asserts that when telemetry is OFF, no file ever
  appears under `$XDG_DATA_HOME/openlore/telemetry-buffer/`.
- A CI gate (part of `kpi-5-offline` per `ci-cd-pipeline.md` §4.3) asserts
  no outbound HTTP call is made by the telemetry layer even when ON
  (because no endpoint configured in slice-01).

## Earned Trust

The `--features network-audit` debug build (per `observability.md` §4.3)
exposes a `network_calls_total_debug` counter. An integration test exercises
the full opt-in path with telemetry enabled and asserts the counter remains
at zero throughout compose-and-sign — proving the telemetry layer does not
break the local-first invariant.

## Revisit Trigger

- A future slice (likely a sibling-feature DEVOPS wave) introduces the
  telemetry endpoint. At that point: this ADR becomes superseded by a
  follow-up ADR specifying the endpoint protocol, auth, and rollup format.
- A user requests a self-hosted endpoint URL config: the `endpoint =`
  field already exists for that; no new ADR needed.
