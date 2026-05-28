//! Slice-03 acceptance — `openlore claim counter <target_cid> --reason "..."` verb.
//!
//! The counter-claim sugar verb (WD-17 + ADR-013): constructs an
//! unsigned claim with `references[].type == Counters` pointing at
//! `<target_cid>` + `reason: Some(<text>)` then threads it through the
//! slice-01 `VerbClaimPublish` pipeline unchanged (WD-22 +
//! single-publish-path invariant per ADR-003 / I-FED-5).
//!
//! Covers:
//! - US-FED-004: author + publish a counter-claim (happy path + 4
//!   sad/edge paths)
//! - WD-20: `--reason` is REQUIRED on counter-claims (1..=1000 chars)
//! - WD-34: self-counter rejected in pure-core BEFORE compose preview
//! - WD-35: `--reason` is NFC-normalized before sign (idempotency
//!   property; ADR-015)
//! - WD-43: first-counter-claim framing block fires EXACTLY ONCE
//!   (resolved from `# DISTILL: confirm` habit scenario 2)
//! - WD-44: publish-time no auto-notification to target peer (resolved
//!   from `# DISTILL: confirm` anxiety scenario 4)
//
// SCAFFOLD: true

mod support;

#[allow(unused_imports)]
use support::*;

// =============================================================================
// US-FED-004 — happy path
// =============================================================================

/// CC-1: `openlore claim counter <peer_cid> --reason "..." [claim flags]`
/// renders a compose preview containing BOTH "not as truth" (inherited
/// from slice-01 / I-7) AND "counter-claims coexist, never overwrite"
/// (slice-03 content-frozen literal) AND
/// "counters: <peer_cid> (by <peer_did>)" AND the --reason text
/// verbatim wrapped at 78 cols. On Enter, the claim is signed via the
/// slice-01 pipeline; on Y, published. The counter-claim ends up in
/// `author_claims` (NOT `peer_claims`) — it is the user's own published
/// artifact. Subsequent federated query annotates Maria's row with
/// "counters <peer_cid> ..." AND Rachel's row with "countered-by ...".
/// (US-FED-004 AC 1-9 + UAT scenario #5; integration gates 1 + 3;
/// KPI-FED-3 + KPI-FED-1 + KPI-FED-2.)
///
/// @us-fed-004 @real-io @driving_port @j-003b @j-001 @kpi-fed-3 @happy
#[test]
fn counter_claim_compose_signs_and_publishes_via_slice_01_pipeline_with_required_framing() {
    let env = TestEnv::initialized();

    // GIVEN: Rachel (a peer) publishes three honest, REAL-signed claims;
    // the user subscribes and pulls them into `peer_claims`. The pull
    // pipeline (PP-1) recomputes + verifies each CID, so the target CID we
    // counter is the genuine on-disk peer-claim CID — exactly what the
    // round-trip gate (integration gate 3) needs.
    let peer_did = "did:plc:rachel-test";
    let rachel_seed = [7u8; 32];
    let (records, rachel_pubkey_hex) = build_verifiable_peer_records(peer_did, rachel_seed);
    assert_eq!(records.len(), 3, "Rachel publishes exactly three claims");
    let peer = PeerPds::for_peer(peer_did, records);

    // Subscribe + pull through the real verbs so peer_claims is populated
    // exactly as a user would create it.
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
    let pulled = run_openlore_pull(
        &env,
        &["peer", "pull"],
        peer_did,
        peer.endpoint_url(),
        &rachel_pubkey_hex,
    );
    assert_eq!(
        pulled.status, 0,
        "peer pull precondition must succeed;\n--- stdout ---\n{}\n--- stderr ---\n{}",
        pulled.stdout, pulled.stderr
    );
    assert_peer_claims_attributed_to(&env, peer_did, 3);

    // The target we counter is the first of Rachel's three peer claims.
    // We recompute its CID locally exactly the way the pull pipeline did,
    // so the test owns the round-trip oracle (integration gate 3).
    let target_cid = first_peer_claim_cid(peer_did, rachel_seed);

    // Universe BEFORE the counter: the peer cache holds Rachel's three
    // claims; the user's own author `claims` table is at its post-pull
    // count; the user's own PDS has accepted zero records.
    let author_claims_before = author_claim_count_now(&env);
    assert_no_pds_call_was_made(&env);

    // WHEN: the user composes a counter-claim against Rachel's claim with a
    // free-text reason, then confirms BOTH prompts (Enter to sign, Y to
    // publish). The reason carries a decomposed accent so the NFC
    // normalization beat (WD-35) is exercised end-to-end.
    let reason = "The cited cafe\u{0301} benchmark was retracted by upstream maintainers.";
    let outcome = run_openlore_with_peer_resolver_stdin(
        &env,
        &["claim", "counter", &target_cid, "--reason", reason],
        peer_did,
        peer.endpoint_url(),
        "\nY\n",
    );

    // THEN (criterion 1): exit 0.
    assert_eq!(
        outcome.status, 0,
        "claim counter must exit 0;\n--- stdout ---\n{}\n--- stderr ---\n{}",
        outcome.stdout, outcome.stderr
    );

    // THEN (criterion 2): the compose preview carries BOTH framing literals
    // — the inherited "not as truth" (I-7) AND the slice-03 content-frozen
    // "counter-claims coexist, never overwrite".
    assert_compose_preview_contains_not_as_truth(&outcome);
    assert!(
        outcome
            .stdout
            .contains("counter-claims coexist, never overwrite"),
        "compose preview must contain the slice-03 content-frozen literal \
         \"counter-claims coexist, never overwrite\";\n--- stdout ---\n{}",
        outcome.stdout
    );

    // THEN (criterion 3): the preview names the target + its peer author —
    // "counters: <target_cid> (by <peer_did>)".
    assert!(
        outcome
            .stdout
            .contains(&format!("counters: {target_cid} (by {peer_did})")),
        "compose preview must name the countered target + its peer author as \
         \"counters: {target_cid} (by {peer_did})\";\n--- stdout ---\n{}",
        outcome.stdout
    );

    // THEN (criterion 4): the --reason text appears verbatim in the preview,
    // NFC-normalized (the decomposed accent composes to the precomposed
    // form before display + sign).
    let normalized_reason: String = claim_domain::normalize_reason(reason);
    assert!(
        outcome.stdout.contains(&normalized_reason),
        "compose preview must contain the NFC-normalized --reason verbatim;\n\
         expected: {normalized_reason:?}\n--- stdout ---\n{}",
        outcome.stdout
    );

    // THEN (criterion 5 — integration gate 3 `counter_target_cid_round_trip`):
    // the published counter-claim lands in the user's OWN author `claims`
    // table (NOT peer_claims), and its signed payload carries a
    // references[] entry { type: Counters, cid: target_cid } — the target
    // CID in the preview == the references[].cid in the signed artifact.
    assert_eq!(
        author_claim_count_now(&env),
        author_claims_before + 1,
        "the counter-claim must add exactly ONE row to the user's OWN author \
         `claims` table (it is the user's own published artifact, not a peer claim)"
    );
    let counter_cid = parse_counter_claim_cid(&outcome.stdout);
    assert_counter_claim_references(&env, &counter_cid, &target_cid, &normalized_reason);

    // THEN (criterion 6 — anti-merging): peer_claims is UNCHANGED — the
    // counter is the user's own artifact, never written into the peer cache.
    assert_peer_claims_attributed_to(&env, peer_did, 3);

    // THEN (criterion 7 — single-publish-path / WD-44): the counter-claim
    // was published to the user's OWN PDS exactly once, and NO write was
    // made against the peer's PDS (only the prior pull's reads). The user's
    // own fake PDS now holds the one counter-claim record.
    let counter_at_uri = format!(
        "at://{}/org.openlore.claim/{counter_cid}",
        env.identity.author_did()
    );
    assert_pds_contains_record_at(&env, &counter_at_uri);
    assert_eq!(
        env.pds.records().len(),
        1,
        "single-publish-path (I-FED-5): the user's OWN PDS must hold exactly ONE \
         record (the counter-claim) — no parallel publish path;\n actual: {:?}",
        env.pds.records()
    );
}

/// Recompute the CID of the FIRST of Rachel's three honest peer claims.
/// Mirrors `build_verifiable_peer_records` byte-for-byte so the test owns
/// the round-trip oracle (integration gate 3) — the value MUST equal the
/// `rkey` the peer published and the CID `peer pull` stored.
fn first_peer_claim_cid(peer_did: &str, peer_seed: [u8; 32]) -> String {
    use claim_domain::{canonicalize, compute_cid, Confidence, Did, UnsignedClaim};

    let confidence: Confidence =
        serde_json::from_value(serde_json::json!(0.42)).expect("confidence value is well-formed");
    let unsigned = UnsignedClaim {
        subject: "github:rust-lang/cargo".to_string(),
        predicate: "embodiesPhilosophy".to_string(),
        object: "org.openlore.philosophy.dependency-pinning".to_string(),
        evidence: vec!["https://github.com/rust-lang/cargo".to_string()],
        confidence,
        author_did: Did(format!("{peer_did}#org.openlore.application")),
        composed_at: "2026-05-22T09:18:44Z".to_string(),
        references: Vec::new(),
        reason: None,
    };
    let _ = peer_seed; // CID is content-derived; the seed only signs.
    let canonical = canonicalize(&unsigned).expect("canonicalize first peer claim");
    compute_cid(&canonical).0
}

/// Parse the `Computing claim CID <cid>` marker the sign step emits to
/// recover the counter-claim's own CID (mirrors the slice-01 WS-6 parser).
/// Uses a substring search rather than a line-prefix match because the
/// marker may share a line with the (newline-free) sign prompt.
fn parse_counter_claim_cid(stdout: &str) -> String {
    const MARKER: &str = "Computing claim CID ";
    let start = stdout
        .find(MARKER)
        .map(|i| i + MARKER.len())
        .unwrap_or_else(|| {
            panic!("expected a `Computing claim CID <cid>` marker in stdout;\n--- stdout ---\n{stdout}")
        });
    stdout[start..]
        .split_whitespace()
        .next()
        .expect("CID token after the marker")
        .to_string()
}

/// Universe-bound: count rows in the user's OWN author `claims` table.
/// Port-exposed name: `author_claims.row_count`. Raw SQL is acceptable in
/// test-support; production goes through StoragePort.
fn author_claim_count_now(env: &TestEnv) -> usize {
    let conn =
        duckdb::Connection::open(env.duckdb_path()).expect("open DuckDB for author-claims count");
    let total: i64 = conn
        .query_row("SELECT count(*) FROM claims", [], |r| r.get(0))
        .expect("count author claims");
    total.max(0) as usize
}

/// Universe-bound: "the on-disk counter-claim artifact at
/// `claims/<counter_cid>.json` deserializes to a SignedClaim whose
/// references[] contains { type: Counters, cid: target_cid } AND whose
/// signed payload carries the NFC-normalized reason verbatim (so the CID
/// and signature cover the reason — ADR-006 lex order)."
fn assert_counter_claim_references(
    env: &TestEnv,
    counter_cid: &str,
    target_cid: &str,
    normalized_reason: &str,
) {
    let artifact = env.claims_dir().join(format!("{counter_cid}.json"));
    let bytes = std::fs::read(&artifact).unwrap_or_else(|e| {
        panic!(
            "expected counter-claim file at {}; got {e}",
            artifact.display()
        )
    });
    let signed: claim_domain::SignedClaim = serde_json::from_slice(&bytes)
        .unwrap_or_else(|e| panic!("deserialize counter-claim at {}: {e}", artifact.display()));

    let has_counters = signed.unsigned.references.iter().any(|r| {
        matches!(r.ref_type, claim_domain::ReferenceType::Counters) && r.cid.0 == target_cid
    });
    assert!(
        has_counters,
        "the counter-claim must carry references[] {{type=Counters, cid={target_cid}}} \
         (integration gate 3 round-trip); actual references={:?}",
        signed.unsigned.references
    );
    assert_eq!(
        signed.unsigned.reason.as_deref(),
        Some(normalized_reason),
        "the signed payload MUST carry the NFC-normalized reason verbatim so the \
         CID + signature cover it (ADR-006 lex order)"
    );
}

// =============================================================================
// US-FED-004 — sad / edge paths
// =============================================================================

/// CC-2 / Sad (WD-20): `openlore claim counter <peer_cid>` invoked
/// WITHOUT `--reason` (other claim flags valid) exits non-zero
/// pre-compose with the error message "counter-claims require
/// --reason; explain your disagreement". NO file is written. NO
/// network call is made. (US-FED-004 AC 2 + UAT scenario #2.)
///
/// @us-fed-004 @real-io @driving_port @j-003b @error @wd-20
#[test]
fn counter_claim_rejects_missing_reason_pre_compose() {
    let env = TestEnv::initialized();

    // GIVEN: a peer publishes claims and the user pulls them, so a valid
    // counter target genuinely exists in the peer cache. The reason — not
    // the target — is what must trip the pre-compose guard.
    let peer_did = "did:plc:rachel-test";
    let rachel_seed = [7u8; 32];
    let (records, rachel_pubkey_hex) = build_verifiable_peer_records(peer_did, rachel_seed);
    let peer = PeerPds::for_peer(peer_did, records);
    let added = run_openlore_with_peer_resolver(
        &env,
        &["peer", "add", peer_did],
        peer_did,
        peer.endpoint_url(),
    );
    assert_eq!(added.status, 0, "peer add precondition must succeed");
    let pulled = run_openlore_pull(
        &env,
        &["peer", "pull"],
        peer_did,
        peer.endpoint_url(),
        &rachel_pubkey_hex,
    );
    assert_eq!(pulled.status, 0, "peer pull precondition must succeed");
    let target_cid = first_peer_claim_cid(peer_did, rachel_seed);

    // WHEN: the user counters that valid target but supplies an EMPTY
    // `--reason` (a totally-absent flag is a clap parse error; the empty
    // string reaches the verb, which must reject it pre-compose — WD-20).
    let outcome = run_openlore_with_peer_resolver(
        &env,
        &["claim", "counter", &target_cid, "--reason", ""],
        peer_did,
        peer.endpoint_url(),
    );

    // THEN: non-zero exit.
    assert_ne!(
        outcome.status, 0,
        "missing-reason counter-claim must exit non-zero;\n--- stdout ---\n{}\n--- stderr ---\n{}",
        outcome.stdout, outcome.stderr
    );

    // THEN: stderr names the requirement with the content-frozen literal.
    assert!(
        outcome
            .stderr
            .contains("counter-claims require --reason; explain your disagreement"),
        "stderr must carry the CC-2 literal \"counter-claims require --reason; explain your \
         disagreement\";\n--- stderr ---\n{}",
        outcome.stderr
    );

    // THEN (pre-compose ordering): NO compose preview was rendered — the
    // guard fired BEFORE any preview reached stdout.
    assert!(
        !outcome
            .stdout
            .contains("counter-claims coexist, never overwrite"),
        "the rejection must happen PRE-COMPOSE — no compose preview may be rendered;\n\
         --- stdout ---\n{}",
        outcome.stdout
    );

    // THEN: nothing signed (no artifact under claims_dir) and nothing
    // published (zero PDS calls).
    assert!(
        !claims_dir_has_any_artifact(&env),
        "no counter-claim artifact may be written when --reason is empty"
    );
    assert_no_pds_call_was_made(&env);
}

/// Universe-bound: "the on-disk claims directory holds zero `*.json`
/// counter-claim artifacts." Port-exposed name: `claims_dir.artifact_count
/// == 0`. Used by the sad-path scenarios (CC-2 / CC-3) to assert nothing
/// was signed when the pre-compose guard rejects.
fn claims_dir_has_any_artifact(env: &TestEnv) -> bool {
    let dir = env.claims_dir();
    match std::fs::read_dir(&dir) {
        Ok(entries) => entries
            .filter_map(|e| e.ok())
            .any(|e| e.path().extension().and_then(|x| x.to_str()) == Some("json")),
        // No directory at all == no artifacts.
        Err(_) => false,
    }
}

/// CC-3 / Edge (WD-20): `--reason` longer than 1000 chars is rejected
/// pre-compose with an error naming the upper bound (1..=1000 per WD-20
/// + ADR-015 minLength/maxLength on the Lexicon `reason` field).
///
/// @us-fed-004 @real-io @driving_port @j-003b @error @wd-20
#[test]
fn counter_claim_rejects_reason_exceeding_one_thousand_chars() {
    let env = TestEnv::initialized();

    // GIVEN: a peer publishes claims and the user pulls them, so the target
    // is genuinely valid. Only the reason LENGTH is out of bounds — proving
    // the upper-bound guard, not an unknown-target error, is what rejects.
    let peer_did = "did:plc:rachel-test";
    let rachel_seed = [7u8; 32];
    let (records, rachel_pubkey_hex) = build_verifiable_peer_records(peer_did, rachel_seed);
    let peer = PeerPds::for_peer(peer_did, records);
    let added = run_openlore_with_peer_resolver(
        &env,
        &["peer", "add", peer_did],
        peer_did,
        peer.endpoint_url(),
    );
    assert_eq!(added.status, 0, "peer add precondition must succeed");
    let pulled = run_openlore_pull(
        &env,
        &["peer", "pull"],
        peer_did,
        peer.endpoint_url(),
        &rachel_pubkey_hex,
    );
    assert_eq!(pulled.status, 0, "peer pull precondition must succeed");
    let target_cid = first_peer_claim_cid(peer_did, rachel_seed);

    // WHEN: the reason is 1001 chars — one past the WD-20 upper bound of
    // 1..=1000 (ADR-015 maxLength on the Lexicon `reason` field).
    let over_limit_reason = "a".repeat(1001);
    let outcome = run_openlore_with_peer_resolver(
        &env,
        &[
            "claim",
            "counter",
            &target_cid,
            "--reason",
            &over_limit_reason,
        ],
        peer_did,
        peer.endpoint_url(),
    );

    // THEN: non-zero exit.
    assert_ne!(
        outcome.status, 0,
        "over-length counter-claim reason must exit non-zero;\n--- stdout ---\n{}\n\
         --- stderr ---\n{}",
        outcome.stdout, outcome.stderr
    );

    // THEN: stderr names the 1000-char upper bound.
    assert!(
        outcome.stderr.contains("1000"),
        "stderr must name the 1000-character upper bound;\n--- stderr ---\n{}",
        outcome.stderr
    );

    // THEN (pre-compose ordering): no compose preview was rendered.
    assert!(
        !outcome
            .stdout
            .contains("counter-claims coexist, never overwrite"),
        "the rejection must happen PRE-COMPOSE — no compose preview may be rendered;\n\
         --- stdout ---\n{}",
        outcome.stdout
    );

    // THEN: nothing signed and nothing published.
    assert!(
        !claims_dir_has_any_artifact(&env),
        "no counter-claim artifact may be written when --reason exceeds 1000 chars"
    );
    assert_no_pds_call_was_made(&env);
}

/// CC-4 / Sad (WD-34): Countering one's OWN claim is rejected
/// pre-compose with the error "cannot counter your own claim" AND a
/// hint to use `openlore claim retract <cid>` instead. The check
/// resolves via `claim_domain::validate_counter_claim` against EITHER
/// `claims` OR `peer_claims` (the target may be in either store; cli
/// hands in a `&dyn ClaimLookup`). No file is written. (US-FED-004
/// AC 6 + UAT scenario #3; Example 2.)
///
/// @us-fed-004 @real-io @driving_port @j-003b @error @wd-34
#[test]
fn counter_claim_rejects_self_counter_with_retract_hint() {
    let env = TestEnv::initialized();

    // GIVEN: the user authors + signs ONE of their OWN claims (declining
    // publish with "n"), so a genuine self-target lives in the user's own
    // `claims` table. The CombinedClaimLookup resolves the own store FIRST,
    // so this is the row that trips the self-counter guard (WD-34).
    let seed = run_openlore_with_stdin(
        &env,
        &[
            "claim",
            "add",
            "--subject",
            "github:rust-lang/rust",
            "--predicate",
            "embodiesPhilosophy",
            "--object",
            "org.openlore.philosophy.memory-safety",
            "--evidence",
            "https://www.rust-lang.org/",
            "--confidence",
            "0.86",
        ],
        "\nn\n", // <Enter> sign, "n" decline publish — keep it local.
    );
    assert_eq!(
        seed.status, 0,
        "seeding an own claim must succeed;\n--- stdout ---\n{}\n--- stderr ---\n{}",
        seed.stdout, seed.stderr
    );
    // Recover the seeded own-claim CID from the sign marker — this is the
    // self-target we will (illegally) try to counter.
    let own_cid = parse_counter_claim_cid(&seed.stdout);

    // Universe BEFORE the self-counter: the user's own `claims` table holds
    // exactly the one seeded claim; one artifact sits under claims_dir; the
    // user's own PDS has accepted zero records (we declined publish).
    let author_claims_before = author_claim_count_now(&env);
    assert_eq!(
        author_claims_before, 1,
        "exactly the one seeded own claim must exist before the self-counter"
    );
    let artifacts_before = claims_dir_artifact_count(&env);
    assert_eq!(
        artifacts_before, 1,
        "exactly one on-disk artifact (the seeded own claim) before the self-counter"
    );
    assert_no_pds_call_was_made(&env);

    // WHEN: the user tries to counter their OWN claim with a perfectly good
    // reason (so ONLY the self-counter rule — not the missing-reason rule —
    // can fire). The reason being valid is what proves WD-34 specifically.
    let outcome = run_openlore_with_stdin(
        &env,
        &[
            "claim",
            "counter",
            &own_cid,
            "--reason",
            "On reflection I no longer stand behind this assertion.",
        ],
        // Provide confirmations defensively: the guard MUST fire pre-compose,
        // so these should never be consumed. If they ARE consumed (the guard
        // failed), the artifact-count assertion below catches it.
        "\nY\n",
    );

    // THEN (criterion 1): non-zero exit — the self-counter is rejected.
    assert_ne!(
        outcome.status, 0,
        "countering your own claim must exit non-zero;\n--- stdout ---\n{}\n--- stderr ---\n{}",
        outcome.stdout, outcome.stderr
    );

    // THEN (criterion 2 — WD-34 message): stderr carries the self-counter
    // refusal AND the retract hint, both content-frozen substrings of
    // `ClaimError::SelfCounter`'s Display.
    assert!(
        outcome.stderr.contains("cannot counter your own claim"),
        "stderr must explain the self-counter refusal \
         (\"cannot counter your own claim\");\n--- stderr ---\n{}",
        outcome.stderr
    );
    assert!(
        outcome.stderr.contains("openlore claim retract"),
        "stderr must HINT the retract path (\"openlore claim retract\");\n--- stderr ---\n{}",
        outcome.stderr
    );

    // THEN (criterion 3 — pre-compose ordering): NO compose preview was
    // rendered; the guard fired BEFORE any preview reached stdout.
    assert!(
        !outcome
            .stdout
            .contains("counter-claims coexist, never overwrite"),
        "the rejection must happen PRE-COMPOSE — no compose preview may be rendered;\n\
         --- stdout ---\n{}",
        outcome.stdout
    );

    // THEN (criterion 4 — nothing signed): the on-disk artifact count is
    // UNCHANGED (still just the seeded own claim) — no counter-claim file
    // was written.
    assert_eq!(
        claims_dir_artifact_count(&env),
        artifacts_before,
        "no counter-claim artifact may be written when self-countering \
         (artifact count must be unchanged from the seeded baseline)"
    );
    // And the own `claims` table is unchanged too — the self-counter never
    // reaches the persist step.
    assert_eq!(
        author_claim_count_now(&env),
        author_claims_before,
        "the user's OWN `claims` table must be unchanged after a rejected self-counter"
    );

    // THEN (criterion 5 — nothing published): zero PDS calls — the guard
    // fires long before any publish prompt.
    assert_no_pds_call_was_made(&env);
}

/// Universe-bound: count `*.json` artifacts under `claims_dir`. Port-exposed
/// name: `claims_dir.artifact_count`. CC-4 asserts this is UNCHANGED across a
/// rejected self-counter (nothing new signed), distinct from CC-2/CC-3 which
/// assert it starts at zero.
fn claims_dir_artifact_count(env: &TestEnv) -> usize {
    let dir = env.claims_dir();
    match std::fs::read_dir(&dir) {
        Ok(entries) => entries
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().and_then(|x| x.to_str()) == Some("json"))
            .count(),
        Err(_) => 0,
    }
}

// =============================================================================
// US-FED-004 — orientation + non-notification (resolves WD-43 + WD-44)
// =============================================================================

/// CC-5 (WD-43): The FIRST EVER `claim counter` invocation per install
/// renders a one-time framing block ("First counter-claim! Some
/// context:" + 4 enumerated points per gherkin-scenarios-expanded.md
/// habit scenario 2) BEFORE the compose preview. Subsequent
/// invocations DO NOT render the framing block. State lives in
/// `~/.config/openlore/identity.toml` under
/// `[federation] first_counter_claim_completed_at`.
/// Resolves `# DISTILL: confirm` flag (habit scenario 2 framing-block
/// trigger; WD-43 LOCKS once-per-user, NOT first-3-times).
///
/// @us-fed-004 @real-io @driving_port @j-003b @habit @wd-43
#[test]
fn counter_claim_first_invocation_renders_one_time_framing_block_then_omits_on_subsequent_invocations(
) {
    let env = TestEnv::initialized();

    // GIVEN: Rachel (a peer) publishes three honest, REAL-signed claims; the
    // user subscribes and pulls them so two genuine, distinct counter targets
    // live in `peer_claims` — one per invocation, so the SECOND counter is a
    // real successful counter (NOT a no-op), proving the framing block is
    // suppressed by the orientation key, not by an early bail-out.
    let peer_did = "did:plc:rachel-test";
    let rachel_seed = [7u8; 32];
    let (records, rachel_pubkey_hex) = build_verifiable_peer_records(peer_did, rachel_seed);
    assert_eq!(records.len(), 3, "Rachel publishes exactly three claims");
    let peer = PeerPds::for_peer(peer_did, records);

    let added = run_openlore_with_peer_resolver(
        &env,
        &["peer", "add", peer_did],
        peer_did,
        peer.endpoint_url(),
    );
    assert_eq!(added.status, 0, "peer add precondition must succeed");
    let pulled = run_openlore_pull(
        &env,
        &["peer", "pull"],
        peer_did,
        peer.endpoint_url(),
        &rachel_pubkey_hex,
    );
    assert_eq!(pulled.status, 0, "peer pull precondition must succeed");
    assert_peer_claims_attributed_to(&env, peer_did, 3);

    // The first counter targets Rachel's first peer claim. (A `peer pull`
    // is NOT a `claim counter`, so the FirstPull orientation does not arm or
    // disarm the FirstCounterClaim milestone — they are independent keys.)
    let target_cid = first_peer_claim_cid(peer_did, rachel_seed);

    // The on-disk orientation key MUST be absent before the first counter —
    // a fresh install has never authored a counter-claim (should_fire).
    let identity_before =
        std::fs::read_to_string(env.identity_toml_path()).expect("identity.toml exists after init");
    assert!(
        !identity_before.contains("first_counter_claim_completed_at"),
        "the first-counter-claim orientation key must be ABSENT before the first \
         counter (a fresh install has never authored one);\n--- identity.toml ---\n{identity_before}"
    );

    // WHEN (1): the user authors + publishes their FIRST EVER counter-claim
    // (Enter to sign, Y to publish).
    let first = run_openlore_with_peer_resolver_stdin(
        &env,
        &["claim", "counter", &target_cid, "--reason", "I disagree."],
        peer_did,
        peer.endpoint_url(),
        "\nY\n",
    );
    assert_eq!(
        first.status, 0,
        "the first counter-claim must exit 0;\n--- stdout ---\n{}\n--- stderr ---\n{}",
        first.stdout, first.stderr
    );

    // THEN (1 — framing block present): the first invocation renders the
    // one-time WD-43 framing block — the heading + all four enumerated
    // points (gherkin habit scenario 2, content-frozen).
    assert!(
        first.stdout.contains("First counter-claim! Some context:"),
        "the FIRST counter-claim must render the one-time framing heading;\n\
         --- stdout ---\n{}",
        first.stdout
    );
    for point in [
        "A counter-claim is a SIGNED public artifact attributed to YOU.",
        "It does NOT delete or hide the target claim; both coexist.",
        "You can retract it later via `openlore claim retract",
        "The target peer is NOT auto-notified",
    ] {
        assert!(
            first.stdout.contains(point),
            "the framing block must include the point {point:?};\n--- stdout ---\n{}",
            first.stdout
        );
    }

    // THEN (1 — framing precedes, never replaces, the standard framing): the
    // one-time block does NOT delay or modify the standard "coexist, never
    // overwrite" framing (gherkin line 241) — the heading appears BEFORE it.
    let heading_at = first
        .stdout
        .find("First counter-claim! Some context:")
        .expect("framing heading present");
    let coexist_at = first
        .stdout
        .find("counter-claims coexist, never overwrite")
        .expect("standard compose-preview framing still present on first invocation");
    assert!(
        heading_at < coexist_at,
        "the one-time framing block must precede (not replace) the standard \
         compose-preview framing;\n--- stdout ---\n{}",
        first.stdout
    );

    // THEN (1 — key recorded): identity.toml now carries the
    // first_counter_claim_completed_at timestamp (the milestone is recorded
    // on success — WD-39 OrientationState mechanism).
    let identity_after_first =
        std::fs::read_to_string(env.identity_toml_path()).expect("read identity.toml after first");
    assert!(
        identity_after_first.contains("first_counter_claim_completed_at"),
        "identity.toml must gain first_counter_claim_completed_at after the first \
         counter succeeds;\n--- identity.toml ---\n{identity_after_first}"
    );

    // WHEN (2): the user authors a SECOND counter-claim (a different target,
    // so it is a genuine successful counter, not a no-op).
    let second_target_cid = second_peer_claim_cid(peer_did, rachel_seed);
    assert_ne!(
        second_target_cid, target_cid,
        "the second target must be a DISTINCT peer claim so the second counter genuinely succeeds"
    );
    let second = run_openlore_with_peer_resolver_stdin(
        &env,
        &[
            "claim",
            "counter",
            &second_target_cid,
            "--reason",
            "I also disagree with this one.",
        ],
        peer_did,
        peer.endpoint_url(),
        "\nY\n",
    );
    assert_eq!(
        second.status, 0,
        "the second counter-claim must also exit 0;\n--- stdout ---\n{}\n--- stderr ---\n{}",
        second.stdout, second.stderr
    );

    // THEN (2 — framing block OMITTED): the second invocation does NOT render
    // the one-time framing block (WD-43 once-per-user lock, NOT first-3-times).
    assert!(
        !second.stdout.contains("First counter-claim! Some context:"),
        "the framing block must be OMITTED on the second counter-claim (WD-43 \
         once-per-user, not first-3-times);\n--- stdout ---\n{}",
        second.stdout
    );
    // The standard compose framing still renders on the second invocation —
    // the suppressed block is ONLY the one-time orientation, not the preview.
    assert!(
        second
            .stdout
            .contains("counter-claims coexist, never overwrite"),
        "the standard compose-preview framing must STILL render on the second \
         counter-claim;\n--- stdout ---\n{}",
        second.stdout
    );
}

/// Recompute the CID of the SECOND of Rachel's three honest peer claims —
/// a DISTINCT, genuinely-publishable counter target for the second
/// invocation in CC-5. Mirrors `build_verifiable_peer_records`'s second
/// triple byte-for-byte (same subject + evidence + composed_at; the object
/// + confidence differ from the first, so the CID is distinct).
fn second_peer_claim_cid(peer_did: &str, peer_seed: [u8; 32]) -> String {
    use claim_domain::{canonicalize, compute_cid, Confidence, Did, UnsignedClaim};

    let confidence: Confidence =
        serde_json::from_value(serde_json::json!(0.71)).expect("confidence value is well-formed");
    let unsigned = UnsignedClaim {
        subject: "github:rust-lang/cargo".to_string(),
        predicate: "embodiesPhilosophy".to_string(),
        object: "org.openlore.philosophy.reproducible-builds".to_string(),
        evidence: vec!["https://github.com/rust-lang/cargo".to_string()],
        confidence,
        author_did: Did(format!("{peer_did}#org.openlore.application")),
        composed_at: "2026-05-22T09:18:44Z".to_string(),
        references: Vec::new(),
        reason: None,
    };
    let _ = peer_seed; // CID is content-derived; the seed only signs.
    let canonical = canonicalize(&unsigned).expect("canonicalize second peer claim");
    compute_cid(&canonical).0
}

/// CC-6 (WD-44 — RESOLVES `# DISTILL: confirm` anxiety scenario 4):
/// Publishing a counter-claim against a peer's claim does NOT trigger
/// any network call to the peer's PDS beyond the user's normal
/// own-PDS publish. The peer learns about the counter-claim only when
/// they later pull from the current user (if they subscribe back).
/// Slice-03 ships NO notification mechanism in either direction.
///
/// @us-fed-004 @real-io @driving_port @j-003b @wd-44
#[test]
fn counter_claim_publish_does_not_auto_notify_target_peer_pds() {
    todo!("DELIVER (slice-03): construct TestEnv with FakePeerPds AND FakePds (user's own). Pull Rachel's records into peer_claims; counter one of them; assert (a) FakePds (user's own PDS) received exactly one create_record call for the counter-claim, (b) FakePeerPds received ZERO writes (only the listRecords / getRecord reads from the prior pull), (c) no notification XRPC method was called against the peer's endpoint")
}
