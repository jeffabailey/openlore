//! The scan task (component-boundaries: Scan task row; ADR-076 §3). It runs
//! in the background: a fresh ownership re-check mints the
//! [`VerifiedOwnership`] no repo read can happen without (I-BRA-4), then
//! the account's saved choice (or today's top 5) is frozen as the run's plan
//! (`repo_scope::account_plan`, ADR-088 §6) → per planned repo: harvest →
//! `derive_candidates` →
//! `reconcile` → persist. Each repo's suggestions are stored as soon as that
//! repo completes, so a paused scan keeps its partial results, and a resumed
//! one offers only keys never seen before. The run records what it found
//! (DWD-11 summary counts, of its own derived keys only). Nothing is written
//! to any PDS; suggestion content is never logged.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use ports::{GithubError, GithubLink, PlannedScanRepo, ScanCounts, ScanStatus, SuggestionSource};
use review_domain::budget::{auto_resume_wait, github_budget_allows, ScanAdmission};
use review_domain::lifecycle::suggestion_from_candidate;
use review_domain::ownership::{prove_ownership, OwnershipRefusal, VerifiedOwnership};
use review_domain::reconcile::{add_counts, reconcile};
use review_domain::repo_scope::{account_plan, AccountChoice, PlannedRepo};
use review_domain::views::ScanRefused;
use scraper_domain::derive_candidates;

use crate::http::App;
use crate::limiter::unix_now_secs;
use crate::routes::github::{account_of, refusal_of};
use crate::routes::signin::random_token;
use crate::wiring::{emit, Counted, LogEvent};

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
    /// No verified GitHub link: verify first.
    NotVerified,
    AlreadyScanning,
    /// The budget said no (logged as `scan.refused`).
    Refused(ScanRefused),
    /// The run could not be recorded (logged as `scan.refused` busy).
    Unavailable,
}

/// Start a scan for `owner_did` if their link is verified and the budget
/// admits it; it then runs in the background. The one way any scan starts,
/// whether the owner pressed Scan or a paused scan resumes on its own.
pub(crate) fn begin_scan(app: &Arc<App>, owner_did: &str) -> ScanStart {
    let link = app.links.github_link(owner_did).ok().flatten();
    let Some(link) = link.filter(|link| link.verified) else {
        return ScanStart::NotVerified;
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
    tokio::spawn(run_scan(app.clone(), owner_did.to_string(), run_id, link));
    ScanStart::Started
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
pub(crate) async fn run_scan(app: Arc<App>, owner_did: String, run_id: String, link: GithubLink) {
    let ended = match reverify(&app, &owner_did, &link).await {
        Ok(ownership) => scan_owned_repos(&app, &ownership, &run_id, link.github_user_id).await,
        Err(refusal) if refusal.disproves_ownership() => {
            let _ =
                app.links
                    .mark_account_unverified(&owner_did, link.github_user_id, refusal.label());
            Ended::with_status(ScanStatus::OwnershipFailed)
        }
        Err(OwnershipRefusal::RateLimited) => paused(&app),
        Err(_) => Ended::with_status(ScanStatus::Interrupted),
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

/// Read the linked profile now and prove ownership again, pinned to the
/// numeric id that was verified.
async fn reverify(
    app: &App,
    owner_did: &str,
    link: &GithubLink,
) -> Result<VerifiedOwnership, OwnershipRefusal> {
    let profile = app
        .github
        .read_account(&link.github_login)
        .await
        .map_err(|error| refusal_of(&error))?;
    prove_ownership(owner_did, &account_of(&profile), Some(link.github_user_id))
}

/// The repo stages; they can only run with proof in hand. The account's
/// choice in force now is frozen as the run's plan before the first read
/// (BR-RRA-4), and exactly the planned repos are read.
async fn scan_owned_repos(
    app: &App,
    ownership: &VerifiedOwnership,
    run_id: &str,
    account_id: u64,
) -> Ended {
    let listing = match app
        .github
        .list_account_repos(ownership.github_login())
        .await
    {
        Ok(listing) => listing,
        Err(error) => return failed(app, &error),
    };
    let owner_did = ownership.owner_did();
    let Ok(saved) = app.choices_read.saved_choice(owner_did, account_id) else {
        return Ended::with_status(ScanStatus::Interrupted);
    };
    let plan = account_plan(&listing, AccountChoice::of(&saved));
    if app
        .scans
        .record_plan(owner_did, run_id, &plan_rows(&plan, account_id))
        .is_err()
    {
        return Ended::with_status(ScanStatus::Interrupted);
    }
    let mut found = ScanCounts::default();
    for (position, repo) in plan.iter().enumerate() {
        if !github_budget_allows(app.github.last_rate_budget().map(|b| b.remaining)) {
            return paused(app).after_finding(found);
        }
        match scan_repo(app, owner_did, repo, account_id).await {
            Ok(counts) => found = add_counts(found, counts),
            Err(ended) => return ended.after_finding(found),
        }
        let _ = app.scans.mark_repo_read(owner_did, run_id, position);
    }
    Ended::with_status(ScanStatus::Completed).after_finding(found)
}

/// The store rows of an account's plan, in plan order.
fn plan_rows(plan: &[PlannedRepo], account_id: u64) -> Vec<PlannedScanRepo> {
    plan.iter()
        .map(|repo| PlannedScanRepo {
            repo_id: repo.repo_id,
            full_name: repo.full_name.clone(),
            source_account_id: account_id,
        })
        .collect()
}

/// Harvest one planned repo, derive and reconcile its suggestions, persist
/// the new with where they came from; what it found counts toward the
/// scan's summary.
async fn scan_repo(
    app: &App,
    owner_did: &str,
    repo: &PlannedRepo,
    account_id: u64,
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
    let source = SuggestionSource {
        account_id,
        repo_id: repo.repo_id,
        outside_default: repo.outside_default,
    };
    app.review_write
        .add_pending_from(owner_did, &source, &reconciled.new)
        .map(|()| reconciled.counts())
        .map_err(interrupted)
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
