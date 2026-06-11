//! Slice-20 acceptance — the `openlore ui` `/search` FULL FOUR-ARM follow-state
//! (US-FS-001/002; ADR-057). COMPLETES the slice-16 binary resolution.
//!
//! slice-16 resolved `/search` follow-state BINARY (`SubscribedPeer` ∈ active set
//! vs `NetworkUnfollowed` otherwise) and EXPLICITLY DEFERRED the two further arms
//! of the four-variant `AuthorRelationship` enum. slice-20 RESOLVES them:
//!
//!   • `You` — the result author IS the operator (own claim). slice-16 misclassified
//!     this as `NetworkUnfollowed` (re-offered a self-follow). slice-20 resolves it
//!     against a NEW LOCAL presence read (`distinct_own_author_dids` over `claims`)
//!     and renders a NEUTRAL self indicator (`SEARCH_SELF_INDICATOR`), NO `peer add`.
//!   • `UnsubscribedCache` — the result author is a peer the operator soft-removed
//!     (cached in `peer_claims`, NOT in the active set — the slice-15 PS-4 residue).
//!     slice-16 misclassified this as `NetworkUnfollowed`. slice-20 resolves it
//!     against a NEW LOCAL presence read (`distinct_cached_peer_author_dids` over
//!     `peer_claims`, NO `removed_at` filter), gated by ∉ active, and renders a
//!     NEUTRAL residue indicator (`SEARCH_REMOVED_CACHED_INDICATOR`), NO `peer add`.
//!
//! Precedence (C-6 / WD-FS-2 / ADR-057 D2): `You` > `SubscribedPeer` >
//! `UnsubscribedCache` > `NetworkUnfollowed`. The slice-16 `SubscribedPeer` /
//! `NetworkUnfollowed` arms are byte-stable (C-7, CARDINAL). The two new arms only
//! ADD. Render-only, read-only, LOCAL/offline, additive — no new route, no new
//! `AuthorRelationship` variant, no new crate (workspace stays 21).
//!
//! Driving discipline (Mandate 1): scenarios enter through the REAL `openlore ui`
//! subprocess (`ViewerServer`) + in-test HTTP GET /search (with/without the
//! `HX-Request` header). The network index is the ONLY mocked boundary — a REAL
//! slice-05 `openlore-indexer serve` over a seeded corpus
//! (`seed_network_index_from_specs` → `ViewerServer::start_with_indexer`). The LOCAL
//! DuckDB store is REAL: the operator's own claim is seeded via the REAL slice-06
//! `claim add` verb (`seed_own_claim_for_search`); the followed peer via the REAL
//! slice-03 `peer add` verb (`seed_active_subscription_for`); the soft-removed-but-
//! cached peer via the REAL `peer add` + `peer pull` + `peer remove` (no `--purge`)
//! verbs (`seed_cached_unsubscribed_peer_for`). NO scenario calls the `viewer-domain`
//! render fns or the adapter resolution fn directly (those are unit-level, DELIVER).
//!
//! Seeding alignment (the load-bearing trick, extended from slice-16): the operator's
//! OWN DID under the viewer is `did:plc:test-jeff` (the `OPENLORE_DID` the
//! `ViewerServer` runs under = `env.identity`). A `You` row appears when the index
//! corpus carries a row authored by `did:plc:test-jeff` AND the operator has
//! published that claim LOCALLY. Tobias (`did:plc:tobias-test`) is cached-then-soft-
//! removed → ∈ cached set, ∉ active set → `UnsubscribedCache`. The fragmented
//! result `author_did`s reconcile against the bare LOCAL set DIDs via the production
//! `bare_did` SSOT (R-FS-6).
//!
//! Layer placement (nw-tdd-methodology Layered Test Discipline matrix): every scenario
//! is a layer-3/layer-5 subprocess + real-I/O test — EXAMPLE-only (Mandate 9/11). Sad
//! paths (the independent-degrade-on-cached-read-failure) are enumerated explicitly,
//! never PBT-generated at this layer. Tier B (state-machine PBT) is NOT warranted:
//! the resolution is a per-row precedence lookup over THREE set memberships (a
//! config-shaped "which render-only affordance" choice), not a ≥3-scenario chained
//! journey over a domain-rich state machine (Mandate 10 skip criteria).
//!
//! Build-before-run note (mirrors slice-08/16): `cargo test` does NOT rebuild a
//! spawned binary automatically — the run MUST `cargo build` BOTH the `openlore` bin
//! (the viewer) AND the `openlore-indexer` bin (the seeded serve) before running
//! these ATs so `ViewerServer::start_with_indexer` spawns the CURRENT viewer over a
//! CURRENT indexer, not a stale one.
//!
//! Mandate 7 RED scaffolds: the ATs import nothing unbuilt at the Rust level (they
//! spawn the bins + HTTP), so they COMPILE now. The follow-state RED is the
//! PRODUCTION arm: the render `@match` arm `You | UnsubscribedCache => {}` is EMPTY
//! and `to_indexed_claim` resolves only the binary (no own/cached presence reads
//! exist), so an own claim AND a soft-removed peer's cached claim BOTH still resolve
//! `NetworkUnfollowed` → render `peer add`. So `assert_search_row_shows_self_
//! indicator` / `assert_search_row_shows_residue_indicator` FAIL for the RIGHT
//! reason (MISSING_FUNCTIONALITY: the two presence reads + the four-arm precedence +
//! the two render arms + the two SSOT consts do not exist yet), NOT a setup/import
//! error. The independent-degrade scenario panics at the `todo!()` cached-peer
//! fault seam (also MISSING_FUNCTIONALITY — OQ-1 escalation). They stay RED until
//! DELIVER's per-scenario RED→GREEN→COMMIT cycles (ADR-025).
//!
//! Covers:
//! - US-FS-002 / Theme A (the load-bearing four-arm completion): FF-1 walking
//!   skeleton (all four arms side by side) · FF-2 You-only · FF-3 UnsubscribedCache-only.
//! - US-FS-001 / Theme B (precedence): FF-4 active-and-cached → SubscribedPeer ·
//!   FF-5 own beats a populated active set → You.
//! - US-FS-002 / Theme C (no-regression, CARDINAL): FF-6 SubscribedPeer +
//!   NetworkUnfollowed byte-stable vs slice-16.
//! - US-FS-001 / Theme D (fragment-strip): FF-7 a soft-removed-then-fragmented author
//!   still resolves UnsubscribedCache.
//! - US-FS-002 / Theme E (read-only / neutral): FF-8 neither new indicator is an
//!   executable control + both are neutral, never pejorative.
//! - US-FS-001 / Theme F (independent degrade): FF-9 a failed cached-peer read
//!   degrades ONLY that arm (RED scaffold via the per-read fault seam, OQ-1).
//! - US-FS-002 / Theme G (parity): FF-10 the four arms render identically under
//!   htmx fragment + no-JS full page.
//
// SCAFFOLD: true

mod support;

#[allow(unused_imports)]
use support::*;

use openlore_test_support::{PRIYA_DID, RACHEL_DID};

// Distinct deterministic seeds per peer DID (so the verifiable records do not
// alias). Rachel = the FOLLOWED peer (active subscription); Tobias = the
// SOFT-REMOVED-but-cached peer (residue).
const RACHEL_ACTIVE_SUB_SEED: [u8; 32] = [7u8; 32];
const TOBIAS_CACHED_SEED: [u8; 32] = [9u8; 32];

// =============================================================================
// US-FS-002 — Theme A: the LOAD-BEARING four-arm completion. (FF-1 walking
// skeleton · FF-2 You-only · FF-3 UnsubscribedCache-only)
// =============================================================================

/// FF-1 / WALKING SKELETON (US-FS-001/002 / Theme A; C-2; R-FS-3/-4/-5 — the
/// riskiest, load-bearing thread): in ONE `/search` over a reachable index that
/// returns ALL FOUR follow-states at once, each row shows its honest affordance —
/// the operator's OWN claim shows the neutral self indicator (no add), Rachel
/// (followed) shows "Following" (no add), Tobias (soft-removed, cached) shows the
/// neutral residue indicator (no add), and Priya (genuinely new) keeps the slice-08
/// `openlore peer add did:plc:priya-test` affordance. This is the thinnest complete
/// thread the slice can demo end-to-end: viewer → THREE LOCAL presence reads (own,
/// active, cached) → four-arm precedence resolution → FOUR DIFFERENTIATED
/// render-only affordances on the SAME page, with `peer add` ONLY on the genuinely-
/// new row.
///
/// Given the operator has her own claim, follows Rachel, soft-removed Tobias
///   (cached), and does not know Priya — and a reachable index holds verified
///   reproducible-builds claims by ALL FOUR;
/// When she opens GET /search?object=reproducible-builds and all four claims appear;
/// Then her own row shows the neutral self indicator + no add, Rachel's shows
///   "Following" + no add, Tobias's shows the neutral residue indicator + no add,
///   and Priya's keeps `openlore peer add did:plc:priya-test` — `peer add` appears
///   ONLY on Priya's row.
///
/// @us-fs-001 @us-fs-002 @walking_skeleton @driving_port @driving_adapter @real-io
/// @follow-state-completeness @search-state-results @c-2 @happy
#[test]
fn all_four_follow_states_render_with_peer_add_only_on_the_genuinely_new_author() {
    // GIVEN the LOCAL store seeded to MATCH all four arms (Pillar 3 — the SAME REAL
    // DuckDB the viewer opens, every fact via a production write verb):
    //   • the operator's OWN claim (via the real `claim add` verb → a `claims` row
    //     keyed on did:plc:test-jeff) → `You`;
    //   • Rachel active (a real `peer add`) → `SubscribedPeer`;
    //   • Tobias cached-then-soft-removed (real `peer add`+`peer pull`+`peer remove`,
    //     no `--purge`) → `UnsubscribedCache`;
    //   • Priya untouched → `NetworkUnfollowed`.
    // …AND a REAL `openlore-indexer serve` over a corpus where all four assert the
    // headline reproducible-builds object.
    // Seeding ORDER matters (the `peer pull` seam pulls ALL active subscriptions at
    // once): seed the cached-then-soft-removed peer FIRST (its pull runs while it is
    // the ONLY active peer), THEN the active subscription (a `peer add` ALONE, no
    // pull — so no cross-peer pull-seam collision). The own claim is a `claim add`
    // (touches no peers).
    let env = TestEnv::initialized();
    seed_own_claim_for_search(&env);
    seed_cached_unsubscribed_peer_for(&env, TRAVERSAL_AUTHOR_TOBIAS, TOBIAS_CACHED_SEED);
    let _rachel_sub = seed_active_subscription_for(&env, RACHEL_DID, RACHEL_ACTIVE_SUB_SEED);
    let indexer = seed_network_index_from_specs(&env, sf_corpus_all_four_arms());
    let viewer = ViewerServer::start_with_indexer(&env, indexer);

    // WHEN she opens the object search (full page).
    let response = viewer.get(&format!("/search?object={SF_OBJECT_REPRODUCIBLE_BUILDS}"));

    assert_eq!(
        response.status, 200,
        "FF-1: GET /search over the four-arm seeded index must be 200; body:\n{}",
        response.body
    );
    // THEN every author is still attributed + verified (the relationship is a per-row
    // enrichment; attribution + the verified marker are UNCHANGED — C-5).
    assert_search_html_every_row_verified_and_attributed(
        &response.body,
        &[SF_OWN_BARE_DID, RACHEL_DID, TRAVERSAL_AUTHOR_TOBIAS, PRIYA_DID],
    );
    // …the operator's OWN claim shows the neutral self indicator + NO add (RED today:
    // the `You` arm is empty + no own-DID read exists → renders `peer add`).
    assert_search_row_shows_self_indicator(&response.body, SF_OWN_BARE_DID);
    // …Rachel (FOLLOWED) shows the slice-16 "Following" indicator + NO add (BYTE-STABLE).
    assert_search_row_following(&response.body, RACHEL_DID);
    // …Tobias (SOFT-REMOVED, cached) shows the neutral residue indicator + NO add (RED
    // today: the `UnsubscribedCache` arm is empty + no cached read exists → `peer add`).
    assert_search_row_shows_residue_indicator(&response.body, TRAVERSAL_AUTHOR_TOBIAS);
    // …Priya (genuinely NEW) KEEPS the slice-08 `openlore peer add did:plc:priya-test`
    // affordance (BYTE-STABLE).
    assert_search_row_offers_follow(&response.body, PRIYA_DID);
    // …and `openlore peer add` appears for Priya ONLY — never for the own claim, the
    // followed peer, or the soft-removed peer (the four-arm completeness guarantee:
    // the add affordance is shown ONLY where it is actionable — a genuinely-new author).
    for suppressed in [SF_OWN_BARE_DID, RACHEL_DID, TRAVERSAL_AUTHOR_TOBIAS] {
        let suppressed_add = format!("{SF_FOLLOW_COMMAND_VERB} {suppressed}");
        assert!(
            !response.body.contains(&suppressed_add),
            "FF-1 (C-2): `peer add` must appear ONLY on the genuinely-new author's row — \
             expected NO {suppressed_add:?}; body:\n{}",
            response.body
        );
    }
}

/// FF-2 (US-FS-002 / Theme A; C-2 — the `You`-in-isolation confirmatory case): a
/// search returning ONLY the operator's own claim shows the neutral self indicator
/// and NO `openlore peer add` command appears ANYWHERE — you cannot follow yourself.
///
/// Given the operator has published her own claim, and the index holds ONLY that claim;
/// When she opens GET /search;
/// Then her row shows the neutral self indicator and no `openlore peer add` command
///   appears anywhere.
///
/// @us-fs-002 @driving_port @real-io @follow-state-completeness @search-state-results
/// @c-2 @happy
#[test]
fn an_own_claim_in_isolation_shows_the_self_indicator_and_no_add_command_anywhere() {
    // GIVEN the operator's OWN claim seeded LOCALLY (via the real `claim add` verb),
    // AND a reachable index whose results are ONLY her own claim.
    let env = TestEnv::initialized();
    seed_own_claim_for_search(&env);
    let indexer = seed_network_index_from_specs(&env, sf_corpus_own_claim_only());
    let viewer = ViewerServer::start_with_indexer(&env, indexer);

    let response = viewer.get(&format!("/search?object={SF_OBJECT_REPRODUCIBLE_BUILDS}"));

    assert_eq!(
        response.status, 200,
        "FF-2: GET /search over the own-claim-only seeded index must be 200; body:\n{}",
        response.body
    );
    // Her own claim is attributed + verified (unchanged).
    assert_search_html_every_row_verified_and_attributed(&response.body, &[SF_OWN_BARE_DID]);
    // …and shows the neutral self indicator + no add for it (RED today).
    assert_search_row_shows_self_indicator(&response.body, SF_OWN_BARE_DID);
    // …and NO `openlore peer add` command appears ANYWHERE in the results (you cannot
    // follow yourself — the strongest own-claim guarantee).
    assert!(
        !response.body.contains(SF_FOLLOW_COMMAND_VERB),
        "FF-2 (C-2): when the only result is the operator's own claim, NO \
         `{SF_FOLLOW_COMMAND_VERB}` command must appear ANYWHERE; body:\n{}",
        response.body
    );
}

/// FF-3 (US-FS-002 / Theme A; C-2 — the `UnsubscribedCache`-in-isolation
/// confirmatory case): a search returning ONLY a soft-removed-but-cached peer's
/// claim shows the neutral residue indicator and NO `openlore peer add` command
/// appears ANYWHERE — he is residue, not a fresh network find.
///
/// Given the operator soft-removed Tobias (his cached claims retained, subscription
///   inactive), and the index holds ONLY Tobias's claim;
/// When she opens GET /search;
/// Then his row shows the neutral residue indicator and no `openlore peer add`
///   command appears anywhere.
///
/// @us-fs-002 @driving_port @real-io @follow-state-completeness @search-state-results
/// @c-2 @happy
#[test]
fn a_soft_removed_cached_peer_in_isolation_shows_the_residue_indicator_and_no_add_anywhere() {
    // GIVEN Tobias cached-then-soft-removed LOCALLY (the residue state seeded via the
    // real `peer add`+`peer pull`+`peer remove`), AND a reachable index whose results
    // are ONLY his claim.
    let env = TestEnv::initialized();
    seed_cached_unsubscribed_peer_for(&env, TRAVERSAL_AUTHOR_TOBIAS, TOBIAS_CACHED_SEED);
    let indexer = seed_network_index_from_specs(&env, sf_corpus_cached_removed_peer_only());
    let viewer = ViewerServer::start_with_indexer(&env, indexer);

    let response = viewer.get(&format!("/search?object={SF_OBJECT_REPRODUCIBLE_BUILDS}"));

    assert_eq!(
        response.status, 200,
        "FF-3: GET /search over the cached-removed-peer-only seeded index must be 200; \
         body:\n{}",
        response.body
    );
    assert_search_html_every_row_verified_and_attributed(&response.body, &[TRAVERSAL_AUTHOR_TOBIAS]);
    // …Tobias shows the neutral residue indicator + no add (RED today: he resolves
    // NetworkUnfollowed → `peer add`, since no cached-peer read exists yet).
    assert_search_row_shows_residue_indicator(&response.body, TRAVERSAL_AUTHOR_TOBIAS);
    // …and NO `openlore peer add` command appears ANYWHERE (he is residue, not a
    // fresh find — distinguishing UnsubscribedCache from NetworkUnfollowed, the
    // own-vs-cache-distinct guarantee for the cache side).
    assert!(
        !response.body.contains(SF_FOLLOW_COMMAND_VERB),
        "FF-3 (C-2): when the only result is a soft-removed peer's cached claim, NO \
         `{SF_FOLLOW_COMMAND_VERB}` command must appear ANYWHERE (he is residue, NOT a \
         fresh network find); body:\n{}",
        response.body
    );
}

// =============================================================================
// US-FS-001 — Theme B: PRECEDENCE (C-6 / WD-FS-2). `You` > `SubscribedPeer` >
// `UnsubscribedCache` > `NetworkUnfollowed`. (FF-4 active>cached · FF-5 own>cached)
// =============================================================================

/// FF-4 (US-FS-001 / Theme B; C-6 — the active-outranks-cached precedence edge): a
/// peer who is BOTH actively followed AND present in the cached-peer set resolves to
/// `SubscribedPeer` ("Following"), never `UnsubscribedCache` (residue). You follow
/// her NOW; the cache is incidental.
///
/// Given the operator actively follows Rachel AND also holds Rachel's cached claims;
/// When she opens GET /search and Rachel's claim appears;
/// Then Rachel's row shows "Following" (active outranks cached), NOT the residue
///   indicator.
///
/// @us-fs-001 @driving_port @real-io @precedence @search-state-results @c-6 @edge
#[test]
fn an_active_and_cached_peer_resolves_to_subscribed_peer_by_precedence() {
    // GIVEN Rachel is BOTH actively followed AND in the cached-peer set: a `peer add`
    // + `peer pull` (NO remove) leaves her subscription ACTIVE (`removed_at IS NULL`)
    // AND her cached claims in `peer_claims` — so she is ∈ active AND ∈ cached.
    // Precedence must pick `SubscribedPeer` (active outranks cached).
    let env = TestEnv::initialized();
    seed_active_and_cached_peer_for(&env, RACHEL_DID, RACHEL_ACTIVE_SUB_SEED);
    let indexer = seed_network_index_from_specs(&env, sf_corpus_one_followed_one_unfollowed());
    let viewer = ViewerServer::start_with_indexer(&env, indexer);

    let response = viewer.get(&format!("/search?object={SF_OBJECT_REPRODUCIBLE_BUILDS}"));

    assert_eq!(
        response.status, 200,
        "FF-4: GET /search over the active-and-cached seeded index must be 200; body:\n{}",
        response.body
    );
    // THEN Rachel resolves to "Following" (active outranks cached, C-6) — RED today
    // (the binary resolution already resolves her SubscribedPeer, but the residue
    // arm must NOT win; once the four-arm resolution lands, precedence pins it).
    assert_search_row_following(&response.body, RACHEL_DID);
    // …and her row does NOT show the residue indicator (the cache is incidental — she
    // is followed NOW). None of the residue phrasings attaches to her row.
    assert!(
        !response.body.contains(SF_REMOVED_CACHED_INDICATOR_DEFAULT),
        "FF-4 (C-6): an active-and-cached peer must resolve `SubscribedPeer` (\"Following\"), \
         NOT the residue indicator {SF_REMOVED_CACHED_INDICATOR_DEFAULT:?} (active outranks \
         cached); body:\n{}",
        response.body
    );
}

/// FF-5 (US-FS-001 / Theme B; C-6 — `You` is the strongest fact, at the TOP of the
/// chain): the operator's own claim resolves to `You` EVEN WHEN the active set is
/// non-empty (she follows OTHER peers). `You` outranks `SubscribedPeer` — her own row
/// is never mistaken for a followed peer's. (The structurally-impossible
/// own-AND-self-followed / own-AND-self-cached edges — `peer add <own-did>` is
/// rejected by design, "cannot subscribe to yourself" — are NOT seedable through the
/// real verbs; the total-precedence pure fn pins those at the DELIVER unit layer.
/// This AT pins the realizable TOP-of-chain edge: own beats a POPULATED active set.)
///
/// Given the operator has published her own claim AND actively follows Rachel (a
///   DIFFERENT author), and the index returns her own claim alongside Rachel's;
/// When she opens GET /search;
/// Then her own row shows the neutral self indicator (`You` wins over the populated
///   active set), and Rachel's row shows "Following" — the two never cross over.
///
/// @us-fs-001 @driving_port @real-io @precedence @search-state-results @c-6 @edge
#[test]
fn an_own_claim_resolves_to_you_even_when_the_active_set_is_populated() {
    // GIVEN the operator's own claim seeded LOCALLY (via `claim add` → the own-DID
    // set), AND a POPULATED active set (she follows Rachel, a DIFFERENT author, via a
    // real `peer add`). `You` must outrank `SubscribedPeer` — the own row resolves
    // `You`, not `SubscribedPeer`. The index returns her own claim + Rachel's.
    let env = TestEnv::initialized();
    seed_own_claim_for_search(&env);
    let _rachel_sub = seed_active_subscription_for(&env, RACHEL_DID, RACHEL_ACTIVE_SUB_SEED);
    let indexer = seed_network_index_from_specs(&env, sf_corpus_own_and_followed());
    let viewer = ViewerServer::start_with_indexer(&env, indexer);

    let response = viewer.get(&format!("/search?object={SF_OBJECT_REPRODUCIBLE_BUILDS}"));

    assert_eq!(
        response.status, 200,
        "FF-5: GET /search over the own-and-followed seeded index must be 200; body:\n{}",
        response.body
    );
    // THEN the own row resolves to `You` (the strongest fact, C-6) — the self
    // indicator, NOT mistaken for a followed peer (RED today: no own-DID read + the
    // empty `You` arm). Rachel's row shows "Following" (the active set is populated +
    // correctly resolves her). The two arms never cross over.
    assert_search_row_shows_self_indicator(&response.body, SF_OWN_BARE_DID);
    assert_search_row_following(&response.body, RACHEL_DID);
}

// =============================================================================
// US-FS-002 — Theme C: NO-REGRESSION (C-7, CARDINAL). The slice-16
// SubscribedPeer + NetworkUnfollowed rendering is byte-stable. (FF-6)
// =============================================================================

/// FF-6 (US-FS-002 / Theme C; C-7, CARDINAL / WD-FS-7 — the no-regression /
/// byte-stability gate): a search returning ONLY a followed author + an unfollowed
/// author (NEITHER the operator's own claim NOR a soft-removed peer present) renders
/// EXACTLY as slice-16 — Rachel "Following" + no add; Priya `openlore peer add
/// did:plc:priya-test`; NO self indicator; NO residue indicator. The two new arms
/// add NOTHING where they do not apply — the slice-16 trust baseline is preserved.
///
/// Given the operator follows Rachel but not Priya; neither her own claim nor a
///   soft-removed peer appears; and the index holds verified claims by both;
/// When she opens GET /search and both claims appear;
/// Then Rachel's row shows "Following" + no add, Priya's keeps `openlore peer add
///   did:plc:priya-test`, and NEITHER the self indicator NOR the residue indicator
///   appears (byte-stable vs slice-16 — the two new arms add nothing here).
///
/// @us-fs-002 @driving_port @real-io @no-regression @search-state-results @c-7
/// @boundary
#[test]
fn the_slice16_followed_and_unfollowed_states_are_byte_stable_with_no_new_indicators() {
    // GIVEN the slice-16 mix (Rachel followed; Priya unfollowed) — NO own claim
    // seeded, NO soft-removed peer seeded. The two new presence sets are EMPTY, so
    // the resolution falls through to the slice-16 binary outcome verbatim.
    let env = TestEnv::initialized();
    let _rachel_sub = seed_active_subscription_for(&env, RACHEL_DID, RACHEL_ACTIVE_SUB_SEED);
    let indexer = seed_network_index_from_specs(&env, sf_corpus_one_followed_one_unfollowed());
    let viewer = ViewerServer::start_with_indexer(&env, indexer);

    let response = viewer.get(&format!("/search?object={SF_OBJECT_REPRODUCIBLE_BUILDS}"));

    assert_eq!(
        response.status, 200,
        "FF-6: GET /search over the slice-16 mix seeded index must be 200; body:\n{}",
        response.body
    );
    // THEN the slice-16 rendering is byte-stable: Rachel "Following" + no add (this
    // assertion PASSES today — slice-16 already resolves it); Priya keeps `peer add`.
    assert_search_row_following(&response.body, RACHEL_DID);
    assert_search_row_offers_follow(&response.body, PRIYA_DID);
    // …and NEITHER new indicator appears anywhere — the two new arms add NOTHING
    // where they do not apply (no own claim, no soft-removed peer → no You, no
    // UnsubscribedCache). This is the additive / no-regression CARDINAL guarantee.
    for unexpected in [SF_SELF_INDICATOR_DEFAULT, SF_REMOVED_CACHED_INDICATOR_DEFAULT] {
        assert!(
            !response.body.contains(unexpected),
            "FF-6 (C-7, CARDINAL): a slice-16-shaped search (no own claim, no soft-removed \
             peer) must be byte-stable — the NEW indicator {unexpected:?} must NOT appear; \
             body:\n{}",
            response.body
        );
    }
}

// =============================================================================
// US-FS-001 — Theme D: FRAGMENT-STRIP (C-3 / R-FS-6). A soft-removed-then-
// fragmented author still resolves UnsubscribedCache. (FF-7)
// =============================================================================

/// FF-7 (US-FS-001 / Theme D; C-3 / R-FS-6 — the bare-DID reconciliation extended
/// to the cached set): a soft-removed peer is matched DESPITE the signing-key
/// fragment on the result DID. The cached-peer set stores the BARE DID
/// (`did:plc:tobias-test`); the search result row's `author_did` carries the
/// `#org.openlore.application` fragment. The comparison strips the fragment via the
/// production `bare_did` SSOT on BOTH sides before set membership, so the fragmented
/// row matches the bare cached DID → `UnsubscribedCache` (never misclassified as
/// `NetworkUnfollowed`).
///
/// Given the operator soft-removed the bare did:plc:tobias-test (cached), and the
///   result row's author DID is did:plc:tobias-test#org.openlore.application;
/// When she opens GET /search and that row appears;
/// Then the row shows the neutral residue indicator (the fragment is stripped before
///   the match) and no add command.
///
/// @us-fs-001 @driving_port @real-io @fragment-strip @local-resolution @r-fs-6 @edge
#[test]
fn a_soft_removed_author_is_matched_despite_the_signing_key_fragment_on_the_result_did() {
    // GIVEN Tobias soft-removed (the cached set holds the BARE did:plc:tobias-test).
    // The indexed corpus attributes his row to the app-identity
    // `did:plc:tobias-test#org.openlore.application` shape (the SAME fragmented shape
    // the viewer renders). The fragment-strip must reconcile the two for the match.
    let env = TestEnv::initialized();
    seed_cached_unsubscribed_peer_for(&env, TRAVERSAL_AUTHOR_TOBIAS, TOBIAS_CACHED_SEED);
    let indexer = seed_network_index_from_specs(&env, sf_corpus_cached_removed_peer_only());
    let viewer = ViewerServer::start_with_indexer(&env, indexer);

    let response = viewer.get(&format!("/search?object={SF_OBJECT_REPRODUCIBLE_BUILDS}"));

    assert_eq!(
        response.status, 200,
        "FF-7: GET /search over the fragmented-cached seeded index must be 200; body:\n{}",
        response.body
    );
    // The viewer renders Tobias's app-identity (fragmented) DID.
    let tobias_app_identity = format!("{TRAVERSAL_AUTHOR_TOBIAS}#org.openlore.application");
    assert!(
        response.body.contains(&tobias_app_identity),
        "FF-7 (R-FS-6): the viewer renders Tobias's app-identity (fragmented) DID \
         {tobias_app_identity:?}; body:\n{}",
        response.body
    );
    // THEN the fragmented result DID is reconciled against the bare cached DID →
    // `UnsubscribedCache` (the residue indicator), never NetworkUnfollowed (RED today:
    // no cached read + the empty arm → falls to `peer add`).
    assert_search_row_shows_residue_indicator(&response.body, TRAVERSAL_AUTHOR_TOBIAS);
}

// =============================================================================
// US-FS-002 — Theme E: READ-ONLY + NEUTRAL FRAMING (C-1, CARDINAL / C-9). Neither
// new indicator is an executable control; both are neutral, never pejorative. (FF-8)
// =============================================================================

/// FF-8 (US-FS-002 / Theme E; C-1, CARDINAL + C-9 / WD-FS-1/8): over a render that
/// carries ALL FOUR follow-states, NEITHER new indicator (self, residue) is an
/// executable control — both are render-only TEXT (no `<button>`, `<form>`,
/// mutating `<a>`, `hx-*` mutation, follow/subscribe input) — AND both are NEUTRAL,
/// never pejorative (no "ex-peer"/"abandoned"/"stale"/judgement). The viewer holds
/// no key. Extends the slice-16 read-only gate so the two NEW arms add no control
/// and carry no judgement.
///
/// Given a /search render carrying all four follow-states over a reachable index;
/// When the results render (full page + htmx fragment);
/// Then neither new indicator is an executable control, and both are neutral, never
///   pejorative.
///
/// @us-fs-002 @driving_port @real-io @read-only @neutral-framing @c-1 @c-9 @happy
#[test]
fn neither_new_indicator_is_an_executable_control_and_both_are_neutral() {
    // GIVEN the four-arm render so BOTH new indicators are present on the SAME surface.
    let env = TestEnv::initialized();
    seed_own_claim_for_search(&env);
    seed_cached_unsubscribed_peer_for(&env, TRAVERSAL_AUTHOR_TOBIAS, TOBIAS_CACHED_SEED);
    let _rachel_sub = seed_active_subscription_for(&env, RACHEL_DID, RACHEL_ACTIVE_SUB_SEED);
    let indexer = seed_network_index_from_specs(&env, sf_corpus_all_four_arms());
    let viewer = ViewerServer::start_with_indexer(&env, indexer);

    // WHEN the results render in BOTH shapes — the read-only + neutral contract holds
    // across every shape.
    let path = format!("/search?object={SF_OBJECT_REPRODUCIBLE_BUILDS}");
    let full_page = viewer.get(&path);
    let fragment = viewer.get_htmx(&path);

    for (label, response) in [("full page", &full_page), ("htmx fragment", &fragment)] {
        assert_eq!(
            response.status, 200,
            "FF-8: GET /search ({label}) over the four-arm seeded index must be 200; body:\n{}",
            response.body
        );
        // THEN neither new indicator is an executable control (the slice-16 render-only
        // scan, which already forbids follow/unfollow/subscribe controls + mutating
        // hx-* — the two new arms must add none either).
        assert_search_follow_state_is_render_only(&response.body);
        // …and both new indicators are NEUTRAL, never pejorative (the C-9 blocklist).
        assert_search_follow_state_framing_is_neutral(&response.body);
    }
}

// =============================================================================
// US-FS-001 — Theme F: INDEPENDENT DEGRADE (C-8 / WD-FS-4). A failed cached-peer
// read degrades ONLY that arm. (FF-9 — RED scaffold via the per-read fault seam;
// OQ-1 escalation — see start_viewer_with_failing_cached_peer_read.)
// =============================================================================

/// FF-9 (US-FS-001 / Theme F; C-8 / WD-FS-4 / ADR-057 D4): a failed LOCAL
/// cached-peer (`distinct_cached_peer_author_dids`) read during a search render
/// degrades ONLY the `UnsubscribedCache` arm — a soft-removed peer falls through to
/// `NetworkUnfollowed` (his arm's slice-16 fallback) — while the `You` + the
/// `SubscribedPeer` arms STILL resolve from their (successful) own + active reads,
/// and the results STILL render with no crash, blank, leaked error, or 5xx. The
/// arm-failure must never break discovery (the enrichment's failure is swallowed
/// into the row's fallback, independently per read).
///
/// OQ-1 escalation (see `start_viewer_with_failing_cached_peer_read` + the
/// red-classification doc): the real-binary subprocess harness cannot inject a
/// per-read `Err` via a fake `StoreReadPort` (DESIGN's D-4 default bet), so this
/// scaffolds the TRUE cached-read-failure path with `todo!()` → RED for the RIGHT
/// reason (MISSING_FUNCTIONALITY) and the feature-delta records that DELIVER MUST
/// add a per-read cfg-gated fault token + extend the xtask VIEWER_FAIL_SEAM_TOKENS.
///
/// Given the operator's LOCAL cached-peer read FAILS during a search render (her own
///   and active reads succeed), with a reachable index holding her own claim, a
///   followed peer, and her soft-removed peer Tobias;
/// When she opens GET /search;
/// Then Tobias's row degrades to `openlore peer add` (his arm's slice-16 fallback);
///   her own claim STILL shows the self indicator and her followed peer STILL shows
///   "Following"; and the results still render with no crash, blank, leaked error,
///   or 5xx.
///
/// @us-fs-001 @driving_port @real-io @graceful-degrade @error @c-8 @wd-fs-4
#[test]
fn a_failed_cached_peer_read_degrades_only_that_arm_without_crashing() {
    // GIVEN the operator's own claim + a followed peer + a soft-removed-cached peer
    // all seeded, AND a reachable index where all three appear, BUT the LOCAL
    // cached-peer read is forced to FAIL mid-request (the own + active reads succeed).
    let env = TestEnv::initialized();
    seed_own_claim_for_search(&env);
    seed_cached_unsubscribed_peer_for(&env, TRAVERSAL_AUTHOR_TOBIAS, TOBIAS_CACHED_SEED);
    let _rachel_sub = seed_active_subscription_for(&env, RACHEL_DID, RACHEL_ACTIVE_SUB_SEED);
    let indexer = seed_network_index_from_specs(&env, sf_corpus_all_four_arms());
    // The per-read cached-peer fault seam (RED scaffold — `todo!()`): DELIVER
    // materializes the mid-request cached-peer-read failure (OQ-1 escalation). Until
    // then this panics → RED (MISSING_FUNCTIONALITY: the per-read degrade path + its
    // seam do not exist).
    let viewer = start_viewer_with_failing_cached_peer_read(&env, indexer);

    let response = viewer.get(&format!("/search?object={SF_OBJECT_REPRODUCIBLE_BUILDS}"));

    // THEN the search results STILL render (200, not a crash/5xx).
    assert_eq!(
        response.status, 200,
        "FF-9 (C-8): a failed cached-peer read must degrade ONLY that arm to a guided \
         200, NOT a crash/5xx; body:\n{}",
        response.body
    );
    assert!(
        !response.body.trim().is_empty(),
        "FF-9 (C-8): the degraded render must NOT be a blank region; body:\n{}",
        response.body
    );
    // …Tobias's row degrades to `openlore peer add` (his arm's slice-16 fallback —
    // the cached read failed so he is no longer recognized as cached residue).
    assert_search_row_offers_follow(&response.body, TRAVERSAL_AUTHOR_TOBIAS);
    // …BUT her own claim STILL shows the self indicator (the own read succeeded) and
    // her followed peer STILL shows "Following" (the active read succeeded) — the
    // degrade is INDEPENDENT per read (only the cached arm fell through).
    assert_search_row_shows_self_indicator(&response.body, SF_OWN_BARE_DID);
    assert_search_row_following(&response.body, RACHEL_DID);
    // …and the degraded render leaks NO transport/internal error.
    assert_search_html_leaks_no_transport_internals(&response.body);
}

// =============================================================================
// US-FS-002 — Theme G: htmx vs no-JS PARITY (C-10). The four arms render
// identically under fragment + full page. (FF-10)
// =============================================================================

/// FF-10 (US-FS-002 / Theme G; C-10 / WD-FS — parity by construction): the four
/// resolved follow-states render IDENTICALLY under the htmx `#search-results`
/// fragment and the no-JS full page. The resolution happens in the shell BEFORE the
/// render; both shapes consume the SAME SearchState, so the self indicator + the
/// residue indicator + the "Following" indicator + the `peer add` affordance appear
/// in BOTH shapes — parity by construction.
///
/// Given the operator's own claim and a soft-removed peer's cached claim both appear
///   in a search;
/// When she requests GET /search WITH HX-Request and again WITHOUT it;
/// Then the htmx fragment carries the self indicator + the residue indicator, and the
///   no-JS full page carries the SAME follow-states, rendered identically.
///
/// @us-fs-002 @driving_port @real-io @parity @c-10 @happy
#[test]
fn the_four_follow_states_render_identically_under_htmx_and_no_js() {
    // GIVEN the four-arm render (own claim, followed peer, soft-removed peer, new author).
    let env = TestEnv::initialized();
    seed_own_claim_for_search(&env);
    seed_cached_unsubscribed_peer_for(&env, TRAVERSAL_AUTHOR_TOBIAS, TOBIAS_CACHED_SEED);
    let _rachel_sub = seed_active_subscription_for(&env, RACHEL_DID, RACHEL_ACTIVE_SUB_SEED);
    let indexer = seed_network_index_from_specs(&env, sf_corpus_all_four_arms());
    let viewer = ViewerServer::start_with_indexer(&env, indexer);

    // WHEN both shapes of the SAME query are fetched.
    let path = format!("/search?object={SF_OBJECT_REPRODUCIBLE_BUILDS}");
    let fragment = viewer.get_htmx(&path);
    let full_page = viewer.get(&path);

    assert!(
        fragment.is_fragment(),
        "FF-10: the htmx shape must return ONLY the #search-results fragment; body:\n{}",
        fragment.body
    );
    assert!(
        full_page.is_full_page(),
        "FF-10: the no-JS shape must return the COMPLETE full page; body:\n{}",
        full_page.body
    );
    // THEN BOTH shapes carry the SAME resolved follow-states: the self indicator
    // (own) + the residue indicator (Tobias) appear in BOTH (RED today — parity by
    // construction once the two arms render). Rachel "Following" + Priya `peer add`
    // in both (byte-stable).
    for (label, body) in [("htmx fragment", &fragment.body), ("full page", &full_page.body)] {
        assert!(
            body.contains(SF_OWN_BARE_DID) && body.contains(TRAVERSAL_AUTHOR_TOBIAS),
            "FF-10: the {label} must attribute the own + cached author rows; body:\n{body}"
        );
        assert_search_row_shows_self_indicator(body, SF_OWN_BARE_DID);
        assert_search_row_shows_residue_indicator(body, TRAVERSAL_AUTHOR_TOBIAS);
        assert_search_row_following(body, RACHEL_DID);
        assert_search_row_offers_follow(body, PRIYA_DID);
    }
}
