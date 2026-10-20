//! "Choose repos" (US-RRA-001): each verified account's owned repos are
//! listed live from GitHub when the page opens, ticked as its saved choice
//! (or today's top 5) says; Save stores exactly the ticked repo ids. The
//! decisions are `review_domain::repo_scope`'s; this module only reads and
//! writes. Nothing here reaches any PDS.

use std::collections::BTreeSet;

use hyper::StatusCode;
use ports::{
    AccountRepoListing, GithubError, GithubLink, ImportedRepo, KpiTally, RepoSource,
    SavedRepoChoice, WebSession, LISTING_PAGE_LIMIT,
};
use review_domain::budget::PageReadAdmission;
use review_domain::kpi::{choice_saved_events, utc_day, KpiEvent};
use review_domain::repo_scope::{
    admit_save, choice_to_save, choose_list, chosen_total, default_ticks, differs_from_default,
    eligible_count, ticked_ids, AccountChoice, ScopeAccount, Standing, SCAN_REPO_CAP,
};
use review_domain::views::{
    self, AccountListing, AccountRepos, ListingFailure, ReposNotice, ReposView, ACCOUNT_ID_FIELD,
    REPO_FIELD,
};

use crate::http::{App, PageRequest, Reply};
use crate::routes::review::not_found;
use crate::routes::signin::{
    csrf_token_for, csrf_token_of, current_session, fields, id_field, signed_in_post, to_landing,
};
use crate::wiring::ScopeBusy;

/// `GET /repos`: the choose page. Opening it stores nothing; it is counted.
pub(crate) async fn choose_repos(app: &App, request: &PageRequest) -> Reply {
    let Some((cookie_value, session)) = current_session(app, request) else {
        return to_landing();
    };
    let today = utc_day(crate::limiter::unix_now_secs());
    let _ = app.kpi.count(&today, KpiEvent::ReposChoiceViewed.name());
    let accounts = listed_accounts(app, &session.owner_did);
    let listings = read_listings(app, &session.owner_did, &verified_logins(&accounts), 0)
        .await
        .unwrap_or_default();
    repos_page(
        app,
        &session,
        &csrf_token_for(&cookie_value),
        None,
        &listings,
    )
}

/// `POST /repos`: "Save choice" for one of the owner's verified accounts.
/// The ticks are read against this request's one listing of each verified
/// account (R3), read before the scope is held (R2); under the scope the
/// account is looked up again and the store writes only while she still
/// lists it, so a Save never brings back a removed account's choice (R1).
/// An account that is not the owner's is not found.
pub(crate) async fn save_choice(app: &App, request: &PageRequest) -> Reply {
    let (_, session) = match signed_in_post(app, request) {
        Ok(signed_in) => signed_in,
        Err(refused) => return refused,
    };
    let owner_did = session.owner_did.as_str();
    let accounts = listed_accounts(app, owner_did);
    let named = id_field(&request.form, ACCOUNT_ID_FIELD);
    let Some(account_id) = named.filter(|id| is_verified(&accounts, *id)) else {
        return not_found();
    };
    let ticked: BTreeSet<u64> = fields(&request.form, REPO_FIELD)
        .iter()
        .filter_map(|id| id.parse().ok())
        .collect();
    let csrf_token = csrf_token_of(request);
    let Some(listings) = read_listings(app, owner_did, &verified_logins(&accounts), 0).await else {
        let busy = Some(ReposNotice::GithubBusy);
        return repos_page(app, &session, &csrf_token, busy, &Listings::default());
    };
    let Some(listing) = listings.listed(account_id) else {
        return repos_page(app, &session, &csrf_token, None, &listings);
    };
    // Ticks and limit are judged on listings read before the lock (N5): safe, as the store
    // re-checks the account under the scope, and the scan and every publish re-read GitHub.
    let notice = match app.scope_locks.hold(owner_did).await {
        Err(ScopeBusy) => Some(ReposNotice::ScopeBusy),
        Ok(scope_guard) => {
            let stored = store_choice(app, owner_did, account_id, listing, &ticked, &listings);
            drop(scope_guard);
            match stored {
                Some(notice) => notice,
                None => return not_found(),
            }
        }
    };
    repos_page(app, &session, &csrf_token, notice, &listings)
}

/// Under the scope: the Save of `ticked` on `listing` for the account
/// `account_id`, checked against the store as it is now and this request's
/// `listings`. `None` when she no longer lists the account verified (it was
/// removed meanwhile): nothing is written.
fn store_choice(
    app: &App,
    owner_did: &str,
    account_id: u64,
    listing: &AccountRepoListing,
    ticked: &BTreeSet<u64>,
    listings: &Listings,
) -> Option<Option<ReposNotice>> {
    if !is_verified(&listed_accounts(app, owner_did), account_id) {
        return None;
    }
    let choice = choice_to_save(&listing.repos, ticked);
    let today = utc_day(crate::limiter::unix_now_secs());
    let admitted = admit_save(
        &owner_scope(app, owner_did, listings),
        &import_ids(app, owner_did),
        account_id,
        &ticked_ids(&choice),
        SCAN_REPO_CAP,
    );
    match admitted {
        Err(refusal) => {
            let _ = app
                .kpi
                .count(&today, KpiEvent::ReposChoiceRefusedOverLimit.name());
            Some(Some(ReposNotice::OverLimit(refusal)))
        }
        Ok(reads) => {
            let events: Vec<&'static str> =
                choice_saved_events(differs_from_default(&listing.repos, &choice))
                    .into_iter()
                    .map(KpiEvent::name)
                    .collect();
            let tally = KpiTally {
                day: &today,
                events: &events,
            };
            match app
                .choices_write
                .save_choice(owner_did, account_id, &choice, tally)
            {
                Ok(true) => Some(Some(ReposNotice::Saved { reads })),
                Ok(false) => None,
                Err(_) => Some(None),
            }
        }
    }
}

/// What this request asked GitHub for one verified account's repos.
pub(crate) enum ListingAnswer {
    Listed(AccountRepoListing),
    Failed(GithubError),
    /// Not asked in this request: the page-read allowance refused it (R3),
    /// or the account was verified after this request read GitHub.
    NotAsked,
}

/// This request's listings of the owner's verified accounts: each asked of
/// GitHub at most once per request (R3), before any scope is held (R2).
#[derive(Default)]
pub(crate) struct Listings(Vec<(u64, ListingAnswer)>);

static NOT_ASKED: ListingAnswer = ListingAnswer::NotAsked;

impl Listings {
    /// What this request asked GitHub for `github_user_id`'s repos.
    pub(crate) fn answer(&self, github_user_id: u64) -> &ListingAnswer {
        self.0
            .iter()
            .find(|(id, _)| *id == github_user_id)
            .map_or(&NOT_ASKED, |(_, answer)| answer)
    }

    /// The listing of `github_user_id`, when this request read it.
    pub(crate) fn listed(&self, github_user_id: u64) -> Option<&AccountRepoListing> {
        match self.answer(github_user_id) {
            ListingAnswer::Listed(listing) => Some(listing),
            _ => None,
        }
    }
}

/// The id and login of each verified account, in the order they were added.
pub(crate) fn verified_logins(accounts: &[GithubLink]) -> Vec<(u64, &str)> {
    accounts
        .iter()
        .filter(|account| account.verified)
        .map(|account| (account.github_user_id, account.github_login.as_str()))
        .collect()
}

/// Admit `wanted` GitHub requests of a page action by `owner_did` against
/// her hourly allowance (R3).
pub(crate) fn page_reads_admitted(app: &App, owner_did: &str, wanted: usize) -> bool {
    app.page_reads.admit(owner_did, wanted) == PageReadAdmission::Admitted
}

/// List each of `verified` once, admitted with `also` more requests the
/// action makes (R3); `None` when the allowance refuses: nothing was asked.
/// Each listing is admitted for its page limit, every page an HTTP request
/// (re-review N2); the pages a listing did not take are given back, and a
/// failed listing keeps its whole admission (which page failed is unknown).
pub(crate) async fn read_listings(
    app: &App,
    owner_did: &str,
    verified: &[(u64, &str)],
    also: usize,
) -> Option<Listings> {
    if !page_reads_admitted(app, owner_did, verified.len() * LISTING_PAGE_LIMIT + also) {
        return None;
    }
    let mut listings = Vec::with_capacity(verified.len());
    for (github_user_id, login) in verified {
        let answer = match app.github.list_account_repos(login).await {
            Ok(listing) => {
                app.page_reads
                    .refund(owner_did, LISTING_PAGE_LIMIT - listing.pages_read());
                let _ = app.listing_note.record_eligible_count(
                    owner_did,
                    *github_user_id,
                    eligible_count(&listing.repos),
                );
                ListingAnswer::Listed(listing)
            }
            Err(error) => ListingAnswer::Failed(error),
        };
        listings.push((*github_user_id, answer));
    }
    Some(Listings(listings))
}

/// The owner's imports: they count toward the limit (BR-RRA-3).
pub(crate) fn listed_imports(app: &App, owner_did: &str) -> Vec<ImportedRepo> {
    app.imports_read.imports(owner_did).unwrap_or_default()
}

/// The repo ids of the owner's imports.
pub(crate) fn import_ids(app: &App, owner_did: &str) -> Vec<u64> {
    listed_imports(app, owner_did)
        .iter()
        .map(|import| import.repo_id)
        .collect()
}

/// The owner's accounts, in the order they were added.
pub(crate) fn listed_accounts(app: &App, owner_did: &str) -> Vec<GithubLink> {
    app.links.github_accounts(owner_did).unwrap_or_default()
}

/// How many of the owner's pending suggestions came from `source`: what
/// removing it deletes, named on its confirm page.
pub(crate) fn pending_from(app: &App, owner_did: &str, source: RepoSource) -> usize {
    app.review_read
        .pending_sources(owner_did)
        .unwrap_or_default()
        .iter()
        .filter(|(_, from)| *from == source)
        .count()
}

fn is_verified(accounts: &[GithubLink], github_user_id: u64) -> bool {
    accounts
        .iter()
        .any(|account| account.verified && account.github_user_id == github_user_id)
}

/// What the page knows of one account: its saved rows and, when verified,
/// what this request asked GitHub for its repos (an unverified account's
/// repos are never asked for).
struct AccountState<'a> {
    link: GithubLink,
    saved: Vec<SavedRepoChoice>,
    listing: Option<&'a ListingAnswer>,
}

/// Every listed account's state, in the order they were added: the store
/// as it is now, with this request's `listings`.
fn account_states<'a>(app: &App, owner_did: &str, listings: &'a Listings) -> Vec<AccountState<'a>> {
    listed_accounts(app, owner_did)
        .into_iter()
        .map(|link| AccountState {
            saved: app
                .choices_read
                .saved_choice(owner_did, link.github_user_id)
                .unwrap_or_default(),
            listing: link.verified.then(|| listings.answer(link.github_user_id)),
            link,
        })
        .collect()
}

/// The owner's accounts as the limit counts them, from the store as it is
/// now and this request's `listings` (no GitHub request).
pub(crate) fn owner_scope(app: &App, owner_did: &str, listings: &Listings) -> Vec<ScopeAccount> {
    account_states(app, owner_did, listings)
        .iter()
        .map(scope_account)
        .collect()
}

fn scope_account(state: &AccountState<'_>) -> ScopeAccount {
    let listed = match state.listing {
        Some(ListingAnswer::Listed(listing)) => Some(listing.repos.as_slice()),
        _ => None,
    };
    ScopeAccount::of(
        state.link.github_user_id,
        Standing::of(state.link.verified),
        &state.saved,
        listed,
    )
}

fn listing_failure(error: &GithubError) -> ListingFailure {
    match error {
        GithubError::RateLimited { .. } => ListingFailure::RateLimited,
        _ => ListingFailure::Unreachable,
    }
}

/// One account's section: its listing ticked as its saved choice says, or
/// (never saved) as its default ticks within the room left say.
fn account_listing(state: &AccountState<'_>, defaults: &[(u64, Vec<u64>)]) -> AccountListing {
    match state.listing {
        None => AccountListing::NeedsReverifying,
        Some(ListingAnswer::NotAsked) => AccountListing::Unavailable(ListingFailure::Busy),
        Some(ListingAnswer::Failed(error)) => AccountListing::Unavailable(listing_failure(error)),
        Some(ListingAnswer::Listed(listing)) => {
            let default_ticked = defaults
                .iter()
                .find(|(id, _)| *id == state.link.github_user_id)
                .map_or(&[][..], |(_, ticked)| ticked.as_slice());
            let choice = match AccountChoice::of(&state.saved) {
                AccountChoice::Saved(rows) => AccountChoice::Saved(rows),
                _ => AccountChoice::DefaultTicks(default_ticked),
            };
            AccountListing::Listed(choose_list(&listing.repos, choice), listing.extent)
        }
    }
}

/// The choose page from the store as it is now and this request's
/// `listings`: rendering it asks GitHub nothing more (R3).
pub(crate) fn repos_page(
    app: &App,
    session: &WebSession,
    csrf_token: &str,
    notice: Option<ReposNotice>,
    listings: &Listings,
) -> Reply {
    let states = account_states(app, &session.owner_did, listings);
    let scope: Vec<ScopeAccount> = states.iter().map(scope_account).collect();
    let imports = listed_imports(app, &session.owner_did);
    let ids: Vec<u64> = imports.iter().map(|import| import.repo_id).collect();
    let defaults = default_ticks(&scope, &ids, SCAN_REPO_CAP);
    let sections = states
        .iter()
        .map(|state| AccountRepos {
            github_user_id: state.link.github_user_id,
            login: &state.link.github_login,
            listing: account_listing(state, &defaults),
        })
        .collect();
    let status = match &notice {
        Some(notice) if notice.is_refusal() => StatusCode::UNPROCESSABLE_ENTITY,
        _ => StatusCode::OK,
    };
    Reply::page(
        status,
        views::repos_page(&ReposView {
            csrf_token,
            accounts: sections,
            imports: &imports,
            chosen: chosen_total(&scope, &ids, SCAN_REPO_CAP),
            notice,
        }),
    )
}
