//! `identity_link` — the GitHub ↔ DID identity link, decided PURELY
//! (ADR-084 wire contract, ADR-085 verification and reader semantics).
//!
//! This module is the single home of the link's rules: the bio proof (the
//! DID-token matcher hoisted verbatim from `review-domain::ownership`,
//! ADR-085 §1), the recorded check verdicts (ADR-085 §4) and the link
//! predicate readers select candidate rows with (Q-RM-4/Q-RM-5: the predicate
//! literal appears nowhere else).

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::num::NonZeroU64;

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

/// A GitHub answer that names no account: no positive account number or no
/// valid login. It is unreadable — it proves nothing and records nothing
/// (ADR-085 §4, fail closed).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MalformedObservation;

impl fmt::Display for MalformedObservation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("GitHub answered without a valid account number and login")
    }
}

/// The account a GitHub answer names: its positive number and valid login.
/// Account 0 or an invalid login names no account (ADR-085 §4, fail
/// closed). The one account rule, shared with the review app's ownership
/// proof.
pub fn account_named_by(
    user_id: u64,
    login: &str,
) -> Result<(NonZeroU64, &str), MalformedObservation> {
    let user_id = NonZeroU64::new(user_id).ok_or(MalformedObservation)?;
    is_github_login(login)
        .then_some((user_id, login))
        .ok_or(MalformedObservation)
}

/// A link whose bio proof passed on a fresh read. Built only by
/// [`check_link`]: holding one is the capability to compose a link, always
/// for a real account (a positive number, a valid login).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProvenLink {
    author: Did,
    login: String,
    user_id: NonZeroU64,
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
        self.user_id.get()
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
/// (the common case: the login read returned the account itself). An
/// observation naming no account is unreadable: it decides nothing.
pub fn check_link(
    author: &Did,
    observation: &GithubObservation,
) -> Result<ProofCheck, MalformedObservation> {
    let (user_id, login) = account_named_by(observation.user_id, &observation.login)?;
    Ok(
        match bio_verdict(observation.bio.as_deref(), bare_did(&author.0)) {
            BioVerdict::Verified => ProofCheck::Verified(ProvenLink {
                author: author.clone(),
                login: login.to_string(),
                user_id,
            }),
            other => ProofCheck::Disproved {
                verdict: recorded_verdict(&other),
                login_seen: login.to_string(),
            },
        },
    )
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
    match check_link(&Did(did.to_string()), observation).ok()? {
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

impl<'a> ClaimFields<'a> {
    /// The fields of `claim` as the projection reads them.
    pub fn of(claim: &'a UnsignedClaim) -> Self {
        Self {
            author: &claim.author_did.0,
            subject: &claim.subject,
            predicate: &claim.predicate,
            object: &claim.object,
            evidence: &claim.evidence,
        }
    }
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

/// The numeric id of an id-URI evidence entry; `None` when it is malformed
/// or names account 0 (no GitHub account has that number).
fn id_of_uri(digits: &str) -> Option<u64> {
    let all_digits = !digits.is_empty() && digits.chars().all(|c| c.is_ascii_digit());
    all_digits
        .then(|| digits.parse::<NonZeroU64>().ok())
        .flatten()
        .map(NonZeroU64::get)
}

/// The ids every id-URI evidence entry names; `None` if one is malformed.
fn evidence_ids(evidence: &[String]) -> Option<Vec<u64>> {
    evidence
        .iter()
        .filter_map(|entry| entry.strip_prefix(ID_URI_PREFIX))
        .map(id_of_uri)
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

/// Past this many whole days a verified line suggests a re-check (U-8).
pub const RECHECK_AFTER_DAYS: u64 = 30;

/// How old a check is as of `now`, in whole elapsed days: never rounded up,
/// and a check dated after `now` is 0 days old.
pub fn check_age_days(checked_at: DateTime<Utc>, now: DateTime<Utc>) -> u64 {
    u64::try_from((now - checked_at).num_days()).unwrap_or(0)
}

/// Whether a line suggests a re-check as of `now`: only a verified line, and
/// only once its check is more than [`RECHECK_AFTER_DAYS`] whole days old (U-8).
pub fn suggests_recheck(standing: &LinkStanding, now: DateTime<Utc>) -> bool {
    match standing {
        LinkStanding::Associated { checked_at } => {
            check_age_days(*checked_at, now) > RECHECK_AFTER_DAYS
        }
        LinkStanding::NotChecked | LinkStanding::ProofGone { .. } | LinkStanding::Contested => {
            false
        }
    }
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

/// `links` once per (DID, account id), ordered by that key; of duplicates
/// the last one stays.
fn once_per_link(links: impl IntoIterator<Item = IdentityLink>) -> Vec<IdentityLink> {
    links
        .into_iter()
        .map(|link| ((link.did.clone(), link.user_id), link))
        .collect::<BTreeMap<_, _>>()
        .into_values()
        .collect()
}

/// The active links, once per (DID, account id).
fn distinct_active(links: &[ReaderLink]) -> Vec<IdentityLink> {
    once_per_link(
        links
            .iter()
            .filter(|reader_link| reader_link.lifecycle == LinkLifecycle::Active)
            .map(|reader_link| reader_link.link.clone()),
    )
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

/// The login a re-check reads `link` under (ADR-085 §4): the one this
/// reader last saw on its latest check of the link, else the signed one.
pub fn login_to_read<'a>(link: &'a IdentityLink, checks: &'a [LatestCheck]) -> &'a str {
    checks
        .iter()
        .filter(|check| check.subject_did == link.did && check.user_id == link.user_id)
        .max_by_key(|check| check.checked_at)
        .map_or(link.login.as_str(), |check| check.login_seen.as_str())
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
    active_link_records(stored, referrers)
        .into_iter()
        .map(|record| record.link)
        .collect()
}

/// A link as one stored record states it, named by the record's ID (its
/// claim CID).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkRecord {
    pub record_id: String,
    pub link: IdentityLink,
}

/// The records stating a link that its own author has neither retracted nor
/// superseded. A record carrying a `retracts` reference is a retraction,
/// never a link, whatever fields it repeats (U-39).
pub fn active_link_records(
    stored: &[StoredClaim<'_>],
    referrers: &[ClaimLineage<'_>],
) -> Vec<LinkRecord> {
    stored
        .iter()
        .filter(|claim| !claim.lineage.is_retraction())
        .filter(|claim| lifecycle_of(claim, referrers) == LinkLifecycle::Active)
        .filter_map(|claim| {
            project(claim.fields, claim.provenance).map(|link| LinkRecord {
                record_id: claim.lineage.cid.to_string(),
                link,
            })
        })
        .collect()
}

/// The IDs of the records stating each link, grouped by (DID, account id) —
/// never by login (GDL-D9) — each group in ascending record-ID order (U-9).
pub fn record_ids_per_link(records: &[LinkRecord]) -> BTreeMap<(String, u64), Vec<String>> {
    records
        .iter()
        .fold(BTreeMap::new(), |mut groups, record| {
            groups
                .entry((record.link.did.clone(), record.link.user_id))
                .or_insert_with(BTreeSet::new)
                .insert(record.record_id.clone());
            groups
        })
        .into_iter()
        .map(|(key, ids)| (key, ids.into_iter().collect()))
        .collect()
}

/// One shown link and the IDs of every record stating it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListedLink {
    pub identity: LinkedIdentity,
    pub record_ids: Vec<String>,
}

/// Each shown link with the IDs of the records stating it (GDL-D9): a
/// person's duplicate records for one account read as one line.
pub fn listed_links(view: Vec<LinkedIdentity>, records: &[LinkRecord]) -> Vec<ListedLink> {
    let record_ids = record_ids_per_link(records);
    view.into_iter()
        .map(|identity| ListedLink {
            record_ids: record_ids
                .get(&(identity.link.did.clone(), identity.link.user_id))
                .cloned()
                .unwrap_or_default(),
            identity,
        })
        .collect()
}

/// How a person view's row is matched to the shown links (U-6).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PersonKey {
    /// The row carries GitHub account ids: a link joins on the id alone.
    AccountIds(BTreeSet<u64>),
    /// The row carries no account id: a link joins on its last-seen login,
    /// compared case-insensitively (ADR-064).
    Login(String),
}

impl PersonKey {
    /// A row's key: its account ids when it carries any (account 0 names
    /// no account), otherwise its login.
    pub fn of(login: &str, user_ids: impl IntoIterator<Item = u64>) -> Self {
        let ids: BTreeSet<u64> = user_ids.into_iter().filter(|id| *id != 0).collect();
        if ids.is_empty() {
            PersonKey::Login(login.to_string())
        } else {
            PersonKey::AccountIds(ids)
        }
    }

    /// A person row's key from its subject (`github:<login>`) and the
    /// contribution links' `(person subject, account id)` pairs: the ids of
    /// the links naming this person, subjects compared case-insensitively,
    /// else the login with the `github:` prefix (in any case) stripped (U-6).
    pub fn of_person<'a>(
        person_subject: &str,
        contribution_links: impl IntoIterator<Item = (&'a str, u64)>,
    ) -> Self {
        PersonKey::of(
            without_github_prefix(person_subject),
            contribution_links
                .into_iter()
                .filter(|(subject, _)| subject.eq_ignore_ascii_case(person_subject))
                .map(|(_, user_id)| user_id),
        )
    }

    /// Does `link` (under its last-seen login) belong to this person?
    pub fn owns(&self, link: &IdentityLink) -> bool {
        match self {
            PersonKey::AccountIds(ids) => ids.contains(&link.user_id),
            PersonKey::Login(login) => login.eq_ignore_ascii_case(&link.login),
        }
    }
}

/// `subject` without its `github:` prefix, the prefix matched in any case
/// (subjects compare case-insensitively); any other subject as given.
fn without_github_prefix(subject: &str) -> &str {
    subject
        .get(..GITHUB_SUBJECT_PREFIX.len())
        .filter(|prefix| prefix.eq_ignore_ascii_case(GITHUB_SUBJECT_PREFIX))
        .map_or(subject, |prefix| &subject[prefix.len()..])
}

/// The shown links that belong to one person (U-6), in shown order.
pub fn links_of_person(shown: &[ListedLink], person: &PersonKey) -> Vec<ListedLink> {
    shown
        .iter()
        .filter(|listed| person.owns(&listed.identity.link))
        .cloned()
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

/// The shown lines within `scope`: a line is selected when a record stating
/// it is within `scope` under the line's last-seen login. Selection only —
/// every selected line is returned exactly as given, standing included.
pub fn lines_in_scope(
    scope: &RecheckScope,
    lines: Vec<ListedLink>,
    records: &[LinkRecord],
) -> Vec<ListedLink> {
    lines
        .into_iter()
        .filter(|line| {
            let shown = &line.identity.link;
            records.iter().any(|record| {
                record.link.did == shown.did
                    && record.link.user_id == shown.user_id
                    && is_in_scope(scope, &record.link, &shown.login)
            })
        })
        .collect()
}

/// What a listing within `scope` shows as of `now` (`identity list`, F7):
/// every standing is decided over ALL the active link records and ALL of
/// this reader's checks, and only then does the scope select lines — so
/// listing one person never hides that their account is contested.
pub fn listing_in_scope(
    scope: &RecheckScope,
    records: &[LinkRecord],
    checks: &[LatestCheck],
    now: DateTime<Utc>,
) -> Vec<ListedLink> {
    let links: Vec<ReaderLink> = records
        .iter()
        .map(|record| ReaderLink {
            link: record.link.clone(),
            lifecycle: LinkLifecycle::Active,
        })
        .collect();
    lines_in_scope(
        scope,
        listed_links(link_view(&links, checks, now), records),
        records,
    )
}

/// The links one re-check covers: every active link, once per
/// (DID, numeric id), so each account is read once (ADR-085 §7).
pub fn links_to_recheck(
    stored: &[StoredClaim<'_>],
    referrers: &[ClaimLineage<'_>],
) -> Vec<IdentityLink> {
    once_per_link(active_links(stored, referrers))
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
fn account_not_found(login_read: &str) -> ProofCheck {
    ProofCheck::Disproved {
        verdict: CheckVerdict::AccountNotFound,
        login_seen: login_read.to_string(),
    }
}

/// Why a re-check decided nothing and recorded nothing (ADR-085 §4): GitHub
/// failed (`E`), or answered with a profile naming no account.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecheckUnreadable<E> {
    Failed(E),
    Malformed(MalformedObservation),
}

impl<E: fmt::Display> fmt::Display for RecheckUnreadable<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RecheckUnreadable::Failed(failure) => failure.fmt(f),
            RecheckUnreadable::Malformed(malformed) => malformed.fmt(f),
        }
    }
}

/// The bio of the linked account decides, against the link's own DID. An
/// answer for any other account does not; one naming no account is
/// unreadable.
fn check_linked_account<E>(
    link: &IdentityLink,
    login_read: &str,
    observed: &GithubObservation,
) -> Result<ProofCheck, RecheckUnreadable<E>> {
    let (user_id, _) = account_named_by(observed.user_id, &observed.login)
        .map_err(RecheckUnreadable::Malformed)?;
    if user_id.get() == link.user_id {
        check_link(&Did(link.did.clone()), observed).map_err(RecheckUnreadable::Malformed)
    } else {
        Ok(account_not_found(login_read))
    }
}

/// Re-check an active `link` over at most two reads (ADR-085 §4). The login
/// read (`login_read`, the last login seen) answering the linked account
/// decides on its bio. A login that is gone or now names another account
/// (renamed, maybe re-registered) asks `read_by_account_number` — the second
/// and last read: the account's current profile decides, under its new
/// login; no account under that number is `account_not_found`. Any
/// unreadable answer — a GitHub failure, or a profile naming no account —
/// records nothing: the reason is returned.
pub fn recheck_link<E>(
    link: &IdentityLink,
    login_read: &str,
    login_answer: GithubRead<E>,
    read_by_account_number: impl FnOnce(u64) -> GithubRead<E>,
) -> Result<ProofCheck, RecheckUnreadable<E>> {
    match login_answer {
        GithubRead::Unreadable(failure) => Err(RecheckUnreadable::Failed(failure)),
        GithubRead::Profile(observed) if observed.user_id == link.user_id => {
            check_linked_account(link, login_read, &observed)
        }
        GithubRead::Profile(observed)
            if account_named_by(observed.user_id, &observed.login).is_err() =>
        {
            Err(RecheckUnreadable::Malformed(MalformedObservation))
        }
        GithubRead::Profile(_) | GithubRead::NotFound => {
            match read_by_account_number(link.user_id) {
                GithubRead::Profile(current) => check_linked_account(link, login_read, &current),
                GithubRead::NotFound => Ok(account_not_found(login_read)),
                GithubRead::Unreadable(failure) => Err(RecheckUnreadable::Failed(failure)),
            }
        }
    }
}

/// The check one profile already read decides for `link` (UC-8 / U-5): only
/// a profile of the linked account decides, exactly as [`check_link`] does.
/// A profile of any other account, or one naming no account, decides
/// nothing — nothing is recorded and the link's standing is unchanged.
pub fn check_on_profile(link: &IdentityLink, profile: &GithubObservation) -> Option<ProofCheck> {
    (profile.user_id == link.user_id)
        .then(|| check_link(&Did(link.did.clone()), profile).ok())
        .flatten()
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

    const DID: &str = "did:plc:abcdefghijklmnopqrstuvwx";

    fn observed(login: &str, user_id: u64) -> GithubObservation {
        GithubObservation {
            login: login.to_string(),
            user_id,
            bio: Some(DID.to_string()),
        }
    }

    fn linked(user_id: u64) -> IdentityLink {
        IdentityLink {
            did: DID.to_string(),
            login: "priyaraman".to_string(),
            user_id,
        }
    }

    /// Account 0 or an invalid login names no account: even a bio naming
    /// the DID proves nothing, composes nothing and records nothing.
    #[test]
    fn an_observation_naming_no_account_proves_nothing() {
        for malformed in [
            observed("priyaraman", 0),
            observed("", 4_210_001),
            observed("4210001/../x", 4_210_001),
            observed("-priya", 4_210_001),
        ] {
            assert_eq!(
                check_link(&Did(DID.to_string()), &malformed),
                Err(MalformedObservation),
                "{malformed:?}"
            );
            assert_eq!(
                self_attested_link_record(DID, &malformed, "2026-10-01T09:00:00Z"),
                None
            );
        }
        assert!(matches!(
            check_link(&Did(DID.to_string()), &observed("priyaraman", 4_210_001)),
            Ok(ProofCheck::Verified(_))
        ));
    }

    /// A re-check over an answer naming no account — by login or by number —
    /// records nothing, never `account_not_found`.
    #[test]
    fn a_recheck_over_an_answer_naming_no_account_records_nothing() {
        let link = linked(4_210_001);
        let by_login = recheck_link::<()>(
            &link,
            "priyaraman",
            GithubRead::Profile(observed("priyaraman", 0)),
            |_| panic!("an unreadable login answer asks nothing more"),
        );
        assert_eq!(
            by_login,
            Err(RecheckUnreadable::Malformed(MalformedObservation))
        );
        let by_number = recheck_link::<()>(&link, "priyaraman", GithubRead::NotFound, |_| {
            GithubRead::Profile(observed("priyaraman", 0))
        });
        assert_eq!(
            by_number,
            Err(RecheckUnreadable::Malformed(MalformedObservation))
        );
        let bad_login = recheck_link::<()>(
            &link,
            "priyaraman",
            GithubRead::Profile(observed("not a login", 4_210_001)),
            |_| GithubRead::NotFound,
        );
        assert_eq!(
            bad_login,
            Err(RecheckUnreadable::Malformed(MalformedObservation))
        );
    }

    #[test]
    fn a_link_naming_account_zero_is_no_link() {
        let evidence = [
            "https://github.com/priyaraman".to_string(),
            "https://api.github.com/user/0".to_string(),
        ];
        let fields = ClaimFields {
            author: DID,
            subject: DID,
            predicate: LINK_PREDICATE,
            object: "github:priyaraman",
            evidence: &evidence,
        };
        let provenance = Provenance::SelfAttested {
            repo_did: Did(DID.to_string()),
        };
        assert_eq!(project(fields, &provenance), None);
        let numbered = [evidence[0].clone(), format!("{ID_URI_PREFIX}4210001")];
        assert!(project(
            ClaimFields {
                evidence: &numbered,
                ..fields
            },
            &provenance
        )
        .is_some());
    }

    /// A DID ending a sentence is still the DID: trailing punctuation is
    /// not part of the token.
    #[test]
    fn a_did_ending_a_sentence_verifies() {
        for bio in [
            "my DID is did:plc:abc.",
            "did:plc:abc, and more",
            "did:plc:abc; then prose",
        ] {
            assert_eq!(
                bio_verdict(Some(bio), "did:plc:abc"),
                BioVerdict::Verified,
                "{bio}"
            );
        }
    }

    proptest::proptest! {
        /// Universe: DID-shaped tokens and every stripped trailing character.
        #[test]
        fn a_did_followed_by_trailing_punctuation_verifies(
            did in "did:[a-z]{1,8}:[A-Za-z0-9]{1,24}",
            trailing in proptest::sample::select(vec!['.', ',', ';', ':', '!', '?', ')', ']', '}', '>']),
        ) {
            let bio = format!("see {did}{trailing}");
            proptest::prop_assert_eq!(bio_verdict(Some(&bio), &did), BioVerdict::Verified);
        }
    }

    proptest::proptest! {
        /// Exact-token matching: a bio naming a longer DID that starts with
        /// the signer's, or only a prefix of the signer's DID, never verifies it.
        #[test]
        fn a_did_prefix_or_extension_never_verifies(
            did in "did:[a-z]{1,8}:[A-Za-z0-9]{2,24}",
            extension in "[A-Za-z0-9]{1,8}",
        ) {
            let longer = format!("see {did}{extension} here");
            let shorter = format!("see {} here", &did[..did.len() - 1]);
            for bio in [longer, shorter] {
                proptest::prop_assert_ne!(bio_verdict(Some(&bio), &did), BioVerdict::Verified, "{}", bio);
            }
        }
    }

    /// A token is DID-shaped only as `did:<lowercase method>:<rest>`: an
    /// empty or uppercase method, or another scheme, is no DID at all.
    #[test]
    fn a_token_not_shaped_as_a_did_is_no_did() {
        for bio in ["did::x", "did:PLC:x", "foo:abc:def"] {
            assert_eq!(bio_verdict(Some(bio), DID), BioVerdict::DidMissing, "{bio}");
        }
    }

    /// The wording `openlore identity verify` shows for an answer naming no
    /// account.
    #[test]
    fn an_answer_naming_no_account_reads_as_such() {
        assert_eq!(
            RecheckUnreadable::<String>::Malformed(MalformedObservation).to_string(),
            "GitHub answered without a valid account number and login"
        );
    }

    /// The link a self-attested record projects when its id URI ends in `id`.
    fn link_naming_id(id: &str) -> Option<IdentityLink> {
        let evidence = [
            "https://github.com/priyaraman".to_string(),
            format!("{ID_URI_PREFIX}{id}"),
        ];
        let fields = ClaimFields {
            author: DID,
            subject: DID,
            predicate: LINK_PREDICATE,
            object: "github:priyaraman",
            evidence: &evidence,
        };
        let provenance = Provenance::SelfAttested {
            repo_did: Did(DID.to_string()),
        };
        project(fields, &provenance)
    }

    /// An id URI names an account only by plain decimal digits: a signed
    /// number such as `+5` is malformed, so the record is no link.
    #[test]
    fn a_signed_account_number_is_no_link() {
        assert_eq!(link_naming_id("+5"), None);
        assert_eq!(link_naming_id("5").map(|link| link.user_id), Some(5));
    }

    proptest::proptest! {
        /// Universe: non-empty id-URI tails holding at least one non-digit.
        #[test]
        fn an_id_with_a_non_digit_is_no_link(
            id in "[0-9]{0,6}[^0-9][0-9]{0,6}",
        ) {
            proptest::prop_assert_eq!(link_naming_id(&id), None);
        }
    }

    /// An account scope covers a renamed link by the login it was signed
    /// under and by the login last seen, case-insensitively, and no other.
    #[test]
    fn an_account_scope_covers_a_renamed_link_by_either_login() {
        let link = IdentityLink {
            did: DID.to_string(),
            login: "oldname".to_string(),
            user_id: 4_210_001,
        };
        let covers =
            |login: &str| is_in_scope(&RecheckScope::Account(login.to_string()), &link, "newname");
        assert!(covers("newname"));
        assert!(covers("OLDNAME"));
        assert!(!covers("someoneelse"));
    }

    proptest::proptest! {
        /// Universe: link records over two DIDs, two account ids, two logins
        /// and a handful of record IDs, in any order with repeats.
        /// GDL-D9 / U-9: one group per (DID, account id) — a login never
        /// splits or merges one — naming every record ID once, ascending.
        #[test]
        fn link_records_group_by_person_and_account_naming_every_record_id_ascending(
            records in proptest::collection::vec(
                (
                    proptest::sample::select(vec!["did:plc:a", "did:plc:b"]),
                    proptest::sample::select(vec!["rachelk", "renamed"]),
                    1u64..=2,
                    "bafy[a-e]{1,2}",
                ),
                0..12,
            ),
        ) {
            let records: Vec<LinkRecord> = records
                .into_iter()
                .map(|(did, login, user_id, record_id)| LinkRecord {
                    record_id,
                    link: IdentityLink { did: did.to_string(), login: login.to_string(), user_id },
                })
                .collect();
            let groups = record_ids_per_link(&records);
            let expected: BTreeMap<(String, u64), BTreeSet<String>> =
                records.iter().fold(BTreeMap::new(), |mut expected, record| {
                    expected
                        .entry((record.link.did.clone(), record.link.user_id))
                        .or_default()
                        .insert(record.record_id.clone());
                    expected
                });
            proptest::prop_assert_eq!(
                groups.keys().collect::<Vec<_>>(),
                expected.keys().collect::<Vec<_>>()
            );
            for (key, ids) in &groups {
                proptest::prop_assert!(ids.windows(2).all(|pair| pair[0] < pair[1]), "{ids:?}");
                proptest::prop_assert_eq!(
                    &ids.iter().cloned().collect::<BTreeSet<_>>(),
                    &expected[key]
                );
            }
        }
    }

    /// A shown link names its own records, and a link with one record names
    /// that one record ID.
    #[test]
    fn a_shown_link_names_the_records_stating_it() {
        let record = |record_id: &str, user_id: u64| LinkRecord {
            record_id: record_id.to_string(),
            link: linked(user_id),
        };
        let shown = |user_id: u64| LinkedIdentity {
            link: linked(user_id),
            standing: LinkStanding::NotChecked,
        };
        let listed = listed_links(
            vec![shown(1), shown(2)],
            &[record("bafyb", 1), record("bafyc", 2), record("bafya", 1)],
        );
        let named: Vec<&[String]> = listed.iter().map(|l| l.record_ids.as_slice()).collect();
        assert_eq!(
            named,
            vec![
                &["bafya".to_string(), "bafyb".to_string()][..],
                &["bafyc".to_string()][..]
            ]
        );
    }

    fn instant(rfc3339: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(rfc3339)
            .expect("a valid instant")
            .with_timezone(&Utc)
    }

    proptest::proptest! {
        /// Universe: a check instant and an elapsed time from -2 days to
        /// 60 days, to the second. N-3 / U-8: the age is the whole days
        /// elapsed, never rounded up (a future check is 0 days old); a
        /// verified line suggests a re-check iff that age exceeds 30 days;
        /// a proof-gone, not-checked or contested line never does.
        #[test]
        fn a_verified_line_suggests_a_recheck_only_past_thirty_whole_days(
            elapsed_seconds in -2 * 86_400i64..=60 * 86_400,
            verdict in proptest::sample::select(CheckVerdict::ALL.to_vec()),
        ) {
            let checked_at = instant("2026-10-01T09:00:00Z");
            let now = checked_at + chrono::Duration::seconds(elapsed_seconds);
            let whole_days = u64::try_from(elapsed_seconds.max(0) / 86_400).expect("non-negative");
            proptest::prop_assert_eq!(check_age_days(checked_at, now), whole_days);
            proptest::prop_assert_eq!(
                suggests_recheck(&LinkStanding::Associated { checked_at }, now),
                whole_days > 30
            );
            let proof_gone = LinkStanding::ProofGone { verdict, checked_at };
            proptest::prop_assert!(!suggests_recheck(&proof_gone, now));
            proptest::prop_assert!(!suggests_recheck(&LinkStanding::NotChecked, now));
            proptest::prop_assert!(!suggests_recheck(&LinkStanding::Contested, now));
        }

        /// Universe: one verified check, and a later failed check dated
        /// before or after `now`. Q-RM-3: a check dated after `now` never
        /// decides; the earlier verified check does, with its own age.
        #[test]
        fn a_check_dated_after_now_never_decides_the_line(
            later_by_seconds in 1i64..=40 * 86_400,
            verdict in proptest::sample::select(vec![
                CheckVerdict::DidMissing,
                CheckVerdict::DifferentDid,
                CheckVerdict::NoBio,
                CheckVerdict::AccountNotFound,
            ]),
        ) {
            let verified_at = instant("2026-10-01T09:00:00Z");
            let now = verified_at + chrono::Duration::days(31);
            let failed_at = now + chrono::Duration::seconds(later_by_seconds);
            let check = |verdict, checked_at| LatestCheck {
                subject_did: DID.to_string(),
                user_id: 1,
                login_seen: "priyaraman".to_string(),
                verdict,
                checked_at,
            };
            let view = link_view(
                &[ReaderLink { link: linked(1), lifecycle: LinkLifecycle::Active }],
                &[check(CheckVerdict::Verified, verified_at), check(verdict, failed_at)],
                now,
            );
            proptest::prop_assert_eq!(
                &view[0].standing,
                &LinkStanding::Associated { checked_at: verified_at }
            );
            proptest::prop_assert!(suggests_recheck(&view[0].standing, now));
            let only_future = link_view(
                &[ReaderLink { link: linked(1), lifecycle: LinkLifecycle::Active }],
                &[check(CheckVerdict::Verified, failed_at)],
                now,
            );
            proptest::prop_assert_eq!(&only_future[0].standing, &LinkStanding::NotChecked);
        }
    }

    fn shown_link(did: &str, login: &str, user_id: u64) -> ListedLink {
        ListedLink {
            identity: LinkedIdentity {
                link: IdentityLink {
                    did: did.to_string(),
                    login: login.to_string(),
                    user_id,
                },
                standing: LinkStanding::NotChecked,
            },
            record_ids: Vec::new(),
        }
    }

    proptest::proptest! {
        /// Universe: a person row with an optional account id (0 = none)
        /// and a login, and a shown link with its own id and last-seen login
        /// in any letter case. U-6: a row carrying an id owns exactly the
        /// links to that id, whatever their login (a rename keeps the join,
        /// a namesake on another account never joins); a row carrying none
        /// owns exactly the links last seen under its login, ignoring case.
        #[test]
        fn a_link_joins_a_person_by_account_id_else_by_login_ignoring_case(
            row_id in 0u64..=3,
            row_login in proptest::sample::select(vec!["BurntSushi", "burnt-old", "rachelk"]),
            link_id in 1u64..=3,
            link_login in proptest::sample::select(vec!["BurntSushi", "burntsushi", "BURNT-OLD", "rachelk"]),
        ) {
            let person = PersonKey::of(row_login, [row_id]);
            let shown = [shown_link(DID, link_login, link_id)];
            let joined = !links_of_person(&shown, &person).is_empty();
            let expected = if row_id == 0 {
                row_login.eq_ignore_ascii_case(link_login)
            } else {
                row_id == link_id
            };
            proptest::prop_assert_eq!(joined, expected);
        }

        /// Universe: two people on distinct account ids and links to either
        /// account under either login. A link is never joined to the wrong
        /// person: each link joins exactly the one whose id it names.
        #[test]
        fn a_link_is_never_joined_to_the_wrong_person(
            links in proptest::collection::vec(
                (1u64..=2, proptest::sample::select(vec!["alice", "bob"])),
                0..8,
            ),
        ) {
            let alice = PersonKey::of("alice", [1]);
            let bob = PersonKey::of("bob", [2]);
            let shown: Vec<ListedLink> = links
                .iter()
                .map(|(id, login)| shown_link(DID, login, *id))
                .collect();
            let of_alice = links_of_person(&shown, &alice);
            let of_bob = links_of_person(&shown, &bob);
            proptest::prop_assert!(of_alice.iter().all(|l| l.identity.link.user_id == 1));
            proptest::prop_assert!(of_bob.iter().all(|l| l.identity.link.user_id == 2));
            proptest::prop_assert_eq!(of_alice.len() + of_bob.len(), shown.len());
        }
    }

    /// The listing one scope selects, over a world of link records and checks.
    fn listing(scope: &RecheckScope, world: &ListingWorld) -> Vec<ListedLink> {
        listing_in_scope(scope, &world.records, &world.checks, world.now)
    }

    struct ListingWorld {
        records: Vec<LinkRecord>,
        checks: Vec<LatestCheck>,
        now: DateTime<Utc>,
    }

    /// Both DIDs verified for one account: listing either one alone still
    /// reads contested (slice-2 review B1).
    #[test]
    fn listing_one_person_still_reads_their_contested_account_as_contested() {
        let at = instant("2026-10-01T09:00:00Z");
        let link_of = |did: &str| IdentityLink {
            did: did.to_string(),
            login: "jeffbailey".to_string(),
            user_id: 1,
        };
        let world = ListingWorld {
            records: ["did:plc:a", "did:plc:b"]
                .iter()
                .map(|did| LinkRecord {
                    record_id: format!("bafy-{did}"),
                    link: link_of(did),
                })
                .collect(),
            checks: ["did:plc:a", "did:plc:b"]
                .iter()
                .map(|did| LatestCheck {
                    subject_did: did.to_string(),
                    user_id: 1,
                    login_seen: "jeffbailey".to_string(),
                    verdict: CheckVerdict::Verified,
                    checked_at: at,
                })
                .collect(),
            now: at,
        };
        for did in ["did:plc:a", "did:plc:b"] {
            let shown = listing(&RecheckScope::Person(did.to_string()), &world);
            assert_eq!(shown.len(), 1, "{did}: {shown:?}");
            assert_eq!(shown[0].identity.link.did, did);
            assert_eq!(shown[0].identity.standing, LinkStanding::Contested);
        }
    }

    proptest::proptest! {
        /// Universe: link records over two DIDs, two accounts and two logins,
        /// this reader's checks of any verdict (some dated after `now`), and
        /// every scope kind (a DID bare or with a fragment, a login in any
        /// case). F7 / B1: a scope only selects lines — each line it shows is
        /// the unscoped line for that link, standing and records included;
        /// a person scope shows exactly that DID's unscoped lines.
        #[test]
        fn a_scope_selects_lines_and_never_changes_a_standing(
            records in proptest::collection::vec(
                (
                    proptest::sample::select(vec!["did:plc:a", "did:plc:b"]),
                    proptest::sample::select(vec!["rachelk", "renamed"]),
                    1u64..=2,
                    "bafy[a-c]",
                ),
                0..8,
            ),
            checks in proptest::collection::vec(
                (
                    proptest::sample::select(vec!["did:plc:a", "did:plc:b"]),
                    1u64..=2,
                    proptest::sample::select(vec!["rachelk", "renamed"]),
                    proptest::sample::select(CheckVerdict::ALL.to_vec()),
                    -2i64..=2,
                ),
                0..8,
            ),
            scope in proptest::sample::select(vec![
                RecheckScope::Everyone,
                RecheckScope::Person("did:plc:a".to_string()),
                RecheckScope::Person("did:plc:b#org.openlore.application".to_string()),
                RecheckScope::Account("RACHELK".to_string()),
                RecheckScope::Account("renamed".to_string()),
                RecheckScope::Account("nobody".to_string()),
            ]),
        ) {
            let now = instant("2026-10-10T09:00:00Z");
            let world = ListingWorld {
                records: records
                    .into_iter()
                    .map(|(did, login, user_id, record_id)| LinkRecord {
                        record_id,
                        link: IdentityLink { did: did.to_string(), login: login.to_string(), user_id },
                    })
                    .collect(),
                checks: checks
                    .into_iter()
                    .map(|(did, user_id, login_seen, verdict, days)| LatestCheck {
                        subject_did: did.to_string(),
                        user_id,
                        login_seen: login_seen.to_string(),
                        verdict,
                        checked_at: now + chrono::Duration::days(days),
                    })
                    .collect(),
                now,
            };
            let everyone = listing(&RecheckScope::Everyone, &world);
            let scoped = listing(&scope, &world);
            for line in &scoped {
                let unscoped = everyone.iter().find(|shown| {
                    shown.identity.link.did == line.identity.link.did
                        && shown.identity.link.user_id == line.identity.link.user_id
                });
                proptest::prop_assert_eq!(Some(line), unscoped);
            }
            if let RecheckScope::Person(did) = &scope {
                let of_person: Vec<&ListedLink> = everyone
                    .iter()
                    .filter(|shown| shown.identity.link.did == bare_did(did))
                    .collect();
                proptest::prop_assert_eq!(scoped.iter().collect::<Vec<_>>(), of_person);
            }
        }
    }

    /// ADR-085 §4: a re-check reads the login last seen on this link's
    /// latest check; with no check of this link (another DID's or another
    /// account's never counts) it reads the signed login.
    #[test]
    fn a_recheck_reads_the_login_last_seen_else_the_signed_one() {
        let link = linked(1);
        let seen = |did: &str, user_id: u64, login: &str, day: &str| LatestCheck {
            subject_did: did.to_string(),
            user_id,
            login_seen: login.to_string(),
            verdict: CheckVerdict::Verified,
            checked_at: instant(&format!("2026-10-{day}T09:00:00Z")),
        };
        assert_eq!(login_to_read(&link, &[]), "priyaraman");
        assert_eq!(
            login_to_read(
                &link,
                &[
                    seen("did:plc:other", 1, "elsewhere", "05"),
                    seen(DID, 2, "otheraccount", "05"),
                ]
            ),
            "priyaraman"
        );
        assert_eq!(
            login_to_read(&link, &[seen(DID, 1, "priya-r", "02")]),
            "priya-r"
        );
        assert_eq!(
            login_to_read(
                &link,
                &[seen(DID, 1, "newest", "09"), seen(DID, 1, "older", "02")]
            ),
            "newest"
        );
    }

    /// U-6: a `github:` person row with no contribution link joins by its
    /// login with the prefix stripped; with one, by the account ids of the
    /// links naming it (subject in any case), never another person's.
    #[test]
    fn a_person_row_keys_on_its_own_account_ids_else_its_stripped_login() {
        assert_eq!(
            PersonKey::of_person("github:BurntSushi", []),
            PersonKey::Login("BurntSushi".to_string())
        );
        assert_eq!(
            PersonKey::of_person(
                "github:BurntSushi",
                [
                    ("GITHUB:burntsushi", 7),
                    ("github:rachelk", 9),
                    ("github:burntsushi", 0)
                ]
            ),
            PersonKey::AccountIds(BTreeSet::from([7]))
        );
        assert_eq!(
            PersonKey::of_person("rachelk", []),
            PersonKey::Login("rachelk".to_string())
        );
    }

    proptest::proptest! {
        /// Universe: a `github:` person subject in any letter case, the
        /// contribution links' (subject, account id) pairs naming it or
        /// another person in any case, and a shown link on account 1..=3
        /// under either person's login in any case. U-6: the case the row
        /// is spelled in never changes which links it owns, and another
        /// person's contribution links never do either.
        #[test]
        fn a_person_rows_case_and_other_peoples_links_never_change_what_it_owns(
            upper in proptest::bool::ANY,
            own_links in proptest::collection::vec(
                (proptest::sample::select(vec!["github:burntsushi", "GITHUB:BURNTSUSHI"]), 0u64..=3),
                0..3,
            ),
            others_links in proptest::collection::vec(
                (proptest::sample::select(vec!["github:rachelk", "GITHUB:RACHELK"]), 0u64..=3),
                0..3,
            ),
            link_id in 1u64..=3,
            link_login in proptest::sample::select(vec!["burntsushi", "BurntSushi", "rachelk"]),
        ) {
            let person = "github:BurntSushi";
            let respelled = if upper { person.to_ascii_uppercase() } else { person.to_ascii_lowercase() };
            let shown = IdentityLink { did: DID.to_string(), login: link_login.to_string(), user_id: link_id };
            let owns = |subject: &str, links: &[(&'static str, u64)]| {
                PersonKey::of_person(subject, links.iter().copied()).owns(&shown)
            };
            let everyone: Vec<(&'static str, u64)> =
                own_links.iter().chain(others_links.iter()).copied().collect();
            proptest::prop_assert_eq!(owns(person, &own_links), owns(&respelled, &own_links));
            proptest::prop_assert_eq!(owns(person, &own_links), owns(person, &everyone));
        }
    }

    /// What one profile read should decide for a link, stated literally.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum Bio {
        NamingTheLinksDid,
        NamingAnotherDid,
        ProseWithoutADid,
        Absent,
    }

    impl Bio {
        fn text(self) -> Option<String> {
            match self {
                Bio::NamingTheLinksDid => Some(format!("I am {DID}.")),
                Bio::NamingAnotherDid => Some("did:plc:someoneelseentirelyxyz".to_string()),
                Bio::ProseWithoutADid => Some("Rust, music and maps".to_string()),
                Bio::Absent => None,
            }
        }
    }

    proptest::proptest! {
        /// Universe: an active link to account 1..=3 and one profile already
        /// read (by the person scrape's resolve) for an account 0..=3, under
        /// a valid or malformed login, with a bio naming the link's DID,
        /// another DID, no DID, or none. U-5, literal expectations: a profile
        /// of another account (account 0 included) decides nothing; a
        /// malformed login decides nothing; the linked account's profile
        /// with a valid login decides Verified (under that login, for that
        /// DID and account) iff its bio names the DID, else the matching
        /// failed verdict.
        #[test]
        fn only_a_profile_of_the_linked_account_decides_a_check(
            link_id in 1u64..=3,
            profile_id in 0u64..=3,
            login in proptest::sample::select(vec!["BurntSushi", "", "-bad-", "a/b"]),
            bio in proptest::sample::select(vec![
                Bio::NamingTheLinksDid,
                Bio::NamingAnotherDid,
                Bio::ProseWithoutADid,
                Bio::Absent,
            ]),
        ) {
            let profile = GithubObservation {
                login: login.to_string(),
                user_id: profile_id,
                bio: bio.text(),
            };
            let decided = check_on_profile(&linked(link_id), &profile);
            let valid_login = login == "BurntSushi";
            let disproved = |verdict| Some(ProofCheck::Disproved {
                verdict,
                login_seen: "BurntSushi".to_string(),
            });
            match (profile_id == link_id, valid_login, bio) {
                (false, _, _) | (true, false, _) => proptest::prop_assert_eq!(decided, None),
                (true, true, Bio::NamingTheLinksDid) => {
                    let Some(ProofCheck::Verified(proven)) = decided else {
                        return Err(proptest::test_runner::TestCaseError::fail(format!(
                            "expected Verified, got {decided:?}"
                        )));
                    };
                    proptest::prop_assert_eq!(proven.login(), "BurntSushi");
                    proptest::prop_assert_eq!(proven.did(), DID);
                    proptest::prop_assert_eq!(proven.user_id(), link_id);
                }
                (true, true, Bio::NamingAnotherDid) => {
                    proptest::prop_assert_eq!(decided, disproved(CheckVerdict::DifferentDid));
                }
                (true, true, Bio::ProseWithoutADid) => {
                    proptest::prop_assert_eq!(decided, disproved(CheckVerdict::DidMissing));
                }
                (true, true, Bio::Absent) => {
                    proptest::prop_assert_eq!(decided, disproved(CheckVerdict::NoBio));
                }
            }
        }
    }

    /// Literal account rows naming no account (moved from the review app's
    /// ownership proof, U-25): account 0, an empty login, a leading hyphen,
    /// a path.
    const MALFORMED_ACCOUNTS: [(&str, u64); 4] = [
        ("priyaraman", 0),
        ("", 4_210_001),
        ("-priya", 4_210_001),
        ("priya/raman", 4_210_001),
    ];

    /// Literal logins GitHub could name, up to the 39-character limit.
    const ACCEPTED_LOGINS: [&str; 6] = [
        "priyaraman",
        "a",
        "7",
        "Priya-Raman",
        "rachel-k-42",
        "abcdefghijklmnopqrstuvwxyz0123456789abc",
    ];

    /// Literal text that is no login: empty, a leading hyphen, a path,
    /// query, fragment, dot, space or underscore, non-ASCII, or 40
    /// characters long.
    const REFUSED_LOGINS: [&str; 11] = [
        "",
        "-priya",
        "priya/raman",
        "priya?raman",
        "priya#raman",
        "priya.raman",
        "priya raman",
        "priya_raman",
        "pri\u{fc}ya",
        "4210001/../x",
        "abcdefghijklmnopqrstuvwxyz0123456789abcd",
    ];

    proptest::proptest! {
        /// Universe: (DID, surrounding words, recorded id, literal malformed
        /// account). An account numbered 0 or without a GitHub login is
        /// never proven — even when its bio names the DID — and is refused
        /// as unreadable, never as a disproving verdict.
        #[test]
        fn an_account_github_did_not_name_is_never_proven(
            words in "[A-Za-z ,!?()]{0,20}",
            malformed in proptest::sample::select(MALFORMED_ACCOUNTS.to_vec()),
        ) {
            let (login, user_id) = malformed;
            proptest::prop_assert_eq!(account_named_by(user_id, login), Err(MalformedObservation));
            let observation = GithubObservation {
                login: login.to_string(),
                user_id,
                bio: Some(format!("{words} {DID}")),
            };
            proptest::prop_assert_eq!(
                check_link(&Did(DID.to_string()), &observation),
                Err(MalformedObservation)
            );
            proptest::prop_assert_eq!(
                check_on_profile(&linked(user_id), &observation),
                None
            );
        }

        /// Universe: typed strings, generated valid logins (an alphanumeric
        /// then up to 38 alphanumerics or hyphens) plus the literal login
        /// tables. Only
        /// GitHub-username-shaped text is a login, so a typed login can never
        /// carry a path or query; an account is named exactly by a login.
        #[test]
        fn only_github_username_shaped_text_is_a_login(
            typed in "[A-Za-z0-9/?#. -]{0,45}",
            valid in "[A-Za-z0-9][A-Za-z0-9-]{0,38}",
        ) {
            let shaped = (1..=39).contains(&typed.len())
                && !typed.starts_with('-')
                && typed.chars().all(|c| c.is_ascii_alphanumeric() || c == '-');
            proptest::prop_assert_eq!(is_github_login(&typed), shaped);
            proptest::prop_assert!(is_github_login(&valid), "{valid:?}");
            proptest::prop_assert_eq!(
                account_named_by(4_210_001, &valid),
                Ok((NonZeroU64::new(4_210_001).unwrap(), valid.as_str()))
            );
            for login in ACCEPTED_LOGINS {
                proptest::prop_assert!(is_github_login(login), "{login:?}");
                proptest::prop_assert_eq!(
                    account_named_by(4_210_001, login),
                    Ok((NonZeroU64::new(4_210_001).unwrap(), login))
                );
            }
            for text in REFUSED_LOGINS {
                proptest::prop_assert!(!is_github_login(text), "{text:?}");
                proptest::prop_assert_eq!(account_named_by(4_210_001, text), Err(MalformedObservation));
            }
            if is_github_login(&typed) {
                proptest::prop_assert!(!typed.contains(['/', '?', '#', '.', ' ']));
                proptest::prop_assert!((1..=39).contains(&typed.len()) && !typed.starts_with('-'));
            }
            proptest::prop_assert_eq!(
                account_named_by(4_210_001, &typed).is_ok(),
                is_github_login(&typed)
            );
        }
    }

    /// One generated link record: its object, evidence, references, CID and
    /// the link it states.
    struct ShapedLink {
        object: String,
        evidence: Vec<String>,
        references: Vec<crate::ClaimReference>,
        cid: String,
        link: IdentityLink,
    }

    proptest::proptest! {
        /// Universe: one well-formed, self-attested link record per account,
        /// each carrying a reference list drawn from every reference type (to
        /// a foreign CID, so nothing in the stored set is withdrawn). U-39:
        /// a record carrying a `retracts` reference never projects as a link
        /// (another client's retraction may copy the link's fields); every
        /// other record still projects exactly its link.
        #[test]
        fn a_record_carrying_a_retracts_reference_never_projects_as_a_link(
            records in proptest::collection::vec(
                (
                    "[a-z][a-z0-9]{0,11}",
                    1u64..=u64::from(u32::MAX),
                    proptest::collection::vec(
                        proptest::sample::select(vec![
                            crate::ReferenceType::Retracts,
                            crate::ReferenceType::Corrects,
                            crate::ReferenceType::Counters,
                            crate::ReferenceType::Supersedes,
                        ]),
                        0..3,
                    ),
                ),
                0..6,
            ),
        ) {
            let provenance = Provenance::SelfAttested { repo_did: Did(DID.to_string()) };
            let shaped: Vec<ShapedLink> = records
                .iter()
                .enumerate()
                .map(|(index, (login, user_id, types))| {
                    let evidence = vec![
                        format!("{PROFILE_URL_PREFIX}{login}"),
                        format!("{ID_URI_PREFIX}{user_id}"),
                    ];
                    let references = types
                        .iter()
                        .map(|ref_type| crate::ClaimReference {
                            ref_type: *ref_type,
                            cid: crate::Cid("bafyforeign".to_string()),
                        })
                        .collect();
                    let link = IdentityLink { did: DID.to_string(), login: login.clone(), user_id: *user_id };
                    ShapedLink { object: format!("github:{login}"), evidence, references, cid: format!("bafylink{index}"), link }
                })
                .collect();
            let stored: Vec<StoredClaim<'_>> = shaped
                .iter()
                .map(|ShapedLink { object, evidence, references, cid, .. }| StoredClaim {
                    fields: ClaimFields { author: DID, subject: DID, predicate: LINK_PREDICATE, object, evidence },
                    provenance: &provenance,
                    lineage: ClaimLineage { author_did: DID, cid, references },
                })
                .collect();
            let referrers: Vec<ClaimLineage<'_>> = stored.iter().map(|claim| claim.lineage).collect();

            let expected: Vec<LinkRecord> = shaped
                .iter()
                .filter(|ShapedLink { references, .. }| {
                    !references.iter().any(|reference| reference.ref_type == crate::ReferenceType::Retracts)
                })
                .map(|ShapedLink { cid, link, .. }| LinkRecord { record_id: cid.clone(), link: link.clone() })
                .collect();
            proptest::prop_assert_eq!(active_link_records(&stored, &referrers), expected);
        }
    }
}
