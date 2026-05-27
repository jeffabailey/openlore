//! Slice-03 acceptance — `openlore claim counter <target_cid> --reason "..."` verb.
//!
//! The counter-claim sugar verb (WD-17 + ADR-013): constructs an
//! unsigned claim with `references[].type == Counters` pointing at
//! `<target_cid>` + `reason: Some(<text>)` then threads it through the
//! slice-01 `VerbClaimPublish` pipeline unchanged (WD-22 +
//! single-publish-path invariant per ADR-003 / I-FED-5).
//!
//! Covers:
//! - US-FED-004: author + publish a counter-claim (happy path + 4
//!   sad/edge paths)
//! - WD-20: `--reason` is REQUIRED on counter-claims (1..=1000 chars)
//! - WD-34: self-counter rejected in pure-core BEFORE compose preview
//! - WD-35: `--reason` is NFC-normalized before sign (idempotency
//!   property; ADR-015)
//! - WD-43: first-counter-claim framing block fires EXACTLY ONCE
//!   (resolved from `# DISTILL: confirm` habit scenario 2)
//! - WD-44: publish-time no auto-notification to target peer (resolved
//!   from `# DISTILL: confirm` anxiety scenario 4)
//
// SCAFFOLD: true

mod support;

#[allow(unused_imports)]
use support::*;

// =============================================================================
// US-FED-004 — happy path
// =============================================================================

/// CC-1: `openlore claim counter <peer_cid> --reason "..." [claim flags]`
/// renders a compose preview containing BOTH "not as truth" (inherited
/// from slice-01 / I-7) AND "counter-claims coexist, never overwrite"
/// (slice-03 content-frozen literal) AND
/// "counters: <peer_cid> (by <peer_did>)" AND the --reason text
/// verbatim wrapped at 78 cols. On Enter, the claim is signed via the
/// slice-01 pipeline; on Y, published. The counter-claim ends up in
/// `author_claims` (NOT `peer_claims`) — it is the user's own published
/// artifact. Subsequent federated query annotates Maria's row with
/// "counters <peer_cid> ..." AND Rachel's row with "countered-by ...".
/// (US-FED-004 AC 1-9 + UAT scenario #5; integration gates 1 + 3;
/// KPI-FED-3 + KPI-FED-1 + KPI-FED-2.)
///
/// @us-fed-004 @real-io @driving_port @j-003b @j-001 @kpi-fed-3 @happy
#[test]
fn counter_claim_compose_signs_and_publishes_via_slice_01_pipeline_with_required_framing() {
    todo!("DELIVER (slice-03): wire VerbClaimCounter → claim_domain::normalize_reason + validate_counter_claim → TtyIO compose preview (assert literals 'not as truth', 'counter-claims coexist, never overwrite', 'counters: <cid> (by <peer_did>)', and the --reason text verbatim) → on Enter call SAME canonicalize/compute_cid/sign as VerbClaimAdd → on Y call VerbClaimPublish internals (single-publish-path; NOT a parallel code path; ADR-003 + I-FED-5). Assert: counter-claim file at claims/<cid>.json, peer_claims UNCHANGED (the counter is the user's OWN), subsequent graph query --federated annotates both rows with bidirectional counters / countered-by. Drives integration gate 3 (counter_target_cid_round_trip).")
}

// =============================================================================
// US-FED-004 — sad / edge paths
// =============================================================================

/// CC-2 / Sad (WD-20): `openlore claim counter <peer_cid>` invoked
/// WITHOUT `--reason` (other claim flags valid) exits non-zero
/// pre-compose with the error message "counter-claims require
/// --reason; explain your disagreement". NO file is written. NO
/// network call is made. (US-FED-004 AC 2 + UAT scenario #2.)
///
/// @us-fed-004 @real-io @driving_port @j-003b @error @wd-20
#[test]
fn counter_claim_rejects_missing_reason_pre_compose() {
    todo!("DELIVER (slice-03): wire VerbClaimCounter → reject if --reason is None/empty BEFORE any compose preview / pure-core call. Assert: exit nonzero, stderr literal 'counter-claims require --reason; explain your disagreement', zero files under claims_dir, zero PDS calls (assert_no_pds_call_was_made)")
}

/// CC-3 / Edge (WD-20): `--reason` longer than 1000 chars is rejected
/// pre-compose with an error naming the upper bound (1..=1000 per WD-20
/// + ADR-015 minLength/maxLength on the Lexicon `reason` field).
///
/// @us-fed-004 @real-io @driving_port @j-003b @error @wd-20
#[test]
fn counter_claim_rejects_reason_exceeding_one_thousand_chars() {
    todo!("DELIVER (slice-03): wire VerbClaimCounter → length validation in claim_domain::validate_counter_claim (or pre-cli arg validator). Assert: exit nonzero, stderr names '1000' upper bound, no file written")
}

/// CC-4 / Sad (WD-34): Countering one's OWN claim is rejected
/// pre-compose with the error "cannot counter your own claim" AND a
/// hint to use `openlore claim retract <cid>` instead. The check
/// resolves via `claim_domain::validate_counter_claim` against EITHER
/// `claims` OR `peer_claims` (the target may be in either store; cli
/// hands in a `&dyn ClaimLookup`). No file is written. (US-FED-004
/// AC 6 + UAT scenario #3; Example 2.)
///
/// @us-fed-004 @real-io @driving_port @j-003b @error @wd-34
#[test]
fn counter_claim_rejects_self_counter_with_retract_hint() {
    todo!("DELIVER (slice-03): wire validate_counter_claim → if lookup(target_cid).author_did == current_user_did → return ClaimError::SelfCounter. Seed an own-claim via VerbClaimAdd first to give the lookup a target, then invoke 'claim counter <own_cid> --reason ...'. Assert: exit nonzero, stderr contains 'cannot counter your own claim' AND 'openlore claim retract', zero new files under claims_dir beyond the seeded own-claim")
}

// =============================================================================
// US-FED-004 — orientation + non-notification (resolves WD-43 + WD-44)
// =============================================================================

/// CC-5 (WD-43): The FIRST EVER `claim counter` invocation per install
/// renders a one-time framing block ("First counter-claim! Some
/// context:" + 4 enumerated points per gherkin-scenarios-expanded.md
/// habit scenario 2) BEFORE the compose preview. Subsequent
/// invocations DO NOT render the framing block. State lives in
/// `~/.config/openlore/identity.toml` under
/// `[federation] first_counter_claim_completed_at`.
/// Resolves `# DISTILL: confirm` flag (habit scenario 2 framing-block
/// trigger; WD-43 LOCKS once-per-user, NOT first-3-times).
///
/// @us-fed-004 @real-io @driving_port @j-003b @habit @wd-43
#[test]
fn counter_claim_first_invocation_renders_one_time_framing_block_then_omits_on_subsequent_invocations() {
    todo!("DELIVER (slice-03): wire OrientationState.first_counter_claim_completed_at check in VerbClaimCounter; assert framing block present in first invocation stdout AND absent in second invocation stdout AND identity.toml gains the timestamp key after success. Confirms WD-43 once-per-user (not first-3-times) lock.")
}

/// CC-6 (WD-44 — RESOLVES `# DISTILL: confirm` anxiety scenario 4):
/// Publishing a counter-claim against a peer's claim does NOT trigger
/// any network call to the peer's PDS beyond the user's normal
/// own-PDS publish. The peer learns about the counter-claim only when
/// they later pull from the current user (if they subscribe back).
/// Slice-03 ships NO notification mechanism in either direction.
///
/// @us-fed-004 @real-io @driving_port @j-003b @wd-44
#[test]
fn counter_claim_publish_does_not_auto_notify_target_peer_pds() {
    todo!("DELIVER (slice-03): construct TestEnv with FakePeerPds AND FakePds (user's own). Pull Rachel's records into peer_claims; counter one of them; assert (a) FakePds (user's own PDS) received exactly one create_record call for the counter-claim, (b) FakePeerPds received ZERO writes (only the listRecords / getRecord reads from the prior pull), (c) no notification XRPC method was called against the peer's endpoint")
}
