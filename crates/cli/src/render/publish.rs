//! Pure renderers for the `openlore publish` verbs (ADR-062). No I/O.

use ports::{RoundTripVerdict, REASON_CID_ROUNDTRIP_FAILED};
use publish_domain::Reconciled;

use crate::verbs::publish::{InsertResult, PullReport, PushReport, PushResult};

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

/// `publish pull`: one line per record, the `N/M CIDs verified` tally, the
/// reconcile summary `matched: M/P, new: N, conflicts: C, rejected: R,
/// overwritten: 0`, the insert summary `inserted: K/S, foreign (not
/// inserted): F` (S = own-author New records selected) with a line per record
/// NOT inserted, a `conflict:` line per genuine conflict naming the record and
/// BOTH CIDs (local kept, nothing auto-resolved), and — when every pulled
/// record is already held locally —
/// the in-sync confirmation. `overwritten` is always 0: the reconcile ADT has
/// no overwrite outcome (D-6).
pub fn render_publish_pull(report: &PullReport) -> String {
    let lines: String = report.verdicts.iter().map(render_pull_verdict).collect();
    let pulled = report.reconciled.len();
    let tally = publish_domain::tally_reconcile(&report.reconciled);
    let in_sync = if tally.in_sync() {
        format!(
            "Local store in sync with {}: nothing new, nothing overwritten\n",
            report.instance_url
        )
    } else {
        String::new()
    };
    let not_inserted: String = report
        .inserts
        .iter()
        .filter_map(render_insert_refusal)
        .chain(report.foreign.iter().map(|foreign| {
            format!(
                "  {} not inserted: foreign author {}\n",
                foreign.cid.0, foreign.author_did
            )
        }))
        .collect();
    let conflicts: String = report
        .reconciled
        .iter()
        .filter_map(render_conflict)
        .collect();
    format!(
        "Pulled {} record(s) from {}\n{lines}{}/{} CIDs verified\n\
         matched: {}/{pulled}, new: {}, conflicts: {}, rejected: {}, overwritten: 0\n\
         inserted: {}/{}, foreign (not inserted): {}\n{not_inserted}{conflicts}{in_sync}",
        report.verdicts.len(),
        report.instance_url,
        report.verified_count(),
        report.verdicts.len(),
        tally.matched,
        tally.new,
        tally.conflicts,
        tally.rejected,
        report.inserted_count(),
        report.inserts.len(),
        report.foreign.len(),
    )
}

/// A conflict line naming the logical record and BOTH CIDs — surfaced for
/// the user, never auto-resolved (D-6).
fn render_conflict(outcome: &Reconciled) -> Option<String> {
    match outcome {
        Reconciled::Conflict {
            pulled,
            local,
            record,
        } => Some(format!(
            "  conflict: {} {} {}: local {} vs instance {} (local kept, not overwritten)\n",
            record.subject, record.predicate, record.object, local.0, pulled.0
        )),
        _ => None,
    }
}

fn render_insert_refusal(result: &InsertResult) -> Option<String> {
    match result {
        InsertResult::Inserted { .. } => None,
        InsertResult::SignatureInvalid { cid } => Some(format!(
            "  {} not inserted: signature does not verify against the local identity\n",
            cid.0
        )),
    }
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
