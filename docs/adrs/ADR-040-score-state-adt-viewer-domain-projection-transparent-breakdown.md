# ADR-040: The `/score` Render — A `ScoreState` ADT, a Pure `viewer-domain` Projection of the slice-04 `WeightedView`, and a Transparent Per-Claim Breakdown Table

- **Status**: Accepted (slice-09 viewer-contributor-scoring, DESIGN 2026-06-05). Resolves OD-CS-3 + OD-CS-6 + the US-CS-002/003 render.
- **Date**: 2026-06-05
- **Deciders**: Morgan (nw-solution-architect), resolving OD-CS-3 + OD-CS-6 for viewer-contributor-scoring (slice-09).
- **Feature**: viewer-contributor-scoring (slice-09)
- **Extends**: ADR-007 (pure/effect split), ADR-022 (the pure `scoring` core + the `WeightedView`/`WeightedPairing`/`Contribution`/`WeightBucket` types reused), ADR-029 (the pure `viewer-domain` render core + `render_confidence` verbatim contract), ADR-032 (the fragment/page split — page = chrome + fragment), ADR-037 (the slice-08 `SearchState` ADT precedent — a pure projection of a reused composition + a payload-free degenerate state).
- **Resolves**: OD-CS-3 (breakdown render: table vs bars vs panel) + OD-CS-6 (empty/no-claims state wording + placement).

## Context

US-CS-002/003 require the viewer to render a contributor's transparent adherence
score AS HTML, computed by the slice-04 pure scorer (ADR-039) over the local feed.
The render is load-bearing on the J-002c transparency thesis, with three cardinal
constraints (the I-CS-* invariants):

- **Transparency — never an opaque number (I-CS-2 / WD-CS-4 / KPI-GRAPH-3)**: every
  rendered weight MUST carry its per-claim `Contribution` breakdown — each
  contributing claim named back to its `author_did` + `cid`, with the verbatim base
  confidence + the applied bonuses + a subtotal, and a running sum equal to the
  displayed weight (reproduce-by-hand). Conflicting/identical-subject claims by
  different authors render as SEPARATE rows (no merge into a faceless consensus
  number).
- **Sparse renders sparse (I-CS-3 / WD-CS-5 / KPI-GRAPH-4)**: a thin pairing renders
  `[SPARSE]` + the "treat as a lead, not a conclusion" honesty line regardless of
  weight magnitude — and the bucket decision is the pure core's
  (`WeightBucket::Sparse`), PROJECTED, never recomputed in the viewer.
- **Confidence + weight verbatim (I-CS-6 / WD-CS-7 / KPI-4)**: every confidence
  renders as the stored `f64` verbatim (`0.86`, never `0.9`/`86%`); the displayed
  weight is the consumed weight (no bucket-midpoint rounding).

The slice-04 pure scorer (ADR-022) already produces the exact decomposition: a
`WeightedView { ranked: Vec<WeightedPairing> }`, each `WeightedPairing` carrying its
`weight`, `bucket: WeightBucket`, `claim_count`, `distinct_author_count`,
`cross_project_span`, and a NON-EMPTY `contributions(): &[Contribution]` (each with
`author_did`, `cid`, `base`, `author_distinct_bonus`,
`cross_project_triangulation_bonus`, `subtotal`). The weight IS the sum of the
contribution subtotals by construction (Gate 2). The viewer's job is to PROJECT this
to HTML — not to recompute anything.

The slice-04 CLI already renders this to STDOUT TEXT (`render_weighted_view` /
`render_weighted_explain`) — the wrong medium for a browser. The viewer has a pure
maud render core (`viewer-domain`, ADR-029) with the established `ScrapeState` /
`SearchState` ADT pattern, the `page_head`/`htmx_script`/`render_tab_nav` chrome, the
`render_confidence` verbatim contract, and the page = chrome + fragment composition
(ADR-032).

## Decision

**Add a NEW pure `viewer-domain` render module that PROJECTS the slice-04
`WeightedView` into HTML — REUSING the `scoring` types (`WeightedView`,
`WeightedPairing`, `Contribution`, `WeightBucket`), NOT the CLI stdout renderer. The
render input is a `ScoreState` ADT (`Form | Scored | NoClaims`); each pairing renders
a per-claim breakdown TABLE whose running sum equals the displayed weight; `[SPARSE]`
is PROJECTED from the pure core's `WeightBucket::Sparse`; the no-claims state is a
guided results-region message.**

### The `ScoreState` ADT (pure render input, mirrors `SearchState`)

```text
pub enum ScoreState {
    /// GET /score with no contributor submitted: the empty contributor form. No
    /// store read attempted (the bare /score landing — OD-CS-1).
    Form,
    /// A scored contributor whose local feed produced >=1 pairing: the ranked
    /// WeightedView, projected per-pairing into the headline weight + bucket label
    /// + the per-claim breakdown table (the running sum == the displayed weight).
    /// Carries the contributor DID (for the heading) + the reused WeightedView
    /// VERBATIM — the viewer holds NO scoring math (anti-merging + the breadth
    /// guard are the pure core's; the renderer only projects them).
    Scored { contributor: String, view: scoring::WeightedView },
    /// The contributor has NO local claims (the feed read returned zero rows):
    /// the guided "No local claims for that contributor." results-region message
    /// (OD-CS-6) — never a blank region, never a crash. DISTINCT from Form (Form
    /// = nothing submitted yet; NoClaims = submitted, nothing found).
    NoClaims { contributor: String },
}
```

**Sparse is a PROPERTY of `Scored`, not a separate ADT arm** (confirmed). A `[SPARSE]`
pairing is still a fully-rendered pairing inside `Scored.view.ranked` — its
`WeightBucket::Sparse` drives the `[SPARSE]` marker + the honesty line on that ONE
pairing, while OTHER pairings in the same view may render Strong/Moderate. Sparseness
is per-pairing (the pure core decides it per `WeightedPairing`), so it cannot be a
top-level state arm without collapsing a mixed view. The renderer matches the bucket
PER PAIRING; the viewer never decides sparseness (I-CS-3 / WD-CS-6).

### The breakdown render: a per-claim TABLE (OD-CS-3)

For each `WeightedPairing` in `view.ranked` (ranked by weight desc, the slice-04
order, REUSED — the viewer does not re-sort), the projection renders:

1. A **headline**: `subject` → the adherence `weight` (verbatim, via a
   weight-formatting sibling of `render_confidence`) + the `WeightBucket` label
   (`Strong` / `Moderate` / `[SPARSE]`).
2. A **per-claim breakdown table** directly beneath: ONE row per `Contribution`
   (NEVER merged), each row naming `author_did` + `cid` + the verbatim `base`
   confidence (via `render_confidence`) + the `author_distinct_bonus` +
   the `cross_project_triangulation_bonus` + the `subtotal`.
3. A **running sum** equal to the displayed `weight` (reproduce-by-hand —
   KPI-GRAPH-3). The sum is rendered from the SAME `view`'s `contributions()` the
   headline `weight` came from, so they agree by construction (see §"Structural
   transparency").
4. For a **`Sparse`** pairing: the `[SPARSE]` marker + the honesty line
   *"based on N claim(s) by M author(s) — treat as a lead, not a conclusion"*,
   where N = `claim_count` and M = `distinct_author_count` are PROJECTED from the
   pairing (never recomputed). The honesty line renders REGARDLESS of weight
   magnitude (the breadth guard already bucketed it Sparse in the pure core).

A table (not stacked bars, not a free-text "why this score" panel) is the clearest
reproduce-by-hand projection of the `Contribution` list: a column per formula term,
a row per claim, a footer row that sums to the weight. DELIVER MAY add a
collapsed/expandable `<details>` affordance over the table (a nicety; the table is
the contract).

### Structural transparency: headline + breakdown from the SAME `WeightedView`

The headline weight and the breakdown table are BOTH projected from the SAME
`WeightedPairing` value (the `weight` field and `contributions()` of the same
pairing). There is no second source for the number and no separate "summary" path
— so "the breakdown sums to the displayed weight" is true by construction (Gate 2 is
a property of the pure core: `weight == Σ subtotal`; the renderer reads both off the
SAME pairing). An opaque-number regression is structurally impossible: the renderer
cannot emit a weight without iterating the SAME pairing's contributions, because the
projection function takes a `&WeightedPairing` and renders both from it.

### No-claims empty state (OD-CS-6)

`NoClaims` renders a single pinned `SCORE_NO_LOCAL_CLAIMS_NOTICE` constant
("No local claims for that contributor.") in the results region, forking by `Shape`
like every other state — both shapes show the same guided message (mirrors the
slice-07 `/scrape` guided states + the slice-08 `SearchState::NoResults`). The
breadth guard is NEVER invoked on an empty feed (the pure scorer returns an empty
`WeightedView`; the shell maps empty → `NoClaims` before any pairing render).

### Page = chrome + fragment (ADR-032 reused; I-CS-7 parity by construction)

```text
pub const SCORE_RESULTS_ID: &str = "score-results";

/// The results-region FRAGMENT — no chrome, no form. Forks every ScoreState's
/// results region (Scored's pairings / NoClaims' message); the form lives in the
/// page. I-CS-7 (= I-HX-1).
pub fn render_score_results_fragment(state: &ScoreState) -> Markup;

/// The full /score page = chrome (head + nav + the contributor form) wrapped
/// AROUND render_score_results_fragment(state) — the EXACT same fragment fn the
/// htmx shape returns alone. I-CS-7 parity by construction (the results-region
/// logic is NOT duplicated).
pub fn render_score_page(state: &ScoreState) -> String;
```

`viewer-domain` gains a pure dependency edge on `scoring` (a pure domain crate,
ADR-022) to consume `WeightedView`/`WeightedPairing`/`Contribution`/`WeightBucket`
— the `check-arch` allowlist edit is in ADR-041 §enforcement. Confidence + weight
render verbatim via the EXISTING `render_confidence` (and a sibling weight formatter)
— ONE place (I-CS-6 / FR-VIEW-8).

## Alternatives Considered

| Option | Evaluation | Rejected because |
|--------|-----------|------------------|
| **Reuse the slice-04 CLI `render_weighted_view` / `render_weighted_explain` (stdout text)** | Maximal reuse. | **Rejected (OD-CS-3 / wrong medium).** The CLI renderer emits plain-text stdout (column lines, no HTML, no `Shape` fork, no `hx-*`). Forcing it into a browser means a parallel HTML path anyway. Reuse the TYPES (`WeightedView`/`Contribution`), project a NEW maud renderer — exactly as slice-08 reused `compose_results` and added a new HTML projection (ADR-037). |
| **Render the score as a single weight + bucket (no breakdown)** | Compact. | **Rejected (I-CS-2 / WD-CS-4 — out of scope).** An opaque number is the EXACT aggregator failure J-002c exists to avoid; it is forbidden, not a lean simplification. The breakdown ships WITH the number in the walking skeleton. |
| **Stacked bars / a free-text "why this score" panel** for the breakdown | Visual. | **Rejected (OD-CS-3 / reproduce-by-hand).** Bars obscure the exact arithmetic (a bar length is not a number to re-sum); a free-text panel is not a column-aligned, row-per-claim, sums-to-the-weight artifact. A table is the clearest KPI-GRAPH-3 projection — a column per formula term, a footer that sums to the weight. (An expandable `<details>` over the table is allowed.) |
| **A separate `Sparse` ADT arm at the top level** | Symmetric arms. | **Rejected (sparseness is per-pairing).** A single view can mix a Strong pairing and a Sparse pairing (US-CS-002 Ex 2: ranked multiple pairings). A top-level `Sparse` arm would force the whole view into one bucket, hiding the mix. Sparse stays a PROPERTY of each `WeightedPairing` (its `WeightBucket`), matched per pairing in the projection. |
| **Recompute / re-derive the bucket or counts in the viewer** | — | **Rejected (WD-CS-6 / I-CS-3).** The breadth guard + bucket + counts are the pure core's (`weight_bucket`, the `WeightedPairing` fields). The renderer PROJECTS `bucket` + `claim_count` + `distinct_author_count` — it recomputes nothing. A second guard would be a second SSOT. |
| **Collapse `NoClaims` into `Form`** | Fewer arms. | **Rejected (OD-CS-6 / honesty).** "Nothing submitted yet" (Form) and "submitted, no local claims" (NoClaims) are semantically distinct — collapsing them would show an empty form where the operator expects "this contributor has no claims here", mistaking absence for a blank prompt (the US-CS-003 Ex 3 anxiety). Distinct arms, each with pinned copy (the `SearchState::Form` vs `NoResults` precedent). |
| **Render in the effect shell directly** | Fewer crates. | **Rejected (ADR-007/029).** Rendering is pure; it belongs in `viewer-domain`. The shell builds `ScoreState` (after the pure `score()` call, ADR-039) and forks by `Shape` only. |

## Consequences

### Positive
- Transparency is STRUCTURAL: the headline weight + the breakdown are projected from
  the SAME `WeightedPairing`, so "the breakdown sums to the weight" holds by
  construction (Gate 2 in the pure core; the renderer reads both off one value). An
  opaque-number regression is impossible — the projection takes a `&WeightedPairing`
  and renders both.
- Anti-merging carries over: each `Contribution` is one table row under its own
  `author_did` — two authors on the same subject are TWO rows, never averaged
  (I-CS-2/I-CS-10), because the renderer iterates the non-merged `contributions()`.
- Sparse renders sparse by PROJECTION: `[SPARSE]` + the honesty counts come from the
  pure core's `WeightBucket::Sparse` + the pairing's counts — the viewer recomputes
  no bucket (I-CS-3 / WD-CS-6).
- Verbatim numbers: confidence + weight render via the SAME `render_confidence` /
  weight-formatter — one place (I-CS-6 / KPI-4).
- Fragment/full-page parity is structural (page embeds the fragment fn, ADR-032 /
  I-CS-7).
- The score is display-only: `ScoreState` is built per query and never persisted
  (zero new persisted types; I-CS-4).

### Negative
- `viewer-domain` takes a new pure dependency on `scoring`. Accepted: both are pure
  domain crates (no I/O); the edge is `viewer-domain → scoring` (never the reverse).
  Requires a `check-arch` pure-core allowlist confirmation (ADR-041) — the same
  shape as the slice-08 `viewer-domain → appview-domain` edge.
- A new render module + the `ScoreState` ADT in `viewer-domain`. Accepted: the
  symmetric counterpart to the existing `SearchState`/`render_search_*` surface,
  reusing the shared chrome (`page_head`, `render_confidence`, the nav).

## Revisit Trigger
- The breakdown grows large enough to need pagination/collapsing → wrap the table in
  a `<details>` (DELIVER nicety) or page the contributions; the `ScoreState` ADT + the
  `Shape` fork stay total.
- A new score dimension (object) is added to the browser → widen the projection /
  add a dimension param; the `ScoreState` arms + the per-pairing projection stay
  total. Out of scope for slice-09 (OD-CS-5).
