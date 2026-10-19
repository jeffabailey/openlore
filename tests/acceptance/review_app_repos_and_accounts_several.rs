//! review-app-repos-and-accounts — Release 2: several GitHub accounts on one
//! sign-in (US-RRA-003, US-RRA-004), removing one (US-RRA-008), and publishing
//! that follows who owns the repo right now (US-RRA-010, BR-RRA-2, ADR-089).
//!
//! Driving port: the REAL `openlore-review-app` over HTTP. Fakes:
//! `FakeGithubAccounts` (several accounts, per-account bio outage, transfers)
//! and the ATProto network (the owner's PDS observes every write attempt).
//! Layer 4: example-only; refusal families are tables (Mandate 9/11).
//!
//! Every scenario is `#[ignore = "DELIVER 02-NN: …"]`.
//!
//! State machine (C2a) of a listed GitHub account, per signed-in DID:
//!
//! | From | Event | To |
//! |---|---|---|
//! | (not listed) | add + bio holds DID | verified |
//! | (not listed) | add + DID missing / held by another DID / already listed / at limit / GitHub unreachable | (not listed), nothing changes |
//! | verified | scan or publish reads a bio without the DID | unverified (its suggestions hidden, not deleted) |
//! | verified / unverified | scan or publish cannot finish the bio read | unchanged (an unfinished check is not a failed check) |
//! | unverified | verify again + bio holds DID | verified (its suggestions shown again) |
//! | verified / unverified | remove + confirm | (not listed): pending deleted, declined/published keys kept |
//! | verified / unverified | remove + cancel | unchanged |

#[path = "support/review_app/mod.rs"]
mod review_app;

use review_app::repos_and_accounts::repos::*;
use review_app::repos_and_accounts::*;
use review_app::state_delta::{assert_state_delta, Delta};
use review_app::*;

const BOTH: [Account; 2] = [Account::Priyaraman, Account::PriyaTidal];

// =============================================================================
// US-RRA-003 — verify a second GitHub account on the same sign-in
// =============================================================================

/// AA-1
/// ```gherkin
/// @US-RRA-003 @AC-003.1 @AC-003.6 @C-GDL-1 @happy @kpi @KPI-RRA-2 @contract-shape:bounded-change
/// Scenario: Priya verifies her work account without losing her personal one
///   Given Priya is verified as github.com/priyaraman
///   And the bio of github.com/priya-tidal contains her DID
///   When she adds GitHub username "priya-tidal" (adding checks its bio at once)
///   Then she sees "Verified: github.com/priya-tidal belongs to @priyaraman.bsky.social"
///   And her account list shows priyaraman and priya-tidal, both verified
///   And no record is written to her Bluesky repo
/// ```
#[test]
fn priya_verifies_her_work_account_without_losing_her_personal_one() {
    let world = rra_world();
    let mut browser = given_owner_verified(&world, Account::Priyaraman);
    let before = capture_rra(&world, &mut browser, Persona::Priya);

    when_adds_account(&mut browser, Account::PriyaTidal);

    then_sees_account_verified(&browser.page, Account::PriyaTidal, Persona::Priya);
    let after = capture_rra(&world, &mut browser, Persona::Priya);
    assert_state_delta(
        &before,
        &after,
        &rra_universe(),
        &Delta::new().with_slot(
            "accounts",
            review_app::state_delta::set_to("priyaraman:true,priya-tidal:true".to_string()),
        ),
    );
    assert!(
        world.atproto.write_attempts().is_empty(),
        "C-GDL-1: nothing published"
    );
    assert_eq!(kpi(&world, "account.add.verified"), 1);
}

/// AA-2 (refusal table; each row its own world)
/// ```gherkin
/// @US-RRA-003 @AC-003.2 @AC-003.3 @AC-003.4 @AC-003.5 @error @C6b @kpi @contract-shape:unbounded-preservation
/// Scenario Outline: An account that cannot be added changes nothing
///   Given <setup>
///   When <who> adds GitHub username "<login>"
///   Then <who> sees "<message>"
///   And the account list, the queue, the repos you don't own and the published claims are exactly as before
///   Examples:
///     | setup                                                            | who    | login       | message |
///     | Priya is verified as priyaraman; priya-tidal's bio lacks her DID | Priya  | priya-tidal | We couldn't find your DID in the bio of github.com/priya-tidal. Add your DID to the bio of your own GitHub account, then press Verify again. |
///     | dvolkov is verified for @dmitri.volkov.dev                       | Sam    | dvolkov     | github.com/dvolkov is already verified for another Bluesky account. Each GitHub account can belong to one Bluesky account only. If it is yours, remove it from that Bluesky account first. |
///     | Priya's list holds priyaraman and priya-tidal                    | Priya  | priya-tidal | github.com/priya-tidal is already on your list. |
///     | Dmitri has 3 verified accounts                                   | Dmitri | dvolkov-old | You can link up to 3 GitHub accounts. Remove one on this page to add another. |
///     | Priya is verified as priyaraman; GitHub drops the connection     | Priya  | priya-tidal | GitHub could not be reached. Please wait a few minutes, then press Verify again. Nothing was lost. |
///     | Priya is verified as priyaraman; GitHub says it is too busy to answer | Priya  | priya-tidal | GitHub could not be reached. Please wait a few minutes, then press Verify again. Nothing was lost. |
/// ```
#[test]
fn an_account_that_cannot_be_added_changes_nothing() {
    #[derive(Debug, Clone, Copy)]
    enum Refusal {
        DidMissing,
        HeldByAnotherSignIn,
        AlreadyListed,
        FourthAccount,
        GithubUnreachable,
        GithubRateLimited,
    }
    for refusal in [
        Refusal::DidMissing,
        Refusal::HeldByAnotherSignIn,
        Refusal::AlreadyListed,
        Refusal::FourthAccount,
        Refusal::GithubUnreachable,
        Refusal::GithubRateLimited,
    ] {
        let world = rra_world();
        let (mut browser, persona, adding, expected): (Browser, Persona, Account, Vec<String>) =
            match refusal {
                Refusal::DidMissing => {
                    let b = given_owner_verified(&world, Account::Priyaraman);
                    removes_did_from_account_bio(&world, Account::PriyaTidal);
                    let expected = vec!["We couldn't find your DID in the bio of github.com/priya-tidal. Add your DID to the bio of your own GitHub account, then press Verify again.".to_string()];
                    (b, Persona::Priya, Account::PriyaTidal, expected)
                }
                Refusal::HeldByAnotherSignIn => {
                    given_owner_verified(&world, Account::Dvolkov);
                    puts_did_in_account_bio(&world, Account::Dvolkov);
                    let b = given_signed_in(&world, Persona::Sam);
                    let expected = vec!["github.com/dvolkov is already verified for another Bluesky account. Each GitHub account can belong to one Bluesky account only. If it is yours, remove it from that Bluesky account first.".to_string()];
                    (b, Persona::Sam, Account::Dvolkov, expected)
                }
                Refusal::AlreadyListed => {
                    let b = given_accounts_verified(&world, &BOTH);
                    let expected =
                        vec!["github.com/priya-tidal is already on your list.".to_string()];
                    (b, Persona::Priya, Account::PriyaTidal, expected)
                }
                Refusal::FourthAccount => {
                    let b = given_accounts_verified(
                        &world,
                        &[Account::Dvolkov, Account::DvolkovWork, Account::DvolkovLab],
                    );
                    let expected = vec!["You can link up to 3 GitHub accounts. Remove one on this page to add another.".to_string()];
                    (b, Persona::Dmitri, Account::DvolkovOld, expected)
                }
                Refusal::GithubUnreachable | Refusal::GithubRateLimited => {
                    let b = given_owner_verified(&world, Account::Priyaraman);
                    let expected = vec!["GitHub could not be reached. Please wait a few minutes, then press Verify again. Nothing was lost.".to_string()];
                    (b, Persona::Priya, Account::PriyaTidal, expected)
                }
            };
        let before = capture_rra(&world, &mut browser, persona);
        match refusal {
            Refusal::GithubUnreachable => world.github.set_offline(true),
            Refusal::GithubRateLimited => world.github.rate_limit_for(600),
            _ => {}
        }

        when_adds_account(&mut browser, adding);

        world.github.set_offline(false);
        world.github.clear_rate_limit();
        let text = browser.page.text();
        assert!(
            expected.iter().all(|e| text.contains(e.as_str())),
            "{refusal:?}: expected {expected:?}:\n{text}"
        );
        let after = capture_rra(&world, &mut browser, persona);
        assert_state_delta(&before, &after, &rra_universe(), &Delta::new());
    }
}

/// AA-3
/// ```gherkin
/// @US-RRA-003 @AC-003.7 @AC-001.1 @AC-004.6 @BR-RRA-3 @boundary @contract-shape:unbounded-preservation
/// Scenario: A new account's default ticks fit the limit
///   Given Priya has 8 repos chosen on priyaraman, no imported repo, and the per-scan limit is 10
///   And she just verified github.com/priya-tidal, which owns 4 public repos
///   When she opens "Choose repos"
///   Then only the 2 best priya-tidal repos, ingest-bench and tidal-sdk, are ticked
///   And she sees "2 of 4 ticked: your limit of 10 repos is reached. Untick others to choose more."
/// ```
#[test]
fn a_new_accounts_default_ticks_fit_the_limit() {
    let world = rra_world();
    let mut browser = given_owner_verified(&world, Account::Priyaraman);
    creates_repo(&world, Repo::new("priyaraman", "extra-one"));
    let eight: Vec<Repo> = PRIYA_OWNED_RANKED
        .iter()
        .copied()
        .chain([Repo::new("priyaraman", "extra-one")])
        .collect();
    given_saved_choice(&mut browser, Account::Priyaraman, &eight);

    when_adds_account(&mut browser, Account::PriyaTidal);

    let page = when_opens_choose_repos(&mut browser).clone();

    assert_eq!(
        ticked_repos(&page, Account::PriyaTidal),
        fulls(&TIDAL_RANKED[..2])
    );
    assert!(
        page.shows(
            "2 of 4 ticked: your limit of 10 repos is reached. Untick others to choose more."
        ),
        "{}",
        page.text()
    );
}

// =============================================================================
// US-RRA-004 — one scan reads every verified account's chosen repos
// =============================================================================

/// AS-1
/// ```gherkin
/// @US-RRA-004 @AC-004.1 @AC-004.5 @happy @kpi @KPI-RRA-1 @contract-shape:bounded-change
/// Scenario: One scan covers both accounts and each card names its account
///   Given Priya chose 5 repos on priyaraman and 2 on priya-tidal, and both bios contain her DID
///   When she presses "Scan again"
///   Then 7 repos are read under one of her 6 scans today
///   And the summary says "Ownership re-checked: priyaraman ✓ · priya-tidal ✓"
///   And the card for priya-tidal/ingest-bench says "from github.com/priya-tidal"
/// ```
#[test]
#[ignore = "DELIVER 02-04: one scan, every account, each card names its account"]
fn one_scan_covers_both_accounts_and_each_card_names_its_account() {
    let world = rra_world();
    let mut browser = given_accounts_verified(&world, &BOTH);
    given_saved_choice(&mut browser, Account::Priyaraman, &PRIYA_CHOICE);
    given_saved_choice(
        &mut browser,
        Account::PriyaTidal,
        &[INGEST_BENCH, TIDAL_SDK],
    );

    let read = given_scanned(&world, &mut browser);

    let mut expected = set_of(&PRIYA_CHOICE);
    expected.extend(set_of(&[INGEST_BENCH, TIDAL_SDK]));
    assert_eq!(read, expected);
    assert!(
        scan_summary(&mut browser).contains("Ownership re-checked: priyaraman ✓ · priya-tidal ✓"),
        "{}",
        browser.page.text()
    );
    let card = cards_about(&mut browser, INGEST_BENCH);
    assert!(
        !card.is_empty()
            && card
                .iter()
                .all(|c| c.text.contains("from github.com/priya-tidal")),
        "{card:?}"
    );
    given_scans_used(&world, &mut browser, 5);
    when_presses_scan(&mut browser);
    assert!(
        browser.page.shows(DAILY_LIMIT_REACHED),
        "the two-account scan was ONE of the 6 a day:\n{}",
        browser.page.text()
    );
}

/// AS-2
/// ```gherkin
/// @US-RRA-004 @AC-004.2 @AC-004.3 @AC-004.4 @D-12 @error @KPI-RRA-5 @contract-shape:bounded-change
/// Scenario: An account that fails its check is skipped and named, and its suggestions are hidden
///   Given Priya has pending suggestions from priyaraman and priya-tidal
///   And the bio of github.com/priya-tidal no longer contains her DID
///   When she presses "Scan again"
///   Then no priya-tidal repo is read and her default priyaraman repos are
///   And the summary says "Your DID is not in the bio of github.com/priya-tidal, so its repos were not read and its suggestions are hidden. Put your DID back, then verify."
///   And priya-tidal's suggestions are hidden while priyaraman's stay shown
/// ```
#[test]
#[ignore = "DELIVER 02-05: a failing account is skipped and its suggestions hidden"]
fn an_account_that_fails_its_check_is_skipped_named_and_hidden() {
    let world = rra_world();
    let mut browser = given_accounts_verified(&world, &BOTH);
    given_saved_choice(
        &mut browser,
        Account::PriyaTidal,
        &[INGEST_BENCH, TIDAL_SDK],
    );
    given_scanned(&world, &mut browser);
    let personal = pending_on_queue(&mut browser)
        .into_iter()
        .filter(|p| p.starts_with("priyaraman/"))
        .collect::<Vec<_>>();
    removes_did_from_account_bio(&world, Account::PriyaTidal);

    let read = given_scanned(&world, &mut browser);

    assert!(
        read.iter().all(|r| !r.starts_with("priya-tidal/")),
        "{read:?}"
    );
    assert_eq!(read, set_of(&PRIYA_TOP_5));
    let summary = scan_summary(&mut browser);
    assert!(
        summary.contains(&failed_check_summary(Account::PriyaTidal)),
        "{summary}"
    );
    assert!(
        phrases_about(&mut browser, INGEST_BENCH).is_empty(),
        "hidden"
    );
    let shown: Vec<String> = pending_on_queue(&mut browser)
        .into_iter()
        .filter(|p| p.starts_with("priyaraman/"))
        .collect();
    assert_eq!(shown, personal, "the other account is unaffected");
    assert!(
        world
            .github
            .scrapes_without_preceding_bio_check("priya-tidal")
            .is_empty(),
        "KPI-RRA-5: no repo read after a failed bio"
    );
}

/// AS-2b (chained on AS-2)
/// ```gherkin
/// @US-RRA-004 @AC-004.3 @AC-004.4 @D-12 @contract-shape:bounded-change
/// Scenario: Hidden suggestions come back after re-verifying
///   Given priya-tidal failed its check in a scan and its 2 pending suggestions are hidden
///   And Priya put her DID back in the priya-tidal bio
///   When she verifies github.com/priya-tidal on the GitHub step
///   Then the same 2 suggestions are shown in her queue again
/// ```
#[test]
#[ignore = "DELIVER 02-20: re-verifying shows the hidden suggestions again (hidden, not deleted)"]
fn hidden_suggestions_come_back_after_re_verifying() {
    let world = rra_world();
    let mut browser = given_accounts_verified(&world, &BOTH);
    given_saved_choice(
        &mut browser,
        Account::PriyaTidal,
        &[INGEST_BENCH, TIDAL_SDK],
    );
    given_scanned(&world, &mut browser);
    let tidal_before: Vec<String> = pending_on_queue(&mut browser)
        .into_iter()
        .filter(|p| p.starts_with("priya-tidal/"))
        .collect();
    assert_eq!(tidal_before.len(), 2, "precondition");
    removes_did_from_account_bio(&world, Account::PriyaTidal);
    given_scanned(&world, &mut browser);
    puts_did_in_account_bio(&world, Account::PriyaTidal);

    when_adds_account(&mut browser, Account::PriyaTidal);

    let tidal_after: Vec<String> = pending_on_queue(&mut browser)
        .into_iter()
        .filter(|p| p.starts_with("priya-tidal/"))
        .collect();
    assert_eq!(tidal_after, tidal_before, "hidden, not deleted");
}

/// AS-2c (D-RRA-T1: a scan re-checks every listed account, unverified included)
/// ```gherkin
/// @US-RRA-004 @AC-004.4 @D-RRA-T1 @contract-shape:bounded-change
/// Scenario: A scan re-checks an unverified account and reads it once its DID is back
///   Given priya-tidal failed its check in a scan and its suggestions are hidden
///   And Priya put her DID back in the priya-tidal bio, without verifying on the GitHub step
///   When she presses "Scan again"
///   Then her chosen priya-tidal repos are read
///   And the summary says "Ownership re-checked: priyaraman ✓ · priya-tidal ✓"
///   And priya-tidal's suggestions are shown again
/// ```
#[test]
#[ignore = "DELIVER 02-21: a scan re-checks unverified accounts too (D-RRA-T1)"]
fn a_scan_re_checks_an_unverified_account_and_reads_it_once_its_did_is_back() {
    let world = rra_world();
    let mut browser = given_accounts_verified(&world, &BOTH);
    given_saved_choice(
        &mut browser,
        Account::PriyaTidal,
        &[INGEST_BENCH, TIDAL_SDK],
    );
    given_scanned(&world, &mut browser);
    removes_did_from_account_bio(&world, Account::PriyaTidal);
    given_scanned(&world, &mut browser);
    puts_did_in_account_bio(&world, Account::PriyaTidal);

    let (status, read) = when_scans(&world, &mut browser);

    assert_eq!(status, "completed", "{}", browser.page.text());
    assert!(
        set_of(&[INGEST_BENCH, TIDAL_SDK]).is_subset(&read),
        "{read:?}"
    );
    assert!(
        scan_summary(&mut browser).contains("Ownership re-checked: priyaraman ✓ · priya-tidal ✓"),
        "{}",
        browser.page.text()
    );
    assert!(!phrases_about(&mut browser, INGEST_BENCH).is_empty());
}

/// AS-3 (table over how GitHub fails mid-scan)
/// ```gherkin
/// @US-RRA-004 @AC-004.8 @infrastructure-failure @C7a @contract-shape:unbounded-preservation
/// Scenario Outline: GitHub failing during a scan hides nothing
///   Given Priya has pending suggestions from priya-tidal
///   And <failure> when the scan checks the priya-tidal bio
///   When she presses "Scan again"
///   Then no priya-tidal repo is read in this scan
///   And priya-tidal keeps its verified status and its suggestions stay shown
///   And the summary says "priya-tidal: GitHub could not be reached, so its repos were not read. Nothing was lost. Scan again later."
///   Examples:
///     | failure                                         |
///     | GitHub answers 503 for the priya-tidal profile  |
///     | GitHub drops every connection                   |
///     | GitHub says it is too busy to answer    |
/// ```
#[test]
#[ignore = "DELIVER 02-06: an unfinished bio check is not a failed check"]
fn github_failing_during_a_scan_hides_nothing() {
    #[derive(Debug, Clone, Copy)]
    enum Failure {
        ProfileUnavailable,
        ConnectionDropped,
        RateLimited,
    }
    for failure in [
        Failure::ProfileUnavailable,
        Failure::ConnectionDropped,
        Failure::RateLimited,
    ] {
        let world = rra_world();
        let mut browser = given_accounts_verified(&world, &BOTH);
        given_saved_choice(
            &mut browser,
            Account::PriyaTidal,
            &[INGEST_BENCH, TIDAL_SDK],
        );
        given_scanned(&world, &mut browser);
        let accounts_before = account_list(&mut browser);
        let shown_before = phrases_about(&mut browser, INGEST_BENCH);
        match failure {
            Failure::ProfileUnavailable => world
                .github
                .set_profile_unreachable(Account::PriyaTidal.login(), true),
            Failure::ConnectionDropped => world.github.set_offline(true),
            Failure::RateLimited => world.github.rate_limit_for(600),
        }

        let (_, read) = when_scans(&world, &mut browser);

        world.github.set_offline(false);
        world.github.clear_rate_limit();
        assert!(
            read.iter().all(|r| !r.starts_with("priya-tidal/")),
            "{failure:?}: {read:?}"
        );
        assert_eq!(
            account_list(&mut browser),
            accounts_before,
            "{failure:?}: status kept"
        );
        assert_eq!(
            phrases_about(&mut browser, INGEST_BENCH),
            shown_before,
            "{failure:?}: nothing hidden"
        );
        assert!(
            scan_summary(&mut browser).contains("priya-tidal: GitHub could not be reached, so its repos were not read. Nothing was lost. Scan again later."),
            "{failure:?}: {}",
            browser.page.text()
        );
    }
}

/// AS-4
/// ```gherkin
/// @US-RRA-004 @AC-004.6 @BR-RRA-3 @boundary @error @contract-shape:unbounded-preservation
/// Scenario: Two accounts together cannot exceed the limit, each keeping its own choice
///   Given Priya has 8 repos chosen on priyaraman and the per-scan limit is 10
///   When she ticks 3 repos on priya-tidal and presses "Save choice"
///   Then she sees "You can choose up to 10 repos per scan in total. You have 11 chosen. Untick 1 to save."
///   And neither account's saved choice changes, and each is still shown under its own account heading
/// ```
#[test]
fn two_accounts_together_cannot_exceed_the_limit() {
    let world = rra_world();
    let mut browser = given_accounts_verified(&world, &BOTH);
    creates_repo(&world, Repo::new("priyaraman", "extra-one"));
    let eight: Vec<Repo> = PRIYA_OWNED_RANKED
        .iter()
        .copied()
        .chain([Repo::new("priyaraman", "extra-one")])
        .collect();
    given_saved_choice(&mut browser, Account::PriyaTidal, &TIDAL_RANKED[..2]);
    given_saved_choice(&mut browser, Account::Priyaraman, &eight);
    let before = when_opens_choose_repos(&mut browser).clone();

    when_saves_choice(&mut browser, Account::PriyaTidal, &TIDAL_RANKED[..3]);

    assert!(
        browser.page.shows(&save_over_limit_message(11)),
        "{}",
        browser.page.text()
    );
    let after = when_opens_choose_repos(&mut browser).clone();
    for account in BOTH {
        assert_eq!(
            ticked_repos(&after, account),
            ticked_repos(&before, account)
        );
        assert!(after.shows(&account.page_name()));
    }
}

/// AS-5
/// ```gherkin
/// @US-RRA-004 @AC-004.2 @error @C3 @contract-shape:unbounded-preservation
/// Scenario: When every account fails, nothing is read
///   Given neither bio contains Priya's DID and she has no imported repo
///   When she presses "Scan again"
///   Then no repo is read
///   And the summary says, for each of priyaraman and priya-tidal, "Your DID is not in the bio of github.com/<login>, so its repos were not read and its suggestions are hidden. Put your DID back, then verify."
///   And her published claims are unchanged
/// ```
#[test]
#[ignore = "DELIVER 02-08: every account failing reads nothing"]
fn when_every_account_fails_nothing_is_read() {
    let world = rra_world();
    let mut browser = given_accounts_verified(&world, &BOTH);
    given_scanned(&world, &mut browser);
    given_published_one(
        &world,
        &mut browser,
        TIDEPOOL.embodies(Philosophy::TestDriven),
    );
    let published = claim_subjects(&world, Persona::Priya);
    for account in BOTH {
        removes_did_from_account_bio(&world, account);
    }

    let (status, read) = when_scans(&world, &mut browser);

    assert_eq!(status, "ownership_failed", "{}", browser.page.text());
    assert!(read.is_empty(), "{read:?}");
    let summary = scan_summary(&mut browser);
    for account in BOTH {
        assert!(
            summary.contains(&failed_check_summary(account)),
            "{summary}"
        );
    }
    assert_eq!(claim_subjects(&world, Persona::Priya), published);
}

/// AS-6
/// ```gherkin
/// @US-RRA-004 @AC-004.7 @BR-RRA-4 @C7b @interruption @contract-shape:bounded-change
/// Scenario: A paused scan resumes with the same accounts and choices
///   Given a scan of Priya's chosen repos on both accounts pauses for GitHub's budget part-way
///   When the budget refills and the scan resumes
///   Then the suggestions found before the pause are still in her queue
///   And the remaining chosen repos are read, each once, and no other repo
/// ```
#[test]
#[ignore = "DELIVER 02-09: a resume reads only the frozen plan's unread repos"]
fn a_paused_scan_resumes_with_the_same_accounts_and_choices() {
    let world = rra_world();
    let mut browser = given_accounts_verified(&world, &BOTH);
    given_saved_choice(&mut browser, Account::Priyaraman, &PRIYA_CHOICE);
    given_saved_choice(
        &mut browser,
        Account::PriyaTidal,
        &[INGEST_BENCH, TIDAL_SDK],
    );
    let before_pause = given_scan_paused_part_way(&world, &mut browser);
    let found_before = pending_on_queue(&mut browser);

    let resumed = when_paused_scan_resumes(&world, &mut browser);

    let mut plan = set_of(&PRIYA_CHOICE);
    plan.extend(set_of(&[INGEST_BENCH, TIDAL_SDK]));
    assert!(before_pause.is_disjoint(&resumed), "each repo read once");
    assert_eq!(
        before_pause
            .union(&resumed)
            .cloned()
            .collect::<std::collections::BTreeSet<_>>(),
        plan
    );
    let after = pending_on_queue(&mut browser);
    assert!(found_before.iter().all(|p| after.contains(p)), "kept");
}

// =============================================================================
// US-RRA-008 — stop scanning a GitHub account I no longer want read
// =============================================================================

/// AR-1
/// ```gherkin
/// @US-RRA-008 @AC-008.1 @AC-008.2 @D-RRA-17 @happy @kpi @contract-shape:bounded-change
/// Scenario: Priya removes an account she no longer wants scanned
///   Given Priya's list holds priyaraman and priya-tidal, priya-tidal has 2 pending suggestions
///   And she published one claim about a priya-tidal repo
///   And she pressed "Remove" next to priya-tidal and the confirm says "This deletes its 2 pending suggestions. Your published claims stay in your repo."
///   When she confirms
///   Then she sees "github.com/priya-tidal removed. Its 2 pending suggestions were deleted. Your published claims are unchanged."
///   And her list holds only priyaraman, and priya-tidal's 2 pending suggestions are gone from her queue
///   And her published claim is unchanged and nothing is written to her Bluesky repo
///   And the removal is counted
/// ```
#[test]
#[ignore = "DELIVER 02-10: removing an account deletes its pending suggestions only"]
fn priya_removes_an_account_she_no_longer_wants_scanned() {
    let world = rra_world();
    let mut browser = given_accounts_verified(&world, &BOTH);
    given_saved_choice(
        &mut browser,
        Account::PriyaTidal,
        &[INGEST_BENCH, TIDAL_SDK, NOTES_SITE],
    );
    given_scanned(&world, &mut browser);
    given_published_one(
        &world,
        &mut browser,
        NOTES_SITE.embodies(Philosophy::DependencyPinning),
    );
    let before = capture_rra(&world, &mut browser, Persona::Priya);

    when_presses_remove_account(&mut browser, Account::PriyaTidal);
    assert!(
        browser.page.shows(
            "This deletes its 2 pending suggestions. Your published claims stay in your repo."
        ),
        "precondition: {}",
        browser.page.text()
    );

    when_confirms_removal(&mut browser);

    assert!(
        browser.page.shows("github.com/priya-tidal removed. Its 2 pending suggestions were deleted. Your published claims are unchanged."),
        "{}",
        browser.page.text()
    );
    let after = capture_rra(&world, &mut browser, Persona::Priya);
    let without_tidal = |queue: &str| {
        queue
            .lines()
            .filter(|l| !l.starts_with("priya-tidal/"))
            .collect::<Vec<_>>()
            .join("\n")
    };
    assert_state_delta(
        &before,
        &after,
        &rra_universe(),
        &Delta::new()
            .with_slot(
                "queue.pending",
                review_app::state_delta::set_to(without_tidal(&before["queue.pending"])),
            )
            .with_slot(
                "accounts",
                review_app::state_delta::set_to("priyaraman:true".to_string()),
            ),
    );
    assert_eq!(kpi(&world, "account.removed"), 1);
}

/// AR-2
/// ```gherkin
/// @US-RRA-008 @AC-008.1 @contract-shape:unbounded-preservation
/// Scenario: Cancelling the removal changes nothing
///   Given Priya pressed "Remove" next to priya-tidal
///   When she cancels the confirm
///   Then priya-tidal is still on her list, verified, and its suggestions are shown
/// ```
#[test]
#[ignore = "DELIVER 02-11: cancelling a removal changes nothing"]
fn cancelling_the_removal_changes_nothing() {
    let world = rra_world();
    let mut browser = given_accounts_verified(&world, &BOTH);
    given_saved_choice(
        &mut browser,
        Account::PriyaTidal,
        &[INGEST_BENCH, TIDAL_SDK],
    );
    given_scanned(&world, &mut browser);
    let before = capture_rra(&world, &mut browser, Persona::Priya);

    when_presses_remove_account(&mut browser, Account::PriyaTidal);
    when_cancels_removal(&mut browser);

    let after = capture_rra(&world, &mut browser, Persona::Priya);
    assert_state_delta(&before, &after, &rra_universe(), &Delta::new());
}

/// AR-3
/// ```gherkin
/// @US-RRA-008 @AC-008.3 @BR-1 @BR-2 @C4a @contract-shape:bounded-change
/// Scenario: Re-adding offers suggestions again, but never declined or published ones
///   Given Priya removed priya-tidal while it had 2 pending suggestions, 1 declined one and 1 published claim
///   And she added and verified priya-tidal again
///   When she presses "Scan again"
///   Then the 2 earlier suggestions are offered again as new suggestions
///   And the declined one is not offered
///   And the published one is counted as "already published"
/// ```
#[test]
#[ignore = "DELIVER 02-12: removal keeps declined and published keys (BR-1, BR-2)"]
fn re_adding_offers_suggestions_again_but_never_declined_or_published_ones() {
    let world = rra_world();
    let mut browser = given_accounts_verified(&world, &BOTH);
    given_saved_choice(&mut browser, Account::PriyaTidal, &TIDAL_RANKED);
    given_scanned(&world, &mut browser);
    let declined = REEF_UI.embodies(Philosophy::SemanticVersioning);
    let published = NOTES_SITE.embodies(Philosophy::DependencyPinning);
    when_declines_card(&mut browser, declined);
    given_published_one(&world, &mut browser, published);
    let pending_tidal: Vec<String> = pending_on_queue(&mut browser)
        .into_iter()
        .filter(|p| p.starts_with("priya-tidal/"))
        .collect();
    assert_eq!(pending_tidal.len(), 2, "precondition");
    when_removes_account(&mut browser, Account::PriyaTidal);

    when_adds_account(&mut browser, Account::PriyaTidal);

    let (status, _) = when_scans(&world, &mut browser);

    assert_eq!(status, "completed");
    let again: Vec<String> = pending_on_queue(&mut browser)
        .into_iter()
        .filter(|p| p.starts_with("priya-tidal/"))
        .collect();
    assert_eq!(
        again, pending_tidal,
        "offered again; the declined and published ones are not"
    );
    assert!(
        scan_summary(&mut browser).contains("1 already published"),
        "{}",
        browser.page.text()
    );
}

/// AR-4a (OQ-D4: deleted at once)
/// ```gherkin
/// @US-RRA-008 @AC-008.4 @BR-RRA-4 @OQ-D4 @UC-RRA-8 @C7b @interruption @contract-shape:bounded-change
/// Scenario: Removing an account during a scan deletes its pending suggestions at once
///   Given a scan of Priya's priya-tidal repos is running, paused after reading 1 of 2
///   And that scan already found pending suggestions for priya-tidal
///   When she removes priya-tidal and confirms
///   Then her queue holds no priya-tidal pending suggestion, while the scan is still paused
/// ```
#[test]
#[ignore = "DELIVER 02-13: removal during a scan deletes the source's pending rows at once (OQ-D4)"]
fn removing_an_account_during_a_scan_deletes_its_pending_suggestions_at_once() {
    let world = rra_world();
    let mut browser = given_tidal_scan_paused_after_one_repo(&world);
    assert!(
        pending_on_queue(&mut browser)
            .iter()
            .any(|p| p.starts_with("priya-tidal/")),
        "precondition: the paused scan found priya-tidal suggestions"
    );

    when_removes_account(&mut browser, Account::PriyaTidal);

    assert!(
        pending_on_queue(&mut browser)
            .iter()
            .all(|p| !p.starts_with("priya-tidal/")),
        "deleted at once, not after the scan"
    );
    assert_eq!(
        scan_status(&mut browser),
        "rate_limited",
        "the scan is still paused"
    );
}

/// AR-4b (chained on AR-4a)
/// ```gherkin
/// @US-RRA-008 @AC-008.4 @BR-RRA-4 @OQ-D4 @UC-RRA-8 @C7b @interruption @contract-shape:bounded-change
/// Scenario: A resumed scan neither reads nor adds anything for a removed account
///   Given Priya removed priya-tidal while a scan that had read 1 of its 2 repos was paused
///   When GitHub's budget refills and the scan resumes
///   Then the second priya-tidal repo is not read
///   And when the scan has ended, her queue holds no priya-tidal suggestion
/// ```
#[test]
#[ignore = "DELIVER 02-22: a resumed scan refuses inserts for a removed source (UC-RRA-8)"]
fn a_resumed_scan_neither_reads_nor_adds_anything_for_a_removed_account() {
    let world = rra_world();
    let mut browser = given_tidal_scan_paused_after_one_repo(&world);
    when_removes_account(&mut browser, Account::PriyaTidal);
    let mark = github_mark(&world);

    world.github.set_remaining(4_990);
    when_scan_finishes(&mut browser);

    assert!(
        repos_read_since(&world, mark)
            .iter()
            .all(|r| !r.starts_with("priya-tidal/")),
        "no further read of the removed account"
    );
    assert!(
        pending_on_queue(&mut browser)
            .iter()
            .all(|p| !p.starts_with("priya-tidal/")),
        "no scan-time insert for the removed account"
    );
}

/// GIVEN Priya's scan of her 2 chosen priya-tidal repos (nothing chosen on
/// priyaraman) paused for GitHub's budget after reading 1 of them.
fn given_tidal_scan_paused_after_one_repo(world: &ReviewWorld) -> Browser {
    let mut browser = given_accounts_verified(world, &BOTH);
    given_saved_choice(&mut browser, Account::Priyaraman, &[]);
    given_saved_choice(
        &mut browser,
        Account::PriyaTidal,
        &[INGEST_BENCH, TIDAL_SDK],
    );
    let before_pause = given_scan_paused_part_way(world, &mut browser);
    assert_eq!(before_pause.len(), 1, "precondition: 1 of 2 read");
    browser
}

// =============================================================================
// US-RRA-010 — publishing follows who owns the repo right now (BR-RRA-2)
// =============================================================================

/// AP-1
/// ```gherkin
/// @US-RRA-010 @AC-010.1 @BR-RRA-2 @error @KPI-RRA-4 @kpi @contract-shape:unbounded-preservation
/// Scenario: A repo transferred away after the scan can no longer be published
///   Given Priya has a pending suggestion for priyaraman/kelp
///   And kelp was transferred to tidal-labs after her scan
///   When she opens its preview
///   Then she sees "github.com/priyaraman/kelp is no longer owned by an account you verified, so nothing was published. If you own it under another account, verify that account; otherwise decline this suggestion."
///   And no preview is offered
///   And nothing is written to her Bluesky repo, the suggestion stays pending, and the refusal is counted
/// ```
#[test]
#[ignore = "DELIVER 02-14: preview re-reads the repo owner (ADR-089 permit)"]
fn a_repo_transferred_away_after_the_scan_can_no_longer_be_published() {
    let world = rra_world();
    let mut browser = given_owner_verified(&world, Account::Priyaraman);
    given_scanned(&world, &mut browser);
    let kelp = KELP.embodies(Philosophy::DependencyPinning);
    world
        .github
        .transfer_repo(KELP.owner, KELP.name, KELP_AT_TIDAL_LABS.owner);
    let facts_reads = world.github.repo_facts_reads_of(&KELP.full());
    let before = capture_rra(&world, &mut browser, Persona::Priya);

    when_previews_approval(&mut browser, kelp);

    assert!(
        browser.page.shows("github.com/priyaraman/kelp is no longer owned by an account you verified, so nothing was published. If you own it under another account, verify that account; otherwise decline this suggestion."),
        "{}",
        browser.page.text()
    );
    assert!(!browser.page.offers("Publish to my repo"), "no preview");
    let after = capture_rra(&world, &mut browser, Persona::Priya);
    assert_state_delta(&before, &after, &rra_universe(), &Delta::new());
    assert_eq!(kpi(&world, "publish.refused.not_owned"), 1);
    // Secondary (diagnostic, not the contract): the refusal came from a read
    // made in that request.
    assert!(
        world.github.repo_facts_reads_of(&KELP.full()) > facts_reads,
        "secondary: a fresh read of kelp's owner"
    );
}

/// AP-2 (table over how GitHub fails at preview)
/// ```gherkin
/// @US-RRA-010 @AC-010.3 @infrastructure-failure @C7a @contract-shape:unbounded-preservation
/// Scenario Outline: GitHub failing at preview shows no preview and publishes nothing
///   Given Priya has a pending suggestion for priyaraman/tidepool
///   And <failure>
///   When she opens its preview
///   Then she sees "GitHub could not be reached, so we couldn't check who owns github.com/priyaraman/tidepool. Nothing was published. Please try again in a few minutes."
///   And nothing is written to her Bluesky repo, the suggestion stays pending and priyaraman keeps its verified status
///   Examples:
///     | failure                                      |
///     | GitHub drops every connection                |
///     | GitHub says it is too busy to answer |
/// ```
#[test]
#[ignore = "DELIVER 02-15: an unfinished check at preview changes nothing"]
fn github_failing_at_preview_shows_no_preview_and_publishes_nothing() {
    for rate_limited in [false, true] {
        let world = rra_world();
        let mut browser = given_owner_verified(&world, Account::Priyaraman);
        given_scanned(&world, &mut browser);
        let before = capture_rra(&world, &mut browser, Persona::Priya);
        if rate_limited {
            world.github.rate_limit_for(600);
        } else {
            world.github.set_offline(true);
        }

        when_previews_approval(&mut browser, TIDEPOOL.embodies(Philosophy::TestDriven));

        world.github.set_offline(false);
        world.github.clear_rate_limit();
        assert!(
            browser.page.shows("GitHub could not be reached, so we couldn't check who owns github.com/priyaraman/tidepool. Nothing was published. Please try again in a few minutes."),
            "rate limited {rate_limited}: {}",
            browser.page.text()
        );
        assert!(!browser.page.offers("Publish to my repo"), "no preview");
        let after = capture_rra(&world, &mut browser, Persona::Priya);
        assert_state_delta(&before, &after, &rra_universe(), &Delta::new());
    }
}

/// AP-3
/// ```gherkin
/// @US-RRA-010 @AC-010.2 @BR-RRA-2 @error @contract-shape:bounded-change
/// Scenario: A bio edited between preview and confirm stops the publish
///   Given Priya previewed a claim about priya-tidal/ingest-bench
///   And she removed her DID from the bio of github.com/priya-tidal before confirming
///   When she presses "Publish to my repo"
///   Then nothing is written to her Bluesky repo
///   And she sees "Your DID is no longer in the bio of github.com/priya-tidal, so nothing was published. Put your DID back, verify the account, then publish again."
///   And priya-tidal is marked as needing re-verify
/// ```
#[test]
#[ignore = "DELIVER 02-16: confirm re-reads the bio and marks the account unverified"]
fn a_bio_edited_between_preview_and_confirm_stops_the_publish() {
    let world = rra_world();
    let mut browser = given_accounts_verified(&world, &BOTH);
    given_saved_choice(&mut browser, Account::PriyaTidal, &[INGEST_BENCH]);
    given_scanned(&world, &mut browser);
    when_previews_approval(&mut browser, INGEST_BENCH.embodies(Philosophy::TestDriven));
    removes_did_from_account_bio(&world, Account::PriyaTidal);

    when_confirms_publish(&mut browser);

    assert!(world.atproto.write_attempts().is_empty(), "nothing written");
    assert!(
        browser.page.shows("Your DID is no longer in the bio of github.com/priya-tidal, so nothing was published. Put your DID back, verify the account, then publish again."),
        "{}",
        browser.page.text()
    );
    assert_eq!(
        account_list(&mut browser),
        vec![
            ("priyaraman".to_string(), true),
            ("priya-tidal".to_string(), false)
        ]
    );
}

/// AP-4 (table: ownership as it is at confirm decides)
/// ```gherkin
/// @US-RRA-010 @AC-010.1 @BR-RRA-2 @NFR-RRA-4 @KPI-RRA-4 @kpi @contract-shape:bounded-change
/// Scenario Outline: A claim about her own repo is published only if she still owns it when she confirms
///   Given Priya is looking at the preview of "priyaraman/tidepool embodies test-driven"
///   And <change>
///   When she presses "Publish to my repo"
///   Then <outcome>
///   Examples:
///     | change                                         | outcome                                                                                                       |
///     | nothing changed on GitHub                      | her Bluesky repo holds that one claim, and each of the two requests logged a passed ownership check          |
///     | she transferred tidepool to tidal-labs meanwhile | she sees "github.com/priyaraman/tidepool is no longer owned by an account you verified, so nothing was published. If you own it under another account, verify that account; otherwise decline this suggestion." and nothing is written |
/// ```
#[test]
#[ignore = "DELIVER 02-17: confirm mints its own permit from a fresh owner and bio read"]
fn a_claim_about_her_own_repo_is_published_only_if_she_still_owns_it_when_she_confirms() {
    for transferred in [false, true] {
        let world = rra_world();
        let mut browser = given_owner_verified(&world, Account::Priyaraman);
        given_scanned(&world, &mut browser);
        let target = TIDEPOOL.embodies(Philosophy::TestDriven);
        when_previews_approval(&mut browser, target);
        if transferred {
            world
                .github
                .transfer_repo(TIDEPOOL.owner, TIDEPOOL.name, "tidal-labs");
        }
        let mark = github_mark(&world);

        when_confirms_publish(&mut browser);

        if transferred {
            assert!(
                browser.page.shows("github.com/priyaraman/tidepool is no longer owned by an account you verified, so nothing was published. If you own it under another account, verify that account; otherwise decline this suggestion."),
                "{}",
                browser.page.text()
            );
            assert!(world.atproto.write_attempts().is_empty(), "nothing written");
        } else {
            assert_eq!(claim_subjects(&world, Persona::Priya), [TIDEPOOL.subject()]);
            assert_eq!(
                world
                    .app
                    .logs()
                    .matches("ownership.publish_check.passed")
                    .count(),
                2,
                "KPI-RRA-4 audit: one passed check per request (preview, confirm)"
            );
            // Secondary (NFR-RRA-4 budget, diagnostic): the confirm read
            // GitHub at most twice — the repo's owner and that owner's bio.
            let confirm_reads = world
                .github
                .requests()
                .iter()
                .filter(|r| r.seq > mark)
                .count();
            assert!(
                (1..=2).contains(&confirm_reads),
                "secondary: {confirm_reads}"
            );
        }
    }
}

/// AS-7 (OQ-D3: an unverified account's saved ticks still count)
/// ```gherkin
/// @US-RRA-004 @AC-004.6 @BR-RRA-3 @OQ-D3 @boundary @error @contract-shape:unbounded-preservation
/// Scenario: An unverified account's saved repos still count toward the limit
///   Given Priya saved 5 repos on priyaraman and 5 on priya-tidal
///   And priya-tidal failed its check in a scan, so it needs re-verifying
///   When she ticks a 6th repo on priyaraman and presses "Save choice"
///   Then she sees "You can choose up to 10 repos per scan in total. You have 11 chosen. Untick 1 to save."
///   And neither account's saved choice changes: priya-tidal still has its 5 when she verifies it again
/// ```
#[test]
fn an_unverified_accounts_saved_repos_still_count_toward_the_limit() {
    let world = rra_world();
    let mut browser = given_accounts_verified(&world, &BOTH);
    creates_repo(&world, Repo::new("priya-tidal", "fifth-one"));
    let tidal_five: Vec<Repo> = TIDAL_RANKED
        .iter()
        .copied()
        .chain([Repo::new("priya-tidal", "fifth-one")])
        .collect();
    given_saved_choice(&mut browser, Account::Priyaraman, &PRIYA_TOP_5);
    given_saved_choice(&mut browser, Account::PriyaTidal, &tidal_five);
    removes_did_from_account_bio(&world, Account::PriyaTidal);
    given_scanned(&world, &mut browser);
    let six: Vec<Repo> = PRIYA_OWNED_RANKED[..6].to_vec();

    when_saves_choice(&mut browser, Account::Priyaraman, &six);

    assert!(
        browser.page.shows(&save_over_limit_message(11)),
        "{}",
        browser.page.text()
    );
    let page = when_opens_choose_repos(&mut browser).clone();
    assert_eq!(
        ticked_repos(&page, Account::Priyaraman),
        fulls(&PRIYA_TOP_5)
    );
    puts_did_in_account_bio(&world, Account::PriyaTidal);
    when_adds_account(&mut browser, Account::PriyaTidal);
    let page = when_opens_choose_repos(&mut browser).clone();
    assert_eq!(
        ticked_repos(&page, Account::PriyaTidal),
        fulls(&tidal_five),
        "the unverified account's saved choice is unchanged"
    );
}

// =============================================================================
// Privacy across owners (I-BRA-1, C-RRA-4)
// =============================================================================

/// AV-1
/// ```gherkin
/// @I-BRA-1 @C-RRA-4 @privacy @adversarial @C6a @contract-shape:unbounded-preservation
/// Scenario Outline: One owner never sees or changes another owner's accounts or choices
///   Given Priya has priyaraman and priya-tidal with a saved choice
///   When Sam, signed in as himself, sends the app Priya's own <request> request
///   Then Sam's GitHub step and repo list name none of Priya's accounts or repos
///   And Sam's own list and choice did not take her account or repos
///   And Priya's accounts, choices, queue and published claims are exactly as before
///   Examples:
///     | request                                   |
///     | "Remove" for priya-tidal                  |
///     | "Save choice" naming her ingest-bench     |
/// ```
#[test]
#[ignore = "DELIVER 02-18: accounts and choices are owner-scoped"]
fn one_owner_never_sees_or_changes_another_owners_accounts_or_choices() {
    let world = rra_world();
    let mut priya = given_accounts_verified(&world, &BOTH);
    given_saved_choice(&mut priya, Account::PriyaTidal, &[INGEST_BENCH]);
    given_scanned(&world, &mut priya);
    priya.open("/github");
    let remove_form = priya
        .forms()
        .into_iter()
        .find(|f| f.buttons.iter().any(|(l, _, _)| l.contains(REMOVE)))
        .expect("Priya's remove form");
    when_opens_choose_repos(&mut priya);
    let save_form = priya
        .forms()
        .into_iter()
        .find(|f| f.buttons.iter().any(|(l, _, _)| l.contains(SAVE_CHOICE)))
        .expect("Priya's save form");
    let before = capture_rra(&world, &mut priya, Persona::Priya);

    let mut sam = given_signed_in(&world, Persona::Sam);
    for (request, form) in [("Remove", &remove_form), ("Save choice", &save_form)] {
        sam.open("/github");
        let sams_csrf = csrf_token(&sam);

        sam.submit(&Browser::with_field(form, "csrf", &sams_csrf), None);

        let github_page = sam.open("/github").text();
        let repos_page = sam.open("/repos").text();
        for leak in ["priya-tidal", "priyaraman", INGEST_BENCH.name] {
            assert!(
                !github_page.contains(leak) && !repos_page.contains(leak),
                "{request}: {leak}"
            );
        }
        let after = capture_rra(&world, &mut priya, Persona::Priya);
        assert_state_delta(&before, &after, &rra_universe(), &Delta::new());
    }
}
