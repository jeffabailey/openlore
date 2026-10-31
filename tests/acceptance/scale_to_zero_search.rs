//! scale-to-zero-hosting — searching the index on Lambda (ADR-090 §3, §5).
//!
//! Driving port: the search function's Function URL entry
//! ([`openlore_indexer_lambda::SearchFunction::handle`], payload 2.0), the same
//! entry CloudFront and the raw Function URL reach. Layer 2 (in-process root,
//! real DuckDB copy, fakes of the AWS ports), example-only; the pure freshness
//! decisions are property-tested in `scale_to_zero_core.rs`.
//!
//! What stays out of reach in-process: reserved concurrency is a Lambda setting,
//! so the fourth concurrent search is OP-6, and SE-6 proves only that the
//! per-client limiter is gone. Requests to the store are a cost, not behaviour:
//! SE-4 is the one counted check (D-STZ-10, at most one check per TTL), and the
//! others let Maria's answers carry the behaviour.
//!
//! State of one warm search instance (C2a):
//!
//! ```text
//!   init ── object present ──> Serving(etag)        init ── no object ──> ServingEmpty
//!   init ── store refuses / damaged ──> Refused (never answers, health.startup.refused)
//!   Serving(e) ── within TTL ──> Serving(e)                  (no store request)
//!   Serving(e) ── TTL due, HEAD e ──> Serving(e)             (HEAD only)
//!   Serving(e) ── TTL due, HEAD e' ──> fetch e', open, swap, delete old ──> Serving(e')
//!   Serving(e) ── TTL due, HEAD absent ──> ServingEmpty
//!   Serving(e) ── TTL due, HEAD/fetch/open fails or stalls ──> Serving(e) + freshness_check_failed
//!   ServingEmpty ── TTL due, HEAD e ──> fetch e, open, swap ──> Serving(e)
//! ```
//!
//! Story line (Pillar 2): SE-1 fresh deployment → SE-2 published index read at
//! start → SE-3 a later publish reaches the warm instance → SE-4 nothing
//! changed (SE-3b: nothing of the old copy stays) → SE-5 the index is reset → SE-7 an empty instance gets its first
//! index → SE-8 two instances. Error paths follow.
#![cfg(unix)]

#[path = "../common/state_delta.rs"]
mod state_delta;

#[path = "support/indexer_lambda.rs"]
mod indexer_lambda;

use std::time::{Duration, Instant};

use indexer_lambda::claims::*;
use indexer_lambda::*;

const SECOND: Duration = Duration::from_secs(1);

/// One SE-B1 row: what it is, the request, and the answer it must get.
struct Bound {
    row: &'static str,
    method: &'static str,
    path: &'static str,
    body: Vec<u8>,
    answer: u16,
}

impl Bound {
    fn new(
        row: &'static str,
        method: &'static str,
        path: &'static str,
        body: &[u8],
        answer: u16,
    ) -> Self {
        Self {
            row,
            method,
            path,
            body: body.to_vec(),
            answer,
        }
    }
}

/// GIVEN Priya's cargo-pin claim is in the published index, and a search
/// instance started after that publish (shared by SE-2..SE-5, SE-E3..SE-E5).
fn given_a_warm_instance_serving_priyas_claim(world: &LambdaWorld) -> SearchInstance<'_> {
    world.publishes(Author::Priya, &[CARGO_PIN_REPRODUCIBLE_BUILDS]);
    world.given_a_pass_published_the_index();
    let search = world.cold_search_instance();
    assert_eq!(
        search
            .maria_searches("reproducible-builds")
            .subjects_by(Author::Priya)
            .len(),
        1,
        "the instance serves Priya's claim"
    );
    search
}

/// GIVEN, after that, Priya approved a second claim and the next pass
/// published it 10 s later.
fn given_the_next_pass_published_priyas_new_claim(world: &LambdaWorld) {
    world.publishes(Author::Priya, &[CARGO_PIN_TEST_DRIVEN]);
    world.time_passes(10 * SECOND);
    let run = world.cold_pass_instance().schedule_fires();
    assert!(run.published(), "{}", run.dump());
}

// =============================================================================
// Happy paths
// =============================================================================

/// SE-1
/// ```gherkin
/// @driving_port @real-io @D-STZ-13 @D-STZ-12 @C1a @edge @contract-shape:unbounded-preservation
/// Scenario: A fresh deployment answers before any index was published
///   Given no index has been published yet
///   When Maria searches for reproducible builds
///   Then she gets an empty result, not an error
///   And the index reports it is healthy with nothing published yet
/// ```
#[test]
fn a_fresh_deployment_answers_before_any_index_was_published() {
    let world = LambdaWorld::listing(&[Author::Priya]);
    let search = world.cold_search_instance();

    let reply = search.maria_searches("reproducible-builds");
    let health = search.health();

    assert_eq!(reply.status, 200, "{}", reply.text());
    assert!(reply.results().is_empty(), "{}", reply.text());
    assert_eq!(health.status, 200, "{}", health.text());
    assert_eq!(health.json()["status"], "ok", "{}", health.text());
    assert!(
        health.json()["last_successful_pass_at"].is_null(),
        "nothing published yet: {}",
        health.text()
    );
}

/// SE-2
/// ```gherkin
/// @driving_port @real-io @D-STZ-12 @contract-shape:unbounded-preservation
/// Scenario: The index reports when the copy it serves was published, not when it last looked
///   Given the 14:00 pass published Priya's claim
///   And a search instance started at 14:00
///   When five minutes later anyone asks the index whether it is healthy
///   Then it reports the index as published at 14:00
/// ```
#[test]
fn the_index_reports_when_the_copy_it_serves_was_published() {
    let world = LambdaWorld::listing(&[Author::Priya]);
    let published_at = world.clock.now();
    let search = given_a_warm_instance_serving_priyas_claim(&world);
    world.time_passes(5 * 60 * SECOND);

    let health = search.health();

    assert_eq!(health.status, 200, "{}", health.text());
    let reported: chrono::DateTime<chrono::Utc> = health.json()["last_successful_pass_at"]
        .as_str()
        .unwrap_or_else(|| panic!("a time: {}", health.text()))
        .parse()
        .expect("RFC 3339");
    assert_eq!(reported, published_at, "{}", health.text());
}

/// SE-3
/// ```gherkin
/// @driving_port @real-io @D-STZ-10 @R-STZ-10 @KPI-IXD-2 @kpi @C2 @contract-shape:bounded-change
/// Scenario: A warm instance picks up a newly published claim within a minute
///   Given a warm search instance serves Priya's cargo-pin claim
///   And the next pass published Priya's new test-driven claim 10 seconds later
///   When Maria searches 30 seconds after the instance last looked
///   Then she does not see the new claim yet
///   When she searches again 61 seconds after the instance last looked
///   Then she finds both of Priya's claims
///   And after one more publish the instance still keeps at most two index sizes in its scratch space
/// ```
#[test]
fn a_warm_instance_picks_up_a_newly_published_claim_within_a_minute() {
    let world = LambdaWorld::listing(&[Author::Priya]);
    let search = given_a_warm_instance_serving_priyas_claim(&world);
    given_the_next_pass_published_priyas_new_claim(&world);
    world.time_passes(20 * SECOND);

    let early = search.maria_searches("test-driven");

    assert!(
        early.subjects_by(Author::Priya).is_empty(),
        "{}",
        early.text()
    );
    world.time_passes(31 * SECOND);

    let late = search.maria_searches("test-driven");

    assert_eq!(
        late.subjects_by(Author::Priya),
        vec![CARGO_PIN_TEST_DRIVEN.subject.to_string()],
        "{}",
        late.text()
    );
    assert_eq!(
        search
            .maria_searches("reproducible-builds")
            .subjects_by(Author::Priya)
            .len(),
        1
    );

    world.publishes(Author::Priya, &[FERRITE_REPRODUCIBLE_BUILDS]);
    let run = world.cold_pass_instance().schedule_fires();
    assert!(run.published(), "{}", run.dump());
    world.time_passes(61 * SECOND);
    assert_eq!(
        search
            .maria_searches("reproducible-builds")
            .subjects_by(Author::Priya)
            .len(),
        2,
        "the second swap happened"
    );
    search.assert_scratch_within_two_index_sizes();
}

/// SE-3b (review note 02-05). A reload brings the next copy into a file of its own, so the
/// new index here holds nothing the old one did: what Maria gets comes from the new file.
/// ```gherkin
/// @driving_port @real-io @D-STZ-10 @R-STZ-10 @C2 @contract-shape:bounded-change
/// Scenario: A reload serves what the new index holds, and nothing only the old one held
///   Given a warm search instance serves Priya's cargo-pin claim
///   And the operator took Priya off the list and listed Dmitri
///   And the next pass published an index holding only Dmitri's claim
///   When Maria searches 61 seconds after the instance last looked
///   Then she finds Dmitri's claim, and no longer finds Priya's
///   And the index reports when the new copy was published
/// ```
#[test]
fn a_reload_serves_what_the_new_index_holds_and_nothing_only_the_old_one_held() {
    let world = LambdaWorld::listing(&[Author::Priya]);
    let search = given_a_warm_instance_serving_priyas_claim(&world);
    world.publishes(Author::Dmitri, &[FERRITE_REPRODUCIBLE_BUILDS]);
    world.operator_lists(&[Author::Dmitri]);
    world.time_passes(10 * SECOND);
    let run = world.cold_pass_instance().schedule_fires();
    assert!(run.published(), "{}", run.dump());
    world.time_passes(61 * SECOND);

    let reply = search.maria_searches("reproducible-builds");

    assert_eq!(
        reply.subjects_by(Author::Dmitri),
        vec![FERRITE_REPRODUCIBLE_BUILDS.subject.to_string()],
        "{}",
        reply.text()
    );
    assert!(
        reply.subjects_by(Author::Priya).is_empty(),
        "{}",
        reply.text()
    );
    let health = search.health();
    let reported = health.json()["last_successful_pass_at"].clone();
    let published = world.bucket.current().and_then(|v| v.published_at);
    let time = |text: Option<&str>| text.map(chrono::DateTime::parse_from_rfc3339);
    assert_eq!(
        time(reported.as_str()),
        time(published.as_deref()),
        "{}",
        health.text()
    );
}

/// SE-4 (the one counted check of this suite: at most one check per TTL, and a
/// warm answer stays warm because nothing is downloaded, D-STZ-10)
/// ```gherkin
/// @driving_port @real-io @D-STZ-10 @C4a @contract-shape:unbounded-preservation
/// Scenario: When nothing was published, a warm instance only checks and keeps its copy
///   Given a warm search instance serves Priya's cargo-pin claim
///   And nothing was published since
///   When Maria searches 61 seconds later, and again 61 seconds after that
///   Then she gets the same answer each time
///   And the instance checked the store once per minute and downloaded nothing
/// ```
#[test]
fn when_nothing_was_published_a_warm_instance_only_checks_and_keeps_its_copy() {
    let world = LambdaWorld::listing(&[Author::Priya]);
    let search = given_a_warm_instance_serving_priyas_claim(&world);
    let first = search.maria_searches("reproducible-builds");
    let requests_so_far = world.bucket.requests().len();

    world.time_passes(61 * SECOND);
    let second = search.maria_searches("reproducible-builds");
    world.time_passes(61 * SECOND);
    let third = search.maria_searches("reproducible-builds");

    assert_eq!(first.results(), second.results());
    assert_eq!(second.results(), third.results());
    assert_eq!(
        world.bucket.requests()[requests_so_far..],
        [BucketRequest::Head, BucketRequest::Head],
        "one check per TTL and no download"
    );
}

/// SE-5
/// ```gherkin
/// @driving_port @real-io @D-STZ-13 @D-STZ-27 @C4b @contract-shape:bounded-change
/// Scenario: After the operator resets the index, warm instances answer empty until the next pass
///   Given a warm search instance serves Priya's cargo-pin claim
///   And the operator reset the index
///   When Maria searches 61 seconds later
///   Then she gets an empty result, not an error
///   And the index reports it is healthy with nothing published
/// ```
#[test]
fn after_the_operator_resets_the_index_warm_instances_answer_empty_until_the_next_pass() {
    let world = LambdaWorld::listing(&[Author::Priya]);
    let search = given_a_warm_instance_serving_priyas_claim(&world);
    world.bucket.operator_resets_the_index();
    world.time_passes(61 * SECOND);

    let reply = search.maria_searches("reproducible-builds");
    let health = search.health();

    assert_eq!(reply.status, 200, "{}", reply.text());
    assert!(reply.results().is_empty(), "{}", reply.text());
    assert_eq!(health.status, 200, "{}", health.text());
    assert!(
        health.json()["last_successful_pass_at"].is_null(),
        "{}",
        health.text()
    );
}

/// SE-6 (the per-client limiter is gone. These searches run one after another:
/// the global cap is OP-6, because reserved concurrency cannot be reached
/// in-process)
/// ```gherkin
/// @driving_port @real-io @D-STZ-19 @contract-shape:unbounded-preservation
/// Scenario: Serial searches from one client are all answered
///   Given a warm search instance serves Priya's cargo-pin claim
///   When one client sends 60 searches back to back
///   Then every search is answered with Priya's claim, and none is told to slow down
/// ```
#[test]
fn serial_searches_from_one_client_are_all_answered() {
    let world = LambdaWorld::listing(&[Author::Priya]);
    let search = given_a_warm_instance_serving_priyas_claim(&world);

    let answers: Vec<Reply> = (0..60)
        .map(|_| {
            search.request_from(
                "198.51.100.23",
                "POST",
                SEARCH_PATH,
                &search_body("reproducible-builds"),
            )
        })
        .collect();

    assert!(
        answers
            .iter()
            .all(|a| a.status == 200 && a.subjects_by(Author::Priya).len() == 1),
        "{:?}",
        answers.iter().map(|a| a.status).collect::<Vec<_>>()
    );
}

/// SE-7
/// ```gherkin
/// @driving_port @real-io @D-STZ-10 @D-STZ-13 @C2 @contract-shape:bounded-change
/// Scenario: A warm instance on a fresh deployment picks up the first published index
///   Given a search instance started before any index was published
///   And Maria got an empty result from it
///   And the 14:00 pass then published Priya's claim
///   When Maria searches again 61 seconds later
///   Then she finds Priya's claim
///   And the index reports the index as published at 14:00
/// ```
#[test]
fn a_warm_instance_on_a_fresh_deployment_picks_up_the_first_published_index() {
    let world = LambdaWorld::listing(&[Author::Priya]);
    let search = world.cold_search_instance();
    assert!(search
        .maria_searches("reproducible-builds")
        .results()
        .is_empty());
    let published_at = world.clock.now();
    world.publishes(Author::Priya, &[CARGO_PIN_REPRODUCIBLE_BUILDS]);
    world.given_a_pass_published_the_index();
    world.time_passes(61 * SECOND);

    let reply = search.maria_searches("reproducible-builds");

    assert_eq!(
        reply.subjects_by(Author::Priya).len(),
        1,
        "{}",
        reply.text()
    );
    let health = search.health();
    let reported: chrono::DateTime<chrono::Utc> = health.json()["last_successful_pass_at"]
        .as_str()
        .unwrap_or_else(|| panic!("a time: {}", health.text()))
        .parse()
        .expect("RFC 3339");
    assert_eq!(reported, published_at, "{}", health.text());
    search.assert_scratch_within_two_index_sizes();
}

/// SE-8
/// ```gherkin
/// @driving_port @real-io @D-STZ-10 @R-STZ-10 @C7c @contract-shape:bounded-change
/// Scenario: Two warm instances disagree for at most about a minute after a publish
///   Given one warm search instance last looked at 14:00:00
///   And another last looked at 14:00:50
///   And the 14:00:55 pass published Priya's new test-driven claim
///   When Maria searches at 14:01:01, once on each instance
///   Then the first finds the new claim and the second does not yet
///   When she searches at 14:01:51, once on each instance
///   Then both find it, 56 seconds after the publish
/// ```
#[test]
fn two_warm_instances_disagree_for_at_most_about_a_minute_after_a_publish() {
    let world = LambdaWorld::listing(&[Author::Priya]);
    let first = given_a_warm_instance_serving_priyas_claim(&world);
    world.time_passes(50 * SECOND);
    let second = world.cold_search_instance();
    second.maria_searches("reproducible-builds");
    world.publishes(Author::Priya, &[CARGO_PIN_TEST_DRIVEN]);
    world.time_passes(5 * SECOND);
    let run = world.cold_pass_instance().schedule_fires();
    assert!(run.published(), "{}", run.dump());
    let finds_it = |search: &SearchInstance<'_>| {
        !search
            .maria_searches("test-driven")
            .subjects_by(Author::Priya)
            .is_empty()
    };

    world.time_passes(6 * SECOND);

    assert!(finds_it(&first), "61 s after it last looked");
    assert!(!finds_it(&second), "11 s after it last looked");

    world.time_passes(50 * SECOND);

    assert!(finds_it(&first));
    assert!(finds_it(&second), "61 s after it last looked");
}

// =============================================================================
// Bounds (identical to the self-host `serve`, D-STZ-22)
// =============================================================================

/// SE-B1 (the HTTP detail of each row is in the test body)
/// ```gherkin
/// @driving_port @real-io @D-STZ-22 @ADR-083 @C1b @C6a @error @adversarial
/// @contract-shape:unbounded-preservation
/// Scenario Outline: The search entry turns away what the self-hosted index turns away
///   Given a warm search instance serves Priya's cargo-pin claim
///   When anyone sends <request>
///   Then the entry <answers>
///   And nothing anyone finds changes
///   Examples:
///     | request                                          | answers                   |
///     | a search of exactly the largest allowed size     | answers it                |
///     | a search one byte over the largest allowed size  | refuses it as too large   |
///     | a search term of exactly the longest length      | answers it                |
///     | a search term one byte over the longest length   | refuses it as malformed   |
///     | something that is not a search                   | refuses it as malformed   |
///     | a search with nothing in it                      | refuses it as malformed   |
///     | an attempt to write a record                     | knows no such place       |
///     | a look at the write address                      | knows no such place       |
///     | a search sent the wrong way                      | knows no such place       |
///     | a change sent to the search address              | knows no such place       |
///     | a change sent to the health check                | knows no such place       |
///     | a request to start a pass                        | knows no such place       |
///     | a look at the front page                         | knows no such place       |
///     | a look at an unknown address                     | knows no such place       |
/// ```
#[test]
fn the_search_entry_turns_away_what_the_self_hosted_index_turns_away() {
    let world = LambdaWorld::listing(&[Author::Priya]);
    let search = given_a_warm_instance_serving_priyas_claim(&world);
    let before = search.maria_searches("reproducible-builds").results();
    let write = br#"{"repo":"did:plc:mallory4k1z","record":{}}"#;
    let create_record = "/xrpc/com.atproto.repo.createRecord";
    let cases = [
        Bound::new(
            "largest",
            "POST",
            SEARCH_PATH,
            &search_body_of_size(8192),
            200,
        ),
        Bound::new(
            "too large",
            "POST",
            SEARCH_PATH,
            &search_body_of_size(8193),
            413,
        ),
        Bound::new(
            "longest",
            "POST",
            SEARCH_PATH,
            &search_body_with_value_of(512),
            200,
        ),
        Bound::new(
            "too long",
            "POST",
            SEARCH_PATH,
            &search_body_with_value_of(513),
            400,
        ),
        Bound::new("not a search", "POST", SEARCH_PATH, b"not a search", 400),
        Bound::new("empty", "POST", SEARCH_PATH, b"", 400),
        Bound::new("write", "POST", create_record, write, 404),
        Bound::new("look at write", "GET", create_record, b"", 404),
        Bound::new("wrong way", "GET", SEARCH_PATH, b"", 404),
        Bound::new("change search", "PUT", SEARCH_PATH, write, 404),
        Bound::new("change health", "POST", HEALTH_PATH, b"", 404),
        Bound::new("start a pass", "POST", "/trigger", b"", 404),
        Bound::new("front page", "GET", "/", b"", 404),
        Bound::new("unknown", "GET", "/xrpc/x", b"", 404),
    ];

    let wrong: Vec<(&str, u16)> = cases
        .iter()
        .map(|c| (c, search.request(c.method, c.path, &c.body).status))
        .filter(|(c, status)| *status != c.answer)
        .map(|(c, status)| (c.row, status))
        .collect();

    assert!(wrong.is_empty(), "answers outside the bounds: {wrong:?}");
    let refusal = search.request("POST", SEARCH_PATH, &search_body_with_value_of(513));
    assert!(
        !refusal.text().contains(&"v".repeat(513)),
        "the refusal does not echo the value"
    );
    assert_eq!(
        search.maria_searches("reproducible-builds").results(),
        before
    );
}

// =============================================================================
// Error paths
// =============================================================================

/// SE-E1
/// ```gherkin
/// @driving_port @real-io @D-STZ-22 @ADR-083 @C7b @error @contract-shape:bounded-change
/// Scenario: A search the index cannot answer within 5 seconds is told to retry shortly
///   Given a warm search instance serves Priya's cargo-pin claim
///   And the index is too slow to answer within the 5-second budget
///   When Maria searches for reproducible builds
///   Then she is told the index is busy and to retry in a second
/// ```
#[test]
fn a_search_the_index_cannot_answer_within_5_seconds_is_told_to_retry_shortly() {
    let world = LambdaWorld::listing(&[Author::Priya]);
    let search = given_a_warm_instance_under_fault(&world, Fault::SearchStoreIsSlow);

    let reply = search.maria_searches("reproducible-builds");

    assert_eq!(reply.status, 503, "{}", reply.text());
    assert_eq!(reply.json()["error"], "index_busy", "{}", reply.text());
    assert_eq!(
        reply.headers.get("retry-after").map(String::as_str),
        Some("1")
    );
}

/// The fault hits every search read, so the precondition check of the shared
/// Given cannot run; this one publishes with a clean pass, then starts an
/// instance under the fault.
fn given_a_warm_instance_under_fault(world: &LambdaWorld, fault: Fault) -> SearchInstance<'_> {
    world.publishes(Author::Priya, &[CARGO_PIN_REPRODUCIBLE_BUILDS]);
    world.given_a_pass_published_the_index();
    world.with_fault(fault);
    world.cold_search_instance()
}

/// SE-E2
/// ```gherkin
/// @driving_port @real-io @D-STZ-22 @ADR-083 @C6b @error @contract-shape:bounded-change
/// Scenario: A search the index cannot read fails loudly, never as an empty result
///   Given a warm search instance whose index cannot be read
///   When Maria searches for reproducible builds
///   Then the search fails, saying the index is unavailable
///   And she is not told that nothing matched
/// ```
#[test]
fn a_search_the_index_cannot_read_fails_loudly_never_as_an_empty_result() {
    let world = LambdaWorld::listing(&[Author::Priya]);
    let search = given_a_warm_instance_under_fault(&world, Fault::SearchCannotReadTheStore);

    let reply = search.maria_searches("reproducible-builds");

    assert_eq!(reply.status, 500, "{}", reply.text());
    assert_eq!(
        reply.json()["error"],
        "index_unavailable",
        "{}",
        reply.text()
    );
    assert!(
        reply.json()["results"].is_null(),
        "no result list: {}",
        reply.text()
    );
}

/// SE-E6 (ADR-090 §3 refuses an unusable index at init, which is SE-E5. This is
/// a store that was usable when the instance started and broke while it ran:
/// the instance has already started, so the answer is a 503.)
/// ```gherkin
/// @driving_port @real-io @ADR-083-2 @D-STZ-22 @C6b @error @contract-shape:bounded-change
/// Scenario: An index whose store breaks while it runs says so instead of looking healthy
///   Given a warm search instance reports it is healthy
///   And its store became unusable after it answered Maria's search
///   When anyone asks the index whether it is healthy
///   Then it answers that its store is unusable, not that it is ok
/// ```
#[test]
fn an_index_whose_store_breaks_while_it_runs_says_so_instead_of_looking_healthy() {
    let world = LambdaWorld::listing(&[Author::Priya]);
    let search = given_a_warm_instance_under_fault(&world, Fault::StorePoisonedAfterFirstSearch);
    let healthy = search.health();
    assert_eq!(healthy.status, 200, "{}", healthy.text());
    assert_eq!(healthy.json()["status"], "ok", "{}", healthy.text());
    let reply = search.maria_searches("reproducible-builds");
    assert_eq!(reply.status, 200, "{}", reply.text());

    let health = search.health();

    assert_eq!(health.status, 503, "{}", health.text());
    assert_eq!(
        health.json()["status"],
        "store_unusable",
        "{}",
        health.text()
    );
}

/// SE-E3
/// ```gherkin
/// @driving_port @real-io @D-STZ-11 @C7a @error @contract-shape:bounded-change
/// Scenario: When the freshness check fails, Maria still gets an answer from the current copy
///   Given a warm search instance serves Priya's cargo-pin claim
///   And the next pass published Priya's new test-driven claim
///   And the store cannot be checked right now
///   When Maria searches 61 seconds after the instance last looked
///   Then she gets Priya's cargo-pin claim from the copy the instance holds
///   And the instance logs that its freshness check failed, without the search value
///   And on her next search a second later it does not try again
///   And once the store answers again, a search a minute later finds the new claim
/// ```
#[test]
fn when_the_freshness_check_fails_maria_still_gets_an_answer_from_the_current_copy() {
    let world = LambdaWorld::listing(&[Author::Priya]);
    let search = given_a_warm_instance_serving_priyas_claim(&world);
    given_the_next_pass_published_priyas_new_claim(&world);
    world.bucket.has_trouble(ObjectTrouble::HeadFails);
    world.time_passes(61 * SECOND);
    let mark = world.events.mark();

    let reply = search.maria_searches("reproducible-builds");

    assert_eq!(reply.status, 200, "{}", reply.text());
    assert_eq!(
        reply.subjects_by(Author::Priya).len(),
        1,
        "{}",
        reply.text()
    );
    let failures = named(
        &world.events.since(mark),
        "indexer.lambda.freshness_check_failed",
    );
    assert_eq!(failures.len(), 1, "{:?}", world.events.since(mark));
    assert!(
        !failures[0].to_string().contains("reproducible-builds"),
        "no search value in the event: {}",
        failures[0]
    );
    world.time_passes(SECOND);
    search.maria_searches("reproducible-builds");
    assert_eq!(
        named(
            &world.events.since(mark),
            "indexer.lambda.freshness_check_failed"
        )
        .len(),
        1,
        "it retries after the next TTL, not on every search"
    );
    world.bucket.recovers();
    world.time_passes(60 * SECOND);
    assert_eq!(
        search
            .maria_searches("test-driven")
            .subjects_by(Author::Priya)
            .len(),
        1,
        "the new claim after recovery"
    );
}

/// SE-E4
/// ```gherkin
/// @driving_port @real-io @D-STZ-10 @D-STZ-11 @C6a @C7a @error @contract-shape:bounded-change
/// Scenario Outline: A newer index the instance cannot use leaves Maria on the current copy
///   Given a warm search instance serves Priya's cargo-pin claim
///   And <trouble> with the newer index
///   When Maria searches 61 seconds after the instance last looked, three times over
///   Then she gets Priya's cargo-pin claim from the copy the instance holds, each time
///   And the instance logs each failed freshness check
///   And failed downloads do not fill its scratch space
///   Examples:
///     | trouble                                         |
///     | the newer index is damaged                      |
///     | the instance has no room left to download       |
/// ```
#[test]
fn a_newer_index_the_instance_cannot_use_leaves_maria_on_the_current_copy() {
    each_row(["damaged", "no room"], |trouble| {
        let world = LambdaWorld::listing(&[Author::Priya]);
        let search = given_a_warm_instance_serving_priyas_claim(&world);
        let served = world.bucket.current().map_or(0, |v| v.bytes.len() as u64);
        match *trouble {
            "damaged" => world.bucket.holds_a_damaged_index(),
            _ => {
                given_the_next_pass_published_priyas_new_claim(&world);
                world.bucket.has_trouble(ObjectTrouble::ScratchSpaceFull);
            }
        }
        let mark = world.events.mark();

        for _ in 0..3 {
            world.time_passes(61 * SECOND);
            let reply = search.maria_searches("reproducible-builds");
            assert_eq!(reply.status, 200, "{}", reply.text());
            assert_eq!(
                reply.subjects_by(Author::Priya).len(),
                1,
                "{}",
                reply.text()
            );
        }

        assert_eq!(
            named(
                &world.events.since(mark),
                "indexer.lambda.freshness_check_failed"
            )
            .len(),
            3
        );
        search.assert_scratch_within_two_sizes_of(served);
    });
}

/// SE-E7 (the fake takes the adapter's 1 s operation timeout; RIO-9 holds a real
/// client to that kind of bound)
/// ```gherkin
/// @driving_port @real-io @D-STZ-10 @D-STZ-11 @C7b @error @contract-shape:bounded-change
/// Scenario: A freshness check that stalls still leaves Maria an answer from the current copy
///   Given a warm search instance serves Priya's cargo-pin claim
///   And the next pass published Priya's new test-driven claim
///   And the store stops answering checks
///   When Maria searches 61 seconds after the instance last looked
///   Then she gets Priya's cargo-pin claim from the copy the instance holds, well inside her 5 seconds
///   And the instance logs that its freshness check failed
/// ```
#[test]
fn a_freshness_check_that_stalls_still_leaves_maria_an_answer_from_the_current_copy() {
    let world = LambdaWorld::listing(&[Author::Priya]);
    let search = given_a_warm_instance_serving_priyas_claim(&world);
    given_the_next_pass_published_priyas_new_claim(&world);
    world.bucket.has_trouble(ObjectTrouble::HeadStalls);
    world.time_passes(61 * SECOND);
    let mark = world.events.mark();
    let started = Instant::now();

    let reply = search.maria_searches("reproducible-builds");

    assert!(started.elapsed() < 5 * SECOND, "{:?}", started.elapsed());
    assert_eq!(reply.status, 200, "{}", reply.text());
    assert_eq!(
        reply.subjects_by(Author::Priya).len(),
        1,
        "{}",
        reply.text()
    );
    assert_eq!(
        named(
            &world.events.since(mark),
            "indexer.lambda.freshness_check_failed"
        )
        .len(),
        1
    );
}

/// SE-E8
/// ```gherkin
/// @driving_port @real-io @D-STZ-10 @D-STZ-11 @C7b @error @contract-shape:bounded-change
/// Scenario: A reload that stalls is abandoned, and Maria stays on the current copy
///   Given a warm search instance serves Priya's cargo-pin claim
///   And the next pass published Priya's new test-driven claim
///   And its download stalls half way
///   When Maria searches 61 seconds after the instance last looked
///   Then she gets Priya's cargo-pin claim, well inside her 5 seconds, and does not see the new claim
///   And the instance logs that its freshness check failed
///   And the half-downloaded file is gone, and the index it reports is still the one it had
///   And once the store answers again, a search a minute later finds the new claim
/// ```
#[test]
fn a_reload_that_stalls_is_abandoned_and_maria_stays_on_the_current_copy() {
    let world = LambdaWorld::listing(&[Author::Priya]);
    let search = given_a_warm_instance_serving_priyas_claim(&world);
    let held = search.scratch_bytes();
    let reported = search.health().json()["last_successful_pass_at"].clone();
    given_the_next_pass_published_priyas_new_claim(&world);
    world.bucket.has_trouble(ObjectTrouble::DownloadStalls);
    world.time_passes(61 * SECOND);
    let mark = world.events.mark();
    let started = Instant::now();

    let reply = search.maria_searches("reproducible-builds");

    assert!(started.elapsed() < 5 * SECOND, "{:?}", started.elapsed());
    assert_eq!(reply.status, 200, "{}", reply.text());
    assert_eq!(
        reply.subjects_by(Author::Priya).len(),
        1,
        "{}",
        reply.text()
    );
    assert!(
        search
            .maria_searches("test-driven")
            .subjects_by(Author::Priya)
            .is_empty(),
        "no swap"
    );
    assert_eq!(
        named(
            &world.events.since(mark),
            "indexer.lambda.freshness_check_failed"
        )
        .len(),
        1
    );
    assert!(search.scratch_bytes() <= held, "the partial file is gone");
    assert_eq!(search.health().json()["last_successful_pass_at"], reported);
    world.bucket.recovers();
    world.time_passes(61 * SECOND);
    assert_eq!(
        search
            .maria_searches("test-driven")
            .subjects_by(Author::Priya)
            .len(),
        1,
        "the new claim after recovery"
    );
}

/// SE-E9 (D-STZ-10: on Lambda the 5 s covers the gate and the query together. A
/// query given a fresh 5 s after a gate of about 3 s would answer at about 8 s.)
/// ```gherkin
/// @driving_port @real-io @D-STZ-10 @D-STZ-22 @ADR-083 @C7b @error @contract-shape:bounded-change
/// Scenario: After a slow freshness check, a search has only what is left of its 5 seconds
///   Given a warm search instance whose index is too slow to answer within 5 seconds
///   And the next pass published Priya's new test-driven claim, and its download stalls half way
///   When Maria searches 61 seconds after the instance last looked
///   Then she is told the index is busy and to retry in a second
///   And she is told that at about 5 seconds, not 5 seconds after the check gave up
/// ```
#[test]
fn after_a_slow_freshness_check_a_search_has_only_what_is_left_of_its_5_seconds() {
    let world = LambdaWorld::listing(&[Author::Priya]);
    let search = given_a_warm_instance_under_fault(&world, Fault::SearchStoreIsSlow);
    world.without_a_fault();
    given_the_next_pass_published_priyas_new_claim(&world);
    world.bucket.has_trouble(ObjectTrouble::DownloadStalls);
    world.time_passes(61 * SECOND);
    let started = Instant::now();

    let reply = search.maria_searches("reproducible-builds");

    assert_eq!(reply.status, 503, "{}", reply.text());
    assert_eq!(reply.json()["error"], "index_busy", "{}", reply.text());
    assert!(
        started.elapsed() < 6500 * Duration::from_millis(1),
        "{:?}",
        started.elapsed()
    );
}

/// SE-E5
/// ```gherkin
/// @driving_port @real-io @D-STZ-14 @ADR-083-2 @C6b @C7a @error @contract-shape:unbounded-preservation
/// Scenario Outline: A search instance that cannot get a usable index never starts answering
///   Given <trouble> when a search instance starts
///   When the instance starts
///   Then it refuses to start and logs that its startup was refused
///   Examples:
///     | trouble                                  |
///     | the store refuses the request            |
///     | the store is unavailable                 |
///     | the published index is damaged           |
///     | the instance has no room to download it  |
/// ```
#[test]
fn a_search_instance_that_cannot_get_a_usable_index_never_starts_answering() {
    each_row(
        ["refuses", "unavailable", "damaged", "no room"],
        |trouble| {
            let world = LambdaWorld::listing(&[Author::Priya]);
            world.publishes(Author::Priya, &[CARGO_PIN_REPRODUCIBLE_BUILDS]);
            world.given_a_pass_published_the_index();
            match *trouble {
                "refuses" => world.bucket.has_trouble(ObjectTrouble::Refuses),
                "unavailable" => world.bucket.has_trouble(ObjectTrouble::Unavailable),
                "damaged" => world.bucket.holds_a_damaged_index(),
                _ => world.bucket.has_trouble(ObjectTrouble::ScratchSpaceFull),
            }
            let mark = world.events.mark();

            let started = world.search_instance_starts();

            assert!(started.is_err(), "init must be refused");
            let refusals = named(&world.events.since(mark), "health.startup.refused");
            assert_eq!(refusals.len(), 1, "{:?}", world.events.since(mark));
        },
    );
}
