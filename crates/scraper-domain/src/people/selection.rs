//! Contributor SELECTION (slice-01; DDD-3): from GitHub's RAW contributors
//! rows (bots included, API order untrusted) to the ranked top-N HUMANS the
//! scrape records as contribution links, plus the bots skipped on the way
//! (named to the user).
//!
//! `collapse_by_user_id |> rank_by_contributions |> take_top_humans`.

use std::cmp::Reverse;
use std::collections::BTreeMap;

use ports::{RankedContributor, RawContributor};

/// Default number of HUMAN contributors recorded per scraped repo (D-3: one
/// API page's worth, top by commits).
pub const DEFAULT_CONTRIBUTOR_COUNT: usize = 30;

/// The largest `--contributors N` a scrape accepts: one GitHub page (per_page
/// max 100), so recording contributors stays ONE request (OD-CPI-7 / DDD-13).
pub const MAX_CONTRIBUTOR_COUNT: usize = 100;

/// Why a `--contributors N` request is refused — before any GitHub request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContributorCountError {
    /// N exceeds one page of contributors (OD-CPI-7).
    AboveOnePage { requested: usize },
    /// `--contributors` was given for a person target (UC-3): a person scrape
    /// never crawls contributors.
    PersonTarget { target: String },
}

impl std::fmt::Display for ContributorCountError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AboveOnePage { requested } => write!(
                f,
                "--contributors {requested} is too many: at most \
                 {MAX_CONTRIBUTOR_COUNT} (one GitHub page) may be recorded"
            ),
            Self::PersonTarget { target } => write!(
                f,
                "--contributors applies only to owner/repo targets; \
                 `{target}` is a person, whose scrape records no contributors"
            ),
        }
    }
}

impl std::error::Error for ContributorCountError {}

/// A scrape target names a repo when it has the `owner/repo` form.
fn is_repo_target(target: &str) -> bool {
    target.contains('/')
}

/// The number of HUMAN contributors to record should `target` be a repo:
/// the `--contributors` override (0..=100) or the default. Refuses an
/// override above one page, or any override on a person target — pure over
/// the raw target, so the shell can refuse BEFORE any GitHub request.
pub fn contributor_count_for(
    target: &str,
    requested: Option<usize>,
) -> Result<usize, ContributorCountError> {
    match requested {
        None => Ok(DEFAULT_CONTRIBUTOR_COUNT),
        Some(_) if !is_repo_target(target) => Err(ContributorCountError::PersonTarget {
            target: target.to_string(),
        }),
        Some(requested) if requested > MAX_CONTRIBUTOR_COUNT => {
            Err(ContributorCountError::AboveOnePage { requested })
        }
        Some(requested) => Ok(requested),
    }
}

/// The outcome of selecting a repo's contributors: the ranked humans to
/// record, and the bots skipped while collecting them (named, never linked).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContributorSelection {
    /// Ranked humans, rank 1..=k contiguous (k = min(N, distinct humans)).
    pub people: Vec<RankedContributor>,
    /// Logins of the bots ranked above the last recorded human (DDD-3).
    pub bots_excluded: Vec<String>,
}

/// The bot rule (DDD-3): GitHub typed the account `Bot`, OR its login ends in
/// `[bot]` (case-insensitive) — the latter catches the API lie of a `[bot]`
/// account typed `User`.
pub fn is_bot(row: &RawContributor) -> bool {
    row.account_type.eq_ignore_ascii_case("bot")
        || row.login.to_ascii_lowercase().ends_with("[bot]")
}

/// Select the top `top_n` HUMAN contributors from raw rows. Total: any input
/// (empty, all bots, duplicates, ties) yields a well-formed selection.
pub fn select_contributors(rows: &[RawContributor], top_n: usize) -> ContributorSelection {
    let accounts = rank_by_contributions(collapse_by_user_id(rows));
    take_top_humans(&accounts, top_n)
}

/// One GitHub account after de-duplication by user id.
#[derive(Debug, Clone)]
struct Account {
    best_row: RawContributor,
    is_bot: bool,
}

/// De-duplicate by numeric user id: keep the account's best row (most
/// contributions, then login asc); the account is a bot if ANY of its rows
/// matches the bot rule (conservative — a bot never sneaks in as a human).
fn collapse_by_user_id(rows: &[RawContributor]) -> Vec<Account> {
    rows.iter()
        .fold(BTreeMap::<u64, Account>::new(), |mut accounts, row| {
            accounts
                .entry(row.user_id)
                .and_modify(|account| {
                    account.is_bot |= is_bot(row);
                    if contribution_order_key(row) < contribution_order_key(&account.best_row) {
                        account.best_row = row.clone();
                    }
                })
                .or_insert_with(|| Account {
                    best_row: row.clone(),
                    is_bot: is_bot(row),
                });
            accounts
        })
        .into_values()
        .collect()
}

/// Re-rank by contributions desc, then login asc (API order is not trusted),
/// with the user id as a final tiebreak so the order is total.
fn rank_by_contributions(mut accounts: Vec<Account>) -> Vec<Account> {
    accounts.sort_by(|a, b| {
        contribution_order_key(&a.best_row).cmp(&contribution_order_key(&b.best_row))
    });
    accounts
}

fn contribution_order_key(row: &RawContributor) -> (Reverse<u64>, String, String, u64, String) {
    (
        Reverse(row.contributions),
        row.login.to_ascii_lowercase(),
        row.login.clone(),
        row.user_id,
        row.account_type.clone(),
    )
}

/// Walk the ranked accounts collecting humans until `top_n` are taken; every
/// bot passed on the way is named in `bots_excluded`.
fn take_top_humans(ranked: &[Account], top_n: usize) -> ContributorSelection {
    let mut people = Vec::new();
    let mut bots_excluded = Vec::new();
    for account in ranked {
        if people.len() >= top_n {
            break;
        }
        if account.is_bot {
            bots_excluded.push(account.best_row.login.clone());
        } else {
            people.push(RankedContributor {
                login: account.best_row.login.clone(),
                github_user_id: account.best_row.user_id,
                rank: u32::try_from(people.len() + 1).unwrap_or(u32::MAX),
                contributions: account.best_row.contributions,
            });
        }
    }
    ContributorSelection {
        people,
        bots_excluded,
    }
}
