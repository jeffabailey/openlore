# Definition of Ready: viewer-peer-counter-aware-counts (slice-19)

> 9-item hard gate per story. Verdict at the bottom. Dimension-0 (Elevator Pitch) and JTBD
> traceability checked first as hard-blocking gates.

## Dimension 0 — Elevator Pitch (BLOCKING, checked first)

| Story | Elevator Pitch present (Before/After/Decision)? | Real entry point? | Concrete output? | Job connection? | Verdict |
|---|---|---|---|---|---|
| US-PC-000 | N/A — `@infrastructure` (`job_id: infrastructure-only`) | — | — | enables US-PC-001/002 | EXEMPT (infra) |
| US-PC-001 | YES — Before/After/Decision triplet | YES — `http://127.0.0.1:<port>/` (`GET /`) | YES — "4 peer claims (1 countered)" rendered text | YES — decide whether to read disagreements on cached peer claims first | PASS |
| US-PC-002 | YES — Before/After/Decision triplet | YES — `http://127.0.0.1:<port>/peer-claims` (`GET /peer-claims`) | YES — "(1 countered)" in the list header beside "Peer Claims" | YES — orient the peer list page (1 contested or 30) at a glance | PASS |

Slice-level check: the slice contains TWO non-`@infrastructure`, user-visible stories
(US-PC-001, US-PC-002) with real decisions → the slice has release value. **PASS.**

## JTBD traceability (hard-blocking)

| Story | `job_id` | Valid? |
|---|---|---|
| US-PC-000 | `infrastructure-only` + `infrastructure_rationale` present (in user-stories.md) | PASS |
| US-PC-001 | `J-003b` (counter-claim awareness — orientation/at-a-glance-count facet) | PASS — J-003b exists in `docs/product/jobs.yaml` (~line 253) |
| US-PC-002 | `J-003b` | PASS |

## US-PC-000 — Resolve the countered-peer-claims count (`@infrastructure`)

| DoR Item | Status | Evidence/Issue |
|---|---|---|
| Problem statement clear, domain language | PASS | "slice-18 answers the OWN version; no read answers 'how many of my cached PEER claims have been countered'" — domain-precise (counter-reference tables, presence, outer `peer_claims`). |
| User/persona identified | PASS | P-001 (Maria) indirectly; plumbing for US-PC-001/002. |
| 3+ domain examples with real data | PASS | 3 examples: 4 peer claims/1 countered (one by both Maria + Rachel) in one read; honest zero; failed read degrades independently. Real CIDs (`bafyTobiasRust`, `bafyRachelSemver`, `bafyTobiasTDD`, `bafyRachelDDD`), real counterers (Maria, Rachel). |
| UAT in Given/When/Then (3–7) | PASS | 4 scenarios (single-aggregate read; counted-once across both ref tables; honest zero; independent degrade). |
| AC derived from UAT | PASS | 8 AC, each maps to a scenario/contract. |
| Right-sized (1–3 days, 3–7 scenarios) | PASS | ~0.5 day (one read = slice-18 SQL with outer table swapped + thread into the summary). 4 scenarios. |
| Technical notes: constraints/dependencies | PASS | The counter-ref data + the open read question (count-only aggregate mirroring `count_countered_own_claims` with outer `peer_claims`) + the 5th `LandingSummary` field + the `.ok()` degrade + the fault-seam pattern. |
| Dependencies resolved/tracked | PASS | Reuses slice-12 counter-ref tables (shipped) + slice-17 `LandingSummary` (shipped) + slice-18 `count_countered_own_claims` SQL + `render_countered` helper (shipped). The open read shape is WD-PC-5 (DESIGN), not a blocker. |
| Outcome KPIs with measurable targets | PASS | Landing read budget grows by EXACTLY 1 (5th count); 0/N failures 5xx; invariant to store size. |

**US-PC-000 DoR: PASSED (9/9 + Dimension-0 exempt + JTBD PASS).**

## US-PC-001 — Landing peer countered count

| DoR Item | Status | Evidence/Issue |
|---|---|---|
| Problem statement clear, domain language | PASS | "slice-18 put own disputed-awareness at the front door; the peer line is still bare" — domain language, real surface, the concrete gap. |
| User/persona identified | PASS | P-001 (Maria), counter-aware-orientation hat, opening the viewer at session start. |
| 3+ domain examples with real data | PASS | 3 examples: 4/1 inline; honest "(0 countered)"; multiply-countered counts once + neutral copy. Real CIDs/counterers/confidence (`0.40`). |
| UAT in Given/When/Then (3–7) | PASS | 6 scenarios (shows count; honest zero; counted-once; no re-weight + own line untouched; degrade; offline). |
| AC derived from UAT | PASS | 8 AC, each maps to a scenario. |
| Right-sized (1–3 days, 3–7 scenarios) | PASS | ~0.25 day (extend the slice-17 summary peer line, reuse `render_countered`). 6 scenarios. |
| Technical notes: constraints/dependencies | PASS | The render extension site (`render_landing` ~677, the peer line), the `render_countered` REUSE (no new helper), the slice-18 own line untouched. |
| Dependencies resolved/tracked | PASS | Depends on US-PC-000 (the count). The slice-17 landing summary + slice-18 `render_countered` are shipped. |
| Outcome KPIs with measurable targets | PASS | Leading indicator of KPI-VIEW-1 (now disputed-state across own AND peer); share that drill into `/peer-claims` from a non-zero count. |

**US-PC-001 DoR: PASSED (9/9 + Dimension-0 PASS + JTBD PASS).**

## US-PC-002 — `/peer-claims` header peer countered count

| DoR Item | Status | Evidence/Issue |
|---|---|---|
| Problem statement clear, domain language | PASS | "The `/peer-claims` header gives no at-a-glance total; slice-18 solved this for `/claims` (own), `/peer-claims` is the symmetric gap" — domain language, real surface. |
| User/persona identified | PASS | P-001 (Maria), counter-aware-orientation hat, having drilled into `/peer-claims`. |
| 3+ domain examples with real data | PASS | 3 examples: "(1 countered)" consistent with landing; honest zero + list as slice-06/07; failed count + rows + flags still render. Real CIDs/counterers. |
| UAT in Given/When/Then (3–7) | PASS | 4 scenarios (shows count == landing; honest zero; no re-order/re-weight; degrade without blanking rows). |
| AC derived from UAT | PASS | 6 AC, each maps to a scenario. |
| Right-sized (1–3 days, 3–7 scenarios) | PASS | ~0.25 day (render the SAME count in the `/peer-claims` header, mirror of slice-18's `/claims` header). 4 scenarios. |
| Technical notes: constraints/dependencies | PASS | The header render site (`render_peer_claims_page` ~1170, `h1 { "Peer Claims" }`); single-source with the landing; the additive/no-regression gold; depends on US-PC-000/001. |
| Dependencies resolved/tracked | PASS | Depends on US-PC-000 (the count) + US-PC-001 (the copy/shape — identical to slice-18). The slice-06/07 `/peer-claims` header + slice-13 per-row flags are shipped. |
| Outcome KPIs with measurable targets | PASS | 100% landing==header consistency; 0 list-order/paging/confidence/origin regression vs slice-06/07. |

**US-PC-002 DoR: PASSED (9/9 + Dimension-0 PASS + JTBD PASS).**

---

## DoR verdict: PASSED — 3/3 stories pass 9/9; Dimension-0 PASS (1 infra-exempt + 2 with Elevator Pitch); JTBD PASS (2× J-003b + 1× infrastructure-only with rationale); slice has release value.
