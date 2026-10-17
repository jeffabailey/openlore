//! "Publish this link" (slice 3, ADR-084, U-26): the verified owner's link
//! preview. A same-origin, signed-in POST spends the verify budget, checks
//! the login's shape, reads the profile itself and keeps the plan only when
//! that fresh read proves the signed-in DID (C-GDL-10, ADR-085 §2). The
//! stored verification is never the proof: this module holds no handle on
//! it. HTTP concerns and port calls only; the decision is
//! `review_domain::link_plan::link_preview`. Nothing is written to any repo.
//!
//! The confirm (03-05) takes the kind-publish plan once, admits it only as
//! the owner's link (`restore_link_plan`), spends the budget, reads the
//! profile again and writes only what `confirm_link` admits from that read.

use hyper::StatusCode;
use ports::claim_domain::identity_link::{check_link, GithubObservation};
use ports::claim_domain::Did;
use ports::{GithubAccountProfile, PlanKind, WebSession};
use review_domain::link_plan::{
    confirm_link, link_preview, restore_link_plan, LinkConfirmRefusal, LinkPlan,
};
use review_domain::ownership::{is_github_login, OwnershipRefusal};
use review_domain::plans::{plan_expires_at, rfc3339_utc};
use review_domain::views::{self, LinkNotPublished, OwnershipOutcome, GITHUB_LOGIN_FIELD};

use crate::executor::{execute_link, PublishFailure};
use crate::http::{App, PageRequest, Reply};
use crate::limiter::unix_now_secs;
use crate::routes::confirm::take_confirmed_plan;
use crate::routes::github::{github_page, refusal_of};
use crate::routes::review::not_found;
use crate::routes::signin::{csrf_token_for, field, signed_in_post};
use crate::wiring::{emit, LogEvent};

/// `POST /github/link`: the exact-record preview of the owner's link to the
/// account the offer names, or the refusal explaining why there is none.
pub(crate) async fn preview_link(app: &App, request: &PageRequest) -> Reply {
    let (cookie_value, session) = match signed_in_post(app, request) {
        Ok(signed_in) => signed_in,
        Err(refused) => return refused,
    };
    let named = field(&request.form, GITHUB_LOGIN_FIELD).unwrap_or_default();
    let login = named.trim();
    match kept_link_plan(app, &session, login).await {
        Ok(plan) => Reply::page(
            StatusCode::OK,
            views::link_preview_page(&plan, &csrf_token_for(&cookie_value)),
        ),
        Err(refusal) => {
            let refused = OwnershipOutcome::Refused(&refusal);
            let message =
                views::ownership_message(refused, login, &session.owner_did, &session.handle);
            github_page(app, &session, &cookie_value, Some((refused, &message)))
        }
    }
}

/// Spend the budget, check the login, read the profile, decide, and keep a
/// plan only if one was built. Every guard runs before GitHub is asked.
async fn kept_link_plan(
    app: &App,
    session: &WebSession,
    login: &str,
) -> Result<LinkPlan, OwnershipRefusal> {
    if !app.verify_attempts.try_spend(&session.owner_did) {
        return Err(OwnershipRefusal::TooManyAttempts);
    }
    if !is_github_login(login) {
        return Err(OwnershipRefusal::AccountNotFound);
    }
    let profile = app
        .github
        .read_account(login)
        .await
        .map_err(|error| refusal_of(&error))?;
    let now = unix_now_secs();
    let plan = link_preview(
        &session.owner_did,
        &observation_of(&profile),
        &rfc3339_utc(now),
    )?;
    app.plans
        .put_publish_plan(&session.owner_did, &plan.stored(), plan_expires_at(now))
        .map_err(|_| OwnershipRefusal::GithubUnavailable)?;
    Ok(plan)
}

/// The fresh read as the link proof sees it.
fn observation_of(profile: &GithubAccountProfile) -> GithubObservation {
    GithubObservation {
        login: profile.login.clone(),
        user_id: profile.id,
        bio: profile.bio.clone(),
    }
}

/// `POST /github/link/publish`: confirm the previewed link. The plan is
/// taken exactly once; a plan that is not the owner's link (an ordinary
/// publish plan, a tampered one) is used up and writes nothing.
pub(crate) async fn publish_link(app: &App, request: &PageRequest) -> Reply {
    let confirmed = match take_confirmed_plan(app, request, PlanKind::Publish) {
        Ok(confirmed) => confirmed,
        Err(refused) => return refused,
    };
    let Ok(plan) = restore_link_plan(&confirmed.owner_did, &confirmed.stored) else {
        return not_found();
    };
    if confirmed.expired() {
        return not_published(StatusCode::GONE, LinkNotPublished::Expired, "expired");
    }
    match proven_again(app, plan).await {
        Ok(plan) => write_link(app, &plan).await,
        Err((why, label)) => not_published(StatusCode::CONFLICT, why, label),
    }
}

/// Spend the budget, read the profile again and admit the plan only if that
/// confirm-time read proves exactly its link (C-GDL-10).
async fn proven_again(
    app: &App,
    plan: LinkPlan,
) -> Result<LinkPlan, (LinkNotPublished, &'static str)> {
    if !app.verify_attempts.try_spend(plan.owner_did()) {
        return Err((LinkNotPublished::TooManyAttempts, "too_many_attempts"));
    }
    let profile = app
        .github
        .read_account(plan.login())
        .await
        .map_err(|_| (LinkNotPublished::NotProven, "github_unavailable"))?;
    let fresh = check_link(
        &Did(plan.owner_did().to_string()),
        &observation_of(&profile),
    );
    confirm_link(plan, fresh).map_err(|refusal| (LinkNotPublished::NotProven, label_of(&refusal)))
}

fn label_of(refusal: &LinkConfirmRefusal) -> &'static str {
    match refusal {
        LinkConfirmRefusal::ProofGone(_) => "proof_gone",
        LinkConfirmRefusal::Unreadable => "unreadable",
        LinkConfirmRefusal::IdentityChanged => "identity_changed",
    }
}

/// The create-only write and its read-back; the page names the record's
/// exact address.
async fn write_link(app: &App, plan: &LinkPlan) -> Reply {
    match execute_link(app.repo_write.as_ref(), app.repo_read.as_ref(), plan).await {
        Ok(at_uri) => {
            emit(LogEvent::GithubLinkPublished);
            Reply::page(StatusCode::OK, views::link_published_page(&at_uri))
        }
        Err(failure) => {
            let (why, label) = write_failure(&failure);
            not_published(StatusCode::BAD_GATEWAY, why, label)
        }
    }
}

fn write_failure(failure: &PublishFailure) -> (LinkNotPublished, &'static str) {
    match failure {
        _ if failure.needs_sign_in() => (LinkNotPublished::SessionExpired, "session_expired"),
        PublishFailure::ReadBackMismatch => {
            (LinkNotPublished::NotAsPreviewed, "read_back_mismatch")
        }
        PublishFailure::NotWritten(_) => (LinkNotPublished::NotWritten, "not_written"),
    }
}

fn not_published(status: StatusCode, why: LinkNotPublished, label: &'static str) -> Reply {
    emit(LogEvent::GithubLinkRefused(label));
    Reply::page(status, views::link_not_published_page(why))
}
