//! Slice-16 acceptance — the `openlore ui` `/search` FOLLOW-STATE accuracy
//! (US-SF-001/002; ADR-053).
//!
//! This slice closes the discovery→follow loop on the existing read-only
//! `GET /search` view (slice-08). Today the viewer's `to_indexed_claim`
//! (`crates/adapter-http-viewer/src/lib.rs` ~line 1021) hardcodes
//! `AuthorRelationship::NetworkUnfollowed` for EVERY result author, so the slice-08
//! render-only `openlore peer add <did>` follow affordance is offered even for
//! authors the operator ALREADY follows. slice-16 RESOLVES each result author's
//! relationship in the EFFECT shell against the operator's LOCAL active peer
//! subscriptions (the slice-15 `list_active_peer_subscriptions` read, REUSED,
//! batch-once) and adds ONE pure render arm so an already-followed author
//! (`SubscribedPeer`) shows a neutral "Following" indicator (NO `peer add` command),
//! while a genuinely-unfollowed author (`NetworkUnfollowed`) keeps the slice-08
//! render-only `openlore peer add <did>` affordance.
//!
//! Driving discipline (Mandate 1): scenarios enter through the REAL `openlore ui`
//! subprocess (`ViewerServer`) + in-test HTTP GET /search (with/without the
//! `HX-Request` header). The network index is the ONLY mocked boundary — a REAL
//! slice-05 `openlore-indexer serve` over a seeded corpus
//! (`seed_network_index_from_specs` → `ViewerServer::start_with_indexer`). The LOCAL
//! DuckDB store is REAL: an active subscription whose bare DID equals a search-result
//! author is seeded via the REAL slice-03 `peer add` verb
//! (`seed_active_subscription_for`), so the SAME store the viewer reads holds the
//! active-subscription row. NO scenario calls the `viewer-domain` render fns or the
//! adapter resolution fn directly (those are unit-level, exercised in DELIVER).
//!
//! Seeding alignment (the load-bearing trick): the slice-08 search corpus is keyed on
//! REAL bare DIDs (`RACHEL_DID` = `did:plc:rachel-test`, `PRIYA_DID`, `TOBIAS_DID`),
//! and the slice-03 `peer add` verb writes `peer_subscriptions` rows keyed on the SAME
//! bare DIDs. So Rachel (`did:plc:rachel-test`) seeded as an active subscription AND
//! present in the index resolves to `SubscribedPeer`. The result row's `author_did`
//! carries the `#org.openlore.application` signing fragment; the bare active-set DID
//! matches it after the production `bare_did` strip (R-SF-5).
//!
//! Layer placement (nw-tdd-methodology Layered Test Discipline matrix): every scenario
//! is a layer-3/layer-5 subprocess + real-I/O test — EXAMPLE-only (Mandate 9/11). Sad
//! paths (none-followed status quo, failed active-set read) are enumerated explicitly,
//! never PBT-generated at this layer. Tier B (state-machine PBT) is NOT warranted:
//! the resolution is a binary per-row enrichment (DID ∈ active set or not), not a
//! ≥3-scenario chained journey over a rich state machine (Mandate 10 skip criteria —
//! the observable is "which render-only affordance", a per-row config-shaped choice).
//!
//! Build-before-run note (mirrors slice-08): `cargo test` does NOT rebuild a spawned
//! binary automatically — the run MUST `cargo build` BOTH the `openlore` bin (the
//! viewer) AND the `openlore-indexer` bin (the seeded serve) before running these ATs
//! so `ViewerServer::start_with_indexer` spawns the CURRENT viewer over a CURRENT
//! indexer, not a stale one.
//!
//! Mandate 7 RED scaffolds: the ATs import nothing unbuilt at the Rust level (they
//! spawn the bins + HTTP), so they COMPILE now. The follow-state RED is the PRODUCTION
//! arm: `to_indexed_claim` hardcodes `NetworkUnfollowed`, so a followed author still
//! renders `peer add` → `assert_search_row_following` FAILS for the RIGHT reason
//! (MISSING_FUNCTIONALITY: the `SubscribedPeer` resolution + the `render_following_
//! indicator` arm + `SEARCH_FOLLOWING_INDICATOR` do not exist yet), NOT a setup/import
//! error. The graceful-degrade E2 scenario panics at the `todo!()` fault-injection
//! seam (also MISSING_FUNCTIONALITY). They stay RED until DELIVER's per-scenario
//! RED→GREEN→COMMIT cycles (ADR-025).
//!
//! Covers:
//! - US-SF-002 / Theme A (accuracy, the load-bearing fix): SF-1 walking skeleton
//!   (one followed + one unfollowed, side by side) · SF-2 all-followed · SF-3
//!   none-followed status quo.
//! - US-SF-002 / Theme B (read-only / no write): SF-4 neither affordance is an
//!   executable control.
//! - US-SF-001 / Theme C (LOCAL / offline resolution): SF-5 LOCAL resolution, index
//!   per-user-neutral · SF-6 fragment-strip match.
//! - US-SF-001 / Theme D (no N+1): SF-7 one batch read, invariant to result count.
//! - US-SF-001 / Theme E (graceful degrade): SF-8 failed active-set read degrades to
//!   the slice-08 status quo (RED scaffold via the fault-injection seam).
//! - US-SF-002 / Theme F (htmx vs no-JS parity): SF-9 the follow-state renders
//!   identically under fragment + full page.
//! - US-SF-002 / Theme G (attribution + ranking unchanged): SF-10 following +
//!   unfollowed authors render side by side, grouping/order/verified/confidence
//!   unchanged.
//
// SCAFFOLD: true

mod support;

#[allow(unused_imports)]
use support::*;

use openlore_test_support::{PRIYA_DID, RACHEL_DID};

// The slice-08 keypair seeds are deterministic per fixture DID; the slice-16 active-
// subscription seed reuses the slice-15 `peer add` seam with its own deterministic
// seeds (DISTINCT per peer DID so the verifiable records do not alias).
const RACHEL_ACTIVE_SUB_SEED: [u8; 32] = [7u8; 32];
const TOBIAS_ACTIVE_SUB_SEED: [u8; 32] = [9u8; 32];

// =============================================================================
// US-SF-002 — Theme A: ACCURACY (the load-bearing fix). A followed author shows
// "Following" + no add; an unfollowed author keeps `peer add`.
// (SF-1 walking skeleton · SF-2 all-followed · SF-3 none-followed status quo)
// =============================================================================

/// SF-1 / WALKING SKELETON (US-SF-002 / Theme A; C-2; R-SF-3/-4 — the riskiest,
/// load-bearing thread): in ONE `/search` over a reachable index that returns BOTH a
/// followed author (Rachel, seeded as an active subscription) AND a genuinely-
/// unfollowed author (Priya), Rachel's row shows the neutral render-only "Following"
/// indicator and NO `openlore peer add` command, while Priya's row KEEPS the slice-08
/// render-only `openlore peer add did:plc:priya-test` affordance. This is the thinnest
/// complete thread the slice can demo end-to-end: viewer → LOCAL active-set read →
/// per-row resolution → DIFFERENTIATED render-only affordances on the SAME page.
///
/// Given Maria actively follows did:plc:rachel-test but not did:plc:priya-test, and a
///   reachable index holds verified reproducible-builds claims by BOTH;
/// When she opens GET /search?object=reproducible-builds and both claims appear;
/// Then Rachel's row shows the neutral "Following" indicator and NO `peer add` command,
///   and Priya's row keeps the render-only `openlore peer add did:plc:priya-test`.
///
/// @us-sf-002 @walking_skeleton @driving_port @driving_adapter @real-io
/// @follow-state-accuracy @search-state-results @c-2 @happy
#[test]
fn a_followed_author_shows_following_while_an_unfollowed_author_keeps_peer_add() {
    // GIVEN Maria actively follows Rachel (a REAL `peer add` writing the
    // `peer_subscriptions` row keyed on the bare did:plc:rachel-test) but NOT Priya,
    // AND a REAL `openlore-indexer serve` over a corpus where BOTH Rachel and Priya
    // assert the headline reproducible-builds object. The active subscription is
    // seeded into the SAME REAL DuckDB the viewer opens (OPENLORE_HOME), so the LOCAL
    // active-set read the viewer performs sees Rachel's row.
    let env = TestEnv::initialized();
    let _rachel_sub = seed_active_subscription_for(&env, RACHEL_DID, RACHEL_ACTIVE_SUB_SEED);
    let indexer = seed_network_index_from_specs(&env, sf_corpus_one_followed_one_unfollowed());
    let viewer = ViewerServer::start_with_indexer(&env, indexer);

    // WHEN Maria opens the object search (full page).
    let response = viewer.get(&format!("/search?object={SF_OBJECT_REPRODUCIBLE_BUILDS}"));

    assert_eq!(
        response.status, 200,
        "SF-1: GET /search over a reachable seeded index must be 200; body:\n{}",
        response.body
    );
    // THEN both authors are still attributed (the relationship is a per-row
    // enrichment; attribution + the verified marker are UNCHANGED — C-5).
    assert_search_html_every_row_verified_and_attributed(&response.body, &[RACHEL_DID, PRIYA_DID]);
    // …Rachel (FOLLOWED) shows the neutral "Following" indicator and NO `peer add`
    // command — the already-followed author is NOT re-offered a follow (R-SF-3, the
    // core bug this slice fixes). THIS is the RED assertion: today `to_indexed_claim`
    // hardcodes NetworkUnfollowed, so Rachel still renders `peer add` and this fails
    // for the RIGHT reason (the SubscribedPeer resolution + render arm are MISSING).
    assert_search_row_following(&response.body, RACHEL_DID);
    // …Priya (genuinely UNFOLLOWED) KEEPS the slice-08 render-only `openlore peer add
    // did:plc:priya-test` affordance — no over-correction (R-SF-4).
    assert_search_row_offers_follow(&response.body, PRIYA_DID);
}

/// SF-2 (US-SF-002 / Theme A; C-2 / FR-SF-4): when EVERY result author is followed
/// (Rachel + Tobias, both seeded as active subscriptions), every row shows the neutral
/// "Following" indicator and NO `openlore peer add` command appears ANYWHERE in the
/// results — the all-followed accuracy case.
///
/// Given Maria follows both did:plc:rachel-test and did:plc:tobias-test, and the index
///   holds claims only by those two;
/// When she opens GET /search;
/// Then every row shows "Following" and no `openlore peer add` command appears anywhere.
///
/// @us-sf-002 @driving_port @real-io @follow-state-accuracy @search-state-results
/// @c-2 @happy
#[test]
fn all_followed_results_show_following_everywhere_and_no_add_command_anywhere() {
    // GIVEN Maria follows BOTH Rachel and Tobias (two REAL `peer add`s), AND a
    // reachable index whose results are ONLY by those two.
    let env = TestEnv::initialized();
    let _rachel_sub = seed_active_subscription_for(&env, TRAVERSAL_AUTHOR_RACHEL, RACHEL_ACTIVE_SUB_SEED);
    let _tobias_sub = seed_active_subscription_for(&env, TRAVERSAL_AUTHOR_TOBIAS, TOBIAS_ACTIVE_SUB_SEED);
    let indexer = seed_network_index_from_specs(&env, sf_corpus_all_authors_followed());
    let viewer = ViewerServer::start_with_indexer(&env, indexer);

    let response = viewer.get(&format!("/search?object={SF_OBJECT_REPRODUCIBLE_BUILDS}"));

    assert_eq!(
        response.status, 200,
        "SF-2: GET /search over the all-followed seeded index must be 200; body:\n{}",
        response.body
    );
    // Both authors attributed + verified (unchanged).
    assert_search_html_every_row_verified_and_attributed(
        &response.body,
        &[TRAVERSAL_AUTHOR_RACHEL, TRAVERSAL_AUTHOR_TOBIAS],
    );
    // Every row shows the neutral "Following" indicator and NO `peer add` for it (RED:
    // today both still render `peer add`).
    assert_search_row_following(&response.body, TRAVERSAL_AUTHOR_RACHEL);
    assert_search_row_following(&response.body, TRAVERSAL_AUTHOR_TOBIAS);
    // …and NO `openlore peer add` command appears ANYWHERE in the results (the
    // strongest all-followed guarantee — not merely "not for these two DIDs" but the
    // verb is absent entirely from the rendered surface).
    assert!(
        !response.body.contains(SF_FOLLOW_COMMAND_VERB),
        "SF-2 (C-2): when every result author is followed, NO `{SF_FOLLOW_COMMAND_VERB}` \
         command must appear ANYWHERE in the results; body:\n{}",
        response.body
    );
}

/// SF-3 (US-SF-002 / Theme A; C-2 / FR-SF-5 — the no-over-correction boundary + the
/// graceful-degrade TARGET state): when the operator follows NOBODY who appears in the
/// results, every row keeps the render-only `openlore peer add <did>` guidance — this
/// is EXACTLY the slice-08 behavior, unchanged. (This is the OBSERVABLE degrade-target
/// the failed-active-set-read scenario SF-8 degrades TO, fully exercisable today over
/// an empty active set.)
///
/// Given Maria follows nobody who appears in the results, and the index holds claims by
///   did:plc:priya-test and an unfollowed author;
/// When she opens GET /search;
/// Then both rows show the render-only `openlore peer add <did>` guidance and NO
///   "Following" indicator appears — exactly the slice-08 status quo.
///
/// @us-sf-002 @driving_port @real-io @follow-state-accuracy @search-state-results
/// @status-quo @c-2 @boundary
#[test]
fn none_followed_results_preserve_the_slice08_status_quo() {
    // GIVEN Maria follows NOBODY (a fresh store — zero `peer_subscriptions` rows; the
    // empty active set), AND a reachable index holding the slice-08 headline corpus
    // (nine unfollowed authors). No `seed_active_subscription_for` is called.
    let env = TestEnv::initialized();
    let indexer = seed_network_index(&env, NetworkIndexFixture::ReproducibleBuildsNineAuthorsUnfollowed);
    let viewer = ViewerServer::start_with_indexer(&env, indexer);

    let response = viewer.get(&format!("/search?object={SF_OBJECT_REPRODUCIBLE_BUILDS}"));

    assert_eq!(
        response.status, 200,
        "SF-3: GET /search over the none-followed seeded index must be 200; body:\n{}",
        response.body
    );
    // Priya (unfollowed) is in the headline corpus and KEEPS the render-only `openlore
    // peer add did:plc:priya-test` affordance (the slice-08 behavior, unchanged).
    assert_search_row_offers_follow(&response.body, PRIYA_DID);
    // …and NO "Following" indicator appears anywhere (nobody is followed → every author
    // resolves to NetworkUnfollowed → no SubscribedPeer arm fires). This is exactly the
    // slice-08 status quo — the empty-active-set degrade TARGET (C-7 SF-8 degrades TO
    // this same observable state).
    assert!(
        !response.body.contains(SF_FOLLOWING_INDICATOR),
        "SF-3 (C-2 / status quo): when the operator follows nobody in the results, NO \
         {SF_FOLLOWING_INDICATOR:?} indicator must appear (every author NetworkUnfollowed \
         — the slice-08 status quo, unchanged); body:\n{}",
        response.body
    );
}

// =============================================================================
// US-SF-002 — Theme B: READ-ONLY / NO WRITE (CARDINAL). Neither affordance is an
// executable control. (SF-4)
// =============================================================================

/// SF-4 (US-SF-002 / Theme B; C-1, CARDINAL / NFR-SF-1 / WD-SF-1): over a render that
/// carries BOTH a "Following" indicator (Rachel) AND a `peer add` affordance (Priya),
/// NEITHER is an executable control — both are render-only TEXT, no `<button>`, no
/// `<form>`, no mutating `<a>`, no follow/subscribe input, no `hx-*` mutation. The
/// viewer holds no key and exposes no follow/unfollow route. The slice-16 companion to
/// the slice-08 N-17 + the `no_search_response_adds_a_write_or_sign_control` gold,
/// extended so the NEW "Following" arm adds no control either.
///
/// Given any /search result render carrying both followed and unfollowed authors;
/// When the results render;
/// Then neither the "Following" indicator nor the `peer add` guidance is an executable
///   control, and the viewer exposes no follow/unfollow route.
///
/// @us-sf-002 @driving_port @real-io @read-only @c-1 @nfr-sf-1 @happy
#[test]
fn neither_follow_state_affordance_is_an_executable_control() {
    // GIVEN a mix render (Rachel followed → "Following"; Priya unfollowed → `peer add`)
    // so BOTH affordances are present on the SAME surface.
    let env = TestEnv::initialized();
    let _rachel_sub = seed_active_subscription_for(&env, RACHEL_DID, RACHEL_ACTIVE_SUB_SEED);
    let indexer = seed_network_index_from_specs(&env, sf_corpus_one_followed_one_unfollowed());
    let viewer = ViewerServer::start_with_indexer(&env, indexer);

    // WHEN the results render in BOTH shapes (full page + htmx fragment) — the
    // read-only contract holds across every shape.
    let full_page = viewer.get(&format!("/search?object={SF_OBJECT_REPRODUCIBLE_BUILDS}"));
    let fragment = viewer.get_htmx(&format!("/search?object={SF_OBJECT_REPRODUCIBLE_BUILDS}"));

    for (label, response) in [("full page", &full_page), ("htmx fragment", &fragment)] {
        assert_eq!(
            response.status, 200,
            "SF-4: GET /search ({label}) over the mix seeded index must be 200; body:\n{}",
            response.body
        );
        // THEN NEITHER affordance is an executable control — both are render-only TEXT
        // (C-1, CARDINAL). The NEW "Following" arm must add no control either.
        assert_search_follow_state_is_render_only(&response.body);
    }
}

// =============================================================================
// US-SF-001 — Theme C: LOCAL / offline relationship resolution (per-user-neutral
// index). (SF-5 LOCAL resolution · SF-6 fragment-strip match)
// =============================================================================

/// SF-5 (US-SF-001 / Theme C; C-3 / NFR-SF-4 / WD-SF-4): the relationship is resolved
/// against the LOCAL active-subscription set, not the network. A followed author
/// (Rachel, an active LOCAL subscription) resolves to "Following" using the LOCAL read;
/// the network index query is UNCHANGED and per-user-neutral (it never learns who the
/// operator follows). Behavioral proxy: the SAME reachable index + the SAME query
/// produces DIFFERENT follow-state affordances depending only on the LOCAL active set —
/// proving the resolution is LOCAL (the index, being per-user-neutral, returns the SAME
/// rows regardless of who follows whom; only the LOCAL set changes the affordance).
///
/// Given Maria actively follows did:plc:rachel-test (a LOCAL subscription);
/// When she opens GET /search and a claim by Rachel appears;
/// Then Rachel's row resolves to "Following" using the LOCAL active set, and the index
///   query is unchanged + per-user-neutral (the affordance flips with the LOCAL set,
///   not with any network state).
///
/// @us-sf-001 @driving_port @real-io @local-resolution @offline @c-3 @nfr-sf-4 @happy
#[test]
fn the_relationship_is_resolved_against_the_local_active_set_not_the_network() {
    // GIVEN the SAME reachable index corpus (Rachel + Priya assert the object) — the
    // per-user-neutral network corpus is IDENTICAL regardless of who Maria follows.
    // Maria follows Rachel LOCALLY (the active subscription is a LOCAL DuckDB row, NOT
    // anything the index knows). The index query is the slice-08 query, UNCHANGED.
    let env = TestEnv::initialized();
    let _rachel_sub = seed_active_subscription_for(&env, RACHEL_DID, RACHEL_ACTIVE_SUB_SEED);
    let indexer = seed_network_index_from_specs(&env, sf_corpus_one_followed_one_unfollowed());
    let viewer = ViewerServer::start_with_indexer(&env, indexer);

    let response = viewer.get(&format!("/search?object={SF_OBJECT_REPRODUCIBLE_BUILDS}"));

    assert_eq!(
        response.status, 200,
        "SF-5: GET /search over the reachable index must be 200; body:\n{}",
        response.body
    );
    // THEN Rachel resolves to "Following" via the LOCAL active set (RED today). The
    // resolution is the operator's LOCAL business — the index returned Rachel's row
    // identically for any operator (per-user-neutral), and ONLY the LOCAL active
    // subscription turns it into "Following". Priya (absent from the LOCAL set) stays
    // `peer add` over the SAME index — the affordance tracks the LOCAL set, not the
    // network.
    assert_search_row_following(&response.body, RACHEL_DID);
    assert_search_row_offers_follow(&response.body, PRIYA_DID);
}

/// SF-6 (US-SF-001 / Theme C; FR-SF-3 / R-SF-5): a followed author is matched DESPITE
/// the signing-key fragment on the result DID. The active set stores BARE DIDs
/// (`PeerSubscriptionSummary.peer_did` is bare); the search result row's `author_did`
/// carries the `#org.openlore.application` signing fragment. The comparison strips the
/// fragment via the production `bare_did` SSOT on BOTH sides before set membership, so
/// `did:plc:rachel-test#org.openlore.application` matches the bare `did:plc:rachel-test`
/// in the active set → "Following" (never misclassified as NetworkUnfollowed).
///
/// Given Maria actively follows the bare did:plc:rachel-test, and the result row's
///   author DID is did:plc:rachel-test#org.openlore.application;
/// When she opens GET /search and that row appears;
/// Then the row shows the "Following" indicator (the fragment is stripped before the
///   match).
///
/// @us-sf-001 @driving_port @real-io @fragment-strip @local-resolution @r-sf-5 @edge
#[test]
fn a_followed_author_is_matched_despite_the_signing_key_fragment_on_the_result_did() {
    // GIVEN Maria follows the BARE did:plc:rachel-test (the `peer_subscriptions.peer_did`
    // is bare). The indexed corpus attributes Rachel's row to the app-identity
    // `did:plc:rachel-test#org.openlore.application` shape (the SAME fragmented shape the
    // viewer renders). The fragment-strip must reconcile the two for the match.
    let env = TestEnv::initialized();
    let _rachel_sub = seed_active_subscription_for(&env, RACHEL_DID, RACHEL_ACTIVE_SUB_SEED);
    let indexer = seed_network_index_from_specs(&env, sf_corpus_one_followed_one_unfollowed());
    let viewer = ViewerServer::start_with_indexer(&env, indexer);

    let response = viewer.get(&format!("/search?object={SF_OBJECT_REPRODUCIBLE_BUILDS}"));

    assert_eq!(
        response.status, 200,
        "SF-6: GET /search over the reachable index must be 200; body:\n{}",
        response.body
    );
    // THEN the row whose rendered author DID carries the signing fragment STILL matches
    // the bare active-set DID → "Following" (RED today: the resolution does not exist,
    // so the fragmented row falls to the hardcoded NetworkUnfollowed → `peer add`). The
    // app-identity (fragmented) shape is present AND it resolves to "Following".
    let rachel_app_identity = format!("{RACHEL_DID}#org.openlore.application");
    assert!(
        response.body.contains(&rachel_app_identity),
        "SF-6 (R-SF-5): the viewer renders Rachel's app-identity (fragmented) DID \
         {rachel_app_identity:?}; body:\n{}",
        response.body
    );
    // `assert_search_row_following` matches the BARE DID as a substring of the rendered
    // fragmented DID and asserts the "Following" indicator + no `peer add` — i.e. the
    // fragmented result DID was reconciled against the bare active-set DID (R-SF-5).
    assert_search_row_following(&response.body, RACHEL_DID);
}

// =============================================================================
// US-SF-001 — Theme D: ONE batch read, no N+1. (SF-7)
// =============================================================================

/// SF-7 (US-SF-001 / Theme D; C-4 / NFR-SF-3 / WD-SF-3 — the no-N+1 behavioral proxy):
/// the active-subscription set is read ONCE per render, invariant to the result count.
/// Behavioral proxy (the STRICT 1-read bound is a DELIVER adapter/property concern): a
/// LARGE multi-result search (8 distinct authors, exactly ONE of them followed)
/// resolves ALL rows correctly in ONE render — Rachel → "Following", the other 7 →
/// `peer add` — proving the resolution scales over the result set against a
/// single in-memory active set (no per-result subscription query).
///
/// Given a reachable index returns MANY result rows (8 authors), exactly one followed;
/// When Maria opens GET /search;
/// Then every row resolves correctly in one render (the followed author "Following",
///   the rest `peer add`), invariant to the number of result rows.
///
/// @us-sf-001 @driving_port @real-io @no-n-plus-1 @search-state-results @c-4
/// @nfr-sf-3 @edge
#[test]
fn a_large_multi_result_search_resolves_all_rows_in_one_render() {
    // GIVEN Maria follows exactly ONE author (Rachel) among MANY result authors (8
    // distinct), AND a reachable index returning all 8 for the object search. ONE LOCAL
    // active-set read must resolve all 8 in memory (no per-result query).
    let env = TestEnv::initialized();
    let _rachel_sub = seed_active_subscription_for(&env, RACHEL_DID, RACHEL_ACTIVE_SUB_SEED);
    let indexer = seed_network_index_from_specs(&env, sf_corpus_many_results_one_followed());
    let viewer = ViewerServer::start_with_indexer(&env, indexer);

    let response = viewer.get(&format!("/search?object={SF_OBJECT_REPRODUCIBLE_BUILDS}"));

    assert_eq!(
        response.status, 200,
        "SF-7: GET /search over the many-results seeded index must be 200; body:\n{}",
        response.body
    );
    // THEN the ONE followed author (Rachel) resolves to "Following" (RED today) and
    // representative OTHER authors keep `peer add` — proving the resolution scaled over
    // the multi-result set against a single active set (no N+1; the read count is
    // invariant to the result count — the strict 1-read bound is a DELIVER concern).
    assert_search_row_following(&response.body, RACHEL_DID);
    assert_search_row_offers_follow(&response.body, "did:plc:sf-author1-test");
    assert_search_row_offers_follow(&response.body, "did:plc:sf-author7-test");
}

// =============================================================================
// US-SF-001 — Theme E: GRACEFUL DEGRADATION. A failed active-set read degrades to
// the slice-08 status quo without crashing. (SF-8 — RED scaffold via the
// fault-injection seam; see the SEEDING-SEAM NOTE in support/mod.rs.)
// =============================================================================

/// SF-8 (US-SF-001 / Theme E; C-7 / FR-SF-7 / NFR-SF-6 / WD-SF-6 / ADR-053 D5): a
/// failed LOCAL active-set read during a search render degrades to the slice-08 status
/// quo — every result resolves to NetworkUnfollowed → keeps `openlore peer add <did>` —
/// and the search results STILL render with no crash, blank region, leaked error, or
/// 5xx. The relationship label is an enrichment; its failure must never break discovery.
///
/// SEEDING-SEAM NOTE (documented DISTILL choice; full detail in support/mod.rs on
/// `start_viewer_with_failing_active_set_read`): the slice-08/15 viewer harness holds
/// ONE long-lived DuckDB connection taken at STARTUP, so `make_store_unreadable` would
/// refuse STARTUP rather than exercise a MID-REQUEST read failure. There is NO readily-
/// available mid-request read-failure seam today. The OBSERVABLE degrade-TARGET (empty
/// active set → all `peer add`, byte-equal slice-08) is pinned by SF-3 (none-followed,
/// fully exercisable now); THIS scenario scaffolds the TRUE read-failure path for
/// DELIVER to materialize (the fault-injection seam is `todo!()` → RED for the RIGHT
/// reason: the degrade-on-read-failure path is MISSING).
///
/// Given the operator's LOCAL active-subscription read FAILS during a search render;
/// When Maria opens GET /search with a reachable index;
/// Then every result shows the `openlore peer add <did>` guidance (the slice-08 status
///   quo) and the results still render with no crash, blank, leaked error, or 5xx.
///
/// @us-sf-001 @driving_port @real-io @graceful-degrade @error @c-7 @nfr-sf-6
#[test]
fn a_failed_active_set_read_degrades_to_the_slice08_status_quo_without_crashing() {
    // GIVEN a reachable index where Rachel (whom Maria WOULD follow) + an unfollowed
    // author appear, BUT the LOCAL active-set read is forced to FAIL mid-request. With
    // the read failing, the resolution must degrade to an EMPTY set → every author
    // NetworkUnfollowed (the slice-08 status quo), regardless of any actual subscription.
    let env = TestEnv::initialized();
    let _rachel_sub = seed_active_subscription_for(&env, RACHEL_DID, RACHEL_ACTIVE_SUB_SEED);
    let indexer = seed_network_index_from_specs(&env, sf_corpus_one_followed_one_unfollowed());
    // The fault-injection seam (RED scaffold — `todo!()`): DELIVER materializes the
    // mid-request active-set-read failure. Until then this panics → RED
    // (MISSING_FUNCTIONALITY: the degrade-on-read-failure path does not exist).
    let viewer = start_viewer_with_failing_active_set_read(&env, indexer);

    let response = viewer.get(&format!("/search?object={SF_OBJECT_REPRODUCIBLE_BUILDS}"));

    // THEN the search results STILL render (200, not a crash/5xx).
    assert_eq!(
        response.status, 200,
        "SF-8 (C-7): a failed active-set read must degrade to a guided 200, NOT a \
         crash/5xx; body:\n{}",
        response.body
    );
    assert!(
        !response.body.trim().is_empty(),
        "SF-8 (C-7): the degraded render must NOT be a blank region; body:\n{}",
        response.body
    );
    // …and EVERY result falls back to the `openlore peer add <did>` guidance (the
    // slice-08 status quo — even Rachel, despite her real active subscription, because
    // the failed read yields an EMPTY set). No "Following" indicator appears (the
    // degrade is to all-NetworkUnfollowed).
    assert_search_row_offers_follow(&response.body, RACHEL_DID);
    assert_search_row_offers_follow(&response.body, PRIYA_DID);
    assert!(
        !response.body.contains(SF_FOLLOWING_INDICATOR),
        "SF-8 (C-7): a failed active-set read degrades to all-NetworkUnfollowed — NO \
         {SF_FOLLOWING_INDICATOR:?} indicator must appear; body:\n{}",
        response.body
    );
    // …and the degraded render leaks NO transport/internal error (the relationship
    // enrichment's failure is swallowed into the status quo, never surfaced).
    assert_search_html_leaks_no_transport_internals(&response.body);
}

// =============================================================================
// US-SF-002 — Theme F: htmx vs no-JS PARITY. (SF-9)
// =============================================================================

/// SF-9 (US-SF-002 / Theme F; C-8 / FR-SF-6 / NFR-SF-7 / WD-SF-7): the resolved
/// follow-state renders IDENTICALLY under the htmx `#search-results` fragment and the
/// no-JS full page. The resolution happens in the shell BEFORE the render; both shapes
/// consume the SAME SearchState, so the "Following" indicator (Rachel) + the `peer add`
/// affordance (Priya) appear in BOTH shapes — parity by construction.
///
/// Given Maria follows did:plc:rachel-test and the index holds claims by Rachel + Priya;
/// When she requests GET /search WITH HX-Request and again WITHOUT it;
/// Then the htmx fragment carries Rachel's "Following" indicator + no add command, and
///   the no-JS full page carries the SAME follow-state, rendered identically.
///
/// @us-sf-002 @driving_port @real-io @parity @c-8 @nfr-sf-7 @happy
#[test]
fn the_follow_state_renders_identically_under_htmx_and_no_js() {
    // GIVEN the mix render (Rachel followed; Priya unfollowed).
    let env = TestEnv::initialized();
    let _rachel_sub = seed_active_subscription_for(&env, RACHEL_DID, RACHEL_ACTIVE_SUB_SEED);
    let indexer = seed_network_index_from_specs(&env, sf_corpus_one_followed_one_unfollowed());
    let viewer = ViewerServer::start_with_indexer(&env, indexer);

    // WHEN both shapes of the SAME query are fetched.
    let path = format!("/search?object={SF_OBJECT_REPRODUCIBLE_BUILDS}");
    let fragment = viewer.get_htmx(&path);
    let full_page = viewer.get(&path);

    assert!(
        fragment.is_fragment(),
        "SF-9: the htmx shape must return ONLY the #search-results fragment; body:\n{}",
        fragment.body
    );
    assert!(
        full_page.is_full_page(),
        "SF-9: the no-JS shape must return the COMPLETE full page; body:\n{}",
        full_page.body
    );
    // THEN BOTH shapes carry the SAME resolved follow-state: Rachel "Following" + no
    // add (RED today), Priya `peer add` (parity by construction — both shapes embed the
    // SAME render_search_results_fragment over the SAME resolved SearchState).
    assert_search_row_following(&fragment.body, RACHEL_DID);
    assert_search_row_offers_follow(&fragment.body, PRIYA_DID);
    assert_search_row_following(&full_page.body, RACHEL_DID);
    assert_search_row_offers_follow(&full_page.body, PRIYA_DID);
}

// =============================================================================
// US-SF-002 — Theme G: ATTRIBUTION + RANKING UNCHANGED vs slice-08 (anti-merging).
// (SF-10)
// =============================================================================

/// SF-10 (US-SF-002 / Theme G; C-5 / NFR-SF-5 / J-003a / WD-SF-8): following + unfollowed
/// authors render side by side with attribution + order preserved — relationship
/// resolution does NOT merge or re-rank. Rachel's row shows "Following" with no add;
/// Priya's row shows `openlore peer add did:plc:priya-test`; the two rows are STILL
/// attributed to their own authors with no merged or re-ranked output; and each row's
/// `[verified]` marker + verbatim confidence are unchanged from slice-08.
///
/// Given Maria follows did:plc:rachel-test but not did:plc:priya-test, and the index
///   holds verified claims by both;
/// When she opens GET /search and both claims appear;
/// Then Rachel's row shows "Following" + no add, Priya's row shows the `peer add`
///   command, the two rows stay attributed to their own authors (no merge/re-rank), and
///   each row's [verified] marker + verbatim confidence are unchanged.
///
/// @us-sf-002 @driving_port @real-io @anti-merging @attribution @c-5 @nfr-sf-5 @edge
#[test]
fn following_and_unfollowed_authors_render_side_by_side_attribution_and_order_preserved() {
    // GIVEN the mix render (Rachel followed @ 0.88; Priya unfollowed @ 0.82).
    let env = TestEnv::initialized();
    let _rachel_sub = seed_active_subscription_for(&env, RACHEL_DID, RACHEL_ACTIVE_SUB_SEED);
    let indexer = seed_network_index_from_specs(&env, sf_corpus_one_followed_one_unfollowed());
    let viewer = ViewerServer::start_with_indexer(&env, indexer);

    let response = viewer.get(&format!("/search?object={SF_OBJECT_REPRODUCIBLE_BUILDS}"));

    assert_eq!(
        response.status, 200,
        "SF-10: GET /search over the mix seeded index must be 200; body:\n{}",
        response.body
    );
    // THEN the two authors are STILL attributed to their own DIDs (no merge) and each
    // carries `[verified]` (the relationship is a per-row enrichment; attribution +
    // verified marker UNCHANGED — C-5 / J-003a).
    assert_search_html_every_row_verified_and_attributed(&response.body, &[RACHEL_DID, PRIYA_DID]);
    // …and there is NO merged "network consensus" row (anti-merging held — the
    // resolution touches neither grouping nor order; compose_results is UNCHANGED).
    assert_search_html_has_no_merged_consensus_row(&response.body);
    // …Rachel "Following" + no add (RED today); Priya keeps the `peer add` command —
    // the DIFFERENTIATED per-row affordance does not collapse the two into one row.
    assert_search_row_following(&response.body, RACHEL_DID);
    assert_search_row_offers_follow(&response.body, PRIYA_DID);
    // …and each row's verbatim confidence is unchanged from slice-08 (Rachel 0.88,
    // Priya 0.82 — rendered byte-for-byte, never rounded). The relationship enrichment
    // perturbs no confidence value.
    for confidence in ["0.88", "0.82"] {
        assert!(
            response.body.contains(confidence),
            "SF-10 (C-5): each row's verbatim confidence must be unchanged from slice-08 \
             (expected {confidence:?}, rendered byte-for-byte); body:\n{}",
            response.body
        );
    }
    assert!(
        !response.body.contains("90%") && !response.body.contains("80%"),
        "SF-10 (C-5): confidence must NOT be rounded (the relationship enrichment \
         perturbs no confidence); body:\n{}",
        response.body
    );
}
