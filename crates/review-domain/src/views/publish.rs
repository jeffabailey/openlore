//! The exact-record preview and the publish outcome pages (US-BRA-004).

use maud::html;

use super::*;
use crate::plans::PublishPlan;

/// Where a publish plan is confirmed (or retried).
const PUBLISH_ACTION: &str = "/review/publish";

/// The confirm button of the exact-record preview.
pub const PUBLISH_LABEL: &str = "Publish to my repo";

/// What the preview says a published claim is (and is not).
pub const NOT_AS_TRUTH_NOTICE: &str =
    "Publishing records this as your own claim, signed by your repo: your view, not as truth.";

/// The exact-record preview opened by Approve (AC-004.4): the destination and
/// every field to be written, confidence as shown and as stored, provenance
/// "self-attested". Nothing is written here; only the confirm form writes.
pub fn approval_preview_page(plan: &PublishPlan, csrf_token: &str) -> String {
    let claim = plan.claim();
    let basis_points = u16::try_from(claim.confidence.basis_points()).unwrap_or(u16::MAX);
    page(
        "Preview before publishing",
        html! {
            h1 { "Preview before publishing" }
            p { (suggestion_headline(plan.key())) }
            p { "Destination: your repo " (plan.owner_did()) " on your own PDS, at " (plan.at_uri()) }
            dl {
                dt { "subject" } dd { (claim.subject) }
                dt { "predicate" } dd { (claim.predicate) }
                dt { "object" } dd { (claim.object) }
                dt { "evidence" }
                dd {
                    @for url in &claim.evidence {
                        a href=(url) { (url) } " "
                    }
                }
                dt { "confidence" }
                dd {
                    (confidence_with_bucket(basis_points)) " — stored as " (basis_points)
                }
                dt { "author" } dd { (claim.author_did.0) }
                dt { "composedAt" } dd { (claim.composed_at) }
                dt { "provenance" } dd { "self-attested (no app signature; your repo attests it)" }
            }
            p { strong { (NOT_AS_TRUTH_NOTICE) } }
            p { "Nothing has been published yet." }
            (plan_form(PUBLISH_ACTION, plan.rkey(), csrf_token, PUBLISH_LABEL))
            p { a href="/review" { "Back" } }
        },
    )
}

/// The confirmation after a publish (AC-004.5): the record's address and
/// the way back.
pub fn published_page(at_uri: &str) -> String {
    page(
        "Published",
        html! {
            h1 { "Published to your repo" }
            p { "Your claim is in your own repo at " code { (at_uri) } }
            p {
                "To retract it, open your profile and press Retract: a retraction is a \
                 new record in your repo that points at this one."
            }
            (back_to_queue())
        },
    )
}

/// The button that tries a failed publish again.
pub const RETRY_LABEL: &str = "Retry";

/// What a Retry re-submits: the same plan, under the page's anti-forgery
/// token.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PublishRetry<'a> {
    pub plan_id: &'a str,
    pub csrf_token: &'a str,
}

/// What the owner can do after a publish did not land.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PublishNextStep<'a> {
    /// The plan is kept: Retry publishes it (exactly once).
    Retry(PublishRetry<'a>),
    /// The PDS session ended: sign in again, then publish from the queue.
    SignInAgain,
    /// The plan could not be kept: preview again from the queue.
    BackToQueue,
}

impl<'a> PublishNextStep<'a> {
    /// The next step after a failed publish: an ended session needs a fresh
    /// sign-in whatever else holds; otherwise Retry when the plan was kept
    /// (`retry`), else back to the queue.
    pub fn after_failure(needs_sign_in: bool, retry: Option<PublishRetry<'a>>) -> Self {
        match (needs_sign_in, retry) {
            (true, _) => Self::SignInAgain,
            (false, Some(retry)) => Self::Retry(retry),
            (false, None) => Self::BackToQueue,
        }
    }
}

/// A publish that did not land (AC-004.6): nothing was written, the
/// suggestion stays pending, and the owner can try again.
pub fn publish_failed_page(reason: &str, next_step: PublishNextStep<'_>) -> String {
    page(
        "Nothing was published",
        html! {
            h1 { "Nothing was published" }
            p role="alert" { (reason) }
            p { "Your suggestion is still pending." }
            @match next_step {
                PublishNextStep::Retry(retry) => {
                    (plan_form(PUBLISH_ACTION, retry.plan_id, retry.csrf_token, RETRY_LABEL))
                }
                PublishNextStep::SignInAgain => {
                    p { a href="/" { (SIGN_IN_LABEL) } }
                }
                PublishNextStep::BackToQueue => {}
            }
            (back_to_queue())
        },
    )
}

/// A confirm that arrived after its preview expired (ADR-074): refused,
/// nothing written.
pub fn plan_expired_page() -> String {
    page(
        "This preview has expired",
        html! {
            h1 { "This preview has expired" }
            p role="alert" {
                "Previews can be published for 30 minutes. Nothing was published; \
                 your suggestion is still pending. Press Approve again to see a fresh preview."
            }
            (back_to_queue())
        },
    )
}
