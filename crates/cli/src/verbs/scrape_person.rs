//! `scrape person <user | profile URL> [--repos N] [--sign N[,N...]]` — read
//! a GitHub person: their public profile, their top owned repos, a scrape of
//! each of those repos, then the person view.
//!
//! The pipeline:
//!
//! 1. normalize the target (a profile URL becomes the login) and refuse an
//!    `owner/repo` target, pointing at `scrape github`; validate `--repos`
//!    PURELY — all before any GitHub request;
//! 2. the public-data banner, then ONE `/users/{user}` read (the profile) and
//!    ONE `/users/{user}/repos` read;
//! 3. choose the top N owned repos PURELY (`select_person_repos`: forks and
//!    archived repos skipped, ranked by stars then recent push);
//! 4. scrape each through the SAME per-repo beats as `scrape github`
//!    (`scrape_repo_into_store`): signals, candidates, contributors recorded
//!    as unsigned links. A rate-limit refusal stops the loop; any other
//!    per-repo refusal skips that repo;
//! 5. the person view (`person_view`) over the now-larger store, whose
//!    `--sign` signs PERSON candidates — repo candidates are signed with
//!    `scrape github <repo> --sign N`, which each repo summary names.
//!
//! Output is printed as it happens (a multi-repo scrape takes a while); the
//! person view is returned to the dispatcher like every other verb's output.
//! Like `scrape github`, this verb writes NO claim without `--sign` (I-SCR-1).

use std::io::Write;

use anyhow::{bail, Result};
use ports::GithubError;
use scraper_domain::{
    normalize_target, person_repo_count_for, select_person_repos, DEFAULT_CONTRIBUTOR_COUNT,
};

use crate::render::{
    render_auth_report, render_person_profile, render_person_repo_list, render_person_repo_scrape,
    render_public_data_banner, render_unauthenticated_budget_note, AuthReport,
};
use crate::verbs::claim_publish::build_tokio_runtime;
use crate::verbs::scrape_github::{
    person_view, scrape_repo_into_store, ContributorsOutcome, RepoTarget,
};
use crate::wiring::Wiring;

/// Argument struct for the `scrape person` verb (mirrors the clap subcommand).
#[derive(Debug, Clone)]
pub struct ScrapePersonArgs {
    /// A GitHub login or profile URL.
    pub target: String,
    /// How many owned repos to scrape (`None` = the default).
    pub repos: Option<usize>,
    /// Raw `--sign N[,N...]` over the person view's candidates, unparsed.
    pub sign: Option<String>,
}

/// Outcome of one `scrape person` run — exit code + the person view.
pub struct ScrapePersonOutcome {
    pub exit_code: i32,
    pub stdout: String,
}

/// Run `scrape person` — see the module docs for the pipeline.
pub fn run(wiring: &Wiring, args: &ScrapePersonArgs) -> Result<ScrapePersonOutcome> {
    let user = normalize_target(&args.target);
    if user.contains('/') {
        bail!(
            "`{user}` is a repo; scrape person takes a GitHub user or profile URL. \
             To scrape the repo, run: openlore scrape github {user}"
        );
    }
    let repo_count = person_repo_count_for(args.repos)?;

    emit(&render_public_data_banner());
    let runtime = build_tokio_runtime();
    let profile = runtime
        .block_on(wiring.github.read_person(&user))
        .map_err(anyhow::Error::from)?;
    let auth = adapter_github::take_last_auth_report();
    emit(&format!(
        "Resolving person {} ... ok\n{}{}",
        profile.login,
        render_auth_report(&auth),
        render_person_profile(&profile)
    ));

    let owned = runtime
        .block_on(wiring.github.list_owned_repos(&profile.login))
        .map_err(anyhow::Error::from)?;
    let selection = select_person_repos(&owned, repo_count);
    emit(&render_person_repo_list(&selection));
    if matches!(auth, AuthReport::Anonymous) && !selection.chosen.is_empty() {
        emit(&render_unauthenticated_budget_note(selection.chosen.len()));
    }

    for (done, repo) in selection.chosen.iter().enumerate() {
        let Some((owner, name)) = repo.full_name.split_once('/') else {
            continue;
        };
        let target = RepoTarget {
            owner,
            repo: name,
            contributor_count: DEFAULT_CONTRIBUTOR_COUNT,
        };
        match scrape_repo_into_store(wiring, &runtime, &target) {
            Ok(scraped) => emit(&render_person_repo_scrape(
                &repo.full_name,
                scraped.signal_count,
                &scraped.candidates,
                &contributors_note(&scraped.contributors),
            )),
            Err(err) if is_rate_limited(&err) => {
                emit(&format!(
                    "Stopped: GitHub's rate limit was reached after {done} of {} repos. \
                     Set GITHUB_TOKEN or try again later.\n",
                    selection.chosen.len()
                ));
                break;
            }
            Err(err) => emit(&format!("{} — skipped: {err:#}\n", repo.full_name)),
        }
    }

    let outcome = person_view(wiring, &profile.login, args.sign.as_deref(), String::new())?;
    Ok(ScrapePersonOutcome {
        exit_code: outcome.exit_code,
        stdout: outcome.stdout,
    })
}

/// The contributors part of a repo summary line.
fn contributors_note(outcome: &ContributorsOutcome) -> String {
    match outcome {
        ContributorsOutcome::Recorded { selection, .. } => {
            format!("contributors recorded: {}", selection.people.len())
        }
        ContributorsOutcome::Unavailable(reason) => format!("contributors not recorded ({reason})"),
        ContributorsOutcome::NoneRequested => "contributors not requested".to_string(),
    }
}

fn is_rate_limited(err: &anyhow::Error) -> bool {
    matches!(
        err.downcast_ref::<GithubError>(),
        Some(GithubError::RateLimited { .. })
    )
}

/// Print a chunk now, so a long multi-repo scrape shows its progress.
fn emit(chunk: &str) {
    print!("{chunk}");
    let _ = std::io::stdout().flush();
}
