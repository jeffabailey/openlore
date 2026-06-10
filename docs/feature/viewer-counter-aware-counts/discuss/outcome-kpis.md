# Outcome KPIs: viewer-counter-aware-counts (slice-18)

> slice-18 mints **NO new KPI ID**. Like slices 08–17 it REALIZES inherited KPIs on a new
> facet (the disputed-claim awareness count on the `/` landing summary + the `/claims`
> header). Cross-feature SSOT: `docs/product/kpi-contracts.yaml`.

## Feature: viewer-counter-aware-counts

### Objective

When the operator orients at the front door (or lands on `/claims`), she sees at a glance not
just how much is in her store but how much of her own work has been DISPUTED — connecting the
shipped counter-flag family (slices 11–14) to the front-door orientation (slice-17),
completing the "see what's in my store" picture with "and what's been countered."

### Realized / inherited KPIs

| # | KPI ID | Who | Does What | By How Much | Baseline | Type |
|---|---|---|---|---|---|---|
| 1 | **KPI-VIEW-1** (time-to-see-store-contents — now including disputed-claim state) | P-001 operators opening the viewer | on opening `/`, immediately see how many own claims are countered, without leaving the front door | a measurable share, when the countered count is non-zero, drill into `/claims` to read a contested claim in the same session | today the operator must leave `/` and scan `/claims` (or drill each claim) to learn how much of her work is disputed | leading |
| 2 | **KPI-VIEW-2** (read-only, guardrail) | the viewer process | adds no write/sign/subscribe/follow path; the countered count is render-only | 0 write/sign paths; 0 key reads | MET (slices 06–17) | guardrail |
| 3 | **KPI-5 / KPI-VIEW-5 / KPI-HX-G2** (local-first / offline / no-CDN, guardrails) | the viewer process | resolves the countered count as a LOCAL aggregate; both surfaces render offline | 0 network seams on `/` and `/claims`; references only the vendored htmx asset | MET (slices 06–17) | guardrail |
| 4 | **KPI-FED-3** (counter-claim publication rate — north star, READ-side strengthening) | P-001 operators | the front-door + list-header countered awareness surfaces disputed-claim state earlier, feeding the read side of the counter loop | leading: counter-aware orientation precedes drill-in (no new ID; strengthens the READ side) | the slice-11/12 read-side strengthening continues | leading (secondary) |

> Per slice-11/12 precedent, the counter-awareness surfaces STRENGTHEN the READ side of the
> KPI-FED-3 loop (users cannot engage with disagreement they cannot SEE). No new KPI ID.

### Metric Hierarchy

- **North Star (realized)**: KPI-VIEW-1 — time-to-see-store-contents, now extended to include
  disputed-claim state at the front door.
- **Leading Indicators**: share of sessions where a non-zero countered count on `/` precedes a
  drill-in to a contested claim on `/claims` in the same session.
- **Guardrail Metrics (must NOT degrade)**: KPI-VIEW-2 (read-only / no key); KPI-5 /
  KPI-VIEW-5 (local-first / offline); the no-N+1 read budget (the landing's fixed-read budget
  grows by at most 1); the J-003b accuracy cardinal (presence count, never a re-weight / "by
  N" total); the `/claims` list no-regression (order/paging/count/confidence byte-identical to
  slice-06 / slice-12).

### Measurement Plan

| KPI | Data Source | Collection Method | Frequency | Owner |
|---|---|---|---|---|
| KPI-VIEW-1 (counter-aware facet) | per-feature acceptance GREEN + opt-in telemetry (ADR-010) | the landing/header countered count renders (happy / honest-zero / counted-once / missing-not-zero); cohort: non-zero count → drill-in | per release + cohort | nw-product-owner |
| KPI-VIEW-2 | route inventory + key-access audit + StoreReadPort no-mutation type + xtask viewer rule + behavioral gold | every CI run (release-blocking) | per release | platform-architect |
| KPI-5 / KPI-VIEW-5 | offline render gold (no CDN, no network seam) | every CI run | per release | platform-architect |
| no-N+1 read budget | `@property`/gold acceptance test (countered-count reads invariant to store size) | every CI run | acceptance-designer |
| landing==header consistency | gold acceptance test (same store → same "(N countered)" on both surfaces) | every CI run | acceptance-designer |

### Hypothesis

We believe that surfacing the **countered-own-claims count** beside the own-claims count on
the `/` landing summary and in the `/claims` header (P-001, counter-aware-orientation hat)
will increase the share of dogfood sessions where the operator, on orienting, immediately
knows how much of her own work has been disputed and decides whether to read the
disagreements first — because the front door now answers "what's been pushed back on?"
alongside "what's here?". We will know this is true when, post-slice-18, opt-in telemetry +
dogfood reports show operators who see a non-zero "(N countered)" on `/` navigate to a
contested claim on `/claims` in the same session — closing the gap between the shipped
counter-flag family and the front-door orientation, without ever re-weighting a claim or
showing a verdict.

### Handoff to DEVOPS (platform-architect)

- **Data collection**: instrument the landing/header countered-count render (rendered vs
  missing-marker) and the non-zero-count → `/claims` drill-in navigation (opt-in, ADR-010).
- **Guardrail alerting**: any reachable write/sign path, any key read, any non-loopback bind,
  any CDN reference, or any countered-count read that grows with store size (N+1) is a
  release blocker.
- **Baseline**: none new to collect — the countered count is derived per-request from the
  existing LOCAL counter-reference tables; KPI-VIEW-1 cohort timing rides the inherited
  cold-start-to-first-paint hook.
