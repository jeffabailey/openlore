# Outcome KPIs: htmx-scraper-viewer (slice-06)

## Feature: htmx-scraper-viewer

### Objective
Make the node operator's node legible: let them *see what their node holds* (and what they
could add) in a browser, read-only, without SQL — turning an opaque store into a glanceable
one, while provably never writing, signing, or exposing the key.

### Outcome KPIs

| # | Who | Does What | By How Much | Baseline | Measured By | Type |
|---|-----|-----------|-------------|----------|-------------|------|
| **KPI-VIEW-1** (north star) | Node operator | Views their persisted claims in a browser | In < 10 s from cold viewer start, with **zero** SQL typed | Today: requires DuckDB shell + hand-written SQL (effectively never done routinely) | Leading |
| **KPI-VIEW-2** (guardrail) | The viewer (all routes) | Performs no write and no sign | **0** write/sign code paths reachable from any route; **0** key reads in the process | n/a (new surface) | Guardrail |
| **KPI-VIEW-3** | Federated node operator | Inspects and distinguishes federated peer claims from their own | 100% of peer rows show origin and are separable from own claims | Today: no browser view of `peer_claims` | Leading |
| **KPI-VIEW-4** | Node operator | Reviews scrape proposals in the browser, then signs in the CLI | Candidate triage shifts from CLI batch text to a scannable browser list (review-before-sign in browser ≥ 1 path) | Today: only CLI batch-text review | Leading (secondary) |
| **KPI-VIEW-5** (guardrail) | Node operator | Uses the store view offline | Store views (My Claims, Peer Claims, detail) render with **0** network calls | slice-01 KPI-5 (local-first) | Guardrail |

### Metric Hierarchy

- **North Star**: **KPI-VIEW-1** — time-to-see-store-contents: the operator can view their
  persisted claims in a browser in < 10 s with zero SQL. This is the heart of the feature
  (Job 1, opportunity 15).
- **Leading indicators**: KPI-VIEW-3 (federated dimension inspected), KPI-VIEW-4 (proposals
  triaged in browser) — behaviors that broaden and deepen the legibility outcome.
- **Guardrail metrics (must NOT degrade)**:
  - **KPI-VIEW-2 (read-only)**: zero write/sign paths from the web surface; zero key in the
    web process. A single reachable write/sign path is a release blocker (I-VIEW-1/2/3, I-SCR-1).
  - **KPI-VIEW-5 (local-first)**: store view works fully offline (slice-01 KPI-5, I-VIEW-6).

### Measurement Plan

| KPI | Data Source | Collection Method | Frequency | Owner |
|-----|-------------|-------------------|-----------|-------|
| KPI-VIEW-1 | Viewer timing (cold start → first /claims paint) | Timed test against a representative store | Per release + on perf-sensitive changes | DEVOPS / platform-architect |
| KPI-VIEW-2 | Route inventory + key-access audit | Static route audit + tests asserting no write/sign/key path | Every CI run (blocking) | DELIVER (acceptance + impl) |
| KPI-VIEW-3 | Peer Claims view output | Test: peer rows show origin, distinct from own | Per release | DISTILL / acceptance-designer |
| KPI-VIEW-4 | Live Scrape view output | Test: proposals render + no sign control + CLI-sign directive present | Per release | DISTILL / acceptance-designer |
| KPI-VIEW-5 | Offline test harness | Run store views with network disabled | Every CI run | DELIVER |

### Hypothesis

We believe that a **read-only, localhost htmx viewer of the local DuckDB store** for the
**node operator** will achieve **routine, low-friction legibility of node contents**.
We will know this is true when the **operator** **views their persisted claims in a browser
in < 10 s with zero SQL** (KPI-VIEW-1), while **no write/sign path or key is ever reachable
from the web surface** (KPI-VIEW-2) and **the store view works offline** (KPI-VIEW-5).

### Handoff to DEVOPS (platform-architect)

- **Instrument**: cold-start-to-first-paint timing for /claims (KPI-VIEW-1).
- **Audit/alert**: route inventory must show zero write/sign endpoints; key material never
  read by the viewer process (KPI-VIEW-2) — treat any breach as a blocking alert.
- **Baseline**: confirm offline behavior of store views before release (KPI-VIEW-5).
