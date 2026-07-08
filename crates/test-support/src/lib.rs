//! `openlore-test-support` — shared test doubles + fixture builders.
//!
//! Per DISTILL DD-6 + acceptance-tests.md §6: the test doubles
//! (`FakePds`, `FakeIdentity`) live in a shared crate so adapter-level
//! integration tests and acceptance tests can both depend on the same
//! canonical implementations. Acceptance tests (in
//! `tests/acceptance/support/mod.rs`) currently declare local copies
//! while the scaffold matures; DELIVER will migrate them to import from
//! here.
//!
//! Functional-paradigm note (ADR-007): doubles are records with
//! pure-shaped methods, threaded through tests by value. No global
//! state. No test-class hierarchy.
//!
//! RED-baseline scaffold (step 01-01): the types and method shapes
//! exist; bodies panic.
//
// SCAFFOLD: true

#![allow(dead_code)]
#![forbid(unsafe_code)]

// Step 02-06: canonical claim fixtures used by LC-1 and downstream
// acceptance scenarios. Re-exported flat so tests can write
// `openlore_test_support::fixture_jeff_rust_memory_safety()` directly.
pub mod fixtures;
pub use fixtures::*;

// Step 04-03: deterministic IdentityPort test double. Lives in its own
// module so the Ed25519 dependency and seeded-keypair logic stay
// scoped; re-exported flat so tests can write
// `openlore_test_support::FakeIdentity::jeff()` directly.
pub mod identity;
pub use identity::FakeIdentity;

// Step 04-06: deterministic PdsPort test double. Lives in its own module
// so the Arc/Mutex shared-state plumbing stays scoped; re-exported flat
// so tests can write `openlore_test_support::FakePds::new()` directly.
// Replaces the previous inline panic-scaffold (RED-baseline step 01-01)
// with the real implementation per DD-6.
pub mod fake_pds;
pub use fake_pds::{FakePds, FakePdsHttpHandle, FakePdsRecord};

// Slice-03 step 06-01 (DD-FED-2 + DD-FED-3): read-only test double for a
// PEER's PDS. Distinct from `FakePds` because the peer PDS is HONESTLY a
// different actor — slice-03 pulls from peer PDSes UNAUTHENTICATED and
// NEVER writes to them. Adversarial constructors (tampered signature,
// CID mismatch, self-attribution per WD-40, cross-attribution per WD-41)
// drive the KPI-FED-6 + WD-30 anti-merging acceptance gates.
pub mod fake_peer_pds;
pub use fake_peer_pds::{FakePeerPds, FakePeerPdsHttpHandle, FakePeerRecord};

// Slice-03 step 06-02: canonical peer-claim fixtures. Symmetric with
// `fixtures.rs`; one free function per well-known fixture used across
// US-FED-002..005 acceptance scenarios.
pub mod fixtures_peer;
pub use fixtures_peer::{
    fixture_adversarial_peer_cid_mismatch, fixture_adversarial_peer_cross_attribution,
    fixture_adversarial_peer_self_attribution, fixture_adversarial_peer_tampered_signature,
    fixture_other_developer_three_claims,
};

// Slice-02 step 07-01 (DD-SCR-2 + DD-SCR-3): test double for the PUBLIC
// GitHub API backing the new `GithubPort`. SEPARATE module from `FakePds` /
// `FakePeerPds` because GitHub is a wholly different external system
// (WD-61 / ADR-019) — no shared method shape, auth model, or failure
// surface. Postures (public repo/user, not-found, private, offline,
// rate-limited, token-rejected, authenticated, no-matching-signals,
// multi-signal-one-predicate) are constructor-time-pinned (DD-SCR-3) and
// drive the `scraper_only_reads_public_data` + `candidate_*` +
// `scraper_never_persists_unsigned` acceptance gates. Public-data-only +
// human-gate are STRUCTURAL: the double has no private surface and holds no
// storage/identity/pds reference.
pub mod fake_github;
pub use fake_github::{
    FakeAuthMode, FakeGithub, FakeGithubErrorPosture, FakeGithubHttpHandle, FakeTargetKind,
    FIXTURE_REJECTED_PAT, FIXTURE_REPO_TARGET, FIXTURE_USER_TARGET, FIXTURE_VALID_PAT,
};

// Slice-04 step 07-01 (DD-GRAPH): canonical scoring + traversal fixtures.
// Symmetric with `fixtures_peer.rs` / `fixtures_github.rs`; one free function
// per well-known GRAPH SHAPE used across US-GRAPH-001..005 acceptance
// scenarios. Each fixture describes which authors assert which philosophy on
// which projects at which confidence; the
// `tests/acceptance/support/mod.rs::seed_federated_graph` orchestrator turns a
// fixture into REAL DuckDB rows via the slice-03 `claim add` / `peer add` /
// `peer pull` seam (NO new external fake — scoring/traversal is local
// read-only analysis over the real store). The worked-arithmetic targets back
// the Gate 2 (weight_equals_formula) + Gate 3 (sparse_renders_sparse) +
// SCORE-1 (cross-project-triangulation-counts-as-breadth) acceptance assertions.
pub mod fixtures_scoring;
pub use fixtures_scoring::{
    scoring_fixture_conflicting_confidences_one_project, scoring_fixture_cyclic_two_claim_graph,
    scoring_fixture_dense_fan_out_beyond_depth_two,
    scoring_fixture_dependency_pinning_worked_example, scoring_fixture_rachel_spans_two_projects,
    scoring_fixture_reproducible_builds_multi_author, scoring_fixture_single_sparse_claim,
    ScoringClaimSpec, ScoringRelationship, EXPECTED_AUTHOR_DISTINCT_BONUS,
    EXPECTED_CROSS_PROJECT_TRIANGULATION_BONUS,
};

// Slice-05 step 01-05 (DD-AV-13): the network-ingest fixtures + the validating
// ingest-source fake. `fixtures_ingest` materializes the adversarial + valid
// `RawRecordSpec` builders, the four named ingest fixtures, the real-`z6Mk` PLC
// DID-document fixture (a known test keypair so the ADR-026 decode runs the REAL
// path; AV-4), and the network-search corpora. `fake_ingest_source` hosts those
// records over `ports::IngestSourcePort` and VALIDATES inputs like the real
// adapter (DD-AV-2) — a permissive fake that "verified" anything would hide the
// AV-3 reject-gate wiring (the cardinal verify-before-index gate). Re-exported
// flat so the slice-05 acceptance + harness code can name them directly.
pub mod fixtures_ingest;
pub use fixtures_ingest::{
    corpus_bazel_five_distinct_authors, corpus_deno_dependency_pinning_two_authors,
    corpus_priya_eight_claims_six_subjects, corpus_reproducible_builds_nine_authors, did_doc_for,
    fixture_ingest_adversarial_set_plus_one_valid, fixture_ingest_cid_mismatch,
    fixture_ingest_tampered_signature, fixture_ingest_unsigned, fixture_ingest_valid_signed,
    fixture_real_z6mk_did_doc, DidDocFixture, FixtureKeypair, Posture, RawRecordSpec, PRIYA_DID,
    RACHEL_DID, SVEN_DID,
};

pub mod fake_ingest_source;
pub use fake_ingest_source::FakeIngestSource;

use claim_domain::{Cid, ClaimLookup, Did, SignedClaim};
use ports::{
    AttributedClaim, ClockPort, GraphNode, ProbeOutcome, ScoringFilter, StorageError, StoragePort,
    TraversalBound, TraversalResult,
};

// -----------------------------------------------------------------------------
// FakeClaimLookup — in-memory `ClaimLookup` double for pure-core tests
// -----------------------------------------------------------------------------
//
// Used by acceptance scenarios that exercise the cycle-detection arm of
// `reference_rules_validate` (LC-7 in slice-01). Implements the
// `claim_domain::ClaimLookup` trait synchronously over an in-memory map
// keyed by the signed claim's body CID. No I/O, no async — keeps the
// pure core's invariants intact.

/// In-memory `ClaimLookup` double: maps body CID → SignedClaim.
///
/// Tests insert claims via `insert(cid, signed)`; the lookup returns
/// `Some(signed)` when a query CID matches an inserted key, `None`
/// otherwise. The implementation is purely functional from the
/// trait's perspective (no mutation occurs once the lookup is built).
#[derive(Debug, Default, Clone)]
pub struct FakeClaimLookup {
    by_cid: std::collections::HashMap<Cid, SignedClaim>,
}

impl FakeClaimLookup {
    /// Create an empty lookup. Use `insert` to populate.
    pub fn new() -> Self {
        Self {
            by_cid: std::collections::HashMap::new(),
        }
    }

    /// Insert a signed claim under the supplied CID. The CID is the
    /// key tests query against — typically the body CID of the claim
    /// the author would resolve through the lookup.
    pub fn insert(&mut self, cid: Cid, signed: SignedClaim) {
        self.by_cid.insert(cid, signed);
    }
}

impl ClaimLookup for FakeClaimLookup {
    fn signed_by_cid(&self, cid: &Cid) -> Option<SignedClaim> {
        self.by_cid.get(cid).cloned()
    }
}

// -----------------------------------------------------------------------------
// FakePds — implementation lives in `src/fake_pds.rs` (step 04-06). It
// is re-exported flat above as `FakePds` + `FakePdsRecord`.
// -----------------------------------------------------------------------------

// -----------------------------------------------------------------------------
// FakeIdentity — deterministic IdentityPort double
// -----------------------------------------------------------------------------
//
// Implementation lives in `src/identity.rs` (step 04-03). It is
// re-exported flat above as `FakeIdentity`.

// -----------------------------------------------------------------------------
// FrozenClock — deterministic ClockPort double for tests
// -----------------------------------------------------------------------------

pub struct FrozenClock {
    at: chrono::DateTime<chrono::Utc>,
}

impl FrozenClock {
    pub fn at_rfc3339(_rfc3339: &str) -> Self {
        panic!("Not yet implemented -- RED scaffold");
    }
}

impl ClockPort for FrozenClock {
    fn probe(&self) -> ProbeOutcome {
        panic!("Not yet implemented -- RED scaffold");
    }

    fn now_utc(&self) -> chrono::DateTime<chrono::Utc> {
        panic!("Not yet implemented -- RED scaffold");
    }
}

// -----------------------------------------------------------------------------
// InMemoryStorage — StoragePort double for layer-2 acceptance tests
// -----------------------------------------------------------------------------

pub struct InMemoryStorage {
    _scaffold: (),
}

impl InMemoryStorage {
    pub fn new() -> Self {
        Self { _scaffold: () }
    }
}

impl Default for InMemoryStorage {
    fn default() -> Self {
        Self::new()
    }
}

impl StoragePort for InMemoryStorage {
    fn probe(&self) -> ProbeOutcome {
        panic!("Not yet implemented -- RED scaffold");
    }

    fn write_signed_claim(&self, _signed: &SignedClaim) -> Result<(), StorageError> {
        panic!("Not yet implemented -- RED scaffold");
    }

    // SCAFFOLD: true (slice-24) — the philosophy-mint acceptance scenarios
    // (PA-1..5) drive the REAL `DuckDbStorageAdapter` through the subprocess
    // binary, never this in-memory double, so this method stays a RED scaffold
    // (the whole `InMemoryStorage` is an all-`panic!` scaffold). It exists only
    // to satisfy the extended `StoragePort` trait surface.
    fn write_signed_philosophy(
        &self,
        _signed: &ports::SignedPhilosophy,
    ) -> Result<(), StorageError> {
        panic!("Not yet implemented -- RED scaffold (slice-24 write_signed_philosophy)");
    }

    fn read_signed_claim(&self, _cid: &Cid) -> Result<Option<SignedClaim>, StorageError> {
        panic!("Not yet implemented -- RED scaffold");
    }

    fn query_by_subject(&self, _subject: &str) -> Result<Vec<SignedClaim>, StorageError> {
        panic!("Not yet implemented -- RED scaffold");
    }

    fn query_referencing(
        &self,
        _target_cid: &Cid,
    ) -> Result<Vec<(Cid, claim_domain::ReferenceType)>, StorageError> {
        panic!("Not yet implemented -- RED scaffold");
    }

    fn record_publication(
        &self,
        _cid: &Cid,
        _at_uri: &str,
        _published_at: chrono::DateTime<chrono::Utc>,
    ) -> Result<(), StorageError> {
        panic!("Not yet implemented -- RED scaffold");
    }

    /// Cross-store federated query (slice-03 extension).
    ///
    /// SCAFFOLD: true (slice-03) — RED scaffold. The FQ-* acceptance
    /// scenarios drive a real in-memory federated read (own + peer rows,
    /// each carrying non-Option `author_did`) in a later slice-03 phase.
    fn query_federated_by_subject(
        &self,
        _subject: &str,
    ) -> Result<Vec<ports::FederatedRow>, StorageError> {
        // SCAFFOLD: true (slice-03)
        panic!("Not yet implemented -- RED scaffold (slice-03 query_federated_by_subject)");
    }

    // -------- slice-04 (scoring + graph) read methods --------
    //
    // SCAFFOLD: true (slice-04) — the layer-2 `scoring_core.rs` acceptance file
    // invokes the pure `scoring::score` directly (no StoragePort), so this
    // in-memory double only needs the method shapes to satisfy the extended
    // trait. The subprocess `graph_query_explore.rs` scenarios drive the REAL
    // `DuckDbStorageAdapter` (over a seeded DuckDB), never this double, so the
    // bodies stay RED scaffolds. They materialize only if a future layer-2
    // acceptance scenario seeds an in-memory feed.

    fn query_by_object(&self, _object: &str) -> Result<Vec<AttributedClaim>, StorageError> {
        panic!("Not yet implemented -- RED scaffold (slice-04 query_by_object)");
    }

    fn query_by_contributor(
        &self,
        _author_did: &Did,
    ) -> Result<Vec<AttributedClaim>, StorageError> {
        panic!("Not yet implemented -- RED scaffold (slice-04 query_by_contributor)");
    }

    fn query_attributed_for_scoring(
        &self,
        _filter: &ScoringFilter,
    ) -> Result<Vec<AttributedClaim>, StorageError> {
        panic!("Not yet implemented -- RED scaffold (slice-04 query_attributed_for_scoring)");
    }

    fn traverse_graph(
        &self,
        _start: &GraphNode,
        _bound: &TraversalBound,
    ) -> Result<TraversalResult, StorageError> {
        panic!("Not yet implemented -- RED scaffold (slice-04 traverse_graph)");
    }
}
