//! Page renderers (maud). Each page is chrome + body, as in ADR-032.
//! Copy constants here are the single source of truth for the app's text.

use maud::{html, Markup, DOCTYPE};

use crate::signin::{PermissionMode, SignInFailure};

/// The name Bluesky shows when the app asks for permission.
pub const APP_NAME: &str = "OpenLore review";

/// What the app is for, in one line.
pub const APP_PURPOSE: &str =
    "Review the philosophy claims OpenLore suggests from your public GitHub repos, \
     and publish only the ones you approve to your own Bluesky repo.";

/// The three never-commitments (AC-001.2), shown before the handle field.
pub const NEVER_COMMITMENTS: [&str; 3] = [
    "OpenLore never publishes a suggestion you have not approved.",
    "OpenLore never shows your pending or declined suggestions to anyone.",
    "OpenLore never posts to Bluesky unless you press Post.",
];

/// The public-data banner (NFR-BRA-8).
pub const PUBLIC_DATA_NOTICE: &str =
    "Only public GitHub data is read: your public profile and your public repos.";

/// Why the broad permission is requested in fallback mode (ADR-073).
pub const BROAD_PERMISSION_NOTICE: &str =
    "Your Bluesky server does not yet support asking for claim and post creation alone, \
     so OpenLore asks for the broader permission. It still only creates claims, and posts \
     only when you press Post.";

/// The sign-in form's button label.
pub const SIGN_IN_LABEL: &str = "Sign in with Bluesky";

/// The sign-out button label.
pub const SIGN_OUT_LABEL: &str = "Sign out";

/// What a person sees when a sign-in did not complete.
pub fn sign_in_failure_message(failure: SignInFailure) -> &'static str {
    match failure {
        SignInFailure::HandleNotFound => {
            "We couldn't find that Bluesky handle. Check the spelling."
        }
        SignInFailure::TemporarilyUnavailable => {
            "Signing in with Bluesky is temporarily unavailable. Nothing changed. \
             Please try again in a few minutes."
        }
        SignInFailure::Cancelled => "No access granted. Nothing changed.",
        SignInFailure::ExchangeFailed => {
            "Signing in failed: your Bluesky server did not complete the sign-in. \
             Nothing changed. Please try again."
        }
        SignInFailure::AccountMismatch => {
            "Your Bluesky server answered for a different account than the handle you \
             entered, so you were not signed in. Nothing changed."
        }
        SignInFailure::ReturnNotRecognised => {
            "That sign-in link has expired or was already used. Nothing changed."
        }
    }
}

/// The shared page chrome. `body` is the page-specific fragment.
fn page(title: &str, body: Markup) -> Markup {
    html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                title { (title) " · " (APP_NAME) }
            }
            body {
                header { strong { (APP_NAME) } }
                main { (body) }
            }
        }
    }
}

/// The public landing page: what the app never does, then the handle field.
/// `failure` explains a sign-in that did not complete.
pub fn landing_page(mode: PermissionMode, failure: Option<SignInFailure>) -> String {
    page(
        "Welcome",
        html! {
            h1 { (APP_NAME) }
            p { (APP_PURPOSE) }
            @if let Some(failure) = failure {
                p role="alert" { (sign_in_failure_message(failure)) }
            }
            ul {
                @for commitment in NEVER_COMMITMENTS {
                    li { (commitment) }
                }
            }
            p { (PUBLIC_DATA_NOTICE) }
            @if mode == PermissionMode::BroadFallback {
                p { (BROAD_PERMISSION_NOTICE) }
            }
            form method="post" action="/signin" {
                label for="handle" { "Your Bluesky handle" }
                input id="handle" name="handle" type="text" autocomplete="username"
                    placeholder="you.bsky.social" required;
                button type="submit" { (SIGN_IN_LABEL) }
            }
        },
    )
    .into_string()
}

/// The signed-in review queue (empty until GitHub is linked).
pub fn review_page(handle: &str, csrf_token: &str) -> String {
    page(
        "Your review queue",
        html! {
            p { "Signed in as @" (handle) }
            form method="post" action="/signout" {
                input type="hidden" name="csrf" value=(csrf_token);
                button type="submit" { (SIGN_OUT_LABEL) }
            }
        },
    )
    .into_string()
}
