//! `claim add` — the two-prompt CLI verb (ADR-003).
//!
//! Slice-01 contract (WS-3..WS-5 in `tests/acceptance/walking_skeleton.rs`):
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
//!    - A line with a single Enter (= empty string) confirms the sign.
//! 6. (Step 05-05) On confirmation: canonicalize the UnsignedClaim,
//!    compute its CID, sign via `IdentityPort`, and persist via
//!    `StoragePort::write_signed_claim` (DB row + atomic `<cid>.json`
//!    file under claims_dir). Then print "Computing claim CID …" +
//!    "Written to local store: <path>" so downstream WS scenarios can
//!    locate the file. Step 05-08 wires the second prompt + publish.
//!
//! LOCAL-FIRST INVARIANT (KPI-5): NO storage write and NO PDS call
//! happen before the user confirms. WS-3 verifies the no-write half;
//! WS-5 verifies the on-disk file exists after Enter AND contains NO
//! bucket-label string (WD-10 / D-12).

use std::io::Write;

use anyhow::{anyhow, Context, Result};
use claim_domain::{
    canonicalize, compute_cid, confidence_bucket, ClaimReference, ConfidenceBucket,
    Did, SignedClaim, UnsignedClaim,
};

use crate::io::prompt_line;
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

    // The user pressed Enter. Step 05-05: canonicalize + compute_cid +
    // sign + write_signed_claim. The on-disk JSON is the authoritative
    // artifact; the DuckDB row is the queryable index (data-models.md).
    //
    // WD-10 / D-12 invariant: bucket labels live ONLY in the preview
    // render path above (via `confidence_bucket()`). The persistence
    // path goes through `canonicalize` (Lexicon keys + numeric
    // confidence) and serde-JSON serialization of `SignedClaim` —
    // neither imports `confidence_bucket`, so no bucket-label string
    // can leak into the persisted artifact. WS-5 verifies this
    // end-to-end at the subprocess + filesystem boundary.
    let unsigned = build_unsigned_claim(&composed)?;
    let canonical_bytes = canonicalize(&unsigned)
        .map_err(|e| anyhow!("canonicalizing claim: {e}"))?;
    let unsigned_cid = compute_cid(&canonical_bytes);

    writeln!(std::io::stdout(), "Computing claim CID {}", unsigned_cid.0)?;

    let signature = wiring
        .identity
        .sign(&unsigned_cid)
        .map_err(|e| anyhow!("signing claim: {e}"))?;

    let signed = SignedClaim {
        unsigned,
        signature,
    };

    wiring
        .storage
        .write_signed_claim(&signed)
        .with_context(|| {
            format!("persisting signed claim {} to local store", signed.signature.signed_cid.0)
        })?;

    let artifact_path = wiring
        .paths
        .claims_dir()
        .join(format!("{}.json", signed.signature.signed_cid.0));
    writeln!(
        std::io::stdout(),
        "Written to local store: {}",
        artifact_path.display()
    )?;

    // Step 05-08: second prompt — publish to PDS? (ADR-003).
    //
    // The two-prompt contract has two distinct decision points:
    //   1) Enter at the sign prompt = persist locally
    //   2) Y/y at the publish prompt = federate to the PDS
    // Anything else at the publish prompt (n/N/Enter/EOF) is a clean
    // decline — the local artifact stays put, no PDS call is made,
    // exit 0. This is the KPI-5 local-first beat: nothing leaves the
    // machine without explicit user opt-in for THIS claim.
    let publish_prompt = "\nPublish to your PDS now? (y/N): ";
    let publish_answer =
        prompt_line(&mut stdout, &mut stdin, publish_prompt)?;
    let confirmed_publish = matches!(
        publish_answer.as_deref().map(str::trim),
        Some("y") | Some("Y") | Some("yes") | Some("YES")
    );

    if confirmed_publish {
        // Drop the lock before invoking the publish helper so its own
        // stdout writes (the success block) are not double-locked by
        // the dispatcher's later `print!(outcome.stdout)` path.
        drop(stdout);
        drop(stdin);

        // ADR-003 "single publish code path": funnel through
        // `claim_publish::publish_signed_claim` so the standalone
        // verb and the chained Y branch share one implementation.
        // The success-block rendering is the same WD-6 contract in
        // both invocation paths.
        match crate::verbs::claim_publish::publish_signed_claim(wiring, &signed) {
            Ok(publish_outcome) => {
                let rendered =
                    crate::verbs::claim_publish::render_publish_success(&publish_outcome);
                print!("{}", rendered);
            }
            Err(err) => {
                // WS-10 (sad path): the local artifact is already on
                // disk. Surface the PDS error to stderr with a hint
                // pointing at the retry verb so the user can run
                // `openlore claim publish <cid>` later. Return a
                // non-zero exit code so scripts can detect the
                // partial-success state.
                eprintln!("openlore claim add: publish failed: {err:#}");
                return Ok(ClaimAddOutcome {
                    exit_code: 1,
                    stdout: String::new(),
                });
            }
        }
    }

    Ok(ClaimAddOutcome {
        exit_code: 0,
        stdout: String::new(),
    })
}

/// Lift a `ComposedClaim` (CLI-flag shape) into a `claim_domain::UnsignedClaim`
/// (canonical-CBOR-ready shape). Pure transformation — no I/O.
///
/// Confidence routing: the CLI accepts an `f64` and the range check at
/// the top of `run` has already rejected anything outside `[0.0, 1.0]`.
/// We construct `claim_domain::Confidence` via serde because its inner
/// `f64` field is crate-private to `claim_domain` and the smart
/// constructor `Confidence::try_new` is still a RED-scaffold panic at
/// this slice (the wrapper exists for type-safety, not value validation
/// at this slice — phase 03 hardens that). Mirrors the same trick used
/// in `test-support::fixtures::confidence`.
fn build_unsigned_claim(composed: &ComposedClaim) -> Result<UnsignedClaim> {
    let confidence: claim_domain::Confidence =
        serde_json::from_value(serde_json::json!(composed.confidence))
            .map_err(|e| anyhow!("encoding confidence {}: {e}", composed.confidence))?;

    Ok(UnsignedClaim {
        subject: composed.subject.clone(),
        predicate: composed.predicate.clone(),
        object: composed.object.clone(),
        evidence: composed.evidence.clone(),
        confidence,
        author_did: Did(composed.author_did.clone()),
        composed_at: composed.composed_at.clone(),
        references: Vec::<ClaimReference>::new(),
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
