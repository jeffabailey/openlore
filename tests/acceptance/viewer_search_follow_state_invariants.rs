//! Slice-16 acceptance — `/search` FOLLOW-STATE GOLD / guardrail invariants (the
//! cross-cutting C-1/-3/-4/-5 guardrails that must hold over the WHOLE `/search`
//! follow-state surface, beyond any single Theme-A story).
//!
//! These are the load-bearing, release-relevant guardrail gold tests for the slice-16
//! follow-state DELTA — the BEHAVIORAL layer of the three-layer enforcement (subtype +
//! xtask `check-arch` are the other two, owned by DELIVER; ADR-053 §Enforcement). They
//! drive the REAL `openlore ui` verb via the `ViewerServer` subprocess + in-test HTTP
//! (with/without `HX-Request`) over a REAL DuckDB seeded with active subscriptions (via
//! the REAL slice-03 `peer add` verb), with the network index as the ONLY mocked
//! boundary (a REAL slice-05 `openlore-indexer serve` over a seeded corpus), and assert
//! the hard slice-16 invariants on the OBSERVABLE rendered surface:
//!
//! - `no_search_follow_state_render_adds_an_executable_control` (SF-INV-NoControl,
//!   C-1 / WD-SF-1, CARDINAL): over a render carrying BOTH a "Following" indicator AND
//!   a `peer add` affordance, in BOTH shapes (full page + htmx fragment), NEITHER
//!   affordance is an executable control — no follow/unfollow/subscribe control, no
//!   mutating `hx-*` swap. The NEW "Following" arm adds no write surface; the viewer
//!   holds no key. (The slice-08 sign/publish-control absence is the slice-08 gold's
//!   concern; this slice's NEW surface is the "Following" indicator + the per-row
//!   resolution — its render-only-ness is THIS gold's load-bearing addition.)
//! - `every_search_follow_state_render_leaves_the_store_read_only`
//!   (SF-INV-ReadOnly, C-1 / I-VIEW): resolving relationships against the LOCAL active
//!   set — a READ — leaves the `peer_subscriptions` + `claims` + `peer_claims` row
//!   counts UNCHANGED across every shape (the resolution persists nothing; results +
//!   the in-memory active set are computed per render). Asserted via the universe-bound
//!   `assert_store_read_only` (Mandate 8: universe = the port-exposed counts, each
//!   `unchanged`).
//! - `the_relationship_resolution_adds_no_network_seam_index_stays_per_user_neutral`
//!   (SF-INV-LocalPerUserNeutral, C-3 / WD-SF-4): the relationship is resolved against
//!   the LOCAL active set; the network index query is UNCHANGED and per-user-neutral
//!   (the SAME query over the SAME index produces the SAME result ROWS regardless of
//!   the LOCAL active set — only the per-row AFFORDANCE flips with the LOCAL set). The
//!   index never learns who the operator follows.
//! - `the_follow_state_resolution_does_not_merge_or_rerank`
//!   (SF-INV-AttributionUnchanged, C-5 / J-003a / WD-SF-8): the resolution sets the
//!   per-row relationship ONLY — the grouping + order + `[verified]` marker + verbatim
//!   confidence are UNCHANGED whether or not an active subscription is present. The
//!   SAME index + SAME query renders the SAME attributed rows with-and-without a
//!   followed author present; only the per-row affordance differs (no merged row).
//!
//! Driving discipline (Mandate 1): every assertion enters through the REAL `openlore
//! ui` subprocess + HTTP — never internal `viewer-domain` functions or the adapter
//! resolution fn. The local DuckDB is REAL (active subscriptions seeded via the REAL
//! `peer add` verb); the network index is the REUSED slice-05 `openlore-indexer serve`
//! (the ONLY mocked boundary).
//!
//! Layer placement (Mandate 9/11): layer-3/layer-5 subprocess + real-I/O, EXAMPLE-only.
//! These guardrails are example-based, never PBT-generated at this layer (the
//! `@property` tag marks them as universal invariants for the reader + the DELIVER
//! crafter; the generative exploration of the pure render/resolution core is a
//! layer-1/2 concern, out of this file's scope). Tier B (state-machine PBT) is NOT
//! warranted (Mandate 10 — binary per-row resolution, not a chained state machine).
//!
//! Build-before-run note: as with `viewer_search_follow_state.rs`, the run MUST `cargo
//! build` BOTH the `openlore` (viewer) and `openlore-indexer` (seeded serve) bins
//! before running these ATs.
//!
//! Mandate 7 RED scaffolds: each body classifies RED for the RIGHT reason. The
//! NoControl + ReadOnly + PerUserNeutral golds run today (the render exists) but the
//! follow-state-specific assertions (e.g. "Following" present where a subscription is
//! seeded) FAIL because the SubscribedPeer resolution + render arm are MISSING — RED
//! (MISSING_FUNCTIONALITY), NOT BROKEN. They stay RED until DELIVER.
//!
//! Inherited gold (NOT re-authored here — already covered by
//! `viewer_network_search_invariants.rs` over the WHOLE `/search` surface, and slice-16
//! adds NO new route/CDN/store-write/sign-control): the offline-chrome (no-CDN) gold,
//! the verified-by-construction gold, and the no-sign/publish-control gold inherit
//! VERBATIM. slice-16 adds ONLY the follow-state-specific guardrails above.
//
// SCAFFOLD: true

mod support;

#[allow(unused_imports)]
use support::*;

use openlore_test_support::{PRIYA_DID, RACHEL_DID};

const RACHEL_ACTIVE_SUB_SEED: [u8; 32] = [7u8; 32];

// =============================================================================
// C-1 / WD-SF-1 (CARDINAL) — no /search follow-state render adds an executable
// control (SF-INV-NoControl). The NEW "Following" arm adds no write surface.
// =============================================================================

/// SF-INV-NoControl / GOLD `no_search_follow_state_render_adds_an_executable_control`
/// (C-1 / WD-SF-1, CARDINAL): over a render carrying BOTH the NEW "Following" indicator
/// (a followed author) AND the slice-08 `peer add` affordance (an unfollowed author),
/// in BOTH shapes (full page + htmx fragment), NEITHER affordance is an executable
/// control — no `name="follow"`/`name="unfollow"`/`name="subscribe"` input, no
/// `>Follow<`/`>Following<`/`>Subscribe<` control element, no mutating `hx-post`/
/// `hx-delete`/`hx-put`. The viewer holds no key and exposes no follow/unfollow route.
/// The slice-16 companion to the slice-08 `no_search_response_adds_a_write_or_sign_
/// control` gold — extended so the NEW "Following" arm is proven render-only TEXT too.
///
/// Given the viewer serves a /search render carrying both a followed and an unfollowed
///   author over a reachable index;
/// When every /search response shape is requested;
/// Then no response renders a follow/unfollow/subscribe executable control (both
///   affordances are render-only TEXT).
///
/// @us-sf-002 @property @driving_port @real-io @read-only @c-1 @gold
#[test]
fn no_search_follow_state_render_adds_an_executable_control() {
    // GIVEN Maria follows Rachel (a REAL active subscription) + a reachable index where
    // Rachel (followed → "Following") and Priya (unfollowed → `peer add`) both appear,
    // so BOTH follow-state affordances are present on the SAME surface.
    let env = TestEnv::initialized();
    let _rachel_sub = seed_active_subscription_for(&env, RACHEL_DID, RACHEL_ACTIVE_SUB_SEED);

    // Collect EVERY /search response shape (full page + htmx fragment) over the
    // reachable seeded index, inside a scope so the viewer's exclusive DuckDB lock is
    // released on drop (mirrors the slice-08 gold collection discipline).
    let mut responses = Vec::new();
    {
        let indexer = seed_network_index_from_specs(&env, sf_corpus_one_followed_one_unfollowed());
        let viewer = ViewerServer::start_with_indexer(&env, indexer);

        let path = format!("/search?object={SF_OBJECT_REPRODUCIBLE_BUILDS}");
        responses.push((format!("GET {path} (full page)"), viewer.get(&path)));
        responses.push((
            format!("GET {path} (htmx fragment)"),
            viewer.get_htmx(&path),
        ));
        // `viewer` (and `indexer`) drop here.
    }

    for (label, response) in &responses {
        assert_eq!(
            response.status, 200,
            "SF-INV-NoControl: /search route {label:?} over a reachable seeded index must \
             render successfully (200) so the no-control scan is over REAL follow-state \
             content; got {}",
            response.status
        );
        // THEN NEITHER affordance is an executable control — both the "Following"
        // indicator AND the `peer add` guidance are render-only TEXT (C-1, CARDINAL).
        assert_search_follow_state_is_render_only(&response.body);
    }
}

// =============================================================================
// C-1 / I-VIEW — resolving relationships against the LOCAL active set is a READ:
// every follow-state render leaves the store read-only (SF-INV-ReadOnly).
// =============================================================================

/// SF-INV-ReadOnly / GOLD `every_search_follow_state_render_leaves_the_store_read_only`
/// (C-1 / I-VIEW / Mandate 8): resolving each result author's relationship against the
/// LOCAL active-subscription set — a READ — leaves the `peer_subscriptions` + `claims`
/// + `peer_claims` row counts UNCHANGED across every /search shape. The resolution
/// reads the active set and computes the per-row relationship in memory; it persists
/// NOTHING. Asserted via the universe-bound state-delta (Mandate 8: universe = the
/// port-exposed counts, each `unchanged`). The slice-16 companion to the slice-08
/// `every_search_route_leaves_the_store_read_only` gold — extended so the NEW LOCAL
/// active-set read is proven non-mutating.
///
/// Given a store seeded with an active subscription + a reachable index;
/// When the /search follow-state render is exercised in both shapes;
/// Then the `peer_subscriptions`, `claims`, and `peer_claims` row counts are UNCHANGED.
///
/// @us-sf-001 @property @driving_port @real-io @read-only @c-1 @gold
#[test]
fn every_search_follow_state_render_leaves_the_store_read_only() {
    // GIVEN a REAL store seeded with an active subscription (Rachel) so the read-only
    // universe is NON-TRIVIAL (a populated `peer_subscriptions` table the resolution
    // reads), PLUS a reachable index where Rachel + Priya appear.
    let env = TestEnv::initialized();
    let _rachel_sub = seed_active_subscription_for(&env, RACHEL_DID, RACHEL_ACTIVE_SUB_SEED);

    // Capture the read-only universe (the port-exposed row counts) BEFORE any /search
    // follow-state render runs.
    let before = capture_store_row_count_universe(&env);

    // Exercise the /search follow-state render in BOTH shapes inside a scope so the
    // viewer's exclusive DuckDB lock is RELEASED (on drop) BEFORE the `after` snapshot —
    // the read-only proof is about what the viewer LEFT BEHIND (mirrors slice-08
    // N-INV-ReadOnly).
    {
        let indexer = seed_network_index_from_specs(&env, sf_corpus_one_followed_one_unfollowed());
        let viewer = ViewerServer::start_with_indexer(&env, indexer);

        let path = format!("/search?object={SF_OBJECT_REPRODUCIBLE_BUILDS}");
        let full_page = viewer.get(&path);
        assert_eq!(
            full_page.status, 200,
            "SF-INV-ReadOnly: GET /search (full page) must be 200; body:\n{}",
            full_page.body
        );
        let fragment = viewer.get_htmx(&path);
        assert_eq!(
            fragment.status, 200,
            "SF-INV-ReadOnly: GET /search (htmx fragment) must be 200; body:\n{}",
            fragment.body
        );
        // `viewer` (and `indexer`) drop here — the lock is released before the `after`
        // snapshot re-opens the store.
    }

    // Capture the read-only universe AFTER the follow-state render ran.
    let after = capture_store_row_count_universe(&env);

    // The persisted-store row counts are UNCHANGED — every universe slot `unchanged`
    // (any change is an UNSHIPPABLE write-surface breach; C-1 / I-VIEW). The LOCAL
    // active-set READ + the in-memory resolution persisted nothing.
    assert_store_read_only(&before, &after);
}

// =============================================================================
// C-3 / WD-SF-4 — the relationship resolution adds NO network seam; the index stays
// per-user-neutral (SF-INV-LocalPerUserNeutral).
// =============================================================================

/// SF-INV-LocalPerUserNeutral / GOLD
/// `the_relationship_resolution_adds_no_network_seam_index_stays_per_user_neutral`
/// (C-3 / WD-SF-4): the relationship is resolved against the LOCAL active set, not the
/// network. Behavioral proxy: the SAME reachable index + the SAME query produces the
/// SAME result ROWS (the per-user-neutral corpus is identical for any operator); only
/// the per-row AFFORDANCE flips with the LOCAL active set. Two viewers over the SAME
/// index — one whose operator follows Rachel, one whose operator follows nobody —
/// render the SAME attributed rows; the followed operator sees "Following" on Rachel's
/// row where the un-following operator sees `peer add`. The index never learns who
/// follows whom (it returned the SAME rows both times).
///
/// Given the SAME reachable index corpus and the SAME query;
/// When the search is rendered for an operator who follows Rachel and one who follows
///   nobody;
/// Then BOTH renders carry the SAME attributed result rows (the index is per-user-
///   neutral), and only the per-row follow-state affordance differs (resolved LOCALLY).
///
/// @us-sf-001 @property @driving_port @real-io @local-resolution @offline @c-3 @gold
#[test]
fn the_relationship_resolution_adds_no_network_seam_index_stays_per_user_neutral() {
    // GIVEN — render-1: an operator who FOLLOWS Rachel, over the reachable index.
    let env_following = TestEnv::initialized();
    let _rachel_sub =
        seed_active_subscription_for(&env_following, RACHEL_DID, RACHEL_ACTIVE_SUB_SEED);
    let following_body = {
        let indexer =
            seed_network_index_from_specs(&env_following, sf_corpus_one_followed_one_unfollowed());
        let viewer = ViewerServer::start_with_indexer(&env_following, indexer);
        let r = viewer.get(&format!("/search?object={SF_OBJECT_REPRODUCIBLE_BUILDS}"));
        assert_eq!(
            r.status, 200,
            "SF-INV-PerUserNeutral: following-operator render must be 200; body:\n{}",
            r.body
        );
        r.body
    };

    // GIVEN — render-2: an operator who follows NOBODY, over the SAME index corpus
    // (a SEPARATE clean store — no `peer add`). The index corpus is byte-identical.
    let env_none = TestEnv::initialized();
    let none_body = {
        let indexer =
            seed_network_index_from_specs(&env_none, sf_corpus_one_followed_one_unfollowed());
        let viewer = ViewerServer::start_with_indexer(&env_none, indexer);
        let r = viewer.get(&format!("/search?object={SF_OBJECT_REPRODUCIBLE_BUILDS}"));
        assert_eq!(
            r.status, 200,
            "SF-INV-PerUserNeutral: none-following-operator render must be 200; body:\n{}",
            r.body
        );
        r.body
    };

    // THEN BOTH renders carry the SAME attributed result ROWS — the index is
    // per-user-neutral (it returned Rachel + Priya identically for BOTH operators; it
    // never learned who follows whom). Both rows are attributed + verified in BOTH.
    for body in [&following_body, &none_body] {
        assert_search_html_every_row_verified_and_attributed(body, &[RACHEL_DID, PRIYA_DID]);
    }
    // …and ONLY the per-row AFFORDANCE differs, resolved LOCALLY (RED today): the
    // following operator sees Rachel "Following"; the none operator sees Rachel
    // `peer add`. Priya stays `peer add` for both.
    assert_search_row_following(&following_body, RACHEL_DID);
    assert_search_row_offers_follow(&none_body, RACHEL_DID);
    assert_search_row_offers_follow(&following_body, PRIYA_DID);
    assert_search_row_offers_follow(&none_body, PRIYA_DID);
}

// =============================================================================
// C-5 / J-003a / WD-SF-8 — the resolution sets the per-row relationship ONLY; it does
// not merge or re-rank (SF-INV-AttributionUnchanged).
// =============================================================================

/// SF-INV-AttributionUnchanged / GOLD `the_follow_state_resolution_does_not_merge_or_
/// rerank` (C-5 / J-003a / WD-SF-8): relationship resolution sets the per-row
/// relationship ONLY — grouping + order + the `[verified]` marker + verbatim confidence
/// are UNCHANGED whether or not an active subscription is present. The SAME index +
/// SAME query renders the SAME attributed rows with-and-without a followed author
/// present; only the per-row affordance differs (no merged / re-ranked output). The
/// slice-16 companion to the slice-08 anti-merging golds — proving the NEW resolution
/// is a per-row enrichment, not a second grouping path.
///
/// Given the SAME reachable index corpus and the SAME query;
/// When the search is rendered with an active subscription present and again without;
/// Then both renders carry the SAME attributed rows with the SAME verified markers +
///   verbatim confidence + no merged consensus row (only the per-row affordance flips).
///
/// @us-sf-002 @property @driving_port @real-io @anti-merging @attribution @c-5 @gold
#[test]
fn the_follow_state_resolution_does_not_merge_or_rerank() {
    // GIVEN render-WITH: an operator who follows Rachel, over the reachable index.
    let env_with = TestEnv::initialized();
    let _rachel_sub = seed_active_subscription_for(&env_with, RACHEL_DID, RACHEL_ACTIVE_SUB_SEED);
    let with_body = {
        let indexer =
            seed_network_index_from_specs(&env_with, sf_corpus_one_followed_one_unfollowed());
        let viewer = ViewerServer::start_with_indexer(&env_with, indexer);
        let r = viewer.get(&format!("/search?object={SF_OBJECT_REPRODUCIBLE_BUILDS}"));
        assert_eq!(
            r.status, 200,
            "SF-INV-AttributionUnchanged: with-subscription render must be 200; body:\n{}",
            r.body
        );
        r.body
    };

    // GIVEN render-WITHOUT: an operator who follows nobody, over the SAME corpus.
    let env_without = TestEnv::initialized();
    let without_body = {
        let indexer =
            seed_network_index_from_specs(&env_without, sf_corpus_one_followed_one_unfollowed());
        let viewer = ViewerServer::start_with_indexer(&env_without, indexer);
        let r = viewer.get(&format!("/search?object={SF_OBJECT_REPRODUCIBLE_BUILDS}"));
        assert_eq!(
            r.status, 200,
            "SF-INV-AttributionUnchanged: without-subscription render must be 200; body:\n{}",
            r.body
        );
        r.body
    };

    // THEN both renders carry the SAME attributed rows (both authors), the SAME
    // verified markers, and NO merged consensus row — grouping/order/attribution are
    // UNCHANGED by the resolution (C-5 / J-003a).
    for body in [&with_body, &without_body] {
        assert_search_html_every_row_verified_and_attributed(body, &[RACHEL_DID, PRIYA_DID]);
        assert_search_html_has_no_merged_consensus_row(body);
        // Verbatim confidence is unchanged in BOTH (Rachel 0.88, Priya 0.82).
        for confidence in ["0.88", "0.82"] {
            assert!(
                body.contains(confidence),
                "SF-INV-AttributionUnchanged (C-5): verbatim confidence {confidence:?} must \
                 be unchanged whether or not a subscription is present; body:\n{body}"
            );
        }
    }
    // …and ONLY the per-row affordance differs (RED today): WITH the subscription,
    // Rachel shows "Following"; WITHOUT, Rachel shows `peer add` — but the ROW set,
    // order, attribution, and confidence are identical. The resolution is a per-row
    // enrichment, never a merge/re-rank.
    assert_search_row_following(&with_body, RACHEL_DID);
    assert_search_row_offers_follow(&without_body, RACHEL_DID);
}
