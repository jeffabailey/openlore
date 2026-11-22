//! The queue accounts for a whole scan, not only for its last run.
//!
//! A scan that reads one of three repos, pauses because GitHub's request
//! budget is used up and is then picked up and read to its end has read three
//! repos. Her queue must say so: the run that picks a paused plan up takes
//! over the reads the earlier runs of that scan already saved, so the count is
//! the count of the scan however often it was picked up, and a repo already
//! read is not read — or counted — a second time.
//!
//! Driving port: the app's HTTP pages, opened by a signed-in owner through the
//! JS-less browser. GitHub and the ATProto network are the hermetic doubles;
//! nothing here opens the app's store.
//!
//! The two journeys observe the count itself. Beneath them, the rule deciding
//! which repos a run picking a plan up reads now is reached through ONE
//! `sut_*` binding that maps the production answer onto the view types
//! declared here: those view types, and the laws asserted over them, are the
//! contract, and the binding reads only answers this feature leaves as they
//! are, so nothing in this file is written twice.
//!
//! Every scenario also shows what the owner sees — the repos GitHub was asked
//! to read, her run's stored status and her queue's status region — before it
//! judges them, so a reader of the run reads the same thing the assertions do
//! (`--nocapture`).

#[path = "support/review_app/mod.rs"]
mod review_app;

use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::{AtomicBool, Ordering};

use proptest::prelude::*;

use review_app::app::{
    repos_read_count, repos_read_lines, show_queue_evidence, show_repos_read, QueueEvidence,
};
use review_app::repos_and_accounts::repos::*;
use review_app::repos_and_accounts::*;
use review_app::*;

// =============================================================================
// Vocabulary — a scan, its plan and its runs, in the owner's words
// =============================================================================

/// How far a run of a scan got with one repo of its frozen plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RowState {
    /// Not read yet by any run of this scan.
    StillToRead,
    /// Already read, and the store saved that.
    AlreadyRead,
    /// Never to be read by this scan.
    Skipped,
}

/// Where a planned repo comes from: one of the owner's GitHub accounts, or a
/// repo she imported. Accounts are checked again by every run; imports are
/// not.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Source {
    Account(u64),
    Import(u64),
}

/// One repo of a paused run's frozen plan, as the run picking it up finds it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PlannedRepo {
    source: Source,
    state: RowState,
}

/// How a run's own check of one GitHub account ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Check {
    Passed,
    Failed,
    Unfinished,
}

/// One repo of the plan the run picking it up keeps as its own, and whether
/// it keeps it as a repo this scan has already read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct KeptRepo {
    position: usize,
    carries_an_earlier_read: bool,
}

/// What a run picking up a paused plan does with it: the repos it keeps as
/// its own plan, which of them it reads now, and which this scan will never
/// read.
#[derive(Debug, Clone, PartialEq, Eq)]
struct PickedUp {
    kept: Vec<KeptRepo>,
    reads_now: Vec<usize>,
    never_reads: Vec<usize>,
}

// =============================================================================
// The binding — ONE call into the rule under test
// =============================================================================

/// What a run picking up the paused plan `plan` does with it, given its own
/// account checks and the sources the owner still lists.
///
/// ONE call: `resume_plan`, the single owner of that verdict, read through
/// the plan the run keeps with the status each of its repos carries, the
/// repos it reads now and the repos it will never read.
fn sut_picked_up(
    plan: &[PlannedRepo],
    checks: &BTreeMap<Source, Check>,
    no_longer_listed: &BTreeSet<Source>,
) -> PickedUp {
    use ports::PlanRowStatus;
    use review_domain::repo_scope::{resume_plan, BioCheck, FrozenRow, PlanSource};

    fn plan_source(source: Source) -> PlanSource {
        match source {
            Source::Account(id) => PlanSource::Account(id),
            Source::Import(id) => PlanSource::Imported(id),
        }
    }

    let rows: Vec<FrozenRow> = plan
        .iter()
        .map(|repo| FrozenRow {
            source: plan_source(repo.source),
            status: match repo.state {
                RowState::StillToRead => PlanRowStatus::Planned,
                RowState::AlreadyRead => PlanRowStatus::Read,
                RowState::Skipped => PlanRowStatus::Skipped,
            },
        })
        .collect();
    let bio_checks: BTreeMap<u64, BioCheck> = checks
        .iter()
        .filter_map(|(source, check)| match source {
            Source::Account(id) => Some((
                *id,
                match check {
                    Check::Passed => BioCheck::Passed,
                    Check::Failed => BioCheck::Failed,
                    Check::Unfinished => BioCheck::Unfinished,
                },
            )),
            Source::Import(_) => None,
        })
        .collect();
    let gone: BTreeSet<PlanSource> = no_longer_listed.iter().map(|s| plan_source(*s)).collect();

    let verdict = resume_plan(&rows, &bio_checks, &gone);

    PickedUp {
        kept: verdict
            .kept()
            .into_iter()
            .map(|row| KeptRepo {
                position: row.position,
                carries_an_earlier_read: row.status == PlanRowStatus::Read,
            })
            .collect(),
        reads_now: verdict.to_read(),
        never_reads: verdict
            .skipped()
            .iter()
            .map(|(position, _)| *position)
            .collect(),
    }
}

// =============================================================================
// The paused plans a run can be given
// =============================================================================

/// Four GitHub accounts and four imported repos: enough sources for a plan
/// whose rows come from several of them, and for one of them to be gone.
const SOURCES: [Source; 8] = [
    Source::Account(1),
    Source::Account(2),
    Source::Account(3),
    Source::Account(4),
    Source::Import(1),
    Source::Import(2),
    Source::Import(3),
    Source::Import(4),
];

/// A paused scan as the run picking it up finds it.
#[derive(Debug, Clone, PartialEq, Eq)]
struct PausedScan {
    plan: Vec<PlannedRepo>,
    checks: BTreeMap<Source, Check>,
    no_longer_listed: BTreeSet<Source>,
}

fn a_source() -> impl Strategy<Value = Source> {
    proptest::sample::select(SOURCES.to_vec())
}

fn an_account() -> impl Strategy<Value = Source> {
    proptest::sample::select(SOURCES[..4].to_vec())
}

fn a_state() -> impl Strategy<Value = RowState> {
    prop_oneof![
        Just(RowState::StillToRead),
        Just(RowState::AlreadyRead),
        Just(RowState::Skipped),
    ]
}

fn a_check() -> impl Strategy<Value = Check> {
    prop_oneof![
        Just(Check::Passed),
        Just(Check::Failed),
        Just(Check::Unfinished)
    ]
}

fn a_planned_repo() -> impl Strategy<Value = PlannedRepo> {
    (a_source(), a_state()).prop_map(|(source, state)| PlannedRepo { source, state })
}

/// Any paused scan: a plan of up to eight repos over her accounts and
/// imports, any check of any account, any sources no longer listed.
fn any_paused_scan() -> impl Strategy<Value = PausedScan> {
    (
        proptest::collection::vec(a_planned_repo(), 0..=8),
        proptest::collection::btree_map(an_account(), a_check(), 0..=4),
        proptest::collection::btree_set(a_source(), 0..=3),
    )
        .prop_map(|(plan, checks, no_longer_listed)| PausedScan {
            plan,
            checks,
            no_longer_listed,
        })
}

/// Put `repo` into `plan` at the generated position `at`.
fn put(plan: &mut Vec<PlannedRepo>, at: proptest::sample::Index, repo: PlannedRepo) {
    let position = at.index(plan.len() + 1);
    plan.insert(position, repo);
}

/// Paused scans that always hold a repo already read and a repo still to
/// read: the shape a run picking the plan up must tell apart, built in
/// rather than hoped for.
fn a_paused_scan_read_part_way() -> impl Strategy<Value = PausedScan> {
    (
        any_paused_scan(),
        a_source(),
        a_source(),
        any::<proptest::sample::Index>(),
        any::<proptest::sample::Index>(),
    )
        .prop_map(|(mut paused, read_from, to_read_from, first, second)| {
            put(
                &mut paused.plan,
                first,
                PlannedRepo {
                    source: read_from,
                    state: RowState::AlreadyRead,
                },
            );
            put(
                &mut paused.plan,
                second,
                PlannedRepo {
                    source: to_read_from,
                    state: RowState::StillToRead,
                },
            );
            paused
        })
}

/// Paused scans that always hold a repo already read whose account passes its
/// check now — the account whose repos this run is about to read.
fn a_paused_scan_read_part_way_by_an_account_checking_out() -> impl Strategy<Value = PausedScan> {
    (
        a_paused_scan_read_part_way(),
        an_account(),
        any::<proptest::sample::Index>(),
    )
        .prop_map(|(mut paused, account, at)| {
            put(
                &mut paused.plan,
                at,
                PlannedRepo {
                    source: account,
                    state: RowState::AlreadyRead,
                },
            );
            paused.checks.insert(account, Check::Passed);
            paused.no_longer_listed.remove(&account);
            paused
        })
}

/// Paused scans that always hold a repo this scan already read whose account
/// fails its check now and whose source the owner has removed since: what the
/// scan read stands whatever has happened to the source it came from.
fn a_paused_scan_read_by_a_source_she_has_since_removed() -> impl Strategy<Value = PausedScan> {
    (
        a_paused_scan_read_part_way(),
        an_account(),
        any::<proptest::sample::Index>(),
    )
        .prop_map(|(mut paused, account, at)| {
            put(
                &mut paused.plan,
                at,
                PlannedRepo {
                    source: account,
                    state: RowState::AlreadyRead,
                },
            );
            paused.checks.insert(account, Check::Failed);
            paused.no_longer_listed.insert(account);
            paused
        })
}

/// Paused scans holding repos this scan already read: some of them from
/// sources still listed and checking out, some from a source whose check
/// fails now and which the owner has removed.
fn a_paused_scan_with_reads_to_keep() -> impl Strategy<Value = PausedScan> {
    prop_oneof![
        a_paused_scan_read_part_way(),
        a_paused_scan_read_part_way_by_an_account_checking_out(),
        a_paused_scan_read_by_a_source_she_has_since_removed(),
    ]
}

/// Paused scans that always hold a repo this scan will never read and a repo
/// whose source the owner has since removed.
fn a_paused_scan_holding_a_skip_and_a_removed_source() -> impl Strategy<Value = PausedScan> {
    (
        a_paused_scan_read_part_way(),
        a_source(),
        a_source(),
        any::<proptest::sample::Index>(),
        any::<proptest::sample::Index>(),
    )
        .prop_map(|(mut paused, skipped_from, removed, first, second)| {
            put(
                &mut paused.plan,
                first,
                PlannedRepo {
                    source: skipped_from,
                    state: RowState::Skipped,
                },
            );
            put(
                &mut paused.plan,
                second,
                PlannedRepo {
                    source: removed,
                    state: RowState::StillToRead,
                },
            );
            paused.no_longer_listed.insert(removed);
            paused
        })
}

/// Show one of the paused plans a property generated and what the run
/// picking it up does with it. The cases are many and alike; a reader needs
/// one of them, so each property shows its first (`--nocapture`).
fn show_one_paused_scan(
    moment: &str,
    shown: &AtomicBool,
    paused: &PausedScan,
    picked_up: &PickedUp,
) {
    if shown.swap(true, Ordering::SeqCst) {
        return;
    }
    println!("==== one of the paused scans: {moment} ====");
    for (position, repo) in paused.plan.iter().enumerate() {
        println!("  - {position}: {:?} from {:?}", repo.state, repo.source);
    }
    if paused.plan.is_empty() {
        println!("  (a plan of no repos at all)");
    }
    println!("  her accounts' checks now: {:?}", paused.checks);
    println!(
        "  sources she no longer lists: {:?}",
        paused.no_longer_listed
    );
    println!(
        "  the run keeps as already read: {:?}",
        picked_up
            .kept
            .iter()
            .filter(|kept| kept.carries_an_earlier_read)
            .map(|kept| kept.position)
            .collect::<Vec<usize>>()
    );
    println!("  the run reads now: {:?}", picked_up.reads_now);
    println!("  the run gives up on: {:?}", picked_up.never_reads);
    println!("==== end of one of the paused scans ====");
}

/// The positions of the plan whose repos an earlier run of the scan read.
fn already_read(plan: &[PlannedRepo]) -> BTreeSet<usize> {
    plan.iter()
        .enumerate()
        .filter(|(_, repo)| repo.state == RowState::AlreadyRead)
        .map(|(position, _)| position)
        .collect()
}

// =============================================================================
// What a run picking up a paused plan reads
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    /// L1 — the run picking a paused plan up keeps as already read exactly
    /// the repos of that plan this scan already read: whatever its own
    /// account checks answer now, and whichever of its sources the owner has
    /// removed since, a repo the scan read stays read and stays counted.
    #[test]
    fn a_run_picking_a_paused_scan_up_keeps_every_repo_the_scan_already_read(
        paused in a_paused_scan_with_reads_to_keep(),
    ) {
        let picked_up = sut_picked_up(&paused.plan, &paused.checks, &paused.no_longer_listed);

        static SHOWN: AtomicBool = AtomicBool::new(false);
        show_one_paused_scan("one she had read part way", &SHOWN, &paused, &picked_up);
        let kept_as_read: BTreeSet<usize> = picked_up
            .kept
            .iter()
            .filter(|kept| kept.carries_an_earlier_read)
            .map(|kept| kept.position)
            .collect();
        prop_assert_eq!(
            kept_as_read.clone(),
            already_read(&paused.plan),
            "WHAT: the run picking this paused scan up keeps {:?} as already read, when \
             the repos this scan has already read are {:?}.\n\
             WHY: the count her queue shows her is the count of the repos her scan's \
             plan holds as read; a read dropped here — because the account it came from \
             fails its check now, or because she has removed that source since — is work \
             she paid for and is never told about, and a read invented here tells her of \
             a repo nobody read.\n\
             HOW: a repo this scan already read is kept as read whatever the checks and \
             the removed sources say now; they decide what is read NOW, never what was \
             read then.\n\
             the paused scan: {:?}",
            kept_as_read,
            already_read(&paused.plan),
            paused
        );
    }

    /// L2 — whatever this run's own account checks say, a repo of the plan an
    /// earlier run already read is never among the repos it reads now: the
    /// scan reads each of her repos once, and so counts it once.
    #[test]
    fn a_run_picking_a_paused_scan_up_does_not_read_a_repo_the_scan_already_read(
        paused in a_paused_scan_read_part_way_by_an_account_checking_out(),
    ) {
        let picked_up = sut_picked_up(&paused.plan, &paused.checks, &paused.no_longer_listed);

        static SHOWN: AtomicBool = AtomicBool::new(false);
        show_one_paused_scan(
            "one whose already-read repo's account checks out now",
            &SHOWN,
            &paused,
            &picked_up,
        );
        let reads_now: BTreeSet<usize> = picked_up.reads_now.iter().copied().collect();
        let read_again: Vec<usize> = already_read(&paused.plan)
            .intersection(&reads_now)
            .copied()
            .collect();
        prop_assert!(
            read_again.is_empty(),
            "WHAT: the run picking this paused scan up reads {read_again:?} again, \
             which this scan has already read and saved.\n\
             WHY: reading a repo again spends the owner's GitHub budget on work her \
             scan already did, and a repo accounted for twice makes the count she is \
             shown larger than the scan it reports on.\n\
             HOW: a run reads the repos of the plan still to read, never the ones the \
             scan has already read.\n\
             the paused scan: {:?}\n\
             what the run does with it: {:?}",
            paused,
            picked_up
        );
    }

    /// L3 — the run picking a paused plan up reads and gives up on repos of
    /// that plan only, in plan order, each at most once: a plan never grows
    /// from one run to the next, however often a scan is picked up.
    #[test]
    fn a_run_picks_up_repos_of_the_plan_it_was_given_and_no_others(
        paused in a_paused_scan_holding_a_skip_and_a_removed_source(),
    ) {
        let picked_up = sut_picked_up(&paused.plan, &paused.checks, &paused.no_longer_listed);

        static SHOWN: AtomicBool = AtomicBool::new(false);
        show_one_paused_scan(
            "one holding a repo given up on and a removed source",
            &SHOWN,
            &paused,
            &picked_up,
        );
        let kept: Vec<usize> = picked_up.kept.iter().map(|kept| kept.position).collect();
        let mut accounted: Vec<usize> = kept
            .iter()
            .chain(&picked_up.never_reads)
            .copied()
            .collect();
        let distinct: BTreeSet<usize> = accounted.iter().copied().collect();
        accounted.sort_unstable();
        let within_the_plan = accounted.iter().all(|p| *p < paused.plan.len());
        let in_plan_order = kept.windows(2).all(|pair| pair[0] < pair[1])
            && picked_up.never_reads.windows(2).all(|pair| pair[0] < pair[1]);
        prop_assert!(
            within_the_plan && in_plan_order && distinct.len() == accounted.len(),
            "WHAT: the run picking this paused scan up accounts for {accounted:?}, which \
             is not a part of its {}-repo plan taken in plan order, each repo once.\n\
             WHY: the owner's scan is the choice she made when it started; a run that \
             takes on a repo outside that plan, or takes one on twice, reads and counts \
             repos her scan was never about.\n\
             HOW: the run reads and gives up on rows of the plan it picked up, in the \
             order of that plan, each position at most once.\n\
             the paused scan: {:?}\n\
             what the run does with it: {:?}",
            paused.plan.len(),
            paused,
            picked_up
        );
    }
}

// =============================================================================
// Her scan, paused for GitHub's budget and picked up
// =============================================================================

/// WHEN GitHub's budget refills, she picks the scan up, and the budget runs
/// low again once the run has read one more repo: the repos that run read.
fn when_she_picks_it_up_and_it_pauses_again(
    world: &ReviewWorld,
    browser: &mut Browser,
) -> BTreeSet<String> {
    let mark = github_mark(world);
    world.github.set_remaining(4_990);
    world.github.budget_runs_low_after_repos(1, 120);
    let status = when_scan_finishes(browser);
    assert_eq!(
        status,
        "rate_limited",
        "GIVEN broken: the run she picked up was meant to pause for GitHub's budget \
         again:\n{}",
        browser.page.text()
    );
    repos_read_since(world, mark)
}

/// THEN the runs of her scan read her three chosen repos between them, each
/// run reading repos none of the others read: the repos the whole scan read.
fn then_the_runs_read_her_chosen_repos_between_them(
    runs: &[(&str, &BTreeSet<String>)],
) -> BTreeSet<String> {
    let mut whole_scan: BTreeSet<String> = BTreeSet::new();
    for (moment, read) in runs {
        let read_again: Vec<&String> = read.intersection(&whole_scan).collect();
        assert!(
            read_again.is_empty(),
            "WHAT: {moment} read {read_again:?} again, which this scan had already \
             read.\n\
             WHY: every claim below is about a scan that read each of her repos once; a \
             repo read twice is her GitHub budget spent on work the scan had already \
             done, and would make a count of three true for the wrong reason.\n\
             HOW: a run picking a scan up reads the repos of its plan not yet read, and \
             leaves the rest alone."
        );
        whole_scan.extend((*read).iter().cloned());
    }
    assert_eq!(
        whole_scan,
        set_of(&PRIYA_CHOICE_OF_THREE),
        "WHAT: the runs of her scan read {whole_scan:?} between them, not the three \
         repos she chose.\n\
         WHY: every claim below is about a scan that read her three chosen repos; over \
         any other scan a count of three says nothing.\n\
         HOW: a scan reads the plan it froze from her choice, and the runs picking it up \
         finish that plan."
    );
    whole_scan
}

/// THEN her queue accounts for the repos her whole scan read — one count, and
/// that count the repos of the scan, not of its last run.
fn then_her_queue_accounts_for_the_whole_scan(seen: &QueueEvidence, whole_scan: &BTreeSet<String>) {
    let lines = repos_read_lines(&seen.status_region);
    assert_eq!(
        lines.len(),
        1,
        "WHAT: her finished scan's queue carries {} lines accounting for repos read, \
         not exactly one: {lines:?}.\n\
         WHY: a finished run that has a plan owes her one account of it; no line leaves \
         her scan unaccounted for, and two lines account for one scan twice.\n\
         HOW: show the repos-read line once for a completed run that has a plan.\n\
         her queue's status region: {:?}",
        lines.len(),
        seen.status_region
    );
    let counted = repos_read_count(&lines[0]).unwrap_or_else(|| {
        panic!(
            "WHAT: her queue's line {:?} accounts for no number of repos read.\n\
             WHY: the line exists to tell her how many of her repos the scan read; \
             without a number it tells her nothing.\n\
             HOW: the line names the whole number of repos the scan read.",
            lines[0]
        )
    });
    assert_eq!(
        counted,
        whole_scan.len(),
        "WHAT: her queue says her scan read {counted} repos, when it read {} — {:?} — \
         across the runs of that scan.\n\
         WHY: the count is the only account she gets of what her scan did; counting \
         only the run that finished hides the repos the earlier runs read and tells her \
         repos of her choice were left unread, while a larger count names repos nobody \
         read, or repos of a scan before this one.\n\
         HOW: a run picking a paused plan up takes over the reads that plan already \
         holds, and a scan counts the repos of its own plan only.\n\
         the line she reads: {:?}",
        whole_scan.len(),
        whole_scan,
        lines[0]
    );
}

/// ```gherkin
/// @real-io
/// Scenario: A scan picked up after a pause accounts for every repo it read
///   Given Priya is signed in with her GitHub account verified
///   And she chose three of her repos
///   And her scan read one of them and paused because GitHub's request budget was used up
///   When she picks that scan up and it reads the other two and finishes
///   Then her queue carries exactly one line accounting for repos read
///   And that line says three repos read
/// ```
#[test]
fn a_scan_picked_up_after_a_pause_accounts_for_the_repos_of_the_whole_scan() {
    let world = rra_world();
    let mut browser = given_owner_verified(&world, Account::Priyaraman);
    given_saved_choice(&mut browser, Account::Priyaraman, &PRIYA_CHOICE_OF_THREE);
    let before_the_pause = given_scan_paused_part_way(&world, &mut browser);
    show_queue_evidence(
        "her queue, her scan paused because GitHub's budget was used up",
        &mut browser,
    );

    let after_picking_up = when_paused_scan_resumes(&world, &mut browser);

    show_repos_read("her scan before GitHub's budget ran out", &before_the_pause);
    show_repos_read("the run she picked up", &after_picking_up);
    let seen = show_queue_evidence("her queue, the scan she picked up over", &mut browser);
    let whole_scan = then_the_runs_read_her_chosen_repos_between_them(&[
        ("her scan before the pause", &before_the_pause),
        ("the run she picked up", &after_picking_up),
    ]);
    then_her_queue_accounts_for_the_whole_scan(&seen, &whole_scan);
}

/// ```gherkin
/// @real-io
/// Scenario: A scan picked up twice accounts for every repo it read along the way
///   Given Priya is signed in with her GitHub account verified
///   And she chose three of her repos
///   And her scan read one of them and paused because GitHub's request budget was used up
///   When she picks it up and it reads a second repo before the budget runs out again
///   And she picks it up once more and it reads the last repo and finishes
///   Then each run read a repo none of the others read, the three she chose between them
///   And her queue carries exactly one line accounting for repos read
///   And that line says three repos read
/// ```
#[test]
fn a_scan_picked_up_twice_accounts_for_every_repo_it_read_along_the_way() {
    let world = rra_world();
    let mut browser = given_owner_verified(&world, Account::Priyaraman);
    given_saved_choice(&mut browser, Account::Priyaraman, &PRIYA_CHOICE_OF_THREE);
    let before_the_pause = given_scan_paused_part_way(&world, &mut browser);
    show_queue_evidence(
        "her queue, her scan paused because GitHub's budget was used up",
        &mut browser,
    );

    let first_pick_up = when_she_picks_it_up_and_it_pauses_again(&world, &mut browser);
    let second_pick_up = when_paused_scan_resumes(&world, &mut browser);

    show_repos_read("her scan before GitHub's budget ran out", &before_the_pause);
    show_repos_read("the first time she picked it up", &first_pick_up);
    show_repos_read("the second time she picked it up", &second_pick_up);
    let seen = show_queue_evidence("her queue, the scan she picked up twice over", &mut browser);
    let whole_scan = then_the_runs_read_her_chosen_repos_between_them(&[
        ("her scan before the pause", &before_the_pause),
        ("the first run she picked up", &first_pick_up),
        ("the second run she picked up", &second_pick_up),
    ]);
    then_her_queue_accounts_for_the_whole_scan(&seen, &whole_scan);
}

// =============================================================================
// Her scan cut short by a store that could not save what it read
// =============================================================================

/// The repos of the whole scan, however often one of them had to be read:
/// the repos GitHub was asked to read across its runs, each named once.
fn repos_of_the_whole_scan(runs: &[(&str, &BTreeSet<String>)]) -> BTreeSet<String> {
    runs.iter()
        .flat_map(|(_, read)| (*read).iter().cloned())
        .collect()
}

/// The repos more than one run read: the reads her scan had to make again
/// because the first of them was never saved.
fn repos_read_again(runs: &[(&str, &BTreeSet<String>)]) -> BTreeSet<String> {
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut again: BTreeSet<String> = BTreeSet::new();
    for (_, read) in runs {
        for repo in *read {
            if !seen.insert(repo.clone()) {
                again.insert(repo.clone());
            }
        }
    }
    again
}

/// THEN her scan read the repos she chose, however often it had to read one
/// of them again: the repos of the whole scan.
fn then_her_scan_read_the_repos_she_chose(
    runs: &[(&str, &BTreeSet<String>)],
    chosen: &[Repo],
) -> BTreeSet<String> {
    let whole_scan = repos_of_the_whole_scan(runs);
    assert_eq!(
        whole_scan,
        set_of(chosen),
        "WHAT: the runs of her scan read {whole_scan:?} between them, not the repos she \
         chose, {:?}.\n\
         WHY: every claim below is about a scan that read the repos she chose; over any \
         other scan its count says nothing.\n\
         HOW: a scan reads the plan it froze from her choice, and the runs picking it up \
         finish that plan.",
        set_of(chosen)
    );
    whole_scan
}

/// THEN no repo the scan had already read and saved was read again by a
/// later run: the later runs read only what was still owed.
fn then_what_was_saved_was_not_read_again(
    saved: &BTreeSet<String>,
    later_runs: &[(&str, &BTreeSet<String>)],
) {
    for (moment, read) in later_runs {
        let read_again: Vec<&String> = read.intersection(saved).collect();
        assert!(
            read_again.is_empty(),
            "WHAT: {moment} read {read_again:?} again, which her scan had already read \
             and saved.\n\
             WHY: the run that was cut short saved those reads, so reading them again \
             spends her GitHub budget on work her scan had already paid for, and it \
             hides whether the count she is shown comes from the whole scan or only \
             from the run that finished.\n\
             HOW: a run picking a scan up takes over the reads its plan holds and reads \
             only the repos still owed."
        );
    }
}

/// THEN her scan ended `expected`, as the status she is shown says.
fn then_the_run_ended(status: &str, expected: &str, moment: &str) {
    assert_eq!(
        status, expected,
        "WHAT: {moment} ended {status:?}, not {expected:?}.\n\
         WHY: every claim below is about that run ending that way; on another ending \
         the scenario is not the one under test and its count says nothing.\n\
         HOW: build the scenario so the run ends {expected:?} before its queue is read."
    );
}

/// ```gherkin
/// @real-io
/// Scenario: A run cut short keeps the repos it had saved, and the run after it reads only the rest
///   Given Priya chose three of her repos
///   And her scan read one of them and paused because GitHub's request budget was used up
///   When her store stops saving that a repo was read, and she picks the run up
///   Then that run ends interrupted, having saved nothing new
///   When her store saves again and she picks the run up once more
///   Then the last run reads only the repos still owed, never the one already saved
///   And her queue says three repos read
/// ```
#[test]
fn a_run_cut_short_keeps_what_it_saved_and_the_run_after_it_reads_only_the_rest() {
    let world = rra_world();
    let mut browser = given_owner_verified(&world, Account::Priyaraman);
    given_saved_choice(&mut browser, Account::Priyaraman, &PRIYA_CHOICE_OF_THREE);
    let saved_before_the_pause = given_scan_paused_part_way(&world, &mut browser);
    show_repos_read(
        "her scan before GitHub's budget ran out",
        &saved_before_the_pause,
    );
    show_queue_evidence(
        "her queue, her scan paused because GitHub's budget was used up",
        &mut browser,
    );

    let world = when_her_store_stops_saving_reads(world);
    world.github.set_remaining(4_990);
    let (cut_short, read_while_cut_short) = when_she_picks_the_run_up(&world, &mut browser);
    show_repos_read(
        "the run she picked up while her store could not save a read",
        &read_while_cut_short,
    );
    show_queue_evidence("her queue, the run she picked up cut short", &mut browser);
    then_the_run_ended(
        &cut_short,
        "interrupted",
        "the run she picked up while her store could not save a read",
    );

    let world = when_her_store_saves_reads_again(world);
    let (last_run, read_by_the_last_run) = when_she_picks_the_run_up(&world, &mut browser);

    show_repos_read("the run she picked up last", &read_by_the_last_run);
    let seen = show_queue_evidence("her queue, her scan over", &mut browser);
    then_the_run_ended(&last_run, "completed", "the run she picked up last");
    then_what_was_saved_was_not_read_again(
        &saved_before_the_pause,
        &[
            ("the run that was cut short", &read_while_cut_short),
            ("the run she picked up last", &read_by_the_last_run),
        ],
    );
    let whole_scan = then_her_scan_read_the_repos_she_chose(
        &[
            ("her scan before the pause", &saved_before_the_pause),
            ("the run that was cut short", &read_while_cut_short),
            ("the run she picked up last", &read_by_the_last_run),
        ],
        &PRIYA_CHOICE_OF_THREE,
    );
    then_her_queue_accounts_for_the_whole_scan(&seen, &whole_scan);
}

/// ```gherkin
/// @real-io
/// Scenario: Repos her scan had to read again are still counted once each
///   Given Priya chose three of her repos and her store cannot save that a repo was read
///   When she scans, and picks the run up after her store saves again, and the budget runs out
///   And her store stops saving reads once more and she picks the run up
///   And her store saves again and she picks the run up for the last time
///   Then two of her repos were read by more than one run
///   And her queue says three repos read, not one count per read
/// ```
#[test]
fn repos_her_scan_had_to_read_again_are_counted_once_each() {
    let world = rra_world();
    let mut browser = given_owner_verified(&world, Account::Priyaraman);
    given_saved_choice(&mut browser, Account::Priyaraman, &PRIYA_CHOICE_OF_THREE);

    let world = when_her_store_stops_saving_reads(world);
    let (first, first_run) = when_scans(&world, &mut browser);
    then_the_run_ended(&first, "interrupted", "her first scan");

    let world = when_her_store_saves_reads_again(world);
    world.github.budget_runs_low_after_repos(1, 120);
    let (second, second_run) = when_she_picks_the_run_up(&world, &mut browser);
    then_the_run_ended(&second, "rate_limited", "the first run she picked up");

    let world = when_her_store_stops_saving_reads(world);
    world.github.set_remaining(4_990);
    let (third, third_run) = when_she_picks_the_run_up(&world, &mut browser);
    then_the_run_ended(&third, "interrupted", "the second run she picked up");

    let world = when_her_store_saves_reads_again(world);
    let (fourth, fourth_run) = when_she_picks_the_run_up(&world, &mut browser);

    show_repos_read("her first scan, her store saving no read", &first_run);
    show_repos_read("the first run she picked up", &second_run);
    show_repos_read("the second run she picked up", &third_run);
    show_repos_read("the last run she picked up", &fourth_run);
    let seen = show_queue_evidence("her queue, her scan over", &mut browser);
    then_the_run_ended(&fourth, "completed", "the last run she picked up");

    let runs = [
        ("her first scan", &first_run),
        ("the first run she picked up", &second_run),
        ("the second run she picked up", &third_run),
        ("the last run she picked up", &fourth_run),
    ];
    let read_again = repos_read_again(&runs);
    assert!(
        read_again.len() >= 2,
        "WHAT: only {read_again:?} was read by more than one run, where this scenario \
         needs at least two of her repos read again.\n\
         WHY: the claim below is that the number she is shown counts her repos and not \
         her scan's read attempts; with fewer repos read twice the two numbers are too \
         close to tell apart.\n\
         HOW: let her store lose the read of one repo, save the next, then lose another, \
         so two different repos are read again."
    );
    let whole_scan = then_her_scan_read_the_repos_she_chose(&runs, &PRIYA_CHOICE_OF_THREE);
    then_her_queue_accounts_for_the_whole_scan(&seen, &whole_scan);
}

// =============================================================================
// Her next scan counts its own repos
// =============================================================================

/// ```gherkin
/// @real-io
/// Scenario: A new scan accounts for its own repos only
///   Given Priya chose three of her repos and her scan read all three and finished
///   And her queue says three repos read
///   When she unticks one of them, leaving two, and scans again
///   Then that scan reads the two repos she now chose
///   And her queue says two repos read, not the five her two scans read between them
/// ```
#[test]
fn a_new_scan_accounts_for_its_own_repos_only() {
    let world = rra_world();
    let mut browser = given_owner_verified(&world, Account::Priyaraman);
    given_saved_choice(&mut browser, Account::Priyaraman, &PRIYA_CHOICE_OF_THREE);
    let read_by_her_first_scan = given_scanned(&world, &mut browser);
    show_repos_read("her first scan", &read_by_her_first_scan);
    let after_her_first_scan = show_queue_evidence("her queue, her first scan over", &mut browser);
    then_her_queue_accounts_for_the_whole_scan(&after_her_first_scan, &read_by_her_first_scan);

    given_saved_choice(&mut browser, Account::Priyaraman, &PRIYA_CHOICE_OF_TWO);
    let (status, read_by_her_second_scan) = when_scans(&world, &mut browser);

    show_repos_read("her second scan", &read_by_her_second_scan);
    let seen = show_queue_evidence("her queue, her second scan over", &mut browser);
    then_the_run_ended(&status, "completed", "her second scan");
    assert_eq!(
        read_by_her_second_scan,
        set_of(&PRIYA_CHOICE_OF_TWO),
        "WHAT: her second scan read {read_by_her_second_scan:?}, not the two repos she \
         had chosen by then.\n\
         WHY: the claim below is that the count she is shown is of the scan she is \
         looking at; over a scan that read something else the claim says nothing.\n\
         HOW: a new scan plans from the choice in force when it starts."
    );
    then_her_queue_accounts_for_the_whole_scan(&seen, &read_by_her_second_scan);
}

// =============================================================================
// Her scan cut short after it had saved a read of its own
// =============================================================================

/// THEN the run she picked up read only the repos her scan still owed her:
/// fewer repos than her queue accounts for, so the count can only come from
/// the run before it as well.
fn then_the_run_she_picked_up_read_only_what_was_owed(
    read_by_the_run_she_picked_up: &BTreeSet<String>,
    whole_scan: &BTreeSet<String>,
) {
    assert!(
        read_by_the_run_she_picked_up.len() < whole_scan.len(),
        "WHAT: the run she picked up read {read_by_the_run_she_picked_up:?}, as many \
         repos as her whole scan read ({whole_scan:?}).\n\
         WHY: the claim below is that her queue accounts for the whole scan and not \
         only for the run that finished; while the last run reads everything, both \
         answers give the same number and the claim cannot tell them apart.\n\
         HOW: the run before it must save a read of its own, and the run picking its \
         plan up must read only the repos still owed."
    );
    assert!(
        !read_by_the_run_she_picked_up.is_empty(),
        "WHAT: the run she picked up read nothing at all.\n\
         WHY: her scan was cut short with a repo still unread; a run that picks it up \
         and reads nothing leaves her scan unfinished, whatever the queue then says.\n\
         HOW: the run picking a plan up reads the repos of it still to read."
    );
}

/// ```gherkin
/// @real-io
/// Scenario: A run cut short after saving a read of its own keeps it, and the run after it reads only the rest
///   Given Priya chose two of her repos
///   And her store saves the first read of a run and refuses the ones after it
///   When she scans, and that run saves the first repo it reads and is cut short on the second
///   Then her queue offers to pick the run up and accounts for no repos read yet
///   When her store saves her reads again and she picks the run up
///   Then that run reads only the repo still owed, never the one her scan had saved
///   And her queue says two repos read, although the run that finished read one
/// ```
#[test]
fn a_run_cut_short_after_saving_a_read_of_its_own_keeps_it_for_the_run_after_it() {
    let world = rra_world();
    let mut browser = given_owner_verified(&world, Account::Priyaraman);
    given_saved_choice(&mut browser, Account::Priyaraman, &PRIYA_CHOICE_OF_TWO);

    let world = when_her_store_saves_only_the_first_read_of_a_run(world);
    let (cut_short, read_before_it_was_cut_short) = when_scans(&world, &mut browser);
    show_repos_read(
        "her scan, her store saving only the first read it made",
        &read_before_it_was_cut_short,
    );
    let while_cut_short = show_queue_evidence("her queue, her scan cut short", &mut browser);
    then_the_run_ended(&cut_short, "interrupted", "her scan");
    assert_eq!(
        read_before_it_was_cut_short,
        set_of(&PRIYA_CHOICE_OF_TWO),
        "WHAT: her scan read {read_before_it_was_cut_short:?}, not the two repos she \
         chose.\n\
         WHY: the claim below is about a run that saved the read of one repo and was \
         cut short on the next; over a run that read something else it says nothing.\n\
         HOW: let her scan read its first repo, save it, and be cut short saving the \
         second."
    );
    assert!(
        repos_read_lines(&while_cut_short.status_region).is_empty(),
        "WHAT: her queue accounts for repos read while her scan is unfinished: {:?}.\n\
         WHY: a run that did not finish has not done what a finished run promises, and \
         a count on it would tell her the scan was over.\n\
         HOW: account for repos read only on a run that finished.",
        repos_read_lines(&while_cut_short.status_region)
    );

    let world = when_her_store_saves_reads_again(world);
    let (last, read_by_the_run_she_picked_up) = when_she_picks_the_run_up(&world, &mut browser);

    show_repos_read("the run she picked up", &read_by_the_run_she_picked_up);
    let seen = show_queue_evidence("her queue, her scan over", &mut browser);
    then_the_run_ended(&last, "completed", "the run she picked up");
    let whole_scan = then_her_scan_read_the_repos_she_chose(
        &[
            (
                "her scan before it was cut short",
                &read_before_it_was_cut_short,
            ),
            ("the run she picked up", &read_by_the_run_she_picked_up),
        ],
        &PRIYA_CHOICE_OF_TWO,
    );
    then_the_run_she_picked_up_read_only_what_was_owed(&read_by_the_run_she_picked_up, &whole_scan);
    then_her_queue_accounts_for_the_whole_scan(&seen, &whole_scan);
}
