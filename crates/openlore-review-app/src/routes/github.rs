//! The GitHub step (US-BRA-002, ADR-076; US-RRA-003): show the exact DID
//! to put in the bio, then check the bio of each account the owner adds, up
//! to three, and remove one after a confirm naming its pending suggestions
//! (US-RRA-008). The one holder of the accounts write and removal ports. HTTP concerns and port calls only; the verdict,
//! the capability and every message are `review_domain` functions. The bio
//! itself is never logged.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use hyper::StatusCode;
use ports::claim_domain::identity_link::LinkRecord;
use ports::claim_domain::RecordOrigin;
use ports::{
    AccountAdmission, AccountAdmitted, AccountRefusal, AccountRemoval, GithubAccountProfile,
    GithubError, GithubLink, ImportedRepo, KpiTally, ListedRepo, RepoSource, ReviewStoreError,
    WebSession,
};
use review_domain::budget::{budget_allows, HOUR_SECS, VERIFY_ATTEMPTS_PER_HOUR};
use review_domain::kpi::{utc_day, AccountAddRefused, KpiEvent};
use review_domain::ownership::{
    is_github_login, prove_ownership, GithubAccount, OwnershipRefusal, VerifiedOwnership,
};
use review_domain::published::active_owner_link_records;
use review_domain::repo_scope::{
    admit_account, choice_to_save, current_ticks, moves, owned_imports, OtherOwner, Scope,
    Standing, MAX_GITHUB_ACCOUNTS, SCAN_REPO_CAP,
};
use review_domain::views::{self, GithubStep, OwnershipOutcome};

use crate::http::{App, PageRequest, Reply};
use crate::routes::repos::{
    import_ids, listed_accounts, owner_scope, page_reads_admitted, pending_from, read_listings,
    verified_logins, Listings,
};
use crate::routes::review::not_found;
use crate::routes::signin::{
    csrf_token_for, current_session, field, id_field, signed_in_post, to_landing,
};
use crate::wiring::{emit, observe, Counted, LogEvent, ScopeBusy};

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

/// An account a Verify press added or verified again.
#[derive(Debug, Clone, Copy)]
struct Verified {
    admitted: AccountAdmitted,
    github_user_id: u64,
}

/// Why a Verify press did not verify an account.
enum NotVerified {
    Check(OwnershipRefusal),
    Listing(AccountRefusal),
    /// Another change held her scope too long (R2); nothing changed.
    ScopeBusy,
}

/// `POST /github`: check the named account's bio for the signed-in DID and
/// add it to her list (up to [`MAX_GITHUB_ACCOUNTS`]), or verify a listed
/// one again. GitHub is read first (the profile and, when an import could
/// move, the listings BR-RRA-5 needs), then the scope is held to decide
/// against the store as it is now and write (R2).
pub(crate) async fn verify(app: &App, request: &PageRequest) -> Reply {
    let (cookie_value, session) = match signed_in_post(app, request) {
        Ok(signed_in) => signed_in,
        Err(refused) => return refused,
    };
    let typed = field(&request.form, "github_login").unwrap_or_default();
    let typed = typed.trim();
    let attempt = Attempt::of(&listed_accounts(app, &session.owner_did));
    if attempt == Attempt::AnotherAccount {
        observe(app, LogEvent::AccountAddStarted);
    }
    let read = read_profile(app, &session.owner_did, typed, attempt).await;
    let move_listings = match &read {
        Ok(profile) => listings_for_moves(app, &session.owner_did, profile).await,
        Err(_) => None,
    };
    let (login, outcome) = match app.scope_locks.hold(&session.owner_did).await {
        Err(ScopeBusy) => (typed.to_string(), Err(NotVerified::ScopeBusy)),
        Ok(scope_guard) => {
            let decided = check_bio(app, &session, typed, read);
            if let (Ok(verified), Some(listings)) = (&decided.1, &move_listings) {
                move_owned_imports(app, &session.owner_did, verified.github_user_id, listings);
            }
            drop(scope_guard);
            decided
        }
    };
    let shown = match &outcome {
        Ok(verified) => {
            emit(attempt.admitted_event(verified.admitted));
            OwnershipOutcome::Verified
        }
        Err(not_verified) => {
            count_refusal(app, attempt, not_verified);
            match not_verified {
                NotVerified::Check(refusal) => OwnershipOutcome::Refused(refusal),
                NotVerified::Listing(refusal) => OwnershipOutcome::NotAdded(*refusal),
                NotVerified::ScopeBusy => OwnershipOutcome::ScopeBusy,
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
        NotVerified::ScopeBusy => None,
    };
    if let (Attempt::AnotherAccount, Some(refused)) = (attempt, counted) {
        observe(app, LogEvent::AccountAddRefused(refused));
    }
}

/// Read the typed account's profile, within the hourly verify attempts and
/// the page-read allowance (R3); before any scope is held.
async fn read_profile(
    app: &App,
    owner_did: &str,
    typed: &str,
    attempt: Attempt,
) -> Result<GithubAccountProfile, OwnershipRefusal> {
    if !app.verify_attempts.try_spend(owner_did) {
        return Err(OwnershipRefusal::TooManyAttempts);
    }
    if !is_github_login(typed) {
        return Err(OwnershipRefusal::AccountNotFound);
    }
    if !page_reads_admitted(app, owner_did, 1) {
        let busy = GithubError::RateLimited {
            authenticated: true,
        };
        return Err(attempt.read_refusal(&busy));
    }
    app.github
        .read_account(typed)
        .await
        .map_err(|error| attempt.read_refusal(&error))
}

/// The listings a BR-RRA-5 move after this verify needs, read before the
/// scope is held: only when she lists imports and the profile's bio proves
/// the DID, the verified account's and her other verified accounts' (for
/// the limit). `None`: no move is tried (the next scan stores it).
async fn listings_for_moves(
    app: &App,
    owner_did: &str,
    profile: &GithubAccountProfile,
) -> Option<Listings> {
    let proven = prove_ownership(owner_did, &account_of(profile), None).is_ok();
    if !proven || import_ids(app, owner_did).is_empty() {
        return None;
    }
    let accounts = listed_accounts(app, owner_did);
    let others = verified_logins(&accounts)
        .into_iter()
        .filter(|(id, _)| *id != profile.id);
    let verified: Vec<(u64, &str)> = std::iter::once((profile.id, profile.login.as_str()))
        .chain(others)
        .collect();
    read_listings(app, owner_did, &verified, 0).await
}

/// Under the scope: decide whether the account whose profile was `read`
/// may join the list, prove ownership and add it. Returns the login to
/// name in the message (GitHub's spelling once the profile is read).
fn check_bio(
    app: &App,
    session: &WebSession,
    typed: &str,
    read: Result<GithubAccountProfile, OwnershipRefusal>,
) -> (String, Result<Verified, NotVerified>) {
    let profile = match read {
        Ok(profile) => profile,
        Err(refusal) => return (typed.to_string(), Err(NotVerified::Check(refusal))),
    };
    let listed = listed_accounts(app, &session.owner_did);
    let attempt = Attempt::of(&listed);
    let proven = || {
        prove_ownership(&session.owner_did, &account_of(&profile), None).map_err(|refusal| {
            unverify_if_listed(app, session, &listed, &profile, &refusal);
            NotVerified::Check(refusal)
        })
    };
    let outcome = match admission(app, session, &listed, profile.id) {
        // A listed account's bio is still read: a failed check marks it
        // unverified, as a re-check anywhere does.
        AccountAdmission::Refused(AccountRefusal::AlreadyListed) => {
            proven().and(Err(NotVerified::Listing(AccountRefusal::AlreadyListed)))
        }
        AccountAdmission::Refused(refusal) => Err(NotVerified::Listing(refusal)),
        AccountAdmission::Admitted(admitted) => proven().and_then(|ownership| {
            add_account(app, &ownership, attempt.admitted_event(admitted)).map(|admitted| {
                Verified {
                    admitted,
                    github_user_id: ownership.github_user_id(),
                }
            })
        }),
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
        .map(|account| (account.github_user_id, Standing::of(account.verified)))
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

/// BR-RRA-5 after a verify, under the scope: each import the account
/// `github_user_id` now owns, as this request's `listings` show it, moves to
/// its choice. No GitHub request.
fn move_owned_imports(app: &App, owner_did: &str, github_user_id: u64, listings: &Listings) {
    let imports = import_ids(app, owner_did);
    let Some(listing) = listings.listed(github_user_id) else {
        return;
    };
    let owned = owned_imports(&listing.repos, &imports);
    if owned.is_empty() {
        return;
    }
    let mut scope = Scope {
        accounts: owner_scope(app, owner_did, listings),
        imports,
    };
    for repo_id in owned {
        scope = moves(&scope, repo_id, github_user_id, SCAN_REPO_CAP);
        let ticked = current_ticks(&scope, github_user_id, SCAN_REPO_CAP);
        let _ = store_move(
            app,
            owner_did,
            &listing.repos,
            repo_id,
            github_user_id,
            &ticked,
        );
    }
}

/// Store one BR-RRA-5 move in one transaction, counted with it: the repo,
/// named as the account's `listing` shows it now, joins that account's
/// choice, which (when it had none) becomes `ticked`.
pub(crate) fn store_move(
    app: &App,
    owner_did: &str,
    listing: &[ListedRepo],
    repo_id: u64,
    github_user_id: u64,
    ticked: &[u64],
) -> Result<bool, ReviewStoreError> {
    let Some(listed) = listing.iter().find(|listed| listed.repo_id == repo_id) else {
        return Ok(false);
    };
    let moved = ImportedRepo {
        repo_id,
        full_name: listed.repo.full_name.clone(),
    };
    let choice = choice_to_save(listing, &ticked.iter().copied().collect());
    let today = utc_day(crate::limiter::unix_now_secs());
    let tally = KpiTally {
        day: &today,
        events: &[KpiEvent::ImportMovedToOwned.name()],
    };
    app.scope_moves
        .move_import_to_account(owner_did, &moved, github_user_id, &choice, tally)
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

/// The signed-in owner's listed account named by the form's id. The owner
/// is always the session's: an id she does not list names nothing.
fn named_account(app: &App, session: &WebSession, request: &PageRequest) -> Option<GithubLink> {
    let named = id_field(&request.form, views::ACCOUNT_ID_FIELD)?;
    listed_accounts(app, &session.owner_did)
        .into_iter()
        .find(|account| account.github_user_id == named)
}

/// `POST /github/remove`: the confirm naming how many pending suggestions
/// removing the account deletes. Nothing is deleted here.
pub(crate) fn remove_account_confirm(app: &App, request: &PageRequest) -> Reply {
    let (cookie_value, session) = match signed_in_post(app, request) {
        Ok(signed_in) => signed_in,
        Err(refused) => return refused,
    };
    let Some(account) = named_account(app, &session, request) else {
        return not_found();
    };
    let pending = pending_from(
        app,
        &session.owner_did,
        RepoSource::Account(account.github_user_id),
    );
    Reply::page(
        StatusCode::OK,
        views::remove_account_confirm_page(
            &account.github_login,
            account.github_user_id,
            pending,
            &csrf_token_for(&cookie_value),
        ),
    )
}

/// `POST /github/remove/confirm`: delete the account, its saved choice and
/// its pending suggestions at once (ADR-088 §8); declined and published
/// keys, and her repo, are untouched. Counted with the removal.
pub(crate) async fn remove_account(app: &App, request: &PageRequest) -> Reply {
    let (cookie_value, session) = match signed_in_post(app, request) {
        Ok(signed_in) => signed_in,
        Err(refused) => return refused,
    };
    let Ok(scope_guard) = app.scope_locks.hold(&session.owner_did).await else {
        let shown = OwnershipOutcome::ScopeBusy;
        let message = views::ownership_message(shown, "", &session.owner_did, &session.handle);
        return github_page(app, &session, &cookie_value, Some((shown, &message))).await;
    };
    let Some(account) = named_account(app, &session, request) else {
        return not_found();
    };
    let counted = Counted::of(&LogEvent::AccountRemoved);
    let removal = app.scope_removal.remove_account(
        &session.owner_did,
        account.github_user_id,
        counted.tally(),
    );
    drop(scope_guard);
    let Ok(AccountRemoval::Removed { deleted_pending }) = removal else {
        return github_page(app, &session, &cookie_value, None).await;
    };
    emit(LogEvent::AccountRemoved);
    let shown = OwnershipOutcome::Removed { deleted_pending };
    let message = views::ownership_message(
        shown,
        &account.github_login,
        &session.owner_did,
        &session.handle,
    );
    github_page(app, &session, &cookie_value, Some((shown, &message))).await
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
        gone if gone.is_gone() => OwnershipRefusal::AccountNotFound,
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
    let accounts = listed_accounts(app, &session.owner_did);
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
