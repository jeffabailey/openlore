//! `purge_plan` — which indexed authors a pass purges (ADR-082, data-models §3).
//!
//! A removal is an operator decision: an author the index holds but the
//! loaded, non-empty DID list no longer names. The plan is a pure set
//! difference over BARE DIDs, so a listed DID is never planned whatever its
//! fetch outcome (skips never delete, ADR-078) and an empty list holds the
//! whole purge back (an SSM blip never empties the index). A refused list
//! never reaches this function: the caller has only a loaded list to pass.
//!
//! NO I/O.

use std::collections::BTreeSet;

use claim_domain::Did;

/// An author as the index groups them: the text of an `author_did` before
/// the first `#` (`did:plc:x#org.openlore.application` and `did:plc:x` are
/// the same author).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BareDid(pub String);

impl BareDid {
    /// The bare DID of a stored `author_did` (or of a listed DID).
    #[must_use]
    pub fn of(author_did: &str) -> Self {
        Self(
            author_did
                .split('#')
                .next()
                .unwrap_or(author_did)
                .to_string(),
        )
    }
}

/// Why a pass purges nobody although purging is enabled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PurgeSuppressed {
    /// The list loaded but named no DID.
    EmptyList,
}

impl PurgeSuppressed {
    /// The `reason` token of `indexer.ingest.purge_suppressed`.
    #[must_use]
    pub const fn token(self) -> &'static str {
        match self {
            Self::EmptyList => "empty_list",
        }
    }
}

/// What a pass purges.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PurgePlan {
    /// Purge exactly these authors (possibly none).
    Purge(BTreeSet<BareDid>),
    /// Purge nobody, for this reason.
    Suppressed(PurgeSuppressed),
}

/// The authors to purge: every indexed author (by bare DID) the loaded list
/// no longer names; nobody when the list is empty.
pub fn plan_purge<'a>(
    listed: &[Did],
    indexed_authors: impl IntoIterator<Item = &'a str>,
) -> PurgePlan {
    if listed.is_empty() {
        return PurgePlan::Suppressed(PurgeSuppressed::EmptyList);
    }
    let still_listed: BTreeSet<BareDid> = listed.iter().map(|did| BareDid::of(&did.0)).collect();
    PurgePlan::Purge(
        indexed_authors
            .into_iter()
            .map(BareDid::of)
            .filter(|author| !still_listed.contains(author))
            .collect(),
    )
}
