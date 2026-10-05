//! indexer-per-did-pds-fetch — every author's claims are read from the author's
//! OWN PDS (US-IPF-001), and an unresolvable author falls back without
//! weakening provenance (US-IPF-003). ADR-077, ADR-071 §4 (amended).
//!
//! Layer 3 (subprocess + real `index.duckdb`), example-only (Mandate 9/11).
//! Driving ports: the REAL `openlore-indexer ingest` / `serve` binaries and the
//! REAL `openlore search` CLI. Driven-external systems — the PLC directory and
//! every PDS host (one loopback server per host, plus the fallback) — are ONE
//! `FakeAtprotoNetwork` (`crates/test-support/src/fake_atproto.rs`).
//!
//! Story line (Pillar 2): WS-1 → WS-2 → IPF-01 (move) chain on the same world
//! builder; US-IPF-003 scenarios chain on `given_ghost_is_unresolvable_…`.
//!
//! All scenarios are `#[ignore]`d at DISTILL hand-off; DELIVER removes one
//! ignore at a time (see `docs/feature/indexer-per-did-pds-fetch/distill/`).
//
// SCAFFOLD: true

mod support;

#[path = "support/indexer_network.rs"]
mod indexer_network;

use indexer_network::claims::*;
use indexer_network::*;

// =============================================================================
// Shared Given steps (story line US-IPF-001)
// =============================================================================

/// GIVEN Priya (bsky.social) has 3 self-attested claims on her own PDS, Dmitri
/// (pds.volkov.dev) has 2 app-signed claims on his, and Jeff has 1 app-signed
/// claim on his; the indexer is configured with all three repo DIDs and NO
/// fallback source.
fn given_three_authors_publish_on_their_own_pdses() -> (IndexerWorld, Published) {
    let mut world = IndexerWorld::configured_with(&[Author::Priya, Author::Dmitri, Author::Jeff]);
    let priya = world.publishes_self_attested(
        Author::Priya,
        &[
            CARGO_PIN_REPRODUCIBLE_BUILDS,
            CARGO_PIN_DEPENDENCY_PINNING,
            TIDEPOOL_MEMORY_SAFETY,
        ],
    );
    let dmitri = world.publishes_app_signed(
        Author::Dmitri,
        &[FERRITE_REPRODUCIBLE_BUILDS, FERRITE_DEPENDENCY_PINNING],
    );
    let jeff = world.publishes_app_signed(Author::Jeff, &[OPENLORE_LOCAL_FIRST]);
    (
        world,
        Published {
            priya,
            dmitri,
            jeff,
        },
    )
}

/// The record keys (CIDs) each author published.
struct Published {
    priya: Vec<String>,
    dmitri: Vec<String>,
    jeff: Vec<String>,
}

/// THEN every author's claims are indexed, attributed to that author, with the
/// expected provenance — and nothing else is in the index.
fn then_index_holds_exactly(world: &IndexerWorld, expected: &[(&str, &[String], Provenance)]) {
    let mut want: Vec<(String, String, String)> = expected
        .iter()
        .flat_map(|(author, cids, provenance)| {
            cids.iter().map(move |cid| {
                (
                    author.to_string(),
                    cid.clone(),
                    provenance.token().to_string(),
                )
            })
        })
        .collect();
    want.sort();
    let mut got: Vec<(String, String, String)> = world
        .indexed_rows()
        .into_iter()
        .map(|r| (r.author_did, r.cid, r.provenance))
        .collect();
    got.sort();
    assert_eq!(
        got, want,
        "the index holds exactly the expected attributed rows"
    );
}

// =============================================================================
// Walking skeletons
// =============================================================================

/// WS-1
/// ```gherkin
/// @walking_skeleton @driving_port @driving_adapter @real-io @US-IPF-001
/// @AC-001.1 @AC-001.2 @AC-001.3 @J-005 @contract-shape:bounded-change
/// Scenario: Maria finds Priya's self-attested claim published on her bsky.social PDS
///   Given Priya's DID document names her bsky.social PDS and she published there
///     that github:priyaraman/cargo-pin embodies reproducible-builds
///   And Dmitri's DID document names pds.volkov.dev, where he published app-signed claims
///   When one ingest pass runs
///   And Maria searches the network for reproducible-builds
///   Then Maria sees Priya's claim on cargo-pin attributed to did:plc:priyaraman7x2k
///   And she sees Dmitri's claim on ferrite next to it, attributed to Dmitri
/// ```
#[test]
#[ignore = "DELIVER 01-01: per-DID PDS resolution (ADR-077) — walking skeleton"]
fn maria_finds_priyas_self_attested_claim_published_on_her_own_pds() {
    let (world, _published) = given_three_authors_publish_on_their_own_pdses();

    let pass = world.one_ingest_pass_runs();
    assert_eq!(
        pass.status,
        0,
        "MISSING_FUNCTIONALITY: the pass must read each author from their own PDS\n{}",
        pass.dump()
    );
    let found = world.maria_searches(&["--object", &CARGO_PIN_REPRODUCIBLE_BUILDS.object()]);

    assert_eq!(found.status, 0, "{}\n{}", found.stdout, found.stderr);
    let rows = search_rows(&found.stdout);
    assert!(
        rows.iter().any(|r| r.author_did == Author::Priya.did()
            && r.subject == CARGO_PIN_REPRODUCIBLE_BUILDS.subject),
        "Maria sees Priya's cargo-pin claim attributed to her DID:\n{}",
        found.stdout
    );
    assert!(
        rows.iter()
            .any(|r| r.author_did == Author::Dmitri.app_identity()
                && r.subject == FERRITE_REPRODUCIBLE_BUILDS.subject),
        "Dmitri's app-signed claim is found too, attributed to Dmitri:\n{}",
        found.stdout
    );
}

/// WS-2
/// ```gherkin
/// @walking_skeleton @driving_port @real-io @adapter-integration @US-IPF-001
/// @AC-001.1 @AC-001.2 @AC-002.7 @I-IPF-5 @contract-shape:bounded-change
/// Scenario: Authors on different PDSes are all found in one pass
///   Given Priya has 3 self-attested claims on her bsky.social PDS
///   And Dmitri has 2 app-signed claims on pds.volkov.dev and Jeff 1 on pds.jeffbailey.us
///   When one ingest pass runs
///   Then all 6 claims are indexed, each attributed to its own author
///   And Priya's 3 are recorded as self-attested and Dmitri's and Jeff's as app-signed
///   And each repo was listed on the PDS its DID document names, and only there
///   And Jeff sees a pass summary of 3 configured, 3 read from their own PDS, none skipped
/// ```
#[test]
#[ignore = "DELIVER 01-01: per-DID listing + pass summary (ADR-077/078)"]
fn authors_on_different_pdses_are_all_found_in_one_pass() {
    let (world, published) = given_three_authors_publish_on_their_own_pdses();

    let pass = world.one_ingest_pass_runs();

    pass.assert_pass_completed(0, 3, 0, 0);
    then_index_holds_exactly(
        &world,
        &[
            (
                Author::Priya.did(),
                &published.priya,
                Provenance::SelfAttested,
            ),
            (
                &Author::Dmitri.app_identity(),
                &published.dmitri,
                Provenance::AppSigned,
            ),
            (
                &Author::Jeff.app_identity(),
                &published.jeff,
                Provenance::AppSigned,
            ),
        ],
    );
    assert_eq!(
        world.listings_on(Host::MorelBsky),
        vec![Author::Priya.did()]
    );
    assert_eq!(
        world.listings_on(Host::VolkovDev),
        vec![Author::Dmitri.did()]
    );
    assert_eq!(
        world.listings_on(Host::JeffbaileyUs),
        vec![Author::Jeff.did()]
    );
    assert_eq!(
        pass.refused_for(RefusalReason::Provenance),
        0,
        "no own-PDS record is refused for provenance (KPI)\n{}",
        pass.dump()
    );
    for author in [Author::Priya, Author::Dmitri, Author::Jeff] {
        pass.assert_read_from_own_pds(author.did());
    }
}

// =============================================================================
// US-IPF-001 focused scenarios
// =============================================================================

/// IPF-01
/// ```gherkin
/// @US-IPF-001 @AC-001.4 @real-io @contract-shape:bounded-change
/// Scenario: An author who moved PDS is followed to the new one
///   Given Priya's claims were indexed from her bsky.social PDS on the previous pass
///   And her DID document now names https://pds.priyaraman.dev
///   When the next ingest pass runs
///   Then her DID document was resolved again for this pass
///   And her claims are read from pds.priyaraman.dev and still admitted as self-attested
///   And nothing is read from her old PDS
/// ```
#[test]
#[ignore = "DELIVER 01-01: resolve every pass; follow a moved PDS (FR-2)"]
fn an_author_who_moved_pds_is_followed_to_the_new_one() {
    let (world, published) = given_three_authors_publish_on_their_own_pdses();
    world
        .one_ingest_pass_runs()
        .assert_pass_completed(0, 3, 0, 0);
    world.author_moves_to(Author::Priya, Host::PriyaramanDev);
    world.net.clear_request_log();

    let next = world.one_ingest_pass_runs();

    next.assert_pass_completed(0, 3, 0, 0);
    assert_eq!(
        world.net.did_document_fetches(Author::Priya.did()),
        1,
        "Priya's DID document is resolved afresh in the new pass"
    );
    assert_eq!(
        world.listings_on(Host::PriyaramanDev),
        vec![Author::Priya.did()]
    );
    assert!(
        world.listings_on(Host::MorelBsky).is_empty(),
        "nothing is read from the old PDS"
    );
    assert_eq!(
        next.refused_for(RefusalReason::Provenance),
        0,
        "{}",
        next.dump()
    );
    let priya_rows = world.rows_of(Author::Priya);
    assert_eq!(priya_rows.len(), published.priya.len());
    assert!(priya_rows.iter().all(|r| r.provenance == "self-attested"));
}

/// IPF-02
/// ```gherkin
/// @US-IPF-001 @AC-001.5 @error @adversarial @real-io @contract-shape:unbounded-preservation
/// Scenario: Records of another repo are never attributed to the requested author
///   Given Priya has 3 self-attested claims on her own PDS
///   And the PDS named for did:plc:mallory4k1z answers with Priya's records
///   When one ingest pass runs
///   Then no record is indexed under did:plc:mallory4k1z
///   And each of the 3 records is counted as refused for belonging to a foreign repo
///   And Priya's own 3 claims are indexed exactly once, under Priya
/// ```
#[test]
#[ignore = "DELIVER 01-02: records_of counts foreign-repo records (FR-4)"]
fn records_of_another_repo_are_never_attributed_to_the_requested_author() {
    let world = IndexerWorld::configured_with(&[Author::Priya, Author::Mallory]);
    let priya = world.publishes_self_attested(
        Author::Priya,
        &[
            CARGO_PIN_REPRODUCIBLE_BUILDS,
            CARGO_PIN_DEPENDENCY_PINNING,
            TIDEPOOL_MEMORY_SAFETY,
        ],
    );
    world.repo_answers_with_records_of(Author::Mallory, Author::Priya);

    let pass = world.one_ingest_pass_runs();

    pass.assert_pass_completed(0, 2, 0, 0);
    assert!(
        world.rows_of(Author::Mallory).is_empty(),
        "nothing indexed under Mallory"
    );
    assert_eq!(
        pass.refused_for(RefusalReason::ForeignRepo),
        3,
        "{}",
        pass.dump()
    );
    assert!(
        pass.refused_total() >= 3,
        "the rejected count includes foreign_repo"
    );
    then_index_holds_exactly(
        &world,
        &[(Author::Priya.did(), &priya, Provenance::SelfAttested)],
    );
}

/// IPF-03
/// ```gherkin
/// @US-IPF-001 @AC-001.6 @I-IPF-1 @error @adversarial @real-io @contract-shape:bounded-change
/// Scenario: A tampered self-attested record is still refused
///   Given Priya has 2 honest self-attested claims on her own PDS
///   And a third record of hers is stored under a key that does not match its content
///   When one ingest pass runs
///   Then that record is refused as a CID mismatch
///   And her other 2 claims are indexed as self-attested
/// ```
#[test]
#[ignore = "DELIVER 01-02: CID == rkey still refuses tampered records via own PDS"]
fn a_tampered_self_attested_record_is_still_refused() {
    let world = IndexerWorld::configured_with(&[Author::Priya]);
    let honest = world.publishes_self_attested(
        Author::Priya,
        &[CARGO_PIN_REPRODUCIBLE_BUILDS, CARGO_PIN_DEPENDENCY_PINNING],
    );
    let tampered =
        world.publishes_self_attested_under_a_wrong_key(Author::Priya, TIDEPOOL_MEMORY_SAFETY);

    let pass = world.one_ingest_pass_runs();

    pass.assert_pass_completed(0, 1, 0, 0);
    assert_eq!(
        pass.refused_for(RefusalReason::CidMismatch),
        1,
        "{}",
        pass.dump()
    );
    then_index_holds_exactly(
        &world,
        &[(Author::Priya.did(), &honest, Provenance::SelfAttested)],
    );
    assert!(world.indexed_rows().iter().all(|r| r.cid != tampered));
}

// =============================================================================
// US-IPF-003 — fallback for unresolvable authors (story line)
// =============================================================================

/// GIVEN did:plc:ghost0000 has no DID document anywhere; Jeff's PDS
/// (pds.jeffbailey.us) is the configured fallback and holds 2 app-signed
/// claims and 1 self-attested claim of Ghost's; Priya resolves normally.
fn given_ghost_is_unresolvable_and_jeffs_pds_is_the_fallback(
) -> (IndexerWorld, Vec<String>, Vec<String>) {
    let mut world = IndexerWorld::configured_with(&[Author::Priya, Author::Ghost]);
    world.publishes_self_attested(Author::Priya, &[CARGO_PIN_REPRODUCIBLE_BUILDS]);
    let app_signed =
        world.publishes_app_signed(Author::Ghost, &[GHOST_APP_SIGNED_ONE, GHOST_APP_SIGNED_TWO]);
    let self_attested = world.publishes_self_attested(Author::Ghost, &[GHOST_SELF_ATTESTED]);
    world.fallback_is(Host::JeffbaileyUs);
    (world, app_signed, self_attested)
}

/// IPF-04
/// ```gherkin
/// @US-IPF-003 @AC-003.1 @I-IPF-2 @real-io @contract-shape:bounded-change
/// Scenario: An unresolvable author's app-signed claims come through the fallback
///   Given did:plc:ghost0000 cannot be resolved
///   And the fallback pds.jeffbailey.us holds 2 app-signed claims and 1 self-attested claim for it
///   When one ingest pass runs
///   Then the 2 app-signed claims are indexed
///   And the self-attested claim is refused for provenance and is not in the index
///   And Jeff sees did:plc:ghost0000 read through the fallback because it was unresolvable
///   And the pass summary counts 1 own-PDS read and 1 fallback read
/// ```
#[test]
#[ignore = "DELIVER 01-02: plan_listing → Fallback; fallback is always relay origin"]
fn an_unresolvable_authors_app_signed_claims_come_through_the_fallback() {
    let (world, app_signed, self_attested) =
        given_ghost_is_unresolvable_and_jeffs_pds_is_the_fallback();

    let pass = world.one_ingest_pass_runs();

    pass.assert_pass_completed(0, 1, 1, 0);
    let ghost_rows = world.rows_of(Author::Ghost);
    let mut got: Vec<String> = ghost_rows.iter().map(|r| r.cid.clone()).collect();
    got.sort();
    let mut want = app_signed.clone();
    want.sort();
    assert_eq!(got, want, "exactly Ghost's app-signed claims are indexed");
    assert!(ghost_rows.iter().all(|r| r.provenance == "app-signed"));
    assert!(world
        .indexed_rows()
        .iter()
        .all(|r| r.cid != self_attested[0]));
    assert_eq!(
        pass.refused_for(RefusalReason::Provenance),
        1,
        "{}",
        pass.dump()
    );
    let read = pass.fallback_reads_of(Author::Ghost.did());
    assert_eq!(
        read.len(),
        1,
        "one source_fallback event for Ghost\n{}",
        pass.dump()
    );
    assert_eq!(read[0]["reason"], SkipReason::DidUnresolvable.token());
    assert_eq!(read[0]["fallback_url"], world.url_of(Host::JeffbaileyUs));
    assert!(
        pass.skips_of(Author::Ghost.did()).is_empty(),
        "a fallback read is not a skip"
    );
}

/// IPF-05
/// ```gherkin
/// @US-IPF-003 @AC-003.2 @I-IPF-2 @real-io @contract-shape:unbounded-preservation
/// Scenario: A resolvable author is never read from the fallback
///   Given Priya resolves to her bsky.social PDS
///   And the fallback pds.jeffbailey.us could also answer for her repo
///   When one ingest pass runs
///   Then Priya's records are read only from her bsky.social PDS
///   And the fallback was asked only about did:plc:ghost0000
/// ```
#[test]
#[ignore = "DELIVER 01-03: a resolved DID has no path to the fallback (FR-5)"]
fn a_resolvable_author_is_never_read_from_the_fallback() {
    let (world, _, _) = given_ghost_is_unresolvable_and_jeffs_pds_is_the_fallback();

    let pass = world.one_ingest_pass_runs();

    pass.assert_pass_completed(0, 1, 1, 0);
    assert_eq!(
        world.listings_on(Host::MorelBsky),
        vec![Author::Priya.did()]
    );
    assert_eq!(
        world.listings_on(Host::JeffbaileyUs),
        vec![Author::Ghost.did()],
        "the fallback is contacted only for the unresolvable DID"
    );
    pass.assert_read_from_own_pds(Author::Priya.did());
}

/// IPF-06
/// ```gherkin
/// @US-IPF-003 @AC-003.1 @I-IPF-2 @error @adversarial @real-io @contract-shape:unbounded-preservation
/// Scenario: A fallback that happens to be somebody's own PDS still never vouches for self-attested claims
///   Given did:plc:ghost0000 cannot be resolved
///   And the operator configured Priya's bsky.social PDS as the fallback
///   And that PDS answers for Ghost's repo with 1 self-attested claim
///   When one ingest pass runs
///   Then Ghost's self-attested claim is refused for provenance
///   And Priya's own claim is still admitted as self-attested
/// ```
#[test]
#[ignore = "DELIVER 01-02: Fallback is Relay with no URL comparison (OQ-IPF-3)"]
fn a_fallback_that_is_somebodys_own_pds_never_vouches_for_self_attested_claims() {
    let mut world = IndexerWorld::configured_with(&[Author::Priya, Author::Ghost]);
    let priya = world.publishes_self_attested(Author::Priya, &[CARGO_PIN_REPRODUCIBLE_BUILDS]);
    world.publishes_self_attested(Author::Ghost, &[GHOST_SELF_ATTESTED]);
    world.fallback_is(Host::MorelBsky);

    let pass = world.one_ingest_pass_runs();

    pass.assert_pass_completed(0, 1, 1, 0);
    assert_eq!(
        pass.refused_for(RefusalReason::Provenance),
        1,
        "{}",
        pass.dump()
    );
    assert!(world.rows_of(Author::Ghost).is_empty());
    then_index_holds_exactly(
        &world,
        &[(Author::Priya.did(), &priya, Provenance::SelfAttested)],
    );
}

/// IPF-07
/// ```gherkin
/// @US-IPF-003 @AC-003.3 @I-IPF-3 @error @real-io @contract-shape:bounded-change
/// Scenario: A failing fallback is isolated like any other source
///   Given did:plc:ghost0000 cannot be resolved and the fallback pds.jeffbailey.us answers 503
///   When one ingest pass runs
///   Then did:plc:ghost0000 is skipped as unresolvable, showing the fallback was tried and why it failed
///   And Priya's claim is indexed and the pass completes successfully
/// ```
#[test]
#[ignore = "DELIVER 01-04: fallback failure → skip with fallback_used/fallback_failure"]
fn a_failing_fallback_is_isolated_like_any_other_source() {
    let (world, _, _) = given_ghost_is_unresolvable_and_jeffs_pds_is_the_fallback();
    world.host_answers(Host::JeffbaileyUs, ListingPosture::Status(503));

    let pass = world.one_ingest_pass_runs();

    pass.assert_pass_completed(0, 1, 0, 1);
    let skip = pass.assert_skipped(Author::Ghost.did(), SkipReason::DidUnresolvable);
    assert_eq!(skip["fallback_used"], true, "{skip}");
    assert_eq!(
        skip["fallback_failure"],
        SkipReason::PdsUnreachable.token(),
        "{skip}"
    );
    assert_eq!(skip["pds_url"], world.url_of(Host::JeffbaileyUs), "{skip}");
    assert_eq!(world.rows_of(Author::Priya).len(), 1);
    assert!(world.rows_of(Author::Ghost).is_empty());
}

/// IPF-08
/// ```gherkin
/// @US-IPF-003 @AC-003.4 @DD-IPF-5 @error @adversarial @real-io @contract-shape:unbounded-preservation
/// Scenario: A fallback whose name leads only to a private address is never contacted
///   Given the production transport policy is in force (no loopback test seam)
///   And did:plc:ghost0000 cannot be resolved
///   And the fallback is https://localhost:<port>, a name that resolves only to loopback
///   When one ingest pass runs
///   Then did:plc:ghost0000 is skipped as unresolvable, with the fallback refused for its address
///   And no connection reached the fallback address
///   And because every configured DID was skipped, the pass ends with the total-outage code 3
/// ```
#[test]
#[ignore = "DELIVER 02-02: guarded DNS resolver refuses the fallback after DNS (AC-003.4)"]
fn a_fallback_whose_name_leads_only_to_a_private_address_is_never_contacted() {
    let directory = Tripwire::arm();
    let fallback = Tripwire::arm();
    let mut world = IndexerWorld::configured_with(&[Author::Ghost]);
    world.without_loopback_seam();
    world.directory_url_is(&directory.https_localhost_url());
    world.fallback_text_is(&fallback.https_localhost_url());

    let pass = world.one_ingest_pass_runs();

    pass.assert_pass_completed(3, 0, 0, 1);
    let skip = pass.assert_skipped(Author::Ghost.did(), SkipReason::DidUnresolvable);
    assert_eq!(skip["fallback_used"], true, "{skip}");
    assert_eq!(
        skip["fallback_failure"],
        SkipReason::PdsAddressRefused.token(),
        "{skip}"
    );
    assert_eq!(
        fallback.connections(),
        0,
        "the refused fallback address is never connected to"
    );
    assert_eq!(
        directory.connections(),
        0,
        "the refused directory address is never connected to"
    );
}

/// IPF-09
/// ```gherkin
/// @US-IPF-003 @AC-003.1 @DD-IPF-4 @OQ-IPF-2 @error @real-io @contract-shape:bounded-change
/// Scenario: A did:web author whose web host is down is read through the fallback
///   Given did:web:localhost is configured and its web host serves no DID document
///   And the fallback pds.jeffbailey.us holds 1 app-signed claim of that author
///   When one ingest pass runs
///   Then the did:web identity is accepted as a repo DID
///   And its app-signed claim is indexed through the fallback because it was unresolvable
/// ```
/// (Positive did:web resolution needs a TLS host on port 443 and is proven at
/// the adapter level by DELIVER — `pds_endpoint_of` + the fake did:web test.)
#[test]
#[ignore = "DELIVER 01-03: did:web accepted; unresolvable did:web is fallback-eligible"]
fn a_did_web_author_whose_web_host_is_down_is_read_through_the_fallback() {
    let mut world = IndexerWorld::configured_with(&[Author::Priya, Author::Wren]);
    world.publishes_self_attested(Author::Priya, &[CARGO_PIN_REPRODUCIBLE_BUILDS]);
    let wren = world.publishes_app_signed(Author::Wren, &[WREN_APP_SIGNED]);
    world.fallback_is(Host::JeffbaileyUs);

    let pass = world.one_ingest_pass_runs();

    pass.assert_pass_completed(0, 1, 1, 0);
    assert_eq!(
        pass.fallback_reads_of(Author::Wren.did()).len(),
        1,
        "{}",
        pass.dump()
    );
    let rows = world.rows_of(Author::Wren);
    assert_eq!(rows.iter().map(|r| r.cid.clone()).collect::<Vec<_>>(), wren);
}
