//! Slice-06 acceptance — the `openlore ui` viewer GOLD / guardrail invariants
//! (I-VIEW-1/2/4/5/6; DESIGN §7 / ADR-030 §Earned-Trust).
//!
//! These are the load-bearing, release-relevant guardrail gold tests — the
//! BEHAVIORAL layer of the three-layer enforcement (type + xtask `check-arch` are
//! the other two, owned by DELIVER). They drive the REAL `openlore ui` verb via
//! the `ViewerServer` subprocess + in-test HTTP over a REAL DuckDB, and assert the
//! hard slice-06 invariants on the OBSERVABLE surface:
//!
//! - `viewer_is_read_only` (V-INV-1, I-VIEW-1): the persisted-store row counts
//!   (`claims` + `peer_claims`) are UNCHANGED after exercising EVERY route incl.
//!   POST /scrape — asserted via the universe-bound `assert_store_read_only`
//!   (Mandate 8; universe = the two port-exposed counts, all `unchanged`).
//! - `derived_from_only_on_candidates` (V-INV-2, I-VIEW-5 / WD-62): `derived-from`
//!   appears ONLY on `/scrape` candidates, NEVER on any `/claims`,
//!   `/claims/{cid}`, or `/peer-claims` response body.
//! - `store_views_work_offline` (V-INV-3, I-VIEW-6 / KPI-VIEW-5): with the network
//!   unavailable, `/claims`, `/claims/{cid}`, and `/peer-claims` render fully from
//!   the local store.
//! - `web_process_holds_no_signing_key` (V-INV-4, I-VIEW-1/2 / I-SCR-1): the
//!   viewer process is wired with NO signing identity / NO write path; no route
//!   mutates state or signs (asserted behaviorally — the read-only delta + the
//!   no-sign-affordance surface).
//!
//! Driving discipline (Mandate 1): every assertion enters through the REAL
//! `openlore ui` subprocess + HTTP — never internal functions. The local DuckDB is
//! REAL; GitHub (only reachable via `/scrape`) is the reused `FakeGithub` double.
//!
//! Layer placement (Mandate 11): layer-3/layer-5 subprocess + real-I/O,
//! example-only. These guardrails are EXAMPLE-based, never PBT-generated at this
//! layer (the `@property` tag marks them as universal invariants for the reader +
//! the DELIVER crafter; the generative exploration of the pure render/ingest cores
//! is a layer-2 concern, out of this file's scope).
//!
//! Covers: I-VIEW-1/2/4/5/6 + the cross-view half of AC-005.2.
//
// SCAFFOLD: true

mod support;

#[allow(unused_imports)]
use support::*;

// =============================================================================
// V-INV-1 — viewer_is_read_only (I-VIEW-1; the load-bearing read-only gold test)
// =============================================================================

/// V-INV-1 / GOLD `viewer_is_read_only` (I-VIEW-1; release-relevant): the
/// persisted store (`claims` + `peer_claims` row counts) is UNCHANGED after
/// exercising EVERY viewer route — `/`, `/claims`, `/claims/{cid}`,
/// `/peer-claims`, GET /scrape, AND POST /scrape (the live harvest that must
/// persist NOTHING, BR-VIEW-2). The structural read-only guarantee, proven
/// behaviorally via a universe-bound state-delta (Mandate 8): the universe is the
/// two port-exposed counts, each expected `unchanged`.
///
/// Given a store seeded with own + peer claims and a reachable scrape target;
/// When every viewer route (incl. POST /scrape) is exercised;
/// Then the `claims` and `peer_claims` row counts are UNCHANGED.
///
/// @us-view-001 @us-view-005 @property @driving_port @real-io @i-view-1 @gold
/// @kpi-view-2
#[test]
fn viewer_is_read_only() {
    // GIVEN a REAL store seeded (through the production write paths) with own +
    // peer claims, plus a reachable scrape target (the reused FakeGithub) so the
    // POST /scrape live harvest actually runs (and must still persist nothing).
    let env = TestEnv::initialized();
    let own_cid = seed_own_claim_with_evidence(
        &env,
        "rust-lang/rust",
        "is-maintained-by",
        "The Rust Project",
        0.90,
        &["https://github.com/rust-lang/rust/blob/HEAD/COPYRIGHT"],
    );
    seed_peer_claims_via_pull(&env, "did:plc:peer-axum", 3);
    let github = GithubServer::start(FakeGithub::for_public_repo_with_all_signals(
        "rust-lang/cargo",
    ));

    // Capture the read-only universe BEFORE exercising any route (port-exposed
    // names: `claims.row_count`, `peer_claims.row_count`).
    let before = capture_store_row_count_universe(&env);

    // WHEN every route is exercised, INCLUDING the POST /scrape live harvest. The
    // viewer is scoped so it is STOPPED (its exclusive DuckDB lock released) before
    // the `after` snapshot — the persisted store is observed in the same way the
    // operator would inspect it after the viewer exits, and the read-only proof is
    // about what the viewer LEFT BEHIND, not the live lock.
    {
        let viewer = ViewerServer::start_with_github(&env, github);
        let _ = viewer.get("/");
        let _ = viewer.get("/claims");
        let _ = viewer.get(&format!("/claims/{own_cid}"));
        let _ = viewer.get("/peer-claims");
        let _ = viewer.get("/scrape");
        let _ = viewer.post_form("/scrape", &[("target", "rust-lang/cargo")]);
        let _ = viewer.get("/no-such-route"); // the guided 404 path too
    } // viewer dropped here — the `openlore ui` process is killed, releasing the lock

    // THEN the persisted-store row counts are UNCHANGED — every universe slot is
    // `unchanged` (the structural read-only proof; any change is UNSHIPPABLE).
    let after = capture_store_row_count_universe(&env);
    assert_store_read_only(&before, &after);
}

// =============================================================================
// V-INV-2 — derived_from_only_on_candidates (I-VIEW-5 / WD-62; provenance honesty)
// =============================================================================

/// V-INV-2 / GOLD `derived_from_only_on_candidates` (I-VIEW-5 / WD-62 /
/// AC-005.2): `derived-from` appears ONLY on `/scrape` candidate rows and NEVER on
/// any persisted-claim view (`/claims`, `/claims/{cid}`, `/peer-claims`). The
/// type-level guarantee (persisted view-models carry no derived-from slot,
/// data-models.md §3) proven behaviorally: the same word that MUST appear on
/// /scrape must be ABSENT from every persisted view.
///
/// Given own + peer claims persisted and a live scrape target;
/// When the persisted views and the live-scrape view are rendered;
/// Then derived-from appears on the live-scrape candidates and on NO persisted
/// view.
///
/// @us-view-005 @property @driving_port @real-io @i-view-5 @wd-62 @gold
#[test]
fn derived_from_only_on_candidates() {
    // GIVEN own + peer claims persisted, plus a reachable scrape target so the
    // live candidates (which DO carry derived-from) render.
    let env = TestEnv::initialized();
    let own_cid = seed_own_claim_with_evidence(
        &env,
        "tokio-rs/tokio",
        "has-license",
        "MIT",
        0.95,
        &["https://github.com/tokio-rs/tokio/blob/HEAD/LICENSE"],
    );
    seed_peer_claims_via_pull(&env, "did:plc:peer-axum", 2);
    let github = GithubServer::start(FakeGithub::for_public_repo_with_all_signals(
        "tokio-rs/tokio",
    ));
    let viewer = ViewerServer::start_with_github(&env, github);

    // WHEN the live-scrape view AND every persisted view are rendered.
    let scrape = viewer.post_form("/scrape", &[("target", "tokio-rs/tokio")]);
    let claims = viewer.get("/claims");
    let detail = viewer.get(&format!("/claims/{own_cid}"));
    let peers = viewer.get("/peer-claims");

    // THEN derived-from appears on the live candidates (provenance shown where it
    // is honest) ...
    assert!(
        scrape.body_contains("derived-from"),
        "the live-scrape candidates MUST carry derived-from (WD-62); body was:\n{}",
        scrape.body
    );

    // ... and NEVER on any persisted-claim view (it is not stored, so it cannot be
    // shown there — I-VIEW-5). The load-bearing ABSENCE.
    for (route, page) in [
        ("/claims", &claims),
        ("/claims/{cid}", &detail),
        ("/peer-claims", &peers),
    ] {
        assert!(
            !page.body_contains("derived-from"),
            "derived-from must NEVER appear on the persisted-claim view {route} \
             (I-VIEW-5 / WD-62 — it is not stored); body was:\n{}",
            page.body
        );
    }
}

// =============================================================================
// V-INV-3 — store_views_work_offline (I-VIEW-6 / KPI-VIEW-5; local-first)
// =============================================================================

/// V-INV-3 / GOLD `store_views_work_offline` (I-VIEW-6 / slice-01 KPI-5 /
/// KPI-VIEW-5): with the network UNAVAILABLE, the store views — `/claims`,
/// `/claims/{cid}`, and `/peer-claims` — render FULLY from the local DuckDB.
/// Local-first: the operator's own-store inspection never depends on the network.
///
/// Given the network is unavailable and the store holds own + peer claims;
/// When the store views are rendered;
/// Then `/claims`, `/claims/{cid}`, and `/peer-claims` render fully from the local
/// store.
///
/// @us-view-001 @us-view-002 @us-view-003 @property @driving_port @real-io
/// @i-view-6 @offline @gold
#[test]
fn store_views_work_offline() {
    // GIVEN own + peer claims persisted, and the viewer started with NO GitHub
    // seam wired (the store views never touch GitHub; `ViewerServer::start` wires
    // no `/scrape` network reachability — the store views are offline by
    // construction, exactly as the operator's offline machine would be).
    let env = TestEnv::initialized();
    let own_cid = seed_own_claim_with_evidence(
        &env,
        "rust-lang/rust",
        "is-maintained-by",
        "The Rust Project",
        0.90,
        &[],
    );
    seed_peer_claims_via_pull(&env, "did:plc:peer-axum", 2);
    let viewer = ViewerServer::start(&env);

    // WHEN the store views are rendered (no network available).
    let claims = viewer.get("/claims");
    let detail = viewer.get(&format!("/claims/{own_cid}"));
    let peers = viewer.get("/peer-claims");

    // THEN each store view renders fully from the local store (200 + the persisted
    // content), with no network access.
    assert_eq!(claims.status, 200, "/claims must render offline");
    assert!(
        claims.body_contains("rust-lang/rust"),
        "/claims must render the persisted claim offline; body was:\n{}",
        claims.body
    );
    assert_eq!(detail.status, 200, "/claims/{{cid}} must render offline");
    assert!(
        detail.body_contains("The Rust Project"),
        "the detail view must render the persisted claim offline; body was:\n{}",
        detail.body
    );
    assert_eq!(peers.status, 200, "/peer-claims must render offline");
    assert!(
        peers.body_contains("did:plc:peer-axum"),
        "/peer-claims must render the federated claims offline; body was:\n{}",
        peers.body
    );
}

// =============================================================================
// V-INV-4 — web_process_holds_no_signing_key (I-VIEW-1/2 / I-SCR-1; no write path)
// =============================================================================

/// V-INV-4 / GOLD `web_process_holds_no_signing_key` (I-VIEW-1/2 / I-SCR-1):
/// the viewer process is incapable of signing by construction — it is wired with
/// NO signing identity and NO write path. Asserted BEHAVIORALLY on the observable
/// surface: (a) no route renders a sign control on ANY page (the human gate is
/// structural — the persisted/candidate view-models carry no sign affordance);
/// and (b) exercising every route leaves the store row counts unchanged (the
/// no-write companion to V-INV-1, surfaced as the same read-only universe). The
/// type + xtask-`check-arch` capability layers (deps exclude `adapter-atproto-pds`
/// + no `IdentityPort`) are DELIVER's structural concern.
///
/// Given the viewer is running over a populated store;
/// When every route is requested;
/// Then no route renders a sign control and no route writes or signs.
///
/// @us-view-001 @property @driving_port @real-io @i-view-1 @i-view-2 @i-scr-1
/// @gold
#[test]
fn web_process_holds_no_signing_key() {
    // GIVEN a populated store + the viewer running.
    let env = TestEnv::initialized();
    let own_cid = seed_own_claim_with_evidence(
        &env,
        "rust-lang/rust",
        "is-maintained-by",
        "The Rust Project",
        0.90,
        &[],
    );
    let before = capture_store_row_count_universe(&env);

    // WHEN every store route is requested (no GitHub seam — the store routes do not
    // touch the network; the /scrape no-sign-control is asserted in V-S1). The
    // viewer is scoped so it is STOPPED (its exclusive DuckDB lock released) before
    // the `after` snapshot — the persisted store is observed the way the operator
    // would after the viewer exits (the no-write proof is about what the viewer LEFT
    // BEHIND, not the live lock).
    let (landing, claims, detail, peers) = {
        let viewer = ViewerServer::start(&env);
        let landing = viewer.get("/");
        let claims = viewer.get("/claims");
        let detail = viewer.get(&format!("/claims/{own_cid}"));
        let peers = viewer.get("/peer-claims");
        (landing, claims, detail, peers)
    }; // viewer dropped here — the `openlore ui` process is killed, releasing the lock

    // THEN no store route renders a sign control (the human gate is structural —
    // I-SCR-1). Asserting the load-bearing ABSENCE of a sign affordance on every
    // store surface.
    for (route, page) in [
        ("/", &landing),
        ("/claims", &claims),
        ("/claims/{cid}", &detail),
        ("/peer-claims", &peers),
    ] {
        for sign_control_marker in ["name=\"sign\"", "Sign claim", "value=\"sign\""] {
            assert!(
                !page.body_contains(sign_control_marker),
                "no viewer route may render a sign control — {route} rendered \
                 {sign_control_marker:?} (I-VIEW-1/2 / I-SCR-1); body was:\n{}",
                page.body
            );
        }
    }

    // AND no route wrote or signed — the store row counts are unchanged (the
    // no-write companion to the read-only gold test).
    let after = capture_store_row_count_universe(&env);
    assert_store_read_only(&before, &after);
}
