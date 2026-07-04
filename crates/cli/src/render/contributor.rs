//! `openlore graph --contributor` — the contributor claim trail.

use super::*;

/// Slice-04 content-frozen honest-framing footer for the `--contributor`
/// dimension view (US-GRAPH-002 / J-002 "published reasoning trail, not
/// surveillance"). The contributor view shows ONE developer's RAW trail — each
/// claim's compose-time confidence verbatim, NEVER an aggregate/community
/// score. Do NOT paraphrase — the exact string is the user-visible contract.
pub const CONTRIBUTOR_TRAIL_FOOTER: &str =
    "This is one developer's reasoning trail, not a community consensus.";

/// Slice-04 content-frozen graceful-degrade hint for the `--contributor`
/// dimension view when the queried DID has NO local claims (not subscribed /
/// not pulled). US-GRAPH-002 Example 3 / UAT scenario 3: an absent contributor
/// is a valid empty result (exit 0), NOT an error — the view degrades to a
/// no-local-claims message plus a subscribe/pull next step pointing at
/// `openlore peer add` + `openlore peer pull` so the user can populate that
/// contributor's trail (J-002 anxiety mitigation; slice-03 `peer add`/`pull`
/// hint precedent). `{contributor}` is filled with the queried DID. Do NOT
/// paraphrase — the exact phrasing is the user-visible contract.
const CONTRIBUTOR_ABSENT_HINT_TEMPLATE: &str =
    "No local claims authored by {contributor}. Subscribe and pull with `openlore peer add` + `openlore peer pull`.";

/// Render the `graph query --contributor <did>` dimension result: every claim
/// that DID authored, across all subjects, listed under the contributor's DID
/// with subject/object/confidence/cid. Pure function — no I/O, no storage
/// access.
///
/// ## Honest-trail contract (US-GRAPH-002 / J-002; anti-merging WD-73)
///
/// Each [`AttributedClaim`] carries its `author_did` at the type level
/// (non-`Option`). For a contributor query every row is by the SAME author, so
/// the listing reads as that one developer's published reasoning trail:
///
/// - A header names the contributor DID, annotated with its relationship to the
///   local user (`(you)` for a self-review / `(subscribed peer)` /
///   `(unsubscribed cache)`).
/// - Every claim row prints `subject`, `object`, the numeric `confidence` shown
///   HONESTLY (the raw compose-time `f64` + its display-only bucket — NOT a
///   manufactured aggregate weight), and the `cid` — so every claim in the
///   trail is independently attributable to exactly one signed claim.
/// - The footer states the claim count AND the content-frozen
///   [`CONTRIBUTOR_TRAIL_FOOTER`] ("one developer's reasoning trail, not a
///   community consensus") so the view never reads as community endorsement.
///
/// ## Empty branch (GQE-8 / US-GRAPH-002 Example 3 — absent contributor)
///
/// When `claims` is empty the queried DID has NO local claims (the user has
/// not subscribed to / pulled that contributor). This is a VALID empty result,
/// not an error — the renderer degrades GRACEFULLY to a no-local-claims message
/// naming the queried DID plus the content-frozen subscribe/pull hint
/// ([`CONTRIBUTOR_ABSENT_HINT_TEMPLATE`]) pointing at `openlore peer add` +
/// `openlore peer pull`. The honest-trail footer is omitted — there is no trail
/// to frame (J-002 anxiety mitigation: sparse renders sparse, with a next step).
/// The verb keeps exit 0.
pub fn render_contributor_query_trail(contributor: &str, claims: &[AttributedClaim]) -> String {
    // GQE-8: an absent contributor (no local claims) degrades to the
    // no-local-claims message + subscribe/pull hint. Branch on the data shape
    // (empty vs found) so the found-trail path below stays uncluttered.
    if claims.is_empty() {
        return render_contributor_absent_hint(contributor);
    }

    let mut out = String::new();
    out.push_str(&format!(
        "Reasoning trail for contributor {contributor}:\n\n"
    ));

    // The contributor's relationship to the local user is uniform across the
    // trail (every row is by the same author). Read it from the first row so the
    // header annotation (you / subscribed peer / unsubscribed cache) is honest.
    if let Some(first) = claims.first() {
        out.push_str(&format!(
            "author_did: {} {}\n",
            first.author_did.0,
            relationship_annotation(first.relationship)
        ));
    }

    for claim in claims {
        out.push_str(&render_one_contributor_claim(claim));
    }
    out.push('\n');

    out.push_str(&render_contributor_trail_footer(claims.len()));
    out
}

/// Render one claim row of a contributor's trail: its subject, object, the
/// numeric confidence + display-only bucket (shown honestly — the raw
/// compose-time value, never an aggregate), and the cid. Every row is
/// independently attributable to one signed claim (anti-merging behavioral
/// layer). Pure helper.
pub(crate) fn render_one_contributor_claim(claim: &AttributedClaim) -> String {
    let mut out = String::new();
    out.push_str(&format!("  subject:    {}\n", claim.subject));
    out.push_str(&format!("  object:     {}\n", claim.object));
    out.push_str(&format!(
        "  confidence: {} ({})\n",
        render_candidate_confidence(claim.confidence),
        confidence_bucket_label(claim.confidence)
    ));
    out.push_str(&format!("  cid:        {}\n", claim.cid.0));
    out
}

/// Render the `--contributor` dimension footer: the claim count plus the
/// content-frozen honest-trail framing (US-GRAPH-002). Pure helper.
pub(crate) fn render_contributor_trail_footer(claim_count: usize) -> String {
    format!("{claim_count} claim(s). {CONTRIBUTOR_TRAIL_FOOTER}\n")
}

/// Render the absent-contributor graceful-degrade hint (GQE-8 / US-GRAPH-002
/// Example 3): the queried DID has no local claims, so emit the content-frozen
/// [`CONTRIBUTOR_ABSENT_HINT_TEMPLATE`] with `{contributor}` filled in. No
/// per-claim row, no honest-trail footer — there is no trail to frame, only the
/// subscribe/pull next step. Pure helper.
pub(crate) fn render_contributor_absent_hint(contributor: &str) -> String {
    format!(
        "{}\n",
        CONTRIBUTOR_ABSENT_HINT_TEMPLATE.replace("{contributor}", contributor)
    )
}

// -----------------------------------------------------------------------------
// Slice-04 (ADR-020 / US-GRAPH-004) — `graph query --traverse` tree renderer
// -----------------------------------------------------------------------------
