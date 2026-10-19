//! "Choose repos" (US-RRA-001): one Save form per verified GitHub account,
//! each eligible repo a checkbox named by its `owner/name`.

use maud::{html, Markup};

use super::*;
use crate::repo_scope::ChooseList;

/// Where the choose page lives and its Save form posts.
pub const REPOS_PATH: &str = "/repos";

/// The button that stores an account's ticks.
pub const SAVE_CHOICE_LABEL: &str = "Save choice";

/// The form field naming the account a Save is for.
pub const ACCOUNT_ID_FIELD: &str = "github_user_id";

/// The form field of each ticked repo (its numeric GitHub id).
pub const REPO_FIELD: &str = "repo";

/// What GitHub said when an account's repos were listed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AccountListing {
    Listed(ChooseList),
    /// The listing could not be read; nothing can be chosen right now.
    Unavailable,
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

fn counted(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

/// The choose-repos page.
pub fn repos_page(view: &ReposView<'_>) -> String {
    page(
        "Choose repos",
        html! {
            h1 { "Choose repos" }
            @if let Some(ReposNotice::Saved { reads }) = view.notice {
                p role="status" { strong { (saved_message(reads)) } }
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
        AccountListing::Listed(list) => html! {
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
                                }
                            }
                        }
                    }
                    @if let Some(skipped) = skipped_message(list.forks_skipped, list.archived_skipped) {
                        p { (skipped) }
                    }
                }
                button type="submit" { (SAVE_CHOICE_LABEL) }
            }
        },
        AccountListing::Unavailable => html! {
            section {
                h2 { (name) }
                p { "Your repo list couldn't be loaded. Your saved choice is unchanged. Please try again in a few minutes." }
            }
        },
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
