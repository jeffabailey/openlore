//! Review R3 (crafter-owned): a page action asks GitHub for each verified
//! account's repos at most once per request, and renders its page from that
//! one listing. Driven over HTTP through the acceptance support (read-only
//! reuse of its world and browser).

#[path = "../../../tests/acceptance/support/review_app/mod.rs"]
mod review_app;

use review_app::repos_and_accounts::repos::*;
use review_app::repos_and_accounts::*;
use review_app::*;

const BOTH: [Account; 2] = [Account::Priyaraman, Account::PriyaTidal];

/// The last request GitHub has seen so far.
fn last_seq(world: &ReviewWorld) -> u64 {
    world
        .github
        .requests()
        .last()
        .map_or(0, |request| request.seq)
}

/// How many times each of `accounts` was listed after request `since`, up
/// to and including request `until`.
fn listings_between(
    world: &ReviewWorld,
    (since, until): (u64, u64),
    accounts: &[Account],
) -> Vec<(String, usize)> {
    let requests = world.github.requests();
    accounts
        .iter()
        .map(|account| {
            let listing = format!("/users/{}/repos", account.login());
            let count = requests
                .iter()
                .filter(|r| (since + 1..=until).contains(&r.seq))
                .filter(|r| r.path.eq_ignore_ascii_case(&listing))
                .count();
            (account.login().to_string(), count)
        })
        .collect()
}

fn once_each(accounts: &[Account]) -> Vec<(String, usize)> {
    accounts
        .iter()
        .map(|account| (account.login().to_string(), 1))
        .collect()
}

#[test]
fn a_save_lists_each_verified_account_once() {
    let world = rra_world();
    let mut browser = given_accounts_verified(&world, &BOTH);
    when_opens_choose_repos(&mut browser);
    let since = last_seq(&world);

    when_submits_choice_on_open_page(&mut browser, Account::Priyaraman, &PRIYA_TOP_5);
    let until = last_seq(&world);

    assert!(
        browser.page.shows("Saved. Your next scan reads"),
        "{}",
        browser.page.text()
    );
    assert_eq!(
        listings_between(&world, (since, until), &BOTH),
        once_each(&BOTH)
    );
}

#[test]
fn an_add_repo_lists_each_verified_account_once() {
    let world = rra_world();
    let mut browser = given_accounts_verified(&world, &BOTH);
    when_opens_choose_repos(&mut browser);
    let since = last_seq(&world);

    browser.fill(IMPORT_FIELD, &CARGO.full());
    browser.press(ADD_REPO);
    let until = last_seq(&world);

    assert!(
        imported_list(&mut browser).contains(&CARGO.full()),
        "{}",
        browser.page.text()
    );

    assert_eq!(
        listings_between(&world, (since, until), &BOTH),
        once_each(&BOTH)
    );
    assert_eq!(world.github.repo_facts_reads_of(&CARGO.full()), 1);
}
