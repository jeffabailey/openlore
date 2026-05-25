//! Shared acceptance-test support.
//!
//! All helpers in this module are RED-ready scaffolds; bodies panic via
//! `todo!()` / `unimplemented!()`. DELIVER (functional-software-crafter)
//! fills them in alongside production code per the standard outside-in
//! TDD cycle.
//!
//! See `docs/feature/openlore-foundation/distill/acceptance-tests.md`
//! and `docs/feature/openlore-foundation/distill/wave-decisions.md` for
//! the design context.
//!
//! Functional-paradigm note (ADR-007): helpers are free functions over
//! immutable values; no test-class hierarchy. Setup returns a `TestEnv`
//! VALUE; assertions are stand-alone functions; doubles are plain
//! `pub struct` records that the helpers thread through. Composition,
//! not inheritance.
//
// SCAFFOLD: true
//
// External dependencies the production code is EXPECTED to expose
// (DELIVER will scaffold these crates per
// docs/feature/openlore-foundation/design/component-boundaries.md):
//
//   * the `openlore` binary at `crates/cli/src/main.rs`, discoverable via
//     `assert_cmd::Command::cargo_bin("openlore")`
//   * a `test-support` crate (or `tests/common/` module) — TBD by DELIVER
//
// At DISTILL handoff time `cargo test` will FAIL TO COMPILE because none
// of these exist yet. This is the intended RED-baseline state; DELIVER
// closes the gap by scaffolding Cargo.toml + the 8 crates from
// component-boundaries.md, after which every `#[test]` panics at
// `todo!()` → tests classify as RED, not BROKEN.

#![allow(dead_code)] // scaffolds; usage lands in DELIVER

use std::path::PathBuf;

/// A sealed test environment.
///
/// Holds an isolated `HOME` so XDG paths (`~/.config/openlore`,
/// `~/.local/share/openlore`) resolve under a temporary directory that
/// auto-cleans on drop.
///
/// One `TestEnv` per scenario. Multiple `TestEnv`s within one test
/// process do NOT share state (parallel-safe).
pub struct TestEnv {
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
        todo!("DELIVER: create a tempdir, return a TestEnv pointing at it")
    }

    /// Convenience: a TestEnv that has already had `openlore init` run
    /// successfully. Most claim scenarios start here.
    pub fn initialized() -> Self {
        todo!("DELIVER: build fresh() then invoke `openlore init` with the test identity")
    }

    /// Path to the local claims directory: `{home}/.local/share/openlore/claims/`.
    pub fn claims_dir(&self) -> PathBuf {
        todo!("DELIVER: join home and the XDG-relative claims path")
    }

    /// Path to the local DuckDB file: `{home}/.local/share/openlore/openlore.duckdb`.
    pub fn duckdb_path(&self) -> PathBuf {
        todo!("DELIVER: join home and the XDG-relative DuckDB path")
    }

    /// Path to the identity config: `{home}/.config/openlore/identity.toml`.
    pub fn identity_toml_path(&self) -> PathBuf {
        todo!("DELIVER: join home and the XDG-relative identity path")
    }
}

/// A test double for `adapter-atproto-pds`.
///
/// Records `create_record` calls in memory; replays `get_record` and
/// `list_records` deterministically. The OpenLore binary is configured
/// (via an env var or config flag DELIVER picks) to point at this
/// in-process double instead of a real PDS.
///
/// Implementation technique (in-process HTTP stub via `wiremock` OR
/// recorded XRPC fixture replay) is DELIVER's call per DD-6.
pub struct FakePds {
    /// In-memory record store keyed by `(collection, rkey)`.
    /// Slice-01 only writes `org.openlore.claim`.
    records: Vec<FakePdsRecord>,
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
    /// Start the fake PDS. If the implementation is an HTTP stub, this
    /// binds to a free localhost port; the binary reaches it via the
    /// `OPENLORE_PDS_ENDPOINT` env var (or similar — DELIVER's call).
    pub fn start() -> Self {
        todo!("DELIVER: stand up wiremock or initialize the recorded-fixture replay engine")
    }

    /// All records the fake has accepted so far.
    pub fn records(&self) -> &[FakePdsRecord] {
        todo!("DELIVER: return the in-memory record vec")
    }

    /// Find one record by its at-uri.
    pub fn record_at(&self, at_uri: &str) -> Option<&FakePdsRecord> {
        todo!("DELIVER: linear-scan the records vec")
    }

    /// Inject an "unreachable" failure mode: subsequent `create_record`
    /// calls return a network-error shape that the production adapter
    /// classifies as `PdsError::Unreachable`. Used by WS-10.
    pub fn simulate_unreachable(&mut self) {
        todo!("DELIVER: flip an internal flag the stub honors")
    }

    /// Restore normal operation after `simulate_unreachable`.
    pub fn restore(&mut self) {
        todo!("DELIVER: flip the flag back")
    }
}

/// A test double for `adapter-atproto-did`.
///
/// Holds a known test DID (`did:plc:test-jeff`) and a deterministic
/// Ed25519 keypair. The OpenLore binary uses this in place of the real
/// keychain-backed identity adapter.
pub struct FakeIdentity {
    pub did: String, // e.g. "did:plc:test-jeff#org.openlore.application"
}

impl FakeIdentity {
    /// Construct the canonical fake identity used across slice-01 tests.
    pub fn jeff() -> Self {
        todo!("DELIVER: hardcode did:plc:test-jeff + deterministic Ed25519 key")
    }

    /// A second known identity used by anxiety-scenario tests that
    /// involve Maria (US-002 Example 3, US-003 Example 2, WS-10).
    pub fn maria() -> Self {
        todo!("DELIVER: hardcode did:plc:test-maria + a different deterministic key")
    }

    /// The raw author DID (without the key fragment).
    pub fn author_did(&self) -> &str {
        todo!("DELIVER: strip the #fragment from self.did")
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

/// Run `openlore <args>` with `HOME` set to `env.home`, the PDS endpoint
/// pointed at `env.pds`, and the identity pointed at `env.identity`.
///
/// Provides no stdin; for scenarios that need to send `<Enter>` / `<Y>`
/// at the chained prompts use `run_openlore_with_stdin`.
pub fn run_openlore(env: &TestEnv, args: &[&str]) -> CliOutcome {
    todo!("DELIVER: spawn the binary via assert_cmd, set env vars per env.home/pds/identity, capture output")
}

/// Run `openlore <args>` feeding `stdin_lines` (newline-joined) on
/// stdin. Used for the two-prompt chained flow: pass "\n" to confirm
/// the sign prompt, "Y\n" to confirm publish.
pub fn run_openlore_with_stdin(
    env: &TestEnv,
    args: &[&str],
    stdin_lines: &str,
) -> CliOutcome {
    todo!("DELIVER: as run_openlore, but pipe stdin_lines into the child process")
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
//
// Per Mandate 8 (universe-bound state-delta) these helpers are the Rust
// idiomatic mirror of `assert_state_delta(before, after, universe,
// expected)`. They wrap one observable port-exposed name each. DELIVER
// will (per DD-3) migrate the cross-cutting ones to the formal
// `tests/common/state_delta.rs` API once the port is bootstrapped.

/// Assert that the CLI invocation exited with status 0 and the given
/// substring appears in stdout. Failure prints the full stdout/stderr
/// for debuggability.
pub fn assert_exit_zero_and_stdout_contains(outcome: &CliOutcome, expected_substring: &str) {
    todo!("DELIVER: assert outcome.status == 0 and outcome.stdout.contains(expected_substring); on failure print both streams")
}

/// Assert non-zero exit AND the stderr contains the given substring.
pub fn assert_exit_nonzero_and_stderr_contains(outcome: &CliOutcome, expected_substring: &str) {
    todo!("DELIVER: assert outcome.status != 0 and outcome.stderr.contains(expected_substring)")
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
