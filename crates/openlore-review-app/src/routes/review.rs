//! The private queue and its per-suggestion actions (US-BRA-003/004/006).
//! Every read and write is scoped to the signed-in DID (I-BRA-1): a
//! suggestion named by anyone else's form is simply not found.

use hyper::StatusCode;
use ports::{Suggestion, SuggestionKey, SuggestionState, WebSession};
use review_domain::lifecycle::decline;
use review_domain::views::{self, QueueView, ScanRefused};

use crate::http::{App, PageRequest, Reply};
use crate::routes::github::github_step_of;
use crate::routes::signin::{
    csrf_matches, csrf_token_for, current_session, field, forbidden, to_landing,
};

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
    refused: Option<ScanRefused>,
) -> Reply {
    let link = app.links.github_link(&session.owner_did).ok().flatten();
    let verified = link.as_ref().is_some_and(|link| link.verified);
    let pending = verified
        .then(|| app.review_read.pending_suggestions(&session.owner_did).ok())
        .flatten()
        .unwrap_or_default();
    let latest_scan = app.scans.latest_scan(&session.owner_did).ok().flatten();
    let csrf_token = csrf_token_for(cookie_value);
    Reply::Page {
        status,
        html: views::review_page(&QueueView {
            handle: &session.handle,
            csrf_token: &csrf_token,
            github: github_step_of(link.as_ref()),
            latest_scan,
            pending: &pending,
            refused,
        }),
        set_cookie: None,
    }
}

/// `POST /review/approve`: the exact-record preview of one of the owner's
/// visible pending suggestions. Writes nothing.
pub(crate) fn approve(app: &App, request: &PageRequest) -> Reply {
    with_own_suggestion(app, request, |_, suggestion| Reply::Page {
        status: StatusCode::OK,
        html: views::approval_preview_page(&suggestion),
        set_cookie: None,
    })
}

/// `POST /review/decline`: "Not me" — the suggestion leaves the queue,
/// declined, and is never offered again.
pub(crate) fn decline_suggestion(app: &App, request: &PageRequest) -> Reply {
    with_own_suggestion(app, request, |session, suggestion| {
        let declined = decline(SuggestionState::Pending).is_some_and(|to| {
            app.review_write
                .change_state(
                    &session.owner_did,
                    &suggestion.key,
                    SuggestionState::Pending,
                    to,
                )
                .unwrap_or(false)
        });
        if declined {
            Reply::Redirect {
                location: "/review".to_string(),
                set_cookie: None,
            }
        } else {
            not_found()
        }
    })
}

/// Run `act` on the visible pending suggestion the form names, if it is
/// the signed-in owner's; anything else is not found.
fn with_own_suggestion(
    app: &App,
    request: &PageRequest,
    act: impl FnOnce(&WebSession, Suggestion) -> Reply,
) -> Reply {
    let Some((_, session)) = current_session(app, request) else {
        return to_landing();
    };
    if !csrf_matches(request, &session) {
        return forbidden();
    }
    let verified = app
        .links
        .github_link(&session.owner_did)
        .ok()
        .flatten()
        .is_some_and(|link| link.verified);
    let named = key_of(request);
    let found = named.filter(|_| verified).and_then(|key| {
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

/// The suggestion key a card's form names.
fn key_of(request: &PageRequest) -> Option<SuggestionKey> {
    Some(SuggestionKey {
        subject: field(&request.form, "subject")?,
        predicate: field(&request.form, "predicate")?,
        object: field(&request.form, "object")?,
    })
}

fn not_found() -> Reply {
    Reply::Page {
        status: StatusCode::NOT_FOUND,
        html: views::not_found_page(),
        set_cookie: None,
    }
}
