//! Slice-08 acceptance — network-search GOLD / guardrail invariants (the
//! cross-cutting I-NS-1/4/7/8 guardrails that must hold over the WHOLE `/search`
//! surface, beyond any single story).
//!
//! These are the load-bearing, release-relevant guardrail gold tests for the
//! network-search DELTA — the BEHAVIORAL layer of the three-layer enforcement (type
//! + xtask `check-arch` are the other two, owned by DELIVER). They drive the REAL
//! `openlore ui` verb via the `ViewerServer` subprocess + in-test HTTP (with/without
//! `HX-Request`) over a REAL DuckDB, with the network index as the ONLY mocked
//! boundary (a REAL slice-05 `openlore-indexer serve` over a seeded corpus), and
//! assert the hard slice-08 invariants on the OBSERVABLE surface:
//!
//! - `every_search_route_leaves_the_store_read_only` (N-INV-ReadOnly, I-NS-8 /
//!   WD-NS-7 / KPI-VIEW-2): exercising EVERY `/search` route across EVERY dimension
//!   (object / contributor / subject) AND both shapes (full page + htmx fragment),
//!   over a reachable index, leaves `claims` + `peer_claims` row counts UNCHANGED —
//!   asserted via the universe-bound `assert_store_read_only` (Mandate 8; universe =
//!   the two port-exposed counts, all `unchanged`). The network READ persists
//!   nothing (results computed per query).
//! - `no_search_response_adds_a_write_or_sign_control` (N-INV-NoWrite, I-NS-1 /
//!   WD-NS-3 / I-SCR-1): no `/search` response shape (full page or fragment, over a
//!   reachable index) renders a sign / publish / subscribe / executable-follow
//!   control — the human gate stays in the CLI; the follow affordance is render-only
//!   TEXT (asserted in the N-17 story scenario). Asserted on the observable rendered
//!   surface across every shape.
//! - `the_search_page_chrome_stays_offline_no_cdn` (N-INV-OfflineChrome, I-NS-7 /
//!   KPI-HX-G2): the `/search` full page references ONLY the LOCAL
//!   `/static/htmx.min.js` script src and NO off-host CDN — the page CHROME stays
//!   offline-capable even though the SEARCH itself needs the network (exactly like
//!   `/scrape`).
//! - `every_rendered_search_row_is_verified_by_construction` (N-INV-Verified,
//!   I-NS-4 / KPI-AV-3): across the dimension surface (object / contributor /
//!   subject) over a reachable index, EVERY rendered result row carries `[verified]`
//!   + an author DID and NO `[unverified]` / "unknown signature" state ever appears —
//!   verified-by-construction (the indexer is the verify gate; the viewer has no
//!   second verification path).
//!
//! Driving discipline (Mandate 1): every assertion enters through the REAL `openlore
//! ui` subprocess + HTTP — never internal `viewer-domain` functions. The local
//! DuckDB is REAL; the network index is the REUSED slice-05 `openlore-indexer serve`
//! (the ONLY mocked boundary, via `seed_network_index` →
//! `ViewerServer::start_with_indexer`).
//!
//! Layer placement (Mandate 9/11): layer-3/layer-5 subprocess + real-I/O,
//! EXAMPLE-only. These guardrails are example-based, never PBT-generated at this
//! layer (the `@property` tag marks them as universal invariants for the reader +
//! the DELIVER crafter; the generative exploration of the pure render/compose core
//! is a layer-1/2 concern, out of this file's scope).
//!
//! Build-before-run note: as with `viewer_network_search.rs`, the run MUST `cargo
//! build` BOTH the `openlore` (viewer) and `openlore-indexer` (seeded serve) bins
//! before running these ATs.
//!
//! Mandate 7 RED scaffolds: each body is `todo!()` → panics → classifies RED
//! (MISSING_FUNCTIONALITY), NOT BROKEN. They stay RED until DELIVER.
//!
//! Covers: the cross-cutting I-NS-1 / I-NS-4 / I-NS-7 / I-NS-8 guardrails over the
//! whole `/search` surface (the gold companions to the US-NS-001..004 story
//! scenarios in `viewer_network_search.rs`).
//
// SCAFFOLD: true

mod support;

#[allow(unused_imports)]
use support::*;

// =============================================================================
// I-NS-8 / WD-NS-7 — read-only preserved: every /search route + dimension + shape
// leaves the store unchanged (N-INV-ReadOnly). The network READ persists nothing.
// =============================================================================

/// N-INV-ReadOnly / GOLD `every_search_route_leaves_the_store_read_only` (I-NS-8 /
/// WD-NS-7 / KPI-VIEW-2): exercising EVERY `/search` route — every dimension
/// (object / contributor / subject) in BOTH shapes (full page + htmx fragment), over
/// a REACHABLE index — leaves the `claims` + `peer_claims` row counts UNCHANGED. The
/// network-search companion to the slice-06 `viewer_is_read_only` + slice-07
/// `htmx_fragment_routes_leave_the_store_read_only` gold tests, asserted via the
/// universe-bound state-delta (Mandate 8: universe = the two port-exposed counts,
/// each `unchanged`). Results are computed per query and never persisted.
///
/// Given a store seeded with own claims and a reachable network index;
/// When EVERY /search route (object/contributor/subject, full + fragment) is exercised;
/// Then the `claims` and `peer_claims` row counts are UNCHANGED.
///
/// @us-ns-001 @us-ns-002 @us-ns-003 @property @driving_port @real-io @read-only
/// @i-ns-8 @gold
#[test]
fn every_search_route_leaves_the_store_read_only() {
    // GIVEN a REAL store seeded (production write path) with own claims so the
    // read-only delta is over a non-trivial universe, PLUS a REAL `openlore-indexer
    // serve` over a corpus that has matches across all three dimensions (the ONLY
    // mocked boundary). Capture the read-only universe (port-exposed counts:
    // `claims.row_count`, `peer_claims.row_count`) BEFORE exercising any /search route.
    // WHEN every /search route is exercised — every dimension, both shapes
    // (get + get_htmx) — within a scope so the viewer's exclusive DuckDB lock is
    // released before the `after` snapshot (the read-only proof is about what the
    // viewer LEFT BEHIND, mirroring V-INV-1).
    // THEN the persisted-store row counts are UNCHANGED (assert_store_read_only;
    // any change is an UNSHIPPABLE write-surface breach — I-NS-8 / WD-NS-7).
    todo!(
        "DELIVER N-INV-ReadOnly: seed own claims; seed_network_index(a corpus with \
         object+contributor+subject matches); before = \
         capture_store_row_count_universe(env); {{ viewer = \
         start_with_indexer(env, indexer); exercise get + get_htmx for \
         /search?object=..., /search?contributor=..., /search?subject=... }}; after = \
         capture_store_row_count_universe(env); assert_store_read_only(&before, &after)"
    )
}

// =============================================================================
// I-NS-1 / WD-NS-3 / I-SCR-1 — no /search response adds a write/sign/follow surface
// (N-INV-NoWrite). The viewer stays read-only; following stays a CLI action.
// =============================================================================

/// N-INV-NoWrite / GOLD `no_search_response_adds_a_write_or_sign_control` (I-NS-1 /
/// WD-NS-3 / I-SCR-1): no `/search` response shape — full page OR htmx fragment,
/// across every dimension over a reachable index — renders a sign / publish /
/// subscribe / executable-follow control. The human gate stays in the CLI; the only
/// follow affordance on the surface is render-only `openlore peer add <did>` TEXT
/// (the executable-control ABSENCE is asserted here; the TEXT PRESENCE is the N-17
/// story scenario). The network-search companion to the slice-06 V-INV-4 / slice-07
/// `no_swap_route_adds_a_write_or_sign_surface` gold tests.
///
/// Given the viewer serves the /search routes over a reachable index;
/// When every /search response shape is requested;
/// Then no response renders a sign / publish / subscribe / executable-follow control.
///
/// @us-ns-001 @us-ns-004 @property @driving_port @real-io @read-only @i-ns-1
/// @i-scr-1 @gold
#[test]
fn no_search_response_adds_a_write_or_sign_control() {
    // GIVEN a reachable index over a corpus with matches + the viewer.
    // WHEN every /search response shape is requested (full page + fragment, across
    // the dimensions).
    // THEN NO response renders a sign/publish/subscribe/executable-follow control —
    // scan for `name="sign"`, `value="sign"`, `Sign claim`, `Sign & publish`,
    // `Publish claim`, `name="follow"`, `Subscribe`, and an `hx-post`/`<form>`
    // "follow"/"subscribe" affordance — across EVERY response. Any hit is an
    // UNSHIPPABLE write/sign/follow-surface breach (I-NS-1 / WD-NS-3). The render-
    // only `openlore peer add <did>` TEXT is NOT a control (it carries no
    // form/button/hx-* attribute) and is the N-17 story's PRESENCE assertion.
    todo!(
        "DELIVER N-INV-NoWrite: reachable index; collect full-page + fragment \
         responses across object/contributor/subject; assert NONE contains a \
         sign/publish/subscribe/executable-follow control marker (name=\\\"sign\\\", \
         value=\\\"sign\\\", Sign claim, Sign & publish, Publish claim, name=\\\"follow\\\", \
         Subscribe button, hx-post follow)"
    )
}

// =============================================================================
// I-NS-7 / KPI-HX-G2 — the /search page chrome stays offline (no CDN), even though
// the search itself needs the network (N-INV-OfflineChrome).
// =============================================================================

/// N-INV-OfflineChrome / GOLD `the_search_page_chrome_stays_offline_no_cdn` (I-NS-7 /
/// KPI-HX-G2 / BR-HX-6): the `/search` FULL PAGE references ONLY the LOCAL
/// `/static/htmx.min.js` script src and NO off-host CDN — the page chrome stays
/// offline-capable even though the SEARCH itself needs the network (exactly like the
/// slice-06 `/scrape` page). The network-search companion to the slice-07 H-5b
/// `no_viewer_page_references_an_external_cdn` gold gate.
///
/// Given the viewer serves the /search page;
/// When its served HTML is inspected (view-source);
/// Then it references NO external CDN and references the local `/static/htmx.min.js`.
///
/// @us-ns-004 @property @driving_port @real-io @offline @no-cdn @i-ns-7 @gold
#[test]
fn the_search_page_chrome_stays_offline_no_cdn() {
    // GIVEN the viewer running. The /search page chrome carries the script src
    // regardless of index reachability, so this holds even with an unconfigured
    // index — but use a reachable index so the Form+Results chrome is the
    // production shape.
    // WHEN the full-page `/search` (and a `/search?object=...` results page) HTML is
    // inspected (full pages, no header — the script src lives in the chrome).
    // THEN NO page references an external CDN (`references_external_cdn()` is false)
    // AND each carries the local `/static/htmx.min.js` script src (the offline
    // guarantee for the chrome; the search itself reaching the network is expected).
    todo!(
        "DELIVER N-INV-OfflineChrome: reachable index; for path in [\"/search\", \
         \"/search?object=...\"] {{ page = get(path); assert \
         !page.references_external_cdn() AND \
         page.body_contains(\"/static/htmx.min.js\") }}"
    )
}

// =============================================================================
// I-NS-4 / KPI-AV-3 — every rendered /search row is verified by construction
// (N-INV-Verified), across the whole dimension surface.
// =============================================================================

/// N-INV-Verified / GOLD `every_rendered_search_row_is_verified_by_construction`
/// (I-NS-4 / KPI-AV-3): across the dimension surface (object / contributor /
/// subject) over a reachable index, EVERY rendered result row carries `[verified]` +
/// an author DID, and NO `[unverified]` / "unknown signature" state EVER appears —
/// verified-by-construction (the indexer is the verify gate; the viewer has no
/// second verification path). The network-search companion to the slice-05
/// `assert_verified_marker_is_universal` gold discipline, on the browser surface.
///
/// Given a reachable index with verified claims across all three dimensions;
/// When each dimension's results render;
/// Then every rendered row carries `[verified]` + an author DID, and no row ever
///   shows `[unverified]` / "unknown signature".
///
/// @us-ns-002 @us-ns-003 @property @driving_port @real-io @verified @i-ns-4 @gold
#[test]
fn every_rendered_search_row_is_verified_by_construction() {
    // GIVEN a reachable index with verified claims matchable across object,
    // contributor, AND subject dimensions.
    // WHEN each dimension's results render (object / contributor / subject — full
    // page is sufficient; parity is the story-level concern).
    // THEN for each rendered results body,
    // assert_search_html_every_row_verified_and_attributed over that dimension's
    // expected author DIDs holds — every row [verified] + attributed, never an
    // unverified state (I-NS-4). This pins the universal verified-marker invariant
    // across the dimension surface (marked @property for the reader; example-pinned
    // at this layer per Mandate 9/11).
    todo!(
        "DELIVER N-INV-Verified: reachable index; for each dimension (object/\
         contributor/subject) get the results page and call \
         assert_search_html_every_row_verified_and_attributed(body, &[expected DIDs \
         for that dimension])"
    )
}
