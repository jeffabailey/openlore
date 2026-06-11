# Outcome KPIs: viewer-peer-counter-aware-counts (slice-19)

> slice-19 mints **NO new KPI ID**. Like slices 08–18 it REALIZES inherited KPIs on a new
> facet (the disputed-PEER-claim awareness count on the `/` landing peer line + the
> `/peer-claims` header). Cross-feature SSOT: `docs/product/kpi-contracts.yaml`.

## Feature: viewer-peer-counter-aware-counts

### Objective

When the operator orients at the front door (or lands on `/peer-claims`), she sees at a glance
not just how many peer claims she has cached but how many of THOSE have been DISPUTED —
completing the counter-aware orientation slice-18 began for her OWN claims, so the front door
now answers "what's been pushed back on?" across BOTH own and peer claims.

### Realized / inherited KPIs

| # | KPI ID | Who | Does What | By How Much | Baseline | Type |
|---|---|---|---|---|---|---|
| 1 | **KPI-VIEW-1** (time-to-see-store-contents — now disputed-claim state across own AND peer) | P-001 operators opening the viewer | on opening `/`, immediately see how many cached peer claims are countered (completing own+peer orientation), without leaving the front door | a measurable share, when the peer countered count is non-zero, drill into `/peer-claims` to read a contested peer claim in the same session | today `/` shows the own disputed count (slice-18) but the peer line is bare; the operator must leave `/` and scan `/peer-claims` to learn how much cached peer material is disputed | leading |
| 2 | **KPI-VIEW-2** (read-only, guardrail) | the viewer process | adds no write/sign/subscribe/follow path; the countered-peer count is render-only | 0 write/sign paths; 0 key reads | MET (slices 06–18) | guardrail |
| 3 | **KPI-5 / KPI-VIEW-5 / KPI-HX-G2** (local-first / offline / no-CDN, guardrails) | the viewer process | resolves the countered-peer count as a LOCAL aggregate; both surfaces render offline | 0 network seams on `/` and `/peer-claims`; references only the vendored htmx asset | MET (slices 06–18) | guardrail |
| 4 | **KPI-FED-3** (counter-claim publication rate — north star, READ-side strengthening) | P-001 operators | the front-door + peer-list-header countered awareness surfaces disputed cached-peer-claim state earlier, feeding the read side of the counter loop | leading: counter-aware orientation across own+peer precedes drill-in (no new ID; strengthens the READ side) | the slice-11/12/18 read-side strengthening continues | leading (secondary) |

> Per slice-11/12/18 precedent, the counter-awareness surfaces STRENGTHEN the READ side of the
> KPI-FED-3 loop (users cannot engage with disagreement they cannot SEE). No new KPI ID.

### Metric Hierarchy

- **North Star (realized)**: KPI-VIEW-1 — time-to-see-store-contents, now extended to include
  disputed-claim state across BOTH own (slice-18) AND peer (slice-19) claims at the front door.
- **Leading Indicators**: share of sessions where a non-zero peer countered count on `/`
  precedes a drill-in to a contested peer claim on `/peer-claims` in the same session.
- **Guardrail Metrics (must NOT degrade)**: KPI-VIEW-2 (read-only / no key); KPI-5 /
  KPI-VIEW-5 (local-first / offline); the no-N+1 read budget (the landing's fixed-read budget
  grows by exactly 1 — a 5th count read); the J-003b accuracy cardinal (presence count, never a
  re-weight / "by N" total); the `/peer-claims` list no-regression (order/paging/count/
  confidence/origin byte-identical to slice-06/07 / slice-13); the slice-18 own-claims
  countered surfaces (landing + `/claims` header) UNTOUCHED.

### Measurement Plan

| KPI | Data Source | Collection Method | Frequency | Owner |
|---|---|---|---|---|
| KPI-VIEW-1 (peer counter-aware facet) | per-feature acceptance GREEN + opt-in telemetry (ADR-010) | the landing/header peer countered count renders (happy / honest-zero / counted-once / missing-not-zero); cohort: non-zero count → drill-in | per release + cohort | nw-product-owner |
| KPI-VIEW-2 | route inventory + key-access audit + StoreReadPort no-mutation type + xtask viewer rule + behavioral gold | every CI run (release-blocking) | per release | platform-architect |
| KPI-5 / KPI-VIEW-5 | offline render gold (no CDN, no network seam) | every CI run | per release | platform-architect |
| no-N+1 read budget | `@property`/gold acceptance test (countered-peer-count reads invariant to store size) | every CI run | acceptance-designer |
| landing==header consistency | gold acceptance test (same store → same "(N countered)" on the landing peer line + the `/peer-claims` header) | every CI run | acceptance-designer |
| slice-18 own surfaces untouched | gold acceptance test (the `/claims` + landing own line still render "(N countered)" unchanged) | every CI run | acceptance-designer |

### Hypothesis

We believe that surfacing the **countered-peer-claims count** beside the peer-claims count on
the `/` landing summary and in the `/peer-claims` header (P-001, counter-aware-orientation hat)
will increase the share of dogfood sessions where the operator, on orienting, immediately knows
how much of her cached peer material has been disputed and decides whether to read those
disagreements first — because the front door now answers "what's been pushed back on?" across
BOTH own and peer claims (completing slice-18). We will know this is true when, post-slice-19,
opt-in telemetry + dogfood reports show operators who see a non-zero "(N countered)" on the peer
line of `/` navigate to a contested peer claim on `/peer-claims` in the same session — closing
the remaining half of the gap between the shipped counter-flag family and the front-door
orientation, without ever re-weighting a claim or showing a verdict.

### Handoff to DEVOPS (platform-architect)

- **Data collection**: instrument the landing/header peer countered-count render (rendered vs
  missing-marker) and the non-zero-count → `/peer-claims` drill-in navigation (opt-in, ADR-010).
- **Guardrail alerting**: any reachable write/sign path, any key read, any non-loopback bind,
  any CDN reference, or any countered-peer-count read that grows with store size (N+1) is a
  release blocker.
- **Baseline**: none new to collect — the countered-peer count is derived per-request from the
  existing LOCAL counter-reference tables; KPI-VIEW-1 cohort timing rides the inherited
  cold-start-to-first-paint hook.
