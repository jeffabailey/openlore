# Definition of Ready: viewer-search-follow-state (slice-16)

> Owner: Luna (nw-product-owner) · 2026-06-09 · Job: **J-005c**
> 9-item hard gate + Dimension-0 Elevator-Pitch check + JTBD traceability check.
> Verdict: **PASS** for both stories.

## Dimension 0 — Elevator Pitch Test (BLOCKING, checked first)

| Story | Pitch present? | Real entry point? | Concrete output? | Job connection? | Verdict |
|---|---|---|---|---|---|
| US-SF-001 | N/A (`@infrastructure`) | — | — | enables US-SF-002 | EXEMPT (infra) |
| US-SF-002 | YES (Before/After/Decision) | YES — `GET /search` (HTTP route on `openlore ui`) | YES — a followed author's row shows a neutral "Following" indicator + no add command; an unfollowed author's row shows the render-only `openlore peer add <did>` TEXT | YES — decides which genuinely-new discovered authors to follow next (J-005c discovery→federation) | PASS |

**Slice-level check**: the slice contains ONE user-visible story (US-SF-002) with a real
decision — NOT all-`@infrastructure`. Slice has release value. **PASS.**

## JTBD traceability (hard-blocking, per Decision 1, 2026-04-28)

| Story | `job_id` | Valid? |
|---|---|---|
| US-SF-001 | `infrastructure-only` + `infrastructure_rationale` (feature-delta + user-stories) | PASS |
| US-SF-002 | `J-005c` (entry in `docs/product/jobs.yaml`, sub-job of J-005) | PASS |

Every story carries a `job_id`; the infra story carries its rationale. **PASS.**

## 9-Item DoR Checklist

### US-SF-001 — Resolve relationship against the LOCAL active set + wire it into `/search` (`@infrastructure`)

| # | DoR Item | Status | Evidence |
|---|---|---|---|
| 1 | Problem clear, domain language | PASS | "`to_indexed_claim` hardcodes `NetworkUnfollowed` for every result author, so the render cannot tell a followed author from an unfollowed one; the active set is read for `/peers` but not threaded into `/search`." |
| 2 | User/persona with characteristics | PASS | P-001 (Maria), network-discovery hat, indirectly — the resolution plumbing for US-SF-002. |
| 3 | 3+ domain examples, real data | PASS | followed Rachel + unfollowed Priya from one batch read; fragmented result DID vs bare active DID; failed read → status-quo degrade. |
| 4 | UAT Given/When/Then (3–7) | PASS | 3 scenarios (one-batch-read resolution; fragment-strip match; failed-read degrade). |
| 5 | AC derived from UAT | PASS | 9 AC items; each traces to a scenario / constraint. |
| 6 | Right-sized (1–3 days, 3–7 scenarios) | PASS | ~0.5 day; one read thread + one resolution fn (REUSES the slice-15 read; no new method/SQL); 3 scenarios. |
| 7 | Technical notes (constraints/deps) | PASS | resolution seam (`resolve_search_state` + `to_indexed_claim`); REUSES `list_active_peer_subscriptions` + `bare_did`; rejects N+1; degrade path; no new variant/method. |
| 8 | Dependencies resolved or tracked | PASS | depends on slice-15 active read + slice-08 `/search`/`to_indexed_claim` (both SHIPPED); `AuthorRelationship` enum SHIPPED. No open dependency. |
| 9 | Outcome KPIs (measurable) | PASS | 1 active-set read/render invariant to result count; 100% of followed authors resolved `SubscribedPeer`; behavioral assertion. |

### US-SF-002 — "Following" for a followed author; `peer add` only for new ones

| # | DoR Item | Status | Evidence |
|---|---|---|---|
| 1 | Problem clear, domain language | PASS | "EVERY `/search` row shows `peer add` — even for developers Maria already follows — so she cannot tell new authors from ones she has already acted on, and is told to 'add' peers she already has." |
| 2 | User/persona with characteristics | PASS | P-001 (Maria), network-discovery hat, scanning search results in the browser, wants to spot already-followed vs new authors. |
| 3 | 3+ domain examples, real data | PASS | followed Rachel "Following" + unfollowed Priya keeps add; all-followed (no add anywhere); none-followed (slice-08 status quo). |
| 4 | UAT Given/When/Then (3–7) | PASS | 4 scenarios (followed→Following; unfollowed→add; side-by-side attribution; htmx/no-JS parity). |
| 5 | AC derived from UAT | PASS | 7 AC items; each maps to a scenario / cardinal constraint. |
| 6 | Right-sized | PASS | ~0.5 day; one render arm (mirrors slice-08 `render_follow_guidance`); 4 scenarios. |
| 7 | Technical notes | PASS | extend `render_search_results_fragment` with a `SubscribedPeer → "Following"` arm; REUSE `render_follow_guidance`/`SEARCH_FOLLOW_GUIDANCE_PREFIX` for the `NetworkUnfollowed` arm; new render-only "Following" indicator. |
| 8 | Dependencies resolved or tracked | PASS | depends on US-SF-001 (in-slice) + slice-08 `render_follow_guidance` (SHIPPED). |
| 9 | Outcome KPIs | PASS | leading indicator OF KPI-AV-4; per-feature GREEN + cohort funnel quality. |

## Completeness / bias checks (review-dimensions)

- **Happy-path bias?** NO — sad/edge paths covered: failed active-set read degrades to status
  quo (US-SF-001); fragment-mismatch (US-SF-001); all-followed (no add anywhere) + none-followed
  (status quo preserved, no over-correction) (US-SF-002); offline resolution (all). The
  over-correction risk (R-SF-4: unfollowed author wrongly loses the affordance) is explicitly
  tested.
- **Technology bias?** NO — requirements are solution-neutral (DESIGN owns the resolution fn
  shape, set type, "Following" indicator markup/copy, match arms); the read-only / LOCAL /
  no-N+1 / index-neutral invariants are product constraints, not tech choices.
- **Testable AC?** YES — every AC is observable through the real `openlore ui` subprocess
  (rendered "Following" vs `peer add` text, presence/absence, read count invariant, both shapes,
  grouping/order equality).
- **NFRs?** YES — read-only, accuracy, LOCAL/offline resolution, no-N+1, attribution+ranking
  unchanged, graceful degradation, parity (requirements.md §3), each with a measurable criterion.

## Verdict

**DoR Status: PASSED — 9/9 for both stories; Dimension 0 PASS; JTBD traceability PASS.**
Ready for peer review and DESIGN handoff (solution-architect), pending the non-blocking DIVERGE
risk noted in `wave-decisions.md` (R-SF-1).
