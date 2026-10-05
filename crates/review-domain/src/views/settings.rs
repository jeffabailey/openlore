//! Settings and "Disconnect and forget me" (US-BRA-012).

use maud::html;

use super::*;

/// The settings link and the forget-me entry point (US-BRA-012).
pub const FORGET_ME_LABEL: &str = "Disconnect and forget me";

/// The confirm button of forget me.
pub const CONFIRM_FORGET_ME_LABEL: &str = "Yes, forget me";

/// What forget me deletes and what it keeps (AC-012.1).
pub const FORGET_ME_NOTICE: &str = "This deletes your pending suggestions, declines and GitHub \
     link from OpenLore review. Claims you published stay in your own repo.";

/// Settings: who is signed in, and the way out.
pub fn settings_page(handle: &str) -> String {
    page(
        "Settings",
        html! {
            h1 { "Settings" }
            p { "Signed in as @" (handle) }
            p { a href="/settings/forget" { (FORGET_ME_LABEL) } }
            (back_to_queue())
        },
    )
}

/// The forget-me confirmation (AC-012.1, AC-012.4): what is deleted, what is
/// kept, and the one button that does it. Nothing is deleted here.
pub fn forget_me_page(csrf_token: &str) -> String {
    page(
        "Disconnect and forget me?",
        html! {
            h1 { "Disconnect and forget me?" }
            p { strong { (FORGET_ME_NOTICE) } }
            p {
                "Your permission for OpenLore review is revoked at your Bluesky server, and \
                 OpenLore review keeps no access to your account."
            }
            form method="post" action="/settings/forget" {
                (csrf_field(csrf_token))
                button type="submit" { (CONFIRM_FORGET_ME_LABEL) }
            }
            p { a href="/review" { (CANCEL_LABEL) } }
        },
    )
}

/// Forget me could not finish (the store failed): nothing was removed
/// from her PDS, and she can try again.
pub fn forget_me_failed_page() -> String {
    page(
        "Not finished",
        html! {
            h1 { "We could not finish forgetting you" }
            p { "Nothing in your own repo was touched. Please try again in a moment." }
            p { a href="/settings/forget" { (FORGET_ME_LABEL) } }
        },
    )
}
