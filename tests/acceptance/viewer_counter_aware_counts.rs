//! Slice-18 acceptance — the `openlore ui` COUNTERED-OWN-CLAIMS COUNT surfaced on the
//! `GET /` landing summary AND in the `GET /claims` list header: "12 own claims
//! (3 countered)" (US-CC-000/001/002; ADR-055).
//!
//! slice-17 turned `GET /` into a read-only orientation front door with a three-count
//! `LandingSummary` (own claims, peer claims, active peers), each an `Option<usize>`
//! degrading independently via `.ok()`. slice-18 EXTENDS that summary with a FOURTH
//! additive `Option<usize>` field `countered_own_claims` and threads the SAME number
//! into the `GET /claims` list header — answering, at a glance, "how many of my own
//! claims have been countered?". The count is `count_countered_own_claims()` =
//! `COUNT(DISTINCT c.cid) FROM claims c WHERE c.cid IN (SELECT referenced_cid FROM
//! claim_references WHERE ref_type='counters' UNION SELECT referenced_cid FROM
//! peer_claim_references WHERE ref_type='counters')` — a PRESENCE count (a claim
//! countered by N peers counts ONCE, no JOIN-fanout), own-only by query shape, invariant
//! to store size (ADR-055 D1). A shared pure `render_countered(Option<usize>) -> String`
//! helper renders the SAME parenthetical on BOTH surfaces (single source — ADR-055 D3):
//! `Some(3) → "(3 countered)"`, `Some(0) → "(0 countered)"` (honest zero), `None → "(—
//! countered)"` (missing marker). The own-claims "12" + the slice-06 list order/paging +
//! the slice-12 per-row flags are BYTE-IDENTICAL to the no-counter-count baseline (the
//! countered count is additive header text — C-4 / WD-CC-9). Read-only / no-key, LOCAL /
//! offline, missing≠zero, presence-once, anti-misread neutral copy.
//!
//! Driving discipline (Mandate 1): every scenario enters through the REAL `openlore ui`
//! subprocess (`ViewerServer::start`) + in-test HTTP GET against `/` and `/claims` — NO
//! scenario calls `viewer-domain::render_landing` / `render_countered` / the count read
//! directly (those are unit/property-level, exercised in DELIVER). The local DuckDB store
//! is REAL, seeded through the PRODUCTION write paths (own claims via `claim add`; the
//! peer counters that flag own claims via `peer add` + `peer pull` of verifiable
//! `counters`-referencing peer records — the self-counter rule means an OWN claim is
//! countered ONLY by a PEER, so the countered own-claim CID lands as a countered
//! `referenced_cid` in `peer_claim_references`), so the rows the count read returns are
//! produced by production code, not hand-inserted (Pillar 3 / BR-VIEW-4). NO
//! external/network boundary exists — `/` and `/claims` are LOCAL + OFFLINE (the count is
//! a LOCAL `COUNT(DISTINCT)` aggregate with no outbound edge). Every assertion is on the
//! rendered HTML the operator's browser shows (Mandate 8 universe = port-exposed rendered
//! surface).
//!
//! Layer placement (nw-tdd-methodology Layered Test Discipline matrix + Mandate 9/11):
//! every scenario is a layer-3/layer-5 subprocess + real-I/O test — EXAMPLE-only. The sad
//! paths (honest "(0 countered)", failed countered-count read → "(— countered)") are
//! enumerated explicitly, never PBT-generated at this layer (the generative exploration
//! of the pure `render_countered` over the Some(0)/Some(n)/None cases is a layer-1/2
//! DELIVER concern). Tier B (state-machine PBT) is NOT warranted: the count is a
//! single-shot additive render with no chained ≥3-scenario journey and no domain-rich
//! input space (one `Option<usize>`) — Tier A example coverage is exact (Mandate 10 skip
//! criteria).
//!
//! Build-before-run note (carry into the DELIVER roadmap, mirrors slice-06..17): `cargo
//! test` does NOT rebuild a spawned binary automatically — the roadmap/run MUST `cargo
//! build` the `openlore` bin (the viewer) before running these ATs so
//! `ViewerServer::start` spawns the CURRENT viewer, not a stale one. The count is a LOCAL
//! DuckDB read — no second binary needed.
//!
//! Mandate 7 RED scaffolds: the ATs spawn the bin + HTTP, so they COMPILE now with the
//! new `seed_landing_store_with_countered_own_claims` /
//! `seed_landing_store_none_countered` /
//! `seed_landing_store_one_own_claim_countered_twice` /
//! `start_viewer_with_failing_countered_count` seeds +
//! `assert_landing_countered_count` / `assert_landing_countered_missing` /
//! `assert_claims_header_countered_count` /
//! `assert_landing_and_claims_countered_consistent` / `assert_countered_copy_is_neutral`
//! asserts (which compile — they drive the EXISTING `claim add`/`peer add`/`peer pull`
//! verbs + scan strings + read the REAL store). Each scenario body runs to a `GET /` or
//! `GET /claims` HTTP assertion that FAILS because the production routes do NOT render the
//! countered count yet, and `count_countered_own_claims` / `render_countered` / the 4th
//! `LandingSummary` field / the `render_claims_page` `Option<usize>` param do NOT exist —
//! so "(3 countered)" is ABSENT from both rendered bodies → classifies RED
//! (MISSING_FUNCTIONALITY), NOT BROKEN. The ATs drive the routes via subprocess HTTP
//! (never the Rust `render_landing` / `render_claims_page` signatures), so the production
//! signature changes (the 4th field + the `/claims` param) are DELIVER's job and do not
//! affect AT compilation. They stay RED until DELIVER's per-scenario RED→GREEN→COMMIT
//! cycles (ADR-025).
//!
//! Covers (acceptance-criteria.md, 9 themes for US-CC-001/002 + US-CC-000 read wiring):
//! - CC-WS (Theme 1, US-CC-001): WALKING SKELETON — GET / over a seeded store (12 own, 3
//!   countered by peers, one of them by 2 peers) renders "12 own claims (3 countered)"
//!   beside the unchanged own-claims count.
//! - Theme 1 Ex 2 (US-CC-002): GET /claims renders the SAME "(3 countered)" in the list
//!   header; landing count == /claims header count for the same store (single source).
//! - Theme 2: presence-once — an own claim countered by 2 distinct peers counts ONCE
//!   ("(1 countered)", never "(2 countered)").
//! - Theme 3: honest zero — own claims, NONE countered → "(0 countered)" (Some(0)),
//!   DISTINCT from the missing marker; the /claims list renders as slice-06.
//! - Theme 4: missing≠zero — a FAILED countered-count read → "(— countered)" on BOTH
//!   surfaces while the own-claims "12" + the other counts + the rows render, page 200.
//! - Theme 5: no re-weight / additive — the own-claims "12" is unchanged; the /claims
//!   list order/paging/count + the slice-12 per-row flags are byte-identical baseline.
//! - Theme 6: read-only / no-write — / and /claims render no write/sort/filter/mutating
//!   control from the count.
//! - Theme 7: LOCAL/offline — both render fully network-down; vendored htmx only.
//! - Theme 8 (US-CC-000): no-N+1 — the countered count is one aggregate read, invariant
//!   to store size (a large store with a known countered subset renders the right count).
//! - Theme 9: anti-misread — the "(N countered)" copy NEVER contains penalty/deduction/
//!   "disputed by N"/verdict language; the countered claim's confidence renders verbatim.
//
// SCAFFOLD: true

mod support;

#[allow(unused_imports)]
use support::*;

// =============================================================================
// CC-WS — WALKING SKELETON (US-CC-001 Theme 1; the riskiest-assumption thread):
// GET / over a seeded store (12 own, 3 countered by peers) renders "12 own claims
// (3 countered)" beside the unchanged own-claims count, on the existing landing
// dashboard. This is the thinnest complete thread the slice can demo: viewer → the
// LOCAL countered-count aggregate → the extended LandingSummary → pure render → the
// front-door summary showing the disputed-claim awareness count.
// =============================================================================

/// CC-WS / WALKING SKELETON (US-CC-001 Theme 1; AC "The front door shows how many of my
/// own claims are countered"): from the LOCAL store seeded with 12 own claims, 3 of which
/// are countered by peers (one of them by TWO distinct peers, proving presence-once),
/// `GET /` returns a 200 page that shows "12 own claims (3 countered)" — the countered
/// count beside the UNCHANGED own-claims count. The thinnest demo-able thread: the front
/// door now orients the operator on not just how much is in her store but how much of her
/// own work has been disputed, read-only, offline.
///
/// Given Maria's store has 12 own claims, 3 of which have ≥1 counter (one countered by
///   both Rachel and Tobias);
/// When she opens GET / in the openlore ui viewer;
/// Then the landing summary shows "12 own claims" with "(3 countered)" beside it, and the
///   own-claims count "12" is unchanged by the presence of the countered count.
///
/// @us-cc-001 @walking_skeleton @driving_port @driving_adapter @real-io @kpi-view-1 @happy
#[test]
fn the_front_door_shows_how_many_own_claims_are_countered() {
    // GIVEN a REAL local store seeded (production `claim add` + `peer add` + `peer pull`
    // paths, Pillar 3) with 12 own claims, 3 of them countered by peers (one by two).
    let env = TestEnv::initialized();
    let _held = seed_landing_store_with_countered_own_claims(&env);

    // WHEN Maria opens GET / in the viewer (full page — `/` is full-page-only, ADR-054 D5).
    let viewer = ViewerServer::start(&env);
    let page = viewer.get(LANDING_PATH);

    // THEN the front door renders a 200 page showing the own-claims count "12" UNCHANGED
    // AND the countered count "(3 countered)" beside it (the genuine seeded presence
    // count, ADR-055 D1/D3).
    assert_eq!(
        page.status, 200,
        "GET / must render the landing dashboard (200); got {}",
        page.status
    );
    // The own-claims count "12" still renders (additive — the countered count never
    // re-weights it, C-4).
    assert_landing_shows_count(&page.body, "own claims", LANDING_OWN_CLAIMS);
    // The NEW countered count "(3 countered)" renders beside it (US-CC-001 headline).
    assert_landing_countered_count(&page.body, COUNTERED_OWN_CLAIMS);
}

// =============================================================================
// Theme 1 Ex 2 — the `/claims` header shows the SAME disputed-claim awareness count
// (US-CC-002); landing count == /claims header count for the same store (single
// source — WD-CC-8).
// =============================================================================

/// CC-HEADER (US-CC-002 Theme 1 Ex 2; AC "The `/claims` header shows the same
/// disputed-claim awareness"): over the SAME store, `GET /claims` renders "(3 countered)"
/// in the list header — the SAME count the landing shows beside "12 own claims". Both
/// surfaces resolve the count from the SAME `count_countered_own_claims` read and render
/// through the SAME `render_countered` helper (single source — WD-CC-8 / ADR-055 D3), so
/// the landing and `/claims` counts cannot diverge.
///
/// Given Maria's store has 12 own claims, 3 of which have ≥1 counter;
/// When she opens GET /claims;
/// Then the list header shows "(3 countered)", the SAME count the landing shows beside
///   "12 own claims".
///
/// @us-cc-002 @driving_port @real-io @single-source @wd-cc-8 @happy
#[test]
fn the_claims_header_shows_the_same_countered_count_as_the_landing() {
    // GIVEN a store seeded with 12 own claims, 3 countered by peers.
    let env = TestEnv::initialized();
    let _held = seed_landing_store_with_countered_own_claims(&env);

    // WHEN Maria opens GET / AND GET /claims over the SAME store.
    let viewer = ViewerServer::start(&env);
    let landing = viewer.get(LANDING_PATH);
    let claims = viewer.get(CLAIMS_LIST_PATH);

    // THEN the `/claims` header shows the SAME "(3 countered)" the landing shows, and the
    // two counts are EQUAL (single source — WD-CC-8).
    assert_eq!(landing.status, 200, "GET / must render (200)");
    assert_eq!(claims.status, 200, "GET /claims must render (200)");
    assert_landing_countered_count(&landing.body, COUNTERED_OWN_CLAIMS);
    assert_claims_header_countered_count(&claims.body, COUNTERED_OWN_CLAIMS);
    assert_landing_and_claims_countered_consistent(&landing.body, &claims.body);
}

// =============================================================================
// Theme 2 — presence count: a twice-countered claim counts once (US-CC-000/001;
// C-4 / BR-CC-1). COUNT(DISTINCT) collapses two peer counters of the SAME own CID
// to ONE → "(1 countered)", never "(2 countered)".
// =============================================================================

/// CC-PRESENCE (US-CC-000/001 Theme 2; AC "A claim countered by multiple peers counts
/// once"): Maria's claim is countered by BOTH Rachel and Tobias, and it is her only
/// countered claim. When she opens GET /, the landing summary shows "(1 countered)", NOT
/// "(2 countered)" — the count is a PRESENCE count (the de-duped UNION IN-set +
/// COUNT(DISTINCT) collapses the two distinct-author counters of the SAME own CID to ONE
/// membership), and the copy shows NO "disputed by N" sum of counters.
///
/// Given Maria's claim is countered by both Rachel and Tobias, and it is her only
///   countered claim;
/// When she opens GET /;
/// Then the landing summary shows "(1 countered)", not "(2 countered)", and the count
///   shows no "disputed by N" sum of counters.
///
/// @us-cc-000 @us-cc-001 @driving_port @real-io @presence-once @c-4 @br-cc-1 @cardinal
/// @boundary
#[test]
fn a_claim_countered_by_multiple_peers_counts_once() {
    // GIVEN one own claim countered by TWO distinct peers (Rachel + Tobias), the only
    // countered claim — the COUNT(DISTINCT) must collapse it to ONE.
    let env = TestEnv::initialized();
    let _held = seed_landing_store_one_own_claim_countered_twice(&env);

    // WHEN Maria opens GET /.
    let viewer = ViewerServer::start(&env);
    let page = viewer.get(LANDING_PATH);

    // THEN the landing shows "(1 countered)", NEVER "(2 countered)" (presence-once), and
    // the copy carries no "disputed by N" total (anti-misread).
    assert_eq!(page.status, 200, "the landing page must render");
    assert_landing_countered_count(&page.body, 1);
    assert!(
        !page.body_contains("(2 countered)"),
        "a claim countered by TWO peers must count ONCE — the landing must NOT show \
         \"(2 countered)\" (presence-once, COUNT(DISTINCT); C-4 / BR-CC-1); body was:\n{}",
        page.body
    );
    assert_countered_copy_is_neutral(&page.body);
}

// =============================================================================
// Theme 3 — honest zero when nothing is countered (US-CC-001/002; C-5): Some(0) →
// "(0 countered)", a SUCCESSFUL read of zero, DISTINCT from the missing marker; the
// /claims list renders as slice-06.
// =============================================================================

/// CC-ZERO-LANDING (US-CC-001 Theme 3; AC "An honest (0 countered) on the landing"): a
/// store with 12 own claims, NONE of which has drawn a counter. When Maria opens GET /,
/// the landing shows "12 own claims (0 countered)" — "(0 countered)" is a SUCCESSFUL read
/// of zero (Some(0)), DISTINCT from the missing-number state "(— countered)" and not an
/// omitted count. The success side of `0 ≠ missing`.
///
/// Given Maria's store has 12 own claims, none of which has drawn a counter;
/// When she opens GET /;
/// Then the landing summary shows "12 own claims (0 countered)", and "(0 countered)" is a
///   successful read of zero, not a missing-number state and not an omitted count.
///
/// @us-cc-001 @driving_port @real-io @honest-zero @c-5 @edge
#[test]
fn an_honest_zero_countered_on_the_landing_when_nothing_is_disputed() {
    // GIVEN 12 own claims, NONE countered (the count read returns Some(0)).
    let env = TestEnv::initialized();
    let _held = seed_landing_store_none_countered(&env);

    // WHEN Maria opens GET /.
    let viewer = ViewerServer::start(&env);
    let page = viewer.get(LANDING_PATH);

    // THEN the landing shows the own-claims "12" AND "(0 countered)" (Some(0), honest
    // zero) — DISTINCT from the missing marker "(— countered)".
    assert_eq!(page.status, 200, "the landing page must render");
    assert_landing_shows_count(&page.body, "own claims", LANDING_OWN_CLAIMS);
    assert_landing_countered_count(&page.body, 0);
    // The honest zero is NOT the missing-number state (the success side of 0 ≠ missing).
    assert!(
        !page.body_contains("(— countered)"),
        "an honest empty-counter store renders \"(0 countered)\" (Some(0)), NOT the \
         missing-number marker \"(— countered)\" (which would mean a FAILED read; C-5); \
         body was:\n{}",
        page.body
    );
}

/// CC-ZERO-HEADER (US-CC-002 Theme 3; AC "An honest (0 countered) in the `/claims`
/// header, list renders as slice-06"): the SAME none-countered store. When Maria opens
/// GET /claims, the list header shows "(0 countered)" and the list renders its rows with
/// NO per-row Countered flags, exactly as slice-06 (nothing is countered, so the slice-12
/// presence read returns the empty set).
///
/// Given Maria's store has 12 own claims, none of which has drawn a counter;
/// When she opens GET /claims;
/// Then the list header shows "(0 countered)" and the list renders its rows with no
///   per-row Countered flags, exactly as slice-06.
///
/// @us-cc-002 @driving_port @real-io @honest-zero @no-noise @c-5 @edge
#[test]
fn an_honest_zero_countered_in_the_claims_header_list_as_slice_06() {
    // GIVEN 12 own claims, NONE countered.
    let env = TestEnv::initialized();
    let _held = seed_landing_store_none_countered(&env);

    // WHEN Maria opens GET /claims.
    let viewer = ViewerServer::start(&env);
    let page = viewer.get(CLAIMS_LIST_PATH);

    // THEN the header shows "(0 countered)" AND the list carries NO per-row "Countered"
    // flag (nothing is countered — the slice-12 presence read is empty; the rows render
    // exactly as slice-06).
    assert_eq!(page.status, 200, "the /claims page must render");
    assert_claims_header_countered_count(&page.body, 0);
    // No per-row flag anchor appears anywhere — the body carries no `>Countered<` marker
    // text (the header "(0 countered)" is the ONLY countered text, and it is not the
    // per-row flag shape `>Countered</a>`).
    assert!(
        !page.body_contains(&format!(">{LIST_COUNTERED_FLAG_TEXT}</a>")),
        "a none-countered store must render the `/claims` rows with NO per-row Countered \
         flag, exactly as slice-06 (the header \"(0 countered)\" is the only countered \
         text); body was:\n{}",
        page.body
    );
}

// =============================================================================
// Theme 4 — missing ≠ zero: a failed count read degrades independently, no 5xx
// (US-CC-000/001/002; C-2 / C-5 CARDINAL). Both surfaces render "(— countered)"
// while the sibling counts / rows render; page 200.
// =============================================================================

/// CC-DEGRADE-LANDING (US-CC-000/001 Theme 4 / C-2 / C-5 CARDINAL; AC "A failed
/// countered-count read degrades gracefully on the front door"): Maria's countered-count
/// read fails while the own-claims count succeeds. When she opens GET /, the own-claims
/// count + the rest of the summary + the nav hub still render, the countered count renders
/// as the missing-number state "(— countered)" (NOT a fabricated "(0 countered)"), and the
/// page is a normal 200 — never a 5xx, never a blanked summary. The per-count independent
/// degrade (`.ok()` → `None` → `render_countered(None)`, ADR-055 D4).
///
/// SEEDING-SEAM NOTE (documented DISTILL choice, slice-17 LD-DEGRADE precedent): the
/// viewer holds one long-lived DuckDB connection taken at startup, so there is no
/// readily-available mid-request per-count read-failure seam in the slice-06/15 harness.
/// This scenario drives the TEST-ONLY effect-shell fault seam
/// (`start_viewer_with_failing_countered_count` → the `OPENLORE_VIEWER_FAIL_COUNTERED_
/// COUNT` env var, materialized by DELIVER exactly as slice-17 materialized
/// `OPENLORE_VIEWER_FAIL_PEER_CLAIMS_COUNT`). The Some(0) success side is fully exercised
/// by the honest-zero scenarios above; this scenario pins the FAILURE side. Until DELIVER
/// materializes the seam the scenario fails at `start_inner`'s `todo!()` body → RED
/// MISSING_FUNCTIONALITY, never BROKEN.
///
/// Given Maria's countered-own-claims count read fails while the own-claims count
///   succeeds;
/// When she opens GET /;
/// Then the own-claims count + the rest of the summary + the nav hub still render, the
///   countered count renders as "(— countered)" (not a fabricated "(0 countered)"), and
///   the page is a normal 200.
///
/// @us-cc-000 @us-cc-001 @driving_port @real-io @infrastructure-failure @missing-not-zero
/// @c-2 @c-5 @cardinal @error
#[test]
fn a_failed_countered_count_read_degrades_gracefully_on_the_front_door() {
    // GIVEN a store seeded with 12 own + 3 countered, BUT the countered-count read forced
    // to FAIL mid-request (the own-claims count + the other reads still succeed).
    let env = TestEnv::initialized();
    let _held = seed_landing_store_with_countered_own_claims(&env);

    // WHEN Maria opens GET / against the viewer whose countered-count read fails.
    let viewer = start_viewer_with_failing_countered_count(&env);
    let page = viewer.get(LANDING_PATH);

    // THEN the page is a NORMAL 200 (never a 5xx), the own-claims count + the nav hub
    // still render, and the countered count renders the missing marker "(— countered)"
    // (NOT a fabricated "(0 countered)").
    assert_eq!(
        page.status, 200,
        "a failed countered-count read must degrade to a 200 page, NEVER a 5xx (C-2 \
         CARDINAL / NFR-VIEW-6); got {}",
        page.status
    );
    // The own-claims count STILL renders (the degrade is per-count, independent).
    assert_landing_shows_count(&page.body, "own claims", LANDING_OWN_CLAIMS);
    // The nav hub still renders in full (the failure is per-count, not page-wide).
    assert_landing_links_all_surfaces(&page.body);
    // The FAILED countered count renders the missing marker "(— countered)" (not 0).
    assert_landing_countered_missing(&page.body);
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

/// CC-DEGRADE-HEADER (US-CC-000/002 Theme 4 / C-2 / C-5 CARDINAL; AC "A failed header
/// count degrades without blanking the `/claims` list"): Maria's countered-count read
/// fails. When she opens GET /claims, the list header renders the missing-number state
/// "(— countered)" (not a fabricated "(0 countered)"), the list rows still render, and the
/// page is a normal 200 — the countered-count read is INDEPENDENT of the list read.
///
/// Given Maria's countered-own-claims count read fails;
/// When she opens GET /claims;
/// Then the list header renders "(— countered)" (not a fabricated "(0 countered)"), the
///   list rows still render, and the page is a normal 200.
///
/// @us-cc-000 @us-cc-002 @driving_port @real-io @infrastructure-failure @missing-not-zero
/// @c-2 @c-5 @cardinal @error
#[test]
fn a_failed_header_count_degrades_without_blanking_the_claims_list() {
    // GIVEN a store seeded with 12 own + 3 countered, BUT the countered-count read forced
    // to FAIL (the list read still succeeds).
    let env = TestEnv::initialized();
    let _held = seed_landing_store_with_countered_own_claims(&env);

    // WHEN Maria opens GET /claims against the viewer whose countered-count read fails.
    let viewer = start_viewer_with_failing_countered_count(&env);
    let page = viewer.get(CLAIMS_LIST_PATH);

    // THEN the page is a NORMAL 200, the header renders the missing marker "(— countered)"
    // (not a fabricated "(0 countered)"), and the list rows still render.
    assert_eq!(
        page.status, 200,
        "a failed countered-count read on /claims must degrade to a 200 page, NEVER a 5xx \
         (C-2 CARDINAL); got {}",
        page.status
    );
    assert_claims_header_countered_missing(&page.body);
    // The list still renders the My Claims body (the rows are an INDEPENDENT read — the
    // header missing the count must not blank the list).
    assert!(
        page.body_contains("My Claims"),
        "a failed header count must NOT blank the `/claims` list — the rows render \
         independently (ADR-055 D4); body was:\n{}",
        page.body
    );
}

// =============================================================================
// Theme 5 — the count never re-weights, re-orders, filters, or deducts
// (US-CC-001/002; C-4). The own-claims "12" is unchanged; the /claims list
// order/paging/count + the slice-12 per-row flags are byte-identical baseline.
// =============================================================================

/// CC-NO-REWEIGHT (US-CC-001 Theme 5 / C-4; AC "The countered count never re-weights the
/// own-claims count"): Maria has 12 own claims, 3 countered. When she opens GET /, the
/// own-claims count renders "12" EXACTLY (the countered count is additive awareness, never
/// a deduction), and the front door contains NO penalty, score, "refuted", or "false"
/// language.
///
/// Given Maria has 12 own claims, 3 countered;
/// When she opens GET /;
/// Then the own-claims count renders "12" exactly (the countered count is additive, never
///   a deduction), and the front door contains no penalty/score/"refuted"/"false"
///   language.
///
/// @us-cc-001 @driving_port @real-io @additive @c-4 @anti-misread @happy
#[test]
fn the_countered_count_never_re_weights_the_own_claims_count() {
    // GIVEN 12 own claims, 3 countered.
    let env = TestEnv::initialized();
    let _held = seed_landing_store_with_countered_own_claims(&env);

    // WHEN Maria opens GET /.
    let viewer = ViewerServer::start(&env);
    let page = viewer.get(LANDING_PATH);

    // THEN the own-claims count renders "12" EXACTLY (additive, never a deduction — the
    // count is NOT 12 - 3 = 9), AND the copy is neutral (no penalty/verdict language).
    assert_eq!(page.status, 200, "the landing page must render");
    assert_landing_shows_count(&page.body, "own claims", LANDING_OWN_CLAIMS);
    assert_landing_countered_count(&page.body, COUNTERED_OWN_CLAIMS);
    // The own-claims count is NOT deducted by the countered count (never shows 9 as the
    // own-claims number — the count is additive awareness).
    assert!(
        !page.body_contains(&format!(
            "{} own claims",
            LANDING_OWN_CLAIMS - COUNTERED_OWN_CLAIMS
        )),
        "the own-claims count must NOT be re-weighted/deducted by the countered count — \
         it renders \"12 own claims\", never \"9 own claims\" (C-4 additive); body was:\n{}",
        page.body
    );
    assert_countered_copy_is_neutral(&page.body);
}

/// CC-NO-REORDER (US-CC-002 Theme 5 / C-4 / WD-CC-9; AC "The `/claims` header count does
/// not re-order, filter, or re-weight the list"): Maria's store has a mix of countered and
/// un-countered claims. When she opens GET /claims, the row order (`composed_at DESC,
/// cid`), page boundaries, total count, and every row's confidence are BYTE-IDENTICAL to a
/// render without the header count (the countered rows are not pulled to the top or
/// grouped). The header count is additive header text — never a transform of the list.
///
/// Given Maria's store has a mix of countered and un-countered claims spanning the page;
/// When she opens GET /claims;
/// Then the row order, page boundaries, total count, and every row's confidence are
///   byte-identical to a render without the header count, and the countered rows are not
///   pulled to the top or grouped.
///
/// @us-cc-002 @driving_port @real-io @additive @no-regression @c-4 @wd-cc-9 @happy
#[test]
fn the_claims_header_count_does_not_re_order_filter_or_re_weight_the_list() {
    // GIVEN a mix of countered + un-countered own claims (reuse the slice-12 mixed-pages
    // fixture: N own claims with a known countered subset interleaved among them).
    let env = TestEnv::initialized();
    let _seeded = seed_claims_list_mixed_pages(&env);

    // Record the slice-06 baseline (order + paging + total + each row's confidence) of the
    // SAME store BEFORE the header count is rendered (the no-regression anchor). Derive the
    // EXPECTED header count from the direct ADR-055 oracle over the seeded store (the
    // mixed-pages fixture counters a known subset of own claims).
    let baseline = read_slice06_list_baseline(&env);
    let expected_countered = read_countered_own_claims_count(&env);
    assert!(
        expected_countered >= 1,
        "the mixed-pages fixture must counter ≥1 own claim so the header count is \
         falsifiable; got {expected_countered}"
    );

    // WHEN Maria opens GET /claims (the render now carries the additive header count).
    let viewer = ViewerServer::start(&env);
    let page = viewer.get(CLAIMS_LIST_PATH);

    // THEN (a) the header RENDERS the additive countered count (the feature is present — so
    // this scenario cannot pass vacuously against the slice-12 baseline before the count is
    // implemented; No Fixture Theater), AND (b) the list order / paging / total count /
    // every row's confidence are byte-identical to the slice-06 baseline (the header count
    // + the slice-12 per-row flag anchors elided) — the header count re-ordered/filtered/
    // re-weighted NOTHING (C-4 / WD-CC-9). REUSES the slice-12 byte-identity assert (which
    // elides the additive `<a href>Countered</a>` anchors; the header "(N countered)" lives
    // OUTSIDE the row body, so the row-order/confidence/position checks are unaffected).
    assert_eq!(page.status, 200, "the /claims page must render");
    // (a) the additive header count is present (drives the feature — kills the vacuous pass).
    assert_claims_header_countered_count(&page.body, expected_countered);
    // (b) the list itself is byte-identical to the no-header-count baseline.
    assert_list_order_and_confidence_byte_identical(&page.body, &baseline);
}

// =============================================================================
// Theme 6 — read-only / no write control / no key (US-CC-001/002; C-1 CARDINAL):
// the counter-aware / and /claims expose no write/compose/sign/subscribe/follow
// control; the countered count is render-only text, not a sort/filter/mutating
// control.
// =============================================================================

/// CC-READONLY-LANDING (US-CC-001 Theme 6 / C-1 CARDINAL; AC "The counter-aware front door
/// exposes no write, compose, sign, subscribe, or follow control"): when Maria inspects
/// the GET / page, it contains no form, no button, and no control to compose, sign,
/// subscribe, or follow — every navigation affordance is a plain link, not a mutating
/// control, and the viewer process holds no signing key. (The no-key guarantee is
/// structural — slice-06 `web_process_holds_no_signing_key` gold + xtask check-arch; here
/// the operator-facing surface carries no mutating control even with the count present.)
///
/// Given Maria opens GET /;
/// When she inspects the rendered page;
/// Then it contains no form, no button, and no compose/sign/subscribe/follow control —
///   every navigation affordance is a plain link, not a mutating control.
///
/// @us-cc-001 @driving_port @real-io @read-only @c-1 @cardinal @happy
#[test]
fn the_counter_aware_front_door_exposes_no_write_control() {
    // GIVEN a seeded counter-aware store (so the no-control scan is over REAL content).
    let env = TestEnv::initialized();
    let _held = seed_landing_store_with_countered_own_claims(&env);

    // WHEN Maria opens GET / and inspects the page.
    let viewer = ViewerServer::start(&env);
    let page = viewer.get(LANDING_PATH);

    // THEN the page renders AND carries no write/compose/sign/subscribe/follow control —
    // the countered count is render-only text, never a mutating control (C-1 CARDINAL).
    assert_eq!(
        page.status, 200,
        "the landing page must render (200) so the no-control scan is over REAL content"
    );
    assert_landing_countered_count(&page.body, COUNTERED_OWN_CLAIMS);
    assert_landing_read_only_no_control(&page.body);
}

/// CC-READONLY-HEADER (US-CC-002 Theme 6 / C-1 CARDINAL; AC "The counter-aware `/claims`
/// header adds no write control"): when Maria inspects the GET /claims header, the
/// countered count is render-only text, NOT a sort, filter, or mutating control, and the
/// route adds no write, compose, sign, subscribe, or follow affordance.
///
/// Given Maria opens GET /claims;
/// When she inspects the rendered header;
/// Then the countered count is render-only text, not a sort/filter/mutating control, and
///   the route adds no write/compose/sign/subscribe/follow affordance.
///
/// @us-cc-002 @driving_port @real-io @read-only @c-1 @cardinal @happy
#[test]
fn the_counter_aware_claims_header_adds_no_write_control() {
    // GIVEN a seeded counter-aware store.
    let env = TestEnv::initialized();
    let _held = seed_landing_store_with_countered_own_claims(&env);

    // WHEN Maria opens GET /claims and inspects the header.
    let viewer = ViewerServer::start(&env);
    let page = viewer.get(CLAIMS_LIST_PATH);

    // THEN the header shows the render-only countered count AND carries no write/compose/
    // sign/subscribe/follow control — the count is render-only text, never a sort/filter/
    // mutating control (C-1 CARDINAL). REUSES the slice-17 banned-control scan.
    assert_eq!(page.status, 200, "the /claims page must render");
    assert_claims_header_countered_count(&page.body, COUNTERED_OWN_CLAIMS);
    assert_landing_read_only_no_control(&page.body);
}

// =============================================================================
// Theme 7 — LOCAL / offline (US-CC-001/002; C-2 CARDINAL): both surfaces render
// fully network-down; the countered count is a LOCAL aggregate with no outbound
// edge; vendored htmx only (no CDN).
// =============================================================================

/// CC-OFFLINE-LANDING (US-CC-001 Theme 7 / C-2 CARDINAL; AC "The front door countered
/// count renders fully with the network down"): Maria's store has countered claims and the
/// network is unavailable. When she opens GET /, the landing summary including the
/// countered count renders, no outbound network request is made by the route (the count is
/// a LOCAL `COUNT(DISTINCT)` aggregate — no PDS fetch, no DID re-resolution, no peer pull),
/// and the page references only the vendored local /static/htmx.min.js (no CDN).
///
/// Given Maria's store has countered claims and the network is unavailable;
/// When she opens GET /;
/// Then the landing summary including the countered count renders, and the page references
///   only the vendored local htmx asset (no CDN).
///
/// @us-cc-001 @driving_port @real-io @offline @no-cdn @c-2 @cardinal @happy
#[test]
fn the_front_door_countered_count_renders_fully_with_the_network_down() {
    // GIVEN a seeded counter-aware store. The viewer is started with NO network
    // reachability wired — the count is LOCAL by construction, so an absent network is
    // exactly the operator's offline machine.
    let env = TestEnv::initialized();
    let _held = seed_landing_store_with_countered_own_claims(&env);

    // WHEN Maria opens GET / (offline — `ViewerServer::start` wires no outbound seam).
    let viewer = ViewerServer::start(&env);
    let page = viewer.get(LANDING_PATH);

    // THEN the summary + the countered count render fully offline, and the page references
    // only the vendored local htmx asset — never a CDN.
    assert_eq!(page.status, 200, "GET / must render fully offline (200)");
    assert_landing_shows_count(&page.body, "own claims", LANDING_OWN_CLAIMS);
    assert_landing_countered_count(&page.body, COUNTERED_OWN_CLAIMS);
    assert!(
        !page.references_external_cdn(),
        "the front door must reference only the vendored local /static/htmx.min.js — no \
         off-host CDN (C-2 / KPI-HX-G2); body was:\n{}",
        page.body
    );
}

/// CC-OFFLINE-HEADER (US-CC-002 Theme 7 / C-2 CARDINAL; AC "The `/claims` header countered
/// count renders offline"): the network is unreachable and Maria's store holds countered
/// claims. When she opens GET /claims, the header countered count renders, and no network
/// call is made by the route.
///
/// Given the network is unreachable and Maria's store holds countered claims;
/// When she opens GET /claims;
/// Then the header countered count renders, and no network call is made by the route.
///
/// @us-cc-002 @driving_port @real-io @offline @no-cdn @c-2 @cardinal @happy
#[test]
fn the_claims_header_countered_count_renders_offline() {
    // GIVEN a seeded counter-aware store, viewer started with no outbound seam.
    let env = TestEnv::initialized();
    let _held = seed_landing_store_with_countered_own_claims(&env);

    // WHEN Maria opens GET /claims offline.
    let viewer = ViewerServer::start(&env);
    let page = viewer.get(CLAIMS_LIST_PATH);

    // THEN the header countered count renders fully offline, and the page references only
    // the vendored local htmx asset.
    assert_eq!(page.status, 200, "GET /claims must render fully offline (200)");
    assert_claims_header_countered_count(&page.body, COUNTERED_OWN_CLAIMS);
    assert!(
        !page.references_external_cdn(),
        "the `/claims` header must reference only the vendored local /static/htmx.min.js \
         — no off-host CDN (C-2); body was:\n{}",
        page.body
    );
}

// =============================================================================
// Theme 8 — no N+1: the countered count is a fixed aggregate read (US-CC-000; C-3).
// A large store with a known countered subset renders the right count in one request.
// =============================================================================

/// CC-NO-N-PLUS-1 (US-CC-000 Theme 8 / C-3 CARDINAL; AC "The countered-own-claims count is
/// a fixed aggregate read, invariant to store size"): Maria's store has many own claims
/// with a KNOWN countered subset. When she opens GET /, the countered count resolves to the
/// right number in a FIXED set of aggregate reads (the landing's read budget grows by at
/// most one), invariant to store size — no per-claim counter-presence loop. The behavioral
/// proxy: a LARGE store with a known countered subset renders the right count in ONE
/// request (the strict 1-query bound is a DELIVER adapter-duckdb unit/property test).
///
/// Given Maria's store has a LARGE number of own claims with a known countered subset of 3;
/// When she opens GET /;
/// Then the countered count resolves to 3 in one request — invariant to store size (no
///   per-claim loop).
///
/// @us-cc-000 @property @driving_port @real-io @no-n-plus-1 @c-3 @cardinal @boundary
#[test]
fn the_countered_count_is_a_fixed_aggregate_read_invariant_to_store_size() {
    // GIVEN a LARGE store: the 3 named countered own claims + many more PLAIN own claims
    // (well beyond the headline 12) — so a per-row counter-presence loop would be
    // observably costly / mis-count, while the single aggregate read returns the right
    // number invariant to store size.
    const LARGE_EXTRA_OWN_CLAIMS: usize = 120;
    let env = TestEnv::initialized();
    let _held = seed_landing_store_with_countered_own_claims(&env);
    // Pile on many MORE plain own claims (none countered) — the countered count must stay
    // 3 (invariant to the own-claims total).
    seed_own_claims_via_cli(&env, LARGE_EXTRA_OWN_CLAIMS);
    // Pin: the countered count is STILL 3 over the large store (the ADR-055 oracle).
    let countered = read_countered_own_claims_count(&env);
    assert_eq!(
        countered, COUNTERED_OWN_CLAIMS,
        "the countered count must stay {COUNTERED_OWN_CLAIMS} over a large store (invariant \
         to store size — the extra plain claims draw no counter); got {countered}"
    );

    // WHEN Maria opens GET / over the large store.
    let viewer = ViewerServer::start(&env);
    let page = viewer.get(LANDING_PATH);

    // THEN the countered count renders correctly (3) in ONE request — the aggregate read
    // returns the right total invariant to store size (a per-row loop would miscount / be
    // observably slow; the strict 1-query bound is a DELIVER adapter-duckdb unit test).
    assert_eq!(page.status, 200, "the large store must render in one request");
    assert_landing_countered_count(&page.body, COUNTERED_OWN_CLAIMS);
}

// =============================================================================
// Theme 9 — anti-misread: neutral disputed-claim awareness copy (US-CC-001/002;
// C-6 / WD-CC-10): the "(N countered)" copy is never a verdict/penalty; the
// countered claim's confidence renders verbatim, never re-weighted by the count.
// =============================================================================

/// CC-ANTI-MISREAD (US-CC-001 Theme 9 / C-6 / WD-CC-10; AC "The countered count is neutral
/// disputed-claim awareness, never a verdict or penalty"): Maria's claim (confidence 0.30)
/// is countered by both Rachel and Tobias, her only countered claim. When she opens GET /,
/// the landing summary shows "(1 countered)" — a neutral presence count — its confidence
/// (when she drills in) renders 0.30 VERBATIM (never re-weighted by the count), and the
/// copy is never "refuted", "false", "disputed by 2", a score, or a deduction.
///
/// Given Maria's claim (confidence 0.30) is countered by both Rachel and Tobias, her only
///   countered claim;
/// When she opens GET /;
/// Then the landing summary shows "(1 countered)" — a neutral presence count — and the
///   copy is never "refuted", "false", "disputed by 2", a score, or a deduction.
///
/// @us-cc-001 @driving_port @real-io @anti-misread @c-6 @wd-cc-10 @presence-once @boundary
#[test]
fn the_countered_count_is_neutral_awareness_never_a_verdict_or_penalty() {
    // GIVEN one own claim (confidence 0.30) countered by TWO peers, her only countered
    // claim — the count is "(1 countered)" (presence-once).
    let env = TestEnv::initialized();
    let _held = seed_landing_store_one_own_claim_countered_twice(&env);

    // WHEN Maria opens GET /.
    let viewer = ViewerServer::start(&env);
    let page = viewer.get(LANDING_PATH);

    // THEN the landing shows the neutral presence count "(1 countered)" (never
    // "(2 countered)"), AND the copy carries NO verdict/penalty/"disputed by N" language.
    assert_eq!(page.status, 200, "the landing page must render");
    assert_landing_countered_count(&page.body, 1);
    assert!(
        !page.body_contains("(2 countered)"),
        "the count is a neutral PRESENCE count, never a \"by N\" total — must not show \
         \"(2 countered)\" for a claim countered by 2 peers (C-6 / WD-CC-10); body was:\n{}",
        page.body
    );
    assert_countered_copy_is_neutral(&page.body);
}
