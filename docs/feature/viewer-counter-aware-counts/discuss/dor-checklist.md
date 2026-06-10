# Definition of Ready: viewer-counter-aware-counts (slice-18)

> 9-item hard gate per story. Verdict at the bottom. Dimension-0 (Elevator Pitch) and JTBD
> traceability checked first as hard-blocking gates.

## Dimension 0 — Elevator Pitch (BLOCKING, checked first)

| Story | Elevator Pitch present (Before/After/Decision)? | Real entry point? | Concrete output? | Job connection? | Verdict |
|---|---|---|---|---|---|
| US-CC-000 | N/A — `@infrastructure` (`job_id: infrastructure-only`) | — | — | enables US-CC-001/002 | EXEMPT (infra) |
| US-CC-001 | YES — Before/After/Decision triplet | YES — `http://127.0.0.1:<port>/` (`GET /`) | YES — "12 own claims (3 countered)" rendered text | YES — decide whether to read disagreements on own claims first | PASS |
| US-CC-002 | YES — Before/After/Decision triplet | YES — `http://127.0.0.1:<port>/claims` (`GET /claims`) | YES — "(3 countered)" in the list header | YES — orient the list page (3 contested or 30) at a glance | PASS |

Slice-level check: the slice contains TWO non-`@infrastructure`, user-visible stories
(US-CC-001, US-CC-002) with real decisions → the slice has release value. **PASS.**

## JTBD traceability (hard-blocking)

| Story | `job_id` | Valid? |
|---|---|---|
| US-CC-000 | `infrastructure-only` + `infrastructure_rationale` present (in user-stories.md) | PASS |
| US-CC-001 | `J-003b` (counter-claim awareness — orientation/at-a-glance-count facet) | PASS — J-003b exists in `docs/product/jobs.yaml` |
| US-CC-002 | `J-003b` | PASS |

## US-CC-000 — Resolve the countered-own-claims count (`@infrastructure`)

| DoR Item | Status | Evidence/Issue |
|---|---|---|
| Problem statement clear, domain language | PASS | "No read answers 'how many of my own claims have been countered' in one cheap aggregate" — domain-precise (counter-reference tables, presence). |
| User/persona identified | PASS | P-001 (Maria) indirectly; plumbing for US-CC-001/002. |
| 3+ domain examples with real data | PASS | 3 examples: 12 claims/3 countered (one twice-countered) in one read; honest zero; failed read degrades independently. Real CIDs (`bafyMariaRust`, `bafyMariaTDD`, `bafyMariaSemver`), real peers (Rachel, Tobias). |
| UAT in Given/When/Then (3–7) | PASS | 4 scenarios (single-aggregate read; counted-once; honest zero; independent degrade). |
| AC derived from UAT | PASS | 7 AC, each maps to a scenario/contract. |
| Right-sized (1–3 days, 3–7 scenarios) | PASS | ~0.5 day (one read + thread into the summary). 4 scenarios. |
| Technical notes: constraints/dependencies | PASS | The counter-ref data + the open read question (count-only aggregate vs `counter_presence_for(...).len()`) + the `LandingSummary` thread + the `.ok()` degrade. |
| Dependencies resolved/tracked | PASS | Reuses slice-12 counter-ref tables (shipped) + slice-17 `LandingSummary` (shipped). The open read shape is WD-CC-5 (DESIGN), not a blocker. |
| Outcome KPIs with measurable targets | PASS | Landing read budget grows by AT MOST 1; 0/N failures 5xx; invariant to store size. |

**US-CC-000 DoR: PASSED (9/9 + Dimension-0 exempt + JTBD PASS).**

## US-CC-001 — Landing countered count

| DoR Item | Status | Evidence/Issue |
|---|---|---|
| Problem statement clear, domain language | PASS | "The landing tells her how MUCH is in her store but not how much has been DISPUTED" — domain language, real surface. |
| User/persona identified | PASS | P-001 (Maria), counter-aware-orientation hat, opening the viewer at session start. |
| 3+ domain examples with real data | PASS | 3 examples: 12/3 inline; honest "(0 countered)"; twice-countered counts once + neutral copy. Real CIDs/peers/confidence (`0.30`). |
| UAT in Given/When/Then (3–7) | PASS | 6 scenarios (shows count; honest zero; counted-once; no re-weight; degrade; offline). |
| AC derived from UAT | PASS | 8 AC, each maps to a scenario. |
| Right-sized (1–3 days, 3–7 scenarios) | PASS | ~0.5 day (extend the slice-17 summary render). 6 scenarios. |
| Technical notes: constraints/dependencies | PASS | The render extension site (`render_landing` ~636), the `render_count` reuse, the anti-misread copy, the open phrasing/markup question. |
| Dependencies resolved/tracked | PASS | Depends on US-CC-000 (the count). The slice-17 landing summary is shipped. |
| Outcome KPIs with measurable targets | PASS | Leading indicator of KPI-VIEW-1 (now including disputed-claim state); share that drill into `/claims` from a non-zero count. |

**US-CC-001 DoR: PASSED (9/9 + Dimension-0 PASS + JTBD PASS).**

## US-CC-002 — `/claims` header countered count

| DoR Item | Status | Evidence/Issue |
|---|---|---|
| Problem statement clear, domain language | PASS | "The list header gives her no at-a-glance total of how many claims are contested" — domain language, real surface. |
| User/persona identified | PASS | P-001 (Maria), counter-aware-orientation hat, having drilled into `/claims`. |
| 3+ domain examples with real data | PASS | 3 examples: "(3 countered)" consistent with landing; honest zero + list as slice-06; failed count + rows still render. Real CIDs/peers. |
| UAT in Given/When/Then (3–7) | PASS | 4 scenarios (shows count == landing; honest zero; no re-order/re-weight; degrade without blanking rows). |
| AC derived from UAT | PASS | 6 AC, each maps to a scenario. |
| Right-sized (1–3 days, 3–7 scenarios) | PASS | ~0.25 day (render the SAME count in the `/claims` header). 4 scenarios. |
| Technical notes: constraints/dependencies | PASS | The header render site (`render_claims_page` ~373); single-source-of-truth with the landing; the additive/no-regression gold; depends on US-CC-000/001. |
| Dependencies resolved/tracked | PASS | Depends on US-CC-000 (the count) + US-CC-001 (the copy/shape). The slice-06 `/claims` header + slice-12 per-row flags are shipped. |
| Outcome KPIs with measurable targets | PASS | 100% landing==header consistency; 0 list-order/paging/confidence regression vs slice-06. |

**US-CC-002 DoR: PASSED (9/9 + Dimension-0 PASS + JTBD PASS).**

---

## DoR verdict: PASSED — 3/3 stories pass 9/9; Dimension-0 PASS (1 infra-exempt + 2 with Elevator Pitch); JTBD PASS (2× J-003b + 1× infrastructure-only with rationale); slice has release value.
