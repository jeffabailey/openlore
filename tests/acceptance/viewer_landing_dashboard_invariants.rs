//! Slice-17 acceptance — landing-dashboard GOLD / guardrail invariants (the
//! cross-cutting invariants that must hold over the WHOLE `GET /` front-door surface,
//! beyond any single story). ADR-054.
//!
//! These are the load-bearing, release-relevant guardrail gold tests for the
//! landing-dashboard DELTA — the BEHAVIORAL layer of the three-layer enforcement (type
//! [`StoreReadPort` declares no mutation method] + xtask `check-arch`
//! [`check_viewer_capability_boundary`] are the other two, owned by DELIVER). They
//! drive the REAL `openlore ui` verb via the `ViewerServer` subprocess + in-test HTTP
//! GET over a REAL seeded LOCAL DuckDB, with NO mocked boundary (`/` is a LOCAL read +
//! PURE render — offline-STRONGER than `/search` [which mocks the indexer] and
//! `/scrape` [which reaches GitHub]; the front door has NO outbound edge at all). They
//! assert the hard slice-17 invariants on the OBSERVABLE surface:
//!
//! - `every_landing_render_leaves_the_store_read_only` (LD-INV-ReadOnly, C-1 / WD-LD-1
//!   / KPI-VIEW-2): exercising `GET /` over a seeded store leaves the `claims` +
//!   `peer_claims` row counts UNCHANGED, asserted via the universe-bound
//!   `assert_store_read_only` (Mandate 8; universe = the two port-exposed counts, all
//!   `unchanged`). The `LandingSummary` is computed per request and persists nothing
//!   (I-LD-6 / WD-LD-10 — zero new persisted types).
//! - `no_landing_response_adds_a_write_or_mutating_control` (LD-INV-NoWrite, C-1 /
//!   WD-LD-1, CARDINAL): no `GET /` response renders a write / compose / sign /
//!   subscribe / follow control — every navigation affordance is a plain `<a href>`
//!   link; the viewer holds no key.
//! - `the_landing_page_chrome_stays_offline_no_cdn` (LD-INV-OfflineChrome, C-2 /
//!   KPI-HX-G2): the `GET /` page references ONLY the LOCAL `/static/htmx.min.js`
//!   script src and NO off-host CDN.
//! - `the_landing_surface_works_fully_offline` (LD-INV-Offline, C-2 / KPI-5): the
//!   `GET /` view renders fully with the network unavailable — the store summary + the
//!   full nav hub (not just the chrome) are a LOCAL read with NO outbound edge to take
//!   down (no PDS fetch, no DID re-resolution, no peer pull, no network search —
//!   offline-STRONGER).
//! - `the_landing_summary_is_a_fixed_set_of_reads_invariant_to_store_size`
//!   (LD-INV-NoNPlus1, C-4 / WD-LD-4/5 / I-LD-7): a LARGE store renders the same three
//!   counts in the same FIXED set of aggregate reads — the N+1 behavioral proxy
//!   mirroring the slice-10/13/14/15 single-request multi-breakdown proxy (the strict
//!   3-fixed-reads bound is the DELIVER adapter-duckdb unit/property test).
//! - `missing_is_distinct_from_zero_on_the_front_door` (LD-INV-MissingNotZero, C-2 /
//!   WD-LD-8 / BR-LD-3, CARDINAL): a FAILED count read renders the missing-number
//!   marker "—", DISTINCT from a SUCCESSFUL read of 0 ("0 own claims" on an empty
//!   store) — a fabricated 0 on failure is forbidden (and unrepresentable, since the
//!   shell maps a failed read to `None`, never `Some(0)`).
//! - `the_front_door_links_all_eight_surfaces` (LD-INV-Discoverability, C-3 / WD-LD-7):
//!   over any populated front door, the nav hub links ALL 8 shipped top-level surfaces
//!   (the discoverability completeness guarantee — a missing surface is an unshippable
//!   navigation gap).
//!
//! These INHERIT the slice-06 viewer GOLD invariants (`viewer_is_read_only`,
//! `store_views_work_offline`, `web_process_holds_no_signing_key`) which already cover
//! the whole-viewer read-only / offline / no-key guarantees; the slice-17 gold tests
//! add the FRONT-DOOR-specific invariants (missing≠zero, discoverability completeness,
//! the 3-fixed-reads no-N+1 proxy) that the slice-06 corpus does not cover (the
//! slice-06 `/` was near-empty).
//!
//! Driving discipline (Mandate 1): every assertion enters through the REAL `openlore
//! ui` subprocess + HTTP — never internal `viewer-domain` `render_landing` / the count
//! reads. The local DuckDB is REAL (seeded via the production `claim add` + `peer add`
//! + `peer pull` path); there is NO mocked boundary (`/` is LOCAL).
//!
//! Layer placement (Mandate 9/11): every test here is a layer-3/layer-5 subprocess +
//! real-I/O test — EXAMPLE-only. The missing-number sad path is enumerated explicitly,
//! never PBT-generated at this layer (the generative exploration of the pure
//! `render_landing` over the 2³ Option combinations is a layer-1/2 DELIVER concern).
//!
//! Build-before-run note (carry into the DELIVER roadmap): `cargo test` does NOT
//! rebuild a spawned binary automatically — the roadmap/run MUST `cargo build` the
//! `openlore` bin before running these ATs so `ViewerServer::start` spawns the CURRENT
//! viewer.
//!
//! SCAFFOLD: true (slice-17) — the seeds + asserts COMPILE now (they drive EXISTING
//! verbs + scan strings); the SCENARIOS stay RED because the production `/` route is
//! storeless (renders no counts, links only `/claims`) and the
//! `LandingSummary`/`SCRAPE_URL`/`count_active_peer_subscriptions`/`MISSING_COUNT_MARKER`
//! seams do NOT exist yet. RED = MISSING_FUNCTIONALITY, never BROKEN. The
//! missing≠zero gold panics at the `start_inner` `todo!()` fault seam (also
//! MISSING_FUNCTIONALITY).
//
// SCAFFOLD: true

mod support;

#[allow(unused_imports)]
use support::*;

// =============================================================================
// LD-INV-ReadOnly — every_landing_render_leaves_the_store_read_only (C-1 / WD-LD-1;
// the load-bearing read-only gold test for the front door).
// =============================================================================

/// LD-INV-ReadOnly / GOLD `every_landing_render_leaves_the_store_read_only` (C-1 /
/// WD-LD-1 / KPI-VIEW-2; release-relevant): exercising `GET /` over a seeded store
/// leaves the persisted-store row counts (`claims.row_count` + `peer_claims.row_count`)
/// UNCHANGED, asserted via the universe-bound `assert_store_read_only` (Mandate 8;
/// universe = the two port-exposed counts, each `unchanged`). The `LandingSummary` is
/// computed per request and persists nothing (I-LD-6 / WD-LD-10).
///
/// Given a store seeded with claims + peers;
/// When GET / is exercised;
/// Then the persisted `claims` + `peer_claims` row counts are byte-unchanged.
///
/// @us-ld-001 @driving_port @real-io @read-only @c-1 @gold
#[test]
fn every_landing_render_leaves_the_store_read_only() {
    // GIVEN a store seeded with the known landing summary.
    let env = TestEnv::initialized();
    let _held = seed_landing_store_summary(&env);

    // Capture the read-only universe BEFORE exercising the front door (port-exposed
    // counts — the operator would inspect them after the viewer exits).
    let before = capture_store_row_count_universe(&env);

    // WHEN GET / is exercised (inside a scope so the viewer's exclusive DuckDB lock is
    // released on drop before the after-capture).
    {
        let viewer = ViewerServer::start(&env);
        let page = viewer.get(LANDING_PATH);
        assert_eq!(
            page.status, 200,
            "GET / must render so the read-only proof is over a REAL render"
        );
        // viewer drops here — the `openlore ui` process is killed, the lock released.
    }

    // THEN the persisted row counts are UNCHANGED (the structural read-only proof; any
    // change is an UNSHIPPABLE read-only breach, I-LD-6 / WD-LD-1).
    let after = capture_store_row_count_universe(&env);
    assert_store_read_only(&before, &after);
}

// =============================================================================
// LD-INV-NoWrite — no_landing_response_adds_a_write_or_mutating_control (C-1 /
// WD-LD-1, CARDINAL).
// =============================================================================

/// LD-INV-NoWrite / GOLD `no_landing_response_adds_a_write_or_mutating_control` (C-1 /
/// WD-LD-1, CARDINAL): no `GET /` response renders a write / compose / sign /
/// subscribe / follow control — every navigation affordance is a plain `<a href>`
/// link. The viewer holds no key (the no-key audit is structural — the slice-06
/// `web_process_holds_no_signing_key` gold + xtask check-arch).
///
/// Given a store seeded with claims + peers;
/// When the GET / response is inspected;
/// Then it carries no form/button/mutating control — every affordance is a plain link.
///
/// @us-ld-001 @driving_port @real-io @read-only @no-write @c-1 @cardinal @gold
#[test]
fn no_landing_response_adds_a_write_or_mutating_control() {
    // GIVEN a store seeded with the known landing summary (so the no-control scan is
    // over REAL rendered content).
    let env = TestEnv::initialized();
    let _held = seed_landing_store_summary(&env);

    // WHEN the GET / response is inspected.
    let viewer = ViewerServer::start(&env);
    let page = viewer.get(LANDING_PATH);

    // THEN the page renders (200) and carries NO write/compose/sign/subscribe/follow
    // control — every affordance is a plain <a href> (C-1 CARDINAL).
    assert_eq!(
        page.status, 200,
        "GET / must render (200) so the no-control scan is over REAL content"
    );
    assert_landing_read_only_no_control(&page.body);
}

// =============================================================================
// LD-INV-OfflineChrome — the_landing_page_chrome_stays_offline_no_cdn (C-2 /
// KPI-HX-G2).
// =============================================================================

/// LD-INV-OfflineChrome / GOLD `the_landing_page_chrome_stays_offline_no_cdn` (C-2 /
/// KPI-HX-G2): the `GET /` page references ONLY the LOCAL `/static/htmx.min.js` script
/// src and NO off-host CDN — the page CHROME stays offline-capable (and so does the
/// SUMMARY itself, since the three reads are LOCAL — even stronger than `/search`).
///
/// Given the viewer renders the GET / page;
/// When the page's script references are inspected;
/// Then the only htmx asset reference is the local /static/htmx.min.js — no CDN.
///
/// @us-ld-001 @driving_port @real-io @offline @no-cdn @c-2 @gold
#[test]
fn the_landing_page_chrome_stays_offline_no_cdn() {
    // GIVEN a seeded store + the viewer rendering the GET / page.
    let env = TestEnv::initialized();
    let _held = seed_landing_store_summary(&env);
    let viewer = ViewerServer::start(&env);
    let page = viewer.get(LANDING_PATH);

    // THEN the page references no off-host CDN (only the vendored local htmx asset).
    assert_eq!(page.status, 200, "GET / must render the full page");
    assert!(
        !page.references_external_cdn(),
        "the front door must reference only the vendored local /static/htmx.min.js — \
         no off-host CDN (C-2 / KPI-HX-G2); body was:\n{}",
        page.body
    );
}

// =============================================================================
// LD-INV-Offline — the_landing_surface_works_fully_offline (C-2 / KPI-5).
// =============================================================================

/// LD-INV-Offline / GOLD `the_landing_surface_works_fully_offline` (C-2 / KPI-5): the
/// `GET /` view renders fully with the network unavailable — the store summary + the
/// full nav hub (not just the chrome) are a LOCAL read with NO outbound edge to take
/// down (no PDS fetch, no DID re-resolution, no peer pull, no network search —
/// offline-STRONGER than `/search`).
///
/// Given a store seeded with claims + peers and no network reachability wired;
/// When she opens GET /;
/// Then the store summary + the full nav hub render (the LOCAL reads have no outbound
///   edge to fail).
///
/// @us-ld-001 @driving_port @real-io @offline @c-2 @gold
#[test]
fn the_landing_surface_works_fully_offline() {
    // GIVEN a seeded store. `ViewerServer::start` wires NO outbound seam (no GitHub, no
    // indexer) — the front door is LOCAL by construction, so an absent network is
    // exactly the operator's offline machine.
    let env = TestEnv::initialized();
    let _held = seed_landing_store_summary(&env);

    // WHEN she opens GET / offline.
    let viewer = ViewerServer::start(&env);
    let page = viewer.get(LANDING_PATH);

    // THEN the summary + the full hub render fully offline (the three LOCAL reads have
    // no outbound edge to take down).
    assert_eq!(page.status, 200, "GET / must render fully offline (200)");
    assert_landing_shows_count(&page.body, "own claims", LANDING_OWN_CLAIMS);
    assert_landing_shows_count(&page.body, "peer claims", LANDING_PEER_CLAIMS);
    assert_landing_shows_count(&page.body, "active peers", LANDING_ACTIVE_PEERS);
    assert_landing_links_all_surfaces(&page.body);
}

// =============================================================================
// LD-INV-NoNPlus1 — the_landing_summary_is_a_fixed_set_of_reads_invariant_to_store_
// size (C-4 / WD-LD-4/5 / I-LD-7; the N+1 behavioral proxy).
// =============================================================================

/// LD-INV-NoNPlus1 / GOLD `the_landing_summary_is_a_fixed_set_of_reads_invariant_to_
/// store_size` (C-4 / WD-LD-4/5 / I-LD-7): a LARGE store renders the same three counts
/// in the same FIXED set of aggregate reads — the read count does not grow with the
/// store size (no per-claim or per-peer loop). The behavioral proxy mirrors the
/// slice-10/13/14/15 N+1 proxies — a large store rendered correctly in ONE request
/// (the strict 3-fixed-reads bound is the DELIVER adapter-duckdb unit/property test).
///
/// Given Maria's store has a LARGE number of own claims;
/// When she opens GET /;
/// Then the summary renders the correct own-claims count in one request (no per-row
///   loop) — invariant to store size.
///
/// @us-ld-001 @property @driving_port @real-io @no-n-plus-1 @c-4 @gold
#[test]
fn the_landing_summary_is_a_fixed_set_of_reads_invariant_to_store_size() {
    // GIVEN a LARGE store: 120 own claims (10× the headline 12) + the seeded peers.
    // The render must resolve the three aggregate counts WITHOUT a per-row loop — the
    // count is a single aggregate read whose cost is invariant to store size.
    const LARGE_OWN_CLAIMS: usize = 120;
    let env = TestEnv::initialized();
    seed_own_claims_via_cli(&env, LARGE_OWN_CLAIMS);
    let _rachel = seed_active_subscription_for(&env, LANDING_PEER_RACHEL_DID, [7u8; 32]);
    let _tobias = seed_active_subscription_for(&env, LANDING_PEER_TOBIAS_DID, [9u8; 32]);

    // WHEN she opens GET / over the large store.
    let viewer = ViewerServer::start(&env);
    let page = viewer.get(LANDING_PATH);

    // THEN the own-claims count renders correctly (120) in ONE request — the aggregate
    // read returns the right total invariant to store size (a per-row loop would
    // miscount / be observably slow; the strict 1-query bound per count is a DELIVER
    // adapter-duckdb unit/property test). The active-peer count is 2.
    assert_eq!(
        page.status, 200,
        "the large store must render in one request"
    );
    assert_landing_shows_count(&page.body, "own claims", LARGE_OWN_CLAIMS);
    assert_landing_shows_count(&page.body, "active peers", 2);
}

// =============================================================================
// LD-INV-MissingNotZero — missing_is_distinct_from_zero_on_the_front_door (C-2 /
// WD-LD-8 / BR-LD-3, CARDINAL).
// =============================================================================

/// LD-INV-MissingNotZero / GOLD `missing_is_distinct_from_zero_on_the_front_door` (C-2
/// / WD-LD-8 / BR-LD-3, CARDINAL): a FAILED count read renders the missing-number
/// marker "—", DISTINCT from a SUCCESSFUL read of 0 ("0 own claims" on an empty store).
/// A fabricated 0 on failure is forbidden — and unrepresentable, since the effect shell
/// maps a failed read to `None` (`.ok()`), never `Some(0)`. This gold test exercises
/// BOTH sides of the distinction: the empty-store SUCCESSFUL zero (fully exercisable
/// today) AND the FAILED-read missing marker (via the test-only fault seam — see the
/// SEEDING-SEAM NOTE on `start_viewer_with_failing_peer_claims_count`).
///
/// Given a fresh empty store (a successful read of 0) and, separately, a store whose
///   peer-claims read fails;
/// When GET / is rendered for each;
/// Then the empty store shows "0" for peer claims (NOT the missing marker), while the
///   failed read shows the missing marker "—" (NOT a fabricated 0).
///
/// @us-ld-000 @us-ld-001 @driving_port @real-io @missing-not-zero @c-2 @cardinal
/// @infrastructure-failure @gold
#[test]
fn missing_is_distinct_from_zero_on_the_front_door() {
    // SIDE 1 — a fresh EMPTY store: the peer-claims read SUCCEEDS, returning 0. The
    // front door must render "0" for peer claims, NOT the missing marker (Some(0), an
    // honest empty store).
    {
        let env = TestEnv::initialized();
        seed_empty_store_for_landing(&env);
        let viewer = ViewerServer::start(&env);
        let page = viewer.get(LANDING_PATH);
        assert_eq!(
            page.status, 200,
            "the empty store must render a 200 front door"
        );
        assert_landing_shows_count(&page.body, "peer claims", 0);
        // Scan each COUNT position (`"— <label>"`), NOT the bare marker: the page chrome
        // title ("OpenLore — Viewer") legitimately carries the em-dash, so a bare-marker
        // scan would collide with the title rather than the count surface.
        for label in ["own claims", "peer claims", "active peers"] {
            let missing_count = format!("{LANDING_MISSING_COUNT_MARKER} {label}");
            assert!(
                !page.body_contains(&missing_count),
                "an empty store is a SUCCESSFUL read of 0 — it must NOT render the \
                 missing-number count {missing_count:?} (the success side of \
                 `0 ≠ missing`, WD-LD-8); body was:\n{}",
                page.body
            );
        }
    }

    // SIDE 2 — a store whose peer-claims read FAILS mid-request (own + active still
    // succeed). The front door must render the missing marker "—" for peer claims, NOT
    // a fabricated 0 (None, a failed read — the failure side of `0 ≠ missing`).
    {
        let env = TestEnv::initialized();
        let _held = seed_landing_store_summary(&env);
        let viewer = start_viewer_with_failing_peer_claims_count(&env);
        let page = viewer.get(LANDING_PATH);
        assert_eq!(
            page.status, 200,
            "a failed count read must still render a 200 front door (C-2 CARDINAL — \
             never a 5xx); got {}",
            page.status
        );
        assert_landing_count_missing(&page.body, "peer claims");
        // The OTHER two counts still render their numbers (the degrade is per-count).
        assert_landing_shows_count(&page.body, "own claims", LANDING_OWN_CLAIMS);
        assert_landing_shows_count(&page.body, "active peers", LANDING_ACTIVE_PEERS);
    }
}

// =============================================================================
// LD-INV-Discoverability — the_front_door_links_all_eight_surfaces (C-3 / WD-LD-7;
// navigation completeness).
// =============================================================================

/// LD-INV-Discoverability / GOLD `the_front_door_links_all_eight_surfaces` (C-3 /
/// WD-LD-7): over any populated front door, the nav hub links ALL 8 shipped top-level
/// surfaces (the discoverability completeness guarantee — a missing surface is an
/// unshippable navigation gap) AND no deep/parameterized route is a top-level link.
/// Closes the discoverability gap (today only /claims is reachable from /).
///
/// Given a seeded store + the viewer rendering the GET / hub;
/// When the hub links are inspected;
/// Then all 8 shipped top-level surfaces are linked via plain <a href>, and no
///   deep/parameterized route is a top-level link.
///
/// @us-ld-001 @driving_port @real-io @discoverability @c-3 @gold
#[test]
fn the_front_door_links_all_eight_surfaces() {
    // GIVEN a seeded store + the viewer rendering the GET / hub.
    let env = TestEnv::initialized();
    let _held = seed_landing_store_summary(&env);
    let viewer = ViewerServer::start(&env);
    let page = viewer.get(LANDING_PATH);

    // THEN all 8 shipped top-level surfaces are linked, and no deep/parameterized route
    // is a top-level affordance.
    assert_eq!(page.status, 200, "the landing hub must render");
    assert_landing_links_all_surfaces(&page.body);
    assert_landing_no_deep_route_toplevel(&page.body);
}
