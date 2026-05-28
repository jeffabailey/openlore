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
fn hex_lower(bytes: &[u8]) -> String {
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
        // SCAFFOLD: true (slice-04)
        let _ = (env, claim);
        todo!(
            "DELIVER (slice-04): subscribe + pull the additional contributing peer claim via the \
             real peer add + peer pull verbs so the local store gains a row and the weight \
             recomputes (GQE-14 query-time-compute proof)"
        )
    }
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
    // SCAFFOLD: true (slice-04)
    let _ = env;
    todo!(
        "DELIVER (slice-04): scan every DuckDB table + every on-disk claim artifact for a persisted \
         adherence_weight / weight_bucket (STRONG|MODERATE|SPARSE) substring and assert NONE exists \
         — weights are display-only, computed at query time (Gate 4; WD-72/WD-89)"
    )
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
