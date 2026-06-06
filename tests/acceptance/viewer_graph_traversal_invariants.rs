//! Slice-10 acceptance — graph-traversal GOLD / guardrail invariants (the
//! cross-cutting I-GT-1/2/3/4/7 guardrails that must hold over the WHOLE `/project` +
//! `/philosophy` traversal surface, beyond any single story).
//!
//! These are the load-bearing, release-relevant guardrail gold tests for the
//! graph-traversal DELTA — the BEHAVIORAL layer of the three-layer enforcement (type
//! + xtask `check-arch` are the other two, owned by DELIVER). They drive the REAL
//! `openlore ui` verb via the `ViewerServer` subprocess + in-test HTTP (with/without
//! `HX-Request`) over a REAL seeded LOCAL DuckDB, with NO mocked boundary (both
//! traversal routes are a LOCAL read + PURE group/render — distinct from `/search`,
//! which mocks the indexer; and OFFLINE-STRONGER than `/search` — these routes have
//! NO outbound edge at all). They assert the hard slice-10 invariants on the
//! OBSERVABLE surface:
//!
//! - `every_traversal_route_leaves_the_store_read_only` (GT-INV-ReadOnly, I-GT-1 /
//!   WD-GT-3 / KPI-VIEW-2): exercising EVERY traversal route — `/project` +
//!   `/philosophy`, across postures (populated / claim-less) AND both shapes (full
//!   page + htmx fragment) — leaves `claims` + `peer_claims` row counts UNCHANGED,
//!   asserted via the universe-bound `assert_store_read_only` (Mandate 8; universe =
//!   the two port-exposed counts, all `unchanged`). Surveys are computed per query
//!   and persist nothing (I-GT-8 — zero new persisted types).
//! - `no_traversal_response_adds_a_write_or_sign_control` (GT-INV-NoWrite, I-GT-1 /
//!   WD-GT-3): no traversal response shape (full page or fragment, either route)
//!   renders a sign / publish / subscribe / executable-follow control — traversal is
//!   a read; the cross-links are render-only `<a href>` navigation TEXT.
//! - `the_traversal_page_chrome_stays_offline_no_cdn` (GT-INV-OfflineChrome, I-GT-7 /
//!   KPI-HX-G2): the `/project` + `/philosophy` full pages reference ONLY the LOCAL
//!   `/static/htmx.min.js` script src and NO off-host CDN.
//! - `the_traversal_surface_works_fully_offline` (GT-INV-Offline, I-GT-2 / KPI-5):
//!   both traversal views render fully with the network unavailable — the survey DATA
//!   (not just the chrome) is a LOCAL read with NO outbound edge to take down
//!   (distinct from `/search` and `/scrape`; offline-STRONGER, I-GT-2 / I-GT-7).
//! - `no_traversal_href_lets_a_claim_controlled_uri_inject` (GT-INV-Security, the
//!   ADR-044 §security gold — I-GT-3/4 hostile-input boundary): a hostile peer-
//!   authored subject (carrying `"`/`<`/`&`/space) renders its cross-link href
//!   PERCENT-ENCODED across both shapes — it can never break out of the `href`
//!   attribute or smuggle a second query param. An injection regression silently
//!   re-opens a markup/param-smuggling vector on an attacker-influenced surface; this
//!   gold makes it unshippable.
//!
//! Driving discipline (Mandate 1): every assertion enters through the REAL `openlore
//! ui` subprocess + HTTP — never internal `viewer-domain` `render_*` / `group_*` fns.
//! The local DuckDB is REAL (seeded via the production `peer add` + `peer pull` /
//! `claim add` path); there is NO mocked boundary (both routes are LOCAL).
//!
//! Layer placement (Mandate 9/11): layer-3/layer-5 subprocess + real-I/O,
//! EXAMPLE-only. These guardrails are example-based, never PBT-generated at this
//! layer (the `@property` tag marks them as universal invariants for the reader + the
//! DELIVER crafter; the generative exploration of the pure group/render core + the
//! `encode_query_component` round-trip PROPERTY is a layer-1/2 concern in the DELIVER
//! `viewer-domain` units, out of this file's scope).
//!
//! Build-before-run note: as with `viewer_graph_traversal.rs`, the run MUST `cargo
//! build` the `openlore` (viewer) bin before running these ATs. No second binary is
//! needed — both traversal routes are a LOCAL read.
//!
//! Mandate 7 RED scaffolds: each body is `todo!()` (via the `todo!()`-stubbed
//! `assert_traversal_*` helpers or directly) → panics → classifies RED
//! (MISSING_FUNCTIONALITY), NOT BROKEN. They stay RED until DELIVER.
//!
//! Covers: the cross-cutting I-GT-1 / I-GT-2 / I-GT-3 / I-GT-4 / I-GT-7 guardrails
//! over the whole traversal surface (the gold companions to the US-GT-002/003/004
//! story scenarios in `viewer_graph_traversal.rs`).
//
// SCAFFOLD: true

mod support;

#[allow(unused_imports)]
use support::*;

// =============================================================================
// I-GT-1 / WD-GT-3 — read-only preserved: every traversal route + posture + shape
// leaves the store unchanged (GT-INV-ReadOnly). Surveys are computed per query and
// persist nothing.
// =============================================================================

/// GT-INV-ReadOnly / GOLD `every_traversal_route_leaves_the_store_read_only` (I-GT-1 /
/// WD-GT-3 / KPI-VIEW-2): exercising EVERY traversal route — `/project` +
/// `/philosophy`, populated AND claim-less, in BOTH shapes (full page + htmx
/// fragment) — leaves the `claims` + `peer_claims` row counts UNCHANGED. The traversal
/// companion to the slice-06 `viewer_is_read_only` + slice-08/09 read-only gold tests,
/// asserted via the universe-bound state-delta (Mandate 8: universe = the two
/// port-exposed counts, each `unchanged`). Surveys are recomputed per query and NEVER
/// persisted (I-GT-8 — zero new persisted types).
///
/// Given a store seeded with a project trail and a philosophy trail;
/// When EVERY traversal route (/project + /philosophy, populated/claim-less, full +
///   fragment) is exercised;
/// Then the `claims` and `peer_claims` row counts are UNCHANGED.
///
/// @us-gt-002 @us-gt-003 @us-gt-004 @property @driving_port @real-io @read-only
/// @i-gt-1 @gold
#[test]
fn every_traversal_route_leaves_the_store_read_only() {
    // GIVEN a REAL store seeded (production federation path) with BOTH a project trail
    // AND a philosophy trail so the read-only delta is over a non-trivial universe (a
    // `0 == 0` delta would not prove the viewer leaves a POPULATED store untouched).
    // Capture the read-only universe (port-exposed counts: `claims.row_count`,
    // `peer_claims.row_count`) BEFORE exercising any traversal route.
    // WHEN every traversal route is exercised — both routes, populated + claim-less,
    // both shapes (get + get_htmx) — within a scope so the viewer's exclusive DuckDB
    // lock is RELEASED before the `after` snapshot (the read-only proof is about what
    // the viewer LEFT BEHIND, mirroring the slice-06/08/09 gold tests).
    // THEN the persisted-store row counts are UNCHANGED (assert_store_read_only; any
    // change is an UNSHIPPABLE write-surface breach — I-GT-1 / WD-GT-3).
    let env = TestEnv::initialized();

    // Seed BOTH a project trail (cargo) and a philosophy trail (reproducible-builds)
    // through the PRODUCTION federation write path so the read-only universe is
    // NON-TRIVIAL.
    seed_project_survey_trail(&env, TRAVERSAL_PROJECT_CARGO, TRAVERSAL_AUTHOR_RACHEL);
    seed_philosophy_survey_trail(
        &env,
        TRAVERSAL_PHILOSOPHY_REPRO_BUILDS,
        TRAVERSAL_AUTHOR_RACHEL,
    );

    // Capture the read-only universe BEFORE any traversal route runs (Mandate 8: the
    // universe is the inherited capture, NOT internal struct fields).
    let before = capture_store_row_count_universe(&env);

    // Every traversal posture: populated `/project` + `/philosophy`, AND a claim-less
    // `/project` + `/philosophy` (the guided NoClaims arm). All are LOCAL reads + pure
    // grouping — none may persist any survey/group/edge (I-GT-8 — zero new persisted
    // types).
    let project_path = format!("/project?subject={TRAVERSAL_PROJECT_CARGO}");
    let philosophy_path =
        format!("/philosophy?object={TRAVERSAL_PHILOSOPHY_REPRO_BUILDS}");
    let empty_project_path = format!("/project?subject={TRAVERSAL_PROJECT_UNKNOWN}");
    let empty_philosophy_path =
        format!("/philosophy?object={TRAVERSAL_PHILOSOPHY_UNKNOWN}");

    // Exercise EVERY traversal route inside a scope so the viewer's exclusive DuckDB
    // lock is RELEASED (on drop) BEFORE the `after` snapshot re-opens the store.
    {
        let viewer = ViewerServer::start(&env);

        for path in [
            project_path.as_str(),
            philosophy_path.as_str(),
            empty_project_path.as_str(),
            empty_philosophy_path.as_str(),
        ] {
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
    // (any change is an UNSHIPPABLE write-surface breach; I-GT-1 / WD-GT-3). The
    // surveys were recomputed per query and persisted nothing.
    assert_store_read_only(&before, &after);
}

// =============================================================================
// I-GT-1 / WD-GT-3 — no write/sign control on ANY traversal response shape
// (GT-INV-NoWrite). The human gate stays in the CLI; cross-links are render-only.
// =============================================================================

/// GT-INV-NoWrite / GOLD `no_traversal_response_adds_a_write_or_sign_control` (I-GT-1
/// / WD-GT-3): NO traversal response shape (full page or fragment, either route)
/// renders a sign / publish / subscribe / executable-follow control — traversal is a
/// read, and every cross-link (subject/object/contributor) is render-only navigation
/// TEXT (an `<a href>` anchor), never a control. Asserted on the observable rendered
/// surface across every shape.
///
/// Given the viewer renders a project survey and a philosophy survey;
/// When every traversal response shape (full page + fragment, both routes) is
///   inspected;
/// Then none renders a sign / publish / subscribe / follow control, and any
///   `/score`/`/project`/`/philosophy` cross-link present is render-only `<a href>`
///   navigation TEXT.
///
/// @us-gt-002 @us-gt-003 @us-gt-004 @property @driving_port @real-io @read-only
/// @i-gt-1 @gold
#[test]
fn no_traversal_response_adds_a_write_or_sign_control() {
    // GIVEN a store seeded with a project trail AND a philosophy trail + the viewer
    // rendering both surveys in BOTH shapes.
    // WHEN each shape (get full page + get_htmx fragment) of each route is inspected.
    // THEN none carries a sign/publish/subscribe/follow affordance
    // (assert_traversal_html_has_no_write_or_sign_control over EVERY shape × route;
    // I-GT-1 / WD-GT-3), AND any cross-link present is render-only navigation TEXT —
    // an `<a href>` anchor, never an executable write/sign control. The viewer holds
    // no key (the no-key audit is structural — xtask check-arch).
    let env = TestEnv::initialized();

    seed_project_survey_trail(&env, TRAVERSAL_PROJECT_CARGO, TRAVERSAL_AUTHOR_RACHEL);
    seed_philosophy_survey_trail(
        &env,
        TRAVERSAL_PHILOSOPHY_REPRO_BUILDS,
        TRAVERSAL_AUTHOR_RACHEL,
    );

    let project_path = format!("/project?subject={TRAVERSAL_PROJECT_CARGO}");
    let philosophy_path =
        format!("/philosophy?object={TRAVERSAL_PHILOSOPHY_REPRO_BUILDS}");

    // Collect EVERY traversal response shape — each route in BOTH shapes — inside a
    // scope so the viewer's exclusive DuckDB lock is released on drop (mirrors the
    // slice-08/09 no-write collection discipline).
    let mut responses = Vec::new();
    {
        let viewer = ViewerServer::start(&env);
        for path in [project_path.as_str(), philosophy_path.as_str()] {
            responses.push((format!("GET {path} (full page)"), viewer.get(path)));
            responses.push((format!("GET {path} (htmx fragment)"), viewer.get_htmx(path)));
        }
        // `viewer` drops here — the `openlore ui` process is killed.
    }

    for (label, r) in &responses {
        // Each traversal route renders successfully (200) so the no-control assertion
        // is over REAL rendered content, not an error page.
        assert_eq!(
            r.status, 200,
            "traversal route {label:?} over the LOCAL store must render successfully \
             (200) so the no-control scan is over REAL content; got {} body:\n{}",
            r.status, r.body
        );

        // (a) NO sign / publish / subscribe / executable-follow control on ANY shape
        // or route (the inherited no-write harness; I-GT-1 / WD-GT-3). Any hit is an
        // UNSHIPPABLE write/sign-surface breach.
        assert_traversal_html_has_no_write_or_sign_control(&r.body);

        // (b) Any traversal cross-link present (subject→/project, object→/philosophy,
        // contributor→/score) is render-only navigation TEXT — an `<a href>` anchor —
        // never an executable write/sign control (no `<button>`/`<form>` wrapping the
        // link; I-GT-1 / WD-GT-3). Vacuously true when this shape renders no
        // cross-link; load-bearing when one IS present.
        let lower = r.body.to_lowercase();
        for crosslink in ["/score?contributor=", "/project?subject=", "/philosophy?object="] {
            if lower.contains(crosslink) {
                assert!(
                    lower.contains("<a href"),
                    "I-GT-1 / WD-GT-3: any traversal cross-link must be render-only \
                     navigation TEXT (an `<a href>` anchor), not an executable \
                     control; {label:?} references {crosslink:?} with no `<a href` \
                     anchor:\n{}",
                    r.body
                );
            }
        }
    }
}

// =============================================================================
// I-GT-7 / KPI-HX-G2 — offline chrome: the traversal pages reference only the local
// vendored htmx asset, no CDN (GT-INV-OfflineChrome).
// =============================================================================

/// GT-INV-OfflineChrome / GOLD `the_traversal_page_chrome_stays_offline_no_cdn`
/// (I-GT-7 / KPI-HX-G2): the `/project` + `/philosophy` full pages reference ONLY the
/// LOCAL `/static/htmx.min.js` script src and NO off-host CDN — the page CHROME stays
/// offline-capable (and so does the SURVEY itself, since the read is LOCAL — even
/// stronger than `/search`).
///
/// Given the viewer renders the /project + /philosophy full pages;
/// When each page's script references are inspected;
/// Then the only htmx asset reference is the local /static/htmx.min.js — no CDN.
///
/// @us-gt-002 @us-gt-003 @property @driving_port @real-io @offline @no-cdn @i-gt-7
/// @gold
#[test]
fn the_traversal_page_chrome_stays_offline_no_cdn() {
    // GIVEN a project + philosophy trail + the viewer rendering both full pages.
    // WHEN each page's script references are inspected.
    // THEN `references_external_cdn()` is FALSE for both (the only htmx asset is the
    // local /static/htmx.min.js; I-GT-7 / KPI-HX-G2). NO network seam is wired (plain
    // `ViewerServer::start`): both traversal routes are LOCAL reads, so the page
    // CHROME and the SURVEY itself are both offline-capable.
    let env = TestEnv::initialized();

    seed_project_survey_trail(&env, TRAVERSAL_PROJECT_CARGO, TRAVERSAL_AUTHOR_RACHEL);
    seed_philosophy_survey_trail(
        &env,
        TRAVERSAL_PHILOSOPHY_REPRO_BUILDS,
        TRAVERSAL_AUTHOR_RACHEL,
    );
    let viewer = ViewerServer::start(&env);

    let project_path = format!("/project?subject={TRAVERSAL_PROJECT_CARGO}");
    let philosophy_path =
        format!("/philosophy?object={TRAVERSAL_PHILOSOPHY_REPRO_BUILDS}");

    for path in [project_path.as_str(), philosophy_path.as_str()] {
        // The no-header full page carries the chrome (the htmx `<script src>`). The
        // HX-Request fragment is the bare `#traversal-results` region — neither shape
        // may reference an off-host CDN (the only htmx asset is the LOCAL
        // /static/htmx.min.js; I-GT-7 / KPI-HX-G2). Asserted over BOTH shapes.
        let full_page = viewer.get(path);
        let fragment = viewer.get_htmx(path);

        assert_eq!(
            full_page.status, 200,
            "GT-INV-OfflineChrome: GET {path:?} (full page) must render successfully \
             (200) so the no-CDN scan is over REAL chrome; body was:\n{}",
            full_page.body
        );
        assert_eq!(
            fragment.status, 200,
            "GT-INV-OfflineChrome: GET {path:?} (htmx fragment) must render \
             successfully (200); body was:\n{}",
            fragment.body
        );

        assert!(
            full_page.is_full_page(),
            "GT-INV-OfflineChrome: the no-JS {path:?} response must be a complete full \
             page (chrome present — it is the surface that loads the htmx asset); body \
             was:\n{}",
            full_page.body
        );
        assert!(
            fragment.is_fragment(),
            "GT-INV-OfflineChrome: the HX-Request {path:?} response must be a bare \
             fragment (no chrome); body was:\n{}",
            fragment.body
        );

        // The hard invariant (I-GT-7 / KPI-HX-G2): NO traversal shape references an
        // off-host CDN for the htmx library — the only htmx asset is the LOCAL
        // /static/htmx.min.js the viewer serves itself. Any CDN host hit is an
        // UNSHIPPABLE offline-guarantee breach.
        assert!(
            !full_page.references_external_cdn(),
            "I-GT-7: the {path:?} full page must reference ONLY the local \
             /static/htmx.min.js — no off-host CDN; body was:\n{}",
            full_page.body
        );
        assert!(
            !fragment.references_external_cdn(),
            "I-GT-7: the {path:?} htmx fragment must reference ONLY the local \
             /static/htmx.min.js — no off-host CDN; body was:\n{}",
            fragment.body
        );
    }
}

// =============================================================================
// I-GT-2 / KPI-5 — local-first / offline: the traversal surface works with the
// network unavailable (GT-INV-Offline). The survey DATA is a LOCAL read with NO
// outbound edge — offline-STRONGER than /search.
// =============================================================================

/// GT-INV-Offline / GOLD `the_traversal_surface_works_fully_offline` (I-GT-2 / KPI-5):
/// the `/project` + `/philosophy` views render fully with NO network available — the
/// survey DATA (not just the chrome) is computed over the LOCAL DuckDB store + the
/// PURE grouper, so the network being down NEVER degrades it (distinct from `/search`
/// and `/scrape`, the only network-requiring routes). Because the traversal path has
/// NO outbound edge to take down, this gold pins that the rendered survey is identical
/// to an on-network render — the LOCAL read is self-sufficient by construction
/// (offline-STRONGER than `/search`, I-GT-2 / I-GT-7).
///
/// Given the viewer is started over a seeded store with NO network seam wired (no
///   indexer URL, no GitHub base — exactly the LOCAL-only posture);
/// When the project + philosophy are surveyed;
/// Then the full attributed surveys render (no Unavailable/degraded state, no network
///   call) — the surveys are a LOCAL read.
///
/// @us-gt-002 @us-gt-003 @property @driving_port @real-io @offline @local-first
/// @i-gt-2 @kpi-5 @gold
#[test]
fn the_traversal_surface_works_fully_offline() {
    // GIVEN `ViewerServer::start(&env)` — the store-only posture with NEITHER the
    // /scrape GitHub seam NOR the /search indexer seam wired (the LOCAL-only viewer).
    // A project + philosophy trail is seeded into the LOCAL store. WHEN each is
    // surveyed. THEN the full attributed survey renders (a real Found state) with NO
    // Unavailable/degraded notice and NO network call — proving traversal is LOCAL +
    // offline by construction (I-GT-2; distinct from the slice-08 Unavailable arm).
    let env = TestEnv::initialized();

    seed_project_survey_trail(&env, TRAVERSAL_PROJECT_CARGO, TRAVERSAL_AUTHOR_RACHEL);
    seed_philosophy_survey_trail(
        &env,
        TRAVERSAL_PHILOSOPHY_REPRO_BUILDS,
        TRAVERSAL_AUTHOR_RACHEL,
    );

    // The plain `ViewerServer::start` is THE proof of "no network seam wired": it
    // calls `start_inner(env, None, None, None)` — NO `_github` (/scrape) AND NO
    // `_indexer` (/search) handle, and NEITHER `OPENLORE_GITHUB_API_BASE` NOR
    // `OPENLORE_INDEXER_URL` is exported to the viewer process. With no outbound edge
    // to take down, a full Found render here proves traversal is LOCAL + offline by
    // construction (I-GT-2 / KPI-5).
    let viewer = ViewerServer::start(&env);

    let project_path = format!("/project?subject={TRAVERSAL_PROJECT_CARGO}");
    let philosophy_path =
        format!("/philosophy?object={TRAVERSAL_PHILOSOPHY_REPRO_BUILDS}");

    for (route_label, path) in [("project", &project_path), ("philosophy", &philosophy_path)] {
        // Both shapes — the no-header full page (`get`) AND the htmx fragment
        // (`get_htmx`) — render the survey over the LOCAL store with no network.
        let full_page = viewer.get(path);
        let fragment = viewer.get_htmx(path);

        assert_eq!(
            full_page.status, 200,
            "GT-INV-Offline ({route_label}): GET {path:?} (full page) must render a \
             calm 200 over the LOCAL store with no network wired; body was:\n{}",
            full_page.body
        );
        assert_eq!(
            fragment.status, 200,
            "GT-INV-Offline ({route_label}): GET {path:?} (htmx fragment) must render \
             a calm 200 over the LOCAL store with no network wired; body was:\n{}",
            fragment.body
        );

        for (shape, body) in [("full page", &full_page.body), ("fragment", &fragment.body)] {
            // The `#traversal-results` region rendered (a REAL Found state), not a
            // blank / error surface — the survey is a LOCAL read, self-sufficient with
            // no network.
            assert!(
                body.contains(TRAVERSAL_RESULTS_ID),
                "GT-INV-Offline ({route_label} / {shape}): the offline traversal \
                 response must carry the `#traversal-results` region (a REAL survey \
                 render, not a blank/error page); body was:\n{body}"
            );
            // NO Unavailable / degraded notice — traversal has no outbound edge to
            // fail, so it NEVER renders the slice-08 `/search` Unavailable arm.
            let lowered = body.to_ascii_lowercase();
            for banned in ["unavailable", "network error", "could not reach", "try again"] {
                assert!(
                    !lowered.contains(banned),
                    "GT-INV-Offline ({route_label} / {shape}): the offline traversal \
                     render must NOT show a network-degraded notice ({banned:?}) — \
                     traversal has no outbound edge (I-GT-2); body was:\n{body}"
                );
            }
        }
    }
}

// =============================================================================
// I-GT-3/4 / ADR-044 §security — the injection boundary: a claim-controlled URI can
// never inject the href (GT-INV-Security). THE SECURITY GOLD.
// =============================================================================

/// GT-INV-Security / SECURITY GOLD `no_traversal_href_lets_a_claim_controlled_uri_
/// inject` (ADR-044 §security — the hostile-input boundary): a hostile subject a PEER
/// authored into a signed claim (carrying `"`/`<`/`>`/`&`/space) renders its
/// `/project` cross-link href PERCENT-ENCODED across BOTH shapes (full page +
/// fragment) — it can NEVER break out of the `href` attribute or smuggle a second
/// query param. This is the load-bearing slice-10 security invariant: subject/object
/// are attacker-influenced strings (a peer may author any URI), so the href is the
/// defense-in-depth injection boundary (over maud's auto-escape). An injection
/// regression (an un-encoded hostile URI in a cross-link) silently re-opens a
/// markup/param-smuggling vector; this gold makes it unshippable. Asserted on the
/// OBSERVABLE rendered HTML across both shapes.
///
/// Given a peer's signed claim whose subject is github:evil/x"><script>&q= space
///   embodies dependency-pinning;
/// When the philosophy survey that lists that hostile subject as a /project cross-link
///   renders (full page + fragment);
/// Then in BOTH shapes the rendered href percent-encodes the hostile subject and never
///   breaks out of the href attribute or injects markup.
///
/// @us-gt-004 @property @driving_port @real-io @security @injection-boundary @adr-044
/// @i-gt-3 @i-gt-4 @gold
#[test]
fn no_traversal_href_lets_a_claim_controlled_uri_inject() {
    // GIVEN a store seeded with a PEER claim whose SUBJECT is the hostile
    // TRAVERSAL_INJECTION_SUBJECT on dependency-pinning (the attacker-influenced
    // input). WHEN the `/philosophy?object=dependency-pinning` survey (which lists that
    // hostile subject as a /project cross-link) renders in BOTH shapes. THEN in EACH
    // shape the rendered href PERCENT-ENCODES the hostile subject
    // (TRAVERSAL_INJECTION_SUBJECT_ENCODED) and does NOT let the raw `"><script>` /
    // un-encoded `&`/space break out of the href attribute (ADR-044 §security). The
    // injection boundary holds in BOTH shapes (parity by construction — the page
    // EMBEDS the fragment).
    let env = TestEnv::initialized();
    let object = seed_injection_uri_subject(&env);
    let viewer = ViewerServer::start(&env);

    let path = format!("/philosophy?object={object}");
    let full_page = viewer.get(&path);
    let fragment = viewer.get_htmx(&path);

    assert_eq!(
        full_page.status, 200,
        "GT-INV-Security: GET {path:?} (full page) for the injection-subject survey \
         must return 200; body was:\n{}",
        full_page.body
    );
    assert_eq!(
        fragment.status, 200,
        "GT-INV-Security: GET {path:?} (htmx fragment) for the injection-subject \
         survey must return 200; body was:\n{}",
        fragment.body
    );

    // The injection boundary holds in BOTH shapes: the hostile subject's `/project`
    // cross-link href percent-encodes every reserved/unsafe byte
    // (TRAVERSAL_INJECTION_SUBJECT_ENCODED) and does NOT break out of the href
    // attribute or smuggle a second query param (ADR-044 §security). An injection
    // regression in EITHER shape is an UNSHIPPABLE security breach on an
    // attacker-influenced surface.
    assert_traversal_href_percent_encoded(&full_page.body);
    assert_traversal_href_percent_encoded(&fragment.body);
}
