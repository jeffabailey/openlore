//! review-app-repos-and-accounts — crafter-owned regression checks from the
//! adversarial review (F1-F3): publishing is gated by the permit alone
//! (ADR-089 §2), never by a stored verified flag, and a refused confirm
//! keeps its preview.
//!
//! Driving port: the REAL `openlore-review-app` over HTTP, the same fakes as
//! `review_app_repos_and_accounts_several.rs`.

#[path = "support/review_app/mod.rs"]
mod review_app;

use review_app::repos_and_accounts::repos::*;
use review_app::repos_and_accounts::*;
use review_app::*;

const BOTH: [Account; 2] = [Account::Priyaraman, Account::PriyaTidal];
const PUBLISH: &str = "Publish to my repo";

/// F2: the first listed account stored unverified does not stop a repo of
/// the second listed account from publishing while that account's bio,
/// read at confirm, holds the DID.
#[test]
fn a_second_accounts_repo_publishes_though_the_first_account_is_stored_unverified() {
    let world = rra_world();
    let mut browser = given_accounts_verified(&world, &BOTH);
    given_saved_choice(&mut browser, Account::PriyaTidal, &[INGEST_BENCH]);
    given_scanned(&world, &mut browser);
    when_previews_approval(&mut browser, INGEST_BENCH.embodies(Philosophy::TestDriven));
    let preview = publish_form(&browser);
    removes_did_from_account_bio(&world, Account::Priyaraman);
    when_scans(&world, &mut browser);
    assert_eq!(
        account_list(&mut browser),
        vec![
            ("priyaraman".to_string(), false),
            ("priya-tidal".to_string(), true)
        ],
        "precondition: the first account is stored unverified"
    );

    browser.submit(&preview, Some(PUBLISH));

    assert_eq!(
        claim_subjects(&world, Persona::Priya),
        [INGEST_BENCH.subject()],
        "{}",
        browser.page.text()
    );
}

/// F3 + F1: a confirm refused because the bio lost the DID keeps the
/// preview; once the DID is back, confirming the same preview publishes,
/// and the account the fresh read proved is verified again.
#[test]
fn a_refused_confirm_keeps_the_preview_and_a_later_confirm_publishes_and_reverifies() {
    let world = rra_world();
    let mut browser = given_accounts_verified(&world, &BOTH);
    given_saved_choice(&mut browser, Account::PriyaTidal, &[INGEST_BENCH]);
    given_scanned(&world, &mut browser);
    when_previews_approval(&mut browser, INGEST_BENCH.embodies(Philosophy::TestDriven));
    let preview = publish_form(&browser);
    removes_did_from_account_bio(&world, Account::PriyaTidal);
    when_confirms_publish(&mut browser);
    assert!(world.atproto.write_attempts().is_empty(), "nothing written");
    puts_did_in_account_bio(&world, Account::PriyaTidal);

    browser.submit(&preview, Some(PUBLISH));

    assert_eq!(
        claim_subjects(&world, Persona::Priya),
        [INGEST_BENCH.subject()],
        "{}",
        browser.page.text()
    );
    assert_eq!(
        account_list(&mut browser),
        vec![
            ("priyaraman".to_string(), true),
            ("priya-tidal".to_string(), true)
        ],
        "the fresh read at confirm verified priya-tidal again"
    );
}

/// The "Publish to my repo" form of the preview on the page.
fn publish_form(browser: &Browser) -> html::Form {
    browser
        .forms()
        .into_iter()
        .find(|form| {
            form.buttons
                .iter()
                .any(|(label, _, _)| html::label_matches(label, PUBLISH))
        })
        .expect("the preview offers Publish to my repo")
}
