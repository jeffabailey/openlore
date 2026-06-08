# Definition of Ready: viewer-counter-flags-score-surface (slice-14)

> 9-item hard gate. Each item PASSES with evidence. Brownfield DELTA on slices 09/11/12/13;
> REUSES the slice-12 `counter_presence_for` read (no new read method) + the slice-13
> `render_countered_link` SSOT (no new render fn).

## Per-story DoR

### US-CF-001 — Reuse the batch counter-presence read in the `/score` handler (`@infrastructure`)

| DoR Item | Status | Evidence |
|----------|--------|----------|
| 1. Problem statement clear, domain language | PASS | "The slice-12 batch presence read exists and is proven on /claims + the graph surfaces, but the /score handler does not yet collect its contribution CID set nor call it; a naive per-contribution call would reintroduce N+1 across a breakdown with many contributions and pairings." |
| 2. User/persona with specific characteristics | PASS | Infra-wiring story; persona P-001 indirectly (the plumbing US-CF-002 consumes). `job_id: infrastructure-only` with rationale (slice has 1 user-visible story → release value). |
| 3. 3+ domain examples with real data | PASS | (1) `/score?contributor=did:plc:t0bi` 3 pairings / 9 contributions → ONE call; (2) a contribution CID reused across two pairings → deduped into one by-CID call; (3) `NoClaims` (did:plc:nobody) → no query / all-un-countered → empty set, no query. |
| 4. UAT in Given/When/Then (3-7) | PASS | 3 scenarios (breakdown one-query; NoClaims → no query; empty → no query). |
| 5. AC derived from UAT | PASS | AC-001-ONE-CALL, AC-001-INVARIANT, AC-001-UNCHANGED-READ, AC-001-NO-NEW-METHOD. |
| 6. Right-sized (1-3 days, 3-7 scenarios) | PASS | Handler wiring only; REUSES the read. <1 day. |
| 7. Technical notes: constraints/dependencies | PASS | REUSES `counter_presence_for` (slice-12); depends on slices 12/13 (SHIPPED); CID set from `Contribution.cid` across `WeightedPairing.contributions()`; seam `score_page`/`resolve_score_state`. |
| 8. Dependencies resolved or tracked | PASS | slice-12 read SHIPPED; slice-09 `/score` + `ScoreState` + `WeightedView` SHIPPED. No open dependency. |
| 9. Outcome KPIs with measurable targets | PASS | "Exactly 1 `counter_presence_for` call per render (or 0 for Form/NoClaims/empty), invariant to contribution/pairing count (0 N+1)"; behavioral assertion + inherited slice-12 adapter property. |

### US-CF-002 — "Countered" flag on `/score` contribution rows (score-orthogonal)

| DoR Item | Status | Evidence |
|----------|--------|----------|
| 1. Problem statement clear, domain language | PASS | "Maria reads a contributor's /score breakdown to decide whether to trust their adherence score; today she cannot tell which contributions have already drawn a counter without opening each thread, and the score is intentionally counter-agnostic, so she has no in-context cue where to apply judgment." |
| 2. User/persona with specific characteristics | PASS | P-001 "Maria", counter-claim-scanner hat, reading the `/score` breakdown; decorates J-002c without changing it. |
| 3. 3+ domain examples with real data | PASS | (1) Tobias's `bafy...t0bi` (0.88, subtotal 1.03) in the cargo→dependency-pinning pairing (weight 1.42) flagged, weight unchanged; (2) Maria's `bafy...mr1` (0.91, 0.39) un-flagged, sum-to-weight intact; (3) `bafy...dup` countered by two authors → ONE marker, rank unchanged. |
| 4. UAT in Given/When/Then (3-7) | PASS | 7 scenarios: marker+weight-unchanged; un-countered shows nothing; sum-to-weight preserved; byte-identity vs slice-09; anti-misread copy/orthogonality; two-author → one marker; LOCAL+offline+htmx/no-JS parity. |
| 5. AC derived from UAT | PASS | AC-002-MARKER, AC-002-LINK, AC-SCORE-SUMWEIGHT, AC-SCORE-BYTEID, AC-SCORE-ANTIMISREAD, AC-SCORE-PRESENCE, AC-SCORE-LOCAL, AC-002-PARITY, AC-002-NO-NOISE. |
| 6. Right-sized (1-3 days, 3-7 scenarios) | PASS | One projection seam + one render-site arm on an existing route, REUSING the slice-13 render; the only NEW work is the anti-misread copy + the sum-to-weight gold; <1 day; 7 scenarios (within 3-7). |
| 7. Technical notes: constraints/dependencies | PASS | Threads presence into `render_score_breakdown` (~line 1968); REUSE `render_countered_link` + `COUNTERED_PRESENCE_FLAG`; sum-to-weight preserved BY CONSTRUCTION (subtotals + weight project the SAME unchanged `WeightedPairing`); byte-identity via the slice-12/13 baseline+marker-elision tactic. |
| 8. Dependencies resolved or tracked | PASS | Depends on US-CF-001 (in-slice). slice-09 `/score` + slice-11 thread + slice-12 read + slice-13 render all SHIPPED. |
| 9. Outcome KPIs with measurable targets | PASS | Leading indicator OF KPI-FED-3 (navigate score-flag → thread); guardrail KPI sum-to-weight + byte-identity + anti-misread comprehension; per-feature GREEN + the flagged-render gold. |

## DoR Status: PASSED (9/9 for both stories)

## Elevator-Pitch Test (Dimension 0 — checked first, BLOCKING)

| Story | Pitch present (Before/After/Decision)? | Real entry point? | Concrete output? | Decision enabled? | Verdict |
|---|---|---|---|---|---|
| US-CF-001 | N/A — `@infrastructure` (no pitch required; produces no user-visible output) | — | — | enables US-CF-002 | PASS (infra) |
| US-CF-002 | YES (Before/After/Decision) | YES — `GET /score?contributor=<did>` (HTTP route on the real `openlore ui`) | YES — a rendered "Countered" marker + `<a href="/claims/{cid}">` link beside an UNCHANGED subtotal/weight, plus the anti-misread legend | YES — which of a contributor's contributions to scrutinize before trusting the score | PASS |

**Slice-level check (Dimension 0.5):** the slice contains ONE non-`@infrastructure`, user-visible
story (US-CF-002). NOT every story is `@infrastructure` → the slice has release value. PASS.

## JTBD traceability (hard-blocking per Decision 1, 2026-04-28)

| Story | `job_id` | Valid? |
|---|---|---|
| US-CF-001 | `infrastructure-only` + `infrastructure_rationale` (present) | PASS — slice retains ≥1 non-infra story |
| US-CF-002 | `J-003b` (in `docs/product/jobs.yaml`) | PASS |

## Anti-pattern scan

| Anti-pattern | Present? | Note |
|---|---|---|
| Implement-X | No | Stories start from Maria's pain (cannot see contested contributions while reading a score; no cue that disagreement is orthogonal to the score). |
| Generic data | No | Real DIDs/CIDs/predicates/weights (Tobias `bafy...t0bi`, Maria `bafy...mr1`, cargo→dependency-pinning, 0.88, subtotal 1.03, weight 1.42). |
| Technical AC | No | AC are observable outcomes (marker shown, link target, subtotals sum to weight, byte-identity vs slice-09, plain-language copy, one query) — not "use SQL X". |
| Technical scenario title | No | Scenarios describe what Maria achieves (spot a contested contribution; the score is unchanged), not how the read works. |
| Oversized story | No | 2 stories, ≤7 scenarios each, ~1 day total; reuse-only. |
| Abstract requirements | No | 3+ concrete examples per story with real data. |

## Score-orthogonality / anti-misread completeness (slice-specific gate)

| Check | Status | Evidence |
|---|---|---|
| Sum-to-weight CARDINAL re-asserted as an explicit AC? | PASS | AC-SCORE-SUMWEIGHT (subtotals still sum to weight on a flagged breakdown). |
| Byte-identity vs the slice-09 `/score` baseline (markers elided)? | PASS | AC-SCORE-BYTEID. |
| Anti-misread copy an explicit AC + KPI? | PASS | AC-SCORE-ANTIMISREAD + outcome-kpis.md KPI #2 (comprehension). |
| Presence-only (one marker per ≥1-counter author-cid)? | PASS | AC-SCORE-PRESENCE. |
| LOCAL / offline invariant? | PASS | AC-SCORE-LOCAL (slice-09 `/score` is already a fully-offline LOCAL read). |
| Counter SHOWN never APPLIED (full weight retained)? | PASS | C-2 / C-9 + AC-SCORE-SUMWEIGHT + AC-SCORE-BYTEID. |

## DoR overall verdict: PASSED (9/9, Dimension 0 PASS, JTBD PASS, anti-patterns clean, score-orthogonality gate PASS)
