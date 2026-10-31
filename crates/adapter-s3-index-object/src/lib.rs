//! `adapter-s3-index-object` — the EFFECT shell over the one published index
//! object and the operator's DID list (ADR-090, D-STZ-3):
//!
//! * [`S3IndexObjectAdapter`] implements `IndexObjectReadPort` and
//!   `IndexObjectWritePort` over S3 (D-STZ-38): `HeadObject`, `GetObject`,
//!   conditional `PutObject` with `If-Match` / `If-None-Match: *`, metadata
//!   `published-at` and `pass-id`. A missing object is `Absent` (the role has
//!   `s3:ListBucket`, so S3 answers 404, not 403); a missing bucket is a store
//!   failure; a 412 is `PublishOutcome::Conflict`. See `s3`, which holds the
//!   mapping. The write probe (D-STZ-39) writes only
//!   `index/.write-probe`, and the pure [`write_probe_outcome`] reads its PUTs.
//! * [`SsmRepoDidListAdapter`] implements `RepoDidListSourcePort` over SSM
//!   `GetParameter`, returning the raw text.
//!
//! The only crate besides the Lambda root that may link `aws-*` (xtask
//! `aws_sdk_only_in_lambda_edge`).
//
// SCAFFOLD: true  (the port probes and the SSM adapter still panic: 06-02, 06-03.
// Contract: tests/acceptance/scale_to_zero_object_store_real_io.rs)

#![forbid(unsafe_code)]

mod client_settings;
mod s3;

use std::path::Path;
use std::time::Duration;

use async_trait::async_trait;
use ports::{
    HeadOutcome, IndexObjectError, IndexObjectReadPort, IndexObjectWritePort, LastPass,
    ObjectVersion, ProbeOutcome, PublishMetadata, PublishOutcome, RepoDidListError,
    RepoDidListSourcePort, WriteCondition, PUBLISH_OPERATION_TIMEOUT,
};

pub use client_settings::{client_settings, ClientSettings};
pub use ports::{
    location_names_write_probe_key, write_probe_outcome, RepeatAnswer, WriteProbeSeen,
    WRITE_PROBE_KEY,
};

/// Every call this adapter makes (DESIGN § Timeouts).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StoreCall {
    WriteProbePut,
    ReadProbeHead,
    SearchInitGet,
    FreshnessHead,
    Reload,
    GetParameter,
    PassGet,
    Publish,
    PutLastPass,
    GetLastPass,
}

/// One call's bound: an operation timeout, and how many attempts it may make.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CallBudget {
    pub operation_timeout: Duration,
    pub max_attempts: u32,
}

/// The one place the § Timeouts table lives. Every S3 and SSM client takes its
/// settings from here, so each figure is the real worst case.
pub fn call_budget(call: StoreCall) -> CallBudget {
    let seconds = match call {
        StoreCall::FreshnessHead => 1,
        StoreCall::WriteProbePut | StoreCall::ReadProbeHead => 2,
        StoreCall::Reload | StoreCall::GetLastPass => 3,
        StoreCall::GetParameter | StoreCall::PutLastPass => 5,
        StoreCall::SearchInitGet => 8,
        StoreCall::Publish => PUBLISH_OPERATION_TIMEOUT.as_secs(),
        StoreCall::PassGet => 60,
    };
    CallBudget {
        operation_timeout: Duration::from_secs(seconds),
        max_attempts: 1,
    }
}

/// Where the index object lives.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct S3Location {
    pub region: String,
    /// `AWS_ENDPOINT_URL` override (moto in tests and in the CI lambda-smoke).
    pub endpoint_url: Option<String>,
    pub bucket: String,
    /// `index/index.duckdb`.
    pub index_key: String,
    /// `index/last-pass.json`.
    pub last_pass_key: String,
}

/// Where the DID list lives.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SsmLocation {
    pub region: String,
    pub endpoint_url: Option<String>,
    /// `/openlore/prod/indexer/repo-dids`.
    pub parameter: String,
}

/// `IndexObjectReadPort` and `IndexObjectWritePort` over S3.
pub struct S3IndexObjectAdapter {
    store: s3::IndexObjectStore,
}

impl S3IndexObjectAdapter {
    /// Build the adapter (credentials from the standard provider chain). Refused
    /// when [`location_names_write_probe_key`] says a configured key is
    /// [`WRITE_PROBE_KEY`].
    pub async fn connect(location: S3Location) -> Result<Self, IndexObjectError> {
        if location_names_write_probe_key(&location.index_key, &location.last_pass_key) {
            return Err(IndexObjectError::ProbeRefused {
                detail: format!("a configured key is the write probe's key {WRITE_PROBE_KEY}"),
            });
        }
        Ok(Self {
            store: s3::IndexObjectStore::connect(location).await,
        })
    }

    /// The budget `fetch_to` runs under, by default [`StoreCall::PassGet`]. Search
    /// composes [`StoreCall::SearchInitGet`], and holds a reload to its own bound.
    #[must_use]
    pub fn fetching_as(self, call: StoreCall) -> Self {
        Self {
            store: self.store.fetching_as(call),
        }
    }
}

#[async_trait]
impl IndexObjectReadPort for S3IndexObjectAdapter {
    fn probe(&self) -> ProbeOutcome {
        panic!("Not yet implemented -- RED scaffold")
    }

    async fn head(&self) -> Result<HeadOutcome, IndexObjectError> {
        self.store.head().await
    }

    async fn fetch_to(&self, dest: &Path) -> Result<Option<ObjectVersion>, IndexObjectError> {
        self.store.fetch_to(dest).await
    }

    async fn get_last_pass(&self) -> Result<Option<LastPass>, IndexObjectError> {
        self.store.get_last_pass().await
    }
}

#[async_trait]
impl IndexObjectWritePort for S3IndexObjectAdapter {
    fn probe(&self) -> ProbeOutcome {
        panic!("Not yet implemented -- RED scaffold")
    }

    async fn publish_if_match(
        &self,
        src: &Path,
        condition: WriteCondition,
        metadata: PublishMetadata,
        limit: Duration,
    ) -> Result<PublishOutcome, IndexObjectError> {
        self.store.publish(src, condition, metadata, limit).await
    }

    async fn put_last_pass(&self, last: &LastPass) -> Result<(), IndexObjectError> {
        self.store.put_last_pass(last).await
    }
}

/// `RepoDidListSourcePort` over SSM.
pub struct SsmRepoDidListAdapter {
    _location: SsmLocation,
}

impl SsmRepoDidListAdapter {
    /// Build the adapter (credentials from the standard provider chain).
    pub async fn connect(location: SsmLocation) -> Result<Self, RepoDidListError> {
        let _ = location;
        panic!("Not yet implemented -- RED scaffold")
    }
}

#[async_trait]
impl RepoDidListSourcePort for SsmRepoDidListAdapter {
    fn probe(&self) -> ProbeOutcome {
        panic!("Not yet implemented -- RED scaffold")
    }

    async fn current(&self) -> Result<String, RepoDidListError> {
        panic!("Not yet implemented -- RED scaffold")
    }
}
