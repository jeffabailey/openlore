# ADR-034: Tab-Switch History — `hx-push-url` Keeps the Real URLs, Converging the htmx Path with the No-JS Path

- **Status**: Accepted / shipped (slice-07 viewer-htmx-swaps, DELIVER 2026-06-02)
- **Date**: 2026-06-02
- **Deciders**: Morgan (nw-solution-architect), per OD-HX-4 for viewer-htmx-swaps (slice-07).
- **Feature**: viewer-htmx-swaps (slice-07)
- **Extends**: ADR-032 (fragment/page split — the tab swaps the view-panel fragment), ADR-033 (the same URL serves both shapes by header).
- **Resolves**: OD-HX-4 (tab-switch URL/history strategy) + the `active_view_url` shared artifact contract.

## Context

The My Claims ↔ Peer Claims tabs map to the existing real routes `/claims` and
`/peer-claims`. In the no-JS path, a tab is a plain `<a href="/peer-claims">` link → a
full-page navigation that changes the browser URL natively (bookmarkable, Back works). In
the htmx path (US-HX-006), the tab swaps the active view-panel fragment in place WITHOUT a
navigation — so by default the browser URL would NOT change, breaking bookmark/Back/reload
and diverging from the no-JS path.

The contract (`active_view_url`, FR-HX-4, US-HX-006 AC): after an htmx tab swap, the
browser URL must reflect `/claims` or `/peer-claims` so the active view is bookmarkable,
Back works, and reloading that URL re-enters via the full page (ADR-033) for that view —
converging the htmx path onto the SAME real URLs the no-JS path already uses.

## Decision

**The tab links are real anchors to `/claims` and `/peer-claims` (the no-JS path, intact).
For the htmx path, add htmx attributes to the tab anchors so the swap targets the view
panel AND pushes the real URL into history: `hx-get="/peer-claims"`,
`hx-target="#view-panel"`, `hx-swap="innerHTML"` (or `outerHTML` of the panel's inner
region), and `hx-push-url="true"`. `hx-push-url="true"` makes htmx push the SAME path it
fetched (`/peer-claims`) onto `history`, so the address bar shows the real URL and Back/
forward replay the swaps. Reloading or bookmarking that URL hits ADR-033's dispatch with NO
`HX-Request` (a fresh navigation) → the FULL page for that view. The htmx and no-JS paths
converge on identical URLs.**

### How it composes with the other ADRs

- The tab anchors live in the pure chrome (`viewer-domain`), emitted as ordinary markup:
  `a href="/peer-claims" hx-get="/peer-claims" hx-target="#view-panel" hx-swap="innerHTML"
  hx-push-url="true" { "Peer Claims" }`. The `href` is the no-JS fallback; htmx enhances the
  same anchor. (Pure core stays unaware of HTTP — these are static attribute strings.)
- `hx-get="/peer-claims"` is the SAME URL as the `href` and the SAME URL ADR-033 dispatches
  on by header — so the swap response is the view-panel fragment (ADR-032), and a later
  reload of `/peer-claims` (no header) is the full page. One URL, two shapes, one history
  entry.
- `hx-target="#view-panel"` (`ID_VIEW_PANEL`, ADR-032): the tab swaps the active view panel,
  which contains the list table. The list fragment returned for `/peer-claims` under
  `HX-Request` is `render_peer_claims_table_fragment` wrapped to land in `#view-panel`.
- Because the pushed URL equals the fetched URL equals the no-JS href, there is exactly ONE
  source of truth for "where am I" — the path — and bookmark/Back/reload/curl all behave
  identically to a direct navigation (no divergence, no htmx-only state).

## Alternatives Considered

| Option | Evaluation | Rejected because |
|--------|-----------|------------------|
| **No history update** (swap silently, leave the URL) | Simplest; fewest attributes. | **Rejected (FR-HX-4).** Bookmark/Back/reload break; the htmx path diverges from the no-JS path (the address bar lies about which view is showing). Fails the `active_view_url` contract outright. |
| **`hx-push-url="/some/other/path"`** (push a synthetic/fragment-specific URL) | Could namespace fragment state. | **Rejected (BR-HX-1 / convergence).** A synthetic URL would NOT be a real route → reload/bookmark would 404 or need a new route (forbidden: no new data routes). The whole point is to push the REAL route so reload re-enters via the full page. `hx-push-url="true"` (push the fetched URL) is exactly right. |
| **`hx-replace-url`** (replace instead of push) | No extra history entries. | **Rejected (Back must work).** Replace clobbers the previous entry, so Back would NOT return to the prior view (US-HX-006 Ex 2 requires Back → My Claims after switching to Peer Claims). Push preserves the Back stack. |
| **`hx-boost` on the whole nav** (boost all links into ajax) | One attribute, blanket enhancement. | **Rejected (scope + control).** Boosting swaps the whole `<body>` by default (not the targeted view panel), reintroducing a near-full repaint and losing the precise `#view-panel` target; it also enhances links we do not want swapped. Per-anchor `hx-get`/`hx-target`/`hx-push-url` is the precise, in-place mechanism US-HX-006 asks for. |

## Consequences

### Positive
- The htmx tab path and the no-JS path use the SAME real URLs; bookmark, Back, forward, and
  reload all behave like a direct navigation (FR-HX-4 / `active_view_url` satisfied).
- Reload/bookmark of a switched URL re-enters via the full page (ADR-033's no-header arm) —
  progressive enhancement holds (I-HX-1): the htmx path is purely additive over the real URL.
- One source of truth for "current view" — the path — so there is no htmx-only history state
  to keep in sync.
- No new route: `/claims` and `/peer-claims` already exist; the tab just enhances the
  anchors (BR-HX-1).

### Negative
- The tab anchors carry both `href` (no-JS) and htmx attributes (enhanced) — slight markup
  duplication of the path string on each tab. Accepted: it is what makes the SAME anchor
  work in both modes; a single chrome helper emits both from one path value.
- Correct Back/forward behavior depends on htmx's history cache restoring the swapped
  panel; htmx handles this, but it is a behavior to verify. Accepted: the acceptance test
  (DELIVER) drives switch → Back → assert My Claims, and reload → assert full page.

## Revisit Trigger
- Pagination within a view should also be bookmarkable (`?page=N` in the URL after a paging
  swap) → add `hx-push-url="true"` to the Prev/Next anchors too (same mechanism; a small
  extension, not a new ADR).
- A view gains client-only state that is not a real URL → would need a different history
  model (new ADR).
