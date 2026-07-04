//! `openlore scrape` output — the public-data banner + candidate list.

use super::*;

/// Content-frozen public-data-only banner (WD-51 / I-SCR-2; journey
/// `scrape-propose-sign.yaml` step 1 tui_mockup). Printed BEFORE any harvest
/// so the user is reassured no private data is read before any network beat
/// begins (emotional arc: skeptical -> reassured). Names BOTH "only PUBLIC
/// GitHub data is read" AND "Nothing published". Do NOT paraphrase.
pub const PUBLIC_DATA_BANNER: &str = "Only PUBLIC GitHub data is read. The target is the SUBJECT \
of any claim you may later sign — never a controller. Nothing published.";

/// Content-frozen candidate-list footer (journey step 2 tui_mockup): nothing
/// the scraper proposes is a claim until the human signs it (the human-gate,
/// I-SCR-1). Do NOT paraphrase — the exact string is the user-visible
/// reassurance contract.
pub const NOTHING_IS_A_CLAIM_FOOTER: &str =
    "Remember: nothing is a claim until you sign it. Select one to sign: `--sign N`.";

/// Render the public-data-only banner block (WD-51). Pure function — no I/O.
/// The verb prints this BEFORE invoking the harvest so the ordering AC
/// (SG-2: banner precedes harvest) holds structurally.
pub fn render_public_data_banner() -> String {
    format!("{PUBLIC_DATA_BANNER}\n")
}

/// Render the numbered candidate list (journey step 2 tui_mockup). Pure
/// function of the derived candidates — no I/O, no clock.
///
/// Each candidate renders as:
///
/// ```text
///  [1] embodiesPhilosophy  org.openlore.philosophy.dependency-pinning
///      from signal : Cargo.lock committed (exact pins)
///      confidence  : 0.25 (speculative)
/// ```
///
/// Every candidate NAMES its source signal(s) verbatim (auditability,
/// I-SCR-4 / KPI-SCR-3) and displays the conservative default confidence
/// `0.25` with the `speculative` bucket label (compose-time display only;
/// WD-10). Multiple contributing signals each get their own `from signal :`
/// line (US-SCR-002 Ex 4 collapse). A footer reassures nothing is a claim
/// until signed.
pub fn render_candidate_list(subject: &str, candidates: &[CandidateClaim]) -> String {
    let mut out = String::new();
    out.push_str(&format!("Candidate claims for subject {subject}\n"));
    out.push_str(&format!(
        "({} derived — NOTHING is signed or published; you choose)\n",
        candidates.len()
    ));
    for (idx, candidate) in candidates.iter().enumerate() {
        let number = idx + 1;
        out.push_str(&format!(
            " [{number}] {}  {}\n",
            candidate.predicate, candidate.object
        ));
        for signal in candidate.source_signals() {
            out.push_str(&format!("     from signal : {}\n", signal.value));
        }
        out.push_str(&format!(
            "     confidence  : {} ({})\n",
            render_candidate_confidence(candidate.confidence),
            confidence_bucket_label(candidate.confidence),
        ));
    }
    out.push_str(&format!("{NOTHING_IS_A_CLAIM_FOOTER}\n"));
    out
}

/// Render the auth-mode / rate-budget report line for a harvest (ADR-019 §5;
/// US-SCR-004; journey step 1 auth output). PURE function of the observed
/// [`AuthReport`] — no I/O.
///
/// - [`AuthReport::Authenticated`] => `authenticated (N/M rate budget)` so the
///   user sees the harvest ran on the higher PAT budget and how much remains.
/// - [`AuthReport::Anonymous`] => `unauthenticated` (no budget to report).
///
/// By construction this can NEVER echo a token value: an `AuthReport` carries
/// only the budget numbers, never the PAT bytes (no-token-leak; US-SCR-004).
pub fn render_auth_report(report: &AuthReport) -> String {
    match report {
        AuthReport::Authenticated { remaining, limit } => {
            format!("authenticated ({remaining}/{limit} rate budget)\n")
        }
        AuthReport::Anonymous => "unauthenticated\n".to_string(),
    }
}
