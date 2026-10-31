//! The S3 calls behind the index object (D-STZ-5, D-STZ-6, D-STZ-12, D-STZ-38).
//!
//! What an answer means is pure, and lives at the top of this file: a 404 is
//! absent, a 412 is a conflict, and anything else is a store failure. The rest
//! makes the calls and reads their answers through those functions.
//!
//! Every call takes its limits from `client_settings(call)`. A `GetObject` also
//! runs inside [`within`], because the SDK's operation timeout ends when the
//! headers arrive and leaves the body unbounded (DESIGN § Timeouts). `head` uses
//! it too, for the two calls a 404 can take.
//!
//! A `HEAD` carries no body, so its 404 cannot say whether the key or the bucket
//! is missing. A `GET` of the same key can, and every role that may `HEAD` the key
//! may `GET` it. `HeadBucket` would not do: the roles' `s3:ListBucket` holds only
//! with an `s3:prefix` condition, and that call sends no prefix.

use std::collections::HashMap;
use std::fmt::Display;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::time::Duration;

use aws_config::timeout::TimeoutConfig;
use aws_sdk_s3::config::http::HttpResponse;
use aws_sdk_s3::error::{DisplayErrorContext, ProvideErrorMetadata, SdkError};
use aws_sdk_s3::operation::get_object::GetObjectOutput;
use aws_sdk_s3::primitives::ByteStream;
use aws_sdk_s3::Client;
use ports::{
    HeadOutcome, IndexObjectError, LastPass, ObjectVersion, ProbeOutcome, ProbeRefusalReason,
    ProbeRefused, PublishMetadata, PublishOutcome, WriteCondition,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;

use crate::client_settings::{client_settings, ClientSettings};
use crate::{call_budget, shared_config, S3Location, StoreCall};

/// `x-amz-meta-published-at` (D-STZ-12).
const PUBLISHED_AT_KEY: &str = "published-at";
/// `x-amz-meta-pass-id`.
const PASS_ID_KEY: &str = "pass-id";

/// S3's error code for a bucket that does not exist.
const NO_SUCH_BUCKET: &str = "NoSuchBucket";

/// How a store failed: it answered no, or it did not answer usefully.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StoreFailure {
    Refused,
    Unavailable,
}

impl StoreFailure {
    /// A 4xx is the store refusing. A 5xx, an odd status or no answer at all
    /// says only that it could not serve the call.
    fn of(status: Option<u16>) -> Self {
        match status {
            Some(400..=499) => Self::Refused,
            _ => Self::Unavailable,
        }
    }

    fn error(self, detail: String) -> IndexObjectError {
        match self {
            Self::Refused => IndexObjectError::Refused { detail },
            Self::Unavailable => IndexObjectError::Unavailable { detail },
        }
    }
}

/// What a failed read means.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReadAnswer {
    Absent,
    Failed(StoreFailure),
}

/// A 404 is absent, unless the store says it is the bucket that is missing.
fn read_answer(status: Option<u16>, code: Option<&str>) -> ReadAnswer {
    match (status, code) {
        (Some(404), Some(NO_SUCH_BUCKET)) => ReadAnswer::Failed(StoreFailure::Refused),
        (Some(404), _) => ReadAnswer::Absent,
        _ => ReadAnswer::Failed(StoreFailure::of(status)),
    }
}

/// What a failed conditional write means.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WriteAnswer {
    Conflict,
    Failed(StoreFailure),
}

/// Only a 412 is a lost update (D-STZ-6, D-STZ-41).
fn write_answer(status: Option<u16>) -> WriteAnswer {
    match status {
        Some(412) => WriteAnswer::Conflict,
        _ => WriteAnswer::Failed(StoreFailure::of(status)),
    }
}

/// The status and error code of a call the store answered. A timeout or a dropped
/// connection has neither.
fn store_reply<E: ProvideErrorMetadata>(
    failed: &SdkError<E, HttpResponse>,
) -> (Option<u16>, Option<&str>) {
    match failed {
        SdkError::ServiceError(reply) => (Some(reply.raw().status().as_u16()), reply.err().code()),
        _ => (None, None),
    }
}

fn detail_of<E>(failed: &SdkError<E, HttpResponse>) -> String
where
    E: std::error::Error + 'static,
{
    DisplayErrorContext(failed).to_string()
}

/// `Ok` when the failed read found nothing there.
fn absent_or_failed<E>(failed: &SdkError<E, HttpResponse>) -> Result<(), IndexObjectError>
where
    E: ProvideErrorMetadata + std::error::Error + 'static,
{
    let (status, code) = store_reply(failed);
    match read_answer(status, code) {
        ReadAnswer::Absent => Ok(()),
        ReadAnswer::Failed(failure) => Err(failure.error(detail_of(failed))),
    }
}

fn unavailable(detail: impl Display) -> IndexObjectError {
    IndexObjectError::Unavailable {
        detail: detail.to_string(),
    }
}

fn local_write(failed: impl Display) -> IndexObjectError {
    IndexObjectError::LocalWrite {
        detail: failed.to_string(),
    }
}

/// The version an answer describes. An answer without the object's identity or
/// size is not one this adapter can act on.
fn object_version(
    etag: Option<&str>,
    metadata: Option<&HashMap<String, String>>,
    size_bytes: Option<i64>,
) -> Result<ObjectVersion, IndexObjectError> {
    let named = |key: &str| metadata.and_then(|held| held.get(key)).cloned();
    Ok(ObjectVersion {
        etag: etag_of(etag)?,
        published_at: named(PUBLISHED_AT_KEY),
        pass_id: named(PASS_ID_KEY),
        size_bytes: size_bytes
            .and_then(|size| u64::try_from(size).ok())
            .ok_or_else(|| unavailable("the store gave no object size"))?,
    })
}

fn etag_of(etag: Option<&str>) -> Result<String, IndexObjectError> {
    etag.map(str::to_string)
        .ok_or_else(|| unavailable("the store gave no ETag"))
}

/// The version a publish made: the store names it, and the rest is what was sent.
fn published_version(
    etag: Option<&str>,
    metadata: PublishMetadata,
    size_bytes: u64,
) -> Result<ObjectVersion, IndexObjectError> {
    Ok(ObjectVersion {
        etag: etag_of(etag)?,
        published_at: Some(metadata.published_at),
        pass_id: Some(metadata.pass_id),
        size_bytes,
    })
}

/// D-STZ-39: the probe proves the role reaches the store. An index that is not
/// there yet is a reachable store (D-STZ-13), and every failure refuses the start.
fn read_probe_outcome(checked: Result<HeadOutcome, IndexObjectError>) -> ProbeOutcome {
    match checked {
        Ok(HeadOutcome::Present(_) | HeadOutcome::Absent) => ProbeOutcome::Ok,
        Err(failed) => ProbeRefused {
            reason: ProbeRefusalReason::IndexObjectUnusable,
            detail: failed.to_string(),
            structured: json!({}),
        }
        .into(),
    }
}

/// DuckDB opens a truncated file, because opening reads only metadata, so a short
/// body has to stop here.
fn whole_body(written: u64, object_size: u64) -> Result<(), IndexObjectError> {
    if written == object_size {
        Ok(())
    } else {
        Err(unavailable(format!(
            "the download ended at {written} of {object_size} bytes"
        )))
    }
}

/// The publish's settings under `limit`, the time the pass can still give it
/// (D-STZ-40a). `None` when there is none left.
fn publish_settings(limit: Duration) -> Option<ClientSettings> {
    let budget = call_budget(StoreCall::Publish).operation_timeout;
    (!limit.is_zero()).then(|| ClientSettings {
        timeouts: TimeoutConfig::builder()
            .operation_timeout(limit.min(budget))
            .build(),
        ..client_settings(StoreCall::Publish)
    })
}

/// `index/last-pass.json`, the record's stored form (D-STZ-7).
#[derive(Serialize, Deserialize)]
struct StoredPassRecord {
    pass_id: String,
    exit_code: i32,
    finished_at: String,
    published: bool,
    index_published_at: Option<String>,
}

impl From<&LastPass> for StoredPassRecord {
    fn from(last: &LastPass) -> Self {
        Self {
            pass_id: last.pass_id.clone(),
            exit_code: last.exit_code,
            finished_at: last.finished_at.clone(),
            published: last.published,
            index_published_at: last.index_published_at.clone(),
        }
    }
}

impl From<StoredPassRecord> for LastPass {
    fn from(stored: StoredPassRecord) -> Self {
        Self {
            pass_id: stored.pass_id,
            exit_code: stored.exit_code,
            finished_at: stored.finished_at,
            published: stored.published,
            index_published_at: stored.index_published_at,
        }
    }
}

/// A record that exists and does not parse is an error, never "no pass yet"
/// (UI-12).
fn parse_pass_record(bytes: &[u8]) -> Result<LastPass, IndexObjectError> {
    serde_json::from_slice::<StoredPassRecord>(bytes)
        .map(LastPass::from)
        .map_err(|unread| unavailable(format!("the pass record does not parse: {unread}")))
}

/// Hold `work` to the budget of `call`, whatever it is waiting on.
async fn within<T>(
    call: StoreCall,
    work: impl Future<Output = Result<T, IndexObjectError>>,
) -> Result<T, IndexObjectError> {
    let limit = call_budget(call).operation_timeout;
    tokio::time::timeout(limit, work)
        .await
        .unwrap_or_else(|_| Err(unavailable(format!("{call:?} ran past {limit:?}"))))
}

/// A download under way. Unless it finishes, dropping it removes its file, so a
/// failure, a timeout and a caller that gave up all leave no file behind.
struct PartialDownload {
    file: PathBuf,
    finished: bool,
}

impl PartialDownload {
    fn at(file: &Path) -> Self {
        Self {
            file: file.to_path_buf(),
            finished: false,
        }
    }

    fn finish(mut self) {
        self.finished = true;
    }
}

impl Drop for PartialDownload {
    fn drop(&mut self) {
        if !self.finished {
            let _ = std::fs::remove_file(&self.file);
        }
    }
}

/// Stream `body` into `file`, returning how many bytes reached it.
async fn write_body(mut body: ByteStream, file: &mut File) -> Result<u64, IndexObjectError> {
    let mut written = 0;
    while let Some(chunk) = body
        .try_next()
        .await
        .map_err(|broken| unavailable(format!("the download broke off: {broken}")))?
    {
        file.write_all(&chunk).await.map_err(local_write)?;
        written += chunk.len() as u64;
    }
    file.flush().await.map_err(local_write)?;
    Ok(written)
}

/// The shared client (see `shared_config`), at the configured endpoint.
async fn client_at(location: &S3Location) -> Client {
    let shared = shared_config(&location.region).await;
    let config = aws_sdk_s3::config::Builder::from(&shared);
    let config = match &location.endpoint_url {
        Some(endpoint) => config.endpoint_url(endpoint).force_path_style(true),
        None => config,
    };
    Client::from_conf(config.build())
}

/// The bucket's two keys, and the budget a download of the index runs under.
pub(crate) struct IndexObjectStore {
    client: Client,
    bucket: String,
    index_key: String,
    last_pass_key: String,
    fetch_call: StoreCall,
}

impl IndexObjectStore {
    pub(crate) async fn connect(location: S3Location) -> Self {
        Self {
            client: client_at(&location).await,
            bucket: location.bucket,
            index_key: location.index_key,
            last_pass_key: location.last_pass_key,
            fetch_call: StoreCall::PassGet,
        }
    }

    pub(crate) fn fetching_as(self, call: StoreCall) -> Self {
        Self {
            fetch_call: call,
            ..self
        }
    }

    pub(crate) async fn head(&self) -> Result<HeadOutcome, IndexObjectError> {
        self.head_as(StoreCall::FreshnessHead).await
    }

    /// The read probe: the same check as `head`, under its own budget and through
    /// the same role, so it cannot use `HeadBucket` either.
    pub(crate) async fn read_probe(&self) -> ProbeOutcome {
        read_probe_outcome(self.head_as(StoreCall::ReadProbeHead).await)
    }

    async fn head_as(&self, call: StoreCall) -> Result<HeadOutcome, IndexObjectError> {
        within(call, self.head_within_budget(call)).await
    }

    async fn head_within_budget(&self, call: StoreCall) -> Result<HeadOutcome, IndexObjectError> {
        let checked = self
            .client
            .head_object()
            .bucket(&self.bucket)
            .key(&self.index_key)
            .customize()
            .config_override(client_settings(call).s3_config())
            .send()
            .await;
        match checked {
            Ok(found) => object_version(found.e_tag(), found.metadata(), found.content_length())
                .map(HeadOutcome::Present),
            Err(failed) => {
                absent_or_failed(&failed)?;
                self.bucket_exists(call).await.map(|()| HeadOutcome::Absent)
            }
        }
    }

    /// After a `HEAD` 404. An object that appeared since then is the next check's.
    async fn bucket_exists(&self, call: StoreCall) -> Result<(), IndexObjectError> {
        self.get(&self.index_key, call).await.map(drop)
    }

    pub(crate) async fn fetch_to(
        &self,
        dest: &Path,
    ) -> Result<Option<ObjectVersion>, IndexObjectError> {
        within(self.fetch_call, self.download(dest)).await
    }

    async fn download(&self, dest: &Path) -> Result<Option<ObjectVersion>, IndexObjectError> {
        let Some(object) = self.get(&self.index_key, self.fetch_call).await? else {
            return Ok(None);
        };
        let version = object_version(object.e_tag(), object.metadata(), object.content_length())?;
        let partial = PartialDownload::at(dest);
        let mut file = File::create(dest).await.map_err(local_write)?;
        let written = write_body(object.body, &mut file).await?;
        whole_body(written, version.size_bytes)?;
        partial.finish();
        Ok(Some(version))
    }

    pub(crate) async fn get_last_pass(&self) -> Result<Option<LastPass>, IndexObjectError> {
        within(StoreCall::GetLastPass, self.read_pass_record()).await
    }

    async fn read_pass_record(&self) -> Result<Option<LastPass>, IndexObjectError> {
        let call = StoreCall::GetLastPass;
        let Some(object) = self.get(&self.last_pass_key, call).await? else {
            return Ok(None);
        };
        let bytes = object.body.collect().await.map_err(unavailable)?;
        parse_pass_record(&bytes.into_bytes()).map(Some)
    }

    /// A `GetObject`. `None` when the key is not there.
    async fn get(
        &self,
        key: &str,
        call: StoreCall,
    ) -> Result<Option<GetObjectOutput>, IndexObjectError> {
        let got = self
            .client
            .get_object()
            .bucket(&self.bucket)
            .key(key)
            .customize()
            .config_override(client_settings(call).s3_config())
            .send()
            .await;
        match got {
            Ok(object) => Ok(Some(object)),
            Err(failed) => absent_or_failed(&failed).map(|()| None),
        }
    }

    pub(crate) async fn publish(
        &self,
        src: &Path,
        condition: WriteCondition,
        metadata: PublishMetadata,
        limit: Duration,
    ) -> Result<PublishOutcome, IndexObjectError> {
        let settings =
            publish_settings(limit).ok_or_else(|| unavailable("no time left to publish"))?;
        let size_bytes = tokio::fs::metadata(src).await.map_err(local_write)?.len();
        let body = ByteStream::from_path(src).await.map_err(local_write)?;
        let request = self
            .client
            .put_object()
            .bucket(&self.bucket)
            .key(&self.index_key)
            .metadata(PUBLISHED_AT_KEY, &metadata.published_at)
            .metadata(PASS_ID_KEY, &metadata.pass_id)
            .body(body);
        let request = match condition {
            WriteCondition::IfMatch(etag) => request.if_match(etag),
            WriteCondition::IfNoneMatch => request.if_none_match("*"),
        };
        let sent = request
            .customize()
            .config_override(settings.s3_config())
            .send()
            .await;
        match sent {
            Ok(put) => {
                published_version(put.e_tag(), metadata, size_bytes).map(PublishOutcome::Published)
            }
            Err(failed) => match write_answer(store_reply(&failed).0) {
                WriteAnswer::Conflict => Ok(PublishOutcome::Conflict),
                WriteAnswer::Failed(failure) => Err(failure.error(detail_of(&failed))),
            },
        }
    }

    pub(crate) async fn put_last_pass(&self, last: &LastPass) -> Result<(), IndexObjectError> {
        let record = serde_json::to_vec(&StoredPassRecord::from(last)).map_err(unavailable)?;
        let sent = self
            .client
            .put_object()
            .bucket(&self.bucket)
            .key(&self.last_pass_key)
            .content_type("application/json")
            .body(ByteStream::from(record))
            .customize()
            .config_override(client_settings(StoreCall::PutLastPass).s3_config())
            .send()
            .await;
        match sent {
            Ok(_) => Ok(()),
            Err(failed) => Err(StoreFailure::of(store_reply(&failed).0).error(detail_of(&failed))),
        }
    }
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    const THIRTY_FIVE_SECONDS: Duration = Duration::from_secs(35);

    fn any_code() -> impl Strategy<Value = Option<String>> {
        prop::option::of(prop_oneof![
            Just("NoSuchKey".to_string()),
            Just(NO_SUCH_BUCKET.to_string()),
            Just("AccessDenied".to_string()),
            Just("PreconditionFailed".to_string()),
            "[A-Za-z]{1,12}",
        ])
    }

    fn any_status() -> impl Strategy<Value = Option<u16>> {
        prop::option::of(prop_oneof![
            Just(403u16),
            Just(404),
            Just(412),
            Just(500),
            100..600u16,
        ])
    }

    fn failure_for(status: Option<u16>) -> StoreFailure {
        match status {
            Some(status) if (400..500).contains(&status) => StoreFailure::Refused,
            _ => StoreFailure::Unavailable,
        }
    }

    fn the_limit_of(settings: ClientSettings) -> Option<Duration> {
        settings.timeouts.operation_timeout()
    }

    proptest! {
        // RIO-3, UI-12: nothing but a 404 for a key is "not there", so a refusal
        // or an outage can never read as an empty start.
        #[test]
        fn a_read_is_absent_only_on_a_404_that_does_not_name_the_bucket(
            status in any_status(),
            code in any_code(),
        ) {
            let answer = read_answer(status, code.as_deref());

            let bucket_missing = code.as_deref() == Some(NO_SUCH_BUCKET);
            let expected = match status {
                Some(404) if !bucket_missing => ReadAnswer::Absent,
                Some(404) => ReadAnswer::Failed(StoreFailure::Refused),
                _ => ReadAnswer::Failed(failure_for(status)),
            };
            prop_assert_eq!(answer, expected);
        }

        // D-STZ-6, D-STZ-41, RIO-7: only a 412 is a lost update.
        #[test]
        fn a_write_is_a_conflict_only_on_a_412(status in any_status()) {
            let expected = if status == Some(412) {
                WriteAnswer::Conflict
            } else {
                WriteAnswer::Failed(failure_for(status))
            };

            prop_assert_eq!(write_answer(status), expected);
        }

        // D-STZ-12: each stamp comes back from its own key.
        #[test]
        fn a_version_reads_each_stamp_from_its_own_key(
            etag in "[a-f0-9]{1,16}",
            published_at in "[0-9TZ:-]{1,20}",
            pass_id in "[a-z0-9-]{1,20}",
            size_bytes in 0..i64::MAX,
        ) {
            let metadata = HashMap::from([
                ("published-at".to_string(), published_at.clone()),
                ("pass-id".to_string(), pass_id.clone()),
            ]);

            let version = object_version(Some(&etag), Some(&metadata), Some(size_bytes));

            prop_assert_eq!(version, Ok(ObjectVersion {
                etag,
                published_at: Some(published_at),
                pass_id: Some(pass_id),
                size_bytes: size_bytes as u64,
            }));
        }

        // The pass's index_bytes comes from here.
        #[test]
        fn a_published_version_holds_what_was_sent(
            etag in "[a-f0-9]{1,16}",
            published_at in "[0-9TZ:-]{1,20}",
            pass_id in "[a-z0-9-]{1,20}",
            size_bytes in any::<u64>(),
        ) {
            let metadata = PublishMetadata {
                published_at: published_at.clone(),
                pass_id: pass_id.clone(),
            };

            let version = published_version(Some(&etag), metadata, size_bytes);

            prop_assert_eq!(version, Ok(ObjectVersion {
                etag,
                published_at: Some(published_at),
                pass_id: Some(pass_id),
                size_bytes,
            }));
        }

        // FROM 04-08: only a body of exactly the object's size is a fetch.
        #[test]
        fn a_download_is_whole_only_at_the_objects_exact_size(
            written in 0..64u64,
            object_size in 0..64u64,
        ) {
            prop_assert_eq!(whole_body(written, object_size).is_ok(), written == object_size);
        }

        // D-STZ-40a: the publish gets the time left, and never more than its 35 s.
        #[test]
        fn a_publish_gets_the_limit_it_is_given_up_to_its_budget(
            limit in (1..120_000u64).prop_map(Duration::from_millis),
        ) {
            let settings = publish_settings(limit).expect("time left");

            prop_assert_eq!(the_limit_of(settings), Some(limit.min(THIRTY_FIVE_SECONDS)));
        }

        // RIO-2's shape, over every value the record can take.
        #[test]
        fn a_pass_record_reads_back_as_it_was_written(
            pass_id in "[ -~]{0,20}",
            exit_code in any::<i32>(),
            finished_at in "[ -~]{0,20}",
            published in any::<bool>(),
            index_published_at in prop::option::of("[ -~]{0,20}"),
        ) {
            let record = LastPass {
                pass_id,
                exit_code,
                finished_at,
                published,
                index_published_at,
            };

            let stored = serde_json::to_vec(&StoredPassRecord::from(&record)).expect("json");

            prop_assert_eq!(parse_pass_record(&stored), Ok(record));
        }
    }

    // bypass: the three shapes a check can end in, each a fixed case.
    #[test]
    fn a_probe_passes_on_any_answer_but_a_failure() {
        let version = ObjectVersion {
            etag: "e".to_string(),
            published_at: None,
            pass_id: None,
            size_bytes: 0,
        };
        let unusable = |checked| {
            matches!(
                read_probe_outcome(checked),
                ProbeOutcome::Refused {
                    reason: ProbeRefusalReason::IndexObjectUnusable,
                    ..
                }
            )
        };

        assert!(!unusable(Ok(HeadOutcome::Present(version))));
        assert!(!unusable(Ok(HeadOutcome::Absent)));
        assert!(unusable(Err(IndexObjectError::Refused {
            detail: "no such bucket".to_string()
        })));
        assert!(unusable(Err(IndexObjectError::Unavailable {
            detail: "no answer".to_string()
        })));
    }

    // bypass: the one way an answer can name no version: three fixed absences.
    #[test]
    fn an_answer_without_an_etag_or_a_size_is_not_a_version() {
        assert!(object_version(None, None, Some(1)).is_err());
        assert!(object_version(Some("e"), None, None).is_err());
        assert!(object_version(Some("e"), None, Some(-1)).is_err());
        assert!(object_version(Some("e"), None, Some(0)).is_ok());
    }

    // bypass: one fixed value at the edge of the range the property covers.
    #[test]
    fn a_publish_with_no_time_left_has_no_settings() {
        assert!(publish_settings(Duration::ZERO).is_none());
    }

    // bypass: the publish's other settings are one fixed table row.
    #[test]
    fn a_narrowed_publish_keeps_its_one_attempt() {
        let settings = publish_settings(Duration::from_secs(1)).expect("time left");

        assert_eq!(settings.retry.max_attempts(), 1);
    }

    // bypass: an empty body and a garbled one are the fixed shapes RIO-8 does not send.
    #[test]
    fn a_body_that_is_not_a_record_is_an_error() {
        assert!(parse_pass_record(b"").is_err());
        assert!(parse_pass_record(b"{\"pass_id\":\"p\"}").is_err());
    }

    // bypass: a fixed fact of one built client, read back from the SDK config: no
    // default HTTPS client exists here, and a call with no override cannot retry.
    #[tokio::test]
    async fn the_shared_client_has_an_http_client_and_one_attempt() {
        let client = client_at(&S3Location {
            region: "us-east-1".to_string(),
            endpoint_url: None,
            bucket: "b".to_string(),
            index_key: "k".to_string(),
            last_pass_key: "l".to_string(),
        })
        .await;

        let config = client.config();
        assert!(config.http_client().is_some());
        assert_eq!(config.retry_config().expect("retry").max_attempts(), 1);
    }

    // bypass: a drop guard has two fixed paths.
    #[test]
    fn a_download_that_does_not_finish_takes_its_file_with_it() {
        let scratch = tempfile::tempdir().expect("scratch");
        let kept = scratch.path().join("kept");
        let dropped = scratch.path().join("dropped");
        std::fs::write(&kept, b"whole").expect("a file");
        std::fs::write(&dropped, b"half").expect("a file");

        PartialDownload::at(&kept).finish();
        drop(PartialDownload::at(&dropped));

        assert!(kept.exists());
        assert!(!dropped.exists());
    }
}
