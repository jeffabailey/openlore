//! serverless-philosophy-federation — `openlore publish pull` (reconcile).
//!
//! Slice-03: US-SF-004 (pull my instance back into local DuckDB — reconcile /
//! rebuild, J-008; requirement 2). Pull is an ADDITIVE reconcile: an in-sync
//! record is a no-op; a genuine CID conflict is SURFACED, never silently
//! overwritten; on an empty local store it reconstructs DuckDB with each record
//! verified before insert (D-6). Round-trip integrity (KPI-SF-1) holds: a claim
//! pushed then pulled here recomputes to an identical CID.
//!
//! Reconcile reuses the local store's existing never-silently-mutate discipline.
//! Only the CLI↔Worker seam is faked (`FakeInstance`); the real `adapter-duckdb`
//! local store does the reconcile. Layer 3; example-only (Mandate 11 — the
//! conflict sad path is a named example).
//
// SCAFFOLD: true

mod support;

#[allow(unused_imports)]
use support::*;

// =============================================================================
// US-SF-004 — happy path (in-sync no-op)
// =============================================================================

/// PR-1: Maria's instance holds the 42 claims she pushed and her local store
/// holds the same 42. When she runs `openlore publish pull`, every pulled
/// record's CID matches the local CID, no local claim is overwritten, and the
/// CLI reports the stores are in sync. (US-SF-004 · AC 1 · UAT #1 · Domain
/// example 1.)
///
/// @us-sf-004 @driving_port @real-io @j-008 @happy
#[test]
#[ignore = "DELIVER: unskip one-at-a-time (US-SF-004 in-sync reconcile is a no-op)"]
fn publish_pull_is_an_additive_reconcile_that_never_overwrites_local_claims() {
    todo!(
        "DELIVER: Given instance and local both holding the same 42 claims; When `publish pull`; \
         Then 42/42 CIDs match, 0 rows overwritten, 'in sync' reported. \
         Universe: local.claims.cids (unchanged), cli.publish_pull.{{matched,overwritten}}."
    );
}

// =============================================================================
// US-SF-004 — boundary (fresh-machine rebuild)
// =============================================================================

/// PR-2: Maria is on a fresh machine with an EMPTY local store and her instance
/// holds 42 claims. When she runs `openlore publish pull`, all 42 claims are
/// reconstructed into local DuckDB with attribution intact, and each record's
/// CID is verified before it is stored. (US-SF-004 · AC 2 · UAT #2 · Domain
/// example 2.)
///
/// @us-sf-004 @real-io @j-008 @boundary
#[test]
#[ignore = "DELIVER: unskip one-at-a-time (US-SF-004 fresh-machine rebuild)"]
fn publish_pull_rebuilds_local_duckdb_on_a_fresh_machine_with_attribution_intact() {
    todo!(
        "DELIVER: Given an empty local store and a FakeInstance holding 42; When `publish pull`; \
         Then local.claims.row_count 0 → 42, each attributed to its author_did, and each CID \
         recomputed + verified BEFORE insert."
    );
}

// =============================================================================
// US-SF-004 — error (conflict surfaced, not overwritten)
// =============================================================================

/// PR-3: A pulled record's CID differs from a local claim's CID for the same
/// logical record (a genuine conflict). When Maria runs `openlore publish pull`,
/// the CLI SURFACES the conflict and does NOT silently overwrite the local
/// claim — the no-silent-overwrite guardrail (D-6). (US-SF-004 · AC 3 · UAT #3
/// · Domain example 3.)
///
/// @us-sf-004 @error @j-008
#[test]
#[ignore = "DELIVER: unskip one-at-a-time (US-SF-004 conflict surfaced, not overwritten)"]
fn publish_pull_surfaces_a_conflict_instead_of_silently_overwriting() {
    todo!(
        "DELIVER: Given a pulled record whose CID differs from a local claim's for the same \
         logical record; When `publish pull`; Then the conflict is surfaced (reported, exit \
         non-zero), the local claim is UNCHANGED, and nothing is auto-resolved."
    );
}

/// PR-4: Round-trip integrity holds on pull — a claim pushed (US-SF-003) and
/// pulled here recomputes to an IDENTICAL CID over the opaque transport
/// (KPI-SF-1). The Rust core is the sole canonicalizer; pull re-parses,
/// re-canonicalizes, recomputes, and byte-matches the key (ADR-062 §3).
/// (US-SF-004 · AC 4 · KPI-SF-1.)
///
/// @us-sf-004 @kpi-sf-1 @j-008
#[test]
#[ignore = "DELIVER: unskip one-at-a-time (US-SF-004 pull recomputes identical CID)"]
fn publish_pull_recomputes_an_identical_cid_for_every_pushed_claim() {
    todo!(
        "DELIVER: Given a FakeInstance holding pushed claims (incl. the 0.0/0.5/1.0 gold \
         values); When `publish pull`; Then every recomputed CID byte-matches the stored key."
    );
}

// =============================================================================
// US-SF-004 — boundary (unreachable instance)
// =============================================================================

/// PR-5: Maria's instance is unreachable. When she runs `openlore publish pull`,
/// the CLI reports the instance is unreachable and exits non-zero; her local
/// store is untouched and local query still works offline (KPI-SF-5).
/// (US-SF-004 · local-first preserved.)
///
/// @us-sf-004 @error @kpi-sf-5 @j-008
#[test]
#[ignore = "DELIVER: unskip one-at-a-time (US-SF-004 unreachable instance on pull)"]
fn publish_pull_exits_nonzero_when_the_instance_is_unreachable() {
    todo!(
        "DELIVER: Given a FakeInstance::unreachable() registered target; When `publish pull`; \
         Then exit non-zero with 'instance unreachable', local.claims UNCHANGED, and a \
         subsequent `graph query` succeeds offline."
    );
}
