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
    todo!(
        "DELIVER (slice-04): assert `graph query --contributor` lists all 5 of Rachel's claims \
         across 4 subjects with subject/object/confidence/cid, and the footer states 'one \
         developer's reasoning trail, not a community consensus' (US-GRAPH-002 Example 1);\n\
         --- graph ---\n{graph:?}"
    )
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

    todo!(
        "DELIVER (slice-04): assert querying one's OWN DID lists the own claims annotated '(you)' \
         (not '(subscribed peer)') — a valid self-review — and exit 0 (US-GRAPH-002 Example 2);\n\
         --- graph ---\n{graph:?}"
    )
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

    let outcome = run_openlore(
        &env,
        &["graph", "query", "--contributor", "did:plc:stranger-test"],
    );

    // Absent contributor is NOT an error: exit 0 (a valid empty result).
    assert_eq!(
        outcome.status, 0,
        "graph query --contributor for an absent DID must exit 0 (valid empty result);\n\
         --- stdout ---\n{}\n--- stderr ---\n{}",
        outcome.stdout, outcome.stderr
    );

    todo!(
        "DELIVER (slice-04): assert an absent contributor yields a no-local-claims message + a \
         subscribe/pull hint ('openlore peer add' + 'openlore peer pull') and exit 0 \
         (US-GRAPH-002 Example 3);\n--- graph ---\n{graph:?}"
    )
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

    todo!(
        "DELIVER (slice-04): assert a soft-removed peer's cached claims list annotated \
         '(unsubscribed cache)' (not '(subscribed peer)'), every row carries Tobias's DID, and \
         exit 0 (US-GRAPH-002 Example 4; slice-03 relationship-label reuse);\n--- graph ---\n{graph:?}"
    )
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

    // 1. Projects ranked by adherence weight (desc).
    // 2. Each weight shown WITH its inputs: claim count, distinct author count,
    //    max confidence, cross-project span.
    // 3. The formula is printed AND the output states "no ML".
    // 4. A footer states weights are a display-only aggregate view, never stored.
    // DELIVER materializes `assert_weighted_view_transparent` pinning the
    // universe: cli.graph_query.ranking_order, cli.graph_query.formula_printed,
    // cli.graph_query.no_ml_stated, cli.graph_query.never_stored_footer.
    todo!(
        "DELIVER (slice-04): assert `--weighted` ranks projects by weight, shows each weight with \
         its inputs (claim count, distinct authors, max confidence, span), prints the formula and \
         states 'no ML', and footers the never-stored display-only notice (US-GRAPH-003 Example 1; \
         Gate 2; KPI-GRAPH-3);\n--- graph ---\n{graph:?}"
    )
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
    // manufactures NO confidence. DELIVER materializes
    // `assert_sparse_rendered_as_sparse` (universe: cli.graph_query.bucket[subject],
    // cli.graph_query.sparse_honesty_line_present, cli.graph_query.no_manufactured_confidence).
    todo!(
        "DELIVER (slice-04): assert the single-claim single-author tokio pairing renders [SPARSE] \
         with the 'based on 1 claim by 1 author' honesty line + lead-not-conclusion advice, and no \
         confidence is manufactured (US-GRAPH-003 Example 2; Gate 3 sparse_renders_sparse; \
         KPI-GRAPH-4 release-gate);\n--- graph ---\n{graph:?}"
    )
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

    todo!(
        "DELIVER (slice-04): assert deno's weight includes the per-additional-distinct-author \
         bonus, the breakdown states 'multi-author: 2 distinct authors raise triangulation', deno \
         ranks above the single-author comparator at similar max confidence, and both authors \
         remain individually attributed (US-GRAPH-003 Example 3; KPI-GRAPH-1/2);\n--- graph ---\n{graph:?}"
    )
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

    todo!(
        "DELIVER (slice-04): assert both conflicting claims (0.85 and 0.20) contribute to the \
         weight per their confidence, the breakdown shows both authors + both confidences, and NO \
         claim is dropped or collapsed into a single averaged value (US-GRAPH-003 Example 4; \
         KPI-GRAPH-2);\n--- graph ---\n{graph:?}"
    )
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

    todo!(
        "DELIVER (slice-04): assert no weight/bucket string is persisted in any table or artifact \
         after the weighted query, AND the re-run after adding a contributing claim yields a \
         DIFFERENT weight for the affected pairing (US-GRAPH-003 Example 5; Gate 4 \
         weight_and_bucket_never_persisted; release-gate);\n--- graph ---\n{graph:?}"
    )
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

    todo!(
        "DELIVER (slice-04): assert the weighted view renders fully with the network disabled and \
         NO network call is attempted (I-GRAPH-7 local-first);\n--- graph ---\n{graph:?}"
    )
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
    // displayed weight; no claim is merged. DELIVER materializes
    // `assert_explain_sums_to_weight` + `assert_weight_decomposes_to_per_author`.
    todo!(
        "DELIVER (slice-04): assert `--explain github:denoland/deno` enumerates Tobias's + Maria's \
         contributing claims (author DID + cid + confidence + applied bonus each), the running sum \
         equals the displayed weight (reproduce-by-hand), and no claim is merged into a faceless \
         aggregate (US-GRAPH-005 Example 1; Gate 1 + Gate 2);\n--- graph ---\n{graph:?}"
    )
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

    todo!(
        "DELIVER (slice-04): assert `--explain` on the sparse tokio subject shows the single \
         contributing claim with no bonuses + running sum 0.50, AND repeats the [SPARSE] 'based on \
         1 claim by 1 author' honesty line (US-GRAPH-005 Example 2; Gate 3);\n--- graph ---\n{graph:?}"
    )
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

    todo!(
        "DELIVER (slice-04): assert `--explain github:foo/bar` (absent from the result set) prints \
         'Subject github:foo/bar is not in this result set.' and exits non-zero — a usage error \
         distinct from an empty dimension query's exit 0 (US-GRAPH-005 Example 3);\n--- graph ---\n{graph:?}"
    )
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

    todo!(
        "DELIVER (slice-04): assert cargo's --explain breakdown shows the base claim PLUS a \
         '+0.5 cross-project triangulation' line attributed to did:plc:rachel-test (who spans \
         cargo+nixpkgs), and the running sum equals cargo's displayed weight (US-GRAPH-005 \
         Example 4; Gate 1 attribution-of-bonus);\n--- graph ---\n{graph:?}"
    )
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
    todo!(
        "DELIVER (slice-04): assert `--traverse` renders the philosophy->projects->authors tree, a \
         'Connections found' callout names did:plc:rachel-test spanning 2 projects, every edge maps \
         to exactly one signed claim, and the output states 'Traversal does not invent edges.' \
         (US-GRAPH-004 Example 1; KPI-GRAPH-1 north star; Gate 5);\n--- graph ---\n{graph:?}"
    )
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

    todo!(
        "DELIVER (slice-04): assert the single isolated tokio node renders with 'no connecting \
         edges found at depth 2' and NO connection to any other project/contributor is fabricated \
         (US-GRAPH-004 Example 2; Gate 5 traversal_invents_no_edges);\n--- graph ---\n{graph:?}"
    )
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
    // count + how to go deeper. DELIVER materializes the bounded-depth assertion
    // (universe: cli.graph_query.max_depth_shown, cli.graph_query.omitted_edge_count_reported).
    todo!(
        "DELIVER (slice-04): assert the dense traversal is bounded to depth 2 by default, reports \
         how many edges were omitted and how to go deeper with --depth, and returns responsively \
         (US-GRAPH-004 Example 3; WD-76/WD-91 bounded);\n--- graph ---\n{graph:?}"
    )
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

    todo!(
        "DELIVER (slice-04): assert `--depth 3` reveals edges the default depth-2 run omitted, the \
         deeper edges are real signed claims (Gate 5, lookuppable via --subject), and the omitted \
         count shrinks accordingly (US-GRAPH-004 Example 3 --depth override);\n--- graph ---\n{graph:?}"
    )
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
    // `--subject` lookup. DELIVER materializes `assert_every_edge_has_backing_claim`
    // (universe: cli.graph_query.edge_cids, cli.graph_query.edge_cid_resolvable[cid]).
    // Every edge also carries the author DID of its backing claim (anti-merging).
    todo!(
        "DELIVER (slice-04): for every traversal edge, assert its claim_cid is lookuppable via \
         `graph query --subject <project>` (it traces to a real signed claim), and every edge \
         carries the author DID of its backing claim — no edge lacks a backing signed claim \
         (US-GRAPH-004 Example 4; Gate 5 + anti-merging);\n--- graph ---\n{graph:?}"
    )
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

    todo!(
        "DELIVER (slice-04): assert the traversal tree renders fully with the network disabled and \
         NO network call is attempted (I-GRAPH-7 local-first);\n--- graph ---\n{graph:?}"
    )
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
