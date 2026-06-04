//! Slice-08 acceptance — the `openlore ui` NETWORK-SEARCH view (US-NS-001..004;
//! ADR-036/037/038).
//!
//! The `/search` route (DESIGN §Route and Handler Design): the operator picks a
//! dimension (object / contributor / subject) + a value on a GET form; the viewer
//! queries the slice-05 network index over HTTP (`org.openlore.appview.searchClaims`
//! via the REUSED `IndexQueryPort` + `HttpIndexQueryAdapter`), re-composes the flat
//! rows per-author with the REUSED pure `appview-domain::compose_results` (no merge,
//! counter kept), and renders the verified + attributed `NetworkSearchResult` as
//! HTML — a full page WITHOUT `HX-Request` and the same `#search-results` region
//! fragment WITH it (the slice-07 `Shape` fork). It persists NOTHING (WD-NS-7),
//! holds NO signing key (I-NS-1), and renders NO executable follow control (the
//! `openlore peer add <did>` affordance is guidance TEXT — following stays a CLI
//! action, WD-NS-3).
//!
//! Driving discipline (Mandate 1): scenarios enter through the REAL `openlore ui`
//! subprocess (`ViewerServer`) + in-test HTTP GET /search (with/without the
//! `HX-Request` header — the slice-07 `get`/`get_htmx` pair). The network index is
//! the ONLY mocked boundary — a REAL slice-05 `openlore-indexer serve` over a
//! seeded fixture corpus (`seed_network_index` → `ViewerServer::start_with_indexer`),
//! NOT a hand-rolled HTTP double; the verified/attributed rows come from the
//! production ingest+serve path. The local DuckDB store is REAL but UNTOUCHED by
//! `/search` (zero persistence — proven by the gold guardrail in
//! viewer_network_search_invariants.rs). NO scenario calls the `viewer-domain`
//! `render_search_*` fns directly (those are unit-level, exercised in DELIVER).
//!
//! Indexer-double mechanism (REUSED, not net-new): the slice-05 `IndexerHandle`
//! (`seed_network_index(env, fixture)`) spawns a REAL `openlore-indexer serve` over
//! a seeded `index.duckdb` on an ephemeral localhost port; the NEW
//! `ViewerServer::start_with_indexer` threads its `indexer_url()` to the spawned
//! `openlore ui` via the slice-05 `OPENLORE_INDEXER_URL` env-var seam (OD-NS-6 —
//! the SAME seam the `openlore search` CLI verb reads). Postures:
//!   - REACHABLE-with-results  → `start_with_indexer(env, seed_network_index(..))`
//!   - REACHABLE-zero-results  → a query no indexed author matched (typo'd object /
//!                               absent contributor) over a reachable index
//!   - UNREACHABLE/offline     → `start_with_unreachable_indexer(env, &ClosedIndexerPort)`
//!   - UNCONFIGURED            → `ViewerServer::start(env)` (OPENLORE_INDEXER_URL unset)
//! These drive the four `SearchState` arms (Results / NoResults / Unavailable / Form).
//!
//! Layer placement (nw-tdd-methodology Layered Test Discipline matrix): every
//! `/search` scenario is a layer-3/layer-5 subprocess + real-I/O test — EXAMPLE-only
//! (Mandate 9/11). Sad paths (no results, unreachable, unconfigured) are enumerated
//! explicitly, never PBT-generated at this layer.
//!
//! Build-before-run note (carry into the DELIVER roadmap, mirrors the slice-05/06/07
//! viewer + indexer ATs): `cargo test` does NOT rebuild a spawned binary
//! automatically — the roadmap/run MUST `cargo build` BOTH the `openlore` bin (the
//! viewer) AND the `openlore-indexer` bin (the seeded serve) before running these
//! ATs so `ViewerServer::start_with_indexer` spawns the CURRENT viewer over a
//! CURRENT indexer, not a stale one.
//!
//! Mandate 7 RED scaffolds: the ATs import nothing unbuilt at the Rust level (they
//! spawn the bins + HTTP), so they COMPILE now with `todo!()` bodies + the new
//! `start_with_indexer` seam (which compiles since it just spawns with an env var).
//! Each scenario body is `todo!()` → panics → classifies RED (MISSING_FUNCTIONALITY),
//! NOT BROKEN. They stay RED until DELIVER's per-scenario RED→GREEN→COMMIT cycles.
//!
//! Covers:
//! - US-NS-001 (walking skeleton, N-1): GET /search?object=<nsid> WITH HX-Request,
//!   reachable index with results → ONLY the verified+attributed #search-results
//!   fragment (the thinnest end-to-end thread: viewer → indexer HTTP → verified
//!   rows → HTML fragment).
//! - US-NS-002 (search-by-object, N-2..N-5): no-JS full page (form + results) +
//!   fragment-vs-full-page parity + identical-content-two-authors = TWO rows
//!   (anti-merging) + no-results guided empty state.
//! - US-NS-003 (contributor / subject, N-6..N-10): one developer's trail under one
//!   author DID + honesty footer; N-author subject survey grouped (no merge); absent
//!   contributor → named NO-suggestion empty state; both dimensions fork by Shape.
//! - US-NS-004 (trust + degradation, N-11..N-17): public-data framing up front;
//!   counter shown-not-applied; unreachable AND unconfigured index → fixed
//!   Unavailable notice (no leaked transport internals) in BOTH shapes; unfollowed-
//!   author row shows `openlore peer add <did>` guidance TEXT + NO executable follow
//!   control.
//
// SCAFFOLD: true

mod support;

#[allow(unused_imports)]
use support::*;

// The headline reproducible-builds object NSID the walking skeleton + object
// searches use (the slice-05 `ReproducibleBuildsNineAuthorsUnfollowed` corpus is
// keyed on it). Kept as one source of truth so the query value + the rendered-row
// assertions never drift.
const OBJECT_REPRODUCIBLE_BUILDS: &str = "org.openlore.philosophy.reproducible-builds";

// =============================================================================
// US-NS-001 — bootstrap the viewer's indexer-query capability (the walking
// skeleton; @infrastructure). N-1 is the thinnest end-to-end thread.
// =============================================================================

/// N-1 / WALKING SKELETON (US-NS-001; AC-001.1; the riskiest-assumption thread):
/// from a REACHABLE network index with results, `GET /search?object=<nsid>` WITH
/// the `HX-Request` header returns ONLY the `#search-results` fragment — verified +
/// attributed rows (per-author, each carrying `[verified]` + the author DID +
/// verbatim confidence), with NO full-page chrome. This is the thinnest complete
/// thread the slice can demo: viewer → indexer HTTP → verified rows → HTML fragment,
/// proving the read-only viewer can take on the new outbound network-query
/// capability while preserving the verified/attributed/PE invariants.
///
/// Given Maria opens her read-only viewer wired to a reachable network index that
///   holds verified reproducible-builds claims by unfollowed authors;
/// When she submits an object search WITH the htmx header
///   (`GET /search?object=org.openlore.philosophy.reproducible-builds`, HX-Request);
/// Then she receives ONLY the `#search-results` fragment (no chrome), with the
///   matching claims as per-author attributed rows, each `[verified]`.
///
/// @us-ns-001 @walking_skeleton @driving_port @driving_adapter @real-io
/// @htmx-fragment @search-state-results @i-ns-3 @i-ns-4 @i-ns-6 @kpi-av-1 @happy
#[test]
fn search_by_object_with_htmx_returns_only_the_verified_results_fragment() {
    // GIVEN a REAL `openlore-indexer serve` over the headline reproducible-builds
    // corpus (9 distinct authors, incl. UNFOLLOWED Priya), wired to the viewer via
    // OPENLORE_INDEXER_URL (start_with_indexer). The index is the ONLY mocked
    // boundary — a REAL slice-05 binary, not a hand-rolled double.
    //
    // WHEN Maria submits `GET /search?object=<reproducible-builds>` WITH the
    // HX-Request header (get_htmx).
    //
    // THEN the response is ONLY the `#search-results` fragment (`is_fragment()`,
    // NOT a full page), and it renders the matching claims as per-author attributed
    // rows — each carrying `[verified]` + the author DID + verbatim confidence — with
    // NO merged consensus row. (Observable rendered surface only.)
    let env = TestEnv::initialized();
    let indexer = seed_network_index(
        &env,
        NetworkIndexFixture::ReproducibleBuildsNineAuthorsUnfollowed,
    );
    let viewer = ViewerServer::start_with_indexer(&env, indexer);

    let response = viewer.get_htmx(&format!("/search?object={OBJECT_REPRODUCIBLE_BUILDS}"));

    assert_eq!(
        response.status, 200,
        "GET /search WITH HX-Request over a reachable seeded index must be 200; body:\n{}",
        response.body
    );
    assert!(
        response.is_fragment(),
        "the htmx shape must return ONLY the #search-results fragment (no full-page \
         chrome); body:\n{}",
        response.body
    );
    // Per-author attributed rows, each carrying [verified] + the author DID + the
    // verbatim confidence — Priya (UNFOLLOWED) is in the headline corpus.
    assert_search_html_every_row_verified_and_attributed(&response.body, &[PRIYA_DID]);
    assert_search_html_has_no_merged_consensus_row(&response.body);
}

/// N-1b (US-NS-001; AC-001.2/AC-001.3 — the capability holds no write/sign surface):
/// the walking-skeleton search runs over a reachable index AND the viewer process
/// exposes NO write/sign/subscribe route and renders NO sign control on the result
/// surface — the new outbound capability is a READ only (I-NS-1 / WD-NS-3). The
/// read-only STORE delta is the gold guardrail (invariants file); here the
/// user-facing "no sign/write control on the /search surface" contract is pinned.
///
/// Given the viewer is wired to a reachable index and renders object results;
/// When the `/search` results surface is inspected;
/// Then it renders no sign / publish / subscribe control (the capability is a
///   public-data READ; signing/following stays in the CLI).
///
/// @us-ns-001 @infrastructure @driving_port @real-io @read-only @i-ns-1 @happy
#[test]
fn the_search_capability_exposes_no_write_or_sign_surface() {
    // GIVEN a reachable index + the viewer rendering object results.
    // WHEN the rendered `/search` surface (full page) is inspected.
    // THEN it carries NO sign/publish/subscribe affordance — no `name="sign"`,
    // `Sign claim`, `Sign & publish`, `Subscribe`, `Follow` control (I-NS-1 /
    // WD-NS-3; the only "follow" on the surface is the render-only `openlore peer
    // add <did>` TEXT, asserted in N-17). The viewer holds no key (the no-key audit
    // is structural — xtask check-arch — and the STORE read-only delta is the gold
    // guardrail).
    let env = TestEnv::initialized();
    let indexer = seed_network_index(
        &env,
        NetworkIndexFixture::ReproducibleBuildsNineAuthorsUnfollowed,
    );
    let viewer = ViewerServer::start_with_indexer(&env, indexer);

    let response = viewer.get(&format!("/search?object={OBJECT_REPRODUCIBLE_BUILDS}"));

    assert_eq!(
        response.status, 200,
        "GET /search over a reachable seeded index must be 200; body:\n{}",
        response.body
    );
    // We are inspecting a REAL result surface (verified+attributed rows), not the
    // empty form — Priya (UNFOLLOWED) is in the headline corpus.
    assert_search_html_every_row_verified_and_attributed(&response.body, &[PRIYA_DID]);
    // …and that surface carries NO write/sign/subscribe affordance (I-NS-1 /
    // WD-NS-3 — the capability is a public-data READ; signing/following stays in
    // the CLI). The only "follow" surface is the render-only `openlore peer add
    // <did>` guidance TEXT (asserted in N-17), which is neither a control element
    // nor a bare ">Follow<" label.
    let lowered = response.body.to_ascii_lowercase();
    for banned in [
        "name=\"sign\"",
        "sign claim",
        "sign & publish",
        "sign &amp; publish",
        "subscribe",
        ">follow<",
    ] {
        assert!(
            !lowered.contains(&banned.to_ascii_lowercase()),
            "I-NS-1 / WD-NS-3: the `/search` surface must expose NO write/sign/\
             subscribe control; found {banned:?} in body:\n{}",
            response.body
        );
    }
}

// =============================================================================
// US-NS-002 — search by philosophy/object in the browser, attribution preserved
// (N-2 no-JS full page · N-3 parity · N-4 identical-content-two-authors ·
//  N-5 no-results guided empty state)
// =============================================================================

/// N-2 (US-NS-002 happy; AC-002.1/AC-002.2): `GET /search?object=<nsid>` WITHOUT the
/// `HX-Request` header serves the COMPLETE full page — the dimension form AND the
/// rendered per-author results region — the no-JS / bookmarkable path. Each row
/// shows the author DID, `[verified]`, and verbatim confidence (`0.85`, never
/// `0.9`/`90%`).
///
/// Given Maria opens `/search` (no JS) wired to a reachable index;
/// When she submits an object search (`?object=<reproducible-builds>`, no header);
/// Then she gets the COMPLETE full page with the dimension form AND per-author
///   attributed result rows, each `[verified]` with verbatim confidence.
///
/// @us-ns-002 @driving_port @driving_adapter @real-io @no-js @full-page
/// @search-state-results @i-ns-6 @i-ns-9 @happy
#[test]
fn search_by_object_without_htmx_returns_the_full_page_with_form_and_results() {
    // GIVEN a reachable index over the reproducible-builds corpus + the viewer.
    // WHEN `GET /search?object=<reproducible-builds>` is fetched WITHOUT the header (get).
    // THEN the response `is_full_page()` (DOCTYPE + <html> chrome), carries the
    // dimension form (a `<form ... action="/search">`), AND renders the per-author
    // attributed rows ([verified] + author DID + verbatim confidence). Verbatim
    // confidence is asserted on a fixture value (e.g. the corpus carries a `0.82`/
    // `0.88` row rendered byte-for-byte, never `0.9`/`90%` — I-NS-9).
    let env = TestEnv::initialized();
    let indexer = seed_network_index(
        &env,
        NetworkIndexFixture::ReproducibleBuildsNineAuthorsUnfollowed,
    );
    let viewer = ViewerServer::start_with_indexer(&env, indexer);

    let response = viewer.get(&format!("/search?object={OBJECT_REPRODUCIBLE_BUILDS}"));

    assert_eq!(
        response.status, 200,
        "GET /search WITHOUT HX-Request over a reachable seeded index must be 200; \
         body:\n{}",
        response.body
    );
    assert!(
        response.is_full_page(),
        "the no-JS shape must return the COMPLETE full page (DOCTYPE + <html> \
         chrome); body:\n{}",
        response.body
    );
    assert!(
        response.body.contains("action=\"/search\""),
        "the full page must carry the dimension form (a `<form ... \
         action=\"/search\">`); body:\n{}",
        response.body
    );
    // Per-author attributed rows — Priya (UNFOLLOWED) is in the headline corpus.
    assert_search_html_every_row_verified_and_attributed(&response.body, &[PRIYA_DID]);
    // Verbatim confidence: Priya's row is 0.82, rendered byte-for-byte (I-NS-9) —
    // never rounded to `0.9`/`90%`.
    assert!(
        response.body.contains("0.82"),
        "I-NS-9: confidence must render VERBATIM (the corpus carries a `0.82` row); \
         body:\n{}",
        response.body
    );
    assert!(
        !response.body.contains("90%") && !response.body.contains("0.9 "),
        "I-NS-9: confidence must NOT be rounded to `0.9`/`90%`; body:\n{}",
        response.body
    );
}

/// N-3 (US-NS-002 parity; AC-002.4 / I-NS-6): for the SAME object query, the htmx
/// fragment content equals the `#search-results` region of the full page — same
/// per-author rows, same author DIDs, same `[verified]` markers, same verbatim
/// confidence. The full page EMBEDS the fragment fn (ADR-037), so this asserts the
/// two shapes agree empirically in the running viewer (the parity Earned-Trust
/// check, mirroring slice-07 H-1c).
///
/// Given a reachable index over the reproducible-builds corpus;
/// When `/search?object=<nsid>` is fetched BOTH with and without `HX-Request`;
/// Then the fragment's load-bearing rendered content is also present in the full
///   page's results region (same rows + DIDs + markers + verbatim confidence).
///
/// @us-ns-002 @driving_port @real-io @parity @i-ns-6 @search-state-results @happy
#[test]
fn object_search_fragment_equals_the_full_page_results_region() {
    // GIVEN a reachable index over the reproducible-builds corpus + the viewer.
    // WHEN both shapes of `/search?object=<reproducible-builds>` are fetched
    // (get_htmx + get).
    // THEN the fragment `is_fragment()` and the full page `is_full_page()`, and the
    // fragment's load-bearing needles (an author DID, `[verified]`, a verbatim
    // confidence value) are ALSO present in the full page's results region — parity
    // asserted behaviorally on the observable text the operator sees (the full page
    // is chrome + form wrapped around the SAME fragment fn).
    let env = TestEnv::initialized();
    let indexer = seed_network_index(
        &env,
        NetworkIndexFixture::ReproducibleBuildsNineAuthorsUnfollowed,
    );
    let viewer = ViewerServer::start_with_indexer(&env, indexer);

    let path = format!("/search?object={OBJECT_REPRODUCIBLE_BUILDS}");
    let fragment = viewer.get_htmx(&path);
    let full_page = viewer.get(&path);

    assert!(
        fragment.is_fragment(),
        "the htmx shape must return ONLY the #search-results fragment; body:\n{}",
        fragment.body
    );
    assert!(
        full_page.is_full_page(),
        "the no-JS shape must return the COMPLETE full page; body:\n{}",
        full_page.body
    );
    // Parity (I-NS-6): every load-bearing needle the fragment renders — an author
    // DID, the `[verified]` marker, and the verbatim confidence (Priya's 0.82) —
    // is ALSO present in the full page's results region (the full page is chrome +
    // form wrapped around the SAME fragment fn, so parity is by construction).
    for needle in [PRIYA_DID, "[verified]", "0.82"] {
        assert!(
            fragment.body.contains(needle),
            "I-NS-6: the fragment must carry {needle:?}; body:\n{}",
            fragment.body
        );
        assert!(
            full_page.body.contains(needle),
            "I-NS-6: the full page's results region must carry the SAME {needle:?} \
             the fragment renders (parity by construction); body:\n{}",
            full_page.body
        );
    }
}

/// N-4 (US-NS-002 edge; AC-002.3 / I-NS-3): identical content by two DIFFERENT
/// authors renders as TWO rows under two author groups — NEVER one merged
/// "network consensus" row. The anti-merging guarantee at network scale, made
/// observable on the browser surface (the viewer REUSES the slice-05 per-author
/// `compose_results` — no second grouping path).
///
/// Given a reachable index where two unfollowed authors EACH claim the same
///   object (the deno dependency-pinning identical-content fixture);
/// When Maria submits that object search;
/// Then the results region shows TWO attributed rows (one per author DID) and NO
///   merged / faceless consensus row.
///
/// @us-ns-002 @driving_port @real-io @anti-merging @i-ns-3 @search-state-results @edge
#[test]
fn identical_content_two_authors_renders_two_rows_never_a_merged_row() {
    // GIVEN a reachable index over the deno dependency-pinning corpus — two
    // UNFOLLOWED authors (Priya + Sven) each asserting the SAME content
    // (NetworkIndexFixture::DenoDependencyPinningTwoUnfollowedAuthors).
    // WHEN Maria submits the matching object/subject search.
    // THEN the body attributes a row to BOTH Priya AND Sven
    // (assert_search_html_every_row_verified_and_attributed with both DIDs) and
    // carries NO merged consensus row (assert_search_html_has_no_merged_consensus_row).
    let env = TestEnv::initialized();
    let indexer = seed_network_index(
        &env,
        NetworkIndexFixture::DenoDependencyPinningTwoUnfollowedAuthors,
    );
    let viewer = ViewerServer::start_with_indexer(&env, indexer);

    // The deno corpus keys both authors' identical content on this object NSID
    // (`org.openlore.philosophy.dependency-pinning`).
    let response = viewer.get("/search?object=org.openlore.philosophy.dependency-pinning");

    assert_eq!(
        response.status, 200,
        "GET /search over the deno two-author seeded index must be 200; body:\n{}",
        response.body
    );
    // BOTH unfollowed authors get an attributed row — TWO author groups, each row
    // [verified]. The helper proves at least one [verified] marker per author DID.
    assert_search_html_every_row_verified_and_attributed(&response.body, &[PRIYA_DID, SVEN_DID]);
    // …and NO merged / faceless "network consensus" row collapses the two (I-NS-3 —
    // the viewer REUSES the slice-05 per-author `compose_results`, no second
    // grouping path).
    assert_search_html_has_no_merged_consensus_row(&response.body);
}

/// N-5 (US-NS-002 boundary; AC-002.5 / SearchState::NoResults): an object no
/// indexed author has claimed (a typo) renders a GUIDED plain-language "no claims
/// found" empty state — never a blank region or a crash. The viewer queries a
/// REACHABLE index that simply returns zero rows for the typo'd object.
///
/// Given a reachable index that holds reproducible-builds claims;
/// When Maria searches a typo'd object no author claimed (`...reprducible`);
/// Then the results region shows a plain-language "no claims found" guidance — not
///   a blank region, not a crash (status 200).
///
/// @us-ns-002 @driving_port @real-io @empty-state @search-state-noresults @boundary
#[test]
fn object_with_no_network_claims_renders_a_guided_empty_state() {
    // GIVEN a reachable index over the reproducible-builds corpus + the viewer.
    // WHEN Maria searches a typo'd object that no indexed author claimed
    // (`org.openlore.philosophy.reprducible` — the slice-05 AV near-match fixture).
    // THEN the response is status 200 (NOT a crash), is NOT blank (carries the
    // guided message), and the body contains a plain-language "no claims found"
    // guidance (the dimension-aware NoResults arm; the near-match suggestion is an
    // optional DESIGN nicety, OD-NS-3).
    let env = TestEnv::initialized();
    let indexer = seed_network_index(
        &env,
        NetworkIndexFixture::ReproducibleBuildsNineAuthorsUnfollowed,
    );
    let viewer = ViewerServer::start_with_indexer(&env, indexer);

    // A typo'd object NSID no indexed author claimed — the reachable index returns
    // zero rows → SearchState::NoResults.
    let typo = "org.openlore.philosophy.reprducible";
    let response = viewer.get(&format!("/search?object={typo}"));

    assert_eq!(
        response.status, 200,
        "GET /search for a no-match object over a reachable index must be 200 \
         (NoResults, not a crash); body:\n{}",
        response.body
    );
    assert!(
        !response.body.trim().is_empty(),
        "the NoResults render must NOT be a blank region; body:\n{}",
        response.body
    );
    // The guided plain-language empty state NAMES the queried value (the dimension-
    // aware NoResults arm) — never a fake row, never a blank region.
    assert!(
        response.body.contains("No claims found"),
        "the NoResults arm must render plain-language `No claims found` guidance; \
         body:\n{}",
        response.body
    );
    assert!(
        response.body.contains(typo),
        "the NoResults guidance must NAME the queried value {typo:?}; body:\n{}",
        response.body
    );
}

// =============================================================================
// US-NS-003 — search by contributor or subject in the browser
// (N-6 contributor trail + footer · N-7 contributor parity ·
//  N-8 subject N-author survey · N-9 subject parity ·
//  N-10 absent contributor no-suggestion empty state)
// =============================================================================

/// N-6 (US-NS-003 happy contributor; AC-003.2): a contributor search renders ONE
/// developer's verified trail under a SINGLE author DID, with the honesty footer
/// "one developer's reasoning trail, not a community consensus" — never a merged
/// row. `github:priya` resolves to `did:plc:priya-test#org.openlore.application`
/// (the slice-05 handle→DID resolver, reused).
///
/// Given a reachable index where did:plc:priya-test authors several verified claims;
/// When Maria selects the contributor dimension and submits `?contributor=github:priya`;
/// Then the results region shows Priya's verified claims under her single author
///   DID, with the "one developer's reasoning trail, not a community consensus" footer.
///
/// @us-ns-003 @driving_port @driving_adapter @real-io @search-state-results
/// @i-ns-3 @contributor @happy
#[test]
fn contributor_search_renders_one_authors_trail_with_honesty_footer() {
    // GIVEN a reachable index over the Priya-eight-claims corpus
    // (NetworkIndexFixture::PriyaEightClaimsSixSubjects).
    // WHEN Maria submits `GET /search?contributor=github:priya`.
    // THEN every rendered row is attributed to the SINGLE
    // `did:plc:priya-test#org.openlore.application` DID, each `[verified]`
    // (assert_search_html_every_row_verified_and_attributed with the one DID), the
    // body carries the honesty footer ("not a community consensus"), and there is
    // NO merged row.
    let _ = PRIYA_DID;
    let env = TestEnv::initialized();
    let indexer = seed_network_index(&env, NetworkIndexFixture::PriyaEightClaimsSixSubjects);
    let viewer = ViewerServer::start_with_indexer(&env, indexer);

    let response = viewer.get("/search?contributor=github:priya");

    assert_eq!(
        response.status, 200,
        "GET /search?contributor=github:priya over a reachable seeded index must be \
         200; body:\n{}",
        response.body
    );
    // Every rendered row is attributed to the SINGLE Priya app-identity DID
    // (`github:priya` resolves to `did:plc:priya-test#org.openlore.application` via
    // the REUSED slice-05 handle→DID resolver), each carrying `[verified]`.
    assert_search_html_every_row_verified_and_attributed(
        &response.body,
        &["did:plc:priya-test#org.openlore.application"],
    );
    // The honest-framing footer: one developer's trail is NOT a community consensus.
    assert!(
        response.body.contains("not a community consensus"),
        "the contributor render must carry the \"not a community consensus\" footer; \
         body:\n{}",
        response.body
    );
    // …and there is NO merged / faceless consensus row (the footer is a PROMISE, not
    // an aggregate verdict — the viewer REUSES the per-author `compose_results`).
    assert_search_html_has_no_merged_consensus_row(&response.body);
}

/// N-7 (US-NS-003 parity contributor; AC-003.5 / I-NS-6): the contributor search
/// forks by `Shape` — the htmx fragment content equals the full page's
/// `#search-results` region (same trail rows + author DID + footer). The
/// progressive-enhancement contract holds on the contributor dimension too.
///
/// Given a reachable index where did:plc:priya-test authors a verified trail;
/// When `/search?contributor=github:priya` is fetched both with and without `HX-Request`;
/// Then the fragment's load-bearing content (trail rows + author DID + footer) is
///   also present in the full page's results region.
///
/// @us-ns-003 @driving_port @real-io @parity @i-ns-6 @contributor @happy
#[test]
fn contributor_search_fragment_equals_the_full_page_results_region() {
    // GIVEN a reachable index over the Priya-eight-claims corpus + the viewer.
    // WHEN both shapes of `/search?contributor=github:priya` are fetched
    // (get_htmx + get).
    // THEN the fragment `is_fragment()`, the full page `is_full_page()`, and the
    // load-bearing needles (Priya's author DID, `[verified]`, the honesty footer)
    // are present in BOTH bodies (parity by construction).
    let env = TestEnv::initialized();
    let indexer = seed_network_index(&env, NetworkIndexFixture::PriyaEightClaimsSixSubjects);
    let viewer = ViewerServer::start_with_indexer(&env, indexer);

    let path = "/search?contributor=github:priya";
    let fragment = viewer.get_htmx(path);
    let full_page = viewer.get(path);

    assert!(
        fragment.is_fragment(),
        "the htmx shape must return ONLY the #search-results fragment; body:\n{}",
        fragment.body
    );
    assert!(
        full_page.is_full_page(),
        "the no-JS shape must return the COMPLETE full page; body:\n{}",
        full_page.body
    );
    // Parity (I-NS-6): every load-bearing needle the contributor fragment renders —
    // Priya's resolved author DID, the `[verified]` marker, and the honest-framing
    // footer — is ALSO present in the full page's results region (the full page is
    // chrome + form wrapped around the SAME fragment fn, so parity is by construction).
    for needle in [
        "did:plc:priya-test#org.openlore.application",
        "[verified]",
        "not a community consensus",
    ] {
        assert!(
            fragment.body.contains(needle),
            "I-NS-6: the contributor fragment must carry {needle:?}; body:\n{}",
            fragment.body
        );
        assert!(
            full_page.body.contains(needle),
            "I-NS-6: the full page's results region must carry the SAME {needle:?} \
             the fragment renders (parity by construction); body:\n{}",
            full_page.body
        );
    }
}

/// N-8 (US-NS-003 happy subject; AC-003.3 / I-NS-3): a subject search renders the
/// project's claims grouped BY AUTHOR — N distinct author groups, each row
/// `[verified]` — with NO merged "the network thinks X about bazel" consensus row.
/// The subject-survey anti-merging guarantee on the browser surface.
///
/// Given a reachable index where 5 DISTINCT authors claim something about a project;
/// When Maria selects the subject dimension and submits `?subject=github:bazelbuild/bazel`;
/// Then the results region shows the claims grouped by their distinct authors, each
///   `[verified]`, with no merged consensus row.
///
/// @us-ns-003 @driving_port @real-io @search-state-results @anti-merging @i-ns-3
/// @subject @happy
#[test]
fn subject_search_renders_n_author_groups_never_a_consensus_row() {
    // GIVEN a reachable index over the bazel-five-distinct-authors corpus
    // (NetworkIndexFixture::BazelFiveDistinctAuthors).
    // WHEN Maria submits `GET /search?subject=github:bazelbuild/bazel`.
    // THEN the body attributes a row to each of the 5 distinct author DIDs, each
    // `[verified]` (assert_search_html_every_row_verified_and_attributed over the 5
    // DIDs), and carries NO merged consensus row
    // (assert_search_html_has_no_merged_consensus_row).
    let env = TestEnv::initialized();
    let indexer = seed_network_index(&env, NetworkIndexFixture::BazelFiveDistinctAuthors);
    let viewer = ViewerServer::start_with_indexer(&env, indexer);

    let response = viewer.get("/search?subject=github:bazelbuild/bazel");

    assert_eq!(
        response.status, 200,
        "GET /search?subject=github:bazelbuild/bazel over a reachable seeded index \
         must be 200; body:\n{}",
        response.body
    );
    // Each of the 5 DISTINCT authors who claim something about bazel is attributed
    // its OWN row under its OWN app-identity DID, each `[verified]` — N author
    // groups, no merge (I-NS-3 / anti-merging on the subject dimension).
    assert_search_html_every_row_verified_and_attributed(
        &response.body,
        &[
            "did:plc:priya-test#org.openlore.application",
            "did:plc:sven-test#org.openlore.application",
            "did:plc:tobias-test#org.openlore.application",
            "did:plc:aanya-test#org.openlore.application",
            "did:plc:lena-test#org.openlore.application",
        ],
    );
    // …and there is NO merged "the network thinks X about bazel" consensus row — the
    // subject survey REUSES the per-author `compose_results` (no second grouping
    // path) and renders NO contributor footer either (subject is not contributor).
    assert_search_html_has_no_merged_consensus_row(&response.body);
    assert!(
        !response.body.contains("not a community consensus"),
        "the SUBJECT dimension must NOT render the contributor honesty footer (it is \
         contributor-specific); body:\n{}",
        response.body
    );
}

/// N-9 (US-NS-003 parity subject; AC-003.5 / I-NS-6): the subject search forks by
/// `Shape` — the htmx fragment equals the full page's `#search-results` region
/// (same author groups + markers). Progressive enhancement on the subject dimension.
///
/// Given a reachable index where 5 distinct authors claim something about a project;
/// When `/search?subject=github:bazelbuild/bazel` is fetched both with and without
///   `HX-Request`;
/// Then the fragment's load-bearing content (author groups + markers) is also
///   present in the full page's results region.
///
/// @us-ns-003 @driving_port @real-io @parity @i-ns-6 @subject @happy
#[test]
fn subject_search_fragment_equals_the_full_page_results_region() {
    // GIVEN a reachable index over the bazel-five-distinct-authors corpus + the viewer.
    // WHEN both shapes of `/search?subject=github:bazelbuild/bazel` are fetched
    // (get_htmx + get).
    // THEN the fragment `is_fragment()`, the full page `is_full_page()`, and the
    // load-bearing needles (an author DID, `[verified]`) are present in BOTH bodies.
    let env = TestEnv::initialized();
    let indexer = seed_network_index(&env, NetworkIndexFixture::BazelFiveDistinctAuthors);
    let viewer = ViewerServer::start_with_indexer(&env, indexer);

    let path = "/search?subject=github:bazelbuild/bazel";
    let fragment = viewer.get_htmx(path);
    let full_page = viewer.get(path);

    assert!(
        fragment.is_fragment(),
        "the htmx shape must return ONLY the #search-results fragment; body:\n{}",
        fragment.body
    );
    assert!(
        full_page.is_full_page(),
        "the no-JS shape must return the COMPLETE full page; body:\n{}",
        full_page.body
    );
    // Parity (I-NS-6): every load-bearing survey needle the subject fragment renders
    // — one of the distinct author DIDs and the `[verified]` marker — is ALSO present
    // in the full page's results region (the full page is chrome + form wrapped
    // around the SAME fragment fn, so parity is by construction).
    for needle in ["did:plc:priya-test#org.openlore.application", "[verified]"] {
        assert!(
            fragment.body.contains(needle),
            "I-NS-6: the subject fragment must carry {needle:?}; body:\n{}",
            fragment.body
        );
        assert!(
            full_page.body.contains(needle),
            "I-NS-6: the full page's results region must carry the SAME {needle:?} \
             the fragment renders (parity by construction); body:\n{}",
            full_page.body
        );
    }
}

/// N-10 (US-NS-003 edge; AC-003.4 / SearchState::NoResults): a contributor handle no
/// indexed author matches renders a NAMED plain-language empty state that names the
/// queried handle AND offers NO near-match suggestion (an absent contributor is not
/// a typo — the slice-05 `EmptyPolicy::NoSuggestion` precedent, reused).
///
/// Given a reachable index;
/// When Maria searches a contributor handle no indexed author matches
///   (`?contributor=github:nobody-here`);
/// Then the results region names the queried handle with a plain-language empty
///   state AND offers no near-match suggestion.
///
/// @us-ns-003 @driving_port @real-io @empty-state @search-state-noresults
/// @contributor @no-suggestion @edge
#[test]
fn absent_contributor_renders_a_named_no_suggestion_empty_state() {
    // GIVEN a reachable index (any corpus that does NOT contain the queried handle).
    // WHEN Maria submits `GET /search?contributor=github:nobody-here`.
    // THEN status 200 (no crash), the body NAMES the queried handle
    // (`github:nobody-here`), shows a plain-language "no claims for that
    // contributor" empty state, and offers NO near-match suggestion (no "did you
    // mean" phrasing — an absent contributor is not a typo).
    let env = TestEnv::initialized();
    let indexer = seed_network_index(&env, NetworkIndexFixture::PriyaEightClaimsSixSubjects);
    let viewer = ViewerServer::start_with_indexer(&env, indexer);

    let response = viewer.get("/search?contributor=github:nobody-here");

    assert_eq!(
        response.status, 200,
        "GET /search?contributor=github:nobody-here over a reachable seeded index \
         must be 200 (the NoResults arm, never a crash); body:\n{}",
        response.body
    );
    // The empty state NAMES the queried handle VERBATIM (`github:nobody-here`, the
    // handle the operator typed — never the resolved DID; AV-17) so she sees WHAT
    // was searched.
    assert!(
        response.body.contains("github:nobody-here"),
        "the NoResults empty state must NAME the queried contributor handle \
         (`github:nobody-here`); body:\n{}",
        response.body
    );
    // …with a plain-language "no claims found" empty state (the dimension-aware
    // NoResults arm) — not a blank region.
    assert!(
        response.body.contains("No claims found"),
        "the absent-contributor render must show a plain-language empty state; \
         body:\n{}",
        response.body
    );
    // …and offers NO near-match suggestion — an absent contributor is NOT a typo
    // (the slice-05 `EmptyPolicy::NoSuggestion` precedent), so no "did you mean"
    // phrasing appears.
    assert!(
        !response.body.to_ascii_lowercase().contains("did you mean"),
        "an absent contributor is not a typo — the empty state must offer NO \
         near-match suggestion (no \"did you mean\"); body:\n{}",
        response.body
    );
    // …and the page carries the dimension-selector form offering ALL THREE
    // dimensions (object / contributor / subject) so the operator can re-submit
    // along any dimension (the form is present on every `/search` render).
    assert!(
        response.body.contains("action=\"/search\""),
        "the absent-contributor page must carry the dimension form (a `<form ... \
         action=\"/search\">`); body:\n{}",
        response.body
    );
    for dimension_field in ["name=\"object\"", "name=\"contributor\"", "name=\"subject\""] {
        assert!(
            response.body.contains(dimension_field),
            "the dimension-selector form must offer the {dimension_field} input \
             (object / contributor / subject); body:\n{}",
            response.body
        );
    }
}

// =============================================================================
// US-NS-004 — trust a browser discovery: verified framing, counter shown-not-
// applied, honest degradation, CLI-only follow guidance
// (N-11 public-data framing · N-12 counter shown-not-applied ·
//  N-13 unreachable full page · N-14 unreachable fragment ·
//  N-15 unconfigured full page · N-16 unconfigured fragment ·
//  N-17 follow guidance text, no control)
// =============================================================================

/// N-11 (US-NS-004 happy trust framing; AC-004.1 / I-NS-5): the `/search` page
/// states UP FRONT — before any results — that discovery indexes only PUBLIC signed
/// claims verified before indexing, and that nothing private is read. Every result
/// row she later sees carries `[verified]` + the author DID.
///
/// Given Maria opens `/search` in her read-only viewer (reachable index);
/// When she views the page before/with results;
/// Then she reads the public-data framing (only public signed claims, verified
///   before indexing) before the results, and every result row carries `[verified]`
///   + the author DID.
///
/// @us-ns-004 @driving_port @real-io @public-data-framing @i-ns-5 @i-ns-4 @happy
#[test]
fn the_search_page_states_what_it_indexes_up_front() {
    // GIVEN a reachable index over the reproducible-builds corpus + the viewer.
    // WHEN Maria opens the full-page `/search?object=<reproducible-builds>`.
    // THEN the rendered body carries the public-data framing — a banner stating
    // discovery indexes only PUBLIC signed claims, verified before indexing, and
    // nothing private is read (I-NS-5) — AND every result row carries `[verified]` +
    // the author DID (assert_search_html_every_row_verified_and_attributed). The
    // framing precedes the results region in the rendered chrome (banner-before-
    // results — the full-page arm shows it on Form + Results).
    let env = TestEnv::initialized();
    let indexer = seed_network_index(
        &env,
        NetworkIndexFixture::ReproducibleBuildsNineAuthorsUnfollowed,
    );
    let viewer = ViewerServer::start_with_indexer(&env, indexer);

    let response = viewer.get(&format!("/search?object={OBJECT_REPRODUCIBLE_BUILDS}"));

    assert_eq!(
        response.status, 200,
        "GET full-page /search over a reachable seeded index must be 200; body:\n{}",
        response.body
    );
    assert!(
        response.is_full_page(),
        "the no-JS shape must return the COMPLETE full page (form + framing + \
         results); body:\n{}",
        response.body
    );
    // The public-data framing is stated UP FRONT (I-NS-5): discovery indexes only
    // PUBLIC signed claims, verified before indexing, and nothing private is read.
    for framing_needle in [
        "public signed claims",
        "verified before indexing",
        "nothing private",
    ] {
        assert!(
            response.body.contains(framing_needle),
            "N-11 (I-NS-5): the /search full page must state the public-data framing \
             up front — expected {framing_needle:?}; body:\n{}",
            response.body
        );
    }
    // The framing precedes the results region in the rendered chrome (banner-before-
    // results): the public-data notice appears earlier in the document than the
    // first result row's [verified] marker.
    let framing_at = response
        .body
        .find("public signed claims")
        .expect("framing needle present (asserted above)");
    let first_result_at = response
        .body
        .find("[verified]")
        .expect("at least one verified result row is rendered");
    assert!(
        framing_at < first_result_at,
        "N-11: the public-data framing must precede the results region (banner- \
         before-results); body:\n{}",
        response.body
    );
    // …and every result row she later sees carries [verified] + the author DID.
    assert_search_html_every_row_verified_and_attributed(&response.body, &[PRIYA_DID]);
}

/// N-12 (US-NS-004 edge; AC-004.3 / I-NS-3): a result row for a claim another author
/// COUNTERED shows the counter-annotation inline ("countered by <did>") — and the
/// original claim is STILL shown verbatim, NOT merged or over-ridden. The
/// counter-shown-not-applied guarantee on the browser surface (the viewer REUSES the
/// slice-05 counter render discipline — shown, never filtered/applied).
///
/// Given a reachable index holding a verified claim C that a later verified claim K
///   counters (the countered-claim-plus-counter fixture);
/// When the results render for the shared object;
/// Then the row for C shows the counter-annotation inline AND C is still shown
///   verbatim, not merged or over-ridden.
///
/// @us-ns-004 @driving_port @real-io @counter-shown-not-applied @i-ns-3
/// @search-state-results @edge
#[test]
fn a_countered_claim_is_shown_with_its_counter_never_applied() {
    // GIVEN a reachable index over the countered-claim-plus-counter corpus
    // (NetworkIndexFixture::CounteredClaimPlusCounter — C by Priya, countered by K
    // from Sven, same object).
    // WHEN Maria submits the matching object search.
    // THEN the body STILL renders C's attributed row (Priya, [verified]) AND shows
    // a counter-annotation inline naming the countering author (Sven / K). C is NOT
    // filtered, merged, or down-weighted (counter SHOWN, never APPLIED — I-NS-3).
    // The original claim is still attributed + verified (no merge).
    let env = TestEnv::initialized();
    let indexer = seed_network_index(&env, NetworkIndexFixture::CounteredClaimPlusCounter);
    let viewer = ViewerServer::start_with_indexer(&env, indexer);

    // The indexed `author_did`s carry the app-identity fragment (the SAME shape the
    // viewer renders): C is attributed to Priya, the counter (K) to Sven.
    let priya_app = format!("{PRIYA_DID}#org.openlore.application");
    let sven_app = format!("{SVEN_DID}#org.openlore.application");

    let response = viewer.get(&format!("/search?object={OBJECT_REPRODUCIBLE_BUILDS}"));

    assert_eq!(
        response.status, 200,
        "GET /search over the countered-claim-plus-counter index must be 200; body:\n{}",
        response.body
    );
    // The OD-AV-7 / I-NS-3 render gate on the browser surface: C's row is STILL
    // shown verbatim (Priya, the object, [verified]) AND carries an inline
    // counter-annotation naming the countering author (Sven). C is NOT filtered,
    // merged, or down-weighted — the counter is SHOWN, never APPLIED.
    assert_search_html_counter_shown_not_applied(
        &response.body,
        &priya_app,
        OBJECT_REPRODUCIBLE_BUILDS,
        &sven_app,
    );
    // Both rows survive as verified attributed results (anti-merging preserved —
    // C the countered + K the countering author both appear).
    assert_search_html_every_row_verified_and_attributed(&response.body, &[&priya_app, &sven_app]);
    assert_search_html_has_no_merged_consensus_row(&response.body);
}

/// N-13 (US-NS-004 error; AC-004.4 / I-NS-2 / SearchState::Unavailable): an
/// UNREACHABLE configured index degrades to the fixed plain-language `Unavailable`
/// notice on the FULL PAGE — "the network index is unavailable; your local store
/// views still work" — with NO leaked transport internals (no HTTP status, no
/// "connection refused", no raw URL, no stack trace) and NO crash/hang. The viewer
/// is pointed at a CLOSED localhost port (connect-refused by construction).
///
/// Given Maria's configured indexer is unreachable (a closed port);
/// When she submits a network search WITHOUT the htmx header (full page);
/// Then the full page shows the fixed "index unavailable; local store views still
///   work" notice with no leaked transport internals and no crash/hang.
///
/// @us-ns-004 @driving_port @real-io @network-failure @search-state-unavailable
/// @i-ns-2 @no-js @full-page @error
#[test]
fn unreachable_index_degrades_to_a_calm_full_page_notice() {
    // GIVEN the viewer wired to an UNREACHABLE index — a freed/closed localhost
    // port (ClosedIndexerPort::reserve → start_with_unreachable_indexer); connect
    // is refused by construction (no live serve, no hang).
    // WHEN Maria submits a full-page object search (get, no header).
    // THEN status 200 (a guided page, NOT a crash), the body states the index is
    // unavailable AND that local store views still work, and it leaks NO transport
    // internals (assert_search_html_leaks_no_transport_internals).
    let _: fn(&TestEnv, &ClosedIndexerPort) -> ViewerServer =
        ViewerServer::start_with_unreachable_indexer;
    todo!(
        "DELIVER N-13: ClosedIndexerPort::reserve(); \
         ViewerServer::start_with_unreachable_indexer; get(\"/search?object=...\"); \
         assert status 200, is_full_page(), body states index unavailable + local \
         store views still work, and assert_search_html_leaks_no_transport_internals"
    )
}

/// N-14 (US-NS-004 error; AC-004.4 / I-NS-2): the SAME unreachable degradation holds
/// in the FRAGMENT shape — `GET /search?object=...` WITH `HX-Request` returns ONLY
/// the `#search-results` fragment carrying the fixed `Unavailable` notice, with NO
/// leaked transport internals. The unit-variant render is identical across both
/// shapes (degradation parity — WD-NS-4).
///
/// Given Maria's configured indexer is unreachable (a closed port);
/// When she submits a network search WITH the htmx header (fragment);
/// Then the fragment carries the fixed unavailable notice with no leaked transport
///   internals.
///
/// @us-ns-004 @driving_port @real-io @network-failure @search-state-unavailable
/// @i-ns-2 @htmx-fragment @error
#[test]
fn unreachable_index_degrades_to_a_calm_fragment_notice() {
    // GIVEN the viewer wired to an UNREACHABLE index (closed port).
    // WHEN Maria submits a fragment object search (get_htmx, HX-Request).
    // THEN the response `is_fragment()` (no chrome), carries the fixed unavailable
    // notice (index unavailable + local store views still work), and leaks NO
    // transport internals (assert_search_html_leaks_no_transport_internals) — the
    // SAME notice as the full-page N-13 (degradation parity).
    todo!(
        "DELIVER N-14: ClosedIndexerPort + start_with_unreachable_indexer; \
         get_htmx(\"/search?object=...\"); assert is_fragment(), body states index \
         unavailable + local store works, assert_search_html_leaks_no_transport_internals"
    )
}

/// N-15 (US-NS-004 error; AC-004.4 / I-NS-2): an UNCONFIGURED index (the
/// `OPENLORE_INDEXER_URL` env var is UNSET) degrades to the SAME fixed `Unavailable`
/// notice on the FULL PAGE — WITHOUT attempting any network call — with no leaked
/// internals and no crash. (The `ViewerServer::start` path sets no indexer URL.)
///
/// Given Maria's viewer has NO indexer configured (the env var is unset);
/// When she submits a network search WITHOUT the htmx header (full page);
/// Then the full page shows the fixed unavailable notice (no network call
///   attempted) with no leaked internals and no crash.
///
/// @us-ns-004 @driving_port @real-io @unconfigured @search-state-unavailable
/// @i-ns-2 @no-js @full-page @error
#[test]
fn unconfigured_index_degrades_to_a_calm_full_page_notice() {
    // GIVEN the viewer started with NO indexer URL configured
    // (ViewerServer::start — OPENLORE_INDEXER_URL unset; the unconfigured arm yields
    // Unavailable WITHOUT a network call, US-NS-001 Ex 2 / I-NS-2).
    // WHEN Maria submits a full-page object search (get, no header).
    // THEN status 200 (no crash), the body shows the fixed unavailable notice
    // (index unavailable + local store views still work), and leaks NO transport
    // internals (assert_search_html_leaks_no_transport_internals).
    todo!(
        "DELIVER N-15: ViewerServer::start(env) (no indexer configured); \
         get(\"/search?object=...\"); assert status 200, is_full_page(), body states \
         index unavailable + local store works, assert_search_html_leaks_no_transport_internals"
    )
}

/// N-16 (US-NS-004 error; AC-004.4 / I-NS-2): the SAME unconfigured degradation holds
/// in the FRAGMENT shape — `GET /search?object=...` WITH `HX-Request` over a viewer
/// with no indexer configured returns ONLY the `#search-results` fragment carrying
/// the fixed unavailable notice, with no leaked internals. Degradation parity across
/// shapes for the unconfigured arm too.
///
/// Given Maria's viewer has NO indexer configured;
/// When she submits a network search WITH the htmx header (fragment);
/// Then the fragment carries the fixed unavailable notice with no leaked internals.
///
/// @us-ns-004 @driving_port @real-io @unconfigured @search-state-unavailable
/// @i-ns-2 @htmx-fragment @error
#[test]
fn unconfigured_index_degrades_to_a_calm_fragment_notice() {
    // GIVEN the viewer started with NO indexer URL configured (ViewerServer::start).
    // WHEN Maria submits a fragment object search (get_htmx, HX-Request).
    // THEN the response `is_fragment()`, carries the fixed unavailable notice, and
    // leaks NO transport internals — the SAME notice as the full-page N-15.
    todo!(
        "DELIVER N-16: ViewerServer::start(env) (no indexer); \
         get_htmx(\"/search?object=...\"); assert is_fragment(), body states index \
         unavailable + local store works, assert_search_html_leaks_no_transport_internals"
    )
}

/// N-17 (US-NS-004 edge; AC-004.5 / I-NS-1 / WD-NS-3): a result row by an author
/// Maria does NOT yet follow shows the `openlore peer add <did>` guidance TEXT (to
/// run in the CLI) and renders NO clickable follow / subscribe control — the viewer
/// stays read-only; following is a deliberate CLI action.
///
/// Given a reachable index returning a verified row by an unfollowed author;
/// When the row renders;
/// Then it shows the `openlore peer add <did>` guidance TEXT and NO clickable
///   follow / subscribe control.
///
/// @us-ns-004 @driving_port @real-io @follow-guidance @read-only @i-ns-1 @edge
#[test]
fn an_unfollowed_author_row_shows_cli_follow_guidance_text_only() {
    // GIVEN a reachable index over the reproducible-builds corpus — Priya
    // (did:plc:priya-test) is UNFOLLOWED.
    // WHEN Maria submits the object search and the row by the unfollowed author
    // renders.
    // THEN the body contains the `openlore peer add did:plc:priya-test` guidance
    // TEXT (the slice-05 follow path, render-only), AND renders NO executable follow
    // control — no `<button>`/`<form>`/`hx-*` "follow"/"subscribe" affordance
    // (`name="follow"`, `Subscribe`, `hx-post` follow). Following stays a CLI action
    // (WD-NS-3 / I-NS-1).
    let _ = PRIYA_DID;
    todo!(
        "DELIVER N-17: reachable reproducible-builds index; get the object search; \
         assert body contains \\\"openlore peer add did:plc:priya-test\\\" guidance TEXT; \
         assert body carries NO executable follow/subscribe control (no name=\\\"follow\\\", \
         Subscribe button, hx-post follow)"
    )
}
