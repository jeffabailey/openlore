# Outcome KPIs: viewer-search-full-follow-state (slice-20)

> Job: **J-005c** · Owner: Luna (nw-product-owner) · 2026-06-11
> slice-20 mints **NO new KPI ID**. Like slice-08–19 it REALIZES inherited KPIs on a new
> facet (the four-arm `/search` follow-state completeness — `You` + `UnsubscribedCache`).
> Cross-feature SSOT: `docs/product/kpi-contracts.yaml`.

## Feature: viewer-search-full-follow-state

### Objective

COMPLETE the `/search` follow affordance to its full four honest states — show a neutral self
indicator for the operator's OWN claim, a neutral residue indicator for a peer she soft-removed
(cached), the slice-16 "Following" for a followed peer, and `openlore peer add` ONLY for a
genuinely-new author — so discovery stays a clean front-door to growing the trusted local graph
(J-005c) and the only follow affordance shown is one the operator could meaningfully run.

### Inherited KPIs realized on the four-arm `/search` follow-state facet

| # | Inherited KPI | Who | Does What | Realized how on `/search` | Type |
|---|---|---|---|---|---|
| 1 | **KPI-AV-4** (Discovery→federation funnel — the J-005c north-star) | P-001 operators | follow a genuinely-NEW discovered author via the CLI | the `peer add` affordance is shown ONLY for genuinely-new authors (0% re-offered to own claims OR soft-removed peers' cached claims, on top of slice-16's 0% to followed peers), so the funnel's front-door is fully accurate | Leading |
| 2 | **KPI-VIEW-2** (Read-only viewer — guardrail) | the viewer process | never mutates the follow graph | all four affordances are render-only TEXT; no control; no key | Guardrail |
| 3 | **KPI-AV-2 / KPI-FED-1/2** (Anti-merging — guardrail) | the viewer process | keeps every result per-author, unranked-by-relationship | each arm is a per-row enrichment; grouping + order unchanged | Guardrail |
| 4 | **KPI-5 / KPI-AV per-user-neutral boundary** (local-first / index neutrality — guardrail) | the viewer process | resolves all four arms LOCALLY, never telling the index who you are, follow, or removed | resolution reads three LOCAL sets only; the index query is unchanged + per-user-neutral | Guardrail |
| 5 | **KPI-HX-G1/G2/G3** (no-JS / offline / no-CDN — guardrail) | the viewer process | renders all four follow-states offline, no-JS, no CDN | LOCAL resolution; full page without HX-Request; same fragment both shapes; vendored htmx | Guardrail |
| 6 | **No-regression (slice-16 I-SF-4 facet — guardrail)** | the viewer process | preserves the slice-16 `SubscribedPeer`/`NetworkUnfollowed` rendering byte-stable | the two new arms only ADD; the slice-16 rows render identically | Guardrail |

### Metric Hierarchy

- **North Star (inherited)**: KPI-AV-4 — the discovery→federation funnel (J-005c). slice-20
  COMPLETES its accuracy: discovery surfaces a follow affordance only where it is actionable
  (never on the operator's own claims or her removed-peer residue), so a `peer add` shown is a
  `peer add` that can be meaningfully run.
- **Leading Indicators**: a measurable share of `/search` sessions surface ≥1 own-claim row (shown
  the self indicator) and/or ≥1 removed-peer cached row (shown the residue indicator) — proof the
  two new arms fire; the search→`peer add` funnel carries fewer non-actionable (own / removed)
  affordances.
- **Guardrail Metrics (must NOT degrade)**: read-only (KPI-VIEW-2), anti-merging
  (KPI-AV-2 / KPI-FED-1/2), local-first / index-neutral (KPI-5), htmx no-regression
  (KPI-HX-G1/G2/G3), slice-16 follow-state no-regression (I-SF-4), batch reads (no N+1).

### Per-story success criteria (tied to the inherited KPIs)

| Story | Who | Does what | By how much | Measured by | Baseline |
|---|---|---|---|---|---|
| US-FS-001 (infra) | the viewer process | resolves every result author's relationship to the full four-arm `AuthorRelationship` against three LOCAL sets | each LOCAL set read AT MOST 1× per render (0 N+1), invariant to result count; 100% of own-author results resolved `You`; 100% of cached-but-inactive results resolved `UnsubscribedCache` | behavioral assertion via the real `openlore ui` subprocess (read-count invariant; seeded own claim → `You`; seeded soft-removed peer's cached claim → `UnsubscribedCache`) | own claims + soft-removed peers' cached claims both resolve `NetworkUnfollowed` today (0% accurate for these two states) |
| US-FS-002 | P-001 operators discovering on `/search` | distinguish the four honest states (self / Following / residue / `peer add`) and follow only the genuinely-new authors via CLI | leading indicator OF KPI-AV-4 — the `peer add` affordance shown ONLY where actionable (0% re-offered to own claims or removed-peer residue); the slice-16 states byte-stable | per-feature GREEN (own → self + no add; removed → residue + no add; followed → "Following" unchanged; new → add unchanged); cohort via opt-in telemetry (ADR-010) | today (post-slice-16) 100% of own claims and removed-peer cached claims are wrongly re-offered a follow |

### Measurement Plan

| KPI | Data Source | Collection Method | Frequency | Owner |
|---|---|---|---|---|
| KPI-AV-4 (funnel accuracy) | per-feature acceptance suite + opt-in telemetry (ADR-010) | GREEN gate (own → self, removed → residue, followed → "Following", new → add) + search→`peer add` funnel quality | per-release + day-30 | DEVOPS (platform-architect) |
| Guardrails (read-only / anti-merging / index-neutral / offline / no-regression / batch-read) | acceptance suite | per-release GREEN gate (release-blocking) | per-release | DISTILL (acceptance-designer) |

### Hypothesis

We believe that completing the `/search` follow-state to its full four arms — a neutral self
indicator for the operator's own claim, a neutral residue indicator for a soft-removed peer's
cached claim, the slice-16 "Following" for a followed peer, and the `openlore peer add` command
ONLY for a genuinely-new author (P-001, network-discovery hat) — will further increase the accuracy
and signal of the discovery→federation funnel, because the follow affordance stops being noise on
the operator's own claims and on peers she deliberately removed. We will know this is true when,
post-slice-20, the `/search` surface never re-offers a follow to an own claim or a removed-peer
cached claim (per-feature GREEN), the slice-16 followed/unfollowed rendering is byte-stable
(no-regression GREEN), and opt-in telemetry shows the search→`peer add` funnel carries only
actionable affordances (KPI-AV-4 quality).

### Smell-test pass

- Measurable today? YES — per-feature GREEN measures the four-arm resolution + the affordance
  choice + no-regression; cohort behavior pends the inherited opt-in telemetry (ADR-010), same as
  slice-08–19.
- Rate not total? The leading indicators are rates (share of sessions surfacing a self/residue
  indicator; funnel-accuracy ratio), not gross counts.
- Outcome not output? YES — the behavior change is "the operator distinguishes the four honest
  states and follows only the genuinely-new authors", not "shipped the resolution".
- Has baseline? YES — today (post-slice-16) 0% of own claims and removed-peer cached claims are
  recognized (both resolve `NetworkUnfollowed`).
- Team can influence? YES — the team builds the two LOCAL presence reads + the precedence
  resolution + the two render arms directly.
- Has guardrails? YES — read-only, anti-merging, index-neutral, offline, slice-16 no-regression,
  batch reads (all release-blocking).

### Handoff to DEVOPS (platform-architect)

- **Instrument**: `/search` four-arm follow-state outcomes (own → self, removed → residue,
  followed → Following, new → add); the search→`peer add` funnel quality (opt-in, ADR-010).
- **Dashboards**: KPI-AV-4 funnel accuracy on the `/search` four-arm facet.
- **Alerting thresholds**: guardrail breaches (any follow/unfollow control on `/search`; any N+1
  presence query; any resolution leaking own-DID / follow-state / removed-peer state to the index;
  any re-rank/merge; any slice-16 follow-state regression) are release-blocking, not alert-only.
- **Baseline collection**: none new required — baseline is "0% of own claims and removed-peer
  cached claims recognized" (both resolve `NetworkUnfollowed` post-slice-16).
