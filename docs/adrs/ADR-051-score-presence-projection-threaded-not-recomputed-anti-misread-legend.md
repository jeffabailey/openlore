# ADR-051: Thread the Counter-Presence Set Into the `/score` Render (Not Onto the Scoring Types), the Anti-Misread Legend SSOT, and Sum-to-Weight Orthogonality By Construction

- **Status**: Accepted
- **Date**: 2026-06-08
- **Deciders**: Morgan (nw-solution-architect), DESIGN wave for `viewer-counter-flags-score-surface` (slice-14)
- **Feature**: viewer-counter-flags-score-surface (slice-14)
- **Extends**: ADR-050 (the slice-13 flatten-CIDs-before-render + bool-on-the-view-model decision — this ADR DIVERGES from its bool-on-the-model half for a typed reason), ADR-049 (reuse the slice-12 read across handlers), ADR-048 (the batch presence read), ADR-040 (the `ScoreState` ADT + the pure `viewer-domain` projection of the `WeightedView` + the transparent breakdown table + the sum-to-weight BY CONSTRUCTION property), ADR-039 (the pure `scoring` core), ADR-007 (functional paradigm)

## Context

slice-14 must flag, on the `GET /score?contributor=<did>` breakdown, each contribution whose
claim has ≥1 counter — REUSING the slice-12 `counter_presence_for` read and the slice-13
`render_countered_link` SSOT, with NO new read method, render fn, route, or crate. The `/score`
surface differs structurally from the slice-13 traversal surfaces in a way that forces a
different seam AND raises a misread risk slices 12/13 did not:

1. **The flagged rows are NOT a `viewer-domain` view-model.** `render_score_breakdown` projects
   `scoring::Contribution` rows held inside `scoring::WeightedPairing` — types owned by the
   **`scoring` crate** (ADR-039/040), which the viewer must NOT mutate and NEVER recompute
   (feature-delta out-of-scope; D-14-2; I-CF-9). slice-13's tactic (ADR-050: add `is_countered`
   to the `EdgeRow` view-model and set it in the grouper) is unavailable: `EdgeRow` is
   `viewer-domain`-owned; `scoring::Contribution` is not. Adding a presentation field to a
   `scoring` type would corrupt the pure scoring core and the sum-to-weight cardinal.

2. **The flag sits beside SCORING MATH.** On `/score` the marker sits next to a weight, a
   confidence, two bonuses, and a subtotal inside a ranked breakdown whose subtotals sum to a
   headline pairing weight (the slice-09 CARDINAL, ADR-040 Gate 2). Two obligations follow that
   slices 12/13 did not carry: (a) the sum-to-weight cardinal must be PRESERVED with the flag
   present — adding the flag must change NO weight/confidence/bonus/subtotal/total/bucket/rank/
   order (the counter is SHOWN, never APPLIED; a countered claim keeps its FULL original weight);
   and (b) a reader must not misread the marker as "this counter lowered the score," which
   requires deliberate anti-misread copy (D-14-3).

Three coupled questions: WHERE does the presence bool live (since it cannot live on the scoring
type)? WHAT is the anti-misread copy and where does it render? And HOW is the sum-to-weight
orthogonality guaranteed rather than merely hoped?

## Decision

**(1) Thread `presence: &HashSet<String>` down the `/score` render chain; do NOT put a bool on
the scoring types.** Widen `render_score_results_fragment` / `render_score_page` /
`render_score_result` / `render_score_pairing` / `render_score_breakdown` to take
`presence: &HashSet<String>` alongside the UNCHANGED `&ScoreState`. The pure render becomes a
TOTAL function of `(ScoreState, presence)` — the slice-12/13 `from_row_with_presence` projection
seam adapted to a type the viewer does not own. In `render_score_breakdown`, after the existing
six cells (author / cid / confidence / author bonus / triangulation bonus / subtotal) rendered
VERBATIM, emit `render_countered_link(&contribution.cid.0, presence.contains(&contribution.cid.0))`
— the REUSED slice-13 SSOT — as an additive cell. The `WeightedPairing`/`Contribution` are
PROJECTED, never mutated; `scoring` is UNCHANGED.

> This is precisely the option slice-13's ADR-050 REJECTED as "Alternative 3" (pass the presence
> to the renderer). It is the CORRECT choice here because the rejection rationale (it breaks
> "render is a total function of the view-model" and risks view/flag divergence) does not apply
> when the view-model is a foreign immutable type: there is no `viewer-domain` view-model to put
> the bool on, and threading `&presence` is exactly what keeps the `scoring` types pristine. The
> render is still a total function — of `(ScoreState, presence)` — so the "argument-free render"
> concern is replaced by a stronger property: the numbers cannot depend on `presence`.

**(2) Flatten EVERY contribution CID across EVERY pairing into ONE read, in a
`score_counter_presence` effect-shell helper** (mirroring slice-13's `survey_counter_presence`):
`view.ranked.iter().flat_map(|p| p.contributions()).map(|c| c.cid.0.clone())` → ONE
`counter_presence_for` call → `unwrap_or_default()` on error. ONE query per render, invariant to
pairing/contribution count (I-CF-8). `Form`/`NoClaims` build no `view`, so the helper is never
called (0 queries).

**(3) The anti-misread legend is a single SSOT constant, rendered ONCE per scored breakdown.**
Add `pub const SCORE_COUNTER_LEGEND` in `viewer-domain`, rendered once in `render_score_result`'s
`Scored` arm (above the pairings — not per row, not per pairing, never for `Form`/`NoClaims`).
Exact copy:

> *A "Countered" marker means another claim disagrees with this one elsewhere. It is shown for
> you to judge and does not change this contributor's score — each contribution keeps its full
> weight.*

The copy honors AC-SCORE-ANTIMISREAD's blocklist: it contains NONE of "disputed", "refuted",
"false", "penalty", "deduction", "lowered", "disputed score". The marker text itself is the
REUSED `COUNTERED_PRESENCE_FLAG = "Countered"` (already vetted neutral). BOTH the markers AND the
legend are part of the AC-SCORE-BYTEID elision set — with them elided, the render is byte-identical
to the slice-09 baseline.

**(4) Sum-to-weight orthogonality is guaranteed BY CONSTRUCTION** (not just tested): the render
projects the SAME unchanged `WeightedPairing` (so the slice-09 "subtotals sum to weight BY
CONSTRUCTION" doc-comment holds verbatim); the presence set only GATES an additive marker
(`presence.contains` is read nowhere near a number); and because the seam threads `&presence`
rather than mutating `scoring::Contribution`, the `WeightedView` the renderer holds has no
mutation surface from the viewer side. A countered claim contributes its FULL original
`subtotal`, rendered verbatim. Pinned by AC-SCORE-SUMWEIGHT (subtotals sum to weight on a FLAGGED
breakdown) + AC-SCORE-BYTEID (byte-identity vs slice-09 with markers + legend elided).

## Alternatives Considered

### Alternative 1 (seam) — Add `is_countered` to `scoring::Contribution` and set it in `scoring::score` (the slice-13 bool-on-the-model shape)

- **Evaluation**: Mirror ADR-050 exactly — put the bool on the row type and set it at construction.
- **Rejected because**: `Contribution` is owned by the pure `scoring` crate. A presentation flag
  there (a) pollutes the pure scoring core with a viewer concern, (b) means `scoring::score` would
  need the presence set (an I/O-derived input) — destroying its purity/determinism (ADR-039), and
  (c) creates a mutation surface on the value whose immutability is exactly what guarantees the
  sum-to-weight cardinal. Forbidden by the orthogonality boundary (component-boundaries §2).

### Alternative 2 (seam) — A `viewer-domain` flagged wrapper view-model (`ScoredRowView { contribution, is_countered }`)

- **Evaluation**: Project the `WeightedView` into a NEW `viewer-domain` view-model carrying the
  bool (so the bool lives on a viewer-owned type, slice-13-style), then render that.
- **Rejected because**: it duplicates the entire breakdown structure into a parallel view-model
  for a single boolean, adds a new type + a new projection step + new tests, and risks the wrapper
  drifting from the `WeightedPairing` it shadows (the sum-to-weight property now spans TWO types).
  Threading `&presence` to the render achieves the same flag with one parameter, zero new types,
  and the scoring value projected directly (no shadow to drift). The DISCUSS scope is reuse-only
  (~1 day); a new wrapper view-model is more machinery than the slice warrants.

### Alternative 3 (legend placement) — Render the anti-misread copy per countered row

- **Evaluation**: Attach the orthogonality copy to each marker.
- **Rejected because**: it is noisy (repeated on every countered row), it couples the copy to the
  row render (harder to keep ONE SSOT string + one elision point), and it bloats the byte-identity
  diff. One legend per scored breakdown governs every marker below it, renders once, and elides
  cleanly for the slice-09 baseline.

### Alternative 4 (N+1) — Call `counter_presence_for` per pairing

- **Evaluation**: Loop the pairings and read presence per pairing's contribution CIDs.
- **Rejected because**: it is the N+1 regression (I-CF-8). The flat `flat_map` over `view.ranked`
  already yields every contribution CID once; one call covers the whole breakdown — exactly the
  slice-13 flatten-once discipline (ADR-050 §1).

## Consequences

### Positive
- **The `scoring` core stays pristine** — no presentation field, no I/O-derived input, no
  recompute. The pure scorer's purity/determinism (ADR-039) is untouched.
- **Sum-to-weight orthogonality is structural** (not incidental): the render projects the same
  unchanged `WeightedPairing` and the presence set can only gate an additive marker — there is no
  code path by which a presence value reaches a number (architecture-design §7).
- **One presence query per render**, invariant to pairing/contribution count (I-CF-8); 0 for
  Form/NoClaims.
- **One anti-misread legend SSOT** — single string, single render site, single elision point,
  blocklist-compliant.
- **Render stays a total function of `(ScoreState, presence)`** — pure, no-I/O,
  unit/property-testable; parity is structural (the flag + legend live in the one fragment fn both
  shapes embed).
- **Reuse-only**: NO new crate, route, read method, or render fn; workspace stays 21.

### Negative / trade-offs
- The score render chain gains a `&presence` parameter at four call sites — a signature change to
  SHIPPED pure functions (slice-09). Mitigated: it is additive (an empty set yields the slice-09
  behavior; Form/NoClaims pass `&HashSet::new()`), the functions stay pure, and the slice-09 caller
  (`score_page`/`resolve_score_state`) is the one being edited anyway.
- The flatten correctness (one call, all pairings) lives in the CALLER (`score_counter_presence`),
  so it is pinned by a behavioral query-count test rather than the type system. Accepted: the
  `flat_map` over `view.ranked` is the simplest provably-single-call shape, and the test is cheap
  (the inherited slice-12 adapter N+1 property + a query-count gold).
- DIVERGING from ADR-050's bool-on-the-model half could look like inconsistency. Mitigated: the
  divergence is a TYPED necessity (foreign immutable scoring type vs viewer-owned `EdgeRow`) and is
  documented here + in component-boundaries §3; the slice-12/13 `from_row_with_presence`
  "presence-projected render" principle is preserved, only its mechanical form differs.

## Enforcement

- **Behavioral** (DISTILL/CRAFT): query count invariant to pairing/contribution count on `/score`
  (one call per render; 0 for Form/NoClaims); byte-identity gold of the breakdown render with
  markers + legend elided (every weight/confidence/bonus/subtotal/total/bucket/rank/row-order
  unchanged vs slice-09 — the slice-12/13 baseline+marker-elision tactic); AC-SCORE-SUMWEIGHT
  (subtotals sum to the displayed weight on a FLAGGED breakdown); a lint/gold asserting
  `SCORE_COUNTER_LEGEND` contains none of the blocklist words.
- **Structural**: the presence set is threaded as a render parameter; `scoring::Contribution`/
  `WeightedPairing` are UNCHANGED (no presentation field); the legend is ONE constant rendered at
  ONE site.
- **Type**: presence is a `HashSet<String>` (presence, never a count); the trait stays read-only;
  the `scoring` types are immutable to the viewer (projected, never mutated).
- **Arch** (`xtask check-arch`): viewer capability boundary UNCHANGED; `viewer-domain → scoring`
  dep already exists (slice-09); `no_cross_table_join_elides_author` not tripped (REUSED
  ref-table-only query). No new edge.
</content>
