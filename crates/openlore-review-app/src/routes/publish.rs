//! `POST /review/publish`: confirm a previewed plan (US-BRA-004). The plan
//! is taken from the owner's store exactly once, refused if its preview has
//! expired (ADR-074), re-admitted as the owner's own self-attested record,
//! executed, and the suggestion moves to published. A missing or forged
//! anti-forgery token writes nothing. A failed write or a refused permit
//! puts the plan back (same deadline), so Retry — or confirming again once
//! the cause is gone — publishes it exactly once (ADR-071: the record key
//! is the content's CID, so a create that already landed is success).

use hyper::StatusCode;
use ports::claim_domain::authority::SigningPermit;
use ports::{PlanKind, SuggestionState};
use review_domain::kpi::approval_was_edited;
use review_domain::plans::{restore_plan, PublishPlan};
use review_domain::selection::{
    selection_value, tally_batch, BatchFailure, PlanResult, MAX_SELECTED,
};
use review_domain::views::{
    self, BatchOutcome, PublishNextStep, PublishRetry, CONFIRMED_SUGGESTION_FIELD,
};

use crate::executor::{execute_publish, PublishFailure};
use crate::http::{App, PageRequest, Reply};
use crate::routes::confirm::{take_confirmed_plan, take_owned_plan, ConfirmedPlan};
use crate::routes::review::{not_found, owner_has_pending, PublishChecks};
use crate::routes::signin::{csrf_token_of, fields, signed_in_post};
use crate::wiring::{emit, Counted, LogEvent};

/// Confirm "Publish to my repo" (or "Retry").
pub(crate) async fn confirm_publish(app: &App, request: &PageRequest) -> Reply {
    let confirmed = match take_confirmed_plan(app, request, PlanKind::Publish) {
        Ok(confirmed) => confirmed,
        Err(refused) => return refused,
    };
    let Some(confirmed) = bound_to_named(
        app,
        confirmed,
        &fields(&request.form, CONFIRMED_SUGGESTION_FIELD),
    ) else {
        return not_found();
    };
    if confirmed.expired() {
        return Reply::page(StatusCode::GONE, views::plan_expired_page());
    }
    let owner_did = confirmed.owner_did.as_str();
    let key = &confirmed.stored.key;
    if !owner_has_pending(app, owner_did, key) {
        return not_found();
    }
    let mut checks = PublishChecks::new(app, owner_did);
    let permit = match checks.permit(&key.subject).await {
        Ok(permit) => permit,
        Err(refusal) => {
            put_back(app, &confirmed);
            return checks.refused(&key.subject, refusal);
        }
    };
    match restore_plan(&permit, &confirmed.stored).ok() {
        Some(plan) => publish(app, &checks, &permit, &plan, &confirmed).await,
        None => not_found(),
    }
}

async fn publish(
    app: &App,
    checks: &PublishChecks<'_>,
    permit: &SigningPermit,
    plan: &PublishPlan,
    confirmed: &ConfirmedPlan,
) -> Reply {
    match write_plan(app, plan, confirmed).await {
        Ok(at_uri) => {
            checks.proven(permit);
            Reply::page(StatusCode::OK, views::published_page(&at_uri))
        }
        Err(NotWritten { failure, kept }) => {
            let retry = kept.then_some(PublishRetry {
                plan_id: plan.rkey(),
                csrf_token: &confirmed.csrf_token,
            });
            let next_step = PublishNextStep::after_failure(failure.needs_sign_in(), retry);
            Reply::page(
                StatusCode::BAD_GATEWAY,
                views::publish_of_suggestion_failed_page(
                    failure.message(),
                    failure.failed_write(),
                    next_step,
                    Some(plan.key()),
                ),
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
            let kept = put_back(app, confirmed);
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
    let named = fields(&request.form, CONFIRMED_SUGGESTION_FIELD);
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
        .filter_map(|confirmed| bound_to_named(app, confirmed, &named))
        .collect();
    if taken.is_empty() {
        return not_found();
    }
    let mut checks = PublishChecks::new(app, &session.owner_did);
    let mut results = Vec::with_capacity(taken.len());
    for confirmed in &taken {
        results.push(confirm_one(app, &mut checks, confirmed).await);
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

/// A taken plan, if the confirm that took it names its suggestion (or names
/// none: its plan ids alone bind it). A plan re-aimed at another suggestion
/// is put back untouched, and nothing is published.
fn bound_to_named(app: &App, confirmed: ConfirmedPlan, named: &[String]) -> Option<ConfirmedPlan> {
    if confirms_suggestion(named, &confirmed.stored.key) {
        return Some(confirmed);
    }
    put_back(app, &confirmed);
    None
}

/// Put a taken plan back under its id, untouched and with the preview's
/// deadline; was it kept?
fn put_back(app: &App, confirmed: &ConfirmedPlan) -> bool {
    app.plans
        .put_publish_plan(
            &confirmed.owner_did,
            &confirmed.stored,
            confirmed.expires_at,
        )
        .is_ok()
}

/// Does a confirm naming `named` suggestions confirm the plan for `key`?
fn confirms_suggestion(named: &[String], key: &ports::SuggestionKey) -> bool {
    named.is_empty() || named.contains(&selection_value(key))
}

/// Confirm one taken plan of a batch, exactly as a single confirm would.
async fn confirm_one(
    app: &App,
    checks: &mut PublishChecks<'_>,
    confirmed: &ConfirmedPlan,
) -> PlanResult {
    if confirmed.expired() {
        return PlanResult::Expired;
    }
    let key = &confirmed.stored.key;
    if !owner_has_pending(app, &confirmed.owner_did, key) {
        return PlanResult::Skipped;
    }
    let permit = match checks.permit(&key.subject).await {
        Ok(permit) => permit,
        Err(refusal) => {
            put_back(app, confirmed);
            checks.settle(&refusal);
            return PlanResult::Skipped;
        }
    };
    let Some(plan) = restore_plan(&permit, &confirmed.stored).ok() else {
        return PlanResult::Skipped;
    };
    match write_plan(app, &plan, confirmed).await {
        Ok(at_uri) => {
            checks.proven(&permit);
            PlanResult::Published(at_uri)
        }
        Err(NotWritten { failure, kept }) => PlanResult::Failed {
            failure: BatchFailure {
                reason: failure.message().to_string(),
                written: failure.failed_write(),
            },
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
    let outside_default = app
        .review_read
        .outside_default(plan.owner_did(), plan.key())
        .ok()
        .flatten();
    let counted = Counted::of(&LogEvent::SuggestionApproved {
        edited,
        outside_default,
    });
    let moved = app.review_write.change_state(
        plan.owner_did(),
        plan.key(),
        SuggestionState::Pending,
        SuggestionState::Published,
        counted.tally(),
    );
    if matches!(moved, Ok(true)) {
        emit(LogEvent::SuggestionApproved {
            edited,
            outside_default,
        });
    }
}
