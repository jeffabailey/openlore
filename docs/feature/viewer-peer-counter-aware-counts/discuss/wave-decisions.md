# Wave Decisions: viewer-peer-counter-aware-counts (slice-19) — DISCUSS

> Wave: DISCUSS (lean + ask-intelligent) · Owner: Luna (nw-product-owner) · 2026-06-10
> Feature type: User-facing · JTBD: YES (J-003b counter-claim awareness — orientation/
> at-a-glance-count facet) · UX depth: Lightweight · Walking skeleton: brownfield DELTA
> Brownfield DELTA on slice-17 (the `GET /` landing `LandingSummary` PEER line) + slice-06/07
> (the `GET /peer-claims` list header), reusing the slice-12 counter-reference data + the
> slice-18 `render_countered` helper + count-only-aggregate pattern + fault-seam pattern.

This slice is the deferred peer sibling of slice-18 (WD-CC-7: "own-claims-only the core;
peer-claims-countered a recommended additive sibling … if dogfood shows demand"; ADR-055 noted
"the count-only query shape makes the deferred sibling clean to add later"). It extends the
SAME counter-aware-count pattern to the PEER line: it surfaces, at a glance, **how many of the
operator's CACHED PEER claims have been countered**, beside the peer-claims count, on the
landing ("4 peer claims (1 countered)") and in the `/peer-claims` list header. It realizes the
orientation facet of **J-003b** (counter-claim awareness) + **KPI-VIEW-1**
(time-to-see-store-contents — now disputed-claim state across BOTH own AND peer claims). No new
product job; no new sub-job; the user-visible stories trace to the already-validated J-003b; the
read-wiring story is `infrastructure-only` with rationale. This is the own+peer COMPLETION of
counter-aware counts — JUST the peer count (the own count shipped in slice-18); no third
dimension.

## Migration gate

CLEAR — `docs/product/` SSOT exists (`jobs.yaml`, `kpi-contracts.yaml`, `personas/`).
Proceeded without re-running JTBD; J-003b is validated (slice-03/11/12/18 DISCUSS; jobs.yaml
sub-job J-003b at ~line 253).

## Scope Assessment: PASS — 3 stories (1 infra + 2 user-visible), 1 bounded context (the viewer's `/` + `/peer-claims` surfaces), 3 integration points, estimated ~0.5–1 day

Carpaccio gate, run BEFORE journey-visualization investment (Phase 1.5). A near-exact mirror of
slice-18 — expected PASS, confirmed PASS:

- **Stories**: 3 (1 infra read-wiring + 2 user-visible: landing peer line + `/peer-claims`
  header). Within ≤10. PASS.
- **Bounded contexts**: 1 — the viewer's `/` + `/peer-claims` surfaces. Extends `viewer-domain`
  (pure — `LandingSummary` gains a 5th `countered_peer_claims` field; `render_landing` renders
  it on the peer line; `render_peer_claims_page` takes the bare `Option<usize>` for its header —
  all via the EXISTING `render_countered` helper) + `adapter-http-viewer` (effect —
  `landing_page` / `peer_claims_page` resolve the countered-peer count) + at most `ports` /
  `adapter-duckdb` IF DESIGN elects a count-only countered-peer aggregate. All existing; NO new
  crate. PASS.
- **Walking-skeleton integration points**: 3 — (1) resolve the countered-peer-claims count
  (reusing the slice-12 counter-ref tables; the slice-18 SQL with outer table swapped to
  `peer_claims`), (2) thread it into the slice-17 `LandingSummary` + the `/peer-claims` header
  resolution (single source), (3) render "(N countered)" on both surfaces via the existing
  `render_countered`. Within ≤5. PASS.
- **Estimated effort**: ~0.5–1 day (cheaper than slice-18 — the render helper, the fault-seam
  pattern, and the SQL shape already exist; only the outer table + the 5th field + two render
  sites are new). PASS.
- **Multiple independent outcomes**: NO — one outcome ("see how much of my cached peer material
  has been disputed, at a glance, on the orientation surfaces"). PASS.
- **Verdict**: RIGHT-SIZED — a thin DELTA mirroring slice-18 onto peer claims, reusing the
  slice-18 helper + counter data + fault-seam + aggregate pattern. The thing that would make it
  oversized — rendering counter content/threads in the count, a "disputed by N" total, a
  re-weight, a third dimension, or a network seam — is explicitly OUT of scope (WD-PC-1
  read-only, WD-PC-2 LOCAL-only, WD-PC-4 presence-not-total, BR-PC-4 peer-only).

## Locked decisions (WD-PC-*)

| # | Decision | Rationale | Status |
|---|---|---|---|
| WD-PC-1 (CARDINAL) | **Read-only / no key**: `/` and `/peer-claims` read the countered-peer count + render it only — never mutate, never hold a key, add no write/compose/sign/subscribe/follow control. The countered-peer count is a COUNT only; render-only text, not a sort/filter/mutating control. | The read-only / key-less viewer is cardinal across slices 06–18. Adding a LOCAL count read + render preserves it exactly (KPI-VIEW-2). | LOCKED |
| WD-PC-2 (CARDINAL) | **LOCAL-only / offline + graceful degrade**: the countered-peer count is a LOCAL aggregate over the indexed counter-reference tables; NO network seam. Both surfaces render fully network-down, referencing only the vendored `/static/htmx.min.js`. If the countered-peer-count read FAILS, the surface shows the missing marker WITHOUT blanking the peer-claims count, the other landing counts, the nav hub, or the `/peer-claims` rows + slice-13 per-row flags — never a 5xx, never blank, never a raw stack trace. | The orientation surfaces must never break because the countered-peer count couldn't be read. Carries KPI-5 / KPI-VIEW-5 / NFR-VIEW-6. The slice-17 `.ok()` per-count degrade + the slice-18 fault-seam are the precedents. | LOCKED |
| WD-PC-3 (CARDINAL) | **Cheap / no N+1 / invariant to store size**: the countered-peer count is a SMALL FIXED number of aggregate reads per render — ideally ONE count-only aggregate (a 5th sibling of `count_claims` / `count_peer_claims` / `count_active_peer_subscriptions` / `count_countered_own_claims`) — invariant to store size. The landing's "4 fixed reads" budget grows by EXACTLY 1 (a 5th count read); the `/peer-claims` header read grows by 1. NO per-claim `counter_presence_for` loop. | Inherits the slice-17 C-4 fixed-read budget + the slice-12 I-LF-8 single-aggregate discipline + the slice-18 ADR-055 D1 count-only decision. A per-row loop is REJECTED. | LOCKED |
| WD-PC-4 (CARDINAL — J-003b accuracy) | **Presence count, never a total / re-weight / verdict**: the countered-peer count is how many cached peer claims have ≥1 counter — a PRESENCE count. A peer claim countered by 2 counterers (the operator + another peer, or two peers) counts ONCE. It is NEVER a "disputed by N" total, NEVER a re-weight of the peer-claims count (the "4" is unchanged), NEVER a verdict. | The shown-never-applied / accuracy cardinal carried from J-003b (slices 11–14, 18). The peer-claims count + every peer claim's confidence stay verbatim; the countered count is additive awareness. | LOCKED |
| WD-PC-5 (OPEN DESIGN QUESTION) | **Countered-peer-count read shape**: the EXACT MIRROR of slice-18's `count_countered_own_claims` with the OUTER table swapped from `claims` to `peer_claims`: a count-only aggregate `count_countered_peer_claims()` — `SELECT COUNT(DISTINCT p.cid) FROM peer_claims p WHERE p.cid IN (SELECT referenced_cid FROM claim_references WHERE ref_type='counters' UNION SELECT referenced_cid FROM peer_claim_references WHERE ref_type='counters')` (a 5th count-only sibling, mirroring the slice-18 ADR-055 D1 decision); OR reuse the slice-12 `counter_presence_for(all_peer_cids).len()` (zero new port surface; materializes every peer cid + the presence set to count). The PRODUCT contract is a SINGLE aggregate read for the countered-peer count, invariant to store size (WD-PC-3), either way. | Surfaced as the open question per the brief — but the exact read is the natural slice-18 mirror. RECOMMEND the count-only aggregate (the 5th sibling) for SYMMETRY (the landing's other four counts are count-only) + CHEAPNESS (avoids materializing the peer-cid list + presence set) — mirroring slice-18 ADR-055 D1 — but DESIGN decides. If DESIGN adds the count-only variant, it is a read-only method on `StoreReadPort`; `adapter-duckdb` gains ONE aggregate impl; workspace stays 21. Note: the inner `UNION` IN-set is IDENTICAL to slice-18's — only the outer table differs — so the de-dup/presence-once semantics are inherited verbatim. | OPEN — DESIGN resolves |
| WD-PC-6 (Missing ≠ zero) | The countered-peer count is Option-shaped (extend the slice-17 `LandingSummary` with a 5th `countered_peer_claims` field): Some(0) renders "(0 countered)" (honest zero), None renders the slice-17 `MISSING_COUNT_MARKER` ("—") inside the parenthetical ("(— countered)"). A fabricated 0 on a failed read is FORBIDDEN; the failure degrades INDEPENDENTLY of the sibling counts (slice-17 ADR-054 D2 / slice-18 ADR-055 D4 `.ok()`). | A fabricated "(0 countered)" on a failed read would mislead "nothing disputed". The distinction is a product AC, type-level via Option (slice-17/18 precedent). | LOCKED |
| WD-PC-7 (SCOPE — peer-only; this is the own+peer COMPLETION) | This slice adds ONLY the countered-PEER-claims count — the deferred sibling of slice-18. It is the own+peer COMPLETION: own shipped in slice-18, peer ships here. There is NO third dimension. The slice-18 own-claims countered surfaces (landing own line + `/claims` header) are UNTOUCHED. | Confirms the brief's scope note: "JUST the peer count (the own count shipped in slice-18); no third dimension". Own-vs-peer is by outer-table shape (`claims` vs `peer_claims`); the two reads are independent siblings. | LOCKED |
| WD-PC-8 | **Single source for both surfaces**: the landing peer "(N countered)" and the `/peer-claims` header "(N countered)" come from the SAME US-PC-000 read — one number, rendered on both surfaces via the SAME `render_countered` helper. | Consistency between the two orientation surfaces is a product invariant (a gold test pins landing==header). Two independent reads could drift. Mirrors slice-18 WD-CC-8. | LOCKED |
| WD-PC-9 | **Additive on `/peer-claims` — no list regression**: the header count is rendered in the `/peer-claims` header ONLY; the slice-06/07 `list_peer_claims` ordering/paging/count, the slice-13 per-row presence flags, and the per-row peer origin are UNTOUCHED. The header count does not re-order, filter, group, re-page, or re-weight the list. | The peer list stays a faithful, un-reordered federated view (slice-13 no-regression). The header total is orientation, not a transform. Mirrors slice-18 WD-CC-9. | LOCKED |
| WD-PC-10 | **Anti-misread / neutral copy via the SHARED helper**: "(N countered)" reads as NEUTRAL disputed-claim awareness — no penalty, deduction, score, "refuted", "false", or "disputed by N" language. The peer-claims count stands unchanged beside it. Rendered via the SAME pure `render_countered(Option<usize>)` helper slice-18 established (single SSOT copy site — NO new render helper). | Reuses the slice-14 / slice-18 anti-misread sensibility, already proven neutral by the slice-18 `render_countered` unit tests. A countered peer claim is contested, not wrong; the count must not read as a penalty. | LOCKED |
| WD-PC-11 | **No new crate; no new route; no new KPI ID; no new persisted type; loopback-only bind.** Extend `viewer-domain` + `adapter-http-viewer` (+ at most `ports`/`adapter-duckdb` if DESIGN elects the count-only aggregate). Workspace stays 21. Realizes inherited KPIs (KPI-VIEW-1 + guardrails). | Matches slice-08–18 (no new KPI/crate/route per facet slice). The count is computed per-request, never persisted (BR-VIEW-2 / I-VIEW-1/4). | LOCKED |
| WD-PC-12 | **Persona: P-001 (Maria), counter-aware-orientation hat** (the SAME hat slice-18 added; slice-19 extends it from own claims to cached peer claims — the at-a-glance disputed-count behavior now spans both). To be noted in `docs/product/personas/senior-engineer-solo-builder.yaml` (changelog 2026-06-10, slice-19). | Seeing the disputed cached-peer-claim total at the orientation surfaces is the symmetric completion of the slice-18 own-claim behavior under the SAME hat — not a new persona facet, an extension. | LOCKED |

## Open question for DESIGN (carried forward)

- **WD-PC-5** — the countered-peer-count read shape (count-only aggregate `count_countered_peer_claims`
  — the slice-18 mirror with outer table `peer_claims` — vs `counter_presence_for(all_peer_cids).len()`).
  PRODUCT contract: a single aggregate read, invariant to store size. Recommend the count-only
  aggregate (the 5th sibling; symmetry + cheapness, per slice-18 ADR-055 D1). Non-blocking for
  DoR. The inner `UNION` IN-set is identical to slice-18's — only the outer table differs.

## Cardinal decisions (summary for the parent agent)

1. **Read-only / no key** (WD-PC-1) — count + render only; no mutation, no key.
2. **LOCAL-only / offline + independent graceful degrade** (WD-PC-2) — LOCAL aggregate, renders
   offline, a failed read → missing marker without blanking siblings, never a 5xx.
3. **Cheap / no N+1 / fixed aggregate read** (WD-PC-3) — a 5th count-only sibling; landing budget
   grows by exactly 1; invariant to store size.
4. **Presence count, never a "by N" total / re-weight / verdict** (WD-PC-4) — countered by N
   counterers counts ONCE; the "4" is unchanged.
5. **Missing ≠ zero** (WD-PC-6) — Some(0) → "(0 countered)", None → "(— countered)"; no fabricated 0.
6. **Single source** (WD-PC-8) — landing peer line == `/peer-claims` header, one read, the shared
   `render_countered` helper.
7. **Own+peer completion, peer-only, no third dimension** (WD-PC-7 / BR-PC-4) — own shipped in
   slice-18 and UNTOUCHED; this is JUST the peer count.

## Risks logged

### R-PC-1 (RISK) — No DIVERGE wave for slice-19

No `diverge/` directory — consistent with all prior OpenLore slices (incl. slice-18).
NON-BLOCKING: J-003b (counter-claim awareness) is validated in `docs/product/jobs.yaml`; the
counter-flag family (slices 11–14), the landing summary (slice-17), and the slice-18 own mirror
(`count_countered_own_claims` + `render_countered` + the 4th `LandingSummary` field) are SHIPPED.
No design-direction ambiguity — this is an explicit slice-18-deferred sibling.

### R-PC-2 (RISK) — The countered-peer-count read fails and 5xxes / blanks the orientation surface

Mitigated by WD-PC-2/WD-PC-6 + US-PC-000/001/002 AC (independent graceful degrade is a HARD
product commitment: a failed read renders the missing marker WITHOUT blanking the sibling
counts/rows/flags, never a 5xx — NFR-VIEW-6; the slice-17 `.ok()` per-count degrade + the
slice-18 fault-seam are the model). A behavioral test seeds an unreadable countered-peer count
and asserts both surfaces still render at 200.

### R-PC-3 (RISK) — The countered-peer count becomes an N+1 (per-claim `counter_presence_for` loop)

Mitigated by WD-PC-3/WD-PC-5 + US-PC-000 AC (a FIXED aggregate read; the landing budget grows
by exactly 1; a per-row loop is REJECTED). A `@property`/gold test asserts the countered-peer-count
read is invariant to store size.

### R-PC-4 (RISK) — A multiply-countered peer claim is double-counted; "(N countered)" reads as a "by N" total

Mitigated by WD-PC-4/WD-PC-5 + US-PC-000/001 AC (presence count — the de-duped `UNION` IN-set +
`COUNT(DISTINCT)`, identical to slice-18; a peer claim countered by N counterers counts ONCE). A
domain example + scenario pin "(1 countered)" for a peer claim countered by both Maria and Rachel.

### R-PC-5 (RISK) — The count re-weights the peer-claims count or reads as a penalty

Mitigated by WD-PC-4/WD-PC-10 + US-PC-001 AC (the peer-claims "4" is unchanged; the copy is
neutral disputed-claim awareness — no penalty/deduction/score/"refuted"/"false"; confidence stays
verbatim). The slice-18 `render_countered` helper is reused — already unit-tested neutral.

### R-PC-6 (RISK) — The landing peer count and the `/peer-claims` header count drift

Mitigated by WD-PC-8 + US-PC-002 AC (single source — both surfaces render the SAME US-PC-000
number via the SAME helper). A gold test asserts landing peer "(N countered)" == `/peer-claims`
header "(N countered)" for the same store.

### R-PC-7 (RISK) — The `/peer-claims` header count re-orders/filters/re-weights the list

Mitigated by WD-PC-9 + US-PC-002 AC (additive — the header count is rendered in the header only;
the slice-06/07 ordering/paging/count + the slice-13 per-row flags + the peer origin are
untouched). A gold test asserts list byte-identity vs the no-header-count baseline.

### R-PC-8 (RISK) — Scope creep: a third dimension or re-touching the slice-18 own count

Mitigated by WD-PC-7 / BR-PC-4 (this slice adds JUST the peer count; the slice-18 own surfaces
are UNTOUCHED; there is no third dimension). Holds the ≤1-day budget. A gold test asserts the
slice-18 own line + `/claims` header still render "(N countered)" unchanged.

### R-PC-9 (RISK) — The new SQL trips the xtask anti-merging `no_cross_table_join_elides_author` rule

Mitigated by inheritance: the slice-18 mirror SQL names `claim_references` + `peer_claim_references`
(NOT `peer_claims` as a WHOLE WORD in the inner IN-set — the `_references` suffix fails the word
boundary). The OUTER `peer_claims` table reference is the one new wrinkle vs slice-18 — DESIGN must
confirm the classifier's behavior with `peer_claims` in the outer FROM (slice-18's outer was
`claims`). Flagged for DESIGN as a constraint to verify (expected GREEN — it is a single-table
SELECT with a subquery membership test, no merging JOIN/GROUP BY across stores; the count is over
ONE store's own peer-claim cids). Non-blocking for DoR.

## DoR verdict: PASSED (9/9 for all 3 stories; Dimension 0 PASS — 1 infra-exempt + 2 with Elevator Pitch; JTBD PASS — 2× J-003b + 1× infrastructure-only with rationale)

See `dor-checklist.md`.

## Handoff readiness

DISCUSS artifacts complete (lean set): `requirements.md`, `user-stories.md`,
`acceptance-criteria.md`, `outcome-kpis.md`, `dor-checklist.md`, `wave-decisions.md`; the
feature-delta.md DISCUSS section appended. Persona hat extension to be noted at finalize. Ready
for DESIGN (solution-architect) once peer review approves. No code written; no DESIGN performed.
DESIGN inherits one open question (WD-PC-5 read shape — expected to mirror slice-18 ADR-055 D1
with outer table `peer_claims`) and one constraint to verify (R-PC-9 xtask rule with `peer_claims`
in the outer FROM).

## DIVERGE note

No DIVERGE artifacts exist for this slice (`docs/feature/viewer-peer-counter-aware-counts/diverge/`
absent) — consistent with all prior OpenLore slices. Journey work is grounded in the validated
J-003b counter-claim-awareness facet, the shipped counter-flag family (slices 11–14), the shipped
slice-17 landing summary, and the shipped slice-18 own mirror (the explicit source of this deferred
sibling, WD-CC-7).

## SSOT updates to apply (at finalize — not written mid-wave)

- `docs/product/jobs.yaml` — append a changelog entry (2026-06-10) noting slice-19 traces to
  J-003b (orientation / at-a-glance-count facet; no new job/sub-job; surfaces the
  countered-peer-claims count on the `/` peer line + `/peer-claims`, completing the slice-18
  own+peer counter-aware orientation).
- `docs/product/personas/senior-engineer-solo-builder.yaml` — note the counter-aware-orientation
  hat now spans cached peer claims as well as own claims (2026-06-10, slice-19).
- `docs/product/kpi-contracts.yaml` — append a `last_updated` note (slice-19 realizes KPI-VIEW-1
  + guardrails on the peer counter-aware orientation facet; no new KPI minted).
