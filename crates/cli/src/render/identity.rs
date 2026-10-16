//! `openlore identity list` — the reader's linked identities (ADR-085 §5).

use chrono::{DateTime, Utc};
use claim_domain::identity_link::{CheckVerdict, LinkStanding, LinkedIdentity};
use claim_domain::UnsignedClaim;

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

/// What a verdict means, said plainly.
fn verdict_plainly(verdict: CheckVerdict) -> &'static str {
    match verdict {
        CheckVerdict::Verified => "the bio names the DID",
        CheckVerdict::DidMissing => "the bio names no DID",
        CheckVerdict::DifferentDid => "the bio names someone else's DID",
        CheckVerdict::NoBio => "the account has no bio",
        CheckVerdict::AccountNotFound => "the account no longer exists",
    }
}

/// `identity link` refused: the fresh read did not prove the link.
pub fn render_link_refused(login: &str, verdict: CheckVerdict) -> String {
    format!(
        "github:{login} is not linked ({}): {}; nothing published, nothing recorded",
        verdict.token(),
        verdict_plainly(verdict)
    )
}

/// `identity link` refused before GitHub was asked: the login is malformed.
pub fn render_not_a_github_login(login: &str) -> String {
    format!("{login:?} is not a GitHub username; GitHub was not asked, nothing recorded")
}

/// `identity link` failed closed: GitHub could not be read. `reason` never
/// carries the token (ADR-085 §4).
pub fn render_github_unreadable(login: &str, reason: &str) -> String {
    format!(
        "GitHub could not be read for github:{login} ({reason}); not linked, nothing published, nothing recorded"
    )
}

/// The link preview: the exact public record a confirmation would publish
/// and what retracting it would do (ADR-085 §7).
pub fn render_link_preview(claim: &UnsignedClaim) -> String {
    format!(
        "Link preview (asserted by you, not as truth)\n  \
         record:   {} {} {}\n  \
         evidence: {}\n\
         This link is public: it is published to your repo for anyone to read.\n\
         Retracting it later withdraws it but does not erase it.\n",
        claim.subject,
        claim.predicate,
        claim.object,
        claim.evidence.join(", ")
    )
}

/// `identity link` declined at the confirmation.
pub fn render_link_declined() -> String {
    "Not linked; nothing published, nothing recorded.\n".to_string()
}

/// `identity link` refused: the account is already actively linked (D-a).
pub fn render_already_linked(login: &str) -> String {
    format!(
        "github:{login} is already linked to your DID; to re-check it run `openlore identity verify`. Nothing signed, published or recorded"
    )
}

/// `identity link` signed and verified the link but its repo host could not
/// be reached: the link and its check are kept locally (ADR-085 §7, Q-GDL-D1).
pub fn render_link_kept_locally(login: &str, cid: &str, reason: &str) -> String {
    format!(
        "github:{login} is linked and verified locally, but your repo host could not be reached ({reason}); \
         nothing published. The signed link is kept: retry with `openlore claim publish {cid}` once it is reachable"
    )
}
