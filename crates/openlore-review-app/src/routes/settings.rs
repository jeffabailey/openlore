//! Settings and "Disconnect and forget me" (US-BRA-012, ADR-073/074).
//!
//! Forget me revokes her grant at her PDS (best effort: a 200 or 204 is
//! success, an unreachable PDS is not a reason to keep anything), the OAuth
//! adapter deletes her tokens whatever the answer, then every row the app
//! holds about her is purged in one transaction. Nothing is ever written to
//! her PDS: the claims she published stay hers.

use hyper::StatusCode;
use ports::RevokeOutcome;
use review_domain::views;

use crate::http::{App, PageRequest, Reply};
use crate::routes::signin::{
    csrf_token_for, current_session, signed_in_post, signed_out, to_landing,
};
use crate::wiring::{emit, Counted, LogEvent};

/// `GET /settings`.
pub(crate) fn settings(app: &App, request: &PageRequest) -> Reply {
    match current_session(app, request) {
        Some((_, session)) => page(views::settings_page(&session.handle)),
        None => to_landing(),
    }
}

/// `GET /settings/forget`: what is deleted and what is kept. Deletes nothing.
pub(crate) fn forget_me_preview(app: &App, request: &PageRequest) -> Reply {
    match current_session(app, request) {
        Some((cookie_value, _)) => page(views::forget_me_page(&csrf_token_for(&cookie_value))),
        None => to_landing(),
    }
}

/// `POST /settings/forget`: "Yes, forget me". She ends signed out.
pub(crate) async fn confirm_forget_me(app: &App, request: &PageRequest) -> Reply {
    let (_, session) = match signed_in_post(app, request) {
        Ok(signed_in) => signed_in,
        Err(refused) => return refused,
    };
    match forget(app, &session.owner_did).await {
        Ok(_) => signed_out(),
        Err(ForgetFailure::ScopeBusy) => Reply::page(
            StatusCode::SERVICE_UNAVAILABLE,
            views::forget_me_not_purged_page(),
        ),
        Err(ForgetFailure::Store) => Reply::page(
            StatusCode::SERVICE_UNAVAILABLE,
            views::forget_me_failed_page(),
        ),
    }
}

/// Why forget me did not purge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ForgetFailure {
    /// Her grant is revoked, but another change of hers held the scope past
    /// its wait: nothing was purged yet, and her session still works, so
    /// pressing forget me again finishes it (re-review N8).
    ScopeBusy,
    /// The purge transaction failed: nothing was purged.
    Store,
}

/// Forget `owner_did`: revoke (reported, never blocking), then purge while
/// her scope is held, so no Save, "Add repo", account change or scan move
/// in flight can write after the purge (R4). A scan still reading writes
/// no suggestion afterwards: its inserts require a listed source. Shared
/// by her own confirm and the operator's purge on request.
pub(crate) async fn forget(app: &App, owner_did: &str) -> Result<RevokeOutcome, ForgetFailure> {
    let revoked = app.oauth.revoke_grant(owner_did).await;
    let counted = Counted::of(&LogEvent::Disconnect(revoked));
    let Ok(scope_guard) = app.scope_locks.hold(owner_did).await else {
        return Err(ForgetFailure::ScopeBusy);
    };
    let purged = app.forget.purge_owner(owner_did, counted.tally());
    drop(scope_guard);
    purged.map_err(|_| ForgetFailure::Store)?;
    emit(LogEvent::Disconnect(revoked));
    Ok(revoked)
}

fn page(html: String) -> Reply {
    Reply::page(StatusCode::OK, html)
}
