//! Slice-14 acceptance — the `openlore ui` at-a-glance "Countered" PRESENCE FLAG
//! extended to the LAST LOCAL surface the operator scans: the SCORING-BEARING
//! `GET /score?contributor=<did>` per-contribution breakdown rows (US-CF-001 infra
//! wiring + US-CF-002 user-visible flag; ADR-051).
//!
//! slice-11 made disagreement legible once a claim is OPENED (the counter thread on
//! `/claims/{cid}`); slice-12 made it discoverable while scanning the operator's OWN
//! `/claims` list; slice-13 closed the gap on `/peer-claims` + the `/project` +
//! `/philosophy` survey edges. slice-14 closes the LAST surface: each `/score`
//! per-contribution breakdown row whose contribution has >= 1 counter now carries the
//! SAME neutral "Countered" marker — the REUSED slice-13 `render_countered_link`
//! render-only `<a href="/claims/{cid}">Countered</a>` one-hop link to that claim's
//! slice-11 thread.
//!
//! The flag REUSES the slice-12 `StoreReadPort::counter_presence_for(&[cid]) ->
//! HashSet<String>` batch read VERBATIM (ADR-048), wired into the `/score` handler
//! (ADR-051) — NO new read method, NO new SQL, NO new route. The page CID set is the
//! UNION of every `Contribution.cid` across every `WeightedPairing` in
//! `ScoreState::Scored { view }`, flattened ONCE in the effect-shell helper
//! `score_counter_presence` and queried in ONE aggregate read per render (the N+1
//! guard, AC-001-ONE-CALL / AC-001-INVARIANT). `Form`/`NoClaims` build no view → the
//! helper is never called → 0 queries.
//!
//! The slice-14 CARDINAL distinction from slices 12/13: `/score` carries SCORING
//! SEMANTICS, so the flag must be provably ORTHOGONAL to the score — SHOWN, never
//! APPLIED. (1) The per-contribution subtotals STILL sum to the displayed pairing
//! weight WITH the flag present, and a countered contribution renders its FULL original
//! subtotal — the counter subtracts nothing (AC-SCORE-SUMWEIGHT). (2) With the markers
//! AND the `SCORE_COUNTER_LEGEND` anti-misread legend elided, the `/score` render is
//! byte-identical to the slice-09 baseline — every weight, confidence, author bonus,
//! triangulation bonus, subtotal, headline total, bucket, `[SPARSE]` line, pairing
//! ranking, and contribution row order unchanged (AC-SCORE-BYTEID). (3) The breakdown
//! carries a short NEUTRAL legend that NEVER uses a verdict/penalty word
//! ("disputed"/"refuted"/"false"/"penalty"/"deduction"/"lowered"/"disputed score")
//! (AC-SCORE-ANTIMISREAD).
//!
//! The flag is PRESENCE-only (a contribution countered by N distinct authors shows ONE
//! neutral marker, never "countered by N", never a count, never a verdict) and ADDITIVE:
//! it NEVER changes the ranked pairing order, the contribution row order, or any
//! displayed number. An un-countered contribution renders byte-identically to slice-09.
//!
//! Driving discipline (Mandate 1): every scenario enters through the REAL `openlore ui`
//! subprocess (`ViewerServer`) + in-test HTTP GET `/score` (with/without the
//! `HX-Request` header — the slice-07 `get`/`get_htmx` pair) and asserts on the returned
//! HTML. NO scenario calls the `viewer-domain` `render_score_*` fns or
//! `counter_presence_for` directly (those are unit/property-level, exercised in DELIVER).
//! The local DuckDB store is REAL, seeded through the PRODUCTION federation write paths
//! (`peer add` + `peer pull` for the contributor's scoring trail; a DISTINCT peer's
//! verifiable counter for the flag — self-counter is BLOCKED, so a *peer* counters the
//! contribution) — Pillar 3 / BR-VIEW-4. The presence read is LOCAL (DB-index only); NO
//! network seam exists on `/score` (offline by construction, AC-SCORE-LOCAL).
//!
//! Layer placement (nw-tdd-methodology Layered Test Discipline + Mandate 9/11): every
//! scenario here is a layer-3/layer-5 subprocess + real-I/O test — EXAMPLE-only. The
//! sad/edge paths (none-countered, multi-author counter, identical-subtotal anti-misread)
//! are enumerated explicitly, never PBT-generated at this layer. The strict 1-query N+1
//! bound is a DELIVER `adapter-duckdb` unit/property assertion (the REUSED slice-12
//! read); at this subprocess AT layer the N+1 guard is asserted via its behavioral proxy
//! (a multi-pairing, multi-contribution breakdown flags the countered subset correctly
//! in ONE request).
//!
//! Build-before-run note (carry into the DELIVER roadmap, mirrors slice-06..13):
//! `cargo test` does NOT rebuild a spawned binary automatically — the roadmap/run MUST
//! `cargo build` the `openlore` (viewer) bin before running these ATs so
//! `ViewerServer::start` spawns the CURRENT viewer, not a stale one. The flag needs NO
//! second binary — the presence read is a LOCAL read.
//!
//! Mandate 7 RED scaffolds (ADR-025): the ATs spawn the bin + HTTP, so they COMPILE now
//! with `todo!()`-stubbed slice-14 seeds (`seed_score_breakdown_one_contribution_
//! countered` / `_target_two_counters_distinct_authors` / `_identical_subtotals_one_
//! countered` / `_none_countered` / `_many_pairings_known_countered_subset`) + assert
//! helpers (`assert_score_row_flagged_countered` / `_not_flagged` /
//! `assert_score_flag_is_single_neutral_presence` / `assert_score_flag_links_to_thread`
//! / `assert_score_html_breakdown_sums_to_weight_with_flag` /
//! `assert_score_legend_present_and_blocklist_clean` /
//! `assert_score_render_byte_identical_to_slice09`), all `todo!()`-stubbed in
//! support/mod.rs (they compile, then panic). Each scenario body reaches a `todo!()` ->
//! panics -> classifies RED (MISSING_FUNCTIONALITY), NOT BROKEN. They stay RED until
//! DELIVER's per-scenario RED->GREEN->COMMIT cycles.
//!
//! Covers:
//! - US-CF-001 (infra wiring, SF-N1): the N+1-flatten behavioral proxy — a LARGE
//!   multi-pairing / multi-contribution breakdown flags the countered subset correctly
//!   in ONE request (AC-001-ONE-CALL / AC-001-INVARIANT).
//! - US-CF-002 (the `/score` flag, SF-1..SF-7): SF-1 walking skeleton — GET /score WITH
//!   HX-Request over a scored contributor with one countered contribution -> 200, ONLY
//!   the `#score-results` fragment, the countered contribution row carries the neutral
//!   "Countered" `<a href>` marker linking to `/claims/{cid}`, the un-countered rows
//!   none, the legend present, and the subtotals still sum to the weight; + presence-only
//!   single marker for a two-author countered contribution (SF-2); the CARDINAL
//!   sum-to-weight on a FLAGGED breakdown (SF-3); the CARDINAL byte-identity vs slice-09
//!   (SF-4); the anti-misread identical-subtotal copy (SF-5); the no-noise / no-counter
//!   baseline (SF-6); the htmx-vs-no-JS parity (SF-7).
//
// SCAFFOLD: true

mod support;

#[allow(unused_imports)]
use support::*;

// =============================================================================
// US-CF-002 — the neutral "Countered" presence flag on the SCORING-BEARING /score
// per-contribution breakdown rows (SF-1..SF-7). SF-1 is the thinnest end-to-end
// thread (walking skeleton).
// =============================================================================

/// SF-1 / WALKING SKELETON (US-CF-002; the riskiest-assumption thread): from the LOCAL
/// store, `GET /score?contributor=<did>` WITH the `HX-Request` header over a store where
/// a scored contributor's multi-row breakdown has EXACTLY ONE contribution countered (a
/// DISTINCT peer authored a counter targeting it) and the others are NOT returns ONLY the
/// `#score-results` fragment (no full-page chrome) in which the countered contribution
/// row carries the NEUTRAL "Countered" marker — the REUSED render-only
/// `<a href="/claims/{cid}">` link to that claim's slice-11 thread — while the
/// un-countered rows carry NO marker, the breakdown carries the anti-misread legend, and
/// the per-contribution subtotals STILL sum to the displayed pairing weight. This is the
/// thinnest complete slice the `/score` feature can demo: viewer -> LOCAL scoring feed
/// read -> PURE `scoring::score` -> LOCAL batch presence read (REUSED slice-12) -> pure
/// projection threading `&presence` -> HTML fragment, proving the scoring surface can
/// carry an at-a-glance disagreement flag PROVABLY ORTHOGONAL to the score (shown, never
/// applied) while preserving the read-only / presence-only / local-first / progressive-
/// enhancement invariants.
///
/// Given Maria's viewer reads a LOCAL store holding a scored contributor's multi-row
///   breakdown, exactly one contribution of which is countered by a DISTINCT peer;
/// When she opens that contributor's Score breakdown WITH the htmx header
///   (`GET /score?contributor=<did>`, HX-Request);
/// Then she receives ONLY the `#score-results` fragment (no chrome) in which the
///   countered contribution row carries the neutral "Countered" marker linking to its
///   `/claims/{cid}` thread, the un-countered rows carry none, the breakdown carries the
///   anti-misread legend, and the subtotals still sum to the displayed weight.
///
/// @us-cf-002 @walking_skeleton @driving_port @driving_adapter @real-io @htmx-fragment
/// @flag @reuse-render @presence-only @cardinal-sum-to-weight @anti-misread @happy
#[test]
fn open_the_score_breakdown_with_htmx_flags_only_the_countered_contribution() {
    // GIVEN a REAL local store with a scored contributor's multi-row breakdown, exactly
    // ONE contribution of which is countered by a DISTINCT peer (seeded via the production
    // peer add + peer pull federation path; the counter lands in peer_claim_references).
    //
    // WHEN Maria submits `GET /score?contributor=<did>` WITH the HX-Request header.
    //
    // THEN the response is ONLY the `#score-results` fragment (`is_fragment()`, NOT a full
    // page): the countered contribution row carries the neutral "Countered" marker linking
    // to its slice-11 thread; every un-countered contribution row carries NO marker; the
    // breakdown carries the anti-misread legend; the subtotals still sum to the weight.
    let env = TestEnv::initialized();
    let seeded = seed_score_breakdown_one_contribution_countered(&env);
    let server = ViewerServer::start(&env);

    let response = server.get_htmx(&format!("/score?contributor={}", seeded.contributor_did));

    assert_eq!(
        response.status, 200,
        "SF-1: GET /score (HX-Request) must be 200; body was:\n{}",
        response.body
    );
    assert!(
        response.content_type.contains("text/html"),
        "SF-1: GET /score must serve text/html; got {:?}",
        response.content_type
    );
    assert!(
        response.is_fragment(),
        "SF-1: GET /score WITH HX-Request must return ONLY the #score-results fragment (no \
         chrome); body was:\n{}",
        response.body
    );
    assert!(
        response.body.contains(SCORE_RESULTS_ID),
        "SF-1: the fragment must carry the `#score-results` swap-target id; body was:\n{}",
        response.body
    );

    // The countered contribution row carries the neutral "Countered" marker linking to its
    // thread; every un-countered contribution row carries NO marker (AC-002-MARKER / -LINK
    // / -NO-NOISE).
    for countered in &seeded.countered_cids {
        assert_score_row_flagged_countered(&response.body, countered);
    }
    for uncountered in &seeded.uncountered_cids {
        assert_score_row_not_flagged(&response.body, uncountered);
    }
    // The breakdown carries the anti-misread legend (blocklist-clean; AC-SCORE-ANTIMISREAD).
    assert_score_legend_present_and_blocklist_clean(&response.body);
    // CARDINAL: the per-contribution subtotals STILL sum to the displayed pairing weight
    // with the flag present, and the countered contribution keeps its FULL subtotal
    // (AC-SCORE-SUMWEIGHT).
    assert_score_html_breakdown_sums_to_weight_with_flag(&response.body, &seeded.countered_cids);
}

/// SF-2 (US-CF-002 — presence-only GOLD; AC-SCORE-PRESENCE): a contribution countered by
/// TWO DISTINCT authors shows EXACTLY ONE neutral "Countered" marker on its `/score`
/// breakdown row — a PRESENCE marker, NEVER "countered by 2", never a count, never a
/// merged verdict. The per-counter attribution lives in the slice-11 thread the marker
/// LINKS to, not on the breakdown.
///
/// Given Maria's scored contributor has one contribution countered by two distinct
///   authors;
/// When Maria opens that contributor's Score breakdown;
/// Then the contribution row shows EXACTLY ONE neutral "Countered" marker, and the
///   breakdown shows no count, no "countered by N", and no aggregate verdict.
///
/// @us-cf-002 @driving_port @real-io @presence-only @anti-merging @gold
#[test]
fn a_contribution_with_two_counters_shows_one_neutral_presence_marker_on_the_score() {
    let env = TestEnv::initialized();
    let seeded = seed_score_breakdown_target_two_counters_distinct_authors(&env);
    let target_cid = seeded
        .countered_cids
        .first()
        .expect("SF-2: the seed yields exactly one twice-countered contribution")
        .clone();
    let server = ViewerServer::start(&env);

    let response = server.get(&format!("/score?contributor={}", seeded.contributor_did));

    assert_eq!(
        response.status, 200,
        "SF-2: GET /score must be 200; body was:\n{}",
        response.body
    );

    // The twice-countered contribution shows EXACTLY ONE neutral "Countered" marker
    // (presence-only — DISTINCT referenced_cid collapses the two distinct-author counters
    // to one membership), and the body carries NO count / "countered by N" / verdict
    // phrasing (AC-SCORE-PRESENCE).
    assert_score_flag_is_single_neutral_presence(&response.body, &target_cid);
    // The marker links to the slice-11 thread where the two counters are individually
    // attributed (AC-002-LINK).
    assert_score_flag_links_to_thread(&response.body, &target_cid);
    // The un-countered contribution rows carry NO marker (presence-only + additive).
    for uncountered in &seeded.uncountered_cids {
        assert_score_row_not_flagged(&response.body, uncountered);
    }
    // The marker is orthogonal to the score: its subtotal, the pairing weight, and the
    // pairing's rank are unchanged (the subtotals still sum to the weight; AC-SCORE-PRESENCE
    // unchanged-rank half).
    assert_score_html_breakdown_sums_to_weight_with_flag(&response.body, &seeded.countered_cids);
}

/// SF-3 (US-CF-002 — the CARDINAL sum-to-weight on a FLAGGED breakdown;
/// AC-SCORE-SUMWEIGHT): on a breakdown WHERE one contribution carries the "Countered"
/// marker, the displayed per-contribution subtotals STILL sum to the displayed pairing
/// weight, and the countered contribution's subtotal is its FULL original value (the
/// counter subtracts nothing). REUSES/adapts the slice-09 reproduce-by-hand parser on the
/// FLAGGED render (markers elided so the subtotal parse is unaffected).
///
/// Given Maria's contributor breakdown has a pairing whose contributions include a
///   countered one and an un-countered one;
/// When she opens that contributor's Score breakdown;
/// Then the pairing weight is unchanged, the displayed subtotals still sum to the
///   displayed weight, and the countered contribution's subtotal is its FULL original
///   value.
///
/// @us-cf-002 @driving_port @real-io @cardinal-sum-to-weight @shown-never-applied @gold
#[test]
fn the_per_contribution_subtotals_still_sum_to_the_pairing_weight_with_the_flag() {
    let env = TestEnv::initialized();
    let seeded = seed_score_breakdown_one_contribution_countered(&env);
    let server = ViewerServer::start(&env);

    let response = server.get(&format!("/score?contributor={}", seeded.contributor_did));

    assert_eq!(
        response.status, 200,
        "SF-3: GET /score must be 200; body was:\n{}",
        response.body
    );

    // The countered contribution carries its marker (the breakdown IS flagged) ...
    for countered in &seeded.countered_cids {
        assert_score_row_flagged_countered(&response.body, countered);
    }
    // ... yet the per-contribution subtotals STILL sum to the displayed pairing weight
    // (the counter subtracts nothing — the countered contribution's subtotal is its FULL
    // original value). This is the slice-14 CARDINAL (AC-SCORE-SUMWEIGHT): the reproduce-
    // by-hand gate holds on a FLAGGED render.
    assert_score_html_breakdown_sums_to_weight_with_flag(&response.body, &seeded.countered_cids);
    // The anti-misread legend is present (the breakdown is scored + flagged).
    assert_score_legend_present_and_blocklist_clean(&response.body);
}

/// SF-4 (US-CF-002 — the CARDINAL byte-identity vs slice-09; AC-SCORE-BYTEID): adding the
/// flag changes NO weight, ranking, or row order versus the slice-09 baseline. With the
/// "Countered" markers AND the anti-misread legend elided, the `/score` render is
/// byte-identical to slice-09 — every displayed weight, confidence, author bonus,
/// triangulation bonus, subtotal, headline total, bucket, the pairing ranking, and the
/// contribution row order. Uses the slice-12/13 baseline+marker-elision tactic extended
/// to also elide the legend.
///
/// Given Maria's contributor breakdown renders several pairings, a known subset of whose
///   contributions are countered;
/// When she opens that contributor's Score breakdown;
/// Then exactly the countered contributions show the marker and every other contribution
///   renders exactly as slice-09, and with the markers + legend elided every weight /
///   confidence / bonus / subtotal / total / bucket / ranking / row order is byte-identical
///   to the slice-09 render.
///
/// @us-cf-002 @driving_port @real-io @no-regression @cardinal @shown-never-applied @gold
#[test]
fn adding_the_score_flag_changes_no_weight_ranking_or_row_order_versus_slice09() {
    let env = TestEnv::initialized();
    let seeded = seed_score_breakdown_many_pairings_known_countered_subset(&env);
    let server = ViewerServer::start(&env);

    let response = server.get(&format!("/score?contributor={}", seeded.contributor_did));

    assert_eq!(
        response.status, 200,
        "SF-4: GET /score must be 200; body was:\n{}",
        response.body
    );

    // Exactly the countered contributions show the marker; every un-countered contribution
    // renders exactly as slice-09.
    for countered in &seeded.countered_cids {
        assert_score_row_flagged_countered(&response.body, countered);
    }
    for uncountered in &seeded.uncountered_cids {
        assert_score_row_not_flagged(&response.body, uncountered);
    }

    // CARDINAL byte-identity gold (AC-SCORE-BYTEID / I-CF-9): with the additive markers AND
    // the anti-misread legend elided, the render is byte-identical to the slice-09 baseline
    // — every weight/confidence/bonus/subtotal/total/bucket/ranking/row order unchanged.
    assert_score_render_byte_identical_to_slice09(&response.body, &seeded.ordered_cids);
}

/// SF-5 (US-CF-002 — anti-misread copy; AC-SCORE-ANTIMISREAD): the flag is shown for the
/// reader to judge and is unmistakably orthogonal to the score. Two contributions in the
/// same pairing with IDENTICAL confidence + bonuses render IDENTICAL subtotals — one
/// countered, one not — and only the countered one shows the marker; the breakdown carries
/// the plain-language legend; the copy NEVER contains "disputed"/"refuted"/"false"/
/// "penalty"/"deduction"/"lowered"/"disputed score".
///
/// Given two contributions in the same pairing have identical confidence and bonuses, but
///   one is countered and one is not;
/// When Maria opens that contributor's Score breakdown;
/// Then both contributions render the IDENTICAL subtotal (the counter subtracts nothing),
///   only the countered one shows the "Countered" marker, the breakdown carries the
///   plain-language legend, and the copy uses none of the verdict/penalty words.
///
/// @us-cf-002 @driving_port @real-io @anti-misread @shown-never-applied @gold
#[test]
fn two_identical_subtotal_contributions_render_identically_only_one_flagged() {
    let env = TestEnv::initialized();
    let seeded = seed_score_breakdown_identical_subtotals_one_countered(&env);
    let countered_cid = seeded
        .countered_cids
        .first()
        .expect("SF-5: the seed yields exactly one countered contribution with a twin")
        .clone();
    let server = ViewerServer::start(&env);

    let response = server.get(&format!("/score?contributor={}", seeded.contributor_did));

    assert_eq!(
        response.status, 200,
        "SF-5: GET /score must be 200; body was:\n{}",
        response.body
    );

    // Only the countered one shows the marker; its identical-subtotal twin shows none.
    assert_score_row_flagged_countered(&response.body, &countered_cid);
    for uncountered in &seeded.uncountered_cids {
        assert_score_row_not_flagged(&response.body, uncountered);
    }

    // Both contributions render the IDENTICAL subtotal (the counter subtracts nothing) and
    // the subtotals still sum to the weight — the marker is orthogonal to the number. The
    // sum-to-weight guard proves the countered contribution kept its FULL original value;
    // its identical-subtotal twin is in `uncountered_cids`, so the per-row subtotals are
    // equal and both fold into the same displayed weight.
    assert_score_html_breakdown_sums_to_weight_with_flag(&response.body, &seeded.countered_cids);

    // The breakdown carries the plain-language anti-misread legend, and the WHOLE rendered
    // body is blocklist-clean — never "disputed"/"refuted"/"false"/"penalty"/"deduction"/
    // "lowered"/"disputed score" (AC-SCORE-ANTIMISREAD).
    assert_score_legend_present_and_blocklist_clean(&response.body);
}

/// SF-6 (US-CF-002 — no-noise + byte-identity baseline; AC-002-NO-NOISE / AC-SCORE-BYTEID):
/// a contributor with NO countered contributions renders `/score` with no markers — every
/// contribution renders exactly as slice-09, no "0 counters" noise — and (since there is
/// nothing to flag) the breakdown is byte-identical to the slice-09 baseline with the
/// additive legend elided. `counter_presence_for` returns the EMPTY set -> no row is
/// flagged.
///
/// Given Maria's scored contributor has NO contributions countered at all;
/// When she opens that contributor's Score breakdown;
/// Then every contribution renders as in slice-09, no "Countered" marker and no
///   empty-state noise, and the render is byte-identical to the slice-09 baseline (legend
///   elided).
///
/// @us-cf-002 @driving_port @real-io @no-noise @empty-set @no-regression @happy
#[test]
fn a_contributor_with_no_countered_contributions_renders_score_with_no_markers() {
    let env = TestEnv::initialized();
    let seeded = seed_score_breakdown_none_countered(&env);
    let server = ViewerServer::start(&env);

    let response = server.get(&format!("/score?contributor={}", seeded.contributor_did));

    assert_eq!(
        response.status, 200,
        "SF-6: GET /score must be 200; body was:\n{}",
        response.body
    );

    // Every contribution renders as in slice-09: NO marker on any row, no empty-state
    // noise. With NO counters, the flag text appears NOWHERE.
    for cid in &seeded.uncountered_cids {
        assert_score_row_not_flagged(&response.body, cid);
    }
    assert!(
        !response.body.contains(&format!(
            "<a href=\"/claims/{}\">{LIST_COUNTERED_FLAG_TEXT}</a>",
            seeded.uncountered_cids.first().map(String::as_str).unwrap_or("")
        )) && !response.body.contains(&format!(">{LIST_COUNTERED_FLAG_TEXT}</a>")),
        "SF-6: a contributor with NO countered contributions must carry NO \
         {LIST_COUNTERED_FLAG_TEXT:?} flag anchor anywhere on /score (empty presence set -> \
         nothing rendered; AC-002-NO-NOISE); body was:\n{}",
        response.body
    );

    // With nothing flagged, the additive legend elided, the render is byte-identical to the
    // slice-09 baseline (no flag re-ordered/re-ranked anything; AC-SCORE-BYTEID baseline).
    assert_score_render_byte_identical_to_slice09(&response.body, &seeded.ordered_cids);
}

/// SF-7 (US-CF-002 — htmx fragment + no-JS full-page parity; AC-002-PARITY /
/// AC-SCORE-LOCAL): `GET /score?contributor=<did>` WITH `HX-Request` serves the
/// `#score-results` fragment WITH the flag; WITHOUT `HX-Request` serves a COMPLETE full
/// page whose score region renders the SAME flags — parity by construction (the page
/// EMBEDS the same `render_score_results_fragment` fn; the flag + legend live INSIDE the
/// fragment). Both render fully with NO network access (the presence read is a LOCAL
/// indexed lookup; the page references only the vendored local /static/htmx.min.js).
///
/// Given Maria's scored contributor has one countered contribution and the network is
///   down;
/// When she requests her Score breakdown WITH HX-Request and again WITHOUT it;
/// Then both render fully with no network, the htmx response is the score fragment with
///   the flag, and the no-JS response is the full page = chrome + the SAME fragment, with
///   the flag rendered identically.
///
/// @us-cf-002 @driving_port @real-io @no-js @full-page @parity @local-offline @happy
#[test]
fn the_score_flag_renders_identically_under_htmx_and_no_js() {
    let env = TestEnv::initialized();
    let seeded = seed_score_breakdown_one_contribution_countered(&env);
    let server = ViewerServer::start(&env);

    // WHEN the breakdown renders as a full page (no JS) AND as an htmx fragment over the
    // SAME store.
    let route = format!("/score?contributor={}", seeded.contributor_did);
    let full = server.get(&route);
    let fragment = server.get_htmx(&route);

    for (label, response) in [("full page", &full), ("fragment", &fragment)] {
        assert_eq!(
            response.status, 200,
            "SF-7: GET /score ({label}) must be 200; body was:\n{}",
            response.body
        );
        assert!(
            response.content_type.contains("text/html"),
            "SF-7: GET /score ({label}) must serve text/html; got {:?}",
            response.content_type
        );
    }

    // The no-JS request is a COMPLETE full page (chrome present); the htmx request is ONLY
    // the swap-target fragment (no chrome).
    assert!(
        full.is_full_page(),
        "SF-7: GET /score WITHOUT HX-Request must return a COMPLETE full page (chrome \
         present); body was:\n{}",
        full.body
    );
    assert!(
        fragment.is_fragment(),
        "SF-7: GET /score WITH HX-Request must return ONLY the #score-results fragment (no \
         chrome); body was:\n{}",
        fragment.body
    );

    // THEN: BOTH shapes carry the SAME "Countered" marker on the countered contribution —
    // parity by construction (the full page EMBEDS the same fragment fn; AC-002-PARITY).
    // NEITHER shape flags an un-countered contribution, and BOTH carry the legend.
    for countered in &seeded.countered_cids {
        assert_score_row_flagged_countered(&full.body, countered);
        assert_score_row_flagged_countered(&fragment.body, countered);
    }
    for uncountered in &seeded.uncountered_cids {
        assert_score_row_not_flagged(&full.body, uncountered);
        assert_score_row_not_flagged(&fragment.body, uncountered);
    }
    assert_score_legend_present_and_blocklist_clean(&full.body);
    assert_score_legend_present_and_blocklist_clean(&fragment.body);
}

// =============================================================================
// US-CF-001 — the N+1-flatten behavioral proxy on the SCORING surface (SF-N1). The
// /score handler flattens EVERY Contribution.cid across EVERY WeightedPairing into ONE
// counter_presence_for call (ADR-051); this proxy pins the whole breakdown flags
// correctly in one request.
// =============================================================================

/// SF-N1 (US-CF-001 — N+1-flatten behavioral proxy; AC-001-ONE-CALL / AC-001-INVARIANT /
/// ADR-051): a LARGE `/score` breakdown with MANY pairings × MANY contributions and a
/// KNOWN countered subset flags EVERY countered contribution correctly — and only those —
/// in ONE request, with no per-pairing/per-contribution degradation. The at-this-layer
/// behavioral proxy for the single flattened presence call (the strict 1-query bound is a
/// DELIVER `adapter-duckdb` unit/property assertion; query count is not observable at the
/// subprocess AT layer). If the presence read were per-pairing or per-contribution, a
/// large multi-pairing breakdown would either degrade or mis-flag under the fan-out; this
/// proxy pins that the WHOLE breakdown is flagged correctly in one shot from the single
/// flattened call (ADR-051).
///
/// Given Maria's contributor breakdown holds MANY contributions across MANY pairings, a
///   known subset of which are countered;
/// When she opens that contributor's Score breakdown (ONE request);
/// Then EVERY countered contribution carries the marker and EVERY un-countered
///   contribution does not — the whole breakdown is flagged correctly in a single request,
///   ranking + row order unchanged.
///
/// @us-cf-001 @driving_port @real-io @batch-read @no-n-plus-1 @gold
#[test]
fn a_large_multi_pairing_breakdown_flags_every_countered_contribution_in_one_request() {
    let env = TestEnv::initialized();
    let seeded = seed_score_breakdown_many_pairings_known_countered_subset(&env);
    // Sanity: the proxy is only meaningful over a genuinely large multi-pairing breakdown
    // with a real countered subset AND un-countered contributions. Pin both so the seed
    // cannot silently shrink the breakdown (which would hollow out the N+1 proxy).
    assert!(
        !seeded.countered_cids.is_empty(),
        "SF-N1: the large breakdown must carry a non-empty countered subset; got {:?}",
        seeded.countered_cids
    );
    assert!(
        !seeded.uncountered_cids.is_empty(),
        "SF-N1: the large breakdown must carry un-countered contributions too (a MIXED \
         breakdown); got {:?}",
        seeded.uncountered_cids
    );

    let server = ViewerServer::start(&env);

    // WHEN Maria opens the breakdown — ONE GET request renders the whole multi-pairing
    // breakdown.
    let page = server.get(&format!("/score?contributor={}", seeded.contributor_did));

    assert_eq!(
        page.status, 200,
        "SF-N1: GET /score must be 200; body was:\n{}",
        page.body
    );

    // THEN in that SINGLE response EVERY countered contribution carries the marker and
    // EVERY un-countered contribution carries NONE — the whole multi-pairing breakdown is
    // flagged correctly in one request (the subprocess-layer behavioral proxy for the
    // ADR-051 single flattened presence call; the strict 1-query bound is the DELIVER
    // adapter test).
    for countered in &seeded.countered_cids {
        assert_score_row_flagged_countered(&page.body, countered);
    }
    for uncountered in &seeded.uncountered_cids {
        assert_score_row_not_flagged(&page.body, uncountered);
    }
    // And the ranking/row order + every number is byte-identical to slice-09 even at this
    // size (AC-SCORE-BYTEID).
    assert_score_render_byte_identical_to_slice09(&page.body, &seeded.ordered_cids);
}
