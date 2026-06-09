# Outcome KPIs: viewer-search-follow-state (slice-16)

> Job: **J-005c** · Owner: Luna (nw-product-owner) · 2026-06-09
> slice-16 mints **NO new KPI ID**. Like slice-08–15 it REALIZES inherited KPIs on a new
> facet (the `/search` follow-state accuracy). Cross-feature SSOT:
> `docs/product/kpi-contracts.yaml`.

## Feature: viewer-search-follow-state

### Objective

Make the `/search` follow affordance ACCURATE — show "Following" for a developer the operator
already follows and offer `openlore peer add` ONLY for genuinely-new authors — so discovery
becomes a clean front-door to growing the trusted local graph (J-005c) instead of re-offering
follows the operator has already made.

### Inherited KPIs realized on the `/search` follow-state facet

| # | Inherited KPI | Who | Does What | Realized how on `/search` | Type |
|---|---|---|---|---|---|
| 1 | **KPI-AV-4** (Discovery→federation funnel — the J-005c north-star) | P-001 operators | follow a genuinely-NEW discovered author via the CLI | the `peer add` affordance is shown ONLY for unfollowed authors (0% re-offered to already-followed authors), so the funnel's front-door is accurate and uncluttered | Leading |
| 2 | **KPI-VIEW-2** (Read-only viewer — guardrail) | the viewer process | never mutates the follow graph | both affordances are render-only TEXT; no follow/unfollow control; no key | Guardrail |
| 3 | **KPI-AV-2 / KPI-FED-1/2** (Anti-merging — guardrail) | the viewer process | keeps every result per-author, unranked-by-relationship | relationship is a per-row enrichment; grouping + order unchanged vs slice-08 | Guardrail |
| 4 | **KPI-5 / KPI-AV per-user-neutral boundary** (local-first / index neutrality — guardrail) | the viewer process | resolves the relationship LOCALLY, never telling the index who you follow | resolution reads the LOCAL active set only; the index query is unchanged + per-user-neutral | Guardrail |
| 5 | **KPI-HX-G1/G2/G3** (no-JS / offline / no-CDN — guardrail) | the viewer process | renders the follow-state offline, no-JS, no CDN | the resolution is a LOCAL read; full page without HX-Request; same fragment both shapes; vendored htmx | Guardrail |

### Metric Hierarchy

- **North Star (inherited)**: KPI-AV-4 — the discovery→federation funnel (J-005c). slice-16
  STRENGTHENS its accuracy: discovery surfaces a follow affordance only where it is
  actionable, so a `peer add` shown is a `peer add` that can be meaningfully run.
- **Leading Indicators**: a measurable share of `/search` sessions surface ≥1 already-followed
  author shown as "Following" (proof the resolution fires); the search→`peer add` funnel
  carries fewer wasted (already-followed) affordances.
- **Guardrail Metrics (must NOT degrade)**: read-only (KPI-VIEW-2), anti-merging
  (KPI-AV-2 / KPI-FED-1/2), local-first / index-neutral (KPI-5), htmx no-regression
  (KPI-HX-G1/G2/G3), single batch read (no N+1).

### Per-story success criteria (tied to the inherited KPIs)

| Story | Who | Does what | By how much | Measured by | Baseline |
|---|---|---|---|---|---|
| US-SF-001 (infra) | the viewer process | resolves every result author's relationship against the LOCAL active set | exactly 1 active-set read per render (0 N+1), invariant to result count; 100% of followed authors resolved `SubscribedPeer` | behavioral assertion via the real `openlore ui` subprocess (read-count invariant; seeded followed author resolves `SubscribedPeer`) | every author hardcoded `NetworkUnfollowed` today (0% accurate) |
| US-SF-002 | P-001 operators discovering on `/search` | distinguish already-followed authors ("Following") from new ones (`peer add`) and follow the new ones via CLI | leading indicator OF KPI-AV-4 — the `peer add` affordance shown ONLY where actionable (0% re-offered to followed authors) | per-feature GREEN (followed → "Following" + no add; unfollowed → add command); cohort via opt-in telemetry (ADR-010) | today 100% of followed authors are wrongly re-offered a follow |

### Measurement Plan

| KPI | Data Source | Collection Method | Frequency | Owner |
|---|---|---|---|---|
| KPI-AV-4 (funnel accuracy) | per-feature acceptance suite + opt-in telemetry (ADR-010) | GREEN gate (followed shown "Following", unfollowed keeps add) + search→`peer add` funnel quality | per-release + day-30 | DEVOPS (platform-architect) |
| Guardrails (read-only / anti-merging / index-neutral / offline / single-read) | acceptance suite | per-release GREEN gate (release-blocking) | per-release | DISTILL (acceptance-designer) |

### Hypothesis

We believe that resolving each `/search` result author's relationship against the operator's
LOCAL active subscriptions — showing "Following" for an already-followed author and the
`openlore peer add <did>` command ONLY for genuinely-new authors (P-001, network-discovery
hat) — will increase the accuracy and signal of the discovery→federation funnel, because the
follow affordance stops being noise on developers the operator already follows. We will know
this is true when, post-slice-16, the `/search` surface never re-offers a follow to an
already-followed author (per-feature GREEN) and opt-in telemetry shows the search→`peer add`
funnel carries fewer redundant affordances (KPI-AV-4 quality).

### Smell-test pass

- Measurable today? YES — per-feature GREEN measures the resolution + the affordance choice;
  cohort behavior pends the inherited opt-in telemetry (ADR-010), same as slice-08–15.
- Rate not total? The leading indicators are rates (share of sessions with a resolved
  "Following"; funnel-accuracy ratio), not gross counts.
- Outcome not output? YES — the behavior change is "the operator distinguishes followed from
  new discovered authors and follows the new ones", not "shipped the resolution".
- Has baseline? YES — today 0% of followed authors are recognized (all hardcoded
  `NetworkUnfollowed`).
- Team can influence? YES — the team builds the LOCAL resolution + the render directly.
- Has guardrails? YES — read-only, anti-merging, index-neutral, offline, single-read (all
  release-blocking).

### Handoff to DEVOPS (platform-architect)

- **Instrument**: `/search` follow-state outcomes (followed-shown-Following vs unfollowed-
  shown-add); the search→`peer add` funnel quality (opt-in, ADR-010).
- **Dashboards**: KPI-AV-4 funnel accuracy on the `/search` facet.
- **Alerting thresholds**: guardrail breaches (any follow/unfollow control on `/search`; any
  N+1 subscription query; any relationship resolution leaking follow-state to the index; any
  re-rank/merge) are release-blocking, not alert-only.
- **Baseline collection**: none new required — baseline is "0% of followed authors recognized"
  (the slice-08 hardcoded `NetworkUnfollowed`).
