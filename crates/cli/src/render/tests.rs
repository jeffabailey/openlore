use super::*;
use claim_domain::{Cid, ClaimReference, Confidence, Did, SignatureBlock, UnsignedClaim};
use proptest::prelude::*;

fn confidence(value: f64) -> Confidence {
    serde_json::from_value(serde_json::json!(value)).expect("test confidence value is well-formed")
}

/// Build one `scoring::Contribution` for the `--explain` renderer unit
/// tests. Mirrors the shape `scoring::score` emits: a base confidence, the
/// author-distinct multiplier share, an optional triangulation addend, and
/// the subtotal `base * share + triangulation`.
fn contribution(
    author_did: &str,
    cid: &str,
    base: f64,
    author_distinct_bonus: f64,
    triangulation: f64,
) -> scoring::Contribution {
    scoring::Contribution {
        author_did: Did(author_did.to_string()),
        cid: Cid(cid.to_string()),
        base,
        author_distinct_bonus,
        cross_project_triangulation_bonus: triangulation,
        subtotal: base * author_distinct_bonus + triangulation,
    }
}

/// GQE-16 unit (Gate 1 + Gate 2): `render_weighted_explain` enumerates EACH
/// contributing claim under its OWN author DID + cid + base confidence +
/// applied bonus, and the running sum accumulated over the per-claim
/// subtotals EQUALS the displayed adherence weight (reproduce-by-hand). The
/// deno worked example: Tobias 0.55 (x1.0) + Maria 0.40 (x1.25) -> subtotals
/// 0.55 + 0.50 = 1.05 == weight.
#[test]
fn render_weighted_explain_running_sum_equals_displayed_weight_per_author() {
    let contributions = vec![
        contribution("did:plc:tobias-test", "bafytobiasd3no", 0.55, 1.0, 0.0),
        contribution("did:plc:maria-test", "bafymariamz01", 0.40, 1.25, 0.0),
    ];
    // weight == sum of subtotals (the invariant the pure core upholds).
    let weight: f64 = contributions.iter().map(|c| c.subtotal).sum();
    let pairing = scoring::WeightedPairing::new(
        "github:denoland/deno".to_string(),
        "org.openlore.philosophy.dependency-pinning".to_string(),
        weight,
        scoring::WeightBucket::Moderate,
        2,
        2,
        0.55,
        1,
        contributions,
    )
    .expect("non-empty contributions");

    let rendered = render_weighted_explain("org.openlore.philosophy.dependency-pinning", &pairing);

    // Gate 1: EACH contributing claim is enumerated under its OWN author DID
    // + cid + base confidence (no faceless aggregate row).
    assert!(
        rendered.contains("Contribution: did:plc:tobias-test"),
        "expected Tobias's contribution headed by his DID;\n{rendered}"
    );
    assert!(
        rendered.contains("Contribution: did:plc:maria-test"),
        "expected Maria's contribution headed by her DID;\n{rendered}"
    );
    assert!(
        rendered.contains("cid:        bafytobiasd3no")
            && rendered.contains("cid:        bafymariamz01"),
        "expected each contribution to name its own claim cid;\n{rendered}"
    );
    assert!(
        rendered.contains("confidence: 0.55 (base)") && rendered.contains("confidence: 0.4 (base)"),
        "expected each contribution to show its base confidence verbatim;\n{rendered}"
    );

    // Gate 2: each applied bonus is on its own line. Maria's second-author
    // multiplier share (x1.25) is the +0.25 per-add'l-author bonus.
    assert!(
        rendered.contains("author-distinct bonus: x1") // Tobias x1.0 -> "x1"
                && rendered.contains("author-distinct bonus: x1.25"),
        "expected the author-distinct multiplier share shown per claim;\n{rendered}"
    );

    // Gate 2: the per-claim subtotals are visible (0.55, 0.50).
    assert!(
        rendered.contains("subtotal:   0.55") && rendered.contains("subtotal:   0.50"),
        "expected each per-claim subtotal (0.55, 0.50) to be shown;\n{rendered}"
    );

    // Gate 2: the running sum EQUALS the displayed adherence weight (1.05).
    assert!(
        rendered.contains("Running sum 1.05 = displayed adherence weight 1.05"),
        "expected the running sum (0.55 + 0.50 = 1.05) to equal the displayed weight \
             (reproduce-by-hand);\n{rendered}"
    );

    // No claim is merged into a faceless aggregate.
    for label in ["merged", "consensus", "aggregate"] {
        assert!(
            !rendered.to_lowercase().contains(label),
            "the --explain breakdown must contain NO {label:?} row;\n{rendered}"
        );
    }
}

/// GQE-19 unit (Gate 1): the cross-project triangulation addend is rendered
/// on its OWN line attributed to the contribution's author (the one who
/// asserts the object on >= 2 subjects), and folded into the running sum.
#[test]
fn render_weighted_explain_attributes_triangulation_bonus_on_its_own_line() {
    let contributions = vec![contribution(
        "did:plc:rachel-test",
        "bafyrachelcargo",
        0.91,
        1.0,
        0.5,
    )];
    let weight: f64 = contributions.iter().map(|c| c.subtotal).sum();
    let pairing = scoring::WeightedPairing::new(
        "github:rust-lang/cargo".to_string(),
        "org.openlore.philosophy.dependency-pinning".to_string(),
        weight,
        scoring::WeightBucket::Strong,
        1,
        1,
        0.91,
        2,
        contributions,
    )
    .expect("non-empty contributions");

    let rendered = render_weighted_explain("org.openlore.philosophy.dependency-pinning", &pairing);

    assert!(
        rendered.contains("Contribution: did:plc:rachel-test"),
        "expected the contribution attributed to Rachel;\n{rendered}"
    );
    assert!(
        rendered.contains("+0.5 cross-project triangulation"),
        "expected the +0.5 cross-project triangulation addend on its own line;\n{rendered}"
    );
    // 0.91 * 1.0 + 0.5 = 1.41 == weight (reproduce-by-hand).
    assert!(
        rendered.contains("Running sum 1.41 = displayed adherence weight 1.41"),
        "expected the running sum (0.91 + 0.5 = 1.41) to equal the displayed weight;\n{rendered}"
    );
}

/// Build a `FederatedRow` for a given author DID + cid + relationship +
/// source table. The claim body fields are deterministic stand-ins; the
/// federated renderer's contract is about attribution + grouping, not
/// the (already-tested) per-claim field rendering.
fn federated_row(
    author_did: &str,
    cid: &str,
    relationship: AuthorRelationship,
    source_table: SourceTable,
) -> FederatedRow {
    FederatedRow {
        author_did: Did(author_did.to_string()),
        author_relationship: relationship,
        signed_claim: SignedClaim {
            unsigned: UnsignedClaim {
                subject: "github:rust-lang/cargo".to_string(),
                predicate: "embodiesPhilosophy".to_string(),
                object: "org.openlore.philosophy.memory-safety".to_string(),
                evidence: vec!["https://github.com/rust-lang/cargo".to_string()],
                confidence: confidence(0.5),
                author_did: Did(format!("{author_did}#org.openlore.application")),
                composed_at: "2026-05-22T09:18:44Z".to_string(),
                references: Vec::<ClaimReference>::new(),
                reason: None,
            },
            signature: SignatureBlock {
                signed_cid: Cid(cid.to_string()),
                signature_bytes: vec![0u8; 64],
                verification_method: format!("{author_did}#org.openlore.application"),
            },
        },
        source_table,
    }
}

/// Build a `FederatedRow` whose claim carries a single `Counters`
/// reference to `counters_target`. Used to exercise the bidirectional
/// counter annotation (FQ-5): the row is a counter-claim pointing at
/// another row's CID.
fn federated_counter_row(
    author_did: &str,
    cid: &str,
    relationship: AuthorRelationship,
    source_table: SourceTable,
    counters_target: &str,
) -> FederatedRow {
    let mut row = federated_row(author_did, cid, relationship, source_table);
    row.signed_claim.unsigned.references = vec![ClaimReference {
        ref_type: claim_domain::ReferenceType::Counters,
        cid: Cid(counters_target.to_string()),
    }];
    row
}

/// FQ-5 (US-FED-003 AC #8): when one federated row counters another, the
/// renderer annotates BOTH rows bidirectionally — the counter-claim row
/// shows `counters <target_cid> by <peer_did>` and the countered row shows
/// `countered-by <counter_cid> by <author_did>` — and the summary line
/// states the counter-relationship count. The annotation is per-row
/// METADATA computed from the reference graph over the row set; it never
/// merges the two rows (both authors keep their own headers).
#[test]
fn render_federated_query_result_annotates_counter_relationships_bidirectionally() {
    // Rachel's target claim + the local user's counter pointing at it.
    let rows = vec![
        federated_counter_row(
            "did:plc:test-jeff",
            "bafycounter1",
            AuthorRelationship::You,
            SourceTable::Own,
            "bafytarget1",
        ),
        federated_row(
            "did:plc:rachel-test",
            "bafytarget1",
            AuthorRelationship::SubscribedPeer,
            SourceTable::Peer,
        ),
    ];

    let rendered = render_federated_query_result(&rows);

    // Forward: the counter-claim row names what it counters + the target's
    // author DID.
    assert!(
        rendered.contains("counters bafytarget1 by did:plc:rachel-test"),
        "expected the counter-claim row annotated \
             'counters bafytarget1 by did:plc:rachel-test' (forward); got:\n{rendered}"
    );

    // Backward: the countered row names what counters it + that counter's
    // author DID.
    assert!(
        rendered.contains("countered-by bafycounter1 by did:plc:test-jeff"),
        "expected the countered row annotated \
             'countered-by bafycounter1 by did:plc:test-jeff' (backward); got:\n{rendered}"
    );

    // The summary line states the counter-relationship count (exactly 1).
    assert!(
        rendered.contains("1 counter relationship"),
        "expected the summary line to state the counter-relationship count \
             (1 counter relationship); got:\n{rendered}"
    );

    // Anti-merging: both authors keep their own per-author header — the
    // annotation is metadata, never a merge.
    assert!(
        rendered.contains("author: did:plc:test-jeff (you)")
            && rendered.contains("author: did:plc:rachel-test (subscribed peer)"),
        "expected BOTH authors to keep their own headers (no merge); got:\n{rendered}"
    );
}

fn fixture_signed() -> SignedClaim {
    SignedClaim {
        unsigned: UnsignedClaim {
            subject: "github:rust-lang/rust".to_string(),
            predicate: "embodiesPhilosophy".to_string(),
            object: "org.openlore.philosophy.memory-safety".to_string(),
            evidence: vec!["https://www.rust-lang.org/".to_string()],
            confidence: confidence(0.86),
            author_did: Did("did:plc:test-jeff#org.openlore.application".to_string()),
            composed_at: "2026-05-25T12:00:00Z".to_string(),
            references: Vec::<ClaimReference>::new(),
            reason: None,
        },
        signature: SignatureBlock {
            signed_cid: Cid("bafytestcid".to_string()),
            signature_bytes: vec![0u8; 64],
            verification_method: "did:plc:test-jeff#org.openlore.application".to_string(),
        },
    }
}

/// KPI-4: confidence renders as the original f64, not as a bucket
/// label. None of "speculative" / "weighted" / "well-evidenced" /
/// "triangulated" appear in the rendered output.
#[test]
fn render_graph_query_result_never_emits_bucket_label() {
    let rendered = render_graph_query_result(&[fixture_signed()]);
    for label in &["speculative", "weighted", "well-evidenced", "triangulated"] {
        assert!(
            !rendered.contains(label),
            "rendered output contained bucket label '{label}' (WD-10 forbids); got:\n{rendered}"
        );
    }
    assert!(
        rendered.contains("confidence:  0.86"),
        "expected confidence rendered as 0.86; got:\n{rendered}"
    );
}

/// US-FED-004 AC: the counter-claim compose preview carries BOTH
/// framing literals, names the countered target + its peer author, and
/// shows the reason verbatim. Pins the load-bearing compose UX copy
/// without spawning a subprocess.
#[test]
fn render_counter_compose_preview_contains_both_framing_literals_and_target() {
    let counter = ComposedCounterClaim {
        target_cid: "bafytargetcid001".to_string(),
        target_author_did: "did:plc:rachel-test".to_string(),
        reason: "The cited benchmark was retracted by upstream.".to_string(),
        author_did: "did:plc:test-jeff#org.openlore.application".to_string(),
        composed_at: "2026-05-28T09:42:11+00:00".to_string(),
    };
    let preview = render_counter_compose_preview(&counter);
    assert!(
        preview.contains(NOT_AS_TRUTH_LITERAL),
        "preview must contain the inherited 'not as truth' literal (I-7); got:\n{preview}"
    );
    assert!(
        preview.contains(COUNTER_COEXIST_LITERAL),
        "preview must contain the slice-03 'counter-claims coexist, never overwrite' \
             literal; got:\n{preview}"
    );
    assert!(
        preview.contains("counters: bafytargetcid001 (by did:plc:rachel-test)"),
        "preview must name the countered target + its peer author; got:\n{preview}"
    );
    assert!(
        preview.contains("The cited benchmark was retracted by upstream."),
        "preview must show the reason verbatim; got:\n{preview}"
    );
}

/// The reason is word-wrapped at 78 columns: no rendered line of the
/// reason block exceeds 78 chars (plus the 4-space indent), and the
/// full reason survives concatenation (verbatim, only line-broken).
#[test]
fn render_counter_compose_preview_wraps_reason_at_78_cols() {
    let long_reason = "word ".repeat(40);
    let long_reason = long_reason.trim().to_string();
    let counter = ComposedCounterClaim {
        target_cid: "bafytargetcid".to_string(),
        target_author_did: "did:plc:rachel-test".to_string(),
        reason: long_reason.clone(),
        author_did: "did:plc:test-jeff".to_string(),
        composed_at: "2026-05-28T09:42:11+00:00".to_string(),
    };
    let preview = render_counter_compose_preview(&counter);
    // Each reason line (the 4-space-indented ones) <= 78 cols of content.
    for line in preview.lines() {
        if let Some(content) = line.strip_prefix("    ") {
            assert!(
                content.chars().count() <= 78,
                "reason line exceeds 78 cols: {content:?}"
            );
        }
    }
    // The reason words survive verbatim (rejoined across wrap breaks).
    let rejoined: String = preview
        .lines()
        .filter_map(|l| l.strip_prefix("    "))
        .collect::<Vec<_>>()
        .join(" ");
    assert_eq!(rejoined, long_reason, "reason must survive wrap verbatim");
}

/// FQ-1 (behavioral anti-merging, I-FED-1 layer 3): the federated
/// renderer groups rows under ONE header per distinct author DID and
/// emits a footer that states the distinct-author count AND the
/// content-frozen no-merge guarantee, with NO merged/consensus row.
#[test]
fn render_federated_query_result_groups_by_author_with_no_merge_footer() {
    let rows = vec![
        federated_row(
            "did:plc:test-jeff",
            "bafyown1",
            AuthorRelationship::You,
            SourceTable::Own,
        ),
        federated_row(
            "did:plc:rachel-test",
            "bafypeer1",
            AuthorRelationship::SubscribedPeer,
            SourceTable::Peer,
        ),
        federated_row(
            "did:plc:rachel-test",
            "bafypeer2",
            AuthorRelationship::SubscribedPeer,
            SourceTable::Peer,
        ),
    ];

    let rendered = render_federated_query_result(&rows);

    // Two distinct author headers, each annotated with its relationship.
    assert!(
        rendered.contains("author: did:plc:test-jeff (you)"),
        "expected the local user's per-author header annotated '(you)'; got:\n{rendered}"
    );
    assert!(
        rendered.contains("author: did:plc:rachel-test (subscribed peer)"),
        "expected the peer's per-author header annotated '(subscribed peer)'; got:\n{rendered}"
    );

    // Each row carries author_did + confidence + cid (independently
    // attributable — anti-merging behavioral layer).
    for cid in ["bafyown1", "bafypeer1", "bafypeer2"] {
        assert!(
            rendered.contains(cid),
            "expected each row cid {cid} to appear; got:\n{rendered}"
        );
    }
    assert!(
        rendered.contains("author_did:"),
        "expected each row to pin author_did on its own line; got:\n{rendered}"
    );

    // Footer: distinct-author count (2) + content-frozen no-merge text.
    assert!(
        rendered.contains("2 author(s)."),
        "expected the footer to state the distinct-author count (2); got:\n{rendered}"
    );
    assert!(
        rendered.contains(NO_MERGE_FOOTER_LITERAL),
        "expected the content-frozen no-merge footer; got:\n{rendered}"
    );

    // KPI-FED-2 zero-merge gate: no merged/consensus/aggregate label.
    let lower = rendered.to_lowercase();
    for banned in ["merged", "consensus", "aggregate"] {
        // The no-merge footer contains "merged" inside "are merged" —
        // exclude that one legitimate occurrence by checking it does not
        // appear OUTSIDE the footer literal.
        let without_footer = lower.replace(&NO_MERGE_FOOTER_LITERAL.to_lowercase(), "");
        assert!(
            !without_footer.contains(banned),
            "federated render must not label any row {banned:?}; got:\n{rendered}"
        );
    }
}

/// FQ-4 (US-FED-003 AC #7): when ONLY own rows are present (zero peers
/// contributed), the federated renderer degrades gracefully — the own
/// rows still render under the "(you)" header, but the footer is the
/// content-frozen zero-peers hint, NOT the no-merge guarantee. The hint
/// is an exact user-visible string (content-frozen), so an example-based
/// test pins it (golden-string contract — property-framing would not add
/// coverage over a single literal).
#[test]
fn render_federated_query_result_emits_zero_peers_hint_when_no_peer_rows() {
    let rows = vec![federated_row(
        "did:plc:test-jeff",
        "bafyown1",
        AuthorRelationship::You,
        SourceTable::Own,
    )];

    let rendered = render_federated_query_result(&rows);

    // The own claim still renders under its "(you)" header — degradation
    // never swallows the local rows.
    assert!(
        rendered.contains("author: did:plc:test-jeff (you)"),
        "expected the own claim to render under its '(you)' header; got:\n{rendered}"
    );
    assert!(
        rendered.contains("bafyown1"),
        "expected the own claim cid to render; got:\n{rendered}"
    );

    // The footer is the content-frozen zero-peers hint VERBATIM.
    assert!(
        rendered.contains(NO_PEERS_FOOTER_LITERAL),
        "expected the content-frozen zero-peers hint footer; got:\n{rendered}"
    );

    // And the no-merge guarantee footer is NOT emitted on the degraded
    // path — the two footers are mutually exclusive.
    assert!(
        !rendered.contains(NO_MERGE_FOOTER_LITERAL),
        "expected the no-merge footer to be ABSENT when zero peers contributed; got:\n{rendered}"
    );
    assert!(
            !rendered.contains("author(s)."),
            "expected NO distinct-author-count footer on the zero-peers degraded path; got:\n{rendered}"
        );
}

/// FQ-7 (WD-42 — habit-bridging affordance, KPI-FED-3): every PEER row
/// in the federated render carries an inline copy-pasteable counter
/// template pre-filled with the target claim's CID, subject, predicate,
/// and object, shown BY DEFAULT (no `--verbose` gate at the render layer
/// — the renderer always emits it). OWN rows do NOT get a template (you
/// don't counter your own claim). The template count equals the peer-row
/// count. The exact template prefix is content-frozen UX copy, so an
/// example-based test pins the literal — property-framing would not add
/// coverage over a fixed string.
#[test]
fn render_federated_query_result_emits_inline_counter_template_per_peer_row_only() {
    let rows = vec![
        federated_row(
            "did:plc:test-jeff",
            "bafyown1",
            AuthorRelationship::You,
            SourceTable::Own,
        ),
        federated_row(
            "did:plc:rachel-test",
            "bafypeer1",
            AuthorRelationship::SubscribedPeer,
            SourceTable::Peer,
        ),
        federated_row(
            "did:plc:rachel-test",
            "bafypeer2",
            AuthorRelationship::SubscribedPeer,
            SourceTable::Peer,
        ),
    ];

    let rendered = render_federated_query_result(&rows);

    // Each PEER row carries an inline template naming its CID + pre-filled
    // subject/predicate/object from the target claim (the `federated_row`
    // fixture uses subject github:rust-lang/cargo, predicate
    // embodiesPhilosophy, object org.openlore.philosophy.memory-safety).
    for cid in ["bafypeer1", "bafypeer2"] {
        let expected = format!(
            "openlore claim counter {cid} --reason \"...\" \
                 --subject github:rust-lang/cargo --predicate embodiesPhilosophy \
                 --object org.openlore.philosophy.memory-safety"
        );
        assert!(
            rendered.contains(&expected),
            "expected an inline counter template for peer row {cid}; got:\n{rendered}"
        );
    }

    // The OWN row gets NO template — its CID never follows `counter `.
    assert!(
        !rendered.contains("openlore claim counter bafyown1"),
        "own row must NOT get a counter template (WD-42 own-rows-excluded); got:\n{rendered}"
    );

    // Exactly one template per peer row (2 peers → 2 templates).
    assert_eq!(
        rendered.matches("openlore claim counter ").count(),
        2,
        "expected exactly one template per peer row (2 peer rows); got:\n{rendered}"
    );
}

proptest! {
    /// Property (Modeling / Generalizing, Hebert ch.3): for ANY set of
    /// federated rows over an arbitrary author-DID alphabet, the number
    /// of per-author headers the renderer emits equals the number of
    /// DISTINCT author DIDs in the input, and the footer count agrees.
    /// This is the anti-merging invariant generalized: rows never
    /// collapse across authors, and authors never split into phantom
    /// extra headers.
    #[test]
    fn render_federated_groups_exactly_one_header_per_distinct_author(
        author_indices in prop::collection::vec(0usize..4, 1..12),
    ) {
        // Map the generated indices onto a small DID alphabet so the
        // distinct-author count is controllable + verifiable.
        let alphabet = [
            "did:plc:author-a",
            "did:plc:author-b",
            "did:plc:author-c",
            "did:plc:author-d",
        ];
        let rows: Vec<FederatedRow> = author_indices
            .iter()
            .enumerate()
            .map(|(i, &idx)| {
                federated_row(
                    alphabet[idx],
                    &format!("bafycid{i:03}"),
                    AuthorRelationship::SubscribedPeer,
                    SourceTable::Peer,
                )
            })
            .collect();

        let distinct: std::collections::HashSet<usize> =
            author_indices.iter().copied().collect();
        let expected_authors = distinct.len();

        let rendered = render_federated_query_result(&rows);

        // One header line per distinct author.
        let header_count = rendered
            .lines()
            .filter(|l| l.starts_with("author: "))
            .count();
        prop_assert_eq!(
            header_count,
            expected_authors,
            "expected exactly {} author headers; got {}\n{}",
            expected_authors,
            header_count,
            rendered
        );

        // Footer count agrees with the distinct-author cardinality.
        prop_assert!(
            rendered.contains(&format!("{expected_authors} author(s).")),
            "footer must state distinct-author count {}; got:\n{}",
            expected_authors,
            rendered
        );

        // Every row's cid appears as exactly ONE `cid:` field line (no
        // row dropped, no row duplicated by the grouping). We count the
        // canonical `cid:` field line — NOT raw substring — because each
        // PEER row now also names its cid inside the FQ-7 inline counter
        // template (WD-42), so a raw substring count is 2 per peer row by
        // design. The row-identity invariant is "one cid: field per row".
        for i in 0..author_indices.len() {
            let cid = format!("bafycid{i:03}");
            let cid_field_occurrences = rendered
                .lines()
                .filter(|l| {
                    l.trim_start().starts_with("cid:") && l.trim_end().ends_with(&cid)
                })
                .count();
            prop_assert_eq!(
                cid_field_occurrences,
                1,
                "cid {} must appear as exactly one `cid:` field line (no merge / no drop); got {}",
                cid,
                cid_field_occurrences
            );
        }
    }
}

/// Every compose-time field appears in the output byte-for-byte.
#[test]
fn render_graph_query_result_contains_all_fields_verbatim() {
    let claim = fixture_signed();
    let rendered = render_graph_query_result(&[claim]);
    for expected in &[
        "github:rust-lang/rust",
        "embodiesPhilosophy",
        "org.openlore.philosophy.memory-safety",
        "https://www.rust-lang.org/",
        "did:plc:test-jeff#org.openlore.application",
        "2026-05-25T12:00:00Z",
        "bafytestcid",
    ] {
        assert!(
            rendered.contains(expected),
            "expected rendered output to contain {expected:?}; got:\n{rendered}"
        );
    }
}

/// The empty-`--object` renderer (GQE-4 / US-GRAPH-001 Example 4) names the
/// queried object and, when a near-match is supplied, appends the
/// content-frozen "Did you mean ...?" suggestion — and NEVER manufactures a
/// per-claim row. Pins the user-visible empty-result copy without a
/// subprocess. Example-based: the message is an exact golden string.
#[test]
fn render_object_query_empty_with_suggestion_names_object_and_near_match() {
    let missed = "org.openlore.philosophy.dependancy-pinning";
    let near = "org.openlore.philosophy.dependency-pinning";

    let rendered = render_object_query_grouped_by_subject(missed, &[], Some(near));
    assert!(
        rendered.contains(&format!(
            "No claims found for object {missed}. Did you mean {near}?"
        )),
        "expected the no-claims line to name the queried object + the near-match; got:\n{rendered}"
    );

    // Without a near-match, the bare no-claims line is emitted (no dangling
    // "Did you mean").
    let bare = render_object_query_grouped_by_subject(missed, &[], None);
    assert!(
        bare.contains(&format!("No claims found for object {missed}.")),
        "expected the bare no-claims line to name the queried object; got:\n{bare}"
    );
    assert!(
        !bare.contains("Did you mean"),
        "expected NO suggestion clause when no near-match exists; got:\n{bare}"
    );

    // Empty is honest: neither rendering manufactures a per-claim cid row.
    for out in [&rendered, &bare] {
        assert!(
            !out.lines().any(|l| l.trim_start().starts_with("cid:")),
            "empty --object render must NOT manufacture a cid row; got:\n{out}"
        );
    }
}

proptest! {
    /// Property (Modeling / Generalizing, Hebert ch.3) — the suggestion
    /// ranker's correctness contract: for ANY existing philosophy URI and
    /// ANY single-edit typo of it (transposition / substitution / deletion /
    /// insertion over the philosophy-URI alphabet), the correct URI is among
    /// the candidate neighbours `single_edit_neighbours(typo)` enumerates.
    /// That is the invariant the verb's probe loop relies on: the closest
    /// EXISTING object is always reachable as a single-edit neighbour, so a
    /// one-character typo always recovers its near-match. The original typo
    /// is NEVER its own neighbour (it already came back empty).
    #[test]
    fn single_edit_neighbours_recovers_the_correct_object_from_any_one_char_typo(
        // A realistic philosophy suffix over the URI alphabet, length 4..24.
        suffix in "[a-z][a-z0-9-]{3,23}",
        edit_pos in 0usize..24,
    ) {
        let correct = format!("org.openlore.philosophy.{suffix}");
        let correct_chars: Vec<char> = correct.chars().collect();
        // Build a single-substitution typo at a position inside the suffix
        // (guaranteed in-range + a guaranteed-different replacement char).
        let prefix_len = "org.openlore.philosophy.".chars().count();
        let pos = prefix_len + (edit_pos % suffix.chars().count());
        let original = correct_chars[pos];
        let replacement = if original == 'x' { 'y' } else { 'x' };
        let mut typo_chars = correct_chars.clone();
        typo_chars[pos] = replacement;
        let typo: String = typo_chars.into_iter().collect();

        prop_assume!(typo != correct);

        let neighbours = single_edit_neighbours(&typo);

        // The correct URI is recoverable as a single-edit neighbour of the typo.
        prop_assert!(
            neighbours.iter().any(|n| n == &correct),
            "expected single_edit_neighbours({typo:?}) to contain the correct URI {correct:?}"
        );
        // The typo itself is never emitted as its own neighbour.
        prop_assert!(
            !neighbours.iter().any(|n| n == &typo),
            "single_edit_neighbours must never emit the original string as a neighbour"
        );
    }
}

// -------------------------------------------------------------------------
// Slice-05 (AV-8 / I-AV-2) — the network-search renderer anti-merging property
// -------------------------------------------------------------------------

/// Build one raw attributed network row for the renderer property. Distinct
/// CIDs keep each generated row a distinct multiset member.
fn raw_network_row(author_did: &str, cid: &str, confidence: f64) -> NetworkResultRowRaw {
    NetworkResultRowRaw {
        author_did: Did(author_did.to_string()),
        cid: Cid(cid.to_string()),
        subject: "github:bazelbuild/bazel".to_string(),
        predicate: "embodiesPhilosophy".to_string(),
        object: "org.openlore.philosophy.reproducible-builds".to_string(),
        confidence,
        composed_at: chrono::Utc::now(),
        verified_against: claim_domain::KeyId(format!("{author_did}#org.openlore.application")),
        evidence: vec![format!("https://example.test/e/{cid}")],
        references: Vec::new(),
    }
}

/// A strategy over small attributed result sets: 1..=12 rows, each with an
/// author drawn from a small DID pool (so identical-content distinct-author
/// collisions occur) and a UNIQUE cid (rows stay distinct multiset members).
fn arb_network_rows() -> impl Strategy<Value = Vec<NetworkResultRowRaw>> {
    prop::collection::vec((0usize..5, 0.0f64..=1.0), 1..=12).prop_map(|specs| {
        specs
            .into_iter()
            .enumerate()
            .map(|(idx, (author_idx, conf))| {
                raw_network_row(
                    &format!("did:plc:author{author_idx}#org.openlore.application"),
                    &format!("bafycid{idx}"),
                    conf,
                )
            })
            .collect()
    })
}

proptest! {
    /// AV-8 / I-AV-2 anti-merging RENDER property (the inner-loop decomposition
    /// of the AT's "NO row collapses multiple authors"): for ANY set of
    /// attributed rows, the renderer emits EXACTLY one output `author_did:` row
    /// per input row (no row dropped, no rows merged), every output row carries
    /// the `[verified]` marker, the footer's distinct-author count equals the
    /// number of distinct authors in the input, and NO merged/consensus row
    /// ever appears. The relationship resolver is the identity-unfollowed
    /// closure (the relationship label is orthogonal to the anti-merging
    /// invariant under test).
    #[test]
    fn render_network_search_emits_one_row_per_author_never_merges(
        rows in arb_network_rows()
    ) {
        let result = NetworkSearchResultRaw {
            distinct_author_count: 0, // recomputed by the renderer; not read here
            total_claims: rows.len() as u32,
            results: rows.clone(),
            suggestion: None,
        };
        let unfollowed = |_did: &str| AuthorRelationship::NetworkUnfollowed;

        let rendered =
            render_network_search_result(SearchDimension::Object, &result, &unfollowed);

        // 1. Exactly one output `author_did:` row per input row — no row dropped,
        //    no two rows collapsed onto one.
        let output_rows = rendered
            .lines()
            .filter(|l| l.trim_start().starts_with("author_did:"))
            .count();
        prop_assert_eq!(
            output_rows,
            rows.len(),
            "every attributed input row must render as exactly one output row \
             (anti-merging, I-AV-2); rendered:\n{}",
            rendered
        );

        // 2. Every output row carries the `[verified]` marker (I-AV-1).
        let verified_markers = rendered.matches(VERIFIED_MARKER).count();
        prop_assert_eq!(
            verified_markers,
            rows.len(),
            "every output row must carry the [verified] marker; rendered:\n{}",
            rendered
        );

        // 3. The footer's distinct-author count == the distinct authors in input
        //    (a COUNT over attributed rows, never a merge).
        let distinct_authors: std::collections::HashSet<&str> =
            rows.iter().map(|r| r.author_did.0.as_str()).collect();
        prop_assert!(
            rendered.contains(&format!("{} distinct author(s).", distinct_authors.len())),
            "footer must state the distinct-author count {}; rendered:\n{}",
            distinct_authors.len(),
            rendered
        );

        // 4. NO merged/consensus row ever appears (the cardinal anti-merging gate).
        let lowered = rendered.to_ascii_lowercase();
        for banned in &["authors agree", "the network says", "the network thinks"] {
            prop_assert!(
                !lowered.contains(banned),
                "no merged/consensus row may appear (found {:?}); rendered:\n{}",
                banned,
                rendered
            );
        }
    }

    /// AV-23 / US-AV-004 `--show` trust-inspection RENDER property (the
    /// inner-loop decomposition of the AT's "full record + Signature-VERIFIED +
    /// CID-recomputed lines"): for ANY verified attributed row, the `--show`
    /// view (a) prints the full record fields verbatim (subject / object /
    /// confidence / evidence / author DID), (b) prints
    /// `Signature: VERIFIED against <bare-did>` rendered from the row's STORED
    /// `verified_against` (the ingest verification result — no second path,
    /// US-AV-004 Technical Notes), and (c) prints
    /// `CID: <cid> (recomputed, matches published record)` naming the row's cid.
    /// The author pool produces both followed-shape + bare DIDs so the
    /// fragment-stripping in the signature line is exercised across inputs.
    #[test]
    fn render_show_surfaces_full_record_plus_stored_verification(
        author_idx in 0usize..5,
        cid_idx in 0usize..1000,
        confidence in 0.0f64..=1.0,
    ) {
        let author_did = format!("did:plc:author{author_idx}");
        let cid = format!("bafyshow{cid_idx}");
        // `verified_against` carries the fragment form (the ingest-stored shape);
        // the signature line must surface the BARE did.
        let row = raw_network_row(
            &format!("{author_did}#org.openlore.application"),
            &cid,
            confidence,
        );

        let rendered = render_show_verification_line(&row);

        // (a) The full record fields appear (surfaced for the trust inspection).
        prop_assert!(
            rendered.contains(&format!("subject:     {}", row.subject)),
            "--show must surface the full-record subject; rendered:\n{rendered}"
        );
        prop_assert!(
            rendered.contains(&row.object),
            "--show must surface the record object; rendered:\n{rendered}"
        );
        prop_assert!(
            rendered.contains(&format!("author:      {}", row.author_did.0)),
            "--show must surface the record author DID; rendered:\n{rendered}"
        );
        // The numeric confidence (rendered verbatim, never a bucket-only label).
        prop_assert!(
            rendered.contains(&render_candidate_confidence(confidence)),
            "--show must surface the numeric confidence; rendered:\n{rendered}"
        );

        // (b) The Signature-VERIFIED line, rendered from the STORED
        // `verified_against` (no second path), with the fragment stripped to the
        // bare DID.
        prop_assert!(
            rendered.contains(&format!("Signature: VERIFIED against {author_did}")),
            "--show must surface 'Signature: VERIFIED against {author_did}' from the \
             stored verified_against (no second path); rendered:\n{rendered}"
        );
        prop_assert!(
            !rendered.contains("#org.openlore.application\nCID:")
                && rendered.contains(&format!(
                    "Signature: VERIFIED against {author_did}\n"
                )),
            "the signature line must show the BARE DID (fragment stripped); \
             rendered:\n{rendered}"
        );

        // (c) The CID-recomputed-matches line names the row's cid.
        prop_assert!(
            rendered.contains(&format!(
                "CID: {cid} (recomputed, matches published record)"
            )),
            "--show must surface 'CID: {cid} (recomputed, matches published record)'; \
             rendered:\n{rendered}"
        );
    }
}

// -------------------------------------------------------------------------
// Slice-05 (AV-15 / US-AV-003 / KPI-AV-1) — the contributor-trail honest-
// framing footer (the inner-loop decomposition of the AT's "footer = a
// developer's reasoning trail, not a community consensus + peer add <did>").
// -------------------------------------------------------------------------

/// AV-15 / KPI-AV-1: the Contributor dimension footer frames the result as
/// ONE developer's reasoning trail — NOT a community consensus — and offers
/// the slice-03 `openlore peer add <author-did>` follow path naming the trail's
/// bare author DID. The OBJECT-dimension "N distinct author(s)." count footer
/// must NOT appear (a single-author trail is never a multi-author survey).
///
/// bypass: content-frozen footer strings (exact phrasing is the KPI-AV-1
/// honesty contract); the anti-merging/marker INVARIANTS are covered
/// generatively by `render_network_search_emits_one_row_per_author_never_merges`.
#[test]
fn render_contributor_trail_footer_frames_one_developer_not_consensus() {
    let author = "did:plc:priya-test#org.openlore.application";
    let rows = vec![
        raw_network_row(author, "bafycid0", 0.82),
        raw_network_row(author, "bafycid1", 0.79),
    ];
    let result = NetworkSearchResultRaw {
        distinct_author_count: 1,
        total_claims: rows.len() as u32,
        results: rows,
        suggestion: None,
    };
    let unfollowed = |_did: &str| AuthorRelationship::NetworkUnfollowed;

    let rendered = render_network_search_result(SearchDimension::Contributor, &result, &unfollowed);

    // The honest-framing footer (a trail, NOT a consensus — KPI-AV-1).
    assert!(
        rendered.contains("one developer's reasoning trail, not a community consensus"),
        "the contributor footer must frame the result as a trail, NOT a consensus:\n{rendered}"
    );
    // The slice-03 follow offer naming the trail's BARE author DID.
    assert!(
        rendered.contains("openlore peer add did:plc:priya-test"),
        "the contributor footer must offer `openlore peer add <bare-did>`:\n{rendered}"
    );
    // The OBJECT-dimension distinct-author COUNT footer must NOT leak into the
    // single-author contributor trail.
    assert!(
        !rendered.contains("distinct author(s)."),
        "the contributor footer must NOT be the object-dimension count footer:\n{rendered}"
    );
}

// -------------------------------------------------------------------------
// Slice-05 (US-AV-006 / I-AV-8 / KPI-AV-6) — `--share` query-encoding link
// -------------------------------------------------------------------------

/// AV-26 (US-AV-006 Ex1): the OBJECT-dimension `--share` link emits the exact
/// `Shareable link: openlore://search?object=<value>` affordance line PLUS the
/// "encodes the query, not a frozen snapshot" semantics line. The grammar is
/// pinned here so the user-visible affordance is byte-stable; the no-snapshot
/// INVARIANT (no result payload leaks into the link, for ANY value/dimension)
/// is covered generatively by `render_share_link_encodes_query_never_snapshot`.
///
/// bypass: exact affordance/semantics strings are the user-visible contract
/// (a single-example assertion on a content-frozen line; the invariant lives
/// in the property below).
#[test]
fn render_share_link_object_emits_link_and_query_not_snapshot_semantics() {
    let object = "org.openlore.philosophy.reproducible-builds";

    let rendered = render_share_link(SearchDimension::Object, object);

    // Criterion 1: the exact `Shareable link:` affordance encoding the query.
    assert!(
            rendered.contains(&format!("Shareable link: openlore://search?object={object}")),
            "the object share link must read `Shareable link: openlore://search?object=<value>`:\n{rendered}"
        );
    // Criterion 2: the sharing semantics — the link encodes the QUERY, not a
    // frozen snapshot (US-AV-006 Ex1).
    assert!(
        rendered.contains("encodes the query, not a") && rendered.contains("snapshot"),
        "the share output must state the link encodes the query, NOT a snapshot:\n{rendered}"
    );
}

proptest! {
    /// Property (Invariant, Hebert ch.3): for ANY dimension + ANY value over a
    /// philosophy/DID/project character alphabet, the `--share` link encodes
    /// EXACTLY that `<dimension>=<value>` query and carries NO result-payload /
    /// snapshot token — the link encodes the QUERY, never a frozen result set
    /// (I-AV-8 / KPI-AV-6). This is the query-encoding-not-snapshot invariant:
    /// no author_did, no [verified], no cid, no confidence, no second `&`
    /// parameter ever leaks into the link.
    #[test]
    fn render_share_link_encodes_query_never_snapshot(
        // A philosophy/DID/project-shaped value alphabet that, by construction,
        // cannot itself spell a banned snapshot token (no letters that form
        // `cid`/`confidence`/`results`/`snapshot`/`verified`/`author_did`) — so
        // a banned token in the link can ONLY come from a payload leak, never
        // from the queried value.
        value in "[ab09.:/-]{1,40}",
        which in 0u8..3,
    ) {
        let dimension = match which {
            0 => SearchDimension::Object,
            1 => SearchDimension::Contributor,
            _ => SearchDimension::Subject,
        };
        let flag = match dimension {
            SearchDimension::Object => "object",
            SearchDimension::Contributor => "contributor",
            SearchDimension::Subject => "subject",
        };

        let rendered = render_share_link(dimension, &value);

        // The link is present and encodes EXACTLY this dimension+value.
        let expected_link = format!("openlore://search?{flag}={value}");
        prop_assert!(
            rendered.contains(&expected_link),
            "expected the link `{expected_link}` in:\n{rendered}"
        );

        // Extract the link's query string and assert NO snapshot payload leaks.
        let link = rendered
            .split_whitespace()
            .find(|t| t.starts_with("openlore://search?"))
            .expect("a share link is emitted");
        let query = link
            .strip_prefix("openlore://search?")
            .expect("link carries the share prefix");
        // EXACTLY one query parameter (no `&`-joined snapshot fields).
        prop_assert!(
            !query.contains('&'),
            "the link must encode a single dimension=value, never a multi-field snapshot: {query}"
        );
        for token in ["author_did", "[verified]", "cid", "confidence", "results", "snapshot"] {
            prop_assert!(
                !query.contains(token),
                "the link query must NOT carry a snapshot token `{token}`: {query}"
            );
        }
    }
}
