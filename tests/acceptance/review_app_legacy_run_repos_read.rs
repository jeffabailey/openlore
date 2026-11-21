//! The review queue's "repos read" line tells the truth about the owner's
//! latest run.
//!
//! A completed run kept by a release that stored no scan plan has no plan to
//! count, so her queue says nothing about repos read. A run that does have a
//! plan keeps its count on the queue — including the count nought, when its
//! planned repo could not be read.
//!
//! Driving port: the app's HTTP pages, opened by a signed-in owner through the
//! JS-less browser. GitHub and the ATProto network are the hermetic doubles;
//! nothing in here opens the app's store.
//!
//! Each scenario also shows what the owner sees once it has judged her queue —
//! her run's stored status, her queue's status region and whether a repos-read
//! line is there at all — so a reader of the run reads the same thing the
//! assertions do (`--nocapture`).

#[path = "support/review_app/mod.rs"]
mod review_app;

use std::collections::BTreeSet;

use review_app::app::show_queue_evidence;
use review_app::repos_and_accounts::repos::*;
use review_app::repos_and_accounts::*;
use review_app::*;

// =============================================================================
// Vocabulary — what the queue says about a run, in the owner's words
// =============================================================================

/// What the queue calls a run's count of repos read ("5 repos read", and
/// "1 repo read" for a single one).
fn repos_read_line(read: usize) -> String {
    match read {
        1 => "1 repo read".to_string(),
        n => format!("{n} repos read"),
    }
}

/// Any wording of the repos-read line, whatever its count: a queue that holds
/// neither phrase says nothing at all about repos read.
const REPOS_READ_WORDINGS: [&str; 2] = ["repos read", "repo read"];

/// The action a finished scan offers on the queue.
const SCAN_AGAIN: &str = "Scan again";

/// What a completed run's summary tells her it found.
const FOUND: &str = "new suggestion";

/// The line that names a chosen repo her scan could not read because it was
/// archived after she chose it — visible proof that the run holds a plan row
/// about that repo.
fn could_not_read_archived(repo: Repo) -> String {
    format!(
        "Skipped {}: archived since you chose it. Untick it, or unarchive it on GitHub.",
        repo.full()
    )
}

/// GIVEN her suggestions and her one completed run were kept by the release
/// before this app stored a scan plan, and this app has started on them.
fn given_a_run_kept_before_scan_plans_were_stored() -> ReviewWorld {
    rra_world_on_v1_store()
}

/// GIVEN a repo she chose is archived on GitHub after her choice, so her next
/// scan cannot read it.
fn given_archived_after_her_choice(world: &ReviewWorld, repo: Repo) {
    world.github.archive_repo(repo.owner, repo.name);
}

/// GIVEN her latest scan finished having read none of the repos it planned.
fn given_a_scan_that_read_nothing(world: &ReviewWorld, browser: &mut Browser) -> BTreeSet<String> {
    let read = given_scanned(world, browser);
    assert!(
        read.is_empty(),
        "GIVEN broken: this scan was meant to read nothing, and it read {read:?}"
    );
    read
}

/// WHEN she opens her queue: what she reads there.
fn when_she_opens_her_queue(browser: &mut Browser) -> String {
    scan_summary(browser)
}

/// THEN what she opened is a finished scan's queue: the very page that carries
/// a repos-read line when there is one to carry. Without this, an error page, a
/// signed-out page or a queue with no run at all would satisfy any claim about
/// what the page does NOT say.
fn then_it_is_a_finished_scans_queue(browser: &mut Browser, queue: &str) {
    assert!(
        browser.page.offers(SCAN_AGAIN),
        "WHAT: her queue does not offer {SCAN_AGAIN:?}, so this is not the page a finished \
         scan leaves.\n\
         WHY: every claim below is about what a COMPLETED run's queue says; on another page \
         the claim is vacuous and would pass while the line stayed wrong.\n\
         HOW: make a signed-in owner whose latest run completed reach her queue before \
         reading it. Her page was:\n{queue}"
    );
    assert!(
        queue.contains(FOUND),
        "WHAT: her queue carries no completed-run summary (no {FOUND:?}).\n\
         WHY: the repos-read line belongs to a completed run's summary; without that summary \
         there is no run whose count could be shown or withheld.\n\
         HOW: let her latest run finish and complete before her queue is read. Her page \
         was:\n{queue}"
    );
}

/// THEN her queue says nothing at all about repos read.
fn then_her_queue_says_nothing_about_repos_read(queue: &str) {
    for wording in REPOS_READ_WORDINGS {
        assert!(
            !queue.contains(wording),
            "WHAT: her queue says {wording:?} for a run kept before any scan plan was \
             stored.\n\
             WHY: that run recorded no planned repo, so there is nothing to count; the \
             count she is shown is invented, and \"0 repos read\" tells her the scan read \
             none of her repos when in truth nobody ever wrote down which repos it read.\n\
             HOW: show the repos-read line only for a run that has planned repos, and leave \
             the line out entirely for a run that has none. Her page was:\n{queue}"
        );
    }
}

/// THEN her queue says how many of the run's planned repos it read.
fn then_her_queue_counts_repos_read(queue: &str, read: usize) {
    let line = repos_read_line(read);
    assert!(
        queue.contains(&line),
        "WHAT: her queue does not say {line:?} for a run that planned repos and read \
         {read} of them.\n\
         WHY: a run with a plan has a true count to report, nought included; dropping the \
         line leaves her unable to tell a scan that read her repos from one that read \
         none.\n\
         HOW: keep the repos-read line for every run that has planned repos, whatever its \
         count. Her page was:\n{queue}"
    );
}

/// THEN her queue names the chosen repo this run planned and could not read —
/// so the run demonstrably holds a plan, and its count of nought is a count
/// over something.
fn then_her_queue_names_the_repo_it_planned_and_could_not_read(queue: &str, repo: Repo) {
    let line = could_not_read_archived(repo);
    assert!(
        queue.contains(&line),
        "WHAT: her queue does not say {line:?}.\n\
         WHY: this line is the only public sign that the run planned that repo; without it \
         the scenario cannot tell a run that planned one repo and read none from a run \
         that planned nothing at all — the very distinction under test.\n\
         HOW: her chosen repo must be archived on GitHub before this scan, so the scan \
         plans it and reports that it could not read it. Her page was:\n{queue}"
    );
}

/// THEN her suggestions are exactly the ones the earlier release kept for her.
fn then_her_suggestions_are_the_kept_ones(browser: &mut Browser) {
    let pending = pending_on_queue(browser);
    assert_eq!(
        pending,
        v1_golden_pending(),
        "WHAT: her queue does not show the suggestions the earlier release kept.\n\
         WHY: this is what proves her old run's data was actually read and shown; a queue \
         that lost it is not the queue whose repos-read line is under test.\n\
         HOW: start the app on the kept data and sign her in before reading her queue."
    );
}

// =============================================================================
// A run kept before scan plans were stored
// =============================================================================

/// ```gherkin
/// @real-io
/// Scenario: A run kept from before scan plans were stored says nothing about repos read
///   Given Priya's suggestions and her one completed run were kept by the release before this app stored a scan plan
///   And she has signed in
///   When she opens her queue
///   Then it is a finished scan's queue, showing exactly the suggestions that were kept
///   And it says nothing at all about repos read
/// ```
#[test]
fn a_run_kept_before_scan_plans_were_stored_says_nothing_about_repos_read() {
    let world = given_a_run_kept_before_scan_plans_were_stored();
    let mut browser = given_signed_in(&world, Persona::Priya);

    let queue = when_she_opens_her_queue(&mut browser);

    then_it_is_a_finished_scans_queue(&mut browser, &queue);
    then_her_suggestions_are_the_kept_ones(&mut browser);
    then_her_queue_says_nothing_about_repos_read(&queue);
    show_queue_evidence(
        "her queue, her one completed run kept before scan plans were stored",
        &mut browser,
    );
}

// =============================================================================
// A run that has a plan keeps its count
// =============================================================================

/// ```gherkin
/// @real-io
/// Scenario: A scan that read the repos it planned says how many
///   Given Priya saved her choice of five repos and her scan read exactly those five
///   When she opens her queue
///   Then it is a finished scan's queue
///   And it says "5 repos read"
/// ```
#[test]
fn a_scan_that_read_the_repos_it_planned_says_how_many() {
    let world = rra_world();
    let mut browser = given_owner_verified(&world, Account::Priyaraman);
    given_saved_choice(&mut browser, Account::Priyaraman, &PRIYA_CHOICE);
    let read = given_scanned(&world, &mut browser);
    assert_eq!(
        read,
        set_of(&PRIYA_CHOICE),
        "GIVEN broken: the scan was meant to read exactly the repos she chose"
    );

    let queue = when_she_opens_her_queue(&mut browser);

    then_it_is_a_finished_scans_queue(&mut browser, &queue);
    then_her_queue_counts_repos_read(&queue, read.len());
    show_queue_evidence(
        "her queue, her scan having read the five repos she chose",
        &mut browser,
    );
}

/// ```gherkin
/// @real-io
/// Scenario: A scan that could read none of the repos it planned still says so
///   Given Priya chose only priyaraman/kelp and it was archived after her choice
///   And her scan finished having read no repo
///   When she opens her queue
///   Then it is a finished scan's queue, naming kelp as the repo it could not read
///   And it says "0 repos read"
/// ```
#[test]
fn a_scan_that_could_read_none_of_the_repos_it_planned_still_says_so() {
    let world = rra_world();
    let mut browser = given_owner_verified(&world, Account::Priyaraman);
    given_saved_choice(&mut browser, Account::Priyaraman, &[KELP]);
    given_archived_after_her_choice(&world, KELP);
    let read = given_a_scan_that_read_nothing(&world, &mut browser);

    let queue = when_she_opens_her_queue(&mut browser);

    then_it_is_a_finished_scans_queue(&mut browser, &queue);
    then_her_queue_names_the_repo_it_planned_and_could_not_read(&queue, KELP);
    then_her_queue_counts_repos_read(&queue, read.len());
    show_queue_evidence(
        "her queue, her scan having read none of the one repo she chose",
        &mut browser,
    );
}
