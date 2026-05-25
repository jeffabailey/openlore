# Outcome KPIs — openlore-foundation (slice-01)

## Feature: openlore-foundation (walking skeleton: author and publish a signed claim)

### Objective

Validate that a senior engineer can take a structured philosophical opinion from
their head to a signed, federated, locally-queryable claim — and feel the system
framed it as their reasoning, not as a truth assertion — in a single session.

### Outcome KPIs

| # | Who | Does What | By How Much | Baseline | Measured By | Type |
|---|---|---|---|---|---|---|
| KPI-1 | Senior Engineer (P-001) authoring their first claim | composes, signs, and publishes a claim end-to-end | within 2 minutes wall-clock | n/a (new behavior) | telemetry: `compose_start_at` → `publish_success_at`, opt-in local log | Leading |
| KPI-2 | Senior Engineer (P-001) running `openlore claim add` | reaches publish without abandoning | ≥80% non-abandon rate of started compose sessions | n/a | telemetry: count of started composes vs published or explicitly cancelled (vs aborted) | Leading (activation) |
| KPI-3 | Senior Engineer (P-001) post-first-publish | self-reports the claim "felt like my reasoning, not a truth assertion" | ≥4/5 average on a 5-point post-publish survey (opt-in, one-shot) | n/a | one-time post-publish survey on first 50 publishes | Leading (qualitative-as-quantitative) |
| KPI-4 | Senior Engineer (P-001) querying after publish | sees graph query output matching compose-preview field-for-field | 100% (zero tolerance for silent normalization) | n/a | integration test on every release, plus telemetry counter for `field_mismatch_detected` (target: 0) | Leading (correctness, guardrail) |
| KPI-5 | Senior Engineer (P-001) authoring offline | completes compose-and-sign with network disabled | 100% success of compose-and-sign with network off | n/a | integration test: run the CLI with network namespace disabled; compose-and-sign must succeed; publish must fail cleanly | Leading (guardrail) |
| KPI-6 | Senior Engineer (P-001) after 30 days of use | publishes ≥3 claims they would NOT have published as a blog post | ≥60% of P-001 cohort | n/a (new behavior) | post-30-day survey: "Of your last N claims, how many would you have published as a blog post?" | Leading (the real product hypothesis) |

### Metric Hierarchy

- **North Star**: **KPI-6** — would the user have published this opinion outside OpenLore? This is the actual product hypothesis. Every other metric is a leading indicator of this one.
- **Leading Indicators**: KPI-1 (time-to-publish), KPI-2 (non-abandon rate), KPI-3 (felt-framing).
- **Guardrail Metrics**: KPI-4 (zero silent normalization), KPI-5 (local-first invariant holds offline).

### Measurement Plan

| KPI | Data Source | Collection Method | Frequency | Owner |
|---|---|---|---|---|
| KPI-1 | local telemetry (opt-in) | timestamps on compose/publish events | per-session | DEVOPS |
| KPI-2 | local telemetry (opt-in) | counter of compose-started / publish-success / cancel | weekly aggregate | DEVOPS |
| KPI-3 | opt-in one-time survey | post-first-publish prompt, dismissible | one-shot per user | PO |
| KPI-4 | integration test + telemetry counter | CI test + counter increment on mismatch | every release + continuous | DEVOPS |
| KPI-5 | integration test | network-namespaced CI test | every release | DEVOPS |
| KPI-6 | 30-day survey (opt-in) | email or CLI prompt at day-30 | one-shot per user at day-30 | PO |

### Hypothesis

> We believe that a CLI flow that explicitly frames published opinions as **claims**
> (not truth), surfaces the literal text "not as truth" in compose, and shows the
> retract command at publish-time, will move Senior Engineers (P-001) from
> "I have this opinion but won't blog about it" to "I published it as a claim."
> We will know this is true when **≥60% of P-001 cohort report (at day-30) that
> ≥3 of their last N claims would not have been published as a blog post**.

### Guardrails

- KPI-4 and KPI-5 are guardrails — they MUST NOT degrade. A release that lowers
  either is blocked, regardless of how well KPI-1/2/3/6 are moving.
- An additional guardrail not formalized as a KPI: zero unsolicited network calls
  before the user's explicit publish confirmation. Verified by integration test.

### Handoff to DEVOPS (platform-architect)

Required instrumentation:

1. **Local opt-in telemetry**: `compose_start_at`, `sign_success_at`, `publish_success_at`, `cancel_at`, `abort_at` events with monotonic timestamps. Stored locally (XDG state dir). Never auto-exfiltrated.
2. **Field-mismatch counter**: `KPI-4` requires a counter that increments any time the query output for a just-published claim differs from compose-time field values. Initial target: always 0.
3. **Network-call audit log** (debug build only): every outbound HTTP call, with timestamp, endpoint, and triggering command. Enables verifying the local-first guardrail.
4. **One-shot post-publish survey hook**: CLI-side prompt mechanism to deliver KPI-3 once per user after first publish, dismissible.
5. **Day-30 prompt hook**: same mechanism, scheduled relative to first publish.

Dashboards / alerts deferred — slice-01 ships with local telemetry only; aggregated
dashboards become meaningful only after slice-05 (AppView).
