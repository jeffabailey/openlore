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
fn seed_network_index_from_specs(
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
        Self::start_inner(env, None, None, None)
    }

    /// Start a `openlore ui --port 0` viewer over the env's REAL store AND wire the
    /// `/scrape` route at the supplied `FakeGithub` double (via the
    /// `OPENLORE_GITHUB_API_BASE` seam, exactly as `run_openlore_scrape` does).
    /// Used by the live-scrape scenarios (US-VIEW-005). The double is kept alive
    /// for the viewer's lifetime.
    ///
    /// SCAFFOLD: true (slice-06).
    pub fn start_with_github(env: &TestEnv, github: GithubServer) -> Self {
        Self::start_inner(env, Some(github), None, None)
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
        Self::start_inner(env, None, Some(url), Some(indexer))
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
        Self::start_inner(env, None, Some(closed.indexer_url().to_string()), None)
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
    // DELIVER: derive a deterministic Ed25519 seed from `contributor_did` (as the
    // slice-08 seeders do), build several DISTINCT-subject triples on the shared
    // reproducible-builds object at varied confidences, and drive
    // `seed_peer_authored_graph(env, &[SeedPeer{ peer_did: contributor_did, seed,
    // triples }])` so the contributor's multi-project trail lands in the REAL
    // `peer_claims` table (claim_count >= 4, cross_project_span >= 2 — a non-sparse
    // pairing whose breakdown decomposes). LOCAL only — no network.
    let _ = (env, contributor_did, SCORE_OBJECT_REPRODUCIBLE_BUILDS);
    todo!(
        "slice-09 DELIVER: seed a RICH local trail for {contributor_did} (several \
         distinct subjects on the shared reproducible-builds object, varied \
         confidences) via the production `peer add` + `peer pull` path so the \
         contributor's local feed scores to a real weight + multi-row breakdown"
    )
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
    // DELIVER: one `SeedPeer{ peer_did: contributor_did, seed, triples: &[(one
    // subject, reproducible-builds, 0.95)] }` via `seed_peer_authored_graph` so
    // exactly ONE `peer_claims` row lands — a single-claim/single-author/no-span
    // feed the pure core buckets `[SPARSE]` at any confidence. LOCAL only.
    let _ = (env, contributor_did, SCORE_OBJECT_REPRODUCIBLE_BUILDS);
    todo!(
        "slice-09 DELIVER: seed a SPARSE local trail for {contributor_did} (exactly \
         one claim, one author, one subject, confidence 0.95) via the production \
         `peer add` + `peer pull` path so the pure core buckets it [SPARSE] \
         regardless of the high confidence"
    )
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
    // DELIVER: reuse the `seed_own_plus_peer_graph` identical-content shape — own
    // claim (You) + one pulled peer on the SAME (subject, reproducible-builds) at
    // distinct confidences (e.g. 0.40 own + 0.55 peer) so the contributor-scope
    // feed yields one pairing decomposing into TWO attributed Contribution rows.
    let _ = (env, SCORE_OBJECT_REPRODUCIBLE_BUILDS);
    todo!(
        "slice-09 DELIVER: seed two distinct authors asserting the same \
         (subject, reproducible-builds) at different confidences so the pairing \
         decomposes into two attributed rows (anti-merging; I-CS-2/I-CS-10)"
    )
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
    // DELIVER: extract the displayed pairing weight + each rendered Contribution
    // subtotal from the breakdown table markup, then assert their running sum
    // equals the displayed weight (reproduce-by-hand). The assertion reads ONLY the
    // rendered surface (Mandate 8 universe = port-exposed rendered HTML), never the
    // in-process `WeightedPairing` — the whole point is that the HTML itself is
    // self-consistent.
    let _ = body;
    todo!(
        "slice-09 DELIVER: parse the rendered weight + the per-row subtotals out of \
         the breakdown table and assert Σ subtotal == displayed weight (the J-002c \
         reproduce-by-hand gate; KPI-GRAPH-3)"
    )
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
