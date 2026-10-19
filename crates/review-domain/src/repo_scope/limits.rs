//! The limits (BR-RRA-3, D-RRA-15, D-RRA-16): how many accounts one DID
//! lists, and how many repos one scan reads. Every admission counts the one
//! total `S + I + D` — the saved ticks, the imports and the default ticks
//! within the room they leave — through [`saved_total`] and
//! [`default_ticks`]. PURE.

use ports::{AccountAdmission, AccountAdmitted, AccountRefusal, ListedRepo, SavedRepoChoice};

use super::choose::{ranked, ticked_ids};
use super::DEFAULT_TICKS;

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

impl Standing {
    /// The standing a stored account's `verified` flag records.
    pub fn of(verified: bool) -> Self {
        if verified {
            Self::Verified
        } else {
            Self::Unverified
        }
    }
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
    let saved = saved_total(accounts.iter().map(|account| &account.saved));
    accounts
        .iter()
        .filter(|account| account.takes_default_ticks())
        .scan(room_left(saved, imports, cap), |room, account| {
            Some((
                account.github_user_id,
                take_default_ticks(&account.ranked_eligible, room),
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

/// Import admission (CORE-RRA-3): one more repo is admitted iff the scan's
/// total (the same `S + I + D` a Save and a scan start count) with it added
/// stays within `cap`. Admitted: the total the next scan reads; refused:
/// that would-be total, as a Save's refusal names it (R6).
pub fn admit_import(
    accounts: &[ScopeAccount],
    imports: &[u64],
    cap: usize,
) -> Result<usize, LimitRefusal> {
    within_cap(chosen_total(accounts, imports, cap) + 1, cap)
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

/// `S`, the one definition (OQ-D3): the ticks of every saved choice, the
/// account verified or not. Save, import and scan start all count it.
pub(super) fn saved_total<'a>(saved: impl IntoIterator<Item = &'a SavedTicks>) -> usize {
    saved.into_iter().map(SavedTicks::count).sum()
}

/// The room default ticks may fill: what `saved` ticks and the `imports`
/// leave under `cap`.
pub(super) fn room_left(saved: usize, imports: &[u64], cap: usize) -> usize {
    cap.saturating_sub(saved + imports.len())
}

/// The default ticks of an account ranked `ranked_eligible`: its first
/// `min(5, eligible, room)`, which use up that much of `room`.
pub(super) fn take_default_ticks(ranked_eligible: &[u64], room: &mut usize) -> Vec<u64> {
    let ticked = DEFAULT_TICKS.min(ranked_eligible.len()).min(*room);
    *room -= ticked;
    ranked_eligible[..ticked].to_vec()
}

/// The total itself when it fits under `cap`, else the refusal naming it.
pub(super) fn within_cap(total: usize, cap: usize) -> Result<usize, LimitRefusal> {
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
            Self::Saved(ticked_ids(rows))
        }
    }

    /// How many repos these ticks count toward the limit.
    pub(super) fn count(&self) -> usize {
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
