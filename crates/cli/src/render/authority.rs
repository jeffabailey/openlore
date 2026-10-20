//! The signing refusal and its hand-off (owner-only-signing, ADR-086 §10):
//! why nothing was signed and who can sign instead, rendered from the pure
//! [`AuthorityRefusal`] data. Wording pins: `tests/acceptance/support/authority.rs`.

use claim_domain::authority::{AuthorityRefusal, Unattested, WhoCanSign};

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
fn hand_off(can_sign: &WhoCanSign) -> String {
    match can_sign {
        WhoCanSign::OwnerAfterLinking { login } => {
            format!("{CAN_BE_SIGNED_BY} {login}, after `openlore identity link github {login}`.")
        }
    }
}

/// Render a refused signature: nothing was signed or published, why, and the
/// hand-off.
pub fn render_signing_refused(refusal: &AuthorityRefusal) -> String {
    format!(
        "Not signed: {}. Nothing was signed or published.\n{}\n",
        refusal_reason(&refusal.subject, refusal.reason),
        hand_off(&refusal.can_sign)
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
            let text = render_signing_refused(&AuthorityRefusal {
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
