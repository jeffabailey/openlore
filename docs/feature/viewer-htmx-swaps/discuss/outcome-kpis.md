# Outcome KPIs: viewer-htmx-swaps (slice-07)

> **DELTA on slice-06.** The slice removes the per-click full-page-reload jolt from the four
> viewer interactions while keeping the slice-06 guarantees intact. KPIs are framed as the
> observable *behavior of the response shape* (swap vs reload, fragment vs full page) plus
> bytes-transferred where it is the clearest proxy for the in-place win — all measurable by
> sending/withholding the `HX-Request` header against the real `openlore ui`, with no new
> persisted data. Prefix: **`KPI-HX-`**.

## Feature: viewer-htmx-swaps

### Objective

Make navigating the local viewer feel like moving around one steady place — paging, scraping,
opening a claim, and switching views update in place, not by reloading — without sacrificing
the slice-06 promises that the viewer is read-only, works offline, and works with JavaScript
off.

### Outcome KPIs

| # | Who | Does What | By How Much | Baseline | Measured By | Type |
|---|-----|-----------|-------------|----------|-------------|------|
| KPI-HX-1 | Node operator paging a list | Pages `/claims` and `/peer-claims` with an in-place table swap instead of a full reload | 100% of htmx Prev/Next requests return a table-only fragment (not a full page); non-htmx return the full page; payload of an htmx page request is a fraction of the full-page bytes (table region only, no chrome) | slice-06: every Prev/Next is a full-page reload (whole document re-sent) | HX-Request-present vs absent response shape against the real `openlore ui`; bytes-transferred per request (fragment vs full page) | Leading |
| KPI-HX-2 | Node operator triaging a scrape | Submits a target and reviews proposals via an in-place results swap | 100% of htmx `POST /scrape` requests return a results-only fragment (form preserved); non-htmx return the full page; no sign control, nothing persisted in either shape | slice-06: every submit reloads the whole `/scrape` page | response shape + a route audit confirming no sign control and no persisted candidate | Leading |
| KPI-HX-3 | Node operator inspecting a claim | Opens a claim's detail inline without leaving the list | 100% of htmx `GET /claims/{cid}` requests return a detail-panel fragment (list preserved); non-htmx return the full detail page | slice-06: opening a claim navigates away from the list | response shape against the real `openlore ui` | Leading (secondary) |
| KPI-HX-4 | Federated node operator | Switches My Claims ↔ Peer Claims with an in-place view-panel swap, URL updated | 100% of htmx tab switches return a view-panel fragment AND update the URL so the view is bookmarkable / Back works; non-htmx return the full page | slice-06: every tab switch is a full-page reload | response shape + URL/history assertion after a swap | Leading (secondary) |

### Guardrail KPIs (must NOT degrade — release-relevant)

| # | Guardrail | Threshold | Measured By | Status target |
|---|-----------|-----------|-------------|---------------|
| KPI-HX-G1 (**no-JS no regression**) | Every route serves a complete slice-06 full page when `HX-Request` is absent; non-htmx responses are byte-equivalent to slice-06 | 100% of routes; slice-06 26-scenario corpus GREEN | withhold HX-Request per route + run the slice-06 acceptance corpus | MET (release-blocking) |
| KPI-HX-G2 (**offline**) | Every store view AND every swap works with the network down; no page references a CDN for htmx | 100% offline; 0 CDN references | offline harness (network down) + a property scan of served HTML for off-host htmx URLs | MET (release-blocking) |
| KPI-HX-G3 (**read-only / no new write surface**) | No swap adds a write/sign route; the web process holds no signing key; bind stays loopback-only | 0 new write/sign routes; 0 key reads; loopback only | route inventory + key-access audit (carries slice-06 KPI-VIEW-2) | MET (release-blocking) |

### Metric Hierarchy

- **North Star**: KPI-HX-1 — paging (the most-used, highest-pain interaction on a real-sized
  store) becomes an in-place swap. It is the heart of "navigate without reloads" and the
  walking-skeleton outcome.
- **Leading Indicators**: KPI-HX-2 (scrape in place), KPI-HX-3 (detail inline), KPI-HX-4 (tab
  switch in place) — each removes one more reload from the journey.
- **Guardrail Metrics (must not degrade)**: KPI-HX-G1 (no-JS no-regression), KPI-HX-G2
  (offline), KPI-HX-G3 (read-only / no new write surface). A breach of any guardrail is a
  release blocker — they encode the load-bearing progressive-enhancement + offline + read-only
  contracts.

### Measurement Plan

| KPI | Data Source | Collection Method | Frequency | Owner |
|-----|------------|-------------------|-----------|-------|
| KPI-HX-1 | real `openlore ui` over HTTP | request the route WITH and WITHOUT HX-Request; assert fragment shape vs full page; compare bytes-transferred | per-feature (acceptance) + DEVOPS sweep | acceptance suite (DISTILL) / platform-architect (DEVOPS) |
| KPI-HX-2 | real `openlore ui` | POST /scrape with/without HX-Request; assert results-only fragment + no sign control + no persist | per-feature | acceptance suite |
| KPI-HX-3 | real `openlore ui` | GET /claims/{cid} with/without HX-Request; assert detail fragment vs full page | per-feature | acceptance suite |
| KPI-HX-4 | real `openlore ui` | tab switch with/without HX-Request; assert view-panel fragment + URL update | per-feature | acceptance suite |
| KPI-HX-G1 | real `openlore ui` + slice-06 corpus | withhold HX-Request per route; run slice-06 26 scenarios | per-feature (blocking) | acceptance suite |
| KPI-HX-G2 | offline harness + HTML scan | network down → every view+swap works; scan served HTML for off-host htmx URL | per-feature (blocking) | acceptance suite / DEVOPS |
| KPI-HX-G3 | route inventory + key audit | enumerate routes (no new write/sign); audit key reads (zero); assert loopback bind | per-feature (blocking) | acceptance suite / DEVOPS |

### Hypothesis

We believe that **layering htmx partial-swaps onto the existing read-only viewer routes** for
**the node operator** will achieve **in-place updates on all four interactions without
sacrificing the no-JS, offline, and read-only guarantees**. We will know this is true when
**the operator's Prev/Next, scrape submit, claim open, and tab switch return content-region
fragments under `HX-Request` (and full slice-06 pages without it) in 100% of cases**, while
**the offline harness, the no-JS full-page check, and the read-only route+key audit all stay
green**.

### Handoff to DEVOPS (instrumentation notes)

- **Data collection**: per-route response-shape signal (fragment vs full page keyed on
  HX-Request) + bytes-transferred per request (the cleanest proxy for the in-place win on
  KPI-HX-1). No new persisted data; no user PII; loopback only.
- **Dashboards/reports**: the guardrails (G1/G2/G3) want a release-gate report (pass/fail),
  not a real-time dashboard — they are binary contracts. KPI-HX-1 bytes-transferred can be a
  per-release comparison (htmx page request vs full-page request).
- **Alerting thresholds**: any new write/sign route, any key read, any non-loopback bind, any
  CDN reference, or any slice-06 corpus regression is a release blocker (G1/G2/G3).
- **Baseline**: slice-06 is the baseline for all four — every interaction is a full-page
  reload today; the full-page byte sizes per route are the KPI-HX-1 comparison baseline.
