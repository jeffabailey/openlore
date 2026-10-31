//! The no-publish arm of the pass (D-STZ-44): a full run that skips the publish.
//! It lives in its own module so xtask `lambda_no_publish_is_read_only` can check
//! it, and it takes the read port only, so it has no path to a write.
//
// SCAFFOLD: true  (DISTILL 2026-10-30)

use ports::IndexObjectReadPort;

use crate::Environment;

/// What a no-publish run found, for the reply and its `pass_summary`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoPublishReport {
    pub pass_id: String,
    pub exit_code: i32,
    pub would_publish: bool,
}

/// Run the pass work against a copy of the index and report what a normal pass
/// would have published.
pub async fn run_no_publish(read: &dyn IndexObjectReadPort, env: &Environment) -> NoPublishReport {
    let _ = (read, env);
    panic!("Not yet implemented -- RED scaffold")
}
