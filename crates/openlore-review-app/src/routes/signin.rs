//! Sign-in, the return from the user's PDS, the signed-in queue and sign
//! out (US-BRA-001). HTTP concerns only (cookies, CSRF, `Origin`); every
//! business decision is a `review_domain::signin` function. A sign-in that
//! does not complete changes nothing and holds no session.

use std::sync::Arc;

use hyper::{Method, StatusCode};
use ports::{CompleteAuthorizationError, IdentityLookupError, NewWebSession, PdsCallback};
use rand::rngs::OsRng;
use rand::RngCore;
use review_domain::signin::{
    parse_handle, read_pds_return, sign_in_pin, PdsReturn, SignInFailure, SignInPin,
};
use review_domain::views;
use sha2::{Digest, Sha256};

use crate::http::{App, PageRequest, Reply};
use crate::routes::{github, review, scan};
use crate::wiring::{emit, LogEvent};

/// The browser-session cookie (data-models §3.2): host-only by its prefix.
pub(crate) const SESSION_COOKIE: &str = "__Host-ol_session";

/// A session lives 30 days.
const SESSION_MAX_AGE_SECS: u32 = 30 * 24 * 60 * 60;

/// The pages that talk to the driven ports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PageRoute {
    SignIn,
    PdsReturn,
    Review,
    SignOut,
    GithubStep,
    VerifyGithub,
    StartScan,
    ScanStatus,
    ApproveSuggestion,
    DeclineSuggestion,
}

/// Pure routing of the pages.
pub(crate) fn page_route(method: &Method, path: &str) -> Option<PageRoute> {
    match (method.as_str(), path) {
        ("POST", "/signin") => Some(PageRoute::SignIn),
        ("GET", "/oauth/callback") => Some(PageRoute::PdsReturn),
        ("GET", "/review") => Some(PageRoute::Review),
        ("POST", "/signout") => Some(PageRoute::SignOut),
        ("GET", "/github") => Some(PageRoute::GithubStep),
        ("POST", "/github") => Some(PageRoute::VerifyGithub),
        ("POST", "/scan") => Some(PageRoute::StartScan),
        ("GET", "/scan/status") => Some(PageRoute::ScanStatus),
        ("POST", "/review/approve") => Some(PageRoute::ApproveSuggestion),
        ("POST", "/review/decline") => Some(PageRoute::DeclineSuggestion),
        _ => None,
    }
}

pub(crate) async fn handle(app: &Arc<App>, route: PageRoute, request: &PageRequest) -> Reply {
    let same_origin = request.origin.as_deref() == Some(app.origin.as_str());
    match route {
        PageRoute::SignIn if same_origin => begin_sign_in(app, request).await,
        PageRoute::SignOut if same_origin => sign_out(app, request),
        PageRoute::VerifyGithub if same_origin => github::verify(app, request).await,
        PageRoute::StartScan if same_origin => scan::start_scan(app, request).await,
        PageRoute::ApproveSuggestion if same_origin => review::approve(app, request),
        PageRoute::DeclineSuggestion if same_origin => review::decline_suggestion(app, request),
        PageRoute::SignIn
        | PageRoute::SignOut
        | PageRoute::VerifyGithub
        | PageRoute::StartScan
        | PageRoute::ApproveSuggestion
        | PageRoute::DeclineSuggestion => forbidden(),
        PageRoute::PdsReturn => pds_return(app, request).await,
        PageRoute::Review => review::review(app, request),
        PageRoute::GithubStep => github::github_step(app, request),
        PageRoute::ScanStatus => scan::scan_status(app, request),
    }
}

async fn begin_sign_in(app: &App, request: &PageRequest) -> Reply {
    let typed = field(&request.form, "handle").unwrap_or_default();
    let Some(handle) = parse_handle(&typed) else {
        return failed(app, SignInFailure::HandleNotFound);
    };
    let identity = match app.identity.resolve_identity(handle.as_str()).await {
        Ok(identity) => identity,
        Err(IdentityLookupError::NotFound) => return failed(app, SignInFailure::HandleNotFound),
        Err(IdentityLookupError::Unavailable { .. }) => {
            return failed(app, SignInFailure::TemporarilyUnavailable)
        }
    };
    match app.oauth.begin_authorization(&identity).await {
        Ok(consent_screen) => {
            emit(LogEvent::SignInStarted);
            Reply::Redirect {
                location: consent_screen,
                set_cookie: None,
            }
        }
        Err(_) => failed(app, SignInFailure::TemporarilyUnavailable),
    }
}

async fn pds_return(app: &App, request: &PageRequest) -> Reply {
    match read_pds_return(&request.query) {
        PdsReturn::Declined { state } => {
            if let Some(state) = state {
                app.oauth.abandon_authorization(&state);
            }
            failed(app, SignInFailure::Cancelled)
        }
        PdsReturn::Malformed => failed(app, SignInFailure::ReturnNotRecognised),
        PdsReturn::Approved {
            code,
            state,
            issuer,
        } => {
            let callback = PdsCallback {
                code,
                state,
                issuer,
            };
            match app.oauth.complete_authorization(callback).await {
                Ok(signed_in) => {
                    match sign_in_pin(&signed_in.token_subject, &signed_in.expected.did) {
                        SignInPin::Accepted => start_session(app, &signed_in.expected),
                        SignInPin::Refused => {
                            app.oauth.forget_session(&signed_in.token_subject);
                            failed(app, SignInFailure::AccountMismatch)
                        }
                    }
                }
                Err(CompleteAuthorizationError::NotRecognised) => {
                    failed(app, SignInFailure::ReturnNotRecognised)
                }
                Err(CompleteAuthorizationError::ExchangeFailed) => {
                    failed(app, SignInFailure::ExchangeFailed)
                }
                Err(CompleteAuthorizationError::ExchangePanicContained) => {
                    emit(LogEvent::CallbackPanicIsolated);
                    failed(app, SignInFailure::ExchangeFailed)
                }
            }
        }
    }
}

fn start_session(app: &App, identity: &ports::ResolvedIdentity) -> Reply {
    let cookie_value = random_token();
    let session = NewWebSession {
        session_hash: sha256_hex(&cookie_value),
        csrf_hash: sha256_hex(&csrf_token_for(&cookie_value)),
        owner_did: identity.did.clone(),
        handle: identity.verified_handle.clone(),
        pds_endpoint: identity.pds_endpoint.clone(),
    };
    match app.sessions.start_session(&session) {
        Ok(()) => {
            emit(LogEvent::SignInCompleted);
            Reply::Redirect {
                location: "/review".to_string(),
                set_cookie: Some(session_cookie(&cookie_value)),
            }
        }
        Err(_) => {
            app.oauth.forget_session(&identity.did);
            failed(app, SignInFailure::TemporarilyUnavailable)
        }
    }
}

fn sign_out(app: &App, request: &PageRequest) -> Reply {
    let Some((cookie_value, session)) = current_session(app, request) else {
        return to_landing();
    };
    if !csrf_matches(request, &session) {
        return forbidden();
    }
    match app
        .sessions
        .end_session(&sha256_hex(&cookie_value), &session.owner_did)
    {
        Ok(()) => Reply::Redirect {
            location: "/".to_string(),
            set_cookie: Some(cleared_session_cookie()),
        },
        Err(_) => failed(app, SignInFailure::TemporarilyUnavailable),
    }
}

/// The submitted CSRF token is the session's.
pub(crate) fn csrf_matches(request: &PageRequest, session: &ports::WebSession) -> bool {
    let submitted = field(&request.form, "csrf").unwrap_or_default();
    sha256_hex(&submitted) == session.csrf_hash
}

/// The live session named by the request's cookie, with the cookie value.
pub(crate) fn current_session(
    app: &App,
    request: &PageRequest,
) -> Option<(String, ports::WebSession)> {
    let cookie_value = field(&request.cookies, SESSION_COOKIE)?;
    let session = app
        .sessions
        .resolve_session(&sha256_hex(&cookie_value))
        .ok()??;
    Some((cookie_value, session))
}

/// The landing page explaining why the sign-in did not complete.
fn failed(app: &App, failure: SignInFailure) -> Reply {
    emit(LogEvent::SignInRefused(failure));
    Reply::Page {
        status: failure_status(failure),
        html: views::landing_page(app.permission_mode, Some(failure)),
        set_cookie: None,
    }
}

fn failure_status(failure: SignInFailure) -> StatusCode {
    match failure {
        SignInFailure::Cancelled => StatusCode::OK,
        SignInFailure::TemporarilyUnavailable => StatusCode::SERVICE_UNAVAILABLE,
        SignInFailure::AccountMismatch => StatusCode::FORBIDDEN,
        SignInFailure::HandleNotFound
        | SignInFailure::ExchangeFailed
        | SignInFailure::ReturnNotRecognised => StatusCode::BAD_REQUEST,
    }
}

pub(crate) fn forbidden() -> Reply {
    Reply::Page {
        status: StatusCode::FORBIDDEN,
        html: "Forbidden".to_string(),
        set_cookie: None,
    }
}

pub(crate) fn to_landing() -> Reply {
    Reply::Redirect {
        location: "/".to_string(),
        set_cookie: None,
    }
}

pub(crate) fn field(pairs: &[(String, String)], name: &str) -> Option<String> {
    pairs
        .iter()
        .find(|(key, _)| key == name)
        .map(|(_, value)| value.clone())
}

/// 256 random bits, hex.
pub(crate) fn random_token() -> String {
    let mut bytes = [0u8; 32];
    OsRng.fill_bytes(&mut bytes);
    hex(&bytes)
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn sha256_hex(text: &str) -> String {
    hex(&Sha256::digest(text.as_bytes()))
}

/// The CSRF token of a session, derived from its cookie value so only its
/// hash needs storing (the page re-derives it on render).
pub(crate) fn csrf_token_for(cookie_value: &str) -> String {
    sha256_hex(&format!("csrf:{cookie_value}"))
}

fn session_cookie(value: &str) -> String {
    format!(
        "{SESSION_COOKIE}={value}; Path=/; Secure; HttpOnly; SameSite=Lax; Max-Age={SESSION_MAX_AGE_SECS}"
    )
}

fn cleared_session_cookie() -> String {
    format!("{SESSION_COOKIE}=; Path=/; Secure; HttpOnly; SameSite=Lax; Max-Age=0")
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        /// Universe: cookie values. Every session cookie the app sets is
        /// `__Host-` prefixed, host-only (no Domain), Secure, HttpOnly,
        /// SameSite=Lax and scoped to `/` (NFR-BRA-2).
        #[test]
        fn every_session_cookie_is_out_of_reach_of_scripts_and_other_hosts(value in "[0-9a-f]{64}") {
            for cookie in [session_cookie(&value), cleared_session_cookie()] {
                let lower = cookie.to_ascii_lowercase();
                prop_assert!(cookie.starts_with("__Host-"));
                prop_assert!(!lower.contains("domain="));
                for flag in ["; secure", "; httponly", "; samesite=lax", "; path=/"] {
                    prop_assert!(lower.contains(flag), "{} in {}", flag, cookie);
                }
            }
        }

        /// Universe: pairs of cookie values. The stored hashes never equal the
        /// cookie or its CSRF token, and distinct cookies never share a CSRF token.
        #[test]
        fn stored_hashes_never_reveal_the_cookie_or_its_csrf_token(a in "[0-9a-f]{64}", b in "[0-9a-f]{64}") {
            prop_assert_ne!(sha256_hex(&a), a.clone());
            prop_assert_ne!(sha256_hex(&csrf_token_for(&a)), csrf_token_for(&a));
            prop_assert_eq!(csrf_token_for(&a) == csrf_token_for(&b), a == b);
        }
    }
}
