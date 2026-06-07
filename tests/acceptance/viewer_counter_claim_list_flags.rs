//! Slice-12 acceptance — the `openlore ui` at-a-glance "Countered" PRESENCE FLAG on the
//! `GET /claims` LIST rows (US-LF-001/002/003; ADR-048).
//!
//! The EXISTING read-only `GET /claims` own-claims LIST (slice-06 V-1/11/12/13) is
//! EXTENDED so that each row whose claim has ≥1 counter carries a NEUTRAL "Countered"
//! presence marker — a render-only `<a href="/claims/{cid}">Countered</a>` one-hop link
//! to that claim's slice-11 thread. Un-countered rows carry NOTHING (no badge, no
//! "0 counters" noise). The flag is PRESENCE-ONLY (a boolean per row): a claim countered
//! by TWO distinct authors shows ONE neutral marker, never "disputed by 2", never a
//! count, never a verdict. The flag is ADDITIVE: it NEVER re-orders, re-ranks, filters,
//! re-weights, or re-paginates the list — row ORDER, PAGING, COUNT, and each row's
//! verbatim CONFIDENCE are byte-identical to the slice-06 `/claims` render of the same
//! store (shown-never-applied, I-LF-2).
//!
//! The AT-A-GLANCE / DISCOVERABILITY half of the VIEW side of J-003b (slice-11 shipped
//! the DRILL-IN thread; slice-12 makes disagreement DISCOVERABLE while scanning).
//! Authoring stays EXCLUSIVELY in the slice-03 CLI (`claim counter`); the viewer offers
//! NO write/sign/counter control on any surface (I-LF-1). slice-12 only RENDERS the
//! presence of counters that already exist; the marker LINKS to the slice-11 read-only
//! thread, never to a compose form.
//!
//! The load-bearing technical commitment (ADR-048 / I-LF-8): the per-CID counter-presence
//! lookup across the WHOLE list page is ONE aggregate `referenced_cid IN (...)` UNION-ALL
//! DISTINCT read over the indexed `claim_references ∪ peer_claim_references` tables —
//! NOT one query per row (no N+1). The presence read is read-only, LOCAL (renders
//! offline), ref-tables-only (no JOIN to `claims`/`peer_claims`, no per-row artifact
//! read — the flag carries no reason text), and returns the SET of countered CIDs for
//! the page (`HashSet<String>`, presence membership, anti-merging by type). Empty input
//! → empty set, zero queries.
//!
//! Driving discipline (Mandate 1): every scenario enters through the REAL `openlore ui`
//! subprocess (`ViewerServer`) + in-test HTTP GET (with/without the `HX-Request` header —
//! the slice-07 `get`/`get_htmx` pair) and asserts on the returned HTML. NO scenario
//! calls the `viewer-domain` `render_*` fns or `counter_presence_for` directly (those are
//! unit/property-level, exercised in DELIVER). The local DuckDB store is REAL, seeded
//! through the PRODUCTION write paths — the operator's OWN counter via the `claim counter`
//! verb and PEER counters via the `peer add` + `peer pull` federation path (Pillar 3 /
//! BR-VIEW-4) — reusing the slice-11 seeds (`seed_claim_with_counter` /
//! `seed_claim_two_counters_distinct_authors` / `seed_uncountered_claim`). The presence
//! read is LOCAL (DB index only); NO network seam exists on this route (offline by
//! construction, I-LF-5).
//!
//! Layer placement (nw-tdd-methodology Layered Test Discipline matrix + Mandate 9/11):
//! every scenario here is a layer-3/layer-5 subprocess + real-I/O test — EXAMPLE-only.
//! The sad/edge paths (none-countered, multi-counter, mixed page) are enumerated
//! explicitly, never PBT-generated at this layer. The strict 1-query N+1 bound is a
//! DELIVER unit/property assertion in `adapter-duckdb`; at this subprocess AT layer the
//! N+1 guard is asserted via its behavioral proxy (a page of many countered + uncountered
//! rows all flag correctly in ONE request, with no per-row degradation — LF-7).
//!
//! Build-before-run note (carry into the DELIVER roadmap, mirrors slice-06/07/11):
//! `cargo test` does NOT rebuild a spawned binary automatically — the roadmap/run MUST
//! `cargo build` the `openlore` (viewer) bin before running these ATs so
//! `ViewerServer::start` spawns the CURRENT viewer, not a stale one. The flag needs NO
//! second binary — the presence read is a LOCAL read.
//!
//! Mandate 7 RED scaffolds (ADR-025): the ATs spawn the bin + HTTP, so they COMPILE now
//! with `todo!()` bodies + the new `seed_claims_list_one_countered` /
//! `seed_claims_list_none_countered` / `seed_claims_list_mixed_pages` seeds +
//! `assert_list_row_flagged_countered` / `assert_list_row_not_flagged` /
//! `assert_list_order_and_confidence_byte_identical` / `assert_list_flag_links_to_thread`
//! / `assert_list_flag_is_single_neutral_presence` assert helpers (all `todo!()`-stubbed
//! in support/mod.rs — they compile, then panic). Each scenario body is `todo!()` →
//! panics → classifies RED (MISSING_FUNCTIONALITY), NOT BROKEN. They stay RED until
//! DELIVER's per-scenario RED→GREEN→COMMIT cycles.
//!
//! Covers:
//! - US-LF-002 (the flag, LF-1..LF-5): LF-1 walking skeleton — GET /claims WITH
//!   HX-Request over a store where one own claim is countered (a peer countered it) and
//!   others are not → 200, ONLY the list fragment, the countered row carries the neutral
//!   "Countered" marker linking to /claims/{cid}, the un-countered rows carry no marker +
//!   no-JS full-page parity (LF-2) + presence-only single neutral flag for a two-author
//!   claim (LF-4) + one-hop `<a href>` link to the slice-11 thread (LF-5).
//! - US-LF-003 (no-noise + no-regression, LF-6..LF-9): an un-countered row renders
//!   EXACTLY as slice-06 with no marker / no "0 counters" noise (LF-6); the
//!   shown-never-applied / no-regression GOLD — order, paging, count, and each row's
//!   confidence byte-identical to slice-06 (LF-7); a mixed page flags ONLY the countered
//!   rows in their unchanged composed_at DESC positions (LF-8); the N+1-guard behavioral
//!   proxy — a large mixed page flags every countered row correctly in ONE request (LF-9).
//
// SCAFFOLD: true

mod support;

#[allow(unused_imports)]
use support::*;

// =============================================================================
// US-LF-002 — see the neutral "Countered" presence flag on the /claims list rows
// (LF-1..LF-5). LF-1 is the thinnest end-to-end thread (the walking skeleton).
// =============================================================================

/// LF-1 / WALKING SKELETON (US-LF-002; the riskiest-assumption thread): from the LOCAL
/// store, `GET /claims` WITH the `HX-Request` header over a store where ONE of Maria's
/// own claims is countered (a peer authored a counter targeting it) and the others are
/// NOT returns ONLY the list-panel fragment (no full-page chrome) in which the countered
/// row carries the NEUTRAL "Countered" marker — a render-only `<a href="/claims/{cid}">`
/// link to that claim's slice-11 thread — while the un-countered rows carry NO marker.
/// This is the thinnest complete slice the feature can demo: viewer → LOCAL list read →
/// LOCAL batch presence read → pure projection → HTML fragment, proving the read-only
/// list can carry an at-a-glance disagreement flag while preserving the read-only /
/// presence-only / local-first / progressive-enhancement invariants.
///
/// Given Maria's read-only viewer reads a LOCAL store holding several of her own claims,
///   exactly one of which (`bafyMariaRust`) is countered by a peer;
/// When she opens the My Claims list WITH the htmx header (`GET /claims`, HX-Request);
/// Then she receives ONLY the list-panel fragment (no chrome) in which the
///   `bafyMariaRust` row carries the neutral "Countered" marker linking to
///   `/claims/bafyMariaRust`, and the un-countered rows carry no marker.
///
/// @us-lf-002 @walking_skeleton @driving_port @driving_adapter @real-io @htmx-fragment
/// @i-lf-2 @i-lf-3 @i-lf-6 @kpi-fed-3 @happy
#[test]
fn open_the_claims_list_with_htmx_flags_only_the_countered_row() {
    // GIVEN a REAL local store with several of Maria's own claims, exactly ONE of which
    // is countered by a peer (seeded via the slice-11 federation + `claim counter` paths,
    // widened to a multi-row list). The un-countered rows are plain own claims.
    //
    // WHEN Maria submits `GET /claims` WITH the HX-Request header (get_htmx).
    //
    // THEN the response is ONLY the list-panel fragment (`is_fragment()`, NOT a full
    // page): the countered row carries the neutral "Countered" marker linking to its
    // thread; every un-countered row carries NO marker.
    let env = TestEnv::initialized();
    let seeded = seed_claims_list_one_countered(&env);
    let server = ViewerServer::start(&env);

    let response = server.get_htmx("/claims");

    assert_eq!(
        response.status, 200,
        "GET /claims (HX-Request) must be 200; body was:\n{}",
        response.body
    );
    assert!(
        response.content_type.contains("text/html"),
        "GET /claims must serve text/html; got {:?}",
        response.content_type
    );
    assert!(
        response.is_fragment(),
        "GET /claims WITH HX-Request must return ONLY the list-panel fragment (no chrome); \
         body was:\n{}",
        response.body
    );

    // The countered row carries the neutral "Countered" marker linking to its thread.
    for countered in &seeded.countered_cids {
        assert_list_row_flagged_countered(&response.body, countered);
    }
    // Every un-countered row carries NO marker (and no empty-state noise).
    for uncountered in &seeded.uncountered_cids {
        assert_list_row_not_flagged(&response.body, uncountered);
    }
}

/// LF-2 (US-LF-002 — no-JS full page + parity, I-LF-6): `GET /claims` WITHOUT
/// `HX-Request` serves a COMPLETE full page (chrome + the SAME list region) whose list
/// region renders the SAME flags as the htmx fragment — parity by construction (the page
/// EMBEDS the same list-fragment fn; the flag is rendered INSIDE `render_claim_row`). The
/// no-JS no-regression contract: the full page is the contract, the htmx swap is a nicety.
///
/// Given Maria's store holds a countered claim among her own claims;
/// When the list renders as a full page (no JS) and as an htmx fragment;
/// Then the countered row shows the SAME "Countered" marker in both shapes, and the
///   un-countered rows carry no marker in either.
///
/// @us-lf-002 @driving_port @real-io @no-js @full-page @parity @i-lf-6 @happy
#[test]
fn the_list_flags_render_identically_under_htmx_and_no_js() {
    // GIVEN the same one-countered list store (one peer-countered own claim among plain
    // own claims).
    // WHEN Maria requests the list WITHOUT the htmx header (full page) AND WITH it
    // (fragment).
    // THEN the no-JS response is_full_page() (chrome present) while the htmx response
    // is_fragment() (no chrome); BOTH list regions carry the SAME flag on the countered
    // row and NO flag on the un-countered rows (parity — the page embeds the fragment
    // fn; I-LF-6).
    let env = TestEnv::initialized();
    let seeded = seed_claims_list_one_countered(&env);
    let server = ViewerServer::start(&env);

    // WHEN: the list renders as a full page (no JS, no HX-Request) AND as an htmx
    // fragment (HX-Request) over the SAME store.
    let full = server.get("/claims");
    let fragment = server.get_htmx("/claims");

    // Both are 200 text/html.
    for (label, response) in [("full page", &full), ("fragment", &fragment)] {
        assert_eq!(
            response.status, 200,
            "GET /claims ({label}) must be 200; body was:\n{}",
            response.body
        );
        assert!(
            response.content_type.contains("text/html"),
            "GET /claims ({label}) must serve text/html; got {:?}",
            response.content_type
        );
    }

    // The no-JS request is a COMPLETE full page (chrome present); the htmx request is
    // ONLY the swap-target fragment (no chrome).
    assert!(
        full.is_full_page(),
        "GET /claims WITHOUT HX-Request must return a COMPLETE full page (chrome present); \
         body was:\n{}",
        full.body
    );
    assert!(
        fragment.is_fragment(),
        "GET /claims WITH HX-Request must return ONLY the list-panel fragment (no chrome); \
         body was:\n{}",
        fragment.body
    );

    // THEN: BOTH shapes carry the SAME "Countered" marker on the countered row — parity
    // by construction (the full page EMBEDS the same fragment fn; the flag is rendered
    // INSIDE the row; I-LF-6).
    for countered in &seeded.countered_cids {
        assert_list_row_flagged_countered(&full.body, countered);
        assert_list_row_flagged_countered(&fragment.body, countered);
    }
    // And NEITHER shape flags any un-countered row.
    for uncountered in &seeded.uncountered_cids {
        assert_list_row_not_flagged(&full.body, uncountered);
        assert_list_row_not_flagged(&fragment.body, uncountered);
    }
}

/// LF-3 / GOLD presence-only (US-LF-002; I-LF-3 / KPI-AV-2): a claim countered by TWO
/// DISTINCT authors (Rachel + Tobias) shows EXACTLY ONE neutral "Countered" marker on its
/// list row — a PRESENCE marker, NEVER "disputed by 2", never a count, never a merged
/// verdict. The per-counter attribution (each author + CID + reason) lives in the
/// slice-11 thread the marker LINKS to, not on the list. Reuses the slice-11
/// `seed_claim_two_counters_distinct_authors` anti-merging fixture, observed on the LIST.
///
/// Given Maria's claim `bafyMariaTDD` is countered by both Rachel and Tobias;
/// When Maria opens the My Claims list;
/// Then the `bafyMariaTDD` row shows EXACTLY ONE neutral "Countered" marker, and the
///   list shows no count, no "disputed by N", and no aggregate verdict.
///
/// @us-lf-002 @driving_port @real-io @presence-only @anti-merging @i-lf-3 @kpi-av-2 @gold
#[test]
fn a_claim_with_two_counters_shows_one_neutral_presence_marker_on_the_list() {
    // GIVEN one of Maria's OWN claims countered by TWO DISTINCT authors (two distinct PEER
    // authors each author a `counters`-referencing record targeting the SAME own CID via
    // `peer pull` → `peer_claim_references`), among Maria's own claims on the list (the
    // slice-11 anti-merging fixture adapted into the LIST surface so the target appears on
    // /claims).
    // WHEN Maria opens the My Claims list.
    // THEN the countered row shows EXACTLY ONE neutral "Countered" marker (presence-only —
    // DISTINCT referenced_cid → one membership → one flag; I-LF-3), and the body carries
    // NO count / "disputed by N" / consensus / net-verdict phrasing
    // (assert_list_flag_is_single_neutral_presence).
    let env = TestEnv::initialized();
    let seeded = seed_claims_list_target_two_counters_distinct_authors(&env);
    let target_cid = seeded
        .countered_cids
        .first()
        .expect("LF-3: the seed yields exactly one twice-countered own target");
    let server = ViewerServer::start(&env);

    let response = server.get("/claims");

    assert_eq!(
        response.status, 200,
        "GET /claims must be 200; body was:\n{}",
        response.body
    );
    assert!(
        response.content_type.contains("text/html"),
        "GET /claims must serve text/html; got {:?}",
        response.content_type
    );

    // The twice-countered row shows EXACTLY ONE neutral "Countered" marker (presence-only —
    // DISTINCT referenced_cid collapses the two distinct-author counters to one membership),
    // and the body carries NO count / "disputed by N" / verdict phrasing (I-LF-3 / KPI-AV-2).
    assert_list_flag_is_single_neutral_presence(&response.body, target_cid);
    // The un-countered rows carry NO marker (the flag is presence-only + additive).
    for uncountered in &seeded.uncountered_cids {
        assert_list_row_not_flagged(&response.body, uncountered);
    }
}

/// LF-4 (US-LF-002 — one-hop link to the slice-11 thread, I-LF-6): the "Countered" marker
/// on a countered list row is a render-only `<a href="/claims/{cid}">` ONE-HOP link to
/// that claim's slice-11 counter thread — navigable WITHOUT JS (a plain anchor, never an
/// executable control). Following it lands on the slice-11 detail thread for that claim.
///
/// Given Maria's store holds her claim `bafyMariaRust` countered by Tobias;
/// When Maria opens the My Claims list and follows the "Countered" marker;
/// Then the marker is an `<a href="/claims/bafyMariaRust">` link, and following it shows
///   the slice-11 counter thread for that claim.
///
/// @us-lf-002 @driving_port @real-io @drill-link @one-hop @i-lf-6 @happy
#[test]
fn the_countered_marker_is_a_render_only_one_hop_link_to_the_thread() {
    // GIVEN a one-countered list store (the countered target has a known CID + a slice-11
    // thread).
    // WHEN Maria opens the My Claims list AND then follows the marker's href.
    // THEN (a) the marker on the countered row is a render-only
    // `<a href="/claims/{target_cid}">` anchor (navigation TEXT, never a control,
    // I-LF-1/I-LF-6 — assert_list_flag_links_to_thread); and (b) GET-ing that href shows
    // the slice-11 counter thread for the claim (the one-hop drill works without JS).
    let env = TestEnv::initialized();
    let seeded = seed_claims_list_one_countered(&env);
    let countered_cid = seeded
        .countered_cids
        .first()
        .expect("LF-4: the seed yields exactly one countered own target");
    let server = ViewerServer::start(&env);

    let list = server.get("/claims");
    assert_eq!(
        list.status, 200,
        "GET /claims must be 200; body was:\n{}",
        list.body
    );

    // (a) The marker on the countered row is the render-only one-hop anchor
    // `<a href="/claims/{cid}">Countered</a>` — navigation TEXT, never an executable
    // control (I-LF-1 / I-LF-6).
    assert_list_flag_links_to_thread(&list.body, countered_cid);

    // (b) Following that href (the one-hop drill, no JS) lands on the slice-11 counter
    // thread for the claim: a 200 detail page that IS the claim's thread (it carries the
    // claim's CID + the neutral "Countered" presence flag the slice-11 thread renders).
    let detail = server.get(&format!("/claims/{countered_cid}"));
    assert_eq!(
        detail.status, 200,
        "GET /claims/{countered_cid} (the one-hop drill target) must be 200; body was:\n{}",
        detail.body
    );
    assert!(
        detail.body.contains(countered_cid),
        "LF-4: the drilled-into detail page must be the thread for {countered_cid:?} (it \
         names the claim's CID); body was:\n{}",
        detail.body
    );
    // The detail page IS the slice-11 counter thread — it carries the neutral "Countered"
    // presence flag (and never a count/verdict), confirming the one-hop link reached the
    // thread, not a bare detail.
    assert_counter_thread_presence_flag_is_neutral(&detail.body);
}

// =============================================================================
// US-LF-003 — no-noise discipline + shown-never-applied / no-regression on the list
// (LF-5..LF-8). An un-countered row is byte-unaffected; the flag is purely additive.
// =============================================================================

/// LF-5 (US-LF-003 — no-noise discipline; I-LF-2): an UN-countered row renders EXACTLY as
/// the slice-06 `/claims` row today — NO "Countered" marker, and NO "0 counters" /
/// "no disagreement" empty-state noise. A store with NO counters renders the list
/// byte-identically to slice-06. `counter_presence_for` returns the EMPTY set → no row
/// is flagged → the list is unchanged. This is the byte-unaffected guarantee for the
/// common case (the no-noise half of the trust contract).
///
/// Given Maria's store holds her claims and NOTHING counters any of them;
/// When she opens the My Claims list;
/// Then every row renders as in slice-06, with no "Countered" marker and no empty-state
///   noise anywhere on the page.
///
/// @us-lf-002 @us-lf-003 @driving_port @real-io @no-noise @empty-set @i-lf-2 @happy
#[test]
fn a_store_with_no_counters_renders_the_list_exactly_as_slice_06() {
    // GIVEN an all-un-countered list store (seed_claims_list_none_countered — several
    // plain own claims, nothing references any of them as a counter →
    // `counter_presence_for` returns the EMPTY set).
    // WHEN Maria opens the My Claims list.
    // THEN every row renders as in slice-06 (V-1), the body carries NO "Countered" marker
    // and NO "0 counters" / "no disagreement" empty-state text
    // (assert_list_row_not_flagged for each cid + a body-wide no-noise scan).
    let env = TestEnv::initialized();
    let seeded = seed_claims_list_none_countered(&env);
    let server = ViewerServer::start(&env);

    let page = server.get("/claims");

    assert_eq!(
        page.status, 200,
        "GET /claims must be 200; body was:\n{}",
        page.body
    );
    assert!(
        page.content_type.contains("text/html"),
        "GET /claims must serve text/html; got {:?}",
        page.content_type
    );

    // Every row renders as in slice-06: NO "Countered" marker on any row, and the body
    // carries NO "0 counters" / "no disagreement" empty-state noise. assert_list_row_not_flagged
    // checks BOTH the per-CID marker absence AND the body-wide no-noise scan, so an empty
    // presence set leaves the list byte-unaffected (I-LF-2).
    for cid in &seeded.uncountered_cids {
        assert_list_row_not_flagged(&page.body, cid);
    }
    // Defensive whole-body scan: with NO counters, the "Countered" flag text appears NOWHERE
    // on the list page (the empty presence set renders nothing additive at all).
    assert!(
        !page.body.contains(LIST_COUNTERED_FLAG_TEXT),
        "LF-5 (no-noise): a store with NO counters must carry NO {LIST_COUNTERED_FLAG_TEXT:?} \
         marker anywhere on the list (empty presence set → nothing rendered; US-LF-003 / \
         I-LF-2); body was:\n{}",
        page.body
    );
}

/// LF-6 / GOLD shown-never-applied + no-regression (US-LF-003; I-LF-2 / OD-AV-7 /
/// ADR-015 / slice-11 I-CT-2): the flag NEVER re-orders, re-ranks, filters, or re-weights
/// the list — row ORDER, PAGING, COUNT, and each row's verbatim CONFIDENCE are
/// byte-identical to the slice-06 `/claims` render of the SAME store. The presence read
/// is a SEPARATE set lookup mapped onto rows AFTER `list_claims` pages them; the list SQL
/// (`ORDER BY composed_at DESC, cid LIMIT ? OFFSET ?` + its `COUNT(*)`) is UNTOUCHED. This
/// is the load-bearing slice-12 gold: the flag is additive only. A regression silently
/// lets the flag pick a triage order or re-score a claim; this gold makes it unshippable.
///
/// Given the SAME store is rendered once WITHOUT the flag feature (slice-06 baseline) and
///   once WITH it;
/// When both `/claims` lists render;
/// Then the row order (`composed_at DESC, cid`), the page boundaries / position
///   indicator, the total count, and EVERY row's confidence are byte-identical — the
///   flag changed nothing but the additive marker.
///
/// @us-lf-003 @driving_port @real-io @shown-never-applied @no-regression @i-lf-2 @i-lf-4
/// @gold
#[test]
fn the_flag_never_reorders_repages_recounts_or_reweights_the_list() {
    // GIVEN a mixed store (some countered, some not) rendered as the slice-06 baseline
    // list (no flag) AND as the slice-12 flagged list — the SAME store, so the ordering /
    // paging / count / confidence are directly comparable.
    // WHEN both `/claims` lists render.
    // THEN the row ORDER (composed_at DESC, cid), the position indicator / page
    // boundaries, the total COUNT, and EVERY row's verbatim confidence are
    // byte-IDENTICAL between the flagged and the un-flagged render — the flag is additive
    // only (assert_list_order_and_confidence_byte_identical; I-LF-2). Any divergence
    // (re-order, re-page, re-count, re-weight) is an UNSHIPPABLE no-regression breach.
    let env = TestEnv::initialized();
    // GIVEN a mixed store (known order/count; some own rows peer-countered, some not).
    let _seeded = seed_claims_list_mixed_pages(&env);

    // BASELINE-CAPTURE TACTIC (b) — the RECORDED slice-06 ordering. There is NO pre-flag
    // binary and NO no-flag HTTP render seam (the `/claims` route ALWAYS reads
    // `counter_presence_for`; adding a presence-suppression mode would be a production
    // test-seam, out of scope). Tactic (a)'s twin no-counter store is ALSO unusable: a
    // claim's CID canonicalizes its `composed_at` (claim-domain `canonicalize` →
    // `compute_cid`), so re-seeding the same claims at a different instant yields DIFFERENT
    // CIDs and the two renders could never be byte-identical. So the slice-06 reference is
    // the RECORDED order + total count + verbatim confidence read from the SAME `claims`
    // table in the SAME `composed_at DESC, cid` order the slice-06 list SQL uses
    // (`read_slice06_list_baseline`). The assert then ELIDES the additive
    // `<a href="/claims/{cid}">Countered</a>` anchors from the flagged render and proves the
    // remaining slice-06 body still honours that recorded order/count/paging/confidence
    // byte-for-byte — a non-circular gold: any re-order/re-page/re-count/re-weight survives
    // the additive-marker elision and FAILS.
    let baseline = read_slice06_list_baseline(&env);

    let server = ViewerServer::start(&env);

    // WHEN the slice-12 flagged `/claims` list renders.
    let flagged = server.get("/claims");
    assert_eq!(
        flagged.status, 200,
        "GET /claims must be 200; body was:\n{}",
        flagged.body
    );
    assert!(
        flagged.content_type.contains("text/html"),
        "GET /claims must serve text/html; got {:?}",
        flagged.content_type
    );

    // THEN with the additive "Countered" markers elided, the row ORDER (composed_at DESC,
    // cid), the position indicator / page boundaries, the total COUNT, and EVERY row's
    // verbatim CONFIDENCE are byte-IDENTICAL to the recorded slice-06 baseline — the flag is
    // additive ONLY (I-LF-2 / I-LF-4). Any divergence is an UNSHIPPABLE no-regression breach.
    assert_list_order_and_confidence_byte_identical(&flagged.body, &baseline);
}

/// LF-7 (US-LF-003 — mixed page, only countered rows flagged in unchanged order; I-LF-2):
/// on a page mixing countered + un-countered rows, ONLY the genuinely-countered rows
/// carry the "Countered" marker, and they appear in their ORIGINAL `composed_at DESC, cid`
/// positions — the flag does NOT pull the countered rows together or to the top. A
/// flagged claim's confidence renders verbatim (byte-identical to a no-flag render).
///
/// Given Maria's page is `bafyMariaSemver` (countered), `bafyMariaDoc` (un-countered),
///   `bafyMariaRust` (countered) in `composed_at DESC` order;
/// When Maria opens the My Claims list;
/// Then `bafyMariaSemver` and `bafyMariaRust` carry the marker, `bafyMariaDoc` does not,
///   the three appear in that SAME order, and each confidence renders verbatim.
///
/// @us-lf-003 @driving_port @real-io @mixed-page @shown-never-applied @i-lf-2 @i-lf-4
/// @happy
#[test]
fn a_mixed_page_flags_only_the_countered_rows_in_their_unchanged_positions() {
    // GIVEN a mixed page (countered, un-countered, countered in composed_at DESC order).
    // WHEN Maria opens the My Claims list.
    // THEN ONLY the countered rows carry the marker (assert_list_row_flagged_countered /
    // assert_list_row_not_flagged per row), they appear in the SAME composed_at DESC
    // order (NOT pulled together / to the top), and each row's confidence renders verbatim
    // (the flag re-weights nothing; I-LF-2 / I-LF-4).
    let env = TestEnv::initialized();
    let seeded = seed_claims_list_mixed_pages(&env);
    let server = ViewerServer::start(&env);

    let page = server.get("/claims");

    assert_eq!(
        page.status, 200,
        "GET /claims must be 200; body was:\n{}",
        page.body
    );
    assert!(
        page.content_type.contains("text/html"),
        "GET /claims must serve text/html; got {:?}",
        page.content_type
    );

    // ONLY the genuinely-countered rows carry the neutral "Countered" marker (one-hop
    // link to their slice-11 thread); EVERY un-countered row carries NO marker.
    for countered in &seeded.countered_cids {
        assert_list_row_flagged_countered(&page.body, countered);
    }
    for uncountered in &seeded.uncountered_cids {
        assert_list_row_not_flagged(&page.body, uncountered);
    }

    // The flag is ADDITIVE — it never moves/reorders a row. The rendered row order is the
    // UNCHANGED slice-06 `composed_at DESC, cid` order (`seeded.ordered_cids`): the flagged
    // and un-flagged rows INTERLEAVE in their natural positions, never grouped/pulled to the
    // top. Assert every CID appears EXACTLY once and that their first-seen byte offsets are
    // STRICTLY INCREASING in `ordered_cids` order (the flag is a per-row marker, never a
    // sort/group key; I-LF-2 / I-LF-4).
    let mut prev_offset: Option<usize> = None;
    for cid in &seeded.ordered_cids {
        let offset = page.body.find(cid.as_str()).unwrap_or_else(|| {
            panic!(
                "LF-7: every list row's CID must appear in the rendered body; {cid:?} was \
                 missing; body was:\n{}",
                page.body
            )
        });
        if let Some(prev) = prev_offset {
            assert!(
                offset > prev,
                "LF-7 (order unchanged): the rendered row order must follow the slice-06 \
                 `composed_at DESC, cid` order {:?} verbatim — the additive 'Countered' flag \
                 must NOT move/reorder/group any row; {cid:?} rendered out of position (the \
                 flag is a per-row marker, never a sort key; I-LF-2 / I-LF-4); body was:\n{}",
                seeded.ordered_cids, page.body
            );
        }
        prev_offset = Some(offset);
    }
}

/// LF-8 (US-LF-003 — N+1-guard behavioral proxy; I-LF-8 / ADR-048): a LARGE page mixing
/// MANY countered + un-countered rows flags EVERY countered row correctly in ONE request,
/// with no per-row degradation — the at-this-layer behavioral proxy for the single-query
/// guarantee (the strict 1-query bound is a DELIVER unit/property assertion in
/// `adapter-duckdb`; query count is not observable at the subprocess AT layer). If the
/// presence read were N+1, a large page would either degrade or mis-flag under the
/// per-row fan-out; this proxy pins that the whole page is flagged correctly in one shot.
///
/// Given Maria's page holds MANY claims, a known subset of which are countered;
/// When she opens the My Claims list (ONE request);
/// Then EVERY countered row carries the marker and EVERY un-countered row does not — the
///   whole page is flagged correctly in a single request (no per-row degradation).
///
/// @us-lf-001 @us-lf-003 @driving_port @real-io @n-plus-1-guard @i-lf-8 @gold
#[test]
fn a_large_mixed_page_flags_every_countered_row_correctly_in_one_request() {
    // GIVEN a LARGE mixed page (many own claims; a known subset peer-countered) — the
    // N+1-guard behavioral proxy fixture.
    // WHEN Maria opens the My Claims list (ONE GET request).
    // THEN every countered cid in the subset carries the marker and every other row does
    // NOT — the whole page is flagged correctly in a SINGLE request with no per-row
    // degradation (the subprocess-layer proxy for the ADR-048 single-query bound; the
    // strict 1-query assertion is a DELIVER adapter-duckdb unit/property test).
    let env = TestEnv::initialized();
    // GIVEN a LARGE mixed page (many own claims; a KNOWN subset peer-countered, the
    // countered rows interleaved among un-countered ones) — the N+1-guard behavioral
    // proxy fixture. The presence read for the WHOLE page is one aggregate
    // `referenced_cid IN (...)` read (ADR-048); if it were N+1, this large page would
    // either degrade or mis-flag under the per-row fan-out.
    let seeded = seed_claims_list_mixed_pages(&env);
    // Sanity: the proxy is only meaningful over a genuinely large mixed page with a
    // real countered subset AND un-countered rows. Pin both so the seed cannot silently
    // shrink the page (which would hollow out the N+1 proxy).
    assert!(
        !seeded.countered_cids.is_empty(),
        "LF-8: the large mixed page must carry a non-empty countered subset; got {:?}",
        seeded.countered_cids
    );
    assert!(
        !seeded.uncountered_cids.is_empty(),
        "LF-8: the large mixed page must carry un-countered rows too (a MIXED page); got {:?}",
        seeded.uncountered_cids
    );

    let server = ViewerServer::start(&env);

    // WHEN Maria opens the My Claims list — ONE GET request renders the whole page.
    let page = server.get("/claims");

    assert_eq!(
        page.status, 200,
        "GET /claims must be 200; body was:\n{}",
        page.body
    );
    assert!(
        page.content_type.contains("text/html"),
        "GET /claims must serve text/html; got {:?}",
        page.content_type
    );

    // THEN in that SINGLE response EVERY countered row carries the neutral "Countered"
    // marker and EVERY un-countered row carries NONE — the whole page is flagged
    // correctly in one request with no per-row degradation/fan-out (the subprocess-layer
    // behavioral proxy for the ADR-048 single-aggregate presence read; the strict
    // 1-query bound is the DELIVER adapter-duckdb unit/property assertion below).
    for countered in &seeded.countered_cids {
        assert_list_row_flagged_countered(&page.body, countered);
    }
    for uncountered in &seeded.uncountered_cids {
        assert_list_row_not_flagged(&page.body, uncountered);
    }
}
