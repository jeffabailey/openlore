//! serverless-philosophy-federation — `openlore publish push` (bulk).
//!
//! Slice-02: US-SF-003 (push my WHOLE local graph to my instance — additive,
//! idempotent, per-claim CID-verified, J-008). Generalizes the WS one-claim
//! round-trip to the full graph: diff local-vs-instance by CID set, push only
//! new claims, verify each on pull-back, skip already-present claims, never
//! mutate local (D-6). The opaque transport's `PUT /records/:cid` is idempotent
//! (re-PUT is a no-op — ADR-062 §1), which is what makes re-push and
//! interrupted-resume duplicate-free.
//!
//! Also binds DV-4 write-auth (Q-SF-D2, DISTILL-owned detailed contract): the
//! WRITE path (`publish push`) is owner-authed via a per-instance bearer token;
//! reads stay public (asserted in `public_card.rs`).
//!
//! Only the CLI↔Worker seam is faked (`FakeInstance`). Layer 3; example-only
//! (Mandate 11 — the interrupted-resume sad path is a named example, not PBT).
//
// SCAFFOLD: true

mod support;

#[allow(unused_imports)]
use support::*;

// =============================================================================
// US-SF-003 — happy path
// =============================================================================

/// PP-1: Maria has 42 local claims and 1 already on her instance. When she runs
/// `openlore publish push`, 41 new claims are pushed and 1 is skipped; each
/// pushed claim's CID recomputed on pull-back matches its local CID (41/41
/// verified); no local claim is modified (additive, D-6). (US-SF-003 · AC 1-2-4
/// · UAT #1 · KPI-SF-1.)
///
/// @us-sf-003 @driving_port @real-io @j-008 @happy
#[test]
#[ignore = "DELIVER: unskip one-at-a-time (US-SF-003 bulk push additive + CID-verified)"]
fn publish_push_sends_only_new_claims_additive_and_cid_verified() {
    todo!(
        "DELIVER: Given local_graph_of(42) and a FakeInstance already holding 1 of them; \
         When `publish push`; Then 41 pushed + 1 skipped, 41/41 CIDs verified, \
         instance.records.cids grows by 41, and local.claims.row_count is UNCHANGED. \
         Universe: instance.records.cids (append 41), cli.publish.{{pushed,skipped,verified}}, \
         local.claims.row_count (unchanged)."
    );
}

// =============================================================================
// US-SF-003 — boundary / idempotency
// =============================================================================

/// PP-2: Maria has already pushed all 42 claims. When she re-runs
/// `openlore publish push`, 0 claims are pushed and 42 are skipped; no duplicate
/// records are created on the instance (content-addressed key; re-PUT is a
/// no-op). (US-SF-003 · AC 3 · UAT #2 · Domain example 2.)
///
/// @us-sf-003 @edge @j-008
#[test]
#[ignore = "DELIVER: unskip one-at-a-time (US-SF-003 idempotent re-push, no duplicates)"]
fn publish_push_is_idempotent_re_push_creates_no_duplicates() {
    todo!(
        "DELIVER: Given a FakeInstance already holding all 42; When `publish push` again; \
         Then 0 pushed + 42 skipped and instance.records.cids is UNCHANGED (no duplicate \
         records — PUT /records/:cid idempotent per ADR-062 §1)."
    );
}

/// PP-3: A push was interrupted after 20 of 41 new claims. When Maria re-runs
/// `openlore publish push`, the remaining 21 are pushed and no claim is
/// duplicated on the instance (additive + idempotent resume). (US-SF-003 · AC 3
/// · UAT #3 · Domain example 3.)
///
/// @us-sf-003 @error @j-008
#[test]
#[ignore = "DELIVER: unskip one-at-a-time (US-SF-003 interrupted push resumes without duplicates)"]
fn publish_push_resumes_after_interruption_without_duplicates() {
    todo!(
        "DELIVER: Given a FakeInstance holding 20 of 41 (a mid-push interruption); \
         When `publish push` again; Then the remaining 21 are pushed, instance.records.cids \
         ends at 41 distinct CIDs, and NO CID appears twice."
    );
}

/// PP-4: A push NEVER mutates the local store — the local DuckDB stays the
/// canonical source of truth (D-6). This is the load-bearing local-first
/// guarantee asserted directly on the local-claims universe. (US-SF-003 · AC 4.)
///
/// @us-sf-003 @j-008 @guardrail
#[test]
#[ignore = "DELIVER: unskip one-at-a-time (US-SF-003 push never mutates local store)"]
fn publish_push_never_mutates_the_local_store() {
    todo!(
        "DELIVER: Given local_graph_of(N); capture local.claims universe BEFORE; \
         When `publish push`; Then local.claims.{{row_count, cids, bytes}} are ALL UNCHANGED \
         (additive-only, D-6)."
    );
}

// =============================================================================
// US-SF-003 / US-SF-002 — write-auth (DV-4 / Q-SF-D2)
// =============================================================================

/// PP-5 (DV-4 write-auth): The write path is owner-authed. When the per-instance
/// bearer token is missing or wrong, `openlore publish push` is refused by the
/// instance and reports the write was unauthorized; nothing is stored. With the
/// correct owner token the push succeeds. Reads (records/manifest/card) require
/// no token (asserted in `public_card.rs`). (US-SF-002/003 · DV-4 · Q-SF-D2.)
///
/// @us-sf-003 @error @dv-4 @q-sf-d2 @j-008
#[test]
#[ignore = "DELIVER: unskip one-at-a-time (US-SF-003 write path requires owner token)"]
fn publish_push_requires_the_owner_write_token_and_is_refused_without_it() {
    todo!(
        "DELIVER: Given a FakeInstance::requiring_write_token(tok); \
         When `publish push` WITHOUT the token; Then exit non-zero, 'unauthorized write' \
         reported, instance.records.cids UNCHANGED; \
         And WITH the correct owner token the same push stores the records. Binds Q-SF-D2 \
         detailed write-auth contract; reads-public counterpart is public_card.rs PC-6."
    );
}
