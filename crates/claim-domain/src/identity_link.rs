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

/// Is `predicate` reserved for identity links? Exact, case-sensitive match:
/// generic composers refuse it and point to `openlore identity link`
/// (ADR-084 §4); `claim counter` never asks (D-b).
pub fn is_reserved_predicate(predicate: &str) -> bool {
    predicate == LINK_PREDICATE
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

/// Did the record's provenance pass for `author` itself: signed with the
/// author's own key, or self-attested from the author's own repo (ADR-071)?
fn provenance_passed_for(author: &str, provenance: &Provenance) -> bool {
    let attesting_did = match provenance {
        Provenance::AppSigned { key_id } => bare_did(key_id),
        Provenance::SelfAttested { repo_did } => bare_did(&repo_did.0),
    };
    attesting_did == bare_did(author)
}

/// A claim is a link iff (ADR-084 §3): its provenance passed for its author,
/// predicate `sameAs`, bare author == subject, object `github:<valid login>`,
/// exactly one id URI and a profile URL naming the same login.
pub fn project(claim: ClaimFields<'_>, provenance: &Provenance) -> Option<IdentityLink> {
    let login = claim.object.strip_prefix(GITHUB_SUBJECT_PREFIX)?;
    let ids = evidence_ids(claim.evidence)?;
    let is_link = provenance_passed_for(claim.author, provenance)
        && claim.predicate == LINK_PREDICATE
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

/// One check this reader recorded for a link (insert-only, ADR-085 §7): the
/// latest one not dated after `now` decides (Q-RM-3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LatestCheck {
    pub subject_did: String,
    pub user_id: u64,
    /// The login GitHub reported on that check (a rename shows here).
    pub login_seen: String,
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

/// Where a stored link stands in its author's own lineage (ADR-060).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkLifecycle {
    Active,
    Retracted,
    Superseded,
}

/// A projected link with its lifecycle, as a reader holds it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReaderLink {
    pub link: IdentityLink,
    pub lifecycle: LinkLifecycle,
}

/// The check of `link` that decides as of `now`: the latest recorded one not
/// dated in the future (Q-RM-3); among equal instants the last recorded.
fn latest_for<'a>(
    link: &IdentityLink,
    checks: &'a [LatestCheck],
    now: DateTime<Utc>,
) -> Option<&'a LatestCheck> {
    checks
        .iter()
        .filter(|check| check.subject_did == link.did && check.user_id == link.user_id)
        .filter(|check| check.checked_at <= now)
        .max_by_key(|check| check.checked_at)
}

/// How many active links are verified per account id. Unchecked or
/// disproved links never count (no griefing by an unproven claimant).
fn verified_per_account(
    links: &[IdentityLink],
    checks: &[LatestCheck],
    now: DateTime<Utc>,
) -> BTreeMap<u64, usize> {
    links
        .iter()
        .filter(|link| {
            latest_for(link, checks, now).is_some_and(|c| c.verdict == CheckVerdict::Verified)
        })
        .fold(BTreeMap::new(), |mut counts, link| {
            *counts.entry(link.user_id).or_insert(0) += 1;
            counts
        })
}

/// The active links, once per (DID, account id).
fn distinct_active(links: &[ReaderLink]) -> Vec<IdentityLink> {
    links
        .iter()
        .filter(|reader_link| reader_link.lifecycle == LinkLifecycle::Active)
        .map(|reader_link| {
            let link = &reader_link.link;
            ((link.did.clone(), link.user_id), link.clone())
        })
        .collect::<BTreeMap<_, _>>()
        .into_values()
        .collect()
}

/// The association rule (ADR-085 §5) as of `now`: only active links are
/// shown; one is associated iff this reader's deciding check is Verified and
/// no other active DID is Verified for the same account (else Contested).
pub fn link_view(
    links: &[ReaderLink],
    checks: &[LatestCheck],
    now: DateTime<Utc>,
) -> Vec<LinkedIdentity> {
    let distinct = distinct_active(links);
    let verified = verified_per_account(&distinct, checks, now);
    distinct
        .into_iter()
        .map(|link| {
            let deciding = latest_for(&link, checks, now);
            let standing = match deciding {
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
            LinkedIdentity {
                link: under_login_last_seen(link, deciding),
                standing,
            }
        })
        .collect()
}

/// `link` under the login its deciding check saw: after a rename the account
/// shows under its new name; an unchecked link keeps the signed one.
fn under_login_last_seen(link: IdentityLink, deciding: Option<&LatestCheck>) -> IdentityLink {
    match deciding {
        Some(check) => IdentityLink {
            login: check.login_seen.clone(),
            ..link
        },
        None => link,
    }
}

// =============================================================================
// Link lifecycle (ADR-085 §5): same-author lineage, D-a / D-g / A-4
// =============================================================================

/// One stored claim that may be a link: its fields, the provenance it was
/// stored under and its place in the reference graph.
#[derive(Debug, Clone, Copy)]
pub struct StoredClaim<'a> {
    pub fields: ClaimFields<'a>,
    pub provenance: &'a Provenance,
    pub lineage: ClaimLineage<'a>,
}

/// Where `claim` stands in its own author's lineage (the shared claim-domain
/// withdrawal rules, read against `referrers`, the claims referencing it).
fn lifecycle_of(claim: &StoredClaim<'_>, referrers: &[ClaimLineage<'_>]) -> LinkLifecycle {
    if is_self_retracted(&claim.lineage, referrers) {
        LinkLifecycle::Retracted
    } else if is_superseded_by_author(&claim.lineage, referrers) {
        LinkLifecycle::Superseded
    } else {
        LinkLifecycle::Active
    }
}

/// The links still standing: every stored link its own author has neither
/// retracted nor superseded.
pub fn active_links(
    stored: &[StoredClaim<'_>],
    referrers: &[ClaimLineage<'_>],
) -> Vec<IdentityLink> {
    stored
        .iter()
        .filter(|claim| lifecycle_of(claim, referrers) == LinkLifecycle::Active)
        .filter_map(|claim| project(claim.fields, claim.provenance))
        .collect()
}

/// Which links one re-check covers (`identity verify --did` / `--github`).
/// A scope only selects links; it never changes a verdict.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecheckScope {
    /// Every active link.
    Everyone,
    /// The links of one DID (compared bare).
    Person(String),
    /// The links of one GitHub account, by login (case-insensitive, as
    /// signed or as last seen).
    Account(String),
}

/// Is `link`, last seen under `login_last_seen`, within `scope`?
pub fn is_in_scope(scope: &RecheckScope, link: &IdentityLink, login_last_seen: &str) -> bool {
    match scope {
        RecheckScope::Everyone => true,
        RecheckScope::Person(did) => bare_did(did) == link.did,
        RecheckScope::Account(login) => {
            login.eq_ignore_ascii_case(&link.login) || login.eq_ignore_ascii_case(login_last_seen)
        }
    }
}

/// The links one re-check covers: every active link, once per
/// (DID, numeric id), so each account is read once (ADR-085 §7).
pub fn links_to_recheck(
    stored: &[StoredClaim<'_>],
    referrers: &[ClaimLineage<'_>],
) -> Vec<IdentityLink> {
    active_links(stored, referrers)
        .into_iter()
        .map(|link| ((link.did.clone(), link.user_id), link))
        .collect::<BTreeMap<_, _>>()
        .into_values()
        .collect()
}

/// One GitHub answer as the check consumes it: the profile, "no such
/// account", or a failure (`E`) that records nothing (ADR-085 §4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GithubRead<E> {
    Profile(GithubObservation),
    NotFound,
    Unreadable(E),
}

/// The account was not found under its login nor by its number.
fn account_not_found(link: &IdentityLink, login_read: &str) -> ProofCheck {
    ProofCheck::Disproved {
        verdict: CheckVerdict::AccountNotFound,
        login_seen: login_read.to_string(),
        user_id: link.user_id,
    }
}

/// The bio of the linked account decides, against the link's own DID. An
/// answer for any other account does not.
fn check_linked_account(
    link: &IdentityLink,
    login_read: &str,
    observed: &GithubObservation,
) -> ProofCheck {
    if observed.user_id == link.user_id {
        check_link(&Did(link.did.clone()), observed)
    } else {
        account_not_found(link, login_read)
    }
}

/// Re-check an active `link` over at most two reads (ADR-085 §4). The login
/// read (`login_read`, the last login seen) answering the linked account
/// decides on its bio. A login that is gone or now names another account
/// (renamed, maybe re-registered) asks `read_by_account_number` — the second
/// and last read: the account's current profile decides, under its new
/// login; no account under that number is `account_not_found`. Any
/// unreadable answer records nothing: the failure is returned.
pub fn recheck_link<E>(
    link: &IdentityLink,
    login_read: &str,
    login_answer: GithubRead<E>,
    read_by_account_number: impl FnOnce(u64) -> GithubRead<E>,
) -> Result<ProofCheck, E> {
    match login_answer {
        GithubRead::Unreadable(failure) => Err(failure),
        GithubRead::Profile(observed) if observed.user_id == link.user_id => {
            Ok(check_link(&Did(link.did.clone()), &observed))
        }
        GithubRead::Profile(_) | GithubRead::NotFound => {
            match read_by_account_number(link.user_id) {
                GithubRead::Profile(current) => {
                    Ok(check_linked_account(link, login_read, &current))
                }
                GithubRead::NotFound => Ok(account_not_found(link, login_read)),
                GithubRead::Unreadable(failure) => Err(failure),
            }
        }
    }
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
