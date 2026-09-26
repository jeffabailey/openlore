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
#[ignore = "DELIVER: unskip one-at-a-time (US-SF-005 card renders attributed pushed claims)"]
fn public_card_renders_only_pushed_claims_each_attributed_to_its_author_did() {
    todo!(
        "DELIVER: Given a FakeInstance holding pushed claims; When GET / (the card); \
         Then the card lists each pushed claim attributed to its author_did, exposes NO \
         authoring/edit control, and contains NO consensus/merged row. \
         Universe: card.rows[*].author_did, card.controls.authoring (absent), card.rows.merged_count (0)."
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
#[ignore = "DELIVER: unskip one-at-a-time (US-SF-005 empty instance card is valid)"]
fn public_card_on_an_empty_instance_renders_no_claims_published_yet_not_an_error() {
    todo!(
        "DELIVER: Given a FakeInstance::fresh() (empty manifest); When GET /; \
         Then a 200 card reading 'no claims published yet', card.rows.len == 0, NOT an error page."
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
#[ignore = "DELIVER: unskip one-at-a-time (US-SF-005 card omits unpushed claim)"]
fn public_card_omits_a_local_claim_that_was_never_pushed() {
    todo!(
        "DELIVER: Given a local claim NEVER pushed + others pushed; When GET /; \
         Then the unpushed claim's CID is ABSENT from card.rows (only explicitly-pushed shown)."
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
