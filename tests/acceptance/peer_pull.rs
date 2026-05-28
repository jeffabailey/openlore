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
    let env = TestEnv::initialized();

    // Rachel publishes THREE genuinely-signed honest claims (real crypto via
    // `build_verifiable_peer_records` — the same builder PP-1 uses, so each
    // honest record passes BOTH the CID round-trip AND the signature verify).
    // The `with_self_attribution` posture appends ONE adversarial record at
    // `ADVERSARIAL_RKEY` whose `author` field is the LOCAL USER's DID — the
    // WD-40 key-compromise vector. Even if that record's signature verified
    // against the user's own key, the pull-time WRITE must reject it with
    // `PeerStorageError::SelfAttribution`: the storage-layer guard (layer 2)
    // is an INDEPENDENT defense from the pure pre-check in `evaluate_record`
    // (layer 1). The local user here is `FakeIdentity::jeff` (the DID
    // `TestEnv::initialized` binds), so the offending record's author is wired
    // to exactly `env.identity.author_did()`.
    let peer_did = "did:plc:rachel-test";
    let rachel_seed = [7u8; 32];
    let (honest, rachel_pubkey_hex) = build_verifiable_peer_records(peer_did, rachel_seed);
    assert_eq!(honest.len(), 3, "Rachel publishes three honest claims");

    let local_did = env.identity.author_did().to_string();
    let peer = PeerPds::with_self_attribution(peer_did, &local_did, honest);
    assert_eq!(
        peer.records().len(),
        4,
        "3 honest + 1 self-attributed = 4 records hosted"
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

    // Action: `openlore peer pull`. The self-attributed record is rejected
    // (the storage-layer SelfAttribution guard mirrors the pure pre-check);
    // the 3 honest records verify + store (per-record fault isolation, WD-37).
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
        "a self-attributed record must drive a NON-ZERO exit code (WD-40 / WD-37 / ADR-013);\n\
         --- stdout ---\n{}\n--- stderr ---\n{}",
        outcome.stdout, outcome.stderr
    );

    // 2. The progress block reports exactly one rejection WITH the
    //    self-attribution reason. The pull renderer surfaces per-record reject
    //    reasons on stdout (WD-37 + ADR-013) so the user sees WHY the record
    //    was dropped, not just a count.
    assert!(
        outcome.stdout.contains("rejected  : 1"),
        "the progress block must report exactly one rejected record;\n\
         --- stdout ---\n{}",
        outcome.stdout
    );
    assert!(
        outcome.stdout.contains("self attribution"),
        "the rejection reason must name the self-attribution failure (WD-40);\n\
         --- stdout ---\n{}",
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
        "expected 3 of 4 fetched records valid (1 self-attributed rejected);\n\
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
    //      Rachel, none to any other DID (anti-merging, I-FED-1). Crucially
    //      NO row is attributed to the LOCAL user (I-FED-2: peer_claims.author_did
    //      NEVER == local user).
    assert_peer_claims_attributed_to(&env, peer_did, 3);

    //    - the self-attributed record's CID is ABSENT from peer_claims (under
    //      ANY author — including the local user's DID) AND has NO on-disk
    //      artifact (the WD-40 storage guard holds at the reject path).
    assert_peer_claim_cid_absent(&env, ADVERSARIAL_RKEY);

    //    - author_claims UNCHANGED (a peer pull never touches own claims; the
    //      self-attributed record did NOT leak into the user's own table).
    assert_eq!(
        user_author_claim_count_now(&env),
        author_claims_before,
        "the author `claims` table must be UNCHANGED by a peer pull, even when a \
         record self-attributes to the local user (DD-FED-10 / I-FED-2)"
    );

    //    - exactly 3 `<cid>.json` artifacts under the peer partition (the 3
    //      honest records only — the self-attributed record wrote nothing).
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
    let env = TestEnv::initialized();

    // Rachel publishes THREE genuinely-signed honest claims PLUS one
    // adversarial record whose `author` field references a THIRD PARTY
    // (`did:plc:trusted-third-party-test`), NOT Rachel. Crucially the
    // cross-attributed record is signed by RACHEL's OWN key and is
    // CID-consistent (rkey == compute_cid(body)), so it PASSES the pure
    // layer-1 pre-check (`evaluate_record`: CID round-trip + signature verify
    // against Rachel's DID-doc key). It is therefore NOT caught at layer 1 —
    // it reaches `PeerStoragePort::write_peer_claim`, whose WRITE-TIME guard
    // (WD-41) rejects it with `PeerStorageError::CrossAttribution` because the
    // record's author (the third party) does NOT equal the SUBSCRIBED peer's
    // DID. This isolates the write-time author-vs-subscribed-peer guard as the
    // ONLY thing that can reject this record. The trust model: "subscribing to
    // a peer means accepting THEIR claims; cross-attributed records are out of
    // scope." No back-door "follow Rachel → auto-follow Tobias."
    let peer_did = "did:plc:rachel-test";
    let third_party_did = "did:plc:trusted-third-party-test";
    let rachel_seed = [7u8; 32];
    let (records, rachel_pubkey_hex, cross_rkey) =
        build_verifiable_cross_attribution_peer_records(peer_did, third_party_did, rachel_seed);
    assert_eq!(
        records.len(),
        4,
        "3 honest + 1 cross-attributed = 4 records hosted"
    );

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

    // Action: `openlore peer pull`. The cross-attributed record verifies +
    // round-trips at layer 1, reaches `write_peer_claim`, and is rejected by
    // the WRITE-TIME CrossAttribution guard; the 3 honest records verify +
    // store (per-record fault isolation, WD-37).
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
        "a cross-attributed record must drive a NON-ZERO exit code (WD-41 / WD-37 / ADR-013);\n\
         --- stdout ---\n{}\n--- stderr ---\n{}",
        outcome.stdout, outcome.stderr
    );

    // 2. The progress block reports exactly one rejection WITH the
    //    cross-attribution reason. The pull renderer surfaces per-record
    //    reject reasons on stdout (WD-37 + ADR-013) so the user sees WHY the
    //    record was dropped, not just a count.
    assert!(
        outcome.stdout.contains("rejected  : 1"),
        "the progress block must report exactly one rejected record;\n\
         --- stdout ---\n{}",
        outcome.stdout
    );
    assert!(
        outcome.stdout.contains("cross attribution"),
        "the rejection reason must name the cross-attribution failure (WD-41);\n\
         --- stdout ---\n{}",
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
        "expected 3 of 4 fetched records valid (1 cross-attributed rejected);\n\
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

    //    - the cross-attributed record's CID is ABSENT from peer_claims (under
    //      ANY author — crucially the third party gets ZERO rows: no back-door
    //      "follow Rachel → auto-follow Tobias") AND has NO on-disk artifact
    //      (the WD-41 write-time guard holds at the reject path).
    assert_peer_claim_cid_absent(&env, &cross_rkey);

    //    - the third-party DID has ZERO rows attributed to it (the explicit
    //      anti-back-door invariant of WD-41 — subscribing to Rachel never
    //      silently follows a third party Rachel cross-publishes for).
    assert_no_peer_claims_attributed_to(&env, third_party_did);

    //    - author_claims UNCHANGED (a peer pull never touches own claims; the
    //      cross-attributed record did NOT leak into the user's own table).
    assert_eq!(
        user_author_claim_count_now(&env),
        author_claims_before,
        "the author `claims` table must be UNCHANGED by a peer pull, even when a \
         record cross-attributes to a third party (DD-FED-10)"
    );

    //    - exactly 3 `<cid>.json` artifacts under the peer partition (the 3
    //      honest records only — the cross-attributed record wrote nothing).
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
        "expected exactly 3 `<cid>.json` artifacts (honest only) under {} after the cross-\
         attribution pull; got {artifact_count}",
        partition.display()
    );
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
    let env = TestEnv::initialized();

    // THREE subscribed peers, each on its own in-process PeerPds. Rachel and
    // Sam are honest + reachable (real crypto via `build_verifiable_peer_records`
    // so their records pass BOTH the CID round-trip AND the signature verify).
    // The third peer (Dana) is the one we knock offline. Each peer DID is baked
    // into its records' `author` field, so the three DIDs produce three disjoint
    // CID sets — no aliasing across peers.
    let rachel_did = "did:plc:rachel-test";
    let sam_did = "did:plc:sam-test";
    let dana_did = "did:plc:dana-test";

    let (rachel_records, rachel_pubkey_hex) = build_verifiable_peer_records(rachel_did, [7u8; 32]);
    let (sam_records, sam_pubkey_hex) = build_verifiable_peer_records(sam_did, [9u8; 32]);
    let (dana_records, dana_pubkey_hex) = build_verifiable_peer_records(dana_did, [11u8; 32]);
    assert_eq!(rachel_records.len(), 3, "Rachel publishes three claims");
    assert_eq!(sam_records.len(), 3, "Sam publishes three claims");
    assert_eq!(dana_records.len(), 3, "Dana publishes three claims");

    let rachel = PeerPds::for_peer(rachel_did, rachel_records);
    let sam = PeerPds::for_peer(sam_did, sam_records);
    let dana = PeerPds::for_peer(dana_did, dana_records);

    // Precondition: THREE active subscriptions, created through the real
    // `peer add` verb WHILE every peer is still reachable (the `peer add`
    // resolveDid round-trip needs the peer's PDS up). Dana goes offline
    // only AFTER she is subscribed, so the pull-time fault is genuinely a
    // per-peer transport failure, not a missing subscription.
    for (peer_did, peer) in [(rachel_did, &rachel), (sam_did, &sam), (dana_did, &dana)] {
        let added = run_openlore_with_peer_resolver(
            &env,
            &["peer", "add", peer_did],
            peer_did,
            peer.endpoint_url(),
        );
        assert_eq!(
            added.status, 0,
            "peer add precondition for {peer_did} must succeed;\n\
             --- stdout ---\n{}\n--- stderr ---\n{}",
            added.stdout, added.stderr
        );
    }

    // DD-FED-10 universe BEFORE the pull: the author `claims` table count
    // (a peer pull must never touch the user's own claims, even partially).
    let author_claims_before = user_author_claim_count_now(&env);

    // Knock Dana's PDS offline: subsequent resolveDid / listRecords HTTP
    // calls drop the connection without responding, which the in-binary
    // adapter lifts into a per-peer transport failure (WD-37). Rachel and
    // Sam stay reachable.
    dana.simulate_unreachable();

    // Action: ONE `openlore peer pull` across all three subscriptions. The
    // sequential per-peer loop (ADR-016 — no concurrency) verifies + stores
    // Rachel's and Sam's records and records Dana as a skip; Dana's failure
    // does NOT abort the loop (WD-37 per-peer fault isolation).
    let outcome = run_openlore_pull_multi(
        &env,
        &["peer", "pull"],
        &[
            PeerSeam {
                peer_did: rachel_did,
                peer_endpoint: rachel.endpoint_url(),
                peer_pubkey_hex: &rachel_pubkey_hex,
            },
            PeerSeam {
                peer_did: sam_did,
                peer_endpoint: sam.endpoint_url(),
                peer_pubkey_hex: &sam_pubkey_hex,
            },
            PeerSeam {
                peer_did: dana_did,
                peer_endpoint: dana.endpoint_url(),
                peer_pubkey_hex: &dana_pubkey_hex,
            },
        ],
    );

    // 1. NON-ZERO exit overall — a skipped peer flags the partial failure
    //    (WD-37 fault isolation + ADR-013 exit-code table: pull exits
    //    non-zero on ANY peer skip).
    assert_ne!(
        outcome.status, 0,
        "an unreachable peer must drive a NON-ZERO exit code (WD-37 / ADR-013);\n\
         --- stdout ---\n{}\n--- stderr ---\n{}",
        outcome.stdout, outcome.stderr
    );

    // 2. The progress block names the unreachable peer AND marks it skipped
    //    (the user sees WHICH peer was dropped + that the pull continued —
    //    the defining observable of per-peer fault isolation, WD-37). The
    //    EXACT skip reason wording is an implementation detail of which seam
    //    trips first (DID resolution vs listRecords); the load-bearing
    //    contract is "Dana is named under a `skipped` line".
    assert!(
        outcome.stdout.contains(dana_did),
        "the progress block must name the unreachable peer {dana_did};\n\
         --- stdout ---\n{}",
        outcome.stdout
    );
    assert!(
        outcome.stdout.contains("skipped"),
        "the progress block must mark the unreachable peer as skipped (WD-37);\n\
         --- stdout ---\n{}",
        outcome.stdout
    );

    // 3. The TWO reachable peers proceed NORMALLY: each is named, each
    //    reports its fetched/verified counts, and the run stored their
    //    records as new (Dana's failure isolated to Dana — WD-37).
    for reachable_did in [rachel_did, sam_did] {
        assert!(
            outcome.stdout.contains(reachable_did),
            "the progress block must name the reachable peer {reachable_did};\n\
             --- stdout ---\n{}",
            outcome.stdout
        );
    }
    assert!(
        outcome.stdout.contains("Pulled 6 new peer claims"),
        "the two reachable peers' 3+3 records must all store as new \
         (the unreachable peer contributes ZERO, never aborts the others);\n\
         --- stdout ---\n{}",
        outcome.stdout
    );

    // 4. DD-FED-10 (LOAD-BEARING) — the storage state-delta:
    //    - peer_claims rows attributed to Rachel : 0 → 3 (every row hers).
    //    - peer_claims rows attributed to Sam    : 0 → 3 (every row his).
    //    The per-author helper composes (it asserts only the per-DID count),
    //    so the two reachable peers' partitions coexist.
    assert_peer_claims_row_count_for(&env, rachel_did, 3);
    assert_peer_claims_row_count_for(&env, sam_did, 3);

    //    - peer_claims rows attributed to Dana : STILL 0 — the unreachable
    //      peer stored NOTHING (anti-merging at the skip path; the sum of
    //      non-skipped peers' records == total).
    assert_no_peer_claims_attributed_to(&env, dana_did);

    //    - author_claims UNCHANGED (a peer pull never touches own claims,
    //      not even partially when one peer is skipped — DD-FED-10).
    assert_eq!(
        user_author_claim_count_now(&env),
        author_claims_before,
        "the author `claims` table must be UNCHANGED by a peer pull, even when \
         one of several peers is skipped (DD-FED-10)"
    );

    //    - the on-disk partitions for the two reachable peers each hold
    //      exactly 3 `<cid>.json` artifacts; Dana's partition was never
    //      created (no records fetched).
    for reachable_did in [rachel_did, sam_did] {
        let partition = peer_claims_dir_for(&env, reachable_did);
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
            "expected exactly 3 `<cid>.json` artifacts under {} for the reachable \
             peer {reachable_did}; got {artifact_count}",
            partition.display()
        );
    }
    assert!(
        !peer_claims_dir_for(&env, dana_did).exists(),
        "the unreachable peer {dana_did} must have NO on-disk partition (nothing \
         was fetched or stored — WD-37 skip path leaves zero residue)"
    );
}

/// PP-8: `openlore peer pull` against ZERO subscribed peers exits 0
/// with a "no peers subscribed" stdout line and writes nothing. (cli
/// probe #4 of ADR-013 §Earned Trust; ADR-016 pull-on-demand-only.)
///
/// @us-fed-002 @real-io @driving_port @j-003 @edge
#[test]
fn peer_pull_with_zero_subscriptions_prints_no_peers_subscribed_and_exits_zero() {
    // A freshly-initialized env has run `openlore init` ONLY — no `peer add`,
    // so `list_active_subscriptions` is empty. This is the pull-on-demand-only
    // clean no-op (ADR-016): the user runs `peer pull` before subscribing to
    // anyone. Distinct from PP-7 (a peer SKIP → non-zero); an EMPTY list is a
    // clean no-op → exit ZERO.
    let env = TestEnv::initialized();

    // Action: `openlore peer pull` with ZERO subscriptions. No peer resolver /
    // pubkey seams are wired — the empty-list early-return fires before any
    // peer wiring is reached, so the plain runner is the correct driving-port
    // entry.
    let outcome = run_openlore(&env, &["peer", "pull"]);

    // 1. Exit ZERO + a "no peers subscribed" hint on stdout (ADR-013 §Earned
    //    Trust #4). Asserted case-insensitively so the exact casing of the
    //    rendered line is not over-pinned (the load-bearing contract is the
    //    presence of the hint, not its capitalization).
    assert_eq!(
        outcome.status, 0,
        "an empty subscription list is a clean no-op, NOT an error — exit must be \
         ZERO (distinct from PP-7's peer-skip non-zero);\n--- stdout ---\n{}\n\
         --- stderr ---\n{}",
        outcome.stdout, outcome.stderr
    );
    assert!(
        outcome.stdout.to_lowercase().contains("no peers subscribed"),
        "expected the no-op to print a 'no peers subscribed' hint on stdout \
         (ADR-013 §Earned Trust #4);\n--- stdout ---\n{}",
        outcome.stdout
    );

    // 2. The hint points the user at `peer add` (the next step to take —
    //    journey step 2 empty-subscription-list orientation).
    assert!(
        outcome.stdout.contains("peer add"),
        "the no-op hint must point the user at `peer add`;\n--- stdout ---\n{}",
        outcome.stdout
    );

    // 3. ZERO peer_claims rows written. A pull that subscribes to nobody must
    //    not touch the peer_claims store at all. After `init` only, the
    //    `peer_claims` table may not even exist yet — its ABSENCE is the
    //    strongest possible form of "zero rows written". If the table DOES
    //    exist (created at init), assert it is empty.
    assert_peer_claims_store_empty(&env);

    // 4. ZERO filesystem writes under the `peer_claims/` directory tree. The
    //    no-op writes no artifacts; the partition root is either absent or
    //    empty.
    assert_peer_claims_dir_tree_empty(&env);
}

/// Universe-bound: "the `peer_claims` store holds ZERO rows (under ANY
/// author)". Port-exposed name: `peer_storage.claims.row_count`.
///
/// A clean no-op pull (PP-8) must not touch the peer_claims store. After
/// `openlore init` only, the `peer_claims` TABLE may not exist yet — its
/// absence is the strongest "zero rows" signal, so a missing-table query
/// error is treated as zero. If the table exists, assert the count is zero.
fn assert_peer_claims_store_empty(env: &TestEnv) {
    let db_path = env.duckdb_path();
    if !db_path.exists() {
        return;
    }
    let conn = duckdb::Connection::open(&db_path).unwrap_or_else(|err| {
        panic!(
            "open DuckDB at {} for empty-store assertion: {err}",
            db_path.display()
        )
    });
    // Missing table ⇒ zero rows (no peer claim was ever written).
    let total: i64 = match conn.query_row("SELECT count(*) FROM peer_claims", [], |r| r.get(0)) {
        Ok(n) => n,
        Err(_) => return,
    };
    assert_eq!(
        total, 0,
        "a zero-subscription pull must write ZERO peer_claims rows; got {total}"
    );
}

/// Universe-bound: "no file exists anywhere under the `peer_claims/`
/// directory tree". Port-exposed name: `filesystem.peer_claims_tree.file_count`.
///
/// The no-op writes no artifacts, so the partition root is either absent
/// (strongest form of "nothing written") or present-but-empty.
fn assert_peer_claims_dir_tree_empty(env: &TestEnv) {
    let peer_claims_root = env
        .home
        .join(".local")
        .join("share")
        .join("openlore")
        .join("peer_claims");
    if !peer_claims_root.exists() {
        return;
    }
    let entries: Vec<_> = std::fs::read_dir(&peer_claims_root)
        .unwrap_or_else(|e| panic!("read peer_claims root {}: {e}", peer_claims_root.display()))
        .filter_map(|e| e.ok())
        .collect();
    assert!(
        entries.is_empty(),
        "a zero-subscription pull must write nothing under the peer_claims/ tree \
         at {} but found {} entries: {:?}",
        peer_claims_root.display(),
        entries.len(),
        entries.iter().map(|e| e.file_name()).collect::<Vec<_>>()
    );
}
