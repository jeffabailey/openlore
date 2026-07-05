//! Slice-21 acceptance — the `openlore ui` PERSISTENT LEFT NAV across every viewer
//! surface (US-NAV-001 + US-NAV-002; ADR-058). Extends the slice-07 htmx
//! progressive-enhancement swaps + the slice-17 `LANDING_HUB_SURFACES` nav-hub SSOT.
//!
//! Today the read-only viewer renders a navigation hub ONLY on the landing page
//! (`GET /`); the inner surfaces (`/claims`, `/peer-claims`, `/search`, `/score`,
//! `/project`, `/philosophy`, `/peers`) carry NO cross-surface navigation, and every
//! surface-to-surface move is a full-page reload (nothing "stays open"). slice-21
//! RESOLVES this per ADR-058:
//!
//!   • D1 — an outer content region `<main id="viewer-main">` wraps each surface body.
//!   • D2 — a persistent left `<nav id="viewer-nav">` (with `<ul id="viewer-nav-items">`)
//!     renders OUTSIDE `#viewer-main` on EVERY full page, iterating the SAME slice-17
//!     `LANDING_HUB_SURFACES` SSOT (no second list — AC-001.3), the current surface
//!     marked `aria-current="page"` (AC-001.2). The nav carries `hx-boost="true"` +
//!     `hx-target="#viewer-main"` + `hx-select="#viewer-main"`.
//!   • D3 — `Shape::from_request` gains a prior arm: `HX-Boosted` present -> `FullPage`
//!     (boosted nav click returns the full page so the client `hx-select`s
//!     `#viewer-main`); the existing tab/paging `hx-get`s (`HX-Request` ONLY) are
//!     UNCHANGED.
//!   • D5 — on a boosted response the effect shell ALSO emits an out-of-band
//!     `<ul id="viewer-nav-items" hx-swap-oob="innerHTML">` that updates the active
//!     marker in place while the `<nav>` container persists (AC-002.1 refined + 002.3).
//!   • D6 — `page_shell(title, active, content)` owns the chrome; every `render_*_page`
//!     (and the 404 `render_error`) routes its body through it.
//!
//! Read-only / offline / loopback (I-VIEW-1/3/4) + progressive enhancement (I-HX-1/4/5)
//! preserved: the nav is plain `<a href>` links only (no form/button/mutating control),
//! `hx-boost` rides the ALREADY-vendored htmx asset (no new asset, no CDN), and with JS
//! off the plain links do full-page navigation and the nav renders on every full page.
//!
//! Driving discipline (Mandate 1): scenarios enter through the REAL `openlore ui`
//! subprocess (`ViewerServer`) + in-test HTTP GET against the 8 viewer routes —
//! full page (`get`), htmx fragment (`get_htmx`), and boosted (`get_boosted`). NO
//! scenario calls the `viewer-domain` render fns or the adapter `Shape::from_request`
//! directly (those are unit-level, DELIVER). The LOCAL DuckDB store is REAL; the
//! operator's own claims are seeded via the REAL slice-06 `claim add` verb
//! (`seed_own_claims_via_cli`) so the surfaces render genuine content (Pillar 3).
//! No indexer is wired — `/search` renders its unconfigured full page (still 200),
//! which is all the nav-presence assertions need.
//!
//! Layer placement (nw-tdd-methodology Layered Test Discipline matrix): every scenario
//! is a layer-3/layer-5 subprocess + real-I/O test — EXAMPLE-only (Mandate 9/11). The
//! nav is a render-only chrome affordance over a fixed 8-surface SSOT; there is no
//! ≥3-scenario chained journey over a domain-rich state machine, so Tier B
//! (state-machine PBT) is NOT warranted (Mandate 10 skip criteria).
//!
//! Build-before-run note (mirrors slice-08/16): `cargo test` does NOT rebuild a
//! spawned binary automatically — the run MUST `cargo build` the `openlore` bin (the
//! viewer) before running these ATs so `ViewerServer::start` spawns the CURRENT viewer.
//!
//! Mandate 7 RED scaffolds: the ATs import nothing unbuilt at the Rust level (they
//! spawn the bin + HTTP), so they COMPILE now. The RED is the PRODUCTION chrome: no
//! `<nav id="viewer-nav">`, no `<main id="viewer-main">`, no `aria-current="page"`,
//! no `hx-boost`, no `HX-Boosted` shape fork, and no out-of-band `hx-swap-oob`
//! nav-items exist yet — so every nav-region / boosted-shape / active-marker
//! assertion FAILS for the RIGHT reason (MISSING_FUNCTIONALITY), NOT a setup/import
//! error. They stay RED until DELIVER's per-scenario RED→GREEN→COMMIT cycles (ADR-025).
//!
//! Covers:
//! - US-NAV-001 (a left nav on every surface): NAV-1 walking skeleton (all 8 surfaces
//!   render the nav listing all 8, current marked active) · NAV-2 nav on all 8 routes
//!   (AC-001.1) · NAV-3 exactly-one-active full page + boosted (AC-001.2/002.3) ·
//!   NAV-4 single-source vs `LANDING_HUB_SURFACES` (AC-001.3) · NAV-5 no-JS full-page
//!   nav + working links (AC-001.4).
//! - US-NAV-002 (the nav stays open across navigation): NAV-6 boosted content-only
//!   swap keeps the nav mounted — full page with `#viewer-main` + OOB nav-items
//!   (AC-002.1) · NAV-7 address-bar/history via `hx-boost` attrs (AC-002.2) · NAV-8
//!   boosted content byte-parity with the full-page `#viewer-main` region (AC-002.4) ·
//!   NAV-9 no-regression: prior surface content survives, only the nav is added
//!   (AC-002.5).
//
// SCAFFOLD: true

mod support;

#[allow(unused_imports)]
use support::*;

// -----------------------------------------------------------------------------
// Observable production tokens (SSOT for the OBSERVABLE rendered surface the nav
// introduces; all ABSENT today → the assertions that scan for them are RED for the
// RIGHT reason). Mirror the ADR-058 const/attribute names; DELIVER mints the pure-core
// consts (`VIEWER_NAV_ID` / `viewer-nav-items` / `VIEWER_MAIN_ID`) that render these.
// -----------------------------------------------------------------------------

/// The persistent left-nav container id (ADR-058 D2 — `<nav id="viewer-nav">`).
const NAV_CONTAINER: &str = "id=\"viewer-nav\"";
/// The inner link-list id the OOB active-marker swap targets (ADR-058 D5 —
/// `<ul id="viewer-nav-items">`).
const NAV_ITEMS: &str = "id=\"viewer-nav-items\"";
/// The outer content region `hx-select`/`hx-target` extract on a boosted swap
/// (ADR-058 D1 — `<main id="viewer-main">`).
const VIEWER_MAIN: &str = "id=\"viewer-main\"";
/// The neutral, semantic current-surface marker (ADR-058 D2 — AC-001.2/002.3).
const ARIA_CURRENT: &str = "aria-current=\"page\"";
/// The progressive-enhancement boost attribute the `<nav>` carries (ADR-058 D2/D3).
const HX_BOOST: &str = "hx-boost=\"true\"";
/// The outer swap target the boosted nav points at (ADR-058 D2).
const HX_TARGET_MAIN: &str = "hx-target=\"#viewer-main\"";
/// The out-of-band swap directive that updates the active marker in place on a
/// boosted response (ADR-058 D5).
const HX_SWAP_OOB: &str = "hx-swap-oob=\"innerHTML\"";

/// The 8 viewer routes AC-001.1 requires the nav on (`/` + the 7 inner surfaces).
/// All return 200 today (landing / claims / peer-claims render content; bare
/// `/score` / `/project` / `/philosophy` render their guided empty state; `/peers`
/// renders the empty subscriptions state; unconfigured `/search` renders its calm
/// Unavailable full page). The nav is ABSENT from every one of them today → RED.
const NAV_ROUTES: &[&str] = &[
    "/",
    "/claims",
    "/peer-claims",
    "/search",
    "/score",
    "/project",
    "/philosophy",
    "/peers",
];

/// Extract the inner text of the `<main id="viewer-main">…</main>` content region
/// (from the id anchor to the closing `</main>`), or `None` if the region is absent
/// (as it is TODAY — the byte-parity scenario asserts its presence FIRST, so a `None`
/// here surfaces as the presence assertion's RED, never a silent pass).
fn viewer_main_region(body: &str) -> Option<&str> {
    let start = body.find(VIEWER_MAIN)?;
    let end = body[start..].find("</main>").map(|e| start + e)?;
    Some(&body[start..end])
}

// =============================================================================
// US-NAV-001 — Theme A: a left navigation on EVERY viewer surface. (NAV-1 walking
// skeleton · NAV-2 all-8-routes · NAV-3 exactly-one-active · NAV-4 single-source ·
// NAV-5 no-JS)
// =============================================================================

/// NAV-1 / WALKING SKELETON (US-NAV-001 + US-NAV-002; AC-001.1/001.2 — the thinnest
/// complete thread the slice can demo end-to-end): the operator loads EVERY one of the
/// 8 viewer surfaces and, on each, sees the SAME persistent left navigation listing
/// ALL 8 surfaces from `LANDING_HUB_SURFACES` as plain `<a href>` links inside a
/// `<nav id="viewer-nav">`, wrapped around an outer `<main id="viewer-main">` content
/// region — and the surface she is ON is marked `aria-current="page"`. This is the
/// load-bearing user outcome: "reach every surface from every surface in one click,
/// always knowing where I am."
///
/// Given the viewer is running over a store with the operator's own claims;
/// When she opens each of the 8 viewer surfaces in turn (full page);
/// Then every surface renders the persistent left nav listing all 8 surfaces, wrapped
///   around a `#viewer-main` content region, with the current surface (when it is a
///   nav item) marked `aria-current="page"`.
///
/// @us-nav-001 @us-nav-002 @walking_skeleton @driving_port @driving_adapter @real-io
/// @nav-reach @nav-active @happy
#[test]
fn every_viewer_surface_renders_the_persistent_left_nav_marking_the_current_surface() {
    // GIVEN a REAL store with genuine content (own claims via the production `claim
    // add` verb — Pillar 3), and the REAL `openlore ui` viewer over it.
    let env = TestEnv::initialized();
    seed_own_claims_via_cli(&env, 3);
    let viewer = ViewerServer::start(&env);

    for route in NAV_ROUTES {
        // WHEN she opens the surface as a full page.
        let response = viewer.get(route);
        assert_eq!(
            response.status, 200,
            "NAV-1: GET {route} must render a 200 full page (the nav must be present on \
             EVERY surface, AC-001.1); body:\n{}",
            response.body
        );
        assert!(
            response.is_full_page(),
            "NAV-1: GET {route} (no-JS) must be a COMPLETE full page carrying the nav \
             chrome; body:\n{}",
            response.body
        );
        // THEN the persistent left nav is present (RED today — no `<nav id="viewer-nav">`).
        assert!(
            response.body.contains(NAV_CONTAINER),
            "NAV-1 (AC-001.1): GET {route} must render the persistent left nav \
             ({NAV_CONTAINER}); body:\n{}",
            response.body
        );
        // …wrapped around the outer content region (RED today — no `#viewer-main`).
        assert!(
            response.body.contains(VIEWER_MAIN),
            "NAV-1 (D1): GET {route} must wrap its body in the outer content region \
             ({VIEWER_MAIN}); body:\n{}",
            response.body
        );
        // …listing ALL 8 surfaces from the slice-17 SSOT as plain `<a href>` links
        // (RED today — the inner surfaces carry no cross-surface links).
        assert_landing_links_all_surfaces(&response.body);
        // …and when the current route IS a nav item, exactly that item is marked
        // active (RED today — no `aria-current="page"` anywhere). `/` is NOT a nav
        // item (the landing content region keeps only its summary, ADR-058 D2), so it
        // carries no active marker.
        if *route != "/" {
            let current_href = format!("href=\"{route}\"");
            assert!(
                response.body.contains(ARIA_CURRENT),
                "NAV-1 (AC-001.2): GET {route} must mark the current surface \
                 {current_href} active ({ARIA_CURRENT}); body:\n{}",
                response.body
            );
        }
    }
}

/// NAV-2 (US-NAV-001; AC-001.1 — the reach guarantee, KPI-NAV-1): the persistent left
/// nav is present on EVERY one of the 8 viewer routes, each listing all 8 surfaces —
/// 100% nav reach. The focused companion to the walking skeleton: it pins the
/// route-by-route reach coverage as its own guardrail (the nav is not present on ANY
/// inner surface today — only the landing hub exists).
///
/// Given the viewer is running;
/// When each of the 8 viewer routes is requested;
/// Then every route's response carries the `<nav id="viewer-nav">` listing all 8
///   surfaces.
///
/// @us-nav-001 @driving_port @real-io @nav-reach @kpi @happy
#[test]
fn the_left_nav_is_present_on_every_one_of_the_eight_routes() {
    let env = TestEnv::initialized();
    seed_own_claims_via_cli(&env, 2);
    let viewer = ViewerServer::start(&env);

    for route in NAV_ROUTES {
        let response = viewer.get(route);
        assert_eq!(
            response.status, 200,
            "NAV-2: GET {route} must be 200; body:\n{}",
            response.body
        );
        // THEN the nav container + its inner item list are present (RED today).
        assert!(
            response.body.contains(NAV_CONTAINER) && response.body.contains(NAV_ITEMS),
            "NAV-2 (AC-001.1 / KPI-NAV-1): GET {route} must expose the persistent left \
             nav ({NAV_CONTAINER} wrapping {NAV_ITEMS}); body:\n{}",
            response.body
        );
        // …listing all 8 surfaces from `LANDING_HUB_SURFACES` (RED today).
        assert_landing_links_all_surfaces(&response.body);
    }
}

/// NAV-3 (US-NAV-001 + US-NAV-002; AC-001.2 + AC-002.3 — the active-marker guarantee):
/// on a given surface EXACTLY ONE nav item is marked `aria-current="page"` — the
/// surface the operator is on — and no other. AND after a BOOSTED navigation to a
/// surface, the out-of-band `<ul id="viewer-nav-items" hx-swap-oob="innerHTML">` copy
/// carries the active marker for the newly-active surface (so the marker updates in
/// place while the nav container persists, ADR-058 D5).
///
/// Given the operator is on `/claims`;
/// When the page renders (full page) and again when `/peer-claims` is navigated
///   boosted;
/// Then the full page marks exactly one item (`/claims`) active, and the boosted
///   response's OOB nav-items list marks the newly-active surface (`/peer-claims`)
///   active.
///
/// @us-nav-001 @us-nav-002 @driving_port @real-io @nav-active @boosted @happy
#[test]
fn exactly_one_nav_item_is_marked_active_on_the_page_and_updates_on_a_boosted_swap() {
    let env = TestEnv::initialized();
    seed_own_claims_via_cli(&env, 3);
    let viewer = ViewerServer::start(&env);

    // WHEN she is on `/claims` (full page).
    let full = viewer.get("/claims");
    assert_eq!(full.status, 200, "NAV-3: GET /claims must be 200; body:\n{}", full.body);
    // THEN exactly ONE `aria-current="page"` marker appears (RED today — none exist).
    let active_count = full.body.matches(ARIA_CURRENT).count();
    assert_eq!(
        active_count, 1,
        "NAV-3 (AC-001.2): the nav on /claims must mark EXACTLY ONE item active \
         ({ARIA_CURRENT}), got {active_count}; body:\n{}",
        full.body
    );

    // WHEN she navigates to `/peer-claims` boosted (a left-nav click).
    let boosted = viewer.get_boosted("/peer-claims");
    assert_eq!(
        boosted.status, 200,
        "NAV-3: boosted GET /peer-claims must be 200; body:\n{}",
        boosted.body
    );
    // THEN the boosted response carries the out-of-band nav-items list (RED today —
    // no OOB swap emitted) that updates the active marker in place (AC-002.3 / D5).
    assert!(
        boosted.body.contains(HX_SWAP_OOB) && boosted.body.contains(NAV_ITEMS),
        "NAV-3 (AC-002.3 / D5): the boosted /peer-claims response must append the \
         out-of-band nav-items list ({NAV_ITEMS} with {HX_SWAP_OOB}) to update the \
         active marker in place; body:\n{}",
        boosted.body
    );
    // …marking the newly-active surface active (the OOB copy carries the marker).
    assert!(
        boosted.body.contains(ARIA_CURRENT),
        "NAV-3 (AC-002.3): the boosted /peer-claims OOB nav-items must mark the \
         newly-active surface {ARIA_CURRENT}; body:\n{}",
        boosted.body
    );
}

/// NAV-4 (US-NAV-001; AC-001.3 — the single-source-of-truth guarantee, KPI drift
/// guard): the nav item set on an inner surface is derived from the SAME
/// `LANDING_HUB_SURFACES` table — it lists ALL 8 shipped surfaces and NO deep /
/// parameterized route (`/claims/{cid}`, `?contributor=`, `?subject=`, `?object=`).
/// A surface absent from that table is absent from the nav, and vice-versa (no second,
/// driftable literal list).
///
/// Given the operator is on an inner surface (`/score`);
/// When the page renders;
/// Then the nav lists all 8 `LANDING_HUB_SURFACES` surfaces and links no deep /
///   parameterized route as a top-level item.
///
/// @us-nav-001 @driving_port @real-io @single-source @happy
#[test]
fn the_nav_item_set_is_sourced_solely_from_the_landing_hub_surfaces_ssot() {
    let env = TestEnv::initialized();
    seed_own_claims_via_cli(&env, 2);
    let viewer = ViewerServer::start(&env);

    // WHEN she is on an INNER surface (not the landing) — the nav must carry the SAME
    // full 8-surface set the landing hub carries, from the SAME SSOT.
    let response = viewer.get("/score");
    assert_eq!(
        response.status, 200,
        "NAV-4: GET /score must be 200; body:\n{}",
        response.body
    );
    // The nav container is present (RED today).
    assert!(
        response.body.contains(NAV_CONTAINER),
        "NAV-4 (AC-001.3): GET /score must render the persistent nav ({NAV_CONTAINER}); \
         body:\n{}",
        response.body
    );
    // THEN it lists ALL 8 surfaces from the SSOT (RED today — an inner surface carries
    // no cross-surface links).
    assert_landing_links_all_surfaces(&response.body);
    // …and NO deep / parameterized route appears as a top-level nav item (the single
    // source is the top-level SSOT, never a hand-rolled list that could add drill
    // routes). GREEN-today-shaped scan (the inner surface links no deep route as a nav
    // item), promoted to a nav guardrail once the nav renders.
    assert_landing_no_deep_route_toplevel(&response.body);
}

/// NAV-5 (US-NAV-001; AC-001.4 — progressive enhancement / no-JS, KPI-NAV-3): with
/// JavaScript disabled (a plain `GET`, no `HX-Request`), the left nav STILL renders on
/// an inner surface and every nav link is a working full-page `<a href>` navigation
/// (not an `hx-get`-only affordance). The nav degrades to plain links with JS off.
///
/// Given JavaScript is disabled (a plain full-page GET);
/// When the operator loads an inner surface (`/peer-claims`);
/// Then the left nav renders and every one of the 8 surfaces is a working plain
///   `<a href>` full-page link.
///
/// @us-nav-001 @driving_port @real-io @progressive-enhancement @no-js @kpi @happy
#[test]
fn the_nav_renders_with_working_full_page_links_when_javascript_is_disabled() {
    let env = TestEnv::initialized();
    seed_own_claims_via_cli(&env, 2);
    let viewer = ViewerServer::start(&env);

    // WHEN the operator loads an inner surface WITHOUT htmx (the no-JS / curl path —
    // `get`, no `HX-Request` header).
    let response = viewer.get("/peer-claims");
    assert_eq!(
        response.status, 200,
        "NAV-5: no-JS GET /peer-claims must be 200; body:\n{}",
        response.body
    );
    assert!(
        response.is_full_page(),
        "NAV-5 (AC-001.4): the no-JS response must be a COMPLETE full page; body:\n{}",
        response.body
    );
    // THEN the nav renders (RED today) …
    assert!(
        response.body.contains(NAV_CONTAINER),
        "NAV-5 (AC-001.4 / KPI-NAV-3): the no-JS full page must render the persistent \
         nav ({NAV_CONTAINER}); body:\n{}",
        response.body
    );
    // …with every surface a WORKING plain `<a href>` full-page link (no-JS navigable —
    // an `hx-boost`/`hx-get`-only affordance without `href` would NOT satisfy this).
    assert_landing_links_all_surfaces(&response.body);
}

// =============================================================================
// US-NAV-002 — Theme B: the left nav STAYS OPEN across navigation. (NAV-6 boosted
// keeps nav mounted · NAV-7 hx-boost history attrs · NAV-8 byte-parity · NAV-9
// no-regression)
// =============================================================================

/// NAV-6 (US-NAV-002; AC-002.1 refined — the nav STAYS MOUNTED across a boosted swap,
/// KPI-NAV-2): a boosted navigation to a surface returns the FULL page (so the client
/// can `hx-select="#viewer-main"` the content region) AND appends the out-of-band
/// `<ul id="viewer-nav-items" hx-swap-oob="innerHTML">` copy — so htmx swaps ONLY the
/// `#viewer-main` content while the `<nav id="viewer-nav">` container persists (its
/// active marker updated in place, never re-fetched or torn down, ADR-058 D5).
///
/// Given htmx is active;
/// When the operator clicks a left-nav item (a boosted GET for `/score`);
/// Then the response is a FULL page containing `#viewer-main` (so `hx-select` can
///   target it) and the out-of-band nav-items list (so the nav container persists,
///   not re-fetched).
///
/// @us-nav-002 @driving_port @driving_adapter @real-io @nav-persist @boosted @kpi
/// @happy
#[test]
fn a_boosted_nav_click_returns_a_full_page_with_viewer_main_and_oob_nav_items() {
    let env = TestEnv::initialized();
    seed_own_claims_via_cli(&env, 2);
    let viewer = ViewerServer::start(&env);

    // WHEN a left-nav item is clicked (a boosted GET — carries HX-Request + HX-Boosted).
    let boosted = viewer.get_boosted("/score");
    assert_eq!(
        boosted.status, 200,
        "NAV-6: boosted GET /score must be 200; body:\n{}",
        boosted.body
    );
    // THEN the boosted response is a FULL page (ADR-058 D3 — HX-Boosted forks to
    // FullPage so the client can hx-select the content region). RED today: the
    // `Shape::from_request` HX-Boosted arm does not exist, so an HX-Request GET returns
    // the bare fragment (no full-page chrome).
    assert!(
        boosted.is_full_page(),
        "NAV-6 (AC-002.1 / D3): a boosted nav click must return the COMPLETE full page \
         (so htmx can hx-select #viewer-main), NOT a bare fragment; body:\n{}",
        boosted.body
    );
    // …carrying the outer content region for `hx-select` to extract (RED today).
    assert!(
        boosted.body.contains(VIEWER_MAIN),
        "NAV-6 (AC-002.1 / D1): the boosted full page must carry the outer content \
         region {VIEWER_MAIN} for hx-select; body:\n{}",
        boosted.body
    );
    // …AND the out-of-band nav-items copy so the nav container persists while the
    // active marker updates in place (RED today — no OOB swap emitted).
    assert!(
        boosted.body.contains(NAV_ITEMS) && boosted.body.contains(HX_SWAP_OOB),
        "NAV-6 (AC-002.1 refined / D5): the boosted response must append the OOB \
         nav-items list ({NAV_ITEMS} with {HX_SWAP_OOB}) so the persistent nav is NOT \
         re-fetched or torn down; body:\n{}",
        boosted.body
    );
}

/// NAV-7 (US-NAV-002; AC-002.2 — address bar + history): the persistent nav is driven
/// by `hx-boost` targeting `#viewer-main` — the mechanism that makes htmx push the
/// surface URL into the address bar and makes browser Back/forward return to the prior
/// surface's content (ADR-058 D2/D3: `hx-boost` auto-pushes the URL into history). The
/// full page carries the `hx-boost="true"` + `hx-target="#viewer-main"` +
/// `hx-select="#viewer-main"` attributes on the nav.
///
/// Given the viewer serves a surface;
/// When the page renders;
/// Then its nav carries `hx-boost="true"` targeting/selecting `#viewer-main` — the
///   history-and-address-bar mechanism (AC-002.2).
///
/// @us-nav-002 @driving_port @real-io @history @address-bar @happy
#[test]
fn the_nav_carries_hx_boost_attributes_that_drive_the_address_bar_and_history() {
    let env = TestEnv::initialized();
    seed_own_claims_via_cli(&env, 2);
    let viewer = ViewerServer::start(&env);

    let response = viewer.get("/claims");
    assert_eq!(
        response.status, 200,
        "NAV-7: GET /claims must be 200; body:\n{}",
        response.body
    );
    // THEN the nav carries the boost mechanism (RED today — no `hx-boost` on any nav).
    // `hx-boost` is the htmx feature that auto-pushes the navigated URL into history
    // (address bar updates; Back/forward work) — the AC-002.2 mechanism.
    assert!(
        response.body.contains(HX_BOOST),
        "NAV-7 (AC-002.2): the persistent nav must carry {HX_BOOST} (the mechanism that \
         updates the address bar + makes Back/forward work); body:\n{}",
        response.body
    );
    // …targeting the outer content region (so the boosted GET's hx-select extracts
    // `#viewer-main`, ADR-058 D2). RED today.
    assert!(
        response.body.contains(HX_TARGET_MAIN),
        "NAV-7 (AC-002.2 / D2): the boosted nav must target {HX_TARGET_MAIN}; body:\n{}",
        response.body
    );
}

/// NAV-8 (US-NAV-002; AC-002.4 — content parity, KPI-NAV-4 / I-HX-5): the content a
/// boosted swap yields for a surface is byte-equivalent to the content region of that
/// surface's full-page render — because the boosted response IS the full page and
/// `hx-select="#viewer-main"` extracts exactly the same `#viewer-main` region (ONE
/// render path per surface, ADR-058 D3). Behavioral proxy: the `#viewer-main` inner
/// region of the boosted response equals the `#viewer-main` inner region of the plain
/// full page.
///
/// Given a surface with genuine content (`/claims`);
/// When it is fetched boosted and as a plain full page;
/// Then both carry `#viewer-main`, and the boosted `#viewer-main` region is
///   byte-identical to the full-page `#viewer-main` region.
///
/// @us-nav-002 @driving_port @real-io @parity @kpi @boundary
#[test]
fn the_boosted_content_region_is_byte_identical_to_the_full_page_viewer_main() {
    let env = TestEnv::initialized();
    seed_own_claims_via_cli(&env, 4);
    let viewer = ViewerServer::start(&env);

    let path = "/claims";
    let boosted = viewer.get_boosted(path);
    let full = viewer.get(path);
    assert_eq!(boosted.status, 200, "NAV-8: boosted GET {path} must be 200; body:\n{}", boosted.body);
    assert_eq!(full.status, 200, "NAV-8: full GET {path} must be 200; body:\n{}", full.body);

    // Both responses must carry the outer content region (RED today — no `#viewer-main`
    // exists, so the parity comparison never reaches a false pass).
    assert!(
        boosted.body.contains(VIEWER_MAIN),
        "NAV-8 (AC-002.4 / D1): the boosted response must carry {VIEWER_MAIN}; body:\n{}",
        boosted.body
    );
    assert!(
        full.body.contains(VIEWER_MAIN),
        "NAV-8 (AC-002.4 / D1): the full page must carry {VIEWER_MAIN}; body:\n{}",
        full.body
    );
    // THEN the boosted content region is byte-identical to the full-page content region
    // (parity by construction — the boosted response IS the full page; hx-select cannot
    // diverge from it, I-HX-5).
    let boosted_main = viewer_main_region(&boosted.body)
        .expect("NAV-8: boosted #viewer-main region must be extractable");
    let full_main = viewer_main_region(&full.body)
        .expect("NAV-8: full-page #viewer-main region must be extractable");
    assert_eq!(
        boosted_main, full_main,
        "NAV-8 (AC-002.4 / KPI-NAV-4): the boosted #viewer-main content region must be \
         BYTE-IDENTICAL to the full-page #viewer-main region (one render path per \
         surface); boosted:\n{boosted_main}\n---\nfull:\n{full_main}"
    );
}

/// NAV-9 (US-NAV-002; AC-002.5 — no-regression, KPI-NAV-4): the no-JS full-page render
/// of a surface is byte-unaffected by this feature versus its prior full-page render
/// EXCEPT for the added nav region. Behavioral proxy: an inner surface's pre-existing
/// content (its own heading / read-only chrome) still renders intact on the no-JS full
/// page, AND the new nav region is now additionally present — the nav ADDS a region,
/// it does not perturb the surface's existing content.
///
/// Given the operator loads an inner surface with JS off;
/// When the full page renders;
/// Then the surface's pre-existing content survives intact AND the persistent nav is
///   now additionally present (the feature only ADDS the nav region).
///
/// @us-nav-002 @driving_port @real-io @no-regression @kpi @boundary
#[test]
fn the_no_js_full_page_content_is_unaffected_except_for_the_added_nav_region() {
    let env = TestEnv::initialized();
    seed_own_claims_via_cli(&env, 3);
    let viewer = ViewerServer::start(&env);

    // The claims surface's pre-existing full-page content (heading + read-only notice)
    // must survive — the feature adds the nav region, it does not rewrite the surface.
    let response = viewer.get("/claims");
    assert_eq!(
        response.status, 200,
        "NAV-9: GET /claims must be 200; body:\n{}",
        response.body
    );
    assert!(
        response.is_full_page(),
        "NAV-9: the no-JS /claims response must be a COMPLETE full page; body:\n{}",
        response.body
    );
    // The prior surface content is intact (GREEN-shaped no-regression anchor: the
    // read-only notice + the OpenLore title chrome are byte-present as before).
    assert!(
        response.body.to_lowercase().contains(READ_ONLY_NOTICE_TEXT),
        "NAV-9 (AC-002.5): the /claims surface's pre-existing read-only notice must \
         survive intact (no-regression); body:\n{}",
        response.body
    );
    assert!(
        response.body.contains("OpenLore"),
        "NAV-9 (AC-002.5): the /claims surface's existing OpenLore chrome must survive \
         intact (no-regression); body:\n{}",
        response.body
    );
    // …AND the nav region is now additionally present, wrapped around `#viewer-main`
    // (RED today — the ADDED region does not exist yet). The feature ONLY adds the nav.
    assert!(
        response.body.contains(NAV_CONTAINER) && response.body.contains(VIEWER_MAIN),
        "NAV-9 (AC-002.5): the ONLY change to the /claims full page is the ADDED nav \
         region ({NAV_CONTAINER}) wrapping the content region ({VIEWER_MAIN}); body:\n{}",
        response.body
    );
}
