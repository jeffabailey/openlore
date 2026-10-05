//! `POST /review/publish`: confirm a previewed plan (US-BRA-004). The plan
//! is taken from the owner's store exactly once, refused if its preview has
//! expired (ADR-074), re-admitted as the owner's own self-attested record,
//! executed, and the suggestion moves to published. A missing or forged
//! anti-forgery token writes nothing. A failed write puts the plan back
//! (same deadline) so Retry publishes it exactly once (ADR-071: the record
//! key is the content's CID, so a create that already landed is success).

use hyper::StatusCode;
use ports::{PlanKind, SuggestionState};
use review_domain::kpi::approval_was_edited;
use review_domain::plans::{restore_plan, PublishPlan};
use review_domain::views::{self, PublishNextStep, PublishRetry};

use crate::executor::execute_publish;
use crate::http::{App, PageRequest, Reply};
use crate::routes::confirm::{take_confirmed_plan, ConfirmedPlan};
use crate::routes::review::{not_found, owner_can_publish};
use crate::wiring::{observe, LogEvent};

/// Confirm "Publish to my repo" (or "Retry").
pub(crate) async fn confirm_publish(app: &App, request: &PageRequest) -> Reply {
    let confirmed = match take_confirmed_plan(app, request, PlanKind::Publish) {
        Ok(confirmed) => confirmed,
        Err(refused) => return refused,
    };
    if confirmed.expired() {
        return Reply::page(StatusCode::GONE, views::plan_expired_page());
    }
    let owner_did = confirmed.owner_did.as_str();
    let restored = restore_plan(owner_did, &confirmed.stored)
        .ok()
        .filter(|plan| owner_can_publish(app, owner_did, plan.key()));
    match restored {
        Some(plan) => publish(app, &plan, &confirmed).await,
        None => not_found(),
    }
}

async fn publish(app: &App, plan: &PublishPlan, confirmed: &ConfirmedPlan) -> Reply {
    match execute_publish(app.repo_write.as_ref(), app.repo_read.as_ref(), plan).await {
        Ok(at_uri) => {
            mark_published(app, plan);
            Reply::page(StatusCode::OK, views::published_page(&at_uri))
        }
        Err(failure) => {
            // Nothing changes state; the plan goes back, same deadline.
            let kept = app
                .plans
                .put_publish_plan(plan.owner_did(), &plan.stored(), confirmed.expires_at)
                .is_ok();
            let retry = kept.then_some(PublishRetry {
                plan_id: plan.rkey(),
                csrf_token: &confirmed.csrf_token,
            });
            let next_step = PublishNextStep::after_failure(failure.needs_sign_in(), retry);
            Reply::page(
                StatusCode::BAD_GATEWAY,
                views::publish_failed_page(failure.message(), next_step),
            )
        }
    }
}

/// The landed suggestion moves pending → published; the approval is
/// counted once, noting whether the owner edited it first.
fn mark_published(app: &App, plan: &PublishPlan) {
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
}
