# Definition of Ready: viewer-peer-subscriptions (slice-15)

> Owner: Luna (nw-product-owner) · 2026-06-09 · Job: **J-003c**
> 9-item hard gate + Dimension-0 Elevator-Pitch check + JTBD traceability check.
> Verdict: **PASS** for all 3 stories.

## Dimension 0 — Elevator Pitch Test (BLOCKING, checked first)

| Story | Pitch present? | Real entry point? | Concrete output? | Job connection? | Verdict |
|---|---|---|---|---|---|
| US-PS-001 | N/A (`@infrastructure`) | — | — | enables US-PS-002/003 | EXEMPT (infra) |
| US-PS-002 | YES (Before/After/Decision) | YES — `GET /peers` (HTTP route on `openlore ui`) | YES — rendered list of peer DID + claim count + render-only `peer remove` command text | YES — decides which peer to unsubscribe + sees the clean revocation path | PASS |
| US-PS-003 | YES (Before/After/Decision) | YES — `GET /peers` (empty-state path) | YES — rendered "no peers" message + render-only `peer add` command | YES — confirms "follow no one" + learns how to start | PASS |

**Slice-level check**: the slice contains TWO user-visible stories (US-PS-002, US-PS-003)
with real decisions — NOT all-`@infrastructure`. Slice has release value. **PASS.**

## JTBD traceability (hard-blocking, per Decision 1, 2026-04-28)

| Story | `job_id` | Valid? |
|---|---|---|
| US-PS-001 | `infrastructure-only` + `infrastructure_rationale` (feature-delta + user-stories) | PASS |
| US-PS-002 | `J-003c` (entry in `docs/product/jobs.yaml`, sub-job of J-003) | PASS |
| US-PS-003 | `J-003c` (entry in `docs/product/jobs.yaml`, sub-job of J-003) | PASS |

Every story carries a `job_id`; the infra story carries its rationale. **PASS.**

## 9-Item DoR Checklist

### US-PS-001 — Read-only active-subscription read + `/peers` wiring (`@infrastructure`)

| # | DoR Item | Status | Evidence |
|---|---|---|---|
| 1 | Problem clear, domain language | PASS | "the read-only viewer has no method to list active subscriptions with per-peer claim counts; a naive per-peer loop reintroduces N+1." |
| 2 | User/persona with characteristics | PASS | P-001 (Maria), indirectly — plumbing for the subscription-manager-hat stories. |
| 3 | 3+ domain examples, real data | PASS | Rachel(5)+Tobias(3) one query; soft-removed Rachel excluded; no-subs → empty vec. |
| 4 | UAT Given/When/Then (3–7) | PASS | 3 scenarios (one-query read; soft-removed excluded; empty no-error). |
| 5 | AC derived from UAT | PASS | 8 AC items; each traces to a scenario / constraint. |
| 6 | Right-sized (1–3 days, 3–7 scenarios) | PASS | ~1 day; one read method + one SQL + handler wiring; 3 scenarios. |
| 7 | Technical notes (constraints/deps) | PASS | proposed method signature; new SQL shape; rejects N+1; existing `count_peer_claims`/`list_active_subscriptions` referenced; read-only by construction. |
| 8 | Dependencies resolved or tracked | PASS | depends on slice-03 `peer_subscriptions` + slice-06 read-only viewer (both SHIPPED); no open dependency. |
| 9 | Outcome KPIs (measurable) | PASS | 1 aggregate query/render, invariant to peer count; behavioral assertion. |

### US-PS-002 — See who I follow + render-only revocation command

| # | DoR Item | Status | Evidence |
|---|---|---|---|
| 1 | Problem clear, domain language | PASS | "no browser surface that shows who she currently follows; to leave a peer she has to remember the exact `peer remove` syntax." |
| 2 | User/persona with characteristics | PASS | P-001 (Maria), subscription-manager hat, reviewing federation subscriptions in the browser. |
| 3 | 3+ domain examples, real data | PASS | two peers w/ counts + revoke commands; zero-cached-claims peer still listed; removed peer vanishes on reload. |
| 4 | UAT Given/When/Then (3–7) | PASS | 4 scenarios (list+count+command; removed-absent; htmx/no-JS parity; per-peer not merged). |
| 5 | AC derived from UAT | PASS | 7 AC items; each maps to a scenario / cardinal constraint. |
| 6 | Right-sized | PASS | ~1 day; one render fragment mirroring slice-08; 4 scenarios. |
| 7 | Technical notes | PASS | `render_peers_fragment`; `PEER_REMOVE_GUIDANCE_PREFIX` + `render_remove_guidance` mirroring slice-08; bare-DID strip. |
| 8 | Dependencies resolved or tracked | PASS | depends on US-PS-001 (in-slice) + slice-03/06/07/08 (SHIPPED). |
| 9 | Outcome KPIs | PASS | leading indicator OF KPI-FED-4; per-feature GREEN + cohort funnel. |

### US-PS-003 — Guided empty state

| # | DoR Item | Status | Evidence |
|---|---|---|---|
| 1 | Problem clear, domain language | PASS | "opening `/peers` to a blank page leaves her unsure she follows no one vs the view is broken; no pointer to how to start." |
| 2 | User/persona with characteristics | PASS | P-001 (Maria), subscription-manager hat, following no peers. |
| 3 | 3+ domain examples, real data | PASS | fresh store; only-soft-removed store; offline + both shapes. |
| 4 | UAT Given/When/Then (3–7) | PASS | 3 scenarios (empty state; soft-removed-only still empty; htmx/no-JS parity). |
| 5 | AC derived from UAT | PASS | 6 AC items; each maps to a scenario. |
| 6 | Right-sized | PASS | ~0.25 day; the `is_empty()` arm of the same fragment fn; 3 scenarios. |
| 7 | Technical notes | PASS | empty-state arm of `render_peers_fragment`; `peer add` guidance reuses slice-08 `SEARCH_FOLLOW_GUIDANCE_PREFIX` precedent. |
| 8 | Dependencies resolved or tracked | PASS | depends on US-PS-001 (in-slice); no open dependency. |
| 9 | Outcome KPIs | PASS | leading indicator OF KPI-VIEW-1; per-feature GREEN. |

## Completeness / bias checks (review-dimensions)

- **Happy-path bias?** NO — sad/edge paths covered: soft-removed peer excluded (US-PS-001/002),
  zero-cached-claims peer (US-PS-002), only-soft-removed store (US-PS-003), offline render
  (all), no-subscriptions empty (US-PS-003). The residue-made-visible absence is itself an
  edge-of-residue case.
- **Technology bias?** NO — requirements are solution-neutral (DESIGN owns SQL shape, DTO
  location, markup); the read-only/active-only/local-only invariants are product
  constraints, not tech choices.
- **Testable AC?** YES — every AC is observable through the real `openlore ui` subprocess
  (rendered text, presence/absence, query count, both shapes).
- **NFRs?** YES — read-only, active-only, local/offline, no-N+1, loopback/no-persist,
  plain-language errors (requirements.md §3), each with a measurable criterion.

## Verdict

**DoR Status: PASSED — 9/9 for all 3 stories; Dimension 0 PASS; JTBD traceability PASS.**
Ready for peer review and DESIGN handoff (solution-architect), pending the non-blocking
DIVERGE risk noted in `wave-decisions.md` (R-PS-1).
