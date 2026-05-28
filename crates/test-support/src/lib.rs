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
    FakeAuthMode, FakeGithub, FakeGithubErrorPosture, FakeGithubHttpHandle, FakeSignal,
    FakeTargetKind, FIXTURE_REJECTED_PAT, FIXTURE_REPO_TARGET, FIXTURE_USER_TARGET,
    FIXTURE_VALID_PAT,
};

// Slice-02 step 07-01: canonical GitHub-harvest fixtures. Symmetric with
// `fixtures_peer.rs`; one free function per well-known signal set used
// across US-SCR-001..005 acceptance scenarios. The fixtures supply raw
// harvested EFFECT signals only; the candidate derivation (predicate, 0.25
// default) is the PURE `scraper-domain`'s job downstream (WD-56).
pub mod fixtures_github;
pub use fixtures_github::{
    fixture_cargo_five_signals, fixture_three_docs_signals_one_predicate,
    fixture_torvalds_user_aggregate_signals,
};

use claim_domain::{Cid, ClaimLookup, SignedClaim};
use ports::{ClockPort, ProbeOutcome, StorageError, StoragePort};

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
}
