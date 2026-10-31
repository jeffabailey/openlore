//! `decide` — the PURE decisions of the Lambda root (ADR-090, D-STZ-10/11,
//! D-STZ-7/9, D-STZ-16, D-STZ-23, D-STZ-40, D-STZ-40a, D-STZ-44, D-STZ-48). No I/O: `now`,
//! every age and every outcome are arguments. The handlers only read, call
//! these, and write.
//!
//! Contracts: `tests/acceptance/scale_to_zero_core.rs`.

use std::time::Duration;

use appview_domain::ingest_pass::{PassExit, PassFailure};
use chrono::{DateTime, FixedOffset, Utc};
use ports::IndexFingerprint;

/// DESIGN's figure for how often a warm search re-checks the index object
/// (D-STZ-10). The deployed value comes from the environment (`config`).
pub const FRESHNESS_TTL: Duration = Duration::from_secs(60);
/// A warm pass instance may reuse its last good DID list this long when the
/// parameter store fails (D-STZ-16).
pub const DIDS_COPY_MAX_AGE: Duration = Duration::from_secs(2 * 60 * 60);
/// DESIGN's figure for how long no pass may run before the probe calls the index
/// not live (D-STZ-23). The deployed value comes from the environment (`config`).
pub const LAST_PASS_STALE_AFTER: Duration = Duration::from_secs(45 * 60);
/// The probe compares the served index with the record only when the record is
/// older than this (D-STZ-48).
pub const SERVED_INDEX_MARGIN: Duration = Duration::from_secs(120);

/// What a publish leaves for the 5 s record and the 5 s summary (D-STZ-40a).
pub const AFTER_PUBLISH_RESERVE: Duration = Duration::from_secs(10);

/// Is a freshness check due? `last_checked` is `None` before the first check.
pub fn freshness_due(
    now: DateTime<Utc>,
    last_checked: Option<DateTime<Utc>>,
    ttl: Duration,
) -> bool {
    last_checked.is_none_or(|last| {
        // A clock that went backwards checks now, which also resets `last`.
        (now - last).to_std().map_or(true, |elapsed| elapsed >= ttl)
    })
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
    match (current, head) {
        (_, HeadSeen::Failed) => Reload::KeepAndReport,
        (Some(served), HeadSeen::Present(etag)) if served == etag => Reload::Keep,
        (_, HeadSeen::Present(etag)) => Reload::Fetch(etag.clone()),
        (Some(_), HeadSeen::Absent) => Reload::OpenEmpty,
        (None, HeadSeen::Absent) => Reload::Keep,
    }
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
    _exit: PassExit,
    before: IndexFingerprint,
    after: IndexFingerprint,
    store_usable: bool,
) -> Publish {
    if before != after && store_usable {
        Publish::Publish
    } else {
        Publish::Skip
    }
}

/// D-STZ-40a: the longest a publish may take when `time_left` remains. The
/// record and the summary keep their reserve, even if the pass's local steps ran
/// long. With less than the reserve left, the limit is zero.
pub fn publish_time_limit(time_left: Duration, operation_timeout: Duration) -> Duration {
    operation_timeout.min(time_left.saturating_sub(AFTER_PUBLISH_RESERVE))
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
    let pass_is_recent = facts.last_pass_age.is_some_and(|age| age <= stale_after);
    let checks = [
        (facts.healthz_ok, NotLiveCause::HealthzFailed),
        (facts.search_ok, NotLiveCause::SearchFailed),
        (pass_is_recent, NotLiveCause::NoRecentPass),
    ];
    Liveness {
        causes: checks
            .into_iter()
            .filter_map(|(good, cause)| (!good).then_some(cause))
            .collect(),
    }
}

/// D-STZ-16: may a warm pass reuse its last good DID list of this age?
pub fn dids_copy_usable(age: Duration, max: Duration) -> bool {
    age <= max
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
    match (pass_exit, publish, record) {
        (PassExit::Failed(_), _, _) => pass_exit,
        (_, PublishStep::Conflict, _) => PassExit::Failed(PassFailure::PublishConflict),
        (_, PublishStep::Failed, _) => PassExit::Failed(PassFailure::PublishFailed),
        (_, PublishStep::NoFailure, RecordStep::Failed) => {
            PassExit::Failed(PassFailure::PassRecordFailed)
        }
        (_, PublishStep::NoFailure, RecordStep::NoFailure) => pass_exit,
    }
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
    let fields = event.as_object().ok_or(EventRefused)?;
    let mode = fields.get("mode").and_then(serde_json::Value::as_str);
    match (fields.len(), mode) {
        (0, _) => Ok(PassMode::Normal),
        (1, Some("no-publish")) => Ok(PassMode::NoPublish),
        _ => Err(EventRefused),
    }
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

fn read_instant(text: &str) -> Option<DateTime<FixedOffset>> {
    DateTime::parse_from_rfc3339(text).ok()
}

/// How far `served` is behind `recorded`; `None` when it is not behind.
fn lag_behind(served: DateTime<FixedOffset>, recorded: DateTime<FixedOffset>) -> Option<Duration> {
    let lag = recorded.signed_duration_since(served).to_std().ok()?;
    (!lag.is_zero()).then_some(lag)
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
    let recorded_at = read_instant(recorded?)?;
    if record_age? <= margin {
        return None;
    }
    match served {
        ServedIndex::Unknown => None,
        ServedIndex::NothingPublished => Some(StaleServedIndex { lag: None }),
        ServedIndex::PublishedAt(text) => {
            let lag = lag_behind(read_instant(text)?, recorded_at)?;
            Some(StaleServedIndex { lag: Some(lag) })
        }
    }
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    /// § Timeouts, stated here so a changed reserve shows.
    const OPERATION_TIMEOUT: Duration = Duration::from_secs(35);
    const TEN_SECONDS: Duration = Duration::from_secs(10);

    proptest! {
        // Any time left: the limit is the most the 35 s and the reserve allow.
        #[test]
        fn a_publish_gets_all_the_time_it_may_and_never_the_records_and_summarys_seconds(
            time_left in prop_oneof![
                prop::sample::select(vec![0, 1, 9_999, 10_000, 10_001, 44_999, 45_000, 45_001]),
                0_u64..=900_000,
            ].prop_map(Duration::from_millis),
        ) {
            let limit = publish_time_limit(time_left, OPERATION_TIMEOUT);

            prop_assert!(limit <= OPERATION_TIMEOUT);
            prop_assert!(limit.is_zero() || limit + TEN_SECONDS <= time_left);
            prop_assert!(
                limit == OPERATION_TIMEOUT
                    || limit + TEN_SECONDS == time_left
                    || (limit.is_zero() && time_left <= TEN_SECONDS),
                "{limit:?} wastes time out of {time_left:?}"
            );
        }
    }

    // bypass: CORE-8's generators reach a later-sorting older instant only when
    // two random instants fall within a day, so one fixed row pins that direction.
    #[test]
    fn a_served_index_behind_the_record_is_stale_even_when_its_text_sorts_later() {
        let ten_utc_written_at_fifteen = ServedIndex::PublishedAt("2026-10-27T15:00:00+05:00");
        let found = served_index_stale(
            ten_utc_written_at_fifteen,
            Some("2026-10-27T14:00:00Z"),
            Some(Duration::from_secs(600)),
            SERVED_INDEX_MARGIN,
        );
        assert_eq!(
            found,
            Some(StaleServedIndex {
                lag: Some(Duration::from_secs(4 * 60 * 60)),
            })
        );
    }
}
