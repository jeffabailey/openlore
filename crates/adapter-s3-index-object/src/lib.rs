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
//!   `GetParameter`, returning the raw text (see `ssm`).
//!
//! The port probes are sync (ADR-009) and their calls are async: see `wait_for`.
//!
//! The only crate besides the Lambda root that may link `aws-*` (xtask
//! `aws_sdk_only_in_lambda_edge`).

#![forbid(unsafe_code)]

mod client_settings;
mod s3;
mod ssm;

use std::future::Future;
use std::path::Path;
use std::time::Duration;

use async_trait::async_trait;
use aws_config::{BehaviorVersion, Region, SdkConfig};
use aws_smithy_http_client::tls::rustls_provider::CryptoMode;
use aws_smithy_http_client::tls::Provider;
use aws_smithy_http_client::Builder as HttpClientBuilder;
use ports::{
    HeadOutcome, IndexObjectError, IndexObjectReadPort, IndexObjectWritePort, LastPass,
    ObjectVersion, ProbeOutcome, ProbeRefusalReason, ProbeRefused, PublishMetadata, PublishOutcome,
    RepoDidListError, RepoDidListSourcePort, WriteCondition, PUBLISH_OPERATION_TIMEOUT,
};
use tokio::runtime::{Builder as RuntimeBuilder, Handle};

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

/// What every client starts from: the HTTP client `default-features = false`
/// leaves out (D-STZ-36), built once, and one attempt even for a call with no
/// override.
pub(crate) async fn shared_config(region: &str) -> SdkConfig {
    let http_client = HttpClientBuilder::new()
        .tls_provider(Provider::Rustls(CryptoMode::Ring))
        .build_https();
    aws_config::defaults(BehaviorVersion::latest())
        .http_client(http_client)
        .region(Region::new(region.to_string()))
        .retry_config(client_settings(StoreCall::PassGet).retry)
        .load()
        .await
}

/// Wait for an async call from a sync `probe` (ADR-009).
///
/// Init calls a probe through `spawn_blocking` (D-STZ-39), so the thread can reach
/// the runtime it came from, and the connection the call opens stays with that
/// runtime for the calls after it. Called on a thread that is running async work,
/// this panics, which is why init does not.
///
/// The handle is the caller's and not one kept from `connect`: a current-thread
/// runtime that nobody is running cannot drive a handle's I/O, so a kept handle
/// could wait forever. With no runtime around, the call gets one of its own, for
/// as long as it lasts. `Err` when that one cannot be built.
fn wait_for<T>(call: impl Future<Output = T>) -> std::io::Result<T> {
    if let Ok(runtime) = Handle::try_current() {
        return Ok(runtime.block_on(call));
    }
    RuntimeBuilder::new_current_thread()
        .enable_all()
        .build()
        .map(|own_runtime| own_runtime.block_on(call))
}

/// [`wait_for`] for a probe that refuses a start when it cannot run.
fn run_probe(call: impl Future<Output = ProbeOutcome>, reason: ProbeRefusalReason) -> ProbeOutcome {
    wait_for(call).unwrap_or_else(|failed| {
        ProbeRefused {
            reason,
            detail: format!("no runtime for the probe: {failed}"),
            structured: serde_json::json!({}),
        }
        .into()
    })
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
        run_probe(
            self.store.read_probe(),
            ProbeRefusalReason::IndexObjectUnusable,
        )
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
    /// A write probe that could not run says nothing about conditions, so unlike
    /// [`run_probe`] it is no refusal (D-STZ-39).
    fn probe(&self) -> ProbeOutcome {
        let seen = wait_for(self.store.write_probe_seen()).unwrap_or(WriteProbeSeen::Unknown);
        write_probe_outcome(seen)
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
    list: ssm::DidListParameter,
}

impl SsmRepoDidListAdapter {
    /// Build the adapter (credentials from the standard provider chain).
    pub async fn connect(location: SsmLocation) -> Result<Self, RepoDidListError> {
        Ok(Self {
            list: ssm::DidListParameter::connect(location).await,
        })
    }
}

#[async_trait]
impl RepoDidListSourcePort for SsmRepoDidListAdapter {
    fn probe(&self) -> ProbeOutcome {
        run_probe(self.list.probe(), ProbeRefusalReason::IndexerConfigInvalid)
    }

    async fn current(&self) -> Result<String, RepoDidListError> {
        self.list.read().await
    }
}

#[cfg(test)]
mod tests {
    use tokio::sync::oneshot;

    use super::*;

    /// A probe that leaves a task behind it, as a call leaves a pooled connection.
    /// The task answers only if the runtime it landed on is still there.
    async fn probe_leaving_a_task(
        asked: oneshot::Receiver<()>,
        answer: oneshot::Sender<()>,
    ) -> ProbeOutcome {
        tokio::spawn(async move {
            if asked.await.is_ok() {
                let _ = answer.send(());
            }
        });
        tokio::time::sleep(Duration::from_millis(1)).await;
        ProbeOutcome::Ok
    }

    // bypass: two fixed ways to be called.
    #[tokio::test]
    async fn on_a_blocking_thread_a_probe_stays_on_the_callers_runtime() {
        let (ask, asked) = oneshot::channel();
        let (answer, answered) = oneshot::channel();

        let outcome = tokio::task::spawn_blocking(move || {
            run_probe(
                probe_leaving_a_task(asked, answer),
                ProbeRefusalReason::IndexObjectUnusable,
            )
        })
        .await
        .expect("the probe answered");

        assert!(matches!(outcome, ProbeOutcome::Ok));
        ask.send(()).expect("the task is still there");
        assert_eq!(answered.await, Ok(()));
    }

    // bypass: two fixed ways to be called.
    #[test]
    fn with_no_runtime_around_a_probe_runs_on_its_own() {
        let (_ask, asked) = oneshot::channel();
        let (answer, _answered) = oneshot::channel();

        let outcome = run_probe(
            probe_leaving_a_task(asked, answer),
            ProbeRefusalReason::IndexObjectUnusable,
        );

        assert!(matches!(outcome, ProbeOutcome::Ok));
    }
}
