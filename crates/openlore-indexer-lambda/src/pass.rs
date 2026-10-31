//! Pass mode (ADR-090 §4): `decide::pass_mode` reads the event first, and refuses
//! all but two before any work or call (D-STZ-44). Then, in the D-STZ-40 order,
//! render the DID list from the parameter store, fetch the starting index, run
//! the reused ADR-077/078 pass off the async runtime (D-STZ-42, UI-11) and take
//! its exit, publish on change under a version condition (D-STZ-6, D-STZ-7),
//! write `last-pass.json`, then write the ONE `pass_summary`, with the exit
//! `decide::final_exit` settles.
//!
//! A pass with no usable starting index does no work: it ends as a failed pass and
//! goes through the same record and summary (D-STZ-41).
//!
//! A no-publish run takes the same steps up to the pass work, and reads the same
//! plan value. That arm is `no_publish::run_no_publish`, which takes the read port
//! only, so it has no path to a write (D-STZ-44).

use std::ffi::OsString;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use adapter_index_store::UnusableReason;
use appview_domain::ingest_pass::{PassExit, PassFailure};
use appview_domain::pass_runner::PassId;
use chrono::{DateTime, SecondsFormat, Utc};
use openlore_indexer::config::{IndexerConfig, REPO_DIDS_FILE_VAR};
use openlore_indexer::events::Events;
use openlore_indexer::ingest_pass::{write_pass_summary, EndedPass};
use openlore_indexer::run::Wired;
use openlore_indexer::runner_contract::{BootNonce, PassLabel};
use ports::{
    IndexFingerprint, IndexObjectError, IndexObjectReadPort, LastPass, ObjectVersion, ProbeOutcome,
    ProbeRefused, PublishMetadata, PublishOutcome, WriteCondition, PUBLISH_OPERATION_TIMEOUT,
};
use serde_json::json;
use tokio::task::JoinError;

use crate::decide::{
    dids_copy_usable, final_exit, pass_mode, publish_decision, publish_time_limit, EventRefused,
    PassMode, Publish, PublishStep, RecordStep, DIDS_COPY_MAX_AGE,
};
use crate::no_publish::run_no_publish;
use crate::{
    events_through, pass_wiring, refuse_start, Environment, EventSink, FunctionError, InitRefused,
    PassDriven,
};

/// How a pass whose work panicked ended (D-STZ-43).
const PANICKED: PassExit = PassExit::Failed(PassFailure::PassPanicked);

/// The function's timeout (D-STZ-40a). The deadline clock starts at handler entry,
/// so what a publish may take is measured from there.
const FUNCTION_TIMEOUT: Duration = Duration::from_secs(840);

/// The plan value both arms read: the object the pass saw at its start, the
/// reused pass's work, and whether its store is to be published.
pub(crate) struct WorkedPass {
    pub(crate) seen: Option<ObjectVersion>,
    pub(crate) ended: EndedPass,
    pub(crate) publish: Publish,
}

impl WorkedPass {
    /// D-STZ-41: no work was done, so nothing is published. `seen` names an object
    /// that downloaded but will not open; a failed download saw none (D-STZ-48).
    fn no_usable_start(seen: Option<ObjectVersion>) -> Self {
        Self {
            seen,
            ended: EndedPass::before_work(PassFailure::StartingIndexUnusable),
            publish: Publish::Skip,
        }
    }
}

/// The index a pass published.
struct PublishedIndex {
    bytes: u64,
    published_at: String,
}

/// How a pass's publish ended: what landed, and what it means for the exit.
struct PublishEnd {
    published: Option<PublishedIndex>,
    step: PublishStep,
}

impl PublishEnd {
    const fn nothing_attempted() -> Self {
        Self {
            published: None,
            step: PublishStep::NoFailure,
        }
    }

    const fn not_landed(step: PublishStep) -> Self {
        Self {
            published: None,
            step,
        }
    }

    /// What the store's answer means, stamped as the publish asked.
    fn of(outcome: Result<PublishOutcome, IndexObjectError>, published_at: String) -> Self {
        match outcome {
            Ok(PublishOutcome::Published(version)) => Self {
                published: Some(PublishedIndex {
                    bytes: version.size_bytes,
                    published_at,
                }),
                step: PublishStep::NoFailure,
            },
            Ok(PublishOutcome::Conflict) => Self::not_landed(PublishStep::Conflict),
            Err(_) => Self::not_landed(PublishStep::Failed),
        }
    }
}

/// One pass instance (warm across scheduled events).
pub struct PassFunction {
    driven: PassDriven,
    config: IndexerConfig,
    did_list_file: PathBuf,
    /// When this instance last rendered a good list: the age of its copy.
    list_rendered_at: Mutex<Option<DateTime<Utc>>>,
    boot: BootNonce,
    passes_started: AtomicU64,
}

impl PassFunction {
    /// Lambda init: refuse a key naming the write probe's marker before any PUT,
    /// read the configuration, then run the write probe, whose marker the bucket
    /// keeps and whose two accepted writes refuse the pass (D-STZ-39). The pass
    /// reads its list only from the file it renders, so a
    /// configuration with no such file is refused.
    pub async fn init(env: &Environment, driven: PassDriven) -> Result<Self, InitRefused> {
        env.refuse_a_key_naming_the_marker(&driven.events)?;
        let config = env.indexer_config()?;
        let did_list_file = config.repo_dids_file.clone().ok_or_else(|| InitRefused {
            reason: format!("{REPO_DIDS_FILE_VAR} is not set"),
        })?;
        run_write_probe(&driven).await?;
        Ok(Self {
            driven,
            config,
            did_list_file,
            list_rendered_at: Mutex::new(None),
            boot: BootNonce::now(),
            passes_started: AtomicU64::new(0),
        })
    }

    /// Run one pass for one event. `Ok` carries `{pass_id, exit_code, published,
    /// index_bytes}`; `index_bytes` is `null` when nothing was published. A
    /// no-publish run adds `mode` and `would_publish`. An event `decide::pass_mode`
    /// refuses is a function error, before any work and any call (D-STZ-44).
    pub async fn handle(
        &self,
        event: serde_json::Value,
    ) -> Result<serde_json::Value, FunctionError> {
        let mode = pass_mode(&event).map_err(refused_event)?;
        let started = Instant::now();
        let pass = self.next_pass();
        self.render_did_list().await;
        match mode {
            PassMode::Normal => self.publishing_run(pass, started).await,
            PassMode::NoPublish => self.no_publish_run(pass, started).await,
        }
    }

    /// The arm that may write: publish on change, then the record, then the summary.
    async fn publishing_run(
        &self,
        pass: PassLabel,
        started: Instant,
    ) -> Result<serde_json::Value, FunctionError> {
        let WorkedPass {
            seen,
            ended,
            publish,
        } = work_from_the_starting_index(
            self.driven.index_read.as_ref(),
            &self.config,
            self.driven.events.clone(),
            pass,
            started,
        )
        .await?;
        let events = events_through(self.driven.events.clone());
        let PublishEnd { published, step } = match publish {
            Publish::Publish => {
                let limit = publish_limit(started.elapsed());
                self.publish(pass, seen.as_ref(), limit, &events).await
            }
            Publish::Skip => PublishEnd::nothing_attempted(),
        };
        let exit_so_far = final_exit(ended.exit(), step, RecordStep::NoFailure);
        let recorded = self
            .record(pass, exit_so_far, published.as_ref(), seen.as_ref())
            .await;
        let exit = final_exit(ended.exit(), step, recorded);
        let exit_code = write_pass_summary(&events, pass, &ended, exit, started);
        Ok(json!({
            "pass_id": pass.to_string(),
            "exit_code": exit_code,
            "published": published.is_some(),
            "index_bytes": published.map(|index| index.bytes),
        }))
    }

    /// The arm that cannot: it is handed the read port and none of the rest.
    async fn no_publish_run(
        &self,
        pass: PassLabel,
        started: Instant,
    ) -> Result<serde_json::Value, FunctionError> {
        let read = self.driven.index_read.as_ref();
        let report = run_no_publish(
            read,
            &self.config,
            self.driven.events.clone(),
            pass,
            started,
        )
        .await?;
        Ok(report.reply())
    }

    fn next_pass(&self) -> PassLabel {
        let sequence = self.passes_started.fetch_add(1, Ordering::SeqCst) + 1;
        PassLabel::new(self.boot, PassId(sequence))
    }

    /// Render the operator's list where the reused pass reads it, once a pass
    /// (D-STZ-16). When that fails, `settle_did_list` says whether the last good
    /// copy stays, and the pass's own list refusal decides from there (ADR-081).
    async fn render_did_list(&self) {
        let now = self.driven.clock.now_utc();
        let rendered = match self.driven.repo_dids.current().await {
            Ok(list) => render_by_rename(&self.did_list_file, &list).map_err(|e| e.to_string()),
            Err(unavailable) => Err(unavailable.to_string()),
        };
        let mut rendered_at = self
            .list_rendered_at
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        *rendered_at = settle_did_list(rendered, &self.did_list_file, *rendered_at, now);
    }

    /// Publish the closed index file over the version this pass saw, stamped
    /// with this function's clock (D-STZ-6, D-STZ-12), and log how it ended.
    async fn publish(
        &self,
        pass: PassLabel,
        seen: Option<&ObjectVersion>,
        limit: Duration,
        events: &Events,
    ) -> PublishEnd {
        let published_at = self.now_stamp();
        let metadata = PublishMetadata {
            published_at: published_at.clone(),
            pass_id: pass.to_string(),
        };
        let outcome = self
            .driven
            .index_write
            .publish_if_match(
                &self.config.index_path,
                publish_condition(seen),
                metadata,
                limit,
            )
            .await
            .inspect_err(|failed| eprintln!("openlore-indexer-lambda: not published: {failed}"));
        let end = PublishEnd::of(outcome, published_at);
        log_publish_end(events, pass, &end);
        end
    }

    /// Write `last-pass.json`, every pass (U-STZ-3), carrying the exit decided
    /// before this write. A failed write changes only the summary (D-STZ-40).
    async fn record(
        &self,
        pass: PassLabel,
        exit: PassExit,
        published: Option<&PublishedIndex>,
        seen: Option<&ObjectVersion>,
    ) -> RecordStep {
        let record = LastPass {
            pass_id: pass.to_string(),
            exit_code: exit.code(),
            finished_at: self.now_stamp(),
            published: published.is_some(),
            index_published_at: index_published_at(published, seen),
        };
        match self.driven.index_write.put_last_pass(&record).await {
            Ok(()) => RecordStep::NoFailure,
            Err(failed) => {
                eprintln!("openlore-indexer-lambda: pass not recorded: {failed}");
                RecordStep::Failed
            }
        }
    }

    fn now_stamp(&self) -> String {
        self.driven
            .clock
            .now_utc()
            .to_rfc3339_opts(SecondsFormat::Secs, true)
    }
}

/// A published size feeds the size alarm (D-STZ-33). A conflict is logged here,
/// and its page comes from the one summary's cause (D-STZ-6).
fn log_publish_end(events: &Events, pass: PassLabel, end: &PublishEnd) {
    if let Some(index) = &end.published {
        events.emit_in(
            Some(pass),
            json!({"event": "indexer.lambda.published", "index_bytes": index.bytes}),
        );
    }
    if end.step == PublishStep::Conflict {
        events.emit_in(
            Some(pass),
            json!({"event": "indexer.lambda.publish_conflict"}),
        );
    }
}

/// D-STZ-44: the function error of an event `decide::pass_mode` refused.
fn refused_event(_: EventRefused) -> FunctionError {
    FunctionError {
        message: r#"pass event refused: only {} or {"mode":"no-publish"} is accepted"#.to_string(),
    }
}

/// Fetch the starting index, then work on it. A pass with no starting index it
/// can use does no work (D-STZ-41).
pub(crate) async fn work_from_the_starting_index(
    read: &dyn IndexObjectReadPort,
    config: &IndexerConfig,
    sink: Arc<dyn EventSink>,
    pass: PassLabel,
    started: Instant,
) -> Result<WorkedPass, FunctionError> {
    match fetch_starting_index(read, config).await {
        Ok(seen) => work_the_pass(config, sink, seen, pass, started).await,
        Err(_) => Ok(WorkedPass::no_usable_start(None)),
    }
}

/// Replace the local copy with the current object. `fetch_to` wants a new file,
/// and a warm instance's last copy must not outlive a reset of the bucket.
async fn fetch_starting_index(
    read: &dyn IndexObjectReadPort,
    config: &IndexerConfig,
) -> Result<Option<ObjectVersion>, IndexObjectError> {
    let index_path = &config.index_path;
    if let Err(stuck) = remove_if_present(index_path) {
        eprintln!("openlore-indexer-lambda: last index copy not removed: {stuck}");
    }
    read.fetch_to(index_path)
        .await
        .inspect_err(|failed| eprintln!("openlore-indexer-lambda: not fetched: {failed}"))
}

/// The reused pass, off the async runtime: it blocks on a runtime of its own
/// (UI-11). Its store is closed when this returns. Opening the store is the only
/// step of the wiring that can fail, so a refused wiring is a starting index that
/// will not open: damaged, or no room for what opening writes (D-STZ-41).
async fn work_the_pass(
    config: &IndexerConfig,
    sink: Arc<dyn EventSink>,
    seen: Option<ObjectVersion>,
    pass: PassLabel,
    started: Instant,
) -> Result<WorkedPass, FunctionError> {
    let config = config.clone();
    let index_path = config.index_path.clone();
    tokio::task::spawn_blocking(move || match pass_wiring(config, sink) {
        Ok(wired) => work_on(wired, seen, pass, started, &index_path),
        Err(refused) => {
            let cause = refused.message;
            eprintln!("openlore-indexer-lambda: starting index not opened: {cause}");
            WorkedPass::no_usable_start(seen)
        }
    })
    .await
    .map_err(|crashed| FunctionError {
        message: format!("pass task: {crashed}"),
    })
}

/// The write probe, off the async runtime: it is sync and blocks (D-STZ-39,
/// UI-11). A store that ignored the condition refuses the pass, and says so.
async fn run_write_probe(driven: &PassDriven) -> Result<(), InitRefused> {
    let index_write = Arc::clone(&driven.index_write);
    let answer = tokio::task::spawn_blocking(move || index_write.probe()).await;
    write_probe_verdict(answer)
        .map_err(|refused| refuse_start(&driven.events, "index_object_write", refused))
}

/// D-STZ-39, UI-17: the probe's own refusal is the only one. A probe that never
/// answered says nothing about conditions, so that pass runs and reports through
/// D-STZ-41.
fn write_probe_verdict(answer: Result<ProbeOutcome, JoinError>) -> Result<(), ProbeRefused> {
    match answer {
        Ok(ProbeOutcome::Ok) => Ok(()),
        Ok(ProbeOutcome::Refused {
            reason,
            detail,
            structured,
        }) => Err(ProbeRefused {
            reason,
            detail,
            structured,
        }),
        Err(crashed) => {
            eprintln!("openlore-indexer-lambda: write probe gave no answer: {crashed}");
            Ok(())
        }
    }
}

/// The line the self-host indexer writes for an unusable store, from this root.
fn store_unusable_event(reason: UnusableReason) -> serde_json::Value {
    json!({
        "event": "indexer.store.unusable",
        "adapter": "index_store",
        "reason": reason.token(),
    })
}

/// One pass's work between two fingerprints, ending with the store closed. The
/// work carries the purge `Wired` lends it (ADR-082), so a purge shows as a
/// changed fingerprint and reaches the bucket only through this publish. The
/// store's condition is read once, after the work, for the log and the publish
/// (D-STZ-9). It is checked closed, because a close is what could still fail to
/// fold the log into the file (P-7).
fn work_on(
    wired: Wired,
    seen: Option<ObjectVersion>,
    pass: PassLabel,
    started: Instant,
    index_path: &Path,
) -> WorkedPass {
    let store = Arc::clone(&wired.wiring.store_condition);
    let events = wired.wiring.events.clone();
    let before = store.fingerprint().ok();
    let ended = wired.pass_work(pass, started);
    let after = store.fingerprint().ok();
    let unusable = store.unusable_reason();
    drop(store);
    if let Some(reason) = unusable {
        events.emit_in_unchecked(Some(pass), store_unusable_event(reason));
    }
    let publish = publish_on_change(
        ended.exit(),
        before,
        after,
        ready_to_upload(unusable.is_none(), index_path),
    );
    WorkedPass {
        seen,
        ended,
        publish,
    }
}

/// `decide::publish_decision`, when both fingerprints could be read and the work
/// did not panic. A store that cannot say what it holds is never published, and a
/// panic counts as a store not usable, whatever it left behind (D-STZ-43).
fn publish_on_change(
    exit: PassExit,
    before: Option<IndexFingerprint>,
    after: Option<IndexFingerprint>,
    store_usable: bool,
) -> Publish {
    let usable = store_usable && exit != PANICKED;
    before.zip(after).map_or(Publish::Skip, |(before, after)| {
        publish_decision(exit, before, after, usable)
    })
}

/// D-STZ-6: on the version the pass fetched, or on no object existing when it
/// fetched none.
fn publish_condition(seen: Option<&ObjectVersion>) -> WriteCondition {
    seen.map_or(WriteCondition::IfNoneMatch, |version| {
        WriteCondition::IfMatch(version.etag.clone())
    })
}

/// `decide::publish_time_limit`, `elapsed` after handler entry.
fn publish_limit(elapsed: Duration) -> Duration {
    publish_time_limit(
        FUNCTION_TIMEOUT.saturating_sub(elapsed),
        PUBLISH_OPERATION_TIMEOUT,
    )
}

/// D-STZ-48: this pass's publish, otherwise the object seen at the start.
fn index_published_at(
    published: Option<&PublishedIndex>,
    seen: Option<&ObjectVersion>,
) -> Option<String> {
    published.map_or_else(
        || seen.and_then(|version| version.published_at.clone()),
        |index| Some(index.published_at.clone()),
    )
}

/// P-7: a file uploaded with rows still in its log would lose them.
fn ready_to_upload(store_usable: bool, index_path: &Path) -> bool {
    store_usable && write_ahead_log_is_empty(index_path)
}

/// DuckDB keeps a database's log beside it, as `<database>.wal`. A log that
/// cannot be read counts as holding rows.
fn write_ahead_log_is_empty(index_path: &Path) -> bool {
    let mut log = OsString::from(index_path);
    log.push(".wal");
    match std::fs::metadata(&log) {
        Ok(found) => found.len() == 0,
        Err(unread) => unread.kind() == ErrorKind::NotFound,
    }
}

fn remove_if_present(file: &Path) -> std::io::Result<()> {
    match std::fs::remove_file(file) {
        Err(failed) if failed.kind() != ErrorKind::NotFound => Err(failed),
        _ => Ok(()),
    }
}

/// What a render attempt leaves behind, returning when the copy on disk was last
/// read good. Only a good read moves that time, so an outage never makes a copy
/// younger. A copy past its age is removed, so the ADR-081 path refuses.
fn settle_did_list(
    rendered: Result<(), String>,
    file: &Path,
    rendered_at: Option<DateTime<Utc>>,
    now: DateTime<Utc>,
) -> Option<DateTime<Utc>> {
    let Err(cause) = rendered else {
        return Some(now);
    };
    eprintln!("openlore-indexer-lambda: DID list not rendered: {cause}");
    if !copy_usable(rendered_at, now) {
        if let Err(stuck) = remove_if_present(file) {
            eprintln!("openlore-indexer-lambda: old DID list copy not removed: {stuck}");
        }
    }
    rendered_at
}

/// `decide::dids_copy_usable`, when the copy's age is known. A copy this instance
/// never rendered, or a clock that went back, gives no age to trust.
fn copy_usable(rendered_at: Option<DateTime<Utc>>, now: DateTime<Utc>) -> bool {
    rendered_at
        .and_then(|read| (now - read).to_std().ok())
        .is_some_and(|age| dids_copy_usable(age, DIDS_COPY_MAX_AGE))
}

/// Write `list` to `file` by rename, so a reader never sees half of it.
fn render_by_rename(file: &Path, list: &str) -> std::io::Result<()> {
    if let Some(directory) = file.parent() {
        std::fs::create_dir_all(directory)?;
    }
    let staged = file.with_extension("rendering");
    std::fs::write(&staged, list)?;
    std::fs::rename(&staged, file)
}

#[cfg(test)]
#[path = "../../../tests/common/state_delta.rs"]
mod state_delta;

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, HashSet};

    use chrono::TimeDelta;
    use ports::ProbeRefusalReason;
    use proptest::prelude::*;
    use serde_json::Value;

    use super::state_delta::{assert_state_delta, set_to, Delta};
    use super::*;

    /// D-STZ-16's limit, stated here so a changed constant shows.
    const TWO_HOURS_IN_SECONDS: i64 = 2 * 60 * 60;

    fn fourteen_hundred() -> DateTime<Utc> {
        "2026-10-27T14:00:00Z".parse().expect("a fixed instant")
    }

    /// What a reader can see of the copy: the file, and the age it is held to.
    fn copy_slots(file: &Path, rendered_at: Option<DateTime<Utc>>) -> HashMap<String, Value> {
        let text = std::fs::read_to_string(file).map_or(Value::Null, Value::String);
        HashMap::from([
            ("copy.text".to_string(), text),
            (
                "copy.rendered_at".to_string(),
                json!(rendered_at.map(|at| at.to_rfc3339())),
            ),
        ])
    }

    fn copy_universe() -> HashSet<String> {
        ["copy.text", "copy.rendered_at"]
            .into_iter()
            .map(String::from)
            .collect()
    }

    fn any_exit() -> impl Strategy<Value = PassExit> {
        prop_oneof![
            Just(PassExit::Completed),
            Just(PassExit::TotalOutage),
            Just(PassExit::Failed(PassFailure::PassPanicked)),
            Just(PassExit::Failed(PassFailure::PurgeFailed)),
            Just(PassExit::Failed(PassFailure::UpsertFailed)),
            Just(PassExit::Failed(PassFailure::PassDeadlineExceeded)),
        ]
    }

    /// A few claim counts, so equal fingerprints come up as often as changed ones.
    fn any_fingerprint() -> impl Strategy<Value = Option<IndexFingerprint>> {
        prop::option::of((0..3u64).prop_map(|claims| IndexFingerprint {
            claims,
            ..IndexFingerprint::default()
        }))
    }

    proptest! {
        // D-STZ-7, D-STZ-9, D-STZ-43: only a changed store that is usable and
        // came through a pass that did not panic goes out, whatever else the
        // exit says. PA-E6's fault panics before any work, so its fingerprints
        // match and only this property reaches a panic over a changed store.
        #[test]
        fn only_a_changed_usable_store_that_no_panic_touched_is_published(
            exit in any_exit(),
            before in any_fingerprint(),
            after in any_fingerprint(),
            store_usable in any::<bool>(),
        ) {
            let changed = before.is_some() && after.is_some() && before != after;
            let panicked = exit == PassExit::Failed(PassFailure::PassPanicked);

            let publish = publish_on_change(exit, before, after, store_usable);

            prop_assert_eq!(
                publish == Publish::Publish,
                changed && store_usable && !panicked,
            );
        }

        // Any order of good and failed reads: the file is the last good list for
        // exactly two hours after that read, and only a good read restarts them.
        #[test]
        fn a_copy_lasts_two_hours_from_its_last_good_read_through_any_outage(
            passes in prop::collection::vec(
                (
                    prop_oneof![
                        Just(TWO_HOURS_IN_SECONDS),
                        Just(1),
                        0..=TWO_HOURS_IN_SECONDS + 60,
                    ],
                    any::<bool>(),
                ),
                1..8,
            ),
        ) {
            let scratch = tempfile::tempdir().expect("scratch");
            let file = scratch.path().join("config").join("repo-dids");
            let (mut now, mut rendered_at, mut last_good) = (fourteen_hundred(), None, None);

            for (pass, (seconds_later, read_is_good)) in passes.into_iter().enumerate() {
                now += TimeDelta::seconds(seconds_later);
                let before = copy_slots(&file, rendered_at);

                let expected = if read_is_good {
                    let list = format!("did:plc:list{pass}\n");
                    render_by_rename(&file, &list).expect("a render");
                    rendered_at = settle_did_list(Ok(()), &file, rendered_at, now);
                    last_good = Some(now);
                    Delta::new()
                        .with_slot("copy.text", set_to(json!(list)))
                        .with_slot("copy.rendered_at", set_to(json!(now.to_rfc3339())))
                } else {
                    rendered_at =
                        settle_did_list(Err("outage".to_string()), &file, rendered_at, now);
                    let within_the_limit = last_good.is_some_and(|read: DateTime<Utc>| {
                        (now - read).num_seconds() <= TWO_HOURS_IN_SECONDS
                    });
                    if within_the_limit {
                        Delta::new()
                    } else {
                        Delta::new().with_slot("copy.text", set_to(Value::Null))
                    }
                };

                assert_state_delta(
                    &before,
                    &copy_slots(&file, rendered_at),
                    &copy_universe(),
                    &expected,
                );
            }
        }
    }

    // bypass: D-STZ-40a's budget has three fixed regions, and the rule across all
    // of `time_left` is `publish_time_limit`'s own property.
    #[test]
    fn a_late_publish_gets_the_time_left_less_the_reserve_then_none() {
        let seconds = Duration::from_secs;

        assert_eq!(publish_limit(Duration::ZERO), seconds(35));
        assert_eq!(publish_limit(seconds(795)), seconds(35));
        assert_eq!(publish_limit(seconds(800)), seconds(30));
        assert_eq!(publish_limit(seconds(829)), seconds(1));
        assert_eq!(publish_limit(seconds(830)), Duration::ZERO);
        assert_eq!(publish_limit(seconds(900)), Duration::ZERO);
    }

    // bypass: the port's two answers and a crash are three fixed cases, not a
    // range of inputs.
    #[tokio::test]
    async fn only_the_write_probes_own_refusal_refuses_a_pass() {
        let refused = ProbeRefused {
            reason: ProbeRefusalReason::IndexObjectWriteConditionIgnored,
            detail: "both writes landed".to_string(),
            structured: json!({}),
        };
        let crashed = tokio::spawn(async { panic!("the probe crashed") })
            .await
            .expect_err("a crashed task");

        assert!(write_probe_verdict(Ok(ProbeOutcome::Ok)).is_ok());
        assert!(write_probe_verdict(Err(crashed)).is_ok());
        let verdict = write_probe_verdict(Ok(refused.into())).expect_err("a refusal");
        assert_eq!(verdict.detail, "both writes landed");
    }

    // bypass: a file this instance never rendered and a clock that went back are
    // two fixed facts that no order of reads produces.
    #[test]
    fn a_copy_of_unknown_age_is_not_used() {
        let now = fourteen_hundred();
        let unknown_ages = [
            ("never rendered here", None),
            ("the clock went back", Some(now + TimeDelta::seconds(1))),
        ];
        for (case, rendered_at) in unknown_ages {
            let scratch = tempfile::tempdir().expect("scratch");
            let file = scratch.path().join("repo-dids");
            std::fs::write(&file, "did:plc:priya\n").expect("a copy");

            let kept = settle_did_list(Err("outage".to_string()), &file, rendered_at, now);

            assert!(!file.exists(), "{case}");
            assert_eq!(kept, rendered_at, "{case}");
        }
    }

    // bypass: the two fixed outcomes of one file-system call; a pass cannot make
    // a removal in its own scratch directory fail.
    #[test]
    fn a_file_already_gone_is_removed_and_one_that_stays_is_a_failure() {
        let scratch = tempfile::tempdir().expect("scratch");

        assert!(remove_if_present(&scratch.path().join("gone")).is_ok());
        assert!(remove_if_present(scratch.path()).is_err(), "a directory");
    }

    // bypass: P-7 is one rule over a file's three states, and a real pass can
    // only ever reach the empty one, so these rows plant the others.
    #[test]
    fn a_store_is_ready_to_upload_only_when_usable_with_nothing_in_its_log() {
        let scratch = tempfile::tempdir().expect("scratch");
        let index = scratch.path().join("index.duckdb");
        let log = scratch.path().join("index.duckdb.wal");

        assert!(ready_to_upload(true, &index), "no log");
        assert!(!ready_to_upload(false, &index), "an unusable store");

        std::fs::write(&log, b"").expect("an empty log");
        assert!(ready_to_upload(true, &index), "an empty log");

        std::fs::write(&log, b"rows").expect("a log holding rows");
        assert!(!ready_to_upload(true, &index), "a log holding rows");

        let log_out_of_reach = log.join("index.duckdb");
        assert!(!ready_to_upload(true, &log_out_of_reach), "an unread log");
    }
}
