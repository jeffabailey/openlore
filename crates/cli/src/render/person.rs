//! `openlore scrape person` output — the profile, the repos chosen for
//! scraping, the rate-budget note, and one summary per scraped repo. Pure.

use super::*;
use ports::{OwnedRepo, PersonProfile};
use scraper_domain::PersonRepoSelection;

/// GitHub's unauthenticated budget, in requests per hour.
pub const UNAUTHENTICATED_HOURLY_REQUESTS: usize = 60;
/// Roughly how many GitHub requests one repo scrape makes (resolve, the
/// signal probes, contributors).
pub const REQUESTS_PER_REPO_SCRAPE: usize = 8;

/// The profile block: who they are, then each filled-in field.
pub fn render_person_profile(profile: &PersonProfile) -> String {
    let who = match &profile.name {
        Some(name) => format!("{name} (@{})", profile.login),
        None => format!("@{}", profile.login),
    };
    let mut out = format!("{who} — {}\n", profile.html_url);
    for (label, value) in [
        ("bio", &profile.bio),
        ("company", &profile.company),
        ("location", &profile.location),
        ("blog", &profile.blog),
    ] {
        if let Some(value) = value {
            out.push_str(&format!("  {label:<9} : {value}\n"));
        }
    }
    let since = profile
        .created_at
        .as_deref()
        .map(|at| format!(" · on GitHub since {}", day(at)))
        .unwrap_or_default();
    out.push_str(&format!(
        "  followers : {} · following {} · {} public repo{}{since}\n",
        profile.followers,
        profile.following,
        profile.public_repos,
        plural_suffix(profile.public_repos as usize),
    ));
    out
}

/// The repos `scrape person` will scrape, best first, with what was skipped.
pub fn render_person_repo_list(selection: &PersonRepoSelection) -> String {
    if selection.candidates == 0 {
        return "No owned repos to scrape (forks and archived repos are skipped).\n".to_string();
    }
    if selection.chosen.is_empty() {
        return format!(
            "Top repos: none scraped (--repos 0; {} owned)\n",
            selection.candidates
        );
    }
    let mut skipped = Vec::new();
    if selection.forks_skipped > 0 {
        skipped.push(format!(
            "{} fork{} skipped",
            selection.forks_skipped,
            plural_suffix(selection.forks_skipped)
        ));
    }
    if selection.archived_skipped > 0 {
        skipped.push(format!("{} archived skipped", selection.archived_skipped));
    }
    let skipped = skipped
        .iter()
        .map(|note| format!("; {note}"))
        .collect::<String>();
    let mut out = format!(
        "Top repos ({} of {} owned{skipped}):\n",
        selection.chosen.len(),
        selection.candidates
    );
    let width = selection
        .chosen
        .iter()
        .map(|r| r.full_name.len())
        .max()
        .unwrap_or(0);
    for repo in &selection.chosen {
        out.push_str(&format!(
            "  {:<width$}  ★ {:<5} {}\n",
            repo.full_name,
            repo.stars,
            repo_facts(repo)
        ));
    }
    out
}

/// Language and last push, whichever GitHub served.
fn repo_facts(repo: &OwnedRepo) -> String {
    let pushed = repo
        .pushed_at
        .as_deref()
        .map(|at| format!("pushed {}", day(at)));
    [repo.language.clone(), pushed]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join("  ")
}

/// The note shown before an unauthenticated multi-repo scrape.
pub fn render_unauthenticated_budget_note(repo_count: usize) -> String {
    format!(
        "Note: without GITHUB_TOKEN, GitHub allows {UNAUTHENTICATED_HOURLY_REQUESTS} requests an \
         hour; scraping {repo_count} repo{} makes about {}. Set GITHUB_TOKEN to raise the limit \
         to 5,000.\n",
        plural_suffix(repo_count),
        repo_count * REQUESTS_PER_REPO_SCRAPE
    )
}

/// One scraped repo: counts, the numbered candidates (numbered as
/// `scrape github <repo> --sign N` numbers them), and how to sign.
pub fn render_person_repo_scrape(
    full_name: &str,
    signal_count: usize,
    candidates: &[CandidateClaim],
    contributors_note: &str,
) -> String {
    let claims = match candidates.len() {
        0 => "no candidate claims".to_string(),
        n => format!("{n} candidate claim{}", plural_suffix(n)),
    };
    let mut out = format!(
        "{full_name} — {signal_count} signal{}, {claims}, {contributors_note}\n",
        plural_suffix(signal_count)
    );
    for (idx, candidate) in candidates.iter().enumerate() {
        let signals = candidate
            .source_signals()
            .iter()
            .map(|s| s.value.as_str())
            .collect::<Vec<_>>()
            .join("; ");
        out.push_str(&format!(
            "   [{}] {}  {}  ({signals})\n",
            idx + 1,
            candidate.predicate,
            candidate.object
        ));
    }
    if !candidates.is_empty() {
        out.push_str(&format!(
            "  sign one: openlore scrape github {full_name} --sign N\n"
        ));
    }
    out
}

/// The `YYYY-MM-DD` of an RFC 3339 timestamp (verbatim if shorter).
fn day(at: &str) -> &str {
    at.get(..10).unwrap_or(at)
}

#[cfg(test)]
mod person_tests {
    use super::*;

    fn profile() -> PersonProfile {
        PersonProfile {
            login: "jeffabailey".to_string(),
            id: 1,
            name: Some("Jeff Bailey".to_string()),
            bio: Some("Builds tools".to_string()),
            company: None,
            location: Some("Portland".to_string()),
            blog: None,
            followers: 42,
            following: 3,
            public_repos: 1,
            created_at: Some("2010-05-06T07:08:09Z".to_string()),
            html_url: "https://github.com/jeffabailey".to_string(),
        }
    }

    fn repo(name: &str, stars: u64) -> OwnedRepo {
        OwnedRepo {
            full_name: name.to_string(),
            description: None,
            language: Some("Rust".to_string()),
            stars,
            fork: false,
            archived: false,
            pushed_at: Some("2026-09-29T01:02:03Z".to_string()),
        }
    }

    #[test]
    fn profile_shows_filled_fields_only() {
        let out = render_person_profile(&profile());
        assert!(out.starts_with("Jeff Bailey (@jeffabailey) — https://github.com/jeffabailey\n"));
        assert!(out.contains("  bio       : Builds tools\n"));
        assert!(out.contains("  location  : Portland\n"));
        assert!(!out.contains("company"));
        assert!(out
            .contains("followers : 42 · following 3 · 1 public repo · on GitHub since 2010-05-06"));
    }

    #[test]
    fn profile_without_a_name_shows_the_login() {
        let out = render_person_profile(&PersonProfile {
            name: None,
            ..profile()
        });
        assert!(out.starts_with("@jeffabailey — "));
    }

    #[test]
    fn repo_list_names_what_was_skipped() {
        let selection = PersonRepoSelection {
            chosen: vec![repo("me/big", 12), repo("me/small-one", 3)],
            candidates: 4,
            forks_skipped: 2,
            archived_skipped: 1,
        };
        let out = render_person_repo_list(&selection);
        assert!(out.starts_with("Top repos (2 of 4 owned; 2 forks skipped; 1 archived skipped):\n"));
        assert!(out.contains("  me/big        ★ 12    Rust  pushed 2026-09-29\n"));
    }

    #[test]
    fn repo_list_with_nothing_chosen_or_nothing_owned() {
        let none_chosen = PersonRepoSelection {
            chosen: vec![],
            candidates: 3,
            forks_skipped: 0,
            archived_skipped: 0,
        };
        assert_eq!(
            render_person_repo_list(&none_chosen),
            "Top repos: none scraped (--repos 0; 3 owned)\n"
        );
        let none_owned = PersonRepoSelection {
            candidates: 0,
            ..none_chosen
        };
        assert!(render_person_repo_list(&none_owned).starts_with("No owned repos to scrape"));
    }

    #[test]
    fn budget_note_estimates_requests() {
        assert!(render_unauthenticated_budget_note(3).contains("scraping 3 repos makes about 24"));
    }

    #[test]
    fn repo_scrape_without_candidates_has_no_sign_hint() {
        let out = render_person_repo_scrape("me/x", 0, &[], "contributors recorded: 1");
        assert_eq!(
            out,
            "me/x — 0 signals, no candidate claims, contributors recorded: 1\n"
        );
    }
}
