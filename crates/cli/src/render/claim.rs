//! Single signed-claim block rendering (verbatim fields + evidence + confidence).

use super::*;
use claim_domain::authority::Authority;

/// Who each `github:` assertion's author is to its subject, to this reader,
/// by claim cid (ADR-086 §8). A cid with no entry needs no authority — a
/// dispute, a withdrawal, an identity link or a claim not about GitHub — and
/// carries no label.
pub type AuthorityByCid = std::collections::BTreeMap<String, Authority>;

/// The `whose word:` line of the claim `cid` — the one reader label
/// ([`Authority::label`]) — or nothing when the claim needs no authority. A
/// label never hides a claim: it only sits beside it.
pub(crate) fn render_whose_word_line(cid: &str, labels: &AuthorityByCid) -> String {
    labels.get(cid).map_or_else(String::new, |authority| {
        format!("whose word:  {}\n", authority.label())
    })
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
pub(crate) fn render_one_claim(claim: &SignedClaim) -> String {
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
pub(crate) fn render_evidence(evidence: &[String]) -> String {
    if evidence.is_empty() {
        "(none)".to_string()
    } else {
        evidence.join(", ")
    }
}
