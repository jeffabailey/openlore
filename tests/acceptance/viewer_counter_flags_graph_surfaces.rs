//! Slice-13 acceptance — the `openlore ui` at-a-glance "Countered" PRESENCE FLAG
//! extended to the OTHER two LOCAL surfaces the operator scans: the FEDERATED
//! `GET /peer-claims` LIST rows (US-CF-002) and the GRAPH-TRAVERSAL `GET /project` +
//! `GET /philosophy` EDGE rows (US-CF-003; ADR-049/050).
//!
//! slice-11 made disagreement legible once a claim is OPENED (the counter thread on
//! `/claims/{cid}`); slice-12 made it discoverable while scanning the operator's OWN
//! `/claims` list. slice-13 closes the gap on the two remaining SHARED-SHAPE local
//! surfaces: each `/peer-claims` row whose claim has >= 1 counter, and each
//! `/project`/`/philosophy` EDGE whose claim has >= 1 counter, now carries the SAME
//! neutral "Countered" marker — a render-only `<a href="/claims/{cid}">Countered</a>`
//! one-hop link to that claim's slice-11 thread. `/score` is OUT (deferred slice-14);
//! `/search` already has its own slice-08 annotation; `/claims` shipped slice-12.
//!
//! The flag REUSES the slice-12 `StoreReadPort::counter_presence_for(&[cid]) ->
//! HashSet<String>` batch read VERBATIM (ADR-048), wired into three more handlers
//! (ADR-049) — NO new read method, NO new SQL. For the edge surfaces the page CID set
//! is the UNION of every `EdgeRow.cid` across every `EdgeGroup`, flattened from the
//! FLAT survey rows BEFORE grouping and queried ONCE (ADR-050) — one aggregate query
//! per render, invariant to edge/group count (the N+1 guard, I-CF-8).
//!
//! The flag is PRESENCE-only (a row/edge countered by N distinct authors shows ONE
//! neutral marker, never "disputed by N", never a count, never a verdict) and ADDITIVE:
//! it NEVER re-orders the `/peer-claims` list/paging, and on the edge surfaces NEVER
//! changes the `group_by` grouping, group order, edge order, deduped contributor list,
//! or any cross-link (shown-never-applied, I-CF-2 / I-CF-9). An un-countered row/edge
//! renders byte-identically to slice-06 (`/peer-claims`) / slice-10 (traversal).
//!
//! Driving discipline (Mandate 1): every scenario enters through the REAL `openlore ui`
//! subprocess (`ViewerServer`) + in-test HTTP GET (with/without the `HX-Request`
//! header — the slice-07 `get`/`get_htmx` pair) and asserts on the returned HTML. NO
//! scenario calls the `viewer-domain` `render_*` fns or `counter_presence_for` directly
//! (those are unit/property-level, exercised in DELIVER). The local DuckDB store is
//! REAL, seeded through the PRODUCTION federation write paths (`peer add` + `peer pull`
//! for the surveyed peer claims; a DISTINCT peer's verifiable counter for the flag) —
//! Pillar 3 / BR-VIEW-4. The presence read is LOCAL (DB index only); NO network seam
//! exists on any of these three routes (offline by construction, I-CF-5).
//!
//! Layer placement (nw-tdd-methodology Layered Test Discipline + Mandate 9/11): every
//! scenario here is a layer-3/layer-5 subprocess + real-I/O test — EXAMPLE-only. The
//! sad/edge paths (none-countered, multi-counter, mixed survey) are enumerated
//! explicitly, never PBT-generated at this layer. The strict 1-query N+1 bound is a
//! DELIVER unit/property assertion in `adapter-duckdb` (the REUSED slice-12 read); at
//! this subprocess AT layer the N+1 guard is asserted via its behavioral proxy (a
//! survey of MANY edges across MANY groups all flag correctly in ONE request — CF-N1).
//!
//! Build-before-run note (carry into the DELIVER roadmap, mirrors slice-06/07/10/11/12):
//! `cargo test` does NOT rebuild a spawned binary automatically — the roadmap/run MUST
//! `cargo build` the `openlore` (viewer) bin before running these ATs so
//! `ViewerServer::start` spawns the CURRENT viewer, not a stale one. The flag needs NO
//! second binary — the presence read is a LOCAL read.
//!
//! Mandate 7 RED scaffolds (ADR-025): the ATs spawn the bin + HTTP, so they COMPILE now
//! with `todo!()` bodies + the new slice-13 seeds (`seed_peer_claims_one_countered` /
//! `seed_project_survey_one_edge_countered` / `seed_philosophy_survey_one_edge_countered`
//! / `seed_survey_none_countered` / `seed_*_two_counters_distinct_authors` /
//! `seed_project_survey_many_groups_known_countered_subset`) + the new assert helpers
//! (`assert_peer_claim_row_flagged_countered` / `assert_edge_flagged_countered` /
//! `assert_*_not_flagged` / `assert_*_flag_links_to_thread` /
//! `assert_*_flag_is_single_neutral_presence`), all `todo!()`-stubbed in support/mod.rs
//! (they compile, then panic). Each scenario body reaches a `todo!()` -> panics ->
//! classifies RED (MISSING_FUNCTIONALITY), NOT BROKEN. They stay RED until DELIVER's
//! per-scenario RED->GREEN->COMMIT cycles.
//!
//! Covers:
//! - US-CF-002 (the `/peer-claims` flag, CF-1..CF-5): CF-1 walking skeleton — GET
//!   /peer-claims WITH HX-Request over a store where one peer claim is countered (a
//!   peer countered it) and others are not -> 200, ONLY the list fragment, the
//!   countered peer-claim row carries the neutral "Countered" `<a href>` marker, the
//!   un-countered rows none; + full-page parity (CF-2) + presence-only single neutral
//!   flag for a two-author claim (CF-3) + one-hop link to the slice-11 thread (CF-4) +
//!   peer origin + confidence unchanged beside the flag.
//! - US-CF-003 (the `/project` + `/philosophy` edge flag, CF-5..CF-8): a countered
//!   `/project` edge flagged in its UNCHANGED group/position (CF-5); the SYMMETRIC
//!   `/philosophy` edge flagged (CF-6, SAME render arm); a survey with NO counters
//!   renders exactly as slice-10 (CF-7, no-noise); the edge flag's one-hop link +
//!   presence-only single marker for a twice-countered edge (CF-8); the N+1-flatten
//!   behavioral proxy — a survey of MANY edges across MANY groups flags the countered
//!   subset correctly in ONE request (CF-N1).
//
// SCAFFOLD: true

mod support;

#[allow(unused_imports)]
use support::*;

// =============================================================================
// US-CF-002 — the neutral "Countered" presence flag on the FEDERATED /peer-claims
// LIST rows (CF-1..CF-4). CF-1 is the thinnest end-to-end thread (walking skeleton).
// =============================================================================

/// CF-1 / WALKING SKELETON (US-CF-002; the riskiest-assumption thread): from the LOCAL
/// store, `GET /peer-claims` WITH the `HX-Request` header over a store where ONE pulled
/// peer claim is countered (a DISTINCT peer authored a counter targeting it) and the
/// others are NOT returns ONLY the peer-claims list fragment (no full-page chrome) in
/// which the countered peer row carries the NEUTRAL "Countered" marker — a render-only
/// `<a href="/claims/{cid}">` link to that claim's slice-11 thread — while the
/// un-countered rows carry NO marker. This is the thinnest complete slice the
/// `/peer-claims` feature can demo: viewer -> LOCAL peer-list read -> LOCAL batch
/// presence read (REUSED slice-12) -> pure projection -> HTML fragment, proving the
/// federated list can carry an at-a-glance disagreement flag while preserving the
/// read-only / presence-only / local-first / progressive-enhancement invariants.
///
/// Given Maria's viewer reads a LOCAL store holding several pulled peer claims, exactly
///   one of which is countered by a DISTINCT peer;
/// When she opens the Peer Claims list WITH the htmx header (`GET /peer-claims`,
///   HX-Request);
/// Then she receives ONLY the list fragment (no chrome) in which the countered peer row
///   carries the neutral "Countered" marker linking to its `/claims/{cid}` thread, and
///   the un-countered rows carry no marker.
///
/// @us-cf-002 @walking_skeleton @driving_port @driving_adapter @real-io @htmx-fragment
/// @i-cf-2 @i-cf-3 @i-cf-6 @kpi-fed-3 @happy
#[test]
fn open_the_peer_claims_list_with_htmx_flags_only_the_countered_row() {
    // GIVEN a REAL local store with several pulled peer claims, exactly ONE of which is
    // countered by a DISTINCT peer (seeded via the production peer add + peer pull
    // federation path; the counter lands in peer_claim_references).
    //
    // WHEN Maria submits `GET /peer-claims` WITH the HX-Request header (get_htmx).
    //
    // THEN the response is ONLY the peer-claims list fragment (`is_fragment()`, NOT a
    // full page): the countered peer row carries the neutral "Countered" marker linking
    // to its slice-11 thread; every un-countered peer row carries NO marker.
    let env = TestEnv::initialized();
    let seeded = seed_peer_claims_one_countered(&env);
    let server = ViewerServer::start(&env);

    let response = server.get_htmx("/peer-claims");

    assert_eq!(
        response.status, 200,
        "GET /peer-claims (HX-Request) must be 200; body was:\n{}",
        response.body
    );
    assert!(
        response.content_type.contains("text/html"),
        "GET /peer-claims must serve text/html; got {:?}",
        response.content_type
    );
    assert!(
        response.is_fragment(),
        "GET /peer-claims WITH HX-Request must return ONLY the list fragment (no chrome); \
         body was:\n{}",
        response.body
    );

    // The countered peer row carries the neutral "Countered" marker linking to its thread.
    for countered in &seeded.countered_cids {
        assert_peer_claim_row_flagged_countered(&response.body, countered);
    }
    // Every un-countered peer row carries NO marker (and no empty-state noise).
    for uncountered in &seeded.uncountered_cids {
        assert_peer_claim_row_not_flagged(&response.body, uncountered);
    }
}

/// CF-2 (US-CF-002 — no-JS full page + parity, I-CF-6): `GET /peer-claims` WITHOUT
/// `HX-Request` serves a COMPLETE full page (chrome + the SAME list region) whose list
/// region renders the SAME flags as the htmx fragment — parity by construction (the
/// page EMBEDS the same list-fragment fn; the flag is rendered INSIDE the peer-claim
/// row). The peer-origin column + verbatim confidence render UNCHANGED beside the flag.
///
/// Given Maria's store holds a countered peer claim among pulled peer claims;
/// When the list renders as a full page (no JS) and as an htmx fragment;
/// Then the countered row shows the SAME "Countered" marker in both shapes, the
///   un-countered rows carry no marker in either, and the peer origin + confidence
///   render unchanged.
///
/// @us-cf-002 @driving_port @real-io @no-js @full-page @parity @i-cf-4 @i-cf-6 @happy
#[test]
fn the_peer_claims_flag_renders_identically_under_htmx_and_no_js() {
    let env = TestEnv::initialized();
    let seeded = seed_peer_claims_one_countered(&env);
    let server = ViewerServer::start(&env);

    // WHEN the list renders as a full page (no JS) AND as an htmx fragment over the SAME
    // store.
    let full = server.get("/peer-claims");
    let fragment = server.get_htmx("/peer-claims");

    for (label, response) in [("full page", &full), ("fragment", &fragment)] {
        assert_eq!(
            response.status, 200,
            "GET /peer-claims ({label}) must be 200; body was:\n{}",
            response.body
        );
        assert!(
            response.content_type.contains("text/html"),
            "GET /peer-claims ({label}) must serve text/html; got {:?}",
            response.content_type
        );
    }

    // The no-JS request is a COMPLETE full page (chrome present); the htmx request is
    // ONLY the swap-target fragment (no chrome).
    assert!(
        full.is_full_page(),
        "GET /peer-claims WITHOUT HX-Request must return a COMPLETE full page (chrome \
         present); body was:\n{}",
        full.body
    );
    assert!(
        fragment.is_fragment(),
        "GET /peer-claims WITH HX-Request must return ONLY the list fragment (no chrome); \
         body was:\n{}",
        fragment.body
    );

    // THEN: BOTH shapes carry the SAME "Countered" marker on the countered row — parity
    // by construction (the full page EMBEDS the same fragment fn; I-CF-6). NEITHER shape
    // flags an un-countered row, and the peer origin renders unchanged beside the flag.
    for countered in &seeded.countered_cids {
        assert_peer_claim_row_flagged_countered(&full.body, countered);
        assert_peer_claim_row_flagged_countered(&fragment.body, countered);
    }
    for uncountered in &seeded.uncountered_cids {
        assert_peer_claim_row_not_flagged(&full.body, uncountered);
        assert_peer_claim_row_not_flagged(&fragment.body, uncountered);
    }
    // The peer-origin column renders verbatim BESIDE the flag (the flag is additive;
    // US-CF-002 origin-unchanged / I-CF-4).
    assert_peer_claim_row_origin_unchanged(&full.body, &seeded.peer_did);
    assert_peer_claim_row_origin_unchanged(&fragment.body, &seeded.peer_did);
}

/// CF-3 / GOLD presence-only (US-CF-002; I-CF-3 / KPI-AV-2): a PEER claim countered by
/// TWO DISTINCT authors shows EXACTLY ONE neutral "Countered" marker on its
/// `/peer-claims` row — a PRESENCE marker, NEVER "disputed by 2", never a count, never a
/// merged verdict. The per-counter attribution lives in the slice-11 thread the marker
/// LINKS to, not on the list. Adapts the slice-12 anti-merging fixture to the FEDERATED
/// surface.
///
/// Given Maria's pulled peer claim is countered by two distinct authors;
/// When Maria opens the Peer Claims list;
/// Then the row shows EXACTLY ONE neutral "Countered" marker, and the list shows no
///   count, no "disputed by N", and no aggregate verdict.
///
/// @us-cf-002 @driving_port @real-io @presence-only @anti-merging @i-cf-3 @kpi-av-2 @gold
#[test]
fn a_peer_claim_with_two_counters_shows_one_neutral_presence_marker_on_the_list() {
    let env = TestEnv::initialized();
    let seeded = seed_peer_claims_target_two_counters_distinct_authors(&env);
    let target_cid = seeded
        .countered_cids
        .first()
        .expect("CF-3: the seed yields exactly one twice-countered peer target");
    let server = ViewerServer::start(&env);

    let response = server.get("/peer-claims");

    assert_eq!(
        response.status, 200,
        "GET /peer-claims must be 200; body was:\n{}",
        response.body
    );

    // The twice-countered row shows EXACTLY ONE neutral "Countered" marker (presence-only
    // — DISTINCT referenced_cid collapses the two distinct-author counters to one
    // membership), and the body carries NO count / "disputed by N" / verdict phrasing
    // (I-CF-3 / KPI-AV-2).
    assert_peer_claim_flag_is_single_neutral_presence(&response.body, target_cid);
    // The un-countered rows carry NO marker (the flag is presence-only + additive).
    for uncountered in &seeded.uncountered_cids {
        assert_peer_claim_row_not_flagged(&response.body, uncountered);
    }
}

/// CF-4 (US-CF-002 — one-hop link to the slice-11 thread, I-CF-6): the "Countered"
/// marker on a countered `/peer-claims` row is a render-only `<a href="/claims/{cid}">`
/// ONE-HOP link to that claim's slice-11 counter thread — navigable WITHOUT JS (a plain
/// anchor, never an executable control). Following it lands on the slice-11 detail
/// thread for that claim.
///
/// Given Maria's store holds a countered peer claim;
/// When Maria opens the Peer Claims list and follows the "Countered" marker;
/// Then the marker is an `<a href="/claims/{cid}">` link, and following it shows the
///   slice-11 counter thread for that claim.
///
/// @us-cf-002 @driving_port @real-io @drill-link @one-hop @i-cf-6 @happy
#[test]
fn the_peer_claims_countered_marker_is_a_render_only_one_hop_link_to_the_thread() {
    let env = TestEnv::initialized();
    let seeded = seed_peer_claims_one_countered(&env);
    let countered_cid = seeded
        .countered_cids
        .first()
        .expect("CF-4: the seed yields exactly one countered peer target");
    let server = ViewerServer::start(&env);

    let list = server.get("/peer-claims");
    assert_eq!(
        list.status, 200,
        "GET /peer-claims must be 200; body was:\n{}",
        list.body
    );

    // (a) The marker on the countered row is the render-only one-hop anchor
    // `<a href="/claims/{cid}">Countered</a>` — navigation TEXT, never an executable
    // control (I-CF-1 / I-CF-6).
    assert_peer_claim_flag_links_to_thread(&list.body, countered_cid);

    // (b) Following that href (the one-hop drill, no JS) lands on the slice-11 counter
    // thread for the claim: a 200 detail page that IS the claim's thread (it names the
    // claim's CID + carries the neutral "Countered" presence flag the slice-11 thread
    // renders).
    let detail = server.get(&format!("/claims/{countered_cid}"));
    assert_eq!(
        detail.status, 200,
        "GET /claims/{countered_cid} (the one-hop drill target) must be 200; body was:\n{}",
        detail.body
    );
    assert!(
        detail.body.contains(countered_cid),
        "CF-4: the drilled-into detail page must be the thread for {countered_cid:?} (it \
         names the claim's CID); body was:\n{}",
        detail.body
    );
    assert_counter_thread_presence_flag_is_neutral(&detail.body);
}

/// CF-NoNoise-PEER (US-CF-002 — no-noise discipline; I-CF-2): a store with NO counters
/// renders the `/peer-claims` list byte-identically to slice-06 — no "Countered" marker
/// anywhere, no "0 counters" empty-state noise — AND the recorded slice-06 row order is
/// preserved (the flag re-ordered / re-paged NOTHING). `counter_presence_for` returns
/// the EMPTY set -> no row is flagged.
///
/// Given Maria's store holds pulled peer claims and NOTHING counters any of them;
/// When she opens the Peer Claims list;
/// Then every row renders as in slice-06, no "Countered" marker and no empty-state noise,
///   in the unchanged slice-06 order.
///
/// @us-cf-002 @driving_port @real-io @no-noise @empty-set @shown-never-applied @i-cf-2
/// @happy
#[test]
fn a_store_with_no_counters_renders_the_peer_claims_list_exactly_as_slice_06() {
    let env = TestEnv::initialized();
    let seeded = seed_peer_claims_none_countered(&env);
    let server = ViewerServer::start(&env);

    let page = server.get("/peer-claims");

    assert_eq!(
        page.status, 200,
        "GET /peer-claims must be 200; body was:\n{}",
        page.body
    );

    // Every row renders as in slice-06: NO "Countered" marker on any row, no empty-state
    // noise. With NO counters, the flag text appears NOWHERE.
    for cid in &seeded.uncountered_cids {
        assert_peer_claim_row_not_flagged(&page.body, cid);
    }
    assert!(
        !page.body.contains(LIST_COUNTERED_FLAG_TEXT),
        "CF-NoNoise: a store with NO counters must carry NO {LIST_COUNTERED_FLAG_TEXT:?} \
         marker anywhere on /peer-claims (empty presence set -> nothing rendered; \
         US-CF-002 / I-CF-2); body was:\n{}",
        page.body
    );
    // The flag re-ordered / re-paged NOTHING — the recorded slice-06 order is byte-exact.
    assert_peer_claims_order_byte_identical(&page.body, &seeded.ordered_cids);
}

// =============================================================================
// US-CF-003 — the neutral "Countered" presence flag on the GRAPH-TRAVERSAL /project +
// /philosophy EDGE rows (CF-5..CF-8 + CF-N1). ONE shared render arm covers BOTH routes.
// =============================================================================

/// CF-5 (US-CF-003 — a countered `/project` edge is flagged in its UNCHANGED group +
/// position): `GET /project?subject=<seeded>` over a store where one of the project's
/// edges (a claim by some author) is countered -> that edge row carries the neutral
/// "Countered" `<a href="/claims/{cid}">` marker, the other edges carry none, and the
/// survey grouping + edge order + deduped contributor list are UNCHANGED.
///
/// Given Maria's `/project` survey has an edge whose claim has >= 1 counter, in a group;
/// When she opens `GET /project?subject=<seeded>`;
/// Then that edge shows the neutral "Countered" marker linking to its `/claims/{cid}`
///   thread, the other edges carry none, and the edge stays in its original group at its
///   original position.
///
/// @us-cf-003 @driving_port @real-io @project @i-cf-6 @i-cf-9 @happy
#[test]
fn a_countered_edge_in_a_project_survey_is_flagged_in_its_unchanged_position() {
    let env = TestEnv::initialized();
    let seeded = seed_project_survey_one_edge_countered(&env);
    let server = ViewerServer::start(&env);

    let page = server.get(&format!("/project?subject={}", seeded.entity));

    assert_eq!(
        page.status, 200,
        "GET /project?subject={} must be 200; body was:\n{}",
        seeded.entity, page.body
    );

    // ONLY the genuinely-countered edges carry the neutral "Countered" marker; every
    // un-countered edge carries NONE (renders exactly as slice-10).
    for countered in &seeded.countered_cids {
        assert_edge_flagged_countered(&page.body, countered);
    }
    for uncountered in &seeded.uncountered_cids {
        assert_edge_not_flagged(&page.body, uncountered);
    }

    // CARDINAL no-regroup gold (I-CF-9): with the additive markers elided, the grouping,
    // group order, edge order, and deduped contributor list are byte-identical to the
    // slice-10 render — the flag re-grouped / re-ordered NOTHING.
    assert_survey_grouping_and_order_byte_identical(&page.body, &seeded.ordered_cids);
}

/// CF-6 (US-CF-003 — the SYMMETRIC `/philosophy` edge flag, SAME render arm): `GET
/// /philosophy?object=<seeded>` -> the countered edges across the survey's groups are
/// flagged, the others none; the group order, the edge order within each group, and the
/// deduped contributor list are byte-identical to the no-flag (slice-10) render. The ONE
/// `render_edge_row` arm serves BOTH routes (US-CF-003 AC).
///
/// Given Maria's `/philosophy` survey renders several groups, a KNOWN subset of edges
///   countered;
/// When she opens `GET /philosophy?object=<seeded>`;
/// Then exactly the countered edges show the marker and the others render exactly as
///   slice-10, with grouping/order/contributor list byte-identical to the no-flag render.
///
/// @us-cf-003 @driving_port @real-io @philosophy @symmetric @i-cf-6 @i-cf-9 @happy
#[test]
fn a_philosophy_survey_flags_only_countered_edges_and_never_regroups_or_reorders() {
    let env = TestEnv::initialized();
    let seeded = seed_philosophy_survey_one_edge_countered(&env);
    let server = ViewerServer::start(&env);

    let page = server.get(&format!("/philosophy?object={}", seeded.entity));

    assert_eq!(
        page.status, 200,
        "GET /philosophy?object={} must be 200; body was:\n{}",
        seeded.entity, page.body
    );

    // Exactly the countered edges show the marker; every other edge renders as slice-10.
    for countered in &seeded.countered_cids {
        assert_edge_flagged_countered(&page.body, countered);
    }
    for uncountered in &seeded.uncountered_cids {
        assert_edge_not_flagged(&page.body, uncountered);
    }

    // CARDINAL no-regroup gold (I-CF-9): grouping, group order, edge order, deduped
    // contributor list byte-identical to the slice-10 render with markers elided. This is
    // the SAME shared `render_edge_row` arm as `/project`, exercised on `/philosophy`.
    assert_survey_grouping_and_order_byte_identical(&page.body, &seeded.ordered_cids);
}

/// CF-7 (US-CF-003 — no-noise on the edge surfaces; I-CF-2): a survey with NO counters
/// renders the `/project` (and, symmetrically, `/philosophy`) survey exactly as slice-10
/// — no "Countered" marker anywhere, no empty-state noise — AND the slice-10 grouping +
/// edge order is preserved (`counter_presence_for` returns the EMPTY set).
///
/// Given Maria's survey has no counters at all;
/// When she opens `GET /project?subject=<seeded>`;
/// Then every edge renders exactly as slice-10, with no marker, no noise, and the
///   grouping/order byte-identical to the no-flag render.
///
/// @us-cf-003 @driving_port @real-io @no-noise @empty-set @i-cf-2 @i-cf-9 @happy
#[test]
fn a_survey_with_no_counters_renders_the_edges_exactly_as_slice_10() {
    let env = TestEnv::initialized();
    let seeded = seed_survey_none_countered(&env, "project");
    let server = ViewerServer::start(&env);

    let page = server.get(&format!("/project?subject={}", seeded.entity));

    assert_eq!(
        page.status, 200,
        "GET /project?subject={} must be 200; body was:\n{}",
        seeded.entity, page.body
    );

    // No edge is flagged; the flag text appears NOWHERE (empty presence set).
    for cid in &seeded.uncountered_cids {
        assert_edge_not_flagged(&page.body, cid);
    }
    assert!(
        !page.body.contains(LIST_COUNTERED_FLAG_TEXT),
        "CF-7 (no-noise): a survey with NO counters must carry NO {LIST_COUNTERED_FLAG_TEXT:?} \
         marker anywhere (empty presence set -> nothing rendered; US-CF-003 / I-CF-2); \
         body was:\n{}",
        page.body
    );
    // The grouping + edge order is byte-identical to the slice-10 render.
    assert_survey_grouping_and_order_byte_identical(&page.body, &seeded.ordered_cids);
}

/// CF-8 (US-CF-003 — edge one-hop link + presence-only single marker): the "Countered"
/// marker on a countered EDGE is a render-only `<a href="/claims/{cid}">` ONE-HOP link
/// to that claim's slice-11 thread (navigable without JS), AND an edge countered by TWO
/// distinct authors shows EXACTLY ONE neutral marker (never "disputed by 2") in its
/// unchanged group/position. Exercised on `/project` (the same render arm covers
/// `/philosophy`).
///
/// Given Maria's `/project` survey has an edge countered by two distinct authors;
/// When she opens the survey and follows the marker;
/// Then the edge shows EXACTLY ONE neutral "Countered" marker which is a render-only
///   `<a href="/claims/{cid}">` link, and following it shows the slice-11 thread.
///
/// @us-cf-003 @driving_port @real-io @presence-only @anti-merging @one-hop @i-cf-3
/// @i-cf-6 @kpi-graph-2 @gold
#[test]
fn a_twice_countered_edge_shows_one_neutral_marker_linking_to_its_thread() {
    let env = TestEnv::initialized();
    let seeded = seed_project_survey_edge_two_counters_distinct_authors(&env);
    let target_cid = seeded
        .countered_cids
        .first()
        .expect("CF-8: the seed yields exactly one twice-countered edge")
        .clone();
    let server = ViewerServer::start(&env);

    let page = server.get(&format!("/project?subject={}", seeded.entity));
    assert_eq!(
        page.status, 200,
        "GET /project?subject={} must be 200; body was:\n{}",
        seeded.entity, page.body
    );

    // The twice-countered edge shows EXACTLY ONE neutral marker (presence-only — DISTINCT
    // referenced_cid collapses the two distinct-author counters to one membership), never
    // "disputed by 2" (I-CF-3 / KPI-GRAPH-2).
    assert_edge_flag_is_single_neutral_presence(&page.body, &target_cid);
    // The marker is the render-only one-hop link to the slice-11 thread (navigation TEXT).
    assert_edge_flag_links_to_thread(&page.body, &target_cid);

    // Following the one-hop link lands on the slice-11 thread for that edge's claim.
    let detail = server.get(&format!("/claims/{target_cid}"));
    assert_eq!(
        detail.status, 200,
        "GET /claims/{target_cid} (the one-hop drill target) must be 200; body was:\n{}",
        detail.body
    );
    assert!(
        detail.body.contains(target_cid.as_str()),
        "CF-8: the drilled-into detail page must be the thread for {target_cid:?}; body \
         was:\n{}",
        detail.body
    );
    assert_counter_thread_presence_flag_is_neutral(&detail.body);
}

/// CF-N1 (US-CF-003 / US-CF-001 — N+1-flatten behavioral proxy; I-CF-8 / ADR-050): a
/// LARGE `/project` survey with MANY edges across MANY groups and a KNOWN countered
/// subset flags EVERY countered edge correctly — and only those — in ONE request, with
/// no per-group/per-edge degradation. The at-this-layer behavioral proxy for the single
/// flattened presence call (the strict 1-query bound is a DELIVER `adapter-duckdb`
/// unit/property assertion; query count is not observable at the subprocess AT layer).
/// If the presence read were per-group or per-edge, a large multi-group survey would
/// either degrade or mis-flag under the fan-out; this proxy pins that the WHOLE survey
/// is flagged correctly in one shot from the single flattened call (ADR-050).
///
/// Given Maria's `/project` survey holds MANY edges across MANY groups, a known subset
///   of which are countered;
/// When she opens the survey (ONE request);
/// Then EVERY countered edge carries the marker and EVERY un-countered edge does not —
///   the whole survey is flagged correctly in a single request, grouping unchanged.
///
/// @us-cf-001 @us-cf-003 @driving_port @real-io @n-plus-1-guard @i-cf-8 @i-cf-9 @gold
#[test]
fn a_large_multi_group_survey_flags_every_countered_edge_correctly_in_one_request() {
    let env = TestEnv::initialized();
    let seeded = seed_project_survey_many_groups_known_countered_subset(&env);
    // Sanity: the proxy is only meaningful over a genuinely large multi-group survey with
    // a real countered subset AND un-countered edges. Pin both so the seed cannot silently
    // shrink the survey (which would hollow out the N+1 proxy).
    assert!(
        !seeded.countered_cids.is_empty(),
        "CF-N1: the large survey must carry a non-empty countered subset; got {:?}",
        seeded.countered_cids
    );
    assert!(
        !seeded.uncountered_cids.is_empty(),
        "CF-N1: the large survey must carry un-countered edges too (a MIXED survey); got {:?}",
        seeded.uncountered_cids
    );

    let server = ViewerServer::start(&env);

    // WHEN Maria opens the survey — ONE GET request renders the whole multi-group survey.
    let page = server.get(&format!("/project?subject={}", seeded.entity));

    assert_eq!(
        page.status, 200,
        "GET /project?subject={} must be 200; body was:\n{}",
        seeded.entity, page.body
    );

    // THEN in that SINGLE response EVERY countered edge carries the marker and EVERY
    // un-countered edge carries NONE — the whole multi-group survey is flagged correctly
    // in one request (the subprocess-layer behavioral proxy for the ADR-050 single
    // flattened presence call; the strict 1-query bound is the DELIVER adapter test).
    for countered in &seeded.countered_cids {
        assert_edge_flagged_countered(&page.body, countered);
    }
    for uncountered in &seeded.uncountered_cids {
        assert_edge_not_flagged(&page.body, uncountered);
    }
    // And the grouping/order is byte-identical to slice-10 even at this size (I-CF-9).
    assert_survey_grouping_and_order_byte_identical(&page.body, &seeded.ordered_cids);
}
