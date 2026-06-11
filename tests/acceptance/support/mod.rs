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

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::process::{Command, Stdio};

// DD-FED-10: the universe-bound state-delta assertion module (DD-3
// bootstrap, `tests/common/state_delta.rs`). Pulled in via the documented
// `#[path]` seam (see `tests/common/mod.rs` §Usage) so the PS-6 hard-purge
// scenario can assert the FULL observable purge universe in one call rather
// than a scatter of single-slot asserts.
#[path = "../../common/state_delta.rs"]
pub mod state_delta;

use openlore_test_support::fake_pds::FakePdsHttpHandle;
use openlore_test_support::{FakeIdentity as SharedFakeIdentity, FakePds as SharedFakePds};
use ports::IdentityPort;
use tempfile::TempDir;

// Slice-03 (step 01-05): the peer-PDS double + canonical peer fixtures are
// re-exported flat so the `peer_*`, `counter_claim`, and `federated_query`
// acceptance files can name them via `use support::*` (matching how the
// slice-01 fixtures already surface). The `FakePeerPds` HTTP runtime is
// wrapped below in [`PeerPds`] so it owns its own tokio runtime the same
// way [`FakePds`] does.
pub use openlore_test_support::fake_peer_pds::{
    FakePeerPds as SharedFakePeerPds, FakePeerPdsHttpHandle, FakePeerRecord, ADVERSARIAL_RKEY,
    PEER_CLAIM_COLLECTION,
};
pub use openlore_test_support::{
    fixture_adversarial_peer_cid_mismatch, fixture_adversarial_peer_cross_attribution,
    fixture_adversarial_peer_self_attribution, fixture_adversarial_peer_tampered_signature,
    fixture_other_developer_three_claims,
};

// Slice-05 (step 03-01): the ingest fixtures + the fixture keypair the AV-1
// walking-skeleton scenario seeds are re-exported flat so the `indexer_ingest`
// acceptance file can name them via `use support::*` (matching how the slice-01
// /03 fixtures already surface).
pub use openlore_test_support::{
    corpus_deno_dependency_pinning_two_authors, corpus_priya_eight_claims_six_subjects,
    fixture_ingest_adversarial_set_plus_one_valid, fixture_ingest_valid_signed, FixtureKeypair,
    Posture, RawRecordSpec, PRIYA_DID, SVEN_DID,
};

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
            &[
                "init",
                "--handle",
                "jeff.test",
                "--app-password",
                "fake-app-password",
            ],
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
        self.home
            .join(".local")
            .join("share")
            .join("openlore")
            .join("claims")
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
        self.home
            .join(".config")
            .join("openlore")
            .join("identity.toml")
    }
}

/// A test double for `adapter-atproto-pds`.
///
/// Step 05-01 binds this to `openlore_test_support::FakePds`. The real
/// adapter's behavior is mirrored; subsequent steps that need
/// `simulate_unreachable()` get it for free through the underlying
/// shared implementation.
///
/// Step 05-08 (Approach B per crafter design): owns a multi-threaded
/// tokio runtime + an in-process HTTP XRPC server bound to a random
/// `127.0.0.1` port. The server reuses `inner`'s record state via
/// `Arc`, so writes arriving from the `openlore` subprocess over
/// `OPENLORE_PDS_ENDPOINT=<url>` are visible to in-process assertions
/// via `records()` / `record_at()` / `record_count()` — one source of
/// truth across both surfaces. Dropping `FakePds` drops the runtime
/// which aborts the server task — RAII per-scenario isolation.
pub struct FakePds {
    inner: SharedFakePds,
    /// Live HTTP server handle. Dropped (and the server task aborted)
    /// when `TestEnv` is dropped — RAII isolation per scenario.
    http_handle: FakePdsHttpHandle,
    /// Owning handle to the multi-threaded tokio runtime backing the
    /// HTTP server. Held for the lifetime of `FakePds` so spawned tasks
    /// continue to make progress between `run_openlore_*` calls.
    /// `ManuallyDrop` + the explicit `Drop` impl below let us release
    /// the runtime on a background thread so we never block the test
    /// thread on shutdown.
    runtime: std::mem::ManuallyDrop<tokio::runtime::Runtime>,
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
    /// Start the fake PDS and spin up its in-process HTTP XRPC server.
    ///
    /// The HTTP server binds to `127.0.0.1` on an OS-assigned port. The
    /// URL is exposed via [`FakePds::endpoint_url`] so the test harness
    /// can wire it into the subprocess as `OPENLORE_PDS_ENDPOINT`. The
    /// server's backing record store is the same `Arc`-shared state the
    /// in-process port methods read — assertions on `records()` /
    /// `record_at()` observe the union of in-process writes AND HTTP
    /// writes from the spawned `openlore` binary.
    pub fn start() -> Self {
        // A dedicated multi-threaded runtime per FakePds so the HTTP
        // server can accept connections concurrently with whatever the
        // test thread is doing (spawning `openlore` subprocesses,
        // reading their stdout, etc).
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_io()
            .enable_time()
            .thread_name("fake-pds-rt")
            .build()
            .expect("FakePds::start: build tokio multi_thread runtime");
        let inner = SharedFakePds::for_did("did:plc:test-jeff");
        let http_handle = runtime.block_on(inner.serve_http());

        Self {
            inner,
            http_handle,
            runtime: std::mem::ManuallyDrop::new(runtime),
        }
    }

    /// Base URL of the in-process HTTP XRPC server (e.g.
    /// `http://127.0.0.1:54321`). Pass this to the `openlore` subprocess
    /// via `OPENLORE_PDS_ENDPOINT` so it talks to the fake.
    pub fn endpoint_url(&self) -> &str {
        &self.http_handle.base_url
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

impl Drop for FakePds {
    fn drop(&mut self) {
        // SAFETY: ManuallyDrop::take is sound because we only call it
        // here in the Drop impl and never again. Moving the runtime
        // onto a background thread lets the test thread proceed even
        // if a runtime worker is still parking on an accept() call.
        // Without this, tokio's blocking shutdown would deadlock on
        // the listening socket on some platforms (notably macOS).
        let rt = unsafe { std::mem::ManuallyDrop::take(&mut self.runtime) };
        let _ = std::thread::Builder::new()
            .name("fake-pds-shutdown".to_string())
            .spawn(move || drop(rt));
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
///
/// Shape mirrors the three-claims fixture's first entry (same
/// subject/predicate/object/evidence/confidence) so the
/// `subject=github:rust-lang/rust` canonical CID is reproducible
/// across single-claim and multi-claim scenarios. Author DID matches
/// `FakeIdentity::jeff()` so subprocess signing cross-verifies.
pub fn fixture_jeff_rust_memory_safety() -> UnsignedClaimFixture {
    UnsignedClaimFixture {
        subject: "github:rust-lang/rust".to_string(),
        predicate: "embodiesPhilosophy".to_string(),
        object: "org.openlore.philosophy.memory-safety".to_string(),
        evidence: vec!["https://www.rust-lang.org/".to_string()],
        confidence: 0.86,
        author_did: "did:plc:test-jeff#org.openlore.application".to_string(),
        composed_at: "2026-05-25T12:00:00Z".to_string(),
        references: Vec::new(),
    }
}

/// The Maria-on-Mastodon claim from US-001 Example 2 (confidence
/// boundary, 0.55, displayed as 'weighted').
pub fn fixture_maria_mastodon_federation_first() -> UnsignedClaimFixture {
    todo!("DELIVER: build the UnsignedClaimFixture matching US-001 Example 2")
}

/// Three claims about different subjects, used by FR-1 for the
/// federation round-trip.
///
/// Each fixture uses a distinct subject / predicate / object triple so
/// FR-1 cannot accidentally pass via aliasing (e.g. all three CIDs
/// collapsing onto one record because the canonicalised content is the
/// same).  The compose-time fields mirror data-models.md's on-disk
/// example shape verbatim (string subject, string predicate, string
/// object, one HTTPS evidence URL, finite-f64 confidence in [0,1]).
pub fn fixture_three_claims_different_predicates() -> Vec<UnsignedClaimFixture> {
    let author_did = "did:plc:test-jeff#org.openlore.application".to_string();
    let composed_at = "2026-05-25T12:00:00Z".to_string();

    vec![
        UnsignedClaimFixture {
            subject: "github:rust-lang/rust".to_string(),
            predicate: "embodiesPhilosophy".to_string(),
            object: "org.openlore.philosophy.memory-safety".to_string(),
            evidence: vec!["https://www.rust-lang.org/".to_string()],
            confidence: 0.86,
            author_did: author_did.clone(),
            composed_at: composed_at.clone(),
            references: Vec::new(),
        },
        UnsignedClaimFixture {
            subject: "github:torvalds/linux".to_string(),
            predicate: "embodiesPhilosophy".to_string(),
            object: "org.openlore.philosophy.unix-philosophy".to_string(),
            evidence: vec!["https://www.kernel.org/".to_string()],
            confidence: 0.92,
            author_did: author_did.clone(),
            composed_at: composed_at.clone(),
            references: Vec::new(),
        },
        UnsignedClaimFixture {
            subject: "github:mastodon/mastodon".to_string(),
            predicate: "embodiesPhilosophy".to_string(),
            object: "org.openlore.philosophy.federation-first".to_string(),
            evidence: vec!["https://joinmastodon.org/".to_string()],
            confidence: 0.78,
            author_did,
            composed_at,
            references: Vec::new(),
        },
    ]
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
pub fn run_openlore_with_stdin(env: &TestEnv, args: &[&str], stdin_lines: &str) -> CliOutcome {
    use std::io::Write;

    let bin = assert_cmd::cargo::cargo_bin("openlore");
    let mut cmd = Command::new(&bin);
    cmd.args(args)
        .env_clear()
        .env("OPENLORE_HOME", &env.home)
        .env("OPENLORE_DID", env.identity.author_did())
        .env("OPENLORE_KEY_SEED_HEX", &env.identity.seed_hex)
        // Step 05-08: point the in-binary `AtProtoPdsAdapter` at the
        // in-process FakePds HTTP server so the subprocess can publish
        // claims without leaving the test process. The fake's URL is
        // dynamic (random port) so it must be threaded explicitly.
        .env("OPENLORE_PDS_ENDPOINT", env.pds.endpoint_url())
        // PATH is required for libc / dynamic linker resolution on
        // some hosts; pass through the parent's PATH so `cargo bin`
        // can launch.
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

/// Run the `openlore` binary with the per-peer resolver endpoint wired so
/// the in-binary `IdentityPort::resolve_peer` resolves `peer_did` against
/// the supplied `PeerPds` base URL instead of the real PLC directory.
///
/// Mirrors [`run_openlore`] (clean env + the slice-01 stub seams) and adds
/// the `OPENLORE_PEER_PDS_ENDPOINT_<encoded_did>` env var the production
/// resolver reads. Promoted to shared support (step 03-02) so the
/// `peer_subscribe`, `peer_pull`, `counter_claim`, and `federated_query`
/// scaffolds reuse one slice-03 peer-resolver seam.
pub fn run_openlore_with_peer_resolver(
    env: &TestEnv,
    args: &[&str],
    peer_did: &str,
    peer_endpoint: &str,
) -> CliOutcome {
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

/// Like [`run_openlore_with_peer_resolver`] but feeds `stdin_lines` on
/// stdin so the two-prompt counter-claim flow (sign + publish) can be
/// confirmed. The peer-resolver seam lets the counter verb resolve the
/// target peer's DID for the `counters: <cid> (by <peer_did>)` preview
/// line. Used by CC-1: pass "\nY\n" to confirm Enter (sign) then Y (publish).
pub fn run_openlore_with_peer_resolver_stdin(
    env: &TestEnv,
    args: &[&str],
    peer_did: &str,
    peer_endpoint: &str,
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
        .env("OPENLORE_PDS_ENDPOINT", env.pds.endpoint_url())
        .env(peer_resolver_env_var(peer_did), peer_endpoint)
        .env("PATH", std::env::var("PATH").unwrap_or_default())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = cmd
        .spawn()
        .unwrap_or_else(|e| panic!("spawn openlore at {bin:?}: {e}"));
    if !stdin_lines.is_empty() {
        let stdin = child.stdin.as_mut().expect("stdin pipe");
        stdin
            .write_all(stdin_lines.as_bytes())
            .expect("write stdin");
    }
    drop(child.stdin.take());

    let output = child.wait_with_output().expect("wait_with_output");
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
pub fn peer_resolver_env_var(did: &str) -> String {
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

// -----------------------------------------------------------------------------
// Assertion helpers — universe-bound observable checks
// -----------------------------------------------------------------------------

/// Assert that the CLI invocation exited with status 0 and the given
/// substring appears in stdout. Failure prints the full stdout/stderr
/// for debuggability.
pub fn assert_exit_zero_and_stdout_contains(outcome: &CliOutcome, expected_substring: &str) {
    assert_eq!(
        outcome.status, 0,
        "expected exit 0; got {} \n--- stdout ---\n{}\n--- stderr ---\n{}",
        outcome.status, outcome.stdout, outcome.stderr
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
        expected_substring,
        outcome.stdout,
        outcome.stderr
    );
}

/// Universe-bound: "the compose preview contains the literal text
/// 'not as truth'". Asserts on `outcome.stdout`. Port-exposed name:
/// `cli.compose_preview.literal_not_as_truth_present`.
pub fn assert_compose_preview_contains_not_as_truth(outcome: &CliOutcome) {
    assert!(
        outcome.stdout.contains("not as truth"),
        "expected compose preview to contain literal text \"not as truth\" \
         (WD-6 hard AC); got stdout:\n--- stdout ---\n{}\n--- stderr ---\n{}",
        outcome.stdout,
        outcome.stderr
    );
}

/// Universe-bound: "no file was written under
/// `{home}/.local/share/openlore/claims/`". Port-exposed name:
/// `storage.local_claim_store.file_count`.
pub fn assert_no_local_claim_files_exist(env: &TestEnv) {
    let dir = env.claims_dir();
    if !dir.exists() {
        // Treat absence as zero files — that's the strongest possible
        // form of "no file written".
        return;
    }
    let entries: Vec<_> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("read claims_dir {}: {e}", dir.display()))
        .filter_map(|e| e.ok())
        .collect();
    assert!(
        entries.is_empty(),
        "expected no files under claims_dir {} but found {} entries: {:?}",
        dir.display(),
        entries.len(),
        entries.iter().map(|e| e.file_name()).collect::<Vec<_>>()
    );
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
    let records = env.pds.records();
    assert!(
        records.is_empty(),
        "expected no PDS create_record calls (KPI-5 local-first invariant); \
         got {} records: {:?}",
        records.len(),
        records
    );
}

/// Universe-bound: "the fake PDS contains a record at
/// `at://{author_did}/org.openlore.claim/<cid>`". Port-exposed name:
/// `pds.records.contains_at_uri`.
pub fn assert_pds_contains_record_at(env: &TestEnv, at_uri: &str) {
    let found = env.pds.record_at(at_uri);
    assert!(
        found.is_some(),
        "expected fake PDS to contain a record at {at_uri}; \
         actually present at-uris: {:?}",
        env.pds
            .records()
            .into_iter()
            .map(|r| r.at_uri)
            .collect::<Vec<_>>()
    );
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
///
/// The query renderer (WS-11 contract) prints every compose-time field
/// verbatim — subject, predicate, object, each evidence URL, confidence
/// as the original `f64` (NEVER a bucket label per WD-10 / D-12), the
/// author DID, and the claim CID.  This helper asserts each of those
/// values appears as a substring of stdout AND no banned bucket label
/// leaks through.  Mirrors the WS-11 byte-for-byte invariant for the
/// federation-round-trip scenarios.
pub fn assert_graph_query_output_matches_fixture(
    outcome: &CliOutcome,
    fixture: &UnsignedClaimFixture,
    expected_cid: &str,
) {
    assert_eq!(
        outcome.status, 0,
        "graph query must exit 0; got {} \n--- stdout ---\n{}\n--- stderr ---\n{}",
        outcome.status, outcome.stdout, outcome.stderr
    );

    // Author DID column carries only the bare DID (the `#fragment` is a
    // signing-key locator that need not surface in the read-path render).
    let bare_author_did = fixture
        .author_did
        .split('#')
        .next()
        .unwrap_or(&fixture.author_did);

    let mut required: Vec<String> = vec![
        fixture.subject.clone(),
        fixture.predicate.clone(),
        fixture.object.clone(),
        fixture.confidence.to_string(),
        bare_author_did.to_string(),
        expected_cid.to_string(),
    ];
    required.extend(fixture.evidence.iter().cloned());

    for needle in &required {
        assert!(
            outcome.stdout.contains(needle),
            "expected graph query stdout to contain {:?} \
             for fixture subject {:?} (KPI-4 round-trip identity); \
             \n--- stdout ---\n{}\n--- stderr ---\n{}",
            needle,
            fixture.subject,
            outcome.stdout,
            outcome.stderr
        );
    }

    // WD-10 / D-12: bucket labels are compose-time display only — they
    // must NEVER leak into the read-path render.
    for label in &["speculative", "weighted", "well-evidenced", "triangulated"] {
        assert!(
            !outcome.stdout.contains(label),
            "graph query stdout for {:?} must not contain bucket label {:?} \
             (WD-10 / D-12); \n--- stdout ---\n{}\n--- stderr ---\n{}",
            fixture.subject,
            label,
            outcome.stdout,
            outcome.stderr
        );
    }
}

/// Universe-bound: "the persisted JSON file does NOT contain the
/// substring 'speculative', 'weighted', 'well-evidenced', or
/// 'triangulated' in the confidence-bearing area". Port-exposed name:
/// `storage.local_claim_store.no_bucket_label_string`.
pub fn assert_persisted_payload_has_no_bucket_label(env: &TestEnv, cid: &str) {
    todo!(
        "DELIVER: read claims_dir/<cid>.json; assert none of the four bucket-label strings appear"
    )
}

/// Universe-bound: "the DuckDB row for the given CID has `published_at`
/// non-null AND its `at_uri` equals
/// `at://{author_did}/org.openlore.claim/<cid>`". Port-exposed name:
/// `storage.duckdb.publication_metadata_consistent`.
///
/// Opens a raw `duckdb::Connection` for the assertion (rather than going
/// through `DuckDbStorageAdapter`) because slice-01's StoragePort does
/// not expose a `read_publication_metadata` arm — that surface arrives
/// with the graph-query verb later in phase 05. Test-support code is
/// the only place a raw-SQL query is acceptable; production code MUST
/// go through StoragePort.
pub fn assert_duckdb_publication_metadata_for_cid(env: &TestEnv, cid: &str, expected_at_uri: &str) {
    let db_path = env.duckdb_path();
    assert!(
        db_path.exists(),
        "expected DuckDB to exist at {} after publish; file missing",
        db_path.display()
    );

    let conn = duckdb::Connection::open(&db_path).unwrap_or_else(|err| {
        panic!(
            "open DuckDB at {} for publication-metadata assertion: {err}",
            db_path.display()
        )
    });

    let row: Option<(Option<chrono::DateTime<chrono::Utc>>, Option<String>)> = conn
        .query_row(
            "SELECT published_at, at_uri FROM claims WHERE cid = ?",
            duckdb::params![cid],
            |r| Ok((r.get::<_, Option<_>>(0)?, r.get::<_, Option<_>>(1)?)),
        )
        .ok();

    let (published_at, at_uri) =
        row.unwrap_or_else(|| panic!("no claim row in DuckDB for cid {cid}"));
    assert!(
        published_at.is_some(),
        "expected published_at to be non-null for cid {cid} after publish; got NULL"
    );
    assert_eq!(
        at_uri.as_deref(),
        Some(expected_at_uri),
        "expected at_uri column to equal {expected_at_uri} for cid {cid}; got {at_uri:?}"
    );
}

/// Universe-bound: "the retraction's `references` field includes
/// `{type: \"retracts\", cid: <original_cid>}`". Port-exposed name:
/// `claim.references.contains_retracts_target`.
///
/// Reads the on-disk `<retract_cid>.json` artefact under the test
/// env's claims_dir, deserialises into the canonical
/// `claim_domain::SignedClaim`, and asserts the `references[]` array
/// contains at least one entry with `ref_type == Retracts` AND
/// `cid == original_cid`. Reading through the domain type (rather
/// than ad-hoc JSON-poking) pins the contract to whatever serde shape
/// `SignedClaim` actually serializes — refactoring stays GREEN.
pub fn assert_claim_references_retract(env: &TestEnv, retract_cid: &str, original_cid: &str) {
    let artifact_path = env.claims_dir().join(format!("{retract_cid}.json"));
    let json_bytes = std::fs::read(&artifact_path).unwrap_or_else(|e| {
        panic!(
            "expected retraction claim file at {}; got {e}",
            artifact_path.display()
        )
    });
    let signed: claim_domain::SignedClaim =
        serde_json::from_slice(&json_bytes).unwrap_or_else(|e| {
            panic!(
                "could not deserialize retraction claim at {}: {e}\n--- file ---\n{}",
                artifact_path.display(),
                String::from_utf8_lossy(&json_bytes)
            )
        });

    let has_retracts_pointer = signed.unsigned.references.iter().any(|r| {
        matches!(r.ref_type, claim_domain::ReferenceType::Retracts) && r.cid.0 == original_cid
    });
    assert!(
        has_retracts_pointer,
        "expected retraction claim at {} to contain references[] entry with \
         {{type=Retracts, cid={original_cid}}}; actual references={:?}",
        artifact_path.display(),
        signed.unsigned.references,
    );
}

// =============================================================================
// Slice-03 — peer-PDS double + peer-claim assertion helpers (step 01-05)
// =============================================================================
//
// Symmetric with the `FakePds` wrapper above. The four subprocess slice-03
// acceptance files (`peer_subscribe`, `peer_pull`, `counter_claim`,
// `federated_query`) construct a `PeerPds` per subscribed peer, point the
// `openlore` subprocess at its endpoint, and assert on the observable
// post-pull surface via the helpers below.
//
// Per DD-FED-10, the helpers whose load-bearing body materializes per
// scenario in DELIVER phases 03-05 carry a `todo!()` body with a precise
// contract docstring; the SIGNATURES are correct NOW so every test file
// compiles and reaches its own `todo!()` (RED, not BROKEN). The helpers
// that are cheap + deterministic (DID→filesystem encoding, directory
// removal, output substring scan) are implemented in full now because the
// peer_pull / peer_subscribe / federated_query scaffolds reference them
// directly and they have no dependency on unimplemented production code.

/// A test double for a PEER's `adapter-atproto-pds` (read-only).
///
/// Mirrors [`FakePds`] (the user's-own-PDS wrapper): owns a multi-threaded
/// tokio runtime + an in-process read-only HTTP XRPC server bound to a
/// random `127.0.0.1` port. The server hosts the peer's record set AND a
/// `com.atproto.identity.resolveDid` handler on the SAME base URL (one
/// server per peer keeps wiring simple — open-question #2). Dropping
/// `PeerPds` releases the runtime on a background thread (the same
/// `ManuallyDrop` shutdown pattern `FakePds` uses) so the test thread never
/// blocks on a parked accept().
pub struct PeerPds {
    inner: SharedFakePeerPds,
    http_handle: FakePeerPdsHttpHandle,
    runtime: std::mem::ManuallyDrop<tokio::runtime::Runtime>,
}

impl PeerPds {
    /// Start a peer PDS hosting `records` for `peer_did` and spin up its
    /// in-process HTTP server (records + resolveDid on one base URL).
    pub fn for_peer(peer_did: &str, records: Vec<FakePeerRecord>) -> Self {
        Self::from_fake(SharedFakePeerPds::for_peer(peer_did, records))
    }

    /// Start a peer PDS preconfigured with the tampered-signature posture
    /// (KPI-FED-6). Pair with [`fixture_adversarial_peer_tampered_signature`].
    pub fn with_tampered_signature(peer_did: &str, honest: Vec<FakePeerRecord>) -> Self {
        Self::from_fake(SharedFakePeerPds::with_tampered_signature(peer_did, honest))
    }

    /// Start a peer PDS preconfigured with the CID-mismatch posture.
    /// Pair with [`fixture_adversarial_peer_cid_mismatch`].
    pub fn with_cid_mismatch(peer_did: &str, honest: Vec<FakePeerRecord>) -> Self {
        Self::from_fake(SharedFakePeerPds::with_cid_mismatch(peer_did, honest))
    }

    /// Start a peer PDS preconfigured with the self-attribution posture
    /// (WD-40). Pair with [`fixture_adversarial_peer_self_attribution`].
    pub fn with_self_attribution(
        peer_did: &str,
        victim_did: &str,
        honest: Vec<FakePeerRecord>,
    ) -> Self {
        Self::from_fake(SharedFakePeerPds::with_self_attribution(
            peer_did, victim_did, honest,
        ))
    }

    /// Start a peer PDS preconfigured with the cross-attribution posture
    /// (WD-41). Pair with [`fixture_adversarial_peer_cross_attribution`].
    pub fn with_cross_attribution(
        peer_did: &str,
        claimed_author_did: &str,
        honest: Vec<FakePeerRecord>,
    ) -> Self {
        Self::from_fake(SharedFakePeerPds::with_cross_attribution(
            peer_did,
            claimed_author_did,
            honest,
        ))
    }

    fn from_fake(inner: SharedFakePeerPds) -> Self {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_io()
            .enable_time()
            .thread_name("fake-peer-pds-rt")
            .build()
            .expect("PeerPds: build tokio multi_thread runtime");
        let http_handle = runtime.block_on(inner.serve_http());
        Self {
            inner,
            http_handle,
            runtime: std::mem::ManuallyDrop::new(runtime),
        }
    }

    /// Base URL of the peer's in-process HTTP XRPC server (records +
    /// resolveDid). Thread into the `openlore` subprocess via the per-peer
    /// endpoint env var so the in-binary peer adapter talks to this fake.
    pub fn endpoint_url(&self) -> &str {
        &self.http_handle.base_url
    }

    /// The peer DID this fake hosts records for.
    pub fn peer_did(&self) -> &str {
        self.inner.peer_did()
    }

    /// All records the fake would return on a `listRecords` call (honest +
    /// any one adversarial record). Cross-check the production code stored
    /// only the verified subset.
    pub fn records(&self) -> Vec<FakePeerRecord> {
        self.inner.records()
    }

    /// Engage the "unreachable" failure mode (PP-7): the HTTP server drops
    /// connections without responding.
    pub fn simulate_unreachable(&self) {
        self.inner.simulate_unreachable();
    }

    /// Inverse of [`simulate_unreachable`](Self::simulate_unreachable).
    pub fn restore(&self) {
        self.inner.restore();
    }
}

impl Drop for PeerPds {
    fn drop(&mut self) {
        // Same background-thread shutdown as `FakePds` — moving the runtime
        // off the test thread avoids blocking on a parked accept() during
        // teardown on macOS.
        let rt = unsafe { std::mem::ManuallyDrop::take(&mut self.runtime) };
        let _ = std::thread::Builder::new()
            .name("fake-peer-pds-shutdown".to_string())
            .spawn(move || drop(rt));
    }
}

/// Encode a DID into the filesystem-safe partition segment used under
/// `peer_claims/<encoded_did>/` (Q-DELIVER-2): colons become underscores.
///
/// `did:plc:rachel-test` → `did_plc_rachel-test`. Round-trippable + safe on
/// macOS APFS, Linux ext4, and WSL2 DrvFs (per data-models.md §"On-disk
/// artifact format"). This is the single source of truth the assertion
/// helpers and DELIVER's production adapter must agree on.
pub fn did_to_fs_segment(did: &str) -> String {
    did.replace(':', "_")
}

/// Absolute path to a peer's on-disk claim partition:
/// `{home}/.local/share/openlore/peer_claims/<encoded_did>/`.
pub fn peer_claims_dir_for(env: &TestEnv, peer_did: &str) -> PathBuf {
    env.home
        .join(".local")
        .join("share")
        .join("openlore")
        .join("peer_claims")
        .join(did_to_fs_segment(peer_did))
}

/// Universe-bound: "the `peer_claims` store holds exactly `count` rows
/// attributed to `peer_did`, and EVERY such row carries `peer_did` as its
/// author — never any other DID (anti-merging, I-FED-1)". Port-exposed
/// name: `peer_storage.claims.row_count_by_author[did]`.
///
/// DD-FED-10: materialized for PP-1 (step 04-01). Opens a raw
/// `duckdb::Connection` to `env.duckdb_path()`, asserts
/// `SELECT count(*) FROM peer_claims WHERE author_did = peer_did == count`
/// AND that the TOTAL `peer_claims` row count equals `count` too — so no
/// stored CID leaked under any OTHER author_did (anti-merging, I-FED-1).
/// Test-support is the only place raw SQL is acceptable; production goes
/// through `PeerStoragePort`.
pub fn assert_peer_claims_attributed_to(env: &TestEnv, peer_did: &str, count: usize) {
    let db_path = env.duckdb_path();
    let conn = duckdb::Connection::open(&db_path).unwrap_or_else(|err| {
        panic!(
            "open DuckDB at {} for peer_claims attribution assertion: {err}",
            db_path.display()
        )
    });

    let attributed: i64 = conn
        .query_row(
            "SELECT count(*) FROM peer_claims WHERE author_did = ?",
            duckdb::params![peer_did],
            |r| r.get(0),
        )
        .unwrap_or_else(|err| panic!("query peer_claims for {peer_did}: {err}"));
    assert_eq!(
        attributed as usize, count,
        "expected exactly {count} peer_claims rows attributed to {peer_did}; got {attributed}"
    );

    // Anti-merging (I-FED-1): the TOTAL row count must equal the
    // attributed count — no stored CID may appear under any OTHER DID.
    let total: i64 = conn
        .query_row("SELECT count(*) FROM peer_claims", [], |r| r.get(0))
        .unwrap_or_else(|err| panic!("query total peer_claims: {err}"));
    assert_eq!(
        total as usize, count,
        "anti-merging (I-FED-1): expected the TOTAL peer_claims row count to equal \
         {count} (every row attributed to {peer_did}); got {total} total rows — \
         a CID leaked under a DIFFERENT author_did"
    );
}

/// Universe-bound: "the `peer_claims` store holds ZERO rows attributed to
/// `did`". Port-exposed name: `peer_storage.claims.row_count_by_author[did]`.
///
/// DD-FED-10 (WD-41 anti-back-door, PP-6): the cross-attributed third party
/// must have ZERO rows — subscribing to Rachel never silently follows a third
/// party Rachel cross-publishes for. Unlike [`assert_peer_claims_attributed_to`]
/// this asserts ONLY the per-author count (NOT the total), so it composes with
/// the honest rows attributed to the subscribed peer. Test-support is the only
/// place raw SQL is acceptable; production goes through `PeerStoragePort`.
pub fn assert_no_peer_claims_attributed_to(env: &TestEnv, did: &str) {
    let db_path = env.duckdb_path();
    let conn = duckdb::Connection::open(&db_path).unwrap_or_else(|err| {
        panic!(
            "open DuckDB at {} for zero-attribution assertion: {err}",
            db_path.display()
        )
    });
    let attributed: i64 = conn
        .query_row(
            "SELECT count(*) FROM peer_claims WHERE author_did = ?",
            duckdb::params![did],
            |r| r.get(0),
        )
        .unwrap_or_else(|err| panic!("query peer_claims for {did}: {err}"));
    assert_eq!(
        attributed, 0,
        "anti-back-door (WD-41): the third party {did} must have ZERO peer_claims \
         rows — subscribing to a peer never silently follows a cross-attributed \
         third party; got {attributed}"
    );
}

/// Build a verifiable peer record set for the PP-1 happy path.
///
/// The slice-03 peer fixtures (`fixture_other_developer_three_claims`)
/// carry PLACEHOLDER signatures + PLACEHOLDER rkeys — per the
/// `fixtures_peer.rs` docstring, "the real Ed25519 bytes are materialized
/// per-scenario in DELIVER." This helper does that materialization for the
/// honest happy path: it deterministically signs each `(subject, object,
/// confidence)` triple with `peer_seed` and re-keys each record so
/// `rkey == compute_cid(canonical(unsigned))`, mirroring exactly what the
/// production pull pipeline recomputes + verifies.
///
/// Returns `(records, peer_pubkey_hex)`:
///   - `records`: the verifiable wire records to host on `PeerPds::for_peer`.
///   - `peer_pubkey_hex`: the peer's Ed25519 public key as 64-char lowercase
///     hex, wired into the subprocess via [`peer_pubkey_env_var`] so the
///     in-binary resolver surfaces the REAL key for `claim_domain::verify`.
///
/// Test-support is the only place this construction is acceptable; the
/// production populate path is `peer pull` itself.
pub fn build_verifiable_peer_records(
    peer_did: &str,
    peer_seed: [u8; 32],
) -> (Vec<FakePeerRecord>, String) {
    use claim_domain::{canonicalize, compute_cid, sign, SigningKey, VerifyingKey};
    use ed25519_dalek::SigningKey as DalekSigningKey;

    let dalek_sk = DalekSigningKey::from_bytes(&peer_seed);
    let dalek_vk = dalek_sk.verifying_key();
    let signing_key = SigningKey(dalek_sk.to_bytes().to_vec());
    let pubkey_hex = hex_lower(&VerifyingKey(dalek_vk.to_bytes().to_vec()).0);

    // Three distinct (subject shared, object distinct) honest claims —
    // mirrors `fixture_other_developer_three_claims` but with REAL crypto.
    let triples = [
        (
            "github:rust-lang/cargo",
            "org.openlore.philosophy.dependency-pinning",
            0.42,
        ),
        (
            "github:rust-lang/cargo",
            "org.openlore.philosophy.reproducible-builds",
            0.71,
        ),
        (
            "github:rust-lang/cargo",
            "org.openlore.philosophy.workspace-cohesion",
            0.88,
        ),
    ];

    let records = triples
        .iter()
        .map(|(subject, object, confidence)| {
            // Build the unsigned domain claim (Confidence is crate-private,
            // so route through serde — the same trick test-support uses).
            let confidence_wrapper: claim_domain::Confidence =
                serde_json::from_value(serde_json::json!(confidence))
                    .expect("confidence value is well-formed");
            let unsigned = claim_domain::UnsignedClaim {
                subject: (*subject).to_string(),
                predicate: "embodiesPhilosophy".to_string(),
                object: (*object).to_string(),
                evidence: vec!["https://github.com/rust-lang/cargo".to_string()],
                confidence: confidence_wrapper,
                author_did: claim_domain::Did(format!("{peer_did}#org.openlore.application")),
                composed_at: "2026-05-22T09:18:44Z".to_string(),
                references: Vec::new(),
                reason: None,
            };

            let canonical = canonicalize(&unsigned).expect("canonicalize honest claim");
            let cid = compute_cid(&canonical);
            let signature = sign(&cid, &signing_key).expect("sign honest claim");
            let sig_b64 = base64url_no_pad(&signature.signature_bytes);

            // The peer wire shape (lexicon JSON) — what the peer PDS hosts.
            // rkey == the real CID so the pull pipeline's recompute matches.
            let body = serde_json::json!({
                "subject": subject,
                "predicate": "embodiesPhilosophy",
                "object": object,
                "evidence": ["https://github.com/rust-lang/cargo"],
                "confidence": confidence,
                "author": format!("{peer_did}#org.openlore.application"),
                "composedAt": "2026-05-22T09:18:44Z",
                "references": [],
                "signature": {
                    "kid": format!("{peer_did}#org.openlore.application"),
                    "alg": "EdDSA",
                    "sig": sig_b64,
                }
            });
            FakePeerRecord::claim(cid.0, body)
        })
        .collect();

    (records, pubkey_hex)
}

/// Build a verifiable peer record set for a SPECIFIC list of `objects`.
///
/// The multi-peer sibling of [`build_verifiable_peer_records`] (which hardcodes
/// Rachel's three canonical triples). FQ-8's multi-author release gate needs a
/// SECOND distinct peer (Tobias) hosting a DIFFERENT number of records (2), so
/// this helper materializes the same REAL Ed25519 crypto + CID-recompute the
/// pull pipeline verifies, but over a caller-supplied object list. Each object
/// MUST be distinct from the others (so CIDs cannot alias within the peer); a
/// distinct `peer_did` already differentiates CIDs ACROSS peers because
/// `author_did` is part of the canonicalized claim.
///
/// Confidence is derived deterministically per index (`0.30 + i * 0.07`) so the
/// values stay in `[0,1]` and differ per row — keeping the fixture's rows
/// independently identifiable without the caller threading confidences too.
///
/// Returns `(records, peer_pubkey_hex)` exactly like
/// [`build_verifiable_peer_records`]. Test-support is the only place this
/// construction is acceptable; the production populate path is `peer pull`.
pub fn build_verifiable_peer_records_with_objects(
    peer_did: &str,
    peer_seed: [u8; 32],
    objects: &[&str],
) -> (Vec<FakePeerRecord>, String) {
    use claim_domain::{canonicalize, compute_cid, sign, SigningKey, VerifyingKey};
    use ed25519_dalek::SigningKey as DalekSigningKey;

    let dalek_sk = DalekSigningKey::from_bytes(&peer_seed);
    let dalek_vk = dalek_sk.verifying_key();
    let signing_key = SigningKey(dalek_sk.to_bytes().to_vec());
    let pubkey_hex = hex_lower(&VerifyingKey(dalek_vk.to_bytes().to_vec()).0);

    let records = objects
        .iter()
        .enumerate()
        .map(|(i, object)| {
            let confidence = 0.30 + (i as f64) * 0.07;
            let confidence_wrapper: claim_domain::Confidence =
                serde_json::from_value(serde_json::json!(confidence))
                    .expect("confidence value is well-formed");
            let unsigned = claim_domain::UnsignedClaim {
                subject: "github:rust-lang/cargo".to_string(),
                predicate: "embodiesPhilosophy".to_string(),
                object: (*object).to_string(),
                evidence: vec!["https://github.com/rust-lang/cargo".to_string()],
                confidence: confidence_wrapper,
                author_did: claim_domain::Did(format!("{peer_did}#org.openlore.application")),
                composed_at: "2026-05-22T09:18:44Z".to_string(),
                references: Vec::new(),
                reason: None,
            };

            let canonical = canonicalize(&unsigned).expect("canonicalize peer claim");
            let cid = compute_cid(&canonical);
            let signature = sign(&cid, &signing_key).expect("sign peer claim");
            let sig_b64 = base64url_no_pad(&signature.signature_bytes);

            let body = serde_json::json!({
                "subject": "github:rust-lang/cargo",
                "predicate": "embodiesPhilosophy",
                "object": object,
                "evidence": ["https://github.com/rust-lang/cargo"],
                "confidence": confidence,
                "author": format!("{peer_did}#org.openlore.application"),
                "composedAt": "2026-05-22T09:18:44Z",
                "references": [],
                "signature": {
                    "kid": format!("{peer_did}#org.openlore.application"),
                    "alg": "EdDSA",
                    "sig": sig_b64,
                }
            });
            FakePeerRecord::claim(cid.0, body)
        })
        .collect();

    (records, pubkey_hex)
}

/// Build a tampered-signature peer record set for the PP-3 sad path
/// (KPI-FED-6).
///
/// Like [`build_verifiable_peer_records`] this materializes REAL crypto so
/// the honest records pass the pull pipeline's verify + CID round-trip. It
/// returns `honest_count` genuinely-signed records PLUS one record whose
/// `rkey == compute_cid(canonical(body))` (so it PASSES the CID round-trip,
/// WD-24) but whose `signature.sig` last byte is flipped (so
/// `claim_domain::verify` REJECTS it). This is the isolating fixture that
/// drives the SIGNATURE-rejection branch specifically — distinct from the
/// `with_cid_mismatch` posture which trips the earlier CID-round-trip gate.
///
/// Returns `(records, peer_pubkey_hex, tampered_rkey)`:
///   - `records`: `honest_count` honest + 1 tampered = `honest_count + 1`
///     wire records to host on `PeerPds::for_peer`.
///   - `peer_pubkey_hex`: the peer's real Ed25519 pubkey hex for the verify
///     seam (same key signs the honest records; the tampered record's sig is
///     a corrupted signature OVER THE SAME key, so it fails to verify).
///   - `tampered_rkey`: the CID/rkey of the tampered record so the caller can
///     assert it was NEVER stored (DD-FED-10 anti-merging at the reject path).
///
/// Test-support is the only place this construction is acceptable; the
/// production populate path is `peer pull` itself.
pub fn build_tampered_signature_peer_records(
    peer_did: &str,
    peer_seed: [u8; 32],
    honest_count: usize,
) -> (Vec<FakePeerRecord>, String, String) {
    use claim_domain::{canonicalize, compute_cid, sign, SigningKey, VerifyingKey};
    use ed25519_dalek::SigningKey as DalekSigningKey;

    let dalek_sk = DalekSigningKey::from_bytes(&peer_seed);
    let dalek_vk = dalek_sk.verifying_key();
    let signing_key = SigningKey(dalek_sk.to_bytes().to_vec());
    let pubkey_hex = hex_lower(&VerifyingKey(dalek_vk.to_bytes().to_vec()).0);

    // One honest claim builder: a distinct object per index so CIDs cannot
    // alias. Returns the wire record AND its CID (so the tampered record can
    // reuse the construction and keep its rkey CID-consistent).
    let build = |object: &str, confidence: f64, flip_sig: bool| -> (FakePeerRecord, String) {
        let confidence_wrapper: claim_domain::Confidence =
            serde_json::from_value(serde_json::json!(confidence))
                .expect("confidence value is well-formed");
        let unsigned = claim_domain::UnsignedClaim {
            subject: "github:rust-lang/cargo".to_string(),
            predicate: "embodiesPhilosophy".to_string(),
            object: object.to_string(),
            evidence: vec!["https://github.com/rust-lang/cargo".to_string()],
            confidence: confidence_wrapper,
            author_did: claim_domain::Did(format!("{peer_did}#org.openlore.application")),
            composed_at: "2026-05-22T09:18:44Z".to_string(),
            references: Vec::new(),
            reason: None,
        };

        let canonical = canonicalize(&unsigned).expect("canonicalize claim");
        let cid = compute_cid(&canonical);
        let signature = sign(&cid, &signing_key).expect("sign claim");
        let mut sig_bytes = signature.signature_bytes.clone();
        if flip_sig {
            // Flip the LAST signature byte AFTER the nominal sign step — the
            // CID/rkey stays valid, only the Ed25519 signature no longer
            // verifies (the tampered-signature posture, KPI-FED-6).
            if let Some(last) = sig_bytes.last_mut() {
                *last ^= 0x01;
            }
        }
        let sig_b64 = base64url_no_pad(&sig_bytes);

        let body = serde_json::json!({
            "subject": "github:rust-lang/cargo",
            "predicate": "embodiesPhilosophy",
            "object": object,
            "evidence": ["https://github.com/rust-lang/cargo"],
            "confidence": confidence,
            "author": format!("{peer_did}#org.openlore.application"),
            "composedAt": "2026-05-22T09:18:44Z",
            "references": [],
            "signature": {
                "kid": format!("{peer_did}#org.openlore.application"),
                "alg": "EdDSA",
                "sig": sig_b64,
            }
        });
        (FakePeerRecord::claim(cid.0.clone(), body), cid.0)
    };

    let mut records: Vec<FakePeerRecord> = (0..honest_count)
        .map(|i| {
            let (record, _cid) = build(
                &format!("org.openlore.philosophy.honest-claim-{i}"),
                0.50 + (i as f64) * 0.05,
                false,
            );
            record
        })
        .collect();

    // The tampered record: CID-consistent rkey, corrupted signature.
    let (tampered_record, tampered_rkey) =
        build("org.openlore.philosophy.tampered-claim", 0.42, true);
    records.push(tampered_record);

    (records, pubkey_hex, tampered_rkey)
}

/// Build a verifiable cross-attribution peer record set for the PP-6 sad
/// path (WD-41) that exercises the WRITE-TIME `CrossAttribution` guard.
///
/// Like [`build_verifiable_peer_records`] this materializes REAL crypto so
/// EVERY record — honest AND the cross-attributed one — passes the pull
/// pipeline's pure layer-1 pre-check (`evaluate_record`: CID round-trip +
/// signature verify against the SUBSCRIBED peer's key). The cross-attributed
/// record is therefore NOT caught by layer 1; it reaches
/// `PeerStoragePort::write_peer_claim`, which rejects it with
/// `PeerStorageError::CrossAttribution` because its `author` field names
/// `third_party_did` rather than the subscribed `peer_did` (WD-41).
///
/// This is the realistic adversarial vector WD-41 describes: the SUBSCRIBED
/// peer's PDS serves a record signed by the peer's OWN key (so it verifies)
/// but whose `author` field references a DIFFERENT DID. Storing it would be
/// the "follow Rachel → auto-follow whoever Rachel cross-publishes for"
/// back-door the trust model forbids.
///
/// Returns `(records, peer_pubkey_hex, cross_rkey)`:
///   - `records`: three honest peer-attributed claims + one cross-attributed
///     claim = four wire records to host on `PeerPds::for_peer`.
///   - `peer_pubkey_hex`: the subscribed peer's Ed25519 pubkey hex for the
///     verify seam (the SAME key signs every record — the cross-attributed
///     record's signature verifies, isolating the WRITE-TIME guard as the
///     ONLY thing that can reject it).
///   - `cross_rkey`: the CID/rkey of the cross-attributed record so the
///     caller can assert it was NEVER stored (DD-FED-10 anti-merging at the
///     reject path — zero rows under ANY author, including the third party).
///
/// Test-support is the only place this construction is acceptable; the
/// production populate path is `peer pull` itself.
pub fn build_verifiable_cross_attribution_peer_records(
    peer_did: &str,
    third_party_did: &str,
    peer_seed: [u8; 32],
) -> (Vec<FakePeerRecord>, String, String) {
    use claim_domain::{canonicalize, compute_cid, sign, SigningKey, VerifyingKey};
    use ed25519_dalek::SigningKey as DalekSigningKey;

    let dalek_sk = DalekSigningKey::from_bytes(&peer_seed);
    let dalek_vk = dalek_sk.verifying_key();
    let signing_key = SigningKey(dalek_sk.to_bytes().to_vec());
    let pubkey_hex = hex_lower(&VerifyingKey(dalek_vk.to_bytes().to_vec()).0);

    // One verifiable record builder: signs `author`'s body with the
    // SUBSCRIBED peer's key (so it always verifies) and re-keys it so
    // `rkey == compute_cid(canonical(unsigned))` (so it always passes the
    // CID round-trip). The `author` field is what varies — honest records
    // name the peer, the cross-attributed record names the third party.
    let build = |author_did: &str, object: &str, confidence: f64| -> FakePeerRecord {
        let confidence_wrapper: claim_domain::Confidence =
            serde_json::from_value(serde_json::json!(confidence))
                .expect("confidence value is well-formed");
        let unsigned = claim_domain::UnsignedClaim {
            subject: "github:rust-lang/cargo".to_string(),
            predicate: "embodiesPhilosophy".to_string(),
            object: object.to_string(),
            evidence: vec!["https://github.com/rust-lang/cargo".to_string()],
            confidence: confidence_wrapper,
            author_did: claim_domain::Did(format!("{author_did}#org.openlore.application")),
            composed_at: "2026-05-22T09:18:44Z".to_string(),
            references: Vec::new(),
            reason: None,
        };

        let canonical = canonicalize(&unsigned).expect("canonicalize claim");
        let cid = compute_cid(&canonical);
        // Always signed by the SUBSCRIBED peer's key — even the
        // cross-attributed record. This is the WD-41 vector: a peer-signed
        // record whose `author` names someone else. The signature verifies;
        // only the write-time author-vs-peer guard catches it.
        let signature = sign(&cid, &signing_key).expect("sign claim");
        let sig_b64 = base64url_no_pad(&signature.signature_bytes);

        let body = serde_json::json!({
            "subject": "github:rust-lang/cargo",
            "predicate": "embodiesPhilosophy",
            "object": object,
            "evidence": ["https://github.com/rust-lang/cargo"],
            "confidence": confidence,
            "author": format!("{author_did}#org.openlore.application"),
            "composedAt": "2026-05-22T09:18:44Z",
            "references": [],
            "signature": {
                "kid": format!("{peer_did}#org.openlore.application"),
                "alg": "EdDSA",
                "sig": sig_b64,
            }
        });
        FakePeerRecord::claim(cid.0, body)
    };

    // Three honest peer-attributed claims (distinct objects so CIDs cannot
    // alias) + one cross-attributed claim authored by the third party.
    let mut records = vec![
        build(peer_did, "org.openlore.philosophy.dependency-pinning", 0.42),
        build(
            peer_did,
            "org.openlore.philosophy.reproducible-builds",
            0.71,
        ),
        build(peer_did, "org.openlore.philosophy.workspace-cohesion", 0.88),
    ];

    let cross_record = build(
        third_party_did,
        "org.openlore.philosophy.cross-attributed-claim",
        0.55,
    );
    let cross_rkey = cross_record.rkey.clone();
    records.push(cross_record);

    (records, pubkey_hex, cross_rkey)
}

/// Universe-bound: "the `peer_claims` store holds NO row for `cid`, under
/// ANY author_did, AND no on-disk `<cid>.json` artifact exists for it".
/// Port-exposed name:
/// `peer_storage.claims.row_count_for_cid[cid] == 0 && filesystem.artifact_for_cid.absent`.
///
/// DD-FED-10 (PP-3 reject path): the rejected record's CID must be absent
/// from `peer_claims` (anti-merging holds even on the reject path — no
/// leak under any author) AND must have no artifact file in any peer
/// partition. Opens a raw `duckdb::Connection` (test-support is the only
/// place raw SQL is acceptable) and walks the on-disk `peer_claims/` tree.
pub fn assert_peer_claim_cid_absent(env: &TestEnv, cid: &str) {
    let db_path = env.duckdb_path();
    let conn = duckdb::Connection::open(&db_path).unwrap_or_else(|err| {
        panic!(
            "open DuckDB at {} for cid-absence assertion: {err}",
            db_path.display()
        )
    });
    let rows: i64 = conn
        .query_row(
            "SELECT count(*) FROM peer_claims WHERE cid = ?",
            duckdb::params![cid],
            |r| r.get(0),
        )
        .unwrap_or_else(|err| panic!("query peer_claims for cid {cid}: {err}"));
    assert_eq!(
        rows, 0,
        "anti-merging at the reject path: the rejected CID {cid} must have \
         ZERO peer_claims rows under ANY author_did; got {rows}"
    );

    // No artifact file for the rejected CID under any peer partition.
    let peer_claims_root = env
        .home
        .join(".local")
        .join("share")
        .join("openlore")
        .join("peer_claims");
    if peer_claims_root.exists() {
        let artifact_name = format!("{cid}.json");
        for partition in std::fs::read_dir(&peer_claims_root)
            .unwrap_or_else(|e| panic!("read peer_claims root {}: {e}", peer_claims_root.display()))
            .filter_map(|e| e.ok())
        {
            let candidate = partition.path().join(&artifact_name);
            assert!(
                !candidate.exists(),
                "the rejected CID {cid} must have NO on-disk artifact; found {}",
                candidate.display()
            );
        }
    }
}

/// The per-peer pubkey env-var NAME for a DID, carrying the peer's Ed25519
/// public key as 64-char lowercase hex. The in-binary peer resolver reads
/// this so `claim_domain::verify` has the REAL key (the `FakePeerPds`
/// resolveDid DID-document only carries a placeholder multibase string).
/// Same encoding rule as [`peer_resolver_env_var`].
///
/// `did:plc:rachel-test` → `OPENLORE_PEER_PUBKEY_HEX_DID_PLC_RACHEL_TEST`.
pub fn peer_pubkey_env_var(did: &str) -> String {
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
    format!("OPENLORE_PEER_PUBKEY_HEX_{encoded}")
}

/// Run `openlore <args>` with BOTH the per-peer resolver endpoint AND the
/// per-peer pubkey hex wired into the subprocess. Used by `peer pull`
/// (PP-1): the resolver finds the fake PDS via the endpoint seam, and the
/// pull pipeline verifies each record against `peer_pubkey_hex` via the
/// pubkey seam. Mirrors [`run_openlore_with_peer_resolver`].
pub fn run_openlore_pull(
    env: &TestEnv,
    args: &[&str],
    peer_did: &str,
    peer_endpoint: &str,
    peer_pubkey_hex: &str,
) -> CliOutcome {
    let bin = assert_cmd::cargo::cargo_bin("openlore");
    let output = Command::new(&bin)
        .args(args)
        .env_clear()
        .env("OPENLORE_HOME", &env.home)
        .env("OPENLORE_DID", env.identity.author_did())
        .env("OPENLORE_KEY_SEED_HEX", &env.identity.seed_hex)
        .env("OPENLORE_PDS_ENDPOINT", env.pds.endpoint_url())
        .env(peer_resolver_env_var(peer_did), peer_endpoint)
        .env(peer_pubkey_env_var(peer_did), peer_pubkey_hex)
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

/// One peer's resolver + pubkey seam wiring for a multi-peer pull.
///
/// PP-7 fault isolation needs MORE than one subscribed peer in a single
/// `peer pull` invocation, so the per-peer env-var seams ([`peer_resolver_env_var`]
/// + [`peer_pubkey_env_var`]) must be threaded for EACH peer at once.
/// [`run_openlore_pull`] only wires a single peer; this struct is the
/// per-peer tuple [`run_openlore_pull_multi`] consumes.
pub struct PeerSeam<'a> {
    /// The peer DID (e.g. `did:plc:rachel-test`).
    pub peer_did: &'a str,
    /// The peer's in-process `PeerPds` HTTP base URL.
    pub peer_endpoint: &'a str,
    /// The peer's Ed25519 public key as 64-char lowercase hex (the verify
    /// seam). For an unreachable peer this is still wired (resolution +
    /// listRecords drop the connection before the key is ever used).
    pub peer_pubkey_hex: &'a str,
}

/// Run `openlore <args>` with the resolver + pubkey seams wired for EVERY
/// supplied peer at once. The multi-peer sibling of [`run_openlore_pull`]:
/// PP-7 subscribes to ≥2 peers and pulls them in ONE invocation so the
/// per-peer fault-isolation loop (WD-37, sequential per ADR-016) is
/// exercised end-to-end. Each `PeerSeam` contributes its
/// `OPENLORE_PEER_PDS_ENDPOINT_<did>` + `OPENLORE_PEER_PUBKEY_HEX_<did>`
/// env vars; an unreachable peer simply has its `PeerPds` toggled to
/// `simulate_unreachable()` by the caller BEFORE this runs, so its
/// resolveDid / listRecords HTTP calls drop the connection and the
/// in-binary pull records the peer as a skip.
pub fn run_openlore_pull_multi(env: &TestEnv, args: &[&str], peers: &[PeerSeam<'_>]) -> CliOutcome {
    let bin = assert_cmd::cargo::cargo_bin("openlore");
    let mut cmd = Command::new(&bin);
    cmd.args(args)
        .env_clear()
        .env("OPENLORE_HOME", &env.home)
        .env("OPENLORE_DID", env.identity.author_did())
        .env("OPENLORE_KEY_SEED_HEX", &env.identity.seed_hex)
        .env("OPENLORE_PDS_ENDPOINT", env.pds.endpoint_url())
        .env("PATH", std::env::var("PATH").unwrap_or_default());

    for peer in peers {
        cmd.env(peer_resolver_env_var(peer.peer_did), peer.peer_endpoint);
        cmd.env(peer_pubkey_env_var(peer.peer_did), peer.peer_pubkey_hex);
    }

    let output = cmd
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

/// base64url-no-pad encode raw bytes (the lexicon `signature.sig` wire
/// encoding per ADR-006). Hand-rolled to avoid pulling a base64 crate into
/// the acceptance-support dev-deps; the production decoder in
/// `adapter-atproto-pds::peer_read` must agree byte-for-byte.
fn base64url_no_pad(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = chunk.get(1).copied().unwrap_or(0) as u32;
        let b2 = chunk.get(2).copied().unwrap_or(0) as u32;
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(ALPHABET[((n >> 18) & 0x3f) as usize] as char);
        out.push(ALPHABET[((n >> 12) & 0x3f) as usize] as char);
        if chunk.len() > 1 {
            out.push(ALPHABET[((n >> 6) & 0x3f) as usize] as char);
        }
        if chunk.len() > 2 {
            out.push(ALPHABET[(n & 0x3f) as usize] as char);
        }
    }
    out
}

/// Lowercase-hex encode raw bytes (the peer-pubkey env-seam wire shape).
pub fn hex_lower(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push_str(&format!("{b:02x}"));
    }
    out
}

/// Universe-bound: "the `graph query --federated` stdout contains NO merged
/// / consensus / aggregate row — every output row is attributed to exactly
/// one author DID (KPI-FED-2 zero-merge release gate)". Port-exposed name:
/// `cli.graph_query.merged_row_count == 0`.
///
/// The banned-substring half is cheap + final NOW (no production dependency)
/// so the federated_query scaffolds can call it directly. The
/// distinct-(author,cid)-tuple half (counting rows, matching against the
/// expected per-author sum) materializes per scenario in DELIVER's FQ-2 /
/// FQ-8 once the renderer exists.
pub fn assert_no_merged_rows_in_federated_output(outcome: &CliOutcome) {
    // The ADR-013 no-merge FOOTER guarantee legitimately contains the word
    // "merged" ("...No claims are merged."). That sentence is the
    // anti-merging promise itself, not a merged ROW — exclude it before the
    // banned-substring scan so the footer text does not trip its own gate.
    // (Any other occurrence of these substrings would be a real merged-row
    // label, which is what KPI-FED-2 forbids.)
    const NO_MERGE_FOOTER: &str = "No claims are merged.";
    let scanned = outcome.stdout.replace(NO_MERGE_FOOTER, "");
    for banned in &["merged", "consensus", "aggregate"] {
        assert!(
            !scanned.to_lowercase().contains(banned),
            "graph query --federated stdout must contain NO {:?} row \
             (KPI-FED-2 zero-merge gate); \n--- stdout ---\n{}\n--- stderr ---\n{}",
            banned,
            outcome.stdout,
            outcome.stderr
        );
    }
}

/// Seed `count` cached `peer_claims` rows attributed to `peer_did` directly
/// in DuckDB. Used as a PS-5 precondition: the production path that
/// populates `peer_claims` is `peer pull` (Phase 04), so until it lands the
/// soft-remove scenario seeds the cache via raw SQL. Test-support is the
/// only place raw SQL is acceptable (production goes through
/// `PeerStoragePort`). The rows satisfy the migration-v3 CHECK constraints
/// (`author_did <> ''`, `cid <> ''`, `confidence` in [0,1]).
pub fn seed_cached_peer_claims(env: &TestEnv, peer_did: &str, count: usize) {
    let db_path = env.duckdb_path();
    let conn = duckdb::Connection::open(&db_path).unwrap_or_else(|err| {
        panic!(
            "open DuckDB at {} to seed peer_claims: {err}",
            db_path.display()
        )
    });
    for i in 0..count {
        let cid = format!("bafyseed{peer_did}{i}").replace(':', "_");
        conn.execute(
            "INSERT INTO peer_claims \
                (cid, author_did, subject, predicate, object, confidence, \
                 composed_at, fetched_at, fetched_from_pds, signed_record_path) \
             VALUES (?, ?, ?, ?, ?, ?, now(), now(), ?, ?)",
            duckdb::params![
                cid,
                peer_did,
                format!("subject-{i}"),
                "endorses",
                format!("object-{i}"),
                0.8_f64,
                "https://peer.example/pds",
                format!("peer_claims/{}/{cid}.json", did_to_fs_segment(peer_did)),
            ],
        )
        .unwrap_or_else(|err| panic!("seed peer_claims row {i} for {peer_did}: {err}"));
    }
}

/// Universe-bound: "the `peer_subscriptions` row for `peer_did` is
/// soft-removed — `removed_at IS NOT NULL`". Port-exposed name:
/// `peer_storage.subscriptions.removed_at_set[did]`. The defining
/// observable of soft-remove (WD-25). Sibling of
/// `assert_one_active_subscription_for`.
pub fn assert_subscription_soft_removed_for(env: &TestEnv, peer_did: &str) {
    let db_path = env.duckdb_path();
    let conn = duckdb::Connection::open(&db_path).unwrap_or_else(|err| {
        panic!(
            "open DuckDB at {} for soft-remove assertion: {err}",
            db_path.display()
        )
    });

    let (total, removed): (i64, i64) = conn
        .query_row(
            "SELECT \
                count(*), \
                count(*) FILTER (WHERE removed_at IS NOT NULL) \
             FROM peer_subscriptions WHERE peer_did = ?",
            duckdb::params![peer_did],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap_or_else(|err| panic!("query peer_subscriptions for {peer_did}: {err}"));

    assert_eq!(
        total, 1,
        "expected the subscription row for {peer_did} to still EXIST after \
         soft-remove (soft-remove does NOT delete the row); got {total} rows"
    );
    assert_eq!(
        removed, 1,
        "expected the subscription row for {peer_did} to be soft-removed \
         (removed_at IS NOT NULL) after `peer remove`; got {removed} \
         soft-removed rows"
    );
}

/// Universe-bound: "the `peer_claims` store holds exactly `count` rows
/// attributed to `peer_did`". Port-exposed name:
/// `peer_storage.claims.row_count[did]`. PS-5 uses it to assert soft-remove
/// RETAINS every cached peer claim (count unchanged). Test-support is the
/// only place raw SQL is acceptable; production goes through
/// `PeerStoragePort`.
pub fn assert_peer_claims_row_count_for(env: &TestEnv, peer_did: &str, count: usize) {
    let db_path = env.duckdb_path();
    let conn = duckdb::Connection::open(&db_path).unwrap_or_else(|err| {
        panic!(
            "open DuckDB at {} for peer_claims count assertion: {err}",
            db_path.display()
        )
    });

    let total: i64 = conn
        .query_row(
            "SELECT count(*) FROM peer_claims WHERE author_did = ?",
            duckdb::params![peer_did],
            |r| r.get(0),
        )
        .unwrap_or_else(|err| panic!("query peer_claims for {peer_did}: {err}"));

    assert_eq!(
        total as usize, count,
        "expected exactly {count} peer_claims rows for {peer_did}; got {total} \
         (soft-remove must RETAIN cached peer claims — WD-25)"
    );
}

/// Universe-bound: "the on-disk partition `peer_claims/<encoded_did>/` does
/// NOT exist (hard-purge removed it)". Port-exposed name:
/// `storage.peer_claims_fs.dir_exists_for[did] == false`.
///
/// Implemented in full NOW: directory absence is observable without any
/// unimplemented production code (the purge scenario writes the directory
/// via the real adapter, then this helper asserts its removal). Uses the
/// DID→fs encoding (`did_to_fs_segment`) as the single source of truth.
pub fn assert_peer_claims_dir_removed_for(env: &TestEnv, peer_did: &str) {
    let dir = peer_claims_dir_for(env, peer_did);
    assert!(
        !dir.exists(),
        "expected peer_claims partition for {peer_did} to be removed after \
         hard-purge, but it still exists at {} (KPI-FED-4 zero purge residue)",
        dir.display()
    );
}

/// Universe-bound: "the one-time orientation/framing block (named by
/// `marker`) appears EXACTLY ONCE across the two supplied invocation
/// outputs — present in `first`, absent in `second`". Port-exposed name:
/// `cli.orientation.emitted_count[marker] == 1`.
///
/// Covers the WD-39 (first federated query), WD-43 (first counter-claim),
/// and the once-per-user orientation gates. The body is final NOW: it scans
/// observable stdout for `marker` across the two outcomes — no dependency on
/// unimplemented production code beyond the orientation text itself, which
/// each scenario passes in as `marker`.
pub fn assert_orientation_emitted_exactly_once(
    first: &CliOutcome,
    second: &CliOutcome,
    marker: &str,
) {
    assert!(
        first.stdout.contains(marker),
        "expected the one-time orientation marker {marker:?} on the FIRST \
         invocation; \n--- first stdout ---\n{}\n--- first stderr ---\n{}",
        first.stdout,
        first.stderr
    );
    assert!(
        !second.stdout.contains(marker),
        "expected the one-time orientation marker {marker:?} to be ABSENT on \
         the SECOND invocation (once-per-user gate; WD-39 / WD-43); \
         \n--- second stdout ---\n{}\n--- second stderr ---\n{}",
        second.stdout,
        second.stderr
    );
}

// =============================================================================
// Slice-03 — PS-6 hard-purge preconditions + DD-FED-10 state-delta universe
// =============================================================================

/// Seed the on-disk `peer_claims/<encoded_did>/` partition with `count`
/// `<cid>.json` placeholder artifacts — the filesystem half of the cached
/// peer-claim state hard-purge must remove AFTER the DB commit (Q-DELIVER-2
/// colon→underscore encoding; data-models.md §"On-disk artifact format").
/// Pairs with [`seed_cached_peer_claims`] (the DB half). Test-support is the
/// only place these are written directly; `peer pull` (Phase 04) is the
/// production populate path.
pub fn seed_peer_claims_dir(env: &TestEnv, peer_did: &str, count: usize) {
    let dir = peer_claims_dir_for(env, peer_did);
    std::fs::create_dir_all(&dir)
        .unwrap_or_else(|e| panic!("create peer_claims dir {}: {e}", dir.display()));
    for i in 0..count {
        let cid = format!("bafyseed{peer_did}{i}").replace(':', "_");
        let artifact = dir.join(format!("{cid}.json"));
        std::fs::write(&artifact, b"{}\n")
            .unwrap_or_else(|e| panic!("write peer claim artifact {}: {e}", artifact.display()));
    }
}

/// Seed `count` user-authored claims directly into the slice-01 `claims`
/// (author) table — the user's OWN claims (including counter-claims) that
/// hard-purge MUST preserve (WD-25 / WD-41). Distinct table from
/// `peer_claims`; never targeted by purge. Test-support is the only place
/// raw SQL is acceptable; production goes through `StoragePort`.
pub fn seed_user_author_claims(env: &TestEnv, count: usize) {
    let db_path = env.duckdb_path();
    let conn = duckdb::Connection::open(&db_path).unwrap_or_else(|err| {
        panic!(
            "open DuckDB at {} to seed author claims: {err}",
            db_path.display()
        )
    });
    let author_did = env.identity.author_did().to_string();
    for i in 0..count {
        let cid = format!("bafyuser{i}");
        conn.execute(
            "INSERT INTO claims \
                (cid, subject, predicate, object, confidence, author_did, \
                 composed_at, artifact_path) \
             VALUES (?, ?, ?, ?, ?, ?, now(), ?)",
            duckdb::params![
                cid,
                format!("user-subject-{i}"),
                "counters",
                format!("user-object-{i}"),
                0.9_f64,
                author_did,
                format!("claims/{cid}.json"),
            ],
        )
        .unwrap_or_else(|err| panic!("seed author claims row {i}: {err}"));
    }
}

/// Universe-bound: "the `claims` (author) store holds exactly `count` rows".
/// Port-exposed name: `author_claims.row_count`. PS-6 uses it to assert
/// hard-purge PRESERVES every user-authored counter-claim (count unchanged —
/// the user's own table is never targeted). Test-support is the only place
/// raw SQL is acceptable; production goes through `StoragePort`.
pub fn assert_user_author_claim_count(env: &TestEnv, count: usize) {
    let total = author_claim_row_count(env);
    assert_eq!(
        total, count,
        "expected exactly {count} rows in the author `claims` table after \
         hard-purge (user counter-claims PRESERVED — WD-25 / WD-41); got {total}"
    );
}

/// Raw `SELECT count(*) FROM claims`. Port-exposed name:
/// `author_claims.row_count`. The author store is single-tenant, so a global
/// count is the observable surface for "the user's own claims were not
/// touched".
fn author_claim_row_count(env: &TestEnv) -> usize {
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

/// Raw `SELECT count(*) FROM peer_claims WHERE author_did = ?`. Port-exposed
/// name: `peer_storage.claims.row_count_by_author[did]`.
fn peer_claim_row_count(env: &TestEnv, peer_did: &str) -> usize {
    let db_path = env.duckdb_path();
    let conn = duckdb::Connection::open(&db_path).unwrap_or_else(|err| {
        panic!(
            "open DuckDB at {} for peer-claims count: {err}",
            db_path.display()
        )
    });
    let total: i64 = conn
        .query_row(
            "SELECT count(*) FROM peer_claims WHERE author_did = ?",
            duckdb::params![peer_did],
            |r| r.get(0),
        )
        .unwrap_or_else(|err| panic!("query peer_claims count for {peer_did}: {err}"));
    total.max(0) as usize
}

/// Raw `SELECT count(*) FROM peer_subscriptions WHERE peer_did = ?`.
/// Port-exposed name: `peer_storage.subscriptions.row_count[did]`.
fn peer_subscription_row_count(env: &TestEnv, peer_did: &str) -> usize {
    let db_path = env.duckdb_path();
    let conn = duckdb::Connection::open(&db_path).unwrap_or_else(|err| {
        panic!(
            "open DuckDB at {} for subscription count: {err}",
            db_path.display()
        )
    });
    let total: i64 = conn
        .query_row(
            "SELECT count(*) FROM peer_subscriptions WHERE peer_did = ?",
            duckdb::params![peer_did],
            |r| r.get(0),
        )
        .unwrap_or_else(|err| panic!("query subscription count for {peer_did}: {err}"));
    total.max(0) as usize
}

/// The four port-exposed slot NAMES that make up the PS-6 hard-purge
/// universe (DD-FED-10). Kept as one source of truth so `capture` and the
/// `universe` set never drift.
pub const PURGE_SLOT_PEER_CLAIMS: &str = "peer_storage.claims.row_count_by_author[did]";
pub const PURGE_SLOT_AUTHOR_CLAIMS: &str = "author_claims.row_count";
pub const PURGE_SLOT_FS_DIR: &str = "filesystem.peer_claims_dir.exists[did]";
pub const PURGE_SLOT_SUBSCRIPTION: &str = "peer_storage.subscriptions.row_count[did]";

/// DD-FED-10: capture the FULL observable purge universe as a slot-name →
/// value map (all values stringified so the heterogeneous count/bool slots
/// share one comparable type, matching the `state_delta` skeleton's
/// `HashMap<String, String>` shape). Port-exposed names only — never an
/// internal adapter field — so refactoring the adapter stays GREEN.
pub fn capture_purge_universe(env: &TestEnv, peer_did: &str) -> HashMap<String, String> {
    let mut snapshot = HashMap::new();
    snapshot.insert(
        PURGE_SLOT_PEER_CLAIMS.to_string(),
        peer_claim_row_count(env, peer_did).to_string(),
    );
    snapshot.insert(
        PURGE_SLOT_AUTHOR_CLAIMS.to_string(),
        author_claim_row_count(env).to_string(),
    );
    snapshot.insert(
        PURGE_SLOT_FS_DIR.to_string(),
        peer_claims_dir_for(env, peer_did).exists().to_string(),
    );
    snapshot.insert(
        PURGE_SLOT_SUBSCRIPTION.to_string(),
        peer_subscription_row_count(env, peer_did).to_string(),
    );
    snapshot
}

/// DD-FED-10: assert the hard-purge state-delta over the captured universe
/// via the inherited `assert_state_delta` port (`tests/common/state_delta.rs`).
///
/// Expected delta (the integration gate `peer_remove_purge_separation`):
///   - `peer_storage.claims.row_count_by_author[did]` → `set_to("0")` (peer
///     claims for the purged DID deleted);
///   - `author_claims.row_count`                      → UNCHANGED (the user's
///     own counter-claims PRESERVED — the load-bearing separation invariant);
///   - `filesystem.peer_claims_dir.exists[did]`       → `set_to("false")`
///     (the on-disk partition removed after the DB commit);
///   - `peer_storage.subscriptions.row_count[did]`    → `set_to("0")`
///     (the subscription row hard-deleted).
///
/// `assert_state_delta`'s implicit-unchanged rule pins `author_claims.row_count`
/// to byte-equality even though it is not named in `expected` — a regression
/// that deleted a user counter-claim would surface here, NOT silently.
pub fn assert_purge_state_delta(before: &HashMap<String, String>, after: &HashMap<String, String>) {
    use state_delta::{set_to, Delta};

    let universe: HashSet<String> = [
        PURGE_SLOT_PEER_CLAIMS,
        PURGE_SLOT_AUTHOR_CLAIMS,
        PURGE_SLOT_FS_DIR,
        PURGE_SLOT_SUBSCRIPTION,
    ]
    .into_iter()
    .map(String::from)
    .collect();

    let expected = Delta::new()
        .with_slot(PURGE_SLOT_PEER_CLAIMS, set_to("0".to_string()))
        .with_slot(PURGE_SLOT_FS_DIR, set_to("false".to_string()))
        .with_slot(PURGE_SLOT_SUBSCRIPTION, set_to("0".to_string()));
    // PURGE_SLOT_AUTHOR_CLAIMS deliberately omitted → implicit-unchanged:
    // the user's own claims MUST be byte-equal before/after the purge.

    state_delta::assert_state_delta(before, after, &universe, &expected);
}

// =============================================================================
// Slice-02 (github-scraper) support extensions — SCAFFOLD: true
// =============================================================================
//
// DISTILL slice-02 declares the assertion-helper SIGNATURES the scrape_*
// scenarios need; DELIVER materializes the bodies (todo!() until then). The
// scrape verb is driven through the `OPENLORE_GITHUB_API_BASE` seam (the
// FakeGithub base URL) + the optional `GITHUB_TOKEN` env-var (WD-63),
// mirroring how the slice-03 peer verbs use `OPENLORE_PEER_PDS_ENDPOINT_<did>`.
//
// Re-export the FakeGithub double + fixtures flat so the scrape_* files name
// them via `use support::*` (matching how the slice-03 peer doubles surface).
pub use openlore_test_support::fake_github::{
    FakeAuthMode, FakeGithub, FakeGithubErrorPosture, FakeGithubHttpHandle, FakeSignal,
    FakeTargetKind, FIXTURE_REJECTED_PAT, FIXTURE_REPO_TARGET, FIXTURE_USER_TARGET,
    FIXTURE_VALID_PAT,
};
pub use openlore_test_support::{
    fixture_cargo_five_signals, fixture_three_docs_signals_one_predicate,
    fixture_torvalds_user_aggregate_signals,
};

/// A running `FakeGithub` in-process HTTP server, owning its own tokio
/// runtime so the synchronous acceptance tests can stand it up + read its
/// base URL without an `async` test body.
///
/// Mirrors the [`FakePds`] / [`PeerPds`] wrappers exactly: a dedicated
/// multi-threaded runtime block-ons [`FakeGithub::serve_http`], and the
/// runtime is released on a background thread at drop so the test thread
/// never blocks on a parked `accept()` (the macOS shutdown hazard the other
/// two wrappers already work around).
///
/// Step 03-01 materializes this for SG-1 (the slice-02 walking skeleton).
pub struct GithubServer {
    fake: FakeGithub,
    http_handle: FakeGithubHttpHandle,
    runtime: std::mem::ManuallyDrop<tokio::runtime::Runtime>,
}

impl GithubServer {
    /// Start the supplied `FakeGithub` posture on an in-process HTTP server
    /// bound to a random `127.0.0.1` port.
    pub fn start(fake: FakeGithub) -> Self {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_io()
            .enable_time()
            .thread_name("fake-github-rt")
            .build()
            .expect("GithubServer::start: build tokio multi_thread runtime");
        let http_handle = runtime.block_on(fake.serve_http());
        Self {
            fake,
            http_handle,
            runtime: std::mem::ManuallyDrop::new(runtime),
        }
    }

    /// The `http://127.0.0.1:<port>` base URL to feed the `openlore`
    /// subprocess via `OPENLORE_GITHUB_API_BASE`.
    pub fn base_url(&self) -> &str {
        self.http_handle.base_url()
    }

    /// The underlying `FakeGithub` (for `seen_paths` / `saw_token`
    /// observation in the sad-path scenarios).
    pub fn fake(&self) -> &FakeGithub {
        &self.fake
    }
}

impl Drop for GithubServer {
    fn drop(&mut self) {
        // SAFETY: ManuallyDrop::take is sound because we only call it here in
        // the Drop impl and never again. Releasing the runtime on a
        // background thread keeps the test thread from blocking on a parked
        // accept() during teardown (same pattern as FakePds / PeerPds).
        let rt = unsafe { std::mem::ManuallyDrop::take(&mut self.runtime) };
        let _ = std::thread::Builder::new()
            .name("fake-github-shutdown".to_string())
            .spawn(move || drop(rt));
    }
}

/// Run `openlore scrape github <target> ...` against a `FakeGithub` HTTP
/// double, injecting its base URL via `OPENLORE_GITHUB_API_BASE` (the
/// test-only seam). No stdin; for `--sign` flows that need to drive the
/// chained compose/sign/publish prompts use
/// [`run_openlore_scrape_with_stdin`].
///
/// Mirrors [`run_openlore`] (clean env + the slice-01 stub seams + the
/// in-process FakePds endpoint) and adds the `OPENLORE_GITHUB_API_BASE`
/// seam pointing at the FakeGithub server. Step 03-01 materializes this for
/// SG-1.
pub fn run_openlore_scrape(env: &TestEnv, args: &[&str], github_base_url: &str) -> CliOutcome {
    let bin = assert_cmd::cargo::cargo_bin("openlore");
    let output = Command::new(&bin)
        .args(args)
        .env_clear()
        .env("OPENLORE_HOME", &env.home)
        .env("OPENLORE_DID", env.identity.author_did())
        .env("OPENLORE_KEY_SEED_HEX", &env.identity.seed_hex)
        .env("OPENLORE_PDS_ENDPOINT", env.pds.endpoint_url())
        .env("OPENLORE_GITHUB_API_BASE", github_base_url)
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

/// Run `openlore scrape github <target> --sign ...` against a `FakeGithub`
/// HTTP double, feeding `stdin_lines` (newline-joined) at the chained
/// compose/sign/publish prompts. Used by the SS-* sign scenarios.
///
/// Mirrors [`run_openlore_scrape`] (clean env + the slice-01 stub seams +
/// the in-process FakePds endpoint + the `OPENLORE_GITHUB_API_BASE` seam)
/// and additionally pipes `stdin_lines` so the `--sign` compose editor +
/// the two-prompt sign/publish flow can be driven byte-for-byte. The
/// unbuffered byte-at-a-time stdin reader in `crate::io` keeps each prompt
/// in lockstep with the wire (same pattern the slice-01 two-prompt flow uses).
pub fn run_openlore_scrape_with_stdin(
    env: &TestEnv,
    args: &[&str],
    github_base_url: &str,
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
        .env("OPENLORE_PDS_ENDPOINT", env.pds.endpoint_url())
        .env("OPENLORE_GITHUB_API_BASE", github_base_url)
        .env("PATH", std::env::var("PATH").unwrap_or_default())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = cmd
        .spawn()
        .unwrap_or_else(|e| panic!("spawn openlore at {bin:?}: {e}"));
    if !stdin_lines.is_empty() {
        let stdin = child.stdin.as_mut().expect("stdin pipe");
        stdin
            .write_all(stdin_lines.as_bytes())
            .expect("write stdin");
    }
    drop(child.stdin.take());

    let output = child.wait_with_output().expect("wait_with_output");
    CliOutcome {
        status: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    }
}

/// Recover the published claim's CID from a publish success block. The
/// renderer emits `Published claim <cid>.` (or `Claim <cid> already
/// published.`); this parses the CID token so universe-bound assertions can
/// target the exact record without the test hard-coding a CID.
pub fn published_cid_from_stdout(stdout: &str) -> String {
    for line in stdout.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("Published claim ") {
            return rest.trim_end_matches('.').trim().to_string();
        }
        if let Some(rest) = trimmed.strip_prefix("Claim ") {
            if let Some(cid) = rest.strip_suffix(" already published.") {
                return cid.trim().to_string();
            }
        }
    }
    panic!(
        "could not find a 'Published claim <cid>.' line in stdout to recover the CID; \
         \n--- stdout ---\n{stdout}"
    );
}

/// Run `openlore scrape github <target> ...` with a `GITHUB_TOKEN` PAT set
/// in the child env (WD-63 env-var seam) alongside the FakeGithub base URL.
/// Used by the SA-* auth scenarios.
///
/// Mirrors [`run_openlore_scrape`] exactly (clean env + the slice-01 stub
/// seams + the in-process FakePds endpoint + the `OPENLORE_GITHUB_API_BASE`
/// seam) and ADDITIONALLY sets `GITHUB_TOKEN` so `adapter-github` reads the
/// authenticated posture (WD-63 env-var-only PAT). The token leaves the test
/// ONLY into the child's env; the no-token-leak assertion verifies it never
/// surfaces in the captured output.
pub fn run_openlore_scrape_with_token(
    env: &TestEnv,
    args: &[&str],
    github_base_url: &str,
    github_token: &str,
) -> CliOutcome {
    let bin = assert_cmd::cargo::cargo_bin("openlore");
    let output = Command::new(&bin)
        .args(args)
        .env_clear()
        .env("OPENLORE_HOME", &env.home)
        .env("OPENLORE_DID", env.identity.author_did())
        .env("OPENLORE_KEY_SEED_HEX", &env.identity.seed_hex)
        .env("OPENLORE_PDS_ENDPOINT", env.pds.endpoint_url())
        .env("OPENLORE_GITHUB_API_BASE", github_base_url)
        .env("GITHUB_TOKEN", github_token)
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

/// Universe-bound (gate `scraper_never_persists_unsigned`, KPI-SCR-2):
/// assert the human-gate held at the storage layer — running `scrape github`
/// without `--sign` produced ZERO observable persistence. Port-exposed
/// universe: `author_claims.row_count == 0`, `pds.records.len == 0`,
/// `claims_dir.artifact_count == 0`. The load-bearing human-gate proof.
///
/// SCAFFOLD: true — DELIVER materializes this (and MAY migrate it to an
/// explicit `assert_state_delta(before, after, universe, expected)` form per
/// DD-SCR-10, mirroring slice-03's `assert_purge_state_delta`).
pub fn assert_no_claim_persisted(env: &TestEnv) {
    // (1) Zero `claims` rows in DuckDB. The DB file may not exist at all when
    // the scrape ran without `--sign` (no write path ever opened it) — that
    // is the strongest possible form of "zero rows", so treat absence as 0.
    let db_path = env.duckdb_path();
    if db_path.exists() {
        let conn = duckdb::Connection::open(&db_path).unwrap_or_else(|err| {
            panic!(
                "open DuckDB at {} for no-claim-persisted assertion: {err}",
                db_path.display()
            )
        });
        // The `claims` table itself may not exist if no migration ran; a
        // failed query is therefore also "zero rows".
        let row_count: i64 = conn
            .query_row("SELECT count(*) FROM claims", [], |r| r.get(0))
            .unwrap_or(0);
        assert_eq!(
            row_count, 0,
            "scraper_never_persists_unsigned (KPI-SCR-2): running `scrape github` without \
             --sign must write ZERO rows to the `claims` table; got {row_count}"
        );
    }

    // (2) Zero PDS create_record calls (the publish-layer half of the gate).
    assert_no_pds_call_was_made(env);

    // (3) Zero claim artifact files under claims_dir.
    assert_no_local_claim_files_exist(env);
}

/// Universe-bound (gate `candidate_names_source_signal`, KPI-SCR-3): assert
/// each numbered candidate's source-signal line names the expected signal.
/// `expected` pairs each candidate index (1-based) with a substring that
/// MUST appear on its source-signal line (auditability; I-SCR-4).
///
/// SCAFFOLD: true — DELIVER materializes this in step 07-01.
pub fn assert_candidate_names_signal(outcome: &CliOutcome, expected: &[(usize, &str)]) {
    // SCAFFOLD: true
    let _ = (outcome, expected);
    todo!(
        "DELIVER (slice-02): assert each numbered candidate's source-signal line names its \
         originating signal substring (candidate_names_source_signal gate)"
    )
}

/// Universe-bound (gate `candidate_confidence_no_autoinflate`, KPI-SCR-2,
/// proposal half): assert EVERY rendered candidate displays the expected
/// numeric confidence + bucket label, and no candidate displays a confidence
/// above 0.3 (WD-52 / I-SCR-3).
///
/// Materialized (step 04-03).
pub fn assert_candidate_confidence(outcome: &CliOutcome, expected: f64, bucket_label: &str) {
    assert_eq!(
        outcome.status, 0,
        "scrape must exit 0 before its candidate confidences can be asserted; \
         \n--- stdout ---\n{}\n--- stderr ---\n{}",
        outcome.stdout, outcome.stderr
    );

    let stdout = &outcome.stdout;

    // The render layer prints, per candidate:
    //   confidence  : 0.25 (speculative)
    // where the numeric form mirrors serde's minimal-decimal rendering (e.g.
    // `0.25`) and the bucket label is the WD-10 compose-time display bucket.
    // We pin BOTH the exact displayed line AND the parsed numeric value so the
    // no-auto-inflate guardrail (<= 0.3) is enforced over EVERY candidate, not
    // just the literal string match.
    let expected_numeric = serde_json::to_value(expected)
        .map(|value| value.to_string())
        .expect("expected confidence value is well-formed");
    let expected_line = format!("confidence  : {expected_numeric} ({bucket_label})");

    // Collect every rendered candidate's `confidence  :` line. The renderer
    // emits exactly one per candidate, so this IS the per-candidate universe.
    let confidence_lines: Vec<&str> = stdout
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with("confidence  :"))
        .collect();

    assert!(
        !confidence_lines.is_empty(),
        "expected at least one rendered candidate confidence line \
         (`confidence  : ...`) in the candidate list; \n--- stdout ---\n{stdout}"
    );

    for line in &confidence_lines {
        // Exact display contract: confidence shows `expected (bucket_label)`.
        assert_eq!(
            *line, expected_line,
            "every candidate's confidence must display as {expected_line:?} \
             (candidate_confidence_no_autoinflate, proposal half — WD-52 / I-SCR-3); \
             \n--- offending line ---\n{line}\n--- full stdout ---\n{stdout}"
        );

        // No-auto-inflate guardrail: parse the displayed numeric and prove it
        // is never proposed above 0.3 (KPI-SCR-2), independent of the label.
        let numeric_text = line
            .trim_start_matches("confidence  :")
            .split('(')
            .next()
            .map(str::trim)
            .unwrap_or("");
        let numeric: f64 = numeric_text.parse().unwrap_or_else(|err| {
            panic!(
                "candidate confidence {numeric_text:?} must parse as a number \
                 to enforce the no-auto-inflate ceiling: {err}; \n--- line ---\n{line}"
            )
        });
        assert!(
            numeric <= 0.3,
            "no candidate may be proposed with confidence above 0.3 \
             (candidate_confidence_no_autoinflate, KPI-SCR-2); got {numeric} on line {line:?}; \
             \n--- full stdout ---\n{stdout}"
        );
    }
}

/// Universe-bound (gate `candidate_confidence_no_autoinflate`, sign half):
/// assert the signed-from-scraper claim at `cid` recorded EXACTLY the
/// expected numeric confidence (no auto-inflation between proposal and sign
/// unless the human edited it). Port-exposed name:
/// `claims/<cid>.json::confidence`.
///
/// SCAFFOLD: true — DELIVER materializes this in step 07-01.
pub fn assert_candidate_confidence_unchanged(env: &TestEnv, cid: &str, expected: f64) {
    let artifact_path = env.claims_dir().join(format!("{cid}.json"));
    let json_bytes = std::fs::read(&artifact_path).unwrap_or_else(|e| {
        panic!(
            "expected signed-from-scraper claim file at {}; got {e}",
            artifact_path.display()
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

    // Confidence is a crate-private-wrapped f64; round-trip through serde to
    // read its numeric value (the same trick test-support uses to build it).
    let actual: f64 = serde_json::to_value(&signed.unsigned.confidence)
        .ok()
        .and_then(|v| v.as_f64())
        .unwrap_or_else(|| {
            panic!(
                "could not read numeric confidence from signed claim at {}",
                artifact_path.display()
            )
        });
    assert!(
        (actual - expected).abs() < f64::EPSILON,
        "expected signed claim {cid} to record confidence {expected} \
         (sign-time half of candidate_confidence_no_autoinflate); got {actual}"
    );
}

/// Universe-bound (gate `scraper_reuses_slice01_publish_path`, I-SCR-6):
/// assert the signed-from-scraper claim at `cid` was published via the SAME
/// `VerbClaimPublish` path as a hand-authored claim — exactly ONE record on
/// the user's OWN PDS under the user's OWN author DID at-uri, no parallel
/// publish path. Port-exposed names: `pds.records.len`,
/// `pds.records[at_uri].author_did`.
///
/// SCAFFOLD: true — DELIVER materializes this in step 07-01.
pub fn assert_scraper_reuses_slice01_publish_path(env: &TestEnv, cid: &str) {
    // The signed-from-scraper claim is the user's OWN artifact: it lands on
    // the user's OWN PDS under the user's OWN bare author DID at-uri, exactly
    // as a hand-authored `claim add` claim would. The bare DID (no
    // `#fragment` signing-key locator) is what the publish path records.
    let bare_author_did = env
        .identity
        .author_did()
        .split('#')
        .next()
        .unwrap_or_else(|| env.identity.author_did())
        .to_string();
    let expected_at_uri = format!("at://{bare_author_did}/org.openlore.claim/{cid}");

    // Exactly ONE record on the user's OWN PDS — proving the single-publish
    // path ran once (no parallel/forked publish that would double-post or
    // post under a different author DID).
    let records = env.pds.records();
    assert_eq!(
        records.len(),
        1,
        "scraper_reuses_slice01_publish_path (I-SCR-6 / WD-66): the user's OWN PDS must \
         hold EXACTLY ONE record after `--sign` (no parallel publish path); got {}: {:?}",
        records.len(),
        records
    );

    // ... and that one record is the signed claim, at the user's own at-uri,
    // published via the slice-01 VerbClaimPublish path (rkey == CID; FR-2/FR-3).
    let record = &records[0];
    assert_eq!(
        record.at_uri, expected_at_uri,
        "the published record must live at the user's OWN at-uri {expected_at_uri} \
         (published via the slice-01 path); got {}",
        record.at_uri
    );
    assert_eq!(
        record.collection, "org.openlore.claim",
        "the published record must be in the org.openlore.claim collection; got {}",
        record.collection
    );
}

/// Universe-bound (gate `scraper_only_reads_public_data`, KPI-SCR-4 —
/// release-blocking): assert the production code hit ONLY public-endpoint
/// allowlist paths against the FakeGithub double — NO private/authenticated-
/// private endpoint was reached. Reads `FakeGithub::seen_paths()`.
///
/// SCAFFOLD: true — DELIVER materializes this in step 07-01.
pub fn assert_only_public_endpoints_called(github: &FakeGithub) {
    // SCAFFOLD: true
    let _ = github;
    todo!(
        "DELIVER (slice-02): assert every FakeGithub::seen_paths() entry is on the public \
         GitHub endpoint allowlist — no private path was ever called \
         (scraper_only_reads_public_data gate, KPI-SCR-4 release-blocking)"
    )
}

/// Assert the token VALUE never appears in any captured output line
/// (US-SCR-004 no-token-leak). Pairs with `FakeGithub::saw_token(token)`
/// returning true (the production code DID send it) so the test proves auth
/// happened WITHOUT the value ever surfacing to stdout/stderr.
///
/// Materialized (step 04-06).
pub fn assert_token_value_absent(outcome: &CliOutcome, token: &str) {
    assert!(
        !outcome.stdout.contains(token),
        "no-token-leak (US-SCR-004): the PAT value must NEVER appear in stdout; \
         \n--- offending token ---\n{token}\n--- stdout ---\n{}",
        outcome.stdout
    );
    assert!(
        !outcome.stderr.contains(token),
        "no-token-leak (US-SCR-004): the PAT value must NEVER appear in stderr; \
         \n--- offending token ---\n{token}\n--- stderr ---\n{}",
        outcome.stderr
    );
}

// =============================================================================
// Slice-04 — graph-seeding + scoring/traversal assertion helpers (step 07-01)
// =============================================================================
//
// SCAFFOLD: true (slice-04)
//
// Slice-04 is a READ slice over the LOCAL federated graph: own claims
// (slice-01 `claims`), peer claims (slice-03 `peer_claims`), and scraper-signed
// claims (slice-02, normal author claims). Scoring + traversal are local
// read-only analysis over the REAL DuckDB store — so slice-04 needs NO new
// external fake. The graph is SEEDED into the real store by REUSING the
// slice-03 seam: own claims via the real `claim add` verb, peer claims via the
// real `peer add` + `peer pull` verbs against the slice-03 `PeerPds` double
// (`build_verifiable_peer_records` / `build_verifiable_peer_records_with_objects`).
//
// Per DD-GRAPH-10 (symmetric with slice-03 DD-FED-10): the load-bearing
// assertion helpers carry a `todo!()` body with a precise contract docstring;
// the SIGNATURES are correct NOW so every slice-04 test file compiles and
// reaches its own `todo!()` (RED, not BROKEN). DELIVER materializes the bodies
// per scenario; the universe entries each helper names MUST be port-exposed
// (rendered-output substrings, DuckDB row counts, on-disk artifact scans) —
// NEVER internal scoring/StoragePort struct fields (Mandate 8).

/// A named federated-graph fixture: the precondition shape a slice-04 scenario
/// seeds into the REAL DuckDB before exercising the explorer verbs. Each
/// variant maps to a worked example in user-stories.md / data-models.md.
///
/// SCAFFOLD: true (slice-04) — DELIVER fills each variant's concrete seeding
/// recipe (which own claims via `claim add`, which peers via `peer add` +
/// `peer pull` with `build_verifiable_peer_records*`).
#[derive(Debug, Clone)]
pub enum FederatedGraphFixture {
    /// US-GRAPH-001 Example 1: 4 dependency-pinning claims across 3 projects by
    /// 3 authors (Rachel/cargo 0.91, Tobias/deno 0.55, Maria/deno 0.40,
    /// Rachel/nixpkgs 0.88).
    DependencyPinningThreeAuthors,
    /// US-GRAPH-001 Example 3: github:denoland/deno with the SAME (subject,
    /// object) by the local user (0.40) AND a pulled peer (Tobias 0.55) — the
    /// identical-content zero-merge fixture.
    DenoIdenticalContentTwoAuthors,
    /// One own claim + one pulled peer claim about github:rust-lang/cargo — the
    /// bare-`--subject` default-off regression fixture (WD-87).
    CargoOwnPlusOnePeer,
    /// US-GRAPH-002 Example 1: did:plc:rachel-test authors 5 claims across 4
    /// subjects (cargo x2, nixpkgs, tokio, serde).
    RachelFiveClaimsFourSubjects,
    /// Three of the LOCAL user's own claims, no peers (self-review fixture).
    OwnClaimsOnlyThree,
    /// Tobias subscribed + pulled, THEN soft-removed (slice-03 `peer remove`
    /// without `--purge`) so his cache survives as unsubscribed-cache.
    TobiasThenSoftRemoved,
    /// US-GRAPH-003 Example 1 / data-models worked examples: cargo (Rachel 0.91,
    /// spans nixpkgs too), nixpkgs (Rachel 0.88), deno (Tobias 0.55 + Maria
    /// 0.40) — the canonical weighted/explain fixture.
    DependencyPinningWeightedWorkedExample,
    /// US-GRAPH-003 Example 2 / SC-3 leg: a single dependency-pinning—free
    /// actor-model claim (tokio, 1 author, conf 0.50, no span) — the sparse
    /// fixture.
    ActorModelSingleSparseClaim,
    /// US-GRAPH-003 Example 3: deno with reproducible-builds claims from 2
    /// distinct authors (Aanya 0.40 + Tobias 0.55) + a single-author comparator.
    ReproducibleBuildsMultiAuthor,
    /// US-GRAPH-003 Example 4: one project with two sharply-disagreeing
    /// confidences (0.85 and 0.20) by two authors.
    ConflictingConfidencesOneProject,
    /// US-GRAPH-004 Example 1: Rachel asserts dependency-pinning on BOTH cargo
    /// and nixpkgs — the cross-project span the traversal must surface.
    DependencyPinningRachelSpansTwoProjects,
    /// US-GRAPH-004 Example 3: a dense graph where one contributor's claims fan
    /// out beyond depth 2 (many philosophies + co-claimants).
    DenseFanOutBeyondDepthTwo,
}

impl FederatedGraphFixture {
    pub fn dependency_pinning_three_authors() -> Self {
        Self::DependencyPinningThreeAuthors
    }
    pub fn deno_identical_content_two_authors() -> Self {
        Self::DenoIdenticalContentTwoAuthors
    }
    pub fn cargo_own_plus_one_peer() -> Self {
        Self::CargoOwnPlusOnePeer
    }
    pub fn rachel_five_claims_four_subjects() -> Self {
        Self::RachelFiveClaimsFourSubjects
    }
    pub fn own_claims_only_three() -> Self {
        Self::OwnClaimsOnlyThree
    }
    pub fn tobias_then_soft_removed() -> Self {
        Self::TobiasThenSoftRemoved
    }
    pub fn dependency_pinning_weighted_worked_example() -> Self {
        Self::DependencyPinningWeightedWorkedExample
    }
    pub fn actor_model_single_sparse_claim() -> Self {
        Self::ActorModelSingleSparseClaim
    }
    pub fn reproducible_builds_multi_author() -> Self {
        Self::ReproducibleBuildsMultiAuthor
    }
    pub fn conflicting_confidences_one_project() -> Self {
        Self::ConflictingConfidencesOneProject
    }
    pub fn dependency_pinning_rachel_spans_two_projects() -> Self {
        Self::DependencyPinningRachelSpansTwoProjects
    }
    pub fn dense_fan_out_beyond_depth_two() -> Self {
        Self::DenseFanOutBeyondDepthTwo
    }
}

/// A live handle to a seeded federated graph: owns the `PeerPds` doubles (so
/// their HTTP servers stay alive for the duration of the scenario) and records
/// the seeded authors/subjects/cids so assertions can pin per-author rows.
/// Returned by [`seed_federated_graph`]; held by the test for the scenario's
/// lifetime (dropping it tears down the peer servers — RAII per scenario).
///
/// `Debug` so a failing assertion can print the seeded shape.
pub struct SeededGraph {
    /// The peer-PDS doubles, kept alive so their in-process HTTP servers keep
    /// answering for the scenario's lifetime (dropping tears them down). Held
    /// by DID for diagnostics; the `PeerPds` itself is not `Debug`.
    _peers: Vec<PeerPds>,
    /// The seeded (author_did, subject, object, confidence) tuples — the
    /// canonical fixture shape, recorded so assertions can pin per-author rows
    /// without re-deriving the recipe.
    pub seeded: Vec<SeededClaim>,
    /// The Ed25519 pubkey hex of each subscribed peer, in the SAME order as
    /// `_peers`. Retained so [`SeededGraph::add_peer_claim`] can re-wire the
    /// verify seam for EVERY already-subscribed peer on the post-add `peer
    /// pull` (the verb pulls all active subscriptions; an un-seamed peer would
    /// fail resolution and fail the pull).
    peer_pubkeys: Vec<String>,
}

impl std::fmt::Debug for SeededGraph {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SeededGraph")
            .field("peer_count", &self._peers.len())
            .field("seeded", &self.seeded)
            .finish()
    }
}

/// One seeded claim's canonical attribution shape (recorded by the seeder for
/// per-author-row assertions). The bare author DID, subject, object, and the
/// numeric confidence that the dimension view must surface verbatim.
#[derive(Debug, Clone)]
pub struct SeededClaim {
    pub author_did: String,
    pub subject: String,
    pub object: String,
    pub confidence: f64,
}

impl SeededGraph {
    /// Pull an ADDITIONAL peer claim mid-scenario (used by GQE-14 to prove the
    /// weight recomputes at query time after new claims arrive). Subscribes +
    /// pulls the extra claim via the real `peer add` + `peer pull` verbs, so
    /// the store genuinely gains a contributing row.
    ///
    /// SCAFFOLD: true (slice-04).
    pub fn add_peer_claim(&mut self, env: &TestEnv, claim: AddedPeerClaim) {
        let recipe = claim.recipe();

        // Materialize the additional peer's verifiable wire record (REAL
        // Ed25519 + CID recompute — the production pull pipeline verifies it),
        // start its PeerPds, subscribe via `peer add`, then pull it. This grows
        // the real store by a genuine contributing row, so the next weighted
        // query recomputes a DIFFERENT weight for the affected pairing.
        let (records, pubkey_hex) = build_verifiable_peer_records_for_triples(
            recipe.peer_did,
            recipe.seed,
            &[(recipe.subject, recipe.object, recipe.confidence)],
        );
        let pds = PeerPds::for_peer(recipe.peer_did, records);

        let added = run_openlore_with_peer_resolver(
            env,
            &["peer", "add", recipe.peer_did],
            recipe.peer_did,
            pds.endpoint_url(),
        );
        assert_eq!(
            added.status, 0,
            "SeededGraph::add_peer_claim: peer add for {} must succeed;\n\
             --- stdout ---\n{}\n--- stderr ---\n{}",
            recipe.peer_did, added.stdout, added.stderr
        );

        // The `peer pull` verb pulls EVERY active subscription, so the verify
        // seams for ALL already-subscribed peers must be re-wired alongside the
        // new one — otherwise the original peers fail DID resolution and the
        // pull exits non-zero. Their `PeerPds` doubles are still alive in
        // `_peers` (this handle owns them); pair each with its retained pubkey.
        self._peers.push(pds);
        self.peer_pubkeys.push(pubkey_hex);
        let seams: Vec<PeerSeam<'_>> = self
            ._peers
            .iter()
            .zip(self.peer_pubkeys.iter())
            .map(|(peer, pubkey)| PeerSeam {
                peer_did: peer.peer_did(),
                peer_endpoint: peer.endpoint_url(),
                peer_pubkey_hex: pubkey,
            })
            .collect();
        let pulled = run_openlore_pull_multi(env, &["peer", "pull"], &seams);
        assert_eq!(
            pulled.status, 0,
            "SeededGraph::add_peer_claim: peer pull must succeed;\n\
             --- stdout ---\n{}\n--- stderr ---\n{}",
            pulled.stdout, pulled.stderr
        );

        // Record the new contributing claim's attribution shape.
        self.seeded.push(SeededClaim {
            author_did: recipe.peer_did.to_string(),
            subject: recipe.subject.to_string(),
            object: recipe.object.to_string(),
            confidence: recipe.confidence,
        });
    }
}

/// The concrete (peer, subject, object, confidence) seeding recipe for one
/// [`AddedPeerClaim`] variant.
struct AddedPeerClaimRecipe {
    peer_did: &'static str,
    seed: [u8; 32],
    subject: &'static str,
    object: &'static str,
    confidence: f64,
}

/// One additional peer claim to inject mid-scenario via
/// [`SeededGraph::add_peer_claim`].
///
/// SCAFFOLD: true (slice-04) — DELIVER fills the concrete (peer, subject,
/// object, confidence) recipe per variant.
#[derive(Debug, Clone)]
pub enum AddedPeerClaim {
    /// A THIRD author's dependency-pinning claim on github:denoland/deno, so
    /// deno's weight changes on the re-run (GQE-14).
    DenoThirdAuthor,
}

impl AddedPeerClaim {
    pub fn deno_third_author() -> Self {
        Self::DenoThirdAuthor
    }

    /// The concrete seeding recipe for this variant.
    fn recipe(&self) -> AddedPeerClaimRecipe {
        match self {
            // A THIRD author (distinct DID + seed, not colliding with the
            // worked-example peers rachel[7]/tobias[9]/maria[11]) asserts
            // dependency-pinning on github:denoland/deno. After the pull, deno
            // gains a third contributing claim by a third distinct author — so
            // its adherence weight recomputes to a DIFFERENT value (GQE-14
            // query-time-compute proof).
            Self::DenoThirdAuthor => AddedPeerClaimRecipe {
                peer_did: "did:plc:priya-test",
                seed: [13u8; 32],
                subject: "github:denoland/deno",
                object: "org.openlore.philosophy.dependency-pinning",
                confidence: 0.66,
            },
        }
    }
}

/// Seed a named federated-graph fixture into the REAL DuckDB store by reusing
/// the slice-03 seam: own claims via the real `claim add` verb, peer claims via
/// the real `peer add` + `peer pull` verbs against `PeerPds` doubles built with
/// `build_verifiable_peer_records*`. NO new external fake — scoring/traversal
/// is local read-only analysis over the seeded real store.
///
/// Returns a [`SeededGraph`] that OWNS the peer-PDS doubles (keep it alive for
/// the scenario) and records the seeded authors/subjects/cids for assertions.
///
/// SCAFFOLD: true (slice-04) — DELIVER materializes each
/// [`FederatedGraphFixture`] variant's seeding recipe in step 07-* (the first
/// slice-04 step that wires the scoring crate + extended StoragePort + cli
/// flags). The seeder is the single place the slice-04 graph fixtures live, so
/// the per-scenario preconditions stay declarative (a fixture name, not 40
/// lines of subscribe/pull plumbing per test).
pub fn seed_federated_graph(env: &TestEnv, fixture: FederatedGraphFixture) -> SeededGraph {
    match fixture {
        FederatedGraphFixture::DependencyPinningThreeAuthors => {
            // US-GRAPH-001 Example 1: 4 dependency-pinning claims across 3
            // projects by 3 distinct authors — all PEER claims (the local user
            // makes no claim here). Each author is a separate subscribed peer;
            // their claims are materialized with REAL Ed25519 crypto + CID
            // recompute so the production pull pipeline verifies them.
            let dep = "org.openlore.philosophy.dependency-pinning";
            seed_peer_authored_graph(
                env,
                &[
                    SeedPeer {
                        peer_did: "did:plc:rachel-test",
                        seed: [7u8; 32],
                        triples: &[
                            ("github:rust-lang/cargo", dep, 0.91),
                            ("github:NixOS/nixpkgs", dep, 0.88),
                        ],
                    },
                    SeedPeer {
                        peer_did: "did:plc:tobias-test",
                        seed: [9u8; 32],
                        triples: &[("github:denoland/deno", dep, 0.55)],
                    },
                    SeedPeer {
                        peer_did: "did:plc:maria-test",
                        seed: [11u8; 32],
                        triples: &[("github:denoland/deno", dep, 0.40)],
                    },
                ],
            )
        }
        FederatedGraphFixture::DenoIdenticalContentTwoAuthors => {
            // US-GRAPH-001 Example 3 (KPI-GRAPH-2 zero-merge): the SAME
            // (subject, object) on github:denoland/deno asserted by TWO distinct
            // authors — the LOCAL user's OWN claim (0.40) AND a pulled PEER claim
            // from Tobias (0.55). The identical-content pair must render as TWO
            // rows (never merged), so the seeder mixes one own `claim add` with
            // one peer `peer add` + `peer pull`.
            let dep = "org.openlore.philosophy.dependency-pinning";
            let deno = "github:denoland/deno";
            seed_own_plus_peer_graph(
                env,
                &[OwnClaim {
                    subject: deno,
                    object: dep,
                    confidence: 0.40,
                }],
                &[SeedPeer {
                    peer_did: "did:plc:tobias-test",
                    seed: [9u8; 32],
                    triples: &[(deno, dep, 0.55)],
                }],
            )
        }
        FederatedGraphFixture::CargoOwnPlusOnePeer => {
            // GQE-3 (WD-87 bare-`--subject` default-off regression): the SAME
            // subject (github:rust-lang/cargo) asserted by BOTH the local user
            // (own `claim add`) AND one subscribed peer (`peer add` + `peer
            // pull`). The peer row is the load-bearing precondition: if the bare
            // `--subject` path EVER widened to peers under the explorer changes,
            // the peer's DID + cid WOULD surface. It must not — the bare path
            // stays own-claims-only (slice-01/03 contract preserved).
            let dep = "org.openlore.philosophy.dependency-pinning";
            let cargo = "github:rust-lang/cargo";
            seed_own_plus_peer_graph(
                env,
                &[OwnClaim {
                    subject: cargo,
                    object: dep,
                    confidence: 0.91,
                }],
                &[SeedPeer {
                    peer_did: "did:plc:rachel-test",
                    seed: [7u8; 32],
                    triples: &[(cargo, dep, 0.42)],
                }],
            )
        }
        FederatedGraphFixture::RachelFiveClaimsFourSubjects => {
            // US-GRAPH-002 Example 1 (GQE-6): did:plc:rachel-test authors 5
            // claims across 4 subjects (cargo x2, nixpkgs, tokio, serde) — the
            // contributor's full reasoning trail. All PEER claims (Rachel is a
            // subscribed peer; the local user makes no claim here). Materialized
            // with REAL Ed25519 crypto + CID recompute so the production pull
            // pipeline verifies them. The two cargo claims assert DISTINCT
            // philosophies so their canonical CIDs differ (the store keys on cid;
            // two identical triples would collide into one row).
            let dep = "org.openlore.philosophy.dependency-pinning";
            let repro = "org.openlore.philosophy.reproducible-builds";
            let workspace = "org.openlore.philosophy.workspace-cohesion";
            let actor = "org.openlore.philosophy.actor-model";
            let ergonomics = "org.openlore.philosophy.ergonomic-api";
            seed_peer_authored_graph(
                env,
                &[SeedPeer {
                    peer_did: "did:plc:rachel-test",
                    seed: [7u8; 32],
                    triples: &[
                        ("github:rust-lang/cargo", dep, 0.91),
                        ("github:rust-lang/cargo", repro, 0.74),
                        ("github:NixOS/nixpkgs", workspace, 0.88),
                        ("github:tokio-rs/tokio", actor, 0.62),
                        ("github:serde-rs/serde", ergonomics, 0.80),
                    ],
                }],
            )
        }
        FederatedGraphFixture::OwnClaimsOnlyThree => {
            // US-GRAPH-002 Example 2 (GQE-7 self-review): three of the LOCAL
            // user's OWN claims, no peers. Seeded via the real `claim add` verb
            // (source_table "Own" -> AuthorRelationship::You), so a
            // `--contributor <own_did>` query lists them annotated "(you)" — a
            // valid self-review, never "(subscribed peer)". Three DISTINCT
            // (subject, object) triples so their canonical CIDs differ (the
            // store keys on cid; identical triples would collide into one row).
            let dep = "org.openlore.philosophy.dependency-pinning";
            let repro = "org.openlore.philosophy.reproducible-builds";
            let memory = "org.openlore.philosophy.memory-safety";
            seed_own_plus_peer_graph(
                env,
                &[
                    OwnClaim {
                        subject: "github:rust-lang/cargo",
                        object: dep,
                        confidence: 0.91,
                    },
                    OwnClaim {
                        subject: "github:NixOS/nixpkgs",
                        object: repro,
                        confidence: 0.74,
                    },
                    OwnClaim {
                        subject: "github:rust-lang/rust",
                        object: memory,
                        confidence: 0.86,
                    },
                ],
                &[],
            )
        }
        FederatedGraphFixture::TobiasThenSoftRemoved => {
            // US-GRAPH-002 Example 4 (GQE-9): Tobias is subscribed + pulled (his
            // cached claims land in `peer_claims`), THEN soft-removed via the real
            // `peer remove` verb WITHOUT `--purge` (WD-25): his subscription row's
            // `removed_at` is set but his cache survives. A `--contributor
            // <tobias>` query must therefore list his RETAINED cached claims
            // annotated "(unsubscribed cache)" (the `removed_at IS NOT NULL` →
            // `AuthorRelationship::UnsubscribedCache` classification), NOT
            // "(subscribed peer)". The peer-authored seeder runs the real add +
            // pull; the soft-remove is appended here so the seeded shape is the
            // exact unsubscribed-cache relationship state.
            let dep = "org.openlore.philosophy.dependency-pinning";
            let repro = "org.openlore.philosophy.reproducible-builds";
            let tobias_did = "did:plc:tobias-test";
            let graph = seed_peer_authored_graph(
                env,
                &[SeedPeer {
                    peer_did: tobias_did,
                    seed: [9u8; 32],
                    triples: &[
                        ("github:denoland/deno", dep, 0.55),
                        ("github:denoland/deno", repro, 0.71),
                    ],
                }],
            );

            // Soft-remove Tobias via the real `peer remove` verb (no `--purge`):
            // sets `removed_at`, retains the cached `peer_claims` rows (WD-25).
            let removed = run_openlore(env, &["peer", "remove", tobias_did]);
            assert_eq!(
                removed.status, 0,
                "seed TobiasThenSoftRemoved: `peer remove {tobias_did}` (soft, no --purge) must \
                 succeed;\n--- stdout ---\n{}\n--- stderr ---\n{}",
                removed.stdout, removed.stderr
            );
            // Pin the soft-remove storage contract so the fixture is the genuine
            // unsubscribed-cache state, not merely "the verb exited 0": the
            // subscription row is soft-removed AND every cached claim is retained.
            assert_subscription_soft_removed_for(env, tobias_did);
            assert_peer_claims_row_count_for(env, tobias_did, graph.seeded.len());

            graph
        }
        FederatedGraphFixture::DependencyPinningWeightedWorkedExample => {
            // US-GRAPH-003 Example 1 / data-models.md worked examples (GQE-10
            // weighted; Gate 2): the canonical weighted fixture — cargo (Rachel
            // 0.91, ALSO claims nixpkgs so Rachel spans two projects → +0.50
            // cross-project triangulation), nixpkgs (Rachel 0.88, same span),
            // deno (Tobias 0.55 + Maria 0.40 → second-author +0.25 bonus). The
            // SAME peer-authored shape as DependencyPinningThreeAuthors (all PEER
            // claims, REAL Ed25519 crypto + CID recompute so the production pull
            // pipeline verifies them), recorded so the per-author breakdown rows
            // can be pinned. The pure scoring core aggregates these per-claim
            // rows into the ranked weights (Rust, NEVER SQL — I-GRAPH-2/WD-73).
            let dep = "org.openlore.philosophy.dependency-pinning";
            seed_peer_authored_graph(
                env,
                &[
                    SeedPeer {
                        peer_did: "did:plc:rachel-test",
                        seed: [7u8; 32],
                        triples: &[
                            ("github:rust-lang/cargo", dep, 0.91),
                            ("github:NixOS/nixpkgs", dep, 0.88),
                        ],
                    },
                    SeedPeer {
                        peer_did: "did:plc:tobias-test",
                        seed: [9u8; 32],
                        triples: &[("github:denoland/deno", dep, 0.55)],
                    },
                    SeedPeer {
                        peer_did: "did:plc:maria-test",
                        seed: [11u8; 32],
                        triples: &[("github:denoland/deno", dep, 0.40)],
                    },
                ],
            )
        }
        FederatedGraphFixture::DependencyPinningRachelSpansTwoProjects => {
            // US-GRAPH-004 Example 1 (GQE-20 / KPI-GRAPH-1 north star): the
            // cross-project span the traversal must surface. Rachel asserts
            // dependency-pinning on BOTH github:rust-lang/cargo (0.91) AND
            // github:NixOS/nixpkgs (0.88) — so a `--object dependency-pinning
            // --traverse` walk discovers that ONE contributor's claims
            // triangulate across two projects (the non-obvious "aha"). Tobias
            // contributes the third project (github:denoland/deno, 0.55) so the
            // tree fans philosophy -> {cargo, nixpkgs, deno} -> their authors,
            // and Rachel's two-project span stands out against Tobias's one.
            // All PEER claims (the local user makes none here), materialized
            // with REAL Ed25519 crypto + CID recompute so the production pull
            // pipeline verifies them and each edge maps to a real signed claim
            // (Gate 5). The two Rachel triples assert the SAME object on
            // DISTINCT subjects, so their canonical CIDs differ (the store keys
            // on cid; identical triples would collide into one row).
            let dep = "org.openlore.philosophy.dependency-pinning";
            seed_peer_authored_graph(
                env,
                &[
                    SeedPeer {
                        peer_did: "did:plc:rachel-test",
                        seed: [7u8; 32],
                        triples: &[
                            ("github:rust-lang/cargo", dep, 0.91),
                            ("github:NixOS/nixpkgs", dep, 0.88),
                        ],
                    },
                    SeedPeer {
                        peer_did: "did:plc:tobias-test",
                        seed: [9u8; 32],
                        triples: &[("github:denoland/deno", dep, 0.55)],
                    },
                ],
            )
        }
        FederatedGraphFixture::ActorModelSingleSparseClaim => {
            // US-GRAPH-003 Example 2 (GQE-11 sparse) + US-GRAPH-004 Example 2
            // (GQE-21 no-fabrication): ONE isolated claim — the local user's own
            // actor-model claim on github:tokio-rs/tokio at confidence 0.50, with
            // NO cross-project span and NO co-author. A single author on a single
            // project means the `--traverse` walk discovers the node but NO
            // connecting (cross-project) edges; the renderer states "no connecting
            // edges found at depth 2" and fabricates nothing (Gate 5). Seeded via
            // the real `claim add` verb (source_table "Own"), so the single edge
            // maps to a real signed local claim.
            let actor = "org.openlore.philosophy.actor-model";
            seed_own_plus_peer_graph(
                env,
                &[OwnClaim {
                    subject: "github:tokio-rs/tokio",
                    object: actor,
                    confidence: 0.50,
                }],
                &[],
            )
        }
        FederatedGraphFixture::DenseFanOutBeyondDepthTwo => {
            // US-GRAPH-004 Example 3 (GQE-22 / WD-76 bounded): a DENSE graph
            // where Rachel's claims fan out beyond depth 2 so the default
            // depth-2 bound MUST omit deeper edges. The recursive walk hops
            // within a shared subject (`eb.subject = w.subject`), so a project
            // carrying many distinct claims lets the walk reach depth 3+. Seed
            // FOUR distinct authors (Rachel + three co-claimants) asserting
            // dependency-pinning on the SAME shared project
            // (github:rust-lang/cargo): four distinct authors -> four distinct
            // CIDs on one subject, so a `--contributor did:plc:rachel-test
            // --traverse` walk anchored on Rachel's edge (depth 1) steps to the
            // co-claimants (depth 2) and again (depth 3) — the depth-2 default
            // bound cuts the depth-3 edges and reports them as omitted (WD-76).
            // All PEER claims (the local user makes none here), each with REAL
            // Ed25519 crypto + CID recompute so every edge maps to a real signed
            // claim (Gate 5).
            let dep = "org.openlore.philosophy.dependency-pinning";
            let cargo = "github:rust-lang/cargo";
            seed_peer_authored_graph(
                env,
                &[
                    SeedPeer {
                        peer_did: "did:plc:rachel-test",
                        seed: [7u8; 32],
                        triples: &[(cargo, dep, 0.91)],
                    },
                    SeedPeer {
                        peer_did: "did:plc:tobias-test",
                        seed: [9u8; 32],
                        triples: &[(cargo, dep, 0.80)],
                    },
                    SeedPeer {
                        peer_did: "did:plc:maria-test",
                        seed: [11u8; 32],
                        triples: &[(cargo, dep, 0.70)],
                    },
                    SeedPeer {
                        peer_did: "did:plc:aanya-test",
                        seed: [13u8; 32],
                        triples: &[(cargo, dep, 0.60)],
                    },
                ],
            )
        }
        FederatedGraphFixture::ReproducibleBuildsMultiAuthor => {
            // US-GRAPH-003 Example 3 (GQE-12 multi-author triangulation;
            // KPI-GRAPH-1/2): github:denoland/deno carries reproducible-builds
            // claims from TWO distinct authors (Tobias 0.55 + Aanya 0.40), so the
            // pairing earns the per-ADDITIONAL-distinct-author bonus (+0.25 on the
            // second author) and the multi-author breadth line fires. A
            // single-author comparator (Rachel on github:rust-lang/cargo at 0.55 —
            // similar MAX confidence to deno) anchors the ranking so the
            // triangulation lift is OBSERVABLE: deno (2 authors, weight ≈ 1.05+)
            // ranks above cargo (1 author, weight 0.55). All PEER claims, REAL
            // Ed25519 crypto + CID recompute so the production pull pipeline
            // verifies them; both deno authors stay individually attributed in the
            // decomposition (anti-merging, WD-73). No author spans two projects for
            // THIS object, so the lift is multi-author, not cross-project.
            let repro = "org.openlore.philosophy.reproducible-builds";
            let deno = "github:denoland/deno";
            let cargo = "github:rust-lang/cargo";
            seed_peer_authored_graph(
                env,
                &[
                    SeedPeer {
                        peer_did: "did:plc:tobias-test",
                        seed: [9u8; 32],
                        triples: &[(deno, repro, 0.55)],
                    },
                    SeedPeer {
                        peer_did: "did:plc:aanya-test",
                        seed: [13u8; 32],
                        triples: &[(deno, repro, 0.40)],
                    },
                    SeedPeer {
                        peer_did: "did:plc:rachel-test",
                        seed: [7u8; 32],
                        triples: &[(cargo, repro, 0.55)],
                    },
                ],
            )
        }
        FederatedGraphFixture::ConflictingConfidencesOneProject => {
            // US-GRAPH-003 Example 4 (GQE-13 anti-merging; KPI-GRAPH-2): ONE project
            // (github:denoland/deno) on dependency-pinning carries two sharply
            // DISAGREEING confidences by two distinct authors (Rachel 0.85, Tobias
            // 0.20). BOTH must contribute per their OWN confidence — never averaged
            // into a single 0.525 value, never dropped. The decomposition keeps both
            // authors AND both confidences visible (anti-merging, WD-73 / ADR-022).
            // All PEER claims, REAL Ed25519 crypto + CID recompute so the production
            // pull pipeline verifies them.
            let dep = "org.openlore.philosophy.dependency-pinning";
            let deno = "github:denoland/deno";
            seed_peer_authored_graph(
                env,
                &[
                    SeedPeer {
                        peer_did: "did:plc:rachel-test",
                        seed: [7u8; 32],
                        triples: &[(deno, dep, 0.85)],
                    },
                    SeedPeer {
                        peer_did: "did:plc:tobias-test",
                        seed: [9u8; 32],
                        triples: &[(deno, dep, 0.20)],
                    },
                ],
            )
        }
        // The remaining variants materialize per-scenario in later slice-04
        // steps (GQE-10..27 stay RED until then).
        other => {
            let _ = env;
            todo!(
                "DELIVER (slice-04, later step): seed the {other:?} federated-graph fixture \
                 (own claims via `claim add`, peer claims via `peer add` + `peer pull`)"
            )
        }
    }
}

/// One peer's seed recipe: its DID, Ed25519 seed, and the
/// `(subject, object, confidence)` triples it authors.
struct SeedPeer<'a> {
    peer_did: &'a str,
    seed: [u8; 32],
    triples: &'a [(&'a str, &'a str, f64)],
}

/// One of the LOCAL user's own claims to seed via the real `claim add` verb
/// (signed + persisted locally, NOT published — the read path only needs the
/// local row). The author is `env.identity.author_did()` ("(you)").
struct OwnClaim<'a> {
    subject: &'a str,
    object: &'a str,
    confidence: f64,
}

/// Seed a federated graph that MIXES the local user's own claims (via the real
/// `claim add` verb) with subscribed-peer claims (via `peer add` + `peer pull`).
/// Used by the identical-content zero-merge fixture (GQE-2 /
/// `DenoIdenticalContentTwoAuthors`): the same `(subject, object)` is asserted
/// by both the local user AND a peer, and they must land as TWO attributed rows
/// (own → `You`, peer → `SubscribedPeer`) — never a merged aggregate.
///
/// Returns a [`SeededGraph`] owning the live `PeerPds` doubles AND recording the
/// canonical seeded attribution shape (own claims first, attributed to the bare
/// local DID; then peer claims attributed to their authors) so the assertion can
/// pin per-author rows.
fn seed_own_plus_peer_graph(
    env: &TestEnv,
    own_claims: &[OwnClaim<'_>],
    peers: &[SeedPeer<'_>],
) -> SeededGraph {
    let local_did = env.identity.author_did().to_string();

    // -- Own claims via the real `claim add` verb. `\n` confirms the sign
    // prompt; `N` declines publishing (local-only — the read path needs only
    // the persisted local row). --
    let mut seeded: Vec<SeededClaim> = Vec::new();
    for own in own_claims {
        let confidence = own.confidence.to_string();
        let added = run_openlore_with_stdin(
            env,
            &[
                "claim",
                "add",
                "--subject",
                own.subject,
                "--predicate",
                "embodiesPhilosophy",
                "--object",
                own.object,
                "--evidence",
                "https://example.test/own",
                "--confidence",
                &confidence,
            ],
            "\nN\n",
        );
        assert_eq!(
            added.status, 0,
            "seed_own_plus_peer_graph: claim add for {} must succeed;\n--- stdout ---\n{}\n--- stderr ---\n{}",
            own.subject, added.stdout, added.stderr
        );
        seeded.push(SeededClaim {
            author_did: local_did.clone(),
            subject: own.subject.to_string(),
            object: own.object.to_string(),
            confidence: own.confidence,
        });
    }

    // -- Peer claims via the real `peer add` + `peer pull` verbs, reusing the
    // peer-authored seeding path (its returned graph owns the live PeerPds
    // doubles + records the peer attribution). --
    let peer_graph = seed_peer_authored_graph(env, peers);
    seeded.extend(peer_graph.seeded);

    SeededGraph {
        _peers: peer_graph._peers,
        seeded,
        peer_pubkeys: peer_graph.peer_pubkeys,
    }
}

/// Seed a federated graph whose claims are ALL authored by subscribed peers
/// (no local-user own claim). For each peer: build verifiable wire records,
/// start a `PeerPds`, subscribe via the real `peer add` verb, then pull every
/// subscribed peer in ONE `peer pull` so the per-claim rows land in the real
/// `peer_claims` table (each attributed to its author — anti-merging held).
/// Returns a [`SeededGraph`] owning the live `PeerPds` doubles.
fn seed_peer_authored_graph(env: &TestEnv, peers: &[SeedPeer<'_>]) -> SeededGraph {
    let mut held_peers: Vec<PeerPds> = Vec::new();
    let mut pubkeys: Vec<String> = Vec::new();
    let mut seeded: Vec<SeededClaim> = Vec::new();

    // Build records + start each peer's PDS, recording the canonical seeded
    // attribution shape for later per-author-row assertions.
    for peer in peers {
        let (records, pubkey_hex) =
            build_verifiable_peer_records_for_triples(peer.peer_did, peer.seed, peer.triples);
        for (subject, object, confidence) in peer.triples {
            seeded.push(SeededClaim {
                author_did: peer.peer_did.to_string(),
                subject: (*subject).to_string(),
                object: (*object).to_string(),
                confidence: *confidence,
            });
        }
        let pds = PeerPds::for_peer(peer.peer_did, records);

        // Subscribe via the real `peer add` verb (resolver wired for THIS peer).
        let added = run_openlore_with_peer_resolver(
            env,
            &["peer", "add", peer.peer_did],
            peer.peer_did,
            pds.endpoint_url(),
        );
        assert_eq!(
            added.status, 0,
            "seed_federated_graph: peer add for {} must succeed;\n--- stdout ---\n{}\n--- stderr ---\n{}",
            peer.peer_did, added.stdout, added.stderr
        );

        held_peers.push(pds);
        pubkeys.push(pubkey_hex);
    }

    // Pull every subscribed peer in ONE invocation (resolver + pubkey seams
    // wired for all peers at once). The production pull pipeline verifies each
    // record + recomputes its CID, then stores it attributed to its author.
    let seams: Vec<PeerSeam<'_>> = peers
        .iter()
        .zip(held_peers.iter())
        .zip(pubkeys.iter())
        .map(|((peer, pds), pubkey)| PeerSeam {
            peer_did: peer.peer_did,
            peer_endpoint: pds.endpoint_url(),
            peer_pubkey_hex: pubkey,
        })
        .collect();
    let pulled = run_openlore_pull_multi(env, &["peer", "pull"], &seams);
    assert_eq!(
        pulled.status, 0,
        "seed_federated_graph: peer pull must succeed;\n--- stdout ---\n{}\n--- stderr ---\n{}",
        pulled.stdout, pulled.stderr
    );

    SeededGraph {
        _peers: held_peers,
        seeded,
        peer_pubkeys: pubkeys,
    }
}

/// Build a verifiable peer record set over caller-supplied
/// `(subject, object, confidence)` triples. The flexible sibling of
/// [`build_verifiable_peer_records`] (which hardcodes Rachel's cargo triples)
/// and [`build_verifiable_peer_records_with_objects`] (which hardcodes the
/// subject). The slice-04 graph fixtures need full control of subject + object
/// + confidence per claim (cross-project spans, multi-author pairings), so this
/// materializes the same REAL Ed25519 crypto + CID-recompute the pull pipeline
/// verifies, over an arbitrary triple list.
///
/// Returns `(records, peer_pubkey_hex)` exactly like
/// [`build_verifiable_peer_records`]. Test-support is the only place this
/// construction is acceptable; the production populate path is `peer pull`.
pub fn build_verifiable_peer_records_for_triples(
    peer_did: &str,
    peer_seed: [u8; 32],
    triples: &[(&str, &str, f64)],
) -> (Vec<FakePeerRecord>, String) {
    use claim_domain::{canonicalize, compute_cid, sign, SigningKey, VerifyingKey};
    use ed25519_dalek::SigningKey as DalekSigningKey;

    let dalek_sk = DalekSigningKey::from_bytes(&peer_seed);
    let dalek_vk = dalek_sk.verifying_key();
    let signing_key = SigningKey(dalek_sk.to_bytes().to_vec());
    let pubkey_hex = hex_lower(&VerifyingKey(dalek_vk.to_bytes().to_vec()).0);

    let records = triples
        .iter()
        .map(|(subject, object, confidence)| {
            let confidence_wrapper: claim_domain::Confidence =
                serde_json::from_value(serde_json::json!(confidence))
                    .expect("confidence value is well-formed");
            let unsigned = claim_domain::UnsignedClaim {
                subject: (*subject).to_string(),
                predicate: "embodiesPhilosophy".to_string(),
                object: (*object).to_string(),
                evidence: vec![format!("https://example.test/{subject}")],
                confidence: confidence_wrapper,
                author_did: claim_domain::Did(format!("{peer_did}#org.openlore.application")),
                composed_at: "2026-05-22T09:18:44Z".to_string(),
                references: Vec::new(),
                reason: None,
            };

            let canonical = canonicalize(&unsigned).expect("canonicalize triple claim");
            let cid = compute_cid(&canonical);
            let signature = sign(&cid, &signing_key).expect("sign triple claim");
            let sig_b64 = base64url_no_pad(&signature.signature_bytes);

            let body = serde_json::json!({
                "subject": subject,
                "predicate": "embodiesPhilosophy",
                "object": object,
                "evidence": [format!("https://example.test/{subject}")],
                "confidence": confidence,
                "author": format!("{peer_did}#org.openlore.application"),
                "composedAt": "2026-05-22T09:18:44Z",
                "references": [],
                "signature": {
                    "kid": format!("{peer_did}#org.openlore.application"),
                    "alg": "EdDSA",
                    "sig": sig_b64,
                }
            });
            FakePeerRecord::claim(cid.0, body)
        })
        .collect();

    (records, pubkey_hex)
}

/// Build a verifiable peer record set over caller-supplied
/// `(subject, object, confidence, evidence)` quadruples — the IDENTICAL-SUBTOTAL
/// TWIN sibling of [`build_verifiable_peer_records_for_triples`] (which derives
/// `evidence` from `subject`, so two records sharing `(subject, object,
/// confidence)` canonicalize IDENTICALLY → the SAME deterministic CID → a row
/// collision). The slice-14 anti-misread twin fixture (SF-5) needs TWO claims in
/// the SAME `(subject, object)` pairing with the SAME `confidence` (so the pure
/// `scoring::score` apportions IDENTICAL subtotals — same author rank, same
/// triangulation status, same base) yet DISTINCT CIDs so the store keeps both
/// rows and a DISTINCT peer can counter EXACTLY ONE of them. `evidence` is the
/// ONLY canonicalized field that perturbs the CID WITHOUT touching the subtotal
/// (the subtotal reads `confidence` × author-rank-share + triangulation, never
/// `evidence`), so threading a DISTINCT `evidence` per twin yields distinct CIDs +
/// byte-equal subtotals.
///
/// Returns `(records, peer_pubkey_hex)` exactly like
/// [`build_verifiable_peer_records_for_triples`]. Test-support is the only place
/// this construction is acceptable; the production populate path is `peer pull`.
pub fn build_verifiable_peer_records_for_quadruples(
    peer_did: &str,
    peer_seed: [u8; 32],
    quadruples: &[(&str, &str, f64, &str)],
) -> (Vec<FakePeerRecord>, String) {
    use claim_domain::{canonicalize, compute_cid, sign, SigningKey, VerifyingKey};
    use ed25519_dalek::SigningKey as DalekSigningKey;

    let dalek_sk = DalekSigningKey::from_bytes(&peer_seed);
    let dalek_vk = dalek_sk.verifying_key();
    let signing_key = SigningKey(dalek_sk.to_bytes().to_vec());
    let pubkey_hex = hex_lower(&VerifyingKey(dalek_vk.to_bytes().to_vec()).0);

    let records = quadruples
        .iter()
        .map(|(subject, object, confidence, evidence)| {
            let confidence_wrapper: claim_domain::Confidence =
                serde_json::from_value(serde_json::json!(confidence))
                    .expect("confidence value is well-formed");
            let unsigned = claim_domain::UnsignedClaim {
                subject: (*subject).to_string(),
                predicate: "embodiesPhilosophy".to_string(),
                object: (*object).to_string(),
                evidence: vec![(*evidence).to_string()],
                confidence: confidence_wrapper,
                author_did: claim_domain::Did(format!("{peer_did}#org.openlore.application")),
                composed_at: "2026-05-22T09:18:44Z".to_string(),
                references: Vec::new(),
                reason: None,
            };

            let canonical = canonicalize(&unsigned).expect("canonicalize quadruple claim");
            let cid = compute_cid(&canonical);
            let signature = sign(&cid, &signing_key).expect("sign quadruple claim");
            let sig_b64 = base64url_no_pad(&signature.signature_bytes);

            let body = serde_json::json!({
                "subject": subject,
                "predicate": "embodiesPhilosophy",
                "object": object,
                "evidence": [evidence],
                "confidence": confidence,
                "author": format!("{peer_did}#org.openlore.application"),
                "composedAt": "2026-05-22T09:18:44Z",
                "references": [],
                "signature": {
                    "kid": format!("{peer_did}#org.openlore.application"),
                    "alg": "EdDSA",
                    "sig": sig_b64,
                }
            });
            FakePeerRecord::claim(cid.0, body)
        })
        .collect();

    (records, pubkey_hex)
}

/// Build ONE verifiable PEER COUNTER record: a signed claim authored by `peer_did`
/// that carries `references: [{ type: "counters", cid: target_cid }]` (ADR-015) +
/// an optional free-text `reason`, so that when it is `peer pull`ed through the
/// PRODUCTION federation path it lands in `peer_claims` + `peer_claim_references`
/// (referenced_cid == target_cid) and the ADR-046 2-step counter-thread read
/// (`query_counter_claims`) finds it as an attributed peer counter. This is the
/// sibling of [`build_verifiable_peer_records_for_triples`] (which hardcodes
/// `references: Vec::new()` + `reason: None`) — the slice-11 anti-merging /
/// empty-reason fixtures need a counter-shaped peer record, not a plain triple.
///
/// The `references` + `reason` are carried in BOTH the canonicalized
/// [`UnsignedClaim`] (so `compute_cid` is over the true counter shape) AND the
/// wire JSON body (so the pull pipeline's `parse_references` + `reason` decode
/// reconstruct the SAME unsigned claim and the recomputed CID byte-matches the
/// published rkey, WD-24). `reason == None` emits NO `reason` field (the ADR-015
/// wire-optional empty-reason edge — the empty-reason fixture, CT-6).
///
/// Returns `(record, peer_pubkey_hex)` so the caller wires the verify seam.
/// Test-support is the only place this construction is acceptable; the production
/// populate path is `peer pull`.
pub fn build_verifiable_peer_counter_record(
    peer_did: &str,
    peer_seed: [u8; 32],
    target_cid: &str,
    reason: Option<&str>,
) -> (FakePeerRecord, String) {
    use claim_domain::{canonicalize, compute_cid, sign, SigningKey, VerifyingKey};
    use ed25519_dalek::SigningKey as DalekSigningKey;

    let dalek_sk = DalekSigningKey::from_bytes(&peer_seed);
    let dalek_vk = dalek_sk.verifying_key();
    let signing_key = SigningKey(dalek_sk.to_bytes().to_vec());
    let pubkey_hex = hex_lower(&VerifyingKey(dalek_vk.to_bytes().to_vec()).0);

    // A counter is a claim whose `references[]` carries a `Counters` entry whose
    // `cid` is the target. Subject/predicate/object identify the counter's own
    // assertion; what makes it a COUNTER is the `counters` reference (ADR-015).
    let confidence_wrapper: claim_domain::Confidence =
        serde_json::from_value(serde_json::json!(0.40)).expect("confidence value is well-formed");
    let unsigned = claim_domain::UnsignedClaim {
        subject: "github:rust-lang/cargo".to_string(),
        predicate: "embodiesPhilosophy".to_string(),
        object: "org.openlore.philosophy.dependency-pinning".to_string(),
        evidence: vec!["https://example.test/counter".to_string()],
        confidence: confidence_wrapper,
        author_did: claim_domain::Did(format!("{peer_did}#org.openlore.application")),
        composed_at: "2026-05-22T09:18:44Z".to_string(),
        references: vec![claim_domain::ClaimReference {
            ref_type: claim_domain::ReferenceType::Counters,
            cid: claim_domain::Cid(target_cid.to_string()),
        }],
        reason: reason.map(|r| r.to_string()),
    };

    let canonical = canonicalize(&unsigned).expect("canonicalize peer counter claim");
    let cid = compute_cid(&canonical);
    let signature = sign(&cid, &signing_key).expect("sign peer counter claim");
    let sig_b64 = base64url_no_pad(&signature.signature_bytes);

    // The wire JSON body MUST mirror the unsigned claim so the pull pipeline's
    // decode (`parse_references` + `reason`) reconstructs the SAME unsigned claim
    // and the recomputed CID byte-matches the published rkey. `reason == None`
    // omits the field entirely (the wire-optional empty-reason edge).
    let mut body = serde_json::json!({
        "subject": "github:rust-lang/cargo",
        "predicate": "embodiesPhilosophy",
        "object": "org.openlore.philosophy.dependency-pinning",
        "evidence": ["https://example.test/counter"],
        "confidence": 0.40,
        "author": format!("{peer_did}#org.openlore.application"),
        "composedAt": "2026-05-22T09:18:44Z",
        "references": [{ "type": "counters", "cid": target_cid }],
        "signature": {
            "kid": format!("{peer_did}#org.openlore.application"),
            "alg": "EdDSA",
            "sig": sig_b64,
        }
    });
    if let Some(reason) = reason {
        body["reason"] = serde_json::json!(reason);
    }

    (FakePeerRecord::claim(cid.0, body), pubkey_hex)
}

/// Run `openlore <args>` with the network disabled (no PDS/peer endpoint
/// reachable), so a read-only LOCAL explorer command must still succeed.
/// Proves the local-first guardrail (I-GRAPH-7 / WD-79 / WD-92; extends
/// slice-01 KPI-5 / I-9): scoring/traversal/dimension reads touch only the
/// local store and open no socket.
///
/// Network-disable seam: mirror [`run_openlore`] (clean env + the slice-01
/// stub seams `OPENLORE_HOME` / `OPENLORE_DID` / `OPENLORE_KEY_SEED_HEX`) but
/// deliberately do NOT export `OPENLORE_PDS_ENDPOINT` nor any per-peer
/// `OPENLORE_PEER_PDS_ENDPOINT_<did>` resolver var. With `OPENLORE_PDS_ENDPOINT`
/// absent the composition root binds the no-network PdsPort adapter (see
/// `wiring::Wiring::production`), and with no peer resolver endpoint there is no
/// reachable peer either — there is genuinely NO network endpoint to dial.
/// A read-only LOCAL explorer (`--object`/`--contributor`/`--traverse`/
/// `--weighted`) reads only the seeded DuckDB and so still succeeds; any verb
/// that DID need the network would fail to reach it, proving the read path
/// opened no socket. Pair with [`assert_no_pds_call_was_made`] for the
/// no-outbound-call half of the universe.
///
/// `env_clear()` plus the explicit allow-list is what makes the disable real:
/// the parent's `OPENLORE_PDS_ENDPOINT` (if any) is dropped, so the subprocess
/// cannot inherit a live endpoint behind the test's back.
pub fn run_openlore_network_disabled(env: &TestEnv, args: &[&str]) -> CliOutcome {
    let bin = assert_cmd::cargo::cargo_bin("openlore");
    let output = Command::new(&bin)
        .args(args)
        .env_clear()
        .env("OPENLORE_HOME", &env.home)
        .env("OPENLORE_DID", env.identity.author_did())
        .env("OPENLORE_KEY_SEED_HEX", &env.identity.seed_hex)
        // Network disabled: OPENLORE_PDS_ENDPOINT and every per-peer resolver
        // endpoint are intentionally OMITTED. No reachable PDS / peer.
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

/// Universe-bound (Gate 4 `weight_and_bucket_never_persisted`): assert that
/// after a weighted query NO `adherence_weight` and NO `weight_bucket` label
/// appears in any DuckDB table, any on-disk `<cid>.json` artifact, or any
/// record. Scans every table + every artifact for the forbidden substrings
/// (`adherence_weight`, and the bucket labels `STRONG` / `MODERATE` / `SPARSE`
/// in a persisted position), extending the slice-01/03 confidence-bucket
/// no-persist scan. Port-exposed names:
/// `storage.duckdb.no_weight_or_bucket_column`,
/// `storage.local_claim_store.no_weight_or_bucket_string`.
///
/// SCAFFOLD: true (slice-04) — DELIVER materializes the table + artifact scan
/// (raw `duckdb::Connection` is acceptable in test-support; production never
/// has a persist path for these by design, WD-89).
pub fn assert_weight_not_persisted(env: &TestEnv) {
    // The forbidden weight/bucket vocabulary (WD-72/WD-89): the aggregate
    // weight column name and the slice-04 weight-bucket labels. These are a
    // DISPLAY-ONLY aggregate recomputed at query time — none may appear in any
    // persisted position (a table name, a column name, a stored cell, or an
    // on-disk artifact). Matched case-INSENSITIVELY so a `STRONG`/`Strong`/
    // `strong` leak in any casing is still caught.
    const FORBIDDEN: &[&str] = &[
        "adherence_weight",
        "weight_bucket",
        "strong",
        "moderate",
        "sparse",
    ];

    // -- 1. DuckDB scan: enumerate EVERY table dynamically (robust to schema
    // drift), then scan each table's COLUMN NAMES and every ROW's serialized
    // cell text for a forbidden token. Test-support is the only place a raw
    // `duckdb::Connection` + raw SQL is acceptable (production reads go through
    // StoragePort). --
    let db_path = env.duckdb_path();
    assert!(
        db_path.exists(),
        "storage.duckdb.no_weight_or_bucket_column: expected DuckDB to exist at {} after a \
         weighted query; file missing",
        db_path.display()
    );
    let conn = duckdb::Connection::open(&db_path).unwrap_or_else(|err| {
        panic!(
            "open DuckDB at {} for the weight-not-persisted scan: {err}",
            db_path.display()
        )
    });

    let tables: Vec<String> = {
        let mut stmt = conn
            .prepare("SELECT table_name FROM information_schema.tables WHERE table_schema = 'main'")
            .expect("prepare information_schema.tables query");
        let rows = stmt
            .query_map([], |r| r.get::<_, String>(0))
            .expect("query main-schema table names");
        rows.map(|r| r.expect("read table name")).collect()
    };
    assert!(
        !tables.is_empty(),
        "storage.duckdb.no_weight_or_bucket_column: the main schema reported ZERO tables — the \
         scan would be vacuous (expected at least the claims/peer_claims index tables)"
    );

    for table in &tables {
        // A table itself must never be NAMED for a weight/bucket.
        assert_forbidden_token_absent(
            table,
            FORBIDDEN,
            &format!(
                "storage.duckdb.no_weight_or_bucket_column: a DuckDB table is named {table:?}"
            ),
        );

        // Cast EVERY column of EVERY row to text and concatenate, so the scan
        // covers both column NAMES (information_schema) and stored cell VALUES
        // without hard-coding any column list.
        let columns: Vec<String> = {
            let mut stmt = conn
                .prepare(
                    "SELECT column_name FROM information_schema.columns \
                     WHERE table_schema = 'main' AND table_name = ?",
                )
                .expect("prepare information_schema.columns query");
            let rows = stmt
                .query_map(duckdb::params![table], |r| r.get::<_, String>(0))
                .expect("query column names");
            rows.map(|r| r.expect("read column name")).collect()
        };
        for column in &columns {
            assert_forbidden_token_absent(
                column,
                FORBIDDEN,
                &format!(
                    "storage.duckdb.no_weight_or_bucket_column: table {table:?} has a column \
                     named {column:?}"
                ),
            );
        }

        // Concatenate every column cast to VARCHAR for each row; scan the text.
        let cast_list = columns
            .iter()
            .map(|c| format!("COALESCE(CAST(\"{c}\" AS VARCHAR), '')"))
            .collect::<Vec<_>>()
            .join(" || ' ' || ");
        if cast_list.is_empty() {
            continue;
        }
        let sql = format!("SELECT {cast_list} FROM \"{table}\"");
        let mut stmt = conn
            .prepare(&sql)
            .unwrap_or_else(|err| panic!("prepare row scan for table {table}: {err}"));
        let cells = stmt
            .query_map([], |r| r.get::<_, String>(0))
            .unwrap_or_else(|err| panic!("scan rows for table {table}: {err}"));
        for cell in cells {
            let text = cell.unwrap_or_else(|err| panic!("read row text for table {table}: {err}"));
            assert_forbidden_token_absent(
                &text,
                FORBIDDEN,
                &format!(
                    "storage.duckdb.no_weight_or_bucket_column: a stored row in table {table:?} \
                     contains a persisted weight/bucket token"
                ),
            );
        }
    }

    // -- 2. On-disk artifact scan: every `<cid>.json` under the own-claims dir
    // AND every peer-claims partition. The on-disk artifacts are the
    // authoritative claim store; none may carry a persisted weight/bucket. --
    for artifact in collect_claim_artifacts(env) {
        let contents = std::fs::read_to_string(&artifact).unwrap_or_else(|err| {
            panic!(
                "read claim artifact {} for weight scan: {err}",
                artifact.display()
            )
        });
        assert_forbidden_token_absent(
            &contents,
            FORBIDDEN,
            &format!(
                "storage.local_claim_store.no_weight_or_bucket_string: on-disk artifact {} \
                 contains a persisted weight/bucket token",
                artifact.display()
            ),
        );
    }
}

/// Assert NONE of the case-insensitive `forbidden` tokens appears in `haystack`.
/// `context` names the port-exposed slot being scanned for a precise failure.
fn assert_forbidden_token_absent(haystack: &str, forbidden: &[&str], context: &str) {
    let lowered = haystack.to_ascii_lowercase();
    for token in forbidden {
        assert!(
            !lowered.contains(&token.to_ascii_lowercase()),
            "{context}: found forbidden weight/bucket token {token:?} (Gate 4 \
             weight_and_bucket_never_persisted — weights are display-only, recomputed at query \
             time; WD-72/WD-89);\n--- scanned text ---\n{haystack}"
        );
    }
}

/// Recursively collect every `*.json` claim artifact under the local
/// share dir (`{home}/.local/share/openlore`), covering the own-claims dir
/// (`claims/`) AND every peer-claims partition (`peer_claims/<encoded_did>/`).
/// Used by [`assert_weight_not_persisted`] for the on-disk half of the scan.
fn collect_claim_artifacts(env: &TestEnv) -> Vec<PathBuf> {
    let share_root = env.home.join(".local").join("share").join("openlore");
    let mut out = Vec::new();
    collect_json_files(&share_root, &mut out);
    out
}

/// Depth-first walk of `dir`, appending every `*.json` file path to `acc`.
/// Absent directories are treated as empty (the strongest "no artifact" form).
fn collect_json_files(dir: &std::path::Path, acc: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.is_dir() {
            collect_json_files(&path, acc);
        } else if path.extension().is_some_and(|ext| ext == "json") {
            acc.push(path);
        }
    }
}

/// Universe-bound (Gate 1 `scoring_aggregate_preserves_attribution`): assert
/// the `--weighted --explain <subject>` breakdown for `subject` decomposes the
/// aggregate into EXACTLY the expected per-author per-cid contributions —
/// every contributing claim enumerated with its author DID + cid, no claim
/// merged into a faceless aggregate. `expected` pairs each contributing
/// (author_did, claim_cid) the breakdown MUST name. Port-exposed names:
/// `cli.graph_query.explain.contributions[subject]` (author+cid tuples).
///
/// SCAFFOLD: true (slice-04) — the anti-merging-in-aggregates behavioral gate
/// (extends slice-03 `assert_no_merged_rows_in_federated_output` to the
/// weighted aggregate surface).
pub fn assert_weight_decomposes_to_per_author(
    outcome: &CliOutcome,
    subject: &str,
    expected: &[(&str, &str)],
) {
    // SCAFFOLD: true (slice-04)
    let _ = (outcome, subject, expected);
    todo!(
        "DELIVER (slice-04): parse the --explain breakdown for `subject` into its (author_did, cid) \
         contribution tuples and assert it names EXACTLY the expected contributing claims — every \
         claim attributed, none merged into a faceless aggregate (Gate 1 \
         scoring_aggregate_preserves_attribution; WD-73/I-GRAPH-2)"
    )
}

/// Universe-bound (Gate 3 `sparse_renders_sparse`): assert the weighted output
/// renders `subject` with the `[SPARSE]` bucket AND the "based on N claims by M
/// authors" honesty line AND the lead-not-conclusion advice, and that NO
/// confidence was manufactured from the thin evidence. Port-exposed names:
/// `cli.graph_query.bucket[subject]`,
/// `cli.graph_query.sparse_honesty_line_present`.
///
/// SCAFFOLD: true (slice-04) — the load-bearing sparse-honesty gate (KPI-GRAPH-4).
pub fn assert_sparse_rendered_as_sparse(
    outcome: &CliOutcome,
    subject: &str,
    claim_count: usize,
    author_count: usize,
) {
    // SCAFFOLD: true (slice-04)
    let _ = (outcome, subject, claim_count, author_count);
    todo!(
        "DELIVER (slice-04): assert `subject` renders [SPARSE] with the verbatim 'based on \
         {{claim_count}} claim(s) by {{author_count}} author(s)' honesty line + lead-not-conclusion \
         advice, and NO confidence is manufactured from the thin evidence (Gate 3 \
         sparse_renders_sparse; WD-74/WD-90; KPI-GRAPH-4)"
    )
}

/// Universe-bound (Gate 2 `weight_equals_formula`): assert the
/// `--weighted --explain <subject>` running sum reproduces the displayed
/// adherence weight by hand — the breakdown's per-claim subtotals sum (within
/// f64 EPS) to the weight shown for `subject` in the ranked view. Port-exposed
/// names: `cli.graph_query.explain.running_sum[subject]`,
/// `cli.graph_query.weighted.displayed_weight[subject]`.
///
/// SCAFFOLD: true (slice-04) — the scoring-transparency reproduce-by-hand gate
/// (KPI-GRAPH-3); the layer-3 rendering counterpart of `scoring_core.rs` SC-1.
pub fn assert_explain_sums_to_weight(outcome: &CliOutcome, subject: &str) {
    // SCAFFOLD: true (slice-04)
    let _ = (outcome, subject);
    todo!(
        "DELIVER (slice-04): parse the --explain per-claim subtotals + the displayed weight for \
         `subject` from stdout and assert the running sum equals the displayed weight within f64 \
         EPS — the weight is reproducible by hand (Gate 2 weight_equals_formula; KPI-GRAPH-3)"
    )
}

/// Universe-bound (Gate 5 `traversal_invents_no_edges`): assert EVERY edge in a
/// `--traverse` output maps to a backing signed claim — for each displayed edge
/// cid, a `graph query --subject <project>` lookup resolves it to an existing
/// claim, and every edge carries the author DID of its backing claim. No
/// displayed edge lacks a backing signed claim (no fabrication/interpolation).
/// Port-exposed names: `cli.graph_query.traverse.edge_cids`,
/// `cli.graph_query.traverse.edge_cid_resolvable[cid]`.
///
/// SCAFFOLD: true (slice-04) — the auditability gate (WD-76/WD-91); the
/// traversal counterpart of the federated anti-merging assertion.
pub fn assert_every_edge_has_backing_claim(env: &TestEnv, traversal: &CliOutcome) {
    // Parse the `--object --traverse` tree into its edges. The renderer groups
    // edges under a `  project: <subject>` header; under each header every edge
    // is a `    author_did: <did>` line immediately followed by a
    // `    claim_cid:  <cid>` line (render.rs `render_one_traversal_edge`). We
    // recover (project_subject, author_did, claim_cid) for each edge so the
    // resolution probe knows which `--subject` lookup to run.
    let edges = parse_traversal_edges(&traversal.stdout);

    // Gate 5 is vacuously true over zero edges — but the rachel-spans-two-projects
    // fixture genuinely seeds cross-project edges, so an empty parse means the
    // tree did not render (or the format drifted), NOT "no edges to verify".
    assert!(
        !edges.is_empty(),
        "cli.graph_query.traverse.edge_cids: expected the traversal to render at least one edge \
         (the rachel-spans-two-projects fixture seeds cross-project edges); parsed none — the \
         backing-claim check would be vacuous;\n--- traversal stdout ---\n{}",
        traversal.stdout
    );

    // Resolve each project subject AT MOST ONCE. The fixture's claims are all
    // subscribed-PEER claims, so the bare own-only `--subject` honestly returns
    // empty (GQE-3 default-off) — the resolution probe must widen to the same
    // federated scope the traverse walk read under (WD-87), matching the GQE-23
    // depth-3 Gate-5 probe. The cache keeps the helper O(distinct projects)
    // subprocess calls instead of O(edges).
    let mut resolved: std::collections::HashMap<String, CliOutcome> =
        std::collections::HashMap::new();

    for edge in &edges {
        let lookup = resolved
            .entry(edge.project_subject.clone())
            .or_insert_with(|| {
                run_openlore(
                    env,
                    &[
                        "graph",
                        "query",
                        "--subject",
                        &edge.project_subject,
                        "--federated",
                    ],
                )
            });
        assert_eq!(
            lookup.status,
            0,
            "cli.graph_query.traverse.edge_cid_resolvable[{cid}]: the Gate-5 resolution probe \
             `graph query --subject {subject} --federated` must exit 0;\n--- stdout ---\n{}\n\
             --- stderr ---\n{}",
            lookup.stdout,
            lookup.stderr,
            cid = edge.claim_cid,
            subject = edge.project_subject,
        );

        // Gate 5 (traversal invents no edges): the rendered edge cid resolves to
        // a real signed claim — its `claim_cid` appears as an existing row in the
        // independent `--subject` lookup. A fabricated/interpolated edge would
        // carry a cid no claim row backs, and this `contains` would fail.
        assert!(
            lookup.stdout.contains(edge.claim_cid.as_str()),
            "cli.graph_query.traverse.edge_cid_resolvable[{cid}]: the traversal edge cid must \
             resolve to a real signed claim via `graph query --subject {subject} --federated` — \
             every displayed edge traces to a signed claim, none is fabricated (Gate 5 \
             traversal_invents_no_edges; WD-76/WD-91);\n--- subject-lookup stdout ---\n{}\n\
             --- traversal stdout ---\n{}",
            lookup.stdout,
            traversal.stdout,
            cid = edge.claim_cid,
            subject = edge.project_subject,
        );

        // Anti-merging (I-GRAPH-2 / WD-73): the edge carries the author DID of its
        // backing claim, and that same author is attributed in the resolved
        // subject lookup — the edge maps to THIS author's signed claim, never an
        // authorless aggregate.
        assert!(
            lookup.stdout.contains(edge.author_did.as_str()),
            "cli.graph_query.traverse.edge_cids: the traversal edge for cid {cid} carries author \
             DID {did}, which must also be attributed in the backing `--subject {subject} \
             --federated` lookup — the edge maps to that author's signed claim (anti-merging \
             I-GRAPH-2/WD-73);\n--- subject-lookup stdout ---\n{}\n--- traversal stdout ---\n{}",
            lookup.stdout,
            traversal.stdout,
            cid = edge.claim_cid,
            did = edge.author_did,
            subject = edge.project_subject,
        );
    }
}

/// One traversal edge recovered from a `--object --traverse` tree: the project
/// subject it groups under, the author DID of its backing claim, and the
/// backing signed-claim cid. The triple
/// [`assert_every_edge_has_backing_claim`] resolves against the store.
#[derive(Debug, Clone)]
struct TraversalEdge {
    project_subject: String,
    author_did: String,
    claim_cid: String,
}

/// Parse the edges out of a `graph query --object <philosophy> --traverse`
/// rendering (render.rs `render_traversal_from_seed`). The tree groups edges
/// under `  project: <subject>` headers; under each, every edge is an
/// `    author_did: <did>` line immediately followed by a `    claim_cid: <cid>`
/// line. Returns one [`TraversalEdge`] per rendered edge, in render order.
fn parse_traversal_edges(stdout: &str) -> Vec<TraversalEdge> {
    let mut edges = Vec::new();
    let mut current_project: Option<String> = None;
    let mut pending_author: Option<String> = None;

    for line in stdout.lines() {
        let trimmed = line.trim_start();
        if let Some(subject) = trimmed.strip_prefix("project:") {
            current_project = Some(subject.trim().to_string());
            pending_author = None;
        } else if let Some(did) = trimmed.strip_prefix("author_did:") {
            pending_author = Some(did.trim().to_string());
        } else if let Some(cid) = trimmed.strip_prefix("claim_cid:") {
            if let (Some(project_subject), Some(author_did)) =
                (current_project.clone(), pending_author.take())
            {
                edges.push(TraversalEdge {
                    project_subject,
                    author_did,
                    claim_cid: cid.trim().to_string(),
                });
            }
        }
    }
    edges
}

// =============================================================================
// Slice-05 (appview-search) support extensions — SCAFFOLD: true
// =============================================================================
//
// SCAFFOLD: true (slice-05)
//
// Slice-05 introduces the FIRST network service + the FIRST cross-process
// boundary + the FIRST adversarial-input external boundary. Unlike slice-04 (no
// new fake), it needs hermetic doubles for the two new external surfaces (per
// the Architecture of Reference: driven-external -> fake):
//   - `FakeIngestSource`: a bounded fixture network ingest source hosting a
//     `listRecords`-style enumeration, carrying the adversarial set (unsigned /
//     tampered-signature / cid-mismatch) + valid signed records (DD-AV-2/DD-AV-11).
//   - a fixture PLC DID-document resolver carrying a REAL `z6Mk...` (a known test
//     keypair) so the ADR-026 decode runs the REAL decode path (the AV-4 gold).
// The B1 CLI<->indexer boundary is exercised against a REAL `openlore-indexer
// serve` over LOCALHOST bound to an EPHEMERAL `:0` port (read back; parallel-safe,
// DEVOPS open-q 8) — the production composition root (Pillar 3). The funnel
// (AV-19/AV-22) reuses the slice-03 `PeerPds` + `peer add`/`peer pull` verbatim.
//
// Per DD-AV-10 (symmetric with slice-04 DD-GRAPH-10): the load-bearing assertion
// helpers carry a `todo!()` body with a precise contract docstring; the
// SIGNATURES are correct NOW so every slice-05 test file compiles and reaches its
// own `todo!()` (RED, not BROKEN) AFTER DELIVER's bootstrap step lands the
// production crates + the `fixtures_ingest.rs` recipes + the harness bodies
// (DD-AV-13). The universe entries each helper names MUST be port-exposed (CLI
// stdout substrings, indexed-row author_did sets, ingest counters, exit codes,
// the openlore.duckdb byte-unchanged guard) — NEVER internal store/compose struct
// fields (Mandate 8).

/// A named network-index fixture: the precondition corpus a slice-05 scenario
/// seeds into the REAL `index.duckdb` (via the ingest harness) before exercising
/// `openlore search` / `openlore-indexer`. Each variant maps to a worked example
/// in user-stories.md / data-models.md.
///
/// SCAFFOLD: true (slice-05) — DELIVER fills each variant's concrete seeding
/// recipe (which `RawRecordSpec`s the `FakeIngestSource` hosts, which authors are
/// followed vs unfollowed, which PLC `z6Mk` keys resolve) in `fixtures_ingest.rs`.
#[derive(Debug, Clone)]
pub enum NetworkIndexFixture {
    /// US-AV-002 Example 1: 12 verified reproducible-builds claims across 7
    /// subjects by 9 authors, incl. Priya (did:plc:priya-test, UNFOLLOWED, bazel
    /// 0.82) + Rachel (did:plc:rachel-test, SUBSCRIBED peer, nixpkgs 0.88).
    ReproducibleBuildsNineAuthorsUnfollowed,
    /// US-AV-002 Example 2 / AVC-5: github:denoland/deno + dependency-pinning by
    /// two UNFOLLOWED authors (Priya 0.70, Sven 0.65) — the identical-content
    /// zero-merge fixture.
    DenoDependencyPinningTwoUnfollowedAuthors,
    /// US-AV-001 walking-skeleton beat-1: ONE valid signed Priya claim (bazel,
    /// reproducible-builds, 0.82) + a resolvable real-z6Mk DID-doc.
    SingleVerifiedPriyaClaim,
    /// US-AV-001 / AV-3 release gate: the adversarial set (unsigned +
    /// tampered-signature + cid-mismatch) PLUS one valid signed record, on the
    /// same author surface — the verified-before-index reject fixture.
    AdversarialSetPlusOneValid,
    /// US-AV-003 Example 1: did:plc:priya-test authors 8 verified claims across 6
    /// subjects (bazel x2, buck2, nixpkgs, pants, please, ninja); Maria unfollowed.
    PriyaEightClaimsSixSubjects,
    /// US-AV-003 Example 2: github:bazelbuild/bazel with verified claims from 5
    /// DISTINCT network authors (the subject-survey anti-merging fixture).
    BazelFiveDistinctAuthors,
    /// US-AV-003 Example 4 / US-AV-005 Ex2: a corpus including Rachel
    /// (did:plc:rachel-test) whom the user ALREADY follows (subscribed-peer label).
    IncludesAlreadyFollowedRachel,
    /// US-AV-005 Example 1 funnel: Priya's verified network claim (unfollowed) +
    /// a slice-03 `PeerPds` hosting her claims for the post-`peer add` pull.
    PriyaDiscoverableAndPullable,
    /// US-AV-002 / OD-AV-7: a claim C + a later indexed claim K that references C
    /// with ref_type=counters (the counter-shown-not-applied fixture).
    CounteredClaimPlusCounter,
    /// US-AV-004 Example 1: Priya's verified bazel/reproducible-builds claim with
    /// a known cid (bafy...k2) for the `--show` inspect fixture.
    PriyaShowableVerifiedRecord,
}

/// A handle to a REAL `openlore-indexer serve` running on a localhost EPHEMERAL
/// port (bound `:0`, read back). Owns the child process (and its tokio runtime
/// inside the spawned binary); the CLI's `[appview] indexer_url` is pointed at
/// `indexer_url()`. Mirrors the slice-01 `FakePds` / slice-03 `PeerPds`
/// runtime-ownership pattern (the server is released on drop).
///
/// SCAFFOLD: true (slice-05) — DELIVER spawns `openlore-indexer serve` over the
/// seeded `index.duckdb` on an ephemeral port (DEVOPS open-q 8 parallel-safety),
/// reads back the bound port, and exposes `indexer_url()` for the CLI to query.
pub struct IndexerHandle {
    /// The `http://127.0.0.1:<ephemeral-port>` URL the CLI's `indexer_url` points
    /// at — read back from the spawned `serve` process's
    /// `indexer.serve.listening` event.
    url: String,
    /// The live `openlore-indexer serve` child process. Killed on drop (RAII
    /// per-scenario isolation), mirroring the `FakePds` / `PeerPds` pattern.
    child: std::process::Child,
    /// The ingest source kept alive for the serve process's startup gauntlet (the
    /// `ingest_source.probe()` requires a reachable source URL). The corpus is
    /// already ingested; serve only reads the index, but the wire→probe→use gate
    /// (ADR-009) probes every wired adapter. Held so the source's port stays bound
    /// for the serve process's lifetime; dropped (releasing the port) with the
    /// handle.
    _source: FakeIngestServer,
}

impl IndexerHandle {
    /// The `http://127.0.0.1:<ephemeral-port>` URL the CLI's `[appview]
    /// indexer_url` is pointed at.
    pub fn indexer_url(&self) -> String {
        self.url.clone()
    }
}

impl Drop for IndexerHandle {
    fn drop(&mut self) {
        // Kill the serve process so the bound port is released — RAII per-scenario
        // isolation (the ephemeral `:0` port keeps parallel scenarios disjoint).
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Map a [`NetworkIndexFixture`] to the corpus of `RawRecordSpec`s the
/// `FakeIngestServer` hosts. AV-8 (the walking skeleton) wires the headline
/// `ReproducibleBuildsNineAuthorsUnfollowed` corpus; other variants register the
/// same shape in later steps.
fn fixture_corpus(fixture: &NetworkIndexFixture) -> Vec<openlore_test_support::RawRecordSpec> {
    use openlore_test_support::*;
    match fixture {
        NetworkIndexFixture::ReproducibleBuildsNineAuthorsUnfollowed => {
            corpus_reproducible_builds_nine_authors()
        }
        NetworkIndexFixture::DenoDependencyPinningTwoUnfollowedAuthors => {
            corpus_deno_dependency_pinning_two_authors()
        }
        NetworkIndexFixture::SingleVerifiedPriyaClaim => vec![fixture_ingest_valid_signed()],
        NetworkIndexFixture::AdversarialSetPlusOneValid => {
            fixture_ingest_adversarial_set_plus_one_valid()
        }
        NetworkIndexFixture::PriyaEightClaimsSixSubjects => {
            corpus_priya_eight_claims_six_subjects()
        }
        NetworkIndexFixture::BazelFiveDistinctAuthors => corpus_bazel_five_distinct_authors(),
        NetworkIndexFixture::IncludesAlreadyFollowedRachel => {
            corpus_includes_already_followed_rachel()
        }
        NetworkIndexFixture::CounteredClaimPlusCounter => corpus_countered_claim_plus_counter(),
        other => panic!(
            "seed_network_index: corpus for fixture {other:?} not yet materialized (04-01 \
             wires only the AV-8 headline corpus; later steps add the rest)"
        ),
    }
}

/// US-AV-003 Example 4 / US-AV-005 Ex2 corpus: did:plc:rachel-test authors
/// several verified network claims across subjects. Used by AV-18 — Maria
/// ALREADY follows Rachel (a slice-03 `peer add`), so a `--contributor
/// github:rachel` search labels every one of Rachel's network rows
/// "(subscribed peer)" (resolved CLI-side against Maria's peer_subscriptions —
/// the index is per-user-neutral) and shows NO redundant follow affordance.
///
/// Built inline from the public `RawRecordSpec::valid` builder against
/// `RACHEL_DID` (the contributor query matches the indexed
/// `did:plc:rachel-test#org.openlore.application` author_did exactly), so the
/// trail is a substantive multi-claim survey (the subscribed-peer label + the
/// affordance-suppression must hold on EVERY row).
fn corpus_includes_already_followed_rachel() -> Vec<openlore_test_support::RawRecordSpec> {
    use openlore_test_support::{RawRecordSpec, RACHEL_DID};
    let entries = [
        (
            "github:NixOS/nixpkgs",
            "org.openlore.philosophy.reproducible-builds",
            0.88,
        ),
        (
            "github:NixOS/nixpkgs",
            "org.openlore.philosophy.dependency-pinning",
            0.81,
        ),
        (
            "github:rust-lang/cargo",
            "org.openlore.philosophy.dependency-pinning",
            0.91,
        ),
        (
            "github:guix/guix",
            "org.openlore.philosophy.reproducible-builds",
            0.76,
        ),
    ];
    entries
        .iter()
        .map(|(subject, object, conf)| RawRecordSpec::valid(RACHEL_DID, subject, object, *conf))
        .collect()
}

/// US-AV-002 / OD-AV-7 corpus (AV-25): claim C (Priya, bazel, reproducible-builds,
/// 0.82) PLUS a LATER indexed claim K (Sven) that REFERENCES C with
/// `ref_type=counters`. Both are VERIFIED (`RawRecordSpec::valid` runs the real
/// crypto). K asserts the SAME object as C so a `--object reproducible-builds`
/// search returns BOTH rows — the render reconstructs the
/// `countered-by <K.cid> (by <K.author_did>)` annotation from K's `references`
/// (pointing at C) + K's own `author_did`, and C is STILL present (shown, NEVER
/// applied — OD-AV-7 / I-AV-9; mirrors slice-04 WD-85).
///
/// K's reference target is C's PUBLISHED CID, computed by running C's spec through
/// the SAME real crypto (`into_raw_record().published_cid`) the ingest gate
/// recomputes — so the `indexed_claim_references.referenced_cid` K carries matches
/// the `indexed_claims.cid` C is indexed under (the same-store JOIN key).
fn corpus_countered_claim_plus_counter() -> Vec<openlore_test_support::RawRecordSpec> {
    use openlore_test_support::{RawRecordSpec, PRIYA_DID, SVEN_DID};
    let object = "org.openlore.philosophy.reproducible-builds";

    // Claim C — the countered claim (Priya, bazel). Verified; appears as a result.
    let c = RawRecordSpec::valid(PRIYA_DID, "github:bazelbuild/bazel", object, 0.82);

    // C's PUBLISHED CID (the same-store JOIN key K references) — computed by
    // running C's spec through the REAL crypto, exactly as the ingest gate does.
    let c_cid = c.clone().into_raw_record().published_cid;

    // Claim K — the countering claim (Sven). Verified; carries a typed
    // `Counters` reference to C's CID. Same object so it co-appears in the search
    // result set, giving the render K's author_did + cid to attribute the counter.
    let k = RawRecordSpec::valid(SVEN_DID, "github:bazelbuild/bazel", object, 0.40)
        .with_reference(claim_domain::ReferenceType::Counters, &c_cid.0);

    vec![c, k]
}

/// The distinct author DIDs present in a corpus, each paired with its fixture
/// keypair public-key hex (the slice-03 pubkey seam the indexer verifies against).
fn corpus_pubkey_seams(specs: &[openlore_test_support::RawRecordSpec]) -> Vec<(String, String)> {
    let mut seen: HashSet<String> = HashSet::new();
    let mut seams: Vec<(String, String)> = Vec::new();
    for spec in specs {
        let did = spec.author_did.clone();
        if seen.insert(did.clone()) {
            let kp = openlore_test_support::FixtureKeypair::for_did(&did);
            seams.push((did, hex_lower(&kp.verifying_key.0)));
        }
    }
    seams
}

/// Seed the network index for a scenario: host the fixture's records on a
/// `FakeIngestServer`, run a REAL `openlore-indexer ingest` pass to populate a
/// REAL `index.duckdb`, then start a REAL `openlore-indexer serve` over an
/// EPHEMERAL localhost port (`:0`, read back from the `indexer.serve.listening`
/// event), returning the [`IndexerHandle`]. The slice-05 precondition seam (no
/// live network). Point the CLI's `indexer_url` at `handle.indexer_url()` via
/// [`run_openlore_search`].
pub fn seed_network_index(env: &TestEnv, fixture: NetworkIndexFixture) -> IndexerHandle {
    seed_network_index_from_specs(env, fixture_corpus(&fixture))
}

/// Seed the network index from an EXPLICIT corpus of `RawRecordSpec`s (the
/// fixture-agnostic core of [`seed_network_index`]): host the records on a
/// `FakeIngestServer`, run a REAL `openlore-indexer ingest` pass into the REAL
/// `index.duckdb`, then spawn a REAL `openlore-indexer serve` over the SAME index
/// on an ephemeral localhost port. Used directly by AV-28's SECOND ingest pass to
/// re-seed the SAME index with a GROWN corpus (the original claims + two more
/// matching ones) and re-serve it, proving the share link re-runs the QUERY
/// against the CURRENT index (US-AV-006 Ex4 / I-AV-8).
pub fn seed_network_index_from_specs(
    env: &TestEnv,
    specs: Vec<openlore_test_support::RawRecordSpec>,
) -> IndexerHandle {
    let seams = corpus_pubkey_seams(&specs);
    let seam_refs: Vec<(&str, &str)> = seams
        .iter()
        .map(|(d, k)| (d.as_str(), k.as_str()))
        .collect();

    // 1. Host the records + run the one-shot ingest pass into the REAL index.duckdb.
    // The ingest gate de-dups by CID (INSERT OR REPLACE; adapter-index-store
    // `upsert_is_idempotent_by_cid`), so re-ingesting a GROWN corpus (the original
    // records PLUS new ones) idempotently replaces the originals and ADDS the new —
    // the index ends with exactly the union (AV-28's second ingest grows the set).
    let source = FakeIngestServer::start(specs);
    let ingest =
        run_openlore_indexer_with_source(env, &["ingest"], source.source_url(), &seam_refs);
    assert_eq!(
        ingest.status, 0,
        "seed_network_index: `openlore-indexer ingest` must exit 0. stdout: {} stderr: {}",
        ingest.stdout, ingest.stderr
    );

    // 2. Spawn a long-running `openlore-indexer serve` over the SAME index.duckdb,
    // bound to an ephemeral localhost port. The source is kept alive (in the
    // handle) so the serve startup gauntlet's `ingest_source.probe()` passes (the
    // wire→probe→use gate probes every wired adapter, ADR-009). Read the bound port
    // back from the `indexer.serve.listening` event the serve process prints.
    spawn_indexer_serve(env, source)
}

/// AV-28 (US-AV-006 Ex4 / I-AV-8) — run a SECOND `openlore-indexer ingest` pass
/// that adds two MORE verified matching claims to the SAME `index.duckdb`, then
/// re-serve it; returns the NEW [`IndexerHandle`] the re-opened link queries.
///
/// `current` is the handle from the FIRST [`seed_network_index`]; it is consumed
/// (taken by value) so its serve process is KILLED and its index file lock is
/// RELEASED before the second ingest opens the SAME file (DuckDB takes an
/// exclusive lock per file — the serve handle must let go first). `more_specs` is
/// the additional matching corpus; it is unioned with the headline
/// reproducible-builds corpus (the SAME corpus the FIRST seed used) so the second
/// ingest produces the original rows PLUS the new ones (the ingest gate de-dups by
/// CID, so the originals are idempotently replaced, never duplicated).
///
/// The re-spawned serve opens a FRESH connection over the now-grown index, so the
/// re-opened share link — which re-runs the QUERY against the CURRENT index (the
/// 05-13 live resolver) — sees the two new rows. The link encoded the QUERY, not a
/// frozen snapshot (KPI-AV-6 / I-AV-8).
///
/// Universe (port-exposed): the re-served index holds the union corpus; the
/// re-opened link resolves to that CURRENT set (the count grows by the number of
/// new matching claims), each new row attributed + `[verified]`, no merged view.
pub fn ingest_more_matching_claims_and_respawn(
    env: &TestEnv,
    current: IndexerHandle,
    more_specs: Vec<openlore_test_support::RawRecordSpec>,
) -> IndexerHandle {
    // Drop the FIRST serve handle FIRST: killing its serve process releases the
    // exclusive DuckDB lock on index.duckdb (and frees the old FakeIngestServer
    // port) so the SECOND ingest pass can open the SAME file for writing. Without
    // this, the second ingest's `Connection::open` would conflict with the live
    // serve handle's open connection (DuckDB is single-writer per file).
    drop(current);

    // The SECOND ingest's hosted corpus = the headline reproducible-builds corpus
    // (the SAME set the FIRST seed used) UNIONED with the new matching claims. The
    // ingest gate de-dups by CID, so the originals are idempotently re-indexed
    // (not duplicated) and the new claims are ADDED — the index grows by exactly
    // `more_specs.len()`.
    let mut union = openlore_test_support::corpus_reproducible_builds_nine_authors();
    union.extend(more_specs);

    // Re-ingest into the SAME index.duckdb (under the same `env.home`) and re-serve.
    seed_network_index_from_specs(env, union)
}

/// AV-28 — the two MORE verified matching claims the SECOND ingest pass adds: two
/// NEW distinct authors each asserting the headline object
/// (`org.openlore.philosophy.reproducible-builds`), so a re-opened
/// `--object reproducible-builds` link grows from 9 attributed rows to 11. Both
/// are `RawRecordSpec::valid` (the REAL crypto runs; they pass the verify-before-
/// index gate), so each new row renders attributed + `[verified]`.
pub fn av28_two_more_matching_claims() -> Vec<openlore_test_support::RawRecordSpec> {
    use openlore_test_support::RawRecordSpec;
    let object = "org.openlore.philosophy.reproducible-builds";
    vec![
        RawRecordSpec::valid(
            "did:plc:author10-test",
            "github:void/voidlinux",
            object,
            0.69,
        ),
        RawRecordSpec::valid(
            "did:plc:author11-test",
            "github:alpine/aports",
            object,
            0.73,
        ),
    ]
}

/// Spawn `openlore-indexer serve` over `env`'s `index.duckdb` on an ephemeral
/// localhost port, returning an [`IndexerHandle`] whose `indexer_url()` is read
/// back from the serve process's `indexer.serve.listening` stdout event. `source`
/// is kept alive for the serve process's startup probe gauntlet.
fn spawn_indexer_serve(env: &TestEnv, source: FakeIngestServer) -> IndexerHandle {
    use std::io::{BufRead, BufReader};

    let bin = assert_cmd::cargo::cargo_bin("openlore-indexer");
    let mut child = Command::new(&bin)
        .arg("serve")
        .env_clear()
        .env("OPENLORE_HOME", &env.home)
        .env("OPENLORE_INDEXER_INDEX_PATH", index_duckdb_path(env))
        // Point serve at the reachable source so the wire→probe→use gauntlet's
        // ingest-source probe passes (serve reads the index; it does not re-ingest
        // here — the corpus is already indexed).
        .env("OPENLORE_INDEXER_SOURCE_URL", source.source_url())
        // Bind an ephemeral localhost port (parallel-safe; DEVOPS open-q 8).
        .env("OPENLORE_INDEXER_LISTEN_ADDR", "127.0.0.1:0")
        .env("PATH", std::env::var("PATH").unwrap_or_default())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap_or_else(|e| panic!("spawn openlore-indexer serve at {bin:?}: {e}"));

    // Read the bound-address event off stdout. The serve process prints
    // `{"event":"indexer.serve.listening","addr":"127.0.0.1:<port>"}` once bound.
    let stdout = child
        .stdout
        .take()
        .expect("openlore-indexer serve: stdout pipe");
    let mut reader = BufReader::new(stdout);
    let mut addr: Option<String> = None;
    for _ in 0..50 {
        let mut line = String::new();
        match reader.read_line(&mut line) {
            Ok(0) => break, // EOF — serve exited before binding
            Ok(_) => {
                if let Ok(event) = serde_json::from_str::<serde_json::Value>(line.trim()) {
                    if event["event"] == "indexer.serve.listening" {
                        if let Some(a) = event["addr"].as_str() {
                            addr = Some(a.to_string());
                            break;
                        }
                    }
                }
            }
            Err(_) => break,
        }
    }
    let addr = addr.unwrap_or_else(|| {
        let _ = child.kill();
        let mut err = String::new();
        if let Some(mut stderr) = child.stderr.take() {
            use std::io::Read;
            let _ = stderr.read_to_string(&mut err);
        }
        panic!("openlore-indexer serve did not report a bound address on stdout; stderr: {err}");
    });

    IndexerHandle {
        url: format!("http://{addr}"),
        child,
        _source: source,
    }
}

/// Run `openlore <args>` with the CLI's `indexer_url` pointed at `indexer` (the
/// localhost `openlore-indexer serve` URL from [`IndexerHandle::indexer_url`]).
/// Mirrors [`run_openlore`]'s clean-env discipline + adds the
/// `OPENLORE_INDEXER_URL` seam the `search` verb reads. Used by the AV-8 network
/// search scenario.
pub fn run_openlore_search(env: &TestEnv, args: &[&str], indexer: &IndexerHandle) -> CliOutcome {
    let bin = assert_cmd::cargo::cargo_bin("openlore");
    let output = Command::new(&bin)
        .args(args)
        .env_clear()
        .env("OPENLORE_HOME", &env.home)
        .env("OPENLORE_DID", env.identity.author_did())
        .env("OPENLORE_KEY_SEED_HEX", &env.identity.seed_hex)
        .env("OPENLORE_PDS_ENDPOINT", env.pds.endpoint_url())
        .env("OPENLORE_INDEXER_URL", indexer.indexer_url())
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

/// Run `openlore-indexer <args>` (the SECOND binary) with the indexer's own
/// config + index dir under `env.home`. Returns the [`CliOutcome`]. Used by the
/// US-AV-001 infra scenarios (ingest/serve/help/capability-boundary).
///
/// SCAFFOLD: true (slice-05) — DELIVER wires `assert_cmd::cargo_bin("openlore-
/// indexer")` against the indexer config + the FakeIngestSource + the fixture
/// PLC resolver env seams (the indexer-side analog of `run_openlore`).
pub fn run_openlore_indexer(env: &TestEnv, args: &[&str]) -> CliOutcome {
    // SCAFFOLD: true (slice-05)
    let _ = (env, args);
    todo!(
        "DELIVER (slice-05): run `openlore-indexer <args>` via \
         assert_cmd::cargo_bin against the indexer config + FakeIngestSource + \
         fixture PLC resolver seams; capture status/stdout/stderr."
    )
}

/// Assert the slice-05 network-search anti-merging contract over a `search`
/// stdout (the slice-03/04 discipline carried to network scale): count the
/// attributed result rows for `(subject, object)`; assert it equals
/// `expected_rows` AND that NO line matches a merged/consensus/"N authors
/// agree"/mean-confidence template, EXCLUDING the footer's "distinct authors"
/// count line from the row count. The behavioral layer of I-AV-2 (AV-9/AV-16/AV-27).
///
/// SCAFFOLD: true (slice-05) — universe (port-exposed): the count of attributed
/// rows for the pair; the author-set of those rows; absence of any merged row;
/// the footer distinct-author count. NEVER an internal compose struct field.
pub fn assert_network_result_preserves_attribution(
    stdout: &str,
    _subject: &str,
    _object: &str,
    expected_rows: usize,
    expected_authors: &[&str],
) {
    // The attributed-row universe: each `author_did:` line in the output is one
    // attributed result row. (The footer's "distinct author(s)" count line is a
    // SUMMARY, not a row — it never starts with `author_did:`, so it is excluded
    // by construction.)
    let author_rows: Vec<&str> = stdout
        .lines()
        .map(|l| l.trim())
        .filter(|l| l.starts_with("author_did:"))
        .collect();
    assert_eq!(
        author_rows.len(),
        expected_rows,
        "expected {expected_rows} attributed rows (one author_did per row); got {}:\n{stdout}",
        author_rows.len()
    );

    // Every expected author appears as an attributed row author.
    for expected in expected_authors {
        assert!(
            author_rows.iter().any(|row| row.contains(expected)),
            "expected an attributed row for author {expected:?}; rows:\n{author_rows:?}\n\
             full output:\n{stdout}"
        );
    }

    // NO merged/consensus row anywhere (the cardinal anti-merging gate, I-AV-2).
    // The content-frozen no-merge GUARANTEE footer legitimately says "No claims
    // are merged" + "not a community consensus" — those are the PROMISE, not a
    // merged row, so they are excluded from the merge-detection scan.
    let banned_substrings = ["authors agree", "the network says", "the network thinks"];
    for line in stdout.lines() {
        let lowered = line.to_ascii_lowercase();
        for banned in &banned_substrings {
            assert!(
                !lowered.contains(banned),
                "anti-merging (I-AV-2): no merged/consensus row may appear; found {banned:?} \
                 in line {line:?}\nfull output:\n{stdout}"
            );
        }
    }
}

/// Assert every result row in a `search` stdout carries a `[verified]` marker and
/// that NO row shows `[unverified]` / `unknown signature` (the universal-marker
/// construction guarantee; I-AV-1; AV-11).
///
/// SCAFFOLD: true (slice-05) — universe (port-exposed): for every result row,
/// row contains "[verified]"; the strings "[unverified]"/"unknown signature"
/// never appear.
pub fn assert_verified_marker_is_universal(stdout: &str) {
    // Count the attributed result rows (one `author_did:` line each) and the
    // `[verified]` markers; every row must carry the marker (verified-before-index,
    // I-AV-1 — there is no `[unverified]` state).
    let row_count = stdout
        .lines()
        .filter(|l| l.trim().starts_with("author_did:"))
        .count();
    let verified_count = stdout.matches("[verified]").count();
    assert!(
        row_count > 0,
        "expected at least one attributed result row to assert the universal \
         [verified] marker over; got none:\n{stdout}"
    );
    assert_eq!(
        verified_count, row_count,
        "I-AV-1: every result row ({row_count}) must carry a [verified] marker; \
         found {verified_count} markers:\n{stdout}"
    );
    for banned in &["[unverified]", "unknown signature"] {
        assert!(
            !stdout.contains(banned),
            "I-AV-1: no row may show {banned:?} (verification is an ingest \
             precondition — there is no unverified state):\n{stdout}"
        );
    }
}

/// Assert the OD-AV-7 counter-shown-not-applied render contract over a `search`
/// stdout (AV-25 / I-AV-9): the countered claim C (`countered_cid`, authored by
/// `countered_author`) is STILL present as an attributed result row AND its row
/// carries the counter annotation `countered-by <counter_cid> (by <counter_author>)`
/// — the counter is SHOWN, never applied as a filter / removal / down-weight
/// (mirrors slice-04 WD-85). Universe (port-exposed): presence of C's attributed
/// row; the counter annotation on C's row; C NOT filtered (its row is rendered as
/// a normal verified attributed row).
pub fn assert_counter_annotation_shown_not_applied(
    stdout: &str,
    countered_cid: &str,
    countered_author: &str,
    counter_cid: &str,
    counter_author: &str,
) {
    // 1. C is STILL present — its attributed row (author + cid) is rendered (NOT
    //    filtered / dropped). The countered row survives the counter (OD-AV-7).
    assert!(
        stdout.contains(&format!("author_did: {countered_author}")),
        "AV-25: the countered claim's author row must STILL be present (shown, not \
         filtered): expected an `author_did: {countered_author}` row:\n{stdout}"
    );
    assert!(
        stdout.contains(&format!("cid:        {countered_cid}")),
        "AV-25: the countered claim C ({countered_cid}) must STILL appear as a \
         result row (NOT filtered/dropped/down-weighted — OD-AV-7 shown-not-applied):\n{stdout}"
    );

    // 2. C's row carries the counter annotation naming the COUNTERING claim K's cid
    //    + its author (countered-by <K.cid> (by <K.author>)). The counter is SHOWN.
    assert!(
        stdout.contains(&format!("countered-by {counter_cid} (by {counter_author})")),
        "AV-25: the countered row must carry the counter annotation \
         'countered-by {counter_cid} (by {counter_author})' (OD-AV-7 / I-AV-9 — \
         counter SHOWN, never applied):\n{stdout}"
    );

    // 3. The counter is SHOWN, NEVER applied: NO filtering/down-weighting language
    //    appears (the countered claim is not hidden, dimmed, or removed).
    for banned in &[
        "filtered out",
        "down-weighted",
        "suppressed",
        "hidden by counter",
    ] {
        assert!(
            !stdout.to_ascii_lowercase().contains(banned),
            "AV-25: the counter must be SHOWN, never APPLIED — found {banned:?} in the \
             output (OD-AV-7 shown-not-applied / WD-119 default):\n{stdout}"
        );
    }
}

/// Assert a `search` stdout reports a VALID EMPTY dimension result with a
/// near-match suggestion (US-AV-002 Ex 4 / AV-12): it NAMES the queried object
/// ("No network claims found for object <typo>") AND offers the closest known
/// object as a "Did you mean <near>?" line. The empty result is NOT an error
/// (exit 0) — distinct from the `--show`-absent-cid usage error (non-zero).
///
/// Universe (port-exposed): stdout states the empty-for-object message naming
/// the typo, AND the near-match "Did you mean <near>?" line; no attributed result
/// row exists (it was an empty result).
pub fn assert_empty_with_near_match_suggestion(stdout: &str, queried: &str, near_match: &str) {
    // 1. The empty result NAMES the queried object (self-explanatory message).
    assert!(
        stdout.contains(&format!("No network claims found for object {queried}")),
        "AV-12: the empty result must NAME the queried object \
         ('No network claims found for object {queried}'):\n{stdout}"
    );
    // 2. The near-match suggestion line offers the closest known object (AVC-8).
    assert!(
        stdout.contains(&format!("Did you mean {near_match}?")),
        "AV-12: the empty result must offer the near-match suggestion \
         ('Did you mean {near_match}?'):\n{stdout}"
    );
    // 3. It was genuinely EMPTY — no attributed result row was rendered.
    assert!(
        !stdout
            .lines()
            .any(|line| line.trim().starts_with("author_did:")),
        "AV-12: an empty result must render NO attributed `author_did:` row:\n{stdout}"
    );
}

/// Assert a `search` stdout reports a VALID EMPTY CONTRIBUTOR result (US-AV-003 Ex3
/// / AV-17): it NAMES the queried contributor handle ("no network claims found for
/// contributor <handle>") and is genuinely empty (no attributed result row). Unlike
/// the object dimension (AV-12), the contributor empty path offers NO near-match
/// suggestion — a contributor either publishes OpenLore claims or does not; there is
/// no "Did you mean <near>?" guess. The empty result is NOT an error (exit 0).
///
/// Universe (port-exposed): stdout states the empty-for-contributor message naming
/// the handle (case-insensitive on the leading "No"); NO "Did you mean" suggestion
/// line; no attributed `author_did:` row exists (it was an empty result).
pub fn assert_empty_contributor_message(stdout: &str, handle: &str) {
    // 1. The empty result NAMES the queried contributor handle. Compare
    // case-insensitively on the message prefix so the renderer's "No network claims
    // found for contributor <handle>" satisfies the spec's "no network claims found
    // for contributor <handle>" phrasing regardless of leading capitalization.
    let lowered = stdout.to_ascii_lowercase();
    let expected = format!("no network claims found for contributor {handle}").to_ascii_lowercase();
    assert!(
        lowered.contains(&expected),
        "AV-17: the empty contributor result must NAME the queried contributor \
         ('no network claims found for contributor {handle}'):\n{stdout}"
    );
    // 2. The contributor empty path offers NO near-match suggestion (distinct from
    // the object dimension's AV-12 "Did you mean <near>?" line).
    assert!(
        !stdout.contains("Did you mean"),
        "AV-17: an empty contributor result must NOT offer a near-match suggestion \
         (a contributor is absent, not a typo):\n{stdout}"
    );
    // 3. It was genuinely EMPTY — no attributed result row was rendered.
    assert!(
        !stdout
            .lines()
            .any(|line| line.trim().starts_with("author_did:")),
        "AV-17: an empty contributor result must render NO attributed `author_did:` row:\n{stdout}"
    );
}

/// Assert a `search` stdout prints the public-data banner UP FRONT (before the
/// first result row): public-signed-only + verified-before-indexing +
/// nothing-private-read/aggregated (I-AV-4 / KPI-AV-5; AV-10).
///
/// SCAFFOLD: true (slice-05) — universe (port-exposed): the banner present AND
/// positioned before the first result row.
pub fn assert_public_data_banner_precedes_results(stdout: &str) {
    // The content-frozen search public-data banner (KPI-AV-5 / I-AV-4): a stable
    // recognizable substring of `render::SEARCH_PUBLIC_DATA_BANNER`.
    const BANNER_SUBSTR: &str = "Discovery indexes ONLY public, signed claims";
    let banner_pos = stdout.find(BANNER_SUBSTR).unwrap_or_else(|| {
        panic!(
            "expected the public-data banner ({BANNER_SUBSTR:?}) in the search \
             output (KPI-AV-5 / I-AV-4):\n{stdout}"
        )
    });
    // The banner must PRECEDE the first attributed result row.
    let first_row_pos = stdout
        .match_indices("author_did:")
        .next()
        .map(|(idx, _)| idx);
    if let Some(row_pos) = first_row_pos {
        assert!(
            banner_pos < row_pos,
            "the public-data banner must PRECEDE the first result row \
             (banner at {banner_pos}, first row at {row_pos}):\n{stdout}"
        );
    }
    // The banner asserts the three honesty facts (public-only + verified-before-
    // indexing + nothing-private).
    assert!(
        stdout.contains("verified before indexing"),
        "the banner must assert verified-before-indexing:\n{stdout}"
    );
    assert!(
        stdout.contains("Nothing private is read or aggregated"),
        "the banner must assert nothing-private-read/aggregated:\n{stdout}"
    );
}

/// Assert a `search` stdout proves the B1 localhost transport was REACHED (AV-14;
/// the positive complement to AV-13's soft-degradation): a result was returned
/// over the real `openlore-indexer serve` localhost HTTP/XRPC port (NOT the SOFT
/// `Unreachable` local-only degradation), the rendered result is NON-EMPTY +
/// ATTRIBUTED (every attributed row carries `author_did:` AND `[verified]`), and
/// the wire carried per-result `author_did` end-to-end (D-D36 / WD-115). The
/// distinct rendered rows can only exist if the transport preserved each row's
/// attribution.
///
/// Universe (port-exposed): the ABSENCE of the `Unreachable` degradation message
/// (the transport reached the serve port); ≥1 attributed `author_did:` row; every
/// such row carries `[verified]`.
pub fn assert_transport_reached_serve_port(stdout: &str) {
    // The transport was REACHED — NOT the SOFT `Unreachable` local-only
    // degradation (AV-13's "Network index unavailable" soft path). A result over
    // the wire is the positive complement: if `search` had degraded, it would
    // print the local-only pointer instead of attributed rows.
    assert!(
        !stdout.contains("Network index unavailable"),
        "B1 (AV-14): the search must REACH the real localhost serve port — it must \
         NOT fall to the SOFT `Unreachable` local-only degradation:\n{stdout}"
    );

    // A NON-EMPTY attributed result: ≥1 row, each carrying `author_did:` (the wire
    // preserved attribution end-to-end — D-D36). The transport returned a result.
    let author_rows: Vec<&str> = stdout
        .lines()
        .map(|line| line.trim())
        .filter(|line| line.starts_with("author_did:"))
        .collect();
    assert!(
        !author_rows.is_empty(),
        "B1 (AV-14): the transport must return a NON-EMPTY attributed result (≥1 \
         `author_did:` row); a result was NOT returned over the localhost serve \
         port:\n{stdout}"
    );

    // Every attributed row carries `[verified]` (the wire `verified_against` drives
    // the universal marker; I-AV-1) AND the wire carried per-result `author_did`
    // (anti-merging across the transport, D-D36). Reuse the universal-marker gate.
    assert_verified_marker_is_universal(stdout);
}

/// The set of attributed result rows (each `author_did:` line, trimmed) in a
/// `search` stdout — the port-exposed per-author attribution surface. Used to
/// compare a re-run (link-resolved) result against the original query's (AV-27).
fn attributed_author_rows(stdout: &str) -> Vec<String> {
    stdout
        .lines()
        .map(|line| line.trim().to_string())
        .filter(|line| line.starts_with("author_did:"))
        .collect()
}

/// Assert the AV-27 / US-AV-006 Ex2 share-boundary round-trip (KPI-AV-6 /
/// KPI-AV-2 / I-AV-8): opening a shared link RE-RUNS the encoded query, so the
/// `resolved` stdout's per-author attributed rows MATCH the `original` query's
/// rows (same authors, same `[verified]` marks), with NO merged consensus row and
/// the same `peer add` follow affordance for unfollowed authors. The link encoded
/// the QUERY (deterministic per AVC-3b), NOT a snapshot — the resolver re-composes
/// per-author rows from scratch (anti-merging across the share boundary).
///
/// Universe (port-exposed): the resolved result's set of attributed `author_did:`
/// rows == the original query's set; every resolved row carries `[verified]`; NO
/// merged/consensus row in the resolved output; the `openlore peer add` follow
/// affordance present (for the unfollowed-author corpus). NEVER an internal field.
pub fn assert_resolved_link_matches_original_query(original: &str, resolved: &str) {
    // 1. The re-run preserves attribution: the resolved output's set of attributed
    //    author rows EQUALS the original query's set (same authors, same per-author
    //    rows). The link re-ran the QUERY, so the re-composition is deterministic
    //    (AVC-3b) — same authors, same count, NOT a stale/lossy snapshot.
    let mut original_rows = attributed_author_rows(original);
    let mut resolved_rows = attributed_author_rows(resolved);
    assert!(
        !resolved_rows.is_empty(),
        "AV-27: the resolved link must re-run the query to a NON-EMPTY attributed \
         result (>=1 `author_did:` row):\nresolved:\n{resolved}"
    );
    original_rows.sort();
    resolved_rows.sort();
    assert_eq!(
        resolved_rows, original_rows,
        "AV-27: opening the shared link must RE-RUN the query to the SAME per-author \
         attributed rows as the original query (same authors, same [verified] marks). \
         original rows: {original_rows:?}\nresolved rows: {resolved_rows:?}\n\
         original output:\n{original}\nresolved output:\n{resolved}"
    );

    // 2. Every resolved row carries `[verified]`; NO `[unverified]`/unknown-signature
    //    state (the universal-marker guarantee survives the share boundary, I-AV-1).
    assert_verified_marker_is_universal(resolved);

    // 3. NO merged/consensus row in the resolved output (anti-merging across the
    //    share boundary, I-AV-2/KPI-AV-2). The no-merge GUARANTEE footer is the
    //    PROMISE, not a merged row, so it is excluded from the merge-detection scan.
    for banned in &["authors agree", "the network says", "the network thinks"] {
        for line in resolved.lines() {
            assert!(
                !line.to_ascii_lowercase().contains(banned),
                "AV-27 (anti-merging, I-AV-2): no merged/consensus row may appear in \
                 the resolved result; found {banned:?} in line {line:?}\nresolved:\n{resolved}"
            );
        }
    }

    // 4. The same `peer add` follow affordance is present for unfollowed authors —
    //    the discovery->federation funnel (WD-110 / I-AV-7) survives the share
    //    boundary (the resolver re-renders via the SAME network-result renderer).
    assert!(
        resolved.contains("openlore peer add"),
        "AV-27: the resolved result must carry the same `openlore peer add <did>` \
         follow affordance for unfollowed authors:\nresolved:\n{resolved}"
    );
}

/// Assert the AV-28 / US-AV-006 Ex4 query-encoding-NOT-snapshot contract across an
/// INDEX CHANGE (KPI-AV-6 / I-AV-8): after a SECOND ingest pass adds matching
/// claims, re-opening the SAME share link RE-RUNS the encoded query against the
/// CURRENT index, so the resolved result set GREW by `grew_by` rows relative to
/// the `original` pre-ingest result — it includes the newly-ingested claims rather
/// than resolving to a frozen snapshot. Each newly-present author in `new_authors`
/// appears as an attributed row; every row carries `[verified]`; NO merged/
/// consensus row collapses authors into a stored merged view.
///
/// The cardinal AV-28 disprover: if the link resolved to a STALE snapshot the
/// resolved row count would still equal the original (no growth) — this helper
/// fails. The growth proves the link encoded the QUERY, re-run live against the
/// CURRENT index (the 05-13 resolver), never a frozen result set.
///
/// Universe (port-exposed): the resolved attributed-row count == the original's +
/// `grew_by` (CURRENT, not frozen); each `new_authors` DID present as an attributed
/// row; every resolved row `[verified]`; no merged view. NEVER an internal field.
pub fn assert_resolved_link_grew_to_current_results(
    original: &str,
    resolved: &str,
    grew_by: usize,
    new_authors: &[&str],
) {
    let original_rows = attributed_author_rows(original);
    let resolved_rows = attributed_author_rows(resolved);

    // 1. The resolved set GREW by exactly `grew_by` — the link re-ran the QUERY
    //    against the CURRENT (post-ingest) index, never a frozen pre-ingest
    //    snapshot. A stale snapshot would keep the original count (growth == 0).
    assert_eq!(
        resolved_rows.len(),
        original_rows.len() + grew_by,
        "AV-28: re-opening the link after a SECOND ingest must RE-RUN the query \
         against the CURRENT index — the resolved attributed-row count must GROW by \
         {grew_by} (from {} to {}), proving the link encodes the QUERY, not a frozen \
         snapshot (US-AV-006 Ex4 / KPI-AV-6 / I-AV-8). original rows: {original_rows:?}\n\
         resolved rows: {resolved_rows:?}\noriginal output:\n{original}\n\
         resolved output:\n{resolved}",
        original_rows.len(),
        original_rows.len() + grew_by
    );

    // 2. The original result set is PRESERVED across the index change — every
    //    pre-ingest attributed row is STILL present in the re-run (the new ingest
    //    ADDED claims; it did not drop or replace the original attribution). This
    //    rules out a lossy "replace the snapshot" path masquerading as growth.
    for original_row in &original_rows {
        assert!(
            resolved_rows.contains(original_row),
            "AV-28: the re-run must PRESERVE every original attributed row across the \
             index change (the second ingest ADDS, never drops); missing \
             {original_row:?} from the resolved rows: {resolved_rows:?}"
        );
    }

    // 3. Each NEWLY-ingested author appears as an attributed row in the re-run —
    //    the link surfaced the CURRENT claims, each attributed (anti-merging
    //    preserved across the share boundary AND the index change, I-AV-8/KPI-AV-2).
    for new_author in new_authors {
        assert!(
            resolved_rows.iter().any(|row| row.contains(new_author)),
            "AV-28: the re-opened link must include the newly-ingested claim by \
             {new_author:?} as an ATTRIBUTED row (the link re-runs the query against \
             the CURRENT index); resolved rows: {resolved_rows:?}\nresolved output:\n{resolved}"
        );
    }

    // 4. Every resolved row carries `[verified]`; NO `[unverified]`/unknown-signature
    //    state — the universal-marker guarantee holds for the new rows too (the new
    //    claims passed the verify-before-index gate; I-AV-1).
    assert_verified_marker_is_universal(resolved);

    // 5. NO merged/consensus row collapses authors into a stored merged view (the
    //    link NEVER resolves to a merged snapshot that loses attribution; I-AV-8 /
    //    KPI-AV-2). The no-merge GUARANTEE footer is the PROMISE, not a merged row.
    for banned in &["authors agree", "the network says", "the network thinks"] {
        for line in resolved.lines() {
            assert!(
                !line.to_ascii_lowercase().contains(banned),
                "AV-28 (anti-merging, I-AV-8): the link must never resolve to a merged \
                 snapshot that loses attribution; found {banned:?} in line {line:?}\n\
                 resolved:\n{resolved}"
            );
        }
    }
}

/// Extract the `cid:` value rendered for the FIRST result row whose attribution
/// block names `author_substr` (e.g. `did:plc:priya-test`) and whose `subject:`
/// line names `subject_substr` (e.g. `github:bazelbuild/bazel`) in an AV-8-style
/// `search --object` stdout. Used by AV-23 to capture a REAL cid the result list
/// emitted, then `--show` it (chaining off the same search the user just ran).
///
/// The renderer emits each row as `author_did:` then `subject:` / `object:` /
/// `confidence:` / `evidence:` / `cid:` lines (render::render_one_network_row), so
/// the cid for the matched (author, subject) row is the next `cid:` line AFTER the
/// matching `author_did:`+`subject:` pair. Port-exposed: parses only the rendered
/// stdout (the CLI driving-port observable), never an internal struct.
pub fn cid_from_search_row(stdout: &str, author_substr: &str, subject_substr: &str) -> String {
    let lines: Vec<&str> = stdout.lines().collect();
    let mut in_matching_author = false;
    let mut subject_seen = false;
    for line in &lines {
        let trimmed = line.trim();
        if trimmed.starts_with("author_did:") {
            in_matching_author = trimmed.contains(author_substr);
            subject_seen = false;
            continue;
        }
        if !in_matching_author {
            continue;
        }
        if trimmed.starts_with("subject:") {
            subject_seen = trimmed.contains(subject_substr);
            continue;
        }
        if subject_seen && trimmed.starts_with("cid:") {
            return trimmed.trim_start_matches("cid:").trim().to_string();
        }
    }
    panic!(
        "cid_from_search_row: no rendered row found for author {author_substr:?} + \
         subject {subject_substr:?} in search output:\n{stdout}"
    );
}

/// Assert the AV-24 `--show <cid>` absent-cid USAGE-ERROR contract (US-AV-004 Ex4):
/// `--show`ing a CID NOT in the current result set is a usage error — the CLI exits
/// NON-ZERO (deliberately distinct from the empty-search exit-0, AV-12/AV-17, so the
/// user can tell a typo'd `--show` from an empty query) AND names the absent `cid`
/// in the content-frozen "CID ... is not in this search result." message PLUS the
/// remediation hint ("Run the search without --show to list results, then --show a
/// listed CID."). The production verb prints this on the SearchOutcome's stdout.
///
/// Universe (port-exposed, asserted against the `--show` outcome): the NON-ZERO exit
/// code; the "CID <cid> is not in this search result" usage message; the
/// run-without-`--show` remediation hint. Never an internal struct field.
pub fn assert_show_absent_cid_usage_error(outcome: &CliOutcome, absent_cid: &str) {
    // 1. NON-ZERO exit — the ONE non-zero sad path on the search surface (distinct
    //    from the empty-result exit-0; the user can tell a typo'd --show from an
    //    empty query, US-AV-004 Ex4).
    assert_ne!(
        outcome.status, 0,
        "AV-24: `--show <absent cid>` must exit NON-ZERO (a usage error, distinct \
         from the empty-search exit-0). \n--- stdout ---\n{}\n--- stderr ---\n{}",
        outcome.stdout, outcome.stderr
    );

    // 2. The usage message NAMES the absent cid + states it is not in this search
    //    result. The verb writes the SearchOutcome stdout, so the message lands
    //    there; scan both surfaces so the assertion stays robust to the boundary.
    let surfaces = format!("{}\n{}", outcome.stdout, outcome.stderr);
    assert!(
        surfaces.contains(&format!("CID {absent_cid} is not in this search result")),
        "AV-24: the usage error must name the absent cid + state it is not in this \
         search result ('CID {absent_cid} is not in this search result'). \
         \n--- stdout ---\n{}\n--- stderr ---\n{}",
        outcome.stdout,
        outcome.stderr
    );

    // 3. The remediation hint — re-run the search WITHOUT --show to list results,
    //    then --show a LISTED cid (the user-visible recovery path).
    assert!(
        surfaces
            .contains("Run the search without --show to list results, then --show a listed CID."),
        "AV-24: the usage error must carry the remediation hint ('Run the search \
         without --show to list results, then --show a listed CID.'). \
         \n--- stdout ---\n{}\n--- stderr ---\n{}",
        outcome.stdout,
        outcome.stderr
    );
}

/// Assert the AV-23 `--show <cid>` trust-inspection contract (US-AV-004 Ex1 /
/// KPI-AV-3): the output prints the FULL record (subject / object / confidence /
/// evidence / author DID) PLUS the content-frozen
/// `Signature: VERIFIED against <author_did>` line AND the
/// `CID: <cid> (recomputed, matches published record)` line — the SAME pure-core
/// verification result the indexer computed at ingest (no second path). The
/// caller separately asserts the read-only (no-local-mutation) property.
///
/// Universe (port-exposed, asserted against the `--show` stdout): the full record
/// fields + the Signature-VERIFIED line + the CID-recomputed-matches line. Never
/// an internal struct field.
pub fn assert_show_inspects_verified_record(
    stdout: &str,
    cid: &str,
    expected_subject: &str,
    expected_object: &str,
    expected_confidence: &str,
    expected_author_did: &str,
) {
    // The full record fields — surfaced for the trust inspection.
    assert!(
        stdout.contains(&format!("subject:     {expected_subject}"))
            || stdout.contains(&format!("subject: {expected_subject}")),
        "--show must print the full record subject {expected_subject:?}:\n{stdout}"
    );
    assert!(
        stdout.contains(expected_object),
        "--show must print the record object {expected_object:?}:\n{stdout}"
    );
    assert!(
        stdout.contains(expected_confidence),
        "--show must print the record confidence {expected_confidence:?}:\n{stdout}"
    );
    assert!(
        stdout.contains(expected_author_did),
        "--show must print the record author DID {expected_author_did:?}:\n{stdout}"
    );

    // The Signature-VERIFIED line names the author DID (the verification result the
    // indexer computed at ingest — no second path).
    assert!(
        stdout.contains(&format!(
            "Signature: VERIFIED against {expected_author_did}"
        )),
        "--show must print 'Signature: VERIFIED against {expected_author_did}' \
         (the stored ingest verification result; no second path):\n{stdout}"
    );

    // The CID-recomputed-matches line names the inspected cid.
    assert!(
        stdout.contains(&format!(
            "CID: {cid} (recomputed, matches published record)"
        )),
        "--show must print 'CID: {cid} (recomputed, matches published record)' \
         (the cid the indexer recomputed + matched at ingest):\n{stdout}"
    );
}

/// Assert that none of the `adversarial_cids` appears anywhere in the index
/// (`index.duckdb` rows) NOR in any `search` result — the verified-before-index
/// reject contract (I-AV-1 / KPI-AV-3; AV-3). `env` locates the index store; the
/// search outputs are the rendered observable.
///
/// SCAFFOLD: true (slice-05) — universe (port-exposed): the adversarial cids
/// absent from indexed_claims AND absent from every search result; the valid cid
/// present + searchable.
pub fn assert_unverified_claims_never_indexed_nor_searchable(
    env: &TestEnv,
    adversarial: &[SearchAnchors],
    valid: &SearchAnchors,
) {
    // 1. The index holds EXACTLY one row — the valid record. The three
    //    adversarial records produced NO row (the load-bearing count, WD-104).
    let all_cids = read_all_indexed_cids(env);
    assert_eq!(
        all_cids.len(),
        1,
        "KPI-AV-3: index.duckdb must contain EXACTLY the one valid record; the \
         three adversarial records must produce NO row. Found cids: {all_cids:?}"
    );
    assert_eq!(
        all_cids[0], valid.cid,
        "KPI-AV-3: the single indexed row must be the VALID record's cid"
    );

    // 2. Each adversarial CID is absent from indexed_claims AND from a search
    //    across EVERY dimension (object / subject / contributor). A search must
    //    NEVER surface any of the three (the cardinal disprover).
    for adv in adversarial {
        assert!(
            !all_cids.contains(&adv.cid),
            "KPI-AV-3: adversarial cid {} must be ABSENT from indexed_claims; \
             present cids: {all_cids:?}",
            adv.cid
        );
        for (dimension, rows) in search_every_dimension(env, adv) {
            assert!(
                rows.iter().all(|r| r.cid != adv.cid),
                "KPI-AV-3: a search by {dimension} must NEVER return adversarial \
                 cid {}; it leaked into the {dimension} search result: {rows:?}",
                adv.cid
            );
        }
    }

    // 3. The valid record IS searchable across every dimension, attributed, with
    //    a non-empty verified_against (the false-positive direction: the good
    //    claim must NOT be silently dropped — KPI-AV-3 cuts both ways).
    for (dimension, rows) in search_every_dimension(env, valid) {
        let found = rows.iter().find(|r| r.cid == valid.cid).unwrap_or_else(|| {
            panic!(
                "KPI-AV-3 (false-positive guard): the VALID record must be \
                 searchable by {dimension}; it was NOT returned. rows: {rows:?}"
            )
        });
        assert_eq!(
            found.author_did, valid.author_did,
            "the valid row must be attributed to its author across {dimension}"
        );
        assert!(
            !found.verified_against.is_empty(),
            "verified_against must never be empty on the valid indexed row \
             (WD-104), but was empty in the {dimension} search result"
        );
    }
}

/// The per-record search anchors (the values a search keys on across every
/// `SearchDimension` — object / subject / contributor) plus the record's CID.
/// Port-exposed observable surface; never an internal store field.
#[derive(Debug, Clone)]
pub struct SearchAnchors {
    pub cid: String,
    pub object: String,
    pub subject: String,
    /// The signed-payload author DID (the `#fragment` form stored in the row).
    pub author_did: String,
}

/// Search the index across EVERY `SearchDimension` for one record's anchors,
/// returning `(dimension_label, rows)` per dimension. Used by the AV-3
/// search-absence + search-presence assertions.
fn search_every_dimension<'a>(
    env: &TestEnv,
    anchors: &'a SearchAnchors,
) -> Vec<(&'static str, Vec<IndexedRow>)> {
    vec![
        (
            "object",
            read_indexed_claims_by_object(env, &anchors.object),
        ),
        (
            "subject",
            read_indexed_claims_by_subject(env, &anchors.subject),
        ),
        (
            "contributor",
            read_indexed_claims_by_contributor(env, &anchors.author_did),
        ),
    ]
}

/// Assert the user's `openlore.duckdb` is byte-unchanged across an
/// `openlore-indexer` run (the capability boundary: the indexer holds no
/// local-store handle; ADR-023 / I-AV-5; AV-5). Snapshot before, run, compare.
///
/// SCAFFOLD: true (slice-05) — universe (port-exposed): openlore.duckdb
/// bytes/mtime unchanged; only index.duckdb written.
pub fn assert_local_store_untouched_by_indexer(env: &TestEnv, run: impl FnOnce()) {
    // SCAFFOLD: true (slice-05)
    let _ = (env, run);
    todo!(
        "DELIVER (slice-05): snapshot openlore.duckdb bytes; run the indexer \
         closure; assert openlore.duckdb byte-unchanged and only index.duckdb \
         written (ADR-023 / I-AV-5 capability boundary)."
    )
}

/// A parsed shareable link emitted by `openlore search --share`. SCAFFOLD: true.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShareLink {
    pub dimension: String,
    pub value: String,
}

/// The `openlore://search?` scheme+authority prefix every share link carries.
const SHARE_LINK_PREFIX: &str = "openlore://search?";

/// Result-payload / snapshot tokens that MUST NEVER appear inside a share link —
/// the link encodes the QUERY (dimension+value) only, never a frozen result
/// snapshot (I-AV-8 / KPI-AV-6). If any of these leak into the link's
/// query-string the share is a snapshot, not a query encoding.
const SNAPSHOT_PAYLOAD_TOKENS: &[&str] = &[
    "author_did",
    "[verified]",
    "cid",
    "confidence",
    "results",
    "snapshot",
];

/// Parse the `openlore://search?<dimension>=<value>` link from a `--share`
/// stdout, asserting it encodes ONLY the query dimension+value (NO result
/// payload / NO snapshot; I-AV-8; AV-26/AV-29).
///
/// The link is emitted on a `Shareable link: <link>` line. The parser extracts
/// the single `<dimension>=<value>` query parameter and asserts the query-string
/// carries NO result-payload / snapshot token ([`SNAPSHOT_PAYLOAD_TOKENS`]) — the
/// share encodes the QUERY only, never a frozen result set. Returns the parsed
/// [`ShareLink`] so the caller can pin the exact dimension + value.
///
/// universe (port-exposed): the link's encoded dimension + value; the absence of
/// any result/snapshot payload in the link.
pub fn parse_and_assert_query_encoding_share_link(stdout: &str) -> ShareLink {
    // Find the `openlore://search?...` link anywhere in stdout (it is emitted on
    // the `Shareable link: <link>` line). The link is whitespace-delimited.
    let link = stdout
        .split_whitespace()
        .find(|token| token.starts_with(SHARE_LINK_PREFIX))
        .unwrap_or_else(|| {
            panic!("expected an `openlore://search?<dim>=<value>` share link in stdout:\n{stdout}")
        });

    // The query string is everything after the `openlore://search?` prefix.
    let query = link
        .strip_prefix(SHARE_LINK_PREFIX)
        .expect("link starts with the share prefix");

    // The query encodes EXACTLY ONE `<dimension>=<value>` parameter — no result
    // payload, no snapshot. A second `&`-separated parameter would be extra state.
    assert!(
        !query.contains('&'),
        "the share link must encode a SINGLE query parameter (dimension=value), \
         never a multi-field snapshot — got `{query}`"
    );

    // NO result-payload / snapshot token may appear in the query string — the link
    // encodes the QUERY, not a frozen result set (I-AV-8 / KPI-AV-6).
    for token in SNAPSHOT_PAYLOAD_TOKENS {
        assert!(
            !query.contains(token),
            "the share link must NOT carry a result-payload/snapshot token \
             (`{token}`) — it encodes the QUERY only, never a snapshot. link: {link}"
        );
    }

    let (dimension, value) = query.split_once('=').unwrap_or_else(|| {
        panic!("the share link query must be `<dimension>=<value>` — got `{query}`")
    });
    assert!(
        !dimension.is_empty() && !value.is_empty(),
        "the share link must encode a non-empty dimension AND value — got `{query}`"
    );

    ShareLink {
        dimension: dimension.to_string(),
        value: value.to_string(),
    }
}

// =============================================================================
// Slice-05 walking-skeleton beat-1 harness (step 03-01; AV-1).
//
// The AV-1 scenario drives the REAL `openlore-indexer ingest` binary against a
// FAKE network ingest source (a localhost HTTP fixture serving the ATProto
// `com.atproto.repo.listRecords` surface) + the slice-03 PLC pubkey seam, into
// a REAL separate `index.duckdb`. The bodies below are what AV-1 needs; the
// search-side helpers (`seed_network_index`/`IndexerHandle`) stay `todo!()`
// (AV-8+ own them).
//
// The HTTP fixture is hand-rolled over `std::net::TcpListener` (one fixed
// `listRecords` response) rather than `hyper`, because the cli test target's
// dev-deps do NOT include `hyper`/`http-body-util` (only the `openlore-test-
// support` crate links those, via its own `serve_http` methods). A bounded
// single-route HTTP/1.1 fixture is the thinnest path that keeps the real
// `reqwest`-based ingest adapter exercising real network I/O (DD-AV-2 / the
// Architecture-of-Reference fake-for-external rule). Mirrors the slice-03
// `PeerPds` runtime-ownership shape (RAII shutdown on drop).
// =============================================================================

/// The ATProto auth-scoped/private read surface the public-data-only indexer must
/// NEVER call (the AV-7 tripwire). `getRepo` is the canonical "give me the whole
/// repo including non-public records" sync endpoint; an auth-scoped `listRecords`
/// would carry an `Authorization` header. Either is a public-data-only violation.
const AUTH_SCOPED_TRIPWIRE_PATH: &str = "/xrpc/com.atproto.sync.getRepo";

/// One request the [`FakeIngestServer`] received, projected to the
/// public-data-only observable surface (AV-7): the request-target path and
/// whether the request carried an `Authorization` header. NEVER any claim
/// content — the universe is "which endpoints did the indexer touch, and did it
/// authenticate", not "what did it read".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordedIngestRequest {
    /// The request-target (e.g. `/xrpc/com.atproto.repo.listRecords?...`).
    pub path: String,
    /// `true` iff the request carried an `Authorization` header (the marker of
    /// an authenticated / auth-scoped read — forbidden, WD-105 / I-AV-4).
    pub had_authorization_header: bool,
}

impl RecordedIngestRequest {
    /// Whether this request hit the PUBLIC `listRecords` surface.
    pub fn is_public_list_records(&self) -> bool {
        self.path.starts_with("/xrpc/com.atproto.repo.listRecords")
    }

    /// Whether this request hit the auth-scoped/private tripwire surface.
    pub fn is_auth_scoped(&self) -> bool {
        self.path.starts_with(AUTH_SCOPED_TRIPWIRE_PATH)
    }
}

/// A bounded localhost HTTP fixture that serves the ATProto
/// `com.atproto.repo.listRecords` surface for a fixed set of fixture records.
///
/// Owns a background acceptor thread bound to an OS-assigned `127.0.0.1:0`
/// port; the URL is read back via [`FakeIngestServer::source_url`] and wired
/// into the `openlore-indexer` subprocess. Dropping the handle signals the
/// acceptor to stop — RAII per-scenario isolation (mirrors `PeerPds`).
///
/// ## Public-data-only request recording (AV-7 / WD-105)
///
/// Every request the acceptor receives is recorded into a shared
/// `Vec<RecordedIngestRequest>` (path + presence of an `Authorization` header),
/// observable after the ingest pass via [`FakeIngestServer::recorded_requests`].
/// [`FakeIngestServer::start_with_private_tripwire`] additionally hosts an
/// auth-scoped/private surface ([`AUTH_SCOPED_TRIPWIRE_PATH`]) that WOULD serve a
/// private record if the indexer ever called it — the AV-7 tripwire. A
/// public-data-only indexer hits ONLY the public `listRecords` path, with NO
/// `Authorization` header, and NEVER the tripwire.
pub struct FakeIngestServer {
    base_url: String,
    shutdown: std::sync::Arc<std::sync::atomic::AtomicBool>,
    /// Every request the acceptor has received (path + Authorization presence) —
    /// the AV-7 public-data-only universe. Shared with the acceptor thread.
    recorded: std::sync::Arc<std::sync::Mutex<Vec<RecordedIngestRequest>>>,
    join: Option<std::thread::JoinHandle<()>>,
}

impl FakeIngestServer {
    /// Host `specs` (materialized to wire records via the REAL crypto in
    /// `RawRecordSpec::into_raw_record`) on a localhost `listRecords` surface.
    /// The adversarial postures are hosted VERBATIM — the indexer's gate, not
    /// this fixture, rejects them.
    ///
    /// No auth-scoped tripwire is hosted: a request to any path OTHER than the
    /// public `listRecords` surface returns 404 (AV-1..6 only PULL the public
    /// surface). Requests are still recorded so [`Self::recorded_requests`] is
    /// always observable.
    pub fn start(specs: Vec<openlore_test_support::RawRecordSpec>) -> Self {
        Self::start_inner(specs, Vec::new())
    }

    /// Host `public_specs` on the PUBLIC `listRecords` surface AND `private_specs`
    /// on the auth-scoped/private tripwire surface ([`AUTH_SCOPED_TRIPWIRE_PATH`])
    /// — the AV-7 public-data-only fixture (WD-105 / I-AV-4).
    ///
    /// The tripwire is a live route that WOULD serve the private records if the
    /// indexer ever called it; AV-7 asserts (via [`Self::recorded_requests`]) that
    /// the indexer hit ONLY the public surface with NO `Authorization` header and
    /// NEVER the tripwire — so the private records never enter the index.
    pub fn start_with_private_tripwire(
        public_specs: Vec<openlore_test_support::RawRecordSpec>,
        private_specs: Vec<openlore_test_support::RawRecordSpec>,
    ) -> Self {
        Self::start_inner(public_specs, private_specs)
    }

    fn start_inner(
        public_specs: Vec<openlore_test_support::RawRecordSpec>,
        private_specs: Vec<openlore_test_support::RawRecordSpec>,
    ) -> Self {
        use std::io::{Read, Write};
        use std::net::TcpListener;
        use std::sync::atomic::Ordering;

        // Materialize the public + private `listRecords`-shaped JSON bodies ONCE
        // at construction (deterministic; no per-request work).
        let public_body = list_records_body(public_specs);
        let private_body = list_records_body(private_specs);

        let listener =
            TcpListener::bind("127.0.0.1:0").expect("FakeIngestServer: bind 127.0.0.1:0");
        listener
            .set_nonblocking(true)
            .expect("FakeIngestServer: set_nonblocking");
        let local_addr = listener.local_addr().expect("FakeIngestServer: local_addr");
        let base_url = format!("http://{local_addr}");

        let shutdown = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let shutdown_for_thread = std::sync::Arc::clone(&shutdown);
        let recorded = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let recorded_for_thread = std::sync::Arc::clone(&recorded);

        let join = std::thread::Builder::new()
            .name("fake-ingest-source".to_string())
            .spawn(move || {
                let ok_response = |body: &str| {
                    format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\
                         Content-Length: {}\r\nConnection: close\r\n\r\n{}",
                        body.len(),
                        body
                    )
                };
                while !shutdown_for_thread.load(Ordering::SeqCst) {
                    match listener.accept() {
                        Ok((mut stream, _peer)) => {
                            // The listener is non-blocking (to poll the shutdown
                            // flag), but the ACCEPTED stream inherits non-blocking
                            // mode on some platforms (macOS). Force it BLOCKING so
                            // we reliably read the full request header block before
                            // routing — a partial/empty read would otherwise parse
                            // an empty path and misroute (the flake root cause).
                            let _ = stream.set_nonblocking(false);
                            let request = read_http_request_head(&mut stream);
                            let path = request
                                .lines()
                                .next()
                                .and_then(|line| line.split_whitespace().nth(1))
                                .unwrap_or_default()
                                .to_string();
                            let had_authorization_header = request.lines().any(|line| {
                                line.to_ascii_lowercase().starts_with("authorization:")
                            });
                            if let Ok(mut log) = recorded_for_thread.lock() {
                                log.push(RecordedIngestRequest {
                                    path: path.clone(),
                                    had_authorization_header,
                                });
                            }

                            // Route on the path: the auth-scoped tripwire serves
                            // the private body (it WOULD leak if the indexer ever
                            // called it — the AV-7 tripwire); EVERY other request
                            // (the public listRecords surface, and the empty-path
                            // fallback for an unparseable read) serves the public
                            // body. The indexer only legitimately PULLs the public
                            // listRecords surface, so the public body is the safe
                            // default — never a 404 that would spuriously fail a
                            // healthy PULL under parallel load.
                            let response = if path.starts_with(AUTH_SCOPED_TRIPWIRE_PATH) {
                                ok_response(&private_body)
                            } else {
                                ok_response(&public_body)
                            };
                            let _ = stream.write_all(response.as_bytes());
                            let _ = stream.flush();
                        }
                        Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                            std::thread::sleep(std::time::Duration::from_millis(5));
                        }
                        Err(_) => return,
                    }
                }
            })
            .expect("FakeIngestServer: spawn acceptor thread");

        Self {
            base_url,
            shutdown,
            recorded,
            join: Some(join),
        }
    }

    /// The `http://127.0.0.1:<port>` base URL the indexer's ingest adapter PULLs
    /// `listRecords` from (wired via `OPENLORE_INDEXER_SOURCE_URL`).
    pub fn source_url(&self) -> &str {
        &self.base_url
    }

    /// Every request the fixture received during the ingest pass (path +
    /// Authorization presence). The AV-7 public-data-only observable surface —
    /// inspect after the ingest pass to prove the indexer touched ONLY the public
    /// `listRecords` surface with NO `Authorization` header and NEVER the
    /// auth-scoped tripwire. Returns a snapshot (the lock is not held).
    pub fn recorded_requests(&self) -> Vec<RecordedIngestRequest> {
        self.recorded
            .lock()
            .map(|log| log.clone())
            .unwrap_or_default()
    }

    /// The number of requests that hit the auth-scoped/private tripwire surface
    /// ([`AUTH_SCOPED_TRIPWIRE_PATH`]). MUST be zero for a public-data-only
    /// indexer (WD-105 / I-AV-4).
    pub fn auth_scoped_call_count(&self) -> usize {
        self.recorded_requests()
            .iter()
            .filter(|r| r.is_auth_scoped())
            .count()
    }
}

/// Read an HTTP/1.1 request's head (request line + headers, up to the blank
/// `\r\n\r\n`) from a BLOCKING stream. Robust against the request arriving in
/// multiple TCP segments under parallel load — we keep reading until the header
/// terminator is seen (or the peer closes / a bounded cap is hit), so the
/// request-target path + the `Authorization` header are reliably parsed. Returns
/// the decoded head as a `String`. (We only inspect the head; the request body —
/// `listRecords`/`getRepo` are GETs — is irrelevant.)
fn read_http_request_head(stream: &mut std::net::TcpStream) -> String {
    use std::io::Read;
    let mut acc: Vec<u8> = Vec::with_capacity(1024);
    let mut chunk = [0u8; 1024];
    // Cap total reads so a malformed peer can never wedge the acceptor thread.
    for _ in 0..16 {
        match stream.read(&mut chunk) {
            Ok(0) => break, // peer closed
            Ok(n) => {
                acc.extend_from_slice(&chunk[..n]);
                if acc.windows(4).any(|w| w == b"\r\n\r\n") {
                    break; // full header block received
                }
            }
            Err(_) => break,
        }
    }
    String::from_utf8_lossy(&acc).into_owned()
}

/// Materialize a `listRecords`-shaped JSON body from `specs` (each run through
/// the REAL crypto via `RawRecordSpec::into_raw_record`). Shared by the public +
/// private surfaces of [`FakeIngestServer`].
fn list_records_body(specs: Vec<openlore_test_support::RawRecordSpec>) -> String {
    let records: Vec<serde_json::Value> = specs
        .into_iter()
        .map(|spec| raw_record_to_list_records_view(&spec.into_raw_record()))
        .collect();
    serde_json::json!({
        "records": records,
        "cursor": serde_json::Value::Null,
    })
    .to_string()
}

impl Drop for FakeIngestServer {
    fn drop(&mut self) {
        self.shutdown
            .store(true, std::sync::atomic::Ordering::SeqCst);
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

/// Serialize a `ports::RawRecord` into the ATProto `listRecords` record view
/// (`{uri, cid, value}`) the indexer's ingest adapter parses. `value` is the
/// lexicon claim JSON (`author`/`composedAt`/nested `signature:{kid,alg,sig}`);
/// `cid` echoes the published CID so the adapter's recompute-vs-published gate
/// has the published value (the SAME wire shape the slice-03 peer PDS uses).
fn raw_record_to_list_records_view(record: &ports::RawRecord) -> serde_json::Value {
    let claim = &record.raw_payload.unsigned;
    let sig = &record.raw_payload.signature;
    let references: Vec<serde_json::Value> = claim
        .references
        .iter()
        .map(|r| {
            serde_json::json!({
                "type": reference_type_wire(r.ref_type),
                "cid": r.cid.0,
            })
        })
        .collect();
    let value = serde_json::json!({
        "subject": claim.subject,
        "predicate": claim.predicate,
        "object": claim.object,
        "evidence": claim.evidence,
        "confidence": confidence_as_f64(&claim.confidence),
        "author": claim.author_did.0,
        "composedAt": claim.composed_at,
        "references": references,
        "signature": {
            "kid": sig.verification_method,
            "alg": "EdDSA",
            "sig": base64url_no_pad(&sig.signature_bytes),
        }
    });
    serde_json::json!({
        "uri": format!("at://{}/org.openlore.claim/{}", claim.author_did.0, record.published_cid.0),
        "cid": record.published_cid.0,
        "value": value,
    })
}

/// Map a `claim_domain::ReferenceType` to its lexicon wire token.
fn reference_type_wire(ref_type: claim_domain::ReferenceType) -> &'static str {
    match ref_type {
        claim_domain::ReferenceType::Retracts => "retracts",
        claim_domain::ReferenceType::Corrects => "corrects",
        claim_domain::ReferenceType::Counters => "counters",
        claim_domain::ReferenceType::Supersedes => "supersedes",
    }
}

/// Read the numeric confidence out of the domain `Confidence` wrapper via its
/// transparent serde representation (the inner field is crate-private; the same
/// trick the fixtures + the pure ingest gate use).
fn confidence_as_f64(confidence: &claim_domain::Confidence) -> f64 {
    serde_json::to_value(confidence)
        .ok()
        .and_then(|v| v.as_f64())
        .expect("Confidence serializes transparently as a JSON number")
}

/// Path to the indexer's SEPARATE `index.duckdb` for this scenario, under
/// `env.home` (the indexer's own data dir — NOT the user's `openlore.duckdb`).
pub fn index_duckdb_path(env: &TestEnv) -> PathBuf {
    env.home
        .join(".local")
        .join("share")
        .join("openlore-indexer")
        .join("index.duckdb")
}

/// Run `openlore-indexer <args>` (the SECOND binary) against a fake ingest
/// `source_url` + the slice-03 PLC pubkey seam(s) for `pubkey_seams`
/// (`(did, pubkey_hex)`), writing its index to [`index_duckdb_path`].
///
/// The indexer reads its config from env-var seams (the test analog of its
/// `config.toml`, mirroring the CLI's `OPENLORE_*` seams):
///   - `OPENLORE_INDEXER_INDEX_PATH`  — the SEPARATE `index.duckdb` path;
///   - `OPENLORE_INDEXER_SOURCE_URL`  — the fake `listRecords` source URL;
///   - `OPENLORE_PEER_PUBKEY_HEX_<did>` — the per-DID verify-key seam (slice-03).
pub fn run_openlore_indexer_with_source(
    env: &TestEnv,
    args: &[&str],
    source_url: &str,
    pubkey_seams: &[(&str, &str)],
) -> CliOutcome {
    let bin = assert_cmd::cargo::cargo_bin("openlore-indexer");
    let mut cmd = Command::new(&bin);
    cmd.args(args)
        .env_clear()
        .env("OPENLORE_HOME", &env.home)
        .env("OPENLORE_INDEXER_INDEX_PATH", index_duckdb_path(env))
        .env("OPENLORE_INDEXER_SOURCE_URL", source_url)
        .env("PATH", std::env::var("PATH").unwrap_or_default());
    for (did, pubkey_hex) in pubkey_seams {
        cmd.env(peer_pubkey_env_var(did), pubkey_hex);
    }
    let output = cmd
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .unwrap_or_else(|e| panic!("spawn openlore-indexer at {bin:?}: {e}"));
    CliOutcome {
        status: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    }
}

/// Run `openlore-indexer <args>` against a substrate whose fsync is a SILENT
/// NO-OP — the container substrate lie (DESIGN §6.3). The index store's
/// fsync-honesty probe MUST detect the lie and REFUSE to start.
///
/// We cannot reliably mount a real tmpfs/overlayfs/DrvFs across CI + macOS +
/// Linux, so the substrate lie is injected through a seam the index-store probe
/// honors: `OPENLORE_INDEXER_FORCE_FSYNC_NOOP=1` makes the probe treat the
/// durability medium as a fsync no-op (the SAME pragmatic limitation the
/// slice-01 `adapter-duckdb` fsync probe documents inline — userspace cannot
/// fully detect a kernel-side silent no-op without kernel cooperation). The
/// REAL fsync round-trip arm still runs; this seam only forces the no-op verdict
/// the probe would reach on a lying substrate.
///
/// Mirrors [`run_openlore_indexer_with_source`]'s clean-env discipline; the
/// `source_url` is wired so a wiring construction never fails for the wrong
/// reason (the probe, not the source, drives the refusal).
pub fn run_openlore_indexer_with_fsync_lying_store(
    env: &TestEnv,
    args: &[&str],
    source_url: &str,
) -> CliOutcome {
    let bin = assert_cmd::cargo::cargo_bin("openlore-indexer");
    let output = Command::new(&bin)
        .args(args)
        .env_clear()
        .env("OPENLORE_HOME", &env.home)
        .env("OPENLORE_INDEXER_INDEX_PATH", index_duckdb_path(env))
        .env("OPENLORE_INDEXER_SOURCE_URL", source_url)
        // The substrate lie: force the index-store fsync-honesty probe to reach
        // the no-op verdict (storage.fsync_unhonored) it would reach on a real
        // tmpfs/overlayfs/DrvFs durability no-op.
        .env("OPENLORE_INDEXER_FORCE_FSYNC_NOOP", "1")
        .env("PATH", std::env::var("PATH").unwrap_or_default())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .unwrap_or_else(|e| panic!("spawn openlore-indexer at {bin:?}: {e}"));
    CliOutcome {
        status: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    }
}

/// Parse the single `health.startup.refused` event from an indexer's stderr (the
/// DevOps observability contract: one structured JSON line per startup refusal).
/// Returns the parsed JSON value so AV-6 can assert on the `reason` +
/// `structured.event` fields. Panics with the full stderr if no such event is
/// present (a refusal that emits no `health.startup.refused` is itself a failure).
///
/// Port-exposed surface: the `health.startup.refused` event, NOT any internal
/// probe-struct field. Mirrors the AV-3 `indexer.ingest.rejected` line-scan.
pub fn parse_health_startup_refused(outcome: &CliOutcome) -> serde_json::Value {
    outcome
        .stderr
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .find(|event| event["event"] == "health.startup.refused")
        .unwrap_or_else(|| {
            panic!(
                "expected a health.startup.refused event in stderr; got:\n--- stdout ---\n{}\n\
                 --- stderr ---\n{}",
                outcome.stdout, outcome.stderr
            )
        })
}

/// Universe-bound: "the indexer ran NO ingest pass and served NO query — its
/// SEPARATE `index.duckdb` holds ZERO indexed rows (or was never created)".
/// Port-exposed name: `index_storage.indexed_claims.row_count` (== 0).
///
/// The wire → probe → use proof (ADR-009): a startup refusal must happen BEFORE
/// any ingest/serve work, so the index must be empty. Absent (never created) is
/// the strongest form of "no work"; a present-but-empty `indexed_claims` table
/// also proves no row was indexed. Any indexed row is a wire→use-without-probe
/// violation. Test-support is the only place raw SQL is acceptable.
pub fn assert_indexer_did_no_work(env: &TestEnv) {
    let db_path = index_duckdb_path(env);
    if !db_path.exists() {
        // Never created — the refusal happened before the store was opened/written.
        return;
    }
    let conn = duckdb::Connection::open(&db_path).unwrap_or_else(|err| {
        panic!(
            "open index.duckdb at {} for AV-6 no-work assertion: {err}",
            db_path.display()
        )
    });
    // The table may not exist if the refusal happened before migrations; treat a
    // missing table as zero work too.
    let indexed: i64 = conn
        .query_row("SELECT count(*) FROM indexed_claims", [], |r| r.get(0))
        .unwrap_or(0);
    assert_eq!(
        indexed, 0,
        "wire → probe → use (ADR-009): the indexer must REFUSE to start BEFORE any \
         ingest pass — index.duckdb must hold ZERO indexed_claims rows, but found \
         {indexed}. A failing probe must do NO work."
    );
}

// =============================================================================
// Slice-05 (step 03-04; AV-4) — the REAL `z6Mk` PLC DID-document resolver fixture.
//
// The AV-4 gold path proves the indexer decodes the author's verification key via
// the production `claim_domain::decode_ed25519_multibase` path (NOT the slice-03
// `OPENLORE_PEER_PUBKEY_HEX_<did>` env seam). To exercise the REAL decode the
// indexer must resolve a DID document carrying a REAL `z6Mk...` value: this
// fixture serves the W3C DID document (with the `#org.openlore.application`
// verification method's real `publicKeyMultibase`) at `GET /<did>`, the canonical
// PLC-directory shape (ADR-026 §"Resolve the DID document"). The indexer is
// pointed at this fixture via `OPENLORE_INDEXER_PLC_ENDPOINT`; the env seam stays
// UNSET so a seam-only impl could NOT resolve (a seam-only pass is impossible by
// construction).
//
// Hand-rolled over `std::net::TcpListener` (same rationale as `FakeIngestServer`:
// the cli test target's dev-deps do not include `hyper`). It branches on the
// request path so each hosted DID resolves to its own DID document. RAII shutdown
// on drop mirrors `FakeIngestServer` / `PeerPds`.
// =============================================================================

/// A bounded localhost HTTP fixture that serves W3C DID documents (each carrying a
/// REAL `z6Mk...` `publicKeyMultibase`) at the PLC-directory path `GET /<did>`.
///
/// Owns a background acceptor thread bound to an OS-assigned `127.0.0.1:0` port;
/// the URL is read back via [`FakePlcResolver::endpoint_url`] and wired into the
/// `openlore-indexer` subprocess via `OPENLORE_INDEXER_PLC_ENDPOINT`. Dropping the
/// handle signals the acceptor to stop — RAII per-scenario isolation.
pub struct FakePlcResolver {
    base_url: String,
    shutdown: std::sync::Arc<std::sync::atomic::AtomicBool>,
    join: Option<std::thread::JoinHandle<()>>,
}

impl FakePlcResolver {
    /// Host the real-`z6Mk` DID document(s) for `dids` (each derived via the
    /// deterministic `FixtureKeypair`), serving each at `GET /<did>`.
    pub fn start(dids: &[&str]) -> Self {
        use std::io::{Read, Write};
        use std::net::TcpListener;
        use std::sync::atomic::Ordering;

        // Materialize, ONCE at construction, a map from request path (`/<did>`)
        // to the canonical DID-document JSON body carrying the real z6Mk value.
        let docs: std::collections::HashMap<String, String> = dids
            .iter()
            .map(|did| {
                let fixture = openlore_test_support::did_doc_for(did);
                let body = did_document_json(&fixture);
                (format!("/{did}"), body)
            })
            .collect();

        let listener = TcpListener::bind("127.0.0.1:0").expect("FakePlcResolver: bind 127.0.0.1:0");
        listener
            .set_nonblocking(true)
            .expect("FakePlcResolver: set_nonblocking");
        let local_addr = listener.local_addr().expect("FakePlcResolver: local_addr");
        let base_url = format!("http://{local_addr}");

        let shutdown = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let shutdown_for_thread = std::sync::Arc::clone(&shutdown);

        let join = std::thread::Builder::new()
            .name("fake-plc-resolver".to_string())
            .spawn(move || {
                while !shutdown_for_thread.load(Ordering::SeqCst) {
                    match listener.accept() {
                        Ok((mut stream, _peer)) => {
                            let mut buf = [0u8; 2048];
                            let read = stream.read(&mut buf).unwrap_or(0);
                            let request = String::from_utf8_lossy(&buf[..read]);
                            // Parse the request-target out of the request line
                            // (`GET /<did> HTTP/1.1`). The path is percent-encoded
                            // by the client (DIDs carry `:`), so decode it before
                            // looking it up.
                            let path = request
                                .lines()
                                .next()
                                .and_then(|line| line.split_whitespace().nth(1))
                                .map(percent_decode_path)
                                .unwrap_or_default();
                            let response = match docs.get(&path) {
                                Some(body) => format!(
                                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\
                                     Content-Length: {}\r\nConnection: close\r\n\r\n{}",
                                    body.len(),
                                    body
                                ),
                                None => "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\
                                     Connection: close\r\n\r\n"
                                    .to_string(),
                            };
                            let _ = stream.write_all(response.as_bytes());
                            let _ = stream.flush();
                        }
                        Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                            std::thread::sleep(std::time::Duration::from_millis(5));
                        }
                        Err(_) => return,
                    }
                }
            })
            .expect("FakePlcResolver: spawn acceptor thread");

        Self {
            base_url,
            shutdown,
            join: Some(join),
        }
    }

    /// The `http://127.0.0.1:<port>` base URL the indexer resolves DID documents
    /// from (wired via `OPENLORE_INDEXER_PLC_ENDPOINT`).
    pub fn endpoint_url(&self) -> &str {
        &self.base_url
    }
}

impl Drop for FakePlcResolver {
    fn drop(&mut self) {
        self.shutdown
            .store(true, std::sync::atomic::Ordering::SeqCst);
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

/// Render a W3C DID document JSON carrying the fixture's REAL `z6Mk...`
/// `publicKeyMultibase` on the `#org.openlore.application` verification method —
/// the exact shape the production resolve path parses (ADR-026 §"Locate the
/// verification method").
fn did_document_json(fixture: &openlore_test_support::DidDocFixture) -> String {
    serde_json::json!({
        "id": fixture.did,
        "alsoKnownAs": [format!("at://{}.test", short_handle(&fixture.did))],
        "verificationMethod": [{
            "id": fixture.key_id.0,
            "type": "Multikey",
            "controller": fixture.did,
            "publicKeyMultibase": fixture.public_key_multibase,
        }],
        "service": [{
            "id": "#atproto_pds",
            "type": "AtprotoPersonalDataServer",
            "serviceEndpoint": "https://pds.example.test",
        }]
    })
    .to_string()
}

/// A short handle stem from a DID (`did:plc:priya-test` → `priya-test`).
fn short_handle(did: &str) -> String {
    did.rsplit(':').next().unwrap_or(did).to_string()
}

/// Decode the `%XX` escapes a client puts in the request-target so DID `:` (and
/// any other reserved char) round-trips back to the raw DID for the lookup.
fn percent_decode_path(path: &str) -> String {
    let bytes = path.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hi = (bytes[i + 1] as char).to_digit(16);
            let lo = (bytes[i + 2] as char).to_digit(16);
            if let (Some(hi), Some(lo)) = (hi, lo) {
                out.push((hi * 16 + lo) as u8);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Run `openlore-indexer <args>` against a fake ingest `source_url` and a fixture
/// PLC resolver `plc_endpoint` — with the slice-03 pubkey seam UNSET (AV-4 gold
/// path). Forces the indexer down the REAL `z6Mk` PLC-document resolve + decode
/// path (a seam-only impl could not resolve any key → would reject every record).
///
/// Mirrors [`run_openlore_indexer_with_source`] but threads
/// `OPENLORE_INDEXER_PLC_ENDPOINT` instead of any `OPENLORE_PEER_PUBKEY_HEX_<did>`
/// seam. `env_clear()` guarantees the seam is absent.
///
/// ## Test-isolation contract (load-bearing under full-workspace `cargo test`)
///
/// `env_clear()` is what keeps the `OPENLORE_PEER_PUBKEY_HEX_<did>` seam UNSET
/// (AV-4's whole point: prove the REAL `z6Mk` PLC decode ran, not the seam). But
/// `env_clear()` ALSO drops the per-scenario `OPENLORE_HOME` — and without it the
/// indexer's `default_index_path()` would fall back to a `OPENLORE_HOME`-anchored
/// (and ultimately CWD-anchored) SHARED default `index.duckdb`, colliding with
/// other cli test binaries' indexer state when many run concurrently. So after
/// `env_clear()` we RE-SET, in this exact order, the per-scenario isolation the
/// indexer needs so it uses its OWN tempdir-backed index — never a shared default:
///   - `OPENLORE_HOME`               → this scenario's `TestEnv` tempdir home;
///   - `OPENLORE_INDEXER_INDEX_PATH` → the scenario's SEPARATE `index.duckdb`
///     under that home (the indexer reads this FIRST, so even if the
///     `OPENLORE_HOME` fallback regressed the index path stays isolated);
///   - `OPENLORE_INDEXER_SOURCE_URL` / `OPENLORE_INDEXER_PLC_ENDPOINT` → this
///     scenario's fake source + fixture PLC resolver.
/// The seam stays UNSET (NOT re-set) — re-introducing it would defeat AV-4.
/// This mirrors [`run_openlore_indexer_with_source`]'s clean-env-plus-isolation
/// discipline. Removing any of these `.env(...)` lines reintroduces the
/// full-workspace flake.
pub fn run_openlore_indexer_with_plc_resolver(
    env: &TestEnv,
    args: &[&str],
    source_url: &str,
    plc_endpoint: &str,
) -> CliOutcome {
    let bin = assert_cmd::cargo::cargo_bin("openlore-indexer");
    let output = Command::new(&bin)
        .args(args)
        .env_clear()
        // Per-scenario isolation re-set after env_clear() — see the contract above.
        // (The OPENLORE_PEER_PUBKEY_HEX_<did> seam is deliberately NOT re-set.)
        .env("OPENLORE_HOME", &env.home)
        .env("OPENLORE_INDEXER_INDEX_PATH", index_duckdb_path(env))
        .env("OPENLORE_INDEXER_SOURCE_URL", source_url)
        .env("OPENLORE_INDEXER_PLC_ENDPOINT", plc_endpoint)
        .env("PATH", std::env::var("PATH").unwrap_or_default())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .unwrap_or_else(|e| panic!("spawn openlore-indexer at {bin:?}: {e}"));
    CliOutcome {
        status: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    }
}

/// One indexed row, projected from `index.duckdb` for assertions (port-exposed
/// observable surface of the ingest pass — never an internal store struct).
#[derive(Debug, Clone, PartialEq)]
pub struct IndexedRow {
    pub author_did: String,
    pub cid: String,
    pub subject: String,
    pub object: String,
    pub verified_against: String,
}

/// Read every `indexed_claims` row attributed to `author_did` from the
/// indexer's `index.duckdb` (read-only; the AV-1 "is it searchable?" assertion).
/// Test-support is the only place raw SQL is acceptable; production goes through
/// `IndexStorePort`. Returns rows with a NON-`Option` `author_did` (mirrors the
/// type-level anti-merging contract; we SELECT `author_did` explicitly).
pub fn read_indexed_claims_by_object(env: &TestEnv, object: &str) -> Vec<IndexedRow> {
    let db_path = index_duckdb_path(env);
    let conn = duckdb::Connection::open(&db_path).unwrap_or_else(|err| {
        panic!(
            "open index.duckdb at {} for AV-1 searchable assertion: {err}",
            db_path.display()
        )
    });
    let mut stmt = conn
        .prepare(
            "SELECT author_did, cid, subject, object, verified_against \
             FROM indexed_claims WHERE object = ?",
        )
        .unwrap_or_else(|err| panic!("prepare indexed_claims read: {err}"));
    let rows = stmt
        .query_map(duckdb::params![object], |row| {
            Ok(IndexedRow {
                author_did: row.get(0)?,
                cid: row.get(1)?,
                subject: row.get(2)?,
                object: row.get(3)?,
                verified_against: row.get(4)?,
            })
        })
        .unwrap_or_else(|err| panic!("query indexed_claims by object: {err}"));
    rows.map(|r| r.expect("decode indexed_claims row"))
        .collect()
}

/// Read every `indexed_claims` row attributed to `author_did` (the Contributor
/// search dimension) from the indexer's `index.duckdb`. Mirrors
/// [`read_indexed_claims_by_object`] for the AV-3 search-across-every-dimension
/// absence assertion. Test-support is the only place raw SQL is acceptable;
/// production goes through `IndexStorePort::query_by_contributor`.
pub fn read_indexed_claims_by_contributor(env: &TestEnv, author_did: &str) -> Vec<IndexedRow> {
    read_indexed_rows_where(env, "author_did = ?", author_did)
}

/// Read every `indexed_claims` row for `subject` (the Subject search dimension).
/// Mirrors [`read_indexed_claims_by_object`]. Production goes through
/// `IndexStorePort::query_by_subject`.
pub fn read_indexed_claims_by_subject(env: &TestEnv, subject: &str) -> Vec<IndexedRow> {
    read_indexed_rows_where(env, "subject = ?", subject)
}

/// Read EVERY `indexed_claims` CID (no filter) — the universe of what is actually
/// in the index, used by the AV-3 absence assertion to prove the three
/// adversarial CIDs produced NO row.
pub fn read_all_indexed_cids(env: &TestEnv) -> Vec<String> {
    let db_path = index_duckdb_path(env);
    let conn = duckdb::Connection::open(&db_path).unwrap_or_else(|err| {
        panic!(
            "open index.duckdb at {} for AV-3 row-count assertion: {err}",
            db_path.display()
        )
    });
    let mut stmt = conn
        .prepare("SELECT cid FROM indexed_claims")
        .unwrap_or_else(|err| panic!("prepare all-cids read: {err}"));
    let rows = stmt
        .query_map([], |row| row.get::<_, String>(0))
        .unwrap_or_else(|err| panic!("query all indexed cids: {err}"));
    rows.map(|r| r.expect("decode cid")).collect()
}

/// Shared single-bind `indexed_claims` projection used by the dimension readers.
fn read_indexed_rows_where(env: &TestEnv, where_clause: &str, bind: &str) -> Vec<IndexedRow> {
    let db_path = index_duckdb_path(env);
    let conn = duckdb::Connection::open(&db_path).unwrap_or_else(|err| {
        panic!(
            "open index.duckdb at {} for AV-3 dimension read: {err}",
            db_path.display()
        )
    });
    let sql = format!(
        "SELECT author_did, cid, subject, object, verified_against \
         FROM indexed_claims WHERE {where_clause}"
    );
    let mut stmt = conn
        .prepare(&sql)
        .unwrap_or_else(|err| panic!("prepare dimension read: {err}"));
    let rows = stmt
        .query_map(duckdb::params![bind], |row| {
            Ok(IndexedRow {
                author_did: row.get(0)?,
                cid: row.get(1)?,
                subject: row.get(2)?,
                object: row.get(3)?,
                verified_against: row.get(4)?,
            })
        })
        .unwrap_or_else(|err| panic!("query indexed_claims by dimension: {err}"));
    rows.map(|r| r.expect("decode indexed_claims row"))
        .collect()
}

/// Universe-bound: "the indexer's `index.duckdb` contains NO table whose name
/// implies a merged / consensus / aggregate / summary projection across authors".
/// Port-exposed name: `index_storage.schema.no_merge_table_present`.
///
/// The load-bearing ABSENCE (WD-103 / I-AV-2): the ADR-025 schema is exactly
/// `indexed_claims` + `indexed_claim_evidence` + `indexed_claim_references` +
/// `index_schema_version` — NONE of which is a cross-author merge. Aggregates
/// (`distinct_author_count`) are composed in the PURE `appview-domain` core at
/// QUERY time, NEVER stored as a merged row. This is the structural complement
/// to the type-level (ports non-`Option` `author_did`) + behavioral (AV-9)
/// anti-merging layers. Introspects the live `information_schema.tables` rather
/// than asserting against a hard-coded allow-list so a future merge table CANNOT
/// slip in unnoticed. Test-support is the only place raw SQL is acceptable.
pub fn assert_no_merged_consensus_table(env: &TestEnv) {
    let db_path = index_duckdb_path(env);
    let conn = duckdb::Connection::open(&db_path).unwrap_or_else(|err| {
        panic!(
            "open index.duckdb at {} for no-merge-schema assertion: {err}",
            db_path.display()
        )
    });
    let mut stmt = conn
        .prepare(
            "SELECT table_name FROM information_schema.tables \
             WHERE table_schema NOT IN ('information_schema', 'pg_catalog')",
        )
        .unwrap_or_else(|err| panic!("prepare schema introspection: {err}"));
    let table_names: Vec<String> = stmt
        .query_map([], |row| row.get::<_, String>(0))
        .unwrap_or_else(|err| panic!("query information_schema.tables: {err}"))
        .map(|r| r.expect("decode table_name"))
        .collect();

    // Any table whose name suggests a cross-author roll-up is forbidden — the
    // absence IS the WD-103 design. Substring match (case-insensitive) so a
    // `claim_consensus` or `author_aggregate` variant is caught too.
    const BANNED_SUBSTRINGS: &[&str] = &["consensus", "merged", "aggregate", "summary"];
    for name in &table_names {
        let lowered = name.to_ascii_lowercase();
        for banned in BANNED_SUBSTRINGS {
            assert!(
                !lowered.contains(banned),
                "anti-merging at ingest (WD-103 / I-AV-2): index.duckdb must contain NO \
                 merge/consensus/aggregate table; found table {name:?} matching banned \
                 substring {banned:?}. The full table set was: {table_names:?}"
            );
        }
    }
}

/// Universe-bound (AV-7 / WD-105 / I-AV-4): "the indexer's ingest pass touched
/// ONLY the PUBLIC `listRecords` surface — every recorded request hit
/// `com.atproto.repo.listRecords` with NO `Authorization` header, the indexer
/// made AT LEAST one such public read, and it NEVER hit the auth-scoped/private
/// tripwire". Port-exposed name: `ingest_source.requests.public_only`.
///
/// This is the ingest-side half of the public-data honesty contract: the indexer
/// reads only the unauthenticated public surface — no auth-scoped read, no
/// Authorization header — so private records can never enter the index. The
/// search-side user-visible banner is AV-10 (`appview_search.rs`).
pub fn assert_ingest_read_public_records_only(source: &FakeIngestServer) {
    let requests = source.recorded_requests();

    // The indexer must actually have pulled the public surface (otherwise the
    // "no auth-scoped call" witness would be vacuously true on a no-op).
    let public_reads = requests
        .iter()
        .filter(|r| r.is_public_list_records())
        .count();
    assert!(
        public_reads >= 1,
        "expected the indexer to PULL the public listRecords surface at least once \
         (public-data-only ingest, WD-105); recorded requests: {requests:?}"
    );

    // EVERY request the indexer made must be the PUBLIC listRecords surface — no
    // auth-scoped/private endpoint was ever called (the tripwire never fired).
    let auth_scoped: Vec<&RecordedIngestRequest> =
        requests.iter().filter(|r| r.is_auth_scoped()).collect();
    assert!(
        auth_scoped.is_empty(),
        "public-data-only violation (WD-105 / I-AV-4): the indexer hit the \
         auth-scoped/private tripwire surface — it must read ONLY the public \
         listRecords surface; offending requests: {auth_scoped:?}"
    );

    // NO request carried an Authorization header — the public surface is read
    // UNAUTHENTICATED (an Authorization header is the marker of an auth-scoped
    // read even against the listRecords path).
    let authenticated: Vec<&RecordedIngestRequest> = requests
        .iter()
        .filter(|r| r.had_authorization_header)
        .collect();
    assert!(
        authenticated.is_empty(),
        "public-data-only violation (WD-105 / I-AV-4): the indexer sent an \
         Authorization header — it must read the public surface UNAUTHENTICATED; \
         offending requests: {authenticated:?}"
    );

    // Every recorded request must be the public surface (defense-in-depth: catches
    // any OTHER non-public path the indexer might reach for).
    for request in &requests {
        assert!(
            request.is_public_list_records(),
            "public-data-only violation (WD-105 / I-AV-4): the indexer reached a \
             non-public endpoint {:?}; it must read ONLY the public listRecords \
             surface",
            request.path
        );
    }
}

// =============================================================================
// Slice-05 (step 03-05; AV-5) — the capability-boundary harness (ADR-023 / I-AV-5).
//
// The behavioral layer of the three-layer capability-boundary enforcement (type:
// the verify-only/read-only ports; structural: the `xtask check-arch`
// `indexer_holds_no_signing_or_local_store` rule; behavioral: AV-5 + the
// composition-root `capability_boundary_probe`). These helpers observe ONLY the
// port-exposed surface — the indexer's CLI help verb-set + the FILESYSTEM
// (the user's `openlore.duckdb` byte-state, the indexer's own `index.duckdb`) —
// NEVER an internal store struct field.
// =============================================================================

/// Universe-bound: "the `openlore-indexer` help/usage surface lists ONLY the
/// `serve` + `ingest` + `stats` verbs and exposes NO `sign` / `publish` /
/// `claim add` verb (it is signing-INCAPABLE, ADR-023 / I-AV-5)". Port-exposed
/// name: `indexer.cli.help_verb_set`.
///
/// Runs the REAL `openlore-indexer --help` subprocess (NOT the ingest path — the
/// help surface short-circuits BEFORE the wire → probe → use gate) and asserts:
///   1. each of `serve`, `ingest`, `stats` appears in the help text, AND
///   2. NONE of the signing/authoring verbs (`sign`, `publish`, `add`) appear as
///      a subcommand — the absence IS the capability boundary.
pub fn assert_indexer_help_has_no_signing_verb(env: &TestEnv) {
    let outcome = run_openlore_indexer_help(env);
    assert_eq!(
        outcome.status, 0,
        "openlore-indexer --help must exit 0; got {} \n--- stdout ---\n{}\n--- stderr ---\n{}",
        outcome.status, outcome.stdout, outcome.stderr
    );

    // clap prints the help to stdout for `--help`. The subcommand list lives in
    // a `Commands:` section; assert the expected verb-set is fully present.
    let help = &outcome.stdout;
    for verb in ["serve", "ingest", "stats"] {
        assert!(
            help.contains(verb),
            "expected `openlore-indexer --help` to list the {verb:?} verb \
             (the indexer's only verbs are serve/ingest/stats); \n--- help ---\n{help}"
        );
    }

    // The load-bearing ABSENCE (ADR-023 / I-AV-5): the indexer exposes NO
    // signing/authoring verb. We scan the help for each banned authoring verb as
    // a whole word so a substring like `ingest` (which contains no banned verb)
    // or an unrelated word cannot false-positive. A `claim add` / `sign` /
    // `publish` subcommand WOULD appear as a standalone token in the help.
    const BANNED_VERBS: &[&str] = &["sign", "publish", "add"];
    let help_lower = help.to_ascii_lowercase();
    for banned in BANNED_VERBS {
        let appears_as_word = help_lower
            .split(|c: char| !c.is_ascii_alphanumeric())
            .any(|tok| tok == *banned);
        assert!(
            !appears_as_word,
            "capability boundary (ADR-023 / I-AV-5): `openlore-indexer` must expose NO \
             {banned:?} verb — it is signing-INCAPABLE. Found {banned:?} as a token in the \
             help/usage surface: \n--- help ---\n{help}"
        );
    }
}

/// Run `openlore-indexer --help` under the scenario's isolated `OPENLORE_HOME`.
/// Mirrors [`run_openlore_indexer_with_source`]'s clean-env discipline but threads
/// NO source/index seam — the help surface short-circuits before any wiring.
pub fn run_openlore_indexer_help(env: &TestEnv) -> CliOutcome {
    let bin = assert_cmd::cargo::cargo_bin("openlore-indexer");
    let output = Command::new(&bin)
        .args(["--help"])
        .env_clear()
        .env("OPENLORE_HOME", &env.home)
        .env("PATH", std::env::var("PATH").unwrap_or_default())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .unwrap_or_else(|e| panic!("spawn openlore-indexer --help at {bin:?}: {e}"));
    CliOutcome {
        status: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    }
}

/// A snapshot of the user's `openlore.duckdb` byte-state, taken BEFORE an indexer
/// pass so [`assert_user_openlore_duckdb_unchanged`] can prove the indexer never
/// opened or wrote it (the indexer has NO handle to the user's local store).
///
/// `Absent` is the strongest possible form of "untouched": if the indexer has no
/// handle to the user store, an ingest pass must NOT create it.
#[derive(Debug, Clone, PartialEq)]
pub enum UserStoreSnapshot {
    /// The user `openlore.duckdb` did not exist when the snapshot was taken.
    Absent,
    /// The user `openlore.duckdb` existed; we recorded its exact bytes.
    Present { bytes: Vec<u8> },
}

/// Snapshot the user's `openlore.duckdb` (`env.duckdb_path()`) byte-state. Records
/// `Absent` if the file does not yet exist, else the full byte content (the
/// strongest unchanged-witness: we compare bytes, not just mtime/size).
pub fn snapshot_user_openlore_duckdb(env: &TestEnv) -> UserStoreSnapshot {
    let path = env.duckdb_path();
    match std::fs::read(&path) {
        Ok(bytes) => UserStoreSnapshot::Present { bytes },
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => UserStoreSnapshot::Absent,
        Err(err) => panic!("snapshot user openlore.duckdb at {}: {err}", path.display()),
    }
}

/// Universe-bound: "the user's `openlore.duckdb` is byte-identical to (or still as
/// absent as) the `before` snapshot — the indexer never opened or wrote it".
/// Port-exposed name: `user_storage.openlore_duckdb.bytes`.
///
/// The behavioral half of the capability boundary (ADR-023 / I-AV-5): the indexer
/// holds NO handle to the user's local store, so an ingest pass cannot mutate it.
/// `Absent → Absent` (never created) and `Present → byte-identical` both prove the
/// store was untouched; an `Absent → Present` transition (or any byte change) is a
/// capability-boundary breach.
pub fn assert_user_openlore_duckdb_unchanged(env: &TestEnv, before: &UserStoreSnapshot) {
    let after = snapshot_user_openlore_duckdb(env);
    match (before, &after) {
        (UserStoreSnapshot::Absent, UserStoreSnapshot::Absent) => {}
        (UserStoreSnapshot::Absent, UserStoreSnapshot::Present { .. }) => panic!(
            "capability boundary breach (ADR-023 / I-AV-5): the user's openlore.duckdb at {} \
             did NOT exist before the indexer pass but EXISTS after — the indexer must have NO \
             handle to the user's local store and must never create it",
            env.duckdb_path().display()
        ),
        (UserStoreSnapshot::Present { .. }, UserStoreSnapshot::Absent) => panic!(
            "the user's openlore.duckdb at {} existed before the indexer pass but is GONE after \
             — the indexer must never delete the user's local store",
            env.duckdb_path().display()
        ),
        (
            UserStoreSnapshot::Present {
                bytes: before_bytes,
            },
            UserStoreSnapshot::Present { bytes: after_bytes },
        ) => {
            assert_eq!(
                before_bytes.len(),
                after_bytes.len(),
                "capability boundary breach (ADR-023 / I-AV-5): the user's openlore.duckdb at {} \
                 changed SIZE across the indexer pass ({} → {} bytes) — the indexer must never \
                 open or write the user's local store",
                env.duckdb_path().display(),
                before_bytes.len(),
                after_bytes.len()
            );
            assert!(
                before_bytes == after_bytes,
                "capability boundary breach (ADR-023 / I-AV-5): the user's openlore.duckdb at {} \
                 changed CONTENT across the indexer pass — the indexer must never open or write \
                 the user's local store",
                env.duckdb_path().display()
            );
        }
    }
}

/// Seed a populated user `openlore.duckdb` (own claims) so the AV-5 precondition
/// ("a TestEnv with a populated user openlore.duckdb") is exercised against a
/// REAL existing file — making the byte-unchanged witness load-bearing (the
/// indexer must leave a populated store byte-identical, not merely never create
/// one). Writes a small DuckDB at `env.duckdb_path()` with one `claims` row.
///
/// Test-support is the only place a raw `duckdb::Connection` write is acceptable;
/// production goes through `StoragePort`.
pub fn seed_user_openlore_duckdb(env: &TestEnv) {
    let db_path = env.duckdb_path();
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent)
            .unwrap_or_else(|err| panic!("create user store dir {}: {err}", parent.display()));
    }
    let conn = duckdb::Connection::open(&db_path)
        .unwrap_or_else(|err| panic!("seed user openlore.duckdb at {}: {err}", db_path.display()));
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS claims (cid VARCHAR PRIMARY KEY, subject VARCHAR); \
         INSERT INTO claims (cid, subject) VALUES ('bafy_user_own_claim', 'github:rust-lang/rust');",
    )
    .unwrap_or_else(|err| panic!("seed user claims row: {err}"));
    // Drop the connection (flushes + closes the WAL) so the snapshot reads a
    // settled file the indexer pass must not touch.
    drop(conn);
}

/// Universe-bound: "the indexer's OWN `index.duckdb` (the SEPARATE store) was
/// written by the ingest pass". Port-exposed name:
/// `index_storage.index_duckdb.written`.
///
/// The complement to [`assert_user_openlore_duckdb_unchanged`]: the indexer DID
/// persist to its OWN store (so the byte-unchanged witness on the user store is
/// not vacuously true because nothing ran). Asserts the file exists and is
/// non-empty under the indexer's data dir.
pub fn assert_index_duckdb_written(env: &TestEnv) {
    let path = index_duckdb_path(env);
    let meta = std::fs::metadata(&path).unwrap_or_else(|err| {
        panic!(
            "expected the indexer to have written its SEPARATE index.duckdb at {} \
             after an ingest pass; got {err}",
            path.display()
        )
    });
    assert!(
        meta.len() > 0,
        "the indexer's index.duckdb at {} exists but is EMPTY — the ingest pass must have \
         written the verified attributed row",
        path.display()
    );
}

// =============================================================================
// Slice-05 AV-13 (CARDINAL RELEASE GATE `local_first_preserved`; KPI-5) harness
// =============================================================================
//
// The AV-13 disprover drives offline authoring + soft search degradation with
// the discovery INDEXER deliberately UNREACHABLE. Unlike the AV-8..11 happy
// paths (which spawn a REAL `openlore-indexer serve` over a seeded index), AV-13
// points the CLI's `OPENLORE_INDEXER_URL` at a CLOSED localhost port (bound then
// dropped — the OS refuses the connect promptly) so:
//   - `claim add` / offline `claim publish` / `graph query --object` succeed
//     (the indexer is NOT probed at CLI startup, WD-116);
//   - `openlore search --object` degrades softly (the local-only message + the
//     `graph query` pointer), exits NON-fatally, and does NOT hang (the adapter's
//     bounded connect/request timeout returns `Unreachable` promptly).
//
// The user's OWN PDS (`env.pds`, the in-process `FakePds`) stays reachable — the
// cardinal KPI-5 claim is that an unreachable DISCOVERY indexer must not break
// authoring/publish; the user's own infrastructure is a separate concern.

/// A CLOSED localhost port: a `TcpListener` is bound to `127.0.0.1:0`, the
/// OS-assigned port is read back, then the listener is DROPPED so the port is
/// freed. A connect to the (now unbound) port is REFUSED promptly — the
/// fastest, most deterministic "unreachable indexer" seam (no live serve
/// process, no hang). Mirrors the slice-05 `IndexerHandle` shape but with NO
/// running server: `indexer_url()` points at a port nothing listens on.
///
/// The bounded-wall-clock guarantee (DESIGN_CONTEXT 3) lives in the adapter's
/// connect/request timeout; this seam just guarantees the address is a refused
/// connect by construction so the AT is deterministic and fast.
pub struct ClosedIndexerPort {
    url: String,
}

impl ClosedIndexerPort {
    /// Reserve then release a localhost port, returning a handle whose
    /// `indexer_url()` is a `http://127.0.0.1:<freed-port>` that nothing listens
    /// on (connect refused).
    pub fn reserve() -> Self {
        use std::net::TcpListener;
        let listener =
            TcpListener::bind("127.0.0.1:0").expect("ClosedIndexerPort: bind 127.0.0.1:0");
        let port = listener
            .local_addr()
            .expect("ClosedIndexerPort: read local_addr")
            .port();
        // Drop the listener — the OS frees the port; subsequent connects are
        // refused (no server is ever started on it).
        drop(listener);
        Self {
            url: format!("http://127.0.0.1:{port}"),
        }
    }

    /// The `http://127.0.0.1:<freed-port>` URL the CLI's `OPENLORE_INDEXER_URL`
    /// is pointed at — a closed port (connect refused).
    pub fn indexer_url(&self) -> &str {
        &self.url
    }
}

/// Run `openlore <args>` with the discovery indexer UNREACHABLE (the
/// `OPENLORE_INDEXER_URL` points at `closed`'s freed port) feeding `stdin_lines`
/// (newline-joined) on stdin. The user's OWN PDS (`env.pds`) stays reachable so
/// the authoring/publish path works — the cardinal KPI-5 claim is that an
/// unreachable DISCOVERY indexer must not block `claim add` / `claim publish` /
/// `graph query` (WD-116). Mirrors [`run_openlore_with_stdin`]'s clean-env
/// discipline + adds the closed-indexer seam.
///
/// Used by AV-13 sub-assertions 1-3 (the offline authoring verbs): the indexer
/// URL is set so we PROVE the CLI does not probe it at startup — `claim add`
/// must still exit 0.
pub fn run_openlore_unreachable_indexer(
    env: &TestEnv,
    args: &[&str],
    closed: &ClosedIndexerPort,
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
        // The user's OWN PDS stays reachable (the authoring/publish path needs
        // it). The thing under test is the DISCOVERY indexer being down.
        .env("OPENLORE_PDS_ENDPOINT", env.pds.endpoint_url())
        // The discovery indexer is UNREACHABLE: a closed/freed localhost port.
        // If the CLI hard-probed it at startup, `claim add` would fail — the
        // cardinal WD-116 disprover.
        .env("OPENLORE_INDEXER_URL", closed.indexer_url())
        .env("PATH", std::env::var("PATH").unwrap_or_default())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = cmd
        .spawn()
        .unwrap_or_else(|e| panic!("spawn openlore at {bin:?}: {e}"));
    if !stdin_lines.is_empty() {
        let stdin = child.stdin.as_mut().expect("stdin pipe");
        stdin
            .write_all(stdin_lines.as_bytes())
            .expect("write stdin");
    }
    drop(child.stdin.take());

    let output = child.wait_with_output().expect("wait_with_output");
    CliOutcome {
        status: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    }
}

/// The bounded-wall-clock outcome of a `search` against an UNREACHABLE indexer:
/// the captured [`CliOutcome`] plus `hung` — whether the invocation EXCEEDED the
/// bound (a connect-timeout, not an indefinite block). Port-exposed name:
/// `search.hung`.
pub struct BoundedSearchOutcome {
    pub outcome: CliOutcome,
    /// True iff the search did NOT return within the wall-clock bound — i.e. it
    /// hung. The AV-13 gate asserts this is FALSE (the adapter's bounded
    /// connect/request timeout returns `Unreachable` promptly).
    pub hung: bool,
}

/// Run `openlore search <args>` against an UNREACHABLE indexer (`closed`'s freed
/// port) under a BOUNDED wall-clock, returning the outcome + a `hung` flag.
///
/// The search subprocess is spawned on a worker thread and joined with a
/// `bound`-second deadline; if it has not returned by then we record `hung =
/// true` (and kill nothing — a genuinely hung subprocess would be the bug AV-13
/// disproves). A refused connect through the adapter's bounded
/// connect/request timeout resolves in well under the bound, so a healthy soft
/// degradation records `hung = false`. The user's own PDS stays reachable
/// (same clean-env discipline as [`run_openlore_search`], minus a live indexer).
pub fn run_openlore_search_bounded_unreachable(
    env: &TestEnv,
    args: &[&str],
    closed: &ClosedIndexerPort,
    bound: std::time::Duration,
) -> BoundedSearchOutcome {
    let bin = assert_cmd::cargo::cargo_bin("openlore");
    let home = env.home.clone();
    let did = env.identity.author_did().to_string();
    let seed = env.identity.seed_hex.clone();
    let pds = env.pds.endpoint_url().to_string();
    let indexer = closed.indexer_url().to_string();
    let owned_args: Vec<String> = args.iter().map(|a| a.to_string()).collect();

    let (tx, rx) = std::sync::mpsc::channel::<CliOutcome>();
    std::thread::spawn(move || {
        let output = Command::new(&bin)
            .args(&owned_args)
            .env_clear()
            .env("OPENLORE_HOME", &home)
            .env("OPENLORE_DID", &did)
            .env("OPENLORE_KEY_SEED_HEX", &seed)
            .env("OPENLORE_PDS_ENDPOINT", &pds)
            .env("OPENLORE_INDEXER_URL", &indexer)
            .env("PATH", std::env::var("PATH").unwrap_or_default())
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .unwrap_or_else(|e| panic!("spawn openlore at {bin:?}: {e}"));
        let _ = tx.send(CliOutcome {
            status: output.status.code().unwrap_or(-1),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        });
    });

    match rx.recv_timeout(bound) {
        Ok(outcome) => BoundedSearchOutcome {
            outcome,
            hung: false,
        },
        Err(_) => BoundedSearchOutcome {
            // The subprocess did not return within the bound — it hung. Surface a
            // placeholder outcome; the AV-13 gate asserts `hung == false`.
            outcome: CliOutcome {
                status: -1,
                stdout: String::new(),
                stderr: format!(
                    "search did not return within the {:?} bound — the indexer connect HUNG \
                     (no bounded timeout); KPI-5 / WD-116 violation",
                    bound
                ),
            },
            hung: true,
        },
    }
}

/// Recover the just-signed claim's CID from a `claim add` stdout. The
/// `claim_add` verb prints `Computing claim CID <cid>` right before persistence
/// (the load-bearing marker WS-6 keys on); this parses the CID token so AV-13
/// can publish the exact record without hard-coding a CID. Distinct from
/// [`published_cid_from_stdout`] (which keys on the publish-success block) —
/// AV-13 declines publishing in `claim add`, so the publish block is absent.
pub fn claim_add_cid_from_stdout(stdout: &str) -> String {
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
        "found 'Computing claim CID' marker but no CID followed it in stdout:\n{stdout}"
    );
    cid
}

/// Snapshot the LOCAL claims store as a sorted set of file names under
/// `{home}/.local/share/openlore/claims/`. Port-exposed observable surface:
/// `storage.local_claim_store.file_set`. Used by AV-13 to assert the local
/// store is mutated ONLY by the authoring verbs, never by `search` (the set is
/// unchanged across a search invocation).
pub fn local_claim_file_set(env: &TestEnv) -> Vec<String> {
    let dir = env.claims_dir();
    let mut names: Vec<String> = match std::fs::read_dir(&dir) {
        Ok(entries) => entries
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect(),
        // Absence == the empty set (no local claim written yet).
        Err(_) => Vec::new(),
    };
    names.sort();
    names
}

// =============================================================================
// AV-19 — the discovery→federation FUNNEL harness (US-AV-005 Ex1; KPI-AV-4 /
// I-AV-7). The funnel REUSES the slice-03 `peer add`/`peer pull` verbs + the
// slice-03 `PeerPds` double VERBATIM — there is NO new external fake and NO
// executable follow path. The render-only affordance prints the EXISTING
// slice-03 command (`openlore peer add <bare-did>`); the user runs THAT command
// to follow the discovered author. The subscription that results is an ordinary
// slice-03 add (no parallel discovery-subscription state — proven by AV-22's
// purge symmetry).
// =============================================================================

/// Parse the RENDER-ONLY follow affordance for `author_did_substr` out of a
/// `search` stdout and return the VERBATIM `openlore` argv it instructs the user
/// to run (e.g. `["peer", "add", "did:plc:priya-test"]`). The renderer emits the
/// affordance as `    Follow this author: openlore peer add <bare-did>` for an
/// unfollowed network author (`render::render_follow_affordance`, US-AV-005 /
/// I-AV-7); the funnel runs the returned argv UNCHANGED — it must be the SAME
/// slice-03 verb, never a new follow path.
///
/// Port-exposed: parses only the rendered stdout (the CLI driving-port
/// observable), never an internal struct. Panics with the full stdout if no
/// affordance line for the author is present (the affordance is the load-bearing
/// funnel seam — its absence is an AV-19 failure, not a silent skip).
pub fn parse_follow_affordance_command(stdout: &str, author_did_substr: &str) -> Vec<String> {
    const MARKER: &str = "Follow this author: openlore ";
    for line in stdout.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix(MARKER) {
            // The affordance line names the author's bare DID as the final token;
            // only return the one matching the requested author.
            if rest.contains(author_did_substr) {
                return rest.split_whitespace().map(|s| s.to_string()).collect();
            }
        }
    }
    panic!(
        "parse_follow_affordance_command: no `{MARKER}...` follow affordance for \
         author {author_did_substr:?} in search output:\n{stdout}"
    );
}

/// Run the AV-19 funnel's follow + pull steps REUSING the slice-03 verbs
/// VERBATIM. Given the `search` stdout that rendered Priya's follow affordance:
///
///   1. parse the rendered `openlore peer add <bare-did>` affordance argv;
///   2. start a slice-03 `PeerPds` double hosting Priya's bazel/reproducible-
///      builds claim (the SAME (subject, object, confidence) the index ingested,
///      so the LOCAL graph query finds it after the pull) — NO new external fake;
///   3. run the parsed `peer add` argv UNCHANGED against that `PeerPds` (the
///      slice-03 subscribe path; NO new verb, NO auto-follow);
///   4. run `openlore peer pull` (the slice-03 federation pull) so Priya's claim
///      lands in the LOCAL `peer_claims` store.
///
/// Returns the live `PeerPds` (the caller keeps it alive for the pull's HTTP
/// runtime) so a re-pull or a later assertion can reuse it. The (subject,
/// object, confidence) MUST mirror `corpus_reproducible_builds_nine_authors`'s
/// Priya entry (`github:bazelbuild/bazel`, `reproducible-builds`, 0.82).
pub fn funnel_follow_and_pull(env: &TestEnv, search_stdout: &str, priya_did: &str) -> PeerPds {
    // 1. The render-only affordance argv — the slice-03 `peer add <bare-did>`.
    let affordance = parse_follow_affordance_command(search_stdout, priya_did);
    assert_eq!(
        affordance.first().map(String::as_str),
        Some("peer"),
        "AV-19: the follow affordance MUST reuse the slice-03 `openlore peer add` \
         verb verbatim (no new follow path); got argv {affordance:?}"
    );
    assert_eq!(
        affordance.get(1).map(String::as_str),
        Some("add"),
        "AV-19: the follow affordance MUST be `peer add <did>` (the slice-03 \
         subscribe verb); got argv {affordance:?}"
    );
    let affordance_did = affordance
        .get(2)
        .cloned()
        .expect("the affordance argv names the author's bare DID");
    let affordance_args: Vec<&str> = affordance.iter().map(String::as_str).collect();

    // 2. The slice-03 `PeerPds` double serving Priya's claim — the SAME records
    //    the index ingested, materialized with REAL crypto so the pull's
    //    per-record verify + CID-recompute pass. NO new external fake.
    let priya_seed = [19u8; 32];
    let (records, priya_pubkey_hex) = build_verifiable_peer_records_for_triples(
        &affordance_did,
        priya_seed,
        &[(
            "github:bazelbuild/bazel",
            "org.openlore.philosophy.reproducible-builds",
            0.82,
        )],
    );
    let priya_peer = PeerPds::for_peer(&affordance_did, records);

    // 3. Run the rendered affordance argv UNCHANGED — the slice-03 `peer add`.
    let added = run_openlore_with_peer_resolver(
        env,
        &affordance_args,
        &affordance_did,
        priya_peer.endpoint_url(),
    );
    assert_eq!(
        added.status,
        0,
        "AV-19: running the rendered slice-03 follow affordance `openlore {}` \
         verbatim must exit 0;\n--- stdout ---\n{}\n--- stderr ---\n{}",
        affordance_args.join(" "),
        added.stdout,
        added.stderr
    );

    // 4. The slice-03 `peer pull` — Priya's claim lands in the LOCAL graph.
    let pulled = run_openlore_pull(
        env,
        &["peer", "pull"],
        &affordance_did,
        priya_peer.endpoint_url(),
        &priya_pubkey_hex,
    );
    assert_eq!(
        pulled.status, 0,
        "AV-19: `openlore peer pull` (the slice-03 federation pull) must exit 0 \
         after following the discovered author;\n--- stdout ---\n{}\n--- stderr ---\n{}",
        pulled.stdout, pulled.stderr
    );

    priya_peer
}

/// Assert the subscription created via the funnel is EXACTLY a slice-03 `peer
/// add` — one ACTIVE row in `peer_subscriptions` for `peer_did` (`removed_at IS
/// NULL`), with NO parallel discovery-subscription state (I-AV-7). This is the
/// SAME `peer_subscriptions` row state slice-03 PS-1 asserts for a plain `peer
/// add`; the funnel introduces no separate follow store. Port-exposed name:
/// `peer_storage.subscriptions.active_row_count[did]` — the slice-03 store, the
/// load-bearing absence of a parallel path.
pub fn assert_funnel_subscription_is_slice03(env: &TestEnv, peer_did: &str) {
    let db_path = env.duckdb_path();
    let conn = duckdb::Connection::open(&db_path).unwrap_or_else(|err| {
        panic!(
            "open DuckDB at {} for AV-19 subscription assertion: {err}",
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
        "AV-19: the funnel must create EXACTLY ONE `peer_subscriptions` row for \
         {peer_did} — a slice-03 add, no parallel discovery-subscription state \
         (I-AV-7); got {total} rows"
    );
    assert_eq!(
        active, 1,
        "AV-19: the funnel's `peer_subscriptions` row for {peer_did} must be \
         ACTIVE (removed_at IS NULL) — exactly as a slice-03 `peer add`; got \
         {active} active rows"
    );
}

/// AV-20 (US-AV-005 Ex3 / I-AV-7): snapshot the FULL `peer_subscriptions` table —
/// the port-exposed observable that `openlore peer list` renders (the subscription
/// state a user inspects). Returns every row as a deterministic, sorted list of
/// stringified `(peer_did, peer_handle, peer_pds_endpoint, subscribed_at,
/// removed_at)` tuples so two snapshots are byte-comparable across a `search` +
/// `--show` invocation.
///
/// Port-exposed name: `peer_storage.subscriptions.rows`. Test-support is the only
/// place raw SQL is acceptable; production reads go through `PeerStoragePort`. The
/// baseline may be EMPTY — a fresh env has no DB file (no peer-write path opened it
/// yet) or no `peer_subscriptions` table; both are the strongest form of "zero
/// subscriptions", so absence maps to an empty snapshot (NOT a panic).
pub fn peer_subscriptions_snapshot(env: &TestEnv) -> Vec<String> {
    let db_path = env.duckdb_path();
    // A fresh env may have no DB file at all (no write path ever opened it) — that
    // is an EMPTY subscription state, the AV-20 baseline.
    if !db_path.exists() {
        return Vec::new();
    }
    let conn = duckdb::Connection::open(&db_path).unwrap_or_else(|err| {
        panic!(
            "open DuckDB at {} for the AV-20 peer_subscriptions snapshot: {err}",
            db_path.display()
        )
    });
    // The `peer_subscriptions` table may not exist if no peer-write migration ran;
    // a failed query is therefore also the EMPTY subscription state. Every column is
    // cast to text so the heterogeneous timestamp/nullable slots compare as strings.
    let mut rows: Vec<String> = match conn.prepare(
        "SELECT \
            CAST(peer_did AS VARCHAR), \
            CAST(peer_handle AS VARCHAR), \
            CAST(peer_pds_endpoint AS VARCHAR), \
            CAST(subscribed_at AS VARCHAR), \
            CAST(COALESCE(CAST(removed_at AS VARCHAR), '<active>') AS VARCHAR) \
         FROM peer_subscriptions",
    ) {
        Ok(mut stmt) => {
            let mapped = stmt.query_map([], |r| {
                Ok(format!(
                    "peer_did={} | handle={} | pds={} | subscribed_at={} | removed_at={}",
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, String>(4)?,
                ))
            });
            match mapped {
                Ok(iter) => iter
                    .map(|r| r.expect("read peer_subscriptions row"))
                    .collect(),
                Err(_) => return Vec::new(),
            }
        }
        // No such table → empty subscription state (no peer-write path ran).
        Err(_) => return Vec::new(),
    };
    // Deterministic order so the before/after comparison is stable regardless of
    // DuckDB's row-return order.
    rows.sort();
    rows
}

/// AV-20 / RELEASE-GATE-ADJACENT (US-AV-005 Ex3 / I-AV-7): assert that discovery
/// (`openlore search` along any dimension + `--show`) NEVER auto-subscribes — the
/// `peer_subscriptions` state (what `peer list` renders) is BYTE-IDENTICAL before
/// and after. Discovery is read-only; following is always an explicit, separate
/// human action; the render-only follow affordance never executes a follow.
///
/// Port-exposed name: `peer_storage.subscriptions.rows` before == after. Asserts on
/// the OBSERVABLE subscription surface, never an internal adapter field — refactor
/// stays GREEN.
pub fn assert_subscriptions_unchanged(before: &[String], after: &[String]) {
    assert_eq!(
        after, before,
        "AV-20 (US-AV-005 Ex3 / I-AV-7): `openlore search` + `--show` must NEVER \
         auto-subscribe — the `peer_subscriptions` state (`peer list`) must be \
         UNCHANGED across discovery. Following is always an explicit human action; \
         the render-only affordance never executes a follow.\n\
         before: {before:?}\nafter:  {after:?}"
    );
}

// =============================================================================
// Slice-06 (htmx-scraper-viewer) — the `openlore ui` viewer harness.
//
// DISTILL builds the doubles/harness for slice-06 the way slice-02 built
// FakeGithub: a `ViewerServer` spawn helper that drives the NEW long-running
// `openlore ui --port <P>` verb (ADR-028) as a subprocess, waits for readiness,
// and exposes `get` / `post_form` HTTP helpers over the returned HTML.
//
// Hexagonal discipline (hard requirement): scenarios drive the CLI driving port
// (`openlore ui` subprocess) + HTTP — NEVER the `viewer-domain` render fns
// directly (those are unit-level, exercised in DELIVER). The external GitHub API
// is the ONLY mocked boundary (reused `FakeGithub` via `GithubServer`); the local
// DuckDB store is REAL (BR-VIEW-4 — the SAME store the CLI writes).
//
// Layer placement (nw-tdd-methodology Layered Test Discipline matrix): every
// viewer scenario is a layer-3/layer-5 subprocess + real-I/O test — example-only
// (Mandate 11). Sad paths (unreadable store, unknown CID, zero candidates,
// network down) are enumerated explicitly, never PBT-generated.
//
// Build-before-run note (carry into DELIVER roadmap, mirrors the indexer ATs):
// `cargo test` does NOT rebuild a spawned binary automatically — the roadmap/run
// MUST `cargo build` the `openlore` bin before running these viewer ATs so the
// `ViewerServer` spawns the CURRENT `openlore ui`, not a stale one.
//
// SCAFFOLD: true (slice-06) — `ViewerServer::start` spawns `openlore ui` (resolved
// at RUNTIME via `assert_cmd::cargo_bin`), so this helper COMPILES now even though
// the `ui` verb does not exist yet; the scenarios fail at RUNTIME (correct RED).
// =============================================================================

/// One HTTP response from the viewer: the status code + the response body
/// (rendered HTML). The viewer is server-rendered HTML (ADR-028/029 — maud), so
/// `body` is the HTML the operator's browser would display.
#[derive(Debug, Clone)]
pub struct ViewerResponse {
    /// HTTP status code (200, 404, ...). Read-only views are 200; the `*` guided
    /// not-found route is 404 (DESIGN §5 route table).
    pub status: u16,
    /// The rendered HTML body the browser displays.
    pub body: String,
    /// The response `Content-Type` header, lowercased (empty when absent). The
    /// asset route (`/static/htmx.min.js`) serves a JS content-type; the page
    /// routes serve `text/html` — H-5a asserts the asset's JS-ish type on this
    /// observable header (the browser keys script execution off it).
    pub content_type: String,
}

impl ViewerResponse {
    /// Convenience: does the rendered HTML contain `needle`? (case-sensitive).
    /// Scenarios assert on OBSERVABLE rendered text (what the operator SEES),
    /// never internal struct fields (Mandate 8 universe = port-exposed surface).
    pub fn body_contains(&self, needle: &str) -> bool {
        self.body.contains(needle)
    }

    /// Does this response carry full-page CHROME? slice-06 full pages are complete
    /// maud documents that emit `<!DOCTYPE html>` + `<html>` + the `<title>OpenLore`
    /// chrome (viewer-domain `render_*_page`). An htmx FRAGMENT carries NONE of that
    /// — it is just the swap-target `<div id=...>` region. This is the observable
    /// discriminator the slice-07 scenarios assert on (I-HX-1): a fragment must NOT
    /// be a full page, and a no-JS response MUST be a full page.
    pub fn is_full_page(&self) -> bool {
        let lower = self.body.to_lowercase();
        lower.contains("<!doctype html>") && lower.contains("<html")
    }

    /// The inverse discriminator: a true htmx FRAGMENT has no full-page chrome
    /// (no `<!DOCTYPE html>`, no `<html>`, no `<head>`/`<title>` shell). Asserting
    /// `is_fragment()` on an `HX-Request` response proves the swap target was
    /// returned ALONE (NFR-HX-6 in-place feel; I-HX-1 shape selection).
    pub fn is_fragment(&self) -> bool {
        !self.is_full_page()
    }

    /// Does the rendered HTML reference an OFF-HOST URL to load the htmx library?
    /// The offline-first guarantee (I-HX-2 / US-HX-005) requires the htmx asset to
    /// be served by the viewer ITSELF (loopback `/static/htmx.min.js` or inlined) —
    /// NEVER a CDN. This scans for the well-known htmx CDN hosts; a `true` result is
    /// an offline-guarantee breach. Used by the no-CDN gold scenario.
    pub fn references_external_cdn(&self) -> bool {
        let lower = self.body.to_lowercase();
        [
            "unpkg.com",
            "cdn.jsdelivr.net",
            "jsdelivr",
            "cdnjs",
            "//cdn.",
        ]
        .iter()
        .any(|host| lower.contains(host))
    }

    /// Does the response's `Content-Type` advertise JavaScript? The browser keys
    /// `<script src>` execution off the served type, so the local htmx asset MUST
    /// arrive as a JS-ish content-type (`application/javascript` or the legacy
    /// `text/javascript`) — H-5a (US-HX-005 / FR-HX-6). The optional `; charset`
    /// suffix is tolerated (we match the media-type prefix).
    pub fn content_type_looks_like_javascript(&self) -> bool {
        self.content_type.contains("application/javascript")
            || self.content_type.contains("text/javascript")
    }
}

/// A handle to a REAL long-running `openlore ui --port 0` process bound to a
/// localhost EPHEMERAL port (`:0`, read back — parallel-safe per the slice-05
/// indexer-serve precedent). Owns the child process; killed on drop (RAII
/// per-scenario isolation, mirroring `IndexerHandle` / `FakePds` / `PeerPds`).
///
/// The viewer reads the SAME `OPENLORE_HOME`-resolved DuckDB the CLI verbs write
/// (BR-VIEW-4) and, for `/scrape`, reaches GitHub through the
/// `OPENLORE_GITHUB_API_BASE` seam (so the reused `FakeGithub` double serves the
/// live harvest). The `base_url()` is `http://127.0.0.1:<ephemeral-port>`.
///
/// SCAFFOLD: true (slice-06) — DELIVER spawns `openlore ui --port 0`, reads the
/// bound port back from the `viewer.serve.listening` event (mirrors
/// `indexer.serve.listening`), polls a TCP connect until the listener accepts,
/// and exposes `get` / `post_form`.
pub struct ViewerServer {
    /// `http://127.0.0.1:<ephemeral-port>` — the base URL the scenario issues
    /// HTTP GET/POST against. Read back from the spawned `ui` process's
    /// `viewer.serve.listening` event (the bound `:0` address).
    base_url: String,
    /// The live `openlore ui` child process. Killed on drop so the bound port is
    /// released (RAII per-scenario isolation; the ephemeral `:0` port keeps
    /// parallel scenarios disjoint).
    child: std::process::Child,
    /// The `GithubServer` double kept alive for the viewer process's lifetime so
    /// the `/scrape` route's `OPENLORE_GITHUB_API_BASE` seam stays reachable.
    /// `None` for store-only scenarios that never exercise `/scrape`. Held so the
    /// double's port stays bound; dropped (releasing it) with the handle.
    _github: Option<GithubServer>,
    /// The `IndexerHandle` for the slice-08 `/search` route's network index
    /// (slice-05 reuse). Kept alive for the viewer process's lifetime so the
    /// REAL `openlore-indexer serve` the viewer queries (via
    /// `OPENLORE_INDEXER_URL`) stays bound. `None` for store-only / `/scrape`
    /// scenarios that never exercise `/search`, AND for the slice-08
    /// `Unavailable` scenarios that point the viewer at a CLOSED port (the env
    /// var carries a freed `ClosedIndexerPort` URL, but no live serve is held).
    /// Held so the indexer's port stays bound; dropped (releasing it) with the
    /// handle.
    _indexer: Option<IndexerHandle>,
}

impl ViewerServer {
    /// Start a `openlore ui --port 0` viewer over the env's REAL store, with NO
    /// `/scrape` GitHub seam wired (store-only scenarios: `/claims`,
    /// `/claims/{cid}`, `/peer-claims`). Waits until the listener accepts a
    /// connection, then exposes `base_url()`.
    ///
    /// SCAFFOLD: true (slice-06).
    pub fn start(env: &TestEnv) -> Self {
        Self::start_inner(env, None, None, None, false, false, false, false)
    }

    /// Start a `openlore ui --port 0` viewer over the env's REAL store AND wire the
    /// `/scrape` route at the supplied `FakeGithub` double (via the
    /// `OPENLORE_GITHUB_API_BASE` seam, exactly as `run_openlore_scrape` does).
    /// Used by the live-scrape scenarios (US-VIEW-005). The double is kept alive
    /// for the viewer's lifetime.
    ///
    /// SCAFFOLD: true (slice-06).
    pub fn start_with_github(env: &TestEnv, github: GithubServer) -> Self {
        Self::start_inner(env, Some(github), None, None, false, false, false, false)
    }

    /// Start a `openlore ui --port 0` viewer over the env's REAL store AND wire the
    /// NEW slice-08 `/search` route at the supplied REAL network index — by
    /// threading the slice-05 `OPENLORE_INDEXER_URL` seam (the SAME env-var the
    /// `openlore search` CLI verb reads, `run_openlore_search`) at the
    /// `IndexerHandle`'s ephemeral serve URL. The handle owns a REAL
    /// `openlore-indexer serve` over a seeded `index.duckdb` (`seed_network_index`);
    /// it is kept alive for the viewer's lifetime so the index the viewer queries
    /// stays bound. Used by the REACHABLE-indexer `/search` scenarios (US-NS-001/
    /// 002/003 + the trust framing of US-NS-004). Mirrors `start_with_github` — the
    /// ONLY delta is the env-var seam wired.
    ///
    /// The indexer is the ONLY mocked-boundary surface (a REAL slice-05 binary over
    /// a fixture corpus — NOT a hand-rolled HTTP double; the verified/attributed
    /// rows come from the production ingest+serve path). The local DuckDB store
    /// stays REAL and UNTOUCHED by `/search` (read-only — proven by the gold
    /// guardrail). NO signing key enters the viewer process (I-NS-1).
    ///
    /// SCAFFOLD: true (slice-08) — `start_inner` is `todo!()`; DELIVER materializes
    /// the spawn + readiness exactly as it does for `start_with_github`, adding only
    /// the `OPENLORE_INDEXER_URL` env-var thread.
    pub fn start_with_indexer(env: &TestEnv, indexer: IndexerHandle) -> Self {
        let url = indexer.indexer_url();
        Self::start_inner(env, None, Some(url), Some(indexer), false, false, false, false)
    }

    /// Start a `openlore ui --port 0` viewer whose `/search` route is wired to an
    /// UNREACHABLE indexer: `OPENLORE_INDEXER_URL` is set to the supplied
    /// `ClosedIndexerPort`'s FREED localhost port (connect-refused by construction —
    /// no live serve, no hang). This is the slice-08 graceful-degradation seam for
    /// the `SearchState::Unavailable` arm when the index is configured-but-down
    /// (US-NS-004 Example 3). The handler must map the soft `Unreachable` outcome to
    /// the fixed plain-language `Unavailable` notice — never a crash/hang/leak. The
    /// closed port is reserved by the CALLER and kept alive for the URL's lifetime.
    ///
    /// SCAFFOLD: true (slice-08).
    pub fn start_with_unreachable_indexer(env: &TestEnv, closed: &ClosedIndexerPort) -> Self {
        Self::start_inner(
            env,
            None,
            Some(closed.indexer_url().to_string()),
            None,
            false,
            false,
            false,
            false,
        )
    }

    /// Shared spawn + readiness core. Spawns the REAL `openlore ui --port 0`
    /// binary (resolved at runtime via `assert_cmd::cargo_bin` — so this COMPILES
    /// before the `ui` verb exists), threads the SAME clean env-seams the other
    /// subprocess helpers use (`OPENLORE_HOME` so the viewer opens the env's REAL
    /// DuckDB; optionally `OPENLORE_GITHUB_API_BASE` for `/scrape`), reads the
    /// bound `:0` port back off stdout, and polls a TCP connect until the
    /// listener accepts.
    ///
    /// SCAFFOLD: true (slice-06) — body is `todo!()`; DELIVER materializes it the
    /// way `spawn_indexer_serve` materializes the indexer's long-running serve.
    fn start_inner(
        env: &TestEnv,
        github: Option<GithubServer>,
        indexer_url: Option<String>,
        indexer: Option<IndexerHandle>,
        fail_active_set_read: bool,
        fail_peer_claims_count: bool,
        fail_countered_count: bool,
        fail_countered_peer_count: bool,
    ) -> Self {
        use std::io::{BufRead, BufReader};

        let bin = assert_cmd::cargo::cargo_bin("openlore");
        let mut cmd = Command::new(&bin);
        cmd.args(["ui", "--port", "0"])
            .env_clear()
            // OPENLORE_HOME so the viewer opens the env's REAL DuckDB — the SAME
            // store the CLI verbs write (BR-VIEW-4, Pillar 3).
            .env("OPENLORE_HOME", &env.home)
            .env("OPENLORE_DID", env.identity.author_did())
            .env("OPENLORE_KEY_SEED_HEX", &env.identity.seed_hex)
            .env("OPENLORE_PDS_ENDPOINT", env.pds.endpoint_url())
            .env("PATH", std::env::var("PATH").unwrap_or_default())
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        // The `/scrape` route reaches GitHub through the OPENLORE_GITHUB_API_BASE
        // seam (US-VIEW-005). Store-only scenarios pass `None` and never set it.
        if let Some(github) = &github {
            cmd.env("OPENLORE_GITHUB_API_BASE", github.base_url());
        }
        // The NEW slice-08 `/search` route reaches the network index through the
        // OPENLORE_INDEXER_URL seam (the SAME slice-05 env-var seam the `openlore
        // search` CLI verb reads — OD-NS-6). When `indexer_url` is `Some`, the
        // viewer's index-query port resolves to that URL: a REAL serve (reachable)
        // or a freed/closed port (unreachable → SearchState::Unavailable). When
        // `None`, the env var is UNSET — the UNCONFIGURED degradation case
        // (US-NS-001 Ex 2 / US-NS-004): the handler must yield Unavailable WITHOUT
        // attempting a network call (I-NS-2).
        if let Some(url) = &indexer_url {
            cmd.env("OPENLORE_INDEXER_URL", url);
        }
        // slice-16 (US-SF-001 / Theme E / C-7 / ADR-053 §Earned-Trust): the TEST-ONLY
        // mid-request active-set-read fault-injection seam. When set, the viewer's
        // effect-shell `active_set_read_with_fault_seam` (a `#[cfg(debug_assertions)]`
        // gate, release-forbidden + xtask-guarded) substitutes a genuine read `Err`,
        // forcing the PRODUCTION degrade path (`Err → empty set → all-NetworkUnfollowed`,
        // the slice-08 status quo) — never a crash/blank/5xx/leak.
        if fail_active_set_read {
            cmd.env("OPENLORE_VIEWER_FAIL_ACTIVE_SET_READ", "1");
        }
        // slice-17 (US-LD-000/001 / Theme 4 / C-2 CARDINAL / WD-LD-2 / WD-LD-8 / ADR-054 D2):
        // the TEST-ONLY GET / peer-claims-count fault-injection seam. When set, the viewer's
        // landing effect shell substitutes a genuine `Err(StoreReadError)` for the REAL
        // `count_peer_claims()` read, forcing the PRODUCTION per-count degrade (`Err → .ok()
        // → None → MISSING_COUNT_MARKER "—"`, ADR-054 D2) — the own-claims + active-peer
        // counts STILL resolve, the nav hub renders in full, the page stays 200 (never a
        // 5xx / blank / raw stack trace). Mirrors the slice-16
        // `OPENLORE_VIEWER_FAIL_ACTIVE_SET_READ` seam discipline (a
        // `#[cfg(debug_assertions)]`-gated, release-forbidden, xtask-guarded effect-shell
        // seam materialized by DELIVER); chosen per the slice-16 SF-8 precedent because the
        // viewer holds ONE long-lived DuckDB connection taken at startup, so there is no
        // readily-available mid-request per-count read-failure seam in the slice-06/15
        // harness. DISTILL scaffolds the OBSERVABLE missing-number contract; DELIVER
        // materializes the per-count fault seam with the SAME observable target.
        if fail_peer_claims_count {
            cmd.env("OPENLORE_VIEWER_FAIL_PEER_CLAIMS_COUNT", "1");
        }
        // slice-18 (US-CC-000/001/002 / Theme 4 / C-2 / C-5 CARDINAL / WD-CC-2/6 / ADR-055
        // D4): the TEST-ONLY GET / + GET /claims COUNTERED-count fault-injection seam. When
        // set, the viewer's landing + claims effect shells substitute a genuine
        // `Err(StoreReadError)` for the REAL `count_countered_own_claims()` read, forcing the
        // PRODUCTION per-count degrade (`Err → .ok() → None → render_countered(None) →
        // "(— countered)"`, ADR-055 D4) — the own-claims "12" + the other landing counts + the
        // nav hub + the `/claims` rows STILL resolve, the page stays 200 (never a 5xx / blank /
        // raw stack trace), and a fabricated "(0 countered)" is unrepresentable (the shell maps
        // a failed read to `None`, never `Some(0)`). Mirrors the slice-17
        // `OPENLORE_VIEWER_FAIL_PEER_CLAIMS_COUNT` seam discipline VERBATIM (a
        // `#[cfg(debug_assertions)]`-gated, release-forbidden, xtask-guarded effect-shell seam
        // materialized by DELIVER) — chosen per the slice-16 SF-8 / slice-17 LD-DEGRADE
        // precedent because the viewer holds ONE long-lived DuckDB connection taken at startup,
        // so there is no readily-available mid-request per-count read-failure seam in the
        // slice-06/15 harness. DISTILL scaffolds the OBSERVABLE missing-marker contract; DELIVER
        // materializes the per-count fault seam with the SAME observable target.
        if fail_countered_count {
            cmd.env("OPENLORE_VIEWER_FAIL_COUNTERED_COUNT", "1");
        }
        // slice-19 (US-PC-000/001/002 / Theme 4 / C-2 / C-5 CARDINAL / WD-PC-2/6 / ADR-056
        // D4): the TEST-ONLY GET / + GET /peer-claims COUNTERED-PEER-count fault-injection
        // seam. When set, the viewer's landing + peer-claims effect shells substitute a
        // genuine `Err(StoreReadError)` for the REAL `count_countered_peer_claims()` read,
        // forcing the PRODUCTION per-count degrade (`Err → .ok() → None → render_countered(None)
        // → "(— countered)"`, ADR-056 D4) — the peer-claims "4" + the slice-18 own line
        // "12 own claims (3 countered)" + the other landing counts + the nav hub + the
        // `/peer-claims` rows + slice-13 per-row flags STILL resolve, the page stays 200 (never
        // a 5xx / blank / raw stack trace), and a fabricated "(0 countered)" is unrepresentable
        // (the shell maps a failed read to `None`, never `Some(0)`). A 4th DISTINCT token (NOT
        // a reuse of the slice-18 `OPENLORE_VIEWER_FAIL_COUNTERED_COUNT`) so the PEER count can
        // fail INDEPENDENTLY of the own count — the missing≠zero AT asserts the slice-18 own
        // line stays untouched while only the peer count degrades (WD-PC-7, ADR-056 D4).
        // Mirrors the slice-18 `OPENLORE_VIEWER_FAIL_COUNTERED_COUNT` seam discipline VERBATIM
        // (a `#[cfg(debug_assertions)]`-gated, release-forbidden, xtask-guarded effect-shell
        // seam materialized by DELIVER + appended to the xtask `VIEWER_FAIL_SEAM_TOKENS` guard)
        // — chosen per the slice-16 SF-8 / slice-17/18 degrade precedent because the viewer
        // holds ONE long-lived DuckDB connection taken at startup, so there is no
        // readily-available mid-request per-count read-failure seam in the slice-06/15 harness.
        // DISTILL scaffolds the OBSERVABLE missing-marker contract; DELIVER materializes the
        // per-count fault seam with the SAME observable target.
        if fail_countered_peer_count {
            cmd.env("OPENLORE_VIEWER_FAIL_COUNTERED_PEER_COUNT", "1");
        }

        let mut child = cmd
            .spawn()
            .unwrap_or_else(|e| panic!("spawn `openlore ui --port 0` at {bin:?}: {e}"));

        // Read the bound-address event off stdout. The serve process prints
        // `{"event":"viewer.serve.listening","addr":"127.0.0.1:<port>"}` once
        // bound (mirrors `indexer.serve.listening`).
        let stdout = child.stdout.take().expect("openlore ui: stdout pipe");
        let mut reader = BufReader::new(stdout);
        let mut addr: Option<String> = None;
        for _ in 0..50 {
            let mut line = String::new();
            match reader.read_line(&mut line) {
                Ok(0) => break, // EOF — the viewer exited before binding
                Ok(_) => {
                    if let Ok(event) = serde_json::from_str::<serde_json::Value>(line.trim()) {
                        if event["event"] == "viewer.serve.listening" {
                            if let Some(a) = event["addr"].as_str() {
                                addr = Some(a.to_string());
                                break;
                            }
                        }
                    }
                }
                Err(_) => break,
            }
        }
        let addr = addr.unwrap_or_else(|| {
            let _ = child.kill();
            let mut err = String::new();
            if let Some(mut stderr) = child.stderr.take() {
                use std::io::Read;
                let _ = stderr.read_to_string(&mut err);
            }
            panic!("`openlore ui` did not report a bound address on stdout; stderr: {err}");
        });

        // Poll a TCP connect until the listener accepts (the readiness signal
        // means bound; this confirms the accept loop is live before the first
        // GET). ~5s budget.
        let socket: std::net::SocketAddr = addr
            .parse()
            .unwrap_or_else(|e| panic!("viewer reported an unparseable addr {addr:?}: {e}"));
        for _ in 0..50 {
            if std::net::TcpStream::connect_timeout(&socket, std::time::Duration::from_millis(100))
                .is_ok()
            {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }

        Self {
            base_url: format!("http://{addr}"),
            child,
            _github: github,
            _indexer: indexer,
        }
    }

    /// The `http://127.0.0.1:<ephemeral-port>` base URL the scenario issues HTTP
    /// requests against.
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// Issue an HTTP `GET <base_url><path>` and return the status + rendered HTML
    /// body. `path` includes the leading slash and any query string (e.g.
    /// `/claims?page=2`). Uses the workspace `reqwest` blocking client (the same
    /// HTTP client the indexer ATs use to reach the localhost serve).
    ///
    /// SCAFFOLD: true (slice-06).
    pub fn get(&self, path: &str) -> ViewerResponse {
        let url = format!("{}{}", self.base_url, path);
        let response = reqwest::blocking::Client::new()
            .get(&url)
            .timeout(std::time::Duration::from_secs(10))
            .send()
            .unwrap_or_else(|e| panic!("GET {url}: {e}"));
        let status = response.status().as_u16();
        let content_type = content_type_of(&response);
        let body = response
            .text()
            .unwrap_or_else(|e| panic!("read body of GET {url}: {e}"));
        ViewerResponse {
            status,
            body,
            content_type,
        }
    }

    /// Issue an HTTP `POST <base_url><path>` with `fields` as an
    /// `application/x-www-form-urlencoded` body (the `/scrape` target form), and
    /// return the status + rendered HTML. The form is how the operator submits a
    /// scrape target on the Live Scrape view (DESIGN §5 — POST /scrape).
    ///
    /// SCAFFOLD: true (slice-06).
    pub fn post_form(&self, path: &str, fields: &[(&str, &str)]) -> ViewerResponse {
        let url = format!("{}{}", self.base_url, path);
        let response = reqwest::blocking::Client::new()
            .post(&url)
            .timeout(std::time::Duration::from_secs(10))
            .form(fields)
            .send()
            .unwrap_or_else(|e| panic!("POST {url}: {e}"));
        let status = response.status().as_u16();
        let content_type = content_type_of(&response);
        let body = response
            .text()
            .unwrap_or_else(|e| panic!("read body of POST {url}: {e}"));
        ViewerResponse {
            status,
            body,
            content_type,
        }
    }

    /// Issue an HTTP `GET <base_url><path>` WITH the `HX-Request: true` header —
    /// the slice-07 htmx (FRAGMENT) shape driver (ADR-035, OD-HX-6). A mirror of
    /// [`get`](Self::get); the ONLY delta is the added header. The header is what
    /// the htmx library sets on swap-driven requests; ADR-033's `Shape::from_request`
    /// keys on its PRESENCE (the value `"true"` matches real htmx for fidelity, but
    /// is not load-bearing). Under this header the viewer returns ONLY the swap-
    /// target region (no chrome, no `<!DOCTYPE html>`), per I-HX-1.
    ///
    /// The companion no-header [`get`](Self::get) stays the no-JS / full-page driver
    /// (curl / bookmark / direct URL / JS-off), so the slice-06 corpus that uses it
    /// is byte-unaffected (I-HX-4).
    pub fn get_htmx(&self, path: &str) -> ViewerResponse {
        let url = format!("{}{}", self.base_url, path);
        let response = reqwest::blocking::Client::new()
            .get(&url)
            .header("HX-Request", "true")
            .timeout(std::time::Duration::from_secs(10))
            .send()
            .unwrap_or_else(|e| panic!("GET (htmx) {url}: {e}"));
        let status = response.status().as_u16();
        let content_type = content_type_of(&response);
        let body = response
            .text()
            .unwrap_or_else(|e| panic!("read body of GET (htmx) {url}: {e}"));
        ViewerResponse {
            status,
            body,
            content_type,
        }
    }

    /// Issue an HTTP `POST <base_url><path>` form-urlencoded WITH the
    /// `HX-Request: true` header — the slice-07 htmx scrape-results (FRAGMENT) shape
    /// driver (ADR-035, US-HX-003). A mirror of [`post_form`](Self::post_form); the
    /// ONLY delta is the added header. Under this header `POST /scrape` returns ONLY
    /// the `#scrape-results` region swapped below the form (candidates / zero-
    /// candidate / network-down guidance), with NO surrounding chrome and NO sign
    /// control (BR-HX-4 / I-SCR-1). The companion no-header [`post_form`](Self::post_form)
    /// stays the no-JS full-page driver.
    pub fn post_form_htmx(&self, path: &str, fields: &[(&str, &str)]) -> ViewerResponse {
        let url = format!("{}{}", self.base_url, path);
        let response = reqwest::blocking::Client::new()
            .post(&url)
            .header("HX-Request", "true")
            .timeout(std::time::Duration::from_secs(10))
            .form(fields)
            .send()
            .unwrap_or_else(|e| panic!("POST (htmx) {url}: {e}"));
        let status = response.status().as_u16();
        let content_type = content_type_of(&response);
        let body = response
            .text()
            .unwrap_or_else(|e| panic!("read body of POST (htmx) {url}: {e}"));
        ViewerResponse {
            status,
            body,
            content_type,
        }
    }
}

/// Read the `Content-Type` header off a blocking reqwest response, lowercased
/// (empty when absent). Called BEFORE `.text()` consumes the response. The
/// asset route (H-5a) is the only place this is load-bearing today; pulled out
/// so all four drivers populate `ViewerResponse::content_type` identically.
fn content_type_of(response: &reqwest::blocking::Response) -> String {
    response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_lowercase()
}

impl Drop for ViewerServer {
    fn drop(&mut self) {
        // Kill the `openlore ui` process so the bound ephemeral port is released —
        // RAII per-scenario isolation (mirrors IndexerHandle / FakePds / PeerPds).
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Seed `count` of the operator's OWN signed claims into the env's REAL slice-01
/// `claims` table through the PRODUCTION write path — the `openlore claim add`
/// verb (Pillar 3: the production composition root, the SAME store the viewer
/// reads, BR-VIEW-4). The walking skeleton (V-1) seeds via this real verb so the
/// rows the viewer renders are produced by production code, not hand-inserted.
///
/// SCAFFOLD: true (slice-06) — DELIVER drives `run_openlore` with `claim add`
/// (+ the chained sign/publish prompts as the existing claim scenarios do) so
/// `count` real signed rows land in the REAL DuckDB the viewer then opens.
pub fn seed_own_claims_via_cli(env: &TestEnv, count: usize) {
    // Drive the PRODUCTION `openlore claim add` write path `count` times so
    // `count` real signed rows land in the env's REAL slice-01 `claims` table —
    // the SAME store `openlore ui` opens (BR-VIEW-4, Pillar 3). Each claim gets a
    // DISTINCT subject (`owner/repo-{i}`) so the rows are distinct records (no CID
    // aliasing) and their CIDs differ — making the deterministic page boundaries
    // (`ORDER BY composed_at DESC, cid ASC`) observable across requests
    // (AC-004.3). A fixed predicate/object/confidence keeps every other field
    // stable so only the subject/CID vary. Reuses `seed_own_claim_with_evidence`'s
    // mechanism (the production `claim add` subprocess) in a tight loop — slow at
    // 312, but the only path that honors BR-VIEW-4.
    for i in 0..count {
        let subject = format!("owner/repo-{i:04}");
        seed_own_claim_with_evidence(
            env,
            &subject,
            "is-maintained-by",
            "The Maintainers",
            0.90,
            &[],
        );
    }
}

/// Seed ONE specific own claim (subject/predicate/object/confidence + optional
/// evidence URLs) into the REAL `claims` table via the production `claim add`
/// path, returning its CID (for the `/claims/{cid}` detail scenarios). The CID is
/// read back from the `claim add` stdout (the `published_cid_from_stdout` shape).
///
/// SCAFFOLD: true (slice-06).
pub fn seed_own_claim_with_evidence(
    env: &TestEnv,
    subject: &str,
    predicate: &str,
    object: &str,
    confidence: f64,
    evidence_urls: &[&str],
) -> String {
    // Drive the PRODUCTION `openlore claim add` write path (Pillar 3 — the SAME
    // store `openlore ui` reads, BR-VIEW-4). Build the arg vector with one
    // `--evidence <url>` per URL (clap `Vec<String>`). A single `\n` on stdin
    // confirms the SIGN prompt (Enter); EOF after that DECLINES the publish
    // prompt — we want the claim signed + persisted locally, NOT published to
    // the PDS (the viewer reads the local store; publication is irrelevant to
    // V-1 and avoids depending on the fake-PDS round-trip).
    let confidence_str = format!("{confidence}");
    let mut args: Vec<&str> = vec![
        "claim",
        "add",
        "--subject",
        subject,
        "--predicate",
        predicate,
        "--object",
        object,
        "--confidence",
        &confidence_str,
    ];
    for url in evidence_urls {
        args.push("--evidence");
        args.push(url);
    }

    let outcome = run_openlore_with_stdin(env, &args, "\n");
    if outcome.status != 0 {
        panic!(
            "seed_own_claim_with_evidence: `openlore claim add` failed (exit {}). \
             stdout: {} stderr: {}",
            outcome.status, outcome.stdout, outcome.stderr
        );
    }
    // `claim add` prints `Computing claim CID <cid>` once the user confirms the
    // sign prompt. Recover the CID from that line so the `/claims/{cid}` detail
    // scenarios can address the exact record without hard-coding a CID.
    signed_cid_from_stdout(&outcome.stdout)
}

/// Recover the signed claim's CID from the `claim add` stdout. The verb prints
/// `Computing claim CID <cid>` after the sign prompt is confirmed (before the
/// publish prompt), so the CID is recoverable even when the claim is NOT
/// published (the V-1 seed path declines publish). Distinct from
/// [`published_cid_from_stdout`], which parses the post-PUBLISH success block.
pub fn signed_cid_from_stdout(stdout: &str) -> String {
    // The sign prompt is written WITHOUT a trailing newline, so the
    // `Computing claim CID <cid>` text shares a line with the prompt:
    // `Press Enter to sign locally (...): Computing claim CID bafy...`.
    // Find the marker anywhere in the line and take the first whitespace-
    // delimited token after it as the CID.
    const MARKER: &str = "Computing claim CID ";
    for line in stdout.lines() {
        if let Some(pos) = line.find(MARKER) {
            let rest = &line[pos + MARKER.len()..];
            if let Some(cid) = rest.split_whitespace().next() {
                return cid.to_string();
            }
        }
    }
    panic!(
        "could not find a 'Computing claim CID <cid>' line in stdout to recover \
         the signed CID; \n--- stdout ---\n{stdout}"
    );
}

/// Seed `count` peer claims (federated from `peer_did`) into the env's REAL
/// slice-03 `peer_claims` table through the PRODUCTION federation path — the
/// `openlore peer pull` verb against a `PeerPds` double (the SAME store the viewer
/// reads). Used by the peer-claims view + pagination scenarios (US-VIEW-003/004).
///
/// SCAFFOLD: true (slice-06) — DELIVER reuses the slice-03 `run_openlore_pull` +
/// `PeerPds` seam (the established federation write path) so `count` real
/// `peer_claims` rows (carrying `author_did` + `fetched_from_pds`) land in the
/// REAL DuckDB the viewer opens.
pub fn seed_peer_claims_via_pull(env: &TestEnv, peer_did: &str, count: usize) {
    // Drive the PRODUCTION slice-03 federation write path: build `count`
    // verifiable wire records for `peer_did`, `peer add` (subscribe), then
    // `peer pull`. The pull pipeline verifies each record, recomputes its CID,
    // and persists a `peer_claims` row (author_did + fetched_from_pds) into the
    // env's REAL DuckDB — the SAME store `openlore ui` opens (BR-VIEW-4). The
    // pull completes synchronously before this returns, so the returned
    // SeededGraph (which owns the live PeerPds doubles) can be dropped here: the
    // rows are already persisted; the viewer reads them from the store, not the
    // PDS.
    //
    // Each peer claim gets a distinct subject so the `count` rows are distinct
    // records (no CID aliasing) and a fixed confidence the renderer shows
    // verbatim. The peer's Ed25519 seed is derived from `peer_did` so repeated
    // calls with different DIDs cross-verify against distinct keys.
    let triples: Vec<(String, String, f64)> = (0..count)
        .map(|i| {
            (
                format!("github:peer/{peer_did}-{i}"),
                format!("org.openlore.philosophy.peer-{i}"),
                0.70,
            )
        })
        .collect();
    let triple_refs: Vec<(&str, &str, f64)> = triples
        .iter()
        .map(|(s, o, c)| (s.as_str(), o.as_str(), *c))
        .collect();

    let mut seed = [0u8; 32];
    for (slot, byte) in seed.iter_mut().zip(peer_did.bytes()) {
        *slot = byte;
    }
    // Avoid an all-zero seed colliding with the local identity's key material.
    seed[31] = seed[31].wrapping_add(1);

    let peers = [SeedPeer {
        peer_did,
        seed,
        triples: &triple_refs,
    }];
    // The SeededGraph is dropped at end of scope — the pull already persisted the
    // rows, so the PeerPds doubles are no longer needed for the viewer read.
    let _graph = seed_peer_authored_graph(env, &peers);
}

/// Seed ONE peer_claims row whose origin (`author_did`) is BLANK/absent — a
/// DEFENSIVE fixture that bypasses the slice-03 schema CHECK (which makes
/// `author_did` non-empty) to exercise the viewer's "unknown" render path
/// (US-VIEW-003 boundary / AC-003.3). The row must still RENDER (labeled
/// "unknown"), never be dropped. Test-support is the only place raw SQL +
/// CHECK-bypass is acceptable; production federation goes through `peer pull`.
///
/// SCAFFOLD: true (slice-06) — DELIVER inserts a `peer_claims` row with a blank
/// origin via raw SQL (bypassing the production write path so the defensive
/// render branch is reachable), into the env's REAL DuckDB the viewer opens.
pub fn seed_peer_claim_with_blank_origin(env: &TestEnv) {
    let db_path = env.duckdb_path();
    let conn = duckdb::Connection::open(&db_path).unwrap_or_else(|err| {
        panic!(
            "open DuckDB at {} to seed a blank-origin peer_claims row: {err}",
            db_path.display()
        )
    });

    // The slice-03 schema makes `author_did` non-empty (`NOT NULL` +
    // `CHECK (author_did <> '')`), so a blank origin cannot be inserted while
    // that CHECK stands. To exercise the viewer's DEFENSIVE "unknown" render
    // path we must inject data that BYPASSES the CHECK — exactly the data that
    // predates/bypasses the constraint. DuckDB's CHECK constraints here are
    // unnamed, so they cannot be dropped by name; instead we rename the table
    // aside, recreate it WITHOUT the `author_did` CHECK (keeping every other
    // column + constraint the viewer reads), copy any existing rows back, and
    // drop the staging table. Everything that DEPENDS on `peer_claims` blocks
    // the rename and must be dropped first, then recreated: the three indexes
    // (`idx_peer_claims_*`) and the two FK tables (`peer_claim_references`,
    // `peer_claim_evidence`) that reference `peer_claims (cid)`. A
    // freshly-initialized env has none of these rows yet, so recreating the
    // dependent tables empty preserves the schema the viewer reads. Test-support
    // is the only place raw SQL + CHECK-bypass is acceptable; production
    // federation goes through `peer pull`.
    conn.execute_batch(
        "DROP INDEX IF EXISTS idx_peer_claims_author;
         DROP INDEX IF EXISTS idx_peer_claims_subject;
         DROP INDEX IF EXISTS idx_peer_claims_composed_at;
         DROP TABLE IF EXISTS peer_claim_references;
         DROP TABLE IF EXISTS peer_claim_evidence;
         ALTER TABLE peer_claims RENAME TO peer_claims_check_bypass;
         CREATE TABLE peer_claims (
             cid                 VARCHAR PRIMARY KEY,
             author_did          VARCHAR NOT NULL,
             subject             VARCHAR NOT NULL,
             predicate           VARCHAR NOT NULL,
             object              VARCHAR NOT NULL,
             confidence          DOUBLE  NOT NULL CHECK (confidence >= 0.0 AND confidence <= 1.0),
             composed_at         TIMESTAMP NOT NULL,
             fetched_at          TIMESTAMP NOT NULL,
             fetched_from_pds    VARCHAR NOT NULL,
             signed_record_path  VARCHAR NOT NULL,
             CHECK (cid <> '')
         );
         INSERT INTO peer_claims SELECT * FROM peer_claims_check_bypass;
         DROP TABLE peer_claims_check_bypass;
         CREATE INDEX IF NOT EXISTS idx_peer_claims_author       ON peer_claims (author_did);
         CREATE INDEX IF NOT EXISTS idx_peer_claims_subject      ON peer_claims (subject);
         CREATE INDEX IF NOT EXISTS idx_peer_claims_composed_at  ON peer_claims (composed_at);
         CREATE TABLE peer_claim_references (
             referencing_cid     VARCHAR NOT NULL,
             referenced_cid      VARCHAR NOT NULL,
             ref_type            VARCHAR NOT NULL CHECK (ref_type IN ('retracts','corrects','counters','supersedes')),
             PRIMARY KEY (referencing_cid, referenced_cid, ref_type),
             FOREIGN KEY (referencing_cid) REFERENCES peer_claims (cid)
         );
         CREATE INDEX IF NOT EXISTS idx_peer_claim_refs_referenced ON peer_claim_references (referenced_cid);
         CREATE TABLE peer_claim_evidence (
             cid         VARCHAR NOT NULL,
             evidence    VARCHAR NOT NULL,
             ordinal     INTEGER NOT NULL,
             PRIMARY KEY (cid, ordinal),
             FOREIGN KEY (cid) REFERENCES peer_claims (cid)
         );",
    )
    .unwrap_or_else(|err| {
        panic!("drop the peer_claims author_did CHECK to seed a blank-origin row: {err}")
    });

    // Now insert ONE row whose origin (`author_did`) is the empty string — the
    // adapter maps `author_did == ""` to `PeerOrigin::Unknown` (defensive),
    // never dropping the row. Every OTHER field is populated normally so the
    // row renders in full (AC-003.3 #3).
    conn.execute(
        "INSERT INTO peer_claims \
            (cid, author_did, subject, predicate, object, confidence, \
             composed_at, fetched_at, fetched_from_pds, signed_record_path) \
         VALUES (?, '', ?, ?, ?, ?, now(), now(), ?, ?)",
        duckdb::params![
            "bafyblankorigin0",
            "github:peer/orphan-repo",
            "endorses",
            "an-unattributed-object",
            0.7_f64,
            "https://peer.example/pds",
            "peer_claims/blank-origin/bafyblankorigin0.json",
        ],
    )
    .unwrap_or_else(|err| panic!("seed blank-origin peer_claims row: {err}"));
}

/// The two port-exposed slot NAMES that make up the I-VIEW-1 read-only
/// universe (`viewer_is_read_only`). Kept as one source of truth so `capture`
/// and the `universe` set never drift (Mandate 8 — observable count names,
/// never an internal adapter field).
pub const STORE_SLOT_CLAIMS: &str = "claims.row_count";
pub const STORE_SLOT_PEER_CLAIMS: &str = "peer_claims.row_count";

/// Capture the read-only universe: the row counts of BOTH persisted tables
/// (`claims` + `peer_claims`) in the env's REAL DuckDB. Port-exposed observable
/// names (`claims.row_count`, `peer_claims.row_count`) — NEVER an internal
/// adapter struct field (Mandate 8). The `viewer_is_read_only` gold test snapshots
/// this BEFORE and AFTER exercising every route (incl. POST /scrape) and asserts
/// the delta is all-`unchanged` via `assert_state_delta`.
///
/// SCAFFOLD: true (slice-06) — DELIVER reads `SELECT COUNT(*)` from `claims` and
/// `peer_claims` (test-support is the only place raw SQL is acceptable; the
/// VIEWER goes through the read-only `StoreReadPort`) into the universe HashMap.
pub fn capture_store_row_count_universe(env: &TestEnv) -> std::collections::HashMap<String, u64> {
    let db_path = env.duckdb_path();
    let conn = duckdb::Connection::open(&db_path).unwrap_or_else(|err| {
        panic!(
            "open DuckDB at {} for read-only universe capture: {err}",
            db_path.display()
        )
    });

    let count_of = |table: &str| -> u64 {
        let total: i64 = conn
            .query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r.get(0))
            .unwrap_or_else(|err| panic!("query {table} row_count for read-only universe: {err}"));
        total.max(0) as u64
    };

    let mut universe = std::collections::HashMap::new();
    universe.insert(STORE_SLOT_CLAIMS.to_string(), count_of("claims"));
    universe.insert(STORE_SLOT_PEER_CLAIMS.to_string(), count_of("peer_claims"));
    universe
}

/// Universe-bound read-only assertion (Mandate 8): the persisted-store row counts
/// (`claims.row_count` + `peer_claims.row_count`) are UNCHANGED after exercising
/// every viewer route incl. POST /scrape (I-VIEW-1 read-only; gold test
/// `viewer_is_read_only`). Built on the project `state_delta` port — the universe
/// is the two port-exposed counts, each expected `unchanged()`.
///
/// SCAFFOLD: true (slice-06).
pub fn assert_store_read_only(
    before: &std::collections::HashMap<String, u64>,
    after: &std::collections::HashMap<String, u64>,
) {
    // The universe is the two port-exposed counts; every slot is implicit-
    // unchanged (an EMPTY `Delta` → `assert_state_delta` pins each to byte-
    // equality). Any row-count change is an UNSHIPPABLE read-only breach
    // (I-VIEW-1).
    let universe: std::collections::HashSet<String> = [STORE_SLOT_CLAIMS, STORE_SLOT_PEER_CLAIMS]
        .into_iter()
        .map(String::from)
        .collect();

    let expected = state_delta::Delta::new();

    state_delta::assert_state_delta(before, after, &universe, &expected);
}

// =============================================================================
// slice-08 (viewer-network-search) `/search` render assertions — the HTML
// counterparts to the slice-05 stdout assertions (`assert_verified_marker_is_
// universal` / `assert_network_result_preserves_attribution`). The slice-05
// helpers parse `author_did:`-prefixed stdout LINES; the viewer renders HTML, so
// these scan the rendered BODY for the same OBSERVABLE facts (Mandate 8 universe
// = port-exposed rendered surface, never an internal struct field). Reused across
// the htmx-fragment + no-JS-full-page shapes (parity by construction).
// =============================================================================

/// Assert the slice-08 `/search` rendered HTML carries a `[verified]` marker for
/// every attributed author row and NEVER an `[unverified]` / "unknown signature"
/// state (I-NS-4 — verified-by-construction; the indexer is the verify gate, the
/// viewer has no second verification path). The HTML counterpart of
/// [`assert_verified_marker_is_universal`]. Universe (port-exposed rendered
/// surface): the rendered body contains `[verified]`; it never contains
/// `[unverified]` / "unknown signature". Each `expected_author` DID appears in the
/// body AND the `[verified]` marker count is at least the number of expected
/// author rows (every row carries it).
///
/// SCAFFOLD: true (slice-08).
pub fn assert_search_html_every_row_verified_and_attributed(body: &str, expected_authors: &[&str]) {
    // Every expected author DID is rendered (attribution — non-Option author_did,
    // I-NS-3; the viewer renders the SAME `did:plc:…#org.openlore.application`
    // shape the CLI search renders).
    for did in expected_authors {
        assert!(
            body.contains(did),
            "I-NS-3: the `/search` render must attribute a row to {did:?} (every \
             row carries its author_did); body was:\n{body}"
        );
    }
    // The `[verified]` marker is present at least once per expected author row
    // (verified-by-construction; I-NS-4) — and there are at least as many markers
    // as author rows so no rendered row is missing it.
    let verified_count = body.matches("[verified]").count();
    assert!(
        verified_count >= expected_authors.len() && verified_count > 0,
        "I-NS-4: every rendered result row must carry a [verified] marker \
         (expected at least {} markers, one per author row); found {verified_count}:\n{body}",
        expected_authors.len()
    );
    // There is NO unverified state on the surface (I-NS-4 — the viewer cannot
    // render an unverified result).
    for banned in ["[unverified]", "unknown signature"] {
        assert!(
            !body.contains(banned),
            "I-NS-4: the `/search` render must never show {banned:?} (verification \
             is an ingest precondition — there is no unverified state in the \
             viewer); body was:\n{body}"
        );
    }
}

/// Assert the slice-08 `/search` rendered HTML contains NO merged / faceless
/// "network consensus" row (I-NS-3 — anti-merging at network scale; the viewer
/// REUSES the slice-05 per-author `compose_results` with no second grouping
/// path). The HTML counterpart of the anti-merging scan inside
/// [`assert_network_result_preserves_attribution`]. Universe (port-exposed
/// rendered surface): the rendered body contains none of the merged-consensus
/// phrasings. The honesty FOOTER ("not a community consensus") is a PROMISE, not a
/// merged row, so it is excluded by scanning only for the merge ASSERTION
/// phrasings.
///
/// SCAFFOLD: true (slice-08).
pub fn assert_search_html_has_no_merged_consensus_row(body: &str) {
    let lowered = body.to_ascii_lowercase();
    for banned in [
        "authors agree",
        "the network says",
        "the network thinks",
        "network consensus",
    ] {
        assert!(
            !lowered.contains(banned),
            "I-NS-3 (anti-merging): the `/search` render must show NO merged / \
             faceless consensus row; found {banned:?} in body:\n{body}"
        );
    }
}

/// Assert a slice-08 `/search` rendered body (fragment OR full page) shows the
/// counter SHOWN-not-applied (OD-AV-7 / I-NS-3) on the browser surface: the
/// countered claim C STILL renders verbatim (its author DID + object + the
/// `[verified]` marker are present — NOT filtered, merged, or over-ridden) AND C's
/// row carries an inline counter-annotation NAMING the countering author DID
/// (`countered by <K.author>`). The HTML counterpart of the slice-05 CLI gate
/// [`assert_counter_annotation_shown_not_applied`]: same shown-not-applied
/// discipline, projected to the viewer's rendered markup. No filter/down-weight
/// language appears.
///
/// SCAFFOLD: true (slice-08).
pub fn assert_search_html_counter_shown_not_applied(
    body: &str,
    countered_author: &str,
    countered_object: &str,
    counter_author: &str,
) {
    // 1. C is STILL shown verbatim — its author attribution, its object, and the
    //    `[verified]` marker are all present (the counter never removes/merges C).
    assert!(
        body.contains(countered_author),
        "I-NS-3: the countered claim's author row must STILL be attributed \
         ({countered_author:?}) — counter SHOWN, never applied; body was:\n{body}"
    );
    assert!(
        body.contains(countered_object),
        "I-NS-3: the countered claim must STILL be shown verbatim (object \
         {countered_object:?} present — NOT filtered/merged/over-ridden); body \
         was:\n{body}"
    );
    assert!(
        body.contains("[verified]"),
        "I-NS-4: the countered row must STILL carry the [verified] marker (it is a \
         verified attributed result; the counter is an annotation, not a filter); \
         body was:\n{body}"
    );

    // 2. C's row carries an INLINE counter-annotation naming the COUNTERING author
    //    (`countered by <K.author>`) — the counter is SHOWN on the countered row.
    assert!(
        body.contains(&format!("countered by {counter_author}")),
        "I-NS-3 / OD-AV-7: the countered row must carry an inline counter-annotation \
         'countered by {counter_author}' (counter SHOWN, never applied); body \
         was:\n{body}"
    );

    // 3. The counter is SHOWN, NEVER applied: no filter/down-weight language.
    let lowered = body.to_ascii_lowercase();
    for banned in ["filtered out", "down-weighted", "suppressed", "hidden by counter"] {
        assert!(
            !lowered.contains(banned),
            "I-NS-3: the counter must be SHOWN, never APPLIED — found {banned:?} in \
             the rendered body (OD-AV-7 shown-not-applied); body was:\n{body}"
        );
    }
}

/// Assert a slice-08 `/search` rendered body (fragment OR full page) leaks NO
/// transport internals — the degradation no-leak gate (I-NS-2). The viewer counter-
/// part of the slice-06 `/scrape` V-S4 negative-needle scan: a down/unconfigured
/// index renders the FIXED plain-language `Unavailable` notice and NEVER an HTTP
/// status, "connection refused" / "timed out" / "DNS" jargon, a raw URL, or a
/// stack trace. Universe (port-exposed rendered surface): the body contains NONE
/// of the leaked-internal needles. Reused for BOTH shapes (the unit-variant render
/// is identical across fragment + full page — I-NS-2 / WD-NS-4).
///
/// SCAFFOLD: true (slice-08).
pub fn assert_search_html_leaks_no_transport_internals(body: &str) {
    let lowered = body.to_ascii_lowercase();
    for leaked_internal in [
        "connection refused",
        "connecterror",
        "timed out",
        "dns",
        "503",
        "502",
        "500",
        "http://127.0.0.1",
        "panicked at",
        "no such host",
        "econnrefused",
    ] {
        assert!(
            !lowered.contains(&leaked_internal.to_lowercase()),
            "I-NS-2: the `/search` Unavailable render must leak NO transport \
             internals ({leaked_internal:?}) — a fixed plain-language notice only \
             (the SearchState::Unavailable unit variant cannot interpolate a \
             transport string); body was:\n{body}"
        );
    }
}

/// Make the env's REAL store file UNREADABLE by the viewer (US-VIEW-001 Ex 3 /
/// AC-001.4 — "is another process using it?"). Returns a guard whose `Drop`
/// restores access (so the tempdir cleanup still works). The startup probe
/// (ADR-030 §Earned-Trust step 1) surfaces this as a `health.startup.refused`
/// at `openlore ui` start — a plain-language refusal, never a per-request stack
/// trace.
///
/// SCAFFOLD: materialized (slice-06, step 01-04). Holds a second DuckDB
/// `Connection` open against the SAME `openlore.duckdb` file. DuckDB takes an
/// exclusive file lock per open handle, so while this guard is alive the viewer
/// process's own `Connection::open` of the same file fails with a lock conflict —
/// exactly the "another process is using it" condition US-VIEW-001 Example 3
/// describes. The lock (and the guard's connection) is released on `Drop`, so the
/// `TestEnv` tempdir cleanup still succeeds afterwards.
pub fn make_store_unreadable(env: &TestEnv) -> StoreLockGuard {
    let db_path = env.duckdb_path();
    // `TestEnv::initialized()` has already run `openlore init`, which created and
    // migrated the file, so this is a re-open of an EXISTING store — the same
    // operation the viewer attempts. Holding it open denies the viewer the lock.
    let lock = duckdb::Connection::open(&db_path).unwrap_or_else(|err| {
        panic!(
            "make_store_unreadable: could not take the holding lock on {}: {err}",
            db_path.display()
        )
    });
    StoreLockGuard { _lock: lock }
}

/// RAII guard returned by [`make_store_unreadable`]; holds the conflicting DuckDB
/// open handle so the viewer cannot acquire the file lock. Dropping it releases
/// the lock, restoring store readability so the `TestEnv` tempdir can clean up.
pub struct StoreLockGuard {
    /// The held DuckDB connection whose exclusive file lock blocks the viewer.
    /// Dropped (releasing the lock) when the guard goes out of scope.
    _lock: duckdb::Connection,
}

/// Try to start `openlore ui` over an UNREADABLE store and capture the startup
/// refusal outcome (exit code + stderr) WITHOUT spawning a long-running server.
/// The viewer refuses to serve (WIRE→PROBE→USE; ADR-009/030) with a plain-language
/// message naming the store path — NOT a raw stack trace (NFR-VIEW-6). Used by the
/// unreadable-store scenario (V-4).
///
/// SCAFFOLD: materialized (slice-06, step 01-04). Spawns the REAL `openlore ui
/// --port 0` binary over the env's (now unreadable) store, threading the SAME
/// clean env-seams `ViewerServer::start_inner` uses so the viewer resolves the
/// env's REAL DuckDB. Because the store is locked, the viewer walks WIRE→PROBE
/// (ADR-009/030), refuses BEFORE binding a serve loop, and exits — so
/// `wait_with_output()` returns promptly (no long-running server to kill). The
/// captured stdout carries the structured `health.startup.refused` event line;
/// stderr carries the plain-language refusal.
pub fn run_openlore_ui_expecting_startup_refusal(env: &TestEnv) -> CliOutcome {
    let bin = assert_cmd::cargo::cargo_bin("openlore");
    let output = Command::new(&bin)
        .args(["ui", "--port", "0"])
        .env_clear()
        .env("OPENLORE_HOME", &env.home)
        .env("OPENLORE_DID", env.identity.author_did())
        .env("OPENLORE_KEY_SEED_HEX", &env.identity.seed_hex)
        .env("OPENLORE_PDS_ENDPOINT", env.pds.endpoint_url())
        .env("PATH", std::env::var("PATH").unwrap_or_default())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .unwrap_or_else(|e| panic!("spawn `openlore ui --port 0` at {bin:?}: {e}"));

    CliOutcome {
        status: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    }
}

// =============================================================================
// slice-09 (viewer-contributor-scoring) — LOCAL contributor-score seeding +
// render assertions. The `/score` route reads the contributor's LOCAL attributed
// feed (claims ∪ local peer_claims) over the read-only DuckDB store via
// `StoreReadPort::query_contributor_scoring_feed(&Did)` (NO network — I-CS-5),
// runs the REUSED slice-04 PURE `scoring::score`, and renders the ranked
// `WeightedView` as HTML. These seeders land the contributor's LOCAL trail
// through the PRODUCTION federation write path (`peer add` + `peer pull` — the
// SAME store the viewer reads, Pillar 3), in three postures mirroring the
// slice-04 priya-rich / bjorn-sparse / nobody-empty fixtures:
//
//   - RICH  → `seed_contributor_rich_trail`  : a contributor across DISTINCT
//             subjects on a shared object (cross-project span ≥ 2 → lifts out of
//             Sparse; a real weight + a multi-row breakdown that decomposes).
//   - SPARSE→ `seed_contributor_sparse_trail`: a single claim by a single author
//             on a single subject (claim_count=1, distinct_author=1, span=1 →
//             `[SPARSE]` regardless of confidence magnitude — the breadth guard).
//   - EMPTY → no seeding: a DID with zero local rows → the guided `NoClaims`
//             state (no crash, no network).
//
// The contributor is identified by `author_did`. The seeders write `peer_claims`
// rows attributed to the contributor DID (a contributor whose claims the operator
// has already pulled locally), which the `claims ∪ peer_claims` feed read returns.
// =============================================================================

/// The canonical slice-09 contributor DIDs — the rich / sparse fixtures (mirrors
/// the slice-04 priya-rich + bjorn-sparse shape; the empty case uses a DID that is
/// never seeded). Kept as one source of truth so the seed call + the rendered-row
/// assertions never drift on the DID string.
pub const CONTRIBUTOR_RICH_DID: &str = "did:plc:priya-test";
pub const CONTRIBUTOR_SPARSE_DID: &str = "did:plc:bjorn-test";
pub const CONTRIBUTOR_EMPTY_DID: &str = "did:plc:nobody-local";

/// The headline object NSID the rich/sparse contributor trails assert on (the
/// slice-04 reproducible-builds philosophy). One source of truth so the score
/// pairing's object + the assertions never drift.
pub const SCORE_OBJECT_REPRODUCIBLE_BUILDS: &str = "org.openlore.philosophy.reproducible-builds";

/// Seed a RICH local trail for `contributor_did` through the PRODUCTION federation
/// write path (`peer add` + `peer pull` against a `PeerPds` double — the SAME
/// store `openlore ui` opens, Pillar 3 / BR-VIEW-4). The contributor asserts the
/// SAME object (reproducible-builds) across SEVERAL DISTINCT subjects, so the pure
/// scorer's breadth guard sees `cross_project_span >= 2` (lifting the pairing OUT
/// of `[SPARSE]`) and the contributor's feed decomposes into a multi-row breakdown
/// whose subtotals sum to the displayed weight (reproduce-by-hand; KPI-GRAPH-3).
/// Confidences vary per claim so the verbatim-render scenarios have distinct
/// numbers to assert (`0.86`, `0.90`, …).
///
/// SCAFFOLD: true (slice-09) — DELIVER materializes it via the EXISTING
/// `seed_peer_authored_graph` (`peer add` + `peer pull`), passing one `SeedPeer`
/// for `contributor_did` whose `triples` are several DISTINCT subjects on the
/// shared reproducible-builds object at varied confidences. No new mechanism — the
/// rows land in the REAL `peer_claims` table the viewer's local feed read returns.
pub fn seed_contributor_rich_trail(env: &TestEnv, contributor_did: &str) {
    // The contributor asserts the SHARED reproducible-builds object across FOUR
    // DISTINCT subjects (projects) at varied confidences, so the pure scorer sees
    // `cross_project_span >= 2` (lifting the pairing OUT of `[SPARSE]`) and the
    // feed decomposes into a multi-row breakdown. Confidences vary per claim so the
    // verbatim-render scenarios have distinct numbers to assert (`0.86`, `0.90`,
    // `0.74`, `0.62`). DISTINCT subjects keep the canonical CIDs distinct (the
    // store keys on cid; identical triples would collide into one row). Materialized
    // through the PRODUCTION federation write path (`peer add` + `peer pull`) so the
    // rows land in the REAL `peer_claims` table the viewer's LOCAL feed read returns
    // — no network at score time (Pillar 3 / BR-VIEW-4 / I-CS-5).
    seed_peer_authored_graph(env, &[rich_trail_seed_peer(contributor_did)]);
}

/// The RICH-trail [`SeedPeer`] definition (FOUR distinct subjects on the shared
/// reproducible-builds object at varied confidences → `cross_project_span >= 2`,
/// NOT sparse). Held in ONE place so the standalone [`seed_contributor_rich_trail`]
/// and the combined [`seed_contributor_rich_and_sparse_trails`] (C-10) seed the
/// IDENTICAL trail through ONE `seed_peer_authored_graph` call — no drift on the
/// subjects / confidences / seed.
fn rich_trail_seed_peer(contributor_did: &str) -> SeedPeer<'_> {
    // The triple list is inlined with the `&'static str` object const so the array
    // literal is fully constant → promoted to `'static` (the `SeedPeer<'a>` triples
    // borrow). Binding the object to a `let` first would make the array a temporary
    // owned by this fn (E0515 on return).
    SeedPeer {
        peer_did: contributor_did,
        seed: [23u8; 32],
        triples: &[
            ("github:bazelbuild/bazel", SCORE_OBJECT_REPRODUCIBLE_BUILDS, 0.86),
            ("github:NixOS/nixpkgs", SCORE_OBJECT_REPRODUCIBLE_BUILDS, 0.90),
            (
                "github:reproducible-builds/diffoscope",
                SCORE_OBJECT_REPRODUCIBLE_BUILDS,
                0.74,
            ),
            ("github:GNOME/meson", SCORE_OBJECT_REPRODUCIBLE_BUILDS, 0.62),
        ],
    }
}

/// Seed a SPARSE local trail for `contributor_did`: EXACTLY ONE claim, by that one
/// author, on ONE subject (claim_count=1, distinct_author_count=1,
/// cross_project_span=1) — through the PRODUCTION federation write path. The pure
/// core's breadth guard buckets this `[SPARSE]` regardless of how HIGH the single
/// claim's confidence is (the load-bearing epistemic-honesty fixture, US-CS-003 /
/// KPI-GRAPH-4). The confidence is intentionally HIGH (`0.95`) so the scenario
/// proves magnitude does not dress a thin opinion up as Strong.
///
/// SCAFFOLD: true (slice-09) — DELIVER materializes it via `seed_peer_authored_
/// graph` with ONE `SeedPeer` carrying a SINGLE triple (one subject, the shared
/// object, confidence 0.95).
pub fn seed_contributor_sparse_trail(env: &TestEnv, contributor_did: &str) {
    // ONE `SeedPeer{ peer_did: contributor_did, seed, triples: &[(one subject,
    // reproducible-builds, 0.95)] }` via `seed_peer_authored_graph` so exactly ONE
    // `peer_claims` row lands — a single-claim / single-author / no-span feed
    // (claim_count=1, distinct_author_count=1, cross_project_span=1) the pure core
    // buckets `[SPARSE]` at ANY confidence. The confidence is intentionally HIGH
    // (0.95) so the scenario proves magnitude does not dress a thin opinion up as
    // Strong. Materialized through the PRODUCTION federation write path (`peer add`
    // + `peer pull`) so the row lands in the REAL `peer_claims` table the viewer's
    // LOCAL feed read returns — no network at score time (Pillar 3 / I-CS-5).
    seed_peer_authored_graph(env, &[sparse_trail_seed_peer(contributor_did)]);
}

/// The VERBATIM rendered headline weight of the SPARSE trail's single pairing
/// (`0.95`). One claim, one author (rank 1 → multiplier share `1.0`), no
/// cross-project triangulation (`+0.0`), so `subtotal = 0.95 * 1.0 + 0.0 = 0.95`
/// and the pairing weight IS that lone subtotal (Gate 2: weight == Σ subtotal). Held
/// here next to `sparse_trail_seed_peer` (the `0.95` confidence source of truth) so
/// the CARDINAL gold's sparse single-row reproduce-by-hand check
/// ([`assert_score_html_single_row_subtotal_equals_weight`]) can pin the rendered
/// weight against the KNOWN seeded value — catching a `render_weight` `{:.2}` →
/// `{:.1}` format mutation on the sparse surface.
pub const CONTRIBUTOR_SPARSE_RENDERED_WEIGHT: f64 = 0.95;

/// The SPARSE-trail [`SeedPeer`] definition (ONE subject on the shared object at a
/// HIGH 0.95 confidence → claim_count=1, distinct_author_count=1,
/// cross_project_span=1 → `[SPARSE]` at any magnitude). Held in ONE place so the
/// standalone [`seed_contributor_sparse_trail`] and the combined
/// [`seed_contributor_rich_and_sparse_trails`] (C-10) seed the IDENTICAL trail.
fn sparse_trail_seed_peer(contributor_did: &str) -> SeedPeer<'_> {
    // Inlined object const (constant array → `'static` promotion; see
    // [`rich_trail_seed_peer`]).
    SeedPeer {
        peer_did: contributor_did,
        seed: [29u8; 32],
        triples: &[("github:torvalds/linux", SCORE_OBJECT_REPRODUCIBLE_BUILDS, 0.95)],
    }
}

/// Seed BOTH a RICH trail (for `rich_did`) and a SPARSE trail (for `sparse_did`) in
/// ONE `seed_peer_authored_graph` invocation (C-10 breadth-vs-magnitude). A SINGLE
/// call is REQUIRED: `seed_peer_authored_graph` keeps every peer's PDS server alive
/// only for the duration of the call AND wires the resolver/pubkey seams for ALL
/// peers into the ONE `peer pull`. Seeding the two trails through two SEPARATE calls
/// would drop the first peer's PDS before the second call's pull re-pulls every
/// subscribed peer → the first peer's DID resolution 404s. Both contributors assert
/// the SAME reproducible-builds object, so it is BREADTH (cross-project span), not
/// weight magnitude, that separates the buckets (the rich one spans ≥2 projects →
/// non-Sparse; the sparse one is a single claim → `[SPARSE]`). PRODUCTION federation
/// write path (`peer add` + `peer pull`); LOCAL only at score time.
pub fn seed_contributor_rich_and_sparse_trails(env: &TestEnv, rich_did: &str, sparse_did: &str) {
    seed_peer_authored_graph(
        env,
        &[
            rich_trail_seed_peer(rich_did),
            sparse_trail_seed_peer(sparse_did),
        ],
    );
}

/// Seed a trail where TWO DISTINCT authors assert the SAME (subject, object) at
/// DIFFERENT confidences, so the contributor's feed for that pairing decomposes
/// into TWO separate `Contribution` rows under their OWN author DIDs — never
/// averaged/merged into one faceless consensus row (the anti-merging guarantee,
/// US-CS-002 Example 4 / I-CS-2 / I-CS-10). Returns the two author DIDs (in seeded
/// order) so the scenario can assert both rows are present + attributed.
///
/// SCAFFOLD: true (slice-09) — DELIVER materializes it via `seed_own_plus_peer_
/// graph` (the GQE-2 identical-content-two-authors fixture shape): the local user
/// (`You`) + a pulled peer both assert the same (subject, reproducible-builds) at
/// different confidences, landing two attributed rows on one pairing.
pub fn seed_contributor_conflicting_authors(env: &TestEnv) -> (String, String) {
    // Mirror the GQE-2 `DenoIdenticalContentTwoAuthors` identical-content shape via
    // the PRODUCTION federation write path: the LOCAL user's OWN claim (via the real
    // `claim add` verb → `You`, author `did:plc:test-jeff`) AND a pulled PEER claim
    // (via the real `peer add` + `peer pull` verbs → `SubscribedPeer`) by a SECOND,
    // DISTINCT author both assert the SAME (subject, reproducible-builds) at DISTINCT
    // confidences (0.40 own + 0.55 peer). The own row lands in `claims`, the peer row
    // in `peer_claims`; BOTH fall under the scored contributor's author scope, so the
    // production `query_contributor_scoring_feed` read returns both and the pure
    // scorer decomposes the ONE pairing into TWO attributed `Contribution` rows under
    // their own author DIDs — never averaged/merged (anti-merging; I-CS-2 / I-CS-10).
    // NO hand-inserted store rows. Returns the two distinct author DIDs in seeded
    // order (own, peer) so the scenario can assert both rows are present + attributed.
    let repro = SCORE_OBJECT_REPRODUCIBLE_BUILDS;
    let subject = "github:denoland/deno";
    // A second, distinct PLC identity. It shares the local user's contributor scope
    // (so the contributor-feed read returns BOTH authors' claims on the shared
    // pairing) yet is a SEPARATE author DID — the row renders under its own DID, so
    // the anti-merging guarantee (two distinct authors → two rows, no average) is
    // genuinely exercised through the production peer path.
    let peer_did = "did:plc:test-jeff-collaborator";
    seed_own_plus_peer_graph(
        env,
        &[OwnClaim {
            subject,
            object: repro,
            confidence: 0.40,
        }],
        &[SeedPeer {
            peer_did,
            seed: [37u8; 32],
            triples: &[(subject, repro, 0.55)],
        }],
    );
    (env.identity.author_did().to_string(), peer_did.to_string())
}

/// Seed ALL THREE contributor-score postures — RICH (`rich_did`), SPARSE
/// (`sparse_did`), and CONFLICTING-authors (the LOCAL user `You`) — in ONE
/// `peer pull` so the CARDINAL transparency gate can exercise every posture over
/// ONE viewer/store. A SINGLE combined call is REQUIRED for the SAME reason
/// [`seed_contributor_rich_and_sparse_trails`] documents: `seed_own_plus_peer_graph`
/// keeps every peer's PDS alive only for the duration of the ONE call AND wires the
/// resolver/pubkey seams for ALL peers into the ONE pull. Seeding the postures
/// through SEPARATE seeder calls would drop the earlier peers' PDS before a later
/// call's pull re-pulls every subscribed peer → the earlier peers' DID resolution
/// 404s (the failure mode the per-seeder docs warn about).
///
/// The conflicting posture is the GQE-2 identical-content shape: the LOCAL user's
/// OWN claim (deno @ 0.40, via the real `claim add` verb → `You`) plus a SECOND,
/// DISTINCT collaborator peer (deno @ 0.55) both assert the SAME (subject,
/// reproducible-builds) pairing, so the local user's contributor feed decomposes
/// that ONE pairing into TWO attributed rows — never merged. The rich + sparse peers
/// ride the SAME single pull. PRODUCTION federation write path; LOCAL only at score
/// time. Returns the conflicting contributor's DID (the LOCAL user).
pub fn seed_contributor_rich_sparse_and_conflicting(
    env: &TestEnv,
    rich_did: &str,
    sparse_did: &str,
) -> String {
    let repro = SCORE_OBJECT_REPRODUCIBLE_BUILDS;
    let conflict_subject = "github:denoland/deno";
    let collaborator_did = "did:plc:test-jeff-collaborator";
    seed_own_plus_peer_graph(
        env,
        // The LOCAL user's own claim on the conflicting pairing (deno @ 0.40 → `You`).
        &[OwnClaim {
            subject: conflict_subject,
            object: repro,
            confidence: 0.40,
        }],
        // ALL peers pulled in ONE invocation: the collaborator (deno @ 0.55, the
        // SECOND author on the conflicting pairing) + the rich trail + the sparse
        // trail. The rich/sparse SeedPeer shapes are the SAME single-source-of-truth
        // builders the standalone seeders use, so the postures never drift.
        &[
            SeedPeer {
                peer_did: collaborator_did,
                seed: [37u8; 32],
                triples: &[(conflict_subject, repro, 0.55)],
            },
            rich_trail_seed_peer(rich_did),
            sparse_trail_seed_peer(sparse_did),
        ],
    );
    env.identity.author_did().to_string()
}

/// The `#score-results` swap-target id the `/score` fragment renders under (the
/// sibling of slice-08's `#search-results`). Asserting it appears in BOTH the
/// fragment and the full-page score region proves the parity-by-construction
/// embedding (I-CS-7). One source of truth so the scenarios never drift.
pub const SCORE_RESULTS_ID: &str = "score-results";

/// Assert a rendered `/score` body (fragment OR full page) shows a pairing's
/// transparent breakdown: the displayed weight AND a per-claim breakdown that
/// NAMES every expected contributing author DID + carries a verbatim confidence —
/// and NO opaque-number / merged-consensus render. The OBSERVABLE counterpart of
/// the slice-04 `--explain` decomposition (I-CS-2 / I-CS-10 / KPI-GRAPH-3); scans
/// the rendered HTML only (Mandate 8 universe = port-exposed rendered surface,
/// never an internal struct field).
///
/// SCAFFOLD: true (slice-09) — DELIVER asserts: every `expected_author` DID is in
/// the body (per-row attribution); each `expected_confidence_verbatim` string is
/// present byte-for-byte (`0.86`, never `0.9`/`86%`); and NO merged-consensus
/// phrasing appears (the breakdown is per-claim, not a faceless aggregate).
pub fn assert_score_html_breakdown_attributed_and_verbatim(
    body: &str,
    expected_authors: &[&str],
    expected_confidences_verbatim: &[&str],
) {
    // Every contributing author DID is rendered (per-row attribution — non-Option
    // author_did, I-CS-10).
    for did in expected_authors {
        assert!(
            body.contains(did),
            "I-CS-10: the `/score` breakdown must attribute a row to {did:?} (every \
             contribution carries its author DID); body was:\n{body}"
        );
    }
    // Each confidence is rendered VERBATIM — the exact stored `f64` string, never a
    // truncated `0.9` or a `%`-formatted value (I-CS-6 / KPI-4).
    for conf in expected_confidences_verbatim {
        assert!(
            body.contains(conf),
            "I-CS-6: the `/score` breakdown must render the confidence {conf:?} \
             verbatim (never 0.9 / 90%); body was:\n{body}"
        );
    }
    // The score is NEVER a faceless merged consensus number — the breakdown is
    // per-claim (anti-merging in aggregates, I-CS-2).
    let lowered = body.to_ascii_lowercase();
    for banned in [
        "authors agree",
        "the network says",
        "consensus score",
        "community consensus",
    ] {
        assert!(
            !lowered.contains(banned),
            "I-CS-2 (anti-merging): the `/score` render must show NO merged / \
             faceless consensus row; found {banned:?} in body:\n{body}"
        );
    }
}

/// Read the canonical CIDs of every `peer_claims` row attributed to `peer_did`
/// (the rows `peer pull` recomputed + stored), so a `/score` scenario can assert
/// the rendered breakdown NAMES each contributing claim's cid (I-CS-10). Read
/// from the SAME store the viewer's local feed read returns; the CIDs are the
/// production-recomputed values, never hand-stamped fixtures. Held here so the
/// cid-naming assertion never drifts from the seeded rows.
pub fn read_peer_claim_cids_for(env: &TestEnv, peer_did: &str) -> Vec<String> {
    let db_path = env.duckdb_path();
    let conn = duckdb::Connection::open(&db_path).unwrap_or_else(|err| {
        panic!(
            "open DuckDB at {} for peer_claims cid read: {err}",
            db_path.display()
        )
    });
    let mut stmt = conn
        .prepare("SELECT cid FROM peer_claims WHERE author_did = ?")
        .unwrap_or_else(|err| panic!("prepare peer_claims cid read: {err}"));
    let rows = stmt
        .query_map(duckdb::params![peer_did], |row| row.get::<_, String>(0))
        .unwrap_or_else(|err| panic!("query peer_claims cids for {peer_did}: {err}"));
    rows.map(|r| r.expect("decode peer_claims cid")).collect()
}

/// Read every OWN-claim CID from the env's REAL `claims` table in the EXACT slice-06
/// `/claims` list render order (`composed_at DESC, cid` — mirrors
/// `DuckDbStoreReadAdapter::list_claims`). The slice-12 list-flag seeds return their
/// CIDs in this order so a scenario can address rows by their rendered position +
/// the no-regression gold can pin order byte-identity. Read-only; opens a SECOND
/// short-lived connection (the env's `ui` viewer holds its own handle).
pub fn read_own_claim_cids_in_list_order(env: &TestEnv) -> Vec<String> {
    let db_path = env.duckdb_path();
    let conn = duckdb::Connection::open(&db_path).unwrap_or_else(|err| {
        panic!(
            "open DuckDB at {} for claims cid read: {err}",
            db_path.display()
        )
    });
    let mut stmt = conn
        .prepare("SELECT cid FROM claims ORDER BY composed_at DESC, cid")
        .unwrap_or_else(|err| panic!("prepare own claims list-order cid read: {err}"));
    let rows = stmt
        .query_map([], |row| row.get::<_, String>(0))
        .unwrap_or_else(|err| panic!("query own claims cids in list order: {err}"));
    rows.map(|r| r.expect("decode own claims cid")).collect()
}

/// Read the RECORDED slice-06 baseline for the own-claims `/claims` list of THIS store
/// (tactic (b) for the LF-6 no-regression gold): every own-claim CID in the slice-06
/// `composed_at DESC, cid` order, paired with that row's VERBATIM confidence cell string
/// (`render_confidence` → `"0.90"`, two decimals). Reads the SAME `claims` table the viewer
/// reads, in the SAME order the slice-06 list SQL uses, so the returned [`Slice06Baseline`]
/// is the no-flag reference render's order + count + confidence WITHOUT depending on a
/// pre-flag binary or a no-flag HTTP seam (neither exists). The CID-canonicalizes-composed_at
/// constraint that rules out a twin-store baseline does NOT affect this read — it reflects
/// the SAME rows the flagged render projects.
pub fn read_slice06_list_baseline(env: &TestEnv) -> Slice06Baseline {
    let db_path = env.duckdb_path();
    let conn = duckdb::Connection::open(&db_path).unwrap_or_else(|err| {
        panic!(
            "open DuckDB at {} for slice-06 baseline read: {err}",
            db_path.display()
        )
    });
    let mut stmt = conn
        .prepare("SELECT cid, confidence FROM claims ORDER BY composed_at DESC, cid")
        .unwrap_or_else(|err| panic!("prepare slice-06 baseline read: {err}"));
    let rows = stmt
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, f64>(1)?))
        })
        .unwrap_or_else(|err| panic!("query slice-06 baseline rows: {err}"));
    let mut ordered_cids = Vec::new();
    let mut confidence_cells = Vec::new();
    for r in rows {
        let (cid, confidence) = r.expect("decode slice-06 baseline row");
        ordered_cids.push(cid);
        // Mirror viewer-domain `render_confidence`: two decimals, VERBATIM (0.90, never 0.9).
        confidence_cells.push(format!("{confidence:.2}"));
    }
    Slice06Baseline {
        ordered_cids,
        confidence_cells,
    }
}

/// Assert a rendered `/score` body NAMES every expected claim cid in its per-claim
/// breakdown (I-CS-10): every breakdown row identifies the contributing claim by
/// its cid, so the weight is never an opaque number detached from the claims that
/// produced it. The OBSERVABLE counterpart of the slice-04 `--explain` cid column;
/// scans the rendered HTML only (Mandate 8 universe = port-exposed rendered
/// surface). Used ALONGSIDE [`assert_score_html_breakdown_attributed_and_verbatim`]
/// (author DID + verbatim confidence) to pin the FULL per-row identity
/// (author_did + cid) the C-4 anti-opaque-number criterion requires.
pub fn assert_score_html_breakdown_names_cids(body: &str, expected_cids: &[String]) {
    assert!(
        !expected_cids.is_empty(),
        "I-CS-10: the cid-naming assertion needs ≥1 expected cid (the seeded rich \
         trail produced none); body was:\n{body}"
    );
    for cid in expected_cids {
        assert!(
            body.contains(cid.as_str()),
            "I-CS-10: the `/score` breakdown must name the contributing claim cid \
             {cid:?} (every row identifies its claim — the weight is never an opaque \
             number); body was:\n{body}"
        );
    }
}

/// Assert the rendered `/score` body marks a pairing `[SPARSE]` and carries the
/// "treat as a lead, not a conclusion" honesty line — the projected breadth-guard
/// state for thin evidence (I-CS-3 / KPI-GRAPH-4). Scans the OBSERVABLE rendered
/// surface only; the `[SPARSE]` decision is the pure core's (`WeightBucket::
/// Sparse`), the viewer PROJECTS it (never recomputes). Also asserts the pairing
/// is NOT labelled `Strong` (a thin opinion is never dressed up by magnitude).
///
/// SCAFFOLD: true (slice-09).
pub fn assert_score_html_renders_sparse_honesty(body: &str) {
    assert!(
        body.contains("[SPARSE]"),
        "I-CS-3: a thin (single-claim/single-author/no-span) pairing must render \
         the `[SPARSE]` marker; body was:\n{body}"
    );
    let lowered = body.to_ascii_lowercase();
    assert!(
        lowered.contains("treat as a lead"),
        "I-CS-3: a `[SPARSE]` pairing must carry the \"treat as a lead, not a \
         conclusion\" honesty line; body was:\n{body}"
    );
    // The honesty line carries the COUNTS (N claims, M authors) PROJECTED from the
    // pure core's `claim_count` / `distinct_author_count` (WD-CS-6 — the viewer
    // recomputes NO bucket and NO counts): "based on 1 claim(s) by 1 author(s)".
    // A single-claim / single-author sparse trail is N=1, M=1.
    assert!(
        lowered.contains("based on 1 claim") && lowered.contains("by 1 author"),
        "I-CS-3 / KPI-GRAPH-4: a `[SPARSE]` pairing's honesty line must name the \
         projected counts (based on N claim(s) by M author(s)) — here N=1, M=1; \
         body was:\n{body}"
    );
    // The thin pairing is NEVER labelled Strong, regardless of confidence magnitude
    // (the breadth guard, not the weight, decides the bucket).
    assert!(
        !body.contains("Strong"),
        "I-CS-3: a thin pairing must NOT be labelled Strong (magnitude does not \
         dress a single opinion up as well-supported); body was:\n{body}"
    );
}

/// Assert the rendered `/score` body shows the guided `NoClaims` empty state for a
/// contributor with no local claims — the fixed plain-language "No local claims for
/// that contributor." notice, naming nothing fake (OD-CS-6 / I-CS-5). Asserts the
/// queried DID is named (so the operator knows WHO was looked up) and that NO score
/// / weight / `[SPARSE]` / stack-trace leaks (emptiness is not a zero score).
///
/// SCAFFOLD: true (slice-09).
pub fn assert_score_html_renders_no_claims(body: &str, queried_did: &str) {
    let lowered = body.to_ascii_lowercase();
    assert!(
        lowered.contains("no local claims"),
        "OD-CS-6: an unknown contributor must render the guided \"No local claims \
         for that contributor.\" notice; body was:\n{body}"
    );
    assert!(
        body.contains(queried_did),
        "OD-CS-6: the empty state must name the queried DID {queried_did:?} so the \
         operator knows who was looked up; body was:\n{body}"
    );
    // Emptiness is NOT a zero score — no fabricated weight/bucket appears, and no
    // raw error/stack trace leaks (I-CS-5 reliability).
    for banned in ["[SPARSE]", "weight", "panicked", "RUST_BACKTRACE", "thread 'main'"] {
        assert!(
            !body.contains(banned),
            "OD-CS-6: the empty state must show NO fabricated score and NO leaked \
             error internals; found {banned:?} in body:\n{body}"
        );
    }
}

/// Assert the running sum of a pairing's per-claim subtotals EQUALS its displayed
/// weight, READ FROM THE RENDERED HTML (the J-002c reproduce-by-hand release gate,
/// KPI-GRAPH-3 / I-CS-2). This is the cardinal transparency-by-construction
/// assertion: it parses the headline weight + the per-row subtotals out of the
/// rendered breakdown TABLE and checks `Σ subtotal == weight` (within an epsilon) —
/// proving the operator can reproduce the number by hand from what she SEES, not by
/// trusting the pure core off-screen.
///
/// SCAFFOLD: true (slice-09) — DELIVER parses the rendered weight value + the
/// per-row subtotal values out of the breakdown markup (the table the renderer
/// emits) and asserts the row subtotals sum to the displayed weight (|Σ − weight| <
/// 1e-9). The PARSE shape is DELIVER's against the render tests; the CONTRACT
/// (rendered subtotals sum to the rendered weight) is fixed here.
pub fn assert_score_html_breakdown_sums_to_displayed_weight(body: &str) {
    // Parse the OBSERVABLE rendered surface (Mandate 8 universe = port-exposed
    // rendered HTML), never the in-process `WeightedPairing` — the whole point is
    // that the HTML itself is self-consistent. Each pairing renders as one
    // `<section>` carrying its headline `Weight: <2dp>` (a `<p>`) and a breakdown
    // `<table>` whose `<tbody>` rows end in the per-claim `Subtotal` `<td>` (the
    // last cell). For EACH pairing the running sum of the row subtotals must equal
    // that pairing's displayed weight (reproduce-by-hand; KPI-GRAPH-3).
    let pairings = parse_score_pairings(body);
    assert!(
        !pairings.is_empty(),
        "C-5 reproduce-by-hand: the rendered `/score` body must carry ≥1 pairing \
         <section> with a Weight + breakdown table to reproduce by hand; body \
         was:\n{body}"
    );
    // The whole rendered surface must decompose >1 contribution overall — a
    // reproduce-by-hand gate over a TRIVIAL single row proves nothing (the brief's
    // multi-row requirement). The rich trail seeds a multi-pairing/multi-row feed.
    let total_rows: usize = pairings.iter().map(|p| p.subtotals.len()).sum();
    assert!(
        total_rows > 1,
        "C-5 reproduce-by-hand must hold over a NON-trivial breakdown (>1 \
         contribution rendered across the pairings, not a single row); parsed \
         {total_rows} subtotal row(s) from body:\n{body}"
    );
    for pairing in &pairings {
        assert!(
            !pairing.subtotals.is_empty(),
            "C-5: a rendered pairing (weight {}) carried a breakdown table with NO \
             subtotal rows — a weight must never appear without its decomposition; \
             body was:\n{body}",
            pairing.weight
        );
        let running: f64 = pairing.subtotals.iter().sum();
        // Both the weight and each subtotal are rendered to two decimals, so a row's
        // displayed value carries up to 0.005 of rounding error; the sum of `n`
        // rows is within `0.005 * n` of the displayed weight (plus the weight's own
        // ≤0.005). A re-render is byte-identical, so this tolerance never flakes.
        let epsilon = 0.005 * (pairing.subtotals.len() as f64 + 1.0);
        assert!(
            (running - pairing.weight).abs() <= epsilon,
            "C-5 reproduce-by-hand (KPI-GRAPH-3): the running sum of the rendered \
             per-claim subtotals ({running:.2}) must equal the displayed pairing \
             weight ({:.2}); subtotals parsed = {:?}; body was:\n{body}",
            pairing.weight,
            pairing.subtotals,
        );
    }
}

/// Assert the SPARSE single-row reproduce-by-hand case on a rendered `/score` body:
/// the displayed pairing weight EQUALS its single breakdown-row subtotal, BOTH read
/// from the rendered HTML, AND that displayed weight matches `expected_weight`
/// verbatim (the known seeded value). This is the trivial-but-real Σ(one row) ==
/// weight case the multi-row [`assert_score_html_breakdown_sums_to_displayed_weight`]
/// guard deliberately skips (it requires >1 row), so the sparse surface's render
/// path is left otherwise un-pinned. Closing that gap: it parses the OBSERVABLE
/// rendered surface (Mandate 8 universe = port-exposed HTML, never the in-process
/// `WeightedPairing`), reusing the SAME `parse_score_pairings` logic as the multi-row
/// helper, so the sparse `render_weight` (`{:.2}`) formatter is exercised here too.
///
/// Catches a `render_weight` format mutation: `{:.2}` → `{:.1}` renders the headline
/// weight as `1.0` instead of `0.95`, so the parsed weight no longer equals the
/// `expected_weight` literal (the equality-vs-subtotal half stays true under that
/// mutation because BOTH cells share `render_weight`, but the verbatim literal half
/// fails). Asserts exactly ONE pairing with exactly ONE subtotal row (the sparse
/// shape by construction); a multi-row body here would mean the wrong posture.
pub fn assert_score_html_single_row_subtotal_equals_weight(body: &str, expected_weight: f64) {
    // Reuse the SAME observable-HTML parse the multi-row reproduce-by-hand gate uses
    // (a `<section>` per pairing → headline `Weight:` + the breakdown table's last-
    // `<td>` subtotals), so the sparse surface is held to the identical render
    // contract — never an internal struct.
    let pairings = parse_score_pairings(body);
    assert_eq!(
        pairings.len(),
        1,
        "sparse reproduce-by-hand: the SPARSE `/score` body must render EXACTLY one \
         pairing <section> (single subject/object); parsed {} pairing(s) from body:\n{body}",
        pairings.len()
    );
    let pairing = &pairings[0];
    assert_eq!(
        pairing.subtotals.len(),
        1,
        "sparse reproduce-by-hand: the SPARSE pairing must render EXACTLY one \
         breakdown subtotal row (single claim, single author); parsed {} subtotal \
         row(s): {:?}; body was:\n{body}",
        pairing.subtotals.len(),
        pairing.subtotals
    );
    let subtotal = pairing.subtotals[0];

    // (a) The trivial-but-real Σ(one row) == weight case, on the OBSERVABLE surface:
    // the lone parsed subtotal equals the parsed displayed weight (both two-decimal
    // renders, so |diff| < 0.005 covers the per-cell rounding; a re-render is byte-
    // identical so this never flakes). This pins the single-row arithmetic the multi-
    // row helper skips.
    assert!(
        (subtotal - pairing.weight).abs() < 0.005,
        "sparse reproduce-by-hand: the SPARSE pairing's lone rendered subtotal \
         ({subtotal:.2}) must equal its displayed weight ({:.2}) — the trivial \
         Σ(one row) == weight case; body was:\n{body}",
        pairing.weight
    );

    // (b) The displayed weight matches the KNOWN seeded value VERBATIM. This is the
    // load-bearing half that catches a `render_weight` `{:.2}` → `{:.1}` mutation:
    // the equality-vs-subtotal half above survives such a mutation (both cells share
    // `render_weight`, so both shift together), but the rendered weight would read
    // `1.0` instead of `0.95`, so the parsed value no longer equals `expected_weight`.
    assert!(
        (pairing.weight - expected_weight).abs() < 0.005,
        "sparse reproduce-by-hand: the SPARSE pairing's displayed weight \
         ({:.2}) must equal the known seeded value ({expected_weight:.2}) verbatim — \
         a `render_weight` format mutation (e.g. `{{:.2}}` → `{{:.1}}`) would render \
         the wrong value here; body was:\n{body}",
        pairing.weight
    );
}

/// Assert the ANTI-OPAQUE structural invariant on a rendered `/score` body: NO
/// displayed weight EVER appears without its accompanying per-claim breakdown table
/// (I-CS-2 / J-002c, the anti-opaque half of the CARDINAL gate). An opaque-number
/// regression — rendering a `Weight:` headline while hiding the per-claim breakdown
/// — silently re-creates the aggregator failure J-002 exists to avoid; this guard
/// makes it unshippable. Distinct from
/// [`assert_score_html_breakdown_sums_to_displayed_weight`] (which checks the
/// ARITHMETIC Σ-subtotal == weight): THIS guard checks the STRUCTURE — every weight
/// is STRUCTURALLY paired with a breakdown `<table>`, scanning the OBSERVABLE
/// rendered HTML only (Mandate 8 universe = port-exposed rendered surface).
///
/// Scans EVERY pairing `<section>` (the unit a weight renders under): a section that
/// shows a `Weight:` headline MUST also carry a breakdown `<table>`. Also asserts at
/// least one weight is present (a vacuous pass over a body with no weight would prove
/// nothing on a Scored surface).
pub fn assert_score_html_every_weight_has_a_breakdown(body: &str) {
    // Split on the per-pairing `<section>` boundary (the SAME boundary
    // `parse_score_pairings` uses). Text before the first `<section>` is chrome.
    let sections: Vec<&str> = body.split("<section").skip(1).collect();

    // The whole rendered surface must show ≥1 weight — a guard that passes vacuously
    // over a body with NO `Weight:` proves nothing on a Scored surface.
    let total_weights = body.matches("Weight:").count();
    assert!(
        total_weights >= 1,
        "anti-opaque (I-CS-2 / J-002c): the rendered `/score` body must show ≥1 \
         `Weight:` headline for the breakdown-presence guard to be meaningful; body \
         was:\n{body}"
    );

    // EVERY section that shows a weight MUST carry a breakdown `<table>` — no weight
    // is ever an opaque number detached from its per-claim decomposition. A weight
    // rendered OUTSIDE any `<section>` (i.e. in the chrome, before the first section)
    // would also be a breach: every `Weight:` occurrence must fall inside a section
    // that carries a table, so the in-section count must equal the total count.
    let mut weights_in_sections = 0usize;
    for section in &sections {
        let section_weights = section.matches("Weight:").count();
        if section_weights == 0 {
            continue;
        }
        weights_in_sections += section_weights;
        assert!(
            section.contains("<table"),
            "anti-opaque (I-CS-2 / J-002c): a `/score` pairing section renders a \
             `Weight:` headline with NO accompanying breakdown `<table>` — a weight \
             must NEVER be shown without its per-claim breakdown (opaque-number \
             regression); offending section:\n<section{section}"
        );
    }
    assert_eq!(
        weights_in_sections, total_weights,
        "anti-opaque (I-CS-2 / J-002c): every displayed `Weight:` must fall inside a \
         pairing <section> that carries a breakdown table; found {total_weights} \
         weight(s) total but only {weights_in_sections} inside breakdown-bearing \
         sections (a weight rendered outside any breakdown section is an opaque \
         number); body was:\n{body}"
    );
}

/// One parsed `/score` pairing: its displayed headline weight + the per-claim
/// subtotals read out of the breakdown table — both extracted from the OBSERVABLE
/// rendered HTML (never an internal struct). Used by
/// [`assert_score_html_breakdown_sums_to_displayed_weight`].
struct ParsedScorePairing {
    weight: f64,
    subtotals: Vec<f64>,
}

/// Parse every rendered pairing out of a `/score` body: split on the per-pairing
/// `<section>` boundary, then for each section extract the headline `Weight:`
/// value and the breakdown table's per-row subtotals (the LAST `<td>` of each
/// `<tbody>` row — the `Subtotal` column the renderer emits last). Plain string
/// scanning over the maud-emitted (compact) markup; no HTML/regex dependency so
/// the helper stays self-contained in the support module.
fn parse_score_pairings(body: &str) -> Vec<ParsedScorePairing> {
    body.split("<section")
        .skip(1) // text before the first <section> is chrome, never a pairing
        .filter_map(|section| {
            let weight = parse_weight(section)?;
            let subtotals = parse_row_subtotals(section);
            Some(ParsedScorePairing { weight, subtotals })
        })
        .collect()
}

/// Extract the headline weight from one pairing section: the number following the
/// rendered `Weight: ` text node (e.g. `Weight: 1.36 Strong`).
fn parse_weight(section: &str) -> Option<f64> {
    let after = section.split("Weight: ").nth(1)?;
    parse_leading_f64(after)
}

/// Extract each breakdown row's subtotal (the LAST `<td>` of every `<tbody>`
/// row). Scopes to the `<tbody>` so the `<thead>` header cells are never parsed
/// as numbers; the subtotal is the last `<td>` because the renderer emits it last
/// (Author, CID, Confidence, Author bonus, Triangulation bonus, Subtotal).
fn parse_row_subtotals(section: &str) -> Vec<f64> {
    let tbody = match section.split_once("<tbody>") {
        Some((_, rest)) => rest.split("</tbody>").next().unwrap_or(rest),
        None => return Vec::new(),
    };
    tbody
        .split("<tr>")
        .skip(1)
        .filter_map(|row| {
            // The subtotal is the LAST cell: take the text of the final `<td>`.
            let last_cell = row.rsplit("<td>").next()?;
            let value = last_cell.split("</td>").next()?;
            value.trim().parse::<f64>().ok()
        })
        .collect()
}

/// Parse the leading floating-point number out of a string slice (digits, an
/// optional leading sign, and a single decimal point), ignoring any trailing
/// markup/text. Returns `None` when no number leads the slice.
fn parse_leading_f64(s: &str) -> Option<f64> {
    let trimmed = s.trim_start();
    let mut seen_dot = false;
    let end = trimmed
        .char_indices()
        .take_while(|(i, c)| {
            c.is_ascii_digit()
                || (*c == '-' && *i == 0)
                || (*c == '.' && !std::mem::replace(&mut seen_dot, true))
        })
        .last()
        .map(|(i, c)| i + c.len_utf8())?;
    trimmed[..end].parse::<f64>().ok()
}

/// Assert a rendered `/score` body carries NO sign / publish / follow / subscribe
/// control — the read-only-surface guarantee on the score view (I-CS-1 / WD-CS-3).
/// The sibling of the slice-08 search no-write assertion. Scans the OBSERVABLE
/// rendered surface for control elements + bare control labels.
///
/// SCAFFOLD: true (slice-09).
pub fn assert_score_html_has_no_write_or_sign_control(body: &str) {
    let lowered = body.to_ascii_lowercase();
    for banned in [
        "name=\"sign\"",
        "sign claim",
        "sign & publish",
        "sign &amp; publish",
        "subscribe",
        ">follow<",
    ] {
        assert!(
            !lowered.contains(banned),
            "I-CS-1 / WD-CS-3: the `/score` surface must render NO sign / publish / \
             follow / subscribe control (the score is a read + pure compute; \
             signing/following stays in the CLI); found {banned:?} in body:\n{body}"
        );
    }
}

// =============================================================================
// slice-10 (viewer-graph-traversal) `/project` + `/philosophy` traversal harness
// (US-GT-002/003/004; ADR-042/043/044/045). The two new LOCAL read-only routes —
// `GET /project?subject=<uri>` (project survey) and `GET /philosophy?object=<uri>`
// (philosophy survey) — plus the cross-link wiring that turns every survey row
// into a clickable traversal edge.
//
// Driving discipline (Mandate 1): scenarios enter through the REAL `openlore ui`
// subprocess (`ViewerServer`) + in-test HTTP GET (with/without `HX-Request` — the
// slice-07 `get`/`get_htmx` pair). The LOCAL DuckDB store is REAL, seeded through
// the PRODUCTION federation write path (`seed_own_plus_peer_graph` /
// `seed_peer_authored_graph` — the SAME `peer add` + `peer pull` seam slice-09
// uses), so the rows the survey reads are produced by production code, not
// hand-inserted (Pillar 3 / BR-VIEW-4). NO external/network boundary exists —
// `/project` + `/philosophy` are LOCAL + OFFLINE (distinct from `/scrape`'s GitHub
// edge and `/search`'s indexer edge; offline-STRONGER than `/search` — I-GT-2 /
// I-GT-7). NO scenario calls the `viewer-domain` `render_project_*` /
// `render_philosophy_*` fns OR the `group_*` projections directly (those are
// unit/property-level, exercised in DELIVER) — every assertion is on the rendered
// HTML the operator's browser shows (Mandate 8 universe = port-exposed rendered
// surface, never an internal struct field).
//
// Layer placement (nw-tdd-methodology Layered Test Discipline matrix): every
// traversal scenario is a layer-3/layer-5 subprocess + real-I/O test — EXAMPLE-only
// (Mandate 9/11). The sad paths (no-claims, the injection URI) are enumerated
// explicitly, never PBT-generated at this layer (the generative exploration of the
// pure group/render core + the `encode_query_component` round-trip PROPERTY are a
// layer-1/2 DELIVER concern).
//
// Anti-merging seeding postures (the key harness pieces):
//   - PROJECT trail → `seed_project_survey_trail`: ONE subject (a project) with
//                     several attributed claims on DISTINCT objects (philosophies)
//                     so the project survey groups multiple philosophies embodied,
//                     each an attributed edge.
//   - PHILOSOPHY    → `seed_philosophy_survey_trail`: ONE object (a philosophy)
//                     embodied by several DISTINCT subjects (projects), each an
//                     attributed edge — with a SHARED contributor spanning ≥2
//                     projects (the canonical "aha").
//   - TWO AUTHORS   → `seed_two_author_same_edge`: two DISTINCT authors claim the
//                     SAME (subject, object) → two attributed rows under their own
//                     author_dids, never merged/averaged (WD-GT-5 / I-GT-3).
//   - EMPTY         → no seeding for an unknown entity → guided `NoClaims`.
//   - INJECTION URI → `seed_injection_uri_subject`: a claim-controlled subject
//                     carrying HTML/quote/`&`/space characters → the survey row's
//                     href must percent-ENCODE it (the ADR-044 §security boundary).
// =============================================================================

/// The canonical slice-10 traversal entities — one source of truth so the seed
/// call + the rendered-edge assertions never drift on the URI strings. Mirrors the
/// journey's real data (cargo / nixpkgs / bazel / dependency-pinning /
/// reproducible-builds; did:plc:maria-test / rachel-test / tobias-test).
pub const TRAVERSAL_PROJECT_CARGO: &str = "github:rust-lang/cargo";
pub const TRAVERSAL_PROJECT_NIXPKGS: &str = "github:NixOS/nixpkgs";
pub const TRAVERSAL_PROJECT_BAZEL: &str = "github:bazelbuild/bazel";
pub const TRAVERSAL_PROJECT_UNKNOWN: &str = "github:nonexistent/repo";

/// The percent-encoded forms of the well-known project subjects as they MUST appear
/// in a traversal href's `?subject=` query component (the `:` and `/` of a
/// `github:owner/repo` URI → `%3A` / `%2F`; ADR-044 `encode_query_component`). One
/// source of truth so the cross-link assertions pin the EXACT encoded value the
/// production helper must emit. `github:NixOS/nixpkgs` → `github%3ANixOS%2Fnixpkgs`;
/// `github:bazelbuild/bazel` → `github%3Abazelbuild%2Fbazel`.
pub const TRAVERSAL_PROJECT_NIXPKGS_ENCODED: &str = "github%3ANixOS%2Fnixpkgs";
pub const TRAVERSAL_PROJECT_BAZEL_ENCODED: &str = "github%3Abazelbuild%2Fbazel";
pub const TRAVERSAL_PHILOSOPHY_DEP_PINNING: &str = "org.openlore.philosophy.dependency-pinning";
pub const TRAVERSAL_PHILOSOPHY_REPRO_BUILDS: &str = "org.openlore.philosophy.reproducible-builds";
pub const TRAVERSAL_PHILOSOPHY_UNKNOWN: &str = "org.openlore.philosophy.actor-model";

/// The two SECOND-author DIDs the anti-merging surveys attribute their peer rows
/// to (the LOCAL user is the OWN author). Rachel is the canonical spanning
/// contributor (claims across ≥2 projects); Tobias is the second author on a
/// shared project (the two-authors-one-project no-merge fixture).
pub const TRAVERSAL_AUTHOR_RACHEL: &str = "did:plc:rachel-test";
pub const TRAVERSAL_AUTHOR_TOBIAS: &str = "did:plc:tobias-test";

/// The `#traversal-results` swap-target id the `/project` + `/philosophy` fragment
/// renders under (the sibling of slice-09's `#score-results` + slice-08's
/// `#search-results`). Asserting it appears in BOTH the fragment and the full-page
/// results region proves the parity-by-construction embedding (I-GT-6). One source
/// of truth so the scenarios never drift.
pub const TRAVERSAL_RESULTS_ID: &str = "traversal-results";

/// A claim-controlled subject URI carrying HOSTILE characters — an HTML/attribute
/// breakout attempt a PEER could author into a signed claim (`"`, `<`, `>`, `&`,
/// and a space). It is the ADR-044 §security fixture: when this subject renders as
/// a `/philosophy` survey row (or any cross-link), its href MUST percent-ENCODE
/// every reserved/unsafe byte so the value cannot break out of the `href` attribute
/// or smuggle a second query param. The raw value also round-trips back to the
/// stored subject verbatim (the linked key resolves to the SAME survey).
pub const TRAVERSAL_INJECTION_SUBJECT: &str = "github:evil/x\"><script>&q= space";

/// The percent-encoded form of [`TRAVERSAL_INJECTION_SUBJECT`] as it MUST appear in
/// a traversal href's `?subject=` query component (every byte outside the
/// unreserved set `A-Z a-z 0-9 - _ . ~` → `%XX`). One source of truth so the
/// injection-boundary assertion pins the EXACT encoded value the production
/// `encode_query_component` (ADR-044) must emit. `"` → `%22`, `<` → `%3C`, `>` →
/// `%3E`, `&` → `%26`, `=` → `%3D`, space → `%20`, `:` → `%3A`, `/` → `%2F`.
pub const TRAVERSAL_INJECTION_SUBJECT_ENCODED: &str =
    "github%3Aevil%2Fx%22%3E%3Cscript%3E%26q%3D%20space";

/// Seed a PROJECT-survey trail through the PRODUCTION federation write path
/// (`peer add` + `peer pull` against a `PeerPds` double — the SAME store `openlore
/// ui` opens, Pillar 3 / BR-VIEW-4): ONE subject (`project`) carrying several
/// attributed claims on DISTINCT objects (philosophies) at varied confidences, so
/// the project survey groups MULTIPLE philosophies-embodied edges (US-GT-002). The
/// claims are authored by the spanning contributor (`author_did`) so the survey's
/// "Contributors who claimed" list names that DID (a link to `/score`). DISTINCT
/// objects keep the canonical CIDs distinct (the store keys on cid). Confidences
/// vary so the verbatim-render + bucket scenarios have distinct numbers to assert
/// (`0.90` triangulated, `0.74` well-evidenced, `0.25` speculative).
///
/// SCAFFOLD: true (slice-10) — DELIVER materializes it via the EXISTING
/// `seed_peer_authored_graph` (`peer add` + `peer pull`), one `SeedPeer` for
/// `author_did` whose `triples` are several DISTINCT objects on the shared
/// `project` subject. No new mechanism — the rows land in the REAL `peer_claims`
/// table the viewer's LOCAL project-survey read returns.
pub fn seed_project_survey_trail(env: &TestEnv, project: &str, author_did: &str) {
    // The contributor asserts the SHARED `project` subject across THREE DISTINCT
    // objects (philosophies) at varied confidences, so the project survey groups
    // three philosophies-embodied edges, each attributed to `author_did`. DISTINCT
    // objects keep the canonical CIDs distinct (identical triples would collide
    // into one row). Materialized through the PRODUCTION federation write path so
    // the rows land in the REAL `peer_claims` table the viewer's LOCAL survey read
    // returns — no network at survey time (Pillar 3 / I-GT-2).
    // ONE `SeedPeer{ peer_did: author_did, triples: &[(project, philosophy_i, conf_i)] }`
    // via `seed_peer_authored_graph` so THREE `peer_claims` rows land on the SHARED
    // `project` subject across DISTINCT objects (philosophies) at varied confidences
    // (dependency-pinning 0.90 → triangulated, a second philosophy 0.74 →
    // well-evidenced, a third 0.25 → speculative). DISTINCT objects keep the canonical
    // CIDs distinct (identical triples collide into one row). The single contributor
    // (`author_did`) is named under "Contributors who claimed" (a `/score` link).
    // Materialized through the PRODUCTION federation write path (`peer add` +
    // `peer pull`) so the rows land in the REAL `peer_claims` table the viewer's LOCAL
    // project-survey read returns — no network at survey time (Pillar 3 / I-GT-2).
    seed_peer_authored_graph(
        env,
        &[SeedPeer {
            peer_did: author_did,
            seed: [41u8; 32],
            triples: &[
                (project, TRAVERSAL_PHILOSOPHY_DEP_PINNING, 0.90),
                (project, "org.openlore.philosophy.reproducible-builds", 0.74),
                (project, "org.openlore.philosophy.memory-safety", 0.25),
            ],
        }],
    );
}

/// Seed a PHILOSOPHY-survey trail through the PRODUCTION federation write path: ONE
/// object (`philosophy`) embodied by several DISTINCT subjects (projects), each an
/// attributed claim by the SHARED contributor (`spanning_author_did`) — so the
/// philosophy survey lists multiple projects-that-embody edges AND the spanning
/// contributor appears ONCE under "Contributors who claimed" (the canonical
/// cross-project "aha", US-GT-003). DISTINCT subjects keep the CIDs distinct.
///
/// SCAFFOLD: true (slice-10) — DELIVER materializes it via `seed_peer_authored_
/// graph` with one `SeedPeer` for `spanning_author_did` whose triples are several
/// DISTINCT subjects on the shared `philosophy` object.
pub fn seed_philosophy_survey_trail(env: &TestEnv, philosophy: &str, spanning_author_did: &str) {
    // The spanning contributor embodies the SHARED `philosophy` object across TWO
    // DISTINCT subjects (nixpkgs 0.92, bazel 0.85), so the philosophy survey lists
    // two projects-that-embody edges AND names the ONE spanning contributor under
    // "Contributors who claimed" (deduped — appears once; the non-obvious span).
    // PRODUCTION federation write path; LOCAL only at survey time (I-GT-2).
    //
    // ONE `SeedPeer{ peer_did: spanning_author_did, triples: &[(subject_i, philosophy,
    // conf_i)] }` via `seed_peer_authored_graph` so TWO `peer_claims` rows land on the
    // SHARED `philosophy` object across DISTINCT subjects (projects) at varied
    // confidences (nixpkgs 0.92 → triangulated, bazel 0.85 → well-evidenced). DISTINCT
    // subjects keep the canonical CIDs distinct (identical triples collide into one
    // row). The single spanning contributor (`spanning_author_did`) is named ONCE under
    // "Contributors who claimed" (a `/score` link). The SYMMETRIC mirror of
    // `seed_project_survey_trail`, swapping subject↔object: one shared object embodied
    // by several distinct subjects (vs one shared subject embodying several objects).
    // Materialized through the PRODUCTION federation write path (`peer add` + `peer
    // pull`) so the rows land in the REAL `peer_claims` table the viewer's LOCAL
    // philosophy-survey read returns — no network at survey time (Pillar 3 / I-GT-2).
    seed_peer_authored_graph(
        env,
        &[SeedPeer {
            peer_did: spanning_author_did,
            seed: [42u8; 32],
            triples: &[
                (TRAVERSAL_PROJECT_NIXPKGS, philosophy, 0.92),
                (TRAVERSAL_PROJECT_BAZEL, philosophy, 0.85),
            ],
        }],
    );
}

/// Seed a trail where TWO DISTINCT authors claim the SAME (subject, object) at
/// DIFFERENT confidences, so the survey for that entity decomposes into TWO
/// separate attributed rows under their OWN author_dids — never averaged/merged
/// into one faceless consensus row (the cardinal anti-merging guarantee, WD-GT-5 /
/// I-GT-3). Returns the two author DIDs (in seeded order: own, peer) so the
/// scenario can assert both rows are present + attributed. Mirrors the slice-09
/// `seed_contributor_conflicting_authors` shape exactly, swapping the survey key.
///
/// SCAFFOLD: true (slice-10) — DELIVER materializes it via `seed_own_plus_peer_
/// graph` (the GQE-2 identical-content-two-authors fixture shape): the LOCAL user
/// (`You`) + a pulled peer both assert the same (subject, object) at different
/// confidences, landing two attributed rows on one survey group.
pub fn seed_two_author_same_edge(
    env: &TestEnv,
    subject: &str,
    object: &str,
) -> (String, String) {
    // The LOCAL user's OWN claim (via the real `claim add` verb → the local DID) AND
    // a pulled PEER claim (via the real `peer add` + `peer pull` verbs → Tobias) by
    // a SECOND, DISTINCT author both assert the SAME (subject, object) at DISTINCT
    // confidences (0.92 own + 0.70 peer). The own row lands in `claims`, the peer row
    // in `peer_claims`; the survey read returns BOTH (UNION ALL, no merge) and the
    // pure `group_*` projection decomposes the ONE group into TWO attributed
    // `EdgeRow`s under their own author_dids — never averaged/merged (anti-merging;
    // I-GT-3). NO hand-inserted store rows. Returns the two distinct author DIDs in
    // seeded order (own, peer). Mirrors slice-09's `seed_contributor_conflicting_
    // authors` shape exactly, swapping the survey key to the passed (subject, object).
    seed_own_plus_peer_graph(
        env,
        &[OwnClaim {
            subject,
            object,
            confidence: 0.92,
        }],
        &[SeedPeer {
            peer_did: TRAVERSAL_AUTHOR_TOBIAS,
            seed: [43u8; 32],
            triples: &[(subject, object, 0.70)],
        }],
    );
    (
        env.identity.author_did().to_string(),
        TRAVERSAL_AUTHOR_TOBIAS.to_string(),
    )
}

/// Seed ONE attributed claim whose SUBJECT is the HOSTILE claim-controlled URI
/// [`TRAVERSAL_INJECTION_SUBJECT`] (carrying `"`, `<`, `>`, `&`, space) on a known
/// philosophy object — so a `/philosophy` survey row (and any cross-link) must
/// render that subject's `/project` href with the value PERCENT-ENCODED (the
/// ADR-044 §security injection boundary). The claim is authored by a peer (the
/// hostile-input source is a PEER's signed claim, the attacker-influenced surface).
/// Returns the philosophy object the injection subject embodies (so the scenario
/// can query `/philosophy?object=<that>` and find the hostile subject as a row).
///
/// SCAFFOLD: true (slice-10) — DELIVER materializes it via `seed_peer_authored_
/// graph` with one `SeedPeer` whose single triple is `(INJECTION_SUBJECT,
/// dependency-pinning, 0.50)`. The subject is a valid stored string; the SECURITY
/// contract is purely on the OUTBOUND href encoding, not on rejecting the claim.
pub fn seed_injection_uri_subject(env: &TestEnv) -> String {
    // One peer claim whose SUBJECT is the hostile URI on the dependency-pinning
    // object, via the PRODUCTION federation write path. The hostile value is stored
    // verbatim (claims are not rejected for their subject text — anti-merging /
    // no-invented-edge are the only survey contracts); the SECURITY boundary is that
    // when the `/philosophy` survey for dependency-pinning renders this subject as a
    // `/project` cross-link, the href percent-ENCODES it (ADR-044). Returns the
    // object so the scenario queries `/philosophy?object=<dependency-pinning>`.
    seed_peer_authored_graph(
        env,
        &[SeedPeer {
            peer_did: TRAVERSAL_AUTHOR_RACHEL,
            seed: [7u8; 32],
            triples: &[(
                TRAVERSAL_INJECTION_SUBJECT,
                TRAVERSAL_PHILOSOPHY_DEP_PINNING,
                0.50,
            )],
        }],
    );
    TRAVERSAL_PHILOSOPHY_DEP_PINNING.to_string()
}

/// Assert a rendered traversal survey body (fragment OR full page) groups its
/// attributed edges correctly: every expected group `key` (a philosophy on
/// `/project`, a project on `/philosophy`) is rendered, each expected `author_did`
/// is attributed on an edge row, each `expected_confidence_verbatim` string is
/// present byte-for-byte (`0.90`, never `0.9`/`90%`; I-GT-5), each expected
/// `bucket_label` (the REUSED claim-domain display-only bucket) is shown, and NO
/// merged/averaged consensus row appears (anti-merging; I-GT-3). The OBSERVABLE
/// counterpart of the slice-09 `assert_score_html_breakdown_attributed_and_verbatim`
/// — scans the rendered HTML only (Mandate 8 universe = port-exposed rendered
/// surface, never an internal struct field).
///
/// SCAFFOLD: true (slice-10).
pub fn assert_traversal_html_groups_attributed_and_verbatim(
    body: &str,
    expected_group_keys: &[&str],
    expected_authors: &[&str],
    expected_confidences_verbatim: &[&str],
    expected_bucket_labels: &[&str],
) {
    // Every expected group key (a philosophy on `/project`, a project on
    // `/philosophy`) is rendered — the survey groups by the OTHER dimension.
    for key in expected_group_keys {
        assert!(
            body.contains(key),
            "I-GT-3: the traversal survey must render the group key {key:?} (the \
             OTHER-dimension traversal target); body was:\n{body}"
        );
    }
    // Every expected author DID is attributed on an edge row (per-row, non-Option
    // attribution — two authors → two rows, never merged away; I-GT-3).
    for did in expected_authors {
        assert!(
            body.contains(did),
            "I-GT-3: the traversal survey must attribute an edge to {did:?} (every \
             edge carries its author DID; anti-merging); body was:\n{body}"
        );
    }
    // Each confidence is rendered VERBATIM — the exact stored `f64` string, never a
    // truncated `0.9` or a `%`-formatted value (I-GT-5).
    for conf in expected_confidences_verbatim {
        assert!(
            body.contains(conf),
            "I-GT-5: the traversal survey must render the confidence {conf:?} verbatim \
             (never 0.9 / 90%); body was:\n{body}"
        );
    }
    // Each expected display-only bucket label (the REUSED claim-domain confidence
    // bucket) is shown on its edge — the viewer recomputes NO bucket.
    for bucket in expected_bucket_labels {
        assert!(
            body.contains(bucket),
            "I-GT-5: the traversal survey must render the display-only confidence \
             bucket {bucket:?} (REUSED claim-domain bucket); body was:\n{body}"
        );
    }
    // The survey is NEVER a faceless merged consensus — each edge is per-claim
    // (anti-merging; I-GT-3). No averaged-consensus phrasing appears.
    let lowered = body.to_ascii_lowercase();
    for banned in [
        "authors agree",
        "the network says",
        "consensus score",
        "community consensus",
        "averaged",
    ] {
        assert!(
            !lowered.contains(banned),
            "I-GT-3 (anti-merging): the traversal survey must show NO merged / \
             averaged consensus row; found {banned:?} in body:\n{body}"
        );
    }
}

/// Assert a rendered traversal survey body names every `cid` it is built from — each
/// displayed edge maps to exactly ONE signed claim (no invented edges; I-GT-4). The
/// sibling of slice-09's `assert_score_html_breakdown_names_cids`. The CIDs are read
/// from the SAME store the viewer's survey read returns (production-recomputed, never
/// hand-stamped).
///
/// SCAFFOLD: true (slice-10).
pub fn assert_traversal_html_names_cids(body: &str, expected_cids: &[String]) {
    assert!(
        !expected_cids.is_empty(),
        "I-GT-4: the cid-naming assertion needs ≥1 expected cid (the seeded survey \
         trail produced none); body was:\n{body}"
    );
    for cid in expected_cids {
        assert!(
            body.contains(cid.as_str()),
            "I-GT-4: the traversal survey must NAME the contributing claim cid {cid:?} \
             on its edge row (every edge maps to exactly one signed claim — no invented \
             edges); body was:\n{body}"
        );
    }
}

/// Assert a rendered traversal survey body lists each expected contributor DID as a
/// traversal LINK to `/score?contributor=<bare-did>` (the slice-09 terminus reused;
/// the bare-DID form, ADR-044 Q1). A spanning contributor appears ONCE (deduped).
/// The link is a render-only `<a href>` (no executable control). Scans the
/// OBSERVABLE rendered surface only.
///
/// SCAFFOLD: true (slice-10).
pub fn assert_traversal_html_contributors_link_to_score(body: &str, expected_dids: &[&str]) {
    assert!(
        !expected_dids.is_empty(),
        "GT-4: the contributors-link assertion needs ≥1 expected contributor DID (the \
         seeded two-author survey produced none); body was:\n{body}"
    );
    // The labeled distinct-contributors section MUST be present (the survey lists the
    // contributors-who-claimed, never a blank region).
    assert!(
        body.contains("Contributors who claimed"),
        "GT-4: the survey must render the labeled \"Contributors who claimed\" section; \
         body was:\n{body}"
    );
    for did in expected_dids {
        // Each contributor links to the slice-09 `/score` terminus in BARE-DID form
        // (ADR-044 Q1): the signing `#fragment` locator is dropped, then the bare DID is
        // percent-encoded into the query component (ADR-044 §security). The expected
        // href mirrors the production `encode_query_component` exactly.
        let bare = did.split('#').next().unwrap_or(did);
        let expected_href = format!("/score?contributor={}", encode_query_component_for_test(bare));
        // Render-only navigation TEXT: a plain `<a href=…>` anchor, never a button/form
        // control. The exact `<a href="<expected>">` opening tag MUST appear (so a no-JS
        // click is a full navigation that LANDS on the contributor's /score).
        let expected_anchor = format!("<a href=\"{expected_href}\">");
        assert!(
            body.contains(&expected_anchor),
            "GT-4: contributor {did:?} must render as a render-only `<a href>` traversal \
             link to its bare-DID /score terminus ({expected_anchor:?}); body was:\n{body}"
        );
    }
    // Anti-merging: the two distinct authors are NEVER folded into one aggregate
    // contributor link — each expected DID resolves to a DISTINCT bare /score href.
    let distinct_hrefs: std::collections::BTreeSet<String> = expected_dids
        .iter()
        .map(|did| {
            let bare = did.split('#').next().unwrap_or(did);
            format!("/score?contributor={}", encode_query_component_for_test(bare))
        })
        .collect();
    assert_eq!(
        distinct_hrefs.len(),
        expected_dids.len(),
        "GT-4: each contributor must resolve to a DISTINCT /score link — the survey must \
         NOT merge the authors into a single aggregate contributor; body was:\n{body}"
    );
}

/// Percent-encode a value into an href QUERY COMPONENT the SAME way the production
/// `viewer_domain::encode_query_component` does (ADR-044 §security): every byte
/// OUTSIDE the unreserved set (`A-Z a-z 0-9 - _ . ~`) becomes `%XX` (uppercase hex).
/// Lives in the test harness (NOT a viewer-domain import) so the assertion scans the
/// OBSERVABLE rendered surface against an INDEPENDENT oracle of the expected href —
/// Mandate 8 (the rendered byte sequence, not a re-call of the production fn).
fn encode_query_component_for_test(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for &byte in value.as_bytes() {
        let unreserved =
            byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~');
        if unreserved {
            out.push(byte as char);
        } else {
            out.push('%');
            out.push_str(&format!("{byte:02X}"));
        }
    }
    out
}

/// Assert a rendered traversal survey body renders each survey cross-link as a plain
/// `<a href>` to the correct route — subject → `/project?subject=`, object →
/// `/philosophy?object=` — so a no-JS click is a FULL navigation (progressive
/// enhancement; the href value MUST be present so the navigation lands). For each
/// `(raw_value, expected_route_prefix)` the rendered href is
/// `<route_prefix>?<key>=<encoded(raw_value)>` (the percent-encoded value, ADR-044).
/// The render-only `<a href>` carries no executable control.
///
/// SCAFFOLD: true (slice-10).
pub fn assert_traversal_html_crosslink_is_plain_anchor(
    body: &str,
    expected_hrefs: &[&str],
) {
    assert!(
        !expected_hrefs.is_empty(),
        "GT-9/GT-12/GT-14: the crosslink-anchor assertion needs ≥1 expected href (the \
         seeded survey rendered no traversal cross-link); body was:\n{body}"
    );
    for href in expected_hrefs {
        // The cross-link is a render-only `<a href="…">` opening tag — a plain anchor,
        // so a no-JS click is a FULL navigation that lands on the OTHER-dimension survey
        // (progressive enhancement; WD-GT-3). The exact opening tag (with the
        // percent-encoded value already baked into the expected href, ADR-044) MUST
        // appear verbatim so the navigation target is the inert, injection-safe route.
        let expected_anchor = format!("<a href=\"{href}\">");
        assert!(
            body.contains(&expected_anchor),
            "GT-9/GT-12/GT-14: the traversal cross-link must render as a plain `<a href>` \
             anchor to {href:?} ({expected_anchor:?}) — a no-JS click is a full \
             navigation; body was:\n{body}"
        );
        // The href value MUST NOT be smuggled into an executable control: it appears in
        // an `<a href>`, never as a `<button>`'s/`<form>`'s `formaction`/`action`
        // attribute. A render-only navigation edge carries NO write/submit control
        // (I-GT-1 / WD-GT-8).
        for control_attr in [
            format!("action=\"{href}\""),
            format!("formaction=\"{href}\""),
        ] {
            assert!(
                !body.contains(&control_attr),
                "GT-9/GT-12/GT-14: the traversal cross-link to {href:?} must be a plain \
                 `<a href>`, never a button/form executable control (found \
                 {control_attr:?}); body was:\n{body}"
            );
        }
    }
}

/// Assert a rendered traversal survey body carries the hostile claim-controlled
/// subject's `/project` cross-link with the value PERCENT-ENCODED — the ADR-044
/// §security injection boundary. The rendered href MUST contain the EXACT encoded
/// form ([`TRAVERSAL_INJECTION_SUBJECT_ENCODED`]) and MUST NOT contain the raw
/// hostile characters (`"><script>`, the un-encoded `&`/space) INSIDE the href
/// attribute — so a peer's hostile subject cannot break out of the attribute or
/// smuggle a second query param. The raw subject MAY still appear as escaped TEXT in
/// the row's visible label (maud auto-escapes text); the assertion is specifically
/// about the HREF being inert.
///
/// SCAFFOLD: true (slice-10).
pub fn assert_traversal_href_percent_encoded(body: &str) {
    // The hostile subject renders as a `/project` cross-link. The expected, inert
    // anchor carries the EXACT percent-encoded form (one source of truth: the
    // production `encode_query_component` must emit precisely this). Independently
    // re-derive it from the raw subject so the oracle is not a copy of the constant.
    let expected_value = encode_query_component_for_test(TRAVERSAL_INJECTION_SUBJECT);
    assert_eq!(
        expected_value, TRAVERSAL_INJECTION_SUBJECT_ENCODED,
        "test oracle drift: encode_query_component_for_test must agree with \
         TRAVERSAL_INJECTION_SUBJECT_ENCODED"
    );
    let expected_href = format!("/project?subject={expected_value}");
    let expected_anchor = format!("<a href=\"{expected_href}\">");
    assert!(
        body.contains(&expected_anchor),
        "GT-14 (ADR-044 §security): the hostile subject's `/project` cross-link must \
         render with the value PERCENT-ENCODED ({expected_anchor:?}) so every \
         reserved/unsafe byte is %XX; body was:\n{body}"
    );

    // Parse EVERY `/project?subject=` href value from the rendered HTML and assert no
    // raw reserved/unsafe byte survives inside it — the href cannot break out of the
    // attribute or smuggle a second query param. We scan the OBSERVABLE bytes between
    // `href="` and the closing `"`, not an internal struct field (Mandate 8).
    let needle = "href=\"/project?subject=";
    let mut found_href = false;
    let mut rest = body;
    while let Some(pos) = rest.find(needle) {
        found_href = true;
        let after = &rest[pos + needle.len()..];
        let end = after
            .find('"')
            .expect("a rendered href attribute must be closed by a quote");
        let href_value = &after[..end];
        // Every reserved/unsafe byte MUST be percent-encoded — none may appear raw
        // inside the href value. A raw `"` is impossible (it would have closed the
        // attribute early), a raw `<`/`>` could inject markup, a raw `&` could smuggle
        // a second query param, and a raw space/`?`/`#` could break the URL.
        for unsafe_char in ['"', '<', '>', '&', ' ', '?', '#'] {
            assert!(
                !href_value.contains(unsafe_char),
                "GT-14 (ADR-044 §security): the `/project` href value {href_value:?} \
                 leaked a RAW unsafe byte {unsafe_char:?} — it must be %XX-encoded so \
                 it cannot break out of the attribute or smuggle a query param; \
                 body was:\n{body}"
            );
        }
        rest = &after[end..];
    }
    assert!(
        found_href,
        "GT-14: the injection survey must render the hostile subject as a `/project` \
         cross-link href (none found); body was:\n{body}"
    );

    // Defense-in-depth: the hostile subject's markup payload must NEVER appear as an
    // EXECUTABLE breakout anywhere in the response — no injected `<script>` tag and no
    // attribute-closing `\"><script` sequence (the classic href breakout). The raw
    // subject MAY appear as auto-escaped visible TEXT (maud escapes `<`→`&lt;`), but
    // never as live markup.
    for breakout in ["<script>", "\"><script", "\"></a><script"] {
        assert!(
            !body.contains(breakout),
            "GT-14 (ADR-044 §security): the hostile subject must NOT inject markup — \
             found {breakout:?} in the rendered body:\n{body}"
        );
    }
}

/// Assert a rendered traversal survey body shows the guided `NoClaims` state for an
/// entity with no local claims (US-GT-002/003 Example 3 / I-GT-4): the guided "no
/// claims … in your local graph" notice naming the queried entity, a CLI next-step
/// hint (`graph query` / `scrape`), NO fabricated edge, and NO leaked stack trace.
/// The sibling of slice-09's `assert_score_html_renders_no_claims`.
///
/// SCAFFOLD: true (slice-10).
pub fn assert_traversal_html_renders_no_claims(body: &str, queried_entity: &str) {
    let lowered = body.to_ascii_lowercase();
    // The guided plain-language notice: emptiness is recognized as emptiness, naming
    // that there are no claims in the LOCAL graph (US-GT-002/003 Example 3 / I-GT-4).
    assert!(
        lowered.contains("no claims") && lowered.contains("local graph"),
        "I-GT-4: a claim-less entity must render the guided \"No claims … in your \
         local graph\" notice; body was:\n{body}"
    );
    // The notice NAMES the queried subject so the operator knows WHAT was looked up.
    assert!(
        body.contains(queried_entity),
        "I-GT-4: the guided NoClaims state must name the queried entity \
         {queried_entity:?} so the operator knows what was looked up; body was:\n{body}"
    );
    // The CLI next-step hint points the operator at the CLI (`graph query` / `scrape`)
    // rather than a dead end (NFR-VIEW-6 / I-GT-4).
    assert!(
        lowered.contains("graph query") || lowered.contains("scrape"),
        "I-GT-4: the guided NoClaims state must hint a CLI next step (graph query / \
         scrape) so emptiness points somewhere actionable; body was:\n{body}"
    );
    // No FABRICATED traversal edge: emptiness invents no edge row — no traversal
    // `<a href>` to the OTHER dimension and no cid surfaces (I-GT-4).
    for banned in ["href=\"/project", "href=\"/philosophy", "href=\"/score", "cid"] {
        assert!(
            !lowered.contains(banned),
            "I-GT-4: the guided NoClaims state must fabricate NO traversal edge \
             (found {banned:?}); body was:\n{body}"
        );
    }
    // No leaked stack trace / raw error internals — a calm guided state, never a panic
    // surface (I-GT-4 / NFR-VIEW-6).
    for banned in ["panicked", "RUST_BACKTRACE", "thread 'main'", "stack backtrace"] {
        assert!(
            !body.contains(banned),
            "I-GT-4: the guided NoClaims state must leak NO stack trace / error \
             internal (found {banned:?}); body was:\n{body}"
        );
    }
}

/// Assert a rendered traversal survey body carries NO sign / publish / follow /
/// subscribe control on any shape — the read-only-surface guarantee on the
/// traversal routes (I-GT-1 / WD-GT-3). The sibling of slice-09's
/// `assert_score_html_has_no_write_or_sign_control`. Cross-links ARE present and
/// are render-only `<a href>` navigation TEXT, never executable controls.
///
/// SCAFFOLD: true (slice-10).
pub fn assert_traversal_html_has_no_write_or_sign_control(body: &str) {
    let lowered = body.to_ascii_lowercase();
    for banned in [
        "name=\"sign\"",
        "sign claim",
        "sign & publish",
        "sign &amp; publish",
        "subscribe",
        ">follow<",
    ] {
        assert!(
            !lowered.contains(banned),
            "I-GT-1 / WD-GT-3: the traversal surface (`/project` + `/philosophy`) \
             must render NO sign / publish / follow / subscribe control (traversal is \
             a read; cross-links are render-only `<a href>` navigation TEXT; \
             signing/following stays in the CLI); found {banned:?} in body:\n{body}"
        );
    }
}

// =============================================================================
// slice-11 (viewer-counter-claim-threads) — counter-thread on `GET /claims/{cid}`
// (US-CT-002/003; ADR-046/047). The thread reuses the slice-03 counter-claim model
// (a counter is an ordinary signed claim carrying a `references[].type == counters`
// entry + a mandatory verbatim `reason`, ADR-015) — slice-11 only READS it via the
// new read-only `StoreReadPort::query_counter_claims(target_cid)` (the 2-step ADR-046
// read: indexed `claim_references`/`peer_claim_references` lookup by `referenced_cid`,
// UNION ALL attributed no-merge, + a per-row `read_artifact_at` for each counter's
// reason). Seeding drives the PRODUCTION CLI counter path: an OWN counter via
// `claim counter --reason <R> <CID>` (lands in `claims`); a PEER counter via the
// `peer add` + `peer pull` federation path (a peer who authored a `counters`-
// referencing signed claim; lands in `peer_claims`). The assert helpers scan the
// OBSERVABLE rendered HTML only (Mandate 8 universe = port-exposed rendered surface,
// never an internal struct field).
// =============================================================================

/// The well-known counter-thread author DIDs (reuse the slice-10 spanning-author DIDs
/// so the corpus never mints throwaway identities): Maria authors the operator's OWN
/// counter; Tobias is the second, DISTINCT peer author for the anti-merging fixture.
/// The countered claim is authored by Rachel (a pulled peer).
pub const COUNTER_TARGET_AUTHOR_RACHEL: &str = "did:plc:rachel-test";
pub const COUNTER_AUTHOR_TOBIAS: &str = "did:plc:tobias-test";

/// The verbatim free-text reason the OWN counter is authored with (the verbatim-render
/// contract, CT-5): one source of truth so the seed + the assertion never drift. The
/// punctuation (`;` and `,`) is load-bearing — it must render byte-for-byte (NFC-
/// normalized at author time by the slice-03 `claim counter` verb, ADR-015 / WD-35).
pub const COUNTER_REASON_VERBATIM: &str =
    "Cargo's dependency pinning is opt-in, not philosophical; pinning is a tool, not a value.";

/// The verbatim free-text reason the PEER (Tobias) counter is authored with — DISTINCT
/// from [`COUNTER_REASON_VERBATIM`] so the anti-merging fixture (CT-4) proves the two
/// counters render as two attributed items with their OWN reasons, never collapsed into
/// one merged "disputed by 2" row. One source of truth so the seed + assertion never
/// drift; the punctuation is load-bearing (renders byte-for-byte, ADR-015 / WD-35).
pub const COUNTER_PEER_REASON_VERBATIM: &str =
    "Reproducibility is a different axis; pinning serves builds, not philosophy.";

/// The `#claim-detail` swap-target id the `/claims/{cid}` detail fragment renders under
/// (the slice-06/07 detail region the slice-11 counter-thread is rendered INSIDE — the
/// page EMBEDS this fragment, so the htmx fragment and the no-JS full page are
/// byte-identical in the swap region, I-CT-6). Asserting it appears in BOTH the fragment
/// and the full-page detail region proves the parity-by-construction embedding. One
/// source of truth so the scenarios never drift.
pub const CLAIM_DETAIL_REGION_ID: &str = "claim-detail";

/// The handle a counter-thread seed returns: the target claim's CID (the
/// `GET /claims/{target_cid}` path + the `query_counter_claims(target_cid)` arg) and
/// each seeded counter's OWN CID (the `<a href="/claims/{counter_cid}">` drill-link
/// target + the per-item attribution). Captured so the scenario addresses the exact
/// records without hard-coding a CID (mirrors `seed_own_claim_with_evidence`'s returned
/// CID + the slice-09 `read_peer_claim_cids_for` shape).
#[derive(Debug, Clone)]
pub struct SeededCounterThread {
    /// The countered claim's CID — the `/claims/{cid}` path + `query_counter_claims`
    /// target.
    pub target_cid: String,
    /// The countered claim's stored confidence, rendered VERBATIM + UNCHANGED by the
    /// counter (the shown-never-applied contract; e.g. `0.91`).
    pub target_confidence: f64,
    /// One row per seeded counter: `(author_did, counter_cid, reason)`. `reason` is
    /// `None` for the empty-reason edge (CT-6). Preserves seeded order (the adapter's
    /// deterministic `composed_at, source_table, cid` order).
    pub counters: Vec<SeededCounter>,
}

/// One seeded counter targeting the thread's claim (own or peer).
#[derive(Debug, Clone)]
pub struct SeededCounter {
    /// The counter author's DID (Maria for own; a peer DID for peer counters).
    pub author_did: String,
    /// The counter's OWN content-addressed CID — the `/claims/{cid}` drill-link target.
    pub cid: String,
    /// The verbatim reason; `None`/empty for the ADR-015 wire-optional empty-reason edge
    /// → rendered as "no reason provided".
    pub reason: Option<String>,
}

/// Seed a countered claim through the PRODUCTION write paths: Rachel's claim (a pulled
/// peer claim at confidence 0.91) countered by the operator's OWN counter authored via
/// the slice-03 `claim counter --reason <COUNTER_REASON_VERBATIM> <target_cid>` verb
/// (Pillar 3 / BR-VIEW-4 — the SAME store `openlore ui` reads). The own counter lands
/// in `claims` carrying a `references[].type == counters` entry whose `cid == target`
/// (ADR-015) + the verbatim reason in its on-disk artifact. Returns the
/// [`SeededCounterThread`] so the scenario addresses the exact records (the
/// `/claims/{target_cid}` path + the counter's `/claims/{counter_cid}` drill-link).
///
/// SCAFFOLD: true (slice-11) — DELIVER materializes it via the EXISTING federation +
/// counter seams: `seed_peer_authored_graph` (one `SeedPeer` for Rachel, ONE triple at
/// 0.91 → the target lands in `peer_claims`), recover the target CID (the production-
/// recomputed peer-claim CID, e.g. via `read_peer_claim_cids_for(env, RACHEL)`), then
/// drive `run_openlore_with_peer_resolver_stdin(env, ["claim","counter",&target_cid,
/// "--reason",COUNTER_REASON_VERBATIM], …, "\nN\n")` so the OWN counter is signed +
/// persisted locally (decline publish — the read path needs only the local row).
/// Recover the counter CID from stdout (`signed_cid_from_stdout` / the slice-03
/// `parse_counter_claim_cid`). NO hand-inserted store rows.
pub fn seed_claim_with_counter(env: &TestEnv) -> SeededCounterThread {
    // STEP 1 — seed Rachel's claim (confidence 0.91) as a PULLED PEER claim via the
    // production `peer add` + `peer pull` federation path (it lands in `peer_claims`).
    // ONE triple at 0.91 so the target shape matches `seed_uncountered_claim` (the
    // shown-never-applied byte-diff baseline). Rachel's seed `[7u8; 32]` mirrors the
    // slice-03 counter_claim.rs convention.
    let rachel_seed = [7u8; 32];
    let _graph = seed_peer_authored_graph(
        env,
        &[SeedPeer {
            peer_did: COUNTER_TARGET_AUTHOR_RACHEL,
            seed: rachel_seed,
            triples: &[(
                "github:rust-lang/cargo",
                "org.openlore.philosophy.dependency-pinning",
                0.91,
            )],
        }],
    );

    // Recover the production-recomputed target CID from the real `peer_claims` table
    // (the pull pipeline verified + content-addressed it — no hand-inserted row).
    let target_cids = read_peer_claim_cids_for(env, COUNTER_TARGET_AUTHOR_RACHEL);
    assert_eq!(
        target_cids.len(),
        1,
        "seed_claim_with_counter: expected exactly ONE pulled Rachel claim to counter; \
         got {target_cids:?}"
    );
    let target_cid = target_cids.into_iter().next().expect("one target CID");

    // STEP 2 — author the operator's OWN counter against the target via the slice-03
    // `claim counter --reason <R> <CID>` verb (it lands in the user's OWN `claims`
    // table carrying references[].type == counters + the verbatim reason in its
    // on-disk artifact). The verb resolves the target from the LOCAL stores (own
    // `claims` then the peer cache) — NO network resolver needed. Confirm the sign
    // prompt, DECLINE publish ("\nN\n") — the read path needs only the LOCAL row.
    let outcome = run_openlore_with_stdin(
        env,
        &[
            "claim",
            "counter",
            &target_cid,
            "--reason",
            COUNTER_REASON_VERBATIM,
        ],
        "\nN\n",
    );
    assert_eq!(
        outcome.status, 0,
        "seed_claim_with_counter: `claim counter` must exit 0;\n--- stdout ---\n{}\n\
         --- stderr ---\n{}",
        outcome.stdout, outcome.stderr
    );

    // Recover the OWN counter's content-addressed CID from the verb's
    // `Computing claim CID <cid>` line (the same marker `claim add` prints; the CID is
    // recoverable even when publish is declined).
    let counter_cid = signed_cid_from_stdout(&outcome.stdout);

    SeededCounterThread {
        target_cid,
        target_confidence: 0.91,
        counters: vec![SeededCounter {
            author_did: env.identity.author_did().to_string(),
            cid: counter_cid,
            reason: Some(COUNTER_REASON_VERBATIM.to_string()),
        }],
    }
}

/// Seed a claim countered by TWO DISTINCT authors through the PRODUCTION CLI counter
/// path — the anti-merging fixture (CT-4 / I-CT-3): Rachel's claim (0.91) countered by
/// (a) the operator's OWN counter via `claim counter` (→ `claims`) AND (b) peer Tobias's
/// counter via the `peer add` + `peer pull` federation path (Tobias authored a signed
/// claim carrying `references[].type == counters` whose `cid == target` + his own
/// reason → `peer_claims`). The two counters MUST render as two attributed items under
/// their own author DIDs + CIDs — never a merged "disputed by 2" aggregate. Returns the
/// [`SeededCounterThread`] with BOTH counters in deterministic order.
///
/// SCAFFOLD: true (slice-11) — DELIVER materializes it by (1) seeding Rachel's target
/// claim + recovering its CID (as `seed_claim_with_counter`); (2) authoring Maria's OWN
/// counter via `claim counter`; (3) building a verifiable PEER record for Tobias that
/// carries a `references: [{ type: "counters", cid: target_cid }]` entry + a reason
/// (extend `build_verifiable_peer_records_for_triples` with a references+reason variant,
/// e.g. `build_verifiable_peer_counter_record(tobias_did, seed, target_cid, reason)`),
/// `peer add` + `peer pull` it so the peer counter lands in `peer_claims`; (4) recover
/// both counter CIDs. NO hand-inserted store rows; the peer-counter shape is verified +
/// CID-recomputed by the production pull pipeline.
pub fn seed_claim_two_counters_distinct_authors(env: &TestEnv) -> SeededCounterThread {
    // STEP 1 — build BOTH peers' verifiable wire records UP FRONT, holding each
    // `PeerPds` ALIVE for the whole function (so a SINGLE `peer pull` over BOTH peers
    // succeeds — pulling one peer at a time would leave the other's now-dropped PDS
    // unreachable and fail the pull). Rachel hosts the TARGET claim (0.91); Tobias
    // hosts a COUNTER referencing the target. Rachel's target CID is DETERMINISTIC
    // (the pull pipeline recomputes the SAME CID the builder computes), so Tobias's
    // counter can reference it before either is pulled.
    let rachel_seed = [7u8; 32];
    let (rachel_records, rachel_pubkey_hex) = build_verifiable_peer_records_for_triples(
        COUNTER_TARGET_AUTHOR_RACHEL,
        rachel_seed,
        &[(
            "github:rust-lang/cargo",
            "org.openlore.philosophy.dependency-pinning",
            0.91,
        )],
    );
    let target_cid = rachel_records
        .first()
        .expect("Rachel's target record")
        .rkey
        .clone();

    let tobias_seed = [9u8; 32];
    let (tobias_record, tobias_pubkey_hex) = build_verifiable_peer_counter_record(
        COUNTER_AUTHOR_TOBIAS,
        tobias_seed,
        &target_cid,
        Some(COUNTER_PEER_REASON_VERBATIM),
    );

    let rachel_pds = PeerPds::for_peer(COUNTER_TARGET_AUTHOR_RACHEL, rachel_records);
    let tobias_pds = PeerPds::for_peer(COUNTER_AUTHOR_TOBIAS, vec![tobias_record]);

    // STEP 2 — subscribe to BOTH peers via the real `peer add` verb (resolver wired
    // per peer), then `peer pull` BOTH in ONE invocation while both PDS are alive.
    // The production pull pipeline verifies each record + recomputes its CID + writes
    // peer_claims (+ peer_claim_references for Tobias's `counters` reference).
    for (did, pds) in [
        (COUNTER_TARGET_AUTHOR_RACHEL, &rachel_pds),
        (COUNTER_AUTHOR_TOBIAS, &tobias_pds),
    ] {
        let added = run_openlore_with_peer_resolver(
            env,
            &["peer", "add", did],
            did,
            pds.endpoint_url(),
        );
        assert_eq!(
            added.status, 0,
            "seed_claim_two_counters_distinct_authors: peer add for {did} must succeed;\n\
             --- stdout ---\n{}\n--- stderr ---\n{}",
            added.stdout, added.stderr
        );
    }
    let pulled = run_openlore_pull_multi(
        env,
        &["peer", "pull"],
        &[
            PeerSeam {
                peer_did: COUNTER_TARGET_AUTHOR_RACHEL,
                peer_endpoint: rachel_pds.endpoint_url(),
                peer_pubkey_hex: &rachel_pubkey_hex,
            },
            PeerSeam {
                peer_did: COUNTER_AUTHOR_TOBIAS,
                peer_endpoint: tobias_pds.endpoint_url(),
                peer_pubkey_hex: &tobias_pubkey_hex,
            },
        ],
    );
    assert_eq!(
        pulled.status, 0,
        "seed_claim_two_counters_distinct_authors: peer pull must succeed;\n\
         --- stdout ---\n{}\n--- stderr ---\n{}",
        pulled.stdout, pulled.stderr
    );

    // Confirm the pull recomputed Rachel's target CID to the SAME value the builder
    // computed (so the local target_cid the own counter targets is the stored one).
    let target_cids = read_peer_claim_cids_for(env, COUNTER_TARGET_AUTHOR_RACHEL);
    assert_eq!(
        target_cids, vec![target_cid.clone()],
        "seed_claim_two_counters_distinct_authors: the pulled Rachel target CID must \
         match the deterministically computed one; got {target_cids:?}"
    );

    // STEP 3 — author the operator's OWN counter against the target via the slice-03
    // `claim counter --reason <R> <CID>` verb (→ the user's OWN `claims` table,
    // carrying references[].type == counters + the verbatim reason). Confirm sign,
    // DECLINE publish ("\nN\n") — the read path needs only the LOCAL row.
    let own_outcome = run_openlore_with_stdin(
        env,
        &[
            "claim",
            "counter",
            &target_cid,
            "--reason",
            COUNTER_REASON_VERBATIM,
        ],
        "\nN\n",
    );
    assert_eq!(
        own_outcome.status, 0,
        "seed_claim_two_counters_distinct_authors: `claim counter` (own) must exit 0;\n\
         --- stdout ---\n{}\n--- stderr ---\n{}",
        own_outcome.stdout, own_outcome.stderr
    );
    let own_counter_cid = signed_cid_from_stdout(&own_outcome.stdout);

    // STEP 4 — recover Tobias's production-recomputed counter CID from `peer_claims`
    // (verified + content-addressed by the pull pipeline — no hand-inserted row).
    let tobias_cids = read_peer_claim_cids_for(env, COUNTER_AUTHOR_TOBIAS);
    assert_eq!(
        tobias_cids.len(),
        1,
        "seed_claim_two_counters_distinct_authors: expected exactly ONE pulled Tobias \
         counter; got {tobias_cids:?}"
    );
    let peer_counter_cid = tobias_cids.into_iter().next().expect("one Tobias counter CID");

    // Return BOTH counters in deterministic order (own first, then peer) — each
    // attributed to its OWN author DID + CID (anti-merging by construction).
    SeededCounterThread {
        target_cid,
        target_confidence: 0.91,
        counters: vec![
            SeededCounter {
                author_did: env.identity.author_did().to_string(),
                cid: own_counter_cid,
                reason: Some(COUNTER_REASON_VERBATIM.to_string()),
            },
            SeededCounter {
                author_did: COUNTER_AUTHOR_TOBIAS.to_string(),
                cid: peer_counter_cid,
                reason: Some(COUNTER_PEER_REASON_VERBATIM.to_string()),
            },
        ],
    }
}

/// Seed a claim countered by a PEER record whose `reason` is ABSENT/empty — the ADR-015
/// wire-optional asymmetry edge (CT-6 / ADR-047): a non-OpenLore client may author a
/// counter with no reason. The viewer must render the counter's author DID + CID with
/// an explicit "no reason provided" state — never a blank line, never a crash. Returns
/// the [`SeededCounterThread`] whose single counter has `reason == None`.
///
/// SCAFFOLD: true (slice-11) — DELIVER materializes it by seeding Rachel's target claim
/// + a verifiable PEER counter record (via the references+reason builder) whose
/// `unsigned.reason` is OMITTED (`None`), `peer add` + `peer pull`ed so it lands in
/// `peer_claims`. NO hand-inserted store rows.
pub fn seed_counter_empty_reason(env: &TestEnv) -> SeededCounterThread {
    // STEP 1 — build BOTH peers' verifiable wire records UP FRONT, holding each
    // `PeerPds` ALIVE for the whole function so a SINGLE `peer pull` over BOTH peers
    // succeeds. Rachel hosts the TARGET claim (0.91); Tobias hosts a COUNTER
    // referencing the target whose `reason` is OMITTED (`None`) — the ADR-015
    // wire-optional empty-reason edge. Rachel's target CID is DETERMINISTIC (the pull
    // pipeline recomputes the SAME CID the builder computes), so Tobias's counter can
    // reference it before either is pulled. Mirrors
    // `seed_claim_two_counters_distinct_authors`, minus the operator's OWN counter and
    // with `reason: None`.
    let rachel_seed = [7u8; 32];
    let (rachel_records, rachel_pubkey_hex) = build_verifiable_peer_records_for_triples(
        COUNTER_TARGET_AUTHOR_RACHEL,
        rachel_seed,
        &[(
            "github:rust-lang/cargo",
            "org.openlore.philosophy.dependency-pinning",
            0.91,
        )],
    );
    let target_cid = rachel_records
        .first()
        .expect("Rachel's target record")
        .rkey
        .clone();

    // The empty-reason counter: `build_verifiable_peer_counter_record` with `reason:
    // None` OMITS the wire `reason` field entirely (the production reason-absent form).
    let tobias_seed = [9u8; 32];
    let (tobias_record, tobias_pubkey_hex) =
        build_verifiable_peer_counter_record(COUNTER_AUTHOR_TOBIAS, tobias_seed, &target_cid, None);

    let rachel_pds = PeerPds::for_peer(COUNTER_TARGET_AUTHOR_RACHEL, rachel_records);
    let tobias_pds = PeerPds::for_peer(COUNTER_AUTHOR_TOBIAS, vec![tobias_record]);

    // STEP 2 — subscribe to BOTH peers via the real `peer add` verb, then `peer pull`
    // BOTH in ONE invocation while both PDS are alive. The production pull pipeline
    // verifies each record + recomputes its CID + writes peer_claims (+
    // peer_claim_references for Tobias's `counters` reference).
    for (did, pds) in [
        (COUNTER_TARGET_AUTHOR_RACHEL, &rachel_pds),
        (COUNTER_AUTHOR_TOBIAS, &tobias_pds),
    ] {
        let added =
            run_openlore_with_peer_resolver(env, &["peer", "add", did], did, pds.endpoint_url());
        assert_eq!(
            added.status, 0,
            "seed_counter_empty_reason: peer add for {did} must succeed;\n\
             --- stdout ---\n{}\n--- stderr ---\n{}",
            added.stdout, added.stderr
        );
    }
    let pulled = run_openlore_pull_multi(
        env,
        &["peer", "pull"],
        &[
            PeerSeam {
                peer_did: COUNTER_TARGET_AUTHOR_RACHEL,
                peer_endpoint: rachel_pds.endpoint_url(),
                peer_pubkey_hex: &rachel_pubkey_hex,
            },
            PeerSeam {
                peer_did: COUNTER_AUTHOR_TOBIAS,
                peer_endpoint: tobias_pds.endpoint_url(),
                peer_pubkey_hex: &tobias_pubkey_hex,
            },
        ],
    );
    assert_eq!(
        pulled.status, 0,
        "seed_counter_empty_reason: peer pull must succeed;\n\
         --- stdout ---\n{}\n--- stderr ---\n{}",
        pulled.stdout, pulled.stderr
    );

    // Confirm the pull recomputed Rachel's target CID to the SAME value the builder
    // computed.
    let target_cids = read_peer_claim_cids_for(env, COUNTER_TARGET_AUTHOR_RACHEL);
    assert_eq!(
        target_cids,
        vec![target_cid.clone()],
        "seed_counter_empty_reason: the pulled Rachel target CID must match the \
         deterministically computed one; got {target_cids:?}"
    );

    // Recover Tobias's production-recomputed empty-reason counter CID from `peer_claims`
    // (verified + content-addressed by the pull pipeline — no hand-inserted row).
    let tobias_cids = read_peer_claim_cids_for(env, COUNTER_AUTHOR_TOBIAS);
    assert_eq!(
        tobias_cids.len(),
        1,
        "seed_counter_empty_reason: expected exactly ONE pulled Tobias counter; \
         got {tobias_cids:?}"
    );
    let peer_counter_cid = tobias_cids
        .into_iter()
        .next()
        .expect("one Tobias counter CID");

    // The single counter carries `reason == None` — the empty-reason edge (ADR-047).
    SeededCounterThread {
        target_cid,
        target_confidence: 0.91,
        counters: vec![SeededCounter {
            author_did: COUNTER_AUTHOR_TOBIAS.to_string(),
            cid: peer_counter_cid,
            reason: None,
        }],
    }
}

/// Seed an UN-countered claim (CT-7 no-noise; CT-3 / CT-INV-ShownNeverApplied baseline):
/// the SAME claim shape `seed_claim_with_counter` targets (Rachel's claim at confidence
/// 0.91) but with NOTHING countering it, so `query_counter_claims` returns an empty vec
/// → `CounterThread::None` → the detail renders the claim ALONE (no section, no flag, no
/// "0 counters" noise). Returns the target CID so the scenario opens `/claims/{cid}`.
/// Used as the byte-identical baseline the shown-never-applied gold diffs against.
///
/// SCAFFOLD: true (slice-11) — DELIVER materializes it via `seed_peer_authored_graph`
/// (one `SeedPeer` for Rachel, ONE triple at 0.91 — the SAME target shape as
/// `seed_claim_with_counter`, minus any counter) and returns the recovered target CID.
/// The IDENTICAL claim shape is what makes the shown-never-applied byte-diff meaningful.
pub fn seed_uncountered_claim(env: &TestEnv) -> String {
    // Seed Rachel's claim (confidence 0.91) as a PULLED PEER claim via the production
    // `peer add` + `peer pull` federation path — the EXACT step 1 of
    // `seed_claim_with_counter`, minus any counter. ONE triple at 0.91 so the target
    // shape is byte-identical to the countered baseline (what makes the
    // shown-never-applied diff meaningful, CT-3). Rachel's seed `[7u8; 32]` mirrors the
    // slice-03 counter_claim.rs convention.
    let rachel_seed = [7u8; 32];
    let _graph = seed_peer_authored_graph(
        env,
        &[SeedPeer {
            peer_did: COUNTER_TARGET_AUTHOR_RACHEL,
            seed: rachel_seed,
            triples: &[(
                "github:rust-lang/cargo",
                "org.openlore.philosophy.dependency-pinning",
                0.91,
            )],
        }],
    );

    // Recover the production-recomputed target CID from the real `peer_claims` table
    // (the pull pipeline verified + content-addressed it — no hand-inserted row). With
    // NOTHING countering it, `query_counter_claims` returns empty → CounterThread::None.
    let target_cids = read_peer_claim_cids_for(env, COUNTER_TARGET_AUTHOR_RACHEL);
    assert_eq!(
        target_cids.len(),
        1,
        "seed_uncountered_claim: expected exactly ONE pulled Rachel claim; \
         got {target_cids:?}"
    );
    target_cids.into_iter().next().expect("one target CID")
}

/// Locate and DELETE an OWN counter's on-disk `SignedClaim` artifact (review D2):
/// the operator's own counter (authored via `claim counter`) persists its artifact at
/// `{claims_dir}/{counter_cid}.json` (the absolute own-claim path the slice-06 read
/// resolves). Deleting it simulates a counter whose artifact is missing/unreadable
/// locally — a plausible real scenario (e.g. a pulled peer counter whose artifact was
/// never fetched) — WITHOUT touching the authoritative DB ref row. The counter STILL
/// has its `author_did` + `cid` in the DB; only the artifact (the free-text `reason`
/// source) is gone. Asserts the artifact existed first (so the test fails loudly if the
/// seed shape changes) and that the delete succeeds.
pub fn delete_counter_artifact(env: &TestEnv, counter_cid: &str) {
    let artifact_path = env.claims_dir().join(format!("{counter_cid}.json"));
    assert!(
        artifact_path.exists(),
        "delete_counter_artifact: the own counter's artifact must exist before deletion \
         (the seed persists it at {}); the test's premise depends on it",
        artifact_path.display()
    );
    std::fs::remove_file(&artifact_path).unwrap_or_else(|e| {
        panic!(
            "delete_counter_artifact: failed to delete the counter artifact {}: {e}",
            artifact_path.display()
        )
    });
    assert!(
        !artifact_path.exists(),
        "delete_counter_artifact: the counter artifact {} must be gone after deletion",
        artifact_path.display()
    );
}

/// Assert a rendered claim-detail body (fragment OR full page) renders the
/// counter-thread correctly: every expected counter's `author_did` is attributed, its
/// OWN `cid` is shown, and its verbatim `reason` text appears byte-for-byte — and the
/// original claim's `expected_confidence_verbatim` renders unchanged (`0.91`, never
/// `0.9`/`91%`; I-CT-4). The OBSERVABLE counterpart of the slice-08
/// `assert_search_html_counter_shown_not_applied` + the slice-10 traversal asserts;
/// scans the rendered HTML only (Mandate 8 universe = port-exposed rendered surface).
///
/// SCAFFOLD: true (slice-11) — DELIVER asserts: each expected (author_did, cid, reason)
/// triple is present in the body (per-counter attribution + drill-target + verbatim
/// reason, I-CT-3); the `expected_confidence_verbatim` string is present byte-for-byte
/// (the countered claim's confidence, UNCHANGED — I-CT-2/I-CT-4); the body carries the
/// `#claim-detail` region id (I-CT-6).
pub fn assert_counter_thread_renders_attributed_verbatim(
    body: &str,
    expected_counters: &[SeededCounter],
    expected_confidence_verbatim: &str,
) {
    assert!(
        !expected_counters.is_empty(),
        "I-CT-3: the attributed-thread assertion needs ≥1 expected counter; body was:\n{body}"
    );
    // The body carries the `#claim-detail` swap-target region (I-CT-6 — the page
    // EMBEDS the same fragment fn, so this id appears in BOTH shapes).
    assert!(
        body.contains(CLAIM_DETAIL_REGION_ID),
        "I-CT-6: the detail body must carry the #{CLAIM_DETAIL_REGION_ID} region id; \
         body was:\n{body}"
    );
    // The original claim's confidence renders VERBATIM + UNCHANGED by the counter
    // (shown-never-applied; never `0.9`/`91%`; I-CT-2 / I-CT-4).
    assert!(
        body.contains(expected_confidence_verbatim),
        "I-CT-2/I-CT-4: the countered claim's confidence must render verbatim + \
         unchanged ({expected_confidence_verbatim:?}); body was:\n{body}"
    );
    // Per-counter attribution: every expected counter's author DID + its own CID
    // (the drill-link target) + its verbatim reason render byte-for-byte (I-CT-3).
    for counter in expected_counters {
        assert!(
            body.contains(&counter.author_did),
            "I-CT-3: the counter-thread must name the counter author DID \
             {:?}; body was:\n{body}",
            counter.author_did
        );
        assert!(
            body.contains(&counter.cid),
            "I-CT-3: the counter-thread must show the counter's own CID {:?} (the \
             drill-link target); body was:\n{body}",
            counter.cid
        );
        if let Some(reason) = &counter.reason {
            assert!(
                body.contains(reason.as_str()),
                "I-CT-3: the counter-thread must render the verbatim reason {reason:?} \
                 byte-for-byte; body was:\n{body}"
            );
        }
    }
}

/// Assert a rendered claim-detail body renders EXACTLY two attributed counter entries
/// (one per distinct author + cid, each with its verbatim reason) and NO merged
/// "disputed by N" / consensus aggregate row — the anti-merging gold (CT-4 / I-CT-3 /
/// KPI-AV-2). The OBSERVABLE counterpart of the slice-08
/// `assert_search_html_has_no_merged_consensus_row` + the slice-09/10 anti-merging
/// asserts; scans the rendered HTML only (Mandate 8 universe = port-exposed rendered
/// surface).
///
/// SCAFFOLD: true (slice-11) — DELIVER asserts: BOTH expected (author_did, cid) pairs
/// are present (two attributed items, each with its verbatim reason); NO merged /
/// faceless consensus phrasing appears ("disputed by", "disputed by 2", "consensus",
/// "net verdict", "N people disagree", "X people disagree") — the thread is per-counter,
/// never an aggregate (I-CT-3).
pub fn assert_counter_thread_two_attributed_no_merge(
    body: &str,
    expected_counters: &[SeededCounter],
) {
    assert_eq!(
        expected_counters.len(),
        2,
        "CT-4 anti-merging needs EXACTLY two distinct-author counters; got {} \
         (body was:\n{body})",
        expected_counters.len()
    );

    // Two DISTINCT authors and two DISTINCT CIDs — the precondition that makes the
    // anti-merging assertion meaningful (each is its own (author, cid) identity).
    assert_ne!(
        expected_counters[0].author_did, expected_counters[1].author_did,
        "CT-4: the two counters must be by DISTINCT authors; both were {:?}",
        expected_counters[0].author_did
    );
    assert_ne!(
        expected_counters[0].cid, expected_counters[1].cid,
        "CT-4: the two counters must carry DISTINCT CIDs; both were {:?}",
        expected_counters[0].cid
    );

    // BOTH expected (author_did, cid) pairs render as two attributed items, each with
    // its OWN verbatim reason (per-counter attribution — anti-merging, I-CT-3).
    for counter in expected_counters {
        assert!(
            body.contains(&counter.author_did),
            "CT-4: each counter must render under its OWN author DID {:?}; body was:\n{body}",
            counter.author_did
        );
        assert!(
            body.contains(&counter.cid),
            "CT-4: each counter must render its OWN CID {:?} (the drill-link target); \
             body was:\n{body}",
            counter.cid
        );
        if let Some(reason) = &counter.reason {
            assert!(
                body.contains(reason.as_str()),
                "CT-4: each counter must render its OWN verbatim reason {reason:?} \
                 byte-for-byte; body was:\n{body}"
            );
        }
    }

    // NO merged / faceless consensus aggregate row: the thread is per-counter, never a
    // "disputed by N" / consensus / net-verdict collapse (anti-merging, I-CT-3 /
    // KPI-AV-2). Case-insensitive so a capitalized variant can never sneak through.
    let lowered = body.to_ascii_lowercase();
    for merged in [
        "disputed by",
        "disputed by 2",
        "consensus",
        "net verdict",
        "people disagree",
    ] {
        assert!(
            !lowered.contains(merged),
            "CT-4: the thread must NEVER emit a merged {merged:?} aggregate (two distinct \
             (author, cid) → two items, never one); body was:\n{body}"
        );
    }
}

/// Assert a rendered claim-detail body shows the empty-reason counter state correctly
/// (CT-6 / ADR-047): the counter entry STILL shows its author DID + its CID, AND shows
/// an explicit "no reason provided" state — never a blank line, never a crash. Scans the
/// rendered HTML only (Mandate 8 universe = port-exposed rendered surface).
///
/// SCAFFOLD: true (slice-11) — DELIVER asserts: the empty-reason counter's `author_did`
/// + `cid` are present (attribution never elided), AND the body carries the explicit
/// "no reason provided" state for that entry (ADR-047 — total at the type level via
/// `reason: Option<String>`).
pub fn assert_counter_thread_empty_reason_state(body: &str, expected_counter: &SeededCounter) {
    // Attribution is NEVER elided by an absent reason: the author DID + the counter's
    // own CID are STILL shown (anti-merging; I-CT-3).
    assert!(
        body.contains(&expected_counter.author_did),
        "CT-6: the empty-reason counter's author DID {:?} must still be shown;\n\
         --- body ---\n{body}",
        expected_counter.author_did
    );
    assert!(
        body.contains(&expected_counter.cid),
        "CT-6: the empty-reason counter's CID {:?} must still be shown;\n\
         --- body ---\n{body}",
        expected_counter.cid
    );

    // The absent reason renders the EXPLICIT "no reason provided" state — never a blank
    // line, never a crash (ADR-047; total at the type level via reason: Option<String>).
    assert!(
        body.contains("no reason provided"),
        "CT-6: a counter with no reason must render the explicit 'no reason provided' \
         state (never a blank line);\n--- body ---\n{body}"
    );
}

/// Assert a rendered claim-detail body shows a NEUTRAL "Countered" presence flag (CT-8 /
/// I-CT-3): a presence marker only — never a verdict, a score, or a count-based re-rank
/// ("disputed by N"). Scans the rendered HTML only (Mandate 8 universe = port-exposed
/// rendered surface).
///
/// SCAFFOLD: true (slice-11) — DELIVER asserts: the body carries the neutral "Countered"
/// presence flag, AND NO verdict / score / count-based phrasing ("disputed by N",
/// "consensus", "net verdict", "X people disagree") appears alongside it (the flag is a
/// presence marker, never a weight/verdict — I-CT-3).
pub fn assert_counter_thread_presence_flag_is_neutral(body: &str) {
    // The countered claim carries the neutral "Countered" PRESENCE flag — disagreement
    // is made legible WITHOUT picking a winner (CT-8 / I-CT-3).
    assert!(
        body.contains("Countered"),
        "CT-8: a countered claim must carry the neutral 'Countered' presence flag;\n\
         --- body ---\n{body}"
    );

    // The flag is PRESENCE-ONLY: it is NEVER a verdict, a score, or a count-based
    // re-rank. NONE of these verdict / count-judgement phrasings may appear. Lowercased
    // so a capitalized variant ("Disputed", "Refuted") can never sneak through. The
    // verdict words ("disputed", "refuted", "false", "wrong") are checked as whole words
    // to avoid false positives (e.g. "falsehood" is not "false" the verdict — but the
    // neutral flag never emits any of these regardless).
    let lowered = body.to_ascii_lowercase();
    for verdict in [
        // count-based / merged-judgement phrasing (never a count aggregated to a verdict)
        "disputed by",
        "consensus",
        "net verdict",
        "people disagree",
        // verdict words — the flag asserts presence, never correctness of the counter
        "disputed",
        "refuted",
        "is false",
        "is wrong",
    ] {
        assert!(
            !lowered.contains(verdict),
            "CT-8: the 'Countered' flag is presence-only — it must NEVER emit a verdict / \
             score / count-based phrasing ({verdict:?}); it asserts the claim HAS \
             disagreement, never that the counter is correct (I-CT-3);\n--- body ---\n{body}"
        );
    }
}

/// Assert the no-noise discipline (CT-7 / I-CT-2): an UN-countered claim's detail body
/// carries NO "Counter-claims" section, NO "Countered" presence flag, and NO "0
/// counters" / "no disagreement" empty-state noise — `CounterThread::None` renders
/// nothing extra. Scans the rendered HTML only.
///
/// SCAFFOLD: true (slice-11) — DELIVER asserts the body contains NONE of: "Counter-
/// claims", "Countered", "0 counters", "no disagreement", "no counter" — the
/// un-countered claim renders exactly as slice-06 (byte-unaffected for the common case).
pub fn assert_no_counter_thread_noise(body: &str) {
    // An un-countered claim (CounterThread::None) renders NOTHING extra: no
    // "Counter-claims" section heading, no "Countered" presence flag, and no
    // empty-state "0 counters" / "no disagreement" noise (slice-06 parity; I-CT-2).
    for noise in [
        "Counter-claims",
        "Countered",
        "0 counters",
        "no disagreement",
    ] {
        assert!(
            !body.contains(noise),
            "I-CT-2: an un-countered claim must render NO {noise:?} noise \
             (CounterThread::None renders nothing extra);\n--- body ---\n{body}"
        );
    }
}

/// Assert NO detail response shape renders a write / sign / counter / publish control
/// (CT-INV-NoWrite / I-CT-1): authoring stays the slice-03 CLI; the counter CID
/// drill-links are render-only `<a href>` navigation TEXT. The slice-11 sibling of
/// `assert_traversal_html_has_no_write_or_sign_control`. Scans the rendered HTML only.
///
/// SCAFFOLD: true (slice-11) — DELIVER asserts the body carries NONE of the write/sign/
/// counter affordances (`name="sign"`, "sign claim", "sign & publish", "subscribe",
/// ">follow<", "counter this", "add counter", a `<form`/`<button` wrapping a write
/// action) — the viewer renders counters, never offers to author one.
pub fn assert_detail_html_has_no_write_or_sign_control(body: &str) {
    let lowered = body.to_ascii_lowercase();
    for banned in [
        "name=\"sign\"",
        "sign claim",
        "sign & publish",
        "sign &amp; publish",
        "subscribe",
        ">follow<",
        "counter this",
        "add counter",
    ] {
        assert!(
            !lowered.contains(banned),
            "I-CT-1: the `/claims/{{cid}}` detail surface (countered or not, full page \
             or fragment) must render NO write / sign / counter / publish / follow / \
             subscribe control (authoring stays the slice-03 CLI; the counter CID \
             drill-links are render-only `<a href>` navigation TEXT); found {banned:?} \
             in body:\n{body}"
        );
    }
}

/// Assert the shown-never-applied invariant (CT-3 / CT-INV-ShownNeverApplied / I-CT-2 /
/// OD-AV-7 / ADR-015): the SAME claim's rendered confidence + fields are byte-IDENTICAL
/// whether or not it is countered — the counter never filters/merges/re-weights/re-ranks
/// the claim. Diffs the claim region of the countered render against the un-countered
/// render (and pins the verbatim confidence). Scans the rendered HTML only (Mandate 8
/// universe = port-exposed rendered surface).
///
/// SCAFFOLD: true (slice-11) — DELIVER asserts: the countered body contains the
/// `expected_confidence_verbatim` string byte-for-byte (`0.91`, never `0.9`/`91%`); AND
/// the claim region (subject/predicate/object/confidence/author/cid) of the countered
/// render is byte-identical to the un-countered render — the counter changed nothing
/// above the thread (I-CT-2 / I-CT-4). Any divergence in the claim region is an
/// UNSHIPPABLE shown-never-applied breach.
pub fn assert_counter_claim_verbatim_unchanged(
    uncountered_body: &str,
    countered_body: &str,
    expected_confidence_verbatim: &str,
) {
    // The countered claim's confidence renders VERBATIM (0.91, never 0.9/91%) and
    // UNCHANGED by the counter — the load-bearing shown-never-applied pin (I-CT-2 /
    // I-CT-4). It must be present in BOTH renders byte-for-byte.
    assert!(
        uncountered_body.contains(expected_confidence_verbatim),
        "I-CT-4: the un-countered baseline must render confidence \
         {expected_confidence_verbatim:?} verbatim;\n--- uncountered ---\n{uncountered_body}"
    );
    assert!(
        countered_body.contains(expected_confidence_verbatim),
        "I-CT-2/I-CT-4: the countered claim's confidence must render \
         {expected_confidence_verbatim:?} verbatim + UNCHANGED by the counter;\n\
         --- countered ---\n{countered_body}"
    );

    // The claim region (subject/predicate/object/confidence/author/composed_at/cid +
    // its evidence) is the `<dl>…</dl>` + evidence block inside `#claim-detail`. The
    // counter is additive context only — the neutral "Countered" presence flag is
    // inserted ABOVE the fields (between the `#claim-detail` open and the `<dl>`) and the
    // thread `<section>` BELOW the evidence (between the evidence and the `#claim-detail`
    // close); the FIELD + EVIDENCE block itself is byte-identical. Extract that
    // contiguous claim-region byte-run from the un-countered render by STABLE delimiters
    // and assert it appears VERBATIM inside the countered render. Any divergence is an
    // UNSHIPPABLE shown-never-applied breach (I-CT-2 / OD-AV-7 / ADR-015).
    //
    // STABLE delimiters (D1 — replaces the fragile `rfind("</div>")` heuristic, which
    // could anchor on a coincidental chrome `</div>` and let a byte-shifted claim region
    // pass): the region is pinned by the `#claim-detail` swap-target open, the `<dl>`
    // field-list open as the START, and the FIRST structural boundary that follows the
    // evidence as the END. In the COUNTERED render that boundary is the counter thread's
    // `<section`; in the UN-countered render (no thread) it is the `#claim-detail`-closing
    // `</div>`. We take whichever appears FIRST after the fields so the extracted run is
    // EXACTLY the field + evidence block — never trailing chrome — and the comparison
    // genuinely pins those bytes as identical countered-vs-uncountered.
    assert!(
        uncountered_body.contains(&format!("id=\"{CLAIM_DETAIL_REGION_ID}\"")),
        "D1: the un-countered detail must carry the #{CLAIM_DETAIL_REGION_ID} swap-target \
         region (the stable claim-region anchor);\n--- uncountered ---\n{uncountered_body}"
    );
    let region_start = uncountered_body
        .find("<dl>")
        .expect("the un-countered detail must render the claim-field <dl> region");
    let region_tail = &uncountered_body[region_start..];
    // The claim region ends at the FIRST of: the counter thread `<section` (absent in the
    // un-countered baseline) or the `#claim-detail`-closing `</div>`. Taking the earlier
    // boundary excludes any trailing chrome from the extracted run (D1 tightening).
    let section_end = region_tail.find("<section");
    let div_end = region_tail.find("</div>");
    let region_end = match (section_end, div_end) {
        (Some(s), Some(d)) => s.min(d),
        (Some(s), None) => s,
        (None, Some(d)) => d,
        (None, None) => panic!(
            "the claim-detail region must close with </div> (or a counter <section>); \
             un-countered render:\n{uncountered_body}"
        ),
    };
    let claim_region = &region_tail[..region_end];

    // Guard: the extracted region must actually carry the claim FIELDS and the EVIDENCE
    // heading — so the byte-identical assertion below pins the real claim region, not an
    // empty / truncated substring that could pass coincidentally (D1).
    assert!(
        claim_region.contains("</dl>") && claim_region.contains("Evidence"),
        "D1: the extracted claim region must span the field `<dl>…</dl>` AND the Evidence \
         section (so the byte-identical pin is meaningful, not a coincidental substring); \
         extracted:\n{claim_region}"
    );

    assert!(
        countered_body.contains(claim_region),
        "I-CT-2 / OD-AV-7 / ADR-015 (shown-never-applied): the claim region must be \
         BYTE-IDENTICAL with and without a counter (the counter never \
         re-weights/filters/merges/re-ranks the claim);\n--- expected claim region ---\n\
         {claim_region}\n--- countered ---\n{countered_body}"
    );
}

// =============================================================================
// slice-12 (viewer-counter-claim-list-flags) — the `/claims` LIST counter-presence FLAG
// seeds + assert helpers (US-LF-001/002/003; ADR-048). The flag is the slice-11
// `COUNTERED_PRESENCE_FLAG = "Countered"` neutral marker (viewer-domain), rendered as a
// render-only `<a href="/claims/{cid}">Countered</a>` one-hop link on each LIST row whose
// claim has ≥1 counter. These seeds REUSE the slice-11 production write paths (own counter
// via `claim counter`; peer counter via `peer add` + `peer pull`) widened to a MULTI-ROW
// own-claims list; the asserts scan the rendered LIST HTML (Mandate 8 universe = the
// port-exposed rendered surface, never an internal struct field).
//
// SCAFFOLD: true (slice-12) — every body is `todo!()` so the ATs COMPILE and panic at
// runtime → classify RED (MISSING_FUNCTIONALITY), NOT BROKEN. DELIVER materializes them
// (the DELIVER guidance is inlined in each doc comment).
// =============================================================================

/// The neutral list-row presence marker text — the slice-11 `COUNTERED_PRESENCE_FLAG`
/// reused VERBATIM ("Countered"). One source of truth so the slice-12 seeds + asserts
/// never drift from the viewer-domain constant. The marker is PRESENCE-ONLY: it is never a
/// verdict ("disputed"/"refuted"/"false"), never a count ("disputed by N"), and never a
/// sort/filter control (I-LF-3).
pub const LIST_COUNTERED_FLAG_TEXT: &str = "Countered";

/// The handle a `/claims`-list flag seed returns: the full set of own-claim CIDs on the
/// page (in `composed_at DESC, cid` order, the slice-06 list order) split into the
/// COUNTERED subset (rows that must carry the "Countered" marker) and the UN-COUNTERED
/// subset (rows that must carry NO marker). The scenario addresses exact rows via these
/// CIDs (the `assert_list_row_*` helpers + the `<a href="/claims/{cid}">` flag-link
/// targets). `ordered_cids` is the EXACT rendered order so the no-regression gold can pin
/// order byte-identity.
#[derive(Debug, Clone)]
pub struct SeededClaimsList {
    /// Every own-claim CID on the page, in the slice-06 rendered order
    /// (`composed_at DESC, cid`). The no-regression gold pins this order byte-identical
    /// with and without the flag (I-LF-2).
    pub ordered_cids: Vec<String>,
    /// The subset of `ordered_cids` whose claim has ≥1 counter — each row MUST carry the
    /// neutral "Countered" marker linking to `/claims/{cid}` (US-LF-002).
    pub countered_cids: Vec<String>,
    /// The subset of `ordered_cids` with NO counter — each row MUST carry NO marker and no
    /// "0 counters" noise (US-LF-003 no-noise).
    pub uncountered_cids: Vec<String>,
}

/// Seed an own-claims `/claims` page with EXACTLY ONE peer-countered claim among several
/// plain own claims (the LF-1 walking-skeleton + LF-2/LF-4/LF-INV-* fixture). The
/// countered claim is countered by a PEER (so the presence read exercises the
/// `peer_claim_references` arm of the UNION-ALL), pulled via the production `peer add` +
/// `peer pull` federation path; the rest are plain own claims signed via `claim add`. All
/// rows land in the SAME local store `openlore ui` reads (Pillar 3 / BR-VIEW-4). Returns
/// the [`SeededClaimsList`] so the scenario addresses the exact countered + un-countered
/// rows.
///
/// SCAFFOLD: true (slice-12) — DELIVER materializes it by: (1) signing several plain own
/// claims via `seed_own_claim_with_evidence` (distinct subjects so distinct CIDs);
/// (2) seeding ONE more own claim that a PEER then counters — reuse the slice-11 pattern
/// (`build_verifiable_peer_counter_record(COUNTER_AUTHOR_TOBIAS, seed, target_cid, reason)`
/// + `peer add` + `peer pull`) so a `counters`-referencing peer record targeting that
/// claim lands in `peer_claim_references`; (3) recover all own-claim CIDs in the slice-06
/// `composed_at DESC, cid` list order (e.g. via `read_own_claim_cids_in_list_order(env)`
/// or by capturing each `claim add` CID + sorting per the list contract). NO hand-inserted
/// store rows.
pub fn seed_claims_list_one_countered(env: &TestEnv) -> SeededClaimsList {
    // STEP 1 — sign several PLAIN own claims via the production `claim add` write path
    // (distinct subjects → distinct CIDs). These are the un-countered rows; nothing
    // references them.
    for (subject, predicate, object) in [
        ("github:rust-lang/rust", "embodiesPhilosophy", "org.openlore.philosophy.memory-safety"),
        ("github:denoland/deno", "embodiesPhilosophy", "org.openlore.philosophy.secure-by-default"),
        ("github:ziglang/zig", "embodiesPhilosophy", "org.openlore.philosophy.no-hidden-control-flow"),
    ] {
        seed_own_claim_with_evidence(env, subject, predicate, object, 0.90, &[]);
    }

    // STEP 2 — sign ONE more own claim that a PEER then counters. Recover its
    // content-addressed CID (the counter's target).
    let target_cid = seed_own_claim_with_evidence(
        env,
        "github:rust-lang/cargo",
        "embodiesPhilosophy",
        "org.openlore.philosophy.dependency-pinning",
        0.90,
        &[],
    );

    // STEP 3 — peer Tobias authors a verifiable COUNTER referencing the OWN target CID
    // (a `references[].type == counters` entry whose `cid == target_cid`, ADR-015),
    // delivered through the production `peer add` + `peer pull` federation path. The
    // pull verifies + content-addresses it: the counter lands in `peer_claims` and its
    // `counters` reference lands in `peer_claim_references` with `referenced_cid ==
    // target_cid` (so the presence read exercises the peer arm of the UNION-ALL).
    let tobias_seed = [9u8; 32];
    let (tobias_record, tobias_pubkey_hex) = build_verifiable_peer_counter_record(
        COUNTER_AUTHOR_TOBIAS,
        tobias_seed,
        &target_cid,
        Some(COUNTER_PEER_REASON_VERBATIM),
    );
    let tobias_pds = PeerPds::for_peer(COUNTER_AUTHOR_TOBIAS, vec![tobias_record]);

    let added = run_openlore_with_peer_resolver(
        env,
        &["peer", "add", COUNTER_AUTHOR_TOBIAS],
        COUNTER_AUTHOR_TOBIAS,
        tobias_pds.endpoint_url(),
    );
    assert_eq!(
        added.status, 0,
        "seed_claims_list_one_countered: peer add for {COUNTER_AUTHOR_TOBIAS} must succeed;\n\
         --- stdout ---\n{}\n--- stderr ---\n{}",
        added.stdout, added.stderr
    );
    let pulled = run_openlore_pull_multi(
        env,
        &["peer", "pull"],
        &[PeerSeam {
            peer_did: COUNTER_AUTHOR_TOBIAS,
            peer_endpoint: tobias_pds.endpoint_url(),
            peer_pubkey_hex: &tobias_pubkey_hex,
        }],
    );
    assert_eq!(
        pulled.status, 0,
        "seed_claims_list_one_countered: peer pull must succeed;\n\
         --- stdout ---\n{}\n--- stderr ---\n{}",
        pulled.stdout, pulled.stderr
    );

    // STEP 4 — recover every own-claim CID in the slice-06 `composed_at DESC, cid`
    // list order; split into the ONE countered + the rest un-countered.
    let ordered_cids = read_own_claim_cids_in_list_order(env);
    assert!(
        ordered_cids.contains(&target_cid),
        "seed_claims_list_one_countered: the countered target CID {target_cid:?} must be \
         among the own claims; got {ordered_cids:?}"
    );
    let uncountered_cids = ordered_cids
        .iter()
        .filter(|cid| *cid != &target_cid)
        .cloned()
        .collect::<Vec<_>>();

    SeededClaimsList {
        ordered_cids,
        countered_cids: vec![target_cid],
        uncountered_cids,
    }
}

/// Seed an own-claims `/claims` page where ONE of Maria's OWN claims is countered by TWO
/// DISTINCT authors (the LF-3 presence-only GOLD fixture; I-LF-3 / KPI-AV-2). Adapts the
/// slice-11 `seed_claim_two_counters_distinct_authors` anti-merging fixture into the LIST
/// context: the target is one of Maria's OWN claims (so it appears on `/claims`), countered
/// by TWO distinct PEER authors (Rachel + Tobias) — each authors a verifiable `counters`-
/// referencing record targeting the SAME own CID, delivered through the production
/// `peer add` + `peer pull` federation path, so BOTH land in `peer_claim_references` with
/// `referenced_cid == target_cid`. The `counter_presence_for` UNION-ALL DISTINCT collapses
/// the two distinct-author counters of the SAME CID to ONE presence membership → the row
/// carries EXACTLY ONE neutral "Countered" marker (never "disputed by 2"). All rows land in
/// the SAME local store `openlore ui` reads (Pillar 3 / BR-VIEW-4). Returns the
/// [`SeededClaimsList`] whose single `countered_cids` entry is the twice-countered own
/// target.
pub fn seed_claims_list_target_two_counters_distinct_authors(env: &TestEnv) -> SeededClaimsList {
    // STEP 1 — sign several PLAIN own claims via the production `claim add` write path
    // (distinct subjects → distinct CIDs). These are the un-countered rows.
    for (subject, predicate, object) in [
        ("github:rust-lang/rust", "embodiesPhilosophy", "org.openlore.philosophy.memory-safety"),
        ("github:denoland/deno", "embodiesPhilosophy", "org.openlore.philosophy.secure-by-default"),
    ] {
        seed_own_claim_with_evidence(env, subject, predicate, object, 0.90, &[]);
    }

    // STEP 2 — sign ONE more own claim that TWO distinct peers then counter. Recover its
    // content-addressed CID (both counters' shared target).
    let target_cid = seed_own_claim_with_evidence(
        env,
        "github:rust-lang/cargo",
        "embodiesPhilosophy",
        "org.openlore.philosophy.dependency-pinning",
        0.90,
        &[],
    );

    // STEP 3 — TWO distinct peers (Rachel + Tobias) each author a verifiable COUNTER
    // referencing the SAME OWN target CID (a `references[].type == counters` entry whose
    // `cid == target_cid`, ADR-015), delivered through the production `peer add` +
    // `peer pull` federation path. Build BOTH peers' records UP FRONT, holding each PDS
    // ALIVE for the whole function so a SINGLE `peer pull` over BOTH peers succeeds. The
    // pull verifies + content-addresses each: both counters land in `peer_claims` and
    // their `counters` references land in `peer_claim_references` with the SAME
    // `referenced_cid == target_cid` — two DISTINCT authors, one referenced CID.
    let rachel_seed = [7u8; 32];
    let (rachel_record, rachel_pubkey_hex) = build_verifiable_peer_counter_record(
        COUNTER_TARGET_AUTHOR_RACHEL,
        rachel_seed,
        &target_cid,
        Some(COUNTER_REASON_VERBATIM),
    );
    let tobias_seed = [9u8; 32];
    let (tobias_record, tobias_pubkey_hex) = build_verifiable_peer_counter_record(
        COUNTER_AUTHOR_TOBIAS,
        tobias_seed,
        &target_cid,
        Some(COUNTER_PEER_REASON_VERBATIM),
    );

    let rachel_pds = PeerPds::for_peer(COUNTER_TARGET_AUTHOR_RACHEL, vec![rachel_record]);
    let tobias_pds = PeerPds::for_peer(COUNTER_AUTHOR_TOBIAS, vec![tobias_record]);

    for (did, pds) in [
        (COUNTER_TARGET_AUTHOR_RACHEL, &rachel_pds),
        (COUNTER_AUTHOR_TOBIAS, &tobias_pds),
    ] {
        let added = run_openlore_with_peer_resolver(
            env,
            &["peer", "add", did],
            did,
            pds.endpoint_url(),
        );
        assert_eq!(
            added.status, 0,
            "seed_claims_list_target_two_counters_distinct_authors: peer add for {did} must \
             succeed;\n--- stdout ---\n{}\n--- stderr ---\n{}",
            added.stdout, added.stderr
        );
    }
    let pulled = run_openlore_pull_multi(
        env,
        &["peer", "pull"],
        &[
            PeerSeam {
                peer_did: COUNTER_TARGET_AUTHOR_RACHEL,
                peer_endpoint: rachel_pds.endpoint_url(),
                peer_pubkey_hex: &rachel_pubkey_hex,
            },
            PeerSeam {
                peer_did: COUNTER_AUTHOR_TOBIAS,
                peer_endpoint: tobias_pds.endpoint_url(),
                peer_pubkey_hex: &tobias_pubkey_hex,
            },
        ],
    );
    assert_eq!(
        pulled.status, 0,
        "seed_claims_list_target_two_counters_distinct_authors: peer pull must succeed;\n\
         --- stdout ---\n{}\n--- stderr ---\n{}",
        pulled.stdout, pulled.stderr
    );

    // STEP 4 — recover every own-claim CID in the slice-06 `composed_at DESC, cid` list
    // order; split into the ONE twice-countered own target + the rest un-countered. (The
    // peer counters live in `peer_claims`, NOT `claims`, so the own list is exactly the
    // three `claim add` rows — the target plus the two plain rows.)
    let ordered_cids = read_own_claim_cids_in_list_order(env);
    assert!(
        ordered_cids.contains(&target_cid),
        "seed_claims_list_target_two_counters_distinct_authors: the countered target CID \
         {target_cid:?} must be among the own claims; got {ordered_cids:?}"
    );
    let uncountered_cids = ordered_cids
        .iter()
        .filter(|cid| *cid != &target_cid)
        .cloned()
        .collect::<Vec<_>>();

    SeededClaimsList {
        ordered_cids,
        countered_cids: vec![target_cid],
        uncountered_cids,
    }
}

/// Seed an own-claims `/claims` page with NO counters at all (the LF-5 / LF-INV-NoWrite
/// no-noise fixture): several plain own claims, NOTHING references any of them as a
/// counter, so `counter_presence_for` returns the EMPTY set and the list renders
/// byte-identically to slice-06. Returns the [`SeededClaimsList`] with an EMPTY
/// `countered_cids` (every CID is in `uncountered_cids`).
///
/// SCAFFOLD: true (slice-12) — DELIVER materializes it by signing several plain own claims
/// via `seed_own_claim_with_evidence` (distinct subjects → distinct CIDs), recovering them
/// in the slice-06 `composed_at DESC, cid` list order, and returning them all as
/// `uncountered_cids` with an empty `countered_cids`. NO counter is authored. NO
/// hand-inserted store rows.
pub fn seed_claims_list_none_countered(env: &TestEnv) -> SeededClaimsList {
    // Sign several PLAIN own claims via the production `claim add` write path (distinct
    // subjects → distinct CIDs). NOTHING references any of them as a counter, so
    // `counter_presence_for` returns the EMPTY set and the list renders byte-identically
    // to slice-06. NO counter is authored, NO peer is added/pulled.
    for (subject, predicate, object) in [
        ("github:rust-lang/rust", "embodiesPhilosophy", "org.openlore.philosophy.memory-safety"),
        ("github:denoland/deno", "embodiesPhilosophy", "org.openlore.philosophy.secure-by-default"),
        ("github:ziglang/zig", "embodiesPhilosophy", "org.openlore.philosophy.no-hidden-control-flow"),
        ("github:rust-lang/cargo", "embodiesPhilosophy", "org.openlore.philosophy.dependency-pinning"),
    ] {
        seed_own_claim_with_evidence(env, subject, predicate, object, 0.90, &[]);
    }

    // Recover every own-claim CID in the slice-06 `composed_at DESC, cid` list order.
    // With no counter authored, EVERY row is un-countered: countered_cids is EMPTY and
    // every CID lands in uncountered_cids.
    let ordered_cids = read_own_claim_cids_in_list_order(env);
    let uncountered_cids = ordered_cids.clone();

    SeededClaimsList {
        ordered_cids,
        countered_cids: Vec::new(),
        uncountered_cids,
    }
}

/// Seed an own-claims `/claims` page MIXING countered + un-countered claims in a known
/// `composed_at DESC, cid` order (the LF-6/LF-7/LF-8 + LF-INV-ShownNeverApplied fixture).
/// The page is large enough that the LF-8 N+1-guard behavioral proxy is meaningful (many
/// rows, a known countered subset), and the order is fixed so the no-regression gold can
/// pin order/paging/count/confidence byte-identity with and without the flag. Counters are
/// a mix of OWN (`claim counter` → `claim_references`) and PEER (`peer pull` →
/// `peer_claim_references`) so BOTH UNION-ALL arms are exercised. Returns the
/// [`SeededClaimsList`].
///
/// SCAFFOLD: true (slice-12) — DELIVER materializes it by signing N plain own claims, then
/// countering a KNOWN subset (some via Maria's OWN `claim counter`, some via a PEER
/// `build_verifiable_peer_counter_record` + `peer add` + `peer pull`), recovering every
/// own-claim CID in the slice-06 `composed_at DESC, cid` order, and returning the ordered
/// + countered + un-countered CID vecs. The COUNT, ORDER, and each row's confidence must
/// match the slice-06 render of the SAME store (the no-regression baseline). NO
/// hand-inserted store rows.
pub fn seed_claims_list_mixed_pages(env: &TestEnv) -> SeededClaimsList {
    // STEP 1 — sign N plain own claims via the production `claim add` write path (distinct
    // subjects → distinct CIDs). A page large enough that the LF-8 N+1-guard behavioral
    // proxy is meaningful (many rows, a known countered subset interleaved among them).
    let mut own_targets: Vec<String> = Vec::new();
    for (subject, predicate, object) in [
        ("github:rust-lang/rust", "embodiesPhilosophy", "org.openlore.philosophy.memory-safety"),
        ("github:denoland/deno", "embodiesPhilosophy", "org.openlore.philosophy.secure-by-default"),
        ("github:ziglang/zig", "embodiesPhilosophy", "org.openlore.philosophy.no-hidden-control-flow"),
        ("github:rust-lang/cargo", "embodiesPhilosophy", "org.openlore.philosophy.dependency-pinning"),
        ("github:golang/go", "embodiesPhilosophy", "org.openlore.philosophy.simplicity"),
        ("github:python/cpython", "embodiesPhilosophy", "org.openlore.philosophy.readability"),
        ("github:nodejs/node", "embodiesPhilosophy", "org.openlore.philosophy.event-driven"),
        ("github:elixir-lang/elixir", "embodiesPhilosophy", "org.openlore.philosophy.fault-tolerance"),
    ] {
        own_targets.push(seed_own_claim_with_evidence(env, subject, predicate, object, 0.90, &[]));
    }

    // STEP 2 — counter a KNOWN subset of Maria's OWN claims via distinct PEER counters
    // (the operator can NEVER counter her OWN claim — `claim counter` rejects a self-target
    // with "use retract instead" — so an OWN list row is flagged ONLY through the
    // `peer_claim_references` arm: a PEER who authored a `counters`-referencing record
    // targeting that own CID). The countered targets are spread across the list (indices
    // 1, 4, 6) so the flagged rows INTERLEAVE with un-countered rows in the rendered order,
    // never grouped (LF-7). Rachel authors TWO counters (targets A + B) and Tobias ONE
    // (target C), exercising one peer with MULTIPLE counters + a second DISTINCT peer —
    // three DISTINCT `referenced_cid`s, three flagged own rows.
    let peer_target_a = own_targets[1].clone();
    let peer_target_b = own_targets[4].clone();
    let peer_target_c = own_targets[6].clone();

    // PEER arm — build BOTH peers' records up front, hold each PDS alive for the whole
    // function, and pull BOTH in a SINGLE `peer pull` so each `counters` reference lands in
    // `peer_claim_references` with `referenced_cid == <own target CID>`.
    let rachel_seed = [7u8; 32];
    let (rachel_record_a, rachel_pubkey_hex) = build_verifiable_peer_counter_record(
        COUNTER_TARGET_AUTHOR_RACHEL,
        rachel_seed,
        &peer_target_a,
        Some(COUNTER_REASON_VERBATIM),
    );
    let (rachel_record_b, _) = build_verifiable_peer_counter_record(
        COUNTER_TARGET_AUTHOR_RACHEL,
        rachel_seed,
        &peer_target_b,
        Some(COUNTER_REASON_VERBATIM),
    );
    let tobias_seed = [9u8; 32];
    let (tobias_record, tobias_pubkey_hex) = build_verifiable_peer_counter_record(
        COUNTER_AUTHOR_TOBIAS,
        tobias_seed,
        &peer_target_c,
        Some(COUNTER_PEER_REASON_VERBATIM),
    );

    let rachel_pds =
        PeerPds::for_peer(COUNTER_TARGET_AUTHOR_RACHEL, vec![rachel_record_a, rachel_record_b]);
    let tobias_pds = PeerPds::for_peer(COUNTER_AUTHOR_TOBIAS, vec![tobias_record]);

    for (did, pds) in [
        (COUNTER_TARGET_AUTHOR_RACHEL, &rachel_pds),
        (COUNTER_AUTHOR_TOBIAS, &tobias_pds),
    ] {
        let added = run_openlore_with_peer_resolver(env, &["peer", "add", did], did, pds.endpoint_url());
        assert_eq!(
            added.status, 0,
            "seed_claims_list_mixed_pages: peer add for {did} must succeed;\n\
             --- stdout ---\n{}\n--- stderr ---\n{}",
            added.stdout, added.stderr
        );
    }
    let pulled = run_openlore_pull_multi(
        env,
        &["peer", "pull"],
        &[
            PeerSeam {
                peer_did: COUNTER_TARGET_AUTHOR_RACHEL,
                peer_endpoint: rachel_pds.endpoint_url(),
                peer_pubkey_hex: &rachel_pubkey_hex,
            },
            PeerSeam {
                peer_did: COUNTER_AUTHOR_TOBIAS,
                peer_endpoint: tobias_pds.endpoint_url(),
                peer_pubkey_hex: &tobias_pubkey_hex,
            },
        ],
    );
    assert_eq!(
        pulled.status, 0,
        "seed_claims_list_mixed_pages: peer pull must succeed;\n\
         --- stdout ---\n{}\n--- stderr ---\n{}",
        pulled.stdout, pulled.stderr
    );

    // STEP 3 — recover EVERY own-claim CID in the slice-06 `composed_at DESC, cid` list
    // order. The countered subset is the three peer-countered targets; everything else (the
    // remaining plain own rows) is un-countered. The peer counters live in `peer_claims`,
    // NOT `claims`, so the own list is exactly the eight `claim add` rows.
    let ordered_cids = read_own_claim_cids_in_list_order(env);
    let countered_cids = vec![peer_target_a, peer_target_b, peer_target_c];
    for cid in &countered_cids {
        assert!(
            ordered_cids.contains(cid),
            "seed_claims_list_mixed_pages: countered target CID {cid:?} must be among the \
             own claims; got {ordered_cids:?}"
        );
    }
    let uncountered_cids = ordered_cids
        .iter()
        .filter(|cid| !countered_cids.contains(cid))
        .cloned()
        .collect::<Vec<_>>();

    SeededClaimsList {
        ordered_cids,
        countered_cids,
        uncountered_cids,
    }
}

/// Assert a rendered `/claims` LIST body (fragment OR full page) FLAGS the given countered
/// row: the row carries the neutral "Countered" marker ([`LIST_COUNTERED_FLAG_TEXT`])
/// rendered as a render-only `<a href="/claims/{cid}">Countered</a>` one-hop link to that
/// claim's slice-11 thread (US-LF-002 / I-LF-6). Scans the rendered HTML only (Mandate 8
/// universe = the port-exposed rendered surface).
///
/// SCAFFOLD: true (slice-12) — DELIVER asserts: the body contains the marker text
/// "Countered" associated with this row's CID, AND the marker is the render-only anchor
/// `<a href="/claims/{cid}">Countered</a>` (the one-hop drill-link, never an executable
/// control — I-LF-1/I-LF-6).
pub fn assert_list_row_flagged_countered(body: &str, countered_cid: &str) {
    // The flag is the render-only one-hop anchor `<a href="/claims/{cid}">Countered</a>`
    // (maud emits no whitespace inside the element). Scan the rendered HTML only.
    let marker = format!(
        "<a href=\"/claims/{countered_cid}\">{LIST_COUNTERED_FLAG_TEXT}</a>"
    );
    assert!(
        body.contains(&marker),
        "assert_list_row_flagged_countered: the countered row for {countered_cid:?} must \
         carry the render-only marker {marker:?} (neutral presence flag + one-hop link; \
         US-LF-002 / I-LF-6); body was:\n{body}"
    );
}

/// Assert a rendered `/claims` LIST body does NOT flag the given un-countered row: the row
/// for `uncountered_cid` carries NO "Countered" marker and NO "0 counters" / empty-state
/// noise — it renders exactly as slice-06 (US-LF-003 no-noise / I-LF-2). Scans the
/// rendered HTML only.
///
/// SCAFFOLD: true (slice-12) — DELIVER asserts: the un-countered row's rendered region
/// carries NO `<a href="/claims/{cid}">Countered</a>` marker and NO "0 counters" /
/// "no disagreement" empty-state text (the un-countered row is byte-unaffected by the
/// flag — I-LF-2).
pub fn assert_list_row_not_flagged(body: &str, uncountered_cid: &str) {
    // The un-countered row carries NO `<a href="/claims/{cid}">Countered</a>` flag
    // anchor. (Its bare CID still appears in the row's CID cell — we assert the
    // absence of the FLAG anchor specifically, not the CID text.)
    let marker = format!(
        "<a href=\"/claims/{uncountered_cid}\">{LIST_COUNTERED_FLAG_TEXT}</a>"
    );
    assert!(
        !body.contains(&marker),
        "assert_list_row_not_flagged: the un-countered row for {uncountered_cid:?} must \
         carry NO 'Countered' flag marker {marker:?} (renders exactly as slice-06; \
         US-LF-003 / I-LF-2); body was:\n{body}"
    );
    // And NO "0 counters" / "no disagreement" empty-state noise anywhere on the page.
    for noise in ["0 counters", "no disagreement", "no counters"] {
        assert!(
            !body.to_lowercase().contains(noise),
            "assert_list_row_not_flagged: an un-countered list must carry no {noise:?} \
             empty-state noise (no-noise discipline; US-LF-003); body was:\n{body}"
        );
    }
}

/// Assert the "Countered" marker on a countered LIST row is a render-only
/// `<a href="/claims/{cid}">` ONE-HOP link to that claim's slice-11 thread — navigation
/// TEXT, never an executable write/sign/counter control (US-LF-002 / I-LF-1 / I-LF-6).
///
/// SCAFFOLD: true (slice-12) — DELIVER asserts: the body contains the exact anchor
/// `<a href="/claims/{cid}">` wrapping the "Countered" marker for this row, and the marker
/// is render-only navigation TEXT (no form/button/onclick — the human gate stays the CLI).
pub fn assert_list_flag_links_to_thread(body: &str, countered_cid: &str) {
    // The marker is the render-only one-hop anchor `<a href="/claims/{cid}">Countered</a>`
    // — navigation TEXT to the slice-11 thread, never an executable control (maud emits no
    // whitespace inside the element). Scan the rendered HTML only.
    let anchor = format!(
        "<a href=\"/claims/{countered_cid}\">{LIST_COUNTERED_FLAG_TEXT}</a>"
    );
    assert!(
        body.contains(&anchor),
        "assert_list_flag_links_to_thread: the 'Countered' marker on the countered row for \
         {countered_cid:?} must be the render-only one-hop link {anchor:?} to its slice-11 \
         thread (navigation TEXT, never a control; US-LF-002 / I-LF-1 / I-LF-6); body \
         was:\n{body}"
    );
    // It is a plain anchor — never an executable write/sign/counter control. The list flag
    // never emits a form, button, or onclick wrapping the marker (the human gate stays the
    // CLI; I-LF-1).
    for control in ["<form", "<button", "onclick"] {
        assert!(
            !body.to_ascii_lowercase().contains(control),
            "assert_list_flag_links_to_thread: the list flag must be a render-only anchor, \
             never an executable control ({control:?}); the only write path is the CLI \
             (I-LF-1); body was:\n{body}"
        );
    }
}

/// Assert a claim countered by N (>1) authors shows EXACTLY ONE neutral "Countered" marker
/// on its LIST row — presence-only, NEVER a count ("disputed by N"), a verdict
/// ("disputed"/"refuted"/"false"), or a merged consensus row (US-LF-002 / I-LF-3 /
/// KPI-AV-2). Reuses the slice-11 neutral-flag vocabulary (the `assert_counter_thread_
/// presence_flag_is_neutral` verdict-word blocklist) on the LIST surface.
///
/// SCAFFOLD: true (slice-12) — DELIVER asserts: the row for `target_cid` carries EXACTLY
/// one "Countered" marker (presence membership → one flag, DISTINCT referenced_cid), and
/// the list body carries NONE of the count/verdict phrasings ("disputed by", "consensus",
/// "net verdict", "disputed", "refuted", "is false", "is wrong").
pub fn assert_list_flag_is_single_neutral_presence(body: &str, target_cid: &str) {
    // PRESENCE-only: a CID countered by N distinct authors collapses (DISTINCT
    // referenced_cid) to ONE presence membership → EXACTLY ONE render-only "Countered"
    // marker on its row, NEVER one-per-counter and never a count. Count the exact anchor
    // occurrences for this CID — it must appear EXACTLY once.
    let marker = format!(
        "<a href=\"/claims/{target_cid}\">{LIST_COUNTERED_FLAG_TEXT}</a>"
    );
    let occurrences = body.matches(&marker).count();
    assert_eq!(
        occurrences, 1,
        "assert_list_flag_is_single_neutral_presence: the row for {target_cid:?} (countered \
         by ≥2 distinct authors) must carry EXACTLY ONE neutral 'Countered' marker \
         (presence membership → one flag, DISTINCT referenced_cid; I-LF-3 / KPI-AV-2), got \
         {occurrences} occurrences of {marker:?}; body was:\n{body}"
    );

    // The flag is PRESENCE-only: the list body carries NONE of the count / verdict /
    // merged-judgement phrasings — never "disputed by N", never a consensus or net
    // verdict. Lowercased so a capitalized variant can never sneak through (mirrors the
    // slice-11 `assert_counter_thread_presence_flag_is_neutral` blocklist on the LIST).
    let lowered = body.to_ascii_lowercase();
    for verdict in [
        // count-based / merged-judgement phrasing (never a count aggregated to a verdict)
        "disputed by",
        "consensus",
        "net verdict",
        "people disagree",
        // verdict words — the flag asserts presence, never correctness of the counter
        "disputed",
        "refuted",
        "is false",
        "is wrong",
    ] {
        assert!(
            !lowered.contains(verdict),
            "assert_list_flag_is_single_neutral_presence: the list 'Countered' flag is \
             presence-only — it must NEVER emit a count / verdict / merged-judgement \
             phrasing ({verdict:?}); a twice-countered row shows ONE neutral marker, never \
             'disputed by 2' (I-LF-3 / KPI-AV-2); body was:\n{body}"
        );
    }
}

/// Assert the flagged `/claims` LIST render is byte-identical to the slice-06 reference
/// render of the SAME store in every dimension EXCEPT the additive "Countered" markers:
/// the row ORDER (`composed_at DESC, cid`), the PAGING / position indicator, the total
/// COUNT, and EVERY row's CONFIDENCE are byte-IDENTICAL — the flag never re-orders,
/// re-paginates, re-counts, or re-weights the list (US-LF-003 no-regression GOLD / I-LF-2
/// / I-LF-4).
///
/// ## Baseline-capture tactic (b) — the RECORDED slice-06 ordering
///
/// There is NO pre-flag binary and NO no-flag HTTP render seam (the `/claims` route in
/// `adapter-http-viewer` ALWAYS reads `counter_presence_for` — adding a presence-suppression
/// mode would be a production test-seam, out of scope). Tactic (a)'s "twin no-counter store"
/// is ALSO unusable: a claim's CID canonicalizes its `composed_at` (claim-domain
/// `canonicalize` → `compute_cid`), so re-seeding the same claims at a different wall-clock
/// instant yields DIFFERENT CIDs — the two renders would never be byte-identical. So we use
/// tactic (b): the slice-06 reference is the RECORDED order + count + verbatim confidence the
/// seed already captures ([`SeededClaimsList::ordered_cids`] from the slice-06
/// `SELECT cid FROM claims ORDER BY composed_at DESC, cid` read), and we PROVE the flag is
/// additive-only by ELIDING the `<a href="/claims/{cid}">Countered</a>` anchors from
/// `flagged` (the additive markers) and asserting the elided slice-06 body still honours that
/// recorded order/count/paging/confidence byte-for-byte.
///
/// `flagged` is the slice-12 `/claims` render; `baseline` is the RECORDED slice-06 spec for
/// the SAME store (the seed's ordered CIDs + each row's verbatim confidence string). The
/// elision is non-circular: it removes ONLY the additive anchors, so any structural
/// regression (a re-order, re-page, re-count, or re-weight) SURVIVES elision and FAILS the
/// recorded-order / position-indicator / confidence-cell assertions below.
pub struct Slice06Baseline {
    /// The own-claim CIDs in the recorded slice-06 `composed_at DESC, cid` order — the order
    /// the elided (no-flag) body's rows MUST appear in (strictly increasing byte offsets).
    pub ordered_cids: Vec<String>,
    /// Each row's verbatim confidence cell as slice-06 renders it (`render_confidence` →
    /// `"0.90"`), parallel to `ordered_cids`. Every cell MUST survive the elision unchanged.
    pub confidence_cells: Vec<String>,
}

/// Assert (US-LF-003 no-regression GOLD; tactic (b)) the slice-12 `flagged` render, with the
/// additive "Countered" anchors elided, is byte-identical to the recorded slice-06
/// `baseline` of the SAME store in ROW ORDER, the POSITION INDICATOR (paging + total count),
/// and EVERY row's CONFIDENCE cell — the flag changed nothing but the additive marker.
pub fn assert_list_order_and_confidence_byte_identical(flagged: &str, baseline: &Slice06Baseline) {
    // ELIDE the additive markers: remove every `<a href="/claims/{cid}">Countered</a>`
    // anchor (the ONLY thing slice-12 adds — appended INSIDE the CID `<td>`, see
    // viewer-domain `render_claim_row` / `render_list_presence_flag`). What remains IS the
    // slice-06 byte-stream for this store. Eliding for EVERY recorded CID (countered rows
    // carry one; un-countered rows carry none, so the replace is a no-op there) keeps the
    // helper agnostic to WHICH rows were flagged.
    let mut elided = flagged.to_string();
    for cid in &baseline.ordered_cids {
        let anchor = format!("<a href=\"/claims/{cid}\">{LIST_COUNTERED_FLAG_TEXT}</a>");
        elided = elided.replace(&anchor, "");
    }
    // No additive marker survives the elision — the remaining body is pure slice-06.
    assert!(
        !elided.contains(LIST_COUNTERED_FLAG_TEXT),
        "assert_list_order_and_confidence_byte_identical: every additive {LIST_COUNTERED_FLAG_TEXT:?} \
         anchor must elide cleanly so the remaining body is the slice-06 render; a residual \
         marker means the flag is NOT purely the recorded `<a href>` anchor (US-LF-003 / \
         I-LF-2); elided body was:\n{elided}"
    );

    // ROW ORDER byte-identity: every recorded CID appears EXACTLY once in the elided
    // (no-flag) body, and their first-seen byte offsets are STRICTLY INCREASING in the
    // recorded `composed_at DESC, cid` order — the flag re-ordered NOTHING (I-LF-2 / I-LF-4).
    let mut prev_offset: Option<usize> = None;
    for cid in &baseline.ordered_cids {
        let offset = elided.find(cid.as_str()).unwrap_or_else(|| {
            panic!(
                "assert_list_order_and_confidence_byte_identical: the elided slice-06 body must \
                 contain every recorded row CID; {cid:?} was missing — the flag dropped/replaced a \
                 row (a re-count/re-page regression); elided body was:\n{elided}"
            )
        });
        if let Some(prev) = prev_offset {
            assert!(
                offset > prev,
                "assert_list_order_and_confidence_byte_identical: the elided slice-06 row order must \
                 follow the recorded `composed_at DESC, cid` order {:?} VERBATIM — {cid:?} rendered \
                 out of position, so the flag RE-ORDERED the list (US-LF-003 / I-LF-2); elided body \
                 was:\n{elided}",
                baseline.ordered_cids
            );
        }
        prev_offset = Some(offset);
    }

    // POSITION INDICATOR byte-identity (paging + total COUNT): the count is the recorded row
    // count; with the fixed page size (50, ADR-030) a single page renders the verbatim
    // `1–N of N` indicator (EN DASH U+2013, matching viewer-domain `render_position_indicator`).
    // The flag never re-paged or re-counted — the indicator is byte-exact (I-LF-2 / I-LF-4).
    let total = baseline.ordered_cids.len();
    let expected_indicator = format!("1\u{2013}{total} of {total}");
    assert!(
        elided.contains(&expected_indicator),
        "assert_list_order_and_confidence_byte_identical: the elided slice-06 body must carry the \
         verbatim position indicator {expected_indicator:?} (paging + total count unchanged by the \
         flag; US-LF-003 / I-LF-2); elided body was:\n{elided}"
    );

    // CONFIDENCE cell byte-identity: every recorded row's verbatim confidence cell
    // (`<td>0.90</td>`, slice-06 `render_confidence`) survives the elision UNCHANGED — the
    // flag RE-WEIGHTED nothing (I-LF-2 / I-LF-4). Assert one `<td>{conf}</td>` per row so the
    // count of confidence cells matches the recorded row count (no row's weight was altered
    // or dropped).
    for confidence in &baseline.confidence_cells {
        let cell = format!("<td>{confidence}</td>");
        let occurrences = elided.matches(&cell).count();
        assert!(
            occurrences >= 1,
            "assert_list_order_and_confidence_byte_identical: the elided slice-06 body must render \
             the verbatim confidence cell {cell:?} (the flag re-weights nothing; US-LF-003 / \
             I-LF-4); elided body was:\n{elided}"
        );
    }
    assert_eq!(
        elided.matches("</td>").count() % baseline.ordered_cids.len().max(1),
        0,
        "assert_list_order_and_confidence_byte_identical: the elided slice-06 body's cell count must \
         be a whole multiple of the recorded row count — a fractional remainder means a row was \
         dropped or duplicated (a re-count regression; US-LF-003 / I-LF-2); elided body was:\n{elided}"
    );
}
// =============================================================================
// slice-13 (viewer-counter-flags-graph-surfaces; DISTILL) — the SAME neutral
// "Countered" presence flag (slice-11/12 `COUNTERED_PRESENCE_FLAG`, REUSED
// verbatim) extended to the OTHER two LOCAL surfaces the operator scans: the
// FEDERATED `/peer-claims` list (US-CF-002) and the GRAPH-TRAVERSAL `/project` +
// `/philosophy` EDGE surveys (US-CF-003). REUSES the slice-12
// `StoreReadPort::counter_presence_for(&[cid]) -> HashSet<String>` batch read
// (ADR-048 / ADR-049) — NO new read method. `/score` is OUT (deferred slice-14).
//
// The flag is render-only `<a href="/claims/{cid}">Countered</a>` navigation
// TEXT (one-hop link to the slice-11 thread), PRESENCE-only (a row/edge countered
// by N authors shows ONE neutral marker, never "disputed by N"), and ADDITIVE: it
// NEVER re-orders the peer list / paging, and on the edge surfaces NEVER changes
// the `group_by` grouping, group order, edge order, deduped contributor list, or
// any cross-link (I-CF-2 / I-CF-9 shown-never-applied). An un-countered row/edge
// renders byte-identically to slice-06 (`/peer-claims`) / slice-10 (traversal).
//
// Seeding drives the PRODUCTION paths (Pillar 3 / BR-VIEW-4): peer claims +
// survey edges land via `peer add` + `peer pull` (REUSING `seed_peer_claims_via_
// pull` / `seed_project_survey_trail` / `seed_philosophy_survey_trail`); the
// COUNTER targeting a row/edge's CID lands via a DISTINCT peer's
// `build_verifiable_peer_counter_record` + `peer add` + `peer pull` (so its
// `counters` reference lands in `peer_claim_references` with
// `referenced_cid == <target cid>`, the peer arm of the UNION-ALL). NO
// hand-inserted store rows. The presence read is LOCAL (DB-index only); NO network
// seam on any of these three routes (offline by construction, I-CF-5).
//
// Layer placement (Mandate 9/11): every slice-13 scenario is a layer-3/layer-5
// subprocess + real-I/O test — EXAMPLE-only. The sad/edge paths (none-countered,
// multi-counter, mixed survey) are enumerated explicitly, never PBT-generated at
// this layer. The strict 1-query N+1 bound is a DELIVER `adapter-duckdb`
// unit/property assertion (REUSED slice-12 read); at this subprocess AT layer the
// N+1 guard is asserted via its behavioral proxy (a survey of MANY edges across
// MANY groups flags the countered subset correctly in ONE request).
//
// Mandate 7 RED scaffolds (ADR-025): every seed + assert below is `todo!()` —
// it COMPILES now (signatures resolve so the AT files build), then PANICS at
// runtime → classifies RED (MISSING_FUNCTIONALITY), NOT BROKEN. They stay RED
// until DELIVER's per-scenario RED→GREEN→COMMIT cycles.
// =============================================================================

/// The handle a `/peer-claims` flag seed returns: every peer-claim CID on the page
/// (in the slice-06 `/peer-claims` rendered order), split into the COUNTERED subset
/// (rows that MUST carry the "Countered" marker linking to `/claims/{cid}`) and the
/// UN-COUNTERED subset (rows that MUST carry NO marker). Mirrors [`SeededClaimsList`]
/// for the federated surface.
#[derive(Debug, Clone)]
pub struct SeededPeerClaimsList {
    /// Every peer-claim CID on the page, in the slice-06 `/peer-claims` rendered
    /// order. The no-regression gold pins this order byte-identical with the flag.
    pub ordered_cids: Vec<String>,
    /// The subset of `ordered_cids` whose claim has >= 1 counter — each row MUST carry
    /// the neutral "Countered" marker linking to `/claims/{cid}` (US-CF-002).
    pub countered_cids: Vec<String>,
    /// The subset of `ordered_cids` with NO counter — each row MUST carry NO marker
    /// and no "0 counters" noise (US-CF-002 no-noise).
    pub uncountered_cids: Vec<String>,
    /// The peer DID whose claims populate the list (the row-origin column must keep
    /// showing this DID verbatim beside any flag — US-CF-002 origin-unchanged AC).
    pub peer_did: String,
}

/// The handle a traversal-survey flag seed returns (`/project` or `/philosophy`):
/// every EDGE CID across the WHOLE survey (the UNION of every `EdgeRow.cid` across
/// every `EdgeGroup`, the flatten point of ADR-050), split into the COUNTERED subset
/// (edges that MUST carry the marker in their UNCHANGED group + position) and the
/// UN-COUNTERED subset (edges that MUST render exactly as slice-10). `entity` is the
/// traversal target the scenario queries (`?subject=` for `/project`, `?object=` for
/// `/philosophy`).
#[derive(Debug, Clone)]
pub struct SeededSurveyEdges {
    /// The traversal target the scenario queries (`subject` for `/project`,
    /// `object` for `/philosophy`).
    pub entity: String,
    /// Every edge CID across the whole survey, in the slice-10 rendered (grouped)
    /// order. The no-regression gold pins grouping/edge-order byte-identical with the
    /// flag (I-CF-9).
    pub ordered_cids: Vec<String>,
    /// The subset of `ordered_cids` whose claim has >= 1 counter — each edge MUST carry
    /// the neutral "Countered" marker in its UNCHANGED group + position (US-CF-003).
    pub countered_cids: Vec<String>,
    /// The subset of `ordered_cids` with NO counter — each edge MUST render exactly as
    /// slice-10 (no marker, no noise; US-CF-003).
    pub uncountered_cids: Vec<String>,
}

/// Read every PEER-claim CID from the env's REAL `peer_claims` table in the EXACT
/// slice-06 `/peer-claims` list render order (mirrors the production
/// `list_peer_claims` ordering). The slice-13 `/peer-claims` flag seeds return their
/// CIDs in this order so a scenario can address rows by their rendered position + the
/// no-regression gold can pin order byte-identity. Read-only; opens a SECOND
/// short-lived connection. The sibling of [`read_own_claim_cids_in_list_order`] for
/// the FEDERATED surface.
///
/// SCAFFOLD: true (slice-13).
pub fn read_peer_claim_cids_in_list_order(env: &TestEnv) -> Vec<String> {
    let db_path = env.duckdb_path();
    let conn = duckdb::Connection::open(&db_path).unwrap_or_else(|err| {
        panic!(
            "open DuckDB at {} for peer_claims list-order cid read: {err}",
            db_path.display()
        )
    });
    // Mirror the production `DuckDbStoreReadAdapter::list_peer_claims` order
    // (`composed_at DESC, cid`) so the returned CIDs match the rendered row order.
    let mut stmt = conn
        .prepare("SELECT cid FROM peer_claims ORDER BY composed_at DESC, cid")
        .unwrap_or_else(|err| panic!("prepare peer_claims list-order cid read: {err}"));
    let rows = stmt
        .query_map([], |row| row.get::<_, String>(0))
        .unwrap_or_else(|err| panic!("query peer_claims cids in list order: {err}"));
    rows.map(|r| r.expect("decode peer_claims cid")).collect()
}

/// Read every EDGE CID a traversal survey is built from, in the slice-10 grouped
/// render order — the UNION of every edge's claim CID across every `EdgeGroup` (the
/// `subject`-survey for `/project`, the `object`-survey for `/philosophy`). Reads the
/// SAME `claims` U `peer_claims` rows the production `query_project_survey` /
/// `query_philosophy_survey` return, in the SAME order, so the slice-13 traversal flag
/// seeds can return the EXACT edge CID set + order (ADR-050: the survey rows ARE the
/// flat union of every future-group edge). `dimension` selects the survey: `"project"`
/// keys on `subject == entity`, `"philosophy"` keys on `object == entity`. Read-only.
///
/// SCAFFOLD: true (slice-13).
pub fn read_survey_edge_cids_in_render_order(
    env: &TestEnv,
    dimension: &str,
    entity: &str,
) -> Vec<String> {
    // Mirror the production `query_survey` engine (adapter-duckdb): own `claims` UNION
    // ALL local `peer_claims`, filtered by the survey dimension, ORDERED by the OTHER
    // dimension then `source_table, cid` — the SAME flat union + order the viewer's
    // `query_project_survey` / `query_philosophy_survey` return and `group_by` then
    // groups in render order (ADR-050: the survey rows ARE the flat union of every
    // future-group edge). `"project"` keys on `subject == entity` (groups by `object`);
    // `"philosophy"` keys on `object == entity` (groups by `subject`).
    let (filter_col, order_col) = match dimension {
        "project" => ("subject", "object"),
        "philosophy" => ("object", "subject"),
        other => panic!(
            "read_survey_edge_cids_in_render_order: dimension must be \"project\" or \
             \"philosophy\", got {other:?}"
        ),
    };
    let db_path = env.duckdb_path();
    let conn = duckdb::Connection::open(&db_path).unwrap_or_else(|err| {
        panic!(
            "open DuckDB at {} for survey edge-cid read: {err}",
            db_path.display()
        )
    });
    let sql = format!(
        "SELECT cid FROM ( \
           SELECT c.cid AS cid, c.{order_col} AS order_col, 'Own' AS source_table \
           FROM claims c WHERE c.{filter_col} = ? \
           UNION ALL \
           SELECT pc.cid AS cid, pc.{order_col} AS order_col, 'Peer' AS source_table \
           FROM peer_claims pc WHERE pc.{filter_col} = ? \
         ) ORDER BY order_col, source_table, cid"
    );
    let mut stmt = conn
        .prepare(&sql)
        .unwrap_or_else(|err| panic!("prepare survey edge-cid read: {err}"));
    let rows = stmt
        .query_map(duckdb::params![entity, entity], |row| {
            row.get::<_, String>(0)
        })
        .unwrap_or_else(|err| panic!("query survey edge cids for {entity}: {err}"));
    rows.map(|r| r.expect("decode survey edge cid")).collect()
}

/// Seed a FEDERATED `/peer-claims` page with EXACTLY ONE countered peer claim among
/// several un-countered peer claims (the CF-1 walking-skeleton + CF-2/CF-4/CF-INV-*
/// fixture). The countered peer claim is countered by a DISTINCT peer (Tobias) — so
/// the presence read exercises the `peer_claim_references` arm of the UNION-ALL —
/// pulled via the PRODUCTION `peer add` + `peer pull` federation path; the rest are
/// plain pulled peer claims. ALL rows land in the SAME local store `openlore ui` reads
/// (Pillar 3 / BR-VIEW-4). Returns the [`SeededPeerClaimsList`] so the scenario
/// addresses the exact countered + un-countered rows + the peer-origin DID.
///
/// SCAFFOLD: true (slice-13) — DELIVER materializes it by: (1) pulling several plain
/// peer claims from a surveyed peer (REUSE `seed_peer_claims_via_pull`) whose rows land
/// in `peer_claims`; (2) recovering ONE of those peer-claim CIDs as the counter target;
/// (3) a DISTINCT peer (Tobias) authoring a verifiable `counters`-referencing record
/// targeting that CID (`build_verifiable_peer_counter_record` + `peer add` +
/// `peer pull`) so it lands in `peer_claim_references` with `referenced_cid ==
/// target_cid`; (4) recovering every peer-claim CID in the slice-06 list order
/// (`read_peer_claim_cids_in_list_order`) and splitting the ONE countered + the rest.
pub fn seed_peer_claims_one_countered(env: &TestEnv) -> SeededPeerClaimsList {
    // STEP 1 — build BOTH peers' verifiable wire records UP FRONT, holding each `PeerPds`
    // ALIVE for the whole function so a SINGLE `peer pull` over BOTH peers succeeds (pulling
    // one peer at a time would leave the other's now-dropped PDS unreachable and fail the
    // pull). Rachel is the SURVEYED peer hosting several plain claims (the `/peer-claims`
    // rows); Tobias is the DISTINCT peer hosting a COUNTER referencing ONE of Rachel's
    // claims. Rachel's target CID is DETERMINISTIC (the pull pipeline recomputes the SAME
    // CID the builder computes), so Tobias's counter can reference it before either is
    // pulled.
    let surveyed_peer = COUNTER_TARGET_AUTHOR_RACHEL;
    let rachel_seed = [7u8; 32];
    let (rachel_records, rachel_pubkey_hex) = build_verifiable_peer_records_for_triples(
        surveyed_peer,
        rachel_seed,
        &[
            (
                "github:peer/rachel-axum",
                "org.openlore.philosophy.ergonomics",
                0.70,
            ),
            (
                "github:peer/rachel-tokio",
                "org.openlore.philosophy.async-runtime",
                0.70,
            ),
            (
                "github:peer/rachel-serde",
                "org.openlore.philosophy.zero-copy",
                0.70,
            ),
        ],
    );
    // Counter Rachel's FIRST surveyed claim — its deterministic CID is the counter target.
    let target_cid = rachel_records
        .first()
        .expect("seed_peer_claims_one_countered: Rachel's first surveyed record")
        .rkey
        .clone();

    let tobias_seed = [9u8; 32];
    let (tobias_record, tobias_pubkey_hex) = build_verifiable_peer_counter_record(
        COUNTER_AUTHOR_TOBIAS,
        tobias_seed,
        &target_cid,
        Some(COUNTER_PEER_REASON_VERBATIM),
    );

    let rachel_pds = PeerPds::for_peer(surveyed_peer, rachel_records);
    let tobias_pds = PeerPds::for_peer(COUNTER_AUTHOR_TOBIAS, vec![tobias_record]);

    // STEP 2 — subscribe to BOTH peers via the real `peer add` verb (resolver wired per
    // peer), then `peer pull` BOTH in ONE invocation while both PDS are alive. The
    // production pull pipeline verifies each record, recomputes its CID, and writes
    // `peer_claims` (+ `peer_claim_references` for Tobias's `counters` reference, landing
    // with `referenced_cid == target_cid` — the peer arm of the UNION-ALL the presence
    // read exercises).
    for (did, pds) in [
        (surveyed_peer, &rachel_pds),
        (COUNTER_AUTHOR_TOBIAS, &tobias_pds),
    ] {
        let added = run_openlore_with_peer_resolver(
            env,
            &["peer", "add", did],
            did,
            pds.endpoint_url(),
        );
        assert_eq!(
            added.status, 0,
            "seed_peer_claims_one_countered: peer add for {did} must succeed;\n\
             --- stdout ---\n{}\n--- stderr ---\n{}",
            added.stdout, added.stderr
        );
    }
    let pulled = run_openlore_pull_multi(
        env,
        &["peer", "pull"],
        &[
            PeerSeam {
                peer_did: surveyed_peer,
                peer_endpoint: rachel_pds.endpoint_url(),
                peer_pubkey_hex: &rachel_pubkey_hex,
            },
            PeerSeam {
                peer_did: COUNTER_AUTHOR_TOBIAS,
                peer_endpoint: tobias_pds.endpoint_url(),
                peer_pubkey_hex: &tobias_pubkey_hex,
            },
        ],
    );
    assert_eq!(
        pulled.status, 0,
        "seed_peer_claims_one_countered: peer pull must succeed;\n\
         --- stdout ---\n{}\n--- stderr ---\n{}",
        pulled.stdout, pulled.stderr
    );

    // STEP 3 — recover the SURVEYED peer's claim CIDs; split the ONE countered target +
    // the rest un-countered. (The un-countered set is Rachel's OTHER claims — NOT Tobias's
    // counter row, which is the disagreement itself, not one of the rows under scrutiny.)
    let surveyed_cids = read_peer_claim_cids_for(env, surveyed_peer);
    assert!(
        surveyed_cids.contains(&target_cid),
        "seed_peer_claims_one_countered: the countered target CID {target_cid:?} must be \
         among the surveyed peer's claims; got {surveyed_cids:?}"
    );
    let uncountered_cids = surveyed_cids
        .iter()
        .filter(|cid| *cid != &target_cid)
        .cloned()
        .collect::<Vec<_>>();

    // Every peer-claim CID in the slice-06 `/peer-claims` render order (for the
    // no-regression gold). Includes Tobias's counter row + Rachel's rows.
    let ordered_cids = read_peer_claim_cids_in_list_order(env);

    SeededPeerClaimsList {
        ordered_cids,
        countered_cids: vec![target_cid],
        uncountered_cids,
        peer_did: surveyed_peer.to_string(),
    }
}

/// Seed a FEDERATED `/peer-claims` page where ONE peer claim is countered by TWO
/// DISTINCT authors (the CF presence-only GOLD fixture; I-CF-3 / KPI-AV-2). Two
/// distinct peers each author a verifiable `counters`-referencing record targeting the
/// SAME peer-claim CID, delivered through the PRODUCTION `peer add` + `peer pull`
/// federation path, so BOTH land in `peer_claim_references` with the SAME
/// `referenced_cid`. The `counter_presence_for` UNION-ALL DISTINCT collapses the two
/// distinct-author counters of the SAME CID to ONE presence membership → the row
/// carries EXACTLY ONE neutral "Countered" marker (never "disputed by 2"). Returns the
/// [`SeededPeerClaimsList`] whose single `countered_cids` entry is the twice-countered
/// peer row.
///
/// SCAFFOLD: true (slice-13) — adapts `seed_claims_list_target_two_counters_distinct_
/// authors` to the FEDERATED surface (the target is a PEER claim on `/peer-claims`).
pub fn seed_peer_claims_target_two_counters_distinct_authors(
    env: &TestEnv,
) -> SeededPeerClaimsList {
    // STEP 1 — build the SURVEYED peer's (Rachel's) plain claims + BOTH distinct COUNTER
    // authors' verifiable records UP FRONT, holding every `PeerPds` ALIVE for the whole
    // function so a SINGLE `peer pull` over all three peers succeeds. Rachel hosts the
    // `/peer-claims` rows; Tobias and Maria are the TWO DISTINCT counter authors, each
    // referencing the SAME Rachel-claim CID (the twice-countered target). Rachel's target
    // CID is DETERMINISTIC (the pull pipeline recomputes the SAME CID the builder computes),
    // so both counters can reference it before any record is pulled.
    let surveyed_peer = COUNTER_TARGET_AUTHOR_RACHEL;
    let rachel_seed = [7u8; 32];
    let (rachel_records, rachel_pubkey_hex) = build_verifiable_peer_records_for_triples(
        surveyed_peer,
        rachel_seed,
        &[
            (
                "github:peer/rachel-axum",
                "org.openlore.philosophy.ergonomics",
                0.70,
            ),
            (
                "github:peer/rachel-tokio",
                "org.openlore.philosophy.async-runtime",
                0.70,
            ),
            (
                "github:peer/rachel-serde",
                "org.openlore.philosophy.zero-copy",
                0.70,
            ),
        ],
    );
    // Counter Rachel's FIRST surveyed claim — its deterministic CID is the shared target of
    // BOTH distinct-author counters.
    let target_cid = rachel_records
        .first()
        .expect("seed_peer_claims_target_two_counters_distinct_authors: Rachel's first record")
        .rkey
        .clone();

    // TWO DISTINCT counter authors (Tobias + Maria), each authoring a verifiable
    // `counters`-referencing record targeting the SAME `target_cid` (ADR-015). Both land in
    // `peer_claim_references` with the SAME `referenced_cid` — two DISTINCT authors, one
    // referenced CID. The `counter_presence_for` UNION-ALL DISTINCT collapses them to ONE
    // presence membership → ONE flag.
    let tobias_seed = [9u8; 32];
    let (tobias_record, tobias_pubkey_hex) = build_verifiable_peer_counter_record(
        COUNTER_AUTHOR_TOBIAS,
        tobias_seed,
        &target_cid,
        Some(COUNTER_PEER_REASON_VERBATIM),
    );
    let maria_did = "did:plc:maria-test";
    let maria_seed = [11u8; 32];
    let (maria_record, maria_pubkey_hex) = build_verifiable_peer_counter_record(
        maria_did,
        maria_seed,
        &target_cid,
        Some(COUNTER_REASON_VERBATIM),
    );

    let rachel_pds = PeerPds::for_peer(surveyed_peer, rachel_records);
    let tobias_pds = PeerPds::for_peer(COUNTER_AUTHOR_TOBIAS, vec![tobias_record]);
    let maria_pds = PeerPds::for_peer(maria_did, vec![maria_record]);

    // STEP 2 — subscribe to ALL THREE peers via the real `peer add` verb (resolver wired per
    // peer), then `peer pull` ALL in ONE invocation while every PDS is alive. The production
    // pull pipeline verifies each record, recomputes its CID, and writes `peer_claims` (+
    // `peer_claim_references` for BOTH counters, each landing with
    // `referenced_cid == target_cid`).
    for (did, pds) in [
        (surveyed_peer, &rachel_pds),
        (COUNTER_AUTHOR_TOBIAS, &tobias_pds),
        (maria_did, &maria_pds),
    ] {
        let added = run_openlore_with_peer_resolver(
            env,
            &["peer", "add", did],
            did,
            pds.endpoint_url(),
        );
        assert_eq!(
            added.status, 0,
            "seed_peer_claims_target_two_counters_distinct_authors: peer add for {did} must \
             succeed;\n--- stdout ---\n{}\n--- stderr ---\n{}",
            added.stdout, added.stderr
        );
    }
    let pulled = run_openlore_pull_multi(
        env,
        &["peer", "pull"],
        &[
            PeerSeam {
                peer_did: surveyed_peer,
                peer_endpoint: rachel_pds.endpoint_url(),
                peer_pubkey_hex: &rachel_pubkey_hex,
            },
            PeerSeam {
                peer_did: COUNTER_AUTHOR_TOBIAS,
                peer_endpoint: tobias_pds.endpoint_url(),
                peer_pubkey_hex: &tobias_pubkey_hex,
            },
            PeerSeam {
                peer_did: maria_did,
                peer_endpoint: maria_pds.endpoint_url(),
                peer_pubkey_hex: &maria_pubkey_hex,
            },
        ],
    );
    assert_eq!(
        pulled.status, 0,
        "seed_peer_claims_target_two_counters_distinct_authors: peer pull must succeed;\n\
         --- stdout ---\n{}\n--- stderr ---\n{}",
        pulled.stdout, pulled.stderr
    );

    // STEP 3 — recover the SURVEYED peer's claim CIDs; split the ONE twice-countered target +
    // the rest un-countered. (The two counters live under Tobias + Maria in `peer_claims`,
    // NOT under Rachel — so the surveyed set is exactly Rachel's three rows.)
    let surveyed_cids = read_peer_claim_cids_for(env, surveyed_peer);
    assert!(
        surveyed_cids.contains(&target_cid),
        "seed_peer_claims_target_two_counters_distinct_authors: the twice-countered target \
         CID {target_cid:?} must be among the surveyed peer's claims; got {surveyed_cids:?}"
    );
    let uncountered_cids = surveyed_cids
        .iter()
        .filter(|cid| *cid != &target_cid)
        .cloned()
        .collect::<Vec<_>>();

    // Every peer-claim CID in the slice-06 `/peer-claims` render order (for the
    // no-regression gold). Includes both counter rows + Rachel's rows.
    let ordered_cids = read_peer_claim_cids_in_list_order(env);

    SeededPeerClaimsList {
        ordered_cids,
        countered_cids: vec![target_cid],
        uncountered_cids,
        peer_did: surveyed_peer.to_string(),
    }
}

/// Seed a FEDERATED `/peer-claims` page with NO counters at all (the CF no-noise
/// fixture): several plain pulled peer claims, NOTHING references any of them as a
/// counter, so `counter_presence_for` returns the EMPTY set and the list renders
/// byte-identically to slice-06. Returns the [`SeededPeerClaimsList`] with an EMPTY
/// `countered_cids` (every CID is in `uncountered_cids`).
///
/// SCAFFOLD: true (slice-13).
pub fn seed_peer_claims_none_countered(env: &TestEnv) -> SeededPeerClaimsList {
    // Pull several PLAIN peer claims from ONE surveyed peer (Rachel) via the PRODUCTION
    // `peer add` + `peer pull` federation path — NOTHING counters any of them, so
    // `counter_presence_for` returns the EMPTY set and the list renders byte-identically
    // to slice-06. NO counter is authored, NO second peer is added/pulled.
    let surveyed_peer = COUNTER_TARGET_AUTHOR_RACHEL;
    let rachel_seed = [7u8; 32];
    let (rachel_records, rachel_pubkey_hex) = build_verifiable_peer_records_for_triples(
        surveyed_peer,
        rachel_seed,
        &[
            (
                "github:peer/rachel-axum",
                "org.openlore.philosophy.ergonomics",
                0.70,
            ),
            (
                "github:peer/rachel-tokio",
                "org.openlore.philosophy.async-runtime",
                0.70,
            ),
            (
                "github:peer/rachel-serde",
                "org.openlore.philosophy.zero-copy",
                0.70,
            ),
        ],
    );

    let rachel_pds = PeerPds::for_peer(surveyed_peer, rachel_records);

    let added = run_openlore_with_peer_resolver(
        env,
        &["peer", "add", surveyed_peer],
        surveyed_peer,
        rachel_pds.endpoint_url(),
    );
    assert_eq!(
        added.status, 0,
        "seed_peer_claims_none_countered: peer add for {surveyed_peer} must succeed;\n\
         --- stdout ---\n{}\n--- stderr ---\n{}",
        added.stdout, added.stderr
    );
    let pulled = run_openlore_pull_multi(
        env,
        &["peer", "pull"],
        &[PeerSeam {
            peer_did: surveyed_peer,
            peer_endpoint: rachel_pds.endpoint_url(),
            peer_pubkey_hex: &rachel_pubkey_hex,
        }],
    );
    assert_eq!(
        pulled.status, 0,
        "seed_peer_claims_none_countered: peer pull must succeed;\n\
         --- stdout ---\n{}\n--- stderr ---\n{}",
        pulled.stdout, pulled.stderr
    );

    // Recover every peer-claim CID in the slice-06 `/peer-claims` render order. With no
    // counter authored, EVERY row is un-countered: countered_cids is EMPTY and every CID
    // lands in uncountered_cids.
    let ordered_cids = read_peer_claim_cids_in_list_order(env);
    let uncountered_cids = ordered_cids.clone();

    SeededPeerClaimsList {
        ordered_cids,
        countered_cids: Vec::new(),
        uncountered_cids,
        peer_did: surveyed_peer.to_string(),
    }
}

/// Seed a `/project?subject=<entity>` survey with EXACTLY ONE countered edge among
/// several un-countered edges spread across the survey's groups (the CF-3 walking
/// fixture for the EDGE surface). The surveyed edges are authored by Rachel via
/// `seed_project_survey_trail` (REUSED, landing in `peer_claims`); ONE edge's claim CID
/// is then countered by a DISTINCT peer (Tobias) via `build_verifiable_peer_counter_
/// record` + `peer add` + `peer pull` (landing in `peer_claim_references` with
/// `referenced_cid == that edge's cid`). ALL rows land in the SAME local store
/// `openlore ui` reads (Pillar 3 / I-GT-2). Returns the [`SeededSurveyEdges`] so the
/// scenario addresses the exact flagged edge in its unchanged group/position.
///
/// SCAFFOLD: true (slice-13) — DELIVER materializes it by: (1) seeding the project
/// survey (REUSE `seed_project_survey_trail` so >= 3 edges across groups land in
/// `peer_claims`); (2) recovering the survey's edge CIDs in render order
/// (`read_survey_edge_cids_in_render_order(env, "project", entity)`); (3) a DISTINCT
/// peer countering ONE of those edge CIDs; (4) splitting the ONE countered + the rest.
pub fn seed_project_survey_one_edge_countered(env: &TestEnv) -> SeededSurveyEdges {
    // STEP 1 — build BOTH peers' verifiable wire records UP FRONT, holding each `PeerPds`
    // ALIVE for the whole function so a SINGLE `peer pull` over BOTH peers succeeds (a
    // second pull after the first peer is already subscribed would re-resolve that peer
    // with no resolver wired and fail). Rachel is the SURVEYED contributor asserting the
    // SHARED project subject across THREE DISTINCT philosophies (so three
    // philosophies-embodied edges across three groups land in `peer_claims` — the LOCAL
    // `/project` survey rows, Pillar 3 / I-GT-2). Tobias is the DISTINCT peer hosting a
    // COUNTER referencing ONE of Rachel's edge claims. Rachel's target CID is DETERMINISTIC
    // (the pull pipeline recomputes the SAME CID the builder computes), so Tobias's counter
    // references it before either is pulled.
    let surveyed_project = "github:peer/rachel-cargo";
    let surveyed_author = COUNTER_TARGET_AUTHOR_RACHEL;
    let rachel_seed = [41u8; 32];
    let (rachel_records, rachel_pubkey_hex) = build_verifiable_peer_records_for_triples(
        surveyed_author,
        rachel_seed,
        &[
            (surveyed_project, TRAVERSAL_PHILOSOPHY_DEP_PINNING, 0.90),
            (
                surveyed_project,
                "org.openlore.philosophy.reproducible-builds",
                0.74,
            ),
            (
                surveyed_project,
                "org.openlore.philosophy.memory-safety",
                0.25,
            ),
        ],
    );
    // Counter Rachel's FIRST surveyed edge — its deterministic CID is the counter target.
    let target_cid = rachel_records
        .first()
        .expect("seed_project_survey_one_edge_countered: Rachel's first surveyed record")
        .rkey
        .clone();

    let tobias_seed = [9u8; 32];
    let (tobias_record, tobias_pubkey_hex) = build_verifiable_peer_counter_record(
        COUNTER_AUTHOR_TOBIAS,
        tobias_seed,
        &target_cid,
        Some(COUNTER_PEER_REASON_VERBATIM),
    );

    let rachel_pds = PeerPds::for_peer(surveyed_author, rachel_records);
    let tobias_pds = PeerPds::for_peer(COUNTER_AUTHOR_TOBIAS, vec![tobias_record]);

    // STEP 2 — subscribe to BOTH peers via the real `peer add` verb (resolver wired per
    // peer), then `peer pull` BOTH in ONE invocation while both PDS are alive. The
    // production pull pipeline verifies each record, recomputes its CID, and writes
    // `peer_claims` (+ `peer_claim_references` for Tobias's `counters` reference, landing
    // with `referenced_cid == target_cid` — the peer arm of the UNION-ALL the presence read
    // exercises).
    for (did, pds) in [
        (surveyed_author, &rachel_pds),
        (COUNTER_AUTHOR_TOBIAS, &tobias_pds),
    ] {
        let added = run_openlore_with_peer_resolver(
            env,
            &["peer", "add", did],
            did,
            pds.endpoint_url(),
        );
        assert_eq!(
            added.status, 0,
            "seed_project_survey_one_edge_countered: peer add for {did} must succeed;\n\
             --- stdout ---\n{}\n--- stderr ---\n{}",
            added.stdout, added.stderr
        );
    }
    let pulled = run_openlore_pull_multi(
        env,
        &["peer", "pull"],
        &[
            PeerSeam {
                peer_did: surveyed_author,
                peer_endpoint: rachel_pds.endpoint_url(),
                peer_pubkey_hex: &rachel_pubkey_hex,
            },
            PeerSeam {
                peer_did: COUNTER_AUTHOR_TOBIAS,
                peer_endpoint: tobias_pds.endpoint_url(),
                peer_pubkey_hex: &tobias_pubkey_hex,
            },
        ],
    );
    assert_eq!(
        pulled.status, 0,
        "seed_project_survey_one_edge_countered: peer pull must succeed;\n\
         --- stdout ---\n{}\n--- stderr ---\n{}",
        pulled.stdout, pulled.stderr
    );

    // STEP 3 — recover the survey's edge CIDs in the slice-10 grouped render order (the
    // flat union of every EdgeRow.cid across every EdgeGroup — ADR-050's flatten point),
    // then split the ONE countered edge + the rest un-countered. (Tobias's counter row
    // carries a DIFFERENT subject, so it never appears in THIS project's survey.)
    let ordered_cids = read_survey_edge_cids_in_render_order(env, "project", surveyed_project);
    assert!(
        ordered_cids.contains(&target_cid),
        "seed_project_survey_one_edge_countered: the countered target CID {target_cid:?} \
         must be among the surveyed project's edges; got {ordered_cids:?}"
    );
    let uncountered_cids = ordered_cids
        .iter()
        .filter(|cid| *cid != &target_cid)
        .cloned()
        .collect::<Vec<_>>();

    SeededSurveyEdges {
        entity: surveyed_project.to_string(),
        ordered_cids,
        countered_cids: vec![target_cid],
        uncountered_cids,
    }
}

/// Seed a `/philosophy?object=<entity>` survey with a KNOWN countered subset across
/// several groups (the CF-3 SYMMETRIC fixture for the EDGE surface). The SYMMETRIC
/// mirror of [`seed_project_survey_one_edge_countered`], swapping subject<->object: the
/// surveyed edges are authored by Rachel via `seed_philosophy_survey_trail` (REUSED,
/// landing in `peer_claims`); a KNOWN subset of edge claim CIDs is then countered by a
/// DISTINCT peer (Tobias) via the federation path (landing in `peer_claim_references`).
/// Returns the [`SeededSurveyEdges`].
///
/// SCAFFOLD: true (slice-13).
pub fn seed_philosophy_survey_one_edge_countered(env: &TestEnv) -> SeededSurveyEdges {
    // The SYMMETRIC mirror of `seed_project_survey_one_edge_countered`, swapping
    // subject↔object: Rachel embodies the SHARED philosophy OBJECT across THREE DISTINCT
    // subjects (projects), so three projects-that-embody edges across the philosophy
    // survey's groups land in `peer_claims` (the LOCAL `/philosophy` survey rows, Pillar 3
    // / I-GT-2). Tobias is the DISTINCT peer hosting a COUNTER referencing ONE of Rachel's
    // edge claims. Rachel's target CID is DETERMINISTIC (the pull pipeline recomputes the
    // SAME CID the builder computes), so Tobias's counter references it before either is
    // pulled. Build BOTH peers' verifiable wire records UP FRONT, holding each `PeerPds`
    // ALIVE for the whole function so a SINGLE `peer pull` over BOTH peers succeeds.
    let surveyed_philosophy = TRAVERSAL_PHILOSOPHY_DEP_PINNING;
    let surveyed_author = COUNTER_TARGET_AUTHOR_RACHEL;
    let rachel_seed = [42u8; 32];
    let (rachel_records, rachel_pubkey_hex) = build_verifiable_peer_records_for_triples(
        surveyed_author,
        rachel_seed,
        &[
            (TRAVERSAL_PROJECT_NIXPKGS, surveyed_philosophy, 0.92),
            (TRAVERSAL_PROJECT_BAZEL, surveyed_philosophy, 0.85),
            ("github:rust-lang/cargo", surveyed_philosophy, 0.74),
        ],
    );
    // Counter Rachel's FIRST surveyed edge — its deterministic CID is the counter target.
    let target_cid = rachel_records
        .first()
        .expect("seed_philosophy_survey_one_edge_countered: Rachel's first surveyed record")
        .rkey
        .clone();

    let tobias_seed = [9u8; 32];
    let (tobias_record, tobias_pubkey_hex) = build_verifiable_peer_counter_record(
        COUNTER_AUTHOR_TOBIAS,
        tobias_seed,
        &target_cid,
        Some(COUNTER_PEER_REASON_VERBATIM),
    );

    let rachel_pds = PeerPds::for_peer(surveyed_author, rachel_records);
    let tobias_pds = PeerPds::for_peer(COUNTER_AUTHOR_TOBIAS, vec![tobias_record]);

    for (did, pds) in [
        (surveyed_author, &rachel_pds),
        (COUNTER_AUTHOR_TOBIAS, &tobias_pds),
    ] {
        let added = run_openlore_with_peer_resolver(
            env,
            &["peer", "add", did],
            did,
            pds.endpoint_url(),
        );
        assert_eq!(
            added.status, 0,
            "seed_philosophy_survey_one_edge_countered: peer add for {did} must succeed;\n\
             --- stdout ---\n{}\n--- stderr ---\n{}",
            added.stdout, added.stderr
        );
    }
    let pulled = run_openlore_pull_multi(
        env,
        &["peer", "pull"],
        &[
            PeerSeam {
                peer_did: surveyed_author,
                peer_endpoint: rachel_pds.endpoint_url(),
                peer_pubkey_hex: &rachel_pubkey_hex,
            },
            PeerSeam {
                peer_did: COUNTER_AUTHOR_TOBIAS,
                peer_endpoint: tobias_pds.endpoint_url(),
                peer_pubkey_hex: &tobias_pubkey_hex,
            },
        ],
    );
    assert_eq!(
        pulled.status, 0,
        "seed_philosophy_survey_one_edge_countered: peer pull must succeed;\n\
         --- stdout ---\n{}\n--- stderr ---\n{}",
        pulled.stdout, pulled.stderr
    );

    // Recover the survey's edge CIDs in the slice-10 grouped render order (the flat union of
    // every EdgeRow.cid across every EdgeGroup — ADR-050's flatten point), then split the
    // ONE countered edge + the rest un-countered. (Tobias's counter row carries a DIFFERENT
    // object, so it never appears in THIS philosophy's survey.)
    let ordered_cids =
        read_survey_edge_cids_in_render_order(env, "philosophy", surveyed_philosophy);
    assert!(
        ordered_cids.contains(&target_cid),
        "seed_philosophy_survey_one_edge_countered: the countered target CID {target_cid:?} \
         must be among the surveyed philosophy's edges; got {ordered_cids:?}"
    );
    let uncountered_cids = ordered_cids
        .iter()
        .filter(|cid| *cid != &target_cid)
        .cloned()
        .collect::<Vec<_>>();

    SeededSurveyEdges {
        entity: surveyed_philosophy.to_string(),
        ordered_cids,
        countered_cids: vec![target_cid],
        uncountered_cids,
    }
}

/// Seed a `/project?subject=<entity>` survey where ONE edge is countered by TWO
/// DISTINCT authors (the EDGE presence-only GOLD fixture; I-CF-3 / KPI-GRAPH-2). Two
/// distinct peers each author a verifiable `counters`-referencing record targeting the
/// SAME edge claim CID via a single `peer pull`, so BOTH land in
/// `peer_claim_references` with the SAME `referenced_cid` → the DISTINCT read collapses
/// them to ONE presence membership → the edge carries EXACTLY ONE neutral marker in its
/// unchanged group/position (never "disputed by 2"). Returns the [`SeededSurveyEdges`]
/// whose single `countered_cids` entry is the twice-countered edge.
///
/// SCAFFOLD: true (slice-13).
pub fn seed_project_survey_edge_two_counters_distinct_authors(
    env: &TestEnv,
) -> SeededSurveyEdges {
    // Rachel is the SURVEYED contributor asserting the SHARED project subject across THREE
    // DISTINCT philosophies (three edges across three groups land in `peer_claims` — the
    // LOCAL `/project` survey rows, Pillar 3 / I-GT-2). TWO DISTINCT peers (Tobias + Uli)
    // each author a verifiable COUNTER referencing the SAME edge claim CID (Rachel's first
    // edge), so BOTH land in `peer_claim_references` with the SAME `referenced_cid` → the
    // DISTINCT read collapses them to ONE presence membership → the edge carries EXACTLY ONE
    // neutral marker (never "disputed by 2"). Build ALL THREE peers' records UP FRONT,
    // holding each `PeerPds` ALIVE so a SINGLE `peer pull` over all three succeeds.
    let surveyed_project = "github:peer/rachel-cargo";
    let surveyed_author = COUNTER_TARGET_AUTHOR_RACHEL;
    let rachel_seed = [41u8; 32];
    let (rachel_records, rachel_pubkey_hex) = build_verifiable_peer_records_for_triples(
        surveyed_author,
        rachel_seed,
        &[
            (surveyed_project, TRAVERSAL_PHILOSOPHY_DEP_PINNING, 0.90),
            (surveyed_project, TRAVERSAL_PHILOSOPHY_REPRO_BUILDS, 0.74),
            (surveyed_project, "org.openlore.philosophy.memory-safety", 0.25),
        ],
    );
    // BOTH counters target Rachel's FIRST surveyed edge — its deterministic CID.
    let target_cid = rachel_records
        .first()
        .expect("seed_project_survey_edge_two_counters_distinct_authors: Rachel's first record")
        .rkey
        .clone();

    // TWO DISTINCT counter authors, each referencing the SAME target CID (distinct reasons
    // so they are genuinely two separate signed records, collapsed only by DISTINCT
    // referenced_cid — never merged into a count).
    let second_counter_author = "did:plc:uli-test";
    let tobias_seed = [9u8; 32];
    let (tobias_record, tobias_pubkey_hex) = build_verifiable_peer_counter_record(
        COUNTER_AUTHOR_TOBIAS,
        tobias_seed,
        &target_cid,
        Some(COUNTER_PEER_REASON_VERBATIM),
    );
    let uli_seed = [11u8; 32];
    let (uli_record, uli_pubkey_hex) = build_verifiable_peer_counter_record(
        second_counter_author,
        uli_seed,
        &target_cid,
        Some(COUNTER_REASON_VERBATIM),
    );

    let rachel_pds = PeerPds::for_peer(surveyed_author, rachel_records);
    let tobias_pds = PeerPds::for_peer(COUNTER_AUTHOR_TOBIAS, vec![tobias_record]);
    let uli_pds = PeerPds::for_peer(second_counter_author, vec![uli_record]);

    for (did, pds) in [
        (surveyed_author, &rachel_pds),
        (COUNTER_AUTHOR_TOBIAS, &tobias_pds),
        (second_counter_author, &uli_pds),
    ] {
        let added = run_openlore_with_peer_resolver(
            env,
            &["peer", "add", did],
            did,
            pds.endpoint_url(),
        );
        assert_eq!(
            added.status, 0,
            "seed_project_survey_edge_two_counters_distinct_authors: peer add for {did} must \
             succeed;\n--- stdout ---\n{}\n--- stderr ---\n{}",
            added.stdout, added.stderr
        );
    }
    let pulled = run_openlore_pull_multi(
        env,
        &["peer", "pull"],
        &[
            PeerSeam {
                peer_did: surveyed_author,
                peer_endpoint: rachel_pds.endpoint_url(),
                peer_pubkey_hex: &rachel_pubkey_hex,
            },
            PeerSeam {
                peer_did: COUNTER_AUTHOR_TOBIAS,
                peer_endpoint: tobias_pds.endpoint_url(),
                peer_pubkey_hex: &tobias_pubkey_hex,
            },
            PeerSeam {
                peer_did: second_counter_author,
                peer_endpoint: uli_pds.endpoint_url(),
                peer_pubkey_hex: &uli_pubkey_hex,
            },
        ],
    );
    assert_eq!(
        pulled.status, 0,
        "seed_project_survey_edge_two_counters_distinct_authors: peer pull must succeed;\n\
         --- stdout ---\n{}\n--- stderr ---\n{}",
        pulled.stdout, pulled.stderr
    );

    // Recover the survey's edge CIDs in slice-10 render order, then split the ONE
    // twice-countered edge + the rest. (Both counters' rows carry a DIFFERENT subject, so
    // they never appear in THIS project's survey.)
    let ordered_cids = read_survey_edge_cids_in_render_order(env, "project", surveyed_project);
    assert!(
        ordered_cids.contains(&target_cid),
        "seed_project_survey_edge_two_counters_distinct_authors: the twice-countered target \
         CID {target_cid:?} must be among the surveyed project's edges; got {ordered_cids:?}"
    );
    let uncountered_cids = ordered_cids
        .iter()
        .filter(|cid| *cid != &target_cid)
        .cloned()
        .collect::<Vec<_>>();

    SeededSurveyEdges {
        entity: surveyed_project.to_string(),
        ordered_cids,
        countered_cids: vec![target_cid],
        uncountered_cids,
    }
}

/// Seed a survey with NO counters at all (the EDGE no-noise fixture): several plain
/// surveyed edges, NOTHING references any of them as a counter, so
/// `counter_presence_for` returns the EMPTY set and the survey renders byte-identically
/// to slice-10. Returns the [`SeededSurveyEdges`] with an EMPTY `countered_cids`.
/// `dimension` selects `"project"` (subject survey) or `"philosophy"` (object survey)
/// so the one helper serves the no-noise scenario on BOTH routes.
///
/// SCAFFOLD: true (slice-13).
pub fn seed_survey_none_countered(env: &TestEnv, dimension: &str) -> SeededSurveyEdges {
    // The no-noise mirror of `seed_{project,philosophy}_survey_one_edge_countered`: seed
    // ONLY Rachel's surveyed edges (three triples sharing the queried entity, so several
    // edges across the survey's groups land in `peer_claims` via the PRODUCTION peer add +
    // peer pull path, Pillar 3 / I-GT-2) — and author NO counter against ANY of them. With
    // nothing in `peer_claim_references` referencing these edge CIDs, the flattened
    // `counter_presence_for` over the survey's edges returns the EMPTY set, so no edge is
    // flagged and the survey renders byte-identically to slice-10 (US-CF-003 / I-CF-2).
    let surveyed_author = COUNTER_TARGET_AUTHOR_RACHEL;
    let rachel_seed = [43u8; 32];
    let (entity, triples): (&str, [(&str, &str, f64); 3]) = match dimension {
        "project" => {
            let surveyed_project = "github:peer/rachel-cargo";
            (
                surveyed_project,
                [
                    (surveyed_project, TRAVERSAL_PHILOSOPHY_DEP_PINNING, 0.90),
                    (
                        surveyed_project,
                        "org.openlore.philosophy.reproducible-builds",
                        0.74,
                    ),
                    (
                        surveyed_project,
                        "org.openlore.philosophy.memory-safety",
                        0.25,
                    ),
                ],
            )
        }
        "philosophy" => {
            let surveyed_philosophy = TRAVERSAL_PHILOSOPHY_DEP_PINNING;
            (
                surveyed_philosophy,
                [
                    (TRAVERSAL_PROJECT_NIXPKGS, surveyed_philosophy, 0.92),
                    (TRAVERSAL_PROJECT_BAZEL, surveyed_philosophy, 0.85),
                    ("github:rust-lang/cargo", surveyed_philosophy, 0.74),
                ],
            )
        }
        other => panic!(
            "seed_survey_none_countered: unknown dimension {other:?} (expected \
             \"project\" or \"philosophy\")"
        ),
    };

    let (rachel_records, rachel_pubkey_hex) =
        build_verifiable_peer_records_for_triples(surveyed_author, rachel_seed, &triples);
    let rachel_pds = PeerPds::for_peer(surveyed_author, rachel_records);

    // Subscribe to Rachel via the real `peer add` verb, then `peer pull` her surveyed
    // edges. NO second peer + NO counter record is seeded — the no-noise condition.
    let added = run_openlore_with_peer_resolver(
        env,
        &["peer", "add", surveyed_author],
        surveyed_author,
        rachel_pds.endpoint_url(),
    );
    assert_eq!(
        added.status, 0,
        "seed_survey_none_countered: peer add for {surveyed_author} must succeed;\n\
         --- stdout ---\n{}\n--- stderr ---\n{}",
        added.stdout, added.stderr
    );
    let pulled = run_openlore_pull_multi(
        env,
        &["peer", "pull"],
        &[PeerSeam {
            peer_did: surveyed_author,
            peer_endpoint: rachel_pds.endpoint_url(),
            peer_pubkey_hex: &rachel_pubkey_hex,
        }],
    );
    assert_eq!(
        pulled.status, 0,
        "seed_survey_none_countered: peer pull must succeed;\n\
         --- stdout ---\n{}\n--- stderr ---\n{}",
        pulled.stdout, pulled.stderr
    );

    // Recover the survey's edge CIDs in the slice-10 grouped render order. With NO counter
    // authored against any of them, EVERY edge is un-countered (countered_cids EMPTY).
    let ordered_cids = read_survey_edge_cids_in_render_order(env, dimension, entity);
    assert!(
        !ordered_cids.is_empty(),
        "seed_survey_none_countered: the surveyed {dimension} must render >= 1 edge so the \
         no-noise assertion is meaningful; got an empty survey for {entity:?}"
    );

    SeededSurveyEdges {
        entity: entity.to_string(),
        uncountered_cids: ordered_cids.clone(),
        ordered_cids,
        countered_cids: Vec::new(),
    }
}

/// Seed a LARGE `/project?subject=<entity>` survey with MANY edges across MANY groups
/// and a KNOWN countered subset (the CF N+1-flatten behavioral-proxy fixture; I-CF-8 /
/// ADR-050). The survey is large enough that a per-group or per-edge presence call
/// would be observably wrong; the known countered subset is spread across DISTINCT
/// groups so the single flattened call (ADR-050: collect every `EdgeRow.cid` across all
/// groups from the FLAT survey rows BEFORE grouping) must flag every countered edge —
/// and only those — in ONE request. Returns the [`SeededSurveyEdges`].
///
/// SCAFFOLD: true (slice-13).
pub fn seed_project_survey_many_groups_known_countered_subset(
    env: &TestEnv,
) -> SeededSurveyEdges {
    // Rachel is the SURVEYED contributor asserting the SHARED project subject across MANY
    // DISTINCT philosophies — eight edges. Since `/project` groups by `object`
    // (philosophy), eight DISTINCT objects yield EIGHT groups: a genuinely LARGE,
    // multi-group survey over which a per-group or per-edge presence call would be
    // observably wrong, but the ADR-050 single flattened call (every EdgeRow.cid across
    // every group, collected from the FLAT survey rows BEFORE grouping) must flag every
    // countered edge — and only those — in ONE request. THREE of the eight edges are then
    // countered, each by a DISTINCT peer, and the targets are spread across DISTINCT
    // groups (the 1st, 4th, and 7th surveyed objects) so the proxy genuinely exercises the
    // cross-group flatten — a per-group call would miss the groups it never visits.
    //
    // Build ALL FOUR peers' verifiable wire records UP FRONT, holding each `PeerPds` ALIVE
    // for the whole function so a SINGLE `peer pull` over all of them succeeds (a second
    // pull after a peer is already subscribed would re-resolve it with no resolver wired
    // and fail).
    let surveyed_project = "github:peer/rachel-cargo";
    let surveyed_author = COUNTER_TARGET_AUTHOR_RACHEL;
    let rachel_seed = [41u8; 32];
    // EIGHT distinct philosophy objects → eight edges across eight groups. Distinct
    // confidences keep each triple genuinely distinct (no canonical-CID aliasing).
    let surveyed_objects = [
        TRAVERSAL_PHILOSOPHY_DEP_PINNING,
        TRAVERSAL_PHILOSOPHY_REPRO_BUILDS,
        "org.openlore.philosophy.memory-safety",
        "org.openlore.philosophy.unix-philosophy",
        "org.openlore.philosophy.federation-first",
        "org.openlore.philosophy.capability-security",
        "org.openlore.philosophy.zero-copy",
        "org.openlore.philosophy.actor-model",
    ];
    let triples = surveyed_objects
        .iter()
        .enumerate()
        .map(|(i, object)| {
            // Confidences in (0,1], distinct per edge: 0.90, 0.81, 0.72, ...
            let confidence = 0.90 - (i as f64) * 0.09;
            (surveyed_project, *object, confidence)
        })
        .collect::<Vec<_>>();
    let (rachel_records, rachel_pubkey_hex) =
        build_verifiable_peer_records_for_triples(surveyed_author, rachel_seed, &triples);

    // The KNOWN countered subset: the surveyed edges at indices 0, 3, and 6 — spread
    // across THREE DISTINCT groups (objects), so a per-group presence read could not flag
    // them all from a single group's CIDs. Their deterministic CIDs are the counter
    // targets (the pull pipeline recomputes the SAME CID the builder computed).
    let countered_indices = [0usize, 3, 6];
    let target_cids = countered_indices
        .iter()
        .map(|&i| {
            rachel_records
                .get(i)
                .unwrap_or_else(|| {
                    panic!(
                        "seed_project_survey_many_groups_known_countered_subset: Rachel's \
                         surveyed record #{i} must exist"
                    )
                })
                .rkey
                .clone()
        })
        .collect::<Vec<_>>();

    // THREE DISTINCT counter authors, one per target (distinct seeds → distinct keys;
    // distinct target CIDs → distinct counter-record CIDs). Each lands in
    // `peer_claim_references` with `referenced_cid == its target`, the peer arm of the
    // UNION-ALL the single flattened presence read exercises.
    let counter_authors: [(&str, [u8; 32]); 3] = [
        (COUNTER_AUTHOR_TOBIAS, [9u8; 32]),
        ("did:plc:uli-test", [11u8; 32]),
        ("did:plc:wren-test", [13u8; 32]),
    ];
    let counters = counter_authors
        .iter()
        .zip(target_cids.iter())
        .map(|((did, seed), target_cid)| {
            let (record, pubkey_hex) = build_verifiable_peer_counter_record(
                did,
                *seed,
                target_cid,
                Some(COUNTER_PEER_REASON_VERBATIM),
            );
            (*did, record, pubkey_hex)
        })
        .collect::<Vec<_>>();

    let rachel_pds = PeerPds::for_peer(surveyed_author, rachel_records);
    let counter_pds = counters
        .iter()
        .map(|(did, record, _)| (*did, PeerPds::for_peer(did, vec![record.clone()])))
        .collect::<Vec<_>>();

    // STEP 2 — subscribe to every peer via the real `peer add` verb (resolver wired per
    // peer), then `peer pull` ALL of them in ONE invocation while every PDS is alive.
    let added = run_openlore_with_peer_resolver(
        env,
        &["peer", "add", surveyed_author],
        surveyed_author,
        rachel_pds.endpoint_url(),
    );
    assert_eq!(
        added.status, 0,
        "seed_project_survey_many_groups_known_countered_subset: peer add for \
         {surveyed_author} must succeed;\n--- stdout ---\n{}\n--- stderr ---\n{}",
        added.stdout, added.stderr
    );
    for (did, pds) in &counter_pds {
        let added = run_openlore_with_peer_resolver(env, &["peer", "add", did], did, pds.endpoint_url());
        assert_eq!(
            added.status, 0,
            "seed_project_survey_many_groups_known_countered_subset: peer add for {did} must \
             succeed;\n--- stdout ---\n{}\n--- stderr ---\n{}",
            added.stdout, added.stderr
        );
    }

    let mut seams = vec![PeerSeam {
        peer_did: surveyed_author,
        peer_endpoint: rachel_pds.endpoint_url(),
        peer_pubkey_hex: &rachel_pubkey_hex,
    }];
    for ((did, _record, pubkey_hex), (_, pds)) in counters.iter().zip(counter_pds.iter()) {
        seams.push(PeerSeam {
            peer_did: did,
            peer_endpoint: pds.endpoint_url(),
            peer_pubkey_hex: pubkey_hex,
        });
    }
    let pulled = run_openlore_pull_multi(env, &["peer", "pull"], &seams);
    assert_eq!(
        pulled.status, 0,
        "seed_project_survey_many_groups_known_countered_subset: peer pull must succeed;\n\
         --- stdout ---\n{}\n--- stderr ---\n{}",
        pulled.stdout, pulled.stderr
    );

    // STEP 3 — recover the survey's edge CIDs in the slice-10 grouped render order (the
    // flat union of every EdgeRow.cid across every EdgeGroup — ADR-050's flatten point),
    // then split the KNOWN countered subset + the rest un-countered. (Every counter row
    // carries a DIFFERENT subject, so none appear in THIS project's survey.)
    let ordered_cids = read_survey_edge_cids_in_render_order(env, "project", surveyed_project);
    for target_cid in &target_cids {
        assert!(
            ordered_cids.contains(target_cid),
            "seed_project_survey_many_groups_known_countered_subset: the countered target CID \
             {target_cid:?} must be among the surveyed project's edges; got {ordered_cids:?}"
        );
    }
    let uncountered_cids = ordered_cids
        .iter()
        .filter(|cid| !target_cids.contains(cid))
        .cloned()
        .collect::<Vec<_>>();

    SeededSurveyEdges {
        entity: surveyed_project.to_string(),
        ordered_cids,
        countered_cids: target_cids,
        uncountered_cids,
    }
}

/// Assert a rendered `/peer-claims` LIST body (fragment OR full page) FLAGS the given
/// countered peer row: the row carries the neutral "Countered" marker
/// ([`LIST_COUNTERED_FLAG_TEXT`]) rendered as a render-only
/// `<a href="/claims/{cid}">Countered</a>` one-hop link to that claim's slice-11
/// thread (US-CF-002 / I-CF-6). The FEDERATED-surface sibling of
/// [`assert_list_row_flagged_countered`]. Scans the rendered HTML only.
///
/// SCAFFOLD: true (slice-13).
pub fn assert_peer_claim_row_flagged_countered(body: &str, countered_cid: &str) {
    // The flag is the render-only one-hop anchor `<a href="/claims/{cid}">Countered</a>`
    // (maud emits no whitespace inside the element). Scan the rendered HTML only.
    let marker = format!(
        "<a href=\"/claims/{countered_cid}\">{LIST_COUNTERED_FLAG_TEXT}</a>"
    );
    assert!(
        body.contains(&marker),
        "assert_peer_claim_row_flagged_countered: the countered /peer-claims row for \
         {countered_cid:?} must carry the render-only marker {marker:?} (neutral presence \
         flag + one-hop link; US-CF-002 / I-CF-6); body was:\n{body}"
    );
}

/// Assert a rendered `/peer-claims` LIST body does NOT flag the given un-countered peer
/// row: the row for `uncountered_cid` carries NO "Countered" marker and NO "0 counters"
/// / empty-state noise — it renders exactly as slice-06 (US-CF-002 no-noise). The
/// FEDERATED sibling of [`assert_list_row_not_flagged`]. Scans the rendered HTML only.
///
/// SCAFFOLD: true (slice-13).
pub fn assert_peer_claim_row_not_flagged(body: &str, uncountered_cid: &str) {
    // The un-countered row carries NO `<a href="/claims/{cid}">Countered</a>` flag
    // anchor. (Its bare CID still appears in the row's CID cell — we assert the
    // absence of the FLAG anchor specifically, not the CID text.)
    let marker = format!(
        "<a href=\"/claims/{uncountered_cid}\">{LIST_COUNTERED_FLAG_TEXT}</a>"
    );
    assert!(
        !body.contains(&marker),
        "assert_peer_claim_row_not_flagged: the un-countered /peer-claims row for \
         {uncountered_cid:?} must carry NO 'Countered' flag marker {marker:?} (renders \
         exactly as slice-06; US-CF-002 no-noise); body was:\n{body}"
    );
    // And NO "0 counters" / "no disagreement" empty-state noise anywhere on the page.
    for noise in ["0 counters", "no disagreement", "no counters"] {
        assert!(
            !body.to_lowercase().contains(noise),
            "assert_peer_claim_row_not_flagged: an un-countered /peer-claims list must carry \
             no {noise:?} empty-state noise (no-noise discipline; US-CF-002); body was:\n{body}"
        );
    }
}

/// Assert the "Countered" marker on a countered `/peer-claims` row is a render-only
/// `<a href="/claims/{cid}">` ONE-HOP link to that claim's slice-11 thread — navigation
/// TEXT, never an executable write/sign/counter control (US-CF-002 / I-CF-1 / I-CF-6).
/// The FEDERATED sibling of [`assert_list_flag_links_to_thread`].
///
/// SCAFFOLD: true (slice-13).
pub fn assert_peer_claim_flag_links_to_thread(body: &str, countered_cid: &str) {
    // The marker is the render-only one-hop anchor `<a href="/claims/{cid}">Countered</a>`
    // — navigation TEXT to the slice-11 thread, never an executable control (maud emits no
    // whitespace inside the element). Scan the rendered HTML only.
    let anchor = format!(
        "<a href=\"/claims/{countered_cid}\">{LIST_COUNTERED_FLAG_TEXT}</a>"
    );
    assert!(
        body.contains(&anchor),
        "assert_peer_claim_flag_links_to_thread: the 'Countered' marker on the countered \
         /peer-claims row for {countered_cid:?} must be the render-only one-hop link \
         {anchor:?} to its slice-11 thread (navigation TEXT, never a control; US-CF-002 / \
         I-CF-1 / I-CF-6); body was:\n{body}"
    );
    // It is a plain anchor — never an executable write/sign/counter control. The peer list
    // flag never emits a form, button, or onclick wrapping the marker (the human gate stays
    // the CLI; I-CF-1).
    for control in ["<form", "<button", "onclick"] {
        assert!(
            !body.to_ascii_lowercase().contains(control),
            "assert_peer_claim_flag_links_to_thread: the /peer-claims flag must be a \
             render-only anchor, never an executable control ({control:?}); the only write \
             path is the CLI (I-CF-1); body was:\n{body}"
        );
    }
}

/// Assert a `/peer-claims` row countered by N (>1) authors shows EXACTLY ONE neutral
/// "Countered" marker — presence-only, NEVER a count ("disputed by N"), a verdict, or a
/// merged consensus row (US-CF-002 / I-CF-3 / KPI-AV-2). The FEDERATED sibling of
/// [`assert_list_flag_is_single_neutral_presence`].
///
/// SCAFFOLD: true (slice-13).
pub fn assert_peer_claim_flag_is_single_neutral_presence(body: &str, target_cid: &str) {
    // PRESENCE-only: a peer CID countered by N distinct authors collapses (DISTINCT
    // referenced_cid) to ONE presence membership → EXACTLY ONE render-only "Countered"
    // marker on its row, NEVER one-per-counter and never a count. Count the exact anchor
    // occurrences for this CID — it must appear EXACTLY once.
    let marker = format!(
        "<a href=\"/claims/{target_cid}\">{LIST_COUNTERED_FLAG_TEXT}</a>"
    );
    let occurrences = body.matches(&marker).count();
    assert_eq!(
        occurrences, 1,
        "assert_peer_claim_flag_is_single_neutral_presence: the /peer-claims row for \
         {target_cid:?} (countered by ≥2 distinct authors) must carry EXACTLY ONE neutral \
         'Countered' marker (presence membership → one flag, DISTINCT referenced_cid; \
         I-CF-3 / KPI-AV-2), got {occurrences} occurrences of {marker:?}; body was:\n{body}"
    );

    // The flag is PRESENCE-only: the list body carries NONE of the count / verdict /
    // merged-judgement phrasings — never "disputed by N", never a consensus or net
    // verdict. Lowercased so a capitalized variant can never sneak through (mirrors the
    // slice-11 `assert_counter_thread_presence_flag_is_neutral` blocklist on the LIST).
    let lowered = body.to_ascii_lowercase();
    for verdict in [
        // count-based / merged-judgement phrasing (never a count aggregated to a verdict)
        "disputed by",
        "consensus",
        "net verdict",
        "people disagree",
        // verdict words — the flag asserts presence, never correctness of the counter
        "disputed",
        "refuted",
        "is false",
        "is wrong",
    ] {
        assert!(
            !lowered.contains(verdict),
            "assert_peer_claim_flag_is_single_neutral_presence: the /peer-claims 'Countered' \
             flag is presence-only — it must NEVER emit a count / verdict / merged-judgement \
             phrasing ({verdict:?}); a twice-countered row shows ONE neutral marker, never \
             'disputed by 2' (I-CF-3 / KPI-AV-2); body was:\n{body}"
        );
    }
}

/// Assert the `/peer-claims` row for the given peer still shows its peer ORIGIN (the
/// peer's `author_did`) verbatim BESIDE any flag — the flag is ADDITIVE and changes
/// nothing about the origin column (US-CF-002 origin-unchanged AC / I-CF-4). Scans the
/// rendered HTML only.
///
/// SCAFFOLD: true (slice-13).
pub fn assert_peer_claim_row_origin_unchanged(body: &str, peer_did: &str) {
    // The peer-origin column renders the peer's `author_did` VERBATIM (viewer-domain
    // `render_peer_origin` -> `{author_did} (via {pds})`). The flag is ADDITIVE: it lives
    // in the trailing CID cell and changes NOTHING about the origin column, so the bare
    // peer DID still appears verbatim in the rendered body beside the flag.
    assert!(
        body.contains(peer_did),
        "assert_peer_claim_row_origin_unchanged: the flagged /peer-claims body must still \
         show the peer origin {peer_did:?} verbatim beside the flag — the flag is additive \
         and changes nothing about the origin column (US-CF-002 / I-CF-4); body was:\n{body}"
    );
}

/// Assert the flagged `/peer-claims` render is byte-identical to the slice-06 reference
/// render of the SAME store in row ORDER and paging EXCEPT the additive "Countered"
/// markers — the flag never re-orders or re-pages the federated list (US-CF-002
/// no-regression / I-CF-2). Uses the slice-12 baseline+marker-elision tactic adapted to
/// the `/peer-claims` order.
///
/// SCAFFOLD: true (slice-13).
pub fn assert_peer_claims_order_byte_identical(flagged: &str, ordered_cids: &[String]) {
    // ELIDE the additive markers: remove every `<a href="/claims/{cid}">Countered</a>`
    // anchor (the ONLY thing slice-13 adds — appended INSIDE the CID `<td>`, see
    // viewer-domain `render_peer_claim_row` / `render_peer_list_presence_flag`). What
    // remains IS the slice-06 `/peer-claims` byte-stream for this store. Eliding for EVERY
    // recorded CID (countered rows carry one; un-countered rows carry none, so the replace
    // is a no-op there) keeps the helper agnostic to WHICH rows were flagged.
    let mut elided = flagged.to_string();
    for cid in ordered_cids {
        let anchor = format!("<a href=\"/claims/{cid}\">{LIST_COUNTERED_FLAG_TEXT}</a>");
        elided = elided.replace(&anchor, "");
    }
    // No additive marker survives the elision — the remaining body is pure slice-06.
    assert!(
        !elided.contains(LIST_COUNTERED_FLAG_TEXT),
        "assert_peer_claims_order_byte_identical: every additive {LIST_COUNTERED_FLAG_TEXT:?} \
         anchor must elide cleanly so the remaining body is the slice-06 /peer-claims render; \
         a residual marker means the flag is NOT purely the recorded `<a href>` anchor \
         (US-CF-002 / I-CF-2); elided body was:\n{elided}"
    );

    // ROW ORDER byte-identity: every recorded peer-claim CID appears in the elided
    // (no-flag) body, and their first-seen byte offsets are STRICTLY INCREASING in the
    // recorded `/peer-claims` render order — the flag re-ordered / re-paged NOTHING
    // (US-CF-002 / I-CF-2).
    let mut prev_offset: Option<usize> = None;
    for cid in ordered_cids {
        let offset = elided.find(cid.as_str()).unwrap_or_else(|| {
            panic!(
                "assert_peer_claims_order_byte_identical: the elided slice-06 /peer-claims body \
                 must contain every recorded row CID; {cid:?} was missing — the flag \
                 dropped/replaced a row (a re-count/re-page regression); elided body was:\n{elided}"
            )
        });
        if let Some(prev) = prev_offset {
            assert!(
                offset > prev,
                "assert_peer_claims_order_byte_identical: the elided slice-06 /peer-claims row \
                 order must follow the recorded render order {ordered_cids:?} VERBATIM — {cid:?} \
                 rendered out of position, so the flag RE-ORDERED the list (US-CF-002 / I-CF-2); \
                 elided body was:\n{elided}"
            );
        }
        prev_offset = Some(offset);
    }
}

/// Assert a rendered traversal survey body (`/project` or `/philosophy`, fragment OR
/// full page) FLAGS the given countered EDGE: the edge carries the neutral "Countered"
/// marker rendered as a render-only `<a href="/claims/{cid}">Countered</a>` one-hop link
/// to that claim's slice-11 thread (US-CF-003 / I-CF-6). The EDGE-surface sibling of
/// [`assert_list_row_flagged_countered`]. Scans the rendered HTML only.
///
/// SCAFFOLD: true (slice-13).
pub fn assert_edge_flagged_countered(body: &str, countered_cid: &str) {
    // The flag is the render-only one-hop anchor `<a href="/claims/{cid}">Countered</a>`
    // (maud emits no whitespace inside the element). Scan the rendered HTML only.
    let marker = format!(
        "<a href=\"/claims/{countered_cid}\">{LIST_COUNTERED_FLAG_TEXT}</a>"
    );
    assert!(
        body.contains(&marker),
        "assert_edge_flagged_countered: the traversal edge for the countered CID \
         {countered_cid:?} must carry the neutral render-only one-hop {marker:?} anchor to \
         its slice-11 thread (US-CF-003 / I-CF-6); body was:\n{body}"
    );
}

/// Assert a rendered traversal survey body does NOT flag the given un-countered EDGE:
/// the edge for `uncountered_cid` carries NO "Countered" marker and NO empty-state noise
/// — it renders exactly as slice-10 (US-CF-003 no-noise). The EDGE sibling of
/// [`assert_list_row_not_flagged`]. Scans the rendered HTML only.
///
/// SCAFFOLD: true (slice-13).
pub fn assert_edge_not_flagged(body: &str, uncountered_cid: &str) {
    // The un-countered edge carries NO `<a href="/claims/{cid}">Countered</a>` flag anchor.
    // (Its bare CID still appears in the edge's CID cell — we assert the absence of the
    // FLAG anchor specifically, not the CID text.)
    let marker = format!(
        "<a href=\"/claims/{uncountered_cid}\">{LIST_COUNTERED_FLAG_TEXT}</a>"
    );
    assert!(
        !body.contains(&marker),
        "assert_edge_not_flagged: the un-countered traversal edge for {uncountered_cid:?} \
         must carry NO 'Countered' flag marker {marker:?} (renders exactly as slice-10; \
         US-CF-003 / I-CF-2); body was:\n{body}"
    );
    // And NO "0 counters" / "no disagreement" empty-state noise anywhere on the page.
    for noise in ["0 counters", "no disagreement", "no counters"] {
        assert!(
            !body.to_lowercase().contains(noise),
            "assert_edge_not_flagged: an un-countered survey must carry no {noise:?} \
             empty-state noise (no-noise discipline; US-CF-003); body was:\n{body}"
        );
    }
}

/// Assert the "Countered" marker on a countered traversal EDGE is a render-only
/// `<a href="/claims/{cid}">` ONE-HOP link to that claim's slice-11 thread — navigation
/// TEXT, never an executable write/sign/counter control (US-CF-003 / I-CF-1 / I-CF-6).
/// The EDGE sibling of [`assert_list_flag_links_to_thread`].
///
/// SCAFFOLD: true (slice-13).
pub fn assert_edge_flag_links_to_thread(body: &str, countered_cid: &str) {
    // The marker is the render-only one-hop anchor `<a href="/claims/{cid}">Countered</a>`
    // — navigation TEXT to the slice-11 thread, never an executable control (maud emits no
    // whitespace inside the element). Scan the rendered HTML only. The EDGE sibling of
    // [`assert_peer_claim_flag_links_to_thread`].
    let anchor = format!(
        "<a href=\"/claims/{countered_cid}\">{LIST_COUNTERED_FLAG_TEXT}</a>"
    );
    assert!(
        body.contains(&anchor),
        "assert_edge_flag_links_to_thread: the 'Countered' marker on the countered traversal \
         edge for {countered_cid:?} must be the render-only one-hop link {anchor:?} to its \
         slice-11 thread (navigation TEXT, never a control; US-CF-003 / I-CF-1 / I-CF-6); \
         body was:\n{body}"
    );
    // It is a plain anchor — never an executable write/sign/counter control. The edge flag
    // never emits a form, button, or onclick wrapping the marker (the human gate stays the
    // CLI; I-CF-1).
    for control in ["<form", "<button", "onclick"] {
        assert!(
            !body.to_ascii_lowercase().contains(control),
            "assert_edge_flag_links_to_thread: the traversal-edge flag must be a render-only \
             anchor, never an executable control ({control:?}); the only write path is the \
             CLI (I-CF-1); body was:\n{body}"
        );
    }
}

/// Assert a traversal EDGE countered by N (>1) authors shows EXACTLY ONE neutral
/// "Countered" marker in its unchanged group/position — presence-only, NEVER a count,
/// a verdict, or a merged consensus row (US-CF-003 / I-CF-3 / KPI-GRAPH-2). The EDGE
/// sibling of [`assert_list_flag_is_single_neutral_presence`].
///
/// SCAFFOLD: true (slice-13).
pub fn assert_edge_flag_is_single_neutral_presence(body: &str, target_cid: &str) {
    // PRESENCE-only: an edge countered by N distinct authors collapses (DISTINCT
    // referenced_cid) to ONE presence membership → EXACTLY ONE render-only "Countered"
    // marker on its edge, NEVER one-per-counter and never a count. Count the exact anchor
    // occurrences for this CID — it must appear EXACTLY once. The EDGE sibling of
    // [`assert_peer_claim_flag_is_single_neutral_presence`].
    let marker = format!(
        "<a href=\"/claims/{target_cid}\">{LIST_COUNTERED_FLAG_TEXT}</a>"
    );
    let occurrences = body.matches(&marker).count();
    assert_eq!(
        occurrences, 1,
        "assert_edge_flag_is_single_neutral_presence: the traversal edge for {target_cid:?} \
         (countered by ≥2 distinct authors) must carry EXACTLY ONE neutral 'Countered' marker \
         (presence membership → one flag, DISTINCT referenced_cid; I-CF-3 / KPI-GRAPH-2), got \
         {occurrences} occurrences of {marker:?}; body was:\n{body}"
    );

    // The flag is PRESENCE-only: the survey body carries NONE of the count / verdict /
    // merged-judgement phrasings — never "disputed by N", never a consensus or net verdict.
    // Lowercased so a capitalized variant can never sneak through (mirrors the LIST sibling).
    let lowered = body.to_ascii_lowercase();
    for verdict in [
        // count-based / merged-judgement phrasing (never a count aggregated to a verdict)
        "disputed by",
        "consensus",
        "net verdict",
        "people disagree",
        // verdict words — the flag asserts presence, never correctness of the counter
        "disputed",
        "refuted",
        "is false",
        "is wrong",
    ] {
        assert!(
            !lowered.contains(verdict),
            "assert_edge_flag_is_single_neutral_presence: the traversal-edge 'Countered' flag \
             is presence-only — it must NEVER emit a count / verdict / merged-judgement \
             phrasing ({verdict:?}); a twice-countered edge shows ONE neutral marker, never \
             'disputed by 2' (I-CF-3 / KPI-GRAPH-2); body was:\n{body}"
        );
    }
}

/// Assert the flagged traversal-survey render is byte-identical to the slice-10
/// reference render of the SAME store in GROUPING, group order, edge order, and the
/// deduped contributor list EXCEPT the additive "Countered" markers — the flag never
/// re-groups, re-orders, or re-deduplicates the survey (US-CF-003 no-regression GOLD /
/// I-CF-9). Uses the slice-12 baseline+marker-elision tactic adapted to the traversal
/// survey: elide every additive "Countered" anchor and prove the remaining slice-10
/// body honours the recorded grouping + edge order (`ordered_cids`, strictly increasing
/// byte offsets) byte-for-byte. This is the CARDINAL no-regroup gold (I-CF-9).
///
/// SCAFFOLD: true (slice-13).
pub fn assert_survey_grouping_and_order_byte_identical(
    flagged: &str,
    ordered_cids: &[String],
) {
    // ELIDE the additive markers: remove every `<a href="/claims/{cid}">Countered</a>`
    // anchor (the ONLY thing slice-13 adds — appended INSIDE the CID `<td>`, see
    // viewer-domain `render_edge_row`). What remains IS the slice-10 traversal byte-stream
    // for this store. Eliding for EVERY recorded CID (countered edges carry one;
    // un-countered edges carry none, so the replace is a no-op there) keeps the helper
    // agnostic to WHICH edges were flagged.
    let mut elided = flagged.to_string();
    for cid in ordered_cids {
        let anchor = format!("<a href=\"/claims/{cid}\">{LIST_COUNTERED_FLAG_TEXT}</a>");
        elided = elided.replace(&anchor, "");
    }
    // No additive marker survives the elision — the remaining body is pure slice-10.
    assert!(
        !elided.contains(LIST_COUNTERED_FLAG_TEXT),
        "assert_survey_grouping_and_order_byte_identical: every additive \
         {LIST_COUNTERED_FLAG_TEXT:?} anchor must elide cleanly so the remaining body is the \
         slice-10 traversal render; a residual marker means the flag is NOT purely the \
         recorded `<a href>` anchor (US-CF-003 / I-CF-9); elided body was:\n{elided}"
    );

    // GROUPING + EDGE ORDER byte-identity: every recorded edge CID appears in the elided
    // (no-flag) body, and their first-seen byte offsets are STRICTLY INCREASING in the
    // recorded slice-10 grouped render order — the flag re-grouped / re-ordered NOTHING
    // (US-CF-003 CARDINAL no-regroup gold / I-CF-9).
    let mut prev_offset: Option<usize> = None;
    for cid in ordered_cids {
        let offset = elided.find(cid.as_str()).unwrap_or_else(|| {
            panic!(
                "assert_survey_grouping_and_order_byte_identical: the elided slice-10 \
                 traversal body must contain every recorded edge CID; {cid:?} was missing — \
                 the flag dropped/replaced an edge (a re-group/re-order regression); elided \
                 body was:\n{elided}"
            )
        });
        if let Some(prev) = prev_offset {
            assert!(
                offset > prev,
                "assert_survey_grouping_and_order_byte_identical: the elided slice-10 \
                 traversal edge order must follow the recorded grouped render order \
                 {ordered_cids:?} VERBATIM — {cid:?} rendered out of position, so the flag \
                 RE-GROUPED / RE-ORDERED the survey (US-CF-003 / I-CF-9); elided body \
                 was:\n{elided}"
            );
        }
        prev_offset = Some(offset);
    }
}

// =============================================================================
// slice-14 (viewer-counter-flags-score-surface; DISTILL) — the SAME neutral
// "Countered" presence flag (slice-11/12/13 `COUNTERED_PRESENCE_FLAG`, REUSED
// verbatim) extended to the LAST LOCAL surface the operator scans: the
// SCORING-BEARING `GET /score?contributor=<did>` per-contribution breakdown rows
// (US-CF-001 infra wiring + US-CF-002 user-visible flag). REUSES the slice-12
// `StoreReadPort::counter_presence_for(&[cid]) -> HashSet<String>` batch read
// (ADR-048 / ADR-051) — NO new read method, NO new SQL, NO new route. The flag is
// the REUSED slice-13 `render_countered_link(&c.cid.0, presence.contains(&c.cid.0))`
// SSOT (`<a href="/claims/{cid}">Countered</a>`) — NO new render fn, NO new string.
//
// The slice-14 CARDINAL distinction from slices 12/13 (D-14-2 / I-CF-9): `/score`
// carries SCORING SEMANTICS, so the flag must be provably ORTHOGONAL to the score —
// SHOWN, never APPLIED. The per-contribution subtotals STILL sum to the displayed
// pairing weight WITH the flag present; a countered contribution renders its FULL
// original subtotal (the counter subtracts nothing); and with the markers AND the
// `SCORE_COUNTER_LEGEND` anti-misread legend elided, the `/score` render is
// byte-identical to the slice-09 baseline (every weight / confidence / author bonus
// / triangulation bonus / subtotal / headline total / bucket / `[SPARSE]` line /
// pairing ranking / contribution row order unchanged). The legend (the ONE genuinely
// NEW artifact, DD-14-3) carries plain-language orthogonality copy and NEVER any
// verdict/penalty word ("disputed"/"refuted"/"false"/"penalty"/"deduction"/
// "lowered"/"disputed score").
//
// Seeding drives the PRODUCTION paths (Pillar 3 / BR-VIEW-4): the contributor's
// scoring trail lands via `peer add` + `peer pull` (REUSING the slice-09
// `build_verifiable_peer_records_for_triples` rich-trail shape — several DISTINCT
// subjects on the shared object so the pure `scoring::score` yields a real weight +
// a MULTI-ROW breakdown that decomposes, NOT `[SPARSE]`). The COUNTER targeting ONE
// contribution's CID lands via a DISTINCT peer's `build_verifiable_peer_counter_
// record` + `peer add` + `peer pull` (so its `counters` reference lands in
// `peer_claim_references` with `referenced_cid == <target cid>`, the peer arm of the
// presence UNION-ALL). SELF-COUNTER IS BLOCKED, so the countered contribution must be
// authored such that a *peer* (Tobias) counters it — the contributor's own claims are
// pulled-peer rows, and the counter is a SECOND distinct peer's record. ALL rows land
// in the SAME local store `openlore ui` reads in ONE `peer pull` (the single-pull
// discipline the slice-09 score seeds document — pulling one peer at a time would
// drop the other's now-dead PDS and 404 its DID resolution). NO hand-inserted store
// rows. The presence read is LOCAL (DB-index only); NO network seam on `/score`
// (offline by construction, AC-SCORE-LOCAL).
//
// Layer placement (Mandate 9/11): every slice-14 scenario is a layer-3/layer-5
// subprocess + real-I/O test — EXAMPLE-only. The sad/edge paths (none-countered,
// multi-author counter, identical-subtotal anti-misread) are enumerated explicitly,
// never PBT-generated at this layer. The strict 1-query N+1 bound is a DELIVER
// `adapter-duckdb` unit/property assertion (REUSED slice-12 read); at this subprocess
// AT layer the N+1 guard is asserted via its behavioral proxy (a multi-pairing,
// multi-contribution breakdown flags the countered subset correctly in ONE request).
//
// Mandate 7 RED scaffolds (ADR-025): every seed + assert below is `todo!()` — it
// COMPILES now (signatures resolve so the AT files build), then PANICS at runtime →
// classifies RED (MISSING_FUNCTIONALITY), NOT BROKEN. They stay RED until DELIVER's
// per-scenario RED→GREEN→COMMIT cycles.
// =============================================================================

/// The exact slice-14 anti-misread legend copy (DD-14-3 / AC-SCORE-ANTIMISREAD) the
/// rendered scored `/score` breakdown MUST carry ONCE — the `viewer-domain`
/// `SCORE_COUNTER_LEGEND` SSOT constant mirrored here so the seeds + asserts never
/// drift from the production string. The copy is deliberately NEUTRAL: it states the
/// marker is shown for the reader to judge and does NOT change the score; it carries
/// NONE of the verdict/penalty words on [`SCORE_LEGEND_BLOCKLIST`]. Held in ONE place
/// (the byte-identity elision + the legend-present-and-clean assert both reference it).
pub const SCORE_COUNTER_LEGEND_TEXT: &str =
    "A “Countered” marker means another claim disagrees with this one elsewhere. \
     It is shown for you to judge and does not change this contributor's score — \
     each contribution keeps its full weight.";

/// The verdict / penalty / subtraction blocklist the anti-misread legend (and the
/// whole scored `/score` body) must NEVER contain (AC-SCORE-ANTIMISREAD; reuses the
/// slice-11 verdict-word blocklist + the score-specific subtraction words). A reader
/// must not be able to misread the flag as a score deduction, so none of these may
/// appear anywhere on the rendered breakdown. Lowercased at the comparison site so a
/// capitalized variant ("Disputed", "Penalty") can never sneak through.
pub const SCORE_LEGEND_BLOCKLIST: &[&str] = &[
    "disputed",
    "refuted",
    "false",
    "penalty",
    "deduction",
    "lowered",
    "disputed score",
];

/// The handle a `/score` flag seed returns: the contributor whose breakdown is
/// rendered (`contributor_did`), the set of every contribution CID across every
/// pairing in the rendered breakdown (the flatten point of DD-14-2 / ADR-051), split
/// into the COUNTERED subset (contribution rows that MUST carry the "Countered"
/// marker linking to `/claims/{cid}`) and the UN-COUNTERED subset (contribution rows
/// that MUST render exactly as slice-09 — no marker). Mirrors [`SeededPeerClaimsList`]
/// / [`SeededSurveyEdges`] for the SCORING surface.
#[derive(Debug, Clone)]
pub struct SeededScoreBreakdown {
    /// The contributor DID the scenario queries (`/score?contributor=<did>`).
    pub contributor_did: String,
    /// Every contribution CID across the whole rendered breakdown, in the slice-09
    /// rendered (ranked pairing → row) order. The byte-identity gold pins this order
    /// + every weight/subtotal byte-identical to slice-09 with markers + legend
    /// elided (AC-SCORE-BYTEID / I-CF-9).
    pub ordered_cids: Vec<String>,
    /// The subset of `ordered_cids` whose contribution has >= 1 counter — each row
    /// MUST carry the neutral "Countered" marker in its UNCHANGED position, its FULL
    /// original subtotal preserved (US-CF-002 / AC-SCORE-SUMWEIGHT).
    pub countered_cids: Vec<String>,
    /// The subset of `ordered_cids` with NO counter — each row MUST render exactly as
    /// slice-09 (no marker, no noise; AC-002-NO-NOISE).
    pub uncountered_cids: Vec<String>,
}

/// Read every contribution CID a `/score` breakdown is built from, in the slice-09
/// rendered (ranked) order — every `peer_claims` row attributed to `contributor_did`
/// (the contributor's scoring feed `claims ∪ peer_claims`; the trail seeds land the
/// rows in `peer_claims`). Reads the SAME store the viewer's local feed read returns,
/// so the slice-14 score-flag seeds can return the EXACT contribution CID set the
/// breakdown renders (and the byte-identity gold can pin order). Read-only; opens a
/// SECOND short-lived connection. The SCORING-surface sibling of
/// [`read_peer_claim_cids_in_list_order`] — reuses [`read_peer_claim_cids_for`]
/// (the contributor's rows are `peer_claims` attributed to its DID).
///
/// SCAFFOLD: true (slice-14).
pub fn read_score_contribution_cids(env: &TestEnv, contributor_did: &str) -> Vec<String> {
    // The contributor's scoring trail rows are `peer_claims` attributed to its BARE
    // DID (the rich-trail seed pulls them via `peer add` + `peer pull`; the production
    // pull stores `peer_claims.author_did` as the bare DID — see slice-09's
    // `read_peer_claim_cids_for(&env, CONTRIBUTOR_RICH_DID)`). Recover them through the
    // EXISTING `read_peer_claim_cids_for` read — no new SQL. The score breakdown is
    // built from exactly these rows (a DISTINCT counter-author peer's counter lives
    // under ITS own author DID, so it is excluded by construction).
    read_peer_claim_cids_for(env, contributor_did)
}

/// Seed a SCORED `/score` breakdown for a RICH-trail contributor with EXACTLY ONE
/// countered contribution among several un-countered contributions (the WS + Scenario
/// 1/2/3/4 fixture). The contributor's trail (several DISTINCT subjects on the shared
/// reproducible-builds object → `cross_project_span >= 2`, a real weight + a MULTI-ROW
/// breakdown, NOT `[SPARSE]`) is pulled via the PRODUCTION `peer add` + `peer pull`
/// path so its rows land in `peer_claims`; a DISTINCT peer (Tobias) counters ONE of
/// those contribution CIDs (self-counter is BLOCKED, so the counter MUST be a peer's),
/// landing in `peer_claim_references` with `referenced_cid == target_cid` (the peer
/// arm of the presence UNION-ALL). ALL rows ride ONE `peer pull` (single-pull
/// discipline). Returns the [`SeededScoreBreakdown`] so the scenario addresses the
/// exact countered + un-countered contribution rows.
///
/// SCAFFOLD: true (slice-14) — DELIVER materializes it by: (1) building the
/// contributor's rich-trail records UP FRONT via `build_verifiable_peer_records_for_
/// triples(contributor_did, seed, &[(subject, repro, conf); 4])`, keeping the PDS
/// ALIVE; (2) recovering ONE record's deterministic CID as the counter target; (3)
/// building Tobias's `build_verifiable_peer_counter_record(COUNTER_AUTHOR_TOBIAS,
/// seed, &target_cid, …)`; (4) `peer add` BOTH then `run_openlore_pull_multi` over
/// BOTH `PeerSeam`s in ONE pull; (5) recovering every contribution CID
/// (`read_score_contribution_cids`) and splitting the ONE countered + the rest.
pub fn seed_score_breakdown_one_contribution_countered(env: &TestEnv) -> SeededScoreBreakdown {
    // The scored contributor (a DISTINCT peer DID, so its rows land in `peer_claims`
    // attributed to it; the local user is NOT the contributor here). Its rich trail
    // (FOUR distinct subjects on the shared reproducible-builds object at varied
    // confidences → cross_project_span ≥ 2, a real weight + a MULTI-ROW breakdown,
    // NOT `[SPARSE]`) reuses the slice-09 rich-trail shape VERBATIM.
    let contributor_did = CONTRIBUTOR_RICH_DID;
    let contributor_seed = [23u8; 32];
    let repro = SCORE_OBJECT_REPRODUCIBLE_BUILDS;
    let contributor_triples: [(&str, &str, f64); 4] = [
        ("github:bazelbuild/bazel", repro, 0.86),
        ("github:NixOS/nixpkgs", repro, 0.90),
        ("github:reproducible-builds/diffoscope", repro, 0.74),
        ("github:GNOME/meson", repro, 0.62),
    ];

    // STEP 1 — build the contributor's verifiable trail records UP FRONT (the same
    // REAL Ed25519 crypto + deterministic CID-recompute the pull pipeline verifies).
    // Each record's `rkey` IS its deterministic CID, so we can pick ONE as the
    // counter's target BEFORE either is pulled.
    let (contributor_records, contributor_pubkey_hex) =
        build_verifiable_peer_records_for_triples(
            contributor_did,
            contributor_seed,
            &contributor_triples,
        );
    // The counter targets the FIRST contribution CID (any one of the rich trail's
    // rows; a DISTINCT peer counters it — self-counter is BLOCKED by construction).
    let target_cid = contributor_records
        .first()
        .expect("the rich trail yields ≥1 contribution record")
        .rkey
        .clone();

    // STEP 2 — a DISTINCT peer (Tobias) authors a verifiable COUNTER referencing that
    // ONE contribution CID (a `references[].type == counters` entry whose `cid ==
    // target_cid`, ADR-015). When pulled it lands in `peer_claims` (under Tobias's own
    // DID) and its `counters` reference lands in `peer_claim_references` with
    // `referenced_cid == target_cid` — the peer arm of the presence UNION-ALL.
    let tobias_seed = [9u8; 32];
    let (tobias_record, tobias_pubkey_hex) = build_verifiable_peer_counter_record(
        COUNTER_AUTHOR_TOBIAS,
        tobias_seed,
        &target_cid,
        Some(COUNTER_PEER_REASON_VERBATIM),
    );

    let contributor_pds = PeerPds::for_peer(contributor_did, contributor_records);
    let tobias_pds = PeerPds::for_peer(COUNTER_AUTHOR_TOBIAS, vec![tobias_record]);

    // STEP 3 — subscribe to BOTH peers via the real `peer add` verb (resolver wired
    // per peer), holding each PDS ALIVE for the whole function so a SINGLE `peer pull`
    // over BOTH succeeds (single-pull discipline). The contributor IS a subscribed
    // peer (its trail rides the federation path), and so is Tobias (the counter).
    for (did, pds) in [
        (contributor_did, &contributor_pds),
        (COUNTER_AUTHOR_TOBIAS, &tobias_pds),
    ] {
        let added = run_openlore_with_peer_resolver(
            env,
            &["peer", "add", did],
            did,
            pds.endpoint_url(),
        );
        assert_eq!(
            added.status, 0,
            "seed_score_breakdown_one_contribution_countered: peer add for {did} must \
             succeed;\n--- stdout ---\n{}\n--- stderr ---\n{}",
            added.stdout, added.stderr
        );
    }
    let pulled = run_openlore_pull_multi(
        env,
        &["peer", "pull"],
        &[
            PeerSeam {
                peer_did: contributor_did,
                peer_endpoint: contributor_pds.endpoint_url(),
                peer_pubkey_hex: &contributor_pubkey_hex,
            },
            PeerSeam {
                peer_did: COUNTER_AUTHOR_TOBIAS,
                peer_endpoint: tobias_pds.endpoint_url(),
                peer_pubkey_hex: &tobias_pubkey_hex,
            },
        ],
    );
    assert_eq!(
        pulled.status, 0,
        "seed_score_breakdown_one_contribution_countered: peer pull must succeed;\n\
         --- stdout ---\n{}\n--- stderr ---\n{}",
        pulled.stdout, pulled.stderr
    );

    // STEP 4 — recover every contribution CID in the contributor's breakdown (the
    // `peer_claims` rows attributed to its DID — Tobias's counter is under Tobias's
    // OWN DID, so it is excluded). Split the ONE countered from the rest.
    let ordered_cids = read_score_contribution_cids(env, contributor_did);
    assert!(
        ordered_cids.contains(&target_cid),
        "seed_score_breakdown_one_contribution_countered: the countered target CID \
         {target_cid:?} must be among the contributor's contribution CIDs; got \
         {ordered_cids:?}"
    );
    assert!(
        ordered_cids.len() >= 2,
        "seed_score_breakdown_one_contribution_countered: the rich trail must yield a \
         MULTI-row breakdown (≥2 contributions) so exactly one is countered and the \
         rest are not; got {ordered_cids:?}"
    );
    let uncountered_cids = ordered_cids
        .iter()
        .filter(|cid| *cid != &target_cid)
        .cloned()
        .collect::<Vec<_>>();

    SeededScoreBreakdown {
        contributor_did: contributor_did.to_string(),
        ordered_cids,
        countered_cids: vec![target_cid],
        uncountered_cids,
    }
}

/// Seed a SCORED `/score` breakdown where ONE contribution is countered by TWO
/// DISTINCT authors (the presence-only GOLD fixture; AC-SCORE-PRESENCE). Two distinct
/// peers each author a verifiable `counters`-referencing record targeting the SAME
/// contribution CID, delivered through the PRODUCTION `peer add` + `peer pull` path,
/// so BOTH land in `peer_claim_references` with the SAME `referenced_cid`. The
/// `counter_presence_for` UNION-ALL DISTINCT collapses the two distinct-author
/// counters of the SAME CID to ONE presence membership → the contribution row carries
/// EXACTLY ONE neutral "Countered" marker (never "countered by 2"). Returns the
/// [`SeededScoreBreakdown`] whose single `countered_cids` entry is the twice-countered
/// contribution.
///
/// SCAFFOLD: true (slice-14) — adapts [`seed_score_breakdown_one_contribution_
/// countered`] with a SECOND distinct counter-author peer (e.g. Rachel) ALSO
/// countering the SAME target contribution CID, all riding the ONE `peer pull`.
pub fn seed_score_breakdown_target_two_counters_distinct_authors(
    env: &TestEnv,
) -> SeededScoreBreakdown {
    // The scored contributor (a DISTINCT peer DID, so its rows land in `peer_claims`
    // attributed to it). Its rich trail (FOUR distinct subjects on the shared
    // reproducible-builds object at varied confidences → cross_project_span ≥ 2, a
    // real weight + a MULTI-ROW breakdown, NOT `[SPARSE]`) reuses the slice-09
    // rich-trail shape VERBATIM (mirrors `seed_score_breakdown_one_contribution_
    // countered`).
    let contributor_did = CONTRIBUTOR_RICH_DID;
    let contributor_seed = [23u8; 32];
    let repro = SCORE_OBJECT_REPRODUCIBLE_BUILDS;
    let contributor_triples: [(&str, &str, f64); 4] = [
        ("github:bazelbuild/bazel", repro, 0.86),
        ("github:NixOS/nixpkgs", repro, 0.90),
        ("github:reproducible-builds/diffoscope", repro, 0.74),
        ("github:GNOME/meson", repro, 0.62),
    ];

    // STEP 1 — build the contributor's verifiable trail records UP FRONT. Each
    // record's `rkey` IS its deterministic CID, so we can pick ONE as the SHARED
    // counter target BEFORE either peer is pulled.
    let (contributor_records, contributor_pubkey_hex) =
        build_verifiable_peer_records_for_triples(
            contributor_did,
            contributor_seed,
            &contributor_triples,
        );
    // Both counters target the SAME (FIRST) contribution CID — the presence-only
    // proof: TWO distinct authors, ONE referenced_cid → ONE marker.
    let target_cid = contributor_records
        .first()
        .expect("the rich trail yields ≥1 contribution record")
        .rkey
        .clone();

    // STEP 2 — TWO DISTINCT peers (Tobias + Rachel) each author a verifiable COUNTER
    // referencing the SAME contribution CID (a `references[].type == counters` entry
    // whose `cid == target_cid`, ADR-015). When pulled BOTH land in `peer_claims`
    // (each under its OWN DID) and BOTH `counters` references land in
    // `peer_claim_references` with the SAME `referenced_cid == target_cid` — two
    // DISTINCT authors, one referenced CID. The `counter_presence_for` UNION-ALL
    // DISTINCT collapses them to ONE presence membership.
    let tobias_seed = [9u8; 32];
    let (tobias_record, tobias_pubkey_hex) = build_verifiable_peer_counter_record(
        COUNTER_AUTHOR_TOBIAS,
        tobias_seed,
        &target_cid,
        Some(COUNTER_PEER_REASON_VERBATIM),
    );
    let rachel_seed = [7u8; 32];
    let (rachel_record, rachel_pubkey_hex) = build_verifiable_peer_counter_record(
        COUNTER_TARGET_AUTHOR_RACHEL,
        rachel_seed,
        &target_cid,
        Some(COUNTER_REASON_VERBATIM),
    );

    let contributor_pds = PeerPds::for_peer(contributor_did, contributor_records);
    let tobias_pds = PeerPds::for_peer(COUNTER_AUTHOR_TOBIAS, vec![tobias_record]);
    let rachel_pds = PeerPds::for_peer(COUNTER_TARGET_AUTHOR_RACHEL, vec![rachel_record]);

    // STEP 3 — subscribe to ALL THREE peers via the real `peer add` verb (resolver
    // wired per peer), holding each PDS ALIVE for the whole function so a SINGLE
    // `peer pull` over ALL THREE succeeds (single-pull discipline). The contributor
    // IS a subscribed peer (its trail rides the federation path); Tobias and Rachel
    // are the two DISTINCT counter authors.
    for (did, pds) in [
        (contributor_did, &contributor_pds),
        (COUNTER_AUTHOR_TOBIAS, &tobias_pds),
        (COUNTER_TARGET_AUTHOR_RACHEL, &rachel_pds),
    ] {
        let added = run_openlore_with_peer_resolver(
            env,
            &["peer", "add", did],
            did,
            pds.endpoint_url(),
        );
        assert_eq!(
            added.status, 0,
            "seed_score_breakdown_target_two_counters_distinct_authors: peer add for \
             {did} must succeed;\n--- stdout ---\n{}\n--- stderr ---\n{}",
            added.stdout, added.stderr
        );
    }
    let pulled = run_openlore_pull_multi(
        env,
        &["peer", "pull"],
        &[
            PeerSeam {
                peer_did: contributor_did,
                peer_endpoint: contributor_pds.endpoint_url(),
                peer_pubkey_hex: &contributor_pubkey_hex,
            },
            PeerSeam {
                peer_did: COUNTER_AUTHOR_TOBIAS,
                peer_endpoint: tobias_pds.endpoint_url(),
                peer_pubkey_hex: &tobias_pubkey_hex,
            },
            PeerSeam {
                peer_did: COUNTER_TARGET_AUTHOR_RACHEL,
                peer_endpoint: rachel_pds.endpoint_url(),
                peer_pubkey_hex: &rachel_pubkey_hex,
            },
        ],
    );
    assert_eq!(
        pulled.status, 0,
        "seed_score_breakdown_target_two_counters_distinct_authors: peer pull must \
         succeed;\n--- stdout ---\n{}\n--- stderr ---\n{}",
        pulled.stdout, pulled.stderr
    );

    // STEP 4 — recover every contribution CID in the contributor's breakdown (the
    // `peer_claims` rows attributed to its DID — the two counters live under Tobias's
    // and Rachel's OWN DIDs, so they are excluded). Split the ONE twice-countered from
    // the rest.
    let ordered_cids = read_score_contribution_cids(env, contributor_did);
    assert!(
        ordered_cids.contains(&target_cid),
        "seed_score_breakdown_target_two_counters_distinct_authors: the twice-countered \
         target CID {target_cid:?} must be among the contributor's contribution CIDs; \
         got {ordered_cids:?}"
    );
    assert!(
        ordered_cids.len() >= 2,
        "seed_score_breakdown_target_two_counters_distinct_authors: the rich trail must \
         yield a MULTI-row breakdown (≥2 contributions) so exactly one is countered and \
         the rest are not; got {ordered_cids:?}"
    );
    let uncountered_cids = ordered_cids
        .iter()
        .filter(|cid| *cid != &target_cid)
        .cloned()
        .collect::<Vec<_>>();

    SeededScoreBreakdown {
        contributor_did: contributor_did.to_string(),
        ordered_cids,
        countered_cids: vec![target_cid],
        uncountered_cids,
    }
}

/// Seed a SCORED `/score` breakdown with TWO contributions in the SAME pairing that
/// have IDENTICAL confidence + author bonus + triangulation bonus (so they render the
/// IDENTICAL subtotal), of which EXACTLY ONE is countered (the anti-misread GOLD
/// fixture; AC-SCORE-ANTIMISREAD). The countered contribution's subtotal is its FULL
/// original value — identical to its un-countered twin — proving the counter
/// subtracts nothing; only the countered one shows the marker; the breakdown carries
/// the [`SCORE_COUNTER_LEGEND_TEXT`] legend. Returns the [`SeededScoreBreakdown`] so
/// the scenario can address the countered + the identical-subtotal un-countered row.
///
/// SCAFFOLD: true (slice-14) — DELIVER seeds the contributor's trail so two
/// contributions FALL IN THE SAME pairing with IDENTICAL confidence + bonuses (same
/// object, same rank tier — the pure scorer yields equal subtotals), then a DISTINCT
/// peer counters EXACTLY ONE of the two via `build_verifiable_peer_counter_record` +
/// the ONE `peer pull`. The `countered_cids` carries the countered twin; the
/// `uncountered_cids` carries (at least) its identical-subtotal twin.
pub fn seed_score_breakdown_identical_subtotals_one_countered(
    env: &TestEnv,
) -> SeededScoreBreakdown {
    // The scored contributor (a DISTINCT peer DID, so its rows land in `peer_claims`
    // attributed to it; the local user is NOT the contributor here). The trail is
    // shaped so the PURE `scoring::score` yields TWO contributions in the SAME
    // `(subject, object)` pairing with IDENTICAL confidence + author bonus +
    // triangulation bonus → IDENTICAL rendered subtotals (the anti-misread twins),
    // EXACTLY ONE of which a DISTINCT peer counters.
    let contributor_did = CONTRIBUTOR_RICH_DID;
    let contributor_seed = [23u8; 32];
    let object = SCORE_OBJECT_REPRODUCIBLE_BUILDS;
    let twin_subject = "github:bazelbuild/bazel";
    // The shared confidence of the two TWINS. Both twins are by the SAME author on
    // the SAME `(twin_subject, object)` pairing at this SAME confidence → same author
    // rank (1) → same author-distinct share, same triangulation status (the author
    // asserts `object` on ≥2 distinct subjects below) → BYTE-EQUAL subtotals. They
    // differ ONLY in `evidence`, the one canonicalized field that perturbs the CID
    // WITHOUT touching the subtotal, so the store keeps BOTH rows (distinct CIDs) and
    // a peer can counter EXACTLY one twin.
    let twin_confidence = 0.74_f64;
    // A SECOND distinct subject for the SAME object so the author triangulates
    // `object` (≥2 distinct subjects → `cross_project_span ≥ 2`: a real weight + a
    // non-`[SPARSE]` breakdown, and the triangulation bonus applies EQUALLY to both
    // twins since triangulation is keyed on `(author, object)`, not on subject).
    let triangulating_subject = "github:NixOS/nixpkgs";

    // STEP 1 — build the contributor's verifiable trail UP FRONT. Two TWINS share
    // `(twin_subject, object, twin_confidence)` but carry DISTINCT `evidence`
    // (distinct CIDs, byte-equal subtotals); a third claim on a DISTINCT subject for
    // the SAME object triangulates it. Each record's `rkey` IS its deterministic CID,
    // so we can pick ONE twin as the counter target BEFORE anything is pulled.
    let contributor_quadruples: [(&str, &str, f64, &str); 3] = [
        (twin_subject, object, twin_confidence, "https://example.test/twin-a"),
        (twin_subject, object, twin_confidence, "https://example.test/twin-b"),
        (
            triangulating_subject,
            object,
            0.90,
            "https://example.test/triangulating",
        ),
    ];
    let (contributor_records, contributor_pubkey_hex) =
        build_verifiable_peer_records_for_quadruples(
            contributor_did,
            contributor_seed,
            &contributor_quadruples,
        );
    // The two twins are records #0 and #1 (same pairing + confidence, distinct
    // evidence → distinct CIDs). The counter targets the FIRST twin; the SECOND is
    // its identical-subtotal un-countered twin.
    let countered_twin_cid = contributor_records[0].rkey.clone();
    let uncountered_twin_cid = contributor_records[1].rkey.clone();
    assert_ne!(
        countered_twin_cid, uncountered_twin_cid,
        "seed_score_breakdown_identical_subtotals_one_countered: the two identical-\
         subtotal twins must have DISTINCT CIDs (distinct evidence) so the store keeps \
         both rows and the counter targets exactly one; got {countered_twin_cid:?}"
    );

    // STEP 2 — a DISTINCT peer (Tobias) authors a verifiable COUNTER referencing the
    // FIRST twin's CID (self-counter is BLOCKED, so the counter MUST be a peer's).
    // When pulled it lands in `peer_claims` (under Tobias's own DID) and its
    // `counters` reference lands in `peer_claim_references` with `referenced_cid ==
    // countered_twin_cid` — the peer arm of the presence UNION-ALL.
    let tobias_seed = [9u8; 32];
    let (tobias_record, tobias_pubkey_hex) = build_verifiable_peer_counter_record(
        COUNTER_AUTHOR_TOBIAS,
        tobias_seed,
        &countered_twin_cid,
        Some(COUNTER_PEER_REASON_VERBATIM),
    );

    let contributor_pds = PeerPds::for_peer(contributor_did, contributor_records);
    let tobias_pds = PeerPds::for_peer(COUNTER_AUTHOR_TOBIAS, vec![tobias_record]);

    // STEP 3 — subscribe to BOTH peers via the real `peer add` verb, holding each PDS
    // ALIVE so a SINGLE `peer pull` over BOTH succeeds (single-pull discipline).
    for (did, pds) in [
        (contributor_did, &contributor_pds),
        (COUNTER_AUTHOR_TOBIAS, &tobias_pds),
    ] {
        let added = run_openlore_with_peer_resolver(
            env,
            &["peer", "add", did],
            did,
            pds.endpoint_url(),
        );
        assert_eq!(
            added.status, 0,
            "seed_score_breakdown_identical_subtotals_one_countered: peer add for {did} \
             must succeed;\n--- stdout ---\n{}\n--- stderr ---\n{}",
            added.stdout, added.stderr
        );
    }
    let pulled = run_openlore_pull_multi(
        env,
        &["peer", "pull"],
        &[
            PeerSeam {
                peer_did: contributor_did,
                peer_endpoint: contributor_pds.endpoint_url(),
                peer_pubkey_hex: &contributor_pubkey_hex,
            },
            PeerSeam {
                peer_did: COUNTER_AUTHOR_TOBIAS,
                peer_endpoint: tobias_pds.endpoint_url(),
                peer_pubkey_hex: &tobias_pubkey_hex,
            },
        ],
    );
    assert_eq!(
        pulled.status, 0,
        "seed_score_breakdown_identical_subtotals_one_countered: peer pull must \
         succeed;\n--- stdout ---\n{}\n--- stderr ---\n{}",
        pulled.stdout, pulled.stderr
    );

    // STEP 4 — recover every contribution CID in the contributor's breakdown (the
    // `peer_claims` rows attributed to its DID — Tobias's counter is under Tobias's
    // OWN DID, so it is excluded). Pin that BOTH twins survived (distinct CIDs → no
    // collision) and split the ONE countered twin from the rest (its identical-
    // subtotal twin + the triangulating contribution).
    let ordered_cids = read_score_contribution_cids(env, contributor_did);
    for twin in [&countered_twin_cid, &uncountered_twin_cid] {
        assert!(
            ordered_cids.contains(twin),
            "seed_score_breakdown_identical_subtotals_one_countered: the twin CID \
             {twin:?} must be among the contributor's contribution CIDs (both twins \
             must survive the pull); got {ordered_cids:?}"
        );
    }
    assert!(
        ordered_cids.len() >= 3,
        "seed_score_breakdown_identical_subtotals_one_countered: the trail must yield a \
         MULTI-row breakdown (the two identical-subtotal twins + the triangulating \
         contribution) so exactly one twin is countered and the rest are not; got \
         {ordered_cids:?}"
    );
    let uncountered_cids = ordered_cids
        .iter()
        .filter(|cid| *cid != &countered_twin_cid)
        .cloned()
        .collect::<Vec<_>>();
    assert!(
        uncountered_cids.contains(&uncountered_twin_cid),
        "seed_score_breakdown_identical_subtotals_one_countered: the un-countered \
         identical-subtotal twin {uncountered_twin_cid:?} must be in uncountered_cids \
         so the scenario can assert it renders the SAME subtotal but NO marker; got \
         {uncountered_cids:?}"
    );

    SeededScoreBreakdown {
        contributor_did: contributor_did.to_string(),
        ordered_cids,
        countered_cids: vec![countered_twin_cid],
        uncountered_cids,
    }
}

/// Seed a SCORED `/score` breakdown with a RICH-trail contributor but NO counters at
/// all (the no-noise + byte-identity-baseline fixture; AC-002-NO-NOISE / AC-SCORE-
/// BYTEID). `counter_presence_for` returns the EMPTY set → NO contribution row is
/// flagged, NO legend-induced diff vs the slice-09 baseline beyond the additive legend
/// markup (which the byte-identity gold elides). Returns the [`SeededScoreBreakdown`]
/// with an EMPTY `countered_cids` (every contribution un-countered).
///
/// SCAFFOLD: true (slice-14) — DELIVER seeds ONLY the contributor's rich trail (REUSE
/// the slice-09 `seed_contributor_rich_trail` shape via `build_verifiable_peer_
/// records_for_triples`), pulling NO counter peer; recovers every contribution CID
/// into `ordered_cids` + `uncountered_cids` with `countered_cids` EMPTY.
pub fn seed_score_breakdown_none_countered(env: &TestEnv) -> SeededScoreBreakdown {
    // The scored contributor reuses the slice-09 rich-trail shape VERBATIM (mirrors
    // `seed_score_breakdown_one_contribution_countered`): a DISTINCT peer DID whose
    // FOUR distinct subjects on the shared reproducible-builds object at varied
    // confidences → cross_project_span ≥ 2, a real weight + a MULTI-ROW breakdown,
    // NOT `[SPARSE]`. The ONLY difference from the one-countered seed: NO counter peer
    // is built or pulled, so EVERY contribution CID is un-countered (genuinely no
    // counters land in `peer_claim_references` — not a synthetic empty set).
    //
    // Confidences are STRICTLY DESCENDING in insertion order (0.90 > 0.86 > 0.74 >
    // 0.62), so each single-contribution pairing's weight descends in the same order —
    // the DB row order `read_score_contribution_cids` recovers therefore MATCHES the
    // render's weight-descending ranked order BY CONSTRUCTION (the byte-identity gold
    // pins `ordered_cids` against the rendered ranking; this mirrors the
    // `seed_score_breakdown_many_pairings_known_countered_subset` monotone-confidence
    // tactic that keeps insertion order == ranked render order).
    let contributor_did = CONTRIBUTOR_RICH_DID;
    let contributor_seed = [23u8; 32];
    let repro = SCORE_OBJECT_REPRODUCIBLE_BUILDS;
    let contributor_triples: [(&str, &str, f64); 4] = [
        ("github:NixOS/nixpkgs", repro, 0.90),
        ("github:bazelbuild/bazel", repro, 0.86),
        ("github:reproducible-builds/diffoscope", repro, 0.74),
        ("github:GNOME/meson", repro, 0.62),
    ];

    // STEP 1 — build the contributor's verifiable trail records UP FRONT (the same
    // REAL Ed25519 crypto + deterministic CID-recompute the pull pipeline verifies).
    let (contributor_records, contributor_pubkey_hex) =
        build_verifiable_peer_records_for_triples(
            contributor_did,
            contributor_seed,
            &contributor_triples,
        );
    let contributor_pds = PeerPds::for_peer(contributor_did, contributor_records);

    // STEP 2 — subscribe to the contributor via the real `peer add` verb (resolver
    // wired per peer), holding the PDS ALIVE for the whole function so the `peer pull`
    // succeeds. NO counter peer is added — there is genuinely NOTHING to counter.
    let added = run_openlore_with_peer_resolver(
        env,
        &["peer", "add", contributor_did],
        contributor_did,
        contributor_pds.endpoint_url(),
    );
    assert_eq!(
        added.status, 0,
        "seed_score_breakdown_none_countered: peer add for {contributor_did} must \
         succeed;\n--- stdout ---\n{}\n--- stderr ---\n{}",
        added.stdout, added.stderr
    );

    // STEP 3 — pull the contributor's trail (single peer; no counter rides this pull).
    let pulled = run_openlore_pull_multi(
        env,
        &["peer", "pull"],
        &[PeerSeam {
            peer_did: contributor_did,
            peer_endpoint: contributor_pds.endpoint_url(),
            peer_pubkey_hex: &contributor_pubkey_hex,
        }],
    );
    assert_eq!(
        pulled.status, 0,
        "seed_score_breakdown_none_countered: peer pull must succeed;\n\
         --- stdout ---\n{}\n--- stderr ---\n{}",
        pulled.stdout, pulled.stderr
    );

    // STEP 4 — recover every contribution CID in the contributor's breakdown (the
    // `peer_claims` rows attributed to its DID). With NO counter pulled, the
    // `peer_claim_references` counter arm is EMPTY → `counter_presence_for` returns the
    // empty set → no row is flagged. EVERY CID is un-countered; `countered_cids` is
    // genuinely empty (no synthetic empty set).
    let ordered_cids = read_score_contribution_cids(env, contributor_did);
    assert!(
        ordered_cids.len() >= 2,
        "seed_score_breakdown_none_countered: the rich trail must yield a MULTI-row \
         breakdown (≥2 contributions) so the no-noise + byte-identity baseline is \
         exercised over several un-countered rows; got {ordered_cids:?}"
    );

    SeededScoreBreakdown {
        contributor_did: contributor_did.to_string(),
        uncountered_cids: ordered_cids.clone(),
        ordered_cids,
        countered_cids: Vec::new(),
    }
}

/// Seed a SCORED `/score` breakdown spanning MANY pairings × MANY contributions with a
/// KNOWN countered subset (the N+1-flatten behavioral proxy fixture; AC-001-ONE-CALL /
/// AC-001-INVARIANT). Mirrors the slice-13 `seed_project_survey_many_groups_known_
/// countered_subset`: the breakdown is genuinely LARGE (multiple ranked pairings,
/// multiple contributions per pairing) so that a per-pairing or per-contribution
/// presence read would degrade or mis-flag under fan-out; this proxy pins that the
/// WHOLE breakdown is flagged correctly in ONE request from the single flattened
/// `counter_presence_for` call (DD-14-2 / ADR-051). Returns the [`SeededScoreBreakdown`].
///
/// SCAFFOLD: true (slice-14) — DELIVER seeds the contributor's trail across MANY
/// DISTINCT subjects/objects (→ many ranked pairings, many contributions), then a
/// DISTINCT peer counters a KNOWN SUBSET of those contribution CIDs, all riding the
/// ONE `peer pull`; returns the SeededScoreBreakdown with the known countered subset +
/// the un-countered remainder.
pub fn seed_score_breakdown_many_pairings_known_countered_subset(
    env: &TestEnv,
) -> SeededScoreBreakdown {
    // The scored contributor (a DISTINCT peer DID, so its rows land in `peer_claims`
    // attributed to it). Its trail spans MANY (subject, object) pairings: SEVERAL
    // DISTINCT philosophy objects, each asserted across SEVERAL DISTINCT subjects, so
    // the contributor's scoring feed decomposes into a genuinely LARGE multi-pairing /
    // multi-contribution breakdown (NOT `[SPARSE]` — every object spans ≥ 2 projects →
    // cross_project_span ≥ 2). A per-pairing or per-contribution presence read would
    // degrade or mis-flag under this fan-out; the ADR-051 single flattened
    // `counter_presence_for` call (every Contribution.cid across every WeightedPairing,
    // DD-14-2) must flag every countered contribution — and only those — in ONE request.
    let contributor_did = CONTRIBUTOR_RICH_DID;
    let contributor_seed = [23u8; 32];

    // THREE distinct philosophy objects, each across FOUR distinct subjects → a large,
    // multi-pairing breakdown (≥ 3 pairings, ≥ 12 contributions). Distinct confidences
    // keep every triple genuinely distinct (no canonical-CID aliasing → no row collision).
    let objects = [
        SCORE_OBJECT_REPRODUCIBLE_BUILDS,
        "org.openlore.philosophy.dependency-pinning",
        "org.openlore.philosophy.memory-safety",
    ];
    let subjects = [
        "github:bazelbuild/bazel",
        "github:NixOS/nixpkgs",
        "github:reproducible-builds/diffoscope",
        "github:GNOME/meson",
    ];
    let contributor_triples = objects
        .iter()
        .enumerate()
        .flat_map(|(obj_idx, object)| {
            subjects.iter().enumerate().map(move |(subj_idx, subject)| {
                // Confidences in (0,1], distinct per (object, subject): 0.86, 0.83, ...
                let confidence = 0.86 - ((obj_idx * subjects.len() + subj_idx) as f64) * 0.03;
                (*subject, *object, confidence)
            })
        })
        .collect::<Vec<_>>();

    // STEP 1 — build the contributor's verifiable trail records UP FRONT (the same REAL
    // Ed25519 crypto + deterministic CID-recompute the pull pipeline verifies). Each
    // record's `rkey` IS its deterministic CID, so we can pick a KNOWN SUBSET as the
    // counter targets BEFORE anything is pulled.
    let (contributor_records, contributor_pubkey_hex) =
        build_verifiable_peer_records_for_triples(
            contributor_did,
            contributor_seed,
            &contributor_triples,
        );

    // The KNOWN countered subset: contributions at indices 0, 5, and 10 — spread across
    // the THREE DISTINCT objects (index 0 in object #0, index 5 in object #1, index 10 in
    // object #2), so the targets fall in DISTINCT pairings. A per-pairing presence read
    // could not flag them all from a single pairing's CIDs — the flatten must collect
    // EVERY Contribution.cid across EVERY WeightedPairing into ONE call (ADR-051).
    let countered_indices = [0usize, 5, 10];
    let target_cids = countered_indices
        .iter()
        .map(|&i| {
            contributor_records
                .get(i)
                .unwrap_or_else(|| {
                    panic!(
                        "seed_score_breakdown_many_pairings_known_countered_subset: the \
                         contributor's trail record #{i} must exist (large multi-pairing trail)"
                    )
                })
                .rkey
                .clone()
        })
        .collect::<Vec<_>>();

    // STEP 2 — THREE DISTINCT counter authors, one per target (distinct seeds → distinct
    // keys; distinct target CIDs → distinct counter-record CIDs). Each lands in
    // `peer_claims` (under its OWN DID) and its `counters` reference lands in
    // `peer_claim_references` with `referenced_cid == its target` — the peer arm of the
    // presence UNION-ALL across DISTINCT pairings.
    let counter_authors: [(&str, [u8; 32]); 3] = [
        (COUNTER_AUTHOR_TOBIAS, [9u8; 32]),
        ("did:plc:uli-test", [11u8; 32]),
        ("did:plc:wren-test", [13u8; 32]),
    ];
    let counters = counter_authors
        .iter()
        .zip(target_cids.iter())
        .map(|((did, seed), target_cid)| {
            let (record, pubkey_hex) = build_verifiable_peer_counter_record(
                did,
                *seed,
                target_cid,
                Some(COUNTER_PEER_REASON_VERBATIM),
            );
            (*did, record, pubkey_hex)
        })
        .collect::<Vec<_>>();

    let contributor_pds = PeerPds::for_peer(contributor_did, contributor_records);
    let counter_pds = counters
        .iter()
        .map(|(did, record, _)| (*did, PeerPds::for_peer(did, vec![record.clone()])))
        .collect::<Vec<_>>();

    // STEP 3 — subscribe to every peer via the real `peer add` verb (resolver wired per
    // peer), holding each PDS ALIVE for the whole function, then `peer pull` ALL of them
    // in ONE invocation (single-pull discipline). The contributor IS a subscribed peer
    // (its large trail rides the federation path); the three counter authors are the
    // DISTINCT counters spread across pairings.
    let added = run_openlore_with_peer_resolver(
        env,
        &["peer", "add", contributor_did],
        contributor_did,
        contributor_pds.endpoint_url(),
    );
    assert_eq!(
        added.status, 0,
        "seed_score_breakdown_many_pairings_known_countered_subset: peer add for \
         {contributor_did} must succeed;\n--- stdout ---\n{}\n--- stderr ---\n{}",
        added.stdout, added.stderr
    );
    for (did, pds) in &counter_pds {
        let added =
            run_openlore_with_peer_resolver(env, &["peer", "add", did], did, pds.endpoint_url());
        assert_eq!(
            added.status, 0,
            "seed_score_breakdown_many_pairings_known_countered_subset: peer add for {did} \
             must succeed;\n--- stdout ---\n{}\n--- stderr ---\n{}",
            added.stdout, added.stderr
        );
    }

    let mut seams = vec![PeerSeam {
        peer_did: contributor_did,
        peer_endpoint: contributor_pds.endpoint_url(),
        peer_pubkey_hex: &contributor_pubkey_hex,
    }];
    for ((did, _record, pubkey_hex), (_, pds)) in counters.iter().zip(counter_pds.iter()) {
        seams.push(PeerSeam {
            peer_did: did,
            peer_endpoint: pds.endpoint_url(),
            peer_pubkey_hex: pubkey_hex,
        });
    }
    let pulled = run_openlore_pull_multi(env, &["peer", "pull"], &seams);
    assert_eq!(
        pulled.status, 0,
        "seed_score_breakdown_many_pairings_known_countered_subset: peer pull must \
         succeed;\n--- stdout ---\n{}\n--- stderr ---\n{}",
        pulled.stdout, pulled.stderr
    );

    // STEP 4 — recover every contribution CID in the contributor's breakdown (the
    // `peer_claims` rows attributed to its DID — every counter lives under ITS OWN DID,
    // so the counters are excluded by construction). Split the KNOWN countered subset
    // from the un-countered remainder. The N+1 proxy pins BOTH non-empty so the seed
    // cannot silently shrink.
    let ordered_cids = read_score_contribution_cids(env, contributor_did);
    for target_cid in &target_cids {
        assert!(
            ordered_cids.contains(target_cid),
            "seed_score_breakdown_many_pairings_known_countered_subset: the countered \
             target CID {target_cid:?} must be among the contributor's contribution CIDs; \
             got {ordered_cids:?}"
        );
    }
    assert!(
        ordered_cids.len() > target_cids.len(),
        "seed_score_breakdown_many_pairings_known_countered_subset: the large breakdown \
         must yield MORE contributions than the countered subset (a MIXED breakdown with \
         un-countered contributions too); got {} contribution(s), {} countered; \
         {ordered_cids:?}",
        ordered_cids.len(),
        target_cids.len()
    );
    let uncountered_cids = ordered_cids
        .iter()
        .filter(|cid| !target_cids.contains(cid))
        .cloned()
        .collect::<Vec<_>>();

    SeededScoreBreakdown {
        contributor_did: contributor_did.to_string(),
        ordered_cids,
        countered_cids: target_cids,
        uncountered_cids,
    }
}

/// Assert a rendered `/score` body (fragment OR full page) FLAGS the given countered
/// contribution row: the row carries the neutral "Countered" marker
/// ([`LIST_COUNTERED_FLAG_TEXT`]) rendered as the REUSED slice-13 `render_countered_
/// link` one-hop link `<a href="/claims/{cid}">Countered</a>` to that claim's slice-11
/// thread (US-CF-002 / AC-002-MARKER / AC-002-LINK). The SCORING-surface sibling of
/// [`assert_peer_claim_row_flagged_countered`] / [`assert_edge_flagged_countered`].
/// Scans the rendered HTML only (Mandate 8 universe = port-exposed rendered surface).
///
/// SCAFFOLD: true (slice-14).
pub fn assert_score_row_flagged_countered(body: &str, countered_cid: &str) {
    // The flag is the REUSED slice-13 `render_countered_link` render-only one-hop anchor
    // `<a href="/claims/{cid}">Countered</a>` (maud emits no whitespace inside the
    // element), rendered BESIDE the contribution's verbatim subtotal. Scan the rendered
    // HTML only (Mandate 8 universe = the port-exposed rendered surface).
    let marker = format!(
        "<a href=\"/claims/{countered_cid}\">{LIST_COUNTERED_FLAG_TEXT}</a>"
    );
    assert!(
        body.contains(&marker),
        "assert_score_row_flagged_countered: the countered /score contribution row for \
         {countered_cid:?} must carry the render-only marker {marker:?} (neutral presence \
         flag + one-hop link; US-CF-002 / AC-002-MARKER / AC-002-LINK); body was:\n{body}"
    );
}

/// Assert a rendered `/score` body does NOT flag the given un-countered contribution
/// row: the row for `uncountered_cid` carries NO "Countered" marker and NO empty-state
/// noise — it renders exactly as slice-09 (AC-002-NO-NOISE). The SCORING sibling of
/// [`assert_peer_claim_row_not_flagged`] / [`assert_edge_not_flagged`]. Scans the
/// rendered HTML only.
///
/// SCAFFOLD: true (slice-14).
pub fn assert_score_row_not_flagged(body: &str, uncountered_cid: &str) {
    // The un-countered contribution row carries NO `<a href="/claims/{cid}">Countered</a>`
    // flag anchor. (Its bare CID still appears in the row's CID cell — we assert the
    // absence of the FLAG anchor specifically, not the CID text.) Renders exactly as
    // slice-09 (AC-002-NO-NOISE / I-CF-2).
    let marker = format!(
        "<a href=\"/claims/{uncountered_cid}\">{LIST_COUNTERED_FLAG_TEXT}</a>"
    );
    assert!(
        !body.contains(&marker),
        "assert_score_row_not_flagged: the un-countered /score contribution row for \
         {uncountered_cid:?} must carry NO 'Countered' flag marker {marker:?} (renders \
         exactly as slice-09; AC-002-NO-NOISE / I-CF-2); body was:\n{body}"
    );
    // And NO "0 counters" / "no disagreement" empty-state noise anywhere on the page.
    for noise in ["0 counters", "no disagreement", "no counters"] {
        assert!(
            !body.to_lowercase().contains(noise),
            "assert_score_row_not_flagged: an un-countered /score breakdown must carry no \
             {noise:?} empty-state noise (no-noise discipline; AC-002-NO-NOISE); body \
             was:\n{body}"
        );
    }
}

/// Assert a `/score` contribution row countered by N (>1) authors shows EXACTLY ONE
/// neutral "Countered" marker — presence-only, NEVER a count ("countered by N" /
/// "disputed by N"), a verdict, or a merged consensus row (US-CF-002 /
/// AC-SCORE-PRESENCE). The SCORING sibling of
/// [`assert_peer_claim_flag_is_single_neutral_presence`] /
/// [`assert_edge_flag_is_single_neutral_presence`]. Scans the rendered HTML only.
///
/// SCAFFOLD: true (slice-14).
pub fn assert_score_flag_is_single_neutral_presence(body: &str, target_cid: &str) {
    // PRESENCE-only: a contribution countered by N distinct authors collapses (DISTINCT
    // referenced_cid) to ONE presence membership → EXACTLY ONE render-only "Countered"
    // marker on its breakdown row, NEVER one-per-counter and never a count. Count the
    // exact anchor occurrences for this CID — it must appear EXACTLY once. The SCORING
    // sibling of [`assert_edge_flag_is_single_neutral_presence`] /
    // [`assert_list_flag_is_single_neutral_presence`].
    let marker = format!(
        "<a href=\"/claims/{target_cid}\">{LIST_COUNTERED_FLAG_TEXT}</a>"
    );
    let occurrences = body.matches(&marker).count();
    assert_eq!(
        occurrences, 1,
        "assert_score_flag_is_single_neutral_presence: the /score contribution row for \
         {target_cid:?} (countered by ≥2 distinct authors) must carry EXACTLY ONE neutral \
         'Countered' marker (presence membership → one flag, DISTINCT referenced_cid; \
         AC-SCORE-PRESENCE), got {occurrences} occurrences of {marker:?}; body was:\n{body}"
    );

    // The flag is PRESENCE-only: the score body carries NONE of the count / verdict /
    // merged-judgement phrasings — never "countered by N", never a consensus or net
    // verdict, and (the SCORING-surface anti-misread extension) never a penalty/deduction
    // word that would misread the orthogonal flag as a score adjustment. Lowercased so a
    // capitalized variant can never sneak through (mirrors the LIST/EDGE blocklist plus
    // the score anti-misread words).
    let lowered = body.to_ascii_lowercase();
    for verdict in [
        // count-based / merged-judgement phrasing (never a count aggregated to a verdict)
        "countered by",
        "disputed by",
        "consensus",
        "net verdict",
        "people disagree",
        // verdict words — the flag asserts presence, never correctness of the counter
        "disputed",
        "refuted",
        "is false",
        "is wrong",
        // SCORING-surface anti-misread — the flag is SHOWN, never APPLIED to the score
        "penalty",
        "deduction",
        "lowered",
    ] {
        assert!(
            !lowered.contains(verdict),
            "assert_score_flag_is_single_neutral_presence: the /score 'Countered' flag is \
             presence-only — it must NEVER emit a count / verdict / merged-judgement / \
             penalty phrasing ({verdict:?}); a twice-countered contribution shows ONE \
             neutral marker, never 'countered by 2' (AC-SCORE-PRESENCE); body was:\n{body}"
        );
    }
}

/// Assert the "Countered" marker on a countered `/score` contribution row is a
/// render-only `<a href="/claims/{cid}">` ONE-HOP link to that claim's slice-11 thread
/// — navigation TEXT, never an executable write/sign/counter control (US-CF-002 /
/// AC-002-LINK). The SCORING sibling of [`assert_peer_claim_flag_links_to_thread`] /
/// [`assert_edge_flag_links_to_thread`].
///
/// SCAFFOLD: true (slice-14).
pub fn assert_score_flag_links_to_thread(body: &str, countered_cid: &str) {
    // The marker is the render-only one-hop anchor `<a href="/claims/{cid}">Countered</a>`
    // — navigation TEXT to the slice-11 thread, never an executable control (maud emits no
    // whitespace inside the element). Scan the rendered HTML only. The SCORING sibling of
    // [`assert_list_flag_links_to_thread`] / [`assert_edge_flag_links_to_thread`].
    let anchor = format!(
        "<a href=\"/claims/{countered_cid}\">{LIST_COUNTERED_FLAG_TEXT}</a>"
    );
    assert!(
        body.contains(&anchor),
        "assert_score_flag_links_to_thread: the 'Countered' marker on the countered /score \
         row for {countered_cid:?} must be the render-only one-hop link {anchor:?} to its \
         slice-11 thread (navigation TEXT, never a control; US-CF-002 / AC-002-LINK); body \
         was:\n{body}"
    );
    // The MARKER is a plain anchor — never an executable write/sign/counter control wrapping
    // the "Countered" text. (NB: the full `/score` page legitimately carries the chrome
    // search `<form method="get" action="/score">` + its submit `<button>` — that is NOT a
    // flag control, so we scope the check to the marker's immediate wrapping, not the whole
    // body.) The flag never renders the "Countered" text as a form/button/onclick control;
    // the only write path is the CLI (I-LF-1).
    for control in ["form", "button"] {
        let wrapped = format!("<{control}");
        let countered_control = format!("{wrapped} href=\"/claims/{countered_cid}\"");
        assert!(
            !body.contains(&countered_control),
            "assert_score_flag_links_to_thread: the /score flag must render the 'Countered' \
             marker as a render-only anchor, never an executable <{control}> control \
             ({countered_control:?}); the only write path is the CLI (I-LF-1); body was:\n{body}"
        );
    }
    // The marker carries no inline onclick handler (it is navigation TEXT only).
    let onclick_marker = format!("/claims/{countered_cid}\" onclick");
    assert!(
        !body.contains(&onclick_marker),
        "assert_score_flag_links_to_thread: the /score 'Countered' marker must be a \
         render-only anchor with NO onclick handler (navigation TEXT only; I-LF-1); body \
         was:\n{body}"
    );
}

/// Assert the running sum of a pairing's per-contribution subtotals STILL EQUALS its
/// displayed weight on a FLAGGED `/score` breakdown — the CARDINAL sum-to-weight
/// orthogonality gate (AC-SCORE-SUMWEIGHT). REUSES/extends the slice-09 reproduce-by-
/// hand parser ([`assert_score_html_breakdown_sums_to_displayed_weight`]) but on a
/// breakdown WHERE a contribution carries the additive "Countered" marker: the marker
/// is elided (so the subtotal parse is unaffected) and the per-row subtotals must
/// STILL sum to the displayed weight, AND the countered contribution's subtotal must
/// be its FULL original value (the counter subtracts nothing). Scans the OBSERVABLE
/// rendered HTML only.
///
/// SCAFFOLD: true (slice-14) — DELIVER elides every additive
/// `<a href="/claims/{cid}">Countered</a>` marker from the flagged body, then reuses
/// the slice-09 `parse_score_pairings` + Σ-subtotal==weight check on the remaining
/// slice-09 breakdown markup; ALSO asserts the countered contribution's parsed
/// subtotal equals its un-countered twin's (FULL original value preserved).
pub fn assert_score_html_breakdown_sums_to_weight_with_flag(
    body: &str,
    countered_cids: &[String],
) {
    // ELIDE every additive `<a href="/claims/{cid}">Countered</a>` marker from the
    // FLAGGED body so the subtotal parse sees the UNCHANGED slice-09 breakdown markup
    // (the marker is additive markup BESIDE the verbatim subtotal cell; eliding it must
    // leave the per-row subtotal value untouched).
    let mut elided = body.to_string();
    for cid in countered_cids {
        let marker = format!("<a href=\"/claims/{cid}\">{LIST_COUNTERED_FLAG_TEXT}</a>");
        elided = elided.replace(&marker, "");
    }
    // After eliding the markers, NO residual flag anchor for any countered CID may
    // survive (a stray marker inside a subtotal cell would corrupt the parse).
    for cid in countered_cids {
        let marker = format!("<a href=\"/claims/{cid}\">{LIST_COUNTERED_FLAG_TEXT}</a>");
        assert!(
            !elided.contains(&marker),
            "assert_score_html_breakdown_sums_to_weight_with_flag: every additive marker \
             {marker:?} must elide cleanly so the remaining body is the slice-09 \
             breakdown; residual marker survived in:\n{elided}"
        );
    }
    // On the marker-elided breakdown, the per-contribution subtotals STILL sum to the
    // displayed pairing weight — the CARDINAL sum-to-weight orthogonality gate, reusing
    // the slice-09 reproduce-by-hand parser VERBATIM. Because the countered
    // contribution's subtotal cell is byte-identical to slice-09 once the marker is
    // elided, this proves the counter subtracts NOTHING (the contribution keeps its
    // FULL original value; AC-SCORE-SUMWEIGHT).
    assert_score_html_breakdown_sums_to_displayed_weight(&elided);
}

/// Assert the scored `/score` breakdown carries the anti-misread legend
/// ([`SCORE_COUNTER_LEGEND_TEXT`]) EXACTLY ONCE and that the WHOLE rendered body is
/// BLOCKLIST-CLEAN — it contains NONE of the verdict/penalty/subtraction words on
/// [`SCORE_LEGEND_BLOCKLIST`] ("disputed"/"refuted"/"false"/"penalty"/"deduction"/
/// "lowered"/"disputed score"). The orthogonality copy makes the flag unmistakably
/// SHOWN-not-APPLIED (AC-SCORE-ANTIMISREAD). Scans the rendered HTML only.
///
/// SCAFFOLD: true (slice-14) — DELIVER asserts the body contains
/// `SCORE_COUNTER_LEGEND_TEXT` exactly once (a scored breakdown → one legend, ABOVE
/// the pairings; never per row/pairing), AND the lowercased body contains none of
/// `SCORE_LEGEND_BLOCKLIST`.
pub fn assert_score_legend_present_and_blocklist_clean(body: &str) {
    // A scored breakdown carries the anti-misread legend EXACTLY ONCE (ABOVE the
    // pairings; ADR-051 §6.3 / DD-14-3 — one legend per scored breakdown, never per
    // row/pairing). The legend copy mirrors the production `viewer-domain::
    // SCORE_COUNTER_LEGEND` SSOT (held here as `SCORE_COUNTER_LEGEND_TEXT` so the
    // assert never drifts).
    let occurrences = body.matches(SCORE_COUNTER_LEGEND_TEXT).count();
    assert_eq!(
        occurrences, 1,
        "assert_score_legend_present_and_blocklist_clean: the scored /score breakdown \
         must carry the anti-misread legend EXACTLY ONCE (one legend per scored \
         breakdown, never per row/pairing; AC-SCORE-ANTIMISREAD); found {occurrences} \
         occurrence(s) of {SCORE_COUNTER_LEGEND_TEXT:?}; body was:\n{body}"
    );
    // The WHOLE rendered body is BLOCKLIST-CLEAN — it contains NONE of the
    // verdict/penalty/subtraction words a reader could misread as a score deduction.
    // Lowercased at the comparison site so a capitalized variant cannot sneak through.
    let lowered = body.to_ascii_lowercase();
    for banned in SCORE_LEGEND_BLOCKLIST {
        assert!(
            !lowered.contains(banned),
            "assert_score_legend_present_and_blocklist_clean: the scored /score body must \
             be blocklist-clean — it must NEVER contain the verdict/penalty word \
             {banned:?} (a reader must not misread the flag as a score deduction; \
             AC-SCORE-ANTIMISREAD); body was:\n{body}"
        );
    }
}

/// Assert the `/score` body does NOT carry the anti-misread legend
/// ([`SCORE_COUNTER_LEGEND_TEXT`]) — for the `NoClaims` arm where there is NO scored
/// breakdown at all the legend is NOT rendered (it governs markers that do not appear;
/// DD-14-3 placement: `render_score_result`'s `Scored` arm only). Used by the
/// no-claims scenario so the legend is provably scoped to scored breakdowns. Scans the
/// rendered HTML only.
///
/// SCAFFOLD: true (slice-14).
pub fn assert_score_legend_absent(body: &str) {
    let _ = body;
    todo!(
        "slice-14 RED scaffold: assert the /score body does NOT carry \
         SCORE_COUNTER_LEGEND_TEXT (the NoClaims arm renders no scored breakdown → no \
         legend; DD-14-3 placement)"
    );
}

/// Assert the flagged `/score` render is byte-identical to the slice-09 reference
/// render of the SAME store in every weight / confidence / author bonus / triangulation
/// bonus / subtotal / headline total / bucket / `[SPARSE]` line / pairing ranking /
/// contribution row order EXCEPT the additive "Countered" markers AND the additive
/// [`SCORE_COUNTER_LEGEND_TEXT`] legend — the flag changes NOTHING about the score
/// (the CARDINAL byte-identity gold, AC-SCORE-BYTEID / I-CF-9). Uses the slice-12/13
/// baseline+marker-elision tactic extended to ALSO elide the legend: elide every
/// additive "Countered" anchor AND the legend string, then prove the remaining
/// slice-09 body honours the recorded ranked render order (`ordered_cids`, strictly
/// increasing byte offsets) byte-for-byte. Scans the rendered HTML only.
///
/// SCAFFOLD: true (slice-14) — DELIVER elides every
/// `<a href="/claims/{cid}">Countered</a>` anchor (for every recorded CID) AND the
/// `SCORE_COUNTER_LEGEND_TEXT` legend string from the flagged body, asserts no residual
/// marker/legend survives, then pins every recorded contribution CID present + in
/// strictly-increasing byte order (the slice-09 ranked order) — the flag + legend are
/// purely additive markup.
pub fn assert_score_render_byte_identical_to_slice09(flagged: &str, ordered_cids: &[String]) {
    // ELIDE the additive markup the flag adds over the slice-09 baseline: (1) every
    // `<a href="/claims/{cid}">Countered</a>` anchor (rendered BESIDE the verbatim
    // subtotal by the REUSED slice-13 `render_countered_link`; un-countered rows carry
    // none, so the replace is a no-op there — the helper stays agnostic to WHICH rows
    // were flagged) AND (2) the `SCORE_COUNTER_LEGEND_TEXT` legend (rendered ONCE ABOVE
    // the pairings in the `Scored` arm). What remains IS the slice-09 `/score` byte-stream
    // for this store (the slice-12/13 baseline+marker-elision tactic extended to the legend).
    let mut elided = flagged.to_string();
    for cid in ordered_cids {
        let anchor = format!("<a href=\"/claims/{cid}\">{LIST_COUNTERED_FLAG_TEXT}</a>");
        elided = elided.replace(&anchor, "");
    }
    elided = elided.replace(SCORE_COUNTER_LEGEND_TEXT, "");

    // No additive marker survives the elision — the remaining body is pure slice-09. A
    // residual marker would mean the flag is NOT purely the recorded `<a href>` anchor.
    assert!(
        !elided.contains(LIST_COUNTERED_FLAG_TEXT),
        "assert_score_render_byte_identical_to_slice09: every additive \
         {LIST_COUNTERED_FLAG_TEXT:?} anchor must elide cleanly so the remaining body is the \
         slice-09 /score render; a residual marker means the flag is NOT purely the recorded \
         `<a href>` anchor (AC-SCORE-BYTEID / I-CF-9); elided body was:\n{elided}"
    );
    // No residual legend survives either — the legend is purely additive markup.
    assert!(
        !elided.contains(SCORE_COUNTER_LEGEND_TEXT),
        "assert_score_render_byte_identical_to_slice09: the additive anti-misread legend \
         must elide cleanly so the remaining body is the slice-09 /score render (the legend \
         is purely additive, never perturbs a number/order; AC-SCORE-BYTEID); elided body \
         was:\n{elided}"
    );

    // ROW ORDER + RANKING byte-identity: every recorded contribution CID appears in the
    // elided (no-flag) body, and their first-seen byte offsets are STRICTLY INCREASING in
    // the recorded slice-09 ranked render order — the flag re-ordered / re-ranked NOTHING
    // (AC-SCORE-BYTEID / I-CF-9). Because eliding the anchor leaves each subtotal cell
    // byte-identical to slice-09, this also pins every weight/confidence/bonus/subtotal/
    // total/bucket/[SPARSE]-line unchanged.
    let mut prev_offset: Option<usize> = None;
    for cid in ordered_cids {
        let offset = elided.find(cid.as_str()).unwrap_or_else(|| {
            panic!(
                "assert_score_render_byte_identical_to_slice09: the elided slice-09 /score \
                 body must contain every recorded contribution CID; {cid:?} was missing — the \
                 flag dropped/replaced a row (a re-rank/re-order regression); elided body \
                 was:\n{elided}"
            )
        });
        if let Some(prev) = prev_offset {
            assert!(
                offset > prev,
                "assert_score_render_byte_identical_to_slice09: the elided slice-09 /score \
                 contribution row order must follow the recorded ranked render order \
                 {ordered_cids:?} VERBATIM — {cid:?} rendered out of position, so the flag \
                 RE-ORDERED / RE-RANKED the breakdown (AC-SCORE-BYTEID / I-CF-9); elided body \
                 was:\n{elided}"
            );
        }
        prev_offset = Some(offset);
    }
}

// =============================================================================
// slice-15 (viewer-peer-subscriptions) — the read-only `GET /peers` view (US-PS-002/003;
// ADR-052). The new route lists the operator's ACTIVE subscriptions (`peer_subscriptions`
// WHERE `removed_at IS NULL`) — each peer's DID VERBATIM + its PER-PEER local claim count
// (`COUNT(pc.cid) FROM peer_claims WHERE author_did = peer_did`) — plus a RENDER-ONLY
// `openlore peer remove <did>` revocation command per peer (mirrors the slice-08
// `render_follow_guidance` render-only `openlore peer add` precedent). A guided empty
// state when there are none. The read is ONE aggregate query, invariant to peer count
// (no N+1; DD-PS-1). Active-only / residue-made-visible (I-PS-2): a `peer remove`d peer
// VANISHES even though its cached `peer_claims` survive on disk.
//
// These seeds drive the SAME production federation write path slices 03/09/10 use — the
// real `peer add` + `peer pull` verbs against `PeerPds` doubles (built with
// `build_verifiable_peer_records_for_triples`) — so the rows the `/peers` read returns are
// produced by production code, never hand-inserted (Pillar 3 / BR-VIEW-4). A
// subscribe-but-never-pull seed (`seed_peer_subscribed_zero_claims`) drives `peer add`
// ALONE (no pull) so the LEFT JOIN + `COUNT(pc.cid)` zero-claims design (DD-PS-2) is
// exercised. A `seed_peer_subscribed_then_removed` runs `peer add` + `peer pull` THEN the
// real `peer remove` verb (soft-remove, no `--purge`) so the cached claims survive while
// the subscription's `removed_at` is set — the precondition for the active-only filter.
//
// The asserts scan ONLY the rendered HTML the operator's browser shows (Mandate 8 universe
// = port-exposed rendered surface, never an internal `viewer-domain` struct field). NO
// scenario calls `list_active_peer_subscriptions` / `render_peers_*` directly — every
// assertion is on the `/peers` HTTP response (Mandate 1, driving-port discipline). The
// read-only / no-write / offline-chrome / offline-data / N+1 gold invariants REUSE the
// slice-06/08/10 `capture_store_row_count_universe` + `assert_store_read_only` +
// `references_external_cdn` harness VERBATIM.
//
// SCAFFOLD: true (slice-15) — the seeds + asserts COMPILE now (they drive EXISTING
// `peer add`/`peer pull`/`peer remove` verbs + scan strings); the SCENARIOS stay RED
// because the production `/peers` route + `list_active_peer_subscriptions` read +
// `PeersView` / `render_peers_*` / `render_remove_guidance` seams do NOT exist yet (the
// route 404s / renders no `#peers` region). RED = MISSING_FUNCTIONALITY, never BROKEN.
// =============================================================================

/// The `/peers` route path (the slice-15 net-new read-only route; DD-PS-7).
pub const PEERS_PATH: &str = "/peers";

/// The swap-target id the `/peers` fragment carries (mirrors the slice-10
/// `TRAVERSAL_RESULTS_ID` / the slice-08 search region id). The htmx fragment IS this
/// region; the no-JS full page EMBEDS it (parity by construction, I-PS-5).
pub const PEERS_REGION_ID: &str = "peers";

/// The two ACTIVE peers the walking-skeleton + anti-merging + per-peer-count scenarios
/// follow: Rachel (5 cached claims) and Tobias (3 cached claims). REAL DIDs, REUSED from
/// the slice-09/10 traversal constants so the seeded attribution shape is consistent
/// across the viewer slices.
pub const PEERS_RACHEL_DID: &str = TRAVERSAL_AUTHOR_RACHEL; // did:plc:rachel-test
pub const PEERS_TOBIAS_DID: &str = TRAVERSAL_AUTHOR_TOBIAS; // did:plc:tobias-test
pub const PEERS_RACHEL_CLAIM_COUNT: usize = 5;
pub const PEERS_TOBIAS_CLAIM_COUNT: usize = 3;

/// A subscribed-but-never-pulled peer (count 0 — proves the LEFT JOIN + `COUNT(pc.cid)`
/// design keeps the row at 0, DD-PS-2). REAL DID.
pub const PEERS_NEWPEER_DID: &str = "did:plc:newpeer-test";

/// The render-only revocation command verb shape (the slice-03 `peer remove` verb, the
/// SINGLE source of truth held in the production `PEER_REMOVE_GUIDANCE_PREFIX`; mirrors the
/// slice-08 `openlore peer add` follow-guidance shape). The assert scans for
/// `openlore peer remove <bare-did>` as render-only TEXT.
pub const PEER_REMOVE_COMMAND_VERB: &str = "openlore peer remove";

/// The empty-state starting command verb shape (`openlore peer add`, the slice-03
/// subscribe verb; the production `PEER_ADD_GUIDANCE_PREFIX`).
pub const PEER_ADD_COMMAND_VERB: &str = "openlore peer add";

/// A held subscription seam: a subscribed peer's `PeerPds` double kept ALIVE so the
/// subscription stays resolvable for the lifetime of the test. Returned by the
/// zero-claims seed (subscribe-without-pull) so the caller can keep the PDS alive while
/// the `/peers` read runs. Dropping it tears down the peer's PDS (harmless after the
/// subscription row is written — the `/peers` read is LOCAL and never re-resolves).
pub struct HeldSubscriptions {
    _peers: Vec<PeerPds>,
}

/// Seed TWO ACTIVE peers with KNOWN per-peer claim counts through the PRODUCTION
/// federation write path: Rachel (5 cached claims) and Tobias (3 cached claims), each via
/// the real `peer add` + `peer pull` verbs (DISTINCT subjects per triple so the canonical
/// CIDs do not alias). The walking-skeleton + anti-merging + per-peer-count seed
/// (US-PS-002 Ex 1; AC theme 1/7). After seeding, the `peer_subscriptions` table holds two
/// ACTIVE rows and `peer_claims` holds 5 Rachel + 3 Tobias rows — exactly what the new
/// `list_active_peer_subscriptions` read must surface as two attributed rows with counts
/// 5 and 3 (NEVER a merged 8). The counts are pinned with `assert_peer_claims_row_count_for`
/// so the fixture is the GENUINE per-peer shape, not merely "the verbs exited 0".
///
/// SCAFFOLD: true (slice-15) — drives the EXISTING `peer add` + `peer pull` verbs via
/// `seed_peer_authored_graph` (the SAME seam slice-09/10 use); the rows land in the REAL
/// `peer_subscriptions` + `peer_claims` tables the viewer's LOCAL `/peers` read returns.
pub fn seed_peers_two_active_with_claims(env: &TestEnv) {
    let dep = "org.openlore.philosophy.dependency-pinning";
    let repro = "org.openlore.philosophy.reproducible-builds";
    let workspace = "org.openlore.philosophy.workspace-cohesion";
    let memory = "org.openlore.philosophy.memory-safety";
    let actor = "org.openlore.philosophy.actor-model";

    // Rachel: 5 DISTINCT triples (5 cached claims). Tobias: 3 DISTINCT triples (3 cached
    // claims). DISTINCT (subject, object) per triple so the canonical CIDs are distinct
    // (the store keys on cid; identical triples would collide into one row). Materialized
    // through the PRODUCTION `peer add` + `peer pull` path (ONE pull over BOTH peers).
    let graph = seed_peer_authored_graph(
        env,
        &[
            SeedPeer {
                peer_did: PEERS_RACHEL_DID,
                seed: [7u8; 32],
                triples: &[
                    ("github:rust-lang/cargo", dep, 0.90),
                    ("github:NixOS/nixpkgs", repro, 0.74),
                    ("github:bazelbuild/bazel", workspace, 0.61),
                    ("github:rust-lang/rust", memory, 0.88),
                    ("github:erlang/otp", actor, 0.55),
                ],
            },
            SeedPeer {
                peer_did: PEERS_TOBIAS_DID,
                seed: [9u8; 32],
                triples: &[
                    ("github:denoland/deno", dep, 0.42),
                    ("github:torvalds/linux", repro, 0.71),
                    ("github:GNOME/meson", workspace, 0.33),
                ],
            },
        ],
    );
    drop(graph);

    // Pin the GENUINE per-peer cached-claim shape so the fixture is the real two-peer
    // distinct-count state (5 vs 3), not merely "the verbs exited 0" — the load-bearing
    // anti-merging precondition (J-003a / I-PS-3).
    assert_peer_claims_row_count_for(env, PEERS_RACHEL_DID, PEERS_RACHEL_CLAIM_COUNT);
    assert_peer_claims_row_count_for(env, PEERS_TOBIAS_DID, PEERS_TOBIAS_CLAIM_COUNT);
}

/// Seed a peer SUBSCRIBED but NEVER pulled (active subscription, ZERO cached claims) via
/// the real `peer add` verb ALONE (no `peer pull`). Proves the LEFT JOIN + `COUNT(pc.cid)`
/// design (DD-PS-2): a never-pulled peer stays in the `/peers` result at count 0 — not
/// dropped (inner-JOIN bug), not counted as 1 (`COUNT(*)`-of-NULL bug). US-PS-002 Ex 2.
/// Returns the held `PeerPds` so the caller keeps it alive while seeding (the subscription
/// row is already written by `peer add`; the `/peers` read is LOCAL + never re-resolves,
/// so the PDS may drop after this returns — the handle is returned for symmetry / explicit
/// lifetime control).
///
/// SCAFFOLD: true (slice-15) — drives the EXISTING `peer add` verb (resolver wired for the
/// peer) with NO pull; one ACTIVE `peer_subscriptions` row, ZERO `peer_claims` rows.
pub fn seed_peer_subscribed_zero_claims(env: &TestEnv) -> HeldSubscriptions {
    // Build a verifiable record set + start the peer's PDS so `peer add` can resolve the
    // DID and register the subscription. We deliberately do NOT `peer pull`, so NO
    // `peer_claims` row lands — the active subscription has a per-peer count of 0.
    let (records, _pubkey_hex) =
        build_verifiable_peer_records_for_triples(PEERS_NEWPEER_DID, [13u8; 32], &[(
            "github:newpeer/repo",
            "org.openlore.philosophy.dependency-pinning",
            0.50,
        )]);
    let pds = PeerPds::for_peer(PEERS_NEWPEER_DID, records);

    // Subscribe via the real `peer add` verb (resolver wired for THIS peer) — NO pull.
    let added = run_openlore_with_peer_resolver(
        env,
        &["peer", "add", PEERS_NEWPEER_DID],
        PEERS_NEWPEER_DID,
        pds.endpoint_url(),
    );
    assert_eq!(
        added.status, 0,
        "seed_peer_subscribed_zero_claims: `peer add {PEERS_NEWPEER_DID}` must succeed;\n\
         --- stdout ---\n{}\n--- stderr ---\n{}",
        added.stdout, added.stderr
    );

    // Pin the GENUINE zero-claims shape: ONE active subscription, ZERO cached claims (the
    // never-pulled state the LEFT JOIN + COUNT(pc.cid) design must keep at count 0).
    assert_one_active_subscription_for(env, PEERS_NEWPEER_DID);
    assert_peer_claims_row_count_for(env, PEERS_NEWPEER_DID, 0);

    HeldSubscriptions { _peers: vec![pds] }
}

/// Seed a peer SUBSCRIBED + PULLED, THEN soft-removed via the real `peer remove` verb (no
/// `--purge`): the subscription row's `removed_at` is set, but the cached `peer_claims`
/// rows SURVIVE on disk. The active-only / residue-made-visible precondition (I-PS-2 /
/// US-PS-002 Ex 3): a `peer remove`d peer must be ABSENT from `/peers` on the next render
/// even though its cached claims remain (no `--purge`). REUSES the slice-03 soft-remove
/// storage contract pinned by `assert_subscription_soft_removed_for` +
/// `assert_peer_claims_row_count_for` (the cache is RETAINED).
///
/// SCAFFOLD: true (slice-15) — drives the EXISTING `peer add` + `peer pull` + `peer remove`
/// verbs; the soft-removed subscription is residue the `/peers` read must EXCLUDE while its
/// cached claims remain on disk.
pub fn seed_peer_subscribed_then_removed(env: &TestEnv) {
    let dep = "org.openlore.philosophy.dependency-pinning";
    let repro = "org.openlore.philosophy.reproducible-builds";

    // Subscribe + pull Rachel (2 cached claims) through the PRODUCTION federation path.
    let graph = seed_peer_authored_graph(
        env,
        &[SeedPeer {
            peer_did: PEERS_RACHEL_DID,
            seed: [7u8; 32],
            triples: &[
                ("github:rust-lang/cargo", dep, 0.90),
                ("github:NixOS/nixpkgs", repro, 0.74),
            ],
        }],
    );
    let cached = graph.seeded.len();
    drop(graph);

    // Soft-remove Rachel via the real `peer remove` verb (no `--purge`): sets `removed_at`,
    // RETAINS the cached `peer_claims` rows (slice-03 WD-25 / ADR-014).
    let removed = run_openlore(env, &["peer", "remove", PEERS_RACHEL_DID]);
    assert_eq!(
        removed.status, 0,
        "seed_peer_subscribed_then_removed: `peer remove {PEERS_RACHEL_DID}` (soft, no \
         --purge) must succeed;\n--- stdout ---\n{}\n--- stderr ---\n{}",
        removed.stdout, removed.stderr
    );

    // Pin the GENUINE residue state: the subscription is soft-removed (`removed_at IS NOT
    // NULL`) AND every cached peer claim is RETAINED on disk (no --purge). This is the
    // precondition the active-only `/peers` filter must render as ABSENCE.
    assert_subscription_soft_removed_for(env, PEERS_RACHEL_DID);
    assert_peer_claims_row_count_for(env, PEERS_RACHEL_DID, cached);
}

/// Seed a store with NO active subscriptions (the guided empty-state precondition;
/// US-PS-003 Ex 1). A no-op over a fresh `TestEnv::initialized()` store (no `peer add` ever
/// run), named explicitly so the empty-state scenarios read in the domain language and the
/// intent ("the operator follows no one") is legible at the call site.
///
/// SCAFFOLD: true (slice-15) — the empty store IS the precondition; the `/peers` read must
/// return an empty result (not an error) and the viewer must render the guided empty state.
pub fn seed_no_active_subscriptions(_env: &TestEnv) {
    // Intentionally empty: a freshly `initialized()` store has zero `peer_subscriptions`
    // rows. The named seed documents the empty-state precondition at the call site.
}

/// Seed a store whose ONLY subscription was soft-removed (US-PS-003 Ex 2): there is ONE
/// `peer_subscriptions` row, soft-removed (`removed_at` set), and the operator follows no
/// one ELSE. The active-only filter must yield an EMPTY active set → the SAME guided empty
/// state (the soft-removed row is residue, not an active subscription). Distinct from
/// `seed_peer_subscribed_then_removed` (which leaves the removed peer's CACHE on disk too —
/// here that cache also remains, but no OTHER active peer exists, so `/peers` is empty).
///
/// SCAFFOLD: true (slice-15) — drives the EXISTING `peer add` + `peer pull` + `peer remove`
/// verbs; REUSES `seed_peer_subscribed_then_removed` (the single soft-removed peer IS the
/// only-subscription-removed shape).
pub fn seed_only_subscription_removed(env: &TestEnv) {
    // The single soft-removed peer IS the only-subscription-removed shape: one
    // `peer_subscriptions` row, soft-removed, no other active peer. REUSE the residue seed
    // (the chained-narrative precondition: Given+When of the residue scenario = Given of
    // the empty-state-via-residue scenario, Pillar 2).
    seed_peer_subscribed_then_removed(env);
}

/// Seed `N` ACTIVE peers with KNOWN per-peer claim counts for the N+1 behavioral proxy
/// (US-PS-001 single-aggregate-query / I-PS-8): the `/peers` read must resolve the whole
/// active set + every per-peer count in ONE aggregate query, invariant to peer count. The
/// behavioral proxy mirrors the slice-10/13/14 N+1 proxies — a MULTI-peer breakdown
/// resolved correctly in ONE request (the strict 1-query bound is the DELIVER adapter-duckdb
/// unit/property test). Returns the seeded `(did, count)` pairs in seeded order so the
/// scenario can assert every peer row is present with its correct per-peer count.
///
/// SCAFFOLD: true (slice-15) — drives the EXISTING `peer add` + `peer pull` verbs over `N`
/// peers in ONE pull (single-pull discipline); each peer gets a DISTINCT, KNOWN number of
/// cached claims so a wrong/merged/N+1 count is detectable.
pub fn seed_many_active_peers_known_counts(env: &TestEnv) -> Vec<(String, usize)> {
    let dep = "org.openlore.philosophy.dependency-pinning";
    let repro = "org.openlore.philosophy.reproducible-builds";
    let workspace = "org.openlore.philosophy.workspace-cohesion";
    let memory = "org.openlore.philosophy.memory-safety";

    // FOUR active peers with DISTINCT known counts (4, 3, 2, 1) so the per-peer breakdown
    // is non-trivial and a merged/dropped/N+1-miscount is detectable in ONE request.
    let alice = "did:plc:alice-test";
    let bob = "did:plc:bob-test";
    let carol = "did:plc:carol-test";
    let dave = "did:plc:dave-test";

    let graph = seed_peer_authored_graph(
        env,
        &[
            SeedPeer {
                peer_did: alice,
                seed: [31u8; 32],
                triples: &[
                    ("github:a/one", dep, 0.90),
                    ("github:a/two", repro, 0.80),
                    ("github:a/three", workspace, 0.70),
                    ("github:a/four", memory, 0.60),
                ],
            },
            SeedPeer {
                peer_did: bob,
                seed: [33u8; 32],
                triples: &[
                    ("github:b/one", dep, 0.55),
                    ("github:b/two", repro, 0.45),
                    ("github:b/three", workspace, 0.35),
                ],
            },
            SeedPeer {
                peer_did: carol,
                seed: [35u8; 32],
                triples: &[("github:c/one", dep, 0.66), ("github:c/two", repro, 0.44)],
            },
            SeedPeer {
                peer_did: dave,
                seed: [37u8; 32],
                triples: &[("github:d/one", dep, 0.50)],
            },
        ],
    );
    drop(graph);

    let expected = vec![
        (alice.to_string(), 4usize),
        (bob.to_string(), 3usize),
        (carol.to_string(), 2usize),
        (dave.to_string(), 1usize),
    ];
    // Pin the GENUINE per-peer cached-claim shape so the proxy is over a REAL non-trivial
    // multi-peer breakdown (a wrong count is a real bug, not a seeding artifact).
    for (did, count) in &expected {
        assert_peer_claims_row_count_for(env, did, *count);
    }
    expected
}

/// Universe-bound: "the `peer_subscriptions` store holds exactly ONE ACTIVE row
/// (`removed_at IS NULL`) for `peer_did`". Port-exposed name:
/// `peer_storage.subscriptions.active_row_count[did] == 1`. The active-subscription
/// sibling of `assert_subscription_soft_removed_for`; used by the zero-claims seed to pin
/// the subscribe-without-pull state (one active row, no cache).
pub fn assert_one_active_subscription_for(env: &TestEnv, peer_did: &str) {
    let db_path = env.duckdb_path();
    let conn = duckdb::Connection::open(&db_path).unwrap_or_else(|err| {
        panic!(
            "open DuckDB at {} for active-subscription assertion: {err}",
            db_path.display()
        )
    });

    let active: i64 = conn
        .query_row(
            "SELECT count(*) FROM peer_subscriptions \
             WHERE peer_did = ? AND removed_at IS NULL",
            duckdb::params![peer_did],
            |r| r.get(0),
        )
        .unwrap_or_else(|err| panic!("query active peer_subscriptions for {peer_did}: {err}"));

    assert_eq!(
        active, 1,
        "expected exactly ONE ACTIVE subscription row (removed_at IS NULL) for {peer_did}; \
         got {active} (subscribe-without-pull must leave one active row)"
    );
}

/// Assert the `/peers` rendered HTML carries a row for `peer_did` showing its DID VERBATIM
/// AND its per-peer `local_claim_count` (US-PS-002; AC theme 1/7/8). Universe (port-exposed
/// rendered surface): the rendered body contains the DID verbatim and the count. The count
/// is asserted PER-PEER — a merged total would render a DIFFERENT number, so a 5/3 →
/// merged-8 regression fails (the count this peer's row shows must be ITS OWN count).
///
/// SCAFFOLD: true (slice-15).
pub fn assert_peer_row_present(body: &str, peer_did: &str, count: usize) {
    // The peer DID is rendered VERBATIM (attribution discipline, I-PS-3 — never elided,
    // never merged into a faceless "all peers" row).
    assert!(
        body.contains(peer_did),
        "US-PS-002 (I-PS-3): the /peers render must show a row attributing {peer_did:?} \
         VERBATIM (each peer is its own attributed row keyed by its DID); body was:\n{body}"
    );
    // The PER-PEER local claim count is rendered for THIS peer (never a merged total). The
    // count appears as a standalone token in the body — a 5/3 → merged-8 regression would
    // show the wrong number on this peer's row.
    let count_str = count.to_string();
    assert!(
        body.contains(&count_str),
        "US-PS-002 (J-003a / I-PS-3): the /peers row for {peer_did:?} must show its PER-PEER \
         local claim count {count} (never a merged total); body was:\n{body}"
    );
}

/// Assert the `/peers` rendered HTML does NOT mention `peer_did` at all — the
/// active-only / residue-made-visible guarantee (I-PS-2 / US-PS-002 Ex 3 / US-PS-003 Ex 2):
/// a `peer remove`d (soft-removed) peer VANISHES from `/peers` even though its cached
/// `peer_claims` remain on disk. Universe (port-exposed rendered surface): the DID does NOT
/// appear in the rendered body.
///
/// SCAFFOLD: true (slice-15).
pub fn assert_peer_absent(body: &str, peer_did: &str) {
    assert!(
        !body.contains(peer_did),
        "I-PS-2 (active-only / residue made visible): a soft-removed peer must be ABSENT \
         from the /peers render (its absence IS the J-003c residue-free promise rendered) \
         even though its cached peer_claims remain on disk; found {peer_did:?} in body:\n{body}"
    );
}

/// Assert the `/peers` rendered HTML shows the RENDER-ONLY `openlore peer remove <bare-did>`
/// revocation command for `peer_did` as TEXT — never an executable control (I-PS-1 / I-PS-8;
/// mirrors the slice-08 `render_follow_guidance` render-only `openlore peer add` assert,
/// N-17). The `bare_did` is the DID with any app-identity `#…` fragment stripped (the
/// `render_remove_guidance` bare-DID strip). Universe (port-exposed rendered surface): the
/// body contains the `openlore peer remove <bare-did>` command text, and there is NO
/// executable revoke/unsubscribe control (no `<button>`, no `<form>`, no mutating
/// `hx-post`, no `name="remove"`).
///
/// SCAFFOLD: true (slice-15).
pub fn assert_peer_remove_command_is_render_only(body: &str, peer_did: &str) {
    // The bare DID (strip any `#…` app-identity fragment — the render_remove_guidance
    // bare-DID strip; the slice-03 `peer remove` verb accepts the bare form).
    let bare_did = peer_did.split('#').next().unwrap_or(peer_did);
    let command = format!("{PEER_REMOVE_COMMAND_VERB} {bare_did}");
    // THE render-only revocation command TEXT for this peer (the slice-03 verb, the single
    // source of truth `PEER_REMOVE_GUIDANCE_PREFIX`).
    assert!(
        body.contains(&command),
        "US-PS-002 (I-PS-8 / WD-PS-6): the /peers row for {peer_did:?} must show the \
         render-only revocation command TEXT {command:?} (mirrors the slice-08 \
         render_follow_guidance render-only `openlore peer add` precedent); body was:\n{body}"
    );
    // …and the command is TEXT ONLY — NO executable revoke/unsubscribe control anywhere on
    // the surface. The unsubscribe stays a deliberate CLI action; the viewer is read-only
    // and holds no key (I-PS-1 / WD-PS-1). No `name="remove"` input, no `Unsubscribe`
    // affordance, no `>Remove<` button label, no `hx-post`/`hx-delete` mutating swap, no
    // `--purge` control.
    let lowered = body.to_ascii_lowercase();
    for banned in [
        "name=\"remove\"",
        "name=\"unsubscribe\"",
        ">remove<",
        ">unsubscribe<",
        "hx-post",
        "hx-delete",
        "hx-put",
        "--purge",
    ] {
        assert!(
            !lowered.contains(&banned.to_ascii_lowercase()),
            "I-PS-1 / WD-PS-1: the revocation command must be render-only TEXT — NO \
             executable remove/unsubscribe/purge control on /peers (the viewer holds no \
             key; unsubscribe stays the slice-03 CLI); found {banned:?} in body:\n{body}"
        );
    }
}

/// Assert the `/peers` rendered HTML is the GUIDED EMPTY STATE (US-PS-003): a plain-language
/// "you follow no peers" notice + the render-only `openlore peer add <did>` STARTING command
/// — never blank, never an error, never a stack trace. Universe (port-exposed rendered
/// surface): the body names the empty state AND carries the `openlore peer add` starting
/// command, AND leaks no stack trace / 5xx internals.
///
/// SCAFFOLD: true (slice-15).
pub fn assert_peers_empty_state_present(body: &str) {
    let lowered = body.to_ascii_lowercase();
    // The guided plain-language empty-state notice: emptiness is recognized as emptiness
    // ("you are not subscribed to any peers" / "no peers"), naming that the operator
    // follows no one (US-PS-003; never blank, never an error).
    assert!(
        lowered.contains("not subscribed to any peers") || lowered.contains("no peers"),
        "US-PS-003: an empty active subscription set must render the guided empty state \
         (\"You are not subscribed to any peers.\"), never blank, never an error; body \
         was:\n{body}"
    );
    // The render-only STARTING command (`openlore peer add <did>`, the slice-03 subscribe
    // verb) points the operator at how to start following — in-context, not a dead end.
    assert!(
        body.contains(PEER_ADD_COMMAND_VERB),
        "US-PS-003: the guided empty state must show the render-only starting command \
         {PEER_ADD_COMMAND_VERB:?} so the operator learns how to start subscribing; body \
         was:\n{body}"
    );
    // The empty state is NOT a blank page and NOT an error surface — no leaked stack trace
    // / panic / 5xx internals (graceful degradation, NFR-PS-6).
    for banned in ["panicked", "RUST_BACKTRACE", "thread 'main'", "stack backtrace", "500 Internal"] {
        assert!(
            !body.contains(banned),
            "US-PS-003: the guided empty state must leak NO stack trace / error internal \
             (found {banned:?}); body was:\n{body}"
        );
    }
}

/// Assert the `/peers` rendered HTML carries NO write / subscribe / unsubscribe / purge
/// control on ANY shape — the read-only-surface guarantee on the `/peers` route (I-PS-1 /
/// WD-PS-1, CARDINAL; the sibling of slice-10's
/// `assert_traversal_html_has_no_write_or_sign_control`). The ONLY revocation affordance is
/// the render-only `openlore peer remove <did>` command TEXT; every reference is
/// non-executable. Universe (port-exposed rendered surface): the body contains none of the
/// write/subscribe/unsubscribe/purge control markers.
///
/// SCAFFOLD: true (slice-15).
pub fn assert_peers_no_write_or_subscribe_control(body: &str) {
    let lowered = body.to_ascii_lowercase();
    for banned in [
        "name=\"subscribe\"",
        "name=\"unsubscribe\"",
        "name=\"remove\"",
        "name=\"follow\"",
        ">subscribe<",
        ">unsubscribe<",
        ">follow<",
        ">remove<",
        "hx-post",
        "hx-delete",
        "hx-put",
        "--purge",
    ] {
        assert!(
            !lowered.contains(&banned.to_ascii_lowercase()),
            "I-PS-1 / WD-PS-1 (CARDINAL): the /peers surface must render NO write / \
             subscribe / unsubscribe / remove / purge control (the viewer is read-only and \
             holds no key; the only revocation affordance is the render-only \
             `openlore peer remove <did>` command TEXT; subscribe/unsubscribe stays the \
             slice-03 CLI); found {banned:?} in body:\n{body}"
        );
    }
}

// =============================================================================
// Slice-16 (viewer-search-follow-state; DISTILL) — the `/search` FOLLOW-STATE
// seeds + asserts. The slice resolves each `/search` result author's relationship
// against the operator's LOCAL active peer subscriptions (the slice-15
// `list_active_peer_subscriptions` read, REUSED) and renders a neutral "Following"
// indicator for an already-followed author (SubscribedPeer, NO `peer add` command)
// while keeping the slice-08 render-only `openlore peer add <did>` affordance for a
// genuinely-unfollowed author (NetworkUnfollowed). ADR-053.
//
// Seeding composition (the load-bearing alignment): a search-result author IS made
// an active subscription by `peer add`-ing the SAME bare DID the index corpus is
// keyed on. The index corpus uses `RACHEL_DID` / `PRIYA_DID` / `TOBIAS_DID`
// (`did:plc:rachel-test` etc.) and the `peer add` verb writes `peer_subscriptions`
// rows keyed on the SAME bare DIDs — so Rachel (`did:plc:rachel-test`) seeded as an
// active subscription AND present in the index resolves to SubscribedPeer. The
// result row's `author_did` carries the `#org.openlore.application` signing fragment;
// the bare active-set DID matches it after the production `bare_did` strip (R-SF-5).
//
// The active set is read from the SAME REAL DuckDB the viewer opens (OPENLORE_HOME),
// so the seed and the viewer agree by construction (Pillar 3). The index is the ONLY
// mocked boundary (a REAL slice-05 `openlore-indexer serve` over the seeded corpus).
//
// Layer placement (Mandate 9/11): layer-3/layer-5 subprocess + real-I/O, EXAMPLE-only.
// Sad paths (none-followed status quo, failed active-set read) are enumerated, never
// PBT-generated at this layer.
//
// SCAFFOLD: true (slice-16) — the seeds REUSE the slice-08 `seed_network_index_*` +
// the slice-15 `peer add` seam (both real); the asserts carry concrete bodies (they
// scan the OBSERVABLE rendered surface). The follow-state RED is the PRODUCTION arm:
// today `to_indexed_claim` hardcodes `NetworkUnfollowed`, so a followed author still
// renders `peer add` and `assert_search_row_following` FAILS for the RIGHT reason
// (MISSING_FUNCTIONALITY — the SubscribedPeer resolution + render arm do not exist
// yet), NOT a setup/import error.

/// The headline reproducible-builds object NSID the slice-16 follow-state scenarios
/// search on (the SAME object the slice-08 walking skeleton + the
/// `IncludesAlreadyFollowedRachel` corpus are keyed on). One source of truth so the
/// query value never drifts from the corpus.
pub const SF_OBJECT_REPRODUCIBLE_BUILDS: &str = "org.openlore.philosophy.reproducible-builds";

/// The render-only follow-guidance command verb a NetworkUnfollowed row carries (the
/// slice-08 `SEARCH_FOLLOW_GUIDANCE_PREFIX` shape). The slice-16 asserts scan for
/// `openlore peer add <bare-did>` as render-only TEXT — UNCHANGED from slice-08.
pub const SF_FOLLOW_COMMAND_VERB: &str = "openlore peer add";

/// The neutral render-only "Following" indicator a SubscribedPeer row carries
/// (the slice-16 `SEARCH_FOLLOWING_INDICATOR` copy, ADR-053 D3). A NEUTRAL label —
/// no command, no verb-phrase, no DID. The asserts scan for it as render-only TEXT.
pub const SF_FOLLOWING_INDICATOR: &str = "Following";

/// Seed ONE ACTIVE peer subscription for `peer_did` via the real `peer add` verb
/// ALONE (no `peer pull` — the relationship resolution reads `peer_subscriptions`,
/// NOT `peer_claims`, so no cached claim is needed). Mirrors
/// [`seed_peer_subscribed_zero_claims`] but parameterized on the DID so a
/// search-result author can be made followed. Returns the held `PeerPds` so the
/// caller keeps the peer resolvable for the lifetime of the `peer add` (the
/// subscription row is written by `peer add`; the LOCAL active-set read the viewer
/// performs never re-resolves the peer, so the PDS may drop afterwards — the handle
/// is returned for explicit lifetime control / symmetry with the slice-15 seed).
///
/// `seed` is the fixture keypair seed used to build verifiable wire records so the
/// real `peer add` can resolve + register the subscription (the same shape
/// `seed_peer_subscribed_zero_claims` uses). After this returns, the
/// `peer_subscriptions` table holds ONE ACTIVE row (`removed_at IS NULL`) for
/// `peer_did` — pinned with `assert_one_active_subscription_for` so the fixture is
/// the GENUINE active state, not merely "the verb exited 0".
///
/// SCAFFOLD: true (slice-16) — drives the EXISTING slice-03 `peer add` verb via the
/// slice-15 `PeerPds` + `run_openlore_with_peer_resolver` seam (REUSED, not net-new).
pub fn seed_active_subscription_for(env: &TestEnv, peer_did: &str, seed: [u8; 32]) -> PeerPds {
    // Build a verifiable record set + start the peer's PDS so `peer add` can resolve
    // the DID and register the subscription. We deliberately do NOT `peer pull`, so
    // NO `peer_claims` row lands — the relationship resolution only needs the ACTIVE
    // `peer_subscriptions` row.
    let (records, _pubkey_hex) = build_verifiable_peer_records_for_triples(
        peer_did,
        seed,
        &[(
            "github:seed/active-subscription",
            "org.openlore.philosophy.dependency-pinning",
            0.50,
        )],
    );
    let pds = PeerPds::for_peer(peer_did, records);

    let added = run_openlore_with_peer_resolver(
        env,
        &["peer", "add", peer_did],
        peer_did,
        pds.endpoint_url(),
    );
    assert_eq!(
        added.status, 0,
        "seed_active_subscription_for: `peer add {peer_did}` must succeed (the slice-03 \
         subscribe verb, REUSED);\n--- stdout ---\n{}\n--- stderr ---\n{}",
        added.stdout, added.stderr
    );

    // Pin the GENUINE active state: ONE active subscription row (removed_at IS NULL).
    assert_one_active_subscription_for(env, peer_did);

    pds
}

/// Slice-16 corpus: ONE followed author (Rachel, `did:plc:rachel-test`) + ONE
/// genuinely-unfollowed author (Priya, `did:plc:priya-test`) EACH asserting the
/// headline reproducible-builds object, so a single `?object=reproducible-builds`
/// search returns BOTH rows. Built inline from the public `RawRecordSpec::valid`
/// builder so the result authors are exactly Rachel + Priya (the
/// `#org.openlore.application` app-identity shape the viewer renders), and the
/// follow-state resolution must produce DIFFERENT affordances on the two rows
/// (Rachel → "Following"; Priya → `peer add`). The two authors assert DISTINCT
/// subjects so the canonical CIDs do not alias.
pub fn sf_corpus_one_followed_one_unfollowed() -> Vec<openlore_test_support::RawRecordSpec> {
    use openlore_test_support::{RawRecordSpec, PRIYA_DID, RACHEL_DID};
    let object = SF_OBJECT_REPRODUCIBLE_BUILDS;
    vec![
        // Rachel — the FOLLOWED author (seeded as an active subscription by the
        // scenario). Her row must resolve to SubscribedPeer → "Following".
        RawRecordSpec::valid(RACHEL_DID, "github:NixOS/nixpkgs", object, 0.88),
        // Priya — the genuinely-UNFOLLOWED author. Her row must stay
        // NetworkUnfollowed → keep the `openlore peer add did:plc:priya-test` command.
        RawRecordSpec::valid(PRIYA_DID, "github:bazelbuild/bazel", object, 0.82),
    ]
}

/// Slice-16 corpus: ALL result authors are followed — Rachel + Tobias
/// (`did:plc:rachel-test` + `did:plc:tobias-test`) EACH asserting the headline
/// object, with NO unfollowed author present. The scenario seeds BOTH as active
/// subscriptions, so every row resolves to SubscribedPeer → "Following" and NO
/// `peer add` command appears ANYWHERE (the all-followed accuracy case).
pub fn sf_corpus_all_authors_followed() -> Vec<openlore_test_support::RawRecordSpec> {
    use openlore_test_support::RawRecordSpec;
    let object = SF_OBJECT_REPRODUCIBLE_BUILDS;
    vec![
        RawRecordSpec::valid(TRAVERSAL_AUTHOR_RACHEL, "github:NixOS/nixpkgs", object, 0.88),
        RawRecordSpec::valid(
            TRAVERSAL_AUTHOR_TOBIAS,
            "github:rust-lang/cargo",
            object,
            0.74,
        ),
    ]
}

/// Slice-16 corpus: MANY result authors (8 distinct), exactly ONE of whom is
/// followed (Rachel). The no-N+1 behavioral proxy (C-4 / WD-SF-3): a large
/// multi-result search resolves ALL rows correctly against the active set read ONCE
/// per render — Rachel → "Following", the other 7 → `peer add` — invariant to the
/// result count. (The STRICT 1-read bound is a DELIVER adapter/property concern; this
/// is the observable proxy.) Every author asserts a DISTINCT subject (no CID alias).
pub fn sf_corpus_many_results_one_followed() -> Vec<openlore_test_support::RawRecordSpec> {
    use openlore_test_support::{RawRecordSpec, RACHEL_DID};
    let object = SF_OBJECT_REPRODUCIBLE_BUILDS;
    let mut specs = vec![
        // Rachel — the ONE followed author among many.
        RawRecordSpec::valid(RACHEL_DID, "github:NixOS/nixpkgs", object, 0.88),
    ];
    // Seven OTHER distinct (unfollowed) authors, each on a distinct subject.
    for (i, subject) in [
        "github:bazelbuild/bazel",
        "github:rust-lang/rust",
        "github:torvalds/linux",
        "github:denoland/deno",
        "github:guix/guix",
        "github:void/voidlinux",
        "github:alpine/aports",
    ]
    .iter()
    .enumerate()
    {
        specs.push(RawRecordSpec::valid(
            &format!("did:plc:sf-author{}-test", i + 1),
            subject,
            object,
            0.60 + (i as f64) * 0.01,
        ));
    }
    specs
}

/// Assert a `/search` rendered body shows the row for `peer_did` (a FOLLOWED author)
/// as a SubscribedPeer: it carries the neutral render-only "Following" indicator AND
/// it carries NO `openlore peer add <did>` command for that author (the load-bearing
/// accuracy fix, C-2 / ADR-053). Universe (port-exposed rendered surface): the body
/// contains the bare DID + the "Following" indicator, and contains NO
/// `openlore peer add <bare-did>` command naming this author.
///
/// `peer_did` is the BARE DID (`did:plc:rachel-test`); the viewer renders the
/// `did:plc:rachel-test#org.openlore.application` app-identity shape, which
/// `contains(peer_did)` matches (the fragment is a suffix of the rendered DID).
///
/// SCAFFOLD: true (slice-16) — RED today: `to_indexed_claim` hardcodes
/// `NetworkUnfollowed`, so a followed author still renders `peer add` and the
/// "no add command" + "Following present" assertions FAIL for the RIGHT reason
/// (the SubscribedPeer resolution + render arm are MISSING).
pub fn assert_search_row_following(body: &str, peer_did: &str) {
    // The followed author is still attributed VERBATIM (the resolution is a per-row
    // enrichment; attribution is UNCHANGED — C-5).
    assert!(
        body.contains(peer_did),
        "C-2 (slice-16): the /search render must still attribute a row to the followed \
         author {peer_did:?} (the relationship label is a per-row enrichment, attribution \
         unchanged); body was:\n{body}"
    );
    // The neutral render-only "Following" indicator is present (the SubscribedPeer arm,
    // ADR-053 D3) — a developer she ALREADY follows is shown as such.
    assert!(
        body.contains(SF_FOLLOWING_INDICATOR),
        "C-2 (slice-16): a followed author's row must show the neutral render-only \
         {SF_FOLLOWING_INDICATOR:?} indicator (the SubscribedPeer arm); body was:\n{body}"
    );
    // …and the `openlore peer add <bare-did>` command for THIS author is ABSENT — an
    // already-followed author is NOT re-offered a follow (the core bug this slice fixes,
    // R-SF-3). The follow command names the BARE DID (the slice-03 verb form).
    let follow_command = format!("{SF_FOLLOW_COMMAND_VERB} {peer_did}");
    assert!(
        !body.contains(&follow_command),
        "C-2 (slice-16, R-SF-3): a followed author's row must NOT re-offer a follow — \
         expected NO {follow_command:?} command for {peer_did:?}; body was:\n{body}"
    );
}

/// Assert a `/search` rendered body shows the row for `peer_did` (a genuinely-
/// UNFOLLOWED author) keeping the slice-08 render-only follow affordance: the
/// `openlore peer add <bare-did>` command is present as plain TEXT (no over-correction,
/// R-SF-4 / C-2). Universe (port-exposed rendered surface): the body contains the bare
/// DID + the `openlore peer add <bare-did>` command. This is byte-equivalent to the
/// slice-08 N-17 affordance — UNCHANGED for the NetworkUnfollowed arm.
///
/// SCAFFOLD: true (slice-16) — this assertion PASSES today for an unfollowed author
/// (slice-08 already renders the affordance for the hardcoded NetworkUnfollowed); it
/// pins the no-over-correction guarantee (the slice-16 resolution must NOT strip the
/// affordance from a genuinely-unfollowed author).
pub fn assert_search_row_offers_follow(body: &str, peer_did: &str) {
    // The unfollowed author is attributed VERBATIM.
    assert!(
        body.contains(peer_did),
        "C-2 (slice-16): the /search render must attribute a row to the unfollowed \
         author {peer_did:?}; body was:\n{body}"
    );
    // The slice-08 render-only `openlore peer add <bare-did>` follow affordance is
    // RETAINED for a genuinely-unfollowed author (no over-correction, R-SF-4).
    let follow_command = format!("{SF_FOLLOW_COMMAND_VERB} {peer_did}");
    assert!(
        body.contains(&follow_command),
        "C-2 (slice-16, R-SF-4): a genuinely-unfollowed author's row must KEEP the \
         render-only {follow_command:?} affordance (the slice-08 status quo, unchanged); \
         body was:\n{body}"
    );
    // …and the unfollowed row does NOT show the "Following" indicator (binary resolution,
    // C-6 — not-in-active-set → NetworkUnfollowed, never SubscribedPeer).
    // (We do not assert global absence of "Following" here — a MIX render carries both;
    // the per-author discrimination is asserted by pairing this with
    // `assert_search_row_following` on the followed author in the MIX scenario.)
}

/// Assert NEITHER `/search` follow-state affordance is an executable control (C-1,
/// CARDINAL / WD-SF-1): both the "Following" indicator AND the `openlore peer add <did>`
/// guidance are render-only TEXT — no `<button>`, no `<form>`, no mutating `<a>`, no
/// `hx-*` control, no follow/subscribe input. The viewer holds no key and exposes no
/// follow/unfollow route. The slice-16 companion to the slice-08 N-17 + the
/// `no_search_response_adds_a_write_or_sign_control` gold scan — extended so it holds
/// over a render that ALSO carries the new "Following" indicator (the new arm must add
/// no control either). Universe (port-exposed rendered surface): the body contains
/// NONE of the executable-control markers.
///
/// SCAFFOLD: true (slice-16).
pub fn assert_search_follow_state_is_render_only(body: &str) {
    let lowered = body.to_ascii_lowercase();
    for banned in [
        // executable FOLLOW / UNFOLLOW / SUBSCRIBE controls (reused from slice-08 N-17 +
        // the slice-15 /peers no-control gold)
        "name=\"follow\"",
        "name=\"unfollow\"",
        "name=\"subscribe\"",
        ">follow<",
        ">unfollow<",
        ">subscribe<",
        ">following<", // the indicator must be a neutral LABEL, never a >Following< control element
        // mutating htmx swaps (a render-only affordance carries NO hx-* mutation)
        "hx-post",
        "hx-delete",
        "hx-put",
    ] {
        assert!(
            !lowered.contains(&banned.to_ascii_lowercase()),
            "C-1 (slice-16, CARDINAL): the /search follow-state affordances must BOTH be \
             render-only TEXT — NO executable follow/unfollow/subscribe control, NO \
             mutating hx-* swap (the viewer holds no key; the follow stays the slice-03 \
             CLI); found {banned:?} in body:\n{body}"
        );
    }
}

/// Slice-16 graceful-degrade (E) — the RED-scaffolded TRUE active-set-read-failure
/// seam. Start the viewer wired to a reachable index BUT with the LOCAL
/// active-subscription read forced to FAIL mid-request, so the relationship
/// resolution must degrade to an EMPTY active set → every author NetworkUnfollowed
/// (the slice-08 status quo) — no crash, no blank, no 5xx, no leaked error
/// (C-7 / WD-SF-6 / ADR-053 D5 / §Earned-Trust).
///
/// SEEDING-SEAM NOTE (documented DISTILL choice): the slice-08/15 viewer harness
/// holds ONE long-lived DuckDB connection taken at STARTUP (wire→probe→use,
/// ADR-028/030), so the existing `make_store_unreadable` lock would refuse STARTUP
/// rather than exercise a MID-REQUEST read failure. There is NO readily-available
/// mid-request read-failure seam in the slice-08/15 harness. Per the DISTILL guidance,
/// the OBSERVABLE degrade-TARGET contract (empty active set → all-`peer add`,
/// byte-equal slice-08) is pinned by the none-followed scenario (fully exercisable
/// today); this seam scaffolds the TRUE read-failure path for DELIVER to materialize
/// (mirroring how slice-08 left `start_inner` as `todo!()` for DELIVER). DELIVER picks
/// the fault-injection mechanism (e.g. an `OPENLORE_VIEWER_FAIL_ACTIVE_SET_READ` test
/// seam in the effect shell, or a per-request connection that can be poisoned) with the
/// SAME observable contract: a failed active-set read degrades to all-NetworkUnfollowed.
///
/// MATERIALIZED (slice-16 DELIVER, step 02-03): the fault-injection mechanism is the
/// `OPENLORE_VIEWER_FAIL_ACTIVE_SET_READ` env seam, honored ONLY by the effect shell's
/// `#[cfg(debug_assertions)]`-gated `active_set_read_with_fault_seam` (release-forbidden,
/// mirroring the ADR-026 `OPENLORE_PEER_PUBKEY_HEX_` seam discipline; enforced by
/// `xtask check-arch`). The seam substitutes a genuine `Err(StoreReadError::Unreadable)`
/// for the REAL mid-request active-set read, so the PRODUCTION degrade path
/// (`Err → unwrap_or_default() → EMPTY set → every author NetworkUnfollowed`, the
/// slice-08 status quo, ADR-053 D5) is the thing exercised. The index stays reachable —
/// ONLY the LOCAL active-set read fails — so the degrade is observed against a live render.
pub fn start_viewer_with_failing_active_set_read(
    env: &TestEnv,
    indexer: IndexerHandle,
) -> ViewerServer {
    let url = indexer.indexer_url();
    ViewerServer::start_inner(env, None, Some(url), Some(indexer), true, false, false, false)
}

// =============================================================================
// Slice-17 (viewer-landing-dashboard; DISTILL) — the `GET /` LANDING DASHBOARD
// seeds + asserts. The slice turns the storeless front door into a navigation
// hub + at-a-glance LOCAL store summary: it threads the read-only store into
// `landing_page`, resolves THREE LOCAL aggregate counts (own claims via
// `count_claims`, peer claims via `count_peer_claims`, active peers via the NEW
// count-only `count_active_peer_subscriptions`) — each `Result → Option` via
// `.ok()` in the effect shell — into an Option-shaped `LandingSummary`, and
// renders the three counts + a nav hub of plain `<a href>` links to all 8 shipped
// surfaces via URL consts. ADR-054. Full-page-only (GET / does not fork by Shape —
// parity by construction, ADR-054 D5).
//
// Seeding composition (Pillar 3 — production write paths, the SAME REAL DuckDB the
// viewer reads, BR-VIEW-4): own claims via the real `claim add` verb
// (`seed_own_claims_via_cli`, slice-06), peer claims via the real `peer add` +
// `peer pull` federation path (`seed_peer_authored_graph`, slice-09/10/15), active
// subscriptions via the real `peer add` verb (`seed_active_subscription_for`,
// slice-16). NO hand-inserted rows. NO network/external boundary — `/` is LOCAL +
// OFFLINE (offline-STRONGER than `/search`/`/scrape`; the three reads are LOCAL
// `COUNT(*)` aggregates with no outbound edge).
//
// The asserts scan ONLY the rendered HTML the operator's browser shows (Mandate 8
// universe = port-exposed rendered surface, never an internal `viewer-domain`
// struct field). NO scenario calls `render_landing` / the count reads directly
// (those are unit/property-level, exercised in DELIVER) — every assertion is on the
// `GET /` HTTP response (Mandate 1, driving-port discipline). The read-only /
// no-write / offline-chrome / no-N+1 / missing≠zero / discoverability gold
// invariants REUSE the slice-06/15 `capture_store_row_count_universe` +
// `assert_store_read_only` + `references_external_cdn` harness VERBATIM.
//
// Layer placement (Mandate 9/11): every scenario is a layer-3/layer-5 subprocess +
// real-I/O test — EXAMPLE-only. The sad paths (honest empty store, failed
// peer-claims-count read) are enumerated explicitly, never PBT-generated at this
// layer (the generative exploration of the pure `render_landing` over the 2³
// Option combinations is a layer-1/2 DELIVER concern). Tier B is NOT warranted:
// `GET /` is a single-shot orientation render with no chained ≥3-scenario journey
// and no domain-rich input space (three counts + 8 fixed links) — Tier A example
// coverage is exact (Mandate 10 skip criteria).
//
// SCAFFOLD: true (slice-17) — the seeds + asserts COMPILE now (they drive EXISTING
// `claim add` / `peer add` / `peer pull` verbs + scan strings); the SCENARIOS stay
// RED because the production `/` route is STORELESS (`render_landing()` takes no
// summary, renders only the `<h1>` + `READ_ONLY_NOTICE` + a single `/claims` link),
// and `SCRAPE_URL` / `count_active_peer_subscriptions` / `LandingSummary` /
// `MISSING_COUNT_MARKER` do NOT exist yet — so the three counts + the 8-surface hub
// are ABSENT from the rendered body. RED = MISSING_FUNCTIONALITY, never BROKEN. The
// ATs drive `GET /` via subprocess HTTP (never the Rust `render_landing` signature),
// so the production signature change (adding the `&LandingSummary` param) is
// DELIVER's job and does not affect AT compilation — the AT compiles and fails at
// the HTTP body assertion.
// =============================================================================

/// The landing route path (`GET /` — the slice-06 front door, EXTENDED this slice).
pub const LANDING_PATH: &str = "/";

/// The substring the read-only notice carries on the front door (the slice-06
/// `READ_ONLY_NOTICE` shape — "nothing here can change your store"). The slice-06 V-3
/// test asserts `body_contains("read-only")`; this const pins the observable read-only
/// assurance text so the landing scenarios scan the same surface. The full production
/// const lives in `viewer-domain::READ_ONLY_NOTICE` (unchanged this slice).
pub const READ_ONLY_NOTICE_TEXT: &str = "read-only";

/// The known summary the walking-skeleton + counts-correct scenarios seed: 12 own
/// claims, 7 peer claims, 2 active peers (did:plc:rachel-test + did:plc:tobias-test).
/// The brief's headline numbers (US-LD-001 Theme 1). One source of truth so the seed
/// and the assertions agree.
pub const LANDING_OWN_CLAIMS: usize = 12;
pub const LANDING_PEER_CLAIMS: usize = 7;
pub const LANDING_ACTIVE_PEERS: usize = 2;

/// The two ACTIVE peers the landing summary counts (REAL DIDs, REUSED from the
/// slice-15 peer-subscription constants so the seeded attribution shape is
/// consistent across the viewer slices).
pub const LANDING_PEER_RACHEL_DID: &str = TRAVERSAL_AUTHOR_RACHEL; // did:plc:rachel-test
pub const LANDING_PEER_TOBIAS_DID: &str = TRAVERSAL_AUTHOR_TOBIAS; // did:plc:tobias-test

/// The missing-number marker the landing renders for a count whose read FAILED
/// (the `LandingSummary` field is `None` → `MISSING_COUNT_MARKER`, ADR-054 D2 /
/// WD-LD-8). A horizontal bar "—", visually + semantically DISTINCT from the digit
/// "0" (a successful read of an empty store). The asserts scan for it as the
/// missing-number state. SINGLE source of truth mirroring the production
/// `viewer-domain::MISSING_COUNT_MARKER` const (DELIVER mints it).
pub const LANDING_MISSING_COUNT_MARKER: &str = "—";

/// The 8 shipped top-level entry-point surfaces the nav hub must link, as
/// `(label, href)` pairs. The href is the route's URL CONST value from
/// `viewer-domain` (7 existing + the slice-17 `SCRAPE_URL = "/scrape"`, ADR-054 D4) —
/// NOT a hardcoded literal that could drift. The discoverability contract (WD-LD-7 /
/// Theme 2 / C-3): the hub links ALL 8, each a plain `<a href>` (no-JS navigable).
/// The labels are the human-facing surface names; the asserts scan the rendered body
/// for the href (the load-bearing navigation target) — see
/// [`assert_landing_links_all_surfaces`].
pub const LANDING_TOP_LEVEL_SURFACES: &[(&str, &str)] = &[
    ("My Claims", "/claims"),         // MY_CLAIMS_URL
    ("Peer Claims", "/peer-claims"),  // PEER_CLAIMS_URL
    ("Project Survey", "/project"),   // PROJECT_URL
    ("Philosophy Survey", "/philosophy"), // PHILOSOPHY_URL
    ("Contributor Score", "/score"),  // SCORE_URL
    ("Network Search", "/search"),    // SEARCH_URL
    ("Live Scrape", "/scrape"),       // SCRAPE_URL (NEW this slice)
    ("Peer Subscriptions", "/peers"), // PEERS_URL
];

/// The deep / parameterized routes that must NOT appear as a top-level hub link
/// (FR-LD-5 / Theme 2): drilling into who-said-what is reached THROUGH the 8
/// top-level surfaces, never linked directly from the front door. The asserts scan
/// that NONE of these appears as a hub `href=` target — see
/// [`assert_landing_no_deep_route_toplevel`].
pub const LANDING_DEEP_ROUTES_FORBIDDEN_AT_TOPLEVEL: &[&str] = &[
    "/claims/bafy",      // /claims/{cid} detail (a CID-addressed deep route)
    "?contributor=",     // /score?contributor=… parameterized
    "?subject=",         // /project?subject=… parameterized
    "?object=",          // /philosophy?object=… parameterized
];

/// Seed the env's REAL store to the KNOWN landing summary — 12 own claims (real
/// `claim add`), 7 peer claims (real `peer add` + `peer pull` over ONE peer), 2
/// ACTIVE peer subscriptions (Rachel + Tobias). The walking-skeleton + counts-correct
/// precondition (US-LD-001 Theme 1/3). After seeding, `count_claims()` returns 12,
/// `count_peer_claims()` returns 7, and `count_active_peer_subscriptions()` returns 2
/// — exactly the three aggregates the landing summary must surface as "12 own claims,
/// 7 peer claims, 2 active peers". The fixture is pinned with the existing per-table
/// asserts so it is the GENUINE summary shape, not merely "the verbs exited 0".
///
/// Composition: 7 peer claims are seeded as 7 DISTINCT triples authored by Rachel via
/// the production federation path (`seed_peer_authored_graph` — `peer add` + `peer
/// pull`), making Rachel an ACTIVE subscription with 7 cached claims. Tobias is then
/// added as a SECOND active subscription via `seed_active_subscription_for` (no pull —
/// the active-peer COUNT reads `peer_subscriptions`, not `peer_claims`), so the active
/// set is 2. Own claims are 12 via `seed_own_claims_via_cli`. The two PDS handles are
/// returned so the caller keeps the peers resolvable for the lifetime of the seed.
///
/// SCAFFOLD: true (slice-17) — drives the EXISTING `claim add` / `peer add` / `peer
/// pull` verbs (REUSED slice-06/15/16 seams); the rows land in the REAL `claims` +
/// `peer_claims` + `peer_subscriptions` tables the viewer's LOCAL `GET /` reads.
pub fn seed_landing_store_summary(env: &TestEnv) -> HeldSubscriptions {
    let dep = "org.openlore.philosophy.dependency-pinning";
    let repro = "org.openlore.philosophy.reproducible-builds";
    let workspace = "org.openlore.philosophy.workspace-cohesion";
    let memory = "org.openlore.philosophy.memory-safety";
    let actor = "org.openlore.philosophy.actor-model";

    // 12 OWN claims via the production `claim add` write path (DISTINCT subjects so
    // distinct CIDs — `seed_own_claims_via_cli` handles the loop).
    seed_own_claims_via_cli(env, LANDING_OWN_CLAIMS);

    // 7 PEER claims authored by Rachel via the production `peer add` + `peer pull`
    // federation path (7 DISTINCT triples so the canonical CIDs do not alias). Rachel
    // becomes an ACTIVE subscription (1 of the 2) AND contributes the 7 peer claims.
    let graph = seed_peer_authored_graph(
        env,
        &[SeedPeer {
            peer_did: LANDING_PEER_RACHEL_DID,
            seed: [7u8; 32],
            triples: &[
                ("github:rust-lang/cargo", dep, 0.90),
                ("github:NixOS/nixpkgs", repro, 0.74),
                ("github:bazelbuild/bazel", workspace, 0.61),
                ("github:rust-lang/rust", memory, 0.88),
                ("github:erlang/otp", actor, 0.55),
                ("github:tokio-rs/tokio", dep, 0.66),
                ("github:serde-rs/serde", repro, 0.42),
            ],
        }],
    );
    drop(graph);

    // Tobias as a SECOND ACTIVE subscription via the real `peer add` verb (no pull —
    // the active-peer COUNT reads `peer_subscriptions`, so a subscription with zero
    // cached claims still counts as 1 active peer). Active set = {Rachel, Tobias} = 2.
    let tobias_pds = seed_active_subscription_for(env, LANDING_PEER_TOBIAS_DID, [9u8; 32]);

    // Pin the GENUINE summary shape: 12 own claims, 7 peer claims (all Rachel's), 2
    // active subscriptions — so the fixture is the REAL three-count state the landing
    // summary must surface (not merely "the verbs exited 0").
    assert_user_author_claim_count(env, LANDING_OWN_CLAIMS);
    assert_peer_claims_row_count_for(env, LANDING_PEER_RACHEL_DID, LANDING_PEER_CLAIMS);
    assert_one_active_subscription_for(env, LANDING_PEER_RACHEL_DID);
    assert_one_active_subscription_for(env, LANDING_PEER_TOBIAS_DID);

    HeldSubscriptions {
        _peers: vec![tobias_pds],
    }
}

/// Seed a FRESH EMPTY store for the honest-zeros landing scenario (US-LD-001 Theme 1
/// Ex 2 / Theme 4 Ex 2): NO own claims, NO peer claims, NO active subscriptions. A
/// no-op over a freshly `initialized()` store (the production `init` ran, no write
/// verb did), named explicitly so the empty-store scenario reads in the domain
/// language. The three count reads must return `Some(0)` (a SUCCESSFUL read of zero —
/// an honest empty store) so the landing renders "0 own claims / 0 peer claims / 0
/// active peers", DISTINCT from the missing-number marker "—" (a FAILED read).
///
/// SCAFFOLD: true (slice-17) — the empty store IS the precondition; the three reads
/// must return `Ok(0) → Some(0)` and the viewer renders honest zeros + the full hub.
pub fn seed_empty_store_for_landing(_env: &TestEnv) {
    // Intentionally empty: a freshly `initialized()` store has zero claims, zero peer
    // claims, and zero peer_subscriptions rows. The named seed documents the
    // honest-empty-store precondition at the call site (Pillar 1 domain language).
}

/// Start the `openlore ui` viewer over the env's REAL store with the LOCAL
/// `count_peer_claims()` read forced to FAIL mid-request — the slice-17 graceful-degrade
/// seam (US-LD-000/001 Theme 4 / C-2 CARDINAL / WD-LD-2 / WD-LD-8 / ADR-054 D2). The
/// own-claims + active-peer count reads STILL succeed; ONLY the peer-claims count
/// fails, so the landing must render the missing-number marker "—" for the peer-claims
/// number while the OTHER TWO counts + the full nav hub render, the page staying a
/// normal 200 (never a 5xx / blank / raw stack trace). The failed read maps to `None`
/// (`.ok()`), DISTINCT from a fabricated `Some(0)`.
///
/// SEEDING-SEAM NOTE (documented DISTILL choice, mirroring the slice-16 SF-8 precedent):
/// the slice-06/15 viewer harness holds ONE long-lived DuckDB connection taken at
/// STARTUP (wire→probe→use, ADR-028/030), so the existing `make_store_unreadable` lock
/// would refuse STARTUP rather than exercise a MID-REQUEST per-count read failure. There
/// is NO readily-available mid-request per-count read-failure seam in the slice-06/15
/// harness. Per the DISTILL guidance, the OBSERVABLE missing-number contract (a failed
/// peer-claims read → "—" while the other two counts render, page 200) is scaffolded
/// against a TEST-ONLY effect-shell fault seam (the `OPENLORE_VIEWER_FAIL_PEER_CLAIMS_
/// COUNT` env var, threaded by `start_inner`); the SUCCESSFUL-zero distinction is fully
/// exercisable today via `seed_empty_store_for_landing`. DELIVER materializes the
/// per-count fault seam (a `#[cfg(debug_assertions)]`-gated, release-forbidden,
/// xtask-guarded effect-shell branch substituting `Err(StoreReadError)` for the REAL
/// `count_peer_claims()` read) with the SAME observable target — exactly as slice-16
/// materialized `OPENLORE_VIEWER_FAIL_ACTIVE_SET_READ`. Until then the scenario panics at
/// the `todo!()` `start_inner` body (slice-06) → RED MISSING_FUNCTIONALITY, never BROKEN.
///
/// SCAFFOLD: true (slice-17).
pub fn start_viewer_with_failing_peer_claims_count(env: &TestEnv) -> ViewerServer {
    ViewerServer::start_inner(env, None, None, None, false, true, false, false)
}

/// Assert the landing render shows the count `n` for the surface labelled `label`
/// (e.g. `("own claims", 12)`). Universe (Mandate 8 — port-exposed rendered surface):
/// the rendered body contains both the number `n` and the `label` text, so the count
/// is attributed to the right surface (not a stray digit). Scans the OBSERVABLE HTML
/// the operator's browser shows; never an internal `LandingSummary` field. Used by the
/// happy-path + counts-correct + honest-zeros scenarios.
///
/// SCAFFOLD: true (slice-17).
pub fn assert_landing_shows_count(body: &str, label: &str, n: usize) {
    assert!(
        body.contains(label),
        "the landing summary must label the {label:?} count so the operator can read \
         WHICH count is which (Theme 1); body was:\n{body}"
    );
    let needle = n.to_string();
    assert!(
        body.contains(&needle),
        "the landing summary must show the count {n} for {label:?} (the genuine \
         seeded aggregate, Theme 3); body was:\n{body}"
    );
}

/// Assert the landing render shows the MISSING-NUMBER marker "—" for the surface
/// labelled `label` (a FAILED count read, ADR-054 D2 / WD-LD-8), DISTINCT from a
/// successful `0`. Universe (port-exposed rendered surface): the rendered body
/// contains the marker AT THE COUNT POSITION for this surface — i.e. the substring
/// `"— <label>"` (the SAME `render_count(count) " <label>"` shape the pure render
/// emits). We scan the COUNT POSITION, NOT the bare marker: the page chrome title
/// ("OpenLore — Viewer") legitimately carries the em-dash, so a bare-marker scan
/// would collide with the title and pass trivially even when the count rendered a
/// number — mirroring the honest-zeros scenario's `"— <label>"`-position scan
/// (the falsifiable form of `0 ≠ missing`). Used by the failed-read degrade
/// scenario (Theme 4). The caller separately asserts the OTHER counts still render
/// their numbers + the page is 200 (the degrade is per-count, independent).
pub fn assert_landing_count_missing(body: &str, label: &str) {
    assert!(
        body.contains(label),
        "the landing summary must still label the {label:?} count even when its read \
         FAILED (the surface is present, only the number is missing, Theme 4); body \
         was:\n{body}"
    );
    let missing_count = format!("{LANDING_MISSING_COUNT_MARKER} {label}");
    assert!(
        body.contains(&missing_count),
        "a FAILED count read must render the missing-number marker at the count \
         position ({missing_count:?}) — DISTINCT from a fabricated 0 AND from the chrome \
         title's em-dash (ADR-054 D2 / WD-LD-8); body was:\n{body}"
    );
}

/// Assert the landing nav hub links ALL 8 shipped top-level surfaces (the
/// discoverability contract, WD-LD-7 / Theme 2 / C-3). Universe (port-exposed rendered
/// surface): for each of the 8 surfaces, the rendered body contains an `<a>` anchor
/// whose `href` equals the surface's route URL const value, AND each is a plain
/// no-JS-navigable link (the `href` attribute is present — not an `hx-get`-only
/// affordance). Scans the OBSERVABLE HTML; never the internal URL-const symbols. A
/// missing surface is an UNSHIPPABLE discoverability gap (the front door must reach
/// every shipped surface).
///
/// SCAFFOLD: true (slice-17).
pub fn assert_landing_links_all_surfaces(body: &str) {
    for (label, href) in LANDING_TOP_LEVEL_SURFACES {
        // Each surface is reachable via a plain `<a href="…">` (no-JS navigable, C-5).
        // We scan for the `href="<route>"` attribute — the load-bearing navigation
        // target — so an `hx-get`-only affordance (no `href`) would NOT satisfy it.
        let href_attr = format!("href=\"{href}\"");
        assert!(
            body.contains(&href_attr),
            "the nav hub must link the {label:?} surface via a plain <a {href_attr}> \
             (no-JS navigable, WD-LD-7 / C-3 / C-5) — discoverability gap otherwise; \
             body was:\n{body}"
        );
    }
}

/// Assert the landing nav hub links NO deep / parameterized route as a top-level
/// affordance (FR-LD-5 / Theme 2): `/claims/{cid}`, `/score?contributor=…`,
/// `/project?subject=…`, `/philosophy?object=…` are reached THROUGH the 8 top-level
/// surfaces, never linked directly from the front door. Universe (port-exposed
/// rendered surface): the rendered body contains NONE of the forbidden deep-route
/// fragments as an `href` target. Scans the OBSERVABLE HTML.
///
/// SCAFFOLD: true (slice-17).
pub fn assert_landing_no_deep_route_toplevel(body: &str) {
    for forbidden in LANDING_DEEP_ROUTES_FORBIDDEN_AT_TOPLEVEL {
        let href_attr = format!("href=\"{forbidden}");
        assert!(
            !body.contains(&href_attr),
            "the nav hub must NOT link a deep/parameterized route as a top-level \
             affordance (FR-LD-5 / Theme 2 — drilling in is reached THROUGH the 8 \
             surfaces); found a hub link to {forbidden:?}; body was:\n{body}"
        );
    }
}

/// Assert the landing render exposes NO write / compose / sign / subscribe / follow
/// control — every navigation affordance is a plain link, not a mutating control
/// (US-LD-001 Theme 3 / C-1 CARDINAL / WD-LD-1). Universe (port-exposed rendered
/// surface): the rendered body contains no `<form>`, no `<button>`, and no
/// mutating `hx-post`/`hx-put`/`hx-delete` swap. REUSES the slice-15 banned-control
/// vocabulary; adds the form/button/sign/compose scan specific to the front door.
/// The no-key guarantee is STRUCTURAL (the viewer process links no IdentityPort —
/// proven by the slice-06 `web_process_holds_no_signing_key` gold + xtask check-arch);
/// here we assert the operator-facing rendered surface carries no mutating control.
///
/// SCAFFOLD: true (slice-17).
pub fn assert_landing_read_only_no_control(body: &str) {
    let lowered = body.to_ascii_lowercase();
    for banned in [
        "<form",
        "<button",
        "hx-post",
        "hx-put",
        "hx-delete",
        "name=\"compose\"",
        "name=\"sign\"",
        "name=\"subscribe\"",
        "name=\"follow\"",
        ">compose<",
        ">sign<",
        ">subscribe<",
        ">follow<",
    ] {
        assert!(
            !lowered.contains(&banned.to_ascii_lowercase()),
            "C-1 (slice-17, CARDINAL): the front door must render NO write / compose / \
             sign / subscribe / follow control — every navigation affordance is a plain \
             <a href> link, never a mutating control (the viewer is read-only and holds \
             no key, WD-LD-1); found {banned:?} in body:\n{body}"
        );
    }
}

// =============================================================================
// Slice-18 (viewer-counter-aware-counts; DISTILL) — the COUNTERED-OWN-CLAIMS count
// rendered beside the own-claims count on the `GET /` landing summary AND in the
// `GET /claims` list header: "12 own claims (3 countered)" (US-CC-000/001/002;
// ADR-055). EXTENDS the slice-17 `LandingSummary` with a FOURTH additive
// `Option<usize>` field `countered_own_claims` (`.ok()`-degraded), threaded into
// `render_landing` (beside the unchanged own-claims line) + a NEW `render_claims_page`
// `countered_own_claims: Option<usize>` param via a SHARED pure
// `render_countered(Option<usize>) -> String` helper (single source — ADR-055 D3). The
// count is `count_countered_own_claims()` = `COUNT(DISTINCT c.cid) FROM claims c WHERE
// c.cid IN (SELECT referenced_cid FROM claim_references WHERE ref_type='counters' UNION
// SELECT referenced_cid FROM peer_claim_references WHERE ref_type='counters')` — a
// presence count (a claim countered by N peers counts ONCE), own-only by query shape,
// invariant to store size (ADR-055 D1).
//
// The data shape (the load-bearing seeding fact): the self-counter rule BLOCKS the
// operator from countering her OWN claim (`claim counter` rejects a self-target with
// "use retract instead") — so an OWN claim is countered ONLY through the
// `peer_claim_references` arm: a PEER who authored a `counters`-referencing record
// targeting that own CID. The slice-18 seeds therefore reuse the slice-12
// `build_verifiable_peer_counter_record` + `peer add` + `peer pull` path to land each
// peer counter's `counters` reference in `peer_claim_references` with `referenced_cid ==
// <own target CID>`. One own claim countered by TWO distinct peers (Rachel + Tobias)
// proves the presence-once contract (`COUNT(DISTINCT)` collapses the two references of
// the SAME own CID to ONE → "(1 countered)", never "(2 countered)").
//
// The asserts scan ONLY the rendered HTML the operator's browser shows (Mandate 8
// universe = port-exposed rendered surface, never an internal `LandingSummary` field).
// NO scenario calls `render_landing` / `render_countered` / the count read directly
// (those are unit/property-level, exercised in DELIVER) — every assertion is on the
// `GET /` or `GET /claims` HTTP response (Mandate 1 driving-port discipline). The seeds
// pin the GENUINE countered-count with a DIRECT DuckDB `COUNT(DISTINCT)` assert (the
// SAME ADR-055 SQL), so the fixture is the REAL countered shape, not merely "the verbs
// exited 0".
//
// Layer placement (Mandate 9/11): every scenario is a layer-3/layer-5 subprocess +
// real-I/O test — EXAMPLE-only. The sad paths (honest "(0 countered)", failed
// countered-count read → "(— countered)") are enumerated explicitly, never
// PBT-generated at this layer (the generative exploration of the pure `render_countered`
// over the Some(0)/Some(n)/None cases is a layer-1/2 DELIVER concern). Tier B
// (state-machine PBT) is NOT warranted: the count is a single-shot additive render with
// no chained ≥3-scenario journey and no domain-rich input space (one Option<usize>) —
// Tier A example coverage is exact (Mandate 10 skip criteria).
//
// SCAFFOLD: true (slice-18) — the seeds + asserts COMPILE now (they drive EXISTING
// `claim add` / `peer add` / `peer pull` verbs + scan strings + read the REAL store);
// the SCENARIOS stay RED because the production `/` + `/claims` routes do NOT render the
// countered count yet, and `count_countered_own_claims` / `render_countered` / the 4th
// `LandingSummary` field / the `render_claims_page` `Option<usize>` param do NOT exist —
// so "(3 countered)" is ABSENT from both rendered surfaces → RED MISSING_FUNCTIONALITY,
// never BROKEN. The ATs drive `GET /` + `GET /claims` via subprocess HTTP (never the
// Rust `render_landing` / `render_claims_page` signatures), so the production signature
// changes (the 4th field + the `/claims` param) are DELIVER's job and do not affect AT
// compilation. The missing≠zero failed-read scenario drives the test-only
// `OPENLORE_VIEWER_FAIL_COUNTERED_COUNT` effect-shell fault seam (slice-17 LD-DEGRADE
// precedent), panicking at the `start_inner` `todo!()` body until DELIVER materializes
// it — also MISSING_FUNCTIONALITY.
// =============================================================================

/// The `/claims` list route path (the slice-06 My Claims list, whose HEADER this slice
/// extends with the countered count "(N countered)" — US-CC-002).
pub const CLAIMS_LIST_PATH: &str = "/claims";

/// The headline countered-own-claims count the slice-18 seeds pin: of Maria's 12 own
/// claims, EXACTLY 3 are countered (one of them countered by TWO distinct peers, proving
/// the presence-once `COUNT(DISTINCT)` collapses it to ONE). The brief's headline number
/// ("12 own claims (3 countered)", US-CC-001). One source of truth so the seed and the
/// assertions agree.
pub const COUNTERED_OWN_CLAIMS: usize = 3;

/// The missing-marker the countered count renders for a FAILED read — reusing the
/// slice-17 `MISSING_COUNT_MARKER` "—" inside the parenthetical: `render_countered(None)`
/// → "(— countered)" (ADR-055 D3). DISTINCT from a SUCCESSFUL `Some(0)` → "(0 countered)".
/// The asserts scan the COUNTERED-COUNT POSITION (`"(— countered)"`), not the bare marker
/// (the chrome title's em-dash would collide). SINGLE source mirroring the production
/// `viewer-domain::MISSING_COUNT_MARKER` (DELIVER mints the helper).
pub const COUNTERED_MISSING_MARKER: &str = "—";

/// The two PEER counter authors the slice-18 seeds use to counter Maria's OWN claims
/// (REUSED from the slice-11/12 counter constants so the seeded attribution shape is
/// consistent across the viewer slices). The self-counter rule means an OWN claim is
/// countered ONLY by a PEER (`peer_claim_references` arm).
pub const COUNTERED_PEER_RACHEL_DID: &str = COUNTER_TARGET_AUTHOR_RACHEL; // did:plc:rachel-test
pub const COUNTERED_PEER_TOBIAS_DID: &str = COUNTER_AUTHOR_TOBIAS; // did:plc:tobias-test

/// The penalty / verdict / "disputed by N" vocabulary the neutral "(N countered)" copy
/// must NEVER contain (C-6 / WD-CC-10 — the slice-14 anti-misread sensibility). A
/// countered claim is contested, not wrong; the count is disputed-claim AWARENESS, never
/// a score/deduction/verdict. The asserts scan the rendered body (lowercased) for NONE of
/// these — see [`assert_countered_copy_is_neutral`].
pub const COUNTERED_BANNED_VERDICT_COPY: &[&str] = &[
    "disputed by",   // a "by N" total (the count is presence-once, never a tally)
    "refuted",       // a verdict
    "false",         // a verdict
    "penalty",       // a deduction
    "deduction",     // a deduction
    "deducted",      // a deduction
    "invalid",       // a verdict
    "wrong",         // a verdict
    "discredited",   // a verdict
];

/// Run the ADR-055 countered-own-claims `COUNT(DISTINCT)` aggregate DIRECTLY against the
/// env's REAL DuckDB store and return the count — the SAME SQL `count_countered_own_claims`
/// will implement (ADR-055 D1). Used by the seeds to PIN the genuine countered-count
/// (e.g. assert it is exactly 3 — including the presence-once collapse of the
/// twice-countered claim) so the fixture is the REAL countered shape, not merely "the
/// verbs exited 0". This is a TEST-side oracle over the production-written rows (NO
/// hand-inserted rows): the own claims came from `claim add`, the peer counters from
/// `peer add` + `peer pull`, so the count this oracle returns is the count the production
/// read will return.
pub fn read_countered_own_claims_count(env: &TestEnv) -> usize {
    let db_path = env.duckdb_path();
    let conn = duckdb::Connection::open(&db_path).unwrap_or_else(|err| {
        panic!(
            "open DuckDB at {} for countered-own-claims count read: {err}",
            db_path.display()
        )
    });
    // The EXACT ADR-055 D1 aggregate: COUNT(DISTINCT own cid) appearing as a countered
    // referenced_cid across the two indexed ref tables (presence count via the de-duped
    // UNION IN-set + COUNT(DISTINCT) — a claim countered N times counts ONCE; own-only by
    // the outer `claims` table; parameter-free / injection-safe).
    let sql = "SELECT COUNT(DISTINCT c.cid) FROM claims c WHERE c.cid IN (\
                   SELECT referenced_cid FROM claim_references      WHERE ref_type = 'counters' \
                   UNION \
                   SELECT referenced_cid FROM peer_claim_references WHERE ref_type = 'counters')";
    conn.query_row(sql, [], |row| row.get::<_, i64>(0))
        .unwrap_or_else(|err| panic!("query countered-own-claims count: {err}")) as usize
}

/// Seed the env's REAL store to the KNOWN slice-18 landing summary — 12 own claims (real
/// `claim add`), 3 of which are countered by PEERS (real `peer add` + `peer pull` of
/// `counters`-referencing peer records targeting the own CIDs), ONE of those three
/// countered by TWO distinct peers (Rachel + Tobias) to prove the presence-once
/// `COUNT(DISTINCT)` collapse. The walking-skeleton + landing/header precondition
/// (US-CC-001/002). After seeding, `count_claims()` returns 12 and
/// `count_countered_own_claims()` returns 3 (NOT 4 — the twice-countered claim counts
/// once) — exactly the two aggregates the landing summary + `/claims` header must surface
/// as "12 own claims (3 countered)". The fixture is pinned with the direct
/// `read_countered_own_claims_count` ADR-055 oracle so it is the GENUINE countered shape.
///
/// Composition: 12 own claims via `seed_own_claims_via_cli` (distinct subjects → distinct
/// CIDs). Among them, 3 are then countered by peers:
///   - own claim A (`github:slice18/aaa`) countered by Rachel (one peer);
///   - own claim B (`github:slice18/bbb`) countered by Tobias (one peer);
///   - own claim C (`github:slice18/ccc`) countered by BOTH Rachel AND Tobias (two peers
///     → presence-once: contributes ONE to the count, never two).
/// The 3 named countered own claims are seeded via `seed_own_claim_with_evidence` (so
/// their CIDs are recoverable as the counter targets) ON TOP of the 12 plain own claims
/// — so the own-claims total is 12 + 3 = 15? NO: the 3 countered claims ARE part of the
/// 12 (the 12 includes them). DELIVER materializes the exact composition; the contract
/// this seed PINS is `count_claims == 12` AND `count_countered_own_claims == 3`, asserted
/// directly below.
///
/// The two PDS handles are returned (via `HeldSubscriptions`) so the caller keeps the
/// peers resolvable for the lifetime of the seed.
///
/// SCAFFOLD: true (slice-18) — drives the EXISTING `claim add` / `peer add` / `peer pull`
/// verbs + the slice-12 `build_verifiable_peer_counter_record` (REUSED slice-06/11/12
/// seams); the rows land in the REAL `claims` + `peer_claims` + `peer_claim_references`
/// tables the viewer's LOCAL `/` + `/claims` reads. DELIVER fills the exact own-claim
/// composition that makes `count_claims == 12` AND the 3 named claims countered; the
/// PINNING asserts below are the falsifiable contract.
pub fn seed_landing_store_with_countered_own_claims(env: &TestEnv) -> HeldSubscriptions {
    // STEP 1 — seed 3 NAMED own claims that peers will counter (their CIDs are the counter
    // targets). These are part of the 12 own claims (the remaining 9 are plain).
    let target_a = seed_own_claim_with_evidence(
        env,
        "github:slice18/aaa",
        "embodiesPhilosophy",
        "org.openlore.philosophy.dependency-pinning",
        0.90,
        &[],
    );
    let target_b = seed_own_claim_with_evidence(
        env,
        "github:slice18/bbb",
        "embodiesPhilosophy",
        "org.openlore.philosophy.memory-safety",
        0.90,
        &[],
    );
    let target_c = seed_own_claim_with_evidence(
        env,
        "github:slice18/ccc",
        "embodiesPhilosophy",
        "org.openlore.philosophy.reproducible-builds",
        0.30,
        &[],
    );

    // STEP 2 — seed the remaining 9 PLAIN own claims (no counters) so `count_claims == 12`.
    seed_own_claims_via_cli(env, LANDING_OWN_CLAIMS - COUNTERED_OWN_CLAIMS);

    // STEP 3 — peers author verifiable COUNTER records targeting the 3 own CIDs. The
    // self-counter rule means an OWN claim is countered ONLY by a PEER. Build ALL peer
    // records UP FRONT, holding each PDS ALIVE for the whole function so a SINGLE `peer
    // pull` over both peers succeeds. Rachel counters A and C; Tobias counters B and C —
    // so claim C is countered by BOTH (presence-once). Each `counters` reference lands in
    // `peer_claim_references` with `referenced_cid == <own target CID>`.
    let rachel_seed = [7u8; 32];
    let tobias_seed = [9u8; 32];
    let (rachel_counter_a, rachel_pubkey_hex) = build_verifiable_peer_counter_record(
        COUNTERED_PEER_RACHEL_DID,
        rachel_seed,
        &target_a,
        Some(COUNTER_PEER_REASON_VERBATIM),
    );
    let (rachel_counter_c, _rachel_pubkey_hex_c) = build_verifiable_peer_counter_record(
        COUNTERED_PEER_RACHEL_DID,
        rachel_seed,
        &target_c,
        Some(COUNTER_PEER_REASON_VERBATIM),
    );
    let (tobias_counter_b, tobias_pubkey_hex) = build_verifiable_peer_counter_record(
        COUNTERED_PEER_TOBIAS_DID,
        tobias_seed,
        &target_b,
        Some(COUNTER_PEER_REASON_VERBATIM),
    );
    let (tobias_counter_c, _tobias_pubkey_hex_c) = build_verifiable_peer_counter_record(
        COUNTERED_PEER_TOBIAS_DID,
        tobias_seed,
        &target_c,
        Some(COUNTER_PEER_REASON_VERBATIM),
    );

    let rachel_pds = PeerPds::for_peer(
        COUNTERED_PEER_RACHEL_DID,
        vec![rachel_counter_a, rachel_counter_c],
    );
    let tobias_pds = PeerPds::for_peer(
        COUNTERED_PEER_TOBIAS_DID,
        vec![tobias_counter_b, tobias_counter_c],
    );

    for (did, pds) in [
        (COUNTERED_PEER_RACHEL_DID, &rachel_pds),
        (COUNTERED_PEER_TOBIAS_DID, &tobias_pds),
    ] {
        let added = run_openlore_with_peer_resolver(
            env,
            &["peer", "add", did],
            did,
            pds.endpoint_url(),
        );
        assert_eq!(
            added.status, 0,
            "seed_landing_store_with_countered_own_claims: peer add for {did} must succeed;\n\
             --- stdout ---\n{}\n--- stderr ---\n{}",
            added.stdout, added.stderr
        );
    }
    let pulled = run_openlore_pull_multi(
        env,
        &["peer", "pull"],
        &[
            PeerSeam {
                peer_did: COUNTERED_PEER_RACHEL_DID,
                peer_endpoint: rachel_pds.endpoint_url(),
                peer_pubkey_hex: &rachel_pubkey_hex,
            },
            PeerSeam {
                peer_did: COUNTERED_PEER_TOBIAS_DID,
                peer_endpoint: tobias_pds.endpoint_url(),
                peer_pubkey_hex: &tobias_pubkey_hex,
            },
        ],
    );
    assert_eq!(
        pulled.status, 0,
        "seed_landing_store_with_countered_own_claims: peer pull must succeed;\n\
         --- stdout ---\n{}\n--- stderr ---\n{}",
        pulled.stdout, pulled.stderr
    );

    // PIN the GENUINE summary shape: 12 own claims AND exactly 3 countered (the
    // twice-countered claim C collapses to ONE via COUNT(DISTINCT) — the presence-once
    // contract). The direct ADR-055 oracle proves the fixture is the REAL countered shape
    // the production read will surface, NOT merely "the verbs exited 0".
    assert_user_author_claim_count(env, LANDING_OWN_CLAIMS);
    let countered = read_countered_own_claims_count(env);
    assert_eq!(
        countered, COUNTERED_OWN_CLAIMS,
        "seed_landing_store_with_countered_own_claims: the ADR-055 COUNT(DISTINCT) must be \
         exactly {COUNTERED_OWN_CLAIMS} (claim C countered by BOTH Rachel and Tobias counts \
         ONCE — presence-once); got {countered}"
    );

    HeldSubscriptions {
        _peers: vec![rachel_pds, tobias_pds],
    }
}

/// Seed an own-claims store where NONE of Maria's claims is countered (the honest-zero
/// fixture, US-CC-001/002 Theme 3 — C-5): several plain own claims via `claim add`,
/// NOTHING references any of them as a counter, so `count_countered_own_claims()` returns
/// `Some(0)` and both surfaces render "(0 countered)" — a SUCCESSFUL read of zero,
/// DISTINCT from the missing marker "(— countered)". The direct ADR-055 oracle pins the
/// count at 0.
///
/// SCAFFOLD: true (slice-18) — drives EXISTING `claim add`; NO counter is authored, NO
/// peer is added/pulled.
pub fn seed_landing_store_none_countered(env: &TestEnv) -> HeldSubscriptions {
    // 12 plain own claims, NONE countered (the own-claims count is the same 12 as the
    // headline so the "(0 countered)" honest-zero reads against the SAME own total).
    seed_own_claims_via_cli(env, LANDING_OWN_CLAIMS);

    // PIN: 12 own claims, exactly 0 countered (Some(0), an honest zero — not a failed read).
    assert_user_author_claim_count(env, LANDING_OWN_CLAIMS);
    let countered = read_countered_own_claims_count(env);
    assert_eq!(
        countered, 0,
        "seed_landing_store_none_countered: with no counter authored the ADR-055 \
         COUNT(DISTINCT) must be 0 (an honest Some(0), not a failed read); got {countered}"
    );

    HeldSubscriptions { _peers: vec![] }
}

/// Seed a store where EXACTLY ONE of Maria's own claims is countered by TWO DISTINCT
/// peers (Rachel + Tobias) and NO other claim is countered — the presence-once boundary
/// fixture (US-CC-000/001 Theme 2 — C-4 / BR-CC-1). The `COUNT(DISTINCT)` must collapse
/// the two `peer_claim_references` rows (same `referenced_cid`, two distinct authors) to
/// ONE → both surfaces render "(1 countered)", NEVER "(2 countered)". The direct ADR-055
/// oracle pins the count at 1. Mirrors the slice-12
/// `seed_claims_list_target_two_counters_distinct_authors` shape, here for the COUNT.
///
/// SCAFFOLD: true (slice-18) — drives EXISTING `claim add` + `build_verifiable_peer_
/// counter_record` + `peer add` + `peer pull`. NO hand-inserted rows.
pub fn seed_landing_store_one_own_claim_countered_twice(env: &TestEnv) -> HeldSubscriptions {
    // STEP 1 — sign several plain own claims; ONE of them is the twice-countered target.
    for (subject, predicate, object) in [
        ("github:slice18/plain-rust", "embodiesPhilosophy", "org.openlore.philosophy.memory-safety"),
        ("github:slice18/plain-deno", "embodiesPhilosophy", "org.openlore.philosophy.secure-by-default"),
    ] {
        seed_own_claim_with_evidence(env, subject, predicate, object, 0.90, &[]);
    }
    // The twice-countered target — confidence 0.30 so the anti-misread scenario can pin
    // it renders VERBATIM, never re-weighted by the count (Theme 9).
    let target_cid = seed_own_claim_with_evidence(
        env,
        "github:slice18/tdd",
        "embodiesPhilosophy",
        "org.openlore.philosophy.test-driven",
        0.30,
        &[],
    );

    // STEP 2 — TWO distinct peers each author a verifiable COUNTER referencing the SAME
    // own target CID. Build BOTH up front, hold each PDS alive, pull BOTH in ONE pull so
    // both `counters` references land in `peer_claim_references` with the SAME
    // `referenced_cid == target_cid` — two DISTINCT authors, ONE referenced CID.
    let rachel_seed = [7u8; 32];
    let (rachel_record, rachel_pubkey_hex) = build_verifiable_peer_counter_record(
        COUNTERED_PEER_RACHEL_DID,
        rachel_seed,
        &target_cid,
        Some(COUNTER_REASON_VERBATIM),
    );
    let tobias_seed = [9u8; 32];
    let (tobias_record, tobias_pubkey_hex) = build_verifiable_peer_counter_record(
        COUNTERED_PEER_TOBIAS_DID,
        tobias_seed,
        &target_cid,
        Some(COUNTER_PEER_REASON_VERBATIM),
    );
    let rachel_pds = PeerPds::for_peer(COUNTERED_PEER_RACHEL_DID, vec![rachel_record]);
    let tobias_pds = PeerPds::for_peer(COUNTERED_PEER_TOBIAS_DID, vec![tobias_record]);

    for (did, pds) in [
        (COUNTERED_PEER_RACHEL_DID, &rachel_pds),
        (COUNTERED_PEER_TOBIAS_DID, &tobias_pds),
    ] {
        let added = run_openlore_with_peer_resolver(
            env,
            &["peer", "add", did],
            did,
            pds.endpoint_url(),
        );
        assert_eq!(
            added.status, 0,
            "seed_landing_store_one_own_claim_countered_twice: peer add for {did} must succeed;\n\
             --- stdout ---\n{}\n--- stderr ---\n{}",
            added.stdout, added.stderr
        );
    }
    let pulled = run_openlore_pull_multi(
        env,
        &["peer", "pull"],
        &[
            PeerSeam {
                peer_did: COUNTERED_PEER_RACHEL_DID,
                peer_endpoint: rachel_pds.endpoint_url(),
                peer_pubkey_hex: &rachel_pubkey_hex,
            },
            PeerSeam {
                peer_did: COUNTERED_PEER_TOBIAS_DID,
                peer_endpoint: tobias_pds.endpoint_url(),
                peer_pubkey_hex: &tobias_pubkey_hex,
            },
        ],
    );
    assert_eq!(
        pulled.status, 0,
        "seed_landing_store_one_own_claim_countered_twice: peer pull must succeed;\n\
         --- stdout ---\n{}\n--- stderr ---\n{}",
        pulled.stdout, pulled.stderr
    );

    // PIN: exactly 1 countered own claim (the twice-countered target collapses to ONE via
    // COUNT(DISTINCT) — the presence-once contract; NEVER 2).
    let countered = read_countered_own_claims_count(env);
    assert_eq!(
        countered, 1,
        "seed_landing_store_one_own_claim_countered_twice: a claim countered by TWO peers \
         must count ONCE (presence-once, COUNT(DISTINCT)); got {countered}"
    );

    HeldSubscriptions {
        _peers: vec![rachel_pds, tobias_pds],
    }
}

/// Start the `openlore ui` viewer over the env's REAL store with the LOCAL
/// `count_countered_own_claims()` read forced to FAIL mid-request — the slice-18
/// graceful-degrade seam (US-CC-000/001/002 Theme 4 / C-2 / C-5 CARDINAL / WD-CC-2/6 /
/// ADR-055 D4). The own-claims + the other landing counts STILL succeed; ONLY the
/// countered-count read fails, so BOTH the landing summary and the `/claims` header must
/// render the missing marker "(— countered)" while the own-claims "12" + the other
/// counts + the nav hub + the `/claims` rows render, the page staying a normal 200 (never
/// a 5xx / blank / raw stack trace). The failed read maps to `None` (`.ok()`), DISTINCT
/// from a fabricated `Some(0)` → "(0 countered)".
///
/// SEEDING-SEAM NOTE (documented DISTILL choice, mirroring the slice-17 LD-DEGRADE
/// precedent): the slice-06/15 viewer harness holds ONE long-lived DuckDB connection
/// taken at STARTUP, so the existing `make_store_unreadable` lock would refuse STARTUP
/// rather than exercise a MID-REQUEST per-count read failure. There is NO
/// readily-available mid-request per-count read-failure seam in the slice-06/15 harness.
/// Per the DISTILL guidance, the OBSERVABLE missing-marker contract (a failed
/// countered-count read → "(— countered)" while the own-claims count + rows render, page
/// 200) is scaffolded against a TEST-ONLY effect-shell fault seam (the
/// `OPENLORE_VIEWER_FAIL_COUNTERED_COUNT` env var, threaded by `start_inner`); the
/// SUCCESSFUL-zero distinction is fully exercisable today via
/// `seed_landing_store_none_countered`. DELIVER materializes the per-count fault seam (a
/// `#[cfg(debug_assertions)]`-gated, release-forbidden, xtask-guarded effect-shell branch
/// substituting `Err(StoreReadError)` for the REAL `count_countered_own_claims()` read on
/// BOTH the `/` and `/claims` handlers) with the SAME observable target — exactly as
/// slice-17 materialized `OPENLORE_VIEWER_FAIL_PEER_CLAIMS_COUNT`. Until then the scenario
/// panics at the `todo!()` `start_inner` body (slice-06) → RED MISSING_FUNCTIONALITY,
/// never BROKEN.
///
/// SCAFFOLD: true (slice-18).
pub fn start_viewer_with_failing_countered_count(env: &TestEnv) -> ViewerServer {
    ViewerServer::start_inner(env, None, None, None, false, false, true, false)
}

/// Assert the LANDING render shows the countered count "(`n` countered)" beside the
/// own-claims line (US-CC-001 — the headline "12 own claims (3 countered)"). Universe
/// (Mandate 8 — port-exposed rendered surface): the rendered body contains the exact
/// parenthetical `"({n} countered)"` (the `render_countered(Some(n))` output, ADR-055 D3).
/// Scans the OBSERVABLE HTML the operator's browser shows; never an internal
/// `LandingSummary` field. The caller separately asserts the own-claims "12" still renders
/// (the count is additive, never a re-weight — C-4).
///
/// SCAFFOLD: true (slice-18).
pub fn assert_landing_countered_count(body: &str, n: usize) {
    let needle = format!("({n} countered)");
    assert!(
        body.contains(&needle),
        "the landing summary must show the countered-own-claims count {needle:?} beside the \
         own-claims count (US-CC-001 — \"12 own claims (3 countered)\"; the genuine seeded \
         presence count, ADR-055 D3); body was:\n{body}"
    );
}

/// Assert the LANDING render shows the MISSING-marker "(— countered)" for the countered
/// count (a FAILED read, ADR-055 D4 / C-5), DISTINCT from a successful "(0 countered)".
/// Universe (port-exposed rendered surface): the rendered body contains the exact
/// `"(— countered)"` parenthetical (the `render_countered(None)` output). We scan the
/// COUNTERED-COUNT POSITION, NOT the bare marker: the page chrome title ("OpenLore —
/// Viewer") legitimately carries the em-dash, so a bare-marker scan would collide with the
/// title and pass trivially even when the count rendered a number. Used by the failed-read
/// degrade scenario (Theme 4). The caller separately asserts the own-claims count still
/// renders + the page is 200 (the degrade is per-count, independent).
///
/// SCAFFOLD: true (slice-18).
pub fn assert_landing_countered_missing(body: &str) {
    let needle = format!("({COUNTERED_MISSING_MARKER} countered)");
    assert!(
        body.contains(&needle),
        "a FAILED countered-count read must render the missing-marker at the count position \
         {needle:?} — DISTINCT from a fabricated \"(0 countered)\" AND from the chrome title's \
         em-dash (ADR-055 D4 / C-5); body was:\n{body}"
    );
    // The failed read must NOT fabricate a "(0 countered)" (that would mislead "nothing of
    // mine is disputed" — C-5 / R-CC-1). `Option` makes 0 ≠ missing representable.
    assert!(
        !body.contains("(0 countered)"),
        "a FAILED countered-count read must NOT fabricate \"(0 countered)\" — `None` renders \
         the missing marker, never a fabricated zero (C-5); body was:\n{body}"
    );
}

/// Assert the `/claims` HEADER render shows the countered count "(`n` countered)" — the
/// SAME helper output the landing renders (US-CC-002, single source — ADR-055 D3).
/// Universe (port-exposed rendered surface): the rendered `/claims` body contains the
/// exact `"({n} countered)"` parenthetical in the header region (near the "My Claims"
/// heading + read-only notice). The caller separately asserts the list rows / order /
/// paging are byte-identical to the no-header-count baseline (the header count is
/// additive, never a re-order/filter/re-weight — C-4 / WD-CC-9).
///
/// SCAFFOLD: true (slice-18).
pub fn assert_claims_header_countered_count(body: &str, n: usize) {
    let needle = format!("({n} countered)");
    assert!(
        body.contains(&needle),
        "the `/claims` list header must show the countered count {needle:?} (the SAME \
         single-source count the landing shows, US-CC-002 / ADR-055 D3); body was:\n{body}"
    );
}

/// Assert the `/claims` HEADER render shows the MISSING-marker "(— countered)" for the
/// countered count (a FAILED read, ADR-055 D4 / C-5) while the list rows still render.
/// Universe (port-exposed rendered surface): the rendered `/claims` body contains the
/// exact `"(— countered)"` parenthetical, NOT a fabricated "(0 countered)". Used by the
/// `/claims` failed-read degrade scenario (Theme 4). The caller separately asserts the
/// list rows still render + the page is 200.
///
/// SCAFFOLD: true (slice-18).
pub fn assert_claims_header_countered_missing(body: &str) {
    let needle = format!("({COUNTERED_MISSING_MARKER} countered)");
    assert!(
        body.contains(&needle),
        "a FAILED countered-count read on `/claims` must render the missing-marker at the \
         header count position {needle:?} — DISTINCT from a fabricated \"(0 countered)\" \
         (ADR-055 D4 / C-5); body was:\n{body}"
    );
    assert!(
        !body.contains("(0 countered)"),
        "a FAILED countered-count read on `/claims` must NOT fabricate \"(0 countered)\" \
         (C-5); body was:\n{body}"
    );
}

/// Assert the LANDING "(N countered)" count EQUALS the `/claims` header "(N countered)"
/// count for the SAME store (US-CC-002 single-source consistency — WD-CC-8 / R-CC-6). The
/// shared `render_countered` helper renders the SAME number on both surfaces; this pins
/// the equality on the OBSERVABLE rendered surfaces (Mandate 8). Universe (port-exposed):
/// the `"(N countered)"` parenthetical extracted from each body matches. A divergence is
/// an UNSHIPPABLE single-source breach (the two orientation surfaces must agree).
///
/// SCAFFOLD: true (slice-18).
pub fn assert_landing_and_claims_countered_consistent(landing_body: &str, claims_body: &str) {
    let landing_count = extract_countered_parenthetical(landing_body).unwrap_or_else(|| {
        panic!(
            "assert_landing_and_claims_countered_consistent: the landing body must carry a \
             \"(N countered)\" parenthetical to compare; body was:\n{landing_body}"
        )
    });
    let claims_count = extract_countered_parenthetical(claims_body).unwrap_or_else(|| {
        panic!(
            "assert_landing_and_claims_countered_consistent: the `/claims` body must carry a \
             \"(N countered)\" parenthetical to compare; body was:\n{claims_body}"
        )
    });
    assert_eq!(
        landing_count, claims_count,
        "the landing \"({landing_count} countered)\" must EQUAL the `/claims` header \
         \"({claims_count} countered)\" for the same store (single source — WD-CC-8 / \
         R-CC-6); the two orientation surfaces diverged"
    );
}

/// Extract the inner token of the FIRST `"(<token> countered)"` parenthetical in a
/// rendered body (the digit string, or "—" for the missing marker). Returns `None` when
/// no countered parenthetical is present. Used by
/// [`assert_landing_and_claims_countered_consistent`] to compare the two surfaces'
/// rendered counts on the OBSERVABLE surface (never an internal field).
fn extract_countered_parenthetical(body: &str) -> Option<String> {
    let suffix = " countered)";
    let close = body.find(suffix)?;
    // Walk back to the opening '(' before the token.
    let open = body[..close].rfind('(')?;
    Some(body[open + 1..close].trim().to_string())
}

/// Assert the countered-count COPY is NEUTRAL disputed-claim awareness — it carries NONE
/// of the penalty / deduction / verdict / "disputed by N" vocabulary (C-6 / WD-CC-10 —
/// the slice-14 anti-misread sensibility). Universe (port-exposed rendered surface): the
/// rendered body (lowercased) contains none of [`COUNTERED_BANNED_VERDICT_COPY`]. A
/// countered claim is contested, not wrong; the count is awareness, never a score. Used
/// by the anti-misread scenario (Theme 9). NOTE: scans the WHOLE rendered surface around
/// the count — the count must read as neutral presence, never a verdict.
///
/// SCAFFOLD: true (slice-18).
pub fn assert_countered_copy_is_neutral(body: &str) {
    let lowered = body.to_ascii_lowercase();
    for banned in COUNTERED_BANNED_VERDICT_COPY {
        assert!(
            !lowered.contains(&banned.to_ascii_lowercase()),
            "C-6 / WD-CC-10 (slice-18 anti-misread): the countered-count copy must be NEUTRAL \
             disputed-claim awareness — never a penalty / deduction / verdict / \"disputed by \
             N\" total; found {banned:?} in body:\n{body}"
        );
    }
}

// =============================================================================
// Slice-19 (viewer-peer-counter-aware-counts; DISTILL) — the COUNTERED-PEER-CLAIMS count
// rendered beside the peer-claims count on the `GET /` landing summary AND in the
// `GET /peer-claims` list header: "4 peer claims (1 countered)" (US-PC-000/001/002;
// ADR-056). The deferred PEER sibling of slice-18. EXTENDS the slice-17/18 `LandingSummary`
// with a FIFTH additive `Option<usize>` field `countered_peer_claims` (`.ok()`-degraded),
// threaded into `render_landing` (beside the unchanged PEER line) + a NEW
// `render_peer_claims_page` `countered_peer_claims: Option<usize>` param via the REUSED
// pure `render_countered(Option<usize>) -> String` helper slice-18 established (single
// source — ADR-056 D3; NO new helper). The count is `count_countered_peer_claims()` = the
// EXACT slice-18 SQL with the OUTER table swapped `claims c → peer_claims p`:
// `COUNT(DISTINCT p.cid) FROM peer_claims p WHERE p.cid IN (SELECT referenced_cid FROM
// claim_references WHERE ref_type='counters' UNION SELECT referenced_cid FROM
// peer_claim_references WHERE ref_type='counters')` — a presence count (a peer claim
// countered by N counterers counts ONCE), peer-only by query shape, invariant to store size
// (ADR-056 D1). The inner `UNION` IN-set is BYTE-IDENTICAL to slice-18's — only the outer
// table differs.
//
// The data shape (the load-bearing seeding fact): a cached PEER claim is countered by EITHER
//   - the OPERATOR (her counter authored via `claim counter <peer_cid>` lands in the user's
//     OWN `claims` table → `claim_references` arm with `referenced_cid == <peer cid>`), OR
//   - ANOTHER PEER (their `counters`-referencing record landing via `peer add` + `peer pull`
//     in `peer_claim_references` with `referenced_cid == <peer cid>`).
// Both arms of the UNION IN-set contribute; the slice-19 seeds exercise BOTH. One peer claim
// countered by TWO distinct counterers (the operator + a peer) proves the presence-once
// `COUNT(DISTINCT)` collapse (→ "(1 countered)", never "(2 countered)").
//
// The asserts scan ONLY the rendered HTML the operator's browser shows (Mandate 8 universe =
// port-exposed rendered surface, never an internal `LandingSummary` field). NO scenario calls
// `render_landing` / `render_peer_claims_page` / `render_countered` / the count read directly
// (those are unit/property-level, exercised in DELIVER) — every assertion is on the `GET /`
// or `GET /peer-claims` HTTP response (Mandate 1 driving-port discipline). The seeds pin the
// GENUINE countered-peer-count with a DIRECT DuckDB `COUNT(DISTINCT)` assert (the SAME
// ADR-056 SQL with outer `peer_claims`), so the fixture is the REAL countered shape, not
// merely "the verbs exited 0".
//
// Layer placement (Mandate 9/11): every scenario is a layer-3/layer-5 subprocess + real-I/O
// test — EXAMPLE-only. The sad paths (honest "(0 countered)", failed countered-peer-count
// read → "(— countered)") are enumerated explicitly, never PBT-generated at this layer. Tier
// B (state-machine PBT) is NOT warranted: a single-shot additive render with no chained
// ≥3-scenario journey and no domain-rich input space (one Option<usize>) — Tier A example
// coverage is exact (Mandate 10 skip criteria).
//
// SCAFFOLD: true (slice-19) — the seeds + asserts COMPILE now (they drive EXISTING `peer add`
// / `peer pull` / `claim counter` verbs + the slice-12/13 `build_verifiable_peer_counter_
// record` + `seed_peer_claims_one_countered` + REUSE the slice-18 `read_countered_*`/render
// asserts); the SCENARIOS stay RED because the production `/` + `/peer-claims` routes do NOT
// render the countered-peer count yet, and `count_countered_peer_claims` / the 5th
// `LandingSummary` field / the `render_peer_claims_page` `Option<usize>` param do NOT exist —
// so "(1 countered)" on the PEER line / `/peer-claims` header is ABSENT → RED
// MISSING_FUNCTIONALITY, never BROKEN. The missing≠zero failed-read scenario drives the
// test-only `OPENLORE_VIEWER_FAIL_COUNTERED_PEER_COUNT` (4th DISTINCT token) effect-shell
// fault seam (slice-18 precedent), panicking at the `start_inner` `todo!()` body until DELIVER
// materializes it — also MISSING_FUNCTIONALITY.
// =============================================================================

/// The `/peer-claims` list route path (the slice-06/07 federated Peer Claims list, whose
/// HEADER this slice extends with the countered count "(N countered)" — US-PC-002).
pub const PEER_CLAIMS_LIST_PATH: &str = "/peer-claims";

/// The headline cached-peer-claims TOTAL the slice-19 headline seed pins: Maria caches 4
/// peer claims (the "4" beside which "(1 countered)" renders — "4 peer claims (1 countered)",
/// US-PC-001). One source of truth so the seed and the assertions agree.
pub const LANDING_COUNTERED_PEER_TOTAL: usize = 4;

/// The headline countered-PEER-claims count the slice-19 seeds pin: of Maria's 4 cached peer
/// claims, EXACTLY 1 is countered. The brief's headline number ("4 peer claims (1 countered)",
/// US-PC-001). One source of truth so the seed and the assertions agree.
pub const COUNTERED_PEER_CLAIMS: usize = 1;

/// Run the ADR-056 countered-PEER-claims `COUNT(DISTINCT)` aggregate DIRECTLY against the
/// env's REAL DuckDB store and return the count — the SAME SQL `count_countered_peer_claims`
/// will implement (ADR-056 D1): the EXACT slice-18 `read_countered_own_claims_count` oracle
/// with the OUTER table swapped `claims c → peer_claims p`. Used by the seeds to PIN the
/// genuine countered-peer-count (e.g. assert it is exactly 1 — including the presence-once
/// collapse of a peer claim countered by two counterers) so the fixture is the REAL countered
/// shape, not merely "the verbs exited 0". A TEST-side oracle over the production-written rows
/// (NO hand-inserted rows): the peer claims came from `peer add` + `peer pull`, the counters
/// from `claim counter` (own arm) + `peer pull` (peer arm), so the count this oracle returns
/// is the count the production read will return.
pub fn read_countered_peer_claims_count(env: &TestEnv) -> usize {
    let db_path = env.duckdb_path();
    let conn = duckdb::Connection::open(&db_path).unwrap_or_else(|err| {
        panic!(
            "open DuckDB at {} for countered-peer-claims count read: {err}",
            db_path.display()
        )
    });
    // The EXACT ADR-056 D1 aggregate: COUNT(DISTINCT peer cid) appearing as a countered
    // referenced_cid across the two indexed ref tables (presence count via the de-duped
    // UNION IN-set + COUNT(DISTINCT) — a peer claim countered N times counts ONCE; peer-only
    // by the outer `peer_claims` table; parameter-free / injection-safe). The inner UNION
    // IN-set is BYTE-IDENTICAL to slice-18's — only the outer table differs.
    let sql = "SELECT COUNT(DISTINCT p.cid) FROM peer_claims p WHERE p.cid IN (\
                   SELECT referenced_cid FROM claim_references      WHERE ref_type = 'counters' \
                   UNION \
                   SELECT referenced_cid FROM peer_claim_references WHERE ref_type = 'counters')";
    conn.query_row(sql, [], |row| row.get::<_, i64>(0))
        .unwrap_or_else(|err| panic!("query countered-peer-claims count: {err}")) as usize
}

/// Counter a single cached PEER claim (`peer_cid`) via the OPERATOR's OWN `claim counter`
/// verb — the `claim_references` arm of the countered-peer-count UNION. The operator CAN
/// counter a PEER claim (the self-counter rule only blocks countering her OWN claim); the
/// counter lands in the user's OWN `claims` table carrying `references[].type == counters`
/// whose `cid == peer_cid` (ADR-015) → a `claim_references` row with `referenced_cid ==
/// peer_cid`. Confirms the sign prompt, DECLINES publish (the read path needs only the LOCAL
/// row). Reuses the slice-11 `seed_claim_with_counter` operator-counter mechanism.
///
/// SCAFFOLD: true (slice-19) — drives the EXISTING slice-03 `claim counter` verb.
fn counter_peer_claim_by_operator(env: &TestEnv, peer_cid: &str) {
    let outcome = run_openlore_with_stdin(
        env,
        &["claim", "counter", peer_cid, "--reason", COUNTER_REASON_VERBATIM],
        "\nN\n",
    );
    assert_eq!(
        outcome.status, 0,
        "counter_peer_claim_by_operator: `claim counter` against the peer cid {peer_cid:?} must \
         exit 0 (the operator CAN counter a PEER claim — only her OWN claim is self-counter \
         blocked);\n--- stdout ---\n{}\n--- stderr ---\n{}",
        outcome.stdout, outcome.stderr
    );
}

/// Seed the env's REAL store to the KNOWN slice-19 headline shape — 4 cached PEER claims
/// (Rachel's, via real `peer add` + `peer pull`), 1 of which is countered by the OPERATOR
/// (her `claim counter` against ONE of Rachel's peer-claim CIDs, landing in
/// `claim_references`). The walking-skeleton + landing/header precondition (US-PC-001/002).
/// After seeding, `count_peer_claims()` returns 4 (the operator's counter lands in her OWN
/// `claims` table, NOT `peer_claims`, so the cached-peer-claims TOTAL stays a clean 4) and
/// `count_countered_peer_claims()` returns 1 — exactly the two aggregates the landing summary
/// + `/peer-claims` header must surface as "4 peer claims (1 countered)". The fixture is
/// pinned with the direct `read_countered_peer_claims_count` ADR-056 oracle so it is the
/// GENUINE countered shape. Returns the [`SeededPeerClaimsList`] (REUSED from slice-13) so the
/// caller can address the countered + un-countered rows by their rendered order.
///
/// Composition: Rachel hosts 4 plain surveyed peer claims (→ `peer_claims`); the OPERATOR
/// counters ONE of them via `claim counter <peer_cid>` (→ her OWN `claims` row carrying a
/// `references[].type == counters` entry whose `cid == peer_cid` → a `claim_references` row
/// with `referenced_cid == peer_cid`). The operator-counter arm keeps the cached-peer total a
/// clean 4 (a distinct-peer counter would add a 5th `peer_claims` row); both arms of the
/// UNION IN-set count a peer cid, so a single counted peer claim from EITHER arm is exercised
/// across the seeds (the distinct-peer arm is exercised by the each-arm + countered-twice
/// seeds). The countered peer claim is Rachel's at confidence 0.40 (so the anti-misread
/// scenario can pin it renders VERBATIM).
///
/// SCAFFOLD: true (slice-19) — drives the EXISTING `peer add` / `peer pull` verbs (Rachel's 4
/// plain claims) + the slice-03 `claim counter` verb (the operator's counter of ONE peer cid,
/// the `claim_references` arm). The rows land in the REAL `peer_claims` + `claim_references`
/// tables the viewer's LOCAL `/` + `/peer-claims` reads. The PINNING asserts below are the
/// falsifiable contract (`count_peer_claims == 4` AND `count_countered_peer_claims == 1`).
pub fn seed_landing_store_with_countered_peer_claims(env: &TestEnv) -> SeededPeerClaimsList {
    // Rachel hosts 4 plain surveyed peer claims; the operator counters the FIRST (at 0.40 so
    // the anti-misread scenario can pin verbatim confidence).
    let surveyed_peer = COUNTER_TARGET_AUTHOR_RACHEL;
    let rachel_seed = [7u8; 32];
    let (rachel_records, rachel_pubkey_hex) = build_verifiable_peer_records_for_triples(
        surveyed_peer,
        rachel_seed,
        &[
            ("github:peer/rachel-axum", "org.openlore.philosophy.ergonomics", 0.40),
            ("github:peer/rachel-tokio", "org.openlore.philosophy.async-runtime", 0.70),
            ("github:peer/rachel-serde", "org.openlore.philosophy.zero-copy", 0.70),
            ("github:peer/rachel-hyper", "org.openlore.philosophy.composability", 0.70),
        ],
    );
    // Counter Rachel's FIRST surveyed claim — its deterministic CID is the counter target.
    let target_cid = rachel_records
        .first()
        .expect("seed_landing_store_with_countered_peer_claims: Rachel's first surveyed record")
        .rkey
        .clone();

    let rachel_pds = PeerPds::for_peer(surveyed_peer, rachel_records);

    let added = run_openlore_with_peer_resolver(
        env,
        &["peer", "add", surveyed_peer],
        surveyed_peer,
        rachel_pds.endpoint_url(),
    );
    assert_eq!(
        added.status, 0,
        "seed_landing_store_with_countered_peer_claims: peer add for {surveyed_peer} must \
         succeed;\n--- stdout ---\n{}\n--- stderr ---\n{}",
        added.stdout, added.stderr
    );
    let pulled = run_openlore_pull_multi(
        env,
        &["peer", "pull"],
        &[PeerSeam {
            peer_did: surveyed_peer,
            peer_endpoint: rachel_pds.endpoint_url(),
            peer_pubkey_hex: &rachel_pubkey_hex,
        }],
    );
    assert_eq!(
        pulled.status, 0,
        "seed_landing_store_with_countered_peer_claims: peer pull must succeed;\n\
         --- stdout ---\n{}\n--- stderr ---\n{}",
        pulled.stdout, pulled.stderr
    );

    // Recover the SURVEYED peer's claim CIDs; split the ONE countered target + the rest.
    let surveyed_cids = read_peer_claim_cids_for(env, surveyed_peer);
    assert!(
        surveyed_cids.contains(&target_cid),
        "seed_landing_store_with_countered_peer_claims: the countered target CID {target_cid:?} \
         must be among the surveyed peer's claims; got {surveyed_cids:?}"
    );

    // The OPERATOR counters the target peer claim via `claim counter` (the `claim_references`
    // arm — keeps the cached-peer TOTAL a clean 4, since the counter lands in OWN `claims`).
    counter_peer_claim_by_operator(env, &target_cid);

    let uncountered_cids = surveyed_cids
        .iter()
        .filter(|cid| *cid != &target_cid)
        .cloned()
        .collect::<Vec<_>>();
    let ordered_cids = read_peer_claim_cids_in_list_order(env);

    // PIN the GENUINE shape: 4 cached peer claims (Rachel's; the operator's counter is in OWN
    // `claims`, NOT `peer_claims`, so the cached-peer total is a clean 4) AND exactly 1
    // countered (the ADR-056 COUNT(DISTINCT) over peer_claims). The direct oracle proves the
    // fixture is the REAL countered shape the production read will surface, NOT merely "the
    // verbs exited 0".
    assert_peer_claims_row_count_for(env, surveyed_peer, LANDING_COUNTERED_PEER_TOTAL);
    let countered = read_countered_peer_claims_count(env);
    assert_eq!(
        countered, COUNTERED_PEER_CLAIMS,
        "seed_landing_store_with_countered_peer_claims: the ADR-056 COUNT(DISTINCT) over \
         peer_claims must be exactly {COUNTERED_PEER_CLAIMS}; got {countered}"
    );

    SeededPeerClaimsList {
        ordered_cids,
        countered_cids: vec![target_cid],
        uncountered_cids,
        peer_did: surveyed_peer.to_string(),
    }
}

/// Seed a cached-peer-claims store where NONE of Maria's cached peer claims is countered
/// (the honest-zero fixture, US-PC-001/002 Theme 3 — C-5): exactly 4 plain cached peer
/// claims (so the peer total matches the headline "4 peer claims") via the production `peer
/// add` + `peer pull` path, NOTHING references any of them as a counter, so
/// `count_countered_peer_claims()` returns `Some(0)` and both surfaces render "(0 countered)"
/// — a SUCCESSFUL read of zero, DISTINCT from the missing marker "(— countered)". The direct
/// ADR-056 oracle pins the count at 0.
///
/// SCAFFOLD: true (slice-19) — drives the EXISTING `peer add` + `peer pull` over Rachel's 4
/// plain triples; NO counter is authored (neither operator nor peer arm), so the ADR-056
/// COUNT(DISTINCT) is an honest Some(0).
pub fn seed_landing_store_no_peer_claim_countered(env: &TestEnv) -> SeededPeerClaimsList {
    // Rachel hosts 4 plain surveyed peer claims; NOTHING counters any of them — the peer total
    // is a clean 4 (matching the headline), the countered count an honest Some(0).
    let surveyed_peer = COUNTER_TARGET_AUTHOR_RACHEL;
    let rachel_seed = [7u8; 32];
    let (rachel_records, rachel_pubkey_hex) = build_verifiable_peer_records_for_triples(
        surveyed_peer,
        rachel_seed,
        &[
            ("github:peer/rachel-axum", "org.openlore.philosophy.ergonomics", 0.70),
            ("github:peer/rachel-tokio", "org.openlore.philosophy.async-runtime", 0.70),
            ("github:peer/rachel-serde", "org.openlore.philosophy.zero-copy", 0.70),
            ("github:peer/rachel-hyper", "org.openlore.philosophy.composability", 0.70),
        ],
    );
    let rachel_pds = PeerPds::for_peer(surveyed_peer, rachel_records);

    let added = run_openlore_with_peer_resolver(
        env,
        &["peer", "add", surveyed_peer],
        surveyed_peer,
        rachel_pds.endpoint_url(),
    );
    assert_eq!(
        added.status, 0,
        "seed_landing_store_no_peer_claim_countered: peer add for {surveyed_peer} must \
         succeed;\n--- stdout ---\n{}\n--- stderr ---\n{}",
        added.stdout, added.stderr
    );
    let pulled = run_openlore_pull_multi(
        env,
        &["peer", "pull"],
        &[PeerSeam {
            peer_did: surveyed_peer,
            peer_endpoint: rachel_pds.endpoint_url(),
            peer_pubkey_hex: &rachel_pubkey_hex,
        }],
    );
    assert_eq!(
        pulled.status, 0,
        "seed_landing_store_no_peer_claim_countered: peer pull must succeed;\n\
         --- stdout ---\n{}\n--- stderr ---\n{}",
        pulled.stdout, pulled.stderr
    );

    let ordered_cids = read_peer_claim_cids_in_list_order(env);

    // PIN: 4 cached peer claims AND exactly 0 countered (Some(0), an honest zero — not a
    // failed read).
    assert_peer_claims_row_count_for(env, surveyed_peer, LANDING_COUNTERED_PEER_TOTAL);
    let countered = read_countered_peer_claims_count(env);
    assert_eq!(
        countered, 0,
        "seed_landing_store_no_peer_claim_countered: with no counter authored the ADR-056 \
         COUNT(DISTINCT) over peer_claims must be 0 (an honest Some(0), not a failed read); \
         got {countered}"
    );

    SeededPeerClaimsList {
        ordered_cids: ordered_cids.clone(),
        countered_cids: vec![],
        uncountered_cids: ordered_cids,
        peer_did: surveyed_peer.to_string(),
    }
}

/// Seed a store where EXACTLY ONE cached peer claim is countered by TWO DISTINCT counterers
/// (the OPERATOR via `claim counter` → `claim_references` AND another peer via `peer pull`
/// → `peer_claim_references`) and NO other cached peer claim is countered — the presence-once
/// boundary fixture (US-PC-000/001 Theme 2 — C-4 / BR-PC-1). The `COUNT(DISTINCT)` must
/// collapse the two ref rows (same `referenced_cid == peer target`, two distinct tables /
/// authors) to ONE → both surfaces render "(1 countered)", NEVER "(2 countered)". The direct
/// ADR-056 oracle pins the count at 1. The countered peer claim is Tobias's at confidence
/// 0.40 (so the anti-misread scenario can pin it renders VERBATIM, never re-weighted).
///
/// SCAFFOLD: true (slice-19) — drives EXISTING `peer add` + `peer pull` (Tobias hosts the
/// target peer claim at 0.40; Rachel hosts a peer counter of it) + `claim counter` (the
/// operator's own counter of the SAME peer cid). NO hand-inserted rows.
pub fn seed_landing_store_one_peer_claim_countered_twice(env: &TestEnv) -> SeededPeerClaimsList {
    // STEP 1 — Tobias hosts the TARGET peer claim (confidence 0.40); Rachel hosts a verifiable
    // COUNTER of it (the `peer_claim_references` arm). Build BOTH peers' records UP FRONT,
    // holding each PDS alive, so a SINGLE `peer pull` over both lands everything. Tobias's
    // target CID is DETERMINISTIC, so Rachel's counter can reference it before either pulls.
    let tobias_seed = [9u8; 32];
    let (tobias_records, tobias_pubkey_hex) = build_verifiable_peer_records_for_triples(
        COUNTER_AUTHOR_TOBIAS,
        tobias_seed,
        &[
            ("github:peer/tobias-rust", "org.openlore.philosophy.memory-safety", 0.40),
            ("github:peer/tobias-tokio", "org.openlore.philosophy.async-runtime", 0.70),
        ],
    );
    let target_cid = tobias_records
        .first()
        .expect("seed_landing_store_one_peer_claim_countered_twice: Tobias's first record")
        .rkey
        .clone();

    let rachel_seed = [7u8; 32];
    let (rachel_counter, rachel_pubkey_hex) = build_verifiable_peer_counter_record(
        COUNTER_TARGET_AUTHOR_RACHEL,
        rachel_seed,
        &target_cid,
        Some(COUNTER_PEER_REASON_VERBATIM),
    );

    let tobias_pds = PeerPds::for_peer(COUNTER_AUTHOR_TOBIAS, tobias_records);
    let rachel_pds = PeerPds::for_peer(COUNTER_TARGET_AUTHOR_RACHEL, vec![rachel_counter]);

    for (did, pds) in [
        (COUNTER_AUTHOR_TOBIAS, &tobias_pds),
        (COUNTER_TARGET_AUTHOR_RACHEL, &rachel_pds),
    ] {
        let added = run_openlore_with_peer_resolver(env, &["peer", "add", did], did, pds.endpoint_url());
        assert_eq!(
            added.status, 0,
            "seed_landing_store_one_peer_claim_countered_twice: peer add for {did} must succeed;\n\
             --- stdout ---\n{}\n--- stderr ---\n{}",
            added.stdout, added.stderr
        );
    }
    let pulled = run_openlore_pull_multi(
        env,
        &["peer", "pull"],
        &[
            PeerSeam {
                peer_did: COUNTER_AUTHOR_TOBIAS,
                peer_endpoint: tobias_pds.endpoint_url(),
                peer_pubkey_hex: &tobias_pubkey_hex,
            },
            PeerSeam {
                peer_did: COUNTER_TARGET_AUTHOR_RACHEL,
                peer_endpoint: rachel_pds.endpoint_url(),
                peer_pubkey_hex: &rachel_pubkey_hex,
            },
        ],
    );
    assert_eq!(
        pulled.status, 0,
        "seed_landing_store_one_peer_claim_countered_twice: peer pull must succeed;\n\
         --- stdout ---\n{}\n--- stderr ---\n{}",
        pulled.stdout, pulled.stderr
    );

    // STEP 2 — the OPERATOR ALSO counters the SAME peer target via `claim counter` (the
    // `claim_references` arm). Now the SAME peer CID is referenced by BOTH a `claim_references`
    // row (Maria) AND a `peer_claim_references` row (Rachel) — two distinct counterers, ONE
    // referenced peer CID. The de-duped UNION + COUNT(DISTINCT) must collapse to 1.
    counter_peer_claim_by_operator(env, &target_cid);

    // PIN: exactly 1 countered peer claim (the twice-countered target collapses to ONE via
    // COUNT(DISTINCT) — the presence-once contract; NEVER 2).
    let countered = read_countered_peer_claims_count(env);
    assert_eq!(
        countered, 1,
        "seed_landing_store_one_peer_claim_countered_twice: a peer claim countered by TWO \
         counterers (operator + peer) must count ONCE (presence-once, COUNT(DISTINCT)); got \
         {countered}"
    );

    let surveyed_cids = read_peer_claim_cids_for(env, COUNTER_AUTHOR_TOBIAS);
    let uncountered_cids = surveyed_cids
        .iter()
        .filter(|cid| *cid != &target_cid)
        .cloned()
        .collect::<Vec<_>>();
    let ordered_cids = read_peer_claim_cids_in_list_order(env);

    SeededPeerClaimsList {
        ordered_cids,
        countered_cids: vec![target_cid],
        uncountered_cids,
        peer_did: COUNTER_AUTHOR_TOBIAS.to_string(),
    }
}

/// Seed a store where TWO distinct cached peer claims are countered, ONE through EACH ref
/// table arm — the either-table-contributes-once fixture (US-PC-000/001 Theme 2 Ex 2 —
/// C-4 / BR-PC-1). Peer claim A is countered by the OPERATOR (`claim counter` →
/// `claim_references`); peer claim B is countered by ANOTHER peer (`peer pull` →
/// `peer_claim_references`); no other cached peer claim is countered. The de-duped UNION
/// across BOTH arms must sum to 2 — each countered peer claim contributing EXACTLY ONCE
/// regardless of which ref table holds its counter. The direct ADR-056 oracle pins 2.
///
/// SCAFFOLD: true (slice-19) — drives EXISTING `peer add` + `peer pull` + `claim counter`.
pub fn seed_landing_store_peer_claims_countered_each_arm(env: &TestEnv) -> SeededPeerClaimsList {
    // STEP 1 — Rachel hosts the two surveyed peer claims (A + B). Tobias hosts a verifiable
    // COUNTER of claim B (the `peer_claim_references` arm). Build records UP FRONT, hold both
    // PDS alive, pull both in ONE invocation.
    let surveyed_peer = COUNTER_TARGET_AUTHOR_RACHEL;
    let rachel_seed = [7u8; 32];
    let (rachel_records, rachel_pubkey_hex) = build_verifiable_peer_records_for_triples(
        surveyed_peer,
        rachel_seed,
        &[
            ("github:peer/rachel-arm-a", "org.openlore.philosophy.ergonomics", 0.70),
            ("github:peer/rachel-arm-b", "org.openlore.philosophy.zero-copy", 0.70),
        ],
    );
    // Claim A — countered by the operator (`claim_references` arm). Claim B — countered by
    // Tobias (`peer_claim_references` arm).
    let target_a = rachel_records
        .first()
        .expect("seed_landing_store_peer_claims_countered_each_arm: Rachel's claim A")
        .rkey
        .clone();
    let target_b = rachel_records
        .get(1)
        .expect("seed_landing_store_peer_claims_countered_each_arm: Rachel's claim B")
        .rkey
        .clone();

    let tobias_seed = [9u8; 32];
    let (tobias_counter_b, tobias_pubkey_hex) = build_verifiable_peer_counter_record(
        COUNTER_AUTHOR_TOBIAS,
        tobias_seed,
        &target_b,
        Some(COUNTER_PEER_REASON_VERBATIM),
    );

    let rachel_pds = PeerPds::for_peer(surveyed_peer, rachel_records);
    let tobias_pds = PeerPds::for_peer(COUNTER_AUTHOR_TOBIAS, vec![tobias_counter_b]);

    for (did, pds) in [
        (surveyed_peer, &rachel_pds),
        (COUNTER_AUTHOR_TOBIAS, &tobias_pds),
    ] {
        let added = run_openlore_with_peer_resolver(env, &["peer", "add", did], did, pds.endpoint_url());
        assert_eq!(
            added.status, 0,
            "seed_landing_store_peer_claims_countered_each_arm: peer add for {did} must succeed;\n\
             --- stdout ---\n{}\n--- stderr ---\n{}",
            added.stdout, added.stderr
        );
    }
    let pulled = run_openlore_pull_multi(
        env,
        &["peer", "pull"],
        &[
            PeerSeam {
                peer_did: surveyed_peer,
                peer_endpoint: rachel_pds.endpoint_url(),
                peer_pubkey_hex: &rachel_pubkey_hex,
            },
            PeerSeam {
                peer_did: COUNTER_AUTHOR_TOBIAS,
                peer_endpoint: tobias_pds.endpoint_url(),
                peer_pubkey_hex: &tobias_pubkey_hex,
            },
        ],
    );
    assert_eq!(
        pulled.status, 0,
        "seed_landing_store_peer_claims_countered_each_arm: peer pull must succeed;\n\
         --- stdout ---\n{}\n--- stderr ---\n{}",
        pulled.stdout, pulled.stderr
    );

    // STEP 2 — the OPERATOR counters claim A via `claim counter` (the `claim_references` arm).
    counter_peer_claim_by_operator(env, &target_a);

    // PIN: exactly 2 countered peer claims (A via claim_references + B via
    // peer_claim_references — each contributes ONCE through its arm of the de-duped UNION).
    let countered = read_countered_peer_claims_count(env);
    assert_eq!(
        countered, 2,
        "seed_landing_store_peer_claims_countered_each_arm: two peer claims, one countered via \
         EACH ref-table arm, must sum to 2 (each contributes once); got {countered}"
    );

    let surveyed_cids = read_peer_claim_cids_for(env, surveyed_peer);
    let uncountered_cids = surveyed_cids
        .iter()
        .filter(|cid| *cid != &target_a && *cid != &target_b)
        .cloned()
        .collect::<Vec<_>>();
    let ordered_cids = read_peer_claim_cids_in_list_order(env);

    SeededPeerClaimsList {
        ordered_cids,
        countered_cids: vec![target_a, target_b],
        uncountered_cids,
        peer_did: surveyed_peer.to_string(),
    }
}

/// Seed BOTH the slice-19 peer shape (1 countered peer claim) AND the slice-18 own shape
/// (12 own claims, 3 countered) into the SAME store — the no-re-weight / own-untouched /
/// independent-degrade fixture (US-PC-001 Theme 5 + Theme 4 + the PC-INV-OwnUntouched gold).
/// After seeding: `count_countered_peer_claims()` == 1, `count_claims()` == 12,
/// `count_countered_own_claims()` == 3 — so the landing renders both the peer
/// "(1 countered)" AND "12 own claims (3 countered)", and the slice-19 PEER degrade (4th
/// fault-seam token) leaves the slice-18 own count intact. Pins all three with the direct
/// ADR-055/056 oracles. The peer-claims TOTAL is the genuine `count_peer_claims()` over the
/// combined store (the slice-18 own-counter peer rows + the slice-19 surveyed peer rows) —
/// exposed via [`landing_peer_total`], NOT hardcoded, since the slice-18 own seed itself
/// pulls peer-counter rows into `peer_claims`.
///
/// SCAFFOLD: true (slice-19) — a UNIFIED single-pull combined fixture (NOT a re-pull on top
/// of the slice-18 helper, which would 404 the slice-18 peers whose resolver vars a second
/// call cannot re-thread). It seeds the slice-18 OWN shape (12 own claims, 3 countered by
/// Rachel/Tobias — landing in `peer_claim_references` targeting OWN cids) PLUS a surveyed peer
/// (Priya) hosting 4 plain peer claims, ONE of which Tobias ALSO counters (a DISTINCT-peer
/// counter targeting a PEER cid — landing in `peer_claim_references` with `referenced_cid ==
/// <peer cid>`) — ALL in ONE `peer pull` over the three peers (each PDS held alive). Tobias's
/// PDS therefore carries his two own-claim counters (B, C) AND his Priya-peer counter. The
/// own `claims` table stays a clean 12 (the peer counter does NOT touch OWN claims), the own
/// countered count stays 3, and the countered-PEER count is exactly 1 (Priya's countered
/// claim). The own + peer counter reads are independent siblings (own-only by `claims` outer
/// table, peer-only by `peer_claims` outer table — ADR-056 D1 / WD-PC-7).
pub fn seed_landing_store_with_countered_peer_and_own(env: &TestEnv) -> HeldSubscriptions {
    // STEP 1 — the slice-18 OWN substrate: 3 NAMED own claims to be peer-countered (part of
    // the 12) + 9 plain own claims so `count_claims == 12`.
    let target_a = seed_own_claim_with_evidence(
        env, "github:slice19/aaa", "embodiesPhilosophy",
        "org.openlore.philosophy.dependency-pinning", 0.90, &[],
    );
    let target_b = seed_own_claim_with_evidence(
        env, "github:slice19/bbb", "embodiesPhilosophy",
        "org.openlore.philosophy.memory-safety", 0.90, &[],
    );
    let target_c = seed_own_claim_with_evidence(
        env, "github:slice19/ccc", "embodiesPhilosophy",
        "org.openlore.philosophy.reproducible-builds", 0.30, &[],
    );
    seed_own_claims_via_cli(env, LANDING_OWN_CLAIMS - COUNTERED_OWN_CLAIMS);

    // STEP 2 — Priya hosts 4 plain PEER claims; her FIRST claim's deterministic CID is the
    // PEER counter target. Rachel counters own A + C; Tobias counters own B + C AND Priya's
    // peer claim P (the `peer_claim_references` arm targeting a PEER cid → 1 countered peer
    // claim, WITHOUT touching OWN `claims`). Build EVERY record UP FRONT, hold every PDS alive,
    // pull ALL THREE peers in ONE invocation.
    let priya_seed = [13u8; 32];
    let (priya_records, priya_pubkey_hex) = build_verifiable_peer_records_for_triples(
        PEER_COUNT_SURVEYED_DID,
        priya_seed,
        &[
            ("github:peer/priya-axum", "org.openlore.philosophy.ergonomics", 0.40),
            ("github:peer/priya-tokio", "org.openlore.philosophy.async-runtime", 0.70),
            ("github:peer/priya-serde", "org.openlore.philosophy.zero-copy", 0.70),
            ("github:peer/priya-hyper", "org.openlore.philosophy.composability", 0.70),
        ],
    );
    let peer_target = priya_records
        .first()
        .expect("seed_landing_store_with_countered_peer_and_own: Priya's first peer claim")
        .rkey
        .clone();

    let rachel_seed = [7u8; 32];
    let tobias_seed = [9u8; 32];
    let (rachel_counter_a, rachel_pubkey_hex) =
        build_verifiable_peer_counter_record(COUNTERED_PEER_RACHEL_DID, rachel_seed, &target_a, Some(COUNTER_PEER_REASON_VERBATIM));
    let (rachel_counter_c, _r2) =
        build_verifiable_peer_counter_record(COUNTERED_PEER_RACHEL_DID, rachel_seed, &target_c, Some(COUNTER_PEER_REASON_VERBATIM));
    let (tobias_counter_b, tobias_pubkey_hex) =
        build_verifiable_peer_counter_record(COUNTERED_PEER_TOBIAS_DID, tobias_seed, &target_b, Some(COUNTER_PEER_REASON_VERBATIM));
    let (tobias_counter_c, _t2) =
        build_verifiable_peer_counter_record(COUNTERED_PEER_TOBIAS_DID, tobias_seed, &target_c, Some(COUNTER_PEER_REASON_VERBATIM));
    // Tobias ALSO counters Priya's PEER claim P (the peer-arm countered-peer-claim).
    let (tobias_counter_peer, _t3) =
        build_verifiable_peer_counter_record(COUNTERED_PEER_TOBIAS_DID, tobias_seed, &peer_target, Some(COUNTER_PEER_REASON_VERBATIM));

    let priya_pds = PeerPds::for_peer(PEER_COUNT_SURVEYED_DID, priya_records);
    let rachel_pds = PeerPds::for_peer(COUNTERED_PEER_RACHEL_DID, vec![rachel_counter_a, rachel_counter_c]);
    let tobias_pds = PeerPds::for_peer(
        COUNTERED_PEER_TOBIAS_DID,
        vec![tobias_counter_b, tobias_counter_c, tobias_counter_peer],
    );

    for (did, pds) in [
        (PEER_COUNT_SURVEYED_DID, &priya_pds),
        (COUNTERED_PEER_RACHEL_DID, &rachel_pds),
        (COUNTERED_PEER_TOBIAS_DID, &tobias_pds),
    ] {
        let added = run_openlore_with_peer_resolver(env, &["peer", "add", did], did, pds.endpoint_url());
        assert_eq!(
            added.status, 0,
            "seed_landing_store_with_countered_peer_and_own: peer add for {did} must succeed;\n\
             --- stdout ---\n{}\n--- stderr ---\n{}",
            added.stdout, added.stderr
        );
    }
    let pulled = run_openlore_pull_multi(
        env,
        &["peer", "pull"],
        &[
            PeerSeam { peer_did: PEER_COUNT_SURVEYED_DID, peer_endpoint: priya_pds.endpoint_url(), peer_pubkey_hex: &priya_pubkey_hex },
            PeerSeam { peer_did: COUNTERED_PEER_RACHEL_DID, peer_endpoint: rachel_pds.endpoint_url(), peer_pubkey_hex: &rachel_pubkey_hex },
            PeerSeam { peer_did: COUNTERED_PEER_TOBIAS_DID, peer_endpoint: tobias_pds.endpoint_url(), peer_pubkey_hex: &tobias_pubkey_hex },
        ],
    );
    assert_eq!(
        pulled.status, 0,
        "seed_landing_store_with_countered_peer_and_own: peer pull must succeed;\n\
         --- stdout ---\n{}\n--- stderr ---\n{}",
        pulled.stdout, pulled.stderr
    );

    // PIN the load-bearing aggregates: own claims 12 (untouched), own countered 3 (slice-18
    // sibling), peer countered exactly 1 (Priya's Tobias-countered claim). The peer-claims
    // TOTAL is the genuine combined count (read via `landing_peer_total`), NOT hardcoded.
    assert_user_author_claim_count(env, LANDING_OWN_CLAIMS);
    let countered_own = read_countered_own_claims_count(env);
    assert_eq!(
        countered_own, COUNTERED_OWN_CLAIMS,
        "seed_landing_store_with_countered_peer_and_own: the slice-18 countered-OWN count must \
         stay {COUNTERED_OWN_CLAIMS} (independent sibling, untouched by the peer shape); got \
         {countered_own}"
    );
    let countered_peer = read_countered_peer_claims_count(env);
    assert_eq!(
        countered_peer, COUNTERED_PEER_CLAIMS,
        "seed_landing_store_with_countered_peer_and_own: the countered-PEER count must be \
         {COUNTERED_PEER_CLAIMS} (Priya's one Tobias-countered peer claim); got {countered_peer}"
    );

    HeldSubscriptions {
        _peers: vec![priya_pds, rachel_pds, tobias_pds],
    }
}

/// The genuine `count_peer_claims()` TOTAL over the env's REAL store — the number the landing
/// renders beside "peer claims". Used by the combined peer+own scenarios where the total is
/// NOT a clean headline 4 (the slice-18 own seed itself pulls peer-counter rows into
/// `peer_claims`), so the no-re-weight assert reads the GENUINE total rather than a hardcoded
/// constant. Read-only; opens a SECOND short-lived connection.
pub fn landing_peer_total(env: &TestEnv) -> usize {
    let db_path = env.duckdb_path();
    let conn = duckdb::Connection::open(&db_path).unwrap_or_else(|err| {
        panic!("open DuckDB at {} for peer_claims total read: {err}", db_path.display())
    });
    conn.query_row("SELECT COUNT(*) FROM peer_claims", [], |row| row.get::<_, i64>(0))
        .unwrap_or_else(|err| panic!("query peer_claims total: {err}")) as usize
}

/// Pile on `count` MORE plain cached peer claims (a SECOND surveyed peer, none countered) so
/// the countered-peer count's invariance to store size is observable (US-PC-000 Theme 8 /
/// C-3 — no N+1). Each is a plain pulled peer claim via the production `peer add` + `peer
/// pull` path; NOTHING counters them, so the ADR-056 COUNT(DISTINCT) stays unchanged. Used by
/// the no-N+1 scenarios to inflate `peer_claims` WITHOUT changing the countered count.
///
/// SCAFFOLD: true (slice-19) — drives the EXISTING `peer add` + `peer pull` over a fresh
/// surveyed peer hosting `count` plain triples.
pub fn seed_extra_plain_peer_claims(env: &TestEnv, count: usize) {
    if count == 0 {
        return;
    }
    let bulk_peer = PEER_COUNT_BULK_DID;
    let bulk_seed = [23u8; 32];
    let triples: Vec<(String, String, f64)> = (0..count)
        .map(|i| {
            (
                format!("github:peer/bulk-{i:04}"),
                "org.openlore.philosophy.maintainability".to_string(),
                0.60,
            )
        })
        .collect();
    let triple_refs: Vec<(&str, &str, f64)> = triples
        .iter()
        .map(|(s, p, c)| (s.as_str(), p.as_str(), *c))
        .collect();
    let (records, pubkey_hex) =
        build_verifiable_peer_records_for_triples(bulk_peer, bulk_seed, &triple_refs);
    let pds = PeerPds::for_peer(bulk_peer, records);
    let added = run_openlore_with_peer_resolver(env, &["peer", "add", bulk_peer], bulk_peer, pds.endpoint_url());
    assert_eq!(
        added.status, 0,
        "seed_extra_plain_peer_claims: peer add for {bulk_peer} must succeed;\n\
         --- stdout ---\n{}\n--- stderr ---\n{}",
        added.stdout, added.stderr
    );
    let pulled = run_openlore_pull_multi(
        env,
        &["peer", "pull"],
        &[PeerSeam {
            peer_did: bulk_peer,
            peer_endpoint: pds.endpoint_url(),
            peer_pubkey_hex: &pubkey_hex,
        }],
    );
    assert_eq!(
        pulled.status, 0,
        "seed_extra_plain_peer_claims: peer pull must succeed;\n\
         --- stdout ---\n{}\n--- stderr ---\n{}",
        pulled.stdout, pulled.stderr
    );
    // Keep the bulk PDS alive for the rest of the test via a process-lifetime leak — the
    // viewer reads the LOCAL store, not the PDS, after the pull, so the endpoint need not
    // stay bound; dropping `pds` here is safe (the rows are persisted locally).
    drop(pds);
}

/// The surveyed peer DID the combined peer+own fixture uses to host the 4 cached peer claims
/// (distinct from the slice-18 own-counter peers Rachel/Tobias). One of its claims is countered
/// by Tobias (the peer arm) so the combined fixture has exactly 1 countered peer claim while the
/// own claims stay 12.
pub const PEER_COUNT_SURVEYED_DID: &str = "did:plc:priya-test";

/// The bulk-fill peer DID `seed_extra_plain_peer_claims` uses to host MANY plain (un-countered)
/// peer claims for the no-N+1 store-size-invariance proxy (distinct from the slice-18
/// own-counter peers Rachel/Tobias so the two shapes do not collide).
pub const PEER_COUNT_BULK_DID: &str = "did:plc:bulk-test";

/// Start the `openlore ui` viewer over the env's REAL store with the LOCAL
/// `count_countered_peer_claims()` read forced to FAIL mid-request — the slice-19
/// graceful-degrade seam (US-PC-000/001/002 Theme 4 / C-2 / C-5 CARDINAL / WD-PC-2/6 /
/// ADR-056 D4). The peer-claims + the slice-18 own counts STILL succeed; ONLY the
/// countered-PEER-count read fails (a 4th DISTINCT fault-seam token,
/// `OPENLORE_VIEWER_FAIL_COUNTERED_PEER_COUNT`, so the PEER count fails INDEPENDENTLY of the
/// own count), so BOTH the landing summary and the `/peer-claims` header render the missing
/// marker "(— countered)" while the peer-claims "4" + the slice-18 own "(3 countered)" + the
/// other counts + the nav hub + the `/peer-claims` rows + slice-13 flags render, the page
/// staying a normal 200 (never a 5xx / blank / raw stack trace). The failed read maps to
/// `None` (`.ok()`), DISTINCT from a fabricated `Some(0)` → "(0 countered)".
///
/// SEEDING-SEAM NOTE (documented DISTILL choice, mirroring the slice-18 degrade precedent):
/// the slice-06/15 viewer harness holds ONE long-lived DuckDB connection taken at STARTUP, so
/// the existing `make_store_unreadable` lock would refuse STARTUP rather than exercise a
/// MID-REQUEST per-count read failure. There is NO readily-available mid-request per-count
/// read-failure seam in the slice-06/15 harness. Per the DISTILL guidance, the OBSERVABLE
/// missing-marker contract (a failed countered-peer-count read → "(— countered)" while the
/// sibling counts + rows render, page 200) is scaffolded against a TEST-ONLY effect-shell
/// fault seam (the `OPENLORE_VIEWER_FAIL_COUNTERED_PEER_COUNT` env var, threaded by
/// `start_inner` — a 4th DISTINCT token from the slice-18 `OPENLORE_VIEWER_FAIL_COUNTERED_
/// COUNT` so the PEER count degrades independently); the SUCCESSFUL-zero distinction is fully
/// exercisable today via `seed_landing_store_no_peer_claim_countered`. DELIVER materializes
/// the per-count fault seam (a `#[cfg(debug_assertions)]`-gated, release-forbidden,
/// xtask-guarded effect-shell branch substituting `Err(StoreReadError)` for the REAL
/// `count_countered_peer_claims()` read on BOTH the `/` and `/peer-claims` handlers, appended
/// to the xtask `VIEWER_FAIL_SEAM_TOKENS` guard) with the SAME observable target — exactly as
/// slice-18 materialized `OPENLORE_VIEWER_FAIL_COUNTERED_COUNT`. Until then the scenario panics
/// at the `todo!()` `start_inner` body (slice-06) → RED MISSING_FUNCTIONALITY, never BROKEN.
///
/// SCAFFOLD: true (slice-19).
pub fn start_viewer_with_failing_countered_peer_count(env: &TestEnv) -> ViewerServer {
    ViewerServer::start_inner(env, None, None, None, false, false, false, true)
}

/// Assert the LANDING render shows the countered-PEER count "(`n` countered)" beside the
/// peer-claims line (US-PC-001 — the headline "4 peer claims (1 countered)"). Universe
/// (Mandate 8 — port-exposed rendered surface): the rendered body contains the exact
/// parenthetical `"({n} countered)"` (the REUSED `render_countered(Some(n))` output, ADR-056
/// D3). Scans the OBSERVABLE HTML the operator's browser shows; never an internal
/// `LandingSummary` field. The caller separately asserts the peer-claims "4" still renders
/// (the count is additive, never a re-weight — C-4) + the slice-18 own line is untouched.
///
/// NOTE: the slice-18 own line ALSO renders a `"(N countered)"` parenthetical on the landing
/// (the own count). The headline seeds keep the peer count (1) DISTINCT from the own count
/// (3) so a `"(1 countered)"` scan unambiguously targets the peer line; the
/// `assert_landing_own_line_untouched` companion pins the own "(3 countered)" still renders.
///
/// SCAFFOLD: true (slice-19).
pub fn assert_landing_peer_countered_count(body: &str, n: usize) {
    // The landing renders BOTH the slice-18 own "(N countered)" AND the slice-19 peer
    // "(N countered)". To target the PEER count unambiguously (not the own line's
    // parenthetical), scan the count at the PEER-LINE position: the rendered shape is
    // "<peer total> peer claims (<n> countered)" (the slice-18 own line is the SEPARATE
    // "<own total> own claims (<own countered> countered)"). We assert the "peer claims (N
    // countered)" adjacency — so a bare "(0 countered)" from the own line cannot satisfy a
    // peer assert vacuously (No Fixture Theater). The exact whitespace/markup between
    // "peer claims" and the parenthetical is DELIVER's render decision; we tolerate a single
    // space (the headline copy) and fall back to a same-line scan that still requires the
    // "peer claims" label to PRECEDE the parenthetical.
    assert_countered_count_beside_label(body, "peer claims", n, "US-PC-001 — \"4 peer claims (1 countered)\"");
}

/// Assert the rendered landing body shows "(`n` countered)" at the count position of the
/// surface labelled `label` ("peer claims" or "own claims") — i.e. the FIRST
/// "(<token> countered)" parenthetical that FOLLOWS the `label` text precedes any later
/// surface label. This disambiguates the TWO countered parentheticals the landing renders
/// (slice-18 own + slice-19 peer) so a peer assert cannot be satisfied vacuously by the own
/// line's parenthetical, and vice-versa (No Fixture Theater). Universe (Mandate 8 —
/// port-exposed rendered surface): the substring window from `label` to the next surface
/// boundary contains exactly "(`n` countered)". Scans the OBSERVABLE HTML; never an internal
/// `LandingSummary` field.
fn assert_countered_count_beside_label(body: &str, label: &str, n: usize, headline: &str) {
    let label_pos = body.find(label).unwrap_or_else(|| {
        panic!(
            "the landing summary must label the {label:?} count so the {label:?} countered \
             count can be read at its position ({headline}); body was:\n{body}"
        )
    });
    // The window from the label to the END of body (the parenthetical the render appends
    // immediately after the count for THIS surface is the FIRST "(… countered)" after the
    // label). Extract the first countered parenthetical in that window.
    let window = &body[label_pos..];
    let found = extract_countered_parenthetical(window).unwrap_or_else(|| {
        panic!(
            "the landing summary must show a countered count beside the {label:?} count \
             ({headline}); no \"(… countered)\" parenthetical followed {label:?}; body \
             was:\n{body}"
        )
    });
    assert_eq!(
        found,
        n.to_string(),
        "the {label:?} countered count must be \"({n} countered)\" at the {label:?} position \
         ({headline}); found \"({found} countered)\" instead; body was:\n{body}"
    );
}

/// Assert the LANDING render shows the MISSING-marker "(— countered)" for the countered-PEER
/// count (a FAILED read, ADR-056 D4 / C-5), DISTINCT from a successful "(0 countered)".
/// Universe (port-exposed rendered surface): the rendered body contains the exact
/// `"(— countered)"` parenthetical (the `render_countered(None)` output). We scan the
/// COUNTERED-COUNT POSITION, NOT the bare marker (the chrome title's em-dash would collide).
/// Used by the failed-read degrade scenario (Theme 4). The caller separately asserts the
/// peer-claims count + the slice-18 own line still render + the page is 200 (the degrade is
/// per-count, independent — the PEER count fails via the 4th distinct fault-seam token).
///
/// SCAFFOLD: true (slice-19).
pub fn assert_landing_peer_countered_missing(body: &str) {
    // Scan the count at the PEER-LINE position (the slice-18 own line legitimately renders its
    // own "(N countered)" — a failed PEER read must NOT be confused with the own count). The
    // peer-line parenthetical must be the missing marker "(— countered)".
    let label = "peer claims";
    let label_pos = body.find(label).unwrap_or_else(|| {
        panic!(
            "the landing summary must still label the {label:?} count even when its read \
             FAILED (the surface is present, only the number is missing); body was:\n{body}"
        )
    });
    let window = &body[label_pos..];
    let found = extract_countered_parenthetical(window).unwrap_or_else(|| {
        panic!(
            "a FAILED countered-peer-count read must render the missing-marker beside the \
             {label:?} count; no \"(… countered)\" parenthetical followed {label:?}; body \
             was:\n{body}"
        )
    });
    assert_eq!(
        found, COUNTERED_MISSING_MARKER,
        "a FAILED countered-peer-count read must render the missing-marker \
         \"({COUNTERED_MISSING_MARKER} countered)\" at the {label:?} position — DISTINCT from a \
         fabricated \"(0 countered)\" (ADR-056 D4 / C-5); found \"({found} countered)\" instead; \
         body was:\n{body}"
    );
}

/// Assert the `/peer-claims` HEADER render shows the countered-PEER count "(`n` countered)" —
/// the SAME helper output the landing renders (US-PC-002, single source — ADR-056 D3).
/// Universe (port-exposed rendered surface): the rendered `/peer-claims` body contains the
/// exact `"({n} countered)"` parenthetical in the header region (near the "Peer Claims"
/// heading + read-only notice). The caller separately asserts the list rows / order / paging /
/// origin + slice-13 flags are byte-identical to the no-header-count baseline (the header
/// count is additive, never a re-order/filter/re-weight — C-4 / WD-PC-9).
///
/// SCAFFOLD: true (slice-19).
pub fn assert_peer_claims_header_countered_count(body: &str, n: usize) {
    let needle = format!("({n} countered)");
    assert!(
        body.contains(&needle),
        "the `/peer-claims` list header must show the countered-peer count {needle:?} (the SAME \
         single-source count the landing shows, US-PC-002 / ADR-056 D3); body was:\n{body}"
    );
}

/// Assert the `/peer-claims` HEADER render shows the MISSING-marker "(— countered)" for the
/// countered-PEER count (a FAILED read, ADR-056 D4 / C-5) while the list rows still render.
/// Universe (port-exposed rendered surface): the rendered `/peer-claims` body contains the
/// exact `"(— countered)"` parenthetical, NOT a fabricated "(0 countered)". Used by the
/// `/peer-claims` failed-read degrade scenario (Theme 4). The caller separately asserts the
/// list rows + slice-13 per-row flags still render + the page is 200.
///
/// SCAFFOLD: true (slice-19).
pub fn assert_peer_claims_header_countered_missing(body: &str) {
    let needle = format!("({COUNTERED_MISSING_MARKER} countered)");
    assert!(
        body.contains(&needle),
        "a FAILED countered-peer-count read on `/peer-claims` must render the missing-marker at \
         the header count position {needle:?} — DISTINCT from a fabricated \"(0 countered)\" \
         (ADR-056 D4 / C-5); body was:\n{body}"
    );
    assert!(
        !body.contains("(0 countered)"),
        "a FAILED countered-peer-count read on `/peer-claims` must NOT fabricate \
         \"(0 countered)\" (C-5); body was:\n{body}"
    );
}

/// Assert the LANDING "(N countered)" peer count EQUALS the `/peer-claims` header
/// "(N countered)" count for the SAME store (US-PC-002 single-source consistency — WD-PC-8 /
/// R-PC-6). The REUSED `render_countered` helper renders the SAME number on both surfaces;
/// this pins the equality on the OBSERVABLE rendered surfaces (Mandate 8). Universe
/// (port-exposed): the `"(N countered)"` parenthetical extracted from the `/peer-claims` body
/// (which carries ONLY the peer count — no own count on that route) matches the SAME
/// parenthetical on the landing PEER line. A divergence is an UNSHIPPABLE single-source
/// breach (the two orientation surfaces must agree).
///
/// IMPLEMENTATION NOTE: the `/peer-claims` route renders ONLY the peer countered count, so
/// `extract_countered_parenthetical(peer_body)` yields the peer number directly. The landing
/// renders BOTH the slice-18 own "(N countered)" AND the slice-19 peer "(N countered)"; rather
/// than positionally disambiguate, this assert pins that the `/peer-claims` peer number is
/// PRESENT on the landing — the strong single-source equality — leaving the slice-18 own line
/// to `assert_landing_own_line_untouched`. The headline seeds keep peer (1) ≠ own (3) so the
/// presence check is unambiguous.
///
/// SCAFFOLD: true (slice-19).
pub fn assert_landing_and_peer_claims_countered_consistent(landing_body: &str, peer_claims_body: &str) {
    let peer_count = extract_countered_parenthetical(peer_claims_body).unwrap_or_else(|| {
        panic!(
            "assert_landing_and_peer_claims_countered_consistent: the `/peer-claims` body must \
             carry a \"(N countered)\" parenthetical to compare; body was:\n{peer_claims_body}"
        )
    });
    let needle = format!("({peer_count} countered)");
    assert!(
        landing_body.contains(&needle),
        "the `/peer-claims` header \"({peer_count} countered)\" must ALSO appear on the landing \
         PEER line for the same store (single source — WD-PC-8 / R-PC-6); the two orientation \
         surfaces diverged. landing body was:\n{landing_body}"
    );
}

/// Assert the slice-18 OWN line "12 own claims (3 countered)" renders UNTOUCHED on the landing
/// — the slice-19 peer count must NOT re-touch / re-weight / blank the slice-18 own surface
/// (BR-PC-4 / WD-PC-7 — own+peer completion, peer-only, no third dimension). Universe
/// (port-exposed rendered surface): the rendered landing body contains the own-claims label +
/// count "12" AND the slice-18 own "(3 countered)" parenthetical. Used by the no-re-weight
/// scenario, the independent-degrade scenario (the own count survives the peer count's
/// failure), and the PC-INV-OwnUntouched gold. Reuses the slice-17/18 own count consts.
///
/// SCAFFOLD: true (slice-19).
pub fn assert_landing_own_line_untouched(body: &str) {
    // The slice-18 own line still labels + counts the OWN claims ("12 own claims").
    assert_landing_shows_count(body, "own claims", LANDING_OWN_CLAIMS);
    // AND the slice-18 own "(3 countered)" parenthetical still renders at the OWN-line position
    // (untouched by the peer count — independent sibling). Position-aware so the peer line's
    // parenthetical cannot satisfy it vacuously.
    assert_countered_count_beside_label(
        body,
        "own claims",
        COUNTERED_OWN_CLAIMS,
        "BR-PC-4 / WD-PC-7 — \"12 own claims (3 countered)\" untouched",
    );
}
