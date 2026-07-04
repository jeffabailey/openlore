//! Shared low-level render helpers reused across the verb-output modules:
//! confidence/weight formatting, text wrapping, pluralization, and the
//! distinct-count aggregates.

use super::*;

/// Render a candidate's confidence as the minimal decimal matching the
/// original `f64` (e.g. `0.25`) via serde — never `{:.2}` (that would be
/// normalization). Mirrors the read-path `render_confidence` rule.
pub(crate) fn render_candidate_confidence(confidence: f64) -> String {
    serde_json::to_value(confidence)
        .map(|v| v.to_string())
        .unwrap_or_else(|_| "(unrenderable)".to_string())
}

/// The compose-time display bucket label for a confidence value (WD-10).
/// Slice-02 candidates are always the conservative `0.25` default
/// (speculative); the full bucket scale is a slice-01 concern. This label is
/// DISPLAY-ONLY — it never enters a signed payload (the signed claim records
/// the numeric `f64`).
pub(crate) fn confidence_bucket_label(confidence: f64) -> &'static str {
    if confidence < 0.4 {
        "speculative"
    } else if confidence < 0.7 {
        "weighted"
    } else if confidence < 0.9 {
        "well-evidenced"
    } else {
        "triangulated"
    }
}

/// Word-wrap `text` to at most `width` columns per line, breaking on ASCII
/// spaces. A single word longer than `width` is emitted on its own line
/// uncut (we never split inside a word — that could corrupt a URL or CID).
/// Pure helper; the reason text is shown verbatim, only line-broken.
pub(crate) fn wrap_at(text: &str, width: usize) -> Vec<String> {
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

/// The count of distinct subjects in an attributed result set. Pure helper.
pub(crate) fn distinct_subject_count(claims: &[AttributedClaim]) -> usize {
    let mut seen: std::collections::HashSet<&str> = std::collections::HashSet::new();
    for claim in claims {
        seen.insert(claim.subject.as_str());
    }
    seen.len()
}

/// The count of distinct (bare) author DIDs in an attributed result set. Pure
/// helper.
pub(crate) fn distinct_author_count(claims: &[AttributedClaim]) -> usize {
    let mut seen: std::collections::HashSet<&str> = std::collections::HashSet::new();
    for claim in claims {
        seen.insert(claim.author_did.0.as_str());
    }
    seen.len()
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
pub(crate) fn render_confidence(confidence: &claim_domain::Confidence) -> String {
    serde_json::to_value(confidence)
        .map(|v| v.to_string())
        .unwrap_or_else(|_| "(unrenderable)".to_string())
}

// -----------------------------------------------------------------------------
// Slice-04 (ADR-020) — `graph query --object <philosophy> --weighted` renderer
// -----------------------------------------------------------------------------

/// Pluralize a count + singular noun for the honesty line: `1 claim`, `2
/// claims`, `0 authors`. English `-s` plural suffices for the domain nouns
/// (claim/author). Pure helper.
pub(crate) fn pluralize(count: u32, singular: &str) -> String {
    if count == 1 {
        format!("{count} {singular}")
    } else {
        format!("{count} {singular}s")
    }
}
