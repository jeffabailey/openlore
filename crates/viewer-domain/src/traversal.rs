//! `/project` + `/philosophy` — the graph-traversal edge-survey surfaces.

use super::*;

/// The HTML `id` of the `/project` + `/philosophy` traversal results swap-target
/// region (slice-10; the sibling of slice-09's [`SCORE_RESULTS_ID`] + slice-08's
/// [`SEARCH_RESULTS_ID`]). htmx swaps the element whose id matches; the no-JS full
/// page EMBEDS the SAME `<div id="traversal-results">` so the fragment and the
/// full-page results region are structurally identical (I-GT-6 parity by
/// construction). Held in ONE place so the fragment fn, the page slot, and any
/// `hx-target` all reference the SAME id (one mutation site).
pub const TRAVERSAL_RESULTS_ID: &str = "traversal-results";

/// The real route the project survey is served at (`/project`) — the no-JS `href`,
/// any htmx `hx-get`, AND the subject cross-link target all reference this one path
/// (ADR-044: one source of truth for the project-survey route). Held in ONE place so
/// the references can never drift apart.
pub const PROJECT_URL: &str = "/project";

/// The real route the philosophy survey is served at (`/philosophy`) — the object
/// cross-link target (object → philosophy traversal edge; ADR-044). Held in ONE
/// place so the cross-link href and the (slice-10) `/philosophy` route agree.
pub const PHILOSOPHY_URL: &str = "/philosophy";

/// The guided plain-language notice the [`TraversalView::NoClaims`] arm renders for
/// an entity with NO claims in the local store (US-GT-002/003 Example 3 / I-GT-4).
/// Held in ONE place AND emitted as a fixed constant so emptiness is recognized as
/// emptiness — never a fabricated edge, never a leaked error internal. The queried
/// entity is named alongside it, and a CLI next-step hint follows, so the operator
/// knows WHAT was looked up and WHERE to go next.
pub const TRAVERSAL_NO_CLAIMS_NOTICE: &str = "No claims about this in your local graph.";

/// The CLI next-step hint appended to the guided [`TraversalView::NoClaims`] state —
/// emptiness points the operator at the CLI (`graph query` / `scrape`) rather than a
/// dead end (NFR-VIEW-6 / I-GT-4). Held in ONE place (one mutation site).
pub const TRAVERSAL_NO_CLAIMS_HINT: &str =
    "Use the openlore CLI (graph query / scrape) to add claims to your local graph.";

/// The pure render input for a project (or, slice-10 later, philosophy) survey: the
/// queried entity + its direct attributed edges, grouped by the OTHER dimension
/// (data-models.md §2 / ADR-043). An ADT so the renderer matches TOTALLY
/// (nw-fp-domain-modeling §1): a non-empty survey is `Found`; an empty one (or a
/// read error) is the guided `NoClaims`. The effect shell builds this from the LOCAL
/// survey read via the pure [`group_project`]; the renderer is a pure total function
/// over it.
///
/// `PartialEq` (not `Eq`) because [`EdgeRow`] carries an `f64` confidence.
#[derive(Debug, Clone, PartialEq)]
pub enum TraversalView {
    /// ≥1 claim about the entity: the grouped, attributed edges + the distinct
    /// contributors. Grouping is in PURE Rust (anti-merging), NEVER SQL (I-GT-3).
    Found {
        /// The queried subject (project) or object (philosophy).
        entity: String,
        /// The edge groups keyed by the OTHER dimension (a philosophy on `/project`;
        /// a project on `/philosophy`). Each group's key is a traversal target.
        groups: Vec<EdgeGroup>,
        /// The distinct contributor `author_did`s across all edges, order-preserved
        /// and DEDUPED (a spanning author appears ONCE) — each a link to `/score`.
        contributors: Vec<String>,
    },
    /// Zero claims (or bare route / read error): the guided "no claims" state naming
    /// the entity — NEVER a fabricated edge (I-GT-4).
    NoClaims {
        /// The queried entity, named in the guided empty state.
        entity: String,
    },
}

/// One group of attributed traversal edges sharing the OTHER-dimension key
/// (data-models.md §2). On `/project` the `key` is an `object` (a philosophy
/// embodied); on `/philosophy` it is a `subject` (a project). The key is itself a
/// traversal target rendered as an `<a href>` to the next survey.
#[derive(Debug, Clone, PartialEq)]
pub struct EdgeGroup {
    /// The OTHER-dimension key (a philosophy on `/project`) — a traversal `<a href>`.
    pub key: String,
    /// One [`EdgeRow`] per `(author, cid)` — NEVER averaged into a consensus row.
    pub edges: Vec<EdgeRow>,
}

/// One attributed traversal edge = one signed claim (data-models.md §2). Carries the
/// non-`Option` `author_did` (attribution, never merged away — I-GT-3), the VERBATIM
/// `confidence` (rendered via [`render_confidence`] + the REUSED display-only bucket
/// — I-GT-5), and the non-`Option` `cid` (every edge maps to exactly one claim —
/// I-GT-4).
#[derive(Debug, Clone, PartialEq)]
pub struct EdgeRow {
    /// The claim author DID — non-`Option` attribution; rendered + linked to `/score`.
    pub author_did: String,
    /// The stored confidence DOUBLE — rendered VERBATIM + as a display-only bucket.
    pub confidence: f64,
    /// The claim CID — non-`Option`; every edge maps to exactly one signed claim.
    pub cid: String,
    /// Whether this edge's claim has >= 1 counter (slice-13 / US-CF-003 / ADR-048): the
    /// at-a-glance "Countered" PRESENCE flag. A boolean per edge (presence membership,
    /// NEVER a count) — set inside [`group_by`] from the `counter_presence_for` set the
    /// effect shell reads ONCE over the flattened union of every edge CID (ADR-050), so
    /// the render stays a TOTAL function of the (presence-projected) [`TraversalView`].
    /// ADDITIVE: it NEVER changes group key_order, per-key edge accumulation, edge order,
    /// the deduped contributor list, or any confidence/bucket (shown-never-applied,
    /// I-CF-2 / I-CF-9).
    pub is_countered: bool,
}

/// Group a project survey's flat [`SurveyRow`]s into a [`TraversalView`] (PURE,
/// anti-merging — data-models.md §2 "Grouping rules"). Groups the `rows` by `object`
/// (the philosophy embodied); within a group, ONE [`EdgeRow`] per row (one signed
/// claim), so two authors on the same object yield TWO rows — NEVER averaged into a
/// consensus row (I-GT-3). `contributors` is the distinct `author_did` across all
/// rows, ORDER-PRESERVED and DEDUPED (a spanning author appears ONCE in the list,
/// never deduped among the per-group edges). Group order + edge order follow the
/// `rows` order (the adapter ordered by `object, source_table, cid` — deterministic).
/// An EMPTY `rows` slice → [`TraversalView::NoClaims`] (never a fabricated edge,
/// I-GT-4). PURE total function — no I/O.
pub fn group_project(
    entity: &str,
    rows: &[SurveyRow],
    presence: &std::collections::HashSet<String>,
) -> TraversalView {
    group_by(entity, rows, presence, |row| row.object.clone())
}

/// Group a philosophy survey's flat [`SurveyRow`]s into a [`TraversalView`] (PURE,
/// anti-merging — data-models.md §2). The SYMMETRIC mirror of [`group_project`],
/// swapping subject↔object: groups the `rows` by `subject` (the project that EMBODIES
/// the philosophy), so the `/philosophy` survey lists projects-that-embody edges (vs
/// `/project`'s philosophies-embodied edges). Within a group, ONE [`EdgeRow`] per row
/// (one signed claim), so two authors on the same subject yield TWO rows — NEVER
/// averaged (I-GT-3). `contributors` is the distinct `author_did` across all rows,
/// ORDER-PRESERVED and DEDUPED (a spanning contributor appears ONCE — the canonical
/// cross-project "aha", US-GT-003). Group + edge order follow the `rows` order (the
/// adapter ordered by `subject, source_table, cid` — deterministic). An EMPTY `rows`
/// slice → [`TraversalView::NoClaims`] (never a fabricated edge, I-GT-4). PURE total
/// function — no I/O. REUSES the identical [`group_by`] anti-merging engine.
pub fn group_philosophy(
    entity: &str,
    rows: &[SurveyRow],
    presence: &std::collections::HashSet<String>,
) -> TraversalView {
    group_by(entity, rows, presence, |row| row.subject.clone())
}

/// Shared grouping engine for the two surveys (PURE, anti-merging). `key_of` selects
/// the OTHER-dimension key per row (`object` for `/project`, `subject` for
/// `/philosophy`). Order-preserving: groups appear in first-seen key order; edges
/// appear in row order; `contributors` in first-seen author order (deduped). Empty
/// `rows` → [`TraversalView::NoClaims`]. Held in ONE place so the project + philosophy
/// groupers share the identical anti-merging machinery.
///
/// `presence` is the `counter_presence_for` SET the effect shell reads ONCE over the
/// FLATTENED union of every edge CID across all groups (ADR-050 — collected from the
/// FLAT survey rows BEFORE grouping, never per-group/per-edge): each built [`EdgeRow`]
/// has `is_countered = presence.contains(&row.cid)`, so the slice-13 "Countered" flag is
/// projected HERE and the render stays a TOTAL function of the resulting view. The flag
/// is ADDITIVE — it touches NEITHER `key_order`, per-key edge accumulation, edge order,
/// NOR the deduped `contributors` (I-CF-2 / I-CF-9).
fn group_by(
    entity: &str,
    rows: &[SurveyRow],
    presence: &std::collections::HashSet<String>,
    key_of: impl Fn(&SurveyRow) -> String,
) -> TraversalView {
    if rows.is_empty() {
        return TraversalView::NoClaims {
            entity: entity.to_string(),
        };
    }
    // Order-preserving group accumulation: a parallel key-order vec drives the output
    // order while the map collects each key's edges (a BTreeMap would re-sort keys and
    // break the deterministic adapter ordering the scenarios pin).
    let mut key_order: Vec<String> = Vec::new();
    let mut grouped: std::collections::HashMap<String, Vec<EdgeRow>> =
        std::collections::HashMap::new();
    let mut contributors: Vec<String> = Vec::new();

    for row in rows {
        let key = key_of(row);
        if !grouped.contains_key(&key) {
            key_order.push(key.clone());
        }
        grouped.entry(key).or_default().push(EdgeRow {
            author_did: row.author_did.clone(),
            confidence: row.confidence,
            // The "Countered" presence flag (slice-13 / I-CF-9): membership in the
            // ONE flattened `counter_presence_for` set — ADDITIVE, set as the edge is
            // built so it never re-orders/re-groups (the key + push above are unchanged).
            is_countered: presence.contains(&row.cid),
            cid: row.cid.clone(),
        });
        if !contributors.contains(&row.author_did) {
            contributors.push(row.author_did.clone());
        }
    }

    let groups = key_order
        .into_iter()
        .map(|key| {
            let edges = grouped.remove(&key).unwrap_or_default();
            EdgeGroup { key, edges }
        })
        .collect();

    TraversalView::Found {
        entity: entity.to_string(),
        groups,
        contributors,
    }
}

/// Render the project-survey swap-target FRAGMENT (slice-10; ADR-043): the
/// `<div id="traversal-results">` wrapping the grouped attributed philosophy edges
/// (or the guided no-claims notice) for the given [`TraversalView`]. The group key
/// (a philosophy) is a traversal `<a href>` to `/philosophy?object=<encoded>`; each
/// edge row names its author DID (a link to `/score?contributor=<bare-did>`), the
/// VERBATIM confidence + the REUSED display-only bucket, and the `cid`. PURE: a total
/// function — NO full-page chrome and NO form, so an `HX-Request` response carries
/// ONLY this region (I-GT-6). Renders NO sign/publish/follow control (I-GT-1 —
/// traversal is a READ; the cross-links are render-only navigation TEXT, WD-GT-3).
/// [`render_project_page`] EMBEDS this SAME fn, so the fragment and the full page's
/// results region are byte-identical by construction (I-GT-6 parity).
pub fn render_project_fragment(view: &TraversalView) -> Markup {
    html! {
        div id=(TRAVERSAL_RESULTS_ID) {
            (render_traversal_result(view, GroupDimension::Philosophy))
        }
    }
}

/// Render the project-survey page (`GET /project?subject=<uri>`, US-GT-002) as a
/// complete HTML document (maud). PURE: a total function from the [`TraversalView`]
/// to an HTML string — no I/O, no network. Renders the page chrome (incl. the local
/// offline-first htmx `<script src>` + a nav link back to the other views) THEN the
/// traversal results region.
///
/// COMPOSITION (slice-10; ADR-043): the results region is chrome + nav wrapped AROUND
/// [`render_project_fragment`] — the EXACT same fragment fn the htmx shape returns
/// alone. Because the results region is the SAME fn in both shapes, fragment/full-page
/// parity is structural, not asserted by duplicating render logic (I-GT-6). The
/// `<head>` emits exactly ONE local `<script src="/static/htmx.min.js">`
/// (offline-first, never a CDN).
pub fn render_project_page(view: &TraversalView) -> String {
    // slice-21 (ADR-058 D6): composed through `page_shell` (persistent left nav +
    // `<main id="viewer-main">`); `active = PROJECT_URL` (the base-path const for the
    // query-bearing `/project?subject=…` route) marks the Project Survey nav item
    // current. The `render_*_fragment` fn is UNCHANGED (it rides `Shape::Fragment` for
    // the #traversal-results swap).
    let body = html! {
        h1 { "Project Survey" }
        nav {
            a href=(MY_CLAIMS_URL) { "My Claims" }
        }
        (render_project_fragment(view))
    };
    page_shell("OpenLore — Project Survey", PROJECT_URL, body)
}

/// Render the philosophy-survey swap-target FRAGMENT (slice-10; ADR-043) — the
/// SYMMETRIC mirror of [`render_project_fragment`], swapping subject↔object: the
/// `<div id="traversal-results">` wrapping the grouped attributed PROJECT edges (the
/// projects that EMBODY the philosophy) for the given [`TraversalView`]. The group key
/// (a project) is a traversal `<a href>` to `/project?subject=<encoded>`; each edge row
/// names its author DID (a link to `/score?contributor=<bare-did>`), the VERBATIM
/// confidence + the REUSED display-only bucket, and the `cid`. PURE: a total function —
/// NO full-page chrome and NO form, so an `HX-Request` response carries ONLY this region
/// (I-GT-6). Renders NO sign/publish/follow control (I-GT-1). [`render_philosophy_page`]
/// EMBEDS this SAME fn, so the fragment and the full page's results region are
/// byte-identical by construction (I-GT-6 parity). REUSES the SAME `#traversal-results`
/// region renderer, forked only on the group-key dimension (project, not philosophy).
pub fn render_philosophy_fragment(view: &TraversalView) -> Markup {
    html! {
        div id=(TRAVERSAL_RESULTS_ID) {
            (render_traversal_result(view, GroupDimension::Project))
        }
    }
}

/// Render the philosophy-survey page (`GET /philosophy?object=<uri>`, US-GT-003) as a
/// complete HTML document (maud) — the SYMMETRIC mirror of [`render_project_page`]. PURE:
/// a total function from the [`TraversalView`] to an HTML string — no I/O, no network.
/// Renders the page chrome (incl. the local offline-first htmx `<script src>` + a nav
/// link back to the other views) THEN the traversal results region.
///
/// COMPOSITION (slice-10; ADR-043): the results region is chrome + nav wrapped AROUND
/// [`render_philosophy_fragment`] — the EXACT same fragment fn the htmx shape returns
/// alone. Because the results region is the SAME fn in both shapes, fragment/full-page
/// parity is structural, not asserted by duplicating render logic (I-GT-6). The `<head>`
/// emits exactly ONE local `<script src="/static/htmx.min.js">` (offline-first).
pub fn render_philosophy_page(view: &TraversalView) -> String {
    // slice-21 (ADR-058 D6): composed through `page_shell` (persistent left nav +
    // `<main id="viewer-main">`); `active = PHILOSOPHY_URL` (the base-path const for the
    // query-bearing `/philosophy?object=…` route) marks the Philosophy Survey nav item
    // current. The `render_*_fragment` fn is UNCHANGED (it rides `Shape::Fragment` for
    // the #traversal-results swap).
    let body = html! {
        h1 { "Philosophy Survey" }
        nav {
            a href=(MY_CLAIMS_URL) { "My Claims" }
        }
        (render_philosophy_fragment(view))
    };
    page_shell("OpenLore — Philosophy Survey", PHILOSOPHY_URL, body)
}

/// Which dimension a survey's GROUP KEY belongs to — drives the per-group traversal
/// `<a href>` route (`/project` groups BY philosophy → the key links to
/// `/philosophy`; `/philosophy` groups BY project → the key links to `/project`).
/// PURE display selector.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GroupDimension {
    /// The group key is a philosophy (the `/project` survey) → link to `/philosophy`.
    Philosophy,
    /// The group key is a project (the `/philosophy` survey) → link to `/project`.
    Project,
}

/// Render the traversal results region for the given [`TraversalView`]. PURE total
/// match over the ADT: a `Found` survey renders each attributed edge group + the
/// distinct contributors-who-claimed list; a `NoClaims` survey renders the guided
/// empty state naming the queried entity + a CLI next-step hint (no fabricated edge,
/// I-GT-4).
fn render_traversal_result(view: &TraversalView, dimension: GroupDimension) -> Markup {
    html! {
        @match view {
            TraversalView::Found {
                entity,
                groups,
                contributors,
            } => {
                h2 { "Survey of " (entity) }
                @for group in groups {
                    (render_edge_group(group, dimension))
                }
                (render_contributors(contributors))
            }
            // No-claims (US-GT-002/003 Example 3 / I-GT-4): the guided plain-language
            // empty state naming the queried entity + a CLI next-step hint — never a
            // blank region, never a fabricated edge, never a crash.
            TraversalView::NoClaims { entity } => {
                p { (TRAVERSAL_NO_CLAIMS_NOTICE) " (" (entity) ")" }
                p { (TRAVERSAL_NO_CLAIMS_HINT) }
            }
        }
    }
}

/// Render ONE edge group: the group key as a traversal `<a href>` to the OTHER
/// dimension's survey, then the per-edge attributed rows. The key href percent-encodes
/// the claim-controlled key (ADR-044 §security). The rows are NEVER averaged — one row
/// per signed claim, each under its own author DID (anti-merging, I-GT-3).
fn render_edge_group(group: &EdgeGroup, dimension: GroupDimension) -> Markup {
    let href = match dimension {
        GroupDimension::Philosophy => href_philosophy(&group.key),
        GroupDimension::Project => href_project(&group.key),
    };
    html! {
        section {
            h3 {
                a href=(href) { (group.key) }
            }
            table {
                thead {
                    tr {
                        th { "Contributor" }
                        th { "Confidence" }
                        th { "Bucket" }
                        th { "CID" }
                    }
                }
                tbody {
                    @for edge in &group.edges {
                        (render_edge_row(edge))
                    }
                }
            }
        }
    }
}

/// Render ONE attributed traversal edge row: the author DID as an `<a href>` link to
/// `/score?contributor=<bare-did>` (the slice-09 terminus REUSED; bare-DID form,
/// ADR-044 Q1), the VERBATIM confidence (via [`render_confidence`] — `0.90`, never
/// `0.9`/`90%`; I-GT-5), the REUSED display-only confidence bucket label, and the cid
/// (every edge = one signed claim, I-GT-4). The bare DID is percent-encoded into the
/// href (ADR-044). NO sign/follow control (I-GT-1 — the link is render-only TEXT).
///
/// SHARED edge arm (slice-13 / US-CF-003): consumed by BOTH `render_project_fragment`
/// and `render_philosophy_fragment` (via `render_edge_group`). It appends — ONLY when
/// `is_countered` — a render-only `<a href="/claims/{cid}">Countered</a>` ONE-HOP link
/// to that claim's slice-11 thread INSIDE the cid `<td>`, REUSING the shared
/// [`COUNTERED_PRESENCE_FLAG`] (one SSOT with the list + detail surfaces). An UN-countered
/// edge renders NOTHING extra — no marker, no "0 counters" noise (I-CF-2). The flag is
/// ADDITIVE context beside the edge: it changes nothing about which edges appear, in
/// which group, in which order (I-CF-9).
fn render_edge_row(edge: &EdgeRow) -> Markup {
    html! {
        tr {
            td {
                a href=(href_score(&edge.author_did)) { (edge.author_did) }
            }
            td { (render_confidence(edge.confidence)) }
            td { (render_confidence_bucket(edge.confidence)) }
            td {
                (edge.cid)
                (render_edge_presence_flag(edge))
            }
        }
    }
}

/// Render the slice-13 "Countered" PRESENCE flag for ONE traversal edge (US-CF-003 /
/// I-CF-6) — appended inside the edge's cid `<td>`. A thin surface-typed wrapper over the
/// shared [`render_countered_link`] SSOT body, REUSING the shared [`COUNTERED_PRESENCE_FLAG`]
/// (one SSOT with the list + peer presence flags).
fn render_edge_presence_flag(edge: &EdgeRow) -> Markup {
    render_countered_link(&edge.cid, edge.is_countered)
}

/// Render the distinct "Contributors who claimed" list: each contributor DID as an
/// `<a href>` link to `/score?contributor=<bare-did>` (the slice-09 terminus REUSED).
/// A spanning contributor appears ONCE (the list is already deduped in
/// [`group_by`]). Render-only navigation TEXT — no executable control (I-GT-1).
fn render_contributors(contributors: &[String]) -> Markup {
    html! {
        h3 { "Contributors who claimed" }
        ul {
            @for did in contributors {
                li {
                    a href=(href_score(did)) { (did) }
                }
            }
        }
    }
}
