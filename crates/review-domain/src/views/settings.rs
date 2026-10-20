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

/// What happened when forget me revoked her permission but another change
/// of her repos or accounts still held her scope (re-review N8).
pub const FORGET_ME_NOT_PURGED_MESSAGE: &str = "Your permission for OpenLore review is revoked \
     at your Bluesky server, so it can no longer write to your repo. Your suggestions, declines \
     and GitHub links are not deleted yet: another change to your repos or accounts was still \
     being saved. Please press \"Disconnect and forget me\" again to finish.";

/// Forget me revoked her permission but purged nothing yet (her scope was
/// busy): says exactly that, and offers the retry.
pub fn forget_me_not_purged_page() -> String {
    page(
        "Not finished",
        html! {
            h1 { "We could not finish forgetting you" }
            p role="alert" { (FORGET_ME_NOT_PURGED_MESSAGE) }
            p { a href="/settings/forget" { (FORGET_ME_LABEL) } }
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Re-review N8: the busy-scope page says the permission is revoked,
    /// the data is not yet deleted, and offers forget me again.
    #[test]
    fn a_busy_forget_me_says_what_was_revoked_and_what_is_left() {
        let html = forget_me_not_purged_page();
        assert!(html.contains("is revoked"));
        assert!(html.contains("not deleted yet"));
        assert!(html.contains(r#"href="/settings/forget""#));
        assert!(!html.contains("nothing was purged"));
    }
}
