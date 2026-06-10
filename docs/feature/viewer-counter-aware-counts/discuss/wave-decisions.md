# Wave Decisions: viewer-counter-aware-counts (slice-18) — DISCUSS

> Wave: DISCUSS (lean + ask-intelligent) · Owner: Luna (nw-product-owner) · 2026-06-09
> Feature type: User-facing · JTBD: YES (J-003b counter-claim awareness — orientation/
> at-a-glance-count facet) · UX depth: Lightweight · Walking skeleton: brownfield DELTA
> Brownfield DELTA on slice-17 (the `GET /` landing `LandingSummary`) + slice-06 (the
> `GET /claims` list header), reusing the slice-12 counter-reference data.

This slice ties the shipped counter-flag family (slices 11–14) INTO the front-door
orientation (slice-17): it surfaces, at a glance, **how many of the operator's own claims
have been countered**, beside the own-claims count, on the landing ("12 own claims
(3 countered)") and in the `/claims` list header. It realizes the orientation facet of
**J-003b** (counter-claim awareness) + **KPI-VIEW-1** (time-to-see-store-contents — now
including disputed-claim state). No new product job; no new sub-job; the user-visible stories
trace to the already-validated J-003b; the read-wiring story is `infrastructure-only` with
rationale.

## Migration gate

CLEAR — `docs/product/` SSOT exists (`jobs.yaml`, `kpi-contracts.yaml`, `personas/`).
Proceeded without re-running JTBD; J-003b is validated (slice-03/11/12 DISCUSS; jobs.yaml
sub-job J-003b at ~line 253; changelog 2026-05-27/2026-06-06).

## Scope Assessment: PASS — 3 stories (1 infra + 2 user-visible), 1 bounded context (the viewer's `/` + `/claims` surfaces), 3 integration points, estimated ~0.5–1 day

Carpaccio gate, run BEFORE journey-visualization investment (Phase 1.5):

- **Stories**: 3 (1 infra read-wiring + 2 user-visible: landing + `/claims` header). Within
  ≤10. PASS.
- **Bounded contexts**: 1 — the viewer's `/` + `/claims` surfaces. Extends `viewer-domain`
  (pure — `LandingSummary` gains a countered field; `render_landing` + the `/claims` header
  render it) + `adapter-http-viewer` (effect — `landing_page` / `claims_page` resolve the
  countered count) + at most `ports` / `adapter-duckdb` IF DESIGN elects a count-only
  countered aggregate. All existing; NO new crate. PASS.
- **Walking-skeleton integration points**: 3 — (1) resolve the countered-own-claims count
  (reusing the slice-12 counter-ref tables), (2) thread it into the slice-17 `LandingSummary`
  + the `/claims` header resolution (single source), (3) render "(N countered)" on both
  surfaces. Within ≤5. PASS.
- **Estimated effort**: ~0.5–1 day. PASS.
- **Multiple independent outcomes**: NO — one outcome ("see how much of my own work has been
  disputed, at a glance, on the orientation surfaces"). PASS.
- **Verdict**: RIGHT-SIZED — a thin DELTA extending an existing summary + reusing existing
  counter data. The thing that would make it oversized — rendering counter content/threads in
  the count, a "disputed by N" total, a re-weight, a peer-claims-countered count too, or a
  network seam — is explicitly OUT of scope (WD-CC-1 read-only, WD-CC-2 LOCAL-only, WD-CC-4
  presence-not-total, WD-CC-7 own-claims-only).

## Locked decisions (WD-CC-*)

| # | Decision | Rationale | Status |
|---|---|---|---|
| WD-CC-1 (CARDINAL) | **Read-only / no key**: `/` and `/claims` read the countered count + render it only — never mutate, never hold a key, add no write/compose/sign/subscribe/follow control. The countered count is a COUNT only; render-only text, not a sort/filter/mutating control. | The read-only / key-less viewer is cardinal across slices 06–17. Adding a LOCAL count read + render preserves it exactly (KPI-VIEW-2). | LOCKED |
| WD-CC-2 (CARDINAL) | **LOCAL-only / offline + graceful degrade**: the countered count is a LOCAL aggregate over the indexed counter-reference tables; NO network seam. Both surfaces render fully network-down, referencing only the vendored `/static/htmx.min.js`. If the countered-count read FAILS, the surface shows the missing marker WITHOUT blanking the own-claims count, the other landing counts, the nav hub, or the `/claims` rows — never a 5xx, never blank, never a raw stack trace. | The orientation surfaces must never break because the countered count couldn't be read. Carries KPI-5 / KPI-VIEW-5 / NFR-VIEW-6. The slice-17 `.ok()` per-count degrade + the slice-12 `counter_presence_for(...).unwrap_or_default()` are the precedents. | LOCKED |
| WD-CC-3 (CARDINAL) | **Cheap / no N+1 / invariant to store size**: the countered count is a SMALL FIXED number of aggregate reads per render — ideally ONE count-only aggregate, OR it folds into the existing summary resolution — invariant to store size. The landing's "3 fixed reads" budget grows by AT MOST 1. NO per-claim `counter_presence_for` loop. | Inherits the slice-17 C-4 fixed-read budget + the slice-12 I-LF-8 single-aggregate discipline. A per-row loop is REJECTED. | LOCKED |
| WD-CC-4 (CARDINAL — J-003b accuracy) | **Presence count, never a total / re-weight / verdict**: the countered count is how many own claims have ≥1 counter — a PRESENCE count. A claim countered by 2 peers counts ONCE. It is NEVER a "disputed by N" total, NEVER a re-weight of the own-claims count (the "12" is unchanged), NEVER a verdict. | The shown-never-applied / accuracy cardinal carried from J-003b (slices 11–14). The own-claims count + every claim's confidence stay verbatim; the countered count is additive awareness. | LOCKED |
| WD-CC-5 (OPEN DESIGN QUESTION) | **Countered-count read shape**: (a) a count-only aggregate `count_countered_own_claims()` — `COUNT(DISTINCT c.cid) FROM claims c WHERE c.cid IN (SELECT referenced_cid FROM claim_references WHERE ref_type='counters' UNION SELECT referenced_cid FROM peer_claim_references WHERE ref_type='counters')` (mirrors slice-17's `count_active_peer_subscriptions` count-only decision); OR (b) reuse the slice-12 `counter_presence_for(all_own_cids).len()` (zero new port surface; materializes every own cid + the presence set to count). The PRODUCT contract is a SINGLE aggregate read for the countered count, invariant to store size (WD-CC-3), either way. | Surfaced as an open question per the brief — do not over-specify. RECOMMEND the count-only aggregate for SYMMETRY (the landing's other counts are count-only) + CHEAPNESS (avoids materializing the own-cid list + presence set) — mirroring slice-17 ADR-054 D3 — but DESIGN decides. If DESIGN adds the count-only variant, it is a read-only method on `StoreReadPort`; `adapter-duckdb` gains ONE aggregate impl; workspace stays 21. | OPEN — DESIGN resolves |
| WD-CC-6 (Missing ≠ zero) | The countered count is Option-shaped (extend the slice-17 `LandingSummary` with a countered field, or a parallel Option): Some(0) renders "(0 countered)" (honest zero), None renders the slice-17 `MISSING_COUNT_MARKER` ("—"). A fabricated 0 on a failed read is FORBIDDEN; the failure degrades INDEPENDENTLY of the sibling counts (slice-17 ADR-054 D2 `.ok()`). | A fabricated "(0 countered)" on a failed read would mislead "nothing disputed". The distinction is a product AC, type-level via Option (slice-17 precedent). | LOCKED |
| WD-CC-7 (SCOPE DECISION — own-claims-only the core; peer-claims-countered deferred) | This slice surfaces the **countered-OWN-claims** count as the load-bearing orientation signal ("how much of MY work has been disputed"). Whether to ALSO add a "(N countered)" to the PEER-claims count is RECOMMENDED DEFERRED — peer-claims-countered is arguably less central to "how much of my work drew pushback", and adding it widens the read + the render to a second count. Surfaced for the user, not silently dropped. | Own-claims-countered is the headline J-003b orientation signal. Peer-claims-countered can follow in a recommended slice if dogfood shows demand. Keeping it out holds the ≤1-day budget + the single-count clarity. | DECISION SURFACED — recommend own-claims-only |
| WD-CC-8 | **Single source for both surfaces**: the landing "(N countered)" and the `/claims` header "(N countered)" come from the SAME US-CC-000 read — one number, rendered on both surfaces. | Consistency between the two orientation surfaces is a product invariant (a gold test pins landing==header). Two independent reads could drift. | LOCKED |
| WD-CC-9 | **Additive on `/claims` — no list regression**: the header count is rendered in the `/claims` header ONLY; the slice-06 `list_claims` ordering/paging/count and the slice-12 per-row presence flags are UNTOUCHED. The header count does not re-order, filter, group, re-page, or re-weight the list. | The list stays a faithful, un-reordered view (slice-12 I-LF-2). The header total is orientation, not a transform. | LOCKED |
| WD-CC-10 | **Anti-misread / neutral copy**: "(N countered)" reads as NEUTRAL disputed-claim awareness — no penalty, deduction, score, "refuted", "false", or "disputed by N" language. The own-claims count stands unchanged beside it. | Reuses the slice-14 anti-misread sensibility. A countered claim is contested, not wrong; the count must not read as a penalty. | LOCKED |
| WD-CC-11 | **No new crate; no new route; no new KPI ID; no new persisted type; loopback-only bind.** Extend `viewer-domain` + `adapter-http-viewer` (+ at most `ports`/`adapter-duckdb` if DESIGN elects the count-only aggregate). Workspace stays 21. Realizes inherited KPIs (KPI-VIEW-1 + guardrails). | Matches slice-08–17 (no new KPI/crate/route per facet slice). The count is computed per-request, never persisted (BR-VIEW-2 / I-VIEW-1/4). | LOCKED |
| WD-CC-12 | **Persona: P-001 (Maria), counter-aware-orientation hat** (the front-door orientation hat from slice-17 + the counter-claim-scanner hat from slice-12, combined for the at-a-glance disputed-count behavior). To be appended to `docs/product/personas/senior-engineer-solo-builder.yaml` (changelog 2026-06-09, slice-18). | Seeing the disputed-claim TOTAL at the orientation surfaces is a distinct first-touch behavior from scanning per-row flags (slice-12) or front-door navigation (slice-17). | LOCKED |

## Open question for DESIGN (carried forward)

- **WD-CC-5** — the countered-count read shape (count-only aggregate vs
  `counter_presence_for(all_own_cids).len()`). PRODUCT contract: a single aggregate read,
  invariant to store size. Recommend the count-only aggregate (symmetry + cheapness, per
  slice-17 ADR-054 D3). Non-blocking for DoR.
- **WD-CC-7** — own-claims-only (recommended) vs also adding peer-claims-countered. Recommend
  own-claims-only as the core; peer optional/deferred. Surfaced for user confirmation.

## DESIGN resolutions (2026-06-09 — Morgan, nw-solution-architect · ADR-055)

| # | Resolution | Decision |
|---|---|---|
| WD-CC-5 (RESOLVED) | **Count-only aggregate `count_countered_own_claims()`** chosen over `counter_presence_for(own_cids).len()`. SQL (parameter-free, injection-safe): `SELECT COUNT(DISTINCT c.cid) FROM claims c WHERE c.cid IN (SELECT referenced_cid FROM claim_references WHERE ref_type='counters' UNION SELECT referenced_cid FROM peer_claim_references WHERE ref_type='counters')`. Rationale: SYMMETRY with the three count-only landing reads + CHEAPNESS (avoids materializing the own-cid list + presence set), mirroring slice-17 ADR-054 D3. Presence count by construction (de-duped `UNION` IN-set + `COUNT(DISTINCT)` — a claim countered N times counts once, no JOIN-fanout). Own-only by query shape (outer table `claims`). A read-only method on `StoreReadPort`; `adapter-duckdb` gains ONE aggregate impl; workspace stays 21. | ADR-055 D1 |
| WD-CC-7 (RESOLVED) | **Own-claims-only** confirmed. Peer-claims-countered DEFERRED (a recommended additive sibling — a second count-only aggregate over `peer_claims` + a second `Option` field + a second parenthetical — if dogfood shows demand). The count-only query shape makes the deferred sibling clean to add later. | ADR-055 D3 (Alternatives) |
| WD-CC-6 (DESIGN form) | **Additive 4th `Option<usize>` field** `countered_own_claims` on the slice-17 `LandingSummary` (NOT a separate view-model); the `/claims` header takes the bare `Option<usize>` as a `render_claims_page` param. `0 ≠ missing` type-level; per-count independent `.ok()` degrade. | ADR-055 D2/D4 |
| WD-CC-8 (DESIGN form) | **Shared pure `render_countered(Option<usize>) -> String` helper** drives "(N countered)" on BOTH surfaces — single source for the COPY; both routes resolve the count independently per render (read method + render helper are the single source, not a cached value). | ADR-055 D3 |
| xtask SQL rule | **`no_cross_table_join_elides_author` is GREEN by construction** — the new SQL names `claim_references` + `peer_claim_references` (no `peer_claims` WHOLE WORD — the `_references` suffix fails the word boundary), so `is_cross_store` is false and the classifier returns `None` (the exact reason slice-12's `counter_presence_for` is GREEN). The index-store `mentions_aggregation` variant scans `adapter-index-store` only, not `adapter-duckdb`. xtask boundary UNCHANGED. | component-boundaries §5 |

> NO new crate / route / KPI / persisted type introduced (WD-CC-11 honored). Workspace stays 21.

## Risks logged

### R-CC-1 (RISK) — No DIVERGE wave for slice-18

No `diverge/` directory — consistent with all prior OpenLore slices. NON-BLOCKING: J-003b
(counter-claim awareness) is validated in `docs/product/jobs.yaml`; the counter-flag family
(slices 11–14) + the landing summary (slice-17) are SHIPPED; the counter-reference tables +
the slice-17 `LandingSummary` already exist. No design-direction ambiguity.

### R-CC-2 (RISK) — The countered-count read fails and 5xxes / blanks the orientation surface

Mitigated by WD-CC-2/WD-CC-6 + US-CC-000/001/002 AC (independent graceful degrade is a HARD
product commitment: a failed read renders the missing marker WITHOUT blanking the sibling
counts/rows, never a 5xx — NFR-VIEW-6; slice-17 `.ok()` per-count degrade is the model). A
behavioral test seeds an unreadable countered count and asserts both surfaces still render at
200.

### R-CC-3 (RISK) — The countered count becomes an N+1 (per-claim `counter_presence_for` loop)

Mitigated by WD-CC-3/WD-CC-5 + US-CC-000 AC (a FIXED aggregate read; the landing budget grows
by at most 1; a per-row loop is REJECTED). A `@property`/gold test asserts the countered-count
read is invariant to store size.

### R-CC-4 (RISK) — A twice-countered claim is double-counted; "(N countered)" reads as a "by N" total

Mitigated by WD-CC-4 + US-CC-000/001 AC (presence count — `COUNT(DISTINCT …)` / a set
intersection; a claim countered by N peers counts ONCE). A domain example + scenario pin
"(1 countered)" for a claim countered by both Rachel and Tobias.

### R-CC-5 (RISK) — The count re-weights the own-claims count or reads as a penalty

Mitigated by WD-CC-4/WD-CC-10 + US-CC-001 AC (the own-claims "12" is unchanged; the copy is
neutral disputed-claim awareness — no penalty/deduction/score/"refuted"/"false"; confidence
stays verbatim). The slice-14 anti-misread sensibility is reused.

### R-CC-6 (RISK) — The landing count and the `/claims` header count drift

Mitigated by WD-CC-8 + US-CC-002 AC (single source — both surfaces render the SAME US-CC-000
number). A gold test asserts landing "(N countered)" == `/claims` header "(N countered)" for
the same store.

### R-CC-7 (RISK) — The `/claims` header count re-orders/filters/re-weights the list

Mitigated by WD-CC-9 + US-CC-002 AC (additive — the header count is rendered in the header
only; the slice-06 ordering/paging/count + the slice-12 per-row flags are untouched). A gold
test asserts list byte-identity vs the no-header-count baseline.

### R-CC-8 (RISK) — Scope creep: peer-claims-countered added unasked

Mitigated by WD-CC-7 (own-claims-only is the recommended core; peer-claims-countered is
surfaced as an explicit deferred scope decision, not silently included). Holds the ≤1-day
budget.

## DoR verdict: PASSED (9/9 for all 3 stories; Dimension 0 PASS — 1 infra-exempt + 2 with Elevator Pitch; JTBD PASS — 2× J-003b + 1× infrastructure-only with rationale)

See `dor-checklist.md`.

## Handoff readiness

DISCUSS artifacts complete (lean set): `requirements.md`, `user-stories.md`,
`acceptance-criteria.md`, `outcome-kpis.md`, `dor-checklist.md`, `wave-decisions.md`; the
feature-delta.md DISCUSS section appended. Persona hat to be appended at finalize. Ready for
DESIGN (solution-architect) once peer review approves. No code written; no DESIGN performed.
DESIGN inherits two open questions (WD-CC-5 read shape; WD-CC-7 own-vs-peer scope).

## DIVERGE note

No DIVERGE artifacts exist for this slice (`docs/feature/viewer-counter-aware-counts/diverge/`
absent) — consistent with all prior OpenLore slices. Journey work is grounded in the validated
J-003b counter-claim-awareness facet, the shipped counter-flag family (slices 11–14), and the
shipped slice-17 landing summary.

## SSOT updates to apply (at finalize — not written mid-wave)

- `docs/product/jobs.yaml` — append a changelog entry (2026-06-09) noting slice-18 traces to
  J-003b (orientation / at-a-glance-count facet; no new job/sub-job; surfaces the
  countered-own-claims count on `/` + `/claims`).
- `docs/product/personas/senior-engineer-solo-builder.yaml` — append the
  counter-aware-orientation hat (2026-06-09, slice-18).
- `docs/product/kpi-contracts.yaml` — append a `last_updated` note (slice-18 realizes
  KPI-VIEW-1 + guardrails on the counter-aware orientation facet; no new KPI minted).
