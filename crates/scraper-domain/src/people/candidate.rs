//! The inference value types: a signed repo claim as inference sees it, the
//! support it gathers, and the smart-constructed [`PersonCandidate`] (only
//! constructible with non-empty provenance; D-9).

use claim_domain::{ClaimLineage, ClaimReference};
use ports::{AuthorRelationship, FederatedRow};

use super::confidence::{inferred_confidence, Hundredths};
use super::provenance::CitedClaim;
use super::strip_github_prefix;

/// A signed repo claim as inference sees it: the port-level facts of one
/// `FederatedRow`, confidence already in hundredths.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoClaim {
    pub repo_subject: String,
    pub predicate: String,
    pub philosophy: String,
    pub author_did: String,
    pub relationship: AuthorRelationship,
    pub cid: String,
    pub confidence: Hundredths,
    /// The claim's typed references (retracts / counters / supersedes /
    /// corrects) — the reference graph DDD-7 eligibility reads.
    pub references: Vec<ClaimReference>,
}

impl RepoClaim {
    /// The borrowed reference-graph view the shared `claim-domain`
    /// withdrawal rules read.
    pub(super) fn lineage(&self) -> ClaimLineage<'_> {
        ClaimLineage {
            author_did: &self.author_did,
            cid: &self.cid,
            references: &self.references,
        }
    }
}

impl From<&FederatedRow> for RepoClaim {
    fn from(row: &FederatedRow) -> Self {
        let unsigned = &row.signed_claim.unsigned;
        Self {
            repo_subject: unsigned.subject.clone(),
            predicate: unsigned.predicate.clone(),
            philosophy: unsigned.object.clone(),
            author_did: row.author_did.0.clone(),
            relationship: row.author_relationship,
            cid: row.signed_claim.signature.signed_cid.0.clone(),
            confidence: Hundredths::floor_of(unsigned.confidence.value()),
            references: unsigned.references.clone(),
        }
    }
}

/// One repo supporting an inference: the person's rank there and the eligible
/// signed claims (each with its own author) that support the philosophy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SupportingRepo {
    pub repo_subject: String,
    pub rank: u32,
    pub claims: Vec<SupportingClaim>,
}

/// A cited supporting claim plus the confidence its author signed.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct SupportingClaim {
    pub cited: CitedClaim,
    pub confidence: Hundredths,
}

/// Why a [`PersonCandidate`] could not be built.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CandidateError {
    /// No supporting repo, or a supporting repo citing no claim: an inference
    /// without provenance is never proposed (D-9).
    EmptyProvenance,
}

/// "`person` adheres to `philosophy`", proposed from signed repo claims. Only
/// constructible with non-empty provenance; support is kept in canonical
/// (ADR-064 §3) order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PersonCandidate {
    person_subject: String,
    philosophy: String,
    support: Vec<SupportingRepo>,
    confidence: Hundredths,
}

impl PersonCandidate {
    /// Smart constructor: rejects empty provenance; canonicalizes support
    /// order; derives the DDD-9 confidence.
    pub fn new(
        person_subject: &str,
        philosophy: &str,
        support: Vec<SupportingRepo>,
    ) -> Result<Self, CandidateError> {
        let hollow = support.is_empty() || support.iter().any(|repo| repo.claims.is_empty());
        if hollow {
            return Err(CandidateError::EmptyProvenance);
        }
        let support = canonical_support_order(support);
        let max_supporting = max_claim_confidence(&support);
        Ok(Self {
            person_subject: person_subject.to_string(),
            philosophy: philosophy.to_string(),
            confidence: inferred_confidence(support.len(), max_supporting),
            support,
        })
    }

    /// `github:<login>`.
    pub fn person_subject(&self) -> &str {
        &self.person_subject
    }

    /// The philosophy id (claim object).
    pub fn philosophy(&self) -> &str {
        &self.philosophy
    }

    /// Supporting repos in ADR-064 order.
    pub fn support(&self) -> &[SupportingRepo] {
        &self.support
    }

    /// The proposed (DDD-9) confidence.
    pub fn confidence(&self) -> Hundredths {
        self.confidence
    }

    /// The strongest supporting claim's confidence.
    pub fn max_supporting_confidence(&self) -> Hundredths {
        max_claim_confidence(&self.support)
    }

    /// The GitHub login of the person.
    pub fn login(&self) -> &str {
        strip_github_prefix(&self.person_subject)
    }
}

/// ADR-064 §3 order: repos by case-folded subject; claims within a repo by
/// (author, cid). Exact duplicates collapse.
fn canonical_support_order(support: Vec<SupportingRepo>) -> Vec<SupportingRepo> {
    let mut repos: Vec<SupportingRepo> = support
        .into_iter()
        .map(|mut repo| {
            repo.claims.sort();
            repo.claims.dedup();
            repo
        })
        .collect();
    repos.sort_by(|a, b| {
        (a.repo_subject.to_ascii_lowercase(), &a.repo_subject, a.rank).cmp(&(
            b.repo_subject.to_ascii_lowercase(),
            &b.repo_subject,
            b.rank,
        ))
    });
    repos
}

fn max_claim_confidence(support: &[SupportingRepo]) -> Hundredths {
    support
        .iter()
        .flat_map(|repo| repo.claims.iter().map(|c| c.confidence))
        .max()
        .unwrap_or(Hundredths::new(0))
}
