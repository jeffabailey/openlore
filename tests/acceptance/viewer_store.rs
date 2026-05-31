//! Slice-06 acceptance — the `openlore ui` read-only htmx viewer's STORE views
//! (US-VIEW-001/002/003/004).
//!
//! The viewer is a NEW `openlore ui --port <P>` verb (ADR-028): a long-running
//! hyper server bound to 127.0.0.1 ONLY, with NO auth and NO signing key. It
//! serves the operator's OWN node store — `claims` (slice-01) + `peer_claims`
//! (slice-03) — as server-rendered HTML (maud, ADR-029) over a READ-ONLY
//! `StoreReadPort` (ADR-030). Signing stays EXCLUSIVELY in the CLI (I-VIEW-3 /
//! I-SCR-1). Note: the DISCUSS user-stories used the placeholder verb
//! `openlore viewer`; DESIGN + ADR-028 (governing) settled the verb as
//! `openlore ui --port` — the authoritative name used throughout this corpus.
//!
//! Driving discipline (hard requirement, Mandate 1): every scenario enters
//! through the CLI driving port — the REAL `openlore ui` subprocess (via the
//! `ViewerServer` spawn helper) + in-test HTTP GET/POST — and asserts on the
//! returned HTML. NO scenario calls `viewer-domain` render fns directly (those
//! are unit-level, exercised in DELIVER). The local DuckDB is REAL (BR-VIEW-4 —
//! the SAME store the CLI writes, seeded through the production `claim add` /
//! `peer pull` write paths, Pillar 3). GitHub is NOT touched by any store view
//! (offline by construction, I-VIEW-6).
//!
//! Layer placement (nw-tdd-methodology Layered Test Discipline matrix +
//! Mandate 11): every test here is a layer-3/layer-5 subprocess + real-I/O test
//! — example-only. The sad paths (empty store, unreadable store, unknown CID, no
//! peers, missing origin) are enumerated explicitly, never PBT-generated.
//!
//! Build-before-run (carry into DELIVER roadmap, mirrors the indexer ATs):
//! `cargo test` does NOT rebuild a spawned binary — the run MUST `cargo build`
//! the `openlore` bin first so `ViewerServer` spawns the CURRENT `openlore ui`.
//!
//! Covers:
//! - US-VIEW-001: see my store in the browser (WALKING SKELETON, V-1) + empty
//!   store guidance (V-2) + read-only-on-localhost-no-key launch (V-3) +
//!   unreadable-store helpful error (V-4)
//! - US-VIEW-002: inspect one claim's full evidence (V-5) + no-evidence claim
//!   (V-6) + unknown CID guided not-found (V-7)
//! - US-VIEW-003: distinguish federated peer claims from own (V-8) + no-peers
//!   guidance (V-9) + unknown peer origin still renders (V-10)
//! - US-VIEW-004: pagination — page through a large store (V-11) + bounded last
//!   page (V-12) + small store has no controls (V-13)
//
// SCAFFOLD: true

mod support;

#[allow(unused_imports)]
use support::*;

// =============================================================================
// US-VIEW-001 — WALKING SKELETON + the read-only launch + empty/error states
// =============================================================================

/// V-1 (US-VIEW-001 happy; WALKING SKELETON for slice-06): Maria has signed
/// claims through the CLI; she starts `openlore ui`, opens the My Claims page in
/// her browser, and sees one of her signed claims rendered as a row — subject,
/// predicate, object, the stored confidence numeric (0.90 verbatim, FR-VIEW-8),
/// and its CID — having typed NO SQL. This is the thinnest end-to-end thread:
/// HTTP in -> read-only DuckDB query -> HTML out, read-only, offline. It drives
/// the REAL CLI driving adapter (`openlore ui` subprocess) + REAL DuckDB seeded
/// through the production `claim add` write path + in-test HTTP.
///
/// Given Maria has signed a claim ("rust-lang/rust","is-maintained-by","The Rust
/// Project") at confidence 0.90 through the CLI;
/// When she starts `openlore ui` and opens the My Claims page in her browser;
/// Then she sees that claim as a row with subject, predicate, object, confidence
/// 0.90, and its CID — and she wrote no SQL.
///
/// @us-view-001 @walking_skeleton @driving_port @driving_adapter @real-io
/// @kpi-view-1 @happy
#[test]
fn operator_sees_their_signed_claims_in_the_browser_with_zero_sql() {
    // GIVEN Maria has signed the headline claim through the PRODUCTION `claim add`
    // write path (Pillar 3 — the SAME store `openlore ui` reads, BR-VIEW-4).
    let env = TestEnv::initialized();
    let cid = seed_own_claim_with_evidence(
        &env,
        "rust-lang/rust",
        "is-maintained-by",
        "The Rust Project",
        0.90,
        &[],
    );

    // WHEN she starts `openlore ui` and opens the My Claims page (GET /claims).
    let viewer = ViewerServer::start(&env);
    let page = viewer.get("/claims");

    // THEN the page renders (200) and shows the claim as a row — subject,
    // predicate, object, the stored confidence numeric VERBATIM (0.90, FR-VIEW-8),
    // and its CID. Observable rendered text only (the operator's browser view).
    assert_eq!(page.status, 200, "GET /claims must render the My Claims page");
    for needle in [
        "rust-lang/rust",
        "is-maintained-by",
        "The Rust Project",
        "0.90",
        cid.as_str(),
    ] {
        assert!(
            page.body_contains(needle),
            "the My Claims page must render {needle:?} for the seeded claim; \
             body was:\n{}",
            page.body
        );
    }
}

/// V-2 (US-VIEW-001 edge; AC-001.3): a fresh operator (Tom) has signed nothing.
/// He opens the My Claims page and sees GUIDANCE — "signed claims appear here and
/// are created via the CLI" — not a blank page (FR-VIEW-7 / NFR-VIEW-6).
///
/// Given Tom has signed no claims;
/// When he opens the My Claims page;
/// Then he sees guidance that signed claims appear here and are created via the
/// CLI (not a blank page).
///
/// @us-view-001 @driving_port @real-io @empty-state @edge
#[test]
fn empty_store_guides_a_first_run_operator() {
    // GIVEN a fresh, initialized env with ZERO signed claims (the store exists but
    // is empty — the production `init` ran, no `claim add` did).
    let env = TestEnv::initialized();

    // WHEN Tom opens the My Claims page.
    let viewer = ViewerServer::start(&env);
    let page = viewer.get("/claims");

    // THEN he sees guided empty-state text pointing to the CLI — never a blank
    // page. Assert on the OBSERVABLE guidance, framed in domain language.
    assert_eq!(page.status, 200, "an empty store still renders a guided page");
    assert!(
        page.body_contains("not signed any claims yet")
            || page.body_contains("claims you sign with the CLI will appear here"),
        "the empty My Claims page must guide the operator to the CLI (FR-VIEW-7), \
         not show a blank page; body was:\n{}",
        page.body
    );
}

/// V-3 (US-VIEW-001 happy; AC-001.2): when Maria starts `openlore ui` it reports
/// a loopback listen URL and states the view is read-only, and NO signing key is
/// loaded into the process (NFR-VIEW-2/3 / I-VIEW-2/3/4). The launch-time
/// read-only + localhost + no-key contract surfaced to the operator.
///
/// Given Maria has a local store;
/// When she starts `openlore ui`;
/// Then it reports a loopback listen URL and states the view is read-only, and no
/// signing key is loaded into the process.
///
/// @us-view-001 @driving_port @driving_adapter @real-io @i-view-2 @i-view-4 @happy
#[test]
fn viewer_starts_read_only_on_localhost_with_no_signing_key() {
    // GIVEN Maria has a local (initialized) store.
    let env = TestEnv::initialized();

    // WHEN she starts `openlore ui`.
    let viewer = ViewerServer::start(&env);

    // THEN the bound listen URL is a loopback address (127.0.0.1 — I-VIEW-4), and
    // the landing page states the view is read-only (the operator is told, up
    // front, that nothing here can change her store). The no-key guarantee is
    // STRUCTURAL (ADR-030: the process links no IdentityPort) and is proven
    // behaviorally by the `web_process_holds_no_signing_key` gold test
    // (viewer_invariants.rs V-INV-4); here we assert the operator-facing surface.
    assert!(
        viewer.base_url().contains("127.0.0.1"),
        "the viewer must bind a loopback address (I-VIEW-4); got {}",
        viewer.base_url()
    );
    let landing = viewer.get("/");
    assert_eq!(landing.status, 200, "the landing page must render");
    assert!(
        landing.body_contains("read-only"),
        "the viewer must state the view is read-only on its surface (NFR-VIEW-1); \
         body was:\n{}",
        landing.body
    );
}

/// V-4 (US-VIEW-001 error; AC-001.4): another process holds the store file. When
/// Maria starts `openlore ui` it REFUSES to serve with a plain-language message
/// naming the store path and asking if another process is using it — NO raw stack
/// trace (NFR-VIEW-6). The startup store-readability probe (ADR-030 §Earned-Trust
/// step 1) surfaces US-VIEW-001 Example 3 as a clean startup refusal, not a
/// per-request crash.
///
/// Given the store file is locked by another process;
/// When Maria starts the viewer to open the My Claims page;
/// Then she sees a plain-language message naming the path and asking if another
/// process uses it, with no raw stack trace.
///
/// @us-view-001 @driving_port @real-io @infrastructure-failure @error
#[test]
fn unreadable_store_shows_a_helpful_error() {
    // GIVEN the env's store file is locked / unreadable by the viewer (another
    // process holds it). The guard restores access on drop.
    let env = TestEnv::initialized();
    let _lock = make_store_unreadable(&env);

    // WHEN Maria starts `openlore ui` (it walks the WIRE->PROBE->USE gauntlet;
    // ADR-009/030) — the store-readability probe fails before the serve loop.
    let outcome = run_openlore_ui_expecting_startup_refusal(&env);

    // THEN the viewer REFUSES to start with a plain-language message naming the
    // path + asking if another process holds it — NEVER a raw stack trace.
    assert_ne!(
        outcome.status, 0,
        "the viewer must refuse to serve when the store is unreadable (ADR-030); \
         stdout: {} stderr: {}",
        outcome.stdout, outcome.stderr
    );
    assert!(
        outcome.stderr.contains("store") && outcome.stderr.contains("another process"),
        "the refusal must name the store + ask if another process holds it \
         (NFR-VIEW-6); stderr was:\n{}",
        outcome.stderr
    );
    for stack_trace_marker in ["panicked at", "RUST_BACKTRACE", "stack backtrace"] {
        assert!(
            !outcome.stderr.contains(stack_trace_marker),
            "the refusal must be plain-language — no raw stack trace ({stack_trace_marker:?}); \
             stderr was:\n{}",
            outcome.stderr
        );
    }
}

// =============================================================================
// US-VIEW-002 — inspect one claim's full evidence (`/claims/{cid}`)
// =============================================================================

/// V-5 (US-VIEW-002 happy; AC-002.1): Maria opens a claim's detail page and sees
/// all its fields plus the COMPLETE evidence[] array (FR-VIEW-3). The detail view
/// shows the evidence the list view summarizes away.
///
/// Given Maria's claim has two evidence URLs;
/// When she opens its detail page;
/// Then she sees all claim fields and both evidence URLs.
///
/// @us-view-002 @driving_port @real-io @happy
#[test]
fn operator_views_the_full_evidence_behind_one_claim() {
    // GIVEN Maria has signed a claim WITH two evidence URLs through the production
    // `claim add` path; its CID addresses the detail page.
    let env = TestEnv::initialized();
    let cid = seed_own_claim_with_evidence(
        &env,
        "tokio-rs/tokio",
        "has-license",
        "MIT",
        0.95,
        &[
            "https://github.com/tokio-rs/tokio/blob/HEAD/LICENSE",
            "https://github.com/tokio-rs/tokio/blob/HEAD/Cargo.toml",
        ],
    );

    // WHEN she opens its detail page (GET /claims/{cid}).
    let viewer = ViewerServer::start(&env);
    let page = viewer.get(&format!("/claims/{cid}"));

    // THEN she sees all claim fields AND both evidence URLs.
    assert_eq!(page.status, 200, "the detail page must render for a known CID");
    for needle in [
        "tokio-rs/tokio",
        "has-license",
        "MIT",
        "0.95",
        "https://github.com/tokio-rs/tokio/blob/HEAD/LICENSE",
        "https://github.com/tokio-rs/tokio/blob/HEAD/Cargo.toml",
    ] {
        assert!(
            page.body_contains(needle),
            "the detail page must render {needle:?} (all fields + complete \
             evidence[]); body was:\n{}",
            page.body
        );
    }
}

/// V-6 (US-VIEW-002 edge; AC-002.2): a claim signed WITHOUT evidence shows an
/// explicit "no evidence attached" state, not a blank evidence section
/// (FR-VIEW-3 / NFR-VIEW-6).
///
/// Given Maria has a claim signed without evidence;
/// When she opens its detail page;
/// Then she sees "no evidence attached" rather than a blank section.
///
/// @us-view-002 @driving_port @real-io @empty-state @edge
#[test]
fn claim_with_no_evidence_is_shown_clearly() {
    // GIVEN a claim signed WITHOUT evidence (empty evidence[]).
    let env = TestEnv::initialized();
    let cid = seed_own_claim_with_evidence(
        &env,
        "serde-rs/serde",
        "is-maintained-by",
        "dtolnay",
        0.80,
        &[],
    );

    // WHEN she opens its detail page.
    let viewer = ViewerServer::start(&env);
    let page = viewer.get(&format!("/claims/{cid}"));

    // THEN she sees the explicit "no evidence attached" state.
    assert_eq!(page.status, 200, "the detail page must render");
    assert!(
        page.body_contains("no evidence attached"),
        "a claim with empty evidence must show an explicit \"no evidence attached\" \
         state, not a blank section; body was:\n{}",
        page.body
    );
}

/// V-7 (US-VIEW-002 error; AC-002.3): a CID that is not in the store shows a
/// guided not-found message ("No claim with that identifier in your store") with
/// a back link to the list (FR-VIEW-3 / NFR-VIEW-6). The guided 404 for a
/// mistyped CID — never a stack trace.
///
/// Given no claim with the requested CID exists in the store;
/// When Maria opens that detail page;
/// Then she sees "No claim with that identifier in your store" and a link back to
/// the list.
///
/// @us-view-002 @driving_port @real-io @error
#[test]
fn unknown_cid_guides_the_operator_back() {
    // GIVEN a store with at least one real claim, but NOT the mistyped CID.
    let env = TestEnv::initialized();
    let _present = seed_own_claim_with_evidence(
        &env,
        "rust-lang/rust",
        "is-maintained-by",
        "The Rust Project",
        0.90,
        &[],
    );

    // WHEN Maria opens a detail page for a CID that is not in her store.
    let viewer = ViewerServer::start(&env);
    let page = viewer.get("/claims/bafyreigh2akzzz-not-a-real-cid");

    // THEN she sees the guided not-found message + a back link to My Claims. The
    // guided 404 reuses `render_error` (DESIGN §5) — no stack trace.
    assert!(
        page.body_contains("No claim with that identifier in your store"),
        "an unknown CID must show the guided not-found message; body was:\n{}",
        page.body
    );
    assert!(
        page.body_contains("/claims"),
        "the not-found page must link back to the My Claims list; body was:\n{}",
        page.body
    );
}

// =============================================================================
// US-VIEW-003 — distinguish federated peer claims from own (`/peer-claims`)
// =============================================================================

/// V-8 (US-VIEW-003 happy; AC-003.1): Maria opens the Peer Claims view and sees
/// federated claims WITH their peer origin (the peer's `author_did` +
/// `fetched_from_pds` — there is no `peer_origin` column; the origin IS those two
/// fields per data-models.md), on a surface clearly SEPARATE from her own claims
/// (FR-VIEW-4 / BR-VIEW-5). "Mine vs federated" is never ambiguous.
///
/// Given Maria has federated peer claims from a peer;
/// When she opens the Peer Claims view;
/// Then she sees federated claims with their peer origin, separate from her own
/// claims.
///
/// @us-view-003 @driving_port @real-io @happy
#[test]
fn operator_distinguishes_federated_peer_claims_from_their_own() {
    // GIVEN Maria has federated peer claims from a peer through the PRODUCTION
    // `peer pull` federation path (slice-03 — the SAME store, BR-VIEW-4), AND has
    // her OWN signed claim so the "distinct from own" contrast is load-bearing.
    let env = TestEnv::initialized();
    let _own = seed_own_claim_with_evidence(
        &env,
        "rust-lang/rust",
        "is-maintained-by",
        "The Rust Project",
        0.90,
        &[],
    );
    let peer_did = "did:plc:peer-axum";
    seed_peer_claims_via_pull(&env, peer_did, 3);

    // WHEN she opens the Peer Claims view (a SEPARATE route from /claims).
    let viewer = ViewerServer::start(&env);
    let page = viewer.get("/peer-claims");

    // THEN federated claims render WITH their peer origin (the peer's DID), on a
    // surface distinct from her own claims. The peer's DID appears (origin shown);
    // the route itself is the structural separation (BR-VIEW-5).
    assert_eq!(page.status, 200, "the Peer Claims view must render");
    assert!(
        page.body_contains(peer_did),
        "each peer row must show its peer origin (the peer's author_did); \
         body was:\n{}",
        page.body
    );
}

/// V-9 (US-VIEW-003 edge; AC-003.2): a node that has federated NOTHING shows the
/// guided "No federated claims yet" empty state (FR-VIEW-7).
///
/// Given Maria has federated no peer claims;
/// When she opens the Peer Claims view;
/// Then she sees "No federated claims yet" guidance.
///
/// @us-view-003 @driving_port @real-io @empty-state @edge
#[test]
fn no_federated_claims_yet_is_guided() {
    // GIVEN an initialized env with ZERO peer claims.
    let env = TestEnv::initialized();

    // WHEN Maria opens the Peer Claims view.
    let viewer = ViewerServer::start(&env);
    let page = viewer.get("/peer-claims");

    // THEN she sees the guided no-peers message — never a blank page.
    assert_eq!(page.status, 200, "an empty peer set still renders a guided page");
    assert!(
        page.body_contains("No federated claims yet"),
        "the empty Peer Claims view must show the guided no-peers message; \
         body was:\n{}",
        page.body
    );
}

/// V-10 (US-VIEW-003 boundary; AC-003.3): a federated peer claim whose origin is
/// absent/blank (a defensive path — the slice-03 schema CHECK makes `author_did`
/// non-empty, so this is data that predates/bypasses the CHECK) STILL renders,
/// labeled origin "unknown", rather than being DROPPED (FR-VIEW-4 / NFR-VIEW-6).
///
/// Given a federated peer claim has no recorded origin;
/// When Maria opens the Peer Claims view;
/// Then that claim still renders with origin shown as "unknown".
///
/// @us-view-003 @driving_port @real-io @boundary @edge
#[test]
fn peer_claim_with_unknown_origin_still_renders() {
    // GIVEN a peer_claims row with a blank/absent origin (a defensive fixture —
    // bypasses the slice-03 CHECK to exercise the "unknown" render path).
    let env = TestEnv::initialized();
    seed_peer_claim_with_blank_origin(&env);

    // WHEN Maria opens the Peer Claims view.
    let viewer = ViewerServer::start(&env);
    let page = viewer.get("/peer-claims");

    // THEN the claim still renders, labeled origin "unknown" — never dropped.
    assert_eq!(page.status, 200, "the Peer Claims view must render");
    assert!(
        page.body_contains("unknown"),
        "a peer claim with absent origin must render labeled \"unknown\" (not be \
         dropped); body was:\n{}",
        page.body
    );
}

// =============================================================================
// US-VIEW-004 — navigate a large store with pagination
// =============================================================================

/// V-11 (US-VIEW-004 happy; AC-004.1): Maria has 312 signed claims; the My Claims
/// page renders 50 per page with a position indicator. She clicks Next (GET
/// /claims?page=2) and sees claims 51–100 of 312 (FR-VIEW-6; page size 50, sort
/// composed_at DESC per ADR-030 / data-models.md §2).
///
/// Given Maria has 312 signed claims and a page size of 50;
/// When she opens My Claims and goes to the next page;
/// Then she sees claims 51–100 of 312 with a position indicator.
///
/// @us-view-004 @driving_port @real-io @pagination @happy
#[test]
fn operator_pages_through_a_large_store() {
    // GIVEN Maria has 312 signed claims (seeded through the production write path;
    // a real-sized store, US-VIEW-004 scale).
    let env = TestEnv::initialized();
    seed_own_claims_via_cli(&env, 312);

    // WHEN she opens My Claims and goes to page 2.
    let viewer = ViewerServer::start(&env);
    let page_one = viewer.get("/claims");
    let page_two = viewer.get("/claims?page=2");

    // THEN page 1 shows the "1–50 of 312" indicator and page 2 shows "51–100 of
    // 312" — the fixed page size 50 + the position indicator (FR-VIEW-6).
    assert_eq!(page_one.status, 200, "page 1 must render");
    assert!(
        page_one.body_contains("1–50 of 312"),
        "page 1 must show the \"1–50 of 312\" position indicator; body was:\n{}",
        page_one.body
    );
    assert_eq!(page_two.status, 200, "page 2 must render");
    assert!(
        page_two.body_contains("51–100 of 312"),
        "page 2 must show the \"51–100 of 312\" position indicator; body was:\n{}",
        page_two.body
    );
}

/// V-12 (US-VIEW-004 boundary; AC-004.2): on the LAST page Maria sees the bounded
/// indicator "301–312 of 312" and there is no further Next action (FR-VIEW-6 —
/// bounds respected at the last page).
///
/// Given Maria is on the last page of 312 claims;
/// Then she sees 301–312 of 312 and no further Next action.
///
/// @us-view-004 @driving_port @real-io @pagination @boundary
#[test]
fn last_page_is_bounded_correctly() {
    // GIVEN Maria has 312 claims (page size 50 -> 7 pages; last page is page 7,
    // rows 301–312).
    let env = TestEnv::initialized();
    seed_own_claims_via_cli(&env, 312);

    // WHEN she is on the last page (page 7).
    let viewer = ViewerServer::start(&env);
    let last = viewer.get("/claims?page=7");

    // THEN the bounded "301–312 of 312" indicator shows AND there is no Next
    // action (the next-page control is absent/disabled at the bound).
    assert_eq!(last.status, 200, "the last page must render");
    assert!(
        last.body_contains("301–312 of 312"),
        "the last page must show the bounded \"301–312 of 312\" indicator; \
         body was:\n{}",
        last.body
    );
    assert!(
        !last.body_contains("?page=8"),
        "the last page must offer no further Next action (no link to page 8); \
         body was:\n{}",
        last.body
    );
}

/// V-13 (US-VIEW-004 edge; AC-004.3): a small store (12 claims, page size 50)
/// renders all 12 on one page with NO pagination controls (FR-VIEW-6 — stores
/// smaller than one page show no controls).
///
/// Given Maria has 12 claims and a page size of 50;
/// When she opens My Claims;
/// Then all 12 render and no pagination controls are shown.
///
/// @us-view-004 @driving_port @real-io @pagination @edge
#[test]
fn small_store_needs_no_pagination_controls() {
    // GIVEN Maria has 12 claims (< one page).
    let env = TestEnv::initialized();
    seed_own_claims_via_cli(&env, 12);

    // WHEN she opens My Claims.
    let viewer = ViewerServer::start(&env);
    let page = viewer.get("/claims");

    // THEN all 12 render on one page with the "1–12 of 12" indicator and NO
    // page-navigation controls (no next/prev links).
    assert_eq!(page.status, 200, "the single page must render");
    assert!(
        page.body_contains("1–12 of 12"),
        "a single-page store must show the \"1–12 of 12\" indicator; body was:\n{}",
        page.body
    );
    assert!(
        !page.body_contains("?page=2") && !page.body_contains("?page="),
        "a store smaller than one page must show no pagination controls; \
         body was:\n{}",
        page.body
    );
}
