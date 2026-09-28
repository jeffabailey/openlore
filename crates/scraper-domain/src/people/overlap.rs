//! Cross-repo overlap and possible renames over the recorded contribution
//! links (US-CPI-001 AC4; Q-CPI-D7; DDD-5 case-folded keys). Read-only:
//! links are never merged or rewritten (D-5).

use std::collections::{BTreeMap, BTreeSet};

use ports::{ContributionLink, RankedContributor};

use super::subject_key;

/// One recorded person of the scraped repo who is ALSO linked to another
/// repo the user scraped earlier — rendered `login → owner/repo`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SharedContributor {
    /// The person's login as the current selection ranked it.
    pub login: String,
    /// The other repo, display form without the `github:` scheme.
    pub other_repo: String,
}

/// The people of `current_repo` also linked to OTHER scraped repos, in the
/// selection's rank order then by other repo — one line per (person, repo).
/// Subjects are compared case-folded (DDD-5 keys); the scraped repo itself is
/// never "other", so the result is the same whether `recorded_links` was read
/// before or after this scrape's snapshot was recorded.
pub fn shared_contributors(
    current_repo: &str,
    people: &[RankedContributor],
    recorded_links: &[ContributionLink],
) -> Vec<SharedContributor> {
    let current_repo_key = subject_key(current_repo);
    let other_repos_by_person = recorded_links
        .iter()
        .filter(|link| subject_key(&link.repo_subject) != current_repo_key)
        .fold(
            BTreeMap::<String, BTreeMap<String, String>>::new(),
            |mut by_person, link| {
                by_person
                    .entry(subject_key(&link.person_subject))
                    .or_default()
                    .entry(subject_key(&link.repo_subject))
                    .or_insert_with(|| repo_display(&link.repo_subject));
                by_person
            },
        );
    let mut people_seen = BTreeSet::new();
    people
        .iter()
        .filter(|person| people_seen.insert(subject_key(&person.person_subject())))
        .flat_map(|person| {
            other_repos_by_person
                .get(&subject_key(&person.person_subject()))
                .into_iter()
                .flat_map(BTreeMap::values)
                .map(|other_repo| SharedContributor {
                    login: person.login.clone(),
                    other_repo: other_repo.clone(),
                })
        })
        .collect()
}

/// The OTHER person subjects recorded with the same numeric GitHub user id as
/// `person_subject` — a possible rename (OD-CPI-1 / Q-CPI-D7). Subjects are
/// compared case-folded (DDD-5 keys), so a re-cased login is never its own
/// rename; each other person appears once, in its smallest recorded spelling,
/// ordered by key. Only FLAGS: links are read, never merged or rewritten (D-5).
pub fn possible_renames(person_subject: &str, links: &[ContributionLink]) -> Vec<String> {
    let person_key = subject_key(person_subject);
    let own_user_ids: BTreeSet<u64> = links
        .iter()
        .filter(|link| subject_key(&link.person_subject) == person_key)
        .map(|link| link.github_user_id)
        .collect();
    links
        .iter()
        .filter(|link| own_user_ids.contains(&link.github_user_id))
        .filter(|link| subject_key(&link.person_subject) != person_key)
        .fold(BTreeMap::<String, &str>::new(), |mut others, link| {
            let spelling = others
                .entry(subject_key(&link.person_subject))
                .or_insert(&link.person_subject);
            *spelling = (*spelling).min(link.person_subject.as_str());
            others
        })
        .into_values()
        .map(str::to_string)
        .collect()
}

/// A repo subject's display form without the `github:` scheme.
fn repo_display(repo_subject: &str) -> String {
    repo_subject
        .strip_prefix("github:")
        .unwrap_or(repo_subject)
        .to_string()
}
