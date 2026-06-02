//! Slice-07 acceptance — htmx partial-swaps as a PROGRESSIVE ENHANCEMENT over the
//! shipped slice-06 `openlore ui` read-only viewer (US-HX-001..004 + US-HX-006;
//! US-HX-005 lives in `viewer_htmx_invariants.rs`).
//!
//! The DELTA (DESIGN architecture-design.md): the HTTP surface (URLs + methods) is
//! UNCHANGED; only the response SHAPE varies by the `HX-Request` header (ADR-033).
//! Present → just the swap-target FRAGMENT (the region, no chrome); absent → the
//! COMPLETE slice-06 full page, byte-equivalent (I-HX-1 / I-HX-4). The full page
//! EMBEDS the same fragment fn, so parity is structural (ADR-032 / I-HX-5). The htmx
//! asset is served locally, never a CDN (ADR-031 / I-HX-2).
//!
//! EVERY interaction story carries THREE scenario kinds (acceptance-criteria.md):
//!   (a) htmx fragment — request WITH `HX-Request` returns ONLY the swap-target
//!       region (assert: NOT a full page — no `<!DOCTYPE html>`/`<html>` chrome —
//!       AND the region's content present).
//!   (b) no-JS full page — the SAME request WITHOUT `HX-Request` returns the COMPLETE
//!       slice-06 full page (assert: full-page chrome + the region).
//!   (c) parity — the fragment content equals the corresponding region of the full
//!       page (same rows / indicator / verbatim confidence / peer origin).
//!
//! Driving discipline (Mandate 1, hard requirement): every scenario enters through
//! the CLI driving port — the REAL `openlore ui` subprocess (via the `ViewerServer`
//! spawn helper) + in-test HTTP with/without the `HX-Request` header (the ADR-035
//! `get`/`get_htmx` + `post_form`/`post_form_htmx` pair). NO scenario calls the
//! `viewer-domain` `render_*_fragment` fns directly (those are unit-level, exercised
//! in DELIVER). The local DuckDB is REAL (BR-VIEW-4 — the SAME store the CLI writes,
//! seeded through the production `claim add` write path / the fast bulk peer seed).
//! GitHub (only on `/scrape`) is the REUSED slice-02 `FakeGithub` double via the
//! existing `OPENLORE_GITHUB_API_BASE` env seam — a NEW double is NOT built.
//!
//! Layer placement (nw-tdd-methodology Layered Test Discipline matrix + Mandate 11):
//! every test here is a layer-3/layer-5 subprocess + real-I/O test — EXAMPLE-ONLY.
//! The sad paths (zero candidates, network down, unknown CID, missing origin,
//! over-the-end page) are enumerated explicitly, NEVER PBT-generated at this layer.
//!
//! Build-before-run (carry into DELIVER roadmap, mirrors the slice-06 viewer ATs):
//! `cargo test` does NOT rebuild a spawned binary — the run MUST `cargo build` the
//! `openlore` bin first so `ViewerServer` spawns the CURRENT `openlore ui`.
//!
//! No-regression GATE: the slice-06 26-scenario corpus (`viewer_store.rs` /
//! `viewer_scrape.rs` / `viewer_invariants.rs`) MUST stay green — the no-header
//! `get`/`post_form` drivers are byte-unchanged (ADR-035 / I-HX-4). The slice-07
//! byte-equivalence guardrails live in `viewer_htmx_invariants.rs`.
//!
//! Covers:
//! - US-HX-001 (WALKING SKELETON): /claims?page=N paging — fragment (H-1a) +
//!   no-JS full page (H-1b) + parity (H-1c) + over-the-end clamp in both shapes (H-1d)
//! - US-HX-002: /peer-claims?page=N paging — fragment (H-2a) + no-JS (H-2b) +
//!   parity (H-2c) + unknown-origin still renders in the fragment (H-2d)
//! - US-HX-003: POST /scrape results swap — fragment w/ derived-from + no sign
//!   control (H-3a) + zero-candidates fragment (H-3b) + network-down fragment (H-3c) +
//!   no-JS full page (H-3d) + parity (H-3e)
//! - US-HX-004: GET /claims/{cid} detail — fragment (H-4a) + no-JS (H-4b) +
//!   unknown-CID guided 404 in both shapes (H-4c) + no-evidence in both shapes (H-4d) +
//!   parity (H-4e)
//! - US-HX-006: tab switch /claims <-> /peer-claims — fragment into #view-panel
//!   (H-6a) + no-JS full page per URL (H-6b) + bookmark/reload re-enters full page
//!   (H-6c) + parity (H-6d)
//
// SCAFFOLD: true (slice-07) — every test body is `todo!()`; the `get_htmx` /
// `post_form_htmx` harness seam COMPILES now (it only adds a header to the existing
// reqwest call), so each scenario fails at RUNTIME for a business reason (the
// fragment shape is unimplemented) — correct RED, not BROKEN. DELIVER fills the
// bodies one scenario at a time.

mod support;

#[allow(unused_imports)]
use support::*;

// =============================================================================
// US-HX-001 — Pagination swaps the claims table in place (WALKING SKELETON, H-1)
// =============================================================================
//
// The thinnest end-to-end htmx thread: it proves header-drives-shape + fragment/
// full-page parity + no-JS fallback on the SAME `/claims?page=N` route. Demo-able:
// "Maria clicks Next and only the table updates; with JS off the same link returns
// the whole page." Route: GET /claims?page=N. Swap target: #claims-table.

/// H-1a (US-HX-001 happy; AC htmx-request fragment): WITH `HX-Request`, page 2 of a
/// 312-claim store returns ONLY the `#claims-table` fragment — the next 50 rows +
/// the "51–100 of 312" position indicator + Prev/Next — and NOT a full page (no
/// `<!DOCTYPE html>`/`<html>` chrome). This is the walking-skeleton htmx thread.
///
/// Given Maria has 312 signed claims rendered 50 per page;
/// When her browser requests page 2 WITH the `HX-Request` header;
/// Then the response is ONLY the claims-table fragment showing 51–100 of 312, not a
/// full page.
///
/// @us-hx-001 @walking_skeleton @driving_port @driving_adapter @real-io @htmx-fragment
/// @kpi-hx-1 @happy
#[test]
fn paging_claims_with_htmx_returns_only_the_table_fragment() {
    // GIVEN Maria has 312 signed claims through the PRODUCTION `claim add` write
    // path (Pillar 3 — the SAME store `openlore ui` reads, BR-VIEW-4).
    // WHEN she requests `/claims?page=2` WITH the `HX-Request` header (get_htmx).
    // THEN the response is ONLY the `#claims-table` fragment: it shows "51–100 of
    // 312" and the page-3 Prev/Next anchors, it carries NO full-page chrome
    // (`response.is_fragment()` / NOT `is_full_page()`), and it renders confidence
    // verbatim (0.90). DELIVER materializes this first (walking skeleton).
    let env = TestEnv::initialized();
    seed_own_claims_via_cli(&env, 312);
    let viewer = ViewerServer::start(&env);

    let page = viewer.get_htmx("/claims?page=2");

    assert_eq!(page.status, 200, "the htmx claims request returns 200; got {}", page.status);
    assert!(
        page.is_fragment(),
        "the HX-Request response must be ONLY the swap-target fragment (no full-page \
         chrome); got:\n{}",
        page.body
    );
    assert!(
        !page.is_full_page(),
        "the HX-Request response must NOT carry full-page chrome (no <!DOCTYPE html>/\
         <html>); got:\n{}",
        page.body
    );
    assert!(
        page.body_contains("51\u{2013}100 of 312"),
        "the fragment must show the page-2 indicator \"51\u{2013}100 of 312\" (EN DASH); \
         got:\n{}",
        page.body
    );
    assert!(
        page.body_contains("id=\"claims-table\""),
        "the fragment must be wrapped in the swap-target element id=\"claims-table\"; \
         got:\n{}",
        page.body
    );
    assert!(
        page.body_contains("0.90"),
        "the fragment renders confidence verbatim (0.90); got:\n{}",
        page.body
    );
    assert!(
        page.body_contains("?page=1"),
        "page 2 links Previous to ?page=1; got:\n{}",
        page.body
    );
    assert!(
        page.body_contains("?page=3"),
        "page 2 links Next to ?page=3; got:\n{}",
        page.body
    );
}

/// H-1b (US-HX-001 edge; AC no-JS full-page fallback): the SAME `/claims?page=2`
/// request WITHOUT `HX-Request` (plain Next link, JS off, curl, bookmark) returns
/// the COMPLETE slice-06 full page — full-page chrome (`<!DOCTYPE html>` + `<html>`
/// + the My Claims title) AROUND the same table region (I-HX-1).
///
/// Given JavaScript is disabled;
/// When Maria requests `/claims?page=2` WITHOUT the `HX-Request` header;
/// Then the server returns the complete `/claims?page=2` page (full-page chrome +
/// the table region).
///
/// @us-hx-001 @driving_port @real-io @no-js @full-page @edge
#[test]
fn paging_claims_without_htmx_returns_the_full_page() {
    // GIVEN 312 own claims seeded via the production write path.
    // WHEN she requests `/claims?page=2` WITHOUT the header (the unchanged `get`).
    // THEN the response is the COMPLETE slice-06 full page: `page.is_full_page()`
    // (carries `<!DOCTYPE html>` + `<html>` + the My Claims chrome) AND the same
    // table region ("51–100 of 312"). Byte-equivalence vs slice-06 is the
    // guardrail in viewer_htmx_invariants.rs; here we pin the SHAPE (full page).
    let env = TestEnv::initialized();
    seed_own_claims_via_cli(&env, 312);
    let viewer = ViewerServer::start(&env);

    let page = viewer.get("/claims?page=2");

    assert_eq!(
        page.status, 200,
        "the no-header claims request returns 200; got {}",
        page.status
    );
    assert!(
        page.is_full_page(),
        "WITHOUT HX-Request the response must be the COMPLETE slice-06 full page \
         (<!DOCTYPE html> + <html> chrome); got:\n{}",
        page.body
    );
    assert!(
        page.body_contains("51\u{2013}100 of 312"),
        "the full page must show the page-2 indicator \"51\u{2013}100 of 312\" (EN DASH) \
         in its table region; got:\n{}",
        page.body
    );
    assert!(
        page.body_contains("id=\"claims-table\""),
        "the full page must wrap the same swap-target region id=\"claims-table\"; \
         got:\n{}",
        page.body
    );
}

/// H-1c (US-HX-001; AC fragment/full-page parity, I-HX-5): for the SAME input
/// (`/claims?page=2`), the htmx fragment content equals the table region of the
/// full page — same rows, same "51–100 of 312" indicator, same verbatim confidence.
/// The full page EMBEDS the fragment fn (ADR-032), so this asserts the two shapes
/// agree empirically in the running viewer (the Earned-Trust parity check).
///
/// Given Maria has 312 signed claims;
/// When `/claims?page=2` is fetched both WITH and WITHOUT the `HX-Request` header;
/// Then the fragment is contained within the full page's table region (same rows +
/// indicator + verbatim confidence).
///
/// @us-hx-001 @driving_port @real-io @parity @i-hx-5 @happy
#[test]
fn claims_fragment_equals_the_full_page_table_region() {
    // GIVEN 312 own claims (production write path).
    // WHEN both shapes of `/claims?page=2` are fetched (get_htmx + get).
    // THEN the fragment's load-bearing content (the "51–100 of 312" indicator, the
    // first row's subject/predicate/object, the verbatim 0.90 confidence) is also
    // present in the full page — the full page is chrome wrapped around the SAME
    // fragment. Parity asserted on the observable rendered text the operator sees.
    let env = TestEnv::initialized();
    seed_own_claims_via_cli(&env, 312);
    let viewer = ViewerServer::start(&env);

    let frag = viewer.get_htmx("/claims?page=2");
    let full = viewer.get("/claims?page=2");

    assert_eq!(frag.status, 200, "the fragment request returns 200");
    assert_eq!(full.status, 200, "the full-page request returns 200");

    // The fragment is ONLY the swap-target region; the full page is chrome wrapped
    // AROUND the SAME fragment fn (ADR-032) — so the two shapes agree empirically.
    assert!(
        frag.is_fragment(),
        "the HX-Request response must be ONLY the fragment (no chrome); got:\n{}",
        frag.body
    );
    assert!(
        full.is_full_page(),
        "the no-header response must be the complete full page; got:\n{}",
        full.body
    );

    // PARITY: the load-bearing rendered content of the page-2 fragment is also
    // present in the full page's table region. Every page-2 row shares the seeded
    // predicate/object/confidence; the indicator + swap-target id are exact. These
    // needles are guaranteed present on any non-empty page (seed fields are fixed),
    // so parity is asserted behaviorally on the text the operator sees.
    for needle in [
        "51\u{2013}100 of 312",
        "id=\"claims-table\"",
        "is-maintained-by",
        "The Maintainers",
        "0.90",
    ] {
        assert!(
            frag.body_contains(needle),
            "the fragment must contain {needle:?} (page-2 parity); got:\n{}",
            frag.body
        );
        assert!(
            full.body_contains(needle),
            "the full page must contain the SAME {needle:?} in its table region \
             (page-2 parity); got:\n{}",
            full.body
        );
    }
}

/// H-1d (US-HX-001 boundary; AC over-the-end clamp): a request past the last page
/// (`?page=99` on 312 claims) clamps to the LAST page "301–312 of 312" — NOT a
/// blank result — and this holds in BOTH shapes (slice-06 DV-5 clamp preserved in
/// the fragment AND the full page).
///
/// Given 312 claims at page size 50 (last page is 301–312 of 312);
/// When a request asks for `?page=99` WITH and WITHOUT the header;
/// Then both the htmx fragment and the full page show "301–312 of 312", not a blank
/// result.
///
/// @us-hx-001 @driving_port @real-io @boundary @clamp @edge
#[test]
fn over_the_end_page_clamps_in_both_shapes() {
    // GIVEN 312 own claims (last page is 301–312).
    // WHEN `/claims?page=99` is fetched WITH (get_htmx) and WITHOUT (get) the header.
    // THEN the fragment shows "301–312 of 312" (and is a fragment) AND the full page
    // shows "301–312 of 312" (and is a full page) — neither is blank. The clamp is
    // the slice-06 DV-5 behavior, preserved across both shapes (I-HX-4 / I-HX-5).
    let env = TestEnv::initialized();
    seed_own_claims_via_cli(&env, 312);
    let viewer = ViewerServer::start(&env);

    let frag = viewer.get_htmx("/claims?page=99");
    let full = viewer.get("/claims?page=99");

    assert_eq!(frag.status, 200, "the over-the-end fragment request returns 200");
    assert_eq!(full.status, 200, "the over-the-end full-page request returns 200");

    // The slice-06 DV-5 clamp (PageView::paged) resolves ?page=99 to the LAST page,
    // so BOTH shapes show "301–312 of 312" — never a blank result (I-HX-4 / I-HX-5).
    assert!(
        frag.body_contains("301\u{2013}312 of 312"),
        "the clamped fragment must show the last-page indicator \
         \"301\u{2013}312 of 312\" (EN DASH), not a blank result; got:\n{}",
        frag.body
    );
    assert!(
        frag.is_fragment(),
        "the HX-Request clamped response must still be ONLY the fragment; got:\n{}",
        frag.body
    );
    assert!(
        full.body_contains("301\u{2013}312 of 312"),
        "the clamped full page must show the last-page indicator \
         \"301\u{2013}312 of 312\" (EN DASH), not a blank result; got:\n{}",
        full.body
    );
    assert!(
        full.is_full_page(),
        "the no-header clamped response must still be the complete full page; got:\n{}",
        full.body
    );
}

// =============================================================================
// US-HX-002 — Pagination swaps the peer-claims table in place (H-2)
// =============================================================================
//
// The US-HX-001 pattern applied to GET /peer-claims?page=N. NOTE (DESIGN
// component-boundaries §peer_claims_page): DELIVER MUST thread `?page=N` into the
// peer handler (slice-06 only served page 1) — reusing the SAME `parse_page` +
// `PageView::paged` machinery the claims handler already uses. Route: GET
// /peer-claims?page=N. Swap target: #claims-table (inside #view-panel).

/// H-2a (US-HX-002 happy; AC htmx-request fragment): WITH `HX-Request`, page 2 of a
/// federated peer set returns ONLY the peer-claims-table fragment — the next 50
/// rows WITH their origin (the peer's DID) + the "X–Y of N" indicator — and NOT a
/// full page. Peer rows keep their origin and stay separable from own claims
/// (KPI-VIEW-3 carried into the fragment shape).
///
/// Given Maria has a federated peer set rendered 50 per page;
/// When her browser requests peer-claims page 2 WITH the `HX-Request` header;
/// Then the response is ONLY the peer-claims-table fragment with the next rows and
/// their origin, not a full page.
///
/// @us-hx-002 @driving_port @real-io @htmx-fragment @peer-origin @happy
#[test]
fn paging_peer_claims_with_htmx_returns_only_the_peer_table_fragment() {
    // GIVEN a federated peer set large enough to page (120 rows from one peer,
    // seeded via the fast bulk `seed_cached_peer_claims` — the SAME `peer_claims`
    // table `openlore ui` reads, BR-VIEW-4; a real-sized federated set is exercised
    // by the slice-06 corpus, this scenario only needs >1 page).
    // WHEN she requests `/peer-claims?page=2` WITH the header (get_htmx) — DELIVER
    // threads `?page=N` into the peer handler.
    // THEN the response is ONLY the peer-table fragment: it carries the peer's
    // origin (the peer DID) on the rows, shows the page-2 "51–100 of 120"
    // indicator, and is a fragment (NOT a full page).
    todo!(
        "DELIVER H-2a: seed_cached_peer_claims(env, \"did:plc:peer-axum\", 120); \
         frag = viewer.get_htmx(\"/peer-claims?page=2\"); assert frag.status==200, \
         frag.is_fragment(), frag.body_contains(\"did:plc:peer-axum\"), \
         frag.body_contains(\"51–100 of 120\")"
    );
}

/// H-2b (US-HX-002 edge; AC no-JS full-page fallback): the SAME `/peer-claims?page=2`
/// request WITHOUT `HX-Request` returns the COMPLETE slice-06 peer-claims page
/// (full-page chrome + the peer table region) (I-HX-1).
///
/// Given JavaScript is disabled;
/// When Maria requests `/peer-claims?page=2` WITHOUT the header;
/// Then the server returns the complete slice-06 `/peer-claims?page=2` page.
///
/// @us-hx-002 @driving_port @real-io @no-js @full-page @edge
#[test]
fn paging_peer_claims_without_htmx_returns_the_full_page() {
    // GIVEN 120 peer rows (fast bulk seed).
    // WHEN she requests `/peer-claims?page=2` WITHOUT the header (the unchanged get).
    // THEN the response is the COMPLETE slice-06 full page (`is_full_page()`) with
    // the same peer table region (the page-2 indicator + the peer origin).
    todo!(
        "DELIVER H-2b: seed_cached_peer_claims(env, \"did:plc:peer-axum\", 120); \
         full = viewer.get(\"/peer-claims?page=2\"); assert full.status==200, \
         full.is_full_page(), full.body_contains(\"51–100 of 120\")"
    );
}

/// H-2c (US-HX-002; AC parity, I-HX-5): the peer-claims fragment for `?page=2`
/// equals the peer-table region of the full page — same rows, same indicator, same
/// peer origin (KPI-VIEW-3). The full page composes the same fragment fn (ADR-032).
///
/// Given Maria has a federated peer set;
/// When `/peer-claims?page=2` is fetched both WITH and WITHOUT the header;
/// Then the fragment's rows + indicator + peer origin are contained in the full
/// page's peer-table region.
///
/// @us-hx-002 @driving_port @real-io @parity @peer-origin @i-hx-5 @happy
#[test]
fn peer_claims_fragment_equals_the_full_page_peer_table_region() {
    // GIVEN 120 peer rows.
    // WHEN both shapes of `/peer-claims?page=2` are fetched.
    // THEN the page-2 indicator + a known peer row's origin/subject appear in BOTH
    // the fragment and the full page (parity); the fragment is a fragment and the
    // full page is a full page.
    todo!(
        "DELIVER H-2c: seed 120 peer rows; frag = viewer.get_htmx(\"/peer-claims?page=2\"); \
         full = viewer.get(\"/peer-claims?page=2\"); for needle in [\"51–100 of 120\", \
         peer DID, a known subject]: assert frag.body_contains(needle) && \
         full.body_contains(needle); assert frag.is_fragment() && full.is_full_page()"
    );
}

/// H-2d (US-HX-002 boundary; AC unknown origin still renders): a peer row with NO
/// recorded origin still renders in the FRAGMENT, labeled origin "unknown" — never
/// dropped (carries slice-06 V-10 behavior into the fragment shape).
///
/// Given a federated peer claim has no recorded origin;
/// When Maria pages the Peer Claims list WITH the `HX-Request` header;
/// Then that row still renders in the fragment with origin shown as "unknown".
///
/// @us-hx-002 @driving_port @real-io @htmx-fragment @boundary @edge
#[test]
fn peer_claim_with_unknown_origin_still_renders_in_the_fragment() {
    // GIVEN a peer_claims row with a blank/absent origin (the defensive
    // `seed_peer_claim_with_blank_origin` fixture — bypasses the slice-03 CHECK to
    // reach the "unknown" render path).
    // WHEN Maria pages the Peer Claims list WITH the header (get_htmx).
    // THEN the row still renders in the fragment labeled "unknown" — never dropped —
    // and the response is a fragment (no full-page chrome).
    todo!(
        "DELIVER H-2d: seed_peer_claim_with_blank_origin(&env); \
         frag = viewer.get_htmx(\"/peer-claims\"); assert frag.is_fragment(), \
         frag.body_contains(\"unknown\")"
    );
}

// =============================================================================
// US-HX-003 — Live scrape swaps results below the form (H-3)
// =============================================================================
//
// POST /scrape via post_form_htmx, REUSING the slice-02 FakeGithub double (the ONLY
// mocked boundary, via the existing OPENLORE_GITHUB_API_BASE seam — a NEW double is
// NOT built). Swap target: #scrape-results. NO sign control in the fragment;
// nothing persisted; derived-from present on candidates (BR-HX-4/5 / I-SCR-1).

/// H-3a (US-HX-003 happy; AC htmx-request fragment): WITH `HX-Request`, submitting a
/// scrape target returns ONLY the `#scrape-results` region (the candidates with
/// their derived-from provenance) — NOT a full page, NO sign control, nothing
/// persisted. The form + its value stay in place (browser-side; the server returns
/// just the results region).
///
/// Given Maria is on the Live Scrape view with network available and a target that
/// would propose candidates;
/// When she submits the target WITH the `HX-Request` header;
/// Then ONLY the results region updates to show the candidates with derived-from,
/// no sign control is rendered, and the response is a fragment.
///
/// @us-hx-003 @driving_port @driving_adapter @real-io @htmx-fragment @derived-from
/// @kpi-hx-2 @happy
#[test]
fn submitting_scrape_with_htmx_returns_only_the_results_fragment() {
    // GIVEN an initialized env + the REUSED slice-02 FakeGithub serving a public
    // repo with harvestable signals (the ONLY mocked boundary). The viewer reaches
    // it through `ViewerServer::start_with_github` (the existing OPENLORE_GITHUB_API_BASE
    // seam) — exactly as the slice-06 scrape scenarios do; NO new double.
    // WHEN Maria submits the target WITH the header (post_form_htmx).
    // THEN the response is ONLY the `#scrape-results` fragment: candidates render
    // their subject + derived-from provenance, NO sign control marker appears, and
    // the response is a fragment (no full-page chrome). Persistence-zero is the
    // read-only gold guardrail (viewer_htmx_invariants.rs); here we pin the SHAPE.
    todo!(
        "DELIVER H-3a: github = GithubServer::start(FakeGithub::for_public_repo(\
         \"rust-lang/cargo\", fixture_cargo_five_signals())); \
         viewer = ViewerServer::start_with_github(&env, github); \
         frag = viewer.post_form_htmx(\"/scrape\", &[(\"target\", \"rust-lang/cargo\")]); \
         assert frag.status==200, frag.is_fragment(), \
         frag.body_contains(\"rust-lang/cargo\"), frag.body_contains(\"derived-from\"); \
         for m in [\"name=\\\"sign\\\"\", \"Sign claim\"]: assert !frag.body_contains(m)"
    );
}

/// H-3b (US-HX-003 edge; AC zero candidates): a target that derives NO candidates
/// swaps in the guided "No candidate claims could be derived" message in the
/// FRAGMENT — not a blank region, not a full page.
///
/// Given a live scrape of "some-org/empty-repo" derives no candidates;
/// When Maria submits that target WITH the `HX-Request` header;
/// Then the results fragment shows "No candidate claims could be derived" with a
/// suggestion.
///
/// @us-hx-003 @driving_port @real-io @htmx-fragment @empty-state @edge
#[test]
fn scrape_with_no_candidates_swaps_in_guidance_fragment() {
    // GIVEN a public repo that harvests successfully but yields NO derivable
    // candidates (the REUSED FakeGithub with no signals).
    // WHEN Maria submits the target WITH the header (post_form_htmx).
    // THEN the results fragment shows the guided zero-candidates message — and is a
    // fragment (no full-page chrome).
    todo!(
        "DELIVER H-3b: github = GithubServer::start(FakeGithub::for_public_repo(\
         \"some-org/empty-repo\", vec![])); viewer = start_with_github; \
         frag = viewer.post_form_htmx(\"/scrape\", &[(\"target\", \"some-org/empty-repo\")]); \
         assert frag.is_fragment(), frag.body_contains(\"No candidate claims could be derived\")"
    );
}

/// H-3c (US-HX-003 error; AC network down): GitHub unreachable. Submitting WITH
/// `HX-Request` swaps in a FRAGMENT that states GitHub could not be reached AND that
/// her store view still works offline — leaking NO transport/stack internals
/// (NFR-HX-7, carrying the slice-06 DV-4 payload-free error pattern).
///
/// Given Maria cannot reach GitHub;
/// When she submits "tokio-rs/tokio" WITH the `HX-Request` header;
/// Then the results fragment shows that GitHub could not be reached, notes the store
/// view still works offline, and leaks no transport internals.
///
/// @us-hx-003 @driving_port @real-io @htmx-fragment @network-failure @no-leak @error
#[test]
fn scrape_network_down_swaps_in_offline_guidance_fragment_without_leaking() {
    // GIVEN GitHub is unreachable (the REUSED slice-02 `FakeGithub::offline()`
    // posture — the established network-down double).
    // WHEN Maria submits a target WITH the header (post_form_htmx).
    // THEN the results fragment names the cause in domain language ("GitHub could
    // not be reached"), reassures the store view still works offline, is a fragment,
    // and leaks NO transport internals (no status codes, no "connection refused",
    // no raw URLs, no stack trace) — the same no-leak set the slice-06 V-S4 pins.
    todo!(
        "DELIVER H-3c: github = GithubServer::start(FakeGithub::offline()); \
         viewer = start_with_github; frag = viewer.post_form_htmx(\"/scrape\", \
         &[(\"target\", \"tokio-rs/tokio\")]); assert frag.is_fragment(), \
         frag.body_contains(\"GitHub could not be reached\"), \
         frag.body_contains(\"store view still works offline\"); for leaked in \
         [\"connection refused\",\"timed out\",\"503\",\"http://\",\"panicked at\"]: \
         assert !frag.body.to_lowercase().contains(leaked)"
    );
}

/// H-3d (US-HX-003 edge; AC no-JS full-page fallback): submitting the scrape form
/// WITHOUT `HX-Request` (plain `POST /scrape`) returns the COMPLETE slice-06
/// `/scrape` page with the candidates below the form (I-HX-1).
///
/// Given JavaScript is disabled;
/// When Maria submits the scrape form WITHOUT the header (plain `POST /scrape`);
/// Then the server returns the complete `/scrape` page with the candidates below the
/// form.
///
/// @us-hx-003 @driving_port @real-io @no-js @full-page @edge
#[test]
fn submitting_scrape_without_htmx_returns_the_full_page() {
    // GIVEN the REUSED FakeGithub serving a public repo with harvestable signals.
    // WHEN Maria submits WITHOUT the header (the unchanged post_form).
    // THEN the response is the COMPLETE slice-06 `/scrape` full page
    // (`is_full_page()`) with the candidates (the same subject + derived-from)
    // rendered below the form.
    todo!(
        "DELIVER H-3d: github = for_public_repo(\"rust-lang/cargo\", fixture_cargo_five_signals()); \
         full = viewer.post_form(\"/scrape\", &[(\"target\", \"rust-lang/cargo\")]); \
         assert full.status==200, full.is_full_page(), \
         full.body_contains(\"rust-lang/cargo\"), full.body_contains(\"derived-from\")"
    );
}

/// H-3e (US-HX-003; AC parity, I-HX-5): the scrape-results fragment equals the
/// `#scrape-results` region of the full `/scrape` page — same candidates, same
/// derived-from, same verbatim confidence. The full page composes the same fragment
/// fn (ADR-032).
///
/// Given a target that would propose candidates;
/// When `POST /scrape` is submitted both WITH and WITHOUT the header;
/// Then the fragment's candidates + derived-from + confidence are contained in the
/// full page's results region.
///
/// @us-hx-003 @driving_port @real-io @parity @derived-from @i-hx-5 @happy
#[test]
fn scrape_results_fragment_equals_the_full_page_results_region() {
    // GIVEN the REUSED FakeGithub serving a public repo with harvestable signals.
    // WHEN both shapes of `POST /scrape` are submitted (post_form_htmx + post_form).
    // THEN a known candidate's subject + "derived-from" + verbatim confidence appear
    // in BOTH the fragment and the full page (parity); the fragment is a fragment
    // and the full page is a full page.
    todo!(
        "DELIVER H-3e: github = for_public_repo(\"rust-lang/cargo\", fixture_cargo_five_signals()); \
         frag = viewer.post_form_htmx(\"/scrape\", &[(\"target\",\"rust-lang/cargo\")]); \
         full = viewer.post_form(\"/scrape\", &[(\"target\",\"rust-lang/cargo\")]); \
         for needle in [\"rust-lang/cargo\", \"derived-from\"]: assert \
         frag.body_contains(needle) && full.body_contains(needle); \
         assert frag.is_fragment() && full.is_full_page()"
    );
}

// =============================================================================
// US-HX-004 — Claim detail loads inline (H-4)
// =============================================================================
//
// GET /claims/{cid}. Swap target: #claim-detail. The shape fork is AFTER the
// found/not-found decision (DESIGN component-boundaries) — 404 status carries
// through BOTH shapes for an unknown CID.

/// H-4a (US-HX-004 happy; AC htmx-request fragment): WITH `HX-Request`, opening a
/// claim returns ONLY the `#claim-detail` region — all fields + the complete
/// evidence[] + confidence VERBATIM (0.90) — and NOT a full page. The claims list
/// stays in place (browser-side; the server returns just the detail region).
///
/// Given Maria's claim has two evidence URLs;
/// When she opens that claim WITH the `HX-Request` header;
/// Then the detail region shows all claim fields and both evidence URLs, confidence
/// verbatim 0.90, and the response is a fragment.
///
/// @us-hx-004 @driving_port @real-io @htmx-fragment @happy
#[test]
fn opening_a_claim_with_htmx_returns_only_the_detail_fragment() {
    // GIVEN Maria has signed a claim WITH two evidence URLs through the production
    // `claim add` path (its CID addresses the detail route).
    // WHEN she opens `/claims/{cid}` WITH the header (get_htmx).
    // THEN the response is ONLY the `#claim-detail` fragment: it shows all fields +
    // both evidence URLs + verbatim 0.90, and is a fragment (no full-page chrome).
    todo!(
        "DELIVER H-4a: cid = seed_own_claim_with_evidence(&env, \"rust-lang/rust\", \
         \"is-maintained-by\", \"The Rust Project\", 0.90, &[ev1, ev2]); \
         frag = viewer.get_htmx(&format!(\"/claims/{{cid}}\")); assert frag.status==200, \
         frag.is_fragment(), frag.body_contains(\"0.90\"), frag.body_contains(ev1), \
         frag.body_contains(ev2)"
    );
}

/// H-4b (US-HX-004 edge; AC no-JS full-page fallback): opening `/claims/{cid}`
/// WITHOUT `HX-Request` (direct URL / bookmark / JS off) returns the COMPLETE
/// slice-06 detail page (full-page chrome + the detail region) (I-HX-1).
///
/// Given JavaScript is disabled (or the URL is opened directly);
/// When Maria opens `/claims/{cid}` WITHOUT the header;
/// Then the server returns the complete slice-06 claim detail page.
///
/// @us-hx-004 @driving_port @real-io @no-js @full-page @edge
#[test]
fn opening_a_claim_without_htmx_returns_the_full_detail_page() {
    // GIVEN a seeded claim with evidence.
    // WHEN she opens `/claims/{cid}` WITHOUT the header (the unchanged get).
    // THEN the response is the COMPLETE slice-06 detail full page (`is_full_page()`)
    // with the same detail region (all fields + evidence).
    todo!(
        "DELIVER H-4b: cid = seed_own_claim_with_evidence(...); \
         full = viewer.get(&format!(\"/claims/{{cid}}\")); assert full.status==200, \
         full.is_full_page(), full.body_contains(\"The Rust Project\")"
    );
}

/// H-4c (US-HX-004 error; AC unknown CID in both shapes): an unknown CID returns the
/// guided not-found region ("No claim with that identifier in your store") with a
/// back link in BOTH the fragment AND the full page, with the 404 status carried
/// through both (the shape fork is AFTER the not-found decision).
///
/// Given no claim with the requested CID exists in the store;
/// When Maria opens that claim WITH or WITHOUT the `HX-Request` header;
/// Then she sees "No claim with that identifier in your store" with a back link in
/// both shapes.
///
/// @us-hx-004 @driving_port @real-io @htmx-fragment @no-js @error
#[test]
fn unknown_cid_guides_the_operator_in_both_shapes() {
    // GIVEN a store with at least one real claim, but NOT the mistyped CID.
    // WHEN Maria opens the unknown CID WITH (get_htmx) and WITHOUT (get) the header.
    // THEN the fragment shows the guided not-found message + a `/claims` back link
    // and is a fragment; the full page shows the SAME guided message + back link and
    // is a full page; the not-found status (404) carries through both shapes.
    todo!(
        "DELIVER H-4c: seed one real claim; frag = viewer.get_htmx(\"/claims/bafyrei-zzz\"); \
         full = viewer.get(\"/claims/bafyrei-zzz\"); for r in [&frag,&full]: assert \
         r.body_contains(\"No claim with that identifier in your store\") && \
         r.body_contains(\"/claims\"); assert frag.is_fragment() && full.is_full_page()"
    );
}

/// H-4d (US-HX-004 edge; AC no evidence in both shapes): a claim signed WITHOUT
/// evidence shows the explicit "no evidence attached" state in BOTH the fragment and
/// the full page — never a blank section.
///
/// Given Maria has a claim signed without evidence;
/// When she opens its detail WITH and WITHOUT the header;
/// Then she sees "no evidence attached" in both shapes.
///
/// @us-hx-004 @driving_port @real-io @htmx-fragment @no-js @empty-state @edge
#[test]
fn claim_with_no_evidence_renders_clearly_in_both_shapes() {
    // GIVEN a claim signed WITHOUT evidence (empty evidence[]).
    // WHEN she opens `/claims/{cid}` WITH (get_htmx) and WITHOUT (get) the header.
    // THEN both shapes show the explicit "no evidence attached" state; the fragment
    // is a fragment and the full page is a full page.
    todo!(
        "DELIVER H-4d: cid = seed_own_claim_with_evidence(&env, \"serde-rs/serde\", \
         \"is-maintained-by\", \"dtolnay\", 0.80, &[]); \
         frag = viewer.get_htmx(&format!(\"/claims/{{cid}}\")); \
         full = viewer.get(&format!(\"/claims/{{cid}}\")); for r in [&frag,&full]: \
         assert r.body_contains(\"no evidence attached\"); \
         assert frag.is_fragment() && full.is_full_page()"
    );
}

/// H-4e (US-HX-004; AC parity, I-HX-5): the detail fragment equals the
/// `#claim-detail` region of the full detail page — all fields + complete evidence[]
/// + verbatim confidence. The full page composes the same fragment fn (ADR-032).
///
/// Given Maria's claim has two evidence URLs;
/// When `/claims/{cid}` is fetched both WITH and WITHOUT the header;
/// Then the fragment's fields + evidence + verbatim confidence are contained in the
/// full page's detail region.
///
/// @us-hx-004 @driving_port @real-io @parity @i-hx-5 @happy
#[test]
fn claim_detail_fragment_equals_the_full_page_detail_region() {
    // GIVEN a seeded claim with two evidence URLs + verbatim 0.90.
    // WHEN both shapes of `/claims/{cid}` are fetched (get_htmx + get).
    // THEN the fields (subject/predicate/object), both evidence URLs, and the
    // verbatim 0.90 appear in BOTH the fragment and the full page (parity); the
    // fragment is a fragment and the full page is a full page.
    todo!(
        "DELIVER H-4e: cid = seed_own_claim_with_evidence(... 0.90, &[ev1, ev2]); \
         frag = viewer.get_htmx(&format!(\"/claims/{{cid}}\")); \
         full = viewer.get(&format!(\"/claims/{{cid}}\")); for needle in \
         [\"rust-lang/rust\", \"0.90\", ev1, ev2]: assert frag.body_contains(needle) \
         && full.body_contains(needle); assert frag.is_fragment() && full.is_full_page()"
    );
}

// =============================================================================
// US-HX-006 — Switch My Claims <-> Peer Claims in place (H-6)
// =============================================================================
//
// Tab switch GET /claims <-> GET /peer-claims. Swap target: #view-panel; the tab
// carries hx-push-url (ADR-034) so the active view is bookmarkable + Back works.
// hx-push-url is client-side (the HTTP harness can't run JS — ADR-035) — so we
// assert the SERVER contract: each URL serves the correct fragment under the header
// and the correct full page without it; reload/bookmark of the switched-to URL
// yields the full page for that view.

/// H-6a (US-HX-006 happy; AC htmx-request fragment): switching to Peer Claims WITH
/// `HX-Request` returns ONLY the active view-panel fragment for the Peer Claims list
/// — peer rows with their origin, separable from own claims — and NOT a full page.
/// (The browser URL update via hx-push-url is client-side; the server contract is
/// "GET /peer-claims under HX-Request returns the peer view-panel fragment".)
///
/// Given Maria is on the My Claims view with a federated peer set;
/// When she switches to Peer Claims WITH the `HX-Request` header (`GET /peer-claims`);
/// Then ONLY the view panel updates to the Peer Claims list showing each row's
/// origin, separable from her own claims, and the response is a fragment.
///
/// @us-hx-006 @driving_port @real-io @htmx-fragment @peer-origin @tab-switch @happy
#[test]
fn switching_to_peer_claims_with_htmx_returns_only_the_view_panel_fragment() {
    // GIVEN Maria has her OWN claim AND a federated peer set (the "mine vs
    // federated" contrast is load-bearing): own via the production `claim add`,
    // peer via the fast bulk `seed_cached_peer_claims`.
    // WHEN she switches to Peer Claims WITH the header (`viewer.get_htmx("/peer-claims")`).
    // THEN the response is ONLY the view-panel fragment for the Peer Claims list: it
    // carries the peer origin (the peer DID), and is a fragment (no full-page chrome).
    todo!(
        "DELIVER H-6a: seed_own_claim_with_evidence(...); \
         seed_cached_peer_claims(env, \"did:plc:peer-axum\", 120); \
         frag = viewer.get_htmx(\"/peer-claims\"); assert frag.status==200, \
         frag.is_fragment(), frag.body_contains(\"did:plc:peer-axum\")"
    );
}

/// H-6b (US-HX-006 edge; AC no-JS full-page fallback): the Peer Claims tab WITHOUT
/// `HX-Request` (plain link) returns the COMPLETE slice-06 `/peer-claims` page; and
/// symmetrically `GET /claims` WITHOUT the header returns the full My Claims page —
/// each URL serves the correct full page for that view (I-HX-1, no-JS tab nav).
///
/// Given JavaScript is disabled;
/// When Maria clicks the Peer Claims tab WITHOUT the header (plain link to
/// `/peer-claims`), and likewise the My Claims tab to `/claims`;
/// Then the server returns the complete slice-06 page for each view.
///
/// @us-hx-006 @driving_port @real-io @no-js @full-page @tab-switch @edge
#[test]
fn tab_switch_without_htmx_returns_the_full_page_per_url() {
    // GIVEN own + peer claims seeded.
    // WHEN each tab URL is fetched WITHOUT the header (the unchanged get): `/claims`
    // and `/peer-claims`.
    // THEN each response is the COMPLETE slice-06 full page for that view
    // (`is_full_page()`): My Claims renders the own claim; Peer Claims renders the
    // peer origin. The two URLs converge with the no-JS real-URL path (ADR-034).
    todo!(
        "DELIVER H-6b: seed own + seed_cached_peer_claims; \
         mine = viewer.get(\"/claims\"); peers = viewer.get(\"/peer-claims\"); \
         assert mine.is_full_page() && peers.is_full_page(); \
         assert peers.body_contains(\"did:plc:peer-axum\")"
    );
}

/// H-6c (US-HX-006 edge; AC bookmark/reload re-enters via the full page): reloading
/// (or opening a bookmark of) the switched-to `/peer-claims` URL yields the COMPLETE
/// slice-06 `/peer-claims` page — the htmx path and the no-JS real-URL path converge
/// on the SAME URL (ADR-034). A bookmark is a plain GET with no `HX-Request`.
///
/// Given Maria switched to Peer Claims and bookmarked the page;
/// When she later opens that bookmark (or reloads the URL — a plain GET);
/// Then she lands on the complete slice-06 `/peer-claims` page.
///
/// @us-hx-006 @driving_port @real-io @no-js @full-page @bookmark @edge
#[test]
fn bookmark_of_the_switched_view_re_enters_via_the_full_page() {
    // GIVEN a federated peer set (the switched-to view).
    // WHEN the switched-to `/peer-claims` URL is opened as a plain GET (bookmark /
    // reload — no `HX-Request` header).
    // THEN the response is the COMPLETE slice-06 `/peer-claims` full page
    // (`is_full_page()`) — the bookmark re-enters the real URL, not a stray fragment.
    todo!(
        "DELIVER H-6c: seed_cached_peer_claims(env, \"did:plc:peer-axum\", 120); \
         page = viewer.get(\"/peer-claims\"); assert page.status==200, \
         page.is_full_page(), page.body_contains(\"did:plc:peer-axum\")"
    );
}

/// H-6d (US-HX-006; AC parity, I-HX-5): the view-panel fragment for `/peer-claims`
/// equals the `#view-panel` region of the full `/peer-claims` page — same rows, same
/// peer origin. The full page composes the same fragment fn (ADR-032).
///
/// Given Maria has a federated peer set;
/// When `/peer-claims` is fetched both WITH and WITHOUT the header;
/// Then the fragment's rows + peer origin are contained in the full page's view-panel
/// region.
///
/// @us-hx-006 @driving_port @real-io @parity @peer-origin @i-hx-5 @happy
#[test]
fn view_panel_fragment_equals_the_full_page_view_panel_region() {
    // GIVEN 120 peer rows.
    // WHEN both shapes of `/peer-claims` are fetched (get_htmx + get).
    // THEN a known peer row's origin/subject appears in BOTH the fragment and the
    // full page (parity); the fragment is a fragment and the full page is a full page.
    todo!(
        "DELIVER H-6d: seed_cached_peer_claims(env, \"did:plc:peer-axum\", 120); \
         frag = viewer.get_htmx(\"/peer-claims\"); full = viewer.get(\"/peer-claims\"); \
         for needle in [\"did:plc:peer-axum\", a known subject]: assert \
         frag.body_contains(needle) && full.body_contains(needle); \
         assert frag.is_fragment() && full.is_full_page()"
    );
}
