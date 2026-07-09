//! Slice-27 acceptance — the `GET /philosophies` vocabulary-surface GOLD / guardrail
//! invariants (the cross-cutting I-VIEW-1/3 read-only + offline guardrails, plus the
//! slice-21 single-source nav invariant, that must hold over the WHOLE surface beyond any
//! single US-PV-006 assertion; ADR-059 §5 row 27). These are the load-bearing,
//! release-relevant guardrail gold tests for the slice-27 DELTA — the BEHAVIORAL layer of
//! enforcement (the pure-core `seeds()`→HTML unit/property tests + `xtask check-arch`'s
//! 21-member / no-new-crate check are the other two, owned by DELIVER).
//!
//! They drive the REAL `openlore ui` verb via the `ViewerServer` subprocess + in-test
//! HTTP (full page `get`) over a REAL DuckDB seeded via the REAL `claim add` verb, and
//! assert the hard slice-27 invariants on the OBSERVABLE rendered surface:
//!
//! - `the_philosophies_surface_adds_no_executable_control` (VP-INV-NoControl, I-VIEW-1/3,
//!   CARDINAL): the read-only vocabulary surface carries ONLY plain `<a href>` links — NO
//!   `<form>`, `<button>`, or mutating `hx-post`/`hx-put`/`hx-delete`. The viewer holds no
//!   signing key in the web process; minting a philosophy stays the slice-24 CLI action.
//! - `the_philosophies_surface_stays_offline_with_no_external_asset` (VP-INV-Offline,
//!   I-VIEW-3): the surface references NO external (CDN) asset host — the vocabulary is a
//!   PURE render over the embedded `seeds()` (no store read, no network), and the page
//!   rides only the already-vendored local htmx asset.
//! - `the_philosophies_nav_link_holds_on_every_route_from_one_ssot` (VP-INV-SingleSource,
//!   AC-006.2): once `/philosophies` is a `LANDING_HUB_SURFACES` entry, the persistent nav
//!   links it on EVERY viewer route (one source of truth — not a philosophies-only literal
//!   list). The nav item set never drifts from the single SSOT.
//!
//! Driving discipline (Mandate 1): every assertion enters through the REAL `openlore ui`
//! subprocess + HTTP — never internal `viewer-domain` render fns or `lexicon` resolvers.
//! The local DuckDB is REAL (own claims via the real `claim add` verb); the `/philosophies`
//! render is store-independent (a pure projection of the embedded vocabulary).
//!
//! Layer placement (Mandate 9/11): layer-3/layer-5 subprocess + real-I/O, EXAMPLE-only.
//! These guardrails are example-based, never PBT-generated at this layer (the `@property`
//! tag marks them as universal invariants for the reader + the DELIVER crafter; the
//! generative exploration of the pure `seeds()`→HTML projection over the whole embedded
//! vocabulary is a layer-1/2 DELIVER concern). Tier B (state-machine PBT) is NOT warranted
//! (Mandate 10 — a fixed-vocabulary render, not a chained state machine).
//!
//! Build-before-run note: as with `viewer_philosophies.rs`, the run MUST `cargo build
//! --bin openlore` before running these ATs.
//!
//! Mandate 7 RED scaffolds: each body classifies RED for the RIGHT reason. The NoControl /
//! Offline golds FAIL because the `/philosophies` surface is MISSING (a GET → terse 404,
//! so there is no read-only content to scan) — RED (MISSING_FUNCTIONALITY), NOT BROKEN.
//! The SingleSource gold FAILS because `LANDING_HUB_SURFACES` carries no Philosophies
//! entry yet, so no route's nav links `/philosophies` — RED. All stay RED until DELIVER.
//
// SCAFFOLD: true

mod support;

#[allow(unused_imports)]
use support::*;

/// The NEW read-only viewer vocabulary route (ADR-059 §5 row 27 — `GET /philosophies`).
const PHILOSOPHIES_URL: &str = "/philosophies";
/// The persistent-nav link to `/philosophies` (a `LANDING_HUB_SURFACES` entry once minted).
const PHILOSOPHIES_NAV_LINK: &str = "href=\"/philosophies\"";

/// The viewer routes the Philosophies nav link must hold on once `/philosophies` is a
/// `LANDING_HUB_SURFACES` entry (AC-006.2 — reachable from EVERY page). Every route here
/// renders a 200 full page with the persistent nav today (slice-21 SHIPPED); the
/// Philosophies link is ABSENT from every one of them today → RED. (`/philosophies` is
/// itself excluded here — it is a 404 today; VP-2 in the story file drives its 200 +
/// active marker.)
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

// =============================================================================
// I-VIEW-1/3 (CARDINAL) — the read-only vocabulary surface adds no executable control
// (VP-INV-NoControl). Plain links only; the viewer holds no key.
// =============================================================================

/// VP-INV-NoControl / GOLD `the_philosophies_surface_adds_no_executable_control`
/// (I-VIEW-1/3 / AC-006.1, CARDINAL): the whole `/philosophies` surface carries ONLY plain
/// `<a href>` links — NO `<form>`, `<button>`, or mutating `hx-post`/`hx-put`/`hx-delete`
/// control. The read-only viewer holds no signing key in the web process; minting a
/// philosophy stays the slice-24 `openlore philosophy add` CLI action. (Unlike `/search`
/// and `/scrape`, the philosophies surface carries no content-region form of its own, so a
/// whole-page scan is the correct scope.)
///
/// Given the viewer serves the philosophies surface;
/// When the page renders;
/// Then it carries no executable / mutating control (plain links only).
///
/// @us-pv-006 @property @driving_port @real-io @read-only @no-control @gold
#[test]
fn the_philosophies_surface_adds_no_executable_control() {
    let env = TestEnv::initialized();
    seed_own_claims_via_cli(&env, 2);
    let viewer = ViewerServer::start(&env);

    let response = viewer.get(PHILOSOPHIES_URL);
    // The surface must exist as a 200 read-only page (RED today — 404) before the scan
    // runs over REAL content (never a silent pass on an empty 404 body).
    assert_eq!(
        response.status, 200,
        "VP-INV-NoControl (AC-006.1): GET {PHILOSOPHIES_URL} must render the read-only \
         vocabulary page (200) so the no-control scan runs over real content; body:\n{}",
        response.body
    );
    let lowered = response.body.to_ascii_lowercase();
    for banned in ["<form", "<button", "hx-post", "hx-put", "hx-delete"] {
        assert!(
            !lowered.contains(banned),
            "VP-INV-NoControl (I-VIEW-1/3, CARDINAL): the read-only philosophies surface \
             must carry NO executable/mutating control — found {banned:?}; body:\n{}",
            response.body
        );
    }
}

// =============================================================================
// I-VIEW-3 — the vocabulary surface stays offline: no external asset host
// (VP-INV-Offline). A pure render over the embedded seeds — no store, no network.
// =============================================================================

/// VP-INV-Offline / GOLD `the_philosophies_surface_stays_offline_with_no_external_asset`
/// (I-VIEW-3 / AC-006.1): the `/philosophies` surface references NO external (CDN) asset
/// host — the vocabulary is a PURE render over the embedded `lexicon::philosophy::seeds()`
/// (no store read, no network dependency), and the page rides only the already-vendored
/// local htmx asset (never a CDN). Proves the surface is served offline, as ADR-059 §5 row
/// 27 requires ("Read-only, no authoring control, offline").
///
/// Given the viewer serves the philosophies surface;
/// When the page renders;
/// Then it references no external CDN host (offline-first).
///
/// @us-pv-006 @property @driving_port @real-io @offline @no-cdn @gold
#[test]
fn the_philosophies_surface_stays_offline_with_no_external_asset() {
    let env = TestEnv::initialized();
    seed_own_claims_via_cli(&env, 2);
    let viewer = ViewerServer::start(&env);

    let response = viewer.get(PHILOSOPHIES_URL);
    assert_eq!(
        response.status, 200,
        "VP-INV-Offline (AC-006.1): GET {PHILOSOPHIES_URL} must be a 200 surface; body:\n{}",
        response.body
    );
    // The surface references NO external CDN host — offline-first (I-VIEW-3). The
    // vocabulary render depends only on the embedded seeds + the vendored local asset.
    assert!(
        !response.references_external_cdn(),
        "VP-INV-Offline (I-VIEW-3): the philosophies surface must reference NO external \
         CDN host (a pure render over the embedded vocabulary, offline); body:\n{}",
        response.body
    );
}

// =============================================================================
// AC-006.2 — the Philosophies nav link is sourced from the single LANDING_HUB_SURFACES
// SSOT, so it holds on EVERY route (VP-INV-SingleSource).
// =============================================================================

/// VP-INV-SingleSource / GOLD `the_philosophies_nav_link_holds_on_every_route_from_one_ssot`
/// (AC-006.2 / slice-21 single-source): once `/philosophies` is a `LANDING_HUB_SURFACES`
/// entry, the persistent nav links it on EVERY one of the shipped viewer routes — a single
/// source of truth (not a philosophies-only literal). The nav item set never drifts: adding
/// the surface to the ONE SSOT makes it reachable from every page at once.
///
/// Given the operator loads each shipped viewer route;
/// When the nav renders;
/// Then every route's nav links `/philosophies`.
///
/// @us-pv-006 @property @driving_port @real-io @single-source @nav-reach @gold
#[test]
fn the_philosophies_nav_link_holds_on_every_route_from_one_ssot() {
    let env = TestEnv::initialized();
    seed_own_claims_via_cli(&env, 2);
    let viewer = ViewerServer::start(&env);

    for route in NAV_ROUTES {
        let response = viewer.get(route);
        assert_eq!(
            response.status, 200,
            "VP-INV-SingleSource: GET {route} must be 200; body:\n{}",
            response.body
        );
        // THEN the persistent nav on every route links `/philosophies` (RED today — the
        // SSOT has no Philosophies entry, so no route's nav carries the link).
        assert!(
            response.body.contains(PHILOSOPHIES_NAV_LINK),
            "VP-INV-SingleSource (AC-006.2): the persistent nav on {route} must link the \
             Philosophies surface ({PHILOSOPHIES_NAV_LINK}) — one LANDING_HUB_SURFACES \
             source, reachable from every page; body:\n{}",
            response.body
        );
    }
}
