//! Starting a scan (I-BRA-4, ADR-076 §5). A scan re-checks ownership first:
//! only a fresh [`VerifiedOwnership`] lets it go on; any other verdict marks
//! the link unverified and reads no repo.

use hyper::StatusCode;
use ports::{GithubLink, ScanStatus, WebSession};
use review_domain::ownership::{prove_ownership, OwnershipRefusal, VerifiedOwnership};
use review_domain::views;

use crate::http::{App, PageRequest, Reply};
use crate::routes::github::{account_of, refusal_of};
use crate::routes::signin::{csrf_matches, current_session, forbidden, random_token, to_landing};
use crate::wiring::{emit, LogEvent};

/// `POST /scan`: refused (sent to the GitHub step) unless a verified link
/// exists; otherwise re-verify, then scan.
pub(crate) async fn start_scan(app: &App, request: &PageRequest) -> Reply {
    let Some((_, session)) = current_session(app, request) else {
        return to_landing();
    };
    if !csrf_matches(request, &session) {
        return forbidden();
    }
    let link = app.links.github_link(&session.owner_did).ok().flatten();
    let Some(link) = link.filter(|link| link.verified) else {
        return redirect("/github");
    };
    let status = match reverify(app, &session, &link).await {
        Ok(ownership) => scan_repos(&ownership),
        Err(refusal) if refusal.disproves_ownership() => {
            let _ = app
                .links
                .mark_link_unverified(&session.owner_did, refusal.label());
            ScanStatus::OwnershipFailed
        }
        Err(OwnershipRefusal::RateLimited) => ScanStatus::RateLimited,
        Err(_) => ScanStatus::Interrupted,
    };
    emit(LogEvent::ScanFinished(status));
    let _ = app
        .scans
        .record_finished_scan(&session.owner_did, &random_token(), status);
    redirect("/review")
}

/// Read the linked profile now and prove ownership again, pinned to the
/// numeric id that was verified.
async fn reverify(
    app: &App,
    session: &WebSession,
    link: &GithubLink,
) -> Result<VerifiedOwnership, OwnershipRefusal> {
    let profile = app
        .github
        .read_person(&link.github_login)
        .await
        .map_err(|error| refusal_of(&error))?;
    prove_ownership(
        &session.owner_did,
        &account_of(&profile),
        Some(link.github_user_id),
    )
}

/// The repo stages of a scan; they can only run with proof in hand. The
/// repo listing, signal harvest and suggestions land with US-BRA-003.
fn scan_repos(_ownership: &VerifiedOwnership) -> ScanStatus {
    ScanStatus::Completed
}

/// `GET /scan/status`: the status of the person's latest scan.
pub(crate) fn scan_status(app: &App, request: &PageRequest) -> Reply {
    let Some((_, session)) = current_session(app, request) else {
        return to_landing();
    };
    let status = app
        .scans
        .latest_scan_status(&session.owner_did)
        .ok()
        .flatten()
        .map_or("none", ScanStatus::as_str);
    Reply::Page {
        status: StatusCode::OK,
        html: views::scan_status_page(status),
        set_cookie: None,
    }
}

fn redirect(location: &str) -> Reply {
    Reply::Redirect {
        location: location.to_string(),
        set_cookie: None,
    }
}
