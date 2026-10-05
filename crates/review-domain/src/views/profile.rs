//! The public profile (US-BRA-007): published claims only, read live.

use maud::{html, Markup};

use super::*;
use crate::published::PublishedClaim;

/// The provenance label every profile claim carries (I-BRA-5).
pub const SELF_ATTESTED_LABEL: &str = "self-attested";

/// The owner's way from an empty profile to their queue.
pub const REVIEW_SUGGESTIONS_LABEL: &str = "Review suggestions →";

/// An unreachable PDS, stated plainly; nothing stale is shown instead.
pub const PDS_UNREACHABLE_NOTICE: &str =
    "We can't reach this person's PDS right now, so no claims are shown. Try again shortly.";

/// The identity directory could not be reached.
pub const DIRECTORY_UNREACHABLE_NOTICE: &str =
    "We can't look this person up right now. Try again shortly.";

/// What a profile can show.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProfileContent {
    Claims(Vec<PublishedClaim>),
    PdsUnreachable,
    DirectoryUnreachable,
}

/// A profile page: whose it is, whether its owner is looking, and what it shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProfileView<'a> {
    pub handle: &'a str,
    pub viewer_is_owner: bool,
    pub content: &'a ProfileContent,
}

fn profile_card(claim: &PublishedClaim, viewer_is_owner: bool) -> Markup {
    html! {
        article {
            h2 { (claim_headline(&claim.subject, &claim.object)) }
            p {
                (SELF_ATTESTED_LABEL) " · Confidence " (confidence_with_bucket(claim.confidence_bp))
            }
            @if viewer_is_owner {
                p { a href=(format!("/retract?claim={}", claim.rkey)) { (RETRACT_LABEL) } }
            }
        }
    }
}

/// The public profile (US-BRA-007): only what the PDS holds as published.
pub fn profile_page(view: &ProfileView<'_>) -> String {
    page(
        &format!("@{}", view.handle),
        html! {
            h1 { "@" (view.handle) }
            @match view.content {
                ProfileContent::PdsUnreachable => p role="alert" { (PDS_UNREACHABLE_NOTICE) },
                ProfileContent::DirectoryUnreachable => p role="alert" { (DIRECTORY_UNREACHABLE_NOTICE) },
                ProfileContent::Claims(claims) if claims.is_empty() => {
                    p { (view.handle) " hasn't published any claims yet." }
                    @if view.viewer_is_owner {
                        p { a href="/review" { (REVIEW_SUGGESTIONS_LABEL) } }
                    }
                },
                ProfileContent::Claims(claims) => {
                    @for claim in claims { (profile_card(claim, view.viewer_is_owner)) }
                    @if view.viewer_is_owner {
                        p { a href="/share" { (SHARE_LABEL) } }
                    }
                },
            }
        },
    )
}

/// A profile URL that names nobody (PR-4): plainly not found.
pub fn profile_not_found_page() -> String {
    page(
        "Profile not found",
        html! {
            h1 { "Profile not found" }
            p { "No Bluesky account answers to that handle or DID." }
            p { a href="/" { (APP_NAME) } }
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        /// Universe: profile contents and viewers. Every shown claim is
        /// labelled self-attested, never unverified; the owner's call to
        /// action appears only to the owner of an empty profile.
        #[test]
        fn every_shown_claim_is_self_attested_and_only_an_empty_owner_profile_offers_review(
            confidences in proptest::collection::vec(0u16..=10_000, 0..4),
            viewer_is_owner in any::<bool>(),
        ) {
            let claims: Vec<PublishedClaim> = confidences
                .iter()
                .enumerate()
                .map(|(i, bp)| PublishedClaim {
                    subject: format!("github:priyaraman/repo{i}"),
                    object: "org.openlore.philosophy.test-driven".to_string(),
                    confidence_bp: *bp,
                    rkey: format!("bafy{i}"),
                })
                .collect();
            let content = ProfileContent::Claims(claims.clone());
            let html = profile_page(&ProfileView { handle: "priyaraman.bsky.social", viewer_is_owner, content: &content });
            prop_assert_eq!(html.matches("<article>").count(), claims.len());
            prop_assert_eq!(html.matches(SELF_ATTESTED_LABEL).count(), claims.len());
            prop_assert!(!html.contains("unverified"));
            prop_assert_eq!(html.contains(REVIEW_SUGGESTIONS_LABEL), viewer_is_owner && claims.is_empty());
        }
    }

    /// An unreachable PDS is stated and shows no claims.
    // bypass: a closed two-variant outcome; one example per arm is the universe.
    #[test]
    fn an_unreachable_pds_or_directory_is_stated_with_no_claims() {
        for (content, notice) in [
            (ProfileContent::PdsUnreachable, PDS_UNREACHABLE_NOTICE),
            (
                ProfileContent::DirectoryUnreachable,
                DIRECTORY_UNREACHABLE_NOTICE,
            ),
        ] {
            let html = profile_page(&ProfileView {
                handle: "priyaraman.bsky.social",
                viewer_is_owner: false,
                content: &content,
            });
            assert!(html.contains(notice));
            assert!(!html.contains("<article>"));
        }
    }
}
