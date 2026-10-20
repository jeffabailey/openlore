//! The reads the identity verbs share (ADR-085 §4/§7): the stored link
//! lineage (`ports::StoredLinkLineage`, read through the candidate and
//! lineage READ ports only) and a GitHub profile as the observation a check
//! consumes. On top of that lineage: the links a re-check covers (`identity
//! verify`, the person-scrape refresh) within a `RecheckScope`. The links a
//! listing shows are `ports::read_listed_links`, shared with the viewer.
//!
//! The decisions (which candidates are active links, what a read proves) stay
//! in `claim_domain::identity_link`; this module only reads and adapts.

use anyhow::Result;
use chrono::{DateTime, Utc};
use claim_domain::identity_link::{
    is_github_login, is_in_scope, login_to_read, GithubObservation, IdentityLink, ProofCheck,
    RecheckScope,
};
use ports::{
    read_latest_checks, GithubAccountProfile, IdentityLinkCheckReadPort, LinkCandidateReadPort,
    LinkCheck, LinkLineageReadPort, StoredLinkLineage,
};

/// The observation a fresh strict account read makes — only a profile
/// naming an account (`read_account` / `read_person_by_id`) is observed.
pub(crate) fn observation_of(account: GithubAccountProfile) -> GithubObservation {
    let profile = account.into_profile();
    GithubObservation {
        login: profile.login,
        user_id: profile.id,
        bio: profile.bio,
    }
}

/// One active link a re-check covers, with the login to read for it.
pub(crate) struct LinkToRecheck {
    pub link: IdentityLink,
    pub login: String,
}

/// The active links within `scope` as of `now`, once per (DID, account id),
/// each with the login to read for it (`identity verify`, the person scrape).
pub(crate) fn links_in_scope(
    checks: &dyn IdentityLinkCheckReadPort,
    claims: &dyn LinkCandidateReadPort,
    lineage: &dyn LinkLineageReadPort,
    scope: &RecheckScope,
    now: DateTime<Utc>,
) -> Result<Vec<LinkToRecheck>> {
    let latest = read_latest_checks(checks, now)?;
    Ok(StoredLinkLineage::read(claims, lineage)?
        .links_to_recheck()
        .into_iter()
        .filter_map(|link| {
            let login = login_to_read(&link, &latest).to_string();
            is_in_scope(scope, &link, &login).then_some(LinkToRecheck { link, login })
        })
        .collect())
}

/// The signer's own links to the account `login` names — what a sign-time
/// refresh re-checks (OOS-D7). Another DID's link to that account, and the
/// signer's links to other accounts, are never refreshed; a login that is no
/// GitHub login is never sent.
pub(crate) fn signers_links_to_account(
    signer_did: &str,
    login: &str,
    links: Vec<LinkToRecheck>,
) -> Vec<LinkToRecheck> {
    let signer = RecheckScope::Person(signer_did.to_string());
    let account = RecheckScope::Account(login.to_string());
    links
        .into_iter()
        .filter(|to_recheck| {
            is_in_scope(&signer, &to_recheck.link, &to_recheck.login)
                && is_in_scope(&account, &to_recheck.link, &to_recheck.login)
                && is_github_login(&to_recheck.login)
        })
        .collect()
}

/// The one check a decided re-check appends.
pub(crate) fn check_of(link: &IdentityLink, decided: &ProofCheck, now: DateTime<Utc>) -> LinkCheck {
    LinkCheck {
        subject_did: link.did.clone(),
        github_user_id: link.user_id,
        login_seen: decided.login_seen().to_string(),
        verdict: decided.verdict(),
        checked_at: now,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    const SIGNER: &str = "did:plc:signersignersignersigner";
    const PEER: &str = "did:plc:peerpeerpeerpeerpeerpeer";

    fn to_recheck(did: &str, login: &str, last_seen: &str, user_id: u64) -> LinkToRecheck {
        LinkToRecheck {
            link: IdentityLink {
                did: did.to_string(),
                login: login.to_string(),
                user_id,
            },
            login: last_seen.to_string(),
        }
    }

    fn summary(links: &[LinkToRecheck]) -> Vec<(String, u64)> {
        links
            .iter()
            .map(|l| (l.link.did.clone(), l.link.user_id))
            .collect()
    }

    proptest! {
        /// Of the active links, a sign-time refresh covers exactly the
        /// signer's own links to the account — signed or last seen under
        /// that login, in any letter case — never another DID's link to it
        /// nor the signer's links to other accounts.
        #[test]
        fn only_the_signers_own_link_to_the_account_is_refreshed(
            login in "[a-z][a-z0-9]{0,10}",
            other in "[a-z][a-z0-9]{0,10}",
            upper in any::<bool>(),
            renamed in any::<bool>(),
        ) {
            prop_assume!(!login.eq_ignore_ascii_case(&other));
            let asked = if upper { login.to_ascii_uppercase() } else { login.clone() };
            let (signed_as, seen_as) = if renamed {
                (other.as_str(), login.as_str())
            } else {
                (login.as_str(), login.as_str())
            };
            let links = vec![
                to_recheck(PEER, signed_as, seen_as, 1),
                to_recheck(SIGNER, signed_as, seen_as, 2),
                to_recheck(SIGNER, &other, &other, 3),
                to_recheck(PEER, &other, &other, 4),
            ];
            let refreshed = signers_links_to_account(SIGNER, &asked, links);
            prop_assert_eq!(summary(&refreshed), vec![(SIGNER.to_string(), 2)]);
        }
    }

    #[test]
    fn a_stored_login_that_is_no_github_login_is_never_refreshed() {
        let links = vec![to_recheck(SIGNER, "-bad", "-bad", 2)];
        assert!(signers_links_to_account(SIGNER, "-bad", links).is_empty());
    }
}
