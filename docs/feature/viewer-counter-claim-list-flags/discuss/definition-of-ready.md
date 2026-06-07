# Definition of Ready: viewer-counter-claim-list-flags (slice-12)

DoR is a hard 9-item gate. Each item must PASS with evidence before DESIGN handoff.

## Per-story DoR

### US-LF-001 (infrastructure-only) — batch counter-presence read

| DoR Item | Status | Evidence/Issue |
|----------|--------|----------------|
| 1. Problem statement clear, domain language | PASS | "Asking 'is each of these 50 claims countered?' via the per-CID read would be 50 queries (N+1)"; no batch capability exists. |
| 2. User/persona with specific characteristics | PASS | P-001 ("Maria"); plumbing whose decision is enabled by US-LF-002. |
| 3. 3+ domain examples with real data | PASS | 3 examples (3-CID page 1-countered; none-countered empty set; own+peer counters same page) with real CIDs. |
| 4. UAT in Given/When/Then (3-7) | PASS | 5 scenarios (flags-only-genuine, single-query N+1 guard, empty-set, local-offline, read-only). |
| 5. AC derived from UAT | PASS | 5 AC, each from a scenario. |
| 6. Right-sized (1-3 days, 3-7 scenarios) | PASS | Widens slice-11 Step-A to `IN (...)`; ~part of a ~1-day slice; 5 scenarios. |
| 7. Technical notes: constraints/dependencies | PASS | Widens `query_counter_claims` Step-A; DESIGN owns collection type + `IN` binding; no new crate. |
| 8. Dependencies resolved or tracked | PASS | Depends on indexed ref tables (slice-03, exist). Enables US-LF-002/003. |
| 9. Outcome KPIs with measurable target | PASS | Query count == 1 per page, invariant to page size (N+1 guard), gold-tested. |

### US-LF-002 (J-003b) — the at-a-glance "Countered" flag

| DoR Item | Status | Evidence/Issue |
|----------|--------|----------------|
| 1. Problem statement clear, domain language | PASS | "Scanning her list, she is blind to disagreement — she cannot triage where to spend attention." |
| 2. User/persona with specific characteristics | PASS | P-001 ("Maria"), counter-claim-scanner hat, scanning her own `/claims` list. |
| 3. 3+ domain examples with real data | PASS | 3 examples (Tobias counters Rust claim `bafyMariaRust`; own counter; many-counters-one-flag `bafyMariaTDD`). |
| 4. UAT in Given/When/Then (3-7) | PASS | 4 scenarios (neutral marker, link-to-thread, single-marker-no-count, fragment/no-JS parity). |
| 5. AC derived from UAT | PASS | 5 AC, each from a scenario. |
| 6. Right-sized | PASS | One render extension on an existing route; demonstrable in one session. |
| 7. Technical notes | PASS | Extends `ClaimRowView`/`render_claim_row`; reuses `COUNTERED_PRESENCE_FLAG`; no new route/crate. |
| 8. Dependencies resolved or tracked | PASS | Depends on US-LF-001; slice-11 thread (exists) as the link target. |
| 9. Outcome KPIs | PASS | List-flag → thread navigation within a session (leading indicator of KPI-FED-3); baseline 0. |
| Elevator Pitch (Dimension 0) | PASS | Before/After (`/claims` route + observable "Countered" marker)/Decision-enabled all present. |

### US-LF-003 (J-003b) — no-noise + no-regression discipline

| DoR Item | Status | Evidence/Issue |
|----------|--------|----------------|
| 1. Problem statement clear, domain language | PASS | "A flag that quietly reorders or re-scores would pick a triage order FOR her and break shown-never-applied." |
| 2. User/persona with specific characteristics | PASS | P-001 ("Maria"), anxious an automated flag might transform her data/order. |
| 3. 3+ domain examples with real data | PASS | 3 examples (un-countered renders as slice-06; mixed-page order preserved; confidence `0.30` untouched). |
| 4. UAT in Given/When/Then (3-7) | PASS | 4 scenarios (no-marker-no-noise, order/paging/count unchanged, confidence verbatim, mixed-page flags only countered). |
| 5. AC derived from UAT | PASS | 5 AC, each from a scenario. |
| 6. Right-sized | PASS | A thin discipline layer on the same render. |
| 7. Technical notes | PASS | Presence set mapped AFTER `list_claims` (SQL untouched); gold byte-identity test. |
| 8. Dependencies resolved or tracked | PASS | Depends on US-LF-001, US-LF-002. |
| 9. Outcome KPIs | PASS | 100% byte-identical order/paging/count/confidence vs slice-06 (zero tolerance, gold-tested). |
| Elevator Pitch (Dimension 0) | PASS | Before/After/Decision-enabled all present, real `/claims` route + observable output. |

## Slice-level checks

- **Slice composition (Dimension 0)**: PASS — the slice has TWO user-visible value stories (US-LF-002, US-LF-003), not only `@infrastructure`.
- **JTBD traceability**: PASS — every story has a `job_id` (US-LF-001 `infrastructure-only` + rationale; US-LF-002/003 → J-003b).
- **JTBD scope/contradiction gate**: PASS (see feature-delta.md — single job, no sibling/cardinal contradiction).
- **Scope (Elephant Carpaccio)**: PASS — 3 stories, 1 context, 1 new integration point, ~1 day.

## DoR Status: PASSED (9/9 for all three stories; Dimension-0 Elevator Pitch + slice-composition pass)

## Open decision (does NOT block DoR — flagged for user)

- **Scope fork**: `/claims`-only (recommended) vs ALSO `/project`+`/philosophy`+`/score`.
  The DoR above validates the `/claims`-only scope. If the user expands scope to the
  graph/score surfaces, the stories/DoR must expand and the ≤1-day estimate no longer
  holds (it becomes slice-13's work bundled in). Default carried forward: `/claims`-only.
