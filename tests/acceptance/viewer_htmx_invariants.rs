//! Slice-07 acceptance — htmx-swaps GOLD / guardrail invariants (US-HX-005 +
//! the cross-cutting I-HX-2/3/4 guardrails from requirements.md / acceptance-criteria.md).
//!
//! These are the load-bearing, release-relevant guardrail gold tests for the htmx
//! progressive-enhancement DELTA — the BEHAVIORAL layer of the three-layer
//! enforcement (type + xtask `check-arch` are the other two, owned by DELIVER).
//! They drive the REAL `openlore ui` verb via the `ViewerServer` subprocess +
//! in-test HTTP (with/without `HX-Request`, ADR-035) over a REAL DuckDB, and assert
//! the hard slice-07 invariants on the OBSERVABLE surface:
//!
//! - `htmx_asset_served_locally` (H-5a, US-HX-005 / I-HX-2 / FR-HX-6): the new
//!   `GET /static/htmx.min.js` route returns 200 with the vendored JS (non-empty,
//!   looks like htmx) and a JS-ish content-type. (Asset route, not a data route.)
//! - `no_viewer_page_references_an_external_cdn` (H-5b, US-HX-005 / I-HX-2 / BR-HX-6):
//!   NO rendered viewer page references an off-host CDN to load htmx; the script src
//!   is the local `/static/htmx.min.js`. The offline guarantee, made structural.
//! - `serving_the_asset_adds_no_write_surface` (H-5c, US-HX-005 / I-HX-3): the asset
//!   route renders no sign control and writes nothing; the store row counts are
//!   unchanged after fetching it.
//! - `non_htmx_responses_are_byte_equivalent_to_slice_06` (H-INV-NoReg, I-HX-4 /
//!   NFR-HX-4): a non-htmx (no-header) request to each enhanced route returns a full
//!   page byte-for-byte equal to the slice-06 baseline (the page body delta is
//!   bounded to the `<div id>` swap-target wrapper + the local `<script src>` line —
//!   asserted as full-page chrome with the SAME content and NO CDN). The slice-06
//!   26-scenario suite is the companion release gate (run together).
//! - `htmx_fragment_routes_leave_the_store_read_only` (H-INV-ReadOnly, I-HX-3 /
//!   NFR-HX-3): exercising EVERY htmx fragment route (incl. POST /scrape via
//!   post_form_htmx) leaves `claims` + `peer_claims` row counts UNCHANGED — asserted
//!   via the universe-bound `assert_store_read_only` (Mandate 8; universe = the two
//!   port-exposed counts, all `unchanged`).
//! - `no_swap_route_adds_a_write_or_sign_surface` (H-INV-NoWrite, I-HX-3 / I-SCR-1):
//!   no fragment shape (paging, scrape, detail, tab) renders a sign control; no new
//!   write/sign route is reachable; the web process still holds no key.
//!
//! Driving discipline (Mandate 1): every assertion enters through the REAL `openlore
//! ui` subprocess + HTTP — never internal functions. The local DuckDB is REAL;
//! GitHub (only reachable via `/scrape`) is the REUSED slice-02 `FakeGithub` double.
//!
//! Layer placement (Mandate 11): layer-3/layer-5 subprocess + real-I/O, EXAMPLE-only.
//! These guardrails are example-based, never PBT-generated at this layer (the
//! `@property` tag marks them as universal invariants for the reader + the DELIVER
//! crafter; the generative exploration of the pure render core is a layer-2 concern,
//! out of this file's scope).
//!
//! No-regression GATE (release-relevant): H-INV-NoReg pins the slice-07 side; the
//! slice-06 26-scenario corpus (`viewer_store.rs` / `viewer_scrape.rs` /
//! `viewer_invariants.rs`) MUST also stay green — the no-header `get`/`post_form`
//! drivers are byte-unchanged (ADR-035 / I-HX-4). DELIVER runs both suites as the gate.
//!
//! Covers: US-HX-005 (H-5a/b/c) + I-HX-2/3/4 cross-cutting guardrails.
//
// SCAFFOLD: true (slice-07) — every test body is `todo!()`; the `get_htmx` /
// `post_form_htmx` / `references_external_cdn` / `is_full_page` harness seams
// COMPILE now, so each scenario fails at RUNTIME for a business reason (the asset
// route / fragment shape is unimplemented) — correct RED, not BROKEN.

mod support;

#[allow(unused_imports)]
use support::*;

// =============================================================================
// US-HX-005 — htmx served locally so swaps work offline (@infrastructure, H-5)
// =============================================================================

/// H-5a / GOLD `htmx_asset_served_locally` (US-HX-005 / I-HX-2 / FR-HX-6 / ADR-031):
/// the NEW `GET /static/htmx.min.js` asset route returns 200 with the vendored htmx
/// JavaScript — non-empty, recognizably htmx — and a JS-ish content-type. The asset
/// is served by the viewer process ITSELF (include_str! in-binary), loopback-only;
/// it is an asset route, NOT a data route.
///
/// Given the viewer is running;
/// When the local htmx asset route `GET /static/htmx.min.js` is fetched;
/// Then it returns 200 with the vendored htmx library (non-empty, looks like htmx).
///
/// @us-hx-005 @infrastructure @driving_port @real-io @offline @asset @gold
#[test]
fn htmx_asset_served_locally() {
    // GIVEN an initialized env + the viewer running (no GitHub seam — the asset
    // route never touches the network).
    // WHEN `GET /static/htmx.min.js` is fetched (the unchanged `get` — the asset
    // ignores the HX-Request header, ADR-031).
    // THEN it returns 200 with a non-empty body that LOOKS like htmx (carries an
    // htmx marker, e.g. the "htmx" identifier the library defines). The exact
    // content-type assertion is a DELIVER detail (application/javascript); here the
    // observable surface is "the local route serves the library".
    let env = TestEnv::initialized();
    let viewer = ViewerServer::start(&env);

    let asset = viewer.get("/static/htmx.min.js");

    assert_eq!(
        asset.status, 200,
        "the local htmx asset route must return 200; got {}",
        asset.status
    );
    assert!(
        !asset.body.is_empty(),
        "the asset route must serve the non-empty vendored htmx library; got an empty body"
    );
    assert!(
        asset.body_contains("htmx"),
        "the served asset must look like htmx (carry the \"htmx\" identifier the \
         library defines); got a body that does not mention htmx"
    );
    assert!(
        asset.content_type_looks_like_javascript(),
        "the asset must be served with a JavaScript content-type (the browser keys \
         script execution off it; FR-HX-6); got content-type {:?}",
        asset.content_type
    );
}

/// H-5b / GOLD `no_viewer_page_references_an_external_cdn` (US-HX-005 / I-HX-2 /
/// BR-HX-6): NO rendered viewer page references an off-host CDN to load htmx — every
/// page's script src is the LOCAL `/static/htmx.min.js`. The offline guarantee made
/// structural: if any page reached a CDN, swaps would silently break offline.
///
/// Given the viewer is serving its routes;
/// When the served HTML of every page-bearing route is inspected (view-source);
/// Then no page references an external CDN (unpkg / jsdelivr / cdnjs), and every page
/// references the local `/static/htmx.min.js`.
///
/// @us-hx-005 @infrastructure @property @driving_port @real-io @offline @no-cdn @gold
#[test]
fn no_viewer_page_references_an_external_cdn() {
    // GIVEN own + peer claims seeded (so the list/detail/peer pages render real
    // content) + the viewer running.
    // WHEN the served HTML of every page-bearing route is inspected — `/`, `/claims`,
    // `/claims/{cid}`, `/peer-claims` (full pages, no header — the script src lives
    // in the chrome, which only the full page carries).
    // THEN NO page references an external CDN (`references_external_cdn()` is false
    // for each) AND each carries the local `/static/htmx.min.js` script src. This is
    // a universal invariant over the page set (marked @property for the reader; it
    // stays example-pinned at this layer per Mandate 9/11).
    let env = TestEnv::initialized();
    // Seed own + peer claims through the production write paths so the list,
    // detail, and peer pages render REAL content (not empty-state chrome).
    let cid = seed_own_claim_with_evidence(
        &env,
        "rust-lang/rust",
        "is-maintained-by",
        "The Rust Project",
        0.90,
        &["https://github.com/rust-lang/rust/blob/HEAD/LICENSE-MIT"],
    );
    seed_cached_peer_claims(&env, "did:plc:peer-axum", 3);
    let viewer = ViewerServer::start(&env);

    // Every page-bearing route (full pages, no header — the script src lives in the
    // chrome, which only the full page carries). The detail route is addressed by
    // the seeded claim's real CID.
    let detail = format!("/claims/{cid}");
    let pages = ["/", "/claims", detail.as_str(), "/peer-claims"];
    for path in pages {
        let page = viewer.get(path);
        assert_eq!(
            page.status, 200,
            "page-bearing route {path:?} must return 200; got {}",
            page.status
        );
        assert!(
            !page.references_external_cdn(),
            "page {path:?} must NOT reference an external CDN to load htmx (the \
             offline guarantee; I-HX-2); got:\n{}",
            page.body
        );
        assert!(
            page.body_contains("/static/htmx.min.js"),
            "page {path:?} must reference the LOCAL `/static/htmx.min.js` script \
             src (offline-first; US-HX-005); got:\n{}",
            page.body
        );
    }
}

/// H-5c / GOLD `serving_the_asset_adds_no_write_surface` (US-HX-005 / I-HX-3 /
/// I-VIEW-1/2): serving the local htmx asset introduces NO write/sign route — the
/// asset route is GET-only fixed bytes; the web process still holds no key. Asserted
/// behaviorally: fetching the asset leaves the store row counts unchanged, and the
/// asset response renders no sign control.
///
/// Given the local htmx asset is served;
/// When the asset route is fetched (and the bind is inspected);
/// Then no write/sign route is introduced and the store row counts are unchanged.
///
/// @us-hx-005 @infrastructure @property @driving_port @real-io @i-hx-3 @gold
#[test]
fn serving_the_asset_adds_no_write_surface() {
    // GIVEN a populated store + the viewer running; the bind is loopback (I-VIEW-4,
    // proven by `base_url().contains("127.0.0.1")`).
    // WHEN the asset route is fetched (within a scope so the viewer's exclusive
    // DuckDB lock is released before the `after` snapshot — the no-write proof is
    // about what the viewer LEFT BEHIND, mirroring V-INV-1).
    // THEN the store row counts are UNCHANGED (universe-bound assert_store_read_only,
    // Mandate 8) and the asset body renders no sign control.
    let env = TestEnv::initialized();
    // A populated store so the read-only delta is over a non-trivial universe.
    let _cid = seed_own_claim_with_evidence(
        &env,
        "rust-lang/rust",
        "is-maintained-by",
        "The Rust Project",
        0.90,
        &["https://github.com/rust-lang/rust/blob/HEAD/LICENSE-MIT"],
    );

    let before = capture_store_row_count_universe(&env);

    // Fetch the asset inside a scope so the viewer's exclusive DuckDB lock is
    // released (on drop) BEFORE the `after` snapshot re-opens the store — the
    // no-write proof is about what the viewer LEFT BEHIND (mirrors V-INV-1).
    let asset = {
        let viewer = ViewerServer::start(&env);
        assert!(
            viewer.base_url().contains("127.0.0.1"),
            "the viewer must bind loopback-only (I-VIEW-4); got base_url {:?}",
            viewer.base_url()
        );
        viewer.get("/static/htmx.min.js")
    };

    let after = capture_store_row_count_universe(&env);

    // The asset route renders NO sign control — it is fixed JS bytes, not a page
    // with affordances (I-SCR-1; signing stays in the CLI).
    for marker in ["name=\"sign\"", "Sign claim", "value=\"sign\""] {
        assert!(
            !asset.body_contains(marker),
            "the asset route must render NO sign control (it is GET-only fixed \
             bytes; I-HX-3 / I-SCR-1); found {marker:?}"
        );
    }

    // The store row counts are UNCHANGED — every universe slot `unchanged`
    // (any change is an UNSHIPPABLE write-surface breach; I-HX-3).
    assert_store_read_only(&before, &after);
}

// =============================================================================
// I-HX-4 — No-regression: non-htmx responses byte-equivalent to slice-06 (H-INV-NoReg)
// =============================================================================

/// H-INV-NoReg / GOLD `non_htmx_responses_are_byte_equivalent_to_slice_06` (I-HX-4 /
/// NFR-HX-4): a non-htmx (no-header) request to EACH enhanced route returns the
/// COMPLETE slice-06 full page — full-page chrome around the SAME content, with NO
/// CDN reference. The page body delta vs slice-06 is bounded to the `<div id>`
/// swap-target wrapper + the local `<script src>` line (ADR-032 — the page now
/// composes the same fragment fn). The slice-06 26-scenario suite is the companion
/// release gate (run together — DELIVER's no-regression gate).
///
/// Given the htmx enhancement is layered on;
/// When each enhanced route is requested WITHOUT the `HX-Request` header;
/// Then each returns the complete slice-06 full page (full-page chrome + the content),
/// with no CDN reference and no behavioral change.
///
/// @us-hx-001 @us-hx-002 @us-hx-003 @us-hx-004 @us-hx-006 @property @driving_port
/// @real-io @i-hx-4 @no-regression @gold
#[test]
fn non_htmx_responses_are_byte_equivalent_to_slice_06() {
    // GIVEN own + peer claims persisted + the REUSED FakeGithub for `/scrape`.
    // WHEN each enhanced route is requested WITHOUT the header (the unchanged
    // get/post_form): `/`, `/claims`, `/claims?page=2`, `/peer-claims`, `/claims/{cid}`,
    // GET `/scrape`, POST `/scrape`.
    // THEN each returns a COMPLETE slice-06 full page (`is_full_page()`), carries its
    // expected content, and references NO external CDN. (Exact byte-equivalence vs a
    // recorded slice-06 baseline is DELIVER's tightening; the slice-06 corpus staying
    // green is the load-bearing companion. Here we pin the SHAPE + no-CDN guarantee.)
    let env = TestEnv::initialized();
    // Seed own + peer claims through the production write paths so the My Claims,
    // detail, and Peer Claims pages render REAL content (not empty-state chrome).
    let cid = seed_own_claim_with_evidence(
        &env,
        "rust-lang/cargo",
        "is-maintained-by",
        "The Cargo Team",
        0.90,
        &["https://github.com/rust-lang/cargo/blob/HEAD/LICENSE-MIT"],
    );
    seed_cached_peer_claims(&env, "did:plc:peer-axum", 3);
    // The REUSED slice-02 FakeGithub double serves the live POST /scrape harvest —
    // the ONLY mocked boundary; reached via OPENLORE_GITHUB_API_BASE.
    let github = GithubServer::start(FakeGithub::for_public_repo(
        "rust-lang/cargo",
        fixture_cargo_five_signals(),
    ));
    let viewer = ViewerServer::start_with_github(&env, github);

    // Every enhanced route, requested WITHOUT the `HX-Request` header (the unchanged
    // no-JS / bookmark / direct-URL drivers `get` / `post_form`). The detail route is
    // addressed by the seeded claim's real CID.
    let detail = format!("/claims/{cid}");
    let get_routes = [
        "/",
        "/claims",
        "/claims?page=2",
        "/peer-claims",
        detail.as_str(),
        "/scrape",
    ];
    let mut responses = Vec::new();
    for path in get_routes {
        responses.push((path.to_string(), viewer.get(path)));
    }
    // POST /scrape (no header) — the live propose returns the COMPLETE slice-06
    // `/scrape` full page (the full-page arm of the Shape fork), not the
    // `#scrape-results` fragment.
    responses.push((
        "POST /scrape".to_string(),
        viewer.post_form("/scrape", &[("target", "rust-lang/cargo")]),
    ));

    for (path, r) in &responses {
        // Each enhanced route still renders successfully without the header.
        assert_eq!(
            r.status, 200,
            "enhanced route {path:?} WITHOUT HX-Request must return 200 (no \
             behavioral change); got {}",
            r.status
        );
        // The COMPLETE slice-06 full page — full-page chrome around the content, NOT
        // a bare htmx fragment (I-HX-4: the no-header request is unchanged).
        assert!(
            r.is_full_page(),
            "enhanced route {path:?} WITHOUT HX-Request must return the COMPLETE \
             slice-06 full page (`<!DOCTYPE html>` + `<html>` chrome), not a \
             fragment; body was:\n{}",
            r.body
        );
        // No off-host CDN reference — the offline guarantee made structural; the
        // bounded chrome delta references only the LOCAL asset.
        assert!(
            !r.references_external_cdn(),
            "enhanced route {path:?} WITHOUT HX-Request must NOT reference an \
             external CDN (the offline guarantee; I-HX-2); body was:\n{}",
            r.body
        );
        // The bounded chrome delta vs slice-06: where a CDN script line WOULD be,
        // the full page references only the LOCAL `/static/htmx.min.js`. The
        // store-backed pages (`/`, `/claims`, `/peer-claims`, `/claims/{cid}`) carry
        // this chrome line (the htmx-driven tab/paging swaps need the library
        // loaded); the `/scrape` page drives its swap from the POSTed form and
        // carries only the `#scrape-results` swap-target wrapper (no head script in
        // its slice-06 chrome — H-5b's no-CDN gold gate likewise scopes the local
        // script src to the store-backed page set). The universal guarantee across
        // EVERY route is the no-CDN invariant asserted above.
        if !path.starts_with("/scrape") && path != "POST /scrape" {
            assert!(
                r.body_contains("/static/htmx.min.js"),
                "enhanced route {path:?} full page must reference the LOCAL \
                 `/static/htmx.min.js` script src (the bounded chrome delta; \
                 offline-first US-HX-005); body was:\n{}",
                r.body
            );
        }
    }
}

// =============================================================================
// I-HX-3 — Read-only preserved: htmx fragment routes leave the store unchanged
//          (H-INV-ReadOnly) + no swap adds a write/sign surface (H-INV-NoWrite)
// =============================================================================

/// H-INV-ReadOnly / GOLD `htmx_fragment_routes_leave_the_store_read_only` (I-HX-3 /
/// NFR-HX-3): exercising EVERY htmx FRAGMENT route — `/claims?page=N`,
/// `/peer-claims?page=N`, `/claims/{cid}`, AND POST `/scrape` via `post_form_htmx`
/// (the live harvest that must persist NOTHING, BR-HX-4) — leaves the `claims` +
/// `peer_claims` row counts UNCHANGED. The htmx-shape companion to the slice-06
/// V-INV-1 read-only gold test, asserted via the universe-bound state-delta
/// (Mandate 8: universe = the two port-exposed counts, each `unchanged`).
///
/// Given a store seeded with own + peer claims and a reachable scrape target;
/// When every htmx FRAGMENT route (incl. POST /scrape htmx) is exercised;
/// Then the `claims` and `peer_claims` row counts are UNCHANGED.
///
/// @us-hx-001 @us-hx-002 @us-hx-003 @us-hx-004 @us-hx-006 @property @driving_port
/// @real-io @i-hx-3 @read-only @gold
#[test]
fn htmx_fragment_routes_leave_the_store_read_only() {
    // GIVEN a REAL store seeded (through production write paths) with own + peer
    // claims, plus a reachable scrape target (the REUSED FakeGithub) so the POST
    // /scrape htmx live harvest actually runs (and must still persist nothing).
    // WHEN every htmx FRAGMENT route is exercised via the header-setting drivers
    // (get_htmx for the GET routes; post_form_htmx for /scrape), within a scope so
    // the viewer's exclusive DuckDB lock is released before the `after` snapshot.
    // THEN the persisted-store row counts are UNCHANGED — every universe slot
    // `unchanged` (assert_store_read_only; any change is UNSHIPPABLE — I-HX-3).
    todo!(
        "DELIVER H-INV-ReadOnly: own_cid = seed_own_claim_with_evidence(...); \
         seed_cached_peer_claims(env, \"did:plc:peer-axum\", 60); \
         github = for_public_repo(\"rust-lang/cargo\", fixture_cargo_five_signals()); \
         before = capture_store_row_count_universe(&env); \
         in a scope: viewer = ViewerServer::start_with_github(&env, github); \
         let _ = viewer.get_htmx(\"/claims?page=2\"); \
         let _ = viewer.get_htmx(\"/peer-claims?page=2\"); \
         let _ = viewer.get_htmx(&format!(\"/claims/{{own_cid}}\")); \
         let _ = viewer.post_form_htmx(\"/scrape\", &[(\"target\",\"rust-lang/cargo\")]); drop viewer; \
         after = capture_store_row_count_universe(&env); \
         assert_store_read_only(&before, &after)"
    );
}

/// H-INV-NoWrite / GOLD `no_swap_route_adds_a_write_or_sign_surface` (I-HX-3 /
/// I-VIEW-1/2 / I-SCR-1): no htmx FRAGMENT shape renders a sign control — paging,
/// detail, tab-switch, and scrape fragments all carry NO sign affordance (the human
/// gate stays in the CLI). Asserted on the observable rendered surface across every
/// fragment route, the htmx-shape companion to slice-06 V-INV-4.
///
/// Given the viewer is serving the htmx-enhanced routes over a populated store;
/// When every htmx fragment route is requested;
/// Then no fragment renders a sign control (no new write/sign surface).
///
/// @us-hx-001 @us-hx-002 @us-hx-003 @us-hx-004 @us-hx-006 @property @driving_port
/// @real-io @i-hx-3 @i-scr-1 @gold
#[test]
fn no_swap_route_adds_a_write_or_sign_surface() {
    // GIVEN own + peer claims + the REUSED FakeGithub for the /scrape htmx fragment.
    // WHEN every htmx fragment route is requested (get_htmx for the GET routes;
    // post_form_htmx for /scrape).
    // THEN NO fragment renders a sign control (`name="sign"`, `Sign claim`,
    // `value="sign"`) — the load-bearing ABSENCE of a sign affordance on every swap
    // surface (I-SCR-1, signing stays in the CLI; the read-only delta is the
    // companion H-INV-ReadOnly).
    todo!(
        "DELIVER H-INV-NoWrite: own_cid = seed_own_claim_with_evidence(...); \
         seed_cached_peer_claims(...); github = for_public_repo(...); \
         viewer = start_with_github; collect frags from get_htmx(\"/claims?page=1\"), \
         get_htmx(\"/peer-claims\"), get_htmx(&format!(\"/claims/{{own_cid}}\")), \
         post_form_htmx(\"/scrape\", target); for frag in frags: for m in \
         [\"name=\\\"sign\\\"\", \"Sign claim\", \"value=\\\"sign\\\"\"]: assert !frag.body_contains(m)"
    );
}
