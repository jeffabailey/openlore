//! `decide` — the PURE decisions of the Lambda root (ADR-090, D-STZ-10/11,
//! D-STZ-7/9, D-STZ-16, D-STZ-23, D-STZ-40, D-STZ-44, D-STZ-48). No I/O: `now`,
//! every age and every outcome are arguments. The handlers only read, call
//! these, and write.
//!
//! Contracts: `tests/acceptance/scale_to_zero_core.rs`.
//
// SCAFFOLD: true  (DISTILL 2026-10-27: signatures only; every body panics)

use std::time::Duration;

use appview_domain::ingest_pass::PassExit;
use chrono::{DateTime, Utc};
use ports::IndexFingerprint;

/// A warm search instance re-checks the index object at most this often (D-STZ-10).
pub const FRESHNESS_TTL: Duration = Duration::from_secs(60);
/// A warm pass instance may reuse its last good DID list this long when the
/// parameter store fails (D-STZ-16).
pub const DIDS_COPY_MAX_AGE: Duration = Duration::from_secs(2 * 60 * 60);
/// The probe calls the index not live when no pass ran for longer than this (D-STZ-23).
pub const LAST_PASS_STALE_AFTER: Duration = Duration::from_secs(45 * 60);
/// The probe compares the served index with the record only when the record is
/// older than this (D-STZ-48).
pub const SERVED_INDEX_MARGIN: Duration = Duration::from_secs(120);

/// Is a freshness check due? `last_checked` is `None` before the first check.
pub fn freshness_due(
    now: DateTime<Utc>,
    last_checked: Option<DateTime<Utc>>,
    ttl: Duration,
) -> bool {
    let _ = (now, last_checked, ttl);
    panic!("Not yet implemented -- RED scaffold")
}

/// What a `HEAD` of the index object told the search instance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HeadSeen {
    /// The object exists with this ETag.
    Present(String),
    /// No object exists.
    Absent,
    /// The check itself failed.
    Failed,
}

/// What the search instance does with what it saw.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reload {
    /// Keep answering from the current copy.
    Keep,
    /// Download this version, open it, swap, delete the old file.
    Fetch(String),
    /// The object is gone: answer from an empty index (D-STZ-13).
    OpenEmpty,
    /// The check failed: keep the current copy and report the failure (D-STZ-11).
    KeepAndReport,
}

/// Decide from the ETag the instance serves (`None` = an empty index) and the
/// `HEAD` it just made.
pub fn reload_needed(current: Option<&str>, head: &HeadSeen) -> Reload {
    let _ = (current, head);
    panic!("Not yet implemented -- RED scaffold")
}

/// Whether the pass publishes the store it worked on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Publish {
    Publish,
    Skip,
}

/// D-STZ-7 / D-STZ-9: publish iff the fingerprint changed and the store is
/// still usable, whatever the exit code.
pub fn publish_decision(
    exit: PassExit,
    before: IndexFingerprint,
    after: IndexFingerprint,
    store_usable: bool,
) -> Publish {
    let _ = (exit, before, after, store_usable);
    panic!("Not yet implemented -- RED scaffold")
}

/// What one probe run observed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProbeFacts {
    pub healthz_ok: bool,
    pub search_ok: bool,
    /// Age of `last-pass.json`; `None` when there is none or it could not be read.
    pub last_pass_age: Option<Duration>,
}

/// Why the index is not live.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum NotLiveCause {
    HealthzFailed,
    SearchFailed,
    NoRecentPass,
}

/// The probe's verdict: live when there is no cause.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Liveness {
    pub causes: Vec<NotLiveCause>,
}

/// D-STZ-23 / U-STZ-3: not live when `/healthz` or the canned search failed,
/// or when no pass ran within `stale_after`.
pub fn not_live(facts: &ProbeFacts, stale_after: Duration) -> Liveness {
    let _ = (facts, stale_after);
    panic!("Not yet implemented -- RED scaffold")
}

/// D-STZ-16: may a warm pass reuse its last good DID list of this age?
pub fn dids_copy_usable(age: Duration, max: Duration) -> bool {
    let _ = (age, max);
    panic!("Not yet implemented -- RED scaffold")
}

/// How the publish step of a pass ended (D-STZ-40).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PublishStep {
    /// Nothing was attempted, or the object was published.
    NoFailure,
    /// The store answered 412.
    Conflict,
    /// Any other store error, including a timed-out publish (D-STZ-40a).
    Failed,
}

/// How the `last-pass.json` write of a pass ended (D-STZ-40).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecordStep {
    /// Written, or not attempted yet (the record's own exit is decided with
    /// this value, because it carries the exit decided before its own write).
    NoFailure,
    /// The write failed.
    Failed,
}

/// D-STZ-40: the exit the one `pass_summary` carries. A pass that already failed
/// keeps its cause (first failure wins); then a publish failure; then a record
/// failure; then the pass exit.
pub fn final_exit(pass_exit: PassExit, publish: PublishStep, record: RecordStep) -> PassExit {
    let _ = (pass_exit, publish, record);
    panic!("Not yet implemented -- RED scaffold")
}

/// Whether this invocation may write (D-STZ-44).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PassMode {
    Normal,
    NoPublish,
}

/// An event the pass does not accept: a function error, before any work and
/// any call (D-STZ-44).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EventRefused;

/// D-STZ-44, strict: `{}` is a normal pass, and an object whose only key is
/// `mode` with the known value `no-publish` is no-publish. Everything else is
/// refused: a non-object, `null`, a misspelled key, an extra key, an unknown mode.
pub fn pass_mode(event: &serde_json::Value) -> Result<PassMode, EventRefused> {
    let _ = event;
    panic!("Not yet implemented -- RED scaffold")
}

/// What the public `/healthz` said about the index it serves (D-STZ-48).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServedIndex<'a> {
    /// `/healthz` failed.
    Unknown,
    /// It answered with `null`: it serves no published index.
    NothingPublished,
    /// It answered with this `published-at` text.
    PublishedAt(&'a str),
}

/// A served index that lags the record: the fields of
/// `indexer.lambda.served_index_stale`. `lag` is `None` when the served value is
/// `null`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StaleServedIndex {
    pub lag: Option<Duration>,
}

/// D-STZ-48, U-STZ-6. `recorded` is the record's `index_published_at`;
/// `record_age` is `None` when the record is missing or unreadable.
///
/// No comparison, so `None`, when the served value is unknown, the recorded
/// value is `None`, or the record's age is unknown. Otherwise stale when the
/// record is older than `margin` and the served value is older than the recorded
/// one, or `null`. Compares parsed instants, never text. A time that does not
/// parse is unreadable, so it means no comparison.
pub fn served_index_stale(
    served: ServedIndex<'_>,
    recorded: Option<&str>,
    record_age: Option<Duration>,
    margin: Duration,
) -> Option<StaleServedIndex> {
    let _ = (served, recorded, record_age, margin);
    panic!("Not yet implemented -- RED scaffold")
}
