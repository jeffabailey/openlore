//! Slice-18 acceptance — counter-aware-counts GOLD / guardrail invariants (the
//! cross-cutting invariants that must hold over the WHOLE countered-own-claims count
//! surface on `GET /` + `GET /claims`, beyond any single story). ADR-055.
//!
//! These are the load-bearing, release-relevant guardrail gold tests for the
//! counter-aware-counts DELTA — the BEHAVIORAL layer of the three-layer enforcement (type
//! [`StoreReadPort` declares no mutation method — the added `count_countered_own_claims`
//! returns `Result<usize, _>`] + xtask `check-arch` [`check_viewer_capability_boundary` +
//! the `no_cross_table_join_elides_author` SQL rule, GREEN by construction] are the other
//! two, owned by DELIVER). They drive the REAL `openlore ui` verb via the `ViewerServer`
//! subprocess + in-test HTTP GET over a REAL seeded LOCAL DuckDB, with NO mocked boundary
//! (the count is a LOCAL `COUNT(DISTINCT)` read + PURE render — offline-STRONGER than
//! `/search` / `/scrape`; the count has NO outbound edge at all). They assert the hard
//! slice-18 GOLD invariants on the OBSERVABLE surface (acceptance-criteria.md GOLD table):
//!
//! - `every_counter_aware_render_leaves_the_store_read_only` (CC-INV-ReadOnly, C-1 /
//!   WD-CC-1 / KPI-VIEW-2): exercising `GET /` + `GET /claims` over a seeded store leaves
//!   the `claims` + `peer_claims` row counts UNCHANGED, asserted via the universe-bound
//!   `assert_store_read_only` (Mandate 8; universe = the two port-exposed counts, all
//!   `unchanged`). The countered count is computed per request and persists nothing.
//! - `no_counter_aware_render_adds_a_write_or_mutating_control` (CC-INV-NoWrite, C-1 /
//!   WD-CC-1, CARDINAL): neither `GET /` nor `GET /claims` renders a write / compose /
//!   sign / subscribe / follow control — the countered count is render-only text, never a
//!   sort/filter/mutating control; the viewer holds no key.
//! - `the_counter_aware_chrome_stays_offline_no_cdn` (CC-INV-OfflineChrome, C-2 /
//!   KPI-HX-G2): both pages reference ONLY the LOCAL `/static/htmx.min.js` and NO CDN.
//! - `the_counter_aware_surfaces_render_fully_offline` (CC-INV-Offline, C-2 / KPI-5): both
//!   surfaces render the countered count fully network-down — a LOCAL read with NO
//!   outbound edge to take down.
//! - `the_countered_count_is_a_fixed_aggregate_read_invariant_to_store_size`
//!   (CC-INV-NoNPlus1, C-3 CARDINAL): a LARGE store renders the right countered count in
//!   the same FIXED set of aggregate reads (the landing read budget grows by at most one);
//!   the N+1 behavioral proxy (the strict 1-query bound is the DELIVER adapter-duckdb
//!   unit/property test).
//! - `missing_is_distinct_from_zero_for_the_countered_count` (CC-INV-MissingNotZero, C-2 /
//!   C-5 / WD-CC-6, CARDINAL): a FAILED countered-count read renders "(— countered)",
//!   DISTINCT from a SUCCESSFUL read of 0 ("(0 countered)" on a none-countered store) — a
//!   fabricated 0 on failure is forbidden (and unrepresentable, since the shell maps a
//!   failed read to `None`, never `Some(0)`).
//! - `a_claim_countered_by_two_peers_counts_once` (CC-INV-PresenceOnce, C-4 / BR-CC-1,
//!   CARDINAL): a claim countered by TWO distinct peers contributes ONCE to the count
//!   ("(1 countered)", never "(2 countered)") — the `COUNT(DISTINCT)` presence guarantee.
//! - `the_landing_and_claims_header_counts_are_consistent` (CC-INV-SingleSource, WD-CC-8):
//!   the landing "(N countered)" == the `/claims` header "(N countered)" for the same
//!   store (single source — the read method + render helper, not a cached value).
//! - `the_claims_list_is_byte_identical_to_the_no_header_count_baseline`
//!   (CC-INV-NoRegression, C-4 / WD-CC-9): the `/claims` list order/paging/count/confidence
//!   + the slice-12 per-row flags are byte-identical to the no-header-count baseline.
//!
//! These INHERIT the slice-06/17 viewer GOLD invariants (`viewer_is_read_only`,
//! `store_views_work_offline`, `web_process_holds_no_signing_key`, the slice-17 landing
//! golds) which cover the whole-viewer + front-door read-only / offline / no-key
//! guarantees; the slice-18 golds add the COUNTERED-COUNT-specific invariants (presence-
//! once, single-source landing==header, the no-regression `/claims` byte-identity, the
//! missing≠zero countered marker, the no-N+1 countered-read proxy).
//!
//! Driving discipline (Mandate 1): every assertion enters through the REAL `openlore ui`
//! subprocess + HTTP — never internal `viewer-domain` `render_landing` / `render_countered`
//! / the count read. The local DuckDB is REAL (seeded via the production `claim add` +
//! `peer add` + `peer pull` path); there is NO mocked boundary (the count is LOCAL).
//!
//! Layer placement (Mandate 9/11): every test here is a layer-3/layer-5 subprocess +
//! real-I/O test — EXAMPLE-only. The missing-marker sad path is enumerated explicitly,
//! never PBT-generated at this layer (the generative exploration of the pure
//! `render_countered` over the Some(0)/Some(n)/None cases is a layer-1/2 DELIVER concern).
//!
//! Build-before-run note (carry into the DELIVER roadmap): `cargo test` does NOT rebuild a
//! spawned binary automatically — the roadmap/run MUST `cargo build` the `openlore` bin
//! before running these ATs so `ViewerServer::start` spawns the CURRENT viewer.
//!
//! SCAFFOLD: true (slice-18) — the seeds + asserts COMPILE now (they drive EXISTING verbs
//! + scan strings + read the REAL store); the SCENARIOS stay RED because the production `/`
//! + `/claims` routes do NOT render the countered count yet and the
//! `count_countered_own_claims`/`render_countered`/4th-field/`/claims`-param seams do NOT
//! exist. RED = MISSING_FUNCTIONALITY, never BROKEN. The missing≠zero gold panics at the
//! `start_inner` `todo!()` fault seam (also MISSING_FUNCTIONALITY).
//
// SCAFFOLD: true

mod support;

#[allow(unused_imports)]
use support::*;

// =============================================================================
// CC-INV-ReadOnly — every_counter_aware_render_leaves_the_store_read_only (C-1 /
// WD-CC-1; the load-bearing read-only gold test for the counter-aware surfaces).
// =============================================================================

/// CC-INV-ReadOnly / GOLD `every_counter_aware_render_leaves_the_store_read_only` (C-1 /
/// WD-CC-1 / KPI-VIEW-2; release-relevant): exercising `GET /` + `GET /claims` over a
/// seeded counter-aware store leaves the persisted-store row counts
/// (`claims.row_count` + `peer_claims.row_count`) UNCHANGED, asserted via the
/// universe-bound `assert_store_read_only` (Mandate 8; universe = the two port-exposed
/// counts, each `unchanged`). The countered count is computed per request and persists
/// nothing.
///
/// Given a store seeded with own claims + peer counters;
/// When GET / and GET /claims are exercised;
/// Then the persisted `claims` + `peer_claims` row counts are byte-unchanged.
///
/// @us-cc-001 @us-cc-002 @driving_port @real-io @read-only @c-1 @gold
#[test]
fn every_counter_aware_render_leaves_the_store_read_only() {
    // GIVEN a store seeded with own claims + the peer counters that flag them.
    let env = TestEnv::initialized();
    let _held = seed_landing_store_with_countered_own_claims(&env);

    // Capture the read-only universe BEFORE exercising the counter-aware surfaces.
    let before = capture_store_row_count_universe(&env);

    // WHEN GET / and GET /claims are exercised (inside a scope so the viewer's exclusive
    // DuckDB lock is released on drop before the after-capture).
    {
        let viewer = ViewerServer::start(&env);
        let landing = viewer.get(LANDING_PATH);
        let claims = viewer.get(CLAIMS_LIST_PATH);
        assert_eq!(
            landing.status, 200,
            "GET / must render so the read-only proof is real"
        );
        assert_eq!(
            claims.status, 200,
            "GET /claims must render so the read-only proof is real"
        );
        // viewer drops here — the `openlore ui` process is killed, the lock released.
    }

    // THEN the persisted row counts are UNCHANGED (any change is an UNSHIPPABLE read-only
    // breach, WD-CC-1).
    let after = capture_store_row_count_universe(&env);
    assert_store_read_only(&before, &after);
}

// =============================================================================
// CC-INV-NoWrite — no_counter_aware_render_adds_a_write_or_mutating_control (C-1 /
// WD-CC-1, CARDINAL).
// =============================================================================

/// CC-INV-NoWrite / GOLD `no_counter_aware_render_adds_a_write_or_mutating_control` (C-1 /
/// WD-CC-1, CARDINAL): neither `GET /` nor `GET /claims` renders a write / compose / sign
/// / subscribe / follow control — the countered count is render-only text, never a
/// sort/filter/mutating control; the viewer holds no key (structural — slice-06
/// `web_process_holds_no_signing_key` + xtask check-arch).
///
/// Given a store seeded with own claims + peer counters;
/// When the GET / and GET /claims responses are inspected;
/// Then neither carries a write/sort/filter/mutating control — the count is render-only
///   text.
///
/// @us-cc-001 @us-cc-002 @driving_port @real-io @read-only @no-write @c-1 @cardinal @gold
#[test]
fn no_counter_aware_render_adds_a_write_or_mutating_control() {
    // GIVEN a seeded counter-aware store.
    let env = TestEnv::initialized();
    let _held = seed_landing_store_with_countered_own_claims(&env);

    // WHEN the GET / and GET /claims responses are inspected.
    let viewer = ViewerServer::start(&env);
    let landing = viewer.get(LANDING_PATH);
    let claims = viewer.get(CLAIMS_LIST_PATH);

    // THEN both render (200) and carry NO write/compose/sign/subscribe/follow control —
    // the countered count is render-only text (C-1 CARDINAL).
    assert_eq!(landing.status, 200, "GET / must render (200)");
    assert_eq!(claims.status, 200, "GET /claims must render (200)");
    assert_landing_read_only_no_control(&landing.body);
    assert_landing_read_only_no_control(&claims.body);
}

// =============================================================================
// CC-INV-OfflineChrome — the_counter_aware_chrome_stays_offline_no_cdn (C-2 /
// KPI-HX-G2).
// =============================================================================

/// CC-INV-OfflineChrome / GOLD `the_counter_aware_chrome_stays_offline_no_cdn` (C-2 /
/// KPI-HX-G2): both the `GET /` and `GET /claims` pages reference ONLY the LOCAL
/// `/static/htmx.min.js` and NO off-host CDN.
///
/// Given the viewer renders the GET / and GET /claims pages;
/// When the pages' script references are inspected;
/// Then the only htmx asset reference is the local /static/htmx.min.js — no CDN.
///
/// @us-cc-001 @us-cc-002 @driving_port @real-io @offline @no-cdn @c-2 @gold
#[test]
fn the_counter_aware_chrome_stays_offline_no_cdn() {
    // GIVEN a seeded counter-aware store.
    let env = TestEnv::initialized();
    let _held = seed_landing_store_with_countered_own_claims(&env);
    let viewer = ViewerServer::start(&env);
    let landing = viewer.get(LANDING_PATH);
    let claims = viewer.get(CLAIMS_LIST_PATH);

    // THEN neither page references an off-host CDN (only the vendored local htmx asset).
    assert_eq!(landing.status, 200, "GET / must render the full page");
    assert_eq!(claims.status, 200, "GET /claims must render the full page");
    assert!(
        !landing.references_external_cdn(),
        "the front door must reference only the vendored local /static/htmx.min.js — no \
         off-host CDN (C-2 / KPI-HX-G2); body was:\n{}",
        landing.body
    );
    assert!(
        !claims.references_external_cdn(),
        "the `/claims` page must reference only the vendored local /static/htmx.min.js — no \
         off-host CDN (C-2 / KPI-HX-G2); body was:\n{}",
        claims.body
    );
}

// =============================================================================
// CC-INV-Offline — the_counter_aware_surfaces_render_fully_offline (C-2 / KPI-5).
// =============================================================================

/// CC-INV-Offline / GOLD `the_counter_aware_surfaces_render_fully_offline` (C-2 / KPI-5):
/// both `GET /` and `GET /claims` render the countered count fully with the network
/// unavailable — the count is a LOCAL `COUNT(DISTINCT)` read with NO outbound edge to take
/// down (no PDS fetch, no DID re-resolution, no peer pull).
///
/// Given a store seeded with own claims + peer counters and no network reachability wired;
/// When she opens GET / and GET /claims;
/// Then the countered count renders on both surfaces (the LOCAL read has no outbound edge).
///
/// @us-cc-001 @us-cc-002 @driving_port @real-io @offline @c-2 @gold
#[test]
fn the_counter_aware_surfaces_render_fully_offline() {
    // GIVEN a seeded store. `ViewerServer::start` wires NO outbound seam — the count is
    // LOCAL by construction, so an absent network is exactly the operator's offline machine.
    let env = TestEnv::initialized();
    let _held = seed_landing_store_with_countered_own_claims(&env);

    // WHEN she opens GET / and GET /claims offline.
    let viewer = ViewerServer::start(&env);
    let landing = viewer.get(LANDING_PATH);
    let claims = viewer.get(CLAIMS_LIST_PATH);

    // THEN the countered count renders on both surfaces fully offline.
    assert_eq!(landing.status, 200, "GET / must render fully offline (200)");
    assert_eq!(
        claims.status, 200,
        "GET /claims must render fully offline (200)"
    );
    assert_landing_countered_count(&landing.body, COUNTERED_OWN_CLAIMS);
    assert_claims_header_countered_count(&claims.body, COUNTERED_OWN_CLAIMS);
}

// =============================================================================
// CC-INV-NoNPlus1 — the_countered_count_is_a_fixed_aggregate_read_invariant_to_store_
// size (C-3 CARDINAL; the N+1 behavioral proxy).
// =============================================================================

/// CC-INV-NoNPlus1 / GOLD `the_countered_count_is_a_fixed_aggregate_read_invariant_to_
/// store_size` (C-3 CARDINAL): a LARGE store renders the right countered count in the same
/// FIXED set of aggregate reads — the read count does not grow with store size (no
/// per-claim counter-presence loop). The behavioral proxy mirrors the slice-12/17 N+1
/// proxies — a large store rendered correctly in ONE request (the strict 1-query bound is
/// the DELIVER adapter-duckdb unit/property test).
///
/// Given Maria's store has a LARGE number of own claims with a known countered subset of 3;
/// When she opens GET /;
/// Then the countered count renders 3 in one request — invariant to store size (no
///   per-row loop).
///
/// @us-cc-000 @property @driving_port @real-io @no-n-plus-1 @c-3 @cardinal @gold
#[test]
fn the_countered_count_is_a_fixed_aggregate_read_invariant_to_store_size() {
    // GIVEN a LARGE store: the 3 named countered own claims + many MORE plain own claims.
    const LARGE_EXTRA_OWN_CLAIMS: usize = 200;
    let env = TestEnv::initialized();
    let _held = seed_landing_store_with_countered_own_claims(&env);
    seed_own_claims_via_cli(&env, LARGE_EXTRA_OWN_CLAIMS);
    let countered = read_countered_own_claims_count(&env);
    assert_eq!(
        countered, COUNTERED_OWN_CLAIMS,
        "the countered count must stay {COUNTERED_OWN_CLAIMS} over a large store (invariant \
         to store size); got {countered}"
    );

    // WHEN she opens GET / over the large store.
    let viewer = ViewerServer::start(&env);
    let page = viewer.get(LANDING_PATH);

    // THEN the countered count renders correctly (3) in ONE request — the aggregate read
    // returns the right total invariant to store size (a per-row loop would miscount / be
    // observably slow; the strict 1-query bound is a DELIVER adapter-duckdb unit test).
    assert_eq!(
        page.status, 200,
        "the large store must render in one request"
    );
    assert_landing_countered_count(&page.body, COUNTERED_OWN_CLAIMS);
}

// =============================================================================
// CC-INV-MissingNotZero — missing_is_distinct_from_zero_for_the_countered_count (C-2
// / C-5 / WD-CC-6, CARDINAL).
// =============================================================================

/// CC-INV-MissingNotZero / GOLD `missing_is_distinct_from_zero_for_the_countered_count`
/// (C-2 / C-5 / WD-CC-6, CARDINAL): a FAILED countered-count read renders "(— countered)",
/// DISTINCT from a SUCCESSFUL read of 0 ("(0 countered)" on a none-countered store). A
/// fabricated 0 on failure is forbidden — and unrepresentable, since the effect shell maps
/// a failed read to `None` (`.ok()`), never `Some(0)`. This gold test exercises BOTH sides
/// of the distinction: the none-countered SUCCESSFUL zero (fully exercisable today) AND the
/// FAILED-read missing marker (via the test-only fault seam — see the SEEDING-SEAM NOTE on
/// `start_viewer_with_failing_countered_count`).
///
/// Given a none-countered store (a successful read of 0) and, separately, a store whose
///   countered-count read fails;
/// When GET / is rendered for each;
/// Then the none-countered store shows "(0 countered)" (NOT the missing marker), while the
///   failed read shows "(— countered)" (NOT a fabricated "(0 countered)").
///
/// @us-cc-000 @us-cc-001 @driving_port @real-io @missing-not-zero @c-2 @c-5 @cardinal
/// @infrastructure-failure @gold
#[test]
fn missing_is_distinct_from_zero_for_the_countered_count() {
    // SIDE 1 — a none-countered store: the countered-count read SUCCEEDS, returning 0. The
    // landing must render "(0 countered)", NOT the missing marker (Some(0), honest zero).
    {
        let env = TestEnv::initialized();
        let _held = seed_landing_store_none_countered(&env);
        let viewer = ViewerServer::start(&env);
        let page = viewer.get(LANDING_PATH);
        assert_eq!(
            page.status, 200,
            "the none-countered store must render a 200 front door"
        );
        assert_landing_countered_count(&page.body, 0);
        assert!(
            !page.body_contains("(— countered)"),
            "a none-countered store is a SUCCESSFUL read of 0 — it must render \
             \"(0 countered)\", NOT the missing marker \"(— countered)\" (the success side \
             of `0 ≠ missing`, WD-CC-6); body was:\n{}",
            page.body
        );
    }

    // SIDE 2 — a store whose countered-count read FAILS mid-request (own + others still
    // succeed). The landing must render "(— countered)", NOT a fabricated "(0 countered)"
    // (None, a failed read — the failure side of `0 ≠ missing`).
    {
        let env = TestEnv::initialized();
        let _held = seed_landing_store_with_countered_own_claims(&env);
        let viewer = start_viewer_with_failing_countered_count(&env);
        let page = viewer.get(LANDING_PATH);
        assert_eq!(
            page.status, 200,
            "a failed countered-count read must still render a 200 front door (C-2 CARDINAL \
             — never a 5xx); got {}",
            page.status
        );
        assert_landing_countered_missing(&page.body);
        // The own-claims count STILL renders (the degrade is per-count, independent).
        assert_landing_shows_count(&page.body, "own claims", LANDING_OWN_CLAIMS);
    }
}

// =============================================================================
// CC-INV-PresenceOnce — a_claim_countered_by_two_peers_counts_once (C-4 / BR-CC-1,
// CARDINAL): the COUNT(DISTINCT) presence guarantee.
// =============================================================================

/// CC-INV-PresenceOnce / GOLD `a_claim_countered_by_two_peers_counts_once` (C-4 / BR-CC-1,
/// CARDINAL): a claim countered by TWO distinct peers contributes ONCE to the count — both
/// the landing and the `/claims` header render "(1 countered)", never "(2 countered)". The
/// `COUNT(DISTINCT)` presence guarantee (the de-duped UNION IN-set collapses the two
/// distinct-author counters of the SAME own CID to ONE membership).
///
/// Given Maria's only countered claim is countered by both Rachel and Tobias;
/// When she opens GET / and GET /claims;
/// Then both render "(1 countered)", never "(2 countered)".
///
/// @us-cc-000 @us-cc-001 @us-cc-002 @driving_port @real-io @presence-once @c-4 @br-cc-1
/// @cardinal @gold
#[test]
fn a_claim_countered_by_two_peers_counts_once() {
    // GIVEN one own claim countered by TWO distinct peers, the only countered claim.
    let env = TestEnv::initialized();
    let _held = seed_landing_store_one_own_claim_countered_twice(&env);

    // WHEN she opens GET / and GET /claims.
    let viewer = ViewerServer::start(&env);
    let landing = viewer.get(LANDING_PATH);
    let claims = viewer.get(CLAIMS_LIST_PATH);

    // THEN both surfaces render "(1 countered)", NEVER "(2 countered)" (presence-once).
    assert_eq!(landing.status, 200, "GET / must render");
    assert_eq!(claims.status, 200, "GET /claims must render");
    assert_landing_countered_count(&landing.body, 1);
    assert_claims_header_countered_count(&claims.body, 1);
    for (surface, body) in [("/", &landing.body), ("/claims", &claims.body)] {
        assert!(
            !body.contains("(2 countered)"),
            "a claim countered by TWO peers must count ONCE — {surface} must NOT show \
             \"(2 countered)\" (presence-once, COUNT(DISTINCT); C-4 / BR-CC-1); body \
             was:\n{body}"
        );
    }
}

// =============================================================================
// CC-INV-SingleSource — the_landing_and_claims_header_counts_are_consistent (WD-CC-8).
// =============================================================================

/// CC-INV-SingleSource / GOLD `the_landing_and_claims_header_counts_are_consistent`
/// (WD-CC-8 / R-CC-6): the landing "(N countered)" == the `/claims` header "(N countered)"
/// for the SAME store. Both surfaces resolve the count from the SAME
/// `count_countered_own_claims` read and render through the SAME `render_countered` helper
/// (single source — the read method + render helper, not a cached value), so the two
/// orientation surfaces cannot diverge.
///
/// Given a store seeded with own claims + peer counters;
/// When she opens GET / and GET /claims;
/// Then the landing "(N countered)" equals the `/claims` header "(N countered)".
///
/// @us-cc-002 @driving_port @real-io @single-source @wd-cc-8 @gold
#[test]
fn the_landing_and_claims_header_counts_are_consistent() {
    // GIVEN a seeded counter-aware store.
    let env = TestEnv::initialized();
    let _held = seed_landing_store_with_countered_own_claims(&env);

    // WHEN she opens GET / and GET /claims over the SAME store.
    let viewer = ViewerServer::start(&env);
    let landing = viewer.get(LANDING_PATH);
    let claims = viewer.get(CLAIMS_LIST_PATH);

    // THEN the landing "(N countered)" equals the `/claims` header "(N countered)" (single
    // source — WD-CC-8).
    assert_eq!(landing.status, 200, "GET / must render");
    assert_eq!(claims.status, 200, "GET /claims must render");
    assert_landing_and_claims_countered_consistent(&landing.body, &claims.body);
}

// =============================================================================
// CC-INV-NoRegression — the_claims_list_is_byte_identical_to_the_no_header_count_
// baseline (C-4 / WD-CC-9).
// =============================================================================

/// CC-INV-NoRegression / GOLD `the_claims_list_is_byte_identical_to_the_no_header_count_
/// baseline` (C-4 / WD-CC-9): the `/claims` list order (`composed_at DESC, cid`), page
/// boundaries, total count, and every row's verbatim confidence + the slice-12 per-row
/// flags are BYTE-IDENTICAL to the no-header-count baseline of the SAME store — the header
/// count is additive header text, never a re-order/filter/re-page/re-weight. REUSES the
/// slice-12 byte-identity assert (the header "(N countered)" lives OUTSIDE the row body, so
/// the row-order/confidence/position checks are unaffected by it).
///
/// Given a store with a mix of countered + un-countered own claims spanning the page;
/// When she opens GET /claims;
/// Then the list order/paging/count/confidence is byte-identical to the no-header-count
///   baseline.
///
/// @us-cc-002 @driving_port @real-io @additive @no-regression @c-4 @wd-cc-9 @gold
#[test]
fn the_claims_list_is_byte_identical_to_the_no_header_count_baseline() {
    // GIVEN a mix of countered + un-countered own claims (the slice-12 mixed-pages fixture).
    let env = TestEnv::initialized();
    let _seeded = seed_claims_list_mixed_pages(&env);

    // Record the slice-06 baseline (order + paging + total + each row's confidence) BEFORE
    // the header count is rendered (the no-regression anchor).
    let baseline = read_slice06_list_baseline(&env);

    // WHEN she opens GET /claims (the render now carries the additive header count).
    let viewer = ViewerServer::start(&env);
    let page = viewer.get(CLAIMS_LIST_PATH);

    // THEN the list order/paging/count/confidence is byte-identical to the baseline (the
    // header count + the slice-12 flag anchors elided) — additive, no regression (C-4 /
    // WD-CC-9).
    assert_eq!(page.status, 200, "the /claims page must render");
    assert_list_order_and_confidence_byte_identical(&page.body, &baseline);
}
