//! Slice-10 acceptance — the `openlore ui` GRAPH-TRAVERSAL views (US-GT-002/003/004;
//! ADR-042/043/044/045).
//!
//! The two new LOCAL read-only routes (DESIGN §Route and Handler Design): the
//! operator opens `GET /project?subject=<uri>` (the project survey) or
//! `GET /philosophy?object=<uri>` (the philosophy survey); the viewer reads the
//! entity's LOCAL attributed survey over the read-only DuckDB store
//! (`StoreReadPort::query_project_survey` / `query_philosophy_survey` — claims ∪
//! local peer_claims, `UNION ALL`, NO merge, NO network — I-GT-2), groups the
//! attributed claims in the PURE `viewer-domain` core into a `TraversalView` ADT
//! (`Found { groups, contributors } | NoClaims { entity }` — grouping in Rust, never
//! SQL, I-GT-3), and renders it as HTML: per group a traversal `<a href>` to the
//! OTHER dimension, and per edge row the attributed `author_did` + verbatim
//! `confidence` (`0.90`, never `0.9`/`90%`; I-GT-5) + the REUSED claim-domain
//! display-only bucket + the `cid` (every edge = one signed claim, I-GT-4). The
//! distinct contributors are listed as links to `/score?contributor=<bare-did>` (the
//! slice-09 terminus REUSED; bare-DID form, ADR-044 Q1). An empty / unknown entity →
//! the guided `NoClaims` state (200, names the entity, no fabricated edge). Served as
//! a full page WITHOUT `HX-Request` and the SAME `#traversal-results` region fragment
//! WITH it (the slice-07 `Shape` fork; page = chrome + fragment, parity by
//! construction — I-GT-6). It persists NOTHING (I-GT-8), holds NO signing key
//! (I-GT-1), and renders NO write/sign/follow control (traversal is a READ; the
//! cross-links are render-only `<a href>` navigation TEXT — WD-GT-3).
//!
//! US-GT-004 (cross-links) makes traversal a JOURNEY: the subject/object/contributor
//! cells on every survey row render as `<a href>` traversal edges — subject →
//! `/project`, object → `/philosophy`, contributor → `/score` (bare DID). Claim-
//! controlled subject/object URIs are PERCENT-ENCODED into the href query component
//! (the ADR-044 §security injection boundary): a hostile subject carrying `"`/`<`/
//! `&`/space cannot break out of the `href` attribute or smuggle a second query
//! param.
//!
//! Driving discipline (Mandate 1): scenarios enter through the REAL `openlore ui`
//! subprocess (`ViewerServer`) + in-test HTTP GET (with/without the `HX-Request`
//! header — the slice-07 `get`/`get_htmx` pair). The local DuckDB store is REAL,
//! seeded through the PRODUCTION federation write path (`peer add` + `peer pull` via
//! `seed_*_survey_trail` / `seed_two_author_same_edge` — the SAME seam slice-09
//! uses), so the rows the survey reads are produced by production code, not
//! hand-inserted (Pillar 3 / BR-VIEW-4). NO external/network boundary exists —
//! `/project` + `/philosophy` are LOCAL + OFFLINE (distinct from `/scrape`'s GitHub
//! edge and `/search`'s indexer edge; offline-STRONGER than `/search`, I-GT-2 /
//! I-GT-7). NO scenario calls the `viewer-domain` `render_*` / `group_*` fns directly
//! (those are unit/property-level, exercised in DELIVER) — every assertion is on the
//! rendered HTML the operator's browser shows (Mandate 8 universe = port-exposed
//! rendered surface).
//!
//! Layer placement (nw-tdd-methodology Layered Test Discipline matrix): every
//! traversal scenario is a layer-3/layer-5 subprocess + real-I/O test — EXAMPLE-only
//! (Mandate 9/11). The sad paths (no-claims, the injection URI) are enumerated
//! explicitly, never PBT-generated at this layer (the generative exploration of the
//! pure group/render core + the `encode_query_component` round-trip PROPERTY are a
//! layer-1/2 DELIVER concern).
//!
//! Build-before-run note (carry into the DELIVER roadmap, mirrors slice-06/07/08/09):
//! `cargo test` does NOT rebuild a spawned binary automatically — the roadmap/run
//! MUST `cargo build` the `openlore` bin (the viewer) before running these ATs so
//! `ViewerServer::start` spawns the CURRENT viewer, not a stale one. Traversal needs
//! NO second binary (unlike slice-08's indexer) — the survey is a LOCAL read.
//!
//! Mandate 7 RED scaffolds: the ATs spawn the bin + HTTP, so they COMPILE now with
//! `todo!()` bodies + the new `seed_*_survey_trail` / `seed_two_author_same_edge` /
//! `seed_injection_uri_subject` + `assert_traversal_*` / `assert_traversal_href_
//! percent_encoded` helpers (which compile — they drive existing seeding seams or
//! `todo!()` themselves). Each scenario body is `todo!()` → panics → classifies RED
//! (MISSING_FUNCTIONALITY), NOT BROKEN. They stay RED until DELIVER's per-scenario
//! RED→GREEN→COMMIT cycles.
//!
//! Covers:
//! - US-GT-002 (project page, GT-1..GT-6): walking skeleton — GET /project?subject
//!   WITH HX-Request → ONLY the `#traversal-results` fragment (the thinnest
//!   end-to-end thread: viewer → local survey read → pure group → HTML fragment) +
//!   no-JS full-page parity + every philosophy attributed, two authors → two rows
//!   (no merge), verbatim confidence + bucket + cid + contributors as /score links +
//!   claim-less project → guided NoClaims + renders network-disabled.
//! - US-GT-003 (philosophy page, GT-7..GT-11): symmetric walking skeleton for
//!   /philosophy?object + full-page parity + projects-that-embody attributed (two
//!   authors one project → two rows) each a /project link + a shared contributor
//!   appears once as a /score link + claim-less philosophy → guided NoClaims.
//! - US-GT-004 (cross-links, GT-12..GT-14): subject/object/contributor cells render
//!   as plain `<a href>` traversal edges (no-JS click = full navigation) + the
//!   ADR-044 §security boundary — a claim-controlled URI carrying HTML/quote/`&`/
//!   space characters is PERCENT-ENCODED into the href and cannot inject/break.
//
// SCAFFOLD: true

mod support;

#[allow(unused_imports)]
use support::*;

// =============================================================================
// US-GT-002 — land on a project and see the philosophies it embodies + who claimed
// them (GT-1..GT-6). GT-1 is the thinnest end-to-end thread (the walking skeleton).
// =============================================================================

/// GT-1 / WALKING SKELETON (US-GT-002; AC-002.6; the riskiest-assumption thread):
/// from the LOCAL store, `GET /project?subject=<uri>` WITH the `HX-Request` header
/// for a seeded project returns ONLY the `#traversal-results` fragment — the grouped,
/// attributed philosophy edges (each naming its author DID + verbatim confidence +
/// bucket + cid), with NO full-page chrome. This is the thinnest complete thread the
/// slice can demo: viewer → LOCAL survey read → pure group → HTML fragment, proving
/// the read-only viewer can host a local-graph-survey + pure-grouping traversal
/// capability while preserving the anti-merging / read-only / local-first /
/// progressive-enhancement invariants.
///
/// Given Maria's read-only viewer reads a LOCAL store holding several attributed
///   claims about github:rust-lang/cargo;
/// When she opens the project survey WITH the htmx header
///   (`GET /project?subject=github:rust-lang/cargo`, HX-Request);
/// Then she receives ONLY the `#traversal-results` fragment (no chrome), grouping the
///   philosophies the project embodies, each edge attributed to its author DID with
///   the confidence rendered verbatim.
///
/// @us-gt-002 @walking_skeleton @driving_port @driving_adapter @real-io @htmx-fragment
/// @i-gt-3 @i-gt-5 @i-gt-6 @kpi-graph-1 @happy
#[test]
fn open_a_project_survey_with_htmx_returns_only_the_traversal_fragment() {
    // GIVEN a REAL local store seeded (production `peer add` + `peer pull` path) with
    // a PROJECT trail for github:rust-lang/cargo — several distinct philosophies on
    // the shared subject, so the pure grouper yields multiple attributed edges. NO
    // network: `/project` reads the LOCAL store.
    //
    // WHEN Maria submits `GET /project?subject=<cargo>` WITH the HX-Request header
    // (get_htmx).
    //
    // THEN the response is ONLY the `#traversal-results` fragment (`is_fragment()`,
    // NOT a full page), grouping the philosophies embodied — each edge attributed to
    // its author DID, carrying a verbatim confidence + the display-only bucket + cid.
    let env = TestEnv::initialized();
    seed_project_survey_trail(&env, TRAVERSAL_PROJECT_CARGO, TRAVERSAL_AUTHOR_RACHEL);
    let viewer = ViewerServer::start(&env);

    let response = viewer.get_htmx(&format!("/project?subject={TRAVERSAL_PROJECT_CARGO}"));

    assert_eq!(
        response.status, 200,
        "GT-1: GET /project for a seeded project must return 200; body was:\n{}",
        response.body
    );
    assert!(
        response.content_type.contains("text/html"),
        "GT-1: the project survey fragment must be served as text/html; content-type \
         was {:?}",
        response.content_type
    );
    // WITH the HX-Request header the viewer returns ONLY the `#traversal-results`
    // fragment — no full-page chrome (I-GT-6).
    assert!(
        response.is_fragment(),
        "GT-1: an HX-Request `/project` response must be ONLY the fragment (no \
         chrome); body was:\n{}",
        response.body
    );
    assert!(
        response.body_contains(TRAVERSAL_RESULTS_ID),
        "GT-1: the fragment must carry the `#traversal-results` swap-target id; body \
         was:\n{}",
        response.body
    );
    // The grouped, attributed philosophy edges: each names its author DID, a verbatim
    // confidence (`0.90` not `0.9`/`90%`), and the display-only bucket.
    assert_traversal_html_groups_attributed_and_verbatim(
        &response.body,
        &[TRAVERSAL_PHILOSOPHY_DEP_PINNING],
        &[TRAVERSAL_AUTHOR_RACHEL],
        &["0.90", "0.74", "0.25"],
        &["triangulated", "well-evidenced", "speculative"],
    );
}

/// GT-2 (US-GT-002; AC-002.6 — no-JS full page + parity): `GET /project?subject=<uri>`
/// WITHOUT `HX-Request` serves a COMPLETE full page (chrome + the SAME
/// `#traversal-results` region) whose results region is STRUCTURALLY IDENTICAL to the
/// htmx fragment — parity by construction (the page EMBEDS the fragment fn; I-GT-6).
/// The no-JS no-regression contract (KPI-HX-G1): the full page is the contract, the
/// htmx swap is a nicety.
///
/// Given Maria opens /project for a seeded project in a plain browser (no JS);
/// When the page renders, and she also requests it with the htmx header;
/// Then the no-JS response is a full page (chrome) and the htmx response is the bare
///   fragment, and the `#traversal-results` region is identical between them.
///
/// @us-gt-002 @driving_port @real-io @no-js @full-page @parity @i-gt-6 @happy
#[test]
fn the_project_survey_full_page_and_fragment_render_the_same_region() {
    // GIVEN a project trail. WHEN `get` (no HX-Request) AND `get_htmx` for the SAME
    // subject. THEN `get` is_full_page(), `get_htmx` is_fragment(), and the
    // `#traversal-results` region is the same in both (the full page embeds the
    // fragment — parity by construction; I-GT-6).
    let env = TestEnv::initialized();
    seed_project_survey_trail(&env, TRAVERSAL_PROJECT_CARGO, TRAVERSAL_AUTHOR_RACHEL);
    let viewer = ViewerServer::start(&env);

    let full = viewer.get(&format!("/project?subject={TRAVERSAL_PROJECT_CARGO}"));
    let fragment = viewer.get_htmx(&format!("/project?subject={TRAVERSAL_PROJECT_CARGO}"));

    assert_eq!(full.status, 200, "GT-2: the no-JS request must return 200");
    assert_eq!(
        fragment.status, 200,
        "GT-2: the htmx request must return 200"
    );
    // The shapes differ only in chrome: the no-JS request is a full page, the
    // HX-Request response is the bare fragment (no chrome) — I-GT-6.
    assert!(
        full.is_full_page(),
        "GT-2: the no-JS response must be a complete full page (chrome present); body \
         was:\n{}",
        full.body
    );
    assert!(
        fragment.is_fragment(),
        "GT-2: the HX-Request response must be a bare fragment (no chrome); body \
         was:\n{}",
        fragment.body
    );
    // The fragment IS the `#traversal-results` region; the full page EMBEDS the SAME
    // fragment fn, so the fragment body appears verbatim inside the full page —
    // parity by construction (I-GT-6).
    assert!(
        fragment.body.contains(TRAVERSAL_RESULTS_ID),
        "GT-2: the fragment must carry the `#traversal-results` region; body was:\n{}",
        fragment.body
    );
    assert!(
        full.body.contains(fragment.body.trim()),
        "GT-2: the full page's `#traversal-results` region must be identical to the \
         fragment (parity by construction; the page embeds the fragment fn). \
         fragment:\n{}\nfull page:\n{}",
        fragment.body,
        full.body
    );
}

/// GT-3 / CARDINAL anti-merging (US-GT-002 Example 1 / AC-002.1/AC-002.2 — every
/// survey edge attributed, two authors → two rows, no merge): a project survey groups
/// the attributed claims under the philosophies embodied; a philosophy claimed by TWO
/// DISTINCT authors renders as TWO attributed rows under their OWN author DIDs — never
/// averaged into one consensus row — each row carrying its verbatim confidence + the
/// display-only bucket + its cid (WD-GT-5 / I-GT-3 / I-GT-4 / I-GT-5). The cardinal
/// anti-merging scenario for the project surface.
///
/// Given Maria's local store has her own claim (0.92) and Tobias's pulled peer claim
///   (0.70) that github:rust-lang/cargo embodies dependency-pinning;
/// When she opens `GET /project?subject=github:rust-lang/cargo`;
/// Then dependency-pinning shows TWO attributed rows, one per author DID, never
///   averaged, each with its verbatim confidence + bucket + cid.
///
/// @us-gt-002 @driving_port @real-io @anti-merging @i-gt-3 @i-gt-4 @i-gt-5
/// @kpi-graph-2 @boundary
#[test]
fn a_project_survey_renders_two_authors_on_one_philosophy_as_two_rows() {
    // GIVEN a store seeded so two distinct authors assert the SAME (cargo,
    // dependency-pinning) at different confidences (seed_two_author_same_edge). WHEN
    // `get` for that project. THEN BOTH authors' edges render as SEPARATE rows under
    // their own DIDs, each with its verbatim confidence + bucket + cid, and NO
    // merged/averaged consensus row.
    let env = TestEnv::initialized();
    let (own_did, peer_did) = seed_two_author_same_edge(
        &env,
        TRAVERSAL_PROJECT_CARGO,
        TRAVERSAL_PHILOSOPHY_DEP_PINNING,
    );
    // Read the production-recomputed cids the survey read returns (the running viewer
    // holds the DuckDB lock); the rendered edges must NAME these exact cids (I-GT-4).
    let seeded_cids = read_peer_claim_cids_for(&env, &peer_did);
    let viewer = ViewerServer::start(&env);

    let response = viewer.get(&format!("/project?subject={TRAVERSAL_PROJECT_CARGO}"));

    assert_eq!(
        response.status, 200,
        "GT-3: GET /project for the two-authors project must return 200; body \
         was:\n{}",
        response.body
    );
    assert!(
        response.body_contains(TRAVERSAL_RESULTS_ID),
        "GT-3: the response must carry the `#traversal-results` region; body was:\n{}",
        response.body
    );
    // dependency-pinning shows TWO attributed rows — one per author DID (0.92 own +
    // 0.70 peer), each with its display-only bucket — NEVER averaged into one
    // `0.81`/consensus row (anti-merging; I-GT-3).
    assert_traversal_html_groups_attributed_and_verbatim(
        &response.body,
        &[TRAVERSAL_PHILOSOPHY_DEP_PINNING],
        &[&own_did, &peer_did],
        &["0.92", "0.70"],
        &["triangulated", "well-evidenced"],
    );
    // Every rendered edge NAMES its contributing claim's cid (no invented edges;
    // I-GT-4) — the cids are exactly the seeded rows the survey read returned.
    assert_traversal_html_names_cids(&response.body, &seeded_cids);
}

/// GT-4 (US-GT-002 Example 1 / AC-002.3 — contributors as traversal links to /score):
/// a project survey lists each distinct contributor who claimed anything about the
/// project as a link to `/score?contributor=<bare-did>` (the slice-09 terminus
/// REUSED; bare-DID form, ADR-044 Q1) — never merging the authors into a single
/// aggregate. The contributor traversal edge.
///
/// Given github:rust-lang/cargo has claims by the local user and Tobias;
/// When Maria opens `GET /project?subject=github:rust-lang/cargo`;
/// Then both DIDs are listed under "Contributors who claimed", each a link to
///   `/score?contributor=<did>`, with no contributor row merging the two.
///
/// @us-gt-002 @driving_port @real-io @crosslink @i-gt-3 @kpi-graph-1 @happy
#[test]
fn a_project_survey_lists_contributors_as_links_to_their_score() {
    // GIVEN a two-authors project store. WHEN `get` for the project. THEN both author
    // DIDs render under "Contributors who claimed", each as an `<a href>` link to
    // `/score?contributor=<bare-did>`, never merged into one aggregate row.
    let env = TestEnv::initialized();
    let (own_did, peer_did) = seed_two_author_same_edge(
        &env,
        TRAVERSAL_PROJECT_CARGO,
        TRAVERSAL_PHILOSOPHY_DEP_PINNING,
    );
    let viewer = ViewerServer::start(&env);

    let response = viewer.get(&format!("/project?subject={TRAVERSAL_PROJECT_CARGO}"));

    assert_eq!(
        response.status, 200,
        "GT-4: GET /project must return 200; body was:\n{}",
        response.body
    );
    // Both distinct contributors are listed as traversal links to their /score
    // (bare-DID form; the slice-09 terminus reused — built, not rebuilt).
    assert_traversal_html_contributors_link_to_score(&response.body, &[&own_did, &peer_did]);
}

/// GT-5 (US-GT-002 Example 3 / AC-002.4 — a claim-less project renders the guided
/// no-claims state, not a crash): a subject with zero claims in the local store
/// renders the fixed plain-language guided notice — naming the queried subject,
/// hinting a CLI next step — with NO fabricated edge, NO crash, exit 200 (I-GT-4).
/// Sparse honestly renders as sparse; emptiness is recognized as emptiness.
///
/// Given there are zero claims about github:nonexistent/repo in the local store;
/// When Maria requests `GET /project?subject=github:nonexistent/repo`;
/// Then the response is 200, names the queried subject, states there are no claims in
///   the local graph, hints a CLI step, and fabricates no edge.
///
/// @us-gt-002 @driving_port @real-io @no-claims @empty-state @i-gt-4 @error
#[test]
fn a_claim_less_project_renders_the_guided_no_claims_state_not_a_crash() {
    // GIVEN an initialized store with NO rows for TRAVERSAL_PROJECT_UNKNOWN. WHEN
    // `get` AND `get_htmx` for that subject. THEN both shapes render the guided
    // NoClaims notice naming the queried subject, hinting a CLI step, fabricating no
    // edge, leaking no stack trace — a calm 200 in both shapes (I-GT-4).
    let env = TestEnv::initialized();
    let viewer = ViewerServer::start(&env);

    let full = viewer.get(&format!("/project?subject={TRAVERSAL_PROJECT_UNKNOWN}"));
    let fragment = viewer.get_htmx(&format!("/project?subject={TRAVERSAL_PROJECT_UNKNOWN}"));

    // A calm 200 guided state in BOTH shapes — emptiness (and even a read error)
    // degrades to the guided state, never a 5xx / hang / panic (I-GT-4 / NFR-VIEW-6).
    assert_eq!(
        full.status, 200,
        "GT-5: GET /project (no HX-Request) for a claim-less subject must return a \
         calm 200 guided state, never a 5xx; body was:\n{}",
        full.body
    );
    assert_eq!(
        fragment.status, 200,
        "GT-5: GET /project (HX-Request) for a claim-less subject must return a calm \
         200 guided state; body was:\n{}",
        fragment.body
    );
    assert!(
        full.is_full_page(),
        "GT-5: the no-JS NoClaims response must be a complete full page (chrome \
         present); body was:\n{}",
        full.body
    );
    assert!(
        fragment.is_fragment(),
        "GT-5: the HX-Request NoClaims response must be a bare fragment (no chrome); \
         body was:\n{}",
        fragment.body
    );
    // BOTH shapes render the guided "No claims about this project in your local graph"
    // notice naming the queried subject, hinting a CLI next step, with no fabricated
    // edge and no leaked stack trace (I-GT-4).
    assert_traversal_html_renders_no_claims(&full.body, TRAVERSAL_PROJECT_UNKNOWN);
    assert_traversal_html_renders_no_claims(&fragment.body, TRAVERSAL_PROJECT_UNKNOWN);
}

/// GT-6 (US-GT-002 Example / AC-002.5 — the project page renders fully network-
/// disabled): the `/project` survey renders fully with NO network seam wired — the
/// survey DATA (not just the chrome) is computed over the LOCAL DuckDB store, so the
/// network being down NEVER degrades it (distinct from `/search` and `/scrape`; this
/// route has NO outbound edge to take down — offline-STRONGER, I-GT-2). The plain
/// `ViewerServer::start` is the LOCAL-only posture (no indexer URL, no GitHub base).
///
/// Given the viewer is started over a seeded project store with NO network seam wired;
/// When the project is surveyed;
/// Then the full attributed survey renders (no Unavailable/degraded state, no network
///   call) — the survey is a LOCAL read.
///
/// @us-gt-002 @driving_port @real-io @offline @local-first @i-gt-2 @kpi-5 @happy
#[test]
fn the_project_survey_renders_fully_with_the_network_disabled() {
    // GIVEN `ViewerServer::start(&env)` — the store-only posture with NEITHER the
    // /scrape GitHub seam NOR the /search indexer seam wired (the LOCAL-only viewer).
    // A project trail is seeded into the LOCAL store. WHEN the project is surveyed.
    // THEN the full attributed survey renders (a real Found state) with NO
    // Unavailable/degraded notice and NO network call — proving `/project` is LOCAL +
    // offline by construction (I-GT-2; distinct from the slice-08 Unavailable arm).
    let env = TestEnv::initialized();
    seed_project_survey_trail(&env, TRAVERSAL_PROJECT_CARGO, TRAVERSAL_AUTHOR_RACHEL);
    let viewer = ViewerServer::start(&env);

    let response = viewer.get(&format!("/project?subject={TRAVERSAL_PROJECT_CARGO}"));

    assert_eq!(
        response.status, 200,
        "GT-6: GET /project must render a calm 200 over the LOCAL store with no \
         network wired; body was:\n{}",
        response.body
    );
    // The `#traversal-results` region rendered (a REAL Found state), not a blank /
    // error surface — the survey is a LOCAL read, self-sufficient with no network.
    assert!(
        response.body_contains(TRAVERSAL_RESULTS_ID),
        "GT-6: the offline `/project` response must carry the `#traversal-results` \
         region (a REAL survey render, not a blank/error page); body was:\n{}",
        response.body
    );
    // NO Unavailable / degraded notice — `/project` has no outbound edge to fail, so
    // it NEVER renders the slice-08 `/search` Unavailable arm.
    let lowered = response.body.to_ascii_lowercase();
    for banned in [
        "unavailable",
        "network error",
        "could not reach",
        "try again",
    ] {
        assert!(
            !lowered.contains(banned),
            "GT-6: the offline `/project` render must NOT show a network-degraded \
             notice ({banned:?}) — /project has no outbound edge (I-GT-2); body \
             was:\n{}",
            response.body
        );
    }
    // The FULL attributed survey renders (the LOCAL read is self-sufficient with no
    // network present).
    assert_traversal_html_groups_attributed_and_verbatim(
        &response.body,
        &[TRAVERSAL_PHILOSOPHY_DEP_PINNING],
        &[TRAVERSAL_AUTHOR_RACHEL],
        &["0.90", "0.74", "0.25"],
        &["triangulated", "well-evidenced", "speculative"],
    );
}

// =============================================================================
// US-GT-003 — land on a philosophy and see the projects that embody it + who claimed
// them (GT-7..GT-11). GT-7 is the symmetric philosophy walking skeleton.
// =============================================================================

/// GT-7 / WALKING SKELETON (US-GT-003; AC-003.6 — the symmetric philosophy thread):
/// `GET /philosophy?object=<uri>` WITH the `HX-Request` header for a seeded
/// philosophy returns ONLY the `#traversal-results` fragment — the grouped, attributed
/// projects-that-embody edges (each naming its author DID + verbatim confidence +
/// bucket + cid), with NO chrome. The symmetric sibling of GT-1 over the object
/// dimension.
///
/// Given Maria's local store has Rachel's peer claims that BOTH github:NixOS/nixpkgs
///   and github:bazelbuild/bazel embody reproducible-builds;
/// When she opens the philosophy survey WITH the htmx header
///   (`GET /philosophy?object=reproducible-builds`, HX-Request);
/// Then she receives ONLY the `#traversal-results` fragment grouping the projects that
///   embody it, each edge attributed to its author DID with verbatim confidence.
///
/// @us-gt-003 @walking_skeleton @driving_port @driving_adapter @real-io @htmx-fragment
/// @i-gt-3 @i-gt-5 @i-gt-6 @kpi-graph-1 @happy
#[test]
fn open_a_philosophy_survey_with_htmx_returns_only_the_traversal_fragment() {
    // GIVEN a REAL local store seeded (production federation path) with a PHILOSOPHY
    // trail for reproducible-builds — Rachel embodies it across two distinct projects.
    // NO network: `/philosophy` reads the LOCAL store.
    //
    // WHEN Maria submits `GET /philosophy?object=<reproducible-builds>` WITH the
    // HX-Request header (get_htmx).
    //
    // THEN the response is ONLY the `#traversal-results` fragment (`is_fragment()`),
    // grouping the projects that embody the philosophy — each edge attributed to its
    // author DID, carrying a verbatim confidence + bucket + cid.
    let env = TestEnv::initialized();
    seed_philosophy_survey_trail(
        &env,
        TRAVERSAL_PHILOSOPHY_REPRO_BUILDS,
        TRAVERSAL_AUTHOR_RACHEL,
    );
    let viewer = ViewerServer::start(&env);

    let response = viewer.get_htmx(&format!(
        "/philosophy?object={TRAVERSAL_PHILOSOPHY_REPRO_BUILDS}"
    ));

    assert_eq!(
        response.status, 200,
        "GT-7: GET /philosophy for a seeded philosophy must return 200; body was:\n{}",
        response.body
    );
    assert!(
        response.content_type.contains("text/html"),
        "GT-7: the philosophy survey fragment must be served as text/html; \
         content-type was {:?}",
        response.content_type
    );
    assert!(
        response.is_fragment(),
        "GT-7: an HX-Request `/philosophy` response must be ONLY the fragment (no \
         chrome); body was:\n{}",
        response.body
    );
    assert!(
        response.body_contains(TRAVERSAL_RESULTS_ID),
        "GT-7: the fragment must carry the `#traversal-results` swap-target id; body \
         was:\n{}",
        response.body
    );
    // The grouped, attributed projects-that-embody edges (nixpkgs 0.92, bazel 0.85),
    // each attributed to Rachel with a verbatim confidence + bucket.
    assert_traversal_html_groups_attributed_and_verbatim(
        &response.body,
        &[TRAVERSAL_PROJECT_NIXPKGS, TRAVERSAL_PROJECT_BAZEL],
        &[TRAVERSAL_AUTHOR_RACHEL],
        &["0.92", "0.85"],
        &["triangulated", "well-evidenced"],
    );
}

/// GT-8 (US-GT-003; AC-003.6 — no-JS full page + parity): `GET /philosophy?object`
/// WITHOUT `HX-Request` serves a COMPLETE full page whose `#traversal-results` region
/// is STRUCTURALLY IDENTICAL to the htmx fragment — parity by construction (the page
/// EMBEDS the fragment fn; I-GT-6). The philosophy mirror of GT-2.
///
/// Given Maria opens /philosophy for a seeded philosophy in a plain browser (no JS);
/// When the page renders, and she also requests it with the htmx header;
/// Then the no-JS response is a full page and the htmx response is the bare fragment,
///   and the `#traversal-results` region is identical between them.
///
/// @us-gt-003 @driving_port @real-io @no-js @full-page @parity @i-gt-6 @happy
#[test]
fn the_philosophy_survey_full_page_and_fragment_render_the_same_region() {
    // GIVEN a philosophy trail. WHEN `get` and `get_htmx` for the SAME object. THEN
    // `get` is_full_page(), `get_htmx` is_fragment(), and the `#traversal-results`
    // region is the same in both (parity by construction; I-GT-6).
    let env = TestEnv::initialized();
    seed_philosophy_survey_trail(
        &env,
        TRAVERSAL_PHILOSOPHY_REPRO_BUILDS,
        TRAVERSAL_AUTHOR_RACHEL,
    );
    let viewer = ViewerServer::start(&env);

    let full = viewer.get(&format!(
        "/philosophy?object={TRAVERSAL_PHILOSOPHY_REPRO_BUILDS}"
    ));
    let fragment = viewer.get_htmx(&format!(
        "/philosophy?object={TRAVERSAL_PHILOSOPHY_REPRO_BUILDS}"
    ));

    assert_eq!(full.status, 200, "GT-8: the no-JS request must return 200");
    assert_eq!(
        fragment.status, 200,
        "GT-8: the htmx request must return 200"
    );
    assert!(
        full.is_full_page(),
        "GT-8: the no-JS response must be a complete full page (chrome present); body \
         was:\n{}",
        full.body
    );
    assert!(
        fragment.is_fragment(),
        "GT-8: the HX-Request response must be a bare fragment (no chrome); body \
         was:\n{}",
        fragment.body
    );
    assert!(
        fragment.body.contains(TRAVERSAL_RESULTS_ID),
        "GT-8: the fragment must carry the `#traversal-results` region; body was:\n{}",
        fragment.body
    );
    assert!(
        full.body.contains(fragment.body.trim()),
        "GT-8: the full page's `#traversal-results` region must be identical to the \
         fragment (parity by construction). fragment:\n{}\nfull page:\n{}",
        fragment.body,
        full.body
    );
}

/// GT-9 (US-GT-003 Example 2 / AC-003.1/AC-003.2 — projects-that-embody attributed,
/// two authors one project → two rows, each a /project link): a philosophy survey
/// groups the attributed claims under the projects that embody it; a project claimed
/// for the philosophy by TWO DISTINCT authors renders as TWO attributed rows under
/// their OWN author DIDs (never averaged into one `nixpkgs: 0.81` row), and each
/// project group key is a traversal link to `/project?subject=<uri>` (WD-GT-5 /
/// I-GT-3). The anti-merging + traversal-edge scenario for the philosophy surface.
///
/// Given github:NixOS/nixpkgs is claimed for reproducible-builds by the local user
///   (0.92) and Tobias (0.70);
/// When Maria opens `GET /philosophy?object=reproducible-builds`;
/// Then nixpkgs shows two attributed rows, one per author DID, never averaged, and
///   nixpkgs is a link to `/project?subject=github:NixOS/nixpkgs`.
///
/// @us-gt-003 @driving_port @real-io @anti-merging @crosslink @i-gt-3 @i-gt-4
/// @kpi-graph-2 @boundary
#[test]
fn a_philosophy_survey_renders_two_authors_on_one_project_as_two_rows() {
    // GIVEN a store seeded so two distinct authors assert the SAME (nixpkgs,
    // reproducible-builds) at different confidences (seed_two_author_same_edge). WHEN
    // `get` for that philosophy. THEN nixpkgs shows TWO attributed rows under their
    // own DIDs (0.92 own + 0.70 peer), never averaged, AND nixpkgs is a `/project`
    // link.
    let env = TestEnv::initialized();
    let (own_did, peer_did) = seed_two_author_same_edge(
        &env,
        TRAVERSAL_PROJECT_NIXPKGS,
        TRAVERSAL_PHILOSOPHY_REPRO_BUILDS,
    );
    let viewer = ViewerServer::start(&env);

    let response = viewer.get(&format!(
        "/philosophy?object={TRAVERSAL_PHILOSOPHY_REPRO_BUILDS}"
    ));

    assert_eq!(
        response.status, 200,
        "GT-9: GET /philosophy for the two-authors project must return 200; body \
         was:\n{}",
        response.body
    );
    // nixpkgs shows TWO attributed rows — one per author DID (0.92 own + 0.70 peer) —
    // NEVER averaged into one consensus row (anti-merging; I-GT-3).
    assert_traversal_html_groups_attributed_and_verbatim(
        &response.body,
        &[TRAVERSAL_PROJECT_NIXPKGS],
        &[&own_did, &peer_did],
        &["0.92", "0.70"],
        &["triangulated", "well-evidenced"],
    );
    // The project group key is a traversal link to `/project?subject=<nixpkgs>` — the
    // object→project traversal edge (a no-JS click is a full navigation; ADR-044).
    assert_traversal_html_crosslink_is_plain_anchor(
        &response.body,
        &[&format!(
            "/project?subject={TRAVERSAL_PROJECT_NIXPKGS_ENCODED}"
        )],
    );
}

/// GT-10 (US-GT-003 Example 1 / AC-003.3 — a shared contributor across projects is a
/// single traversal link): a contributor who claims MULTIPLE projects for the
/// philosophy appears ONCE under "Contributors who claimed it" (deduped), as a single
/// link to `/score?contributor=<bare-did>` — the canonical cross-project span (the
/// "aha"). The contributor traversal edge, deduped, on the philosophy surface.
///
/// Given did:plc:rachel-test claims both nixpkgs and bazel for reproducible-builds;
/// When Maria opens `GET /philosophy?object=reproducible-builds`;
/// Then did:plc:rachel-test appears ONCE under "Contributors who claimed it", a link
///   to `/score?contributor=did:plc:rachel-test`.
///
/// @us-gt-003 @driving_port @real-io @crosslink @i-gt-3 @kpi-graph-1 @happy
#[test]
fn a_shared_contributor_across_projects_is_a_single_traversal_link() {
    // GIVEN a philosophy trail where Rachel embodies the SAME philosophy across two
    // distinct projects (seed_philosophy_survey_trail spans nixpkgs + bazel). WHEN
    // `get` for that philosophy. THEN Rachel appears ONCE under "Contributors who
    // claimed it" as a single `/score?contributor=<bare-did>` link (deduped — the
    // spanning contributor appears once; the non-obvious connection).
    let env = TestEnv::initialized();
    seed_philosophy_survey_trail(
        &env,
        TRAVERSAL_PHILOSOPHY_REPRO_BUILDS,
        TRAVERSAL_AUTHOR_RACHEL,
    );
    let viewer = ViewerServer::start(&env);

    let response = viewer.get(&format!(
        "/philosophy?object={TRAVERSAL_PHILOSOPHY_REPRO_BUILDS}"
    ));

    assert_eq!(
        response.status, 200,
        "GT-10: GET /philosophy must return 200; body was:\n{}",
        response.body
    );
    // Rachel (spanning two projects) is listed ONCE as a traversal link to her /score
    // (deduped — the contributor list collapses the span to a single entry).
    assert_traversal_html_contributors_link_to_score(&response.body, &[TRAVERSAL_AUTHOR_RACHEL]);
}

/// GT-11 (US-GT-003 Example 3 / AC-003.4 — a claim-less philosophy renders the guided
/// no-claims state): an object with zero claims in the local store renders the guided
/// notice naming the queried object, hinting a CLI next step, with NO fabricated edge,
/// exit 200 (I-GT-4). The philosophy mirror of GT-5.
///
/// Given there are zero claims for org.openlore.philosophy.actor-model in the local
///   store;
/// When Maria requests `GET /philosophy?object=org.openlore.philosophy.actor-model`;
/// Then the response is 200, names the queried object, states there are no claims in
///   the local graph, and fabricates no edge.
///
/// @us-gt-003 @driving_port @real-io @no-claims @empty-state @i-gt-4 @error
#[test]
fn a_claim_less_philosophy_renders_the_guided_no_claims_state() {
    // GIVEN an initialized store with NO rows for TRAVERSAL_PHILOSOPHY_UNKNOWN. WHEN
    // `get` AND `get_htmx` for that object. THEN both shapes render the guided
    // NoClaims notice naming the queried object, fabricating no edge — a calm 200 in
    // both shapes (I-GT-4).
    let env = TestEnv::initialized();
    let viewer = ViewerServer::start(&env);

    let full = viewer.get(&format!(
        "/philosophy?object={TRAVERSAL_PHILOSOPHY_UNKNOWN}"
    ));
    let fragment = viewer.get_htmx(&format!(
        "/philosophy?object={TRAVERSAL_PHILOSOPHY_UNKNOWN}"
    ));

    assert_eq!(
        full.status, 200,
        "GT-11: GET /philosophy (no HX-Request) for a claim-less object must return a \
         calm 200 guided state, never a 5xx; body was:\n{}",
        full.body
    );
    assert_eq!(
        fragment.status, 200,
        "GT-11: GET /philosophy (HX-Request) for a claim-less object must return a \
         calm 200 guided state; body was:\n{}",
        fragment.body
    );
    assert!(
        full.is_full_page(),
        "GT-11: the no-JS NoClaims response must be a complete full page (chrome \
         present); body was:\n{}",
        full.body
    );
    assert!(
        fragment.is_fragment(),
        "GT-11: the HX-Request NoClaims response must be a bare fragment (no chrome); \
         body was:\n{}",
        fragment.body
    );
    // BOTH shapes render the guided "No claims for this philosophy in your local
    // graph" notice naming the queried object, with no fabricated edge (I-GT-4).
    assert_traversal_html_renders_no_claims(&full.body, TRAVERSAL_PHILOSOPHY_UNKNOWN);
    assert_traversal_html_renders_no_claims(&fragment.body, TRAVERSAL_PHILOSOPHY_UNKNOWN);
}

// =============================================================================
// US-GT-004 — make every entity clickable so traversal is one journey (GT-12..GT-14).
// The connective-tissue + security-boundary scenarios.
// =============================================================================

/// GT-12 (US-GT-004 Example 1 / AC-004.1/AC-004.2/AC-004.3 — cross-links are traversal
/// edges): on a survey, the subject cell renders as a link to `/project?subject=`, the
/// object cell as a link to `/philosophy?object=`, and the contributor cell as a link
/// to `/score?contributor=` (bare DID) — so the operator traverses claim → project →
/// philosophy → contributor by clicking (the J-002b "follow the edge to the next
/// entity"). The canonical multi-edge cross-link scenario.
///
/// Given a project survey for github:rust-lang/cargo with attributed philosophy edges
///   by Rachel;
/// When Maria views the project survey;
/// Then the philosophy group key is a link to `/philosophy?object=<uri>`, and the
///   contributor is a link to `/score?contributor=<bare-did>`.
///
/// @us-gt-004 @driving_port @real-io @crosslink @i-gt-6 @kpi-graph-1 @kpi-graph-5
/// @happy
#[test]
fn survey_cells_render_as_traversal_links_to_the_next_entity() {
    // GIVEN a project survey (cargo embodies dependency-pinning + others, claimed by
    // Rachel). WHEN `get` for the project. THEN the philosophy group key is a
    // `/philosophy?object=` link (the object→philosophy edge) AND Rachel is a
    // `/score?contributor=` link (the contributor→score edge) — every cell a clickable
    // traversal edge.
    let env = TestEnv::initialized();
    seed_project_survey_trail(&env, TRAVERSAL_PROJECT_CARGO, TRAVERSAL_AUTHOR_RACHEL);
    let viewer = ViewerServer::start(&env);

    let response = viewer.get(&format!("/project?subject={TRAVERSAL_PROJECT_CARGO}"));

    assert_eq!(
        response.status, 200,
        "GT-12: GET /project must return 200; body was:\n{}",
        response.body
    );
    // The philosophy group key is a traversal link to `/philosophy?object=<uri>` (the
    // object→philosophy edge; the value percent-encoded per ADR-044).
    assert_traversal_html_crosslink_is_plain_anchor(
        &response.body,
        &[&format!(
            "/philosophy?object={TRAVERSAL_PHILOSOPHY_DEP_PINNING}"
        )],
    );
    // The contributor cell is a traversal link to `/score?contributor=<bare-did>` (the
    // contributor→score edge; the slice-09 terminus reused).
    assert_traversal_html_contributors_link_to_score(&response.body, &[TRAVERSAL_AUTHOR_RACHEL]);
}

/// GT-13 (US-GT-004 / AC-004.5 — cross-links are plain `<a href>`, no-JS click = full
/// navigation): every traversal cross-link on a survey is a plain `<a href>` anchor —
/// a no-JS click is a FULL navigation to the target page (progressive enhancement; an
/// htmx swap is OPTIONAL, never required), carrying NO executable control (WD-GT-8 /
/// I-GT-6). The no-JS-traversability scenario.
///
/// Given a philosophy survey whose project group keys are traversal edges;
/// When Maria views the survey with no JS;
/// Then each cross-link is a plain `<a href>` to the target route (a no-JS click is a
///   full navigation), with no executable control wrapping it.
///
/// @us-gt-004 @driving_port @real-io @crosslink @no-js @i-gt-6 @happy
#[test]
fn traversal_cross_links_are_plain_anchors_navigable_without_js() {
    // GIVEN a philosophy survey (Rachel embodies reproducible-builds across nixpkgs +
    // bazel). WHEN `get` (no JS). THEN each project group key is a plain `<a href>`
    // to `/project?subject=<uri>` (a no-JS click is a full navigation; no `hx-`
    // requirement, no executable control).
    let env = TestEnv::initialized();
    seed_philosophy_survey_trail(
        &env,
        TRAVERSAL_PHILOSOPHY_REPRO_BUILDS,
        TRAVERSAL_AUTHOR_RACHEL,
    );
    let viewer = ViewerServer::start(&env);

    let response = viewer.get(&format!(
        "/philosophy?object={TRAVERSAL_PHILOSOPHY_REPRO_BUILDS}"
    ));

    assert_eq!(
        response.status, 200,
        "GT-13: GET /philosophy must return 200; body was:\n{}",
        response.body
    );
    assert!(
        response.is_full_page(),
        "GT-13: the no-JS response must be a complete full page; body was:\n{}",
        response.body
    );
    // Each project group key is a plain `<a href>` to its `/project` survey (a no-JS
    // click is a full navigation; the value percent-encoded per ADR-044). Both
    // nixpkgs + bazel are traversal edges.
    assert_traversal_html_crosslink_is_plain_anchor(
        &response.body,
        &[
            &format!("/project?subject={TRAVERSAL_PROJECT_NIXPKGS_ENCODED}"),
            &format!("/project?subject={TRAVERSAL_PROJECT_BAZEL_ENCODED}"),
        ],
    );
    // The traversal surface carries NO executable write/sign/follow control — the
    // cross-links are render-only navigation TEXT.
    assert_traversal_html_has_no_write_or_sign_control(&response.body);
}

/// GT-14 / SECURITY (US-GT-004 / ADR-044 §security — a claim-controlled URI cannot
/// inject into or break the href): a hostile subject a PEER authored into a signed
/// claim — carrying `"`, `<`, `>`, `&`, and a space (`github:evil/x"><script>&q=
/// space`) — renders its `/project` cross-link with EVERY reserved/unsafe byte
/// PERCENT-ENCODED, so it cannot break out of the `href` attribute or smuggle a second
/// query param. The injection boundary, asserted on the OBSERVABLE rendered href. This
/// is the load-bearing security scenario: subject/object are attacker-influenced
/// strings, so the href is the defense-in-depth boundary (over maud's auto-escape).
///
/// Given a peer's signed claim whose subject is github:evil/x"><script>&q= space
///   embodies dependency-pinning;
/// When Maria surveys `GET /philosophy?object=dependency-pinning` and that hostile
///   subject renders as a project cross-link;
/// Then the rendered href percent-encodes the hostile subject (the exact encoded form)
///   and does NOT break out of the href attribute or inject markup.
///
/// @us-gt-004 @driving_port @real-io @security @injection-boundary @adr-044 @error
#[test]
fn a_claim_controlled_uri_is_percent_encoded_and_cannot_inject_the_href() {
    // GIVEN a store seeded with a PEER claim whose SUBJECT is the hostile
    // TRAVERSAL_INJECTION_SUBJECT on dependency-pinning (the attacker-influenced
    // input). WHEN `get` for `/philosophy?object=dependency-pinning` (the survey that
    // lists that hostile subject as a /project cross-link). THEN the rendered href
    // PERCENT-ENCODES the hostile subject (TRAVERSAL_INJECTION_SUBJECT_ENCODED) and
    // does NOT let the raw `"><script>` / un-encoded `&`/space break out of the href
    // attribute (the ADR-044 §security injection boundary).
    let env = TestEnv::initialized();
    let object = seed_injection_uri_subject(&env);
    let viewer = ViewerServer::start(&env);

    let response = viewer.get(&format!("/philosophy?object={object}"));

    assert_eq!(
        response.status, 200,
        "GT-14: GET /philosophy for the injection-subject survey must return 200; \
         body was:\n{}",
        response.body
    );
    // The hostile subject's `/project` cross-link href percent-encodes every
    // reserved/unsafe byte (TRAVERSAL_INJECTION_SUBJECT_ENCODED) — it cannot break out
    // of the attribute or smuggle a second query param (ADR-044 §security). The raw
    // hostile characters never appear UNESCAPED inside the href attribute.
    assert_traversal_href_percent_encoded(&response.body);
}
