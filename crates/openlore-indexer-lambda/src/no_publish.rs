//! The no-publish arm of the pass (D-STZ-44): a full run that skips the publish
//! and the record, and reports what a normal pass would have published. It lives
//! in its own module so xtask `lambda_no_publish_is_read_only` can check it, and
//! it takes the read port only, so it has no path to a write.

use std::sync::Arc;
use std::time::Instant;

use openlore_indexer::config::IndexerConfig;
use openlore_indexer::ingest_pass::write_no_publish_pass_summary;
use openlore_indexer::runner_contract::PassLabel;
use ports::IndexObjectReadPort;
use serde_json::json;

use crate::decide::Publish;
use crate::pass::{work_from_the_starting_index, WorkedPass};
use crate::{events_through, EventSink, FunctionError};

/// What a no-publish run found, for the reply and its `pass_summary`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoPublishReport {
    pub pass_id: String,
    pub exit_code: i32,
    pub would_publish: bool,
}

impl NoPublishReport {
    /// The usual payload, which published nothing, plus `mode` and `would_publish`.
    #[must_use]
    pub fn reply(&self) -> serde_json::Value {
        json!({
            "pass_id": self.pass_id,
            "exit_code": self.exit_code,
            "published": false,
            "index_bytes": null,
            "mode": "no-publish",
            "would_publish": self.would_publish,
        })
    }
}

/// Run the pass work against a copy of the index, write its one summary, and
/// report what a normal pass would have published. As in a normal pass, a crash
/// is a function error, and no usable starting index is a failed pass that would
/// publish nothing (D-STZ-41).
pub async fn run_no_publish(
    read: &dyn IndexObjectReadPort,
    config: &IndexerConfig,
    sink: Arc<dyn EventSink>,
    pass: PassLabel,
    started: Instant,
) -> Result<NoPublishReport, FunctionError> {
    let WorkedPass { ended, publish, .. } =
        work_from_the_starting_index(read, config, Arc::clone(&sink), pass, started).await?;
    let exit_code = write_no_publish_pass_summary(&events_through(sink), pass, &ended, started);
    Ok(NoPublishReport {
        pass_id: pass.to_string(),
        exit_code,
        would_publish: publish == Publish::Publish,
    })
}
