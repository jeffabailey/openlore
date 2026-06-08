# Wave decisions: viewer-counter-flags-score-surface (slice-14) — DISCUSS

> Owner: Luna (nw-product-owner) · 2026-06-08 · Wave: DISCUSS (lean)

## Scope Assessment: PASS — 2 stories, 1 context, estimated ~1 day

Elephant-Carpaccio gate run in Phase 1.5, BEFORE journey investment (per the brief; expect PASS
on a thin DELTA over shipped surfaces). Right-sized. Full signal table in `feature-delta.md`
§"Scope assessment". This is the slice-13-recommended follow-up that completes J-003b across the
LAST viewer surface. The ONLY thing distinguishing it from a slice-12/13 copy is the
scoring-semantics nuance (sum-to-weight + anti-misread), handled as one extra AC + one extra KPI +
the anti-misread copy — NOT extra stories.

## D-14-1 — REUSE-ONLY scope: NO new crate, route, read-method, or render fn

Confirmed and LOCKED. slice-14 REUSES verbatim:

- `StoreReadPort::counter_presence_for(&[String]) -> HashSet<String>` (slice-12 / ADR-048) — the
  same batch read slice-13 reused across three handlers. The `/score` handler collects every
  `Contribution.cid` across every `WeightedPairing` in `ScoreState::Scored { view }` → ONE call.
  **NO new read method, NO new SQL.**
- `render_countered_link(cid, is_countered)` + `COUNTERED_PRESENCE_FLAG = "Countered"` (slice-13
  unified SSOT) — the contribution-row render calls it directly. **NO new render fn, NO new string.**
- The existing `GET /score?contributor=<did>` route (slice-09). **NO new route.**
- Workspace stays **21 members**. Functional paradigm (ADR-007). **NO new crate.**

The pure render stays a TOTAL function of `(ScoreState, presence)` — mirror the slice-12/13
`from_row_with_presence` projection seam (presence set built in the effect shell). This is C-7/C-8
and the defining property of the slice.

## D-14-2 — Sum-to-weight orthogonality is the CARDINAL (C-9 / I-CF-9)

The load-bearing distinction from slices 12/13. `/score` carries SCORING SEMANTICS, so the flag
must be provably ORTHOGONAL to the score:

- **slice-09 CARDINAL preserved**: the per-claim subtotals STILL sum to the displayed pairing
  weight with the flag present (both project the SAME unchanged `WeightedPairing` —
  `render_score_breakdown` doc-comment ~line 1935; the viewer NEVER recomputes confidence/bonuses/
  buckets, it PROJECTS the reused `scoring::WeightedView`).
- **Shown, never applied**: adding the flag changes NO displayed weight, confidence, bonus,
  subtotal, headline total, bucket, ranking, or row order. A countered claim contributes its FULL
  original weight; the scoring math is intentionally counter-agnostic.
- Re-asserted as explicit ACs: **AC-SCORE-SUMWEIGHT** (subtotals sum to weight on a FLAGGED
  breakdown) + **AC-SCORE-BYTEID** (byte-identity vs the slice-09 baseline with markers elided —
  the slice-12/13 baseline+marker-elision tactic). LOCKED as release-blocking guardrails.

## D-14-3 — Anti-misread copy is a first-class deliverable (AC + KPI)

A reader must not misread the flag as "this counter lowered the contributor's score." Decision:
the breakdown carries a SHORT, NEUTRAL legend stating the flag is orthogonal to the score — *this
contribution's claim has been disagreed with elsewhere; shown for you to judge; it does NOT lower
the contributor's score.* Constrained by **AC-SCORE-ANTIMISREAD** (never "disputed"/"refuted"/
"false"/"penalty"/"deduction"/"lowered"/"disputed score"; reuse the slice-11 verdict-word
blocklist) and tracked as outcome-KPI #2 (comprehension). The legend is the ONLY genuinely NEW
artifact in the slice (a render-only constant; DESIGN owns exact wording within the AC). LOCKED.

## D-14-4 — No new KPI ID; realize inherited KPIs on the LAST facet

Matching slices 08–13, slice-14 mints NO new KPI ID. It STRENGTHENS the READ side of KPI-FED-3 on
the scoring surface, EXTENDS KPI-VIEW-1, and carries the guardrails (KPI-VIEW-2 read-only,
KPI-AV-2/GRAPH-2 anti-merging, **KPI-GRAPH-3 reproduce-by-hand / sum-to-weight UNCHANGED**, KPI-4
verbatim, KPI-5/VIEW-5/HX-G* local-first/offline/parity, ADR-048 N+1 guard). Detail in
`outcome-kpis.md`.

## D-14-5 — Persona: EXTEND the slice-12/13 counter-claim-scanner hat (no new hat)

The `/score` surface is the SAME scanning behavior on one more surface, so the counter-claim-scanner
hat is EXTENDED (the scoring surface + the anti-misread facet appended), not minted anew. To be
appended to `docs/product/personas/senior-engineer-solo-builder.yaml` (changelog 2026-06-08,
slice-14).

## R-14-1 (RISK) — No DIVERGE wave for slice-14

No `diverge/` directory for this feature. Per the workflow, recorded as a NON-BLOCKING risk: the
job (J-003b) is already validated in `docs/product/jobs.yaml`, and slice-13 explicitly recommended
this slice with its scope AND the anti-misread requirement. No design-direction ambiguity — the
flag is the same vetted neutral marker REUSED from slices 11/12/13. No JTBD re-run required; the
journey work is grounded in the validated job.

## R-14-2 (RISK) — Flag misread as a score deduction (the slice-specific high risk)

Mitigated by D-14-2 (sum-to-weight CARDINAL, counter shown-never-applied) + D-14-3 (anti-misread
copy AC + KPI) + the sum-to-weight + byte-identity gold (markers elided). Tracked into DESIGN/DISTILL.

## R-14-3 (RISK) — N+1 on the breakdown (many contributions across many pairings)

Mitigated by C-8 (HARD product commitment) + US-CF-001 AC (flatten ALL contribution CIDs across all
pairings into ONE call) + a behavioral query-count test + the inherited slice-12 adapter N+1 property.

## DoR verdict: PASSED (9/9 for both stories; Dimension 0 PASS; JTBD PASS; score-orthogonality gate PASS)

See `dor-checklist.md`.

---

# Wave decisions: DESIGN (Morgan / nw-solution-architect · 2026-06-08)

> Artifacts: `design/architecture-design.md`, `design/component-boundaries.md`,
> `design/technology-stack.md`; ADR `docs/adrs/ADR-051-*.md`. Reuse-only; workspace stays 21.

## DD-14-1 — Projection seam: thread `&presence` into the render (NOT onto the scoring types)

LOCKED (ADR-051). The `/score` contribution rows project `scoring::Contribution` inside
`scoring::WeightedPairing` — types OWNED by the pure `scoring` crate, which the viewer must NOT
mutate or recompute (D-14-2 / I-CF-9). So slice-14 CANNOT do slice-13's bool-on-the-view-model
(ADR-050: `EdgeRow.is_countered`) — `EdgeRow` is viewer-owned; `scoring::Contribution` is not.

Decision: thread `presence: &HashSet<String>` down the render chain
(`render_score_results_fragment` → `render_score_result` → `render_score_pairing` →
`render_score_breakdown`); the render becomes a TOTAL function of `(ScoreState, presence)`.
`render_score_breakdown` emits the REUSED `render_countered_link(&c.cid.0,
presence.contains(&c.cid.0))` BESIDE the verbatim subtotal. This is slice-13's REJECTED
"Alternative 3" — but the CORRECT choice here because the rejection rationale (breaks
"render is a total function of the view-model") does not apply to a foreign immutable type, and
threading `&presence` is exactly what keeps the `scoring` types pristine + makes the
orthogonality structural. **NO field added to `scoring::Contribution`; `scoring` UNCHANGED.**

## DD-14-2 — Effect shell: `score_counter_presence` flatten-once helper (N+1 guard)

LOCKED. NEW helper in `adapter-http-viewer` (mirrors slice-13's `survey_counter_presence`):
flatten EVERY `Contribution.cid` across EVERY `WeightedPairing`
(`view.ranked.flat_map(|p| p.contributions()).map(|c| c.cid.0.clone())`) → ONE
`counter_presence_for` call (REUSED slice-12 read, ADR-048) → `unwrap_or_default()` on error.
Wired into `resolve_score_state`'s `Scored` arm AFTER `scoring::score` builds the view.
`Form`/`NoClaims` build no view → helper never called → 0 queries. ONE query per render,
invariant to pairing/contribution count (I-CF-8). NO new read method, NO new SQL.

## DD-14-3 — Anti-misread legend SSOT constant (the one genuinely-new artifact)

LOCKED (D-14-3 realized). `pub const SCORE_COUNTER_LEGEND` in `viewer-domain`, rendered ONCE per
scored breakdown (in `render_score_result`'s `Scored` arm, above the pairings — not per row, not
per pairing, never for `Form`/`NoClaims`). Exact copy:

> *A "Countered" marker means another claim disagrees with this one elsewhere. It is shown for
> you to judge and does not change this contributor's score — each contribution keeps its full
> weight.*

Honors AC-SCORE-ANTIMISREAD's blocklist (no disputed/refuted/false/penalty/deduction/lowered/
disputed score). Part of the byte-identity elision set: with the markers AND the legend elided,
the render is byte-identical to the slice-09 baseline (AC-SCORE-BYTEID).

## DD-14-4 — Sum-to-weight orthogonality guaranteed BY CONSTRUCTION (the cardinal)

LOCKED (I-CF-9 / D-14-2). Structural, not merely tested: (1) the render projects the SAME
unchanged `WeightedPairing` (slice-09 "subtotals sum to weight BY CONSTRUCTION" doc-comment holds
verbatim); (2) the presence set only GATES an additive marker — no presence value reaches any
weight/confidence/bonus/subtotal/bucket/rank/order; (3) threading `&presence` (DD-14-1) instead
of mutating `scoring::Contribution` leaves the `WeightedView` with no viewer-side mutation
surface; (4) a countered claim renders its FULL original subtotal. Pinned by AC-SCORE-SUMWEIGHT +
AC-SCORE-BYTEID.

## DD-14-5 — Reuse / boundary confirmations (xtask delta NONE)

LOCKED. NO new crate / route / read-method / render-fn / SQL / KPI ID. Touches TWO crates
(`viewer-domain` pure render-chain widening + breakdown arm + legend; `adapter-http-viewer`
helper + wiring); `ports`/`adapter-duckdb`/`scoring`/`cli` UNCHANGED. xtask check-arch delta
NONE: `viewer-domain → scoring` edge already exists (slice-09); REUSED ref-table-only query does
not trip `no_cross_table_join_elides_author`; viewer capability boundary unchanged. **Workspace
stays 21 members.** NO external integration → no contract-test annotation for the DEVOPS handoff.
Functional paradigm (ADR-007) preserved (types-first; pure core = render + scoring projection;
effect shell = the one REUSED read + flatten helper).

---

## Handoff readiness

DISCUSS artifacts complete: `feature-delta.md`, `user-stories.md`, journey (visual + YAML),
`acceptance-criteria.md`, `shared-artifacts-registry.md`, `dor-checklist.md`, `outcome-kpis.md`,
`wave-decisions.md`. Ready for DESIGN (solution-architect). No code written; no DESIGN performed.
Scope = reuse-only (REUSE `counter_presence_for` + `render_countered_link`; NO new crate / route /
read-method / render fn; workspace stays 21).
