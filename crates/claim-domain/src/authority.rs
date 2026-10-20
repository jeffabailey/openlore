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

use crate::identity_link::{bio_verdict, link_predicate, BioVerdict, CheckVerdict};
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
    /// The repo's owner is the signer's listed GitHub account (its id).
    Owner(u64),
}

/// The signer may publish an assertion about `subject`. Minted only by
/// [`may_sign`]; never serialized, never stored.
#[derive(Debug, PartialEq, Eq)]
pub struct SigningPermit {
    signer: String,
    subject: String,
    basis: PermitBasis,
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
            BioVerdict::Verified => Ok(SigningPermit {
                signer: signer.to_string(),
                subject: subject.to_string(),
                basis: PermitBasis::Owner(owner_id),
            }),
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
        RepoRead::Found(facts)
            if !subject
                .strip_prefix("github:")
                .unwrap_or(subject)
                .eq_ignore_ascii_case(&facts.full_name) =>
        {
            Err(SigningRefusal::RepoRenamed {
                now: facts.full_name.clone(),
            })
        }
        _ => Ok(()),
    }
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

/// Why the signer holds no person authority (ADR-087 §1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unattested {
    /// None of the signer's links names the account.
    NoLink,
    /// The signer's link to the account is not verified by its latest check.
    LinkNotVerified,
    /// The subject login is no longer the login of the signer's linked account.
    LoginNotCurrent,
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

/// What one link says about `login`; `None` when it names another account.
fn link_standing(
    login: &str,
    link: &LinkedAccountEvidence,
    now: DateTime<Utc>,
) -> Option<PersonAuthority> {
    let names_login = |other: &str| other.eq_ignore_ascii_case(login);
    let current_check = link
        .latest_check
        .as_ref()
        .filter(|check| check.checked_at <= now);
    match current_check {
        Some(check) if names_login(&check.login_seen) => Some(match check.verdict {
            CheckVerdict::Verified => PersonAuthority::Attested {
                github_id: link.github_id,
                checked_at: check.checked_at,
            },
            _ => PersonAuthority::Unattested(Unattested::LinkNotVerified),
        }),
        Some(_) if names_login(&link.linked_login) => {
            Some(PersonAuthority::Unattested(Unattested::LoginNotCurrent))
        }
        None if names_login(&link.linked_login) => Some(PersonAuthority::NotChecked),
        _ => None,
    }
}

/// The strongest standing wins: attested (the most recent check first), then
/// the reasons nearest to it.
fn standing_rank(standing: &PersonAuthority) -> (u8, Reverse<Option<DateTime<Utc>>>) {
    match standing {
        PersonAuthority::Attested { checked_at, .. } => (0, Reverse(Some(*checked_at))),
        PersonAuthority::NotChecked => (1, Reverse(None)),
        PersonAuthority::Unattested(Unattested::LinkNotVerified) => (2, Reverse(None)),
        PersonAuthority::Unattested(Unattested::LoginNotCurrent) => (3, Reverse(None)),
        PersonAuthority::Unattested(Unattested::NoLink) => (4, Reverse(None)),
    }
}

/// Who the hand-off names as able to sign (ADR-086 §10).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WhoCanSign {
    /// The account's owner, after linking it to their identity.
    OwnerAfterLinking { login: String },
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
        } if within_signing_window(checked_at, now) => Ok(SigningPermit {
            signer: signer.to_string(),
            subject,
            basis: PermitBasis::Owner(github_id),
        }),
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

/// A check made at `checked_at` (not after `now`) is at most
/// [`SIGNING_WINDOW_DAYS`] old, counted in whole minutes.
fn within_signing_window(checked_at: DateTime<Utc>, now: DateTime<Utc>) -> bool {
    (now - checked_at).num_minutes() <= SIGNING_WINDOW_DAYS * 24 * 60
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
    /// whose current check saw the login unverified is not verified; a link
    /// checked since under another login names a login that is no longer
    /// current; no link naming the login is no link at all.
    fn adr_087_reason(login: &str, links: &[LinkedAccountEvidence]) -> Unattested {
        let same = |other: &str| other.eq_ignore_ascii_case(login);
        let not_verified = links.iter().any(|l| {
            current_check(l)
                .is_some_and(|c| same(&c.login_seen) && c.verdict != CheckVerdict::Verified)
        });
        let renamed_since = links.iter().any(|l| {
            same(&l.linked_login) && current_check(l).is_some_and(|c| !same(&c.login_seen))
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
            match may_sign_from_checks(JEFF, &person(&subject), &links, now()) {
                Ok(permit) => {
                    let PermitBasis::Owner(id) = permit.basis();
                    prop_assert!(fresh.contains(&id), "{links:?}");
                    prop_assert_eq!(permit.subject(), subject.as_str());
                    prop_assert_eq!(permit.signer(), JEFF);
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
        ];
        for (links, reason) in cases {
            let refused = may_sign_from_checks(JEFF, &jeff, &links, now()).unwrap_err();
            let AuthorityRefusal::Unattested { reason: got, .. } = refused else {
                panic!("{links:?} is unattested, got {refused:?}");
            };
            assert_eq!(got, reason, "{links:?}");
        }
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
}
