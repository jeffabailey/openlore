//! indexer-per-did-pds-fetch — `openlore search` labels self-attested claims
//! (US-IPF-005, ADR-079): the stored provenance travels over the search wire
//! and the CLI shows `[self-attested]` next to `[verified]` for those rows only.
//!
//! Layer 3, example-only. Driving ports: the REAL `openlore search` CLI against
//! either the REAL `openlore-indexer serve` (over an index built by a real
//! per-DID ingest pass) or, for the backward-compatibility scenarios, a canned
//! stand-in for an OLD indexer that never sends the provenance field.
//!
//! Story line (Pillar 2): IPF-31 → IPF-32 → IPF-33 reuse
//! `given_priyas_self_attested_and_dmitris_app_signed_claims_are_indexed`.
//!
//! Activated by DELIVER step 02-04.
//
// SCAFFOLD: true

mod support;

#[path = "support/indexer_network.rs"]
mod indexer_network;

use indexer_network::claims::*;
use indexer_network::*;

/// GIVEN Priya's self-attested cargo-pin claim and Dmitri's app-signed ferrite
/// claim, both about reproducible-builds, were indexed by one pass.
fn given_priyas_self_attested_and_dmitris_app_signed_claims_are_indexed() -> IndexerWorld {
    let mut world = IndexerWorld::configured_with(&[Author::Priya, Author::Dmitri]);
    world.publishes_self_attested(Author::Priya, &[CARGO_PIN_REPRODUCIBLE_BUILDS]);
    world.publishes_app_signed(Author::Dmitri, &[FERRITE_REPRODUCIBLE_BUILDS]);
    world
        .one_ingest_pass_runs()
        .assert_pass_completed(0, 2, 0, 0);
    world
}

/// THEN in `stdout`, Priya's row is marked `[verified] [self-attested]` and
/// Dmitri's row only `[verified]`.
fn then_only_priyas_row_is_marked_self_attested(stdout: &str) {
    let rows = search_rows(stdout);
    let priya = rows
        .iter()
        .find(|r| r.author_did == Author::Priya.did())
        .unwrap_or_else(|| panic!("Priya's row is listed:\n{stdout}"));
    assert_eq!(priya.markers, "[verified] [self-attested]", "{stdout}");
    for row in rows.iter().filter(|r| r.author_did != Author::Priya.did()) {
        assert_eq!(
            row.markers, "[verified]",
            "app-signed rows are unchanged:\n{stdout}"
        );
    }
}

/// IPF-31
/// ```gherkin
/// @US-IPF-005 @AC-005.1 @J-005 @real-io @driving_port @contract-shape:pure-function
/// Scenario: Maria sees Priya's claim marked self-attested next to verified
///   Given Priya's self-attested claim and Dmitri's app-signed claim about reproducible-builds are indexed
///   When Maria searches the network for reproducible-builds
///   Then Priya's claim is marked [self-attested] next to [verified]
///   And Dmitri's claim is marked [verified] only
/// ```
#[test]
fn maria_sees_priyas_claim_marked_self_attested_next_to_verified() {
    let world = given_priyas_self_attested_and_dmitris_app_signed_claims_are_indexed();

    let found = world.maria_searches(&["--object", &CARGO_PIN_REPRODUCIBLE_BUILDS.object()]);

    assert_eq!(found.status, 0, "{}\n{}", found.stdout, found.stderr);
    then_only_priyas_row_is_marked_self_attested(&found.stdout);
}

/// IPF-32
/// ```gherkin
/// @US-IPF-005 @AC-005.3 @real-io @driving_port @contract-shape:pure-function
/// Scenario Outline: The label is the same whichever way Maria searches
///   Given Priya's self-attested claim and Dmitri's app-signed claim are indexed
///   When Maria searches by <dimension>
///   Then Priya's claim is marked [self-attested] next to [verified], and no other row is
///   Examples:
///     | dimension                                       |
///     | philosophy  (--object reproducible-builds)      |
///     | project     (--subject github:priyaraman/cargo-pin) |
///     | contributor (--contributor did:plc:priyaraman7x2k)  |
/// ```
#[test]
fn the_label_is_the_same_whichever_way_maria_searches() {
    let world = given_priyas_self_attested_and_dmitris_app_signed_claims_are_indexed();
    let object = CARGO_PIN_REPRODUCIBLE_BUILDS.object();

    for args in [
        vec!["--object", object.as_str()],
        vec!["--subject", CARGO_PIN_REPRODUCIBLE_BUILDS.subject],
        vec!["--contributor", Author::Priya.did()],
    ] {
        let found = world.maria_searches(&args);

        assert_eq!(
            found.status, 0,
            "{args:?}\n{}\n{}",
            found.stdout, found.stderr
        );
        assert!(
            search_rows(&found.stdout)
                .iter()
                .any(|r| r.author_did == Author::Priya.did()),
            "{args:?}: Priya's self-attested claim is found:\n{}",
            found.stdout
        );
        then_only_priyas_row_is_marked_self_attested(&found.stdout);
    }
}

/// IPF-33
/// ```gherkin
/// @US-IPF-005 @AC-005.1 @ADR-071 @real-io @driving_port @contract-shape:pure-function
/// Scenario: Inspecting a self-attested claim never claims a signature was checked
///   Given Priya's self-attested claim is indexed
///   When Maria inspects that claim by its CID
///   Then she is told it is self-attested by did:plc:priyaraman7x2k and was read from the author's own PDS
///   And she is never told a signature was verified
/// ```
#[test]
fn inspecting_a_self_attested_claim_never_claims_a_signature_was_checked() {
    let world = given_priyas_self_attested_and_dmitris_app_signed_claims_are_indexed();
    let cid = world.rows_of(Author::Priya)[0].cid.clone();

    let shown = world.maria_searches(&[
        "--object",
        &CARGO_PIN_REPRODUCIBLE_BUILDS.object(),
        "--show",
        &cid,
    ]);

    assert_eq!(shown.status, 0, "{}\n{}", shown.stdout, shown.stderr);
    assert!(
        shown.stdout.contains(&format!(
            "self-attested by {}; read from the author's own PDS",
            Author::Priya.did()
        )),
        "{}",
        shown.stdout
    );
    assert!(
        !shown.stdout.to_lowercase().contains("signature verified"),
        "{}",
        shown.stdout
    );
}

/// IPF-34
/// ```gherkin
/// @US-IPF-005 @AC-005.2 @ADR-079 @regression @guardrail @real-io @contract-shape:unbounded-preservation
/// Scenario: Results from an indexer that does not report provenance read exactly as before
///   Given an older indexer that answers with Dmitri's claim and no provenance
///   And a current indexer that answers with the same claim labelled app-signed
///   When Maria runs the same search against each
///   Then both outputs are identical, and the claim is marked [verified] only
/// ```
#[test]
fn results_from_an_indexer_that_does_not_report_provenance_read_exactly_as_before() {
    let env = maria_home();
    let dmitri = Author::Dmitri.app_identity();
    let old = CannedIndexer::answering(wire_response(vec![wire_row(
        &dmitri,
        FERRITE_REPRODUCIBLE_BUILDS,
        "bafyreiferritereproducible0001",
        None,
    )]));
    let current = CannedIndexer::answering(wire_response(vec![wire_row(
        &dmitri,
        FERRITE_REPRODUCIBLE_BUILDS,
        "bafyreiferritereproducible0001",
        Some(Provenance::AppSigned.token()),
    )]));
    let args = ["--object", "org.openlore.philosophy.reproducible-builds"];

    let from_old = search_against(&env, &args, &old.url);
    let from_current = search_against(&env, &args, &current.url);

    assert_eq!(from_old.status, 0, "{}", from_old.stderr);
    assert_eq!(from_current.status, 0, "{}", from_current.stderr);
    assert_eq!(
        from_old.stdout, from_current.stdout,
        "absent provenance reads as app-signed"
    );
    let rows = search_rows(&from_current.stdout);
    assert_eq!(rows.len(), 1, "{}", from_current.stdout);
    assert_eq!(rows[0].markers, "[verified]");
}

/// IPF-35
/// ```gherkin
/// @US-IPF-005 @AC-005.2 @ADR-079 @error @adversarial @real-io @contract-shape:bounded-change
/// Scenario: A result with a provenance Maria's CLI does not know is withheld, not misstated
///   Given an indexer answers with Dmitri's app-signed claim and a claim labelled "peer-vouched"
///   When Maria searches for reproducible-builds
///   Then Dmitri's claim is shown as before
///   And the "peer-vouched" claim is not shown, and Maria is told a result was withheld for its provenance
/// ```
#[test]
fn a_result_with_an_unknown_provenance_is_withheld_not_misstated() {
    let env = maria_home();
    let indexer = CannedIndexer::answering(wire_response(vec![
        wire_row(
            &Author::Dmitri.app_identity(),
            FERRITE_REPRODUCIBLE_BUILDS,
            "bafyreiferritereproducible0001",
            Some(Provenance::AppSigned.token()),
        ),
        wire_row(
            Author::Mallory.did(),
            CARGO_PIN_REPRODUCIBLE_BUILDS,
            "bafyreimallorypeervouched00001",
            Some("peer-vouched"),
        ),
    ]));

    let found = search_against(
        &env,
        &["--object", "org.openlore.philosophy.reproducible-builds"],
        &indexer.url,
    );

    assert_eq!(found.status, 0, "{}\n{}", found.stdout, found.stderr);
    let rows = search_rows(&found.stdout);
    assert_eq!(
        rows.len(),
        1,
        "only the known-provenance row is shown:\n{}",
        found.stdout
    );
    assert_eq!(rows[0].author_did, Author::Dmitri.app_identity());
    assert!(
        !found.stdout.contains(Author::Mallory.did()),
        "{}",
        found.stdout
    );
    assert!(
        found.stderr.to_lowercase().contains("provenance"),
        "Maria is told a result was withheld for its provenance:\n{}",
        found.stderr
    );
}
