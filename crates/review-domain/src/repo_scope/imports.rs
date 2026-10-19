//! "Add repo" (CORE-RRA-9, US-RRA-005): a public repo the owner doesn't
//! own, admitted or refused for one named cause (DWD-RRA-4). PURE.

use ports::claim_domain::authority::RepoRead;
use ports::{ImportedRepo, RepoFacts};

use super::limits::{admit_import, LimitRefusal, ScopeAccount};

/// The `owner/name` an owner typed into "Add repo", when it has that shape.
pub fn import_shape(typed: &str) -> Option<(&str, &str)> {
    let (owner, name) = typed.trim().split_once('/')?;
    let part = |s: &str| {
        !s.is_empty()
            && s.chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
    };
    (part(owner) && part(name)).then_some((owner, name))
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
