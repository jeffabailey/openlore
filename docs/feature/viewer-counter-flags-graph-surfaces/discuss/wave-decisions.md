# Wave decisions: viewer-counter-flags-graph-surfaces (slice-13) — DISCUSS

> Owner: Luna (nw-product-owner) · 2026-06-07 · Wave: DISCUSS (lean)

## Scope Assessment: PASS — 3 stories, 1 context, estimated ~1 day

Elephant-Carpaccio gate run BEFORE journey investment. Right-sized under Option B
(see the scope fork). Full signal table in `feature-delta.md` §"Scope assessment".

## D-13-1 — Scope fork: Option B (defer `/score` to slice-14) — DECISION FOR USER

slice-12 deferred FOUR candidate surfaces to "slice-13". They do not fit one ≤1-day
slice. Decision: **ship the two SHARED-SHAPE surfaces now, defer `/score`.**

- **slice-13 (this DISCUSS)**: `/peer-claims` (US-CF-002) + `/project` + `/philosophy`
  EDGE rows (US-CF-003), backed by US-CF-001 (wiring the REUSED slice-12 read). Two
  view-models (`PeerClaimRowView`, `EdgeRow`); `/project` + `/philosophy` share ONE
  `EdgeRow` render arm covering both routes. Mirrors the slice-12 pattern closely.
- **slice-14 (recommended, `viewer-counter-flags-score-surface`)**: the `/score`
  `ScoreState::Scored{WeightedView}` contribution-row flags. Deferred because: (a) it
  projects a structurally-different ADT (the slice-04 `WeightedView`), (b) a flag beside
  a weight carries a "does being countered lower the weight?" misread risk needing its
  own anti-misread copy, (c) it must re-assert the slice-09 CARDINAL sum-to-weight
  guarantee as an explicit AC. Bundling it overflows ≤1-day AND re-opens the highest-risk
  surface inside a shared-shape slice.

Rationale in full: `feature-delta.md` §"Scope fork". **The user should confirm.** If the
user wants `/score` in this slice (Option A), Luna adds US-CF-004 with the
`ScoreState`/`Contribution` projection + anti-misread copy + the sum-to-weight CARDINAL
AC; estimate rises to ~2 days. Default carried forward: Option B.

## D-13-2 — REUSE the slice-12 `counter_presence_for` read — NO new read method

Confirmed `StoreReadPort::counter_presence_for(&[String]) -> HashSet<String>` exists
(`crates/ports/src/store_read.rs` lines 360-384, slice-12 / ADR-048) and is the right
shape for all slice-13 surfaces (presence-only set membership over the page's CID set).
slice-13 WIRES it into three handlers; it adds NO new read method and NO new SQL. This is
I-CF-7 / I-CF-8 and the defining property of the slice (recorded in
`shared-artifacts-registry.md`).

## D-13-3 — No new KPI ID; realize inherited KPIs on two new facets

Matching slice-08/09/10/11/12, slice-13 mints NO new KPI ID. It STRENGTHENS the READ
side of KPI-FED-3 on the federated + traversal surfaces, EXTENDS KPI-VIEW-1/VIEW-3, and
carries the guardrails (KPI-VIEW-2 read-only, KPI-AV-2/GRAPH-2 anti-merging, KPI-GRAPH-4
shown-never-applied on the graph, KPI-4 verbatim, KPI-5/VIEW-5/HX-G1/G2/G3). Detail
inlined in `feature-delta.md` §"Outcome KPIs" (lean — no separate `outcome-kpis.md`).

## D-13-4 — Persona: EXTEND the slice-12 counter-claim-scanner hat (no new hat)

The slice-13 surfaces are the SAME scanning behavior on two more surfaces, so the
counter-claim-scanner hat is EXTENDED (federated + traversal surfaces appended), not
minted anew. Appended to `docs/product/personas/senior-engineer-solo-builder.yaml`
(2026-06-07). Note: that file previously carried only the slice-11 counter-claim-reader
hat; this DISCUSS adds the scanner hat (covering slice-12 + slice-13 surfaces) since the
slice-12 feature-delta's referenced facet was not persisted to the persona file.

## R-13-1 (RISK) — No DIVERGE wave for slice-13

There is no `diverge/` directory for this feature. Per the workflow, this is recorded as
a NON-BLOCKING risk: the job (J-003b) is already validated in `docs/product/jobs.yaml`,
and slice-12 explicitly recommended this slice with its scope. There is no design-
direction ambiguity — the flag is a single well-defined neutral marker REUSED from
slice-12. No JTBD re-run is required; the journey work is grounded in the validated job.

## R-13-2 (RISK) — N+1 on the edge surfaces

The edge CID set spans multiple `EdgeGroup`s, so a naive implementation could call the
read per-group/per-edge. Mitigated by I-CF-8 (HARD product commitment) + US-CF-001 AC
(flatten ALL edge CIDs across groups into ONE call) + a behavioral query-count test +
the inherited slice-12 adapter N+1 property. Tracked into DESIGN/DISTILL.

## R-13-3 (RISK) — Traversal flag re-grouping/re-ordering the survey

Mitigated by I-CF-9 + US-CF-003 AC (byte-identity of grouping/edge-order/contributor-list
with markers elided — the slice-12 baseline+marker-elision tactic from its evolution
archive).

## DoR verdict: PASSED (9/9 for all 3 stories; Dimension 0 PASS; JTBD PASS)

See `definition-of-ready.md`.

## Handoff readiness

DISCUSS artifacts complete: `feature-delta.md`, `user-stories.md`, journey (visual +
YAML), `shared-artifacts-registry.md`, `definition-of-ready.md`, `wave-decisions.md`;
persona hat extended. Ready for DESIGN (solution-architect) once the user confirms the
scope fork (Option B default). No code written; no DESIGN performed.
