//! Slice-19 acceptance — peer-counter-aware-counts GOLD / guardrail invariants (the
//! cross-cutting invariants that must hold over the WHOLE countered-PEER-claims count
//! surface on `GET /` + `GET /peer-claims`, beyond any single story). ADR-056.
//!
//! These are the load-bearing, release-relevant guardrail gold tests for the
//! peer-counter-aware-counts DELTA — the BEHAVIORAL layer of the three-layer enforcement
//! (type [`StoreReadPort` declares no mutation method — the added
//! `count_countered_peer_claims` returns `Result<usize, _>`] + xtask `check-arch`
//! [`check_viewer_capability_boundary` + the `no_cross_table_join_elides_author` SQL rule,
//! GREEN by construction — R-PC-9 verified, `(peer=T, own=F)` → `None`] are the other two,
//! owned by DELIVER). They drive the REAL `openlore ui` verb via the `ViewerServer`
//! subprocess + in-test HTTP GET over a REAL seeded LOCAL DuckDB, with NO mocked boundary
//! (the count is a LOCAL `COUNT(DISTINCT)` read + PURE render — offline-STRONGER than
//! `/search` / `/scrape`; the count has NO outbound edge at all). They assert the hard
//! slice-19 GOLD invariants on the OBSERVABLE surface (acceptance-criteria.md GOLD table):
//!
//! - `every_peer_counter_aware_render_leaves_the_store_read_only` (PC-INV-ReadOnly, C-1 /
//!   WD-PC-1 / KPI-VIEW-2): exercising `GET /` + `GET /peer-claims` over a seeded store
//!   leaves the `claims` + `peer_claims` row counts UNCHANGED, asserted via the
//!   universe-bound `assert_store_read_only` (Mandate 8; universe = the two port-exposed
//!   counts, all `unchanged`). The countered-peer count is computed per request and
//!   persists nothing.
//! - `no_peer_counter_aware_render_adds_a_write_or_mutating_control` (PC-INV-NoWrite, C-1 /
//!   WD-PC-1, CARDINAL): neither `GET /` nor `GET /peer-claims` renders a write / compose /
//!   sign / subscribe / follow control — the countered count is render-only text, never a
//!   sort/filter/mutating control; the viewer holds no key.
//! - `the_peer_counter_aware_chrome_stays_offline_no_cdn` (PC-INV-OfflineChrome, C-2 /
//!   KPI-HX-G2): both pages reference ONLY the LOCAL `/static/htmx.min.js` and NO CDN.
//! - `the_peer_counter_aware_surfaces_render_fully_offline` (PC-INV-Offline, C-2 / KPI-5):
//!   both surfaces render the countered-peer count fully network-down — a LOCAL read with
//!   NO outbound edge to take down.
//! - `the_countered_peer_count_is_a_fixed_aggregate_read_invariant_to_store_size`
//!   (PC-INV-NoNPlus1, C-3 CARDINAL): a LARGE store renders the right countered-peer count
//!   in the same FIXED set of aggregate reads (the landing read budget grows by exactly one
//!   — a 5th count read); the N+1 behavioral proxy (the strict 1-query bound is the DELIVER
//!   adapter-duckdb unit/property test).
//! - `missing_is_distinct_from_zero_for_the_countered_peer_count` (PC-INV-MissingNotZero,
//!   C-2 / C-5 / WD-PC-6, CARDINAL): a FAILED countered-peer-count read renders
//!   "(— countered)", DISTINCT from a SUCCESSFUL read of 0 ("(0 countered)" on a
//!   none-countered store) — a fabricated 0 on failure is forbidden (and unrepresentable,
//!   since the shell maps a failed read to `None`, never `Some(0)`).
//! - `a_peer_claim_countered_by_two_counterers_counts_once` (PC-INV-PresenceOnce, C-4 /
//!   BR-PC-1, CARDINAL): a peer claim countered by TWO distinct counterers contributes ONCE
//!   to the count ("(1 countered)", never "(2 countered)") — the `COUNT(DISTINCT)` presence
//!   guarantee.
//! - `the_landing_and_peer_claims_header_counts_are_consistent` (PC-INV-SingleSource,
//!   WD-PC-8): the landing "(N countered)" == the `/peer-claims` header "(N countered)" for
//!   the same store (single source — the read method + render helper, not a cached value).
//! - `the_peer_claims_list_is_byte_identical_to_the_no_header_count_baseline`
//!   (PC-INV-NoRegression, C-4 / WD-PC-9): the `/peer-claims` list order/paging/count/
//!   confidence/origin + the slice-13 per-row flags are byte-identical to the
//!   no-header-count baseline.
//! - `the_slice_18_own_countered_surfaces_are_untouched` (PC-INV-OwnUntouched, BR-PC-4 /
//!   WD-PC-7): the slice-18 own-claims countered count (landing own line + `/claims` header)
//!   still renders "(N countered)" unchanged — this slice adds JUST the peer count, no
//!   third dimension, the slice-18 own surfaces are byte-untouched.
//!
//! These INHERIT the slice-06/07/17/18 viewer GOLD invariants (`viewer_is_read_only`,
//! `store_views_work_offline`, `web_process_holds_no_signing_key`, the slice-17 landing
//! golds, the slice-18 own-count golds) which cover the whole-viewer + front-door + own-
//! count read-only / offline / no-key guarantees; the slice-19 golds add the
//! COUNTERED-PEER-COUNT-specific invariants (presence-once, single-source landing==header,
//! the no-regression `/peer-claims` byte-identity, the missing≠zero countered marker, the
//! no-N+1 countered-peer-read proxy, the slice-18 own-surface-untouched no-regression).
//!
//! Driving discipline (Mandate 1): every assertion enters through the REAL `openlore ui`
//! subprocess + HTTP — never internal `viewer-domain` `render_landing` /
//! `render_peer_claims_page` / `render_countered` / the count read. The local DuckDB is
//! REAL (seeded via the production `peer add` + `peer pull` + `claim counter` path); there
//! is NO mocked boundary (the count is LOCAL).
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
//! SCAFFOLD: true (slice-19) — the seeds + asserts COMPILE now (they drive EXISTING verbs
//! + scan strings + read the REAL store); the SCENARIOS stay RED because the production `/`
//! + `/peer-claims` routes do NOT render the countered-peer count yet and the
//! `count_countered_peer_claims`/5th-field/`/peer-claims`-param seams do NOT exist. RED =
//! MISSING_FUNCTIONALITY, never BROKEN. The missing≠zero gold panics at the `start_inner`
//! `todo!()` fault seam (also MISSING_FUNCTIONALITY).
//
// SCAFFOLD: true

mod support;

#[allow(unused_imports)]
use support::*;

// =============================================================================
// PC-INV-ReadOnly — every_peer_counter_aware_render_leaves_the_store_read_only (C-1 /
// WD-PC-1; the load-bearing read-only gold test for the peer-counter-aware surfaces).
// =============================================================================

/// PC-INV-ReadOnly / GOLD `every_peer_counter_aware_render_leaves_the_store_read_only`
/// (C-1 / WD-PC-1 / KPI-VIEW-2; release-relevant): exercising `GET /` + `GET /peer-claims`
/// over a seeded counter-aware store leaves the persisted-store row counts
/// (`claims.row_count` + `peer_claims.row_count`) UNCHANGED, asserted via the
/// universe-bound `assert_store_read_only` (Mandate 8; universe = the two port-exposed
/// counts, each `unchanged`). The countered-peer count is computed per request and persists
/// nothing.
///
/// Given a store seeded with peer claims + their counters;
/// When GET / and GET /peer-claims are exercised;
/// Then the persisted `claims` + `peer_claims` row counts are byte-unchanged.
///
/// @us-pc-001 @us-pc-002 @driving_port @real-io @read-only @c-1 @gold
#[test]
fn every_peer_counter_aware_render_leaves_the_store_read_only() {
    // GIVEN a store seeded with cached peer claims + the counters that flag them.
    let env = TestEnv::initialized();
    let _held = seed_landing_store_with_countered_peer_claims(&env);

    // Capture the read-only universe BEFORE exercising the counter-aware surfaces.
    let before = capture_store_row_count_universe(&env);

    // WHEN GET / and GET /peer-claims are exercised (inside a scope so the viewer's
    // exclusive DuckDB lock is released on drop before the after-capture).
    {
        let viewer = ViewerServer::start(&env);
        let landing = viewer.get(LANDING_PATH);
        let peer_claims = viewer.get(PEER_CLAIMS_LIST_PATH);
        assert_eq!(
            landing.status, 200,
            "GET / must render so the read-only proof is real"
        );
        assert_eq!(
            peer_claims.status, 200,
            "GET /peer-claims must render so the read-only proof is real"
        );
        // viewer drops here — the `openlore ui` process is killed, the lock released.
    }

    // THEN the persisted row counts are UNCHANGED (any change is an UNSHIPPABLE read-only
    // breach, WD-PC-1).
    let after = capture_store_row_count_universe(&env);
    assert_store_read_only(&before, &after);
}

// =============================================================================
// PC-INV-NoWrite — no_peer_counter_aware_render_adds_a_write_or_mutating_control (C-1 /
// WD-PC-1, CARDINAL).
// =============================================================================

/// PC-INV-NoWrite / GOLD `no_peer_counter_aware_render_adds_a_write_or_mutating_control`
/// (C-1 / WD-PC-1, CARDINAL): neither `GET /` nor `GET /peer-claims` renders a write /
/// compose / sign / subscribe / follow control — the countered count is render-only text,
/// never a sort/filter/mutating control; the viewer holds no key (structural — slice-06
/// `web_process_holds_no_signing_key` + xtask check-arch).
///
/// Given a store seeded with peer claims + their counters;
/// When the GET / and GET /peer-claims responses are inspected;
/// Then neither carries a write/sort/filter/mutating control — the count is render-only
///   text.
///
/// @us-pc-001 @us-pc-002 @driving_port @real-io @read-only @no-write @c-1 @cardinal @gold
#[test]
fn no_peer_counter_aware_render_adds_a_write_or_mutating_control() {
    // GIVEN a seeded counter-aware peer store.
    let env = TestEnv::initialized();
    let _held = seed_landing_store_with_countered_peer_claims(&env);

    // WHEN the GET / and GET /peer-claims responses are inspected.
    let viewer = ViewerServer::start(&env);
    let landing = viewer.get(LANDING_PATH);
    let peer_claims = viewer.get(PEER_CLAIMS_LIST_PATH);

    // THEN both render (200) and carry NO write/compose/sign/subscribe/follow control —
    // the countered count is render-only text (C-1 CARDINAL).
    assert_eq!(landing.status, 200, "GET / must render (200)");
    assert_eq!(
        peer_claims.status, 200,
        "GET /peer-claims must render (200)"
    );
    assert_landing_read_only_no_control(&landing.body);
    assert_landing_read_only_no_control(&peer_claims.body);
}

// =============================================================================
// PC-INV-OfflineChrome — the_peer_counter_aware_chrome_stays_offline_no_cdn (C-2 /
// KPI-HX-G2).
// =============================================================================

/// PC-INV-OfflineChrome / GOLD `the_peer_counter_aware_chrome_stays_offline_no_cdn` (C-2 /
/// KPI-HX-G2): both the `GET /` and `GET /peer-claims` pages reference ONLY the LOCAL
/// `/static/htmx.min.js` and NO off-host CDN.
///
/// Given the viewer renders the GET / and GET /peer-claims pages;
/// When the pages' script references are inspected;
/// Then the only htmx asset reference is the local /static/htmx.min.js — no CDN.
///
/// @us-pc-001 @us-pc-002 @driving_port @real-io @offline @no-cdn @c-2 @gold
#[test]
fn the_peer_counter_aware_chrome_stays_offline_no_cdn() {
    // GIVEN a seeded counter-aware peer store.
    let env = TestEnv::initialized();
    let _held = seed_landing_store_with_countered_peer_claims(&env);
    let viewer = ViewerServer::start(&env);
    let landing = viewer.get(LANDING_PATH);
    let peer_claims = viewer.get(PEER_CLAIMS_LIST_PATH);

    // THEN neither page references an off-host CDN (only the vendored local htmx asset).
    assert_eq!(landing.status, 200, "GET / must render the full page");
    assert_eq!(
        peer_claims.status, 200,
        "GET /peer-claims must render the full page"
    );
    assert!(
        !landing.references_external_cdn(),
        "the front door must reference only the vendored local /static/htmx.min.js — no \
         off-host CDN (C-2 / KPI-HX-G2); body was:\n{}",
        landing.body
    );
    assert!(
        !peer_claims.references_external_cdn(),
        "the `/peer-claims` page must reference only the vendored local /static/htmx.min.js \
         — no off-host CDN (C-2 / KPI-HX-G2); body was:\n{}",
        peer_claims.body
    );
}

// =============================================================================
// PC-INV-Offline — the_peer_counter_aware_surfaces_render_fully_offline (C-2 / KPI-5).
// =============================================================================

/// PC-INV-Offline / GOLD `the_peer_counter_aware_surfaces_render_fully_offline` (C-2 /
/// KPI-5): both `GET /` and `GET /peer-claims` render the countered-peer count fully with
/// the network unavailable — the count is a LOCAL `COUNT(DISTINCT)` read with NO outbound
/// edge to take down (no PDS fetch, no DID re-resolution, no peer pull).
///
/// Given a store seeded with peer claims + their counters and no network reachability wired;
/// When she opens GET / and GET /peer-claims;
/// Then the countered-peer count renders on both surfaces (the LOCAL read has no outbound
///   edge).
///
/// @us-pc-001 @us-pc-002 @driving_port @real-io @offline @c-2 @gold
#[test]
fn the_peer_counter_aware_surfaces_render_fully_offline() {
    // GIVEN a seeded store. `ViewerServer::start` wires NO outbound seam — the count is
    // LOCAL by construction, so an absent network is exactly the operator's offline machine.
    let env = TestEnv::initialized();
    let _held = seed_landing_store_with_countered_peer_claims(&env);

    // WHEN she opens GET / and GET /peer-claims offline.
    let viewer = ViewerServer::start(&env);
    let landing = viewer.get(LANDING_PATH);
    let peer_claims = viewer.get(PEER_CLAIMS_LIST_PATH);

    // THEN the countered-peer count renders on both surfaces fully offline.
    assert_eq!(landing.status, 200, "GET / must render fully offline (200)");
    assert_eq!(
        peer_claims.status, 200,
        "GET /peer-claims must render fully offline (200)"
    );
    assert_landing_peer_countered_count(&landing.body, COUNTERED_PEER_CLAIMS);
    assert_peer_claims_header_countered_count(&peer_claims.body, COUNTERED_PEER_CLAIMS);
}

// =============================================================================
// PC-INV-NoNPlus1 — the_countered_peer_count_is_a_fixed_aggregate_read_invariant_to_
// store_size (C-3 CARDINAL; the N+1 behavioral proxy).
// =============================================================================

/// PC-INV-NoNPlus1 / GOLD `the_countered_peer_count_is_a_fixed_aggregate_read_invariant_to_
/// store_size` (C-3 CARDINAL): a LARGE store renders the right countered-peer count in the
/// same FIXED set of aggregate reads — the read count does not grow with store size (no
/// per-claim counter-presence loop). The behavioral proxy mirrors the slice-12/17/18 N+1
/// proxies — a large store rendered correctly in ONE request (the strict 1-query bound is
/// the DELIVER adapter-duckdb unit/property test).
///
/// Given Maria's store caches a LARGE number of peer claims with a known countered subset
///   of 1;
/// When she opens GET /;
/// Then the countered-peer count renders 1 in one request — invariant to store size (no
///   per-row loop).
///
/// @us-pc-000 @property @driving_port @real-io @no-n-plus-1 @c-3 @cardinal @gold
#[test]
fn the_countered_peer_count_is_a_fixed_aggregate_read_invariant_to_store_size() {
    // GIVEN a LARGE store: the seeded 1-countered peer store + many MORE plain peer claims.
    const LARGE_EXTRA_PEER_CLAIMS: usize = 200;
    let env = TestEnv::initialized();
    let _held = seed_landing_store_with_countered_peer_claims(&env);
    seed_extra_plain_peer_claims(&env, LARGE_EXTRA_PEER_CLAIMS);
    let countered = read_countered_peer_claims_count(&env);
    assert_eq!(
        countered, COUNTERED_PEER_CLAIMS,
        "the countered-peer count must stay {COUNTERED_PEER_CLAIMS} over a large store \
         (invariant to store size); got {countered}"
    );

    // WHEN she opens GET / over the large store.
    let viewer = ViewerServer::start(&env);
    let page = viewer.get(LANDING_PATH);

    // THEN the countered-peer count renders correctly (1) in ONE request — the aggregate
    // read returns the right total invariant to store size (a per-row loop would miscount /
    // be observably slow; the strict 1-query bound is a DELIVER adapter-duckdb unit test).
    assert_eq!(
        page.status, 200,
        "the large store must render in one request"
    );
    assert_landing_peer_countered_count(&page.body, COUNTERED_PEER_CLAIMS);
}

// =============================================================================
// PC-INV-MissingNotZero — missing_is_distinct_from_zero_for_the_countered_peer_count
// (C-2 / C-5 / WD-PC-6, CARDINAL).
// =============================================================================

/// PC-INV-MissingNotZero / GOLD `missing_is_distinct_from_zero_for_the_countered_peer_count`
/// (C-2 / C-5 / WD-PC-6, CARDINAL): a FAILED countered-peer-count read renders
/// "(— countered)", DISTINCT from a SUCCESSFUL read of 0 ("(0 countered)" on a
/// none-countered store). A fabricated 0 on failure is forbidden — and unrepresentable,
/// since the effect shell maps a failed read to `None` (`.ok()`), never `Some(0)`. This
/// gold test exercises BOTH sides of the distinction: the none-countered SUCCESSFUL zero
/// (fully exercisable today) AND the FAILED-read missing marker (via the 4th-token test-only
/// fault seam — see the SEEDING-SEAM NOTE on `start_viewer_with_failing_countered_peer_count`).
///
/// Given a none-countered peer store (a successful read of 0) and, separately, a store whose
///   countered-peer-count read fails;
/// When GET / is rendered for each;
/// Then the none-countered store shows "(0 countered)" (NOT the missing marker), while the
///   failed read shows "(— countered)" (NOT a fabricated "(0 countered)").
///
/// @us-pc-000 @us-pc-001 @driving_port @real-io @missing-not-zero @c-2 @c-5 @cardinal
/// @infrastructure-failure @gold
#[test]
fn missing_is_distinct_from_zero_for_the_countered_peer_count() {
    // SIDE 1 — a none-countered peer store: the countered-peer-count read SUCCEEDS,
    // returning 0. The landing must render "(0 countered)", NOT the missing marker (Some(0),
    // honest zero).
    {
        let env = TestEnv::initialized();
        let _held = seed_landing_store_no_peer_claim_countered(&env);
        let viewer = ViewerServer::start(&env);
        let page = viewer.get(LANDING_PATH);
        assert_eq!(
            page.status, 200,
            "the none-countered peer store must render a 200 front door"
        );
        assert_landing_peer_countered_count(&page.body, 0);
        assert!(
            !page.body_contains("(— countered)"),
            "a none-countered peer store is a SUCCESSFUL read of 0 — it must render \
             \"(0 countered)\", NOT the missing marker \"(— countered)\" (the success side \
             of `0 ≠ missing`, WD-PC-6); body was:\n{}",
            page.body
        );
    }

    // SIDE 2 — a store whose countered-peer-count read FAILS mid-request (peer-claims +
    // others still succeed). The landing must render "(— countered)", NOT a fabricated
    // "(0 countered)" (None, a failed read — the failure side of `0 ≠ missing`).
    {
        let env = TestEnv::initialized();
        let _held = seed_landing_store_with_countered_peer_claims(&env);
        let viewer = start_viewer_with_failing_countered_peer_count(&env);
        let page = viewer.get(LANDING_PATH);
        assert_eq!(
            page.status, 200,
            "a failed countered-peer-count read must still render a 200 front door (C-2 \
             CARDINAL — never a 5xx); got {}",
            page.status
        );
        assert_landing_peer_countered_missing(&page.body);
        // The peer-claims count STILL renders (the degrade is per-count, independent).
        assert_landing_shows_count(&page.body, "peer claims", LANDING_COUNTERED_PEER_TOTAL);
    }
}

// =============================================================================
// PC-INV-PresenceOnce — a_peer_claim_countered_by_two_counterers_counts_once (C-4 /
// BR-PC-1, CARDINAL): the COUNT(DISTINCT) presence guarantee.
// =============================================================================

/// PC-INV-PresenceOnce / GOLD `a_peer_claim_countered_by_two_counterers_counts_once` (C-4 /
/// BR-PC-1, CARDINAL): a peer claim countered by TWO distinct counterers contributes ONCE to
/// the count — both the landing and the `/peer-claims` header render "(1 countered)", never
/// "(2 countered)". The `COUNT(DISTINCT)` presence guarantee (the de-duped UNION IN-set
/// collapses the two distinct counterers of the SAME peer CID to ONE membership).
///
/// Given Maria's only countered peer claim is countered by both Maria and Rachel;
/// When she opens GET / and GET /peer-claims;
/// Then both render "(1 countered)", never "(2 countered)".
///
/// @us-pc-000 @us-pc-001 @us-pc-002 @driving_port @real-io @presence-once @c-4 @br-pc-1
/// @cardinal @gold
#[test]
fn a_peer_claim_countered_by_two_counterers_counts_once() {
    // GIVEN one cached peer claim countered by TWO distinct counterers, the only countered
    // peer claim.
    let env = TestEnv::initialized();
    let _held = seed_landing_store_one_peer_claim_countered_twice(&env);

    // WHEN she opens GET / and GET /peer-claims.
    let viewer = ViewerServer::start(&env);
    let landing = viewer.get(LANDING_PATH);
    let peer_claims = viewer.get(PEER_CLAIMS_LIST_PATH);

    // THEN both surfaces render "(1 countered)", NEVER "(2 countered)" (presence-once).
    assert_eq!(landing.status, 200, "GET / must render");
    assert_eq!(peer_claims.status, 200, "GET /peer-claims must render");
    assert_landing_peer_countered_count(&landing.body, 1);
    assert_peer_claims_header_countered_count(&peer_claims.body, 1);
    for (surface, body) in [("/", &landing.body), ("/peer-claims", &peer_claims.body)] {
        assert!(
            !body.contains("(2 countered)"),
            "a peer claim countered by TWO counterers must count ONCE — {surface} must NOT \
             show \"(2 countered)\" (presence-once, COUNT(DISTINCT); C-4 / BR-PC-1); body \
             was:\n{body}"
        );
    }
}

// =============================================================================
// PC-INV-SingleSource — the_landing_and_peer_claims_header_counts_are_consistent
// (WD-PC-8).
// =============================================================================

/// PC-INV-SingleSource / GOLD `the_landing_and_peer_claims_header_counts_are_consistent`
/// (WD-PC-8 / R-PC-6): the landing "(N countered)" == the `/peer-claims` header
/// "(N countered)" for the SAME store. Both surfaces resolve the count from the SAME
/// `count_countered_peer_claims` read and render through the SAME `render_countered` helper
/// (single source — the read method + render helper, not a cached value), so the two
/// orientation surfaces cannot diverge.
///
/// Given a store seeded with peer claims + their counters;
/// When she opens GET / and GET /peer-claims;
/// Then the landing "(N countered)" equals the `/peer-claims` header "(N countered)".
///
/// @us-pc-002 @driving_port @real-io @single-source @wd-pc-8 @gold
#[test]
fn the_landing_and_peer_claims_header_counts_are_consistent() {
    // GIVEN a seeded counter-aware peer store.
    let env = TestEnv::initialized();
    let _held = seed_landing_store_with_countered_peer_claims(&env);

    // WHEN she opens GET / and GET /peer-claims over the SAME store.
    let viewer = ViewerServer::start(&env);
    let landing = viewer.get(LANDING_PATH);
    let peer_claims = viewer.get(PEER_CLAIMS_LIST_PATH);

    // THEN the landing "(N countered)" equals the `/peer-claims` header "(N countered)"
    // (single source — WD-PC-8).
    assert_eq!(landing.status, 200, "GET / must render");
    assert_eq!(peer_claims.status, 200, "GET /peer-claims must render");
    assert_landing_and_peer_claims_countered_consistent(&landing.body, &peer_claims.body);
}

// =============================================================================
// PC-INV-NoRegression — the_peer_claims_list_is_byte_identical_to_the_no_header_count_
// baseline (C-4 / WD-PC-9).
// =============================================================================

/// PC-INV-NoRegression / GOLD `the_peer_claims_list_is_byte_identical_to_the_no_header_count
/// _baseline` (C-4 / WD-PC-9): the `/peer-claims` list order (`composed_at DESC, cid`), page
/// boundaries, total count, every row's verbatim confidence + peer origin + the slice-13
/// per-row flags are BYTE-IDENTICAL to the no-header-count baseline of the SAME store — the
/// header count is additive header text, never a re-order/filter/re-page/re-weight. REUSES
/// the slice-13 byte-identity assert (the header "(N countered)" lives OUTSIDE the row body,
/// and the slice-13 flag anchors elide cleanly, so the row-order/confidence/position checks
/// are unaffected by it).
///
/// Given a store caching a mix of countered + un-countered peer claims spanning the page;
/// When she opens GET /peer-claims;
/// Then the list order/paging/count/confidence/origin + slice-13 flags is byte-identical to
///   the no-header-count baseline.
///
/// @us-pc-002 @driving_port @real-io @additive @no-regression @c-4 @wd-pc-9 @gold
#[test]
fn the_peer_claims_list_is_byte_identical_to_the_no_header_count_baseline() {
    // GIVEN a mix of countered + un-countered peer claims (the slice-13
    // `seed_peer_claims_one_countered` fixture, whose ordered_cids pin the slice-06/07
    // render order).
    let env = TestEnv::initialized();
    let seeded = seed_peer_claims_one_countered(&env);

    // WHEN she opens GET /peer-claims (the render now carries the additive header count).
    let viewer = ViewerServer::start(&env);
    let page = viewer.get(PEER_CLAIMS_LIST_PATH);

    // THEN the list order/paging/count/confidence/origin is byte-identical to the baseline
    // (the header count + the slice-13 flag anchors elided) — additive, no regression (C-4 /
    // WD-PC-9).
    assert_eq!(page.status, 200, "the /peer-claims page must render");
    assert_peer_claims_order_byte_identical(&page.body, &seeded.ordered_cids);
}

// =============================================================================
// PC-INV-OwnUntouched — the_slice_18_own_countered_surfaces_are_untouched (BR-PC-4 /
// WD-PC-7): this slice adds JUST the peer count; the slice-18 own surfaces stay
// byte-identical.
// =============================================================================

/// PC-INV-OwnUntouched / GOLD `the_slice_18_own_countered_surfaces_are_untouched` (BR-PC-4 /
/// WD-PC-7): the slice-18 own-claims countered count (the landing own line "12 own claims
/// (3 countered)" + the `/claims` header "(3 countered)") still renders UNCHANGED beside the
/// new peer count — this slice adds JUST the peer count, no third dimension, the slice-18 own
/// surfaces are byte-untouched. Guards the scope-creep / re-touching-the-own-count risk
/// (R-PC-8).
///
/// Given a store seeded with peer claims + their counters AND the slice-18 own 12+3
///   countered;
/// When she opens GET / and GET /claims;
/// Then the slice-18 own line "12 own claims (3 countered)" renders on the landing AND the
///   `/claims` header still shows "(3 countered)" — both untouched by the new peer count.
///
/// @us-pc-001 @driving_port @real-io @no-regression @br-pc-4 @wd-pc-7 @gold
#[test]
fn the_slice_18_own_countered_surfaces_are_untouched() {
    // GIVEN a store seeded with both the new peer counters (4 peer, 1 countered) AND the
    // slice-18 own shape (12 own, 3 countered).
    let env = TestEnv::initialized();
    let _held = seed_landing_store_with_countered_peer_and_own(&env);

    // WHEN she opens GET / and GET /claims.
    let viewer = ViewerServer::start(&env);
    let landing = viewer.get(LANDING_PATH);
    let claims = viewer.get(CLAIMS_LIST_PATH);

    // THEN the slice-18 own line "12 own claims (3 countered)" renders UNTOUCHED on the
    // landing AND the `/claims` header still shows the slice-18 own "(3 countered)" — both
    // byte-untouched by the new peer count (WD-PC-7 / BR-PC-4).
    assert_eq!(landing.status, 200, "GET / must render");
    assert_eq!(claims.status, 200, "GET /claims must render");
    assert_landing_own_line_untouched(&landing.body);
    assert_claims_header_countered_count(&claims.body, COUNTERED_OWN_CLAIMS);
}
