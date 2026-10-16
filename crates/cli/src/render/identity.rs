//! `openlore identity list` — the reader's linked identities (ADR-085 §5).

/// The empty state of `identity list` (Q-GDL-D1, content-frozen wording).
pub const NO_LINKED_IDENTITIES: &str = "No linked identities";

/// Render `identity list` when no stored claim is a link candidate.
pub fn render_no_linked_identities() -> String {
    format!("{NO_LINKED_IDENTITIES}.\n")
}
