//! The opt-in share post (US-BRA-008): previewed, editable, posted on
//! confirm.

use maud::html;

use super::*;

/// The owner's way from their profile to the share preview.
pub const SHARE_LABEL: &str = "Share on Bluesky…";

/// The only button that posts.
pub const POST_LABEL: &str = "Post to Bluesky";

/// Leaving the preview without posting.
pub const DONT_POST_LABEL: &str = "Don't post";

/// The share preview: the editable text, the profile link it carries, and
/// the one button that posts. `notice` explains why an earlier press did
/// not post (too long, refused by the PDS); the owner's text is kept.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SharePreview<'a> {
    pub plan_id: &'a str,
    pub csrf_token: &'a str,
    pub text: &'a str,
    pub profile_url: &'a str,
    pub profile_path: &'a str,
    pub notice: Option<&'a str>,
}

pub fn share_preview_page(view: &SharePreview<'_>) -> String {
    page(
        "Share on Bluesky",
        html! {
            h1 { "Share on Bluesky" }
            @if let Some(notice) = view.notice {
                p role="alert" { (notice) }
            }
            p {
                "Nothing has been posted yet. Edit the text if you like; it is posted to \
                 Bluesky only when you press " (POST_LABEL) "."
            }
            p { "Your post links to your profile: " a href=(view.profile_url) { (view.profile_url) } }
            form method="post" action="/share" {
                (csrf_field(view.csrf_token))
                input type="hidden" name="plan" value=(view.plan_id);
                label for="share-text" { "Post text" }
                textarea id="share-text" name="text" rows="6" cols="60" { (view.text) }
                button type="submit" { (POST_LABEL) }
            }
            p { a href=(view.profile_path) { (DONT_POST_LABEL) } }
        },
    )
}

/// The post landed in the owner's repo.
pub fn share_posted_page(at_uri: &str, profile_path: &str) -> String {
    page(
        "Posted",
        html! {
            h1 { "Posted." }
            p { "Your post is in your repo at " code { (at_uri) } }
            (back_to_profile(profile_path))
        },
    )
}

/// Why a confirmed post did not land, and how to try again.
pub fn share_failed_notice(reason: &str) -> String {
    format!(
        "Your post wasn't published: {reason} Your profile and claims are unchanged. \
         Press {POST_LABEL} to try again."
    )
}

/// Nothing to share (no live published claim), or the preview expired.
pub fn share_unavailable_page(reason: &str) -> String {
    page(
        "Nothing to share",
        html! {
            h1 { "Nothing to share" }
            p role="alert" { (reason) }
            p { a href="/review" { (REVIEW_SUGGESTIONS_LABEL) } }
        },
    )
}
