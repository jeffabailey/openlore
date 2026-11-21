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

/// What a run picking up a paused plan does with it: which of its repos it
/// reads now, and which of them this scan will never read.
#[derive(Debug, Clone, PartialEq, Eq)]
struct PickedUp {
    reads_now: Vec<usize>,
    never_reads: Vec<usize>,
}

// =============================================================================
// The binding — ONE call into the rule under test
// =============================================================================

/// What a run picking up the paused plan `plan` does with it, given its own
/// account checks and the sources the owner still lists.
///
/// ONE call: `resume_plan`, the single owner of that verdict. It is read
/// through the two answers this feature leaves as they are — the repos the
/// run reads now, and the repos it will never read — so no line of this
/// binding moves when the verdict starts naming the repos already read.
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
        reads_now: verdict.to_read.clone(),
        never_reads: verdict.skipped.iter().map(|(position, _)| *position).collect(),
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

    /// L2 — whatever this run's own account checks say, a repo of the plan an
    /// earlier run already read is never among the repos it reads now: the
    /// scan reads each of her repos once, and so counts it once.
    #[test]
    fn a_run_picking_a_paused_scan_up_does_not_read_a_repo_the_scan_already_read(
        paused in a_paused_scan_read_part_way_by_an_account_checking_out(),
    ) {
        let picked_up = sut_picked_up(&paused.plan, &paused.checks, &paused.no_longer_listed);

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

        let mut accounted: Vec<usize> = picked_up
            .reads_now
            .iter()
            .chain(&picked_up.never_reads)
            .copied()
            .collect();
        let distinct: BTreeSet<usize> = accounted.iter().copied().collect();
        accounted.sort_unstable();
        let within_the_plan = accounted.iter().all(|p| *p < paused.plan.len());
        let in_plan_order = picked_up.reads_now.windows(2).all(|pair| pair[0] < pair[1])
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
         between the run that paused and the runs that picked it up.\n\
         WHY: the count is the only account she gets of what her scan did; counting \
         only the run that finished hides the repos the earlier runs read and tells her \
         repos of her choice were left unread, and a larger count names repos nobody \
         read.\n\
         HOW: a run picking a paused plan up takes over the reads that plan already \
         holds, so the count is the count of the whole scan.\n\
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

    let first_pick_up = when_she_picks_it_up_and_it_pauses_again(&world, &mut browser);
    let second_pick_up = when_paused_scan_resumes(&world, &mut browser);

    show_repos_read("her scan before GitHub's budget ran out", &before_the_pause);
    show_repos_read("the first time she picked it up", &first_pick_up);
    show_repos_read("the second time she picked it up", &second_pick_up);
    let seen = show_queue_evidence(
        "her queue, the scan she picked up twice over",
        &mut browser,
    );
    let whole_scan = then_the_runs_read_her_chosen_repos_between_them(&[
        ("her scan before the pause", &before_the_pause),
        ("the first run she picked up", &first_pick_up),
        ("the second run she picked up", &second_pick_up),
    ]);
    then_her_queue_accounts_for_the_whole_scan(&seen, &whole_scan);
}
