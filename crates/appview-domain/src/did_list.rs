//! `did_list` — the operator's repo-DID list read as a whole (ADR-081).
//!
//! ONE pure, total read shared by the `OPENLORE_INDEXER_REPO_DIDS` variable and
//! the `OPENLORE_INDEXER_REPO_DIDS_FILE` each in-`serve` pass reads afresh:
//! entries are separated by commas or any whitespace (so CRLF line endings are
//! tolerated), duplicates collapse to their first occurrence, and the FIRST
//! entry that is not a repo DID refuses the whole list. A byte-order mark is
//! not whitespace, so it makes the first entry bad and the list fails loud —
//! it never silently drops a DID.
//!
//! NO I/O: the effect shell reads the text and names the variable.

use claim_domain::Did;

/// The longest DID a list may carry, in bytes.
pub const MAX_DID_LENGTH: usize = 2048;

/// The DID methods that name a repo.
const REPO_DID_METHODS: [&str; 2] = ["plc", "web"];

/// What is wrong with a list entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryProblem {
    /// Not shaped `did:<method>:<identifier>`.
    NotADid,
    /// DID-shaped but breaking the DID syntax (method, identifier, length).
    NotWellFormed,
    /// A well-formed DID of a method that names no repo.
    NamesNoRepo,
}

impl EntryProblem {
    /// The operator-facing description of the problem.
    pub const fn describe(self) -> &'static str {
        match self {
            Self::NotADid => "is not a DID (did:<method>:<identifier>)",
            Self::NotWellFormed => "is not a well-formed DID",
            Self::NamesNoRepo => "names no repo: only did:plc and did:web DIDs do",
        }
    }
}

/// The first entry that refused a list, and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BadEntry {
    pub entry: String,
    pub problem: EntryProblem,
}

/// The distinct repo DIDs of `text`, in first-seen order, or the first bad entry.
pub fn read_did_list(text: &str) -> Result<Vec<Did>, BadEntry> {
    entries_of(text)
        .map(repo_did)
        .try_fold(Vec::new(), |distinct, did| {
            did.map(|did| with_new(distinct, did))
        })
}

/// The non-empty entries of a comma- or whitespace-separated list.
fn entries_of(text: &str) -> impl Iterator<Item = &str> {
    text.split(|c: char| c == ',' || c.is_whitespace())
        .filter(|entry| !entry.is_empty())
}

/// `distinct` with `did` appended unless already present.
fn with_new(mut distinct: Vec<Did>, did: Did) -> Vec<Did> {
    if !distinct.contains(&did) {
        distinct.push(did);
    }
    distinct
}

/// One list entry as a repo DID: ATProto DID syntax, method `plc` or `web`.
fn repo_did(entry: &str) -> Result<Did, BadEntry> {
    entry_problem(entry).map_or_else(
        || Ok(Did(entry.to_string())),
        |problem| {
            Err(BadEntry {
                entry: entry.to_string(),
                problem,
            })
        },
    )
}

fn entry_problem(entry: &str) -> Option<EntryProblem> {
    let Some((method, identifier)) = entry
        .strip_prefix("did:")
        .and_then(|rest| rest.split_once(':'))
    else {
        return Some(EntryProblem::NotADid);
    };
    if entry.len() > MAX_DID_LENGTH
        || method.is_empty()
        || !method.chars().all(|c| c.is_ascii_lowercase())
        || !did_identifier_valid(identifier)
    {
        Some(EntryProblem::NotWellFormed)
    } else if !REPO_DID_METHODS.contains(&method) {
        Some(EntryProblem::NamesNoRepo)
    } else {
        None
    }
}

/// `[A-Za-z0-9._:%-]+`, not ending in `:` (so no `#fragment`, no empty id).
fn did_identifier_valid(identifier: &str) -> bool {
    !identifier.is_empty()
        && !identifier.ends_with(':')
        && identifier
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | ':' | '%' | '-'))
}
