//! `people` — the pure person-inference area of the scraper domain
//! (contributor-philosophy-inference DDD-1 / DDD-3 / ADR-063 §1).
//!
//! Slice-01 lands the contributor SELECTION: from GitHub's RAW contributors
//! rows (bots included, API order untrusted) to the ranked top-N HUMANS the
//! scrape records as contribution links, plus the bots that were skipped on
//! the way (named to the user). Values in, values out; no I/O.
//!
//! The pipeline:
//! `collapse_by_user_id |> rank_by_contributions |> take_top_humans`.

use std::cmp::Reverse;
use std::collections::BTreeMap;

use ports::{RankedContributor, RawContributor};

/// Default number of HUMAN contributors recorded per scraped repo (D-3: one
/// API page's worth, top by commits).
pub const DEFAULT_CONTRIBUTOR_COUNT: usize = 30;

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proptest_strategies::arb_raw_contributors;
    use proptest::prelude::*;
    use std::collections::BTreeSet;

    /// Oracle: user ids with NO bot-rule row (the distinct humans).
    fn distinct_human_ids(rows: &[RawContributor]) -> BTreeSet<u64> {
        let bot_ids: BTreeSet<u64> = rows
            .iter()
            .filter(|r| is_bot(r))
            .map(|r| r.user_id)
            .collect();
        rows.iter()
            .map(|r| r.user_id)
            .filter(|id| !bot_ids.contains(id))
            .collect()
    }

    proptest! {
        /// Top-N cut + de-dup: exactly min(N, distinct humans) people, each
        /// user id at most once.
        #[test]
        fn selection_records_min_of_n_and_distinct_humans_each_once(
            rows in arb_raw_contributors(),
            top_n in 0usize..=40,
        ) {
            let selection = select_contributors(&rows, top_n);
            let humans = distinct_human_ids(&rows);
            prop_assert_eq!(selection.people.len(), top_n.min(humans.len()));
            let ids: BTreeSet<u64> = selection.people.iter().map(|p| p.github_user_id).collect();
            prop_assert_eq!(ids.len(), selection.people.len());
            prop_assert!(ids.is_subset(&humans));
        }

        /// Bot rule: no recorded person matches it; every named bot does, and
        /// bots are named only while humans are still being collected.
        #[test]
        fn no_bot_is_ever_recorded_and_only_bots_are_named_excluded(
            rows in arb_raw_contributors(),
            top_n in 0usize..=40,
        ) {
            let selection = select_contributors(&rows, top_n);
            let humans = distinct_human_ids(&rows);
            prop_assert!(selection.people.iter().all(|p| !p.login.to_ascii_lowercase().ends_with("[bot]")));
            for bot in &selection.bots_excluded {
                prop_assert!(rows.iter().any(|r| &r.login == bot && !humans.contains(&r.user_id)));
            }
            if top_n == 0 {
                prop_assert!(selection.bots_excluded.is_empty());
            }
        }

        /// Re-rank: ranks are contiguous 1..=k and contributions never rise
        /// as rank falls.
        #[test]
        fn ranks_are_contiguous_and_ordered_by_contributions(
            rows in arb_raw_contributors(),
            top_n in 0usize..=40,
        ) {
            let selection = select_contributors(&rows, top_n);
            let ranks: Vec<u32> = selection.people.iter().map(|p| p.rank).collect();
            let expected: Vec<u32> = (1..=selection.people.len() as u32).collect();
            prop_assert_eq!(ranks, expected);
            prop_assert!(selection.people.windows(2).all(|w| w[0].contributions >= w[1].contributions));
        }

        /// API order is not trusted: any permutation of the rows selects the
        /// identical people and bots.
        #[test]
        fn selection_is_invariant_under_api_row_order(
            (rows, shuffled) in arb_raw_contributors()
                .prop_flat_map(|rows| (Just(rows.clone()), Just(rows).prop_shuffle())),
            top_n in 0usize..=40,
        ) {
            prop_assert_eq!(select_contributors(&rows, top_n), select_contributors(&shuffled, top_n));
        }
    }
}
