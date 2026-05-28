//! Slice-03 acceptance — `openlore peer pull` verb.
//!
//! Drives the real `openlore` CLI as a subprocess via `assert_cmd`.
//! Reuses `support/mod.rs` for TestEnv + assertion helpers; adds the
//! peer-PDS double via `openlore_test_support::FakePeerPds`. The
//! adversarial-peer fixtures (`fixture_adversarial_peer_*`) drive the
//! KPI-FED-6 + WD-40 + WD-41 rejection scenarios.
//!
//! Covers:
//! - US-FED-002: pull peer claims with per-claim signature verification
//!   + per-claim CID recomputation (WD-24)
//! - WD-37 fault isolation (per-peer + per-record)
//! - WD-40 self-attribution rejection at write time
//! - WD-41 cross-attribution rejection at write time
//! - WD-18 / ADR-016 pull-on-demand only (no daemon, no auto-pull)
//!
//! Per Mandate 11 (sad-path coverage at layer 3 is example-based, NEVER
//! PBT-generated) every adversarial scenario is a named example with an
//! explicit fixture. No proptest at this layer; the property-shaped
//! invariants (CID byte-stability across N pulls) live in
//! `lexicon_conformance.rs` at layer 2.
//
// SCAFFOLD: true

mod support;

#[allow(unused_imports)]
use support::*;

// =============================================================================
// US-FED-002 — happy path
// =============================================================================

/// PP-1: `openlore peer pull` against one subscribed peer (Rachel)
/// publishing N records fetches all N via XRPC, verifies each
/// signature against Rachel's DID-doc key, recomputes each CID locally
/// and byte-matches against the peer's published rkey, and stores all
/// N rows in `peer_claims` attributed to `did:plc:rachel-test`. The
/// pull summary reports "N stored, 0 rejected" and stdout contains
/// the content-frozen line "None merged with your own claims."
/// (US-FED-002 AC 1-2-3-5-8 + UAT scenario #1 + ADR-013 output
/// convention.)
///
/// @us-fed-002 @real-io @driving_port @j-003 @j-003a @happy
#[test]
fn peer_pull_fetches_verifies_and_stores_peer_claims_attributed_per_record() {
    let env = TestEnv::initialized();

    // Rachel publishes THREE honest, REAL-signed claims. The slice-03
    // peer fixtures ship placeholder sigs/rkeys (see fixtures_peer.rs); the
    // happy path needs genuine crypto so the pull pipeline's per-record
    // verify + CID-recompute actually pass. `build_verifiable_peer_records`
    // materializes them deterministically and returns Rachel's real pubkey
    // hex for the verify seam.
    let peer_did = "did:plc:rachel-test";
    let rachel_seed = [7u8; 32];
    let (records, rachel_pubkey_hex) = build_verifiable_peer_records(peer_did, rachel_seed);
    assert_eq!(records.len(), 3, "Rachel publishes exactly three claims");

    let peer = PeerPds::for_peer(peer_did, records);

    // Precondition: ONE active subscription, created through the real
    // `peer add` verb (so the row exists exactly as a user would create it).
    let added = run_openlore_with_peer_resolver(
        &env,
        &["peer", "add", peer_did],
        peer_did,
        peer.endpoint_url(),
    );
    assert_eq!(
        added.status, 0,
        "peer add precondition must succeed;\n--- stdout ---\n{}\n--- stderr ---\n{}",
        added.stdout, added.stderr
    );

    // DD-FED-10 universe BEFORE the pull: zero peer claims, the author
    // `claims` table at its post-init count, the peer partition absent.
    let author_claims_before = user_author_claim_count_now(&env);

    // Action: `openlore peer pull`. The in-binary pipeline re-resolves
    // Rachel's PDS, walks listRecords cursors, and for EACH record verifies
    // the signature against Rachel's DID-doc key AND recomputes the CID
    // before writing through PeerStoragePort.write_peer_claim.
    let outcome = run_openlore_pull(
        &env,
        &["peer", "pull"],
        peer_did,
        peer.endpoint_url(),
        &rachel_pubkey_hex,
    );

    // 1. Exit 0 + the ADR-013 content-frozen anti-merging line.
    assert_exit_zero_and_stdout_contains(&outcome, "None merged with your own claims");

    // 2. Per-peer progress block (Q-DELIVER-6 / journey YAML tui_mockup):
    //    fetched / new / verified / stored counts, named under the peer DID.
    assert!(
        outcome.stdout.contains(peer_did),
        "expected the progress block to name the peer DID {peer_did};\n\
         --- stdout ---\n{}",
        outcome.stdout
    );
    for needle in ["fetched", "verified", "stored"] {
        assert!(
            outcome.stdout.contains(needle),
            "expected the per-peer progress block to report `{needle}`;\n\
             --- stdout ---\n{}",
            outcome.stdout
        );
    }
    assert!(
        outcome.stdout.contains("3/3"),
        "expected the progress block to report 3/3 signatures verified;\n\
         --- stdout ---\n{}",
        outcome.stdout
    );

    // 3. First-pull orientation fires (WD-39 once-per-user; the FIRST EVER
    //    successful pull emits the orientation marker — re-asserted absent
    //    on a re-pull by PP-2).
    assert!(
        outcome.stdout.contains("--federated"),
        "expected the first-pull orientation to point at `graph query --federated`;\n\
         --- stdout ---\n{}",
        outcome.stdout
    );

    // 4. DD-FED-10 (LOAD-BEARING) — the storage state-delta:
    //    - peer_storage.claims.row_count_by_author[rachel] : 0 → 3
    //    - every stored CID attributed to Rachel, NEVER any other DID
    //      (anti-merging, I-FED-1 — total row count == 3).
    assert_peer_claims_attributed_to(&env, peer_did, 3);

    //    - author_claims.row_count : UNCHANGED (no merge with own claims).
    assert_eq!(
        user_author_claim_count_now(&env),
        author_claims_before,
        "the author `claims` table must be UNCHANGED by a peer pull (no merge \
         with your own claims — DD-FED-10)"
    );

    //    - filesystem.peer_claims_dir.exists[rachel] : → true, with one
    //      <cid>.json artifact per stored record (Q-DELIVER-2 encoding).
    let partition = peer_claims_dir_for(&env, peer_did);
    assert!(
        partition.exists(),
        "expected the on-disk peer_claims partition for {peer_did} to exist \
         after pull, at {}",
        partition.display()
    );
    let artifact_count = std::fs::read_dir(&partition)
        .unwrap_or_else(|e| panic!("read peer_claims partition {}: {e}", partition.display()))
        .filter(|entry| {
            entry
                .as_ref()
                .ok()
                .and_then(|e| e.path().extension().map(|x| x == "json"))
                .unwrap_or(false)
        })
        .count();
    assert_eq!(
        artifact_count,
        3,
        "expected exactly 3 `<cid>.json` artifacts under {} after pull; got {artifact_count}",
        partition.display()
    );
}

/// Read the author `claims` table row count straight from DuckDB.
/// Port-exposed name: `author_claims.row_count`. The author store is
/// single-tenant, so a global count is the observable surface for "the
/// user's own claims were not touched by a peer pull" (DD-FED-10).
fn user_author_claim_count_now(env: &TestEnv) -> usize {
    let db_path = env.duckdb_path();
    let conn = duckdb::Connection::open(&db_path).unwrap_or_else(|err| {
        panic!(
            "open DuckDB at {} for author-claims count: {err}",
            db_path.display()
        )
    });
    let total: i64 = conn
        .query_row("SELECT count(*) FROM claims", [], |r| r.get(0))
        .unwrap_or_else(|err| panic!("query author claims count: {err}"));
    total.max(0) as usize
}

/// PP-2: Re-running `openlore peer pull` with no new records on the
/// peer's PDS skips already-stored claims by CID and reports
/// "0 new, N already in peer_claims, skipped" with exit 0.
/// (US-FED-002 AC 6 + UAT scenario #4 — pull is idempotent.)
///
/// @us-fed-002 @real-io @driving_port @j-003 @edge
#[test]
fn peer_pull_is_idempotent_skipping_already_stored_claims_by_cid() {
    let env = TestEnv::initialized();

    // Rachel publishes THREE honest, REAL-signed claims (same builder as
    // PP-1). The peer's PDS is STATIC across both pulls — no new records
    // appear between invocations, so the second pull must find every CID
    // already cached.
    let peer_did = "did:plc:rachel-test";
    let rachel_seed = [7u8; 32];
    let (records, rachel_pubkey_hex) = build_verifiable_peer_records(peer_did, rachel_seed);
    assert_eq!(records.len(), 3, "Rachel publishes exactly three claims");

    let peer = PeerPds::for_peer(peer_did, records);

    // Precondition: ONE active subscription created through the real
    // `peer add` verb.
    let added = run_openlore_with_peer_resolver(
        &env,
        &["peer", "add", peer_did],
        peer_did,
        peer.endpoint_url(),
    );
    assert_eq!(
        added.status, 0,
        "peer add precondition must succeed;\n--- stdout ---\n{}\n--- stderr ---\n{}",
        added.stdout, added.stderr
    );

    // FIRST pull: stores all three (PP-1 still green — the happy path is a
    // precondition for the idempotency claim).
    let first = run_openlore_pull(
        &env,
        &["peer", "pull"],
        peer_did,
        peer.endpoint_url(),
        &rachel_pubkey_hex,
    );
    assert_exit_zero_and_stdout_contains(&first, "None merged with your own claims");
    assert_peer_claims_attributed_to(&env, peer_did, 3);
    // First pull reports three NEW; the re-pull marker is absent yet.
    assert!(
        first.stdout.contains("Pulled 3 new peer claims"),
        "first pull reports 3 NEW peer claims;\n--- stdout ---\n{}",
        first.stdout
    );
    // First-pull orientation fires on the FIRST EVER pull (WD-39). The
    // once-per-user marker is the distinctive "First federated pull
    // complete" line — NOT the `--federated` token, which also appears in
    // the always-present content-frozen anti-merging line.
    assert!(
        first.stdout.contains("First federated pull complete"),
        "first pull emits the once-per-user orientation;\n--- stdout ---\n{}",
        first.stdout
    );

    // SECOND pull against the UNCHANGED peer PDS. Every record's CID is
    // already cached ⇒ `write_peer_claim` returns `written: false` for each,
    // so the pull stores ZERO new rows and reports them all as
    // already-present/skipped.
    let second = run_openlore_pull(
        &env,
        &["peer", "pull"],
        peer_did,
        peer.endpoint_url(),
        &rachel_pubkey_hex,
    );

    // 1. Exit 0 — an idempotent re-pull is NOT a failure (no peer skip, no
    //    record rejection).
    assert_exit_zero_and_stdout_contains(&second, "None merged with your own claims");

    // 2. The progress block reports the records as already-present/skipped,
    //    NEVER as "new". This is the user-observable proof that
    //    `WritePeerClaimOutcome.written == false` for each existing CID
    //    (component-boundaries §PeerStoragePort.write_peer_claim).
    assert!(
        second.stdout.contains("already in peer_claims"),
        "second pull must report records as already in peer_claims (skipped), \
         not new;\n--- stdout ---\n{}",
        second.stdout
    );
    assert!(
        second.stdout.contains("Pulled 0 new peer claims"),
        "second pull must report ZERO new peer claims (every CID already \
         cached — written:false);\n--- stdout ---\n{}",
        second.stdout
    );

    // 3. DD-FED-10 (LOAD-BEARING) — the storage state-delta across the
    //    re-pull is EMPTY: the row count attributed to Rachel is STILL
    //    exactly 3 (no duplicate peer_claims rows), and the anti-merging
    //    total-equals-attributed invariant (I-FED-1) still holds.
    assert_peer_claims_attributed_to(&env, peer_did, 3);

    //    The on-disk artifact partition still holds exactly three
    //    `<cid>.json` files — no duplicate artifacts written on re-pull.
    let partition = peer_claims_dir_for(&env, peer_did);
    let artifact_count = std::fs::read_dir(&partition)
        .unwrap_or_else(|e| panic!("read peer_claims partition {}: {e}", partition.display()))
        .filter(|entry| {
            entry
                .as_ref()
                .ok()
                .and_then(|e| e.path().extension().map(|x| x == "json"))
                .unwrap_or(false)
        })
        .count();
    assert_eq!(
        artifact_count,
        3,
        "expected exactly 3 `<cid>.json` artifacts under {} after the re-pull \
         (idempotent — no duplicates); got {artifact_count}",
        partition.display()
    );

    // 4. WD-39: the once-per-user orientation does NOT re-fire on the second
    //    pull (it already fired on the first). Asserted on the distinctive
    //    orientation marker, not the `--federated` token shared with the
    //    always-present anti-merging line.
    assert!(
        !second.stdout.contains("First federated pull complete"),
        "the first-pull orientation must NOT re-fire on a re-pull (WD-39 \
         once-per-user);\n--- stdout ---\n{}",
        second.stdout
    );
}

// =============================================================================
// US-FED-002 — sad paths (Mandate 11: example-based, never PBT-generated)
// =============================================================================

/// PP-3 / Sad: A peer publishes 5 records, one of which has a tampered
/// signature (last byte flipped after the peer's nominal sign step).
/// `openlore peer pull` rejects ONLY that record, stores the other 4,
/// reports "4 stored, 1 rejected (signature invalid)", and exits
/// non-zero overall (WD-37 fault isolation + ADR-013 exit-code table).
/// Drives KPI-FED-6 (zero invalid signatures stored) + integration
/// gate `peer_tampered_signature_rejected`.
///
/// @us-fed-002 @real-io @driving_port @j-003a @error @kpi-fed-6
#[test]
fn peer_pull_rejects_tampered_signature_per_record_and_stores_honest_records() {
    let env = TestEnv::initialized();

    // Rachel publishes FIVE records: FOUR genuinely-signed honest claims +
    // ONE record whose rkey IS the real CID (so it PASSES the CID round-trip,
    // WD-24) but whose `signature.sig` last byte was flipped after the sign
    // step (so `claim_domain::verify` REJECTS it). This isolates the
    // SIGNATURE-rejection branch — the only defect is the signature.
    let peer_did = "did:plc:rachel-test";
    let rachel_seed = [7u8; 32];
    let (records, rachel_pubkey_hex, tampered_rkey) =
        build_tampered_signature_peer_records(peer_did, rachel_seed, 4);
    assert_eq!(records.len(), 5, "4 honest + 1 tampered = 5 records");

    let peer = PeerPds::for_peer(peer_did, records);

    // Precondition: ONE active subscription via the real `peer add` verb.
    let added = run_openlore_with_peer_resolver(
        &env,
        &["peer", "add", peer_did],
        peer_did,
        peer.endpoint_url(),
    );
    assert_eq!(
        added.status, 0,
        "peer add precondition must succeed;\n--- stdout ---\n{}\n--- stderr ---\n{}",
        added.stdout, added.stderr
    );

    // DD-FED-10 universe BEFORE the pull: the author `claims` table count.
    let author_claims_before = user_author_claim_count_now(&env);

    // Action: `openlore peer pull`. Per-record verify → the tampered record's
    // signature fails → reject ONLY that record → continue with the other 4.
    let outcome = run_openlore_pull(
        &env,
        &["peer", "pull"],
        peer_did,
        peer.endpoint_url(),
        &rachel_pubkey_hex,
    );

    // 1. Non-zero exit overall — a rejected record flags the pull (WD-37 +
    //    ADR-013 exit-code table: pull exits non-zero on ANY rejection).
    assert_ne!(
        outcome.status, 0,
        "a tampered record must drive a NON-ZERO exit code (WD-37 / ADR-013);\n\
         --- stdout ---\n{}\n--- stderr ---\n{}",
        outcome.stdout, outcome.stderr
    );

    // 2. The progress block reports the rejection (rejected count ≥ 1) WITH a
    //    reason. KPI-FED-6 wording: "signature invalid".
    assert!(
        outcome.stdout.contains("rejected  : 1"),
        "the progress block must report exactly one rejected record;\n\
         --- stdout ---\n{}",
        outcome.stdout
    );
    assert!(
        outcome.stdout.contains("signature invalid"),
        "the rejection reason must name the signature failure (KPI-FED-6);\n\
         --- stdout ---\n{}",
        outcome.stdout
    );

    // 3. The 4 honest records still verify + store: the `verified : 4/5`
    //    line (4 of 5 fetched valid) + the per-peer progress names Rachel.
    assert!(
        outcome.stdout.contains(peer_did),
        "the progress block must name the peer DID {peer_did};\n\
         --- stdout ---\n{}",
        outcome.stdout
    );
    assert!(
        outcome.stdout.contains("4/5"),
        "expected 4 of 5 fetched signatures valid (1 tampered rejected);\n\
         --- stdout ---\n{}",
        outcome.stdout
    );
    assert!(
        outcome.stdout.contains("Pulled 4 new peer claims"),
        "the 4 honest records must be stored as NEW;\n--- stdout ---\n{}",
        outcome.stdout
    );

    // 4. DD-FED-10 (LOAD-BEARING) — the storage state-delta:
    //    - peer_claims rows == 4 (honest only); every row attributed to
    //      Rachel, none to any other DID (anti-merging, I-FED-1).
    assert_peer_claims_attributed_to(&env, peer_did, 4);

    //    - the tampered record's CID is ABSENT from peer_claims (under ANY
    //      author) AND has NO on-disk artifact (anti-merging holds even at
    //      the reject path — KPI-FED-6: zero invalid signatures stored).
    assert_peer_claim_cid_absent(&env, &tampered_rkey);

    //    - author_claims UNCHANGED (a peer pull never touches own claims).
    assert_eq!(
        user_author_claim_count_now(&env),
        author_claims_before,
        "the author `claims` table must be UNCHANGED by a peer pull (DD-FED-10)"
    );

    //    - exactly 4 `<cid>.json` artifacts under the peer partition (the 4
    //      honest records only — the tampered one wrote nothing).
    let partition = peer_claims_dir_for(&env, peer_did);
    let artifact_count = std::fs::read_dir(&partition)
        .unwrap_or_else(|e| panic!("read peer_claims partition {}: {e}", partition.display()))
        .filter(|entry| {
            entry
                .as_ref()
                .ok()
                .and_then(|e| e.path().extension().map(|x| x == "json"))
                .unwrap_or(false)
        })
        .count();
    assert_eq!(
        artifact_count,
        4,
        "expected exactly 4 `<cid>.json` artifacts (honest only) under {}; got {artifact_count}",
        partition.display()
    );
}

/// PP-4 / Sad: A peer publishes a record whose published rkey does NOT
/// equal the locally-recomputed CID (canonicalization disagreement —
/// "possible adversarial input"). `openlore peer pull` rejects only
/// that record, stores the others, reports the rejection reason
/// verbatim, and exits non-zero. Drives integration gate
/// `peer_cid_round_trip`.
///
/// @us-fed-002 @real-io @driving_port @j-003a @error
#[test]
fn peer_pull_rejects_cid_mismatch_per_record_and_stores_honest_records() {
    let env = TestEnv::initialized();

    // Rachel publishes THREE genuinely-signed honest claims (real crypto via
    // `build_verifiable_peer_records` — same builder PP-1 uses, so each honest
    // record passes BOTH the CID round-trip AND the signature verify). The
    // `with_cid_mismatch` posture appends ONE adversarial record at
    // `ADVERSARIAL_RKEY`: its body is well-formed and its signature field is
    // intact, but the published rkey does NOT equal the locally-recomputed
    // CID (canonicalization disagreement — "possible adversarial input").
    // This isolates the CID-ROUND-TRIP rejection branch (WD-24), which fires
    // BEFORE the signature verify in `evaluate_record`.
    let peer_did = "did:plc:rachel-test";
    let rachel_seed = [7u8; 32];
    let (honest, rachel_pubkey_hex) = build_verifiable_peer_records(peer_did, rachel_seed);
    assert_eq!(honest.len(), 3, "Rachel publishes three honest claims");

    let peer = PeerPds::with_cid_mismatch(peer_did, honest);
    assert_eq!(
        peer.records().len(),
        4,
        "3 honest + 1 cid-mismatch = 4 records hosted"
    );

    // Precondition: ONE active subscription via the real `peer add` verb.
    let added = run_openlore_with_peer_resolver(
        &env,
        &["peer", "add", peer_did],
        peer_did,
        peer.endpoint_url(),
    );
    assert_eq!(
        added.status, 0,
        "peer add precondition must succeed;\n--- stdout ---\n{}\n--- stderr ---\n{}",
        added.stdout, added.stderr
    );

    // DD-FED-10 universe BEFORE the pull: the author `claims` table count.
    let author_claims_before = user_author_claim_count_now(&env);

    // Action: `openlore peer pull`. Per-record verify recomputes each CID and
    // byte-matches it against the peer-published rkey → the mismatch record is
    // rejected → the other 3 verify + store (per-record fault isolation, WD-37).
    let outcome = run_openlore_pull(
        &env,
        &["peer", "pull"],
        peer_did,
        peer.endpoint_url(),
        &rachel_pubkey_hex,
    );

    // 1. Non-zero exit overall — a rejected record flags the pull (WD-37 +
    //    ADR-013 exit-code table: pull exits non-zero on ANY rejection).
    assert_ne!(
        outcome.status, 0,
        "a CID-mismatch record must drive a NON-ZERO exit code (WD-37 / ADR-013);\n\
         --- stdout ---\n{}\n--- stderr ---\n{}",
        outcome.stdout, outcome.stderr
    );

    // 2. The progress block reports exactly one rejection WITH the verbatim
    //    reason (US-FED-002 UAT scenario #3 wording).
    assert!(
        outcome.stdout.contains("rejected  : 1"),
        "the progress block must report exactly one rejected record;\n\
         --- stdout ---\n{}",
        outcome.stdout
    );
    assert!(
        outcome
            .stdout
            .contains("CID mismatch (possible adversarial input)"),
        "the rejection reason must name the CID-mismatch failure verbatim \
         (US-FED-002 UAT scenario #3);\n--- stdout ---\n{}",
        outcome.stdout
    );

    // 3. The 3 honest records still verify + store: the `verified : 3/4` line
    //    (3 of 4 fetched valid) + the per-peer progress names Rachel.
    assert!(
        outcome.stdout.contains(peer_did),
        "the progress block must name the peer DID {peer_did};\n\
         --- stdout ---\n{}",
        outcome.stdout
    );
    assert!(
        outcome.stdout.contains("3/4"),
        "expected 3 of 4 fetched records valid (1 CID-mismatch rejected);\n\
         --- stdout ---\n{}",
        outcome.stdout
    );
    assert!(
        outcome.stdout.contains("Pulled 3 new peer claims"),
        "the 3 honest records must be stored as NEW;\n--- stdout ---\n{}",
        outcome.stdout
    );

    // 4. DD-FED-10 (LOAD-BEARING) — the storage state-delta:
    //    - peer_claims rows == 3 (honest only); every row attributed to
    //      Rachel, none to any other DID (anti-merging, I-FED-1).
    assert_peer_claims_attributed_to(&env, peer_did, 3);

    //    - the CID-mismatch record's published rkey is ABSENT from peer_claims
    //      (under ANY author) AND has NO on-disk artifact (anti-merging holds
    //      at the reject path — zero adversarial records stored).
    assert_peer_claim_cid_absent(&env, ADVERSARIAL_RKEY);

    //    - author_claims UNCHANGED (a peer pull never touches own claims).
    assert_eq!(
        user_author_claim_count_now(&env),
        author_claims_before,
        "the author `claims` table must be UNCHANGED by a peer pull (DD-FED-10)"
    );

    //    - exactly 3 `<cid>.json` artifacts under the peer partition (the 3
    //      honest records only — the mismatch record wrote nothing).
    let partition = peer_claims_dir_for(&env, peer_did);
    let artifact_count = std::fs::read_dir(&partition)
        .unwrap_or_else(|e| panic!("read peer_claims partition {}: {e}", partition.display()))
        .filter(|entry| {
            entry
                .as_ref()
                .ok()
                .and_then(|e| e.path().extension().map(|x| x == "json"))
                .unwrap_or(false)
        })
        .count();
    assert_eq!(
        artifact_count,
        3,
        "expected exactly 3 `<cid>.json` artifacts (honest only) under {}; got {artifact_count}",
        partition.display()
    );
}

/// PP-5 / Sad (WD-40): A peer publishes a record whose `author` field
/// is the LOCAL USER's DID (`did:plc:test-maria`). Even though the
/// signature might verify against the user's own key (which would
/// indicate key compromise — orthogonal failure), the pull-time write
/// MUST reject with `PeerStorageError::SelfAttribution`. Probe #4 of
/// `PeerStoragePort.probe` mirrors this gate.
///
/// @us-fed-002 @real-io @driving_port @j-003a @error @wd-40
#[test]
fn peer_pull_rejects_self_attribution_at_write_time() {
    todo!("DELIVER (slice-03): wire DuckDbPeerStorageAdapter::write_peer_claim → if author_did == identity.author_did() → reject with PeerStorageError::SelfAttribution. Use FakePeerPds::with_self_attribution + fixture_adversarial_peer_self_attribution. Assert: stderr contains 'self attribution' or equivalent message, peer_claims row count for the offending CID == 0, the OTHER honest records ARE stored (fault isolation per WD-37), exit nonzero")
}

/// PP-6 / Sad (WD-41 — RESOLVES `# DISTILL: confirm` anxiety scenario
/// 1.2): A subscribed peer (Rachel) publishes a record whose `author`
/// field references `did:plc:trusted-third-party-test` (a DID OTHER
/// than Rachel). Per WD-41 this MUST be rejected with
/// `PeerStorageError::CrossAttribution`; slice-03 does NOT silently
/// follow cross-attributed records. The user's trust model is
/// "subscribing to a peer means accepting THEIR claims."
///
/// @us-fed-002 @real-io @driving_port @j-003a @error @wd-41
#[test]
fn peer_pull_rejects_cross_attribution_to_third_party_did_at_write_time() {
    todo!("DELIVER (slice-03): wire DuckDbPeerStorageAdapter::write_peer_claim → if signed.author != subscribed_peer.did → reject with PeerStorageError::CrossAttribution. Use FakePeerPds::with_cross_attribution + fixture_adversarial_peer_cross_attribution. Assert: NO peer_claims row attributed to ANY DID for the cross-attributed record (anti-merging holds at reject path too), other honest records stored, exit nonzero")
}

/// PP-7 / Sad: One of three subscribed peers' PDSes is currently
/// unreachable. `openlore peer pull` reports
/// "peer did:plc:down-test: PDS unreachable (connection refused);
/// skipping", proceeds with the other two peers (storing their records
/// normally), and exits non-zero overall to flag the partial failure
/// (WD-37 fault isolation + ADR-013 exit-code table).
///
/// @us-fed-002 @real-io @driving_port @j-003 @error
#[test]
fn peer_pull_skips_unreachable_peer_and_proceeds_with_others() {
    todo!("DELIVER (slice-03): wire VerbPeerPull → iterate subscriptions sequentially per WD-37 → each peer's PdsError::Unreachable becomes a skip-with-message, not a process abort. Use three FakePeerPds doubles, one with simulate_unreachable(). Assert per-peer summary lines + total peer_claims row count = sum(non-skipped peers' records) + exit code != 0")
}

/// PP-8: `openlore peer pull` against ZERO subscribed peers exits 0
/// with a "no peers subscribed" stdout line and writes nothing. (cli
/// probe #4 of ADR-013 §Earned Trust; ADR-016 pull-on-demand-only.)
///
/// @us-fed-002 @real-io @driving_port @j-003 @edge
#[test]
fn peer_pull_with_zero_subscriptions_prints_no_peers_subscribed_and_exits_zero() {
    todo!("DELIVER (slice-03): wire VerbPeerPull early-return on empty list_active_subscriptions; assert stdout contains 'no peers subscribed' (case-insensitive) + exit 0 + zero peer_claims rows + zero filesystem writes under peer_claims/ directory tree")
}
