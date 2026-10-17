//! `identity list` — show linked identities OFFLINE from the last local
//! check (ADR-085 §6/§7).
//!
//! A reader by construction (C-GDL-9): the verb receives only the check-read,
//! link-candidate and lineage read handles — never the whole `Wiring`, no
//! check write or purge, no signing, no GitHub. The link lineage is read as
//! `identity verify` reads it (`identity_reads`), so a link its author
//! retracted or superseded is never listed (ADR-085 §5, ADR-060). The read
//! itself is `identity_reads::read_listed_links`, shared with the person views.
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
use claim_domain::identity_link::RecheckScope;
use ports::{IdentityLinkCheckReadPort, LinkCandidateReadPort, LinkLineageReadPort};

use crate::render::{render_linked_identities, render_no_linked_identities};
use crate::verbs::identity_reads::read_listed_links;

/// The narrow read handles `identity list` holds.
pub struct IdentityListHandles<'a> {
    pub checks: &'a dyn IdentityLinkCheckReadPort,
    pub claims: &'a dyn LinkCandidateReadPort,
    pub lineage: &'a dyn LinkLineageReadPort,
}

/// Run `identity list` within `scope` as of `now`; returns the stdout to print.
pub fn run(
    handles: &IdentityListHandles<'_>,
    scope: &RecheckScope,
    now: DateTime<Utc>,
) -> Result<String> {
    let listed = read_listed_links(handles.checks, handles.claims, handles.lineage, scope, now)?;
    if listed.is_empty() {
        return Ok(render_no_linked_identities());
    }
    Ok(render_linked_identities(&listed, now))
}
