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
//! PURE: no clock, no store, no network.

use crate::identity_link::{bio_verdict, BioVerdict};

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
