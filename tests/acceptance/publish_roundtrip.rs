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
// US-SF-002 — focused scenarios (activated one at a time in DELIVER)
// =============================================================================

/// RT-2: A boundary bug makes the round-trip recompute a DIFFERENT CID for a
/// pushed claim (canonicalization drift). `openlore publish push` rejects that
/// claim as a canonicalization mismatch and reports it; NOTHING is silently
/// stored on the instance (a mismatch is a rejected sync, not a silent accept —
/// KPI-SF-1). (US-SF-002 · AC CID mismatch rejects + reports · D-6.)
///
/// "Nothing is silently stored" is observed through the instance's PORT-EXPOSED
/// record set: `instance.records.cids` = the CIDs COMMITTED to its manifest
/// (`FakeInstance::stored_cids()`). The opaque store has no DELETE (ADR-062),
/// so push is verify-BEFORE-commit: the blob is staged (`PUT` bytes), read
/// back, recomputed in Rust, and the manifest entry — the commit — is appended
/// only on a match. A staged-but-uncommitted blob is invisible to every reader
/// (pull walks the manifest), so "the instance does not retain the mismatched
/// record" means its CID never reaches the manifest.
///
/// @us-sf-002 @error @kpi-sf-1 @j-008
#[test]
fn publish_push_rejects_a_claim_whose_cid_does_not_survive_the_boundary() {
    use support::state_delta::{assert_state_delta, Delta};

    // Given Maria registered an instance whose boundary re-encodes the
    // record's confidence lossily (canonicalization drift).
    let env = TestEnv::initialized();
    let instance = FakeInstance::with_cid_mismatch();
    let init = run_openlore_publish(&env, &["init", instance.endpoint_url()], &instance);
    assert_eq!(
        init.status, 0,
        "`publish init <url>` must register the (reachable, marked) instance;\n\
         --- stdout ---\n{}\n--- stderr ---\n{}",
        init.stdout, init.stderr
    );

    // And she holds ONE signed claim whose confidence does not survive that
    // re-encode.
    let cid = seed_one_signed_local_claim(&env, 0.86);
    let local_before = local_claim_cids(&env);
    let before = capture_publish_universe(&env, &instance, "");

    // When she pushes it.
    let push = run_openlore_publish(&env, &["push"], &instance);
    let after = capture_publish_universe(&env, &instance, "");

    // Then the push fails loudly: non-zero exit, and the claim is named as
    // rejected for a canonicalization mismatch.
    assert_ne!(
        push.status, 0,
        "a push whose CID does not survive the boundary must exit non-zero;\n\
         --- stdout ---\n{}\n--- stderr ---\n{}",
        push.stdout, push.stderr
    );
    assert!(
        push.stdout
            .lines()
            .any(|line| line.contains(&cid) && line.contains("canonicalization mismatch")),
        "push must report {cid} as a canonicalization mismatch;\n--- stdout ---\n{}",
        push.stdout
    );
    assert!(
        push.stdout.contains("0/1 claims pushed"),
        "push must not count the rejected claim as pushed;\n--- stdout ---\n{}",
        push.stdout
    );

    // And NOTHING changed in the universe: the instance committed no record
    // (instance.records.cids stays EMPTY) and her local store is untouched.
    assert_state_delta(&before, &after, &publish_universe(), &Delta::new());
    assert!(
        instance.stored_cids().is_empty(),
        "the mismatched record must never be committed; got {:?}",
        instance.stored_cids()
    );
    assert_local_claims_unchanged(&env, &local_before);
}

/// RT-3: Maria's instance is unreachable. `openlore publish push` reports the
/// instance is unreachable and exits non-zero; her local store is untouched;
/// and `openlore graph query` STILL works offline — authoring is never blocked
/// by an unreachable instance (KPI-SF-5, local-first preserved). (US-SF-002 ·
/// AC unreachable exits non-zero, local query unaffected.)
///
/// The unreachable target is resolved through the CLI's documented
/// publish-target fallback seam (`OPENLORE_PUBLISH_ENDPOINT`, the same
/// `resolve_target` path a registered target takes) aimed at
/// `FakeInstance::unreachable()` — a loopback port bound then released, so the
/// connect is REFUSED deterministically. Registering a live double and then
/// dropping it was rejected as flaky: the double's runtime shuts down on a
/// background thread, so a lingering listener could accept-and-stall instead
/// of refusing. The offline `graph query` runs with EVERY network seam
/// omitted (`run_openlore_network_disabled` — no PDS, no peer, no instance).
///
/// @us-sf-002 @error @kpi-sf-5 @j-008
#[test]
fn publish_push_exits_nonzero_when_the_instance_is_unreachable_and_leaves_authoring_untouched() {
    use support::state_delta::{assert_state_delta, Delta};

    // Given Maria has ONE signed claim in her local store.
    let env = TestEnv::initialized();
    let cid = seed_one_signed_local_claim(&env, 0.86);
    let local_before = local_claim_cids(&env);

    // And her publish target is unreachable.
    let instance = FakeInstance::unreachable();
    let before = capture_publish_universe(&env, &instance, "");

    // When she pushes.
    let push = run_openlore_publish(&env, &["push"], &instance);
    let after = capture_publish_universe(&env, &instance, "");

    // Then the push fails loudly, naming the instance as unreachable.
    assert_ne!(
        push.status, 0,
        "a push to an unreachable instance must exit non-zero;\n\
         --- stdout ---\n{}\n--- stderr ---\n{}",
        push.stdout, push.stderr
    );
    assert!(
        push.stderr.contains("instance unreachable"),
        "push must say the instance is unreachable;\n--- stdout ---\n{}\n--- stderr ---\n{}",
        push.stdout,
        push.stderr
    );

    // And nothing changed: local.claims.row_count UNCHANGED, no record anywhere.
    assert_state_delta(&before, &after, &publish_universe(), &Delta::new());
    assert_local_claims_unchanged(&env, &local_before);

    // And authoring is not blocked: a local `graph query` still succeeds with
    // the network unavailable and still finds her claim (KPI-SF-5).
    let query = run_openlore_network_disabled(
        &env,
        &[
            "graph",
            "query",
            "--object",
            "org.openlore.philosophy.memory-safety",
        ],
    );
    assert_eq!(
        query.status, 0,
        "`graph query` must succeed offline after a failed push;\n\
         --- stdout ---\n{}\n--- stderr ---\n{}",
        query.stdout, query.stderr
    );
    assert!(
        query.stdout.contains("github:rust-lang/rust"),
        "offline `graph query` must still list her claim ({cid});\n--- stdout ---\n{}",
        query.stdout
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
fn publish_round_trips_confidence_zero_half_one_each_cid_identical() {
    // Given Maria has registered her own instance.
    let env = TestEnv::initialized();
    let instance = FakeInstance::fresh();
    let init = run_openlore_publish(&env, &["init", instance.endpoint_url()], &instance);
    assert_eq!(
        init.status, 0,
        "`publish init <url>` must register the instance;\n--- stdout ---\n{}\n--- stderr ---\n{}",
        init.stdout, init.stderr
    );

    // And she holds one signed claim at EACH gold confidence 0.0 / 0.5 / 1.0.
    let gold_cids: Vec<String> = gold_claims_confidence_0_half_1()
        .into_iter()
        .map(|confidence| seed_one_signed_local_claim(&env, confidence))
        .collect();
    let local_before = local_claim_cids(&env);

    // When she pushes them to her instance and pulls them back.
    let push = run_openlore_publish(&env, &["push"], &instance);
    assert_eq!(
        push.status, 0,
        "`publish push` must exit 0;\n--- stdout ---\n{}\n--- stderr ---\n{}",
        push.stdout, push.stderr
    );
    let pull = run_openlore_publish(&env, &["pull"], &instance);
    assert_eq!(
        pull.status, 0,
        "`publish pull` must exit 0 (every CID verified);\n--- stdout ---\n{}\n--- stderr ---\n{}",
        pull.stdout, pull.stderr
    );

    // Then ALL THREE recompute (in Rust) to the byte-identical pushed CID.
    let mut committed = instance.stored_cids();
    committed.sort();
    let mut expected = gold_cids.clone();
    expected.sort();
    assert_eq!(
        committed, expected,
        "the instance must commit exactly the three gold-fixture CIDs"
    );
    gold_cids
        .iter()
        .for_each(|cid| assert_instance_stores_cid(&instance, cid));
    assert_eq!(
        publish_verified_count(&pull.stdout),
        3,
        "pull must verify 3 of 3 gold-fixture CIDs; stdout:\n{}",
        pull.stdout
    );
    assert_local_claims_unchanged(&env, &local_before);
}
