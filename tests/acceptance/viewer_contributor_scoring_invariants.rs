//! Slice-09 acceptance — contributor-score GOLD / guardrail invariants (the
//! cross-cutting I-CS-1/2/4/5/8 guardrails that must hold over the WHOLE `/score`
//! surface, beyond any single story).
//!
//! These are the load-bearing, release-relevant guardrail gold tests for the
//! contributor-score DELTA — the BEHAVIORAL layer of the three-layer enforcement
//! (type + xtask `check-arch` are the other two, owned by DELIVER). They drive the
//! REAL `openlore ui` verb via the `ViewerServer` subprocess + in-test HTTP
//! (with/without `HX-Request`) over a REAL seeded LOCAL DuckDB, with NO mocked
//! boundary (the score is a LOCAL read + PURE compute — distinct from `/search`,
//! which mocks the indexer). They assert the hard slice-09 invariants on the
//! OBSERVABLE surface:
//!
//! - `every_score_route_leaves_the_store_read_only` (C-INV-ReadOnly, I-CS-4 /
//!   WD-CS-3 / KPI-VIEW-2): exercising EVERY `/score` route across postures (rich /
//!   sparse / empty) AND both shapes (full page + htmx fragment) leaves `claims` +
//!   `peer_claims` row counts UNCHANGED — asserted via the universe-bound
//!   `assert_store_read_only` (Mandate 8; universe = the two port-exposed counts,
//!   all `unchanged`). The score is computed per query and never persisted.
//! - `no_score_response_adds_a_write_or_sign_control` (C-INV-NoWrite, I-CS-1 /
//!   WD-CS-3): no `/score` response shape (full page or fragment, any posture)
//!   renders a sign / publish / subscribe / executable-follow control — scoring is
//!   a read + pure compute; signing/following stays in the CLI.
//! - `the_score_page_chrome_stays_offline_no_cdn` (C-INV-OfflineChrome, I-CS-8 /
//!   KPI-HX-G2): the `/score` full page references ONLY the LOCAL
//!   `/static/htmx.min.js` script src and NO off-host CDN.
//! - `the_score_surface_works_fully_offline` (C-INV-Offline, I-CS-5 / KPI-5): the
//!   `/score` view renders fully with the network unavailable — the score DATA
//!   (not just the chrome) is a LOCAL read, distinct from `/search` and `/scrape`,
//!   the only network-requiring routes. No outbound edge exists in the `/score`
//!   path, so there is nothing to take down; the gold pins that the LOCAL read +
//!   pure compute renders identically whether or not any network is present.
//! - `a_rendered_score_is_never_shown_without_its_breakdown_summing_to_the_weight`
//!   (C-INV-Transparency, the CARDINAL gold — I-CS-2 / KPI-GRAPH-3 / J-002c): for
//!   the rich-trail contributor, the rendered breakdown subtotals sum to the
//!   displayed weight (the reproduce-by-hand release gate) AND the score is NEVER
//!   rendered without its breakdown. An opaque-number regression (showing the score
//!   but hiding the breakdown) silently re-creates the aggregator failure J-002
//!   exists to avoid — this gold makes it unshippable.
//!
//! Driving discipline (Mandate 1): every assertion enters through the REAL `openlore
//! ui` subprocess + HTTP — never internal `viewer-domain` `render_score_*` fns OR
//! the `scoring` crate directly. The local DuckDB is REAL (seeded via the production
//! `peer add` + `peer pull` path); there is NO mocked boundary (the score is LOCAL).
//!
//! Layer placement (Mandate 9/11): layer-3/layer-5 subprocess + real-I/O,
//! EXAMPLE-only. These guardrails are example-based, never PBT-generated at this
//! layer (the `@property` tag marks them as universal invariants for the reader +
//! the DELIVER crafter; the generative exploration of the pure score/render core —
//! incl. the Gate-2 `weight == Σ subtotal` PROPERTY — is a layer-1/2 concern in the
//! slice-04 `scoring` suite + the DELIVER render units, out of this file's scope).
//!
//! Build-before-run note: as with `viewer_contributor_scoring.rs`, the run MUST
//! `cargo build` the `openlore` (viewer) bin before running these ATs. No second
//! binary is needed — `/score` is a LOCAL read.
//!
//! Mandate 7 RED scaffolds: each body is `todo!()` → panics → classifies RED
//! (MISSING_FUNCTIONALITY), NOT BROKEN. They stay RED until DELIVER.
//!
//! Covers: the cross-cutting I-CS-1 / I-CS-2 / I-CS-4 / I-CS-5 / I-CS-8 guardrails
//! over the whole `/score` surface (the gold companions to the US-CS-001..003 story
//! scenarios in `viewer_contributor_scoring.rs`).
//
// SCAFFOLD: true

mod support;

#[allow(unused_imports)]
use support::*;

// =============================================================================
// I-CS-4 / WD-CS-3 — read-only preserved: every /score route + posture + shape
// leaves the store unchanged (C-INV-ReadOnly). The score is computed per query and
// persists nothing.
// =============================================================================

/// C-INV-ReadOnly / GOLD `every_score_route_leaves_the_store_read_only` (I-CS-4 /
/// WD-CS-3 / KPI-VIEW-2): exercising EVERY `/score` route — every posture (rich /
/// sparse / empty) in BOTH shapes (full page + htmx fragment) — leaves the `claims`
/// + `peer_claims` row counts UNCHANGED. The contributor-score companion to the
/// slice-06 `viewer_is_read_only` + slice-08 `every_search_route_leaves_the_store_
/// read_only` gold tests, asserted via the universe-bound state-delta (Mandate 8:
/// universe = the two port-exposed counts, each `unchanged`). The derived score is
/// recomputed per query and NEVER persisted (I-CS-4 — zero new persisted types).
///
/// Given a store seeded with a rich + a sparse contributor trail;
/// When EVERY /score route (rich/sparse/empty, full + fragment) is exercised;
/// Then the `claims` and `peer_claims` row counts are UNCHANGED.
///
/// @us-cs-001 @us-cs-002 @us-cs-003 @property @driving_port @real-io @read-only
/// @i-cs-4 @gold
#[test]
fn every_score_route_leaves_the_store_read_only() {
    // GIVEN a REAL store seeded (production `peer add` + `peer pull` path) with a
    // rich AND a sparse contributor trail so the read-only delta is over a
    // non-trivial universe (a `0 == 0` delta would not prove the viewer leaves a
    // POPULATED store untouched). Capture the read-only universe (port-exposed
    // counts: `claims.row_count`, `peer_claims.row_count`) BEFORE exercising any
    // /score route.
    // WHEN every /score route is exercised — every posture (rich/sparse/empty), both
    // shapes (get + get_htmx) — within a scope so the viewer's exclusive DuckDB lock
    // is RELEASED before the `after` snapshot (the read-only proof is about what the
    // viewer LEFT BEHIND, mirroring V-INV-1 / N-INV-ReadOnly).
    // THEN the persisted-store row counts are UNCHANGED (assert_store_read_only; any
    // change is an UNSHIPPABLE write-surface breach — I-CS-4 / WD-CS-3).
    let env = TestEnv::initialized();

    // Seed BOTH a RICH and a SPARSE contributor trail through the PRODUCTION
    // federation write path (`peer add` + `peer pull`) so the read-only universe is
    // NON-TRIVIAL (a `0 == 0` delta would not prove the viewer leaves a POPULATED
    // store untouched). A SINGLE `seed_contributor_rich_and_sparse_trails` call is
    // REQUIRED — seeding the two trails through two separate `peer pull`s would drop
    // the first peer's PDS before the second pull re-pulls it (the helper documents
    // this). The rows land in the REAL `peer_claims` table the viewer's LOCAL feed
    // read returns.
    seed_contributor_rich_and_sparse_trails(&env, CONTRIBUTOR_RICH_DID, CONTRIBUTOR_SPARSE_DID);

    // Capture the read-only universe (the two port-exposed counts:
    // `claims.row_count` + `peer_claims.row_count`) BEFORE any /score route runs
    // (Mandate 8: the universe is the inherited capture, NOT internal struct fields).
    let before = capture_store_row_count_universe(&env);

    // Every /score posture: a rich trail (multi-row breakdown), a sparse trail
    // (`[SPARSE]`), and an empty contributor (never seeded → guided `NoClaims`). All
    // three are LOCAL reads + pure compute — none may persist a derived score /
    // weight / bucket (I-CS-4 / WD-72 — zero new persisted types).
    let rich_path = format!("/score?contributor={CONTRIBUTOR_RICH_DID}");
    let sparse_path = format!("/score?contributor={CONTRIBUTOR_SPARSE_DID}");
    let empty_path = format!("/score?contributor={CONTRIBUTOR_EMPTY_DID}");

    // Exercise EVERY /score route inside a scope so the viewer's exclusive DuckDB
    // lock is RELEASED (on drop) BEFORE the `after` snapshot re-opens the store — the
    // read-only proof is about what the viewer LEFT BEHIND (mirrors the slice-06
    // V-INV-1 / slice-07 H-INV-ReadOnly / slice-08 N-INV-ReadOnly gold tests).
    {
        let viewer = ViewerServer::start(&env);

        // Every posture (rich / sparse / empty) in BOTH shapes — the no-header full
        // page (`get`) AND the htmx fragment (`get_htmx`). Each is a LOCAL read +
        // pure compute that must persist NOTHING.
        for path in [rich_path.as_str(), sparse_path.as_str(), empty_path.as_str()] {
            let full_page = viewer.get(path);
            assert_eq!(
                full_page.status, 200,
                "GET {path:?} (full page) over the LOCAL store must be 200; body:\n{}",
                full_page.body
            );
            let fragment = viewer.get_htmx(path);
            assert_eq!(
                fragment.status, 200,
                "GET {path:?} (htmx fragment) over the LOCAL store must be 200; \
                 body:\n{}",
                fragment.body
            );
        }
        // `viewer` drops here — the `openlore ui` process is killed and its exclusive
        // DuckDB lock released before the `after` snapshot.
    }

    // Capture the read-only universe AFTER every route ran.
    let after = capture_store_row_count_universe(&env);

    // The persisted-store row counts are UNCHANGED — every universe slot `unchanged`
    // (any change is an UNSHIPPABLE write-surface breach; I-CS-4 / WD-CS-3). The
    // score was recomputed per query and persisted nothing (no derived score /
    // weight / bucket written to any store/table/file by the /score path).
    assert_store_read_only(&before, &after);
}

// =============================================================================
// I-CS-1 / WD-CS-3 — no write/sign control on ANY /score response shape
// (C-INV-NoWrite). The human gate stays in the CLI.
// =============================================================================

/// C-INV-NoWrite / GOLD `no_score_response_adds_a_write_or_sign_control` (I-CS-1 /
/// WD-CS-3): NO `/score` response shape (full page or fragment, any posture) renders
/// a sign / publish / subscribe / executable-follow control — scoring is a read +
/// pure compute, and the OPTIONAL author-row "score" link (OD-CS-4) is navigation
/// TEXT, never a control. Asserted on the observable rendered surface across every
/// shape.
///
/// Given the viewer renders a rich contributor's score;
/// When every /score response shape (full page + fragment) is inspected;
/// Then none renders a sign / publish / subscribe / follow control.
///
/// @us-cs-001 @us-cs-002 @property @driving_port @real-io @read-only @i-cs-1 @gold
#[test]
fn no_score_response_adds_a_write_or_sign_control() {
    // GIVEN a store seeded with a RICH and a SPARSE contributor trail (one
    // `seed_contributor_rich_and_sparse_trails` call — see the C-INV-ReadOnly note
    // on why a single call is REQUIRED) + the viewer rendering the score in BOTH
    // shapes across EVERY posture (rich / sparse / empty).
    // WHEN each shape (get full page + get_htmx fragment) of each posture is
    // inspected.
    // THEN none carries a sign/publish/subscribe/follow affordance
    // (assert_score_html_has_no_write_or_sign_control over EVERY shape × posture;
    // I-CS-1 / WD-CS-3), AND any author-row / nav "score" link present is render-only
    // navigation TEXT — an `<a href>` anchor, never an executable write/sign control.
    // The viewer holds no key (the no-key audit is structural — xtask check-arch).
    let env = TestEnv::initialized();

    // Seed BOTH a RICH and a SPARSE contributor trail through the PRODUCTION
    // federation write path so the no-control scan runs over POPULATED score views
    // (rich → multi-row breakdown; sparse → `[SPARSE]`), not just the empty arm. A
    // SINGLE call is REQUIRED (two separate `peer pull`s would drop the first peer's
    // PDS — see the helper doc + the C-INV-ReadOnly twin).
    seed_contributor_rich_and_sparse_trails(&env, CONTRIBUTOR_RICH_DID, CONTRIBUTOR_SPARSE_DID);

    // Every /score posture — a rich trail (multi-row breakdown), a sparse trail
    // (`[SPARSE]` honesty), and an empty contributor (never seeded → guided
    // `NoClaims`). All three are LOCAL reads + pure compute; none may render a sign /
    // publish / subscribe / follow control on ANY response shape.
    let rich_path = format!("/score?contributor={CONTRIBUTOR_RICH_DID}");
    let sparse_path = format!("/score?contributor={CONTRIBUTOR_SPARSE_DID}");
    let empty_path = format!("/score?contributor={CONTRIBUTOR_EMPTY_DID}");

    // Collect EVERY /score response shape — each posture in BOTH shapes (the no-header
    // full page `get` AND the htmx fragment `get_htmx`) — inside a scope so the
    // viewer's exclusive DuckDB lock is released on drop (mirrors the slice-08
    // N-INV-NoWrite collection discipline).
    let mut responses = Vec::new();
    {
        let viewer = ViewerServer::start(&env);
        for path in [rich_path.as_str(), sparse_path.as_str(), empty_path.as_str()] {
            responses.push((format!("GET {path} (full page)"), viewer.get(path)));
            responses.push((format!("GET {path} (htmx fragment)"), viewer.get_htmx(path)));
        }
        // `viewer` drops here — the `openlore ui` process is killed.
    }

    for (label, r) in &responses {
        // Each /score route renders successfully (200) so the no-control assertion is
        // over REAL rendered content, not an error page.
        assert_eq!(
            r.status, 200,
            "/score route {label:?} over the LOCAL store must render successfully \
             (200) so the no-control scan is over REAL content; got {} body:\n{}",
            r.status, r.body
        );

        // (a) NO sign / publish / subscribe / executable-follow control on ANY shape
        // or posture (the inherited no-write harness; I-CS-1 / WD-CS-3). Any hit is an
        // UNSHIPPABLE write/sign-surface breach.
        assert_score_html_has_no_write_or_sign_control(&r.body);

        // (b) Any author-row / nav "score" link present (OD-CS-4) is render-only
        // navigation TEXT — an `<a href>` anchor — never an executable write/sign
        // control (no `<button>`/`<form>` wrapping the score link; I-CS-1). Vacuously
        // true when this posture/shape renders no score link (the OD-CS-4 author-row
        // link is OPTIONAL); load-bearing when one IS present.
        let lower = r.body.to_lowercase();
        if lower.contains("/score?contributor=") {
            assert!(
                lower.contains("<a href"),
                "I-CS-1: any author-row / nav 'score' link must be render-only \
                 navigation TEXT (an `<a href>` anchor), not an executable control; \
                 {label:?} references `/score?contributor=` with no `<a href` anchor:\
                 \n{}",
                r.body
            );
        }
    }
}

// =============================================================================
// I-CS-8 / KPI-HX-G2 — offline chrome: the /score page references only the local
// vendored htmx asset, no CDN (C-INV-OfflineChrome).
// =============================================================================

/// C-INV-OfflineChrome / GOLD `the_score_page_chrome_stays_offline_no_cdn` (I-CS-8 /
/// KPI-HX-G2): the `/score` full page references ONLY the LOCAL `/static/htmx.min.js`
/// script src and NO off-host CDN — the page CHROME stays offline-capable (and so
/// does the SCORE itself, since the read is LOCAL — unlike `/search`).
///
/// Given the viewer renders the /score full page;
/// When the page's script references are inspected;
/// Then the only htmx asset reference is the local /static/htmx.min.js — no CDN.
///
/// @us-cs-002 @property @driving_port @real-io @offline @no-cdn @i-cs-8 @gold
#[test]
fn the_score_page_chrome_stays_offline_no_cdn() {
    // GIVEN a rich-trail store + the viewer rendering the /score full page.
    // WHEN the page's script references are inspected.
    // THEN `references_external_cdn()` is FALSE (the only htmx asset is the local
    // /static/htmx.min.js; I-CS-8 / KPI-HX-G2).
    let _env = TestEnv::initialized();
    todo!(
        "slice-09 C-INV-OfflineChrome: seed_contributor_rich_trail + start; \
         get(\"/score?contributor={CONTRIBUTOR_RICH_DID}\"); assert is_full_page() + \
         !response.references_external_cdn()"
    )
}

// =============================================================================
// I-CS-5 / KPI-5 — local-first / offline: /score works with the network
// unavailable (C-INV-Offline). The score DATA is a LOCAL read, distinct from
// /search and /scrape.
// =============================================================================

/// C-INV-Offline / GOLD `the_score_surface_works_fully_offline` (I-CS-5 / KPI-5):
/// the `/score` view renders fully with NO network available — the score DATA (not
/// just the chrome) is computed over the LOCAL DuckDB store + the PURE scorer, so
/// the network being down NEVER degrades it (distinct from `/search` and `/scrape`,
/// the only network-requiring routes). Because the `/score` path has NO outbound
/// edge to take down, this gold pins that the rendered score is identical to the
/// on-network render — the LOCAL read is self-sufficient by construction.
///
/// Given the viewer is started over a rich local store with NO network seam wired
///   (no indexer URL, no GitHub base — exactly the LOCAL-only posture);
/// When the contributor is scored;
/// Then the full score + breakdown renders (no Unavailable/degraded state, no
///   network call) — the score is a LOCAL read.
///
/// @us-cs-001 @us-cs-002 @property @driving_port @real-io @offline @local-first
/// @i-cs-5 @kpi-5 @gold
#[test]
fn the_score_surface_works_fully_offline() {
    // GIVEN `ViewerServer::start(&env)` — the store-only posture with NEITHER the
    // /scrape GitHub seam NOR the /search indexer seam wired (the LOCAL-only viewer).
    // The rich trail is seeded into the LOCAL store. WHEN the contributor is scored.
    // THEN the full weight + breakdown renders (a real Scored state) with NO
    // Unavailable/degraded notice and NO network call — proving `/score` is LOCAL +
    // offline by construction (I-CS-5; distinct from the slice-08 Unavailable arm).
    let _env = TestEnv::initialized();
    todo!(
        "slice-09 C-INV-Offline: seed_contributor_rich_trail + ViewerServer::start \
         (no network seam wired); get(\"/score?contributor={CONTRIBUTOR_RICH_DID}\"); \
         assert a full Scored render (weight + breakdown) with NO Unavailable/\
         degraded state — the score is a LOCAL read, no network needed"
    )
}

// =============================================================================
// I-CS-2 / KPI-GRAPH-3 / J-002c — transparency-by-construction (C-INV-Transparency).
// THE CARDINAL GOLD: the rendered breakdown subtotals sum to the displayed weight,
// and the score is never rendered without its breakdown.
// =============================================================================

/// C-INV-Transparency / CARDINAL GOLD
/// `a_rendered_score_is_never_shown_without_its_breakdown_summing_to_the_weight`
/// (I-CS-2 / KPI-GRAPH-3 / J-002c): for the rich-trail contributor, the rendered
/// breakdown subtotals sum to the displayed weight (the reproduce-by-hand release
/// gate, the J-002c thesis) AND the score is NEVER rendered without its breakdown.
/// This is the load-bearing slice-09 invariant: an opaque-number regression
/// (showing the weight but hiding the per-claim breakdown) silently re-creates the
/// aggregator failure J-002 exists to avoid. The transparency is BY CONSTRUCTION
/// (the renderer projects the headline weight AND its `Contribution` rows from the
/// SAME `WeightedPairing`), and this gold pins it on the OBSERVABLE rendered HTML.
///
/// Given the viewer renders a rich contributor's score;
/// When the breakdown renders;
/// Then the running sum of the per-claim subtotals shown in the table equals the
///   displayed weight, AND no weight is ever shown without its per-claim breakdown.
///
/// @us-cs-002 @property @driving_port @real-io @reproduce-by-hand @anti-opaque
/// @i-cs-2 @kpi-graph-3 @gold
#[test]
fn a_rendered_score_is_never_shown_without_its_breakdown_summing_to_the_weight() {
    // GIVEN a rich-trail store + the viewer rendering the score (both shapes).
    // WHEN the breakdown renders.
    // THEN (a) the score is shown WITH its per-claim breakdown — never an opaque
    // number (assert_score_html_breakdown_attributed_and_verbatim), AND (b) the
    // rendered subtotals sum to the displayed weight (assert_score_html_breakdown_
    // sums_to_displayed_weight — the reproduce-by-hand gate). Asserted on the
    // OBSERVABLE rendered HTML across both shapes (parity by construction).
    let _env = TestEnv::initialized();
    todo!(
        "slice-09 C-INV-Transparency (CARDINAL): seed_contributor_rich_trail + \
         ViewerServer::start; for shape in [get, get_htmx]: assert the weight is \
         shown WITH its breakdown (no opaque number) AND \
         assert_score_html_breakdown_sums_to_displayed_weight(body)"
    )
}
