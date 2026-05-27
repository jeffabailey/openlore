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
        "claim add must succeed for FR-2 fixture; got status {} \
         \n--- stdout ---\n{}\n--- stderr ---\n{}",
        outcome.status, outcome.stdout, outcome.stderr,
    );

    let cid = parse_cid_from_stdout(&outcome.stdout);

    // The fake PDS contains exactly one record; its rkey == cid and
    // its collection == "org.openlore.claim". This is the federation
    // contract that powers idempotency (WS-9) and at-uri
    // reconstructibility (FR-3).
    let records = env.pds.records();
    assert_eq!(
        records.len(),
        1,
        "expected exactly one PDS record after one publish; got {}: {:?}",
        records.len(),
        records,
    );
    let record = &records[0];
    assert_eq!(
        record.rkey, cid,
        "FR-2 contract violated: PDS record rkey ({:?}) must equal parsed claim CID ({:?}); \
         full record: {:?}",
        record.rkey, cid, record,
    );
    assert_eq!(
        record.collection, "org.openlore.claim",
        "FR-2 contract violated: PDS record collection ({:?}) must equal \"org.openlore.claim\"; \
         full record: {:?}",
        record.collection, record,
    );
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
    assert_eq!(
        outcome.status, 0,
        "claim add must succeed for FR-3 fixture; got status {} \
         \n--- stdout ---\n{}\n--- stderr ---\n{}",
        outcome.status, outcome.stdout, outcome.stderr,
    );

    // Parse the CID and the printed at-uri from stdout (US-003 Example
    // 1 mockup: "at-uri: at://did:plc:test-jeff/org.openlore.claim/bafy...")
    let cid = parse_cid_from_stdout(&outcome.stdout);
    let printed_at_uri = parse_at_uri_from_stdout(&outcome.stdout);

    // Reconstructibility derivation (shared-artifacts-registry rule 3):
    // at_uri == "at://{author_did}/org.openlore.claim/{cid}". The bare
    // author DID (no `#fragment`) is the canonical PDS authority component.
    let bare_author_did = fixture
        .author_did
        .split('#')
        .next()
        .unwrap_or(&fixture.author_did);
    let expected_at_uri = format!("at://{}/org.openlore.claim/{}", bare_author_did, cid);

    // Reconstructibility check #1: the printed value equals the derived value
    assert_eq!(
        printed_at_uri, expected_at_uri,
        "FR-3 contract violated: printed at-uri ({:?}) must equal \
         derived at-uri ({:?}); silent at-uri normalization in the print path",
        printed_at_uri, expected_at_uri,
    );

    // Reconstructibility check #2: same value persisted in the DuckDB row
    assert_duckdb_publication_metadata_for_cid(&env, &cid, &expected_at_uri);
}

/// Parse the at-uri out of `at-uri: <at://...>` in stdout. Mirrors
/// `parse_cid_from_stdout` — the marker text is the load-bearing
/// contract `claim_publish::render_publish_success` prints on the
/// success block.
fn parse_at_uri_from_stdout(stdout: &str) -> String {
    let marker = "at-uri: ";
    let idx = stdout.find(marker).unwrap_or_else(|| {
        panic!("could not locate 'at-uri: <at://...>' marker in stdout:\n{stdout}")
    });
    let tail = &stdout[idx + marker.len()..];
    let at_uri = tail
        .lines()
        .next()
        .map(|s| s.trim().to_string())
        .unwrap_or_default();
    assert!(
        !at_uri.is_empty(),
        "found at-uri marker but no value followed it in stdout:\n{stdout}"
    );
    at_uri
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

    // Capture the compose preview output (sign+publish). The compose
    // preview is written to stdout BEFORE the sign prompt is consumed
    // (claim_add.rs step 4); answering Enter + Y then drives the rest
    // of the flow through the sign + publish branches. The result:
    // `publish_outcome.stdout` carries both the compose preview AND
    // every later block (Computing claim CID …, Written to local store
    // …, at-uri: …) — the preview is the prefix we extract below.
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
    assert_eq!(
        publish_outcome.status, 0,
        "claim add must succeed for FR-4 fixture; got status {} \
         \n--- stdout ---\n{}\n--- stderr ---\n{}",
        publish_outcome.status, publish_outcome.stdout, publish_outcome.stderr,
    );

    // Parse the CID actually published so the field-for-field helper
    // can pin the cid token without DELIVER guessing it (the CID is a
    // function of the live `composed_at` from the system clock — we
    // don't pin OPENLORE_TEST_NOW here because the round-trip identity
    // we're verifying is "the SAME composed_at flows compose → store →
    // query", NOT "the composed_at equals some pinned constant").
    let cid = parse_cid_from_stdout(&publish_outcome.stdout);

    // Capture the query output for the just-published subject.
    let query_outcome = run_openlore(&env, &["graph", "query", "--subject", &fixture.subject]);
    assert_eq!(
        query_outcome.status, 0,
        "graph query must succeed for FR-4 fixture; got status {} \
         \n--- stdout ---\n{}\n--- stderr ---\n{}",
        query_outcome.status, query_outcome.stdout, query_outcome.stderr,
    );

    // The pre-existing helper validates the query stdout against the
    // fixture (presence of subject/predicate/object/evidence/confidence/
    // author/cid, no bucket labels). FR-4 layers the COMPOSE-vs-QUERY
    // cross-comparison on top: every field value rendered at compose
    // time must reappear, byte-for-byte, in the query output.
    assert_graph_query_output_matches_fixture(&query_outcome, &fixture, &cid);

    // FR-4's load-bearing assertion: the value strings the renderer
    // emits for each shared field at compose time must equal what the
    // graph-query renderer emits, byte-for-byte. Any divergence is a
    // silent normalization bug (KPI-4 zero-normalization invariant).
    assert_compose_and_query_fields_match_byte_for_byte(
        &publish_outcome,
        &query_outcome,
    );
}

// -----------------------------------------------------------------------------
// FR-4 helpers — local because the byte-for-byte cross-comparison is unique
// to this scenario. Kept as small composable pure functions per the
// functional-paradigm rule: parse, extract, compare.
// -----------------------------------------------------------------------------

/// Asserts that every field rendered in the compose-preview block of
/// `publish_outcome.stdout` reappears with a byte-for-byte equal value
/// in the per-claim block of `query_outcome.stdout`.
///
/// Shared fields: subject, predicate, object, evidence, confidence,
/// author, composedAt. The renderers differ in column padding and the
/// compose-side bucket-label suffix on `confidence` — see
/// `extract_confidence_numeric` for the numeric-portion isolation.
/// Composed-time-only labels (no query-side counterpart) and
/// query-time-only fields (cid) are EXCLUDED from the cross-check; the
/// `assert_graph_query_output_matches_fixture` call upstream already
/// pins those.
fn assert_compose_and_query_fields_match_byte_for_byte(
    publish_outcome: &support::CliOutcome,
    query_outcome: &support::CliOutcome,
) {
    let compose_fields = extract_compose_preview_fields(&publish_outcome.stdout);
    let query_fields = extract_query_block_fields(&query_outcome.stdout);

    // Cross-checked field set (subset of compose-preview labels — the
    // labels the query renderer also emits). Composed-time-only labels
    // outside this set are intentionally not cross-checked because they
    // have no query-side counterpart.
    let shared_fields = [
        "subject",
        "predicate",
        "object",
        "evidence",
        "author",
        "composedAt",
    ];

    for field in &shared_fields {
        let compose_value = compose_fields.get(*field).unwrap_or_else(|| {
            panic!(
                "compose preview missing field {field:?}; preview:\n{}",
                publish_outcome.stdout
            )
        });
        let query_value = query_fields.get(*field).unwrap_or_else(|| {
            panic!(
                "query output missing field {field:?}; query stdout:\n{}",
                query_outcome.stdout
            )
        });
        assert_eq!(
            compose_value, query_value,
            "FR-4 KPI-4 violation: field {field:?} differs between compose preview \
             ({compose_value:?}) and query output ({query_value:?}) — silent \
             normalization detected.\n\
             --- compose preview stdout ---\n{}\n--- query stdout ---\n{}",
            publish_outcome.stdout, query_outcome.stdout,
        );
    }

    // Confidence is special: compose preview annotates with a bucket
    // label (`0.86 (well-evidenced)`) while query renders the raw f64
    // (`0.86`). The KPI-4 numeric byte-for-byte invariant compares only
    // the numeric portion — the bucket label is display annotation, not
    // a field value (WD-10 / D-12).
    let compose_confidence = compose_fields.get("confidence").unwrap_or_else(|| {
        panic!(
            "compose preview missing confidence; preview:\n{}",
            publish_outcome.stdout
        )
    });
    let query_confidence = query_fields.get("confidence").unwrap_or_else(|| {
        panic!(
            "query output missing confidence; query stdout:\n{}",
            query_outcome.stdout
        )
    });
    let compose_numeric = extract_confidence_numeric(compose_confidence);
    let query_numeric = extract_confidence_numeric(query_confidence);
    assert_eq!(
        compose_numeric, query_numeric,
        "FR-4 KPI-4 violation: confidence numeric differs between compose preview \
         ({compose_numeric:?}) and query output ({query_numeric:?}) — silent \
         normalization detected.\n\
         --- compose preview stdout ---\n{}\n--- query stdout ---\n{}",
        publish_outcome.stdout, query_outcome.stdout,
    );
}

/// Parse the compose-preview block out of `stdout`. The block starts at
/// the line containing `Compose preview` (claim_add.rs:294) and ends at
/// the sign prompt (`Press Enter to sign`). Returns a map from field
/// name (lower-cased label up to the `:`) to value (trimmed text after
/// the `:`). Lines that don't look like `  <label>: <value>` are
/// ignored — the heading line ("Compose preview …") doesn't have a
/// colon-delimited field shape and is intentionally skipped.
fn extract_compose_preview_fields(stdout: &str) -> std::collections::BTreeMap<String, String> {
    let start = stdout
        .find("Compose preview")
        .unwrap_or_else(|| panic!("compose preview marker missing in stdout:\n{stdout}"));
    let after_start = &stdout[start..];
    // Sign prompt marks the end of the preview block — claim_add.rs
    // emits "\nPress Enter to sign locally …" right after flushing the
    // preview. If for any reason the prompt is absent (EOF before
    // prompt printed) the slice runs to end-of-stdout which still
    // captures the entire preview block.
    let end = after_start
        .find("Press Enter to sign")
        .unwrap_or(after_start.len());
    let block = &after_start[..end];

    parse_labeled_field_block(block)
}

/// Parse the per-claim block out of the graph-query stdout. The block
/// sits between the local-only header ("Showing local claims only.")
/// and the federation footer ("(Federated peers …"). Returns a map
/// from field name to value, same shape as
/// [`extract_compose_preview_fields`].
fn extract_query_block_fields(stdout: &str) -> std::collections::BTreeMap<String, String> {
    let header = "Showing local claims only.";
    let footer_marker = "(Federated peers";
    let start = stdout
        .find(header)
        .map(|i| i + header.len())
        .unwrap_or_else(|| panic!("query header missing in stdout:\n{stdout}"));
    let after_header = &stdout[start..];
    let end = after_header
        .find(footer_marker)
        .unwrap_or(after_header.len());
    let block = &after_header[..end];

    parse_labeled_field_block(block)
}

/// Parse a block of `  label: value` lines into a `BTreeMap`. The
/// indent prefix (two spaces in the compose preview, none in the query
/// output) is stripped via `trim_start`. The label is everything before
/// the FIRST `:` (so `subject` and `at-uri` both work). The value is
/// everything after the first `:`, trimmed on both sides.
fn parse_labeled_field_block(
    block: &str,
) -> std::collections::BTreeMap<String, String> {
    let mut fields = std::collections::BTreeMap::new();
    for raw_line in block.lines() {
        let line = raw_line.trim_start();
        // Skip the heading line ("Compose preview (...)"), blank
        // separators, and any non-field UX text (e.g. "retracted by
        // author"). Field lines all match `^\s*<label>:\s*<value>$`.
        let Some(colon_idx) = line.find(':') else {
            continue;
        };
        let label = line[..colon_idx].trim();
        let value = line[colon_idx + 1..].trim();
        // Heuristic: drop anything where the label contains whitespace
        // (e.g. "Compose preview (claim is asserted by you, not as
        // truth)" trips the colon detector on the apostrophe? No — but
        // the heading itself doesn't have a colon. Still, defense in
        // depth: real field labels are single-word identifiers.).
        if label.is_empty() || label.contains(' ') {
            continue;
        }
        fields.insert(label.to_string(), value.to_string());
    }
    fields
}

/// Extract the leading numeric token from a confidence value string.
/// Compose preview renders `0.86 (well-evidenced)`; query renders
/// `0.86`. Both have the numeric portion as the first whitespace-
/// separated token. KPI-4 demands the numeric byte-for-byte; the
/// bucket-label annotation is display only (WD-10 / D-12).
fn extract_confidence_numeric(value: &str) -> &str {
    value
        .split_whitespace()
        .next()
        .unwrap_or(value)
}
