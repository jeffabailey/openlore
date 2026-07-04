//! `/claims` — the My Claims list surface (row view-model + table/page render).

use super::*;

/// One claim rendered as a row in the My Claims list view. The VIEW-model shape
/// (nw-fp-domain-modeling §10): flat display strings + the numeric confidence
/// the renderer formats VERBATIM. Projected from a [`ports::ClaimRow`] by
/// [`ClaimRowView::from_row`] (a total conversion — always succeeds).
#[derive(Debug, Clone, PartialEq)]
pub struct ClaimRowView {
    pub cid: String,
    pub subject: String,
    pub predicate: String,
    pub object: String,
    /// The stored confidence DOUBLE. Rendered VERBATIM as `0.90` by
    /// [`render_confidence`] (FR-VIEW-8) — held as the numeric, not a
    /// pre-formatted string, so the rendering rule lives in ONE place.
    pub confidence: f64,
    /// Whether this claim has ≥1 counter (slice-12 / US-LF-002 / ADR-048): the
    /// at-a-glance "Countered" PRESENCE flag. A boolean per row (presence membership,
    /// NEVER a count) — set in the EFFECT shell from the `counter_presence_for` set via
    /// [`ClaimRowView::from_row_with_presence`], so the pure render stays a TOTAL
    /// function of (page, presence). ADDITIVE: it NEVER changes row order/paging/count
    /// or the confidence cell (shown-never-applied, I-LF-2).
    pub is_countered: bool,
}

impl ClaimRowView {
    /// Project a boundary [`ports::ClaimRow`] into the view-model. Total — never
    /// fails (the view-model is a strict subset of the DTO's display fields;
    /// `author_did`/`composed_at` are not shown in the list view, FR-VIEW-1). The
    /// row is UN-countered (`is_countered = false`); the slice-12 flag is set via
    /// [`Self::from_row_with_presence`] in the effect shell.
    pub fn from_row(row: &ClaimRow) -> Self {
        Self::from_row_with_presence(row, &std::collections::HashSet::new())
    }

    /// Project a boundary [`ports::ClaimRow`] into the view-model, setting the slice-12
    /// "Countered" presence flag from `presence` (the `counter_presence_for` SET):
    /// `is_countered` is true IFF this row's CID is a member (US-LF-002 / ADR-048). A
    /// TOTAL conversion — always succeeds. The flag is ADDITIVE: every display field
    /// is identical to [`Self::from_row`]; only `is_countered` differs. The effect
    /// shell calls THIS per row AFTER `list_claims` pages them, so the presence read
    /// never re-orders / re-pages / re-counts / re-weights the list (I-LF-2).
    pub fn from_row_with_presence(
        row: &ClaimRow,
        presence: &std::collections::HashSet<String>,
    ) -> Self {
        Self {
            cid: row.cid.clone(),
            subject: row.subject.clone(),
            predicate: row.predicate.clone(),
            object: row.object.clone(),
            confidence: row.confidence,
            is_countered: presence.contains(&row.cid),
        }
    }
}

/// Render the My Claims VIEW-PANEL swap-target FRAGMENT (slice-07 H-6a; ADR-034 /
/// DESIGN §6): the `<div id="view-panel">` wrapping the My Claims active list
/// (the inner [`render_claims_table_fragment`], itself the `#claims-table`
/// region). This is what an `HX-Request` tab switch to `/claims` lands on
/// (`hx-target="#view-panel"`, `hx-swap="innerHTML"`). PURE total function — NO
/// full-page chrome (I-HX-1). Because it EMBEDS the same `#claims-table` fragment
/// fn, the inner paging swap (`hx-target="#claims-table"`, H-1a) still lands on the
/// nested region — the two swap behaviors compose without conflict. The full page
/// EMBEDS this SAME view-panel fn, so the fragment and the page's panel region are
/// byte-identical by construction (I-HX-5 parity — no duplicated render logic).
pub fn render_claims_view_panel_fragment(page: &PageView<ClaimRowView>) -> Markup {
    html! {
        div id=(VIEW_PANEL_ID) {
            (render_claims_table_fragment(page))
        }
    }
}

/// Render the My Claims swap-target FRAGMENT (slice-07; ADR-032/033): the
/// `<div id="claims-table">` wrapping the claims table (or the guided empty
/// state) + the position indicator + Prev/Next controls. PURE: a total function
/// from the view-model to an HTML string — NO full-page chrome (no `<!DOCTYPE>`,
/// no `<html>`/`<head>`), so an `HX-Request` response carries ONLY this region
/// (I-HX-1). [`render_claims_page`] EMBEDS this SAME fn inside its chrome, so the
/// fragment and the full page's table region are byte-identical by construction
/// (I-HX-5 parity — the table-rendering logic is NOT duplicated). This is the
/// load-bearing slice-07 structural contract: page = chrome + fragment.
pub fn render_claims_table_fragment(page: &PageView<ClaimRowView>) -> Markup {
    let body = if page.rows.is_empty() {
        render_empty_state()
    } else {
        render_claims_table(&page.rows)
    };
    html! {
        div id=(CLAIMS_TABLE_ID) {
            (body)
            (render_pagination(page))
        }
    }
}

/// Render the My Claims page as a complete HTML document (maud). PURE: a total
/// function from the view-model to an HTML string — no I/O. Each seeded claim
/// renders as a row carrying subject/predicate/object, the VERBATIM confidence
/// (`0.90`, FR-VIEW-8), and its CID. An empty page renders the guided empty
/// state (FR-VIEW-7) instead of a blank table.
///
/// COMPOSITION (slice-07; ADR-032): the page is chrome wrapped AROUND
/// [`render_claims_table_fragment`] — the EXACT same fragment fn the htmx shape
/// returns alone. The `<head>` emits exactly ONE local
/// `<script src="/static/htmx.min.js">` (offline-first, never a CDN; I-HX-2).
/// Because the table region is the SAME fn in both shapes, fragment/full-page
/// parity is structural, not asserted by duplicating render logic (I-HX-5).
pub fn render_claims_page(
    page: &PageView<ClaimRowView>,
    countered_own_claims: Option<usize>,
) -> String {
    let markup = html! {
        (DOCTYPE)
        html {
            (page_head("OpenLore — My Claims"))
            body {
                // slice-18 (ADR-055 D3): the countered count renders in the list HEADER,
                // beside the "My Claims" heading, through the SAME shared `render_countered`
                // helper the landing summary uses (single source — the two surfaces resolve
                // from the SAME `count_countered_own_claims` read + render through the SAME
                // helper, so they cannot diverge, WD-CC-8). Additive header text ONLY — the
                // slice-06 list order/paging/count + the slice-12 per-row flags are
                // UNTOUCHED (C-4 / WD-CC-9).
                h1 { "My Claims " (render_countered(countered_own_claims)) }
                p { "This is a read-only view of the claims you have signed." }
                (render_tab_nav())
                (render_claims_view_panel_fragment(page))
            }
        }
    };
    markup.into_string()
}

/// Render the claims table (one `<tr>` per claim). Small, named, composable —
/// the per-row markup is [`render_claim_row`].
fn render_claims_table(rows: &[ClaimRowView]) -> Markup {
    html! {
        table {
            thead {
                tr {
                    th { "Subject" }
                    th { "Predicate" }
                    th { "Object" }
                    th { "Confidence" }
                    th { "CID" }
                }
            }
            tbody {
                @for row in rows {
                    (render_claim_row(row))
                }
            }
        }
    }
}

/// Render one claim row. The confidence cell goes through [`render_confidence`]
/// so the VERBATIM `0.90` rule lives in exactly one place (FR-VIEW-8). A countered
/// row ALSO carries the neutral "Countered" presence flag (slice-12) — appended to
/// the CID cell as a render-only one-hop link; see [`render_list_presence_flag`].
fn render_claim_row(row: &ClaimRowView) -> Markup {
    html! {
        tr {
            td { (row.subject) }
            td { (row.predicate) }
            td { (row.object) }
            td { (render_confidence(row.confidence)) }
            td { (row.cid) (render_list_presence_flag(row)) }
        }
    }
}

/// Render the at-a-glance "Countered" PRESENCE flag for one LIST row (slice-12 /
/// US-LF-002 / ADR-048). Thin surface-typed wrapper over the shared
/// [`render_countered_link`] SSOT body (one source of truth with the federated + edge
/// presence flags).
fn render_list_presence_flag(row: &ClaimRowView) -> Markup {
    render_countered_link(&row.cid, row.is_countered)
}

/// Render the guided empty state (FR-VIEW-7 / NFR-VIEW-6): a first-run operator
/// who has signed nothing sees guidance pointing to the CLI, NOT a blank page.
fn render_empty_state() -> Markup {
    html! {
        p {
            "You have not signed any claims yet. Claims you sign with the CLI \
             will appear here."
        }
    }
}
