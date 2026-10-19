//! "Choose repos" (US-RRA-001): one Save form per verified GitHub account,
//! each eligible repo a checkbox named by its `owner/name`.

use maud::{html, Markup};

use super::*;
use ports::ListingExtent;

use crate::repo_scope::{ChooseList, LimitRefusal, TicksRoom, SCAN_REPO_CAP};

/// Where the choose page lives and its Save form posts.
pub const REPOS_PATH: &str = "/repos";

/// The button that stores an account's ticks.
pub const SAVE_CHOICE_LABEL: &str = "Save choice";

/// D-RRA-6: a repo listed since the account's choice was saved.
pub const NEW_SINCE_CHOICE_MARK: &str = "new since your last choice";

/// The form field naming the account a Save is for.
pub const ACCOUNT_ID_FIELD: &str = "github_user_id";

/// The form field of each ticked repo (its numeric GitHub id).
pub const REPO_FIELD: &str = "repo";

/// Where "Add repo" names a public repo the owner doesn't own (US-RRA-005).
pub const IMPORTS_PATH: &str = "/repos/imports";

/// The "Add repo" field: the repo as `owner/name`.
pub const IMPORT_REPO_FIELD: &str = "repo";

/// The button that names a public repo the owner doesn't own.
pub const ADD_REPO_LABEL: &str = "Add repo";

/// AC-001.5: a rate-limited listing.
pub const LISTING_RATE_LIMITED_MESSAGE: &str = "GitHub is rate-limiting the app, so your repo list couldn't be loaded. Your saved choice is unchanged. Please try again in a few minutes.";

/// AC-001.5: any other failed listing.
pub const LISTING_UNREACHABLE_MESSAGE: &str = "GitHub couldn't be reached, so your repo list couldn't be loaded. Your saved choice is unchanged. Please try again in a few minutes.";

/// Why an account's repos could not be listed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListingFailure {
    RateLimited,
    Unreachable,
}

/// What the choose page can show for one account.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AccountListing {
    Listed(ChooseList, ListingExtent),
    /// The listing could not be read; nothing can be chosen right now.
    Unavailable(ListingFailure),
    /// The account's proof no longer holds: no listing was asked for.
    NeedsReverifying,
}

/// One verified account's section of the choose page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountRepos<'a> {
    pub github_user_id: u64,
    pub login: &'a str,
    pub listing: AccountListing,
}

/// What a request on the choose page did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReposNotice {
    /// A Save stored a choice whose next scan reads `reads` repos.
    Saved { reads: usize },
    /// A Save was refused: the total would pass the limit (BR-RRA-3).
    OverLimit(LimitRefusal),
}

/// The choose page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReposView<'a> {
    pub csrf_token: &'a str,
    pub accounts: Vec<AccountRepos<'a>>,
    pub notice: Option<ReposNotice>,
}

/// "Saved. Your next scan reads 5 repos."
pub fn saved_message(reads: usize) -> String {
    format!(
        "Saved. Your next scan reads {}.",
        counted(reads, "repo", "repos")
    )
}

/// "You can choose up to 10 repos per scan in total. You have 11 chosen. Untick 1 to save."
pub fn save_over_limit_message(refusal: LimitRefusal) -> String {
    format!(
        "You can choose up to {} repos per scan in total. You have {} chosen. Untick {} to save.",
        refusal.limit, refusal.total, refusal.excess
    )
}

/// "2 of 4 ticked: your limit of 10 repos is reached. Untick others to choose more."
pub fn limit_reached_message(list: &ChooseList) -> String {
    let ticked = list.rows.iter().filter(|row| row.ticked).count();
    format!(
        "{ticked} of {} ticked: your limit of {SCAN_REPO_CAP} repos is reached. Untick others \
         to choose more.",
        list.rows.len()
    )
}

/// "Skipped: 1 fork, 1 archived repo." (`None` when nothing was skipped).
pub fn skipped_message(forks: usize, archived: usize) -> Option<String> {
    let parts: Vec<String> = [
        (forks, counted(forks, "fork", "forks")),
        (
            archived,
            counted(archived, "archived repo", "archived repos"),
        ),
    ]
    .into_iter()
    .filter(|(n, _)| *n > 0)
    .map(|(_, text)| text)
    .collect();
    (!parts.is_empty()).then(|| format!("Skipped: {}.", parts.join(", ")))
}

/// AC-001.5: the cause of a failed listing and what to do next.
pub fn listing_failure_message(failure: ListingFailure) -> &'static str {
    match failure {
        ListingFailure::RateLimited => LISTING_RATE_LIMITED_MESSAGE,
        ListingFailure::Unreachable => LISTING_UNREACHABLE_MESSAGE,
    }
}

/// AC-001.7: the fix for an account whose proof no longer holds.
pub fn needs_reverifying_message(login: &str) -> String {
    format!(
        "Your DID is no longer in the bio of github.com/{login}. Put it back, then verify on the GitHub step."
    )
}

fn counted(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

/// The choose-repos page.
pub fn repos_page(view: &ReposView<'_>) -> String {
    page(
        "Choose repos",
        html! {
            h1 { "Choose repos" }
            @match view.notice {
                Some(ReposNotice::Saved { reads }) => {
                    p role="status" { strong { (saved_message(reads)) } }
                }
                Some(ReposNotice::OverLimit(refusal)) => {
                    p role="alert" { strong { (save_over_limit_message(refusal)) } }
                }
                None => {}
            }
            p { "Tick the repos your next scan reads." }
            @for account in &view.accounts {
                (account_section(account, view.csrf_token))
            }
            (back_to_queue())
        },
    )
}

fn account_section(account: &AccountRepos<'_>, csrf_token: &str) -> Markup {
    let name = format!("github.com/{}", account.login);
    match &account.listing {
        AccountListing::Listed(list, extent) if !list.rows.is_empty() => {
            choose_form(account, list, *extent, &name, csrf_token)
        }
        AccountListing::Listed(list, _) => nothing_to_choose(list, &name, csrf_token),
        AccountListing::Unavailable(failure) => html! {
            section {
                h2 { (name) }
                p role="alert" { (listing_failure_message(*failure)) }
            }
        },
        AccountListing::NeedsReverifying => html! {
            section {
                h2 { (name) }
                p { (needs_reverifying_message(account.login)) }
                p { a href="/github" { "Go to the GitHub step" } }
            }
        },
    }
}

fn choose_form(
    account: &AccountRepos<'_>,
    list: &ChooseList,
    extent: ListingExtent,
    name: &str,
    csrf_token: &str,
) -> Markup {
    html! {
        form method="post" action=(REPOS_PATH) {
            (csrf_field(csrf_token))
            input type="hidden" name=(ACCOUNT_ID_FIELD) value=(account.github_user_id);
            fieldset {
                legend { (name) }
                ul {
                    @for row in &list.rows {
                        li {
                            label {
                                input type="checkbox" name=(REPO_FIELD) value=(row.repo_id)
                                    aria-label=(format!("Include {}", row.full_name))
                                    checked[row.ticked];
                                " " (row.full_name)
                                @if row.new_since_choice {
                                    " (" (NEW_SINCE_CHOICE_MARK) ")"
                                }
                            }
                        }
                    }
                }
                @if list.room == TicksRoom::LimitReached {
                    p { (limit_reached_message(list)) }
                }
                @if let Some(skipped) = skipped_message(list.forks_skipped, list.archived_skipped) {
                    p { (skipped) }
                }
                @if extent == ListingExtent::Truncated {
                    p { "GitHub lists more repos than shown here: these are the first 300." }
                }
            }
            button type="submit" { (SAVE_CHOICE_LABEL) }
        }
    }
}

/// An account with no eligible repo (RC-4) says why, and offers naming a
/// public repo instead.
fn nothing_to_choose(list: &ChooseList, name: &str, csrf_token: &str) -> Markup {
    html! {
        section {
            h2 { (name) }
            p {
                "Forks and archived repos are skipped. "
                (skipped_message(list.forks_skipped, list.archived_skipped)
                    .unwrap_or_else(|| "No owned public repo was found.".to_string()))
            }
            form method="post" action=(IMPORTS_PATH) {
                (csrf_field(csrf_token))
                label {
                    "Add a public repo "
                    input type="text" name=(IMPORT_REPO_FIELD) placeholder="owner/name";
                }
                " "
                button type="submit" { (ADD_REPO_LABEL) }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // bypass: the wording pins of the DISCUSS mockups, as a table.
    #[test]
    fn saved_and_skipped_lines_count_in_words() {
        assert_eq!(saved_message(5), "Saved. Your next scan reads 5 repos.");
        assert_eq!(saved_message(1), "Saved. Your next scan reads 1 repo.");
        assert_eq!(
            skipped_message(1, 1).as_deref(),
            Some("Skipped: 1 fork, 1 archived repo.")
        );
        assert_eq!(
            skipped_message(0, 2).as_deref(),
            Some("Skipped: 2 archived repos.")
        );
        assert_eq!(skipped_message(3, 0).as_deref(), Some("Skipped: 3 forks."));
        assert_eq!(skipped_message(0, 0), None);
    }
}
