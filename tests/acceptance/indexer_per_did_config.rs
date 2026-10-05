//! indexer-per-did-pds-fetch — the indexer's configuration is explained at
//! startup and single-source deployments behave as before (US-IPF-004,
//! @infrastructure), plus the loopback test seam's release refusal (AC-004.5).
//!
//! Layer 3, example-only. Refusals happen before ANY wiring: the observable
//! universe of a refusal is {exit code, `health.startup.refused`, the operator
//! message, network requests (must be none), the index file (must not exist)}.
//! The generative half (parse_config is total; release + seam ⇒ refusal) is
//! `indexer_pass_core.rs` CORE-7/CORE-8.
//!
//! All scenarios are `#[ignore]`d at DISTILL hand-off.
//
// SCAFFOLD: true

mod support;

#[path = "support/indexer_network.rs"]
mod indexer_network;

use std::process::Command;
use std::time::Duration;

use indexer_network::claims::*;
use indexer_network::*;

/// THEN the indexer refused to start, naming `variable` and `value`, before
/// touching the network or creating its index.
fn then_start_is_refused_naming(
    world: &IndexerWorld,
    report: &PassReport,
    variable: &str,
    value: &str,
) {
    assert_eq!(
        report.status,
        2,
        "a configuration refusal exits 2\n{}",
        report.dump()
    );
    let refusal = report.startup_refusal().unwrap_or_else(|| {
        panic!(
            "MISSING_FUNCTIONALITY: no health.startup.refused\n{}",
            report.dump()
        )
    });
    assert_eq!(refusal["adapter"], "config", "{refusal}");
    assert_eq!(refusal["reason"], "IndexerConfigInvalid", "{refusal}");
    assert_eq!(refusal["structured"]["variable"], variable, "{refusal}");
    assert_eq!(refusal["structured"]["value"], value, "{refusal}");
    assert!(
        report.stderr.contains(variable) && report.stderr.contains(value),
        "the operator message names the variable and the offending value\n{}",
        report.dump()
    );
    assert!(
        world.net.requests().is_empty(),
        "nothing is contacted before config is valid"
    );
    assert!(
        !world.index_exists(),
        "no index is created by a refused start"
    );
}

/// IPF-21
/// ```gherkin
/// @US-IPF-004 @AC-004.1 @DD-IPF-4 @infrastructure @error @real-io @contract-shape:unbounded-preservation
/// Scenario Outline: A malformed repo DID is explained at startup
///   Given OPENLORE_INDEXER_REPO_DIDS is "did:plc:priyaraman7x2k,<entry>"
///   When Jeff starts the indexer
///   Then it refuses to start, naming OPENLORE_INDEXER_REPO_DIDS and "<entry>" (not the whole list)
///   Examples:
///     | entry                                   | why                       |
///     | priya                                   | not a DID                 |
///     | did:key:z6MkhaXgBZDvotDkL5257faiztiGiC2Q| method cannot name a repo |
///     | did:plc:                                | empty identifier          |
///     | did:plc:priya#org.openlore.application  | a key reference, not a DID|
/// ```
#[test]
fn a_malformed_repo_did_is_explained_at_startup() {
    for entry in [
        "priya",
        "did:key:z6MkhaXgBZDvotDkL5257faiztiGiC2QtKLGpbnnEGta2doK",
        "did:plc:",
        "did:plc:priya#org.openlore.application",
    ] {
        let mut world = IndexerWorld::configured_with(&[Author::Priya]);
        world.repo_dids_text_is(&format!("{},{entry}", Author::Priya.did()));

        let report = world.jeff_starts_the_indexer();

        then_start_is_refused_naming(&world, &report, var::REPO_DIDS, entry);
    }
}

/// IPF-22
/// ```gherkin
/// @US-IPF-004 @AC-004.2 @AC-003.4 @DD-IPF-5 @infrastructure @error @adversarial @real-io
/// @contract-shape:unbounded-preservation
/// Scenario Outline: A malformed or unsafe fallback source is explained at startup
///   Given OPENLORE_INDEXER_SOURCE_URL is "<value>"
///   When Jeff starts the indexer
///   Then it refuses to start, naming OPENLORE_INDEXER_SOURCE_URL and "<value>"
///   Examples:
///     | value                               | why                        |
///     | pds.jeffbailey.us                   | no scheme                  |
///     | ftp://pds.jeffbailey.us             | not https                  |
///     | http://pds.jeffbailey.us            | plain http to a public host|
///     | https://jeff:secret@pds.jeffbailey.us | credentials in the URL   |
///     | https://pds.jeffbailey.us/?page=2   | query string               |
///     | https://10.0.0.1                    | private address            |
///     | https://[::1]:8443                  | loopback address           |
/// ```
#[test]
fn a_malformed_or_unsafe_fallback_source_is_explained_at_startup() {
    for value in [
        "pds.jeffbailey.us",
        "ftp://pds.jeffbailey.us",
        "http://pds.jeffbailey.us",
        "https://jeff:secret@pds.jeffbailey.us",
        "https://pds.jeffbailey.us/?page=2",
        "https://10.0.0.1",
        "https://[::1]:8443",
    ] {
        let mut world = IndexerWorld::configured_with(&[Author::Priya]);
        world.fallback_text_is(value);

        let report = world.jeff_starts_the_indexer();

        then_start_is_refused_naming(&world, &report, var::FALLBACK, value);
    }
}

/// IPF-23
/// ```gherkin
/// @US-IPF-004 @AC-004.2 @DD-IPF-7 @infrastructure @error @boundary @real-io @contract-shape:unbounded-preservation
/// Scenario Outline: Fan-out bounds outside their range are explained at startup
///   Given <variable> is "<value>"
///   When Jeff starts the indexer
///   Then it refuses to start, naming <variable> and "<value>"
///   Examples:
///     | variable                                | value |
///     | OPENLORE_INDEXER_MAX_CONCURRENT_FETCHES | 0     |
///     | OPENLORE_INDEXER_MAX_CONCURRENT_FETCHES | 17    |
///     | OPENLORE_INDEXER_MAX_CONCURRENT_FETCHES | four  |
///     | OPENLORE_INDEXER_PER_DID_TIMEOUT_SECS   | 0     |
///     | OPENLORE_INDEXER_PER_DID_TIMEOUT_SECS   | 601   |
///     | OPENLORE_INDEXER_PER_DID_TIMEOUT_SECS   | -5    |
/// ```
#[test]
fn fan_out_bounds_outside_their_range_are_explained_at_startup() {
    for (variable, value) in [
        (var::MAX_CONCURRENT, "0"),
        (var::MAX_CONCURRENT, "17"),
        (var::MAX_CONCURRENT, "four"),
        (var::PER_DID_TIMEOUT, "0"),
        (var::PER_DID_TIMEOUT, "601"),
        (var::PER_DID_TIMEOUT, "-5"),
    ] {
        let mut world = IndexerWorld::configured_with(&[Author::Priya]);
        world.setting(variable, value);

        let report = world.jeff_starts_the_indexer();

        then_start_is_refused_naming(&world, &report, variable, value);
    }
}

/// IPF-24
/// ```gherkin
/// @US-IPF-004 @AC-004.3 @DD-IPF-7 @infrastructure @boundary @real-io @contract-shape:bounded-change
/// Scenario Outline: Fan-out bounds at the edge of their range are accepted and reported
///   Given OPENLORE_INDEXER_MAX_CONCURRENT_FETCHES is <fetches> and OPENLORE_INDEXER_PER_DID_TIMEOUT_SECS is <secs>
///   When one ingest pass runs
///   Then the loaded configuration reports <fetches> concurrent fetches and a <secs>-second budget
///   And Priya is indexed
///   Examples:
///     | fetches | secs  |
///     | 1       | 1     |
///     | 16      | 600   |
///     | (unset) | (unset) → defaults 4 and 30 |
/// ```
#[test]
fn fan_out_bounds_at_the_edge_of_their_range_are_accepted_and_reported() {
    for (fetches, secs, want_fetches, want_secs) in [
        (Some("1"), Some("1"), 1, 1),
        (Some("16"), Some("600"), 16, 600),
        (None, None, 4, 30),
    ] {
        let mut world = IndexerWorld::configured_with(&[Author::Priya]);
        world.publishes_self_attested(Author::Priya, &[CARGO_PIN_REPRODUCIBLE_BUILDS]);
        if let Some(f) = fetches {
            world.setting(var::MAX_CONCURRENT, f);
        }
        if let Some(s) = secs {
            world.setting(var::PER_DID_TIMEOUT, s);
        }

        let pass = world.one_ingest_pass_runs();

        pass.assert_pass_completed(0, 1, 0, 0);
        let loaded = pass
            .config_loaded()
            .unwrap_or_else(|| panic!("indexer.config.loaded is reported\n{}", pass.dump()));
        assert_eq!(loaded["max_concurrent_fetches"], want_fetches, "{loaded}");
        assert_eq!(loaded["per_did_time_budget_secs"], want_secs, "{loaded}");
        assert_eq!(world.rows_of(Author::Priya).len(), 1);
    }
}

/// IPF-25
/// ```gherkin
/// @US-IPF-004 @AC-004.3 @AC-002.8 @infrastructure @boundary @real-io @contract-shape:bounded-change
/// Scenario: An empty DID list and no fallback are reported, not refused
///   Given no repo DIDs and no fallback source are configured
///   When one ingest pass runs
///   Then the loaded configuration reports 0 repo DIDs and no fallback
///   And the pass summary shows nothing configured and the pass exits 0
/// ```
#[test]
fn an_empty_did_list_and_no_fallback_are_reported_not_refused() {
    let mut world = IndexerWorld::configured_with(&[]);
    world.repo_dids_text_is("");

    let pass = world.one_ingest_pass_runs();

    pass.assert_pass_completed(0, 0, 0, 0);
    let loaded = pass
        .config_loaded()
        .unwrap_or_else(|| panic!("indexer.config.loaded is reported\n{}", pass.dump()));
    assert_eq!(loaded["repo_did_count"], 0, "{loaded}");
    assert_eq!(loaded["fallback_configured"], false, "{loaded}");
    assert_eq!(
        loaded["transport_policy"], "https_or_loopback_http",
        "{loaded}"
    );
}

/// IPF-26
/// ```gherkin
/// @US-IPF-004 @AC-002.7 @infrastructure @boundary @real-io @contract-shape:bounded-change
/// Scenario: A repo DID listed twice is read once and counted once
///   Given OPENLORE_INDEXER_REPO_DIDS lists Priya twice and Dmitri once
///   When one ingest pass runs
///   Then the pass summary counts 2 configured authors, both read from their own PDS
///   And Priya's PDS was asked for her repo exactly once
/// ```
#[test]
fn a_repo_did_listed_twice_is_read_once_and_counted_once() {
    let mut world = IndexerWorld::configured_with(&[Author::Priya, Author::Dmitri]);
    world.publishes_self_attested(Author::Priya, &[CARGO_PIN_REPRODUCIBLE_BUILDS]);
    world.repo_dids_text_is(&format!(
        "{} {},{}",
        Author::Priya.did(),
        Author::Priya.did(),
        Author::Dmitri.did()
    ));

    let pass = world.one_ingest_pass_runs();

    pass.assert_pass_completed(0, 2, 0, 0);
    assert_eq!(
        world.listings_on(Host::MorelBsky),
        vec![Author::Priya.did()]
    );
    assert_eq!(
        pass.config_loaded().map(|c| c["repo_did_count"].clone()),
        Some(2.into())
    );
}

/// IPF-27
/// ```gherkin
/// @US-IPF-004 @AC-004.4 @NFR-3 @infrastructure @regression @guardrail @real-io @contract-shape:unbounded-preservation
/// Scenario: A single-source deployment indexes exactly what it did before
///   Given every configured author lives on pds.jeffbailey.us, which is also the configured fallback
///   And Jeff's PDS holds Jeff's app-signed claim and Priya's self-attested claim
///   When one ingest pass runs
///   Then both claims are indexed exactly as the single-source indexer indexed them
///   And the fallback is never needed, because every author resolves to that same PDS
/// ```
#[test]
fn a_single_source_deployment_indexes_exactly_what_it_did_before() {
    let mut world = IndexerWorld::configured_with(&[Author::Priya, Author::Jeff]);
    world.author_moves_to(Author::Priya, Host::JeffbaileyUs);
    let priya = world.publishes_self_attested(Author::Priya, &[CARGO_PIN_REPRODUCIBLE_BUILDS]);
    let jeff = world.publishes_app_signed(Author::Jeff, &[OPENLORE_LOCAL_FIRST]);
    world.fallback_is(Host::JeffbaileyUs);

    let pass = world.one_ingest_pass_runs();

    pass.assert_pass_completed(0, 2, 0, 0);
    let mut got: Vec<(String, String, String)> = world
        .indexed_rows()
        .into_iter()
        .map(|r| (r.author_did, r.cid, r.provenance))
        .collect();
    got.sort();
    let mut want = vec![
        (
            Author::Priya.did().to_string(),
            priya[0].clone(),
            "self-attested".to_string(),
        ),
        (
            Author::Jeff.app_identity(),
            jeff[0].clone(),
            "app-signed".to_string(),
        ),
    ];
    want.sort();
    assert_eq!(got, want);
    let mut listed = world.listings_on(Host::JeffbaileyUs);
    listed.sort();
    assert_eq!(
        listed,
        vec![Author::Jeff.did(), Author::Priya.did()],
        "each listed once"
    );
}

/// IPF-28
/// ```gherkin
/// @US-IPF-004 @AC-004.4 @NFR-3 @infrastructure @regression @error @real-io @contract-shape:unbounded-preservation
/// Scenario: With the directory down, a single-source deployment keeps its old behaviour
///   Given every configured author lives on pds.jeffbailey.us, which is also the configured fallback
///   And the DID directory is down for this pass
///   When one ingest pass runs
///   Then Jeff's app-signed claim is indexed through the fallback
///   And Priya's self-attested claim is refused for provenance, exactly as before
/// ```
#[test]
fn with_the_directory_down_a_single_source_deployment_keeps_its_old_behaviour() {
    let mut world = IndexerWorld::configured_with(&[Author::Priya, Author::Jeff]);
    world.author_moves_to(Author::Priya, Host::JeffbaileyUs);
    world.publishes_self_attested(Author::Priya, &[CARGO_PIN_REPRODUCIBLE_BUILDS]);
    let jeff = world.publishes_app_signed(Author::Jeff, &[OPENLORE_LOCAL_FIRST]);
    world.fallback_is(Host::JeffbaileyUs);
    world.net.set_directory_reachable(false);

    let pass = world.one_ingest_pass_runs();

    pass.assert_pass_completed(0, 0, 2, 0);
    assert_eq!(
        pass.refused_for(RefusalReason::Provenance),
        1,
        "{}",
        pass.dump()
    );
    let rows = world.indexed_rows();
    assert_eq!(rows.len(), 1, "{rows:?}");
    assert_eq!(rows[0].cid, jeff[0]);
}

/// IPF-29
/// ```gherkin
/// @US-IPF-004 @AC-004.5 @DD-IPF-5 @infrastructure @error @real-io @contract-shape:unbounded-preservation
/// Scenario: Without the test seam, the production transport policy refuses a loopback fallback
///   Given the loopback test seam is not set
///   And the fallback source is a plain-http loopback address
///   When Jeff starts the indexer
///   Then it refuses to start, naming OPENLORE_INDEXER_SOURCE_URL and that address
/// ```
#[test]
fn without_the_test_seam_the_production_policy_refuses_a_loopback_fallback() {
    let mut world = IndexerWorld::configured_with(&[Author::Priya]);
    world.without_loopback_seam();
    let loopback = world.url_of(Host::JeffbaileyUs);
    world.fallback_text_is(&loopback);

    let report = world.jeff_starts_the_indexer();

    then_start_is_refused_naming(&world, &report, var::FALLBACK, &loopback);
}

/// IPF-30
/// ```gherkin
/// @US-IPF-004 @AC-004.5 @DD-IPF-5 @R-IPF-9 @infrastructure @error @release-gate @real-io
/// @contract-shape:unbounded-preservation
/// Scenario: A release build refuses the loopback test seam
///   Given the release build of the indexer
///   And OPENLORE_INDEXER_ALLOW_LOOPBACK_HTTP is set to 1
///   When Jeff starts the indexer
///   Then it refuses to start, naming OPENLORE_INDEXER_ALLOW_LOOPBACK_HTTP
///   And nothing is contacted
/// ```
/// CI-only: needs `cargo build --release -p openlore-indexer` first (the binary
/// is taken from `OPENLORE_INDEXER_RELEASE_BIN`, else `target/release/`). Run
/// with `-- --ignored` in the CI release-guard step (DEVOPS).
#[test]
#[ignore = "CI release-guard (DELIVER 02-03, ci.yml release-guard step): run with --ignored after a release build"]
fn a_release_build_refuses_the_loopback_test_seam() {
    let release_bin = std::env::var_os("OPENLORE_INDEXER_RELEASE_BIN")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../target/release")
                .join(format!("openlore-indexer{}", std::env::consts::EXE_SUFFIX))
        });
    assert!(
        release_bin.exists(),
        "PRECONDITION (CI release-guard): build it first — cargo build --release -p openlore-indexer \
         (looked at {release_bin:?})"
    );
    let world = IndexerWorld::configured_with(&[Author::Priya]);
    let home = world.env.home.display().to_string();
    let mut cmd = Command::new(&release_bin);
    cmd.arg("ingest")
        .env_clear()
        .env("OPENLORE_HOME", &home)
        .env(
            "OPENLORE_INDEXER_INDEX_PATH",
            format!("{home}/index.duckdb"),
        )
        .env(var::REPO_DIDS, Author::Priya.did())
        .env(var::PLC, world.net.directory_url())
        .env(var::LOOPBACK_SEAM, "1")
        .env("PATH", std::env::var("PATH").unwrap_or_default());

    let report = run_bounded(cmd, Duration::from_secs(60));

    assert_eq!(report.status, 2, "{}", report.dump());
    let refusal = report.startup_refusal().unwrap_or_else(|| {
        panic!(
            "MISSING_FUNCTIONALITY: no health.startup.refused\n{}",
            report.dump()
        )
    });
    assert_eq!(
        refusal["structured"]["variable"],
        var::LOOPBACK_SEAM,
        "{refusal}"
    );
    assert!(
        report.stderr.contains(var::LOOPBACK_SEAM),
        "{}",
        report.dump()
    );
    assert!(
        world.net.requests().is_empty(),
        "nothing is contacted by a refused start"
    );
}
