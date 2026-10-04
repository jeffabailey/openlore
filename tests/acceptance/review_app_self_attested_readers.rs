//! bluesky-claim-review-app — RELEASE 2: OpenLore readers recognise
//! self-attested claims (US-BRA-009, ADR-071, I-BRA-5, KPI-BRA-6).
//!
//! These scenarios exercise OpenLore's EXISTING read surfaces — `openlore peer
//! add/pull` + `graph query`, the `openlore ui` viewer, and the
//! `openlore-indexer` + `openlore search` — against a user's PDS (the
//! `FakeAtprotoNetwork` double) holding records in EXACTLY the shape the review
//! app writes (`self_attested_claim_value`). They do not need the review app
//! running, so DELIVER can turn this release on independently of the app.
//!
//! Provenance table under test (data-models.md §1 / ADR-071):
//! signature absent + bare author == repo DID + author-PDS origin → SelfAttested;
//! `#fragment` author without signature → MalformedProvenance; bare author ≠
//! repo DID → ForeignRepo; relay origin → UnverifiableProvenance; any admitted
//! record whose recomputed CID ≠ record key → integrity failure.
//!
//! Layer 4 (subprocess + real DuckDB): example-only. `#[ignore]`d for
//! one-at-a-time DELIVER (Release 2). AC-009.4's regression half is ALSO the
//! existing app-signed suites (`peer_pull`, `federated_query`,
//! `indexer_ingest`, `appview_search`, `viewer_*`) staying green.

// `support` and `review_app` both pull in tests/common/state_delta.rs.
#![allow(clippy::duplicate_mod)]

mod support;

#[path = "support/review_app/mod.rs"]
mod review_app;

use std::process::{Command, Stdio};

use openlore_test_support::FakeAtprotoNetwork;
use review_app::*;
use support::{run_openlore, run_openlore_with_peer_resolver, CliOutcome, FakeIdentity, TestEnv};

/// The ATProto network with Priya's PDS holding `records` (value, rkey override).
fn priya_pds_with(records: Vec<(serde_json::Value, Option<&str>)>) -> FakeAtprotoNetwork {
    let net = FakeAtprotoNetwork::start(bluesky_accounts());
    for (value, rkey) in records {
        seed_claim_record(&net, Persona::Priya.did(), value, rkey);
    }
    net
}

/// WHEN Maria adds Priya as a peer and pulls.
fn when_maria_adds_priya_and_pulls(env: &TestEnv, net: &FakeAtprotoNetwork) -> CliOutcome {
    let pds = net.pds_url_for(Persona::Priya.did());
    let added = run_openlore_with_peer_resolver(
        env,
        &["peer", "add", Persona::Priya.did()],
        Persona::Priya.did(),
        &pds,
    );
    assert_eq!(
        added.status, 0,
        "peer add of a Bluesky user:\n{}\n{}",
        added.stdout, added.stderr
    );
    run_openlore_with_peer_resolver(env, &["peer", "pull"], Persona::Priya.did(), &pds)
}

fn maria() -> TestEnv {
    TestEnv::initialized_as(FakeIdentity::maria())
}

fn graph_query(env: &TestEnv, subject: &str) -> CliOutcome {
    run_openlore(
        env,
        &["graph", "query", "--subject", subject, "--federated"],
    )
}

/// RD-1
/// ```gherkin
/// @US-BRA-009 @AC-009.1 @AC-009.3 @I-BRA-5 @kpi-bra-6 @real-io @adapter-integration @contract-shape:bounded-change
/// Scenario: Maria pulls Priya's self-attested claims
///   Given Priya published "priyaraman/tidepool embodies memory-safety" via the review app
///   When Maria adds Priya as a peer and pulls
///   Then the claim is in Maria's store, attributed to Priya, and shown as "self-attested", never "unverified"
/// ```
#[test]
#[ignore = "DELIVER R2: unskip one-at-a-time (US-BRA-009 peer pull accepts self-attested)"]
fn maria_pulls_priyas_self_attested_claims() {
    let s = priya::TIDEPOOL_MEMORY_SAFETY;
    let net = priya_pds_with(vec![(
        self_attested_claim_value(Persona::Priya.did(), s, 2500, &[]),
        None,
    )]);
    let env = maria();
    let pulled = when_maria_adds_priya_and_pulls(&env, &net);
    assert_eq!(pulled.status, 0, "{}\n{}", pulled.stdout, pulled.stderr);
    assert!(pulled.stdout.contains("self-attested"), "{}", pulled.stdout);
    let shown = graph_query(&env, &s.subject());
    assert!(
        shown.stdout.contains(Persona::Priya.did()) && shown.stdout.contains("self-attested"),
        "{}",
        shown.stdout
    );
    assert!(!shown.stdout.contains("unverified"));
}

/// RD-2
/// ```gherkin
/// @US-BRA-009 @AC-009.3 @I-BRA-5 @real-io @contract-shape:pure-function
/// Scenario: The viewer labels self-attested claims, never unverified
///   Given Maria pulled Priya's self-attested claim
///   When she opens her peer claims in the OpenLore viewer
///   Then Priya's claim is listed, attributed, with the "self-attested" label
/// ```
#[test]
#[ignore = "DELIVER R2: unskip one-at-a-time (US-BRA-009 viewer label)"]
fn the_viewer_labels_self_attested_claims_never_unverified() {
    let net = priya_pds_with(vec![(
        self_attested_claim_value(
            Persona::Priya.did(),
            priya::TIDEPOOL_MEMORY_SAFETY,
            2500,
            &[],
        ),
        None,
    )]);
    let env = maria();
    assert_eq!(when_maria_adds_priya_and_pulls(&env, &net).status, 0);
    let viewer = support::ViewerServer::start(&env);
    let page = viewer.get("/peer-claims");
    assert_eq!(page.status, 200);
    let text = html::visible_text(&page.body);
    assert!(
        text.contains(Persona::Priya.did()) && text.contains("self-attested"),
        "{text}"
    );
    assert!(!text.contains("unverified"));
}

/// RD-3
/// ```gherkin
/// @US-BRA-009 @AC-009.4 @guardrail @regression @contract-shape:unbounded-preservation
/// Scenario: App-signed claims read exactly as before, side by side with self-attested ones
///   Given Rachel's app-signed claims and Priya's self-attested claim about dependency-pinning
///   When Maria pulls both peers
///   Then Rachel's claims show exactly what they showed before self-attested claims existed
///   And Priya's claim is shown as self-attested, each attributed to its own author
/// ```
#[test]
#[ignore = "DELIVER R2: unskip one-at-a-time (US-BRA-009 app-signed unchanged)"]
fn app_signed_claims_read_exactly_as_before_side_by_side_with_self_attested_ones() {
    let rachel = "did:plc:rachel-test";
    let (rachel_records, rachel_key) = support::build_verifiable_peer_records(rachel, [7u8; 32]);

    // Baseline: Rachel alone.
    let baseline_env = maria();
    let rachel_pds = support::PeerPds::for_peer(rachel, rachel_records.clone());
    run_openlore_with_peer_resolver(
        &baseline_env,
        &["peer", "add", rachel],
        rachel,
        rachel_pds.endpoint_url(),
    );
    support::run_openlore_pull(
        &baseline_env,
        &["peer", "pull"],
        rachel,
        rachel_pds.endpoint_url(),
        &rachel_key,
    );
    let baseline = graph_query(&baseline_env, "github:rust-lang/cargo").stdout;

    // Mixed: Rachel + Priya.
    let env = maria();
    let rachel_pds = support::PeerPds::for_peer(rachel, rachel_records);
    let net = priya_pds_with(vec![(
        self_attested_claim_value(
            Persona::Priya.did(),
            priya::TIDEPOOL_DEPENDENCY_PINNING,
            2500,
            &[],
        ),
        None,
    )]);
    let priya_pds = net.pds_url_for(Persona::Priya.did());
    run_openlore_with_peer_resolver(
        &env,
        &["peer", "add", rachel],
        rachel,
        rachel_pds.endpoint_url(),
    );
    run_openlore_with_peer_resolver(
        &env,
        &["peer", "add", Persona::Priya.did()],
        Persona::Priya.did(),
        &priya_pds,
    );
    let pulled = support::run_openlore_pull_multi(
        &env,
        &["peer", "pull"],
        &[
            support::PeerSeam {
                peer_did: rachel,
                peer_endpoint: rachel_pds.endpoint_url(),
                peer_pubkey_hex: &rachel_key,
            },
            support::PeerSeam {
                peer_did: Persona::Priya.did(),
                peer_endpoint: &priya_pds,
                peer_pubkey_hex: "",
            },
        ],
    );
    assert_eq!(pulled.status, 0, "{}\n{}", pulled.stdout, pulled.stderr);
    assert_eq!(
        graph_query(&env, "github:rust-lang/cargo").stdout,
        baseline,
        "app-signed output unchanged"
    );
    let priya_view = graph_query(&env, &priya::TIDEPOOL_DEPENDENCY_PINNING.subject()).stdout;
    assert!(
        priya_view.contains("self-attested") && priya_view.contains(Persona::Priya.did()),
        "{priya_view}"
    );
}

/// RD-4 — parametrised over the records every reader must refuse (ADR-071).
/// ```gherkin
/// @US-BRA-009 @AC-009.5 @ADR-071 @error @adversarial @C6b @contract-shape:unbounded-preservation
/// Scenario Outline: Records that are not honest self-attestations are refused
///   Given Priya's repo holds a record that <defect>
///   When Maria pulls Priya
///   Then the record is refused with "<reason>" and nothing of it is stored
///   Examples:
///     | defect                                                   | reason                  |
///     | has content that does not match its record key           | integrity check failed  |
///     | claims to be authored by Sam (another DID)               | foreign repo            |
///     | names an application key but carries no signature        | malformed provenance    |
/// ```
#[test]
#[ignore = "DELIVER R2: unskip one-at-a-time (US-BRA-009 tampered / foreign / malformed refused)"]
fn records_that_are_not_honest_self_attestations_are_refused() {
    let s = priya::TIDEPOOL_MEMORY_SAFETY;
    let honest = self_attested_claim_value(Persona::Priya.did(), s, 2500, &[]);
    let tampered_key = recomputed_cid(&self_attested_claim_value(
        Persona::Priya.did(),
        s,
        9900,
        &[],
    ));
    let cases: Vec<(serde_json::Value, Option<String>, &str)> = vec![
        (honest, Some(tampered_key), "integrity check failed"),
        (
            self_attested_claim_value(Persona::Sam.did(), s, 2500, &[]),
            None,
            "foreign repo",
        ),
        (
            self_attested_claim_value(
                &format!("{}#org.openlore.application", Persona::Priya.did()),
                s,
                2500,
                &[],
            ),
            None,
            "malformed provenance",
        ),
    ];
    for (value, rkey, reason) in cases {
        let net = FakeAtprotoNetwork::start(bluesky_accounts());
        seed_claim_record(&net, Persona::Priya.did(), value, rkey.as_deref());
        let env = maria();
        let pulled = when_maria_adds_priya_and_pulls(&env, &net);
        let output = format!("{}{}", pulled.stdout, pulled.stderr);
        assert!(output.contains(reason), "expected {reason:?}:\n{output}");
        let shown = graph_query(&env, &s.subject()).stdout;
        assert!(
            !shown.contains(Persona::Priya.did()),
            "nothing of a refused record is stored:\n{shown}"
        );
    }
}

/// RD-5
/// ```gherkin
/// @US-BRA-009 @AC-011.2 @RC-02 @contract-shape:pure-function
/// Scenario: A claim Priya retracted through the app reads as retracted
///   Given Priya published test-driven and later retracted it through the app
///   When Maria pulls Priya
///   Then both records are stored and the original is marked retracted, never deleted
/// ```
#[test]
#[ignore = "DELIVER R2: unskip one-at-a-time (self-attested retraction read as retracted)"]
fn a_claim_priya_retracted_through_the_app_reads_as_retracted() {
    let s = priya::TIDEPOOL_TEST_DRIVEN;
    let original = self_attested_claim_value(Persona::Priya.did(), s, 4000, &[]);
    let original_cid = recomputed_cid(&original);
    let retraction = self_attested_claim_value(
        Persona::Priya.did(),
        s,
        4000,
        &[("retracts", original_cid.as_str())],
    );
    let net = priya_pds_with(vec![(original, None), (retraction, None)]);
    let env = maria();
    assert_eq!(when_maria_adds_priya_and_pulls(&env, &net).status, 0);
    let shown = graph_query(&env, &s.subject()).stdout;
    assert!(shown.to_lowercase().contains("retracted"), "{shown}");
}

// -----------------------------------------------------------------------------
// The network indexer + `openlore search` (AC-009.2)
// -----------------------------------------------------------------------------

/// Run `openlore-indexer ingest` over Priya's PDS (one repo DID enumerated,
/// origin computed from the PLC-resolved DID document — ADR-071 §4).
fn ingest_priya_repo(env: &TestEnv, net: &FakeAtprotoNetwork, source_url: &str) -> CliOutcome {
    #[allow(deprecated)]
    let bin = assert_cmd::cargo::cargo_bin("openlore-indexer");
    let out = Command::new(bin)
        .arg("ingest")
        .env_clear()
        .env("OPENLORE_HOME", &env.home)
        .env(
            "OPENLORE_INDEXER_INDEX_PATH",
            support::index_duckdb_path(env),
        )
        .env("OPENLORE_INDEXER_SOURCE_URL", source_url)
        .env("OPENLORE_INDEXER_PLC_ENDPOINT", net.directory_url())
        // DISTILL-proposed seam (DWD-9): which repo DIDs a single-PDS source enumerates.
        .env("OPENLORE_INDEXER_REPO_DIDS", Persona::Priya.did())
        .env("PATH", std::env::var("PATH").unwrap_or_default())
        .stdin(Stdio::null())
        .output()
        .expect("spawn openlore-indexer ingest");
    CliOutcome {
        status: out.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
    }
}

fn indexed_rows(env: &TestEnv) -> Vec<(String, String)> {
    let conn =
        duckdb::Connection::open(support::index_duckdb_path(env)).expect("open index.duckdb");
    let mut stmt = conn
        .prepare("SELECT author_did, provenance FROM indexed_claims ORDER BY author_did")
        .expect("indexed_claims carries provenance (data-models §4)");
    let rows = stmt
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
        .expect("query")
        .filter_map(Result::ok)
        .collect();
    rows
}

/// RD-6
/// ```gherkin
/// @US-BRA-009 @AC-009.2 @I-BRA-5 @kpi-bra-6 @real-io @adapter-integration @contract-shape:bounded-change
/// Scenario: The network index includes self-attested claims, attributed and marked
///   Given Priya's PDS holds her self-attested memory-safety claim
///   When the OpenLore indexer ingests Priya's repo from her own PDS
///   Then the index holds her claim, attributed to did:plc:7x3kq2mzv5rj4w6hbn2tqclp, with provenance self-attested
/// ```
#[test]
#[ignore = "DELIVER R2: unskip one-at-a-time (US-BRA-009 indexer includes self-attested)"]
fn the_network_index_includes_self_attested_claims_attributed_and_marked() {
    let net = priya_pds_with(vec![(
        self_attested_claim_value(
            Persona::Priya.did(),
            priya::TIDEPOOL_MEMORY_SAFETY,
            2500,
            &[],
        ),
        None,
    )]);
    let env = maria();
    let ingest = ingest_priya_repo(&env, &net, &net.pds_url_for(Persona::Priya.did()));
    assert_eq!(ingest.status, 0, "{}\n{}", ingest.stdout, ingest.stderr);
    assert_eq!(
        indexed_rows(&env),
        vec![(
            Persona::Priya.did().to_string(),
            "self-attested".to_string()
        )]
    );
}

/// RD-7
/// ```gherkin
/// @US-BRA-009 @ADR-071 @error @adversarial @contract-shape:unbounded-preservation
/// Scenario: A self-attested claim fetched through anything but the author's own PDS is not indexed
///   Given Priya's self-attested claim is served by a host that is not her DID document's PDS (a relay)
///   When the indexer ingests from that host
///   Then the claim is refused as unverifiable provenance and nothing is indexed
/// ```
#[test]
#[ignore = "DELIVER R2: unskip one-at-a-time (relay origin fails closed)"]
fn a_self_attested_claim_fetched_through_anything_but_the_authors_own_pds_is_not_indexed() {
    let net = priya_pds_with(vec![(
        self_attested_claim_value(
            Persona::Priya.did(),
            priya::TIDEPOOL_MEMORY_SAFETY,
            2500,
            &[],
        ),
        None,
    )]);
    // Dmitri's host also answers listRecords for any repo it is asked about — but it is
    // not Priya's DID-document PDS, so it stands in for a relay.
    let relay = net.host_url(PdsHost::VOLKOV);
    let env = maria();
    let ingest = ingest_priya_repo(&env, &net, &relay);
    let output = format!("{}{}", ingest.stdout, ingest.stderr);
    assert!(
        indexed_rows(&env).is_empty(),
        "nothing indexed through a relay:\n{output}"
    );
    assert!(
        output.to_lowercase().contains("unverifiable provenance"),
        "the refusal is reported (non-vacuity: the record WAS fetched and judged):\n{output}"
    );
}
