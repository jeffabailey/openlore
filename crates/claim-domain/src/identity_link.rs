//! `identity_link` — the GitHub ↔ DID identity link, decided PURELY
//! (ADR-084 wire contract, ADR-085 verification and reader semantics).
//!
//! This module is the single home of the link's rules: the bio proof (the
//! DID-token matcher hoisted verbatim from `review-domain::ownership`,
//! ADR-085 §1), the recorded check verdicts (ADR-085 §4) and the link
//! predicate readers select candidate rows with (Q-RM-4/Q-RM-5: the predicate
//! literal appears nowhere else).

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde_json::Value;

use crate::{
    bare_did, claim_record_json, is_self_retracted, is_superseded_by_author, ClaimLineage,
    Confidence, Did, Provenance, UnsignedClaim,
};

/// The reserved predicate of an identity-link claim (ADR-084 §2).
const LINK_PREDICATE: &str = "sameAs";

/// The predicate readers select candidate link rows with. Selection is only
/// a pre-filter: what is a link is decided by the projection, never by the
/// predicate alone (Q-RM-5).
pub fn link_predicate() -> &'static str {
    LINK_PREDICATE
}

// =============================================================================
// Bio proof (ADR-076 §4, hoisted by ADR-085 §1)
// =============================================================================

/// Characters a bio token is made of: maximal runs of `[A-Za-z0-9._:%-]`.
fn is_token_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | ':' | '%' | '-')
}

/// Sentence punctuation stripped from the end of a token (`(did:…).`).
fn is_trailing_punctuation(c: char) -> bool {
    matches!(c, '.' | ',' | ';' | ':' | '!' | '?' | ')' | ']' | '}' | '>')
}

/// `did:<lowercase method>:<non-empty rest>`.
fn is_did_shaped(token: &str) -> bool {
    let mut parts = token.splitn(3, ':');
    let scheme = parts.next();
    let method = parts.next().unwrap_or_default();
    let rest = parts.next().unwrap_or_default();
    scheme == Some("did")
        && !method.is_empty()
        && method.chars().all(|c| c.is_ascii_lowercase())
        && !rest.is_empty()
}

/// The DID-shaped tokens of a bio, in order of appearance.
pub fn did_tokens(bio: &str) -> Vec<&str> {
    bio.split(|c: char| !is_token_char(c))
        .map(|run| run.trim_end_matches(is_trailing_punctuation))
        .filter(|token| is_did_shaped(token))
        .collect()
}

/// What a bio says about one DID.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BioVerdict {
    /// The DID is a byte-equal token of the bio.
    Verified,
    /// The bio holds no DID at all.
    DidMissing,
    /// The bio holds DIDs, none of them this one (the first found).
    DifferentDid(String),
    /// There is no public bio (absent or blank).
    NoBio,
}

/// The verdict of a bio against `did`: exact, case-sensitive token equality;
/// several DIDs may appear, only `did` counts.
pub fn bio_verdict(bio: Option<&str>, did: &str) -> BioVerdict {
    let Some(bio) = bio.filter(|b| !b.trim().is_empty()) else {
        return BioVerdict::NoBio;
    };
    let tokens = did_tokens(bio);
    if tokens.contains(&did) {
        BioVerdict::Verified
    } else {
        tokens
            .first()
            .map(|found| BioVerdict::DifferentDid((*found).to_string()))
            .unwrap_or(BioVerdict::DidMissing)
    }
}

// =============================================================================
// Recorded check verdicts (ADR-085 §4)
// =============================================================================

/// The closed set of verdicts a local proof check records (ADR-076 spelling).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CheckVerdict {
    Verified,
    DidMissing,
    DifferentDid,
    NoBio,
    AccountNotFound,
}

impl CheckVerdict {
    /// Every recorded verdict.
    pub const ALL: [CheckVerdict; 5] = [
        CheckVerdict::Verified,
        CheckVerdict::DidMissing,
        CheckVerdict::DifferentDid,
        CheckVerdict::NoBio,
        CheckVerdict::AccountNotFound,
    ];

    /// The stored token of the verdict.
    pub fn token(self) -> &'static str {
        match self {
            CheckVerdict::Verified => "verified",
            CheckVerdict::DidMissing => "did_missing",
            CheckVerdict::DifferentDid => "different_did",
            CheckVerdict::NoBio => "no_bio",
            CheckVerdict::AccountNotFound => "account_not_found",
        }
    }

    /// The verdict a stored token names, if any.
    pub fn from_token(token: &str) -> Option<CheckVerdict> {
        CheckVerdict::ALL
            .into_iter()
            .find(|verdict| verdict.token() == token)
    }
}

// =============================================================================
// Proof check over a fresh GitHub read (ADR-085 §2, §4)
// =============================================================================

/// One fresh read of a GitHub profile: the login GitHub reported, the stable
/// numeric id and the public bio.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GithubObservation {
    pub login: String,
    pub user_id: u64,
    pub bio: Option<String>,
}

/// A link whose bio proof passed on a fresh read. Built only by
/// [`check_link`]: holding one is the capability to compose a link.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProvenLink {
    author: Did,
    login: String,
    user_id: u64,
}

impl ProvenLink {
    /// The DID the bio names: the author's bare DID.
    pub fn did(&self) -> &str {
        bare_did(&self.author.0)
    }

    /// The login GitHub reported on the proving read.
    pub fn login(&self) -> &str {
        &self.login
    }

    pub fn user_id(&self) -> u64 {
        self.user_id
    }
}

/// What one proof check decided.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProofCheck {
    /// The bio names the author's DID: the link is proven.
    Verified(ProvenLink),
    /// The bio does not prove the link.
    Disproved {
        verdict: CheckVerdict,
        login_seen: String,
        user_id: u64,
    },
}

impl ProofCheck {
    /// The verdict to record.
    pub fn verdict(&self) -> CheckVerdict {
        match self {
            ProofCheck::Verified(_) => CheckVerdict::Verified,
            ProofCheck::Disproved { verdict, .. } => *verdict,
        }
    }

    /// The login GitHub reported on the read.
    pub fn login_seen(&self) -> &str {
        match self {
            ProofCheck::Verified(proven) => proven.login(),
            ProofCheck::Disproved { login_seen, .. } => login_seen,
        }
    }

    /// The account's stable numeric id.
    pub fn user_id(&self) -> u64 {
        match self {
            ProofCheck::Verified(proven) => proven.user_id(),
            ProofCheck::Disproved { user_id, .. } => *user_id,
        }
    }
}

/// The verdict a check records for what the bio says.
fn recorded_verdict(bio: &BioVerdict) -> CheckVerdict {
    match bio {
        BioVerdict::Verified => CheckVerdict::Verified,
        BioVerdict::DidMissing => CheckVerdict::DidMissing,
        BioVerdict::DifferentDid(_) => CheckVerdict::DifferentDid,
        BioVerdict::NoBio => CheckVerdict::NoBio,
    }
}

/// Check `author`'s link to the account observed on a fresh login read
/// (the common case: the login read returned the account itself).
pub fn check_link(author: &Did, observation: &GithubObservation) -> ProofCheck {
    match bio_verdict(observation.bio.as_deref(), bare_did(&author.0)) {
        BioVerdict::Verified => ProofCheck::Verified(ProvenLink {
            author: author.clone(),
            login: observation.login.clone(),
            user_id: observation.user_id,
        }),
        other => ProofCheck::Disproved {
            verdict: recorded_verdict(&other),
            login_seen: observation.login.clone(),
            user_id: observation.user_id,
        },
    }
}

// =============================================================================
// Codec: ProvenLink → LinkDraft → UnsignedClaim (ADR-084 §2)
// =============================================================================

const GITHUB_SUBJECT_PREFIX: &str = "github:";
const PROFILE_URL_PREFIX: &str = "https://github.com/";
const ID_URI_PREFIX: &str = "https://api.github.com/user/";

/// An identity link is asserted at full confidence (ADR-084 §2).
const LINK_CONFIDENCE_BP: i64 = 10_000;

/// The link claim about to be signed or self-attested. Built only from a
/// [`ProvenLink`].
#[derive(Debug, Clone, PartialEq)]
pub struct LinkDraft {
    claim: UnsignedClaim,
}

impl LinkDraft {
    /// The owner's assertion: subject = the author's bare DID, object
    /// `github:<login>`, evidence [profile URL, id URI] in that order.
    pub fn new(proven: &ProvenLink, composed_at: &str) -> Self {
        Self {
            claim: UnsignedClaim {
                subject: proven.did().to_string(),
                predicate: LINK_PREDICATE.to_string(),
                object: format!("{GITHUB_SUBJECT_PREFIX}{}", proven.login),
                evidence: vec![
                    format!("{PROFILE_URL_PREFIX}{}", proven.login),
                    format!("{ID_URI_PREFIX}{}", proven.user_id),
                ],
                confidence: Confidence::from_basis_points(LINK_CONFIDENCE_BP),
                author_did: proven.author.clone(),
                composed_at: composed_at.to_string(),
                references: Vec::new(),
                reason: None,
            },
        }
    }

    /// The exact claim a confirmation would sign and publish.
    pub fn claim(&self) -> &UnsignedClaim {
        &self.claim
    }

    pub fn into_unsigned(self) -> UnsignedClaim {
        self.claim
    }
}

/// The self-attested record JSON of `did`'s link to the observed account,
/// when the observation proves it (the review app's create-only write).
pub fn self_attested_link_record(
    did: &str,
    observation: &GithubObservation,
    composed_at: &str,
) -> Option<Value> {
    match check_link(&Did(did.to_string()), observation) {
        ProofCheck::Verified(proven) => Some(claim_record_json(
            &LinkDraft::new(&proven, composed_at).into_unsigned(),
        )),
        ProofCheck::Disproved { .. } => None,
    }
}

// =============================================================================
// Projection (ADR-084 §3)
// =============================================================================

/// The claim fields the projection reads.
#[derive(Debug, Clone, Copy)]
pub struct ClaimFields<'a> {
    pub author: &'a str,
    pub subject: &'a str,
    pub predicate: &'a str,
    pub object: &'a str,
    pub evidence: &'a [String],
}

/// A projected link: (bare DID, login as signed, numeric id).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct IdentityLink {
    pub did: String,
    pub login: String,
    pub user_id: u64,
}

/// A GitHub login: 1..=39 ASCII alphanumerics or hyphens, not leading `-`
/// (ADR-076). Anything else names no account and never reaches a request.
pub fn is_github_login(login: &str) -> bool {
    (1..=39).contains(&login.len())
        && !login.starts_with('-')
        && login.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
}

/// The numeric id of an id-URI evidence entry; `Err` when it is malformed.
fn id_of_uri(digits: &str) -> Result<u64, ()> {
    if digits.is_empty() || !digits.chars().all(|c| c.is_ascii_digit()) {
        return Err(());
    }
    digits.parse().map_err(|_| ())
}

/// The ids every id-URI evidence entry names; `None` if one is malformed.
fn evidence_ids(evidence: &[String]) -> Option<Vec<u64>> {
    evidence
        .iter()
        .filter_map(|entry| entry.strip_prefix(ID_URI_PREFIX))
        .map(|digits| id_of_uri(digits).ok())
        .collect()
}

/// Does a profile-URL evidence entry name `login` (case-insensitive)?
fn names_profile_of(evidence: &[String], login: &str) -> bool {
    evidence.iter().any(|entry| {
        entry
            .strip_prefix(PROFILE_URL_PREFIX)
            .is_some_and(|named| named.eq_ignore_ascii_case(login))
    })
}

/// A claim whose provenance passed is a link iff: predicate `sameAs`, bare
/// author == subject, object `github:<valid login>`, exactly one id URI and
/// a profile URL naming the same login.
pub fn project(claim: ClaimFields<'_>, _provenance: &Provenance) -> Option<IdentityLink> {
    let login = claim.object.strip_prefix(GITHUB_SUBJECT_PREFIX)?;
    let ids = evidence_ids(claim.evidence)?;
    let is_link = claim.predicate == LINK_PREDICATE
        && bare_did(claim.author) == claim.subject
        && is_github_login(login)
        && ids.len() == 1
        && names_profile_of(claim.evidence, login);
    is_link.then(|| IdentityLink {
        did: claim.subject.to_string(),
        login: login.to_string(),
        user_id: ids[0],
    })
}

// =============================================================================
// Reader view (ADR-085 §5)
// =============================================================================

/// This reader's latest check of one link.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LatestCheck {
    pub subject_did: String,
    pub user_id: u64,
    pub verdict: CheckVerdict,
    pub checked_at: DateTime<Utc>,
}

/// How a reader shows one link.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkStanding {
    /// The sole DID this reader verified for the account.
    Associated { checked_at: DateTime<Utc> },
    /// An assertion this reader never checked.
    NotChecked,
    /// An assertion whose latest check failed.
    ProofGone {
        verdict: CheckVerdict,
        checked_at: DateTime<Utc>,
    },
    /// Several DIDs are verified for the account; none is associated.
    Contested,
}

/// One link as a reader shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkedIdentity {
    pub link: IdentityLink,
    pub standing: LinkStanding,
}

/// The latest check of `link`, if this reader checked it.
fn latest_for<'a>(link: &IdentityLink, latest: &'a [LatestCheck]) -> Option<&'a LatestCheck> {
    latest
        .iter()
        .find(|check| check.subject_did == link.did && check.user_id == link.user_id)
}

/// How many links are verified per account id.
fn verified_per_account(links: &[IdentityLink], latest: &[LatestCheck]) -> BTreeMap<u64, usize> {
    links
        .iter()
        .filter(|link| {
            latest_for(link, latest).is_some_and(|c| c.verdict == CheckVerdict::Verified)
        })
        .fold(BTreeMap::new(), |mut counts, link| {
            *counts.entry(link.user_id).or_insert(0) += 1;
            counts
        })
}

/// The association rule: associated iff this reader's latest check is
/// Verified and no other DID is Verified for the same account.
pub fn link_view(links: &[IdentityLink], latest: &[LatestCheck]) -> Vec<LinkedIdentity> {
    let distinct: Vec<IdentityLink> = links
        .iter()
        .map(|link| ((link.did.clone(), link.user_id), link.clone()))
        .collect::<BTreeMap<_, _>>()
        .into_values()
        .collect();
    let verified = verified_per_account(&distinct, latest);
    distinct
        .into_iter()
        .map(|link| {
            let standing = match latest_for(&link, latest) {
                None => LinkStanding::NotChecked,
                Some(check) if check.verdict != CheckVerdict::Verified => LinkStanding::ProofGone {
                    verdict: check.verdict,
                    checked_at: check.checked_at,
                },
                Some(_) if verified.get(&link.user_id).copied().unwrap_or(0) >= 2 => {
                    LinkStanding::Contested
                }
                Some(check) => LinkStanding::Associated {
                    checked_at: check.checked_at,
                },
            };
            LinkedIdentity { link, standing }
        })
        .collect()
}

// =============================================================================
// Link lifecycle (ADR-085 §5): same-author lineage, D-a / D-g / A-4
// =============================================================================

/// One stored claim that may be a link: its fields and its place in the
/// reference graph.
#[derive(Debug, Clone, Copy)]
pub struct StoredClaim<'a> {
    pub fields: ClaimFields<'a>,
    pub lineage: ClaimLineage<'a>,
}

/// The links still standing: every stored link its own author has neither
/// retracted nor superseded (the shared claim-domain withdrawal rules, read
/// against `referrers`, the claims that reference a stored link).
pub fn active_links(
    stored: &[StoredClaim<'_>],
    referrers: &[ClaimLineage<'_>],
) -> Vec<IdentityLink> {
    let provenance = Provenance::AppSigned {
        key_id: String::new(),
    };
    stored
        .iter()
        .filter(|claim| {
            !is_self_retracted(&claim.lineage, referrers)
                && !is_superseded_by_author(&claim.lineage, referrers)
        })
        .filter_map(|claim| project(claim.fields, &provenance))
        .collect()
}

/// D-a: the proven account is already actively linked to the author's DID,
/// so linking it again is refused. A retracted link no longer counts (D-g),
/// and other accounts of the same DID never do (A-4).
pub fn is_already_linked(proven: &ProvenLink, active: &[IdentityLink]) -> bool {
    active
        .iter()
        .any(|link| link.did == proven.did() && link.user_id == proven.user_id())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_check_verdict_round_trips_through_its_token() {
        for verdict in CheckVerdict::ALL {
            assert_eq!(CheckVerdict::from_token(verdict.token()), Some(verdict));
        }
        assert_eq!(CheckVerdict::from_token("identity_changed"), None);
    }
}
