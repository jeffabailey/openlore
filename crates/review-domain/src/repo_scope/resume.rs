//! Resuming a paused scan from its frozen plan (CORE-RRA-5, BR-RRA-4), and
//! what a scan's own bio checks change (D-RRA-T1). PURE.

use std::collections::{BTreeMap, BTreeSet};

use ports::{PlanRowStatus, ScanStatus};

use super::limits::Standing;
use super::PlanSource;

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
    /// Read by an earlier run of this scan and taken over `read`, so the
    /// whole scan is accounted for and the repo is not read again.
    pub already_read: Vec<usize>,
    pub skipped: Vec<(usize, CarriedSkip)>,
}

/// One row of the resumed plan the resuming run keeps as its own, with the
/// status it carries: `read` for a row an earlier run of the scan already
/// read, `planned` for one this run reads now or carries on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeptRow {
    pub position: usize,
    pub status: PlanRowStatus,
}

impl Resume {
    /// The positions this run reads now.
    ///
    /// Read the verdict through these accessors, never through the fields:
    /// when the verdict becomes one disposition per position, each of them
    /// is derived over that one sequence and the fields go. Each returns an
    /// owned answer, the shape a derived view can keep.
    pub fn to_read(&self) -> Vec<usize> {
        self.to_read.clone()
    }

    /// The positions kept `planned` for a later resume.
    pub fn carried(&self) -> Vec<usize> {
        self.carried.clone()
    }

    /// The positions an earlier run of this scan read, taken over `read`.
    pub fn already_read(&self) -> Vec<usize> {
        self.already_read.clone()
    }

    /// The positions this scan never reads, each with its reason.
    pub fn skipped(&self) -> Vec<(usize, CarriedSkip)> {
        self.skipped.clone()
    }

    /// The rows the resuming run keeps as its own plan, in plan order: the
    /// rows the scan already read, those it reads now and those it carries,
    /// each with the status the resuming run gives it.
    pub fn kept(&self) -> Vec<KeptRow> {
        let read = self.already_read.iter().map(|position| KeptRow {
            position: *position,
            status: PlanRowStatus::Read,
        });
        let planned = self
            .to_read
            .iter()
            .chain(&self.carried)
            .map(|position| KeptRow {
                position: *position,
                status: PlanRowStatus::Planned,
            });
        let mut kept: Vec<KeptRow> = read.chain(planned).collect();
        kept.sort_unstable_by_key(|row| row.position);
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

/// Resume a paused run from its frozen plan (CORE-RRA-5, BR-RRA-4): the
/// plan is never re-planned from the choice in force now (design §6.2
/// step 8). A row this scan already read is taken over `read`, whatever the
/// bio checks and the removed sources say now: the scan did read it, and it
/// is not read again. Of the rows still `planned`, one of a removed source
/// is skipped "removed"; an account's rows are read when its bio check
/// passed now, skipped "account unverified" when it failed (final within
/// the scan), and stay `planned` for a later resume when it did not finish
/// (or was not run). Imports need no check. A row the paused run skipped is
/// no row of this run's plan.
pub fn resume_plan(
    rows: &[FrozenRow],
    checks: &BTreeMap<u64, BioCheck>,
    removed: &BTreeSet<PlanSource>,
) -> Resume {
    rows.iter()
        .enumerate()
        .fold(Resume::default(), |mut resume, (position, row)| {
            match row.status {
                PlanRowStatus::Read => resume.already_read.push(position),
                PlanRowStatus::Skipped => {}
                PlanRowStatus::Planned => match carried_row(row.source, checks, removed) {
                    CarriedRow::Read => resume.to_read.push(position),
                    CarriedRow::Skipped(reason) => resume.skipped.push((position, reason)),
                    CarriedRow::Carried => resume.carried.push(position),
                },
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
