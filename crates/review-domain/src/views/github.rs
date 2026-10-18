//! The GitHub ownership step (US-BRA-002).

use maud::html;

use super::*;
use crate::link_plan::{LinkPlan, LinkRefusal};
use crate::ownership::OwnershipRefusal;
use ports::claim_domain::identity_link::LinkRecord;

/// The label of the button that checks the bio.
pub const VERIFY_LABEL: &str = "Verify";

/// The verified owner's offer to publish the link (Q-GDL-D1).
pub const PUBLISH_LINK_LABEL: &str = "Publish this link";

/// The owner's offer to retract one of her Active published links (RA-4).
pub const RETRACT_LINK_LABEL: &str = "Retract this link";

/// The form field naming the GitHub account to check or link (U-26).
pub const GITHUB_LOGIN_FIELD: &str = "github_login";

/// Where the offer asks for the link preview.
pub const LINK_PREVIEW_ACTION: &str = "/github/link";

/// Where the link preview's plan is confirmed (03-05).
pub const LINK_CONFIRM_ACTION: &str = "/github/link/publish";

/// What publishing a link means (RA-1a, ADR-084 §5).
pub const LINK_PUBLIC_NOTICE: &str = "This link is public: anyone can read it in your repo. \
     Retracting this link later withdraws it but does not erase it.";

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

/// How a verify outcome is announced: a success is a status, a refusal an
/// alert, so a refusal never reads as if it worked.
fn outcome_role(outcome: OwnershipOutcome<'_>) -> &'static str {
    match outcome {
        OwnershipOutcome::Verified => "status",
        OwnershipOutcome::Refused(_) => "alert",
    }
}

/// The GitHub step: the exact DID to copy, where to put it, and the form
/// that checks it. `notice` is the last attempt's outcome and its message.
pub fn github_page(
    session_did: &str,
    csrf_token: &str,
    notice: Option<(OwnershipOutcome<'_>, &str)>,
) -> String {
    github_step_page(session_did, csrf_token, notice, None, &[])
}

/// The GitHub step, offering "Publish this link" for `verified_login` — the
/// account the app has verified for this owner, if any (RA-9) — and
/// "Retract this link" for each of `active_links`, the owner's Active link
/// records read live from her repo (RA-4).
pub fn github_step_page(
    session_did: &str,
    csrf_token: &str,
    notice: Option<(OwnershipOutcome<'_>, &str)>,
    verified_login: Option<&str>,
    active_links: &[LinkRecord],
) -> String {
    page(
        "Prove your GitHub account",
        html! {
            h1 { "Prove your GitHub account is yours" }
            @if let Some((outcome, message)) = notice {
                p role=(outcome_role(outcome)) { (message) }
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
                (csrf_field(csrf_token))
                label for="github_login" { "GitHub username" }
                input id="github_login" name=(GITHUB_LOGIN_FIELD) type="text" required;
                button type="submit" { (VERIFY_LABEL) }
            }
            @if let Some(login) = verified_login {
                (link_offer(login, csrf_token))
            }
            @for record in active_links {
                (retract_link_offer(record))
            }
            (back_to_queue())
            script src="/assets/copy.js" {}
        },
    )
}

/// The offer to preview the link to the verified account `login`.
fn link_offer(login: &str, csrf_token: &str) -> Markup {
    html! {
        form method="post" action=(LINK_PREVIEW_ACTION) {
            (csrf_field(csrf_token))
            input type="hidden" name=(GITHUB_LOGIN_FIELD) value=(login);
            button type="submit" { (PUBLISH_LINK_LABEL) }
        }
    }
}

/// The offer to open the retract preview of one published link record.
fn retract_link_offer(record: &LinkRecord) -> Markup {
    html! {
        p {
            "Published link to github.com/" (record.link.login) ": "
            a href=(format!("/retract?claim={}", record.record_id)) { (RETRACT_LINK_LABEL) }
        }
    }
}

/// The link preview: the exact record a confirm adds to the owner's repo,
/// that it is public, and what retracting it does. Nothing is written here.
pub fn link_preview_page(plan: &LinkPlan, csrf_token: &str) -> String {
    let claim = plan.claim();
    page(
        "Publish this link?",
        html! {
            h1 { "Publish this link?" }
            p { "This record will be added to your repo:" }
            p { code { (claim.subject) " " (claim.predicate) " " (claim.object) } }
            p { "Evidence:" }
            ul {
                @for evidence in &claim.evidence {
                    li { code { (evidence) } }
                }
            }
            p { strong { (LINK_PUBLIC_NOTICE) } }
            p { "It will be added at " code { (plan.at_uri()) } "." }
            p { "Nothing has been published yet." }
            (plan_form(LINK_CONFIRM_ACTION, plan.rkey(), csrf_token, PUBLISH_LABEL))
            p { a href="/github" { (CANCEL_LABEL) } }
        },
    )
}

/// Every link confirm that writes nothing ends with this sentence (U-31).
pub const NOTHING_PUBLISHED: &str = "Nothing was published.";

/// Why a confirmed link is not in the owner's repo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkNotPublished {
    /// The preview proved the link but could not be kept for the confirm
    /// (U-37): nothing was published, and GitHub is not to blame.
    PreviewNotSaved,
    /// The preview's deadline passed before the confirm.
    Expired,
    /// The budget was spent, or GitHub could not be read at confirm (F-10).
    Refused(LinkRefusal),
    /// The confirm-time GitHub read did not prove the previewed link.
    NotProven,
    /// The confirm-time GitHub read disproves the link: the bio no longer
    /// names the DID (RA-2). A new preview has to prove it again.
    ProofGone,
    /// The repo host session ended: only a fresh sign-in helps.
    SessionExpired,
    /// The repo host did not take the record.
    NotWritten,
    /// The record was created but did not read back as previewed.
    NotAsPreviewed,
}

#[cfg(test)]
impl LinkNotPublished {
    /// One value of every variant, in a fixed order — the universe of the
    /// properties over every way a link is not published (F-14).
    pub(crate) fn one_of_each() -> Vec<Self> {
        std::iter::successors(Some(Self::PreviewNotSaved), Self::next_variant).collect()
    }

    /// The variant after this one. Exhaustive, no wildcard: a new variant
    /// does not compile until it is placed in the chain.
    fn next_variant(&self) -> Option<Self> {
        match self {
            Self::PreviewNotSaved => Some(Self::Expired),
            Self::Expired => Some(Self::Refused(crate::link_plan::BUDGET_SPENT)),
            Self::Refused(_) => Some(Self::NotProven),
            Self::NotProven => Some(Self::ProofGone),
            Self::ProofGone => Some(Self::SessionExpired),
            Self::SessionExpired => Some(Self::NotWritten),
            Self::NotWritten => Some(Self::NotAsPreviewed),
            Self::NotAsPreviewed => None,
        }
    }
}

/// The sentence explaining why the link to github.com/`login` is not
/// published.
fn link_not_published_reason(why: &LinkNotPublished, login: &str) -> String {
    let reason = match why {
        LinkNotPublished::Refused(refusal) => return link_refusal_reason(refusal, login),
        LinkNotPublished::PreviewNotSaved => {
            "We could not save this preview. Press \"Publish this link\" \
             to try again."
        }
        LinkNotPublished::Expired => {
            "This preview has expired. Press \"Publish this link\" again \
             for a fresh preview."
        }
        LinkNotPublished::NotProven => {
            "Your GitHub profile did not prove this link when you \
             confirmed it."
        }
        LinkNotPublished::ProofGone => {
            "The proof is gone: your GitHub bio no longer names your DID. \
             Put it back, then press \"Publish this link\" for a fresh preview."
        }
        LinkNotPublished::SessionExpired => {
            "Your Bluesky session has expired. Sign in again, \
             then publish your link from the GitHub step."
        }
        LinkNotPublished::NotWritten => "Your PDS did not take the record.",
        LinkNotPublished::NotAsPreviewed => {
            "Your PDS did not return the link exactly as \
             previewed, so it is not counted as published."
        }
    };
    reason.to_string()
}

/// The link confirm's own wording for a refusal (U-31): the verify step's
/// sentences stay as they are, these name "Publish this link" as the fix.
/// The app's token is never part of any of them.
fn link_refusal_reason(refusal: &LinkRefusal, login: &str) -> String {
    let ownership = match refusal {
        LinkRefusal::TokenRejected => {
            return "GitHub did not accept the app's access, so your link could not be \
                    checked. Please try again later."
                .to_string()
        }
        LinkRefusal::Ownership(ownership) => ownership,
    };
    match ownership {
        OwnershipRefusal::DidMissing => format!(
            "We couldn't find your DID in the bio of github.com/{login}. Add it back, then \
             press \"Publish this link\" for a fresh preview."
        ),
        OwnershipRefusal::NoBio => format!(
            "We couldn't find a public bio on github.com/{login}. Add your DID to it, then \
             press \"Publish this link\" for a fresh preview."
        ),
        OwnershipRefusal::DifferentDid(found) => format!(
            "The bio of github.com/{login} holds a different DID ({found}). Replace it with \
             your DID, then press \"Publish this link\" for a fresh preview."
        ),
        OwnershipRefusal::IdentityChanged => format!(
            "github.com/{login} now belongs to a different GitHub account than the one you \
             previewed. Please verify again."
        ),
        OwnershipRefusal::AccountNotFound => format!(
            "We couldn't find a GitHub account at github.com/{login}. Check that it still \
             exists, then verify it again."
        ),
        OwnershipRefusal::RateLimited => "GitHub is rate-limiting us right now. Please wait a \
             few minutes, then press \"Publish this link\" again."
            .to_string(),
        OwnershipRefusal::GithubUnavailable => "GitHub could not be reached. Please wait a few \
             minutes, then press \"Publish this link\" again."
            .to_string(),
        OwnershipRefusal::LinkedToAnotherAccount => format!(
            "github.com/{login} is already verified for another Bluesky account, so it cannot \
             be linked to this one."
        ),
        OwnershipRefusal::TooManyAttempts => "Too many verification attempts in the last hour. \
             Please try again later."
            .to_string(),
    }
}

/// The confirmed link is in the owner's repo at `at_uri`.
pub fn link_published_page(at_uri: &str) -> String {
    page(
        "Link published",
        html! {
            section class="outcome" {
                h1 { "Your link is published" }
                p { "Your link is in your own repo at " code { (at_uri) } }
                p { a href="/github" { "Back to the GitHub step" } }
            }
        },
    )
}

/// A link confirm to github.com/`login` that did not publish, and the
/// owner's next step: Retry re-submits the kept plan to the link confirm
/// (never the claim publish).
pub fn link_not_published_page(
    why: &LinkNotPublished,
    login: &str,
    retry: Option<PublishRetry<'_>>,
) -> String {
    page(
        "Link not published",
        html! {
            section class="outcome" {
                h1 { "Your link was not published" }
                p role="alert" { (link_not_published_reason(why, login)) }
                @if *why != LinkNotPublished::NotAsPreviewed {
                    p { (NOTHING_PUBLISHED) }
                }
                @match (why, retry) {
                    (LinkNotPublished::SessionExpired, _) => {
                        p { a href="/" { (SIGN_IN_LABEL) } }
                    }
                    (_, Some(retry)) => {
                        (plan_form(LINK_CONFIRM_ACTION, retry.plan_id, retry.csrf_token, RETRY_LABEL))
                        p { a href="/github" { "Back to the GitHub step" } }
                    }
                    (_, None) => {
                        p { a href="/github" { "Back to the GitHub step" } }
                    }
                }
            }
        },
    )
}

#[cfg(test)]
mod tests {
    //! Universe: every way a link confirm ends without publishing × a kept
    //! plan or none. The page offers Retry exactly when the plan was kept and
    //! the session still holds; the Retry posts the plan to the link confirm.
    use super::*;
    use proptest::prelude::*;

    const PLAN_ID: &str = "bafylinkplan";
    const CSRF: &str = "csrf-token-7";
    const LOGIN: &str = "priyaraman";

    fn arb_why() -> impl Strategy<Value = LinkNotPublished> {
        prop::sample::select(vec![
            LinkNotPublished::PreviewNotSaved,
            LinkNotPublished::Expired,
            LinkNotPublished::Refused(crate::link_plan::BUDGET_SPENT),
            LinkNotPublished::Refused(LinkRefusal::TokenRejected),
            LinkNotPublished::NotProven,
            LinkNotPublished::ProofGone,
            LinkNotPublished::SessionExpired,
            LinkNotPublished::NotWritten,
            LinkNotPublished::NotAsPreviewed,
        ])
    }

    proptest! {
        #[test]
        fn a_kept_link_plan_is_retried_through_the_link_confirm_only(
            why in arb_why(), kept in any::<bool>(),
        ) {
            let retry = kept.then_some(PublishRetry { plan_id: PLAN_ID, csrf_token: CSRF });

            let html = link_not_published_page(&why, LOGIN, retry);

            let retry_form = format!(r#"<form method="post" action="{LINK_CONFIRM_ACTION}">"#);
            let offers_retry = kept && why != LinkNotPublished::SessionExpired;
            prop_assert_eq!(html.contains(&retry_form), offers_retry, "{}", html);
            prop_assert_eq!(html.contains(&format!(">{RETRY_LABEL}</button>")), offers_retry, "{}", html);
            prop_assert_eq!(html.contains(&format!(r#"name="plan" value="{PLAN_ID}""#)), offers_retry, "{}", html);
            prop_assert!(!html.contains("/review/publish"), "{}", html);
            prop_assert_eq!(
                html.contains(SIGN_IN_LABEL),
                why == LinkNotPublished::SessionExpired,
                "{}", html
            );
        }
    }

    /// U-37: a preview that could not be kept says so, says nothing was
    /// published, and never blames GitHub.
    #[test]
    fn a_preview_that_could_not_be_saved_says_so_and_never_blames_github() {
        let html = link_not_published_page(&LinkNotPublished::PreviewNotSaved, LOGIN, None);

        assert!(html.contains("We could not save this preview."), "{html}");
        assert!(html.contains(NOTHING_PUBLISHED), "{html}");
        assert!(html.contains(PUBLISH_LINK_LABEL), "{html}");
        assert!(!html.contains("GitHub could not be reached"), "{html}");
        assert!(
            !html.contains(&format!(">{RETRY_LABEL}</button>")),
            "{html}"
        );
    }
}
