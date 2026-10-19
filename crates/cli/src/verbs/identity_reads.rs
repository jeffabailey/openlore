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
    is_in_scope, login_to_read, GithubObservation, IdentityLink, ProofCheck, RecheckScope,
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
