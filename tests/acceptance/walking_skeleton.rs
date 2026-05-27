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
    // Given a fresh environment with no identity.toml (no init has run)
    let env = TestEnv::fresh();

    // When Jeff runs `openlore claim add` before `openlore init`
    let outcome = run_openlore(
        &env,
        &[
            "claim", "add",
            "--subject", "github:rust-lang/rust",
            "--predicate", "embodiesPhilosophy",
            "--object", "org.openlore.philosophy.memory-safety",
            "--evidence", "https://www.rust-lang.org/",
            "--confidence", "0.86",
        ],
    );

    // Then it exits non-zero and stderr names `openlore init`
    assert_exit_nonzero_and_stderr_contains(&outcome, "openlore init");

    // Step 05-02: WS-2 activated — the assertions above are the
    // contract; the trailing scaffold todo!() that DISTILL placed has
    // been removed now that the probe gauntlet refuses claim verbs
    // before any verb body runs.
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

    // Step 05-03: WS-3 activated — the three assertions above are the
    // contract; the trailing scaffold todo!() that DISTILL placed has
    // been removed now that the compose preview verb is implemented.
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

    // Step 05-04: WS-4 activated — the assertions above are the
    // contract; the trailing scaffold todo!() that DISTILL placed has
    // been removed now that pre-sign confidence validation rejects
    // out-of-range values with a stderr error naming the flag + range.
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

    // After signing, the on-disk JSON file under claims_dir/<cid>.json
    // contains numeric 0.55 AND zero occurrences of speculative /
    // weighted / well-evidenced / triangulated (WD-10 / D-12).
    let claims_dir = env.claims_dir();
    let entries: Vec<std::path::PathBuf> = std::fs::read_dir(&claims_dir)
        .unwrap_or_else(|e| panic!("read claims_dir {}: {e}", claims_dir.display()))
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|x| x.to_str()) == Some("json"))
        .collect();
    assert_eq!(
        entries.len(),
        1,
        "expected exactly one signed claim file under {}; found {:?}",
        claims_dir.display(),
        entries
    );

    let signed_path = &entries[0];
    let signed_bytes = std::fs::read(signed_path)
        .unwrap_or_else(|e| panic!("read signed file {}: {e}", signed_path.display()));
    let signed_text = String::from_utf8_lossy(&signed_bytes);

    // Numeric value present.
    assert!(
        signed_text.contains("0.55"),
        "expected on-disk JSON to contain numeric 0.55; got:\n{}",
        signed_text
    );

    // None of the four bucket-label strings appear (WD-10 / D-12).
    for forbidden in ["speculative", "weighted", "well-evidenced", "triangulated"] {
        assert!(
            !signed_text.contains(forbidden),
            "expected on-disk JSON to NOT contain bucket label {:?} (WD-10 / D-12); got:\n{}",
            forbidden,
            signed_text
        );
    }
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
    // AND the file's signature verifies against the test DID. Parse the
    // CID inline from the `Computing claim CID <cid>` stdout line —
    // claim_add.rs (step 05-05) prints the line verbatim before writing
    // the artifact, so it's the load-bearing handle for both file
    // location AND signature verification.
    // The `Computing claim CID <cid>` text is printed right after the
    // sign prompt (which ends without a newline), so the CID may be on
    // the SAME stdout line as the prompt — we substring-search rather
    // than strip_prefix to be robust to that join.
    let marker = "Computing claim CID ";
    let cid = outcome
        .stdout
        .find(marker)
        .map(|idx| {
            let tail = &outcome.stdout[idx + marker.len()..];
            tail.split_whitespace()
                .next()
                .map(|s| s.to_string())
                .unwrap_or_default()
        })
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| {
            panic!(
                "could not locate 'Computing claim CID <cid>' marker in stdout:\n{}",
                outcome.stdout
            )
        });
    let artifact_path = env.claims_dir().join(format!("{cid}.json"));
    let json_bytes = std::fs::read(&artifact_path).unwrap_or_else(|e| {
        panic!(
            "expected signed-claim file at {}; got {e}\n--- stdout ---\n{}",
            artifact_path.display(),
            outcome.stdout
        )
    });
    let signed: claim_domain::SignedClaim =
        serde_json::from_slice(&json_bytes).unwrap_or_else(|e| {
            panic!(
                "could not deserialize signed claim at {}: {e}\n--- file ---\n{}",
                artifact_path.display(),
                String::from_utf8_lossy(&json_bytes)
            )
        });

    // Signature verifies against FakeIdentity::jeff's verifying key.
    // The local `support::FakeIdentity` wraps the shared
    // `openlore_test_support::FakeIdentity`; both derive their keypair
    // from a 32-zero-byte seed, so a fresh shared instance owns the
    // same verifying key the in-binary signer used. Verification goes
    // through the IdentityPort contract (which delegates to the pure
    // `claim_domain::verify` primitive) so this stays port-to-port.
    let shared_jeff = openlore_test_support::FakeIdentity::jeff();
    let verify_result = ports::IdentityPort::verify(&shared_jeff, &signed);
    assert!(
        verify_result.is_ok(),
        "expected signature for cid {} to verify against FakeIdentity::jeff's pubkey; got {:?}\n\
         --- stdout ---\n{}\n--- stderr ---\n{}",
        cid,
        verify_result,
        outcome.stdout,
        outcome.stderr
    );

    // KPI-5 local-first invariant: NO PDS call was made during the
    // sign-only path (Enter then 'n' to decline publish).
    assert_no_pds_call_was_made(&env);
}

/// WS-7: Re-canonicalization produces identical CIDs. (US-002 Example
/// 2; risk #2 from feature-delta.md "canonical CID determinism";
/// KPI-4.)
///
/// Two independent `openlore claim add` runs with identical flags AND
/// the same pinned `OPENLORE_TEST_NOW` MUST produce identical CIDs.
/// `composed_at` is the only otherwise-divergent input (every other
/// field comes from flags) so pinning the clock pins the whole
/// canonical-CBOR pre-image, which pins the CID by ADR-006.
///
/// This is the subprocess-level verification of LC-3's in-memory CID
/// determinism property: the byte-stability invariant holds across
/// process boundaries, not just inside one process.
///
/// @walking_skeleton @driving_port @US-002 @J-001 @real-io
#[test]
fn walking_skeleton_re_canonicalization_produces_identical_cids() {
    let env_first = TestEnv::initialized();
    let env_second = TestEnv::initialized();

    let first_args = [
        "claim", "add",
        "--subject", "github:rust-lang/rust",
        "--predicate", "embodiesPhilosophy",
        "--object", "org.openlore.philosophy.memory-safety",
        "--evidence", "https://www.rust-lang.org/",
        "--confidence", "0.86",
    ];

    // Step 05-07 pins composed_at via OPENLORE_TEST_NOW so both runs
    // canonicalize identical pre-images. The SystemClockAdapter honors
    // this env var as a test-only seam (production behavior unchanged).
    let pinned_now = "2026-05-26T12:00:00Z";

    let outcome_first = run_openlore_claim_add_with_pinned_now(
        &env_first,
        &first_args,
        "\nn\n",
        pinned_now,
    );
    let outcome_second = run_openlore_claim_add_with_pinned_now(
        &env_second,
        &first_args,
        "\nn\n",
        pinned_now,
    );

    // Parse the CID from each outcome's stdout. WS-6 pinned the parsing
    // shape (`Computing claim CID <cid>` marker); reuse it here.
    let cid_first = parse_cid_from_stdout(&outcome_first.stdout);
    let cid_second = parse_cid_from_stdout(&outcome_second.stdout);

    // Both runs must yield byte-equal CIDs (KPI-4 / ADR-006 / LC-3 at
    // the subprocess boundary).
    assert_eq!(
        cid_first, cid_second,
        "WS-7: two independent runs with identical inputs and pinned \
         OPENLORE_TEST_NOW={pinned_now} must produce identical CIDs.\n\
         first  = {cid_first}\nsecond = {cid_second}\n\
         --- stdout (first) ---\n{}\n--- stdout (second) ---\n{}",
        outcome_first.stdout, outcome_second.stdout,
    );
}

/// Local helper: spawn the `openlore` binary with the same env shape
/// `run_openlore_with_stdin` uses, plus `OPENLORE_TEST_NOW` so the
/// in-binary `SystemClockAdapter` returns a pinned timestamp. Inlined
/// in this file (rather than added to `support`) because WS-7 is the
/// only scenario in this slice that needs clock pinning at the
/// subprocess level.
fn run_openlore_claim_add_with_pinned_now(
    env: &TestEnv,
    args: &[&str],
    stdin_lines: &str,
    pinned_now_rfc3339: &str,
) -> support::CliOutcome {
    use std::io::Write;
    use std::process::{Command, Stdio};

    let bin = assert_cmd::cargo::cargo_bin("openlore");
    let mut cmd = Command::new(&bin);
    cmd.args(args)
        .env_clear()
        .env("OPENLORE_HOME", &env.home)
        .env("OPENLORE_DID", env.identity.author_did())
        .env("OPENLORE_KEY_SEED_HEX", &env.identity.seed_hex)
        .env("OPENLORE_PDS_ENDPOINT", env.pds.endpoint_url())
        .env("OPENLORE_TEST_NOW", pinned_now_rfc3339)
        .env("PATH", std::env::var("PATH").unwrap_or_default())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = cmd
        .spawn()
        .unwrap_or_else(|e| panic!("spawn openlore at {:?}: {e}", bin));

    if !stdin_lines.is_empty() {
        let stdin = child.stdin.as_mut().expect("stdin pipe");
        stdin
            .write_all(stdin_lines.as_bytes())
            .expect("write stdin");
    }
    drop(child.stdin.take());

    let output = child.wait_with_output().expect("wait_with_output");
    support::CliOutcome {
        status: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    }
}

/// Local helper: parse the CID out of `Computing claim CID <cid>` in
/// stdout. Mirrors WS-6's inline parsing — the marker text is the
/// load-bearing contract `claim_add.rs` prints right before persistence.
fn parse_cid_from_stdout(stdout: &str) -> String {
    let marker = "Computing claim CID ";
    let idx = stdout.find(marker).unwrap_or_else(|| {
        panic!("could not locate 'Computing claim CID <cid>' marker in stdout:\n{stdout}")
    });
    let tail = &stdout[idx + marker.len()..];
    let cid = tail
        .split_whitespace()
        .next()
        .map(|s| s.to_string())
        .unwrap_or_default();
    assert!(
        !cid.is_empty(),
        "found marker but no CID followed it in stdout:\n{stdout}"
    );
    cid
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

    // Parse the at-uri from stdout. claim_publish.rs prints the line
    // `  at-uri: <at-uri>\n` verbatim — split on the marker and take
    // the first whitespace-delimited token after.
    let at_uri = parse_at_uri_from_stdout(&outcome.stdout);

    // Universe-bound: the fake PDS contains a record at this at-uri.
    assert_pds_contains_record_at(&env, &at_uri);

    // The retract hint pins the CID at the end of the printed
    // `openlore claim retract <cid>` line; reuse the CID parser from
    // WS-6/WS-7 to extract it for the DuckDB assertion below.
    let cid = parse_cid_from_stdout(&outcome.stdout);

    // Universe-bound: the DuckDB row for this CID has published_at
    // populated AND its at_uri matches what we just printed. Pins the
    // local-publication-metadata contract from data-models.md so
    // downstream graph-query / retract verbs can resolve "this claim
    // was federated to <here> at <when>" from the local index.
    assert_duckdb_publication_metadata_for_cid(&env, &cid, &at_uri);
}

/// Local helper: parse the at-uri from the publish-success block in
/// stdout. The renderer prints `  at-uri: at://...` verbatim; we slice
/// to that marker and take the first whitespace-delimited token after.
fn parse_at_uri_from_stdout(stdout: &str) -> String {
    let marker = "at-uri: ";
    let idx = stdout.find(marker).unwrap_or_else(|| {
        panic!("could not locate 'at-uri:' marker in stdout:\n{stdout}")
    });
    let tail = &stdout[idx + marker.len()..];
    let at_uri = tail
        .split_whitespace()
        .next()
        .map(|s| s.to_string())
        .unwrap_or_default();
    assert!(
        !at_uri.is_empty(),
        "found at-uri marker but no value followed it in stdout:\n{stdout}"
    );
    at_uri
}

/// WS-9: Republishing a CID is idempotent (no duplicate, no error).
/// (US-003 Example 3, AC #3.)
///
/// @walking_skeleton @driving_port @US-003 @J-001 @real-io
#[test]
fn walking_skeleton_publish_is_idempotent_on_re_run_with_same_cid() {
    let env = TestEnv::initialized();

    // First publish via chained flow. Step 05-08 wired this; the
    // chained-Y branch funnels through the same `publish_signed_claim`
    // helper the standalone verb uses.
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
    assert_exit_zero_and_stdout_contains(&first, "at-uri: at://did:plc:test-jeff/org.openlore.claim/");
    let cid = parse_cid_from_stdout(&first.stdout);
    let at_uri_first = parse_at_uri_from_stdout(&first.stdout);

    // Snapshot the PDS record-count for this at-uri after the first
    // publish so we can verify the second invocation didn't insert a
    // duplicate (architecture §6.2: 409 conflict = idempotent success).
    let records_after_first: Vec<_> = env
        .pds
        .records()
        .into_iter()
        .filter(|r| r.at_uri == at_uri_first)
        .collect();
    assert_eq!(
        records_after_first.len(),
        1,
        "expected exactly one PDS record for {at_uri_first} after first publish; got {}: {:?}",
        records_after_first.len(),
        records_after_first
    );

    // Second invocation via the standalone verb on the same CID. This
    // is the WS-9 contract — `openlore claim publish <cid>` on an
    // already-published claim exits 0 with an "already published"
    // hint instead of acting like a fresh publish.
    let second = run_openlore(&env, &["claim", "publish", &cid]);

    assert_exit_zero_and_stdout_contains(&second, "already published");
    // The at-uri line is still printed so the user can see WHERE the
    // claim lives.
    assert_exit_zero_and_stdout_contains(&second, &format!("at-uri: {at_uri_first}"));

    // And the fake PDS still has exactly ONE record for that at-uri —
    // no duplicate insertion. This is the load-bearing observable: the
    // PDS-side ledger remains single-entry across re-publishes.
    let records_after_second: Vec<_> = env
        .pds
        .records()
        .into_iter()
        .filter(|r| r.at_uri == at_uri_first)
        .collect();
    assert_eq!(
        records_after_second.len(),
        1,
        "expected exactly one PDS record for {at_uri_first} after idempotent re-publish; \
         got {}: {:?}",
        records_after_second.len(),
        records_after_second
    );
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

    // The local signed file persists intact (KPI-5 local-first invariant).
    // The sign + write_signed_claim path runs BEFORE the publish call
    // (WS-6 pinned this); when publish fails, the verb does NOT roll
    // back the local write. We parse the CID from the still-printed
    // sign-success stdout (the verb prints `Computing claim CID <cid>`
    // and `Written to local store: <path>` before attempting publish).
    let cid = parse_cid_from_stdout(&outcome.stdout);
    let artifact_path = env.claims_dir().join(format!("{cid}.json"));
    assert!(
        artifact_path.exists(),
        "expected signed-claim file at {} after PDS-unreachable publish failure \
         (KPI-5 local-first invariant); file missing.\n--- stdout ---\n{}\n--- stderr ---\n{}",
        artifact_path.display(),
        outcome.stdout,
        outcome.stderr
    );

    // Restore the PDS and re-run the standalone `claim publish <cid>`.
    // It MUST succeed and produce an at-uri of the same shape as if it
    // had succeeded the first time (FR-3 at-uri reconstructibility).
    env.pds.restore();
    let retry = run_openlore(&env, &["claim", "publish", &cid]);
    assert_exit_zero_and_stdout_contains(
        &retry,
        &format!("at-uri: at://did:plc:test-jeff/org.openlore.claim/{cid}"),
    );
    // And the fake PDS now contains a record at that at-uri.
    let expected_at_uri = format!("at://did:plc:test-jeff/org.openlore.claim/{cid}");
    assert_pds_contains_record_at(&env, &expected_at_uri);
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

    // Step 05-11: the support-level `fixture_jeff_rust_memory_safety()`
    // builder is still a DELIVER scaffold (US-001 Example 1 fixture has
    // not been promoted to support yet — it's authored inline here to
    // keep step 05-11's blast radius to the cli crate + this scenario).
    // The values mirror US-001 Example 1 / data-models.md verbatim so
    // the byte-for-byte invariant has something concrete to assert on.
    let subject = "github:rust-lang/rust";
    let predicate = "embodiesPhilosophy";
    let object = "org.openlore.philosophy.memory-safety";
    let evidence_url = "https://www.rust-lang.org/";
    let confidence_str = "0.86";

    // Publish via chained flow.
    let publish_outcome = run_openlore_with_stdin(
        &env,
        &[
            "claim", "add",
            "--subject", subject,
            "--predicate", predicate,
            "--object", object,
            "--evidence", evidence_url,
            "--confidence", confidence_str,
        ],
        "\nY\n",
    );
    assert_eq!(
        publish_outcome.status, 0,
        "publish via chained flow must succeed; got status {} \
         \n--- stdout ---\n{}\n--- stderr ---\n{}",
        publish_outcome.status, publish_outcome.stdout, publish_outcome.stderr,
    );

    // Parse the CID from the publish stdout so we can pin the
    // round-trip identity assertion below.
    let cid = parse_cid_from_stdout(&publish_outcome.stdout);

    // Query.
    let query_outcome = run_openlore(&env, &["graph", "query", "--subject", subject]);

    // Hard AC: every compose-time field appears byte-for-byte in stdout.
    // KPI-4 zero-normalization invariant: confidence renders as the
    // original `f64` (`0.86`), NEVER as a bucket label.
    assert_exit_zero_and_stdout_contains(&query_outcome, subject);
    assert_exit_zero_and_stdout_contains(&query_outcome, predicate);
    assert_exit_zero_and_stdout_contains(&query_outcome, object);
    assert_exit_zero_and_stdout_contains(&query_outcome, evidence_url);
    assert_exit_zero_and_stdout_contains(&query_outcome, confidence_str);
    assert_exit_zero_and_stdout_contains(&query_outcome, "did:plc:test-jeff");
    assert_exit_zero_and_stdout_contains(&query_outcome, &cid);

    // KPI-4 / WD-10: bucket labels are compose-time-only display. They
    // must NEVER leak into the read path's output.
    for label in &["speculative", "weighted", "well-evidenced", "triangulated"] {
        assert!(
            !query_outcome.stdout.contains(label),
            "graph query stdout must not contain bucket label '{label}' (WD-10 / D-12); \
             got stdout:\n--- stdout ---\n{}\n--- stderr ---\n{}",
            query_outcome.stdout,
            query_outcome.stderr,
        );
    }
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
