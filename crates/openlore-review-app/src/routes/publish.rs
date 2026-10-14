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
use review_domain::selection::{tally_batch, PlanResult, MAX_SELECTED};
use review_domain::views::{self, BatchOutcome, PublishNextStep, PublishRetry};

use crate::executor::{execute_publish, PublishFailure};
use crate::http::{App, PageRequest, Reply};
use crate::routes::confirm::{take_confirmed_plan, take_owned_plan, ConfirmedPlan};
use crate::routes::review::{not_found, owner_can_publish};
use crate::routes::signin::{csrf_token_of, fields, signed_in_post};
use crate::wiring::{emit, Counted, LogEvent};

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
    match write_plan(app, plan, confirmed).await {
        Ok(at_uri) => Reply::page(StatusCode::OK, views::published_page(&at_uri)),
        Err(NotWritten { failure, kept }) => {
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

/// A write that did not land, and whether its plan was put back for Retry.
struct NotWritten {
    failure: PublishFailure,
    kept: bool,
}

/// Write one confirmed plan; on success the suggestion moves to published,
/// on failure nothing changes state and the plan goes back, same deadline.
async fn write_plan(
    app: &App,
    plan: &PublishPlan,
    confirmed: &ConfirmedPlan,
) -> Result<String, NotWritten> {
    match execute_publish(app.repo_write.as_ref(), app.repo_read.as_ref(), plan).await {
        Ok(at_uri) => {
            mark_published(app, plan);
            Ok(at_uri)
        }
        Err(failure) => {
            let kept = app
                .plans
                .put_publish_plan(plan.owner_did(), &plan.stored(), confirmed.expires_at)
                .is_ok();
            Err(NotWritten { failure, kept })
        }
    }
}

/// Confirm "Publish N to my repo" (or its Retry): each named plan is taken
/// from the owner's store exactly once and published in turn, as a single
/// confirm would. A replayed confirm finds its plans already taken and
/// writes nothing; a confirm naming none of the owner's plans is not found.
pub(crate) async fn confirm_publish_selected(app: &App, request: &PageRequest) -> Reply {
    let (_, session) = match signed_in_post(app, request) {
        Ok(signed_in) => signed_in,
        Err(refused) => return refused,
    };
    let csrf_token = csrf_token_of(request);
    let mut plan_ids = fields(&request.form, "plan");
    plan_ids.dedup();
    let taken: Vec<ConfirmedPlan> = plan_ids
        .into_iter()
        .take(MAX_SELECTED)
        .filter_map(|plan_id| {
            take_owned_plan(
                app,
                &session.owner_did,
                plan_id,
                PlanKind::Publish,
                csrf_token.clone(),
            )
        })
        .collect();
    if taken.is_empty() {
        return not_found();
    }
    let mut results = Vec::with_capacity(taken.len());
    for confirmed in &taken {
        results.push(confirm_one(app, confirmed).await);
    }
    let tally = tally_batch(results);
    let status = if tally.failures.is_empty() {
        StatusCode::OK
    } else {
        StatusCode::BAD_GATEWAY
    };
    Reply::page(
        status,
        views::batch_published_page(&BatchOutcome {
            published: &tally.published,
            failures: &tally.failures,
            retry_plan_ids: &tally.retry_plan_ids,
            expired: tally.expired,
            needs_sign_in: tally.needs_sign_in,
            csrf_token: &csrf_token,
        }),
    )
}

/// Confirm one taken plan of a batch, exactly as a single confirm would.
async fn confirm_one(app: &App, confirmed: &ConfirmedPlan) -> PlanResult {
    if confirmed.expired() {
        return PlanResult::Expired;
    }
    let restored = restore_plan(&confirmed.owner_did, &confirmed.stored)
        .ok()
        .filter(|plan| owner_can_publish(app, &confirmed.owner_did, plan.key()));
    let Some(plan) = restored else {
        return PlanResult::Skipped;
    };
    match write_plan(app, &plan, confirmed).await {
        Ok(at_uri) => PlanResult::Published(at_uri),
        Err(NotWritten { failure, kept }) => PlanResult::Failed {
            reason: failure.message().to_string(),
            needs_sign_in: failure.needs_sign_in(),
            kept_plan: kept.then(|| plan.rkey().to_string()),
        },
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
    let counted = Counted::of(&LogEvent::SuggestionApproved { edited });
    let moved = app.review_write.change_state(
        plan.owner_did(),
        plan.key(),
        SuggestionState::Pending,
        SuggestionState::Published,
        counted.tally(),
    );
    if matches!(moved, Ok(true)) {
        emit(LogEvent::SuggestionApproved { edited });
    }
}
