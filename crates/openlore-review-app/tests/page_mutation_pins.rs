//! review-app-repos-and-accounts Phase 5 (crafter-owned): behaviour the
//! mutation run found unpinned, driven over HTTP through the acceptance
//! support (read-only reuse of its world and browser).

#[path = "../../../tests/acceptance/support/review_app/mod.rs"]
mod review_app;

use review_app::repos_and_accounts::repos::*;
use review_app::repos_and_accounts::*;
use review_app::*;

const BOTH: [Account; 2] = [Account::Priyaraman, Account::PriyaTidal];
const PUBLISH: &str = "Publish to my repo";

/// Every page action a signed-in owner posts (sign-in and sign-out aside),
/// the destructive ones last.
const PAGE_POSTS: [&str; 20] = [
    "/github",
    "/github/link",
    "/github/link/publish",
    "/github/remove",
    "/github/remove/confirm",
    "/scan",
    "/review/approve",
    "/review/edit",
    "/review/edit/preview",
    "/review/decline",
    "/review/undo",
    "/review/publish",
    "/review/approve-selected",
    "/review/publish-selected",
    "/share",
    "/retract",
    "/repos",
    "/repos/imports",
    "/repos/imports/remove",
    "/repos/imports/remove/confirm",
];

/// POST `path` with the browser's session cookie and its valid CSRF token,
/// as a page on another origin would; the status the app answers.
fn post_from_elsewhere(browser: &Browser, path: &str, csrf: &str) -> u16 {
    let cookies = browser
        .app_cookies()
        .iter()
        .map(|(name, value)| format!("{name}={value}"))
        .collect::<Vec<_>>()
        .join("; ");
    reqwest::blocking::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .expect("client")
        .post(format!("{}{path}", browser.origin()))
        .header("cookie", cookies)
        .header("origin", "https://elsewhere.example")
        .header("content-type", "application/x-www-form-urlencoded")
        .body(format!("csrf={}", openlore_test_support::url_encode(csrf)))
        .send()
        .expect("the app answers")
        .status()
        .as_u16()
}

/// Every page action refuses a request from another origin, even one that
/// carries the owner's session and CSRF token.
#[test]
fn a_page_action_from_another_origin_is_forbidden() {
    let world = rra_world();
    let mut browser = given_owner_verified(&world, Account::Priyaraman);
    browser.open("/repos");
    let csrf = csrf_token(&browser);
    assert!(!csrf.is_empty(), "{}", browser.page.text());

    let refused: Vec<(&str, u16)> = PAGE_POSTS
        .iter()
        .map(|path| (*path, post_from_elsewhere(&browser, path, &csrf)))
        .filter(|(_, status)| *status != 403)
        .collect();

    assert!(refused.is_empty(), "answered, not forbidden: {refused:?}");
    assert_eq!(
        post_from_elsewhere(&browser, "/settings/forget", &csrf),
        403
    );
}

/// A Save naming an account she does not list as verified is not found,
/// whatever her other accounts.
#[test]
fn a_save_for_an_account_she_does_not_list_verified_is_not_found() {
    let world = rra_world();
    let mut browser = given_owner_verified(&world, Account::Priyaraman);
    browser.open("/repos");
    let csrf = csrf_token(&browser);
    let not_hers = Account::Samortega.id().to_string();

    let status = browser
        .post(
            "/repos",
            &[
                ("csrf", csrf.as_str()),
                ("github_user_id", not_hers.as_str()),
            ],
        )
        .status;

    assert_eq!(status, 404, "{}", browser.page.text());
}

/// The repos page answers 422 for a refused change and 200 for one made.
#[test]
fn the_repos_page_answers_422_only_for_a_refused_change() {
    let world = rra_world();
    let mut browser = given_owner_verified(&world, Account::Priyaraman);
    browser.open("/repos");

    let refused = when_imports(&mut browser, "not a repo").status;
    let saved = when_saves_choice(&mut browser, Account::Priyaraman, &PRIYA_TOP_5).status;

    assert_eq!((refused, saved), (422, 200), "{}", browser.page.text());
}

/// With no verified account and no import she sees no queue, so a decline
/// posted from an earlier page is not found and changes nothing.
#[test]
fn a_decline_without_a_verified_account_or_an_import_is_not_found() {
    let world = rra_world();
    let mut browser = given_owner_verified(&world, Account::Priyaraman);
    given_scanned(&world, &mut browser);
    browser.open("/review");
    let decline = browser
        .forms()
        .into_iter()
        .find(|form| form.action == "/review/decline")
        .expect("a pending card offers a decline");
    removes_did_from_account_bio(&world, Account::Priyaraman);
    when_scans(&world, &mut browser);
    assert_eq!(
        account_list(&mut browser),
        vec![("priyaraman".to_string(), false)],
        "precondition: her only account is stored unverified"
    );

    let status = browser.submit(&decline, None).status;

    assert_eq!(status, 404, "{}", browser.page.text());
}

/// A refused add of another account is counted by why it was refused
/// (KPI-RRA-2).
#[test]
fn a_refused_add_of_another_account_is_counted() {
    let world = rra_world();
    let mut browser = given_owner_verified(&world, Account::Priyaraman);
    removes_did_from_account_bio(&world, Account::PriyaTidal);

    when_adds_account(&mut browser, Account::PriyaTidal);

    assert_eq!(kpi(&world, "account.add.refused.did_missing"), 1);
}

/// A preview of `INGEST_BENCH` embodies test-driven, from priya-tidal.
fn ingest_bench_preview(world: &ReviewWorld) -> (Browser, html::Form) {
    let mut browser = given_accounts_verified(world, &BOTH);
    given_saved_choice(&mut browser, Account::PriyaTidal, &[INGEST_BENCH]);
    given_scanned(world, &mut browser);
    when_previews_approval(&mut browser, INGEST_BENCH.embodies(Philosophy::TestDriven));
    let form = browser
        .forms()
        .into_iter()
        .find(|form| {
            form.buttons
                .iter()
                .any(|(label, _, _)| html::label_matches(label, PUBLISH))
        })
        .expect("the preview offers Publish to my repo");
    (browser, form)
}

/// A confirm re-aimed at another suggestion than its plan's publishes
/// nothing; the plan is kept, so the preview's own confirm still publishes.
#[test]
fn a_confirm_re_aimed_at_another_suggestion_publishes_nothing() {
    let world = rra_world();
    let (mut browser, preview) = ingest_bench_preview(&world);
    let re_aimed = Browser::with_field(
        &preview,
        "suggestion",
        "github:priya-tidal/ingest-bench embodiesPhilosophy org.openlore.philosophy.elsewhere",
    );

    browser.submit(&re_aimed, Some(PUBLISH));
    assert!(
        claim_subjects(&world, Persona::Priya).is_empty(),
        "{}",
        browser.page.text()
    );

    browser.submit(&preview, Some(PUBLISH));
    assert_eq!(
        claim_subjects(&world, Persona::Priya),
        [INGEST_BENCH.subject()],
        "{}",
        browser.page.text()
    );
}

/// A repo GitHub no longer serves is refused as gone at confirm, not as an
/// unfinished check (which KPI-RRA-4 would count as unreachable).
#[test]
fn a_repo_gone_at_confirm_is_refused_as_gone() {
    let world = rra_world();
    let (mut browser, _) = ingest_bench_preview(&world);
    world.github.delete_account(Account::PriyaTidal.login());

    when_confirms_publish(&mut browser);

    assert!(world.atproto.write_attempts().is_empty(), "nothing written");
    assert_eq!(kpi(&world, "publish.refused.unreachable"), 0);
}

/// The choose page's GitHub listings are spent from her hourly page
/// allowance (R3): opened again and again, it stops asking GitHub before
/// the opens run out.
#[test]
fn the_choose_page_stops_listing_once_her_hourly_allowance_is_spent() {
    const OPENS: usize = 220;
    let world = rra_world();
    let mut browser = given_owner_verified(&world, Account::Priyaraman);
    let listing = format!("/users/{}/repos", Account::Priyaraman.login());
    let listed = |world: &ReviewWorld| {
        world
            .github
            .requests()
            .iter()
            .filter(|request| request.path.eq_ignore_ascii_case(&listing))
            .count()
    };
    let before = listed(&world);

    for _ in 0..OPENS {
        browser.open("/repos");
    }

    let listings = listed(&world) - before;
    assert!(
        (1..OPENS).contains(&listings),
        "{listings} listings for {OPENS} opens"
    );
}

/// How many times `account`'s repos were listed after GitHub request `since`.
fn listed_since(world: &ReviewWorld, account: Account, since: u64) -> usize {
    let listing = format!("/users/{}/repos", account.login());
    world
        .github
        .requests()
        .iter()
        .filter(|request| request.seq > since && request.path.eq_ignore_ascii_case(&listing))
        .count()
}

/// A verify that moves an import lists her other verified accounts too (the
/// move's limit counts their ticks), each once.
#[test]
fn a_verify_that_moves_an_import_lists_her_other_verified_account() {
    let world = rra_world();
    creates_repo(&world, TIDAL_SDK_NOTES);
    let mut browser = given_owner_verified(&world, Account::Priyaraman);
    given_imported(&mut browser, TIDAL_SDK_NOTES);
    puts_did_in_account_bio(&world, Account::PriyaTidal);
    let since = github_mark(&world);

    when_adds_account(&mut browser, Account::PriyaTidal);

    assert!(imported_list(&mut browser).is_empty(), "moved");
    assert_eq!(
        listed_since(&world, Account::Priyaraman, since),
        2,
        "the move's and the choose page's"
    );
}
