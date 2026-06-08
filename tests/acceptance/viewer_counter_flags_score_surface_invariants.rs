//! Slice-14 acceptance — counter-presence-FLAG GOLD / guardrail invariants over the
//! SCORING-BEARING `GET /score?contributor=<did>` per-contribution breakdown surface
//! (the cross-cutting I-CF-1/5/8/9 + the slice-14 CARDINAL sum-to-weight/byte-identity
//! guardrails that must hold over the WHOLE flagged score surface, beyond any single
//! story).
//!
//! These are the load-bearing, release-relevant guardrail gold tests for the slice-14
//! flag DELTA — the BEHAVIORAL layer of the three-layer enforcement (type + xtask
//! `check-arch` are the other two, owned by DELIVER; the xtask delta is NONE per
//! component-boundaries.md, so the behavioral gold is the active slice-14 guardrail
//! layer). They drive the REAL `openlore ui` verb via the `ViewerServer` subprocess +
//! in-test HTTP (with/without `HX-Request`) over a REAL seeded LOCAL DuckDB, with NO
//! mocked boundary (the batch presence read is a LOCAL DB-index lookup + a PURE
//! projection/render — OFFLINE by construction: `/score` has NO outbound edge). They
//! assert the hard slice-14 invariants on the OBSERVABLE rendered surface across both
//! shapes (full page + htmx fragment):
//!
//! - `every_flagged_score_render_leaves_the_store_read_only` (SF-INV-ReadOnly, I-CF-1 /
//!   KPI-VIEW-2): exercising `/score` across postures (countered + un-countered) AND both
//!   shapes leaves `claims` + `peer_claims` row counts UNCHANGED, via the universe-bound
//!   `assert_store_read_only` (Mandate 8; universe = the two port-exposed counts, all
//!   `unchanged`). The REUSED presence read is a read-only SELECT and persists nothing;
//!   the score is computed per query, nothing persisted.
//! - `no_flagged_score_render_adds_a_write_or_sign_control` (SF-INV-NoWrite, I-CF-1): no
//!   `/score` response shape (full page or fragment) renders a write / sign / counter /
//!   publish / subscribe control — authoring stays the slice-03 CLI; the "Countered"
//!   markers are render-only `<a href="/claims/{cid}">` navigation TEXT.
//! - `the_flagged_score_chrome_stays_offline_no_cdn` (SF-INV-OfflineChrome, I-CF-5 /
//!   KPI-HX-G2): the flagged `/score` full page references ONLY the LOCAL
//!   `/static/htmx.min.js` script src and NO off-host CDN.
//! - `the_flagged_score_surface_renders_fully_offline` (SF-INV-Offline, I-CF-5 / KPI-5 /
//!   AC-SCORE-LOCAL): the flag renders fully with the network unavailable — the presence
//!   read is a LOCAL DB-index lookup with NO outbound edge; the score is a LOCAL feed read
//!   + PURE compute. The countered contribution (countered by a PULLED PEER, verified at
//!   `peer pull` time) STILL carries its marker; the viewer re-verifies nothing.
//! - `the_score_render_is_byte_identical_with_and_without_the_flag` (SF-INV-ByteId, the
//!   CARDINAL shown-never-applied / sum-to-weight GOLD — I-CF-9 / D-14-2 / AC-SCORE-BYTEID
//!   + AC-SCORE-SUMWEIGHT): the SAME store's rendered breakdown — every weight, confidence,
//!   author bonus, triangulation bonus, subtotal, headline total, bucket, `[SPARSE]` line,
//!   pairing ranking, and contribution row order — is byte-IDENTICAL whether or not the
//!   flag is active (markers + the anti-misread legend elided); AND the per-contribution
//!   subtotals STILL sum to the displayed pairing weight on the FLAGGED render. A
//!   regression silently lets the flag perturb a number or re-rank a pairing; this gold
//!   makes it unshippable. THE slice-14 CARDINAL.
//! - `a_large_multi_pairing_breakdown_resolves_presence_in_one_request` (SF-INV-N1,
//!   I-CF-8 / ADR-051): the N+1-flatten behavioral proxy — a breakdown of MANY
//!   contributions across MANY pairings flags the countered subset correctly in ONE
//!   request (the contribution-CID flatten across pairings is a single presence call; the
//!   strict 1-query bound is a DELIVER `adapter-duckdb` unit/property test).
//!
//! Driving discipline (Mandate 1): every assertion enters through the REAL `openlore ui`
//! subprocess + HTTP — never internal `viewer-domain` `render_score_*` fns or
//! `counter_presence_for` directly. The local DuckDB is REAL (seeded via the production
//! `peer add` + `peer pull` federation paths + a DISTINCT peer's verifiable counter);
//! there is NO mocked boundary (`/score` is a LOCAL read + PURE compute).
//!
//! Layer placement (Mandate 9/11): layer-3/layer-5 subprocess + real-I/O, EXAMPLE-only.
//! These guardrails are example-based, never PBT-generated at this layer (the `@property`
//! tag marks them as universal invariants for the reader + the DELIVER crafter; the
//! generative exploration of the pure projection/render is a layer-1/2 concern in the
//! DELIVER `viewer-domain` units, out of this file's scope). The strict single-query N+1
//! bound is likewise a DELIVER `adapter-duckdb` unit/property assertion — at this layer
//! the N+1 guard is the SF-INV-N1 behavioral proxy.
//!
//! Build-before-run note: as with the story file, the run MUST `cargo build` the
//! `openlore` (viewer) bin before running these ATs. No second binary is needed — the
//! presence read is a LOCAL read.
//!
//! Mandate 7 RED scaffolds: each body reaches a `todo!()` (via the `todo!()`-stubbed
//! seed / assert helpers or directly) -> panics -> classifies RED (MISSING_FUNCTIONALITY),
//! NOT BROKEN. They stay RED until DELIVER.
//!
//! Covers: the cross-cutting I-CF-1 / I-CF-5 / I-CF-8 / I-CF-9 guardrails + the slice-14
//! CARDINAL sum-to-weight/byte-identity gold over the whole flagged `/score` surface (the
//! gold companions to the US-CF-001/002 story scenarios in
//! `viewer_counter_flags_score_surface.rs`).
//
// SCAFFOLD: true

mod support;

#[allow(unused_imports)]
use support::*;

// =============================================================================
// I-CF-1 / KPI-VIEW-2 — read-only preserved: the flagged /score surface + every posture
// + shape leave the store unchanged (SF-INV-ReadOnly). The REUSED presence read + the
// per-query score compute persist nothing.
// =============================================================================

/// SF-INV-ReadOnly / GOLD `every_flagged_score_render_leaves_the_store_read_only`
/// (I-CF-1 / KPI-VIEW-2): exercising the flagged `/score` surface — in BOTH shapes (full
/// page + htmx fragment) — leaves the `claims` + `peer_claims` row counts UNCHANGED. The
/// slice-14 companion to the slice-09/12/13 read-only gold tests, asserted via the
/// universe-bound state-delta (Mandate 8: universe = the two port-exposed counts, each
/// `unchanged`). The REUSED batch presence read is a read-only SELECT; the score is
/// computed per query, nothing persisted.
///
/// Given a store seeded with a scored contributor whose breakdown has a countered
///   contribution;
/// When the `/score` surface (both shapes) is exercised;
/// Then the `claims` and `peer_claims` row counts are UNCHANGED.
///
/// @us-cf-002 @property @driving_port @real-io @read-only @i-cf-1 @gold
#[test]
fn every_flagged_score_render_leaves_the_store_read_only() {
    // GIVEN a REAL store seeded so the universe is NON-TRIVIAL: a scored contributor's
    // multi-row breakdown with a countered contribution (via the production peer add +
    // peer pull paths) so the read-only delta is over a POPULATED store (a `0 == 0` delta
    // would not prove the viewer leaves a populated store untouched). Capture the read-only
    // universe (port-exposed counts) BEFORE any route.
    let env = TestEnv::initialized();
    let seeded = seed_score_breakdown_one_contribution_countered(&env);

    let before = capture_store_row_count_universe(&env);

    // WHEN the flagged surface is exercised in BOTH shapes — inside a scope so the viewer's
    // exclusive DuckDB lock is RELEASED before the `after` snapshot.
    {
        let server = ViewerServer::start(&env);
        let route = format!("/score?contributor={}", seeded.contributor_did);
        let full = server.get(&route);
        let fragment = server.get_htmx(&route);
        for (label, response) in [("full page", &full), ("fragment", &fragment)] {
            assert_eq!(
                response.status, 200,
                "SF-INV-ReadOnly: GET {route} ({label}) must be 200; body was:\n{}",
                response.body
            );
        }
    }

    // THEN the persisted-store row counts are UNCHANGED — any change is an UNSHIPPABLE
    // write-surface breach (I-CF-1 / KPI-VIEW-2). The presence read is a read-only SELECT
    // and the score is computed per query; nothing is persisted.
    let after = capture_store_row_count_universe(&env);
    assert_store_read_only(&before, &after);
}

// =============================================================================
// I-CF-1 — no write/sign/counter control on ANY flagged /score response shape
// (SF-INV-NoWrite). The human gate stays the CLI; markers are render-only `<a href>`.
// =============================================================================

/// SF-INV-NoWrite / GOLD `no_flagged_score_render_adds_a_write_or_sign_control` (I-CF-1):
/// NO `/score` response shape (full page or fragment, countered or not) renders a write /
/// sign / counter / publish / subscribe control — authoring stays EXCLUSIVELY in the
/// slice-03 CLI, and every "Countered" marker is render-only navigation TEXT (an
/// `<a href="/claims/{cid}">` anchor), never a control. Asserted on the observable
/// rendered surface across both shapes, reusing the slice-09 score no-write blocklist.
///
/// Given the viewer renders the flagged `/score` surface;
/// When every response shape (full page + fragment) is inspected;
/// Then none renders a write / sign / counter / publish / subscribe control, and every
///   `/claims/{cid}` reference is render-only `<a href>` navigation TEXT.
///
/// @us-cf-002 @property @driving_port @real-io @read-only @i-cf-1 @gold
#[test]
fn no_flagged_score_render_adds_a_write_or_sign_control() {
    // GIVEN a store seeded with a scored contributor whose breakdown has a countered
    // contribution (so the flagged surface has at least one marker present).
    let env = TestEnv::initialized();
    let seeded = seed_score_breakdown_one_contribution_countered(&env);

    // WHEN every flagged-surface response shape is collected in a scope (so the viewer's
    // DuckDB lock releases on drop).
    let bodies: Vec<(String, String)> = {
        let server = ViewerServer::start(&env);
        let route = format!("/score?contributor={}", seeded.contributor_did);
        let shapes = [
            ("/score full page".to_string(), server.get(&route)),
            ("/score fragment".to_string(), server.get_htmx(&route)),
        ];
        shapes
            .into_iter()
            .map(|(label, response)| {
                assert_eq!(
                    response.status, 200,
                    "SF-INV-NoWrite: GET {label} must be 200; body was:\n{}",
                    response.body
                );
                (label, response.body)
            })
            .collect()
    };

    // THEN no shape carries a write / sign / counter / publish / subscribe / follow
    // affordance (the viewer holds no key; authoring stays the slice-03 CLI), AND every
    // `/claims/{cid}` reference is a render-only `<a href>` anchor (navigation TEXT, never
    // an executable control; I-CF-1). The slice-09 score no-write blocklist
    // (`assert_score_html_has_no_write_or_sign_control`) covers the sign/follow/subscribe
    // affordances; the per-reference anchor scan covers the flag links.
    for (label, body) in &bodies {
        assert_score_html_has_no_write_or_sign_control(body);

        for (idx, _) in body.match_indices("/claims/") {
            let prefix = &body[..idx];
            let anchor_open = prefix.rfind("<a href");
            let tag_open = prefix.rfind('<');
            assert!(
                anchor_open.is_some() && anchor_open == tag_open,
                "I-CF-1 ({label}): every `/claims/` reference must be a render-only \
                 `<a href>` navigation anchor (never a write/control); the reference at \
                 byte {idx} is not inside an `<a href` tag; body was:\n{body}"
            );
        }
    }
}

// =============================================================================
// I-CF-5 / KPI-HX-G2 — offline chrome: the flagged /score surface references only the
// local vendored htmx asset, no CDN (SF-INV-OfflineChrome).
// =============================================================================

/// SF-INV-OfflineChrome / GOLD `the_flagged_score_chrome_stays_offline_no_cdn` (I-CF-5 /
/// KPI-HX-G2): the flagged `/score` full page references ONLY the LOCAL
/// `/static/htmx.min.js` script src and NO off-host CDN — the page CHROME stays
/// offline-capable (and so does the FLAG itself, since the presence read is LOCAL).
///
/// Given the viewer renders the flagged `/score` full page;
/// When the page's script references are inspected;
/// Then the only htmx asset reference is the local /static/htmx.min.js — no CDN.
///
/// @us-cf-002 @property @driving_port @real-io @offline @no-cdn @i-cf-5 @gold
///
/// DELIVERED slice-14 step 03-02 (SF-INV-OfflineChrome): GREEN with NO production delta —
/// offline chrome is STRUCTURAL. The flagged `/score` full page already references ONLY
/// the local `/static/htmx.min.js` (shipped with the chrome); `references_external_cdn()`
/// is false by construction. This GOLD guardrail pins that the flag DELTA never reaches
/// for a CDN.
#[test]
fn the_flagged_score_chrome_stays_offline_no_cdn() {
    let env = TestEnv::initialized();
    let seeded = seed_score_breakdown_one_contribution_countered(&env);

    // WHEN the flagged full page is rendered — under the plain, store-only
    // `ViewerServer::start` (NO /scrape GitHub seam, NO /search indexer seam): the presence
    // read is a LOCAL DB-index lookup + the score is a LOCAL compute, so there is no
    // outbound edge to wire.
    let server = ViewerServer::start(&env);
    let full = server.get(&format!("/score?contributor={}", seeded.contributor_did));

    assert_eq!(
        full.status, 200,
        "SF-INV-OfflineChrome: GET /score (full page) must be 200; body was:\n{}",
        full.body
    );
    assert!(
        full.is_full_page(),
        "SF-INV-OfflineChrome: the no-HX /score response must be a full page; body was:\n{}",
        full.body
    );
    // THEN the full page references NO off-host CDN — the only htmx asset is the LOCAL
    // /static/htmx.min.js, so the page CHROME (and, since the flag read is LOCAL, the FLAG
    // itself) stays offline-capable (I-CF-5 / KPI-HX-G2).
    assert!(
        !full.references_external_cdn(),
        "SF-INV-OfflineChrome: the flagged /score full page must reference NO off-host CDN \
         (only the local /static/htmx.min.js); body was:\n{}",
        full.body
    );
}

// =============================================================================
// I-CF-5 / KPI-5 / AC-SCORE-LOCAL — local-first / offline: the flag renders with the
// network unavailable (SF-INV-Offline). The presence read is a LOCAL DB-index lookup +
// the score is a LOCAL compute, with NO outbound edge.
// =============================================================================

/// SF-INV-Offline / GOLD `the_flagged_score_surface_renders_fully_offline` (I-CF-5 /
/// KPI-5 / AC-SCORE-LOCAL): the flagged `/score` surface renders fully with NO network
/// available — the presence read (the INDEXED `referenced_cid IN (...)` ref lookup) is
/// LOCAL, the score is a LOCAL feed read + PURE compute, with NO outbound edge, so the
/// network being down NEVER degrades it. The countered contribution (countered by a
/// PULLED PEER, already verified at `peer pull` time) STILL carries its marker; the viewer
/// re-verifies nothing.
///
/// Given the viewer is started over a seeded store with NO network seam wired;
/// When the flagged `/score` surface is opened;
/// Then the countered contribution STILL carries the "Countered" marker, with no degraded
///   state and no network call.
///
/// @us-cf-002 @property @driving_port @real-io @offline @local-first @i-cf-5 @kpi-5 @gold
///
/// DELIVERED slice-14 step 03-02 (SF-INV-Offline): GREEN with NO production delta —
/// offline render is STRUCTURAL. Under the plain store-only `ViewerServer::start` (NO
/// /scrape GitHub seam, NO /search indexer seam), the presence read is a LOCAL DB-index
/// lookup + the score is a LOCAL feed read + PURE compute, so there is NO outbound edge to
/// degrade. The peer-countered contribution STILL carries its marker (verified at pull
/// time; the viewer re-verifies nothing) and the surface shows NO degraded notice.
#[test]
fn the_flagged_score_surface_renders_fully_offline() {
    let env = TestEnv::initialized();
    let seeded = seed_score_breakdown_one_contribution_countered(&env);
    let countered = seeded
        .countered_cids
        .first()
        .expect("SF-INV-Offline: the /score seed must produce one countered contribution")
        .clone();

    // WHEN the flagged surface is opened under the plain, store-only `ViewerServer::start`
    // — NEITHER the /scrape GitHub seam NOR the /search indexer seam is wired, so the
    // LOCAL-only viewer has NO outbound edge: the presence read is a LOCAL DB-index lookup +
    // the score is a LOCAL compute, OFFLINE by construction. The scope releases the DuckDB
    // lock on drop.
    let body = {
        let server = ViewerServer::start(&env);
        let response = server.get(&format!("/score?contributor={}", seeded.contributor_did));
        assert_eq!(
            response.status, 200,
            "SF-INV-Offline: GET /score must be 200; body was:\n{}",
            response.body
        );
        response.body
    };

    // THEN the countered contribution STILL carries its render-only "Countered" marker —
    // the peer counter was verified at pull time; the viewer re-verifies nothing, makes no
    // network call, and never degrades (I-CF-5 / KPI-5 / AC-SCORE-LOCAL).
    assert_score_row_flagged_countered(&body, &countered);
    let lower = body.to_lowercase();
    for notice in ["unavailable", "network error", "could not reach", "try again"] {
        assert!(
            !lower.contains(notice),
            "SF-INV-Offline: the offline-rendered /score ({notice:?}) must show NO degraded \
             notice — the presence read is LOCAL + the score is a LOCAL compute, no outbound \
             edge to take down; body was:\n{body}"
        );
    }
}

// =============================================================================
// I-CF-9 / D-14-2 / AC-SCORE-BYTEID + AC-SCORE-SUMWEIGHT — the CARDINAL shown-never-
// applied / sum-to-weight / byte-identity gold: the score render is byte-identical with
// and without the flag, and the subtotals still sum to the weight on a FLAGGED render
// (SF-INV-ByteId). THE slice-14 CARDINAL.
// =============================================================================

/// SF-INV-ByteId / CARDINAL SHOWN-NEVER-APPLIED GOLD
/// `the_score_render_is_byte_identical_with_and_without_the_flag` (I-CF-9 / D-14-2 /
/// AC-SCORE-BYTEID + AC-SCORE-SUMWEIGHT): the SAME store's rendered breakdown — every
/// weight, confidence, author bonus, triangulation bonus, subtotal, headline total,
/// bucket, `[SPARSE]` line, pairing ranking, and contribution row order — is byte-IDENTICAL
/// whether or not the flag is active (the additive "Countered" markers AND the anti-misread
/// legend elided), AND the per-contribution subtotals STILL sum to the displayed pairing
/// weight on the FLAGGED render. This is the load-bearing slice-14 invariant: the flag is
/// additive context BESIDE each contribution and changes NOTHING about any number, the
/// ranking, or the row order — the counter is SHOWN, never APPLIED. A byte-identity / sum
/// breach silently lets the flag perturb a subtotal or re-rank a pairing; this gold makes it
/// unshippable. Asserted on the OBSERVABLE rendered HTML (markers + legend elided), over a
/// breakdown with a MIXED countered + un-countered subset so the elision is non-trivial.
///
/// Given the SAME store (a scored breakdown with a mix of countered + un-countered
///   contributions across pairings) is rendered with the flag;
/// When `/score` renders;
/// Then with the additive markers + legend elided, every weight / confidence / bonus /
///   subtotal / total / bucket / `[SPARSE]` line / pairing ranking / contribution row order
///   is byte-identical to the slice-09 baseline, AND the per-contribution subtotals still
///   sum to the displayed pairing weight on the FLAGGED render.
///
/// @us-cf-002 @property @driving_port @real-io @shown-never-applied @no-regression
/// @cardinal-sum-to-weight @cardinal @i-cf-9 @gold
#[test]
fn the_score_render_is_byte_identical_with_and_without_the_flag() {
    // GIVEN a scored breakdown with a MIX of countered + un-countered contributions across
    // pairings. The recorded slice-09 ranked render order is the seed's `ordered_cids`, so
    // the order is directly comparable. Mirrors the slice-12/13 baseline+marker-elision
    // tactic (b): there is NO pre-flag binary and NO no-flag HTTP seam (the route ALWAYS
    // reads `counter_presence_for`), so the slice-09 reference is the RECORDED order, and
    // the gold ELIDES the additive `<a href="/claims/{cid}">Countered</a>` anchors AND the
    // anti-misread legend and proves the remaining slice-09 body honours that recorded
    // ranking/order + every number byte-for-byte.
    let env = TestEnv::initialized();
    let seeded = seed_score_breakdown_many_pairings_known_countered_subset(&env);

    // WHEN the flagged breakdown renders over its SAME store.
    let body = {
        let server = ViewerServer::start(&env);
        let response = server.get(&format!("/score?contributor={}", seeded.contributor_did));
        assert_eq!(
            response.status, 200,
            "SF-INV-ByteId: GET /score must be 200; body was:\n{}",
            response.body
        );
        response.body
    };

    // THEN with the additive "Countered" markers AND the anti-misread legend elided, the
    // breakdown is byte-IDENTICAL to the recorded slice-09 baseline — every weight /
    // confidence / bonus / subtotal / total / bucket / `[SPARSE]` line / pairing ranking /
    // contribution row order unchanged. Any divergence is an UNSHIPPABLE shown-never-applied
    // breach (AC-SCORE-BYTEID / I-CF-9).
    assert_score_render_byte_identical_to_slice09(&body, &seeded.ordered_cids);

    // AND the per-contribution subtotals STILL sum to the displayed pairing weight on the
    // FLAGGED render — the counter subtracts nothing (AC-SCORE-SUMWEIGHT, the other half of
    // the CARDINAL).
    assert_score_html_breakdown_sums_to_weight_with_flag(&body, &seeded.countered_cids);
}

// =============================================================================
// I-CF-8 / ADR-051 — N+1-flatten behavioral proxy: a breakdown of MANY contributions
// across MANY pairings resolves presence in ONE request (SF-INV-N1).
// =============================================================================

/// SF-INV-N1 / GOLD `a_large_multi_pairing_breakdown_resolves_presence_in_one_request`
/// (I-CF-8 / ADR-051): a LARGE `/score` breakdown with MANY contributions across MANY
/// pairings and a KNOWN countered subset flags EVERY countered contribution correctly —
/// and only those — in ONE request, with no per-pairing/per-contribution degradation. The
/// at-this-layer behavioral proxy for the single flattened presence call: the
/// contribution-CID flatten collects every `Contribution.cid` across every
/// `WeightedPairing` from `view.ranked` and queries ONCE (ADR-051 / DD-14-2). If the
/// presence read were per-pairing or per-contribution, a large multi-pairing breakdown
/// would either degrade or mis-flag under the fan-out; this proxy pins the whole breakdown
/// is flagged correctly in one shot. The strict 1-query bound is a DELIVER `adapter-duckdb`
/// unit/property test (query count is not observable at this layer).
///
/// Given Maria's contributor breakdown holds MANY contributions across MANY pairings, a
///   known subset countered;
/// When she opens that contributor's Score breakdown (ONE request);
/// Then EVERY countered contribution carries the marker and EVERY un-countered
///   contribution does not, the whole breakdown flagged correctly in a single request with
///   ranking unchanged.
///
/// @us-cf-001 @property @driving_port @real-io @n-plus-1-guard @i-cf-8 @gold
#[test]
fn a_large_multi_pairing_breakdown_resolves_presence_in_one_request() {
    let env = TestEnv::initialized();
    let seeded = seed_score_breakdown_many_pairings_known_countered_subset(&env);
    // Sanity: the proxy is only meaningful over a genuinely large multi-pairing breakdown
    // with a real countered subset AND un-countered contributions. Pin both so the seed
    // cannot silently shrink the breakdown (which would hollow out the N+1 proxy).
    assert!(
        !seeded.countered_cids.is_empty(),
        "SF-INV-N1: the large breakdown must carry a non-empty countered subset; got {:?}",
        seeded.countered_cids
    );
    assert!(
        !seeded.uncountered_cids.is_empty(),
        "SF-INV-N1: the large breakdown must carry un-countered contributions too; got {:?}",
        seeded.uncountered_cids
    );

    let server = ViewerServer::start(&env);

    // WHEN Maria opens the breakdown — ONE GET request renders the whole multi-pairing
    // breakdown.
    let page = server.get(&format!("/score?contributor={}", seeded.contributor_did));

    assert_eq!(
        page.status, 200,
        "SF-INV-N1: GET /score must be 200; body was:\n{}",
        page.body
    );

    // THEN every countered contribution carries the marker and every un-countered
    // contribution does not — the whole multi-pairing breakdown is flagged correctly in one
    // request (the behavioral proxy for the ADR-051 single flattened presence call across
    // all pairings).
    for countered in &seeded.countered_cids {
        assert_score_row_flagged_countered(&page.body, countered);
    }
    for uncountered in &seeded.uncountered_cids {
        assert_score_row_not_flagged(&page.body, uncountered);
    }
    // And the ranking/order + every number is byte-identical to slice-09 even at this size
    // (I-CF-9).
    assert_score_render_byte_identical_to_slice09(&page.body, &seeded.ordered_cids);
}
