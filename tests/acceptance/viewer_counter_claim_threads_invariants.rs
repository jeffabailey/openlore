//! Slice-11 acceptance — counter-claim-thread GOLD / guardrail invariants (the
//! cross-cutting I-CT-1/2/3/5/6 guardrails that must hold over the WHOLE counter-thread
//! detail surface, beyond any single story).
//!
//! These are the load-bearing, release-relevant guardrail gold tests for the
//! counter-thread DELTA — the BEHAVIORAL layer of the three-layer enforcement (type +
//! xtask `check-arch` are the other two, owned by DELIVER). They drive the REAL
//! `openlore ui` verb via the `ViewerServer` subprocess + in-test HTTP (with/without
//! `HX-Request`) over a REAL seeded LOCAL DuckDB, with NO mocked boundary (the 2-step
//! counter-thread read is a LOCAL DB-index lookup + a local artifact `fs::read` + a
//! PURE projection/render — distinct from `/search`, which mocks the indexer; and
//! OFFLINE-STRONGER than `/search` — this route has NO outbound edge at all). They
//! assert the hard slice-11 invariants on the OBSERVABLE surface:
//!
//! - `every_detail_route_with_counters_leaves_the_store_read_only` (CT-INV-ReadOnly,
//!   I-CT-1 / KPI-VIEW-2): exercising the detail route across postures (countered +
//!   un-countered) AND both shapes (full page + htmx fragment) leaves `claims` +
//!   `peer_claims` row counts UNCHANGED, asserted via the universe-bound
//!   `assert_store_read_only` (Mandate 8; universe = the two port-exposed counts, all
//!   `unchanged`). The thread is computed per query and persists nothing.
//! - `no_detail_response_with_counters_adds_a_write_or_sign_control` (CT-INV-NoWrite,
//!   I-CT-1): no detail response shape (full page or fragment, countered or not)
//!   renders a write / sign / counter / publish control — authoring stays the slice-03
//!   CLI; the counter CID drill-links are render-only `<a href>` navigation TEXT.
//! - `the_counter_thread_page_chrome_stays_offline_no_cdn` (CT-INV-OfflineChrome,
//!   I-CT-5 / KPI-HX-G2): the countered-claim detail full page references ONLY the
//!   LOCAL `/static/htmx.min.js` script src and NO off-host CDN.
//! - `the_counter_thread_renders_fully_offline` (CT-INV-Offline, I-CT-5 / KPI-5): the
//!   counter-thread renders fully with the network unavailable — the 2-step read (DB
//!   index + local artifact `fs::read`) is LOCAL with NO outbound edge to take down
//!   (offline-STRONGER than `/search`; peer counters were verified at `peer pull` time
//!   and the viewer re-verifies nothing — I-CT-5).
//! - `the_countered_claim_confidence_is_byte_identical_with_and_without_counters`
//!   (CT-INV-ShownNeverApplied, the shown-never-applied GOLD — I-CT-2 / OD-AV-7 /
//!   ADR-015): the SAME claim's rendered confidence/fields are byte-IDENTICAL whether
//!   or not it is countered — the counter never filters/merges/re-weights/re-ranks the
//!   claim. A shown-never-applied regression silently lets disagreement mutate the
//!   claim being read; this gold makes it unshippable.
//!
//! Driving discipline (Mandate 1): every assertion enters through the REAL `openlore
//! ui` subprocess + HTTP — never internal `viewer-domain` `render_*` fns. The local
//! DuckDB is REAL (seeded via the production `claim add` / `claim counter` / `peer add`
//! + `peer pull` paths); there is NO mocked boundary (the route is LOCAL).
//!
//! Layer placement (Mandate 9/11): layer-3/layer-5 subprocess + real-I/O, EXAMPLE-only.
//! These guardrails are example-based, never PBT-generated at this layer (the
//! `@property` tag marks them as universal invariants for the reader + the DELIVER
//! crafter; the generative exploration of the pure projection/render is a layer-1/2
//! concern in the DELIVER `viewer-domain` units, out of this file's scope).
//!
//! Build-before-run note: as with `viewer_counter_claim_threads.rs`, the run MUST
//! `cargo build` the `openlore` (viewer) bin before running these ATs. No second binary
//! is needed — the counter-thread read is a LOCAL read.
//!
//! Mandate 7 RED scaffolds: each body is `todo!()` (via the `todo!()`-stubbed seed /
//! assert helpers or directly) → panics → classifies RED (MISSING_FUNCTIONALITY), NOT
//! BROKEN. They stay RED until DELIVER.
//!
//! Covers: the cross-cutting I-CT-1 / I-CT-2 / I-CT-3 / I-CT-5 / I-CT-6 guardrails over
//! the whole counter-thread surface (the gold companions to the US-CT-002/003 story
//! scenarios in `viewer_counter_claim_threads.rs`).
//
// SCAFFOLD: true

mod support;

#[allow(unused_imports)]
use support::*;

// =============================================================================
// I-CT-1 / KPI-VIEW-2 — read-only preserved: the detail route with counters + every
// posture + shape leaves the store unchanged (CT-INV-ReadOnly). The thread is computed
// per query and persists nothing.
// =============================================================================

/// CT-INV-ReadOnly / GOLD `every_detail_route_with_counters_leaves_the_store_read_only`
/// (I-CT-1 / KPI-VIEW-2): exercising the detail route — countered AND un-countered, in
/// BOTH shapes (full page + htmx fragment) — leaves the `claims` + `peer_claims` row
/// counts UNCHANGED. The slice-11 companion to the slice-06 `viewer_is_read_only` +
/// slice-08/09/10 read-only gold tests, asserted via the universe-bound state-delta
/// (Mandate 8: universe = the two port-exposed counts, each `unchanged`). The
/// counter-thread is recomputed per query and NEVER persisted.
///
/// Given a store seeded with a countered claim;
/// When the detail route (countered + un-countered CID, full + fragment) is exercised;
/// Then the `claims` and `peer_claims` row counts are UNCHANGED.
///
/// @us-ct-002 @us-ct-003 @property @driving_port @real-io @read-only @i-ct-1 @gold
#[test]
fn every_detail_route_with_counters_leaves_the_store_read_only() {
    // GIVEN a REAL store seeded so the universe is NON-TRIVIAL: a countered claim (its
    // own counter via `claim counter` → `claims`; a peer counter via `peer pull` →
    // `peer_claims`), so the read-only delta is over a POPULATED store (a `0 == 0`
    // delta would not prove the viewer leaves a populated store untouched). Capture the
    // read-only universe (port-exposed counts) BEFORE exercising any route.
    // WHEN the detail route is exercised — the countered CID AND a non-existent CID,
    // both shapes (get + get_htmx) — inside a scope so the viewer's exclusive DuckDB
    // lock is RELEASED before the `after` snapshot.
    // THEN the persisted-store row counts are UNCHANGED (assert_store_read_only; any
    // change is an UNSHIPPABLE write-surface breach — I-CT-1).
    todo!(
        "DELIVER (CT-INV-ReadOnly): seed_claim_two_counters_distinct_authors (own + \
         peer counter so the universe is non-trivial across BOTH tables); capture \
         before = capture_store_row_count_universe(&env); in a scope, start \
         ViewerServer and GET the countered CID + a non-existent CID, each via get + \
         get_htmx (all 200/404 as appropriate); drop the viewer; after = \
         capture_store_row_count_universe(&env); assert_store_read_only(&before, \
         &after) (all slots unchanged; the thread persists nothing — I-CT-1)"
    );
}

// =============================================================================
// I-CT-1 — no write/sign/counter control on ANY detail response shape (CT-INV-NoWrite).
// The human gate stays in the CLI; counter CID drill-links are render-only.
// =============================================================================

/// CT-INV-NoWrite / GOLD `no_detail_response_with_counters_adds_a_write_or_sign_control`
/// (I-CT-1): NO detail response shape (full page or fragment, countered or not) renders
/// a write / sign / counter / publish control — authoring stays EXCLUSIVELY in the
/// slice-03 CLI, and every counter CID drill-link is render-only navigation TEXT (an
/// `<a href>` anchor), never a control. Asserted on the observable rendered surface
/// across every shape.
///
/// Given the viewer renders a countered claim and an un-countered claim;
/// When every detail response shape (full page + fragment, both postures) is inspected;
/// Then none renders a write / sign / counter / publish control, and any
///   `/claims/{cid}` drill-link present is render-only `<a href>` navigation TEXT.
///
/// @us-ct-002 @us-ct-003 @property @driving_port @real-io @read-only @i-ct-1 @gold
#[test]
fn no_detail_response_with_counters_adds_a_write_or_sign_control() {
    // GIVEN a store seeded with a countered claim AND an un-countered claim + the
    // viewer rendering both in BOTH shapes.
    // WHEN each shape (get full page + get_htmx fragment) of each posture is inspected.
    // THEN none carries a write/sign/counter/publish affordance
    // (assert_detail_html_has_no_write_or_sign_control over EVERY shape × posture;
    // I-CT-1), AND any counter CID drill-link present is render-only navigation TEXT —
    // an `<a href>` anchor, never an executable write/sign/counter control. The viewer
    // holds no key (the no-key audit is structural — xtask check-arch).
    todo!(
        "DELIVER (CT-INV-NoWrite): seed a countered claim (seed_claim_with_counter) AND \
         an un-countered claim (seed_uncountered_claim); collect every detail response \
         shape (countered + un-countered, get + get_htmx) in a scope; for each, assert \
         status renders content + assert_detail_html_has_no_write_or_sign_control(&body) \
         (no sign/publish/counter/subscribe control) + any '/claims/' drill-link is \
         wrapped in an `<a href` anchor (render-only navigation TEXT, never a control — \
         I-CT-1)"
    );
}

// =============================================================================
// I-CT-5 / KPI-HX-G2 — offline chrome: the countered-claim detail page references only
// the local vendored htmx asset, no CDN (CT-INV-OfflineChrome).
// =============================================================================

/// CT-INV-OfflineChrome / GOLD `the_counter_thread_page_chrome_stays_offline_no_cdn`
/// (I-CT-5 / KPI-HX-G2): the countered-claim detail full page references ONLY the LOCAL
/// `/static/htmx.min.js` script src and NO off-host CDN — the page CHROME stays
/// offline-capable (and so does the THREAD itself, since the 2-step read is LOCAL —
/// even stronger than `/search`).
///
/// Given the viewer renders the countered-claim detail full page;
/// When the page's script references are inspected;
/// Then the only htmx asset reference is the local /static/htmx.min.js — no CDN.
///
/// @us-ct-002 @property @driving_port @real-io @offline @no-cdn @i-ct-5 @gold
#[test]
fn the_counter_thread_page_chrome_stays_offline_no_cdn() {
    // GIVEN a countered claim + the viewer rendering its detail full page.
    // WHEN the page's script references are inspected (both shapes).
    // THEN `references_external_cdn()` is FALSE for both (the only htmx asset is the
    // local /static/htmx.min.js; I-CT-5 / KPI-HX-G2). NO network seam is wired (plain
    // `ViewerServer::start`): the 2-step read is LOCAL, so the page CHROME and the
    // THREAD itself are both offline-capable.
    todo!(
        "DELIVER (CT-INV-OfflineChrome): seed_claim_with_counter; start ViewerServer; \
         GET /claims/{{target_cid}} via get (full page) + get_htmx (fragment); assert \
         the full page is_full_page() + the fragment is_fragment() + both return 200 + \
         !full_page.references_external_cdn() && !fragment.references_external_cdn() \
         (only the local /static/htmx.min.js — I-CT-5 / KPI-HX-G2)"
    );
}

// =============================================================================
// I-CT-5 / KPI-5 — local-first / offline: the counter-thread renders with the network
// unavailable (CT-INV-Offline). The 2-step read is LOCAL with NO outbound edge.
// =============================================================================

/// CT-INV-Offline / GOLD `the_counter_thread_renders_fully_offline` (I-CT-5 / KPI-5):
/// the countered-claim detail renders fully with NO network available — the 2-step read
/// (the INDEXED DB ref lookup + a local artifact `fs::read` for each counter's reason)
/// is LOCAL, so the network being down NEVER degrades it. Peer counters were
/// signature-verified at `peer pull` time and the viewer re-verifies nothing (no second
/// verification path); there is NO outbound edge on this route to take down
/// (offline-STRONGER than `/search`, I-CT-5).
///
/// Given the viewer is started over a seeded store with NO network seam wired, and the
///   network is disabled;
/// When a countered claim (with a PEER counter, already pulled) is opened;
/// Then the full counter-thread renders (the peer counter's author DID + CID + reason),
///   with no Unavailable/degraded state and no network call.
///
/// @us-ct-002 @property @driving_port @real-io @offline @local-first @i-ct-5 @kpi-5
/// @gold
#[test]
fn the_counter_thread_renders_fully_offline() {
    // GIVEN `ViewerServer::start(&env)` — the store-only posture with NEITHER the
    // /scrape GitHub seam NOR the /search indexer seam wired (the LOCAL-only viewer) —
    // over a store seeded with a countered claim whose counter is a PULLED PEER record
    // (verified at pull time). WHEN the claim detail is opened. THEN the full
    // counter-thread renders (the peer counter's author DID + CID + reason) with NO
    // Unavailable/degraded notice and NO network call — proving the 2-step read is
    // LOCAL + offline by construction (I-CT-5; the viewer re-verifies nothing).
    todo!(
        "DELIVER (CT-INV-Offline): seed_claim_with_counter so the counter is a PULLED \
         PEER record (verified at pull time); start the plain ViewerServer (no /scrape, \
         no /search seam); GET /claims/{{target_cid}} via get + get_htmx; assert both \
         200 + the body carries the peer counter's author DID + CID + verbatim reason + \
         NO 'unavailable'/'network error'/'could not reach'/'try again' degraded notice \
         (the 2-step read is LOCAL — I-CT-5 / KPI-5)"
    );
}

// =============================================================================
// I-CT-2 / OD-AV-7 / ADR-015 — shown-never-applied: the countered claim's confidence is
// byte-identical with and without counters (CT-INV-ShownNeverApplied). THE GOLD.
// =============================================================================

/// CT-INV-ShownNeverApplied / SHOWN-NEVER-APPLIED GOLD
/// `the_countered_claim_confidence_is_byte_identical_with_and_without_counters` (I-CT-2
/// / OD-AV-7 / ADR-015): the SAME claim's rendered confidence AND fields are
/// byte-IDENTICAL whether or not it is countered — the counter never filters, merges,
/// down-weights, or re-ranks the claim. This is the load-bearing slice-11 invariant:
/// disagreement is additive context BELOW the claim and changes NOTHING above. A
/// shown-never-applied regression silently lets a counter mutate the claim being read
/// (a re-weight, a filter, a "net" score); this gold makes it unshippable. Asserted on
/// the OBSERVABLE rendered HTML across BOTH the un-countered and countered renders.
///
/// Given the SAME claim (subject/predicate/object/confidence 0.91) is rendered once
///   un-countered and once countered;
/// When both detail pages render;
/// Then the claim's confidence (0.91) and fields are byte-identical in both — the
///   counter changed nothing above the thread.
///
/// @us-ct-002 @us-ct-003 @property @driving_port @real-io @shown-never-applied @i-ct-2
/// @i-ct-4 @gold
#[test]
fn the_countered_claim_confidence_is_byte_identical_with_and_without_counters() {
    // GIVEN two REAL stores: one where the target claim (confidence 0.91) is
    // UN-countered, one where the SAME claim is countered (own + peer counters).
    // WHEN each claim's detail renders (the SAME subject/predicate/object/confidence).
    // THEN the countered render shows the claim's confidence VERBATIM (0.91) AND the
    // claim's fields are byte-IDENTICAL to the un-countered render — the counter is
    // additive context BELOW, never a re-weight ABOVE
    // (assert_counter_claim_verbatim_unchanged over both renders; I-CT-2 / I-CT-4). Any
    // divergence in the claim region is an UNSHIPPABLE shown-never-applied breach.
    todo!(
        "DELIVER (CT-INV-ShownNeverApplied gold): render the SAME claim (confidence \
         0.91) un-countered (seed_uncountered_claim) and countered \
         (seed_claim_with_counter); assert via assert_counter_claim_verbatim_unchanged \
         that the countered render shows '0.91' verbatim AND the claim region is \
         byte-identical to the un-countered render — the counter never \
         re-weights/filters/merges/re-ranks the claim (I-CT-2 / OD-AV-7 / ADR-015)"
    );
}
