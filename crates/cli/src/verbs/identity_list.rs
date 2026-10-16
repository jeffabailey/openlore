//! `identity list` — show linked identities OFFLINE from the last local
//! check (ADR-085 §6/§7).
//!
//! A reader by construction (C-GDL-9): the verb receives only the check-read
//! and link-candidate read handles — never the whole `Wiring`, no check write
//! or purge, no signing, no GitHub. Candidate rows are selected with the
//! `claim_domain::identity_link` predicate API (Q-RM-5); the projection
//! decides what is a link.

use anyhow::Result;
use chrono::{DateTime, Utc};
use claim_domain::identity_link::{
    link_predicate, link_view, project, ClaimFields, IdentityLink, LatestCheck,
};
use ports::{
    CheckFilter, IdentityLinkCheckReadPort, LinkCandidate, LinkCandidateReadPort, LinkCheck,
};

use crate::render::{render_linked_identities, render_no_linked_identities};

/// The narrow read handles `identity list` holds.
pub struct IdentityListHandles<'a> {
    pub checks: &'a dyn IdentityLinkCheckReadPort,
    pub claims: &'a dyn LinkCandidateReadPort,
}

/// A stored candidate as a link, if it is one, under the provenance the row
/// was stored with (own: app-signed; peer: `peer_claims.provenance`).
fn projected(candidate: &LinkCandidate) -> Option<IdentityLink> {
    let fields = ClaimFields {
        author: &candidate.author_did,
        subject: &candidate.subject,
        predicate: &candidate.predicate,
        object: &candidate.object,
        evidence: &candidate.evidence,
    };
    project(fields, &candidate.provenance)
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
    let links: Vec<IdentityLink> = handles
        .claims
        .link_candidates(link_predicate())?
        .iter()
        .filter_map(projected)
        .collect();
    if links.is_empty() {
        return Ok(render_no_linked_identities());
    }
    let latest: Vec<LatestCheck> = handles
        .checks
        .latest_checks(&CheckFilter::All, now)?
        .into_iter()
        .map(latest_of)
        .collect();
    Ok(render_linked_identities(&link_view(&links, &latest), now))
}
