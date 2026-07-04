//! `openlore graph --object` — claims grouped by subject, anti-merge footer.

use super::*;

/// Slice-04 content-frozen no-merge guarantee for the `--object` dimension
/// view (US-GRAPH-001 / KPI-GRAPH-2; component-boundaries.md §"Render contract
/// (cli)"). Reuses the slice-03 ADR-013 phrasing so the anti-merging promise
/// reads identically across the federated subject view and the object view.
/// Do NOT paraphrase — the exact string is the user-visible contract.
pub const OBJECT_QUERY_NO_MERGE_FOOTER: &str =
    "Each claim is attributed to its author DID. No claims are merged.";

/// Generate the single-edit-distance neighbours of `object` confined to the
/// philosophy-URI character class (`[a-z0-9.-]`). A typo'd philosophy URI is
/// almost always one edit away from the correct one (a transposed / dropped /
/// doubled / swapped character), so the near-miss suggestion engine (GQE-4 /
/// US-GRAPH-001 Example 4) probes these candidates against the store and
/// proposes the first that has claims — "the closest existing object string"
/// (data-models.md §object near-miss suggestion). Pure function — no I/O.
///
/// Returns candidates in deterministic, dedup'd order so the verb's probe loop
/// (and any property test) is reproducible. The original `object` is never
/// emitted as its own neighbour (it already came back empty).
pub fn single_edit_neighbours(object: &str) -> Vec<String> {
    const ALPHABET: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789.-";
    let chars: Vec<char> = object.chars().collect();
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut out: Vec<String> = Vec::new();
    let mut push = |candidate: String, seen: &mut std::collections::HashSet<String>| {
        if candidate != object && seen.insert(candidate.clone()) {
            out.push(candidate);
        }
    };

    // Transpositions (adjacent swap) — the commonest typo class; probe first.
    for i in 0..chars.len().saturating_sub(1) {
        let mut next = chars.clone();
        next.swap(i, i + 1);
        push(next.into_iter().collect(), &mut seen);
    }
    // Substitutions.
    for i in 0..chars.len() {
        for &byte in ALPHABET {
            let mut next = chars.clone();
            next[i] = byte as char;
            push(next.into_iter().collect(), &mut seen);
        }
    }
    // Deletions (a doubled / extra character).
    for i in 0..chars.len() {
        let mut next = chars.clone();
        next.remove(i);
        push(next.into_iter().collect(), &mut seen);
    }
    // Insertions (a dropped character).
    for i in 0..=chars.len() {
        for &byte in ALPHABET {
            let mut next = chars.clone();
            next.insert(i, byte as char);
            push(next.into_iter().collect(), &mut seen);
        }
    }
    out
}

/// Slice-04 content-frozen empty-`--object` explainer prefix (US-GRAPH-001
/// Example 4 / UAT scenario 4). Names the queried object so the message is
/// self-explanatory. Followed by an optional near-match suggestion. Do NOT
/// paraphrase — the exact phrasing is the user-visible contract.
pub(crate) fn render_no_claims_for_object(object: &str, suggestion: Option<&str>) -> String {
    match suggestion {
        Some(near) => format!("No claims found for object {object}. Did you mean {near}?\n"),
        None => format!("No claims found for object {object}.\n"),
    }
}

/// Render the `graph query --object <philosophy>` dimension result: the
/// attributed per-claim rows GROUPED BY SUBJECT (project), each row carrying
/// its `author_did` + numeric confidence + display-only bucket + cid. Pure
/// function — no I/O, no storage access.
///
/// ## Anti-merging contract (I-GRAPH-2 / WD-73; US-GRAPH-001)
///
/// Each [`AttributedClaim`] carries its `author_did` at the type level
/// (non-`Option`). This renderer surfaces that attribution per row and NEVER
/// collapses two authors' claims about the same `(subject, object)` into one
/// aggregate:
///
/// - Rows are grouped under a per-subject header (first-seen subject order).
/// - Every claim row prints `author_did` (annotated with its relationship —
///   `(you)` / `(subscribed peer)` / `(unsubscribed cache)`), the numeric
///   `confidence`, its DISPLAY-ONLY bucket label, and the `cid` — so an
///   operator can attribute any single row to exactly one author.
/// - Two claims with identical `(subject, object)` by DIFFERENT authors render
///   as TWO rows (never merged).
/// - The footer states the distinct-SUBJECT count AND the distinct-AUTHOR
///   count AND the content-frozen [`OBJECT_QUERY_NO_MERGE_FOOTER`].
///
/// The `suggestion` argument carries the near-match the verb resolved by
/// probing the store (GQE-4 / US-GRAPH-001 Example 4) when the dimension read
/// came back empty. It is `None` when claims were found (the happy path) or when
/// no near-match exists.
pub fn render_object_query_grouped_by_subject(
    object: &str,
    claims: &[AttributedClaim],
    suggestion: Option<&str>,
) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "Claims embodying {object} (grouped by subject):\n\n"
    ));

    if claims.is_empty() {
        // Empty is HONEST (US-GRAPH-001 Example 4): name the queried object and,
        // if the store holds a near-match, suggest it. No per-claim row is
        // manufactured; exit code stays 0 (a valid not-yet-found state).
        out.push_str(&render_no_claims_for_object(object, suggestion));
        return out;
    }

    for (subject, subject_claims) in &group_by_subject(claims) {
        out.push_str(&format!("subject: {subject}\n"));
        for claim in subject_claims {
            out.push_str(&render_one_attributed_claim(claim));
        }
        out.push('\n');
    }

    out.push_str(&render_object_query_footer(
        distinct_subject_count(claims),
        distinct_author_count(claims),
    ));
    out
}

/// Group attributed claims by subject, preserving first-seen subject order
/// (stable, hash-randomization-free output). Returns one entry per distinct
/// subject carrying its claims. Pure helper.
pub(crate) fn group_by_subject<'a>(
    claims: &'a [AttributedClaim],
) -> Vec<(String, Vec<&'a AttributedClaim>)> {
    let mut order: Vec<String> = Vec::new();
    let mut grouped: Vec<(String, Vec<&'a AttributedClaim>)> = Vec::new();
    for claim in claims {
        match order.iter().position(|s| s == &claim.subject) {
            Some(pos) => grouped[pos].1.push(claim),
            None => {
                order.push(claim.subject.clone());
                grouped.push((claim.subject.clone(), vec![claim]));
            }
        }
    }
    grouped
}

/// Render one attributed claim row under its subject group: the author DID
/// (with its relationship annotation), the numeric confidence + display-only
/// bucket, and the cid. Every value is independently attributable (anti-merging
/// behavioral layer). Pure helper.
pub(crate) fn render_one_attributed_claim(claim: &AttributedClaim) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "  author_did: {} {}\n",
        claim.author_did.0,
        relationship_annotation(claim.relationship)
    ));
    out.push_str(&format!(
        "    confidence: {} ({})\n",
        render_candidate_confidence(claim.confidence),
        confidence_bucket_label(claim.confidence)
    ));
    out.push_str(&format!("    cid:        {}\n", claim.cid.0));
    out
}

/// Render the `--object` dimension footer: the distinct-subject count AND the
/// distinct-author count AND the content-frozen no-merge guarantee
/// (US-GRAPH-001). Pure helper.
pub(crate) fn render_object_query_footer(subject_count: usize, author_count: usize) -> String {
    format!(
        "{subject_count} subject(s), {author_count} author(s). {OBJECT_QUERY_NO_MERGE_FOOTER}\n"
    )
}

// -----------------------------------------------------------------------------
// Slice-04 (ADR-020) — `graph query --contributor <did>` dimension renderer
// -----------------------------------------------------------------------------
