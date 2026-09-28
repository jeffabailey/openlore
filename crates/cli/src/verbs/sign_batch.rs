//! The SHARED `--sign N[,N...]` batch (contributor-philosophy-inference
//! DDD-12): the verb-neutral sequence of individual human-gates that BOTH
//! `scrape github --sign` and `infer people --sign` run. Extracted verbatim
//! from the slice-02 scraper batch so the scraper's behavior is unchanged;
//! the verb supplies pre-filled [`SignableCandidate`]s and its own
//! display-only `derived-from` line.
//!
//! This is the ONLY sign path for candidate batches (single-publish-path
//! invariant, WD-66 / I-SCR-6): compose -> canonicalize -> CID -> sign ->
//! persist -> optional publish via `claim_publish::publish_signed_claim`.

use std::io::Write;

use anyhow::{anyhow, Result};
use claim_domain::{canonicalize, compute_cid, ClaimReference, SignedClaim};

use crate::io::prompt_line;
use crate::verbs::claim_add::{build_unsigned_claim, render_compose_preview, ComposedClaim};
use crate::verbs::claim_publish::{publish_signed_claim, render_publish_success};
use crate::wiring::Wiring;

/// One candidate as the shared compose editor pre-fills it. Verb-neutral:
/// the scraper and person inference both lift their candidates into this.
#[derive(Debug, Clone)]
pub struct SignableCandidate {
    pub subject: String,
    pub predicate: String,
    pub object: String,
    pub evidence: Vec<String>,
    /// The proposed confidence (the human may override it at the prompt).
    pub confidence: f64,
    /// Typed references carried into the signed claim (empty for a new claim;
    /// a STRONGER inference adds `supersedes`).
    pub references: Vec<ClaimReference>,
    /// DISPLAY-ONLY `derived-from` block appended to the compose preview
    /// (already rendered, newline-terminated). Never part of the payload.
    pub derived_from: String,
}

/// Run the `--sign N[,N...]` batch: validate the selection, then walk each
/// selected candidate through its OWN slice-01 compose-sign-publish gesture.
///
/// The selection is validated BEFORE any compose begins (out-of-range /
/// duplicate rejected up front; SS-4 / SS-9). The batch is a SEQUENCE of
/// individual human-gates (US-SCR-005; WD-49 / J-004c) — never a "sign all"
/// bypass — each routed through the SINGLE publish path (no parallel publish;
/// WD-66 / I-SCR-6). Between candidates a running "(k of M signed)" progress
/// line surfaces the batch advancing one conscious signature at a time.
///
/// A candidate may be SKIPPED mid-batch (the human cancels its compose at the
/// sign prompt). A skip is fault-isolated (US-SCR-005 Ex 2): it neither
/// persists a claim nor aborts the remaining selection — the batch proceeds to
/// the next candidate and the final summary reports the signed-vs-skipped
/// tally. The progress line counts ACTUAL signatures, not iterations, so it
/// stays accurate across skips.
///
/// `candidate_list_out` is the already-rendered candidate-list block from
/// `run`; it is emitted to stdout NOW (before the first compose) so the user
/// reviews it before signing.
pub fn sign_selected(
    wiring: &Wiring,
    candidates: &[SignableCandidate],
    raw_selection: &str,
    candidate_list_out: &str,
) -> Result<i32> {
    let selection = parse_selection(raw_selection, candidates.len()).map_err(|e| anyhow!(e))?;
    print!("{candidate_list_out}");
    std::io::stdout().flush()?;

    let total_selected = selection.len();
    let mut signed_count = 0usize;
    let mut skipped_count = 0usize;
    for index in selection {
        // After each signed candidate, announce progress BEFORE the next
        // candidate's compose preview: "(1 of 3 signed)" precedes the next
        // preview, "(2 of 3 signed)" the one after, and so on. A skip does NOT
        // bump this count (it tracks signatures, not attempts).
        if signed_count > 0 {
            println!("\n({signed_count} of {total_selected} signed)");
            std::io::stdout().flush()?;
        }
        // 1-based selection -> 0-based slice access (validated above).
        let candidate = &candidates[index - 1];
        match sign_candidate_via_slice01(wiring, candidate)? {
            SignOutcome::Signed => signed_count += 1,
            SignOutcome::Skipped => {
                // The skip is VISIBLE (named by its 1-based selection index)
                // and NON-FATAL — the loop continues to the next candidate.
                println!("\nskipped candidate {index}");
                std::io::stdout().flush()?;
                skipped_count += 1;
            }
        }
    }

    // Final batch summary — the signed-vs-skipped tally over the whole pass
    // (US-SCR-005 Ex 2). Always emitted for a `--sign` batch so the human has a
    // single closing line confirming what crossed the human-gate.
    println!("\n{signed_count} signed, {skipped_count} skipped");
    std::io::stdout().flush()?;

    Ok(0)
}

/// Whether one candidate's slice-01 compose-sign gesture resulted in a signed
/// claim or was SKIPPED by the human (compose canceled at the sign prompt).
/// Modeling the per-candidate result as a type — rather than a bare `()` —
/// lets the batch loop tally signed-vs-skipped without inspecting I/O state
/// (US-SCR-005 Ex 2; the skip is a first-class outcome, not an error).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SignOutcome {
    Signed,
    Skipped,
}

/// Parse + validate the raw `--sign N[,N...]` selection against the derived
/// candidate count. Returns the 1-based indices in input order, or a
/// domain-shaped error naming EVERY offending value at once. Pure — no I/O —
/// so the rejection happens BEFORE any compose preview (SS-4 / SS-9 pre-compose
/// ordering). SS-1 exercises the single-index happy path; the multi-index +
/// duplicate-rejection paths are pinned by SS-7 / SS-9.
///
/// Validation is APPLICATIVE, not short-circuiting (nw-fp-domain-modeling §8):
/// a single pass over the list accumulates ALL problems — every out-of-range
/// index AND every duplicate index — so the human can fix the whole list in one
/// edit (US-SCR-005 Ex 3) rather than fixing one error at a time. A
/// non-numeric token is a structural parse failure and still short-circuits
/// (the list is unintelligible past that point). The out-of-range message is
/// byte-identical to SS-4's single-index reject so its contract is unchanged.
fn parse_selection(raw: &str, candidate_count: usize) -> Result<Vec<usize>, String> {
    let mut indices = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut errors = Vec::new();
    for token in raw.split(',') {
        let token = token.trim();
        let index: usize = token.parse().map_err(|_| {
            format!("invalid --sign selection {token:?}; expected 1-based candidate indices")
        })?;
        if index == 0 || index > candidate_count {
            errors.push(format!(
                "candidate {index} does not exist; valid range 1..{candidate_count}"
            ));
        } else if !seen.insert(index) {
            // The first occurrence is `insert`ed (returns true); any repeat
            // returns false and is flagged as a duplicate selection.
            errors.push(format!("duplicate candidate index {index}"));
        }
        indices.push(index);
    }
    if errors.is_empty() {
        Ok(indices)
    } else {
        Err(errors.join("; "))
    }
}

/// Carry ONE selected candidate through the SAME slice-01 compose-sign-publish
/// pipeline a hand-authored `claim add` uses (WD-66 / I-SCR-6 — the single
/// publish path). The candidate pre-fills the editable compose fields; the
/// human accepts each (Enter) or overrides, then performs the two-prompt sign
/// (Enter) + publish (Y) gesture. The `derived-from` provenance line is
/// DISPLAY-ONLY (WD-62 / I-SCR-7) — it appears in the preview but is NEVER a
/// signed-payload field, so the signed CID is byte-identical to a
/// hand-authored claim's.
fn sign_candidate_via_slice01(
    wiring: &Wiring,
    candidate: &SignableCandidate,
) -> Result<SignOutcome> {
    let mut stdin = std::io::stdin().lock();
    let mut stdout = std::io::stdout().lock();

    // Pre-fill the editable compose fields from the candidate; the human
    // accepts each default (Enter) or overrides it.
    let subject = prompt_field(&mut stdout, &mut stdin, "subject", &candidate.subject)?;
    let predicate = prompt_field(&mut stdout, &mut stdin, "predicate", &candidate.predicate)?;
    let object = prompt_field(&mut stdout, &mut stdin, "object", &candidate.object)?;
    let evidence_default = candidate.evidence.join(", ");
    let evidence_raw = prompt_field(&mut stdout, &mut stdin, "evidence", &evidence_default)?;
    let evidence: Vec<String> = evidence_raw
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect();
    let confidence = prompt_confidence(&mut stdout, &mut stdin, candidate.confidence)?;

    // Assemble the composed claim — SAME shape `claim add` builds. composed_at
    // from the clock port for testable determinism.
    let composed = ComposedClaim {
        subject,
        predicate,
        object,
        evidence,
        confidence,
        author_did: wiring.identity.author_did().0.clone(),
        composed_at: wiring.clock.now_utc().to_rfc3339(),
        references: candidate.references.clone(),
    };

    // Render the slice-01 compose preview (carries the "not as truth" framing
    // + the WD-10 bucket label) PLUS the DISPLAY-ONLY derived-from provenance
    // line naming the candidate's source signal (WD-62 / I-SCR-7).
    let preview = render_compose_preview(&composed);
    stdout.write_all(preview.as_bytes())?;
    stdout.write_all(candidate.derived_from.as_bytes())?;
    stdout.flush()?;

    // Two-prompt (ADR-003). Enter to sign; the `skip` gesture (or EOF before
    // any input) is a clean cancel of THIS candidate with NO side effects
    // (US-SCR-005 Ex 2; Q-DELIVER-5 — the skip is in-band so a batch can drop
    // one candidate and keep going). A skipped candidate consumes NO publish
    // answer below — it returns immediately.
    let sign_prompt = "\nPress Enter to sign this candidate locally (or type 'skip' to skip it): ";
    let confirmation = prompt_line(&mut stdout, &mut stdin, sign_prompt)?;
    if is_skip_gesture(confirmation.as_deref()) {
        return Ok(SignOutcome::Skipped);
    }

    // Canonicalize -> compute_cid -> sign -> persist. SAME pure-core path
    // `claim add` uses; the derived-from line is NOT folded in (display-only),
    // so the CID is byte-identical to a hand-authored claim's.
    let unsigned = build_unsigned_claim(&composed)?;
    let canonical_bytes =
        canonicalize(&unsigned).map_err(|e| anyhow!("canonicalizing candidate claim: {e}"))?;
    let unsigned_cid = compute_cid(&canonical_bytes);
    writeln!(stdout, "Computing claim CID {}", unsigned_cid.0)?;
    stdout.flush()?;

    let signature = wiring
        .identity
        .sign(&unsigned_cid)
        .map_err(|e| anyhow!("signing candidate claim: {e}"))?;
    let signed = SignedClaim {
        unsigned,
        signature,
    };

    // The signed-from-scraper claim is the user's OWN artifact — own `claims`
    // table + own `claims/<cid>.json`.
    wiring.storage.write_signed_claim(&signed).map_err(|e| {
        anyhow!(
            "persisting signed candidate claim {} to local store: {e:#}",
            signed.signature.signed_cid.0
        )
    })?;
    let artifact_path = wiring
        .paths
        .claims_dir()
        .join(format!("{}.json", signed.signature.signed_cid.0));
    writeln!(
        stdout,
        "Written to local store: {}",
        artifact_path.display()
    )?;
    stdout.flush()?;

    // Second prompt — publish? Y/y publishes via the SINGLE publish path
    // (WD-66 / I-SCR-6); anything else is a clean decline (local artifact
    // stays, no PDS call).
    let publish_prompt = "\nPublish this claim to your PDS now? (y/N): ";
    let publish_answer = prompt_line(&mut stdout, &mut stdin, publish_prompt)?;
    let confirmed_publish = matches!(
        publish_answer.as_deref().map(str::trim),
        Some("y") | Some("Y") | Some("yes") | Some("YES")
    );
    if confirmed_publish {
        drop(stdout);
        drop(stdin);
        // SINGLE publish code path (WD-66 / I-SCR-6) — the SAME helper
        // `claim add`'s Y branch, `claim counter`, and `claim retract` use.
        match publish_signed_claim(wiring, &signed) {
            Ok(publish_outcome) => {
                // The publish prompt above ends without a newline (the user's
                // y/N answer follows it inline). Start the success block on a
                // fresh line so its first line — `Published claim <cid>.` —
                // is recoverable line-by-line (the SS-1 oracle keys off it).
                println!();
                print!("{}", render_publish_success(&publish_outcome));
            }
            Err(err) => {
                eprint!(
                    "{}",
                    crate::verbs::claim_publish::render_publish_error(&err)
                );
                return Err(anyhow!("publishing signed candidate claim failed"));
            }
        }
    } else {
        // Decline (SS-6): the publish prompt was answered with anything other
        // than Y/yes (n / N / Enter / EOF). This is the local-only outcome —
        // the signed claim STAYS on disk (the sign + write above ran first and
        // is NOT rolled back) and NO PDS call is made (KPI-5 local-first). Hint
        // the standalone publish verb naming the exact CID so the human can
        // federate it later at will (`openlore claim publish <cid>`).
        let cid = &signed.signature.signed_cid.0;
        writeln!(
            stdout,
            "\nNot published. Publish it later with: openlore claim publish {cid}"
        )?;
        stdout.flush()?;
    }

    // The candidate cleared the sign prompt and was locally persisted (whether
    // or not the human then published — declining publish, SS-6, is still a
    // signature). So this is a SIGNED outcome for the batch tally.
    Ok(SignOutcome::Signed)
}

/// Whether the sign-prompt answer is the SKIP gesture: the literal `skip` /
/// `s` (case-insensitive, trimmed) OR EOF (`None` — the user closed stdin
/// without typing anything, the canonical "I changed my mind" signal). An
/// empty `Some("")` (a bare Enter) is NOT a skip — it is the affirmative sign
/// gesture. Pure function of the prompt answer (US-SCR-005 Ex 2; Q-DELIVER-5).
fn is_skip_gesture(answer: Option<&str>) -> bool {
    match answer {
        None => true,
        Some(line) => matches!(line.trim().to_ascii_lowercase().as_str(), "skip" | "s"),
    }
}

/// Prompt the user to accept (Enter) or override a pre-filled compose field.
/// An empty line keeps the candidate's pre-filled value; a non-empty line
/// replaces it. The slice-02 compose editor is "pre-fill + edit", so the
/// no-edit path signs the proposal byte-for-byte (SS-2).
fn prompt_field<W: Write, R: std::io::Read>(
    writer: &mut W,
    reader: &mut R,
    label: &str,
    default: &str,
) -> Result<String> {
    let prompt = format!("{label} [{default}]: ");
    match prompt_line(writer, reader, &prompt)? {
        Some(line) if !line.trim().is_empty() => Ok(line.trim().to_string()),
        _ => Ok(default.to_string()),
    }
}

/// Prompt for the confidence field, re-prompting on an out-of-range value
/// (SS-5). An empty line keeps the candidate's conservative default; a valid
/// `[0.0, 1.0]` value overrides it. No claim is written until a valid value is
/// entered (the re-prompt loop runs BEFORE the compose preview).
fn prompt_confidence<W: Write, R: std::io::Read>(
    writer: &mut W,
    reader: &mut R,
    default: f64,
) -> Result<f64> {
    loop {
        let prompt = format!("confidence [{default}]: ");
        match prompt_line(writer, reader, &prompt)? {
            Some(line) if !line.trim().is_empty() => match line.trim().parse::<f64>() {
                Ok(value) if (0.0..=1.0).contains(&value) => return Ok(value),
                _ => {
                    writeln!(writer, "confidence must be between 0.0 and 1.0")?;
                    writer.flush()?;
                }
            },
            _ => return Ok(default),
        }
    }
}
