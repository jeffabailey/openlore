//! "Choose repos" (US-RRA-001): each verified account's owned repos are
//! listed live from GitHub when the page opens, ticked as its saved choice
//! (or today's top 5) says; Save stores exactly the ticked repo ids. The
//! decisions are `review_domain::repo_scope`'s; this module only reads and
//! writes. Nothing here reaches any PDS.

use std::collections::BTreeSet;

use hyper::StatusCode;
use ports::{GithubLink, KpiTally, WebSession};
use review_domain::repo_scope::{choice_to_save, choose_list, AccountChoice};
use review_domain::views::{
    self, AccountListing, AccountRepos, ReposNotice, ReposView, ACCOUNT_ID_FIELD, REPO_FIELD,
};

use crate::http::{App, PageRequest, Reply};
use crate::routes::review::not_found;
use crate::routes::signin::{
    csrf_token_for, csrf_token_of, current_session, field, fields, signed_in_post, to_landing,
};

/// `GET /repos`: the choose page.
pub(crate) async fn choose_repos(app: &App, request: &PageRequest) -> Reply {
    let Some((cookie_value, session)) = current_session(app, request) else {
        return to_landing();
    };
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
    let choice = choice_to_save(&listing, &ticked);
    let saved = app.choices_write.save_choice(
        &session.owner_did,
        account.github_user_id,
        &choice,
        KpiTally::NONE,
    );
    let notice = saved.ok().map(|()| ReposNotice::Saved {
        reads: choice.iter().filter(|row| row.ticked).count(),
    });
    repos_page(app, &session, &csrf_token, notice).await
}

/// The owner's verified accounts, in the order they were added.
fn verified_accounts(app: &App, owner_did: &str) -> Vec<GithubLink> {
    app.links
        .github_accounts(owner_did)
        .unwrap_or_default()
        .into_iter()
        .filter(|account| account.verified)
        .collect()
}

fn verified_account(app: &App, owner_did: &str, github_user_id: u64) -> Option<GithubLink> {
    verified_accounts(app, owner_did)
        .into_iter()
        .find(|account| account.github_user_id == github_user_id)
}

/// One account's section: its live listing, ticked as its choice says.
async fn account_listing(app: &App, owner_did: &str, account: &GithubLink) -> AccountListing {
    match app.github.list_account_repos(&account.github_login).await {
        Ok(listing) => {
            let saved = app
                .choices_read
                .saved_choice(owner_did, account.github_user_id)
                .unwrap_or_default();
            AccountListing::Listed(choose_list(&listing, AccountChoice::of(&saved)))
        }
        Err(_) => AccountListing::Unavailable,
    }
}

async fn repos_page(
    app: &App,
    session: &WebSession,
    csrf_token: &str,
    notice: Option<ReposNotice>,
) -> Reply {
    let accounts = verified_accounts(app, &session.owner_did);
    let mut sections = Vec::with_capacity(accounts.len());
    for account in &accounts {
        sections.push(AccountRepos {
            github_user_id: account.github_user_id,
            login: &account.github_login,
            listing: account_listing(app, &session.owner_did, account).await,
        });
    }
    Reply::page(
        StatusCode::OK,
        views::repos_page(&ReposView {
            csrf_token,
            accounts: sections,
            notice,
        }),
    )
}
