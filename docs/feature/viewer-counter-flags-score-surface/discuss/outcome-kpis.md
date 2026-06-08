# Outcome KPIs: viewer-counter-flags-score-surface (slice-14)

> slice-14 mints **NO new KPI ID** (matching slices 08–13). It REALIZES inherited KPIs on the LAST
> facet (the `/score` contributor-scoring contribution flag) and carries one slice-specific
> guardrail HYPOTHESIS for the score-orthogonality / sum-to-weight invariant (a realization of
> KPI-GRAPH-3 + the slice-09 CARDINAL, not a new contract ID). The cross-feature SSOT is
> `docs/product/kpi-contracts.yaml`. Detail inlined here (lean precedent: slice-08/11/12/13).

## Feature: viewer-counter-flags-score-surface

### Objective
Complete the at-a-glance J-003b facet across EVERY local viewer surface: while reading a
contributor's transparent `/score` breakdown, the operator can instantly spot which contributions
have drawn disagreement and open the thread — WITHOUT the flag touching any weight, subtotal, rank,
or order, and without misreading the flag as a score deduction.

### Outcome KPIs (inherited; realized on the scoring surface)

| # | Who | Does What | By How Much | Baseline | Measured By | Type |
|---|-----|-----------|-------------|----------|-------------|------|
| 1 | P-001 dogfood operators reading a contributor's `/score` breakdown | open a contested contribution's slice-11 thread directly from the score-flag (instead of blind drill-in) | a measurable share navigate score-flag → thread (leading indicator OF KPI-FED-3) | today the `/score` breakdown shows NO counter indication; discovery requires copying each CID and opening the thread | per-feature GREEN (flag renders for countered contributions) + opt-in telemetry (ADR-010) | Leading (secondary) |
| 2 | P-001 operators reading the flagged `/score` breakdown | correctly understand the flag as ORTHOGONAL to the score (not a deduction) | 0 reported misreads in dogfood; the anti-misread copy AC is GREEN | today there is no in-context cue that disagreement is orthogonal to the score | anti-misread copy AC (AC-SCORE-ANTIMISREAD) + dogfood comprehension feedback | Leading (comprehension) |

### Guardrail metrics (release-blocking — MET by construction + AC)

| Guardrail KPI | What must NOT degrade | Enforced by |
|---|---|---|
| **Sum-to-weight (slice-09 CARDINAL / KPI-GRAPH-3 reproduce-by-hand)** | the per-claim subtotals must STILL sum to the displayed pairing weight with the flag present | AC-SCORE-SUMWEIGHT — subtotals-sum-to-weight asserted on a FLAGGED breakdown |
| **Shown-never-applied (ADR-015 / KPI-GRAPH-4)** | every weight/confidence/bonus/subtotal/total/bucket + the ranking + the row order byte-identical to slice-09; a countered claim keeps its FULL original weight | AC-SCORE-BYTEID — byte-identity vs the slice-09 baseline with markers elided |
| **KPI-VIEW-2 (read-only)** | no write/sign/counter route on `/score`; no signing key | C-1 + xtask check-arch + behavioral gold |
| **KPI-AV-2 / KPI-GRAPH-2 (anti-merging)** | each flag presence-only (a boolean per CID), never a merged "disputed by N"; attribution stays in the slice-11 thread | AC-SCORE-PRESENCE |
| **KPI-4 (verbatim confidence/weight)** | every confidence (`0.90`), bonus, subtotal, weight renders exactly as slice-09 | C-4 + AC-SCORE-BYTEID |
| **KPI-5 / KPI-VIEW-5 / KPI-HX-G1/G2/G3 (local-first / offline / no-CDN / parity)** | the `/score` flag read is LOCAL (no network); the page renders offline; htmx + no-JS parity | AC-SCORE-LOCAL + AC-002-PARITY |
| **N+1 guard (ADR-048)** | exactly 1 `counter_presence_for` call per render, invariant to contribution/pairing count | US-CF-001 AC + the inherited slice-12 adapter property test |

### Metric Hierarchy
- **North Star (inherited)**: **KPI-FED-3** (counter-claim publication rate — the J-003b loop). slice-14 strengthens the READ side of the loop on the LAST surface: seeing, while reading a score, that a contribution drew a counter — without hunting.
- **Leading Indicators**: navigate score-flag → thread (KPI #1); correct comprehension of orthogonality (KPI #2). Both feed KPI-VIEW-1 (time-to-see-store-contents — at-a-glance disagreement on the scoring surface, zero drill-in, zero SQL).
- **Guardrail Metrics (must NOT degrade)**: sum-to-weight; shown-never-applied byte-identity; read-only; anti-merging; verbatim; local-first; the N+1 guard. The sum-to-weight + anti-misread guardrails are the LOAD-BEARING ones unique to this surface.

### Measurement Plan
| KPI | Data Source | Collection Method | Frequency | Owner |
|-----|------------|-------------------|-----------|-------|
| Navigate score-flag → thread | opt-in telemetry endpoint (ADR-010) | per-feature GREEN + cohort opt-in | per-release + day-30 | platform-architect (DEVOPS) |
| Anti-misread comprehension | dogfood feedback + AC-SCORE-ANTIMISREAD | qualitative + AC gate | per-release | nw-product-owner |
| Sum-to-weight + byte-identity | the flagged-render gold (markers elided) + the slice-09 transparency unit test | automated test gate | every CI run | acceptance-designer (DISTILL) |
| Presence-query count (N+1 guard) | behavioral subprocess assertion + slice-12 adapter property | automated test gate | every CI run | acceptance-designer (DISTILL) |

### Hypothesis
> We believe that surfacing the neutral "Countered" flag on the `/score` contributor-scoring
> breakdown (P-001, counter-claim-scanner hat), with copy that makes its orthogonality to the score
> unmistakable, will increase the share of dogfood operators who OPEN a contested contribution's
> thread while reading a score (a leading indicator of KPI-FED-3) AND will keep the score legible
> and trustworthy, because seeing the flag in-context removes the need to blind-open every
> contribution while the anti-misread copy + the preserved sum-to-weight keep the flag from being
> read as a deduction. We will know this is true when, post-slice-14, operators report (and opt-in
> telemetry shows) they navigate from a score-flag to a counter thread, no operator reports
> misreading the flag as a score penalty, and the sum-to-weight + byte-identity gold stays GREEN.

### Smell tests (per the framework)
- **Measurable today?** Yes — per-feature GREEN is a test gate; cohort is the inherited opt-in
  telemetry. The guardrails are automated gold tests.
- **Outcome not output?** Yes — KPI #1/#2 describe a behavior change (navigate to thread; correct
  comprehension), not "ship a flag".
- **Has guardrails?** Yes — sum-to-weight, byte-identity, read-only, anti-merging, verbatim,
  local-first, N+1 — all release-blocking.
