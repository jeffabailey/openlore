//! Retraction (US-BRA-011): a new record that points at the claim, never
//! a delete.

use maud::html;

use super::*;
use crate::plans::RetractPlan;

/// Where a retract plan is confirmed (or retried).
const RETRACT_ACTION: &str = "/retract";

/// The owner's way from a profile card to the retract preview.
pub const RETRACT_LABEL: &str = "Retract";

/// The only button that retracts.
pub const CONFIRM_RETRACTION_LABEL: &str = "Confirm retraction";

/// What a retraction is (AC-011.1, AC-011.2).
pub const RETRACTION_NOTICE: &str = "A public retraction referencing this claim will be added to \
     your repo. The original stays readable, marked retracted.";

/// The retract preview: which claim, what will be added, and the one
/// button that adds it. Nothing is written here.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RetractPreview<'a> {
    pub plan: &'a RetractPlan,
    pub csrf_token: &'a str,
    pub profile_path: &'a str,
}

pub fn retract_preview_page(view: &RetractPreview<'_>) -> String {
    let claim = view.plan.claim();
    page(
        "Retract this claim?",
        html! {
            h1 { "Retract this claim?" }
            p { (claim_headline(&claim.subject, &claim.object)) }
            p { strong { (RETRACTION_NOTICE) } }
            p {
                "The retraction is added at " code { (view.plan.at_uri()) }
                " and references " code { (view.plan.retracted_cid()) } "."
            }
            p { "Nothing has been retracted yet." }
            (plan_form(RETRACT_ACTION, view.plan.rkey(), view.csrf_token, CONFIRM_RETRACTION_LABEL))
            p { a href=(view.profile_path) { (CANCEL_LABEL) } }
        },
    )
}

/// The retraction landed (or the claim already was retracted): the claim
/// no longer shows on the profile; the original record is untouched.
pub fn retracted_page(at_uri: Option<&str>, profile_path: &str) -> String {
    page(
        "Retracted",
        html! {
            h1 { "Retracted" }
            @match at_uri {
                Some(uri) => p { "Your retraction is in your repo at " code { (uri) } },
                None => p { "This claim was already retracted; nothing more was added." },
            }
            p { "The original claim stays readable, marked retracted. It no longer shows on your profile." }
            (back_to_profile(profile_path))
        },
    )
}

/// A retraction that did not land (AC-011.4): the claim is still active and
/// the owner can try again (`retry`: the same plan, once).
pub fn retract_failed_page(
    reason: &str,
    retry: Option<PublishRetry<'_>>,
    profile_path: &str,
) -> String {
    page(
        "Nothing was retracted",
        html! {
            h1 { "Nothing was retracted" }
            p role="alert" { (reason) }
            p { "Your claim is still active and still on your profile." }
            @if let Some(retry) = retry {
                (plan_form(RETRACT_ACTION, retry.plan_id, retry.csrf_token, RETRY_LABEL))
            }
            (back_to_profile(profile_path))
        },
    )
}
