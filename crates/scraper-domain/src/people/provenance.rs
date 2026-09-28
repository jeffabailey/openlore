//! ADR-064 §3/§5 provenance: a person candidate's supporting claims encoded
//! as `evidence[]` AT-URIs (plus each repo's commits URL), and parsed back.

use std::collections::BTreeSet;

use claim_domain::bare_did;

use super::candidate::PersonCandidate;
use super::strip_github_prefix;

/// One supporting claim, cited by its author (bare DID) and CID (D-7 / D-9).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CitedClaim {
    pub author_did: String,
    pub cid: String,
}

impl CitedClaim {
    /// Cite a claim; the author DID is stored bare (fragment stripped) by the
    /// SAME rule `claim publish` mints at-uris with (Q-CPI-D3).
    pub fn new(author_did: &str, cid: &str) -> Self {
        Self {
            author_did: bare_did(author_did).to_string(),
            cid: cid.to_string(),
        }
    }

    /// ADR-064 §3 canonical AT-URI of the cited claim.
    pub fn at_uri(&self) -> String {
        format!("at://{}/{CLAIM_COLLECTION}/{}", self.author_did, self.cid)
    }
}

const CLAIM_COLLECTION: &str = "org.openlore.claim";

/// ADR-064 §3: the candidate's provenance as `evidence[]` strings.
pub fn encode_provenance(candidate: &PersonCandidate) -> Vec<String> {
    candidate
        .support()
        .iter()
        .flat_map(|repo| {
            repo.claims
                .iter()
                .map(|claim| claim.cited.at_uri())
                .chain(std::iter::once(commits_url(
                    &repo.repo_subject,
                    candidate.login(),
                )))
        })
        .collect()
}

/// The person-specific public source for one repo (ADR-064 §3).
fn commits_url(repo_subject: &str, login: &str) -> String {
    format!(
        "https://github.com/{}/commits?author={login}",
        strip_github_prefix(repo_subject)
    )
}

/// ADR-064 §5: the claims an `evidence[]` cites — every entry that parses as
/// `at://<did>/org.openlore.claim/<cid>`, in order.
pub fn parse_provenance(evidence: &[String]) -> Vec<CitedClaim> {
    evidence
        .iter()
        .filter_map(|entry| parse_claim_at_uri(entry))
        .collect()
}

/// `at://<did>/org.openlore.claim/<cid>` → the cited claim, or `None`.
fn parse_claim_at_uri(entry: &str) -> Option<CitedClaim> {
    let (author_did, tail) = entry.strip_prefix("at://")?.split_once('/')?;
    let cid = tail.strip_prefix(CLAIM_COLLECTION)?.strip_prefix('/')?;
    let well_formed = !author_did.is_empty() && !cid.is_empty() && !cid.contains('/');
    well_formed.then(|| CitedClaim::new(author_did, cid))
}

/// The distinct CIDs of the claims an `evidence[]` cites (ADR-064 §5).
pub(super) fn cited_cids(evidence: &[String]) -> BTreeSet<String> {
    parse_provenance(evidence)
        .into_iter()
        .map(|cited| cited.cid)
        .collect()
}
