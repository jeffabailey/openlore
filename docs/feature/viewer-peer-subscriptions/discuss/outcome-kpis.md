# Outcome KPIs: viewer-peer-subscriptions (slice-15)

> Job: **J-003c** · Owner: Luna (nw-product-owner) · 2026-06-09
> slice-15 mints **NO new KPI ID**. Like slice-08–14 it REALIZES inherited KPIs on a new
> facet (the `/peers` federation-management view). Cross-feature SSOT:
> `docs/product/kpi-contracts.yaml`.

## Feature: viewer-peer-subscriptions

### Objective

Make the operator's federation subscription set LEGIBLE in the browser and the
residue-free revocation path VISIBLE — so subscription stops feeling like a one-way
commitment (J-003c).

### Inherited KPIs realized on the `/peers` facet

| # | Inherited KPI | Who | Does What | Realized how on `/peers` | Type |
|---|---|---|---|---|---|
| 1 | **KPI-FED-4** (Zero purge residue — J-003c sovereignty) | P-001 operators | observe a removed peer VANISH from the subscription list | a peer removed via the CLI is absent from `/peers` (active-only) — the residue-free guarantee made visible, not merely trusted | Leading |
| 2 | **KPI-VIEW-1** (Time-to-see-store-contents) | P-001 operators | answer "who do I follow?" at a glance | the active subscription set + per-peer counts render in one read-only view, zero CLI | Leading |
| 3 | **KPI-VIEW-2** (Read-only viewer — guardrail) | the viewer process | never mutates the subscription set | `/peers` holds `StoreReadPort` only; no write/subscribe/unsubscribe control; no key | Guardrail |
| 4 | **KPI-AV-2 / KPI-FED-1/2** (Anti-merging — guardrail) | the viewer process | keeps every peer + count per-peer | per-peer rows, per-peer `COUNT(*)`, never a merged total | Guardrail |
| 5 | **KPI-5 / KPI-VIEW-5 / KPI-HX-G1/G2/G3** (local-first / offline / no-CDN / no-JS — guardrail) | the viewer process | renders the view offline, no-JS, no CDN | LOCAL DB read; full page without HX-Request; vendored htmx asset | Guardrail |

### Metric Hierarchy

- **North Star (inherited)**: KPI-FED-4 — zero purge residue (the J-003c sovereignty
  signal). slice-15 strengthens its READ side: the operator can SEE the residue-free
  outcome.
- **Leading Indicators**: operators open `/peers` to review subscriptions; operators copy
  the render-only `peer remove` command and then observe the removed peer vanish on
  reload (KPI-VIEW-1 → KPI-FED-4 funnel).
- **Guardrail Metrics (must NOT degrade)**: read-only (KPI-VIEW-2), anti-merging
  (KPI-AV-2 / KPI-FED-1/2), local-first/offline (KPI-5 / KPI-VIEW-5), htmx no-regression
  (KPI-HX-G1/G2/G3), single-query (no N+1).

### Per-story success criteria (tied to the inherited KPIs)

| Story | Who | Does what | By how much | Measured by | Baseline |
|---|---|---|---|---|---|
| US-PS-001 (infra) | the viewer process | resolves active subscriptions + per-peer counts | exactly 1 aggregate query per render (0 N+1), invariant to peer count | behavioral query-count assertion via the real `openlore ui` subprocess | no peers read exists today |
| US-PS-002 | P-001 operators | review who they follow + copy the render-only `peer remove` command; observe a removed peer vanish | leading indicator OF KPI-FED-4 — a measurable share open `/peers`, copy the command, see the residue-free absence | per-feature GREEN (list + count + render-only command render; removed peer absent); cohort via opt-in telemetry (ADR-010) | no browser subscription surface today |
| US-PS-003 | P-001 operators (follow no one) | confirm "no peers" + learn how to start | leading indicator OF KPI-VIEW-1 — first-run operators reach an unambiguous "no peers + how to start" state, zero CLI inspection | per-feature GREEN (empty state + starting command render; never blank/error) | no `/peers` surface today |

### Measurement Plan

| KPI | Data Source | Collection Method | Frequency | Owner |
|---|---|---|---|---|
| KPI-FED-4 (read side) | per-feature acceptance suite + opt-in telemetry (ADR-010) | GREEN gate (removed peer absent from `/peers`) + cohort funnel | per-release + day-30 | DEVOPS (platform-architect) |
| KPI-VIEW-1 ( /peers facet ) | opt-in telemetry (ADR-010) | `/peers` open count; review→`peer remove`→reload-vanish funnel | weekly | DEVOPS |
| Guardrails (read-only / anti-merging / offline / single-query) | acceptance suite | per-release GREEN gate (release-blocking) | per-release | DISTILL (acceptance-designer) |

### Hypothesis

We believe that surfacing the active subscription set + per-peer claim counts + the
render-only `openlore peer remove <did>` command on `/peers` (P-001, subscription-manager
hat) will increase the share of dogfood users who can answer "who do I follow and how do
I leave cleanly?" without leaving the browser. We will know this is true when,
post-slice-15, users report (and opt-in telemetry shows) they open `/peers` to review
subscriptions, copy the render-only `peer remove` command, and observe the removed peer
vanish on reload — strengthening the READ side of KPI-FED-4 (the residue-free guarantee
made visible).

### Smell-test pass

- Measurable today? YES — per-feature GREEN measures the render + the active-only
  absence; cohort behavior pends the inherited opt-in telemetry (ADR-010), same as
  slice-08–14.
- Rate not total? The leading indicators are rates (share of operators opening `/peers`;
  funnel conversion), not gross counts.
- Outcome not output? YES — the behavior change is "operator reviews subscriptions and
  sees the residue-free outcome," not "shipped the `/peers` route."
- Has baseline? YES — today there is no browser subscription surface (baseline = 0
  in-browser review).
- Team can influence? YES — the team builds the legible view and the residue-made-visible
  guarantee directly.
- Has guardrails? YES — read-only, anti-merging, offline, single-query (all
  release-blocking).

### Handoff to DEVOPS (platform-architect)

- **Instrument**: `/peers` opens; the review→`peer remove`→reload-vanish funnel (opt-in,
  ADR-010).
- **Dashboards**: KPI-VIEW-1 ( /peers facet ); KPI-FED-4 read-side funnel.
- **Alerting thresholds**: guardrail breaches (any write surface on `/peers`; any N+1;
  any soft-removed peer leaking into the list) are release-blocking, not alert-only.
- **Baseline collection**: none new required — baseline is 0 (no surface today).
