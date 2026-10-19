//! The GitHub step (US-BRA-002, ADR-076; US-RRA-003): show the exact DID
//! to put in the bio, then check the bio of each account the owner adds, up
//! to three. The one holder of the accounts write port. HTTP concerns and port calls only; the verdict,
//! the capability and every message are `review_domain` functions. The bio
//! itself is never logged.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use hyper::StatusCode;
use ports::claim_domain::identity_link::LinkRecord;
use ports::claim_domain::RecordOrigin;
use ports::{
    AccountAdmission, AccountAdmitted, AccountRefusal, GithubAccountProfile, GithubError,
    GithubLink, WebSession,
};
use review_domain::budget::{budget_allows, HOUR_SECS, VERIFY_ATTEMPTS_PER_HOUR};
use review_domain::kpi::AccountAddRefused;
use review_domain::ownership::{
    is_github_login, prove_ownership, GithubAccount, OwnershipRefusal, VerifiedOwnership,
};
use review_domain::published::active_owner_link_records;
use review_domain::repo_scope::{admit_account, OtherOwner, Standing, MAX_GITHUB_ACCOUNTS};
use review_domain::views::{self, GithubStep, OwnershipOutcome};

use crate::http::{App, PageRequest, Reply};
use crate::routes::signin::{csrf_token_for, current_session, field, signed_in_post, to_landing};
use crate::wiring::{emit, observe, Counted, LogEvent};

/// Recent verify attempts per DID (in memory; resets on restart, ADR-076 §2).
#[derive(Default)]
pub(crate) struct VerifyAttempts(Mutex<HashMap<String, Vec<Instant>>>);

impl VerifyAttempts {
    /// Count an attempt by `owner_did` if the hourly budget allows it.
    pub(crate) fn try_spend(&self, owner_did: &str) -> bool {
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

/// `GET /github`: the exact DID, where to put it, the Verify form, for a
/// verified owner the offer to publish the link and, for each of her Active
/// published links, the offer to retract it (RA-4).
pub(crate) async fn github_step(app: &App, request: &PageRequest) -> Reply {
    let Some((cookie_value, session)) = current_session(app, request) else {
        return to_landing();
    };
    github_page(app, &session, &cookie_value, None).await
}

/// The owner's Active link records, read live from her own repo (U-22).
/// A repo that cannot be read offers nothing to retract.
async fn published_links(app: &App, owner_did: &str) -> Vec<LinkRecord> {
    let Ok(identity) = app.identity.resolve_did(owner_did).await else {
        return Vec::new();
    };
    let Ok(listing) = app
        .repo_listing
        .list_repo_claims(&identity.pds_endpoint, &identity.did)
        .await
    else {
        return Vec::new();
    };
    let origin = RecordOrigin::of(&listing.fetched_from, &identity.pds_endpoint);
    active_owner_link_records(&identity.did, &listing.records, origin)
}

/// Whether a Verify press adds an account to a list that already holds
/// one (US-RRA-003, counted by KPI-RRA-2) or names the owner's first.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Attempt {
    FirstAccount,
    AnotherAccount,
}

impl Attempt {
    fn of(listed: &[GithubLink]) -> Self {
        if listed.is_empty() {
            Self::FirstAccount
        } else {
            Self::AnotherAccount
        }
    }

    /// Adding another account names any GitHub trouble one way: it could
    /// not be reached (AC-003.2).
    fn read_refusal(self, error: &GithubError) -> OwnershipRefusal {
        match (self, refusal_of(error)) {
            (Self::AnotherAccount, OwnershipRefusal::RateLimited) => {
                OwnershipRefusal::GithubUnavailable
            }
            (_, refusal) => refusal,
        }
    }

    /// The counters an admitted account adds with its row.
    fn admitted_event(self, admitted: AccountAdmitted) -> LogEvent<'static> {
        match (self, admitted) {
            (Self::AnotherAccount, AccountAdmitted::Added) => LogEvent::AccountAdded,
            _ => LogEvent::GithubVerified,
        }
    }
}

/// Why a Verify press did not verify an account.
enum NotVerified {
    Check(OwnershipRefusal),
    Listing(AccountRefusal),
}

/// `POST /github`: check the named account's bio for the signed-in DID and
/// add it to her list (up to [`MAX_GITHUB_ACCOUNTS`]), or verify a listed
/// one again.
pub(crate) async fn verify(app: &App, request: &PageRequest) -> Reply {
    let (cookie_value, session) = match signed_in_post(app, request) {
        Ok(signed_in) => signed_in,
        Err(refused) => return refused,
    };
    let typed = field(&request.form, "github_login").unwrap_or_default();
    let typed = typed.trim();
    let listed = app
        .links
        .github_accounts(&session.owner_did)
        .unwrap_or_default();
    let attempt = Attempt::of(&listed);
    if attempt == Attempt::AnotherAccount {
        observe(app, LogEvent::AccountAddStarted);
    }
    let (login, outcome) = check_bio(app, &session, typed, &listed).await;
    let shown = match &outcome {
        Ok(admitted) => {
            emit(attempt.admitted_event(*admitted));
            OwnershipOutcome::Verified
        }
        Err(not_verified) => {
            count_refusal(app, attempt, not_verified);
            match not_verified {
                NotVerified::Check(refusal) => OwnershipOutcome::Refused(refusal),
                NotVerified::Listing(refusal) => OwnershipOutcome::NotAdded(*refusal),
            }
        }
    };
    let message = views::ownership_message(shown, &login, &session.owner_did, &session.handle);
    github_page(app, &session, &cookie_value, Some((shown, &message))).await
}

fn count_refusal(app: &App, attempt: Attempt, not_verified: &NotVerified) {
    let counted = match not_verified {
        NotVerified::Check(refusal) => {
            observe(app, LogEvent::GithubVerifyRefused(refusal.label()));
            AccountAddRefused::of_check(refusal)
        }
        NotVerified::Listing(refusal) => Some(AccountAddRefused::of_listing(*refusal)),
    };
    if let (Attempt::AnotherAccount, Some(refused)) = (attempt, counted) {
        observe(app, LogEvent::AccountAddRefused(refused));
    }
}

/// Read the profile, decide whether the account may join the list, prove
/// ownership and add it. Returns the login to name in the message
/// (GitHub's spelling once the profile is read).
async fn check_bio(
    app: &App,
    session: &WebSession,
    typed: &str,
    listed: &[GithubLink],
) -> (String, Result<AccountAdmitted, NotVerified>) {
    let refused = |refusal| (typed.to_string(), Err(NotVerified::Check(refusal)));
    if !app.verify_attempts.try_spend(&session.owner_did) {
        return refused(OwnershipRefusal::TooManyAttempts);
    }
    if !is_github_login(typed) {
        return refused(OwnershipRefusal::AccountNotFound);
    }
    let attempt = Attempt::of(listed);
    let profile = match app.github.read_account(typed).await {
        Ok(profile) => profile,
        Err(error) => return refused(attempt.read_refusal(&error)),
    };
    let proven = || {
        prove_ownership(&session.owner_did, &account_of(&profile), None).map_err(|refusal| {
            unverify_if_listed(app, session, listed, &profile, &refusal);
            NotVerified::Check(refusal)
        })
    };
    let outcome = match admission(app, session, listed, profile.id) {
        // A listed account's bio is still read: a failed check marks it
        // unverified, as a re-check anywhere does.
        AccountAdmission::Refused(AccountRefusal::AlreadyListed) => {
            proven().and(Err(NotVerified::Listing(AccountRefusal::AlreadyListed)))
        }
        AccountAdmission::Refused(refusal) => Err(NotVerified::Listing(refusal)),
        AccountAdmission::Admitted(admitted) => proven()
            .and_then(|ownership| add_account(app, &ownership, attempt.admitted_event(admitted))),
    };
    (profile.into_profile().login, outcome)
}

/// The pure account admission (CORE-RRA-8) on the owner's list as read now.
fn admission(
    app: &App,
    session: &WebSession,
    listed: &[GithubLink],
    github_user_id: u64,
) -> AccountAdmission {
    let standings: Vec<(u64, Standing)> = listed
        .iter()
        .map(|account| (account.github_user_id, standing_of(account)))
        .collect();
    let other_owner = match app
        .links
        .verified_for_another_owner(&session.owner_did, github_user_id)
    {
        Ok(true) => OtherOwner::HoldsItVerified,
        _ => OtherOwner::None,
    };
    admit_account(&standings, github_user_id, other_owner, MAX_GITHUB_ACCOUNTS)
}

fn standing_of(account: &GithubLink) -> Standing {
    if account.verified {
        Standing::Verified
    } else {
        Standing::Unverified
    }
}

/// Add the proven account (both limits checked again as it is written); its
/// verification is counted with it.
fn add_account(
    app: &App,
    ownership: &VerifiedOwnership,
    event: LogEvent<'_>,
) -> Result<AccountAdmitted, NotVerified> {
    let counted = Counted::of(&event);
    match app.accounts_write.add_or_reverify(
        ownership.owner_did(),
        ownership.github_login(),
        ownership.github_user_id(),
        MAX_GITHUB_ACCOUNTS,
        counted.tally(),
    ) {
        Ok(AccountAdmission::Admitted(admitted)) => Ok(admitted),
        Ok(AccountAdmission::Refused(refusal)) => Err(NotVerified::Listing(refusal)),
        Err(_) => Err(NotVerified::Check(OwnershipRefusal::GithubUnavailable)),
    }
}

/// A failed check of an account on the owner's list marks it unverified.
fn unverify_if_listed(
    app: &App,
    session: &WebSession,
    listed: &[GithubLink],
    profile: &GithubAccountProfile,
    refusal: &OwnershipRefusal,
) {
    let is_listed = listed
        .iter()
        .any(|account| account.github_user_id == profile.id);
    if refusal.disproves_ownership() && is_listed {
        let _ = app.account_status.mark_account_unverified(
            &session.owner_did,
            profile.id,
            refusal.label(),
        );
    }
}

/// The facts of a profile the ownership proof reads.
pub(crate) fn account_of(profile: &GithubAccountProfile) -> GithubAccount<'_> {
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

/// The GitHub step page, after a GET or a refused or accepted POST alike:
/// "Publish this link" is offered only while the owner's stored link is
/// verified (RA-9) — the offer is all the row decides, the link itself is
/// proven by the preview's own fresh read — and each Active published link
/// is offered for retraction, read live from her repo every time.
pub(crate) async fn github_page(
    app: &App,
    session: &WebSession,
    cookie_value: &str,
    notice: Option<(OwnershipOutcome<'_>, &str)>,
) -> Reply {
    let active_links = published_links(app, &session.owner_did).await;
    let accounts = app
        .links
        .github_accounts(&session.owner_did)
        .unwrap_or_default();
    Reply::page(
        StatusCode::OK,
        views::github_accounts_page(
            &session.owner_did,
            &csrf_token_for(cookie_value),
            notice,
            &accounts,
            &active_links,
        ),
    )
}
