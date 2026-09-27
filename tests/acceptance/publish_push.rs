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
fn publish_push_sends_only_new_claims_additive_and_cid_verified() {
    use support::state_delta::{assert_state_delta, set_to, Delta};

    // Given Maria has 42 signed claims in her local graph, and her instance
    // already holds 1 of them (a prior push).
    let env = TestEnv::initialized_as(FakeIdentity::maria());
    let local_cids = local_graph_of(&env, 42);
    let already_pushed = local_cids[17].clone();
    let instance = instance_already_holding(&env, std::slice::from_ref(&already_pushed));
    let before = capture_push_universe(&env, &instance, "");

    // When she pushes her whole graph.
    let push = run_openlore_publish(&env, &["push"], &instance);
    assert_eq!(
        push.status, 0,
        "`publish push` must exit 0;\n--- stdout ---\n{}\n--- stderr ---\n{}",
        push.stdout, push.stderr
    );
    let after = capture_push_universe(&env, &instance, &push.stdout);

    // Then 41 are pushed, 1 skipped, 41/41 CID-verified; the instance's record
    // set grows by exactly the 41 new CIDs (it now holds all 42); her local
    // store's row count is unchanged (implicit-unchanged slot).
    let mut all_local = local_cids.clone();
    all_local.sort();
    let expected = Delta::new()
        .with_slot("instance.records.cids", set_to(format!("{all_local:?}")))
        .with_slot("cli.publish.pushed", set_to("41".to_string()))
        .with_slot("cli.publish.skipped", set_to("1".to_string()))
        .with_slot("cli.publish.verified", set_to("41/41".to_string()));
    assert_state_delta(&before, &after, &push_universe(), &expected);

    // And each newly pushed record recomputes (in Rust) to its local CID.
    for cid in local_cids.iter().filter(|cid| **cid != already_pushed) {
        assert_instance_stores_cid(&instance, cid);
    }

    // And the plan came from the manifest: the already-present claim was never
    // probed, re-read, or re-sent record-by-record.
    let touched_skipped: Vec<String> = instance
        .recorded_requests()
        .iter()
        .filter(|r| r.path == format!("/records/{already_pushed}"))
        .map(|r| format!("{} {}", r.method, r.path))
        .collect();
    assert!(
        touched_skipped.is_empty(),
        "the already-present claim must be skipped from the manifest diff, never probed; \
         got {touched_skipped:?}"
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
fn publish_push_is_idempotent_re_push_creates_no_duplicates() {
    use support::state_delta::{assert_state_delta, set_to, Delta};

    // Given Maria has 42 signed claims in her local graph, and her instance
    // already holds ALL 42 of them (a prior complete push).
    let env = TestEnv::initialized_as(FakeIdentity::maria());
    let local_cids = local_graph_of(&env, 42);
    let instance = instance_already_holding(&env, &local_cids);
    let before = capture_push_universe(&env, &instance, "");

    // When she re-runs the push.
    let push = run_openlore_publish(&env, &["push"], &instance);
    assert_eq!(
        push.status, 0,
        "a re-push must exit 0;\n--- stdout ---\n{}\n--- stderr ---\n{}",
        push.stdout, push.stderr
    );
    let after = capture_push_universe(&env, &instance, &push.stdout);

    // Then 0 are pushed and all 42 are skipped; the instance's record set is
    // UNCHANGED (no duplicate records — content-addressed, idempotent), and so
    // is her local row count (implicit-unchanged slots: pushed stays 0,
    // verified stays 0/0).
    let expected = Delta::new().with_slot("cli.publish.skipped", set_to("42".to_string()));
    assert_state_delta(&before, &after, &push_universe(), &expected);
}

/// PP-3: A push was interrupted after 20 of 41 new claims. When Maria re-runs
/// `openlore publish push`, the remaining 21 are pushed and no claim is
/// duplicated on the instance (additive + idempotent resume). (US-SF-003 · AC 3
/// · UAT #3 · Domain example 3.)
///
/// The interruption is modelled by its observable aftermath, not by killing a
/// process mid-push: the instance is preloaded with exactly the 20 records a
/// push would have committed before dying. There is no client-side resume
/// marker — the instance's manifest IS the resume state (Q-SF-D3), so this
/// precondition is indistinguishable, at the port, from a real interruption.
///
/// @us-sf-003 @error @j-008
#[test]
fn publish_push_resumes_after_interruption_without_duplicates() {
    use support::state_delta::{assert_state_delta, set_to, Delta};

    // Given Maria has 41 signed claims in her local graph, and an earlier push
    // was interrupted after committing 20 of them to her instance.
    let env = TestEnv::initialized_as(FakeIdentity::maria());
    let local_cids = local_graph_of(&env, 41);
    let instance = instance_already_holding(&env, &local_cids[..20]);
    let before = capture_push_universe(&env, &instance, "");

    // When she re-runs the push.
    let push = run_openlore_publish(&env, &["push"], &instance);
    assert_eq!(
        push.status, 0,
        "a resumed push must exit 0;\n--- stdout ---\n{}\n--- stderr ---\n{}",
        push.stdout, push.stderr
    );
    let after = capture_push_universe(&env, &instance, &push.stdout);

    // Then the remaining 21 are pushed (21/21 CID-verified), the 20 already
    // committed are skipped, and the instance ends holding exactly her 41 CIDs.
    let mut all_local = local_cids.clone();
    all_local.sort();
    let expected = Delta::new()
        .with_slot("instance.records.cids", set_to(format!("{all_local:?}")))
        .with_slot("cli.publish.pushed", set_to("21".to_string()))
        .with_slot("cli.publish.skipped", set_to("20".to_string()))
        .with_slot("cli.publish.verified", set_to("21/21".to_string()));
    assert_state_delta(&before, &after, &push_universe(), &expected);

    // And NO CID appears twice in the instance's manifest (commit order).
    let committed = instance.stored_cids();
    let distinct: std::collections::BTreeSet<&String> = committed.iter().collect();
    assert_eq!(
        (committed.len(), distinct.len()),
        (41, 41),
        "the manifest must list 41 distinct CIDs with no duplicate entry; got {committed:?}"
    );
}

/// PP-4: A push NEVER mutates the local store — the local DuckDB stays the
/// canonical source of truth (D-6). This is the load-bearing local-first
/// guarantee asserted directly on the local-claims universe. (US-SF-003 · AC 4.)
///
/// @us-sf-003 @j-008 @guardrail
#[test]
fn publish_push_never_mutates_the_local_store() {
    use support::state_delta::{assert_state_delta, Delta};

    // Given Maria's local graph of 5 signed claims and her empty instance.
    let env = TestEnv::initialized_as(FakeIdentity::maria());
    local_graph_of(&env, 5);
    let instance = FakeInstance::fresh();
    let before = capture_local_claims_universe(&env);

    // When she pushes her whole graph.
    let push = run_openlore_publish(&env, &["push"], &instance);
    assert_eq!(
        push.status, 0,
        "`publish push` must exit 0;\n--- stdout ---\n{}\n--- stderr ---\n{}",
        push.stdout, push.stderr
    );
    let after = capture_local_claims_universe(&env);

    // Then her local store is UNCHANGED in every slot — row count, CID set,
    // and every byte on disk (additive-only, D-6): the empty delta.
    assert_state_delta(&before, &after, &local_claims_universe(), &Delta::new());
    assert_eq!(
        instance.stored_cids().len(),
        5,
        "the push itself must have happened (all 5 claims on the instance)"
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
fn publish_push_requires_the_owner_write_token_and_is_refused_without_it() {
    use support::state_delta::{assert_state_delta, set_to, Delta};

    // Given Maria's local graph of 5 signed claims, and her EMPTY instance
    // that accepts writes only with her per-instance owner token.
    let owner_token = "pp5-owner-write-token-9f3c1e7a";
    let env = TestEnv::initialized_as(FakeIdentity::maria());
    let local_cids = local_graph_of(&env, 5);
    let instance = FakeInstance::requiring_write_token(owner_token);
    let before = capture_push_universe(&env, &instance, "");

    // When she pushes WITHOUT the token, and again with a WRONG one.
    for (attempt, token) in [
        ("no token", None),
        ("wrong token", Some("not-the-owner-token")),
    ] {
        let refused = run_openlore_publish_with_token(&env, &["push"], &instance, token);

        // Then the instance refuses the write: the CLI exits non-zero and
        // says the write was unauthorized ...
        assert_ne!(
            refused.status, 0,
            "a push with {attempt} must exit non-zero;\n--- stdout ---\n{}\n--- stderr ---\n{}",
            refused.stdout, refused.stderr
        );
        assert!(
            format!("{}{}", refused.stdout, refused.stderr).contains("unauthorized write"),
            "a push with {attempt} must report 'unauthorized write';\n--- stdout ---\n{}\n\
             --- stderr ---\n{}",
            refused.stdout,
            refused.stderr
        );
        assert_token_never_echoed(&refused, owner_token);
        // ... and NOTHING is stored: every slot of the push universe
        // (instance.records.cids, the push counts, the local row count) is
        // unchanged — the empty delta.
        let after_refusal = capture_push_universe(&env, &instance, &refused.stdout);
        assert_state_delta(&before, &after_refusal, &push_universe(), &Delta::new());
    }

    // When she pushes WITH the correct owner token.
    let accepted = run_openlore_publish_with_token(&env, &["push"], &instance, Some(owner_token));
    assert_eq!(
        accepted.status, 0,
        "a push with the owner token must exit 0;\n--- stdout ---\n{}\n--- stderr ---\n{}",
        accepted.stdout, accepted.stderr
    );
    assert_token_never_echoed(&accepted, owner_token);
    let after = capture_push_universe(&env, &instance, &accepted.stdout);

    // Then the same push stores all 5 records, CID-verified.
    let mut all_local = local_cids.clone();
    all_local.sort();
    let expected = Delta::new()
        .with_slot("instance.records.cids", set_to(format!("{all_local:?}")))
        .with_slot("cli.publish.pushed", set_to("5".to_string()))
        .with_slot("cli.publish.verified", set_to("5/5".to_string()));
    assert_state_delta(&before, &after, &push_universe(), &expected);

    // And the token only ever travels on writes: no read (GET) that crossed
    // the seam carried an Authorization header (capability split, ADR-062 §6).
    assert_reads_carry_no_credentials(&instance);
}
