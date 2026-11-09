//! A scan that cannot save what it did tells the truth about it.
//!
//! When the review store refuses one of the writes a scan makes — marking a
//! planned repo read, or recording how each GitHub account's check ended —
//! the owner must not be told the scan finished. The run ends interrupted,
//! her queue offers "Resume scan" instead of a count of repos read, and once
//! the store accepts those writes again, resuming reads the repos whose
//! reads were never saved.
//!
//! Both refusals are driven through the app's public HTTP port on a
//! development build started with the review app's fault seam
//! (`REVIEW_APP_TEST_FAULT`), the same seam the migration scenarios use. The
//! store, the sign-in, the ownership proof and the scan are the real ones;
//! only GitHub and the ATProto network are doubles.

#[path = "support/review_app/mod.rs"]
mod review_app;

use std::collections::BTreeSet;

use review_app::app::TEST_FAULT_VAR;
use review_app::repos_and_accounts::{github_mark, repos_read_since, when_scans};
use review_app::*;

// =============================================================================
// Domain vocabulary
// =============================================================================

/// The one scan write the owner's store refuses, as she would describe it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RefusedWrite {
    /// Saving that a planned repo has been read.
    SavingThatARepoWasRead,
    /// Saving how each of her GitHub accounts' checks ended.
    SavingHerAccountChecks,
}

impl RefusedWrite {
    /// How the owner would name the refusal in her own words.
    fn named(self) -> &'static str {
        match self {
            Self::SavingThatARepoWasRead => "saving that a repo was read",
            Self::SavingHerAccountChecks => "saving her account checks",
        }
    }

    /// The fault the development build is started with to refuse it.
    fn fault(self) -> (String, String) {
        let name = match self {
            Self::SavingThatARepoWasRead => "refuse_mark_repo_read",
            Self::SavingHerAccountChecks => "refuse_record_account_checks",
        };
        (TEST_FAULT_VAR.to_string(), name.to_string())
    }
}

/// What the queue says about repos read, in both its wordings.
const REPOS_READ_WORDINGS: [&str; 2] = ["repos read", "repo read"];

/// The actions that start a new scan instead of picking up an unfinished
/// one; the first is what a finished run offers.
const FRESH_SCAN_LABELS: [&str; 2] = ["Scan again", "Scan my repos"];

/// The action a finished scan offers; a run that is not finished must not.
const SCAN_AGAIN: &str = FRESH_SCAN_LABELS[0];

/// The action an unfinished run offers its owner.
const RESUME_SCAN: &str = "Resume scan";

// =============================================================================
// Given / When / Then steps
// =============================================================================

/// GIVEN the owner's review store refuses `write` every time the scan makes
/// it, and behaves normally otherwise.
fn given_store_refusing(write: RefusedWrite) -> ReviewWorld {
    let (name, value) = write.fault();
    let asked = value.clone();
    let (github, atproto, startup) = ReviewWorld::launch(
        |_, _| {},
        move |settings| settings.extra_env.push((name, value)),
    );
    match startup {
        Startup::Ready(app) => ReviewWorld {
            github,
            atproto,
            app: *app,
        },
        Startup::Refused { exit_code, logs } => panic!(
            "WHAT: the review app refused to start when asked to refuse {} \
             (asked for {asked:?}, exit {exit_code:?}).\n\
             WHY: a scan whose store write fails must end interrupted and stay \
             resumable instead of claiming it finished; with no way to refuse \
             that write, nothing can show the owner the honest outcome.\n\
             HOW: a development build must accept {asked:?} on its fault seam, \
             make that one store write return an error every time the scan \
             calls it while every other read and write behaves normally, keep \
             refusing to start in a release build, and keep the release binary \
             free of the new names.\n--- logs ---\n{logs}",
            write.named()
        ),
    }
}

/// GIVEN the owner is signed in and her GitHub account is verified.
fn given_owner_ready_to_scan(world: &ReviewWorld) -> Browser {
    given_github_verified(world, Persona::Priya)
}

/// WHEN the store accepts those writes again (the app runs again on the same
/// store, holding no fault).
fn when_the_store_accepts_writes_again(world: ReviewWorld) -> ReviewWorld {
    let ReviewWorld {
        github,
        atproto,
        app,
    } = world;
    ReviewWorld {
        github,
        atproto,
        app: app.restart_without_test_fault(),
    }
}

/// WHEN she picks the unfinished scan up from her queue — the only scan
/// action it offers — and it runs to its end: the status it ended on, and the
/// repos the resumed run read.
fn when_she_resumes(world: &ReviewWorld, browser: &mut Browser) -> (String, BTreeSet<String>) {
    let mark = github_mark(world);
    let page = browser.open("/review");
    let queue = page.text();
    assert!(
        page.offers(RESUME_SCAN),
        "WHAT: the queue offers no {RESUME_SCAN:?} to pick the unfinished scan up with.\n\
         WHY: a scan the store cut short keeps the suggestions it found and must be \
         resumable by its owner, or the repos it never recorded are lost to her.\n\
         HOW: end a run whose store write was refused the way an unreadable source \
         list ends it, so the queue offers {RESUME_SCAN:?}.\nqueue:\n{queue}"
    );
    for starting_over in FRESH_SCAN_LABELS {
        assert!(
            !page.offers(starting_over),
            "WHAT: the queue offers {starting_over:?} beside {RESUME_SCAN:?} for a scan \
             the store cut short.\n\
             WHY: she is owed the run she already paid for: starting a new scan abandons \
             the suggestions it found and spends another of her day's scans.\n\
             HOW: a run that did not finish offers only {RESUME_SCAN:?}.\nqueue:\n{queue}"
        );
    }
    browser.press(RESUME_SCAN);
    let status = then_scan_reaches_on_its_own(browser, "completed", 60);
    (status, repos_read_since(world, mark))
}

/// THEN the scan she ran did not finish: it ended interrupted.
fn then_the_scan_ended_interrupted(status: &str, write: RefusedWrite) {
    assert_eq!(
        status,
        "interrupted",
        "WHAT: a scan the store stopped by refusing {} ended {status:?}.\n\
         WHY: a run that could not save what it did has not done what a finished \
         run promises, and reporting it as finished is what makes the queue claim \
         repos were read that were not.\n\
         HOW: when one of the scan's store writes is refused, end the run \
         interrupted, as an unreadable source list already ends it.",
        write.named()
    );
}

/// THEN her queue offers to resume that scan and claims no repos read.
fn then_her_queue_offers_to_resume_and_claims_no_count(browser: &mut Browser) {
    let page = browser.open("/review");
    let text = page.text();
    assert!(
        page.offers(RESUME_SCAN),
        "WHAT: the queue does not offer {RESUME_SCAN:?} after a scan the store cut short.\n\
         WHY: the suggestions the scan already found are kept, so the owner must be \
         able to pick that run up rather than start over.\n\
         HOW: record the run as interrupted so the queue offers {RESUME_SCAN:?}.\nqueue:\n{text}"
    );
    assert!(
        !page.offers(SCAN_AGAIN),
        "WHAT: the queue offers {SCAN_AGAIN:?} after a scan the store cut short, \
         which is what it offers a run that finished.\n\
         WHY: telling the owner the scan finished hides that nothing of it was saved.\n\
         HOW: record the run as interrupted, not completed.\nqueue:\n{text}"
    );
    for wording in REPOS_READ_WORDINGS {
        assert!(
            !text.contains(wording),
            "WHAT: the queue claims {wording:?} after a scan the store cut short.\n\
             WHY: the count comes from what the store holds for that run, and the \
             refused write means it holds less than the scan actually read; shown on \
             an unfinished run it is simply untrue.\n\
             HOW: only a finished run accounts for the repos it read.\nqueue:\n{text}"
        );
    }
}

/// THEN the resumed run read every repo whose read was never saved, and read
/// something.
fn then_the_resume_read_what_was_never_saved(
    read_before: &BTreeSet<String>,
    read_by_resume: &BTreeSet<String>,
) {
    assert!(
        !read_by_resume.is_empty(),
        "WHAT: the resumed scan read no repo at all.\n\
         WHY: nothing of the cut-short run was saved as read, so a resume that reads \
         nothing leaves the owner with a scan that can never account for her repos.\n\
         HOW: a resume reads the planned repos that are not marked read."
    );
    let skipped: Vec<&String> = read_before.difference(read_by_resume).collect();
    assert!(
        skipped.is_empty(),
        "WHAT: the resumed scan skipped {skipped:?}, whose earlier read was never saved.\n\
         WHY: a repo the store never recorded as read has, as far as anything can \
         tell, not been read; skipping it loses whatever it would suggest.\n\
         HOW: treat only the repos the store holds as read as done, and read the rest."
    );
}

/// Every repos-read number the queue claims, in the order it shows them: the
/// whole number in front of each wording, so an over-count that merely ends
/// in the true one is read as the number it is.
fn counts_claimed(text: &str) -> Vec<usize> {
    let mut claimed: Vec<(usize, usize)> = Vec::new();
    for wording in REPOS_READ_WORDINGS {
        let mut from = 0;
        while let Some(found) = text[from..].find(wording) {
            let at = from + found;
            let digits: String = text[..at]
                .trim_end()
                .chars()
                .rev()
                .take_while(|c| c.is_ascii_digit())
                .collect::<Vec<char>>()
                .into_iter()
                .rev()
                .collect();
            if let Ok(number) = digits.parse::<usize>() {
                claimed.push((at, number));
            }
            from = at + wording.len();
        }
    }
    claimed.sort_unstable();
    claimed.into_iter().map(|(_, number)| number).collect()
}

/// THEN her queue now accounts for exactly the repos the resumed run read —
/// one count, neither more nor fewer than she actually got.
fn then_her_queue_counts_exactly(browser: &mut Browser, repos: &BTreeSet<String>) {
    let expected = vec![repos.len()];
    let page = browser.open("/review");
    let text = page.text();
    let claimed = counts_claimed(&text);
    assert_eq!(
        claimed, expected,
        "WHAT: after resuming, the queue accounts for {claimed:?} repos read, not \
         exactly {expected:?}, though the resumed scan read {repos:?}.\n\
         WHY: the whole point of the count is that it matches the repos the scan \
         actually read and the store actually holds; a larger number tells her repos \
         were read that never were, a smaller one hides work she is owed.\n\
         HOW: mark each planned repo read as it is read, and account for exactly \
         those.\nqueue:\n{text}"
    );
}

/// THEN the run she resumed finished.
fn then_the_scan_ended_completed(status: &str) {
    assert_eq!(
        status, "completed",
        "WHAT: the resumed scan ended {status:?} instead of finishing, though the \
         store accepts its writes again.\n\
         WHY: an owner whose store recovered must be able to finish the run that \
         was cut short, or her queue never accounts for her repos.\n\
         HOW: a resume that meets no refusal reads the plan's remaining repos and \
         completes."
    );
}

// =============================================================================
// Scenarios
// =============================================================================

/// ```gherkin
/// Scenario: A scan that cannot save that it read a repo is offered for resuming
///   Given Priya's store refuses to save that a repo was read
///   And she is signed in with her GitHub account verified
///   When she scans her repos
///   Then the scan ended interrupted
///   And her queue offers to resume it and claims no repos read
///   When the store accepts those writes again and she resumes
///   Then the resumed scan read every repo whose read was never saved
///   And her queue then accounts for exactly those repos
/// ```
#[test]
fn a_scan_that_cannot_save_that_it_read_a_repo_is_offered_for_resuming() {
    let refused = RefusedWrite::SavingThatARepoWasRead;
    let world = given_store_refusing(refused);
    let mut browser = given_owner_ready_to_scan(&world);

    let (status, read_before) = when_scans(&world, &mut browser);

    then_the_scan_ended_interrupted(&status, refused);
    then_her_queue_offers_to_resume_and_claims_no_count(&mut browser);

    let world = when_the_store_accepts_writes_again(world);
    let (resumed_status, read_by_resume) = when_she_resumes(&world, &mut browser);

    then_the_scan_ended_completed(&resumed_status);
    then_the_resume_read_what_was_never_saved(&read_before, &read_by_resume);
    then_her_queue_counts_exactly(&mut browser, &read_by_resume);
}

/// ```gherkin
/// Scenario: A scan that cannot save her account checks is offered for resuming
///   Given Priya's store refuses to save her account checks
///   And she is signed in with her GitHub account verified
///   When she scans her repos
///   Then the scan ended interrupted
///   And her queue offers to resume it and claims no repos read
///   When the store accepts those writes again and she resumes
///   Then the resumed scan read every repo whose read was never saved
///   And her queue then accounts for exactly those repos
/// ```
#[test]
fn a_scan_that_cannot_save_her_account_checks_is_offered_for_resuming() {
    let refused = RefusedWrite::SavingHerAccountChecks;
    let world = given_store_refusing(refused);
    let mut browser = given_owner_ready_to_scan(&world);

    let (status, read_before) = when_scans(&world, &mut browser);

    then_the_scan_ended_interrupted(&status, refused);
    then_her_queue_offers_to_resume_and_claims_no_count(&mut browser);

    let world = when_the_store_accepts_writes_again(world);
    let (resumed_status, read_by_resume) = when_she_resumes(&world, &mut browser);

    then_the_scan_ended_completed(&resumed_status);
    then_the_resume_read_what_was_never_saved(&read_before, &read_by_resume);
    then_her_queue_counts_exactly(&mut browser, &read_by_resume);
}
