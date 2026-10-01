//! Which of a person's repos `scrape person` scrapes — PURE.
//!
//! GitHub lists every repo a person owns, forks and archived ones included.
//! A fork mostly carries someone else's work and an archived repo is no
//! longer maintained, so neither says much about how the person builds
//! software today: both are skipped (and counted, so the output can say so).
//! The rest are ranked by stars, then most recent push, then name, and the
//! top N are scraped.

use std::cmp::Reverse;

use ports::OwnedRepo;

/// Repos scraped when `--repos` is not given.
pub const DEFAULT_PERSON_REPO_COUNT: usize = 5;
/// The most repos one `scrape person` may scrape (each costs several
/// GitHub requests against an hourly budget).
pub const MAX_PERSON_REPO_COUNT: usize = 30;

/// Why a `--repos N` request is refused — before any GitHub request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PersonRepoCountError {
    pub requested: usize,
}

impl std::fmt::Display for PersonRepoCountError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "--repos {} is too many: at most {MAX_PERSON_REPO_COUNT} repos may be scraped per person",
            self.requested
        )
    }
}

impl std::error::Error for PersonRepoCountError {}

/// The `--repos` override (0..=30) or the default.
pub fn person_repo_count_for(requested: Option<usize>) -> Result<usize, PersonRepoCountError> {
    match requested {
        None => Ok(DEFAULT_PERSON_REPO_COUNT),
        Some(requested) if requested > MAX_PERSON_REPO_COUNT => {
            Err(PersonRepoCountError { requested })
        }
        Some(requested) => Ok(requested),
    }
}

/// The repos chosen for a person scrape, and what was passed over.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PersonRepoSelection {
    /// The top `n` candidates, best first.
    pub chosen: Vec<OwnedRepo>,
    /// Owned repos that are neither forks nor archived.
    pub candidates: usize,
    pub forks_skipped: usize,
    pub archived_skipped: usize,
}

/// Choose the top `n` of `repos`: forks and archived repos skipped, the rest
/// ranked by stars, then most recent push, then name.
pub fn select_person_repos(repos: &[OwnedRepo], n: usize) -> PersonRepoSelection {
    let mut candidates: Vec<&OwnedRepo> = repos.iter().filter(|r| !r.fork && !r.archived).collect();
    candidates.sort_by_key(|r| {
        (
            Reverse(r.stars),
            Reverse(r.pushed_at.clone()),
            r.full_name.clone(),
        )
    });
    PersonRepoSelection {
        candidates: candidates.len(),
        chosen: candidates.into_iter().take(n).cloned().collect(),
        forks_skipped: repos.iter().filter(|r| r.fork).count(),
        archived_skipped: repos.iter().filter(|r| !r.fork && r.archived).count(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repo(name: &str, stars: u64, pushed_at: &str) -> OwnedRepo {
        OwnedRepo {
            full_name: format!("me/{name}"),
            description: None,
            language: None,
            stars,
            fork: false,
            archived: false,
            pushed_at: Some(pushed_at.to_string()),
        }
    }

    fn names(selection: &PersonRepoSelection) -> Vec<&str> {
        selection
            .chosen
            .iter()
            .map(|r| r.full_name.as_str())
            .collect()
    }

    #[test]
    fn ranks_by_stars_then_recent_push_then_name() {
        let repos = [
            repo("old", 5, "2020-01-01T00:00:00Z"),
            repo("new", 5, "2026-01-01T00:00:00Z"),
            repo("popular", 50, "2019-01-01T00:00:00Z"),
            repo("b-tie", 1, "2024-01-01T00:00:00Z"),
            repo("a-tie", 1, "2024-01-01T00:00:00Z"),
        ];
        let selection = select_person_repos(&repos, 5);
        assert_eq!(
            names(&selection),
            ["me/popular", "me/new", "me/old", "me/a-tie", "me/b-tie"]
        );
    }

    #[test]
    fn skips_forks_and_archived_and_counts_them() {
        let mut fork = repo("fork", 900, "2026-01-01T00:00:00Z");
        fork.fork = true;
        let mut archived = repo("archived", 100, "2026-01-01T00:00:00Z");
        archived.archived = true;
        let repos = [fork, archived, repo("kept", 1, "2026-01-01T00:00:00Z")];

        let selection = select_person_repos(&repos, 5);
        assert_eq!(names(&selection), ["me/kept"]);
        assert_eq!(
            (
                selection.candidates,
                selection.forks_skipped,
                selection.archived_skipped
            ),
            (1, 1, 1)
        );
    }

    #[test]
    fn takes_at_most_n() {
        let repos = [
            repo("a", 3, "2026-01-01T00:00:00Z"),
            repo("b", 2, "2026-01-01T00:00:00Z"),
            repo("c", 1, "2026-01-01T00:00:00Z"),
        ];
        assert_eq!(names(&select_person_repos(&repos, 2)), ["me/a", "me/b"]);
        assert!(select_person_repos(&repos, 0).chosen.is_empty());
        assert_eq!(select_person_repos(&repos, 0).candidates, 3);
    }

    #[test]
    fn repo_count_defaults_and_is_bounded() {
        assert_eq!(person_repo_count_for(None), Ok(DEFAULT_PERSON_REPO_COUNT));
        assert_eq!(person_repo_count_for(Some(0)), Ok(0));
        assert_eq!(person_repo_count_for(Some(30)), Ok(30));
        assert_eq!(
            person_repo_count_for(Some(31)),
            Err(PersonRepoCountError { requested: 31 })
        );
    }
}
