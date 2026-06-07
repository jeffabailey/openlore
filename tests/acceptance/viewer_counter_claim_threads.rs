//! Slice-11 acceptance — the `openlore ui` COUNTER-CLAIM THREAD on the detail route
//! (US-CT-002/003; ADR-046/047).
//!
//! The EXISTING read-only `GET /claims/{cid}` detail route (slice-06 V-5/6/7) is
//! EXTENDED so that, BENEATH the original claim + its evidence, ALL counter-claims
//! targeting that CID render as a thread — each with its OWN author DID, its OWN CID
//! (linked to `/claims/{counter_cid}`), and its verbatim free-text `--reason`. The
//! original claim is rendered VERBATIM with its ORIGINAL confidence; the counters are
//! SHOWN, never applied (shown-never-applied, I-CT-2 / ADR-015). The read is the
//! 2-step decode-and-filter of ADR-046: the INDEXED `claim_references` /
//! `peer_claim_references` lookup by `referenced_cid` (UNION ALL, attributed, no
//! merge) for the counter CIDs + attribution + order, then a per-row `read_artifact_at`
//! for each counter's `reason` (the reason is NOT a DB column — it lives in the on-disk
//! `SignedClaim` artifact, ADR-015). The thread is DEPTH-1 (ADR-047): a counter's CID
//! is a render-only `<a href>` drill-link; there is NO nested/recursive counter render.
//! An un-countered claim renders EXACTLY as slice-06 today — no empty-thread noise.
//!
//! The VIEW half of J-003b. Authoring stays EXCLUSIVELY in the slice-03 CLI
//! (`claim counter --reason <R> <CID>`); the viewer offers NO write/sign/counter
//! control on any surface (I-CT-1). slice-11 only RENDERS counters that already exist.
//!
//! Driving discipline (Mandate 1): every scenario enters through the REAL `openlore
//! ui` subprocess (`ViewerServer`) + in-test HTTP GET (with/without the `HX-Request`
//! header — the slice-07 `get`/`get_htmx` pair) and asserts on the returned HTML. NO
//! scenario calls the `viewer-domain` `render_*` fns directly (those are unit/property-
//! level, exercised in DELIVER). The local DuckDB store is REAL, seeded through the
//! PRODUCTION write paths — the operator's OWN counter via the `claim counter` verb
//! (`seed_claim_with_counter` / `seed_claim_two_counters_distinct_authors`) and a
//! PEER's counter via the `peer add` + `peer pull` federation path (Pillar 3 /
//! BR-VIEW-4) — so the rows the thread reads are produced by production code, not
//! hand-inserted. The 2-step read is LOCAL (DB index + local artifact `fs::read`); NO
//! network seam exists on this route (offline by construction, I-CT-5).
//!
//! Layer placement (nw-tdd-methodology Layered Test Discipline matrix + Mandate 9/11):
//! every scenario here is a layer-3/layer-5 subprocess + real-I/O test — EXAMPLE-only.
//! The sad paths (empty-reason counter, unknown CID) are enumerated explicitly, never
//! PBT-generated at this layer (generative exploration of the pure projection +
//! render is a layer-1/2 DELIVER concern in the `viewer-domain` units).
//!
//! Build-before-run note (carry into the DELIVER roadmap, mirrors slice-06/07/08/09/10):
//! `cargo test` does NOT rebuild a spawned binary automatically — the roadmap/run MUST
//! `cargo build` the `openlore` (viewer) bin before running these ATs so
//! `ViewerServer::start` spawns the CURRENT viewer, not a stale one. The thread needs
//! NO second binary — the 2-step read is a LOCAL read.
//!
//! Mandate 7 RED scaffolds: the ATs spawn the bin + HTTP, so they COMPILE now with
//! `todo!()` bodies + the new `seed_claim_with_counter` /
//! `seed_claim_two_counters_distinct_authors` / `seed_counter_empty_reason` /
//! `seed_uncountered_claim` + `assert_counter_thread_*` /
//! `assert_counter_claim_verbatim_unchanged` helpers (which compile — they drive
//! existing seeding seams or `todo!()` themselves). Each scenario body is `todo!()` →
//! panics → classifies RED (MISSING_FUNCTIONALITY), NOT BROKEN. They stay RED until
//! DELIVER's per-scenario RED→GREEN→COMMIT cycles.
//!
//! Covers:
//! - US-CT-002 (the thread, CT-1..CT-5): CT-1 walking skeleton — GET /claims/{cid}
//!   WITH HX-Request over a countered claim → ONLY the detail fragment (the claim
//!   verbatim + a counter-thread naming the counter's author DID + CID + verbatim
//!   reason, no chrome) + no-JS full-page parity + shown-never-applied (original
//!   confidence verbatim/unchanged) + anti-merging (two distinct authors → two items,
//!   no "disputed by 2") + verbatim reason + counter CID drill-links + the empty-reason
//!   "no reason provided" edge.
//! - US-CT-003 (no-noise + presence flag, CT-6..CT-8): an un-countered claim renders
//!   exactly as slice-06 (no section, no "0 counters" noise) + a countered claim shows
//!   a neutral "Countered" presence flag (never a verdict / count-based re-rank) +
//!   unknown CID → the existing guided 404 unchanged (no regression of slice-06 V-7).
//
// SCAFFOLD: true

mod support;

#[allow(unused_imports)]
use support::*;

// =============================================================================
// US-CT-002 — see the counter-claim thread beneath a countered claim
// (CT-1..CT-5). CT-1 is the thinnest end-to-end thread (the walking skeleton).
// =============================================================================

/// CT-1 / WALKING SKELETON (US-CT-002; the riskiest-assumption thread): from the
/// LOCAL store, `GET /claims/{cid}` WITH the `HX-Request` header for a claim that has
/// ≥1 counter returns ONLY the detail fragment — the claim rendered VERBATIM plus a
/// counter-thread section naming the counter's author DID + its own CID + its verbatim
/// reason — with NO full-page chrome. This is the thinnest complete thread the slice
/// can demo: viewer → LOCAL 2-step read (indexed ref lookup + artifact reason) → pure
/// projection → HTML fragment, proving the read-only viewer can host a counter-thread
/// while preserving the read-only / shown-never-applied / anti-merging / local-first /
/// progressive-enhancement invariants.
///
/// Given Maria's read-only viewer reads a LOCAL store holding Rachel's claim and her
///   OWN counter (with a verbatim reason) targeting it;
/// When she opens the claim detail WITH the htmx header
///   (`GET /claims/{cid}`, HX-Request);
/// Then she receives ONLY the `#claim-detail` fragment (no chrome) showing the claim
///   verbatim and, beneath it, the counter-thread naming the counter's author DID, its
///   CID, and the verbatim reason.
///
/// @us-ct-002 @walking_skeleton @driving_port @driving_adapter @real-io @htmx-fragment
/// @i-ct-2 @i-ct-3 @i-ct-6 @kpi-fed-3 @happy
#[test]
fn open_a_countered_claim_with_htmx_returns_only_the_detail_fragment_with_the_thread() {
    // GIVEN a REAL local store seeded so Rachel's claim (confidence 0.91) is countered
    // by Maria's OWN counter (`bafy...new`, a verbatim reason) — the own counter via
    // the production `claim counter` verb, against a target seeded via the production
    // federation path (Pillar 3 / BR-VIEW-4). NO network: the 2-step read is LOCAL.
    //
    // WHEN Maria submits `GET /claims/{target_cid}` WITH the HX-Request header
    // (get_htmx).
    //
    // THEN the response is ONLY the `#claim-detail` fragment (`is_fragment()`, NOT a
    // full page): the claim is rendered VERBATIM (confidence 0.91), and BENEATH it the
    // counter-thread names the counter's author DID + its CID + its verbatim reason.
    let env = TestEnv::initialized();
    let seeded = seed_claim_with_counter(&env);

    let viewer = ViewerServer::start(&env);
    let response = viewer.get_htmx(&format!("/claims/{}", seeded.target_cid));

    // THEN: 200 text/html, ONLY the #claim-detail fragment (no chrome).
    assert_eq!(
        response.status, 200,
        "CT-1: GET /claims/{{cid}} with HX-Request over a countered claim must return \
         200;\n--- body ---\n{}",
        response.body
    );
    assert!(
        response.content_type.contains("text/html"),
        "CT-1: the detail fragment must be served as text/html; content_type was {:?}",
        response.content_type
    );
    assert!(
        response.is_fragment(),
        "CT-1: the HX-Request response must be ONLY the #claim-detail fragment (no \
         full-page chrome);\n--- body ---\n{}",
        response.body
    );

    // THEN: the claim renders VERBATIM (0.91) and, beneath it, the counter-thread names
    // the counter's author DID + its CID + its verbatim reason.
    assert_counter_thread_renders_attributed_verbatim(&response.body, &seeded.counters, "0.91");
}

/// CT-2 (US-CT-002 — no-JS full page + parity, I-CT-6): `GET /claims/{cid}` WITHOUT
/// `HX-Request` serves a COMPLETE full page (chrome + the SAME `#claim-detail` region)
/// whose detail region is STRUCTURALLY IDENTICAL to the htmx fragment — parity by
/// construction (the page EMBEDS the fragment fn; the thread is rendered INSIDE
/// `render_claim_detail_fragment`). The no-JS no-regression contract: the full page is
/// the contract, the htmx swap is a nicety.
///
/// Given Maria opens a countered claim in a plain browser (no JS);
/// When the page renders, and she also requests it with the htmx header;
/// Then the no-JS response is a full page whose `#claim-detail` region embeds the SAME
///   claim + counter-thread the htmx fragment returns (parity).
///
/// @us-ct-002 @driving_port @real-io @no-js @full-page @parity @i-ct-6 @happy
#[test]
fn a_countered_claim_renders_identically_under_htmx_and_no_js() {
    // GIVEN the same countered-claim store (Rachel's claim + Maria's own counter).
    let env = TestEnv::initialized();
    let seeded = seed_claim_with_counter(&env);

    let viewer = ViewerServer::start(&env);

    // WHEN Maria requests the detail WITHOUT the htmx header (full page) AND WITH it
    // (fragment).
    let full_page = viewer.get(&format!("/claims/{}", seeded.target_cid));
    let fragment = viewer.get_htmx(&format!("/claims/{}", seeded.target_cid));

    // THEN both succeed as text/html.
    assert_eq!(
        full_page.status, 200,
        "CT-2: GET /claims/{{cid}} WITHOUT HX-Request over a countered claim must \
         return 200;\n--- body ---\n{}",
        full_page.body
    );
    assert_eq!(
        fragment.status, 200,
        "CT-2: GET /claims/{{cid}} WITH HX-Request over a countered claim must return \
         200;\n--- body ---\n{}",
        fragment.body
    );
    assert!(
        full_page.content_type.contains("text/html"),
        "CT-2: the full page must be served as text/html; content_type was {:?}",
        full_page.content_type
    );
    assert!(
        fragment.content_type.contains("text/html"),
        "CT-2: the htmx fragment must be served as text/html; content_type was {:?}",
        fragment.content_type
    );

    // THEN: the no-JS response is_full_page() (chrome present) while the htmx response
    // is_fragment() (no chrome) — the two SHAPES differ (ADR-033) even though the
    // detail REGION is identical.
    assert!(
        full_page.is_full_page(),
        "CT-2: the no-JS response must be a COMPLETE full page (chrome present);\n\
         --- body ---\n{}",
        full_page.body
    );
    assert!(
        fragment.is_fragment(),
        "CT-2: the HX-Request response must be ONLY the #claim-detail fragment (no \
         chrome);\n--- body ---\n{}",
        fragment.body
    );

    // THEN: the full page EMBEDS the SAME fragment — its `#claim-detail` region carries
    // the SAME claim (confidence 0.91 verbatim) AND the SAME counter-thread (the
    // counter's author DID + its CID + its verbatim reason) the htmx fragment returns.
    // No divergence: parity by construction (the page embeds the fragment fn — I-CT-6).
    assert_counter_thread_renders_attributed_verbatim(&full_page.body, &seeded.counters, "0.91");
    assert_counter_thread_renders_attributed_verbatim(&fragment.body, &seeded.counters, "0.91");
}

/// CT-3 / GOLD shown-never-applied (US-CT-002; I-CT-2 / OD-AV-7 / ADR-015): the
/// countered claim is rendered VERBATIM with its ORIGINAL confidence (0.91), UNCHANGED
/// by the existence of the counter — the counter never filters, merges, re-weights, or
/// re-ranks it. This is the load-bearing slice-11 gold: the countered claim's rendered
/// confidence/fields are byte-IDENTICAL with and without the counter present. (The
/// invariants file carries the cross-cutting read-only / no-write / offline gold; this
/// story-local gold pins the shown-never-applied confidence directly against the
/// matching un-countered render.)
///
/// Given the SAME claim is rendered once with no counter and once with a counter;
/// When both detail pages render;
/// Then the claim's confidence (0.91) and fields are IDENTICAL in both — the counter
///   changed nothing above the thread.
///
/// @us-ct-002 @driving_port @real-io @shown-never-applied @i-ct-2 @i-ct-4 @gold
#[test]
fn the_countered_claim_is_shown_verbatim_never_re_weighted_by_its_counter() {
    // GIVEN two stores: one where the target claim is UN-countered, one where the SAME
    // claim (same subject/predicate/object/confidence 0.91) IS countered.
    // WHEN each claim's detail renders.
    // THEN the countered render shows the claim's confidence VERBATIM (0.91) and its
    // fields IDENTICAL to the un-countered render — the counter is additive context
    // BELOW, never a re-weight ABOVE (assert_counter_claim_verbatim_unchanged; I-CT-2).
    // GIVEN two independent stores seeding the SAME claim shape (Rachel's claim at
    // 0.91): one with NOTHING countering it, one with the operator's OWN counter.
    let uncountered_env = TestEnv::initialized();
    let uncountered_cid = seed_uncountered_claim(&uncountered_env);

    let countered_env = TestEnv::initialized();
    let seeded = seed_claim_with_counter(&countered_env);

    // The two stores seed the SAME content-addressed target — so the claim region the
    // shown-never-applied gold diffs is the SAME claim, not two different ones.
    assert_eq!(
        uncountered_cid, seeded.target_cid,
        "CT-3: the un-countered and countered seeds must target the SAME claim CID \
         (same subject/predicate/object/confidence) for the byte-diff to be meaningful; \
         uncountered {uncountered_cid:?} vs countered {:?}",
        seeded.target_cid
    );

    // WHEN each claim's detail renders.
    let uncountered_viewer = ViewerServer::start(&uncountered_env);
    let uncountered = uncountered_viewer.get(&format!("/claims/{uncountered_cid}"));

    let countered_viewer = ViewerServer::start(&countered_env);
    let countered = countered_viewer.get(&format!("/claims/{}", seeded.target_cid));

    assert_eq!(
        uncountered.status, 200,
        "CT-3: the un-countered detail must return 200;\n--- body ---\n{}",
        uncountered.body
    );
    assert_eq!(
        countered.status, 200,
        "CT-3: the countered detail must return 200;\n--- body ---\n{}",
        countered.body
    );

    // THEN: the countered render shows the claim's confidence VERBATIM (0.91) and the
    // claim region is byte-identical to the un-countered render — the counter is additive
    // context BELOW, never a re-weight/filter/merge ABOVE (I-CT-2 / OD-AV-7 / ADR-015).
    assert_counter_claim_verbatim_unchanged(&uncountered.body, &countered.body, "0.91");
}

/// CT-4 / GOLD anti-merging (US-CT-002; I-CT-3 / KPI-AV-2 / KPI-GRAPH-2 / KPI-FED-1):
/// a claim countered by TWO DISTINCT authors renders TWO attributed counter entries —
/// each under its OWN author DID + its OWN CID + its verbatim reason — and NEVER a
/// merged "disputed by 2" / consensus aggregate row. The two counters are seeded via
/// the PRODUCTION CLI counter path: the operator's OWN counter via `claim counter`,
/// and the second author's counter via the federation `peer add` + `peer pull` path
/// (a peer who authored a `counters`-referencing signed claim). UNION ALL over
/// `claims ∪ peer_claims` with explicit author_did + cid — anti-merging by
/// construction.
///
/// Given Rachel's claim is countered by Maria's OWN counter AND by peer Tobias's
///   counter (two distinct authors);
/// When Maria opens the claim detail;
/// Then exactly TWO attributed counter entries render (Maria + her CID; Tobias + his
///   CID), and NO single row aggregates them into a "disputed by 2" / consensus row.
///
/// @us-ct-002 @driving_port @real-io @anti-merging @i-ct-3 @kpi-av-2 @gold
#[test]
fn two_counters_by_distinct_authors_render_as_two_attributed_items_never_merged() {
    // GIVEN Rachel's claim countered by TWO DISTINCT authors: Maria's OWN counter (via
    // the production `claim counter` verb → `claims`) AND peer Tobias's counter (via
    // the production `peer add` + `peer pull` federation path → `peer_claims`).
    // WHEN Maria opens the claim detail.
    // THEN the counter-thread renders EXACTLY two attributed entries — Maria (own DID +
    // her CID) and Tobias (his DID + his CID), each with its verbatim reason — and NO
    // merged "disputed by N" / consensus aggregate row appears
    // (assert_counter_thread_two_attributed_no_merge; UNION ALL explicit author_did +
    // cid; I-CT-3).
    let env = TestEnv::initialized();
    let seeded = seed_claim_two_counters_distinct_authors(&env);

    let viewer = ViewerServer::start(&env);
    let response = viewer.get(&format!("/claims/{}", seeded.target_cid));

    assert_eq!(
        response.status, 200,
        "CT-4: GET /claims/{{cid}} over a claim countered by two distinct authors must \
         return 200;\n--- body ---\n{}",
        response.body
    );

    // THEN: both counters render as two attributed items (Maria own + Tobias peer),
    // each with its OWN author DID + CID + verbatim reason, and the original claim's
    // confidence renders verbatim + unchanged (shown-never-applied).
    assert_counter_thread_renders_attributed_verbatim(&response.body, &seeded.counters, "0.91");

    // AND: EXACTLY two attributed entries, NO merged "disputed by 2" / consensus /
    // net-verdict aggregate row (the load-bearing anti-merging gold; I-CT-3 / KPI-AV-2).
    assert_counter_thread_two_attributed_no_merge(&response.body, &seeded.counters);
}

/// CT-5 (US-CT-002 — verbatim reason + counter CID drill-link): the counter's
/// free-text `--reason` renders EXACTLY as authored (including punctuation), the
/// original claim's confidence renders verbatim (`0.91`, never `0.9`/`91%`; I-CT-4),
/// and the counter's CID is a render-only `<a href>` to `/claims/{counter_cid}` — a
/// ONE-HOP drill into the counter's own detail, with NO nested/recursive counter
/// render (depth-1, ADR-047).
///
/// Given Rachel's claim is countered by Maria's counter `bafy...new` with a specific
///   punctuated verbatim reason;
/// When Maria opens the claim detail;
/// Then the reason renders byte-for-byte, the claim's confidence renders `0.91`
///   verbatim, and the counter's CID is an `<a href="/claims/bafy...new">` drill-link.
///
/// @us-ct-002 @driving_port @real-io @verbatim @drill-link @i-ct-3 @i-ct-4 @happy
#[test]
fn the_counter_reason_renders_verbatim_and_its_cid_is_a_one_hop_drill_link() {
    // GIVEN Rachel's claim countered by Maria's counter (CID `counter_cid`) whose
    // `--reason` carries specific punctuation (the verbatim-render contract).
    // WHEN Maria opens the claim detail.
    // THEN (a) the reason text renders byte-for-byte; (b) the original claim's
    // confidence renders `0.91` verbatim (never `0.9`/`91%`, I-CT-4); and (c) the
    // counter's CID renders as an `<a href="/claims/{counter_cid}">` render-only
    // drill-link (one hop; NO nested counter render — depth-1, ADR-047). Asserted on
    // the OBSERVABLE rendered HTML.
    let env = TestEnv::initialized();
    let seeded = seed_claim_with_counter(&env);
    let counter = seeded
        .counters
        .first()
        .expect("CT-5: seed_claim_with_counter must seed exactly one counter");

    let viewer = ViewerServer::start(&env);
    let response = viewer.get(&format!("/claims/{}", seeded.target_cid));

    assert_eq!(
        response.status, 200,
        "CT-5: GET /claims/{{cid}} over a countered claim must return 200;\n\
         --- body ---\n{}",
        response.body
    );

    // (a) the counter's verbatim free-text reason renders byte-for-byte AND
    // (b) the original claim's confidence renders `0.91` verbatim (the
    // shown-never-applied / verbatim-confidence contract; I-CT-2 / I-CT-4).
    assert_counter_thread_renders_attributed_verbatim(&response.body, &seeded.counters, "0.91");

    // (a, sharpened) the punctuated reason is present byte-for-byte — the `;` and
    // `,` are load-bearing (NFC-normalized at author time; ADR-015 / WD-35).
    let reason = counter
        .reason
        .as_deref()
        .expect("CT-5: the seeded own counter carries a verbatim reason");
    assert!(
        response.body.contains(reason),
        "CT-5: the counter's free-text reason must render EXACTLY as authored \
         (verbatim, including punctuation) {reason:?};\n--- body ---\n{}",
        response.body
    );

    // (c) the counter's CID is a render-only ONE-HOP drill-link — an
    // `<a href="/claims/{counter_cid}">` anchor (navigation TEXT, never an
    // executable control; ADR-047 / I-CT-1).
    let drill_link = format!("<a href=\"/claims/{}\">", counter.cid);
    assert!(
        response.body.contains(&drill_link),
        "CT-5: the counter's CID must render as a render-only one-hop drill-link \
         {drill_link:?};\n--- body ---\n{}",
        response.body
    );

    // (c, depth-1) the thread does NOT recurse into the counter's own counters:
    // the rendered counter-thread section heading appears EXACTLY once (the target
    // claim's thread only — no nested/recursive counter render; ADR-047).
    let thread_heading = "<h2>Counter-claims</h2>";
    assert_eq!(
        response.body.matches(thread_heading).count(),
        1,
        "CT-5: the thread must be DEPTH-1 — the counter-thread heading {thread_heading:?} \
         must appear EXACTLY once (no nested/recursive counter render; ADR-047);\n\
         --- body ---\n{}",
        response.body
    );
}

/// CT-6 (US-CT-002 edge — empty-reason counter; ADR-047 / ADR-015 wire-optional): a
/// counter whose `reason` is absent (a peer record authored by a non-OpenLore client,
/// `unsigned.reason == None`) renders an explicit "no reason provided" state — the
/// counter's author DID + its CID are STILL shown — never a blank line and never a
/// crash. The empty-reason edge is total at the type level (`reason: Option<String>`).
///
/// Given Rachel's claim is countered by a peer record whose reason is empty/absent;
/// When Maria opens the claim detail;
/// Then the counter entry shows the author DID + CID AND an explicit "no reason
///   provided" state (not a blank line, not a crash).
///
/// @us-ct-002 @driving_port @real-io @empty-reason @edge @adr-047
#[test]
fn a_counter_with_no_reason_renders_an_explicit_no_reason_provided_state() {
    // GIVEN Rachel's claim countered by a PEER record whose `reason` is absent/empty
    // (ADR-015 wire-optional asymmetry; seeded via the production federation path with
    // a `counters`-referencing record that omits the reason).
    // WHEN Maria opens the claim detail.
    // THEN the counter entry STILL shows the author DID + its CID, and shows the
    // explicit "no reason provided" state — never a blank line, never a crash
    // (assert_counter_thread_empty_reason_state; ADR-047).
    let env = TestEnv::initialized();
    let seeded = seed_counter_empty_reason(&env);
    let counter = seeded
        .counters
        .first()
        .expect("CT-6: seed_counter_empty_reason must seed exactly one counter");
    assert!(
        counter.reason.is_none(),
        "CT-6: the seeded empty-reason counter must carry reason == None (the \
         wire-optional edge)"
    );

    let viewer = ViewerServer::start(&env);
    let response = viewer.get(&format!("/claims/{}", seeded.target_cid));

    // THEN: 200 (never a 5xx/panic on the empty-reason edge).
    assert_eq!(
        response.status, 200,
        "CT-6: GET /claims/{{cid}} over a claim countered by an empty-reason peer record \
         must return 200 (never a 5xx/panic);\n--- body ---\n{}",
        response.body
    );

    // THEN: the counter entry STILL shows its author DID + its CID AND an explicit
    // "no reason provided" state — never a blank line (ADR-047).
    assert_counter_thread_empty_reason_state(&response.body, counter);
}

// =============================================================================
// US-CT-003 — no-noise discipline + the neutral "Countered" presence flag
// (CT-7..CT-9). An un-countered claim is byte-unaffected; a countered one is flagged.
// =============================================================================

/// CT-7 (US-CT-003 — no-noise discipline; I-CT-2): an UN-countered claim renders
/// EXACTLY as the slice-06 detail today — NO "Counter-claims" section, NO "Countered"
/// flag, and NO "0 counters" / "no disagreement" empty-state noise. `query_counter_
/// claims` returns an empty vec → `CounterThread::None` → the renderer shows the claim
/// alone. This is the byte-unaffected guarantee for the common case.
///
/// Given Maria's store contains her claim and NOTHING counters it;
/// When she opens its detail page;
/// Then the claim + evidence render as in slice-06, with no counter section, no flag,
///   and no empty-state noise.
///
/// @us-ct-003 @driving_port @real-io @no-noise @i-ct-2 @happy
#[test]
fn an_un_countered_claim_shows_no_counter_section_and_no_empty_noise() {
    // GIVEN an UN-countered claim (seed_uncountered_claim — a plain claim, nothing
    // references it as a counter).
    // WHEN Maria opens its detail page.
    // THEN the claim + evidence render as in slice-06 (V-5/6), and the body carries NO
    // "Counter-claims" section, NO "Countered" presence flag, and NO "0 counters" /
    // "no disagreement" empty-state text (assert_no_counter_thread_noise; I-CT-2 —
    // `CounterThread::None` renders nothing extra).
    let env = TestEnv::initialized();
    let target_cid = seed_uncountered_claim(&env);

    let viewer = ViewerServer::start(&env);
    let response = viewer.get(&format!("/claims/{target_cid}"));

    // THEN: 200 text/html, the claim renders (slice-06 parity — confidence 0.91
    // verbatim + the subject), and NO counter-thread noise.
    assert_eq!(
        response.status, 200,
        "CT-7: GET /claims/{{cid}} over an un-countered claim must return 200;\n\
         --- body ---\n{}",
        response.body
    );
    assert!(
        response.content_type.contains("text/html"),
        "CT-7: the detail must be served as text/html; content_type was {:?}",
        response.content_type
    );

    // THEN: the claim + evidence render as in slice-06 (the confidence renders verbatim;
    // the subject is shown) — the common-case detail is byte-unaffected by slice-11.
    assert!(
        response.body.contains("0.91"),
        "CT-7: the un-countered claim's confidence must render verbatim (slice-06 \
         parity);\n--- body ---\n{}",
        response.body
    );
    assert!(
        response.body.contains("github:rust-lang/cargo"),
        "CT-7: the un-countered claim's subject must render (slice-06 parity);\n\
         --- body ---\n{}",
        response.body
    );

    // THEN: NO "Counter-claims" section, NO "Countered" presence flag, and NO
    // "0 counters" / "no disagreement" empty-state noise (assert_no_counter_thread_noise;
    // I-CT-2 — `CounterThread::None` renders nothing extra).
    assert_no_counter_thread_noise(&response.body);
}

/// CT-8 (US-CT-003 — neutral presence flag; I-CT-3): a countered claim shows a neutral
/// "Countered" presence indicator near the claim — a PRESENCE marker only, NEVER a
/// verdict, a score, a count ("disputed by N"), or a count-based re-rank. The flag
/// makes disagreement legible without picking a winner.
///
/// Given Rachel's claim is countered;
/// When Maria opens its detail page;
/// Then a neutral "Countered" presence flag marks the claim — with no score, no
///   verdict, and no "disputed by N" count.
///
/// @us-ct-003 @driving_port @real-io @presence-flag @i-ct-3 @happy
#[test]
fn a_countered_claim_shows_a_neutral_countered_presence_flag_not_a_verdict() {
    // GIVEN a countered claim (seed_claim_with_counter — Rachel's claim countered by
    // Maria's OWN counter, via the production federation + `claim counter` paths).
    // WHEN Maria opens its detail page.
    // THEN a neutral "Countered" presence flag marks the claim (presence only), and NO
    // verdict / score / "disputed by N" count-based phrasing appears
    // (assert_counter_thread_presence_flag_is_neutral; I-CT-3 — the flag is a marker,
    // never a weight or re-rank).
    let env = TestEnv::initialized();
    let seeded = seed_claim_with_counter(&env);

    let viewer = ViewerServer::start(&env);
    let response = viewer.get(&format!("/claims/{}", seeded.target_cid));

    assert_eq!(
        response.status, 200,
        "CT-8: GET /claims/{{cid}} over a countered claim must return 200;\n\
         --- body ---\n{}",
        response.body
    );
    assert!(
        response.content_type.contains("text/html"),
        "CT-8: the detail must be served as text/html; content_type was {:?}",
        response.content_type
    );

    // THEN: the countered claim carries the neutral "Countered" presence flag — a marker
    // that the claim HAS disagreement — and NO verdict ("disputed"/"refuted"/"false"/
    // "wrong") and NO count-based re-rank / aggregate judgement appears. The flag is
    // presence-only: it does NOT assert the counter is correct (I-CT-3).
    assert_counter_thread_presence_flag_is_neutral(&response.body);

    // AND: shown-never-applied — the original claim's confidence renders VERBATIM (0.91),
    // UNCHANGED by the presence of the counter (the flag inserts no count-based re-rank /
    // re-weight above the claim; I-CT-2).
    assert!(
        response.body.contains("0.91"),
        "CT-8: the countered claim's confidence must render verbatim + unchanged (0.91) — \
         the neutral flag never re-weights the claim (I-CT-2);\n--- body ---\n{}",
        response.body
    );

    // AND: the no-flag-when-uncountered half of the contract (reuse the CT-7 no-noise
    // discipline + seed_uncountered_claim): the SAME claim shape with NOTHING countering
    // it carries NO "Countered" presence flag and no counter-thread noise.
    let uncountered_env = TestEnv::initialized();
    let uncountered_cid = seed_uncountered_claim(&uncountered_env);

    let uncountered_viewer = ViewerServer::start(&uncountered_env);
    let uncountered = uncountered_viewer.get(&format!("/claims/{uncountered_cid}"));

    assert_eq!(
        uncountered.status, 200,
        "CT-8: GET /claims/{{cid}} over an un-countered claim must return 200;\n\
         --- body ---\n{}",
        uncountered.body
    );
    assert_no_counter_thread_noise(&uncountered.body);
}

/// CT-9 (US-CT-003 boundary — unknown CID, no slice-06 regression): a CID that is not
/// in the store STILL shows the EXISTING slice-06 guided not-found page ("No claim with
/// that identifier in your store" + a back link) — UNCHANGED by slice-11. The 404 path
/// gains NO counter thread / flag (the thread is only built on the `Ok(Some(detail))`
/// arm). This pins that slice-11 does not regress slice-06 V-7.
///
/// Given no claim with the requested CID exists in the store;
/// When Maria opens that detail page;
/// Then she sees the existing guided not-found message + back link, with NO counter
///   thread or "Countered" flag added to the 404 path.
///
/// @us-ct-003 @driving_port @real-io @not-found @boundary @no-regression @error
#[test]
fn an_unknown_cid_keeps_the_existing_guided_not_found_with_no_thread_added() {
    // GIVEN a store with at least one real (countered) claim, but NOT the mistyped CID.
    // WHEN Maria opens a detail page for a CID that is not in her store.
    // THEN she sees the EXISTING slice-06 guided not-found message ("No claim with that
    // identifier in your store") + a back link to /claims, and the 404 path carries NO
    // "Counter-claims" section and NO "Countered" flag (the thread is built only on the
    // claim-found arm — no regression of slice-06 V-7).
    todo!(
        "DELIVER (CT-9 unknown CID no-regression): seed_claim_with_counter (so the \
         store is non-empty), start ViewerServer, GET a NON-existent /claims/{{cid}}; \
         assert the existing guided not-found text 'No claim with that identifier in \
         your store' + a '/claims' back link render UNCHANGED, AND the 404 body carries \
         NO 'Counter-claims' section and NO 'Countered' flag (no slice-06 V-7 \
         regression)"
    );
}
