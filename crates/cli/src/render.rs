//! `render` — pure-function renderers for verb output blocks.
//!
//! Step 05-11 introduces `render_graph_query_result`: turn a list of
//! `SignedClaim` values (as returned by `StoragePort::query_by_subject`)
//! into a human-friendly stdout block. The renderer is pure (no I/O,
//! no clock, no storage access) so it can be unit-tested without spinning
//! up a wiring.
//!
//! ## Byte-for-byte invariant (KPI-4)
//!
//! The graph-query output prints every field VERBATIM from the SignedClaim
//! serde model — the same model the write path canonicalizes through. No
//! normalization happens here:
//!
//! - `confidence` is rendered as the original `f64` (e.g. `0.86`), NEVER
//!   as a bucket label like `well-evidenced`. Bucket labels are
//!   compose-time display only and MUST NOT appear here (WD-10 / D-12).
//! - `evidence` URLs are printed verbatim, one per line, no
//!   reformatting or scheme normalization.
//! - `composedAt` keeps the exact RFC3339 string the author composed
//!   with, including timezone marker (no conversion to local time).
//! - `author` is the full DID string with verification-method fragment.
//!
//! These invariants make round-trip identity (compose → sign → publish →
//! query) verifiable byte-for-byte at the CLI boundary, which is KPI-4's
//! zero-silent-normalization promise.
//!
//! ## Field label format
//!
//! Each claim renders as a labeled block:
//!
//! ```text
//! subject:     github:rust-lang/rust
//! predicate:   embodiesPhilosophy
//! object:      org.openlore.philosophy.memory-safety
//! evidence:    https://www.rust-lang.org/
//! confidence:  0.86
//! author:      did:plc:test-jeff#org.openlore.application
//! composedAt:  2026-05-25T12:00:00Z
//! cid:         <signed_cid>
//! ```
//!
//! When multiple claims match, blocks are separated by a blank line so
//! awk/grep/cut-style downstream tooling can split on `\n\n`.
//!
//! ## WS-15: retraction annotation (ADR-008 Behavioral rule 3 + WD-11)
//!
//! Per WD-11 "no hard-delete", a retracted claim is preserved verbatim
//! in both the local store and the PDS. The retraction is published as
//! a NEW counter-claim referencing the original. To make the retract
//! VISIBLE without mutating immutable history, the render layer
//! annotates the original claim with the literal string
//! `retracted by author` on its own line at the end of the block.
//!
//! The annotation is content-frozen UX (WD-11) — do NOT paraphrase. The
//! annotation list is computed by the verb via
//! `StoragePort::query_referencing` and passed alongside each claim so
//! the renderer stays pure (no I/O, no storage access).

use claim_domain::{Cid, SignedClaim};
use ports::{AuthorRelationship, FederatedRow};

/// Inherited slice-01 framing literal (I-7 / WD-6): a claim is asserted by
/// you, NOT as truth. Content-frozen; do NOT paraphrase.
pub const NOT_AS_TRUTH_LITERAL: &str = "not as truth";

/// Slice-03 content-frozen no-merge guarantee (ADR-013 footer convention).
/// Printed in the `graph query --federated` footer. Do NOT paraphrase —
/// the exact string is the KPI-FED-2 anti-merging user-visible contract.
pub const NO_MERGE_FOOTER_LITERAL: &str =
    "Each claim is attributed to its author DID. No claims are merged.";

/// Slice-03 content-frozen framing literal for counter-claims: a counter
/// NEVER overwrites its target — both coexist. Pinned by US-FED-004 AC;
/// do NOT paraphrase. The compose preview MUST carry it verbatim.
pub const COUNTER_COEXIST_LITERAL: &str = "counter-claims coexist, never overwrite";

/// Pure data shape the counter-claim compose preview renders. Mirrors the
/// fields the user composed plus the countered target + its author DID, so
/// the render layer stays decoupled from the canonical `UnsignedClaim`.
#[derive(Debug, Clone)]
pub struct ComposedCounterClaim {
    /// The countered target's CID.
    pub target_cid: String,
    /// The bare DID of the target's author (the "peer" being countered).
    pub target_author_did: String,
    /// The NFC-normalized free-text reason (WD-35) — shown verbatim.
    pub reason: String,
    /// The user's own author DID (composing the counter).
    pub author_did: String,
    /// RFC3339 UTC compose timestamp (ClockPort::now_utc()).
    pub composed_at: String,
}

/// Pure function: render the counter-claim compose preview. Three
/// load-bearing contracts (US-FED-004 AC):
///
/// 1. BOTH framing literals appear: the inherited [`NOT_AS_TRUTH_LITERAL`]
///    (I-7) AND the slice-03 [`COUNTER_COEXIST_LITERAL`].
/// 2. The countered target + its author are named on one line:
///    `counters: <target_cid> (by <peer_did>)`.
/// 3. The `--reason` text appears verbatim (NFC-normalized upstream),
///    word-wrapped at 78 columns so the preview stays terminal-friendly.
pub fn render_counter_compose_preview(counter: &ComposedCounterClaim) -> String {
    let mut out = String::new();
    // Framing line 1 — inherited "not as truth" (I-7).
    out.push_str(&format!(
        "Compose preview (a counter-claim is asserted by you, {NOT_AS_TRUTH_LITERAL})\n"
    ));
    // Framing line 2 — slice-03 "counter-claims coexist, never overwrite".
    out.push_str(&format!("  ({COUNTER_COEXIST_LITERAL})\n"));
    // The countered target + its peer author.
    out.push_str(&format!(
        "  counters: {} (by {})\n",
        counter.target_cid, counter.target_author_did
    ));
    out.push_str(&format!("  author:     {}\n", counter.author_did));
    out.push_str(&format!("  composedAt: {}\n", counter.composed_at));
    // The reason, wrapped at 78 cols, shown verbatim under a labeled block.
    out.push_str("  reason:\n");
    for line in wrap_at(&counter.reason, 78) {
        out.push_str(&format!("    {line}\n"));
    }
    out
}

/// Word-wrap `text` to at most `width` columns per line, breaking on ASCII
/// spaces. A single word longer than `width` is emitted on its own line
/// uncut (we never split inside a word — that could corrupt a URL or CID).
/// Pure helper; the reason text is shown verbatim, only line-broken.
fn wrap_at(text: &str, width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current = String::new();
    for word in text.split(' ') {
        if current.is_empty() {
            current.push_str(word);
        } else if current.chars().count() + 1 + word.chars().count() <= width {
            current.push(' ');
            current.push_str(word);
        } else {
            lines.push(std::mem::take(&mut current));
            current.push_str(word);
        }
    }
    if !current.is_empty() || lines.is_empty() {
        lines.push(current);
    }
    lines
}

/// One claim plus the set of CIDs that reference it back-pointer-style.
/// Built by the verb (graph_query) from
/// `StoragePort::query_referencing(claim.signature.signed_cid)` and
/// passed to the renderer so the render layer stays pure.
///
/// The renderer only inspects the boolean `is_retracted` projection —
/// the full reference list lives in the verb in case future slices need
/// finer-grained annotations (e.g. "corrected by ...", "superseded
/// by ..."). Carrying the bool keeps the render-time decision a
/// constant-time check.
#[derive(Debug, Clone)]
pub struct AnnotatedClaim {
    pub claim: SignedClaim,
    /// `true` if any other local claim back-references this CID with
    /// `ReferenceType::Retracts`. Drives the `retracted by author`
    /// annotation per ADR-008 Behavioral rule 3.
    pub is_retracted: bool,
}

/// Render a slice of `SignedClaim` values into the graph-query stdout
/// block. Pure function — no I/O, no clock access.
///
/// Back-compat entry point for callers that don't carry annotation
/// data. Equivalent to passing all claims with `is_retracted = false`.
/// Production callers use [`render_annotated_graph_query_result`] so
/// the WS-15 annotation appears.
pub fn render_graph_query_result(claims: &[SignedClaim]) -> String {
    let annotated: Vec<AnnotatedClaim> = claims
        .iter()
        .cloned()
        .map(|claim| AnnotatedClaim {
            claim,
            is_retracted: false,
        })
        .collect();
    render_annotated_graph_query_result(&annotated)
}

/// Render a slice of `AnnotatedClaim` values into the graph-query
/// stdout block. Pure function — no I/O, no clock access. The
/// annotation decision is precomputed by the verb (see
/// [`AnnotatedClaim::is_retracted`]).
pub fn render_annotated_graph_query_result(annotated: &[AnnotatedClaim]) -> String {
    let mut out = String::new();
    for (idx, ann) in annotated.iter().enumerate() {
        if idx > 0 {
            out.push('\n');
        }
        out.push_str(&render_one_claim(&ann.claim));
        if ann.is_retracted {
            // Content-frozen per WD-11 — exact string is the contract.
            out.push_str("retracted by author\n");
        }
    }
    out
}

/// Render the `graph query --subject <S> --federated` result block: rows
/// from BOTH the user's own `claims` AND `peer_claims`, GROUPED BY author
/// DID. Pure function — no I/O, no storage access.
///
/// ## Anti-merging contract (I-FED-1 layer 3, behavioral — WD-30)
///
/// Each `FederatedRow` carries its `author_did` at the type level
/// (non-Option). This renderer surfaces that attribution per row and
/// NEVER collapses two authors' rows into one aggregate:
///
/// - Rows are grouped under a per-author header (first-seen author order),
///   one header per distinct DID. The header annotates the author's
///   relationship to the local user: `(you)` / `(subscribed peer)` /
///   `(unsubscribed cache)`.
/// - Every claim row prints `author_did`, `confidence`, and `cid` so an
///   operator can attribute any single row to exactly one author.
/// - The footer states the count of distinct authors AND the
///   content-frozen [`NO_MERGE_FOOTER_LITERAL`] (ADR-013). No row is ever
///   labeled "merged" / "consensus" / "aggregate".
pub fn render_federated_query_result(rows: &[FederatedRow]) -> String {
    let groups = group_by_author(rows);

    let mut out = String::new();
    for (author_did, relationship, author_rows) in &groups {
        out.push_str(&format!(
            "author: {} {}\n",
            author_did,
            relationship_annotation(*relationship)
        ));
        for (idx, row) in author_rows.iter().enumerate() {
            if idx > 0 {
                out.push('\n');
            }
            out.push_str(&render_one_federated_row(author_did, row));
        }
        out.push('\n');
    }

    out.push_str(&render_federation_footer(groups.len()));
    out
}

/// Group federated rows by author DID, preserving first-seen author order
/// (so the local user's "(you)" block — typically the `Own` source — keeps
/// a stable position rather than hash-randomized). Returns one entry per
/// distinct DID carrying its `AuthorRelationship` and the rows attributed
/// to it. Pure helper.
fn group_by_author(rows: &[FederatedRow]) -> Vec<(String, AuthorRelationship, Vec<&FederatedRow>)> {
    let mut order: Vec<String> = Vec::new();
    let mut grouped: Vec<(String, AuthorRelationship, Vec<&FederatedRow>)> = Vec::new();
    for row in rows {
        let did = row.author_did.0.clone();
        match order.iter().position(|d| d == &did) {
            Some(pos) => grouped[pos].2.push(row),
            None => {
                order.push(did.clone());
                grouped.push((did, row.author_relationship, vec![row]));
            }
        }
    }
    grouped
}

/// The human-readable relationship annotation appended to a per-author
/// header. Content-frozen per ADR-013 header convention.
fn relationship_annotation(relationship: AuthorRelationship) -> &'static str {
    match relationship {
        AuthorRelationship::You => "(you)",
        AuthorRelationship::SubscribedPeer => "(subscribed peer)",
        AuthorRelationship::UnsubscribedCache => "(unsubscribed cache)",
    }
}

/// Render one federated row. Reuses the slice-01 per-claim field block
/// (subject/predicate/object/evidence/confidence/author/composedAt/cid)
/// and additionally pins the row's `author_did` on its own line so every
/// row is independently attributable (anti-merging behavioral layer).
fn render_one_federated_row(author_did: &str, row: &FederatedRow) -> String {
    let mut out = String::new();
    out.push_str(&format!("  author_did:  {author_did}\n"));
    for line in render_one_claim(&row.signed_claim).lines() {
        out.push_str(&format!("  {line}\n"));
    }
    out
}

/// Render the federation footer: the distinct-author count plus the
/// content-frozen no-merge guarantee. Pure helper.
fn render_federation_footer(author_count: usize) -> String {
    format!(
        "{author_count} author(s). {NO_MERGE_FOOTER_LITERAL}\n"
    )
}

/// Compute the `is_retracted` flag for one CID given the back-reference
/// list `StoragePort::query_referencing` returns. Pure helper kept here
/// (next to the renderer that consumes it) so the projection rule lives
/// in one place; the verb wires storage I/O around it.
pub fn is_retracted_by(_target: &Cid, referencing: &[(Cid, claim_domain::ReferenceType)]) -> bool {
    referencing
        .iter()
        .any(|(_, ref_type)| matches!(ref_type, claim_domain::ReferenceType::Retracts))
}

/// Render one `SignedClaim` as a labeled block. The label widths are
/// padded so the values line up visually; the load-bearing contract is
/// "value text matches compose-time byte-for-byte", not the column
/// alignment.
fn render_one_claim(claim: &SignedClaim) -> String {
    let mut out = String::new();
    out.push_str(&format!("subject:     {}\n", claim.unsigned.subject));
    out.push_str(&format!("predicate:   {}\n", claim.unsigned.predicate));
    out.push_str(&format!("object:      {}\n", claim.unsigned.object));
    out.push_str(&format!(
        "evidence:    {}\n",
        render_evidence(&claim.unsigned.evidence)
    ));
    out.push_str(&format!(
        "confidence:  {}\n",
        render_confidence(&claim.unsigned.confidence)
    ));
    out.push_str(&format!("author:      {}\n", claim.unsigned.author_did.0));
    out.push_str(&format!("composedAt:  {}\n", claim.unsigned.composed_at));
    out.push_str(&format!("cid:         {}\n", claim.signature.signed_cid.0));
    out
}

/// Render the evidence list. Empty -> "(none)" so the line is never
/// orphaned; single entry -> the URL verbatim; multiple entries -> a
/// comma-joined list. URLs are NEVER normalized (no scheme lowering,
/// no trailing-slash stripping, no percent-encode round-trip) — that's
/// the KPI-4 zero-normalization invariant.
fn render_evidence(evidence: &[String]) -> String {
    if evidence.is_empty() {
        "(none)".to_string()
    } else {
        evidence.join(", ")
    }
}

/// Render the confidence field. Goes through serde so we read the
/// original `f64` (the `Confidence` newtype's inner is crate-private to
/// `claim_domain`; its `value()` accessor is a RED-scaffold panic at
/// this slice). `serde_json::to_value` returns a JSON number, which
/// `Display` renders as the minimal decimal representation matching the
/// original `f64` (e.g. `0.86`, not `0.860000`).
///
/// We deliberately do NOT use `{:.2}` formatting here — that would be
/// normalization (forcing 2 decimal places) and would break KPI-4 for
/// values like `0.123456` that the user might legitimately compose with.
fn render_confidence(confidence: &claim_domain::Confidence) -> String {
    serde_json::to_value(confidence)
        .map(|v| v.to_string())
        .unwrap_or_else(|_| "(unrenderable)".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use claim_domain::{Cid, ClaimReference, Confidence, Did, SignatureBlock, UnsignedClaim};
    use ports::SourceTable;
    use proptest::prelude::*;

    fn confidence(value: f64) -> Confidence {
        serde_json::from_value(serde_json::json!(value))
            .expect("test confidence value is well-formed")
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

            // Every row's cid is present exactly once (no row dropped, no
            // row duplicated by the grouping).
            for i in 0..author_indices.len() {
                let cid = format!("bafycid{i:03}");
                let occurrences = rendered.matches(&cid).count();
                prop_assert_eq!(
                    occurrences,
                    1,
                    "cid {} must appear exactly once (no merge / no drop); got {}",
                    cid,
                    occurrences
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
}
