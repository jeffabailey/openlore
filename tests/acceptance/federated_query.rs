//! Slice-03 acceptance — `openlore graph query --federated [--subject <S>]`.
//!
//! The load-bearing surface for J-003a (anti-merging) per ADR-014
//! invariant I-FED-1 + KPI-FED-1 + KPI-FED-2. Every output row carries
//! its author DID; NO row represents a multi-author aggregate; the
//! footer states the no-merge guarantee verbatim.
//!
//! Covers:
//! - US-FED-003: federated query with per-author attribution (happy +
//!   edge + same-content-different-authors + author-only-default)
//! - WD-42: inline counter-claim template is shown by default in
//!   `--federated` output (resolved from `# DISTILL: confirm` habit
//!   scenario 2 inline-template trigger)
//! - WD-39: first-federated-query orientation fires once-per-user
//!   (resolved from `# DISTILL: confirm` habit scenario 1)
//! - Integration gate `federation_attribution_preserved` (mandatory;
//!   KPI-FED-1 + KPI-FED-2 release-gate)
//
// SCAFFOLD: true

mod support;

#[allow(unused_imports)]
use support::*;

// =============================================================================
// US-FED-003 — happy + edge paths
// =============================================================================

/// FQ-1: Maria has 1 of her own claims about
/// `github:rust-lang/cargo` + 2 pulled claims from
/// `did:plc:rachel-test` about the same subject. Running
/// `openlore graph query --subject github:rust-lang/cargo --federated`
/// returns exactly 3 rows grouped under 2 author headers:
/// `did:plc:test-maria (you)` (1 row) and
/// `did:plc:rachel-test (subscribed peer)` (2 rows). Every row carries
/// author_did + confidence + cid. The footer states the count of
/// distinct authors (2) AND the literal "Each claim is attributed to
/// its author DID. No claims are merged." Drives integration gate 1
/// (`federation_attribution_preserved`) + KPI-FED-1 + KPI-FED-2.
///
/// @us-fed-003 @real-io @driving_port @j-003a @kpi-fed-1 @kpi-fed-2 @happy
#[test]
fn federated_query_returns_author_and_peer_claims_grouped_by_author_did() {
    let env = TestEnv::initialized();

    // The shared subject both authors make claims about. The verifiable
    // peer-record builder publishes its claims under this exact subject,
    // so the user's own claim must use it too for the federated query to
    // group both authors under one subject.
    let subject = "github:rust-lang/cargo";

    // -- Author 1: the LOCAL user adds ONE of their own claims --
    // Routed through the real `claim add` verb (no PDS publish — the
    // first two-prompt step signs + persists locally, which is all the
    // federated READ path needs). `\n` confirms the sign prompt; the
    // empty publish answer declines publishing (local-only).
    let own = run_openlore_with_stdin(
        &env,
        &[
            "claim",
            "add",
            "--subject",
            subject,
            "--predicate",
            "embodiesPhilosophy",
            "--object",
            "org.openlore.philosophy.local-first",
            "--evidence",
            "https://github.com/rust-lang/cargo",
            "--confidence",
            "0.91",
        ],
        "\nN\n",
    );
    assert_eq!(
        own.status, 0,
        "claim add precondition must succeed;\n--- stdout ---\n{}\n--- stderr ---\n{}",
        own.stdout, own.stderr
    );

    // -- Author 2: a subscribed PEER (Rachel) publishes claims about the
    // SAME subject. `build_verifiable_peer_records` materializes real
    // Ed25519 signatures over `github:rust-lang/cargo` so the pull
    // pipeline's per-record verify + CID-recompute pass. --
    let peer_did = "did:plc:rachel-test";
    let rachel_seed = [7u8; 32];
    let (records, rachel_pubkey_hex) = build_verifiable_peer_records(peer_did, rachel_seed);
    let peer_claim_count = records.len();
    let peer = PeerPds::for_peer(peer_did, records);

    // Precondition: subscribe via the real `peer add` verb.
    let added = run_openlore_with_peer_resolver(
        &env,
        &["peer", "add", peer_did],
        peer_did,
        peer.endpoint_url(),
    );
    assert_eq!(
        added.status, 0,
        "peer add precondition must succeed;\n--- stdout ---\n{}\n--- stderr ---\n{}",
        added.stdout, added.stderr
    );

    // Precondition: pull Rachel's claims into `peer_claims` via the real
    // `peer pull` verb (each row attributed to Rachel; anti-merging held).
    let pulled = run_openlore_pull(
        &env,
        &["peer", "pull"],
        peer_did,
        peer.endpoint_url(),
        &rachel_pubkey_hex,
    );
    assert_eq!(
        pulled.status, 0,
        "peer pull precondition must succeed;\n--- stdout ---\n{}\n--- stderr ---\n{}",
        pulled.stdout, pulled.stderr
    );
    // Confirm the cache is populated as expected before the read.
    assert_peer_claims_attributed_to(&env, peer_did, peer_claim_count);

    // -- Action: the federated read through the driving port --
    let outcome = run_openlore(&env, &["graph", "query", "--subject", subject, "--federated"]);

    assert_eq!(
        outcome.status, 0,
        "graph query --federated must exit 0;\n--- stdout ---\n{}\n--- stderr ---\n{}",
        outcome.status,
        outcome.stdout,
    );

    let stdout = &outcome.stdout;

    // 1. BOTH authors appear, grouped under per-author headers. The local
    //    user's bare DID carries the "(you)" annotation; Rachel's the
    //    "(subscribed peer)" annotation (ADR-013 header convention).
    let local_did = env.identity.author_did(); // bare DID, no key fragment
    assert!(
        stdout.contains(local_did),
        "expected a per-author header naming the local user's DID {local_did};\n\
         --- stdout ---\n{stdout}"
    );
    assert!(
        stdout.contains(peer_did),
        "expected a per-author header naming the peer DID {peer_did};\n\
         --- stdout ---\n{stdout}"
    );
    assert!(
        stdout.contains("(you)"),
        "expected the local user's header annotated '(you)';\n--- stdout ---\n{stdout}"
    );
    assert!(
        stdout.contains("(subscribed peer)"),
        "expected the peer's header annotated '(subscribed peer)';\n--- stdout ---\n{stdout}"
    );

    // 2. Every row carries author_did + confidence + cid. The own claim's
    //    confidence (0.91) and each peer claim's CID must surface verbatim.
    assert!(
        stdout.contains("0.91"),
        "expected the own claim's confidence 0.91 rendered verbatim;\n\
         --- stdout ---\n{stdout}"
    );
    assert!(
        stdout.contains("org.openlore.philosophy.local-first"),
        "expected the own claim's object rendered;\n--- stdout ---\n{stdout}"
    );
    // Every peer record's CID (== its rkey) appears as a row CID.
    for record in peer.records() {
        assert!(
            stdout.contains(&record.rkey),
            "expected the peer claim cid {} to appear as a row cid;\n--- stdout ---\n{stdout}",
            record.rkey
        );
    }
    // The "cid:" / "confidence:" / "author_did" labels frame every row.
    for label in ["confidence", "cid"] {
        assert!(
            stdout.contains(label),
            "expected each row to carry a `{label}` field;\n--- stdout ---\n{stdout}"
        );
    }

    // 3. Footer states the count of distinct authors (2) AND the literal
    //    no-merge guarantee (ADR-013 footer convention; content-frozen).
    assert!(
        stdout.contains("Each claim is attributed to its author DID. No claims are merged."),
        "expected the content-frozen no-merge footer guarantee;\n--- stdout ---\n{stdout}"
    );
    assert!(
        stdout.contains('2'),
        "expected the footer to state the distinct-author count (2);\n--- stdout ---\n{stdout}"
    );

    // 4. KPI-FED-2 zero-merge gate: NO row labels itself merged / consensus
    //    / aggregate.
    assert_no_merged_rows_in_federated_output(&outcome);
}

/// FQ-2: Maria + Rachel publish two DIFFERENT claims with the SAME
/// (subject, predicate, object) triple but different confidence
/// values. The federated query renders BOTH as distinct rows under
/// their respective author headers. There is NO single "Both authors
/// agree" / "consensus" / "merged" row. (KPI-FED-2 zero-merge
/// guardrail; US-FED-003 AC 5 + Example 3.)
///
/// @us-fed-003 @real-io @driving_port @j-003a @kpi-fed-2 @anti-merging
#[test]
fn federated_query_renders_identical_content_from_different_authors_as_two_separate_rows() {
    let env = TestEnv::initialized();

    // The shared subject + the IDENTICAL (subject, predicate, object) triple
    // both authors make a claim about. Rachel's verifiable peer records use
    // this exact subject/predicate; her FIRST triple's object
    // (`org.openlore.philosophy.dependency-pinning`) is the one the local
    // user mirrors, so the two claims share an identical (subject, predicate,
    // object) content tuple under DIFFERENT authors — the precise zero-merge
    // fixture (KPI-FED-2; US-FED-003 AC 5 + Example 3).
    let subject = "github:rust-lang/cargo";
    let shared_predicate = "embodiesPhilosophy";
    let shared_object = "org.openlore.philosophy.dependency-pinning";

    // -- Author 1 (the LOCAL user) adds ONE of her OWN claims with the SAME
    // content triple but a DIFFERENT confidence (0.91 vs Rachel's 0.42). `\n`
    // confirms the sign prompt; `N\n` declines publishing (local-only — the
    // federated READ path only needs the locally-signed claim). --
    let own = run_openlore_with_stdin(
        &env,
        &[
            "claim",
            "add",
            "--subject",
            subject,
            "--predicate",
            shared_predicate,
            "--object",
            shared_object,
            "--evidence",
            "https://github.com/rust-lang/cargo",
            "--confidence",
            "0.91",
        ],
        "\nN\n",
    );
    assert_eq!(
        own.status, 0,
        "claim add precondition must succeed;\n--- stdout ---\n{}\n--- stderr ---\n{}",
        own.stdout, own.stderr
    );

    // -- Author 2 (subscribed PEER Rachel) publishes verifiable claims about
    // the SAME subject. Her FIRST record carries the IDENTICAL (subject,
    // predicate, object) triple the local user just authored (confidence
    // 0.42) — so the federated read sees identical content from two distinct
    // authors. --
    let peer_did = "did:plc:rachel-test";
    let rachel_seed = [7u8; 32];
    let (records, rachel_pubkey_hex) = build_verifiable_peer_records(peer_did, rachel_seed);
    // The first verifiable record IS the identical-content sibling (its
    // (subject, predicate, object) == the local user's triple, confidence
    // 0.42). Capture its CID so the two-distinct-rows assertion can pin both.
    let peer = PeerPds::for_peer(peer_did, records);
    let peer_records = peer.records();
    let shared_content_peer_cid = peer_records[0].rkey.clone();

    // Precondition: subscribe to Rachel via the real `peer add` verb.
    let added = run_openlore_with_peer_resolver(
        &env,
        &["peer", "add", peer_did],
        peer_did,
        peer.endpoint_url(),
    );
    assert_eq!(
        added.status, 0,
        "peer add precondition must succeed;\n--- stdout ---\n{}\n--- stderr ---\n{}",
        added.stdout, added.stderr
    );

    // Precondition: pull Rachel's verified claims into `peer_claims`.
    let pulled = run_openlore_pull(
        &env,
        &["peer", "pull"],
        peer_did,
        peer.endpoint_url(),
        &rachel_pubkey_hex,
    );
    assert_eq!(
        pulled.status, 0,
        "peer pull precondition must succeed;\n--- stdout ---\n{}\n--- stderr ---\n{}",
        pulled.stdout, pulled.stderr
    );
    assert_peer_claims_attributed_to(&env, peer_did, peer_records.len());

    // -- Action: the federated read through the driving port --
    let outcome = run_openlore(&env, &["graph", "query", "--subject", subject, "--federated"]);
    assert_eq!(
        outcome.status, 0,
        "graph query --federated must exit 0;\n--- stdout ---\n{}\n--- stderr ---\n{}",
        outcome.stdout, outcome.stderr,
    );

    let stdout = &outcome.stdout;
    let local_did = env.identity.author_did(); // bare DID, no key fragment

    // DD-FED-10 (LOAD-BEARING) — federated-output universe over the
    // observable stdout surface. The zero-merge guardrail is the assertion
    // that identical content from two authors NEVER collapses into one row.
    //
    // Universe slots (port-exposed, derived from the rendered output —
    // never internal struct fields):
    //   - cli.graph_query.distinct_authors_in_output
    //   - cli.graph_query.own_content_row_present
    //   - cli.graph_query.peer_content_row_present
    //   - cli.graph_query.rows_collapsed (must be ZERO)

    // Slot: distinct_authors_in_output == 2. BOTH author headers appear, each
    // under its own relationship annotation — never one combined "both
    // authors" header.
    assert!(
        stdout.contains(local_did),
        "expected the local user's per-author header naming DID {local_did};\n\
         --- stdout ---\n{stdout}"
    );
    assert!(
        stdout.contains(peer_did),
        "expected the peer's per-author header naming DID {peer_did};\n\
         --- stdout ---\n{stdout}"
    );
    assert!(
        stdout.contains("(you)"),
        "expected the local user's header annotated '(you)';\n--- stdout ---\n{stdout}"
    );
    assert!(
        stdout.contains("(subscribed peer)"),
        "expected the peer's header annotated '(subscribed peer)';\n--- stdout ---\n{stdout}"
    );
    let distinct_authors_in_output = stdout
        .lines()
        .filter(|l| l.starts_with("author: "))
        .count();
    assert_eq!(
        distinct_authors_in_output, 2,
        "DD-FED-10: expected exactly 2 distinct author headers (zero-merge: \
         identical content from two authors renders under two SEPARATE author \
         headers, never one aggregate);\n--- stdout ---\n{stdout}"
    );

    // Slot: own_content_row_present + peer_content_row_present — the
    // identical-content pair survives as TWO distinct rows. Both confidences
    // appear verbatim (0.91 from the local user, 0.42 from Rachel) for the
    // SAME (subject, predicate, object) triple: proof no aggregation occurred.
    assert!(
        stdout.contains(shared_object),
        "expected the shared (identical-content) object {shared_object} to render;\n\
         --- stdout ---\n{stdout}"
    );
    assert!(
        stdout.contains("0.91"),
        "expected the local user's confidence 0.91 rendered verbatim (its own row);\n\
         --- stdout ---\n{stdout}"
    );
    assert!(
        stdout.contains("0.42"),
        "expected the peer's confidence 0.42 rendered verbatim (its own row) — \
         the identical-content claims are NOT averaged/merged into one row;\n\
         --- stdout ---\n{stdout}"
    );

    // Slot: both cids present AND distinct. The local own claim's cid and the
    // identical-content peer claim's cid each appear EXACTLY ONCE — no row
    // collapse (rows_collapsed == 0), no row duplication.
    assert_eq!(
        stdout.matches(&shared_content_peer_cid).count(),
        1,
        "DD-FED-10: the identical-content peer claim cid {shared_content_peer_cid} must \
         appear EXACTLY ONCE as its own distinct row (no merge / no drop / no dup);\n\
         --- stdout ---\n{stdout}"
    );
    // The two authors' rows are independently attributable: the peer cid must
    // sit under the peer header, and the local DID owns a separate header — so
    // the identical-content claims cannot have collapsed into a single row.
    let peer_header_idx = stdout
        .find(&format!("author: {peer_did}"))
        .expect("peer author header present");
    let peer_cid_idx = stdout
        .find(&shared_content_peer_cid)
        .expect("peer content cid present");
    assert!(
        peer_cid_idx > peer_header_idx,
        "DD-FED-10: the identical-content peer cid must render UNDER the peer's own \
         author header (distinct rows preserved);\n--- stdout ---\n{stdout}"
    );

    // KPI-FED-2 zero-merge gate (release-blocking): NO row labels itself
    // merged / consensus / aggregate. The no-merge footer's own "are merged"
    // sentence is excluded by the helper.
    assert_no_merged_rows_in_federated_output(&outcome);
}

/// FQ-3: `openlore graph query --subject <S>` WITHOUT `--federated`
/// behaves byte-identically to slice-01: shows ONLY the user's own
/// claims, footer announces "Use --federated to include N subscribed
/// peer(s)". This is the regression gate that ensures the new flag is
/// strictly opt-in and does NOT alter the default. (US-FED-003 AC 2 +
/// UAT scenario #3.)
///
/// @us-fed-003 @real-io @driving_port @j-003 @regression @default-off
#[test]
fn federated_query_default_without_flag_is_byte_identical_to_slice_01_behavior() {
    let env = TestEnv::initialized();
    let subject = "github:rust-lang/cargo";

    // -- The LOCAL user adds ONE of their own claims (the only row the
    // default, non-federated path may ever surface). --
    let own = run_openlore_with_stdin(
        &env,
        &[
            "claim",
            "add",
            "--subject",
            subject,
            "--predicate",
            "embodiesPhilosophy",
            "--object",
            "org.openlore.philosophy.local-first",
            "--evidence",
            "https://github.com/rust-lang/cargo",
            "--confidence",
            "0.91",
        ],
        "\nN\n",
    );
    assert_eq!(
        own.status, 0,
        "claim add precondition must succeed;\n--- stdout ---\n{}\n--- stderr ---\n{}",
        own.stdout, own.stderr
    );

    // BASELINE: capture the default (non-federated) output BEFORE any peer
    // exists. This is the slice-01 behaviour — own claim only, the
    // content-frozen local-only header, and the federation pointer footer
    // naming `--federated` + `slice-03` (WS-12). It is the byte-for-byte
    // oracle the post-subscription run must reproduce.
    let baseline = run_openlore(&env, &["graph", "query", "--subject", subject]);
    assert_eq!(
        baseline.status, 0,
        "baseline graph query must exit 0;\n--- stdout ---\n{}\n--- stderr ---\n{}",
        baseline.stdout, baseline.stderr
    );

    // -- Now subscribe to a peer AND pull 2+ of their verified claims about
    // the SAME subject, so `peer_claims` is genuinely populated. This makes
    // the regression assertion load-bearing: if the default path EVER widened
    // to peers, these rows WOULD appear. They must not. --
    let peer_did = "did:plc:rachel-test";
    let rachel_seed = [7u8; 32];
    let (records, rachel_pubkey_hex) = build_verifiable_peer_records(peer_did, rachel_seed);
    let peer_claim_count = records.len();
    let peer = PeerPds::for_peer(peer_did, records);

    let added = run_openlore_with_peer_resolver(
        &env,
        &["peer", "add", peer_did],
        peer_did,
        peer.endpoint_url(),
    );
    assert_eq!(
        added.status, 0,
        "peer add precondition must succeed;\n--- stdout ---\n{}\n--- stderr ---\n{}",
        added.stdout, added.stderr
    );

    let pulled = run_openlore_pull(
        &env,
        &["peer", "pull"],
        peer_did,
        peer.endpoint_url(),
        &rachel_pubkey_hex,
    );
    assert_eq!(
        pulled.status, 0,
        "peer pull precondition must succeed;\n--- stdout ---\n{}\n--- stderr ---\n{}",
        pulled.stdout, pulled.stderr
    );
    // Confirm peer_claims is non-empty — the precondition that makes the
    // default-off regression meaningful (peers exist, yet the default path
    // ignores them).
    assert_peer_claims_attributed_to(&env, peer_did, peer_claim_count);

    // -- Action: the DEFAULT read through the driving port — NO `--federated`. --
    let outcome = run_openlore(&env, &["graph", "query", "--subject", subject]);
    assert_eq!(
        outcome.status, 0,
        "default graph query must exit 0;\n--- stdout ---\n{}\n--- stderr ---\n{}",
        outcome.stdout, outcome.stderr
    );

    let stdout = &outcome.stdout;

    // 1. BYTE-IDENTITY (the regression guarantee, US-FED-003 AC #2 +
    //    architecture §5.1 invariant #5): the non-federated output AFTER
    //    subscribing + pulling peers is byte-for-byte equal to the slice-01
    //    baseline captured BEFORE any peer existed. Subscribing to peers does
    //    NOT alter the default path in any byte.
    assert_eq!(
        stdout, &baseline.stdout,
        "default (non-federated) output must be BYTE-IDENTICAL to the slice-01 \
         baseline (captured before any peer existed) — the --federated flag is \
         strictly opt-in and does NOT change the default path;\n\
         --- baseline stdout ---\n{}\n--- after-peers stdout ---\n{}",
        baseline.stdout, stdout
    );

    // 2. The slice-01 contract holds verbatim (WS-12): own claim rendered,
    //    content-frozen local-only header, federation-pointer footer naming
    //    both `--federated` and `slice-03`.
    assert!(
        stdout.contains("Showing local claims only"),
        "expected the content-frozen slice-01 local-only header;\n--- stdout ---\n{stdout}"
    );
    assert!(
        stdout.contains("--federated") && stdout.contains("slice-03"),
        "expected the slice-01 footer to name both `--federated` and `slice-03`;\n\
         --- stdout ---\n{stdout}"
    );
    assert!(
        stdout.contains("0.91") && stdout.contains("org.openlore.philosophy.local-first"),
        "expected the user's OWN claim (confidence 0.91 + its object) to render;\n\
         --- stdout ---\n{stdout}"
    );

    // 3. NO peer rows: the peer's DID never appears, and none of the pulled
    //    peer claim CIDs leak into the default output.
    assert!(
        !stdout.contains(peer_did),
        "default (non-federated) output must NOT name the peer DID {peer_did} — \
         peer claims are excluded unless --federated is passed;\n--- stdout ---\n{stdout}"
    );
    for record in peer.records() {
        assert!(
            !stdout.contains(&record.rkey),
            "default output must NOT contain the peer claim cid {} — peer rows are \
             excluded without --federated;\n--- stdout ---\n{stdout}",
            record.rkey
        );
    }

    // 4. NO federated grouping artefacts: neither the federated no-merge
    //    guarantee footer nor the per-author relationship annotations
    //    (`(you)` / `(subscribed peer)`) that ONLY the --federated grouping
    //    renders may appear on the default path. (The `author:` field label
    //    is shared with the slice-01 per-claim block, so it is NOT a
    //    federated-only marker — the relationship annotations are.)
    assert!(
        !stdout.contains("Each claim is attributed to its author DID. No claims are merged."),
        "default output must NOT carry the federated no-merge footer (that footer \
         belongs only to the --federated path);\n--- stdout ---\n{stdout}"
    );
    assert!(
        !stdout.contains("(subscribed peer)") && !stdout.contains("(you)"),
        "default output must NOT carry the federated per-author relationship \
         annotations ('(you)' / '(subscribed peer)') — federated grouping is opt-in;\n\
         --- stdout ---\n{stdout}"
    );
}

/// FQ-4: `--federated` requested with zero peer subscriptions degrades
/// gracefully: output shows ONLY the user's own claims; footer is
/// "No peers subscribed. Use `openlore peer add <did>` to follow a
/// peer's claim stream." (US-FED-003 AC 7 + UAT scenario #4.)
///
/// @us-fed-003 @real-io @driving_port @j-003 @edge
#[test]
fn federated_query_with_zero_peers_subscribed_degrades_with_hint() {
    todo!("DELIVER (slice-03): assert --federated against zero subscriptions returns own-only rows + footer literal 'No peers subscribed. Use `openlore peer add <did>` to follow a peer's claim stream.' + exit 0")
}

// =============================================================================
// US-FED-003 — counter-relationship annotation (bidirectional)
// =============================================================================

/// FQ-5: After Maria publishes a counter-claim (`bafy...new`) against
/// Rachel's `bafy...n4ka`, a subsequent federated query annotates BOTH
/// rows bidirectionally: Maria's row shows
/// "counters bafy...n4ka by did:plc:rachel-test"; Rachel's row shows
/// "countered-by bafy...new by did:plc:test-maria". The summary line
/// states the count of counter-relationships explicitly. (US-FED-004
/// AC 9 + US-FED-003 AC 8; chained narrative across counter +
/// federated query.)
///
/// @us-fed-003 @us-fed-004 @real-io @driving_port @j-003a @j-003b @happy
#[test]
fn federated_query_annotates_counter_relationships_bidirectionally() {
    todo!("DELIVER (slice-03): chained scenario — Maria pulls Rachel's records (state set up by reusing peer_pull step-method invocation), runs claim counter, then graph query --federated. Assert bidirectional annotations on both rows + summary line names the count. Uses StoragePort.query_federated_by_subject + peer_claim_references / claim_references join per data-models.md § Cross-store query examples.")
}

// =============================================================================
// US-FED-003 — habit-bridging affordances (resolve WD-39 + WD-42)
// =============================================================================

/// FQ-6 (WD-39 — RESOLVES `# DISTILL: confirm` habit scenario 1
/// first-federated-query trigger): The FIRST EVER
/// `openlore graph query --federated` invocation per install emits a
/// one-line orientation message verbatim:
/// "First federated query complete. Peer claims appear under their
/// author DIDs. No claims are merged. Use `openlore peer add <did>` to
/// follow more peers."
/// Subsequent invocations DO NOT emit the orientation. State lives in
/// `~/.config/openlore/identity.toml` under
/// `[federation] first_federated_query_completed_at`.
///
/// @us-fed-003 @real-io @driving_port @j-003 @habit @wd-39
#[test]
fn federated_query_first_invocation_emits_orientation_then_omits_on_subsequent_invocations() {
    todo!("DELIVER (slice-03): wire OrientationState.first_federated_query_completed_at check in VerbGraphQuery --federated branch; assert orientation present in first invocation stdout AND absent in second invocation stdout AND identity.toml gains the timestamp key after success")
}

/// FQ-7 (WD-42 — RESOLVES `# DISTILL: confirm` habit scenario 2 inline
/// template trigger): Per peer-claim row in `--federated` output, the
/// renderer includes a copy-pasteable counter template:
/// `openlore claim counter <peer_cid> --reason "..." --subject ...
/// --predicate ... --object ... --evidence ... --confidence ...`
/// The template pre-fills subject + predicate + object from the target
/// claim; the user fills in --reason + --evidence + --confidence.
/// Shown by DEFAULT (WD-42; NOT gated behind `--verbose`).
///
/// @us-fed-003 @real-io @driving_port @j-003b @habit @wd-42
#[test]
fn federated_query_renders_inline_counter_template_per_peer_row_by_default() {
    todo!("DELIVER (slice-03): pull one peer claim; run graph query --federated; assert stdout contains the literal 'openlore claim counter <peer_cid>' line per peer row with subject/predicate/object pre-filled from the target claim. WD-42 LOCKS this on by default; assert template appears WITHOUT --verbose. Habit-bridging affordance for KPI-FED-3.")
}

// =============================================================================
// US-FED-003 — KPI-FED-2 standalone gate (the zero-merged-rows guardrail)
// =============================================================================

/// FQ-8 (KPI-FED-2 release gate): Across a multi-author multi-record
/// fixture (Maria 1 own + Rachel 3 peer + Tobias 2 peer), every output
/// row from `graph query --federated` MUST have a distinct
/// (author_did, claim_cid) tuple. NO row is labeled "merged" /
/// "consensus" / "aggregate". The number of rows equals the sum of
/// per-author claim counts. Drives integration gate 1
/// (`federation_attribution_preserved`) and the KPI-FED-2 release
/// blocker per outcome-kpis.md.
///
/// @us-fed-003 @real-io @driving_port @j-003a @kpi-fed-1 @kpi-fed-2 @release-gate
#[test]
fn federated_query_no_merged_rows_across_multi_author_multi_record_fixture() {
    todo!("DELIVER (slice-03): seed 1 own + 3 peer-Rachel + 2 peer-Tobias claims about same subject; assert exactly 6 output rows + 6 distinct (author_did, cid) tuples + 3 distinct author headers + zero substring 'merged' / 'consensus' / 'aggregate' in stdout. Mandatory release-blocking gate per KPI-FED-2 + outcome-kpis.md alerting threshold.")
}
