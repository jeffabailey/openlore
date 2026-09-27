//! serverless-philosophy-federation — cross-instance federated read.
//!
//! Slice-05: US-SF-006 (expand my dataset by pulling from ANOTHER user's
//! Cloudflare instance — federated read, J-003 transport delta; requirement 4).
//! This REUSES the shipped J-003 `openlore peer add` / `openlore peer pull`
//! flow verbatim; the ONLY delta is that the peer's DID document
//! `serviceEndpoint` resolves to their Cloudflare instance instead of a
//! bsky.social PDS (OD-SF-3 / DDD-5). Per ADR-062 §4 the record transport is
//! the byte-preserving OPAQUE read (`GET /manifest` + `GET /records/:cid`), and
//! the Rust side recomputes the CID + verifies the signature LOCALLY — reusing
//! J-003's verify / per-author attribution / anti-merging (peer claims land in
//! `peer_claims`, never merged with the reader's own).
//!
//! Only the peer's CLI↔Worker seam is faked (`FakeInstance` as a peer, with the
//! DID-doc resolver pointing at it); the local verify/store path is REAL.
//! Layer 3; example-only (Mandate 11 — tampered/unreachable sad paths are named
//! examples, never PBT). The J-003 verify machinery itself is already shipped
//! and unit-tested; these scenarios assert the TRANSPORT DELTA only.

mod support;

#[allow(unused_imports)]
use support::*;

// =============================================================================
// US-SF-006 — happy path
// =============================================================================

/// CI-1: Rachel's DID `did:plc:rachel-test` resolves her serviceEndpoint to
/// `https://openlore.rachel.workers.dev`. Maria has subscribed. When she runs
/// `openlore peer pull`, Rachel's claims are fetched from her Cloudflare
/// instance via the opaque read, each signature is verified and each CID
/// recomputed before storage, and the claims land in `peer_claims` attributed
/// to Rachel — never merged with Maria's own. (US-SF-006 · AC 1-2-3 · UAT #1 ·
/// Domain example 1.)
///
/// @us-sf-006 @driving_port @real-io @j-003 @happy
#[test]
fn peer_pull_fetches_a_peers_claims_from_their_cloudflare_instance_into_peer_claims_attributed() {
    use support::state_delta::{assert_state_delta, set_to, Delta};

    // Given Rachel has published 7 REAL-signed claims.
    let env = TestEnv::initialized_as(FakeIdentity::maria());
    let rachel = build_verifiable_peer_instance_records(RACHEL_DID, RACHEL_SEED, 7);

    // And Maria subscribed (real `peer add`) back when Rachel's DID resolved
    // to a bsky PDS, and pulled the 2 claims Rachel had published then —
    // so 2 of the 7 are already in peer_claims and 5 are new to Maria.
    subscribe_and_pull_earlier_from_pds(
        &env,
        RACHEL_DID,
        rachel.pds_records[..2].to_vec(),
        &rachel.pubkey_hex,
    );

    // And Rachel's DID now resolves to her Cloudflare instance holding all 7.
    let instance = FakeInstance::for_peer(RACHEL_DID, rachel.instance_records.clone());
    let before = capture_cross_instance_pull_universe(&env, RACHEL_DID);

    // When Maria pulls.
    let pull = run_openlore_pull(
        &env,
        &["peer", "pull"],
        RACHEL_DID,
        instance.endpoint_url(),
        &rachel.pubkey_hex,
    );
    let after = capture_cross_instance_pull_universe(&env, RACHEL_DID);

    // Then the pull succeeds: 7 fetched, 5 new, 5/5 verified, none merged.
    assert_exit_zero_and_stdout_contains(&pull, "None merged with your own claims");
    for needle in [
        "fetched   : 7 records",
        "new       : 5 (2 already in peer_claims, skipped)",
        "verified  : 5/5",
    ] {
        assert!(
            pull.stdout.contains(needle),
            "expected `{needle}` in the pull report;\n--- stdout ---\n{}\n--- stderr ---\n{}",
            pull.stdout,
            pull.stderr
        );
    }

    // And every record crossed the OPAQUE read (GET /records/:cid), with the
    // peer's instance only ever read.
    for record in &rachel.instance_records {
        assert_instance_served_record(&instance, &record.cid);
    }
    assert_peer_instance_only_read(&instance);

    // And (CM-E) +5 rows attributed to Rachel; Maria's own claims untouched.
    let rachel_before: usize = before[&peer_row_count_slot(RACHEL_DID)]
        .parse()
        .expect("row count");
    let expected = Delta::new().with_slot(
        peer_row_count_slot(RACHEL_DID),
        set_to((rachel_before + 5).to_string()),
    );
    assert_state_delta(
        &before,
        &after,
        &cross_instance_pull_universe(RACHEL_DID),
        &expected,
    );
    assert_peer_claims_attributed_to(&env, RACHEL_DID, 7);
}

// =============================================================================
// US-SF-006 — error (tampered peer claim)
// =============================================================================

/// CI-2: One of Rachel's published records fails signature verification. When
/// Maria runs `openlore peer pull`, that record is rejected and reported, the
/// valid records are stored normally, and the exit code is non-zero to flag the
/// rejection (J-003 semantics carry over unchanged). (US-SF-006 · AC 3 · UAT #2
/// · Domain example 3.)
///
/// @us-sf-006 @error @j-003
#[test]
fn peer_pull_rejects_a_tampered_peer_claim_and_stores_the_valid_ones() {
    // Given Rachel's Cloudflare instance serves 4 honest REAL-signed claims
    // plus ONE whose signature was tampered after signing. Its CID is still
    // honest (the CID covers only the unsigned claim), so ONLY the signature
    // verify can catch it.
    let env = TestEnv::initialized_as(FakeIdentity::maria());
    let (pds_records, rachel_pubkey_hex, tampered_cid) =
        build_tampered_signature_peer_records(RACHEL_DID, RACHEL_SEED, 4);
    let served: Vec<PreloadedRecord> = pds_records.iter().map(instance_record_of).collect();
    let honest_cids: Vec<String> = served
        .iter()
        .map(|record| record.cid.clone())
        .filter(|cid| cid != &tampered_cid)
        .collect();
    assert_eq!(honest_cids.len(), 4, "fixture: 4 honest + 1 tampered");
    let instance = FakeInstance::for_peer(RACHEL_DID, served);

    // And Maria subscribed to Rachel via the real `peer add`.
    subscribe_to_peer_instance(&env, RACHEL_DID, &instance);

    // When Maria pulls.
    let pull = run_openlore_pull(
        &env,
        &["peer", "pull"],
        RACHEL_DID,
        instance.endpoint_url(),
        &rachel_pubkey_hex,
    );

    // Then the tampered record was READ through the opaque transport, then
    // rejected and reported by its signature failure; the pull exits non-zero.
    assert_instance_served_record(&instance, &tampered_cid);
    assert_ne!(
        pull.status, 0,
        "a rejected record flags the pull non-zero (KPI-FED-6);\n--- stdout ---\n{}\n--- stderr ---\n{}",
        pull.stdout, pull.stderr
    );
    assert!(
        pull.stdout.contains("signature invalid"),
        "the rejection reason must name the signature failure;\n--- stdout ---\n{}",
        pull.stdout
    );

    // And exactly the 4 honest records are stored, attributed to Rachel; the
    // tampered record is never stored.
    let mut expected: Vec<(String, String)> = honest_cids
        .iter()
        .map(|cid| (cid.clone(), RACHEL_DID.to_string()))
        .collect();
    expected.sort();
    assert_eq!(
        peer_claim_rows(&env),
        expected,
        "exactly the honest records land in peer_claims;\n--- stdout ---\n{}",
        pull.stdout
    );
    assert_peer_claim_cid_absent(&env, &tampered_cid);
    assert!(
        local_claim_cids(&env).is_empty(),
        "a peer pull never writes into Maria's own claims"
    );
    assert_peer_instance_only_read(&instance);
}

// =============================================================================
// US-SF-006 — boundary (unreachable peer instance)
// =============================================================================

/// CI-3: Rachel's Cloudflare instance is unreachable. When Maria runs
/// `openlore peer pull` with multiple subscribed peers, Rachel's pull is skipped
/// and reported, the other peers' pulls proceed, and the exit code is non-zero
/// overall (same fault isolation as any unreachable PDS in J-003). (US-SF-006 ·
/// UAT #3 · Domain example 2.)
///
/// @us-sf-006 @error @j-003
#[test]
fn peer_pull_skips_only_the_unreachable_peer_instance_and_proceeds_with_others() {
    // Given Maria follows two peers who each moved to their own Cloudflare
    // instance: Tobias (reachable, 3 REAL-signed claims) and Rachel.
    let env = TestEnv::initialized_as(FakeIdentity::maria());
    let tobias = build_verifiable_peer_instance_records(TOBIAS_DID, TOBIAS_SEED, 3);
    let rachel = build_verifiable_peer_instance_records(RACHEL_DID, RACHEL_SEED, 3);
    let tobias_instance = FakeInstance::for_peer(TOBIAS_DID, tobias.instance_records.clone());
    subscribe_to_peer_instance(&env, TOBIAS_DID, &tobias_instance);
    {
        // Rachel was subscribed (real `peer add`) while her instance was up ...
        let rachel_while_up = FakeInstance::for_peer(RACHEL_DID, rachel.instance_records.clone());
        subscribe_to_peer_instance(&env, RACHEL_DID, &rachel_while_up);
    }
    // ... and her DID now resolves to an instance that is unreachable.
    let rachel_down = FakeInstance::unreachable();

    // When Maria pulls both peers in ONE invocation.
    let pull = run_openlore_pull_multi(
        &env,
        &["peer", "pull"],
        &[
            PeerSeam {
                peer_did: TOBIAS_DID,
                peer_endpoint: tobias_instance.endpoint_url(),
                peer_pubkey_hex: &tobias.pubkey_hex,
            },
            PeerSeam {
                peer_did: RACHEL_DID,
                peer_endpoint: rachel_down.endpoint_url(),
                peer_pubkey_hex: &rachel.pubkey_hex,
            },
        ],
    );

    // Then the pull exits non-zero overall and names Rachel as skipped.
    assert_ne!(
        pull.status, 0,
        "an unreachable peer instance flags the pull non-zero;\n--- stdout ---\n{}\n--- stderr ---\n{}",
        pull.stdout, pull.stderr
    );
    let rachel_block_is_skipped = pull
        .stdout
        .lines()
        .skip_while(|line| !line.contains(RACHEL_DID))
        .take_while(|line| !line.contains(TOBIAS_DID))
        .any(|line| line.trim_start().starts_with("skipped"));
    assert!(
        rachel_block_is_skipped,
        "the report must name {RACHEL_DID} and mark it skipped;\n--- stdout ---\n{}",
        pull.stdout
    );

    // And Tobias's pull proceeded: every record crossed his opaque read and
    // exactly his 3 claims are stored, attributed to him; nothing is stored
    // for Rachel and Maria's own claims are untouched.
    for record in &tobias.instance_records {
        assert_instance_served_record(&tobias_instance, &record.cid);
    }
    let mut expected: Vec<(String, String)> = tobias
        .instance_records
        .iter()
        .map(|record| (record.cid.clone(), TOBIAS_DID.to_string()))
        .collect();
    expected.sort();
    assert_eq!(
        peer_claim_rows(&env),
        expected,
        "only the reachable peer's claims land in peer_claims;\n--- stdout ---\n{}",
        pull.stdout
    );
    assert!(
        local_claim_cids(&env).is_empty(),
        "a peer pull never writes into Maria's own claims"
    );
    assert_peer_instance_only_read(&tobias_instance);
}

// =============================================================================
// US-SF-006 — guardrail (anti-merging / peer_claims separation)
// =============================================================================

/// CI-4: A cross-instance pull NEVER merges peer claims with the reader's own.
/// Peer claims land in the separate `peer_claims` layer; `author_claims` are
/// never modified (J-003 invariant, carried over). (US-SF-006 · AC 2 ·
/// anti-merging.)
///
/// @us-sf-006 @anti-merging @j-003 @guardrail
#[test]
fn peer_pull_from_an_instance_never_merges_peer_claims_with_my_own() {
    use support::state_delta::{assert_state_delta, set_to, Delta};

    // Given Maria has her OWN signed claims in her local store.
    let env = TestEnv::initialized_as(FakeIdentity::maria());
    let own_cids = [
        seed_one_signed_local_claim(&env, 0.5),
        seed_one_signed_local_claim(&env, 0.86),
    ];

    // And she follows Rachel (real `peer add`; 2 of Rachel's 7 claims pulled
    // earlier over the PDS), whose DID now resolves to her Cloudflare
    // instance holding all 7.
    let rachel = build_verifiable_peer_instance_records(RACHEL_DID, RACHEL_SEED, 7);
    subscribe_and_pull_earlier_from_pds(
        &env,
        RACHEL_DID,
        rachel.pds_records[..2].to_vec(),
        &rachel.pubkey_hex,
    );
    let instance = FakeInstance::for_peer(RACHEL_DID, rachel.instance_records.clone());
    let before = capture_cross_instance_pull_universe(&env, RACHEL_DID);
    assert_eq!(
        before["author_claims.row_count"], "2",
        "precondition: Maria's two own claims are in her local store"
    );

    // When Maria pulls from Rachel's instance.
    let pull = run_openlore_pull(
        &env,
        &["peer", "pull"],
        RACHEL_DID,
        instance.endpoint_url(),
        &rachel.pubkey_hex,
    );
    let after = capture_cross_instance_pull_universe(&env, RACHEL_DID);
    assert_exit_zero_and_stdout_contains(&pull, "None merged with your own claims");

    // Then (CM-E) peer_claims grows by 5 for Rachel while Maria's own
    // author_claims {row_count, cids} are UNCHANGED (implicit-unchanged slots).
    let rachel_before: usize = before[&peer_row_count_slot(RACHEL_DID)]
        .parse()
        .expect("row count");
    let expected = Delta::new().with_slot(
        peer_row_count_slot(RACHEL_DID),
        set_to((rachel_before + 5).to_string()),
    );
    assert_state_delta(
        &before,
        &after,
        &cross_instance_pull_universe(RACHEL_DID),
        &expected,
    );

    // And no pulled row is attributed to a DID other than its TRUE author:
    // each peer_claims row's author is the DID its instance bytes were
    // signed as, and none is Maria.
    let rows = peer_claim_rows(&env);
    assert_eq!(
        rows.len(),
        7,
        "exactly Rachel's 7 claims in peer_claims: {rows:?}"
    );
    for (cid, attributed_to) in &rows {
        let bytes = instance
            .record_bytes(cid)
            .unwrap_or_else(|| panic!("peer_claims row {cid} is not one of Rachel's records"));
        let (_, true_author) = recompute_record(&bytes);
        assert_eq!(
            attributed_to, &true_author,
            "peer_claims row {cid} must be attributed to its true author"
        );
        assert_ne!(
            attributed_to,
            env.identity.author_did(),
            "a pulled claim must never be attributed to Maria"
        );
    }

    // And nothing was merged INTO Maria's own claims: her store holds
    // exactly her own CIDs, none of Rachel's.
    let mut own_sorted = own_cids.to_vec();
    own_sorted.sort();
    assert_eq!(local_claim_cids(&env), own_sorted);
}

// =============================================================================
// US-SF-006 — verify-before-trust on the opaque read (KPI-SF-1 / J-003)
// =============================================================================

/// CI-5: The opaque read is byte-preserving, so the Rust side recomputes each
/// peer claim's CID from the returned bytes and verifies it BEFORE store —
/// exactly the round-trip integrity discipline that makes cross-instance reuse
/// "free" (ADR-062 §4). A byte-divergent read would fail the recompute and be
/// rejected. (US-SF-006 · KPI-SF-1 · J-003 verify-before-trust.)
///
/// @us-sf-006 @kpi-sf-1 @j-003
#[test]
fn peer_pull_recomputes_each_peer_claim_cid_from_the_opaque_read_before_store() {
    // Given Rachel's instance serves 5 claims byte-verbatim, plus ONE record
    // whose bytes DIVERGED from its key (a byte-divergent read: its listed
    // CID is honest, its returned bytes are not the ones that were signed).
    let env = TestEnv::initialized_as(FakeIdentity::maria());
    let rachel = build_verifiable_peer_instance_records(RACHEL_DID, RACHEL_SEED, 6);
    let (verbatim, divergent_source) = rachel.instance_records.split_at(5);
    let divergent = byte_divergent(&divergent_source[0]);
    assert_ne!(
        recompute_record(&divergent.bytes).0,
        divergent.cid,
        "fixture: the divergent bytes must recompute to a CID other than their key"
    );
    let served: Vec<PreloadedRecord> = verbatim
        .iter()
        .cloned()
        .chain([divergent.clone()])
        .collect();
    let instance = FakeInstance::for_peer(RACHEL_DID, served);

    // And Maria subscribed to Rachel via the real `peer add`.
    subscribe_to_peer_instance(&env, RACHEL_DID, &instance);

    // When Maria pulls.
    let pull = run_openlore_pull(
        &env,
        &["peer", "pull"],
        RACHEL_DID,
        instance.endpoint_url(),
        &rachel.pubkey_hex,
    );

    // Then each stored claim is one whose RETURNED bytes Rust recomputed to
    // exactly its manifest key, read through the opaque transport ...
    let stored: Vec<String> = peer_claim_rows(&env)
        .into_iter()
        .map(|(cid, _)| cid)
        .collect();
    let mut expected: Vec<String> = verbatim.iter().map(|r| r.cid.clone()).collect();
    expected.sort();
    assert_eq!(
        stored, expected,
        "exactly the byte-verbatim records are stored;\n--- stdout ---\n{}",
        pull.stdout
    );
    for cid in &stored {
        assert_instance_served_record(&instance, cid);
        let bytes = instance.record_bytes(cid).expect("stored CID is served");
        assert_eq!(
            &recompute_record(&bytes).0,
            cid,
            "stored CID {cid} must equal the CID recomputed from the returned bytes"
        );
    }

    // ... while the byte-divergent record was READ, failed the recompute,
    // and was rejected before any insert (flagged by a non-zero exit).
    assert_instance_served_record(&instance, &divergent.cid);
    assert!(
        !stored.contains(&divergent.cid),
        "the byte-divergent record must never be stored"
    );
    assert_ne!(pull.status, 0, "a rejected record flags the pull non-zero");
    assert!(
        pull.stdout.contains("CID mismatch"),
        "the rejection names the CID recompute failure;\n--- stdout ---\n{}",
        pull.stdout
    );
    assert_peer_instance_only_read(&instance);
}
