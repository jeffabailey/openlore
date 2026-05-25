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

use async_trait::async_trait;
use claim_domain::{Cid, Did, SignatureBlock, SignedClaim};
use ports::{
    AtUri, ClockPort, IdentityError, IdentityPort, PdsError, PdsPort, ProbeOutcome, StorageError,
    StoragePort,
};

// -----------------------------------------------------------------------------
// FakePds — in-memory PdsPort double
// -----------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct FakePdsRecord {
    pub collection: String,
    pub rkey: String,
    pub body: serde_json::Value,
    pub author_did: String,
    pub at_uri: String,
}

pub struct FakePds {
    records: std::sync::Mutex<Vec<FakePdsRecord>>,
    unreachable: std::sync::Mutex<bool>,
}

impl FakePds {
    pub fn new() -> Self {
        Self {
            records: std::sync::Mutex::new(Vec::new()),
            unreachable: std::sync::Mutex::new(false),
        }
    }

    pub fn records(&self) -> Vec<FakePdsRecord> {
        panic!("Not yet implemented -- RED scaffold");
    }

    pub fn record_at(&self, _at_uri: &str) -> Option<FakePdsRecord> {
        panic!("Not yet implemented -- RED scaffold");
    }

    pub fn simulate_unreachable(&self) {
        panic!("Not yet implemented -- RED scaffold");
    }

    pub fn restore(&self) {
        panic!("Not yet implemented -- RED scaffold");
    }
}

impl Default for FakePds {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl PdsPort for FakePds {
    fn probe(&self) -> ProbeOutcome {
        panic!("Not yet implemented -- RED scaffold");
    }

    async fn create_record(
        &self,
        _collection: &str,
        _rkey: &str,
        _body: serde_json::Value,
    ) -> Result<AtUri, PdsError> {
        panic!("Not yet implemented -- RED scaffold");
    }

    async fn get_record(
        &self,
        _collection: &str,
        _rkey: &str,
    ) -> Result<Option<serde_json::Value>, PdsError> {
        panic!("Not yet implemented -- RED scaffold");
    }

    async fn list_records(
        &self,
        _collection: &str,
    ) -> Result<Vec<serde_json::Value>, PdsError> {
        panic!("Not yet implemented -- RED scaffold");
    }
}

// -----------------------------------------------------------------------------
// FakeIdentity — deterministic IdentityPort double
// -----------------------------------------------------------------------------

pub struct FakeIdentity {
    did: Did,
}

impl FakeIdentity {
    /// Canonical did:plc:test-jeff identity used across slice-01 tests.
    pub fn jeff() -> Self {
        panic!("Not yet implemented -- RED scaffold");
    }

    /// Secondary did:plc:test-maria identity (US-002 Ex 3, US-003 Ex 2, WS-10).
    pub fn maria() -> Self {
        panic!("Not yet implemented -- RED scaffold");
    }
}

impl IdentityPort for FakeIdentity {
    fn probe(&self) -> ProbeOutcome {
        panic!("Not yet implemented -- RED scaffold");
    }

    fn author_did(&self) -> &Did {
        &self.did
    }

    fn sign(&self, _unsigned_cid: &Cid) -> Result<SignatureBlock, IdentityError> {
        panic!("Not yet implemented -- RED scaffold");
    }

    fn verify(&self, _signed: &SignedClaim) -> Result<(), IdentityError> {
        panic!("Not yet implemented -- RED scaffold");
    }
}

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
}
