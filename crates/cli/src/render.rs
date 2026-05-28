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

/// Inherited slice-01 framing literal (I-7 / WD-6): a claim is asserted by
/// you, NOT as truth. Content-frozen; do NOT paraphrase.
pub const NOT_AS_TRUTH_LITERAL: &str = "not as truth";

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

    fn confidence(value: f64) -> Confidence {
        serde_json::from_value(serde_json::json!(value))
            .expect("test confidence value is well-formed")
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
