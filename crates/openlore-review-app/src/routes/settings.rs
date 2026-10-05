//! Settings and "Disconnect and forget me" (US-BRA-012, ADR-073/074).
//!
//! Forget me revokes her grant at her PDS (best effort: a 200 or 204 is
//! success, an unreachable PDS is not a reason to keep anything), the OAuth
//! adapter deletes her tokens whatever the answer, then every row the app
//! holds about her is purged in one transaction. Nothing is ever written to
//! her PDS: the claims she published stay hers.

use hyper::StatusCode;
use ports::{ReviewStoreError, RevokeOutcome};
use review_domain::views;

use crate::http::{App, PageRequest, Reply};
use crate::routes::signin::{
    csrf_token_for, current_session, signed_in_post, signed_out, to_landing,
};
use crate::wiring::{observe, LogEvent};

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
        Err(_) => Reply::page(
            StatusCode::SERVICE_UNAVAILABLE,
            views::forget_me_failed_page(),
        ),
    }
}

/// Forget `owner_did`: revoke (reported, never blocking), then purge.
/// Shared by her own confirm and the operator's purge on request.
pub(crate) async fn forget(app: &App, owner_did: &str) -> Result<RevokeOutcome, ReviewStoreError> {
    let revoked = app.oauth.revoke_grant(owner_did).await;
    app.forget.purge_owner(owner_did)?;
    observe(app, LogEvent::Disconnect(revoked));
    Ok(revoked)
}

fn page(html: String) -> Reply {
    Reply::page(StatusCode::OK, html)
}
