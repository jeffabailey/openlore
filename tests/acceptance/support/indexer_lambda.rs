//! scale-to-zero-hosting — shared acceptance harness (DISTILL 2026-10-27).
//!
//! Included by the `scale_to_zero_*` suites with
//! `#[path = "support/indexer_lambda.rs"] mod indexer_lambda;`. Registered as
//! `[[test]]`s of `crates/openlore-indexer-lambda` (the `cli` package may not
//! link the new crates, check-arch I-3 second axis).
//!
//! Architecture of Reference (`docs/architecture/atdd-infrastructure-policy.md`):
//!
//! | Port | Treatment here |
//! |---|---|
//! | Search, pass and probe entries of `openlore-indexer-lambda` (driving) | REAL composition root, called in-process with a real Lambda event (Function URL payload 2.0; the schedules' `{}`) |
//! | The index copy in the function's `/tmp` (driven internal) | REAL DuckDB file in the instance's scratch directory |
//! | S3 index object + `last-pass.json` (driven external, AWS) | [`FakeIndexBucket`], behind `IndexObjectReadPort` and `IndexObjectWritePort`: versions with ETags, metadata, conditional writes (412), the write probe's key, 404, refusals, outages, damaged objects, a local-write failure; request log |
//! | SSM DID list (driven external, AWS) | [`FakeDidList`]: text, outage; read count |
//! | Clock (non-deterministic) | [`FakeClock`]: manual advance |
//! | stdout events (output) | [`CapturedEvents`]: every JSON line, in order |
//! | PLC directory + authors' PDSes (driven external) | ONE `FakeAtprotoNetwork` (loopback; the TEST-ONLY loopback seam is set) |
//! | The public edge, as the probe sees it (driven external) | [`CannedEdge`] (loopback HTTP, with a request log) or a refusing address |
//!
//! The real-I/O contract of the S3/SSM adapter against moto lives in
//! `scale_to_zero_object_store_real_io.rs` (adapter package).
//!
//! Vocabulary (Mandate-12 SSOT): typed [`Author`]s and [`Claim`]s (the same
//! DIDs and claims as `indexer_network.rs`), [`Exit`], [`Fault`],
//! [`ObjectTrouble`], one [`LambdaWorld`] whose methods are the Given/When
//! steps, and readers on [`PassRun`], [`Reply`] and [`ProbeRun`] for the Then
//! observables. Every observable is port-exposed: the function's answer, its
//! event lines, the fake bucket's versions and request log, and the size of the
//! instance's scratch directory (the `/tmp` budget, D-STZ-10 / open question 2).
//!
//! The request log is a port-visible cost, not a way to read behaviour. A Then
//! counts requests only where the count IS the requirement: SE-4 (D-STZ-10: at
//! most one check per TTL), PA-4 (KPI-IXD-4: the list is read on every pass), and
//! PA-E11 and PR-E3 (a refused event makes no call, after init). Everywhere else
//! the observable answers carry the behaviour.

#![allow(dead_code)]

use std::collections::{BTreeMap, HashMap, HashSet};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use base64::Engine as _;
use chrono::{DateTime, Utc};
use openlore_indexer_lambda::{
    Environment, EventSink, FunctionError, InitRefused, PassDriven, PassFunction, ProbeDriven,
    ProbeFunction, SearchDriven, SearchFunction,
};
use openlore_test_support::{BlueskyAccount, FakeAtprotoNetwork, CLAIM_COLLECTION};
use ports::{
    ClockPort, HeadOutcome, IndexObjectError, IndexObjectReadPort, IndexObjectWritePort, LastPass,
    ObjectVersion, ProbeOutcome, ProbeRefusalReason, PublishMetadata, PublishOutcome,
    RepoDidListError, RepoDidListSourcePort, WriteCondition,
};
use serde_json::{json, Value};

use crate::state_delta::SlotPredicate;

// =============================================================================
// Domain vocabulary
// =============================================================================

/// The people of the journey (same DIDs as `indexer_network.rs`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Author {
    /// bsky.social author; self-attested claims.
    Priya,
    /// Self-hosted author on `pds.volkov.dev`; self-attested claims here.
    Dmitri,
    /// bsky.social author the operator adds to the list later (KPI-IXD-4).
    Tomas,
    /// A DID nobody can resolve.
    Ghost,
}

impl Author {
    pub const fn did(self) -> &'static str {
        match self {
            Author::Priya => "did:plc:priyaraman7x2k",
            Author::Dmitri => "did:plc:dvolkov3m9q",
            Author::Tomas => "did:plc:therrera2v6w",
            Author::Ghost => "did:plc:ghost0000",
        }
    }

    pub const fn handle(self) -> &'static str {
        match self {
            Author::Priya => "priyaraman.bsky.social",
            Author::Dmitri => "dmitri.volkov.dev",
            Author::Tomas => "tomasherrera.bsky.social",
            Author::Ghost => "ghost.invalid",
        }
    }

    /// The PDS host label (`None`: no DID document anywhere).
    pub const fn home(self) -> Option<&'static str> {
        match self {
            Author::Priya | Author::Tomas => Some("morel-us-east-host-bsky-network"),
            Author::Dmitri => Some("pds-volkov-dev"),
            Author::Ghost => None,
        }
    }

    pub const ALL: [Author; 4] = [Author::Priya, Author::Dmitri, Author::Tomas, Author::Ghost];
}

/// One "project embodies philosophy" claim.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Claim {
    pub subject: &'static str,
    pub philosophy: &'static str,
    pub basis_points: i64,
}

impl Claim {
    pub fn object(&self) -> String {
        format!("org.openlore.philosophy.{}", self.philosophy)
    }
}

/// The claims of the journey.
pub mod claims {
    use super::Claim;

    pub const CARGO_PIN_REPRODUCIBLE_BUILDS: Claim = Claim {
        subject: "github:priyaraman/cargo-pin",
        philosophy: "reproducible-builds",
        basis_points: 8200,
    };
    pub const CARGO_PIN_TEST_DRIVEN: Claim = Claim {
        subject: "github:priyaraman/cargo-pin",
        philosophy: "test-driven",
        basis_points: 6400,
    };
    pub const FERRITE_REPRODUCIBLE_BUILDS: Claim = Claim {
        subject: "github:dvolkov/ferrite",
        philosophy: "reproducible-builds",
        basis_points: 7000,
    };
    pub const LOOM_REPRODUCIBLE_BUILDS: Claim = Claim {
        subject: "github:therrera/loom",
        philosophy: "reproducible-builds",
        basis_points: 6900,
    };
}

/// A pass's exit code (ADR-078).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Exit {
    Completed,
    Failed,
    AllSkipped,
}

impl Exit {
    pub const fn code(self) -> i64 {
        match self {
            Exit::Completed => 0,
            Exit::Failed => 2,
            Exit::AllSkipped => 3,
        }
    }
}

/// A fault only a test can provoke: the existing TEST-ONLY
/// `OPENLORE_INDEXER_TEST_FAULT` seam (debug builds only).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fault {
    /// The first pass panics (an existing token).
    FirstPassPanics,
    /// The store's lock is poisoned at the next pass (existing).
    StorePoisoned,
    /// A search instance's store is poisoned right after its first search has
    /// answered, the way a panic under the lock would. The instance starts
    /// normally: the fault cannot be set on a running instance, because the
    /// environment is read at init. NEW token, DISTILL-proposed (UI-5).
    StorePoisonedAfterFirstSearch,
    /// Every search read of the store fails (existing). It acts on reads after
    /// init, as `StorePoisonedAfterFirstSearch` does: the instance starts and
    /// reports healthy, so this is never an init refusal.
    SearchCannotReadTheStore,
    /// Every search read of the store takes longer than the 5 s request budget.
    /// It acts on reads after init, like the fault above. NEW token,
    /// DISTILL-proposed (upstream-issues.md).
    SearchStoreIsSlow,
}

impl Fault {
    pub const fn token(self) -> &'static str {
        match self {
            Fault::FirstPassPanics => "first_pass_panics",
            Fault::StorePoisoned => "store_poisoned",
            Fault::StorePoisonedAfterFirstSearch => "store_poisoned_after_first_search",
            Fault::SearchCannotReadTheStore => "search_store_read_fails",
            Fault::SearchStoreIsSlow => "search_store_slow",
        }
    }
}

/// How the object store misbehaves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectTrouble {
    /// Every request is refused (403-like).
    Refuses,
    /// Every request fails (5xx / unreachable).
    Unavailable,
    /// Only `HEAD` fails; downloads work.
    HeadFails,
    /// `HEAD` never answers. The adapter's 1 s operation timeout is its port
    /// contract (§ Timeouts), so the fake takes that second and then fails.
    HeadStalls,
    /// A download writes half the bytes, then takes far longer than any bound
    /// and fails. Nothing below the root should end it: the 3 s bound covers the
    /// download and the open together, so only the root can apply it (D-STZ-10).
    /// It ends at all so a root with no bound fails a test instead of hanging it.
    DownloadStalls,
    /// Downloads cannot be written locally (no space left in `/tmp`): half
    /// the bytes land, then the write fails.
    ScratchSpaceFull,
    /// Only index downloads are refused; everything else works (a pass can
    /// still record itself, UI-2).
    DownloadsRefused,
    /// Only index downloads fail; everything else works (UI-2).
    DownloadsFail,
    /// Only publishing fails, with a store error rather than a lost update.
    PublishFails,
    /// Only the pass record cannot be written.
    PassRecordCannotBeWritten,
}

/// The Lambda environment variables (one place to rename; DISTILL-proposed
/// names for the Lambda-only ones, existing names for the rest).
pub mod var {
    pub const INDEX_PATH: &str = "OPENLORE_INDEXER_INDEX_PATH";
    pub const REPO_DIDS_FILE: &str = "OPENLORE_INDEXER_REPO_DIDS_FILE";
    pub const PURGE_UNLISTED: &str = "OPENLORE_INDEXER_PURGE_UNLISTED";
    pub const PLC: &str = "OPENLORE_INDEXER_PLC_ENDPOINT";
    pub const LOOPBACK_SEAM: &str = "OPENLORE_INDEXER_ALLOW_LOOPBACK_HTTP";
    pub const PASS_DEADLINE: &str = "OPENLORE_INDEXER_PASS_DEADLINE_SECS";
    pub const MAX_CONCURRENT: &str = "OPENLORE_INDEXER_MAX_CONCURRENT_FETCHES";
    pub const PER_DID_TIMEOUT: &str = "OPENLORE_INDEXER_PER_DID_TIMEOUT_SECS";
    pub const DUCKDB_MEMORY: &str = "OPENLORE_INDEXER_DUCKDB_MEMORY_LIMIT_MB";
    pub const DUCKDB_THREADS: &str = "OPENLORE_INDEXER_DUCKDB_THREADS";
    pub const TEST_FAULT: &str = "OPENLORE_INDEXER_TEST_FAULT";
    /// Lambda-only (DISTILL-proposed).
    pub const FRESHNESS_TTL: &str = "OPENLORE_INDEXER_LAMBDA_FRESHNESS_TTL_SECS";
    pub const PROBE_BASE_URL: &str = "OPENLORE_INDEXER_LAMBDA_PROBE_BASE_URL";
    pub const LAST_PASS_STALE: &str = "OPENLORE_INDEXER_LAMBDA_LAST_PASS_STALE_SECS";
    pub const INDEX_KEY: &str = "OPENLORE_INDEXER_LAMBDA_INDEX_KEY";
    pub const LAST_PASS_KEY: &str = "OPENLORE_INDEXER_LAMBDA_LAST_PASS_KEY";
}

/// The write check's marker key. A literal here: the harness links no adapter.
pub const WRITE_CHECK_MARKER_KEY: &str = "index/.write-probe";

/// Routes of the public surface.
pub const SEARCH_PATH: &str = "/xrpc/org.openlore.appview.searchClaims";
pub const HEALTH_PATH: &str = "/healthz";

/// When every scenario starts: Tuesday 2026-10-27 14:00:00 UTC.
pub const START: &str = "2026-10-27T14:00:00Z";

/// Run one outline's rows, and report every row that fails rather than stopping
/// at the first, so one failure does not hide the others.
pub fn each_row<T: std::fmt::Debug>(rows: impl IntoIterator<Item = T>, row: impl Fn(&T)) {
    let failed: Vec<String> = rows
        .into_iter()
        .filter_map(|case| {
            catch_unwind(AssertUnwindSafe(|| row(&case)))
                .err()
                .map(|cause| format!("{case:?}: {}", panic_text(cause.as_ref())))
        })
        .collect();
    assert!(
        failed.is_empty(),
        "{} row(s) failed:\n{}",
        failed.len(),
        failed.join("\n")
    );
}

fn panic_text(cause: &(dyn std::any::Any + Send)) -> String {
    cause
        .downcast_ref::<String>()
        .cloned()
        .or_else(|| cause.downcast_ref::<&str>().map(|s| (*s).to_string()))
        .unwrap_or_else(|| "a panic with no message".to_string())
}

// =============================================================================
// Records as the author's PDS holds them (self-attested, ADR-071)
// =============================================================================

pub fn self_attested_value(author: &str, claim: Claim) -> Value {
    json!({
        "$type": CLAIM_COLLECTION,
        "subject": claim.subject,
        "predicate": "embodiesPhilosophy",
        "object": claim.object(),
        "evidence": [format!("https://{}", claim.subject.replace("github:", "github.com/"))],
        "confidence": claim.basis_points,
        "author": author,
        "composedAt": "2026-10-27T13:02:11Z",
    })
}

/// The CID an honest PDS keys a self-attested record under (`CID == rkey`).
pub fn recomputed_cid(value: &Value) -> String {
    let text = |k: &str| {
        value
            .get(k)
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string()
    };
    let claim = claim_domain::UnsignedClaim {
        subject: text("subject"),
        predicate: text("predicate"),
        object: text("object"),
        evidence: value
            .get("evidence")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(|e| e.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default(),
        confidence: value
            .get("confidence")
            .and_then(claim_domain::Confidence::from_wire)
            .unwrap_or_else(|| claim_domain::Confidence::from_basis_points(-1)),
        author_did: claim_domain::Did(text("author")),
        composed_at: text("composedAt"),
        references: Vec::new(),
        reason: None,
    };
    let bytes = claim_domain::canonicalize(&claim).expect("canonicalize a claim value");
    claim_domain::compute_cid(&bytes).0
}

// =============================================================================
// Fakes of the external ports
// =============================================================================

/// One stored version of the index object.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredVersion {
    pub etag: String,
    pub bytes: Vec<u8>,
    pub published_at: Option<String>,
    pub pass_id: Option<String>,
}

/// A request the bucket saw.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BucketRequest {
    Head,
    Get,
    Publish,
    PutLastPass,
    GetLastPass,
    WriteProbe,
}

/// One ordered log across the bucket and the event lines, so a scenario can pin
/// the order of a publish, a record and a summary (D-STZ-40).
#[derive(Default)]
pub struct Timeline {
    steps: Mutex<Vec<String>>,
}

impl Timeline {
    fn add(&self, step: String) {
        self.steps.lock().unwrap().push(step);
    }

    pub fn mark(&self) -> usize {
        self.steps.lock().unwrap().len()
    }

    /// Where `step` first appears after `mark`.
    pub fn position_since(&self, mark: usize, step: &str) -> Option<usize> {
        self.steps.lock().unwrap()[mark..]
            .iter()
            .position(|s| s == step)
    }

    pub fn since(&self, mark: usize) -> Vec<String> {
        self.steps.lock().unwrap()[mark..].to_vec()
    }
}

/// The steps a scenario orders.
pub mod step {
    pub const PUBLISH: &str = "bucket: publish";
    pub const RECORD: &str = "bucket: record";
    pub const SUMMARY: &str = "event: indexer.ingest.pass_summary";
}

#[derive(Default)]
struct BucketState {
    current: Option<StoredVersion>,
    history: Vec<StoredVersion>,
    last_pass: Option<LastPass>,
    next_etag: u64,
    requests: Vec<BucketRequest>,
    trouble: Option<ObjectTrouble>,
    last_pass_unreadable: bool,
    another_writer_before_next_publish: bool,
    write_probe_versions: usize,
    ignores_write_conditions: bool,
    publish_limits: Vec<Duration>,
}

impl BucketState {
    fn new_etag(&mut self) -> String {
        self.next_etag += 1;
        format!("\"etag-{:04}\"", self.next_etag)
    }

    fn store(
        &mut self,
        bytes: Vec<u8>,
        published_at: Option<String>,
        pass_id: Option<String>,
    ) -> StoredVersion {
        let version = StoredVersion {
            etag: self.new_etag(),
            bytes,
            published_at,
            pass_id,
        };
        self.current = Some(version.clone());
        self.history.push(version.clone());
        version
    }

    fn refusal(&self, request: BucketRequest) -> Option<IndexObjectError> {
        match (self.trouble, request) {
            (Some(ObjectTrouble::Refuses), _)
            | (Some(ObjectTrouble::DownloadsRefused), BucketRequest::Get) => {
                Some(IndexObjectError::Refused {
                    detail: "access denied (fake)".to_string(),
                })
            }
            (Some(ObjectTrouble::Unavailable), _)
            | (Some(ObjectTrouble::HeadFails), BucketRequest::Head)
            | (Some(ObjectTrouble::DownloadsFail), BucketRequest::Get)
            | (Some(ObjectTrouble::PublishFails), BucketRequest::Publish)
            | (Some(ObjectTrouble::PassRecordCannotBeWritten), BucketRequest::PutLastPass) => {
                Some(IndexObjectError::Unavailable {
                    detail: "service unavailable (fake)".to_string(),
                })
            }
            _ => None,
        }
    }
}

fn version_of(stored: &StoredVersion) -> ObjectVersion {
    ObjectVersion {
        etag: stored.etag.clone(),
        published_at: stored.published_at.clone(),
        pass_id: stored.pass_id.clone(),
        size_bytes: stored.bytes.len() as u64,
    }
}

/// The index bucket: `index/index.duckdb` (versioned) + `index/last-pass.json`.
#[derive(Default)]
pub struct FakeIndexBucket {
    state: Mutex<BucketState>,
    timeline: Arc<Timeline>,
}

impl FakeIndexBucket {
    fn on(timeline: Arc<Timeline>) -> Self {
        Self {
            state: Mutex::default(),
            timeline,
        }
    }

    // ------------------------------------------------------------- postures

    pub fn has_trouble(&self, trouble: ObjectTrouble) {
        self.state.lock().unwrap().trouble = Some(trouble);
    }

    pub fn recovers(&self) {
        self.state.lock().unwrap().trouble = None;
    }

    /// Another writer publishes between this pass's read and its publish
    /// (a lost-update attempt, D-STZ-6).
    pub fn another_writer_publishes_during_the_next_pass(&self) {
        self.state
            .lock()
            .unwrap()
            .another_writer_before_next_publish = true;
    }

    /// Somebody replaces the index with bytes that are not an index, as many of
    /// them as the current one, so a leaked download would show in `/tmp`.
    pub fn holds_a_damaged_index(&self) {
        let mut state = self.state.lock().unwrap();
        let mut bytes = b"this is not a DuckDB file".to_vec();
        let size = state.current.as_ref().map_or(0, |v| v.bytes.len());
        bytes.resize(size.max(bytes.len()), b'x');
        state.store(
            bytes,
            Some("2026-10-27T13:45:00Z".to_string()),
            Some("damaged".to_string()),
        );
    }

    /// The write probe's key is already in the bucket (D-STZ-39). Separates what
    /// init does from what the no-publish arm does (U-STZ-8).
    pub fn holds_the_write_check_marker(&self) {
        let mut state = self.state.lock().unwrap();
        state.write_probe_versions = state.write_probe_versions.max(1);
    }

    /// From now on the store accepts a conditional write it should refuse.
    pub fn ignores_write_conditions(&self) {
        self.state.lock().unwrap().ignores_write_conditions = true;
    }

    /// `deploy.sh lambda reset-index`: a delete marker on the index object.
    pub fn operator_resets_the_index(&self) {
        self.state.lock().unwrap().current = None;
    }

    /// GIVEN `last-pass.json` records this pass (a precondition for the probe).
    pub fn records_last_pass(&self, exit: Exit, finished_at: DateTime<Utc>) {
        self.records_a_pass_of_the_index(exit, finished_at, None);
    }

    /// GIVEN `last-pass.json` records this pass and the index it saw (U-STZ-6).
    pub fn records_a_pass_of_the_index(
        &self,
        exit: Exit,
        finished_at: DateTime<Utc>,
        index_published_at: Option<&str>,
    ) {
        self.state.lock().unwrap().last_pass = Some(LastPass {
            pass_id: "seeded-pass".to_string(),
            exit_code: exit.code() as i32,
            finished_at: finished_at.to_rfc3339(),
            published: false,
            index_published_at: index_published_at.map(str::to_string),
        });
    }

    /// GIVEN `last-pass.json` holds a time nobody can read.
    pub fn holds_a_garbled_pass_record(&self) {
        self.state.lock().unwrap().last_pass = Some(LastPass {
            pass_id: "seeded-pass".to_string(),
            exit_code: 0,
            finished_at: "not a time".to_string(),
            published: false,
            index_published_at: None,
        });
    }

    pub fn last_pass_cannot_be_read(&self) {
        self.state.lock().unwrap().last_pass_unreadable = true;
    }

    // ------------------------------------------------------------ observers

    pub fn current(&self) -> Option<StoredVersion> {
        self.state.lock().unwrap().current.clone()
    }

    pub fn versions(&self) -> Vec<StoredVersion> {
        self.state.lock().unwrap().history.clone()
    }

    pub fn last_pass(&self) -> Option<LastPass> {
        self.state.lock().unwrap().last_pass.clone()
    }

    pub fn requests(&self) -> Vec<BucketRequest> {
        self.state.lock().unwrap().requests.clone()
    }

    /// The limit each publish was handed, in order.
    pub fn publish_limits(&self) -> Vec<Duration> {
        self.state.lock().unwrap().publish_limits.clone()
    }

    /// How many versions of the write probe's key exist (0 or 1 in a store that
    /// honours conditions; a store that ignores them gains 2 per run).
    pub fn write_check_marker_versions(&self) -> usize {
        self.state.lock().unwrap().write_probe_versions
    }
}

#[async_trait]
impl IndexObjectReadPort for FakeIndexBucket {
    fn probe(&self) -> ProbeOutcome {
        ProbeOutcome::Ok
    }

    async fn head(&self) -> Result<HeadOutcome, IndexObjectError> {
        let stalled = {
            let mut state = self.state.lock().unwrap();
            state.requests.push(BucketRequest::Head);
            if let Some(err) = state.refusal(BucketRequest::Head) {
                return Err(err);
            }
            state.trouble == Some(ObjectTrouble::HeadStalls)
        };
        if stalled {
            tokio::time::sleep(Duration::from_secs(1)).await;
            return Err(IndexObjectError::Unavailable {
                detail: "operation timed out (fake)".to_string(),
            });
        }
        let state = self.state.lock().unwrap();
        Ok(state
            .current
            .as_ref()
            .map_or(HeadOutcome::Absent, |v| HeadOutcome::Present(version_of(v))))
    }

    async fn fetch_to(&self, dest: &Path) -> Result<Option<ObjectVersion>, IndexObjectError> {
        let (stored, trouble) = {
            let mut state = self.state.lock().unwrap();
            state.requests.push(BucketRequest::Get);
            if let Some(err) = state.refusal(BucketRequest::Get) {
                return Err(err);
            }
            (state.current.clone(), state.trouble)
        };
        let Some(stored) = stored else {
            return Ok(None);
        };
        if trouble == Some(ObjectTrouble::DownloadStalls) {
            let half = &stored.bytes[..stored.bytes.len() / 2];
            let _ = std::fs::write(dest, half);
            tokio::time::sleep(Duration::from_secs(30)).await;
            return Err(IndexObjectError::Unavailable {
                detail: "download stalled (fake)".to_string(),
            });
        }
        if trouble == Some(ObjectTrouble::ScratchSpaceFull) {
            let half = &stored.bytes[..stored.bytes.len() / 2];
            let _ = std::fs::write(dest, half);
            return Err(IndexObjectError::LocalWrite {
                detail: "no space left on device (fake)".to_string(),
            });
        }
        std::fs::write(dest, &stored.bytes).map_err(|err| IndexObjectError::LocalWrite {
            detail: err.to_string(),
        })?;
        Ok(Some(version_of(&stored)))
    }

    async fn get_last_pass(&self) -> Result<Option<LastPass>, IndexObjectError> {
        let mut state = self.state.lock().unwrap();
        state.requests.push(BucketRequest::GetLastPass);
        if let Some(err) = state.refusal(BucketRequest::GetLastPass) {
            return Err(err);
        }
        if state.last_pass_unreadable {
            return Err(IndexObjectError::Unavailable {
                detail: "last-pass.json unreadable (fake)".to_string(),
            });
        }
        Ok(state.last_pass.clone())
    }
}

#[async_trait]
impl IndexObjectWritePort for FakeIndexBucket {
    /// The write probe (D-STZ-39), as the port sees it. A store that honours the
    /// condition keeps one marker. A store that ignores it takes both PUTs, on a
    /// first run or any later one, and that alone is a refusal. An outage is not:
    /// the pass runs and fails with its own cause (D-STZ-41, UI-17). The two-PUT
    /// classification itself is WP-1.
    fn probe(&self) -> ProbeOutcome {
        let mut state = self.state.lock().unwrap();
        state.requests.push(BucketRequest::WriteProbe);
        if state.refusal(BucketRequest::WriteProbe).is_some() {
            return ProbeOutcome::Ok;
        }
        match (state.write_probe_versions, state.ignores_write_conditions) {
            (0, false) => {
                state.write_probe_versions = 1;
                ProbeOutcome::Ok
            }
            (_, false) => ProbeOutcome::Ok,
            (_, true) => {
                state.write_probe_versions += 2;
                ProbeOutcome::Refused {
                    reason: ProbeRefusalReason::IndexObjectWriteConditionIgnored,
                    detail: "the store accepted both writes of the write check (fake)".to_string(),
                    structured: json!({"key": "index/.write-probe"}),
                }
            }
        }
    }

    async fn publish_if_match(
        &self,
        src: &Path,
        condition: WriteCondition,
        metadata: PublishMetadata,
        limit: Duration,
    ) -> Result<PublishOutcome, IndexObjectError> {
        self.state.lock().unwrap().publish_limits.push(limit);
        if limit.is_zero() {
            return Err(IndexObjectError::Unavailable {
                detail: "no time left to publish (fake)".to_string(),
            });
        }
        let bytes = std::fs::read(src).map_err(|err| IndexObjectError::LocalWrite {
            detail: format!("read the file to publish: {err}"),
        })?;
        self.timeline.add(step::PUBLISH.to_string());
        let mut state = self.state.lock().unwrap();
        state.requests.push(BucketRequest::Publish);
        if let Some(err) = state.refusal(BucketRequest::Publish) {
            return Err(err);
        }
        if std::mem::take(&mut state.another_writer_before_next_publish) {
            let theirs = state
                .current
                .as_ref()
                .map(|v| v.bytes.clone())
                .unwrap_or_default();
            state.store(
                theirs,
                Some("2026-10-27T14:00:30Z".to_string()),
                Some("another-writer".to_string()),
            );
        }
        let holds = match (&condition, &state.current) {
            (WriteCondition::IfMatch(etag), Some(current)) => *etag == current.etag,
            (WriteCondition::IfMatch(_), None) => false,
            (WriteCondition::IfNoneMatch, current) => current.is_none(),
        };
        if !holds {
            return Ok(PublishOutcome::Conflict);
        }
        let stored = state.store(bytes, Some(metadata.published_at), Some(metadata.pass_id));
        Ok(PublishOutcome::Published(version_of(&stored)))
    }

    async fn put_last_pass(&self, last: &LastPass) -> Result<(), IndexObjectError> {
        self.timeline.add(step::RECORD.to_string());
        let mut state = self.state.lock().unwrap();
        state.requests.push(BucketRequest::PutLastPass);
        if let Some(err) = state.refusal(BucketRequest::PutLastPass) {
            return Err(err);
        }
        state.last_pass = Some(last.clone());
        Ok(())
    }
}

/// A write port for a no-publish run. The write probe still reaches the bucket,
/// because init runs it whatever the event (U-STZ-8). Any object write fails the
/// test: it is counted, because a panic here could be caught by the pass.
pub struct ForbiddenWrites {
    bucket: Arc<FakeIndexBucket>,
    attempts: AtomicUsize,
}

impl ForbiddenWrites {
    pub fn around(bucket: Arc<FakeIndexBucket>) -> Self {
        Self {
            bucket,
            attempts: AtomicUsize::new(0),
        }
    }

    fn attempted(&self) -> IndexObjectError {
        self.attempts.fetch_add(1, Ordering::SeqCst);
        IndexObjectError::Refused {
            detail: "a no-publish run has no write path (test)".to_string(),
        }
    }

    pub fn attempts(&self) -> usize {
        self.attempts.load(Ordering::SeqCst)
    }
}

#[async_trait]
impl IndexObjectWritePort for ForbiddenWrites {
    fn probe(&self) -> ProbeOutcome {
        IndexObjectWritePort::probe(self.bucket.as_ref())
    }

    async fn publish_if_match(
        &self,
        _src: &Path,
        _condition: WriteCondition,
        _metadata: PublishMetadata,
        _limit: Duration,
    ) -> Result<PublishOutcome, IndexObjectError> {
        Err(self.attempted())
    }

    async fn put_last_pass(&self, _last: &LastPass) -> Result<(), IndexObjectError> {
        Err(self.attempted())
    }
}

/// The operator's DID list in the parameter store.
#[derive(Default)]
pub struct FakeDidList {
    text: Mutex<String>,
    failing: Mutex<bool>,
    reads: AtomicUsize,
}

impl FakeDidList {
    pub fn saves(&self, text: &str) {
        *self.text.lock().unwrap() = text.to_string();
    }

    pub fn fails(&self, failing: bool) {
        *self.failing.lock().unwrap() = failing;
    }

    pub fn reads(&self) -> usize {
        self.reads.load(Ordering::SeqCst)
    }
}

#[async_trait]
impl RepoDidListSourcePort for FakeDidList {
    fn probe(&self) -> ProbeOutcome {
        ProbeOutcome::Ok
    }

    async fn current(&self) -> Result<String, RepoDidListError> {
        self.reads.fetch_add(1, Ordering::SeqCst);
        if *self.failing.lock().unwrap() {
            return Err(RepoDidListError::Unavailable {
                detail: "parameter store unavailable (fake)".to_string(),
            });
        }
        Ok(self.text.lock().unwrap().clone())
    }
}

/// A clock the scenario moves by hand.
pub struct FakeClock {
    now: Mutex<DateTime<Utc>>,
}

impl FakeClock {
    pub fn at(start: &str) -> Self {
        Self {
            now: Mutex::new(start.parse().expect("a fixed RFC 3339 instant")),
        }
    }

    pub fn advance(&self, by: Duration) {
        let mut now = self.now.lock().unwrap();
        *now += chrono::Duration::from_std(by).expect("a small duration");
    }

    pub fn now(&self) -> DateTime<Utc> {
        *self.now.lock().unwrap()
    }
}

impl ClockPort for FakeClock {
    fn probe(&self) -> ProbeOutcome {
        ProbeOutcome::Ok
    }

    fn now_utc(&self) -> DateTime<Utc> {
        self.now()
    }
}

/// Every JSON line the functions wrote, in order.
#[derive(Default)]
pub struct CapturedEvents {
    lines: Mutex<Vec<String>>,
    timeline: Arc<Timeline>,
}

impl EventSink for CapturedEvents {
    fn line(&self, line: &str) {
        self.lines.lock().unwrap().push(line.to_string());
        if let Ok(event) = serde_json::from_str::<Value>(line) {
            let name = event["event"].as_str().unwrap_or("?");
            self.timeline.add(format!("event: {name}"));
        }
    }
}

impl CapturedEvents {
    fn on(timeline: Arc<Timeline>) -> Self {
        Self {
            lines: Mutex::default(),
            timeline,
        }
    }

    pub fn mark(&self) -> usize {
        self.lines.lock().unwrap().len()
    }

    pub fn since(&self, mark: usize) -> Vec<Value> {
        self.lines.lock().unwrap()[mark..]
            .iter()
            .filter_map(|l| serde_json::from_str(l).ok())
            .collect()
    }

    pub fn raw(&self) -> Vec<String> {
        self.lines.lock().unwrap().clone()
    }
}

/// Events with this `event` name.
pub fn named(events: &[Value], name: &str) -> Vec<Value> {
    events
        .iter()
        .filter(|e| e["event"] == name)
        .cloned()
        .collect()
}

// =============================================================================
// Lambda events and answers
// =============================================================================

/// A Function URL request event, payload format 2.0, as Lambda delivers it.
pub fn function_url_event(method: &str, path: &str, client_ip: &str, body: &[u8]) -> Value {
    json!({
        "version": "2.0",
        "routeKey": "$default",
        "rawPath": path,
        "rawQueryString": "",
        "headers": {
            "content-type": "application/json",
            "x-forwarded-for": client_ip,
            "x-forwarded-proto": "https",
            "host": "abc123.lambda-url.us-east-1.on.aws",
        },
        "requestContext": {
            "accountId": "anonymous",
            "apiId": "abc123",
            "domainName": "abc123.lambda-url.us-east-1.on.aws",
            "domainPrefix": "abc123",
            "http": {
                "method": method,
                "path": path,
                "protocol": "HTTP/1.1",
                "sourceIp": client_ip,
                "userAgent": "openlore/0.0.1",
            },
            "requestId": "c6af9ac6-7b61-11e6-9a41-93e8deadbeef",
            "routeKey": "$default",
            "stage": "$default",
            "time": "27/Oct/2026:14:00:00 +0000",
            "timeEpoch": 1_793_109_600_000_u64,
        },
        "body": String::from_utf8_lossy(body),
        "isBase64Encoded": false,
    })
}

/// What both schedules deliver: they set `input = "{}"` (U-STZ-7), so this is
/// the whole event, not the EventBridge envelope. `{}` is a normal pass and a
/// normal probe, and the pass refuses anything richer (D-STZ-44).
pub fn scheduled_event() -> Value {
    json!({})
}

/// What the deploy's pre-flip pass gate sends (D-STZ-44).
pub fn no_publish_event() -> Value {
    json!({"mode": "no-publish"})
}

/// A search request body.
pub fn search_body(philosophy: &str) -> Vec<u8> {
    json!({"dimension": "object", "value": format!("org.openlore.philosophy.{philosophy}")})
        .to_string()
        .into_bytes()
}

/// A valid, non-empty search for reproducible builds, padded to exactly `total`
/// bytes (an unknown field, which the request type ignores).
pub fn search_body_of_size(total: usize) -> Vec<u8> {
    let value = "org.openlore.philosophy.reproducible-builds";
    let skeleton = json!({"dimension": "object", "pad": "", "value": value}).to_string();
    let pad = total.saturating_sub(skeleton.len());
    json!({"dimension": "object", "pad": "x".repeat(pad), "value": value})
        .to_string()
        .into_bytes()
}

/// A search with a value of `len` bytes.
pub fn search_body_with_value_of(len: usize) -> Vec<u8> {
    json!({"dimension": "object", "value": "v".repeat(len)})
        .to_string()
        .into_bytes()
}

/// A Function URL answer.
#[derive(Debug, Clone)]
pub struct Reply {
    pub status: u16,
    pub headers: BTreeMap<String, String>,
    pub body: Vec<u8>,
}

impl Reply {
    pub fn from_function_url_response(response: &Value) -> Self {
        let status = response["statusCode"].as_u64().unwrap_or(0) as u16;
        let headers = response["headers"]
            .as_object()
            .map(|h| {
                h.iter()
                    .map(|(k, v)| (k.to_ascii_lowercase(), v.as_str().unwrap_or("").to_string()))
                    .collect()
            })
            .unwrap_or_default();
        let text = response["body"].as_str().unwrap_or("");
        let body = if response["isBase64Encoded"].as_bool().unwrap_or(false) {
            base64::engine::general_purpose::STANDARD
                .decode(text)
                .expect("a base64 body")
        } else {
            text.as_bytes().to_vec()
        };
        Self {
            status,
            headers,
            body,
        }
    }

    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }

    pub fn json(&self) -> Value {
        serde_json::from_slice(&self.body).unwrap_or(Value::Null)
    }

    /// The (author DID, subject) of every result row.
    pub fn results(&self) -> Vec<(String, String)> {
        self.json()["results"]
            .as_array()
            .map(|rows| {
                rows.iter()
                    .map(|r| {
                        (
                            r["author_did"].as_str().unwrap_or("").to_string(),
                            r["subject"].as_str().unwrap_or("").to_string(),
                        )
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn subjects_by(&self, author: Author) -> Vec<String> {
        self.results()
            .into_iter()
            .filter(|(did, _)| did.starts_with(author.did()))
            .map(|(_, subject)| subject)
            .collect()
    }
}

// =============================================================================
// The world (Given / When steps)
// =============================================================================

/// One hermetic world: the authors' PDSes, the index bucket, the DID list,
/// the clock, the captured events, and a scratch directory per function
/// instance (its `/tmp`).
pub struct LambdaWorld {
    pub net: FakeAtprotoNetwork,
    pub bucket: Arc<FakeIndexBucket>,
    pub did_list: Arc<FakeDidList>,
    pub clock: Arc<FakeClock>,
    pub events: Arc<CapturedEvents>,
    pub timeline: Arc<Timeline>,
    scratch: tempfile::TempDir,
    instances: AtomicUsize,
    settings: Mutex<BTreeMap<String, String>>,
    runtime: tokio::runtime::Runtime,
}

impl LambdaWorld {
    /// GIVEN every cast author on their PDS, an EMPTY index bucket, and the
    /// operator's DID list naming `listed`.
    pub fn listing(listed: &[Author]) -> Self {
        let accounts = Author::ALL
            .iter()
            .filter_map(|a| {
                a.home()
                    .map(|h| BlueskyAccount::new(a.handle(), a.did(), h))
            })
            .collect();
        let timeline = Arc::new(Timeline::default());
        let world = Self {
            net: FakeAtprotoNetwork::start(accounts),
            bucket: Arc::new(FakeIndexBucket::on(timeline.clone())),
            did_list: Arc::new(FakeDidList::default()),
            clock: Arc::new(FakeClock::at(START)),
            events: Arc::new(CapturedEvents::on(timeline.clone())),
            timeline,
            scratch: tempfile::tempdir().expect("scratch root"),
            instances: AtomicUsize::new(0),
            settings: Mutex::new(BTreeMap::new()),
            runtime: tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("a test runtime"),
        };
        world.operator_lists(listed);
        world
    }

    /// GIVEN `author` published these claims (self-attested) on their PDS.
    pub fn publishes(&self, author: Author, claims: &[Claim]) -> Vec<String> {
        claims
            .iter()
            .map(|c| {
                let value = self_attested_value(author.did(), *c);
                let key = recomputed_cid(&value);
                self.net
                    .seed_record(author.did(), CLAIM_COLLECTION, &key, value);
                key
            })
            .collect()
    }

    /// The operator saves the DID list naming exactly `authors` (no deploy).
    pub fn operator_lists(&self, authors: &[Author]) {
        let text: Vec<&str> = authors.iter().map(|a| a.did()).collect();
        self.did_list.saves(&text.join("\n"));
    }

    pub fn operator_saves_list_text(&self, text: &str) {
        self.did_list.saves(text);
    }

    pub fn parameter_store_fails(&self, failing: bool) {
        self.did_list.fails(failing);
    }

    /// An author's PDS stops answering.
    pub fn pds_goes_down(&self, author: Author) {
        if let Some(host) = author.home() {
            self.net.set_host_reachable(host, false);
        }
    }

    pub fn time_passes(&self, by: Duration) {
        self.clock.advance(by);
    }

    /// The index cannot store `author`'s claims: a regular file sits where the
    /// author's artifact directory belongs in the pass instance's scratch
    /// directory (a real store-write failure, as in indexer-deployment PS-17).
    pub fn index_cannot_store_claims_of(&self, pass: &PassInstance<'_>, author: Author) {
        let root = pass.scratch.join("indexed_claims");
        std::fs::create_dir_all(&root).expect("indexed_claims");
        for identity in [
            author.did().to_string(),
            format!("{}#org.openlore.application", author.did()),
        ] {
            std::fs::write(root.join(identity.replace(':', "_")), b"not a dir")
                .expect("a regular file where the author's artifacts go");
        }
    }

    /// Set a variable in every function's environment from now on.
    pub fn setting(&self, variable: &str, value: &str) {
        self.settings
            .lock()
            .unwrap()
            .insert(variable.to_string(), value.to_string());
    }

    pub fn with_fault(&self, fault: Fault) {
        self.setting(var::TEST_FAULT, fault.token());
    }

    /// Instances that start from now on have no fault.
    pub fn without_a_fault(&self) {
        self.settings.lock().unwrap().remove(var::TEST_FAULT);
    }

    // ------------------------------------------------------------ instances

    fn new_scratch(&self, kind: &str) -> PathBuf {
        let n = self.instances.fetch_add(1, Ordering::SeqCst);
        let dir = self.scratch.path().join(format!("{kind}-{n:02}"));
        std::fs::create_dir_all(&dir).expect("instance scratch dir");
        dir
    }

    fn environment(&self, scratch: &Path) -> Environment {
        let mut env = BTreeMap::from([
            (
                var::INDEX_PATH.to_string(),
                scratch.join("index.duckdb").display().to_string(),
            ),
            (
                var::REPO_DIDS_FILE.to_string(),
                scratch
                    .join("config")
                    .join("repo-dids")
                    .display()
                    .to_string(),
            ),
            (var::PURGE_UNLISTED.to_string(), "1".to_string()),
            (var::PLC.to_string(), self.net.directory_url().to_string()),
            (var::LOOPBACK_SEAM.to_string(), "1".to_string()),
            (var::PASS_DEADLINE.to_string(), "780".to_string()),
            (var::MAX_CONCURRENT.to_string(), "1".to_string()),
            (var::PER_DID_TIMEOUT.to_string(), "10".to_string()),
            (var::DUCKDB_MEMORY.to_string(), "48".to_string()),
            (var::DUCKDB_THREADS.to_string(), "1".to_string()),
            (var::FRESHNESS_TTL.to_string(), "60".to_string()),
            (var::LAST_PASS_STALE.to_string(), "2700".to_string()),
            (var::INDEX_KEY.to_string(), "index/index.duckdb".to_string()),
            (
                var::LAST_PASS_KEY.to_string(),
                "index/last-pass.json".to_string(),
            ),
        ]);
        env.extend(self.settings.lock().unwrap().clone());
        Environment(env)
    }

    fn pass_driven(&self, index_write: Arc<dyn IndexObjectWritePort>) -> PassDriven {
        PassDriven {
            index_read: self.bucket.clone(),
            index_write,
            repo_dids: self.did_list.clone(),
            clock: self.clock.clone(),
            events: self.events.clone(),
        }
    }

    fn search_driven(&self) -> SearchDriven {
        SearchDriven {
            index_object: self.bucket.clone(),
            clock: self.clock.clone(),
            events: self.events.clone(),
        }
    }

    fn probe_driven(&self) -> ProbeDriven {
        ProbeDriven {
            index_object: self.bucket.clone(),
            clock: self.clock.clone(),
            events: self.events.clone(),
        }
    }

    fn pass_instance_on(
        &self,
        index_write: Arc<dyn IndexObjectWritePort>,
    ) -> Result<PassInstance<'_>, InitRefused> {
        let scratch = self.new_scratch("pass");
        let env = self.environment(&scratch);
        self.runtime
            .block_on(PassFunction::init(&env, self.pass_driven(index_write)))
            .map(|function| PassInstance {
                world: self,
                function,
                scratch,
            })
    }

    /// A pass function instance starts cold; `Err` when init is refused.
    pub fn pass_instance_starts(&self) -> Result<PassInstance<'_>, InitRefused> {
        self.pass_instance_on(self.bucket.clone())
    }

    /// A pass function instance starts cold (it must start).
    pub fn cold_pass_instance(&self) -> PassInstance<'_> {
        self.pass_instance_starts()
            .unwrap_or_else(|refused| panic!("the pass function starts: {refused:?}"))
    }

    /// A pass instance whose write port fails the test on any object write.
    pub fn cold_pass_instance_forbidden_to_write(
        &self,
    ) -> (PassInstance<'_>, Arc<ForbiddenWrites>) {
        let writes = Arc::new(ForbiddenWrites::around(self.bucket.clone()));
        let instance = self
            .pass_instance_on(writes.clone())
            .unwrap_or_else(|refused| panic!("the pass function starts: {refused:?}"));
        (instance, writes)
    }

    /// A search instance starts cold; `Err` when init is refused.
    pub fn search_instance_starts(&self) -> Result<SearchInstance<'_>, InitRefused> {
        let scratch = self.new_scratch("search");
        let env = self.environment(&scratch);
        self.runtime
            .block_on(SearchFunction::init(&env, self.search_driven()))
            .map(|function| SearchInstance {
                world: self,
                function,
                scratch,
            })
    }

    /// A search instance starts cold (it must start).
    pub fn cold_search_instance(&self) -> SearchInstance<'_> {
        self.search_instance_starts()
            .unwrap_or_else(|refused| panic!("the search function starts: {refused:?}"))
    }

    /// A probe instance starts cold; `Err` when init is refused.
    pub fn probe_instance_starts(&self, edge_url: &str) -> Result<ProbeInstance<'_>, InitRefused> {
        self.setting(var::PROBE_BASE_URL, edge_url);
        let scratch = self.new_scratch("probe");
        let env = self.environment(&scratch);
        self.runtime
            .block_on(ProbeFunction::init(&env, self.probe_driven()))
            .map(|function| ProbeInstance {
                world: self,
                function,
            })
    }

    /// A probe instance starts cold, probing the edge at `edge_url`.
    pub fn cold_probe_instance(&self, edge_url: &str) -> ProbeInstance<'_> {
        self.probe_instance_starts(edge_url)
            .unwrap_or_else(|refused| panic!("the probe function starts: {refused:?}"))
    }

    // --------------------------------------------------------- shared steps

    /// GIVEN a scheduled pass already published the index (asserted).
    pub fn given_a_pass_published_the_index(&self) {
        let run = self.cold_pass_instance().schedule_fires();
        run.assert_exit(Exit::Completed);
        assert!(
            run.published(),
            "the precondition pass published: {}",
            run.dump()
        );
    }

    // ------------------------------------------------------------- universe

    /// The port-exposed observable state of a pass (Mandate 8 universe).
    pub fn pass_universe(&self) -> HashMap<String, Value> {
        let all = self.events.since(0);
        let mut universe = self.bucket_universe();
        universe.extend([
            (
                "events.pass_summary.count".to_string(),
                json!(named(&all, "indexer.ingest.pass_summary").len()),
            ),
            (
                "events.lambda.published.count".to_string(),
                json!(named(&all, "indexer.lambda.published").len()),
            ),
            (
                "events.lambda.publish_conflict.count".to_string(),
                json!(named(&all, "indexer.lambda.publish_conflict").len()),
            ),
        ]);
        universe
    }

    pub fn pass_universe_names(&self) -> HashSet<String> {
        self.pass_universe().keys().cloned().collect()
    }

    /// Everything under `index/`: the no-publish arm's whole universe (D-STZ-44).
    pub fn bucket_universe(&self) -> HashMap<String, Value> {
        let current = self.bucket.current();
        let last = self.bucket.last_pass();
        HashMap::from([
            (
                "index.etag".to_string(),
                json!(current.as_ref().map(|v| v.etag.clone())),
            ),
            (
                "index.published_at".to_string(),
                json!(current.as_ref().and_then(|v| v.published_at.clone())),
            ),
            (
                "index.version_count".to_string(),
                json!(self.bucket.versions().len()),
            ),
            (
                "last_pass.pass_id".to_string(),
                json!(last.as_ref().map(|l| l.pass_id.clone())),
            ),
            (
                "last_pass.exit_code".to_string(),
                json!(last.as_ref().map(|l| l.exit_code)),
            ),
            (
                "last_pass.published".to_string(),
                json!(last.as_ref().map(|l| l.published)),
            ),
            (
                "last_pass.index_published_at".to_string(),
                json!(last.as_ref().and_then(|l| l.index_published_at.clone())),
            ),
            (
                "write_check_marker.versions".to_string(),
                json!(self.bucket.write_check_marker_versions()),
            ),
        ])
    }

    pub fn bucket_universe_names(&self) -> HashSet<String> {
        self.bucket_universe().keys().cloned().collect()
    }
}

/// Slot predicate: the value changed (to anything).
pub fn changed() -> SlotPredicate<Value> {
    Box::new(|before: &Value, after: &Value| {
        if before != after {
            Ok(())
        } else {
            Err(format!("expected a change, still {after}"))
        }
    })
}

/// Slot predicate: a count grew by exactly `n`.
pub fn grew_by(n: u64) -> SlotPredicate<Value> {
    Box::new(move |before: &Value, after: &Value| {
        let (b, a) = (before.as_u64().unwrap_or(0), after.as_u64().unwrap_or(0));
        if a == b + n {
            Ok(())
        } else {
            Err(format!("expected {b} + {n}, got {a}"))
        }
    })
}

// -----------------------------------------------------------------------------
// Pass
// -----------------------------------------------------------------------------

/// A warm pass instance.
pub struct PassInstance<'w> {
    world: &'w LambdaWorld,
    function: PassFunction,
    pub scratch: PathBuf,
}

/// What one scheduled pass did, as its invoker and the logs see it.
pub struct PassRun {
    pub answer: Result<Value, FunctionError>,
    pub events: Vec<Value>,
}

impl PassInstance<'_> {
    /// WHEN the 15-minute schedule fires this instance.
    pub fn schedule_fires(&self) -> PassRun {
        self.invoked_with(scheduled_event())
    }

    /// WHEN this instance is invoked with `event`.
    pub fn invoked_with(&self, event: Value) -> PassRun {
        let mark = self.world.events.mark();
        let answer = self.world.runtime.block_on(self.function.handle(event));
        PassRun {
            answer,
            events: self.world.events.since(mark),
        }
    }
}

impl PassRun {
    /// The invocation succeeded (no function error) and its payload.
    pub fn payload(&self) -> Value {
        match &self.answer {
            Ok(payload) => payload.clone(),
            Err(err) => panic!("the pass invocation failed: {err:?}\n{}", self.dump()),
        }
    }

    pub fn published(&self) -> bool {
        self.payload()["published"].as_bool().unwrap_or(false)
    }

    pub fn summaries(&self) -> Vec<Value> {
        named(&self.events, "indexer.ingest.pass_summary")
    }

    /// Exactly one summary; the invocation succeeded with the same exit code
    /// and pass id in its payload (U-STZ-5).
    pub fn assert_exit(&self, exit: Exit) -> Value {
        let summaries = self.summaries();
        assert_eq!(
            summaries.len(),
            1,
            "exactly one pass summary:\n{}",
            self.dump()
        );
        let summary = summaries[0].clone();
        let payload = self.payload();
        assert_eq!(
            summary["exit_code"].as_i64(),
            Some(exit.code()),
            "{}",
            self.dump()
        );
        assert_eq!(
            payload["exit_code"].as_i64(),
            Some(exit.code()),
            "{}",
            self.dump()
        );
        assert_eq!(payload["pass_id"], summary["pass_id"], "{}", self.dump());
        summary
    }

    pub fn dump(&self) -> String {
        format!(
            "answer: {:?}\nevents:\n{}",
            self.answer,
            self.events
                .iter()
                .map(Value::to_string)
                .collect::<Vec<_>>()
                .join("\n")
        )
    }
}

// -----------------------------------------------------------------------------
// Search
// -----------------------------------------------------------------------------

/// A warm search instance.
pub struct SearchInstance<'w> {
    world: &'w LambdaWorld,
    function: SearchFunction,
    pub scratch: PathBuf,
}

impl SearchInstance<'_> {
    /// WHEN anyone sends this request through the Function URL.
    pub fn request(&self, method: &str, path: &str, body: &[u8]) -> Reply {
        self.request_from("203.0.113.7", method, path, body)
    }

    pub fn request_from(&self, client_ip: &str, method: &str, path: &str, body: &[u8]) -> Reply {
        let event = function_url_event(method, path, client_ip, body);
        let response = self.world.runtime.block_on(self.function.handle(event));
        Reply::from_function_url_response(&response)
    }

    /// WHEN Maria searches for a philosophy (what `openlore search` sends).
    pub fn maria_searches(&self, philosophy: &str) -> Reply {
        self.request("POST", SEARCH_PATH, &search_body(philosophy))
    }

    pub fn health(&self) -> Reply {
        self.request("GET", HEALTH_PATH, b"")
    }

    /// The bytes this instance keeps in its `/tmp`: what the 512 MB budget sees
    /// (D-STZ-30, open question 2), however it names its files.
    pub fn scratch_bytes(&self) -> u64 {
        fn bytes_under(dir: &Path) -> u64 {
            std::fs::read_dir(dir)
                .map(|entries| {
                    entries
                        .filter_map(Result::ok)
                        .map(|e| match e.metadata() {
                            Ok(meta) if meta.is_dir() => bytes_under(&e.path()),
                            Ok(meta) => meta.len(),
                            Err(_) => 0,
                        })
                        .sum()
                })
                .unwrap_or(0)
        }
        bytes_under(&self.scratch)
    }

    /// After a swap the instance keeps at most two index sizes in `/tmp`. Two
    /// swaps in a row are what catch a leak: one leaked copy still fits the
    /// bound, two do not.
    pub fn assert_scratch_within_two_index_sizes(&self) {
        let index = self
            .world
            .bucket
            .current()
            .map_or(0, |v| v.bytes.len() as u64);
        self.assert_scratch_within_two_sizes_of(index);
    }

    /// The same bound, against an index of `index` bytes.
    pub fn assert_scratch_within_two_sizes_of(&self, index: u64) {
        assert!(index > 0, "an index to measure against");
        let held = self.scratch_bytes();
        assert!(
            held <= 2 * index,
            "{held} bytes in /tmp against an index of {index}"
        );
    }
}

// -----------------------------------------------------------------------------
// Probe
// -----------------------------------------------------------------------------

/// A probe instance.
pub struct ProbeInstance<'w> {
    world: &'w LambdaWorld,
    function: ProbeFunction,
}

pub struct ProbeRun {
    pub answer: Result<Value, FunctionError>,
    pub events: Vec<Value>,
}

impl ProbeInstance<'_> {
    /// WHEN the 5-minute schedule fires the probe.
    pub fn schedule_fires(&self) -> ProbeRun {
        self.invoked_with(scheduled_event())
    }

    /// WHEN the probe is invoked with `event`.
    pub fn invoked_with(&self, event: Value) -> ProbeRun {
        let mark = self.world.events.mark();
        let answer = self.world.runtime.block_on(self.function.handle(event));
        ProbeRun {
            answer,
            events: self.world.events.since(mark),
        }
    }
}

impl ProbeRun {
    /// The invocation succeeded and wrote exactly one health line. The "exactly
    /// one" is about `indexer.host.health` only: a run can also write
    /// `indexer.lambda.served_index_stale` (D-STZ-48).
    pub fn health_line(&self) -> Value {
        assert!(self.answer.is_ok(), "no function error: {:?}", self.answer);
        let lines = named(&self.events, "indexer.host.health");
        assert_eq!(lines.len(), 1, "exactly one health line: {:?}", self.events);
        lines[0].clone()
    }

    /// The `indexer.lambda.served_index_stale` lines of this run.
    pub fn stale_readings(&self) -> Vec<Value> {
        named(&self.events, "indexer.lambda.served_index_stale")
    }
}

/// What the canned edge's `/healthz` says it serves, when it answers 200.
pub const SERVED_AT: &str = "2026-10-27T13:45:01Z";

/// The public edge as the probe sees it: a loopback server answering
/// `/healthz` and the canned search with fixed statuses, logging what it was
/// asked, in order.
pub struct CannedEdge {
    url: String,
    asked: Arc<Mutex<Vec<String>>>,
}

impl CannedEdge {
    pub fn answering(healthz_status: u16, search_status: u16) -> Self {
        let body = format!(r#"{{"status":"ok","last_successful_pass_at":"{SERVED_AT}"}}"#);
        Self::with_health_body(healthz_status, search_status, body)
    }

    /// An edge whose `/healthz` reports this `published-at`, or `null`.
    pub fn serving_an_index_published_at(served: Option<&str>) -> Self {
        let body = json!({"status": "ok", "last_successful_pass_at": served}).to_string();
        Self::with_health_body(200, 200, body)
    }

    /// An edge whose `/healthz` answers 200 with this body.
    pub fn with_health_body(healthz_status: u16, search_status: u16, body: String) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind the canned edge");
        let url = format!("http://{}", listener.local_addr().expect("edge address"));
        let asked = Arc::new(Mutex::new(Vec::new()));
        let log = asked.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let mut stream = stream;
                let mut reader = BufReader::new(stream.try_clone().expect("clone"));
                let mut request_line = String::new();
                if reader.read_line(&mut request_line).is_err() {
                    continue;
                }
                let mut content_length = 0usize;
                loop {
                    let mut header = String::new();
                    if reader.read_line(&mut header).is_err()
                        || header == "\r\n"
                        || header.is_empty()
                    {
                        break;
                    }
                    if let Some(v) = header.to_ascii_lowercase().strip_prefix("content-length:") {
                        content_length = v.trim().parse().unwrap_or(0);
                    }
                }
                let mut request_body = vec![0u8; content_length];
                let _ = reader.read_exact(&mut request_body);
                let asked_for_health = request_line.contains(HEALTH_PATH);
                log.lock().unwrap().push(
                    if asked_for_health {
                        HEALTH_PATH
                    } else {
                        SEARCH_PATH
                    }
                    .to_string(),
                );
                let (status, payload) = if asked_for_health {
                    let unusable = r#"{"status":"store_unusable"}"#.to_string();
                    let answer = if healthz_status == 200 {
                        body.clone()
                    } else {
                        unusable
                    };
                    (healthz_status, answer)
                } else {
                    let answer = if search_status == 200 {
                        r#"{"results":[],"distinct_author_count":0,"total_claims":0,"suggestion":null,"withheld_unknown_provenance":0}"#
                    } else {
                        r#"{"error":"index_unavailable"}"#
                    };
                    (search_status, answer.to_string())
                };
                let response = format!(
                    "HTTP/1.1 {status} X\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{payload}",
                    payload.len()
                );
                let _ = stream.write_all(response.as_bytes());
            }
        });
        Self { url, asked }
    }

    /// An edge that accepts every connection and never answers.
    pub fn hanging() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind the hanging edge");
        let url = format!("http://{}", listener.local_addr().expect("edge address"));
        std::thread::spawn(move || {
            let held: Vec<_> = listener.incoming().flatten().collect();
            drop(held);
        });
        Self {
            url,
            asked: Arc::default(),
        }
    }

    pub fn url(&self) -> &str {
        &self.url
    }

    /// The paths asked, in order.
    pub fn asked(&self) -> Vec<String> {
        self.asked.lock().unwrap().clone()
    }
}
