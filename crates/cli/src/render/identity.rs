//! `openlore identity list` — the reader's linked identities (ADR-085 §5).

use chrono::{DateTime, Utc};
use claim_domain::identity_link::{LinkStanding, LinkedIdentity};

/// The empty state of `identity list` (Q-GDL-D1, content-frozen wording).
pub const NO_LINKED_IDENTITIES: &str = "No linked identities";

/// Render `identity list` when no stored claim is a link candidate.
pub fn render_no_linked_identities() -> String {
    format!("{NO_LINKED_IDENTITIES}.\n")
}

/// How long ago a check ran, in whole days (`today`, `1 day ago`, `N days ago`).
fn check_age(checked_at: DateTime<Utc>, now: DateTime<Utc>) -> String {
    match (now - checked_at).num_days() {
        days if days <= 0 => "today".to_string(),
        1 => "1 day ago".to_string(),
        days => format!("{days} days ago"),
    }
}

/// One linked identity, as one line.
fn render_linked_identity(identity: &LinkedIdentity, now: DateTime<Utc>) -> String {
    let did = &identity.link.did;
    let account = format!("github:{}", identity.link.login);
    match &identity.standing {
        LinkStanding::Associated { checked_at } => {
            format!(
                "{did} is {account} — verified {}",
                check_age(*checked_at, now)
            )
        }
        LinkStanding::NotChecked => format!("{did} claims {account} — not checked"),
        LinkStanding::ProofGone {
            verdict,
            checked_at,
        } => format!(
            "{did} claims {account} — proof gone ({}), checked {}",
            verdict.token(),
            check_age(*checked_at, now)
        ),
        LinkStanding::Contested => format!("{account} is contested — {did} claims it"),
    }
}

/// Render `identity list`: one line per linked identity.
pub fn render_linked_identities(identities: &[LinkedIdentity], now: DateTime<Utc>) -> String {
    identities
        .iter()
        .map(|identity| format!("{}\n", render_linked_identity(identity, now)))
        .collect()
}
