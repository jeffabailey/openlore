<!-- markdownlint-disable MD013 -->
# RED Classification — slice-21 (viewer-persistent-left-nav)

> DISTILL Pre-DELIVER fail-for-the-right-reason gate (nw-distill §"Pre-DELIVER
> fail-for-the-right-reason gate"). Every slice-21 acceptance scenario was run once
> against the CURRENT (unimplemented) production code and classified. DELIVER reads
> this file at the RED-phase entry gate (ADR-025 D2) to confirm RED is genuine.
>
> Owner: Quinn (nw-acceptance-designer) · 2026-07-05 · Rust / cucumber-free
> subprocess-HTTP acceptance shape (mirrors slice-16/20).

## How the run was performed

```
cargo build --bin openlore                                   # build-before-run (the viewer bin is spawned, not rebuilt by cargo test)
cargo test -p cli --test viewer_persistent_left_nav -- --test-threads=1
cargo test -p cli --test viewer_persistent_left_nav_invariants -- --test-threads=1
```

Both targets COMPILE green (no `error[...]`, no unbuilt Rust dependency) — they spawn
the real `openlore ui` bin over HTTP and import only the existing `support` harness
(plus the NEW `ViewerServer::get_boosted` method added this slice). Therefore EVERY
failure is a RUNTIME assertion on the OBSERVABLE rendered HTTP body — RED, never
BROKEN. Unlike slice-20, there is NO `todo!()` scaffold seam: `get_boosted` is a real
HTTP method (HX-Request + HX-Boosted headers), so every RED is an assertion RED.

## What is missing today (the RED cause)

The current viewer renders THREE distinct, unrelated `<nav>`s and no outer content
region:

- `GET /` (landing) renders the slice-17 hub `<nav><ul><li><a href="/claims">…</a>…</ul></nav>`
  listing all 8 surfaces — but with NO `id="viewer-nav"`, NO `id="viewer-nav-items"`.
- `GET /claims` + `/peer-claims` render the tab `<nav>` (My Claims / Peer Claims only,
  `hx-get` + `hx-target="#view-panel"` + `hx-push-url`) — NOT the surface nav.
- `GET /score` etc. render `<nav><a href="/claims">My Claims</a></nav>` (a single back-link).

None carries `id="viewer-nav"`, `id="viewer-main"`, `aria-current="page"`, `hx-boost`,
or the OOB `hx-swap-oob` nav-items; and `Shape::from_request` has no `HX-Boosted` arm
(so a boosted GET returns the bare fragment, not the full page). Every slice-21 RED
scenario fails on exactly one of these MISSING affordances.

## Classification key

- **RED (MISSING_FUNCTIONALITY, assertion)** ✅ — the assertion fires because the nav
  chrome (`#viewer-nav` / `#viewer-main` / `aria-current` / `hx-boost` / the `HX-Boosted`
  FullPage fork / the OOB `hx-swap-oob` nav-items) is unimplemented. Correct RED.
- **GREEN-today (guardrail / no-regression)** ✅ — the scenario PASSES against current
  code because it pins a CARDINAL that must NOT regress (the landing still links all 8;
  the page stays offline / no-CDN with the vendored htmx asset). Intentionally
  green-from-the-start; DELIVER must keep them green when it moves the links into the
  persistent nav.
- **BROKEN / SETUP / IMPORT** ❌ — would block handoff. **NONE remain.**

## Tally

| File | Scenario | Classification | Why |
|---|---|---|---|
| `viewer_persistent_left_nav.rs` | NAV-1 `every_viewer_surface_renders_the_persistent_left_nav_marking_the_current_surface` (WS) | RED ✅ | no `id="viewer-nav"` on any route (landing hub `<nav>` has no id; inner surfaces have only tab/back navs) |
| | NAV-2 `the_left_nav_is_present_on_every_one_of_the_eight_routes` (AC-001.1) | RED ✅ | no `id="viewer-nav"` wrapping `id="viewer-nav-items"` on any route |
| | NAV-3 `exactly_one_nav_item_is_marked_active_on_the_page_and_updates_on_a_boosted_swap` (AC-001.2/002.3) | RED ✅ | `aria-current="page"` count is 0, expected 1; no OOB nav-items on the boosted response |
| | NAV-4 `the_nav_item_set_is_sourced_solely_from_the_landing_hub_surfaces_ssot` (AC-001.3) | RED ✅ | `/score` renders no `id="viewer-nav"` (single back-link nav only) |
| | NAV-5 `the_nav_renders_with_working_full_page_links_when_javascript_is_disabled` (AC-001.4) | RED ✅ | `/peer-claims` no-JS full page has the tab nav, not the persistent `id="viewer-nav"` |
| | NAV-6 `a_boosted_nav_click_returns_a_full_page_with_viewer_main_and_oob_nav_items` (AC-002.1) | RED ✅ | boosted `GET /score` returns the bare fragment `<div id="score-results">` — no `HX-Boosted`→FullPage fork, no `#viewer-main`, no OOB |
| | NAV-7 `the_nav_carries_hx_boost_attributes_that_drive_the_address_bar_and_history` (AC-002.2) | RED ✅ | no `hx-boost="true"` / `hx-target="#viewer-main"` on any nav (the tab nav uses `hx-get`+`#view-panel`) |
| | NAV-8 `the_boosted_content_region_is_byte_identical_to_the_full_page_viewer_main` (AC-002.4) | RED ✅ | boosted response has no `id="viewer-main"` (it is the bare `#view-panel` fragment) — parity comparison never reached |
| | NAV-9 `the_no_js_full_page_content_is_unaffected_except_for_the_added_nav_region` (AC-002.5) | RED ✅ | the pre-existing content survives (read-only notice + OpenLore chrome present) but the ADDED `id="viewer-nav"` + `id="viewer-main"` region is absent |
| `viewer_persistent_left_nav_invariants.rs` | NAV-INV-NoControl `the_persistent_nav_adds_no_executable_control` (AC-001.5) | RED ✅ | nav region (`id="viewer-nav"`..`</nav>`) not extractable — the persistent nav is absent |
| | NAV-INV-Offline `the_persistent_nav_stays_offline_with_the_vendored_asset` (I-HX-2) | GREEN-today ✅ | the page references no CDN and `/static/htmx.min.js` serves locally as JS today; must stay green |
| | NAV-INV-NoJs `the_nav_renders_with_plain_links_on_every_route_with_js_off` (AC-001.4) | RED ✅ | inner routes render no `id="viewer-nav"` on the no-JS full page |
| | NAV-INV-SingleSource `the_nav_item_set_never_drifts_from_the_surface_ssot` (AC-001.3) | RED ✅ | `/peer-claims` renders no `id="viewer-nav"` (tab nav only) |
| | NAV-INV-LandingNoRegression `the_landing_still_links_all_eight_surfaces` (Migration) | GREEN-today ✅ | the slice-17 landing hub already links all 8 surfaces; must stay green after the migration into the nav |
| | NAV-INV-LandingViaNav `the_landing_now_sources_its_surface_links_from_the_persistent_nav` (Migration) | RED ✅ | the landing hub `<nav>` has no `id="viewer-nav"` — the links do not yet come via the shared `page_shell` + `render_viewer_nav` path |

### Numeric summary (slice-21 scenarios only; excludes the 2 pre-existing `state_delta` framework self-tests per binary)

| Classification | Count |
|---|---|
| RED — MISSING_FUNCTIONALITY (assertion) | 13 |
| RED — MISSING_FUNCTIONALITY (`todo!()` scaffold) | 0 |
| GREEN-today (no-regression / offline guardrail) | 2 |
| **BROKEN / SETUP / IMPORT** | **0** |
| **Total slice-21 scenarios** | **15** |

RED total = **13** (all assertion; no scaffold seam this slice). GREEN-today guardrails
= **2**. Zero BROKEN. Observed runner output: main `2 passed; 9 failed` (the 2 passes are
the `support::state_delta` framework self-tests, not slice-21 scenarios); invariants
`4 passed; 4 failed` (2 passes are the same framework self-tests, 2 are the GREEN-today
guardrails).

The 2 GREEN-today scenarios are correct by design: they pin cardinals (the landing still
links all 8; the chrome stays offline / no-CDN with the vendored asset) that the current
code already satisfies — they exist to FAIL if DELIVER's migration drops a link or
introduces a CDN, not to drive new implementation. The 13 RED scenarios drive the new
persistent-nav behavior.

## Gate verdict

**PASS.** Every failing scenario fails for the RIGHT reason (MISSING_FUNCTIONALITY — a
runtime assertion on the unimplemented persistent nav / outer content region / boosted
shape fork / OOB active-marker swap). Zero scenarios are in category 2 (IMPORT_ERROR /
FIXTURE_BROKEN / SETUP_FAILURE) or category 3 (WRONG_ASSERTION / internal-struct
coupling — every assertion scans the OBSERVABLE rendered HTTP body, never a
`viewer-domain` struct field). Handoff to DELIVER is UNBLOCKED.

## DELIVER pointers (from the observed RED)

1. Mint the pure-core consts + fns (viewer-domain): `VIEWER_MAIN_ID = "viewer-main"`,
   `render_viewer_nav(active)` → `<nav id="viewer-nav"><ul id="viewer-nav-items">…</ul></nav>`
   over `LANDING_HUB_SURFACES` (marking the `active`-matching row `aria-current="page"`,
   carrying `hx-boost="true"` + `hx-target="#viewer-main"` + `hx-select="#viewer-main"`),
   and `render_viewer_nav_oob(active)` → the bare `<ul id="viewer-nav-items" hx-swap-oob="innerHTML">`.
2. Add `page_shell(title, active, content)` and route every `render_*_page` + the 404
   `render_error` through it; the landing content region keeps ONLY its store summary
   (the slice-17 inline hub is superseded — the two GREEN-today golds guard the migration).
3. Extend `Shape::from_request` (adapter-http-viewer) with the prior arm `HX-Boosted`
   present → `FullPage`; append `render_viewer_nav_oob(active)` to the full-page string
   for boosted responses only.
4. `xtask check-arch` stays 21 members / no new crate / no new route (pure-render + one
   adapter fork line).
