//! Pure renderers for the `openlore publish` verbs (ADR-062). No I/O.

use ports::{RoundTripVerdict, REASON_CID_ROUNDTRIP_FAILED};

use crate::verbs::publish::{PullReport, PushResult};

/// `publish init` success: the registered instance + the UNCHANGED signing
/// identity (the instance never becomes a signing authority).
pub fn render_publish_init(instance_url: &str, author_did: &str) -> String {
    format!(
        "Registered publish target {instance_url}\n  \
         author: {author_did} (unchanged — signing stays local)\n"
    )
}

/// `publish push`: one line per claim + the committed tally.
pub fn render_publish_push(instance_url: &str, results: &[PushResult]) -> String {
    let lines: String = results.iter().map(render_push_result).collect();
    let committed = results
        .iter()
        .filter(|r| matches!(r, PushResult::Committed { .. }))
        .count();
    format!(
        "Pushing to {instance_url}\n{lines}{committed}/{} claims pushed and CID-verified\n",
        results.len()
    )
}

fn render_push_result(result: &PushResult) -> String {
    match result {
        PushResult::Committed { cid } => format!("  {} stored, CID verified\n", cid.0),
        PushResult::Rejected { cid, detail } => format!(
            "  {} REJECTED ({REASON_CID_ROUNDTRIP_FAILED}): canonicalization mismatch — {detail}\n",
            cid.0
        ),
    }
}

/// `publish pull`: one line per record + the `N/M CIDs verified` tally.
pub fn render_publish_pull(report: &PullReport) -> String {
    let lines: String = report.verdicts.iter().map(render_pull_verdict).collect();
    format!(
        "Pulled {} record(s) from {}\n{lines}{}/{} CIDs verified\n",
        report.verdicts.len(),
        report.instance_url,
        report.verified_count(),
        report.verdicts.len()
    )
}

fn render_pull_verdict(verdict: &RoundTripVerdict) -> String {
    match verdict {
        RoundTripVerdict::Verified { cid } => format!("  {} verified\n", cid.0),
        RoundTripVerdict::CidMismatch { pushed, recomputed } => format!(
            "  {} MISMATCH ({REASON_CID_ROUNDTRIP_FAILED}): recomputed {}\n",
            pushed.0, recomputed.0
        ),
    }
}

/// `publish status`: the registered target + its reachability.
pub fn render_publish_status(instance_url: &str, reachable: bool) -> String {
    let reachability = if reachable {
        "reachable"
    } else {
        "unreachable"
    };
    format!("Publish target: {instance_url}\n  status: {reachability}\n")
}
