//! indexer-per-did-pds-fetch — one unreachable author never blocks the others
//! (US-IPF-002), bounded fan-out and the total-outage exit code (ADR-078), and
//! the SSRF guard on resolved PDS addresses (DD-IPF-5, AC-002.9).
//!
//! Layer 3 (subprocess + real `index.duckdb`), example-only: every failure mode
//! of architecture-design.md §9 is a NAMED example (Mandate 11), never
//! generated. The pure classification behind them (`classify_fetch_failure`,
//! `plan_listing`, `pass_exit_code`) is property-tested at layer 2 in
//! `indexer_pass_core.rs`.
//!
//! ## The per-DID unit as a state machine (C2a; architecture-design.md §6)
//!
//! ```text
//! configured ──resolve_pds──▶ resolved(url) ──pre-check ok──▶ listing(own PDS) ──ok──▶ READ(own_pds)
//!     │                          └─pre-check refused──▶ SKIPPED(pds_address_refused)   [never → fallback]
//!     └─resolution failed ──fallback set──▶ listing(fallback) ──ok──▶ READ(fallback)
//!                         └─no fallback──▶ SKIPPED(did_unresolvable)
//! listing(own PDS) ──unreachable/5xx/429──▶ SKIPPED(pds_unreachable) │ ──bad response/3xx──▶ SKIPPED(listing_failed)
//!                  ──deadline──▶ SKIPPED(pds_timeout)
//! listing(fallback) ──any failure──▶ SKIPPED(did_unresolvable, fallback_used, fallback_failure)
//! ```
//! Terminal states are READ / SKIPPED; every pass restarts every DID at
//! `configured` (nothing about a skip persists). Illegal transitions asserted:
//! resolved → fallback (IPF-05/IPF-20), skipped → rows removed (IPF-14/IPF-19).
//!
//! Story line (Pillar 2): IPF-10 (one PDS down) → IPF-14 (earlier claims stay)
//! → IPF-16 (retried next pass) share `given_three_authors_with_dmitri_indexed…`.
//!
//! All scenarios are `#[ignore]`d at DISTILL hand-off.
//
// SCAFFOLD: true

mod support;

#[path = "support/indexer_network.rs"]
mod indexer_network;

use std::time::Duration;

use indexer_network::claims::*;
use indexer_network::*;

/// GIVEN Priya (3 self-attested), Dmitri (2 app-signed) and Jeff (1 app-signed)
/// each publish on their own PDS; no fallback.
fn given_three_authors_publish_on_their_own_pdses() -> IndexerWorld {
    let mut world = IndexerWorld::configured_with(&[Author::Priya, Author::Dmitri, Author::Jeff]);
    world.publishes_self_attested(
        Author::Priya,
        &[
            CARGO_PIN_REPRODUCIBLE_BUILDS,
            CARGO_PIN_DEPENDENCY_PINNING,
            TIDEPOOL_MEMORY_SAFETY,
        ],
    );
    world.publishes_app_signed(
        Author::Dmitri,
        &[FERRITE_REPRODUCIBLE_BUILDS, FERRITE_DEPENDENCY_PINNING],
    );
    world.publishes_app_signed(Author::Jeff, &[OPENLORE_LOCAL_FIRST]);
    world
}

/// GIVEN … and a first pass already indexed everyone (Dmitri's 2 claims included).
fn given_three_authors_with_dmitri_indexed_on_an_earlier_pass() -> IndexerWorld {
    let world = given_three_authors_publish_on_their_own_pdses();
    world
        .one_ingest_pass_runs()
        .assert_pass_completed(0, 3, 0, 0);
    assert_eq!(world.rows_of(Author::Dmitri).len(), 2);
    world
}

// =============================================================================
// Isolation and reasons
// =============================================================================

/// IPF-10
/// ```gherkin
/// @US-IPF-002 @AC-002.1 @AC-002.3 @I-IPF-3 @error @real-io @driving_port @contract-shape:bounded-change
/// Scenario: One PDS being down does not hide the others
///   Given pds.volkov.dev answers every listing with 502
///   When one ingest pass runs
///   Then Priya's and Jeff's claims are indexed
///   And Jeff sees did:plc:dvolkov3m9q skipped with reason pds_unreachable and its PDS address
///   And the skip notice carries no claim content
///   And the pass completes successfully
/// ```
#[test]
fn one_pds_being_down_does_not_hide_the_others() {
    let world = given_three_authors_publish_on_their_own_pdses();
    world.host_answers(Host::VolkovDev, ListingPosture::Status(502));

    let pass = world.one_ingest_pass_runs();

    pass.assert_pass_completed(0, 2, 0, 1);
    let skip = pass.assert_skipped(Author::Dmitri.did(), SkipReason::PdsUnreachable);
    assert_eq!(skip["pds_url"], world.url_of(Host::VolkovDev), "{skip}");
    assert_eq!(skip["fallback_used"], false, "{skip}");
    assert_eq!(world.rows_of(Author::Priya).len(), 3);
    assert_eq!(world.rows_of(Author::Jeff).len(), 1);
    assert!(world.rows_of(Author::Dmitri).is_empty());
}

/// IPF-11
/// ```gherkin
/// @US-IPF-002 @AC-002.1 @AC-002.3 @error @adversarial @real-io @contract-shape:bounded-change
/// Scenario Outline: Every way an author's PDS can fail skips only that author, with its reason
///   Given Dmitri's PDS <misbehaves>
///   When one ingest pass runs
///   Then Dmitri is skipped with reason <reason>
///   And Priya's and Jeff's claims are indexed and the pass completes successfully
///   Examples:
///     | misbehaves                                   | reason          |
///     | is down (drops every connection)             | pds_unreachable |
///     | answers 503                                  | pds_unreachable |
///     | answers 429 (rate limited)                   | pds_unreachable |
///     | answers 404                                  | listing_failed  |
///     | answers a maintenance page instead of records| listing_failed  |
///     | redirects to another address                 | listing_failed  |
/// ```
#[test]
fn every_way_an_authors_pds_can_fail_skips_only_that_author_with_its_reason() {
    let redirect_target = Tripwire::arm();
    let cases: Vec<(&str, Option<ListingPosture>, SkipReason)> = vec![
        ("is down", None, SkipReason::PdsUnreachable),
        (
            "answers 503",
            Some(ListingPosture::Status(503)),
            SkipReason::PdsUnreachable,
        ),
        (
            "answers 429",
            Some(ListingPosture::Status(429)),
            SkipReason::PdsUnreachable,
        ),
        (
            "answers 404",
            Some(ListingPosture::Status(404)),
            SkipReason::ListingFailed,
        ),
        (
            "answers a maintenance page",
            Some(ListingPosture::NotJson),
            SkipReason::ListingFailed,
        ),
        (
            "redirects elsewhere",
            Some(ListingPosture::RedirectTo(format!(
                "http://127.0.0.1:{}/xrpc/com.atproto.repo.listRecords",
                redirect_target.port()
            ))),
            SkipReason::ListingFailed,
        ),
    ];
    for (misbehaviour, posture, reason) in cases {
        let world = given_three_authors_publish_on_their_own_pdses();
        match posture {
            Some(p) => world.host_answers(Host::VolkovDev, p),
            None => world.host_is_reachable(Host::VolkovDev, false),
        }

        let pass = world.one_ingest_pass_runs();

        assert_eq!(
            pass.status,
            0,
            "Dmitri's PDS {misbehaviour}\n{}",
            pass.dump()
        );
        pass.assert_skipped(Author::Dmitri.did(), reason);
        assert_eq!(
            world.rows_of(Author::Priya).len(),
            3,
            "Dmitri's PDS {misbehaviour}"
        );
        assert_eq!(
            world.rows_of(Author::Jeff).len(),
            1,
            "Dmitri's PDS {misbehaviour}"
        );
    }
    assert_eq!(
        redirect_target.connections(),
        0,
        "redirects are never followed"
    );
}

/// IPF-12
/// ```gherkin
/// @US-IPF-002 @AC-002.2 @AC-002.3 @error @real-io @contract-shape:bounded-change
/// Scenario: An unresolvable author is skipped with a reason
///   Given did:plc:ghost0000 cannot be resolved and no fallback is configured
///   When one ingest pass runs
///   Then the other configured authors are indexed
///   And Jeff sees did:plc:ghost0000 skipped with reason did_unresolvable, without a PDS address
/// ```
#[test]
fn an_unresolvable_author_is_skipped_with_a_reason() {
    let world = IndexerWorld::configured_with(&[Author::Priya, Author::Ghost]);
    world.publishes_self_attested(Author::Priya, &[CARGO_PIN_REPRODUCIBLE_BUILDS]);

    let pass = world.one_ingest_pass_runs();

    pass.assert_pass_completed(0, 1, 0, 1);
    let skip = pass.assert_skipped(Author::Ghost.did(), SkipReason::DidUnresolvable);
    assert_eq!(skip["fallback_used"], false, "{skip}");
    assert!(
        skip.get("pds_url").is_none() || skip["pds_url"].is_null(),
        "{skip}"
    );
    assert_eq!(world.rows_of(Author::Priya).len(), 1);
}

/// IPF-13
/// ```gherkin
/// @US-IPF-002 @AC-002.2 @DD-IPF-4 @error @adversarial @real-io @contract-shape:bounded-change
/// Scenario Outline: Every way a DID document can fail makes that author unresolvable
///   Given Priya's DID document <fails>
///   When one ingest pass runs
///   Then Priya is skipped with reason did_unresolvable and Dmitri is indexed
///   Examples:
///     | fails                                              |
///     | is not found                                       |
///     | cannot be served (directory error)                 |
///     | describes a different DID                          |
///     | names no PDS                                       |
/// ```
#[test]
fn every_way_a_did_document_can_fail_makes_that_author_unresolvable() {
    for posture in [
        DidDocPosture::NotFound,
        DidDocPosture::ServerError,
        DidDocPosture::IdMismatch,
        DidDocPosture::NoPdsService,
    ] {
        let mut world = IndexerWorld::configured_with(&[Author::Priya, Author::Dmitri]);
        world.publishes_self_attested(Author::Priya, &[CARGO_PIN_REPRODUCIBLE_BUILDS]);
        world.publishes_app_signed(Author::Dmitri, &[FERRITE_REPRODUCIBLE_BUILDS]);
        world.did_document_of(Author::Priya, posture.clone());

        let pass = world.one_ingest_pass_runs();

        assert_eq!(pass.status, 0, "{posture:?}\n{}", pass.dump());
        pass.assert_skipped(Author::Priya.did(), SkipReason::DidUnresolvable);
        assert!(
            world.listings_on(Host::MorelBsky).is_empty(),
            "{posture:?}: no listing"
        );
        assert_eq!(world.rows_of(Author::Dmitri).len(), 1, "{posture:?}");
    }
}

/// IPF-14
/// ```gherkin
/// @US-IPF-002 @AC-002.4 @I-IPF-3 @error @real-io @contract-shape:unbounded-preservation
/// Scenario: A skipped author's earlier claims stay searchable
///   Given Dmitri's 2 claims were indexed on an earlier pass
///   And pds.volkov.dev is down for this pass
///   When the next ingest pass runs
///   Then Dmitri is skipped as unreachable
///   And Maria still finds both of Dmitri's claims when she searches for his work
/// ```
#[test]
fn a_skipped_authors_earlier_claims_stay_searchable() {
    let world = given_three_authors_with_dmitri_indexed_on_an_earlier_pass();
    let before = world.rows_of(Author::Dmitri);
    world.host_is_reachable(Host::VolkovDev, false);

    let pass = world.one_ingest_pass_runs();

    pass.assert_pass_completed(0, 2, 0, 1);
    pass.assert_skipped(Author::Dmitri.did(), SkipReason::PdsUnreachable);
    assert_eq!(
        world.rows_of(Author::Dmitri),
        before,
        "Dmitri's rows are untouched"
    );
    let found = world.maria_searches(&["--subject", FERRITE_REPRODUCIBLE_BUILDS.subject]);
    assert_eq!(found.status, 0, "{}\n{}", found.stdout, found.stderr);
    let dmitri_rows: Vec<_> = search_rows(&found.stdout)
        .into_iter()
        .filter(|r| r.author_did == Author::Dmitri.app_identity())
        .collect();
    assert_eq!(dmitri_rows.len(), 2, "{}", found.stdout);
}

/// IPF-15
/// ```gherkin
/// @US-IPF-002 @AC-002.5 @NFR-2 @error @real-io @contract-shape:bounded-change
/// Scenario: A hanging PDS cannot stall the pass
///   Given the per-author time budget is 2 seconds
///   And pds.slowhost.example accepts connections and never answers
///   When one ingest pass runs
///   Then did:plc:samslowhost6w is skipped with reason pds_timeout
///   And every other author is indexed
///   And the pass finishes within a small multiple of the budget
/// ```
#[test]
#[ignore = "DELIVER 02-01: one deadline per DID (timeout_at) → pds_timeout"]
fn a_hanging_pds_cannot_stall_the_pass() {
    let mut world = IndexerWorld::configured_with(&[Author::Priya, Author::Sam]);
    world.publishes_self_attested(Author::Priya, &[CARGO_PIN_REPRODUCIBLE_BUILDS]);
    world.publishes_self_attested(Author::Sam, &[SAM_SLOW_CLAIM]);
    world.setting(var::PER_DID_TIMEOUT, "2");
    world.host_answers(Host::SlowhostExample, ListingPosture::Hang);

    let pass = world.one_ingest_pass_runs();

    pass.assert_pass_completed(0, 1, 0, 1);
    let skip = pass.assert_skipped(Author::Sam.did(), SkipReason::PdsTimeout);
    assert_eq!(
        skip["pds_url"],
        world.url_of(Host::SlowhostExample),
        "{skip}"
    );
    assert_eq!(world.rows_of(Author::Priya).len(), 1);
    assert!(
        pass.elapsed < Duration::from_secs(20),
        "the pass is bounded by the per-author budget, took {:?}",
        pass.elapsed
    );
}

/// IPF-16
/// ```gherkin
/// @US-IPF-002 @AC-002.5 @error @real-io @contract-shape:bounded-change
/// Scenario: A DID document that never arrives counts against the same budget
///   Given the per-author time budget is 2 seconds and no fallback is configured
///   And the directory never answers for Priya's DID document
///   When one ingest pass runs
///   Then Priya is skipped as unresolvable and Dmitri is indexed
///   And the pass finishes within a small multiple of the budget
/// ```
#[test]
#[ignore = "DELIVER 02-01: the deadline covers resolving (ResolutionFailure::TimedOut)"]
fn a_did_document_that_never_arrives_counts_against_the_same_budget() {
    let mut world = IndexerWorld::configured_with(&[Author::Priya, Author::Dmitri]);
    world.publishes_app_signed(Author::Dmitri, &[FERRITE_REPRODUCIBLE_BUILDS]);
    world.setting(var::PER_DID_TIMEOUT, "2");
    world.did_document_of(Author::Priya, DidDocPosture::Hang);

    let pass = world.one_ingest_pass_runs();

    pass.assert_pass_completed(0, 1, 0, 1);
    pass.assert_skipped(Author::Priya.did(), SkipReason::DidUnresolvable);
    assert_eq!(world.rows_of(Author::Dmitri).len(), 1);
    assert!(
        pass.elapsed < Duration::from_secs(20),
        "took {:?}",
        pass.elapsed
    );
}

/// IPF-17
/// ```gherkin
/// @US-IPF-002 @AC-002.6 @real-io @contract-shape:bounded-change
/// Scenario: Skipped authors are retried on the next pass
///   Given Dmitri was skipped on the previous pass because his PDS was down
///   And pds.volkov.dev is reachable again with a third claim
///   When the next ingest pass runs
///   Then all 3 of Dmitri's claims are indexed
///   And no skip is reported for him
/// ```
#[test]
fn skipped_authors_are_retried_on_the_next_pass() {
    let mut world = given_three_authors_publish_on_their_own_pdses();
    world.host_is_reachable(Host::VolkovDev, false);
    world
        .one_ingest_pass_runs()
        .assert_pass_completed(0, 2, 0, 1);
    world.host_is_reachable(Host::VolkovDev, true);
    world.publishes_app_signed(Author::Dmitri, &[FERRITE_TEST_DRIVEN]);

    let next = world.one_ingest_pass_runs();

    next.assert_pass_completed(0, 3, 0, 0);
    next.assert_read_from_own_pds(Author::Dmitri.did());
    assert_eq!(world.rows_of(Author::Dmitri).len(), 3);
}

// =============================================================================
// Bounds and the total-outage exit code
// =============================================================================

/// IPF-18
/// ```gherkin
/// @US-IPF-002 @AC-002.7 @NFR-1 @I-IPF-4 @real-io @contract-shape:bounded-change
/// Scenario: Many authors never cause unbounded concurrent requests
///   Given 40 authors spread across 12 PDS hosts are configured
///   And the operator allows at most 3 requests at a time
///   And every PDS answers slowly, and one host with 3 of the authors answers 502
///   When one ingest pass runs
///   Then at no moment were more than 3 requests in flight
///   And the pass summary accounts for all 40 authors: 37 from their own PDS, 3 skipped
/// ```
#[test]
#[ignore = "DELIVER 02-01: buffered(max_concurrent_fetches) fan-out (ADR-078)"]
fn many_authors_never_cause_unbounded_concurrent_requests() {
    let (mut world, by_host) = IndexerWorld::crowd(40, 12);
    world.setting(var::MAX_CONCURRENT, "3");
    for host in by_host.keys() {
        world
            .net
            .set_listing_posture(host, ListingPosture::Slow(Duration::from_millis(150)));
    }
    let (down_host, down_authors) = by_host
        .iter()
        .find(|(_, dids)| dids.len() == 3)
        .expect("a host serving 3 authors");
    world
        .net
        .set_listing_posture(down_host, ListingPosture::Status(502));

    let pass = world.one_ingest_pass_runs();

    pass.assert_pass_completed(0, 37, 0, 3);
    assert!(
        world.net.max_requests_in_flight() <= 3,
        "peak in-flight requests {} exceeded the cap 3",
        world.net.max_requests_in_flight()
    );
    for did in down_authors {
        pass.assert_skipped(did, SkipReason::PdsUnreachable);
    }
}

/// IPF-19
/// ```gherkin
/// @US-IPF-002 @AC-002.8 @DD-IPF-6 @error @real-io @contract-shape:bounded-change
/// Scenario: When every author's source fails, the pass reports a total outage
///   Given Priya's, Dmitri's and Jeff's PDSes are all down
///   When one ingest pass runs
///   Then Jeff sees a pass summary of 3 configured and 3 skipped, marked with exit code 3
///   And the summary is reported before the indexer exits with code 3
///   And nothing previously indexed is removed
/// ```
#[test]
#[ignore = "DELIVER 02-01: pass_exit_code == 3 iff every configured DID skipped"]
fn when_every_authors_source_fails_the_pass_reports_a_total_outage() {
    let world = given_three_authors_with_dmitri_indexed_on_an_earlier_pass();
    let before = world.indexed_rows();
    for host in [Host::MorelBsky, Host::VolkovDev, Host::JeffbaileyUs] {
        world.host_is_reachable(host, false);
    }

    let pass = world.one_ingest_pass_runs();

    pass.assert_pass_completed(3, 0, 0, 3);
    let last_event = pass.events().last().cloned().unwrap_or_default();
    assert_eq!(
        last_event["event"],
        "indexer.ingest.pass_summary",
        "pass_summary is the last word before the exit\n{}",
        pass.dump()
    );
    assert_eq!(
        world.indexed_rows(),
        before,
        "a total outage deletes nothing"
    );
}

/// IPF-20
/// ```gherkin
/// @US-IPF-002 @AC-002.9 @DD-IPF-5 @error @adversarial @real-io @contract-shape:unbounded-preservation
/// Scenario Outline: A DID document pointing at a private or insecure address is never followed
///   Given Dmitri's DID document names <address> as his PDS
///   And a fallback source is configured
///   When one ingest pass runs
///   Then Dmitri is skipped with reason pds_address_refused, naming <address>
///   And the fallback is not contacted for Dmitri
///   And Priya is indexed and the pass completes successfully
///   Examples:
///     | address                       |
///     | http://10.0.0.1               |
///     | https://169.254.169.254       |
///     | https://192.168.1.20          |
///     | https://[::ffff:10.0.0.1]     |
///     | https://[fe80::1]             |
///     | http://pds.volkov.example     |
/// ```
#[test]
#[ignore = "DELIVER 02-02: pds_endpoint_admissible pre-check → Skip(PdsAddressRefused)"]
fn a_did_document_pointing_at_a_private_or_insecure_address_is_never_followed() {
    for address in [
        "http://10.0.0.1",
        "https://169.254.169.254",
        "https://192.168.1.20",
        "https://[::ffff:10.0.0.1]",
        "https://[fe80::1]",
        "http://pds.volkov.example",
    ] {
        let mut world = IndexerWorld::configured_with(&[Author::Priya, Author::Dmitri]);
        world.publishes_self_attested(Author::Priya, &[CARGO_PIN_REPRODUCIBLE_BUILDS]);
        world.publishes_app_signed(Author::Dmitri, &[FERRITE_REPRODUCIBLE_BUILDS]);
        world.fallback_is(Host::JeffbaileyUs);
        world.did_document_of(
            Author::Dmitri,
            DidDocPosture::PdsEndpoint(address.to_string()),
        );

        let pass = world.one_ingest_pass_runs();

        pass.assert_pass_completed(0, 1, 0, 1);
        let skip = pass.assert_skipped(Author::Dmitri.did(), SkipReason::PdsAddressRefused);
        assert_eq!(skip["fallback_used"], false, "{address}: {skip}");
        // The refused address is named. IPv6 literals may be reported in their
        // canonical form (`[::ffff:a00:1]`), so those only need the scheme.
        let named = skip["pds_url"].as_str().unwrap_or_default();
        let scheme = address.split("://").next().unwrap_or_default();
        assert!(
            if address.contains('[') {
                named.starts_with(&format!("{scheme}://["))
            } else {
                named.starts_with(address)
            },
            "{address}: the refused address is named: {skip}"
        );
        assert!(
            world.listings_on(Host::JeffbaileyUs).is_empty(),
            "{address}: the fallback is never used for a DID that resolved"
        );
        assert_eq!(world.rows_of(Author::Priya).len(), 1, "{address}");
    }
}
