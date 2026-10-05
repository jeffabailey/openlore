//! The scan task (component-boundaries: Scan task row; ADR-076 §3). It runs
//! in the background: a fresh ownership re-check mints the
//! [`VerifiedOwnership`] no repo read can happen without (I-BRA-4), then
//! `select_person_repos` → per repo: harvest → `derive_candidates` →
//! `reconcile` → persist. Each repo's suggestions are stored as soon as that
//! repo completes, so a paused scan keeps its partial results, and a resumed
//! one offers only keys never seen before. The run records what it found
//! (DWD-11 summary counts, of its own derived keys only). Nothing is written
//! to any PDS; suggestion content is never logged.

use std::collections::BTreeMap;
use std::sync::Arc;

use ports::{GithubError, GithubLink, OwnedRepo, ScanCounts, ScanStatus};
use review_domain::budget::github_budget_allows;
use review_domain::lifecycle::suggestion_from_candidate;
use review_domain::ownership::{prove_ownership, OwnershipRefusal, VerifiedOwnership};
use review_domain::reconcile::{add_counts, reconcile};
use scraper_domain::{derive_candidates, select_person_repos, DEFAULT_PERSON_REPO_COUNT};

use crate::http::App;
use crate::limiter::unix_now_secs;
use crate::routes::github::{account_of, refusal_of};
use crate::wiring::{observe, LogEvent};

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

/// Run one admitted scan for `owner_did` to its end, then free its slot and
/// record how it ended.
pub(crate) async fn run_scan(app: Arc<App>, owner_did: String, run_id: String, link: GithubLink) {
    let ended = match reverify(&app, &owner_did, &link).await {
        Ok(ownership) => scan_owned_repos(&app, &ownership).await,
        Err(refusal) if refusal.disproves_ownership() => {
            let _ = app.links.mark_link_unverified(&owner_did, refusal.label());
            Ended::with_status(ScanStatus::OwnershipFailed)
        }
        Err(OwnershipRefusal::RateLimited) => paused(&app),
        Err(_) => Ended::with_status(ScanStatus::Interrupted),
    };
    app.scan_limiter.release(&owner_did);
    observe(&app, LogEvent::ScanFinished(ended.status));
    let _ = app.scans.finish_scan(
        &owner_did,
        &run_id,
        ended.status,
        ended.resume_after,
        ended.counts,
    );
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
        .read_person(&link.github_login)
        .await
        .map_err(|error| refusal_of(&error))?;
    prove_ownership(owner_did, &account_of(&profile), Some(link.github_user_id))
}

/// The repo stages; they can only run with proof in hand.
async fn scan_owned_repos(app: &App, ownership: &VerifiedOwnership) -> Ended {
    let repos = match app.github.list_owned_repos(ownership.github_login()).await {
        Ok(repos) => repos,
        Err(error) => return failed(app, &error),
    };
    let selection = select_person_repos(&repos, DEFAULT_PERSON_REPO_COUNT);
    let mut found = ScanCounts::default();
    for repo in &selection.chosen {
        if !github_budget_allows(app.github.last_rate_budget().map(|b| b.remaining)) {
            return paused(app).after_finding(found);
        }
        match scan_repo(app, ownership.owner_did(), repo).await {
            Ok(counts) => found = add_counts(found, counts),
            Err(ended) => return ended.after_finding(found),
        }
    }
    Ended::with_status(ScanStatus::Completed).after_finding(found)
}

/// Harvest one repo, derive and reconcile its suggestions, persist the new;
/// what it found counts toward the scan's summary.
async fn scan_repo(app: &App, owner_did: &str, repo: &OwnedRepo) -> Result<ScanCounts, Ended> {
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
    app.review_write
        .add_pending(owner_did, &reconciled.new)
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
