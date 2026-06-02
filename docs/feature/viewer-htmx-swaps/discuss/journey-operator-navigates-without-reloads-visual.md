# Journey (visual): operator-navigates-without-reloads — viewer-htmx-swaps (slice-07)

> **Brownfield DELTA on slice-06 (`htmx-scraper-viewer`).** The shipped `openlore ui`
> viewer already serves server-rendered **maud** HTML over six read-only routes, and was
> built "htmx-ready (progressive enhancement)" — but **today every route returns a FULL
> page**. This slice layers real **htmx partial-swaps** on the SAME routes so the operator
> gets in-place updates: no full-page reload, no scroll reset, no flash. The enhancement is
> **purely additive** — a no-JS operator gets the exact slice-06 experience.
>
> Persona: **Maria Santos**, node operator, browsing her own node at `http://127.0.0.1:8788`
> on her own machine (loopback only, no auth — I-VIEW-4). Lightweight UX depth: happy path
> + the key no-JS and offline error paths.

---

## The pain being removed (slice-06 → slice-07)

In slice-06 Maria can finally *see* her store in a browser. But every interaction —
clicking **Next** on a 1,840-row peer list, submitting a scrape target, opening a claim,
switching My Claims ↔ Peer Claims — triggers a **full-page reload**: the browser repaints
the whole document, the scroll position jumps to the top, and there is a visible flash.
On a real-sized store this makes paging feel heavy and loses her place. She is browsing,
not navigating a form, yet every click costs her a full repaint.

The job to be done: **navigate the store without full-page reloads** — keep my place, keep
the page steady, only the part I changed should change.

---

## Emotional arc

```
  Start: SETTLED (slice-06 works; I can see my store)
     │
     │  but every click → FULL RELOAD → JOLT (scroll jumps, flash, lose my place)
     ▼
  Middle: SMOOTH  — Prev/Next, scrape submit, claim open, tab switch
     │              swap only the changed region; the page holds still
     ▼
  End: FLUID + TRUSTING — "this feels like one place I'm moving around in,
                           and it still works with JS off / offline / from a bookmark"
```

Arc pattern: **Problem Relief** (jolt → smooth → fluid). The peak tension in slice-06 was
the per-click jolt; slice-07 dissolves it. Crucially the arc must NOT introduce a NEW
anxiety — so the no-JS / direct-URL / offline path is a **first-class** branch of every
step, not an afterthought. Maria (and a curl/bookmark/no-JS operator) must never hit a
broken half-page.

---

## ASCII flow — the four enhanced interactions (all on EXISTING routes)

```
                         ┌─────────────────────────────────────────────┐
                         │  openlore ui  →  http://127.0.0.1:8788        │
                         │  (slice-06 routes, UNCHANGED URLs/methods)    │
                         └─────────────────────────────────────────────┘
                                              │
        ┌──────────────────┬─────────────────┼──────────────────┬─────────────────┐
        ▼                  ▼                 ▼                  ▼                 ▼
  [1] PAGINATION      [2] SCRAPE FORM   [3] CLAIM DETAIL   [4] TAB SWITCH    (no-JS / curl /
   GET /claims?page    POST /scrape      GET /claims/{cid}  My ↔ Peer Claims   bookmark / view-src)
   GET /peer-claims?p                                       GET /claims |
                                                            GET /peer-claims
        │                  │                 │                  │                 │
   HX-Request?        HX-Request?       HX-Request?        HX-Request?       NO HX-Request
        │                  │                 │                  │                 │
   ┌────┴────┐        ┌────┴────┐       ┌────┴────┐        ┌────┴────┐       ┌────┴────┐
   ▼         ▼        ▼         ▼       ▼         ▼        ▼         ▼       ▼         │
 yes        no       yes       no      yes       no       yes       no      always full page
  │          │        │         │       │         │        │         │       (byte-equivalent
 FRAGMENT  FULL     FRAGMENT  FULL    FRAGMENT  FULL     FRAGMENT  FULL       to slice-06)
 (table +  PAGE     (results  PAGE    (detail   PAGE     (panel    PAGE
 position) (slice-  panel)    (...)   panel)    (nav     for the   (nav      └─ THE LOAD-BEARING
 swapped    06                          inline   to it)   active    to it)      CONTRACT: every
 in place)  exact)                       target)          view)               route still serves a
   │                                                                          complete navigable page
   ▼                                                                          when HX-Request absent.
 page holds still · scroll preserved · no flash · URL still reflects state
```

**The single mechanism (carried as I-HX-1):** the HTTP surface is UNCHANGED — same URLs,
same methods (the existing GETs + the existing `POST /scrape`). The handler inspects the
**`HX-Request` header**: present → return just the **fragment** (the table / results /
detail / tab panel) of the SAME content; absent → return the **full page** exactly as
slice-06 does. **No new data routes.** Only the response *shape* varies by header.

---

## Step-by-step TUI/browser mockups

### Step 1 — Pagination swap (WALKING SKELETON candidate)

Maria is on `/claims` (or `/peer-claims`) and clicks **Next**.

```
BEFORE (slice-06, full reload on every Next):
  click Next → GET /claims?page=2 → whole document repainted → scroll JUMPS to top → flash

AFTER (slice-07, htmx swap):
+-- My Claims --------------------------------- [My Claims] [Peer Claims] --+
| Read-only · http://127.0.0.1:8788 · 312 claims                            |
|                                                                           |
|  ┌──────────── swap target: #claims-table ───────────────────────────┐   |   <- ONLY this
|  │ subject            predicate          object        conf   cid     │   │      region is
|  │ rust-lang/rust     is-maintained-by   The Rust Pr…  0.90   bafy…1   │   │      replaced
|  │ tokio-rs/tokio     has-license        MIT           0.95   bafy…2   │   │
|  │ …                                                                   │   │
|  │ 51–100 of 312          [ Prev ]                       [ Next ]      │   │
|  └─────────────────────────────────────────────────────────────────────┘ |
+---------------------------------------------------------------------------+
   ↑ page header, nav, chrome stay put · scroll position preserved · no flash
```

- htmx request (`HX-Request: true`): handler returns the **claims-table fragment** only
  (the `<table>` rows + the position indicator `51–100 of 312` + Prev/Next) → swapped into
  `#claims-table`. The chrome (read-only banner, nav tabs, count header) is untouched.
- no-JS request (no header): handler returns the **full `/claims?page=2` page** — exactly
  the slice-06 behavior, a complete navigable page (Prev/Next are plain links).

### Step 2 — Live scrape form swap

Maria enters a target on `/scrape` and submits.

```
+-- Live Scrape ------------------------------- [My Claims] [Peer Claims] --+
|  Target: [ tokio-rs/tokio                    ]  [ Propose ]   ← form STAYS |
|                                                                           |
|  ┌──────────── swap target: #scrape-results ─────────────────────────┐   |
|  │ 7 candidates · NOTHING signed or saved · sign in the CLI           │   │   <- results
|  │ subject          predicate     object  conf   derived-from         │   │      (or the
|  │ tokio-rs/tokio   has-license    MIT     0.95   LICENSE @ HEAD       │   │      zero / network
|  │ …                                                                   │   │      guidance) swap
|  └─────────────────────────────────────────────────────────────────────┘ |      in BELOW form
+---------------------------------------------------------------------------+
```

- htmx (`POST /scrape` with `HX-Request`): returns the **candidates fragment** (or the
  zero-candidates / network-down guidance fragment) → swapped into `#scrape-results`; the
  form stays, no reload. **No sign control** rendered (I-VIEW-3 / I-SCR-1 carried forward).
- no-JS: full `/scrape` page re-renders with candidates below the form — slice-06 exact.

### Step 3 — Claim detail inline

Maria clicks a claim row on `/claims`.

```
+-- My Claims ---------------------------------------------------------------+
|  … claims table … (row clicked: bafy…1)                                     |
|  ┌──────────── swap target: #claim-detail ───────────────────────────┐     |
|  │ rust-lang/rust · is-maintained-by · The Rust Project · 0.90        │     │   <- detail
|  │ author did:plc:maria… · composed 2026-04-18T09:12:03Z              │     │      loads inline,
|  │ evidence: github.com/rust-lang/rust · crates.io/…                  │     │      no navigate-away
|  └─────────────────────────────────────────────────────────────────────┘   |
+----------------------------------------------------------------------------+
```

- htmx: `GET /claims/{cid}` with `HX-Request` returns the **claim-detail fragment** →
  swapped into an inline `#claim-detail` panel; the list stays in place.
- no-JS: clicking the row is a plain link to `/claims/{cid}` → full detail page (slice-06).

### Step 4 — My Claims ↔ Peer Claims tab switch

Maria clicks the **Peer Claims** tab.

```
+-- [ My Claims ]  [*Peer Claims*] ------------------------------------------+
|  ┌──────────── swap target: #view-panel ─────────────────────────────┐     |
|  │ Peer Claims · 1,840 from 4 peers                                   │     │   <- the active
|  │ axum/axum   has-license  MIT  0.88  origin: peer-A                 │     │      view panel
|  │ …                                                                  │     │      swaps in place
|  └─────────────────────────────────────────────────────────────────────┘   |
+----------------------------------------------------------------------------+
```

- htmx: `GET /peer-claims` with `HX-Request` returns the **peer-claims panel fragment** →
  swapped into `#view-panel`; the tab nav stays, the browser URL/history updates so the
  view is bookmarkable and Back works (mechanism = OD-HX, e.g. `hx-push-url`).
- no-JS: the tab is a plain link to `/peer-claims` → full page (slice-06).

---

## The offline-first asset (carried as I-HX-2)

htmx itself must be **served by the viewer locally** (vendored or inlined) — **never from a
CDN** — because the dashboard must keep working fully offline (inherits I-VIEW-6 / KPI-5).
This adds ONE static-asset concern (e.g. `GET /static/htmx.min.js`, or inlining the script
into the page chrome). The exact delivery mechanism is **OD-HX-1** for DESIGN.

```
  Page chrome (slice-06 layout/shell helper in viewer-domain)
     └─ references htmx  ──►  served from THIS process (loopback), NOT a CDN
                              ├─ option A: GET /static/htmx.min.js  (new static route)
                              └─ option B: inline <script> in the shell
        offline test: pull the network → every store view + every swap still works
```

---

## No-JS / direct-URL / curl / bookmark / view-source (the first-class fallback branch)

This is **not** a degraded mode — it is the slice-06 contract, preserved byte-for-byte.

```
  curl http://127.0.0.1:8788/claims?page=2          → FULL page (no HX-Request header)
  open a bookmark to /peer-claims                    → FULL page
  JS disabled, click Next                            → browser follows the plain link → FULL page
  view-source on any route                           → complete, navigable HTML

  Guardrail: for a non-htmx request, the response is BYTE-EQUIVALENT to slice-06.
  The slice-06 acceptance suite (26 scenarios) stays GREEN — zero regression.
```

---

## Integration checkpoints (horizontal coherence)

1. **One content, two shapes**: the fragment a route returns under `HX-Request` MUST be the
   SAME content as the corresponding region of the full page (same rows, same position
   indicator, same confidence verbatim). A fragment that diverges from its full-page region
   is an integration failure.
2. **Swap target ids are shared artifacts**: `#claims-table`, `#scrape-results`,
   `#claim-detail`, `#view-panel` must each have a single documented definition (in the
   page chrome) that both the full page and the fragment agree on. See
   `shared-artifacts-registry.md`.
3. **htmx asset single source**: the vendored/inlined htmx asset has ONE source; every page
   references that one source (no CDN, no second copy). Offline test is the gate.
4. **Read-only preserved end-to-end**: swaps ride the existing GET routes + existing
   `POST /scrape`; NO new write/sign route appears; the web process still holds no signing
   key (I-VIEW-1/2 unchanged).
