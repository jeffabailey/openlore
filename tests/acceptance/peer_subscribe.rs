//! Slice-03 acceptance — `openlore peer add` + `openlore peer remove` verbs.
//!
//! Drives the real `openlore` CLI as a subprocess via `assert_cmd` per
//! Mandate 1 (Hexagonal Boundary) + slice-01 DD-5. PDS + Identity +
//! Peer-PDS are faked; everything else is real (real DuckDB, real
//! filesystem, real clap). Pattern inherited verbatim from
//! `walking_skeleton.rs` — the shared `support/mod.rs` + the new
//! `openlore_test_support::FakePeerPds` provide the test seam.
//!
//! Covers:
//! - US-FED-001: subscribe to a peer's claim stream (add + idempotent
//!   re-add + self-DID rejection + unresolvable-DID rejection)
//! - US-FED-005: remove a peer subscription with optional purge
//!   (soft-remove + hard-purge + `--no-tty` refusal per WD-36)
//!
//! Per Mandate 7 (RED-ready scaffolding) + slice-03 DD-FED-5
//! (inherits slice-01 DD-2 fail-for-right-reason gate deferred until
//! slice-03 production code lands): every test body panics via
//! `todo!()`. DELIVER's first slice-03 step bootstraps the new types
//! (`PeerStoragePort`, `IdentityPort::resolve_peer`, `VerbPeerAdd`,
//! `VerbPeerRemove`) so the panic-at-`todo!()` classifies as RED, not
//! BROKEN (import error). DELIVER then unskips one at a time per the
//! standard outside-in TDD loop.
//
// SCAFFOLD: true

mod support;

#[allow(unused_imports)]
use support::*;

// =============================================================================
// US-FED-001 — `openlore peer add <did>`
// =============================================================================

/// PS-1: `openlore peer add did:plc:rachel-test` resolves the peer DID
/// via the identity adapter, probes that the peer's PDS exposes
/// `org.openlore.claim`, persists a subscription row in
/// `peer_subscriptions`, and prints the next-step hint + the
/// unsubscribe hint. (US-FED-001 AC 1-2-6; UAT scenario #1.)
///
/// Port-to-port (subprocess): the driving port is the real `openlore`
/// binary; the driven boundaries are faked — `FakePeerPds` substitutes
/// both the PLC `resolveDid` handler (so `IdentityPort::resolve_peer`
/// resolves) AND the peer PDS itself, while the local DuckDB store is
/// REAL. The observable universe is `{exit code, stdout lines,
/// peer_subscriptions row state}`; assertions read the binary's stdout
/// and the persisted DuckDB row, never internal verb state.
///
/// @us-fed-001 @real-io @driving_port @j-003
#[test]
fn peer_subscribe_add_resolves_did_and_persists_subscription() {
    let env = TestEnv::initialized();

    // The peer PDS double hosts Rachel's records AND her resolveDid DID
    // document on one base URL. The DID document's service endpoint points
    // back at this same fake, so `resolve_peer` learns the PDS endpoint
    // from the resolved document.
    let peer_did = "did:plc:rachel-test";
    let peer = PeerPds::for_peer(peer_did, fixture_other_developer_three_claims());

    // The in-binary peer resolver finds the fake via the per-peer endpoint
    // env-var seam (mirrors the slice-01 `OPENLORE_PDS_ENDPOINT` pattern;
    // see acceptance-tests.md §test-doubles `OPENLORE_PEER_PDS_ENDPOINT_<did>`).
    let outcome = run_openlore_with_peer_resolver(
        &env,
        &["peer", "add", peer_did],
        peer_did,
        peer.endpoint_url(),
    );

    // 1. Resolve confirmation names the DID + handle + claim collection.
    assert_exit_zero_and_stdout_contains(&outcome, peer_did);
    assert!(
        outcome.stdout.contains("org.openlore.claim"),
        "expected the resolve step to confirm the peer exposes the \
         org.openlore.claim collection;\n--- stdout ---\n{}\n--- stderr ---\n{}",
        outcome.stdout,
        outcome.stderr
    );

    // 2. Next-pull hint + 3. unsubscribe hint (ADR-013 / journey step 1).
    assert!(
        outcome.stdout.contains("openlore peer pull"),
        "expected the next-pull hint `openlore peer pull`;\n--- stdout ---\n{}",
        outcome.stdout
    );
    assert!(
        outcome
            .stdout
            .contains(&format!("openlore peer remove {peer_did}")),
        "expected the unsubscribe hint `openlore peer remove {peer_did}`;\n\
         --- stdout ---\n{}",
        outcome.stdout
    );

    // 4. A subscription row is persisted in `peer_subscriptions` — exactly
    //    one row, active (removed_at IS NULL), attributed to the peer DID.
    assert_one_active_subscription_for(&env, peer_did);
}

/// Universe-bound: "the `peer_subscriptions` store holds exactly ONE row
/// for `peer_did`, and that row is active (`removed_at IS NULL`)".
/// Port-exposed name: `peer_storage.subscriptions.active_row_count[did]`.
///
/// Opens a raw `duckdb::Connection` for the assertion (test-support is the
/// only place raw SQL is acceptable; production goes through
/// `PeerStoragePort`). Mirrors `assert_duckdb_publication_metadata_for_cid`.
fn assert_one_active_subscription_for(env: &TestEnv, peer_did: &str) {
    let db_path = env.duckdb_path();
    assert!(
        db_path.exists(),
        "expected DuckDB to exist at {} after peer add; file missing",
        db_path.display()
    );
    let conn = duckdb::Connection::open(&db_path).unwrap_or_else(|err| {
        panic!(
            "open DuckDB at {} for subscription assertion: {err}",
            db_path.display()
        )
    });

    let (total, active): (i64, i64) = conn
        .query_row(
            "SELECT \
                count(*), \
                count(*) FILTER (WHERE removed_at IS NULL) \
             FROM peer_subscriptions WHERE peer_did = ?",
            duckdb::params![peer_did],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap_or_else(|err| panic!("query peer_subscriptions for {peer_did}: {err}"));

    assert_eq!(
        total, 1,
        "expected exactly one peer_subscriptions row for {peer_did}; got {total}"
    );
    assert_eq!(
        active, 1,
        "expected the peer_subscriptions row for {peer_did} to be active \
         (removed_at IS NULL); got {active} active rows"
    );
}

/// PS-2: Re-running `openlore peer add` against an already-subscribed
/// peer prints "already subscribed since <ts>" with the original
/// subscription timestamp, does NOT duplicate the row, and exits 0.
/// (US-FED-001 AC 3; UAT scenario #2; Example 2.)
///
/// @us-fed-001 @real-io @driving_port @j-003 @edge
#[test]
fn peer_subscribe_add_is_idempotent_on_re_subscribe() {
    let env = TestEnv::initialized();

    let peer_did = "did:plc:rachel-test";
    let peer = PeerPds::for_peer(peer_did, fixture_other_developer_three_claims());

    // First add: fresh subscribe. Persists exactly one active row whose
    // `subscribed_at` becomes the canonical "since" timestamp the second
    // add must echo back unchanged.
    let first = run_openlore_with_peer_resolver(
        &env,
        &["peer", "add", peer_did],
        peer_did,
        peer.endpoint_url(),
    );
    assert_exit_zero_and_stdout_contains(&first, peer_did);
    assert_one_active_subscription_for(&env, peer_did);

    // The original `subscribed_at`, read straight from the persisted row —
    // this is the universe slot the idempotent path must NOT mutate.
    let original_subscribed_at = subscribed_at_for(&env, peer_did);

    // Second add of the SAME peer: idempotent re-subscribe. Exits 0, prints
    // "already subscribed since <original_ts>", and does NOT duplicate the
    // row. (US-FED-001 AC 3; Example 2.)
    let second = run_openlore_with_peer_resolver(
        &env,
        &["peer", "add", peer_did],
        peer_did,
        peer.endpoint_url(),
    );

    assert_exit_zero_and_stdout_contains(&second, "already subscribed since");
    assert!(
        second.stdout.contains(&original_subscribed_at.to_rfc3339()),
        "expected the idempotent re-add to echo the ORIGINAL subscribed_at \
         {} (not a fresh clock read);\n--- stdout ---\n{}\n--- stderr ---\n{}",
        original_subscribed_at.to_rfc3339(),
        second.stdout,
        second.stderr
    );

    // The defining idempotency invariant: still exactly ONE active row —
    // the second add appended nothing.
    assert_one_active_subscription_for(&env, peer_did);
}

/// Read the persisted `subscribed_at` for `peer_did` straight from the
/// DuckDB `peer_subscriptions` row. Port-exposed name:
/// `peer_storage.subscriptions.subscribed_at[did]`. Test-support is the
/// only place raw SQL is acceptable; production goes through
/// `PeerStoragePort`. Mirrors `assert_one_active_subscription_for`.
fn subscribed_at_for(env: &TestEnv, peer_did: &str) -> chrono::DateTime<chrono::Utc> {
    let db_path = env.duckdb_path();
    let conn = duckdb::Connection::open(&db_path).unwrap_or_else(|err| {
        panic!(
            "open DuckDB at {} for subscribed_at read: {err}",
            db_path.display()
        )
    });
    conn.query_row(
        "SELECT subscribed_at FROM peer_subscriptions WHERE peer_did = ?",
        duckdb::params![peer_did],
        |r| r.get::<_, chrono::DateTime<chrono::Utc>>(0),
    )
    .unwrap_or_else(|err| panic!("read subscribed_at for {peer_did}: {err}"))
}

/// PS-3: `openlore peer add did:plc:not-a-real-did` exits non-zero with
/// stderr naming the DID and the resolution failure cause; no
/// peer_subscriptions row is written. (US-FED-001 AC 4 + Example 3.)
///
/// @us-fed-001 @real-io @driving_port @j-003 @error
#[test]
fn peer_subscribe_add_rejects_unresolvable_did_and_writes_no_subscription() {
    let env = TestEnv::initialized();

    // An unresolvable DID: a `PeerPds` whose resolveDid handler is driven
    // into the "unreachable" failure mode (PP-7 seam). Wiring the per-peer
    // resolver env var at this dead endpoint makes resolution fail
    // DETERMINISTICALLY — `IdentityPort::resolve_peer` lifts the transport
    // error to `PeerResolutionFailed`, which `VerbPeerAdd` propagates as a
    // non-zero exit BEFORE any storage call. No real PLC-directory network
    // egress, so the @error path is hermetic.
    let bad_did = "did:plc:not-a-real-did";
    let dead_peer = PeerPds::for_peer(bad_did, vec![]);
    dead_peer.simulate_unreachable();

    let outcome = run_openlore_with_peer_resolver(
        &env,
        &["peer", "add", bad_did],
        bad_did,
        dead_peer.endpoint_url(),
    );

    // Exit non-zero with stderr naming BOTH the DID and the
    // resolution-failure cause (ADR-013 exit-code table / Example 3).
    assert_exit_nonzero_and_stderr_contains(&outcome, bad_did);
    assert!(
        outcome.stderr.contains("resolve"),
        "expected stderr to name the resolution-failure cause for {bad_did};\n\
         --- stdout ---\n{}\n--- stderr ---\n{}",
        outcome.stdout,
        outcome.stderr
    );

    // The defining no-write invariant: ZERO peer_subscriptions rows — the
    // resolve failure short-circuited before `add_subscription` ran.
    assert_zero_subscriptions_for(&env, bad_did);
}

/// PS-4: `openlore peer add <self_did>` (where <self_did> is the local
/// user's own DID from identity.toml) exits non-zero with the error
/// message "you are already your own author; cannot subscribe to
/// yourself"; no peer_subscriptions row is written. (US-FED-001 AC 5 +
/// UAT scenario #4.)
///
/// @us-fed-001 @real-io @driving_port @j-003 @error
#[test]
fn peer_subscribe_add_rejects_self_did_subscription() {
    let env = TestEnv::initialized();

    // The local user's own DID, read from the same identity the harness
    // wires into the subprocess via `OPENLORE_DID`. `peer add <self_did>`
    // must short-circuit in `VerbPeerAdd` BEFORE any network or storage
    // call (anti-merging; PS-4 / UAT scenario #4).
    let self_did = env.identity.author_did().to_string();

    let outcome = run_openlore(&env, &["peer", "add", &self_did]);

    // Exit non-zero with the literal self-subscribe refusal (US-FED-001 AC 5).
    assert_exit_nonzero_and_stderr_contains(&outcome, "cannot subscribe to yourself");

    // No-write invariant: ZERO peer_subscriptions rows for the self DID —
    // the short-circuit fired before `add_subscription`.
    assert_zero_subscriptions_for(&env, &self_did);
}

/// Universe-bound: "the `peer_subscriptions` store holds ZERO rows for
/// `peer_did`" — the no-write invariant both error paths (PS-3 + PS-4)
/// must satisfy. Port-exposed name:
/// `peer_storage.subscriptions.row_count[did] == 0`.
///
/// Opens a raw `duckdb::Connection` for the assertion (test-support is the
/// only place raw SQL is acceptable; production goes through
/// `PeerStoragePort`). Sibling of `assert_one_active_subscription_for`.
/// The `peer_subscriptions` table exists post-`init` (migration v3), so a
/// COUNT against it is well-defined even when the verb wrote nothing.
fn assert_zero_subscriptions_for(env: &TestEnv, peer_did: &str) {
    let db_path = env.duckdb_path();
    assert!(
        db_path.exists(),
        "expected DuckDB to exist at {} after init; file missing",
        db_path.display()
    );
    let conn = duckdb::Connection::open(&db_path).unwrap_or_else(|err| {
        panic!(
            "open DuckDB at {} for no-write assertion: {err}",
            db_path.display()
        )
    });

    let total: i64 = conn
        .query_row(
            "SELECT count(*) FROM peer_subscriptions WHERE peer_did = ?",
            duckdb::params![peer_did],
            |r| r.get(0),
        )
        .unwrap_or_else(|err| panic!("query peer_subscriptions for {peer_did}: {err}"));

    assert_eq!(
        total, 0,
        "expected ZERO peer_subscriptions rows for {peer_did} on the error \
         path (no-write invariant); got {total}"
    );
}

// =============================================================================
// US-FED-005 — `openlore peer remove <did> [--purge]`
// =============================================================================

/// PS-5: `openlore peer remove <did>` (no --purge) sets
/// `peer_subscriptions.removed_at` for that peer but leaves the
/// `peer_claims` row count unchanged; subsequent
/// `graph query --federated` annotates those rows as "(unsubscribed
/// cache)". (US-FED-005 AC 1-2; UAT scenario #1; WD-25 soft-remove
/// retains cache.)
///
/// @us-fed-005 @real-io @driving_port @j-003 @j-003c @happy
#[test]
fn peer_subscribe_remove_soft_keeps_cached_peer_claims() {
    let env = TestEnv::initialized();

    // Precondition 1: an ACTIVE subscription to the peer (via the real
    // `peer add` verb, so the row is written through the production
    // PeerStoragePort exactly as a user would create it).
    let peer_did = "did:plc:rachel-test";
    let peer = PeerPds::for_peer(peer_did, fixture_other_developer_three_claims());
    let added = run_openlore_with_peer_resolver(
        &env,
        &["peer", "add", peer_did],
        peer_did,
        peer.endpoint_url(),
    );
    assert_one_active_subscription_for(&env, peer_did);

    // Precondition 2: cached peer_claims rows for this peer. `peer pull`
    // (the production path that populates peer_claims) lands in Phase 04,
    // so the test seeds the cache directly via the test-support raw-SQL
    // helper (test-support is the only place raw SQL is acceptable; the
    // soft-remove contract under test is "retain WHATEVER is cached",
    // independent of how it got there).
    let cached_count = 3;
    seed_cached_peer_claims(&env, peer_did, cached_count);
    assert_one_active_subscription_for(&env, peer_did); // seeding peer_claims must not touch subscriptions
    let _ = added;

    // Action: soft-remove (no --purge).
    let outcome = run_openlore(&env, &["peer", "remove", peer_did]);

    // Observable #1 — the CLI confirms the soft-remove and the retained
    // cache count (US-FED-005 Example 1: "Removed subscription. N cached
    // peer claims retained (use --purge to delete them).").
    assert_exit_zero_and_stdout_contains(&outcome, "Removed subscription");
    assert!(
        outcome
            .stdout
            .contains(&format!("{cached_count} cached peer claims retained")),
        "expected the soft-remove output to report {cached_count} retained \
         cached peer claims;\n--- stdout ---\n{}\n--- stderr ---\n{}",
        outcome.stdout,
        outcome.stderr
    );

    // Observable #2 — the storage-level soft-remove contract (WD-25 /
    // component-boundaries §adapter-duckdb soft-remove isolation probe #5):
    //   (a) the subscription row's `removed_at` is now SET (soft-removed,
    //       no longer active), and
    //   (b) the `peer_claims` row count for the peer is UNCHANGED.
    // (The "(unsubscribed cache)" federated-query annotation is FQ-territory
    // in Phase 05; PS-5 pins the observable-now storage state.)
    assert_subscription_soft_removed_for(&env, peer_did);
    assert_peer_claims_row_count_for(&env, peer_did, cached_count);
}

/// PS-6: `openlore peer remove <did> --purge` shows the cached-record
/// count, prompts `Proceed? [y/N]`, and on confirmation DELETES the
/// subscription AND ALL of that peer's cached claims from
/// `peer_claims`, but PRESERVES user-authored counter-claims in
/// `author_claims` referencing those (now-deleted) CIDs (WD-25
/// invariant). (US-FED-005 AC 3 + 5; UAT scenarios #2 + #5; integration
/// gate `peer_remove_purge_separation`.)
///
/// @us-fed-005 @real-io @driving_port @j-003 @j-003c @happy
#[test]
fn peer_subscribe_remove_purge_with_confirmation_deletes_peer_claims_and_preserves_user_counters() {
    todo!("DELIVER (slice-03): wire VerbPeerRemove --purge branch → TtyIO prompt → PeerStoragePort.hard_purge; assert (a) peer_subscriptions row gone, (b) peer_claims rows for that peer gone, (c) peer_claims/<did>/ directory removed, (d) author_claims (including counter-claims) untouched. Drives integration gate 4 (peer_remove_purge_separation).")
}

/// PS-7: `openlore peer remove <did> --purge` answered "n" to the
/// confirmation prompt leaves BOTH the subscription AND the cached
/// peer claims unchanged; CLI prints "Cancelled. Subscription and
/// cached peer claims unchanged." and exits 0. (US-FED-005 AC 4; UAT
/// scenario #3.)
///
/// @us-fed-005 @real-io @driving_port @j-003 @j-003c @edge
#[test]
fn peer_subscribe_remove_purge_declined_leaves_state_unchanged() {
    todo!("DELIVER (slice-03): feed 'n\\n' to the --purge prompt via run_openlore_with_stdin; assert peer_subscriptions row STILL present AND peer_claims rows unchanged AND stdout literal 'Cancelled. Subscription and cached peer claims unchanged.'")
}

/// PS-8: `openlore peer remove --purge --no-tty <did>` REFUSES to run
/// the destructive branch — exits non-zero with a directing error
/// message naming the missing TTY and pointing at slice-04's future
/// `--yes` flag. NO purge happens. (US-FED-005 + WD-21 + WD-36 lock;
/// ADR-013 exit-code table for `peer remove`.)
///
/// @us-fed-005 @real-io @driving_port @j-003c @error
#[test]
fn peer_subscribe_remove_purge_refuses_no_tty_mode() {
    todo!("DELIVER (slice-03): wire VerbPeerRemove --purge + --no-tty detection BEFORE TtyIO.confirm is called; assert exit-nonzero + stderr containing the directing error per WD-36; assert peer_subscriptions row STILL present AND peer_claims rows unchanged (no silent purge)")
}
