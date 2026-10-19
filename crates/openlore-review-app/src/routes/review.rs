//! The private queue and its per-suggestion actions (US-BRA-003/004/006).
//! Every read and write is scoped to the signed-in DID (I-BRA-1): a
//! suggestion named by anyone else's form is simply not found.

use hyper::StatusCode;
use ports::{
    GithubLink, GithubLinkPort, KpiTally, ReviewStateRead, Suggestion, SuggestionKey,
    SuggestionState, WebSession,
};
use review_domain::edits::{edit_claim, philosophy_choices};
use review_domain::lifecycle::{
    owner_step, tally, visible_and_approvable, LifecycleEvent, OwnerStep, Tally,
};
use review_domain::plans::{
    plan_expires_at, publish_edited_plan, publish_plan, rfc3339_utc, ClaimDraft, PlanError,
    PublishPlan,
};
use review_domain::selection::{chosen_suggestions, SELECTION_FIELD};
use review_domain::views::{self, EditView, QueueNotice, QueueView};

use crate::http::{App, PageRequest, Reply};
use crate::limiter::unix_now_secs;
use crate::routes::github::github_step_of;
use crate::routes::signin::{
    csrf_token_for, csrf_token_of, current_session, field, fields, signed_in_post, to_landing,
};
use crate::wiring::{emit, Counted, LogEvent};

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
    let OwnerQueue {
        link,
        pending,
        tally,
    } = owner_queue(
        app.links.as_ref(),
        app.review_read.as_ref(),
        &session.owner_did,
    );
    let latest_scan = app.scans.latest_scan(&session.owner_did).ok().flatten();
    let skipped = app
        .scan_plans
        .latest_skipped(&session.owner_did)
        .unwrap_or_default();
    let csrf_token = csrf_token_for(cookie_value);
    Reply::page(
        status,
        views::review_page_naming_skips(
            &QueueView {
                handle: &session.handle,
                csrf_token: &csrf_token,
                github: github_step_of(link.as_ref()),
                latest_scan,
                pending: &pending,
                tally,
                notice,
            },
            &skipped,
        ),
    )
}

/// What the owner's queue page lists: the GitHub link, the pending
/// suggestions (only while that link is verified) and their tally.
pub(crate) struct OwnerQueue {
    pub(crate) link: Option<GithubLink>,
    pub(crate) pending: Vec<Suggestion>,
    pub(crate) tally: Tally,
}

/// The queue composition `queue_page` renders, read from the owner's links
/// and review state.
pub(crate) fn owner_queue(
    links: &dyn GithubLinkPort,
    review_read: &dyn ReviewStateRead,
    owner_did: &str,
) -> OwnerQueue {
    let link = links.github_link(owner_did).ok().flatten();
    let verified = link.as_ref().is_some_and(|link| link.verified);
    let pending = verified
        .then(|| review_read.pending_suggestions(owner_did).ok())
        .flatten()
        .unwrap_or_default();
    let tally = tally(
        verified
            .then(|| review_read.suggestion_states(owner_did).ok())
            .flatten()
            .unwrap_or_default()
            .into_iter()
            .map(|(_, state)| state),
    );
    OwnerQueue {
        link,
        pending,
        tally,
    }
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

/// `POST /review/approve-selected`: the batch preview of every ticked card
/// that names one of the owner's visible pending suggestions (at most
/// `MAX_SELECTED`). One plan is built and kept per suggestion, exactly as a
/// single Approve keeps one; nothing is written to any repo. A selection
/// naming nothing of the owner's goes back to the queue.
pub(crate) fn approve_selected(app: &App, request: &PageRequest) -> Reply {
    let (_, session) = match signed_in_post(app, request) {
        Ok(signed_in) => signed_in,
        Err(refused) => return refused,
    };
    let visible = visible_and_approvable(
        SuggestionState::Pending,
        link_verified(app, &session.owner_did),
    );
    let pending = visible
        .then(|| app.review_read.pending_suggestions(&session.owner_did).ok())
        .flatten()
        .unwrap_or_default();
    let values = fields(&request.form, SELECTION_FIELD);
    let composed_at = composed_now();
    let kept: Vec<PublishPlan> = chosen_suggestions(pending, &values)
        .into_iter()
        .filter_map(|suggestion| {
            publish_plan(&session.owner_did, &draft_of(suggestion), &composed_at).ok()
        })
        .filter(|plan| keep_plan(app, &session, plan))
        .collect();
    if kept.is_empty() {
        return Reply::redirect("/review");
    }
    Reply::page(
        StatusCode::OK,
        views::batch_preview_page(&kept, &csrf_token_of(request)),
    )
}

/// Keep `plan` until its preview deadline; false if it could not be kept.
fn keep_plan(app: &App, session: &WebSession, plan: &PublishPlan) -> bool {
    app.plans
        .put_publish_plan(
            &session.owner_did,
            &plan.stored(),
            plan_expires_at(unix_now_secs()),
        )
        .is_ok()
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
    let kept = plan.ok().filter(|plan| keep_plan(app, session, plan));
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
        |cookie_value, session, key, _| {
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
/// A decline that moves is logged and counted in the move's transaction.
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
    let logged = (event == LifecycleEvent::Decline).then_some(LogEvent::SuggestionDeclined);
    let counted = logged.as_ref().map(Counted::of);
    let tally = counted.as_ref().map_or(KpiTally::NONE, Counted::tally);
    let moved = match current.and_then(|state| owner_step(state, event)) {
        Some(OwnerStep::Move { from, to }) => app
            .review_write
            .change_state(&session.owner_did, &key, from, to, tally)
            .ok(),
        Some(OwnerStep::AlreadyDone) => Some(false),
        None => None,
    };
    if let (Some(true), Some(event)) = (moved, logged) {
        emit(event);
    }
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

#[cfg(test)]
mod tests {
    //! C-6 (U-37): a stored link plan never appears in the review queue. The
    //! queue is composed by `owner_queue`, the function `queue_page` renders.
    use super::{owner_queue, OwnerQueue};
    use crate::routes::github::github_step_of;
    use adapter_review_store::{DataKey, ReviewStore};
    use ports::claim_domain::identity_link::{link_predicate, GithubObservation};
    use ports::{
        AccountAdmission, AccountAdmitted, GithubAccountsWrite, KpiTally, PublishPlanPort,
        ReviewStateWrite, Suggestion, SuggestionKey,
    };
    use review_domain::lifecycle::tally;
    use review_domain::link_plan::link_preview;
    use review_domain::views::{self, QueueView};

    const OWNER: &str = "did:plc:priyaraman7777777777777777";
    const LINKED_LOGIN: &str = "priyaraman";
    const LINKED_ID: u64 = 4_210_001;

    fn pending_suggestion() -> Suggestion {
        Suggestion {
            key: SuggestionKey {
                subject: "github:tideworks/tidepool".into(),
                predicate: "embodiesPhilosophy".into(),
                object: "org.openlore.philosophy.memory-safety".into(),
            },
            confidence_bp: 2500,
            evidence: vec![],
            why: vec![],
            source_repo: "tideworks/tidepool".into(),
        }
    }

    #[test]
    fn a_stored_link_plan_never_appears_in_the_review_queue() {
        let store = ReviewStore::open_in_memory(DataKey::generate()).expect("store");
        assert_eq!(
            store
                .add_or_reverify(OWNER, LINKED_LOGIN, LINKED_ID, 3, KpiTally::NONE)
                .expect("linked"),
            AccountAdmission::Admitted(AccountAdmitted::Added)
        );
        store
            .add_pending(OWNER, &[pending_suggestion()])
            .expect("queued");
        let observation = GithubObservation {
            login: LINKED_LOGIN.to_string(),
            user_id: LINKED_ID,
            bio: Some(format!("my DID: {OWNER}")),
        };
        let plan = link_preview(OWNER, &observation, "2026-10-17T09:30:00Z").expect("proven");

        store
            .put_publish_plan(OWNER, &plan.stored(), 4_102_444_800)
            .expect("kept");

        let OwnerQueue {
            link,
            pending,
            tally: queue_tally,
        } = owner_queue(&store, &store, OWNER);
        assert_eq!(pending, vec![pending_suggestion()]);
        assert_eq!(queue_tally, tally([ports::SuggestionState::Pending]));
        let html = views::review_page(&QueueView {
            handle: "priya.example",
            csrf_token: "csrf",
            github: github_step_of(link.as_ref()),
            latest_scan: None,
            pending: &pending,
            tally: queue_tally,
            notice: None,
        });
        assert!(html.contains("tideworks/tidepool"), "{html}");
        for link_part in [plan.claim().object.as_str(), link_predicate(), plan.rkey()] {
            assert!(!html.contains(link_part), "{link_part}: {html}");
        }
    }
}
