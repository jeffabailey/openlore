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

/// Run the `openlore` binary with the per-peer resolver endpoint wired so
/// the in-binary `IdentityPort::resolve_peer` resolves `peer_did` against
/// the supplied `FakePeerPds` base URL instead of the real PLC directory.
///
/// Mirrors `run_openlore` (clean env + the slice-01 stub seams) and adds
/// the `OPENLORE_PEER_PDS_ENDPOINT_<encoded_did>` env var the production
/// resolver reads. Lives here (not in `support/mod.rs`) because the
/// per-peer-endpoint seam is slice-03-specific to the peer verbs.
fn run_openlore_with_peer_resolver(
    env: &TestEnv,
    args: &[&str],
    peer_did: &str,
    peer_endpoint: &str,
) -> CliOutcome {
    use std::process::{Command, Stdio};

    let bin = assert_cmd::cargo::cargo_bin("openlore");
    let output = Command::new(&bin)
        .args(args)
        .env_clear()
        .env("OPENLORE_HOME", &env.home)
        .env("OPENLORE_DID", env.identity.author_did())
        .env("OPENLORE_KEY_SEED_HEX", &env.identity.seed_hex)
        .env("OPENLORE_PDS_ENDPOINT", env.pds.endpoint_url())
        .env(peer_resolver_env_var(peer_did), peer_endpoint)
        .env("PATH", std::env::var("PATH").unwrap_or_default())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .unwrap_or_else(|e| panic!("spawn openlore at {bin:?}: {e}"));

    CliOutcome {
        status: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    }
}

/// The per-peer resolver env-var NAME for a DID. Encoding: uppercase the
/// DID and replace every non-`[A-Z0-9]` character with `_` so the result
/// is a legal POSIX environment-variable name. This MUST agree with the
/// production resolver's lookup (adapter-atproto-did `peer_resolve`).
///
/// `did:plc:rachel-test` → `OPENLORE_PEER_PDS_ENDPOINT_DID_PLC_RACHEL_TEST`.
fn peer_resolver_env_var(did: &str) -> String {
    let encoded: String = did
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_uppercase()
            } else {
                '_'
            }
        })
        .collect();
    format!("OPENLORE_PEER_PDS_ENDPOINT_{encoded}")
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
    todo!("DELIVER (slice-03): assert AddSubscriptionOutcome::AlreadyExisted dispatch path emits the original subscribed_at timestamp; assert exactly one peer_subscriptions row remains after two add invocations")
}

/// PS-3: `openlore peer add did:plc:not-a-real-did` exits non-zero with
/// stderr naming the DID and the resolution failure cause; no
/// peer_subscriptions row is written. (US-FED-001 AC 4 + Example 3.)
///
/// @us-fed-001 @real-io @driving_port @j-003 @error
#[test]
fn peer_subscribe_add_rejects_unresolvable_did_and_writes_no_subscription() {
    todo!("DELIVER (slice-03): wire IdentityPort.resolve_peer failure path → VerbPeerAdd exit-1 + stderr containing the DID + resolution-failure message per ADR-013 exit-code table; assert zero peer_subscriptions rows")
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
    todo!("DELIVER (slice-03): wire self-DID short-circuit in VerbPeerAdd BEFORE PeerStoragePort.add_subscription is called; assert stderr literal 'cannot subscribe to yourself' + zero peer_subscriptions rows")
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
    todo!("DELIVER (slice-03): wire VerbPeerRemove default branch → PeerStoragePort.soft_remove; assert peer_subscriptions.removed_at IS NOT NULL AND peer_claims row count unchanged; assert subsequent graph query --federated annotates rows '(unsubscribed cache)'")
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
