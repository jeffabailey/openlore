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
use claim_domain::{
    canonicalize, compute_cid, reference_rules_validate, ClaimLookup, ClaimReference, SignedClaim,
};

use crate::io::prompt_line;
use crate::verbs::claim_add::{build_unsigned_claim, render_compose_preview, ComposedClaim};
use crate::verbs::claim_publish::{publish_signed_claim, render_publish_success};
use crate::verbs::StorageClaimLookup;
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

/// What a listing verb hands back to the dispatcher: its exit code and the
/// stdout chunk still to print (empty after a `--sign` batch, which prints
/// the listing itself before the first compose).
#[derive(Debug)]
pub struct ListOrSignOutcome {
    pub exit_code: i32,
    pub stdout: String,
}

/// The shared tail of every candidate-listing verb: WITHOUT `--sign` return
/// the rendered listing untouched (zero writes); WITH `--sign` run the batch
/// over `candidates` — the SAME numbering the listing shows (UC-8).
pub fn list_or_sign(
    wiring: &Wiring,
    candidates: &[SignableCandidate],
    raw_selection: Option<&str>,
    rendered_list: String,
) -> Result<ListOrSignOutcome> {
    let Some(raw_selection) = raw_selection else {
        return Ok(ListOrSignOutcome {
            exit_code: 0,
            stdout: rendered_list,
        });
    };
    let exit_code = sign_selected(wiring, candidates, raw_selection, &rendered_list)?;
    Ok(ListOrSignOutcome {
        exit_code,
        stdout: String::new(),
    })
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
    let selection = selected_candidates(candidates, raw_selection).map_err(|e| anyhow!(e))?;
    print!("{candidate_list_out}");
    std::io::stdout().flush()?;

    let total_selected = selection.len();
    let mut signed_count = 0usize;
    let mut skipped_count = 0usize;
    for (index, candidate) in selection {
        // After each signed candidate, announce progress BEFORE the next
        // candidate's compose preview: "(1 of 3 signed)" precedes the next
        // preview, "(2 of 3 signed)" the one after, and so on. A skip does NOT
        // bump this count (it tracks signatures, not attempts).
        if signed_count > 0 {
            println!("\n({signed_count} of {total_selected} signed)");
            std::io::stdout().flush()?;
        }
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

/// Resolve the raw `--sign N[,N...]` selection to the candidates it names,
/// in selection order, each paired with its 1-based number. Pure: selection
/// `i` is exactly the `i`-th candidate of the SAME list the verb rendered
/// (US-CPI-003 / UC-8 — list and sign share one numbering), and an invalid
/// selection is refused here, before any compose begins.
fn selected_candidates<'a>(
    candidates: &'a [SignableCandidate],
    raw_selection: &str,
) -> Result<Vec<(usize, &'a SignableCandidate)>, String> {
    let selection = parse_selection(raw_selection, candidates.len())?;
    // 1-based selection -> 0-based slice access (validated by the parser).
    Ok(selection
        .into_iter()
        .map(|index| (index, &candidates[index - 1]))
        .collect())
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
    // The shipped reference rules (self-reference / two-hop cycle) gate any
    // reference a candidate carries — a STRONGER inference's `supersedes` —
    // BEFORE signing. No references: the rules have nothing to check.
    let lookup = StorageClaimLookup {
        storage: wiring.storage.as_ref(),
    };
    reference_rules_validate(&unsigned, Some(&lookup as &dyn ClaimLookup))
        .map_err(|e| anyhow!("reference rules rejected the candidate claim: {e}"))?;
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
            Some(line) if !line.trim().is_empty() => match parse_confidence(&line) {
                Ok(value) => return Ok(value),
                Err(refusal) => {
                    writeln!(writer, "{refusal}")?;
                    writer.flush()?;
                }
            },
            _ => return Ok(default),
        }
    }
}

/// Parse a typed confidence override: a number within `[0.0, 1.0]`, or a
/// refusal naming the rejected text (US-CPI-003 IS-7). Pure — the prompt loop
/// re-asks on `Err`, so nothing is signed until a valid value is given.
fn parse_confidence(raw: &str) -> Result<f64, String> {
    let typed = raw.trim();
    typed
        .parse::<f64>()
        .ok()
        .filter(|value| (0.0..=1.0).contains(value))
        .ok_or_else(|| format!("confidence must be between 0.0 and 1.0 (got {typed})"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn candidate_numbered(number: usize) -> SignableCandidate {
        SignableCandidate {
            subject: format!("github:person{number}"),
            predicate: "adheres-to-philosophy".to_string(),
            object: format!("philosophy-{number}"),
            evidence: Vec::new(),
            confidence: 0.2,
            references: Vec::new(),
            derived_from: String::new(),
        }
    }

    fn listed_candidates(count: usize) -> Vec<SignableCandidate> {
        (1..=count).map(candidate_numbered).collect()
    }

    /// A list of candidates plus a non-empty selection of DISTINCT in-range
    /// 1-based numbers, in arbitrary order.
    fn list_and_valid_selection() -> impl Strategy<Value = (usize, Vec<usize>)> {
        (1usize..12).prop_flat_map(|count| {
            (
                Just(count),
                Just((1..=count).collect::<Vec<_>>()).prop_shuffle(),
                1..=count,
            )
                .prop_map(|(count, shuffled, take)| (count, shuffled[..take].to_vec()))
        })
    }

    fn raw_selection(numbers: &[usize]) -> String {
        numbers
            .iter()
            .map(usize::to_string)
            .collect::<Vec<_>>()
            .join(",")
    }

    proptest! {
        /// Selecting number `i` resolves to exactly the `i`-th listed
        /// candidate, one per selected number, in selection order — the list
        /// and `--sign` share one numbering (US-CPI-003 IS-5 / IS-9, UC-8).
        #[test]
        fn each_selected_number_resolves_to_the_candidate_listed_under_it(
            (count, numbers) in list_and_valid_selection(),
        ) {
            let listed = listed_candidates(count);
            let resolved = selected_candidates(&listed, &raw_selection(&numbers))
                .expect("a distinct in-range selection is accepted");
            let resolved_subjects: Vec<(usize, String)> = resolved
                .iter()
                .map(|(number, candidate)| (*number, candidate.subject.clone()))
                .collect();
            let expected: Vec<(usize, String)> = numbers
                .iter()
                .map(|&number| (number, listed[number - 1].subject.clone()))
                .collect();
            prop_assert_eq!(resolved_subjects, expected);
        }

        /// A selection naming a number outside 1..=count is refused before
        /// any candidate is resolved, the refusal naming the number and the
        /// valid range (US-CPI-003 IS-4).
        #[test]
        fn a_selection_outside_the_listed_range_is_refused_naming_the_valid_range(
            (count, numbers) in list_and_valid_selection(),
            beyond in 1usize..50,
            position in any::<prop::sample::Index>(),
        ) {
            let listed = listed_candidates(count);
            let missing = count + beyond;
            let mut selection = numbers.clone();
            selection.insert(position.index(selection.len() + 1), missing);
            let refusal = selected_candidates(&listed, &raw_selection(&selection))
                .expect_err("an out-of-range selection must be refused");
            let expected = format!("candidate {missing} does not exist; valid range 1..{count}");
            prop_assert!(refusal.contains(&expected), "{:?} must contain {:?}", refusal, expected);
        }

        /// Exactly the values in [0.0, 1.0] are accepted, unchanged.
        #[test]
        fn a_confidence_within_the_unit_interval_is_accepted_unchanged(value in 0.0f64..=1.0) {
            prop_assert_eq!(parse_confidence(&value.to_string()), Ok(value));
        }

        /// Any number outside [0.0, 1.0] is refused, the refusal naming what
        /// was typed and the valid range.
        #[test]
        fn a_confidence_outside_the_unit_interval_is_refused_naming_the_value(
            value in prop_oneof![-1.0e6f64..-f64::EPSILON, (1.0f64 + 1.0e-9)..1.0e6],
        ) {
            let typed = value.to_string();
            let refusal = parse_confidence(&typed).expect_err("out of range must be refused");
            prop_assert!(refusal.contains(&typed), "refusal {:?} must name {:?}", refusal, typed);
            prop_assert!(refusal.contains("confidence must be between 0.0 and 1.0"));
        }

        /// Text that is not a number (including NaN) is refused, never signed.
        #[test]
        fn a_non_numeric_confidence_is_refused(typed in "[a-zA-Z]{1,8}") {
            prop_assert!(parse_confidence(&typed).is_err());
        }
    }
}
