//! Pass mode (ADR-090 §4): in the D-STZ-40 order, render the DID list from the
//! parameter store, fetch the starting index, run the reused ADR-077/078 pass
//! off the async runtime (D-STZ-42, UI-11) and take its exit, publish on change
//! under a version condition (D-STZ-6, D-STZ-7), write `last-pass.json`, then
//! write the ONE `pass_summary`.
//!
//! Thin for now, each closed by a named step: init ignores what the write probe
//! found (T5, 03-01); the pass ignores its event (T2, 03-02). A no-publish run
//! will stop after the pass work: that arm is `no_publish::run_no_publish`, which
//! takes the read port only, so it has no path to a write (D-STZ-44).

use std::ffi::OsString;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;

use appview_domain::ingest_pass::PassExit;
use appview_domain::pass_runner::PassId;
use chrono::SecondsFormat;
use openlore_indexer::config::{IndexerConfig, REPO_DIDS_FILE_VAR};
use openlore_indexer::events::Events;
use openlore_indexer::ingest_pass::{write_pass_summary, EndedPass};
use openlore_indexer::run::Wired;
use openlore_indexer::runner_contract::{BootNonce, PassLabel};
use ports::{
    IndexFingerprint, LastPass, ObjectVersion, PublishMetadata, PublishOutcome, WriteCondition,
};
use serde_json::json;

use crate::decide::{publish_decision, Publish};
use crate::{events_through, pass_wiring, Environment, FunctionError, InitRefused, PassDriven};

/// The reused pass's work, and whether its store is to be published.
struct WorkedPass {
    ended: EndedPass,
    publish: Publish,
}

/// The index a pass published.
struct PublishedIndex {
    bytes: u64,
    published_at: String,
}

/// One pass instance (warm across scheduled events).
pub struct PassFunction {
    driven: PassDriven,
    config: IndexerConfig,
    did_list_file: PathBuf,
    boot: BootNonce,
    passes_started: AtomicU64,
}

impl PassFunction {
    /// Lambda init: read the configuration, then run the write probe, whose
    /// marker the bucket keeps (D-STZ-39). The pass reads its list only from the
    /// file it renders, so a configuration with no such file is refused.
    pub async fn init(env: &Environment, driven: PassDriven) -> Result<Self, InitRefused> {
        let config = env.indexer_config()?;
        let did_list_file = config.repo_dids_file.clone().ok_or_else(|| InitRefused {
            reason: format!("{REPO_DIDS_FILE_VAR} is not set"),
        })?;
        run_write_probe(&driven).await?;
        Ok(Self {
            driven,
            config,
            did_list_file,
            boot: BootNonce::now(),
            passes_started: AtomicU64::new(0),
        })
    }

    /// Run one pass for one event. `Ok` carries `{pass_id, exit_code, published,
    /// index_bytes}`; `index_bytes` is `null` when nothing was published.
    pub async fn handle(
        &self,
        event: serde_json::Value,
    ) -> Result<serde_json::Value, FunctionError> {
        let _ = event;
        let started = Instant::now();
        let pass = self.next_pass();
        self.render_did_list().await;
        let seen = self.fetch_starting_index().await;
        let WorkedPass { ended, publish } = self.pass_work(pass, started).await?;
        let events = events_through(self.driven.events.clone());
        let published = match publish {
            Publish::Publish => self.publish(pass, seen.as_ref(), &events).await,
            Publish::Skip => None,
        };
        self.record(pass, ended.exit(), published.as_ref(), seen.as_ref())
            .await;
        let exit_code = write_pass_summary(&events, pass, &ended, ended.exit(), started);
        Ok(json!({
            "pass_id": pass.to_string(),
            "exit_code": exit_code,
            "published": published.is_some(),
            "index_bytes": published.map(|index| index.bytes),
        }))
    }

    fn next_pass(&self) -> PassLabel {
        let sequence = self.passes_started.fetch_add(1, Ordering::SeqCst) + 1;
        PassLabel::new(self.boot, PassId(sequence))
    }

    /// Render the operator's list where the reused pass reads it (D-STZ-16). A
    /// list that cannot be rendered leaves the file as it was, and the pass's
    /// own list refusal decides from there (ADR-081).
    async fn render_did_list(&self) {
        let rendered = match self.driven.repo_dids.current().await {
            Ok(list) => render_by_rename(&self.did_list_file, &list).map_err(|e| e.to_string()),
            Err(unavailable) => Err(unavailable.to_string()),
        };
        if let Err(cause) = rendered {
            eprintln!("openlore-indexer-lambda: DID list not rendered: {cause}");
        }
    }

    /// Replace the local copy with the current object. `fetch_to` wants a new
    /// file, and a warm instance's last copy must not outlive a reset of the
    /// bucket. A failed fetch saw no object (D-STZ-48).
    async fn fetch_starting_index(&self) -> Option<ObjectVersion> {
        let index_path = &self.config.index_path;
        if let Err(stuck) = remove_if_present(index_path) {
            eprintln!("openlore-indexer-lambda: last index copy not removed: {stuck}");
        }
        self.driven
            .index_read
            .fetch_to(index_path)
            .await
            .inspect_err(|failed| eprintln!("openlore-indexer-lambda: not fetched: {failed}"))
            .ok()
            .flatten()
    }

    /// The reused pass, off the async runtime: it blocks on a runtime of its own
    /// (UI-11). Its store is closed when this returns.
    async fn pass_work(
        &self,
        pass: PassLabel,
        started: Instant,
    ) -> Result<WorkedPass, FunctionError> {
        let config = self.config.clone();
        let index_path = config.index_path.clone();
        let sink = self.driven.events.clone();
        tokio::task::spawn_blocking(move || {
            pass_wiring(config, sink).map(|wired| work_on(wired, pass, started, &index_path))
        })
        .await
        .map_err(|crashed| FunctionError {
            message: format!("pass task: {crashed}"),
        })?
    }

    /// Publish the closed index file over the version this pass saw, stamped
    /// with this function's clock (D-STZ-6, D-STZ-12), and log its size for the
    /// size alarm (D-STZ-33). `None` when it did not land.
    async fn publish(
        &self,
        pass: PassLabel,
        seen: Option<&ObjectVersion>,
        events: &Events,
    ) -> Option<PublishedIndex> {
        let published_at = self.now_stamp();
        let metadata = PublishMetadata {
            published_at: published_at.clone(),
            pass_id: pass.to_string(),
        };
        let outcome = self
            .driven
            .index_write
            .publish_if_match(&self.config.index_path, publish_condition(seen), metadata)
            .await
            .inspect_err(|failed| eprintln!("openlore-indexer-lambda: not published: {failed}"));
        let Ok(PublishOutcome::Published(version)) = outcome else {
            return None;
        };
        events.emit_in(
            Some(pass),
            json!({"event": "indexer.lambda.published", "index_bytes": version.size_bytes}),
        );
        Some(PublishedIndex {
            bytes: version.size_bytes,
            published_at,
        })
    }

    /// Write `last-pass.json`, every pass (U-STZ-3). What a failed write means
    /// for the exit is `decide::final_exit` (02-03).
    async fn record(
        &self,
        pass: PassLabel,
        exit: PassExit,
        published: Option<&PublishedIndex>,
        seen: Option<&ObjectVersion>,
    ) {
        let record = LastPass {
            pass_id: pass.to_string(),
            exit_code: exit.code(),
            finished_at: self.now_stamp(),
            published: published.is_some(),
            index_published_at: index_published_at(published, seen),
        };
        let written = self.driven.index_write.put_last_pass(&record).await;
        if let Err(failed) = written {
            eprintln!("openlore-indexer-lambda: pass not recorded: {failed}");
        }
    }

    fn now_stamp(&self) -> String {
        self.driven
            .clock
            .now_utc()
            .to_rfc3339_opts(SecondsFormat::Secs, true)
    }
}

/// The write probe, off the async runtime: it is sync and blocks (D-STZ-39,
/// UI-11). What it found is not acted on until 03-01 (T5).
async fn run_write_probe(driven: &PassDriven) -> Result<(), InitRefused> {
    let index_write = Arc::clone(&driven.index_write);
    tokio::task::spawn_blocking(move || index_write.probe())
        .await
        .map(drop)
        .map_err(|crashed| InitRefused {
            reason: format!("write probe task: {crashed}"),
        })
}

/// One pass's work between two fingerprints, ending with the store closed. The
/// store is checked closed, because a close is what could still fail to fold the
/// log into the file (P-7).
fn work_on(wired: Wired, pass: PassLabel, started: Instant, index_path: &Path) -> WorkedPass {
    let store = Arc::clone(&wired.wiring.store_condition);
    let before = store.fingerprint().ok();
    let ended = wired.pass_work(pass, started);
    let after = store.fingerprint().ok();
    let store_usable = store.unusable_reason().is_none();
    drop(store);
    let publish = publish_on_change(
        ended.exit(),
        before,
        after,
        ready_to_upload(store_usable, index_path),
    );
    WorkedPass { ended, publish }
}

/// `decide::publish_decision`, when both fingerprints could be read. A store that
/// cannot say what it holds is never published.
fn publish_on_change(
    exit: PassExit,
    before: Option<IndexFingerprint>,
    after: Option<IndexFingerprint>,
    store_usable: bool,
) -> Publish {
    before.zip(after).map_or(Publish::Skip, |(before, after)| {
        publish_decision(exit, before, after, store_usable)
    })
}

/// D-STZ-6: on the version the pass fetched, or on no object existing when it
/// fetched none.
fn publish_condition(seen: Option<&ObjectVersion>) -> WriteCondition {
    seen.map_or(WriteCondition::IfNoneMatch, |version| {
        WriteCondition::IfMatch(version.etag.clone())
    })
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
mod tests {
    use super::*;

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
