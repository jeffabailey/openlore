//! serverless-philosophy-federation — WALKING SKELETON.
//!
//! Slice-01: deploy + register my OWN serverless instance (US-SF-001, J-007)
//! and round-trip ONE signed claim through it (US-SF-002, J-008), CID-verified.
//! This is the thinnest end-to-end that connects `openlore publish init` →
//! `publish push` → my Worker → `publish pull`, proving the load-bearing
//! guardrail KPI-SF-1 (round-trip CID integrity) on the OPAQUE transport
//! (ADR-062 / DDD-1).
//!
//! Drives the real `openlore` CLI as a subprocess via `assert_cmd`. Only the
//! NEW external boundary — the CLI↔Cloudflare-Worker seam — is faked, via
//! `openlore_test_support::FakeInstance` (an opaque content-addressed HTTP
//! double mirroring the ADR-062 contract: `PUT/GET /records/:cid`,
//! `GET /manifest`, `GET /`). Everything internal is REAL: `claim-domain`
//! (the SOLE canonicalizer — SPIKE-00 invariant), `lexicon`, the local
//! DuckDB store. The LIVE `wrangler dev`/workerd round-trip is NOT an
//! acceptance test — it is the DEVOPS CI contract test `publish-contract.yml`
//! (DV-1); this suite asserts the observable CLI contract against the double.
//!
//! Layer: walking-skeleton subprocess (layer 5) + subprocess/FS acceptance
//! (layer 3). Per Mandate 9 every scenario is example-only; per Mandate 11
//! sad paths are named examples, never PBT-generated.
//!
//! Covers:
//! - US-SF-001 register-own-instance (the deploy half is `wrangler deploy`,
//!   user-run — the test drives the `publish init` registration only)
//! - US-SF-002 round-trip one signed claim, CID-verified (KPI-SF-1)
//! - The `0.0`/`0.5`/`1.0` f16-representable confidence GOLD FIXTURE — the
//!   regression guard against the REJECTED re-encode `putRecord` transport
//!   (ADR-062 Alternatives; DESIGN handoff).
//
// SCAFFOLD: true

mod support;

#[allow(unused_imports)]
use support::*;

// =============================================================================
// WALKING SKELETON — US-SF-001 + US-SF-002 (NOT #[ignore]: must be GREEN to
// close DISTILL → the first slice DELIVER makes pass; RED at handoff)
// =============================================================================

/// WS-1: Maria has deployed the `atproto/` Worker to her own Cloudflare
/// account and registered it with `openlore publish init`. She has ONE signed
/// claim `bafy…m9pq` in her local DuckDB. When she runs `openlore publish push`
/// then `openlore publish pull`, the instance stores the claim VERBATIM under
/// its Rust-minted CID, the CLI recomputes the CID on pull and byte-matches the
/// key, and reports `1/1 CIDs verified`. Her local claim is unmodified
/// (additive, D-6). This is the demo-able user outcome: "my instance is a
/// faithful, verifiable mirror I own."
/// (US-SF-001 + US-SF-002 · AC round-trip identical CID · KPI-SF-1.)
///
/// @walking_skeleton @driving_port @real-io @us-sf-001 @us-sf-002 @j-007 @j-008 @kpi-sf-1
#[test]
fn publish_round_trips_one_signed_claim_through_my_own_instance_with_identical_cid() {
    use support::state_delta::{assert_state_delta, set_to, Delta};

    // Given Maria has deployed her own instance and registered it.
    let env = TestEnv::initialized();
    let instance = FakeInstance::fresh();
    let init = run_openlore_publish(&env, &["init", instance.endpoint_url()], &instance);
    assert_eq!(
        init.status, 0,
        "`publish init <url>` must register a reachable openlore instance;\n\
         --- stdout ---\n{}\n--- stderr ---\n{}",
        init.stdout, init.stderr
    );

    // And she has ONE signed claim in her local store.
    let cid = seed_one_signed_local_claim(&env, 0.86);
    let local_before = local_claim_cids(&env);
    let before = capture_publish_universe(&env, &instance, "");

    // When she pushes it to her instance and pulls it back.
    let push = run_openlore_publish(&env, &["push"], &instance);
    assert_eq!(
        push.status, 0,
        "`publish push` must exit 0;\n--- stdout ---\n{}\n--- stderr ---\n{}",
        push.stdout, push.stderr
    );
    let pull = run_openlore_publish(&env, &["pull"], &instance);
    assert_eq!(
        pull.status, 0,
        "`publish pull` must exit 0;\n--- stdout ---\n{}\n--- stderr ---\n{}",
        pull.stdout, pull.stderr
    );
    let after = capture_publish_universe(&env, &instance, &pull.stdout);

    // Then the instance gains exactly her claim's CID, the pull verifies 1 of
    // 1, and her local store is unchanged (implicit-unchanged slot).
    let expected = Delta::new()
        .with_slot(
            "instance.records.cids",
            set_to(format!("{:?}", vec![cid.clone()])),
        )
        .with_slot("cli.publish.verified_count", set_to("1".to_string()));
    assert_state_delta(&before, &after, &publish_universe(), &expected);

    // And the stored bytes recompute (in Rust) to that identical CID.
    assert_instance_stores_cid(&instance, &cid);
    assert!(
        pull.stdout.contains("1/1"),
        "pull must tell her the claim verified 1 of 1; stdout:\n{}",
        pull.stdout
    );
    assert_local_claims_unchanged(&env, &local_before);
}

// =============================================================================
// US-SF-002 — focused scenarios (all #[ignore]: DELIVER unskips one at a time)
// =============================================================================

/// RT-2: A boundary bug makes the round-trip recompute a DIFFERENT CID for a
/// pushed claim (canonicalization drift). `openlore publish push` rejects that
/// claim as a canonicalization mismatch and reports it; NOTHING is silently
/// stored on the instance (a mismatch is a rejected sync, not a silent accept —
/// KPI-SF-1). (US-SF-002 · AC CID mismatch rejects + reports · D-6.)
///
/// @us-sf-002 @error @kpi-sf-1 @j-008
#[test]
#[ignore = "DELIVER: unskip one-at-a-time (US-SF-002 canonicalization-drift rejection)"]
fn publish_push_rejects_a_claim_whose_cid_does_not_survive_the_boundary() {
    todo!(
        "DELIVER: Given a FakeInstance::with_cid_mismatch (recomputes a divergent CID); \
         When `publish push`; Then the claim is rejected as a canonicalization mismatch, \
         reported to the user, exit non-zero, and instance.records.cids stays EMPTY \
         (no silent store)."
    );
}

/// RT-3: Maria's instance is unreachable. `openlore publish push` reports the
/// instance is unreachable and exits non-zero; her local store is untouched;
/// and `openlore graph query` STILL works offline — authoring is never blocked
/// by an unreachable instance (KPI-SF-5, local-first preserved). (US-SF-002 ·
/// AC unreachable exits non-zero, local query unaffected.)
///
/// @us-sf-002 @error @kpi-sf-5 @j-008
#[test]
#[ignore = "DELIVER: unskip one-at-a-time (US-SF-002 unreachable-instance / offline-first)"]
fn publish_push_exits_nonzero_when_the_instance_is_unreachable_and_leaves_authoring_untouched() {
    todo!(
        "DELIVER: Given a FakeInstance::unreachable registered target; When `publish push`; \
         Then exit non-zero with an 'instance unreachable' message, local.claims.row_count \
         UNCHANGED; And a subsequent `graph query` succeeds offline (KPI-SF-5)."
    );
}

/// RT-4 — GOLD FIXTURE (float regression guard): a claim at confidence `0.0`,
/// one at `0.5`, and one at `1.0` — the three f16-representable values SPIKE-00
/// proved a re-encoding `putRecord` PDS would corrupt — each push→pull back with
/// an IDENTICAL recomputed CID over the opaque transport. This is the standing
/// guard against the REJECTED re-encode model creeping back (ADR-062
/// Alternatives + §3 + DESIGN handoff). (US-SF-002 · KPI-SF-1.)
///
/// @us-sf-002 @kpi-sf-1 @gold-fixture @float-guard @regression @j-008
#[test]
#[ignore = "DELIVER: unskip one-at-a-time (US-SF-002 0.0/0.5/1.0 float round-trip guard)"]
fn publish_round_trips_confidence_zero_half_one_each_cid_identical() {
    todo!(
        "DELIVER: For each confidence in [0.0, 0.5, 1.0] (gold_claims_confidence_0_half_1): \
         push the claim to a FakeInstance and pull it back; assert the recomputed CID is \
         byte-identical to the pushed CID for ALL THREE. This fences off the rejected \
         re-encode transport (SPIKE-00 / ADR-062)."
    );
}
