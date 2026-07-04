//! `openlore score` — the weighted adherence view + `--explain` breakdown.

use super::*;

/// Content-frozen never-stored notice for the `--weighted` view (WD-72; journey
/// `journey-explore-the-graph-visual.md` step 4 tui_mockup). Weights are a
/// DERIVED, DISPLAY-ONLY aggregate VIEW computed at query time — they are NOT
/// stored, NOT signed, and NOT published; a re-run after a `peer pull` may
/// change them; and (anti-merging, I-GRAPH-2) each weight decomposes to the
/// exact claims that produced it. Do NOT paraphrase — the exact phrasing is the
/// user-visible contract.
const WEIGHTED_NEVER_STORED_FOOTER: &str =
    "Note: weights are a DISPLAY-ONLY aggregate VIEW computed at query time from the claims \
above. They are NOT stored, NOT signed, and NOT published. Re-running after a `peer pull` may \
change them. Each weight decomposes to the exact claims that produced it; nothing is merged or \
invented.";

/// Render the `graph query --object <philosophy> --weighted` result: the
/// projects ranked by adherence weight (descending), each weight shown WITH its
/// inputs, the transparent NO-ML formula, and the never-stored display-only
/// footer. Pure function — no I/O, no storage access.
///
/// ## Transparency contract (WD-71 / KPI-GRAPH-3; US-GRAPH-003)
///
/// `view.ranked` is already sorted weight-descending by the pure `scoring::score`
/// core. A weight is NEVER shown as a bare number: each carries its claim count,
/// distinct-author count, and max confidence (the formula inputs), so a user can
/// reproduce it by hand against the printed formula. The formula block states
/// "no ML" verbatim (WD-71). Cross-project triangulation by the SAME author and
/// multi-author support are surfaced as the breadth that lifts a weight.
///
/// ## Anti-merging (I-GRAPH-2 / WD-73)
///
/// A weight is an AGGREGATE VIEW, never a merge that loses attribution. The
/// breakdown surfaces the contributing authors by DID (from the pairing's
/// non-empty `contributions`), so the aggregate always decomposes to its claims.
///
/// ## Sparse honesty (WD-74; Gate 3) — RED for GQE-11
///
/// The `[SPARSE]` honesty line for a thin (1-claim 1-author) pairing lands with
/// GQE-11 (step 05-02). This renderer prints the bucket label for every pairing;
/// the sparse-specific "(!) based on N claims by M authors" advice is added then.
pub fn render_weighted_view(object: &str, view: &scoring::WeightedView) -> String {
    let mut out = String::new();
    let heading = format!("Weighted view: {object}");
    out.push_str(&heading);
    out.push('\n');
    out.push_str(&"=".repeat(heading.len()));
    out.push_str("\n\n");
    out.push_str("Projects ranked by adherence weight (transparent formula below):\n\n");

    for (index, pairing) in view.ranked.iter().enumerate() {
        out.push_str(&render_weighted_pairing(index + 1, pairing));
    }

    out.push_str(&render_weight_formula_block());
    out.push('\n');
    out.push_str(WEIGHTED_NEVER_STORED_FOOTER);
    out.push('\n');
    out
}

/// Render one ranked `(subject, object)` pairing: its rank, subject, weight, and
/// display-only bucket, then the weight inputs (claims / authors / max
/// confidence) and the breadth line (cross-project span and/or multi-author
/// support) naming the contributing authors. Pure helper.
pub(crate) fn render_weighted_pairing(rank: usize, pairing: &scoring::WeightedPairing) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "  {rank}. {subject}   weight {weight:.2}   [{bucket}]\n",
        subject = pairing.subject,
        weight = pairing.weight,
        bucket = weight_bucket_label(pairing.bucket),
    ));
    out.push_str(&format!(
        "       claims  : {claims}   authors: {authors}   max-confidence {max_conf}\n",
        claims = pairing.claim_count,
        authors = pairing.distinct_author_count,
        max_conf = render_candidate_confidence(pairing.max_confidence),
    ));
    out.push_str(&render_pairing_breadth_line(pairing));
    // WD-74 (Gate 3 sparse_renders_sparse): a thin (single-claim single-author
    // no-span) pairing carries an epistemic-honesty block naming its ACTUAL
    // evidence base + lead-not-conclusion advice, so a single high-confidence
    // opinion is NEVER presented as a settled verdict (mitigates J-002).
    out.push_str(&render_sparse_honesty_block(pairing));
    out.push('\n');
    out
}

/// Content-frozen sparse-honesty line template (WD-74; GQE-11 docstring). Names
/// the actual evidence base verbatim — "based on N claim(s) by M author(s)" —
/// with `{claims}` / `{authors}` filled by [`render_sparse_honesty_block`]. Do
/// NOT paraphrase; the exact wording is the user-visible epistemic-honesty
/// contract (Gate 3 sparse_renders_sparse).
const SPARSE_HONESTY_LINE_TEMPLATE: &str = "(!) based on {claims} by {authors}";

/// Content-frozen lead-not-conclusion advice (WD-74; GQE-11 docstring). Thin
/// evidence is a LEAD to investigate, never a defensible conclusion. Do NOT
/// paraphrase — the exact phrasing is the user-visible contract.
const SPARSE_LEAD_NOT_CONCLUSION_ADVICE: &str =
    "treat as a lead, not a defensible conclusion — investigate before relying on it";

/// Render the WD-74 epistemic-honesty block for a [`WeightBucket::Sparse`]
/// pairing: a line naming the real evidence base ("(!) based on N claim(s) by
/// M author(s)") plus the lead-not-conclusion advice. Returns the empty string
/// for a non-sparse pairing (Strong/Moderate already cleared the breadth guard,
/// so they need no honesty caveat). Pure helper.
///
/// The counts come straight off the pairing's `claim_count` /
/// `distinct_author_count` — the SAME inputs that drove the
/// [`scoring::weight_bucket`] breadth guard (WD-74/WD-90) — so the sentence
/// can never disagree with the `[SPARSE]` label it accompanies.
pub(crate) fn render_sparse_honesty_block(pairing: &scoring::WeightedPairing) -> String {
    if !matches!(pairing.bucket, scoring::WeightBucket::Sparse) {
        return String::new();
    }
    let honesty_line = SPARSE_HONESTY_LINE_TEMPLATE
        .replace("{claims}", &pluralize(pairing.claim_count, "claim"))
        .replace(
            "{authors}",
            &pluralize(pairing.distinct_author_count, "author"),
        );
    format!("       {honesty_line}\n       {SPARSE_LEAD_NOT_CONCLUSION_ADVICE}\n")
}

/// The breadth line surfaced under a pairing's inputs: the cross-project span
/// (the SAME author asserting this philosophy on >= 2 distinct subjects, which
/// the formula rewards with the triangulation bonus) and/or multi-author support
/// (distinct authors raising triangulation). Both name the contributing authors
/// so the aggregate stays attributed (anti-merging). Returns the empty string
/// for a thin single-author single-project pairing. Pure helper.
pub(crate) fn render_pairing_breadth_line(pairing: &scoring::WeightedPairing) -> String {
    let mut out = String::new();
    if pairing.cross_project_span > 1 {
        // Name the author whose cross-project span earned the triangulation
        // bonus (the contribution carrying a non-zero triangulation bonus).
        if let Some(spanning) = pairing
            .contributions()
            .iter()
            .find(|c| c.cross_project_triangulation_bonus > 0.0)
        {
            out.push_str(&format!(
                "       also-claimed-by: {} spans {} projects\n",
                spanning.author_did.0, pairing.cross_project_span,
            ));
        }
    }
    if pairing.distinct_author_count > 1 {
        out.push_str(&format!(
            "       multi-author: {} distinct authors raise triangulation\n",
            pairing.distinct_author_count,
        ));
        // List the contributing authors by DID *with each claim's own
        // confidence* so the multi-author aggregate decomposes to its attributed
        // claims (anti-merging, WD-73). Surfacing every contribution's confidence
        // is what keeps a CONFLICTING pair (e.g. 0.85 vs 0.20) honest: both
        // claims stay visible per their OWN confidence, never averaged away
        // (GQE-13; ADR-022 anti-merging-in-aggregates).
        for contribution in pairing.contributions() {
            out.push_str(&format!(
                "         - {} (confidence {})\n",
                contribution.author_did.0,
                render_candidate_confidence(contribution.base),
            ));
        }
    }
    out
}

/// The display-only bucket label for a `WeightBucket` (WD-72; never persisted).
/// Pure helper.
pub(crate) fn weight_bucket_label(bucket: scoring::WeightBucket) -> &'static str {
    match bucket {
        scoring::WeightBucket::Strong => "STRONG",
        scoring::WeightBucket::Moderate => "MODERATE",
        scoring::WeightBucket::Sparse => "SPARSE",
    }
}

/// The auditable NO-ML formula block (WD-71 / WD-77 SSOT; journey step 4
/// tui_mockup). Names every formula input and the constants, and states
/// "no ML" verbatim so the weight is reproducible by hand (Gate 2). The
/// bucket labels are flagged DISPLAY-ONLY (WD-72). Pure helper.
pub(crate) fn render_weight_formula_block() -> String {
    let mut out = String::new();
    out.push_str("How weight is computed (auditable, no ML):\n");
    out.push_str(
        "  weight = sum over claims of [ confidence\n\
\x20                               x author_distinct_bonus\n\
\x20                               x cross_project_triangulation_bonus ]\n",
    );
    out.push_str(
        "  - author_distinct_bonus        : 1.0 for the first author, \
+0.25 per add'l distinct author on the SAME (subject,object)\n",
    );
    out.push_str(
        "  - cross_project_triangulation  : +0.5 if the SAME author asserts \
this philosophy on >=2 distinct subjects\n",
    );
    out.push_str(
        "  - bucket labels [STRONG]/[MODERATE]/[SPARSE] are DISPLAY-ONLY; never persisted.\n",
    );
    out
}

// -----------------------------------------------------------------------------
// Slice-04 (ADR-020) — `graph query --weighted --explain <subject>` renderer
// -----------------------------------------------------------------------------

/// Content-frozen closing line for the `--explain` breakdown (WD-71 / Gate 2;
/// GQE-16 reproduce-by-hand). States that the running sum of the visible
/// per-claim subtotals EQUALS the displayed adherence weight, so a user can
/// reproduce the aggregate by hand. The `{running}` / `{weight}` placeholders are
/// filled with the SAME pairing weight (they are equal by construction —
/// `weight == sum(contributions.subtotal)`). Pure helper.
const EXPLAIN_RUNNING_SUM_EQUALS_WEIGHT: &str =
    "Running sum {running} = displayed adherence weight {weight} (reproduce-by-hand; Gate 2).";

/// Render the `graph query --weighted --explain <subject>` breakdown for ONE
/// matched pairing: the verbose sibling of [`render_weighted_view`]. Enumerates
/// EACH contributing claim (author DID + cid + base confidence + every applied
/// bonus on its own line + a per-claim subtotal + a running sum), and closes with
/// the line stating the running sum EQUALS the displayed adherence weight. Pure
/// function — no I/O, no storage access.
///
/// ## Gate 1 — aggregate preserves attribution (anti-merging, I-GRAPH-2 / WD-73)
///
/// Every contribution is shown under its OWN author DID + cid. No contributing
/// claim is collapsed into a faceless consensus row — the decomposition the
/// `WeightedPairing` carries (non-empty by construction) is rendered verbatim.
///
/// ## Gate 2 — weight == formula (reproduce-by-hand, WD-71 / KPI-GRAPH-3)
///
/// The running sum is accumulated over the SAME `Contribution::subtotal` values
/// the pure `scoring::score` core summed to produce `pairing.weight`. The closing
/// line states `running sum == weight`; they are equal by construction
/// (`weight == sum(contributions.subtotal)`), so the audit reproduces the
/// displayed weight exactly.
///
/// The applied-bonus lines surface each multiplier/addend that shaped a claim's
/// subtotal: the author-distinct multiplier share (`x1.0` for the first author,
/// raised per additional distinct author) and the `+0.5 cross-project
/// triangulation` addend (only when this author asserts the object on >= 2
/// distinct subjects — attributed to the author who earned it, GQE-19).
pub fn render_weighted_explain(object: &str, pairing: &scoring::WeightedPairing) -> String {
    let mut out = String::new();
    let heading = format!(
        "Explain: {subject}  (object {object})",
        subject = pairing.subject
    );
    out.push_str(&heading);
    out.push('\n');
    out.push_str(&"=".repeat(heading.len()));
    out.push_str("\n\n");

    out.push_str(&format!(
        "Adherence weight {weight} reproduced from each contributing claim:\n\n",
        weight = render_weight_value(pairing.weight),
    ));

    // WD-74 (Gate 3) sparse-honesty: a thin (single-claim single-author no-span)
    // pairing repeats the [SPARSE] honesty line so the per-claim audit never reads
    // as a settled verdict (GQE-17 extends this).
    out.push_str(&render_sparse_honesty_block(pairing));
    if matches!(pairing.bucket, scoring::WeightBucket::Sparse) {
        out.push('\n');
    }

    // Accumulate the running sum over the SAME subtotals the pure core summed to
    // the displayed weight (Gate 2 reproduce-by-hand).
    let mut running = 0.0_f64;
    for contribution in pairing.contributions() {
        running += contribution.subtotal;
        out.push_str(&render_explain_contribution(contribution, running));
    }

    out.push('\n');
    let running_line = EXPLAIN_RUNNING_SUM_EQUALS_WEIGHT
        .replace("{running}", &render_weight_value(running))
        .replace("{weight}", &render_weight_value(pairing.weight));
    out.push_str(&running_line);
    out.push('\n');
    out
}

/// Render a DERIVED weight value (the aggregate weight, a per-claim subtotal, or
/// the running sum) at two decimal places — the SAME `{:.2}` presentation
/// [`render_weighted_pairing`] uses for the `--weighted` view, so the
/// `--explain` running sum reads as the displayed weight (`1.05`, not the raw
/// `1.0500000000000002` an f64 sum can carry). DISPLAY-ONLY: these are computed
/// at query time, never persisted. The compose-time `base` confidence is NOT
/// rendered through this — it stays verbatim via [`render_candidate_confidence`]
/// (KPI-4 zero-normalization). Pure helper.
pub(crate) fn render_weight_value(value: f64) -> String {
    format!("{value:.2}")
}

/// Render ONE contribution block for the `--explain` breakdown: the author DID,
/// the claim cid, the base confidence, each applied bonus on its OWN line, the
/// per-claim subtotal, and the running sum after this claim. Pure helper.
///
/// The author-distinct multiplier share is ALWAYS shown (it is `x1.0` for the
/// first author — making the no-bonus case explicit rather than silent); the
/// `+0.5 cross-project triangulation` addend is shown ONLY when it applied,
/// attributed to this contribution's author (GQE-19). The subtotal is the value
/// the pure core computed — `base x author-distinct-share + triangulation`.
pub(crate) fn render_explain_contribution(
    contribution: &scoring::Contribution,
    running: f64,
) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "  Contribution: {author}\n",
        author = contribution.author_did.0,
    ));
    out.push_str(&format!(
        "    cid:        {cid}\n",
        cid = contribution.cid.0
    ));
    out.push_str(&format!(
        "    confidence: {base} (base)\n",
        base = render_candidate_confidence(contribution.base),
    ));
    // Author-distinct multiplier share: x1.0 for the first author; raised by
    // +0.25 per additional distinct author on the SAME (subject, object).
    out.push_str(&format!(
        "    author-distinct bonus: x{share}\n",
        share = render_candidate_confidence(contribution.author_distinct_bonus),
    ));
    // Cross-project triangulation: surfaced ONLY when it applied (attributed to
    // the author who earned it — they assert this object on >= 2 subjects; GQE-19).
    if contribution.cross_project_triangulation_bonus > 0.0 {
        out.push_str(&format!(
            "    +{bonus} cross-project triangulation\n",
            bonus = render_candidate_confidence(contribution.cross_project_triangulation_bonus),
        ));
    }
    out.push_str(&format!(
        "    subtotal:   {subtotal}\n",
        subtotal = render_weight_value(contribution.subtotal),
    ));
    out.push_str(&format!(
        "    running sum: {running}\n",
        running = render_weight_value(running),
    ));
    out
}

// -----------------------------------------------------------------------------
// Slice-05 (ADR-027) — `openlore search` network-result renderers
// -----------------------------------------------------------------------------
//
// The NETWORK discovery verb's render layer (WD-113). Per criterion 2 the
// renderers live HERE in render.rs (slice-04 lesson: renderers are NOT in a
// render/ subpath). All are PURE functions of the re-composed per-author network
// result — no I/O, no clock, no storage access. Bootstrap SCAFFOLD (step 01-04):
// the content-frozen literals are real; the render bodies are `todo!()` (the
// live renders land in Phase 03/04 driven by the AV-* acceptance scenarios).
