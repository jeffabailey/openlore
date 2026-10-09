//! indexer-deployment — WALKING SKELETON (thin slice US-IXD-001 + US-IXD-002).
//!
//! Maria's `openlore search` against a running `openlore-indexer serve` returns
//! claims from authors on several PDSes, after the host timer's `trigger` ran a
//! pass INSIDE `serve` (ADR-080). One process owns the index; search answers
//! while the pass writes; the DID list is a host-rendered file (ADR-081).
//!
//! Layer 4 (`@walking_skeleton @wiring_e2e`): the real `serve`, the real
//! `trigger`, the real `openlore search`, a real `index.duckdb`; only PLC and
//! the PDSes are fakes (`FakeAtprotoNetwork`). Example-only (Mandate 9).
//!
//! Story line (Pillar 2): WS-1 (Maria finds Priya and Dmitri) → WS-2 (a claim
//! Priya approves later shows up after the next pass). WS-0 is the edge before
//! any pass. All share `journey::given_authors_publish_on_their_own_pdses`.
#![cfg(unix)]

mod support;

#[path = "support/indexer_network.rs"]
mod indexer_network;

#[path = "support/indexer_live.rs"]
mod indexer_live;

use std::collections::BTreeSet;

use indexer_live::journey::*;
use indexer_live::*;
use indexer_network::{search_rows, Author};

/// WS-1
/// ```gherkin
/// @walking_skeleton @wiring_e2e @driving_port @real-io @US-IXD-001 @US-IXD-002 @AC-001.1
/// @AC-002.4 @KPI-IXD-1 @kpi @contract-shape:bounded-change
/// Scenario: Maria finds claims from authors on different PDSes after the scheduled pass
///   Given Priya published 2 self-attested claims on her bsky.social PDS
///   And Dmitri published 2 app-signed claims on pds.volkov.dev
///   And Jeff deployed the index listing Priya and Dmitri
///   When the 15-minute timer fires
///   Then the pass completes and reports exactly one summary
///   And Maria's search for reproducible builds shows Priya's cargo-pin claim and Dmitri's ferrite claim
///   And each claim is attributed to its own author, from 2 different PDS hosts
///   And Priya's claim is marked self-attested
/// ```
#[test]
fn maria_finds_claims_from_authors_on_different_pdses_after_the_scheduled_pass() {
    let world = given_authors_publish_on_their_own_pdses();
    let live = given_the_index_is_live_listing(&world, &[Author::Priya, Author::Dmitri]);

    let pass = live.timer_fires();

    let summary = assert_one_summary_matching(&live, &pass, PassExit::Completed);
    assert_eq!(summary["configured"].as_u64(), Some(2), "{summary}");
    assert_eq!(summary["own_pds"].as_u64(), Some(2), "{summary}");
    let rows = maria_searches_reproducible_builds(&world, &live);
    let priya = rows_by(&rows, Author::Priya);
    let dmitri = rows_by(&rows, Author::Dmitri);
    assert_eq!(priya.len(), 1, "Priya's cargo-pin claim: {rows:#?}");
    assert_eq!(priya[0].subject, "github:priyaraman/cargo-pin");
    assert!(
        priya[0].markers.contains("[self-attested]"),
        "{:?}",
        priya[0]
    );
    assert_eq!(dmitri.len(), 1, "Dmitri's ferrite claim: {rows:#?}");
    assert_eq!(dmitri[0].subject, "github:dvolkov/ferrite");
    let hosts: BTreeSet<&str> = [Author::Priya, Author::Dmitri]
        .iter()
        .filter(|a| !rows_by(&rows, **a).is_empty())
        .map(|a| a.home().expect("a home PDS").label())
        .collect();
    assert_eq!(hosts.len(), 2, "claims from 2 distinct PDS hosts");
    assert!(
        rows.iter().all(|r| !r.author_did.is_empty()),
        "every result is attributed to an author: {rows:#?}"
    );
}

/// WS-0
/// ```gherkin
/// @walking_skeleton @driving_port @real-io @US-IXD-001 @AC-001.4 @edge @contract-shape:unbounded-preservation
/// Scenario: A fresh deployment answers before its first pass
///   Given Jeff has just deployed the index listing Priya and Dmitri
///   And no pass has run
///   When Maria searches for reproducible builds
///   Then she gets an empty result quickly, not an error
///   And the index reports it is healthy with no successful pass yet
/// ```
#[test]
fn a_fresh_deployment_answers_before_its_first_pass() {
    let world = given_authors_publish_on_their_own_pdses();
    let live = given_the_index_is_live_listing(&world, &[Author::Priya, Author::Dmitri]);

    let rows = maria_searches_reproducible_builds(&world, &live);
    let (status, health) = live.health();

    assert!(rows.is_empty(), "nothing indexed yet: {rows:#?}");
    assert_eq!(
        status,
        200,
        "healthy before the first pass: {health}\n{}",
        live.dump()
    );
    assert_eq!(health["status"], "ok", "{health}");
    assert!(
        health["last_successful_pass_at"].is_null(),
        "no successful pass yet: {health}"
    );
    assert!(live.pass_summaries().is_empty(), "no pass ran by itself");
}

/// WS-2
/// ```gherkin
/// @walking_skeleton @driving_port @real-io @US-IXD-002 @AC-002.1 @KPI-IXD-2 @kpi @contract-shape:bounded-change
/// Scenario: A claim Priya approves after a pass becomes searchable on the next pass
///   Given the 14:00 pass indexed Priya's and Dmitri's claims
///   And Priya approves a new test-driven claim on cargo-pin at 14:02
///   And Maria does not find it yet
///   When the 14:15 timer fires
///   Then Maria finds Priya's new claim
///   And nothing was redeployed or restarted
/// ```
#[test]
fn a_claim_priya_approves_after_a_pass_becomes_searchable_on_the_next_pass() {
    let world = given_authors_publish_on_their_own_pdses();
    let live = given_the_index_is_live_listing(&world, &[Author::Priya, Author::Dmitri]);
    given_a_pass_indexed(&live);
    world.publishes_self_attested(Author::Priya, &[CARGO_PIN_TEST_DRIVEN]);
    then_maria_finds(&world, &live, Author::Priya, 2);

    let pass = live.timer_fires();

    assert_one_summary_matching(&live, &pass, PassExit::Completed);
    then_maria_finds(&world, &live, Author::Priya, 3);
    let out = live.maria_searches(&world, &["--subject", "github:priyaraman/cargo-pin"]);
    assert!(
        search_rows(&out.stdout)
            .iter()
            .any(|r| r.object == CARGO_PIN_TEST_DRIVEN.object()),
        "the new claim is searchable\n{}",
        out.stdout
    );
    assert_eq!(live.times_started(), 1, "serve was never restarted");
}
