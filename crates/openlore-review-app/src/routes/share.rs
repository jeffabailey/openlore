//! `GET /share` and `POST /share`: the opt-in share post (US-BRA-008,
//! I-BRA-6). The preview builds a pure `SharePostPlan` from the owner's
//! claims read LIVE from their own PDS (never from private review state) and
//! keeps it in the take-once plan store; nothing is posted. Only "Post to
//! Bluesky" posts, exactly the confirmed text, once. "Don't post" or
//! leaving simply lets the plan expire. The post text is never logged.

use hyper::StatusCode;
use ports::{PlanKind, ResolvedIdentity};
use review_domain::plans::{plan_expires_at, rfc3339_utc};
use review_domain::share::{compose_post, restore_share_plan, share_post_plan, SharePostPlan};
use review_domain::views::{self, ProfileContent, SharePreview};

use crate::executor::execute_share;
use crate::http::{App, PageRequest, Reply};
use crate::limiter::unix_now_secs;
use crate::routes::confirm::{take_confirmed_plan, ConfirmedPlan};
use crate::routes::profile::read_published;
use crate::routes::review::not_found;
use crate::routes::signin::{csrf_token_for, current_session, field, random_token, to_landing};
use crate::wiring::{observe, LogEvent};

/// What a preview that could not be kept is told.
const NOT_PREPARED_NOTICE: &str =
    "We couldn't prepare your post right now. Nothing was posted; try again shortly.";

/// What a confirm that arrived after its preview's deadline is told.
const EXPIRED_NOTICE: &str =
    "This preview has expired. Nothing was posted; open Share on Bluesky… again.";

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
    let ProfileContent::Claims(published) =
        read_published(app.repo_listing.as_ref(), &identity).await
    else {
        return unavailable(
            StatusCode::SERVICE_UNAVAILABLE,
            views::PDS_UNREACHABLE_NOTICE,
        );
    };
    let plan = match share_post_plan(&profile_url(app, &identity), &published) {
        Ok(plan) => plan,
        Err(refusal) => return unavailable(StatusCode::OK, &refusal.message()),
    };
    let plan_id = random_token();
    let stored = plan.stored(&identity.did, &plan_id);
    if app
        .plans
        .put_publish_plan(&identity.did, &stored, plan_expires_at(unix_now_secs()))
        .is_err()
    {
        return unavailable(StatusCode::SERVICE_UNAVAILABLE, NOT_PREPARED_NOTICE);
    }
    preview(
        StatusCode::OK,
        &plan,
        &SharePreviewState {
            plan_id: &plan_id,
            csrf_token: &csrf_token_for(&cookie),
            text: plan.draft().text(),
        },
        None,
    )
}

/// "Post to Bluesky": post exactly the confirmed text of a previewed plan.
pub(crate) async fn confirm_share(app: &App, request: &PageRequest) -> Reply {
    let confirmed = match take_confirmed_plan(app, request, PlanKind::Share) {
        Ok(confirmed) => confirmed,
        Err(refused) => return refused,
    };
    if confirmed.expired() {
        return unavailable(StatusCode::GONE, EXPIRED_NOTICE);
    }
    let Ok(plan) = restore_share_plan(&confirmed.owner_did, &confirmed.stored) else {
        return not_found();
    };
    let share = Share {
        app,
        plan: &plan,
        confirmed: &confirmed,
        text: &field(&request.form, "text").unwrap_or_default(),
    };
    share.post().await
}

/// A taken, fresh share plan with the owner's confirmed text.
struct Share<'a> {
    app: &'a App,
    plan: &'a SharePostPlan,
    confirmed: &'a ConfirmedPlan,
    text: &'a str,
}

impl Share<'_> {
    async fn post(&self) -> Reply {
        let post = match compose_post(self.text, self.plan.profile_url()) {
            Ok(post) => post,
            Err(refusal) => return self.not_posted(StatusCode::OK, &refusal.message()),
        };
        let created_at = rfc3339_utc(unix_now_secs());
        let owner_did = &self.confirmed.owner_did;
        match execute_share(self.app.repo_write.as_ref(), owner_did, &post, &created_at).await {
            Ok(created) => {
                observe(self.app, LogEvent::SharePosted);
                Reply::page(
                    StatusCode::OK,
                    views::share_posted_page(&created.uri, &profile_path(self.plan)),
                )
            }
            Err(failure) => self.not_posted(
                StatusCode::BAD_GATEWAY,
                &views::share_failed_notice(failure.message()),
            ),
        }
    }

    /// Nothing was posted: the plan goes back (same deadline) and the
    /// preview is shown again with the owner's text and why.
    fn not_posted(&self, status: StatusCode, notice: &str) -> Reply {
        let confirmed = self.confirmed;
        let stored = self.plan.stored(&confirmed.owner_did, &confirmed.plan_id);
        let _ =
            self.app
                .plans
                .put_publish_plan(&confirmed.owner_did, &stored, confirmed.expires_at);
        preview(
            status,
            self.plan,
            &SharePreviewState {
                plan_id: &confirmed.plan_id,
                csrf_token: &confirmed.csrf_token,
                text: self.text,
            },
            Some(notice),
        )
    }
}

/// What the preview form re-submits: the plan, the token and the text.
struct SharePreviewState<'a> {
    plan_id: &'a str,
    csrf_token: &'a str,
    text: &'a str,
}

fn preview(
    status: StatusCode,
    plan: &SharePostPlan,
    state: &SharePreviewState<'_>,
    notice: Option<&str>,
) -> Reply {
    Reply::page(
        status,
        views::share_preview_page(&SharePreview {
            plan_id: state.plan_id,
            csrf_token: state.csrf_token,
            text: state.text,
            profile_url: plan.profile_url(),
            profile_path: &profile_path(plan),
            notice,
        }),
    )
}

fn unavailable(status: StatusCode, reason: &str) -> Reply {
    Reply::page(status, views::share_unavailable_page(reason))
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
