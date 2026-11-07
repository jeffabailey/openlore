//! "Add repo" (CORE-RRA-9, US-RRA-005): a public repo the owner doesn't
//! own, admitted or refused for one named cause (DWD-RRA-4). PURE.

use ports::claim_domain::authority::RepoRead;
use ports::{ImportedRepo, RepoFacts};

use super::limits::{admit_import, LimitRefusal, ScopeAccount};

/// The `owner/name` an owner typed into "Add repo", when it has that shape:
/// `owner/name` itself, or the repo's github.com link (with or without the
/// scheme or `www.`, a trailing `/` or `.git`, a query or a fragment).
pub fn import_shape(typed: &str) -> Option<(&str, &str)> {
    let (owner, name) = github_repo_path(typed.trim()).split_once('/')?;
    let part = |s: &str| {
        !s.is_empty()
            && s.chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
    };
    (part(owner) && part(name)).then_some((owner, name))
}

/// The repo a refusal names: the `owner/name` the typed text names, or the
/// text as typed when it names none.
pub fn shown_import_name(typed: &str) -> String {
    import_shape(typed).map_or_else(
        || typed.trim().to_string(),
        |(owner, name)| format!("{owner}/{name}"),
    )
}

/// The path after `github.com/` of a github.com link, without a trailing
/// `/`, `.git`, query or fragment; anything else unchanged.
fn github_repo_path(typed: &str) -> &str {
    let rest = strip_prefix_ignoring_case(typed, "https://")
        .or_else(|| strip_prefix_ignoring_case(typed, "http://"))
        .unwrap_or(typed);
    let rest = strip_prefix_ignoring_case(rest, "www.").unwrap_or(rest);
    let Some(path) = strip_prefix_ignoring_case(rest, "github.com/") else {
        return typed;
    };
    let path = path.split(['?', '#']).next().unwrap_or(path);
    let path = path.strip_suffix('/').unwrap_or(path);
    path.strip_suffix(".git").unwrap_or(path)
}

fn strip_prefix_ignoring_case<'a>(text: &'a str, prefix: &str) -> Option<&'a str> {
    let head = text.get(..prefix.len())?;
    head.eq_ignore_ascii_case(prefix)
        .then(|| &text[prefix.len()..])
}

/// Why a named repo is not imported (DWD-RRA-4: the first that applies).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportRefusal {
    /// Not `owner/name`: decided without asking GitHub.
    Shape,
    /// GitHub says no public repo is there (missing or private).
    NotFound,
    /// GitHub could not answer (outage or rate limit).
    Unreachable,
    /// A listed account (verified or not) owns it (D-RRA-T2).
    OwnedByListedAccount,
    /// It is already on "Repos you don't own" (D-RRA-T3).
    AlreadyImported,
    /// Adding it would pass the limit.
    OverLimit(LimitRefusal),
}

/// What "Add repo" decides (CORE-RRA-9): the import to store, or the one
/// refusal that names its cause.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportVerdict {
    Admitted(ImportedRepo),
    Refused(ImportRefusal),
}

/// Classify an "Add repo" request (CORE-RRA-9, DWD-RRA-4) in a fixed order:
/// shape → the repo read → owned by a listed account → already imported →
/// over the limit → admitted.
pub fn classify_import(
    typed: &str,
    read: RepoRead<'_>,
    listed: &[u64],
    accounts: &[ScopeAccount],
    imports: &[u64],
    cap: usize,
) -> ImportVerdict {
    let admitted = import_shape(typed)
        .ok_or(ImportRefusal::Shape)
        .and(found(read))
        .and_then(|facts| not_owned(facts, listed))
        .and_then(|facts| not_yet_imported(facts, imports))
        .and_then(|facts| {
            admit_import(accounts, imports, cap)
                .map(|_| import_of(facts))
                .map_err(ImportRefusal::OverLimit)
        });
    match admitted {
        Ok(import) => ImportVerdict::Admitted(import),
        Err(refusal) => ImportVerdict::Refused(refusal),
    }
}

/// The repo GitHub served, or why it could not be read.
fn found(read: RepoRead<'_>) -> Result<&RepoFacts, ImportRefusal> {
    match read {
        RepoRead::Found(facts) => Ok(facts),
        RepoRead::Gone => Err(ImportRefusal::NotFound),
        RepoRead::Unfinished => Err(ImportRefusal::Unreachable),
    }
}

fn not_owned<'a>(facts: &'a RepoFacts, listed: &[u64]) -> Result<&'a RepoFacts, ImportRefusal> {
    if listed.contains(&facts.owner_id) {
        Err(ImportRefusal::OwnedByListedAccount)
    } else {
        Ok(facts)
    }
}

fn not_yet_imported<'a>(
    facts: &'a RepoFacts,
    imports: &[u64],
) -> Result<&'a RepoFacts, ImportRefusal> {
    if imports.contains(&facts.repo_id) {
        Err(ImportRefusal::AlreadyImported)
    } else {
        Ok(facts)
    }
}

/// The import a public repo read from GitHub becomes, named as GitHub
/// serves it now.
pub fn import_of(facts: &RepoFacts) -> ImportedRepo {
    ImportedRepo {
        repo_id: facts.repo_id,
        full_name: facts.full_name.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::{import_shape, shown_import_name};

    #[test]
    fn a_github_link_names_the_same_repo_as_owner_slash_name() {
        for typed in [
            "rust-lang/cargo",
            " rust-lang/cargo ",
            "https://github.com/rust-lang/cargo",
            "http://github.com/rust-lang/cargo",
            "https://www.github.com/rust-lang/cargo/",
            "github.com/rust-lang/cargo",
            "www.github.com/rust-lang/cargo",
            "https://github.com/rust-lang/cargo.git",
            "HTTPS://GitHub.com/rust-lang/cargo",
            "https://github.com/rust-lang/cargo?tab=readme-ov-file",
            "https://github.com/rust-lang/cargo#readme",
        ] {
            assert_eq!(
                import_shape(typed),
                Some(("rust-lang", "cargo")),
                "{typed:?}"
            );
            assert_eq!(shown_import_name(typed), "rust-lang/cargo", "{typed:?}");
        }
    }

    #[test]
    fn a_link_that_is_not_a_github_repo_has_no_shape() {
        for typed in [
            "https://github.com/rust-lang",
            "https://github.com/rust-lang/",
            "https://github.com/rust-lang/cargo/issues",
            "https://github.com/rust-lang/cargo/tree/master",
            "https://gitlab.com/rust-lang/cargo",
            "https://github.com.evil.example/rust-lang/cargo",
            "https://github.com//cargo",
            "ftp://github.com/rust-lang/cargo",
            "github.com",
            "https://",
        ] {
            assert_eq!(import_shape(typed), None, "{typed:?}");
            assert_eq!(shown_import_name(typed), typed.trim(), "{typed:?}");
        }
    }
}
