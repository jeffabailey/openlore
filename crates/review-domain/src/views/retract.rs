//! Retraction (US-BRA-011): a new record that points at the claim, never
//! a delete.

use maud::html;

use super::*;
use crate::failed_write::FailedWrite;
use crate::plans::RetractPlan;
use ports::claim_domain::identity_link::is_reserved_predicate;
use ports::claim_domain::UnsignedClaim;

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
    /// The claim `plan` retracts: the headline names it by its own fields,
    /// since a link's retraction restates none of them (U-39).
    pub retracted: &'a UnsignedClaim,
    pub csrf_token: &'a str,
    pub profile_path: &'a str,
}

pub fn retract_preview_page(view: &RetractPreview<'_>) -> String {
    page(
        "Retract this claim?",
        html! {
            h1 { "Retract this claim?" }
            p { (retracted_headline(view.retracted)) }
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

/// The claim being retracted: an identity link as its record's own triple
/// (it names no repo or philosophy), any other claim by its headline.
fn retracted_headline(claim: &UnsignedClaim) -> Markup {
    if is_reserved_predicate(&claim.predicate) {
        html! { code { (claim.subject) " " (claim.predicate) " " (claim.object) } }
    } else {
        claim_headline(&claim.subject, &claim.object)
    }
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

/// The heading of a failed retraction: "Nothing was retracted" only when the
/// app knows it (U-45).
fn retract_failed_heading(written: FailedWrite) -> &'static str {
    match written {
        FailedWrite::Definite => "Nothing was retracted",
        FailedWrite::MayHaveStored => "Your retraction may have been saved",
    }
}

/// What a Retry does after a retraction the PDS may have stored (U-45).
const RETRACT_RETRY_CHECKS: &str = "Press Retry to check; it never adds a second retraction.";

/// A retraction that did not land (AC-011.4): the owner can try again
/// (`retry`: the same plan, once). The claim is still active unless the PDS
/// may have stored the retraction (`written`), which the page then says
/// (U-45).
pub fn retract_failed_page(
    reason: &str,
    written: FailedWrite,
    retry: Option<PublishRetry<'_>>,
    profile_path: &str,
) -> String {
    let heading = retract_failed_heading(written);
    page(
        heading,
        html! {
            h1 { (heading) }
            p role="alert" { (reason) }
            @match written {
                FailedWrite::Definite => {
                    p { "Your claim is still active and still on your profile." }
                }
                FailedWrite::MayHaveStored => {
                    @if retry.is_some() {
                        p { (RETRACT_RETRY_CHECKS) }
                    }
                }
            }
            @if let Some(retry) = retry {
                (plan_form(RETRACT_ACTION, retry.plan_id, retry.csrf_token, RETRY_LABEL))
            }
            (back_to_profile(profile_path))
        },
    )
}

#[cfg(test)]
mod tests {
    //! 03-09: the retract preview of a link names the link it retracts, not
    //! the retraction's own (empty, U-39) fields.
    use super::*;
    use crate::link_plan::link_plan;
    use crate::plans::retract_plan;
    use ports::claim_domain::identity_link::GithubObservation;
    use ports::claim_domain::identity_link::{check_link, link_predicate, ProofCheck};
    use ports::claim_domain::Did;
    use proptest::prelude::*;

    proptest! {
        /// Universe: owner DIDs × GitHub accounts. The headline is the
        /// original link's subject, predicate and object; nothing in it is
        /// built from the empty retraction.
        #[test]
        fn a_link_retract_preview_names_the_link_it_retracts(
            did in "[a-z2-7]{24}".prop_map(|s| format!("did:plc:{s}")),
            login in "[a-z][a-z0-9-]{0,15}",
            user_id in 1u64..=u64::from(u32::MAX),
        ) {
            let observation = GithubObservation { login: login.clone(), user_id, bio: Some(format!("my DID: {did}")) };
            let Ok(ProofCheck::Verified(proven)) = check_link(&Did(did.clone()), &observation) else {
                panic!("the bio names the DID");
            };
            let link = link_plan(&did, &proven, "2026-10-17T09:30:00Z").expect("link plan");
            let retraction = retract_plan(&did, link.rkey(), link.claim(), "2026-10-17T10:00:00Z").expect("retraction");

            let html = retract_preview_page(&RetractPreview {
                plan: &retraction,
                retracted: link.claim(),
                csrf_token: "csrf",
                profile_path: "/@priya.example",
            });

            let headline = format!("<code>{did} {} github:{login}</code>", link_predicate());
            prop_assert!(html.contains(&headline), "{}", html);
            prop_assert!(!html.contains("https://github.com//"), "{}", html);
            prop_assert!(!html.contains(" embodies "), "{}", html);
        }
    }
}
