//! Which owned repos a scan reads (US-RRA-001, US-RRA-002; ADR-088 §3, §6).
//! PURE: inputs are the live listing and the saved choice, outputs are the
//! choose page's rows and the scan's plan.
//!
//! An account's eligible repos (owned, not a fork, not archived) are ranked
//! by `scraper_domain::select_person_repos`, the ranking today's scan uses.
//! Without a saved choice its top [`DEFAULT_TICKS`] are ticked; with one,
//! exactly the saved ticks are. A scan reads exactly the ticked repos, and a
//! repo outside today's top 5 is tagged [`OutsideDefault::OwnedChoice`].

use std::collections::{BTreeMap, BTreeSet};

use ports::claim_domain::authority::RepoRead;
use ports::{
    AccountAdmission, AccountAdmitted, AccountRefusal, AccountRepoListing, ImportedRepo,
    ListedRepo, ListingExtent, OutsideDefault, OwnedRepo, PlanRowStatus, PlannedScanRepo,
    RepoFacts, SavedRepoChoice, ScanStatus, SkippedScanRepo,
};
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

/// Repos one scan may read across every account and import (BR-RRA-3,
/// D-RRA-16): one constant, no setting.
pub const SCAN_REPO_CAP: usize = 10;

/// GitHub accounts one Bluesky sign-in may list (D-RRA-15): one constant.
pub const MAX_GITHUB_ACCOUNTS: usize = 3;

/// Whether another DID holds an account verified.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OtherOwner {
    None,
    HoldsItVerified,
}

/// Account admission (CORE-RRA-8): adding `github_user_id` to the owner's
/// `listed` accounts (id and standing, in `added_at` order). A listed
/// account is verified again when unverified, else already listed; an
/// account another DID holds verified is refused; past `max` accounts
/// nothing is added.
pub fn admit_account(
    listed: &[(u64, Standing)],
    github_user_id: u64,
    other_owner: OtherOwner,
    max: usize,
) -> AccountAdmission {
    let listed_as = listed
        .iter()
        .find(|(id, _)| *id == github_user_id)
        .map(|(_, standing)| *standing);
    match (listed_as, other_owner) {
        (Some(Standing::Unverified), _) => AccountAdmission::Admitted(AccountAdmitted::Reverified),
        (Some(Standing::Verified), _) => AccountAdmission::Refused(AccountRefusal::AlreadyListed),
        (None, OtherOwner::HoldsItVerified) => {
            AccountAdmission::Refused(AccountRefusal::HeldByAnotherOwner)
        }
        (None, OtherOwner::None) if listed.len() >= max => {
            AccountAdmission::Refused(AccountRefusal::AtLimit)
        }
        (None, OtherOwner::None) => AccountAdmission::Admitted(AccountAdmitted::Added),
    }
}

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
    let room = cap
        .saturating_sub(saved_total(accounts.iter().map(|account| &account.saved)) + imports.len());
    accounts
        .iter()
        .filter(|account| account.takes_default_ticks())
        .scan(room, |room, account| {
            let ticked = default_count(account.ranked_eligible.len(), *room);
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
    let others_saved = saved_total(
        accounts
            .iter()
            .filter(|account| others(&account.github_user_id))
            .map(|account| &account.saved),
    );
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

/// `S`, the one definition (OQ-D3): the ticks of every saved choice, the
/// account verified or not. Save, import and scan start all count it.
fn saved_total<'a>(saved: impl IntoIterator<Item = &'a SavedTicks>) -> usize {
    saved.into_iter().map(SavedTicks::count).sum()
}

/// What the next scan reads now: `S + I + D`, the saved ticks, the imports
/// and the default ticks within the room they leave.
pub fn chosen_total(accounts: &[ScopeAccount], imports: &[u64], cap: usize) -> usize {
    let defaults: usize = default_ticks(accounts, imports, cap)
        .iter()
        .map(|(_, ticks)| ticks.len())
        .sum();
    saved_total(accounts.iter().map(|account| &account.saved)) + imports.len() + defaults
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

/// How many of a listing's repos are eligible (owned, not a fork, not
/// archived): the account's `eligible_count_seen`.
pub fn eligible_count(listing: &[ListedRepo]) -> usize {
    ranked(listing).0.len()
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

/// What one listed account brings to a scan before it is checked or listed:
/// its saved ticks (frozen at scan start, BR-RRA-4) and, when known, how
/// many eligible repos it had (`None`: unknown, so its default ticks count).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanAccount {
    pub github_user_id: u64,
    pub saved: SavedTicks,
    pub eligible_seen: Option<usize>,
}

/// A scan's start, decided before admission (§6.2 step 1): refused, or the
/// scope frozen for the plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScanIntent {
    /// No account has a non-empty choice or a default, and no import (AC-002.5).
    NothingToRead,
    /// `S + I` passes the limit (BR-RRA-3 scan start).
    OverLimit(LimitRefusal),
    Read(FrozenScope),
}

/// The scope a scan admitted to read: every listed account in `added_at`
/// order, the imports and the limit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrozenScope {
    accounts: Vec<ScanAccount>,
    imports: Vec<u64>,
    cap: usize,
}

/// An account's eligible repos now: their ids in today's ranking, and the
/// listing they were ranked from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Listing {
    pub ranked_eligible: Vec<u64>,
    pub repos: Vec<ListedRepo>,
}

/// The scan's own bio check of one account (D-RRA-T1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AccountCheck {
    /// The bio holds the DID: the account was listed.
    Passed(Listing),
    /// The bio read failed or the DID is gone: none of its repos is read.
    Failed,
    /// Rate limit or outage: not reached, none of its repos is read.
    Unfinished,
}

/// Why a chosen repo is not read.
pub use ports::RepoSkipReason as SkipReason;

/// One chosen repo the scan does not read, and why.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SkippedRepo {
    pub repo_id: u64,
    pub reason: SkipReason,
}

/// What a scan does with one account.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AccountPlan {
    Read {
        planned: Vec<PlannedRepo>,
        skipped: Vec<SkippedRepo>,
    },
    /// Its bio check failed: nothing of it is read.
    Unverified,
    /// Its bio check did not finish: nothing of it is read.
    NotReached,
}

/// A scan's frozen plan: each account's plan in order, then the imports.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanPlan {
    pub accounts: Vec<(u64, AccountPlan)>,
    pub imports: Vec<u64>,
}

/// Decide, before admission, whether a scan starts (CORE-RRA-4): nothing to
/// read and a total past the limit are refused without spending a scan.
pub fn scan_intent(accounts: &[ScanAccount], imports: &[u64], cap: usize) -> ScanIntent {
    if imports.is_empty() && !accounts.iter().any(ScanAccount::has_something_to_read) {
        return ScanIntent::NothingToRead;
    }
    let saved = saved_total(accounts.iter().map(|account| &account.saved));
    match within_cap(saved + imports.len(), cap) {
        Err(refusal) => ScanIntent::OverLimit(refusal),
        Ok(_) => ScanIntent::Read(FrozenScope {
            accounts: accounts.to_vec(),
            imports: imports.to_vec(),
            cap,
        }),
    }
}

impl ScanAccount {
    /// A non-empty saved choice, or default ticks not known to be empty.
    fn has_something_to_read(&self) -> bool {
        match &self.saved {
            SavedTicks::Saved(ticked) => !ticked.is_empty(),
            SavedTicks::NeverSaved => self.eligible_seen != Some(0),
        }
    }

    /// The repos chosen for this account in a scan: its saved ticks, else
    /// (when its check passed) its default ticks within `room`, which they use up.
    fn chosen(&self, check: &AccountCheck, room: &mut usize) -> Vec<u64> {
        match (&self.saved, check) {
            (SavedTicks::Saved(ticked), _) => ticked.clone(),
            (SavedTicks::NeverSaved, AccountCheck::Passed(listing)) => {
                let ticked = default_count(listing.ranked_eligible.len(), *room);
                *room -= ticked;
                listing.ranked_eligible[..ticked].to_vec()
            }
            (SavedTicks::NeverSaved, _) => Vec::new(),
        }
    }
}

/// How many default ticks an account with `eligible` repos gets in `room`.
fn default_count(eligible: usize, room: usize) -> usize {
    DEFAULT_TICKS.min(eligible).min(room)
}

impl Listing {
    /// A listing as GitHub returned it, its eligible repos ranked as today.
    pub fn of(repos: Vec<ListedRepo>) -> Self {
        let ranked_eligible = ranked(&repos)
            .0
            .iter()
            .map(|listed| listed.repo_id)
            .collect();
        Self {
            ranked_eligible,
            repos,
        }
    }

    /// The listed repo `repo_id` when it can be read now, else why not.
    fn readable(&self, repo_id: u64) -> Result<&ListedRepo, SkipReason> {
        match self.repos.iter().find(|listed| listed.repo_id == repo_id) {
            None => Err(SkipReason::MissingOrTransferred),
            Some(listed) if listed.repo.fork => Err(SkipReason::Fork),
            Some(listed) if listed.repo.archived => Err(SkipReason::Archived),
            Some(listed) => Ok(listed),
        }
    }
}

/// The ticked repos of a saved choice a listing does not show, when only a
/// fresh read can say what became of them (I-06): none for a complete
/// listing (absence proves them missing), every unlisted tick for a
/// truncated one (absence proves nothing).
pub fn unlisted_ticks<'a>(
    saved: &'a [SavedRepoChoice],
    listing: &AccountRepoListing,
) -> Vec<&'a SavedRepoChoice> {
    match listing.extent {
        ListingExtent::Complete => Vec::new(),
        ListingExtent::Truncated => saved
            .iter()
            .filter(|row| row.ticked)
            .filter(|row| !listing.repos.iter().any(|l| l.repo_id == row.repo_id))
            .collect(),
    }
}

/// The account's listing completed by the fresh reads of its unlisted
/// ticks: a read joins it when it is the chosen repo (same id) and the
/// account `owner_id` still owns it; any other read leaves it absent, so
/// it is skipped as missing or transferred.
pub fn listing_with_reads(
    listing: AccountRepoListing,
    owner_id: u64,
    reads: &[(u64, RepoFacts)],
) -> Listing {
    let still_owned = reads
        .iter()
        .filter(|(chosen_id, facts)| facts.repo_id == *chosen_id && facts.owner_id == owner_id)
        .map(|(_, facts)| listed_from_read(facts));
    Listing::of(listing.repos.into_iter().chain(still_owned).collect())
}

/// A fresh read as a listing row (it carries no stars, so it ranks last).
fn listed_from_read(facts: &RepoFacts) -> ListedRepo {
    ListedRepo {
        repo_id: facts.repo_id,
        owner_id: facts.owner_id,
        repo: OwnedRepo {
            full_name: facts.full_name.clone(),
            description: None,
            language: None,
            stars: 0,
            fork: facts.fork,
            archived: facts.archived,
            pushed_at: None,
        },
    }
}

/// The skipped repos of a plan, each named as it was chosen (`saved`), for
/// the run's summary. Only names and reasons: no bio, no DID.
pub fn named_skips(plan: &ScanPlan, saved: &[SavedRepoChoice]) -> Vec<SkippedScanRepo> {
    plan.accounts
        .iter()
        .flat_map(|(account_id, account)| {
            let skipped: &[SkippedRepo] = match account {
                AccountPlan::Read { skipped, .. } => skipped,
                AccountPlan::Unverified | AccountPlan::NotReached => &[],
            };
            skipped.iter().map(move |skip| (*account_id, skip))
        })
        .filter_map(|(account_id, skip)| {
            saved
                .iter()
                .find(|row| row.repo_id == skip.repo_id)
                .map(|row| SkippedScanRepo {
                    repo_id: skip.repo_id,
                    full_name: row.full_name.clone(),
                    source_account_id: account_id,
                    reason: skip.reason,
                })
        })
        .collect()
}

/// The plan of one account from the repos `chosen` for it and its check:
/// a passing account reads its chosen repos still readable, in ranking
/// order, and names why each other chosen repo is skipped.
pub fn plan_account(chosen: &[u64], check: &AccountCheck) -> AccountPlan {
    match check {
        AccountCheck::Failed => AccountPlan::Unverified,
        AccountCheck::Unfinished => AccountPlan::NotReached,
        AccountCheck::Passed(listing) => AccountPlan::Read {
            planned: planned_repos(chosen, listing),
            skipped: skipped_repos(chosen, listing),
        },
    }
}

fn planned_repos(chosen: &[u64], listing: &Listing) -> Vec<PlannedRepo> {
    listing
        .ranked_eligible
        .iter()
        .enumerate()
        .filter(|(_, repo_id)| chosen.contains(repo_id))
        .filter_map(|(rank, repo_id)| {
            listing.readable(*repo_id).ok().map(|listed| PlannedRepo {
                repo_id: listed.repo_id,
                full_name: listed.repo.full_name.clone(),
                outside_default: (rank >= DEFAULT_TICKS).then_some(OutsideDefault::OwnedChoice),
            })
        })
        .collect()
}

fn skipped_repos(chosen: &[u64], listing: &Listing) -> Vec<SkippedRepo> {
    chosen
        .iter()
        .filter_map(|repo_id| {
            listing.readable(*repo_id).err().map(|reason| SkippedRepo {
                repo_id: *repo_id,
                reason,
            })
        })
        .collect()
}

impl ScanPlan {
    /// The owned repos the scan reads, each with its account, in plan order.
    pub fn owned_repos(&self) -> Vec<(u64, &PlannedRepo)> {
        self.accounts
            .iter()
            .flat_map(|(account_id, plan)| {
                let planned: &[PlannedRepo] = match plan {
                    AccountPlan::Read { planned, .. } => planned,
                    AccountPlan::Unverified | AccountPlan::NotReached => &[],
                };
                planned.iter().map(move |repo| (*account_id, repo))
            })
            .collect()
    }
}

impl FrozenScope {
    /// The plan from each account's check, in account order: default ticks
    /// follow the scan's own checks (D-RRA-T1) and fill only the room the
    /// saved ticks and imports leave under the limit.
    pub fn plan(&self, checks: &[AccountCheck]) -> ScanPlan {
        let saved: usize = self.accounts.iter().map(|a| a.saved.count()).sum();
        let room = self.cap.saturating_sub(saved + self.imports.len());
        ScanPlan {
            accounts: self
                .accounts
                .iter()
                .zip(checks)
                .enumerate()
                .scan(room, |room, (position, (account, check))| {
                    let chosen = account.chosen(check, room);
                    let plan = plan_account(&chosen, check);
                    let plan = if position == 0 {
                        plan
                    } else {
                        from_other_account(plan)
                    };
                    Some((account.github_user_id, plan))
                })
                .collect(),
            imports: self.imports.clone(),
        }
    }
}

/// An account plan of an account other than the owner's first: each of its
/// repos is one the pre-feature scan (first account only) never read.
fn from_other_account(plan: AccountPlan) -> AccountPlan {
    match plan {
        AccountPlan::Read { planned, skipped } => AccountPlan::Read {
            planned: planned
                .into_iter()
                .map(|repo| PlannedRepo {
                    outside_default: Some(OutsideDefault::OtherAccount),
                    ..repo
                })
                .collect(),
            skipped,
        },
        other => other,
    }
}

/// A planned repo from `source` as its frozen plan row (ADR-088 §6),
/// its outside-default tag kept (U-RRA-D2).
pub fn plan_row(source: PlanSource, repo: &PlannedRepo) -> PlannedScanRepo {
    PlannedScanRepo {
        repo_id: repo.repo_id,
        full_name: repo.full_name.clone(),
        source,
        outside_default: repo.outside_default,
    }
}

impl ScanPlan {
    /// Every row the scan reads, in plan order: each passing account's
    /// repos, then the imports (§6.2 step 4: no bio check), each named as
    /// the owner's import list names it.
    pub fn rows(&self, imported: &[ImportedRepo]) -> Vec<PlannedScanRepo> {
        let owned = self
            .owned_repos()
            .into_iter()
            .map(|(account_id, repo)| plan_row(PlanSource::Account(account_id), repo));
        let imports = self.imports.iter().filter_map(|repo_id| {
            imported
                .iter()
                .find(|import| import.repo_id == *repo_id)
                .map(|import| PlannedScanRepo {
                    repo_id: import.repo_id,
                    full_name: import.full_name.clone(),
                    source: PlanSource::Imported(import.repo_id),
                    outside_default: None,
                })
        });
        owned.chain(imports).collect()
    }
}

/// The `owner/name` an owner typed into "Add repo", when it has that shape.
pub fn import_shape(typed: &str) -> Option<(&str, &str)> {
    let (owner, name) = typed.trim().split_once('/')?;
    let part = |s: &str| {
        !s.is_empty()
            && s.chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
    };
    (part(owner) && part(name)).then_some((owner, name))
}

/// Why a named repo is not imported (DWD-RRA-4: the first that applies).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportRefusal {
    /// Not `owner/name`: decided without asking GitHub.
    Shape,
    /// GitHub says no public repo is there (missing or private).
    NotFound,
    /// GitHub could not answer (outage or rate limit).
    Unreachable,
    /// A listed account (verified or not) owns it (D-RRA-T2).
    OwnedByListedAccount,
    /// It is already on "Repos you don't own" (D-RRA-T3).
    AlreadyImported,
    /// Adding it would pass the limit.
    OverLimit(LimitRefusal),
}

/// What "Add repo" decides (CORE-RRA-9): the import to store, or the one
/// refusal that names its cause.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportVerdict {
    Admitted(ImportedRepo),
    Refused(ImportRefusal),
}

/// Import admission (CORE-RRA-3): one more repo is admitted iff the scan's
/// total (the same `S + I + D` a Save and a scan start count) stays within
/// `cap`. Admitted: the total the next scan reads.
pub fn admit_import(
    accounts: &[ScopeAccount],
    imports: &[u64],
    cap: usize,
) -> Result<usize, LimitRefusal> {
    let total = chosen_total(accounts, imports, cap);
    if total < cap {
        Ok(total + 1)
    } else {
        Err(LimitRefusal {
            limit: cap,
            total,
            excess: total + 1 - cap,
        })
    }
}

/// Classify an "Add repo" request (CORE-RRA-9, DWD-RRA-4) in a fixed order:
/// shape → the repo read → owned by a listed account → already imported →
/// over the limit → admitted.
pub fn classify_import(
    typed: &str,
    read: RepoRead<'_>,
    listed: &[u64],
    accounts: &[ScopeAccount],
    imports: &[u64],
    cap: usize,
) -> ImportVerdict {
    let admitted = import_shape(typed)
        .ok_or(ImportRefusal::Shape)
        .and(found(read))
        .and_then(|facts| not_owned(facts, listed))
        .and_then(|facts| not_yet_imported(facts, imports))
        .and_then(|facts| {
            admit_import(accounts, imports, cap)
                .map(|_| import_of(facts))
                .map_err(ImportRefusal::OverLimit)
        });
    match admitted {
        Ok(import) => ImportVerdict::Admitted(import),
        Err(refusal) => ImportVerdict::Refused(refusal),
    }
}

/// The repo GitHub served, or why it could not be read.
fn found(read: RepoRead<'_>) -> Result<&RepoFacts, ImportRefusal> {
    match read {
        RepoRead::Found(facts) => Ok(facts),
        RepoRead::Gone => Err(ImportRefusal::NotFound),
        RepoRead::Unfinished => Err(ImportRefusal::Unreachable),
    }
}

fn not_owned<'a>(facts: &'a RepoFacts, listed: &[u64]) -> Result<&'a RepoFacts, ImportRefusal> {
    if listed.contains(&facts.owner_id) {
        Err(ImportRefusal::OwnedByListedAccount)
    } else {
        Ok(facts)
    }
}

fn not_yet_imported<'a>(
    facts: &'a RepoFacts,
    imports: &[u64],
) -> Result<&'a RepoFacts, ImportRefusal> {
    if imports.contains(&facts.repo_id) {
        Err(ImportRefusal::AlreadyImported)
    } else {
        Ok(facts)
    }
}

/// The import a public repo read from GitHub becomes, named as GitHub
/// serves it now.
pub fn import_of(facts: &RepoFacts) -> ImportedRepo {
    ImportedRepo {
        repo_id: facts.repo_id,
        full_name: facts.full_name.clone(),
    }
}

/// A frozen plan row a resume reads, as the planned repo it was: its
/// suggestions carry the same outside-default tag (KPI-RRA-1).
pub fn carried_repo(row: &PlannedScanRepo) -> PlannedRepo {
    PlannedRepo {
        repo_id: row.repo_id,
        full_name: row.full_name.clone(),
        outside_default: row.outside_default,
    }
}

/// Where a row of a frozen plan (and each suggestion it yields) comes
/// from: a listed account, or an imported repo.
pub use ports::RepoSource as PlanSource;

/// Visibility (CORE-RRA-7, OQ-5): each suggestion from `sources` is shown
/// iff it comes from a listed account that is verified, or from a repo on
/// the `imports` list. Derived, never stored: hidden is not deleted.
pub fn visible(sources: &[PlanSource], accounts: &[(u64, Standing)], imports: &[u64]) -> Vec<bool> {
    sources
        .iter()
        .map(|source| shown(*source, accounts, imports))
        .collect()
}

fn shown(source: PlanSource, accounts: &[(u64, Standing)], imports: &[u64]) -> bool {
    match source {
        PlanSource::Account(account_id) => accounts
            .iter()
            .any(|(id, standing)| *id == account_id && *standing == Standing::Verified),
        PlanSource::Imported(repo_id) => imports.contains(&repo_id),
    }
}

/// The sources the owner lists right now, as re-read by a scan before each
/// repo (ADR-088 §7): their account ids and their imported repo ids.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ListedSources {
    pub accounts: Vec<u64>,
    pub imports: Vec<u64>,
}

impl ListedSources {
    /// Is `source` still listed? A removed one is neither read nor added to.
    pub fn holds(&self, source: PlanSource) -> bool {
        match source {
            PlanSource::Account(account_id) => self.accounts.contains(&account_id),
            PlanSource::Imported(repo_id) => self.imports.contains(&repo_id),
        }
    }

    /// The sources of `rows` no longer listed: a resume skips their rows
    /// "removed" ([`resume_plan`]).
    pub fn removed_from(&self, rows: &[FrozenRow]) -> BTreeSet<PlanSource> {
        rows.iter()
            .map(|row| row.source)
            .filter(|source| !self.holds(*source))
            .collect()
    }
}

/// One row of a paused run's frozen plan, as a resume finds it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrozenRow {
    pub source: PlanSource,
    pub status: PlanRowStatus,
}

/// A resume's own bio check of one account in the carried rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BioCheck {
    Passed,
    /// The bio read failed or the DID is gone.
    Failed,
    /// Rate limit or outage: not reached.
    Unfinished,
}

/// Why a carried row is not read by a resume.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum CarriedSkip {
    /// Its source was removed meanwhile.
    Removed,
    /// Its account's bio check failed on this resume (final within the scan).
    AccountUnverified,
}

/// What a scan's own bio check of a listed account changes in its stored
/// standing (D-RRA-T1): a pass re-verifies an unverified account, a
/// failure marks it unverified, an unfinished check changes nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StandingChange {
    Reverify,
    MarkUnverified,
    Keep,
}

/// The standing change a bio check makes to an account stored as `stored`.
pub fn standing_after_check(stored: Standing, check: BioCheck) -> StandingChange {
    match (stored, check) {
        (Standing::Unverified, BioCheck::Passed) => StandingChange::Reverify,
        (_, BioCheck::Failed) => StandingChange::MarkUnverified,
        _ => StandingChange::Keep,
    }
}

/// What a resume does with a paused run's frozen plan (positions in it).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Resume {
    pub to_read: Vec<usize>,
    /// Not read now, kept `planned` for a later resume.
    pub carried: Vec<usize>,
    pub skipped: Vec<(usize, CarriedSkip)>,
}

impl Resume {
    /// The rows the resuming run keeps as its own plan, in plan order:
    /// those it reads now and those it carries.
    pub fn kept(&self) -> Vec<usize> {
        let mut kept: Vec<usize> = self.to_read.iter().chain(&self.carried).copied().collect();
        kept.sort_unstable();
        kept
    }
}

/// Does a new scan resume the latest run's frozen plan (BR-RRA-4)? Only a
/// run paused or interrupted with plan rows; one paused before plans were
/// recorded (at deploy) is planned afresh, once.
pub fn resumes_frozen_plan(latest: Option<ScanStatus>, plan_rows: usize) -> bool {
    matches!(
        latest,
        Some(ScanStatus::RateLimited | ScanStatus::Interrupted)
    ) && plan_rows > 0
}

/// What a resume does with one row still `planned`.
enum CarriedRow {
    Read,
    Carried,
    Skipped(CarriedSkip),
}

/// Resume a paused run from its frozen plan (CORE-RRA-5, BR-RRA-4): only
/// its rows still `planned` are considered, never re-planned from the
/// choice in force now (design §6.2 step 8). A row of a removed source is
/// skipped "removed"; an account's rows are read when its bio check passed
/// now, skipped "account unverified" when it failed (final within the
/// scan), and stay `planned` for a later resume when it did not finish (or
/// was not run). Imports need no check.
pub fn resume_plan(
    rows: &[FrozenRow],
    checks: &BTreeMap<u64, BioCheck>,
    removed: &BTreeSet<PlanSource>,
) -> Resume {
    rows.iter()
        .enumerate()
        .filter(|(_, row)| row.status == PlanRowStatus::Planned)
        .fold(Resume::default(), |mut resume, (position, row)| {
            match carried_row(row.source, checks, removed) {
                CarriedRow::Read => resume.to_read.push(position),
                CarriedRow::Skipped(reason) => resume.skipped.push((position, reason)),
                CarriedRow::Carried => resume.carried.push(position),
            }
            resume
        })
}

fn carried_row(
    source: PlanSource,
    checks: &BTreeMap<u64, BioCheck>,
    removed: &BTreeSet<PlanSource>,
) -> CarriedRow {
    let check = match source {
        PlanSource::Account(account_id) => checks
            .get(&account_id)
            .copied()
            .unwrap_or(BioCheck::Unfinished),
        PlanSource::Imported(_) => BioCheck::Passed,
    };
    match check {
        _ if removed.contains(&source) => CarriedRow::Skipped(CarriedSkip::Removed),
        BioCheck::Unfinished => CarriedRow::Carried,
        BioCheck::Failed => CarriedRow::Skipped(CarriedSkip::AccountUnverified),
        BioCheck::Passed => CarriedRow::Read,
    }
}

/// The owner's scope as the 10-repo limit sees it: her listed accounts in
/// `added_at` order and her imported repo ids.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Scope {
    pub accounts: Vec<ScopeAccount>,
    pub imports: Vec<u64>,
}

/// The BR-RRA-5 move (CORE-RRA-10): the import `repo_id`, now owned by the
/// listed account `github_user_id`, leaves the imports and joins that
/// account's choice — its current ticks (saved, or its default ticks when
/// it has none) plus the repo. The total toward `cap` is unchanged.
pub fn moves(scope: &Scope, repo_id: u64, github_user_id: u64, cap: usize) -> Scope {
    let ticked = with_repo(current_ticks(scope, github_user_id, cap), repo_id);
    Scope {
        accounts: scope
            .accounts
            .iter()
            .map(|account| {
                if account.github_user_id == github_user_id {
                    ScopeAccount {
                        saved: SavedTicks::Saved(ticked.clone()),
                        ..account.clone()
                    }
                } else {
                    account.clone()
                }
            })
            .collect(),
        imports: scope
            .imports
            .iter()
            .copied()
            .filter(|import| *import != repo_id)
            .collect(),
    }
}

/// The ticks of the account `github_user_id` now: its saved ticks, else its
/// default ticks within the room the scope leaves under `cap`.
pub fn current_ticks(scope: &Scope, github_user_id: u64, cap: usize) -> Vec<u64> {
    match scope
        .accounts
        .iter()
        .find(|account| account.github_user_id == github_user_id)
        .map(|account| &account.saved)
    {
        Some(SavedTicks::Saved(ticked)) => ticked.clone(),
        Some(SavedTicks::NeverSaved) => default_ticks(&scope.accounts, &scope.imports, cap)
            .into_iter()
            .find(|(id, _)| *id == github_user_id)
            .map(|(_, ticked)| ticked)
            .unwrap_or_default(),
        None => Vec::new(),
    }
}

/// `ticked` with `repo_id` ticked too (a repo is ticked once).
fn with_repo(mut ticked: Vec<u64>, repo_id: u64) -> Vec<u64> {
    if !ticked.contains(&repo_id) {
        ticked.push(repo_id);
    }
    ticked
}

/// The imports among `imports` that a listing of one account shows: that
/// account owns them now (BR-RRA-5).
pub fn owned_imports(listing: &[ListedRepo], imports: &[u64]) -> Vec<u64> {
    imports
        .iter()
        .copied()
        .filter(|import| listing.iter().any(|listed| listed.repo_id == *import))
        .collect()
}

/// One BR-RRA-5 move a scan found: the import `repo_id` joined the choice
/// of the account `github_user_id`, whose ticks are now `ticked`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScopeMove {
    pub repo_id: u64,
    pub github_user_id: u64,
    pub ticked: Vec<u64>,
}

impl FrozenScope {
    /// The moves this scan's listings show (BR-RRA-5) and the scope after
    /// them: an import a passing account lists now moves to the first such
    /// account, so the plan reads it once, as that account's repo, and the
    /// total stays what it was.
    pub fn after_moves(&self, checks: &[AccountCheck]) -> (Self, Vec<ScopeMove>) {
        let before = Scope {
            accounts: self
                .accounts
                .iter()
                .zip(checks)
                .map(|(account, check)| account.as_scoped(check))
                .collect(),
            imports: self.imports.clone(),
        };
        let (after, found) = self.accounts.iter().zip(checks).fold(
            (before, Vec::new()),
            |(scope, mut found), (account, check)| {
                let AccountCheck::Passed(listing) = check else {
                    return (scope, found);
                };
                let id = account.github_user_id;
                let scope = owned_imports(&listing.repos, &scope.imports)
                    .into_iter()
                    .fold(scope, |scope, repo_id| {
                        let moved = moves(&scope, repo_id, id, self.cap);
                        found.push(ScopeMove {
                            repo_id,
                            github_user_id: id,
                            ticked: current_ticks(&moved, id, self.cap),
                        });
                        moved
                    });
                (scope, found)
            },
        );
        let frozen = Self {
            accounts: self
                .accounts
                .iter()
                .zip(&after.accounts)
                .map(|(account, scoped)| ScanAccount {
                    saved: scoped.saved.clone(),
                    ..account.clone()
                })
                .collect(),
            imports: after.imports,
            cap: self.cap,
        };
        (frozen, found)
    }
}

impl ScanAccount {
    /// This account as the limit sees it once the scan checked it: only a
    /// passing account has default ticks, from its listing now.
    fn as_scoped(&self, check: &AccountCheck) -> ScopeAccount {
        let (standing, ranked_eligible) = match check {
            AccountCheck::Passed(listing) => (Standing::Verified, listing.ranked_eligible.clone()),
            AccountCheck::Failed | AccountCheck::Unfinished => (Standing::Unverified, Vec::new()),
        };
        ScopeAccount {
            github_user_id: self.github_user_id,
            standing,
            saved: self.saved.clone(),
            ranked_eligible,
        }
    }
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
        /// U-RRA-D2: a planned repo frozen as a plan row and read back by a
        /// resume is the same repo with the same outside-default tag.
        #[test]
        fn a_resumed_row_is_the_planned_repo_with_its_tag(
            repo_id in 1u64..10_000,
            account_id in 1u64..10_000,
            tag in proptest::option::of(proptest::sample::select(OutsideDefault::ALL.to_vec())),
        ) {
            let planned = PlannedRepo {
                repo_id,
                full_name: format!("me/repo-{repo_id}"),
                outside_default: tag,
            };
            let row = plan_row(PlanSource::Account(account_id), &planned);
            prop_assert_eq!(row.source, PlanSource::Account(account_id));
            prop_assert_eq!(carried_repo(&row), planned);
        }

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

    fn facts(id: u64, owner_id: u64) -> RepoFacts {
        RepoFacts {
            repo_id: id,
            full_name: format!("me/repo-{id}"),
            owner_id,
            owner_login: "me".to_string(),
            owner_kind: ports::RepoOwnerKind::User,
            fork: false,
            archived: false,
        }
    }

    // bypass: I-06 as three named cases (complete, truncated + still owned,
    // truncated + gone); the AT covers only complete listings.
    #[test]
    fn a_truncated_listing_never_proves_a_chosen_repo_missing() {
        let saved = [
            SavedRepoChoice {
                repo_id: 1,
                full_name: "me/repo-1".to_string(),
                ticked: true,
            },
            SavedRepoChoice {
                repo_id: 9,
                full_name: "me/repo-9".to_string(),
                ticked: true,
            },
        ];
        let listing = |extent| AccountRepoListing {
            repos: vec![listed(1, 5, false, false)],
            extent,
        };
        assert!(unlisted_ticks(&saved, &listing(ListingExtent::Complete)).is_empty());
        let unread: Vec<u64> = unlisted_ticks(&saved, &listing(ListingExtent::Truncated))
            .iter()
            .map(|row| row.repo_id)
            .collect();
        assert_eq!(unread, [9]);

        let read_back = |reads: &[(u64, RepoFacts)]| {
            let listing = listing_with_reads(listing(ListingExtent::Truncated), 1, reads);
            plan_account(&[1, 9], &AccountCheck::Passed(listing))
        };
        let AccountPlan::Read { planned, skipped } = read_back(&[(9, facts(9, 1))]) else {
            panic!("passed");
        };
        assert_eq!(
            planned.iter().map(|p| p.repo_id).collect::<Vec<_>>(),
            [1, 9]
        );
        assert!(skipped.is_empty());
        for gone in [vec![], vec![(9, facts(9, 2))], vec![(9, facts(10, 1))]] {
            let AccountPlan::Read { skipped, .. } = read_back(&gone) else {
                panic!("passed");
            };
            let reasons: Vec<_> = skipped.iter().map(|s| (s.repo_id, s.reason)).collect();
            assert_eq!(reasons, [(9, SkipReason::MissingOrTransferred)], "{gone:?}");
        }

        let plan = ScanPlan {
            accounts: vec![(1, read_back(&[]))],
            imports: Vec::new(),
        };
        assert_eq!(
            named_skips(&plan, &saved),
            [SkippedScanRepo {
                repo_id: 9,
                full_name: "me/repo-9".to_string(),
                source_account_id: 1,
                reason: SkipReason::MissingOrTransferred,
            }],
            "a skip is named as chosen, and carries nothing else"
        );
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

    fn plan_source() -> impl Strategy<Value = PlanSource> {
        prop_oneof![
            (1u64..5).prop_map(PlanSource::Account),
            (1u64..5).prop_map(PlanSource::Imported),
        ]
    }

    proptest! {
        /// Universe: frozen rows from any sources, and any listed accounts
        /// and imports. A resume skips "removed" exactly the planned rows
        /// whose source is no longer listed, whatever their bio check.
        #[test]
        fn a_resume_skips_as_removed_exactly_the_rows_of_unlisted_sources(
            sources in prop::collection::vec(plan_source(), 0..8),
            accounts in prop::collection::vec(1u64..5, 0..4),
            imports in prop::collection::vec(1u64..5, 0..4),
        ) {
            let listed = ListedSources { accounts, imports };
            let rows: Vec<FrozenRow> = sources
                .iter()
                .map(|source| FrozenRow { source: *source, status: PlanRowStatus::Planned })
                .collect();
            let checks: BTreeMap<u64, BioCheck> =
                (1u64..5).map(|id| (id, BioCheck::Passed)).collect();
            let resume = resume_plan(&rows, &checks, &listed.removed_from(&rows));
            let removed: Vec<usize> = resume
                .skipped
                .iter()
                .filter(|(_, why)| *why == CarriedSkip::Removed)
                .map(|(position, _)| *position)
                .collect();
            let unlisted: Vec<usize> = (0..sources.len())
                .filter(|position| !listed.holds(sources[*position]))
                .collect();
            prop_assert_eq!(removed, unlisted);
            prop_assert!(resume.to_read.iter().all(|position| listed.holds(sources[*position])));
        }
    }
}
