# ADR-058: Persistent viewer left-nav — an `hx-boost` content-region swap into an outer `#viewer-main`, an `HX-Boosted` full-page shape fork, and a `LANDING_HUB_SURFACES`-sourced nav on every route

## Status

Accepted (DESIGN wave, slice-21 `viewer-persistent-left-nav`, 2026-07-05). Extends
ADR-031..035 (slice-07 htmx progressive-enhancement swaps) and ADR-054 (slice-17
`LANDING_HUB_SURFACES` nav-hub SSOT). Owner: Morgan (nw-solution-architect).

## Context

Today the read-only `openlore ui` viewer renders a navigation hub ONLY on the
landing page (`GET /`): `render_landing` emits the 8 `LANDING_HUB_SURFACES`
`(label, URL-const)` pairs as plain `<a href>` links (ADR-054). The inner
surfaces (`/claims`, `/peer-claims`, `/search`, `/score`, `/project`,
`/philosophy`, `/peers`) carry NO cross-surface navigation — from `/score` the
only way to another surface is to edit the URL or press Back. And every
surface-to-surface move is a FULL-PAGE reload (plain `<a href>`), so nothing
"stays open": the whole page — nav included — is torn down and repainted.

DISCUSS (`feature-delta.md`) fixed the observable outcome (US-NAV-001/002):
a left navigation present on EVERY surface, that STAYS MOUNTED across navigation
(content-only swap, no re-flash/scroll-reset), marks the current surface, and
falls back to plain full-page links with JS off — preserving the read-only /
offline / loopback (I-VIEW-1/3/4) and progressive-enhancement (I-HX-1/4/5)
invariants.

The existing htmx machinery already has TWO swap targets that must keep working
unchanged:

- **`#view-panel`** (ADR-034) — the My Claims ↔ Peer Claims tab swap (`render_tab_nav`,
  `hx-target="#view-panel"`).
- **`#claims-table`** (ADR-032) — the nested pagination swap (`?page=N`).

The `Shape` fork (`adapter-http-viewer::Shape::from_request`, ADR-033) currently
keys on ONE bit: the presence of the `HX-Request` header → `Fragment`, absence →
`FullPage`. Both swap targets above ride the `Fragment` shape.

The design problem: add a THIRD, OUTER swap concept — the left nav replaces the
whole surface content region while itself persisting — WITHOUT (a) a JS framework,
(b) disturbing the two existing fragment swaps, or (c) breaking full-page / no-JS
parity. The naïve "explicit `hx-get` + `hx-target=#viewer-main`" fails because an
`HX-Request` GET returns a *fragment*, which does not contain `#viewer-main` for
`hx-select` to extract, and the existing `Shape` fork cannot tell a left-nav
request from a tab request (both send only `HX-Request`).

## Decision

Introduce an **outer content region** `<main id="viewer-main">` that wraps each
surface's body, render a **persistent left nav OUTSIDE it on every full page**,
and drive nav navigation with **`hx-boost`** — which fetches the FULL page and
lets htmx client-side-`hx-select` the content region. This sidesteps the
fragment-granularity problem entirely and makes byte-parity structural.

### D1 — Outer content region `#viewer-main` (PURE, viewer-domain)

- New const `VIEWER_MAIN_ID = "viewer-main"` (SSOT for the outer swap target,
  sibling of `VIEW_PANEL_ID` / `CLAIMS_TABLE_ID`).
- New `page_shell(title, active, content: Markup) -> String` chrome helper:
  `page_head(title)` + `<body>` { `render_viewer_nav(active)` (OUTSIDE main) +
  `<main id="viewer-main">` `(content)` }. Every `render_*_page` composes its
  existing body through `page_shell` — the surface body becomes the `content`
  argument, so `#viewer-main`'s inner HTML IS the surface's content region.

### D2 — Persistent left nav from the existing SSOT (PURE, viewer-domain)

- New `render_viewer_nav(active: &str) -> Markup` iterates `LANDING_HUB_SURFACES`
  (the SAME slice-17 table — NO second list, AC-001.3) emitting one `<a href=url>`
  per surface. The `<nav>` carries `hx-boost="true"` + `hx-target="#viewer-main"`
  + `hx-select="#viewer-main"` + `hx-swap="innerHTML"` (cascades to its `<a>`
  children). `hx-boost` auto-pushes the URL into history (Back/forward work,
  AC-002.2). The item whose URL equals `active` gets `aria-current="page"` (a
  neutral, semantic current-page marker — AC-001.2/002.3); no other item does.
- The landing page's prior inline `LANDING_HUB_SURFACES` hub is SUPERSEDED by this
  persistent nav (same source, rendered once, in the shell) — the landing content
  region keeps only its store summary. Single-source preserved (AC-001.3).

### D3 — `HX-Boosted` full-page shape fork (EFFECT, adapter-http-viewer)

Extend `Shape::from_request` with ONE prior condition:

```
HX-Boosted present            -> FullPage   (boosted nav: return the full page so
                                             the client hx-selects #viewer-main)
else HX-Request present        -> Fragment   (existing tab #view-panel + paging
                                             #claims-table swaps — UNCHANGED)
else                           -> FullPage   (direct load / no-JS)
```

`hx-boost` requests carry BOTH `HX-Request` and `HX-Boosted: true`; the existing
tab/paging `hx-get`s carry `HX-Request` only. So the new arm is invisible to every
existing swap, and boosted requests receive the full page. htmx's `hx-select=
"#viewer-main"` then extracts exactly the full page's content region → the swapped
content is **byte-equivalent to the full-page content region by construction**
(AC-002.4), and the left nav (outside `#viewer-main`) is never in the swapped
payload, so it stays mounted (AC-002.1).

### D5 — Active-marker update via an out-of-band nav swap (resolves the AC-002.1↔002.3 conflict)

A content-only swap into `#viewer-main` leaves the nav (outside it) with a STALE
active marker — the new surface renders, but the nav still highlights the old one.
Resolving this WITHOUT re-flashing/reloading the nav:

- The persistent nav is `<nav id="viewer-nav">` wrapping its link list in an inner
  container `<ul id="viewer-nav-items">`.
- On a **boosted** request (`HX-Boosted`), the effect shell returns the full page
  (for `hx-select="#viewer-main"`) AND ALSO emits an **out-of-band** copy of the
  nav's link list: `render_viewer_nav_oob(active)` → `<ul id="viewer-nav-items"
  hx-swap-oob="innerHTML">…active-updated links…</ul>`. htmx processes OOB content
  independently of `hx-select`, replacing ONLY the inner `<ul>`'s children — so the
  `<nav id="viewer-nav">` CONTAINER persists (never torn down, no scroll-reset, no
  flash) while the active marker updates in place (AC-002.1 refined + AC-002.3).
- The pure core stays header-unaware: it offers `render_viewer_nav(active)` (full
  page) and `render_viewer_nav_oob(active)` (the OOB `<ul>`); the EFFECT shell
  chooses to append the OOB copy only for boosted responses. On a direct/no-JS
  full-page load no OOB copy is emitted (and htmx ignores `hx-swap-oob` on initial
  load anyway), so the no-JS path is unaffected.

This refines DISCUSS AC-002.1: the guarantee is that the nav CONTAINER persists and
stays visually stable (no page reload, no flash, no scroll-reset) — its active
marker updates in place via the OOB `<ul>` swap, NOT a full nav re-render. See the
`feature-delta.md` "Changed Assumptions (DESIGN back-propagation)" note.

**Concrete boosted-response body (e.g. `GET /score` with `HX-Boosted`).** The OOB
`<ul>` is appended at BODY-END, after `<main>` — a single sibling with the SAME id
as the in-shell list, carrying `hx-swap-oob="innerHTML"` and the active marker set to
`/score`. htmx swaps `#viewer-main` (via `hx-select`) and, independently, replaces
`#viewer-nav-items`' children from the OOB copy:

```html
<html>
  <head>…page_head…</head>
  <body>
    <nav id="viewer-nav"><ul id="viewer-nav-items"><!-- links, active=/score --></ul></nav>
    <main id="viewer-main"><!-- score surface content --></main>
    <!-- OOB: appended by the effect shell ONLY on boosted responses -->
    <ul id="viewer-nav-items" hx-swap-oob="innerHTML"><!-- same links, active=/score --></ul>
  </body>
</html>
```

Pure-core signatures: `render_viewer_nav(active: &str) -> Markup` (the in-shell
`<nav id="viewer-nav">` with `<ul id="viewer-nav-items">`) and
`render_viewer_nav_oob(active: &str) -> Markup` (JUST the
`<ul id="viewer-nav-items" hx-swap-oob="innerHTML">…</ul>` sibling). The effect
shell appends `render_viewer_nav_oob(active)` to the full-page string for boosted
responses only; direct / no-JS loads emit neither the OOB copy nor any `hx-swap-oob`.

### D6 — `page_shell` refactor shape, `active` provenance, and 404

- **Refactor shape.** `page_shell(title, active, content: Markup) -> String` owns
  `(DOCTYPE)` + `<html>` + `page_head(title)` + `<body>` { `render_viewer_nav(active)`
  OUTSIDE `<main id="viewer-main">` `(content)` }. Each `render_*_page` KEEPS its
  signature and return type; internally it stops spelling out DOCTYPE/head/body and
  instead builds its surface body as `Markup` and returns
  `page_shell(title, active, body)`. Example (claims):
  ```
  pub fn render_claims_page(page: &PageView<ClaimRowView>) -> String {
      let body = html! { (render_tab_nav()) (render_claims_view_panel_fragment(page)) };
      page_shell("OpenLore — My Claims", MY_CLAIMS_URL, body)
  }
  ```
  The `render_*_fragment` fns are UNCHANGED (tab/paging swaps ride `Shape::Fragment`).
- **`active` provenance.** The `active` argument is the surface's own URL const
  (`MY_CLAIMS_URL`, `SEARCH_URL`, …) — a compile-time constant known at each
  `render_*_page` call site. It is NOT read from the request; `render_viewer_nav`
  marks the `LANDING_HUB_SURFACES` row whose URL equals `active` with
  `aria-current="page"`. (For paths with query strings, e.g. `/project?subject=…`,
  the active key is the base path const `PROJECT_URL`.)
- **404.** `render_error` (the unknown-claim 404 full page) ALSO routes through
  `page_shell` (active = none/`""`), so the nav is present on the 404 for
  navigational recovery — consistent with every other full page.

### D4 — Invariants preserved by construction

- **Read-only / no-key** (I-VIEW-1/3): the nav is plain `<a href>` links only — no
  form, button, or mutating control. `hx-boost` issues GETs only (AC-001.5).
- **Offline / no-CDN** (I-HX-2): `hx-boost`/`hx-select` are features of the ALREADY
  vendored `htmx.min.js` (slice-07); no new asset, no network.
- **Loopback** (I-VIEW-4): unchanged — still one `127.0.0.1` bind.
- **Progressive enhancement / no-JS** (I-HX-1/4): with JS off, `hx-boost` is inert;
  the nav's plain `<a href>` links do full-page navigation and the nav renders on
  every full page (AC-001.4).
- **No new crate / route / read-method**: pure-render + one adapter fork line;
  `xtask check-arch` stays 21 members.

## Consequences

**Positive**
- The two existing swaps (`#view-panel`, `#claims-table`) are byte-unchanged —
  they never set `HX-Boosted`, so they never hit the new fork arm.
- Byte-parity (AC-002.4) is structural, not asserted-by-luck: the boosted response
  IS the full page; `hx-select` cannot diverge from it.
- The nav's two levels compose cleanly: the persistent LEFT nav (surfaces, in the
  shell, outside `#viewer-main`) and the in-content My↔Peer TAB (inside
  `#viewer-main`, only on `/claims` + `/peer-claims`) never target the same region.
- One SSOT for surfaces (`LANDING_HUB_SURFACES`) now drives BOTH the (removed)
  landing hub and the persistent nav — less duplication than today.

**Negative / trade-offs**
- A boosted nav click transfers the WHOLE full page over the wire, then discards
  all but `#viewer-main`. Acceptable: this is a localhost, single-user viewer;
  payloads are small; it buys structural parity and a trivial adapter change.
- Every `render_*_page` must route its body through `page_shell` (a mechanical
  edit across the surface renderers) — but each is a one-line wrap, and the
  full-page unit tests pin the result.
- `#viewer-main` becomes a THIRD reserved swap id; documented alongside
  `VIEW_PANEL_ID` / `CLAIMS_TABLE_ID` so the three never collide.

## Migration — the landing hub (slice-17)

`render_landing` currently emits the inline `LANDING_HUB_SURFACES` hub, asserted by
slice-17 landing acceptance tests (e.g. `landing_hub_links_all_eight_surfaces_via_url_consts`).
This ADR SUPERSEDES that inline hub with the persistent left nav (same source).
Required migration, to be executed in DELIVER and pinned in DISTILL:

- Move the surface links out of `render_landing`'s body; the landing `content`
  region keeps only its store summary. The links now render via `render_viewer_nav`
  in `page_shell` — on the landing page AND every other surface.
- Re-baseline the slice-17 hub assertions: "all 8 surfaces linked via URL consts"
  becomes an assertion against the persistent nav, and it now holds on ALL 8 routes,
  not just `/`. This is a STRENGTHENING (broader coverage), not a coverage loss —
  the single-source (`LANDING_HUB_SURFACES`) test (AC-001.3) guards against drift.
- No slice-17 behavior is lost: every link the landing hub offered is still offered
  (now from everywhere); the landing's read-only + offline gold invariants still hold
  (the nav is plain links).

## Review resolutions (DESIGN review, 2026-07-05)

| Finding | Resolution |
|---|---|
| [BLOCKER] active marker can't update on a content-only swap (AC-002.1↔002.3 conflict) | **D5** — OOB `<ul id="viewer-nav-items" hx-swap-oob="innerHTML">` updates the active marker in place; nav container persists. AC-002.1 refined (back-propagated to `feature-delta.md`). |
| [SHOULD-FIX] slice-17 landing-hub removal breaks its tests | **Migration** section above — re-baseline the hub assertions to the persistent nav (strengthening, single-source-guarded). |
| [SHOULD-FIX] `render_*_page` → `page_shell` refactor shape undefined | **D6** — signatures unchanged; each `*_page` returns `page_shell(title, active, body)`; pseudocode given. `*_fragment` fns untouched. |
| [NIT] 404 (`render_error`) integration unspecified | **D6** — `render_error` also routes through `page_shell` (nav present for recovery). |
| [NIT] `active` provenance undefined | **D6** — `active` is the surface's compile-time URL const at each `*_page` call site (not request-read); base-path const for query-bearing routes. |

## Alternatives considered

1. **Explicit `hx-get` + `hx-target="#viewer-main"` on each nav link (no boost).**
   Rejected: an `HX-Request` GET returns a *fragment* under the existing `Shape`
   fork, which lacks `#viewer-main` for `hx-select`; and the fork cannot
   distinguish a left-nav request from a tab request (both send only `HX-Request`)
   without a bespoke custom header. `hx-boost` gives the distinction (`HX-Boosted`)
   AND the full-page response for free.
2. **Global `hx-boost` on `<body>`.** Rejected: it would also boost the My↔Peer tab
   anchors, colliding with their explicit `#view-panel` target. Scoping boost to
   the left `<nav>` keeps the tab's ADR-034 behavior intact.
3. **A per-surface "content fragment" shape (return the surface body, nav-excluded,
   for left-nav requests).** Rejected: it forks byte-parity into a second code path
   (fragment vs full-page content could drift), reintroducing exactly the I-HX-5
   risk slice-07 spent effort to make structural. Full-page + `hx-select` keeps ONE
   render path per surface.
4. **A client-side JS framework / SPA shell.** Rejected: violates offline-first,
   no-CDN, and the progressive-enhancement discipline; htmx already ships the
   capability.
5. **Render the nav but keep full-page reloads (drop US-NAV-002).** Rejected: the
   nav would "flash"/reset on every move — the DISCUSS "stays open" outcome (the
   whole point) would be unmet.
