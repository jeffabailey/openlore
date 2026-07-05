//! Slice-21 acceptance — the persistent left-nav GOLD / guardrail invariants (the
//! cross-cutting I-VIEW-1/3/4 + I-HX-1/2/4/5 guardrails that must hold over the WHOLE
//! nav surface, beyond any single US-NAV-001/002 story; ADR-058 D4). These are the
//! load-bearing, release-relevant guardrail gold tests for the slice-21 persistent-nav
//! DELTA — the BEHAVIORAL layer of enforcement (the pure-core unit tests + `xtask
//! check-arch`'s 21-member / no-new-crate check are the other two, owned by DELIVER).
//!
//! They drive the REAL `openlore ui` verb via the `ViewerServer` subprocess + in-test
//! HTTP (full page `get`, boosted `get_boosted`) over a REAL DuckDB seeded via the REAL
//! `claim add` verb, and assert the hard slice-21 invariants on the OBSERVABLE rendered
//! surface:
//!
//! - `the_persistent_nav_adds_no_executable_control` (NAV-INV-NoControl, I-VIEW-1/3 /
//!   D5 / AC-001.5, CARDINAL): the nav region carries ONLY plain `<a href>` links — NO
//!   `<form>`, `<button>`, or mutating `hx-post`/`hx-put`/`hx-delete` control. The
//!   viewer holds no key; `hx-boost` issues GETs only. (Scanned within the nav region,
//!   NOT the whole page — `/search`/`/scrape` legitimately carry their own forms in the
//!   CONTENT region, outside the nav.)
//! - `the_persistent_nav_stays_offline_with_the_vendored_asset` (NAV-INV-Offline,
//!   I-HX-2 / AC-001.5): the nav references NO external (non-loopback) asset host, and
//!   the `hx-boost`/`hx-select` capability rides the ALREADY-vendored
//!   `/static/htmx.min.js` (served locally, JS content-type) — no new asset, no CDN.
//! - `the_nav_renders_with_plain_links_on_every_route_with_js_off` (NAV-INV-NoJs,
//!   I-HX-1/4 / AC-001.4 / KPI-NAV-3): on EVERY one of the 8 routes, the no-JS full
//!   page renders the nav with all 8 surfaces as working plain `<a href>` links.
//! - `the_nav_item_set_never_drifts_from_the_surface_ssot` (NAV-INV-SingleSource,
//!   AC-001.3 / KPI drift guard): the nav lists EXACTLY the `LANDING_HUB_SURFACES` set
//!   and no deep/parameterized route — one source, no second list.
//! - `the_landing_still_links_all_eight_surfaces` (NAV-INV-LandingNoRegression, ADR-058
//!   Migration / KPI-NAV-4): the slice-17 landing-hub behavior is NOT lost — `GET /`
//!   still links all 8 surfaces (now via the persistent nav). A GREEN-today
//!   no-regression guardrail: DELIVER moves the links into the nav WITHOUT dropping any.
//! - `the_landing_now_sources_its_surface_links_from_the_persistent_nav`
//!   (NAV-INV-LandingViaNav, ADR-058 Migration): `GET /` now carries `id="viewer-nav"`
//!   — the landing's 8 surface links come FROM the persistent nav (the strengthening;
//!   the links now hold on ALL routes, not just `/`).
//!
//! Driving discipline (Mandate 1): every assertion enters through the REAL `openlore
//! ui` subprocess + HTTP — never internal `viewer-domain` render fns or the adapter
//! `Shape::from_request`. The local DuckDB is REAL (own claims via the real `claim add`
//! verb); no mocked boundary (the nav is a LOCAL render-only chrome affordance).
//!
//! Layer placement (Mandate 9/11): layer-3/layer-5 subprocess + real-I/O, EXAMPLE-only.
//! These guardrails are example-based, never PBT-generated at this layer (the `@property`
//! tag marks them as universal invariants for the reader + the DELIVER crafter; the
//! generative exploration of the pure `render_viewer_nav` / `page_shell` core over the
//! 8-surface × active-key space is a layer-1/2 DELIVER concern). Tier B (state-machine
//! PBT) is NOT warranted (Mandate 10 — a fixed 8-surface render, not a chained state
//! machine).
//!
//! Build-before-run note: as with `viewer_persistent_left_nav.rs`, the run MUST `cargo
//! build` the `openlore` (viewer) bin before running these ATs.
//!
//! Mandate 7 RED scaffolds: each body classifies RED for the RIGHT reason. The
//! NoControl / NoJs / SingleSource / LandingViaNav golds FAIL because the nav chrome is
//! MISSING (no `<nav id="viewer-nav">`, no `#viewer-main`) — RED (MISSING_FUNCTIONALITY),
//! NOT BROKEN. The Offline + LandingNoRegression golds PASS today (the page is already
//! offline/no-CDN and the landing hub already links all 8) — intentional GREEN-today
//! guardrails that must NOT regress when DELIVER moves the links into the persistent nav.
//
// SCAFFOLD: true

mod support;

#[allow(unused_imports)]
use support::*;

/// The persistent left-nav container id (ADR-058 D2).
const NAV_CONTAINER: &str = "id=\"viewer-nav\"";
/// The local vendored htmx asset route (slice-07; the offline foundation `hx-boost`
/// rides — no CDN, I-HX-2).
const HTMX_ASSET_PATH: &str = "/static/htmx.min.js";

/// The 8 viewer routes the nav must hold on (AC-001.1). All 200 today.
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

/// Extract the nav region — from the `id="viewer-nav"` anchor to the closing `</nav>`
/// — so the no-control scan is scoped to the NAV ONLY (the `/search` + `/scrape`
/// surfaces legitimately carry their own `<form>` in the CONTENT region, outside the
/// nav). `None` when the nav is absent (as TODAY — the no-control gold asserts the nav
/// is present FIRST, so a `None` surfaces as that presence RED, never a silent pass).
fn nav_region(body: &str) -> Option<&str> {
    let start = body.find(NAV_CONTAINER)?;
    let end = body[start..].find("</nav>").map(|e| start + e)?;
    Some(&body[start..end])
}

// =============================================================================
// I-VIEW-1/3 / D5 / AC-001.5 (CARDINAL) — the persistent nav adds no executable
// control (NAV-INV-NoControl). The nav is plain links only; the viewer holds no key.
// =============================================================================

/// NAV-INV-NoControl / GOLD `the_persistent_nav_adds_no_executable_control` (I-VIEW-1/3
/// / AC-001.5, CARDINAL): over a render carrying the nav, the NAV REGION carries ONLY
/// plain `<a href>` links — NO `<form>`, `<button>`, or mutating
/// `hx-post`/`hx-put`/`hx-delete` control. `hx-boost` issues GETs only; the viewer
/// holds no signing key. The scan is scoped to the nav region so the `/search`/`/scrape`
/// content-region forms do not create a false positive.
///
/// Given the viewer serves a surface carrying the persistent nav;
/// When the page renders (full page + boosted);
/// Then the nav region carries no executable / mutating control (plain links only).
///
/// @us-nav-001 @property @driving_port @real-io @read-only @no-control @gold
#[test]
fn the_persistent_nav_adds_no_executable_control() {
    let env = TestEnv::initialized();
    seed_own_claims_via_cli(&env, 2);
    let viewer = ViewerServer::start(&env);

    let full = viewer.get("/claims");
    let boosted = viewer.get_boosted("/claims");
    for (label, response) in [("full page", &full), ("boosted", &boosted)] {
        assert_eq!(
            response.status, 200,
            "NAV-INV-NoControl: /claims ({label}) must be 200; body:\n{}",
            response.body
        );
        // The nav is present (RED today — absent) and its region is control-free.
        let region = nav_region(&response.body).unwrap_or_else(|| {
            panic!(
                "NAV-INV-NoControl (AC-001.5): /claims ({label}) must render the \
                 persistent nav ({NAV_CONTAINER}) so the no-control scan runs over REAL \
                 nav content; body:\n{}",
                response.body
            )
        });
        let lowered = region.to_ascii_lowercase();
        for banned in ["<form", "<button", "hx-post", "hx-put", "hx-delete"] {
            assert!(
                !lowered.contains(banned),
                "NAV-INV-NoControl (I-VIEW-1/3, CARDINAL): the nav region ({label}) must \
                 carry NO executable/mutating control — found {banned:?}; nav region:\n{region}"
            );
        }
    }
}

// =============================================================================
// I-HX-2 / AC-001.5 — the nav stays offline: no external asset, the vendored htmx
// asset served locally (NAV-INV-Offline). GREEN-today guardrail.
// =============================================================================

/// NAV-INV-Offline / GOLD `the_persistent_nav_stays_offline_with_the_vendored_asset`
/// (I-HX-2 / AC-001.5): the nav references NO external (non-loopback) asset host, and
/// the `hx-boost`/`hx-select` capability the nav uses rides the ALREADY-vendored
/// `/static/htmx.min.js` (served locally with a JS content-type) — no new asset, no
/// CDN. A GREEN-today guardrail: the offline foundation `hx-boost` depends on already
/// holds; DELIVER must keep it holding.
///
/// Given the viewer serves an inner surface + the vendored htmx asset route;
/// When the surface and the asset are fetched;
/// Then the surface references no external CDN host, and the htmx asset serves locally
///   (200, JS content-type).
///
/// @us-nav-001 @property @driving_port @real-io @offline @no-cdn @gold
#[test]
fn the_persistent_nav_stays_offline_with_the_vendored_asset() {
    let env = TestEnv::initialized();
    seed_own_claims_via_cli(&env, 2);
    let viewer = ViewerServer::start(&env);

    // The surface references NO external CDN host (offline-first; GREEN today).
    let surface = viewer.get("/claims");
    assert_eq!(surface.status, 200, "NAV-INV-Offline: GET /claims must be 200; body:\n{}", surface.body);
    assert!(
        !surface.references_external_cdn(),
        "NAV-INV-Offline (I-HX-2): the surface must reference NO external CDN host (the \
         nav's hx-boost rides the vendored asset, not a CDN); body:\n{}",
        surface.body
    );

    // The vendored htmx asset serves LOCALLY with a JS content-type (the offline
    // foundation hx-boost/hx-select ride; GREEN today).
    let asset = viewer.get(HTMX_ASSET_PATH);
    assert_eq!(
        asset.status, 200,
        "NAV-INV-Offline (I-HX-2): the vendored htmx asset {HTMX_ASSET_PATH} must serve \
         locally (200); body:\n{}",
        asset.body
    );
    assert!(
        asset.content_type_looks_like_javascript(),
        "NAV-INV-Offline (I-HX-2): the vendored htmx asset must serve as JavaScript \
         (got content-type {:?}) so the browser executes the hx-boost machinery",
        asset.content_type
    );
}

// =============================================================================
// I-HX-1/4 / AC-001.4 — the nav renders with plain links on EVERY route with JS off
// (NAV-INV-NoJs / KPI-NAV-3).
// =============================================================================

/// NAV-INV-NoJs / GOLD `the_nav_renders_with_plain_links_on_every_route_with_js_off`
/// (I-HX-1/4 / AC-001.4 / KPI-NAV-3): on EVERY one of the 8 viewer routes, the no-JS
/// full-page render (a plain `get`, no `HX-Request`) carries the nav with all 8
/// surfaces as working plain `<a href>` links. Progressive enhancement holds across
/// the WHOLE surface set, not just one sampled route.
///
/// Given JavaScript is disabled;
/// When each of the 8 viewer routes is loaded as a plain full page;
/// Then every route renders the nav with all 8 surfaces as plain `<a href>` links.
///
/// @us-nav-001 @property @driving_port @real-io @progressive-enhancement @no-js @gold
#[test]
fn the_nav_renders_with_plain_links_on_every_route_with_js_off() {
    let env = TestEnv::initialized();
    seed_own_claims_via_cli(&env, 2);
    let viewer = ViewerServer::start(&env);

    for route in NAV_ROUTES {
        let response = viewer.get(route);
        assert_eq!(
            response.status, 200,
            "NAV-INV-NoJs: no-JS GET {route} must be 200; body:\n{}",
            response.body
        );
        assert!(
            response.is_full_page(),
            "NAV-INV-NoJs: no-JS GET {route} must be a COMPLETE full page; body:\n{}",
            response.body
        );
        // The nav is present (RED today) …
        assert!(
            response.body.contains(NAV_CONTAINER),
            "NAV-INV-NoJs (AC-001.4 / KPI-NAV-3): no-JS GET {route} must render the nav \
             ({NAV_CONTAINER}); body:\n{}",
            response.body
        );
        // …with every surface a WORKING plain `<a href>` link (no-JS navigable).
        assert_landing_links_all_surfaces(&response.body);
    }
}

// =============================================================================
// AC-001.3 — the nav item set never drifts from the single SSOT
// (NAV-INV-SingleSource).
// =============================================================================

/// NAV-INV-SingleSource / GOLD `the_nav_item_set_never_drifts_from_the_surface_ssot`
/// (AC-001.3 / D2 / KPI drift guard): the nav on an inner surface lists EXACTLY the
/// `LANDING_HUB_SURFACES` set (all 8 shipped surfaces) and NO deep/parameterized route
/// (`/claims/{cid}`, `?contributor=`, `?subject=`, `?object=`). One source of truth —
/// a surface absent from that table is absent from the nav, and vice-versa; there is
/// no second, driftable literal list.
///
/// Given the operator is on an inner surface;
/// When the nav renders;
/// Then it lists all 8 SSOT surfaces and links no deep/parameterized route.
///
/// @us-nav-001 @property @driving_port @real-io @single-source @gold
#[test]
fn the_nav_item_set_never_drifts_from_the_surface_ssot() {
    let env = TestEnv::initialized();
    seed_own_claims_via_cli(&env, 2);
    let viewer = ViewerServer::start(&env);

    let response = viewer.get("/peer-claims");
    assert_eq!(
        response.status, 200,
        "NAV-INV-SingleSource: GET /peer-claims must be 200; body:\n{}",
        response.body
    );
    // The nav is present (RED today).
    assert!(
        response.body.contains(NAV_CONTAINER),
        "NAV-INV-SingleSource (AC-001.3): GET /peer-claims must render the nav \
         ({NAV_CONTAINER}); body:\n{}",
        response.body
    );
    // THEN it lists ALL 8 SSOT surfaces (RED today — an inner surface carries none) …
    assert_landing_links_all_surfaces(&response.body);
    // …and NO deep/parameterized route appears as a top-level nav item.
    assert_landing_no_deep_route_toplevel(&response.body);
}

// =============================================================================
// ADR-058 Migration — the slice-17 landing hub is superseded by the persistent nav
// WITHOUT losing a link (NAV-INV-LandingNoRegression = GREEN-today; NAV-INV-LandingViaNav
// = RED strengthening).
// =============================================================================

/// NAV-INV-LandingNoRegression / GOLD `the_landing_still_links_all_eight_surfaces`
/// (ADR-058 Migration / KPI-NAV-4): the slice-17 landing-hub behavior is NOT lost —
/// `GET /` STILL links all 8 surfaces. A GREEN-today no-regression guardrail: when
/// DELIVER moves the surface links out of `render_landing`'s inline hub and into the
/// persistent `render_viewer_nav`, every link the landing offered must still be offered
/// (no coverage loss — the ADR calls this a strengthening, not a regression).
///
/// Given the viewer serves the landing page;
/// When `GET /` renders;
/// Then all 8 surfaces are still linked (via the persistent nav now).
///
/// @us-nav-001 @property @driving_port @real-io @no-regression @gold
#[test]
fn the_landing_still_links_all_eight_surfaces() {
    let env = TestEnv::initialized();
    seed_own_claims_via_cli(&env, 2);
    let viewer = ViewerServer::start(&env);

    let response = viewer.get("/");
    assert_eq!(response.status, 200, "NAV-INV-LandingNoRegression: GET / must be 200; body:\n{}", response.body);
    // The landing STILL links all 8 surfaces (GREEN today — the slice-17 inline hub;
    // after DELIVER, the SAME 8 via the persistent nav). No link is lost.
    assert_landing_links_all_surfaces(&response.body);
}

/// NAV-INV-LandingViaNav / GOLD
/// `the_landing_now_sources_its_surface_links_from_the_persistent_nav` (ADR-058
/// Migration — the strengthening): `GET /` now carries `id="viewer-nav"` — the
/// landing's 8 surface links come FROM the persistent nav (rendered once in
/// `page_shell`, on the landing AND every other surface), NOT a landing-only inline
/// hub. This pins that the migration actually happened: the landing is now served
/// through the SAME `page_shell` + `render_viewer_nav` path as every inner surface.
///
/// Given the viewer serves the landing page;
/// When `GET /` renders;
/// Then it carries the persistent nav container (the surface links come via the nav).
///
/// @us-nav-001 @us-nav-002 @property @driving_port @real-io @single-source @gold
#[test]
fn the_landing_now_sources_its_surface_links_from_the_persistent_nav() {
    let env = TestEnv::initialized();
    seed_own_claims_via_cli(&env, 2);
    let viewer = ViewerServer::start(&env);

    let response = viewer.get("/");
    assert_eq!(response.status, 200, "NAV-INV-LandingViaNav: GET / must be 200; body:\n{}", response.body);
    // The landing now carries the persistent nav container (RED today — the landing hub
    // is an inline `<a href>` list, NOT a `<nav id="viewer-nav">`). The migration to the
    // shared `page_shell` + `render_viewer_nav` path is what this gold pins.
    assert!(
        response.body.contains(NAV_CONTAINER),
        "NAV-INV-LandingViaNav (ADR-058 Migration): GET / must source its 8 surface \
         links from the persistent nav ({NAV_CONTAINER}), served through the shared \
         page_shell path; body:\n{}",
        response.body
    );
}
