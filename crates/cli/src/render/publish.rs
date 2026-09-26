//! Pure renderers for the `openlore publish` verbs (ADR-062). No I/O.

use ports::{RoundTripVerdict, REASON_CID_ROUNDTRIP_FAILED};

use crate::verbs::publish::{PullReport, PushReport, PushResult};

/// `publish init` success: the registered instance, the UNCHANGED signing
/// identity, the derived public card URL, and the ownership confirmation —
/// the instance is the user's own, with no central authority in the trust
/// path (D-1/D-4) and never a signing authority (D-6/D-7).
pub fn render_publish_init(instance_url: &str, author_did: &str, card_url: &str) -> String {
    format!(
        "Registered publish target {instance_url}\n  \
         instance_url: {instance_url}\n  \
         author_did: {author_did} (unchanged — signing stays local)\n  \
         card_url: {card_url}\n\
         This instance is yours: no central authority in the trust path.\n"
    )
}

/// `publish push`: one line per claim sent, the committed tally, and the
/// `pushed: P, skipped: S, verified: V/T` summary (skipped = already listed
/// in the instance's manifest, so never sent).
pub fn render_publish_push(report: &PushReport) -> String {
    let lines: String = report.results.iter().map(render_push_result).collect();
    let committed = report.committed_count();
    let sent = report.results.len();
    format!(
        "Pushing to {}\n{lines}{committed}/{sent} claims pushed and CID-verified\n\
         pushed: {committed}, skipped: {}, verified: {committed}/{sent}\n",
        report.instance_url, report.skipped
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

/// `publish status`: the registered target, its derived public card URL, and
/// whether the instance is reachable right now.
pub fn render_publish_status(instance_url: &str, card_url: &str, reachable: bool) -> String {
    let reachability = if reachable {
        "reachable"
    } else {
        "unreachable"
    };
    format!(
        "Publish target {instance_url}\n  \
         instance_url: {instance_url}\n  \
         card_url: {card_url}\n  \
         reachability: {reachability}\n"
    )
}
