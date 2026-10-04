//! `GET /share` and `POST /share`: the opt-in share post (US-BRA-008,
//! I-BRA-6). The preview builds a pure `SharePostPlan` from the owner's
//! claims read LIVE from their own PDS (never from private review state) and
//! keeps it in the take-once plan store; nothing is posted. Only "Post to
//! Bluesky" posts, exactly the confirmed text, once. "Don't post" or
//! leaving simply lets the plan expire. The post text is never logged.

use hyper::StatusCode;
use ports::{PlanKind, ResolvedIdentity, TakenPublishPlan};
use review_domain::plans::{plan_expires_at, plan_freshness, rfc3339_utc, PlanFreshness};
use review_domain::share::{compose_post, restore_share_plan, share_post_plan, SharePostPlan};
use review_domain::views::{self, ProfileContent, SharePreview};

use crate::executor::execute_share;
use crate::http::{App, PageRequest, Reply};
use crate::limiter::unix_now;
use crate::routes::profile::read_published;
use crate::routes::review::not_found;
use crate::routes::signin::{
    csrf_matches, csrf_token_for, current_session, field, forbidden, random_token, to_landing,
    SESSION_COOKIE,
};
use crate::wiring::{emit, LogEvent};

/// Open the share preview: the draft, its profile link, and the Post button.
pub(crate) async fn share_preview(app: &App, request: &PageRequest) -> Reply {
    let Some((cookie, session)) = current_session(app, request) else {
        return to_landing();
    };
    let Ok(identity) = app.identity.resolve_did(&session.owner_did).await else {
        return unavailable(
            StatusCode::SERVICE_UNAVAILABLE,
            views::DIRECTORY_UNREACHABLE_NOTICE,
        );
    };
    let published = match read_published(app.repo_listing.as_ref(), &identity).await {
        ProfileContent::Claims(claims) => claims,
        _ => {
            return unavailable(
                StatusCode::SERVICE_UNAVAILABLE,
                views::PDS_UNREACHABLE_NOTICE,
            )
        }
    };
    let plan = match share_post_plan(&profile_url(app, &identity), &published) {
        Ok(plan) => plan,
        Err(refusal) => return unavailable(StatusCode::OK, &refusal.message()),
    };
    let plan_id = random_token();
    let expires_at = plan_expires_at(now());
    let stored = plan.stored(&identity.did, &plan_id);
    if app
        .plans
        .put_publish_plan(&identity.did, &stored, expires_at)
        .is_err()
    {
        return unavailable(
            StatusCode::SERVICE_UNAVAILABLE,
            "We couldn't prepare your post right now. Nothing was posted; try again shortly.",
        );
    }
    preview(
        StatusCode::OK,
        &plan,
        &plan_id,
        &csrf_token_for(&cookie),
        plan.draft().text(),
        None,
    )
}

/// "Post to Bluesky": post exactly the confirmed text of a previewed plan.
pub(crate) async fn confirm_share(app: &App, request: &PageRequest) -> Reply {
    let Some((_, session)) = current_session(app, request) else {
        return to_landing();
    };
    if !csrf_matches(request, &session) {
        return forbidden();
    }
    let owner_did = session.owner_did.as_str();
    let plan_id = field(&request.form, "plan").unwrap_or_default();
    let Some(TakenPublishPlan { plan, expires_at }) = app
        .plans
        .take_publish_plan(owner_did, &plan_id, PlanKind::Share)
        .ok()
        .flatten()
    else {
        return not_found();
    };
    if plan_freshness(expires_at, now()) == PlanFreshness::Expired {
        return unavailable(
            StatusCode::GONE,
            "This preview has expired. Nothing was posted; open Share on Bluesky… again.",
        );
    }
    let Ok(share_plan) = restore_share_plan(owner_did, &plan) else {
        return not_found();
    };
    let confirmed = Confirmed {
        app,
        owner_did,
        plan_id: &plan_id,
        expires_at,
        csrf_token: &field(&request.cookies, SESSION_COOKIE)
            .map(|cookie| csrf_token_for(&cookie))
            .unwrap_or_default(),
        text: &field(&request.form, "text").unwrap_or_default(),
    };
    confirmed.post(&share_plan).await
}

/// A confirm of a taken, fresh plan with the owner's text.
struct Confirmed<'a> {
    app: &'a App,
    owner_did: &'a str,
    plan_id: &'a str,
    expires_at: i64,
    csrf_token: &'a str,
    text: &'a str,
}

impl Confirmed<'_> {
    async fn post(&self, plan: &SharePostPlan) -> Reply {
        let post = match compose_post(self.text, plan.profile_url()) {
            Ok(post) => post,
            Err(refusal) => return self.not_posted(plan, StatusCode::OK, &refusal.message()),
        };
        let created_at = rfc3339_utc(now());
        match execute_share(
            self.app.repo_write.as_ref(),
            self.owner_did,
            &post,
            &created_at,
        )
        .await
        {
            Ok(created) => {
                emit(LogEvent::SharePosted);
                Reply::Page {
                    status: StatusCode::OK,
                    html: views::share_posted_page(&created.uri, &profile_path(plan)),
                    set_cookie: None,
                }
            }
            Err(failure) => self.not_posted(
                plan,
                StatusCode::BAD_GATEWAY,
                &views::share_failed_notice(failure.message()),
            ),
        }
    }

    /// Nothing was posted: the plan goes back (same deadline) and the
    /// preview is shown again with the owner's text and why.
    fn not_posted(&self, plan: &SharePostPlan, status: StatusCode, notice: &str) -> Reply {
        let stored = plan.stored(self.owner_did, self.plan_id);
        let _ = self
            .app
            .plans
            .put_publish_plan(self.owner_did, &stored, self.expires_at);
        preview(
            status,
            plan,
            self.plan_id,
            self.csrf_token,
            self.text,
            Some(notice),
        )
    }
}

fn preview(
    status: StatusCode,
    plan: &SharePostPlan,
    plan_id: &str,
    csrf_token: &str,
    text: &str,
    notice: Option<&str>,
) -> Reply {
    Reply::Page {
        status,
        html: views::share_preview_page(&SharePreview {
            plan_id,
            csrf_token,
            text,
            profile_url: plan.profile_url(),
            profile_path: &profile_path(plan),
            notice,
        }),
        set_cookie: None,
    }
}

fn unavailable(status: StatusCode, reason: &str) -> Reply {
    Reply::Page {
        status,
        html: views::share_unavailable_page(reason),
        set_cookie: None,
    }
}

/// The owner's public profile on this app: `<origin>/@<handle>`.
fn profile_url(app: &App, identity: &ResolvedIdentity) -> String {
    format!(
        "{}/@{}",
        app.origin.trim_end_matches('/'),
        identity.verified_handle
    )
}

/// The same-origin path of the plan's profile link.
fn profile_path(plan: &SharePostPlan) -> String {
    plan.profile_url()
        .rfind("/@")
        .map(|at| plan.profile_url()[at..].to_string())
        .unwrap_or_else(|| "/review".to_string())
}

fn now() -> i64 {
    i64::try_from(unix_now()).unwrap_or_default()
}
