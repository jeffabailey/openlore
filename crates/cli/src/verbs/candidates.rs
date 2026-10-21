//! `candidates list [--subject github:<owner>/<repo>]` — review what imports
//! derived, OFFLINE (owner-only-signing slice 5, OOS-D12 B, C-OOS-7).
//!
//! A reader by construction: the verb receives the imported-candidate read
//! port only — no write port, no store, no signer, no GitHub. It lists the
//! candidates `scrape github` kept locally; nothing is asked, signed,
//! published or weighed. Signing a candidate still goes through an import
//! (`scrape github <repo> --sign N`), which re-derives it through the gate.

use anyhow::{Context, Result};
use ports::{ImportedCandidateReadPort, KeptCandidate, RepoSubjectKey};

/// Printed when nothing kept matches.
pub const NO_IMPORTED_CANDIDATES: &str = "No imported candidates.\n";

/// Run `candidates list`, narrowed to `subject` when given; returns the
/// stdout to print.
pub fn run(kept: &dyn ImportedCandidateReadPort, subject: Option<&str>) -> Result<String> {
    let subject_key = subject.map(RepoSubjectKey::of);
    let listed = kept
        .list(subject_key.as_ref())
        .context("reading the imported candidates")?;
    Ok(render_kept_candidates(&listed))
}

/// The candidate list, one line per kept candidate. Pure.
fn render_kept_candidates(listed: &[KeptCandidate]) -> String {
    if listed.is_empty() {
        return NO_IMPORTED_CANDIDATES.to_string();
    }
    let lines: String = listed.iter().map(render_kept_candidate).collect();
    format!(
        "Imported candidates (kept locally; not signed, published or weighed):\n{lines}\
         Sign one by importing again: openlore scrape github <owner/repo> --sign N\n"
    )
}

fn render_kept_candidate(kept: &KeptCandidate) -> String {
    let candidate = &kept.candidate;
    format!(
        "  {}  {}  {}  (confidence {:.2}; imported {})\n",
        candidate.subject,
        candidate.predicate,
        candidate.object,
        candidate.confidence,
        kept.imported_at.format("%Y-%m-%d"),
    )
}
