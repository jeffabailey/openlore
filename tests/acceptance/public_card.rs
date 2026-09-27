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
fn public_card_offers_no_authoring_or_write_control_and_holds_no_signing_key() {
    // Given Maria has pushed her claims to her instance through the REAL
    // `publish push`.
    let env = TestEnv::initialized_as(FakeIdentity::maria());
    local_graph_of(&env, 2);
    let instance = FakeInstance::fresh();
    let push = run_openlore_publish(&env, &["push"], &instance);
    assert_eq!(
        push.status, 0,
        "`publish push` must exit 0;\n--- stdout ---\n{}\n--- stderr ---\n{}",
        push.stdout, push.stderr
    );

    // When a visitor opens the card looking for a way to edit or counter.
    let card = open_public_card(&instance);

    // Then the card offers no authoring/edit control of any kind, and its
    // content-security-policy forbids it from submitting anywhere — there is
    // no write affordance reachable from the read surface.
    assert_card_is_read_only_and_unmerged(&card);
    assert!(
        card.content_security_policy.contains("form-action 'none'"),
        "the card must forbid every form submission (CSP form-action 'none'); got {:?}",
        card.content_security_policy
    );

    // And nothing the instance serves to a visitor (card or manifest) holds
    // the owner's signing key — nor did the push ever send it (D-7).
    assert_no_key_material_served(&instance, &env.identity);
    assert_no_key_material_sent(&instance, &env.identity);
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
fn public_card_shows_two_authors_identical_claim_as_two_attributed_rows_never_one_consensus_row() {
    // Given Maria and Jeff each authored the IDENTICAL claim — same subject,
    // same philosophy (one whose very name says "consensus"), same evidence,
    // same confidence — under their own DIDs, and both pushed it to the same
    // instance through the REAL `publish push`.
    let maria = TestEnv::initialized_as(FakeIdentity::maria());
    let jeff = TestEnv::initialized_as(FakeIdentity::jeff());
    let subject = "github:ietf/rfc7282";
    let object = "org.openlore.philosophy.rough-consensus";
    let maria_cid = seed_local_claim_about(&maria, subject, object);
    let jeff_cid = seed_local_claim_about(&jeff, subject, object);
    let instance = FakeInstance::fresh();
    for (author, env) in [("maria", &maria), ("jeff", &jeff)] {
        let push = run_openlore_publish(env, &["push"], &instance);
        assert_eq!(
            push.status, 0,
            "{author}'s `publish push` must exit 0;\n--- stdout ---\n{}\n--- stderr ---\n{}",
            push.stdout, push.stderr
        );
    }

    // When anyone opens the card.
    let card = open_public_card(&instance);

    // Then the identical claim shows as TWO rows, each attributed to its own
    // author DID — never collapsed into one consensus row (KPI-SF-3).
    assert_card_is_read_only_and_unmerged(&card);
    let mut rows = card.rows.clone();
    rows.sort();
    let mut expected = [
        expected_card_rows(&maria, &[maria_cid]),
        expected_card_rows(&jeff, &[jeff_cid]),
    ]
    .concat();
    expected.sort();
    assert_eq!(
        rows, expected,
        "two authors' identical claim must render as two attributed rows;\n{}",
        card.body
    );
    assert_ne!(
        rows[0].author_did, rows[1].author_did,
        "the two rows must carry two distinct author DIDs"
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
fn public_card_and_manifest_reads_require_no_token() {
    // Given Maria's instance demands her owner write token on every write,
    // and she has pushed a claim to it with that token.
    let owner_token = "maria-owner-write-token-9f3c";
    let env = TestEnv::initialized_as(FakeIdentity::maria());
    let pushed_cids = local_graph_of(&env, 1);
    let instance = FakeInstance::requiring_write_token(owner_token);
    let push = run_openlore_publish_with_token(&env, &["push"], &instance, Some(owner_token));
    assert_eq!(
        push.status, 0,
        "an owner-authed `publish push` must exit 0;\n--- stdout ---\n{}\n--- stderr ---\n{}",
        push.stdout, push.stderr
    );

    // When anyone following her Bluesky link opens the card and the manifest
    // WITHOUT any token.
    let card = open_public_card(&instance);
    let (manifest_status, manifest_body) = anonymous_get(&instance, "/manifest");

    // Then both reads succeed — reads are public — and the card shows her
    // pushed claim.
    assert_card_is_read_only_and_unmerged(&card);
    assert_eq!(
        card.rows,
        expected_card_rows(&env, &pushed_cids),
        "the token-less card must show the pushed claim;\n{}",
        card.body
    );
    assert_eq!(
        manifest_status, 200,
        "a token-less GET /manifest must be 200 (reads public, DV-4);\n{manifest_body}"
    );
    assert!(
        manifest_body.contains(&pushed_cids[0]),
        "the token-less manifest must list the pushed claim;\n{manifest_body}"
    );

    // And the asymmetry holds: the same token-less visitor cannot write.
    assert_eq!(
        anonymous_write_status(&instance, "bafyvisitorwrite"),
        401,
        "a token-less write must be refused (writes owner-authed, DV-4)"
    );
}
