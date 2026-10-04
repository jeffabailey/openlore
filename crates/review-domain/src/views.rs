//! Page renderers (maud). Each page is chrome + body, as in ADR-032.
//! Copy constants here are the single source of truth for the app's text.

use maud::{html, Markup, DOCTYPE};

use crate::ownership::OwnershipRefusal;
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

/// Where the signed-in person stands with their GitHub link.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GithubStep<'a> {
    /// No GitHub account linked yet.
    NotLinked,
    /// Linked and verified: a scan may start (after a fresh re-check).
    Verified { login: &'a str },
    /// Linked once, but the last check failed: verify again.
    NeedsReverify { login: &'a str },
}

/// The label of the button that starts a scan.
pub const SCAN_LABEL: &str = "Scan my repos";

/// The label of the button that checks the bio.
pub const VERIFY_LABEL: &str = "Verify";

/// The signed-in review queue: the GitHub step until ownership is proven,
/// then the scan action.
pub fn review_page(handle: &str, csrf_token: &str, github: GithubStep<'_>) -> String {
    page(
        "Your review queue",
        html! {
            p { "Signed in as @" (handle) }
            @match github {
                GithubStep::NotLinked => {
                    p { "Before anything is scanned, verify that your GitHub account is yours." }
                    a href="/github" { "Verify GitHub ownership" }
                }
                GithubStep::NeedsReverify { login } => {
                    p role="alert" {
                        "We could not confirm that github.com/" (login) " is still yours, so nothing \
                         was scanned. Please verify again."
                    }
                    a href="/github" { "Verify GitHub ownership" }
                }
                GithubStep::Verified { login } => {
                    p { "GitHub: github.com/" (login) " (verified)" }
                    form method="post" action="/scan" {
                        input type="hidden" name="csrf" value=(csrf_token);
                        button type="submit" { (SCAN_LABEL) }
                    }
                }
            }
            form method="post" action="/signout" {
                input type="hidden" name="csrf" value=(csrf_token);
                button type="submit" { (SIGN_OUT_LABEL) }
            }
        },
    )
    .into_string()
}

/// What happened when the person pressed Verify.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OwnershipOutcome<'a> {
    Verified,
    Refused(&'a OwnershipRefusal),
}

/// The sentence explaining a verify outcome (AC-002.3: each names the fix).
pub fn ownership_message(
    outcome: OwnershipOutcome<'_>,
    login: &str,
    session_did: &str,
    handle: &str,
) -> String {
    let refusal = match outcome {
        OwnershipOutcome::Verified => {
            return format!("Verified: github.com/{login} belongs to @{handle}")
        }
        OwnershipOutcome::Refused(refusal) => refusal,
    };
    match refusal {
        OwnershipRefusal::DidMissing => format!(
            "We couldn't find your DID in the bio of github.com/{login}. Add your DID to the \
             bio of your own GitHub account, then press Verify again."
        ),
        OwnershipRefusal::NoBio => format!(
            "We couldn't find a public bio on github.com/{login}. Add your DID to the bio of \
             your own GitHub account, then press Verify again."
        ),
        OwnershipRefusal::DifferentDid(found) => format!(
            "The bio of github.com/{login} holds a different DID ({found}). It must match your \
             signed-in account ({session_did}): replace it with your DID, then press Verify again."
        ),
        OwnershipRefusal::IdentityChanged => format!(
            "github.com/{login} now belongs to a different GitHub account than the one you \
             verified. Please verify again."
        ),
        OwnershipRefusal::AccountNotFound => format!(
            "We couldn't find a GitHub account at github.com/{login}. Check the spelling, then \
             press Verify again."
        ),
        OwnershipRefusal::RateLimited => "GitHub is rate-limiting us right now. Please wait a \
             few minutes, then press Verify again. Nothing was lost."
            .to_string(),
        OwnershipRefusal::GithubUnavailable => "GitHub could not be reached. Please wait a few \
             minutes, then press Verify again. Nothing was lost."
            .to_string(),
        OwnershipRefusal::LinkedToAnotherAccount => format!(
            "github.com/{login} is already verified for another Bluesky account. Each GitHub \
             account can belong to one Bluesky account only."
        ),
        OwnershipRefusal::TooManyAttempts => "Too many verification attempts in the last hour. \
             Please try again later. Nothing was lost."
            .to_string(),
    }
}

/// The GitHub step: the exact DID to copy, where to put it, and the form
/// that checks it. `message` explains the last attempt.
pub fn github_page(session_did: &str, csrf_token: &str, message: Option<&str>) -> String {
    page(
        "Prove your GitHub account",
        html! {
            h1 { "Prove your GitHub account is yours" }
            @if let Some(message) = message {
                p role="status" { (message) }
            }
            p { "Your DID:" }
            p { code id="your-did" { (session_did) } " " a href="#your-did" data-copy=(session_did) { "Copy" } }
            p {
                "Add this exact DID anywhere in the bio of your GitHub profile \
                 (github.com/settings/profile). It shows that you, the person signed in \
                 here, control that GitHub account. Then enter your GitHub username and \
                 press Verify."
            }
            form method="post" action="/github" {
                input type="hidden" name="csrf" value=(csrf_token);
                label for="github_login" { "GitHub username" }
                input id="github_login" name="github_login" type="text" required;
                button type="submit" { (VERIFY_LABEL) }
            }
            p { a href="/review" { "Back to your review queue" } }
            script src="/assets/copy.js" {}
        },
    )
    .into_string()
}

/// The scan-status fragment the queue polls (`data-scan-status`).
pub fn scan_status_page(status: &str) -> String {
    page(
        "Scan status",
        html! {
            div data-scan-status=(status) { "Scan status: " (status) }
            p { a href="/review" { "Back to your review queue" } }
        },
    )
    .into_string()
}

/// The page script: copy a `data-copy` value to the clipboard.
pub const COPY_SCRIPT: &str = "document.querySelectorAll('[data-copy]').forEach(function (el) {\n\
  el.addEventListener('click', function (event) {\n\
    event.preventDefault();\n\
    navigator.clipboard.writeText(el.getAttribute('data-copy'));\n\
  });\n\
});\n";
