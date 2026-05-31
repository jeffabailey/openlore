# Prioritization: htmx-scraper-viewer (slice-06)

Prioritized by outcome impact (opportunity scores) and dependency, not by effort or
feature grouping. Job 1 (store inspection, opportunity 15) precedes Job 2 (live-scrape
browsing, opportunity 7). See `jtbd-opportunity-scores.md`.

## Release Priority

| Priority | Release | Target Outcome | KPI | Value×Urgency/Effort | Rationale |
|----------|---------|----------------|-----|----------------------|-----------|
| 1 | Walking Skeleton | End-to-end thread works: HTTP → DuckDB query → HTML list in browser, read-only, offline | KPI-VIEW-1 (seed), KPI-VIEW-2 | 5×5/2 = 12.5 | Validates the riskiest assumption: a read-only web process can serve the local store with no key. Skeleton always first. |
| 2 | R1 — See my store at a glance | Operator views their persisted claims (incl. one claim's full evidence) in a browser in <10s, zero SQL | KPI-VIEW-1, KPI-VIEW-2 | 5×4/2 = 10 | North-star job (opportunity 15). Highest-value behavior change. |
| 3 | R2 — Navigate large/federated store | Operator inspects both own + federated peer claims, distinguishes them, in a real-sized store | KPI-VIEW-3, KPI-VIEW-5 | 4×3/3 = 4 | Same north-star job; required for real stores; depends on R1 rendering. |
| 4 | R3 — Triage scrape proposals | Operator reviews live proposals in browser, then signs in CLI | KPI-VIEW-4 | 3×2/3 = 2 | Secondary job (opportunity 7); CLI already serves it functionally; reuses R1/R2 foundation. |
| 5 | R4 — Fluid big-store UX (future) | Reduce friction (sort/filter/pagination polish) | (supports KPI-VIEW-1/3) | 2×1/3 = 0.7 | Could-have polish; no new outcome. Ship only if capacity allows. |

Tie-break order applied: Walking Skeleton > Riskiest Assumption > Highest Value.

## Backlog Suggestions

> Story IDs assigned in Phase 4 (`user-stories.md`). WS = Walking Skeleton.

| Story | Release | Priority | Outcome Link | Job | Dependencies |
|-------|---------|----------|--------------|-----|--------------|
| US-VIEW-001 | WS | P1 | KPI-VIEW-1, KPI-VIEW-2 | Job 1 | adapter-duckdb (exists), local `claims` (slice-01) |
| US-VIEW-002 | R1 | P2 | KPI-VIEW-1, KPI-VIEW-2 | Job 1 | US-VIEW-001 |
| US-VIEW-003 | R1 | P2 | KPI-VIEW-1 | Job 1 | US-VIEW-001 |
| US-VIEW-004 | R2 | P3 | KPI-VIEW-3, KPI-VIEW-5 | Job 1 | US-VIEW-001, `peer_claims` (slice-03) |
| US-VIEW-005 | R2 | P3 | KPI-VIEW-1 (scale) | Job 1 | US-VIEW-002 |
| US-VIEW-006 | R3 | P4 | KPI-VIEW-4 | Job 2 | US-VIEW-001, slice-02 propose pipeline |

## MoSCoW

- **Must have**: US-VIEW-001 (WS), US-VIEW-002, US-VIEW-003 (Job 1 core legibility).
- **Should have**: US-VIEW-004, US-VIEW-005 (Job 1 at real scale + federated dimension).
- **Could have**: US-VIEW-006 (Job 2 browser triage — CLI already covers the capability).
- **Won't have (this slice)**: any write/sign action from the web surface; public/non-localhost
  exposure; the slice-05 appview/indexer network graph; sort/filter polish (R4).

## Risk-driven note

The riskiest assumption is concentrated in the **walking skeleton**: that a *separate
read-only web process* can open the *same local DuckDB store* the CLI uses, query it
offline, and render HTML — **without** ever holding the signing key (I-SCR-1) or opening
a write path. Validating this in US-VIEW-001 de-risks the entire slice before any
Job 2 / network work begins.
