//! review-app-repos-and-accounts — Release 1: choose which owned repos a scan
//! reads (US-RRA-001, US-RRA-002) and the store v1 → v2 upgrade (ADR-088).
//!
//! Driving port: the REAL `openlore-review-app` over HTTP (session + CSRF,
//! existing harness). Driven internal: the REAL `review-app.duckdb` (never
//! read by a scenario). Fakes: `FakeGithubAccounts` (repo ids, transfers,
//! archived repos, rate limit) and the ATProto network. Layer 4 (real HTTP):
//! example-only; same-shape scenarios are tables, sad paths named examples
//! (Mandate 9/11). Pure rules (default ticks, limit arithmetic, scan plan)
//! carry their properties in `review_app_repos_and_accounts_core.rs`.
//!
//! Every scenario is `#[ignore = "DELIVER 01-NN: …"]`; DELIVER enables one at
//! a time. The fixture regenerator is env-gated and never a scenario.
//!
//! State machine (C2a) of an account's repo choice:
//! `default (no saved rows) --Save--> saved --Save--> saved`;
//! `default --BR-RRA-5 move--> saved`; `saved --remove account--> (gone)`.
//! A scan freezes the choice in force at its start (BR-RRA-4); later Saves
//! apply from the next scan.

#[path = "support/review_app/mod.rs"]
mod review_app;

use review_app::repos_and_accounts::repos::*;
use review_app::repos_and_accounts::*;
use review_app::state_delta::{assert_state_delta, Delta};
use review_app::*;

// =============================================================================
// Walking skeleton
// =============================================================================

/// WS-RRA-1
/// ```gherkin
/// @walking_skeleton @driving_port @real-io @US-RRA-001 @US-RRA-002 @KPI-RRA-1 @kpi @contract-shape:bounded-change
/// Scenario: Priya publishes a claim about a repo her old top 5 never read
///   Given Priya is verified as github.com/priyaraman with 7 owned public repos
///   And she saved a choice that unticks driftwood and ticks ferrous-notes
///   And her scan read exactly her 5 chosen repos
///   And she is looking at the preview of "priyaraman/ferrous-notes embodies dependency-pinning", with nothing published yet
///   When she presses "Publish to my repo"
///   Then her Bluesky repo holds that one claim
///   And her action on a suggestion today's top 5 would never have produced is counted
/// ```
///
/// Environments (Sentinel F7): this skeleton runs on the clean environment.
/// The stale-config environments are covered by focused scenarios instead of
/// skeleton variants — v1 store: MG-1..3; unverified account: RC-7, AS-2,
/// AP-3; GitHub unreachable / rate-limited: RC-5, AS-3, AP-2, IC-5 (waiver
/// recorded in `distill/wave-decisions.md`).
#[test]
fn priya_publishes_a_claim_about_a_repo_her_old_top_5_never_read() {
    let world = rra_world();
    let mut browser = given_owner_verified(&world, Account::Priyaraman);

    given_saved_choice(&mut browser, Account::Priyaraman, &PRIYA_CHOICE);
    let read = given_scanned(&world, &mut browser);
    assert_eq!(
        read,
        set_of(&PRIYA_CHOICE),
        "the scan read exactly the choice"
    );
    let target = FERROUS_NOTES.embodies(Philosophy::DependencyPinning);
    when_previews_approval(&mut browser, target);
    assert!(
        world.atproto.write_attempts().is_empty(),
        "nothing is written before she confirms"
    );

    when_confirms_publish(&mut browser);

    assert_eq!(
        claim_subjects(&world, Persona::Priya),
        [FERROUS_NOTES.subject()]
    );
    assert!(
        kpi(&world, "suggestion.acted.outside_default") == 1,
        "KPI-RRA-1: acting on an outside-default suggestion is counted"
    );
}

// =============================================================================
// US-RRA-001 — choose which owned repos a scan reads
// =============================================================================

/// RC-1
/// ```gherkin
/// @US-RRA-001 @AC-001.1 @AC-001.4 @happy @contract-shape:unbounded-preservation
/// Scenario: First visit shows today's top 5 already ticked
///   Given Priya is verified as github.com/priyaraman with 7 owned public repos, 1 fork and 1 archived repo
///   And she has never saved a repo choice
///   When she opens "Choose repos"
///   Then she sees the 7 repos ranked by stars
///   And tidepool, quill-docs, kelp, harbor-cli and driftwood are ticked
///   And she sees "Skipped: 1 fork, 1 archived repo."
/// ```
#[test]
fn first_visit_shows_todays_top_5_already_ticked() {
    let world = rra_world();
    let mut browser = given_owner_verified(&world, Account::Priyaraman);

    let page = when_opens_choose_repos(&mut browser).clone();

    assert_eq!(
        listed_repos(&page, Account::Priyaraman),
        fulls(&PRIYA_OWNED_RANKED)
    );
    assert_eq!(
        ticked_repos(&page, Account::Priyaraman),
        fulls(&PRIYA_TOP_5)
    );
    assert!(
        page.shows("Skipped: 1 fork, 1 archived repo."),
        "{}",
        page.text()
    );
    assert!(!page.shows(&SERDE_FORK.full()) && !page.shows(&OLD_TIDES_ARCHIVED.full()));
}

/// RC-2 (chained on RC-1)
/// ```gherkin
/// @US-RRA-001 @AC-001.2 @happy @kpi @KPI-RRA-1 @contract-shape:bounded-change
/// Scenario: Priya saves her own choice
///   Given Priya sees her repo list with the top 5 ticked
///   When she unticks driftwood, ticks ferrous-notes and presses "Save choice"
///   Then she sees "Saved. Your next scan reads 5 repos."
///   And her saved choice has ferrous-notes ticked and driftwood unticked
///   And the save is counted once as a change from today's default
/// ```
#[test]
#[ignore = "DELIVER 01-03: Save choice stores exactly the ticked repos"]
fn priya_saves_her_own_choice() {
    let world = rra_world();
    let mut browser = given_owner_verified(&world, Account::Priyaraman);
    when_opens_choose_repos(&mut browser);

    when_saves_choice(&mut browser, Account::Priyaraman, &PRIYA_CHOICE);

    assert!(
        browser.page.shows("Saved. Your next scan reads 5 repos."),
        "{}",
        browser.page.text()
    );
    let reopened = when_opens_choose_repos(&mut browser).clone();
    assert_eq!(
        ticked_repos(&reopened, Account::Priyaraman),
        fulls(&PRIYA_CHOICE)
    );
    assert_eq!(kpi(&world, "repos.choice.saved"), 1);
    assert_eq!(kpi(&world, "repos.choice.changed_from_default"), 1);
}

/// RC-3 (boundary table: max + 1, max + 2; max itself is the Given)
/// ```gherkin
/// @US-RRA-001 @AC-001.3 @BR-RRA-3 @boundary @error @C1b @kpi @contract-shape:unbounded-preservation
/// Scenario Outline: Choosing more than the limit is refused with the fix
///   Given Dmitri is verified as github.com/dvolkov with 12 owned public repos
///   And he saved 10 of them and saw "Saved. Your next scan reads 10 repos."
///   When he ticks <n> repos and presses "Save choice"
///   Then he sees "<message>"
///   And his saved choice is still the same 10 repos
///   And the refusal is counted
///   Examples:
///     | n  | message                                                                              |
///     | 11 | You can choose up to 10 repos per scan in total. You have 11 chosen. Untick 1 to save. |
///     | 12 | You can choose up to 10 repos per scan in total. You have 12 chosen. Untick 2 to save. |
/// ```
#[test]
#[ignore = "DELIVER 01-04: the 10-repo limit at Save (BR-RRA-3)"]
fn choosing_more_than_the_limit_is_refused_with_the_fix() {
    let all = dmitri_repos();
    for n in [SCAN_REPO_CAP + 1, SCAN_REPO_CAP + 2] {
        let world = rra_world();
        let mut browser = given_owner_verified(&world, Account::Dvolkov);
        when_saves_choice(&mut browser, Account::Dvolkov, &all[..SCAN_REPO_CAP]);
        assert!(
            browser.page.shows("Saved. Your next scan reads 10 repos."),
            "{}",
            browser.page.text()
        );

        when_saves_choice(&mut browser, Account::Dvolkov, &all[..n]);

        assert!(
            browser.page.shows(&save_over_limit_message(n)),
            "{n} ticked:\n{}",
            browser.page.text()
        );
        let after = ticked_repos(
            &when_opens_choose_repos(&mut browser).clone(),
            Account::Dvolkov,
        );
        assert_eq!(after, fulls(&all[..SCAN_REPO_CAP]), "{n}: unchanged");
        assert_eq!(kpi(&world, "repos.choice.refused.over_limit"), 1);
    }
}

/// RC-4
/// ```gherkin
/// @US-RRA-001 @AC-001.4 @edge @C1a @contract-shape:unbounded-preservation
/// Scenario: An owner with only forks learns why nothing is listed
///   Given Aisha is verified as github.com/aishab, which owns 3 forks and no other repo
///   When she opens "Choose repos"
///   Then she sees "Forks and archived repos are skipped. Skipped: 3 forks."
///   And she is offered "Add repo" to name a public repo she doesn't own
/// ```
#[test]
fn an_owner_with_only_forks_learns_why_nothing_is_listed() {
    let world = rra_world();
    let mut browser = given_owner_verified(&world, Account::Aishab);

    let page = when_opens_choose_repos(&mut browser).clone();

    assert!(listed_repos(&page, Account::Aishab).is_empty());
    let text = page.text();
    assert!(
        text.contains("Forks and archived repos are skipped. Skipped: 3 forks."),
        "{text}"
    );
    assert!(
        page.offers(ADD_REPO),
        "naming a public repo is offered:\n{text}"
    );
}

/// RC-5
/// ```gherkin
/// @US-RRA-001 @AC-001.5 @error @infrastructure-failure @C7a @contract-shape:unbounded-preservation
/// Scenario: A failed listing leaves the saved choice alone
///   Given Priya saved a choice of 5 repos
///   And GitHub is rate-limiting the app
///   When she opens "Choose repos"
///   Then she sees "GitHub is rate-limiting the app, so your repo list couldn't be loaded. Your saved choice is unchanged. Please try again in a few minutes."
///   And her next scan still reads her 5 saved repos
/// ```
#[test]
fn a_failed_listing_leaves_the_saved_choice_alone() {
    let world = rra_world();
    let mut browser = given_owner_verified(&world, Account::Priyaraman);
    given_saved_choice(&mut browser, Account::Priyaraman, &PRIYA_CHOICE);
    world.github.rate_limit_for(600);

    let text = when_opens_choose_repos(&mut browser).text();

    assert!(text.contains(LISTING_RATE_LIMITED), "{text}");
    world.github.clear_rate_limit();
    assert_eq!(given_scanned(&world, &mut browser), set_of(&PRIYA_CHOICE));
}

/// RC-6
///
/// The JS-less harness cannot send key presses. The When is the request a
/// keyboard activation of "Save choice" submits (space on the box, Enter on
/// the button); the Thens assert what makes that operable by keyboard: every
/// box has an accessible name, nothing reorders focus, the button follows the
/// boxes, and the save lands.
/// ```gherkin
/// @US-RRA-001 @AC-001.6 @NFR-RRA-5 @accessibility @contract-shape:bounded-change
/// Scenario: Priya chooses and saves with the keyboard only
///   Given Priya has "Choose repos" open and uses only the keyboard
///   When she submits "Save choice" with ferrous-notes ticked and driftwood unticked, as her space-bar toggles and Enter would
///   Then each box is announced with its repo name, for example "priyaraman/ferrous-notes"
///   And focus moves through the boxes in list order and reaches "Save choice" after them
///   And she sees "Saved. Your next scan reads 5 repos."
/// ```
#[test]
fn priya_chooses_and_saves_with_the_keyboard_only() {
    let world = rra_world();
    let mut browser = given_owner_verified(&world, Account::Priyaraman);
    let page = when_opens_choose_repos(&mut browser).clone();

    when_saves_choice(&mut browser, Account::Priyaraman, &PRIYA_CHOICE);

    assert_eq!(
        listed_repos(&page, Account::Priyaraman),
        fulls(&PRIYA_OWNED_RANKED),
        "every box is announced by its repo name (aria-label)"
    );
    for (_, tag) in html::opening_tags(&page.html, "input") {
        if let Some(tabindex) = html::attr(tag, "tabindex") {
            assert!(
                tabindex.parse::<i32>().unwrap_or(0) <= 0,
                "no positive tabindex reorders focus: {tag}"
            );
        }
    }
    let save_at = page.html.find(SAVE_CHOICE).expect("a Save choice button");
    let last_box = page.html.rfind("type=\"checkbox\"").expect("repo boxes");
    assert!(
        last_box < save_at,
        "the button follows the boxes in focus order"
    );
    assert!(
        browser.page.shows("Saved. Your next scan reads 5 repos."),
        "{}",
        browser.page.text()
    );
}

/// RC-7
/// ```gherkin
/// @US-RRA-001 @AC-001.7 @error @contract-shape:unbounded-preservation
/// Scenario: An account that needs re-verifying shows no repo list
///   Given Priya's DID is no longer in the bio of github.com/priyaraman, and a scan noticed
///   When she opens "Choose repos"
///   Then she sees no repo list for priyaraman
///   And she sees "Your DID is no longer in the bio of github.com/priyaraman. Put it back, then verify on the GitHub step." with a link to the GitHub step
/// ```
#[test]
fn an_account_that_needs_re_verifying_shows_no_repo_list() {
    let world = rra_world();
    let mut browser = given_owner_verified(&world, Account::Priyaraman);
    removes_did_from_account_bio(&world, Account::Priyaraman);
    when_scan_finishes(&mut browser);

    let page = when_opens_choose_repos(&mut browser).clone();

    assert!(listed_repos(&page, Account::Priyaraman).is_empty());
    assert!(
        page.shows("Your DID is no longer in the bio of github.com/priyaraman. Put it back, then verify on the GitHub step."),
        "{}",
        page.text()
    );
    assert!(
        html::links(&page.html)
            .iter()
            .any(|(_, href)| href.starts_with("/github")),
        "a link to the GitHub step"
    );
}

// =============================================================================
// US-RRA-002 — a scan reads exactly the repos I chose, and remembers them
// =============================================================================

/// RS-1 (table: saved choice, never chose, the full 10)
/// ```gherkin
/// @US-RRA-002 @AC-002.1 @BR-RRA-3 @KPI-RRA-7 @happy @boundary @contract-shape:unbounded-preservation
/// Scenario Outline: The scan reads only the chosen repos, or today's top 5 for an owner who never chose
///   Given <owner> <choice>
///   When <owner> presses "Scan again"
///   Then exactly <read> are read and no other repo
///   And the summary says "<n> repos read"
///   Examples:
///     | owner  | choice                                                   | read                                                                      | n  |
///     | Priya  | saved tidepool, quill-docs, kelp, harbor-cli, ferrous-notes | priyaraman/tidepool, quill-docs, kelp, harbor-cli, ferrous-notes        | 5  |
///     | Priya  | never opened "Choose repos"                              | priyaraman/tidepool, quill-docs, kelp, harbor-cli, driftwood (by stars)   | 5  |
///     | Dmitri | saved forge-01 .. forge-10 of his 12                     | dvolkov/forge-01 .. forge-10                                              | 10 |
/// ```
#[test]
#[ignore = "DELIVER 01-09: a scan reads exactly the saved choice (or the default top 5)"]
fn the_scan_reads_only_the_repos_priya_chose_or_todays_top_5() {
    let dmitri_ten: Vec<Repo> = dmitri_repos()[..SCAN_REPO_CAP].to_vec();
    let rows: [(Account, Option<Vec<Repo>>, Vec<Repo>); 3] = [
        (
            Account::Priyaraman,
            Some(PRIYA_CHOICE.to_vec()),
            PRIYA_CHOICE.to_vec(),
        ),
        (Account::Priyaraman, None, PRIYA_TOP_5.to_vec()),
        (Account::Dvolkov, Some(dmitri_ten.clone()), dmitri_ten),
    ];
    for (account, saved, expected) in rows {
        let world = rra_world();
        let mut browser = given_owner_verified(&world, account);
        if let Some(choice) = &saved {
            given_saved_choice(&mut browser, account, choice);
        }

        let (status, read) = when_scans(&world, &mut browser);

        assert_eq!(status, "completed", "{}", browser.page.text());
        assert_eq!(read, set_of(&expected), "{account:?} saved = {saved:?}");
        let summary = format!("{} repos read", expected.len());
        assert!(browser.page.shows(&summary), "{}", browser.page.text());
        assert!(
            world
                .github
                .scrapes_without_preceding_bio_check(account.login())
                .is_empty(),
            "the bio is checked before any repo is read"
        );
    }
}

/// RS-2a
/// ```gherkin
/// @US-RRA-002 @AC-002.2 @BR-RRA-4 @C7b @interruption @contract-shape:bounded-change
/// Scenario: A paused scan finishes the choice it started with
///   Given Priya's saved choice is tidepool, quill-docs, kelp, harbor-cli and ferrous-notes
///   And a scan with that choice is running, paused for GitHub's budget
///   And she saved a choice without quill-docs while it ran
///   When GitHub's budget refills and the scan resumes on its own
///   Then the scan, before and after the pause, read the 5 repos it started with, each once
/// ```
#[test]
#[ignore = "DELIVER 01-10: the frozen scan plan survives a Save and a resume (BR-RRA-4)"]
fn a_paused_scan_finishes_the_choice_it_started_with() {
    let world = rra_world();
    let mut browser = given_owner_verified(&world, Account::Priyaraman);
    given_saved_choice(&mut browser, Account::Priyaraman, &PRIYA_CHOICE);
    let before_pause = given_scan_paused_part_way(&world, &mut browser);
    given_saved_choice(
        &mut browser,
        Account::Priyaraman,
        &PRIYA_CHOICE_WITHOUT_QUILL,
    );

    let after_resume = when_paused_scan_resumes(&world, &mut browser);

    let whole_scan: std::collections::BTreeSet<String> =
        before_pause.union(&after_resume).cloned().collect();
    assert_eq!(
        whole_scan,
        set_of(&PRIYA_CHOICE),
        "the plan frozen at start"
    );
    assert!(
        before_pause.is_disjoint(&after_resume),
        "the resume reads only what was left"
    );
}

/// RS-2b (chained on RS-2a)
/// ```gherkin
/// @US-RRA-002 @AC-002.2 @BR-RRA-4 @contract-shape:unbounded-preservation
/// Scenario: The next scan uses the choice saved during the last one
///   Given Priya saved a choice without quill-docs while a scan was running, and that scan has finished
///   When she presses "Scan again", without opening "Choose repos"
///   Then exactly tidepool, kelp, harbor-cli and ferrous-notes are read
/// ```
#[test]
#[ignore = "DELIVER 01-18: a choice saved during a scan applies from the next scan"]
fn the_next_scan_uses_the_choice_saved_during_the_last_one() {
    let world = rra_world();
    let mut browser = given_owner_verified(&world, Account::Priyaraman);
    given_saved_choice(&mut browser, Account::Priyaraman, &PRIYA_CHOICE);
    given_scan_paused_part_way(&world, &mut browser);
    given_saved_choice(
        &mut browser,
        Account::Priyaraman,
        &PRIYA_CHOICE_WITHOUT_QUILL,
    );
    when_paused_scan_resumes(&world, &mut browser);

    let (status, read) = when_scans(&world, &mut browser);

    assert_eq!(status, "completed", "{}", browser.page.text());
    assert_eq!(read, set_of(&PRIYA_CHOICE_WITHOUT_QUILL));
}

/// RS-3
/// ```gherkin
/// @US-RRA-002 @AC-002.3 @D-RRA-6 @edge @contract-shape:unbounded-preservation
/// Scenario: A repo created after the choice is not read until ticked
///   Given Priya saved her choice and then created priyaraman/sandpiper
///   When she opens "Choose repos"
///   Then sandpiper is listed unticked and marked "new since your last choice"
///   And her next scan does not read sandpiper
/// ```
#[test]
#[ignore = "DELIVER 01-11: repos new since the choice are marked and not read"]
fn a_repo_created_after_the_choice_is_not_read_until_ticked() {
    let world = rra_world();
    let mut browser = given_owner_verified(&world, Account::Priyaraman);
    given_saved_choice(&mut browser, Account::Priyaraman, &PRIYA_CHOICE);
    creates_repo(&world, SANDPIPER);

    let page = when_opens_choose_repos(&mut browser).clone();

    assert!(listed_repos(&page, Account::Priyaraman).contains(&SANDPIPER.full()));
    assert!(!ticked_repos(&page, Account::Priyaraman).contains(&SANDPIPER.full()));
    assert!(
        marked_new(&page, Account::Priyaraman, SANDPIPER),
        "{}",
        page.text()
    );
    assert!(!given_scanned(&world, &mut browser).contains(&SANDPIPER.full()));
}

/// RS-4 (table over what happened to a chosen repo)
/// ```gherkin
/// @US-RRA-002 @AC-002.4 @error @C6b @contract-shape:unbounded-preservation
/// Scenario Outline: A chosen repo that can no longer be read is skipped and named
///   Given Priya chose priyaraman/kelp and then <change> on GitHub
///   When she presses "Scan again"
///   Then kelp is not read and her other chosen repos are
///   And the summary says "<message>"
///   Examples:
///     | change                       | message                                                                                      |
///     | archived it                  | Skipped priyaraman/kelp: archived since you chose it. Untick it, or unarchive it on GitHub.   |
///     | turned it into a fork        | Skipped priyaraman/kelp: a fork since you chose it. Untick it.                               |
///     | transferred it to tidal-labs | Skipped priyaraman/kelp: missing or owned by someone else since you chose it. Untick it.     |
/// ```
#[test]
#[ignore = "DELIVER 01-12: skipped chosen repos are named with reason and fix"]
fn a_chosen_repo_that_can_no_longer_be_read_is_skipped_and_named() {
    #[derive(Debug, Clone, Copy)]
    enum Change {
        Archived,
        NowAFork,
        TransferredAway,
    }
    for change in [Change::Archived, Change::NowAFork, Change::TransferredAway] {
        let world = rra_world();
        let mut browser = given_owner_verified(&world, Account::Priyaraman);
        given_saved_choice(&mut browser, Account::Priyaraman, &PRIYA_CHOICE);
        let reason = match change {
            Change::Archived => {
                world.github.archive_repo(KELP.owner, KELP.name);
                "Skipped priyaraman/kelp: archived since you chose it. Untick it, or unarchive it on GitHub."
            }
            Change::NowAFork => {
                world.github.put_repo(
                    KELP.owner,
                    openlore_test_support::GithubRepo::forked(KELP.name),
                );
                "Skipped priyaraman/kelp: a fork since you chose it. Untick it."
            }
            Change::TransferredAway => {
                world
                    .github
                    .transfer_repo(KELP.owner, KELP.name, KELP_AT_TIDAL_LABS.owner);
                "Skipped priyaraman/kelp: missing or owned by someone else since you chose it. Untick it."
            }
        };

        let read = given_scanned(&world, &mut browser);

        let mut others = set_of(&PRIYA_CHOICE);
        others.remove(&KELP.full());
        assert_eq!(read, others, "{change:?}: kelp is not read, the rest are");
        let summary = scan_summary(&mut browser);
        assert!(summary.contains(reason), "{change:?}: {summary}");
    }
}

/// RS-5
/// ```gherkin
/// @US-RRA-002 @AC-002.5 @error @C1a @C4b @kpi @contract-shape:unbounded-preservation
/// Scenario: A scan with nothing to read does not start
///   Given Priya unticked every repo and has no imported repo
///   When she presses "Scan again"
///   Then she sees "Choose at least one repo, or name a public repo, before scanning."
///   And no repo is read and the refusal is counted
///   And after she ticks her repos again she can still run all 6 of today's scans, and a 7th says "You have scanned 6 times today"
/// ```
#[test]
#[ignore = "DELIVER 01-13: nothing to read refuses before admission"]
fn a_scan_with_nothing_to_read_does_not_start() {
    let world = rra_world();
    let mut browser = given_owner_verified(&world, Account::Priyaraman);
    given_saved_choice(&mut browser, Account::Priyaraman, &[]);
    let mark = github_mark(&world);
    let completed_before = kpi(&world, "scan.completed");

    when_presses_scan(&mut browser);

    assert!(
        browser.page.shows(NOTHING_TO_SCAN),
        "{}",
        browser.page.text()
    );
    assert!(repos_read_since(&world, mark).is_empty());
    assert_eq!(kpi(&world, "scan.completed"), completed_before);
    assert_eq!(kpi(&world, "scan.refused.nothing_to_read"), 1);
    given_saved_choice(&mut browser, Account::Priyaraman, &PRIYA_CHOICE);
    given_scans_used(&world, &mut browser, 6);
    when_presses_scan(&mut browser);
    assert!(
        browser.page.shows(DAILY_LIMIT_REACHED),
        "the refused scan used none of the 6 a day:\n{}",
        browser.page.text()
    );
}

/// RS-6
/// ```gherkin
/// @US-RRA-002 @AC-002.6 @D-RRA-7 @BR-1 @contract-shape:bounded-change
/// Scenario: Unticking a repo keeps its pending suggestions
///   Given Priya has 2 pending suggestions for priyaraman/driftwood and 1 declined one
///   And she saved a choice without driftwood
///   When she presses "Scan again"
///   Then driftwood is not read
///   And the 2 pending suggestions are still in her queue and the declined one stays declined
/// ```
#[test]
#[ignore = "DELIVER 01-14: unticking stops reads only"]
fn unticking_a_repo_keeps_its_pending_suggestions() {
    let world = rra_world();
    let mut browser = given_owner_verified(&world, Account::Priyaraman);
    given_scanned(&world, &mut browser);
    let declined = DRIFTWOOD.embodies(Philosophy::DependencyPinning);
    when_declines_card(&mut browser, declined);
    let pending_before = phrases_about(&mut browser, DRIFTWOOD);
    assert_eq!(
        pending_before.len(),
        2,
        "precondition: 2 pending driftwood suggestions"
    );

    given_saved_choice(&mut browser, Account::Priyaraman, &PRIYA_CHOICE);

    let (status, read) = when_scans(&world, &mut browser);

    assert_eq!(status, "completed");
    assert!(!read.contains(&DRIFTWOOD.full()));
    assert_eq!(phrases_about(&mut browser, DRIFTWOOD), pending_before);
    assert!(!pending_on_queue(&mut browser).contains(&declined.phrase()));
}

/// RS-7 (table: rename-safe ids, C-RRA-D3)
/// ```gherkin
/// @US-RRA-002 @AC-002.1 @C-RRA-D3 @ADR-088 @edge @contract-shape:unbounded-preservation
/// Scenario: A repo renamed on GitHub stays chosen
///   Given Priya saved her choice including priyaraman/kelp
///   And she renamed kelp to kelp-models on GitHub
///   When she presses "Scan again"
///   Then exactly tidepool, quill-docs, kelp-models, harbor-cli and ferrous-notes are read
///   And "Choose repos" shows priyaraman/kelp-models ticked
/// ```
#[test]
#[ignore = "DELIVER 01-15: a renamed repo stays ticked (repo ids)"]
fn a_repo_renamed_on_github_stays_chosen() {
    let world = rra_world();
    let mut browser = given_owner_verified(&world, Account::Priyaraman);
    given_saved_choice(&mut browser, Account::Priyaraman, &PRIYA_CHOICE);
    world
        .github
        .rename_repo(KELP.owner, KELP.name, "kelp-models");

    let read = given_scanned(&world, &mut browser);

    let mut expected = set_of(&PRIYA_CHOICE);
    expected.remove(&KELP.full());
    expected.insert("priyaraman/kelp-models".to_string());
    assert_eq!(read, expected);
    let page = when_opens_choose_repos(&mut browser).clone();
    assert!(
        ticked_repos(&page, Account::Priyaraman).contains(&"priyaraman/kelp-models".to_string())
    );
}

// =============================================================================
// ADR-088 — the store upgrade (infrastructure, with-stale-config)
// =============================================================================

/// What the frozen v1 store holds besides the pending suggestions.
const V1_DECLINED: Suggestion = DRIFTWOOD.embodies(Philosophy::DependencyPinning);
const V1_PUBLISHED: Suggestion = KELP.embodies(Philosophy::DependencyPinning);

/// MG-1
/// ```gherkin
/// @ADR-088 @migration-v2 @with-stale-config @real-io @contract-shape:unbounded-preservation
/// Scenario: Priya's suggestions are all still there after the update, each naming her account
///   Given Priya's suggestions, one decline and one published claim were kept by the release before this feature
///   And the new release has started on that data
///   And she has signed in
///   When she opens her queue
///   Then she sees exactly the pending suggestions she saw before, each "from github.com/priyaraman"
///   And neither her declined nor her published suggestion is pending
///   And "Choose repos" shows today's top 5 ticked
///   And one dated copy of her data as it was before the update is kept
///   And that copy still holds her old suggestions: started on its own, it shows the same queue
/// ```
#[test]
#[ignore = "DELIVER 01-16: store v1 → v2 keeps every suggestion as it was (ADR-088)"]
fn priyas_suggestions_are_all_still_there_after_the_update() {
    let world = rra_world_on_v1_store();
    let mut browser = given_signed_in(&world, Persona::Priya);

    let pending = pending_on_queue(&mut browser);

    assert_eq!(pending, v1_golden_pending(), "the same pending suggestions");
    for gone in [V1_DECLINED, V1_PUBLISHED] {
        assert!(
            !pending.contains(&gone.phrase()),
            "{} is not pending",
            gone.phrase()
        );
    }
    for card in queue_cards(&mut browser) {
        assert!(
            card.text.contains("from github.com/priyaraman"),
            "{}",
            card.text
        );
    }
    let page = when_opens_choose_repos(&mut browser).clone();
    assert_eq!(
        ticked_repos(&page, Account::Priyaraman),
        fulls(&PRIYA_TOP_5)
    );
    let backups = store_backups(&world);
    assert_eq!(backups.len(), 1, "one v1 copy: {backups:?}");
    assert!(is_v1_backup_name(&backups[0]), "{}", backups[0]);
    let from_backup = rra_world_on_store_file(&world.app.data_dir().join(&backups[0]));
    let mut again = given_signed_in(&from_backup, Persona::Priya);
    assert_eq!(
        pending_on_queue(&mut again),
        v1_golden_pending(),
        "the copy is the v1 data"
    );
}

/// MG-2 (chained on MG-1)
/// ```gherkin
/// @ADR-088 @migration-v2 @idempotent @C4a @contract-shape:unbounded-preservation
/// Scenario: Restarting after the update changes nothing Priya sees
///   Given the new release updated Priya's data once, keeping one copy of the old data
///   When the app restarts
///   Then her queue, accounts, repos you don't own and published claims are exactly as before the restart
///   And the one copy of the old data is unchanged and no second copy is made
/// ```
#[test]
fn restarting_after_the_update_changes_nothing_priya_sees() {
    let world = rra_world_on_v1_store();
    let mut browser = given_signed_in(&world, Persona::Priya);
    let before = capture_rra(&world, &mut browser, Persona::Priya);
    let backups = store_backups(&world);
    assert_eq!(
        backups.len(),
        1,
        "precondition: the first start updated the data once"
    );
    let copy_before = backup_bytes(&world, &backups[0]);

    let world = world.restart_app();

    let mut browser = given_signed_in(&world, Persona::Priya);
    let after = capture_rra(&world, &mut browser, Persona::Priya);
    assert_state_delta(&before, &after, &rra_universe(), &Delta::new());
    assert_eq!(store_backups(&world), backups, "no second copy");
    assert_eq!(
        backup_bytes(&world, &backups[0]),
        copy_before,
        "never overwritten"
    );
}

/// MG-3 (interruption)
///
/// Fault seam (DELIVER): `REVIEW_APP_TEST_FAULT=migrate_before_commit` makes
/// the v1 → v2 transaction stop just before COMMIT and the process exit. The
/// seam is honoured by development builds only; a release build must refuse
/// to start with it set (add it to the xtask release-build env guard, like
/// `REVIEW_APP_CLOCK_SKEW_SECS`).
/// ```gherkin
/// @ADR-088 @migration-v2 @with-stale-config @C7b @interruption @contract-shape:unbounded-preservation
/// Scenario: An update cut off before it finished runs again cleanly
///   Given Priya's data was kept by the release before this feature
///   And the new release's first start was cut off before the update finished
///   When the app starts again
///   Then she sees exactly the pending suggestions she saw before, each "from github.com/priyaraman"
///   And the copy of her old data made by the first start is unchanged, beside any new copy
/// ```
#[test]
#[ignore = "DELIVER 01-19: an interrupted v1 → v2 upgrade re-runs cleanly (ADR-088 §5.2 step 3)"]
fn an_update_cut_off_before_it_finished_runs_again_cleanly() {
    let (world, cut_off) = rra_world_on_v1_store_cut_off_mid_update();
    let first_copies = cut_off.backups.clone();
    let first_copy_bytes = cut_off.first_copy_bytes.clone();
    assert_eq!(
        first_copies.len(),
        1,
        "precondition: the cut-off start copied the old data first"
    );

    let world = cut_off.start_again(world);

    let mut browser = given_signed_in(&world, Persona::Priya);
    assert_eq!(pending_on_queue(&mut browser), v1_golden_pending());
    for card in queue_cards(&mut browser) {
        assert!(
            card.text.contains("from github.com/priyaraman"),
            "{}",
            card.text
        );
    }
    let copies = store_backups(&world);
    assert!(copies.contains(&first_copies[0]), "{copies:?}");
    assert_eq!(
        backup_bytes(&world, &first_copies[0]),
        first_copy_bytes,
        "the first copy is never overwritten"
    );
}
