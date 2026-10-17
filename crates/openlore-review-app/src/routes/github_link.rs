//! "Publish this link" (slice 3, ADR-084, U-26): the verified owner's link
//! preview. A same-origin, signed-in POST spends the verify budget, checks
//! the login's shape, reads the profile itself and keeps the plan only when
//! that fresh read proves the signed-in DID (C-GDL-10, ADR-085 §2). The
//! stored verification is never the proof: this module holds no handle on
//! it. HTTP concerns and port calls only; the decision is
//! `review_domain::link_plan::link_preview`. Nothing is written to any repo.

use hyper::StatusCode;
use ports::claim_domain::identity_link::GithubObservation;
use ports::{GithubAccountProfile, WebSession};
use review_domain::link_plan::{link_preview, LinkPlan};
use review_domain::ownership::{is_github_login, OwnershipRefusal};
use review_domain::plans::{plan_expires_at, rfc3339_utc};
use review_domain::views::{self, OwnershipOutcome, GITHUB_LOGIN_FIELD};

use crate::http::{App, PageRequest, Reply};
use crate::limiter::unix_now_secs;
use crate::routes::github::{github_page, refusal_of};
use crate::routes::signin::{csrf_token_for, field, signed_in_post};

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
