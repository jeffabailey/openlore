//! Who may sign an assertion about a subject (ADR-086 §4), owner tier only
//! (ADR-089 §2): the one pure gate that mints a [`SigningPermit`].
//!
//! `may_sign` decides from values the caller read in the SAME request: the
//! repo as GitHub serves it now ([`RepoFacts`]) and the current bio of the
//! repo's owner. A permit has private fields, no serde and no public
//! constructor, so a publish plan that requires one cannot be built from a
//! stored status. Freshness of the inputs is the caller's duty, held by
//! `xtask check-arch` (`authority_evidence_only_from_adapters`).
//!
//! The CLI decides from this reader's LOCAL checks instead (ADR-086/087,
//! owner-only-signing): [`requirement`] says which tier a subject needs,
//! [`classify_person`] judges the person tier over the signer's identity links
//! and their latest checks ([`LinkedAccountEvidence`]), and
//! [`may_sign_from_checks`] mints the SAME [`SigningPermit`] or returns the
//! hand-off data of an [`AuthorityRefusal`].
//!
//! PURE: no clock, no store, no network — `now` arrives as data.

use std::cmp::Reverse;

use chrono::{DateTime, Utc};

use crate::identity_link::{bio_verdict, is_did_shaped, link_predicate, BioVerdict, CheckVerdict};
use crate::references::ReferenceStance;
use crate::ReferenceType;

/// Who owns a repo, as GitHub's `owner.type` says.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepoOwnerKind {
    User,
    Organization,
}

/// One repo as `GET /repos/{o}/{r}` serves it now (ADR-089 §1), read from
/// the final body after at most one redirect. A plain port value: only
/// adapters and test support build one (`authority_evidence_only_from_adapters`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoFacts {
    pub repo_id: u64,
    /// `owner/name` as served now (a rename or transfer shows here).
    pub full_name: String,
    pub owner_id: u64,
    pub owner_login: String,
    pub owner_kind: RepoOwnerKind,
    pub fork: bool,
    pub archived: bool,
}

/// What reading the repo in this request found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepoRead<'a> {
    Found(&'a RepoFacts),
    /// 404: missing or private (one meaning).
    Gone,
    /// Rate limit or outage: nothing can be decided.
    Unfinished,
}

/// What reading the repo owner's profile in this request found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OwnerBioRead<'a> {
    /// The owner's bio now (`None`: blank).
    Bio(Option<&'a str>),
    /// Not read, rate limited or unreachable.
    Unfinished,
}

/// Why the signer holds the permit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermitBasis {
    /// The subject's owner is the signer's GitHub account (its id).
    Owner(u64),
    /// The claim needs no authority: a dispute, a withdrawal, an identity
    /// link, or a subject that is not on GitHub (ADR-086 §1-2).
    Exempt,
    /// The repo's authority file lists the signer's DID (ADR-087 §3).
    RepoListed,
}

/// The signer may publish a claim about `subject`. Minted only in this
/// module ([`may_sign`], [`may_sign_from_checks`],
/// [`may_sign_repo_from_checks`], [`exempt_permit`], [`republish_permit`]);
/// never serialized, never stored. What it authorises is bound into it
/// (U-OOS-D31): its signer and subject always, the one claim (by CID) for a
/// re-publish, and for an exempt permit only claims needing no authority —
/// [`SigningPermit::covers`] is the check every consumer makes.
#[derive(Debug, PartialEq, Eq)]
pub struct SigningPermit {
    signer: String,
    subject: String,
    basis: PermitBasis,
    /// The CID of the one claim the permit is for (a re-publish); `None`:
    /// any claim its signer, subject and basis cover.
    claim: Option<String>,
}

/// Why a permit does not cover a claim (U-OOS-D31).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PermitMismatch {
    /// The claim is by another author than the permit's signer.
    OtherSigner { permitted: String },
    /// The claim is about another subject than the permit's.
    OtherSubject { permitted: String },
    /// The permit is for one claim (by CID) and this is another.
    OtherClaim { permitted: String },
    /// An exempt permit, and this claim needs authority over its subject.
    NeedsAuthority,
}

impl std::fmt::Display for PermitMismatch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::OtherSigner { permitted } => write!(f, "the permit is for claims by {permitted}"),
            Self::OtherSubject { permitted } => {
                write!(f, "the permit is for claims about {permitted}")
            }
            Self::OtherClaim { permitted } => write!(f, "the permit is for claim {permitted}"),
            Self::NeedsAuthority => {
                write!(f, "the permit is for claims needing no authority")
            }
        }
    }
}

impl SigningPermit {
    pub fn signer(&self) -> &str {
        &self.signer
    }

    pub fn subject(&self) -> &str {
        &self.subject
    }

    pub fn basis(&self) -> PermitBasis {
        self.basis
    }

    /// The CID of the one claim this permit is for, if it is bound to one.
    pub fn claim(&self) -> Option<&str> {
        self.claim.as_deref()
    }

    /// Does this permit cover `claim`, authored by `author`, whose CID is
    /// `cid`? Its signer and subject must be the claim's; a permit bound to
    /// a claim covers only that CID; an exempt permit covers only a claim
    /// that needs no authority.
    pub fn covers(
        &self,
        author: &str,
        claim: ClaimShape<'_>,
        cid: &str,
    ) -> Result<(), PermitMismatch> {
        if self.signer != author {
            return Err(PermitMismatch::OtherSigner {
                permitted: self.signer.clone(),
            });
        }
        if self.subject != claim.subject {
            return Err(PermitMismatch::OtherSubject {
                permitted: self.subject.clone(),
            });
        }
        match (&self.claim, self.basis) {
            (Some(permitted), _) if permitted != cid => Err(PermitMismatch::OtherClaim {
                permitted: permitted.clone(),
            }),
            (None, PermitBasis::Exempt) if requirement(claim) != Requirement::Exempt => {
                Err(PermitMismatch::NeedsAuthority)
            }
            _ => Ok(()),
        }
    }
}

/// Why no permit was minted (ADR-089 §2, a closed set).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SigningRefusal {
    /// The repo's current owner is none of the signer's listed accounts.
    NotOwnedByListedAccount,
    /// An organization owns the repo (no owner tier for it).
    OrgOwned,
    /// The repo is missing or private.
    RepoGone,
    /// GitHub serves the repo under another `owner/name` now (renamed or
    /// transferred since the scan): never publish under the old subject.
    RepoRenamed { now: String },
    /// The listed owner account's bio no longer holds the signer's DID.
    BioNoLongerHolds(u64),
    /// GitHub could not answer: nothing is decided.
    Unfinished,
}

/// The repo owner whose bio must be read for a permit: the owner of a found,
/// user-owned repo listed by the signer; `None` when the repo read already
/// refuses.
pub fn owner_to_read(listed: &[u64], repo: RepoRead<'_>) -> Option<u64> {
    owner_account(listed, repo).ok()
}

/// May `signer` publish an assertion about `subject`? Only when GitHub still
/// serves the repo under `subject`'s `owner/name` (letter case aside), its
/// current owner is a user, is one of the signer's `listed` GitHub ids and
/// that owner's bio, read now, holds the signer's DID.
pub fn may_sign(
    signer: &str,
    subject: &str,
    listed: &[u64],
    repo: RepoRead<'_>,
    owner_bio: OwnerBioRead<'_>,
) -> Result<SigningPermit, SigningRefusal> {
    still_named(subject, repo)?;
    let owner_id = owner_account(listed, repo)?;
    match owner_bio {
        OwnerBioRead::Unfinished => Err(SigningRefusal::Unfinished),
        OwnerBioRead::Bio(bio) => match bio_verdict(bio, signer) {
            BioVerdict::Verified => Ok(permit(signer, subject, PermitBasis::Owner(owner_id))),
            _ => Err(SigningRefusal::BioNoLongerHolds(owner_id)),
        },
    }
}

/// The refusal to explain to the signer: a rename is named (and a new scan
/// offered) only while the repo's owner is still one of her `listed` user
/// accounts; a repo that also moved to an owner she does not list is
/// explained, and counted, as not hers (AP-1).
pub fn refusal_to_explain(
    refusal: SigningRefusal,
    listed: &[u64],
    repo: RepoRead<'_>,
) -> SigningRefusal {
    match refusal {
        SigningRefusal::RepoRenamed { .. } => owner_account(listed, repo).err().unwrap_or(refusal),
        other => other,
    }
}

/// The listed user account that owns the repo now, or why there is none.
fn owner_account(listed: &[u64], repo: RepoRead<'_>) -> Result<u64, SigningRefusal> {
    match repo {
        RepoRead::Unfinished => Err(SigningRefusal::Unfinished),
        RepoRead::Gone => Err(SigningRefusal::RepoGone),
        RepoRead::Found(facts) if facts.owner_kind != RepoOwnerKind::User => {
            Err(SigningRefusal::OrgOwned)
        }
        RepoRead::Found(facts) if !listed.contains(&facts.owner_id) => {
            Err(SigningRefusal::NotOwnedByListedAccount)
        }
        RepoRead::Found(facts) => Ok(facts.owner_id),
    }
}

/// A found repo GitHub now serves under an `owner/name` other than
/// `subject`'s (compared without letter case, as GitHub does) is refused
/// with its new name; anything else is judged by [`owner_account`].
fn still_named(subject: &str, repo: RepoRead<'_>) -> Result<(), SigningRefusal> {
    match repo {
        RepoRead::Found(facts) => match served_under_another_name(subject, &facts.full_name) {
            Some(current_name) => Err(SigningRefusal::RepoRenamed { now: current_name }),
            None => Ok(()),
        },
        _ => Ok(()),
    }
}

/// The `owner/name` GitHub serves `subject`'s repo as, when that is another
/// repo name than `subject`'s — compared without letter case, as GitHub
/// does: letter case alone is not a rename.
fn served_under_another_name(subject: &str, served_as: &str) -> Option<String> {
    let named = subject.strip_prefix("github:").unwrap_or(subject);
    (!named.eq_ignore_ascii_case(served_as)).then(|| served_as.to_string())
}

// -----------------------------------------------------------------------------
// The in-repo authority file (owner-only-signing, ADR-087 §3)
// -----------------------------------------------------------------------------

/// The largest authority file that grants anything (ADR-087 §3: ≤ 4 KiB).
pub const AUTHORITY_FILE_MAX_BYTES: usize = 4 * 1024;

/// The DIDs a repo's `.github/openlore-authority` lists, in file order: UTF-8
/// text, one DID per line, `#` starts a comment. A line that is not exactly
/// one DID lists nothing. `None` when the file is over
/// [`AUTHORITY_FILE_MAX_BYTES`] or not UTF-8: such a file grants nothing.
pub fn authority_file_dids(file: &[u8]) -> Option<Vec<String>> {
    if file.len() > AUTHORITY_FILE_MAX_BYTES {
        return None;
    }
    let text = std::str::from_utf8(file).ok()?;
    Some(
        text.lines()
            .map(without_comment)
            .filter(|line| is_one_did(line))
            .map(str::to_string)
            .collect(),
    )
}

/// A single DID-shaped token, nothing around it.
fn is_one_did(line: &str) -> bool {
    !line.contains(char::is_whitespace) && is_did_shaped(line)
}

/// A line up to its `#` comment, trimmed.
fn without_comment(line: &str) -> &str {
    line.split('#').next().unwrap_or_default().trim()
}

// -----------------------------------------------------------------------------
// The CLI gate over local checks (owner-only-signing, ADR-086 §1-5, ADR-087 §1)
// -----------------------------------------------------------------------------

/// The tier of authority an assertion about `subject` needs (ADR-086 §1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Requirement {
    /// Not a `github:` subject, or not an assertion: anyone may sign.
    Exempt,
    /// `github:<login>`: only the account's owner may sign.
    Person(PersonSubject),
    /// `github:<owner>/<repo>`: the repo's owner or a DID its authority file lists.
    Repo { owner: String, name: String },
    /// A `github:` subject that names no account or repo: nobody may sign.
    Malformed,
}

/// A `github:<login>` subject. Built only by [`requirement`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PersonSubject {
    uri: String,
    login: String,
}

impl PersonSubject {
    /// The subject as signed (`github:<login>`).
    pub fn uri(&self) -> &str {
        &self.uri
    }

    /// The GitHub login the subject names.
    pub fn login(&self) -> &str {
        &self.login
    }
}

/// The claim fields authority is decided from: what it is about, what it
/// says and the types of the references it carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClaimShape<'a> {
    pub subject: &'a str,
    pub predicate: &'a str,
    pub reference_types: &'a [ReferenceType],
}

/// What a claim is (ADR-086 §2). Only an assertion carries weight and needs
/// authority; a withdrawal, a dispute or an identity link never does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClaimKind {
    Assertion,
    Retraction,
    Counter,
    IdentityLink,
}

/// The kind of `claim`: a withdrawal if it carries a `retracts` reference, a
/// dispute if it carries a `counters` one, an identity link if its predicate
/// is exactly `sameAs`; anything else — `supersedes` and `corrects` too — is
/// an assertion.
pub fn kind(claim: ClaimShape<'_>) -> ClaimKind {
    let carries = |stance| {
        claim
            .reference_types
            .iter()
            .any(|reference| reference.stance() == stance)
    };
    if carries(ReferenceStance::Withdraws) {
        ClaimKind::Retraction
    } else if carries(ReferenceStance::Disputes) {
        ClaimKind::Counter
    } else if claim.predicate == link_predicate() {
        ClaimKind::IdentityLink
    } else {
        ClaimKind::Assertion
    }
}

/// Which authority `claim` needs. Only an assertion about a `github:`
/// subject needs any. The `github` scheme is matched without letter case,
/// and a `github:` subject naming no account or repo is [`Requirement::Malformed`],
/// never Exempt (fail closed, DWD-OOS-7).
pub fn requirement(claim: ClaimShape<'_>) -> Requirement {
    match (kind(claim), github_path(claim.subject)) {
        (ClaimKind::Assertion, Some(path)) => github_requirement(claim.subject, path),
        _ => Requirement::Exempt,
    }
}

/// The path after a `github:` scheme in any letter case; `None` for any
/// other subject.
fn github_path(subject: &str) -> Option<&str> {
    let (scheme, path) = subject.split_once(':')?;
    scheme.eq_ignore_ascii_case("github").then_some(path)
}

/// `github:<login>` names a person, `github:<owner>/<repo>` a repo; any
/// other path is malformed.
fn github_requirement(subject: &str, path: &str) -> Requirement {
    match path.split('/').collect::<Vec<_>>().as_slice() {
        [login] if is_github_login(login) => Requirement::Person(PersonSubject {
            uri: subject.to_string(),
            login: login.to_string(),
        }),
        [owner, name] if is_github_login(owner) && is_github_repo_name(name) => Requirement::Repo {
            owner: owner.to_string(),
            name: name.to_string(),
        },
        _ => Requirement::Malformed,
    }
}

/// A GitHub login: 1-39 ASCII letters, digits or hyphens, not starting or
/// ending with a hyphen.
fn is_github_login(login: &str) -> bool {
    (1..=39).contains(&login.len())
        && login.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
        && !login.starts_with('-')
        && !login.ends_with('-')
}

/// A GitHub repo name: 1-100 ASCII letters, digits, `-`, `_` or `.`, other
/// than `.` and `..`.
fn is_github_repo_name(name: &str) -> bool {
    (1..=100).contains(&name.len())
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
        && name != "."
        && name != ".."
}

/// One of the signer's active identity links (ADR-084) with this reader's
/// latest check of it — the evidence the person tier decides from. A plain
/// port value: only adapters and test support build one
/// (`authority_evidence_only_from_adapters`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkedAccountEvidence {
    /// The linked account's stable numeric GitHub id.
    pub github_id: u64,
    /// The login the link was signed under.
    pub linked_login: String,
    /// This reader's latest check of the link not dated in the future
    /// (`None`: never checked here).
    pub latest_check: Option<LatestLinkCheck>,
}

/// This reader's latest check of one identity link (store v7).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LatestLinkCheck {
    /// The login GitHub reported on that check (a rename shows here).
    pub login_seen: String,
    pub verdict: CheckVerdict,
    /// When this reader made the check.
    pub checked_at: DateTime<Utc>,
}

/// Why a claim's author holds no authority over its subject (ADR-087 §1-3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unattested {
    /// None of the author's links names the account.
    NoLink,
    /// The author's link to the account is not verified by its latest check.
    LinkNotVerified,
    /// The subject login is no longer the login of the author's linked account.
    LoginNotCurrent,
    /// A user owns the repo, and it is not an account the author has a
    /// verified link to.
    NotRepoOwner,
    /// An organization owns the repo and its authority file does not list
    /// the author.
    OrgOwnedNotListed,
    /// The repo's authority file is not a valid authority file.
    FileInvalid,
    /// GitHub did not have the repo when it was last observed.
    RepoNotFound,
}

/// The person tier, judged over the signer's links.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PersonAuthority {
    /// The signer owns the account: a verified link whose latest check saw
    /// the subject login, made at `checked_at`.
    Attested {
        github_id: u64,
        checked_at: DateTime<Utc>,
    },
    /// The signer's link to the account has no check here yet.
    NotChecked,
    Unattested(Unattested),
}

/// Does the signer own the account `person` names? Only when one of `links`
/// has a latest check that is verified AND saw the subject login (letter case
/// aside, as GitHub compares logins). Every verified link counts, so a
/// contested account still attests each verified DID (UC-2); of several, the
/// most recently checked is the one the signer stands on. A check dated
/// after `now` counts as missing.
pub fn classify_person(
    person: &PersonSubject,
    links: &[LinkedAccountEvidence],
    now: DateTime<Utc>,
) -> PersonAuthority {
    links
        .iter()
        .filter_map(|link| link_standing(person.login(), link, now))
        .min_by_key(standing_rank)
        .unwrap_or(PersonAuthority::Unattested(Unattested::NoLink))
}

/// What one link says about `login` — the one rule the gate and every
/// reader share (D8); `None` when the link names another account. A link
/// names `login` when it was linked under it or its latest check saw it. Its
/// latest check decides: not verified, the proof is gone whatever login it
/// saw (LinkNotVerified); verified and seeing `login`, the owner; verified
/// under another login, `login` is no longer current; never checked here,
/// not checked.
fn link_standing(
    login: &str,
    link: &LinkedAccountEvidence,
    now: DateTime<Utc>,
) -> Option<PersonAuthority> {
    let names_login = |other: &str| other.eq_ignore_ascii_case(login);
    let check = current_check(link, now);
    let related = names_login(&link.linked_login)
        || check.is_some_and(|check| names_login(&check.login_seen));
    if !related {
        return None;
    }
    Some(match check {
        Some(check) if check.verdict != CheckVerdict::Verified => {
            PersonAuthority::Unattested(Unattested::LinkNotVerified)
        }
        Some(check) if names_login(&check.login_seen) => PersonAuthority::Attested {
            github_id: link.github_id,
            checked_at: check.checked_at,
        },
        Some(_) => PersonAuthority::Unattested(Unattested::LoginNotCurrent),
        None => PersonAuthority::NotChecked,
    })
}

/// The strongest standing wins: attested (the most recent check first), then
/// the reasons nearest to it.
fn standing_rank(standing: &PersonAuthority) -> (u8, Reverse<Option<DateTime<Utc>>>) {
    match standing {
        PersonAuthority::Attested { checked_at, .. } => (0, Reverse(Some(*checked_at))),
        PersonAuthority::NotChecked => (1, Reverse(None)),
        PersonAuthority::Unattested(Unattested::LinkNotVerified) => (2, Reverse(None)),
        PersonAuthority::Unattested(Unattested::LoginNotCurrent) => (3, Reverse(None)),
        PersonAuthority::Unattested(_) => (4, Reverse(None)),
    }
}

/// Who the hand-off names as able to sign (ADR-086 §10).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WhoCanSign {
    /// The account's owner, after linking it to their identity.
    OwnerAfterLinking { login: String },
    /// A user-owned repo's owner, after linking the account, or a DID the
    /// repo's authority file lists.
    RepoOwnerOrListed { login: String },
    /// A DID an organization repo's authority file lists.
    ListedInAuthorityFile,
}

/// The signing window (A-6, DWD-OOS-14): the signer signs from a local check
/// at most this many days old — inclusive, at minute granularity.
pub const SIGNING_WINDOW_DAYS: i64 = 7;

/// Why no permit was minted from local checks (`subject`: as signed).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthorityRefusal {
    /// The signer holds no authority over the subject: why, and who can sign
    /// instead.
    Unattested {
        subject: String,
        reason: Unattested,
        can_sign: WhoCanSign,
    },
    /// The authority rests on no local check within the signing window: the
    /// signer re-checks it (`openlore authority verify`) before signing.
    /// `last_checked`: the check that is too old (`None`: never checked, or
    /// only dated in the future).
    Recheck {
        subject: String,
        last_checked: Option<DateTime<Utc>>,
    },
}

/// The CLI gate (ADR-086 §4-5): may `signer` sign an assertion about
/// `person` at `now`, judged from this reader's local checks of the signer's
/// `links`? Mints the same [`SigningPermit`] as [`may_sign`]; refuses with
/// the hand-off data, or asks for a re-check.
pub fn may_sign_from_checks(
    signer: &str,
    person: &PersonSubject,
    links: &[LinkedAccountEvidence],
    now: DateTime<Utc>,
) -> Result<SigningPermit, AuthorityRefusal> {
    let subject = person.uri().to_string();
    match classify_person(person, links, now) {
        PersonAuthority::Attested {
            github_id,
            checked_at,
        } if within_signing_window(checked_at, now) => {
            Ok(permit(signer, &subject, PermitBasis::Owner(github_id)))
        }
        PersonAuthority::Attested { checked_at, .. } => Err(AuthorityRefusal::Recheck {
            subject,
            last_checked: Some(checked_at),
        }),
        PersonAuthority::NotChecked => Err(AuthorityRefusal::Recheck {
            subject,
            last_checked: None,
        }),
        PersonAuthority::Unattested(reason) => Err(AuthorityRefusal::Unattested {
            subject,
            reason,
            can_sign: WhoCanSign::OwnerAfterLinking {
                login: person.login().to_string(),
            },
        }),
    }
}

/// The repo tier of the CLI gate (ADR-086 §5, ADR-087 §2-3): may `signer`
/// sign an assertion about the repo `subject` at `now`, judged from this
/// reader's latest observation of the repo (`None`: never observed here) and
/// the signer's `links`? Permits when that observation attests the signer —
/// the authority file lists the signer's DID exactly (RepoListed), or a user
/// owns the repo and the signer holds a verified link to that account
/// (Owner) — and every check the tier rests on is within the signing window:
/// the OLDER of the observation and the link check decides an Owner permit.
/// An observation dated after `now` counts as missing.
pub fn may_sign_repo_from_checks(
    signer: &str,
    subject: &str,
    links: &[LinkedAccountEvidence],
    repo: Option<&LatestRepoObservation>,
    now: DateTime<Utc>,
) -> Result<SigningPermit, AuthorityRefusal> {
    let recheck = |last_checked| AuthorityRefusal::Recheck {
        subject: subject.to_string(),
        last_checked,
    };
    let Some(observed) = repo.filter(|observed| observed.checked_at <= now) else {
        return Err(recheck(None));
    };
    let evidence = AuthorityEvidence {
        author: crate::bare_did(signer),
        author_links: links,
        repo: Some(observed),
    };
    match repo_standings(&evidence, observed, now)
        .into_iter()
        .max_by_key(|standing| standing.oldest_check)
    {
        Some(standing) if within_signing_window(standing.oldest_check, now) => {
            Ok(permit(signer, subject, standing.basis))
        }
        Some(standing) => Err(recheck(Some(standing.oldest_check))),
        None => Err(repo_refusal(
            subject,
            observed,
            repo_authority(&evidence, now),
        )),
    }
}

/// This reader's latest observation of a repo subject with the `owner/name`
/// GitHub served the repo under then (`served_as`: `None` when GitHub did
/// not have it). A redirect GitHub answers for a renamed or transferred repo
/// shows here as another name (ADR-087 A-10).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServedRepoObservation {
    pub latest: LatestRepoObservation,
    pub served_as: Option<String>,
}

/// Why no permit was minted for a repo claim from local checks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RepoSigningRefusal {
    /// The latest observation shows GitHub serving `subject`'s repo as
    /// `now` (renamed or transferred): authority is current, so the claim is
    /// signed only under the name GitHub serves now (ADR-087 §2).
    Renamed { subject: String, now: String },
    /// The repo tier of the gate refused ([`may_sign_repo_from_checks`]).
    Authority(AuthorityRefusal),
}

/// The repo gate `claim add` signs through: refuses a `subject` the latest
/// observation (not dated after `now`) shows GitHub serving under another
/// name — letter case alone is not a rename — and otherwise judges authority
/// by [`may_sign_repo_from_checks`].
pub fn may_sign_repo_as_served(
    signer: &str,
    subject: &str,
    links: &[LinkedAccountEvidence],
    repo: Option<&ServedRepoObservation>,
    now: DateTime<Utc>,
) -> Result<SigningPermit, RepoSigningRefusal> {
    let observed = repo.filter(|observed| observed.latest.checked_at <= now);
    if let Some(current_name) = observed
        .and_then(|observed| observed.served_as.as_deref())
        .and_then(|served_as| served_under_another_name(subject, served_as))
    {
        return Err(RepoSigningRefusal::Renamed {
            subject: subject.to_string(),
            now: current_name,
        });
    }
    may_sign_repo_from_checks(
        signer,
        subject,
        links,
        observed.map(|observed| &observed.latest),
        now,
    )
    .map_err(RepoSigningRefusal::Authority)
}

/// One attested repo tier and the oldest check it rests on.
struct RepoStanding {
    basis: PermitBasis,
    oldest_check: DateTime<Utc>,
}

/// Every attested tier the latest observation gives the author: RepoListed
/// rests on the observation alone; Owner on the observation and the verified
/// link check to the owning user's account.
fn repo_standings(
    evidence: &AuthorityEvidence<'_>,
    observed: &LatestRepoObservation,
    now: DateTime<Utc>,
) -> Vec<RepoStanding> {
    let RepoFinding::Found {
        owner_id,
        owner_kind,
        authority_file,
    } = &observed.finding
    else {
        return Vec::new();
    };
    let listed = lists_author(authority_file, evidence.author).then_some(RepoStanding {
        basis: PermitBasis::RepoListed,
        oldest_check: observed.checked_at,
    });
    let owned = (*owner_kind == RepoOwnerKind::User)
        .then(|| verified_check_of(evidence.author_links, *owner_id, now))
        .flatten()
        .map(|link_checked_at| RepoStanding {
            basis: PermitBasis::Owner(*owner_id),
            oldest_check: link_checked_at.min(observed.checked_at),
        });
    listed.into_iter().chain(owned).collect()
}

/// The most recent verified current check of a link to the account `github_id`.
fn verified_check_of(
    links: &[LinkedAccountEvidence],
    github_id: u64,
    now: DateTime<Utc>,
) -> Option<DateTime<Utc>> {
    links
        .iter()
        .filter(|link| link.github_id == github_id)
        .filter_map(|link| current_check(link, now))
        .filter(|check| check.verdict == CheckVerdict::Verified)
        .map(|check| check.checked_at)
        .max()
}

/// The refusal when the latest observation attests nothing: why, and who
/// can sign instead; a link to the owner never checked here asks for a
/// re-check (fail closed).
fn repo_refusal(
    subject: &str,
    observed: &LatestRepoObservation,
    authority: Authority,
) -> AuthorityRefusal {
    match authority {
        Authority::Unattested(reason) => AuthorityRefusal::Unattested {
            subject: subject.to_string(),
            reason,
            can_sign: repo_can_sign(subject, &observed.finding),
        },
        Authority::NotChecked | Authority::Attested(_) => AuthorityRefusal::Recheck {
            subject: subject.to_string(),
            last_checked: None,
        },
    }
}

/// Who can sign about a repo: a DID its authority file lists, and — unless
/// an organization owns it — its owner after linking the account.
fn repo_can_sign(subject: &str, finding: &RepoFinding) -> WhoCanSign {
    match finding {
        RepoFinding::Found {
            owner_kind: RepoOwnerKind::Organization,
            ..
        } => WhoCanSign::ListedInAuthorityFile,
        _ => WhoCanSign::RepoOwnerOrListed {
            login: github_path(subject)
                .and_then(|path| path.split('/').next())
                .unwrap_or_default()
                .to_string(),
        },
    }
}

/// A permit for a claim that needs no authority (ADR-086 §1-2, ruling F2):
/// `None` when `claim` is an assertion about a `github:` subject.
pub fn exempt_permit(signer: &str, claim: ClaimShape<'_>) -> Option<SigningPermit> {
    matches!(requirement(claim), Requirement::Exempt)
        .then(|| permit(signer, claim.subject, PermitBasis::Exempt))
}

/// A permit to re-publish the claim `cid` this store already published
/// (`published_at`: its first publication here; `None`: never published).
/// Re-publishing is idempotent and needs no fresh authority (ADR-086 §4); a
/// claim never published gets no permit here — it goes through the gate.
/// The permit covers that one claim only (U-OOS-D31).
pub fn republish_permit(
    signer: &str,
    subject: &str,
    cid: &str,
    published_at: Option<DateTime<Utc>>,
) -> Option<SigningPermit> {
    published_at.map(|_| SigningPermit {
        claim: Some(cid.to_string()),
        ..permit(signer, subject, PermitBasis::Exempt)
    })
}

fn permit(signer: &str, subject: &str, basis: PermitBasis) -> SigningPermit {
    SigningPermit {
        signer: signer.to_string(),
        subject: subject.to_string(),
        basis,
        claim: None,
    }
}

/// A check made at `checked_at` (not after `now`) is at most
/// [`SIGNING_WINDOW_DAYS`] old, counted in whole minutes.
fn within_signing_window(checked_at: DateTime<Utc>, now: DateTime<Utc>) -> bool {
    (now - checked_at).num_minutes() <= SIGNING_WINDOW_DAYS * 24 * 60
}

// -----------------------------------------------------------------------------
// Reader labels: who a claim's author is to its subject (ADR-086 §8, ADR-087 §1-3)
// -----------------------------------------------------------------------------

/// The `.github/openlore-authority` file as an observation saw it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthorityFileSeen {
    Absent,
    /// The exact DID tokens the file lists.
    Present(Vec<String>),
    /// Any other shape: grants nothing.
    Invalid,
}

/// What this reader's latest observation of a repo found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RepoFinding {
    Found {
        owner_id: u64,
        owner_kind: RepoOwnerKind,
        authority_file: AuthorityFileSeen,
    },
    /// GitHub did not have the repo (missing or private); no file was read.
    NotFound,
}

/// This reader's latest observation of one repo subject (ADR-087 §4-5):
/// authority is current, so only the latest observation decides.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LatestRepoObservation {
    pub finding: RepoFinding,
    pub checked_at: DateTime<Utc>,
}

/// The tier an attested claim stands on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttestedTier {
    /// The author owns the subject: the linked account a person subject
    /// names, or the user account that owns a repo (its GitHub id).
    Owner { github_id: u64 },
    /// The repo's authority file lists the author's DID.
    RepoListed,
}

/// What a claim about a `github:` subject is, to this reader: its author's
/// word about their own account or repo, someone else's word about it, or
/// not yet checked here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Authority {
    Attested(AttestedTier),
    Unattested(Unattested),
    NotChecked,
}

/// The reader label of an attested owner's claim.
pub const OWNER_LABEL: &str = "owner";
/// The reader label of a claim by a DID the repo's authority file lists.
pub const REPO_LISTED_LABEL: &str = "repo-listed";
/// The reader label of a claim by anyone else.
pub const THIRD_PARTY_LABEL: &str = "third-party";
/// The reader label of a claim whose authority has no check here yet.
pub const NOT_CHECKED_LABEL: &str = "authority not checked";

impl Authority {
    /// The reader label — one spelling everywhere.
    pub fn label(&self) -> &'static str {
        match self {
            Authority::Attested(AttestedTier::Owner { .. }) => OWNER_LABEL,
            Authority::Attested(AttestedTier::RepoListed) => REPO_LISTED_LABEL,
            Authority::Unattested(_) => THIRD_PARTY_LABEL,
            Authority::NotChecked => NOT_CHECKED_LABEL,
        }
    }
}

/// Everything local this reader holds about one claim's author and subject.
#[derive(Debug, Clone, Copy)]
pub struct AuthorityEvidence<'a> {
    /// The claim author's bare DID.
    pub author: &'a str,
    /// The author's active identity links, each with this reader's latest check.
    pub author_links: &'a [LinkedAccountEvidence],
    /// The latest observation of the repo a repo claim is about (`None`:
    /// never observed here, or not a repo claim).
    pub repo: Option<&'a LatestRepoObservation>,
}

/// Who `claim`'s author is to its subject, judged from this reader's latest
/// local checks (ADR-087 §1-3). `None` when the claim needs no authority
/// (exempt) or names nothing GitHub could attest (malformed): readers label
/// neither. A check dated after `now` counts as missing; age never changes
/// the label (only the signing gate's window).
pub fn classify(
    claim: ClaimShape<'_>,
    evidence: &AuthorityEvidence<'_>,
    now: DateTime<Utc>,
) -> Option<Authority> {
    match requirement(claim) {
        Requirement::Person(person) => Some(person_authority(&person, evidence.author_links, now)),
        Requirement::Repo { .. } => Some(repo_authority(evidence, now)),
        Requirement::Exempt | Requirement::Malformed => None,
    }
}

/// The person tier as a reader sees it: [`classify_person`], except that a
/// reader holding no identity link of the author at all has not checked
/// their authority (D27) — it cannot tell their own account from someone
/// else's. (The gate, judging the signer's own links, says "no link".)
fn person_authority(
    person: &PersonSubject,
    links: &[LinkedAccountEvidence],
    now: DateTime<Utc>,
) -> Authority {
    if links.is_empty() {
        return Authority::NotChecked;
    }
    classify_person(person, links, now).into()
}

impl From<PersonAuthority> for Authority {
    fn from(person: PersonAuthority) -> Self {
        match person {
            PersonAuthority::Attested { github_id, .. } => {
                Authority::Attested(AttestedTier::Owner { github_id })
            }
            PersonAuthority::NotChecked => Authority::NotChecked,
            PersonAuthority::Unattested(reason) => Authority::Unattested(reason),
        }
    }
}

/// A link's latest check, unless it is dated after `now` (then: missing).
fn current_check(link: &LinkedAccountEvidence, now: DateTime<Utc>) -> Option<&LatestLinkCheck> {
    link.latest_check
        .as_ref()
        .filter(|check| check.checked_at <= now)
}

/// The repo tiers over the latest observation (ADR-087 §2-3). Only the
/// latest observation decides: a former owner reads unattested.
fn repo_authority(evidence: &AuthorityEvidence<'_>, now: DateTime<Utc>) -> Authority {
    match evidence.repo.map(|observed| &observed.finding) {
        None => Authority::NotChecked,
        Some(RepoFinding::NotFound) => Authority::Unattested(Unattested::RepoNotFound),
        Some(RepoFinding::Found {
            owner_id,
            owner_kind,
            authority_file,
        }) => found_repo_authority(evidence, *owner_id, *owner_kind, authority_file, now),
    }
}

/// How the author stands to a repo owner's account through their links.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OwnerLink {
    Verified,
    NotVerified,
    NotChecked,
    None,
}

/// The author's strongest link standing to the account `owner_id`, judged
/// by each link's latest check (by account id: a rename does not matter).
fn owner_link(links: &[LinkedAccountEvidence], owner_id: u64, now: DateTime<Utc>) -> OwnerLink {
    links
        .iter()
        .filter(|link| link.github_id == owner_id)
        .map(|link| match current_check(link, now) {
            Some(check) if check.verdict == CheckVerdict::Verified => OwnerLink::Verified,
            Some(_) => OwnerLink::NotVerified,
            None => OwnerLink::NotChecked,
        })
        .min_by_key(|standing| *standing as u8)
        .unwrap_or(OwnerLink::None)
}

/// The authority file lists `author` as an exact token (no prefix, no case folding).
fn lists_author(authority_file: &AuthorityFileSeen, author: &str) -> bool {
    matches!(authority_file, AuthorityFileSeen::Present(dids) if dids.iter().any(|did| did == author))
}

/// Owner: a user owns the repo and the author has a verified link to that
/// account (an organization is never an owner). RepoListed: the authority
/// file lists the author. Otherwise the nearest reason it is not attested.
fn found_repo_authority(
    evidence: &AuthorityEvidence<'_>,
    owner_id: u64,
    owner_kind: RepoOwnerKind,
    authority_file: &AuthorityFileSeen,
    now: DateTime<Utc>,
) -> Authority {
    let user_owned = owner_kind == RepoOwnerKind::User;
    let link = owner_link(evidence.author_links, owner_id, now);
    if user_owned && link == OwnerLink::Verified {
        return Authority::Attested(AttestedTier::Owner {
            github_id: owner_id,
        });
    }
    if lists_author(authority_file, evidence.author) {
        return Authority::Attested(AttestedTier::RepoListed);
    }
    match (owner_kind, link, authority_file) {
        (RepoOwnerKind::User, OwnerLink::NotVerified, _) => {
            Authority::Unattested(Unattested::LinkNotVerified)
        }
        (RepoOwnerKind::User, OwnerLink::NotChecked, _) => Authority::NotChecked,
        (_, _, AuthorityFileSeen::Invalid) => Authority::Unattested(Unattested::FileInvalid),
        (RepoOwnerKind::Organization, _, _) => Authority::Unattested(Unattested::OrgOwnedNotListed),
        (RepoOwnerKind::User, _, _) => Authority::Unattested(Unattested::NotRepoOwner),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kelp(full_name: &str, owner_id: u64, owner_kind: RepoOwnerKind) -> RepoFacts {
        RepoFacts {
            repo_id: 7,
            full_name: full_name.to_string(),
            owner_id,
            owner_login: "priyaraman".to_string(),
            owner_kind,
            fork: false,
            archived: false,
        }
    }

    const DID: &str = "did:plc:priya";

    #[test]
    fn a_rename_is_explained_only_while_a_listed_user_still_owns_the_repo() {
        let renamed = |facts: &RepoFacts| {
            may_sign(
                DID,
                "github:priyaraman/kelp",
                &[1],
                RepoRead::Found(facts),
                OwnerBioRead::Bio(Some(DID)),
            )
            .unwrap_err()
        };
        let cases = [
            (kelp("priyaraman/kelp-sdk", 1, RepoOwnerKind::User), None),
            (
                kelp("tidal-labs/kelp", 9, RepoOwnerKind::User),
                Some(SigningRefusal::NotOwnedByListedAccount),
            ),
            (
                kelp("tidal-labs/kelp", 9, RepoOwnerKind::Organization),
                Some(SigningRefusal::OrgOwned),
            ),
        ];
        for (facts, moved_away) in cases {
            let refusal = renamed(&facts);
            assert_eq!(
                refusal,
                SigningRefusal::RepoRenamed {
                    now: facts.full_name.clone()
                }
            );
            let explained = refusal_to_explain(refusal.clone(), &[1], RepoRead::Found(&facts));
            assert_eq!(explained, moved_away.unwrap_or(refusal), "{facts:?}");
        }
    }

    #[test]
    fn letter_case_alone_is_not_a_rename() {
        let facts = kelp("PriyaRaman/Kelp", 1, RepoOwnerKind::User);
        let permit = may_sign(
            DID,
            "github:priyaraman/kelp",
            &[1],
            RepoRead::Found(&facts),
            OwnerBioRead::Bio(Some(DID)),
        );
        assert_eq!(permit.map(|p| p.basis()), Ok(PermitBasis::Owner(1)));
    }

    #[test]
    fn other_refusals_are_explained_as_they_are() {
        for refusal in [
            SigningRefusal::RepoGone,
            SigningRefusal::Unfinished,
            SigningRefusal::OrgOwned,
            SigningRefusal::BioNoLongerHolds(1),
        ] {
            assert_eq!(
                refusal_to_explain(refusal.clone(), &[], RepoRead::Unfinished),
                refusal
            );
        }
    }
}

#[cfg(test)]
mod gate_from_checks_tests {
    use super::*;
    use chrono::{Duration, TimeZone};
    use proptest::prelude::*;

    const JEFF: &str = "did:plc:jeff";
    const WINDOW_MINUTES: i64 = SIGNING_WINDOW_DAYS * 24 * 60;

    fn now() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 10, 8, 9, 0, 0).unwrap()
    }

    fn assertion_about(subject: &str) -> ClaimShape<'_> {
        ClaimShape {
            subject,
            predicate: "adheresToPhilosophy",
            reference_types: &[],
        }
    }

    fn person(subject: &str) -> PersonSubject {
        match requirement(assertion_about(subject)) {
            Requirement::Person(person) => person,
            other => panic!("{subject} is a person subject, got {other:?}"),
        }
    }

    fn checked(
        github_id: u64,
        linked: &str,
        check: Option<(&str, CheckVerdict)>,
        checked_at: DateTime<Utc>,
    ) -> LinkedAccountEvidence {
        LinkedAccountEvidence {
            github_id,
            linked_login: linked.to_string(),
            latest_check: check.map(|(login_seen, verdict)| LatestLinkCheck {
                login_seen: login_seen.to_string(),
                verdict,
                checked_at,
            }),
        }
    }

    fn link(
        github_id: u64,
        linked: &str,
        check: Option<(&str, CheckVerdict)>,
    ) -> LinkedAccountEvidence {
        checked(github_id, linked, check, now())
    }

    fn verdict() -> impl Strategy<Value = CheckVerdict> {
        proptest::sample::select(CheckVerdict::ALL.to_vec())
    }

    fn login() -> impl Strategy<Value = String> {
        proptest::sample::select(vec!["jeffbailey", "JeffBailey", "jeff-b", "rachelk"])
            .prop_map(str::to_string)
    }

    /// The age of a check in minutes, heavy on the window's edges (negative:
    /// dated in the future).
    fn age_minutes() -> impl Strategy<Value = i64> {
        prop_oneof![
            Just(0_i64),
            Just(WINDOW_MINUTES),
            Just(WINDOW_MINUTES + 1),
            Just(-1_i64),
            -60_i64..=20_000,
        ]
    }

    fn evidence() -> impl Strategy<Value = LinkedAccountEvidence> {
        (
            1u64..4,
            login(),
            proptest::option::of((login(), verdict(), age_minutes())),
        )
            .prop_map(|(github_id, linked_login, check)| LinkedAccountEvidence {
                github_id,
                linked_login,
                latest_check: check.map(|(login_seen, verdict, age)| LatestLinkCheck {
                    login_seen,
                    verdict,
                    checked_at: now() - Duration::minutes(age),
                }),
            })
    }

    /// The link's latest check as of `now()`: one dated in the future is missing.
    fn current_check(link: &LinkedAccountEvidence) -> Option<&LatestLinkCheck> {
        link.latest_check
            .as_ref()
            .filter(|check| check.checked_at <= now())
    }

    /// ADR-087 §1 stated over the evidence, independently of the gate: the
    /// reason a refusal about `login` gives when no link attests it. A link
    /// naming the login (linked under it, or its current check saw it) whose
    /// current check is not verified is not verified, whatever login that
    /// check saw (D8: the rule readers use too); a link linked under the
    /// login and verified since under another names a login that is no
    /// longer current; no link naming the login is no link at all.
    fn adr_087_reason(login: &str, links: &[LinkedAccountEvidence]) -> Unattested {
        let same = |other: &str| other.eq_ignore_ascii_case(login);
        let not_verified = links.iter().any(|l| {
            current_check(l).is_some_and(|c| {
                c.verdict != CheckVerdict::Verified
                    && (same(&c.login_seen) || same(&l.linked_login))
            })
        });
        let renamed_since = links.iter().any(|l| {
            same(&l.linked_login)
                && current_check(l)
                    .is_some_and(|c| c.verdict == CheckVerdict::Verified && !same(&c.login_seen))
        });
        if not_verified {
            Unattested::LinkNotVerified
        } else if renamed_since {
            Unattested::LoginNotCurrent
        } else {
            Unattested::NoLink
        }
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(256))]

        /// A permit exists iff some link's current check (not dated after
        /// now) is verified, saw the subject login and is at most 7 days old;
        /// its basis is that account. An owner whose checks are all older
        /// asks for a re-check naming the latest; a link never checked (or
        /// checked only in the future) asks for one with no date; otherwise
        /// the refusal names the subject, says the ADR-087 §1 reason and
        /// hands off to the subject login's owner — never to the signer.
        #[test]
        fn a_permit_needs_a_verified_current_check_within_the_window(
            subject_login in login(),
            links in proptest::collection::vec(evidence(), 0..4),
        ) {
            let subject = format!("github:{subject_login}");
            let same = |other: &str| other.eq_ignore_ascii_case(&subject_login);
            let owners: Vec<(u64, DateTime<Utc>)> = links
                .iter()
                .filter_map(|l| current_check(l).map(|c| (l.github_id, c)))
                .filter(|(_, c)| c.verdict == CheckVerdict::Verified && same(&c.login_seen))
                .map(|(id, c)| (id, c.checked_at))
                .collect();
            let fresh: Vec<u64> = owners
                .iter()
                .filter(|(_, at)| (now() - *at).num_minutes() <= WINDOW_MINUTES)
                .map(|(id, _)| *id)
                .collect();
            let never_checked = links
                .iter()
                .any(|l| current_check(l).is_none() && same(&l.linked_login));
            // The owner the signer stands on: the most recently checked
            // (the first such link, on a tie).
            let latest_owner = owners
                .iter()
                .map(|(_, at)| *at)
                .max()
                .and_then(|latest| owners.iter().find(|(_, at)| *at == latest))
                .map(|(id, _)| *id);
            match may_sign_from_checks(JEFF, &person(&subject), &links, now()) {
                Ok(permit) => {
                    prop_assert!(!fresh.is_empty(), "{links:?}");
                    let expected_owner = latest_owner.expect("a permit rests on an owner");
                    prop_assert_eq!(
                        (permit.signer(), permit.subject(), permit.basis()),
                        (JEFF, subject.as_str(), PermitBasis::Owner(expected_owner)),
                        "{:?}", links
                    );
                }
                Err(refusal) => {
                    prop_assert!(fresh.is_empty(), "{links:?}");
                    let expected = if let Some(latest) = owners.iter().map(|(_, at)| *at).max() {
                        AuthorityRefusal::Recheck { subject, last_checked: Some(latest) }
                    } else if never_checked {
                        AuthorityRefusal::Recheck { subject, last_checked: None }
                    } else {
                        AuthorityRefusal::Unattested {
                            subject,
                            reason: adr_087_reason(&subject_login, &links),
                            can_sign: WhoCanSign::OwnerAfterLinking { login: subject_login },
                        }
                    };
                    prop_assert_eq!(refusal, expected, "{:?}", links);
                }
            }
        }

        /// Any `github:` subject (scheme in any letter case) needs authority;
        /// any other subject is exempt.
        #[test]
        fn only_non_github_subjects_are_exempt(
            scheme in proptest::sample::select(vec!["github", "GitHub", "GITHUB", "https", "did", "npm"]),
            path in "[a-zA-Z0-9/_-]{0,12}",
        ) {
            let subject = format!("{scheme}:{path}");
            let required = requirement(assertion_about(&subject));
            prop_assert_eq!(
                required == Requirement::Exempt,
                !scheme.eq_ignore_ascii_case("github")
            );
        }
    }

    // bypass: one named example per refusal reason (closed set of three).
    #[test]
    fn each_refusal_says_why() {
        let jeff = person("github:jeffbailey");
        let cases = [
            (vec![], Unattested::NoLink),
            (
                vec![link(
                    1,
                    "rachelk",
                    Some(("rachelk", CheckVerdict::Verified)),
                )],
                Unattested::NoLink,
            ),
            (
                vec![link(
                    1,
                    "jeffbailey",
                    Some(("jeffbailey", CheckVerdict::DidMissing)),
                )],
                Unattested::LinkNotVerified,
            ),
            (
                vec![link(
                    1,
                    "jeffbailey",
                    Some(("jeff-b", CheckVerdict::Verified)),
                )],
                Unattested::LoginNotCurrent,
            ),
            // D8: a latest check that is not verified says the proof is
            // gone, whatever login it saw — gate and reader alike.
            (
                vec![link(
                    1,
                    "jeffbailey",
                    Some(("jeff-b", CheckVerdict::DidMissing)),
                )],
                Unattested::LinkNotVerified,
            ),
        ];
        for (links, reason) in cases {
            let reader = classify(
                assertion_about("github:jeffbailey"),
                &AuthorityEvidence {
                    author: JEFF,
                    author_links: &links,
                    repo: None,
                },
                now(),
            );
            if !links.is_empty() {
                assert_eq!(
                    reader,
                    Some(Authority::Unattested(reason)),
                    "reader: {links:?}"
                );
            }
            let refused = may_sign_from_checks(JEFF, &jeff, &links, now()).unwrap_err();
            let AuthorityRefusal::Unattested { reason: got, .. } = refused else {
                panic!("{links:?} is unattested, got {refused:?}");
            };
            assert_eq!(got, reason, "{links:?}");
        }
    }

    // bypass: D27 — the reader's two person outcomes with no attesting link.
    #[test]
    fn a_reader_holding_no_link_of_the_author_has_not_checked_their_authority() {
        let about_jeff = assertion_about("github:jeffbailey");
        let judged = |links: &[LinkedAccountEvidence]| {
            classify(
                about_jeff,
                &AuthorityEvidence {
                    author: "did:plc:sven",
                    author_links: links,
                    repo: None,
                },
                now(),
            )
        };
        assert_eq!(judged(&[]), Some(Authority::NotChecked));
        assert_eq!(
            judged(&[link(9, "sven", Some(("sven", CheckVerdict::Verified)))]),
            Some(Authority::Unattested(Unattested::NoLink))
        );
    }

    // bypass: the window's edges at minute granularity (A-6, DWD-OOS-14).
    #[test]
    fn the_window_holds_seven_days_to_the_minute_and_nothing_dated_after_now() {
        let jeff = person("github:jeffbailey");
        let window = Duration::minutes(WINDOW_MINUTES);
        let cases = [
            (Duration::zero(), true),
            (window, true),
            (window + Duration::seconds(59), true),
            (window + Duration::minutes(1), false),
            (Duration::seconds(-30), false),
            (Duration::minutes(-1), false),
        ];
        for (age, permitted) in cases {
            let checked_at = now() - age;
            let links = [checked(
                1,
                "jeffbailey",
                Some(("jeffbailey", CheckVerdict::Verified)),
                checked_at,
            )];
            let got = may_sign_from_checks(JEFF, &jeff, &links, now());
            let expected = match (permitted, checked_at <= now()) {
                (true, _) => Ok(PermitBasis::Owner(1)),
                (false, true) => Err(AuthorityRefusal::Recheck {
                    subject: "github:jeffbailey".to_string(),
                    last_checked: Some(checked_at),
                }),
                (false, false) => Err(AuthorityRefusal::Recheck {
                    subject: "github:jeffbailey".to_string(),
                    last_checked: None,
                }),
            };
            assert_eq!(got.map(|p| p.basis()), expected, "age {age}");
        }
    }

    // bypass: of two owner links (a contested account, UC-2), the signer
    // stands on the most recently checked — pinned by example as well as by
    // the property, which reaches this shape only rarely.
    #[test]
    fn the_permit_rests_on_the_most_recently_checked_owner_link() {
        let verified = Some(("jeffbailey", CheckVerdict::Verified));
        let links = [
            checked(1, "jeffbailey", verified, now() - Duration::days(3)),
            checked(2, "jeffbailey", verified, now() - Duration::days(1)),
        ];
        let permit = may_sign_from_checks(JEFF, &person("github:jeffbailey"), &links, now());
        assert_eq!(permit.map(|p| p.basis()), Ok(PermitBasis::Owner(2)));
    }

    // bypass: the renamed account signs only under its new login (G-2).
    #[test]
    fn a_renamed_account_signs_under_the_login_its_latest_check_saw() {
        let renamed = [link(
            7,
            "jeffbailey",
            Some(("jeff-b", CheckVerdict::Verified)),
        )];
        let permit = may_sign_from_checks(JEFF, &person("github:jeff-b"), &renamed, now());
        assert_eq!(permit.map(|p| p.basis()), Ok(PermitBasis::Owner(7)));
    }

    proptest! {
        /// Every exempt permit is for its claim's own subject and signer,
        /// with the Exempt basis, whatever the subject.
        #[test]
        fn exempt_permits_follow_the_requirement(
            subject in prop_oneof![
                "github:[a-z]{1,8}",
                "github:[a-z]{1,8}/[a-z]{1,8}",
                "(https|did|ex):[a-z]{1,8}",
            ],
            references in prop_oneof![
                Just(vec![]),
                Just(vec![ReferenceType::Counters]),
                Just(vec![ReferenceType::Retracts]),
                Just(vec![ReferenceType::Supersedes]),
            ],
            predicate in prop_oneof![Just("adheresToPhilosophy"), Just("sameAs")],
        ) {
            let claim = ClaimShape { subject: &subject, predicate, reference_types: &references };
            // Which claims get a permit is stated by example in
            // `exempt_permits_go_to_claims_needing_no_authority`; this states
            // the permit's shape.
            if let Some(permit) = exempt_permit(JEFF, claim) {
                prop_assert_eq!(
                    (permit.signer(), permit.subject(), permit.basis()),
                    (JEFF, subject.as_str(), PermitBasis::Exempt)
                );
            }
        }
    }

    // bypass: which claims get an exempt permit, stated by example rather
    // than restated through `requirement` (review: that property was
    // tautological).
    #[test]
    fn exempt_permits_go_to_claims_needing_no_authority() {
        let link = crate::identity_link::link_predicate();
        let cases: [(&str, &str, &[ReferenceType], bool); 9] = [
            ("github:jeffbailey", "adheresToPhilosophy", &[], false),
            (
                "github:jeffbailey/tidepool",
                "embodiesPhilosophy",
                &[],
                false,
            ),
            ("GitHub:jeffbailey", "adheresToPhilosophy", &[], false),
            ("github:a/b/c", "embodiesPhilosophy", &[], false),
            (
                "github:jeffbailey",
                "adheresToPhilosophy",
                &[ReferenceType::Supersedes],
                false,
            ),
            (
                "github:jeffbailey",
                "adheresToPhilosophy",
                &[ReferenceType::Counters],
                true,
            ),
            (
                "github:jeffbailey/tidepool",
                "embodiesPhilosophy",
                &[ReferenceType::Retracts],
                true,
            ),
            ("github:jeffbailey", link, &[], true),
            ("https:example.com", "embodiesPhilosophy", &[], true),
        ];
        for (subject, predicate, reference_types, exempt) in cases {
            let claim = ClaimShape {
                subject,
                predicate,
                reference_types,
            };
            assert_eq!(
                exempt_permit(JEFF, claim).is_some(),
                exempt,
                "{subject} {predicate} {reference_types:?}"
            );
        }
    }

    // bypass: a dispute about a GitHub account needs no authority.
    #[test]
    fn a_dispute_about_an_account_is_exempt() {
        let dispute = ClaimShape {
            subject: "github:someone",
            predicate: "adheresToPhilosophy",
            reference_types: &[ReferenceType::Counters],
        };
        assert_eq!(
            exempt_permit(JEFF, dispute).map(|p| p.basis()),
            Some(PermitBasis::Exempt)
        );
        assert_eq!(exempt_permit(JEFF, assertion_about("github:someone")), None);
    }

    /// Jeff's repo (owner id 1) observed a day ago under `served_as`.
    fn served(
        finding: RepoFinding,
        served_as: Option<&str>,
        age: Duration,
    ) -> ServedRepoObservation {
        ServedRepoObservation {
            latest: LatestRepoObservation {
                finding,
                checked_at: now() - age,
            },
            served_as: served_as.map(str::to_string),
        }
    }

    fn jeffs_repo() -> RepoFinding {
        RepoFinding::Found {
            owner_id: 1,
            owner_kind: RepoOwnerKind::User,
            authority_file: AuthorityFileSeen::Absent,
        }
    }

    // bypass: one example per served-name outcome; wording pins live in the ATs.
    #[test]
    fn a_repo_claim_is_signed_only_under_the_name_github_serves_now() {
        let links = [link(
            1,
            "jeffbailey",
            Some(("jeffbailey", CheckVerdict::Verified)),
        )];
        let subject = "github:jeffbailey/tidepool";
        let one_day = Duration::days(1);
        let owner = Ok(PermitBasis::Owner(1));
        let cases = [
            (
                served(jeffs_repo(), Some("jeffbailey/tidepool"), one_day),
                owner.clone(),
            ),
            (
                served(jeffs_repo(), Some("JeffBailey/TidePool"), one_day),
                owner,
            ),
            (
                served(jeffs_repo(), Some("jeffbailey/tidepool-rs"), one_day),
                Err(RepoSigningRefusal::Renamed {
                    subject: subject.to_string(),
                    now: "jeffbailey/tidepool-rs".to_string(),
                }),
            ),
            (
                served(jeffs_repo(), Some("acme-tools/tidepool"), one_day),
                Err(RepoSigningRefusal::Renamed {
                    subject: subject.to_string(),
                    now: "acme-tools/tidepool".to_string(),
                }),
            ),
            (
                served(RepoFinding::NotFound, None, one_day),
                Err(RepoSigningRefusal::Authority(
                    AuthorityRefusal::Unattested {
                        subject: subject.to_string(),
                        reason: Unattested::RepoNotFound,
                        can_sign: WhoCanSign::RepoOwnerOrListed {
                            login: "jeffbailey".to_string(),
                        },
                    },
                )),
            ),
            (
                served(jeffs_repo(), Some("jeffbailey/tidepool-rs"), -one_day),
                Err(RepoSigningRefusal::Authority(AuthorityRefusal::Recheck {
                    subject: subject.to_string(),
                    last_checked: None,
                })),
            ),
        ];
        for (observed, expected) in cases {
            let decided = may_sign_repo_as_served(JEFF, subject, &links, Some(&observed), now());
            assert_eq!(decided.map(|p| p.basis()), expected, "{observed:?}");
        }
    }

    // bypass: a repo subject names its owner and name.
    #[test]
    fn a_repo_subject_requires_repo_authority() {
        assert_eq!(
            requirement(assertion_about("github:jeffbailey/tidepool")),
            Requirement::Repo {
                owner: "jeffbailey".to_string(),
                name: "tidepool".to_string()
            }
        );
    }

    // bypass: GitHub's login and repo-name syntax, one example per edge.
    #[test]
    fn a_github_subject_names_an_account_or_repo_only_in_githubs_own_syntax() {
        let names_account = |subject: &str| {
            matches!(
                requirement(assertion_about(subject)),
                Requirement::Person(_)
            )
        };
        let names_repo = |subject: &str| {
            matches!(
                requirement(assertion_about(subject)),
                Requirement::Repo { .. }
            )
        };
        let malformed =
            |subject: &str| requirement(assertion_about(subject)) == Requirement::Malformed;

        let accounts = [
            "github:jeff-b".to_string(),
            "github:J3ff".to_string(),
            format!("github:{}", "a".repeat(39)),
        ];
        for subject in &accounts {
            assert!(names_account(subject), "{subject} names an account");
        }
        let not_accounts = [
            "github:".to_string(),
            "github:-jeff".to_string(),
            "github:jeff-".to_string(),
            "github:jeff_b".to_string(),
            "github:jeff.b".to_string(),
            format!("github:{}", "a".repeat(40)),
        ];
        for subject in &not_accounts {
            assert!(malformed(subject), "{subject} names no account");
        }
        let repos = [
            "github:jeffbailey/tide.pool_rs-2".to_string(),
            "github:jeffbailey/.github".to_string(),
            format!("github:jeffbailey/{}", "r".repeat(100)),
        ];
        for subject in &repos {
            assert!(names_repo(subject), "{subject} names a repo");
        }
        let not_repos = [
            "github:jeffbailey/".to_string(),
            "github:jeffbailey/.".to_string(),
            "github:jeffbailey/..".to_string(),
            "github:jeffbailey/tide$pool".to_string(),
            "github:jeffbailey/tide pool".to_string(),
            format!("github:jeffbailey/{}", "r".repeat(101)),
            "github:-jeff/tidepool".to_string(),
            "github:jeff_b/tidepool".to_string(),
        ];
        for subject in &not_repos {
            assert!(malformed(subject), "{subject} names no repo");
        }
    }

    fn observed(finding: RepoFinding, age: Duration) -> LatestRepoObservation {
        LatestRepoObservation {
            finding,
            checked_at: now() - age,
        }
    }

    fn found(owner_kind: RepoOwnerKind, authority_file: AuthorityFileSeen) -> RepoFinding {
        RepoFinding::Found {
            owner_id: 1,
            owner_kind,
            authority_file,
        }
    }

    fn listing(dids: &[&str]) -> AuthorityFileSeen {
        AuthorityFileSeen::Present(dids.iter().map(|did| did.to_string()).collect())
    }

    // bypass: one example per repo tier and reason (ADR-087 §2-3).
    #[test]
    fn a_reader_labels_a_repo_claim_from_the_latest_observation_and_the_authors_links() {
        use RepoOwnerKind::{Organization, User};
        let verified_to = |id| {
            link(
                id,
                "jeffbailey",
                Some(("jeffbailey", CheckVerdict::Verified)),
            )
        };
        let to_owner_verified = [verified_to(1)];
        let to_other_verified = [verified_to(2)];
        let to_owner_not_verified = [link(
            1,
            "jeffbailey",
            Some(("jeffbailey", CheckVerdict::DifferentDid)),
        )];
        let to_owner_unchecked = [link(1, "jeffbailey", None)];
        let owner = Authority::Attested(AttestedTier::Owner { github_id: 1 });
        let listed = Authority::Attested(AttestedTier::RepoListed);
        let cases: [(&[LinkedAccountEvidence], Option<RepoFinding>, Authority); 11] = [
            (
                &to_owner_verified,
                Some(found(User, AuthorityFileSeen::Absent)),
                owner,
            ),
            (
                &to_other_verified,
                Some(found(User, AuthorityFileSeen::Absent)),
                Authority::Unattested(Unattested::NotRepoOwner),
            ),
            (
                &to_owner_not_verified,
                Some(found(User, AuthorityFileSeen::Absent)),
                Authority::Unattested(Unattested::LinkNotVerified),
            ),
            (
                &to_owner_unchecked,
                Some(found(User, AuthorityFileSeen::Absent)),
                Authority::NotChecked,
            ),
            (
                &to_owner_not_verified,
                Some(found(User, listing(&[JEFF]))),
                listed,
            ),
            (
                &to_owner_verified,
                Some(found(Organization, AuthorityFileSeen::Absent)),
                Authority::Unattested(Unattested::OrgOwnedNotListed),
            ),
            (&[], Some(found(Organization, listing(&[JEFF]))), listed),
            (
                &[],
                Some(found(
                    Organization,
                    listing(&["did:plc:rachel", "did:plc:jeff2"]),
                )),
                Authority::Unattested(Unattested::OrgOwnedNotListed),
            ),
            (
                &[],
                Some(found(Organization, AuthorityFileSeen::Invalid)),
                Authority::Unattested(Unattested::FileInvalid),
            ),
            (
                &to_owner_verified,
                Some(RepoFinding::NotFound),
                Authority::Unattested(Unattested::RepoNotFound),
            ),
            (&to_owner_verified, None, Authority::NotChecked),
        ];
        for (links, finding, expected) in cases {
            let observation = finding.map(|finding| observed(finding, Duration::days(1)));
            let evidence = AuthorityEvidence {
                author: JEFF,
                author_links: links,
                repo: observation.as_ref(),
            };
            assert_eq!(
                classify(
                    assertion_about("github:jeffbailey/tidepool"),
                    &evidence,
                    now()
                ),
                Some(expected),
                "{links:?} {observation:?}"
            );
        }
    }

    // bypass: the repo tier honours the signing window to the minute.
    #[test]
    fn a_repo_permit_rests_on_an_observation_within_the_signing_window() {
        let subject = "github:acme-tools/anvil";
        let file = found(RepoOwnerKind::Organization, listing(&[JEFF]));
        let at_edge = observed(file.clone(), Duration::minutes(WINDOW_MINUTES));
        assert_eq!(
            may_sign_repo_from_checks(JEFF, subject, &[], Some(&at_edge), now()).map(|p| p.basis()),
            Ok(PermitBasis::RepoListed)
        );
        let too_old = observed(file, Duration::minutes(WINDOW_MINUTES + 1));
        assert_eq!(
            may_sign_repo_from_checks(JEFF, subject, &[], Some(&too_old), now()),
            Err(AuthorityRefusal::Recheck {
                subject: subject.to_string(),
                last_checked: Some(too_old.checked_at),
            })
        );
    }

    // bypass: an organization repo is signed only by a DID its file lists.
    #[test]
    fn only_a_listed_did_can_sign_about_an_organization_repo() {
        let subject = "github:acme-tools/anvil";
        let org_repo = observed(
            found(RepoOwnerKind::Organization, AuthorityFileSeen::Absent),
            Duration::days(1),
        );
        assert_eq!(
            may_sign_repo_from_checks(JEFF, subject, &[], Some(&org_repo), now()),
            Err(AuthorityRefusal::Unattested {
                subject: subject.to_string(),
                reason: Unattested::OrgOwnedNotListed,
                can_sign: WhoCanSign::ListedInAuthorityFile,
            })
        );
    }

    // bypass: re-publishing needs no fresh authority; first publishing does.
    #[test]
    fn only_a_claim_already_published_here_gets_a_republish_permit() {
        let subject = "github:jeffbailey/tidepool";
        let permit = republish_permit(JEFF, subject, "bafyone", Some(now()));
        assert_eq!(
            permit
                .as_ref()
                .map(|p| (p.signer(), p.subject(), p.basis())),
            Some((JEFF, subject, PermitBasis::Exempt))
        );
        assert_eq!(republish_permit(JEFF, subject, "bafyone", None), None);
    }

    // bypass (U-OOS-D31): a re-publish permit covers the one claim it was
    // minted for, by CID, and no other claim about the same subject.
    #[test]
    fn a_republish_permit_covers_only_the_claim_it_was_minted_for() {
        let subject = "github:jeffbailey/tidepool";
        let permit = republish_permit(JEFF, subject, "bafyone", Some(now())).expect("published");
        assert_eq!(permit.claim(), Some("bafyone"));
        assert_eq!(
            permit.covers(JEFF, assertion_about(subject), "bafyone"),
            Ok(())
        );
        assert_eq!(
            permit.covers(JEFF, assertion_about(subject), "bafytwo"),
            Err(PermitMismatch::OtherClaim {
                permitted: "bafyone".to_string()
            })
        );
    }

    // bypass (U-OOS-D31): an exempt permit minted for a claim that needs no
    // authority covers no assertion about a GitHub subject.
    #[test]
    fn an_exempt_permit_covers_no_assertion_needing_authority() {
        let subject = "github:jeffbailey";
        let withdrawal = ClaimShape {
            subject,
            predicate: "adheresToPhilosophy",
            reference_types: &[ReferenceType::Retracts],
        };
        let permit = exempt_permit(JEFF, withdrawal).expect("a withdrawal is exempt");
        assert_eq!(permit.claim(), None);
        assert_eq!(permit.covers(JEFF, withdrawal, "bafyany"), Ok(()));
        assert_eq!(
            permit.covers(JEFF, assertion_about(subject), "bafyany"),
            Err(PermitMismatch::NeedsAuthority)
        );
    }

    // bypass (U-OOS-D31): every permit covers only its own signer and subject.
    #[test]
    fn a_permit_covers_only_its_signer_and_subject() {
        let permit = may_sign_from_checks(
            JEFF,
            &person("github:jeffbailey"),
            &[link(
                7,
                "jeffbailey",
                Some(("jeffbailey", CheckVerdict::Verified)),
            )],
            now(),
        )
        .expect("the owner may sign");
        let about_jeff = assertion_about("github:jeffbailey");
        assert_eq!(permit.covers(JEFF, about_jeff, "bafyany"), Ok(()));
        assert_eq!(
            permit.covers("did:plc:maria", about_jeff, "bafyany"),
            Err(PermitMismatch::OtherSigner {
                permitted: JEFF.to_string()
            })
        );
        assert_eq!(
            permit.covers(JEFF, assertion_about("github:rachelk"), "bafyany"),
            Err(PermitMismatch::OtherSubject {
                permitted: "github:jeffbailey".to_string()
            })
        );
    }

    // bypass: the reader labels are spelled once, here.
    #[test]
    fn each_authority_reads_under_its_own_label() {
        let labels = [
            (
                Authority::Attested(AttestedTier::Owner { github_id: 1 }),
                "owner",
            ),
            (Authority::Attested(AttestedTier::RepoListed), "repo-listed"),
            (Authority::Unattested(Unattested::NoLink), "third-party"),
            (Authority::NotChecked, "authority not checked"),
        ];
        for (authority, label) in labels {
            assert_eq!(authority.label(), label, "{authority:?}");
        }
    }
}

#[cfg(test)]
mod authority_file_tests {
    use super::*;

    #[test]
    fn an_authority_file_lists_one_did_per_line_and_skips_comments_and_other_lines() {
        let file = "# who may sign\ndid:plc:jeff\n\n  did:web:example.com  # release\nnot a did\ndid:plc:a did:plc:b\n";

        assert_eq!(
            authority_file_dids(file.as_bytes()),
            Some(vec![
                "did:plc:jeff".to_string(),
                "did:web:example.com".to_string()
            ])
        );
    }

    #[test]
    fn a_file_over_four_kibibytes_or_not_utf8_lists_nothing_at_all() {
        let at_limit = format!(
            "did:plc:jeff\n{}",
            "#".repeat(AUTHORITY_FILE_MAX_BYTES - 13)
        );
        assert_eq!(at_limit.len(), AUTHORITY_FILE_MAX_BYTES);
        assert_eq!(
            authority_file_dids(at_limit.as_bytes()),
            Some(vec!["did:plc:jeff".to_string()])
        );

        let over_limit = format!("{at_limit}#");
        assert_eq!(authority_file_dids(over_limit.as_bytes()), None);
        assert_eq!(authority_file_dids(b"did:plc:jeff\n\xff\xfe"), None);
    }

    // bypass: the limit is ADR-087 §3's 4 KiB, stated without the constant.
    #[test]
    fn a_four_kibibyte_file_still_grants() {
        let file = format!("did:plc:jeff\n{}", "#".repeat(4096 - 13));
        assert_eq!(
            authority_file_dids(file.as_bytes()),
            Some(vec!["did:plc:jeff".to_string()])
        );
    }
}
