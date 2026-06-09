# Outcome KPIs: viewer-landing-dashboard (slice-17)

> Job: **J-002** (orientation facet) · Owner: Luna (nw-product-owner) · 2026-06-09
> slice-17 mints **NO new KPI ID**. Like slice-08–16 it REALIZES inherited KPIs on a new
> facet (the `/` front-door dashboard + navigation hub). Cross-feature SSOT:
> `docs/product/kpi-contracts.yaml`.

## Feature: viewer-landing-dashboard

### Objective

Make the viewer's FRONT DOOR (`GET /`) the place the operator orients her whole session —
seeing what's in her LOCAL store at a glance and reaching every shipped surface — so the
11-surface viewer becomes a coherent, navigable app from its own entry point (closing the
discoverability gap; realizing KPI-VIEW-1 as the front door).

### Inherited KPIs realized on the `/` front-door facet

| # | Inherited KPI | Who | Does What | Realized how on `/` | Type |
|---|---|---|---|---|---|
| 1 | **KPI-VIEW-1** (Time-to-see-store-contents) | P-001 operators | answer "what's in my store?" the moment they open the viewer | the LOCAL store summary (own claims, peer claims, active peers) renders at the front door — minimal time-to-orient, at the very first surface | Leading |
| 2 | **KPI-VIEW-1** (discoverability facet) | P-001 operators | reach a surface they could not find before | the navigation hub links all 8 shipped surfaces; today only `/claims` is reachable from `/` | Leading |
| 3 | **KPI-VIEW-2** (Read-only viewer — guardrail) | the viewer process | never mutates the store | `/` holds `StoreReadPort` only; no write/compose/sign/subscribe/follow control; no key | Guardrail |
| 4 | **KPI-5 / KPI-VIEW-5 / KPI-HX-G1/G2/G3** (local-first / offline / no-CDN / no-JS / read-only — guardrail) | the viewer process | renders the front door offline, no-JS, no CDN, degrading gracefully on a count failure | LOCAL DB reads; full page; vendored htmx asset; a failed count read → missing-number state, never a 5xx | Guardrail |

### Metric Hierarchy

- **North Star (inherited)**: KPI-VIEW-1 — time-to-see-store-contents. slice-17 realizes it
  AS THE FRONT DOOR: the operator sees the store summary the instant she opens `/`.
- **Leading Indicators**: operators open `/` and reach a NON-`/claims` surface (peers,
  search, score, project, philosophy) from the navigation hub in the same session
  (discoverability funnel); operators report knowing what's in their store from the front
  door without running a CLI count.
- **Guardrail Metrics (must NOT degrade)**: read-only (KPI-VIEW-2), local-first/offline
  (KPI-5 / KPI-VIEW-5), htmx no-regression (KPI-HX-G1/G2/G3), graceful-degrade (a count
  failure never 5xxes the front door), single-fixed-reads (no N+1).

### Per-story success criteria (tied to the inherited KPIs)

| Story | Who | Does what | By how much | Measured by | Baseline |
|---|---|---|---|---|---|
| US-LD-000 (infra) | the viewer process | resolves the LOCAL store summary (3 counts) for the front door | exactly 3 aggregate reads per render (0 N+1), invariant to store size; 0 of N count failures produce a 5xx | behavioral query-count + seeded-failure assertion via the real `openlore ui` subprocess | `landing_page` takes no store today (queries nothing) |
| US-LD-001 | P-001 operators | open `/`, see the store summary, navigate to a surface from the hub | leading indicator OF KPI-VIEW-1 — a measurable share open `/` and reach a NON-`/claims` surface from the hub in the same session | per-feature GREEN (summary + 8-surface hub render; failed count degrades, never 5xx); cohort via opt-in telemetry (ADR-010) | only `/claims` reachable from `/`; no store state surfaced today |

### Measurement Plan

| KPI | Data Source | Collection Method | Frequency | Owner |
|---|---|---|---|---|
| KPI-VIEW-1 ( `/` front-door facet ) | opt-in telemetry (ADR-010) | `/` open count; `/` → NON-`/claims` surface navigation funnel | weekly | DEVOPS (platform-architect) |
| Guardrails (read-only / offline / graceful-degrade / fixed-reads) | acceptance suite | per-release GREEN gate (release-blocking) | per-release | DISTILL (acceptance-designer) |

### Hypothesis

We believe that turning `/` into a LOCAL store summary (own claims, peer claims, active
peers) + a navigation hub to all 8 shipped surfaces (P-001, orientation hat) will increase
the share of dogfood users who, on opening the viewer, immediately know what's in their
store and reach a second surface — because the front door now answers "what's here?" and
"where can I go?" in one read-only view. We will know this is true when, post-slice-17,
users report (and opt-in telemetry shows) they open `/` and navigate to a NON-`/claims`
surface (peers, search, score, project, philosophy) from the hub in the same session,
closing the discoverability gap.

### Smell-test pass

- Measurable today? YES — per-feature GREEN measures the summary render + the 8-surface hub
  + the graceful-degrade behavior; cohort behavior pends the inherited opt-in telemetry
  (ADR-010), same as slice-08–16.
- Rate not total? The leading indicators are rates (share of operators opening `/`; funnel
  conversion to a non-`/claims` surface), not gross counts. (The three dashboard NUMBERS
  are aggregate store counts — display content, not the KPI metric.)
- Outcome not output? YES — the behavior change is "operator orients from the front door
  and navigates the coherent app," not "shipped the dashboard."
- Has baseline? YES — today only `/claims` is reachable from `/` and no store state is
  surfaced (baseline = single dead-end link).
- Team can influence? YES — the team builds the summary + the hub directly.
- Has guardrails? YES — read-only, offline, graceful-degrade, fixed-reads (all
  release-blocking).

### Handoff to DEVOPS (platform-architect)

- **Instrument**: `/` opens; the `/` → NON-`/claims` surface navigation funnel (opt-in,
  ADR-010).
- **Dashboards**: KPI-VIEW-1 ( `/` front-door facet ): open count + discoverability funnel.
- **Alerting thresholds**: guardrail breaches (any write surface on `/`; any count failure
  that 5xxes the front door; any N+1; any hardcoded surface link drift) are release-blocking,
  not alert-only.
- **Baseline collection**: none new required — baseline is the single-dead-end-link front
  door (no surface today).
