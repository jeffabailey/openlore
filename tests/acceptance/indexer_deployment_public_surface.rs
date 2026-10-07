//! indexer-deployment — the public surface of `serve` (US-IXD-001 AC-001.3,
//! NFR-IXD-7; ADR-083) and the new settings `serve` refuses or accepts at start
//! (data-models.md §1; B5, B6, B9, B13, B3).
//!
//! The binary's OWN allowlist and bounds are asserted here, independent of the
//! Caddy site (whose stock directives are checked against the real file in
//! `xtask/tests/indexer_deployment_platform.rs`): a Caddy mistake must not open
//! a write path (ADR-083 §1, two layers).
//!
//! Layer 3 (real `serve`; in-test HTTP client on the public listener).
//! Example-only; the route allowlist and the bounds classification are
//! properties at layer 2 in `indexer_deployment_core.rs`.
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
use indexer_network::{Author, Claim, IndexerWorld};
use support::state_delta::{assert_state_delta, Delta};

/// GIVEN the first pass indexed Priya's and Dmitri's claims in the live index.
fn given_a_live_index_with_priya_and_dmitri_indexed() -> (IndexerWorld, LiveIndex) {
    let world = given_authors_publish_on_their_own_pdses();
    let live = given_the_index_is_live_listing(&world, &[Author::Priya, Author::Dmitri]);
    given_a_pass_indexed(&live);
    (world, live)
}

fn search_body_of_size(total: usize) -> Vec<u8> {
    let skeleton = serde_json::json!({"dimension": "object", "value": "", "pad": ""}).to_string();
    let pad = total.saturating_sub(skeleton.len());
    serde_json::json!({"dimension": "object", "value": "", "pad": "x".repeat(pad)})
        .to_string()
        .into_bytes()
}

/// AS-50
/// ```gherkin
/// @US-IXD-001 @AC-001.3 @FR-IXD-2 @ADR-083 @C6a @error @adversarial @real-io @driving_port
/// @contract-shape:unbounded-preservation
/// Scenario Outline: Anyone trying to write, or to reach anything but search and health, is refused
///   Given the live index holds Priya's and Dmitri's claims
///   When anyone sends <request>
///   Then it is refused as not found
///   And no pass runs and everything Maria finds is exactly as before
///   Examples:
///     | request                                            |
///     | POST /xrpc/com.atproto.repo.createRecord           |
///     | GET  /xrpc/com.atproto.repo.createRecord           |
///     | GET  /admin                                        |
///     | GET  /xrpc/org.openlore.appview.searchClaims       |
///     | PUT  /xrpc/org.openlore.appview.searchClaims       |
///     | POST /healthz                                      |
///     | POST /trigger                                      |
///     | POST /xrpc/org.openlore.indexer.runPass            |
///     | DELETE /                                           |
/// ```
#[test]
fn anyone_trying_to_write_or_to_reach_anything_but_search_and_health_is_refused() {
    let (world, live) = given_a_live_index_with_priya_and_dmitri_indexed();
    let snapshot = |live: &LiveIndex| -> HashMap<String, Vec<String>> {
        [Author::Priya, Author::Dmitri]
            .iter()
            .map(|a| {
                let mut rows: Vec<String> = maria_looks_up(&world, live, *a)
                    .into_iter()
                    .map(|r| r.block.join("\n"))
                    .collect();
                rows.sort();
                (format!("maria.results.by_author[{}]", a.did()), rows)
            })
            .chain(std::iter::once((
                "serve.pass_summaries.count".to_string(),
                vec![live.pass_summaries().len().to_string()],
            )))
            .collect()
    };
    let before = snapshot(&live);
    let attempts = [
        ("POST", "/xrpc/com.atproto.repo.createRecord"),
        ("GET", "/xrpc/com.atproto.repo.createRecord"),
        ("GET", "/admin"),
        ("GET", SEARCH_PATH),
        ("PUT", SEARCH_PATH),
        ("POST", HEALTH_PATH),
        ("POST", "/trigger"),
        ("POST", "/xrpc/org.openlore.indexer.runPass"),
        ("DELETE", "/"),
    ];

    let refused: Vec<(&str, &str, u16)> = attempts
        .iter()
        .map(|(m, p)| {
            let body = serde_json::json!({"repo": "did:plc:mallory4k1z", "record": {}}).to_string();
            (*m, *p, live.request(m, p, body.into_bytes()).0)
        })
        .collect();

    assert!(
        refused.iter().all(|(_, _, status)| *status == 404),
        "every other route is not found: {refused:?}"
    );
    let after = snapshot(&live);
    let universe: HashSet<String> = before.keys().cloned().collect();
    assert_state_delta(&before, &after, &universe, &Delta::new());
}

/// AS-52
/// ```gherkin
/// @US-IXD-001 @NFR-IXD-7 @ADR-083-3 @C1b @error @real-io @contract-shape:pure-function
/// Scenario Outline: Oversized requests are refused at the boundary, not one byte early
///   Given the live index is up
///   When anyone searches with <request>
///   Then the answer is <status>
///   Examples:
///     | request                               | status            |
///     | a body of exactly 8 KiB               | not "too large"   |
///     | a body of 8 KiB and 1 byte            | 413 too large     |
///     | a search value of exactly 512 bytes   | 200               |
///     | a search value of 513 bytes           | 400 bad request   |
/// ```
#[test]
fn oversized_requests_are_refused_at_the_boundary_not_one_byte_early() {
    let world = given_authors_publish_on_their_own_pdses();
    let live = given_the_index_is_live_listing(&world, &[Author::Priya]);

    let (at_limit, _, _) = live.request("POST", SEARCH_PATH, search_body_of_size(8192));
    let (over_limit, _, _) = live.request("POST", SEARCH_PATH, search_body_of_size(8193));
    let (value_at_limit, _, _) = live.public_search("object", &"v".repeat(512));
    let (value_over_limit, body, _) = live.public_search("object", &"v".repeat(513));

    assert_ne!(at_limit, 413, "8 KiB exactly is not too large");
    assert_eq!(over_limit, 413, "8 KiB + 1 is too large");
    assert_eq!(value_at_limit, 200, "a 512-byte value is searched");
    assert_eq!(value_over_limit, 400, "a 513-byte value is refused");
    assert!(
        !body.contains(&"v".repeat(513)),
        "the refusal does not echo the value"
    );
}

/// AS-53
/// ```gherkin
/// @US-IXD-001 @NFR-IXD-7 @ADR-083-3 @C1b @C3 @real-io @slow @contract-shape:bounded-change
/// Scenario: A search matching more than 1000 claims returns 1000 and logs the cut without the query
///   Given Priya has 1001 indexed claims on reproducible builds
///   When anyone searches for reproducible builds
///   Then exactly 1000 attributed results come back
///   And the index logs that a search was cut at 1000 results, without the search value
/// ```
#[test]
fn a_search_matching_more_than_1000_claims_returns_1000_and_logs_the_cut_without_the_query() {
    let world = IndexerWorld::configured_with(&[Author::Priya]);
    let subjects: Vec<String> = (0..1001)
        .map(|i| format!("github:priyaraman/crate-{i:04}"))
        .collect();
    let claims: Vec<Claim> = subjects
        .iter()
        .map(|s| Claim {
            subject: Box::leak(s.clone().into_boxed_str()),
            philosophy: "reproducible-builds",
            basis_points: 6000,
        })
        .collect();
    world.publishes_self_attested(Author::Priya, &claims);
    let live = given_the_index_is_live_listing(&world, &[Author::Priya]);
    given_a_pass_indexed(&live);

    let (status, body, _) = live.public_search("object", REPRODUCIBLE_BUILDS);

    assert_eq!(status, 200, "{body}");
    let response: serde_json::Value = serde_json::from_str(&body).expect("a search response");
    let results = response["results"].as_array().expect("results");
    assert_eq!(results.len(), 1000, "capped at 1000");
    assert!(
        results
            .iter()
            .all(|r| r["author_did"] == Author::Priya.did()),
        "attributed rows only"
    );
    let cut = live.events_named("indexer.search.truncated");
    assert_eq!(cut.len(), 1, "{}", live.dump());
    assert_eq!(cut[0]["cap"].as_u64(), Some(1000), "{}", cut[0]);
    assert_eq!(cut[0]["dimension"], "object", "{}", cut[0]);
    assert!(
        !cut[0].to_string().contains("reproducible-builds"),
        "no query value: {}",
        cut[0]
    );
}

/// AS-54
/// ```gherkin
/// @US-IXD-001 @NFR-IXD-7 @ADR-083-4 @C7a @real-io @contract-shape:unbounded-preservation
/// Scenario: A burst of 100 searches in 10 seconds is answered and leaves the index healthy
///   Given the live index holds Priya's and Dmitri's claims
///   When one client sends 100 searches in 10 seconds
///   Then every search is answered
///   And the index still reports healthy and was never restarted
/// ```
#[test]
fn a_burst_of_100_searches_in_10_seconds_is_answered_and_leaves_the_index_healthy() {
    let (_world, live) = given_a_live_index_with_priya_and_dmitri_indexed();

    let statuses: Vec<u16> = (0..100)
        .map(|_| {
            let (status, _, _) = live.public_search("object", REPRODUCIBLE_BUILDS);
            std::thread::sleep(Duration::from_millis(100));
            status
        })
        .collect();

    assert!(statuses.iter().all(|s| *s == 200), "{statuses:?}");
    assert_eq!(live.health().0, 200, "{}", live.dump());
    assert_eq!(live.times_started(), 1);
}

/// AS-55
/// ```gherkin
/// @US-IXD-001 @US-IXD-006 @AC-006.5 @data-models-1 @C1b @C5a @C6a @error @real-io
/// @contract-shape:pure-function
/// Scenario Outline: The index refuses settings outside their range and accepts their limits
///   Given Jeff sets <setting> to <value>
///   When the index starts
///   Then it <outcome>
///   Examples:
///     | setting                       | value          | outcome                               |
///     | DuckDB memory limit (MB)      | 15             | refuses with exit 2 naming the setting |
///     | DuckDB memory limit (MB)      | 16             | becomes ready                          |
///     | DuckDB memory limit (MB)      | 1024           | becomes ready                          |
///     | DuckDB memory limit (MB)      | 1025           | refuses with exit 2 naming the setting |
///     | DuckDB memory limit (MB)      | lots           | refuses with exit 2 naming the setting |
///     | DuckDB threads                | 0              | refuses with exit 2 naming the setting |
///     | DuckDB threads                | 4              | becomes ready                          |
///     | DuckDB threads                | 5              | refuses with exit 2 naming the setting |
///     | pass deadline (s)             | 59             | refuses with exit 2 naming the setting |
///     | pass deadline (s)             | 60             | becomes ready                          |
///     | pass deadline (s)             | 7200           | becomes ready                          |
///     | pass deadline (s)             | 7201           | refuses with exit 2 naming the setting |
///     | purge unlisted                | yes            | refuses with exit 2 naming the setting |
///     | purge unlisted                | 0              | refuses with exit 2 naming the setting |
///     | control socket                | 0.0.0.0:9000   | refuses with exit 2 naming the setting |
/// ```
#[test]
fn the_index_refuses_settings_outside_their_range_and_accepts_their_limits() {
    let cases: [(&str, &str, bool); 15] = [
        (var::DUCKDB_MEMORY_LIMIT_MB, "15", false),
        (var::DUCKDB_MEMORY_LIMIT_MB, "16", true),
        (var::DUCKDB_MEMORY_LIMIT_MB, "1024", true),
        (var::DUCKDB_MEMORY_LIMIT_MB, "1025", false),
        (var::DUCKDB_MEMORY_LIMIT_MB, "lots", false),
        (var::DUCKDB_THREADS, "0", false),
        (var::DUCKDB_THREADS, "4", true),
        (var::DUCKDB_THREADS, "5", false),
        (var::PASS_DEADLINE_SECS, "59", false),
        (var::PASS_DEADLINE_SECS, "60", true),
        (var::PASS_DEADLINE_SECS, "7200", true),
        (var::PASS_DEADLINE_SECS, "7201", false),
        (var::PURGE_UNLISTED, "yes", false),
        (var::PURGE_UNLISTED, "0", false),
        (var::CONTROL_SOCKET, "0.0.0.0:9000", false),
    ];
    let world = given_authors_publish_on_their_own_pdses();
    let mut wrong = Vec::new();
    for (setting, value, accepted) in cases {
        let startup = LiveIndex::launch(&world, Deployment::listing(&[]).setting(setting, value));
        match (startup, accepted) {
            (Startup::Ready(live), true) => live.stop(),
            (Startup::Refused(report), false) => {
                let named = report
                    .startup_refusal()
                    .map(|r| r["variable"] == setting || r.to_string().contains(setting))
                    .unwrap_or(false);
                if report.status != 2 || !named {
                    wrong.push(format!(
                        "{setting}={value}: refused without naming it\n{}",
                        report.dump()
                    ));
                }
            }
            (Startup::Ready(live), false) => {
                wrong.push(format!("{setting}={value}: accepted"));
                live.stop();
            }
            (Startup::Refused(report), true) => {
                wrong.push(format!("{setting}={value}: refused\n{}", report.dump()))
            }
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n\n"));
}

/// AS-56
/// ```gherkin
/// @US-IXD-001 @US-IXD-006 @AC-001.6 @AC-006.5 @ci-cd-pipeline-3.2 @real-io @driving_port
/// @contract-shape:unbounded-preservation
/// Scenario: The production configuration starts ready and runs a pass
///   Given Jeff deploys with exactly the production settings (list file, socket, purge, 48 MB, 1 thread, 1500 s)
///   And the only listed DID cannot be resolved
///   When the timer fires
///   Then the index is healthy, an empty search answers, and the pass exits 3 with one summary
/// ```
#[test]
fn the_production_configuration_starts_ready_and_runs_a_pass() {
    let world = given_authors_publish_on_their_own_pdses();
    let live = LiveIndex::deploy(&world, Deployment::listing(&[Author::Ghost.did()]));

    let (health, _) = live.health();
    let (search, body, _) = live.public_search("object", REPRODUCIBLE_BUILDS);
    let pass = live.timer_fires();

    assert_eq!(health, 200, "{}", live.dump());
    assert_eq!(search, 200, "{body}");
    assert_one_summary_matching(&live, &pass, PassExit::TotalOutage);
}
