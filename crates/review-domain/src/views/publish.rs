//! The exact-record preview and the publish outcome pages (US-BRA-004).

use maud::html;

use super::*;
use crate::failed_write::FailedWrite;
use crate::plans::PublishPlan;

/// Where a publish plan is confirmed (or retried).
const PUBLISH_ACTION: &str = "/review/publish";

/// The confirm button of the exact-record preview.
pub const PUBLISH_LABEL: &str = "Publish to my repo";

/// What the preview says a published claim is (and is not).
pub const NOT_AS_TRUTH_NOTICE: &str =
    "Publishing records this as your own claim, signed by your repo: your view, not as truth.";

/// Where a whole batch of previewed plans is confirmed.
const PUBLISH_SELECTED_ACTION: &str = "/review/publish-selected";

/// The exact record a plan writes: the destination and every field,
/// confidence as shown and as stored, provenance "self-attested".
fn record_details(plan: &PublishPlan) -> Markup {
    let claim = plan.claim();
    let basis_points = u16::try_from(claim.confidence.basis_points()).unwrap_or(u16::MAX);
    html! {
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
    }
}

/// The exact-record preview opened by Approve (AC-004.4): the destination and
/// every field to be written, confidence as shown and as stored, provenance
/// "self-attested". Nothing is written here; only the confirm form writes.
/// It is one outcome region: an inline Approve swaps it into the card, and
/// its Publish swaps the outcome in place.
pub fn approval_preview_page(plan: &PublishPlan, csrf_token: &str) -> String {
    page(
        "Preview before publishing",
        html! {
            section class="outcome" {
                h1 { "Preview before publishing" }
                p { (suggestion_headline(plan.key())) }
                (record_details(plan))
                p { strong { (NOT_AS_TRUTH_NOTICE) } }
                p { "Nothing has been published yet." }
                form method="post" action=(PUBLISH_ACTION) hx-post=(PUBLISH_ACTION)
                    hx-target="closest .outcome" hx-select=(OUTCOME_REGION) hx-swap="outerHTML" {
                    (csrf_field(csrf_token))
                    input type="hidden" name="plan" value=(plan.rkey());
                    button type="submit" { (PUBLISH_LABEL) }
                }
                p { a href="/review" { "Back" } }
            }
        },
    )
}

/// "Publish N to my repo", the batch preview's confirm button.
pub fn publish_selected_label(count: usize) -> String {
    format!("Publish {count} to my repo")
}

/// The batch preview opened by "Approve selected": the exact record of every
/// chosen suggestion, then one confirm naming each kept plan. Nothing is
/// written here.
pub fn batch_preview_page(plans: &[PublishPlan], csrf_token: &str) -> String {
    let count = plans.len();
    page(
        "Preview before publishing",
        html! {
            section class="outcome" {
                h1 { "Preview " (count) " claims before publishing" }
                @for plan in plans {
                    h2 { (suggestion_headline(plan.key())) }
                    (record_details(plan))
                }
                p { strong { (NOT_AS_TRUTH_NOTICE) } }
                p { "Nothing has been published yet." }
                form method="post" action=(PUBLISH_SELECTED_ACTION) {
                    (csrf_field(csrf_token))
                    @for plan in plans {
                        input type="hidden" name="plan" value=(plan.rkey());
                    }
                    button type="submit" { (publish_selected_label(count)) }
                }
                p { a href="/review" { "Back" } }
            }
        },
    )
}

/// What a confirmed batch did: the records that landed, the ones that did
/// not (still pending, kept for Retry when possible), and the previews that
/// had expired.
pub struct BatchOutcome<'a> {
    pub published: &'a [String],
    pub failures: &'a [String],
    /// The plans put back after a failed write: Retry publishes them.
    pub retry_plan_ids: &'a [String],
    pub expired: usize,
    pub needs_sign_in: bool,
    pub csrf_token: &'a str,
}

/// The result of "Publish N to my repo" (AC-004.5/.6 for each record).
pub fn batch_published_page(outcome: &BatchOutcome<'_>) -> String {
    let landed = outcome.published.len();
    page(
        "Published",
        html! {
            section class="outcome" {
                h1 { "Published " (landed) " to your repo" }
                @if landed > 0 {
                    ul {
                        @for at_uri in outcome.published {
                            li { code { (at_uri) } }
                        }
                    }
                    p {
                        "To retract one, open your profile and press Retract: a retraction is \
                         a new record in your repo that points at it."
                    }
                }
                @for reason in outcome.failures {
                    p role="alert" { "Not published: " (reason) " That suggestion is still pending." }
                }
                @if outcome.expired > 0 {
                    p role="alert" {
                        (outcome.expired) " of these previews had expired, so they were not \
                         published. Select them again to see a fresh preview."
                    }
                }
                @if outcome.needs_sign_in {
                    p { a href="/" { (SIGN_IN_LABEL) } }
                } @else if !outcome.retry_plan_ids.is_empty() {
                    form method="post" action=(PUBLISH_SELECTED_ACTION) {
                        (csrf_field(outcome.csrf_token))
                        @for plan_id in outcome.retry_plan_ids {
                            input type="hidden" name="plan" value=(plan_id);
                        }
                        button type="submit" { (RETRY_LABEL) }
                    }
                }
                (back_to_queue())
            }
        },
    )
}

/// The confirmation after a publish (AC-004.5): the record's address and
/// the way back.
pub fn published_page(at_uri: &str) -> String {
    page(
        "Published",
        html! {
            section class="outcome" {
                h1 { "Published to your repo" }
                p { "Your claim is in your own repo at " code { (at_uri) } }
                p {
                    "To retract it, open your profile and press Retract: a retraction is a \
                     new record in your repo that points at this one."
                }
                (back_to_queue())
            }
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

/// The heading of a failed publish: "Nothing was published" only when the
/// app knows it (U-45).
fn publish_failed_heading(written: FailedWrite) -> &'static str {
    match written {
        FailedWrite::Definite => "Nothing was published",
        FailedWrite::MayHaveStored => "Your claim may have been saved",
    }
}

/// What a Retry does after a publish the PDS may have stored (U-45).
const PUBLISH_RETRY_CHECKS: &str = "Press Retry to check; it never publishes a second copy.";

/// A publish that did not land (AC-004.6): the suggestion stays pending and
/// the owner can try again. Nothing was written unless the PDS may have
/// stored it (`written`), which the page then says (U-45).
pub fn publish_failed_page(
    reason: &str,
    written: FailedWrite,
    next_step: PublishNextStep<'_>,
) -> String {
    let heading = publish_failed_heading(written);
    page(
        heading,
        html! {
            section class="outcome" {
            h1 { (heading) }
            p role="alert" { (reason) }
            p { "Your suggestion is still pending." }
            @match next_step {
                PublishNextStep::Retry(retry) => {
                    @if written == FailedWrite::MayHaveStored {
                        p { (PUBLISH_RETRY_CHECKS) }
                    }
                    (plan_form(PUBLISH_ACTION, retry.plan_id, retry.csrf_token, RETRY_LABEL))
                }
                PublishNextStep::SignInAgain => {
                    p { a href="/" { (SIGN_IN_LABEL) } }
                }
                PublishNextStep::BackToQueue => {}
            }
            (back_to_queue())
            }
        },
    )
}

/// A confirm that arrived after its preview expired (ADR-074): refused,
/// nothing written.
pub fn plan_expired_page() -> String {
    page(
        "This preview has expired",
        html! {
            section class="outcome" {
            h1 { "This preview has expired" }
            p role="alert" {
                "Previews can be published for 30 minutes. Nothing was published; \
                 your suggestion is still pending. Press Approve again to see a fresh preview."
            }
            (back_to_queue())
            }
        },
    )
}
