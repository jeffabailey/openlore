//! The private queue and its per-suggestion actions (US-BRA-003/004/006).
//! Every read and write is scoped to the signed-in DID (I-BRA-1): a
//! suggestion named by anyone else's form is simply not found.

use hyper::StatusCode;
use ports::{Suggestion, SuggestionKey, SuggestionState, WebSession};
use review_domain::edits::{edit_claim, philosophy_choices};
use review_domain::lifecycle::{
    owner_step, tally, visible_and_approvable, LifecycleEvent, OwnerStep,
};
use review_domain::plans::{
    plan_expires_at, publish_edited_plan, publish_plan, rfc3339_utc, ClaimDraft, PlanError,
    PublishPlan,
};
use review_domain::views::{self, EditView, QueueNotice, QueueView};

use crate::http::{App, PageRequest, Reply};
use crate::limiter::unix_now_secs;
use crate::routes::github::github_step_of;
use crate::routes::signin::{
    csrf_token_for, csrf_token_of, current_session, field, signed_in_post, to_landing,
};
use crate::wiring::{observe, LogEvent};

/// `GET /review`: the signed-in queue.
pub(crate) fn review(app: &App, request: &PageRequest) -> Reply {
    match current_session(app, request) {
        Some((cookie_value, session)) => {
            queue_page(app, &cookie_value, &session, StatusCode::OK, None)
        }
        None => to_landing(),
    }
}

/// The owner's queue page; pending suggestions show only while the GitHub
/// link is verified (D-12: hidden, never deleted).
pub(crate) fn queue_page(
    app: &App,
    cookie_value: &str,
    session: &WebSession,
    status: StatusCode,
    notice: Option<QueueNotice<'_>>,
) -> Reply {
    let link = app.links.github_link(&session.owner_did).ok().flatten();
    let verified = link.as_ref().is_some_and(|link| link.verified);
    let pending = verified
        .then(|| app.review_read.pending_suggestions(&session.owner_did).ok())
        .flatten()
        .unwrap_or_default();
    let tally = tally(
        verified
            .then(|| app.review_read.suggestion_states(&session.owner_did).ok())
            .flatten()
            .unwrap_or_default()
            .into_iter()
            .map(|(_, state)| state),
    );
    let latest_scan = app.scans.latest_scan(&session.owner_did).ok().flatten();
    let csrf_token = csrf_token_for(cookie_value);
    Reply::page(
        status,
        views::review_page(&QueueView {
            handle: &session.handle,
            csrf_token: &csrf_token,
            github: github_step_of(link.as_ref()),
            latest_scan,
            pending: &pending,
            tally,
            notice,
        }),
    )
}

/// `POST /review/approve`: the exact-record preview of one of the owner's
/// visible pending suggestions. Builds and keeps the publish plan (the
/// Plan-value: what is shown is what a confirm writes); writes nothing to
/// any repo.
pub(crate) fn approve(app: &App, request: &PageRequest) -> Reply {
    with_own_suggestion(app, request, "object", |session, suggestion| {
        let draft = draft_of(suggestion);
        let plan = publish_plan(&session.owner_did, &draft, &composed_now());
        keep_and_preview(app, request, session, plan)
    })
}

/// `POST /review/edit`: the edit form of one of the owner's visible pending
/// suggestions, prefilled with the suggestion (US-BRA-005). Writes nothing.
pub(crate) fn edit(app: &App, request: &PageRequest) -> Reply {
    with_own_suggestion(app, request, "object", |_, suggestion| {
        let typed = views::confidence_text(suggestion.confidence_bp);
        edit_form(
            request,
            &suggestion,
            &suggestion.key.object,
            &typed,
            None,
            StatusCode::OK,
        )
    })
}

/// `POST /review/edit/preview`: the exact-record preview of the owner's
/// edit. A refused confidence re-shows the form with guidance and no way to
/// publish; a valid edit becomes a new plan whose record is exactly the
/// edit (its CID is the record key). Writes nothing to any repo.
pub(crate) fn preview_edit(app: &App, request: &PageRequest) -> Reply {
    with_own_suggestion(app, request, "suggested_object", |session, suggestion| {
        let chosen = field(&request.form, "object");
        let typed = field(&request.form, "confidence").unwrap_or_default();
        match edit_claim(&suggestion.key.object, chosen.as_deref(), &typed) {
            Ok(edit) => {
                let draft = draft_of(suggestion);
                let plan = publish_edited_plan(&session.owner_did, &draft, &edit, &composed_now());
                keep_and_preview(app, request, session, plan)
            }
            Err(invalid) => {
                let chosen = chosen.unwrap_or_else(|| suggestion.key.object.clone());
                let guidance = invalid.to_string();
                edit_form(
                    request,
                    &suggestion,
                    &chosen,
                    &typed,
                    Some(&guidance),
                    StatusCode::UNPROCESSABLE_ENTITY,
                )
            }
        }
    })
}

fn edit_form(
    request: &PageRequest,
    suggestion: &Suggestion,
    chosen: &str,
    typed_confidence: &str,
    guidance: Option<&str>,
    status: StatusCode,
) -> Reply {
    let choices = philosophy_choices(&suggestion.key.object);
    Reply::page(
        status,
        views::edit_page(&EditView {
            suggestion,
            csrf_token: &csrf_token_of(request),
            choices: &choices,
            chosen,
            typed_confidence,
            guidance,
        }),
    )
}

fn draft_of(suggestion: Suggestion) -> ClaimDraft {
    ClaimDraft {
        key: suggestion.key,
        evidence: suggestion.evidence,
        confidence_bp: suggestion.confidence_bp,
    }
}

fn composed_now() -> String {
    rfc3339_utc(unix_now_secs())
}

/// Keep a built plan until its deadline and show its exact-record preview;
/// a plan that cannot be built or kept is not found.
fn keep_and_preview(
    app: &App,
    request: &PageRequest,
    session: &WebSession,
    plan: Result<PublishPlan, PlanError>,
) -> Reply {
    let kept = plan.ok().filter(|plan| {
        app.plans
            .put_publish_plan(
                &session.owner_did,
                &plan.stored(),
                plan_expires_at(unix_now_secs()),
            )
            .is_ok()
    });
    match kept {
        Some(plan) => Reply::page(
            StatusCode::OK,
            views::approval_preview_page(&plan, &csrf_token_of(request)),
        ),
        None => not_found(),
    }
}

/// May the owner publish the suggestion `key` right now? Only while it is
/// pending and their GitHub link is verified (CORE-9).
pub(crate) fn owner_can_publish(app: &App, owner_did: &str, key: &SuggestionKey) -> bool {
    let verified = link_verified(app, owner_did);
    app.review_read
        .suggestion_states(owner_did)
        .is_ok_and(|states| {
            states
                .iter()
                .any(|(k, state)| k == key && visible_and_approvable(*state, verified))
        })
}

/// `POST /review/decline`: "Not me" — the suggestion leaves the queue,
/// declined, and is never offered again. Private app state only: this path
/// holds no repo-write capability (I-BRA-2), and declining twice is the
/// same as declining once.
pub(crate) fn decline_suggestion(app: &App, request: &PageRequest) -> Reply {
    with_own_step(
        app,
        request,
        LifecycleEvent::Decline,
        |cookie_value, session, key, moved| {
            if moved {
                observe(app, LogEvent::SuggestionDeclined);
            }
            queue_page(
                app,
                cookie_value,
                session,
                StatusCode::OK,
                Some(QueueNotice::Declined(key)),
            )
        },
    )
}

/// `POST /review/undo`: a declined suggestion is pending again.
pub(crate) fn undo_decline(app: &App, request: &PageRequest) -> Reply {
    with_own_step(app, request, LifecycleEvent::Undo, |_, _, _, _| {
        Reply::redirect("/review")
    })
}

/// Take the owner's `event` on the suggestion the form names, if it is the
/// signed-in owner's and their GitHub link is verified; `done` gets whether
/// the suggestion moved (`false`: it already stood where `event` leaves it).
/// A refused event, or anyone else's suggestion, is not found.
fn with_own_step(
    app: &App,
    request: &PageRequest,
    event: LifecycleEvent,
    done: impl FnOnce(&str, &WebSession, &SuggestionKey, bool) -> Reply,
) -> Reply {
    let (cookie_value, session) = match signed_in_post(app, request) {
        Ok(signed_in) => signed_in,
        Err(refused) => return refused,
    };
    let Some(key) = key_of(request, "object").filter(|_| link_verified(app, &session.owner_did))
    else {
        return not_found();
    };
    let current = app
        .review_read
        .suggestion_states(&session.owner_did)
        .ok()
        .and_then(|states| states.into_iter().find(|(k, _)| *k == key))
        .map(|(_, state)| state);
    let moved = match current.and_then(|state| owner_step(state, event)) {
        Some(OwnerStep::Move { from, to }) => app
            .review_write
            .change_state(&session.owner_did, &key, from, to)
            .ok(),
        Some(OwnerStep::AlreadyDone) => Some(false),
        None => None,
    };
    match moved {
        Some(moved) => done(&cookie_value, &session, &key, moved),
        None => not_found(),
    }
}

/// Is the owner's GitHub link currently verified?
fn link_verified(app: &App, owner_did: &str) -> bool {
    app.links
        .github_link(owner_did)
        .ok()
        .flatten()
        .is_some_and(|link| link.verified)
}

/// Run `act` on the visible pending suggestion the form names (its object
/// in `object_field`), if it is the signed-in owner's; anything else is not
/// found.
fn with_own_suggestion(
    app: &App,
    request: &PageRequest,
    object_field: &str,
    act: impl FnOnce(&WebSession, Suggestion) -> Reply,
) -> Reply {
    let (_, session) = match signed_in_post(app, request) {
        Ok(signed_in) => signed_in,
        Err(refused) => return refused,
    };
    let visible = visible_and_approvable(
        SuggestionState::Pending,
        link_verified(app, &session.owner_did),
    );
    let named = key_of(request, object_field);
    let found = named.filter(|_| visible).and_then(|key| {
        app.review_read
            .pending_suggestions(&session.owner_did)
            .ok()?
            .into_iter()
            .find(|suggestion| suggestion.key == key)
    });
    match found {
        Some(suggestion) => act(&session, suggestion),
        None => not_found(),
    }
}

/// The suggestion key a form names, its object in `object_field`.
fn key_of(request: &PageRequest, object_field: &str) -> Option<SuggestionKey> {
    Some(SuggestionKey {
        subject: field(&request.form, "subject")?,
        predicate: field(&request.form, "predicate")?,
        object: field(&request.form, object_field)?,
    })
}

pub(crate) fn not_found() -> Reply {
    Reply::page(StatusCode::NOT_FOUND, views::not_found_page())
}
