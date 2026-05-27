//! Slice-03 acceptance — `openlore graph query --federated [--subject <S>]`.
//!
//! The load-bearing surface for J-003a (anti-merging) per ADR-014
//! invariant I-FED-1 + KPI-FED-1 + KPI-FED-2. Every output row carries
//! its author DID; NO row represents a multi-author aggregate; the
//! footer states the no-merge guarantee verbatim.
//!
//! Covers:
//! - US-FED-003: federated query with per-author attribution (happy +
//!   edge + same-content-different-authors + author-only-default)
//! - WD-42: inline counter-claim template is shown by default in
//!   `--federated` output (resolved from `# DISTILL: confirm` habit
//!   scenario 2 inline-template trigger)
//! - WD-39: first-federated-query orientation fires once-per-user
//!   (resolved from `# DISTILL: confirm` habit scenario 1)
//! - Integration gate `federation_attribution_preserved` (mandatory;
//!   KPI-FED-1 + KPI-FED-2 release-gate)
//
// SCAFFOLD: true

mod support;

#[allow(unused_imports)]
use support::*;

// =============================================================================
// US-FED-003 — happy + edge paths
// =============================================================================

/// FQ-1: Maria has 1 of her own claims about
/// `github:rust-lang/cargo` + 2 pulled claims from
/// `did:plc:rachel-test` about the same subject. Running
/// `openlore graph query --subject github:rust-lang/cargo --federated`
/// returns exactly 3 rows grouped under 2 author headers:
/// `did:plc:test-maria (you)` (1 row) and
/// `did:plc:rachel-test (subscribed peer)` (2 rows). Every row carries
/// author_did + confidence + cid. The footer states the count of
/// distinct authors (2) AND the literal "Each claim is attributed to
/// its author DID. No claims are merged." Drives integration gate 1
/// (`federation_attribution_preserved`) + KPI-FED-1 + KPI-FED-2.
///
/// @us-fed-003 @real-io @driving_port @j-003a @kpi-fed-1 @kpi-fed-2 @happy
#[test]
fn federated_query_returns_author_and_peer_claims_grouped_by_author_did() {
    todo!("DELIVER (slice-03): wire VerbGraphQuery::federated branch → StoragePort.query_federated_by_subject (UNION ALL with explicit author_did projection per ADR-014 § Cross-store query examples) → renderer.group_by_author. Assert: exit 0, exactly 3 claim rows, two distinct author headers ('(you)' + '(subscribed peer)'), every row has author_did + confidence + cid, footer contains 'Each claim is attributed to its author DID. No claims are merged.' AND distinct-author-count = 2. Drives gate federation_attribution_preserved.")
}

/// FQ-2: Maria + Rachel publish two DIFFERENT claims with the SAME
/// (subject, predicate, object) triple but different confidence
/// values. The federated query renders BOTH as distinct rows under
/// their respective author headers. There is NO single "Both authors
/// agree" / "consensus" / "merged" row. (KPI-FED-2 zero-merge
/// guardrail; US-FED-003 AC 5 + Example 3.)
///
/// @us-fed-003 @real-io @driving_port @j-003a @kpi-fed-2 @anti-merging
#[test]
fn federated_query_renders_identical_content_from_different_authors_as_two_separate_rows() {
    todo!("DELIVER (slice-03): seed one author claim (Aanya) + pull one peer claim (Rachel) with the same (subject, predicate, object). Assert: exactly 2 rows in output, BOTH cids appear distinctly, NO occurrence of substrings 'merged' OR 'consensus' OR 'aggregate' in stdout, each row under its own author header. This is the load-bearing zero-merge gate (KPI-FED-2 release-blocking).")
}

/// FQ-3: `openlore graph query --subject <S>` WITHOUT `--federated`
/// behaves byte-identically to slice-01: shows ONLY the user's own
/// claims, footer announces "Use --federated to include N subscribed
/// peer(s)". This is the regression gate that ensures the new flag is
/// strictly opt-in and does NOT alter the default. (US-FED-003 AC 2 +
/// UAT scenario #3.)
///
/// @us-fed-003 @real-io @driving_port @j-003 @regression @default-off
#[test]
fn federated_query_default_without_flag_is_byte_identical_to_slice_01_behavior() {
    todo!("DELIVER (slice-03): assert running graph query WITHOUT --federated against a TestEnv with 1 own + 2 peer claims returns ONLY the 1 own claim + footer 'Use --federated to include 1 subscribed peer'; assert exit 0. Reference: WS-12 of slice-01 (regression).")
}

/// FQ-4: `--federated` requested with zero peer subscriptions degrades
/// gracefully: output shows ONLY the user's own claims; footer is
/// "No peers subscribed. Use `openlore peer add <did>` to follow a
/// peer's claim stream." (US-FED-003 AC 7 + UAT scenario #4.)
///
/// @us-fed-003 @real-io @driving_port @j-003 @edge
#[test]
fn federated_query_with_zero_peers_subscribed_degrades_with_hint() {
    todo!("DELIVER (slice-03): assert --federated against zero subscriptions returns own-only rows + footer literal 'No peers subscribed. Use `openlore peer add <did>` to follow a peer's claim stream.' + exit 0")
}

// =============================================================================
// US-FED-003 — counter-relationship annotation (bidirectional)
// =============================================================================

/// FQ-5: After Maria publishes a counter-claim (`bafy...new`) against
/// Rachel's `bafy...n4ka`, a subsequent federated query annotates BOTH
/// rows bidirectionally: Maria's row shows
/// "counters bafy...n4ka by did:plc:rachel-test"; Rachel's row shows
/// "countered-by bafy...new by did:plc:test-maria". The summary line
/// states the count of counter-relationships explicitly. (US-FED-004
/// AC 9 + US-FED-003 AC 8; chained narrative across counter +
/// federated query.)
///
/// @us-fed-003 @us-fed-004 @real-io @driving_port @j-003a @j-003b @happy
#[test]
fn federated_query_annotates_counter_relationships_bidirectionally() {
    todo!("DELIVER (slice-03): chained scenario — Maria pulls Rachel's records (state set up by reusing peer_pull step-method invocation), runs claim counter, then graph query --federated. Assert bidirectional annotations on both rows + summary line names the count. Uses StoragePort.query_federated_by_subject + peer_claim_references / claim_references join per data-models.md § Cross-store query examples.")
}

// =============================================================================
// US-FED-003 — habit-bridging affordances (resolve WD-39 + WD-42)
// =============================================================================

/// FQ-6 (WD-39 — RESOLVES `# DISTILL: confirm` habit scenario 1
/// first-federated-query trigger): The FIRST EVER
/// `openlore graph query --federated` invocation per install emits a
/// one-line orientation message verbatim:
/// "First federated query complete. Peer claims appear under their
/// author DIDs. No claims are merged. Use `openlore peer add <did>` to
/// follow more peers."
/// Subsequent invocations DO NOT emit the orientation. State lives in
/// `~/.config/openlore/identity.toml` under
/// `[federation] first_federated_query_completed_at`.
///
/// @us-fed-003 @real-io @driving_port @j-003 @habit @wd-39
#[test]
fn federated_query_first_invocation_emits_orientation_then_omits_on_subsequent_invocations() {
    todo!("DELIVER (slice-03): wire OrientationState.first_federated_query_completed_at check in VerbGraphQuery --federated branch; assert orientation present in first invocation stdout AND absent in second invocation stdout AND identity.toml gains the timestamp key after success")
}

/// FQ-7 (WD-42 — RESOLVES `# DISTILL: confirm` habit scenario 2 inline
/// template trigger): Per peer-claim row in `--federated` output, the
/// renderer includes a copy-pasteable counter template:
/// `openlore claim counter <peer_cid> --reason "..." --subject ...
/// --predicate ... --object ... --evidence ... --confidence ...`
/// The template pre-fills subject + predicate + object from the target
/// claim; the user fills in --reason + --evidence + --confidence.
/// Shown by DEFAULT (WD-42; NOT gated behind `--verbose`).
///
/// @us-fed-003 @real-io @driving_port @j-003b @habit @wd-42
#[test]
fn federated_query_renders_inline_counter_template_per_peer_row_by_default() {
    todo!("DELIVER (slice-03): pull one peer claim; run graph query --federated; assert stdout contains the literal 'openlore claim counter <peer_cid>' line per peer row with subject/predicate/object pre-filled from the target claim. WD-42 LOCKS this on by default; assert template appears WITHOUT --verbose. Habit-bridging affordance for KPI-FED-3.")
}

// =============================================================================
// US-FED-003 — KPI-FED-2 standalone gate (the zero-merged-rows guardrail)
// =============================================================================

/// FQ-8 (KPI-FED-2 release gate): Across a multi-author multi-record
/// fixture (Maria 1 own + Rachel 3 peer + Tobias 2 peer), every output
/// row from `graph query --federated` MUST have a distinct
/// (author_did, claim_cid) tuple. NO row is labeled "merged" /
/// "consensus" / "aggregate". The number of rows equals the sum of
/// per-author claim counts. Drives integration gate 1
/// (`federation_attribution_preserved`) and the KPI-FED-2 release
/// blocker per outcome-kpis.md.
///
/// @us-fed-003 @real-io @driving_port @j-003a @kpi-fed-1 @kpi-fed-2 @release-gate
#[test]
fn federated_query_no_merged_rows_across_multi_author_multi_record_fixture() {
    todo!("DELIVER (slice-03): seed 1 own + 3 peer-Rachel + 2 peer-Tobias claims about same subject; assert exactly 6 output rows + 6 distinct (author_did, cid) tuples + 3 distinct author headers + zero substring 'merged' / 'consensus' / 'aggregate' in stdout. Mandatory release-blocking gate per KPI-FED-2 + outcome-kpis.md alerting threshold.")
}
