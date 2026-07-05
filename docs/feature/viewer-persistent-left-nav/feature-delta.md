# Feature Delta — viewer-persistent-left-nav (slice-21)

> DISCUSS wave output. Density: **lean** (Tier-1 [REF] only). Persistent left
> navigation across the read-only `openlore ui` viewer surfaces.

---

## Wave: DISCUSS / [REF] Persona

**P-001 — Senior Engineer Solo Builder** (`docs/product/personas/senior-engineer-solo-builder.yaml`),
wearing a new **viewer-navigator** hat: browsing across the read-only viewer's
exploration/reading surfaces (`/claims`, `/peer-claims`, `/search`, `/score`,
`/project`, `/philosophy`, `/peers`) to inform a tech/community decision, and
wanting the navigation to stay put so moving between surfaces never costs a
re-orientation. Secondary: **P-002 — Researcher / Tech Lead** wears the same hat
(the graph-explorer of `explore-the-graph.yaml`).

## Wave: DISCUSS / [REF] JTBD one-liner

**Job (aspect of J-002 — "Explore the philosophy graph to inform a decision"):**
> When I'm moving between the viewer's surfaces to explore the graph, I want the
> navigation to stay open and mark where I am, so I can jump between surfaces
> fluidly without losing my place or hunting for the way back.

Traceability: **`job_id: J-002`** (the navigation-heavy exploration job; the
persistent nav also lowers friction for the J-003/J-004/J-005 reading surfaces,
but J-002 is the primary traversal-across-surfaces job). This is a UX-affordance
improvement to an existing job — it introduces no new job.

- **Functional**: reach every viewer surface from every viewer surface in one click.
- **Emotional**: oriented, in-control ("I always know where I am and how to get back").
- **Social**: n/a (single-user local viewer).

## Wave: DISCUSS / [REF] Locked decisions

- **[D1] Nav on every surface.** A left navigation renders on ALL viewer HTML
  routes, not just the landing hub (`GET /`). Rationale: it can only "stay open"
  if it exists everywhere. (see: current gap — `LANDING_HUB_SURFACES` renders
  only on `GET /`.)
- **[D2] Persist via progressive-enhancement swap.** Nav clicks swap only the
  main content region while the left-nav shell stays in the DOM (no tear-down,
  no flash, no scroll-reset), the address bar updates, and browser back/forward
  work — reusing the shipped htmx `hx-get`/`hx-target`/`hx-push-url` mechanism
  (slice-07, ADR-031..035). No-JS falls back to plain `<a href>` full-page
  navigation, and the nav still renders on each full page (I-HX-1/4/5 progressive
  enhancement preserved). *The exact swap mechanism (hx-boost vs per-link
  hx-get + shared target) is a DESIGN decision; DISCUSS fixes only the observable
  outcome.*
- **[D3] Active-surface indicator.** The nav marks the current surface as active
  (a neutral current-page marker) so the user always knows where they are.
- **[D4] Single source of truth for the item set.** The persistent nav reuses
  the existing `LANDING_HUB_SURFACES` (label, URL-const) table — no second,
  driftable list. Adding a surface stays a one-line change in one place.
- **[D5] Read-only / offline / loopback preserved.** Nav items are plain links
  only — NO form, button, or mutating control (I-VIEW-1/3); all assets local /
  vendored (offline, no CDN); loopback-only (I-VIEW-4). The nav renders offline
  and holds no signing key.

## Wave: DISCUSS / [REF] User stories

### US-NAV-001 — A left navigation on every viewer surface

As P-001 browsing the read-only viewer, I want a left navigation listing every
viewer surface to be present on every page, so I can reach any surface from
wherever I am without going back to the landing page.

`job_id: J-002`

#### Elevator Pitch
Before: the nav hub exists only on `GET /`; from `/claims` or `/score` there is no on-page way to reach another surface — I must edit the URL or hit back.
After: open `http://127.0.0.1:8788/claims` → sees a left navigation listing all viewer surfaces (My Claims, Peer Claims, Search, Score, Project, Philosophy, Peers), with the current surface marked active.
Decision enabled: I decide which surface to explore next and go there in one click, staying oriented.

#### Acceptance Criteria
- **AC-001.1** GIVEN the viewer is running, WHEN I `GET` any viewer surface
  (`/`, `/claims`, `/peer-claims`, `/search`, `/score`, `/project`, `/philosophy`,
  `/peers`), THEN the response HTML contains a left navigation region listing all
  surfaces from `LANDING_HUB_SURFACES`, each as a plain `<a href>` to its URL const.
- **AC-001.2** GIVEN I am on surface X, WHEN the page renders, THEN the nav marks
  exactly ONE item — surface X — as the active/current item, and no other.
- **AC-001.3** GIVEN the nav item set, THEN it is derived from the SAME
  `LANDING_HUB_SURFACES` table (no second literal list); a surface absent from
  that table is absent from the nav, and vice-versa.
- **AC-001.4** GIVEN JavaScript is disabled (curl / no-JS browser), WHEN I load any
  surface, THEN the left nav still renders and every nav link is a working
  full-page navigation (progressive enhancement, I-HX-4).
- **AC-001.5** GIVEN any nav render, THEN it contains NO form, button, or mutating
  control and references NO external (non-loopback) asset (read-only/offline, I-VIEW-1/3/4).

### US-NAV-002 — The left nav stays open across navigation

As P-001 clicking between viewer surfaces, I want the left navigation to stay in
place as the content changes, so moving between surfaces never re-flashes or
resets the nav and I never lose my place.

`job_id: J-002`

#### Elevator Pitch
Before: every surface change is a full-page reload — the whole page (nav included) is torn down and repainted, scroll resets, and there is a visible flash.
After: with htmx active, click a left-nav item → the main content region swaps to that surface while the left-nav shell stays mounted; the address bar updates to the surface URL, and browser Back returns to the previous surface.
Decision enabled: I explore several surfaces in a row fluidly, treating the viewer as one app rather than a set of separate pages.

#### Acceptance Criteria
- **AC-002.1** GIVEN htmx is active, WHEN I click a left-nav item for surface Y,
  THEN only the main content region is replaced with surface Y's fragment; the
  left-nav element is NOT re-requested or torn down (it persists across the swap).
- **AC-002.2** GIVEN the boosted navigation, THEN the browser address bar updates
  to surface Y's URL (`hx-push-url`), AND browser Back returns to the prior
  surface's content (history works).
- **AC-002.3** GIVEN a boosted nav click, THEN the newly-active surface is marked
  active in the (persisted) nav and the previously-active item is no longer marked.
- **AC-002.4** GIVEN a boosted swap, THEN the rendered content for surface Y is
  byte-equivalent to the content region of surface Y's full-page render
  (parity — the same surface, one code path, I-HX-5).
- **AC-002.5** GIVEN the no-JS path, WHEN a nav link is followed as a plain link,
  THEN the full-page render of surface Y is byte-unaffected by this feature versus
  its prior full-page render EXCEPT for the added nav region (no-regression).

## Wave: DISCUSS / [REF] Outcome KPIs

| KPI | Target | Measurement |
|-----|--------|-------------|
| KPI-NAV-1 — Nav reach | 100% of the 8 viewer surfaces expose the left nav | Acceptance test asserts the nav region on every route. |
| KPI-NAV-2 — Persistence | Boosted nav click replaces only the content region; nav element identity preserved (0 nav re-renders) | AT asserts the nav node is not in the swapped fragment; content-only swap. |
| KPI-NAV-3 — Progressive enhancement | 100% of nav links work as full-page navigation with JS off | No-JS AT (curl) asserts nav + working links on every surface. |
| KPI-NAV-4 — No regression | Existing surface content byte-identical except for the added nav region | Baseline diff AT against pre-feature full-page renders. |
| KPI-NAV-5 — Read-only/offline | 0 mutating controls, 0 external asset references in nav | Gold invariant AT (blocklist scan). |

## Wave: DISCUSS / [REF] Definition of Done

1. US-NAV-001 + US-NAV-002 ACs all pass as automated acceptance tests.
2. Left nav renders on all 8 viewer routes (full-page + boosted).
3. Active-surface indicator correct on every route and after a boosted swap.
4. No-JS fallback verified (curl): nav + working full-page links on every surface.
5. Content parity: boosted swap content == full-page content region (byte-equal).
6. No-regression: prior surface content byte-identical except the added nav region.
7. Read-only/offline/loopback gold invariants green (no control, no CDN).
8. `xtask check-arch` OK (workspace stays 21 members; no new crate).
9. Nav item set sourced solely from `LANDING_HUB_SURFACES` (no drift; single-source test).

## Wave: DISCUSS / [REF] Out of scope

- A collapse/expand toggle, animation, or remembered collapsed-state.
- Mobile hamburger / responsive breakpoints (beyond not breaking existing layout).
- Any NEW viewer surface or route.
- Any mutating control, auth, or signing affordance in the nav (stays CLI-only).
- Keyboard-shortcut navigation, breadcrumbs, or search-within-nav.
- Re-theming / visual redesign beyond placing the nav on the left.

## Wave: DISCUSS / [REF] WS strategy

**Strategy B (extend existing) — no walking skeleton.** Brownfield: the viewer
(slice-06) and htmx swap mechanism (slice-07) are shipped; the landing nav hub
(slice-17) already lists all surfaces. This slice extends the existing viewer
chrome (`page_head` / `render_tab_nav` in `viewer-domain::common`) to render the
nav on every route and boost its links. One thin end-to-end slice.

## Wave: DISCUSS / [REF] Driving ports

- Viewer HTTP GET routes (the 8 surfaces) served by `adapter-http-viewer`.
- Pure render surface in `viewer-domain` (the nav is server-rendered HTML chrome;
  `Shape::from_request` already forks fragment vs full-page for htmx).

## Wave: DISCUSS / [REF] Pre-requisites

- slice-06 `htmx-scraper-viewer` (viewer + `page_head` chrome) — SHIPPED.
- slice-07 `viewer-htmx-swaps` (vendored htmx 2.0.4, `hx-get`/`hx-target`/`hx-push-url`,
  `Shape::from_request`, page = chrome + fragment parity) — SHIPPED.
- slice-17 `viewer-landing-dashboard` (`LANDING_HUB_SURFACES` SSOT of surfaces) — SHIPPED.

## Wave: DISCUSS / [REF] Story map

Single activity — **Navigate the viewer** — one thin slice:

```
Navigate the viewer
└── slice-21 · viewer-persistent-left-nav   (US-NAV-001 + US-NAV-002)
    left nav on every surface + persists across boosted navigation + no-JS fallback
```

No release-bucket split: one slice, ships end-to-end in ≤1 day. Slice brief:
`docs/feature/viewer-persistent-left-nav/slices/slice-21-persistent-left-nav.md`.

## Wave: DISCUSS / [REF] Definition of Ready (9/9)

1. **User need clear** ✓ — persistent left nav across viewer surfaces (J-002 aspect).
2. **Job traceability** ✓ — both stories `job_id: J-002` (existing validated job).
3. **Stories have elevator pitches** ✓ — Before/After/Decision on US-NAV-001/002, real endpoints + observable output.
4. **ACs testable** ✓ — every AC is a Given/When/Then over an HTTP route with observable HTML (nav presence, active marker, swap target, no-JS render, byte-parity).
5. **Outcome KPIs measurable** ✓ — KPI-NAV-1..5 each numeric/binary with an AT measurement method.
6. **Scope bounded** ✓ — explicit out-of-scope; single slice; no new route/crate.
7. **Dependencies satisfied** ✓ — all three prerequisite slices SHIPPED.
8. **Invariants named** ✓ — I-VIEW-1/3/4, I-HX-1/4/5 preserved (D5); check-arch 21.
9. **Sizing** ✓ — one elephant-carpaccio slice, ≤1 day, one learning hypothesis (see brief).

## Wave: DISCUSS / [REF] Wave decisions summary

### Key Decisions
- [D1] Nav on every surface — persistence is impossible if the nav only exists on `/`.
- [D2] Persist via the shipped htmx boosted-swap mechanism; no-JS full-page fallback preserved.
- [D3] Active-surface indicator so the user is always oriented.
- [D4] Reuse `LANDING_HUB_SURFACES` as the single source of the item set (no drift).
- [D5] Read-only/offline/loopback invariants preserved (plain links only).

### Requirements Summary
- Primary need: a left navigation that stays open and current as the user moves between viewer surfaces.
- Walking skeleton: none (brownfield strategy B — extend existing chrome).
- Feature type: user-facing (viewer UX).

### Constraints Established
- No new crate/route (workspace stays 21; `check-arch` OK).
- Nav item set single-sourced from `LANDING_HUB_SURFACES`.
- Progressive enhancement mandatory: no-JS renders the nav + working full-page links.
- Byte-parity (boosted == full-page content) and no-regression (prior content unchanged but for the nav).

### Upstream Changes
- None. No DISCOVER artifacts existed; no prior assumptions changed. SSOT `docs/product/jobs.yaml`
  J-002 gains a navigation-affordance aspect (recorded here; no job re-scoring).

## Wave: DISCUSS / [REF] Changed Assumptions (DESIGN back-propagation, 2026-07-05)

The DESIGN review (ADR-058) found that AC-002.1 as originally written conflicts
with AC-002.3: a strictly-untouched nav (outside the swapped `#viewer-main`) can
never update its active marker after a boosted navigation.

- **Original AC-002.1 (this artifact):** "only the main content region is replaced;
  the left-nav element is NOT re-requested or torn down (it persists across the swap)."
- **Refined AC-002.1 (DESIGN, ADR-058 D5):** on a boosted navigation the nav
  CONTAINER (`<nav id="viewer-nav">`) persists and stays visually stable — no page
  reload, no flash, no scroll-reset — and its active marker updates IN PLACE via an
  out-of-band swap of the nav's inner link list (`<ul id="viewer-nav-items"
  hx-swap-oob="innerHTML">`). The nav is NOT re-fetched as a page and the container
  element is not destroyed; only its links (carrying the new active marker) are
  replaced.
- **Rationale:** honors the user intent ("keep the nav open / stable while moving")
  AND the active-marker requirement (AC-002.3) together, which the original
  over-strict wording made mutually exclusive. The out-of-band mechanism is standard
  htmx and adds no new asset. AC-002.3/002.4 are unchanged.

DISCOVER documents are unmodified (none exist for this feature).
