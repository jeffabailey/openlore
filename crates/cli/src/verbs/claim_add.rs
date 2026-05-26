//! `claim add` — first half of the two-prompt CLI verb (ADR-003).
//!
//! Slice-01 contract (WS-3 + WS-4 in `tests/acceptance/walking_skeleton.rs`):
//!
//! 1. Validate `--confidence` is in `[0.0, 1.0]` — out-of-range is a
//!    pre-sign hard error. WS-4 (step 05-04) pins the user-facing error
//!    text: stderr names the `--confidence` flag AND the range
//!    `[0.0, 1.0]` AND the offending value. NO local file is written
//!    and NO PDS call is made before this check runs (defense-in-depth
//!    on top of LC-5's Lexicon-boundary check).
//! 2. Construct an `UnsignedClaim` value from the flags + the
//!    `ClockPort::now_utc()` for `composed_at`.
//! 3. Render the compose preview to stdout. The preview MUST contain
//!    the literal text `not as truth` per WD-6 (load-bearing UX moment).
//! 4. Print the "Press Enter to sign locally" prompt to stdout.
//! 5. Block reading one line from stdin.
//!    - In scripted mode (`--no-tty` / piped stdin) an empty stdin =
//!      EOF on first read = clean cancel; the binary exits 0 with the
//!      preview shown but no side effect.
//!    - A line with a single Enter (= empty string) confirms the sign
//!      — step 05-06 implements signing; for slice-01 step 05-03 we
//!      simply exit cleanly after the prompt is consumed.
//!
//! LOCAL-FIRST INVARIANT (KPI-5): NO storage write and NO PDS call
//! happen before the user confirms. WS-3 verifies both — there is no
//! claims_dir/ file after this verb exits with empty stdin, and the
//! fake PDS records list is empty.

use std::io::Write;

use anyhow::{anyhow, Result};
use claim_domain::{confidence_bucket, ConfidenceBucket};

use crate::io::{prompt_line, read_one_line};
use crate::wiring::Wiring;

/// Argument struct for the `claim add` verb (mirrors the clap subcommand).
#[derive(Debug, Clone)]
pub struct ClaimAddArgs {
    pub subject: String,
    pub predicate: String,
    pub object: String,
    pub evidence: Vec<String>,
    pub confidence: f64,
}

/// Outcome of one `claim add` invocation. The exit code + stdout chunk
/// the dispatcher emits.
pub struct ClaimAddOutcome {
    pub exit_code: i32,
    pub stdout: String,
}

/// Pure data shape consumed by `render_compose_preview`. Mirrors the
/// fields the user composed; held here (not in `claim_domain::UnsignedClaim`)
/// so the render layer stays decoupled from the canonical-CBOR
/// `UnsignedClaim` shape, which step 05-06 will reach for during sign.
#[derive(Debug, Clone)]
pub struct ComposedClaim {
    pub subject: String,
    pub predicate: String,
    pub object: String,
    pub evidence: Vec<String>,
    pub confidence: f64,
    pub author_did: String,
    /// RFC3339 UTC, produced by `ClockPort::now_utc()` at compose time.
    pub composed_at: String,
}

/// Run the `claim add` verb. Returns once the user has either confirmed
/// the sign prompt (Enter) or canceled (EOF on stdin). Slice-01 step
/// 05-03 stops AFTER the prompt is consumed — signing + persistence are
/// step 05-06; publishing is step 05-08.
///
/// In step 05-03 the verb does NOT write to disk, does NOT contact a
/// PDS, and does NOT sign. It is intentionally a "preview only" half of
/// the two-prompt flow so the WS-3 invariants (no local file, no PDS
/// call before user confirms) hold by construction.
pub fn run(wiring: &Wiring, args: &ClaimAddArgs) -> Result<ClaimAddOutcome> {
    // Step 1: pre-sign confidence-range validation (WS-4 / step 05-04).
    // This runs BEFORE any side effects: no compose preview is rendered,
    // no signing happens, no PDS call is made, no local file is written.
    // The error message names the `--confidence` flag AND the range
    // `[0.0, 1.0]` AND the offending value — these three substrings are
    // load-bearing for WS-4's stderr-contains assertions. This is
    // defense-in-depth on top of LC-5's Lexicon-boundary check; the CLI
    // refuses the value here so users see a friendly error long before
    // the canonical-CBOR layer would reject it anyway.
    if !(0.0..=1.0).contains(&args.confidence) {
        return Err(anyhow!(
            "--confidence must be in [0.0, 1.0]; got {}",
            args.confidence
        ));
    }

    // Step 2: assemble the composed claim. composed_at comes from the
    // clock port for testability — the test harness can pin it later
    // when CID determinism becomes load-bearing (WS-7).
    let composed = ComposedClaim {
        subject: args.subject.clone(),
        predicate: args.predicate.clone(),
        object: args.object.clone(),
        evidence: args.evidence.clone(),
        confidence: args.confidence,
        author_did: wiring.identity.author_did().0.clone(),
        composed_at: wiring.clock.now_utc().to_rfc3339(),
    };

    // Step 3: render the preview into a String (pure function).
    let preview = render_compose_preview(&composed);

    // Step 4 + 5: print the preview and block on the sign prompt.
    // We write directly to stdout (not buffered into the outcome) so
    // the user sees the preview BEFORE the prompt is consumed —
    // important for both interactive and piped-stdin modes.
    {
        let mut stdout = std::io::stdout().lock();
        stdout.write_all(preview.as_bytes())?;
        stdout.flush()?;
    }

    // Sign prompt. Empty stdin (EOF) means "the user canceled before
    // confirming" — we treat that as a clean exit, matching WS-3's
    // "binary either waits for input on stdin OR exits cleanly with
    // the preview shown" contract.
    let sign_prompt = "\nPress Enter to sign locally (or Ctrl-C to cancel): ";
    let mut stdin = std::io::stdin().lock();
    let mut stdout = std::io::stdout().lock();
    let confirmation =
        prompt_line(&mut stdout, &mut stdin, sign_prompt)?;

    if confirmation.is_none() {
        // EOF before any input — user canceled. Exit cleanly; no side
        // effects have happened (KPI-5).
        return Ok(ClaimAddOutcome {
            exit_code: 0,
            stdout: String::new(),
        });
    }

    // The user pressed Enter. Step 05-06 will reach in here to sign +
    // persist; step 05-03 stops at this point with a clean exit so the
    // local-first invariant remains intact.
    //
    // We deliberately do NOT consume an additional line here — that's
    // the publish prompt, which step 05-08 wires.
    let _ = read_one_line(&mut stdin).ok();

    Ok(ClaimAddOutcome {
        exit_code: 0,
        stdout: String::new(),
    })
}

/// Pure function: render the compose preview text. WD-6 mandates the
/// literal substring `not as truth` appears here. The bucket label
/// (`speculative` / `weighted` / `well-evidenced` / `triangulated`) is
/// display-only per WD-10 — it never gets persisted into the signed
/// claim CBOR (that invariant is enforced in step 05-05 / WS-5).
pub fn render_compose_preview(claim: &ComposedClaim) -> String {
    let bucket_label = bucket_to_label(confidence_bucket(claim.confidence));
    let evidence_line = if claim.evidence.is_empty() {
        "(none)".to_string()
    } else {
        claim.evidence.join(", ")
    };

    let mut out = String::new();
    out.push_str("Compose preview (claim is asserted by you, not as truth)\n");
    out.push_str(&format!("  subject:    {}\n", claim.subject));
    out.push_str(&format!("  predicate:  {}\n", claim.predicate));
    out.push_str(&format!("  object:     {}\n", claim.object));
    out.push_str(&format!("  evidence:   {}\n", evidence_line));
    out.push_str(&format!(
        "  confidence: {:.2} ({})\n",
        claim.confidence, bucket_label
    ));
    out.push_str(&format!("  author:     {}\n", claim.author_did));
    out.push_str(&format!("  composedAt: {}\n", claim.composed_at));
    out
}

/// Render the bucket enum into its lowercase display label. The four
/// labels are pinned by WD-10 / WD-6 — anxiety-path scenarios (WS-17)
/// check for the literal `(well-evidenced)` / `(weighted)` substring in
/// stdout, so this mapping is load-bearing.
fn bucket_to_label(bucket: ConfidenceBucket) -> &'static str {
    match bucket {
        ConfidenceBucket::Speculative => "speculative",
        ConfidenceBucket::Weighted => "weighted",
        ConfidenceBucket::WellEvidenced => "well-evidenced",
        ConfidenceBucket::Triangulated => "triangulated",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// WD-6 hard AC: the literal string "not as truth" appears in the
    /// preview. If this ever fails, the compose UX has lost its
    /// load-bearing copy.
    #[test]
    fn render_compose_preview_contains_not_as_truth_literal() {
        let claim = ComposedClaim {
            subject: "github:rust-lang/rust".into(),
            predicate: "embodiesPhilosophy".into(),
            object: "org.openlore.philosophy.memory-safety".into(),
            evidence: vec!["https://www.rust-lang.org/".into()],
            confidence: 0.86,
            author_did: "did:plc:test-jeff".into(),
            composed_at: "2026-05-26T12:00:00+00:00".into(),
        };
        let preview = render_compose_preview(&claim);
        assert!(
            preview.contains("not as truth"),
            "preview must contain literal 'not as truth' per WD-6; got:\n{preview}"
        );
    }

    /// Confidence 0.86 renders with the `well-evidenced` bucket label
    /// in the preview (per WD-10 thresholds + WS-3 fixture value).
    #[test]
    fn render_compose_preview_uses_well_evidenced_label_for_086() {
        let claim = ComposedClaim {
            subject: "github:rust-lang/rust".into(),
            predicate: "embodiesPhilosophy".into(),
            object: "org.openlore.philosophy.memory-safety".into(),
            evidence: vec!["https://www.rust-lang.org/".into()],
            confidence: 0.86,
            author_did: "did:plc:test-jeff".into(),
            composed_at: "2026-05-26T12:00:00+00:00".into(),
        };
        let preview = render_compose_preview(&claim);
        assert!(
            preview.contains("0.86 (well-evidenced)"),
            "expected '0.86 (well-evidenced)' in preview; got:\n{preview}"
        );
    }

    /// Confidence 0.55 renders with the `weighted` bucket label (WS-5
    /// fixture value — pins the boundary mapping the persistence layer
    /// must NOT learn about).
    #[test]
    fn render_compose_preview_uses_weighted_label_for_055() {
        let claim = ComposedClaim {
            subject: "github:mastodon/mastodon".into(),
            predicate: "embodiesPhilosophy".into(),
            object: "org.openlore.philosophy.federation-first".into(),
            evidence: vec!["https://joinmastodon.org/".into()],
            confidence: 0.55,
            author_did: "did:plc:test-jeff".into(),
            composed_at: "2026-05-26T12:00:00+00:00".into(),
        };
        let preview = render_compose_preview(&claim);
        assert!(
            preview.contains("0.55 (weighted)"),
            "expected '0.55 (weighted)' in preview; got:\n{preview}"
        );
    }
}
