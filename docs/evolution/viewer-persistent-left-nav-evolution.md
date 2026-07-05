# Evolution: viewer-persistent-left-nav (slice-21 — a persistent left `<nav>` on every read-only viewer page via an hx-boost content-region swap into an outer `<main id="viewer-main">`)

> Feature archive. Authored at finalize (DELIVER close). Source of truth for all
> detail remains the feature workspace `docs/feature/viewer-persistent-left-nav/`
> (a single-narrative `feature-delta.md` carrying the DISCUSS/DESIGN/DISTILL/DELIVER
> [REF] sections, plus `discuss/`, `design/`, `distill/`, `deliver/`) and ADR-058 under
> `docs/adrs/`; this file is the post-mortem summary. This slice is a **cross-cutting
> chrome DELTA on shipped work** — it wraps EVERY existing read-only viewer page in a
> shared **page shell** (an outer `<main id="viewer-main">` content region + a persistent
> left `<nav id="viewer-nav">`) WITHOUT changing any page's inner content, any route, any
> read method, or the workspace member count. It builds directly on slice-17's landing hub
> (`viewer-landing-dashboard`, the source of `LANDING_HUB_SURFACES`) — the nav is that same
> single surface list, now rendered as persistent chrome rather than a one-page dashboard,
> and slice-17's landing page is **migrated onto the shell** so the surface list stays
> single-sourced. Read the slice-17 archive
> (`docs/evolution/viewer-landing-dashboard-evolution.md`) for `LANDING_HUB_SURFACES` — the
> single surface SSOT this slice reads from exactly one renderer. This slice realizes
> **J-002** ("move between the viewer's surfaces without losing my place") in its persistent
> form: the nav is on every full page, the active surface is marked `aria-current="page"`,
> and boosted navigation swaps only the `#viewer-main` content region while the nav frame
> stays put.

## Summary

`viewer-persistent-left-nav` introduces a single pure **`page_shell(title, active, content)`**
that wraps every read-only viewer response in an outer **`<main id="viewer-main">`** content
region alongside a persistent left **`<nav id="viewer-nav">`** built from the slice-17
**`LANDING_HUB_SURFACES`** list. Nav links carry **`hx-boost`** + **`hx-select="#viewer-main"`**,
so with JS on a click fetches the target page and swaps ONLY its `#viewer-main` content region
into place — the nav frame is never re-fetched-and-replaced-visibly, the operator keeps their
place. The active surface is marked **`aria-current="page"`**. Because a boosted swap replaces
only `#viewer-main`, the active marker in the persistent nav would go stale; the DESIGN-review
blocker (**AC-002.1 ↔ AC-002.3**: "the nav must persist" vs "the active marker must track the
current surface") was resolved by **`render_viewer_nav_oob`** — an out-of-band
`<ul id="viewer-nav-items" hx-swap-oob="innerHTML">` fragment appended after `</main>` on
boosted responses that re-renders the nav item list with the new active marker, updating the
frame's marker WITHOUT re-swapping the frame. The adapter's **`Shape::from_request`** gains an
**`HX-Boosted → FullPage`** classification (a boosted request still renders the FULL page — the
client's `hx-select` extracts `#viewer-main` and the appended OOB updates the marker), and
`render_error` 404 now renders THROUGH the shell so error pages carry the nav too. slice-17's
landing hub is **migrated onto the persistent nav** so `LANDING_HUB_SURFACES` is read by exactly
ONE renderer (the shell), not two.

The load-bearing thesis: **a cross-cutting chrome wrapper that takes on authority over nothing —
the nav is plain `<a href>` GET-only links with no executable control, the shell is a pure
function over `(title, active, content)`, the boosted response IS the full page (progressive
enhancement: with JS off every link is an ordinary full-page navigation and the nav is on every
page anyway), and the surface list is single-sourced from `LANDING_HUB_SURFACES`.** The viewer
signs/writes/persists nothing and holds no signing key; the nav adds no route and no read method
(the workspace stays at **21 members**); and every existing inner page renders **byte-unchanged**
inside `#viewer-main` (the existing `#view-panel` tab swaps and `#claims-table` paging swaps NEVER
set `HX-Boosted`, so they are structurally untouched by the boosted-shape fork). The one
genuinely-new bit of machinery is the **OOB active-marker fragment** (`render_viewer_nav_oob`) —
the mechanism that lets a content-region-only swap keep the persistent nav's active marker honest.

The slice ships **ZERO new crates** (workspace stays at **21 members**), **ZERO new routes**, and
**ZERO new read methods**. It is **near-all-EXTEND — a chrome wrapper, not a re-architecture**: the
work is one pure `page_shell` + one pure `render_viewer_nav_oob` (viewer-domain), one
`Shape::from_request` `HX-Boosted → FullPage` classification + the boosted OOB append
(adapter-http-viewer), `render_error` 404 routed through the shell, and the slice-17 landing hub
migrated onto the shell. The lean-density decision from DISCUSS held: the nav is a flat surface
list with NO expansion trigger, NO collapse state, NO client-side nav JS beyond vendored htmx.

### What shipped (one paragraph)

Every read-only viewer response now renders through a single pure **`page_shell(title, active,
content)`** (viewer-domain): an outer **`<main id="viewer-main">`** wrapping the page's existing
inner content, preceded by a persistent left **`<nav id="viewer-nav">`** whose items come from the
slice-17 **`LANDING_HUB_SURFACES`** list, with the current surface marked **`aria-current="page"`**.
Nav links carry **`hx-boost`** + **`hx-select="#viewer-main"`**: with JS on, a click fetches the
target full page and the client extracts and swaps only its `#viewer-main` region, so the nav frame
stays visually put and the operator keeps their place; with JS off, each link is an ordinary
`<a href>` full-page GET navigation and the nav is present on every full page regardless. The
adapter's **`Shape::from_request`** gains an **`HX-Boosted → FullPage`** arm (a boosted request
renders the SAME full page as a cold load — the client's `hx-select` does the extraction), and on a
boosted response the handler appends **`render_viewer_nav_oob`** — an out-of-band
`<ul id="viewer-nav-items" hx-swap-oob="innerHTML">` fragment after `</main>` that re-renders the nav
item list with the new active marker, keeping the persistent nav's `aria-current` honest across a
content-region-only swap (the resolution of the DESIGN-review AC-002.1 ↔ AC-002.3 active-marker
blocker). `render_error` 404 now renders through the shell so error pages carry the nav. slice-17's
landing hub is migrated onto the shell so `LANDING_HUB_SURFACES` is read by exactly ONE renderer.
The existing `#view-panel` tab swaps and `#claims-table` paging swaps are **byte-unchanged** (they
never set `HX-Boosted`, so the boosted-shape fork never touches them). No new crate, no new route,
no new read method; the bind stays loopback-only; nothing is persisted; the viewer holds no key.

### Wave timeline

| Wave    | Date       | Owner                                                     |
|---------|------------|----------------------------------------------------------|
| DISCUSS | 2026-07-04 | Luna (nw-product-owner)                                  |
| DESIGN  | 2026-07-04 | Morgan (nw-solution-architect)                          |
| DISTILL | 2026-07-04 | Quinn (nw-acceptance-designer)                          |
| DELIVER | 2026-07-04 | Crafter (nw-functional-software-crafter) + orchestration |

### Shipping metrics

- **5 roadmap steps** done across **3 phases** (all COMMIT/PASS — or APPROVED_SKIP with
  rationale — in `deliver/execution-log.json`). Roadmap ratio 0.36, APPROVED.
- **Acceptance scenarios GREEN**: the `viewer_persistent_left_nav` corpus (NAV-1..NAV-9 — including
  the **thick walking skeleton** at NAV-1: a viewer page rendered through the shell with the
  persistent nav + the active marker + the boosted content-region swap) + the GOLD invariants
  (`viewer_persistent_left_nav_invariants` — the **6 GOLD invariants** NAV-INV-*). The `ViewerServer`
  harness drives the REAL `openlore ui` over HTTP; the DISTILL wave added the **`get_boosted()`**
  harness helper (a request carrying `HX-Boosted: true`) to drive the boosted shape end-to-end.
- **Slices 08/15/16/17 corpora GREEN — zero regression**: every existing inner page renders
  byte-unchanged inside `#viewer-main`; the `#view-panel` tab swaps and `#claims-table` paging swaps
  are byte-stable (they never set `HX-Boosted`). The full workspace acceptance suite is green across
  all slices.
- **NO new crate, NO new route, NO new read method (near-all-EXTEND)**: extends
  `crates/viewer-domain` (PURE — the new `page_shell(title, active, content)` + `render_viewer_nav_oob`
  + the `aria-current="page"` active marker; slice-17 landing migrated onto the shell) and
  `crates/adapter-http-viewer` (EFFECT — `Shape::from_request` gains the `HX-Boosted → FullPage`
  classification via the single-sited `request_is_boosted` predicate + the boosted OOB append; every
  handler renders through the shell; `render_error` 404 through the shell). REUSES the slice-17
  `LANDING_HUB_SURFACES` surface list (now read by exactly one renderer) and the vendored htmx. The
  workspace member count stays **21**; `cargo xtask check-arch` reports "21 workspace members".
- **Single-source preserved**: `LANDING_HUB_SURFACES` is read by exactly ONE renderer (the shell) —
  the slice-17 landing page was migrated onto the shell so the surface list is not duplicated across a
  dashboard renderer and a nav renderer.
- **Progressive enhancement BY CONSTRUCTION**: nav links are plain `<a href>` GET-only; `hx-boost` is
  inert with JS off (ordinary full-page navigation), and the nav is present on every full page
  regardless of JS.
- **Mutation**: the pure `viewer-domain` core is **100% (7/7)**; the one genuine adapter survivor (the
  `hx-swap-oob` boosted-append guard) was killed with an in-crate test (45d7e08); the 13 remaining
  "missed" are cross-crate-coverage artifacts (killed by the acceptance layer). The per-feature gate
  (≥80% of viable) is MET.
- **1 ADR** (ADR-058) Accepted/shipped.
- DES integrity: **5/5** steps have complete DES traces.
- Adversarial review (Phase 4): **APPROVED**, **0 defects, 0 Testing Theater** (7 patterns clean).
- Gates: **DoR 9/9**, DESIGN **BLOCKED → APPROVED** (the active-marker AC-002.1 ↔ AC-002.3 blocker
  resolved via the D5 OOB fragment), DISTILL verified compile-green + RED, Phase-3 refactor **one L1/L4
  applied** (single-sited the HX-Boosted detection via `request_is_boosted`, 82f9dfc), review
  **APPROVED**, mutation **pure-core 100% (7/7)** + the one genuine adapter survivor killed, integrity
  **5/5**, `check-arch` **OK (21)**.

## Wave-by-wave changelog

### DISCUSS (2026-07-04)

Luna framed the slice under **J-002** ("move between the viewer's surfaces without losing my place"),
adding the **J-002d aspect** (a persistent frame, not a one-page hub) — the operator wants the surface
list ALWAYS present, not only on the landing page, and wants to move between surfaces without a jarring
full-page reload losing scroll/place. Two stories: **US-NAV-001** (a persistent left nav on every
viewer page) + **US-NAV-002** (navigate between surfaces via the nav keeping the operator's place). The
load-bearing DISCUSS decision was the **lean-density** framing: the nav is a FLAT surface list with NO
expansion trigger, NO collapsible sections, NO client-side nav state machine — just the slice-17
surface list rendered as persistent chrome. The CARDINAL framing carried forward from slice-08/15/16/17:
read-only/no-key (the nav is plain `<a href>` GET-only, no executable control), LOCAL/offline
(loopback-only, vendored htmx only), progressive enhancement (the nav works with JS off; `hx-boost` is
an enhancement, not a requirement), single-source (the surface list stays single-sourced from
`LANDING_HUB_SURFACES`), and no-regression (every existing inner page byte-unchanged). **KPI-NAV-1..5**
were framed. DoR PASS (**9/9**).

### DESIGN (2026-07-04)

Morgan formalized the slice as **ADR-058** — a persistent viewer left nav via an hx-boost
content-region swap into an outer `<main id="viewer-main">`, with an `HX-Boosted` full-page shape fork:

- **ADR-058**: a single pure **`page_shell(title, active, content)`** (viewer-domain) wraps every
  read-only viewer response — an outer `<main id="viewer-main">` content region + a persistent left
  `<nav id="viewer-nav">` built from `LANDING_HUB_SURFACES`, active surface marked
  `aria-current="page"`. Nav links carry `hx-boost` + `hx-select="#viewer-main"` so a boosted click
  swaps only the content region. The adapter's `Shape::from_request` gains an **`HX-Boosted → FullPage`**
  arm (a boosted request renders the SAME full page; the client's `hx-select` extracts `#viewer-main`).
  `render_error` 404 renders through the shell. slice-17's landing hub is migrated onto the shell so
  `LANDING_HUB_SURFACES` is read by exactly one renderer (no new crate, no new route, no new read
  method; workspace stays 21). Alternatives rejected: a client-side SPA nav (keyless-viewer-breaking,
  JS-required — violates progressive enhancement); a per-page duplicated nav renderer (would break the
  single-source cardinal); an iframe/frameset frame (breaks deep-linking + accessibility).

- **DESIGN review BLOCKED → APPROVED**: the reviewer raised the **active-marker blocker** (AC-002.1 ↔
  AC-002.3): AC-002.1 requires the nav to PERSIST across navigation (so a boosted swap must NOT re-fetch
  and replace the nav frame), while AC-002.3 requires the active marker to TRACK the current surface —
  but a content-region-only swap that replaces just `#viewer-main` leaves the persistent nav's
  `aria-current` marker STALE. Resolved via **decision D5**: **`render_viewer_nav_oob`** — an
  out-of-band `<ul id="viewer-nav-items" hx-swap-oob="innerHTML">` fragment appended after `</main>` on
  boosted responses, re-rendering the nav item list with the new active marker. This updates the marker
  WITHOUT re-swapping the frame, satisfying both AC-002.1 (frame persists) and AC-002.3 (marker tracks).
  With D5 in place DESIGN closed **APPROVED**.

The C4 Context/Container/Component diagrams (the shell as the outermost render layer wrapping every
handler; the boosted-shape fork in `Shape::from_request`; the OOB append seam) are in the DESIGN
sections of `feature-delta.md` and `design/`.

### DISTILL (2026-07-04)

Quinn authored the executable acceptance corpus across two `[[test]]` targets, plus a harness helper:

- **`viewer_persistent_left_nav.rs`** (Tier A — `NAV-` ids NAV-1..NAV-9): the **thick walking
  skeleton** (NAV-1 — a viewer page rendered through the shell with the persistent `<nav id="viewer-nav">`
  + the outer `<main id="viewer-main">` + the `aria-current="page"` active marker + a boosted click
  swapping only `#viewer-main`), the **nav-on-every-page** assertions (the nav present on each full
  page), the **active-marker tracking** assertions (the marker follows the current surface, including
  across a boosted swap via the OOB fragment), the **boosted content-region swap** (the boosted response
  IS the full page; the client extracts `#viewer-main`; the OOB appended after `</main>`), the
  **no-regression** assertions (every inner page byte-unchanged; the `#view-panel` tab + `#claims-table`
  paging swaps byte-stable, never set `HX-Boosted`), and the **error-through-shell** (a 404 carries the
  nav).
- **`viewer_persistent_left_nav_invariants.rs`** (gold guardrails — the **6 GOLD invariants**): **NAV-INV-NoControl**
  (read-only / no key — the nav is plain `<a href>` GET-only, no executable control on any shape),
  **NAV-INV-Offline** (LOCAL / offline — loopback-only; vendored htmx only), **NAV-INV-NoJs**
  (progressive enhancement — `hx-boost` inert with JS off; every link an ordinary full-page navigation;
  the nav on every full page), **NAV-INV-SingleSource** (`LANDING_HUB_SURFACES` read by exactly one
  renderer), **NAV-INV-LandingViaNav** (the slice-17 landing reachable via the persistent nav), and
  **NAV-INV-LandingNoRegression** (the slice-17 landing page byte-stable after migration onto the shell).

The driving port is the REAL `openlore ui` subprocess over HTTP (`ViewerServer`); DISTILL added the
**`ViewerServer::get_boosted()`** harness helper (a GET carrying `HX-Boosted: true`) to drive the
boosted shape end-to-end. DISTILL verified the corpus **compile-green + RED** (15 RED scenarios: 9 main
NAV-* + 6 gold NAV-INV-*). The Reconciliation HARD GATE passed. The DoR closed **9/9**.

### DELIVER (2026-07-04)

Executed **5 roadmap steps across 3 phases** via DES-monitored crafter dispatches, each commit carrying
a `Step-ID: NN-NN` trailer. Per-step SHAs are in `deliver/execution-log.json`.

- **Phase 01 — walking skeleton + landing migration + single-source (01-xx)**: **01-01 is the THICK
  walking skeleton** (9ccdc6c) — the pure `page_shell(title, active, content)` + the outer
  `<main id="viewer-main">` + the persistent `<nav id="viewer-nav">` from `LANDING_HUB_SURFACES` + the
  `aria-current="page"` active marker + the `HX-Boosted → FullPage` classification + the hx-boost /
  hx-select links. **01-02 (ae74ff6)** migrated the slice-17 landing hub onto the persistent nav and
  single-sourced `LANDING_HUB_SURFACES` (read by exactly one renderer).
- **Phase 02 — OOB active-marker + byte-parity/no-regression (02-xx)**: **02-01 (2d0e9cb)** the **OOB
  active-marker fragment** `render_viewer_nav_oob` (`<ul id="viewer-nav-items" hx-swap-oob="innerHTML">`
  appended after `</main>` on boosted responses — the D5 resolution of the DESIGN-review AC-002.1 ↔
  AC-002.3 blocker) — **the real implementation work of the slice**; **02-02 (3f22cb6)** the
  **byte-parity / no-regression confirm** (every inner page byte-unchanged inside `#viewer-main`; the
  `#view-panel` tab + `#claims-table` paging swaps byte-stable, never set `HX-Boosted`).
- **Phase 03 — gold (03-xx)**: **03-01 (74ee4cd)** the **6 GOLD invariants**. They flipped GREEN off the
  confirmatory shell + boosted render path.

The 5-step shape: a **thorough walking skeleton at 01-01** shipped the shell + the boosted-shape fork on
day one, so the nav-on-every-page / byte-parity / progressive-enhancement scenarios became confirmatory
off the structure; the real implementation work was the **OOB active-marker fragment (02-01)** — the one
piece of genuinely-new machinery that lets a content-region-only swap keep the persistent nav's marker
honest.

**Phase-3 refactor — one L1/L4 applied**: the `HX-Boosted` detection was **single-sited** via a
`request_is_boosted` predicate (82f9dfc), so the boosted-shape classification and the boosted OOB-append
decision read the same one predicate rather than duplicating the header check at two sites.

**Phase-4 adversarial review — APPROVED**: **0 defects, 0 Testing Theater, 7 patterns clean**. The
persistent nav confirmed read-only render-only (plain `<a href>` GET-only, no executable control); the
boosted content-region swap + the OOB active-marker fragment confirmed load-bearing and honest; the
single-source + progressive-enhancement + no-regression confirmed structural.

**Phase-5 mutation**: the pure `viewer-domain` core scored **100% (7/7)**; the one genuine adapter
survivor (the `hx-swap-oob` boosted-append guard) was killed with an in-crate test (45d7e08); the 13
remaining "missed" are cross-crate-coverage artifacts (killed by the acceptance layer). The per-feature
gate (≥80% of viable) is MET.

**Phase-6 integrity**: all 5 steps have complete DES traces; `cargo xtask check-arch` OK (21 workspace
members).

## DELIVER-wave decisions

| # | Decision | Why it mattered |
|---|----------|-----------------|
| DV-NAV-1 | DES `project_id` header carried in `execution-log.json` (same hook-defect workaround as slice-02..20 DV-1). | Stop-hook reads `project_id`; `des-init-log` writes `feature_id`. Unblocked every step's stop-hook without touching the append-only event trail. |
| DV-NAV-2 | Mutation = per-feature: the pure `viewer-domain` core 100% (7/7); the one genuine adapter survivor (the `hx-swap-oob` boosted-append guard) killed with an in-crate test (45d7e08); the 13 remaining "missed" = cross-crate-coverage artifacts killed by the acceptance layer. | Per-feature gate at deliver-time + DEVOPS sweep backstop. The pure shell/nav render logic is the killing surface and it is 100% in-crate; the boosted-append guard needed a dedicated adapter test to reach a genuinely-viable adapter mutant; the 13 cross-crate artifacts are killed through the real binary. ≥80%-of-viable gate MET. |
| DV-NAV-3 | **One pure `page_shell(title, active, content)` wraps EVERY read-only viewer response — NO new route, NO new crate, NO new read method** (ADR-058). | A cross-cutting chrome wrapper belongs in exactly one pure function so every handler renders through the same shell; adding a route/crate/read for chrome would be over-engineering. The nav needs no data beyond the already-loaded `LANDING_HUB_SURFACES` list. Workspace stays 21. |
| DV-NAV-4 | **Persistent nav from `LANDING_HUB_SURFACES` — read by exactly ONE renderer (the shell); slice-17 landing migrated onto the shell** (ADR-058). | The surface list is the single source of truth; rendering it from both a landing-dashboard renderer AND a nav renderer would double-source it. Migrating the slice-17 landing onto the shell keeps `LANDING_HUB_SURFACES` single-sourced (NAV-INV-SingleSource) and the landing byte-stable (NAV-INV-LandingNoRegression). |
| DV-NAV-5 | **The active-marker AC-002.1 ↔ AC-002.3 blocker resolved via `render_viewer_nav_oob` — an OOB `<ul id="viewer-nav-items" hx-swap-oob="innerHTML">` fragment appended after `</main>` on boosted responses** (ADR-058 D5). | A boosted swap replaces ONLY `#viewer-main`, so the persistent nav's `aria-current` marker would go stale (AC-002.3 fails) unless the frame is re-swapped (AC-002.1 fails). The OOB fragment updates just the nav item list's active marker WITHOUT re-swapping the frame — satisfying both: the frame persists AND the marker tracks. This was the DESIGN-review blocker and the real implementation work of the slice. |
| DV-NAV-6 | **`Shape::from_request` gains an `HX-Boosted → FullPage` classification — a boosted request renders the SAME full page; the client's `hx-select="#viewer-main"` extracts the content region** (ADR-058). | A boosted request must NOT render a bare fragment (that would strip the nav and the shell); it renders the full page and the client extracts `#viewer-main` + applies the appended OOB. Making the boosted shape a FullPage fork (not a fragment) is what makes the boosted response byte-identical to a cold load below the extraction point — the byte-parity cardinal. |
| DV-NAV-7 | **The nav is plain `<a href>` GET-only links — no executable control on any shape** (ADR-058). | The viewer is read-only and holds no key; the nav is navigation chrome, not a mutation surface. Plain anchors keep progressive enhancement (they navigate with JS off) and keep the read-only/no-key cardinal (no POST, no command affordance in the nav). |
| DV-NAV-8 | **`render_error` 404 renders THROUGH the shell** (ADR-058). | An error page is still a viewer page; rendering it through the shell means the operator keeps the persistent nav even on a 404, so navigation is never a dead end. Consistency of chrome across success and error responses. |
| DV-NAV-9 | **The existing `#view-panel` tab swaps and `#claims-table` paging swaps NEVER set `HX-Boosted` — so the boosted-shape fork never touches them; they stay byte-unchanged** (ADR-058). | The pre-existing htmx swaps are content-region swaps of a DIFFERENT kind (tab/paging, not nav); the boosted-shape fork keys strictly on `HX-Boosted`, which those swaps never send. This makes the no-regression guarantee structural: the fork is unreachable for the existing swaps by construction. |
| DV-NAV-10 | **Phase-3 refactor: one L1/L4 — single-sited the `HX-Boosted` detection via `request_is_boosted`** (82f9dfc). | The boosted-shape classification (`Shape::from_request`) and the boosted OOB-append decision both need to know "is this request boosted?"; reading the header at two sites would risk the two decisions diverging. A single `request_is_boosted` predicate makes the two decisions read one source — the classification and the OOB append can never disagree. |
| DV-NAV-11 | **Lean density held: a FLAT surface list, NO expansion trigger, NO collapse state, NO client-side nav JS beyond vendored htmx** (DISCUSS lean-density decision). | The DISCUSS decision was to avoid a nav state machine; a flat always-visible list keeps progressive enhancement trivial (no JS-required expand/collapse) and keeps the nav a pure render over the surface list. No collapse state means no client-side state to persist or restore. |

## Cardinal release gates + slice-21 invariants (I-NAV-1..n)

The cardinal release gates realized on the persistent left-nav chrome surface — all release-blocking:

1. **Read-only / no key (CARDINAL, I-NAV-1)** — the nav is plain `<a href>` GET-only links with no
   executable control; the viewer holds no key. Three-layer: TYPE (no write method; no new read method)
   + STRUCTURAL (`xtask check-arch` viewer capability rule) + BEHAVIORAL (NAV-INV-NoControl gold).
2. **Progressive enhancement (CARDINAL, I-NAV-2)** — `hx-boost` is inert with JS off (every link is an
   ordinary full-page GET navigation), and the persistent nav is present on every full page regardless
   of JS. STRUCTURAL (plain anchors + hx-boost as enhancement) + BEHAVIORAL (NAV-INV-NoJs gold + the
   nav-on-every-page scenarios).
3. **Byte-parity structural (CARDINAL, I-NAV-3)** — the boosted response IS the full page; the client's
   `hx-select="#viewer-main"` extracts the content region; the OOB nav-items fragment is appended after
   `</main>`. The boosted response is byte-identical to a cold load below the extraction point.
   STRUCTURAL (the `HX-Boosted → FullPage` fork, DV-NAV-6) + BEHAVIORAL (the boosted-swap scenarios).
4. **Single-source (CARDINAL, I-NAV-4)** — `LANDING_HUB_SURFACES` is read by exactly ONE renderer (the
   shell); the slice-17 landing hub was migrated onto the shell so the surface list is not duplicated.
   STRUCTURAL (one renderer, DV-NAV-4) + BEHAVIORAL (NAV-INV-SingleSource + NAV-INV-LandingViaNav gold).
5. **LOCAL / offline (CARDINAL, I-NAV-5)** — the nav renders from the LOCAL surface list; vendored htmx
   only; loopback-only bind preserved. STRUCTURAL (no network fetch for the nav) + BEHAVIORAL
   (NAV-INV-Offline gold).
6. **Additive / no-regression (CARDINAL, I-NAV-6)** — every existing inner page renders byte-unchanged
   inside `#viewer-main`; the `#view-panel` tab swaps and `#claims-table` paging swaps are byte-stable
   (they never set `HX-Boosted`); the slice-17 landing is byte-stable after migration. STRUCTURAL (the
   inner content unchanged; the fork keys on `HX-Boosted` which the existing swaps never send, DV-NAV-9)
   + BEHAVIORAL (NAV-INV-LandingNoRegression + the byte-parity scenarios).
7. **Active-marker honesty (I-NAV-7)** — the persistent nav's `aria-current="page"` marker tracks the
   current surface even across a content-region-only boosted swap, via the `render_viewer_nav_oob` OOB
   fragment (the D5 resolution of the AC-002.1 ↔ AC-002.3 blocker). STRUCTURAL (the OOB nav-items append,
   DV-NAV-5) + BEHAVIORAL (the active-marker-tracking scenarios).
8. **No new route / crate / read method (I-NAV-8)** — the shell adds no route, no crate, no read method;
   the workspace stays 21. STRUCTURAL (`xtask check-arch` reports 21; DV-NAV-3).

| # | Invariant | Enforcement |
|---|---|---|
| I-NAV-1 | Read-only / no key (the nav is plain `<a href>` GET-only; no executable control; no key in the process). | TYPE (no write method; no new read method) + STRUCTURAL (`xtask check-arch` viewer capability rule) + BEHAVIORAL (NAV-INV-NoControl, DV-NAV-7). Cardinal. |
| I-NAV-2 | Progressive enhancement (`hx-boost` inert with JS off; every link an ordinary full-page navigation; the nav on every full page). | STRUCTURAL (plain anchors + hx-boost as enhancement, DV-NAV-7/11) + BEHAVIORAL (NAV-INV-NoJs). Cardinal. |
| I-NAV-3 | Byte-parity structural (the boosted response IS the full page; `hx-select` extracts `#viewer-main`; the OOB nav-items appended after `</main>`). | STRUCTURAL (the `HX-Boosted → FullPage` fork, DV-NAV-6) + BEHAVIORAL (the boosted-swap scenarios). Cardinal. |
| I-NAV-4 | Single-source (`LANDING_HUB_SURFACES` read by exactly one renderer; the slice-17 landing migrated onto the shell). | STRUCTURAL (one renderer, DV-NAV-4) + BEHAVIORAL (NAV-INV-SingleSource + NAV-INV-LandingViaNav). Cardinal. |
| I-NAV-5 | LOCAL / offline (the nav renders from the LOCAL surface list; vendored htmx only; loopback-only). | STRUCTURAL (no network fetch for the nav) + BEHAVIORAL (NAV-INV-Offline). Cardinal. |
| I-NAV-6 | Additive / no-regression (every inner page byte-unchanged inside `#viewer-main`; the `#view-panel` tab + `#claims-table` paging swaps byte-stable; the slice-17 landing byte-stable). | STRUCTURAL (the inner content unchanged; the fork keys on `HX-Boosted` which the existing swaps never send, DV-NAV-9) + BEHAVIORAL (NAV-INV-LandingNoRegression + the byte-parity scenarios). Cardinal. |
| I-NAV-7 | Active-marker honesty (`aria-current="page"` tracks the current surface across a content-region-only boosted swap, via the OOB fragment). | STRUCTURAL (the `render_viewer_nav_oob` OOB nav-items append, DV-NAV-5) + BEHAVIORAL (the active-marker-tracking scenarios). |
| I-NAV-8 | No new route / crate / read method (the workspace stays 21). | STRUCTURAL (`xtask check-arch` reports 21; DV-NAV-3). |

All slice-21 invariants INHERIT the slice-08 I-NS-1..9 + slice-16 I-SF-1..7 + slice-17 landing-hub
invariant sets (read-only / no key / offline + loopback / progressive enhancement / structural
fragment/page parity / anti-merging / verified-by-construction / public-data framing); the persistent
nav is cross-cutting chrome that touches none of them — it wraps the existing pages without changing
their inner content.

## Quality gates — final report

- **Acceptance / integration**: the `viewer_persistent_left_nav` corpus (NAV-1..NAV-9, the thick
  walking skeleton at NAV-1) + the GOLD `viewer_persistent_left_nav_invariants` (the 6 GOLD invariants)
  GREEN; slices 08/15/16/17 corpora GREEN — zero regression. The `ViewerServer` harness drives the REAL
  `openlore ui` over HTTP; DISTILL added `ViewerServer::get_boosted()` (a request carrying
  `HX-Boosted: true`) to drive the boosted shape end-to-end.
- **`cargo xtask check-arch`**: **OK (21 workspace members)** — no new crate, no new route, no new read
  method. The viewer capability rule is unchanged (read-only; no signing/identity/PDS, no store-write).
- **Refactor (L1-L4)**: clippy + check-arch clean; **Phase-3 refactor one L1/L4 applied** — the
  `HX-Boosted` detection single-sited via `request_is_boosted` (82f9dfc), so the boosted-shape
  classification and the boosted OOB-append decision read one predicate. `viewer-domain` purity intact
  (no I/O imports; maud + ports only; `page_shell` and `render_viewer_nav_oob` are pure over
  `(title, active, content)` / the surface list; the boosted-shape decision lives in the effect shell).
- **Adversarial review (Phase 4)**: **APPROVED**, **0 defects, 0 Testing Theater, 7 patterns clean**.
  The persistent nav confirmed read-only render-only (plain `<a href>` GET-only, no executable control,
  DV-NAV-7); the boosted content-region swap + the OOB active-marker fragment confirmed load-bearing and
  honest (DV-NAV-5/6); the single-source + progressive-enhancement + no-regression confirmed structural
  (DV-NAV-4/9/11).
- **DES integrity**: PASS — all 5 steps have complete DES traces (**5/5**).

## Mutation testing — final report

**Scope**: the new pure `viewer-domain` production functions (`page_shell(title, active, content)` +
`render_viewer_nav_oob` + the `aria-current="page"` active-marker logic) + the `adapter-http-viewer`
effect-shell boosted-shape fork (the `HX-Boosted → FullPage` classification via `request_is_boosted` +
the boosted OOB append). The slice-16/17 cross-package lesson stays applied — the `viewer-domain` tests
pin the pure functions IN-crate, and the one genuinely-viable adapter mutant (the `hx-swap-oob`
boosted-append guard) was pinned by a dedicated in-crate `adapter-http-viewer` unit test.

| Mutant category | Viable | Caught | Missed | Kill rate |
|---|---:|---:|---:|---|
| `viewer-domain` pure core (the shell + the OOB nav-items + the active-marker logic) | 7 | 7 | 0 | **100% (7/7)** |
| `adapter-http-viewer` boosted-append guard (the `hx-swap-oob` OOB append) | 1 | 1 | 0 | killed by the in-crate unit test (45d7e08) |
| cross-crate-coverage artifacts (killed by the acceptance layer) | — | — | 13 | cross-crate artifact (killed through the real binary) |

**Mutation note (precise)**: the pure `viewer-domain` core is **100% (7/7)** — every pure shell / nav /
active-marker mutant is killed in-crate. One genuinely-viable adapter survivor (the `hx-swap-oob`
boosted-append guard — the branch that decides whether to append the OOB nav-items fragment on a boosted
response) was killed with a dedicated in-crate `adapter-http-viewer` unit test (45d7e08). The **13
remaining "missed"** are **cross-crate-coverage artifacts** — adapter mutants whose killing coverage
lives in the acceptance layer (the real-binary `ViewerServer` corpus), not in-crate; they are killed
through the real binary, the slice-16/17 package-scoped-harness precedent. The slice-21 per-feature gate
(≥80% of viable) is **MET** (pure core 100%; the one genuine adapter survivor killed). `adapter-http-viewer`
is otherwise not mutation-swept by design (effect shell; covered by the GOLD invariants + the dedicated
unit test through the real binary). DEVOPS sweep is the ongoing backstop.

## Lessons learned / issues

- **A content-region-only swap breaks the persistent frame's active marker unless the frame's marker is
  updated out-of-band (DV-NAV-5)**: the DESIGN-review blocker (AC-002.1 "the nav must persist" ↔ AC-002.3
  "the active marker must track") is the core tension of ANY persistent-chrome-plus-content-swap design.
  Persisting the frame means NOT re-swapping it; tracking the marker means the frame's marker MUST change.
  The OOB fragment (`render_viewer_nav_oob`) resolves it: swap only the content region, and update ONLY
  the frame's marker out-of-band. **Lesson: when a persistent frame surrounds a swapped content region,
  any per-navigation state IN the frame (active marker, breadcrumb, title) must be updated via an OOB
  fragment — the frame stays put, its stateful bits get surgically re-rendered. Design the OOB seam at
  DESIGN time; it is not an afterthought, it is what makes "persistent frame + content swap" honest.**
- **A boosted request must render the FULL page, not a fragment — the byte-parity cardinal depends on it
  (DV-NAV-6)**: making `HX-Boosted → FullPage` (not a bare fragment) is what keeps the boosted response
  byte-identical to a cold load below the extraction point; the client's `hx-select` does the extraction.
  A fragment-shape boosted response would strip the shell and diverge from the cold-load HTML. **Lesson:
  with hx-boost + hx-select, render the full page server-side and let the client extract — the server
  should not try to pre-extract the content region, or the boosted and cold paths diverge and byte-parity
  is lost.**
- **A thorough walking skeleton drove most of the thread green for free (DV-NAV-3)**: the 01-01 WS shipped
  the pure shell + the boosted-shape fork on day one, so nav-on-every-page, byte-parity, and
  progressive-enhancement became confirmatory off the structure. The real work was the ONE genuinely-new
  bit — the OOB active-marker fragment (02-01). **Lesson: when a slice's cardinals are a
  render-shape decision (a shell wrapping every page), get the shell right inside the walking skeleton and
  most downstream scenarios become confirmation of the structure — the real work is the one seam the
  DESIGN review surfaced (here, the OOB marker).**
- **Single-sourcing chrome data means migrating the prior single-page consumer onto the new frame
  (DV-NAV-4)**: `LANDING_HUB_SURFACES` was slice-17's landing-hub list; leaving the landing renderer AND
  adding a nav renderer would double-source it. Migrating the landing ONTO the shell keeps the list
  single-sourced. **Lesson: when chrome (a nav) reuses a data list a prior single-page feature owned,
  migrate that page onto the chrome rather than reading the list twice — single-source is preserved by
  consolidation, not by copying.**
- **Keying the boosted-shape fork strictly on `HX-Boosted` makes no-regression structural (DV-NAV-9)**:
  the pre-existing `#view-panel` tab swaps and `#claims-table` paging swaps never send `HX-Boosted`, so
  the boosted-shape fork is UNREACHABLE for them — the no-regression guarantee is by construction, not by
  test enumeration. **Lesson: when adding a new response-shape fork alongside existing swaps, key the fork
  on a header the existing swaps provably never send — then their non-regression is structural, not just
  observed.**
- **Single-siting the header detection prevents two decisions from disagreeing (DV-NAV-10)**: the
  boosted-shape classification and the boosted OOB-append decision both ask "is this boosted?"; reading
  the header at two sites risks divergence (classify-as-boosted but forget-the-OOB, or vice versa).
  `request_is_boosted` makes both read one predicate. **Lesson: when two decisions both branch on the same
  request property, extract the property test to one predicate so the decisions can never disagree — a
  cheap L1/L4 refactor that removes a whole class of latent bug.**

## Deviations: planned (DESIGN) vs shipped

| # | Planned at DESIGN | Shipped state | Disposition |
|---|-------------------|---------------|-------------|
| 1 | ADR-058 fixed the contracts; field-level shaping (the `page_shell` signature, the OOB fragment id, the `request_is_boosted` predicate) left to DELIVER. | All adopted; `page_shell(title, active, content)` + `render_viewer_nav_oob` (`<ul id="viewer-nav-items" hx-swap-oob="innerHTML">`) + the `request_is_boosted` predicate materialized at DELIVER against the shell + boosted render tests. | Resolved at DELIVER; no contract deviation. |
| 2 | ADR-058 fixed "one pure `page_shell` wraps every response; no new crate/route/read method; workspace stays 21." | Shipped exactly — one `page_shell`; the boosted-shape fork in `Shape::from_request`; no new crate/route/read method; workspace stays 21. | Resolved at DELIVER. |
| 3 | DESIGN review raised the active-marker AC-002.1 ↔ AC-002.3 blocker (persist the frame vs track the marker). | Resolved via D5 — `render_viewer_nav_oob` OOB nav-items fragment appended after `</main>` on boosted responses; DESIGN closed APPROVED; materialized at 02-01. | Escalated at DESIGN (BLOCKED → APPROVED); resolved at DELIVER (DV-NAV-5). |
| 4 | ADR-058: `HX-Boosted → FullPage` classification; the client's `hx-select` extracts `#viewer-main`. | Shipped exactly — a boosted request renders the full page; the client extracts `#viewer-main`; the OOB nav-items appended after `</main>`. | Resolved at DELIVER (DV-NAV-6). |
| 5 | ADR-058: slice-17 landing migrated onto the shell for single-source. | Shipped — the landing hub migrated onto the persistent nav; `LANDING_HUB_SURFACES` read by exactly one renderer; the landing byte-stable. | Resolved at DELIVER (DV-NAV-4, NAV-INV-SingleSource / NAV-INV-LandingNoRegression). |
| 6 | ADR-058: `render_error` 404 through the shell. | Shipped — a 404 renders through the shell and carries the persistent nav. | Resolved at DELIVER (DV-NAV-8). |
| 7 | Phase-3 refactor expected to be evaluated. | One L1/L4 applied — the `HX-Boosted` detection single-sited via `request_is_boosted` (82f9dfc). | Confirmed at DELIVER (refactor applied, DV-NAV-10). |
| 8 | Mutation expected 100% on the pure core. | Pure `viewer-domain` core 100% (7/7); the one genuine adapter survivor (the `hx-swap-oob` boosted-append guard) killed in-crate (45d7e08); the 13 remaining = cross-crate-coverage artifacts. ≥80%-of-viable gate MET. | Recorded; the artifact explained (DV-NAV-2). |
| 9 | Review expected to pass clean. | Review APPROVED, 0 defects, 0 Testing Theater, 7 patterns clean (the persistent nav read-only, the boosted swap + OOB honest, single-source + progressive-enhancement + no-regression structural). | Confirmed at DELIVER. |

## KPI status

- **J-002 / J-002d** ("move between the viewer's surfaces without losing my place" — the persistent-frame
  aspect): realized — the nav is on every full page; boosted navigation swaps only `#viewer-main` so the
  operator keeps their place; the active marker tracks the current surface across boosted swaps.
- **US-NAV-001** (a persistent left nav on every viewer page): SHIPPED — the shell wraps every read-only
  response, including the 404 error page.
- **US-NAV-002** (navigate between surfaces via the nav keeping the operator's place): SHIPPED — the
  hx-boost + hx-select content-region swap + the OOB active-marker fragment.
- **KPI-NAV-1..5**: instrumented as the framed guardrails (nav-on-every-page; active-marker honesty;
  progressive-enhancement / no-JS parity; single-source; no-regression) — all met by the GOLD invariants
  (NAV-INV-*). The read-only viewer emits no telemetry; these KPIs are verified structurally + by the
  acceptance corpus, not by runtime metrics.

## Pointers

- **Feature workspace** (DISCUSS through DELIVER, all detail — PRESERVED):
  `docs/feature/viewer-persistent-left-nav/` — the single-narrative `feature-delta.md`
  (DISCUSS/DESIGN/DISTILL/DELIVER [REF] sections), `discuss/`, `design/`, `distill/`, `deliver/`
  (roadmap.json, execution-log.json), `slices/`.
- **Parent slice-17 archive** (the landing hub — the source of `LANDING_HUB_SURFACES`, the surface list
  this slice renders as persistent chrome; the landing page migrated onto the shell):
  `docs/evolution/viewer-landing-dashboard-evolution.md`
- **Slice-20 archive** (the immediately-prior slice — the four-arm search follow-state):
  `docs/evolution/viewer-search-full-follow-state-evolution.md`
- **Grandparent slice-08 archive** (the read-only `GET /search` network-discovery view, one of the nav
  surfaces): `docs/evolution/viewer-network-search-evolution.md`
- **Slice-21 ADR**:
  `docs/adrs/ADR-058-persistent-viewer-left-nav-hx-boost-content-region-swap-into-outer-viewer-main-with-hx-boosted-full-page-shape-fork.md`
- **Architecture design / component boundaries / C4 / boosted-shape data-flow**:
  `docs/feature/viewer-persistent-left-nav/design/` + the DESIGN sections of `feature-delta.md`
- **DELIVER execution log + roadmap**:
  `docs/feature/viewer-persistent-left-nav/deliver/execution-log.json`,
  `docs/feature/viewer-persistent-left-nav/deliver/roadmap.json`
- **Acceptance corpus (executable SSOT)**:
  `tests/acceptance/viewer_persistent_left_nav.rs` (NAV-1..NAV-9, the thick walking skeleton at NAV-1),
  `tests/acceptance/viewer_persistent_left_nav_invariants.rs` (the 6 GOLD invariants NAV-INV-*)
- **New shell + nav (this slice)**: `crates/viewer-domain` (`page_shell(title, active, content)`,
  `render_viewer_nav_oob`, the `aria-current="page"` active marker; the slice-17 landing migrated onto
  the shell), `crates/adapter-http-viewer` (`Shape::from_request` `HX-Boosted → FullPage` via
  `request_is_boosted` + the boosted OOB append; every handler + `render_error` 404 through the shell)
- **Reused surface list**: `crates/viewer-domain::LANDING_HUB_SURFACES` (slice-17) — now read by exactly
  one renderer (the shell)
- **Vendored htmx** (the hx-boost / hx-select / hx-swap-oob machinery): the viewer's vendored htmx asset
  (offline; no CDN)
- **Cross-feature architecture brief** (SSOT): `docs/product/architecture/brief.md`
- **KPI contracts** (cross-feature SSOT): `docs/product/kpi-contracts.yaml` — J-002 (move between surfaces
  without losing place — now realized in its persistent-frame form)
- **Prior evolution archives**: `docs/evolution/openlore-foundation-evolution.md`,
  `openlore-github-scraper-evolution.md`, `openlore-federated-read-evolution.md`,
  `openlore-scoring-graph-evolution.md`, `openlore-appview-search-evolution.md`,
  `htmx-scraper-viewer-evolution.md`, `viewer-htmx-swaps-evolution.md`,
  `viewer-network-search-evolution.md`, `viewer-contributor-scoring-evolution.md`,
  `viewer-graph-traversal-evolution.md`, `viewer-counter-claim-threads-evolution.md`,
  `viewer-counter-claim-list-flags-evolution.md`, `viewer-counter-flags-graph-surfaces-evolution.md`,
  `viewer-counter-flags-score-surface-evolution.md`, `viewer-peer-subscriptions-evolution.md`,
  `viewer-search-follow-state-evolution.md`, `viewer-landing-dashboard-evolution.md`,
  `viewer-counter-aware-counts-evolution.md`, `viewer-peer-counter-aware-counts-evolution.md`,
  `viewer-search-full-follow-state-evolution.md`
- **Supply-chain policy**: `deny.toml`
- **Paradigm**: `docs/adrs/ADR-007-paradigm-functional-rust.md`

## Commit trail

DISCUSS → DESIGN (ADR-058, BLOCKED → APPROVED) → DISTILL (15 RED + `get_boosted()` harness) →
roadmap (5 steps, ratio 0.36, APPROVED) → 01-01 9ccdc6c (walking skeleton) →
01-02 ae74ff6 (landing migration + single-source) → 02-01 2d0e9cb (OOB active-marker, D5) →
02-02 3f22cb6 (byte-parity / no-regression confirm) → 03-01 74ee4cd (gold) →
82f9dfc (Phase-3 refactor: single-sited HX-Boosted via `request_is_boosted`) →
45d7e08 (mutation-gate: kill the `hx-swap-oob` boosted-append guard survivor).
