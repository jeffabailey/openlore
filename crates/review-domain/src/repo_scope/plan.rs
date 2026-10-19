//! A scan's start and its frozen plan (CORE-RRA-4, BR-RRA-4; ADR-088 §6):
//! the scope admitted before any read, each account's listing as the
//! scan's own check found it, and the rows the scan reads. PURE.

use ports::{
    AccountRepoListing, ImportedRepo, ListedRepo, ListingExtent, OutsideDefault, OwnedRepo,
    PlannedScanRepo, RepoFacts, SavedRepoChoice, SkippedScanRepo,
};

use super::choose::{outside_default_at, ranked, PlannedRepo};
use super::limits::{
    room_left, saved_total, take_default_ticks, within_cap, LimitRefusal, SavedTicks,
};
use super::PlanSource;

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
    pub(super) accounts: Vec<ScanAccount>,
    pub(super) imports: Vec<u64>,
    pub(super) cap: usize,
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

impl AccountPlan {
    /// The repos this account's scan reads (none unless it reads).
    fn planned(&self) -> &[PlannedRepo] {
        match self {
            Self::Read { planned, .. } => planned,
            Self::Unverified | Self::NotReached => &[],
        }
    }

    /// The chosen repos this account's scan skips (none unless it reads).
    fn skipped(&self) -> &[SkippedRepo] {
        match self {
            Self::Read { skipped, .. } => skipped,
            Self::Unverified | Self::NotReached => &[],
        }
    }
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
                take_default_ticks(&listing.ranked_eligible, room)
            }
            (SavedTicks::NeverSaved, _) => Vec::new(),
        }
    }
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
            account
                .skipped()
                .iter()
                .map(move |skip| (*account_id, skip))
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
                outside_default: outside_default_at(rank),
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
                plan.planned().iter().map(move |repo| (*account_id, repo))
            })
            .collect()
    }

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

impl FrozenScope {
    /// The plan from each account's check, in account order: default ticks
    /// follow the scan's own checks (D-RRA-T1) and fill only the room the
    /// saved ticks and imports leave under the limit.
    pub fn plan(&self, checks: &[AccountCheck]) -> ScanPlan {
        let saved = saved_total(self.accounts.iter().map(|account| &account.saved));
        ScanPlan {
            accounts: self
                .accounts
                .iter()
                .zip(checks)
                .enumerate()
                .scan(
                    room_left(saved, &self.imports, self.cap),
                    |room, (position, (account, check))| {
                        let plan = plan_account(&account.chosen(check, room), check);
                        let plan = if position == 0 {
                            plan
                        } else {
                            from_other_account(plan)
                        };
                        Some((account.github_user_id, plan))
                    },
                )
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

/// A frozen plan row a resume reads, as the planned repo it was: its
/// suggestions carry the same outside-default tag (KPI-RRA-1).
pub fn carried_repo(row: &PlannedScanRepo) -> PlannedRepo {
    PlannedRepo {
        repo_id: row.repo_id,
        full_name: row.full_name.clone(),
        outside_default: row.outside_default,
    }
}
