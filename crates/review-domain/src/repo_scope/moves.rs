//! The BR-RRA-5 move (CORE-RRA-10): an import a listed account owns now
//! leaves the imports and joins that account's choice, the total toward
//! the limit unchanged. PURE.

use ports::ListedRepo;

use super::limits::{default_ticks, SavedTicks, ScopeAccount, Standing};
use super::plan::{AccountCheck, FrozenScope, ScanAccount};

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
