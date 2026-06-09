//! Slice-17 acceptance — the `openlore ui` LANDING DASHBOARD: `GET /` as a
//! read-only navigation hub + at-a-glance LOCAL store summary (US-LD-000/001;
//! ADR-054).
//!
//! `GET /` is the only viewer handler that takes no store; today it renders an
//! `<h1>`, the `READ_ONLY_NOTICE`, and a single hardcoded `/claims` link ("queries
//! nothing"). slice-17 threads the read-only store the viewer ALREADY holds into
//! `landing_page`, resolves THREE LOCAL aggregate counts — own claims via
//! `StoreReadPort::count_claims`, peer claims via `count_peer_claims`, active peers
//! via the NEW count-only `count_active_peer_subscriptions` (`SELECT COUNT(*) FROM
//! peer_subscriptions WHERE removed_at IS NULL`, ADR-054 D3) — each `Result<usize,
//! StoreReadError> → Option<usize>` via `.ok()` in the EFFECT shell, builds a flat
//! Option-shaped `LandingSummary { own_claims, peer_claims, active_peers }`, and
//! passes it to the extended PURE `render_landing(&summary) -> String`. The pure
//! render keeps the `<h1>` + `READ_ONLY_NOTICE`, renders the three counts (`Some(n)`
//! → the number; `None` → `MISSING_COUNT_MARKER` "—", DISTINCT from a real 0), and a
//! nav hub of plain `<a href>` links to all 8 shipped top-level surfaces (`/claims`,
//! `/peer-claims`, `/project`, `/philosophy`, `/score`, `/search`, `/scrape` [the new
//! `SCRAPE_URL` const], `/peers`) via URL consts. Full-page-only — `GET /` does NOT
//! fork by `Shape` (ADR-054 D5; parity by construction). Read-only / no-key, LOCAL /
//! offline, missing≠zero, no-N+1 (3 fixed reads per render).
//!
//! Driving discipline (Mandate 1): every scenario enters through the REAL `openlore
//! ui` subprocess (`ViewerServer::start`) + in-test HTTP GET against `/` — NO scenario
//! calls `viewer-domain::render_landing` / the count reads directly (those are
//! unit/property-level, exercised in DELIVER). The local DuckDB store is REAL, seeded
//! through the PRODUCTION write paths (own claims via `claim add`; peer claims via
//! `peer add` + `peer pull`; active subscriptions via `peer add`), so the rows the
//! three reads return are produced by production code, not hand-inserted (Pillar 3 /
//! BR-VIEW-4). NO external/network boundary exists — `/` is LOCAL + OFFLINE
//! (offline-STRONGER than `/search`/`/scrape`; the three reads are LOCAL `COUNT(*)`
//! aggregates with no outbound edge). Every assertion is on the rendered HTML the
//! operator's browser shows (Mandate 8 universe = port-exposed rendered surface).
//!
//! Layer placement (nw-tdd-methodology Layered Test Discipline matrix + Mandate 9/11):
//! every scenario is a layer-3/layer-5 subprocess + real-I/O test — EXAMPLE-only. The
//! sad paths (honest empty store, failed peer-claims-count read) are enumerated
//! explicitly, never PBT-generated at this layer (the generative exploration of the
//! pure `render_landing` over the 2³ Option combinations is a layer-1/2 DELIVER
//! concern). Tier B (state-machine PBT) is NOT warranted: `GET /` is a single-shot
//! orientation render with no chained ≥3-scenario journey and no domain-rich input
//! space (three counts + 8 fixed links) — Tier A example coverage is exact (Mandate 10
//! skip criteria).
//!
//! Build-before-run note (carry into the DELIVER roadmap, mirrors slice-06..16): `cargo
//! test` does NOT rebuild a spawned binary automatically — the roadmap/run MUST `cargo
//! build` the `openlore` bin (the viewer) before running these ATs so
//! `ViewerServer::start` spawns the CURRENT viewer, not a stale one. `/` needs NO
//! second binary — the three reads are LOCAL DuckDB reads.
//!
//! Mandate 7 RED scaffolds: the ATs spawn the bin + HTTP, so they COMPILE now with the
//! new `seed_landing_store_summary` / `seed_empty_store_for_landing` /
//! `start_viewer_with_failing_peer_claims_count` seeds + `assert_landing_*` asserts
//! (which compile — they drive the EXISTING `claim add`/`peer add`/`peer pull` verbs +
//! scan strings). Each scenario body runs to a `GET /` HTTP assertion that FAILS
//! because the production `/` route is STORELESS (`render_landing()` takes no summary,
//! renders only the `<h1>` + `READ_ONLY_NOTICE` + a single `/claims` link) and
//! `SCRAPE_URL` / `count_active_peer_subscriptions` / `LandingSummary` /
//! `MISSING_COUNT_MARKER` do NOT exist yet — so the three counts + the 8-surface hub
//! are ABSENT from the rendered body → classifies RED (MISSING_FUNCTIONALITY), NOT
//! BROKEN. The ATs drive `GET /` via subprocess HTTP (never the Rust `render_landing`
//! signature), so the production signature change (adding the `&LandingSummary` param)
//! is DELIVER's job and does not affect AT compilation. They stay RED until DELIVER's
//! per-scenario RED→GREEN→COMMIT cycles (ADR-025).
//!
//! Covers (acceptance-criteria.md, 8 themes for US-LD-001 + US-LD-000 read wiring):
//! - LD-WS (Theme 1, US-LD-001): WALKING SKELETON — GET / over a seeded store (12 own,
//!   7 peer, 2 active) returns a 200 full page showing the 3 counts AND nav links to
//!   all 8 surfaces (+ keeps h1 + READ_ONLY_NOTICE).
//! - Theme 1 Ex 2: a fresh empty store shows honest zeros (0/0/0) + the full hub.
//! - Theme 2: discoverability — the hub links ALL 8 surfaces via their hrefs; NO
//!   deep/parameterized route is a top-level link; each link is a plain <a href>.
//! - Theme 3: read-only / no-write — GET / renders no form/button/mutating control.
//! - Theme 4: missing≠zero — a FAILED peer-claims-count read renders "—" (the other
//!   two counts still render), page 200; DISTINCT from a fresh store's real 0.
//! - Theme 5: LOCAL/offline — GET / renders fully network-down; references only the
//!   vendored /static/htmx.min.js (no CDN); no-N+1 (3 fixed reads invariant to size).
//! - Theme 7: counts are aggregates, never merges — the peer-claims count is a single
//!   number, NO per-author content/score/merged "consensus" on the front door.
//! - Theme 8 (US-LD-000): a soft-removed peer is not counted in the active-peer count.
//
// SCAFFOLD: true

mod support;

#[allow(unused_imports)]
use support::*;

// =============================================================================
// LD-WS — WALKING SKELETON (US-LD-001 Theme 1; the riskiest-assumption thread):
// GET / over a seeded store surfaces the 3 LOCAL counts AND the full 8-surface
// nav hub on a 200 full page, preserving the read-only notice. This is the
// thinnest complete thread the slice can demo: viewer → 3 LOCAL aggregate reads →
// LandingSummary → pure render → full HTML page (summary + hub).
// =============================================================================

/// LD-WS / WALKING SKELETON (US-LD-001 Theme 1; AC "The front door shows the LOCAL
/// store summary"): from the LOCAL store seeded with 12 own claims, 7 peer claims, and
/// 2 active peer subscriptions (Rachel + Tobias), `GET /` returns a 200 full page that
/// shows the three counts ("12 own claims, 7 peer claims, 2 active peers") AND a
/// navigation hub linking all 8 shipped surfaces — while keeping the `<h1>` and the
/// `READ_ONLY_NOTICE`. This is the thinnest demo-able thread: the front door orients
/// the operator (what's in my store + where can I go) with zero SQL, read-only,
/// offline.
///
/// Given Maria's store has 12 own claims, 7 peer claims, and 2 active peer
///   subscriptions (did:plc:rachel-test, did:plc:tobias-test);
/// When she opens GET / in the openlore ui viewer;
/// Then she sees a store summary showing 12 own claims, 7 peer claims, and 2 active
///   peers, the full navigation hub to all 8 surfaces, and the read-only notice.
///
/// @us-ld-001 @walking_skeleton @driving_port @driving_adapter @real-io @kpi-view-1
/// @happy
#[test]
fn the_front_door_shows_the_local_store_summary_and_the_full_navigation_hub() {
    // GIVEN a REAL local store seeded (production `claim add` + `peer add` + `peer
    // pull` paths, Pillar 3) with 12 own claims, 7 peer claims, and 2 active peers.
    let env = TestEnv::initialized();
    let _held = seed_landing_store_summary(&env);

    // WHEN Maria opens GET / in the viewer (full page — `/` is full-page-only, no
    // HX-Request, ADR-054 D5).
    let viewer = ViewerServer::start(&env);
    let page = viewer.get(LANDING_PATH);

    // THEN the front door renders a 200 full page showing the three counts attributed
    // to their surfaces, the full 8-surface nav hub, and the read-only notice.
    assert_eq!(
        page.status, 200,
        "GET / must render the landing dashboard (200); got {}",
        page.status
    );
    assert!(
        page.is_full_page(),
        "GET / is full-page-only (ADR-054 D5) — the response must be a complete \
         document (chrome + summary + hub); body was:\n{}",
        page.body
    );
    // The three LOCAL counts (Theme 1 / Theme 3 — the genuine seeded aggregates).
    assert_landing_shows_count(&page.body, "own claims", LANDING_OWN_CLAIMS);
    assert_landing_shows_count(&page.body, "peer claims", LANDING_PEER_CLAIMS);
    assert_landing_shows_count(&page.body, "active peers", LANDING_ACTIVE_PEERS);
    // The full navigation hub to all 8 shipped surfaces (Theme 2 discoverability).
    assert_landing_links_all_surfaces(&page.body);
    // The read-only notice is preserved (the slice-06 front-door promise, NFR-VIEW-1).
    assert!(
        page.body_contains(READ_ONLY_NOTICE_TEXT),
        "the front door must keep the read-only notice telling Maria nothing here can \
         change her store (NFR-VIEW-1); body was:\n{}",
        page.body
    );
}

// =============================================================================
// Theme 1 Ex 2 — a fresh empty store shows honest zeros (0/0/0), DISTINCT from
// the missing-number state, plus the full hub so a new operator can navigate.
// =============================================================================

/// LD-ZEROS (US-LD-001 Theme 1 Ex 2; AC "A fresh empty store shows honest zero
/// counts"): a fresh operator with 0 own claims, 0 peer claims, and 0 active
/// subscriptions opens GET / and sees the summary showing 0 own claims, 0 peer claims,
/// and 0 active peers — each a SUCCESSFUL read of zero (an honest empty store), NOT a
/// missing-number state — plus the full nav hub (so a new user can navigate to /scrape
/// or /search to start). The `0 ≠ missing` distinction made visible on the success
/// side.
///
/// Given Maria has a fresh store with 0 own claims, 0 peer claims, and 0 active
///   subscriptions;
/// When she opens GET /;
/// Then the summary shows 0 own claims, 0 peer claims, and 0 active peers (each a
///   successful read of zero, not a missing-number state) + the full hub.
///
/// @us-ld-001 @driving_port @real-io @empty-state @edge
#[test]
fn a_fresh_empty_store_shows_honest_zero_counts_and_the_full_hub() {
    // GIVEN a fresh, initialized store with ZERO claims, peer claims, and
    // subscriptions (the three reads return Ok(0) → Some(0)).
    let env = TestEnv::initialized();
    seed_empty_store_for_landing(&env);

    // WHEN Maria opens GET /.
    let viewer = ViewerServer::start(&env);
    let page = viewer.get(LANDING_PATH);

    // THEN the summary shows honest zeros (Some(0) → "0"), DISTINCT from the
    // missing-number marker, AND the full hub renders so the new operator can navigate.
    assert_eq!(page.status, 200, "an empty store still renders a 200 landing page");
    assert_landing_shows_count(&page.body, "own claims", 0);
    assert_landing_shows_count(&page.body, "peer claims", 0);
    assert_landing_shows_count(&page.body, "active peers", 0);
    // The zeros must NOT be the missing-number marker — an empty store is a SUCCESSFUL
    // read of zero, not a failed read (the `0 ≠ missing` distinction, WD-LD-8). Scan
    // each COUNT position (`"— <label>"`), NOT the bare marker: the page chrome title
    // ("OpenLore — Viewer") legitimately carries the em-dash, so a bare-marker scan
    // would collide with the title rather than the count surface.
    for label in ["own claims", "peer claims", "active peers"] {
        let missing_count = format!("{LANDING_MISSING_COUNT_MARKER} {label}");
        assert!(
            !page.body_contains(&missing_count),
            "a fresh empty store renders honest zeros (Some(0)), NOT the missing-number \
             count {missing_count:?} (which would mean a FAILED read); body was:\n{}",
            page.body
        );
    }
    // The full hub is present so a brand-new operator can reach /scrape or /search.
    assert_landing_links_all_surfaces(&page.body);
}

// =============================================================================
// Theme 2 — discoverability: the hub links ALL 8 surfaces via their hrefs; NO
// deep/parameterized route is a top-level link; each link is a plain <a href>.
// =============================================================================

/// LD-DISCOVER (US-LD-001 Theme 2 / C-3; AC "The front door links to every shipped
/// surface"): the navigation hub links all 8 shipped top-level surfaces — My Claims
/// (/claims), Peer Claims (/peer-claims), Project Survey (/project), Philosophy Survey
/// (/philosophy), Contributor Score (/score), Network Search (/search), Live Scrape
/// (/scrape), Peer Subscriptions (/peers) — each as a plain `<a href>` (no-JS
/// navigable), and NO deep/parameterized route (/claims/{cid}, /score?contributor,
/// /project?subject) is a top-level link. Closes the discoverability gap (today only
/// /claims is reachable from /).
///
/// Given Maria opens GET /;
/// When she looks at the navigation hub;
/// Then she sees a plain-link to all 8 top-level surfaces and NO deep/parameterized
///   route as a top-level link.
///
/// @us-ld-001 @driving_port @real-io @discoverability @c-3 @happy
#[test]
fn the_front_door_links_every_shipped_surface_and_no_deep_route() {
    // GIVEN a seeded store (so the page renders fully, with the hub over real content).
    let env = TestEnv::initialized();
    let _held = seed_landing_store_summary(&env);

    // WHEN Maria opens GET / and looks at the nav hub.
    let viewer = ViewerServer::start(&env);
    let page = viewer.get(LANDING_PATH);

    // THEN all 8 top-level surfaces are linked via plain <a href> (no-JS navigable),
    // and NO deep/parameterized route is a top-level hub link.
    assert_eq!(page.status, 200, "the landing page must render");
    assert_landing_links_all_surfaces(&page.body);
    assert_landing_no_deep_route_toplevel(&page.body);
}

/// LD-URLCONST (US-LD-001 Theme 2; AC "Each surface link uses the route's
/// single-source-of-truth URL constant"): each hub link's href equals the route's URL
/// constant value from viewer-domain (MY_CLAIMS_URL, PEER_CLAIMS_URL, PROJECT_URL,
/// PHILOSOPHY_URL, SCORE_URL, SEARCH_URL, PEERS_URL, and the NEW SCRAPE_URL = "/scrape")
/// — no link is a hardcoded path literal that could drift from its route. The minted
/// SCRAPE_URL closes the one missing const (R-LD-4 / WD-LD-7). Observed behaviorally:
/// the href values match the canonical route paths exactly (the URL-const drift guard
/// is also a DELIVER unit-level concern; here we pin the rendered hrefs).
///
/// Given the viewer renders the navigation hub on GET /;
/// Then each link's href equals its route's canonical path (the URL const value),
///   including the newly-minted /scrape.
///
/// @us-ld-001 @driving_port @real-io @discoverability @scrape-url @happy
#[test]
fn each_surface_link_uses_the_routes_url_constant_including_scrape() {
    // GIVEN a seeded store + the viewer rendering the nav hub on GET /.
    let env = TestEnv::initialized();
    let _held = seed_landing_store_summary(&env);
    let viewer = ViewerServer::start(&env);
    let page = viewer.get(LANDING_PATH);
    assert_eq!(page.status, 200, "the landing page must render the hub");

    // THEN every hub link's href equals its route's canonical path value — the
    // load-bearing single-source-of-truth target (each pair in
    // LANDING_TOP_LEVEL_SURFACES is the URL-const value, NOT a drifting literal).
    // `assert_landing_links_all_surfaces` scans for `href="<route>"` over all 8.
    assert_landing_links_all_surfaces(&page.body);
    // The NEWLY-MINTED /scrape const must be among them (the one const that did not
    // exist before slice-17, ADR-054 D4) — making the hub 8 consistent consts.
    assert!(
        page.body_contains("href=\"/scrape\""),
        "the hub must link Live Scrape via the newly-minted SCRAPE_URL (\"/scrape\", \
         ADR-054 D4) so every link is a URL const, no drift (R-LD-4); body was:\n{}",
        page.body
    );
}

// =============================================================================
// Theme 3 — read-only / no-write (CARDINAL, C-1): the front door exposes no
// write/compose/sign/subscribe/follow control; every affordance is a plain link.
// =============================================================================

/// LD-READONLY (US-LD-001 Theme 3 / C-1 CARDINAL; AC "The front door exposes no write,
/// compose, sign, subscribe, or follow control"): when Maria inspects the rendered
/// page, it contains no form, no button, and no control to compose, sign, subscribe, or
/// follow — every navigation affordance is a plain link, not a mutating control. (The
/// no-key guarantee is structural — proven by the slice-06 `web_process_holds_no_
/// signing_key` gold + xtask check-arch; here the operator-facing surface carries no
/// mutating control.)
///
/// Given Maria opens GET /;
/// When she inspects the rendered page;
/// Then it contains no form, no button, and no compose/sign/subscribe/follow control —
///   every navigation affordance is a plain link.
///
/// @us-ld-001 @driving_port @real-io @read-only @c-1 @cardinal @happy
#[test]
#[ignore = "enabled in roadmap step 01-04 (LD-READONLY) — progressive implementation"]
fn the_front_door_exposes_no_write_compose_sign_subscribe_or_follow_control() {
    // GIVEN a seeded store (so the no-control scan is over REAL rendered content, not
    // an error/empty page).
    let env = TestEnv::initialized();
    let _held = seed_landing_store_summary(&env);

    // WHEN Maria opens GET / and inspects the page.
    let viewer = ViewerServer::start(&env);
    let page = viewer.get(LANDING_PATH);

    // THEN the page renders successfully AND carries no write/compose/sign/subscribe/
    // follow control — every affordance is a plain <a href> link (C-1 CARDINAL).
    assert_eq!(
        page.status, 200,
        "the landing page must render (200) so the no-control scan is over REAL content"
    );
    assert_landing_read_only_no_control(&page.body);
}

// =============================================================================
// Theme 4 — graceful degrade if a count read fails (CARDINAL, C-2 / WD-LD-2 /
// WD-LD-8): a FAILED count read renders "—" for that count (others still render),
// the page is 200, DISTINCT from a fresh store's real 0.
// =============================================================================

/// LD-DEGRADE (US-LD-000/001 Theme 4 / C-2 CARDINAL; AC "A failed count read degrades
/// to a missing-number state without a 5xx"): Maria's peer-claims-count read fails
/// transiently while the own-claims and active-peer reads succeed. When she opens
/// GET /, the navigation hub renders in full, the own-claims count shows 12 and the
/// active-peer count shows 2, and the peer-claims number renders as the missing-number
/// state "—" (NOT a fabricated 0), and the page is a normal 200 — never a 5xx, never a
/// raw stack trace. The per-count independent degrade (`.ok()` → `None` →
/// `MISSING_COUNT_MARKER`, ADR-054 D2).
///
/// SEEDING-SEAM NOTE (documented DISTILL choice, slice-16 SF-8 precedent): the viewer
/// holds one long-lived DuckDB connection taken at startup, so there is no
/// readily-available mid-request per-count read-failure seam in the slice-06/15
/// harness. This scenario drives the TEST-ONLY effect-shell fault seam
/// (`start_viewer_with_failing_peer_claims_count` → the
/// `OPENLORE_VIEWER_FAIL_PEER_CLAIMS_COUNT` env var, materialized by DELIVER exactly as
/// slice-16 materialized `OPENLORE_VIEWER_FAIL_ACTIVE_SET_READ`). The structural
/// `Some(0)`-vs-`None` distinction's success side is fully exercised by the
/// honest-zeros scenario above; this scenario pins the FAILURE side. Until DELIVER
/// materializes the seam the scenario fails at `start_inner`'s `todo!()` body → RED
/// MISSING_FUNCTIONALITY, never BROKEN.
///
/// Given Maria's peer-claims count read fails transiently while the own-claims and
///   active-peer reads succeed (12 own, 2 active);
/// When she opens GET /;
/// Then the hub renders in full, own-claims shows 12, active-peers shows 2, the
///   peer-claims number renders as "—" (not a fabricated 0), and the page is 200.
///
/// @us-ld-000 @us-ld-001 @driving_port @real-io @infrastructure-failure @missing-not-zero
/// @c-2 @cardinal @error
#[test]
#[ignore = "enabled in roadmap step 02-01 (LD-DEGRADE) — needs the peer-claims-count fault seam"]
fn a_failed_peer_claims_read_degrades_to_a_missing_number_state_without_a_5xx() {
    // GIVEN a store seeded with 12 own + 2 active peers, BUT the peer-claims count read
    // forced to FAIL mid-request (the own-claims + active-peer reads still succeed).
    let env = TestEnv::initialized();
    let _held = seed_landing_store_summary(&env);

    // WHEN Maria opens GET / against the viewer whose peer-claims count read fails.
    let viewer = start_viewer_with_failing_peer_claims_count(&env);
    let page = viewer.get(LANDING_PATH);

    // THEN the page is a NORMAL 200 (never a 5xx), the hub renders in full, the
    // own-claims + active-peer counts STILL show their numbers, and the peer-claims
    // number renders as the missing-number marker "—" (NOT a fabricated 0).
    assert_eq!(
        page.status, 200,
        "a failed count read must degrade to a 200 page, NEVER a 5xx (C-2 CARDINAL / \
         NFR-VIEW-6); got {}",
        page.status
    );
    // The hub still renders in full (the failure is per-count, not page-wide).
    assert_landing_links_all_surfaces(&page.body);
    // The two SUCCESSFUL counts still render their numbers.
    assert_landing_shows_count(&page.body, "own claims", LANDING_OWN_CLAIMS);
    assert_landing_shows_count(&page.body, "active peers", LANDING_ACTIVE_PEERS);
    // The FAILED peer-claims count renders the missing-number marker "—" (not 0).
    assert_landing_count_missing(&page.body, "peer claims");
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

// =============================================================================
// Theme 5 — LOCAL / offline: the front door renders fully with the network down,
// references only the vendored local htmx asset (no CDN).
// =============================================================================

/// LD-OFFLINE (US-LD-000/001 Theme 5 / C-2; AC "The front door renders fully with the
/// network down"): Maria's store has claims and peers and the network is unavailable.
/// When she opens GET /, the store summary and the full navigation hub render, no
/// outbound network request is made by the route (the three reads are LOCAL `COUNT(*)`
/// aggregates — no PDS fetch, no DID re-resolution, no peer pull, no network search),
/// and the page references only the vendored local /static/htmx.min.js (no CDN). The
/// front door is offline-STRONGER than /search/scrape — it has NO outbound edge at all.
///
/// Given Maria's store has claims and peers and the network is unavailable;
/// When she opens GET /;
/// Then the store summary and the full navigation hub render, and the page references
///   only the vendored local htmx asset (no CDN).
///
/// @us-ld-001 @driving_port @real-io @offline @no-cdn @c-2 @happy
#[test]
#[ignore = "enabled in roadmap step 02-02 (LD-OFFLINE) — progressive implementation"]
fn the_front_door_renders_fully_with_the_network_down() {
    // GIVEN a seeded store. The viewer is started with NO network reachability wired
    // (no GitHub seam, no indexer seam) — the front door is LOCAL by construction, so
    // an absent network is exactly the operator's offline machine.
    let env = TestEnv::initialized();
    let _held = seed_landing_store_summary(&env);

    // WHEN Maria opens GET / (offline — `ViewerServer::start` wires no outbound seam).
    let viewer = ViewerServer::start(&env);
    let page = viewer.get(LANDING_PATH);

    // THEN the summary + the full hub render fully offline (the three LOCAL reads have
    // no outbound edge to take down), and the page references only the vendored local
    // htmx asset — never a CDN.
    assert_eq!(page.status, 200, "GET / must render fully offline (200)");
    assert_landing_shows_count(&page.body, "own claims", LANDING_OWN_CLAIMS);
    assert_landing_shows_count(&page.body, "peer claims", LANDING_PEER_CLAIMS);
    assert_landing_shows_count(&page.body, "active peers", LANDING_ACTIVE_PEERS);
    assert_landing_links_all_surfaces(&page.body);
    assert!(
        !page.references_external_cdn(),
        "the front door must reference only the vendored local /static/htmx.min.js — \
         no off-host CDN (I-VIEW-6 / KPI-HX-G2); body was:\n{}",
        page.body
    );
}

// =============================================================================
// Theme 7 — counts are aggregates, never merges (C-7 / BR-LD-1): the summary
// shows a single peer-claims count (an aggregate), NEVER per-author content,
// scores, or a merged "consensus" record on the front door.
// =============================================================================

/// LD-AGGREGATE (US-LD-001 Theme 7 / C-7 / BR-LD-1; AC "The store summary shows
/// aggregate counts, never a merged consensus record"): Maria's store has peer claims
/// from did:plc:rachel-test and did:plc:tobias-test. When she opens GET /, the summary
/// shows a SINGLE peer-claims count (how many, an aggregate) and does NOT render any
/// per-author content, score, or merged "consensus" claim on the front door — reading
/// who-said-what is reached by navigating to the attributed surfaces (/peer-claims,
/// /score). The anti-merging invariant protects per-author CONTENT rendering; a
/// store-wide count is a legitimate aggregate.
///
/// Given Maria's store has peer claims from two distinct authors;
/// When she opens GET /;
/// Then the summary shows a single aggregate peer-claims count and renders NO
///   per-author content/score/merged-consensus row on the front door.
///
/// @us-ld-001 @driving_port @real-io @anti-merging @c-7 @br-ld-1 @happy
#[test]
#[ignore = "enabled in roadmap step 02-02 (LD-AGGREGATE) — progressive implementation"]
fn the_store_summary_shows_an_aggregate_count_never_a_merged_consensus_record() {
    // GIVEN a store seeded with peer claims from Rachel + an active Tobias subscription
    // (two distinct authors in the active set).
    let env = TestEnv::initialized();
    let _held = seed_landing_store_summary(&env);

    // WHEN Maria opens GET /.
    let viewer = ViewerServer::start(&env);
    let page = viewer.get(LANDING_PATH);

    // THEN the summary shows the SINGLE aggregate peer-claims count (7) — an aggregate,
    // not a per-author breakdown.
    assert_eq!(page.status, 200, "the landing page must render");
    assert_landing_shows_count(&page.body, "peer claims", LANDING_PEER_CLAIMS);

    // AND the front door renders NO per-author content / score / merged "consensus"
    // row — the three numbers are aggregates; content stays on the attributed surfaces
    // (WD-LD-6 / BR-LD-1). The per-author DIDs must NOT appear on the front door (they
    // are reached THROUGH /peer-claims, /peers, /score), and no merged-consensus
    // phrasing appears.
    for per_author in [LANDING_PEER_RACHEL_DID, LANDING_PEER_TOBIAS_DID] {
        assert!(
            !page.body_contains(per_author),
            "the front door must render NO per-author content — the peer DID {per_author:?} \
             must NOT appear on / (it is reached THROUGH /peer-claims, /peers, /score; \
             WD-LD-6 / BR-LD-1); body was:\n{}",
            page.body
        );
    }
    let lowered = page.body.to_ascii_lowercase();
    for merged in ["consensus", "authors agree", "the network says"] {
        assert!(
            !lowered.contains(merged),
            "the front door must render NO merged \"consensus\" record — the counts are \
             aggregates, never a merge (BR-LD-1); found {merged:?} in body:\n{}",
            page.body
        );
    }
}

// =============================================================================
// Theme 8 — US-LD-000 read wiring: a soft-removed peer is not counted in the
// active-peer summary (the active-only `removed_at IS NULL` definition, BR-LD-2).
// =============================================================================

/// LD-SOFTREMOVED (US-LD-000 Theme 8; AC "A soft-removed peer is not counted in the
/// active-peer summary"): Maria subscribed to did:plc:rachel-test then ran `openlore
/// peer remove did:plc:rachel-test` (no --purge), and she still actively follows
/// did:plc:tobias-test. When she opens GET /, the active-peer count is 1 (only the
/// active subscription) and the soft-removed did:plc:rachel-test is not counted. This
/// pins the active-only definition (`count_active_peer_subscriptions` =
/// `COUNT(*) WHERE removed_at IS NULL`, ADR-054 D3 / BR-LD-2) — the residue of a
/// soft-removed peer is excluded from the front-door count, even though its cached
/// claims remain on disk.
///
/// Given Maria subscribed to Rachel then soft-removed her (no --purge), and still
///   actively follows Tobias;
/// When she opens GET /;
/// Then the active-peer count is 1 (only the active subscription) — the soft-removed
///   Rachel is not counted.
///
/// @us-ld-000 @driving_port @real-io @active-only @br-ld-2 @boundary
#[test]
#[ignore = "enabled in roadmap step 02-03 (LD-SOFTREMOVED) — progressive implementation"]
fn a_soft_removed_peer_is_not_counted_in_the_active_peer_summary() {
    // GIVEN Maria subscribed + pulled Rachel, THEN soft-removed her (no --purge — the
    // subscription's `removed_at` is set, the cached claims survive). She still
    // actively follows Tobias (a fresh active subscription).
    let env = TestEnv::initialized();
    seed_peer_subscribed_then_removed(&env); // Rachel: subscribed+pulled then soft-removed
    let _tobias = seed_active_subscription_for(&env, LANDING_PEER_TOBIAS_DID, [9u8; 32]);

    // WHEN Maria opens GET /.
    let viewer = ViewerServer::start(&env);
    let page = viewer.get(LANDING_PATH);

    // THEN the active-peer count is 1 (only Tobias — the active-only `removed_at IS
    // NULL` set, BR-LD-2). The soft-removed Rachel is NOT counted, and her DID does not
    // appear on the front door (aggregate-only, Theme 7).
    assert_eq!(page.status, 200, "the landing page must render");
    assert_landing_shows_count(&page.body, "active peers", 1);
    assert!(
        !page.body_contains(LANDING_PEER_RACHEL_DID),
        "the soft-removed peer {LANDING_PEER_RACHEL_DID:?} must NOT appear on the front \
         door (excluded from the active-peer count by the `removed_at IS NULL` filter, \
         BR-LD-2); body was:\n{}",
        page.body
    );
}
