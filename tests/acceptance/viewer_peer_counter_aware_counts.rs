//! Slice-19 acceptance — the `openlore ui` COUNTERED-PEER-CLAIMS COUNT surfaced on the
//! `GET /` landing summary AND in the `GET /peer-claims` list header: "4 peer claims
//! (1 countered)" (US-PC-000/001/002; ADR-056). The deferred PEER sibling of slice-18.
//!
//! slice-17 turned `GET /` into a read-only orientation front door with a three-count
//! `LandingSummary`; slice-18 EXTENDED it with a FOURTH `Option<usize>` field
//! `countered_own_claims` (the countered-OWN count beside the own line + in the `/claims`
//! header). slice-19 ships the symmetric PEER count: a FIFTH additive `Option<usize>`
//! field `countered_peer_claims` rendered beside the PEER line on the landing ("4 peer
//! claims (1 countered)") AND in the `/peer-claims` list header ("Peer Claims
//! (1 countered)") — answering, at a glance, "how many of my cached PEER claims have been
//! countered?". The count is `count_countered_peer_claims()` = the EXACT slice-18 SQL with
//! the OUTER table swapped `claims c → peer_claims p`: `COUNT(DISTINCT p.cid) FROM
//! peer_claims p WHERE p.cid IN (SELECT referenced_cid FROM claim_references WHERE
//! ref_type='counters' UNION SELECT referenced_cid FROM peer_claim_references WHERE
//! ref_type='counters')` — a PRESENCE count (a peer claim countered by N counterers counts
//! ONCE, no JOIN-fanout), peer-only by the outer table, invariant to store size (ADR-056
//! D1). The inner `UNION` IN-set is BYTE-IDENTICAL to slice-18's — only the outer table
//! differs (the de-dup / presence-once semantics are inherited verbatim). A cached PEER
//! claim is countered by the OPERATOR (her counter in `claim_references`) OR by ANOTHER
//! PEER (their counter in `peer_claim_references`, slice-11) — both arms contribute.
//!
//! The SHARED pure `render_countered(Option<usize>) -> String` helper slice-18 established
//! renders the SAME parenthetical on BOTH surfaces (single source — ADR-056 D3; NO new
//! helper, WD-PC-10): `Some(1) → "(1 countered)"`, `Some(0) → "(0 countered)"` (honest
//! zero), `None → "(— countered)"` (missing marker). The peer-claims "4" + the slice-06/07
//! `/peer-claims` list order/paging + the slice-13 per-row flags are BYTE-IDENTICAL to the
//! no-counter-count baseline (the countered count is additive header text — C-4 / WD-PC-9).
//! The slice-18 OWN surfaces (landing own line + `/claims` header) are UNTOUCHED (WD-PC-7 /
//! BR-PC-4). Read-only / no-key, LOCAL / offline, missing≠zero, presence-once, anti-misread
//! neutral copy.
//!
//! Driving discipline (Mandate 1): every scenario enters through the REAL `openlore ui`
//! subprocess (`ViewerServer::start`) + in-test HTTP GET against `/` and `/peer-claims` —
//! NO scenario calls `viewer-domain::render_landing` / `render_peer_claims_page` /
//! `render_countered` / the count read directly (those are unit/property-level, exercised
//! in DELIVER). The local DuckDB store is REAL, seeded through the PRODUCTION federation
//! write paths (peer claims via `peer add` + `peer pull`; the counters that flag them via a
//! distinct peer's `counters`-referencing record OR the operator's own `claim counter` —
//! both land the peer-claim CID as a countered `referenced_cid`), so the rows the count
//! read returns are produced by production code, not hand-inserted (Pillar 3 / BR-VIEW-4).
//! NO external/network boundary exists — `/` and `/peer-claims` are LOCAL + OFFLINE (the
//! count is a LOCAL `COUNT(DISTINCT)` aggregate with no outbound edge). Every assertion is
//! on the rendered HTML the operator's browser shows (Mandate 8 universe = port-exposed
//! rendered surface).
//!
//! Layer placement (nw-tdd-methodology Layered Test Discipline matrix + Mandate 9/11):
//! every scenario is a layer-3/layer-5 subprocess + real-I/O test — EXAMPLE-only. The sad
//! paths (honest "(0 countered)", failed countered-peer-count read → "(— countered)") are
//! enumerated explicitly, never PBT-generated at this layer (the generative exploration of
//! the pure `render_countered` over the Some(0)/Some(n)/None cases is a layer-1/2 DELIVER
//! concern). Tier B (state-machine PBT) is NOT warranted: the count is a single-shot
//! additive render with no chained ≥3-scenario journey and no domain-rich input space (one
//! `Option<usize>`) — Tier A example coverage is exact (Mandate 10 skip criteria).
//!
//! Build-before-run note (carry into the DELIVER roadmap, mirrors slice-06..18): `cargo
//! test` does NOT rebuild a spawned binary automatically — the roadmap/run MUST `cargo
//! build` the `openlore` bin (the viewer) before running these ATs so
//! `ViewerServer::start` spawns the CURRENT viewer, not a stale one. The count is a LOCAL
//! DuckDB read — no second binary needed.
//!
//! Mandate 7 RED scaffolds: the ATs spawn the bin + HTTP, so they COMPILE now with the
//! new `seed_landing_store_with_countered_peer_claims` /
//! `seed_landing_store_no_peer_claim_countered` /
//! `seed_landing_store_one_peer_claim_countered_twice` /
//! `start_viewer_with_failing_countered_peer_count` seeds +
//! `assert_landing_peer_countered_count` / `assert_landing_peer_countered_missing` /
//! `assert_peer_claims_header_countered_count` /
//! `assert_landing_and_peer_claims_countered_consistent` seeds/asserts (which compile —
//! they drive the EXISTING `peer add`/`peer pull`/`claim counter` verbs + scan strings +
//! read the REAL store) plus the REUSED slice-18 `assert_countered_copy_is_neutral` +
//! own-surface-untouched asserts. Each scenario body runs to a `GET /` or `GET
//! /peer-claims` HTTP assertion that FAILS because the production routes do NOT render the
//! peer countered count yet, and `count_countered_peer_claims` / the 5th `LandingSummary`
//! field / the `render_peer_claims_page` `Option<usize>` param do NOT exist — so
//! "(1 countered)" on the PEER line / `/peer-claims` header is ABSENT → classifies RED
//! (MISSING_FUNCTIONALITY), NOT BROKEN. The ATs drive the routes via subprocess HTTP
//! (never the Rust `render_landing` / `render_peer_claims_page` signatures), so the
//! production signature changes (the 5th field + the `/peer-claims` param) are DELIVER's
//! job and do not affect AT compilation. They stay RED until DELIVER's per-scenario
//! RED→GREEN→COMMIT cycles (ADR-025).
//!
//! Covers (acceptance-criteria.md, 9 themes for US-PC-001/002 + US-PC-000 read wiring):
//! - PC-WS (Theme 1, US-PC-001): WALKING SKELETON — GET / over a seeded store (4 peer
//!   claims, 1 countered) renders "4 peer claims (1 countered)" beside the unchanged
//!   peer-claims count, with the slice-18 own line "12 own claims (3 countered)" untouched.
//! - Theme 1 Ex 2 (US-PC-002): GET /peer-claims renders the SAME "(1 countered)" in the
//!   list header; landing peer count == /peer-claims header count for the same store.
//! - Theme 2: presence-once — a peer claim countered by 2 distinct counterers counts ONCE
//!   ("(1 countered)", never "(2 countered)"); + either-ref-table-contributes-once.
//! - Theme 3: honest zero — peer claims, NONE countered → "(0 countered)" (Some(0)),
//!   DISTINCT from the missing marker; the /peer-claims list renders as slice-06/07.
//! - Theme 4: missing≠zero — a FAILED countered-peer-count read → "(— countered)" on BOTH
//!   surfaces while the peer-claims "4" + the other counts (incl. the slice-18 own
//!   "(3 countered)") + the rows + slice-13 flags render, page 200.
//! - Theme 5: no re-weight / additive — the peer-claims "4" is unchanged; the slice-18 own
//!   line untouched; the /peer-claims list order/paging + the slice-13 per-row flags
//!   byte-identical baseline.
//! - Theme 6: read-only / no-write — / and /peer-claims render no write/sort/filter/
//!   mutating control from the count.
//! - Theme 7: LOCAL/offline — both render fully network-down; vendored htmx only.
//! - Theme 8 (US-PC-000): no-N+1 — the countered-peer count is one aggregate read,
//!   invariant to store size (a large store with a known countered subset → right count).
//! - Theme 9: anti-misread — the "(N countered)" copy NEVER contains penalty/deduction/
//!   "disputed by N"/verdict language; the countered peer claim's confidence verbatim.
//
// SCAFFOLD: true

mod support;

#[allow(unused_imports)]
use support::*;

// =============================================================================
// PC-WS — WALKING SKELETON (US-PC-001 Theme 1; the riskiest-assumption thread):
// GET / over a seeded store (4 peer claims, 1 countered) renders "4 peer claims
// (1 countered)" beside the unchanged peer-claims count, on the existing landing
// dashboard, with the slice-18 own line untouched. This is the thinnest complete
// thread the slice can demo: viewer → the LOCAL countered-PEER-count aggregate →
// the extended LandingSummary (5th field) → the REUSED render_countered → the
// front-door summary showing the disputed-peer-claim awareness count.
// =============================================================================

/// PC-WS / WALKING SKELETON (US-PC-001 Theme 1; AC "The front door shows how many of my
/// cached peer claims are countered"): from the LOCAL store seeded with 4 cached peer
/// claims, 1 of which is countered (countered by a distinct peer OR the operator),
/// `GET /` returns a 200 page that shows "4 peer claims (1 countered)" — the countered
/// count beside the UNCHANGED peer-claims count. The thinnest demo-able thread: the front
/// door now orients the operator on not just how much peer material is in her store but
/// how much of it has been disputed, read-only, offline. The slice-18 own line is
/// untouched beside it.
///
/// Given Maria's store caches 4 peer claims, 1 of which has ≥1 counter;
/// When she opens GET / in the openlore ui viewer;
/// Then the landing summary shows "4 peer claims" with "(1 countered)" beside it, and the
///   peer-claims count "4" is unchanged by the presence of the countered count.
///
/// @us-pc-001 @walking_skeleton @driving_port @driving_adapter @real-io @kpi-view-1 @happy
#[test]
fn the_front_door_shows_how_many_cached_peer_claims_are_countered() {
    // GIVEN a REAL local store seeded (production `peer add` + `peer pull` + counter paths,
    // Pillar 3) with 4 cached peer claims, 1 of them countered.
    let env = TestEnv::initialized();
    let _held = seed_landing_store_with_countered_peer_claims(&env);

    // WHEN Maria opens GET / in the viewer (full page — `/` is full-page-only, ADR-054 D5).
    let viewer = ViewerServer::start(&env);
    let page = viewer.get(LANDING_PATH);

    // THEN the front door renders a 200 page showing the peer-claims count "4" UNCHANGED
    // AND the countered-peer count "(1 countered)" beside it (the genuine seeded presence
    // count, ADR-056 D1/D3).
    assert_eq!(
        page.status, 200,
        "GET / must render the landing dashboard (200); got {}",
        page.status
    );
    // The peer-claims count "4" still renders (additive — the countered count never
    // re-weights it, C-4).
    assert_landing_shows_count(&page.body, "peer claims", LANDING_COUNTERED_PEER_TOTAL);
    // The NEW countered-peer count "(1 countered)" renders beside it (US-PC-001 headline).
    assert_landing_peer_countered_count(&page.body, COUNTERED_PEER_CLAIMS);
}

// =============================================================================
// Theme 1 Ex 2 — the `/peer-claims` header shows the SAME disputed-claim awareness
// count (US-PC-002); landing peer count == /peer-claims header count for the same
// store (single source — WD-PC-8).
// =============================================================================

/// PC-HEADER (US-PC-002 Theme 1 Ex 2; AC "The `/peer-claims` header shows the same
/// disputed-claim awareness"): over the SAME store, `GET /peer-claims` renders
/// "(1 countered)" in the list header — the SAME count the landing shows beside "4 peer
/// claims". Both surfaces resolve the count from the SAME `count_countered_peer_claims`
/// read and render through the SAME `render_countered` helper (single source — WD-PC-8 /
/// ADR-056 D3), so the landing and `/peer-claims` counts cannot diverge.
///
/// Given Maria's store caches 4 peer claims, 1 of which has ≥1 counter;
/// When she opens GET /peer-claims;
/// Then the list header shows "(1 countered)", the SAME count the landing shows beside
///   "4 peer claims".
///
/// @us-pc-002 @driving_port @real-io @single-source @wd-pc-8 @happy
#[test]
fn the_peer_claims_header_shows_the_same_countered_count_as_the_landing() {
    // GIVEN a store seeded with 4 peer claims, 1 countered.
    let env = TestEnv::initialized();
    let _held = seed_landing_store_with_countered_peer_claims(&env);

    // WHEN Maria opens GET / AND GET /peer-claims over the SAME store.
    let viewer = ViewerServer::start(&env);
    let landing = viewer.get(LANDING_PATH);
    let peer_claims = viewer.get(PEER_CLAIMS_LIST_PATH);

    // THEN the `/peer-claims` header shows the SAME "(1 countered)" the landing shows, and
    // the two counts are EQUAL (single source — WD-PC-8).
    assert_eq!(landing.status, 200, "GET / must render (200)");
    assert_eq!(peer_claims.status, 200, "GET /peer-claims must render (200)");
    assert_landing_peer_countered_count(&landing.body, COUNTERED_PEER_CLAIMS);
    assert_peer_claims_header_countered_count(&peer_claims.body, COUNTERED_PEER_CLAIMS);
    assert_landing_and_peer_claims_countered_consistent(&landing.body, &peer_claims.body);
}

// =============================================================================
// Theme 2 — presence count: a multiply-countered peer claim counts once
// (US-PC-000/001; C-4 / BR-PC-1). COUNT(DISTINCT) collapses two counterers of the
// SAME peer CID to ONE → "(1 countered)", never "(2 countered)". + either ref
// table contributes exactly once.
// =============================================================================

/// PC-PRESENCE (US-PC-000/001 Theme 2; AC "A peer claim countered by both the operator
/// and another peer counts once"): Maria's cached peer claim is countered by BOTH her own
/// counter (in `claim_references`) AND another peer's counter (in `peer_claim_references`),
/// and it is her only countered peer claim. When she opens GET /, the landing summary shows
/// "(1 countered)", NOT "(2 countered)" — the count is a PRESENCE count (the de-duped UNION
/// IN-set + COUNT(DISTINCT) collapses the two distinct-table counters of the SAME peer CID
/// to ONE membership), and the copy shows NO "disputed by N" sum of counters.
///
/// Given Maria's cached peer claim is countered by both Maria's own counter and Rachel's
///   counter, and it is her only countered peer claim;
/// When she opens GET /;
/// Then the landing summary shows "(1 countered)", not "(2 countered)", and the count
///   shows no "disputed by N" sum of counters.
///
/// @us-pc-000 @us-pc-001 @driving_port @real-io @presence-once @c-4 @br-pc-1 @cardinal
/// @boundary
#[test]
fn a_peer_claim_countered_by_two_counterers_counts_once() {
    // GIVEN one cached peer claim countered by TWO distinct counterers (one in each ref
    // table — the operator + another peer), the only countered peer claim — the
    // COUNT(DISTINCT) must collapse it to ONE.
    let env = TestEnv::initialized();
    let _held = seed_landing_store_one_peer_claim_countered_twice(&env);

    // WHEN Maria opens GET /.
    let viewer = ViewerServer::start(&env);
    let page = viewer.get(LANDING_PATH);

    // THEN the landing shows "(1 countered)", NEVER "(2 countered)" (presence-once), and
    // the copy carries no "disputed by N" total (anti-misread).
    assert_eq!(page.status, 200, "the landing page must render");
    assert_landing_peer_countered_count(&page.body, 1);
    assert!(
        !page.body_contains("(2 countered)"),
        "a peer claim countered by TWO counterers must count ONCE — the landing must NOT \
         show \"(2 countered)\" (presence-once, COUNT(DISTINCT); C-4 / BR-PC-1); body \
         was:\n{}",
        page.body
    );
    assert_countered_copy_is_neutral(&page.body);
}

/// PC-EITHER-TABLE (US-PC-000/001 Theme 2 Ex 2; AC "A peer claim countered from either ref
/// table contributes exactly once"): one cached peer claim is countered by Maria's OWN
/// counter (landing in `claim_references`); a SECOND cached peer claim is countered by
/// ANOTHER peer's counter (landing in `peer_claim_references`); no other cached peer claim
/// is countered. When she opens GET /, the landing summary shows "(2 countered)" — each
/// countered peer claim contributes EXACTLY ONCE regardless of which ref table holds its
/// counter (the de-duped UNION across both arms).
///
/// Given one cached peer claim countered by Maria's own counter (in claim_references) AND a
///   second cached peer claim countered by another peer's counter (in peer_claim_references),
///   and no other cached peer claim is countered;
/// When she opens GET /;
/// Then the landing summary shows "(2 countered)", each countered peer claim contributing
///   exactly once regardless of which ref table holds its counter.
///
/// @us-pc-000 @us-pc-001 @driving_port @real-io @presence-once @c-4 @br-pc-1 @boundary
#[test]
fn a_peer_claim_countered_from_either_ref_table_contributes_once() {
    // GIVEN two distinct cached peer claims, one countered via the `claim_references` arm
    // (Maria's own counter) and one via the `peer_claim_references` arm (another peer's
    // counter) — the UNION must sum them to 2, each contributing once.
    let env = TestEnv::initialized();
    let _held = seed_landing_store_peer_claims_countered_each_arm(&env);

    // WHEN Maria opens GET /.
    let viewer = ViewerServer::start(&env);
    let page = viewer.get(LANDING_PATH);

    // THEN the landing shows "(2 countered)" — each of the two peer claims contributes once
    // (one from each ref-table arm of the de-duped UNION IN-set).
    assert_eq!(page.status, 200, "the landing page must render");
    assert_landing_peer_countered_count(&page.body, 2);
}

// =============================================================================
// Theme 3 — honest zero when nothing is countered (US-PC-001/002; C-5): Some(0) →
// "(0 countered)", a SUCCESSFUL read of zero, DISTINCT from the missing marker; the
// /peer-claims list renders as slice-06/07.
// =============================================================================

/// PC-ZERO-LANDING (US-PC-001 Theme 3; AC "An honest (0 countered) on the landing"): a
/// store caching 4 peer claims, NONE of which has drawn a counter. When Maria opens GET /,
/// the landing shows "4 peer claims (0 countered)" — "(0 countered)" is a SUCCESSFUL read
/// of zero (Some(0)), DISTINCT from the missing-number state "(— countered)" and not an
/// omitted count. The success side of `0 ≠ missing`.
///
/// Given Maria's store caches 4 peer claims, none of which has drawn a counter;
/// When she opens GET /;
/// Then the landing summary shows "4 peer claims (0 countered)", and "(0 countered)" is a
///   successful read of zero, not a missing-number state and not an omitted count.
///
/// @us-pc-001 @driving_port @real-io @honest-zero @c-5 @edge
#[test]
fn an_honest_zero_countered_on_the_landing_when_no_peer_claim_is_disputed() {
    // GIVEN 4 cached peer claims, NONE countered (the count read returns Some(0)).
    let env = TestEnv::initialized();
    let _held = seed_landing_store_no_peer_claim_countered(&env);

    // WHEN Maria opens GET /.
    let viewer = ViewerServer::start(&env);
    let page = viewer.get(LANDING_PATH);

    // THEN the landing shows the peer-claims "4" AND "(0 countered)" (Some(0), honest
    // zero) — DISTINCT from the missing marker "(— countered)".
    assert_eq!(page.status, 200, "the landing page must render");
    assert_landing_shows_count(&page.body, "peer claims", LANDING_COUNTERED_PEER_TOTAL);
    assert_landing_peer_countered_count(&page.body, 0);
    // The honest zero is NOT the missing-number state (the success side of 0 ≠ missing).
    assert!(
        !page.body_contains("(— countered)"),
        "an honest empty-counter peer store renders \"(0 countered)\" (Some(0)), NOT the \
         missing-number marker \"(— countered)\" (which would mean a FAILED read; C-5); \
         body was:\n{}",
        page.body
    );
}

/// PC-ZERO-HEADER (US-PC-002 Theme 3; AC "An honest (0 countered) in the `/peer-claims`
/// header, list renders as slice-06/07"): the SAME none-countered peer store. When Maria
/// opens GET /peer-claims, the list header shows "(0 countered)" and the list renders its
/// rows with NO per-row Countered flags, exactly as slice-06/07 (nothing is countered, so
/// the slice-13 presence read returns the empty set).
///
/// Given Maria's store caches 4 peer claims, none of which has drawn a counter;
/// When she opens GET /peer-claims;
/// Then the list header shows "(0 countered)" and the list renders its rows with no
///   per-row Countered flags, exactly as slice-06/07.
///
/// @us-pc-002 @driving_port @real-io @honest-zero @no-noise @c-5 @edge
#[test]
fn an_honest_zero_countered_in_the_peer_claims_header_list_as_slice_06_07() {
    // GIVEN 4 cached peer claims, NONE countered.
    let env = TestEnv::initialized();
    let _held = seed_landing_store_no_peer_claim_countered(&env);

    // WHEN Maria opens GET /peer-claims.
    let viewer = ViewerServer::start(&env);
    let page = viewer.get(PEER_CLAIMS_LIST_PATH);

    // THEN the header shows "(0 countered)" AND the list carries NO per-row "Countered"
    // flag (nothing is countered — the slice-13 presence read is empty; the rows render
    // exactly as slice-06/07).
    assert_eq!(page.status, 200, "the /peer-claims page must render");
    assert_peer_claims_header_countered_count(&page.body, 0);
    // No per-row flag anchor appears anywhere — the body carries no `>Countered</a>` marker
    // (the header "(0 countered)" is the ONLY countered text, not the per-row flag shape).
    assert!(
        !page.body_contains(&format!(">{LIST_COUNTERED_FLAG_TEXT}</a>")),
        "a none-countered peer store must render the `/peer-claims` rows with NO per-row \
         Countered flag, exactly as slice-06/07 (the header \"(0 countered)\" is the only \
         countered text); body was:\n{}",
        page.body
    );
}

// =============================================================================
// Theme 4 — missing ≠ zero: a failed count read degrades independently, no 5xx
// (US-PC-000/001/002; C-2 / C-5 CARDINAL). Both surfaces render "(— countered)"
// while the sibling counts (incl. the slice-18 own "(3 countered)") / rows render;
// page 200.
// =============================================================================

/// PC-DEGRADE-LANDING (US-PC-000/001 Theme 4 / C-2 / C-5 CARDINAL; AC "A failed
/// countered-peer-count read degrades gracefully on the front door"): Maria's
/// countered-peer-count read fails while the peer-claims count + the slice-18 own counts
/// succeed. When she opens GET /, the peer-claims count + the slice-18 own line
/// ("12 own claims (3 countered)") + the rest of the summary + the nav hub still render,
/// the countered-peer count renders as the missing-number state "(— countered)" (NOT a
/// fabricated "(0 countered)"), and the page is a normal 200 — never a 5xx, never a blanked
/// summary. The per-count independent degrade (`.ok()` → `None` → `render_countered(None)`,
/// ADR-056 D2/D4) — the PEER count fails INDEPENDENTLY of the own count (4th DISTINCT
/// fault-seam token).
///
/// SEEDING-SEAM NOTE (documented DISTILL choice, slice-17/18 degrade precedent): the viewer
/// holds one long-lived DuckDB connection taken at startup, so there is no readily-available
/// mid-request per-count read-failure seam in the slice-06/15 harness. This scenario drives
/// the TEST-ONLY effect-shell fault seam (`start_viewer_with_failing_countered_peer_count`
/// → the `OPENLORE_VIEWER_FAIL_COUNTERED_PEER_COUNT` env var, a 4th DISTINCT token so the
/// PEER count fails independently of the slice-18 own count — materialized by DELIVER
/// exactly as slice-18 materialized `OPENLORE_VIEWER_FAIL_COUNTERED_COUNT`). The Some(0)
/// success side is fully exercised by the honest-zero scenarios above; this scenario pins
/// the FAILURE side. Until DELIVER materializes the seam the scenario fails at `start_inner`'s
/// `todo!()` body → RED MISSING_FUNCTIONALITY, never BROKEN.
///
/// Given Maria's countered-peer-claims count read fails while the peer-claims count
///   succeeds;
/// When she opens GET /;
/// Then the peer-claims count + the slice-18 own line ("12 own claims (3 countered)") + the
///   rest of the summary + the nav hub still render, the countered-peer count renders as
///   "(— countered)" (not a fabricated "(0 countered)"), and the page is a normal 200.
///
/// @us-pc-000 @us-pc-001 @driving_port @real-io @infrastructure-failure @missing-not-zero
/// @c-2 @c-5 @cardinal @error
#[test]
fn a_failed_countered_peer_count_read_degrades_gracefully_on_the_front_door() {
    // GIVEN a store seeded with 4 peer + 1 countered AND the slice-18 own 12+3 countered,
    // BUT the countered-PEER-count read forced to FAIL mid-request (the peer-claims count +
    // the slice-18 own counts + the other reads still succeed).
    let env = TestEnv::initialized();
    let _held = seed_landing_store_with_countered_peer_and_own(&env);
    // The genuine peer-claims total over the combined store (slice-18 own-counter rows +
    // Priya's 4) — NOT a clean headline 4, so read it from the store.
    let peer_total = landing_peer_total(&env);

    // WHEN Maria opens GET / against the viewer whose countered-PEER-count read fails.
    let viewer = start_viewer_with_failing_countered_peer_count(&env);
    let page = viewer.get(LANDING_PATH);

    // THEN the page is a NORMAL 200 (never a 5xx), the peer-claims count + the slice-18 own
    // line ("12 own claims (3 countered)") + the nav hub still render, and the
    // countered-PEER count renders the missing marker "(— countered)" (NOT a fabricated
    // "(0 countered)").
    assert_eq!(
        page.status, 200,
        "a failed countered-peer-count read must degrade to a 200 page, NEVER a 5xx (C-2 \
         CARDINAL / NFR-VIEW-6); got {}",
        page.status
    );
    // The peer-claims count STILL renders (the degrade is per-count, independent).
    assert_landing_shows_count(&page.body, "peer claims", peer_total);
    // The slice-18 OWN line ("12 own claims (3 countered)") is UNTOUCHED — the PEER count
    // fails INDEPENDENTLY of the own count (4th distinct fault-seam token, WD-PC-7).
    assert_landing_own_line_untouched(&page.body);
    // The nav hub still renders in full (the failure is per-count, not page-wide).
    assert_landing_links_all_surfaces(&page.body);
    // The FAILED countered-PEER count renders the missing marker "(— countered)" (not 0).
    assert_landing_peer_countered_missing(&page.body);
    // No raw stack trace leaks to the rendered surface (NFR-VIEW-6).
    for stack_trace_marker in ["panicked at", "RUST_BACKTRACE", "stack backtrace"] {
        assert!(
            !page.body_contains(stack_trace_marker),
            "the degraded page must be plain — no raw stack trace ({stack_trace_marker:?}); \
             body was:\n{}",
            page.body
        );
    }
}

/// PC-DEGRADE-HEADER (US-PC-000/002 Theme 4 / C-2 / C-5 CARDINAL; AC "A failed header count
/// degrades without blanking the `/peer-claims` list"): Maria's countered-peer-count read
/// fails. When she opens GET /peer-claims, the list header renders the missing-number state
/// "(— countered)" (not a fabricated "(0 countered)"), the list rows + their slice-13
/// per-row flags still render, and the page is a normal 200 — the countered-peer-count read
/// is INDEPENDENT of the list read.
///
/// Given Maria's countered-peer-claims count read fails;
/// When she opens GET /peer-claims;
/// Then the list header renders "(— countered)" (not a fabricated "(0 countered)"), the
///   list rows + their slice-13 per-row flags still render, and the page is a normal 200.
///
/// @us-pc-000 @us-pc-002 @driving_port @real-io @infrastructure-failure @missing-not-zero
/// @c-2 @c-5 @cardinal @error
#[test]
fn a_failed_header_count_degrades_without_blanking_the_peer_claims_list() {
    // GIVEN a store seeded with 4 peer + 1 countered, BUT the countered-peer-count read
    // forced to FAIL (the list read + the slice-13 per-row flag read still succeed).
    let env = TestEnv::initialized();
    let seeded = seed_landing_store_with_countered_peer_claims(&env);

    // WHEN Maria opens GET /peer-claims against the viewer whose countered-peer-count read
    // fails.
    let viewer = start_viewer_with_failing_countered_peer_count(&env);
    let page = viewer.get(PEER_CLAIMS_LIST_PATH);

    // THEN the page is a NORMAL 200, the header renders the missing marker "(— countered)"
    // (not a fabricated "(0 countered)"), and the list rows + slice-13 per-row flags still
    // render.
    assert_eq!(
        page.status, 200,
        "a failed countered-peer-count read on /peer-claims must degrade to a 200 page, \
         NEVER a 5xx (C-2 CARDINAL); got {}",
        page.status
    );
    assert_peer_claims_header_countered_missing(&page.body);
    // The list still renders the countered peer row's slice-13 per-row flag (the rows are
    // an INDEPENDENT read — the header missing the count must not blank the list / flags).
    let countered_cid = seeded
        .countered_cids
        .first()
        .expect("the seeded peer-claims list must carry one countered cid");
    assert!(
        page.body_contains(countered_cid),
        "a failed header count must NOT blank the `/peer-claims` list — the rows + slice-13 \
         per-row flags render independently (ADR-056 D4); the countered row {countered_cid:?} \
         was missing from body:\n{}",
        page.body
    );
}

// =============================================================================
// Theme 5 — the count never re-weights, re-orders, filters, or deducts
// (US-PC-001/002; C-4). The peer-claims "4" is unchanged; the slice-18 own line
// untouched; the /peer-claims list order/paging + the slice-13 per-row flags are
// byte-identical baseline.
// =============================================================================

/// PC-NO-REWEIGHT (US-PC-001 Theme 5 / C-4; AC "The countered count never re-weights the
/// peer-claims count, and the own line is untouched"): Maria caches 4 peer claims, 1
/// countered, and has 12 own claims, 3 countered. When she opens GET /, the peer-claims
/// count renders "4" EXACTLY (the countered count is additive awareness, never a
/// deduction), the slice-18 own line still renders "12 own claims (3 countered)" unchanged,
/// and the front door contains NO penalty, score, "refuted", or "false" language.
///
/// Given Maria caches 4 peer claims, 1 countered, and has 12 own claims, 3 countered;
/// When she opens GET /;
/// Then the peer-claims count renders "4" exactly (additive, never a deduction), the
///   slice-18 own line still renders "12 own claims (3 countered)" unchanged, and the front
///   door contains no penalty/score/"refuted"/"false" language.
///
/// @us-pc-001 @driving_port @real-io @additive @c-4 @anti-misread @no-regression @happy
#[test]
fn the_countered_count_never_re_weights_the_peer_claims_count_and_own_line_untouched() {
    // GIVEN 4 peer claims (Priya's), 1 countered, AND 12 own claims, 3 countered (slice-18
    // shape). The peer-claims total over the combined store is the genuine count.
    let env = TestEnv::initialized();
    let _held = seed_landing_store_with_countered_peer_and_own(&env);
    let peer_total = landing_peer_total(&env);

    // WHEN Maria opens GET /.
    let viewer = ViewerServer::start(&env);
    let page = viewer.get(LANDING_PATH);

    // THEN the peer-claims count renders the genuine total EXACTLY (additive, never a
    // deduction — the count is NOT total - 1), the slice-18 own line is UNTOUCHED, AND the
    // copy is neutral.
    assert_eq!(page.status, 200, "the landing page must render");
    assert_landing_shows_count(&page.body, "peer claims", peer_total);
    assert_landing_peer_countered_count(&page.body, COUNTERED_PEER_CLAIMS);
    // The peer-claims count is NOT deducted by the countered count (never "total-1 peer claims").
    assert!(
        !page.body_contains(&format!("{} peer claims", peer_total - COUNTERED_PEER_CLAIMS)),
        "the peer-claims count must NOT be re-weighted/deducted by the countered count — it \
         renders the genuine total beside the count, never total-minus-countered (C-4 \
         additive); body was:\n{}",
        page.body
    );
    // The slice-18 own line ("12 own claims (3 countered)") is BYTE-UNTOUCHED (WD-PC-7).
    assert_landing_own_line_untouched(&page.body);
    assert_countered_copy_is_neutral(&page.body);
}

/// PC-NO-REORDER (US-PC-002 Theme 5 / C-4 / WD-PC-9; AC "The `/peer-claims` header count
/// does not re-order, filter, or re-weight the list"): Maria's store caches a mix of
/// countered and un-countered peer claims spanning the page. When she opens GET /peer-claims,
/// the row order (`composed_at DESC`), page boundaries, total count, every row's confidence,
/// and every row's peer origin are BYTE-IDENTICAL to a render without the header count (the
/// countered rows are not pulled to the top or grouped). The header count is additive header
/// text — never a transform of the list. The slice-13 per-row flags stay byte-identical too.
///
/// Given Maria's store caches a mix of countered and un-countered peer claims spanning the
///   page;
/// When she opens GET /peer-claims;
/// Then the row order, page boundaries, total count, every row's confidence, and every row's
///   peer origin are byte-identical to a render without the header count, and the countered
///   rows are not pulled to the top or grouped.
///
/// @us-pc-002 @driving_port @real-io @additive @no-regression @c-4 @wd-pc-9 @happy
#[test]
fn the_peer_claims_header_count_does_not_re_order_filter_or_re_weight_the_list() {
    // GIVEN a mix of countered + un-countered peer claims (reuse the slice-13
    // `seed_peer_claims_one_countered` fixture: several peer claims with a known countered
    // subset of 1, ordered by the slice-06/07 `/peer-claims` render order).
    let env = TestEnv::initialized();
    let seeded = seed_peer_claims_one_countered(&env);

    // Derive the EXPECTED header count from the direct ADR-056 oracle over the seeded store.
    let expected_countered = read_countered_peer_claims_count(&env);
    assert!(
        expected_countered >= 1,
        "the one-countered fixture must counter ≥1 peer claim so the header count is \
         falsifiable; got {expected_countered}"
    );

    // WHEN Maria opens GET /peer-claims (the render now carries the additive header count).
    let viewer = ViewerServer::start(&env);
    let page = viewer.get(PEER_CLAIMS_LIST_PATH);

    // THEN (a) the header RENDERS the additive countered count (the feature is present — so
    // this scenario cannot pass vacuously against the slice-13 baseline before the count is
    // implemented; No Fixture Theater), AND (b) the list order / paging / total count /
    // every row's confidence / peer origin + the slice-13 per-row flags are byte-identical
    // to the slice-06/07 baseline (the header count lives OUTSIDE the row body, and the
    // slice-13 flag anchors elide cleanly) — the header count re-ordered/filtered/re-weighted
    // NOTHING (C-4 / WD-PC-9). REUSES the slice-13 byte-identity assert.
    assert_eq!(page.status, 200, "the /peer-claims page must render");
    // (a) the additive header count is present (drives the feature — kills the vacuous pass).
    assert_peer_claims_header_countered_count(&page.body, expected_countered);
    // (b) the list itself is byte-identical to the no-header-count slice-06/07 baseline.
    assert_peer_claims_order_byte_identical(&page.body, &seeded.ordered_cids);
}

// =============================================================================
// Theme 6 — read-only / no write control / no key (US-PC-001/002; C-1 CARDINAL):
// the counter-aware / and /peer-claims expose no write/compose/sign/subscribe/
// follow control; the countered count is render-only text, not a sort/filter/
// mutating control.
// =============================================================================

/// PC-READONLY-LANDING (US-PC-001 Theme 6 / C-1 CARDINAL; AC "The counter-aware front door
/// exposes no write, compose, sign, subscribe, or follow control"): when Maria inspects the
/// GET / page, it contains no form, no button, and no control to compose, sign, subscribe,
/// or follow — every navigation affordance is a plain link, not a mutating control, and the
/// viewer process holds no signing key.
///
/// Given Maria opens GET /;
/// When she inspects the rendered page;
/// Then it contains no form, no button, and no compose/sign/subscribe/follow control —
///   every navigation affordance is a plain link, not a mutating control.
///
/// @us-pc-001 @driving_port @real-io @read-only @c-1 @cardinal @happy
#[test]
fn the_counter_aware_front_door_exposes_no_write_control() {
    // GIVEN a seeded counter-aware peer store (so the no-control scan is over REAL content).
    let env = TestEnv::initialized();
    let _held = seed_landing_store_with_countered_peer_claims(&env);

    // WHEN Maria opens GET / and inspects the page.
    let viewer = ViewerServer::start(&env);
    let page = viewer.get(LANDING_PATH);

    // THEN the page renders AND carries no write/compose/sign/subscribe/follow control —
    // the countered count is render-only text, never a mutating control (C-1 CARDINAL).
    assert_eq!(
        page.status, 200,
        "the landing page must render (200) so the no-control scan is over REAL content"
    );
    assert_landing_peer_countered_count(&page.body, COUNTERED_PEER_CLAIMS);
    assert_landing_read_only_no_control(&page.body);
}

/// PC-READONLY-HEADER (US-PC-002 Theme 6 / C-1 CARDINAL; AC "The counter-aware
/// `/peer-claims` header adds no write control"): when Maria inspects the GET /peer-claims
/// header, the countered count is render-only text, NOT a sort, filter, or mutating control,
/// and the route adds no write, compose, sign, subscribe, or follow affordance.
///
/// Given Maria opens GET /peer-claims;
/// When she inspects the rendered header;
/// Then the countered count is render-only text, not a sort/filter/mutating control, and
///   the route adds no write/compose/sign/subscribe/follow affordance.
///
/// @us-pc-002 @driving_port @real-io @read-only @c-1 @cardinal @happy
#[test]
fn the_counter_aware_peer_claims_header_adds_no_write_control() {
    // GIVEN a seeded counter-aware peer store.
    let env = TestEnv::initialized();
    let _held = seed_landing_store_with_countered_peer_claims(&env);

    // WHEN Maria opens GET /peer-claims and inspects the header.
    let viewer = ViewerServer::start(&env);
    let page = viewer.get(PEER_CLAIMS_LIST_PATH);

    // THEN the header shows the render-only countered count AND carries no write/compose/
    // sign/subscribe/follow control — the count is render-only text (C-1 CARDINAL). REUSES
    // the slice-17 banned-control scan.
    assert_eq!(page.status, 200, "the /peer-claims page must render");
    assert_peer_claims_header_countered_count(&page.body, COUNTERED_PEER_CLAIMS);
    assert_landing_read_only_no_control(&page.body);
}

// =============================================================================
// Theme 7 — LOCAL / offline (US-PC-001/002; C-2 CARDINAL): both surfaces render
// fully network-down; the countered-peer count is a LOCAL aggregate with no
// outbound edge; vendored htmx only (no CDN).
// =============================================================================

/// PC-OFFLINE-LANDING (US-PC-001 Theme 7 / C-2 CARDINAL; AC "The front door peer countered
/// count renders fully with the network down"): Maria's store caches countered peer claims
/// and the network is unavailable. When she opens GET /, the landing summary including the
/// peer countered count renders, no outbound network request is made by the route (the count
/// is a LOCAL `COUNT(DISTINCT)` aggregate — no PDS fetch, no DID re-resolution, no peer
/// pull), and the page references only the vendored local /static/htmx.min.js (no CDN).
///
/// Given Maria's store caches countered peer claims and the network is unavailable;
/// When she opens GET /;
/// Then the landing summary including the peer countered count renders, and the page
///   references only the vendored local htmx asset (no CDN).
///
/// @us-pc-001 @driving_port @real-io @offline @no-cdn @c-2 @cardinal @happy
#[test]
fn the_front_door_peer_countered_count_renders_fully_with_the_network_down() {
    // GIVEN a seeded counter-aware peer store. The viewer is started with NO network
    // reachability wired — the count is LOCAL by construction, so an absent network is
    // exactly the operator's offline machine.
    let env = TestEnv::initialized();
    let _held = seed_landing_store_with_countered_peer_claims(&env);

    // WHEN Maria opens GET / (offline — `ViewerServer::start` wires no outbound seam).
    let viewer = ViewerServer::start(&env);
    let page = viewer.get(LANDING_PATH);

    // THEN the summary + the peer countered count render fully offline, and the page
    // references only the vendored local htmx asset — never a CDN.
    assert_eq!(page.status, 200, "GET / must render fully offline (200)");
    assert_landing_shows_count(&page.body, "peer claims", LANDING_COUNTERED_PEER_TOTAL);
    assert_landing_peer_countered_count(&page.body, COUNTERED_PEER_CLAIMS);
    assert!(
        !page.references_external_cdn(),
        "the front door must reference only the vendored local /static/htmx.min.js — no \
         off-host CDN (C-2 / KPI-HX-G2); body was:\n{}",
        page.body
    );
}

/// PC-OFFLINE-HEADER (US-PC-002 Theme 7 / C-2 CARDINAL; AC "The `/peer-claims` header
/// countered count renders offline"): the network is unreachable and Maria's store caches
/// countered peer claims. When she opens GET /peer-claims, the header countered count
/// renders, and no network call is made by the route.
///
/// Given the network is unreachable and Maria's store caches countered peer claims;
/// When she opens GET /peer-claims;
/// Then the header countered count renders, and no network call is made by the route.
///
/// @us-pc-002 @driving_port @real-io @offline @no-cdn @c-2 @cardinal @happy
#[test]
fn the_peer_claims_header_countered_count_renders_offline() {
    // GIVEN a seeded counter-aware peer store, viewer started with no outbound seam.
    let env = TestEnv::initialized();
    let _held = seed_landing_store_with_countered_peer_claims(&env);

    // WHEN Maria opens GET /peer-claims offline.
    let viewer = ViewerServer::start(&env);
    let page = viewer.get(PEER_CLAIMS_LIST_PATH);

    // THEN the header countered count renders fully offline, and the page references only
    // the vendored local htmx asset.
    assert_eq!(page.status, 200, "GET /peer-claims must render fully offline (200)");
    assert_peer_claims_header_countered_count(&page.body, COUNTERED_PEER_CLAIMS);
    assert!(
        !page.references_external_cdn(),
        "the `/peer-claims` header must reference only the vendored local /static/htmx.min.js \
         — no off-host CDN (C-2); body was:\n{}",
        page.body
    );
}

// =============================================================================
// Theme 8 — no N+1: the countered-peer count is a fixed aggregate read
// (US-PC-000; C-3). A large store with a known countered subset renders the right
// count in one request, invariant to store size.
// =============================================================================

/// PC-NO-N-PLUS-1 (US-PC-000 Theme 8 / C-3 CARDINAL; AC "The countered-peer-claims count is
/// a fixed aggregate read, invariant to store size"): Maria's store caches many peer claims
/// with a KNOWN countered subset. When she opens GET /, the countered-peer count resolves to
/// the right number in a FIXED set of aggregate reads (the landing's read budget grows by
/// exactly one — a 5th count read), invariant to store size — no per-claim counter-presence
/// loop. The behavioral proxy: a LARGE store with a known countered subset renders the right
/// count in ONE request (the strict 1-query bound is a DELIVER adapter-duckdb unit/property
/// test).
///
/// Given Maria's store caches a LARGE number of peer claims with a known countered subset of 1;
/// When she opens GET /;
/// Then the countered-peer count resolves to 1 in one request — invariant to store size (no
///   per-claim loop), the landing read budget growing by exactly one (a 5th count read).
///
/// @us-pc-000 @property @driving_port @real-io @no-n-plus-1 @c-3 @cardinal @boundary
#[test]
fn the_countered_peer_count_is_a_fixed_aggregate_read_invariant_to_store_size() {
    // GIVEN a LARGE store: the seeded counter-aware peer store (1 countered) + many MORE
    // plain cached peer claims (none countered) — so a per-row counter-presence loop would
    // be observably costly / mis-count, while the single aggregate read returns the right
    // number invariant to store size.
    const LARGE_EXTRA_PEER_CLAIMS: usize = 120;
    let env = TestEnv::initialized();
    let _held = seed_landing_store_with_countered_peer_claims(&env);
    // Pile on many MORE plain cached peer claims (none countered) — the countered-peer count
    // must stay 1 (invariant to the peer-claims total).
    seed_extra_plain_peer_claims(&env, LARGE_EXTRA_PEER_CLAIMS);
    // Pin: the countered-peer count is STILL 1 over the large store (the ADR-056 oracle).
    let countered = read_countered_peer_claims_count(&env);
    assert_eq!(
        countered, COUNTERED_PEER_CLAIMS,
        "the countered-peer count must stay {COUNTERED_PEER_CLAIMS} over a large store \
         (invariant to store size — the extra plain peer claims draw no counter); got \
         {countered}"
    );

    // WHEN Maria opens GET / over the large store.
    let viewer = ViewerServer::start(&env);
    let page = viewer.get(LANDING_PATH);

    // THEN the countered-peer count renders correctly (1) in ONE request — the aggregate
    // read returns the right total invariant to store size (a per-row loop would miscount /
    // be observably slow; the strict 1-query bound is a DELIVER adapter-duckdb unit test).
    assert_eq!(page.status, 200, "the large store must render in one request");
    assert_landing_peer_countered_count(&page.body, COUNTERED_PEER_CLAIMS);
}

// =============================================================================
// Theme 9 — anti-misread: neutral disputed-claim awareness copy (US-PC-001/002;
// C-6 / WD-PC-10): the "(N countered)" copy is never a verdict/penalty; the
// countered peer claim's confidence renders verbatim, never re-weighted by the count.
// =============================================================================

/// PC-ANTI-MISREAD (US-PC-001 Theme 9 / C-6 / WD-PC-10; AC "The countered-peer count is
/// neutral disputed-claim awareness, never a verdict or penalty"): Maria's cached peer claim
/// (Tobias's, confidence 0.40) is countered by both Maria and Rachel, her only countered
/// peer claim. When she opens GET /, the landing summary shows "4 peer claims (1 countered)"
/// — a neutral presence count — its confidence (when she drills in) renders 0.40 VERBATIM
/// (never re-weighted by the count), and the copy is never "refuted", "false", "disputed by
/// 2", a score, or a deduction.
///
/// Given Maria's cached peer claim (Tobias's, confidence 0.40) is countered by both Maria
///   and Rachel, her only countered peer claim;
/// When she opens GET /;
/// Then the landing summary shows "(1 countered)" — a neutral presence count — and the copy
///   is never "refuted", "false", "disputed by 2", a score, or a deduction.
///
/// @us-pc-001 @driving_port @real-io @anti-misread @c-6 @wd-pc-10 @presence-once @boundary
#[test]
fn the_countered_peer_count_is_neutral_awareness_never_a_verdict_or_penalty() {
    // GIVEN one cached peer claim (confidence 0.40) countered by TWO counterers, her only
    // countered peer claim — the count is "(1 countered)" (presence-once).
    let env = TestEnv::initialized();
    let _held = seed_landing_store_one_peer_claim_countered_twice(&env);

    // WHEN Maria opens GET /.
    let viewer = ViewerServer::start(&env);
    let page = viewer.get(LANDING_PATH);

    // THEN the landing shows the neutral presence count "(1 countered)" (never
    // "(2 countered)"), AND the copy carries NO verdict/penalty/"disputed by N" language.
    assert_eq!(page.status, 200, "the landing page must render");
    assert_landing_peer_countered_count(&page.body, 1);
    assert!(
        !page.body_contains("(2 countered)"),
        "the count is a neutral PRESENCE count, never a \"by N\" total — must not show \
         \"(2 countered)\" for a peer claim countered by 2 counterers (C-6 / WD-PC-10); body \
         was:\n{}",
        page.body
    );
    assert_countered_copy_is_neutral(&page.body);
}
