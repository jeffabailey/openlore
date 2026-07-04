//! `/score` — the contributor-scoring transparency surface.

use super::*;

/// The anti-misread legend rendered ONCE per scored `/score` breakdown (slice-14 /
/// US-CF-002 / ADR-051 §6.3 / AC-SCORE-ANTIMISREAD). A `/score` breakdown carries
/// SCORING SEMANTICS, so the at-a-glance "Countered" marker MUST be provably
/// ORTHOGONAL to the score — SHOWN, never APPLIED. This plain-language legend states,
/// in NEUTRAL terms, that the marker means another claim disagrees ELSEWHERE, that it
/// is shown for the reader to judge, and that it does NOT change the contributor's
/// score (each contribution keeps its full weight). It carries NONE of the
/// verdict/penalty/subtraction words a reader could misread as a deduction
/// ("disputed"/"refuted"/"false"/"penalty"/"deduction"/"lowered"/"disputed score").
/// Held in ONE place (the SSOT for the legend copy) and rendered EXACTLY ONCE in
/// [`render_score_result`]'s `Scored` arm (ADR-051 §6.3 / DD-14-3 — one legend per
/// scored breakdown governs every marker below it; never per row, never per pairing).
/// The `Form`/`NoClaims` arms render NO legend (it governs markers that do not appear).
pub const SCORE_COUNTER_LEGEND: &str =
    "A “Countered” marker means another claim disagrees with this one elsewhere. \
     It is shown for you to judge and does not change this contributor's score — \
     each contribution keeps its full weight.";

/// The HTML `id` of the `/score` results swap-target region (slice-09; the
/// sibling of slice-08's [`SEARCH_RESULTS_ID`]). htmx swaps the element whose id
/// matches; the no-JS full page EMBEDS the SAME `<div id="score-results">` so the
/// fragment and the full-page score region are structurally identical (I-CS-7
/// parity by construction). Held in ONE place so the fragment fn, the page slot,
/// and the form's `hx-target` all reference the SAME id (one mutation site).
pub const SCORE_RESULTS_ID: &str = "score-results";

/// The real route the `/score` form GETs back to (`/score`) — the no-JS `action`,
/// the htmx `hx-get`, AND the nav link all reference this one path (ADR-034: one
/// source of truth for "where am I"). Held in ONE place so the references can
/// never drift apart.
pub const SCORE_URL: &str = "/score";

/// The honesty-line SUFFIX a `[SPARSE]` pairing carries beneath its bucket label
/// (I-CS-3 / KPI-GRAPH-4): thin evidence is a LEAD, not a conclusion. The full line
/// is "based on N claim(s) by M author(s) — treat as a lead, not a conclusion",
/// where N / M are PROJECTED from the pure core's `claim_count` /
/// `distinct_author_count` (WD-CS-6 — the viewer recomputes NO bucket and NO
/// counts). Held in ONE place so the honesty promise is a single source of truth +
/// a single mutation site; [`render_sparse_honesty_line`] interpolates the counts.
pub const SCORE_SPARSE_HONESTY_NOTICE: &str = "treat as a lead, not a conclusion.";

/// Build the `[SPARSE]` honesty line from the pairing's PROJECTED counts (WD-CS-6 —
/// the viewer recomputes NEITHER the bucket NOR the counts; it reads `claim_count`
/// and `distinct_author_count` off the pure-core `WeightedPairing` and projects
/// them): "based on N claim(s) by M author(s) — treat as a lead, not a conclusion."
/// PURE total function over the two counts. Held in ONE place so the honesty phrasing
/// (the counts + the lead-not-conclusion clause) is a single source of truth.
fn render_sparse_honesty_line(claim_count: u32, distinct_author_count: u32) -> String {
    format!(
        "based on {claim_count} claim(s) by {distinct_author_count} author(s) — \
         {SCORE_SPARSE_HONESTY_NOTICE}"
    )
}

/// The fixed plain-language notice the [`ScoreState::NoClaims`] arm renders when a
/// contributor has NO claims in the local store (OD-CS-6 / I-CS-5). Held in ONE
/// place AND emitted as a fixed constant so emptiness is recognized as emptiness —
/// never a fabricated zero score, never a leaked error internal. The queried DID
/// is named alongside it so the operator knows WHO was looked up.
pub const SCORE_NO_CLAIMS_NOTICE: &str = "No local claims for that contributor.";

/// The state the contributor-score results region renders (the pure render
/// input). An ADT over the three outcomes of a `/score` interaction so the
/// renderer matches totally (nw-fp-domain-modeling §1): the empty GET form, a
/// scored contributor (the REUSED `scoring::WeightedView`), or the guided
/// no-claims empty state. The effect shell builds this from the LOCAL feed read +
/// the pure `scoring::score` outcome (a bare `GET /score` with no `?contributor`
/// → `Form`; a non-empty feed → `Scored`; an empty feed → `NoClaims`); the
/// renderer is a pure total function over it.
///
/// `PartialEq` (not `Eq`) because the embedded `WeightedView` carries `f64`
/// weights.
#[derive(Debug, Clone, PartialEq)]
pub enum ScoreState {
    /// `GET /score` with no `?contributor` supplied: the empty contributor form,
    /// no score run yet.
    Form,
    /// The contributor's LOCAL feed scored to ≥1 ranked pairing: render each
    /// pairing's headline weight + `WeightBucket` label + the per-claim breakdown
    /// TABLE. Carries the REUSED `scoring::score` output VERBATIM — the viewer
    /// holds NO scoring math (the weight + decomposition are the pure core's job;
    /// the renderer only projects them).
    Scored {
        /// The REUSED ranked `WeightedView` (the `WeightedPairing`s + their
        /// per-claim `Contribution` decomposition — anti-merging by construction).
        view: scoring::WeightedView,
    },
    /// The contributor has NO claims in the LOCAL store (OD-CS-6 / I-CS-5): render
    /// the guided [`SCORE_NO_CLAIMS_NOTICE`] naming the queried DID — never a blank
    /// region, never a fabricated zero score, never a crash.
    NoClaims {
        /// The queried contributor DID, named in the guided empty state (so the
        /// operator sees WHO was looked up).
        contributor: String,
    },
}

/// Render the contributor-score swap-target FRAGMENT (slice-09; ADR-039/040/041):
/// the `<div id="score-results">` wrapping the ranked pairings (or the guided
/// no-claims notice) for the given [`ScoreState`]. PURE: a total function from the
/// view-model to a `Markup` — NO full-page chrome (no `<!DOCTYPE>`, no
/// `<html>`/`<head>`) and NO form, so an `HX-Request` response carries ONLY this
/// results region (I-CS-7). Renders NO sign/publish/follow control (I-CS-1 /
/// WD-CS-3 — the score is a read + pure compute; signing/following stays a CLI
/// action). [`render_score_page`] EMBEDS this SAME fn beneath the form, so the
/// fragment and the full page's score region are byte-identical by construction
/// (I-CS-7 parity — the score-rendering logic is NOT duplicated).
pub fn render_score_results_fragment(
    state: &ScoreState,
    presence: &std::collections::HashSet<String>,
) -> Markup {
    html! {
        div id=(SCORE_RESULTS_ID) {
            (render_score_result(state, presence))
        }
    }
}

/// Render the contributor-score page (`GET /score`, US-CS-001..003) as a complete
/// HTML document (maud). PURE: a total function from the [`ScoreState`] to an HTML
/// string — no I/O, no network. Renders the page chrome (incl. the local
/// offline-first htmx `<script src>` + a nav link back to the other views), the
/// labeled contributor form, THEN the score results region.
///
/// COMPOSITION (slice-09; ADR-041): the results region is chrome + nav + form
/// wrapped AROUND [`render_score_results_fragment`] — the EXACT same fragment fn
/// the htmx shape returns alone. Because the results region is the SAME fn in both
/// shapes, fragment/full-page parity is structural, not asserted by duplicating
/// render logic (I-CS-7). The `<head>` emits exactly ONE local
/// `<script src="/static/htmx.min.js">` (offline-first, never a CDN) so the form's
/// `hx-get` swap works in-browser instead of falling back to a full GET.
pub fn render_score_page(
    state: &ScoreState,
    presence: &std::collections::HashSet<String>,
) -> String {
    let markup = html! {
        (DOCTYPE)
        html {
            (page_head("OpenLore — Contributor Score"))
            body {
                h1 { "Contributor Score" }
                nav {
                    a href=(MY_CLAIMS_URL) { "My Claims" }
                }
                (render_score_form())
                (render_score_results_fragment(state, presence))
            }
        }
    };
    markup.into_string()
}

/// Render the labeled contributor form (`GET /score` and the top of every score
/// render). PURE. The form GETs back to `/score` with a labeled input for the
/// `contributor` DID so the operator can submit / re-submit. It carries NO
/// sign/follow control. Enhanced with `hx-get`/`hx-target` so an in-browser submit
/// swaps ONLY the `#score-results` region; the no-JS path is a plain `GET` to
/// `/score`.
fn render_score_form() -> Markup {
    html! {
        form method="get" action=(SCORE_URL)
             hx-get=(SCORE_URL)
             hx-target=(format!("#{SCORE_RESULTS_ID}"))
             hx-swap="innerHTML" {
            label for="contributor" { "Contributor DID" }
            input type="text" id="contributor" name="contributor";
            button type="submit" { "Score" }
        }
    }
}

/// Render the results region beneath the form for the given [`ScoreState`]. PURE
/// total match over the ADT: the GET form shows nothing yet; a scored contributor
/// shows the ranked pairings; no-claims shows the guided empty state naming the
/// queried DID.
fn render_score_result(state: &ScoreState, presence: &std::collections::HashSet<String>) -> Markup {
    html! {
        @match state {
            ScoreState::Form => {}
            ScoreState::Scored { view } => {
                // The anti-misread legend renders EXACTLY ONCE, ABOVE the pairings
                // (ADR-051 §6.3 / DD-14-3): one legend per scored breakdown governs
                // every "Countered" marker below it. SHOWN, never APPLIED — the copy
                // states the marker does not change the score. Additive markup only:
                // the byte-identity gold elides it (AC-SCORE-BYTEID).
                p { (SCORE_COUNTER_LEGEND) }
                @for pairing in &view.ranked {
                    (render_score_pairing(pairing, presence))
                }
            }
            // No-claims (OD-CS-6 / I-CS-5): the guided plain-language empty state
            // naming the queried DID — never a blank region or a crash.
            ScoreState::NoClaims { contributor } => {
                p { (SCORE_NO_CLAIMS_NOTICE) " (" (contributor) ")" }
            }
        }
    }
}

/// Render ONE ranked `(subject, object)` pairing: its headline weight + the
/// `WeightBucket` label, then the per-claim breakdown TABLE. The headline weight
/// AND the breakdown rows are projected from the SAME [`scoring::WeightedPairing`],
/// so the rendered subtotals sum to the rendered weight BY CONSTRUCTION (Gate 2 /
/// KPI-GRAPH-3 reproduce-by-hand). The weight is rendered VERBATIM (the exact
/// consumed `f64`, two decimals — never a bucket-midpoint rounding). A score is
/// NEVER shown without its breakdown (I-CS-2; the J-002c thesis).
fn render_score_pairing(
    pairing: &scoring::WeightedPairing,
    presence: &std::collections::HashSet<String>,
) -> Markup {
    html! {
        section {
            h2 { (pairing.subject) " — " (pairing.object) }
            p {
                "Weight: " (render_weight(pairing.weight))
                " " (render_weight_bucket(pairing.bucket))
            }
            // A `[SPARSE]` pairing carries the honesty line PROJECTED from the pure
            // core's `WeightBucket::Sparse` + the `claim_count` / `distinct_author_
            // count` counts (I-CS-3 / KPI-GRAPH-4) — the viewer recomputes NEITHER
            // the bucket NOR the counts (WD-CS-6).
            @if matches!(pairing.bucket, scoring::WeightBucket::Sparse) {
                p { (render_sparse_honesty_line(pairing.claim_count, pairing.distinct_author_count)) }
            }
            (render_score_breakdown(pairing, presence))
        }
    }
}

/// Render the per-claim breakdown TABLE for a pairing (the `--explain`
/// decomposition made VISIBLE, I-CS-2 / I-CS-10). One row per
/// [`scoring::Contribution`]: the contribution's author DID (non-`Option`
/// attribution, never merged away), the claim CID, the VERBATIM base confidence
/// (via [`render_confidence`] — `0.86`, never `0.9`/`86%`; I-CS-6), the
/// author-distinct + cross-project-triangulation bonuses, and the subtotal. The
/// subtotals sum to the pairing's headline weight (Gate 2) because both are
/// projected from the SAME `WeightedPairing`.
fn render_score_breakdown(
    pairing: &scoring::WeightedPairing,
    presence: &std::collections::HashSet<String>,
) -> Markup {
    html! {
        table {
            thead {
                tr {
                    th { "Author" }
                    th { "CID" }
                    th { "Confidence" }
                    th { "Author bonus" }
                    th { "Triangulation bonus" }
                    th { "Subtotal" }
                }
            }
            tbody {
                @for contribution in pairing.contributions() {
                    tr {
                        td { (contribution.author_did().0) }
                        td { (contribution.cid.0) }
                        td { (render_confidence(contribution.base)) }
                        td { (render_weight(contribution.author_distinct_bonus)) }
                        td { (render_weight(contribution.cross_project_triangulation_bonus)) }
                        // The VERBATIM subtotal (UNCHANGED — read straight off the pure
                        // core's `WeightedPairing`, never recomputed) THEN the additive
                        // "Countered" marker BESIDE it (ADR-051 §7). The REUSED slice-13
                        // `render_countered_link` SSOT emits the render-only one-hop link
                        // `<a href="/claims/{cid}">Countered</a>` IFF the contribution's
                        // CID is in the threaded presence set; an un-countered
                        // contribution emits NOTHING, so its cell is byte-identical to
                        // slice-09. The presence bool can ONLY gate the marker — it NEVER
                        // reaches the subtotal (the sum-to-weight orthogonality).
                        td {
                            (render_weight(contribution.subtotal))
                            (render_countered_link(&contribution.cid.0, presence.contains(&contribution.cid.0)))
                        }
                    }
                }
            }
        }
    }
}

/// Format a derived weight / bonus / subtotal `f64` VERBATIM for display: two
/// decimal places (`0.55`), mirroring [`render_confidence`]'s verbatim contract so
/// the displayed numbers are byte-stable and the operator can reproduce the running
/// sum by hand (KPI-GRAPH-3). Held separately from `render_confidence` because a
/// weight is NOT a `[0.0, 1.0]` confidence (it can exceed 1.0), but the two-decimal
/// rendering is identical, so the reproduce-by-hand arithmetic lines up.
pub(crate) fn render_weight(value: f64) -> String {
    format!("{value:.2}")
}

/// Render the display-only [`scoring::WeightBucket`] label PROJECTED from the pure
/// core (WD-CS-6 — the viewer recomputes no bucket). `Sparse` renders the
/// load-bearing `[SPARSE]` marker the honesty scenarios assert on (I-CS-3); the
/// breadth guard, not the weight magnitude, decided it in the pure core. PURE total
/// match over the ADT.
fn render_weight_bucket(bucket: scoring::WeightBucket) -> &'static str {
    match bucket {
        scoring::WeightBucket::Strong => "Strong",
        scoring::WeightBucket::Moderate => "Moderate",
        scoring::WeightBucket::Sparse => "[SPARSE]",
    }
}

// =============================================================================
// Graph-Traversal view (slice-10; ADR-042/043/044/045) — `GET /project?subject=<uri>`
// =============================================================================
//
// The `/project` route reads the project's LOCAL attributed survey over the
// read-only `StoreReadPort::query_project_survey` (claims ∪ local peer_claims, NO
// network — I-GT-2), groups the attributed rows in the PURE `viewer-domain` core
// into a [`TraversalView`] (grouping in Rust, NEVER SQL — I-GT-3), and renders it
// here. Each EDGE is one signed claim attributed to its author DID (non-`Option`,
// never merged — I-GT-3/I-GT-4) carrying a VERBATIM confidence (`0.90`, I-GT-5) +
// the REUSED claim-domain display-only confidence bucket. This crate holds NO
// grouping/bucketing math beyond the projection — it REUSES
// `claim_domain::confidence_bucket` (the WD-10 thresholds, one SSOT).
