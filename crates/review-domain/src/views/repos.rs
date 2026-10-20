//! "Choose repos" (US-RRA-001): one Save form per verified GitHub account,
//! each eligible repo a checkbox named by its `owner/name`.

use maud::{html, Markup};

use super::*;
use ports::{ImportedRepo, ListingExtent, LISTING_PAGE_LIMIT, LISTING_PAGE_SIZE};

use crate::repo_scope::{ChooseList, ImportRefusal, LimitRefusal, TicksRoom, SCAN_REPO_CAP};

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

/// The heading of the owner's imports (US-RRA-005).
pub const REPOS_YOU_DONT_OWN_HEADING: &str = "Repos you don't own";

/// Where an import's "Remove" posts (US-RRA-009).
pub const REMOVE_IMPORT_PATH: &str = "/repos/imports/remove";

/// Where the removal confirm's "Remove" posts (US-RRA-009).
pub const CONFIRM_REMOVE_IMPORT_PATH: &str = "/repos/imports/remove/confirm";

/// The button that removes an import.
pub const REMOVE_IMPORT_LABEL: &str = "Remove";

/// AC-001.5: a rate-limited listing.
pub const LISTING_RATE_LIMITED_MESSAGE: &str = "GitHub is rate-limiting the app, so your repo list couldn't be loaded. Your saved choice is unchanged. Please try again in a few minutes.";

/// AC-001.5: any other failed listing.
pub const LISTING_UNREACHABLE_MESSAGE: &str = "GitHub couldn't be reached, so your repo list couldn't be loaded. Your saved choice is unchanged. Please try again in a few minutes.";

/// Review R3: the app spent its GitHub allowance for now, so it did not ask.
pub const LISTING_BUSY_MESSAGE: &str = "GitHub is busy right now, so your repo list wasn't loaded. Your saved choice is unchanged. Please try again in a few minutes.";

/// Review R3: a Save or "Add repo" the app could not check with GitHub now.
pub const GITHUB_BUSY_MESSAGE: &str =
    "GitHub is busy right now, so nothing was changed. Please try again in a few minutes.";

/// Review R2: another change to her repos or accounts held the way too long.
pub const SCOPE_BUSY_MESSAGE: &str = "Another change to your repos or accounts is still being saved, so nothing was changed. Please try again in a moment.";

/// Why an account's repos could not be listed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListingFailure {
    RateLimited,
    Unreachable,
    /// The app's GitHub allowance for page actions is spent for now (R3).
    Busy,
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
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReposNotice {
    /// A Save stored a choice whose next scan reads `reads` repos.
    Saved { reads: usize },
    /// A Save was refused: the total would pass the limit (BR-RRA-3).
    OverLimit(LimitRefusal),
    /// "Add repo" of `repo` (as typed) was refused, nothing changed.
    ImportRefused {
        refusal: ImportRefusal,
        repo: String,
    },
    /// The import `repo` and its `deleted_pending` pending suggestions were
    /// deleted (US-RRA-009).
    ImportRemoved {
        repo: String,
        deleted_pending: usize,
    },
    /// GitHub could not be asked now (R3); nothing changed.
    GithubBusy,
    /// Another change to her scope held it too long (R2); nothing changed.
    ScopeBusy,
}

impl ReposNotice {
    /// Whether the notice refuses the request (an alert, not a status).
    pub fn is_refusal(&self) -> bool {
        matches!(
            self,
            Self::OverLimit(_) | Self::ImportRefused { .. } | Self::GithubBusy | Self::ScopeBusy
        )
    }
}

/// The choose page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReposView<'a> {
    pub csrf_token: &'a str,
    pub accounts: Vec<AccountRepos<'a>>,
    /// The public repos she named, in the order she added them.
    pub imports: &'a [ImportedRepo],
    /// The repos her next scan reads: saved and default ticks and imports.
    pub chosen: usize,
    pub notice: Option<ReposNotice>,
}

/// "7 of 10 repos chosen." (BR-RRA-3).
pub fn chosen_line(chosen: usize) -> String {
    format!("{chosen} of {SCAN_REPO_CAP} repos chosen.")
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
        "{} Untick {} to save.",
        over_limit_lead(refusal),
        refusal.excess
    )
}

/// What every over-limit refusal opens with: the limit and the total.
fn over_limit_lead(refusal: LimitRefusal) -> String {
    format!(
        "You can choose up to {} repos per scan in total. You have {} chosen.",
        refusal.limit, refusal.total
    )
}

/// AC-009.1: "rust-lang/cargo removed. Its 3 pending suggestions were deleted."
pub fn import_removed_message(repo: &str, deleted_pending: usize) -> String {
    format!(
        "{repo} removed. Its {} {} deleted.",
        counted(deleted_pending, "pending suggestion", "pending suggestions"),
        if deleted_pending == 1 { "was" } else { "were" }
    )
}

/// The import removal confirm (AC-009.1): what removing `repo` deletes and
/// what it keeps. Nothing is deleted here; Cancel goes back unchanged.
pub fn remove_import_confirm_page(
    import: &ImportedRepo,
    pending: usize,
    csrf_token: &str,
) -> String {
    page(
        "Remove this repo?",
        html! {
            h1 { "Remove " (import.full_name) "?" }
            p {
                strong {
                    "This deletes its "
                    (counted(pending, "pending suggestion", "pending suggestions")) "."
                }
            }
            p { "It is no longer scanned. Suggestions you declined stay declined." }
            form method="post" action=(CONFIRM_REMOVE_IMPORT_PATH) {
                (csrf_field(csrf_token))
                input type="hidden" name=(IMPORT_REPO_FIELD) value=(import.repo_id);
                button type="submit" { (REMOVE_IMPORT_LABEL) }
            }
            p { a href=(REPOS_PATH) { (CANCEL_LABEL) } }
        },
    )
}

/// AC-005.*: the cause of a refused "Add repo" of `repo` and the fix.
pub fn import_refusal_message(refusal: ImportRefusal, repo: &str) -> String {
    match refusal {
        ImportRefusal::Shape => IMPORT_SHAPE_MESSAGE.to_string(),
        ImportRefusal::NotFound => format!(
            "We couldn't find a public repo at github.com/{repo}. Check the spelling. Private repos can't be read."
        ),
        ImportRefusal::Unreachable => format!(
            "GitHub could not be reached, so we couldn't check github.com/{repo}. Please try again in a few minutes. Nothing was lost."
        ),
        ImportRefusal::OwnedByListedAccount => {
            format!("github.com/{repo} is yours. Tick it in your repo list instead.")
        }
        ImportRefusal::AlreadyImported => format!("github:{repo} is already in your list."),
        ImportRefusal::OverLimit(refusal) => format!(
            "You can choose up to {} repos per scan in total. Adding it would make {}. Untick \
             or remove {} before adding another.",
            refusal.limit,
            refusal.total,
            counted(refusal.excess, "repo", "repos")
        ),
    }
}

/// AC-005.6: a name that is not `owner/name`.
pub const IMPORT_SHAPE_MESSAGE: &str = "Enter the repo as owner/name, for example rust-lang/cargo.";

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
        ListingFailure::Busy => LISTING_BUSY_MESSAGE,
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
            @match &view.notice {
                Some(ReposNotice::Saved { reads }) => {
                    p role="status" { strong { (saved_message(*reads)) } }
                }
                Some(ReposNotice::OverLimit(refusal)) => {
                    p role="alert" { strong { (save_over_limit_message(*refusal)) } }
                }
                Some(ReposNotice::ImportRefused { refusal, repo }) => {
                    p role="alert" { strong { (import_refusal_message(*refusal, repo)) } }
                }
                Some(ReposNotice::ImportRemoved { repo, deleted_pending }) => {
                    p role="status" { strong { (import_removed_message(repo, *deleted_pending)) } }
                }
                Some(ReposNotice::GithubBusy) => {
                    p role="alert" { strong { (GITHUB_BUSY_MESSAGE) } }
                }
                Some(ReposNotice::ScopeBusy) => {
                    p role="alert" { strong { (SCOPE_BUSY_MESSAGE) } }
                }
                None => {}
            }
            p { "Tick the repos your next scan reads." }
            @for account in &view.accounts {
                (account_section(account, view.csrf_token))
            }
            (imports_section(view.imports, view.csrf_token))
            p { (chosen_line(view.chosen)) }
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
                    p {
                        "GitHub lists more repos than shown here: these are the first "
                        (LISTING_PAGE_LIMIT * LISTING_PAGE_SIZE) "."
                    }
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
            (add_repo_form(csrf_token))
        }
    }
}

/// "Add repo": name a public repo as `owner/name`.
fn add_repo_form(csrf_token: &str) -> Markup {
    html! {
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

/// The repos she named that none of her accounts owns: their suggestions
/// stay private. Each can be removed; any public repo can be added.
fn imports_section(imports: &[ImportedRepo], csrf_token: &str) -> Markup {
    html! {
        section {
            h2 { (REPOS_YOU_DONT_OWN_HEADING) }
            @if imports.is_empty() {
                p { "Name a public repo you don't own to get private suggestions about it." }
            }
            ul {
                @for import in imports {
                    li {
                        form method="post" action=(REMOVE_IMPORT_PATH) {
                            (csrf_field(csrf_token))
                            input type="hidden" name=(IMPORT_REPO_FIELD) value=(import.repo_id);
                            (import.full_name) " "
                            button type="submit" { (REMOVE_IMPORT_LABEL) }
                        }
                    }
                }
            }
            (add_repo_form(csrf_token))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Mutation pin (Phase 5): only a refused change is a refusal (the
    /// page answers 422 for it); a save or a removal is not.
    #[test]
    fn only_a_refused_change_is_a_refusal() {
        let over = LimitRefusal {
            limit: 10,
            total: 11,
            excess: 1,
        };
        let notices = [
            (ReposNotice::Saved { reads: 5 }, false),
            (
                ReposNotice::ImportRemoved {
                    repo: "rust-lang/cargo".into(),
                    deleted_pending: 0,
                },
                false,
            ),
            (ReposNotice::OverLimit(over), true),
            (
                ReposNotice::ImportRefused {
                    refusal: ImportRefusal::NotFound,
                    repo: "rust-lang/cargo".into(),
                },
                true,
            ),
            (ReposNotice::GithubBusy, true),
            (ReposNotice::ScopeBusy, true),
        ];
        for (notice, refusal) in notices {
            assert_eq!(notice.is_refusal(), refusal, "{notice:?}");
        }
    }

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
