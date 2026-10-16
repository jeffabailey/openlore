//! `identity list` — show linked identities OFFLINE from the last local
//! check (ADR-085 §6/§7).
//!
//! A reader by construction (C-GDL-9): the verb receives only the check-read
//! and link-candidate read handles — never the whole `Wiring`, no check write
//! or purge, no signing, no GitHub. Candidate rows are selected with the
//! `claim_domain::identity_link` predicate API (Q-RM-5); the projection
//! decides what is a link.

use anyhow::{bail, Result};
use claim_domain::identity_link::link_predicate;
use ports::{IdentityLinkCheckReadPort, LinkCandidateReadPort};

use crate::render::render_no_linked_identities;

/// The narrow read handles `identity list` holds.
pub struct IdentityListHandles<'a> {
    pub checks: &'a dyn IdentityLinkCheckReadPort,
    pub claims: &'a dyn LinkCandidateReadPort,
}

/// Run `identity list`; returns the stdout to print.
pub fn run(handles: &IdentityListHandles<'_>) -> Result<String> {
    let candidates = handles.claims.link_candidates(link_predicate())?;
    if candidates.is_empty() {
        return Ok(render_no_linked_identities());
    }
    // SCAFFOLD: the link projection (`identity_link::project` + `link_view`
    // over `handles.checks`) lands with the first linked-identity scenario;
    // until then the verb fails closed rather than misreport stored links.
    let _ = handles.checks;
    bail!(
        "{} stored identity-link claim(s) cannot be shown yet",
        candidates.len()
    )
}
