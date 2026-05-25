# KPI Instrumentation — openlore-foundation (slice-01)

- **Wave**: DEVOPS
- **Date**: 2026-05-25
- **Architect**: Apex
- **Source-of-truth KPIs**: `docs/feature/openlore-foundation/discuss/outcome-kpis.md`

This document traces each of the 6 outcome KPIs to:

- **What** measures it (event/test/survey).
- **Where** the data lives.
- **How** the developer-as-operator reads it (CLI command, log query, dashboard).
- **Feasibility tag**: GREEN / YELLOW / RED for instrumentation under
  slice-01 constraints.

## 1. Summary table

| KPI | Type | Instrumentation | Read mechanism | Feasibility |
|---|---|---|---|---|
| KPI-1 (e2e < 2 min) | Leading | `tracing` events `compose.started` -> `publish.success`; histogram | `openlore stats` OR `jq` over JSON log | **GREEN** |
| KPI-2 (>=80% non-abandon) | Leading (activation) | `tracing` event ratio `publish.success | cancel | abort` / `compose.started` | `openlore stats` OR `jq` | **GREEN** |
| KPI-3 (>=4/5 felt-framing) | Leading (qualitative) | one-shot CLI prompt + local survey-response file | survey JSON file in `$XDG_DATA_HOME/openlore/surveys/` | **YELLOW** (delivery vector solid; aggregation depends on opt-in telemetry) |
| KPI-4 (100% round-trip, 0 mismatches) | Guardrail | integration test in CI + `field_mismatch_total` counter | CI status + `openlore stats` counter | **GREEN** |
| KPI-5 (100% offline compose/sign) | Guardrail | network-namespace integration test in CI | CI status | **GREEN** |
| KPI-6 (>=60% cohort, >=3 claims would-not-have-blogged) | North Star | day-30 CLI prompt + local survey-response file | survey JSON file; cohort aggregation requires future endpoint | **YELLOW** (delivery solid; cohort rollup requires opt-in telemetry to enabled endpoint, deferred to post-slice-01) |

**No KPI is RED.** Every one has a designed local capture mechanism that
ships in slice-01. The two YELLOW items reflect "aggregation across the
cohort requires a backend we don't operate yet" — the per-user signal IS
captured and readable.

## 2. KPI-1 — Time-to-publish < 2 min

### What
Wall-clock interval between `compose.started` and `publish.success` events
for a single session. The session is identified by a `tracing` span (`verb.claim_add`).

### Where the data lives
JSON log file: `$XDG_DATA_HOME/openlore/logs/openlore.log` (per `observability.md` §3.3). Events tagged with span id; a session's start and end events share a span.

### How the developer reads it

**Preferred (if `openlore stats` lands per `observability.md` §4.2)**:
```
$ openlore stats
...
Time to publish (last 30 sessions):
  p50: 38 s   p90: 1m 12 s   p99: 1m 51 s   max: 2m 04 s
  Sessions above 2 min target: 1 (of 30) = 3.3%
```

**Fallback (always available)**:
```
$ jq -s '
    map(select(.event == "compose.started" or .event == "publish.success"))
    | group_by(.span.id)
    | map(select(length == 2))
    | map({
        sid: .[0].span.id,
        dur_s: ((.[1].ts | fromdateiso8601) - (.[0].ts | fromdateiso8601))
      })
    | sort_by(.dur_s)
  ' $XDG_DATA_HOME/openlore/logs/openlore.log
```

DELIVER should ship a `scripts/kpi-1.jq` snippet so the user does not have
to write the jq inline.

### Aggregation across cohort
NOT in slice-01. Per-user only. Cross-user aggregation requires the future
opt-in telemetry endpoint (proposed ADR-010).

### Feasibility: GREEN

## 3. KPI-2 — Non-abandon rate >= 80%

### What
Ratio of `(publish.success + cancel)` events to `compose.started` events
over a rolling window. Sessions ending in `abort` (the user killed the
process / power failure) are counted as ABANDON.

### Where the data lives
Same JSON log file. Events: `compose.started`, `publish.success`, `cancel`
(user dismissed the publish prompt with `n`), `abort` (process exited
without either of the prior two — inferred from "no terminal event in the
span before next session's `compose.started`").

### How the developer reads it
- `openlore stats` renders `Non-abandon rate (last 30 days): 87% (26 sessions, 4 cancel, 0 abort)`.
- Fallback: a `scripts/kpi-2.jq` shipped by DELIVER.

### Aggregation across cohort
NOT in slice-01 (same constraint as KPI-1).

### Feasibility: GREEN

## 4. KPI-3 — Felt-framing >= 4/5 (post-publish survey)

### What
A one-shot 5-point Likert survey delivered AFTER the user's first
successful `publish.success`. Question: "The claim felt like my reasoning,
not a truth assertion." Scale 1-5. Dismissable.

### Where the data lives

**Delivery state** (so it's truly one-shot): `$XDG_DATA_HOME/openlore/surveys/post-publish.status` — file present after first delivery; never re-delivered.

**Response** (if user answers): `$XDG_DATA_HOME/openlore/surveys/post-publish.response.json`:
```json
{
  "kpi": "KPI-3",
  "delivered_at": "2026-05-25T12:34:56Z",
  "answered_at": "2026-05-25T12:35:42Z",
  "score": 4,
  "free_text_optional": null
}
```

### How the developer reads it
- `openlore stats --surveys` prints the survey response status + answer.
- The file is human-readable JSON.

### Delivery mechanism
After the FIRST `publish.success` event in a session AND the post-publish
status file is absent, the CLI prints (to stdout, after the success
message):

```
A quick one-time question to improve OpenLore (press Enter to skip):
  "The claim felt like my reasoning, not a truth assertion."
  Rate 1-5 (1=disagree, 5=agree, Enter=skip): _
```

Then writes the status file regardless of whether the user answered, so it
is never re-prompted.

### Aggregation across cohort
Slice-01: per-user only (file on user's disk).
Post-slice-01: if the user opts into telemetry, the score IS the kind of
anonymizable signal that can be sent (no PII). The free-text field is
**never sent**, only the numeric score.

Cohort-level "≥4/5 average across the first 50 publishes" can only be
computed once the telemetry endpoint exists. Per-user the prompt fires
correctly and the data is captured.

### Feasibility: YELLOW
Delivery mechanism is solid and ships in slice-01. Per-user signal is
captured. Cohort aggregation requires future endpoint — DEVOPS-future
problem.

## 5. KPI-4 — Zero silent normalization (100% round-trip identity)

### What
Two-layer enforcement:
1. **CI gate (every PR, every release)**: integration test `kpi_4_roundtrip` in `tests/kpi_4_roundtrip.rs` (per `ci-cd-pipeline.md` §4.2). Composes a claim with adversarial field values (Unicode normalization edge cases, leading/trailing whitespace, float boundary values, RFC3339 with non-UTC offset, large evidence arrays). Writes → DB + JSON file. Reads → asserts byte-equal field-for-field.
2. **Runtime counter (telemetry)**: every time `verb_query` renders a claim, the read pipeline asserts the rendered Lexicon-shape values equal the stored values. On any mismatch, increment `field_mismatch_total` counter AND emit `tracing::error!("kpi4.field_mismatch", cid, field, stored, rendered)`. Target = 0 across all users, forever.

### Where the data lives
- CI status: GitHub Actions checks tab on every PR / tag.
- Runtime counter: in the JSON log file (count occurrences of `kpi4.field_mismatch` events).

### How the developer reads it
- CI: pass/fail in the PR view.
- Runtime: `openlore stats` shows `Field mismatches detected: 0` (anything else is a P0 bug-report-worthy event).
- The runtime event is also logged at ERROR level, which trips on stderr
  unconditionally (per `observability.md` §3.5 — ERROR always escapes the
  default suppression).

### Aggregation across cohort
The CI gate is the cohort signal — if it passes, EVERY user's binary has
the property. The runtime counter is a per-user signal; aggregating across
users requires future telemetry endpoint (and the counter being non-zero
anywhere triggers an automatic GitHub issue if telemetry enabled).

### Feasibility: GREEN
CI gate is fully designed in `ci-cd-pipeline.md` §4.2. Counter is fully
designed in `observability.md` §4.3.

## 6. KPI-5 — Offline compose-and-sign succeeds (100%)

### What
Integration test `kpi_5_offline` in `tests/kpi_5_offline.rs` (per
`ci-cd-pipeline.md` §4.3). On Linux (strict variant): runs the binary
under `unshare -n`; asserts `openlore claim add --no-tty` produces a
signed claim on disk; asserts the subsequent `openlore claim publish`
call fails cleanly with non-zero exit and a user-actionable stderr
message. On macOS: degraded variant via `HTTPS_PROXY=http://127.0.0.1:1`.

Also: the second guardrail from `outcome-kpis.md` §Guardrails — "zero
unsolicited network calls before the user's explicit publish confirmation"
— is enforced by the `network_calls_total_debug` counter in
`observability.md` §4.3. Tested by an additional integration test
`kpi_5_no_unsolicited_network` that runs the CLI through compose+sign and
asserts `network_calls_total_debug == 0` BEFORE the publish prompt.

### Where the data lives
- CI status: GitHub Actions.
- Runtime: the network-audit counter requires `--features network-audit`
  (only enabled in CI test builds; not in shipped binaries — keeps
  release binary lean).

### How the developer reads it
- CI status (primary signal).
- Locally, if a user enables `--features network-audit` in a debug build,
  `openlore stats` shows `Network calls per session: 0 (before publish), N (after publish)`.

### Aggregation across cohort
CI gate is the cohort signal. Per-user runtime measurement requires the
debug feature flag; not relevant in production.

### Feasibility: GREEN

## 7. KPI-6 — North Star — Day-30 cohort survey (>=60% report >=3 claims would-not-have-blogged)

### What
Two-question survey delivered 30 days after the user's first
`publish.success`:

1. "Of your last N claims, how many would you have published as a blog post?"
   (numeric: 0 to N)
2. "Of your last N claims, how many would you have left in a private notebook?"
   (numeric: 0 to N)

The KPI fires when (response to Q1) <= (N - 3), interpreted as ">=3 claims
would NOT have been blogged".

### Where the data lives

**Delivery state**:
`$XDG_DATA_HOME/openlore/surveys/day-30-prompt.status` (records when first
publish happened; used to schedule the prompt on the first invocation on
or after first-publish + 30 days).

**Response**:
`$XDG_DATA_HOME/openlore/surveys/day-30.response.json`:
```json
{
  "kpi": "KPI-6",
  "first_publish_at": "2026-05-25T12:00:00Z",
  "prompted_at": "2026-06-24T09:14:21Z",
  "answered_at": "2026-06-24T09:15:03Z",
  "n_last_claims": 7,
  "would_have_blogged": 2,
  "would_have_private_notebooked": 4
}
```

### Delivery mechanism
On every CLI invocation, check the `day-30-prompt.status` file. If first
publish was >= 30 days ago AND the response file does not yet exist AND
the user has run the CLI at least 5 times since first publish (sanity
check — don't ambush someone who used it once and walked away), print the
prompt at the start of the invocation. Dismissable (Enter to skip).

### How the developer reads it
- `openlore stats --surveys` shows their own answer.
- For the cohort signal: requires future telemetry endpoint (the YELLOW
  classification).

### Aggregation across cohort
THIS is the load-bearing aggregation. Without it, the north-star KPI is
visible only one user at a time. Per Morgan and Luna's design intent, the
slice-01 release ships the per-user capture, and DEVOPS-future (or a
sibling-feature wave introducing the telemetry endpoint) ships the
aggregation.

The mitigation in `outcome-kpis.md` §Risks logged that "KPI-3 + KPI-6 are
designed to surface job mis-prioritization within 30 days of slice-01
release" assumes either (a) telemetry is enabled by 30 days post-release
OR (b) the PO (Luna) coordinates an out-of-band survey via direct outreach
to known users.

### Feasibility: YELLOW
Per-user signal: GREEN (designed, ships in slice-01).
Cohort aggregation: requires telemetry endpoint, deferred.

## 8. Survey delivery non-design notes

- **Cancellable**: every survey prompt accepts Enter as "skip". A skipped
  prompt is RECORDED (status file gets `answered_at: null, skipped: true`)
  so it is not re-shown.
- **Never block automation**: in `--no-tty` mode, the prompt does NOT
  appear (would deadlock scripts). The status file gets `skipped_in_no_tty: true`.
- **No PII**: questions are about the user's experience, not their identity.
  Free-text fields are explicitly NOT sent over telemetry (kept local for
  the user only).
- **Reset path**: the user can `rm -rf ~/.local/share/openlore/surveys/` if
  they want to be re-asked. Documented in the user-facing `--help`.

## 9. Cross-references

- `observability.md` §4.3 — metric event names that back KPI-1, KPI-2, KPI-4-counter, KPI-5-network-audit
- `observability.md` §6 — telemetry opt-in (the YELLOW path)
- `ci-cd-pipeline.md` §4.2 (kpi-4-roundtrip), §4.3 (kpi-5-offline) — the GREEN CI gates
- `discuss/outcome-kpis.md` — authoritative KPI definitions
- Proposed ADR-010 — telemetry opt-in policy (governs the YELLOW path)

## 10. KPI sign-off readiness (per task spec §Phase 6)

| KPI | Per-user instrumented in slice-01? | Cohort aggregated? | Status |
|---|---|---|---|
| KPI-1 | YES | NO (future endpoint) | GREEN |
| KPI-2 | YES | NO (future endpoint) | GREEN |
| KPI-3 | YES (one-shot prompt) | NO (future endpoint) | YELLOW |
| KPI-4 | YES (CI gate + runtime counter) | YES (CI = cohort property) | GREEN |
| KPI-5 | YES (CI gate) | YES (CI = cohort property) | GREEN |
| KPI-6 | YES (day-30 prompt) | NO (future endpoint) | YELLOW |

No RED. All 6 KPIs are at minimum per-user-readable from slice-01 binary.
