# ADR-032: Fragment / Full-Page Rendering Split — Page Composes the Same Pure Fragment Function (Structural Parity)

- **Status**: Accepted (DESIGN — slice-07 viewer-htmx-swaps)
- **Date**: 2026-06-02
- **Deciders**: Morgan (nw-solution-architect), per OD-HX-2 / OD-HX-3 for viewer-htmx-swaps (slice-07).
- **Feature**: viewer-htmx-swaps (slice-07)
- **Extends**: ADR-029 (maud pure-core), ADR-007 (pure/effect split). Slice-06 `viewer-domain` `render_*_page` functions.
- **Resolves**: OD-HX-2 (fragment/page split), OD-HX-3 (swap-target ids + htmx attributes), and is the structural enforcer of I-HX-5 (parity).

## Context

slice-06's `viewer-domain` renders a COMPLETE HTML document inside each `render_*_page`
function. The list/results/detail markup is already factored into private helpers
(`render_claims_table`, `render_peer_claims_table`, `render_scrape_result`,
`render_claim_fields` + `render_evidence_section`) — but those helpers are PRIVATE, emit
markup WITHOUT the swap-target `id`, and the page wraps them in chrome inline.

slice-07 must serve, for each of four interactions, BOTH shapes of the SAME content:
- **Fragment** (htmx request): just the changed region, with the swap-target `id`.
- **Full page** (non-htmx request): chrome + that same region.

The cardinal integration risk (shared-artifacts-registry.md) is the two shapes
**drifting apart** — the fragment showing different rows, a different `X–Y of N`, a
reformatted confidence, or a dropped peer origin than the full page's region. I-HX-5
requires the fragment to EQUAL the corresponding region of the full page for identical
inputs. Parity must be guaranteed **structurally** (the same code produces both), not by
a convention that two renderers stay in sync.

## Decision

**Factor each view in `viewer-domain` so the page is composed AROUND the same pure
fragment function: `page = chrome(fragment(view-model))`. Promote each region renderer to
a PUBLIC `render_*_fragment(...) -> Markup` that emits the swap-target-`id`-bearing region,
and rewrite each existing `render_*_page(...) -> String` to embed that SAME fragment fn
inside the document chrome. The fragment fn is the single source of the region's markup;
the full page cannot diverge from it because it literally calls it. Both shapes stay PURE
and TOTAL (no I/O, no header awareness — the shell picks which to call, ADR-033).**

### The split, per interaction

For each region, `viewer-domain` exposes a public fragment renderer that wraps the
region in its swap-target container with the canonical `id` (a single `const` per id,
referenced by both the fragment and any full-page chrome that defines the slot):

| Interaction (US) | Public fragment fn (returns `Markup`) | Swap-target id `const` | Full page that embeds it |
|---|---|---|---|
| Claims pagination (US-HX-001) | `render_claims_table_fragment(&PageView<ClaimRowView>)` | `ID_CLAIMS_TABLE = "claims-table"` | `render_claims_page` |
| Peer-claims pagination (US-HX-002) | `render_peer_claims_table_fragment(&PageView<PeerClaimRowView>)` | `ID_CLAIMS_TABLE` (the active view-panel's table) | `render_peer_claims_page` |
| Scrape results (US-HX-003) | `render_scrape_results_fragment(&ScrapeState)` | `ID_SCRAPE_RESULTS = "scrape-results"` | `render_scrape_page` |
| Claim detail (US-HX-004) | `render_claim_detail_fragment(&ClaimDetailView)` and the not-found `render_claim_not_found_fragment()` | `ID_CLAIM_DETAIL = "claim-detail"` | `render_claim_detail` / `render_error` |
| Tab switch (US-HX-006) | the active list fragment swapped into `ID_VIEW_PANEL = "view-panel"` (see ADR-034) | `ID_VIEW_PANEL = "view-panel"` | `render_claims_page` / `render_peer_claims_page` |

**Composition shape (illustrative — crafter owns exact maud):**

```text
render_claims_page(page) =
    DOCTYPE html { head { ... <script src="/static/htmx.min.js"> } body {
        chrome / nav / read-only banner
        render_claims_table_fragment(page)   // <-- the SAME fn the htmx request returns
    } }

render_claims_table_fragment(page) -> Markup =
    div id=(ID_CLAIMS_TABLE) {            // the swap-target container
        (claims table rows + position indicator + Prev/Next)   // slice-06 markup, moved here
    }
```

Pagination Prev/Next anchors, the position indicator, and the confidence-verbatim render
all live INSIDE the fragment, so the indicator string and clamp behavior are identical in
both shapes (position_indicator parity, shared-artifacts-registry.md).

### Swap-target containers carry the id

The `id` lives on the fragment's outer container (`div id=(ID_CLAIMS_TABLE)`), defined in
exactly one place (the fragment fn). The full page contains the same container because it
calls the fragment fn. An htmx swap with `hx-target="#claims-table" hx-swap="outerHTML"`
replaces that container with the fragment's freshly-rendered container — ids agree by
construction (one `const`, one producing function). This resolves OD-HX-3's "both shapes
must agree on every id" structurally.

### Parity is checkable AND structural

- **Structural** (the primary guarantee): the full page calls the fragment fn, so for the
  same `view-model` the region bytes are identical — there is no second renderer to drift.
- **Checkable** (the belt-and-suspenders test, DISTILL/DELIVER): a parity test asserts the
  full-page HTML *contains* the fragment HTML verbatim for the same input
  (`render_claims_page(v).contains(render_claims_table_fragment(v).into_string())`), and an
  acceptance test drives the route with/without `HX-Request` and asserts the fragment's
  region equals the full page's region (same rows, same `X–Y of N`, verbatim confidence,
  peer origin). This is the Earned-Trust probe for the parity contract.

## Alternatives Considered

| Option | Evaluation | Rejected because |
|--------|-----------|------------------|
| **Duplicate renderers** (a separate `render_claims_fragment` and an independent `render_claims_page`, each writing the table markup) | Simple to start. | **Rejected.** Two independent producers of the same region WILL drift (a column added to one, a confidence format tweaked in one) — exactly the I-HX-5 failure mode the registry flags as the cardinal risk. Parity would rest on a convention + tests, not structure. |
| **Templating-include / partials** (a runtime template `{% include %}` shared between page and fragment) | Conventional in template engines. | **Rejected (ADR-029).** Runtime templates were already rejected for the pure core (filesystem I/O breaks ADR-007). maud has no runtime include; the in-code equivalent IS "the page calls the fragment fn" — which is exactly this decision, minus the template engine. |
| **String slicing** (render the full page, then return a substring between markers for the fragment) | One renderer. | **Rejected.** Fragile (marker drift, escaping hazards), couples the fragment to the full page's byte layout, and produces a fragment that carries page-only wrapper bytes. Composing the page FROM the fragment is the clean inverse: the small piece is the unit, the page is the composite. |
| **Header-aware renderer in `viewer-domain`** (pass `is_htmx: bool` into the renderer; it decides shape) | One entry point. | **Rejected (ADR-007 / ADR-033).** The pure core would then know about HTTP/the header — purity breach. The shape decision belongs in the effect shell (ADR-033); the pure core exposes BOTH shapes and stays unaware of why one was chosen. |

## Consequences

### Positive
- I-HX-5 parity is **structural**: the full page embeds the exact fragment fn output; no
  second renderer exists to drift. The registry's cardinal integration risk is closed by
  construction.
- The slice-06 page content is preserved: the page's region markup is the moved-intact
  slice-06 table/results/detail markup, now wrapped in a `div id=...` container — the only
  content delta is the wrapping container + the chrome's `<script src>` line (I-HX-4 — the
  non-htmx full-page body stays byte-stable except for those bounded additions; the
  slice-06 26-scenario suite is the regression gate).
- Fragments stay pure + total + testable with zero substrate (fixture view-model in,
  `Markup` out) — same property as slice-06.
- The swap-target ids are single `const`s shared by both shapes; OD-HX-3 ids agree
  structurally.

### Negative
- The slice-06 `render_*_page` functions are rewritten to delegate to the new fragment fns.
  Accepted: this is a pure refactor (extract-and-compose) whose correctness is pinned by
  the existing slice-06 in-crate tests + the no-regression acceptance suite; the moved
  markup is byte-identical.
- Wrapping each region in a `div id=...` container slightly changes the slice-06 page DOM
  (an added wrapper element). Accepted and bounded: it is an additive structural wrapper,
  not a content change; the no-regression gate asserts the rendered TEXT/rows are
  unchanged, and the wrapper is what makes the swap target addressable.

## Revisit Trigger
- A view needs MORE than its region swapped (e.g. swap two disjoint regions in one
  response) → htmx out-of-band swaps; extend the fragment fns with OOB containers (new ADR
  if it changes the page composition model).
- A fifth interaction is added → add a fragment fn + an id `const`; the pattern generalizes.
