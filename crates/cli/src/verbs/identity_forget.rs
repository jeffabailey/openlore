//! `identity forget --did <did>` — erase one DID's LOCAL proof-check history
//! (C-GDL-5, ADR-085 §7).
//!
//! The verb holds the purge handle and nothing else: no `Wiring`, no check
//! read or write, no claim, publish, signing or GitHub handle — so it cannot
//! change a claim. Check-arch `identity_link_purge_only_in_forget_verb` keeps
//! the purge out of every other verb.

use anyhow::Result;
use claim_domain::{bare_did, Did};
use ports::IdentityLinkCheckPurgePort;

use crate::render::render_forgot;

/// Run `identity forget`: purge the checks of `did`'s bare form; returns the
/// stdout to print.
pub fn run(purge: &dyn IdentityLinkCheckPurgePort, did: &Did) -> Result<String> {
    let subject_did = bare_did(&did.0);
    let forgotten = purge.purge_subject(subject_did)?;
    Ok(render_forgot(subject_did, forgotten))
}
