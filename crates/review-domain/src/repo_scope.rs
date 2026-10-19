//! Which owned repos a scan reads (US-RRA-001, US-RRA-002; ADR-088 §3, §6).
//! PURE: inputs are the live listing and the saved choice, outputs are the
//! choose page's rows and the scan's plan.
//!
//! An account's eligible repos (owned, not a fork, not archived) are ranked
//! by `scraper_domain::select_person_repos`, the ranking today's scan uses.
//! Without a saved choice its top [`DEFAULT_TICKS`] are ticked; with one,
//! exactly the saved ticks are. A scan reads exactly the ticked repos, and a
//! repo outside today's top 5 is tagged [`OutsideDefault::OwnedChoice`].

use std::collections::BTreeSet;

use ports::{ListedRepo, OutsideDefault, OwnedRepo, SavedRepoChoice};
use scraper_domain::{select_person_repos, DEFAULT_PERSON_REPO_COUNT};

/// Repos ticked for an account without a saved choice: today's top 5.
pub const DEFAULT_TICKS: usize = DEFAULT_PERSON_REPO_COUNT;

/// An account's choice as stored: none saved yet, or the saved rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccountChoice<'a> {
    /// Never saved: today's top 5 are ticked.
    Default,
    /// Never saved: exactly these default ticks, today's top 5 cut to the
    /// room left under the limit ([`default_ticks`]).
    DefaultTicks(&'a [u64]),
    Saved(&'a [SavedRepoChoice]),
}

impl<'a> AccountChoice<'a> {
    /// The choice the stored rows hold (no rows: never saved).
    pub fn of(rows: &'a [SavedRepoChoice]) -> Self {
        if rows.is_empty() {
            Self::Default
        } else {
            Self::Saved(rows)
        }
    }

    /// Is the repo `repo_id`, at `rank` in today's ranking, ticked?
    fn ticks(self, repo_id: u64, rank: usize) -> bool {
        match self {
            Self::Default => rank < DEFAULT_TICKS,
            Self::DefaultTicks(ticked) => ticked.contains(&repo_id),
            Self::Saved(rows) => rows.iter().any(|row| row.repo_id == repo_id && row.ticked),
        }
    }
}

/// Repos one scan may read across every account and import (BR-RRA-3,
/// D-RRA-16): one constant, no setting.
pub const SCAN_REPO_CAP: usize = 10;

/// A change refused because the scan's total would pass the limit: it names
/// the limit, the total it would reach and how many repos to untick.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LimitRefusal {
    pub limit: usize,
    pub total: usize,
    pub excess: usize,
}

/// Whether an account's DID-in-bio proof still holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Standing {
    Verified,
    /// A re-check failed: no listing, no default ticks until re-verified.
    Unverified,
}

/// An account's ticks as the limit counts them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SavedTicks {
    NeverSaved,
    /// The ticked repo ids of the saved choice (they count even while the
    /// account is unverified, OQ-D3).
    Saved(Vec<u64>),
}

/// One listed account, as the 10-repo limit sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScopeAccount {
    pub github_user_id: u64,
    pub standing: Standing,
    pub saved: SavedTicks,
    /// Its eligible repo ids in today's ranking (empty when not listed).
    pub ranked_eligible: Vec<u64>,
}

/// The default ticks of each verified account without a saved choice, in
/// `added_at` order: the first `min(5, eligible, room left)` of its ranking,
/// the room being what the saved ticks, the imports and earlier defaults
/// leave under `cap`.
pub fn default_ticks(
    accounts: &[ScopeAccount],
    imports: &[u64],
    cap: usize,
) -> Vec<(u64, Vec<u64>)> {
    let saved: usize = accounts.iter().map(|account| account.saved.count()).sum();
    let room = cap.saturating_sub(saved + imports.len());
    accounts
        .iter()
        .filter(|account| account.takes_default_ticks())
        .scan(room, |room, account| {
            let ticked = DEFAULT_TICKS.min(account.ranked_eligible.len()).min(*room);
            *room -= ticked;
            Some((
                account.github_user_id,
                account.ranked_eligible[..ticked].to_vec(),
            ))
        })
        .collect()
}

/// Save admission (CORE-RRA-2): a Save of `ticked` on the account
/// `github_user_id` is admitted iff its ticks, the other accounts' saved
/// ticks (verified or not, OQ-D3), the imports and the other accounts'
/// default ticks fit under `cap`. Admitted: the total the next scan reads.
pub fn admit_save(
    accounts: &[ScopeAccount],
    imports: &[u64],
    github_user_id: u64,
    ticked: &[u64],
    cap: usize,
) -> Result<usize, LimitRefusal> {
    let others = |id: &u64| *id != github_user_id;
    let others_saved: usize = accounts
        .iter()
        .filter(|account| others(&account.github_user_id))
        .map(|account| account.saved.count())
        .sum();
    let others_defaults: usize = default_ticks(accounts, imports, cap)
        .iter()
        .filter(|(id, _)| others(id))
        .map(|(_, ticks)| ticks.len())
        .sum();
    within_cap(
        ticked.len() + others_saved + imports.len() + others_defaults,
        cap,
    )
}

/// The total itself when it fits under `cap`, else the refusal naming it.
fn within_cap(total: usize, cap: usize) -> Result<usize, LimitRefusal> {
    if total <= cap {
        Ok(total)
    } else {
        Err(LimitRefusal {
            limit: cap,
            total,
            excess: total - cap,
        })
    }
}

impl SavedTicks {
    /// The ticks a saved choice holds (the stored rows' ticked ids).
    pub fn of(rows: &[SavedRepoChoice]) -> Self {
        if rows.is_empty() {
            Self::NeverSaved
        } else {
            Self::Saved(
                rows.iter()
                    .filter(|row| row.ticked)
                    .map(|row| row.repo_id)
                    .collect(),
            )
        }
    }

    /// How many repos these ticks count toward the limit.
    fn count(&self) -> usize {
        match self {
            Self::NeverSaved => 0,
            Self::Saved(ticked) => ticked.len(),
        }
    }
}

impl ScopeAccount {
    /// An account as the limit sees it: its saved rows, and its eligible
    /// repos ranked from `listing` (`None`: not listed, so none).
    pub fn of(
        github_user_id: u64,
        standing: Standing,
        saved_rows: &[SavedRepoChoice],
        listing: Option<&[ListedRepo]>,
    ) -> Self {
        Self {
            github_user_id,
            standing,
            saved: SavedTicks::of(saved_rows),
            ranked_eligible: listing.map_or_else(Vec::new, |listing| {
                ranked(listing)
                    .0
                    .iter()
                    .map(|listed| listed.repo_id)
                    .collect()
            }),
        }
    }

    fn takes_default_ticks(&self) -> bool {
        self.standing == Standing::Verified && self.saved == SavedTicks::NeverSaved
    }
}

/// One repo row of an account's choose list, in ranking order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChooseRow {
    pub repo_id: u64,
    /// `owner/name`.
    pub full_name: String,
    pub ticked: bool,
}

/// An account's choose list: its eligible repos ranked, and what the
/// listing held that cannot be chosen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChooseList {
    pub rows: Vec<ChooseRow>,
    pub forks_skipped: usize,
    pub archived_skipped: usize,
}

/// One repo a scan reads, in plan order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedRepo {
    pub repo_id: u64,
    /// `owner/name`.
    pub full_name: String,
    /// `Some` when today's scan would never have read it.
    pub outside_default: Option<OutsideDefault>,
}

/// The eligible listed repos in today's ranking, with what was skipped.
fn ranked(listing: &[ListedRepo]) -> (Vec<&ListedRepo>, usize, usize) {
    let owned: Vec<OwnedRepo> = listing.iter().map(|listed| listed.repo.clone()).collect();
    let selection = select_person_repos(&owned, owned.len());
    let ranked = selection
        .chosen
        .iter()
        .filter_map(|chosen| {
            listing
                .iter()
                .find(|listed| listed.repo.full_name == chosen.full_name)
        })
        .collect();
    (ranked, selection.forks_skipped, selection.archived_skipped)
}

/// The choose list of an account: every eligible repo in today's ranking,
/// ticked as `choice` says.
pub fn choose_list(listing: &[ListedRepo], choice: AccountChoice<'_>) -> ChooseList {
    let (ranked, forks_skipped, archived_skipped) = ranked(listing);
    ChooseList {
        rows: ranked
            .into_iter()
            .enumerate()
            .map(|(rank, listed)| ChooseRow {
                repo_id: listed.repo_id,
                full_name: listed.repo.full_name.clone(),
                ticked: choice.ticks(listed.repo_id, rank),
            })
            .collect(),
        forks_skipped,
        archived_skipped,
    }
}

/// What a Save stores: every repo shown (the eligible listing), ticked iff
/// its id is in `ticked`. Ids the listing does not show are ignored.
pub fn choice_to_save(listing: &[ListedRepo], ticked: &BTreeSet<u64>) -> Vec<SavedRepoChoice> {
    ranked(listing)
        .0
        .into_iter()
        .map(|listed| SavedRepoChoice {
            repo_id: listed.repo_id,
            full_name: listed.repo.full_name.clone(),
            ticked: ticked.contains(&listed.repo_id),
        })
        .collect()
}

/// The ticked repo ids of a choice about to be saved.
pub fn ticked_ids(choice: &[SavedRepoChoice]) -> Vec<u64> {
    choice
        .iter()
        .filter(|row| row.ticked)
        .map(|row| row.repo_id)
        .collect()
}

/// Does a saved choice tick anything other than today's top 5?
pub fn differs_from_default(listing: &[ListedRepo], choice: &[SavedRepoChoice]) -> bool {
    let top: BTreeSet<u64> = ranked(listing)
        .0
        .iter()
        .take(DEFAULT_TICKS)
        .map(|listed| listed.repo_id)
        .collect();
    ticked_ids(choice).into_iter().collect::<BTreeSet<u64>>() != top
}

/// The repos one account's scan reads: exactly the ticked rows of its
/// choose list, each tagged when it lies outside today's top 5.
pub fn account_plan(listing: &[ListedRepo], choice: AccountChoice<'_>) -> Vec<PlannedRepo> {
    choose_list(listing, choice)
        .rows
        .into_iter()
        .enumerate()
        .filter(|(_, row)| row.ticked)
        .map(|(rank, row)| PlannedRepo {
            repo_id: row.repo_id,
            full_name: row.full_name,
            outside_default: (rank >= DEFAULT_TICKS).then_some(OutsideDefault::OwnedChoice),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn listed(id: u64, stars: u64, fork: bool, archived: bool) -> ListedRepo {
        ListedRepo {
            repo_id: id,
            owner_id: 1,
            repo: OwnedRepo {
                full_name: format!("me/repo-{id}"),
                description: None,
                language: None,
                stars,
                fork,
                archived,
                pushed_at: None,
            },
        }
    }

    fn arb_listing() -> impl Strategy<Value = Vec<ListedRepo>> {
        prop::collection::vec((0u64..500, any::<bool>(), any::<bool>()), 0..12).prop_map(|rows| {
            rows.into_iter()
                .enumerate()
                .map(|(i, (stars, fork, archived))| listed(i as u64 + 1, stars, fork, archived))
                .collect()
        })
    }

    proptest! {
        /// Universe: listings and any set of ticked ids. A scan reads exactly
        /// the eligible repos ticked at Save, and tags exactly those ranked
        /// outside today's top 5; with no saved choice it reads today's top 5,
        /// none tagged.
        #[test]
        fn a_scan_reads_exactly_the_saved_ticks(
            listing in arb_listing(),
            ticked in prop::collection::btree_set(0u64..14, 0..14),
        ) {
            let (ranked, _, _) = ranked(&listing);
            let saved = choice_to_save(&listing, &ticked);
            let plan = account_plan(&listing, AccountChoice::of(&saved));
            let expected: Vec<u64> = ranked
                .iter()
                .map(|l| l.repo_id)
                .filter(|id| ticked.contains(id))
                .collect();
            prop_assert_eq!(plan.iter().map(|p| p.repo_id).collect::<Vec<_>>(), expected);
            for planned in &plan {
                let rank = ranked.iter().position(|l| l.repo_id == planned.repo_id).unwrap();
                prop_assert_eq!(planned.outside_default.is_some(), rank >= DEFAULT_TICKS);
            }

            let default = account_plan(&listing, AccountChoice::Default);
            let top: Vec<u64> = ranked.iter().take(DEFAULT_TICKS).map(|l| l.repo_id).collect();
            prop_assert_eq!(default.iter().map(|p| p.repo_id).collect::<Vec<_>>(), top);
            prop_assert!(default.iter().all(|p| p.outside_default.is_none()));
        }
    }

    // bypass: the DISCUSS example (7 owned, 1 fork, 1 archived) as a table.
    #[test]
    fn first_visit_ticks_the_top_5_and_counts_what_was_skipped() {
        let listing = [
            listed(1, 412, false, false),
            listed(2, 900, true, false),
            listed(3, 88, false, false),
            listed(4, 700, false, true),
            listed(5, 51, false, false),
            listed(6, 37, false, false),
            listed(7, 12, false, false),
            listed(8, 3, false, false),
            listed(9, 0, false, false),
        ];
        let list = choose_list(&listing, AccountChoice::Default);
        let ticked: Vec<u64> = list
            .rows
            .iter()
            .filter(|r| r.ticked)
            .map(|r| r.repo_id)
            .collect();
        assert_eq!(list.rows.len(), 7);
        assert_eq!(ticked, [1, 3, 5, 6, 7]);
        assert_eq!((list.forks_skipped, list.archived_skipped), (1, 1));
    }
}
