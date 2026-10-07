//! indexer-deployment — scheduled passes inside `serve` (US-IXD-002, ADR-080,
//! ADR-078 amended): search keeps answering while a pass writes, passes never
//! overlap, the `trigger` exit code is the pass's exit code (0/2/3, 4 when
//! `serve` is unreachable), every pass ends with exactly one `pass_summary`,
//! and every way a pass can break leaves `serve` honest (H2, M1, M3, M4).
//!
//! Layer 3 (real `serve` + real `trigger` + real `index.duckdb`; PLC and PDSes
//! faked). Example-only, every failure mode a NAMED scenario (Mandates 9, 11).
//! The pure decisions behind them (exit-code precedence, deadline outcome,
//! single-flight transitions, health projection) are property-tested at layer 2
//! in `indexer_deployment_core.rs`.
//!
//! ## The pass runner as a state machine (C2a; ADR-080 §3-§7)
//!
//! ```text
//! IDLE ──run_pass──▶ RUNNING(pass_id) ──ends 0/2/3──▶ IDLE      (one pass_summary, trigger exits with it)
//! RUNNING ──run_pass──▶ RUNNING (reply busy; trigger exits 0, indexer.trigger.coalesced)   [never a 2nd pass]
//! RUNNING ──panic──▶ IDLE (summary exit 2 pass_panicked)          [slot freed]
//! RUNNING ──deadline──▶ IDLE (summary exit 2 pass_deadline_exceeded; committed upserts kept)
//! RUNNING ──store poisoned──▶ UNUSABLE ──▶ serve exits 2           [/healthz never 200, search never empty 200]
//! RUNNING ──serve killed──▶ (trigger exits 4) ──restart──▶ IDLE    [committed claims searchable]
//! (no serve) ──run_pass──▶ trigger exits 4, no pass, no store opened
//! ```
//!
//! Story line (Pillar 2): PS-10 → PS-13 → PS-25 chain from
//! `journey::given_authors_publish_on_their_own_pdses` + a live index.
//!
//! All scenarios are `#[ignore]`d at DISTILL hand-off.
//
// SCAFFOLD: true
#![cfg(unix)]

mod support;

#[path = "support/indexer_network.rs"]
mod indexer_network;

#[path = "support/indexer_live.rs"]
mod indexer_live;

use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

use indexer_live::journey::*;
use indexer_live::*;
use indexer_network::{Author, Host, IndexerWorld, ListingPosture};
use support::state_delta::{assert_state_delta, Delta};

/// One second: the search budget during a pass (NFR-IXD-3, AC-002.2).
const SEARCH_BUDGET: Duration = Duration::from_secs(1);

/// GIVEN the index is live listing Priya and Dmitri, and Dmitri's PDS answers
/// slowly (4 s), so a pass stays in progress long enough to act during it.
fn given_a_live_index_whose_pass_takes_a_while(world: &IndexerWorld) -> LiveIndex {
    world.host_answers(
        Host::VolkovDev,
        ListingPosture::Slow(Duration::from_secs(4)),
    );
    given_the_index_is_live_listing(world, &[Author::Priya, Author::Dmitri])
}

/// GIVEN a crowd of 40 authors on 4 PDS hosts, each host answering in 2 s,
/// listed in a live index (a pass of ~20 s with many store writes).
fn given_a_crowd_index_with_a_long_write_heavy_pass() -> (IndexerWorld, LiveIndex) {
    let (world, by_host) = IndexerWorld::crowd(40, 4);
    for host in by_host.keys() {
        world
            .net
            .set_listing_posture(host, ListingPosture::Slow(Duration::from_secs(2)));
    }
    let dids = world.configured_dids();
    let refs: Vec<&str> = dids.iter().map(String::as_str).collect();
    let live = LiveIndex::deploy(&world, Deployment::listing(&refs));
    (world, live)
}

/// WHEN a pass has visibly started inside `serve` (its list loaded).
fn when_the_pass_is_underway(live: &LiveIndex) {
    live.wait_for_pass_event("indexer.config.loaded", Duration::from_secs(10))
        .unwrap_or_else(|| {
            panic!(
                "MISSING_FUNCTIONALITY: no pass started inside serve (no indexer.config.loaded \
                 with a pass_id)\n{}",
                live.dump()
            )
        });
}

// =============================================================================
// Search keeps answering (AC-002.2, AC-002.3)
// =============================================================================

/// PS-10
/// ```gherkin
/// @US-IXD-002 @AC-002.2 @NFR-IXD-3 @real-io @driving_port @contract-shape:unbounded-preservation
/// Scenario: Search keeps answering within a second while a pass runs
///   Given the index is live listing Priya and Dmitri, and Dmitri's PDS answers slowly
///   When the timer fires and Maria searches for reproducible builds while the pass runs
///   Then every one of her searches answers within 1 second with no error
///   And the pass completes
/// ```
#[test]
fn search_keeps_answering_within_a_second_while_a_pass_runs() {
    let world = given_authors_publish_on_their_own_pdses();
    let live = given_a_live_index_whose_pass_takes_a_while(&world);

    let pending = live.timer_fires_in_background();
    when_the_pass_is_underway(&live);
    let mut searches = 0;
    while !pending.is_finished() {
        let started = Instant::now();
        let out = live.maria_searches(&world, &["--object", REPRODUCIBLE_BUILDS]);
        let took = started.elapsed();
        assert_eq!(out.status, 0, "{}\n{}", out.stdout, out.stderr);
        assert!(
            !out.stdout.contains("Network index unavailable"),
            "no error during the pass\n{}",
            out.stdout
        );
        assert!(
            took <= SEARCH_BUDGET,
            "a search during the pass took {took:?}"
        );
        searches += 1;
    }
    let pass = pending.finish();

    assert!(
        searches >= 3,
        "Maria searched during the pass ({searches} times)"
    );
    assert_one_summary_matching(&live, &pass, PassExit::Completed);
}

/// PS-11
/// ```gherkin
/// @US-IXD-002 @AC-002.3 @NFR-IXD-7 @ADR-080 @real-io @driving_port @contract-shape:bounded-change
/// Scenario: A pass is never refused because search is busy
///   Given 40 authors on 4 slow PDS hosts are listed in the live index
///   When the timer fires while anyone sends 100 searches over 10 seconds
///   Then the pass completes with exit 0 and indexes all 40 authors
///   And every search answers within 1 second
/// ```
#[test]
fn a_pass_is_never_refused_because_search_is_busy() {
    let (_world, live) = given_a_crowd_index_with_a_long_write_heavy_pass();

    let pending = live.timer_fires_in_background();
    let mut slow_or_failed = Vec::new();
    for i in 0..100 {
        let (status, _, took) = live.public_search("object", REPRODUCIBLE_BUILDS);
        if status != 200 || took > SEARCH_BUDGET {
            slow_or_failed.push((i, status, took));
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    let pass = pending.finish();

    let summary = assert_one_summary_matching(&live, &pass, PassExit::Completed);
    assert_eq!(summary["own_pds"].as_u64(), Some(40), "{summary}");
    assert!(
        slow_or_failed.is_empty(),
        "searches over budget or failed: {slow_or_failed:?}"
    );
}

/// PS-12
/// ```gherkin
/// @US-IXD-002 @AC-002.2 @M3 @B14 @real-io @contract-shape:unbounded-preservation
/// Scenario: Searches overlapping the end of a pass still answer within a second
///   Given 40 authors on 4 slow PDS hosts are listed in the live index
///   When the timer fires and searches run back to back until the pass has ended
///   Then every search answers within 1 second, including those during the pass's final save
/// ```
#[test]
fn searches_overlapping_the_end_of_a_pass_still_answer_within_a_second() {
    let (_world, live) = given_a_crowd_index_with_a_long_write_heavy_pass();

    let pending = live.timer_fires_in_background();
    let mut timings: Vec<(Instant, Duration, u16)> = Vec::new();
    while !pending.is_finished() {
        let started = Instant::now();
        let (status, _, took) = live.public_search("object", REPRODUCIBLE_BUILDS);
        timings.push((started, took, status));
    }
    let pass_ended = Instant::now();
    let pass = pending.finish();

    assert_one_summary_matching(&live, &pass, PassExit::Completed);
    assert!(
        timings
            .iter()
            .any(|(at, _, _)| pass_ended.duration_since(*at) <= Duration::from_millis(500)),
        "a search was in flight during the last half-second of the pass"
    );
    let bad: Vec<_> = timings
        .iter()
        .filter(|(_, took, status)| *status != 200 || *took > SEARCH_BUDGET)
        .collect();
    assert!(
        bad.is_empty(),
        "{} of {} searches over budget or failed: {bad:?}",
        bad.len(),
        timings.len()
    );
}

// =============================================================================
// Single-flight (AC-002.4)
// =============================================================================

/// PS-13
/// ```gherkin
/// @US-IXD-002 @AC-002.4 @ADR-080 @C7c @real-io @driving_port @contract-shape:bounded-change
/// Scenario: Passes never overlap
///   Given a pass is still running when the timer fires again
///   When the second trigger arrives
///   Then it exits 0 reporting it joined the running pass
///   And only one pass summary is written for that pass
///   And the next timer firing after it ends starts a new pass
/// ```
#[test]
fn passes_never_overlap() {
    let world = given_authors_publish_on_their_own_pdses();
    let live = given_a_live_index_whose_pass_takes_a_while(&world);
    let first = live.timer_fires_in_background();
    when_the_pass_is_underway(&live);

    let second = live.timer_fires();

    assert_eq!(
        second.status,
        PassExit::Completed.code(),
        "{}",
        second.dump()
    );
    let coalesced = second.events_named("indexer.trigger.coalesced");
    assert_eq!(
        coalesced.len(),
        1,
        "the second trigger joined the running pass\n{}",
        second.dump()
    );
    let first = first.finish();
    let summary = assert_one_summary_matching(&live, &first, PassExit::Completed);
    assert_eq!(
        coalesced[0]["running_pass_id"],
        summary["pass_id"],
        "{}",
        second.dump()
    );
    assert_eq!(
        live.pass_summaries().len(),
        1,
        "no second pass ran\n{}",
        live.dump()
    );

    let third = live.timer_fires();

    let next = assert_one_summary_matching(&live, &third, PassExit::Completed);
    assert_ne!(
        next["pass_id"], summary["pass_id"],
        "a new pass, with a new pass_id"
    );
    assert_eq!(live.pass_summaries().len(), 2);
}

/// PS-25
/// ```gherkin
/// @US-IXD-002 @C4a @real-io @contract-shape:unbounded-preservation
/// Scenario: A pass with nothing new on the network changes nothing Maria sees
///   Given a pass already indexed Priya's and Dmitri's claims
///   When the timer fires again and nothing changed on their PDSes
///   Then the pass completes and purges no one
///   And Maria's results for every author are exactly as before
/// ```
#[test]
fn a_pass_with_nothing_new_on_the_network_changes_nothing_maria_sees() {
    let world = given_authors_publish_on_their_own_pdses();
    let live = given_the_index_is_live_listing(&world, &[Author::Priya, Author::Dmitri]);
    given_a_pass_indexed(&live);
    let snapshot = |live: &LiveIndex| -> HashMap<String, Vec<String>> {
        [Author::Priya, Author::Dmitri, Author::Jeff, Author::Tomas]
            .iter()
            .map(|a| {
                let mut blocks: Vec<String> = maria_looks_up(&world, live, *a)
                    .into_iter()
                    .map(|r| r.block.join("\n"))
                    .collect();
                blocks.sort();
                (format!("maria.results.by_author[{}]", a.did()), blocks)
            })
            .collect()
    };
    let before = snapshot(&live);

    let pass = live.timer_fires();

    let summary = assert_one_summary_matching(&live, &pass, PassExit::Completed);
    assert_eq!(summary["purged_authors"].as_u64(), Some(0), "{summary}");
    let after = snapshot(&live);
    let universe: HashSet<String> = before.keys().cloned().collect();
    assert_state_delta(&before, &after, &universe, &Delta::new());
}

// =============================================================================
// Exit codes and summaries (B4; inputs to alarms A1/A2)
// =============================================================================

/// PS-14
/// ```gherkin
/// @US-IXD-002 @US-IXD-004 @AC-004.1 @AC-004.2 @AC-004.3 @B4 @KPI-IXD-5 @kpi @real-io @error
/// @contract-shape:bounded-change
/// Scenario Outline: The timer sees the pass's own exit code and exactly one summary
///   Given the live index lists <list>
///   When the timer fires
///   Then trigger exits <code>
///   And serve wrote exactly one pass summary with exit code <code> and a pass id
///   Examples:
///     | list                                | code | cause               |
///     | Priya (her PDS answers)             | 0    |                     |
///     | Priya and Dmitri (Dmitri's PDS 502) | 0    |                     |
///     | only a DID nobody can resolve       | 3    |                     |
///     | "did:plc:therrera2v6w,tomas"        | 2    | repo_dids_malformed |
/// ```
#[test]
fn the_timer_sees_the_pass_s_own_exit_code_and_exactly_one_summary() {
    type Arrange = fn(&IndexerWorld);
    let cases: Vec<(&str, &str, Arrange, PassExit, Option<FailureCause>)> = vec![
        (
            "Priya answers",
            Author::Priya.did(),
            |_| {},
            PassExit::Completed,
            None,
        ),
        (
            "partial skip",
            "did:plc:priyaraman7x2k,did:plc:dvolkov3m9q",
            |w| w.host_answers(Host::VolkovDev, ListingPosture::Status(502)),
            PassExit::Completed,
            None,
        ),
        (
            "total outage",
            Author::Ghost.did(),
            |_| {},
            PassExit::TotalOutage,
            None,
        ),
        (
            "malformed list",
            "did:plc:therrera2v6w,tomas",
            |_| {},
            PassExit::Failed,
            Some(FailureCause::ListMalformed),
        ),
    ];
    for (label, list, arrange, exit, cause) in cases {
        let world = given_authors_publish_on_their_own_pdses();
        arrange(&world);
        let live = LiveIndex::deploy(&world, Deployment::with_list_text(list));

        let pass = live.timer_fires();

        let summary = assert_one_summary_matching(&live, &pass, exit);
        if let Some(cause) = cause {
            assert_eq!(summary["cause"], cause.token(), "[{label}] {summary}");
        }
        assert!(!summary["pass_id"].is_null(), "[{label}] {summary}");
    }
}

/// PS-15
/// ```gherkin
/// @US-IXD-002 @ADR-080 @C4b @error @real-io @contract-shape:unbounded-preservation
/// Scenario: The timer firing while serve is down reports that no pass ran
///   Given the index container is stopped
///   When the timer fires
///   Then trigger exits 4 and reports serve unreachable at its socket
///   And no pass summary is written and no index is opened by the timer
/// ```
#[test]
fn the_timer_firing_while_serve_is_down_reports_that_no_pass_ran() {
    let world = given_authors_publish_on_their_own_pdses();
    let live = given_the_index_is_live_listing(&world, &[Author::Priya]);
    let untouched_index = world.env.home.join("never-opened").join("index.duckdb");
    let mut env: Vec<(String, String)> = live.environment().to_vec();
    env.retain(|(k, _)| k != var::INDEX_PATH);
    env.push((
        var::INDEX_PATH.to_string(),
        untouched_index.display().to_string(),
    ));
    let socket = live.socket().display().to_string();
    live.stop();

    let pass = timer_fires_with(&env);

    assert_eq!(
        pass.status,
        PassExit::ServeUnreachable.code(),
        "{}",
        pass.dump()
    );
    let unreachable = pass.events_named("indexer.trigger.unreachable");
    assert_eq!(unreachable.len(), 1, "{}", pass.dump());
    assert_eq!(unreachable[0]["socket"], socket.as_str(), "{}", pass.dump());
    assert!(
        pass.events_named("indexer.ingest.pass_summary").is_empty(),
        "{}",
        pass.dump()
    );
    assert!(!untouched_index.exists(), "the timer never opens an index");
}

/// PS-16
/// ```gherkin
/// @US-IXD-002 @M4 @ADR-080-9 @C6a @real-io @contract-shape:unbounded-preservation
/// Scenario: The timer needs nothing but the control socket to start a pass
///   Given the index is live listing Priya
///   When the timer fires with only the socket setting, beside a broken DID list setting and an unusable index path
///   Then the pass runs and trigger exits 0
///   And the timer itself never loads or refuses a configuration
/// ```
#[test]
fn the_timer_needs_nothing_but_the_control_socket_to_start_a_pass() {
    let world = given_authors_publish_on_their_own_pdses();
    let live = given_the_index_is_live_listing(&world, &[Author::Priya]);
    let socket = live.socket().display().to_string();

    let pass = live.trigger_with_only(&[
        (var::CONTROL_SOCKET, socket.as_str()),
        (var::REPO_DIDS, "not-a-did"),
        (var::INDEX_PATH, "/nonexistent/openlore/index.duckdb"),
    ]);

    assert_one_summary_matching(&live, &pass, PassExit::Completed);
    assert!(pass.startup_refusal().is_none(), "{}", pass.dump());
    assert!(
        pass.config_loaded().is_none(),
        "the client never loads a configuration\n{}",
        pass.dump()
    );
    then_maria_finds(&world, &live, Author::Priya, 2);
}

// =============================================================================
// Every way a pass can break leaves serve honest (H2, M1)
// =============================================================================

/// PS-17
/// ```gherkin
/// @US-IXD-002 @US-IXD-004 @AC-004.2 @B4 @error @real-io @contract-shape:bounded-change
/// Scenario: A pass that cannot store a claim ends with exit 2 and search keeps answering
///   Given the index cannot store Dmitri's claims
///   When the timer fires
///   Then trigger exits 2 and the one pass summary names the store failure
///   And Maria still finds Priya's claims and the index stays up
/// ```
#[test]
fn a_pass_that_cannot_store_a_claim_ends_with_exit_2_and_search_keeps_answering() {
    let world = given_authors_publish_on_their_own_pdses();
    world.index_cannot_store_claims_of(Author::Dmitri);
    let live = given_the_index_is_live_listing(&world, &[Author::Priya, Author::Dmitri]);

    let pass = live.timer_fires();

    let summary = assert_one_summary_matching(&live, &pass, PassExit::Failed);
    assert_eq!(
        summary["cause"],
        FailureCause::UpsertFailed.token(),
        "{summary}"
    );
    then_maria_finds(&world, &live, Author::Priya, 2);
    assert_eq!(live.health().0, 200, "{}", live.dump());
    assert_eq!(live.times_started(), 1, "serve kept running");
}

/// PS-18
/// ```gherkin
/// @US-IXD-002 @H2 @ADR-080-7 @error @real-io @contract-shape:bounded-change
/// Scenario: A pass that crashes frees the runner for the next pass
///   Given the first pass will crash
///   When the timer fires
///   Then trigger exits 2 and the one pass summary says the pass crashed
///   And when the timer fires again a new pass runs and completes instead of reporting busy
/// ```
#[test]
fn a_pass_that_crashes_frees_the_runner_for_the_next_pass() {
    let world = given_authors_publish_on_their_own_pdses();
    let live = LiveIndex::deploy(
        &world,
        Deployment::listing(&dids(&[Author::Priya])).with_fault(Fault::FirstPassPanics),
    );

    let crashed = live.timer_fires();

    let summary = assert_one_summary_matching(&live, &crashed, PassExit::Failed);
    assert_eq!(
        summary["cause"],
        FailureCause::PassPanicked.token(),
        "{summary}"
    );
    let next = live.timer_fires();
    assert!(
        next.events_named("indexer.trigger.coalesced").is_empty(),
        "{}",
        next.dump()
    );
    let summary = assert_one_summary_matching(&live, &next, PassExit::Completed);
    assert_eq!(summary["own_pds"].as_u64(), Some(1), "{summary}");
    then_maria_finds(&world, &live, Author::Priya, 2);
}

/// PS-19
/// ```gherkin
/// @US-IXD-002 @M1 @ADR-080 @C7b @error @real-io @slow @contract-shape:bounded-change
/// Scenario: A pass that overruns its deadline ends with exit 2 and keeps what it saved
///   Given the pass deadline is 60 seconds and Sam's PDS never answers (per-author limit 120 seconds)
///   And Priya is listed before Sam
///   When the timer fires
///   Then trigger exits 2 after about a minute and the summary says the deadline passed
///   And Maria finds Priya's claims saved before the deadline
/// ```
#[test]
fn a_pass_that_overruns_its_deadline_ends_with_exit_2_and_keeps_what_it_saved() {
    let world = IndexerWorld::configured_with(&[Author::Priya, Author::Sam]);
    world.publishes_self_attested(
        Author::Priya,
        &[
            indexer_network::claims::CARGO_PIN_REPRODUCIBLE_BUILDS,
            indexer_network::claims::CARGO_PIN_DEPENDENCY_PINNING,
        ],
    );
    world.publishes_self_attested(Author::Sam, &[indexer_network::claims::SAM_SLOW_CLAIM]);
    world.host_answers(Host::SlowhostExample, ListingPosture::Hang);
    let live = LiveIndex::deploy(
        &world,
        Deployment::listing(&dids(&[Author::Priya, Author::Sam]))
            .setting(var::PASS_DEADLINE_SECS, "60")
            .setting(var::PER_DID_TIMEOUT, "120"),
    );

    let pass = live.timer_fires();

    let summary = assert_one_summary_matching(&live, &pass, PassExit::Failed);
    assert_eq!(
        summary["cause"],
        FailureCause::PassDeadlineExceeded.token(),
        "{summary}"
    );
    assert!(
        pass.elapsed >= Duration::from_secs(58) && pass.elapsed < Duration::from_secs(110),
        "ended at the deadline, not the per-author limit: {:?}",
        pass.elapsed
    );
    then_maria_finds(&world, &live, Author::Priya, 2);
}

/// PS-20
/// ```gherkin
/// @US-IXD-002 @US-IXD-004 @H2 @ADR-080-7 @ADR-083 @AC-004.2 @error @real-io @contract-shape:bounded-change
/// Scenario: A broken index makes serve stop instead of answering falsely
///   Given the index's store breaks during the next pass
///   When the timer fires
///   Then serve reports the store unusable and exits 2 so it can be restarted
///   And until it exits it never reports healthy and never answers a search with an empty success
/// ```
#[test]
fn a_broken_index_makes_serve_stop_instead_of_answering_falsely() {
    let world = given_authors_publish_on_their_own_pdses();
    let mut live = LiveIndex::deploy(
        &world,
        Deployment::listing(&dids(&[Author::Priya])).with_fault(Fault::StorePoisonedAtNextPass),
    );

    let pending = live.timer_fires_in_background();
    let unusable = (0..500).find_map(|_| {
        std::thread::sleep(Duration::from_millis(20));
        live.events_named("indexer.store.unusable")
            .into_iter()
            .next()
    });
    let unusable = unusable.unwrap_or_else(|| {
        panic!(
            "MISSING_FUNCTIONALITY: no indexer.store.unusable\n{}",
            live.dump()
        )
    });
    assert_eq!(unusable["reason"], "mutex_poisoned", "{unusable}");
    let mut answers = Vec::new();
    for _ in 0..10 {
        answers.push(("health", live.health().0));
        answers.push((
            "search",
            live.public_search("object", REPRODUCIBLE_BUILDS).0,
        ));
    }
    let exit = live.exits_within(Duration::from_secs(10));
    let pass = pending.finish();

    assert_eq!(exit, Some(2), "serve exits 2\n{}", live.dump());
    assert!(
        answers.iter().all(|(_, status)| *status != 200),
        "never a 200 while the store is unusable: {answers:?}"
    );
    assert_ne!(pass.status, PassExit::Completed.code(), "{}", pass.dump());
}

/// PS-21
/// ```gherkin
/// @US-IXD-001 @ADR-083-2 @error @real-io @contract-shape:unbounded-preservation
/// Scenario: A search that cannot read the index is reported as unavailable, never as no results
///   Given the index cannot be read when anyone searches
///   When Maria searches for reproducible builds
///   Then the public search answers with a server error carrying no query text
///   And Maria is told the network index is unavailable, not that nothing matched
/// ```
#[test]
fn a_search_that_cannot_read_the_index_is_reported_as_unavailable_never_as_no_results() {
    let world = given_authors_publish_on_their_own_pdses();
    let live = LiveIndex::deploy(
        &world,
        Deployment::listing(&dids(&[Author::Priya])).with_fault(Fault::SearchCannotReadTheStore),
    );

    let (status, body, _) = live.public_search("object", REPRODUCIBLE_BUILDS);
    let out = live.maria_searches(&world, &["--object", REPRODUCIBLE_BUILDS]);

    assert_eq!(status, 500, "{body}\n{}", live.dump());
    assert!(
        !body.contains("reproducible-builds"),
        "no query text in the error: {body}"
    );
    assert!(
        out.stdout.contains("Network index unavailable"),
        "Maria is told the index is unavailable\n{}\n{}",
        out.stdout,
        out.stderr
    );
}

// =============================================================================
// Health and restarts (ADR-083 §2, ADR-080 Earned Trust)
// =============================================================================

/// PS-22
/// ```gherkin
/// @US-IXD-002 @US-IXD-005 @ADR-083-2 @KPI-IXD-3 @kpi @real-io @contract-shape:bounded-change
/// Scenario: The health response shows when the last good pass ended, and only that
///   Given the index is live listing Priya and reports no good pass yet
///   When a pass completes
///   Then the health response shows that pass's end time
///   And when a later pass skips every author the time stays the same
///   And the response carries only the status and that time
/// ```
#[test]
fn the_health_response_shows_when_the_last_good_pass_ended_and_only_that() {
    let world = given_authors_publish_on_their_own_pdses();
    let live = given_the_index_is_live_listing(&world, &[Author::Priya]);
    assert!(live.health().1["last_successful_pass_at"].is_null());
    let before = chrono::Utc::now();

    given_a_pass_indexed(&live);

    let after = chrono::Utc::now();
    let (status, health) = live.health();
    assert_eq!(status, 200, "{health}");
    let stamp = health["last_successful_pass_at"]
        .as_str()
        .unwrap_or_else(|| panic!("a timestamp after a good pass: {health}"));
    let at = chrono::DateTime::parse_from_rfc3339(stamp)
        .expect("RFC3339")
        .with_timezone(&chrono::Utc);
    assert!(
        at >= before - chrono::Duration::seconds(1) && at <= after + chrono::Duration::seconds(1),
        "{stamp}"
    );
    let keys: HashSet<&str> = health
        .as_object()
        .expect("object")
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(
        keys,
        HashSet::from(["status", "last_successful_pass_at"]),
        "{health}"
    );

    world.host_is_reachable(Host::MorelBsky, false);
    let outage = live.timer_fires();

    assert_one_summary_matching(&live, &outage, PassExit::TotalOutage);
    assert_eq!(
        live.health().1["last_successful_pass_at"],
        stamp,
        "unchanged by a failed pass"
    );
}

/// PS-23
/// ```gherkin
/// @US-IXD-002 @US-IXD-006 @AC-002.5 @AC-006.2 @ADR-080 @C7b @error @real-io @contract-shape:bounded-change
/// Scenario: An index killed in the middle of a pass comes back with what it had saved
///   Given a pass is running and has already saved Priya's claims
///   When the index container is killed and the runtime starts it again
///   Then the timer client reports that serve went away
///   And Maria finds Priya's claims at once, with no restore
///   And the next timer firing completes the pass and Maria finds Dmitri's claims too
/// ```
#[test]
fn an_index_killed_in_the_middle_of_a_pass_comes_back_with_what_it_had_saved() {
    let world = given_authors_publish_on_their_own_pdses();
    world.host_answers(
        Host::VolkovDev,
        ListingPosture::Slow(Duration::from_secs(8)),
    );
    let live = given_the_index_is_live_listing(&world, &[Author::Priya, Author::Dmitri]);
    let pending = live.timer_fires_in_background();
    when_the_pass_is_underway(&live);
    let saved = (0..100).any(|_| {
        std::thread::sleep(Duration::from_millis(50));
        rows_by(&maria_looks_up(&world, &live, Author::Priya), Author::Priya).len() == 2
    });
    assert!(
        saved,
        "Priya's claims were saved before the kill\n{}",
        live.dump()
    );

    let stopped = live.gets_killed();
    let abandoned = pending.finish();
    let live = match stopped.restarts() {
        Startup::Ready(live) => live,
        Startup::Refused(report) => panic!("serve restarts on the same index\n{}", report.dump()),
    };

    assert_eq!(
        abandoned.status,
        PassExit::ServeUnreachable.code(),
        "{}",
        abandoned.dump()
    );
    then_maria_finds(&world, &live, Author::Priya, 2);
    assert!(
        live.health().1["last_successful_pass_at"].is_null(),
        "in-memory, reset by a restart"
    );
    let next = live.timer_fires();
    assert_one_summary_matching(&live, &next, PassExit::Completed);
    then_maria_finds(&world, &live, Author::Dmitri, 2);
}

/// PS-24
/// ```gherkin
/// @US-IXD-002 @ADR-080 @C6a @error @real-io @contract-shape:bounded-change
/// Scenario: A socket file left over from a crash does not stop the next start
///   Given the index was killed and left a stale file at its control socket path
///   When the runtime starts it again
///   Then it becomes ready and the next timer firing runs a pass
/// ```
#[test]
fn a_socket_file_left_over_from_a_crash_does_not_stop_the_next_start() {
    let world = given_authors_publish_on_their_own_pdses();
    let live = given_the_index_is_live_listing(&world, &[Author::Priya]);
    let stopped = live.gets_killed();
    stopped.leaves_a_stale_socket_file();

    let live = match stopped.restarts() {
        Startup::Ready(live) => live,
        Startup::Refused(report) => panic!("serve replaces a stale socket\n{}", report.dump()),
    };

    let pass = live.timer_fires();
    assert_one_summary_matching(&live, &pass, PassExit::Completed);
}

/// PS-26
/// ```gherkin
/// @US-IXD-005 @I-IXD-4 @WD-105 @real-io @contract-shape:unbounded-preservation
/// Scenario: What the index logs about its passes never contains claim content
///   Given passes that index, skip, purge and refuse a list have run
///   When Jeff reads the index's log lines
///   Then every pass line carries only structural fields: DIDs, counts, reasons, causes and ids
/// ```
#[test]
fn what_the_index_logs_about_its_passes_never_contains_claim_content() {
    let world = given_authors_publish_on_their_own_pdses();
    world.host_answers(Host::JeffbaileyUs, ListingPosture::Status(502));
    let live =
        given_the_index_is_live_listing(&world, &[Author::Priya, Author::Dmitri, Author::Jeff]);
    given_a_pass_indexed(&live);
    live.operator_saves_list(&dids(&[Author::Priya, Author::Jeff]));
    live.timer_fires();
    live.operator_saves_list_text("did:plc:priyaraman7x2k,tomas");
    live.timer_fires();

    let events = live.events();

    assert!(
        !live.events_named("indexer.ingest.author_purged").is_empty(),
        "{}",
        live.dump()
    );
    assert!(
        !live.events_named("indexer.ingest.pass_refused").is_empty(),
        "{}",
        live.dump()
    );
    assert_events_carry_no_claim_content(&events, &[]);
}
