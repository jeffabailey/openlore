//! `POST /review/publish`: confirm a previewed plan (US-BRA-004). The plan
//! is taken from the owner's store exactly once, refused if its preview has
//! expired (ADR-074), re-admitted as the owner's own self-attested record,
//! executed, and the suggestion moves to published. A missing or forged
//! anti-forgery token writes nothing. A failed write puts the plan back
//! (same deadline) so Retry publishes it exactly once (ADR-071: the record
//! key is the content's CID, so a create that already landed is success).

use hyper::StatusCode;
use ports::{PlanKind, SuggestionState, TakenPublishPlan};
use review_domain::kpi::approval_was_edited;
use review_domain::plans::{plan_freshness, restore_plan, PlanFreshness, PublishPlan};
use review_domain::views::{self, PublishRetry};

use crate::executor::execute_publish;
use crate::http::{App, PageRequest, Reply};
use crate::limiter::unix_now;
use crate::routes::review::{not_found, owner_can_publish};
use crate::routes::signin::{
    csrf_matches, csrf_token_for, current_session, field, forbidden, to_landing, SESSION_COOKIE,
};
use crate::wiring::{observe, LogEvent};

/// Confirm "Publish to my repo" (or "Retry").
pub(crate) async fn confirm_publish(app: &App, request: &PageRequest) -> Reply {
    let Some((_, session)) = current_session(app, request) else {
        return to_landing();
    };
    if !csrf_matches(request, &session) {
        return forbidden();
    }
    let owner_did = session.owner_did.as_str();
    let taken = field(&request.form, "plan").and_then(|plan_id| {
        app.plans
            .take_publish_plan(owner_did, &plan_id, PlanKind::Publish)
            .ok()?
    });
    let Some(TakenPublishPlan { plan, expires_at }) = taken else {
        return not_found();
    };
    let now = i64::try_from(unix_now()).unwrap_or_default();
    if plan_freshness(expires_at, now) == PlanFreshness::Expired {
        return Reply::Page {
            status: StatusCode::GONE,
            html: views::plan_expired_page(),
            set_cookie: None,
        };
    }
    let restored = restore_plan(owner_did, &plan)
        .ok()
        .filter(|plan| owner_can_publish(app, owner_did, plan.key()));
    let csrf_token = field(&request.cookies, SESSION_COOKIE)
        .map(|cookie| csrf_token_for(&cookie))
        .unwrap_or_default();
    match restored {
        Some(plan) => publish(app, &plan, expires_at, &csrf_token).await,
        None => not_found(),
    }
}

async fn publish(app: &App, plan: &PublishPlan, expires_at: i64, csrf_token: &str) -> Reply {
    match execute_publish(app.repo_write.as_ref(), app.repo_read.as_ref(), plan).await {
        Ok(at_uri) => {
            let offered = app
                .review_read
                .pending_suggestions(plan.owner_did())
                .unwrap_or_default()
                .into_iter()
                .find(|suggestion| &suggestion.key == plan.key());
            let edited = approval_was_edited(plan.claim(), offered.as_ref());
            let moved = app.review_write.change_state(
                plan.owner_did(),
                plan.key(),
                SuggestionState::Pending,
                SuggestionState::Published,
            );
            if matches!(moved, Ok(true)) {
                observe(app, LogEvent::SuggestionApproved { edited });
            }
            Reply::Page {
                status: StatusCode::OK,
                html: views::published_page(&at_uri),
                set_cookie: None,
            }
        }
        Err(failure) => {
            // Nothing changes state; the plan goes back, same deadline.
            let kept = app
                .plans
                .put_publish_plan(plan.owner_did(), &plan.stored(), expires_at)
                .is_ok();
            let retry = PublishRetry {
                plan_id: plan.rkey(),
                csrf_token,
            };
            let next_step = match (&failure, kept) {
                (failure, _) if failure.needs_sign_in() => views::PublishNextStep::SignInAgain,
                (_, true) => views::PublishNextStep::Retry(retry),
                (_, false) => views::PublishNextStep::BackToQueue,
            };
            Reply::Page {
                status: StatusCode::BAD_GATEWAY,
                html: views::publish_failed_page(failure.message(), next_step),
                set_cookie: None,
            }
        }
    }
}
