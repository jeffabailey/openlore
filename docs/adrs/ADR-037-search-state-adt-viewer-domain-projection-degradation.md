# ADR-037: The `/search` Render — A New Pure `viewer-domain` Projection of the `appview-domain` Composition, a `SearchState` ADT, and a Payload-Free `Unavailable` Degradation Variant

- **Status**: Accepted / shipped (slice-08 viewer-network-search, DELIVER 2026-06-04). The pure `viewer-domain` `SearchState` ADT (`Form | Results | NoResults | Unavailable`) + `render_search_*` projection of the reused `appview-domain` composition shipped; the payload-free `Unavailable` unit variant (both shapes, unreachable + unconfigured) materialized; 100% mutation kill on the production functions.
- **Date**: 2026-06-04
- **Deciders**: Morgan (nw-solution-architect), resolving OD-NS-2 + OD-NS-3 for viewer-network-search (slice-08).
- **Feature**: viewer-network-search (slice-08)
- **Extends**: ADR-007 (pure/effect split), ADR-025/027 (the `appview-domain` anti-merging composition + the `NetworkResultRow`/`NetworkSearchResult` types reused), ADR-029 (the pure `viewer-domain` render core), ADR-032 (the fragment/page rendering split — page = chrome + fragment), ADR-033 (the `Shape` fork in the effect shell).
- **Resolves**: OD-NS-2 (render reuse vs new viewer-domain fragment) + OD-NS-3 (degradation UX placement/wording).

## Context

US-NS-002/003/004 require the viewer to render the slice-05 verified-attributed
network search results AS HTML — per-author groups, every row carrying `[verified]`
+ `author_did`, `counter_annotation` SHOWN-not-applied, confidence VERBATIM — plus a
guided empty state, a public-data framing banner, an `openlore peer add <did>`
follow GUIDANCE text (render-only, never an executable control), and a graceful
degradation message when the indexer is unreachable/unconfigured.

The slice-05 result COMPOSITION is the pure `appview-domain::compose_results`
returning a `NetworkSearchResult { by_author: Vec<(Did, Vec<NetworkResultRow>)>,
distinct_author_count, total_claims, suggestion }` — per-author grouping, no merged
row, attribution non-`Option`, counter-annotation carried (OD-AV-7 / I-AV-2/9). The
slice-05 RENDER, however, is `crates/cli/src/render` emitting STDOUT TEXT — the wrong
medium for a browser. The viewer already has a pure render core (`viewer-domain`,
ADR-029) using maud, with the established `ScrapeState` ADT pattern (a payload-free
`NetworkDown` unit variant that structurally cannot leak transport internals), the
`page_head`/`htmx_script`/`render_tab_nav` chrome, the `render_confidence` verbatim
contract (FR-VIEW-8 / I-NS-9), and the page = chrome + fragment composition
(ADR-032).

The degradation question (OD-NS-3) is the wording + placement of the
unreachable/unconfigured message: a page-level banner vs a results-region message,
and how to structurally guarantee no leaked transport internals in BOTH the fragment
and full-page shapes (I-NS-2 / WD-NS-4).

## Decision

**Add a NEW pure `viewer-domain` render module that PROJECTS the `appview-domain`
result types into HTML — REUSING the composition (`compose_results`,
`NetworkResultRow`, `NetworkSearchResult`, per-author grouping, anti-merging,
counter-annotation), NOT the CLI stdout text renderer. The render input is a NEW
`SearchState` ADT mirroring `ScrapeState`, and the degradation outcome is a
payload-free `Unavailable` UNIT variant (the structural analog of
`ScrapeState::NetworkDown`) rendered as a FIXED results-region message in both
shapes.**

### The `SearchState` ADT (pure render input, mirrors `ScrapeState`)

```text
pub enum SearchState {
    /// GET /search with no dimension/value submitted: the empty search form
    /// (+ the public-data framing banner). No network call attempted.
    Form,
    /// A submitted search that returned >=1 verified attributed row: the
    /// per-author result region (projected from NetworkSearchResult).
    Results(NetworkSearchResultView),
    /// A submitted search that returned ZERO rows: the guided "no claims found"
    /// state, dimension-aware (object/subject may carry a near-match suggestion;
    /// contributor carries NONE — an absent contributor is not a typo, AV-17 /
    /// EmptyPolicy::NoSuggestion). Names the queried value; never a blank region.
    NoResults(NoResultsView),
    /// The indexer was unreachable OR unconfigured (IndexQueryError::Unreachable,
    /// or no indexer wired): the FIXED plain-language guidance. A UNIT variant
    /// carrying NO transport detail — the raw error/URL/status CANNOT be
    /// interpolated, so no internals can leak by construction (I-NS-2). Mirrors
    /// ScrapeState::NetworkDown exactly.
    Unavailable,
}
```

`NetworkSearchResultView` is a thin view-model projecting `NetworkSearchResult`
(per-author groups → render rows; each row carries `author_did`, the `[verified]`
marker driven by `verified_against`, the verbatim confidence via `render_confidence`,
the `counter_annotation` shown-not-applied, and the render-only `openlore peer add
<did>` guidance text for an unfollowed author). The dimension-aware footer ("N
distinct authors — all verified, no merged row" / "one developer's reasoning trail,
not a community consensus" for contributor) is part of the projection.

### Degradation: a FIXED results-region message, payload-free, both shapes (OD-NS-3)

`Unavailable` renders a single pinned `SEARCH_UNAVAILABLE_NOTICE` constant — a fixed
DOMAIN-language sentence (e.g. *"The network index is unavailable, so no network
results could be fetched. Your local store views still work."*) held in ONE place,
exactly like `SCRAPE_NETWORK_DOWN_NOTICE`. Because the variant is a UNIT variant
carrying no error value, the renderer CANNOT interpolate an HTTP status, "connection
refused", a raw URL, or a stack trace — no-leak is structural, not a discipline
(NFR-VIEW-6/7 / I-NS-2). It appears in the RESULTS REGION (not a page-level banner)
so it forks by `Shape` like every other state: the htmx fragment swaps the
results-region message in place; the full page embeds the SAME message. Both the
unreachable AND the unconfigured cases map to the SAME `Unavailable` variant (the
viewer cannot tell — and must not leak — which).

### Page = chrome + fragment (ADR-032 reused; I-NS-6 parity by construction)

```text
pub const SEARCH_RESULTS_ID: &str = "search-results";

/// The results-region FRAGMENT — no chrome, no form. Forks every SearchState
/// EXCEPT Form's form chrome (the form lives in the page; the fragment is the
/// results region the htmx submit swaps). I-HX-1.
pub fn render_search_results_fragment(state: &SearchState) -> Markup;

/// The full /search page = chrome (head + nav + public-data banner + the search
/// form) wrapped AROUND render_search_results_fragment(state) — the EXACT same
/// fragment fn the htmx shape returns alone. I-HX-5 parity by construction
/// (the results-region logic is NOT duplicated).
pub fn render_search_page(state: &SearchState) -> String;
```

The public-data framing banner (I-NS-5) is page chrome, shown before any results on
`Form` and `Results` alike. `viewer-domain` gains a pure dependency edge on
`appview-domain` (a pure domain crate) to consume `NetworkResultRow`/
`NetworkSearchResult` — see ADR-038 §enforcement for the `check-arch` allowlist.

## Alternatives Considered

| Option | Evaluation | Rejected because |
|--------|-----------|------------------|
| **Reuse the slice-05 CLI `render` (stdout text)** | Maximal reuse. | **Rejected (OD-NS-2 / wrong medium).** The CLI renderer emits plain-text stdout (banner lines, `[verified]` markers in text, ANSI-free columns) — not HTML, not maud, no `Shape` fork, no `hx-*` attributes. Forcing it into a browser would mean HTML-escaping text output or a parallel HTML path anyway. Reuse the COMPOSITION (`compose_results` + the types), project a NEW HTML renderer. |
| **A page-level degradation BANNER (chrome) instead of a results-region message** | One banner site. | **Rejected (OD-NS-3 / shape parity).** A chrome banner does not fork by `Shape` cleanly — an htmx results swap would not replace it, so a stale "unavailable" banner could persist over fresh results (or vice versa). Putting `Unavailable` in the results region makes it ONE more `SearchState` arm that forks identically to `Results`/`NoResults` — both shapes agree by construction. |
| **A `String`-carrying `Unavailable(String)` variant** (interpolate a friendly cause) | Richer message. | **Rejected (I-NS-2 / no-leak structural).** Any payload is an interpolation site where a raw transport string could leak (the exact failure mode `ScrapeState::NetworkDown` was made a unit variant to prevent). A UNIT variant makes leaking impossible, not merely discouraged. |
| **Collapse `NoResults` into `Unavailable`** | Fewer arms. | **Rejected.** A reachable indexer returning zero rows is a VALID not-yet-found state (guided empty + optional near-match) — semantically distinct from "the index is unavailable". Collapsing them would mislead P-001 (a typo'd philosophy would read as "the network is down"). Distinct arms, each with pinned copy (the `ScrapeState::ZeroCandidates` vs `NetworkDown` precedent). |
| **Render in the effect shell (`adapter-http-viewer`) directly** | Fewer crates touched. | **Rejected (ADR-007/029).** Rendering is pure; it belongs in `viewer-domain`. The effect shell builds the `SearchState` and forks by `Shape` only. |

## Consequences

### Positive
- The verified/attributed/anti-merging guarantees are NOT reimplemented: the viewer
  consumes `appview-domain::compose_results` and the non-`Option` `author_did`
  `NetworkResultRow`, so per-author grouping, the no-merge invariant, and the
  counter-shown-not-applied behavior carry over by construction (I-NS-3/4/9 / WD-NS-5).
- Degradation cannot leak internals: the `Unavailable` unit variant + the single
  pinned notice make no-leak a type-level property (I-NS-2), in both shapes.
- Fragment/full-page parity is structural (page embeds the fragment fn, ADR-032 /
  I-NS-6) — the same load-bearing slice-07 contract the existing four routes uphold.
- Confidence renders verbatim via the SAME `render_confidence` (I-NS-9 / FR-VIEW-8).
- The follow affordance is a render-only `openlore peer add <did>` text node — no
  `<form>`, no `<button>`, no `hx-*` — so the viewer stays read-only (WD-NS-3 / I-NS-1).

### Negative
- `viewer-domain` takes a new pure dependency on `appview-domain`. Accepted: both are
  pure domain crates (no I/O); the edge is `viewer-domain → appview-domain` (never the
  reverse). Requires a `check-arch` pure-core allowlist confirmation (ADR-038).
- A new render module + view-model in `viewer-domain`. Accepted: it is the symmetric
  counterpart to the existing `ScrapeState`/`render_scrape_*` surface and reuses the
  shared chrome (`page_head`, `render_confidence`, the nav).

## Revisit Trigger
- The result set grows large enough to need pagination in the browser → reuse the
  existing `PageView<T>` machinery (generic; already in `viewer-domain`); add a paged
  `SearchState::Results`. Out of scope for the walking skeleton.
- A new search dimension is added → widen the dimension projection; the `SearchState`
  ADT and the `Shape` fork stay total.
