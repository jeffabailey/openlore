//! Starting a scan (I-BRA-4, ADR-076 §2/§5). A scan starts only with a
//! verified link and the budget's admission; it then runs in the background
//! (`crate::scan`), and the queue polls its status.

use std::sync::Arc;

use hyper::StatusCode;
use ports::ScanStatus;
use review_domain::budget::ScanAdmission;
use review_domain::views::{self, QueueNotice, ScanRefused};

use crate::http::{App, PageRequest, Reply};
use crate::routes::review::queue_page;
use crate::routes::signin::{current_session, random_token, signed_in_post, to_landing};
use crate::scan::run_scan;

/// `POST /scan`: refused (sent to the GitHub step) unless a verified link
/// exists; refused with the reason when the budget says no; otherwise the
/// scan starts in the background and the queue shows its progress.
pub(crate) async fn start_scan(app: &Arc<App>, request: &PageRequest) -> Reply {
    let (cookie_value, session) = match signed_in_post(app, request) {
        Ok(signed_in) => signed_in,
        Err(refused) => return refused,
    };
    let link = app.links.github_link(&session.owner_did).ok().flatten();
    let Some(link) = link.filter(|link| link.verified) else {
        return Reply::redirect("/github");
    };
    let refused = match app.scan_limiter.admit(&session.owner_did) {
        ScanAdmission::Admitted => None,
        ScanAdmission::AlreadyScanning => return Reply::redirect("/review"),
        ScanAdmission::DailyLimitReached => Some(ScanRefused::DailyLimitReached),
        ScanAdmission::AppBusy => Some(ScanRefused::AppBusy),
    };
    if let Some(refused) = refused {
        return queue_page(
            app,
            &cookie_value,
            &session,
            StatusCode::TOO_MANY_REQUESTS,
            Some(QueueNotice::ScanRefused(refused)),
        );
    }
    let run_id = random_token();
    if app.scans.start_scan(&session.owner_did, &run_id).is_err() {
        app.scan_limiter.release(&session.owner_did);
        return queue_page(
            app,
            &cookie_value,
            &session,
            StatusCode::SERVICE_UNAVAILABLE,
            Some(QueueNotice::ScanRefused(ScanRefused::AppBusy)),
        );
    }
    tokio::spawn(run_scan(
        app.clone(),
        session.owner_did.clone(),
        run_id,
        link,
    ));
    Reply::redirect("/review")
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
