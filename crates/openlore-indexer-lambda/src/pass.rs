//! Pass mode (ADR-090 §4): in the D-STZ-40 order, render the DID list from the
//! parameter store, run the reused ADR-077/078 pass off the async runtime
//! (D-STZ-42, UI-11) and take its exit, publish the index stamped with this
//! function's clock, then write the ONE `pass_summary`.
//!
//! Thin for now, each closed by a named step: the pass publishes whenever it ran,
//! under the none-exists condition, so only a first object lands (T1, 01-10); it
//! ignores its event (T2, 03-02). A no-publish run will stop after the pass work:
//! that arm is `no_publish::run_no_publish`, which takes the read port only, so
//! it has no path to a write (D-STZ-44).

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use appview_domain::pass_runner::PassId;
use chrono::SecondsFormat;
use openlore_indexer::config::{IndexerConfig, REPO_DIDS_FILE_VAR};
use openlore_indexer::ingest_pass::{write_pass_summary, EndedPass};
use openlore_indexer::runner_contract::{BootNonce, PassLabel};
use ports::{PublishMetadata, PublishOutcome, WriteCondition};
use serde_json::json;

use crate::{events_through, pass_wiring, Environment, FunctionError, InitRefused, PassDriven};

/// One pass instance (warm across scheduled events).
pub struct PassFunction {
    driven: PassDriven,
    config: IndexerConfig,
    did_list_file: PathBuf,
    boot: BootNonce,
    passes_started: AtomicU64,
}

impl PassFunction {
    /// Lambda init: read the configuration. The pass reads its list only from
    /// the file it renders, so a configuration with no such file is refused.
    pub async fn init(env: &Environment, driven: PassDriven) -> Result<Self, InitRefused> {
        let config = env.indexer_config()?;
        let did_list_file = config.repo_dids_file.clone().ok_or_else(|| InitRefused {
            reason: format!("{REPO_DIDS_FILE_VAR} is not set"),
        })?;
        Ok(Self {
            driven,
            config,
            did_list_file,
            boot: BootNonce::now(),
            passes_started: AtomicU64::new(0),
        })
    }

    /// Run one pass for one event. `Ok` carries `{pass_id, exit_code, published}`.
    pub async fn handle(
        &self,
        event: serde_json::Value,
    ) -> Result<serde_json::Value, FunctionError> {
        let _ = event;
        let started = Instant::now();
        let pass = self.next_pass();
        self.render_did_list().await;
        let ended = self.pass_work(pass, started).await?;
        let published = self.publish(pass).await;
        let events = events_through(self.driven.events.clone());
        let exit_code = write_pass_summary(&events, pass, &ended, ended.exit(), started);
        Ok(json!({
            "pass_id": pass.to_string(),
            "exit_code": exit_code,
            "published": published,
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

    /// The reused pass, off the async runtime: it blocks on a runtime of its own
    /// (UI-11). Its wiring is gone when it returns, so the store is closed.
    async fn pass_work(
        &self,
        pass: PassLabel,
        started: Instant,
    ) -> Result<EndedPass, FunctionError> {
        let config = self.config.clone();
        let sink = self.driven.events.clone();
        tokio::task::spawn_blocking(move || {
            pass_wiring(config, sink).map(|wired| wired.pass_work(pass, started))
        })
        .await
        .map_err(|crashed| FunctionError {
            message: format!("pass task: {crashed}"),
        })?
    }

    /// Publish the closed index file, stamped with this function's clock
    /// (D-STZ-12). True when it landed.
    async fn publish(&self, pass: PassLabel) -> bool {
        let metadata = PublishMetadata {
            published_at: self.published_at(),
            pass_id: pass.to_string(),
        };
        let outcome = self
            .driven
            .index_write
            .publish_if_match(
                &self.config.index_path,
                WriteCondition::IfNoneMatch,
                metadata,
            )
            .await
            .inspect_err(|failed| eprintln!("openlore-indexer-lambda: not published: {failed}"));
        matches!(outcome, Ok(PublishOutcome::Published(_)))
    }

    fn published_at(&self) -> String {
        self.driven
            .clock
            .now_utc()
            .to_rfc3339_opts(SecondsFormat::Secs, true)
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
