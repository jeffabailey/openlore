//! `identity list` — show linked identities OFFLINE from the last local
//! check (ADR-085 §6/§7).
//!
//! A reader by construction (C-GDL-9): the verb receives only the check-read,
//! link-candidate and lineage read handles — never the whole `Wiring`, no
//! check write or purge, no signing, no GitHub. The link lineage is read as
//! `identity verify` reads it (`identity_reads`), so a link its author
//! retracted or superseded is never listed (ADR-085 §5, ADR-060).

use anyhow::Result;
use chrono::{DateTime, Utc};
use claim_domain::identity_link::{link_view, LatestCheck, ReaderLink};
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

/// Run `identity list` as of `now`; returns the stdout to print.
pub fn run(handles: &IdentityListHandles<'_>, now: DateTime<Utc>) -> Result<String> {
    let links: Vec<ReaderLink> =
        StoredLinkLineage::read(handles.claims, handles.lineage)?.reader_links();
    if links.is_empty() {
        return Ok(render_no_linked_identities());
    }
    let latest: Vec<LatestCheck> = handles
        .checks
        .latest_checks(&CheckFilter::All, now)?
        .into_iter()
        .map(latest_of)
        .collect();
    Ok(render_linked_identities(
        &link_view(&links, &latest, now),
        now,
    ))
}
