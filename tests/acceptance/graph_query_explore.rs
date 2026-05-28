//! Slice-04 acceptance — `openlore graph query` explorer flags
//! (`--object`, `--contributor`, `--traverse`/`--depth`, `--weighted`,
//! `--explain <subject>`) per ADR-020.
//!
//! The user-visible surface for J-002 (explore the philosophy graph to inform
//! a decision). Every output row carries its author DID; every weighted
//! aggregate decomposes to per-author per-cid contributions (anti-merging in
//! aggregates, WD-73/I-GRAPH-2, extends slice-03 I-FED-1); sparse subgraphs
//! render honestly as [SPARSE]; weights are display-only and never persisted;
//! every traversal edge maps to exactly one signed claim.
//!
//! Layer 3 (subprocess / FS acceptance) per nw-tdd-methodology Layered Test
//! Discipline matrix + DD-GRAPH-1. Every scenario enters through the CLI
//! driving adapter via the real `openlore` binary (subprocess), exercises the
//! real `claim-domain` + `scoring` + `adapter-duckdb` pure-core / local-effect
//! stack over a REAL DuckDB seeded with own + peer claims, and (per Mandate 11)
//! is EXAMPLE-ONLY — sad paths are enumerated explicitly, never PBT-generated.
//! The pure scoring formula PROPERTIES live at layer 2 in `scoring_core.rs`.
//!
//! The graph is SEEDED into the REAL DuckDB (no new external fake): own claims
//! via the real `claim add` verb, peer claims via the real `peer add` +
//! `peer pull` verbs against the slice-03 `PeerPds` double. Scoring/traversal
//! is local read-only analysis over the real store, so NO new external fake is
//! needed — the slice-03 seeding helpers are reused + extended with a
//! multi-project graph seeder (`seed_federated_graph`) for the
//! triangulation/traversal fixtures.
//!
//! READ-ONLY slice (WD-79): NO scenario writes or signs a claim as the
//! behavior under test. Claims are seeded as preconditions; scoring/traversal
//! reads them. Every explorer command succeeds with the network disabled
//! (local-first; I-GRAPH-7).
//!
//! Covers:
//! - US-GRAPH-001: query by object (philosophy) + subject, attribution preserved
//! - US-GRAPH-002: query by contributor (DID) — one developer's reasoning trail
//! - US-GRAPH-003: transparent weighted/scored view; sparse renders sparse
//! - US-GRAPH-004: traverse contributor<->project<->philosophy edges
//! - US-GRAPH-005: audit a weight with --explain per-claim arithmetic
//! - WD-84/ADR-020: explorer flags on the existing `graph query` verb
//! - WD-87: explorer verbs imply federated scope (own + peers) by default
//! - WD-90/Q-DELIVER-SCORE-1: cross-project triangulation counts as breadth
//! - Integration gates 1-6 (shared-artifacts-registry.md): see acceptance-tests.md §7
//
// SCAFFOLD: true

mod support;

#[allow(unused_imports)]
use support::*;

// =============================================================================
// US-GRAPH-001 — query by object (philosophy), attribution preserved
// =============================================================================

/// GQE-1 (US-GRAPH-001 happy; WALKING SKELETON for slice-04): Maria's local
/// graph has 4 claims asserting `org.openlore.philosophy.dependency-pinning`
/// across 3 projects by 3 authors (Rachel on cargo 0.91, Tobias on deno 0.55,
/// Maria on deno 0.40, Rachel on nixpkgs 0.88). Running
/// `openlore graph query --object org.openlore.philosophy.dependency-pinning`
/// groups claims by subject (project), shows every claim under its author DID
/// with numeric confidence + display-only bucket + cid, and the footer states
/// the distinct-subject count (3) AND distinct-author count (3) AND the
/// no-merge guarantee. Drives integration gate 1
/// (`scoring_aggregate_preserves_attribution`, dimension baseline).
///
/// @us-graph-001 @real-io @driving_port @walking_skeleton @j-002 @kpi-graph-2 @happy
#[test]
fn graph_query_by_object_groups_by_subject_with_per_author_attribution() {
    let env = TestEnv::initialized();
    let object = "org.openlore.philosophy.dependency-pinning";

    // -- Precondition: seed the federated dependency-pinning subgraph into the
    // REAL DuckDB (own claims via `claim add`, peer claims via `peer add` +
    // `peer pull`). The seeder owns the canonical 4-claim / 3-project /
    // 3-author fixture from US-GRAPH-001 Example 1; it returns the live handles
    // (peer endpoints + pubkeys) so the assertion can pin per-author rows. --
    let graph = seed_federated_graph(
        &env,
        FederatedGraphFixture::dependency_pinning_three_authors(),
    );

    // -- Action: the object-dimension read through the driving port. Explorer
    // verbs imply federated scope (WD-87) — own + peers without an explicit
    // `--federated`. --
    let outcome = run_openlore(&env, &["graph", "query", "--object", object]);
    assert_eq!(
        outcome.status, 0,
        "graph query --object must exit 0;\n--- stdout ---\n{}\n--- stderr ---\n{}",
        outcome.stdout, outcome.stderr
    );

    // 1. Grouped BY SUBJECT (project): each of the 3 subjects heads a group.
    // 2. Every claim row carries its author DID + numeric confidence + a
    //    display-only bucket + cid (per-author attribution, anti-merging).
    // 3. Two claims with identical (subject, object) but different authors
    //    (deno: Tobias + Maria) render as TWO rows — NO multi-author aggregate.
    // 4. Footer states distinct-subject (3) + distinct-author (3) counts + the
    //    no-merge guarantee verbatim.
    //
    // Universe (port-exposed observable surface of the `--object` dimension
    // view): cli.graph_query.distinct_subjects_in_output (3),
    // cli.graph_query.distinct_authors_in_output (3),
    // cli.graph_query.rows_collapsed (0 — deno's two authors render as TWO
    // rows). Asserted against stdout (the CLI driving-port observable).
    let stdout = &outcome.stdout;

    // 1. Grouped BY SUBJECT: each of the 3 seeded subjects heads a group.
    let subjects = [
        "github:rust-lang/cargo",
        "github:NixOS/nixpkgs",
        "github:denoland/deno",
    ];
    let distinct_subjects_in_output = subjects
        .iter()
        .filter(|s| stdout.contains(&format!("subject: {s}")))
        .count();
    assert_eq!(
        distinct_subjects_in_output, 3,
        "cli.graph_query.distinct_subjects_in_output: expected all 3 subjects to head a group; \
         got {distinct_subjects_in_output};\n--- stdout ---\n{stdout}\n--- graph ---\n{graph:?}"
    );

    // 2. Every claim row carries its author DID + numeric confidence + a
    //    display-only bucket + cid (per-author attribution, anti-merging).
    let authors = [
        "did:plc:rachel-test",
        "did:plc:tobias-test",
        "did:plc:maria-test",
    ];
    let distinct_authors_in_output = authors
        .iter()
        .filter(|did| stdout.contains(&format!("author_did: {did}")))
        .count();
    assert_eq!(
        distinct_authors_in_output, 3,
        "cli.graph_query.distinct_authors_in_output: expected all 3 author DIDs to appear on \
         per-claim rows; got {distinct_authors_in_output};\n--- stdout ---\n{stdout}"
    );
    // Each seeded confidence appears verbatim as the numeric value (Gate 6 —
    // no bucket-rounding) followed by its display-only bucket label.
    for (confidence, bucket) in [
        ("0.91", "triangulated"),
        ("0.88", "well-evidenced"),
        ("0.55", "weighted"),
        // 0.4 sits at the [0.4, 0.7) display bucket boundary -> "weighted"
        // (the `< 0.4` speculative band is exclusive of 0.4).
        ("0.4", "weighted"),
    ] {
        assert!(
            stdout.contains(&format!("confidence: {confidence} ({bucket})")),
            "expected a per-claim row showing numeric confidence {confidence} with its \
             display-only bucket ({bucket});\n--- stdout ---\n{stdout}"
        );
    }
    // Every seeded claim's cid is present (each row independently attributable).
    let cid_rows = stdout
        .lines()
        .filter(|line| line.trim_start().starts_with("cid:"))
        .count();
    assert_eq!(
        cid_rows, 4,
        "expected exactly 4 cid-bearing rows (one per seeded claim, none merged); \
         got {cid_rows};\n--- stdout ---\n{stdout}"
    );

    // 3. deno has TWO claims by TWO authors (Tobias 0.55 + Maria 0.40); they
    //    render as TWO rows — NO multi-author aggregate (rows_collapsed == 0).
    for label in ["merged", "consensus", "aggregate"] {
        // The no-merge FOOTER legitimately contains "merged"; strip it first.
        let scanned = stdout.replace("No claims are merged.", "");
        assert!(
            !scanned.to_lowercase().contains(label),
            "anti-merging (KPI-GRAPH-2): the --object output must contain NO {label:?} row; \
             \n--- stdout ---\n{stdout}"
        );
    }

    // 4. Footer states distinct-subject (3) + distinct-author (3) counts + the
    //    content-frozen no-merge guarantee verbatim.
    assert!(
        stdout.contains("3 subject(s), 3 author(s)."),
        "expected the footer to state 3 subjects + 3 authors;\n--- stdout ---\n{stdout}"
    );
    assert!(
        stdout.contains("Each claim is attributed to its author DID. No claims are merged."),
        "expected the footer to carry the content-frozen no-merge guarantee verbatim; \
         \n--- stdout ---\n{stdout}"
    );
}

/// GQE-2 (US-GRAPH-001 edge; KPI-GRAPH-2): Aanya has her OWN claim that
/// `github:denoland/deno` embodies dependency-pinning (0.40) AND a pulled peer
/// claim from Tobias asserting the SAME (subject, object) at 0.55. Running
/// `--object dependency-pinning` displays BOTH as distinct rows under
/// `github:denoland/deno` — one under "(you)", one under "(subscribed peer)".
/// There is NO single "deno: 2 authors agree" row. (US-GRAPH-001 Example 3.)
///
/// @us-graph-001 @real-io @driving_port @j-002 @kpi-graph-2 @anti-merging @edge
#[test]
fn graph_query_by_object_identical_content_different_authors_renders_two_rows() {
    let env = TestEnv::initialized();
    let object = "org.openlore.philosophy.dependency-pinning";

    // Seed the identical-content pair on github:denoland/deno: the local user's
    // own claim (0.40) + a pulled peer claim from Tobias (0.55), same (subject,
    // object). The precise zero-merge fixture (US-GRAPH-001 Example 3).
    let graph = seed_federated_graph(
        &env,
        FederatedGraphFixture::deno_identical_content_two_authors(),
    );

    let outcome = run_openlore(&env, &["graph", "query", "--object", object]);
    assert_eq!(
        outcome.status, 0,
        "graph query --object must exit 0;\n--- stdout ---\n{}\n--- stderr ---\n{}",
        outcome.stdout, outcome.stderr
    );

    // Both claims appear as distinct rows under github:denoland/deno: one under
    // "(you)" (local 0.40), one under "(subscribed peer)" (Tobias 0.55). NO row
    // collapses the two authors. The renderer keeps each author's cid distinct.
    //
    // Universe (port-exposed observable surface of the `--object` dimension
    // view): cli.graph_query.distinct_subjects_in_output (1 — only deno),
    // cli.graph_query.distinct_authors_in_output (2 — local + Tobias),
    // cli.graph_query.cid_rows (2 — identical content stays TWO rows),
    // cli.graph_query.rows_collapsed (0 — no merged/consensus/aggregate row).
    // Asserted against stdout (the CLI driving-port observable).
    let stdout = &outcome.stdout;
    let local_did = env.identity.author_did(); // bare DID, "(you)"

    // 1. ONE subject heads the group: github:denoland/deno (both claims share it).
    assert!(
        stdout.contains("subject: github:denoland/deno"),
        "cli.graph_query.distinct_subjects_in_output: expected github:denoland/deno to head the \
         (single) subject group;\n--- stdout ---\n{stdout}\n--- graph ---\n{graph:?}"
    );

    // 2. BOTH authors appear on their own per-claim rows, each annotated with its
    //    relationship: the local user's row "(you)", Tobias's "(subscribed peer)".
    //    Identical content does NOT erase either author's attribution.
    assert!(
        stdout.contains(&format!("author_did: {local_did} (you)")),
        "expected the local user's claim row annotated '(you)' (the OWN 0.40 claim);\n\
         --- stdout ---\n{stdout}"
    );
    assert!(
        stdout.contains("author_did: did:plc:tobias-test (subscribed peer)"),
        "expected Tobias's claim row annotated '(subscribed peer)' (the PEER 0.55 claim);\n\
         --- stdout ---\n{stdout}"
    );

    // 3. Each author's numeric confidence appears verbatim on its row — the two
    //    identical-content claims keep their DISTINCT confidences (0.40 renders as
    //    the minimal decimal `0.4`; both sit in the [0.4, 0.7) 'weighted' bucket).
    for (confidence, bucket) in [("0.4", "weighted"), ("0.55", "weighted")] {
        assert!(
            stdout.contains(&format!("confidence: {confidence} ({bucket})")),
            "expected a per-claim row showing numeric confidence {confidence} with its \
             display-only bucket ({bucket});\n--- stdout ---\n{stdout}"
        );
    }

    // 4. The identical-content pair renders as TWO cid-bearing rows — NOT merged
    //    into one. Count the canonical `cid:` field lines (each row independently
    //    attributable; the renderer never collapses the two authors' claims).
    let cid_rows = stdout
        .lines()
        .filter(|line| line.trim_start().starts_with("cid:"))
        .count();
    assert_eq!(
        cid_rows, 2,
        "anti-merging (KPI-GRAPH-2): expected exactly 2 cid-bearing rows (identical content by \
         two authors stays TWO rows, none merged); got {cid_rows};\n--- stdout ---\n{stdout}"
    );

    // 5. NO combined consensus/aggregate row: the two authors are never collapsed.
    //    The no-merge FOOTER legitimately contains "merged"; strip it first.
    for label in ["merged", "consensus", "aggregate"] {
        let scanned = stdout.replace("No claims are merged.", "");
        assert!(
            !scanned.to_lowercase().contains(label),
            "anti-merging (KPI-GRAPH-2): the --object output must contain NO {label:?} row — the \
             identical-content pair coexists as two attributed rows;\n--- stdout ---\n{stdout}"
        );
    }

    // 6. The footer states the count honestly: 1 subject, 2 authors, plus the
    //    content-frozen no-merge guarantee verbatim.
    assert!(
        stdout.contains("1 subject(s), 2 author(s)."),
        "expected the footer to honestly state 1 subject + 2 authors;\n--- stdout ---\n{stdout}"
    );
    assert!(
        stdout.contains("Each claim is attributed to its author DID. No claims are merged."),
        "expected the footer to carry the content-frozen no-merge guarantee verbatim;\n\
         --- stdout ---\n{stdout}"
    );
}

/// GQE-3 (US-GRAPH-001 regression; WD-87): bare `graph query --subject <S>`
/// WITHOUT any explorer flag is byte-identical to slice-01 behavior (own
/// claims only — the slice-03 federated default-off contract is preserved).
/// The new explorer flags are strictly opt-in; the bare --subject path does
/// NOT widen to peers. (US-GRAPH-001 Example 2 + AC "subject unchanged".)
///
/// @us-graph-001 @real-io @driving_port @j-002 @regression @default-off
#[test]
fn graph_query_bare_subject_is_unchanged_from_prior_slice_behavior() {
    let env = TestEnv::initialized();
    let subject = "github:rust-lang/cargo";

    // Seed an own claim + a pulled peer claim about the SAME subject so the
    // regression is load-bearing: if the bare --subject path EVER widened to
    // peers under the explorer changes, the peer row WOULD appear. It must not.
    let graph = seed_federated_graph(&env, FederatedGraphFixture::cargo_own_plus_one_peer());

    // BASELINE captured before any explorer flag is exercised is the slice-01/03
    // own-claims-only contract. The bare --subject run after seeding peers must
    // still show ONLY the own claim (WD-87: bare --subject is unchanged).
    let outcome = run_openlore(&env, &["graph", "query", "--subject", subject]);
    assert_eq!(
        outcome.status, 0,
        "bare graph query --subject must exit 0;\n--- stdout ---\n{}\n--- stderr ---\n{}",
        outcome.stdout, outcome.stderr
    );

    let stdout = &outcome.stdout;

    // The seeded fixture mixes the local user's OWN cargo claim with one
    // subscribed PEER's cargo claim about the SAME subject. The peer row is the
    // load-bearing precondition: if the bare `--subject` path EVER widened to
    // peers, the peer's DID would surface. Derive the peer's bare DID from the
    // recorded seeding shape (the row whose author is NOT the local user).
    let local_did = env.identity.author_did();
    let peer_did = graph
        .seeded
        .iter()
        .map(|c| c.author_did.as_str())
        .find(|did| *did != local_did)
        .expect("fixture seeds at least one PEER claim distinct from the local user");

    // 1. The OWN claim renders: the bare `--subject` path still surfaces the
    //    local user's cargo claim verbatim (subject + numeric confidence 0.91 +
    //    its object) — the slice-01/03 own-claims-only contract holds.
    assert!(
        stdout.contains(subject),
        "bare --subject output must render the queried subject {subject};\n--- stdout ---\n{stdout}"
    );
    assert!(
        stdout.contains("0.91") && stdout.contains("org.openlore.philosophy.dependency-pinning"),
        "bare --subject output must render the local user's OWN claim verbatim \
         (confidence 0.91 + its object) — the slice-01/03 own-claims-only default;\n\
         --- stdout ---\n{stdout}"
    );

    // 2. DEFAULT-OFF REGRESSION (WD-87): the seeded peer's DID never appears on
    //    the bare `--subject` path. The explorer surface is strictly opt-in —
    //    bare `--subject` (no --object/--contributor/--traverse/--weighted/
    //    --explain, no --federated) does NOT widen to peers.
    assert!(
        !stdout.contains(peer_did),
        "bare --subject output (no explorer flag) must NOT name the seeded peer DID {peer_did} — \
         the explorer surface is additive/opt-in and the bare --subject path stays \
         own-claims-only (WD-87 bare-subject-unchanged regression);\n\
         --- stdout ---\n{stdout}\n--- graph ---\n{graph:?}"
    );
}

/// GQE-4 (US-GRAPH-001 error/edge): Maria typos the philosophy URI
/// (`...dependancy-pinning`, misspelled). The CLI finds zero matches and
/// prints "No claims found for object ...; did you mean ...?" with a near-match
/// suggestion. Exit code is 0 (a valid empty result, not an error).
/// (US-GRAPH-001 Example 4 + UAT scenario 4.)
///
/// @us-graph-001 @real-io @driving_port @j-002 @error
#[test]
fn graph_query_by_object_unknown_philosophy_returns_empty_with_suggestion_exit_zero() {
    let env = TestEnv::initialized();

    // Seed the CORRECT philosophy so the near-match suggestion has a real
    // neighbour to propose ("did you mean org.openlore.philosophy.dependency-pinning?").
    let graph = seed_federated_graph(
        &env,
        FederatedGraphFixture::dependency_pinning_three_authors(),
    );

    // The user queries a MISSPELLED object URI (no claims match).
    let outcome = run_openlore(
        &env,
        &[
            "graph",
            "query",
            "--object",
            "org.openlore.philosophy.dependancy-pinning",
        ],
    );

    // Empty result is NOT an error: exit 0 (a valid not-yet-found state).
    assert_eq!(
        outcome.status, 0,
        "graph query for an unknown object must exit 0 (valid empty result, not an error);\n\
         --- stdout ---\n{}\n--- stderr ---\n{}",
        outcome.stdout, outcome.stderr
    );

    // The misspelled object yields a no-claims-found message naming the queried
    // (misspelled) object, plus a near-match suggestion naming the correctly-
    // spelled philosophy that IS seeded (US-GRAPH-001 Example 4 / UAT scenario 4).
    //
    // Universe (port-exposed observable surface of the empty `--object` view):
    // cli.graph_query.no_claims_message_present (the no-claims-found line names
    // the MISSPELLED object), cli.graph_query.suggestion_present (a "Did you
    // mean ...?" line names the SEEDED correctly-spelled philosophy),
    // cli.graph_query.rows (0 — no per-claim row). Asserted against stdout (the
    // CLI driving-port observable).
    let stdout = &outcome.stdout;

    // The seeded fixture asserts the correctly-spelled philosophy; the
    // near-match suggestion must propose it (the closest existing object string).
    let seeded_object = graph
        .seeded
        .iter()
        .map(|c| c.object.as_str())
        .find(|o| *o == "org.openlore.philosophy.dependency-pinning")
        .expect("fixture seeds the correctly-spelled dependency-pinning philosophy");

    // 1. No-claims-found message NAMES the misspelled object the user queried.
    assert!(
        stdout.contains("No claims found for object org.openlore.philosophy.dependancy-pinning"),
        "expected a no-claims-found message naming the misspelled object the user queried;\n\
         --- stdout ---\n{stdout}\n--- graph ---\n{graph:?}"
    );

    // 2. A near-match suggestion proposes the correctly-spelled seeded philosophy.
    assert!(
        stdout.contains(&format!("Did you mean {seeded_object}?")),
        "expected a near-match suggestion ('Did you mean {seeded_object}?') naming the \
         correctly-spelled philosophy that IS seeded;\n--- stdout ---\n{stdout}"
    );

    // 3. Empty is HONEST — no per-claim cid row is manufactured.
    let cid_rows = stdout
        .lines()
        .filter(|line| line.trim_start().starts_with("cid:"))
        .count();
    assert_eq!(
        cid_rows, 0,
        "expected ZERO per-claim cid rows for an unknown object (clean empty, not a fabricated \
         result); got {cid_rows};\n--- stdout ---\n{stdout}"
    );
}

/// GQE-5 (US-GRAPH-001 / I-GRAPH-7): every explorer command succeeds with the
/// network disabled — `--object` reads the LOCAL graph only (no socket). This
/// is the local-first guardrail (WD-79/WD-92; extends slice-01 KPI-5 / I-9).
///
/// @us-graph-001 @real-io @driving_port @j-002 @local-first @i-graph-7
#[test]
fn graph_query_by_object_succeeds_with_network_disabled() {
    let env = TestEnv::initialized();
    let object = "org.openlore.philosophy.dependency-pinning";

    let graph = seed_federated_graph(
        &env,
        FederatedGraphFixture::dependency_pinning_three_authors(),
    );

    // Run the object query with the per-process network-disabled seam engaged
    // (no PDS/peer endpoint reachable). A read-only local explorer must still
    // succeed: scoring/traversal/dimension reads touch only the local store.
    let outcome = run_openlore_network_disabled(&env, &["graph", "query", "--object", object]);
    assert_eq!(
        outcome.status, 0,
        "graph query --object must succeed with the network disabled (local-first; WD-79/WD-92);\n\
         --- stdout ---\n{}\n--- stderr ---\n{}",
        outcome.stdout, outcome.stderr
    );

    // The object dimension view renders its FULL attributed result from the
    // LOCAL store alone — the same observable surface as the GQE-1 happy path,
    // just with no reachable network. Asserting the SAME universe as GQE-1
    // (port-exposed stdout slots) proves the read path is genuinely local-first,
    // not merely "exits 0".
    //
    // Universe (port-exposed observable surface of the network-disabled
    // `--object` dimension view): cli.graph_query.distinct_subjects_in_output
    // (3), cli.graph_query.distinct_authors_in_output (3),
    // cli.graph_query.cid_rows (4 — none merged), the no-merge footer, AND
    // pds.create_record.call_count (0 — no outbound call attempted).
    let stdout = &outcome.stdout;

    // 1. Grouped BY SUBJECT: each of the 3 seeded subjects heads a group —
    //    rendered from the local DuckDB with no network.
    let subjects = [
        "github:rust-lang/cargo",
        "github:NixOS/nixpkgs",
        "github:denoland/deno",
    ];
    let distinct_subjects_in_output = subjects
        .iter()
        .filter(|s| stdout.contains(&format!("subject: {s}")))
        .count();
    assert_eq!(
        distinct_subjects_in_output, 3,
        "cli.graph_query.distinct_subjects_in_output: expected all 3 subjects to head a group \
         from the LOCAL store with the network disabled; got {distinct_subjects_in_output};\n\
         --- stdout ---\n{stdout}\n--- graph ---\n{graph:?}"
    );

    // 2. Every claim row carries its author DID — full per-author attribution
    //    survives the network-disabled local read (anti-merging, WD-73).
    let authors = [
        "did:plc:rachel-test",
        "did:plc:tobias-test",
        "did:plc:maria-test",
    ];
    let distinct_authors_in_output = authors
        .iter()
        .filter(|did| stdout.contains(&format!("author_did: {did}")))
        .count();
    assert_eq!(
        distinct_authors_in_output, 3,
        "cli.graph_query.distinct_authors_in_output: expected all 3 author DIDs on per-claim rows \
         from the LOCAL store with the network disabled; got {distinct_authors_in_output};\n\
         --- stdout ---\n{stdout}"
    );

    // 3. The full 4-claim attributed result renders (none merged) — the
    //    network-disabled read is complete, not degraded.
    let cid_rows = stdout
        .lines()
        .filter(|line| line.trim_start().starts_with("cid:"))
        .count();
    assert_eq!(
        cid_rows, 4,
        "expected exactly 4 cid-bearing rows from the LOCAL store with the network disabled \
         (full result, none merged); got {cid_rows};\n--- stdout ---\n{stdout}"
    );

    // 4. The content-frozen no-merge footer renders verbatim — the local-first
    //    read carries the same honest framing as the networked path.
    assert!(
        stdout.contains("3 subject(s), 3 author(s)."),
        "expected the footer to state 3 subjects + 3 authors (network disabled);\n\
         --- stdout ---\n{stdout}"
    );
    assert!(
        stdout.contains("Each claim is attributed to its author DID. No claims are merged."),
        "expected the footer to carry the content-frozen no-merge guarantee verbatim \
         (network disabled);\n--- stdout ---\n{stdout}"
    );

    // 5. I-GRAPH-7 local-first: NO outbound PDS call was attempted. The
    //    explorer is a pure LOCAL read — the fake PDS recorded zero
    //    create_record calls (port-exposed name pds.create_record.call_count).
    assert_no_pds_call_was_made(&env);
}

// =============================================================================
// US-GRAPH-002 — query by contributor (DID), one developer's reasoning trail
// =============================================================================

/// GQE-6 (US-GRAPH-002 happy): `did:plc:rachel-test` has authored 5 claims
/// across 4 subjects in Maria's local graph. Running
/// `openlore graph query --contributor did:plc:rachel-test` lists all 5 under
/// Rachel's DID with subject/object/confidence/cid, ending with the footer
/// "one developer's reasoning trail, not a community consensus".
/// (US-GRAPH-002 Example 1.)
///
/// @us-graph-002 @real-io @driving_port @j-002 @kpi-graph-2 @happy
#[test]
fn graph_query_by_contributor_lists_full_reasoning_trail_with_honest_framing() {
    let env = TestEnv::initialized();
    let rachel_did = "did:plc:rachel-test";

    // Seed Rachel's 5-claim / 4-subject trail into the local graph (peer claims
    // pulled into peer_claims). The seeder hosts exactly the US-GRAPH-002
    // Example 1 fixture (cargo x2, nixpkgs, tokio, serde).
    let graph = seed_federated_graph(
        &env,
        FederatedGraphFixture::rachel_five_claims_four_subjects(),
    );

    let outcome = run_openlore(&env, &["graph", "query", "--contributor", rachel_did]);
    assert_eq!(
        outcome.status, 0,
        "graph query --contributor must exit 0;\n--- stdout ---\n{}\n--- stderr ---\n{}",
        outcome.stdout, outcome.stderr
    );

    // All 5 claims listed under Rachel's DID, across all 4 subjects, each with
    // subject + object + confidence + cid. Footer: "one developer's reasoning
    // trail, not a community consensus". Every row carries Rachel's DID (no
    // row without an author; anti-merging).
    //
    // Universe (port-exposed observable surface of the `--contributor` trail
    // view): cli.graph_query.contributor_did_present (Rachel's DID heads the
    // trail), cli.graph_query.subjects_in_trail (all 4 of Rachel's subjects),
    // cli.graph_query.objects_in_trail + confidences_in_trail (each claim's
    // compose-time value verbatim — honest, not aggregated),
    // cli.graph_query.cid_rows (5 — one per seeded claim, none merged/dropped),
    // cli.graph_query.honest_trail_footer (the content-frozen J-002 framing).
    // Asserted against stdout (the CLI driving-port observable).
    let stdout = &outcome.stdout;

    // 1. The trail is headed by Rachel's DID — every claim is attributed to the
    //    queried contributor (no row without its author; anti-merging WD-73).
    assert!(
        stdout.contains(&format!("author_did: {rachel_did}")),
        "expected the trail to be headed by the contributor DID {rachel_did};\n\
         --- stdout ---\n{stdout}\n--- graph ---\n{graph:?}"
    );

    // 2. All 4 of Rachel's distinct subjects appear in the trail (cargo,
    //    nixpkgs, tokio, serde) — the trail spans every project she's claimed on.
    let distinct_subjects: std::collections::HashSet<&str> =
        graph.seeded.iter().map(|c| c.subject.as_str()).collect();
    assert_eq!(
        distinct_subjects.len(),
        4,
        "fixture precondition: Rachel's trail spans 4 distinct subjects; got {distinct_subjects:?}"
    );
    for subject in &distinct_subjects {
        assert!(
            stdout.contains(&format!("subject:    {subject}")),
            "expected the contributor trail to list subject {subject};\n--- stdout ---\n{stdout}"
        );
    }

    // 3. Each of the 5 seeded claims renders with its object + numeric
    //    confidence VERBATIM (honest — the raw compose-time value, never an
    //    aggregate score; J-002 published-trail-not-surveillance).
    assert_eq!(
        graph.seeded.len(),
        5,
        "fixture precondition: Rachel authors exactly 5 claims; got {}",
        graph.seeded.len()
    );
    for claim in &graph.seeded {
        assert!(
            stdout.contains(&format!("object:     {}", claim.object)),
            "expected the trail to list object {} for one of Rachel's claims;\n\
             --- stdout ---\n{stdout}",
            claim.object
        );
        // serde renders the f64 as its minimal decimal (e.g. 0.8 not 0.80) —
        // serialize the seeded value the same way the renderer does so the
        // assertion pins the HONEST numeric, not a re-bucketed label.
        let confidence = serde_json::to_value(claim.confidence)
            .map(|v| v.to_string())
            .expect("seeded confidence serializes");
        assert!(
            stdout.contains(&format!("confidence: {confidence}")),
            "expected the trail to show the honest compose-time confidence {confidence} \
             for one of Rachel's claims (raw value, not an aggregate);\n--- stdout ---\n{stdout}"
        );
    }

    // 4. Exactly 5 cid-bearing rows — one per seeded claim, none merged or
    //    dropped (each claim in the trail is independently attributable).
    let cid_rows = stdout
        .lines()
        .filter(|line| line.trim_start().starts_with("cid:"))
        .count();
    assert_eq!(
        cid_rows, 5,
        "expected exactly 5 cid-bearing rows (one per seeded claim, none merged); \
         got {cid_rows};\n--- stdout ---\n{stdout}"
    );

    // 5. The footer states the honest framing VERBATIM: this is one developer's
    //    reasoning trail, NOT a community consensus (J-002 content-frozen).
    assert!(
        stdout.contains("one developer's reasoning trail, not a community consensus"),
        "expected the footer to carry the content-frozen honest-trail framing \
         ('one developer's reasoning trail, not a community consensus');\n--- stdout ---\n{stdout}"
    );
}

/// GQE-7 (US-GRAPH-002 edge): Tobias runs `--contributor did:plc:tobias-test`
/// (his OWN DID). The output lists his own authored claims annotated "(you)"
/// rather than "(subscribed peer)" — a valid self-review. Exit 0.
/// (US-GRAPH-002 Example 2.)
///
/// @us-graph-002 @real-io @driving_port @j-002 @edge
#[test]
fn graph_query_by_contributor_own_did_is_a_valid_self_review_annotated_you() {
    let env = TestEnv::initialized();

    // Seed the LOCAL user's own claims (no peers needed). The local user's DID
    // is the contributor queried; rows annotate "(you)".
    let graph = seed_federated_graph(&env, FederatedGraphFixture::own_claims_only_three());
    let own_did = env.identity.author_did().to_string();

    let outcome = run_openlore(&env, &["graph", "query", "--contributor", &own_did]);
    assert_eq!(
        outcome.status, 0,
        "graph query --contributor (own DID) must exit 0;\n--- stdout ---\n{}\n--- stderr ---\n{}",
        outcome.stdout, outcome.stderr
    );

    // Querying one's OWN DID is a valid self-review: every row lists the local
    // user's own claim annotated "(you)" (the source_table-"Own" relationship),
    // NEVER "(subscribed peer)"/"(unsubscribed cache)" — those label peers, and
    // there are no peers here. The honest-trail framing still applies: this is
    // ONE developer's reasoning trail (the local user's own), not a consensus.
    //
    // Universe (port-exposed observable surface of the OWN-DID `--contributor`
    // trail view): cli.graph_query.contributor_did_present (the local DID heads
    // the trail), cli.graph_query.self_annotation_you (the header carries
    // "(you)", NOT a peer relationship), cli.graph_query.subjects_in_trail (all
    // 3 of the local user's own subjects), cli.graph_query.cid_rows (3 — one per
    // seeded own claim, none merged/dropped), cli.graph_query.honest_trail_footer
    // (the content-frozen J-002 framing). Asserted against stdout (the CLI
    // driving-port observable).
    let stdout = &outcome.stdout;

    // 1. The trail is headed by the local user's OWN DID, annotated "(you)" — a
    //    self-review. The "(you)" relationship (source_table "Own") is what
    //    distinguishes this from querying a peer's DID.
    assert!(
        stdout.contains(&format!("author_did: {own_did} (you)")),
        "expected the OWN-DID trail header annotated '(you)' (a valid self-review, \
         US-GRAPH-002 Example 2); got own_did={own_did};\n--- stdout ---\n{stdout}\n\
         --- graph ---\n{graph:?}"
    );

    // 2. The self-review is NEVER mislabeled as a peer relationship — "(you)" is
    //    the only annotation present (no "(subscribed peer)"/"(unsubscribed
    //    cache)"; those label OTHER authors, of which there are none here).
    for peer_label in ["(subscribed peer)", "(unsubscribed cache)"] {
        assert!(
            !stdout.contains(peer_label),
            "the OWN-DID self-review must NOT carry the peer annotation {peer_label:?} — \
             the local user's own claims are '(you)', never a peer;\n--- stdout ---\n{stdout}"
        );
    }

    // 3. All 3 of the local user's OWN subjects appear in the trail (each own
    //    claim is independently listed under the self DID; anti-merging WD-73).
    let distinct_subjects: std::collections::HashSet<&str> =
        graph.seeded.iter().map(|c| c.subject.as_str()).collect();
    assert_eq!(
        distinct_subjects.len(),
        3,
        "fixture precondition: the self-review trail spans 3 distinct own subjects; \
         got {distinct_subjects:?}"
    );
    for subject in &distinct_subjects {
        assert!(
            stdout.contains(&format!("subject:    {subject}")),
            "expected the self-review trail to list own subject {subject};\n--- stdout ---\n{stdout}"
        );
    }

    // 4. Exactly 3 cid-bearing rows — one per seeded own claim, none merged or
    //    dropped (each own claim is independently attributable).
    let cid_rows = stdout
        .lines()
        .filter(|line| line.trim_start().starts_with("cid:"))
        .count();
    assert_eq!(
        cid_rows, 3,
        "expected exactly 3 cid-bearing rows (one per seeded own claim, none merged); \
         got {cid_rows};\n--- stdout ---\n{stdout}"
    );

    // 5. The honest-trail footer still frames the self-review verbatim: even
    //    one's OWN trail is one developer's reasoning trail, not a consensus
    //    (J-002 content-frozen).
    assert!(
        stdout.contains("one developer's reasoning trail, not a community consensus"),
        "expected the footer to carry the content-frozen honest-trail framing on the \
         self-review path;\n--- stdout ---\n{stdout}"
    );
}

/// GQE-8 (US-GRAPH-002 edge): Aanya queries a DID she has never subscribed to
/// or pulled. The CLI prints "No local claims authored by <did>. Subscribe and
/// pull with `openlore peer add` + `openlore peer pull`." Exit 0.
/// (US-GRAPH-002 Example 3 + UAT scenario 3.)
///
/// @us-graph-002 @real-io @driving_port @j-002 @error
#[test]
fn graph_query_by_contributor_absent_did_degrades_with_subscribe_pull_hint_exit_zero() {
    let env = TestEnv::initialized();

    // Seed an unrelated own claim so the graph is non-empty but contains NOTHING
    // by the queried stranger DID.
    let graph = seed_federated_graph(&env, FederatedGraphFixture::own_claims_only_three());

    let stranger_did = "did:plc:stranger-test";
    let outcome = run_openlore(&env, &["graph", "query", "--contributor", stranger_did]);

    // Absent contributor is NOT an error: exit 0 (a valid empty result).
    assert_eq!(
        outcome.status, 0,
        "graph query --contributor for an absent DID must exit 0 (valid empty result);\n\
         --- stdout ---\n{}\n--- stderr ---\n{}",
        outcome.stdout, outcome.stderr
    );

    // The absent contributor degrades GRACEFULLY (J-002 anxiety mitigation —
    // sparse renders sparse, with a helpful next step): a no-local-claims
    // message naming the queried DID, plus a subscribe/pull hint pointing at
    // `openlore peer add` + `openlore peer pull` so the user knows how to
    // populate that contributor's trail. (US-GRAPH-002 Example 3 / UAT 3.)
    //
    // Universe (port-exposed observable surface of the absent-contributor
    // view): cli.graph_query.no_local_claims_message_present (the message names
    // the QUERIED stranger DID), cli.graph_query.subscribe_pull_hint_present
    // (the hint names BOTH `openlore peer add` + `openlore peer pull`),
    // cli.graph_query.cid_rows (0 — no fabricated trail row), AND
    // cli.graph_query.honest_consensus_framing_absent (NO "reasoning trail, not
    // a community consensus" footer — there is no trail to frame). Asserted
    // against stdout (the CLI driving-port observable).
    let stdout = &outcome.stdout;

    // 1. The no-local-claims message names the QUERIED stranger DID — the user
    //    sees WHICH contributor came back empty (self-explanatory, not silent).
    assert!(
        stdout.contains(&format!("No local claims authored by {stranger_did}.")),
        "expected a no-local-claims message naming the queried absent contributor {stranger_did};\n\
         --- stdout ---\n{stdout}\n--- graph ---\n{graph:?}"
    );

    // 2. The subscribe/pull hint names BOTH `openlore peer add` and
    //    `openlore peer pull` — the graceful-degrade next step that populates
    //    the absent contributor's trail (slice-03 peer add/pull hint precedent).
    assert!(
        stdout.contains("openlore peer add") && stdout.contains("openlore peer pull"),
        "expected a subscribe/pull hint naming BOTH `openlore peer add` + `openlore peer pull` so \
         the user can populate the absent contributor's trail;\n--- stdout ---\n{stdout}"
    );

    // 3. The degrade is HONEST — NO per-claim cid row is manufactured for a
    //    contributor with no local claims (empty stays empty; J-002).
    let cid_rows = stdout
        .lines()
        .filter(|line| line.trim_start().starts_with("cid:"))
        .count();
    assert_eq!(
        cid_rows, 0,
        "expected ZERO per-claim cid rows for an absent contributor (clean empty degrade, not a \
         fabricated trail); got {cid_rows};\n--- stdout ---\n{stdout}"
    );

    // 4. The honest-trail footer ("one developer's reasoning trail, not a
    //    community consensus") frames a FOUND trail — it must NOT appear when
    //    there is no trail to frame (the empty degrade carries the hint instead).
    assert!(
        !stdout.contains("one developer's reasoning trail, not a community consensus"),
        "the absent-contributor degrade must NOT carry the found-trail framing — there is no \
         trail; it carries the subscribe/pull hint instead;\n--- stdout ---\n{stdout}"
    );
}

/// GQE-9 (US-GRAPH-002 edge): Maria soft-removed Tobias (slice-03
/// `peer remove` without `--purge`) but retained his cached claims. Running
/// `--contributor did:plc:tobias-test` lists his cached claims annotated
/// "(unsubscribed cache)" rather than "(subscribed peer)", preserving the
/// slice-03 relationship labeling. No claim is shown without its author DID.
/// (US-GRAPH-002 Example 4.)
///
/// @us-graph-002 @real-io @driving_port @j-002 @edge
#[test]
fn graph_query_by_contributor_soft_removed_peer_labels_unsubscribed_cache() {
    let env = TestEnv::initialized();
    let tobias_did = "did:plc:tobias-test";

    // Seed Tobias as a SUBSCRIBED peer with cached claims, then soft-remove him
    // (peer remove without --purge) so his cache survives but his subscription
    // is gone — the slice-03 unsubscribed-cache relationship state.
    let graph = seed_federated_graph(&env, FederatedGraphFixture::tobias_then_soft_removed());

    let outcome = run_openlore(&env, &["graph", "query", "--contributor", tobias_did]);
    assert_eq!(
        outcome.status, 0,
        "graph query --contributor (soft-removed) must exit 0;\n--- stdout ---\n{}\n--- stderr ---\n{}",
        outcome.stdout, outcome.stderr
    );

    // A soft-removed peer's RETAINED cached claims still surface on the
    // `--contributor` trail, but the `removed_at IS NOT NULL` subscription state
    // re-classifies him from `SubscribedPeer` to `UnsubscribedCache`: the trail
    // header reads "(unsubscribed cache)", never "(subscribed peer)". Every row
    // is still attributed to Tobias's DID (no claim shown without its author;
    // anti-merging WD-73). (US-GRAPH-002 Example 4; slice-03 relationship reuse.)
    //
    // Universe (port-exposed observable surface of the soft-removed
    // `--contributor` trail view): cli.graph_query.contributor_did_present
    // (Tobias's DID heads the trail), cli.graph_query.relationship_annotation
    // (the header carries "(unsubscribed cache)", NOT "(subscribed peer)"),
    // cli.graph_query.cid_rows (one per retained cached claim — soft-remove drops
    // none), cli.graph_query.honest_trail_footer (the content-frozen J-002
    // framing). Asserted against stdout (the CLI driving-port observable).
    let stdout = &outcome.stdout;

    // 1. The trail is headed by Tobias's DID annotated "(unsubscribed cache)" —
    //    the soft-remove relationship state (removed_at IS NOT NULL →
    //    AuthorRelationship::UnsubscribedCache), reusing the slice-03 label.
    assert!(
        stdout.contains(&format!("author_did: {tobias_did} (unsubscribed cache)")),
        "expected the soft-removed peer's trail header annotated '(unsubscribed cache)' \
         (the removed_at-IS-NOT-NULL relationship; US-GRAPH-002 Example 4);\n\
         --- stdout ---\n{stdout}\n--- graph ---\n{graph:?}"
    );

    // 2. The soft-removed cache is NEVER mislabeled as an active subscription:
    //    "(subscribed peer)" must NOT appear — the subscription is gone, only the
    //    cache survives (WD-25 soft-remove retains cache but drops subscription).
    assert!(
        !stdout.contains("(subscribed peer)"),
        "the soft-removed peer's cached claims must NOT carry the '(subscribed peer)' annotation — \
         his subscription was removed (removed_at IS NOT NULL); the cache is '(unsubscribed cache)';\n\
         --- stdout ---\n{stdout}"
    );

    // 3. Every retained cached claim is listed under Tobias's DID — soft-remove
    //    RETAINS the cache, so each seeded claim appears with its object (anti-
    //    merging WD-73; none dropped).
    for claim in &graph.seeded {
        assert!(
            stdout.contains(&format!("object:     {}", claim.object)),
            "expected the soft-removed trail to list retained cached object {} for one of \
             Tobias's claims;\n--- stdout ---\n{stdout}",
            claim.object
        );
    }

    // 4. Exactly one cid-bearing row per retained cached claim — soft-remove
    //    drops NONE (the cache is intact, only the subscription is gone).
    let cid_rows = stdout
        .lines()
        .filter(|line| line.trim_start().starts_with("cid:"))
        .count();
    assert_eq!(
        cid_rows,
        graph.seeded.len(),
        "expected exactly {} cid-bearing rows (one per RETAINED cached claim; soft-remove drops \
         none); got {cid_rows};\n--- stdout ---\n{stdout}",
        graph.seeded.len()
    );

    // 5. The honest-trail footer still frames a soft-removed peer's cached trail
    //    verbatim: it is one developer's reasoning trail, not a community
    //    consensus (J-002 content-frozen) — the cache stays honestly attributed.
    assert!(
        stdout.contains("one developer's reasoning trail, not a community consensus"),
        "expected the footer to carry the content-frozen honest-trail framing on the \
         soft-removed (unsubscribed-cache) trail;\n--- stdout ---\n{stdout}"
    );
}

// =============================================================================
// US-GRAPH-003 — transparent weighted/scored view; sparse renders sparse
// =============================================================================

/// GQE-10 (US-GRAPH-003 happy; WALKING SKELETON for slice-04; Gate 2): Maria
/// runs `--object dependency-pinning --weighted` over a subgraph with cargo
/// (1 claim, conf 0.91, Rachel spans cargo+nixpkgs), nixpkgs (1 claim, 0.88),
/// deno (2 claims by 2 authors). The output ranks projects by adherence
/// weight, displays each weight WITH its inputs (claim count, distinct author
/// count, max confidence, cross-project span), prints the formula AND states
/// "no ML", and a footer states weights are a display-only aggregate view,
/// never stored. Drives gate 2 (`weight_equals_formula`).
///
/// @us-graph-003 @real-io @driving_port @walking_skeleton @j-002 @kpi-graph-1 @kpi-graph-3 @gate-2 @happy
#[test]
fn graph_query_weighted_ranks_projects_with_transparent_no_ml_formula() {
    let env = TestEnv::initialized();
    let object = "org.openlore.philosophy.dependency-pinning";

    // Seed the canonical weighted fixture from US-GRAPH-003 Example 1 /
    // data-models.md worked examples: cargo (Rachel 0.91, spans nixpkgs too),
    // nixpkgs (Rachel 0.88), deno (Tobias 0.55 + Maria 0.40).
    let graph = seed_federated_graph(
        &env,
        FederatedGraphFixture::dependency_pinning_weighted_worked_example(),
    );

    let outcome = run_openlore(&env, &["graph", "query", "--object", object, "--weighted"]);
    assert_eq!(
        outcome.status, 0,
        "graph query --object --weighted must exit 0;\n--- stdout ---\n{}\n--- stderr ---\n{}",
        outcome.stdout, outcome.stderr
    );

    // The `--weighted` view ranks the three projects by adherence weight (desc),
    // shows each weight WITH its inputs, prints the transparent no-ML formula,
    // and footers the never-stored display-only notice.
    //
    // Universe (port-exposed observable surface of the `--weighted` view, all
    // asserted against stdout — the CLI driving-port observable):
    //   cli.graph_query.ranking_order        — cargo > nixpkgs > deno (weight desc)
    //   cli.graph_query.weight_inputs_shown  — each pairing prints claims +
    //                                           authors + max-confidence + span
    //   cli.graph_query.formula_printed       — the auditable formula block
    //   cli.graph_query.no_ml_stated          — the output states "no ML"
    //   cli.graph_query.never_stored_footer   — the display-only / never-stored notice
    //
    // The expected weights are the documented WD-77 formula applied by hand to
    // the seeded claims (data-models.md §"The scoring formula" worked examples;
    // constants author_distinct_bonus 0.25, cross_project_triangulation_bonus
    // 0.50):
    //   cargo   : Rachel 0.91, spans cargo+nixpkgs -> +0.50 triangulation
    //             -> 0.91 * 1.0 + 0.50 = 1.41   (1 claim, 1 author)
    //   nixpkgs : Rachel 0.88, spans cargo+nixpkgs -> +0.50 triangulation
    //             -> 0.88 * 1.0 + 0.50 = 1.38   (1 claim, 1 author)
    //   deno    : Tobias 0.55 (x1.0) + Maria 0.40 (x1.25 second author)
    //             -> 0.55 + 0.50 = 1.05         (2 claims, 2 authors)
    let stdout = &outcome.stdout;

    // Fixture precondition: exactly the 4-claim / 3-project worked example.
    assert_eq!(
        graph.seeded.len(),
        4,
        "fixture precondition: the weighted worked example seeds 4 claims; got {}",
        graph.seeded.len()
    );

    // 1. RANKING ORDER — the three projects appear ranked by weight descending:
    //    cargo (1.41) > nixpkgs (1.38) > deno (1.05). Derive each project's
    //    first appearance position in stdout and assert the strict ordering
    //    (the port-exposed ranking surface).
    let cargo_pos = stdout
        .find("github:rust-lang/cargo")
        .expect("the weighted view must rank github:rust-lang/cargo");
    let nixpkgs_pos = stdout
        .find("github:NixOS/nixpkgs")
        .expect("the weighted view must rank github:NixOS/nixpkgs");
    let deno_pos = stdout
        .find("github:denoland/deno")
        .expect("the weighted view must rank github:denoland/deno");
    assert!(
        cargo_pos < nixpkgs_pos && nixpkgs_pos < deno_pos,
        "cli.graph_query.ranking_order: expected cargo (1.41) before nixpkgs (1.38) before \
         deno (1.05) — ranked by adherence weight descending;\n--- stdout ---\n{stdout}\n\
         --- graph ---\n{graph:?}"
    );

    // 2. Each project's weight is displayed (the closed-form formula applied to
    //    the seeded claims — reproducible by hand; Gate 2 / KPI-GRAPH-3).
    for (project, weight) in [
        ("github:rust-lang/cargo", "1.41"),
        ("github:NixOS/nixpkgs", "1.38"),
        ("github:denoland/deno", "1.05"),
    ] {
        assert!(
            stdout.contains(&format!("weight {weight}")),
            "cli.graph_query.weight_shown: expected {project} to display the documented \
             adherence weight {weight} (WD-77 formula by hand);\n--- stdout ---\n{stdout}"
        );
    }

    // 3. Each weight is shown WITH its inputs: claim count, distinct author
    //    count, and max confidence (the transparency contract — a weight is
    //    never shown as a bare number). deno carries 2 claims / 2 authors; the
    //    triangulated cargo + nixpkgs carry 1 claim / 1 author each.
    assert!(
        stdout.contains("claims  : 2") && stdout.contains("authors: 2"),
        "cli.graph_query.weight_inputs_shown: expected deno's weight inputs to show \
         claims: 2 + authors: 2;\n--- stdout ---\n{stdout}"
    );
    assert!(
        stdout.contains("claims  : 1") && stdout.contains("authors: 1"),
        "cli.graph_query.weight_inputs_shown: expected the single-author cargo/nixpkgs \
         pairings to show claims: 1 + authors: 1;\n--- stdout ---\n{stdout}"
    );
    for max_conf in ["0.91", "0.88", "0.55"] {
        assert!(
            stdout.contains(&format!("max-confidence {max_conf}")),
            "cli.graph_query.weight_inputs_shown: expected the max-confidence {max_conf} to \
             be displayed alongside its weight;\n--- stdout ---\n{stdout}"
        );
    }
    // Cross-project span: Rachel spans cargo + nixpkgs — surfaced as breadth.
    assert!(
        stdout.contains("spans") && stdout.contains("did:plc:rachel-test"),
        "cli.graph_query.weight_inputs_shown: expected the cross-project span line naming \
         did:plc:rachel-test spanning two projects;\n--- stdout ---\n{stdout}"
    );

    // 4. The FORMULA is printed AND the output states "no ML" (WD-71; the
    //    auditable-no-ML transparency contract). The formula names its inputs.
    assert!(
        stdout.contains("no ML"),
        "cli.graph_query.no_ml_stated: expected the output to state 'no ML' \
         (WD-71 transparency);\n--- stdout ---\n{stdout}"
    );
    assert!(
        stdout.contains("author_distinct_bonus")
            && stdout.contains("cross_project_triangulation")
            && stdout.contains("confidence"),
        "cli.graph_query.formula_printed: expected the printed formula to name its inputs \
         (confidence x author_distinct_bonus x cross_project_triangulation);\n\
         --- stdout ---\n{stdout}"
    );

    // 5. The never-stored footer: weights are a DISPLAY-ONLY aggregate view
    //    computed at query time, NOT stored / signed / published (WD-72).
    assert!(
        stdout.contains("DISPLAY-ONLY") && stdout.contains("NOT stored"),
        "cli.graph_query.never_stored_footer: expected the footer to state weights are a \
         DISPLAY-ONLY aggregate view, NOT stored (WD-72);\n--- stdout ---\n{stdout}"
    );

    // 6. Anti-merging: the view ranks projects but NEVER collapses the two deno
    //    authors into a faceless consensus — both contributing authors remain
    //    nameable in the breakdown (the decomposition survives the aggregate).
    assert!(
        stdout.contains("did:plc:tobias-test") && stdout.contains("did:plc:maria-test"),
        "cli.graph_query.anti_merging: expected deno's two contributing authors (Tobias + \
         Maria) to remain individually attributed in the weighted view;\n--- stdout ---\n{stdout}"
    );
}

/// GQE-11 (US-GRAPH-003 boundary; Gate 3; KPI-GRAPH-4 release-gate): Tobias
/// runs `--object actor-model --weighted` where only ONE claim matches (tokio,
/// 1 author, conf 0.50). The output labels tokio [SPARSE] with "(!) based on 1
/// claim by 1 author ... treat as a lead, not a defensible conclusion." NO
/// confidence is manufactured. This is the load-bearing sparse-honesty gate
/// (`sparse_renders_sparse`).
///
/// @us-graph-003 @real-io @driving_port @j-002 @kpi-graph-4 @gate-3 @sparse @release-gate
#[test]
fn graph_query_weighted_single_claim_single_author_renders_sparse_with_honesty_line() {
    let env = TestEnv::initialized();
    let object = "org.openlore.philosophy.actor-model";

    // Seed a single-claim single-author no-span subgraph (tokio, 1 claim, conf
    // 0.50). The precise sparse fixture (US-GRAPH-003 Example 2 / SC-3 leg).
    let graph = seed_federated_graph(
        &env,
        FederatedGraphFixture::actor_model_single_sparse_claim(),
    );

    let outcome = run_openlore(&env, &["graph", "query", "--object", object, "--weighted"]);
    assert_eq!(
        outcome.status, 0,
        "graph query --object --weighted (sparse) must exit 0;\n--- stdout ---\n{}\n--- stderr ---\n{}",
        outcome.stdout, outcome.stderr
    );

    // The single project is labeled [SPARSE]; the output states "based on 1
    // claim by 1 author", advises treating it as a lead not a conclusion, and
    // manufactures NO confidence.
    //
    // Universe (port-exposed observable surface of the `--weighted` view, all
    // asserted against stdout — the CLI driving-port observable):
    //   cli.graph_query.bucket[tokio]                    — labeled [SPARSE]
    //   cli.graph_query.sparse_honesty_line_present      — "based on 1 claim by 1 author"
    //   cli.graph_query.sparse_lead_not_conclusion       — "treat as a lead, not a
    //                                                       defensible conclusion"
    //   cli.graph_query.no_manufactured_confidence       — only the real 0.50 appears;
    //                                                       no dressed-up Strong/Moderate
    let stdout = &outcome.stdout;

    // Fixture precondition: exactly the 1-claim single-project sparse subgraph.
    assert_eq!(
        graph.seeded.len(),
        1,
        "fixture precondition: the sparse fixture seeds exactly 1 claim; got {}",
        graph.seeded.len()
    );

    // 1. The single tokio pairing is bucketed [SPARSE] — a single high-confidence
    //    opinion is NOT dressed up as Strong/Moderate (Gate 3; WD-74 breadth guard).
    let tokio_pos = stdout
        .find("github:tokio-rs/tokio")
        .expect("the weighted view must rank github:tokio-rs/tokio");
    assert!(
        stdout[tokio_pos..].contains("[SPARSE]"),
        "cli.graph_query.bucket[tokio]: expected the single-claim single-author tokio pairing to \
         be labeled [SPARSE] (Gate 3 sparse_renders_sparse);\n--- stdout ---\n{stdout}\n\
         --- graph ---\n{graph:?}"
    );

    // 2. The epistemic-honesty SENTENCE names the ACTUAL evidence base verbatim
    //    (WD-74): "based on 1 claim by 1 author". This is the 05-02 addition on
    //    top of the [SPARSE] label already printed by 05-01.
    assert!(
        stdout.contains("based on 1 claim by 1 author"),
        "cli.graph_query.sparse_honesty_line_present: expected the verbatim honesty line \
         'based on 1 claim by 1 author' naming the real evidence base (WD-74);\n\
         --- stdout ---\n{stdout}"
    );

    // 3. The lead-not-conclusion framing: a thin pairing is a lead to investigate,
    //    NEVER a settled verdict (WD-74 epistemic honesty; J-002 mitigation).
    assert!(
        stdout.contains("treat as a lead, not a defensible conclusion"),
        "cli.graph_query.sparse_lead_not_conclusion: expected the lead-not-conclusion advice \
         'treat as a lead, not a defensible conclusion' (WD-74);\n--- stdout ---\n{stdout}"
    );

    // 4. NO confidence is manufactured from the thin evidence: only the real
    //    compose-time 0.50 surfaces — never the false-confident STRONG/MODERATE
    //    labels (a single opinion is not aggregated into community endorsement).
    assert!(
        stdout.contains("max-confidence 0.5"),
        "cli.graph_query.no_manufactured_confidence: expected the real compose-time confidence \
         0.50 surfaced honestly;\n--- stdout ---\n{stdout}"
    );
    // Scope the no-false-confidence check to the tokio PAIRING line (the rank
    // line carrying its bucket), NOT the whole stdout — the formula legend
    // always lists all three labels ("bucket labels [STRONG]/[MODERATE]/[SPARSE]
    // are DISPLAY-ONLY"), which is documentation, not a per-pairing bucket.
    let tokio_bucket_line = stdout[tokio_pos..]
        .lines()
        .next()
        .expect("the tokio pairing must render a bucket line");
    assert!(
        !tokio_bucket_line.contains("[STRONG]") && !tokio_bucket_line.contains("[MODERATE]"),
        "cli.graph_query.no_manufactured_confidence: a single-claim single-author opinion must \
         NEVER be dressed up as [STRONG]/[MODERATE] on its pairing line;\n--- tokio line ---\n\
         {tokio_bucket_line}\n--- stdout ---\n{stdout}"
    );
}

/// GQE-12 (US-GRAPH-003 edge): Aanya runs `--object reproducible-builds
/// --weighted`. For github:denoland/deno, two distinct authors (Aanya 0.40,
/// Tobias 0.55) both claim reproducible-builds. The weight applies the
/// +per-additional-distinct-author bonus, ranking deno above a single-author
/// project with similar max confidence, and the breakdown line states
/// "multi-author: 2 distinct authors raise triangulation". Both authors stay
/// individually attributed. (US-GRAPH-003 Example 3.)
///
/// @us-graph-003 @real-io @driving_port @j-002 @kpi-graph-1 @kpi-graph-2 @happy
#[test]
fn graph_query_weighted_multi_author_support_raises_triangulation_weight() {
    let env = TestEnv::initialized();
    let object = "org.openlore.philosophy.reproducible-builds";

    // Seed deno with 2 distinct authors (Aanya 0.40 + Tobias 0.55) on
    // reproducible-builds, plus a single-author comparator project with similar
    // max confidence so the triangulation lift is observable in the ranking.
    let graph = seed_federated_graph(
        &env,
        FederatedGraphFixture::reproducible_builds_multi_author(),
    );

    let outcome = run_openlore(&env, &["graph", "query", "--object", object, "--weighted"]);
    assert_eq!(
        outcome.status, 0,
        "graph query --object --weighted (multi-author) must exit 0;\n--- stdout ---\n{}\n--- stderr ---\n{}",
        outcome.stdout, outcome.stderr
    );

    // deno's two distinct authors (Tobias 0.55 + Aanya 0.40) earn the
    // per-additional-distinct-author bonus, so deno's weight exceeds the
    // single-author cargo comparator at the SAME max confidence (0.55) — the
    // multi-author lift is observable in the ranking. The breakdown states the
    // content-frozen multi-author line, and BOTH deno authors stay individually
    // attributed (the aggregate decomposes; anti-merging WD-73 / ADR-022).
    //
    // Universe (port-exposed observable surface of the `--weighted` view, all
    // asserted against stdout — the CLI driving-port observable):
    //   cli.graph_query.multi_author_breadth_line — "multi-author: 2 distinct
    //                                                authors raise triangulation"
    //   cli.graph_query.ranking_order             — deno (multi-author) before
    //                                                cargo (single-author) at
    //                                                similar max confidence
    //   cli.graph_query.deno_authors_attributed   — both Tobias + Aanya named in
    //                                                the decomposition (no merge)
    let stdout = &outcome.stdout;

    // Fixture precondition: deno (2 authors) + cargo (1 author), 3 claims total.
    assert_eq!(
        graph.seeded.len(),
        3,
        "fixture precondition: the multi-author example seeds 3 claims (deno x2 + cargo x1); got {}",
        graph.seeded.len()
    );

    // 1. MULTI-AUTHOR breadth line: deno's two distinct authors raise
    //    triangulation — the content-frozen wording the renderer surfaces when
    //    distinct_author_count > 1 (the per-additional-author bonus is visible).
    assert!(
        stdout.contains("multi-author: 2 distinct authors raise triangulation"),
        "cli.graph_query.multi_author_breadth_line: expected the breakdown to state \
         'multi-author: 2 distinct authors raise triangulation' for deno's two-author pairing;\n\
         --- stdout ---\n{stdout}\n--- graph ---\n{graph:?}"
    );

    // 2. The multi-author lift is OBSERVABLE in the ranking: deno (2 authors,
    //    weight ≈ 1.05) ranks ABOVE the single-author comparator cargo (weight
    //    0.55) even though both share the same max confidence (0.55). The bonus
    //    — not raw confidence — is what lifts deno above cargo.
    let deno_pos = stdout
        .find("github:denoland/deno")
        .expect("the weighted view must rank github:denoland/deno");
    let cargo_pos = stdout
        .find("github:rust-lang/cargo")
        .expect("the weighted view must rank github:rust-lang/cargo (single-author comparator)");
    assert!(
        deno_pos < cargo_pos,
        "cli.graph_query.ranking_order: expected deno (2 distinct authors, multi-author bonus) to \
         rank ABOVE the single-author cargo comparator at the SAME max confidence (0.55) — the \
         per-additional-distinct-author bonus is the lift;\n--- stdout ---\n{stdout}\n\
         --- graph ---\n{graph:?}"
    );
    // Both pairings display the SAME max confidence (0.55), so the ranking gap is
    // attributable to the multi-author bonus, not to a confidence difference.
    assert!(
        stdout.contains("max-confidence 0.55"),
        "cli.graph_query.weight_inputs_shown: expected the shared max-confidence 0.55 displayed \
         (so the lift is the multi-author bonus, not a confidence gap);\n--- stdout ---\n{stdout}"
    );

    // 3. Both deno authors remain individually attributed in the decomposition —
    //    the multi-author aggregate NEVER collapses into a faceless consensus
    //    (anti-merging, WD-73 / ADR-022). Each contributing author is nameable.
    assert!(
        stdout.contains("did:plc:tobias-test") && stdout.contains("did:plc:aanya-test"),
        "cli.graph_query.deno_authors_attributed: expected BOTH deno authors (Tobias + Aanya) to \
         remain individually attributed in the weighted breakdown (anti-merging);\n\
         --- stdout ---\n{stdout}"
    );
}

/// GQE-13 (US-GRAPH-003 edge): two authors disagree sharply on the same
/// project+philosophy (0.85 and 0.20). BOTH contribute to the weight per their
/// confidence; the breakdown shows both authors and both confidences. NO claim
/// is averaged-into-oblivion or dropped — the view shows the spread honestly.
/// (US-GRAPH-003 Example 4.)
///
/// @us-graph-003 @real-io @driving_port @j-002 @kpi-graph-2 @anti-merging @edge
#[test]
fn graph_query_weighted_conflicting_claims_both_contribute_nothing_dropped() {
    let env = TestEnv::initialized();
    let object = "org.openlore.philosophy.dependency-pinning";

    // Seed a sharply-disagreeing pair on one project (author A 0.85, author B
    // 0.20). Both must contribute per their confidence; nothing dropped.
    let graph = seed_federated_graph(
        &env,
        FederatedGraphFixture::conflicting_confidences_one_project(),
    );

    let outcome = run_openlore(&env, &["graph", "query", "--object", object, "--weighted"]);
    assert_eq!(
        outcome.status, 0,
        "graph query --object --weighted (conflict) must exit 0;\n--- stdout ---\n{}\n--- stderr ---\n{}",
        outcome.stdout, outcome.stderr
    );

    // The two sharply-disagreeing claims (Rachel 0.85, Tobias 0.20) on ONE deno
    // pairing BOTH contribute per their own confidence — never averaged into a
    // single 0.525 value, never dropped. The breakdown shows BOTH authors AND
    // BOTH confidences (the aggregate decomposes honestly; anti-merging WD-73 /
    // ADR-022 — conflict is surfaced, not smoothed away).
    //
    // Universe (port-exposed observable surface of the `--weighted` view, all
    // asserted against stdout — the CLI driving-port observable):
    //   cli.graph_query.both_authors_attributed   — Rachel + Tobias both named
    //   cli.graph_query.both_confidences_shown     — 0.85 AND 0.2 both visible in
    //                                                the decomposition
    //   cli.graph_query.claim_and_author_counts    — claims: 2 + authors: 2 (the
    //                                                conflicting pair is intact)
    //   cli.graph_query.no_averaged_collapse       — the smoothed-away average
    //                                                (0.525) NEVER appears
    let stdout = &outcome.stdout;

    // Fixture precondition: exactly the two-author conflicting pair on one project.
    assert_eq!(
        graph.seeded.len(),
        2,
        "fixture precondition: the conflicting example seeds 2 claims on one project; got {}",
        graph.seeded.len()
    );

    // 1. Nothing dropped/merged: the pairing keeps BOTH claims by BOTH authors —
    //    claims: 2, authors: 2 (a dropped/averaged claim would shrink either count).
    assert!(
        stdout.contains("claims  : 2") && stdout.contains("authors: 2"),
        "cli.graph_query.claim_and_author_counts: expected the conflicting deno pairing to keep \
         claims: 2 + authors: 2 (neither claim dropped nor collapsed);\n--- stdout ---\n{stdout}\n\
         --- graph ---\n{graph:?}"
    );

    // 2. BOTH authors stay individually attributed in the decomposition — the
    //    conflict is shown, never smoothed into a faceless aggregate (WD-73).
    assert!(
        stdout.contains("did:plc:rachel-test") && stdout.contains("did:plc:tobias-test"),
        "cli.graph_query.both_authors_attributed: expected BOTH conflicting authors (Rachel + \
         Tobias) named in the weighted breakdown;\n--- stdout ---\n{stdout}"
    );

    // 3. BOTH confidences appear VERBATIM in the breakdown — each claim
    //    contributes per its OWN confidence (0.85 and 0.20, rendered as the
    //    minimal decimals 0.85 and 0.2). The high-confidence claim does NOT erase
    //    the low one, nor vice versa.
    assert!(
        stdout.contains("0.85"),
        "cli.graph_query.both_confidences_shown: expected Rachel's confidence 0.85 shown verbatim \
         in the conflicting-claims breakdown;\n--- stdout ---\n{stdout}"
    );
    assert!(
        stdout.contains("0.2"),
        "cli.graph_query.both_confidences_shown: expected Tobias's confidence 0.2 (0.20) shown \
         verbatim in the conflicting-claims breakdown — the low-confidence claim is NOT dropped;\n\
         --- stdout ---\n{stdout}"
    );

    // 4. NO averaged-into-oblivion collapse: the arithmetic mean of the two
    //    conflicting confidences (0.525, displayed 0.52) must NEVER appear — the
    //    view surfaces the spread honestly, never a single smoothed value (the
    //    anti-merging crux: aggregates decompose to per-author contributions per
    //    their own confidence, NOT an averaged consensus; ADR-022 / WD-73).
    assert!(
        !stdout.contains("0.52"),
        "cli.graph_query.no_averaged_collapse: the conflicting pair (0.85, 0.20) must NEVER be \
         collapsed into the arithmetic mean 0.525/0.52 — each claim contributes per its OWN \
         confidence (anti-merging, ADR-022);\n--- stdout ---\n{stdout}"
    );
}

/// GQE-14 (US-GRAPH-003 edge; Gate 4 release-gate): after running a weighted
/// query, NO `adherence_weight` or `weight_bucket` appears in any DuckDB
/// table, any `<cid>.json`, or any record; AND re-running the same query after
/// a `peer pull` (new claims arrived) produces DIFFERENT weights — proving
/// weights are computed at query time, never stored.
/// (US-GRAPH-003 Example 5 + AC "never persisted".)
///
/// @us-graph-003 @real-io @driving_port @j-002 @gate-4 @display-only @release-gate
#[test]
fn graph_query_weighted_outputs_are_never_persisted_and_recompute_at_query_time() {
    let env = TestEnv::initialized();
    let object = "org.openlore.philosophy.dependency-pinning";

    // Seed an initial subgraph, run a weighted query, then pull an ADDITIONAL
    // peer claim and re-run — the seeder returns a handle that can add a claim
    // mid-scenario so the weight observably changes (proving query-time compute).
    let mut graph = seed_federated_graph(
        &env,
        FederatedGraphFixture::dependency_pinning_weighted_worked_example(),
    );

    let first = run_openlore(&env, &["graph", "query", "--object", object, "--weighted"]);
    assert_eq!(
        first.status, 0,
        "first weighted query must exit 0;\n--- stdout ---\n{}\n--- stderr ---\n{}",
        first.stdout, first.stderr
    );

    // Gate 4: no weight/bucket string persisted anywhere after a weighted query.
    // DELIVER materializes `assert_weight_not_persisted` scanning every DuckDB
    // table + every on-disk artifact for the forbidden substrings
    // (adherence_weight / STRONG / MODERATE / SPARSE).
    assert_weight_not_persisted(&env);

    // Pull an additional contributing claim, then re-run — the weight changes.
    graph.add_peer_claim(&env, AddedPeerClaim::deno_third_author());
    let second = run_openlore(&env, &["graph", "query", "--object", object, "--weighted"]);
    assert_eq!(
        second.status, 0,
        "second weighted query (after pull) must exit 0;\n--- stdout ---\n{}\n--- stderr ---\n{}",
        second.stdout, second.stderr
    );

    // Gate 4 (still nothing persisted, even after the recompute): the second
    // weighted query — over the now-larger store — also persists no
    // weight/bucket. The display-only aggregate never leaks into storage no
    // matter how many times it is computed.
    assert_weight_not_persisted(&env);

    // QUERY-TIME-COMPUTE PROOF (US-GRAPH-003 Example 5): the AFFECTED pairing
    // (github:denoland/deno — the third author landed on deno) shows a
    // DIFFERENT adherence weight on the re-run. A persisted/cached weight would
    // be unchanged; a recomputed weight reflects the new contributing claim.
    //
    // Universe (port-exposed observable surface of the recompute proof):
    //   cli.graph_query.weighted.displayed_weight[github:denoland/deno] — the
    //   ranked-view weight for the affected pairing, BEFORE vs AFTER the pull.
    // The renderer prints `<subject>   weight <X.XX>   [<bucket>]`
    // (render.rs `render_weighted_pairing`); extract the deno weight from each
    // stdout and assert it changed.
    let affected = "github:denoland/deno";
    let weight_before = extract_displayed_weight(&first.stdout, affected).unwrap_or_else(|| {
        panic!(
            "expected the FIRST weighted view to display a weight for {affected};\n\
             --- stdout ---\n{}",
            first.stdout
        )
    });
    let weight_after = extract_displayed_weight(&second.stdout, affected).unwrap_or_else(|| {
        panic!(
            "expected the SECOND weighted view (after pull) to display a weight for {affected};\n\
             --- stdout ---\n{}",
            second.stdout
        )
    });
    assert_ne!(
        weight_before, weight_after,
        "cli.graph_query.weighted.displayed_weight[{affected}]: expected the affected pairing's \
         adherence weight to CHANGE after a third contributing claim arrived ({weight_before} → \
         {weight_after}) — proving the weight is RECOMPUTED at query time, not \
         persisted/cached (US-GRAPH-003 Example 5; Gate 4 release-gate);\n\
         --- first ---\n{}\n--- second ---\n{}\n--- graph ---\n{graph:?}",
        first.stdout, second.stdout
    );
}

/// Extract the displayed adherence weight for `subject` from a `--weighted`
/// ranked view. The renderer prints one pairing per line as
/// `  <rank>. <subject>   weight <X.XX>   [<bucket>]`
/// (crates/cli/src/render.rs `render_weighted_pairing`). Returns the parsed
/// `f64` weight for the first line naming `subject`, or `None` if absent.
fn extract_displayed_weight(stdout: &str, subject: &str) -> Option<f64> {
    stdout
        .lines()
        .find(|line| line.contains(subject) && line.contains("weight "))
        .and_then(|line| line.split("weight ").nth(1))
        .and_then(|tail| tail.split_whitespace().next())
        .and_then(|token| token.parse::<f64>().ok())
}

/// GQE-15 (US-GRAPH-003 / I-GRAPH-7): a weighted query succeeds with the
/// network disabled — scoring is local read-only over the seeded store
/// (WD-79/WD-92; extends slice-01 KPI-5 / I-9).
///
/// @us-graph-003 @real-io @driving_port @j-002 @local-first @i-graph-7
#[test]
fn graph_query_weighted_succeeds_with_network_disabled() {
    let env = TestEnv::initialized();
    let object = "org.openlore.philosophy.dependency-pinning";

    let graph = seed_federated_graph(
        &env,
        FederatedGraphFixture::dependency_pinning_weighted_worked_example(),
    );

    let outcome =
        run_openlore_network_disabled(&env, &["graph", "query", "--object", object, "--weighted"]);
    assert_eq!(
        outcome.status, 0,
        "weighted query must succeed with the network disabled (local-first; WD-79/WD-92);\n\
         --- stdout ---\n{}\n--- stderr ---\n{}",
        outcome.stdout, outcome.stderr
    );

    // The `--weighted` view renders its FULL transparent ranking from the LOCAL
    // store alone — the same observable surface as the GQE-10 happy path, just
    // with no reachable network. Asserting the SAME universe as GQE-10
    // (port-exposed stdout slots) proves scoring is genuinely local read-only,
    // not merely "exits 0".
    //
    // Universe (port-exposed observable surface of the network-disabled
    // `--weighted` view, all asserted against stdout — the CLI driving-port
    // observable):
    //   cli.graph_query.ranking_order        — cargo (1.41) > nixpkgs (1.38) > deno (1.05)
    //   cli.graph_query.weight_shown          — each pairing displays its WD-77 weight
    //   cli.graph_query.weight_inputs_shown   — claims + authors + max-confidence + span
    //   cli.graph_query.no_ml_stated          — the output states "no ML" (WD-71)
    //   cli.graph_query.never_stored_footer   — the DISPLAY-ONLY / NOT-stored notice (WD-72)
    //   cli.graph_query.anti_merging          — both deno authors stay individually named
    //   AND pds.create_record.call_count (0 — no outbound call attempted, I-GRAPH-7)
    let stdout = &outcome.stdout;

    // Fixture precondition: exactly the 4-claim / 3-project worked example.
    assert_eq!(
        graph.seeded.len(),
        4,
        "fixture precondition: the weighted worked example seeds 4 claims; got {}",
        graph.seeded.len()
    );

    // 1. RANKING ORDER — the three projects rank by adherence weight descending
    //    (cargo 1.41 > nixpkgs 1.38 > deno 1.05), computed from the LOCAL store
    //    with the network disabled.
    let cargo_pos = stdout
        .find("github:rust-lang/cargo")
        .expect("the network-disabled weighted view must rank github:rust-lang/cargo");
    let nixpkgs_pos = stdout
        .find("github:NixOS/nixpkgs")
        .expect("the network-disabled weighted view must rank github:NixOS/nixpkgs");
    let deno_pos = stdout
        .find("github:denoland/deno")
        .expect("the network-disabled weighted view must rank github:denoland/deno");
    assert!(
        cargo_pos < nixpkgs_pos && nixpkgs_pos < deno_pos,
        "cli.graph_query.ranking_order: expected cargo (1.41) before nixpkgs (1.38) before \
         deno (1.05) ranked by adherence weight descending — from the LOCAL store with the \
         network disabled;\n--- stdout ---\n{stdout}\n--- graph ---\n{graph:?}"
    );

    // 2. Each project's WD-77 weight is displayed (reproducible by hand from the
    //    seeded claims; Gate 2 / KPI-GRAPH-3) — derived locally, no network.
    for (project, weight) in [
        ("github:rust-lang/cargo", "1.41"),
        ("github:NixOS/nixpkgs", "1.38"),
        ("github:denoland/deno", "1.05"),
    ] {
        assert!(
            stdout.contains(&format!("weight {weight}")),
            "cli.graph_query.weight_shown: expected {project} to display the documented adherence \
             weight {weight} from the LOCAL store with the network disabled;\n--- stdout ---\n{stdout}"
        );
    }

    // 3. Each weight is shown WITH its inputs (the transparency contract — a
    //    weight is never a bare number): deno carries 2 claims / 2 authors; the
    //    triangulated cargo + nixpkgs carry 1 claim / 1 author each, with their
    //    max-confidence values, plus Rachel's cross-project span.
    assert!(
        stdout.contains("claims  : 2") && stdout.contains("authors: 2"),
        "cli.graph_query.weight_inputs_shown: expected deno's weight inputs to show \
         claims: 2 + authors: 2 (network disabled);\n--- stdout ---\n{stdout}"
    );
    assert!(
        stdout.contains("claims  : 1") && stdout.contains("authors: 1"),
        "cli.graph_query.weight_inputs_shown: expected the single-author cargo/nixpkgs pairings \
         to show claims: 1 + authors: 1 (network disabled);\n--- stdout ---\n{stdout}"
    );
    for max_conf in ["0.91", "0.88", "0.55"] {
        assert!(
            stdout.contains(&format!("max-confidence {max_conf}")),
            "cli.graph_query.weight_inputs_shown: expected the max-confidence {max_conf} displayed \
             alongside its weight (network disabled);\n--- stdout ---\n{stdout}"
        );
    }
    assert!(
        stdout.contains("spans") && stdout.contains("did:plc:rachel-test"),
        "cli.graph_query.weight_inputs_shown: expected the cross-project span line naming \
         did:plc:rachel-test spanning two projects (network disabled);\n--- stdout ---\n{stdout}"
    );

    // 4. The transparent no-ML formula is printed (WD-71) — the auditable
    //    formula contract survives the network-disabled local read.
    assert!(
        stdout.contains("no ML"),
        "cli.graph_query.no_ml_stated: expected the output to state 'no ML' (WD-71 transparency, \
         network disabled);\n--- stdout ---\n{stdout}"
    );

    // 5. The never-stored footer renders: weights are a DISPLAY-ONLY aggregate
    //    computed at query time, NOT stored/signed/published (WD-72) — the
    //    local-first read carries the same honest framing as the networked path.
    assert!(
        stdout.contains("DISPLAY-ONLY") && stdout.contains("NOT stored"),
        "cli.graph_query.never_stored_footer: expected the footer to state weights are a \
         DISPLAY-ONLY aggregate view, NOT stored (WD-72, network disabled);\n--- stdout ---\n{stdout}"
    );

    // 6. Anti-merging (KPI-GRAPH-2): the ranking never collapses deno's two
    //    authors into a faceless consensus — both stay individually nameable.
    assert!(
        stdout.contains("did:plc:tobias-test") && stdout.contains("did:plc:maria-test"),
        "cli.graph_query.anti_merging: expected deno's two contributing authors (Tobias + Maria) \
         to remain individually attributed in the network-disabled weighted view;\n\
         --- stdout ---\n{stdout}"
    );

    // 7. I-GRAPH-7 local-first: NO outbound PDS call was attempted. The weighted
    //    scoring read is a pure LOCAL read over the seeded DuckDB — the fake PDS
    //    recorded zero create_record calls (port-exposed name
    //    pds.create_record.call_count).
    assert_no_pds_call_was_made(&env);
}

// =============================================================================
// US-GRAPH-005 — audit a weight with --explain (the strongest transparency form)
// =============================================================================

/// GQE-16 (US-GRAPH-005 happy; Gate 1 + Gate 2): Maria runs
/// `--object dependency-pinning --weighted --explain github:denoland/deno`.
/// The breakdown enumerates each contributing claim (Tobias bafy...d3no conf
/// 0.55 author-bonus 1.0; Maria bafy...mz01 conf 0.40 +0.25 second-author
/// bonus), shows each applied bonus, and the running sum (0.55 + 0.50 = 1.05)
/// equals the displayed adherence weight. No contributing claim is merged into
/// a faceless aggregate. Drives gate 1 (`scoring_aggregate_preserves_attribution`)
/// + gate 2 (`weight_equals_formula`, reproduce-by-hand).
///
/// @us-graph-005 @real-io @driving_port @j-002 @kpi-graph-2 @kpi-graph-3 @gate-1 @gate-2 @happy
#[test]
fn graph_query_explain_reproduces_weight_from_per_claim_arithmetic() {
    let env = TestEnv::initialized();
    let object = "org.openlore.philosophy.dependency-pinning";

    // Seed the worked-example deno pairing (Tobias 0.55 + Maria 0.40) whose
    // arithmetic the --explain output must reproduce by hand to 1.05.
    let graph = seed_federated_graph(
        &env,
        FederatedGraphFixture::dependency_pinning_weighted_worked_example(),
    );

    let outcome = run_openlore(
        &env,
        &[
            "graph",
            "query",
            "--object",
            object,
            "--weighted",
            "--explain",
            "github:denoland/deno",
        ],
    );
    assert_eq!(
        outcome.status, 0,
        "graph query --weighted --explain must exit 0;\n--- stdout ---\n{}\n--- stderr ---\n{}",
        outcome.stdout, outcome.stderr
    );

    // The breakdown enumerates each contributing claim with author DID + cid +
    // confidence; each applied bonus is shown; the running sum equals the
    // displayed weight; no claim is merged.
    //
    // Universe (port-exposed observable surface of the `--explain` breakdown, all
    // asserted against stdout — the CLI driving-port observable):
    //   cli.graph_query.contributions_per_author — Tobias + Maria each headed by
    //                                               their OWN DID (Gate 1)
    //   cli.graph_query.per_claim_inputs_shown    — author DID + cid + base
    //                                               confidence + applied bonus
    //   cli.graph_query.running_sum_equals_weight — 0.55 + 0.50 = 1.05 == weight
    //                                               (Gate 2 reproduce-by-hand)
    //   cli.graph_query.no_faceless_aggregate     — no merged/consensus row
    //
    // The deno worked example (data-models.md §"The scoring formula"; constants
    // author_distinct_bonus 0.25, cross_project_triangulation_bonus 0.50):
    //   Tobias  0.55 (first author, x1.0, no triangulation) -> subtotal 0.55
    //   Maria   0.40 (second author, x1.25)                 -> subtotal 0.50
    //   running sum 0.55 + 0.50 = 1.05 == displayed adherence weight 1.05
    let stdout = &outcome.stdout;

    // Fixture precondition: the deno pairing has exactly Tobias 0.55 + Maria 0.40.
    let deno_authors: std::collections::HashSet<&str> = graph
        .seeded
        .iter()
        .filter(|c| c.subject == "github:denoland/deno")
        .map(|c| c.author_did.as_str())
        .collect();
    assert_eq!(
        deno_authors.len(),
        2,
        "fixture precondition: the deno pairing has 2 distinct authors; got {deno_authors:?}"
    );

    // 1. Gate 1 — EACH contributing claim is enumerated under its OWN author DID:
    //    Tobias and Maria each head a contribution block (no faceless aggregate).
    assert!(
        stdout.contains("Contribution: did:plc:tobias-test"),
        "cli.graph_query.contributions_per_author: expected Tobias's contribution headed by \
         his DID;\n--- stdout ---\n{stdout}\n--- graph ---\n{graph:?}"
    );
    assert!(
        stdout.contains("Contribution: did:plc:maria-test"),
        "cli.graph_query.contributions_per_author: expected Maria's contribution headed by \
         her DID;\n--- stdout ---\n{stdout}"
    );

    // 2. Each contribution names its own claim cid (every contribution maps to ONE
    //    signed claim — each row independently attributable; Gate 5 analog). The
    //    cids are seeded dynamically, so assert the cid-bearing line count: exactly
    //    2 contributing claims (one per deno author), none merged or dropped.
    let contribution_cid_rows = stdout
        .lines()
        .filter(|line| line.trim_start().starts_with("cid:"))
        .count();
    assert_eq!(
        contribution_cid_rows, 2,
        "cli.graph_query.per_claim_inputs_shown: expected exactly 2 cid-bearing contribution rows \
         (Tobias + Maria, none merged); got {contribution_cid_rows};\n--- stdout ---\n{stdout}"
    );

    // 3. Each contribution shows its base confidence VERBATIM (Gate 6 — no
    //    bucket-rounding). 0.40 renders as the minimal decimal `0.4`.
    assert!(
        stdout.contains("confidence: 0.55 (base)") && stdout.contains("confidence: 0.4 (base)"),
        "cli.graph_query.per_claim_inputs_shown: expected each contribution to show its base \
         confidence verbatim (Tobias 0.55, Maria 0.4);\n--- stdout ---\n{stdout}"
    );

    // 4. Gate 2 — each applied bonus is shown on its own line. Maria's
    //    second-author multiplier share (x1.25) is the +0.25 per-add'l-author
    //    bonus; Tobias (first author) carries x1.0.
    assert!(
        stdout.contains("author-distinct bonus: x1.25"),
        "cli.graph_query.per_claim_inputs_shown: expected Maria's second-author multiplier \
         share (x1.25 — the +0.25 bonus) on its own line;\n--- stdout ---\n{stdout}"
    );

    // 5. Gate 2 — the running sum EQUALS the displayed adherence weight: the
    //    per-claim subtotals (0.55 + 0.50) sum to 1.05, reproduced by hand.
    assert!(
        stdout.contains("subtotal:   0.55") && stdout.contains("subtotal:   0.50"),
        "cli.graph_query.per_claim_inputs_shown: expected the per-claim subtotals (0.55, 0.50) \
         to be shown;\n--- stdout ---\n{stdout}"
    );
    assert!(
        stdout.contains("Running sum 1.05 = displayed adherence weight 1.05"),
        "cli.graph_query.running_sum_equals_weight: expected the running sum (0.55 + 0.50 = 1.05) \
         to EQUAL the displayed adherence weight 1.05 (reproduce-by-hand; Gate 2);\n\
         --- stdout ---\n{stdout}"
    );

    // 6. Gate 1 — no contributing claim is merged into a faceless aggregate: both
    //    authors stay individually nameable, and no merged/consensus/aggregate row
    //    appears.
    for label in ["merged", "consensus", "aggregate"] {
        assert!(
            !stdout.to_lowercase().contains(label),
            "cli.graph_query.no_faceless_aggregate (Gate 1): the --explain breakdown must contain \
             NO {label:?} row — each contributing claim stays attributed;\n--- stdout ---\n{stdout}"
        );
    }
}

/// GQE-17 (US-GRAPH-005 edge; Gate 3): Tobias runs `--object actor-model
/// --weighted --explain github:tokio-rs/tokio`. The breakdown shows the ONE
/// contributing claim (1 author, conf 0.50, no bonuses) and the running sum
/// 0.50, with the [SPARSE] honesty line "based on 1 claim by 1 author"
/// repeated. (US-GRAPH-005 Example 2.)
///
/// @us-graph-005 @real-io @driving_port @j-002 @gate-3 @sparse @edge
#[test]
fn graph_query_explain_on_sparse_subject_repeats_the_honesty_line() {
    let env = TestEnv::initialized();
    let object = "org.openlore.philosophy.actor-model";

    let graph = seed_federated_graph(
        &env,
        FederatedGraphFixture::actor_model_single_sparse_claim(),
    );

    let outcome = run_openlore(
        &env,
        &[
            "graph",
            "query",
            "--object",
            object,
            "--weighted",
            "--explain",
            "github:tokio-rs/tokio",
        ],
    );
    assert_eq!(
        outcome.status, 0,
        "graph query --weighted --explain (sparse) must exit 0;\n--- stdout ---\n{}\n--- stderr ---\n{}",
        outcome.stdout, outcome.stderr
    );

    // The breakdown shows the ONE contributing claim (1 author, conf 0.50, no
    // bonuses, running sum 0.50) AND repeats the [SPARSE] honesty line so a thin
    // single-opinion audit never reads as a settled verdict (WD-74 / Gate 3).
    //
    // Universe (port-exposed observable surface of the sparse `--explain`
    // breakdown, all asserted against stdout — the CLI driving-port observable):
    //   cli.graph_query.single_contribution_shown — the lone tokio claim is
    //                                                enumerated under its author DID
    //   cli.graph_query.no_bonuses_applied         — author-distinct x1.0, NO
    //                                                cross-project triangulation line
    //   cli.graph_query.running_sum_equals_weight  — 0.50 == displayed weight 0.50
    //   cli.graph_query.sparse_honesty_line_repeated — the verbatim 05-02 honesty
    //                                                line + lead-not-conclusion advice
    let stdout = &outcome.stdout;

    // Fixture precondition: exactly the 1-claim single-project sparse subgraph
    // (the local user's own actor-model claim on tokio at confidence 0.50).
    assert_eq!(
        graph.seeded.len(),
        1,
        "fixture precondition: the sparse fixture seeds exactly 1 claim; got {}",
        graph.seeded.len()
    );

    // 1. The single contributing claim is enumerated under its OWN author DID
    //    (the local user). Exactly ONE contribution block — no co-author to merge.
    let contribution_rows = stdout
        .lines()
        .filter(|line| line.trim_start().starts_with("Contribution:"))
        .count();
    assert_eq!(
        contribution_rows, 1,
        "cli.graph_query.single_contribution_shown: expected exactly 1 contribution block (the \
         lone sparse tokio claim, no co-author); got {contribution_rows};\n--- stdout ---\n{stdout}\n\
         --- graph ---\n{graph:?}"
    );

    // 2. The contribution shows its base confidence VERBATIM (0.50 -> minimal
    //    decimal 0.5; KPI-4 zero-normalization), with the no-bonus case made
    //    EXPLICIT: author-distinct multiplier x1.0 (first/only author).
    assert!(
        stdout.contains("confidence: 0.5 (base)"),
        "cli.graph_query.single_contribution_shown: expected the lone claim's base confidence 0.5 \
         shown verbatim;\n--- stdout ---\n{stdout}"
    );
    assert!(
        stdout.contains("author-distinct bonus: x1.0"),
        "cli.graph_query.no_bonuses_applied: expected the first/only author's distinct-bonus share \
         x1.0 (no second author);\n--- stdout ---\n{stdout}"
    );

    // 3. NO cross-project triangulation bonus applied (a lone author on a lone
    //    project triangulates with nothing) — the bonus line is ABSENT.
    assert!(
        !stdout.contains("cross-project triangulation"),
        "cli.graph_query.no_bonuses_applied: a single-claim single-project pairing must apply NO \
         cross-project triangulation bonus;\n--- stdout ---\n{stdout}"
    );

    // 4. The running sum (the lone subtotal 0.50) EQUALS the displayed adherence
    //    weight 0.50 — reproduce-by-hand holds even for the sparse case (Gate 2).
    assert!(
        stdout.contains("subtotal:   0.50"),
        "cli.graph_query.running_sum_equals_weight: expected the lone claim's subtotal 0.50;\n\
         --- stdout ---\n{stdout}"
    );
    assert!(
        stdout.contains("Running sum 0.50 = displayed adherence weight 0.50"),
        "cli.graph_query.running_sum_equals_weight: expected the running sum (0.50) to EQUAL the \
         displayed adherence weight 0.50 (reproduce-by-hand; Gate 2);\n--- stdout ---\n{stdout}"
    );

    // 5. The [SPARSE] honesty line is REPEATED in the per-claim audit, using the
    //    SAME verbatim wording as the non-explain sparse view from 05-02 (WD-74,
    //    Gate 3): "based on 1 claim by 1 author" + the lead-not-conclusion advice.
    //    A single high-confidence opinion is NEVER presented as a settled verdict.
    assert!(
        stdout.contains("based on 1 claim by 1 author"),
        "cli.graph_query.sparse_honesty_line_repeated: expected the verbatim 05-02 honesty line \
         'based on 1 claim by 1 author' repeated in the --explain audit (WD-74; Gate 3);\n\
         --- stdout ---\n{stdout}"
    );
    assert!(
        stdout.contains("treat as a lead, not a defensible conclusion"),
        "cli.graph_query.sparse_honesty_line_repeated: expected the lead-not-conclusion advice \
         'treat as a lead, not a defensible conclusion' repeated in the --explain audit (WD-74);\n\
         --- stdout ---\n{stdout}"
    );
}

/// GQE-18 (US-GRAPH-005 error): Aanya runs `--object dependency-pinning
/// --weighted --explain github:foo/bar` where foo/bar has no dependency-pinning
/// claims. The CLI prints "Subject github:foo/bar is not in this result set."
/// and exits NON-ZERO (a usage error — distinct from an empty dimension query
/// which exits 0). (US-GRAPH-005 Example 3 + UAT scenario 3.)
///
/// @us-graph-005 @real-io @driving_port @j-002 @error
#[test]
fn graph_query_explain_for_subject_absent_from_result_set_is_a_usage_error() {
    let env = TestEnv::initialized();
    let object = "org.openlore.philosophy.dependency-pinning";

    // Seed the dependency-pinning subgraph WITHOUT any github:foo/bar claim, so
    // the explained subject is genuinely absent from the result set.
    let graph = seed_federated_graph(
        &env,
        FederatedGraphFixture::dependency_pinning_weighted_worked_example(),
    );

    let outcome = run_openlore(
        &env,
        &[
            "graph",
            "query",
            "--object",
            object,
            "--weighted",
            "--explain",
            "github:foo/bar",
        ],
    );

    // --explain for an absent subject is a USAGE ERROR: non-zero exit (distinct
    // from an empty dimension query which exits 0).
    assert_ne!(
        outcome.status, 0,
        "graph query --explain for a subject absent from the result set must exit non-zero \
         (usage error, NOT exit 0);\n--- stdout ---\n{}\n--- stderr ---\n{}",
        outcome.stdout, outcome.stderr
    );

    // The error names the ABSENT subject so the operator knows which lookup
    // missed. This is a USAGE ERROR (the non-zero exit asserted above), distinct
    // from an empty dimension query's exit 0 (architecture-design §5.2 invariant
    // 5). The dispatcher surfaces the bail message on stderr.
    //
    // Universe (port-exposed observable surface of the absent-subject error):
    //   cli.graph_query.exit_code         — non-zero (asserted above)
    //   cli.graph_query.absent_subject_msg — the exact content-frozen message on
    //                                        stderr naming github:foo/bar
    assert!(
        outcome
            .stderr
            .contains("Subject github:foo/bar is not in this result set."),
        "cli.graph_query.absent_subject_msg: expected the exact message 'Subject github:foo/bar is \
         not in this result set.' (naming the absent subject) on stderr;\n--- stdout ---\n{}\n\
         --- stderr ---\n{}\n--- graph ---\n{graph:?}",
        outcome.stdout, outcome.stderr
    );
}

/// GQE-19 (US-GRAPH-005 edge; Gate 1): Maria runs `--explain
/// github:rust-lang/cargo` where Rachel's cross-project span (cargo+nixpkgs)
/// raised cargo's weight. The breakdown shows the base claim PLUS the explicit
/// "+0.5 cross-project triangulation" line attributed to did:plc:rachel-test,
/// so Maria sees exactly why the triangulation bonus applied and to whom. The
/// running sum equals cargo's displayed weight. (US-GRAPH-005 Example 4.)
///
/// @us-graph-005 @real-io @driving_port @j-002 @kpi-graph-2 @kpi-graph-3 @gate-1 @edge
#[test]
fn graph_query_explain_attributes_triangulation_bonus_to_the_contributor_who_earned_it() {
    let env = TestEnv::initialized();
    let object = "org.openlore.philosophy.dependency-pinning";

    // Seed the worked-example graph where Rachel asserts dependency-pinning on
    // BOTH cargo and nixpkgs (the cross-project span that earns the +0.5
    // triangulation bonus on cargo). data-models.md §"Worked example (cargo)".
    let graph = seed_federated_graph(
        &env,
        FederatedGraphFixture::dependency_pinning_weighted_worked_example(),
    );

    let outcome = run_openlore(
        &env,
        &[
            "graph",
            "query",
            "--object",
            object,
            "--weighted",
            "--explain",
            "github:rust-lang/cargo",
        ],
    );
    assert_eq!(
        outcome.status, 0,
        "graph query --weighted --explain cargo must exit 0;\n--- stdout ---\n{}\n--- stderr ---\n{}",
        outcome.stdout, outcome.stderr
    );

    // Gate 1 (aggregate preserves attribution): cargo's weight was raised by
    // Rachel's cross-project span (cargo+nixpkgs). The --explain breakdown must
    // show the base claim PLUS the explicit "+0.5 cross-project triangulation"
    // line attributed to the contributor who earned it (did:plc:rachel-test),
    // NOT a faceless aggregate. The running sum equals cargo's displayed weight.
    //
    // The cargo arithmetic (data-models.md §"Worked example (cargo)"; constant
    // cross_project_triangulation_bonus 0.50): Rachel is the sole cargo author
    // (first author, x1.0) AND spans two projects (cargo+nixpkgs), so her
    // contribution carries the +0.5 triangulation addend:
    //   Rachel  0.91 (x1.0) + 0.50 triangulation -> subtotal 1.41
    //   running sum 1.41 == displayed adherence weight 1.41
    //
    // Universe (port-exposed observable surface of cargo's `--explain` breakdown,
    // all asserted against stdout — the CLI driving-port observable):
    //   cli.graph_query.bonus_attributed_to_earner — the "+0.5 cross-project
    //                                                 triangulation" line appears
    //                                                 under Rachel's contribution
    //                                                 (the DID that earned it)
    //   cli.graph_query.base_claim_shown           — Rachel's base confidence 0.91
    //   cli.graph_query.running_sum_equals_weight  — 0.91 + 0.50 = 1.41 == weight
    //   cli.graph_query.no_faceless_aggregate      — no merged/consensus row
    let stdout = &outcome.stdout;

    // Fixture precondition (the cross-project span that EARNS the bonus): Rachel
    // asserts dependency-pinning on BOTH cargo and nixpkgs.
    let rachel_subjects: std::collections::HashSet<&str> = graph
        .seeded
        .iter()
        .filter(|c| c.author_did == "did:plc:rachel-test")
        .map(|c| c.subject.as_str())
        .collect();
    assert!(
        rachel_subjects.contains("github:rust-lang/cargo")
            && rachel_subjects.contains("github:NixOS/nixpkgs"),
        "fixture precondition: Rachel must span cargo+nixpkgs (the >= 2-subject reach that earns \
         the +0.5 triangulation bonus on cargo); got {rachel_subjects:?}"
    );

    // 1. Gate 1 — the base claim is enumerated under its OWN author DID (Rachel),
    //    showing her base confidence verbatim (0.91; KPI-4 zero-normalization).
    assert!(
        stdout.contains("Contribution: did:plc:rachel-test"),
        "cli.graph_query.base_claim_shown: expected cargo's contribution headed by Rachel's DID;\n\
         --- stdout ---\n{stdout}\n--- graph ---\n{graph:?}"
    );
    assert!(
        stdout.contains("confidence: 0.91 (base)"),
        "cli.graph_query.base_claim_shown: expected Rachel's base confidence 0.91 shown verbatim;\n\
         --- stdout ---\n{stdout}"
    );

    // 2. Gate 1 (attribution-of-bonus) — the +0.5 cross-project triangulation
    //    line appears, AND it is attributed to the contributor who earned it
    //    (Rachel). Maria sees exactly WHY the bonus applied and TO WHOM: the
    //    addend line falls within Rachel's contribution block, so the breakdown
    //    never reads as a faceless aggregate bonus.
    assert!(
        stdout.contains("+0.5 cross-project triangulation"),
        "cli.graph_query.bonus_attributed_to_earner: expected the explicit '+0.5 cross-project \
         triangulation' line in cargo's breakdown;\n--- stdout ---\n{stdout}"
    );
    let rachel_block_carries_bonus = stdout
        .split("Contribution: ")
        .any(|block| {
            block.starts_with("did:plc:rachel-test")
                && block.contains("+0.5 cross-project triangulation")
        });
    assert!(
        rachel_block_carries_bonus,
        "cli.graph_query.bonus_attributed_to_earner (Gate 1): the +0.5 cross-project triangulation \
         addend must fall within RACHEL's contribution block (the contributor who earned it by \
         spanning cargo+nixpkgs), so Maria sees who + why — not a faceless aggregate bonus;\n\
         --- stdout ---\n{stdout}"
    );

    // 3. Gate 1 — the running sum EQUALS cargo's displayed weight: Rachel's base
    //    (0.91) plus the +0.5 triangulation reproduces 1.41 by hand.
    assert!(
        stdout.contains("Running sum 1.41 = displayed adherence weight 1.41"),
        "cli.graph_query.running_sum_equals_weight: expected the running sum (0.91 + 0.50 = 1.41) \
         to EQUAL cargo's displayed adherence weight 1.41 (the triangulation bonus folded in);\n\
         --- stdout ---\n{stdout}"
    );

    // 4. Gate 1 — the contributor stays individually nameable: no merged/
    //    consensus/aggregate row swallows the bonus.
    for label in ["merged", "consensus", "aggregate"] {
        assert!(
            !stdout.to_lowercase().contains(label),
            "cli.graph_query.no_faceless_aggregate (Gate 1): cargo's --explain breakdown must \
             contain NO {label:?} row — the triangulation bonus stays attributed to Rachel;\n\
             --- stdout ---\n{stdout}"
        );
    }
}

// =============================================================================
// US-GRAPH-004 — traverse contributor<->project<->philosophy edges
// =============================================================================

/// GQE-20 (US-GRAPH-004 happy; KPI-GRAPH-1 north star; Gate 5): Maria runs
/// `--object dependency-pinning --traverse`. The tree shows the philosophy ->
/// {cargo, nixpkgs, deno} -> their claim authors. A "Connections found" callout
/// reads "did:plc:rachel-test spans 2 of these projects (cargo, nixpkgs) -> a
/// contributor whose dependency-pinning claims triangulate across projects."
/// Every displayed edge maps to exactly one signed claim; the output states
/// "Traversal does not invent edges." This is the non-obvious connection
/// (KPI-GRAPH-1) + gate 5 (`traversal_invents_no_edges`).
///
/// @us-graph-004 @real-io @driving_port @walking_skeleton @j-002 @kpi-graph-1 @gate-5 @happy
#[test]
fn graph_query_traverse_surfaces_a_non_obvious_cross_project_contributor_connection() {
    let env = TestEnv::initialized();
    let object = "org.openlore.philosophy.dependency-pinning";

    // Seed the cross-project span: Rachel asserts dependency-pinning on BOTH
    // cargo and nixpkgs (the connection the traversal must surface). The seeder
    // hosts the US-GRAPH-004 Example 1 fixture.
    let graph = seed_federated_graph(
        &env,
        FederatedGraphFixture::dependency_pinning_rachel_spans_two_projects(),
    );

    let outcome = run_openlore(&env, &["graph", "query", "--object", object, "--traverse"]);
    assert_eq!(
        outcome.status, 0,
        "graph query --object --traverse must exit 0;\n--- stdout ---\n{}\n--- stderr ---\n{}",
        outcome.stdout, outcome.stderr
    );

    // 1. A tree from the philosophy to its projects to their claim authors.
    // 2. A "Connections found" callout names Rachel as spanning 2 projects.
    // 3. Every displayed edge maps to exactly one signed claim (Gate 5).
    // 4. The output states "Traversal does not invent edges."
    //
    // Universe (port-exposed observable surface of the `--traverse` view):
    // cli.graph_query.tree_roots_at_philosophy (the queried object heads the
    // tree), cli.graph_query.projects_in_tree (cargo, nixpkgs, deno),
    // cli.graph_query.connection_callout (names did:plc:rachel-test spanning 2
    // projects cargo+nixpkgs), cli.graph_query.edge_cid_rows (one cid per
    // displayed edge — every edge maps to exactly one seeded signed claim, none
    // fabricated), cli.graph_query.invents_no_edges_notice (the content-frozen
    // Gate-5 line). Asserted against stdout (the CLI driving-port observable).
    let stdout = &outcome.stdout;

    // 1. The tree roots at the queried philosophy and fans to its 3 projects.
    assert!(
        stdout.contains(&format!("philosophy: {object}")),
        "expected the traversal tree to root at the queried philosophy {object};\n\
         --- stdout ---\n{stdout}\n--- graph ---\n{graph:?}"
    );
    let projects = [
        "github:rust-lang/cargo",
        "github:NixOS/nixpkgs",
        "github:denoland/deno",
    ];
    let projects_in_tree = projects
        .iter()
        .filter(|p| stdout.contains(&format!("project: {p}")))
        .count();
    assert_eq!(
        projects_in_tree, 3,
        "cli.graph_query.projects_in_tree: expected all 3 seeded projects to appear in the tree; \
         got {projects_in_tree};\n--- stdout ---\n{stdout}"
    );

    // 2. The "Connections found" callout names Rachel spanning EXACTLY the two
    //    projects she claims dependency-pinning on (cargo + nixpkgs) — the
    //    non-obvious cross-project contributor connection (KPI-GRAPH-1).
    assert!(
        stdout.contains("Connections found"),
        "expected a 'Connections found' callout surfacing the cross-project connection;\n\
         --- stdout ---\n{stdout}"
    );
    assert!(
        stdout.contains("did:plc:rachel-test spans 2 of these projects (github:rust-lang/cargo, github:NixOS/nixpkgs)"),
        "expected the callout to name did:plc:rachel-test spanning 2 projects \
         (github:rust-lang/cargo, github:NixOS/nixpkgs) — the contributor whose \
         dependency-pinning claims triangulate across projects (KPI-GRAPH-1);\n\
         --- stdout ---\n{stdout}"
    );

    // 3. Every displayed edge maps to exactly ONE signed claim (Gate 5): each
    //    seeded claim's cid appears on its edge, and the edge-cid count equals
    //    the seeded-claim count (no edge is fabricated, none merged). Each edge
    //    also carries its backing claim's author DID (anti-merging WD-73).
    for claim in &graph.seeded {
        assert!(
            stdout.contains(&format!("author_did: {}", claim.author_did)),
            "expected every traversal edge to carry its backing claim's author DID {} \
             (anti-merging WD-73);\n--- stdout ---\n{stdout}",
            claim.author_did
        );
    }
    let edge_cid_rows = stdout
        .lines()
        .filter(|line| line.trim_start().starts_with("claim_cid:"))
        .count();
    assert_eq!(
        edge_cid_rows,
        graph.seeded.len(),
        "Gate 5 (traversal invents no edges): expected exactly {} claim_cid-bearing edge rows \
         (one per seeded signed claim, none fabricated/merged); got {edge_cid_rows};\n\
         --- stdout ---\n{stdout}",
        graph.seeded.len()
    );

    // 4. The output states the content-frozen Gate-5 honesty notice verbatim.
    assert!(
        stdout.contains("Traversal does not invent edges."),
        "expected the output to carry the content-frozen 'Traversal does not invent edges.' \
         notice (Gate 5);\n--- stdout ---\n{stdout}"
    );
}

/// GQE-21 (US-GRAPH-004 edge; Gate 5): Tobias runs `--object actor-model
/// --traverse` where only tokio has a single claim. The output renders tokio
/// under the philosophy with "no connecting edges found at depth 2." It does
/// NOT fabricate a connection to any other project or contributor.
/// (US-GRAPH-004 Example 2.)
///
/// @us-graph-004 @real-io @driving_port @j-002 @gate-5 @edge
#[test]
fn graph_query_traverse_single_node_no_edges_renders_without_fabrication() {
    let env = TestEnv::initialized();
    let object = "org.openlore.philosophy.actor-model";

    // Seed a single isolated claim (tokio, 1 author, no cross-project span).
    let graph = seed_federated_graph(
        &env,
        FederatedGraphFixture::actor_model_single_sparse_claim(),
    );

    let outcome = run_openlore(&env, &["graph", "query", "--object", object, "--traverse"]);
    assert_eq!(
        outcome.status, 0,
        "graph query --object --traverse (single node) must exit 0;\n--- stdout ---\n{}\n--- stderr ---\n{}",
        outcome.stdout, outcome.stderr
    );

    // The isolated tokio node renders UNDER the philosophy, but the walk finds NO
    // connecting (cross-project) edges: a single author on a single project
    // triangulates with nothing. The renderer must say so HONESTLY ("no
    // connecting edges found at depth 2") and fabricate NO connection to any
    // other project or contributor (Gate 5 / I-GRAPH-5). The content-frozen
    // "Traversal does not invent edges." notice still frames the (sparse) result.
    //
    // Universe (port-exposed observable surface of the single-node `--traverse`
    // view): cli.graph_query.tree_roots_at_philosophy (the queried object heads
    // the tree), cli.graph_query.node_rendered (the single tokio project node is
    // present), cli.graph_query.no_connecting_edges_notice (the content-frozen
    // "no connecting edges found at depth 2" line — the depth searched is named),
    // cli.graph_query.connections_callout_absent (NO "Connections found" callout
    // — nothing to triangulate), cli.graph_query.no_other_project_fabricated (no
    // OTHER project/contributor invented), cli.graph_query.invents_no_edges_notice
    // (the Gate-5 honesty notice still present). Asserted against stdout (the CLI
    // driving-port observable).
    let stdout = &outcome.stdout;

    // 1. The tree roots at the queried philosophy (the seed is honestly rendered).
    assert!(
        stdout.contains(&format!("philosophy: {object}")),
        "expected the traversal tree to root at the queried philosophy {object};\n\
         --- stdout ---\n{stdout}\n--- graph ---\n{graph:?}"
    );

    // 2. The single isolated tokio node IS rendered (the node itself is honest —
    //    the sparse result shows what exists, it does not blank the seed).
    assert!(
        stdout.contains("github:tokio-rs/tokio"),
        "expected the single isolated tokio node to be rendered under the philosophy \
         (the node itself is honest);\n--- stdout ---\n{stdout}"
    );

    // 3. The content-frozen no-connecting-edges line is present and NAMES the
    //    depth searched (default depth 2). Honest "nothing found, nothing
    //    fabricated" — US-GRAPH-004 Example 2.
    assert!(
        stdout.contains("No connecting edges found at depth 2."),
        "expected the content-frozen 'No connecting edges found at depth 2.' line for the single \
         isolated node (no cross-project span to surface);\n--- stdout ---\n{stdout}"
    );

    // 4. NO connection is fabricated: there is no "Connections found" callout
    //    (nothing triangulates), and NO OTHER project/contributor is invented —
    //    only tokio and its single seeded author appear (Gate 5 / I-GRAPH-5).
    assert!(
        !stdout.contains("Connections found"),
        "the single-node traversal must NOT emit a 'Connections found' callout — a lone author on \
         a lone project triangulates with nothing (no fabrication, Gate 5);\n--- stdout ---\n{stdout}"
    );
    for other_project in [
        "github:rust-lang/cargo",
        "github:NixOS/nixpkgs",
        "github:denoland/deno",
    ] {
        assert!(
            !stdout.contains(other_project),
            "the single-node traversal must NOT fabricate a connection to any OTHER project \
             ({other_project}) — only the seeded tokio node exists (Gate 5);\n--- stdout ---\n{stdout}"
        );
    }

    // 5. The content-frozen Gate-5 honesty notice still frames the sparse result.
    assert!(
        stdout.contains("Traversal does not invent edges."),
        "expected the content-frozen 'Traversal does not invent edges.' notice to frame the \
         single-node (sparse) result (Gate 5);\n--- stdout ---\n{stdout}"
    );
}

/// GQE-22 (US-GRAPH-004 edge; WD-76/WD-91): Aanya runs `--contributor
/// did:plc:rachel-test --traverse` on a dense graph where Rachel's claims fan
/// out beyond depth 2. The output is bounded to depth 2 by default, prints
/// "Showing depth 2; N edges omitted. Use `--depth 3` to go deeper.", and
/// returns responsively. (US-GRAPH-004 Example 3.)
///
/// @us-graph-004 @real-io @driving_port @j-002 @wd-76 @bounded @edge
#[test]
fn graph_query_traverse_is_bounded_to_default_depth_two_and_reports_omitted_edges() {
    let env = TestEnv::initialized();
    let rachel_did = "did:plc:rachel-test";

    // Seed a DENSE graph where Rachel's claims fan out beyond depth 2 (many
    // philosophies + co-claimants), so the default bound omits edges.
    let graph = seed_federated_graph(
        &env,
        FederatedGraphFixture::dense_fan_out_beyond_depth_two(),
    );

    let outcome = run_openlore(
        &env,
        &["graph", "query", "--contributor", rachel_did, "--traverse"],
    );
    assert_eq!(
        outcome.status, 0,
        "graph query --contributor --traverse (dense) must exit 0;\n--- stdout ---\n{}\n--- stderr ---\n{}",
        outcome.stdout, outcome.stderr
    );

    // The traversal is bounded to depth 2 by default and reports the omitted
    // count + how to go deeper (WD-76). The dense fixture fans Rachel's claims
    // beyond depth 2 (four distinct authors on one shared project), so the
    // default depth-2 bound MUST cut deeper edges and report them honestly.
    //
    // Universe (port-exposed observable surface of the bounded `--contributor
    // --traverse` view): cli.graph_query.max_depth_shown (the report names the
    // default depth 2), cli.graph_query.omitted_edge_count_reported (a positive
    // count of edges omitted beyond the bound, NOT zero — the bound is honest),
    // cli.graph_query.deeper_hint_present (the report names `--depth 3` as the
    // way to go deeper). Asserted against stdout (the CLI driving-port observable).
    let stdout = &outcome.stdout;

    // 1. Bounded to the WD-76 default depth 2 and reports a NON-ZERO omitted
    //    count + how to widen. The report is content-frozen
    //    ("Showing depth 2; N edge(s) omitted. Use `--depth 3` to go deeper.");
    //    the omitted count is parsed from the rendered line and must be > 0 (the
    //    dense graph genuinely has edges beyond depth 2 — an honest bound, not a
    //    fabricated "0 omitted").
    let report_line = stdout
        .lines()
        .find(|line| line.contains("Showing depth 2;") && line.contains("edge(s) omitted."))
        .unwrap_or_else(|| {
            panic!(
                "expected a bounded-depth report naming the default depth 2 + an omitted-edge \
                 count for the dense fan-out traversal (WD-76);\n--- stdout ---\n{stdout}\n\
                 --- graph ---\n{graph:?}"
            )
        });

    // The omitted-edge count is the integer between "Showing depth 2; " and
    // " edge(s) omitted." — parse it and assert it is strictly positive (the
    // dense graph fans out beyond depth 2, so the default bound MUST omit edges).
    let omitted_count: u32 = report_line
        .split("Showing depth 2; ")
        .nth(1)
        .and_then(|rest| rest.split(" edge(s) omitted.").next())
        .and_then(|n| n.trim().parse().ok())
        .unwrap_or_else(|| {
            panic!(
                "expected the bounded-depth report to carry a parseable omitted-edge count;\n\
                 --- report line ---\n{report_line}\n--- stdout ---\n{stdout}"
            )
        });
    assert!(
        omitted_count > 0,
        "cli.graph_query.omitted_edge_count_reported: the dense fan-out traversal must omit a \
         POSITIVE number of edges beyond the default depth-2 bound (honest bound, not a \
         fabricated 0); got omitted_count={omitted_count};\n--- stdout ---\n{stdout}"
    );

    // 2. The report names the way to go deeper: `--depth 3` raises the bound by
    //    one past the default depth 2 (WD-76 — the operator can opt into more).
    assert!(
        report_line.contains("Use `--depth 3` to go deeper."),
        "cli.graph_query.deeper_hint_present: the bounded-depth report must name `--depth 3` as \
         the way to widen the default depth-2 bound;\n--- report line ---\n{report_line}\n\
         --- stdout ---\n{stdout}"
    );

    // 3. The bound is HONEST about its depth: NO edge is reported beyond depth 2.
    //    The report never claims a deeper depth was shown (the only depth named
    //    in a "Showing depth N" line is the default 2 — the bound was respected).
    for deeper in ["Showing depth 3;", "Showing depth 4;", "Showing depth 5;"] {
        assert!(
            !stdout.contains(deeper),
            "the default-bounded traversal must NOT report a depth deeper than 2 ({deeper}) — the \
             WD-76 default bound caps the walk at depth 2;\n--- stdout ---\n{stdout}"
        );
    }
}

/// GQE-23 (US-GRAPH-004 edge; WD-76): Aanya re-runs the dense traversal with
/// `--depth 3` and sees the previously-omitted depth-3 edges. The depth
/// override widens the bound; the deeper edges are real signed claims, not
/// fabrications. (US-GRAPH-004 Example 3 — the --depth override leg.)
///
/// @us-graph-004 @real-io @driving_port @j-002 @wd-76 @gate-5 @edge
#[test]
fn graph_query_traverse_depth_override_reveals_previously_omitted_real_edges() {
    let env = TestEnv::initialized();
    let rachel_did = "did:plc:rachel-test";

    let graph = seed_federated_graph(
        &env,
        FederatedGraphFixture::dense_fan_out_beyond_depth_two(),
    );

    // Depth 2 (default) omits some edges; depth 3 reveals them. Both are real
    // signed claims (Gate 5).
    let depth_two = run_openlore(
        &env,
        &["graph", "query", "--contributor", rachel_did, "--traverse"],
    );
    assert_eq!(
        depth_two.status, 0,
        "default-depth traversal must exit 0;\n--- stdout ---\n{}\n--- stderr ---\n{}",
        depth_two.stdout, depth_two.stderr
    );
    let depth_three = run_openlore(
        &env,
        &[
            "graph",
            "query",
            "--contributor",
            rachel_did,
            "--traverse",
            "--depth",
            "3",
        ],
    );
    assert_eq!(
        depth_three.status, 0,
        "--depth 3 traversal must exit 0;\n--- stdout ---\n{}\n--- stderr ---\n{}",
        depth_three.stdout, depth_three.stderr
    );

    // `--depth 3` widens the WD-76 bound by one: the depth-3 edges the default
    // depth-2 bound CUT-AND-REPORTED-AS-OMITTED now render in the tree, and every
    // edge it surfaces is a REAL signed claim (Gate 5 — the override reveals what
    // already existed in the store, it fabricates nothing). The dense fixture
    // seeds four distinct authors (Rachel + Tobias + Maria + Aanya) all asserting
    // dependency-pinning on the SAME shared subject (github:rust-lang/cargo), so
    // the contributor-seeded walk steps within that subject: depth 2 shows
    // Rachel's seed edge + the first co-claimant hops; depth 3 the deeper paths
    // the default bound omitted and reported via the WD-76 "N edge(s) omitted"
    // line.
    //
    // Universe (port-exposed observable surface of the depth-2 vs depth-3
    // `--contributor --traverse` views): cli.graph_query.traverse.edge_rows (the
    // count of rendered `claim_cid:` edge rows — depth-3 renders strictly MORE
    // than depth-2, the previously-omitted edges now visible),
    // cli.graph_query.omitted_edge_count_reported (depth-2's POSITIVE WD-76
    // omitted count is exactly the number of additional edge rows the depth-3
    // override reveals — `depth_three_rows == depth_two_rows + depth_two_omitted`),
    // cli.graph_query.traverse.edge_cid_resolvable[cid] (every cid the depth-3
    // view renders resolves via `graph query --subject github:rust-lang/cargo` —
    // a real signed claim, Gate 5), cli.graph_query.max_depth_shown (the depth-3
    // report names depth 3, never the superseded default depth 2). Asserted
    // against the two CLI driving-port stdout observables.
    let two_out = &depth_two.stdout;
    let three_out = &depth_three.stdout;
    let shared_subject = "github:rust-lang/cargo";

    // The rendered edge rows of a traversal (the `claim_cid:` lines — every
    // displayed edge maps to exactly one signed claim, Gate 5). Returned in render
    // order; the count is the load-bearing observable (a deeper bound surfaces
    // MORE edge rows), the cid values feed the Gate-5 resolution probe below.
    let edge_cids = |stdout: &str| -> Vec<String> {
        stdout
            .lines()
            .filter_map(|line| {
                line.trim_start()
                    .strip_prefix("claim_cid:")
                    .map(|cid| cid.trim().to_string())
            })
            .collect()
    };

    // The WD-76 omitted-edge count a run reported at a given bound depth (parsed
    // from "Showing depth N; M edge(s) omitted."). No report line ⟹ nothing beyond
    // the bound ⟹ zero omitted.
    let omitted_count = |stdout: &str, depth: u8| -> u32 {
        let marker = format!("Showing depth {depth}; ");
        stdout
            .lines()
            .find(|line| line.contains(&marker) && line.contains("edge(s) omitted."))
            .and_then(|line| line.split(&marker).nth(1))
            .and_then(|rest| rest.split(" edge(s) omitted.").next())
            .and_then(|n| n.trim().parse().ok())
            .unwrap_or(0)
    };

    // Both runs are honest about their walk: each roots at the queried contributor
    // seed and carries the content-frozen Gate-5 notice (the depth override widens
    // the bound, it does NOT change the no-fabrication contract).
    for (label, stdout) in [("depth-2", two_out), ("depth-3", three_out)] {
        assert!(
            stdout.contains(&format!("contributor: {rachel_did}")),
            "expected the {label} traversal to root at the queried contributor {rachel_did};\n\
             --- stdout ---\n{stdout}\n--- graph ---\n{graph:?}"
        );
        assert!(
            stdout.contains("Traversal does not invent edges."),
            "expected the {label} traversal to carry the content-frozen Gate-5 honesty notice;\n\
             --- stdout ---\n{stdout}"
        );
    }

    // 1. The default depth-2 run reported a POSITIVE omitted-edge count — the
    //    precondition the `--depth 3` override relieves (the dense fan-out
    //    genuinely has edges beyond depth 2).
    let two_rows = edge_cids(two_out);
    let three_rows = edge_cids(three_out);
    let two_omitted = omitted_count(two_out, 2);
    assert!(
        two_omitted > 0,
        "cli.graph_query.omitted_edge_count_reported: the default depth-2 run must report a POSITIVE \
         omitted count for the dense fan-out (the precondition the override relieves);\n\
         --- depth-2 stdout ---\n{two_out}"
    );

    // 2. `--depth 3` REVEALS the previously-omitted edges: the depth-3 view renders
    //    strictly MORE edge rows than depth-2, and the increase is EXACTLY the
    //    count depth-2 reported as omitted — the depth-3 layer the default bound
    //    cut is now shown (`depth_three_rows == depth_two_rows + depth_two_omitted`).
    assert!(
        three_rows.len() > two_rows.len(),
        "cli.graph_query.traverse.edge_rows: `--depth 3` must render strictly MORE edge rows than \
         the default depth-2 bound — the previously-omitted deeper edges now appear (US-GRAPH-004 \
         Example 3 --depth override); got depth-2 rows={}, depth-3 rows={};\n\
         --- depth-2 stdout ---\n{two_out}\n--- depth-3 stdout ---\n{three_out}",
        two_rows.len(),
        three_rows.len()
    );
    assert_eq!(
        three_rows.len(),
        two_rows.len() + two_omitted as usize,
        "cli.graph_query.omitted_edge_count_reported: the additional edge rows the `--depth 3` \
         override reveals must be EXACTLY the count the depth-2 run reported as omitted — the \
         previously-omitted depth-3 edges are now revealed, none invented; got depth-2 rows={}, \
         depth-2 omitted={two_omitted}, depth-3 rows={};\n--- depth-2 stdout ---\n{two_out}\n\
         --- depth-3 stdout ---\n{three_out}",
        two_rows.len(),
        three_rows.len()
    );

    // 3. The revealed deeper edges are REAL signed claims (Gate 5): EVERY
    //    `claim_cid` the depth-3 view renders resolves to an existing signed claim
    //    via `graph query --subject github:rust-lang/cargo` (the override surfaces
    //    what already existed in the store — it fabricates nothing). The dense
    //    fixture's claims are all subscribed-PEER claims, and the traverse walk
    //    reads them under the explorer surface's implied federated scope (WD-87),
    //    so the resolution probe widens to the same scope with `--federated` (the
    //    bare own-only `--subject` would honestly return empty for peer claims —
    //    GQE-3 default-off).
    let subject_lookup = run_openlore(
        &env,
        &["graph", "query", "--subject", shared_subject, "--federated"],
    );
    assert_eq!(
        subject_lookup.status, 0,
        "graph query --subject {shared_subject} --federated (Gate-5 resolution probe) must exit 0;\n\
         --- stdout ---\n{}\n--- stderr ---\n{}",
        subject_lookup.stdout, subject_lookup.stderr
    );
    for cid in &three_rows {
        assert!(
            subject_lookup.stdout.contains(cid.as_str()),
            "Gate 5 (traversal invents no edges): the depth-3 edge cid {cid} must resolve to a real \
             signed claim via `graph query --subject {shared_subject} --federated` — the deeper edge \
             is genuine, not a fabrication;\n--- subject-lookup stdout ---\n{}\n\
             --- depth-3 stdout ---\n{three_out}",
            subject_lookup.stdout
        );
    }

    // 4. The depth-3 view is HONEST about its (wider) bound: it never claims to be
    //    still bounded at the default depth 2 — any omitted report it carries names
    //    depth 3 (the actual bound the override applied), not the superseded 2.
    assert!(
        !three_out.contains("Showing depth 2;"),
        "the --depth 3 traversal must NOT report the superseded default depth-2 bound — the \
         override raised the bound to depth 3;\n--- depth-3 stdout ---\n{three_out}"
    );
}

/// GQE-24 (US-GRAPH-004 happy; Gate 5): every traversed edge maps to a
/// VERIFIABLE signed claim — for any displayed edge between a project and a
/// contributor, the corresponding `claim_cid` is lookuppable via
/// `openlore graph query --subject <project>`. There is no edge that does not
/// trace to a signed claim. (US-GRAPH-004 Example 4 + UAT scenario 4.)
///
/// @us-graph-004 @real-io @driving_port @j-002 @gate-5 @anti-merging @happy
#[test]
fn graph_query_traverse_every_edge_maps_to_a_verifiable_signed_claim() {
    let env = TestEnv::initialized();
    let object = "org.openlore.philosophy.dependency-pinning";

    let graph = seed_federated_graph(
        &env,
        FederatedGraphFixture::dependency_pinning_rachel_spans_two_projects(),
    );

    let traversal = run_openlore(&env, &["graph", "query", "--object", object, "--traverse"]);
    assert_eq!(
        traversal.status, 0,
        "graph query --object --traverse must exit 0;\n--- stdout ---\n{}\n--- stderr ---\n{}",
        traversal.stdout, traversal.stderr
    );

    // For each edge in the traversal, the backing claim_cid resolves via a
    // `--subject` lookup (universe: cli.graph_query.traverse.edge_cids,
    // cli.graph_query.traverse.edge_cid_resolvable[cid]). Every edge also carries
    // the author DID of its backing claim, attributed in the same lookup
    // (anti-merging). No edge lacks a backing signed claim — the traversal
    // invents nothing (US-GRAPH-004 Example 4; Gate 5).
    assert_every_edge_has_backing_claim(&env, &traversal);
    let _ = &graph;
}

/// GQE-25 (US-GRAPH-004 / I-GRAPH-7): a traversal succeeds with the network
/// disabled — traversal reads the LOCAL graph only (WD-79/WD-92; extends
/// slice-01 KPI-5 / I-9).
///
/// @us-graph-004 @real-io @driving_port @j-002 @local-first @i-graph-7
#[test]
fn graph_query_traverse_succeeds_with_network_disabled() {
    let env = TestEnv::initialized();
    let object = "org.openlore.philosophy.dependency-pinning";

    let graph = seed_federated_graph(
        &env,
        FederatedGraphFixture::dependency_pinning_rachel_spans_two_projects(),
    );

    let outcome =
        run_openlore_network_disabled(&env, &["graph", "query", "--object", object, "--traverse"]);
    assert_eq!(
        outcome.status, 0,
        "traversal must succeed with the network disabled (local-first; WD-79/WD-92);\n\
         --- stdout ---\n{}\n--- stderr ---\n{}",
        outcome.stdout, outcome.stderr
    );

    // The traversal tree renders its FULL result from the LOCAL store alone — the
    // same observable surface as the GQE-20 happy path (same fixture), just with
    // no reachable network. Asserting the SAME universe (port-exposed stdout
    // slots) proves the traverse path is genuinely local-first (a pure local
    // recursive-CTE read), not merely "exits 0".
    //
    // Universe (port-exposed observable surface of the network-disabled
    // `--traverse` view): cli.graph_query.tree_roots_at_philosophy (the queried
    // object heads the tree), cli.graph_query.projects_in_tree (all 3 seeded
    // projects), cli.graph_query.connection_callout (names did:plc:rachel-test
    // spanning cargo+nixpkgs), cli.graph_query.edge_cid_resolvable[cid] (every
    // displayed edge maps to a backing signed claim resolvable via a LOCAL
    // `--subject` lookup), cli.graph_query.invents_no_edges_notice (the
    // content-frozen Gate-5 line), AND pds.create_record.call_count (0 — no
    // outbound call attempted). Asserted against stdout (the CLI driving-port
    // observable) + the fake PDS record log.
    let stdout = &outcome.stdout;

    // 1. The tree roots at the queried philosophy and fans to its 3 projects —
    //    rendered from the local DuckDB with no network.
    assert!(
        stdout.contains(&format!("philosophy: {object}")),
        "expected the traversal tree to root at the queried philosophy {object} from the LOCAL \
         store with the network disabled;\n--- stdout ---\n{stdout}\n--- graph ---\n{graph:?}"
    );
    let projects = [
        "github:rust-lang/cargo",
        "github:NixOS/nixpkgs",
        "github:denoland/deno",
    ];
    let projects_in_tree = projects
        .iter()
        .filter(|p| stdout.contains(&format!("project: {p}")))
        .count();
    assert_eq!(
        projects_in_tree, 3,
        "cli.graph_query.projects_in_tree: expected all 3 seeded projects in the tree from the \
         LOCAL store with the network disabled; got {projects_in_tree};\n--- stdout ---\n{stdout}"
    );

    // 2. The cross-project "Connections found" callout still surfaces from the
    //    local read — the non-obvious triangulation (KPI-GRAPH-1) is computed
    //    locally, not over the network.
    assert!(
        stdout.contains("Connections found"),
        "expected a 'Connections found' callout from the LOCAL traversal with the network \
         disabled;\n--- stdout ---\n{stdout}"
    );
    assert!(
        stdout.contains("did:plc:rachel-test spans 2 of these projects (github:rust-lang/cargo, github:NixOS/nixpkgs)"),
        "expected the callout to name did:plc:rachel-test spanning cargo+nixpkgs from the LOCAL \
         store with the network disabled (KPI-GRAPH-1);\n--- stdout ---\n{stdout}"
    );

    // 3. The full edge set renders from the local read (none dropped/fabricated):
    //    every seeded claim's cid appears on its edge, and the edge-cid count
    //    equals the seeded-claim count — the network-disabled traversal is
    //    COMPLETE, not degraded. Each edge also carries its backing claim's
    //    author DID (anti-merging WD-73). The networked Gate-5 resolution probe
    //    (`--subject` lookup per edge) is GQE-24's job; here the edge set is
    //    asserted directly on the network-disabled output so no secondary
    //    networked subprocess muddies the local-first claim.
    for claim in &graph.seeded {
        assert!(
            stdout.contains(&format!("author_did: {}", claim.author_did)),
            "expected every network-disabled traversal edge to carry its backing claim's author \
             DID {} (anti-merging WD-73);\n--- stdout ---\n{stdout}",
            claim.author_did
        );
    }
    let edge_cid_rows = stdout
        .lines()
        .filter(|line| line.trim_start().starts_with("claim_cid:"))
        .count();
    assert_eq!(
        edge_cid_rows,
        graph.seeded.len(),
        "expected exactly {} claim_cid-bearing edge rows from the LOCAL store with the network \
         disabled (full traversal, none fabricated/merged); got {edge_cid_rows};\n\
         --- stdout ---\n{stdout}",
        graph.seeded.len()
    );

    // 4. The content-frozen Gate-5 honesty notice still frames the local-first
    //    traversal verbatim.
    assert!(
        stdout.contains("Traversal does not invent edges."),
        "expected the content-frozen 'Traversal does not invent edges.' notice on the \
         network-disabled traversal (Gate 5);\n--- stdout ---\n{stdout}"
    );

    // 5. I-GRAPH-7 local-first: NO outbound PDS call was attempted. The
    //    traversal is a pure LOCAL recursive-CTE read — the fake PDS recorded
    //    zero create_record calls (port-exposed name pds.create_record.call_count).
    assert_no_pds_call_was_made(&env);
}

// =============================================================================
// US-GRAPH-003 + US-GRAPH-004 — Gate 6 scoring uses numeric confidence
// =============================================================================

/// GQE-26 (Gate 6 `scoring_uses_numeric_confidence`): the numeric confidence
/// shown in the per-claim rows of `--object` (the dimension query) is the SAME
/// numeric value the `--weighted` formula consumes — no silent rounding. A
/// claim displayed at 0.91 in the dimension view contributes 0.91 (not a
/// bucket-rounded value) to its weight in the weighted view.
/// (shared-artifacts-registry Gate 6; integration_validation confidence
/// must-match-across [1, 4].)
///
/// @us-graph-003 @real-io @driving_port @j-002 @gate-6 @kpi-graph-3
#[test]
fn graph_query_scoring_uses_the_same_numeric_confidence_shown_in_per_claim_rows() {
    let env = TestEnv::initialized();
    let object = "org.openlore.philosophy.dependency-pinning";

    // Seed a claim with a non-round confidence (e.g. 0.91) so a silent rounding
    // to a bucket-midpoint would be detectable in the explained arithmetic.
    let graph = seed_federated_graph(
        &env,
        FederatedGraphFixture::dependency_pinning_weighted_worked_example(),
    );

    // The dimension view shows the raw numeric confidence per row...
    let dimension = run_openlore(&env, &["graph", "query", "--object", object]);
    assert_eq!(
        dimension.status, 0,
        "dimension query must exit 0;\n--- stdout ---\n{}\n--- stderr ---\n{}",
        dimension.stdout, dimension.stderr
    );
    // ...and the --explain arithmetic must consume that SAME numeric value.
    let explained = run_openlore(
        &env,
        &[
            "graph",
            "query",
            "--object",
            object,
            "--weighted",
            "--explain",
            "github:rust-lang/cargo",
        ],
    );
    assert_eq!(
        explained.status, 0,
        "explain query must exit 0;\n--- stdout ---\n{}\n--- stderr ---\n{}",
        explained.stdout, explained.stderr
    );

    todo!(
        "DELIVER (slice-04): assert the numeric confidence shown in the `--object` per-claim row \
         (e.g. 0.91) is byte-equal to the base value the `--explain` arithmetic consumes for the \
         same claim — no silent rounding to a bucket midpoint (Gate 6 scoring_uses_numeric_confidence; \
         confidence must-match-across steps 1+4);\n--- graph ---\n{graph:?}"
    )
}

// =============================================================================
// US-GRAPH-006 — the slice-04 read-side / scoring contract is wired end-to-end
// =============================================================================

/// GQE-27 (US-GRAPH-006 infra; Gate 4 + Gate 1): a weighted query has run
/// end-to-end through the new read-side path (extended StoragePort scoring-feed
/// + pure scoring core + renderer). No `adherence_weight`/`weight_bucket`
/// appears in any stored/published location, AND the anti-merging check covers
/// the new scoring query path. This is the @infrastructure wiring proof that
/// US-GRAPH-001..005 depend on. (US-GRAPH-006 UAT scenario 2.)
///
/// @us-graph-006 @real-io @driving_port @infrastructure @gate-1 @gate-4
#[test]
fn graph_query_weighted_end_to_end_wires_scoring_feed_without_persisting_outputs() {
    let env = TestEnv::initialized();
    let object = "org.openlore.philosophy.dependency-pinning";

    // Seed the canonical worked-example graph and run a full weighted query —
    // the @infrastructure proof that the extended StoragePort scoring-feed
    // (query_attributed_for_scoring) -> pure scoring core -> WeightedRenderer
    // path is wired through the production composition root.
    let graph = seed_federated_graph(
        &env,
        FederatedGraphFixture::dependency_pinning_weighted_worked_example(),
    );

    let outcome = run_openlore(&env, &["graph", "query", "--object", object, "--weighted"]);
    assert_eq!(
        outcome.status, 0,
        "end-to-end weighted query must exit 0 (the @infrastructure wiring proof);\n\
         --- stdout ---\n{}\n--- stderr ---\n{}",
        outcome.stdout, outcome.stderr
    );

    // Gate 4: nothing persisted. Gate 1: the rendered aggregate decomposes to
    // per-author contributions (no faceless consensus row).
    assert_weight_not_persisted(&env);

    todo!(
        "DELIVER (slice-04): assert a weighted query runs end-to-end through the extended \
         StoragePort scoring-feed -> pure scoring core -> renderer (the @infrastructure wiring), no \
         scoring output is persisted (Gate 4), and the rendered aggregate decomposes to per-author \
         contributions (Gate 1) (US-GRAPH-006 UAT scenario 2);\n--- graph ---\n{graph:?}"
    )
}
