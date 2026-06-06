//! Slice-09 acceptance — the `openlore ui` CONTRIBUTOR-SCORE view (US-CS-001..003;
//! ADR-039/040/041).
//!
//! The `/score` route (DESIGN §Route and Handler Design): the operator opens
//! `GET /score?contributor=<did>`; the viewer reads the contributor's LOCAL
//! attributed feed over the read-only DuckDB store
//! (`StoreReadPort::query_contributor_scoring_feed` — claims ∪ local peer_claims,
//! NO network — I-CS-5), runs the REUSED slice-04 PURE
//! `scoring::score(&feed, &ScoringConfig::DEFAULT)`, and renders the ranked
//! `WeightedView` as HTML: per pairing the verbatim weight + `WeightBucket` label
//! AND a per-claim breakdown TABLE (author DID + cid + verbatim base confidence +
//! bonuses + subtotal) whose running sum EQUALS the displayed weight
//! (reproduce-by-hand; KPI-GRAPH-3). `[SPARSE]` + the "treat as a lead, not a
//! conclusion" honesty line are PROJECTED from the pure core's
//! `WeightBucket::Sparse` (the viewer recomputes NO bucket — WD-CS-6); an empty
//! feed → the guided `NoClaims` state. Served as a full page WITHOUT `HX-Request`
//! and the SAME `#score-results` region fragment WITH it (the slice-07 `Shape`
//! fork). It persists NOTHING (I-CS-4), holds NO signing key (I-CS-1), and renders
//! NO write/sign/follow control (the score is a read + pure compute — WD-CS-3).
//!
//! Driving discipline (Mandate 1): scenarios enter through the REAL `openlore ui`
//! subprocess (`ViewerServer`) + in-test HTTP GET /score (with/without the
//! `HX-Request` header — the slice-07 `get`/`get_htmx` pair). The local DuckDB
//! store is REAL (seeded through the PRODUCTION federation write path — `peer add`
//! + `peer pull`), so the rows the viewer scores are produced by production code,
//! not hand-inserted (Pillar 3 / BR-VIEW-4). NO external/network boundary exists —
//! `/score` is LOCAL + OFFLINE (distinct from `/scrape`'s GitHub edge and
//! `/search`'s indexer edge). NO scenario calls the `viewer-domain` `render_score_*`
//! fns OR the `scoring` crate directly (those are unit/property-level, exercised in
//! DELIVER) — every assertion is on the rendered HTML the operator's browser shows.
//!
//! Seeding postures (the key harness piece — `support::seed_contributor_*`):
//!   - RICH trail   → `seed_contributor_rich_trail(env, CONTRIBUTOR_RICH_DID)`: a
//!                    contributor across DISTINCT subjects on the shared
//!                    reproducible-builds object → a real weight + a multi-row
//!                    breakdown (cross-project span lifts it OUT of sparse).
//!   - SPARSE       → `seed_contributor_sparse_trail(env, CONTRIBUTOR_SPARSE_DID)`:
//!                    one claim / one author / one subject → `[SPARSE]` regardless
//!                    of confidence magnitude.
//!   - EMPTY        → no seeding for `CONTRIBUTOR_EMPTY_DID` → guided `NoClaims`.
//!   - CONFLICTING  → `seed_contributor_conflicting_authors(env)`: two authors on
//!                    the SAME (subject, object) → two attributed rows (no merge).
//!
//! Layer placement (nw-tdd-methodology Layered Test Discipline matrix): every
//! `/score` scenario is a layer-3/layer-5 subprocess + real-I/O test — EXAMPLE-only
//! (Mandate 9/11). The sad paths (sparse, empty) are enumerated explicitly, never
//! PBT-generated at this layer (the generative exploration of the pure
//! render/score core is a layer-1/2 DELIVER concern). The Gate-2 sum-to-weight
//! PROPERTY lives in the slice-04 `scoring` suite + the DELIVER render units; here
//! it is pinned as the OBSERVABLE reproduce-by-hand contract on the rendered HTML.
//!
//! Build-before-run note (carry into the DELIVER roadmap, mirrors slice-06/07/08):
//! `cargo test` does NOT rebuild a spawned binary automatically — the roadmap/run
//! MUST `cargo build` the `openlore` bin (the viewer) before running these ATs so
//! `ViewerServer::start` spawns the CURRENT viewer, not a stale one. `/score` needs
//! NO second binary (unlike slice-08's indexer) — the score is a LOCAL read.
//!
//! Mandate 7 RED scaffolds: the ATs spawn the bin + HTTP, so they COMPILE now with
//! `todo!()` bodies + the new `seed_contributor_*` helpers (which compile — they
//! drive existing seeding seams or `todo!()` themselves). Each scenario body is
//! `todo!()` → panics → classifies RED (MISSING_FUNCTIONALITY), NOT BROKEN. They
//! stay RED until DELIVER's per-scenario RED→GREEN→COMMIT cycles.
//!
//! Covers:
//! - US-CS-001 (walking skeleton, C-1): GET /score?contributor=<did> WITH
//!   HX-Request for a rich-trail contributor → ONLY the `#score-results` fragment
//!   with the headline weight + the per-claim breakdown (the thinnest end-to-end
//!   thread: viewer → local graph read → pure scorer → HTML fragment).
//! - US-CS-002 (transparency, C-2..C-6): no-JS full page (form + score region) +
//!   fragment-vs-full-page parity + the breakdown TABLE renders WITH the weight
//!   (never opaque) + reproduce-by-hand (rendered subtotals sum to the rendered
//!   weight) + every row names author_did + cid + verbatim confidence + conflicting
//!   claims by different authors = TWO rows (anti-merging) + multiple pairings
//!   ranked.
//! - US-CS-003 (honesty + verbatim + empty, C-7..C-10): thin evidence → `[SPARSE]`
//!   + the honesty line regardless of confidence magnitude + confidence/weight
//!   verbatim + an unknown contributor → guided `NoClaims` (names the DID, no fake
//!   score) + sparse-vs-Strong decided by breadth not magnitude.
//
// SCAFFOLD: true

mod support;

#[allow(unused_imports)]
use support::*;

// =============================================================================
// US-CS-001 — bootstrap the viewer's local contributor-scoring read capability
// (the walking skeleton; @infrastructure). C-1 is the thinnest end-to-end thread.
// =============================================================================

/// C-1 / WALKING SKELETON (US-CS-001/US-CS-002; AC-002.1/AC-002.2; the
/// riskiest-assumption thread): from the LOCAL store, `GET /score?contributor=<did>`
/// WITH the `HX-Request` header for a rich-trail contributor returns ONLY the
/// `#score-results` fragment — the headline weight + `WeightBucket` label AND the
/// per-claim breakdown (each row naming its author DID + cid + verbatim
/// confidence), with NO full-page chrome. This is the thinnest complete thread the
/// slice can demo: viewer → LOCAL graph read → pure scorer → HTML fragment, proving
/// the read-only viewer can host a local-graph-read + pure-compute scoring
/// capability while preserving the transparency / read-only / local-first /
/// progressive-enhancement invariants — AND that the BREAKDOWN ships with the
/// number (the load-bearing J-002c thesis), not as later polish.
///
/// Given Maria's read-only viewer reads a LOCAL store holding a contributor's
///   signed claims across several projects;
/// When she scores that contributor WITH the htmx header
///   (`GET /score?contributor=did:plc:priya-test`, HX-Request);
/// Then she receives ONLY the `#score-results` fragment (no chrome), showing the
///   adherence weight + its bucket AND a per-claim breakdown attributing every
///   contribution to its author DID with the confidence rendered verbatim.
///
/// @us-cs-001 @us-cs-002 @walking_skeleton @driving_port @driving_adapter @real-io
/// @htmx-fragment @score-state-scored @i-cs-2 @i-cs-7 @i-cs-10 @kpi-graph-3 @happy
#[test]
fn score_a_rich_contributor_with_htmx_returns_only_the_breakdown_fragment() {
    // GIVEN a REAL local store seeded (production `peer add` + `peer pull` path)
    // with a RICH trail for the contributor — several distinct subjects on the
    // shared reproducible-builds object, so the pure scorer yields a real weight +
    // a multi-row breakdown (NOT sparse). NO network: `/score` reads the LOCAL store.
    //
    // WHEN Maria submits `GET /score?contributor=<priya>` WITH the HX-Request header
    // (get_htmx).
    //
    // THEN the response is ONLY the `#score-results` fragment (`is_fragment()`, NOT
    // a full page), rendering the headline weight + bucket AND the per-claim
    // breakdown — each contribution attributed to its author DID + carrying a
    // verbatim confidence. (Observable rendered surface only.)
    let env = TestEnv::initialized();
    seed_contributor_rich_trail(&env, CONTRIBUTOR_RICH_DID);
    let viewer = ViewerServer::start(&env);

    let response = viewer.get_htmx(&format!("/score?contributor={CONTRIBUTOR_RICH_DID}"));

    assert_eq!(
        response.status, 200,
        "C-1: GET /score for a rich contributor must return 200; body was:\n{}",
        response.body
    );
    assert!(
        response.content_type.contains("text/html"),
        "C-1: the score fragment must be served as text/html; content-type was {:?}",
        response.content_type
    );
    // WITH the HX-Request header the viewer returns ONLY the `#score-results`
    // fragment — no full-page chrome (I-CS-7 / I-HX-1).
    assert!(
        response.is_fragment(),
        "C-1: an HX-Request `/score` response must be ONLY the fragment (no chrome); \
         body was:\n{}",
        response.body
    );
    assert!(
        response.body_contains(SCORE_RESULTS_ID),
        "C-1: the fragment must carry the `#score-results` swap-target id; body was:\n{}",
        response.body
    );
    // The headline weight + per-claim breakdown: every contribution attributed to
    // the contributor's author DID, each confidence rendered VERBATIM (`0.86` not
    // `0.9`/`86%`).
    assert_score_html_breakdown_attributed_and_verbatim(
        &response.body,
        &[CONTRIBUTOR_RICH_DID],
        &["0.86", "0.90", "0.74", "0.62"],
    );
}

/// C-1b (US-CS-001; AC-001.2 — the capability holds no signing/identity/PDS
/// surface): the walking-skeleton score runs over the LOCAL store AND the viewer
/// process exposes NO write/sign/subscribe route and renders NO sign control on the
/// score surface — the new capability is a READ + pure compute only (I-CS-1 /
/// WD-CS-3). The read-only STORE delta is the gold guardrail (invariants file);
/// here the user-facing "no sign/write control on the /score surface" contract is
/// pinned.
///
/// Given the viewer renders a rich contributor's score over the local store;
/// When the `/score` score surface is inspected;
/// Then it renders no sign / publish / subscribe / follow control (scoring is a
///   read + pure compute; signing/following stays in the CLI).
///
/// @us-cs-001 @infrastructure @driving_port @real-io @read-only @i-cs-1 @happy
#[test]
fn the_score_capability_exposes_no_write_or_sign_surface() {
    // GIVEN a rich-trail local store + the viewer rendering the score (full page).
    // WHEN the rendered `/score` surface is inspected.
    // THEN it carries NO sign/publish/subscribe/follow affordance (I-CS-1 /
    // WD-CS-3); the viewer holds no key (the no-key audit is structural — xtask
    // check-arch — and the STORE read-only delta is the gold guardrail).
    let env = TestEnv::initialized();
    seed_contributor_rich_trail(&env, CONTRIBUTOR_RICH_DID);
    let viewer = ViewerServer::start(&env);

    let response = viewer.get(&format!("/score?contributor={CONTRIBUTOR_RICH_DID}"));

    assert_eq!(
        response.status, 200,
        "C-1b: GET /score for a rich contributor must return 200; body was:\n{}",
        response.body
    );
    // A REAL score surface rendered (the weight + per-claim breakdown), so the
    // no-write/sign assertion is over a populated score view, not an empty page.
    assert_score_html_breakdown_attributed_and_verbatim(
        &response.body,
        &[CONTRIBUTOR_RICH_DID],
        &["0.86", "0.90", "0.74", "0.62"],
    );
    // The score surface carries NO sign / publish / follow / subscribe control —
    // the new capability is a READ + pure compute only (I-CS-1 / WD-CS-3).
    assert_score_html_has_no_write_or_sign_control(&response.body);
}

/// C-1c (US-CS-001 Example 3 / AC-001.4 — scoring a contributor with no local
/// claims never crashes the viewer): the capability degrades an empty feed to the
/// guided state, never a crash/hang/network call. This is the infra-story
/// reliability pin; the user-facing empty-state RENDER is C-9 (US-CS-003).
///
/// Given the viewer reads a local store holding NO claims for the queried contributor;
/// When she scores that contributor;
/// Then the viewer responds with a guided state (200), never a crash/hang/stack trace.
///
/// @us-cs-001 @driving_port @real-io @score-state-no-claims @i-cs-5 @error
#[test]
fn scoring_a_contributor_with_no_local_claims_never_crashes_the_viewer() {
    // GIVEN an initialized store with NO rows for CONTRIBUTOR_EMPTY_DID.
    // WHEN she scores that contributor.
    // THEN the response is a calm 200 guided state — no 5xx, no hang, no stack trace.
    let env = TestEnv::initialized();
    let viewer = ViewerServer::start(&env);

    let response = viewer.get(&format!("/score?contributor={CONTRIBUTOR_EMPTY_DID}"));

    // A calm 200 — emptiness (and even a read error) degrades to the guided state,
    // never a 5xx / hang / panic (I-CS-5). A 5xx here would mean the breadth guard /
    // `scoring::score` was asked to bucket an empty feed.
    assert_eq!(
        response.status, 200,
        "C-5: GET /score for a contributor with no local claims must return a calm \
         200 guided state, never a 5xx; body was:\n{}",
        response.body
    );
    // The guided NoClaims state names the queried DID — never a blank region — and
    // leaks NO fabricated score and NO raw error/stack trace (the helper bans
    // \"panicked\" / \"RUST_BACKTRACE\" / \"thread 'main'\").
    assert_score_html_renders_no_claims(&response.body, CONTRIBUTOR_EMPTY_DID);
}

// =============================================================================
// US-CS-002 — see a contributor's transparent score + breakdown in the browser
// (C-2..C-6). The load-bearing transparency scenarios.
// =============================================================================

/// C-2 (US-CS-002; AC-002.1 — no-JS full page): `GET /score?contributor=<did>`
/// WITHOUT `HX-Request` serves a COMPLETE full page (chrome + the contributor form
/// + the score region) — the no-JS no-regression contract (I-CS-7 / KPI-HX-G1). The
/// full page is the contract; the htmx swap is a nicety.
///
/// Given Maria opens /score for a rich contributor in a plain browser (no JS);
/// When the page renders;
/// Then she gets a full page (chrome) carrying the contributor form AND the score
///   region with the weight + breakdown.
///
/// @us-cs-002 @driving_port @real-io @no-js @full-page @score-state-scored @i-cs-7
/// @happy
#[test]
fn score_a_rich_contributor_without_htmx_serves_a_full_page_with_form_and_score() {
    // GIVEN a rich-trail local store. WHEN `get` (no HX-Request). THEN
    // `is_full_page()` (chrome present) AND the body carries the contributor form
    // AND the score region (weight + breakdown).
    let env = TestEnv::initialized();
    seed_contributor_rich_trail(&env, CONTRIBUTOR_RICH_DID);
    let viewer = ViewerServer::start(&env);

    let response = viewer.get(&format!("/score?contributor={CONTRIBUTOR_RICH_DID}"));

    assert_eq!(
        response.status, 200,
        "C-2: GET /score (no HX-Request) for a rich contributor must return 200; \
         body was:\n{}",
        response.body
    );
    // WITHOUT the HX-Request header the viewer returns the COMPLETE full page — the
    // no-JS no-regression contract (I-CS-7 / KPI-HX-G1).
    assert!(
        response.is_full_page(),
        "C-2: a no-JS `/score` response must be a complete full page (chrome \
         present); body was:\n{}",
        response.body
    );
    // The full page carries the contributor form (the no-JS submit/re-submit path).
    assert!(
        response.body_contains("name=\"contributor\""),
        "C-2: the no-JS full page must carry the contributor form; body was:\n{}",
        response.body
    );
    // AND the score region — the weight + per-claim breakdown attributing every
    // contribution to its author DID with verbatim confidence.
    assert!(
        response.body_contains(SCORE_RESULTS_ID),
        "C-2: the full page must carry the `#score-results` region; body was:\n{}",
        response.body
    );
    assert_score_html_breakdown_attributed_and_verbatim(
        &response.body,
        &[CONTRIBUTOR_RICH_DID],
        &["0.86", "0.90", "0.74", "0.62"],
    );
}

/// C-3 (US-CS-002 Example 3 / AC-002.6 — fragment-vs-full-page parity): the SAME
/// `?contributor=<did>` request, once WITHOUT and once WITH `HX-Request`, yields a
/// full page and a fragment whose `#score-results` score region is STRUCTURALLY
/// IDENTICAL — parity by construction (the page EMBEDS the fragment fn; I-CS-7).
/// Only the chrome differs.
///
/// Given Maria scores a rich contributor;
/// When she requests the score with AND without the htmx header;
/// Then the htmx response is a fragment (no chrome) and the no-JS response is a full
///   page, and the score region is identical between them.
///
/// @us-cs-002 @driving_port @real-io @htmx-fragment @full-page @parity @i-cs-7 @happy
#[test]
fn the_score_fragment_and_full_page_render_the_same_score_region() {
    // GIVEN a rich-trail store. WHEN `get` and `get_htmx` for the SAME contributor.
    // THEN `get` is_full_page(), `get_htmx` is_fragment(), and the `#score-results`
    // region is the same in both (the full page embeds the fragment — parity by
    // construction; I-CS-7).
    let env = TestEnv::initialized();
    seed_contributor_rich_trail(&env, CONTRIBUTOR_RICH_DID);
    let viewer = ViewerServer::start(&env);

    let full = viewer.get(&format!("/score?contributor={CONTRIBUTOR_RICH_DID}"));
    let fragment = viewer.get_htmx(&format!("/score?contributor={CONTRIBUTOR_RICH_DID}"));

    assert_eq!(full.status, 200, "C-3: the no-JS request must return 200");
    assert_eq!(fragment.status, 200, "C-3: the htmx request must return 200");
    // The shapes differ only in chrome: the no-JS request is a full page, the
    // HX-Request response is the bare fragment (no chrome) — I-CS-7.
    assert!(
        full.is_full_page(),
        "C-3: the no-JS response must be a full page; body was:\n{}",
        full.body
    );
    assert!(
        fragment.is_fragment(),
        "C-3: the HX-Request response must be a bare fragment (no chrome); body \
         was:\n{}",
        fragment.body
    );
    // The fragment IS the `#score-results` region; the full page EMBEDS the SAME
    // fragment fn, so the fragment body appears verbatim inside the full page —
    // parity by construction (the score region is identical between them; I-CS-7).
    assert!(
        fragment.body.contains(SCORE_RESULTS_ID),
        "C-3: the fragment must carry the `#score-results` region; body was:\n{}",
        fragment.body
    );
    assert!(
        full.body.contains(fragment.body.trim()),
        "C-3: the full page's `#score-results` region must be identical to the \
         fragment (parity by construction; the page embeds the fragment fn). \
         fragment:\n{}\nfull page:\n{}",
        fragment.body,
        full.body
    );
}

/// C-4 (US-CS-002 / AC-002.2/AC-002.4 — the breakdown TABLE renders WITH the weight,
/// never an opaque number): every rendered pairing shows its weight + bucket AND a
/// per-claim breakdown table where each row names the contribution's author DID +
/// cid + verbatim base confidence — no score is shown without its breakdown
/// (I-CS-2; the J-002c thesis). The cardinal anti-opaque-number scenario.
///
/// Given Maria scores a rich contributor;
/// When the score renders;
/// Then each pairing's weight is accompanied by a per-claim breakdown naming every
///   contribution's author DID + cid + verbatim confidence — no opaque number.
///
/// @us-cs-002 @driving_port @real-io @score-state-scored @i-cs-2 @i-cs-10 @i-cs-6
/// @kpi-graph-2 @happy
#[test]
fn a_rich_contributor_score_renders_its_weight_with_a_per_claim_breakdown() {
    // GIVEN a rich-trail store. WHEN `get` for the contributor. THEN the body shows
    // the weight + bucket AND a per-claim breakdown attributing every contribution
    // to its author DID + cid, with verbatim confidence — never an opaque number
    // (assert_score_html_breakdown_attributed_and_verbatim + _names_cids).
    let env = TestEnv::initialized();
    seed_contributor_rich_trail(&env, CONTRIBUTOR_RICH_DID);
    // Read the production-recomputed claim cids from the store BEFORE the viewer
    // opens it (the running server holds the DuckDB lock); the rendered breakdown
    // must NAME these exact cids (I-CS-10).
    let seeded_cids = read_peer_claim_cids_for(&env, CONTRIBUTOR_RICH_DID);
    let viewer = ViewerServer::start(&env);

    let response = viewer.get(&format!("/score?contributor={CONTRIBUTOR_RICH_DID}"));

    assert_eq!(
        response.status, 200,
        "C-4: GET /score for a rich contributor must return 200; body was:\n{}",
        response.body
    );
    // The score region is present AND carries the headline weight (never a region
    // that omits the number it is decomposing).
    assert!(
        response.body_contains(SCORE_RESULTS_ID),
        "C-4: the response must carry the `#score-results` region; body was:\n{}",
        response.body
    );
    assert!(
        response.body_contains("Weight:"),
        "C-4: every rendered pairing must show its headline weight; body was:\n{}",
        response.body
    );
    // The breakdown attributes every contribution to its author DID AND renders the
    // VERBATIM base confidence of each seeded claim (`0.86`, never `0.9`/`86%`;
    // I-CS-6 / I-CS-10) — no opaque, faceless consensus number (I-CS-2).
    assert_score_html_breakdown_attributed_and_verbatim(
        &response.body,
        &[CONTRIBUTOR_RICH_DID],
        &["0.86", "0.90", "0.74", "0.62"],
    );
    // AND every breakdown row NAMES the contributing claim's cid (I-CS-10): the
    // weight + breakdown are projected from the SAME WeightedPairing, so the cids
    // the renderer emits are exactly the seeded peer_claims rows the local feed read
    // returned (read above, before the viewer locked the store).
    assert_score_html_breakdown_names_cids(&response.body, &seeded_cids);
}

/// C-5 / CARDINAL (US-CS-002 / AC-002.3 — reproduce-by-hand): the running sum of a
/// pairing's per-claim subtotals, READ FROM THE RENDERED HTML, EQUALS the displayed
/// weight (the J-002c release gate, KPI-GRAPH-3). The operator can reproduce the
/// number by hand from what she SEES — the headline weight is the sum of the
/// visible row subtotals, not a number she takes on faith.
///
/// Given Maria scores a rich contributor;
/// When the breakdown renders;
/// Then the running sum of the per-claim subtotals shown in the table equals the
///   displayed pairing weight (reproduce-by-hand).
///
/// @us-cs-002 @driving_port @real-io @score-state-scored @reproduce-by-hand @i-cs-2
/// @kpi-graph-3 @happy
#[test]
fn the_rendered_breakdown_subtotals_sum_to_the_displayed_weight() {
    // GIVEN a rich-trail store. WHEN `get` for the contributor. THEN parse the
    // rendered weight + the per-row subtotals out of the breakdown table and assert
    // Σ subtotal == the displayed weight (assert_score_html_breakdown_sums_to_
    // displayed_weight) — the cardinal reproduce-by-hand gate, asserted on the HTML.
    let env = TestEnv::initialized();
    seed_contributor_rich_trail(&env, CONTRIBUTOR_RICH_DID);
    let viewer = ViewerServer::start(&env);

    let response = viewer.get(&format!("/score?contributor={CONTRIBUTOR_RICH_DID}"));

    assert_eq!(
        response.status, 200,
        "C-5: GET /score for a rich contributor must return 200; body was:\n{}",
        response.body
    );
    assert!(
        response.body_contains(SCORE_RESULTS_ID),
        "C-5: the response must carry the `#score-results` region; body was:\n{}",
        response.body
    );
    // CARDINAL reproduce-by-hand (KPI-GRAPH-3): parse the displayed weight + the
    // per-row subtotals out of the rendered breakdown table and assert, for each
    // rendered pairing, that Σ subtotal == the displayed weight — over a non-trivial
    // multi-row rich trail (>1 contribution), read from the OBSERVABLE HTML only.
    assert_score_html_breakdown_sums_to_displayed_weight(&response.body);
}

/// C-6 (US-CS-002 Example 4 / AC-002.5 — conflicting claims both contribute,
/// attribution preserved): two DIFFERENT authors who claim the SAME subject embodies
/// the philosophy at DIFFERENT confidences both appear in the breakdown under their
/// OWN author DIDs — neither averaged nor merged into one faceless consensus row
/// (the anti-merging guarantee made VISIBLE; I-CS-2 / I-CS-10).
///
/// Given two different authors claim the same subject embodies the philosophy at
///   different confidences;
/// When Maria scores the contributor and the breakdown renders;
/// Then both claims appear as separate contributions under their own author DIDs,
///   neither averaged nor merged.
///
/// @us-cs-002 @driving_port @real-io @score-state-scored @anti-merging @i-cs-2
/// @i-cs-10 @kpi-graph-2 @boundary
#[test]
fn conflicting_claims_by_different_authors_render_as_two_attributed_rows() {
    // GIVEN a store seeded so two distinct authors assert the SAME (subject, object)
    // at different confidences (seed_contributor_conflicting_authors). WHEN `get`
    // for that contributor. THEN BOTH authors' contributions render as SEPARATE rows
    // under their own DIDs (assert_score_html_breakdown_attributed_and_verbatim with
    // both DIDs + both verbatim confidences) and NO merged/averaged consensus row.
    let env = TestEnv::initialized();
    let (own_did, peer_did) = seed_contributor_conflicting_authors(&env);
    let viewer = ViewerServer::start(&env);

    // Score the contributor whose feed holds BOTH the own claim AND the pulled peer
    // claim on the SAME (subject, reproducible-builds) — the contributor-scope feed
    // read returns both, and the pure scorer decomposes the one pairing into TWO
    // attributed rows.
    let response = viewer.get(&format!("/score?contributor={own_did}"));

    assert_eq!(
        response.status, 200,
        "C-6: GET /score for the conflicting-authors contributor must return 200; \
         body was:\n{}",
        response.body
    );
    assert!(
        response.body_contains(SCORE_RESULTS_ID),
        "C-6: the response must carry the `#score-results` region; body was:\n{}",
        response.body
    );
    // BOTH authors' contributions render as SEPARATE rows under their OWN author DIDs,
    // each carrying its verbatim base confidence (0.40 own + 0.55 peer) — neither
    // averaged nor merged into one faceless consensus row (anti-merging; the helper
    // also bans the merged-consensus phrasings). The two rows sit within the SAME
    // subject pairing (I-CS-2 / I-CS-10).
    assert_score_html_breakdown_attributed_and_verbatim(
        &response.body,
        &[&own_did, &peer_did],
        &["0.40", "0.55"],
    );
}

// =============================================================================
// US-CS-003 — trust a thin score honestly: sparse rendering, verbatim numbers,
// empty state (C-7..C-10).
// =============================================================================

/// C-7 (US-CS-003 Example 1 / AC-003.1 — thin evidence renders as sparse, not as
/// manufactured confidence): a contributor whose support for a philosophy is a
/// SINGLE claim by a single author renders the pairing `[SPARSE]` + the "treat as a
/// lead, not a conclusion" honesty line — and is NEVER labelled Strong, regardless
/// of how high the claim's confidence is (I-CS-3 / KPI-GRAPH-4). The load-bearing
/// epistemic-honesty scenario.
///
/// Given Maria scores a contributor whose support is a single claim by a single
///   author at a HIGH confidence (0.95);
/// When the score renders;
/// Then the pairing is marked `[SPARSE]` with the "treat as a lead" honesty line and
///   is not labelled Strong.
///
/// @us-cs-003 @driving_port @real-io @score-state-scored @sparse @i-cs-3
/// @kpi-graph-4 @happy
#[test]
fn a_thin_single_claim_contributor_renders_sparse_at_any_confidence() {
    // GIVEN a SPARSE local store for the contributor (one claim/one author/one
    // subject at 0.95 — seed_contributor_sparse_trail). WHEN `get` for that
    // contributor. THEN the pairing renders `[SPARSE]` + the honesty line and NOT
    // Strong, regardless of the high confidence (assert_score_html_renders_sparse_
    // honesty) — the breadth guard, not the magnitude, decides the bucket.
    let env = TestEnv::initialized();
    seed_contributor_sparse_trail(&env, CONTRIBUTOR_SPARSE_DID);
    let viewer = ViewerServer::start(&env);

    let response = viewer.get(&format!("/score?contributor={CONTRIBUTOR_SPARSE_DID}"));

    assert_eq!(
        response.status, 200,
        "C-7: GET /score for a thin (single-claim) contributor must return 200; \
         body was:\n{}",
        response.body
    );
    assert!(
        response.body_contains(SCORE_RESULTS_ID),
        "C-7: the response must carry the `#score-results` region; body was:\n{}",
        response.body
    );
    // The thin pairing renders `[SPARSE]` + the "based on N claim(s) by M author(s)
    // — treat as a lead, not a conclusion" honesty line (projected from the pure
    // core's WeightBucket::Sparse + claim_count/distinct_author_count) and is NEVER
    // labelled Strong, regardless of the HIGH (0.95) confidence — the breadth guard,
    // not the magnitude, decides the bucket (I-CS-3 / KPI-GRAPH-4 / WD-CS-6).
    assert_score_html_renders_sparse_honesty(&response.body);
}

/// C-8 (US-CS-003 Example 2 / AC-003.2 — confidence + weight shown verbatim): a
/// contributing claim's stored confidence renders byte-for-byte (`0.90`, never
/// `0.9` or `90%`), and the displayed pairing weight is the exact consumed value
/// (no bucket-midpoint rounding) — the same `render_confidence` verbatim contract
/// (I-CS-6 / KPI-4).
///
/// Given a contributing claim's stored confidence is 0.90;
/// When the breakdown renders;
/// Then the confidence is shown as "0.90" (never "0.9" or "90%") and the displayed
///   weight is the exact consumed value.
///
/// @us-cs-003 @driving_port @real-io @score-state-scored @verbatim @i-cs-6 @kpi-4
/// @edge
#[test]
fn the_score_breakdown_renders_confidence_and_weight_verbatim() {
    // GIVEN a rich-trail store whose seeded confidences include 0.90 (the
    // `github:NixOS/nixpkgs` claim). WHEN `get` (no-JS full page) AND `get_htmx`
    // (fragment) for the contributor. THEN BOTH shapes render "0.90" verbatim and
    // NOT a "%"-formatted value, AND the displayed weight is the exact consumed
    // value (reproduce-by-hand: Σ subtotal == weight, with no bucket-midpoint
    // rounding). The verbatim guarantee must hold in BOTH the fragment and the
    // full page (no divergence — single-site formatting: render_confidence + the
    // sibling render_weight).
    let env = TestEnv::initialized();
    seed_contributor_rich_trail(&env, CONTRIBUTOR_RICH_DID);
    let viewer = ViewerServer::start(&env);

    let full = viewer.get(&format!("/score?contributor={CONTRIBUTOR_RICH_DID}"));
    let fragment = viewer.get_htmx(&format!("/score?contributor={CONTRIBUTOR_RICH_DID}"));

    assert_eq!(
        full.status, 200,
        "C-8: GET /score (no HX-Request) for a rich contributor must return 200; \
         body was:\n{}",
        full.body
    );
    assert_eq!(
        fragment.status, 200,
        "C-8: GET /score (HX-Request) for a rich contributor must return 200; body \
         was:\n{}",
        fragment.body
    );
    // The two shapes differ ONLY in chrome (the full page embeds the fragment) —
    // so the verbatim guarantee asserted below must hold in BOTH.
    assert!(
        full.is_full_page(),
        "C-8: the no-JS response must be a full page; body was:\n{}",
        full.body
    );
    assert!(
        fragment.is_fragment(),
        "C-8: the HX-Request response must be a bare fragment; body was:\n{}",
        fragment.body
    );

    // The verbatim contract holds in BOTH shapes: the breakdown renders the seeded
    // confidence "0.90" byte-for-byte (via the shared render_confidence, I-CS-6),
    // never a "90%"-formatted value. Asserted over the full page AND the fragment
    // so a second formatting path (which could diverge between shapes) is excluded.
    for (shape, body) in [("full page", &full.body), ("fragment", &fragment.body)] {
        // The contributing claim seeded at 0.90 renders "0.90" verbatim (never
        // truncated, never percentized). The full verbatim confidence list is
        // pinned by C-1/C-4; here C-8 nails the 0.90 boundary specifically.
        assert!(
            body.contains("0.90"),
            "C-8 ({shape}): a claim stored at 0.90 must render \"0.90\" verbatim \
             (I-CS-6); body was:\n{body}"
        );
        // NEVER a "%"-formatted confidence/weight: no "90%" (and no "%" sign at all
        // in the score surface) — confidence + weight are decimals, not percents.
        assert!(
            !body.contains("90%"),
            "C-8 ({shape}): confidence/weight must NOT render as a percent (found \
             \"90%\"); body was:\n{body}"
        );
        assert!(
            !body.contains('%'),
            "C-8 ({shape}): the score surface must render NO \"%\" formatting — \
             confidence + weight are verbatim decimals (single-site render_confidence \
             / render_weight, no second percent path); body was:\n{body}"
        );
    }

    // The displayed pairing weight is the EXACT consumed WeightedPairing.weight with
    // NO bucket-midpoint rounding — pinned observably as reproduce-by-hand: the
    // running sum of the rendered per-claim subtotals EQUALS the rendered weight (the
    // weight is the verbatim consumed value, not a re-derived bucket midpoint). Holds
    // in BOTH the full page AND the fragment (no divergence — one weight formatter).
    assert_score_html_breakdown_sums_to_displayed_weight(&full.body);
    assert_score_html_breakdown_sums_to_displayed_weight(&fragment.body);
    // AND the full per-row identity (verbatim confidences + author attribution) holds
    // in both shapes — the SAME single-site verbatim render the breakdown uses.
    assert_score_html_breakdown_attributed_and_verbatim(
        &full.body,
        &[CONTRIBUTOR_RICH_DID],
        &["0.86", "0.90", "0.74", "0.62"],
    );
    assert_score_html_breakdown_attributed_and_verbatim(
        &fragment.body,
        &[CONTRIBUTOR_RICH_DID],
        &["0.86", "0.90", "0.74", "0.62"],
    );
}

/// C-9 (US-CS-003 Example 3 / AC-003.3 — an unknown contributor shows a guided
/// empty state, not a crash): a contributor with NO claims in the local store
/// renders the fixed plain-language "No local claims for that contributor." notice
/// in BOTH shapes — naming the queried DID, with no blank region, no fabricated
/// score, no stack trace (OD-CS-6 / I-CS-5). Emptiness is recognized as emptiness,
/// never mistaken for a zero score.
///
/// Given Maria scores a contributor that has no claims in her local store;
/// When the score renders;
/// Then the score region shows the plain-language "no local claims" message naming
///   the queried DID, with no blank region and no stack trace.
///
/// @us-cs-003 @driving_port @real-io @score-state-no-claims @empty-state @i-cs-5
/// @od-cs-6 @error
#[test]
fn an_unknown_contributor_shows_a_guided_no_claims_state_not_a_crash() {
    // GIVEN an initialized store with NO rows for CONTRIBUTOR_EMPTY_DID. WHEN `get`
    // AND `get_htmx` for that contributor. THEN both shapes render the guided
    // NoClaims notice naming the queried DID, with no fabricated score and no leaked
    // error internals (assert_score_html_renders_no_claims) — the empty state in
    // both shapes.
    let _env = TestEnv::initialized();
    todo!(
        "slice-09 C-9 empty state: ViewerServer::start over an empty store; get + \
         get_htmx for /score?contributor={CONTRIBUTOR_EMPTY_DID}; \
         assert_score_html_renders_no_claims(body, CONTRIBUTOR_EMPTY_DID) for BOTH"
    )
}

/// C-10 (US-CS-003 Example 4 / AC-003.1 — sparse vs Strong decided by BREADTH, not
/// magnitude): two contributors land at a comparable raw weight, but one has
/// cross-project span (rich trail) and the other does not (single-claim sparse) —
/// the spanning one renders a non-sparse bucket and the thin one renders `[SPARSE]`.
/// The breadth guard, inherited from the pure core, decides the bucket; the browser
/// surface preserves it (I-CS-3 / WD-CS-5).
///
/// Given a rich-trail contributor (cross-project span) and a thin single-claim
///   contributor;
/// When each is scored;
/// Then the spanning contributor's pairing is NOT `[SPARSE]` while the thin one IS —
///   the breadth guard, not the weight magnitude, decides the bucket.
///
/// @us-cs-003 @driving_port @real-io @score-state-scored @sparse @breadth-guard
/// @i-cs-3 @kpi-graph-4 @boundary
#[test]
fn breadth_not_magnitude_decides_sparse_versus_strong_on_the_browser_surface() {
    // GIVEN both a rich-trail (CONTRIBUTOR_RICH_DID) and a sparse (CONTRIBUTOR_
    // SPARSE_DID) contributor seeded. WHEN each is scored over the same viewer.
    // THEN the rich contributor's pairing is NOT `[SPARSE]` (a real bucket) while the
    // sparse contributor's pairing IS `[SPARSE]` — decided by breadth, not magnitude.
    let env = TestEnv::initialized();
    // The RICH trail spans ≥2 projects on the shared object (cross_project_span ≥ 2);
    // the SPARSE trail is one claim/one author/one subject. BOTH on the SAME object,
    // so it is BREADTH (span), not weight magnitude, that separates the buckets. Both
    // are seeded in ONE federation pull (a single seed call keeps both peers' PDS
    // servers alive + wires both resolver seams for the one `peer pull`).
    seed_contributor_rich_and_sparse_trails(&env, CONTRIBUTOR_RICH_DID, CONTRIBUTOR_SPARSE_DID);
    let viewer = ViewerServer::start(&env);

    let rich = viewer.get(&format!("/score?contributor={CONTRIBUTOR_RICH_DID}"));
    let sparse = viewer.get(&format!("/score?contributor={CONTRIBUTOR_SPARSE_DID}"));

    assert_eq!(
        rich.status, 200,
        "C-10: GET /score for the rich (spanning) contributor must return 200; \
         body was:\n{}",
        rich.body
    );
    assert_eq!(
        sparse.status, 200,
        "C-10: GET /score for the sparse (single-claim) contributor must return 200; \
         body was:\n{}",
        sparse.body
    );
    // The SPANNING contributor (cross_project_span ≥ 2) renders its NON-Sparse bucket
    // — its pairing carries NO `[SPARSE]` marker (a real weight + multi-row
    // breakdown). Breadth lifted it out of Sparse.
    assert!(
        !rich.body.contains("[SPARSE]"),
        "C-10: a contributor spanning ≥2 projects must render a NON-Sparse bucket \
         (no `[SPARSE]` marker) — breadth lifts it out of Sparse; body was:\n{}",
        rich.body
    );
    assert_score_html_breakdown_attributed_and_verbatim(
        &rich.body,
        &[CONTRIBUTOR_RICH_DID],
        &["0.86", "0.90", "0.74", "0.62"],
    );
    // The NON-spanning contributor (single claim, no span) renders `[SPARSE]` + the
    // honesty line, regardless of its confidence magnitude — breadth (not magnitude)
    // decided the bucket in the pure core; the browser surface preserves it
    // (I-CS-3 / WD-CS-5 / KPI-GRAPH-4).
    assert_score_html_renders_sparse_honesty(&sparse.body);
}
