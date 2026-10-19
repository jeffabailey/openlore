//! The choose page (US-RRA-001): an account's eligible repos in today's
//! ranking, ticked as its choice says, and what a Save stores. PURE.

use std::collections::BTreeSet;

use ports::{ListedRepo, OutsideDefault, OwnedRepo, SavedRepoChoice};
use scraper_domain::select_person_repos;

use super::DEFAULT_TICKS;

/// An account's choice as stored: none saved yet, or the saved rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccountChoice<'a> {
    /// Never saved: today's top 5 are ticked.
    Default,
    /// Never saved: exactly these default ticks, today's top 5 cut to the
    /// room left under the limit ([`super::default_ticks`]).
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

    /// Is the repo `repo_id` listed since the choice was saved (D-RRA-6)?
    fn is_new(self, repo_id: u64) -> bool {
        match self {
            Self::Saved(rows) => !rows.iter().any(|row| row.repo_id == repo_id),
            Self::Default | Self::DefaultTicks(_) => false,
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

/// One repo row of an account's choose list, in ranking order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChooseRow {
    pub repo_id: u64,
    /// `owner/name`.
    pub full_name: String,
    pub ticked: bool,
    /// Not in the saved choice: listed since it was saved, never read until ticked.
    pub new_since_choice: bool,
}

/// An account's choose list: its eligible repos ranked, and what the
/// listing held that cannot be chosen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChooseList {
    pub rows: Vec<ChooseRow>,
    pub forks_skipped: usize,
    pub archived_skipped: usize,
    pub room: TicksRoom,
}

/// Whether an account's default ticks are today's whole top 5 or were cut
/// to the room the limit leaves (AC-003.7).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TicksRoom {
    Fits,
    LimitReached,
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

/// The tag of the repo at `rank` in today's ranking: outside today's top 5
/// it is an owned choice today's scan would never have read.
pub(super) fn outside_default_at(rank: usize) -> Option<OutsideDefault> {
    (rank >= DEFAULT_TICKS).then_some(OutsideDefault::OwnedChoice)
}

/// How many of a listing's repos are eligible (owned, not a fork, not
/// archived): the account's `eligible_count_seen`.
pub fn eligible_count(listing: &[ListedRepo]) -> usize {
    ranked(listing).0.len()
}

/// The eligible listed repos in today's ranking, with what was skipped.
pub(super) fn ranked(listing: &[ListedRepo]) -> (Vec<&ListedRepo>, usize, usize) {
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
    let room = match choice {
        AccountChoice::DefaultTicks(ticked) if ticked.len() < DEFAULT_TICKS.min(ranked.len()) => {
            TicksRoom::LimitReached
        }
        _ => TicksRoom::Fits,
    };
    ChooseList {
        room,
        rows: ranked
            .into_iter()
            .enumerate()
            .map(|(rank, listed)| ChooseRow {
                repo_id: listed.repo_id,
                full_name: listed.repo.full_name.clone(),
                ticked: choice.ticks(listed.repo_id, rank),
                new_since_choice: choice.is_new(listed.repo_id),
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

/// The ticked repo ids of a choice (saved, or about to be).
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
/// choose list, each tagged when it lies outside today's top 5. The
/// specification [`super::FrozenScope::plan`] is held to; no route plans
/// with it.
#[cfg(test)]
pub(super) fn account_plan(listing: &[ListedRepo], choice: AccountChoice<'_>) -> Vec<PlannedRepo> {
    choose_list(listing, choice)
        .rows
        .into_iter()
        .enumerate()
        .filter(|(_, row)| row.ticked)
        .map(|(rank, row)| PlannedRepo {
            repo_id: row.repo_id,
            full_name: row.full_name,
            outside_default: outside_default_at(rank),
        })
        .collect()
}
