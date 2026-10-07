//! indexer-deployment — the operator's DID list (US-IXD-003; ADR-081) and the
//! purge of authors removed from it (AC-003.2 as amended 2026-10-06; ADR-082,
//! ADR-078 amended: a SKIP keeps claims, a REMOVAL purges them).
//!
//! The list is a host-rendered FILE in a read-only config DIRECTORY, replaced by
//! rename (what `render-dids.sh` does). `serve` reads and validates it at the
//! start of EVERY pass — no restart, no redeploy. The host's own behaviour
//! (SSM read failure keeps the last good file, never swaps the directory) is
//! asserted against the real script in `xtask/tests/indexer_deployment_platform.rs`.
//!
//! Layer 3 (real `serve` + `trigger` + `index.duckdb`; PLC/PDSes faked).
//! Example-only (Mandates 9, 11). The pure decisions — list parsing totality and
//! `plan_purge` (set difference, empty → suppressed) — are properties in
//! `indexer_deployment_core.rs`, together with the purge universe (children,
//! artifact files, segment and prefix collisions) at the adapter.
//!
//! ## List read and purge as a state machine (C2a; ADR-081 table, ADR-082 §3)
//!
//! ```text
//! pass start ──read file──▶ Loaded(non-empty) ──plan_purge──▶ Purge(indexed − listed) ──▶ fetch/gate ──▶ summary 0|3
//!                         ├▶ Loaded(empty) ──▶ purge_suppressed ──▶ summary 0 (configured 0)
//!                         ├▶ Refused(malformed) ──▶ pass_refused ──▶ summary 2   [no fetch, NO purge]
//!                         └▶ Refused(unreadable) ──▶ pass_refused ──▶ summary 2  [no fetch, NO purge]
//! purge flag unset ⇒ Purge(∅) always.  A skipped-but-listed DID is never in Purge(·).
//! ```
//!
//! Story line (Pillar 2): DL-30 (add Tomás) → DL-31 (a second edit) and PG-40
//! (remove Dmitri) → PG-47 (purge again / list him again) chain from
//! `journey::given_authors_publish_on_their_own_pdses` + the first pass.
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
use std::time::Duration;

use indexer_live::journey::*;
use indexer_live::*;
use indexer_network::{search_rows, Author, Host, IndexerWorld, ListingPosture, SearchRow};
use support::state_delta::{assert_state_delta, set_to, Delta};

/// A step that puts the list file into some state.
type Arrange = fn(&LiveIndex);

const LISTED: [Author; 3] = [Author::Priya, Author::Dmitri, Author::Jeff];

/// GIVEN Jeff's live index lists Priya, Dmitri and Jeff, and the first pass
/// indexed all of them (Tomás is on the network, not listed).
fn given_the_first_pass_indexed_priya_dmitri_and_jeff() -> (IndexerWorld, LiveIndex) {
    let world = given_authors_publish_on_their_own_pdses();
    let live = given_the_index_is_live_listing(&world, &LISTED);
    given_a_pass_indexed(&live);
    then_maria_finds(&world, &live, Author::Dmitri, 2);
    (world, live)
}

/// Maria's results per author — the port-exposed universe of a pass's effect.
fn maria_results_by_author(world: &IndexerWorld, live: &LiveIndex) -> HashMap<String, Vec<String>> {
    [Author::Priya, Author::Dmitri, Author::Jeff, Author::Tomas]
        .iter()
        .map(|a| {
            let mut blocks: Vec<String> = rows_by(&maria_looks_up(world, live, *a), *a)
                .into_iter()
                .map(|r: SearchRow| r.block.join("\n"))
                .collect();
            blocks.sort();
            (slot(*a), blocks)
        })
        .collect()
}

fn slot(author: Author) -> String {
    format!("maria.results.by_author[{}]", author.did())
}

fn universe_of(snapshot: &HashMap<String, Vec<String>>) -> HashSet<String> {
    snapshot.keys().cloned().collect()
}

/// The `indexer.config.loaded` event of the pass a summary closed.
fn list_loaded_for(live: &LiveIndex, summary: &serde_json::Value) -> serde_json::Value {
    let pass_id = summary["pass_id"].as_str().expect("pass_id").to_string();
    live.events_of_pass(&pass_id)
        .into_iter()
        .find(|e| e["event"] == "indexer.config.loaded")
        .unwrap_or_else(|| panic!("indexer.config.loaded for pass {pass_id}\n{}", live.dump()))
}

/// THEN the pass refused the list for `cause`, naming `value`, and purged and fetched nothing.
fn then_the_pass_refused_the_list(
    live: &LiveIndex,
    pass: &indexer_network::PassReport,
    cause: FailureCause,
    value: Option<&str>,
) -> serde_json::Value {
    let summary = assert_one_summary_matching(live, pass, PassExit::Failed);
    assert_eq!(summary["cause"], cause.token(), "{summary}");
    for count in [
        "configured",
        "own_pds",
        "fallback",
        "skipped",
        "purged_authors",
    ] {
        assert_eq!(
            summary[count].as_u64(),
            Some(0),
            "{count} is 0 on a refused list: {summary}"
        );
    }
    let pass_id = summary["pass_id"].as_str().expect("pass_id");
    let refused: Vec<_> = live
        .events_of_pass(pass_id)
        .into_iter()
        .filter(|e| e["event"] == "indexer.ingest.pass_refused")
        .collect();
    assert_eq!(refused.len(), 1, "{}", live.dump());
    assert_eq!(refused[0]["cause"], cause.token(), "{}", refused[0]);
    assert_eq!(
        refused[0]["variable"],
        var::REPO_DIDS_FILE,
        "{}",
        refused[0]
    );
    if let Some(value) = value {
        assert!(
            refused[0]["value"].as_str().unwrap_or("").contains(value),
            "names the bad entry `{value}`: {}",
            refused[0]
        );
    }
    assert!(
        live.events_named("indexer.ingest.author_purged").is_empty(),
        "no purge on a refused list"
    );
    summary
}

// =============================================================================
// Editing the list (AC-003.1, KPI-IXD-4, H1)
// =============================================================================

/// DL-30
/// ```gherkin
/// @US-IXD-003 @AC-003.1 @KPI-IXD-4 @kpi @real-io @driving_port @contract-shape:bounded-change
/// Scenario: A DID added to the list is indexed on the next pass without a restart
///   Given the first pass indexed Priya, Dmitri and Jeff
///   When Jeff adds Tomás to the DID list and the timer fires
///   Then the pass reports 4 configured DIDs read from the list file
///   And Maria finds Tomás's claim
///   And the index was never restarted
/// ```
#[test]
fn a_did_added_to_the_list_is_indexed_on_the_next_pass_without_a_restart() {
    let (world, live) = given_the_first_pass_indexed_priya_dmitri_and_jeff();
    live.operator_saves_list(&dids(&[
        Author::Priya,
        Author::Dmitri,
        Author::Jeff,
        Author::Tomas,
    ]));

    let pass = live.timer_fires();

    let summary = assert_one_summary_matching(&live, &pass, PassExit::Completed);
    let loaded = list_loaded_for(&live, &summary);
    assert_eq!(loaded["repo_did_count"].as_u64(), Some(4), "{loaded}");
    assert_eq!(loaded["repo_dids_source"], "file", "{loaded}");
    then_maria_finds(&world, &live, Author::Tomas, 1);
    assert_eq!(live.times_started(), 1, "no restart");
}

/// DL-31
/// ```gherkin
/// @US-IXD-003 @AC-003.1 @H1 @ADR-081 @real-io @contract-shape:bounded-change
/// Scenario: Two list edits in a row are picked up by two passes in a row
///   Given Jeff added Tomás and the next pass reported 4 DIDs
///   When Jeff removes Jeff's own DID and adds nobody, and the timer fires
///   And then Jeff lists Jeff again and the timer fires
///   Then the passes report 3 and then 4 configured DIDs, with no restart
/// ```
#[test]
fn two_list_edits_in_a_row_are_picked_up_by_two_passes_in_a_row() {
    let (world, live) = given_the_first_pass_indexed_priya_dmitri_and_jeff();
    live.operator_saves_list(&dids(&[
        Author::Priya,
        Author::Dmitri,
        Author::Jeff,
        Author::Tomas,
    ]));
    live.timer_fires();

    live.operator_saves_list(&dids(&[Author::Priya, Author::Dmitri, Author::Tomas]));
    let first = live.timer_fires();
    let first = assert_one_summary_matching(&live, &first, PassExit::Completed);
    live.operator_saves_list(&dids(&[
        Author::Priya,
        Author::Dmitri,
        Author::Tomas,
        Author::Jeff,
    ]));
    let second = live.timer_fires();
    let second = assert_one_summary_matching(&live, &second, PassExit::Completed);

    assert_eq!(
        list_loaded_for(&live, &first)["repo_did_count"].as_u64(),
        Some(3)
    );
    assert_eq!(
        list_loaded_for(&live, &second)["repo_did_count"].as_u64(),
        Some(4)
    );
    then_maria_finds(&world, &live, Author::Jeff, 1);
    assert_eq!(live.times_started(), 1, "no restart");
}

/// DL-37
/// ```gherkin
/// @US-IXD-003 @B15 @ADR-081 @M5 @real-io @contract-shape:pure-function
/// Scenario: Each pass reports how old the DID list is
///   Given the list was last rendered 3 hours ago
///   When the timer fires
///   Then the pass reports a list age of about 3 hours
/// ```
#[test]
fn each_pass_reports_how_old_the_did_list_is() {
    let world = given_authors_publish_on_their_own_pdses();
    let live = given_the_index_is_live_listing(&world, &[Author::Priya]);
    live.list_was_rendered_ago(Duration::from_secs(3 * 3600));

    let pass = live.timer_fires();

    let summary = assert_one_summary_matching(&live, &pass, PassExit::Completed);
    let age = list_loaded_for(&live, &summary)["repo_dids_age_secs"].as_u64();
    assert!(
        matches!(age, Some(a) if (3 * 3600..3 * 3600 + 120).contains(&a)),
        "about 3 hours: {age:?}"
    );
}

// =============================================================================
// Bad lists (AC-003.3, AC-003.4 binary side)
// =============================================================================

/// DL-32
/// ```gherkin
/// @US-IXD-003 @US-IXD-004 @AC-003.3 @AC-004.2 @AC-004.4 @error @real-io @contract-shape:bounded-change
/// Scenario: A mistyped list refuses the pass, names the bad entry and keeps search serving
///   Given the first pass indexed Priya, Dmitri and Jeff
///   When Jeff saves the list "did:plc:therrera2v6w,tomas" and the timer fires
///   Then trigger exits 2 and the pass is refused naming the list file setting and "tomas"
///   And Maria still finds Dmitri's claims indexed before the edit
///   And when Jeff fixes the list the next pass completes
/// ```
#[test]
fn a_mistyped_list_refuses_the_pass_names_the_bad_entry_and_keeps_search_serving() {
    let (world, live) = given_the_first_pass_indexed_priya_dmitri_and_jeff();
    live.operator_saves_list_text("did:plc:therrera2v6w,tomas");

    let pass = live.timer_fires();

    then_the_pass_refused_the_list(&live, &pass, FailureCause::ListMalformed, Some("tomas"));
    then_maria_finds(&world, &live, Author::Dmitri, 2);
    live.operator_saves_list(&dids(&[
        Author::Priya,
        Author::Dmitri,
        Author::Jeff,
        Author::Tomas,
    ]));
    let fixed = live.timer_fires();
    assert_one_summary_matching(&live, &fixed, PassExit::Completed);
}

/// DL-33
/// ```gherkin
/// @US-IXD-003 @AC-003.4 @ADR-081 @error @C7a @real-io @contract-shape:unbounded-preservation
/// Scenario Outline: A list the index cannot read refuses the pass and touches nothing
///   Given the first pass indexed Priya, Dmitri and Jeff
///   And the list file <state>
///   When the timer fires
///   Then trigger exits 2 and the pass is refused as unreadable
///   And no author's PDS was contacted and Maria's results are exactly as before
///   Examples:
///     | state                                   |
///     | is missing                              |
///     | is something that cannot be read as a file |
/// ```
#[test]
fn a_list_the_index_cannot_read_refuses_the_pass_and_touches_nothing() {
    let cases: [(&str, Arrange); 2] = [
        ("missing", |live| live.list_file_is_missing()),
        ("unreadable", |live| live.list_file_is_unreadable()),
    ];
    for (label, arrange) in cases {
        let (world, live) = given_the_first_pass_indexed_priya_dmitri_and_jeff();
        let before = maria_results_by_author(&world, &live);
        arrange(&live);
        world.net.clear_request_log();

        let pass = live.timer_fires();

        then_the_pass_refused_the_list(&live, &pass, FailureCause::ListUnreadable, None);
        for host in [Host::MorelBsky, Host::VolkovDev, Host::JeffbaileyUs] {
            assert!(
                world.listings_on(host).is_empty(),
                "[{label}] nothing fetched from {host:?}"
            );
        }
        let after = maria_results_by_author(&world, &live);
        assert_state_delta(&before, &after, &universe_of(&before), &Delta::new());
    }
}

/// DL-34
/// ```gherkin
/// @US-IXD-003 @ADR-081 @C6a @error @real-io @contract-shape:pure-function
/// Scenario Outline: Line endings are tolerated, a hidden byte-order mark is refused loudly
///   Given Jeff's list is saved as <text>
///   When the timer fires
///   Then the pass <outcome>
///   Examples:
///     | text                                            | outcome                                         |
///     | Priya and Dmitri on CRLF-terminated lines       | completes with 2 configured DIDs                |
///     | a byte-order mark then Priya, Dmitri            | is refused, naming the first entry              |
/// ```
#[test]
fn line_endings_are_tolerated_a_hidden_byte_order_mark_is_refused_loudly() {
    let world = given_authors_publish_on_their_own_pdses();
    let crlf = LiveIndex::deploy(
        &world,
        Deployment::with_list_text("did:plc:priyaraman7x2k\r\ndid:plc:dvolkov3m9q\r\n"),
    );
    let pass = crlf.timer_fires();
    let summary = assert_one_summary_matching(&crlf, &pass, PassExit::Completed);
    assert_eq!(summary["configured"].as_u64(), Some(2), "{summary}");
    crlf.stop();

    let world = given_authors_publish_on_their_own_pdses();
    let bom = LiveIndex::deploy(
        &world,
        Deployment::with_list_text("\u{feff}did:plc:priyaraman7x2k,did:plc:dvolkov3m9q"),
    );
    let pass = bom.timer_fires();
    then_the_pass_refused_the_list(
        &bom,
        &pass,
        FailureCause::ListMalformed,
        Some("did:plc:priyaraman7x2k"),
    );
}

/// DL-35
/// ```gherkin
/// @US-IXD-003 @ADR-081 @C5a @error @real-io @contract-shape:unbounded-preservation
/// Scenario: Setting both a list and a list file is refused at start
///   Given Jeff configured both an inline DID list and a DID list file
///   When the index starts
///   Then it refuses to start with exit 2, naming the conflicting setting
/// ```
#[test]
fn setting_both_a_list_and_a_list_file_is_refused_at_start() {
    let world = given_authors_publish_on_their_own_pdses();

    let startup = LiveIndex::launch(
        &world,
        Deployment::listing(&dids(&[Author::Priya])).setting(var::REPO_DIDS, Author::Dmitri.did()),
    );

    let Startup::Refused(report) = startup else {
        panic!("MISSING_FUNCTIONALITY: serve started with both list settings");
    };
    assert_eq!(report.status, 2, "{}", report.dump());
    let refusal = report
        .startup_refusal()
        .unwrap_or_else(|| panic!("health.startup.refused\n{}", report.dump()));
    let named = refusal.to_string();
    assert!(
        named.contains(var::REPO_DIDS_FILE) || named.contains(var::REPO_DIDS),
        "names the conflicting setting: {refusal}"
    );
}

// =============================================================================
// Purge (AC-003.2 amended; ADR-082)
// =============================================================================

/// PG-40
/// ```gherkin
/// @US-IXD-003 @AC-003.2 @ADR-082 @real-io @driving_port @contract-shape:bounded-change
/// Scenario: Removing an author from the list removes their claims on the next pass
///   Given the first pass indexed Priya, Dmitri and Jeff
///   When Jeff removes Dmitri from the list and the timer fires
///   Then the pass reports one purged author, with 2 claims removed for Dmitri
///   And Maria no longer finds Dmitri's claims
///   And everything Maria finds for every other author is exactly as before
///   And no file of Dmitri's is left in the index
/// ```
#[test]
fn removing_an_author_from_the_list_removes_their_claims_on_the_next_pass() {
    let (world, live) = given_the_first_pass_indexed_priya_dmitri_and_jeff();
    let before = maria_results_by_author(&world, &live);
    live.operator_saves_list(&dids(&[Author::Priya, Author::Jeff]));

    let pass = live.timer_fires();

    let summary = assert_one_summary_matching(&live, &pass, PassExit::Completed);
    assert_eq!(summary["purged_authors"].as_u64(), Some(1), "{summary}");
    let purged = live.events_named("indexer.ingest.author_purged");
    assert_eq!(purged.len(), 1, "{}", live.dump());
    assert_eq!(purged[0]["did"], Author::Dmitri.did(), "{}", purged[0]);
    assert_eq!(
        purged[0]["claims_removed"].as_u64(),
        Some(2),
        "{}",
        purged[0]
    );
    let after = maria_results_by_author(&world, &live);
    assert_state_delta(
        &before,
        &after,
        &universe_of(&before),
        &Delta::new().with_slot(slot(Author::Dmitri), set_to(Vec::new())),
    );
    live.stop();
    assert!(
        world.rows_of(Author::Dmitri).is_empty(),
        "no index row of Dmitri's"
    );
    let artifacts = indexer_network_artifacts(&world);
    assert!(
        artifacts.iter().all(|p| !p.contains("dvolkov3m9q")),
        "no artifact file of Dmitri's: {artifacts:?}"
    );
}

/// PG-41
/// ```gherkin
/// @US-IXD-003 @AC-003.2 @ADR-082 @ADR-078 @error @real-io @contract-shape:bounded-change
/// Scenario: A removed author is purged even when every listed author is unreachable
///   Given the first pass indexed Priya, Dmitri and Jeff
///   And Priya's and Jeff's PDSes are down
///   When Jeff removes Dmitri from the list and the timer fires
///   Then trigger exits 3 and Dmitri's claims are purged
///   And Maria still finds Priya's and Jeff's claims
/// ```
#[test]
fn a_removed_author_is_purged_even_when_every_listed_author_is_unreachable() {
    let (world, live) = given_the_first_pass_indexed_priya_dmitri_and_jeff();
    world.host_is_reachable(Host::MorelBsky, false);
    world.host_is_reachable(Host::JeffbaileyUs, false);
    live.operator_saves_list(&dids(&[Author::Priya, Author::Jeff]));

    let pass = live.timer_fires();

    let summary = assert_one_summary_matching(&live, &pass, PassExit::TotalOutage);
    assert_eq!(summary["purged_authors"].as_u64(), Some(1), "{summary}");
    then_maria_finds(&world, &live, Author::Dmitri, 0);
    then_maria_finds(&world, &live, Author::Priya, 2);
    then_maria_finds(&world, &live, Author::Jeff, 1);
}

/// PG-42
/// ```gherkin
/// @US-IXD-003 @AC-003.2 @ADR-078 @error @real-io @contract-shape:unbounded-preservation
/// Scenario: An author who is still listed but unreachable keeps their claims
///   Given the first pass indexed Priya, Dmitri and Jeff
///   And Dmitri's PDS answers every listing with 502
///   When the timer fires
///   Then Dmitri is skipped, nobody is purged
///   And everything Maria finds is exactly as before
/// ```
#[test]
fn an_author_who_is_still_listed_but_unreachable_keeps_their_claims() {
    let (world, live) = given_the_first_pass_indexed_priya_dmitri_and_jeff();
    let before = maria_results_by_author(&world, &live);
    world.host_answers(Host::VolkovDev, ListingPosture::Status(502));

    let pass = live.timer_fires();

    let summary = assert_one_summary_matching(&live, &pass, PassExit::Completed);
    assert_eq!(summary["skipped"].as_u64(), Some(1), "{summary}");
    assert_eq!(summary["purged_authors"].as_u64(), Some(0), "{summary}");
    let after = maria_results_by_author(&world, &live);
    assert_state_delta(&before, &after, &universe_of(&before), &Delta::new());
}

/// PG-43
/// ```gherkin
/// @US-IXD-003 @AC-003.2 @ADR-082 @error @real-io @contract-shape:unbounded-preservation
/// Scenario Outline: A list that drops an author but cannot be used purges nothing
///   Given the first pass indexed Priya, Dmitri and Jeff
///   And Jeff's new list drops Dmitri but <problem>
///   When the timer fires
///   Then the pass is refused and nothing Maria finds changes
///   Examples:
///     | problem                         |
///     | also contains the typo "tomas"  |
///     | cannot be read                  |
/// ```
#[test]
fn a_list_that_drops_an_author_but_cannot_be_used_purges_nothing() {
    let cases: [(&str, Arrange, FailureCause); 2] = [
        (
            "typo",
            |live| {
                live.operator_saves_list_text("did:plc:priyaraman7x2k,did:plc:jeffbailey5n2p,tomas")
            },
            FailureCause::ListMalformed,
        ),
        (
            "unreadable",
            |live| live.list_file_is_unreadable(),
            FailureCause::ListUnreadable,
        ),
    ];
    for (_label, arrange, cause) in cases {
        let (world, live) = given_the_first_pass_indexed_priya_dmitri_and_jeff();
        let before = maria_results_by_author(&world, &live);
        arrange(&live);

        let pass = live.timer_fires();

        then_the_pass_refused_the_list(&live, &pass, cause, None);
        let after = maria_results_by_author(&world, &live);
        assert_state_delta(&before, &after, &universe_of(&before), &Delta::new());
    }
}

/// PG-36
/// ```gherkin
/// @US-IXD-003 @AC-003.2 @ADR-082 @C3 @edge @real-io @contract-shape:unbounded-preservation
/// Scenario: An empty list never empties the index
///   Given the first pass indexed Priya, Dmitri and Jeff
///   When the list is saved empty and the timer fires
///   Then the pass completes with no DIDs configured and reports the purge was held back
///   And everything Maria finds is exactly as before
/// ```
#[test]
fn an_empty_list_never_empties_the_index() {
    let (world, live) = given_the_first_pass_indexed_priya_dmitri_and_jeff();
    let before = maria_results_by_author(&world, &live);
    live.operator_saves_list_text("  \n");

    let pass = live.timer_fires();

    let summary = assert_one_summary_matching(&live, &pass, PassExit::Completed);
    assert_eq!(summary["configured"].as_u64(), Some(0), "{summary}");
    assert_eq!(summary["purged_authors"].as_u64(), Some(0), "{summary}");
    let held = live.events_named("indexer.ingest.purge_suppressed");
    assert_eq!(held.len(), 1, "{}", live.dump());
    assert_eq!(held[0]["reason"], "empty_list", "{}", held[0]);
    let after = maria_results_by_author(&world, &live);
    assert_state_delta(&before, &after, &universe_of(&before), &Delta::new());
}

/// PG-44
/// ```gherkin
/// @US-IXD-003 @ADR-082 @C5a @real-io @contract-shape:unbounded-preservation
/// Scenario: Without the purge setting a removed author's claims stay
///   Given the index runs without the purge setting and indexed Priya, Dmitri and Jeff
///   When Jeff removes Dmitri from the list and the timer fires
///   Then nobody is purged and Maria still finds Dmitri's claims
/// ```
#[test]
fn without_the_purge_setting_a_removed_author_s_claims_stay() {
    let world = given_authors_publish_on_their_own_pdses();
    let live = LiveIndex::deploy(&world, Deployment::listing(&dids(&LISTED)).without_purge());
    given_a_pass_indexed(&live);
    live.operator_saves_list(&dids(&[Author::Priya, Author::Jeff]));

    let pass = live.timer_fires();

    let summary = assert_one_summary_matching(&live, &pass, PassExit::Completed);
    assert_eq!(summary["purged_authors"].as_u64(), Some(0), "{summary}");
    assert!(live.events_named("indexer.ingest.author_purged").is_empty());
    then_maria_finds(&world, &live, Author::Dmitri, 2);
}

/// PG-45
/// ```gherkin
/// @US-IXD-003 @ADR-082 @C1b @adversarial @real-io @contract-shape:bounded-change
/// Scenario: Only the removed author is purged, never one whose DID merely starts the same way
///   Given did:plc:crowdmember0001 and did:plc:crowdmember00011 each have one indexed claim
///   When Jeff removes did:plc:crowdmember0001 and the timer fires
///   Then only did:plc:crowdmember0001's claim is gone
/// ```
#[test]
fn only_the_removed_author_is_purged_never_one_whose_did_merely_starts_the_same_way() {
    let short = "did:plc:crowdmember0001";
    let long = "did:plc:crowdmember00011";
    let world = IndexerWorld::configured_with_members(
        &[],
        &[(short, Host::VolkovDev), (long, Host::MorelBsky)],
    );
    world.did_publishes_self_attested(short, &[indexer_network::claims::FERRITE_TEST_DRIVEN]);
    world.did_publishes_self_attested(long, &[indexer_network::claims::TIDEPOOL_MEMORY_SAFETY]);
    let live = LiveIndex::deploy(&world, Deployment::listing(&[short, long]));
    given_a_pass_indexed(&live);
    live.operator_saves_list(&[long]);

    let pass = live.timer_fires();

    assert_one_summary_matching(&live, &pass, PassExit::Completed);
    let found = |did: &str| {
        let out = live.maria_searches(&world, &["--contributor", did]);
        search_rows(&out.stdout)
            .into_iter()
            .filter(|r| r.author_did == did)
            .count()
    };
    assert_eq!(found(short), 0, "the removed author is purged");
    assert_eq!(found(long), 1, "the listed look-alike keeps its claim");
}

/// PG-46
/// ```gherkin
/// @US-IXD-003 @ADR-082 @real-io @contract-shape:bounded-change
/// Scenario: A removed author's app-signed and self-attested claims are both purged
///   Given Priya has 2 self-attested and 1 app-signed claim indexed
///   When Jeff removes Priya from the list and the timer fires
///   Then the pass reports 3 claims removed for Priya and Maria finds none of them
/// ```
#[test]
fn a_removed_author_s_app_signed_and_self_attested_claims_are_both_purged() {
    let mut world = given_authors_publish_on_their_own_pdses();
    world.publishes_app_signed(
        Author::Priya,
        &[indexer_network::claims::TIDEPOOL_MEMORY_SAFETY],
    );
    let live = given_the_index_is_live_listing(&world, &LISTED);
    given_a_pass_indexed(&live);
    then_maria_finds(&world, &live, Author::Priya, 3);
    live.operator_saves_list(&dids(&[Author::Dmitri, Author::Jeff]));

    let pass = live.timer_fires();

    assert_one_summary_matching(&live, &pass, PassExit::Completed);
    let purged = live.events_named("indexer.ingest.author_purged");
    assert_eq!(purged.len(), 1, "{}", live.dump());
    assert_eq!(
        purged[0]["did"],
        Author::Priya.did(),
        "the bare DID: {}",
        purged[0]
    );
    assert_eq!(
        purged[0]["claims_removed"].as_u64(),
        Some(3),
        "{}",
        purged[0]
    );
    then_maria_finds(&world, &live, Author::Priya, 0);
}

/// PG-47
/// ```gherkin
/// @US-IXD-003 @ADR-082 @R-IXD-D2 @C4a @real-io @contract-shape:bounded-change
/// Scenario: Purging is done once, and listing the author again brings their claims back
///   Given the pass after Jeff removed Dmitri purged his claims
///   When the timer fires again
///   Then nobody is purged
///   And when Jeff lists Dmitri again the next pass indexes his 2 claims again
/// ```
#[test]
fn purging_is_done_once_and_listing_the_author_again_brings_their_claims_back() {
    let (world, live) = given_the_first_pass_indexed_priya_dmitri_and_jeff();
    live.operator_saves_list(&dids(&[Author::Priya, Author::Jeff]));
    live.timer_fires();

    let again = live.timer_fires();

    let summary = assert_one_summary_matching(&live, &again, PassExit::Completed);
    assert_eq!(summary["purged_authors"].as_u64(), Some(0), "{summary}");
    live.operator_saves_list(&dids(&LISTED));
    let back = live.timer_fires();
    assert_one_summary_matching(&live, &back, PassExit::Completed);
    then_maria_finds(&world, &live, Author::Dmitri, 2);
}

/// PG-48
/// ```gherkin
/// @US-IXD-003 @US-IXD-004 @AC-004.2 @ADR-082 @error @C7b @real-io @contract-shape:bounded-change
/// Scenario: A purge that fails ends the pass with exit 2 and finishes on the next pass
///   Given the first pass indexed Priya, Dmitri and Jeff
///   And the purge of the next pass will fail
///   When Jeff removes Dmitri from the list and the timer fires
///   Then trigger exits 2 and the one pass summary says the purge failed
///   And the following pass completes the purge of Dmitri
/// ```
#[test]
#[ignore = "DELIVER 02-03: purge failure → exit 2 purge_failed; resumable (B4, B6)"]
fn a_purge_that_fails_ends_the_pass_with_exit_2_and_finishes_on_the_next_pass() {
    let world = given_authors_publish_on_their_own_pdses();
    let live = LiveIndex::deploy(
        &world,
        Deployment::listing(&dids(&LISTED)).with_fault(Fault::PurgeFails),
    );
    given_a_pass_indexed(&live);
    live.operator_saves_list(&dids(&[Author::Priya, Author::Jeff]));

    let failed = live.timer_fires();

    let summary = assert_one_summary_matching(&live, &failed, PassExit::Failed);
    assert_eq!(
        summary["cause"],
        FailureCause::PurgeFailed.token(),
        "{summary}"
    );
    let next = live.timer_fires();
    assert_one_summary_matching(&live, &next, PassExit::Completed);
    then_maria_finds(&world, &live, Author::Dmitri, 0);
    then_maria_finds(&world, &live, Author::Priya, 2);
}

/// Every artifact file path under the index's `indexed_claims/` (relative).
fn indexer_network_artifacts(world: &IndexerWorld) -> Vec<String> {
    let root = support::index_duckdb_path(&world.env)
        .parent()
        .expect("index directory")
        .join("indexed_claims");
    let mut out = Vec::new();
    let mut stack = vec![root.clone()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else {
                out.push(
                    path.strip_prefix(&root)
                        .unwrap_or(&path)
                        .display()
                        .to_string(),
                );
            }
        }
    }
    out
}
