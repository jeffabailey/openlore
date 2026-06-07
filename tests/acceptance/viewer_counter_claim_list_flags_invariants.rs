//! Slice-12 acceptance — `/claims` counter-presence-FLAG GOLD / guardrail invariants (the
//! cross-cutting I-LF-1/2/5/6/8 guardrails that must hold over the WHOLE flagged-list
//! surface, beyond any single story).
//!
//! These are the load-bearing, release-relevant guardrail gold tests for the list-flag
//! DELTA — the BEHAVIORAL layer of the three-layer enforcement (type + xtask `check-arch`
//! are the other two, owned by DELIVER). They drive the REAL `openlore ui` verb via the
//! `ViewerServer` subprocess + in-test HTTP (with/without `HX-Request`) over a REAL seeded
//! LOCAL DuckDB, with NO mocked boundary (the batch presence read is a LOCAL DB-index
//! lookup + a PURE projection/render — OFFLINE by construction: this route has NO outbound
//! edge at all). They assert the hard slice-12 invariants on the OBSERVABLE surface:
//!
//! - `every_claims_list_render_with_flags_leaves_the_store_read_only` (LF-INV-ReadOnly,
//!   I-LF-1 / KPI-VIEW-2): exercising `/claims` across postures (countered + un-countered)
//!   AND both shapes (full page + htmx fragment) leaves `claims` + `peer_claims` row
//!   counts UNCHANGED, asserted via the universe-bound `assert_store_read_only` (Mandate 8;
//!   universe = the two port-exposed counts, all `unchanged`). The presence read is a
//!   read-only SELECT and persists nothing.
//! - `no_claims_list_render_with_flags_adds_a_write_or_sign_control` (LF-INV-NoWrite,
//!   I-LF-1): no list response shape (full page or fragment, countered or not) renders a
//!   write / sign / counter / publish control — authoring stays the slice-03 CLI; the
//!   "Countered" markers are render-only `<a href="/claims/{cid}">` navigation TEXT.
//! - `the_flagged_claims_list_page_chrome_stays_offline_no_cdn` (LF-INV-OfflineChrome,
//!   I-LF-5 / KPI-HX-G2): the flagged `/claims` full page references ONLY the LOCAL
//!   `/static/htmx.min.js` script src and NO off-host CDN.
//! - `the_flagged_claims_list_renders_fully_offline` (LF-INV-Offline, I-LF-5 / KPI-5):
//!   the flag renders fully with the network unavailable — the presence read is a LOCAL
//!   DB-index lookup (ref-tables-only, no artifact read) with NO outbound edge to take
//!   down. Peer counters were verified at `peer pull` time; the viewer re-verifies nothing.
//! - `the_list_order_and_confidence_are_byte_identical_with_and_without_flags`
//!   (LF-INV-ShownNeverApplied, the shown-never-applied / no-regression GOLD — I-LF-2 /
//!   OD-AV-7 / ADR-015): the SAME store's rendered row ORDER, PAGING, COUNT, and each
//!   row's CONFIDENCE are byte-IDENTICAL whether or not the flag feature is active — the
//!   flag never filters/re-ranks/re-paginates/re-weights the list. A regression silently
//!   lets the flag pick a triage order or re-score a claim; this gold makes it unshippable.
//!
//! Driving discipline (Mandate 1): every assertion enters through the REAL `openlore ui`
//! subprocess + HTTP — never internal `viewer-domain` `render_*` fns or
//! `counter_presence_for` directly. The local DuckDB is REAL (seeded via the production
//! `claim add` / `claim counter` / `peer add` + `peer pull` paths); there is NO mocked
//! boundary (the route is a LOCAL read).
//!
//! Layer placement (Mandate 9/11): layer-3/layer-5 subprocess + real-I/O, EXAMPLE-only.
//! These guardrails are example-based, never PBT-generated at this layer (the `@property`
//! tag marks them as universal invariants for the reader + the DELIVER crafter; the
//! generative exploration of the pure projection/render is a layer-1/2 concern in the
//! DELIVER `viewer-domain` units, out of this file's scope). The strict single-query N+1
//! bound is likewise a DELIVER `adapter-duckdb` unit/property assertion — at this layer
//! the N+1 guard is the LF-8 behavioral proxy in `viewer_counter_claim_list_flags.rs`.
//!
//! Build-before-run note: as with the story file, the run MUST `cargo build` the
//! `openlore` (viewer) bin before running these ATs. No second binary is needed — the
//! presence read is a LOCAL read.
//!
//! Mandate 7 RED scaffolds: each body is `todo!()` (via the `todo!()`-stubbed seed /
//! assert helpers or directly) → panics → classifies RED (MISSING_FUNCTIONALITY), NOT
//! BROKEN. They stay RED until DELIVER.
//!
//! Covers: the cross-cutting I-LF-1 / I-LF-2 / I-LF-5 / I-LF-6 / I-LF-8 guardrails over
//! the whole flagged-list surface (the gold companions to the US-LF-002/003 story
//! scenarios in `viewer_counter_claim_list_flags.rs`).
//
// SCAFFOLD: true

mod support;

#[allow(unused_imports)]
use support::*;

// =============================================================================
// I-LF-1 / KPI-VIEW-2 — read-only preserved: the /claims list with flags + every posture
// + shape leaves the store unchanged (LF-INV-ReadOnly). The presence read persists nothing.
// =============================================================================

/// LF-INV-ReadOnly / GOLD `every_claims_list_render_with_flags_leaves_the_store_read_only`
/// (I-LF-1 / KPI-VIEW-2): exercising `/claims` — countered AND un-countered postures, in
/// BOTH shapes (full page + htmx fragment) — leaves the `claims` + `peer_claims` row
/// counts UNCHANGED. The slice-12 companion to the slice-06/11 read-only gold tests,
/// asserted via the universe-bound state-delta (Mandate 8: universe = the two port-exposed
/// counts, each `unchanged`). The batch presence read is a read-only SELECT and persists
/// nothing.
///
/// Given a store seeded with a countered claim among Maria's own claims;
/// When the `/claims` list (countered + un-countered postures, full + fragment) is
///   exercised;
/// Then the `claims` and `peer_claims` row counts are UNCHANGED.
///
/// @us-lf-002 @us-lf-003 @property @driving_port @real-io @read-only @i-lf-1 @gold
#[test]
fn every_claims_list_render_with_flags_leaves_the_store_read_only() {
    // GIVEN a REAL store seeded so the universe is NON-TRIVIAL: a list with a countered
    // own claim (its counter via the production paths) so the read-only delta is over a
    // POPULATED store (a `0 == 0` delta would not prove the viewer leaves a populated
    // store untouched). Capture the read-only universe (port-exposed counts) BEFORE
    // exercising any route.
    // WHEN the `/claims` list is exercised — both shapes (get + get_htmx), countered +
    // un-countered postures — inside a scope so the viewer's exclusive DuckDB lock is
    // RELEASED before the `after` snapshot.
    // THEN the persisted-store row counts are UNCHANGED (assert_store_read_only; any
    // change is an UNSHIPPABLE write-surface breach — I-LF-1).
    let env = TestEnv::initialized();
    // GIVEN a populated, NON-TRIVIAL store: a list with one peer-countered own claim
    // among several plain own claims (seeded via the production `claim add` / `peer add` +
    // `peer pull` paths). The countered + un-countered postures both appear in this ONE
    // store, so exercising the list covers both postures over a POPULATED store (a
    // `0 == 0` delta would not prove the viewer leaves a populated store untouched).
    let _seeded = seed_claims_list_one_countered(&env);

    // Capture the read-only universe (the two port-exposed row counts) BEFORE any route.
    let before = capture_store_row_count_universe(&env);

    // WHEN the `/claims` list is exercised in BOTH shapes (full page + htmx fragment) —
    // inside a scope so the viewer's exclusive DuckDB lock is RELEASED before the `after`
    // snapshot is read.
    {
        let server = ViewerServer::start(&env);
        let full = server.get("/claims");
        let fragment = server.get_htmx("/claims");
        for (label, response) in [("full page", &full), ("fragment", &fragment)] {
            assert_eq!(
                response.status, 200,
                "LF-INV-ReadOnly: GET /claims ({label}) must be 200; body was:\n{}",
                response.body
            );
        }
    }

    // THEN the persisted-store row counts are UNCHANGED — any change is an UNSHIPPABLE
    // write-surface breach (I-LF-1 / KPI-VIEW-2). The presence read is a read-only SELECT
    // and persists nothing.
    let after = capture_store_row_count_universe(&env);
    assert_store_read_only(&before, &after);
}

// =============================================================================
// I-LF-1 — no write/sign/counter control on ANY list response shape (LF-INV-NoWrite).
// The human gate stays in the CLI; the "Countered" markers are render-only `<a href>`.
// =============================================================================

/// LF-INV-NoWrite / GOLD `no_claims_list_render_with_flags_adds_a_write_or_sign_control`
/// (I-LF-1): NO list response shape (full page or fragment, countered or not) renders a
/// write / sign / counter / publish control — authoring stays EXCLUSIVELY in the slice-03
/// CLI, and every "Countered" marker is render-only navigation TEXT (an
/// `<a href="/claims/{cid}">` anchor), never a control. Asserted on the observable
/// rendered surface across every shape.
///
/// Given the viewer renders a flagged list and an un-flagged list;
/// When every list response shape (full page + fragment, both postures) is inspected;
/// Then none renders a write / sign / counter / publish control, and every "Countered"
///   marker present is render-only `<a href>` navigation TEXT.
///
/// @us-lf-002 @us-lf-003 @property @driving_port @real-io @read-only @i-lf-1 @gold
#[test]
fn no_claims_list_render_with_flags_adds_a_write_or_sign_control() {
    // GIVEN two stores: one with a countered own claim (flagged list) and one all-un-
    // countered (un-flagged list), each rendered in BOTH shapes.
    // WHEN every list response shape (countered + un-countered, get + get_htmx) is
    // collected in a scope (so both viewers' DuckDB locks release on drop).
    // THEN no shape carries a write/sign/counter/publish affordance
    // (assert_detail_html_has_no_write_or_sign_control reused over each list body — the
    // viewer holds no key; the no-key audit is structural, xtask check-arch), AND every
    // `/claims/{cid}` flag-link present is a render-only `<a href>` anchor (I-LF-1).
    let countered_env = TestEnv::initialized();
    let uncountered_env = TestEnv::initialized();
    // GIVEN two stores: one with a peer-countered own claim (the FLAGGED list) and one
    // all-un-countered (the UN-flagged list), seeded via the production write paths.
    let _countered_seeded = seed_claims_list_one_countered(&countered_env);
    let _uncountered_seeded = seed_claims_list_none_countered(&uncountered_env);

    // WHEN every list response shape (countered + un-countered, get + get_htmx) is
    // collected in a scope so both viewers' DuckDB locks release on drop.
    let bodies: Vec<(&str, String)> = {
        let countered_server = ViewerServer::start(&countered_env);
        let uncountered_server = ViewerServer::start(&uncountered_env);

        let shapes = [
            ("countered full page", countered_server.get("/claims")),
            ("countered fragment", countered_server.get_htmx("/claims")),
            ("un-countered full page", uncountered_server.get("/claims")),
            ("un-countered fragment", uncountered_server.get_htmx("/claims")),
        ];

        shapes
            .into_iter()
            .map(|(label, response)| {
                assert_eq!(
                    response.status, 200,
                    "LF-INV-NoWrite: GET /claims ({label}) must be 200; body was:\n{}",
                    response.body
                );
                (label, response.body)
            })
            .collect()
    };

    // THEN no shape carries a write / sign / counter / publish / follow / subscribe
    // affordance (the viewer holds no key; authoring stays the slice-03 CLI), AND every
    // `/claims/{cid}` reference is a render-only `<a href>` anchor (navigation TEXT, never
    // an executable control; I-LF-1).
    for (label, body) in &bodies {
        // No write/sign/counter/publish control on any list shape.
        assert_detail_html_has_no_write_or_sign_control(body);

        // Every `/claims/` reference is preceded by `<a href` — a render-only navigation
        // anchor, never a form action or other control. We scan each occurrence and
        // require that the nearest preceding `<a href` opens the anchor that carries it.
        for (idx, _) in body.match_indices("/claims/") {
            let prefix = &body[..idx];
            let anchor_open = prefix.rfind("<a href");
            let tag_open = prefix.rfind('<');
            assert!(
                anchor_open.is_some() && anchor_open == tag_open,
                "I-LF-1 ({label}): every `/claims/` reference must be a render-only \
                 `<a href>` navigation anchor (never a write/control); the reference at \
                 byte {idx} is not inside an `<a href` tag; body was:\n{body}"
            );
        }
    }
}

// =============================================================================
// I-LF-5 / KPI-HX-G2 — offline chrome: the flagged /claims page references only the local
// vendored htmx asset, no CDN (LF-INV-OfflineChrome).
// =============================================================================

/// LF-INV-OfflineChrome / GOLD `the_flagged_claims_list_page_chrome_stays_offline_no_cdn`
/// (I-LF-5 / KPI-HX-G2): the flagged `/claims` full page references ONLY the LOCAL
/// `/static/htmx.min.js` script src and NO off-host CDN — the page CHROME stays
/// offline-capable (and so does the FLAG itself, since the presence read is LOCAL).
///
/// Given the viewer renders the flagged `/claims` full page;
/// When the page's script references are inspected;
/// Then the only htmx asset reference is the local /static/htmx.min.js — no CDN.
///
/// @us-lf-002 @property @driving_port @real-io @offline @no-cdn @i-lf-5 @gold
#[test]
fn the_flagged_claims_list_page_chrome_stays_offline_no_cdn() {
    // GIVEN a flagged list (a countered own claim) + the viewer rendering its `/claims`
    // full page.
    // WHEN the page's script references are inspected (both shapes).
    // THEN `references_external_cdn()` is FALSE for both (the only htmx asset is the local
    // /static/htmx.min.js; I-LF-5 / KPI-HX-G2). NO network seam is wired (plain
    // `ViewerServer::start`): the presence read is LOCAL, so the page CHROME and the FLAG
    // are both offline-capable.
    let env = TestEnv::initialized();
    // GIVEN a flagged list: one peer-countered own claim among several plain own claims,
    // seeded via the production `claim add` / `peer add` + `peer pull` paths.
    let _seeded = seed_claims_list_one_countered(&env);

    // WHEN the flagged `/claims` list is rendered in BOTH shapes — under the plain,
    // store-only `ViewerServer::start` (NO /scrape GitHub seam, NO /search indexer seam):
    // the presence read is a LOCAL DB-index lookup, so there is no outbound edge to wire.
    // The scope releases the viewer's exclusive DuckDB lock on drop.
    let server = ViewerServer::start(&env);
    let full = server.get("/claims");
    let fragment = server.get_htmx("/claims");

    for (label, response) in [("full page", &full), ("fragment", &fragment)] {
        assert_eq!(
            response.status, 200,
            "LF-INV-OfflineChrome: GET /claims ({label}) must be 200; body was:\n{}",
            response.body
        );
    }

    // The full page carries chrome; the htmx fragment is the bare swap target.
    assert!(
        full.is_full_page(),
        "LF-INV-OfflineChrome: the no-HX /claims response must be a full page; body was:\n{}",
        full.body
    );
    assert!(
        fragment.is_fragment(),
        "LF-INV-OfflineChrome: the HX-Request /claims response must be a bare fragment; \
         body was:\n{}",
        fragment.body
    );

    // THEN neither shape references an off-host CDN — the only htmx asset is the LOCAL
    // /static/htmx.min.js, so the page CHROME (and, since the flag read is LOCAL, the FLAG
    // itself) stays offline-capable (I-LF-5 / KPI-HX-G2).
    assert!(
        !full.references_external_cdn(),
        "LF-INV-OfflineChrome: the flagged /claims full page must reference NO off-host CDN \
         (only the local /static/htmx.min.js); body was:\n{}",
        full.body
    );
    assert!(
        !fragment.references_external_cdn(),
        "LF-INV-OfflineChrome: the flagged /claims fragment must reference NO off-host CDN \
         (only the local /static/htmx.min.js); body was:\n{}",
        fragment.body
    );
}

// =============================================================================
// I-LF-5 / KPI-5 — local-first / offline: the flag renders with the network unavailable
// (LF-INV-Offline). The presence read is a LOCAL DB-index lookup with NO outbound edge.
// =============================================================================

/// LF-INV-Offline / GOLD `the_flagged_claims_list_renders_fully_offline` (I-LF-5 /
/// KPI-5): the flagged `/claims` list renders fully with NO network available — the
/// presence read (the INDEXED `referenced_cid IN (...)` ref lookup) is LOCAL, with NO
/// per-row artifact read and NO outbound edge, so the network being down NEVER degrades
/// it. The countered row (countered by a PULLED PEER, already verified at `peer pull`
/// time) STILL carries its marker; the viewer re-verifies nothing.
///
/// Given the viewer is started over a seeded store with NO network seam wired, and the
///   network is disabled;
/// When the `/claims` list is opened (a row countered by an already-pulled peer);
/// Then the countered row STILL carries the "Countered" marker, with no Unavailable /
///   degraded state and no network call.
///
/// @us-lf-002 @property @driving_port @real-io @offline @local-first @i-lf-5 @kpi-5 @gold
#[test]
fn the_flagged_claims_list_renders_fully_offline() {
    // GIVEN `ViewerServer::start(&env)` — the store-only posture with NEITHER the /scrape
    // GitHub seam NOR the /search indexer seam wired (the LOCAL-only viewer) — over a
    // store seeded with a list whose countered row was countered by a PULLED PEER record
    // (verified at pull time). WHEN the `/claims` list is opened in both shapes. THEN the
    // countered row STILL carries the "Countered" marker with NO Unavailable/degraded
    // notice and NO network call — proving the presence read is LOCAL + offline by
    // construction (I-LF-5; the viewer re-verifies nothing).
    let env = TestEnv::initialized();
    // GIVEN a store seeded with a list whose countered row was countered by a PULLED PEER
    // record (verified at `peer pull` time), via the production federation paths.
    let seeded = seed_claims_list_one_countered(&env);
    let countered_cid = seeded
        .countered_cids
        .first()
        .expect("LF-INV-Offline: seed must produce exactly one countered row")
        .clone();

    // WHEN the `/claims` list is opened in BOTH shapes under the plain, store-only
    // `ViewerServer::start` — NEITHER the /scrape GitHub seam NOR the /search indexer seam
    // is wired, so the LOCAL-only viewer has NO outbound edge: the presence read is a LOCAL
    // DB-index lookup, OFFLINE by construction. The scope releases the DuckDB lock on drop.
    let (full_body, fragment_body) = {
        let server = ViewerServer::start(&env);
        let full = server.get("/claims");
        let fragment = server.get_htmx("/claims");
        for (label, response) in [("full page", &full), ("fragment", &fragment)] {
            assert_eq!(
                response.status, 200,
                "LF-INV-Offline: GET /claims ({label}) must be 200; body was:\n{}",
                response.body
            );
        }
        (full.body, fragment.body)
    };

    // THEN the countered row STILL carries its render-only "Countered" marker in BOTH
    // shapes — the peer counter was verified at pull time; the viewer re-verifies nothing,
    // makes no network call, and never degrades (I-LF-5 / KPI-5).
    for (label, body) in [("full page", &full_body), ("fragment", &fragment_body)] {
        assert_list_row_flagged_countered(body, &countered_cid);
        let lower = body.to_lowercase();
        for notice in ["unavailable", "network error", "could not reach", "try again"] {
            assert!(
                !lower.contains(notice),
                "LF-INV-Offline: the offline-rendered /claims list ({label}) must show NO \
                 degraded notice ({notice:?}) — the presence read is LOCAL, no outbound \
                 edge to take down; body was:\n{body}"
            );
        }
    }
}

// =============================================================================
// I-LF-2 / OD-AV-7 / ADR-015 — shown-never-applied / no-regression: the list order, paging,
// count, and each row's confidence are byte-identical with and without the flag
// (LF-INV-ShownNeverApplied). THE GOLD.
// =============================================================================

/// LF-INV-ShownNeverApplied / SHOWN-NEVER-APPLIED + NO-REGRESSION GOLD
/// `the_list_order_and_confidence_are_byte_identical_with_and_without_flags` (I-LF-2 /
/// OD-AV-7 / ADR-015): the SAME store's rendered row ORDER, PAGING / position indicator,
/// total COUNT, and EVERY row's CONFIDENCE are byte-IDENTICAL whether or not the flag is
/// active — the flag never filters, re-ranks, re-paginates, down-weights, or re-orders the
/// list. This is the load-bearing slice-12 invariant: the flag is additive context BESIDE
/// each row and changes NOTHING about which rows appear, where, or with what confidence. A
/// shown-never-applied / no-regression breach silently lets the flag pick a triage order
/// or re-score a claim; this gold makes it unshippable. Asserted on the OBSERVABLE
/// rendered HTML across the flagged and un-flagged (slice-06 baseline) renders of the SAME
/// store.
///
/// Given the SAME store (a known order + a mix of countered + un-countered claims) is
///   rendered once un-flagged (slice-06 baseline) and once flagged;
/// When both `/claims` lists render;
/// Then the row order, the position indicator / page boundaries, the total count, and
///   every row's confidence are byte-identical in both — the flag changed nothing but the
///   additive marker.
///
/// @us-lf-003 @property @driving_port @real-io @shown-never-applied @no-regression @i-lf-2
/// @i-lf-4 @gold
#[test]
fn the_list_order_and_confidence_are_byte_identical_with_and_without_flags() {
    // GIVEN a store with a known order + a mix of countered + un-countered claims, rendered
    // as the slice-06 baseline list (no flag) AND as the slice-12 flagged list — the SAME
    // store, so order/paging/count/confidence are directly comparable.
    // WHEN both `/claims` lists render (both shapes).
    // THEN the row ORDER (composed_at DESC, cid), the position indicator / page boundaries,
    // the total COUNT, and EVERY row's verbatim confidence are byte-IDENTICAL between the
    // flagged and un-flagged renders — the flag is additive only
    // (assert_list_order_and_confidence_byte_identical; I-LF-2 / I-LF-4). Any divergence is
    // an UNSHIPPABLE no-regression breach.
    let _env = TestEnv::initialized();
    todo!(
        "LF-INV-ShownNeverApplied: seed_claims_list_mixed_pages(&env) (known order + \
         countered/un-countered mix); {{ ViewerServer::start; flagged_full = \
         get(\"/claims\"); flagged_fragment = get_htmx(\"/claims\") }}; (baseline = the \
         slice-06 reference render of the SAME store — DELIVER pins this via a no-flag \
         render path or the recorded slice-06 ordering); \
         assert_list_order_and_confidence_byte_identical for BOTH shapes (row order, \
         position indicator, total count, each row's confidence byte-identical; only the \
         additive marker differs). RED until DELIVER."
    )
}
