//! scale-to-zero-hosting — WALKING SKELETONS (ADR-090).
//!
//! WS-1: the scheduled pass on Lambda pulls Priya's claim from her PDS, publishes
//! the index as one object, and a search instance that starts afterwards answers
//! Maria's search with it. WS-2: the operator edits the DID list, with no deploy,
//! and the next pass brings a new author's claim to search (KPI-IXD-4). One
//! composition root (`openlore-indexer-lambda`), two of its entries (pass,
//! search), the real pass and the real DuckDB index in the instances' `/tmp`; the
//! object store, the DID list, the clock and the PDS network are fakes
//! (Architecture of Reference, policy rows of 2026-10-27).
//!
//! Layer 2 (in-process composition root, doubles of the AWS ports):
//! example-only. The Lambda image under the Runtime Interface Emulator with
//! moto is the CI `lambda-smoke` (DEVOPS DV-STZ-2), not a cargo test.
//!
//! What these do not assert: cold and warm times. R-STZ-1 and R-STZ-2 are
//! measured on a real client by the `lambda measure` gate, and R-STZ-5 by
//! `lambda kpi` (feature-delta § Requirement coverage).
//!
//! RED at hand-off: the Lambda root is a scaffold (`panic!` bodies), so the
//! skeletons are `#[ignore]`d like indexer-deployment's (DWD-IXD precedent);
//! DELIVER's first step un-ignores them.
#![cfg(unix)]

#[path = "../common/state_delta.rs"]
mod state_delta;

#[path = "support/indexer_lambda.rs"]
mod indexer_lambda;

use std::time::Duration;

use indexer_lambda::claims::*;
use indexer_lambda::*;

/// WS-1
/// ```gherkin
/// @walking_skeleton @driving_port @real-io @D-STZ-7 @D-STZ-12 @KPI-IXD-2 @kpi
/// @contract-shape:bounded-change
/// Scenario: Maria finds Priya's claim after the scheduled pass published the index
///   Given Priya published a claim that cargo-pin embodies reproducible builds
///   And the operator's DID list names Priya
///   And no index has been published yet
///   When the 14:00 schedule fires the pass
///   Then the pass completes and reports that it published the index
///   And a search instance that starts afterwards finds Priya's cargo-pin claim, attributed to her
///   And it reports the index as published at 14:00
/// ```
#[test]
#[ignore = "RED scaffold: DELIVER step 1 (Lambda root) un-ignores the walking skeleton"]
fn maria_finds_priyas_claim_after_the_scheduled_pass_published_the_index() {
    let world = LambdaWorld::listing(&[Author::Priya]);
    world.publishes(Author::Priya, &[CARGO_PIN_REPRODUCIBLE_BUILDS]);

    let run = world.cold_pass_instance().schedule_fires();

    run.assert_exit(Exit::Completed);
    assert!(run.published(), "{}", run.dump());
    let search = world.cold_search_instance();
    let reply = search.maria_searches("reproducible-builds");
    assert_eq!(reply.status, 200, "{}", reply.text());
    assert_eq!(
        reply.subjects_by(Author::Priya),
        vec![CARGO_PIN_REPRODUCIBLE_BUILDS.subject.to_string()],
        "{}",
        reply.text()
    );
    let health = search.health();
    assert_eq!(health.status, 200, "{}", health.text());
    let published_at: chrono::DateTime<chrono::Utc> = health.json()["last_successful_pass_at"]
        .as_str()
        .unwrap_or_else(|| panic!("a published-at time: {}", health.text()))
        .parse()
        .expect("RFC 3339");
    assert_eq!(published_at, world.clock.now(), "{}", health.text());
}

/// WS-2
/// ```gherkin
/// @walking_skeleton @driving_port @real-io @D-STZ-16 @R-STZ-6 @KPI-IXD-4 @kpi
/// @contract-shape:bounded-change
/// Scenario: The operator adds an author and, with no deploy, Maria finds their claim
///   Given the 14:00 pass published Priya's claim
///   And Tomás published a claim that loom embodies reproducible builds
///   And Maria cannot find it, because the operator's list does not name him
///   When the operator saves a list naming Priya and Tomás, and the 14:15 schedule fires the pass
///   Then a search instance that starts afterwards finds Tomás's loom claim, attributed to him
/// ```
#[test]
#[ignore = "RED scaffold: DELIVER step 1 (Lambda root) un-ignores the walking skeleton"]
fn the_operator_adds_an_author_and_with_no_deploy_maria_finds_their_claim() {
    let world = LambdaWorld::listing(&[Author::Priya]);
    world.publishes(Author::Priya, &[CARGO_PIN_REPRODUCIBLE_BUILDS]);
    world.publishes(Author::Tomas, &[LOOM_REPRODUCIBLE_BUILDS]);
    world.given_a_pass_published_the_index();
    let before = world
        .cold_search_instance()
        .maria_searches("reproducible-builds");
    assert!(
        before.subjects_by(Author::Tomas).is_empty(),
        "{}",
        before.text()
    );

    world.operator_lists(&[Author::Priya, Author::Tomas]);
    world.time_passes(Duration::from_secs(15 * 60));
    let run = world.cold_pass_instance().schedule_fires();

    run.assert_exit(Exit::Completed);
    let reply = world
        .cold_search_instance()
        .maria_searches("reproducible-builds");
    assert_eq!(
        reply.subjects_by(Author::Tomas),
        vec![LOOM_REPRODUCIBLE_BUILDS.subject.to_string()],
        "{}",
        reply.text()
    );
}
