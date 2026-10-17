//! `identity list` — show linked identities OFFLINE from the last local
//! check (ADR-085 §6/§7).
//!
//! A reader by construction (C-GDL-9): the verb receives only the check-read,
//! link-candidate and lineage read handles — never the whole `Wiring`, no
//! check write or purge, no signing, no GitHub. The link lineage is read as
//! `identity verify` reads it (`identity_reads`), so a link its author
//! retracted or superseded is never listed (ADR-085 §5, ADR-060).
//!
//! `--did` narrows the list to one person (F7): links are selected by the
//! same bare-DID rule `identity verify --did` uses (`is_in_scope`), and only
//! that subject's checks are read. The scope selects lines; it never changes
//! a standing.
//!
//! A person's duplicate records for one account read as one line naming
//! every record ID, ascending (GDL-D9, U-9).

use anyhow::Result;
use chrono::{DateTime, Utc};
use claim_domain::identity_link::{
    is_in_scope, link_view, listed_links, LatestCheck, ReaderLink, RecheckScope,
};
use ports::{
    CheckFilter, IdentityLinkCheckReadPort, LinkCandidateReadPort, LinkCheck, LinkLineageReadPort,
};

use crate::render::{render_linked_identities, render_no_linked_identities};
use crate::verbs::identity_reads::StoredLinkLineage;

/// The narrow read handles `identity list` holds.
pub struct IdentityListHandles<'a> {
    pub checks: &'a dyn IdentityLinkCheckReadPort,
    pub claims: &'a dyn LinkCandidateReadPort,
    pub lineage: &'a dyn LinkLineageReadPort,
}

fn latest_of(check: LinkCheck) -> LatestCheck {
    LatestCheck {
        subject_did: check.subject_did,
        user_id: check.github_user_id,
        login_seen: check.login_seen,
        verdict: check.verdict,
        checked_at: check.checked_at,
    }
}

/// The checks a listing within `scope` needs: one person's, or everyone's.
fn check_filter_of(scope: &RecheckScope) -> CheckFilter {
    match scope {
        RecheckScope::Person(did) => CheckFilter::Subject(claim_domain::bare_did(did).to_string()),
        RecheckScope::Everyone | RecheckScope::Account(_) => CheckFilter::All,
    }
}

/// Run `identity list` within `scope` as of `now`; returns the stdout to print.
pub fn run(
    handles: &IdentityListHandles<'_>,
    scope: &RecheckScope,
    now: DateTime<Utc>,
) -> Result<String> {
    let lineage = StoredLinkLineage::read(handles.claims, handles.lineage)?;
    let links: Vec<ReaderLink> = lineage
        .reader_links()
        .into_iter()
        .filter(|reader| is_in_scope(scope, &reader.link, &reader.link.login))
        .collect();
    if links.is_empty() {
        return Ok(render_no_linked_identities());
    }
    let latest: Vec<LatestCheck> = handles
        .checks
        .latest_checks(&check_filter_of(scope), now)?
        .into_iter()
        .map(latest_of)
        .collect();
    Ok(render_linked_identities(
        &listed_links(
            link_view(&links, &latest, now),
            &lineage.active_link_records(),
        ),
        now,
    ))
}
