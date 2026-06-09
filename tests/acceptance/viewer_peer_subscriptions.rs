//! Slice-15 acceptance — the `openlore ui` PEER-SUBSCRIPTIONS view (US-PS-002/003;
//! ADR-052).
//!
//! The net-new LOCAL read-only route (DESIGN §Route and Handler Design): the operator
//! opens `GET /peers`; the viewer reads the ACTIVE subscription set over the read-only
//! DuckDB store (`StoreReadPort::list_active_peer_subscriptions` — `peer_subscriptions
//! LEFT JOIN peer_claims ON author_did = peer_did`, `WHERE removed_at IS NULL`,
//! `GROUP BY` the subscription identity, `COUNT(pc.cid)` — ONE aggregate query, NO N+1,
//! NO network, DD-PS-1/2), maps the `Vec<PeerSubscriptionSummary>` into a `PeersView` ADT
//! in the PURE `viewer-domain` core (`Subscriptions { peers } | NoSubscriptions`,
//! DD-PS-5), and renders it as HTML: per ACTIVE peer one row showing its DID VERBATIM +
//! its PER-PEER local claim count + the RENDER-ONLY `openlore peer remove <bare-did>`
//! revocation command (`render_remove_guidance`, mirroring the slice-08
//! `render_follow_guidance` render-only `openlore peer add` precedent, DD-PS-6). When the
//! active set is empty, a GUIDED empty state pointing to `openlore peer add <did>`
//! (US-PS-003). Served as a full page WITHOUT `HX-Request` and the SAME `#peers` region
//! fragment WITH it (the slice-07 `Shape` fork; page = chrome + fragment, parity by
//! construction — DD-PS-8 / I-PS-5).
//!
//! The CARDINAL product property (I-PS-2, residue made visible): a peer removed via the
//! CLI `peer remove` (soft-remove, `removed_at` set) VANISHES from `/peers` on the next
//! render — that ABSENCE IS the J-003c "revocable without residue" promise rendered — even
//! though its cached `peer_claims` remain on disk (no `--purge`). The view shows ONLY
//! active subscriptions (`removed_at IS NULL`). The per-peer count is NEVER a merged total
//! (J-003a / I-PS-3): two peers render their OWN counts (5 and 3), never a combined 8, and
//! there is no "all peers" row. A subscribed-but-never-pulled peer still appears with count
//! 0 (the LEFT JOIN + `COUNT(pc.cid)` design, DD-PS-2). The route is READ-ONLY: it holds
//! no signing key (I-PS-1), renders NO write/subscribe/unsubscribe control (the revocation
//! is render-only command TEXT only), and the read is a LOCAL DB read (I-PS-4, renders
//! network-down). It persists NOTHING (I-PS-6); workspace stays 21 (I-PS-7).
//!
//! Driving discipline (Mandate 1): scenarios enter through the REAL `openlore ui`
//! subprocess (`ViewerServer`) + in-test HTTP GET (with/without the `HX-Request` header —
//! the slice-07 `get`/`get_htmx` pair). The local DuckDB store is REAL, seeded through the
//! PRODUCTION federation write path (`peer add` + `peer pull` + `peer remove` via
//! `seed_peers_two_active_with_claims` / `seed_peer_subscribed_then_removed` /
//! `seed_peer_subscribed_zero_claims` — the SAME seam slice-09/10 use), so the rows the
//! read returns are produced by production code, not hand-inserted (Pillar 3 / BR-VIEW-4).
//! NO external/network boundary exists — `/peers` is LOCAL + OFFLINE (distinct from
//! `/scrape`'s GitHub edge and `/search`'s indexer edge; offline-STRONGER, I-PS-4). NO
//! scenario calls the `viewer-domain` `render_peers_*` / the read method directly (those
//! are unit/property-level, exercised in DELIVER) — every assertion is on the rendered HTML
//! the operator's browser shows (Mandate 8 universe = port-exposed rendered surface).
//!
//! Layer placement (nw-tdd-methodology Layered Test Discipline matrix): every scenario is a
//! layer-3/layer-5 subprocess + real-I/O test — EXAMPLE-only (Mandate 9/11). The sad paths
//! (empty state, removed-peer-absent, only-removed-empty) are enumerated explicitly, never
//! PBT-generated at this layer (the generative exploration of the pure projection +
//! `render_remove_guidance` bare-DID strip is a layer-1/2 DELIVER concern).
//!
//! Build-before-run note (carry into the DELIVER roadmap, mirrors slice-06..14): `cargo
//! test` does NOT rebuild a spawned binary automatically — the roadmap/run MUST `cargo
//! build` the `openlore` bin (the viewer) before running these ATs so `ViewerServer::start`
//! spawns the CURRENT viewer, not a stale one. `/peers` needs NO second binary — the read
//! is a LOCAL DuckDB read.
//!
//! Mandate 7 RED scaffolds: the ATs spawn the bin + HTTP, so they COMPILE now with the new
//! `seed_peers_*` / `seed_peer_*` / `assert_peer_*` helpers (which compile — they drive the
//! EXISTING `peer add`/`peer pull`/`peer remove` verbs + scan strings). Each scenario body
//! runs to a `/peers` HTTP assertion that FAILS because the production `/peers` route +
//! `list_active_peer_subscriptions` read + `PeersView` / `render_peers_*` /
//! `render_remove_guidance` seams do NOT exist yet (the route 404s / renders no `#peers`
//! region) → classifies RED (MISSING_FUNCTIONALITY), NOT BROKEN. They stay RED until
//! DELIVER's per-scenario RED→GREEN→COMMIT cycles (ADR-025).
//!
//! Covers:
//! - US-PS-002 (PS-1..PS-5): walking skeleton — GET /peers WITH HX-Request over two active
//!   peers → ONLY the `#peers` fragment, each row the DID VERBATIM + per-peer count +
//!   render-only `peer remove` command + no-JS full-page parity + per-peer-never-merged
//!   (5 and 3, never 8) + removed-peer-absent (residue made visible) + zero-claims peer
//!   appears with count 0.
//! - US-PS-003 (PS-6..PS-7): guided empty state (no active subscriptions) + the SAME empty
//!   state when the ONLY subscription was soft-removed (residue, not an active sub).
//
// SCAFFOLD: true

mod support;

#[allow(unused_imports)]
use support::*;

// =============================================================================
// US-PS-002 — see, on /peers, every peer I currently follow: its DID + per-peer local
// claim count + the render-only `openlore peer remove <did>` command (PS-1..PS-5).
// PS-1 is the thinnest end-to-end thread (the walking skeleton).
// =============================================================================

/// PS-1 / WALKING SKELETON (US-PS-002; AC theme 1; the riskiest-assumption thread): from
/// the LOCAL store, `GET /peers` WITH the `HX-Request` header over two ACTIVE peers
/// (Rachel 5 claims, Tobias 3 claims, seeded via the real `peer add` + `peer pull`) returns
/// ONLY the `#peers` fragment — two attributed peer rows, each showing the DID VERBATIM +
/// its per-peer claim count + the render-only `openlore peer remove <did>` command — with
/// NO full-page chrome. This is the thinnest complete thread the slice can demo: viewer →
/// LOCAL active-subscription read → pure projection → HTML fragment, proving the read-only
/// viewer can host a federation-management VIEWING surface while preserving the read-only /
/// active-only / per-peer / local-first / progressive-enhancement invariants.
///
/// Given Maria actively follows did:plc:rachel-test (5 cached claims) and
///   did:plc:tobias-test (3 cached claims);
/// When she opens GET /peers WITH the htmx header;
/// Then she receives ONLY the `#peers` fragment (no chrome), two rows, each attributing its
///   peer's DID + per-peer count + the render-only `openlore peer remove <did>` command.
///
/// @us-ps-002 @walking_skeleton @driving_port @driving_adapter @real-io @htmx-fragment
/// @i-ps-2 @i-ps-3 @i-ps-5 @i-ps-8 @kpi-fed-4 @happy
#[test]
fn open_peers_with_htmx_returns_only_the_peers_fragment_with_did_count_and_revoke_command() {
    // GIVEN a REAL local store seeded (production `peer add` + `peer pull` path) with two
    // ACTIVE peers — Rachel (5 cached claims) + Tobias (3 cached claims). NO network:
    // `/peers` reads the LOCAL store.
    //
    // WHEN Maria submits `GET /peers` WITH the HX-Request header (get_htmx).
    //
    // THEN the response is ONLY the `#peers` fragment (`is_fragment()`, NOT a full page),
    // with two attributed rows — Rachel (DID verbatim, count 5, render-only remove command)
    // and Tobias (DID verbatim, count 3, render-only remove command).
    let env = TestEnv::initialized();
    seed_peers_two_active_with_claims(&env);
    let viewer = ViewerServer::start(&env);

    let response = viewer.get_htmx(PEERS_PATH);

    assert_eq!(
        response.status, 200,
        "PS-1: GET /peers over a store with two active peers must return 200; body was:\n{}",
        response.body
    );
    assert!(
        response.content_type.contains("text/html"),
        "PS-1: the /peers fragment must be served as text/html; content-type was {:?}",
        response.content_type
    );
    // WITH the HX-Request header the viewer returns ONLY the `#peers` fragment — no
    // full-page chrome (I-PS-5).
    assert!(
        response.is_fragment(),
        "PS-1: an HX-Request `/peers` response must be ONLY the fragment (no chrome); body \
         was:\n{}",
        response.body
    );
    assert!(
        response.body_contains(PEERS_REGION_ID),
        "PS-1: the fragment must carry the `#peers` swap-target region id; body was:\n{}",
        response.body
    );
    // Two attributed rows — each its DID VERBATIM + its PER-PEER claim count (5 / 3) + the
    // render-only `openlore peer remove <did>` command. The thinnest demoable thread.
    assert_peer_row_present(&response.body, PEERS_RACHEL_DID, PEERS_RACHEL_CLAIM_COUNT);
    assert_peer_row_present(&response.body, PEERS_TOBIAS_DID, PEERS_TOBIAS_CLAIM_COUNT);
    assert_peer_remove_command_is_render_only(&response.body, PEERS_RACHEL_DID);
    assert_peer_remove_command_is_render_only(&response.body, PEERS_TOBIAS_DID);
}

/// PS-2 (US-PS-002; AC theme 6 — no-JS full page + parity): `GET /peers` WITHOUT
/// `HX-Request` serves a COMPLETE full page (chrome + the SAME `#peers` region) whose peers
/// region is STRUCTURALLY IDENTICAL to the htmx fragment — parity by construction (the page
/// EMBEDS the fragment fn; DD-PS-8 / I-PS-5). The no-JS no-regression contract (KPI-HX-G1):
/// the full page is the contract, the htmx swap is a nicety.
///
/// Given Maria actively follows one peer;
/// When she requests GET /peers WITH HX-Request and again WITHOUT it;
/// Then the no-JS response is a full page (chrome) and the htmx response is the bare
///   fragment, and the `#peers` region is identical between them.
///
/// @us-ps-002 @driving_port @real-io @no-js @full-page @parity @i-ps-5 @happy
#[test]
fn the_peers_list_full_page_and_fragment_render_the_same_region() {
    // GIVEN two active peers. WHEN `get` (no HX-Request) AND `get_htmx`. THEN `get` is_full_
    // page(), `get_htmx` is_fragment(), and the `#peers` region is the same in both (the
    // full page embeds the fragment — parity by construction; I-PS-5).
    let env = TestEnv::initialized();
    seed_peers_two_active_with_claims(&env);
    let viewer = ViewerServer::start(&env);

    let full = viewer.get(PEERS_PATH);
    let fragment = viewer.get_htmx(PEERS_PATH);

    assert_eq!(full.status, 200, "PS-2: the no-JS request must return 200");
    assert_eq!(fragment.status, 200, "PS-2: the htmx request must return 200");
    // The shapes differ only in chrome: the no-JS request is a full page, the HX-Request
    // response is the bare fragment (no chrome) — I-PS-5.
    assert!(
        full.is_full_page(),
        "PS-2: the no-JS response must be a complete full page (chrome present); body \
         was:\n{}",
        full.body
    );
    assert!(
        fragment.is_fragment(),
        "PS-2: the HX-Request response must be a bare fragment (no chrome); body was:\n{}",
        fragment.body
    );
    // The fragment IS the `#peers` region; the full page EMBEDS the SAME fragment fn, so
    // the fragment body appears verbatim inside the full page — parity by construction
    // (I-PS-5).
    assert!(
        fragment.body.contains(PEERS_REGION_ID),
        "PS-2: the fragment must carry the `#peers` region; body was:\n{}",
        fragment.body
    );
    assert!(
        full.body.contains(fragment.body.trim()),
        "PS-2: the full page's `#peers` region must be identical to the fragment (parity \
         by construction; the page embeds the fragment fn). fragment:\n{}\nfull page:\n{}",
        fragment.body,
        full.body
    );
}

/// PS-3 / CARDINAL anti-merging (US-PS-002; AC theme 7 / J-003a / I-PS-3): two followed
/// peers render their OWN per-peer counts (5 and 3), NEVER a merged total (8), and there is
/// NO "all peers" row — each peer is its own attributed row keyed by its DID. The cardinal
/// anti-merging scenario for the subscription surface (the existing per-peer
/// `count_peer_claims(conn, peer_did)` shape lifted to the whole active set in ONE
/// aggregate query).
///
/// Given Maria follows did:plc:rachel-test (5 cached claims) and did:plc:tobias-test
///   (3 cached claims);
/// When she opens GET /peers;
/// Then Rachel's row shows 5 and Tobias's row shows 3 (per-peer), no row shows a combined
///   total of 8, and there is no merged "all peers" row.
///
/// @us-ps-002 @driving_port @real-io @anti-merging @i-ps-3 @kpi-av-2 @boundary
#[test]
fn the_per_peer_count_is_never_a_merged_total() {
    // GIVEN two active peers with DISTINCT counts (5 vs 3). WHEN `get` /peers. THEN both
    // counts render PER-PEER (5 + 3), there is NO combined-8 total, and NO merged "all
    // peers" / "consensus" row (anti-merging; J-003a / I-PS-3).
    let env = TestEnv::initialized();
    seed_peers_two_active_with_claims(&env);
    let viewer = ViewerServer::start(&env);

    let response = viewer.get(PEERS_PATH);

    assert_eq!(
        response.status, 200,
        "PS-3: GET /peers for the two-peers store must return 200; body was:\n{}",
        response.body
    );
    // Each peer renders its OWN per-peer count under its OWN attributed row (5 + 3).
    assert_peer_row_present(&response.body, PEERS_RACHEL_DID, PEERS_RACHEL_CLAIM_COUNT);
    assert_peer_row_present(&response.body, PEERS_TOBIAS_DID, PEERS_TOBIAS_CLAIM_COUNT);
    // NO merged total of 8 (5 + 3 averaged/summed into a consensus count) — the merged
    // number must NEVER appear, and there is NO "all peers" aggregate row (anti-merging).
    let merged_total = (PEERS_RACHEL_CLAIM_COUNT + PEERS_TOBIAS_CLAIM_COUNT).to_string();
    assert!(
        !response.body.contains(&format!("{merged_total} claims")),
        "PS-3 (J-003a / I-PS-3): the /peers render must NEVER show a merged total of \
         {merged_total} claims (each peer's count is PER-PEER, never summed/averaged); body \
         was:\n{}",
        response.body
    );
    let lowered = response.body.to_ascii_lowercase();
    for banned in ["all peers", "all subscriptions", "consensus", "combined total", "total across"] {
        assert!(
            !lowered.contains(banned),
            "PS-3 (J-003a / I-PS-3): the /peers render must carry NO merged \"all peers\" / \
             consensus aggregate row ({banned:?}); each peer is its own attributed row keyed \
             by its DID; body was:\n{}",
            response.body
        );
    }
}

/// PS-4 / CARDINAL active-only / residue made visible (US-PS-002 Ex 3; AC theme 2 / I-PS-2,
/// CARDINAL): a peer the operator removed via the CLI `peer remove` (soft-remove, no
/// `--purge`) is ABSENT from `/peers` on the next render — that absence IS the J-003c
/// "revocable without residue" promise rendered — EVEN THOUGH its cached `peer_claims`
/// remain on disk. The read filters `removed_at IS NULL`; the soft-removed row is residue,
/// not an active subscription. The defining product property of the slice.
///
/// Given Maria subscribed + pulled did:plc:rachel-test, THEN soft-removed it via
///   `openlore peer remove did:plc:rachel-test` (its cached claims remain, no --purge);
/// When she reopens GET /peers;
/// Then did:plc:rachel-test is ABSENT from the list, and its absence holds even though its
///   cached peer claims remain on disk.
///
/// @us-ps-002 @driving_port @real-io @active-only @residue-made-visible @i-ps-2
/// @kpi-fed-4 @boundary
#[test]
fn a_peer_removed_via_the_cli_is_absent_from_peers_even_though_its_cache_remains() {
    // GIVEN Rachel subscribed + pulled (2 cached claims), THEN soft-removed via the real
    // `peer remove` verb (no --purge): her `removed_at` is set, her cached `peer_claims`
    // survive (the seed pins both with `assert_subscription_soft_removed_for` +
    // `assert_peer_claims_row_count_for`). WHEN `get` /peers. THEN Rachel is ABSENT — the
    // active-only filter (`removed_at IS NULL`) excludes her; her absence IS the residue-
    // free promise rendered (I-PS-2), even though her cache remains on disk.
    let env = TestEnv::initialized();
    seed_peer_subscribed_then_removed(&env);
    let viewer = ViewerServer::start(&env);

    let response = viewer.get(PEERS_PATH);

    assert_eq!(
        response.status, 200,
        "PS-4: GET /peers after a soft-remove must return a calm 200 (the only remaining \
         subscription was removed → the guided empty state, never a 5xx); body was:\n{}",
        response.body
    );
    // Rachel (soft-removed) is ABSENT from the render — the active-only filter excludes
    // her; her absence IS the J-003c residue-free promise rendered (I-PS-2). Her cached
    // peer_claims remain on disk (no --purge) but NEVER surface on /peers.
    assert_peer_absent(&response.body, PEERS_RACHEL_DID);
}

/// PS-5 (US-PS-002 Ex 2; AC theme 1 / DD-PS-2): a peer SUBSCRIBED but NEVER pulled (zero
/// cached claims) still appears on `/peers` with a per-peer claim count of 0 — proving the
/// LEFT JOIN + `COUNT(pc.cid)` design keeps the row at 0 (not dropped by an inner JOIN, not
/// counted as 1 by `COUNT(*)`-of-NULL) — alongside its render-only revoke command.
///
/// Given Maria subscribed to did:plc:newpeer-test but has never run openlore peer pull;
/// When she opens GET /peers;
/// Then did:plc:newpeer-test appears with a local claim count of 0, and its render-only
///   `openlore peer remove did:plc:newpeer-test` command is shown.
///
/// @us-ps-002 @driving_port @real-io @left-join @zero-claims @i-ps-8 @boundary
#[test]
fn a_followed_peer_with_zero_cached_claims_appears_with_count_zero_and_its_revoke_command() {
    // GIVEN a peer subscribed via `peer add` ALONE (no `peer pull`) → ONE active
    // subscription, ZERO cached claims (the seed pins both with
    // `assert_one_active_subscription_for` + `assert_peer_claims_row_count_for(…, 0)`).
    // WHEN `get` /peers. THEN newpeer appears with a per-peer count of 0 (the LEFT JOIN +
    // COUNT(pc.cid) keeps the row at 0, DD-PS-2) + its render-only revoke command. The held
    // PDS is kept alive for the duration so the subscription stays consistent.
    let env = TestEnv::initialized();
    let _held = seed_peer_subscribed_zero_claims(&env);
    let viewer = ViewerServer::start(&env);

    let response = viewer.get(PEERS_PATH);

    assert_eq!(
        response.status, 200,
        "PS-5: GET /peers with one subscribed-but-never-pulled peer must return 200; body \
         was:\n{}",
        response.body
    );
    // The never-pulled peer appears with a per-peer count of 0 (NOT dropped, NOT counted as
    // 1) + its render-only revoke command (DD-PS-2; I-PS-8).
    assert_peer_row_present(&response.body, PEERS_NEWPEER_DID, 0);
    assert_peer_remove_command_is_render_only(&response.body, PEERS_NEWPEER_DID);
}

// =============================================================================
// US-PS-003 — when I follow no peers, see a guided empty state (PS-6..PS-7).
// =============================================================================

/// PS-6 (US-PS-003 Ex 1; AC theme 4): a store with NO active subscriptions renders a guided
/// empty state — "You are not subscribed to any peers." + the render-only `openlore peer
/// add <did>` starting command — never blank, never an error. Both shapes (htmx fragment +
/// no-JS full page) render the SAME guided empty state (parity, I-PS-5).
///
/// Given Maria has no active peer subscriptions;
/// When she opens GET /peers (no HX-Request) and again WITH it;
/// Then both shapes render the guided empty state naming "no peers" + the render-only
///   `openlore peer add <did>` starting command, and neither is blank nor an error.
///
/// @us-ps-003 @driving_port @real-io @empty-state @parity @i-ps-5 @error
#[test]
fn no_active_subscriptions_shows_the_guided_empty_state_in_both_shapes() {
    // GIVEN a freshly initialized store with NO active subscriptions
    // (`seed_no_active_subscriptions`). WHEN `get` AND `get_htmx` /peers. THEN both shapes
    // render the guided empty state ("You are not subscribed to any peers." + the render-
    // only `openlore peer add <did>` starting command) — a calm 200, never blank, never an
    // error (US-PS-003).
    let env = TestEnv::initialized();
    seed_no_active_subscriptions(&env);
    let viewer = ViewerServer::start(&env);

    let full = viewer.get(PEERS_PATH);
    let fragment = viewer.get_htmx(PEERS_PATH);

    assert_eq!(
        full.status, 200,
        "PS-6: GET /peers (no HX-Request) with no subscriptions must return a calm 200 \
         guided empty state, never a 5xx; body was:\n{}",
        full.body
    );
    assert_eq!(
        fragment.status, 200,
        "PS-6: GET /peers (HX-Request) with no subscriptions must return a calm 200 guided \
         empty state; body was:\n{}",
        fragment.body
    );
    assert!(
        full.is_full_page(),
        "PS-6: the no-JS empty-state response must be a complete full page (chrome \
         present); body was:\n{}",
        full.body
    );
    assert!(
        fragment.is_fragment(),
        "PS-6: the HX-Request empty-state response must be a bare fragment (no chrome); \
         body was:\n{}",
        fragment.body
    );
    // BOTH shapes render the guided empty state (named "no peers" + the render-only
    // `openlore peer add <did>` starting command; never blank, never an error — US-PS-003).
    assert_peers_empty_state_present(&full.body);
    assert_peers_empty_state_present(&fragment.body);
}

/// PS-7 (US-PS-003 Ex 2; AC theme 4 / I-PS-2): a store whose ONLY subscription was
/// soft-removed still renders the guided empty state — the soft-removed row is residue, not
/// an active subscription, so the active-only read yields an empty result. The chained
/// narrative continuation of PS-4 (the residue scenario): Given+When of "Rachel soft-
/// removed" = Given of "the only subscription was removed → empty state" (Pillar 2).
///
/// Given Maria's only peer_subscriptions row is soft-removed (removed_at set) and she
///   follows no one else;
/// When she opens GET /peers;
/// Then she sees the guided empty state (the soft-removed row is residue, not an active
///   subscription).
///
/// @us-ps-003 @driving_port @real-io @empty-state @active-only @i-ps-2 @error
#[test]
fn a_store_with_only_a_soft_removed_peer_still_shows_the_empty_state() {
    // GIVEN a store whose ONLY subscription was soft-removed
    // (`seed_only_subscription_removed` → one `peer_subscriptions` row, soft-removed, no
    // other active peer; the chained continuation of PS-4). WHEN `get` /peers. THEN the
    // guided empty state renders (the active-only read yields an empty result; the soft-
    // removed row is residue, not an active subscription — I-PS-2 / US-PS-003 Ex 2).
    let env = TestEnv::initialized();
    seed_only_subscription_removed(&env);
    let viewer = ViewerServer::start(&env);

    let response = viewer.get(PEERS_PATH);

    assert_eq!(
        response.status, 200,
        "PS-7: GET /peers with only a soft-removed subscription must return a calm 200 \
         guided empty state; body was:\n{}",
        response.body
    );
    // The soft-removed peer is ABSENT (residue, not an active subscription) AND the guided
    // empty state renders (the active set is empty) — US-PS-003 Ex 2 / I-PS-2.
    assert_peer_absent(&response.body, PEERS_RACHEL_DID);
    assert_peers_empty_state_present(&response.body);
}
