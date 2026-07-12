//! Slice-15 acceptance — peer-subscriptions GOLD / guardrail invariants (the
//! cross-cutting I-PS-1/2/4/5/8 guardrails that must hold over the WHOLE `GET /peers`
//! surface, beyond any single story).
//!
//! These are the load-bearing, release-relevant guardrail gold tests for the
//! peer-subscriptions DELTA — the BEHAVIORAL layer of the three-layer enforcement (type +
//! xtask `check-arch` are the other two, owned by DELIVER). They drive the REAL `openlore
//! ui` verb via the `ViewerServer` subprocess + in-test HTTP (with/without `HX-Request`)
//! over a REAL seeded LOCAL DuckDB, with NO mocked boundary (`/peers` is a LOCAL read +
//! PURE projection/render — distinct from `/search`, which mocks the indexer; and
//! OFFLINE-STRONGER than `/search` — `/peers` has NO outbound edge at all). They assert the
//! hard slice-15 invariants on the OBSERVABLE surface:
//!
//! - `every_peers_render_leaves_the_store_read_only` (PS-INV-ReadOnly, I-PS-1 / WD-PS-1 /
//!   KPI-VIEW-2): exercising `/peers` across postures (populated / empty) AND both shapes
//!   (full page + htmx fragment) leaves `claims` + `peer_claims` row counts UNCHANGED,
//!   asserted via the universe-bound `assert_store_read_only` (Mandate 8; universe = the
//!   two port-exposed counts, all `unchanged`). The subscription list is computed per
//!   request and persists nothing (I-PS-6 — zero new persisted types).
//! - `no_peers_response_adds_a_write_or_subscribe_control` (PS-INV-NoWrite, I-PS-1 /
//!   WD-PS-1, CARDINAL): no `/peers` response shape (full page or fragment) renders a
//!   write / subscribe / unsubscribe / remove / purge control — the revocation is render-
//!   only `openlore peer remove <did>` command TEXT only; every reference is non-executable.
//! - `the_peers_page_chrome_stays_offline_no_cdn` (PS-INV-OfflineChrome, I-PS-4 /
//!   KPI-HX-G2): the `/peers` full page references ONLY the LOCAL `/static/htmx.min.js`
//!   script src and NO off-host CDN.
//! - `the_peers_surface_works_fully_offline` (PS-INV-Offline, I-PS-4 / KPI-5): the `/peers`
//!   view renders fully with the network unavailable — the subscription list + per-peer
//!   counts (not just the chrome) are a LOCAL read with NO outbound edge to take down
//!   (no PDS fetch, no DID re-resolution, no peer pull — offline-STRONGER, I-PS-4).
//! - `a_large_active_set_resolves_per_peer_counts_in_one_request` (PS-INV-NoNPlus1, I-PS-8 /
//!   DD-PS-1): a MULTI-peer active set with KNOWN distinct per-peer counts is rendered
//!   correctly in ONE request — the N+1 behavioral proxy mirroring the slice-10/13/14
//!   single-request multi-breakdown proxy (the strict 1-query bound is the DELIVER
//!   adapter-duckdb unit/property test).
//!
//! Driving discipline (Mandate 1): every assertion enters through the REAL `openlore ui`
//! subprocess + HTTP — never internal `viewer-domain` `render_peers_*` / the read method.
//! The local DuckDB is REAL (seeded via the production `peer add` + `peer pull` +
//! `peer remove` path); there is NO mocked boundary (`/peers` is LOCAL).
//!
//! Layer placement (Mandate 9/11): layer-3/layer-5 subprocess + real-I/O, EXAMPLE-only.
//! These guardrails are example-based, never PBT-generated at this layer (the `@property`
//! tag marks them as universal invariants for the reader + the DELIVER crafter; the strict
//! 1-query bound + the pure projection exploration are a layer-1/2 DELIVER concern, out of
//! this file's scope).
//!
//! Build-before-run note: as with `viewer_peer_subscriptions.rs`, the run MUST `cargo
//! build` the `openlore` (viewer) bin before running these ATs. No second binary is needed
//! — `/peers` is a LOCAL read.
//!
//! Mandate 7 RED scaffolds: each body runs to a `/peers` HTTP assertion that FAILS because
//! the production `/peers` route + `list_active_peer_subscriptions` read + `PeersView` /
//! `render_peers_*` seams do NOT exist yet (the route 404s) → classifies RED
//! (MISSING_FUNCTIONALITY), NOT BROKEN. They stay RED until DELIVER.
//!
//! Covers: the cross-cutting I-PS-1 / I-PS-2 / I-PS-4 / I-PS-5 / I-PS-8 guardrails over the
//! whole `/peers` surface (the gold companions to the US-PS-002/003 story scenarios in
//! `viewer_peer_subscriptions.rs`).
//
// SCAFFOLD: true

mod support;

#[allow(unused_imports)]
use support::*;

// =============================================================================
// I-PS-1 / WD-PS-1 — read-only preserved: every /peers posture + shape leaves the store
// unchanged (PS-INV-ReadOnly). The subscription list is computed per request and persists
// nothing.
// =============================================================================

/// PS-INV-ReadOnly / GOLD `every_peers_render_leaves_the_store_read_only` (I-PS-1 /
/// WD-PS-1 / KPI-VIEW-2): exercising `/peers` across postures (populated AND empty) in BOTH
/// shapes (full page + htmx fragment) leaves the `claims` + `peer_claims` row counts
/// UNCHANGED. The peer-subscriptions companion to the slice-06 `viewer_is_read_only` +
/// slice-08/10 read-only gold tests, asserted via the universe-bound state-delta (Mandate
/// 8: universe = the two port-exposed counts, each `unchanged`). The subscription list is
/// recomputed per request and NEVER persisted (I-PS-6 — zero new persisted types).
///
/// Given a store seeded with two active peers;
/// When `/peers` (populated AND empty postures, full + fragment) is exercised;
/// Then the `claims` and `peer_claims` row counts are UNCHANGED.
///
/// @us-ps-002 @us-ps-003 @property @driving_port @real-io @read-only @i-ps-1 @gold
#[test]
fn every_peers_render_leaves_the_store_read_only() {
    // GIVEN a REAL store seeded (production federation path) with two active peers so the
    // read-only delta is over a NON-TRIVIAL universe (a `0 == 0` delta would not prove the
    // viewer leaves a POPULATED store untouched). Capture the read-only universe (port-
    // exposed counts: `claims.row_count`, `peer_claims.row_count`) BEFORE exercising
    // `/peers`.
    // WHEN `/peers` is exercised in both shapes (the populated active set) within a scope so
    // the viewer's exclusive DuckDB lock is RELEASED before the `after` snapshot (the
    // read-only proof is about what the viewer LEFT BEHIND, mirroring slice-06/08/10).
    // THEN the persisted-store row counts are UNCHANGED (assert_store_read_only; any change
    // is an UNSHIPPABLE write-surface breach — I-PS-1 / WD-PS-1).
    let env = TestEnv::initialized();
    seed_peers_two_active_with_claims(&env);

    // Capture the read-only universe BEFORE any /peers render runs (Mandate 8: the universe
    // is the inherited capture, NOT internal struct fields).
    let before = capture_store_row_count_universe(&env);

    // Exercise /peers in BOTH shapes inside a scope so the viewer's exclusive DuckDB lock
    // is RELEASED (on drop) BEFORE the `after` snapshot re-opens the store.
    {
        let viewer = ViewerServer::start(&env);

        let full = viewer.get(PEERS_PATH);
        assert_eq!(
            full.status, 200,
            "GET /peers (full page) over the LOCAL store must be 200; body:\n{}",
            full.body
        );
        let fragment = viewer.get_htmx(PEERS_PATH);
        assert_eq!(
            fragment.status, 200,
            "GET /peers (htmx fragment) over the LOCAL store must be 200; body:\n{}",
            fragment.body
        );
        // `viewer` drops here — the `openlore ui` process is killed and its exclusive
        // DuckDB lock released before the `after` snapshot.
    }

    // Capture the read-only universe AFTER /peers ran.
    let after = capture_store_row_count_universe(&env);

    // The persisted-store row counts are UNCHANGED — every universe slot `unchanged` (any
    // change is an UNSHIPPABLE write-surface breach; I-PS-1 / WD-PS-1). The subscription
    // list was recomputed per request and persisted nothing.
    assert_store_read_only(&before, &after);
}

// =============================================================================
// I-PS-1 / WD-PS-1 (CARDINAL) — no write/subscribe/unsubscribe control on ANY /peers
// response shape (PS-INV-NoWrite). The subscribe/unsubscribe gate stays in the slice-03
// CLI; the revocation command is render-only TEXT.
// =============================================================================

/// PS-INV-NoWrite / GOLD `no_peers_response_adds_a_write_or_subscribe_control` (I-PS-1 /
/// WD-PS-1, CARDINAL): NO `/peers` response shape (full page or fragment) renders a write /
/// subscribe / unsubscribe / remove / purge control — `/peers` is a read, and the only
/// revocation affordance is the render-only `openlore peer remove <did>` command TEXT (an
/// `<a href>`-free `<p>`/`<code>`), never an executable control. Asserted on the observable
/// rendered surface across every shape.
///
/// Given the viewer renders the /peers list;
/// When every /peers response shape (full page + fragment) is inspected;
/// Then none renders a write / subscribe / unsubscribe / remove / purge control, and the
///   revocation command is render-only TEXT.
///
/// @us-ps-002 @property @driving_port @real-io @read-only @no-write @i-ps-1 @gold
#[test]
fn no_peers_response_adds_a_write_or_subscribe_control() {
    // GIVEN a store seeded with two active peers + the viewer rendering /peers in BOTH
    // shapes.
    // WHEN each shape (get full page + get_htmx fragment) is inspected.
    // THEN none carries a write/subscribe/unsubscribe/remove/purge affordance
    // (assert_peers_no_write_or_subscribe_control over EVERY shape; I-PS-1 / WD-PS-1), AND
    // the only revocation affordance present is the render-only `openlore peer remove <did>`
    // command TEXT (assert_peer_remove_command_is_render_only). The viewer holds no key (the
    // no-key audit is structural — xtask check-arch).
    let env = TestEnv::initialized();
    seed_peers_two_active_with_claims(&env);

    // Collect BOTH /peers response shapes inside a scope so the viewer's exclusive DuckDB
    // lock is released on drop (mirrors the slice-08/10 no-write collection discipline).
    let mut responses = Vec::new();
    {
        let viewer = ViewerServer::start(&env);
        responses.push(("GET /peers (full page)".to_string(), viewer.get(PEERS_PATH)));
        responses.push((
            "GET /peers (htmx fragment)".to_string(),
            viewer.get_htmx(PEERS_PATH),
        ));
        // `viewer` drops here — the `openlore ui` process is killed.
    }

    for (label, r) in &responses {
        // Each /peers shape renders successfully (200) so the no-control assertion is over
        // REAL rendered content, not an error page.
        assert_eq!(
            r.status, 200,
            "/peers shape {label:?} over the LOCAL store must render successfully (200) so \
             the no-control scan is over REAL content; got {} body:\n{}",
            r.status, r.body
        );

        // (a) NO write / subscribe / unsubscribe / remove / purge control on ANY shape
        // (I-PS-1 / WD-PS-1, CARDINAL). Any hit is an UNSHIPPABLE write-surface breach.
        assert_peers_no_write_or_subscribe_control(&r.body);

        // (b) The only revocation affordance is the render-only `openlore peer remove <did>`
        // command TEXT — render-only, never an executable control (I-PS-1 / I-PS-8). Both
        // active peers' commands are present as TEXT only.
        assert_peer_remove_command_is_render_only(&r.body, PEERS_RACHEL_DID);
        assert_peer_remove_command_is_render_only(&r.body, PEERS_TOBIAS_DID);
    }
}

// =============================================================================
// I-PS-4 / KPI-HX-G2 — offline chrome: the /peers page references only the local vendored
// htmx asset, no CDN (PS-INV-OfflineChrome).
// =============================================================================

/// PS-INV-OfflineChrome / GOLD `the_peers_page_chrome_stays_offline_no_cdn` (I-PS-4 /
/// KPI-HX-G2): the `/peers` full page references ONLY the LOCAL `/static/htmx.min.js`
/// script src and NO off-host CDN — the page CHROME stays offline-capable (and so does the
/// SUBSCRIPTION LIST itself, since the read is LOCAL — even stronger than `/search`).
///
/// Given the viewer renders the /peers full page;
/// When the page's script references are inspected;
/// Then the only htmx asset reference is the local /static/htmx.min.js — no CDN.
///
/// @us-ps-002 @property @driving_port @real-io @offline @no-cdn @i-ps-4 @gold
#[test]
fn the_peers_page_chrome_stays_offline_no_cdn() {
    // GIVEN two active peers + the viewer rendering the /peers full page + fragment.
    // WHEN each shape's script references are inspected.
    // THEN `references_external_cdn()` is FALSE for both (the only htmx asset is the local
    // /static/htmx.min.js; I-PS-4 / KPI-HX-G2). NO network seam is wired (plain
    // `ViewerServer::start`): `/peers` is a LOCAL read, so the page CHROME and the
    // SUBSCRIPTION LIST itself are both offline-capable.
    let env = TestEnv::initialized();
    seed_peers_two_active_with_claims(&env);
    let viewer = ViewerServer::start(&env);

    // The no-header full page carries the chrome (the htmx `<script src>`). The HX-Request
    // fragment is the bare `#peers` region — neither shape may reference an off-host CDN
    // (the only htmx asset is the LOCAL /static/htmx.min.js; I-PS-4 / KPI-HX-G2). Asserted
    // over BOTH shapes.
    let full_page = viewer.get(PEERS_PATH);
    let fragment = viewer.get_htmx(PEERS_PATH);

    assert_eq!(
        full_page.status, 200,
        "PS-INV-OfflineChrome: GET /peers (full page) must render successfully (200) so the \
         no-CDN scan is over REAL chrome; body was:\n{}",
        full_page.body
    );
    assert_eq!(
        fragment.status, 200,
        "PS-INV-OfflineChrome: GET /peers (htmx fragment) must render successfully (200); \
         body was:\n{}",
        fragment.body
    );

    assert!(
        full_page.is_full_page(),
        "PS-INV-OfflineChrome: the no-JS /peers response must be a complete full page \
         (chrome present — it is the surface that loads the htmx asset); body was:\n{}",
        full_page.body
    );
    assert!(
        fragment.is_fragment(),
        "PS-INV-OfflineChrome: the HX-Request /peers response must be a bare fragment (no \
         chrome); body was:\n{}",
        fragment.body
    );

    // The hard invariant (I-PS-4 / KPI-HX-G2): NO /peers shape references an off-host CDN
    // for the htmx library — the only htmx asset is the LOCAL /static/htmx.min.js the viewer
    // serves itself. Any CDN host hit is an UNSHIPPABLE offline-guarantee breach.
    assert!(
        !full_page.references_external_cdn(),
        "I-PS-4: the /peers full page must reference ONLY the local /static/htmx.min.js — \
         no off-host CDN; body was:\n{}",
        full_page.body
    );
    assert!(
        !fragment.references_external_cdn(),
        "I-PS-4: the /peers htmx fragment must reference ONLY the local /static/htmx.min.js \
         — no off-host CDN; body was:\n{}",
        fragment.body
    );
}

// =============================================================================
// I-PS-4 / KPI-5 — local-first / offline: the /peers surface works with the network
// unavailable (PS-INV-Offline). The subscription list is a LOCAL read with NO outbound edge
// — offline-STRONGER than /search.
// =============================================================================

/// PS-INV-Offline / GOLD `the_peers_surface_works_fully_offline` (I-PS-4 / KPI-5): the
/// `/peers` view renders fully with NO network available — the subscription list + per-peer
/// counts (not just the chrome) are computed over the LOCAL DuckDB store + the PURE
/// projection, so the network being down NEVER degrades it (distinct from `/search` and
/// `/scrape`, the only network-requiring routes). Because `/peers` has NO outbound edge to
/// take down (no PDS fetch, no DID re-resolution, no peer pull), this gold pins that the
/// rendered list is identical to an on-network render — the LOCAL read is self-sufficient
/// by construction (offline-STRONGER than `/search`, I-PS-4).
///
/// Given the viewer is started over a seeded store with NO network seam wired (no indexer
///   URL, no GitHub base — exactly the LOCAL-only posture);
/// When /peers is rendered;
/// Then the full peer list with per-peer counts + render-only commands renders (no
///   Unavailable/degraded state, no network call) — the read is LOCAL.
///
/// @us-ps-002 @property @driving_port @real-io @offline @local-first @i-ps-4 @kpi-5 @gold
#[test]
fn the_peers_surface_works_fully_offline() {
    // GIVEN `ViewerServer::start(&env)` — the store-only posture with NEITHER the /scrape
    // GitHub seam NOR the /search indexer seam wired (the LOCAL-only viewer). Two active
    // peers are seeded into the LOCAL store. WHEN /peers is rendered. THEN the full peer
    // list renders (per-peer counts + render-only commands) with NO Unavailable/degraded
    // notice and NO network call — proving /peers is LOCAL + offline by construction (I-PS-4
    // — no PDS fetch, no DID re-resolution, no peer pull on this route).
    let env = TestEnv::initialized();
    seed_peers_two_active_with_claims(&env);

    // The plain `ViewerServer::start` is THE proof of "no network seam wired": it spawns the
    // store-only viewer with NO indexer URL AND NO GitHub base exported. With no outbound
    // edge to take down, a full peer-list render here proves /peers is LOCAL + offline by
    // construction (I-PS-4 / KPI-5).
    let viewer = ViewerServer::start(&env);

    // Both shapes — the no-header full page (`get`) AND the htmx fragment (`get_htmx`) —
    // render the subscription list over the LOCAL store with no network.
    let full_page = viewer.get(PEERS_PATH);
    let fragment = viewer.get_htmx(PEERS_PATH);

    assert_eq!(
        full_page.status, 200,
        "PS-INV-Offline: GET /peers (full page) must render a calm 200 over the LOCAL store \
         with no network wired; body was:\n{}",
        full_page.body
    );
    assert_eq!(
        fragment.status, 200,
        "PS-INV-Offline: GET /peers (htmx fragment) must render a calm 200 over the LOCAL \
         store with no network wired; body was:\n{}",
        fragment.body
    );

    for (shape, body) in [("full page", &full_page.body), ("fragment", &fragment.body)] {
        // The full attributed peer list renders (the LOCAL read is self-sufficient with no
        // network present) — both active peers with their per-peer counts.
        assert_peer_row_present(body, PEERS_RACHEL_DID, PEERS_RACHEL_CLAIM_COUNT);
        assert_peer_row_present(body, PEERS_TOBIAS_DID, PEERS_TOBIAS_CLAIM_COUNT);

        // NO Unavailable / degraded notice — /peers has no outbound edge to fail, so it
        // NEVER renders the slice-08 `/search` Unavailable arm.
        let lowered = body.to_ascii_lowercase();
        for banned in [
            "unavailable",
            "network error",
            "could not reach",
            "try again",
        ] {
            assert!(
                !lowered.contains(banned),
                "PS-INV-Offline ({shape}): the offline /peers render must NOT show a \
                 network-degraded notice ({banned:?}) — /peers has no outbound edge \
                 (I-PS-4); body was:\n{body}"
            );
        }
    }
}

// =============================================================================
// I-PS-8 / DD-PS-1 — no N+1: the active set + every per-peer count resolves in ONE request
// (PS-INV-NoNPlus1). The behavioral proxy (the strict 1-query bound is the DELIVER
// adapter-duckdb unit/property test).
// =============================================================================

/// PS-INV-NoNPlus1 / GOLD `a_large_active_set_resolves_per_peer_counts_in_one_request`
/// (I-PS-8 / DD-PS-1): a MULTI-peer active set with KNOWN distinct per-peer counts (4, 3,
/// 2, 1) is rendered correctly — every peer's OWN count, no merged total, no dropped/
/// miscounted row — in ONE `/peers` request. The N+1 behavioral proxy mirroring the
/// slice-10/13/14 single-request multi-breakdown proxy: a wrong/merged/dropped count in a
/// multi-peer breakdown flags an N+1 / merge / projection regression. The strict
/// "exactly ONE aggregate query, invariant to peer count" bound is the DELIVER
/// adapter-duckdb unit/property test (this AT pins the OBSERVABLE multi-peer render is
/// correct in one request).
///
/// Given Maria actively follows N peers with KNOWN distinct per-peer counts;
/// When she opens GET /peers;
/// Then every peer row shows its OWN per-peer count, resolved correctly in ONE request, with
///   no merged total and no dropped row.
///
/// @us-ps-001 @us-ps-002 @property @driving_port @real-io @no-n-plus-1 @i-ps-8 @gold
#[test]
fn a_large_active_set_resolves_per_peer_counts_in_one_request() {
    // GIVEN a store seeded with FOUR active peers with DISTINCT known counts (4, 3, 2, 1)
    // via the production federation path (seed_many_active_peers_known_counts pins each
    // count). WHEN ONE `get` /peers request renders. THEN every peer row shows its OWN
    // per-peer count (the whole active set + every count resolved correctly in ONE request
    // — the N+1 behavioral proxy; I-PS-8 / DD-PS-1). A wrong/merged/dropped count flags an
    // N+1 / merge / projection regression.
    let env = TestEnv::initialized();
    let expected = seed_many_active_peers_known_counts(&env);
    let viewer = ViewerServer::start(&env);

    let response = viewer.get(PEERS_PATH);

    assert_eq!(
        response.status, 200,
        "PS-INV-NoNPlus1: GET /peers over a multi-peer active set must return 200; body \
         was:\n{}",
        response.body
    );

    // Every active peer renders its OWN per-peer count in this SINGLE request — the active
    // set + every count resolved correctly in ONE render (the N+1 behavioral proxy). A
    // dropped/merged/miscounted row fails here.
    for (did, count) in &expected {
        assert_peer_row_present(&response.body, did, *count);
        // Each peer's revocation command is render-only TEXT (consistency of the whole
        // multi-peer render — every row carries its own render-only command).
        assert_peer_remove_command_is_render_only(&response.body, did);
    }

    // NO merged total across the active set (the sum of all per-peer counts must NEVER
    // appear as a "claims" count — anti-merging extends to the multi-peer render; J-003a /
    // I-PS-3).
    let merged_total: usize = expected.iter().map(|(_, c)| *c).sum();
    assert!(
        !response.body.contains(&format!("{merged_total} claims")),
        "PS-INV-NoNPlus1 (J-003a / I-PS-3): the multi-peer /peers render must NEVER show a \
         merged total of {merged_total} claims (each peer's count is PER-PEER, never summed \
         across the active set); body was:\n{}",
        response.body
    );
}
