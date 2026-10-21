//! The signing refusal and its hand-off (owner-only-signing, ADR-086 §5/§10):
//! why nothing was signed and who can sign instead — or the re-check that
//! lets the signer sign — rendered from the pure [`AuthorityRefusal`] data.
//! Wording pins: `tests/acceptance/support/authority.rs`.

use chrono::{DateTime, Utc};
use claim_domain::authority::{
    Authority, AuthorityRefusal, RepoSigningRefusal, Unattested, WhoCanSign, SIGNING_WINDOW_DAYS,
};

/// The fixed hand-off lead.
pub const CAN_BE_SIGNED_BY: &str = "Can be signed by";

/// The in-repo authority file, as refusals name it (ADR-087 §3).
const AUTHORITY_FILE: &str = ".github/openlore-authority";

/// Why the signer holds no authority over `subject`, as one sentence.
fn refusal_reason(subject: &str, reason: Unattested) -> String {
    match reason {
        Unattested::NoLink => format!("you have no identity link to {subject}"),
        Unattested::LinkNotVerified => {
            format!("your identity link is not verified for {subject}")
        }
        Unattested::LoginNotCurrent => {
            format!("{subject} is no longer the login of your linked GitHub account")
        }
        Unattested::NotRepoOwner => {
            format!("{subject} is not owned by your linked GitHub account")
        }
        Unattested::OrgOwnedNotListed => format!(
            "{subject} is owned by the organization and your DID is not listed in \
             {AUTHORITY_FILE}"
        ),
        Unattested::FileInvalid => {
            format!("{subject}'s {AUTHORITY_FILE} is not a valid authority file")
        }
        Unattested::RepoNotFound => format!("{subject} was not found on GitHub"),
    }
}

/// Who can sign instead, and the command that gets them there.
pub(crate) fn hand_off(can_sign: &WhoCanSign) -> String {
    match can_sign {
        WhoCanSign::OwnerAfterLinking { login } => {
            format!("{CAN_BE_SIGNED_BY} {login}, after `openlore identity link github {login}`.")
        }
        WhoCanSign::RepoOwnerOrListed { login } => format!(
            "{CAN_BE_SIGNED_BY} {login}, after `openlore identity link github {login}`, or by a \
             DID listed in the repo's {AUTHORITY_FILE}."
        ),
        WhoCanSign::ListedInAuthorityFile => {
            format!("{CAN_BE_SIGNED_BY} a DID listed in the repo's {AUTHORITY_FILE}.")
        }
    }
}

/// Why the local check does not let the signer sign about `subject` now.
fn recheck_reason(subject: &str, last_checked: Option<DateTime<Utc>>) -> String {
    match last_checked {
        Some(checked_at) => format!(
            "authority was last checked {} for {subject}, more than {SIGNING_WINDOW_DAYS} days ago",
            checked_at.format("%Y-%m-%d %H:%M UTC")
        ),
        None => format!("authority has not been checked here for {subject}"),
    }
}

/// Render a refused signature: nothing was signed or published, why, and the
/// hand-off or the re-check command.
pub fn render_signing_refused(refusal: &AuthorityRefusal) -> String {
    let (why, next) = match refusal {
        AuthorityRefusal::Unattested {
            subject,
            reason,
            can_sign,
        } => (refusal_reason(subject, *reason), hand_off(can_sign)),
        AuthorityRefusal::Recheck {
            subject,
            last_checked,
        } => (
            recheck_reason(subject, *last_checked),
            format!("Re-check it with `openlore authority verify --subject {subject}`, then sign again."),
        ),
    };
    format!("Not signed: {why}. Nothing was signed or published.\n{next}\n")
}

/// Render a refused repo signature: GitHub serves the repo under another
/// name now (sign it under that name), or the authority refusal.
pub fn render_repo_signing_refused(refusal: &RepoSigningRefusal) -> String {
    match refusal {
        RepoSigningRefusal::Renamed { subject, now } => format!(
            "Not signed: GitHub serves this repo as {now}, not {subject}. Nothing was signed or \
             published.\nSign the claim about github:{now} instead.\n"
        ),
        RepoSigningRefusal::Authority(refusal) => render_signing_refused(refusal),
    }
}

/// Render a refused repo candidate of an import (OOS-D12): the refusal with
/// the command the owner runs to sign it themselves — and, when GitHub could
/// not be asked about the repo in this invocation (`github_unanswered`: why),
/// that first, so a re-check names what stopped the fresh look.
pub fn render_import_signing_refused(
    refusal: &RepoSigningRefusal,
    github_unanswered: Option<&str>,
) -> String {
    let unanswered = match (refusal, github_unanswered) {
        (RepoSigningRefusal::Authority(AuthorityRefusal::Recheck { subject, .. }), Some(why)) => {
            format!("{}.\n", render_github_could_not_be_asked(subject, why))
        }
        _ => String::new(),
    };
    let owner_signs = match refusal {
        RepoSigningRefusal::Authority(AuthorityRefusal::Unattested { subject, .. }) => {
            render_owner_signs_with(subject)
        }
        _ => String::new(),
    };
    format!(
        "{unanswered}{}{owner_signs}",
        render_repo_signing_refused(refusal)
    )
}

/// The command a repo's owner (or a listed DID) runs to sign its imported
/// candidates (OOS-D12 hand-off).
fn render_owner_signs_with(subject: &str) -> String {
    let repo = subject
        .split_once(':')
        .map_or(subject, |(_, owner_and_name)| owner_and_name);
    format!("They would run `openlore scrape github {repo} --sign N`.\n")
}

/// `authority verify` with no `github:` claim in scope.
pub const NOTHING_TO_RECHECK: &str = "No GitHub claims to re-check";

/// Render `authority verify` when no `github:` claim is in scope.
pub fn render_nothing_to_recheck() -> String {
    format!("{NOTHING_TO_RECHECK}; GitHub was not asked.\n")
}

/// `authority verify` could not ask GitHub about one repo (rate limit,
/// outage, spent budget, rejected token, refused redirect): nothing was
/// recorded for it. `reason` is a `GithubError` rendering, which never
/// carries the token (US-SCR-004).
pub fn render_github_could_not_be_asked(subject: &str, reason: &str) -> String {
    format!("GitHub could not be asked about {subject} ({reason}); nothing was recorded for it")
}

/// One re-checked claim: who wrote it, about what, and its label now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LabelledClaim {
    pub subject: String,
    pub author: String,
    pub authority: Authority,
}

/// Render the labels `authority verify` decided, one `subject:` block per
/// subject (in the order given) with one `author:` line per claim. Nothing
/// is signed or published.
pub fn render_authority_labels(claims: &[LabelledClaim]) -> String {
    let blocks: Vec<String> = claims
        .chunk_by(|a, b| a.subject == b.subject)
        .map(|about_one_subject| {
            let rows: String = about_one_subject
                .iter()
                .map(|claim| format!("  author: {}  {}\n", claim.author, claim.authority.label()))
                .collect();
            format!("subject: {}\n{rows}", about_one_subject[0].subject)
        })
        .collect();
    format!(
        "Re-checked authority of {} GitHub claim(s); nothing was signed or published.\n\n{}",
        claims.len(),
        blocks.join("\n")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    // bypass: one example per closed refusal reason; wording pins live in the ATs.
    #[test]
    fn the_refusal_names_the_subject_why_and_the_link_command() {
        for reason in [
            Unattested::NoLink,
            Unattested::LinkNotVerified,
            Unattested::LoginNotCurrent,
        ] {
            let text = render_signing_refused(&AuthorityRefusal::Unattested {
                subject: "github:rachelk".to_string(),
                reason,
                can_sign: WhoCanSign::OwnerAfterLinking {
                    login: "rachelk".to_string(),
                },
            });
            assert!(text.contains("github:rachelk"), "{text}");
            assert!(
                text.contains(
                    "Can be signed by rachelk, after `openlore identity link github rachelk`"
                ),
                "{text}"
            );
        }
    }
}
