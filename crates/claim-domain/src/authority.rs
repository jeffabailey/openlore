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
//! PURE: no clock, no store, no network.

use crate::identity_link::{bio_verdict, BioVerdict, CheckVerdict};

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
    /// Not a `github:` subject: anyone may sign.
    Exempt,
    /// `github:<login>`: only the account's owner may sign.
    Person(PersonSubject),
    /// `github:<owner>/<repo>`: the repo's owner or a DID its authority file lists.
    Repo { owner: String, name: String },
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

/// Which authority an assertion about `subject` needs. The `github` scheme is
/// matched without letter case; any `github:` subject is Required, never
/// Exempt — a malformed one fails closed.
pub fn requirement(subject: &str) -> Requirement {
    match subject.split_once(':') {
        Some((scheme, path)) if scheme.eq_ignore_ascii_case("github") => match path.split_once('/')
        {
            Some((owner, name)) => Requirement::Repo {
                owner: owner.to_string(),
                name: name.to_string(),
            },
            None => Requirement::Person(PersonSubject {
                uri: subject.to_string(),
                login: path.to_string(),
            }),
        },
        _ => Requirement::Exempt,
    }
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
    /// the subject login.
    Attested {
        github_id: u64,
    },
    Unattested(Unattested),
}

/// Does the signer own the account `person` names? Only when one of `links`
/// has a latest check that is verified AND saw the subject login (letter case
/// aside, as GitHub compares logins). Every verified link counts, so a
/// contested account still attests each verified DID (UC-2).
pub fn classify_person(person: &PersonSubject, links: &[LinkedAccountEvidence]) -> PersonAuthority {
    links
        .iter()
        .filter_map(|link| link_standing(person.login(), link))
        .min_by_key(standing_rank)
        .unwrap_or(PersonAuthority::Unattested(Unattested::NoLink))
}

/// What one link says about `login`; `None` when it names another account.
fn link_standing(login: &str, link: &LinkedAccountEvidence) -> Option<PersonAuthority> {
    let names_login = |other: &str| other.eq_ignore_ascii_case(login);
    match &link.latest_check {
        Some(check) if names_login(&check.login_seen) => Some(match check.verdict {
            CheckVerdict::Verified => PersonAuthority::Attested {
                github_id: link.github_id,
            },
            _ => PersonAuthority::Unattested(Unattested::LinkNotVerified),
        }),
        Some(_) if names_login(&link.linked_login) => {
            Some(PersonAuthority::Unattested(Unattested::LoginNotCurrent))
        }
        None if names_login(&link.linked_login) => {
            Some(PersonAuthority::Unattested(Unattested::LinkNotVerified))
        }
        _ => None,
    }
}

/// The strongest standing wins: attested, then the reasons nearest to it.
fn standing_rank(standing: &PersonAuthority) -> u8 {
    match standing {
        PersonAuthority::Attested { .. } => 0,
        PersonAuthority::Unattested(Unattested::LinkNotVerified) => 1,
        PersonAuthority::Unattested(Unattested::LoginNotCurrent) => 2,
        PersonAuthority::Unattested(Unattested::NoLink) => 3,
    }
}

/// Who the hand-off names as able to sign (ADR-086 §10).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WhoCanSign {
    /// The account's owner, after linking it to their identity.
    OwnerAfterLinking { login: String },
}

/// Why no permit was minted from local checks, with who can sign instead.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthorityRefusal {
    /// The subject as signed.
    pub subject: String,
    pub reason: Unattested,
    pub can_sign: WhoCanSign,
}

/// The CLI gate (ADR-086 §4): may `signer` sign an assertion about `person`,
/// judged from this reader's local checks of the signer's `links`? Mints the
/// same [`SigningPermit`] as [`may_sign`]; refuses with the hand-off data.
pub fn may_sign_from_checks(
    signer: &str,
    person: &PersonSubject,
    links: &[LinkedAccountEvidence],
) -> Result<SigningPermit, AuthorityRefusal> {
    match classify_person(person, links) {
        PersonAuthority::Attested { github_id } => Ok(SigningPermit {
            signer: signer.to_string(),
            subject: person.uri().to_string(),
            basis: PermitBasis::Owner(github_id),
        }),
        PersonAuthority::Unattested(reason) => Err(AuthorityRefusal {
            subject: person.uri().to_string(),
            reason,
            can_sign: WhoCanSign::OwnerAfterLinking {
                login: person.login().to_string(),
            },
        }),
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
    use proptest::prelude::*;

    const JEFF: &str = "did:plc:jeff";

    fn person(subject: &str) -> PersonSubject {
        match requirement(subject) {
            Requirement::Person(person) => person,
            other => panic!("{subject} is a person subject, got {other:?}"),
        }
    }

    fn link(
        github_id: u64,
        linked: &str,
        check: Option<(&str, CheckVerdict)>,
    ) -> LinkedAccountEvidence {
        LinkedAccountEvidence {
            github_id,
            linked_login: linked.to_string(),
            latest_check: check.map(|(login_seen, verdict)| LatestLinkCheck {
                login_seen: login_seen.to_string(),
                verdict,
            }),
        }
    }

    fn verdict() -> impl Strategy<Value = CheckVerdict> {
        proptest::sample::select(CheckVerdict::ALL.to_vec())
    }

    fn login() -> impl Strategy<Value = String> {
        proptest::sample::select(vec!["jeffbailey", "JeffBailey", "jeff-b", "rachelk"])
            .prop_map(str::to_string)
    }

    fn evidence() -> impl Strategy<Value = LinkedAccountEvidence> {
        (1u64..4, login(), proptest::option::of((login(), verdict()))).prop_map(
            |(github_id, linked_login, check)| LinkedAccountEvidence {
                github_id,
                linked_login,
                latest_check: check.map(|(login_seen, verdict)| LatestLinkCheck {
                    login_seen,
                    verdict,
                }),
            },
        )
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(256))]

        /// A permit exists iff some link's LATEST check is verified and saw
        /// the subject login; its basis is that account. A refusal names the
        /// subject and hands off to the subject login's owner.
        #[test]
        fn a_permit_needs_a_verified_latest_check_that_saw_the_subject_login(
            subject_login in login(),
            links in proptest::collection::vec(evidence(), 0..4),
        ) {
            let subject = format!("github:{subject_login}");
            let owners: Vec<u64> = links
                .iter()
                .filter(|l| l.latest_check.as_ref().is_some_and(|c| {
                    c.verdict == CheckVerdict::Verified
                        && c.login_seen.eq_ignore_ascii_case(&subject_login)
                }))
                .map(|l| l.github_id)
                .collect();
            match may_sign_from_checks(JEFF, &person(&subject), &links) {
                Ok(permit) => {
                    let PermitBasis::Owner(id) = permit.basis();
                    prop_assert!(owners.contains(&id), "{links:?}");
                    prop_assert_eq!(permit.subject(), subject.as_str());
                    prop_assert_eq!(permit.signer(), JEFF);
                }
                Err(refusal) => {
                    prop_assert!(owners.is_empty(), "{links:?}");
                    prop_assert_eq!(refusal.subject, subject);
                    prop_assert_eq!(
                        refusal.can_sign,
                        WhoCanSign::OwnerAfterLinking { login: subject_login }
                    );
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
            let required = requirement(&format!("{scheme}:{path}"));
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
                vec![link(1, "jeffbailey", None)],
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
            let refused = may_sign_from_checks(JEFF, &jeff, &links).unwrap_err();
            assert_eq!(refused.reason, reason, "{links:?}");
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
        let permit = may_sign_from_checks(JEFF, &person("github:jeff-b"), &renamed);
        assert_eq!(permit.map(|p| p.basis()), Ok(PermitBasis::Owner(7)));
    }

    // bypass: a repo subject names its owner and name.
    #[test]
    fn a_repo_subject_requires_repo_authority() {
        assert_eq!(
            requirement("github:jeffbailey/tidepool"),
            Requirement::Repo {
                owner: "jeffbailey".to_string(),
                name: "tidepool".to_string()
            }
        );
    }
}
