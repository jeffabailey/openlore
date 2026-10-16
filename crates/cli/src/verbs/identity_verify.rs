//! `identity verify` — re-check every active identity link on a fresh GitHub
//! read and append one local check per decided re-check (ADR-085 §4/§7).
//!
//! The verb holds only the check read/write handles, the stored-claim reads
//! the link lineage needs, and GitHub (ADR-085 §7 table) — never the whole
//! wiring, no signing, no publishing (C-GDL-6). It never retracts: a lost
//! proof is a new check, and the published link stays (GDL-D5). A GitHub
//! failure for a link records nothing for it (fail closed, ADR-085 §4).

use anyhow::Result;
use chrono::{DateTime, Utc};
use claim_domain::identity_link::{
    is_github_login, is_in_scope, recheck_link, GithubRead, IdentityLink, ProofCheck, RecheckScope,
};
use ports::{
    CheckFilter, GithubAccountProfile, GithubError, GithubPort, IdentityLinkCheckReadPort,
    IdentityLinkCheckWritePort, LinkCandidateReadPort, LinkCheck, LinkLineageReadPort,
};

use crate::render::{
    render_no_identity_links_to_verify, render_recheck, render_recheck_unreadable,
};
use crate::verbs::identity_reads::{observation_of, StoredLinkLineage};

/// The narrow handles `identity verify` holds.
pub struct IdentityVerifyHandles<'a> {
    pub checks: &'a dyn IdentityLinkCheckReadPort,
    pub check_writes: &'a dyn IdentityLinkCheckWritePort,
    pub claims: &'a dyn LinkCandidateReadPort,
    /// Read-only: the claims referencing a link (its lineage).
    pub lineage: &'a dyn LinkLineageReadPort,
    pub github: &'a dyn GithubPort,
}

/// What one `identity verify` run reports: the decided re-checks (stdout)
/// and the links GitHub could not be read for (stderr; exit 1).
#[derive(Debug, Default)]
pub struct VerifyReport {
    pub stdout: String,
    pub unreadable: Vec<String>,
}

/// The active links in the store, once per (DID, account id).
fn active_links_in_store(handles: &IdentityVerifyHandles<'_>) -> Result<Vec<IdentityLink>> {
    Ok(StoredLinkLineage::read(handles.claims, handles.lineage)?.links_to_recheck())
}

/// The login to read for `link`: the last one this reader saw, else the
/// signed one (ADR-085 §4).
fn login_to_read<'a>(link: &'a IdentityLink, latest: &'a [LinkCheck]) -> &'a str {
    latest
        .iter()
        .find(|check| check.subject_did == link.did && check.github_user_id == link.user_id)
        .map_or(link.login.as_str(), |check| check.login_seen.as_str())
}

/// One GitHub answer as the check consumes it: "no such account" is an
/// answer; any other failure is kept so the caller can report it.
fn answer_of(
    read: std::result::Result<GithubAccountProfile, GithubError>,
) -> GithubRead<GithubError> {
    match read {
        Ok(profile) => GithubRead::Profile(observation_of(profile)),
        Err(GithubError::NotFound { .. }) => GithubRead::NotFound,
        Err(err) => GithubRead::Unreadable(err),
    }
}

/// The one check a decided re-check appends.
fn check_of(link: &IdentityLink, decided: &ProofCheck, now: DateTime<Utc>) -> LinkCheck {
    LinkCheck {
        subject_did: link.did.clone(),
        github_user_id: link.user_id,
        login_seen: decided.login_seen().to_string(),
        verdict: decided.verdict(),
        checked_at: now,
    }
}

/// Why a stored login is never sent to GitHub.
const NOT_A_GITHUB_LOGIN: &str = "the stored login is not a GitHub login; it was not read";

/// Re-check one link: read, decide, append. A GitHub failure, an answer
/// naming no account, or a stored login that is no GitHub login (never
/// sent) records nothing.
fn recheck(
    handles: &IdentityVerifyHandles<'_>,
    runtime: &tokio::runtime::Runtime,
    link: &IdentityLink,
    login: &str,
    now: DateTime<Utc>,
    mut report: VerifyReport,
) -> Result<VerifyReport> {
    if !is_github_login(login) {
        report
            .unreadable
            .push(render_recheck_unreadable(login, NOT_A_GITHUB_LOGIN));
        return Ok(report);
    }
    let github = handles.github;
    let login_answer = answer_of(runtime.block_on(github.read_account(login)));
    let read_by_account_number =
        |user_id: u64| answer_of(runtime.block_on(github.read_person_by_id(user_id)));
    match recheck_link(link, login, login_answer, read_by_account_number) {
        Ok(decided) => {
            let check = check_of(link, &decided, now);
            handles.check_writes.record_check(&check)?;
            report.stdout += &render_recheck(&check.subject_did, &check.login_seen, check.verdict);
        }
        Err(err) => report
            .unreadable
            .push(render_recheck_unreadable(login, &err.to_string())),
    }
    Ok(report)
}

/// Run `identity verify` as of `now`, re-checking the active links (own and
/// followed peers') within `scope`.
pub fn run(
    handles: &IdentityVerifyHandles<'_>,
    scope: &RecheckScope,
    now: DateTime<Utc>,
) -> Result<VerifyReport> {
    let latest = handles.checks.latest_checks(&CheckFilter::All, now)?;
    let links: Vec<IdentityLink> = active_links_in_store(handles)?
        .into_iter()
        .filter(|link| is_in_scope(scope, link, login_to_read(link, &latest)))
        .collect();
    if links.is_empty() {
        return Ok(VerifyReport {
            stdout: render_no_identity_links_to_verify(),
            unreadable: Vec::new(),
        });
    }
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_io()
        .enable_time()
        .build()?;
    links
        .iter()
        .try_fold(VerifyReport::default(), |report, link| {
            recheck(
                handles,
                &runtime,
                link,
                login_to_read(link, &latest),
                now,
                report,
            )
        })
}
