//! Federation-round-trip acceptance tests for openlore-foundation
//! slice-01.
//!
//! The walking-skeleton's entire reason to exist: publish to the PDS,
//! read it back through the local graph query (which IS slice-01's
//! AppView read path; slice-03 adds true federation). These tests prove
//! the round-trip end-to-end across 3 claims with different predicates,
//! exercising:
//!
//!   1. CID stability across compose → sign → publish → store → query
//!   2. PDS rkey == claim_cid (ADR-006 + ADR-003 §verb publish)
//!   3. at_uri is reconstructible from author_did + claim_cid
//!      (shared-artifacts-registry §`at_uri`)
//!   4. graph query output matches compose-time field values byte-for-byte
//!      (KPI-4 the load-bearing one; data-models.md §Validation rules row 4)
//!
//! Layer: subprocess + filesystem + DuckDB + fake PDS (layer 3 / 5 mix
//! per the layered-discipline matrix). All adapters real except the
//! PDS double.
//!
//! Per Mandate 7: every test panics via `todo!()` at DISTILL handoff.
//
// SCAFFOLD: true

mod support;

use support::*;

// =============================================================================
// FR-1: Three claims, three predicates, all round-trip
// =============================================================================

/// FR-1: Publish three claims with different predicates / subjects /
/// philosophy objects; query each by subject; assert all three round-
/// trip with CIDs intact and field values matching the compose-time
/// fixtures. (slice-01 hypothesis: "the walking-skeleton journey works
/// end-to-end.")
///
/// This is the most thorough cross-cutting test in slice-01: it
/// exercises every adapter (CLI, claim-domain, lexicon, DuckDB, fake
/// PDS, fake identity) in their integrated configuration on a
/// non-trivial input set.
///
/// @federation @walking_skeleton @driving_port @US-003 @US-004 @J-001 @real-io
#[test]
fn federation_roundtrip_publish_three_claims_different_predicates_all_round_trip_with_cids_intact() {
    let env = TestEnv::initialized();
    let fixtures = fixture_three_claims_different_predicates();
    assert_eq!(fixtures.len(), 3, "fixture must produce exactly three claims");

    let mut published_cids: Vec<(String, String)> = Vec::new(); // (subject, cid)

    for fixture in &fixtures {
        let outcome = run_openlore_with_stdin(
            &env,
            &[
                "claim", "add",
                "--subject", &fixture.subject,
                "--predicate", &fixture.predicate,
                "--object", &fixture.object,
                "--evidence", &fixture.evidence[0],
                "--confidence", &fixture.confidence.to_string(),
            ],
            "\nY\n",
        );
        assert_eq!(
            outcome.status, 0,
            "claim add must succeed for fixture subject {:?}; got status {} \
             \n--- stdout ---\n{}\n--- stderr ---\n{}",
            fixture.subject, outcome.status, outcome.stdout, outcome.stderr,
        );
        let cid = parse_cid_from_stdout(&outcome.stdout);
        published_cids.push((fixture.subject.clone(), cid));
    }

    // CID-distinctness sanity check: three distinct compose-time inputs
    // MUST produce three distinct CIDs (the round-trip identity has no
    // meaning if two claims alias).
    let unique_cids: std::collections::HashSet<&String> =
        published_cids.iter().map(|(_, cid)| cid).collect();
    assert_eq!(
        unique_cids.len(),
        3,
        "expected three distinct CIDs from three distinct claims; got {:?}",
        published_cids,
    );

    // Each subject queried independently returns its claim with the
    // expected CID, and all fields match the compose-time fixture.
    for (fixture, (subject, cid)) in fixtures.iter().zip(&published_cids) {
        let query_outcome = run_openlore(&env, &["graph", "query", "--subject", subject]);
        assert_graph_query_output_matches_fixture(&query_outcome, fixture, cid);
    }

    // Cross-check: each CID appears in the fake PDS with the correct
    // at-uri.
    for (_subject, cid) in &published_cids {
        let expected_at_uri = format!("at://did:plc:test-jeff/org.openlore.claim/{}", cid);
        assert_pds_contains_record_at(&env, &expected_at_uri);
    }
}

// -----------------------------------------------------------------------------
// Local helpers
// -----------------------------------------------------------------------------

/// Parse the CID out of `Computing claim CID <cid>` in stdout. Mirrors
/// the WS helper of the same name — the marker text is the load-bearing
/// contract `claim_add.rs` prints right before persistence.
fn parse_cid_from_stdout(stdout: &str) -> String {
    let marker = "Computing claim CID ";
    let idx = stdout.find(marker).unwrap_or_else(|| {
        panic!("could not locate 'Computing claim CID <cid>' marker in stdout:\n{stdout}")
    });
    let tail = &stdout[idx + marker.len()..];
    let cid = tail
        .split_whitespace()
        .next()
        .map(|s| s.to_string())
        .unwrap_or_default();
    assert!(
        !cid.is_empty(),
        "found marker but no CID followed it in stdout:\n{stdout}"
    );
    cid
}

// =============================================================================
// FR-2: PDS rkey equals claim_cid
// =============================================================================

/// FR-2: The PDS record's rkey MUST equal the claim's CID. This is the
/// idempotency contract (US-003 Example 3): re-publishing the same CID
/// hits the same rkey and the PDS treats it as a no-op rather than
/// creating a duplicate. (ADR-003 §verb publish + ADR-006.)
///
/// @federation @US-003 @J-001 @real-io
#[test]
fn federation_roundtrip_pds_record_rkey_equals_claim_cid() {
    let env = TestEnv::initialized();
    let fixture = fixture_jeff_rust_memory_safety();

    let _outcome = run_openlore_with_stdin(
        &env,
        &[
            "claim", "add",
            "--subject", &fixture.subject,
            "--predicate", &fixture.predicate,
            "--object", &fixture.object,
            "--evidence", &fixture.evidence[0],
            "--confidence", &fixture.confidence.to_string(),
        ],
        "\nY\n",
    );
    let cid = "bafy..."; // todo!("parse from outcome.stdout")

    // The fake PDS contains exactly one record; its rkey == cid.
    todo!("DELIVER: assert env.pds.records().len() == 1; assert env.pds.records()[0].rkey == cid; assert env.pds.records()[0].collection == \"org.openlore.claim\"")
}

// =============================================================================
// FR-3: at_uri is reconstructible from author_did + claim_cid
// =============================================================================

/// FR-3: For any published claim, the at-uri printed by the CLI MUST
/// equal `at://{author_did}/org.openlore.claim/{claim_cid}` and the
/// same value MUST be present in the DuckDB row's `at_uri` column.
/// (shared-artifacts-registry §`at_uri`, validation rule 3; data-models.md
/// §Validation rules row 3.)
///
/// @federation @US-003 @US-004 @J-001 @real-io
#[test]
fn federation_roundtrip_at_uri_is_reconstructible_from_author_did_and_claim_cid() {
    let env = TestEnv::initialized();
    let fixture = fixture_jeff_rust_memory_safety();

    let outcome = run_openlore_with_stdin(
        &env,
        &[
            "claim", "add",
            "--subject", &fixture.subject,
            "--predicate", &fixture.predicate,
            "--object", &fixture.object,
            "--evidence", &fixture.evidence[0],
            "--confidence", &fixture.confidence.to_string(),
        ],
        "\nY\n",
    );

    // Parse the CID and the printed at-uri from stdout (US-003 Example
    // 1 mockup: "at-uri: at://did:plc:test-jeff/org.openlore.claim/bafy...")
    let cid = "bafy...";
    let printed_at_uri = "at://did:plc:test-jeff/org.openlore.claim/...";
    let expected_at_uri = format!("at://did:plc:test-jeff/org.openlore.claim/{}", cid);

    // Reconstructibility check: the printed value equals the derived value
    assert_eq!(printed_at_uri, expected_at_uri, "printed at-uri must equal derived at-uri");

    // Same value persisted in the DuckDB row
    assert_duckdb_publication_metadata_for_cid(&env, cid, &expected_at_uri);

    let _ = outcome; // silence unused warning until DELIVER wires it
    todo!("DELIVER: parse CID + at-uri from outcome.stdout; satisfy both assertions")
}

// =============================================================================
// FR-4: graph query output matches compose-time field values
// =============================================================================

/// FR-4: KPI-4 — the load-bearing round-trip identity guarantee. The
/// graph-query output for a just-published claim MUST display field
/// values that exactly match what was shown at compose time. ANY
/// silent normalization (timestamp re-formatted, evidence URL
/// re-encoded, confidence re-rendered) breaks this. (data-models.md
/// §Validation rules row 4; KPI-4 from `outcome-kpis.md`.)
///
/// @federation @walking_skeleton @driving_port @US-001 @US-004 @J-001 @real-io @kpi
#[test]
fn federation_roundtrip_graph_query_output_matches_compose_preview_field_for_field() {
    let env = TestEnv::initialized();
    let fixture = fixture_jeff_rust_memory_safety();

    // Capture the compose preview output (sign+publish)
    let publish_outcome = run_openlore_with_stdin(
        &env,
        &[
            "claim", "add",
            "--subject", &fixture.subject,
            "--predicate", &fixture.predicate,
            "--object", &fixture.object,
            "--evidence", &fixture.evidence[0],
            "--confidence", &fixture.confidence.to_string(),
        ],
        "\nY\n",
    );

    // Capture the query output
    let query_outcome = run_openlore(&env, &["graph", "query", "--subject", &fixture.subject]);

    // Field-for-field match. DELIVER's helper parses each output and
    // compares field by field; mismatches print BOTH outputs side by
    // side for debuggability.
    let cid = "bafy...";
    assert_graph_query_output_matches_fixture(&query_outcome, &fixture, cid);

    // Additionally: every field shown in the compose preview that also
    // shows in the query output MUST appear with the same value in
    // both. This is the byte-for-byte invariant.
    let _ = publish_outcome; // silence; DELIVER's helper does the cross-comparison
    todo!("DELIVER: extract the compose-preview block from publish_outcome.stdout; extract the rendered row from query_outcome.stdout; assert subject/predicate/object/evidence/confidence-numeric/author/composed_at all match byte-for-byte (timestamp normalization is a bug, NOT a feature)")
}
