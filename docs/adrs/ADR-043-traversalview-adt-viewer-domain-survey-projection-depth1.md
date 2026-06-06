# ADR-043: The `TraversalView` ADT — A Pure `viewer-domain` Survey Projection (Found{groups,contributors} | NoClaims), Depth-1, Anti-Merging by Construction, with Page=Chrome+Fragment Parity

- **Status**: Accepted (slice-10 viewer-graph-traversal, DESIGN 2026-06-06). Resolves the US-GT-002/003 render shape.
- **Date**: 2026-06-06
- **Deciders**: Morgan (nw-solution-architect), for viewer-graph-traversal (slice-10).
- **Feature**: viewer-graph-traversal (slice-10)
- **Extends**: ADR-007 (pure render core), ADR-029 (maud pure templating), ADR-032 (page = chrome + fragment), ADR-037 (the `SearchState` projection-ADT precedent), ADR-040 (the `ScoreState` projection-ADT precedent), WD-10 (the display-only confidence bucket).
- **Resolves**: the project/philosophy survey view-model + render shape (US-GT-002, US-GT-003).

## Context

`/project` and `/philosophy` are SYMMETRIC surveys over the SAME read pattern
(ADR-042) and the SAME render pattern: a depth-1 survey (the entity + its DIRECT
attributed edges) grouped by the OTHER dimension, with a distinct-contributors
list. The render must:

- decompose the `Vec<SurveyRow>` into per-`(group-key, author, cid)` rows with NO
  merge/average (anti-merging, I-GT-3);
- show each edge attributed: `author_did` (a `/score` link), VERBATIM confidence
  (I-GT-5), the slice-04 display-only bucket, and the `cid` (no-invented-edge,
  I-GT-4);
- make each group key a traversal `<a href>` (philosophy → `/philosophy`,
  project → `/project`) and each contributor a `/score` link;
- render a guided "no claims" state for an empty survey (NEVER a fabricated edge);
- serve a fragment (htmx swap) and a full page that EMBEDS the SAME fragment
  (parity, I-GT-6).

This is exactly the slice-08 `SearchState` / slice-09 `ScoreState` projection
shape: a pure ADT in `viewer-domain` over which the renderer matches totally, and
which the effect shell builds from the read.

## Decision

**Model the survey render input as a unified pure `TraversalView` ADT in
`viewer-domain` — `Found { entity, groups, contributors } | NoClaims { entity }`
— with two pure grouping constructors (`group_project` / `group_philosophy`),
mirrored `render_project_*` / `render_philosophy_*` fragment + page functions
(page EMBEDS the same fragment), and the display-only bucket projected from the
REUSED `claim_domain::confidence_bucket`. The viewer recomputes NO weight; the
contributor edge LINKS to `/score` (the J-002c boundary).**

### The ADT (pure render input)

```text
pub enum TraversalView {
    Found {
        entity: String,            // the queried subject/object
        groups: Vec<EdgeGroup>,    // keyed by the OTHER dimension
        contributors: Vec<String>, // distinct author_did, order-preserved, deduped (spanning author once)
    },
    NoClaims { entity: String },   // guided "no claims" — names the entity, NO fabricated edge (I-GT-4)
}
pub struct EdgeGroup { pub key: String, pub edges: Vec<EdgeRow> }  // key = a traversal <a href> target
pub struct EdgeRow   { pub author_did: String, pub confidence: f64, pub cid: String }
```

ONE unified ADT (not separate `ProjectSurvey` / `PhilosophySurvey` ADTs) because
the two surveys differ ONLY in which dimension is the group key and which is the
link target — a structural symmetry, not two shapes. The `group_project` /
`group_philosophy` constructors and the `render_project_*` / `render_philosophy_*`
renderers carry the asymmetry; the ADT is shared.

### Grouping (pure, anti-merging — the constructors)

```text
impl TraversalView {
    /// /project: group rows by `object` (philosophies embodied). One EdgeRow per
    /// (author, cid); two authors on one object -> two rows. contributors = distinct
    /// author_did. Empty rows -> NoClaims { entity }.
    pub fn group_project(entity: String, rows: Vec<SurveyRow>) -> Self;
    /// /philosophy: group rows by `subject` (projects that embody). Same per-row rule.
    pub fn group_philosophy(entity: String, rows: Vec<SurveyRow>) -> Self;
}
```

These are pure total functions. They CANNOT average (they only bucket by key +
collect attributed rows), so anti-merging holds by construction. A contributor
spanning multiple groups appears ONCE in `contributors`, but their edges are NEVER
deduped within a group.

### Render (mirrored, page embeds fragment)

```text
pub fn render_project_fragment(view: &TraversalView)   -> Markup;   // <div id="traversal-results"> ...
pub fn render_project_page(view: &TraversalView)       -> String;   // chrome + EMBEDS render_project_fragment
pub fn render_philosophy_fragment(view: &TraversalView) -> Markup;
pub fn render_philosophy_page(view: &TraversalView)     -> String;  // chrome + EMBEDS render_philosophy_fragment
```

Each `EdgeRow` renders: `author_did` via `href_score` (a `/score` link, bare DID —
ADR-044), `render_confidence(confidence)` (the REUSED single verbatim site), the
REUSED `claim_domain::confidence_bucket(confidence)` label
(speculative/weighted/well-evidenced/triangulated), and the `cid`. Each group
`key` is a traversal `<a href>` via `href_philosophy` / `href_project`. `NoClaims`
renders the guided state naming the entity + a CLI next-step hint. The page wraps
chrome (the local htmx `<script src>`, a nav link) AROUND the SAME fragment fn, so
fragment/page parity is structural (I-GT-6) — exactly the slice-07/08/09 contract.

### Bucket reuse (no recompute, J-002c boundary)

The edge bucket is `claim_domain::confidence_bucket` (the WD-10 PER-CLAIM
confidence bucket: Speculative/Weighted/WellEvidenced/Triangulated). This is
DISTINCT from the slice-04 scoring `WeightBucket` (Strong/Moderate/Sparse on
`/score`). The traversal edge shows the per-claim CONFIDENCE bucket only; it
recomputes NO weight and renders NO breakdown — the full weighted breakdown stays
at `/score` (the J-002c boundary, WD-GT-7). The dependency edge enabling the reuse
is adjudicated in ADR-045.

### Earned Trust (principle 12)

`viewer-domain` is PURE (no `probe()`). Its Earned-Trust analog is property +
mutation testing of the grouping + render: two-authors → two rows, no-merge,
empty → `NoClaims`, every `EdgeRow` carries author + cid, verbatim confidence,
bucket projected (not recomputed), page embeds fragment (parity). The "environment
lies" question is answered by the effect shell's degrade-to-`NoClaims` on read
failure (ADR-044).

## Alternatives Considered

| Option | Evaluation | Rejected because |
|--------|-----------|------------------|
| **Two separate ADTs** (`ProjectSurvey` + `PhilosophySurvey`) | Maximally explicit. | **Rejected (symmetry).** The two surveys are structurally identical (group by the OTHER dimension; link the key + the contributors); two ADTs would duplicate `EdgeGroup`/`EdgeRow` and the render logic. One `TraversalView` + two constructors + two thin renderers carries the only real asymmetry (which dimension is the key) without duplication. |
| **Render the slice-04 scoring `WeightBucket` on each edge** | Reuse the `/score` bucket. | **Rejected (J-002c / WD-GT-7).** The scoring bucket is a per-pairing WEIGHT bucket (Strong/Moderate/Sparse), computed by the scorer. A survey edge is ONE claim's confidence, not a weighted aggregate — the correct bucket is the per-claim `confidence_bucket`. Showing a weight bucket here would leak the J-002c weighting surface into the survey and require a weight recompute (forbidden). |
| **Recompute the bucket in `viewer-domain`** (inline thresholds) | No new dep edge. | **Rejected (single SSOT, WD-10 / D-12).** The thresholds already live in `claim_domain::confidence_bucket`; a second copy in the viewer is a drift hazard. Reuse the one site (ADR-045 adjudicates the pure→pure dep edge). |
| **A depth-K auto-expanded tree** (the slice-04 `--depth` render) | Richer one-shot view. | **Rejected (out of scope, WD-GT-10).** A survey is depth-1 (entity + direct edges); each edge is a LINK the operator clicks; browser back/forward IS the traversal stack. The CLI `--depth K` tree is explicitly deferred. |
| **Build the `TraversalView` in the effect shell** (grouping in the handler) | Fewer pure functions. | **Rejected (effect/pure symmetry; ADR-042).** The grouping is pure anti-merging logic worth property-testing in isolation; it belongs in `viewer-domain` next to the render (slice-08/09 placed `compose_results`/`score` consumption likewise). |

## Consequences

### Positive
- Anti-merging is a property of the pure constructors (they cannot average), in
  addition to the type-level (non-Option author/cid) and SQL-level guards.
- One ADT + two thin renderers cover both symmetric surfaces; the render logic is
  not duplicated.
- Page = chrome + SAME fragment → parity by construction (I-GT-6), reusing the
  slice-07/08/09 split unchanged.
- The bucket + confidence reuse the single SSOT sites; no recompute; the J-002c
  weighting boundary is clean (link-out only).

### Negative
- `viewer-domain` gains a third projection ADT + four render functions + two
  grouping constructors. Accepted: the symmetric counterpart to `SearchState`
  (slice-08) / `ScoreState` (slice-09), reusing the chrome + fragment split + the
  verbatim-confidence site.

## Revisit Trigger
- Dogfood shows operators want the contributor dimension AS a survey page (not a
  `/score` link) → add a third constructor/renderer over the same ADT.
- A depth-2 inline expansion proves load-bearing → extend `EdgeGroup` to carry a
  nested `Vec<TraversalView>` (a DISTILL/later-slice scenario; deferred now).
