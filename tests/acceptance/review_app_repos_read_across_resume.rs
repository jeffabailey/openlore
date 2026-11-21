//! The queue accounts for a whole scan, not only for its last run.
//!
//! A scan that reads one of three repos, pauses because GitHub's request
//! budget is used up and is then picked up and read to its end has read three
//! repos. Her queue must say so: the run that picks a paused plan up takes
//! over the reads the paused run already saved, so the count is the count of
//! the scan, and a repo read twice (because the first read was never saved)
//! is still counted once.
//!
//! Driving port: the app's HTTP pages, opened by a signed-in owner through the
//! JS-less browser (the journey below). GitHub and the ATProto network are the
//! hermetic doubles.
//!
//! Two of the claims below live where the owner cannot see them one by one:
//! the rule that decides what a resuming run keeps (over every shape of
//! paused plan, L1-L3), and what the owner's store keeps of a frozen plan
//! (L4). Each reaches its production rule through ONE `sut_*` binding that
//! maps the production answer onto the view types declared here — the view
//! types, and the laws asserted over them, are the contract. Where the
//! contract the design declares is not built yet, the binding is written
//! against what production answers TODAY and the law fails on the answer, not
//! on a missing name: that failure is the defect this feature fixes. Each
//! binding carries the one call that replaces it once the declared answer
//! exists.
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
/// repo she imported. Accounts are bio-checked again by every run; imports
/// are not.
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

/// One repo the run picking a plan up keeps as its own, and whether it keeps
/// it as a read an earlier run of the scan already made.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct KeptRepo {
    position: usize,
    carries_an_earlier_read: bool,
}

/// What a run picking up a paused plan does with it.
#[derive(Debug, Clone, PartialEq, Eq)]
struct TakenOver {
    /// The plan it keeps as its own, in plan order.
    kept: Vec<KeptRepo>,
    /// The positions it reads now.
    reads_now: BTreeSet<usize>,
}

/// One repo of a frozen plan as the store is given it and reads it back.
#[derive(Debug, Clone, PartialEq, Eq)]
struct StoredRepo {
    repo: String,
    state: RowState,
}

/// The owner, as the store knows her.
const OWNER: &str = "did:plc:priya";

// =============================================================================
// Bindings — ONE call each into the rule under test
// =============================================================================

/// What a run picking up the paused plan `plan` does with it, given its own
/// account checks and the sources the owner still lists.
///
/// ONE call: `resume_plan`, whose verdict answers which repos the run keeps.
///
/// The verdict the design declares answers each kept repo with the status it
/// carries (`Resume::kept` → `KeptRow { position, status }`); until it does,
/// it answers with positions alone, every one of them a repo still to read,
/// which is what is mapped here. The one line that replaces this mapping once
/// the declared answer exists:
///
/// ```text
/// .map(|row| KeptRepo {
///     position: row.position,
///     carries_an_earlier_read: row.status == PlanRowStatus::Read,
/// })
/// ```
fn sut_taken_over(
    plan: &[PlannedRepo],
    checks: &BTreeMap<Source, Check>,
    no_longer_listed: &BTreeSet<Source>,
) -> TakenOver {
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

    TakenOver {
        kept: verdict
            .kept()
            .into_iter()
            .map(|position| KeptRepo {
                position,
                carries_an_earlier_read: false,
            })
            .collect(),
        reads_now: verdict.to_read.iter().copied().collect(),
    }
}

/// Freeze `rows` as the plan of run `run` of the owner's scan, each repo with
/// how far the scan already got with it.
///
/// ONE call: the store's plan writer, the single owner of plan-row writes.
///
/// The writer the design declares takes each row with its status
/// (`record_plan(…, rows: &[ScanPlanRow])`); until it does, it takes the
/// repos alone and the status each row carries is dropped here, as the store
/// drops it. The one line that replaces this mapping once the declared writer
/// exists:
///
/// ```text
/// store.record_plan(OWNER, run, &rows.iter().map(scan_plan_row).collect::<Vec<_>>())
/// ```
fn sut_freeze_plan(
    store: &adapter_review_store::ReviewStore,
    run: &str,
    rows: &[StoredRepo],
) -> Result<(), ports::ReviewStoreError> {
    use ports::{PlannedScanRepo, RepoSource, ScanRunPort};

    let repos: Vec<PlannedScanRepo> = rows
        .iter()
        .enumerate()
        .map(|(position, row)| PlannedScanRepo {
            repo_id: position as u64 + 1,
            full_name: row.repo.clone(),
            source: RepoSource::Account(1),
            outside_default: None,
        })
        .collect();
    store.record_plan(OWNER, run, &repos)
}

/// The frozen plan of the owner's latest run, as her store reads it back.
///
/// ONE call: the store's plan reader.
fn sut_frozen_plan(store: &adapter_review_store::ReviewStore) -> Vec<StoredRepo> {
    use ports::{PlanRowStatus, ScanPlanRead};

    store
        .latest_plan(OWNER)
        .expect("her store must read back the plan it was given")
        .into_iter()
        .map(|row| StoredRepo {
            repo: row.repo.full_name,
            state: match row.status {
                PlanRowStatus::Planned => RowState::StillToRead,
                PlanRowStatus::Read => RowState::AlreadyRead,
                PlanRowStatus::Skipped => RowState::Skipped,
            },
        })
        .collect()
}

// =============================================================================
// The paused plans a run can be given (L1-L3 generators)
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
    prop_oneof![Just(Check::Passed), Just(Check::Failed), Just(Check::Unfinished)]
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
/// read: the shape a resume must carry a read over, built in rather than
/// hoped for.
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

/// The positions the run picking the plan up keeps as reads already made.
fn carried_reads(taken_over: &TakenOver) -> BTreeSet<usize> {
    taken_over
        .kept
        .iter()
        .filter(|kept| kept.carries_an_earlier_read)
        .map(|kept| kept.position)
        .collect()
}

// =============================================================================
// What a run picking up a paused plan keeps of it
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    /// L1 — whatever this run's own account checks say, and whatever sources
    /// the owner has removed since, the run picking a paused plan up keeps as
    /// already read exactly the repos of that plan an earlier run read. A run
    /// that drops them (N1) leaves the scan accounting only for its last run.
    #[test]
    fn a_run_picking_a_paused_scan_up_keeps_every_read_it_already_made(
        paused in a_paused_scan_read_part_way(),
    ) {
        let taken_over = sut_taken_over(&paused.plan, &paused.checks, &paused.no_longer_listed);

        prop_assert_eq!(
            carried_reads(&taken_over),
            already_read(&paused.plan),
            "WHAT: the run picking this paused scan up keeps {:?} as already read, \
             when the repos its earlier run read are {:?}.\n\
             WHY: the count the owner is shown is the count of the repos her scan's \
             plan holds as read; a read dropped here is work she paid for and is never \
             told about, and a read invented here tells her of a repo nobody read.\n\
             HOW: the run picking a plan up keeps every repo already read as already \
             read, and nothing else — a failed or unfinished check, and a source she \
             has removed, change what is read NOW, never what was read then.\n\
             the paused scan: {:?}",
            carried_reads(&taken_over),
            already_read(&paused.plan),
            paused
        );
    }

    /// L2 — a repo kept as already read is never one the run reads now, so a
    /// repo of a scan is read once and counted once. A run that marks every
    /// repo it carries as read (N2) counts repos nobody read.
    #[test]
    fn a_repo_already_read_is_not_read_again_by_the_run_picking_the_scan_up(
        paused in a_paused_scan_read_part_way_by_an_account_checking_out(),
    ) {
        let taken_over = sut_taken_over(&paused.plan, &paused.checks, &paused.no_longer_listed);

        let carried = carried_reads(&taken_over);
        let read_again: Vec<usize> = carried.intersection(&taken_over.reads_now).copied().collect();
        prop_assert!(
            read_again.is_empty(),
            "WHAT: the run picking this paused scan up reads {read_again:?} again, \
             though it keeps those repos as already read.\n\
             WHY: reading a repo again spends the owner's GitHub budget on work her \
             scan already did, and a repo accounted for twice makes her count larger \
             than the scan it reports on.\n\
             HOW: a run reads the repos of the plan still to read, never the ones it \
             carries as read.\n\
             the paused scan: {:?}\n\
             what the run keeps: {:?}",
            paused,
            taken_over
        );
    }

    /// L3 — the run picking a paused plan up keeps part of that plan, in plan
    /// order, each repo once: a plan never grows from one run to the next,
    /// however many times a scan is picked up.
    #[test]
    fn the_plan_a_run_picks_up_never_grows(
        paused in a_paused_scan_holding_a_skip_and_a_removed_source(),
    ) {
        let taken_over = sut_taken_over(&paused.plan, &paused.checks, &paused.no_longer_listed);

        let positions: Vec<usize> = taken_over.kept.iter().map(|kept| kept.position).collect();
        let distinct: BTreeSet<usize> = positions.iter().copied().collect();
        let within_the_plan = positions.iter().all(|p| *p < paused.plan.len());
        let in_plan_order = positions.windows(2).all(|pair| pair[0] < pair[1]);
        prop_assert!(
            within_the_plan && in_plan_order && distinct.len() == positions.len(),
            "WHAT: the run picking this paused scan up keeps {positions:?}, which is \
             not a part of its {}-repo plan taken in plan order, each repo once.\n\
             WHY: the owner's scan is the choice she made when it started; a run that \
             keeps a repo outside that plan, or keeps one twice, reads and counts \
             repos her scan was never about.\n\
             HOW: the run keeps rows of the plan it picks up, in the order of that \
             plan, each position at most once.\n\
             the paused scan: {:?}",
            paused.plan.len(),
            paused
        );
    }
}

// =============================================================================
// What her store keeps of a frozen plan
// =============================================================================

/// Her store, empty, with one run of her scan started.
fn given_a_store_holding_a_started_run(run: &str) -> adapter_review_store::ReviewStore {
    use ports::ScanRunPort;

    let store = adapter_review_store::ReviewStore::open_in_memory(
        adapter_review_store::DataKey::generate(),
    )
    .expect("her store must open");
    store.start_scan(OWNER, run).expect("her run must start");
    store
}

fn stored(repo: &str, state: RowState) -> StoredRepo {
    StoredRepo {
        repo: repo.to_string(),
        state,
    }
}

/// L4 — a plan frozen with one repo already read reads back with that repo
/// still already read, the same repos in the same order.
#[test]
fn a_frozen_plan_keeps_which_of_its_repos_were_already_read() {
    let store = given_a_store_holding_a_started_run("picked-up");
    let frozen = vec![
        stored("priyaraman/tidepool", RowState::AlreadyRead),
        stored("priyaraman/quill-docs", RowState::StillToRead),
        stored("priyaraman/kelp", RowState::StillToRead),
    ];

    sut_freeze_plan(&store, "picked-up", &frozen).expect("her store must take the plan");

    let read_back = sut_frozen_plan(&store);
    println!("==== the plan her store keeps ====");
    for repo in &read_back {
        println!("  - {} — {:?}", repo.repo, repo.state);
    }
    println!("==== end of the plan her store keeps ====");
    assert_eq!(
        read_back, frozen,
        "WHAT: the plan her store was given reads back as {read_back:?}, not as the \
         {frozen:?} it was given.\n\
         WHY: the run picking a scan up freezes its plan with the reads the paused run \
         already made; a store that forgets which of them were read loses the whole \
         account of the scan and shows her only the reads of the last run.\n\
         HOW: write each plan row with the status it is given, instead of writing \
         every row as still to read."
    );
}

/// L4 — a plan frozen with none of its repos read reads back with none read:
/// the writer carries the status it is given, it does not invent one.
#[test]
fn a_frozen_plan_of_repos_none_read_keeps_them_all_still_to_read() {
    let store = given_a_store_holding_a_started_run("first-run");
    let frozen = vec![
        stored("priyaraman/tidepool", RowState::StillToRead),
        stored("priyaraman/quill-docs", RowState::StillToRead),
        stored("priyaraman/kelp", RowState::StillToRead),
    ];

    sut_freeze_plan(&store, "first-run", &frozen).expect("her store must take the plan");

    let read_back = sut_frozen_plan(&store);
    assert_eq!(
        read_back, frozen,
        "WHAT: a plan of repos none of which were read reads back as {read_back:?}.\n\
         WHY: a repo counted as read before her scan read it is a repo she is told \
         about and never gets suggestions from.\n\
         HOW: write each plan row with the status it is given, and no other."
    );
}

/// L4 — a plan frozen for a run her store no longer holds leaves nothing
/// behind: no row of a run that is gone, and her latest run keeps its own.
#[test]
fn a_plan_frozen_for_a_run_her_store_no_longer_holds_leaves_nothing_behind() {
    let store = given_a_store_holding_a_started_run("picked-up");

    sut_freeze_plan(
        &store,
        "forgotten-run",
        &[
            stored("priyaraman/tidepool", RowState::AlreadyRead),
            stored("priyaraman/kelp", RowState::StillToRead),
        ],
    )
    .expect("her store must answer a plan for a run it no longer holds");

    let read_back = sut_frozen_plan(&store);
    assert!(
        read_back.is_empty(),
        "WHAT: a plan written for a run her store no longer holds left {read_back:?} \
         behind.\n\
         WHY: she asked for her data to be forgotten; rows written for a run that is \
         gone keep her repos in the store and can be counted as a run's own.\n\
         HOW: write a run's plan rows only while the store still holds that run."
    );
}

// =============================================================================
// Her scan, paused for GitHub's budget and picked up
// =============================================================================

/// THEN her scan read each of the three repos she chose, once: one before it
/// paused and the other two when she picked it up.
fn then_her_scan_read_each_chosen_repo_once(
    before_the_pause: &BTreeSet<String>,
    after_picking_up: &BTreeSet<String>,
) -> BTreeSet<String> {
    let whole_scan: BTreeSet<String> = before_the_pause
        .union(after_picking_up)
        .cloned()
        .collect();
    assert!(
        before_the_pause.is_disjoint(after_picking_up),
        "WHAT: the run she picked up read {:?} again, which her paused run had already \
         read.\n\
         WHY: the count below is about a scan that read each of her repos once; a repo \
         read twice here would make a count of three true for the wrong reason.\n\
         HOW: the run picking a scan up reads the repos of its plan not yet read.",
        before_the_pause
            .intersection(after_picking_up)
            .collect::<Vec<_>>()
    );
    assert_eq!(
        whole_scan,
        set_of(&PRIYA_CHOICE_OF_THREE),
        "WHAT: her scan, before and after the pause, read {whole_scan:?}, not the three \
         repos she chose.\n\
         WHY: every claim below is about a scan that read her three chosen repos; over \
         any other scan a count of three says nothing.\n\
         HOW: a scan reads the plan it froze from her choice, and the run picking it up \
         finishes that plan."
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
         one before GitHub's budget ran out and the rest when she picked the run up.\n\
         WHY: the count is the only account she gets of what her scan did; counting \
         only the run that finished hides the repos the paused run read and makes her \
         think repos were left unread, and a larger count names repos nobody read.\n\
         HOW: the run picking a paused plan up takes over the reads that plan already \
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
    let whole_scan = then_her_scan_read_each_chosen_repo_once(&before_the_pause, &after_picking_up);
    then_her_queue_accounts_for_the_whole_scan(&seen, &whole_scan);
}
