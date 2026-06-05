# Slice 02 — Epistemic honesty: sparse rendering, verbatim numbers, empty state

> Release 2 · Stories: US-CS-003 (sparse honesty + verbatim + empty state)
> Job: J-002 (sub-job J-002c — epistemic honesty) · Persona: P-001 (Maria, node operator) · Estimate: ~2 days

## Goal

Make the browser contributor-score view fail honestly, so Maria can CALIBRATE the
number rather than just read it: thin evidence renders as `[SPARSE]` (never manufactured
confidence), confidence + weight render verbatim (never silently rounded into a
reassuring bucket), and a contributor with no local claims renders a guided empty state
(never a blank region or a crash). This hardens the J-002c anxiety mitigation —
"a bad call on biased or sparse data" — on the browser surface.

## IN scope

- Sparse rendering (US-CS-003): a single-claim / single-author / no-cross-project-span
  pairing renders `[SPARSE]` + the "based on N claim(s) by M author(s) — treat as a
  lead, not a conclusion" honesty line, regardless of weight magnitude. The bucket
  decision is PROJECTED from the pure core's `WeightBucket` (the breadth guard) — the
  viewer recomputes nothing.
- Verbatim numbers: every confidence renders via the shared `render_confidence` contract
  (`0.90`, never `0.9`/`90%`); the displayed pairing weight is the exact consumed value
  (no bucket-midpoint rounding — KPI-4 / Gate 6).
- The empty / no-claims-for-contributor state: a contributor with no local claims renders
  a fixed plain-language "no local claims for that contributor" message — in BOTH the
  fragment and full-page shapes — never a blank region, never a stack trace.

## OUT of scope

- The happy-path score + breakdown render (→ slice 01).
- Any write/sign affordance (the viewer stays read-only — WD-CS-3).
- Persisting any derived score (WD-CS-10 / WD-72).
- Object-dimension / traversal browser views (inherited CLI surfaces; deferred).

## Learning hypothesis

If Maria can TRUST a thin score to read as thin (sparse, not false-confident), the
numbers to read verbatim, and an unknown contributor to read as unknown, then the
browser score surface is a complete and calibratable decision aid — and the
false-confidence failure mode J-002c exists to mitigate never reaches her on the browser
surface.

## Acceptance criteria (from US-CS-003 UAT)

- [ ] A single-claim / single-author / no-cross-project-span pairing renders `[SPARSE]`
      + the "based on N claim(s) by M author(s) — treat as a lead, not a conclusion"
      honesty line, regardless of weight magnitude (KPI-GRAPH-4).
- [ ] A high-confidence single opinion is NEVER labelled Strong (the breadth guard, not
      the magnitude, decides the bucket).
- [ ] Every confidence renders verbatim (`0.90`, not `0.9`/`90%`); the displayed weight
      is the consumed weight, no bucket-midpoint rounding (KPI-4).
- [ ] A contributor with no local claims renders the fixed "no local claims for that
      contributor" guided state in BOTH fragment and full-page shapes — no blank region,
      no crash, no stack trace.
- [ ] The `[SPARSE]` bucket + honesty counts are projected from the pure core's
      `WeightBucket` + contribution counts (the viewer recomputes no bucket — WD-CS-6).

## Dependencies

- Slice 01 (the `/score` route + local-scoring read effect + the `WeightedView` render
  fork).
- slice-04 `WeightBucket` breadth guard + the `[SPARSE]` honesty-line precedent
  (`render_weighted_view` sparse rendering) (REUSE / mirror).
- slice-07 guided-state + `Shape`-fork discipline; slice-08 `NoResults`/`Unavailable`
  guided-state precedent (mirror).

## Estimate

~2 days. The bucket decision + counts are inherited from the pure core; the work is the
HTML projection of the `[SPARSE]` marker + honesty line, the verbatim contract reuse,
and the empty-state render in both shapes.
</content>
