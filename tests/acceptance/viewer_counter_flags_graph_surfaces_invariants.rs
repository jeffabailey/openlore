//! Slice-13 acceptance — counter-presence-FLAG GOLD / guardrail invariants over the
//! FEDERATED `/peer-claims` list AND the GRAPH-TRAVERSAL `/project` + `/philosophy` edge
//! surveys (the cross-cutting I-CF-1/2/5/8/9 guardrails that must hold over the WHOLE
//! flagged surface, beyond any single story).
//!
//! These are the load-bearing, release-relevant guardrail gold tests for the slice-13
//! flag DELTA — the BEHAVIORAL layer of the three-layer enforcement (type + xtask
//! `check-arch` are the other two, owned by DELIVER; the xtask delta is NONE per
//! component-boundaries.md §6, so the behavioral gold is the active slice-13 guardrail
//! layer). They drive the REAL `openlore ui` verb via the `ViewerServer` subprocess +
//! in-test HTTP (with/without `HX-Request`) over a REAL seeded LOCAL DuckDB, with NO
//! mocked boundary (the batch presence read is a LOCAL DB-index lookup + a PURE
//! projection/render — OFFLINE by construction: these routes have NO outbound edge).
//! They assert the hard slice-13 invariants on the OBSERVABLE surface across BOTH the
//! `/peer-claims` row surface and the `/project`+`/philosophy` edge surface (and both
//! shapes — full page + htmx fragment):
//!
//! - `every_flagged_graph_surface_render_leaves_the_store_read_only` (CF-INV-ReadOnly,
//!   I-CF-1 / KPI-VIEW-2): exercising `/peer-claims` + `/project` + `/philosophy` across
//!   postures (countered + un-countered) AND both shapes leaves `claims` + `peer_claims`
//!   row counts UNCHANGED, via the universe-bound `assert_store_read_only` (Mandate 8;
//!   universe = the two port-exposed counts, all `unchanged`). The REUSED presence read
//!   is a read-only SELECT and persists nothing on ANY of the three surfaces.
//! - `no_flagged_graph_surface_render_adds_a_write_or_sign_control` (CF-INV-NoWrite,
//!   I-CF-1): no response shape on ANY of the three surfaces renders a write / sign /
//!   counter / publish / subscribe control — authoring stays the slice-03 CLI; the
//!   "Countered" markers are render-only `<a href="/claims/{cid}">` navigation TEXT.
//! - `the_flagged_graph_surface_chrome_stays_offline_no_cdn` (CF-INV-OfflineChrome,
//!   I-CF-5 / KPI-HX-G2): the flagged full pages reference ONLY the LOCAL
//!   `/static/htmx.min.js` script src and NO off-host CDN, on all three surfaces.
//! - `the_flagged_graph_surfaces_render_fully_offline` (CF-INV-Offline, I-CF-5 / KPI-5):
//!   the flag renders fully with the network unavailable on all three surfaces — the
//!   presence read is a LOCAL DB-index lookup (ref-tables-only) with NO outbound edge.
//!   Peer counters were verified at `peer pull` time; the viewer re-verifies nothing.
//! - `the_traversal_grouping_and_order_are_byte_identical_with_and_without_flags`
//!   (CF-INV-ShownNeverApplied, the CARDINAL shown-never-applied / no-regroup GOLD —
//!   I-CF-9 / ADR-015 / slice-10 I-GT-3/4): the SAME store's rendered grouping, group
//!   order, edge order, and deduped contributor list are byte-IDENTICAL whether or not
//!   the flag feature is active — the flag never re-groups/re-orders the survey. A
//!   regression silently lets the flag pick a traversal order or re-group contested
//!   edges; this gold makes it unshippable. (The `/peer-claims` order byte-identity gold
//!   is asserted in the story file CF-NoNoise; this CARDINAL invariant pins the harder
//!   EDGE no-regroup guarantee on both traversal routes.)
//! - `a_large_multi_group_survey_resolves_presence_in_one_request` (CF-INV-N1, I-CF-8 /
//!   ADR-050): the N+1-flatten behavioral proxy — a survey of MANY edges across MANY
//!   groups flags the countered subset correctly in ONE request (the edge-CID flatten
//!   across groups is a single presence call; the strict 1-query bound is a DELIVER
//!   `adapter-duckdb` unit/property test).
//!
//! Driving discipline (Mandate 1): every assertion enters through the REAL `openlore ui`
//! subprocess + HTTP — never internal `viewer-domain` `render_*` / `group_*` fns or
//! `counter_presence_for` directly. The local DuckDB is REAL (seeded via the production
//! `peer add` + `peer pull` federation paths + a DISTINCT peer's verifiable counter);
//! there is NO mocked boundary (these routes are LOCAL reads).
//!
//! Layer placement (Mandate 9/11): layer-3/layer-5 subprocess + real-I/O, EXAMPLE-only.
//! These guardrails are example-based, never PBT-generated at this layer (the `@property`
//! tag marks them as universal invariants for the reader + the DELIVER crafter; the
//! generative exploration of the pure projection/render is a layer-1/2 concern in the
//! DELIVER `viewer-domain` units, out of this file's scope). The strict single-query N+1
//! bound is likewise a DELIVER `adapter-duckdb` unit/property assertion — at this layer
//! the N+1 guard is the CF-INV-N1 behavioral proxy.
//!
//! Build-before-run note: as with the story file, the run MUST `cargo build` the
//! `openlore` (viewer) bin before running these ATs. No second binary is needed — the
//! presence read is a LOCAL read.
//!
//! Mandate 7 RED scaffolds: each body reaches a `todo!()` (via the `todo!()`-stubbed
//! seed / assert helpers or directly) -> panics -> classifies RED (MISSING_FUNCTIONALITY),
//! NOT BROKEN. They stay RED until DELIVER.
//!
//! Covers: the cross-cutting I-CF-1 / I-CF-2 / I-CF-5 / I-CF-8 / I-CF-9 guardrails over
//! the whole flagged `/peer-claims` + `/project` + `/philosophy` surface (the gold
//! companions to the US-CF-002/003 story scenarios in
//! `viewer_counter_flags_graph_surfaces.rs`).
//
// SCAFFOLD: true

mod support;

#[allow(unused_imports)]
use support::*;

// =============================================================================
// I-CF-1 / KPI-VIEW-2 — read-only preserved: the flagged /peer-claims + /project +
// /philosophy surfaces + every posture + shape leave the store unchanged
// (CF-INV-ReadOnly). The REUSED presence read persists nothing on any surface.
// =============================================================================

/// CF-INV-ReadOnly / GOLD `every_flagged_graph_surface_render_leaves_the_store_read_only`
/// (I-CF-1 / KPI-VIEW-2): exercising the flagged `/peer-claims`, `/project`, and
/// `/philosophy` surfaces — in BOTH shapes (full page + htmx fragment) — leaves the
/// `claims` + `peer_claims` row counts UNCHANGED. The slice-13 companion to the
/// slice-06/10/12 read-only gold tests, asserted via the universe-bound state-delta
/// (Mandate 8: universe = the two port-exposed counts, each `unchanged`). The REUSED
/// batch presence read is a read-only SELECT and persists nothing on ANY surface.
///
/// Given a store seeded with a countered peer claim AND a countered survey edge;
/// When the `/peer-claims` + `/project` + `/philosophy` surfaces (both shapes) are
///   exercised;
/// Then the `claims` and `peer_claims` row counts are UNCHANGED.
///
/// @us-cf-002 @us-cf-003 @property @driving_port @real-io @read-only @i-cf-1 @gold
#[test]
fn every_flagged_graph_surface_render_leaves_the_store_read_only() {
    // GIVEN a REAL store seeded so the universe is NON-TRIVIAL: a /peer-claims list with
    // a countered peer row AND a /project survey with a countered edge (both via the
    // production peer add + peer pull paths) so the read-only delta is over a POPULATED
    // store (a `0 == 0` delta would not prove the viewer leaves a populated store
    // untouched). Capture the read-only universe (port-exposed counts) BEFORE any route.
    let env = TestEnv::initialized();
    let _peers = seed_peer_claims_one_countered(&env);
    let edges = seed_project_survey_one_edge_countered(&env);

    let before = capture_store_row_count_universe(&env);

    // WHEN the flagged surfaces are exercised in BOTH shapes — inside a scope so the
    // viewer's exclusive DuckDB lock is RELEASED before the `after` snapshot.
    {
        let server = ViewerServer::start(&env);
        let routes = [
            "/peer-claims".to_string(),
            format!("/project?subject={}", edges.entity),
        ];
        for route in &routes {
            let full = server.get(route);
            let fragment = server.get_htmx(route);
            for (label, response) in [("full page", &full), ("fragment", &fragment)] {
                assert_eq!(
                    response.status, 200,
                    "CF-INV-ReadOnly: GET {route} ({label}) must be 200; body was:\n{}",
                    response.body
                );
            }
        }
    }

    // THEN the persisted-store row counts are UNCHANGED — any change is an UNSHIPPABLE
    // write-surface breach (I-CF-1 / KPI-VIEW-2). The presence read is a read-only SELECT
    // and persists nothing.
    let after = capture_store_row_count_universe(&env);
    assert_store_read_only(&before, &after);
}

// =============================================================================
// I-CF-1 — no write/sign/counter control on ANY flagged surface response shape
// (CF-INV-NoWrite). The human gate stays the CLI; markers are render-only `<a href>`.
// =============================================================================

/// CF-INV-NoWrite / GOLD `no_flagged_graph_surface_render_adds_a_write_or_sign_control`
/// (I-CF-1): NO response shape on ANY of the three flagged surfaces (full page or
/// fragment, countered or not) renders a write / sign / counter / publish / subscribe
/// control — authoring stays EXCLUSIVELY in the slice-03 CLI, and every "Countered"
/// marker is render-only navigation TEXT (an `<a href="/claims/{cid}">` anchor), never a
/// control. Asserted on the observable rendered surface across every shape, reusing the
/// slice-10 traversal no-write blocklist on the edge surfaces + the slice-12 list no-write
/// blocklist on `/peer-claims`.
///
/// Given the viewer renders flagged `/peer-claims` + `/project` + `/philosophy` surfaces;
/// When every response shape (full page + fragment) is inspected;
/// Then none renders a write / sign / counter / publish / subscribe control, and every
///   `/claims/{cid}` reference is render-only `<a href>` navigation TEXT.
///
/// @us-cf-002 @us-cf-003 @property @driving_port @real-io @read-only @i-cf-1 @gold
#[test]
fn no_flagged_graph_surface_render_adds_a_write_or_sign_control() {
    // GIVEN a store seeded with a countered peer row AND a countered survey edge (so
    // every flagged surface has at least one marker present).
    let env = TestEnv::initialized();
    let _peers = seed_peer_claims_one_countered(&env);
    let edges = seed_project_survey_one_edge_countered(&env);

    // WHEN every flagged-surface response shape is collected in a scope (so the viewer's
    // DuckDB lock releases on drop).
    let bodies: Vec<(String, String)> = {
        let server = ViewerServer::start(&env);
        let project = format!("/project?subject={}", edges.entity);
        let shapes = [
            ("/peer-claims full page".to_string(), server.get("/peer-claims")),
            ("/peer-claims fragment".to_string(), server.get_htmx("/peer-claims")),
            ("/project full page".to_string(), server.get(&project)),
            ("/project fragment".to_string(), server.get_htmx(&project)),
        ];
        shapes
            .into_iter()
            .map(|(label, response)| {
                assert_eq!(
                    response.status, 200,
                    "CF-INV-NoWrite: GET {label} must be 200; body was:\n{}",
                    response.body
                );
                (label, response.body)
            })
            .collect()
    };

    // THEN no shape carries a write / sign / counter / publish / subscribe / follow
    // affordance (the viewer holds no key; authoring stays the slice-03 CLI), AND every
    // `/claims/{cid}` reference is a render-only `<a href>` anchor (navigation TEXT,
    // never an executable control; I-CF-1). The traversal no-write blocklist
    // (`assert_traversal_html_has_no_write_or_sign_control`) covers the sign/follow/
    // subscribe affordances; the per-reference anchor scan covers the flag links.
    for (label, body) in &bodies {
        assert_traversal_html_has_no_write_or_sign_control(body);

        for (idx, _) in body.match_indices("/claims/") {
            let prefix = &body[..idx];
            let anchor_open = prefix.rfind("<a href");
            let tag_open = prefix.rfind('<');
            assert!(
                anchor_open.is_some() && anchor_open == tag_open,
                "I-CF-1 ({label}): every `/claims/` reference must be a render-only \
                 `<a href>` navigation anchor (never a write/control); the reference at \
                 byte {idx} is not inside an `<a href` tag; body was:\n{body}"
            );
        }
    }
}

// =============================================================================
// I-CF-5 / KPI-HX-G2 — offline chrome: the flagged surfaces reference only the local
// vendored htmx asset, no CDN (CF-INV-OfflineChrome).
// =============================================================================

/// CF-INV-OfflineChrome / GOLD `the_flagged_graph_surface_chrome_stays_offline_no_cdn`
/// (I-CF-5 / KPI-HX-G2): the flagged `/peer-claims` + `/project` full pages reference
/// ONLY the LOCAL `/static/htmx.min.js` script src and NO off-host CDN — the page CHROME
/// stays offline-capable (and so does the FLAG itself, since the presence read is LOCAL).
///
/// Given the viewer renders the flagged `/peer-claims` + `/project` full pages;
/// When the pages' script references are inspected;
/// Then the only htmx asset reference is the local /static/htmx.min.js — no CDN.
///
/// @us-cf-002 @us-cf-003 @property @driving_port @real-io @offline @no-cdn @i-cf-5 @gold
#[test]
fn the_flagged_graph_surface_chrome_stays_offline_no_cdn() {
    let env = TestEnv::initialized();
    let _peers = seed_peer_claims_one_countered(&env);
    let edges = seed_project_survey_one_edge_countered(&env);

    // WHEN the flagged full pages are rendered — under the plain, store-only
    // `ViewerServer::start` (NO /scrape GitHub seam, NO /search indexer seam): the
    // presence read is a LOCAL DB-index lookup, so there is no outbound edge to wire.
    let server = ViewerServer::start(&env);
    let peer_full = server.get("/peer-claims");
    let project_full = server.get(&format!("/project?subject={}", edges.entity));

    for (label, response) in [("/peer-claims", &peer_full), ("/project", &project_full)] {
        assert_eq!(
            response.status, 200,
            "CF-INV-OfflineChrome: GET {label} (full page) must be 200; body was:\n{}",
            response.body
        );
        assert!(
            response.is_full_page(),
            "CF-INV-OfflineChrome: the no-HX {label} response must be a full page; body \
             was:\n{}",
            response.body
        );
        // THEN the full page references NO off-host CDN — the only htmx asset is the LOCAL
        // /static/htmx.min.js, so the page CHROME (and, since the flag read is LOCAL, the
        // FLAG itself) stays offline-capable (I-CF-5 / KPI-HX-G2).
        assert!(
            !response.references_external_cdn(),
            "CF-INV-OfflineChrome: the flagged {label} full page must reference NO off-host \
             CDN (only the local /static/htmx.min.js); body was:\n{}",
            response.body
        );
    }
}

// =============================================================================
// I-CF-5 / KPI-5 — local-first / offline: the flag renders with the network unavailable
// (CF-INV-Offline). The presence read is a LOCAL DB-index lookup with NO outbound edge.
// =============================================================================

/// CF-INV-Offline / GOLD `the_flagged_graph_surfaces_render_fully_offline` (I-CF-5 /
/// KPI-5): the flagged `/peer-claims` + `/project` surfaces render fully with NO network
/// available — the presence read (the INDEXED `referenced_cid IN (...)` ref lookup) is
/// LOCAL, with NO per-row artifact read and NO outbound edge, so the network being down
/// NEVER degrades it. The countered peer row / edge (countered by a PULLED PEER, already
/// verified at `peer pull` time) STILL carries its marker; the viewer re-verifies nothing.
///
/// Given the viewer is started over a seeded store with NO network seam wired;
/// When the flagged `/peer-claims` + `/project` surfaces are opened;
/// Then the countered row / edge STILL carries the "Countered" marker, with no degraded
///   state and no network call.
///
/// @us-cf-002 @us-cf-003 @property @driving_port @real-io @offline @local-first @i-cf-5
/// @kpi-5 @gold
#[test]
fn the_flagged_graph_surfaces_render_fully_offline() {
    let env = TestEnv::initialized();
    let peers = seed_peer_claims_one_countered(&env);
    let edges = seed_project_survey_one_edge_countered(&env);
    let peer_countered = peers
        .countered_cids
        .first()
        .expect("CF-INV-Offline: the /peer-claims seed must produce one countered row")
        .clone();
    let edge_countered = edges
        .countered_cids
        .first()
        .expect("CF-INV-Offline: the /project seed must produce one countered edge")
        .clone();

    // WHEN the flagged surfaces are opened under the plain, store-only
    // `ViewerServer::start` — NEITHER the /scrape GitHub seam NOR the /search indexer seam
    // is wired, so the LOCAL-only viewer has NO outbound edge: the presence read is a LOCAL
    // DB-index lookup, OFFLINE by construction. The scope releases the DuckDB lock on drop.
    let (peer_body, project_body) = {
        let server = ViewerServer::start(&env);
        let peer = server.get("/peer-claims");
        let project = server.get(&format!("/project?subject={}", edges.entity));
        for (label, response) in [("/peer-claims", &peer), ("/project", &project)] {
            assert_eq!(
                response.status, 200,
                "CF-INV-Offline: GET {label} must be 200; body was:\n{}",
                response.body
            );
        }
        (peer.body, project.body)
    };

    // THEN the countered peer row + edge STILL carry their render-only "Countered" marker
    // — the peer counter was verified at pull time; the viewer re-verifies nothing, makes
    // no network call, and never degrades (I-CF-5 / KPI-5).
    assert_peer_claim_row_flagged_countered(&peer_body, &peer_countered);
    assert_edge_flagged_countered(&project_body, &edge_countered);
    for (label, body) in [("/peer-claims", &peer_body), ("/project", &project_body)] {
        let lower = body.to_lowercase();
        for notice in ["unavailable", "network error", "could not reach", "try again"] {
            assert!(
                !lower.contains(notice),
                "CF-INV-Offline: the offline-rendered {label} ({notice:?}) must show NO \
                 degraded notice — the presence read is LOCAL, no outbound edge to take \
                 down; body was:\n{body}"
            );
        }
    }
}

// =============================================================================
// I-CF-9 / ADR-015 / slice-10 I-GT-3/4 — CARDINAL shown-never-applied / no-regroup: the
// survey grouping, group order, edge order, and contributor list are byte-identical with
// and without the flag (CF-INV-ShownNeverApplied). THE CARDINAL GOLD.
// =============================================================================

/// CF-INV-ShownNeverApplied / CARDINAL NO-REGROUP GOLD
/// `the_traversal_grouping_and_order_are_byte_identical_with_and_without_flags` (I-CF-9 /
/// ADR-015 / slice-10 I-GT-3/4): the SAME store's rendered survey GROUPING, group order,
/// edge order, and deduped CONTRIBUTOR list are byte-IDENTICAL whether or not the flag is
/// active — the flag never re-groups, re-orders, or re-deduplicates the survey. This is
/// the load-bearing slice-13 invariant on the EDGE surfaces: the flag is additive context
/// BESIDE each edge and changes NOTHING about which edges appear, in which group, in which
/// order, or which contributors are listed. A no-regroup breach silently lets the flag
/// pick a traversal order or pull contested edges together; this gold makes it
/// unshippable. Asserted on the OBSERVABLE rendered HTML across BOTH traversal routes
/// (`/project` + `/philosophy`, the SAME shared render arm) with markers elided.
///
/// Given the SAME store (a survey with a mix of countered + un-countered edges across
///   groups) is rendered with the flag, on both traversal routes;
/// When `/project` and `/philosophy` render;
/// Then the grouping, group order, edge order, and deduped contributor list are
///   byte-identical to the slice-10 render with the additive markers elided.
///
/// @us-cf-003 @property @driving_port @real-io @shown-never-applied @no-regroup @i-cf-9
/// @cardinal @gold
#[test]
fn the_traversal_grouping_and_order_are_byte_identical_with_and_without_flags() {
    // GIVEN a /project survey AND a /philosophy survey, each with a mix of countered +
    // un-countered edges across groups (the SAME shared render arm). The recorded slice-10
    // grouping + edge order is the seed's `ordered_cids`, so grouping/order are directly
    // comparable. Mirrors the slice-12 baseline+marker-elision tactic (b): there is NO
    // pre-flag binary and NO no-flag HTTP seam (the route ALWAYS reads
    // `counter_presence_for`), so the slice-10 reference is the RECORDED edge order, and
    // the gold ELIDES the additive `<a href="/claims/{cid}">Countered</a>` anchors and
    // proves the remaining slice-10 body honours that recorded grouping/order byte-for-byte.
    let project_env = TestEnv::initialized();
    let project = seed_project_survey_one_edge_countered(&project_env);

    let philosophy_env = TestEnv::initialized();
    let philosophy = seed_philosophy_survey_one_edge_countered(&philosophy_env);

    // WHEN both traversal routes render their flagged surveys over their SAME stores.
    let project_body = {
        let server = ViewerServer::start(&project_env);
        let response = server.get(&format!("/project?subject={}", project.entity));
        assert_eq!(
            response.status, 200,
            "CF-INV-ShownNeverApplied: GET /project?subject={} must be 200; body was:\n{}",
            project.entity, response.body
        );
        response.body
    };
    let philosophy_body = {
        let server = ViewerServer::start(&philosophy_env);
        let response = server.get(&format!("/philosophy?object={}", philosophy.entity));
        assert_eq!(
            response.status, 200,
            "CF-INV-ShownNeverApplied: GET /philosophy?object={} must be 200; body was:\n{}",
            philosophy.entity, response.body
        );
        response.body
    };

    // THEN for BOTH routes, with the additive "Countered" markers elided, the grouping,
    // group order, edge order, and deduped contributor list are byte-IDENTICAL to the
    // recorded slice-10 baseline — the flag is additive ONLY (I-CF-9). Any divergence
    // (re-group, re-order) is an UNSHIPPABLE no-regroup breach. The SAME shared
    // `render_edge_row` arm is pinned on both routes, so they cannot drift.
    assert_survey_grouping_and_order_byte_identical(&project_body, &project.ordered_cids);
    assert_survey_grouping_and_order_byte_identical(&philosophy_body, &philosophy.ordered_cids);
}

// =============================================================================
// I-CF-8 / ADR-050 — N+1-flatten behavioral proxy: a survey of MANY edges across MANY
// groups resolves presence in ONE request (CF-INV-N1).
// =============================================================================

/// CF-INV-N1 / GOLD `a_large_multi_group_survey_resolves_presence_in_one_request`
/// (I-CF-8 / ADR-050): a LARGE `/project` survey with MANY edges across MANY groups and a
/// KNOWN countered subset flags EVERY countered edge correctly — and only those — in ONE
/// request, with no per-group/per-edge degradation. The at-this-layer behavioral proxy
/// for the single flattened presence call: the edge-CID flatten collects every
/// `EdgeRow.cid` across every group from the FLAT survey rows BEFORE grouping and queries
/// ONCE (ADR-050). If the presence read were per-group or per-edge, a large multi-group
/// survey would either degrade or mis-flag under the fan-out; this proxy pins the whole
/// survey is flagged correctly in one shot. The strict 1-query bound is a DELIVER
/// `adapter-duckdb` unit/property test (query count is not observable at this layer).
///
/// Given Maria's `/project` survey holds MANY edges across MANY groups, a known subset
///   countered;
/// When she opens the survey (ONE request);
/// Then EVERY countered edge carries the marker and EVERY un-countered edge does not, the
///   whole survey flagged correctly in a single request with grouping unchanged.
///
/// @us-cf-001 @us-cf-003 @property @driving_port @real-io @n-plus-1-guard @i-cf-8 @gold
#[test]
fn a_large_multi_group_survey_resolves_presence_in_one_request() {
    let env = TestEnv::initialized();
    let seeded = seed_project_survey_many_groups_known_countered_subset(&env);
    // Sanity: the proxy is only meaningful over a genuinely large multi-group survey with
    // a real countered subset AND un-countered edges. Pin both so the seed cannot silently
    // shrink the survey (which would hollow out the N+1 proxy).
    assert!(
        !seeded.countered_cids.is_empty(),
        "CF-INV-N1: the large survey must carry a non-empty countered subset; got {:?}",
        seeded.countered_cids
    );
    assert!(
        !seeded.uncountered_cids.is_empty(),
        "CF-INV-N1: the large survey must carry un-countered edges too; got {:?}",
        seeded.uncountered_cids
    );

    let server = ViewerServer::start(&env);

    // WHEN Maria opens the survey — ONE GET request renders the whole multi-group survey.
    let page = server.get(&format!("/project?subject={}", seeded.entity));

    assert_eq!(
        page.status, 200,
        "CF-INV-N1: GET /project?subject={} must be 200; body was:\n{}",
        seeded.entity, page.body
    );

    // THEN every countered edge carries the marker and every un-countered edge does not —
    // the whole multi-group survey is flagged correctly in one request (the behavioral
    // proxy for the ADR-050 single flattened presence call across all groups).
    for countered in &seeded.countered_cids {
        assert_edge_flagged_countered(&page.body, countered);
    }
    for uncountered in &seeded.uncountered_cids {
        assert_edge_not_flagged(&page.body, uncountered);
    }
    // And the grouping/order is byte-identical to slice-10 even at this size (I-CF-9).
    assert_survey_grouping_and_order_byte_identical(&page.body, &seeded.ordered_cids);
}
