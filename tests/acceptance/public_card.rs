//! serverless-philosophy-federation — public read-only philosophy card.
//!
//! Slice-04: US-SF-005 (public read-only philosophy card at a stable,
//! Bluesky-linkable URL, J-007; requirement 1). The user's own Worker serves a
//! read-only HTML card at `GET /` rendered from `/manifest`, showing ONLY
//! explicitly-pushed claims, each attributed to its author DID (anti-merging;
//! no consensus row). The card holds no signing key and offers no authoring
//! control — signing- AND write-incapable by construction (D-7, extended by
//! ADR-062 §6).
//!
//! Scope note (honest test boundary): the REAL card HTML render is the `atproto/`
//! Worker's (TS) responsibility and is CI-contract-tested against live workerd
//! by DEVOPS `publish-contract.yml` (DV-1). At THIS acceptance layer the card is
//! exercised through `FakeInstance`'s `GET /` render, which mirrors the ADR-062
//! §1 card contract — exactly as the sibling feature fakes the PDS boundary and
//! defers the real wire contract to DEVOPS. These scenarios assert the OBSERVABLE
//! card contract (only-pushed, per-author attribution, no consensus row,
//! read-only, reads-public); the data the card renders (the manifest DISPLAY
//! projection) is written by the real CLI on push.
//!
//! Layer 3; example-only.
//
// SCAFFOLD: true

mod support;

#[allow(unused_imports)]
use support::*;

// =============================================================================
// US-SF-005 — happy path
// =============================================================================

/// PC-1: Maria has pushed her claims to her instance. When anyone opens
/// `https://openlore.maria.workers.dev/`, the card renders Maria's claims each
/// attributed to `did:plc:maria-test`, offers no authoring or editing control,
/// and shows no claim as a merged or consensus entry. (US-SF-005 · AC 1-2-3 ·
/// UAT #1 · Domain example 1.)
///
/// @us-sf-005 @driving_port @real-io @j-007 @happy
#[test]
fn public_card_renders_only_pushed_claims_each_attributed_to_its_author_did() {
    // Given Maria has pushed her 3 local claims to her (fresh) instance
    // through the REAL `publish push` — the CLI writes the manifest display
    // projection the card renders from.
    let env = TestEnv::initialized_as(FakeIdentity::maria());
    let pushed_cids = local_graph_of(&env, 3);
    let instance = FakeInstance::fresh();
    let push = run_openlore_publish(&env, &["push"], &instance);
    assert_eq!(
        push.status, 0,
        "`publish push` must exit 0;\n--- stdout ---\n{}\n--- stderr ---\n{}",
        push.stdout, push.stderr
    );

    // When anyone opens the card (anonymous GET /).
    let card = open_public_card(&instance);

    // Then it lists exactly her pushed claims, each attributed to the
    // author_did its claim carries; it offers no authoring/edit control and
    // shows no merged/consensus row.
    assert_card_is_read_only_and_unmerged(&card);
    let mut rows = card.rows.clone();
    rows.sort();
    assert_eq!(
        rows,
        expected_card_rows(&env, &pushed_cids),
        "the card must list every pushed claim attributed to its author_did;\n{}",
        card.body
    );
    assert!(
        !card.shows_empty_state,
        "a card with published claims must not read '{CARD_EMPTY_STATE}'"
    );
}

// =============================================================================
// US-SF-005 — boundary (empty but valid)
// =============================================================================

/// PC-2: Before any push, the card renders "no claims published yet", not an
/// error — an empty-but-valid instance. (US-SF-005 · Domain example 2.)
///
/// @us-sf-005 @edge @j-007
#[test]
fn public_card_on_an_empty_instance_renders_no_claims_published_yet_not_an_error() {
    // Given an instance nothing has been pushed to yet (empty manifest).
    let instance = FakeInstance::fresh();

    // When anyone opens the card.
    let card = open_public_card(&instance);

    // Then it is a valid 200 read-only card reading "no claims published yet"
    // with zero rows — not an error page.
    assert_card_is_read_only_and_unmerged(&card);
    assert!(
        card.shows_empty_state,
        "an empty instance's card must read '{CARD_EMPTY_STATE}';\n{}",
        card.body
    );
    assert!(
        card.rows.is_empty(),
        "an empty instance's card must render zero rows; got {:?}",
        card.rows
    );
}

// =============================================================================
// US-SF-005 — guardrail (only explicitly-pushed claims)
// =============================================================================

/// PC-3: Maria has a local claim she has NOT pushed. When anyone opens her card,
/// the unpushed claim does not appear — the card renders ONLY explicitly-pushed
/// claims (D-7). (US-SF-005 · AC 1 · UAT #3 · Domain example — unpushed absent.)
///
/// @us-sf-005 @j-007 @guardrail
#[test]
fn public_card_omits_a_local_claim_that_was_never_pushed() {
    // Given Maria pushed her 2 local claims, then authored a third claim
    // locally that she has NEVER pushed.
    let env = TestEnv::initialized_as(FakeIdentity::maria());
    let pushed_cids = local_graph_of(&env, 2);
    let instance = FakeInstance::fresh();
    let push = run_openlore_publish(&env, &["push"], &instance);
    assert_eq!(
        push.status, 0,
        "`publish push` must exit 0;\n--- stdout ---\n{}\n--- stderr ---\n{}",
        push.stdout, push.stderr
    );
    let never_pushed = seed_one_signed_local_claim(&env, 0.5);
    assert!(
        local_claim_cids(&env).contains(&never_pushed),
        "precondition: the unpushed claim exists locally"
    );

    // When anyone opens her card.
    let card = open_public_card(&instance);

    // Then the unpushed claim is absent; the card renders ONLY the
    // explicitly-pushed claims (D-7).
    assert_card_is_read_only_and_unmerged(&card);
    assert!(
        card.rows.iter().all(|row| row.cid != never_pushed),
        "a never-pushed local claim ({never_pushed}) must not appear on the card;\n{}",
        card.body
    );
    let mut rows = card.rows.clone();
    rows.sort();
    assert_eq!(
        rows,
        expected_card_rows(&env, &pushed_cids),
        "the card must render exactly the explicitly-pushed claims;\n{}",
        card.body
    );
}

// =============================================================================
// US-SF-005 — error / read-only by construction
// =============================================================================

/// PC-4: A visitor tries to edit/counter from the card; the card offers no such
/// control and the instance surface exposes no write route reachable from the
/// read path — the card is read-only and holds no signing key, write-incapable
/// by construction (D-7 / ADR-062 §6 capability split). (US-SF-005 · AC 3 ·
/// Domain example 3.)
///
/// @us-sf-005 @error @d-7 @j-007
#[test]
#[ignore = "DELIVER: unskip one-at-a-time (US-SF-005 card is write-incapable by construction)"]
fn public_card_offers_no_authoring_or_write_control_and_holds_no_signing_key() {
    todo!(
        "DELIVER: Given a FakeInstance card surface; When inspecting GET / + the read surface; \
         Then no authoring/write control is present and the read surface exposes NO write \
         method (the write-capable PublishPort is wired ONLY in the publish composition root — \
         ADR-062 §6). Complements the xtask `publish_write_capability_isolated` arch guard."
    );
}

// =============================================================================
// US-SF-005 — anti-merging (KPI-SF-3)
// =============================================================================

/// PC-5: Two different authors pushed an identical claim. The card shows them as
/// TWO attributed rows, never one "consensus" row — anti-merging on the public
/// surface (carried from J-003). (US-SF-005 · KPI-SF-3 · anti-merging.)
///
/// @us-sf-005 @anti-merging @kpi-sf-3 @j-007
#[test]
#[ignore = "DELIVER: unskip one-at-a-time (US-SF-005 identical claim from two authors = two rows)"]
fn public_card_shows_two_authors_identical_claim_as_two_attributed_rows_never_one_consensus_row() {
    todo!(
        "DELIVER: Given identical claim content pushed under two distinct author_dids; \
         When GET /; Then card.rows contains TWO rows (one per author_did), merged_count == 0."
    );
}

// =============================================================================
// US-SF-005 — reads are public (DV-4 counterpart to publish_push PP-5)
// =============================================================================

/// PC-6 (DV-4 reads-public): The card, manifest, and record reads require NO
/// token — reads are public while writes are owner-authed (DV-4). Anyone
/// following Maria's Bluesky-profile link reaches the read-only card without
/// credentials. (US-SF-005 · DV-4 · AC 4 linkable URL.)
///
/// @us-sf-005 @dv-4 @j-007
#[test]
#[ignore = "DELIVER: unskip one-at-a-time (US-SF-005 reads public, no token)"]
fn public_card_and_manifest_reads_require_no_token() {
    todo!(
        "DELIVER: Given a FakeInstance::requiring_write_token(tok) (writes authed); \
         When GET / and GET /manifest WITHOUT any token; Then both return 200 (reads public), \
         confirming the DV-4 asymmetry: public reads, owner-authed writes (PP-5 counterpart)."
    );
}
