//! Slice-20 acceptance — `/search` FULL FOUR-ARM follow-state GOLD / guardrail
//! invariants (the cross-cutting C-1/-3/-4/-5/-7/-8/-9 guardrails that must hold
//! over the WHOLE `/search` four-arm follow-state surface, beyond any single
//! Theme-A story). Mirror of the slice-16 GOLD set, extended for the two NEW arms.
//!
//! These are the load-bearing, release-relevant guardrail gold tests for the
//! slice-20 four-arm completion DELTA — the BEHAVIORAL layer of the three-layer
//! enforcement (subtype + xtask `check-arch` are the other two, owned by DELIVER;
//! ADR-057 §Enforcement). They drive the REAL `openlore ui` verb via the
//! `ViewerServer` subprocess + in-test HTTP (with/without `HX-Request`) over a REAL
//! DuckDB seeded via the REAL `claim add` / `peer add` / `peer pull` / `peer remove`
//! verbs, with the network index as the ONLY mocked boundary (a REAL slice-05
//! `openlore-indexer serve` over a seeded corpus), and assert the hard slice-20
//! invariants on the OBSERVABLE rendered surface:
//!
//! - `no_four_arm_follow_state_render_adds_an_executable_control`
//!   (FF-INV-NoControl, C-1 / WD-FS-1, CARDINAL): over a render carrying all four
//!   follow-states, in BOTH shapes, NEITHER NEW indicator (self, residue) is an
//!   executable control. The two new arms add no write surface; the viewer holds
//!   no key.
//! - `every_four_arm_follow_state_render_leaves_the_store_read_only`
//!   (FF-INV-ReadOnly, C-1 / I-VIEW / Mandate 8): resolving relationships against
//!   the THREE LOCAL sets (own / active / cached) — all READS — leaves the `claims`
//!   + `peer_claims` row counts UNCHANGED across every shape. Asserted via the
//!   universe-bound `assert_store_read_only` (Mandate 8: universe = the port-exposed
//!   counts, each `unchanged`).
//! - `the_four_arm_resolution_adds_no_network_seam_index_stays_per_user_neutral`
//!   (FF-INV-LocalPerUserNeutral, C-3 / WD-FS-3): the four-arm relationship is
//!   resolved against the THREE LOCAL sets; the network index query is UNCHANGED and
//!   per-user-neutral (the SAME query over the SAME index produces the SAME result
//!   ROWS regardless of the LOCAL sets — only the per-row AFFORDANCE flips). The
//!   index never learns who the operator is or whom she removed.
//! - `the_four_arm_resolution_does_not_merge_or_rerank`
//!   (FF-INV-AttributionUnchanged, C-5 / J-003a / WD-FS — anti-merging): the
//!   resolution sets the per-row relationship ONLY — grouping + order + `[verified]`
//!   marker + verbatim confidence are UNCHANGED whether or not the new arms apply.
//! - `the_two_new_indicators_are_neutral_never_pejorative`
//!   (FF-INV-NeutralFraming, C-9 / WD-FS-8): over a render carrying both new
//!   indicators, NO blocklisted judgement term appears anywhere in the follow-state
//!   surface (the operator reads facts, never a verdict).
//! - `a_removed_but_cached_author_is_not_shown_as_a_fresh_add_candidate`
//!   (FF-INV-OwnVsCacheDistinct, C-2 / WD-FS-2 — the own-vs-cache distinctness): a
//!   `UnsubscribedCache` row is NOT a `NetworkUnfollowed` row — the residue author
//!   is never re-offered the `peer add` affordance (he is residue, not a fresh
//!   find). The defining accuracy guarantee of the cache arm.
//!
//! Driving discipline (Mandate 1): every assertion enters through the REAL `openlore
//! ui` subprocess + HTTP — never internal `viewer-domain` functions or the adapter
//! resolution fn. The local DuckDB is REAL (own + cached + active state seeded via
//! the REAL CLI verbs); the network index is the REUSED slice-05 `openlore-indexer
//! serve` (the ONLY mocked boundary).
//!
//! Layer placement (Mandate 9/11): layer-3/layer-5 subprocess + real-I/O, EXAMPLE-only.
//! These guardrails are example-based, never PBT-generated at this layer (the
//! `@property` tag marks them as universal invariants for the reader + the DELIVER
//! crafter; the generative exploration of the pure render/resolution core is a
//! layer-1/2 concern, out of this file's scope). Tier B (state-machine PBT) is NOT
//! warranted (Mandate 10 — per-row precedence over three set memberships, not a
//! chained state machine).
//!
//! Build-before-run note: as with `viewer_search_full_follow_state.rs`, the run MUST
//! `cargo build` BOTH the `openlore` (viewer) and `openlore-indexer` (seeded serve)
//! bins before running these ATs.
//!
//! Mandate 7 RED scaffolds: each body classifies RED for the RIGHT reason. The
//! NoControl + ReadOnly + PerUserNeutral + NeutralFraming golds run today (the
//! render exists) but the four-arm-specific assertions (the self + residue
//! indicators present where seeded; the residue author NOT a fresh add candidate)
//! FAIL because the You + UnsubscribedCache resolution + render arms are MISSING —
//! RED (MISSING_FUNCTIONALITY), NOT BROKEN. They stay RED until DELIVER.
//!
//! Inherited gold (NOT re-authored here — already covered by
//! `viewer_network_search_invariants.rs` + `viewer_search_follow_state_invariants.rs`
//! over the WHOLE `/search` surface): the offline-chrome (no-CDN) gold, the
//! verified-by-construction gold, the no-sign/publish-control gold, and the slice-16
//! binary follow-state golds inherit VERBATIM. slice-20 adds ONLY the four-arm-
//! specific guardrails above.
//
// SCAFFOLD: true

mod support;

#[allow(unused_imports)]
use support::*;

use openlore_test_support::{PRIYA_DID, RACHEL_DID};

const RACHEL_ACTIVE_SUB_SEED: [u8; 32] = [7u8; 32];
const TOBIAS_CACHED_SEED: [u8; 32] = [9u8; 32];

// =============================================================================
// C-1 / WD-FS-1 (CARDINAL) — no four-arm follow-state render adds an executable
// control (FF-INV-NoControl). The two NEW arms add no write surface.
// =============================================================================

/// FF-INV-NoControl / GOLD `no_four_arm_follow_state_render_adds_an_executable_control`
/// (C-1 / WD-FS-1, CARDINAL): over a render carrying ALL FOUR follow-states — the
/// NEW self indicator (own claim) + the NEW residue indicator (soft-removed peer) +
/// the slice-16 "Following" (followed) + `peer add` (new author) — in BOTH shapes,
/// NEITHER NEW indicator is an executable control. The viewer holds no key and
/// exposes no follow/unfollow route. The slice-20 companion to the slice-16
/// `no_search_follow_state_render_adds_an_executable_control` gold — extended so the
/// two NEW arms are proven render-only TEXT too.
///
/// Given the viewer serves a /search render carrying all four follow-states;
/// When every /search response shape is requested;
/// Then no response renders a follow/unfollow/subscribe executable control (all four
///   affordances are render-only TEXT).
///
/// @us-fs-002 @property @driving_port @real-io @read-only @c-1 @gold
#[test]
fn no_four_arm_follow_state_render_adds_an_executable_control() {
    let env = TestEnv::initialized();
    seed_own_claim_for_search(&env);
    seed_cached_unsubscribed_peer_for(&env, TRAVERSAL_AUTHOR_TOBIAS, TOBIAS_CACHED_SEED);
    let _rachel_sub = seed_active_subscription_for(&env, RACHEL_DID, RACHEL_ACTIVE_SUB_SEED);

    let mut responses = Vec::new();
    {
        let indexer = seed_network_index_from_specs(&env, sf_corpus_all_four_arms());
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
            "FF-INV-NoControl: /search route {label:?} over a four-arm seeded index must \
             render successfully (200) so the no-control scan is over REAL follow-state \
             content; got {}",
            response.status
        );
        // THEN no affordance — including the two NEW indicators — is an executable
        // control (C-1, CARDINAL).
        assert_search_follow_state_is_render_only(&response.body);
    }
}

// =============================================================================
// C-1 / I-VIEW — resolving relationships against the THREE LOCAL sets is a READ:
// every four-arm render leaves the store read-only (FF-INV-ReadOnly).
// =============================================================================

/// FF-INV-ReadOnly / GOLD `every_four_arm_follow_state_render_leaves_the_store_read_only`
/// (C-1 / I-VIEW / Mandate 8): resolving each result author's relationship against
/// the THREE LOCAL sets (own via `distinct_own_author_dids`, active via
/// `list_active_peer_subscriptions`, cached via `distinct_cached_peer_author_dids`)
/// — all READS — leaves the `claims` + `peer_claims` row counts UNCHANGED across
/// every /search shape. The two NEW reads persist NOTHING. Asserted via the
/// universe-bound state-delta (Mandate 8: universe = the port-exposed counts, each
/// `unchanged`). The slice-20 companion to the slice-16 read-only gold — extended so
/// the two NEW LOCAL presence reads are proven non-mutating.
///
/// Given a store seeded with an own claim + a soft-removed cached peer + an active
///   subscription, and a reachable index;
/// When the four-arm /search render is exercised in both shapes;
/// Then the `claims` and `peer_claims` row counts are UNCHANGED.
///
/// @us-fs-001 @property @driving_port @real-io @read-only @c-1 @gold
#[test]
fn every_four_arm_follow_state_render_leaves_the_store_read_only() {
    // GIVEN a REAL store seeded with all three LOCAL sets non-trivially (an own
    // claim, an active subscription, a soft-removed cached peer) so the read-only
    // universe is NON-TRIVIAL (the two new reads have real rows to read).
    let env = TestEnv::initialized();
    seed_own_claim_for_search(&env);
    seed_cached_unsubscribed_peer_for(&env, TRAVERSAL_AUTHOR_TOBIAS, TOBIAS_CACHED_SEED);
    let _rachel_sub = seed_active_subscription_for(&env, RACHEL_DID, RACHEL_ACTIVE_SUB_SEED);

    // Capture the read-only universe (the port-exposed row counts) BEFORE any /search
    // four-arm render runs.
    let before = capture_store_row_count_universe(&env);

    // Exercise the four-arm render in BOTH shapes inside a scope so the viewer's
    // exclusive DuckDB lock is RELEASED (on drop) BEFORE the `after` snapshot.
    {
        let indexer = seed_network_index_from_specs(&env, sf_corpus_all_four_arms());
        let viewer = ViewerServer::start_with_indexer(&env, indexer);

        let path = format!("/search?object={SF_OBJECT_REPRODUCIBLE_BUILDS}");
        let full_page = viewer.get(&path);
        assert_eq!(
            full_page.status, 200,
            "FF-INV-ReadOnly: GET /search (full page) must be 200; body:\n{}",
            full_page.body
        );
        let fragment = viewer.get_htmx(&path);
        assert_eq!(
            fragment.status, 200,
            "FF-INV-ReadOnly: GET /search (htmx fragment) must be 200; body:\n{}",
            fragment.body
        );
        // `viewer` (and `indexer`) drop here — the lock is released.
    }

    // Capture the read-only universe AFTER the four-arm render ran.
    let after = capture_store_row_count_universe(&env);

    // The persisted-store row counts are UNCHANGED — every universe slot `unchanged`
    // (C-1 / I-VIEW). The THREE LOCAL reads + the in-memory resolution persisted
    // nothing. (PASSES today — the resolution is read-only by construction; this gold
    // pins that the two NEW reads add no write.)
    assert_store_read_only(&before, &after);
}

// =============================================================================
// C-3 / WD-FS-3 — the four-arm resolution adds NO network seam; the index stays
// per-user-neutral (FF-INV-LocalPerUserNeutral).
// =============================================================================

/// FF-INV-LocalPerUserNeutral / GOLD
/// `the_four_arm_resolution_adds_no_network_seam_index_stays_per_user_neutral`
/// (C-3 / WD-FS-3): the four-arm relationship is resolved against the THREE LOCAL
/// sets, not the network. Behavioral proxy: the SAME reachable index + the SAME
/// query produces the SAME result ROWS (the per-user-neutral corpus is identical for
/// any operator); only the per-row AFFORDANCE flips with the LOCAL sets. Two viewers
/// over the SAME index — one whose operator owns + caches the authors, one with a
/// clean store — render the SAME attributed rows; only the per-row affordance
/// differs (resolved LOCALLY). The index never learns who the operator is.
///
/// Given the SAME reachable index corpus and the SAME query;
/// When the search is rendered for an operator who owns/caches the authors and one
///   with a clean store;
/// Then BOTH renders carry the SAME attributed result rows (the index is per-user-
///   neutral), and only the per-row follow-state affordance differs (resolved LOCALLY).
///
/// @us-fs-001 @property @driving_port @real-io @local-resolution @offline @c-3 @gold
#[test]
fn the_four_arm_resolution_adds_no_network_seam_index_stays_per_user_neutral() {
    // GIVEN — render-1: an operator who owns the own claim, follows Rachel, and
    // caches the soft-removed Tobias, over the reachable four-arm index.
    let env_local = TestEnv::initialized();
    seed_own_claim_for_search(&env_local);
    seed_cached_unsubscribed_peer_for(&env_local, TRAVERSAL_AUTHOR_TOBIAS, TOBIAS_CACHED_SEED);
    let _rachel_sub = seed_active_subscription_for(&env_local, RACHEL_DID, RACHEL_ACTIVE_SUB_SEED);
    let local_body = {
        let indexer = seed_network_index_from_specs(&env_local, sf_corpus_all_four_arms());
        let viewer = ViewerServer::start_with_indexer(&env_local, indexer);
        let r = viewer.get(&format!("/search?object={SF_OBJECT_REPRODUCIBLE_BUILDS}"));
        assert_eq!(
            r.status, 200,
            "FF-INV-PerUserNeutral: local-operator render must be 200; body:\n{}",
            r.body
        );
        r.body
    };

    // GIVEN — render-2: an operator with a CLEAN store (no own claim, no
    // subscription, no cache), over the SAME index corpus (byte-identical).
    let env_clean = TestEnv::initialized();
    let clean_body = {
        let indexer = seed_network_index_from_specs(&env_clean, sf_corpus_all_four_arms());
        let viewer = ViewerServer::start_with_indexer(&env_clean, indexer);
        let r = viewer.get(&format!("/search?object={SF_OBJECT_REPRODUCIBLE_BUILDS}"));
        assert_eq!(
            r.status, 200,
            "FF-INV-PerUserNeutral: clean-operator render must be 200; body:\n{}",
            r.body
        );
        r.body
    };

    // THEN BOTH renders carry the SAME attributed result ROWS — the index is
    // per-user-neutral (it returned all four authors identically for BOTH operators;
    // it never learned who the operator is or whom she removed).
    for body in [&local_body, &clean_body] {
        assert_search_html_every_row_verified_and_attributed(
            body,
            &[
                SF_OWN_BARE_DID,
                RACHEL_DID,
                TRAVERSAL_AUTHOR_TOBIAS,
                PRIYA_DID,
            ],
        );
    }
    // …and ONLY the per-row AFFORDANCE differs, resolved LOCALLY (RED today): the
    // local operator sees the self + residue indicators; the clean operator sees the
    // SAME rows ALL as `peer add` (the slice-08 status quo — clean store → every
    // author NetworkUnfollowed). Priya is `peer add` for both.
    assert_search_row_shows_self_indicator(&local_body, SF_OWN_BARE_DID);
    assert_search_row_offers_follow(&clean_body, SF_OWN_BARE_DID);
    assert_search_row_shows_residue_indicator(&local_body, TRAVERSAL_AUTHOR_TOBIAS);
    assert_search_row_offers_follow(&clean_body, TRAVERSAL_AUTHOR_TOBIAS);
    assert_search_row_offers_follow(&local_body, PRIYA_DID);
    assert_search_row_offers_follow(&clean_body, PRIYA_DID);
}

// =============================================================================
// C-5 / J-003a — the four-arm resolution sets the per-row relationship ONLY; it
// does not merge or re-rank (FF-INV-AttributionUnchanged).
// =============================================================================

/// FF-INV-AttributionUnchanged / GOLD `the_four_arm_resolution_does_not_merge_or_rerank`
/// (C-5 / J-003a / WD-FS): the four-arm resolution sets the per-row relationship
/// ONLY — grouping + order + `[verified]` marker + verbatim confidence are UNCHANGED
/// whether or not the new arms apply. The SAME index + SAME query renders the SAME
/// attributed rows with-and-without the local state present; only the per-row
/// affordance differs (no merged / re-ranked output). The slice-20 companion to the
/// slice-16 anti-merging gold — proving the two NEW arms are per-row enrichments,
/// not a second grouping path.
///
/// Given the SAME reachable index corpus and the SAME query;
/// When the search is rendered with the local state present and again without;
/// Then both renders carry the SAME attributed rows with the SAME verified markers +
///   verbatim confidence + no merged consensus row (only the per-row affordance flips).
///
/// @us-fs-002 @property @driving_port @real-io @anti-merging @attribution @c-5 @gold
#[test]
fn the_four_arm_resolution_does_not_merge_or_rerank() {
    // GIVEN render-WITH: an operator with all local state, over the four-arm index.
    let env_with = TestEnv::initialized();
    seed_own_claim_for_search(&env_with);
    seed_cached_unsubscribed_peer_for(&env_with, TRAVERSAL_AUTHOR_TOBIAS, TOBIAS_CACHED_SEED);
    let _rachel_sub = seed_active_subscription_for(&env_with, RACHEL_DID, RACHEL_ACTIVE_SUB_SEED);
    let with_body = {
        let indexer = seed_network_index_from_specs(&env_with, sf_corpus_all_four_arms());
        let viewer = ViewerServer::start_with_indexer(&env_with, indexer);
        let r = viewer.get(&format!("/search?object={SF_OBJECT_REPRODUCIBLE_BUILDS}"));
        assert_eq!(
            r.status, 200,
            "FF-INV-AttributionUnchanged: with-state render must be 200; body:\n{}",
            r.body
        );
        r.body
    };

    // GIVEN render-WITHOUT: a clean operator, over the SAME corpus.
    let env_without = TestEnv::initialized();
    let without_body = {
        let indexer = seed_network_index_from_specs(&env_without, sf_corpus_all_four_arms());
        let viewer = ViewerServer::start_with_indexer(&env_without, indexer);
        let r = viewer.get(&format!("/search?object={SF_OBJECT_REPRODUCIBLE_BUILDS}"));
        assert_eq!(
            r.status, 200,
            "FF-INV-AttributionUnchanged: without-state render must be 200; body:\n{}",
            r.body
        );
        r.body
    };

    // THEN both renders carry the SAME attributed rows (all four authors), the SAME
    // verified markers, and NO merged consensus row — grouping/order/attribution are
    // UNCHANGED by the four-arm resolution (C-5 / J-003a). Confidence is byte-stable.
    for body in [&with_body, &without_body] {
        assert_search_html_every_row_verified_and_attributed(
            body,
            &[
                SF_OWN_BARE_DID,
                RACHEL_DID,
                TRAVERSAL_AUTHOR_TOBIAS,
                PRIYA_DID,
            ],
        );
        assert_search_html_has_no_merged_consensus_row(body);
        for confidence in ["0.95", "0.88", "0.74", "0.82"] {
            assert!(
                body.contains(confidence),
                "FF-INV-AttributionUnchanged (C-5): verbatim confidence {confidence:?} must \
                 be unchanged whether or not the local state is present; body:\n{body}"
            );
        }
    }
    // …and ONLY the per-row affordance differs (RED today): WITH the local state, the
    // own row shows the self indicator + Tobias shows the residue indicator; WITHOUT,
    // both show `peer add` — but the ROW set, order, attribution, and confidence are
    // identical. The two new arms are per-row enrichments, never a merge/re-rank.
    assert_search_row_shows_self_indicator(&with_body, SF_OWN_BARE_DID);
    assert_search_row_offers_follow(&without_body, SF_OWN_BARE_DID);
    assert_search_row_shows_residue_indicator(&with_body, TRAVERSAL_AUTHOR_TOBIAS);
    assert_search_row_offers_follow(&without_body, TRAVERSAL_AUTHOR_TOBIAS);
}

// =============================================================================
// C-9 / WD-FS-8 — the two NEW indicators are NEUTRAL, never pejorative
// (FF-INV-NeutralFraming).
// =============================================================================

/// FF-INV-NeutralFraming / GOLD `the_two_new_indicators_are_neutral_never_pejorative`
/// (C-9 / WD-FS-8): over a render carrying BOTH new indicators (the self indicator +
/// the residue indicator), the two new indicators are NEUTRAL descriptive TEXT — NO
/// blocklisted judgement term (`ex-peer`, `abandoned`, `stale`, `disputed`,
/// `refuted`, `blocked`, `banned`, `untrustworthy`, …) appears ANYWHERE in the
/// follow-state surface. The operator reads the self + residue indicators as facts,
/// never as a verdict. AND both indicators ARE present (so the gold is non-vacuous —
/// it pins that the PRESENT copy is neutral, not merely that absent copy is neutral).
///
/// Given the viewer serves a /search render carrying the self + residue indicators;
/// When the results render in both shapes;
/// Then both new indicators are present AND neutral — no pejorative / judgement term.
///
/// @us-fs-002 @property @driving_port @real-io @neutral-framing @c-9 @gold
#[test]
fn the_two_new_indicators_are_neutral_never_pejorative() {
    let env = TestEnv::initialized();
    seed_own_claim_for_search(&env);
    seed_cached_unsubscribed_peer_for(&env, TRAVERSAL_AUTHOR_TOBIAS, TOBIAS_CACHED_SEED);
    let _rachel_sub = seed_active_subscription_for(&env, RACHEL_DID, RACHEL_ACTIVE_SUB_SEED);

    let mut responses = Vec::new();
    {
        let indexer = seed_network_index_from_specs(&env, sf_corpus_all_four_arms());
        let viewer = ViewerServer::start_with_indexer(&env, indexer);

        let path = format!("/search?object={SF_OBJECT_REPRODUCIBLE_BUILDS}");
        responses.push((format!("GET {path} (full page)"), viewer.get(&path)));
        responses.push((
            format!("GET {path} (htmx fragment)"),
            viewer.get_htmx(&path),
        ));
    }

    for (label, response) in &responses {
        assert_eq!(
            response.status, 200,
            "FF-INV-NeutralFraming: /search route {label:?} must render successfully (200); \
             got {}",
            response.status
        );
        // Both new indicators are PRESENT (non-vacuous; RED today — the arms are empty).
        assert_search_row_shows_self_indicator(&response.body, SF_OWN_BARE_DID);
        assert_search_row_shows_residue_indicator(&response.body, TRAVERSAL_AUTHOR_TOBIAS);
        // …and the follow-state surface carries NO pejorative / judgement term (C-9).
        assert_search_follow_state_framing_is_neutral(&response.body);
    }
}

// =============================================================================
// C-2 / WD-FS-2 — own-vs-cache distinctness: a removed-but-cached author is NOT
// shown as a fresh add candidate (FF-INV-OwnVsCacheDistinct).
// =============================================================================

/// FF-INV-OwnVsCacheDistinct / GOLD
/// `a_removed_but_cached_author_is_not_shown_as_a_fresh_add_candidate` (C-2 /
/// WD-FS-2 — the defining accuracy guarantee of the cache arm): an
/// `UnsubscribedCache` row is NOT a `NetworkUnfollowed` row. A peer the operator
/// soft-removed (cached, NOT active) is NEVER re-offered the `openlore peer add`
/// affordance — he is residue, not a fresh network find. This is the load-bearing
/// distinction the slice exists to draw: before slice-20 his cached claim was
/// indistinguishable from a fresh discovery; after, the add affordance is suppressed
/// for him and shown ONLY for the genuinely-new author. Behavioral proxy: in a
/// render carrying BOTH a soft-removed cached peer (Tobias) AND a genuinely-new
/// author (Priya), `peer add` is present for Priya and ABSENT for Tobias.
///
/// Given the operator soft-removed Tobias (cached) and does not know Priya, and the
///   index holds verified claims by both;
/// When she opens GET /search and both appear;
/// Then Tobias's row shows the residue indicator + NO `peer add` (he is residue), and
///   Priya's row shows `openlore peer add did:plc:priya-test` (she is a fresh find).
///
/// @us-fs-001 @us-fs-002 @property @driving_port @real-io @follow-state-completeness
/// @c-2 @gold
#[test]
fn a_removed_but_cached_author_is_not_shown_as_a_fresh_add_candidate() {
    // GIVEN Tobias soft-removed (cached, not active) + Priya unknown, AND a reachable
    // index where BOTH appear. Tobias must NOT be a fresh add candidate; Priya must be.
    let env = TestEnv::initialized();
    seed_cached_unsubscribed_peer_for(&env, TRAVERSAL_AUTHOR_TOBIAS, TOBIAS_CACHED_SEED);
    let indexer = seed_network_index_from_specs(&env, sf_corpus_cached_and_new());
    let viewer = ViewerServer::start_with_indexer(&env, indexer);

    let response = viewer.get(&format!("/search?object={SF_OBJECT_REPRODUCIBLE_BUILDS}"));

    assert_eq!(
        response.status, 200,
        "FF-INV-OwnVsCacheDistinct: GET /search over the cached+new seeded index must be \
         200; body:\n{}",
        response.body
    );
    // THEN Tobias (residue) shows the residue indicator + NO `peer add` (RED today: no
    // cached read → he resolves NetworkUnfollowed → `peer add`, indistinguishable from
    // a fresh find — exactly the bug this gold pins).
    assert_search_row_shows_residue_indicator(&response.body, TRAVERSAL_AUTHOR_TOBIAS);
    // …and Priya (genuinely new) DOES show `openlore peer add did:plc:priya-test` — the
    // add affordance is shown ONLY for the genuinely-new author, distinguishing
    // UnsubscribedCache from NetworkUnfollowed (the own-vs-cache distinctness, C-2).
    assert_search_row_offers_follow(&response.body, PRIYA_DID);
}
