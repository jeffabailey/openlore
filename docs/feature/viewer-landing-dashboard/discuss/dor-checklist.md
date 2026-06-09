# Definition of Ready: viewer-landing-dashboard (slice-17)

> Owner: Luna (nw-product-owner) · 2026-06-09 · Job: **J-002** (orientation facet)
> 9-item hard gate + Dimension-0 Elevator-Pitch check + JTBD traceability check.
> Verdict: **PASS** for both stories.

## Dimension 0 — Elevator Pitch Test (BLOCKING, checked first)

| Story | Pitch present? | Real entry point? | Concrete output? | Job connection? | Verdict |
|---|---|---|---|---|---|
| US-LD-000 | N/A (`@infrastructure`) | — | — | enables US-LD-001 | EXEMPT (infra) |
| US-LD-001 | YES (Before/After/Decision) | YES — `GET /` (HTTP route on `openlore ui`) | YES — rendered store summary (3 counts) + a navigation hub of 8 surface links | YES — decides where to go next + whether the store has anything to explore | PASS |

**Slice-level check**: the slice contains ONE user-visible story (US-LD-001) with a real
decision (orient: what's here + where to go) — NOT all-`@infrastructure`. Slice has release
value. **PASS.**

## JTBD traceability (hard-blocking, per Decision 1, 2026-04-28)

| Story | `job_id` | Valid? |
|---|---|---|
| US-LD-000 | `infrastructure-only` + `infrastructure_rationale` (feature-delta + user-stories) | PASS |
| US-LD-001 | `J-002` (entry in `docs/product/jobs.yaml`) | PASS |

Every story carries a `job_id`; the infra story carries its rationale. **PASS.**

## 9-Item DoR Checklist

### US-LD-000 — Thread the store + resolve the three counts (`@infrastructure`)

| # | DoR Item | Status | Evidence |
|---|---|---|---|
| 1 | Problem clear, domain language | PASS | "`GET /` is the only handler that takes no store; surfacing a store summary needs the store threaded + 3 counts; a naive impl could fabricate a 0 or 5xx the front door." |
| 2 | User/persona with characteristics | PASS | P-001 (Maria), indirectly — plumbing for the orientation-hat story. |
| 3 | 3+ domain examples, real data | PASS | populated store (12/7/2) three reads; fresh store three real zeros; failed peer-claims read → missing-number, no 5xx. |
| 4 | UAT Given/When/Then (3–7) | PASS | 3 scenarios (fixed aggregate reads; real zeros; failed read degrades). |
| 5 | AC derived from UAT | PASS | 8 AC items; each traces to a scenario / constraint. |
| 6 | Right-sized (1–3 days, 3–7 scenarios) | PASS | ~0.5 day; thread the store + 3 existing reads + the `LandingSummary`; 3 scenarios. |
| 7 | Technical notes (constraints/deps) | PASS | thread `store.as_ref()` into `landing_page`; the 3 existing reads cited with line numbers; the open `.len()`-vs-count-only DESIGN question; the `unwrap_or_default` degrade precedent; read-only by construction. |
| 8 | Dependencies resolved or tracked | PASS | depends on slice-06 read-only viewer + `count_claims`/`count_peer_claims` and slice-15 `list_active_peer_subscriptions` (all SHIPPED); no open dependency. |
| 9 | Outcome KPIs (measurable) | PASS | 3 aggregate reads/render invariant to store size; 0 of N failures 5xx; behavioral assertion. |

### US-LD-001 — Open the viewer and orient: store summary + navigate everywhere

| # | DoR Item | Status | Evidence |
|---|---|---|---|
| 1 | Problem clear, domain language | PASS | "she sees only a heading, the read-only notice, and a single claims link; cannot tell what's in her store; cannot discover the 8 other surfaces." |
| 2 | User/persona with characteristics | PASS | P-001 (Maria), orientation hat, opening the viewer for the first time in a session. |
| 3 | 3+ domain examples, real data | PASS | populated store + full hub (clicks to /peers); fresh empty store honest zeros + full hub; failed peer-claims read → "—" + full hub. |
| 4 | UAT Given/When/Then (3–7) | PASS | 5 scenarios (summary; links-to-all-surfaces; read-only; graceful degrade; offline). |
| 5 | AC derived from UAT | PASS | 8 AC items; each maps to a scenario / cardinal constraint. |
| 6 | Right-sized | PASS | ~0.5–1 day; extend one pure render (`render_landing`) with a small input + the nav hub from existing URL consts; 5 scenarios. |
| 7 | Technical notes | PASS | `render_landing(summary)` extension; the `render_tab_nav` link precedent; the 8 URL consts cited with line numbers; the open `SCRAPE_URL`-const + shape-fork DESIGN questions. |
| 8 | Dependencies resolved or tracked | PASS | depends on US-LD-000 (in-slice) + slices 06/07/08/09/10/15 (all SHIPPED surfaces it links to). |
| 9 | Outcome KPIs | PASS | leading indicator OF KPI-VIEW-1 (front-door + discoverability funnel); per-feature GREEN + cohort funnel. |

## Completeness / bias checks (review-dimensions)

- **Happy-path bias?** NO — sad/edge paths covered: a failed count read degrades to a
  missing-number state without a 5xx (US-LD-000/001); a fresh empty store shows honest zeros
  (US-LD-000/001); offline render (all); a soft-removed peer is not counted (US-LD-000). The
  missing-vs-zero distinction is itself an edge-of-failure case.
- **Technology bias?** NO — requirements are solution-neutral (DESIGN owns the
  `LandingSummary` shape, the markup, the `.len()`-vs-count-only choice, whether to fork the
  shape, whether to mint `SCRAPE_URL`); the read-only / LOCAL-only / graceful-degrade
  invariants are product constraints, not tech choices.
- **Testable AC?** YES — every AC is observable through the real `openlore ui` subprocess
  (rendered counts, the 8 surface links, read-only absence-of-controls, the missing-number
  state on a seeded failure, offline render, read-count invariance).
- **NFRs?** YES — read-only, local/offline+degrade, no-N+1, loopback/no-persist,
  plain-language errors, parity (requirements.md §3), each with a measurable criterion.

## Verdict

**DoR Status: PASSED — 9/9 for both stories; Dimension 0 PASS; JTBD traceability PASS.**
Ready for peer review and DESIGN handoff (solution-architect), pending the non-blocking
DIVERGE risk noted in `wave-decisions.md` (R-LD-1). One OPEN DESIGN QUESTION (WD-LD-5: the
active-subs count-read approach) is surfaced for DESIGN, not a DoR blocker — the PRODUCT
contract (a single aggregate read for the active-subs count) holds either way.
