//! "Choose repos" (US-RRA-001): each verified account's owned repos are
//! listed live from GitHub when the page opens, ticked as its saved choice
//! (or today's top 5) says; Save stores exactly the ticked repo ids. The
//! decisions are `review_domain::repo_scope`'s; this module only reads and
//! writes. Nothing here reaches any PDS.

use std::collections::BTreeSet;

use hyper::StatusCode;
use ports::{
    AccountRepoListing, GithubError, GithubLink, ImportedRepo, KpiTally, SavedRepoChoice,
    WebSession,
};
use review_domain::kpi::{choice_saved_events, utc_day, KpiEvent};
use review_domain::repo_scope::{
    admit_save, choice_to_save, choose_list, default_ticks, differs_from_default, eligible_count,
    ticked_ids, AccountChoice, ScopeAccount, Standing, SCAN_REPO_CAP,
};
use review_domain::views::{
    self, AccountListing, AccountRepos, ListingFailure, ReposNotice, ReposView, ACCOUNT_ID_FIELD,
    REPO_FIELD,
};

use crate::http::{App, PageRequest, Reply};
use crate::routes::review::not_found;
use crate::routes::signin::{
    csrf_token_for, csrf_token_of, current_session, field, fields, signed_in_post, to_landing,
};

/// `GET /repos`: the choose page. Opening it stores nothing; it is counted.
pub(crate) async fn choose_repos(app: &App, request: &PageRequest) -> Reply {
    let Some((cookie_value, session)) = current_session(app, request) else {
        return to_landing();
    };
    let today = utc_day(crate::limiter::unix_now_secs());
    let _ = app.kpi.count(&today, KpiEvent::ReposChoiceViewed.name());
    repos_page(app, &session, &csrf_token_for(&cookie_value), None).await
}

/// `POST /repos`: "Save choice" for one of the owner's verified accounts.
/// The ticks are read against a fresh listing, so only repos the page can
/// show are stored; an account that is not the owner's is not found.
pub(crate) async fn save_choice(app: &App, request: &PageRequest) -> Reply {
    let (_, session) = match signed_in_post(app, request) {
        Ok(signed_in) => signed_in,
        Err(refused) => return refused,
    };
    let named = field(&request.form, ACCOUNT_ID_FIELD).and_then(|id| id.parse::<u64>().ok());
    let Some(account) = named.and_then(|id| verified_account(app, &session.owner_did, id)) else {
        return not_found();
    };
    let ticked: BTreeSet<u64> = fields(&request.form, REPO_FIELD)
        .iter()
        .filter_map(|id| id.parse().ok())
        .collect();
    let csrf_token = csrf_token_of(request);
    let Ok(listing) = app.github.list_account_repos(&account.github_login).await else {
        return repos_page(app, &session, &csrf_token, None).await;
    };
    let choice = choice_to_save(&listing.repos, &ticked);
    let today = utc_day(crate::limiter::unix_now_secs());
    let scope = owner_scope(app, &session.owner_did).await;
    let admitted = admit_save(
        &scope,
        &import_ids(app, &session.owner_did),
        account.github_user_id,
        &ticked_ids(&choice),
        SCAN_REPO_CAP,
    );
    let notice = match admitted {
        Err(refusal) => {
            let _ = app
                .kpi
                .count(&today, KpiEvent::ReposChoiceRefusedOverLimit.name());
            Some(ReposNotice::OverLimit(refusal))
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
            app.choices_write
                .save_choice(&session.owner_did, account.github_user_id, &choice, tally)
                .ok()
                .map(|()| ReposNotice::Saved { reads })
        }
    };
    repos_page(app, &session, &csrf_token, notice).await
}

/// The owner's imports: they count toward the limit (BR-RRA-3).
fn listed_imports(app: &App, owner_did: &str) -> Vec<ImportedRepo> {
    app.imports_read.imports(owner_did).unwrap_or_default()
}

fn import_ids(app: &App, owner_did: &str) -> Vec<u64> {
    listed_imports(app, owner_did)
        .iter()
        .map(|import| import.repo_id)
        .collect()
}

/// The owner's accounts, in the order they were added.
fn listed_accounts(app: &App, owner_did: &str) -> Vec<GithubLink> {
    app.links.github_accounts(owner_did).unwrap_or_default()
}

fn verified_account(app: &App, owner_did: &str, github_user_id: u64) -> Option<GithubLink> {
    listed_accounts(app, owner_did)
        .into_iter()
        .find(|account| account.verified && account.github_user_id == github_user_id)
}

/// What the page knows of one account: its saved rows and, when verified,
/// its live listing (an unverified account's repos are never asked for).
struct AccountState {
    link: GithubLink,
    saved: Vec<SavedRepoChoice>,
    listing: Option<Result<AccountRepoListing, GithubError>>,
}

async fn account_state(app: &App, owner_did: &str, link: GithubLink) -> AccountState {
    let listing = if link.verified {
        Some(app.github.list_account_repos(&link.github_login).await)
    } else {
        None
    };
    if let Some(Ok(listed)) = &listing {
        let _ = app.listing_note.record_eligible_count(
            owner_did,
            link.github_user_id,
            eligible_count(&listed.repos),
        );
    }
    let saved = app
        .choices_read
        .saved_choice(owner_did, link.github_user_id)
        .unwrap_or_default();
    AccountState {
        link,
        saved,
        listing,
    }
}

/// Every listed account's state, in the order they were added.
async fn account_states(app: &App, owner_did: &str) -> Vec<AccountState> {
    let mut states = Vec::new();
    for link in listed_accounts(app, owner_did) {
        states.push(account_state(app, owner_did, link).await);
    }
    states
}

/// The owner's accounts as the limit counts them.
async fn owner_scope(app: &App, owner_did: &str) -> Vec<ScopeAccount> {
    account_states(app, owner_did)
        .await
        .iter()
        .map(scope_account)
        .collect()
}

fn standing(link: &GithubLink) -> Standing {
    if link.verified {
        Standing::Verified
    } else {
        Standing::Unverified
    }
}

fn scope_account(state: &AccountState) -> ScopeAccount {
    let listed = match &state.listing {
        Some(Ok(listing)) => Some(listing.repos.as_slice()),
        _ => None,
    };
    ScopeAccount::of(
        state.link.github_user_id,
        standing(&state.link),
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
fn account_listing(state: &AccountState, defaults: &[(u64, Vec<u64>)]) -> AccountListing {
    match &state.listing {
        None => AccountListing::NeedsReverifying,
        Some(Err(error)) => AccountListing::Unavailable(listing_failure(error)),
        Some(Ok(listing)) => {
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

pub(crate) async fn repos_page(
    app: &App,
    session: &WebSession,
    csrf_token: &str,
    notice: Option<ReposNotice>,
) -> Reply {
    let states = account_states(app, &session.owner_did).await;
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
    let status = match notice {
        Some(ReposNotice::OverLimit(_)) => StatusCode::UNPROCESSABLE_ENTITY,
        _ => StatusCode::OK,
    };
    Reply::page(
        status,
        views::repos_page(&ReposView {
            csrf_token,
            accounts: sections,
            imports: &imports,
            notice,
        }),
    )
}
