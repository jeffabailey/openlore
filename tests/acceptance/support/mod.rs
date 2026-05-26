//! Shared acceptance-test support.
//!
//! Step 05-01: TestEnv + run_openlore + WS-1's universe-bound
//! assertion helpers are now implemented. Other helpers (compose-preview,
//! signature-verification, graph-query parsers) remain as `todo!()`
//! scaffolds for subsequent phase-05 steps.
//!
//! Functional-paradigm note (ADR-007): helpers are free functions over
//! immutable values; no test-class hierarchy. Setup returns a `TestEnv`
//! VALUE; assertions are stand-alone functions; doubles are imported
//! from the shared `openlore-test-support` crate. Composition, not
//! inheritance.
//!
//! ## Subprocess seam (DD-2 + DD-5)
//!
//! WS scenarios spawn the real `openlore` binary via
//! `assert_cmd::Command::cargo_bin("openlore")`. The binary respects
//! three env-var seams for test isolation:
//!
//! - `OPENLORE_HOME` — tempdir root; XDG paths resolve under here.
//! - `OPENLORE_DID` — slice-01 stub for did:plc resolution (real PLC
//!   lookup is slice-03).
//! - `OPENLORE_KEY_SEED_HEX` — Ed25519 seed; matches FakeIdentity::jeff
//!   when set to 64 zeros so signatures cross-verify against the
//!   in-process pure-core verify.

#![allow(dead_code)]

use std::path::PathBuf;
use std::process::{Command, Stdio};

use openlore_test_support::{FakeIdentity as SharedFakeIdentity, FakePds as SharedFakePds};
use ports::IdentityPort;
use tempfile::TempDir;

/// A sealed test environment.
///
/// Holds an isolated `HOME` so XDG paths (`~/.config/openlore`,
/// `~/.local/share/openlore`) resolve under a temporary directory that
/// auto-cleans on drop.
///
/// One `TestEnv` per scenario. Multiple `TestEnv`s within one test
/// process do NOT share state (parallel-safe).
pub struct TestEnv {
    /// Owning handle to the tempdir; dropped when TestEnv is dropped.
    _tempdir: TempDir,
    /// Temporary HOME for this scenario. Auto-removed when the
    /// `TestEnv` is dropped.
    pub home: PathBuf,
    /// Fake PDS double (replaces `adapter-atproto-pds` for tests).
    pub pds: FakePds,
    /// Fake identity double (replaces `adapter-atproto-did` for tests).
    pub identity: FakeIdentity,
}

impl TestEnv {
    /// Spin up a fresh environment with no `~/.config/openlore` or
    /// `~/.local/share/openlore` directories.
    ///
    /// After `init()` the caller can invoke `openlore` via the
    /// subprocess helpers; the binary will write to `{home}/.config`
    /// and `{home}/.local/share`.
    pub fn fresh() -> Self {
        let tempdir = TempDir::new().expect("create tempdir for TestEnv");
        let home = tempdir.path().to_path_buf();
        Self {
            _tempdir: tempdir,
            home,
            pds: FakePds::start(),
            identity: FakeIdentity::jeff(),
        }
    }

    /// Convenience: a TestEnv that has already had `openlore init` run
    /// successfully. Most claim scenarios start here.
    ///
    /// Step 05-01 wires this for WS-1 only; subsequent claim scenarios
    /// will exercise it through the subprocess too.
    pub fn initialized() -> Self {
        let env = Self::fresh();
        let outcome = run_openlore(
            &env,
            &["init", "--handle", "jeff.test", "--app-password", "fake-app-password"],
        );
        if outcome.status != 0 {
            panic!(
                "TestEnv::initialized: openlore init failed (exit {}). \
                 stdout: {} stderr: {}",
                outcome.status, outcome.stdout, outcome.stderr
            );
        }
        env
    }

    /// Path to the local claims directory: `{home}/.local/share/openlore/claims/`.
    pub fn claims_dir(&self) -> PathBuf {
        self.home.join(".local").join("share").join("openlore").join("claims")
    }

    /// Path to the local DuckDB file: `{home}/.local/share/openlore/openlore.duckdb`.
    pub fn duckdb_path(&self) -> PathBuf {
        self.home
            .join(".local")
            .join("share")
            .join("openlore")
            .join("openlore.duckdb")
    }

    /// Path to the identity config: `{home}/.config/openlore/identity.toml`.
    pub fn identity_toml_path(&self) -> PathBuf {
        self.home.join(".config").join("openlore").join("identity.toml")
    }
}

/// A test double for `adapter-atproto-pds`.
///
/// Step 05-01 binds this to `openlore_test_support::FakePds`. The real
/// adapter's behavior is mirrored; subsequent steps that need
/// `simulate_unreachable()` get it for free through the underlying
/// shared implementation.
pub struct FakePds {
    inner: SharedFakePds,
}

/// One record as seen by the fake PDS.
#[derive(Debug, Clone)]
pub struct FakePdsRecord {
    pub collection: String,
    pub rkey: String,
    pub body: String, // canonical JSON
    pub author_did: String,
    /// `at://{author_did}/{collection}/{rkey}` — derived, stored for assertion convenience.
    pub at_uri: String,
}

impl FakePds {
    /// Start the fake PDS. Slice-01 init verb does not contact a PDS;
    /// subsequent claim-publish scenarios will exercise this through
    /// the subprocess by binding `OPENLORE_PDS_ENDPOINT` to an
    /// in-process HTTP stub. For now the struct just owns the shared
    /// implementation so `simulate_unreachable()` and friends compile.
    pub fn start() -> Self {
        Self {
            inner: SharedFakePds::for_did("did:plc:test-jeff"),
        }
    }

    /// All records the fake has accepted so far.
    pub fn records(&self) -> Vec<FakePdsRecord> {
        self.inner
            .records()
            .into_iter()
            .map(|r| FakePdsRecord {
                collection: r.collection,
                rkey: r.rkey,
                body: r.body.to_string(),
                author_did: r.author_did,
                at_uri: r.at_uri,
            })
            .collect()
    }

    /// Find one record by its at-uri.
    pub fn record_at(&self, at_uri: &str) -> Option<FakePdsRecord> {
        self.inner.record_at(at_uri).map(|r| FakePdsRecord {
            collection: r.collection,
            rkey: r.rkey,
            body: r.body.to_string(),
            author_did: r.author_did,
            at_uri: r.at_uri,
        })
    }

    /// Inject an "unreachable" failure mode: subsequent `create_record`
    /// calls return a network-error shape that the production adapter
    /// classifies as `PdsError::Unreachable`. Used by WS-10.
    pub fn simulate_unreachable(&mut self) {
        self.inner.simulate_unreachable();
    }

    /// Restore normal operation after `simulate_unreachable`.
    pub fn restore(&mut self) {
        self.inner.restore();
    }
}

/// A test double for `adapter-atproto-did`.
///
/// Holds a known test DID (`did:plc:test-jeff`) and a deterministic
/// Ed25519 keypair. The OpenLore binary uses the same seed (via
/// `OPENLORE_KEY_SEED_HEX`) so signatures cross-verify against the
/// shared `openlore_test_support::FakeIdentity` keypair byte-for-byte.
pub struct FakeIdentity {
    inner: SharedFakeIdentity,
    /// 32-byte Ed25519 seed encoded as 64-char lowercase hex. Passed to
    /// the binary via `OPENLORE_KEY_SEED_HEX` so the in-binary adapter
    /// derives the same keypair the test double uses.
    pub seed_hex: String,
}

impl FakeIdentity {
    /// Construct the canonical fake identity used across slice-01 tests.
    ///
    /// Seed: 32 zero bytes (matches `openlore_test_support::FakeIdentity::jeff`).
    pub fn jeff() -> Self {
        Self {
            inner: SharedFakeIdentity::jeff(),
            seed_hex: "0".repeat(64),
        }
    }

    /// A second known identity used by anxiety-scenario tests that
    /// involve Maria (US-002 Example 3, US-003 Example 2, WS-10).
    pub fn maria() -> Self {
        // Maria's seed is 32 bytes of 0x01 per the shared FakeIdentity.
        let seed_hex: String = std::iter::repeat("01").take(32).collect();
        Self {
            inner: SharedFakeIdentity::maria(),
            seed_hex,
        }
    }

    /// The raw author DID (without the key fragment).
    pub fn author_did(&self) -> &str {
        &self.inner.author_did().0
    }
}

// -----------------------------------------------------------------------------
// Builders for canonical fixture claims
// -----------------------------------------------------------------------------
//
// One free function per "well-known fixture claim" used across multiple
// scenarios. Functional paradigm: each returns a fresh value (no shared
// mutable state); tests compose by passing them through.

/// The canonical Jeff-on-Rust claim from US-001 Example 1 and the
/// journey YAML.
pub fn fixture_jeff_rust_memory_safety() -> UnsignedClaimFixture {
    todo!("DELIVER: build the UnsignedClaimFixture matching US-001 Example 1")
}

/// The Maria-on-Mastodon claim from US-001 Example 2 (confidence
/// boundary, 0.55, displayed as 'weighted').
pub fn fixture_maria_mastodon_federation_first() -> UnsignedClaimFixture {
    todo!("DELIVER: build the UnsignedClaimFixture matching US-001 Example 2")
}

/// Three claims about different subjects, used by FR-1 for the
/// federation round-trip.
pub fn fixture_three_claims_different_predicates() -> Vec<UnsignedClaimFixture> {
    todo!("DELIVER: return three diverse claims (e.g. Rust+memory-safety, Linux+unix-philosophy, Mastodon+federation-first)")
}

/// Pure-language data-only shape DELIVER turns into a clap-parseable
/// flag set OR a direct in-process invocation, depending on which test
/// uses it. The acceptance tests serialize this to CLI flags; the
/// lexicon-conformance tests pass it through the pure core directly.
#[derive(Debug, Clone)]
pub struct UnsignedClaimFixture {
    pub subject: String,
    pub predicate: String,
    pub object: String,
    pub evidence: Vec<String>,
    pub confidence: f64,
    pub author_did: String, // e.g. "did:plc:test-jeff#org.openlore.application"
    pub composed_at: String, // RFC3339 UTC; DELIVER pins this to a known value for determinism
    pub references: Vec<ReferenceFixture>,
}

/// One typed reference (ADR-008 §Lexicon-level design).
#[derive(Debug, Clone)]
pub struct ReferenceFixture {
    pub ref_type: String, // "retracts" | "corrects" | "counters" | "supersedes"
    pub cid: String,
}

// -----------------------------------------------------------------------------
// Subprocess helpers — invoke the real `openlore` binary
// -----------------------------------------------------------------------------

/// Captured outcome of one `openlore` invocation.
#[derive(Debug)]
pub struct CliOutcome {
    pub status: i32,
    pub stdout: String,
    pub stderr: String,
}

/// Run `openlore <args>` with `OPENLORE_HOME` set to `env.home`, plus
/// the slice-01 stub env vars (`OPENLORE_DID`, `OPENLORE_KEY_SEED_HEX`)
/// that drive the in-binary IdentityPort adapter against the same
/// keypair `env.identity` advertises.
///
/// Provides no stdin; for scenarios that need to send `<Enter>` / `<Y>`
/// at the chained prompts use `run_openlore_with_stdin`.
pub fn run_openlore(env: &TestEnv, args: &[&str]) -> CliOutcome {
    run_openlore_with_stdin(env, args, "")
}

/// Run `openlore <args>` feeding `stdin_lines` (newline-joined) on
/// stdin. Used for the two-prompt chained flow: pass "\n" to confirm
/// the sign prompt, "Y\n" to confirm publish.
pub fn run_openlore_with_stdin(
    env: &TestEnv,
    args: &[&str],
    stdin_lines: &str,
) -> CliOutcome {
    use std::io::Write;

    let bin = assert_cmd::cargo::cargo_bin("openlore");
    let mut cmd = Command::new(&bin);
    cmd.args(args)
        .env_clear()
        .env("OPENLORE_HOME", &env.home)
        .env("OPENLORE_DID", env.identity.author_did())
        .env("OPENLORE_KEY_SEED_HEX", &env.identity.seed_hex)
        // PATH is required for libc / dynamic linker resolution on
        // some hosts; pass through the parent's PATH so `cargo bin`
        // can launch.
        .env(
            "PATH",
            std::env::var("PATH").unwrap_or_default(),
        )
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
    // Close stdin so the child observes EOF if it's waiting on a prompt.
    drop(child.stdin.take());

    let output = child.wait_with_output().expect("wait_with_output");
    CliOutcome {
        status: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    }
}

/// Convenience: run with the scripting-mode `--no-tty` flag and the
/// implied `Enter` then `Y`. Used by scenarios that don't care about
/// the interactive prompt itself — only the observable result.
pub fn run_openlore_no_tty(env: &TestEnv, args: &[&str]) -> CliOutcome {
    todo!("DELIVER: invoke run_openlore_with_stdin with the standard scripting confirmations")
}

// -----------------------------------------------------------------------------
// Assertion helpers — universe-bound observable checks
// -----------------------------------------------------------------------------

/// Assert that the CLI invocation exited with status 0 and the given
/// substring appears in stdout. Failure prints the full stdout/stderr
/// for debuggability.
pub fn assert_exit_zero_and_stdout_contains(outcome: &CliOutcome, expected_substring: &str) {
    assert_eq!(
        outcome.status,
        0,
        "expected exit 0; got {} \n--- stdout ---\n{}\n--- stderr ---\n{}",
        outcome.status,
        outcome.stdout,
        outcome.stderr
    );
    assert!(
        outcome.stdout.contains(expected_substring),
        "expected stdout to contain {:?} \n--- stdout ---\n{}\n--- stderr ---\n{}",
        expected_substring,
        outcome.stdout,
        outcome.stderr
    );
}

/// Assert non-zero exit AND the stderr contains the given substring.
pub fn assert_exit_nonzero_and_stderr_contains(outcome: &CliOutcome, expected_substring: &str) {
    assert_ne!(
        outcome.status, 0,
        "expected non-zero exit; got 0 \n--- stdout ---\n{}\n--- stderr ---\n{}",
        outcome.stdout, outcome.stderr
    );
    assert!(
        outcome.stderr.contains(expected_substring),
        "expected stderr to contain {:?} \n--- stdout ---\n{}\n--- stderr ---\n{}",
        expected_substring, outcome.stdout, outcome.stderr
    );
}

/// Universe-bound: "the compose preview contains the literal text
/// 'not as truth'". Asserts on `outcome.stdout`. Port-exposed name:
/// `cli.compose_preview.literal_not_as_truth_present`.
pub fn assert_compose_preview_contains_not_as_truth(outcome: &CliOutcome) {
    todo!("DELIVER: assert outcome.stdout.contains(\"not as truth\")")
}

/// Universe-bound: "no file was written under
/// `{home}/.local/share/openlore/claims/`". Port-exposed name:
/// `storage.local_claim_store.file_count`.
pub fn assert_no_local_claim_files_exist(env: &TestEnv) {
    todo!("DELIVER: scan env.claims_dir(); assert it is empty or does not exist")
}

/// Universe-bound: "a file exists at
/// `{home}/.local/share/openlore/claims/<cid>.json` AND its content
/// canonicalizes to a CBOR sequence whose sha2-256 matches <cid>".
/// Port-exposed name: `storage.local_claim_store.file_for_cid_valid`.
pub fn assert_claim_file_exists_with_cid(env: &TestEnv, cid: &str) {
    todo!("DELIVER: read claims_dir/<cid>.json; canonicalize; re-compute CID; assert equality")
}

/// Universe-bound: "no `create_record` call was made on the fake PDS".
/// Port-exposed name: `pds.create_record.call_count`.
pub fn assert_no_pds_call_was_made(env: &TestEnv) {
    todo!("DELIVER: assert env.pds.records().is_empty()")
}

/// Universe-bound: "the fake PDS contains a record at
/// `at://{author_did}/org.openlore.claim/<cid>`". Port-exposed name:
/// `pds.records.contains_at_uri`.
pub fn assert_pds_contains_record_at(env: &TestEnv, at_uri: &str) {
    todo!("DELIVER: assert env.pds.record_at(at_uri).is_some()")
}

/// Universe-bound: "the published record's signature verifies against
/// the given test DID's public key". Port-exposed name:
/// `pds.records.signature_verifies_against_did`.
pub fn assert_pds_record_signature_verifies(env: &TestEnv, at_uri: &str, did: &str) {
    todo!("DELIVER: fetch the record from env.pds, run claim_domain::verify against the FakeIdentity public key for `did`")
}

/// Universe-bound: "the graph-query output, parsed line by line,
/// matches the field values of the given fixture claim". Port-exposed
/// name: `cli.graph_query.output_field_for_field_match`.
pub fn assert_graph_query_output_matches_fixture(
    outcome: &CliOutcome,
    fixture: &UnsignedClaimFixture,
    expected_cid: &str,
) {
    todo!("DELIVER: parse outcome.stdout, locate the row for fixture.subject, assert every shown field equals the fixture")
}

/// Universe-bound: "the persisted JSON file does NOT contain the
/// substring 'speculative', 'weighted', 'well-evidenced', or
/// 'triangulated' in the confidence-bearing area". Port-exposed name:
/// `storage.local_claim_store.no_bucket_label_string`.
pub fn assert_persisted_payload_has_no_bucket_label(env: &TestEnv, cid: &str) {
    todo!("DELIVER: read claims_dir/<cid>.json; assert none of the four bucket-label strings appear")
}

/// Universe-bound: "the DuckDB row for the given CID has `published_at`
/// non-null AND its `at_uri` equals
/// `at://{author_did}/org.openlore.claim/<cid>`". Port-exposed name:
/// `storage.duckdb.publication_metadata_consistent`.
pub fn assert_duckdb_publication_metadata_for_cid(env: &TestEnv, cid: &str, expected_at_uri: &str) {
    todo!("DELIVER: open the DuckDB at env.duckdb_path(); query claims where cid=?; assert published_at IS NOT NULL and at_uri = expected_at_uri")
}

/// Universe-bound: "the retraction's `references` field includes
/// `{type: \"retracts\", cid: <original_cid>}`". Port-exposed name:
/// `claim.references.contains_retracts_target`.
pub fn assert_claim_references_retract(env: &TestEnv, retract_cid: &str, original_cid: &str) {
    todo!("DELIVER: read claims_dir/<retract_cid>.json, parse references array, assert one entry has type=retracts and cid=original_cid")
}
