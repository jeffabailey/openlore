//! The scan task (component-boundaries: Scan task row; ADR-076 §3). It runs
//! in the background under ONE admission: every listed account, in order,
//! has its bio checked again (D-RRA-T1) before its listing is read —
//! a passing check mints the [`VerifiedOwnership`] its listing needs
//! (I-BRA-4), a failing one marks the account unverified, an unfinished one
//! changes nothing — then each passing account's saved choice (or today's
//! top 5) is frozen as the run's plan (`repo_scope`, ADR-088 §6) → per
//! planned repo: harvest →
//! `derive_candidates` →
//! `reconcile` → persist. Each repo's suggestions are stored as soon as that
//! repo completes, so a paused scan keeps its partial results, and a resumed
//! one offers only keys never seen before. The run records what it found
//! (DWD-11 summary counts, of its own derived keys only). Nothing is written
//! to any PDS; suggestion content is never logged.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use std::time::Duration;

use ports::{
    AccountCheckOutcome, AccountCheckRecord, AccountRepoListing, GithubError, GithubLink,
    ImportedRepo, PlanRowStatus, PlannedScanRepo, RepoFacts, RepoSkipReason, ReviewStoreError,
    SavedRepoChoice, ScanCounts, ScanPlanRow, ScanStatus, SkippedScanRepo, SuggestionSource,
};
use review_domain::budget::{auto_resume_wait, github_budget_allows, ScanAdmission};
use review_domain::kpi::{utc_day, KpiEvent};
use review_domain::lifecycle::suggestion_from_candidate;
use review_domain::ownership::{prove_ownership, OwnershipRefusal, VerifiedOwnership};
use review_domain::reconcile::{add_counts, reconcile};
use review_domain::repo_scope::{
    carried_repo, eligible_count, listing_with_reads, named_skips, resume_plan,
    resumes_frozen_plan, scan_intent, standing_after_check, unlisted_ticks, AccountCheck, BioCheck,
    CarriedSkip, FrozenRow, FrozenScope, LimitRefusal, ListedSources, Listing, PlanSource,
    PlannedRepo, SavedTicks, ScanAccount, ScanIntent, Standing, StandingChange, SCAN_REPO_CAP,
};
use review_domain::views::ScanRefused;
use scraper_domain::derive_candidates;

use crate::http::App;
use crate::limiter::unix_now_secs;
use crate::routes::github::{account_of, refusal_of};
use crate::routes::signin::random_token;
use crate::wiring::{emit, observe, Counted, LogEvent};

/// How long a paused scan waits when GitHub named no reset time.
const DEFAULT_PAUSE_SECS: i64 = 15 * 60;

/// How a scan ended, with when a paused one may resume and what it found
/// before it ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Ended {
    status: ScanStatus,
    resume_after: Option<i64>,
    counts: ScanCounts,
}

impl Ended {
    fn with_status(status: ScanStatus) -> Self {
        Self {
            status,
            resume_after: None,
            counts: ScanCounts::default(),
        }
    }

    /// This ending, having found `counts` first.
    fn after_finding(self, counts: ScanCounts) -> Self {
        Self { counts, ..self }
    }
}

/// What asking to start a scan did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ScanStart {
    Started,
    /// No verified GitHub link and no imported repo: verify first.
    NotVerified,
    AlreadyScanning,
    /// The budget said no (logged as `scan.refused`).
    Refused(ScanRefused),
    /// The run could not be recorded (logged as `scan.refused` busy).
    Unavailable,
    /// Refused before admission: no repo chosen and none named.
    NothingToRead,
    /// Refused before admission: more repos chosen than one scan reads.
    OverLimit(LimitRefusal),
}

/// Start a scan for `owner_did` if one of their accounts is verified or
/// they imported a repo (D-RRA-8: imports need no account), and the budget
/// admits it; it then runs in the background. The one way any scan starts,
/// whether the owner pressed Scan or a paused scan resumes on its own.
pub(crate) fn begin_scan(app: &Arc<App>, owner_did: &str) -> ScanStart {
    let accounts = app.links.github_accounts(owner_did).unwrap_or_default();
    let Ok(imports) = app.imports_read.imports(owner_did) else {
        return ScanStart::Unavailable;
    };
    if imports.is_empty() && !accounts.iter().any(|account| account.verified) {
        return ScanStart::NotVerified;
    }
    let basis = match scan_basis(app, owner_did, &accounts, imports) {
        Ok(basis) => basis,
        Err(refused) => return refused,
    };
    let refused = match app.scan_limiter.admit(owner_did) {
        ScanAdmission::Admitted => None,
        ScanAdmission::AlreadyScanning => return ScanStart::AlreadyScanning,
        ScanAdmission::DailyLimitReached => Some(ScanRefused::DailyLimitReached),
        ScanAdmission::AppBusy => Some(ScanRefused::AppBusy),
    };
    if let Some(refused) = refused {
        emit(LogEvent::ScanRefused(refused));
        return ScanStart::Refused(refused);
    }
    let run_id = random_token();
    if app.scans.start_scan(owner_did, &run_id).is_err() {
        app.scan_limiter.release(owner_did);
        emit(LogEvent::ScanRefused(ScanRefused::AppBusy));
        return ScanStart::Unavailable;
    }
    tokio::spawn(run_scan(
        app.clone(),
        owner_did.to_string(),
        run_id,
        accounts,
        basis,
    ));
    ScanStart::Started
}

/// What a scan reads: a plan made now from the choice in force, or the
/// frozen plan of the paused run it resumes (BR-RRA-4), with what that run
/// skipped (still named in the summary).
pub(crate) enum ScanBasis {
    Fresh(FrozenChoice),
    Resume {
        plan: Vec<ScanPlanRow>,
        skipped: Vec<SkippedScanRepo>,
    },
}

/// Resume the latest run's frozen plan when it paused with one; otherwise
/// freeze the choice in force now. A resume ignores choices saved since.
fn scan_basis(
    app: &App,
    owner_did: &str,
    accounts: &[GithubLink],
    imports: Vec<ImportedRepo>,
) -> Result<ScanBasis, ScanStart> {
    let unavailable = |_| ScanStart::Unavailable;
    let latest = app.scans.latest_scan(owner_did).map_err(unavailable)?;
    let plan = app.scan_plans.latest_plan(owner_did).map_err(unavailable)?;
    if resumes_frozen_plan(latest.map(|run| run.status), plan.len()) {
        let skipped = app
            .scan_plans
            .latest_skipped(owner_did)
            .map_err(unavailable)?;
        return Ok(ScanBasis::Resume { plan, skipped });
    }
    frozen_intent(app, owner_did, accounts, imports).map(ScanBasis::Fresh)
}

/// A scan's scope frozen at its start, with each account's saved choice in
/// account order (the names its skipped repos are reported by) and the
/// imports (the names their plan rows are read by).
pub(crate) struct FrozenChoice {
    scope: FrozenScope,
    saved: Vec<Vec<SavedRepoChoice>>,
    imports: Vec<ImportedRepo>,
}

/// The scan's intent over every listed account, frozen from the store
/// before admission (§6.2 step 1): a refusal here spends none of the day's
/// scans and asks GitHub nothing.
fn frozen_intent(
    app: &App,
    owner_did: &str,
    accounts: &[GithubLink],
    imports: Vec<ImportedRepo>,
) -> Result<FrozenChoice, ScanStart> {
    let saved: Vec<Vec<SavedRepoChoice>> = accounts
        .iter()
        .map(|account| {
            app.choices_read
                .saved_choice(owner_did, account.github_user_id)
                .map_err(|_| ScanStart::Unavailable)
        })
        .collect::<Result<_, _>>()?;
    let scan_accounts: Vec<ScanAccount> = accounts
        .iter()
        .zip(&saved)
        .map(|(account, saved)| ScanAccount {
            github_user_id: account.github_user_id,
            saved: SavedTicks::of(saved),
            eligible_seen: None,
        })
        .collect();
    let import_ids: Vec<u64> = imports.iter().map(|import| import.repo_id).collect();
    match scan_intent(&scan_accounts, &import_ids, SCAN_REPO_CAP) {
        ScanIntent::Read(scope) => Ok(FrozenChoice {
            scope,
            saved,
            imports,
        }),
        ScanIntent::NothingToRead => Err(refused_before_admission(
            app,
            KpiEvent::ScanRefusedNothingToRead,
            ScanStart::NothingToRead,
        )),
        ScanIntent::OverLimit(refusal) => Err(refused_before_admission(
            app,
            KpiEvent::ScanRefusedOverLimit,
            ScanStart::OverLimit(refusal),
        )),
    }
}

/// Count a refusal made before admission and return it.
fn refused_before_admission(app: &App, event: KpiEvent, refused: ScanStart) -> ScanStart {
    let _ = app.kpi.count(&utc_day(unix_now_secs()), event.name());
    refused
}

/// Resume `owner_did`'s unfinished scan on its own when
/// [`auto_resume_wait`] says to, if it is still the latest run then (the
/// owner may have resumed it already). A resume goes through the budget
/// like any scan, so it counts toward the day's scans.
pub(crate) fn resume_on_its_own(app: Arc<App>, owner_did: String, interrupted_by_restart: bool) {
    let Some(run) = app.scans.latest_scan(&owner_did).ok().flatten() else {
        return;
    };
    let Some(wait) = auto_resume_wait(
        run.status,
        run.resume_after,
        unix_now_secs(),
        interrupted_by_restart,
    ) else {
        return;
    };
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_secs(wait)).await;
        let still_unfinished = app
            .scans
            .latest_scan(&owner_did)
            .ok()
            .flatten()
            .is_some_and(|latest| latest == run);
        if still_unfinished {
            let _ = begin_scan(&app, &owner_did);
        }
    });
}

/// Run one admitted scan for `owner_did` to its end, then free its slot and
/// record how it ended; a scan paused for GitHub's budget resumes on its own.
pub(crate) async fn run_scan(
    app: Arc<App>,
    owner_did: String,
    run_id: String,
    accounts: Vec<GithubLink>,
    basis: ScanBasis,
) {
    let ended = match &basis {
        ScanBasis::Fresh(frozen) => {
            scan_accounts(&app, &owner_did, &run_id, &accounts, frozen).await
        }
        ScanBasis::Resume { plan, skipped } => {
            resume_scan(&app, &owner_did, &run_id, &accounts, plan, skipped).await
        }
    };
    app.scan_limiter.release(&owner_did);
    let counted = Counted::of(&LogEvent::ScanFinished(ended.status));
    emit(LogEvent::ScanFinished(ended.status));
    let _ = app.scans.finish_scan(
        &owner_did,
        &run_id,
        ended.status,
        ended.resume_after,
        ended.counts,
        counted.tally(),
    );
    if ended.status == ScanStatus::RateLimited {
        resume_on_its_own(app, owner_did, false);
    }
}

/// Why a bio check (or the listing after it) could not finish.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Halt {
    RateLimited,
    Unavailable,
}

/// One listed account's bio check, as this scan ran it.
enum Checked {
    /// Proven now: the proof its listing is read with.
    Passed(VerifiedOwnership),
    /// Disproven: marked unverified, none of its repos is read.
    Failed(OwnershipRefusal),
    /// Not reached: its status is kept, none of its repos is read.
    Unfinished(Halt),
}

impl Checked {
    fn outcome(&self) -> AccountCheckOutcome {
        match self {
            Self::Passed(_) => AccountCheckOutcome::Passed,
            Self::Failed(_) => AccountCheckOutcome::Failed,
            Self::Unfinished(_) => AccountCheckOutcome::NotReached,
        }
    }

    fn bio_check(&self) -> BioCheck {
        match self {
            Self::Passed(_) => BioCheck::Passed,
            Self::Failed(_) => BioCheck::Failed,
            Self::Unfinished(_) => BioCheck::Unfinished,
        }
    }
}

/// Read the account's profile now by its numeric id (a renamed account
/// answers under its new login) and prove ownership again, pinned to that
/// id. The check updates the stored standing through the narrow status
/// port (D-RRA-T1): a pass re-verifies an unverified account, a disproof
/// marks it unverified, a check that could not finish changes nothing.
async fn check_account(app: &App, owner_did: &str, account: &GithubLink) -> Checked {
    let proof = match app.github.read_person_by_id(account.github_user_id).await {
        Ok(profile) => prove_ownership(
            owner_did,
            &account_of(&profile),
            Some(account.github_user_id),
        ),
        Err(error) => Err(refusal_of(&error)),
    };
    let checked = match proof {
        Ok(ownership) => Checked::Passed(ownership),
        Err(refusal) if refusal.disproves_ownership() => Checked::Failed(refusal),
        Err(OwnershipRefusal::RateLimited) => Checked::Unfinished(Halt::RateLimited),
        Err(_) => Checked::Unfinished(Halt::Unavailable),
    };
    record_standing(app, owner_did, account, &checked);
    checked
}

/// Store what the check changed in the account's standing.
fn record_standing(app: &App, owner_did: &str, account: &GithubLink, checked: &Checked) {
    let stored = if account.verified {
        Standing::Verified
    } else {
        Standing::Unverified
    };
    let id = account.github_user_id;
    let _ = match (standing_after_check(stored, checked.bio_check()), checked) {
        (StandingChange::Reverify, Checked::Passed(ownership)) => app
            .account_status
            .mark_account_verified(owner_did, id, ownership.github_login()),
        (StandingChange::MarkUnverified, Checked::Failed(refusal)) => app
            .account_status
            .mark_account_unverified(owner_did, id, refusal.label()),
        _ => Ok(()),
    };
}

/// Why a GitHub failure stopped a check or a listing.
fn halt_of(error: &GithubError) -> Halt {
    match error {
        GithubError::RateLimited { .. } => Halt::RateLimited,
        _ => Halt::Unavailable,
    }
}

/// Record how each account's check ended in this run and count the failed
/// and unfinished ones (ids and outcomes only).
fn record_checks(app: &App, owner_did: &str, run_id: &str, checked: &[(u64, Checked)]) {
    let records: Vec<AccountCheckRecord> = checked
        .iter()
        .map(|(github_user_id, check)| AccountCheckRecord {
            github_user_id: *github_user_id,
            outcome: check.outcome(),
        })
        .collect();
    let _ = app.scans.record_account_checks(owner_did, run_id, &records);
    for record in &records {
        observe(app, LogEvent::ScanAccountChecked(record.outcome));
    }
}

/// How a scan that reads nothing ends, from its accounts' checks: every
/// account failed → ownership failed; one stopped by GitHub's budget →
/// paused; one stopped by an outage → interrupted; else completed.
fn ended_reading_nothing(app: &App, checked: &[(u64, Checked)]) -> Ended {
    let halted = |halt: Halt| {
        checked
            .iter()
            .any(|(_, check)| matches!(check, Checked::Unfinished(h) if *h == halt))
    };
    let every_account_failed = !checked.is_empty()
        && checked
            .iter()
            .all(|(_, check)| matches!(check, Checked::Failed(_)));
    if every_account_failed {
        Ended::with_status(ScanStatus::OwnershipFailed)
    } else if halted(Halt::RateLimited) {
        paused(app)
    } else if halted(Halt::Unavailable) {
        Ended::with_status(ScanStatus::Interrupted)
    } else {
        Ended::with_status(ScanStatus::Completed)
    }
}

/// One account's check and, when it passed, its listing now.
async fn check_and_list(
    app: &App,
    owner_did: &str,
    account: &GithubLink,
    saved: &[SavedRepoChoice],
) -> (Checked, AccountCheck) {
    match check_account(app, owner_did, account).await {
        Checked::Passed(ownership) => match account_listing(app, &ownership, saved).await {
            Ok(listing) => (Checked::Passed(ownership), AccountCheck::Passed(listing)),
            Err(error) => (
                Checked::Unfinished(halt_of(&error)),
                AccountCheck::Unfinished,
            ),
        },
        failed @ Checked::Failed(_) => (failed, AccountCheck::Failed),
        Checked::Unfinished(halt) => (Checked::Unfinished(halt), AccountCheck::Unfinished),
    }
}

/// A fresh scan: each account in order is checked, then (when it passed)
/// listed, before the next; the choice frozen at the scan's start is
/// planned against the listings, the imports are appended unchecked, the
/// plan is written before the first read (BR-RRA-4), and exactly the
/// planned repos are read.
async fn scan_accounts(
    app: &App,
    owner_did: &str,
    run_id: &str,
    accounts: &[GithubLink],
    frozen: &FrozenChoice,
) -> Ended {
    let mut checked = Vec::with_capacity(accounts.len());
    let mut account_checks = Vec::with_capacity(accounts.len());
    for (account, saved) in accounts.iter().zip(&frozen.saved) {
        let (check, account_check) = check_and_list(app, owner_did, account, saved).await;
        checked.push((account.github_user_id, check));
        account_checks.push(account_check);
    }
    record_checks(app, owner_did, run_id, &checked);
    let scan_plan = frozen.scope.plan(&account_checks);
    let plan = scan_plan.rows(&frozen.imports);
    let every_saved = frozen.saved.concat();
    let recorded = app
        .scans
        .record_plan(owner_did, run_id, &plan)
        .and_then(|()| {
            app.scans
                .record_skipped(owner_did, run_id, &named_skips(&scan_plan, &every_saved))
        });
    if recorded.is_err() {
        return Ended::with_status(ScanStatus::Interrupted);
    }
    if plan.is_empty() {
        return ended_reading_nothing(app, &checked);
    }
    let to_read: Vec<(usize, PlanSource, PlannedRepo)> = plan
        .iter()
        .enumerate()
        .map(|(position, row)| (position, row.source, carried_repo(row)))
        .collect();
    read_planned(app, owner_did, run_id, &to_read).await
}

/// The resume of a paused run: the bios of the accounts in its unread rows
/// are checked again first, then exactly its frozen plan's unread repos of
/// the passing accounts are read, whatever was saved since (BR-RRA-4).
async fn resume_scan(
    app: &App,
    owner_did: &str,
    run_id: &str,
    accounts: &[GithubLink],
    plan: &[ScanPlanRow],
    skipped: &[SkippedScanRepo],
) -> Ended {
    let carried: BTreeSet<u64> = plan
        .iter()
        .filter(|row| row.status == PlanRowStatus::Planned)
        .filter_map(|row| match row.repo.source {
            PlanSource::Account(account_id) => Some(account_id),
            PlanSource::Imported(_) => None,
        })
        .collect();
    let mut checked = Vec::new();
    for account in accounts
        .iter()
        .filter(|account| carried.contains(&account.github_user_id))
    {
        let check = check_account(app, owner_did, account).await;
        checked.push((account.github_user_id, check));
    }
    record_checks(app, owner_did, run_id, &checked);
    let bio_checks: BTreeMap<u64, BioCheck> = checked
        .iter()
        .map(|(id, check)| (*id, check.bio_check()))
        .collect();
    let Ok(listed) = listed_sources(app, owner_did) else {
        return Ended::with_status(ScanStatus::Interrupted);
    };
    let resumed = ResumedPlan::of(plan, skipped, &bio_checks, &listed);
    if resumed.record(app, owner_did, run_id).is_err() {
        return Ended::with_status(ScanStatus::Interrupted);
    }
    if resumed.to_read.is_empty() {
        return ended_reading_nothing(app, &checked);
    }
    let to_read: Vec<(usize, PlanSource, PlannedRepo)> = resumed
        .to_read
        .iter()
        .map(|position| {
            let row = &resumed.rows[*position];
            (*position, row.source, carried_repo(row))
        })
        .collect();
    read_planned(app, owner_did, run_id, &to_read).await
}

/// A resuming run's own plan: the paused run's rows still `planned` that
/// [`resume_plan`] reads now or carries, in plan order, and the paused
/// run's skipped repos.
struct ResumedPlan {
    rows: Vec<PlannedScanRepo>,
    /// Positions in `rows` read now.
    to_read: Vec<usize>,
    skipped: Vec<SkippedScanRepo>,
}

impl ResumedPlan {
    fn of(
        plan: &[ScanPlanRow],
        skipped: &[SkippedScanRepo],
        checks: &BTreeMap<u64, BioCheck>,
        listed: &ListedSources,
    ) -> Self {
        let frozen: Vec<FrozenRow> = plan
            .iter()
            .map(|row| FrozenRow {
                source: row.repo.source,
                status: row.status,
            })
            .collect();
        let resume = resume_plan(&frozen, checks, &listed.removed_from(&frozen));
        let kept = resume.kept();
        let skipped_now = resume
            .skipped
            .iter()
            .filter_map(|(old, why)| named_carried_skip(&plan[*old].repo, *why));
        Self {
            rows: kept.iter().map(|old| plan[*old].repo.clone()).collect(),
            to_read: kept
                .iter()
                .enumerate()
                .filter(|(_, old)| resume.to_read.contains(old))
                .map(|(position, _)| position)
                .collect(),
            skipped: skipped.iter().cloned().chain(skipped_now).collect(),
        }
    }

    /// Record it as the resuming run's plan, before any read.
    fn record(&self, app: &App, owner_did: &str, run_id: &str) -> Result<(), ReviewStoreError> {
        app.scans.record_plan(owner_did, run_id, &self.rows)?;
        app.scans.record_skipped(owner_did, run_id, &self.skipped)
    }
}

/// A carried row a resume skips, named for the summary: an account that
/// failed its check names its repos; a removed source's are not named.
fn named_carried_skip(row: &PlannedScanRepo, why: CarriedSkip) -> Option<SkippedScanRepo> {
    match (why, row.source) {
        (CarriedSkip::AccountUnverified, PlanSource::Account(account_id)) => {
            Some(SkippedScanRepo {
                repo_id: row.repo_id,
                full_name: row.full_name.clone(),
                source_account_id: account_id,
                reason: RepoSkipReason::AccountUnverified,
            })
        }
        _ => None,
    }
}

/// The sources the owner lists now: her accounts and her imports.
fn listed_sources(app: &App, owner_did: &str) -> Result<ListedSources, ReviewStoreError> {
    let accounts = app.links.github_accounts(owner_did)?;
    let imports = app.imports_read.imports(owner_did)?;
    Ok(ListedSources {
        accounts: accounts.iter().map(|a| a.github_user_id).collect(),
        imports: imports.iter().map(|import| import.repo_id).collect(),
    })
}

/// Read the planned repos in order, each marked read at its plan position
/// once stored; pause when GitHub's budget runs low. Before each repo its
/// source's presence is re-read: a removed source's repo is not read
/// (ADR-088 §7).
async fn read_planned(
    app: &App,
    owner_did: &str,
    run_id: &str,
    to_read: &[(usize, PlanSource, PlannedRepo)],
) -> Ended {
    let mut found = ScanCounts::default();
    for (position, source, repo) in to_read {
        if !github_budget_allows(app.github.last_rate_budget().map(|b| b.remaining)) {
            return paused(app).after_finding(found);
        }
        match listed_sources(app, owner_did) {
            Ok(listed) if listed.holds(*source) => {}
            Ok(_) => continue,
            Err(_) => return Ended::with_status(ScanStatus::Interrupted).after_finding(found),
        }
        match scan_repo(app, owner_did, repo, *source).await {
            Ok(counts) => found = add_counts(found, counts),
            Err(ended) => return ended.after_finding(found),
        }
        let _ = app.scans.mark_repo_read(owner_did, run_id, *position);
        emit(LogEvent::ScanRepoRead(AccountCheckOutcome::Passed));
    }
    Ended::with_status(ScanStatus::Completed).after_finding(found)
}

/// The account's owned repos now: its listing, completed (when GitHub had
/// more pages than it reads) by one `read_repo` per ticked repo it does not
/// show, so a truncated listing never proves a chosen repo missing (I-06).
async fn account_listing(
    app: &App,
    ownership: &VerifiedOwnership,
    saved: &[SavedRepoChoice],
) -> Result<Listing, GithubError> {
    let listing: AccountRepoListing = app
        .github
        .list_account_repos(ownership.github_login())
        .await?;
    let _ = app.listing_note.record_eligible_count(
        ownership.owner_did(),
        ownership.github_user_id(),
        eligible_count(&listing.repos),
    );
    let mut reads: Vec<(u64, RepoFacts)> = Vec::new();
    for chosen in unlisted_ticks(saved, &listing) {
        let Some((owner, name)) = chosen.full_name.split_once('/') else {
            continue;
        };
        match app.github.read_repo(owner, name).await {
            Ok(facts) => reads.push((chosen.repo_id, facts)),
            Err(GithubError::NotFound { .. } | GithubError::NotPublic { .. }) => {}
            Err(error) => return Err(error),
        }
    }
    Ok(listing_with_reads(
        listing,
        ownership.github_user_id(),
        &reads,
    ))
}

/// Harvest one planned repo, derive and reconcile its suggestions, persist
/// the new with where they came from (an imported repo's are stored as
/// imported, never publishable); what it found counts toward the scan's
/// summary.
async fn scan_repo(
    app: &App,
    owner_did: &str,
    repo: &PlannedRepo,
    source: PlanSource,
) -> Result<ScanCounts, Ended> {
    let Some((owner, name)) = repo.full_name.split_once('/') else {
        return Ok(ScanCounts::default());
    };
    let signals = app
        .github
        .harvest_repo(owner, name)
        .await
        .map_err(|error| failed(app, &error))?;
    let subject = format!("github:{}", repo.full_name);
    let derived = derive_candidates(&subject, &signals, &app.mapping)
        .iter()
        .map(|candidate| suggestion_from_candidate(candidate, &repo.full_name))
        .collect();
    let interrupted = |_| Ended::with_status(ScanStatus::Interrupted);
    let existing: BTreeMap<_, _> = app
        .review_read
        .suggestion_states(owner_did)
        .map_err(interrupted)?
        .into_iter()
        .collect();
    let reconciled = reconcile(&existing, derived);
    let stored = match source {
        PlanSource::Account(account_id) => app.review_write.add_pending_from_listed_source(
            owner_did,
            &SuggestionSource {
                account_id,
                repo_id: repo.repo_id,
                outside_default: repo.outside_default,
            },
            &reconciled.new,
        ),
        PlanSource::Imported(repo_id) => {
            app.review_write
                .add_pending_from_import(owner_did, repo_id, &reconciled.new)
        }
    };
    stored.map(|()| reconciled.counts()).map_err(interrupted)
}

/// A GitHub failure mid-scan: rate limits pause, anything else interrupts.
fn failed(app: &App, error: &GithubError) -> Ended {
    match error {
        GithubError::RateLimited { .. } => paused(app),
        _ => Ended::with_status(ScanStatus::Interrupted),
    }
}

/// Paused for GitHub's budget: resume when it refills.
fn paused(app: &App) -> Ended {
    let now = unix_now_secs();
    let resume_after = app
        .github
        .last_rate_budget()
        .map(|budget| budget.reset_at)
        .filter(|reset_at| *reset_at > now)
        .unwrap_or(now + DEFAULT_PAUSE_SECS);
    Ended {
        status: ScanStatus::RateLimited,
        resume_after: Some(resume_after),
        counts: ScanCounts::default(),
    }
}
