//! Starting a scan (I-BRA-4, ADR-076 §2/§5). A scan starts only with a
//! verified link and the budget's admission; it then runs in the background
//! (`crate::scan`), and the queue polls its status.

use std::sync::Arc;

use hyper::StatusCode;
use ports::ScanStatus;
use review_domain::views::{self, QueueNotice, ScanRefused};

use crate::http::{App, PageRequest, Reply};
use crate::routes::review::queue_page;
use crate::routes::signin::{current_session, signed_in_post, to_landing};
use crate::scan::{begin_scan, ScanStart};

/// `POST /scan`: refused (sent to the GitHub step) unless a verified link
/// exists; refused with the reason when the budget says no; otherwise the
/// scan starts in the background and the queue shows its progress.
pub(crate) async fn start_scan(app: &Arc<App>, request: &PageRequest) -> Reply {
    let (cookie_value, session) = match signed_in_post(app, request) {
        Ok(signed_in) => signed_in,
        Err(refused) => return refused,
    };
    let refused = |status, refused| {
        queue_page(
            app,
            &cookie_value,
            &session,
            status,
            Some(QueueNotice::ScanRefused(refused)),
        )
    };
    let not_started = |notice| {
        queue_page(
            app,
            &cookie_value,
            &session,
            StatusCode::UNPROCESSABLE_ENTITY,
            Some(notice),
        )
    };
    match begin_scan(app, &session.owner_did) {
        ScanStart::Started | ScanStart::AlreadyScanning => Reply::redirect("/review"),
        ScanStart::NotVerified => Reply::redirect("/github"),
        ScanStart::Refused(why) => refused(StatusCode::TOO_MANY_REQUESTS, why),
        ScanStart::Unavailable => refused(StatusCode::SERVICE_UNAVAILABLE, ScanRefused::AppBusy),
        ScanStart::NothingToRead => not_started(QueueNotice::NothingToScan),
        ScanStart::OverLimit(refusal) => not_started(QueueNotice::ScanOverLimit(refusal)),
    }
}

/// `GET /scan/status`: the status of the person's latest scan.
pub(crate) fn scan_status(app: &App, request: &PageRequest) -> Reply {
    let Some((_, session)) = current_session(app, request) else {
        return to_landing();
    };
    let status = app
        .scans
        .latest_scan(&session.owner_did)
        .ok()
        .flatten()
        .map_or("none", |run| ScanStatus::as_str(run.status));
    Reply::page(StatusCode::OK, views::scan_status_page(status))
}
