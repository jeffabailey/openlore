//! owner-only-signing — slice 4 remainder: the review app's share draft
//! quotes only claims whose authority still holds (UC-21, ADR-086 §8).
//!
//! Reconciliation (DISTILL 2026-10-20, `distill/wave-decisions.md` Upstream
//! Issues): the rest of slice 4 already shipped — the publish-time owner
//! check (ADR-089, `review_app_repos_and_accounts_several.rs` AP-1/AP-2/AP-4,
//! `review_app_publish_permit_alone.rs`) and the identity-link prompt
//! (github-did-identity-link slice 3, `review_app_identity_link.rs`). Only the
//! share-draft narrowing is new here.
//!
//! "Attested" in the app (which keeps no CLI check store, C-OOS-4) is read as
//! the owner tier holding NOW: the share request re-reads each quoted repo's
//! owner and that owner's bio, as publish does (ADR-089 §4). This reading is
//! a DISTILL proposal pending U-OOS-4.
//!
//! Driving port: the REAL `openlore-review-app` over HTTP. Fakes:
//! `FakeGithubAccounts`, the ATProto network. Layer 4: example-only.

#[path = "support/review_app/mod.rs"]
mod review_app;

use review_app::repos_and_accounts::repos::*;
use review_app::repos_and_accounts::*;
use review_app::*;

/// The pinned line for a published claim the draft does not quote.
const NOT_COUNTED_LINE: &str = "not counted: authority not proven";

/// SA-1
/// ```gherkin
/// @driving_port @real-io @uc-21 @adr-086-8 @spec-ambiguity @contract-shape:pure-function
/// Scenario: Priya's share post names only claims about repos she still owns
///   Given Priya published "tidepool is test-driven" and "kelp pins its dependencies"
///   And she then transferred kelp to tidal-labs
///   When she opens the share preview
///   Then the draft names test-driven and not dependency-pinning
///   And the page lists kelp's claim as "not counted: authority not proven"
///   And nothing has been posted
/// ```
#[test]
#[ignore = "DISTILL owner-only-signing SA-1: pending DELIVER (slice 4; U-OOS-4 ruling)"]
fn priyas_share_post_names_only_claims_about_repos_she_still_owns() {
    let world = rra_world();
    let mut browser = given_owner_verified(&world, Account::Priyaraman);
    given_scanned(&world, &mut browser);
    given_published_one(&world, &mut browser, TIDEPOOL.embodies(Philosophy::TestDriven));
    given_published_one(
        &world,
        &mut browser,
        KELP.embodies(Philosophy::DependencyPinning),
    );
    world
        .github
        .transfer_repo(KELP.owner, KELP.name, KELP_AT_TIDAL_LABS.owner);

    when_opens_share_preview(&mut browser, Persona::Priya);

    let draft = browser
        .forms()
        .iter()
        .flat_map(|f| f.fields.clone())
        .find(|(k, _)| k == "text")
        .map(|(_, v)| v)
        .unwrap_or_else(|| panic!("an editable post text:\n{}", browser.page.text()));
    assert!(draft.contains("test-driven"), "{draft}");
    assert!(!draft.contains("dependency-pinning"), "{draft}");
    let page = browser.page.text();
    assert!(
        page.contains(NOT_COUNTED_LINE) && page.contains(&KELP.full()),
        "kelp's claim is listed as not counted:\n{page}"
    );
    assert!(posts_in_pds(&world, Persona::Priya).is_empty(), "nothing posted");
}
