# slice-21 · viewer-persistent-left-nav

**Goal:** A left navigation renders on every read-only viewer surface, stays
mounted (open) across boosted navigation, marks the current surface, and falls
back to plain full-page links with JS off.

## IN scope
- Render a left-nav region on ALL 8 viewer routes (`/`, `/claims`, `/peer-claims`,
  `/search`, `/score`, `/project`, `/philosophy`, `/peers`), sourced from
  `LANDING_HUB_SURFACES`.
- Boost nav links so a click swaps only the main content region while the nav
  shell persists; `hx-push-url` updates the address bar; Back/forward work.
- Active-surface indicator (exactly one active item, correct on full-page and
  after a boosted swap).
- No-JS fallback: nav + working full-page `<a href>` links on every surface.

## OUT of scope
- Collapse/expand toggle, animation, remembered collapsed state.
- Mobile hamburger / responsive redesign.
- Any new route/surface, any mutating control, any visual re-theme beyond
  left-placement.

## Learning hypothesis
- **Confirms if it succeeds:** the shipped htmx boosted-swap shell generalizes
  from the single My↔Peer tab (`#view-panel`) to a whole-viewer persistent nav
  with content-only swaps and correct history — i.e. the viewer can behave as one
  app without a JS framework.
- **Disproves if it fails:** that a server-rendered persistent shell + htmx boost
  can keep the nav mounted AND preserve full-page/no-JS parity — if the boosted
  content and the full-page content diverge, or the nav can't stay mounted without
  a client framework, the "progressive-enhancement one-app viewer" assumption is
  wrong and DESIGN must reconsider the chrome model.

## Acceptance criteria
See `feature-delta.md` → US-NAV-001 (AC-001.1…5) + US-NAV-002 (AC-002.1…5).

## Data
Production data (real local store): the viewer renders the operator's own claims
+ pulled peer claims. Navigation is data-agnostic, but ATs seed a real store
(via `openlore init` + `claim add`, as the slice-06+ viewer ATs already do) so
each surface renders genuine content, not an empty stub.

## Dogfood moment
Same day: run `./run.sh --seed`, open the viewer, and click across all surfaces
from the left nav without the page flashing or the nav resetting.

## Dependencies
- slice-06 (viewer + `page_head` chrome) — SHIPPED.
- slice-07 (htmx boost mechanism: `hx-get`/`hx-target`/`hx-push-url`,
  `Shape::from_request`, chrome+fragment parity) — SHIPPED.
- slice-17 (`LANDING_HUB_SURFACES` SSOT) — SHIPPED.

## Effort estimate
≤1 day (≤6h crafter dispatch). Pure `viewer-domain` chrome render change +
`adapter-http-viewer` shape wiring + ATs. No new crate/route/read-method.

## Reference class
Slices 07 (htmx swaps) and 17 (landing hub) — the same chrome/nav surface,
same htmx mechanism, comparable size (each ≤1 day, render + AT only).

## Pre-slice SPIKE
Not required. The htmx boost mechanism is already shipped and exercised by the
My↔Peer tab (slice-07); this generalizes a proven pattern. (If DESIGN finds the
whole-page boost interacts badly with the existing `#view-panel` tab target, a
30-minute probe on one route would de-risk — but that is a DESIGN-time call.)

## Taste tests
- Ships 4+ new components? NO — extends existing chrome (nav render + boost attrs).
- Every slice depends on a new abstraction? NO — reuses `LANDING_HUB_SURFACES`
  + the shipped htmx attrs; no new abstraction.
- Disproves a pre-commitment? YES — tests whether progressive-enhancement boost
  keeps the nav mounted with full-page/no-JS parity (see learning hypothesis).
- Synthetic-data only? NO — ATs seed a real store via the production write path.
- 2+ slices identical but for scale? N/A — single slice.
