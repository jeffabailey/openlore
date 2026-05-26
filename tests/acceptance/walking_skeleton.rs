//! Walking-skeleton acceptance tests for openlore-foundation slice-01.
//!
//! These tests drive the real `openlore` CLI binary as a subprocess
//! (Mandate 1: hexagonal boundary; Pillar 3: app as in production).
//! Every test exercises a complete user journey from the user's typed
//! command to observable side effects (stdout, exit code, filesystem,
//! DuckDB, fake PDS).
//!
//! Mocked: PDS (`FakePds`) + identity (`FakeIdentity`) only — see
//! `support/mod.rs`. Everything else is real.
//!
//! Per Mandate 7 (RED-ready scaffolding) and DD-2 (pre-DELIVER
//! fail-for-right-reason gate deferred): every test body panics via
//! `todo!()`. After DELIVER scaffolds `Cargo.toml` + the 8 crates from
//! `docs/feature/openlore-foundation/design/component-boundaries.md`,
//! these tests classify as RED (panic at `todo!()`), not BROKEN
//! (import error). DELIVER then enables one at a time.
//!
//! See `docs/feature/openlore-foundation/distill/acceptance-tests.md`
//! and `docs/feature/openlore-foundation/distill/traceability.md` for
//! the full design context, story/job traceability, and the resolved
//! DISTILL flags from `gherkin-scenarios-expanded.md`.
//
// SCAFFOLD: true

mod support;

use support::*;

// =============================================================================
// US-005 — Bootstrap (openlore init)
// =============================================================================

/// WS-1: `openlore init` resolves the user's ATProto identity, writes
/// the identity config, creates the DuckDB, and is idempotent on re-run.
///
/// @walking_skeleton @driving_port @US-005 @J-001 @real-io
#[test]
fn walking_skeleton_init_creates_identity_duckdb_and_is_idempotent() {
    // Given a fresh environment with no ~/.config/openlore/
    // or ~/.local/share/openlore/
    let env = TestEnv::fresh();

    // When Jeff runs `openlore init`
    let first = run_openlore(&env, &["init", "--handle", "jeff.test", "--app-password", "fake-app-password"]);

    // Then the identity config exists and names the test DID
    assert_exit_zero_and_stdout_contains(&first, "OpenLore initialized for did:plc:test-jeff");
    // And the DuckDB file exists with the claims table present
    // And the identity.toml exists
    // (assertion helpers wrap the universe-bound observable checks)

    // When Jeff runs `openlore init` again
    let second = run_openlore(&env, &["init", "--handle", "jeff.test", "--app-password", "fake-app-password"]);

    // Then it exits with an "already initialized" message
    assert_exit_zero_and_stdout_contains(&second, "already initialized for did:plc:test-jeff");

    // Step 05-01: WS-1 activated — the assertions above are the
    // contract; the trailing scaffold todo!() that DISTILL placed has
    // been removed now that the init verb is implemented.
}

/// WS-2: Claim commands gated on init. Running `openlore claim add`
/// without first running `openlore init` exits non-zero with a message
/// naming the required `openlore init` command.
///
/// @walking_skeleton @driving_port @US-005 @J-001 @error
#[test]
fn walking_skeleton_claim_commands_fail_loudly_when_not_initialized() {
    todo!("DELIVER: TestEnv::fresh; run `openlore claim add` with the full Jeff/Rust flag set; assert exit nonzero and stderr names `openlore init`")
}

// =============================================================================
// US-001 — Compose claim intent (the 'not as truth' preview-gate)
// =============================================================================

/// WS-3: The compose preview MUST contain the literal text "not as
/// truth" AND wait at a confirmation prompt BEFORE any signing,
/// persistence, or network I/O. (WD-6 — load-bearing UX moment.)
///
/// @walking_skeleton @driving_port @US-001 @J-001 @real-io
#[test]
fn walking_skeleton_compose_preview_contains_not_as_truth_and_waits_for_confirmation() {
    // Given Jeff has authenticated as did:plc:test-jeff (via initialized env)
    let env = TestEnv::initialized();

    // When Jeff runs `openlore claim add` with all flags and DOES NOT
    // send Enter (we read stdout and kill the process before
    // confirming; the binary should block at the prompt)
    let outcome = run_openlore_with_stdin(
        &env,
        &[
            "claim", "add",
            "--subject", "github:rust-lang/rust",
            "--predicate", "embodiesPhilosophy",
            "--object", "org.openlore.philosophy.memory-safety",
            "--evidence", "https://www.rust-lang.org/",
            "--confidence", "0.86",
        ],
        // No stdin input — the binary should print the preview and
        // wait. DELIVER may instead implement a `--dry-run` mode that
        // prints the preview and exits 0 without prompting; either
        // shape satisfies the AC.
        "",
    );

    // Then the CLI prints the compose preview with the literal text
    assert_compose_preview_contains_not_as_truth(&outcome);

    // And no file has been written under ~/.local/share/openlore/
    assert_no_local_claim_files_exist(&env);

    // And no PDS call has been made
    assert_no_pds_call_was_made(&env);

    todo!("DELIVER: implement the dry-run / pre-confirm preview behavior; satisfy all three assertions")
}

/// WS-4: Out-of-range confidence is rejected pre-sign with a useful
/// error and no side effects. (US-001 Example 3, AC #3.)
///
/// @walking_skeleton @driving_port @US-001 @J-001 @error
#[test]
fn walking_skeleton_compose_rejects_confidence_outside_unit_interval() {
    let env = TestEnv::initialized();

    let outcome = run_openlore(
        &env,
        &[
            "claim", "add",
            "--subject", "github:rust-lang/rust",
            "--predicate", "embodiesPhilosophy",
            "--object", "org.openlore.philosophy.memory-safety",
            "--evidence", "https://www.rust-lang.org/",
            "--confidence", "1.4", // out of range
        ],
    );

    // The CLI exits with a non-zero status and the error names the flag
    // and the valid range [0.0, 1.0]
    assert_exit_nonzero_and_stderr_contains(&outcome, "--confidence must be in [0.0, 1.0]");
    assert_exit_nonzero_and_stderr_contains(&outcome, "1.4");

    // And no file has been written
    assert_no_local_claim_files_exist(&env);

    // And no network call has been made
    assert_no_pds_call_was_made(&env);

    todo!("DELIVER: implement pre-sign confidence validation; surface the named flag and range")
}

/// WS-5: Confidence bucket label is display-only. The compose preview
/// shows `0.55 (weighted)` but the signed claim contains only the
/// numeric value 0.55. (US-001 Example 2 + WD-10 + D-12.)
///
/// @walking_skeleton @driving_port @US-001 @J-001 @real-io
#[test]
fn walking_skeleton_compose_preview_shows_bucket_label_but_signed_payload_has_only_numeric() {
    let env = TestEnv::initialized();

    // Compose with confidence 0.55 and sign (Enter); skip publish (n).
    let outcome = run_openlore_with_stdin(
        &env,
        &[
            "claim", "add",
            "--subject", "github:mastodon/mastodon",
            "--predicate", "embodiesPhilosophy",
            "--object", "org.openlore.philosophy.federation-first",
            "--evidence", "https://joinmastodon.org/",
            "--confidence", "0.55",
        ],
        "\nn\n", // <Enter> at sign prompt, "n" at publish prompt
    );

    // The preview shows the bucket label
    assert_exit_zero_and_stdout_contains(&outcome, "0.55 (weighted)");

    // The signed file does NOT contain any bucket label string
    // (we need the CID to address the file; DELIVER's helper will
    // discover it by listing claims_dir() — there should be exactly
    // one file after this scenario)
    todo!("DELIVER: list env.claims_dir(); assert one file; assert_persisted_payload_has_no_bucket_label(&env, cid)")
}

// =============================================================================
// US-002 — Sign and persist locally (the local-first beat)
// =============================================================================

/// WS-6: Signing produces a verifiable local file. The signature
/// verifies against did:plc:test-jeff's public key AND NO outbound
/// network call to a PDS has occurred. (US-002 AC + KPI-5.)
///
/// @walking_skeleton @driving_port @US-002 @J-001 @real-io
#[test]
fn walking_skeleton_sign_writes_atomic_local_file_with_no_network_call() {
    let env = TestEnv::initialized();

    let outcome = run_openlore_with_stdin(
        &env,
        &[
            "claim", "add",
            "--subject", "github:rust-lang/rust",
            "--predicate", "embodiesPhilosophy",
            "--object", "org.openlore.philosophy.memory-safety",
            "--evidence", "https://www.rust-lang.org/",
            "--confidence", "0.86",
        ],
        "\nn\n", // <Enter> sign, "n" decline publish
    );

    // The CLI announces the CID in stdout (US-002 Example 1 mockup)
    assert_exit_zero_and_stdout_contains(&outcome, "Computing claim CID");
    assert_exit_zero_and_stdout_contains(&outcome, "Written to local store");

    // A file appears under ~/.local/share/openlore/claims/<cid>.json
    // AND the file's signature verifies against the test DID
    // (DELIVER's helper extracts the CID from stdout and asserts file presence)
    todo!("DELIVER: parse the CID out of stdout; assert_claim_file_exists_with_cid; verify the signature against FakeIdentity::jeff's pubkey; assert_no_pds_call_was_made")
}

/// WS-7: Re-canonicalization produces identical CIDs. (US-002 Example
/// 2; risk #2 from feature-delta.md "canonical CID determinism";
/// KPI-4.)
///
/// @walking_skeleton @driving_port @US-002 @J-001 @real-io
#[test]
fn walking_skeleton_re_canonicalization_produces_identical_cids() {
    let env_first = TestEnv::initialized();
    let env_second = TestEnv::initialized();

    // First run
    let first_args = [
        "claim", "add",
        "--subject", "github:rust-lang/rust",
        "--predicate", "embodiesPhilosophy",
        "--object", "org.openlore.philosophy.memory-safety",
        "--evidence", "https://www.rust-lang.org/",
        "--confidence", "0.86",
        // DELIVER pins composed_at via env var or flag for determinism
        // (e.g. OPENLORE_TEST_NOW=2026-05-25T12:00:00Z)
    ];
    let outcome_first = run_openlore_with_stdin(&env_first, &first_args, "\nn\n");
    let outcome_second = run_openlore_with_stdin(&env_second, &first_args, "\nn\n");

    // Both runs produce the same CID
    todo!("DELIVER: parse the CID from each outcome's stdout; assert byte-equality")
}

// =============================================================================
// US-003 — Publish to PDS (the federated boundary)
// =============================================================================

/// WS-8: Successful publish prints `at-uri:` AND the retract-command
/// hint. (US-003 Example 1, AC #1 + WD-6 retract-hint lock.)
///
/// @walking_skeleton @driving_port @US-003 @J-001 @real-io
#[test]
fn walking_skeleton_publish_prints_at_uri_and_retract_hint_after_signing() {
    let env = TestEnv::initialized();

    let outcome = run_openlore_with_stdin(
        &env,
        &[
            "claim", "add",
            "--subject", "github:rust-lang/rust",
            "--predicate", "embodiesPhilosophy",
            "--object", "org.openlore.philosophy.memory-safety",
            "--evidence", "https://www.rust-lang.org/",
            "--confidence", "0.86",
        ],
        "\nY\n", // <Enter> sign, "Y" publish
    );

    assert_exit_zero_and_stdout_contains(&outcome, "at-uri: at://did:plc:test-jeff/org.openlore.claim/");
    assert_exit_zero_and_stdout_contains(&outcome, "openlore claim retract");

    // The fake PDS contains the record at the expected at-uri
    todo!("DELIVER: parse the at-uri from stdout; assert_pds_contains_record_at; assert_duckdb_publication_metadata_for_cid")
}

/// WS-9: Republishing a CID is idempotent (no duplicate, no error).
/// (US-003 Example 3, AC #3.)
///
/// @walking_skeleton @driving_port @US-003 @J-001 @real-io
#[test]
fn walking_skeleton_publish_is_idempotent_on_re_run_with_same_cid() {
    let env = TestEnv::initialized();

    // First publish via chained flow
    let first = run_openlore_with_stdin(
        &env,
        &[
            "claim", "add",
            "--subject", "github:rust-lang/rust",
            "--predicate", "embodiesPhilosophy",
            "--object", "org.openlore.philosophy.memory-safety",
            "--evidence", "https://www.rust-lang.org/",
            "--confidence", "0.86",
        ],
        "\nY\n",
    );
    // Extract CID from first.stdout — DELIVER provides a helper.
    let _cid = "bafy..."; // todo!("parse CID from first.stdout")

    // Second invocation via the standalone verb
    let second = run_openlore(&env, &["claim", "publish", "bafy..."]);

    assert_exit_zero_and_stdout_contains(&second, "already published");
    // And the fake PDS still has exactly one record for that at-uri
    todo!("DELIVER: assert env.pds.records().len() == 1 for the cid; second outcome exit 0; message includes 'already present'")
}

/// WS-10: PDS unreachable leaves the local claim intact and retry-able.
/// (US-003 Example 2, AC #2.)
///
/// @walking_skeleton @driving_port @US-003 @J-001 @error
#[test]
fn walking_skeleton_pds_unreachable_leaves_local_claim_intact_and_retry_actionable() {
    let mut env = TestEnv::initialized();
    env.pds.simulate_unreachable();

    let outcome = run_openlore_with_stdin(
        &env,
        &[
            "claim", "add",
            "--subject", "github:rust-lang/rust",
            "--predicate", "embodiesPhilosophy",
            "--object", "org.openlore.philosophy.memory-safety",
            "--evidence", "https://www.rust-lang.org/",
            "--confidence", "0.86",
        ],
        "\nY\n",
    );

    assert_exit_nonzero_and_stderr_contains(&outcome, "PDS");
    assert_exit_nonzero_and_stderr_contains(&outcome, "retry with `openlore claim publish");

    // The local signed file is intact
    todo!("DELIVER: parse the CID from the (still-printed) sign-success stdout; assert_claim_file_exists_with_cid; restore env.pds; re-run `openlore claim publish <cid>`; assert it now succeeds")
}

// =============================================================================
// US-004 — Read back via graph query
// =============================================================================

/// WS-11: Graph query reads back the just-published claim faithfully —
/// all fields match the compose preview values byte-for-byte.
/// (US-004 AC #1 + KPI-4 round-trip identity.)
///
/// @walking_skeleton @driving_port @US-004 @J-001 @J-002 @real-io
#[test]
fn walking_skeleton_graph_query_returns_just_published_claim_byte_for_byte() {
    let env = TestEnv::initialized();
    let fixture = fixture_jeff_rust_memory_safety();

    // Publish via chained flow
    let _publish_outcome = run_openlore_with_stdin(
        &env,
        &[
            "claim", "add",
            "--subject", &fixture.subject,
            "--predicate", &fixture.predicate,
            "--object", &fixture.object,
            "--evidence", &fixture.evidence[0],
            "--confidence", &fixture.confidence.to_string(),
        ],
        "\nY\n",
    );

    // Query
    let query_outcome = run_openlore(&env, &["graph", "query", "--subject", &fixture.subject]);

    assert_exit_zero_and_stdout_contains(&query_outcome, &fixture.subject);
    // Every field shown matches the fixture
    todo!("DELIVER: parse CID from publish stdout; call assert_graph_query_output_matches_fixture(&query_outcome, &fixture, &cid)")
}

/// WS-12: Local-only is the default and is announced in the footer.
/// (US-004 AC #2 + WD-13 federation = slice-03.)
///
/// @walking_skeleton @driving_port @US-004 @J-001 @J-002 @real-io
#[test]
fn walking_skeleton_graph_query_default_is_local_only_and_footer_announces_it() {
    let env = TestEnv::initialized();

    // Seed one claim so the query has something to render
    let _ = run_openlore_with_stdin(
        &env,
        &[
            "claim", "add",
            "--subject", "github:rust-lang/rust",
            "--predicate", "embodiesPhilosophy",
            "--object", "org.openlore.philosophy.memory-safety",
            "--evidence", "https://www.rust-lang.org/",
            "--confidence", "0.86",
        ],
        "\nn\n",
    );

    let outcome = run_openlore(&env, &["graph", "query", "--subject", "github:rust-lang/rust"]);

    assert_exit_zero_and_stdout_contains(&outcome, "Showing local claims only");
    assert_exit_zero_and_stdout_contains(&outcome, "--federated");
    assert_exit_zero_and_stdout_contains(&outcome, "slice-03");

    todo!("DELIVER: implement the footer string per US-004 AC #2")
}

/// WS-13: Empty result is explained, not silent. (US-004 AC #3.)
///
/// @walking_skeleton @driving_port @US-004 @J-001 @J-002 @error
#[test]
fn walking_skeleton_graph_query_empty_result_is_explained_not_silent() {
    let env = TestEnv::initialized();

    let outcome = run_openlore(&env, &["graph", "query", "--subject", "github:nonexistent/repo"]);

    assert_exit_zero_and_stdout_contains(&outcome, "No local claims about github:nonexistent/repo");
    assert_exit_zero_and_stdout_contains(&outcome, "--federated");

    todo!("DELIVER: implement the empty-result message per US-004 AC #3")
}

// =============================================================================
// ADR-008 — Retraction = counter-claim referencing original CID
// =============================================================================

/// WS-14: `openlore claim retract <cid>` publishes a NEW counter-claim
/// whose `references` field includes `{type: retracts, cid: <cid>}`.
/// (ADR-008 §Adapter implications, §Behavioral rule 1.)
///
/// @walking_skeleton @driving_port @US-003 @J-001 @real-io
#[test]
fn walking_skeleton_retract_publishes_new_counter_claim_referencing_original() {
    let env = TestEnv::initialized();

    // Publish original
    let _publish_outcome = run_openlore_with_stdin(
        &env,
        &[
            "claim", "add",
            "--subject", "github:rust-lang/rust",
            "--predicate", "embodiesPhilosophy",
            "--object", "org.openlore.philosophy.memory-safety",
            "--evidence", "https://www.rust-lang.org/",
            "--confidence", "0.86",
        ],
        "\nY\n",
    );
    let original_cid = "bafy..."; // todo!("parse from _publish_outcome.stdout")

    // Retract
    let retract_outcome = run_openlore(&env, &["claim", "retract", original_cid]);

    assert_exit_zero_and_stdout_contains(&retract_outcome, "at-uri: at://did:plc:test-jeff/org.openlore.claim/");

    // The new claim's CID is different
    let retract_cid = "bafyDIFFERENT..."; // todo!("parse new CID from retract_outcome.stdout")
    assert!(retract_cid != original_cid, "retraction must have its own CID");

    // The new claim's references field points at the original
    assert_claim_references_retract(&env, retract_cid, original_cid);

    todo!("DELIVER: implement `claim retract`; ensure retraction is a fresh signed claim with references[{{type=retracts, cid=original_cid}}]")
}

/// WS-15: Retraction preserves the original record in BOTH the local
/// store AND the fake PDS. No hard-delete. The query lists BOTH claims
/// and annotates the original as "retracted by author". (ADR-008
/// §Behavioral rules 1, 2, 3.)
///
/// @walking_skeleton @driving_port @US-003 @J-001 @real-io
#[test]
fn walking_skeleton_retract_preserves_original_record_in_local_and_remote_stores() {
    let env = TestEnv::initialized();
    let original_cid = "bafy_ORIGINAL...";
    let _retract_cid = "bafy_RETRACT...";

    // Publish + retract (compose helper that DELIVER will write)
    todo!("DELIVER: publish original, then retract; assert original .json file still exists; assert env.pds.record_at(at://.../original_cid) still returns Some; run graph query; assert output lists BOTH claims with 'retracted by author' annotation on the original")
}

// =============================================================================
// Anxiety-path scenarios (from gherkin-scenarios-expanded.md, resolved per DD-9)
// =============================================================================

/// WS-16: Author corrects a typo'd evidence URL by publishing a fresh
/// claim with the right URL AND retracting the typo'd one. (Anxiety
/// scenario 2 from `gherkin-scenarios-expanded.md`; DD-9 binds the
/// "corrective workflow" to a two-step retract + republish using the
/// locked verbs.)
///
/// @walking_skeleton @driving_port @US-001 @US-003 @J-001 @real-io
#[test]
fn walking_skeleton_corrective_workflow_publishes_new_claim_and_retracts_old() {
    let env = TestEnv::initialized();

    // Publish typo'd
    let _typo_outcome = run_openlore_with_stdin(
        &env,
        &[
            "claim", "add",
            "--subject", "github:rust-lang/rust",
            "--predicate", "embodiesPhilosophy",
            "--object", "org.openlore.philosophy.memory-safety",
            "--evidence", "https://www.rustt-lang.org/", // typo
            "--confidence", "0.86",
        ],
        "\nY\n",
    );
    let typo_cid = "bafy_TYPO...";

    // Retract the typo
    let _retract = run_openlore(&env, &["claim", "retract", typo_cid]);

    // Publish corrected
    let _corrected_outcome = run_openlore_with_stdin(
        &env,
        &[
            "claim", "add",
            "--subject", "github:rust-lang/rust",
            "--predicate", "embodiesPhilosophy",
            "--object", "org.openlore.philosophy.memory-safety",
            "--evidence", "https://www.rust-lang.org/", // fixed
            "--confidence", "0.86",
        ],
        "\nY\n",
    );

    // Both claims + the retraction should appear in graph query;
    // typo'd one annotated as retracted
    todo!("DELIVER: implement the three-step corrective workflow; verify graph query lists original, retraction, and corrected claim; original annotated 'retracted by author'")
}

/// WS-17: Calibration anxiety: user reconsiders confidence after
/// seeing the bucket label, cancels with Ctrl-C, and re-runs with a
/// lower confidence. NO claim is signed in the intermediate exchange.
/// (Anxiety scenario 3 from `gherkin-scenarios-expanded.md`; DD-9
/// binds `--edit` to cancel + re-run.)
///
/// @walking_skeleton @driving_port @US-001 @J-001 @real-io
#[test]
fn walking_skeleton_calibration_anxiety_user_cancels_and_re_runs_with_lower_confidence() {
    let env = TestEnv::initialized();

    // First invocation: confidence 0.9, then cancel (close stdin without
    // sending Enter — the binary should exit non-zero / cleanly)
    let first = run_openlore_with_stdin(
        &env,
        &[
            "claim", "add",
            "--subject", "github:rust-lang/rust",
            "--predicate", "embodiesPhilosophy",
            "--object", "org.openlore.philosophy.memory-safety",
            "--evidence", "https://www.rust-lang.org/",
            "--confidence", "0.9",
        ],
        "", // empty stdin = cancel at the prompt (EOF)
    );

    assert_exit_zero_and_stdout_contains(&first, "0.90 (well-evidenced)");
    assert_exit_zero_and_stdout_contains(&first, "not as truth");

    // No file was written during the canceled exchange
    assert_no_local_claim_files_exist(&env);
    // No PDS call was made
    assert_no_pds_call_was_made(&env);

    // Second invocation: confidence 0.55, accept (sign), decline publish
    let second = run_openlore_with_stdin(
        &env,
        &[
            "claim", "add",
            "--subject", "github:rust-lang/rust",
            "--predicate", "embodiesPhilosophy",
            "--object", "org.openlore.philosophy.memory-safety",
            "--evidence", "https://www.rust-lang.org/",
            "--confidence", "0.55",
        ],
        "\nn\n",
    );

    assert_exit_zero_and_stdout_contains(&second, "0.55 (weighted)");

    // Exactly one signed file now exists (the 0.55 one)
    todo!("DELIVER: assert claims_dir() contains exactly one file; assert_persisted_payload_has_no_bucket_label on it")
}
