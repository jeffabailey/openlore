//! `POST /review/publish`: confirm a previewed plan (US-BRA-004). The plan
//! is taken from the owner's store exactly once, re-admitted as the owner's
//! own self-attested record, executed, and the suggestion moves to
//! published. A missing or forged anti-forgery token writes nothing.

use hyper::StatusCode;
use ports::SuggestionState;
use review_domain::plans::{restore_plan, PublishPlan};
use review_domain::views;

use crate::executor::execute_publish;
use crate::http::{App, PageRequest, Reply};
use crate::routes::review::{not_found, owner_can_publish};
use crate::routes::signin::{csrf_matches, current_session, field, forbidden, to_landing};

/// Confirm "Publish to my repo".
pub(crate) async fn confirm_publish(app: &App, request: &PageRequest) -> Reply {
    let Some((_, session)) = current_session(app, request) else {
        return to_landing();
    };
    if !csrf_matches(request, &session) {
        return forbidden();
    }
    let owner_did = session.owner_did.as_str();
    let plan = field(&request.form, "plan")
        .and_then(|plan_id| app.plans.take_publish_plan(owner_did, &plan_id).ok()?)
        .and_then(|stored| restore_plan(owner_did, &stored).ok())
        .filter(|plan| owner_can_publish(app, owner_did, plan.key()));
    match plan {
        Some(plan) => publish(app, &plan).await,
        None => not_found(),
    }
}

async fn publish(app: &App, plan: &PublishPlan) -> Reply {
    match execute_publish(app.repo_write.as_ref(), app.repo_read.as_ref(), plan).await {
        Ok(at_uri) => {
            let _ = app.review_write.change_state(
                plan.owner_did(),
                plan.key(),
                SuggestionState::Pending,
                SuggestionState::Published,
            );
            Reply::Page {
                status: StatusCode::OK,
                html: views::published_page(&at_uri),
                set_cookie: None,
            }
        }
        Err(failure) => {
            // The plan stays available for a retry (same key: idempotent).
            let _ = app.plans.put_publish_plan(plan.owner_did(), &plan.stored());
            Reply::Page {
                status: StatusCode::BAD_GATEWAY,
                html: views::publish_failed_page(failure.message()),
                set_cookie: None,
            }
        }
    }
}
