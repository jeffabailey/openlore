//! The signing refusal and its hand-off (owner-only-signing, ADR-086 §5/§10):
//! why nothing was signed and who can sign instead — or the re-check that
//! lets the signer sign — rendered from the pure [`AuthorityRefusal`] data.
//! Wording pins: `tests/acceptance/support/authority.rs`.

use chrono::{DateTime, Utc};
use claim_domain::authority::{AuthorityRefusal, Unattested, WhoCanSign, SIGNING_WINDOW_DAYS};

/// The fixed hand-off lead.
pub const CAN_BE_SIGNED_BY: &str = "Can be signed by";

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
    }
}

/// Who can sign instead, and the command that gets them there.
pub(crate) fn hand_off(can_sign: &WhoCanSign) -> String {
    match can_sign {
        WhoCanSign::OwnerAfterLinking { login } => {
            format!("{CAN_BE_SIGNED_BY} {login}, after `openlore identity link github {login}`.")
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
