//! The GitHub step (US-BRA-002, ADR-076): show the exact DID to put in the
//! bio, then check the bio. HTTP concerns and port calls only; the verdict,
//! the capability and every message are `review_domain` functions. The bio
//! itself is never logged.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use hyper::StatusCode;
use ports::{GithubError, GithubLink, LinkRecorded, PersonProfile, WebSession};
use review_domain::budget::{budget_allows, HOUR_SECS, VERIFY_ATTEMPTS_PER_HOUR};
use review_domain::ownership::{
    is_github_login, prove_ownership, GithubAccount, OwnershipRefusal, VerifiedOwnership,
};
use review_domain::views::{self, GithubStep, OwnershipOutcome};

use crate::http::{App, PageRequest, Reply};
use crate::routes::signin::{
    csrf_matches, csrf_token_for, current_session, field, forbidden, to_landing,
};
use crate::wiring::{emit, LogEvent};

/// Recent verify attempts per DID (in memory; resets on restart, ADR-076 §2).
#[derive(Default)]
pub(crate) struct VerifyAttempts(Mutex<HashMap<String, Vec<Instant>>>);

impl VerifyAttempts {
    /// Count an attempt by `owner_did` if the hourly budget allows it.
    fn try_spend(&self, owner_did: &str) -> bool {
        let Ok(mut attempts) = self.0.lock() else {
            return false;
        };
        let now = Instant::now();
        let window = Duration::from_secs(HOUR_SECS);
        let recent = attempts.entry(owner_did.to_string()).or_default();
        recent.retain(|at| now.duration_since(*at) < window);
        let ages: Vec<u64> = recent
            .iter()
            .map(|at| now.duration_since(*at).as_secs())
            .collect();
        let allowed = budget_allows(&ages, VERIFY_ATTEMPTS_PER_HOUR, HOUR_SECS);
        if allowed {
            recent.push(now);
        }
        allowed
    }
}

/// The queue's view of a stored link.
pub(crate) fn github_step_of(link: Option<&GithubLink>) -> GithubStep<'_> {
    match link {
        None => GithubStep::NotLinked,
        Some(link) if link.verified => GithubStep::Verified {
            login: &link.github_login,
        },
        Some(link) => GithubStep::NeedsReverify {
            login: &link.github_login,
        },
    }
}

/// `GET /github`: the exact DID, where to put it, and the Verify form.
pub(crate) fn github_step(app: &App, request: &PageRequest) -> Reply {
    match current_session(app, request) {
        Some((cookie_value, session)) => github_page(&session, &cookie_value, None),
        None => to_landing(),
    }
}

/// `POST /github`: check the named account's bio for the signed-in DID.
pub(crate) async fn verify(app: &App, request: &PageRequest) -> Reply {
    let Some((cookie_value, session)) = current_session(app, request) else {
        return to_landing();
    };
    if !csrf_matches(request, &session) {
        return forbidden();
    }
    let typed = field(&request.form, "github_login").unwrap_or_default();
    let typed = typed.trim();
    let (login, outcome) = check_bio(app, &session, typed).await;
    let message = match &outcome {
        Ok(()) => {
            emit(LogEvent::GithubVerified);
            views::ownership_message(
                OwnershipOutcome::Verified,
                &login,
                &session.owner_did,
                &session.handle,
            )
        }
        Err(refusal) => {
            emit(LogEvent::GithubVerifyRefused(refusal.label()));
            views::ownership_message(
                OwnershipOutcome::Refused(refusal),
                &login,
                &session.owner_did,
                &session.handle,
            )
        }
    };
    github_page(&session, &cookie_value, Some(&message))
}

/// Read the profile, prove ownership and record the link. Returns the login
/// to name in the message (GitHub's spelling once the profile is read).
async fn check_bio(
    app: &App,
    session: &WebSession,
    typed: &str,
) -> (String, Result<(), OwnershipRefusal>) {
    if !app.verify_attempts.try_spend(&session.owner_did) {
        return (typed.to_string(), Err(OwnershipRefusal::TooManyAttempts));
    }
    if !is_github_login(typed) {
        return (typed.to_string(), Err(OwnershipRefusal::AccountNotFound));
    }
    let profile = match app.github.read_person(typed).await {
        Ok(profile) => profile,
        Err(error) => return (typed.to_string(), Err(refusal_of(&error))),
    };
    let proven = prove_ownership(&session.owner_did, &account_of(&profile), None);
    let outcome = match proven {
        Ok(ownership) => record_link(app, &ownership),
        Err(refusal) => {
            unverify_if_linked(app, session, &profile, &refusal);
            Err(refusal)
        }
    };
    (profile.login, outcome)
}

fn record_link(app: &App, ownership: &VerifiedOwnership) -> Result<(), OwnershipRefusal> {
    match app.links.record_verified_link(
        ownership.owner_did(),
        ownership.github_login(),
        ownership.github_user_id(),
    ) {
        Ok(LinkRecorded::Linked) => Ok(()),
        Ok(LinkRecorded::HeldByAnotherOwner) => Err(OwnershipRefusal::LinkedToAnotherAccount),
        Err(_) => Err(OwnershipRefusal::GithubUnavailable),
    }
}

/// A failed check of the account this DID links marks the link unverified.
fn unverify_if_linked(
    app: &App,
    session: &WebSession,
    profile: &PersonProfile,
    refusal: &OwnershipRefusal,
) {
    let linked = app.links.github_link(&session.owner_did).ok().flatten();
    if refusal.disproves_ownership() && linked.is_some_and(|link| link.github_user_id == profile.id)
    {
        let _ = app
            .links
            .mark_link_unverified(&session.owner_did, refusal.label());
    }
}

/// The facts of a profile the ownership proof reads.
pub(crate) fn account_of(profile: &PersonProfile) -> GithubAccount<'_> {
    GithubAccount {
        login: &profile.login,
        user_id: profile.id,
        bio: profile.bio.as_deref(),
    }
}

/// A GitHub read that failed, as the person is told.
pub(crate) fn refusal_of(error: &GithubError) -> OwnershipRefusal {
    match error {
        GithubError::NotFound { .. } | GithubError::NotPublic { .. } => {
            OwnershipRefusal::AccountNotFound
        }
        GithubError::RateLimited { .. } => OwnershipRefusal::RateLimited,
        _ => OwnershipRefusal::GithubUnavailable,
    }
}

fn github_page(session: &WebSession, cookie_value: &str, message: Option<&str>) -> Reply {
    Reply::Page {
        status: StatusCode::OK,
        html: views::github_page(&session.owner_did, &csrf_token_for(cookie_value), message),
        set_cookie: None,
    }
}
