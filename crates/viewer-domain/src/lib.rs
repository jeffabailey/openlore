//! `viewer-domain` — the PURE read-only viewer core (slice-06; ADR-029).
//!
//! Holds the view-model ADTs the `openlore ui` `/claims` route projects from the
//! read-only `StoreReadPort`, plus the `render_claims_page` server-rendered-HTML
//! function (maud, compile-time HTML macro). PURE: zero I/O, no knowledge of
//! DuckDB/HTTP/hyper/the network. The effect shell (`adapter-http-viewer`) reads
//! the store, builds a [`PageView`], and calls [`render_claims_page`] — this
//! crate never touches a socket or a file.
//!
//! ## View-model (nw-fp-domain-modeling §10 — persistence ignorance)
//!
//! The viewer has two type hierarchies: the boundary DTOs in `ports`
//! ([`ports::ClaimRow`], flat, from the store) and the VIEW-model here
//! ([`ClaimRowView`], shaped for rendering). The effect shell converts
//! `ClaimRow -> ClaimRowView` (always succeeds — [`ClaimRowView::from_row`]), so
//! the renderer stays a total pure function over an already-shaped view-model.
//!
//! ## Confidence renders VERBATIM (FR-VIEW-8 — the prime mutation target)
//!
//! The stored confidence is a DOUBLE (`f64`). The operator sees it rendered
//! VERBATIM as `0.90` (two decimal places) — NEVER `0.9`, NEVER `90%`. This is
//! [`render_confidence`]; it is the load-bearing UX contract the V-1 walking
//! skeleton + the in-crate property tests below pin.

#![forbid(unsafe_code)]

use maud::{html, Markup, DOCTYPE};
use ports::{
    AuthorRelationship, CandidateClaim, ClaimDetail, ClaimRow, CounterClaimRow, PeerClaimRow,
    PeerOrigin, SurveyRow,
};
// The PURE slice-04 `scoring` core is REUSED for the `/score` view-model
// projection (ADR-039): the renderer projects the `WeightedView` (ranked
// `WeightedPairing`s + their per-claim `Contribution` decomposition) + the
// display-only `WeightBucket`, referenced via the `scoring::` path. The scoring
// math is the pure core's job — never reimplemented here.

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

/// A page of view-model rows ready to render, carrying the pagination bounds the
/// position indicator + Next/Prev controls (FR-VIEW-6) project from. The
/// arithmetic over (`total`, `page`, `page_size`) is PURE and TOTAL — it is the
/// single richest mutation surface in the viewer (step 04-01), pinned by the
/// property tests below.
///
/// - `page` is 1-based (the operator's `?page=N`; the effect shell clamps invalid
///   / `<= 0` input to 1 before constructing this).
/// - `page_size` is the fixed rows-per-page (50, ADR-030).
/// - `total` is the `COUNT(*)` of the whole result set (NOT `rows.len()` — the
///   last page holds fewer rows than `page_size`).
#[derive(Debug, Clone, PartialEq)]
pub struct PageView<T> {
    pub rows: Vec<T>,
    /// 1-based page number the operator is viewing.
    pub page: u64,
    /// Fixed rows-per-page (50, ADR-030).
    pub page_size: u64,
    /// Total matching rows across all pages (`COUNT(*)`).
    pub total: u64,
}

/// Clamp a requested 1-based `page` into the valid range `[1, last_page]` for a
/// result set of `total` rows at `page_size` per page (step 04-02 / AC-004.4).
/// PURE + TOTAL. `last_page = ceil(total / page_size)`; a page past it resolves
/// to `last_page` (the page-beyond-last CLAMP — never an error, never a broken
/// overshoot indicator). An EMPTY set (`total == 0`) has no last page, so the
/// clamp resolves to 1 (the single guided page) — never 0, which would underflow
/// [`PageView::start`]'s `(page - 1) * page_size`. `page_size` is floored at 1 so
/// the ceiling never divides by zero.
fn clamp_page(page: u64, page_size: u64, total: u64) -> u64 {
    if total == 0 {
        return 1;
    }
    let last_page = total.div_ceil(page_size.max(1));
    page.clamp(1, last_page)
}

impl<T> PageView<T> {
    /// Construct a SINGLE-page view from its rows (no pagination): page 1, the
    /// page size equal to the row count, total equal to the row count. Used by
    /// surfaces that render one ungated page (the Peer Claims view) and by tests
    /// — [`Self::start`]/[`Self::end`] then read `1–N of N`, [`Self::has_prev`] /
    /// [`Self::has_next`] are both false (no controls).
    pub fn new(rows: Vec<T>) -> Self {
        let total = rows.len() as u64;
        Self {
            rows,
            page: 1,
            page_size: total.max(1),
            total,
        }
    }

    /// Construct a PAGINATED view (step 04-01): the rows for `page` (1-based), the
    /// fixed `page_size`, and the whole-set `total`. The effect shell reads one
    /// page from the store (`OFFSET (page-1)*page_size LIMIT page_size`) and the
    /// `COUNT(*)` total, then builds this so the renderer projects the indicator +
    /// controls from PURE arithmetic.
    ///
    /// The requested `page` is CLAMPED to the valid range `[1, last_page]` (step
    /// 04-02 / AC-004.4): a page PAST the last (e.g. `?page=999` over 312 rows)
    /// resolves to the LAST page rather than erroring or rendering a broken
    /// `49901–312 of 312` over an empty page — the operator lands on the bounded
    /// last page. An EMPTY result set (`total == 0`) has no last page, so the clamp
    /// resolves to page 1 (the single guided page), never page 0.
    pub fn paged(rows: Vec<T>, page: u64, page_size: u64, total: u64) -> Self {
        Self {
            rows,
            page: clamp_page(page, page_size, total),
            page_size,
            total,
        }
    }

    /// The 1-based ordinal of the FIRST row shown on this page (the `start` of the
    /// `start–end of total` indicator, FR-VIEW-6 / AC-004.4):
    /// `start = (page - 1) * page_size + 1`. Returns `0` for an EMPTY result set
    /// (`total == 0`) so the renderer shows the guided empty state, not `1–0 of 0`.
    pub fn start(&self) -> u64 {
        if self.total == 0 {
            0
        } else {
            (self.page - 1) * self.page_size + 1
        }
    }

    /// The 1-based ordinal of the LAST row shown on this page (the `end` of the
    /// indicator, FR-VIEW-6 / AC-004.4): `end = min(page * page_size, total)` —
    /// the last page is BOUNDED by `total` (AC-004.2), never overshoots.
    pub fn end(&self) -> u64 {
        (self.page * self.page_size).min(self.total)
    }

    /// Whether a PREVIOUS page exists — i.e. the operator is past page 1
    /// (`page > 1`). Drives the Prev control's presence (FR-VIEW-6): absent on the
    /// first page.
    pub fn has_prev(&self) -> bool {
        self.page > 1
    }

    /// Whether a NEXT page exists — i.e. this page does not reach `total`
    /// (`end < total`). Drives the Next control's presence (FR-VIEW-6): absent on
    /// the last page (AC-004.2), and absent entirely when the whole set fits one
    /// page (AC-004.3).
    pub fn has_next(&self) -> bool {
        self.end() < self.total
    }
}

/// The HTML `id` of the My Claims swap-target element — the `<div>` the htmx
/// fragment IS, and the region the full page wraps chrome around (slice-07;
/// ADR-032/033). Held in ONE place so the fragment fn and any future
/// `hx-target`/`hx-swap` reference the SAME id (a mutation to the id has exactly
/// one site to attack — pinned by the unit test). htmx swaps the element whose id
/// matches; the no-JS full page embeds the SAME `<div id="claims-table">` so the
/// two shapes are structurally identical inside the swap target (I-HX-5 parity by
/// construction).
pub const CLAIMS_TABLE_ID: &str = "claims-table";

/// The HTML `id` of the active VIEW-PANEL swap-target element (slice-07 H-6a;
/// ADR-034 / DESIGN §6) — the OUTER container the My Claims ↔ Peer Claims tab
/// switch lands on (`hx-target="#view-panel"`). It WRAPS the inner
/// [`CLAIMS_TABLE_ID`] region, so the two swap behaviors compose: the tab switch
/// replaces the panel's inner content (the active list) while paging targets the
/// nested `#claims-table` (UNCHANGED — H-1a/H-2a still land). Held in ONE place so
/// the fragment fn, the page slot, and the tab anchors' `hx-target` all reference
/// the SAME id (a mutation has exactly one site to attack — pinned by the unit
/// test). The full page embeds the SAME view-panel fragment fn, so the fragment
/// and the full page's panel region are structurally identical (I-HX-5 parity).
pub const VIEW_PANEL_ID: &str = "view-panel";

/// The real route the My Claims tab links to (`/claims`) — the no-JS `href`, the
/// htmx `hx-get`, AND the URL `hx-push-url` pushes into history are ALL this one
/// path (ADR-034: one source of truth for "where am I"). Held in ONE place so the
/// tab anchor's three URL references can never drift apart.
pub const MY_CLAIMS_URL: &str = "/claims";

/// The real route the Peer Claims tab links to (`/peer-claims`) — the no-JS
/// `href`, the htmx `hx-get`, AND the pushed URL are ALL this one path (ADR-034).
/// Held in ONE place so the tab anchor's three URL references stay identical.
pub const PEER_CLAIMS_URL: &str = "/peer-claims";

/// The LOCAL route the viewer serves the vendored htmx library from
/// (`/static/htmx.min.js`) — served by the viewer ITSELF, NEVER a CDN
/// (offline-first; I-HX-2 / ADR-031). Held in ONE place so the path the chrome
/// references and the route the effect shell serves cannot drift apart.
pub const HTMX_ASSET_URL: &str = "/static/htmx.min.js";

/// Emit the SINGLE local htmx `<script src>` chrome line every enhanced page
/// carries (`<script src="/static/htmx.min.js">`, offline-first — never a CDN;
/// I-HX-2 / ADR-031). PURE total function. Extracted so the local-asset contract
/// lives in ONE place: all five page renderers (landing / claims / detail / peer /
/// scrape) embed THIS fn rather than each spelling out the `<script src>`, so a
/// mutation to the asset reference has exactly one site to attack (pinned by the
/// per-page chrome unit tests).
fn htmx_script() -> Markup {
    html! {
        script src=(HTMX_ASSET_URL) {}
    }
}

/// Emit the common full-page `<head>` chrome shared by the enhanced page renderers
/// (landing / claims / detail / peer / scrape): the UTF-8 charset meta, the page
/// `title`, and the single local [`htmx_script`] line. PURE total function over the
/// per-page `title`. Extracted so the head shape + the offline-first htmx contract
/// live in ONE place (page = `page_head(title)` + body), rather than being repeated
/// verbatim across five renderers.
fn page_head(title: &str) -> Markup {
    html! {
        head {
            meta charset="utf-8";
            title { (title) }
            (htmx_script())
        }
    }
}

/// Render the My Claims ↔ Peer Claims TAB navigation (slice-07 H-6a; ADR-034).
/// PURE total function — emits ordinary markup, header-unaware. Each tab is a real
/// `<a href>` (the no-JS path: a plain link → full-page navigation that changes
/// the browser URL natively) ENHANCED with htmx attributes on the SAME anchor:
/// `hx-get` (= the same URL it links to), `hx-target="#view-panel"` (the tab swaps
/// the active view panel, NOT the inner `#claims-table` — that is paging),
/// `hx-swap="innerHTML"` (replace the panel's inner content), and
/// `hx-push-url="true"` (htmx pushes the fetched URL into history, so the address
/// bar shows the real route — bookmark/Back/reload all behave like a direct
/// navigation, ADR-034 / FR-HX-4). The `href` == `hx-get` == the pushed URL, so
/// there is ONE source of truth for the current view (the path); reloading that
/// URL re-enters the FULL page (no `HX-Request`, ADR-033). The pure core stays
/// unaware of HTTP — these are static attribute strings shared by both pages.
pub fn render_tab_nav() -> Markup {
    html! {
        nav {
            a href=(MY_CLAIMS_URL)
              hx-get=(MY_CLAIMS_URL)
              hx-target=(format!("#{VIEW_PANEL_ID}"))
              hx-swap="innerHTML"
              hx-push-url="true" { "My Claims" }
            a href=(PEER_CLAIMS_URL)
              hx-get=(PEER_CLAIMS_URL)
              hx-target=(format!("#{VIEW_PANEL_ID}"))
              hx-swap="innerHTML"
              hx-push-url="true" { "Peer Claims" }
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

/// Render the Peer Claims VIEW-PANEL swap-target FRAGMENT (slice-07 H-6a; ADR-034 /
/// DESIGN §6): the `<div id="view-panel">` wrapping the Peer Claims active list
/// (the inner [`render_peer_claims_table_fragment`], itself the `#claims-table`
/// region). This is what an `HX-Request` tab switch to `/peer-claims` lands on
/// (`hx-target="#view-panel"`, `hx-swap="innerHTML"`) — the contract H-6a pins.
/// PURE total function — NO full-page chrome (I-HX-1). Because it EMBEDS the same
/// `#claims-table` peer fragment fn, the inner peer paging swap
/// (`hx-target="#claims-table"`, H-2a) still lands on the nested region — the tab
/// switch (outer `#view-panel`) and paging (inner `#claims-table`) compose without
/// conflict (DESIGN §6: the peer table is "inside #view-panel"). The full page
/// EMBEDS this SAME view-panel fn, so fragment/page parity is structural (I-HX-5).
pub fn render_peer_claims_view_panel_fragment(page: &PageView<PeerClaimRowView>) -> Markup {
    html! {
        div id=(VIEW_PANEL_ID) {
            (render_peer_claims_table_fragment(page))
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
pub fn render_claims_page(page: &PageView<ClaimRowView>) -> String {
    let markup = html! {
        (DOCTYPE)
        html {
            (page_head("OpenLore — My Claims"))
            body {
                h1 { "My Claims" }
                p { "This is a read-only view of the claims you have signed." }
                (render_tab_nav())
                (render_claims_view_panel_fragment(page))
            }
        }
    };
    markup.into_string()
}

/// Format the position indicator `start–end of total` (FR-VIEW-6 / AC-004.4),
/// e.g. `1–50 of 312`. PURE total function over the page bounds — the EN DASH
/// (U+2013, `–`) separates the range (a mutation to a hyphen fails the acceptance
/// assertion). Held in ONE place so the exact indicator text is a single mutation
/// site. Returns the empty string for an empty result set (`total == 0`) so the
/// guided empty state stands alone.
pub fn render_position_indicator<T>(page: &PageView<T>) -> String {
    if page.total == 0 {
        String::new()
    } else {
        format!("{}\u{2013}{} of {}", page.start(), page.end(), page.total)
    }
}

/// Render the pagination block for the My Claims list (FR-VIEW-6): the position
/// indicator (`start–end of total`) plus the Prev/Next anchor links to
/// `?page=N\u{00b1}1`. PURE total function over the [`PageView`] bounds.
///
/// - An EMPTY result set (`total == 0`) renders NOTHING — the guided empty state
///   stands alone, with no indicator and no controls (AC-001.3).
/// - A store that fits ONE page (`!has_prev && !has_next`) shows the indicator but
///   NO `?page=` controls (AC-004.3).
/// - Prev links to `?page={page-1}` only when [`PageView::has_prev`]; Next links to
///   `?page={page+1}` only when [`PageView::has_next`] (absent on the last page,
///   AC-004.2).
fn render_pagination<T>(page: &PageView<T>) -> Markup {
    html! {
        @if page.total > 0 {
            nav {
                p { (render_position_indicator(page)) }
                @if page.has_prev() {
                    a href=(format!("?page={}", page.page - 1)) { "Previous" }
                }
                @if page.has_next() {
                    a href=(format!("?page={}", page.page + 1)) { "Next" }
                }
            }
        }
    }
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
/// US-LF-002 / ADR-048): a render-only `<a href="/claims/{cid}">Countered</a>`
/// one-hop link to that claim's slice-11 counter thread — navigation TEXT, never an
/// executable write/sign/counter control (I-LF-1). PRESENCE-only: a single neutral
/// marker, NEVER a count ("disputed by N") or a verdict. An UN-countered row
/// (`is_countered == false`) renders NOTHING — no marker, no "0 counters" noise
/// (no-noise discipline, I-LF-2). PURE total function over the row's `is_countered`
/// flag, so the render is a total function of (page, presence). The flag text is the
/// shared [`COUNTERED_PRESENCE_FLAG`] constant (one source of truth with the detail
/// view's presence flag).
fn render_list_presence_flag(row: &ClaimRowView) -> Markup {
    html! {
        @if row.is_countered {
            a href=(format!("/claims/{}", row.cid)) { (COUNTERED_PRESENCE_FLAG) }
        }
    }
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

/// Format the stored confidence DOUBLE VERBATIM for display (FR-VIEW-8): two
/// decimal places, so `0.9` renders as `"0.90"` — NEVER `"0.9"`, NEVER `"90%"`.
/// This is the prime mutation-survivor target; the in-crate property test below
/// pins it across the whole `[0.0, 1.0]` domain.
pub fn render_confidence(confidence: f64) -> String {
    format!("{confidence:.2}")
}

/// The exact read-only assurance phrase the operator sees on both the launch
/// banner AND the landing page (AC-001.2 / NFR-VIEW-1). Held in ONE place so the
/// "read-only" contract text lives at a single source of truth (and so a string
/// mutation has exactly one site to attack — pinned by the unit tests below).
pub const READ_ONLY_NOTICE: &str = "This viewer is read-only — nothing here can change your store.";

/// Render the read-only launch banner the `openlore ui` verb prints to stdout at
/// startup (AC-001.2). PURE: a total function from the bound loopback address to
/// the human-readable launch notice. States, up front, (a) the loopback listen
/// URL the operator opens in a browser, and (b) that the view is read-only and
/// loads no signing key.
///
/// `bound_addr` is the address the server actually bound (e.g. `127.0.0.1:8080`);
/// it is rendered as an `http://` loopback URL. Formatting lives here (not in the
/// effect shell) so it is unit/property-testable and the exact strings are pinned
/// against mutation.
pub fn read_only_launch_banner(bound_addr: &str) -> String {
    format!(
        "OpenLore viewer listening on {} — {} No signing key is loaded.",
        loopback_url(bound_addr),
        READ_ONLY_NOTICE,
    )
}

/// Format a bound socket address as the loopback URL the operator opens in a
/// browser: `127.0.0.1:8080` -> `http://127.0.0.1:8080`. PURE. Kept tiny + named
/// so the `http://` scheme prefix is pinned in one place (a mutation dropping the
/// scheme fails the unit test).
pub fn loopback_url(bound_addr: &str) -> String {
    format!("http://{bound_addr}")
}

/// Render the viewer's landing page (`GET /`) as a complete HTML document (maud).
/// PURE: a total function — no I/O. States the view is read-only (the operator is
/// told, up front, that nothing here can change her store — NFR-VIEW-1) and links
/// to the My Claims list. The read-only assurance text is [`READ_ONLY_NOTICE`],
/// shared verbatim with the launch banner.
pub fn render_landing() -> String {
    let markup = html! {
        (DOCTYPE)
        html {
            (page_head("OpenLore — Viewer"))
            body {
                h1 { "OpenLore Viewer" }
                p { (READ_ONLY_NOTICE) }
                p {
                    a href="/claims" { "View my claims" }
                }
            }
        }
    };
    markup.into_string()
}

/// The EXACT plain-language message the operator sees when she opens a detail
/// page for a CID that is not in her store (AC-002.3 / FR-VIEW-3 / NFR-VIEW-6).
/// Held in ONE place so the not-found phrasing is a single source of truth and a
/// string mutation has exactly one site to attack (pinned by the unit test).
pub const CLAIM_NOT_FOUND_NOTICE: &str = "No claim with that identifier in your store";

/// Render the guided not-found page for an unknown CID (`GET /claims/{cid}` where
/// `get_claim` returns `None`; AC-002.3 / FR-VIEW-3). PURE: a total function — no
/// I/O, takes no error value (it never echoes a raw cause). Shows the
/// plain-language [`CLAIM_NOT_FOUND_NOTICE`] plus a back link to the My Claims
/// list so the operator's next step is obvious — never a blank page, never a
/// stack trace (NFR-VIEW-6). The effect shell maps this body to a `404` status.
pub fn render_error() -> String {
    let markup = html! {
        (DOCTYPE)
        html {
            head {
                meta charset="utf-8";
                title { "OpenLore — Claim Not Found" }
            }
            body {
                h1 { "Claim Not Found" }
                p { (CLAIM_NOT_FOUND_NOTICE) }
                p {
                    a href="/claims" { "Back to My Claims" }
                }
            }
        }
    };
    markup.into_string()
}

/// Render the guided not-found swap-target FRAGMENT for an unknown CID (slice-07
/// H-4c; ADR-032/033). The `<div id="claim-detail">` wrapping the plain-language
/// [`CLAIM_NOT_FOUND_NOTICE`] + a `/claims` back link — the SAME region a found
/// detail would swap into, so an `HX-Request` 404 replaces the detail panel with
/// the guidance in place. PURE: a total function — no I/O, takes no error value
/// (it never echoes a raw cause; NFR-VIEW-6). NO full-page chrome (no `<!DOCTYPE>`,
/// no `<html>`/`<head>`), so the htmx 404 carries ONLY this region (I-HX-1). The
/// effect shell maps both shapes' not-found body to a `404` status; the no-JS
/// shape uses the full-page [`render_error`] instead, which carries the SAME
/// message + back link so the two shapes agree (I-HX-5).
pub fn render_claim_not_found_fragment() -> Markup {
    html! {
        div id=(CLAIM_DETAIL_ID) {
            p { (CLAIM_NOT_FOUND_NOTICE) }
            p {
                a href="/claims" { "Back to My Claims" }
            }
        }
    }
}

/// One claim's FULL detail, shaped for the `/claims/{cid}` detail render
/// (US-VIEW-002). The VIEW-model (nw-fp-domain-modeling §10): flat display
/// strings + the numeric confidence the renderer formats VERBATIM + the
/// ordinal-ordered evidence URLs. Projected from a [`ports::ClaimDetail`] by
/// [`ClaimDetailView::from_detail`] (a total conversion — always succeeds;
/// evidence ORDER is preserved from the DTO, which the adapter ordered by
/// `claim_evidence.ordinal`).
#[derive(Debug, Clone, PartialEq)]
pub struct ClaimDetailView {
    pub cid: String,
    pub subject: String,
    pub predicate: String,
    pub object: String,
    /// The stored confidence DOUBLE. Rendered VERBATIM via [`render_confidence`]
    /// (FR-VIEW-8).
    pub confidence: f64,
    pub author_did: String,
    /// `composed_at` rendered as an RFC-3339 string by the effect shell (the
    /// pure renderer shows it verbatim; held as a string so this crate takes no
    /// `chrono` dependency edge).
    pub composed_at: String,
    /// The evidence URLs in attachment order (ordinal ascending). Empty for a
    /// claim signed without evidence (the renderer then shows an explicit "no
    /// evidence attached" state — step 02-02).
    pub evidence: Vec<String>,
}

impl ClaimDetailView {
    /// Project a boundary [`ports::ClaimDetail`] into the detail view-model.
    /// Total — never fails. `composed_at` is rendered to RFC-3339 here so the
    /// pure renderer needs no `chrono`. Evidence ORDER is carried through
    /// unchanged (the adapter already ordered by `ordinal`).
    pub fn from_detail(detail: &ClaimDetail) -> Self {
        Self {
            cid: detail.cid.clone(),
            subject: detail.subject.clone(),
            predicate: detail.predicate.clone(),
            object: detail.object.clone(),
            confidence: detail.confidence,
            author_did: detail.author_did.clone(),
            composed_at: detail.composed_at.to_rfc3339(),
            evidence: detail.evidence.clone(),
        }
    }
}

/// The HTML `id` of the claim-detail swap-target element — the `<div>` the htmx
/// detail fragment IS, and the region the full detail page wraps chrome around
/// (slice-07 H-4a; ADR-032/033). Held in ONE place so the fragment fn and any
/// future `hx-target`/`hx-swap` reference the SAME id (a mutation to the id has
/// exactly one site to attack — pinned by the unit test). htmx swaps the element
/// whose id matches; the no-JS full page embeds the SAME `<div id="claim-detail">`
/// so the two shapes are structurally identical inside the swap target (I-HX-5
/// parity by construction).
pub const CLAIM_DETAIL_ID: &str = "claim-detail";

/// The exact "no reason provided" state text rendered for a counter whose
/// free-text `reason` is absent (the ADR-015 wire-optional empty-reason edge,
/// CT-6 / ADR-047). Held in ONE place so the empty-reason phrasing is a single
/// source of truth and a string mutation has exactly one site to attack.
pub const COUNTER_NO_REASON_NOTICE: &str = "no reason provided";

/// The neutral "Countered" PRESENCE flag rendered near a claim that has ≥1
/// counter (CT-8 / I-CT-3): a presence marker ONLY — never a verdict, a score, a
/// count ("disputed by N"), or a count-based re-rank. Held in ONE place so the
/// flag text is a single source of truth.
pub const COUNTERED_PRESENCE_FLAG: &str = "Countered";

/// The counter-thread section heading rendered above the attributed counter
/// entries (slice-11 / US-CT-002). Held in ONE place; absent entirely when a
/// claim is un-countered ([`CounterThread::None`] renders nothing, I-CT-2).
pub const COUNTER_THREAD_HEADING: &str = "Counter-claims";

/// One attributed counter in a [`CounterThread`] — the VIEW-model for a single
/// counter rendered BENEATH the verbatim claim (slice-11 / US-CT-002 / ADR-047).
/// Names the counter's author DID + its own CID (a render-only one-hop drill-link
/// toward `/claims/{cid}`, depth-1) + its verbatim free-text `reason` (`None` →
/// the explicit "no reason provided" state). `is_own` distinguishes the
/// operator's own counter from a peer's (display-only; never a re-weight).
#[derive(Debug, Clone, PartialEq)]
pub struct CounterEntry {
    /// The counter author's DID — rendered VERBATIM as attribution (anti-merging,
    /// I-CT-3): never elided, never merged into a faceless aggregate.
    pub author_did: String,
    /// The counter's own content-addressed CID — the render-only
    /// `<a href="/claims/{cid}">` one-hop drill-link target (depth-1, ADR-047).
    pub cid: String,
    /// The counter's verbatim free-text reason; `None` → the explicit
    /// [`COUNTER_NO_REASON_NOTICE`] state (the ADR-015 wire-optional edge).
    pub reason: Option<String>,
    /// Whether this counter is the operator's OWN (display-only). Derived from the
    /// peer ORIGIN (an own counter carries an empty `fetched_from_pds`).
    pub is_own: bool,
}

/// The counter-claim thread for one claim (slice-11 / US-CT-002 / ADR-047): the
/// PURE ADT the detail render threads BENEATH the verbatim claim. Total at the
/// type level so the "no-noise for an un-countered claim" contract (I-CT-2) is
/// structural — an un-countered claim is `None` and renders NOTHING extra (no
/// section, no flag, no "0 counters" empty-state). A countered claim is
/// `Countered { counters }` with ≥1 attributed [`CounterEntry`]; the counters are
/// SHOWN, never APPLIED — they never re-weight/filter/merge the claim above them
/// (shown-never-applied, I-CT-2).
#[derive(Debug, Clone, PartialEq)]
pub enum CounterThread {
    /// The claim is UN-countered — `query_counter_claims` returned an empty vec.
    /// Renders NOTHING extra (no section, no flag, no empty-state noise; I-CT-2).
    None,
    /// The claim has ≥1 counter. Each is an attributed [`CounterEntry`] rendered
    /// beneath the verbatim claim; the count is the length of `counters` (the
    /// thread is per-counter, NEVER a merged "disputed by N" aggregate, I-CT-3).
    Countered { counters: Vec<CounterEntry> },
}

impl CounterThread {
    /// Project the boundary [`ports::CounterClaimRow`]s (the ADR-046 2-step read
    /// output) into the pure [`CounterThread`] ADT — a TOTAL conversion, always
    /// succeeds. An EMPTY slice yields [`CounterThread::None`] (the un-countered
    /// no-noise case, I-CT-2); a non-empty slice yields [`CounterThread::Countered`]
    /// preserving the adapter's deterministic order. `is_own` is derived from the
    /// counter's ORIGIN: an own counter carries `PeerOrigin::Known { fetched_from_pds:
    /// "" }` (empty PDS); a pulled peer counter carries its PDS endpoint. The
    /// grouping/attribution is NEVER recomputed here — each row maps to exactly one
    /// entry (anti-merging by construction, I-CT-3).
    pub fn from_rows(rows: &[CounterClaimRow]) -> Self {
        if rows.is_empty() {
            return CounterThread::None;
        }
        let counters = rows
            .iter()
            .map(|row| CounterEntry {
                author_did: row.author_did.clone(),
                cid: row.cid.clone(),
                reason: row.reason.clone(),
                is_own: counter_is_own(&row.origin),
            })
            .collect();
        CounterThread::Countered { counters }
    }
}

/// True when a counter's ORIGIN marks it as the operator's OWN (display-only):
/// an own counter is a `PeerOrigin::Known` with an EMPTY `fetched_from_pds` (the
/// adapter's own arm sets `'' AS fetched_from_pds`); a pulled peer counter carries
/// a non-empty PDS endpoint, and an `Unknown` origin is never "own".
fn counter_is_own(origin: &PeerOrigin) -> bool {
    matches!(
        origin,
        PeerOrigin::Known {
            fetched_from_pds, ..
        } if fetched_from_pds.is_empty()
    )
}

/// Render the claim-detail swap-target FRAGMENT (slice-07 H-4a; ADR-032/033): the
/// `<div id="claim-detail">` wrapping EVERY claim field (subject, predicate,
/// object, the VERBATIM confidence, author_did, composed_at, CID) PLUS the
/// COMPLETE `evidence[]` array, one URL per row in ordinal order (FR-VIEW-3 /
/// AC-002.1) — and, for a claim with no evidence, the explicit "no evidence
/// attached" state (step 02-02) rather than a blank section. PURE: a total
/// function from the detail view-model to a `Markup` — NO full-page chrome (no
/// `<!DOCTYPE>`, no `<html>`/`<head>`), so an `HX-Request` response carries ONLY
/// this region (I-HX-1). [`render_claim_detail`] EMBEDS this SAME fn inside its
/// chrome, so the fragment and the full page's detail region are byte-identical by
/// construction (I-HX-5 parity — the field/evidence-rendering logic is NOT
/// duplicated). This is the load-bearing slice-07 structural contract: page =
/// chrome + fragment.
pub fn render_claim_detail_fragment(claim: &ClaimDetailView, thread: &CounterThread) -> Markup {
    html! {
        div id=(CLAIM_DETAIL_ID) {
            (render_presence_flag(thread))
            (render_claim_fields(claim))
            (render_evidence_section(&claim.evidence))
            (render_counter_thread(thread))
        }
    }
}

/// Render the neutral "Countered" PRESENCE flag for a claim that has ≥1 counter
/// (CT-8 / I-CT-3): a presence marker ONLY — never a verdict, score, or count.
/// An UN-countered claim ([`CounterThread::None`]) renders NOTHING (no flag, no
/// noise; I-CT-2). PURE total function over the thread ADT.
fn render_presence_flag(thread: &CounterThread) -> Markup {
    html! {
        @if let CounterThread::Countered { .. } = thread {
            p { (COUNTERED_PRESENCE_FLAG) }
        }
    }
}

/// Render the counter-claim thread BENEATH the verbatim claim (slice-11 /
/// US-CT-002 / ADR-047): one attributed entry per counter — its author DID, its
/// own CID as a render-only `<a href="/claims/{cid}">` one-hop drill-link
/// (depth-1, NO nested/recursive counter render), and its verbatim free-text
/// reason (or the explicit "no reason provided" state for the empty-reason edge).
/// The entries are SHOWN, never APPLIED — they never re-weight/filter/merge the
/// claim above (shown-never-applied, I-CT-2), and never collapse into a merged
/// "disputed by N" aggregate (anti-merging, I-CT-3). An UN-countered claim
/// ([`CounterThread::None`]) renders NOTHING — no section, no empty-state noise.
/// PURE total function over the thread ADT.
fn render_counter_thread(thread: &CounterThread) -> Markup {
    html! {
        @if let CounterThread::Countered { counters } = thread {
            section {
                h2 { (COUNTER_THREAD_HEADING) }
                ul {
                    @for entry in counters {
                        li {
                            (render_counter_entry(entry))
                        }
                    }
                }
            }
        }
    }
}

/// Render one counter entry: its author DID (verbatim attribution), its own CID
/// as a render-only one-hop drill-link toward `/claims/{cid}` (depth-1, ADR-047),
/// and its verbatim reason (or [`COUNTER_NO_REASON_NOTICE`] when absent). The
/// drill-link is navigation TEXT only — the viewer offers NO write/sign/counter
/// control (I-CT-1). PURE total function over the entry.
fn render_counter_entry(entry: &CounterEntry) -> Markup {
    let drill_href = format!("/claims/{}", entry.cid);
    html! {
        dl {
            dt { "Counter author" } dd { (entry.author_did) }
            dt { "Counter CID" }
            dd {
                a href=(drill_href) { (entry.cid) }
            }
            dt { "Reason" }
            dd {
                @match &entry.reason {
                    Some(reason) => (reason),
                    None => (COUNTER_NO_REASON_NOTICE),
                }
            }
        }
    }
}

/// Render one claim's detail page as a complete HTML document (maud). PURE: a
/// total function from the detail view-model to an HTML string — no I/O. Shows
/// EVERY claim field (subject, predicate, object, the VERBATIM confidence,
/// author_did, composed_at, CID) PLUS the COMPLETE `evidence[]` array, one URL
/// per row in ordinal order (FR-VIEW-3 / AC-002.1). A claim with no evidence
/// shows an explicit "no evidence attached" state (FR-VIEW-3, step 02-02) rather
/// than a blank section.
///
/// COMPOSITION (slice-07 H-4a; ADR-032): the detail region is chrome wrapped
/// AROUND [`render_claim_detail_fragment`] — the EXACT same fragment fn the htmx
/// shape returns alone. The `<head>` emits exactly ONE local
/// `<script src="/static/htmx.min.js">` (offline-first, never a CDN; I-HX-2).
/// Because the detail region is the SAME fn in both shapes, fragment/full-page
/// parity is structural, not asserted by duplicating render logic (I-HX-5).
pub fn render_claim_detail(claim: &ClaimDetailView, thread: &CounterThread) -> String {
    let markup = html! {
        (DOCTYPE)
        html {
            (page_head("OpenLore — Claim Detail"))
            body {
                h1 { "Claim Detail" }
                p { (READ_ONLY_NOTICE) }
                (render_claim_detail_fragment(claim, thread))
                p {
                    a href="/claims" { "Back to My Claims" }
                }
            }
        }
    };
    markup.into_string()
}

/// Render the claim's scalar fields as a definition list. Each field is labeled
/// in domain language; the confidence cell goes through [`render_confidence`] so
/// the VERBATIM `0.90` rule lives in exactly one place (FR-VIEW-8).
fn render_claim_fields(claim: &ClaimDetailView) -> Markup {
    html! {
        dl {
            dt { "Subject" }    dd { (claim.subject) }
            dt { "Predicate" }  dd { (claim.predicate) }
            dt { "Object" }     dd { (claim.object) }
            dt { "Confidence" } dd { (render_confidence(claim.confidence)) }
            dt { "Author" }     dd { (claim.author_did) }
            dt { "Composed at" } dd { (claim.composed_at) }
            dt { "CID" }        dd { (claim.cid) }
        }
    }
}

/// Render the evidence section: one row per evidence URL, in the order given
/// (the adapter ordered by `claim_evidence.ordinal`, FR-VIEW-3). An EMPTY
/// evidence list renders the explicit "no evidence attached" state, never a
/// blank section (step 02-02 pins this branch).
fn render_evidence_section(evidence: &[String]) -> Markup {
    html! {
        h2 { "Evidence" }
        @if evidence.is_empty() {
            p { "no evidence attached" }
        } @else {
            ul {
                @for url in evidence {
                    li { (url) }
                }
            }
        }
    }
}

// =============================================================================
// Peer Claims view (`/peer-claims`, US-VIEW-003 / FR-VIEW-4)
// =============================================================================

/// The label shown when a peer claim's origin is absent/blank (the defensive
/// `PeerOrigin::Unknown` path, step 03-03 / V-10). Held in ONE place so the
/// "unknown" wording is a single source of truth (a string mutation has exactly
/// one site to attack — pinned by the unit test). Step 03-01 produces only
/// `Known` origins; this constant + the `Unknown` arm exist so 03-03 is a clean
/// extension.
pub const PEER_ORIGIN_UNKNOWN_LABEL: &str = "unknown";

/// One federated peer claim rendered as a row in the Peer Claims list view. The
/// VIEW-model shape (nw-fp-domain-modeling §10): flat display strings + the
/// numeric confidence + the peer ORIGIN. DISTINCT from [`ClaimRowView`] (own
/// claims) so peers render on a SEPARATE surface where "mine vs federated" is
/// never ambiguous (BR-VIEW-5). Projected from a [`ports::PeerClaimRow`] by
/// [`PeerClaimRowView::from_row`] (a total conversion — always succeeds).
#[derive(Debug, Clone, PartialEq)]
pub struct PeerClaimRowView {
    pub cid: String,
    pub subject: String,
    pub predicate: String,
    pub object: String,
    /// The stored confidence DOUBLE. Rendered VERBATIM as `0.90` by
    /// [`render_confidence`] (FR-VIEW-8).
    pub confidence: f64,
    /// The peer ORIGIN — carried through from the boundary [`PeerOrigin`] ADT so
    /// the renderer matches both arms totally (never drops a row). For `Known`,
    /// the peer's `author_did` is rendered VERBATIM (attribution discipline).
    pub origin: PeerOrigin,
    /// Whether this peer claim has ≥1 counter (slice-13 / US-CF-002 / ADR-049): the
    /// at-a-glance "Countered" PRESENCE flag, MIRRORING the slice-12 own-list flag on
    /// the FEDERATED surface. A boolean per row (presence membership, NEVER a count) —
    /// set in the EFFECT shell from the REUSED `counter_presence_for` set via
    /// [`PeerClaimRowView::from_row_with_presence`], so the pure render stays a TOTAL
    /// function of (page, presence). ADDITIVE: it NEVER changes row order/paging/count,
    /// the peer ORIGIN, or the confidence cell (shown-never-applied, I-CF-2/I-CF-4).
    pub is_countered: bool,
}

impl PeerClaimRowView {
    /// Project a boundary [`ports::PeerClaimRow`] into the view-model. Total —
    /// never fails. The peer ORIGIN ADT is carried through unchanged so the
    /// rendering rule (verbatim DID for `Known`, "unknown" label for `Unknown`)
    /// lives in ONE place ([`render_peer_origin`]). The row is UN-countered
    /// (`is_countered = false`); the slice-13 flag is set via
    /// [`Self::from_row_with_presence`] in the effect shell.
    pub fn from_row(row: &PeerClaimRow) -> Self {
        Self::from_row_with_presence(row, &std::collections::HashSet::new())
    }

    /// Project a boundary [`ports::PeerClaimRow`] into the view-model, setting the
    /// slice-13 "Countered" presence flag from `presence` (the REUSED slice-12
    /// `counter_presence_for` SET): `is_countered` is true IFF this row's CID is a
    /// member (US-CF-002 / ADR-049). A TOTAL conversion — always succeeds. The flag is
    /// ADDITIVE: every display field (including the peer ORIGIN) is identical to
    /// [`Self::from_row`]; only `is_countered` differs. The effect shell calls THIS per
    /// row AFTER `list_peer_claims` pages them, so the presence read never re-orders /
    /// re-pages / re-counts the federated list (I-CF-2).
    pub fn from_row_with_presence(
        row: &PeerClaimRow,
        presence: &std::collections::HashSet<String>,
    ) -> Self {
        Self {
            cid: row.cid.clone(),
            subject: row.subject.clone(),
            predicate: row.predicate.clone(),
            object: row.object.clone(),
            confidence: row.confidence,
            origin: row.origin.clone(),
            is_countered: presence.contains(&row.cid),
        }
    }
}

/// Render the Peer Claims page as a complete HTML document (maud). PURE: a total
/// function from the view-model to an HTML string — no I/O. Each federated peer
/// claim renders as a row carrying subject/predicate/object, the VERBATIM
/// confidence (FR-VIEW-8), its CID, and its peer ORIGIN (the peer's `author_did`,
/// rendered VERBATIM — attribution discipline, FR-VIEW-4). This is a SEPARATE
/// surface from the My Claims page (BR-VIEW-5): the heading + the explicit
/// per-row origin column make "mine vs federated" unambiguous. An empty page
/// renders the guided "No federated claims yet" empty state (FR-VIEW-7) instead
/// of a blank table.
pub fn render_peer_claims_page(page: &PageView<PeerClaimRowView>) -> String {
    let markup = html! {
        (DOCTYPE)
        html {
            (page_head("OpenLore — Peer Claims"))
            body {
                h1 { "Peer Claims" }
                p {
                    "This is a read-only view of claims federated from your peers \
                     — these are NOT your own claims."
                }
                (render_tab_nav())
                (render_peer_claims_view_panel_fragment(page))
            }
        }
    };
    markup.into_string()
}

/// Render the Peer Claims swap-target FRAGMENT (slice-07; ADR-032/033 / H-2a): the
/// `<div id="claims-table">` wrapping the peer-claims table (or the guided empty
/// state) + the position indicator + Prev/Next controls. PURE: a total function
/// from the view-model to an HTML string — NO full-page chrome (no `<!DOCTYPE>`,
/// no `<html>`/`<head>`), so an `HX-Request` response carries ONLY this region
/// (I-HX-1). [`render_peer_claims_page`] EMBEDS this SAME fn inside its chrome, so
/// the fragment and the full page's table region are byte-identical by
/// construction (I-HX-5 parity — the peer-table-rendering logic is NOT duplicated).
///
/// SWAP-TARGET id (DESIGN architecture-design.md §6): the peer table REUSES the
/// shared [`CLAIMS_TABLE_ID`] (`#claims-table`, inside `#view-panel`) so the tab
/// swap (US-HX-006) and the peer paging swap (US-HX-002) land on the SAME region;
/// the id lives in ONE place shared by the own-claims fragment + this peer
/// fragment + both pages. The Prev/Next controls reuse the SAME pure
/// [`render_pagination`] arithmetic the My Claims fragment uses (generic over the
/// row type) — peer paging threads `?page=N` through the identical machinery.
pub fn render_peer_claims_table_fragment(page: &PageView<PeerClaimRowView>) -> Markup {
    let body = if page.rows.is_empty() {
        render_peer_empty_state()
    } else {
        render_peer_claims_table(&page.rows)
    };
    html! {
        div id=(CLAIMS_TABLE_ID) {
            (body)
            (render_pagination(page))
        }
    }
}

/// Render the peer-claims table (one `<tr>` per federated claim). Small, named,
/// composable — the per-row markup is [`render_peer_claim_row`].
fn render_peer_claims_table(rows: &[PeerClaimRowView]) -> Markup {
    html! {
        table {
            thead {
                tr {
                    th { "Subject" }
                    th { "Predicate" }
                    th { "Object" }
                    th { "Confidence" }
                    th { "Peer origin" }
                    th { "CID" }
                }
            }
            tbody {
                @for row in rows {
                    (render_peer_claim_row(row))
                }
            }
        }
    }
}

/// Render one peer-claim row. The confidence cell goes through
/// [`render_confidence`] (the VERBATIM `0.90` rule lives in one place,
/// FR-VIEW-8); the origin cell goes through [`render_peer_origin`] (the
/// attribution rule lives in one place — the DID is NEVER elided, FR-VIEW-4).
fn render_peer_claim_row(row: &PeerClaimRowView) -> Markup {
    html! {
        tr {
            td { (row.subject) }
            td { (row.predicate) }
            td { (row.object) }
            td { (render_confidence(row.confidence)) }
            td { (render_peer_origin(&row.origin)) }
            td { (row.cid) (render_peer_list_presence_flag(row)) }
        }
    }
}

/// Render the at-a-glance "Countered" PRESENCE flag for one FEDERATED `/peer-claims`
/// LIST row (slice-13 / US-CF-002 / ADR-049): a render-only
/// `<a href="/claims/{cid}">Countered</a>` one-hop link to that claim's slice-11 counter
/// thread — navigation TEXT, never an executable write/sign/counter control (I-CF-1). The
/// FEDERATED-surface sibling of [`render_list_presence_flag`], emitting the SAME shared
/// [`COUNTERED_PRESENCE_FLAG`] constant (one source of truth across surfaces). PRESENCE-only:
/// a single neutral marker, NEVER a count or verdict. An UN-countered row
/// (`is_countered == false`) renders NOTHING — no marker, no "0 counters" noise (no-noise
/// discipline, I-CF-2). PURE total function over the row's `is_countered` flag, so the
/// render is a total function of (page, presence).
fn render_peer_list_presence_flag(row: &PeerClaimRowView) -> Markup {
    html! {
        @if row.is_countered {
            a href=(format!("/claims/{}", row.cid)) { (COUNTERED_PRESENCE_FLAG) }
        }
    }
}

/// Render a peer claim's ORIGIN for display (FR-VIEW-4 — the load-bearing
/// attribution + mutation target). PURE total function over the [`PeerOrigin`]
/// ADT:
///
/// - `Known` -> the peer's `author_did` VERBATIM (the operator sees exactly who
///   authored it — the DID is NEVER elided). The `fetched_from_pds` is appended
///   so the operator can see which PDS it came from.
/// - `Unknown` -> the [`PEER_ORIGIN_UNKNOWN_LABEL`] ("unknown"), so a row whose
///   origin is absent still renders labeled rather than being dropped (V-10).
///
/// Kept tiny + named so both the verbatim-DID rule and the "unknown" label have
/// exactly one site each to pin against mutation.
pub fn render_peer_origin(origin: &PeerOrigin) -> String {
    match origin {
        PeerOrigin::Known {
            author_did,
            fetched_from_pds,
        } => format!("{author_did} (via {fetched_from_pds})"),
        PeerOrigin::Unknown => PEER_ORIGIN_UNKNOWN_LABEL.to_string(),
    }
}

/// Render the guided Peer Claims empty state (FR-VIEW-7 / NFR-VIEW-6): a node
/// that has federated nothing sees the "No federated claims yet" guidance, NOT a
/// blank page (V-9 pins this branch).
fn render_peer_empty_state() -> Markup {
    html! {
        p { "No federated claims yet." }
    }
}

// =============================================================================
// Live Scrape view (`GET`/`POST /scrape`, US-VIEW-005 / FR-VIEW-5)
// =============================================================================

/// The exact copy stating that NONE of the rendered candidates were signed or
/// saved (BR-VIEW-2 / I-VIEW-1). Held in ONE place so the "nothing signed/saved"
/// contract text is a single source of truth (a string mutation has exactly one
/// site to attack — pinned by the unit test). It MUST contain both "nothing" and
/// either "signed" or "saved" (the V-S1 acceptance assertion) AND direct the
/// operator to the CLI to sign — signing stays in the CLI (BR-VIEW-1 / I-SCR-1).
pub const SCRAPE_NOTHING_SAVED_NOTICE: &str =
    "These are live proposals only — nothing here is signed or saved. To sign a \
     candidate, use the openlore CLI.";

/// The guided zero-candidates message shown when a target harvests successfully
/// but derives NO candidates (AC-005.3 / FR-VIEW-7 / NFR-VIEW-6). Held in ONE
/// place so the phrasing is a single source of truth; pinned by V-S3 + the unit
/// test. Carries a suggested alternative so the operator's next step is obvious,
/// never a blank result.
pub const SCRAPE_NO_CANDIDATES_NOTICE: &str =
    "No candidate claims could be derived from this target. Try a different \
     public repository or user.";

/// The guided NETWORK-DOWN message shown when the live propose step could not
/// reach GitHub (AC-005.4 / V-S4 / NFR-VIEW-6/7). Held in ONE place — a fixed,
/// pre-written DOMAIN-language sentence (NOT interpolated from the transport
/// error) so the message is a single source of truth AND structurally cannot
/// leak internals: it (a) names the cause in plain language ("GitHub could not
/// be reached"), (b) reassures that the offline store view "still works
/// offline" (NFR-VIEW-7), and (c) contains NO HTTP status code, NO "connection
/// refused" / "timed out" / "DNS", NO raw URL, NO stack-trace marker
/// (NFR-VIEW-6). The renderer for [`ScrapeState::NetworkDown`] emits ONLY this
/// constant — the raw [`ports::GithubError::Network`] string is NEVER threaded
/// in. Pinned by V-S4 + the leak-absence unit test.
pub const SCRAPE_NETWORK_DOWN_NOTICE: &str =
    "GitHub could not be reached, so no live proposals could be fetched. Your \
     store view still works offline — the saved claims remain available.";

/// One LIVE-SCRAPE candidate proposal rendered as a row in the Live Scrape view
/// (US-VIEW-005). The VIEW-model shape (nw-fp-domain-modeling §10): flat display
/// strings + the numeric confidence + the DISPLAY-ONLY `derived_from` provenance.
///
/// `CandidateRowView` is the ONLY view-model that carries `derived_from`
/// (WD-62 / I-VIEW-5): the persisted-claim view-models ([`ClaimRowView`],
/// [`ClaimDetailView`], [`PeerClaimRowView`]) MUST NOT — provenance is surfaced
/// ONLY on the live, never-persisted proposal. Projected from a
/// [`ports::CandidateClaim`] by [`CandidateRowView::from_candidate`] (a total
/// conversion — always succeeds; the candidate's non-empty source signals
/// become the human-readable derived-from line).
#[derive(Debug, Clone, PartialEq)]
pub struct CandidateRowView {
    pub subject: String,
    pub predicate: String,
    pub object: String,
    /// The conservative speculative default (`0.25`, WD-52). Rendered VERBATIM
    /// via [`render_confidence`] (FR-VIEW-8).
    pub confidence: f64,
    /// DISPLAY-ONLY provenance (WD-62 / I-VIEW-5): the human-readable source
    /// signal value(s) that produced this candidate. The ONLY view-model field
    /// of its kind — NEVER present on a persisted-claim view-model.
    pub derived_from: String,
}

impl CandidateRowView {
    /// Project a boundary [`ports::CandidateClaim`] into the live-scrape
    /// view-model. Total — never fails. The candidate's source signals (non-empty
    /// by `CandidateClaim::try_new`, I-SCR-4) are joined into the display-only
    /// `derived_from` provenance string.
    pub fn from_candidate(candidate: &CandidateClaim) -> Self {
        let derived_from = candidate
            .source_signals()
            .iter()
            .map(|s| s.value.as_str())
            .collect::<Vec<_>>()
            .join("; ");
        Self {
            subject: candidate.subject.clone(),
            predicate: candidate.predicate.clone(),
            object: candidate.object.clone(),
            confidence: candidate.confidence,
            derived_from,
        }
    }
}

/// The state the Live Scrape page renders (the pure render input). An ADT over
/// the three outcomes of a `/scrape` interaction so the renderer matches totally
/// (nw-fp-domain-modeling §1): the empty GET form, a populated proposal list, or
/// a guided message (zero-candidates / network-down). The effect shell builds
/// this from the live resolve+harvest+derive result; the renderer is a pure
/// total function over it.
#[derive(Debug, Clone, PartialEq)]
pub enum ScrapeState {
    /// `GET /scrape`: the empty target form, no candidates yet (AC-005.1 GET).
    Form,
    /// `POST /scrape` that derived >=1 candidate: render the proposal rows.
    Proposals(Vec<CandidateRowView>),
    /// `POST /scrape` whose target harvested successfully but derived ZERO
    /// candidates (AC-005.3 / V-S3): render the guided
    /// [`SCRAPE_NO_CANDIDATES_NOTICE`] (the no-candidates message + a suggested
    /// alternative) — never a blank result. A DISTINCT arm from
    /// [`ScrapeState::Guidance`] so the zero-candidates failure mode and the
    /// network-down one (V-S4) stay separate ADT arms — each with its own pinned,
    /// single-site copy — rather than collapsing into one generic guidance string.
    ZeroCandidates,
    /// `POST /scrape` whose live propose step could NOT reach GitHub (the
    /// transport/network failure class — `GithubError::Network`; AC-005.4 /
    /// V-S4). Renders the fixed [`SCRAPE_NETWORK_DOWN_NOTICE`]: the plain-language
    /// cause + the offline-store reassurance (NFR-VIEW-7). A DISTINCT arm from
    /// [`ScrapeState::ZeroCandidates`] and [`ScrapeState::Guidance`] so the
    /// network-down failure mode stays a separate ADT arm with its own pinned,
    /// single-site copy. Carries NO transport detail — the arm is a UNIT variant
    /// precisely so the raw error/URL/status CANNOT be interpolated, guaranteeing
    /// no leaked internals (NFR-VIEW-6) by construction.
    NetworkDown,
    /// `POST /scrape` that produced no rows for another (non-network) reason — a
    /// neutral guided message rather than a blank result. The network-down class
    /// now routes to [`ScrapeState::NetworkDown`]; this stays the catch-all for
    /// the remaining refusal classes (resolve/harvest errors other than network).
    Guidance(String),
}

/// The HTML `id` of the Live Scrape swap-target element — the `<div>` the htmx
/// scrape-results fragment IS, and the region the full `/scrape` page wraps chrome
/// (+ the form) around (slice-07; ADR-032/033). Held in ONE place so the fragment
/// fn and any future `hx-target`/`hx-swap` reference the SAME id (a mutation to the
/// id has exactly one site to attack — pinned by the unit test). htmx swaps the
/// element whose id matches; the no-JS full page embeds the SAME
/// `<div id="scrape-results">` so the two shapes are structurally identical inside
/// the swap target (I-HX-5 parity by construction).
pub const SCRAPE_RESULTS_ID: &str = "scrape-results";

/// Render the Live Scrape swap-target FRAGMENT (slice-07 H-3a; ADR-032/033): the
/// `<div id="scrape-results">` wrapping the proposal/candidate rows (or the guided
/// zero-candidates / network-down / catch-all message) for the given
/// [`ScrapeState`]. PURE: a total function from the view-model to a `Markup` — NO
/// full-page chrome (no `<!DOCTYPE>`, no `<html>`/`<head>`) and NO form, so an
/// `HX-Request` response carries ONLY this results region (I-HX-1). Renders NO
/// sign/save affordance (BR-VIEW-1 / I-SCR-1 — signing stays in the CLI).
/// [`render_scrape_page`] EMBEDS this SAME fn beneath the form, so the fragment
/// and the full page's results region are byte-identical by construction (I-HX-5
/// parity — the results-rendering logic is NOT duplicated). This is the
/// load-bearing slice-07 structural contract: page = chrome + form + fragment.
pub fn render_scrape_results_fragment(state: &ScrapeState) -> Markup {
    html! {
        div id=(SCRAPE_RESULTS_ID) {
            (render_scrape_result(state))
        }
    }
}

/// Render the Live Scrape page (`GET`/`POST /scrape`, US-VIEW-005) as a complete
/// HTML document (maud). PURE: a total function from the [`ScrapeState`] to an
/// HTML string — no I/O, no network. ALWAYS renders the labeled target form (so
/// the operator can submit / re-submit). On [`ScrapeState::Proposals`] it renders
/// the candidate rows (subject, predicate, object, the VERBATIM confidence, and
/// the DISPLAY-ONLY derived-from provenance on EACH row) plus the "nothing signed
/// or saved + use the CLI to sign" notice; on [`ScrapeState::Guidance`] it renders
/// the guided message. It renders NO sign/save control anywhere (BR-VIEW-1 /
/// I-SCR-1 — signing stays in the CLI; the live view never offers a sign
/// affordance).
///
/// COMPOSITION (slice-07 H-3a; ADR-032): the results region is chrome + form
/// wrapped AROUND [`render_scrape_results_fragment`] — the EXACT same fragment fn
/// the htmx shape returns alone. Because the results region is the SAME fn in both
/// shapes, fragment/full-page parity is structural, not asserted by duplicating
/// render logic (I-HX-5). The `<head>` emits exactly ONE local
/// `<script src="/static/htmx.min.js">` (offline-first, never a CDN; I-HX-2) —
/// the SAME chrome line every other enhanced page carries, so the form's
/// `hx-post` swap (H-3a) works in-browser instead of falling back to a full POST.
pub fn render_scrape_page(state: &ScrapeState) -> String {
    let markup = html! {
        (DOCTYPE)
        html {
            (page_head("OpenLore — Live Scrape"))
            body {
                h1 { "Live Scrape" }
                p { (READ_ONLY_NOTICE) }
                (render_scrape_form())
                (render_scrape_results_fragment(state))
            }
        }
    };
    markup.into_string()
}

/// Render the labeled target form (`GET /scrape` and the top of every POST
/// render). PURE. The form POSTs the `target` field back to `/scrape`. It carries
/// NO sign/save control — only a "Scrape" submit that runs the live propose step
/// (BR-VIEW-1 / I-SCR-1).
fn render_scrape_form() -> Markup {
    html! {
        form method="post" action="/scrape" {
            label for="target" { "GitHub target (owner/repo or user)" }
            input type="text" id="target" name="target";
            button type="submit" { "Scrape" }
        }
    }
}

/// Render the result region beneath the form for the given [`ScrapeState`]. PURE
/// total match over the ADT: the GET form shows nothing yet; proposals show the
/// candidate rows + the nothing-saved notice; guidance shows the guided message.
fn render_scrape_result(state: &ScrapeState) -> Markup {
    html! {
        @match state {
            ScrapeState::Form => {}
            ScrapeState::Proposals(rows) => {
                (render_candidate_table(rows))
                p { (SCRAPE_NOTHING_SAVED_NOTICE) }
            }
            // Zero candidates (AC-005.3 / V-S3): the guided no-candidates message
            // + suggested alternative, held verbatim in SCRAPE_NO_CANDIDATES_NOTICE
            // (one pinned site) — NO candidate table, never a blank result.
            ScrapeState::ZeroCandidates => {
                p { (SCRAPE_NO_CANDIDATES_NOTICE) }
            }
            // Network-down (AC-005.4 / V-S4): GitHub could not be reached. Emit
            // ONLY the fixed SCRAPE_NETWORK_DOWN_NOTICE — the plain-language cause
            // + offline-store reassurance. The raw transport error is NEVER
            // interpolated here (the arm is a unit variant carrying none), so no
            // HTTP status / "connection refused" / "DNS" / raw URL / stack trace
            // can leak (NFR-VIEW-6) — NO candidate table, never a blank result.
            ScrapeState::NetworkDown => {
                p { (SCRAPE_NETWORK_DOWN_NOTICE) }
            }
            ScrapeState::Guidance(message) => {
                p { (message) }
            }
        }
    }
}

/// Render the live-scrape candidate table (one `<tr>` per proposal). Small,
/// named, composable — the per-row markup is [`render_candidate_row`].
fn render_candidate_table(rows: &[CandidateRowView]) -> Markup {
    html! {
        table {
            thead {
                tr {
                    th { "Subject" }
                    th { "Predicate" }
                    th { "Object" }
                    th { "Confidence" }
                    th { "Derived-from" }
                }
            }
            tbody {
                @for row in rows {
                    (render_candidate_row(row))
                }
            }
        }
    }
}

/// Render one live-scrape candidate row. The confidence cell goes through
/// [`render_confidence`] (the VERBATIM rule lives in one place, FR-VIEW-8); the
/// derived-from cell renders the DISPLAY-ONLY provenance (WD-62 / I-VIEW-5). NO
/// sign/save control is rendered on the row (BR-VIEW-1 / I-SCR-1).
fn render_candidate_row(row: &CandidateRowView) -> Markup {
    html! {
        tr {
            td { (row.subject) }
            td { (row.predicate) }
            td { (row.object) }
            td { (render_confidence(row.confidence)) }
            td { "derived-from: " (row.derived_from) }
        }
    }
}

// =============================================================================
// Network Search view (`GET /search`, US-NS-001..004 / ADR-036/037/038)
// =============================================================================

/// The HTML `id` of the network-search swap-target element — the `<div>` the htmx
/// `#search-results` fragment IS, and the region the full `/search` page wraps
/// chrome (+ the dimension form) around (slice-08; ADR-037 / mirrors
/// [`SCRAPE_RESULTS_ID`]). Held in ONE place so the fragment fn and any future
/// `hx-target`/`hx-swap` reference the SAME id (a mutation to the id has exactly
/// one site to attack — pinned by the unit test). The no-JS full page embeds the
/// SAME `<div id="search-results">`, so the fragment and the full page's results
/// region are structurally identical (I-NS-6 parity by construction).
pub const SEARCH_RESULTS_ID: &str = "search-results";

/// The real route the network-search view is served at (`/search`) — the no-JS
/// `href`/form `action`, the htmx `hx-get`, AND the nav link all reference this
/// one path (one source of truth for "where the search lives"). Held in ONE place
/// so the chrome's nav link and the form's action can never drift apart.
pub const SEARCH_URL: &str = "/search";

/// The `[verified]` marker every rendered network-search row carries (I-NS-4 —
/// verification is an ingest precondition; there is no unverified state on the
/// viewer surface). Held in ONE place so the marker text is a single mutation
/// site. The acceptance gate (`assert_search_html_every_row_verified_and_attributed`)
/// counts these per author row.
pub const SEARCH_VERIFIED_MARKER: &str = "[verified]";

/// The inline counter-annotation prefix a countered row carries (OD-AV-7 / I-NS-3 —
/// shown, NEVER applied): `countered by <K.author> (<K.cid>)`. Held in ONE place so
/// the shown-not-applied annotation text the browser surface renders is a single
/// source of truth. The counter is an ANNOTATION on the still-rendered countered
/// row — it never filters, merges, or over-rides the claim (the viewer inherits the
/// slice-05 CLI counter-render discipline).
pub const SEARCH_COUNTERED_BY_PREFIX: &str = "countered by";

/// The public-data framing banner the `/search` page states UP FRONT (I-NS-5):
/// discovery indexes only PUBLIC signed claims, verified before indexing; nothing
/// private is read. Held in ONE place so the framing is a single source of truth.
pub const SEARCH_PUBLIC_DATA_NOTICE: &str =
    "Discovery indexes only public signed claims, verified before indexing — \
     nothing private is read.";

/// The fixed plain-language notice the `SearchState::Unavailable` arm renders when
/// the configured network index is unreachable OR unconfigured (I-NS-2 / WD-NS-4).
/// Held in ONE place AND emitted as a fixed constant (NEVER interpolated from a
/// transport error) so the degradation message is a single source of truth AND
/// structurally cannot leak internals: it states the index is unavailable and that
/// the operator's LOCAL store views still work, with NO HTTP status, "connection
/// refused", raw URL, or stack-trace marker (the `Unavailable` arm is a UNIT
/// variant precisely so no transport string can be threaded in). Pinned by the
/// leak-absence unit test + the N-13..N-16 acceptance gate.
pub const SEARCH_UNAVAILABLE_NOTICE: &str =
    "The network index is unavailable. Your local store views still work.";

/// The honest-framing FOOTER the CONTRIBUTOR dimension renders beneath a
/// developer's verified trail (US-NS-003 / AC-003.2): a contributor search surfaces
/// ONE developer's reasoning — it is NOT a community consensus, and the footer says
/// so up front so the per-author trail can never be mistaken for an aggregate
/// verdict. Held in ONE place (the SAME wording the slice-05 CLI `--contributor`
/// render emits) so the honesty promise is a single source of truth + a single
/// mutation site. It is a PROMISE, not a merged row — the anti-merging scan
/// (`assert_search_html_has_no_merged_consensus_row`) excludes it by construction.
pub const SEARCH_CONTRIBUTOR_FOOTER: &str =
    "This is one developer's reasoning trail, not a community consensus.";

/// The render-only follow GUIDANCE prefix an UNFOLLOWED network-author row carries
/// (N-17 / AC-004.5 / WD-NS-3 / I-NS-1): the viewer surfaces the slice-03
/// `openlore peer add <bare-did>` command as TEXT so the operator can follow the
/// author FROM THE CLI. It is GUIDANCE ONLY — there is NO executable follow /
/// subscribe control and NO auto-subscribe path; following stays a deliberate CLI
/// action and the read-only viewer holds no key. Held in ONE place (the SAME slice-03
/// verb the CLI `search` follow affordance emits) so the guidance is a single source
/// of truth + a single mutation site. The bare DID (the slice-03 `peer add` verb's
/// accepted form) is appended by [`render_follow_guidance`].
pub const SEARCH_FOLLOW_GUIDANCE_PREFIX: &str = "Follow this author from the CLI: openlore peer add";

/// The state the network-search results region renders (the pure render input). An
/// ADT over the four outcomes of a `/search` interaction so the renderer matches
/// totally (nw-fp-domain-modeling §1): the empty GET form, a populated per-author
/// result, a guided no-results empty state, or the fixed unavailable notice. The
/// effect shell builds this from the index-query outcome (REACHABLE-with-results →
/// `Results`; reachable-zero → `NoResults`; unreachable/unconfigured →
/// `Unavailable`; the bare `GET /search` with no dimension → `Form`); the renderer
/// is a pure total function over it.
#[derive(Debug, Clone, PartialEq)]
pub enum SearchState {
    /// `GET /search` with no dimension supplied: the empty dimension form, no
    /// query run yet.
    Form,
    /// A REACHABLE index returned ≥1 verified row: render the per-author groups.
    /// Carries the REUSED `appview-domain::compose_results` output VERBATIM — the
    /// viewer holds NO second grouping/verification path (anti-merging is the pure
    /// core's job; the renderer only projects it). The `dimension` the search ran
    /// along is carried alongside so the renderer can add the dimension-specific
    /// honest-framing footer (the CONTRIBUTOR dimension surfaces ONE developer's
    /// trail + the "not a community consensus" footer, US-NS-003 / AC-003.2); the
    /// per-author projection itself is dimension-independent.
    Results {
        /// The REUSED per-author `compose_results` output (anti-merging by
        /// construction — there is no merged "network consensus" row).
        result: appview_domain::NetworkSearchResult,
        /// The dimension the search ran along — selects the dimension-specific
        /// footer (CONTRIBUTOR → the honest-framing "not a community consensus"
        /// line; OBJECT/SUBJECT → none). The grouping is unaffected.
        dimension: appview_domain::SearchDimension,
    },
    /// A REACHABLE index returned ZERO rows for the queried dimension+value
    /// (US-NS-002 Ex 4 / SearchState::NoResults): render a guided plain-language
    /// "no claims found" empty state naming the queried value — never a blank
    /// region or a crash.
    NoResults {
        /// The queried value, named in the guided empty state (so the operator
        /// sees WHAT was searched). E.g. a typo'd object or an absent contributor.
        queried_value: String,
    },
    /// The configured index is UNREACHABLE or UNCONFIGURED (I-NS-2): render the
    /// FIXED [`SEARCH_UNAVAILABLE_NOTICE`]. A UNIT variant — it carries NO
    /// transport detail, so the raw error/URL/status CANNOT be interpolated,
    /// guaranteeing no leaked internals (I-NS-2) by construction.
    Unavailable,
}

/// Render the network-search swap-target FRAGMENT (slice-08; ADR-037): the
/// `<div id="search-results">` wrapping the per-author result groups (or the guided
/// no-results / fixed unavailable notice) for the given [`SearchState`]. PURE: a
/// total function from the view-model to a `Markup` — NO full-page chrome (no
/// `<!DOCTYPE>`, no `<html>`/`<head>`) and NO dimension form, so an `HX-Request`
/// response carries ONLY this results region (I-NS-6). Renders NO sign/follow
/// control (I-NS-1 / WD-NS-3 — following stays a CLI action). [`render_search_page`]
/// EMBEDS this SAME fn beneath the form, so the fragment and the full page's results
/// region are byte-identical by construction (I-NS-6 parity — the results-rendering
/// logic is NOT duplicated). This is the slice-08 structural contract: page =
/// chrome + form + fragment.
///
/// The result rows PROJECT `appview-domain`'s per-author [`NetworkSearchResult`] —
/// each group keyed by its author DID, every row carrying the `[verified]` marker,
/// the author DID, and the VERBATIM confidence (via [`render_confidence`]) — and
/// there is NO merged "network consensus" row (the per-author shape is the only
/// output of the REUSED `compose_results`; the viewer never re-groups).
pub fn render_search_results_fragment(state: &SearchState) -> Markup {
    html! {
        div id=(SEARCH_RESULTS_ID) {
            (render_search_result(state))
        }
    }
}

/// Render the network-search page (`GET /search`, US-NS-001..004) as a complete
/// HTML document (maud). PURE: a total function from the [`SearchState`] to an HTML
/// string — no I/O, no network. ALWAYS renders the public-data framing banner UP
/// FRONT (I-NS-5), a nav link back to the other views, and the labeled dimension
/// form (so the operator can submit / re-submit), THEN the results region. Renders
/// NO sign/follow control anywhere (I-NS-1 / WD-NS-3 — following stays a CLI
/// action; the only "follow" surface is the render-only `openlore peer add <did>`
/// guidance TEXT on an unfollowed row).
///
/// COMPOSITION (slice-08; ADR-037): the results region is chrome + framing + form
/// wrapped AROUND [`render_search_results_fragment`] — the EXACT same fragment fn
/// the htmx shape returns alone. Because the results region is the SAME fn in both
/// shapes, fragment/full-page parity is structural, not asserted by duplicating
/// render logic (I-NS-6). The `<head>` emits exactly ONE local
/// `<script src="/static/htmx.min.js">` (offline-first, never a CDN; I-NS-7) — the
/// SAME chrome line every other enhanced page carries, so the form's `hx-get` swap
/// works in-browser instead of falling back to a full GET.
pub fn render_search_page(state: &SearchState) -> String {
    let markup = html! {
        (DOCTYPE)
        html {
            (page_head("OpenLore — Network Search"))
            body {
                h1 { "Network Search" }
                p { (SEARCH_PUBLIC_DATA_NOTICE) }
                nav {
                    a href=(MY_CLAIMS_URL) { "My Claims" }
                }
                (render_search_form())
                (render_search_results_fragment(state))
            }
        }
    };
    markup.into_string()
}

/// Render the labeled dimension form (`GET /search` and the top of every results
/// render). PURE. The form GETs back to `/search` with a labeled input for EACH
/// dimension the handler parses — `object` (philosophy / object URI),
/// `contributor` (a developer handle, US-NS-003), and
/// `subject` (a project target, US-NS-003) — so the operator can submit / re-submit
/// along ANY dimension. The handler checks the fields in object → contributor →
/// subject order (see `parse_search_dimension`), so an empty field is simply "not
/// this dimension". It carries NO sign/follow control. Enhanced with
/// `hx-get`/`hx-target` so an in-browser submit swaps ONLY the `#search-results`
/// region; the no-JS path is a plain `GET` to `/search`.
fn render_search_form() -> Markup {
    html! {
        form method="get" action=(SEARCH_URL)
             hx-get=(SEARCH_URL)
             hx-target=(format!("#{SEARCH_RESULTS_ID}"))
             hx-swap="innerHTML" {
            label for="object" { "Philosophy / object URI" }
            input type="text" id="object" name="object";
            label for="contributor" { "Contributor handle" }
            input type="text" id="contributor" name="contributor";
            label for="subject" { "Project / subject" }
            input type="text" id="subject" name="subject";
            button type="submit" { "Search" }
        }
    }
}

/// Render the results region beneath the form for the given [`SearchState`]. PURE
/// total match over the ADT: the GET form shows nothing yet; results show the
/// per-author groups; no-results shows the guided empty state; unavailable shows
/// the fixed notice.
fn render_search_result(state: &SearchState) -> Markup {
    html! {
        @match state {
            SearchState::Form => {}
            SearchState::Results { result, dimension } => {
                (render_search_author_groups(result))
                (render_search_footer(*dimension))
            }
            // No-results (US-NS-002 Ex 4): the guided plain-language empty state
            // naming the queried value — never a blank region or a crash.
            SearchState::NoResults { queried_value } => {
                p {
                    "No claims found for " (queried_value) "."
                }
            }
            // Unavailable (I-NS-2): the FIXED plain-language notice ONLY — the unit
            // variant carries no transport detail, so nothing can leak.
            SearchState::Unavailable => {
                p { (SEARCH_UNAVAILABLE_NOTICE) }
            }
        }
    }
}

/// Render the per-author result groups (anti-merging, I-NS-3): one section per
/// author DID, each holding that author's verified rows. PROJECTS the REUSED
/// `appview-domain::compose_results` output — there is NO merged "network
/// consensus" row because the per-author shape is the only thing the pure core
/// produces. Each group is keyed by its author DID (rendered VERBATIM —
/// attribution is never elided).
fn render_search_author_groups(result: &appview_domain::NetworkSearchResult) -> Markup {
    html! {
        @for (author_did, rows) in &result.by_author {
            section {
                h2 { "Author: " (author_did.0) }
                @for row in rows {
                    (render_search_result_row(row))
                }
            }
        }
    }
}

/// Render the dimension-specific honest-framing footer beneath the per-author
/// groups. PURE total match over the dimension: the CONTRIBUTOR dimension surfaces
/// ONE developer's reasoning trail, so it emits the [`SEARCH_CONTRIBUTOR_FOOTER`]
/// "not a community consensus" line (US-NS-003 / AC-003.2) — the same honesty
/// promise the slice-05 CLI `--contributor` render emits. The OBJECT + SUBJECT
/// dimensions render NO footer (their per-author survey speaks for itself; the
/// honesty promise is contributor-specific). The footer is a PROMISE, never a
/// merged row, so it does not collide with the anti-merging guarantee.
fn render_search_footer(dimension: appview_domain::SearchDimension) -> Markup {
    html! {
        @if matches!(dimension, appview_domain::SearchDimension::Contributor) {
            p { (SEARCH_CONTRIBUTOR_FOOTER) }
        }
    }
}

/// Render one network-search result row (a verified, attributed claim). Carries the
/// `[verified]` marker (I-NS-4 — there is no unverified state on the surface), the
/// author DID (attribution, I-NS-3), the claim triple, and the VERBATIM confidence
/// (via [`render_confidence`] — `0.85`, never `0.9`/`90%`; FR-VIEW-8). Renders NO
/// sign/follow control (I-NS-1). The per-row markup is small + named so the
/// load-bearing marker + attribution + verbatim-confidence each have one site to
/// pin against mutation.
fn render_search_result_row(row: &appview_domain::NetworkResultRow) -> Markup {
    html! {
        div {
            span { (SEARCH_VERIFIED_MARKER) }
            " "
            span { (row.author_did.0) }
            " "
            span { (row.subject) " " (row.predicate) " " (row.object) }
            " "
            span { (render_confidence(row.confidence)) }
            // OD-AV-7 / I-NS-3: when this row was COUNTERED, show the counter inline
            // (`countered by <K.author> (<K.cid>)`). The claim above is still
            // rendered VERBATIM — the counter is an ANNOTATION, never applied as a
            // filter/merge/override (the viewer reuses the slice-05 shown-not-applied
            // discipline; the annotation is conditional on `Some`).
            @if let Some(counter) = &row.counter_annotation {
                " "
                span {
                    (SEARCH_COUNTERED_BY_PREFIX) " " (counter.counter_author.0)
                    " (" (counter.referencing_cid.0) ")"
                }
            }
            // N-17 / AC-004.5 / WD-NS-3 / I-NS-1: when this row's author is NOT yet
            // followed, show the render-only `openlore peer add <bare-did>` CLI follow
            // GUIDANCE as TEXT (never an executable control). Following stays a
            // deliberate CLI action; the read-only viewer holds no key.
            @if matches!(row.relationship, AuthorRelationship::NetworkUnfollowed) {
                (render_follow_guidance(&row.author_did.0))
            }
        }
    }
}

/// Render the render-only CLI follow GUIDANCE for an UNFOLLOWED network author
/// (N-17 / AC-004.5 / WD-NS-3 / I-NS-1) as plain TEXT inside a `<p>` — the slice-03
/// `openlore peer add <bare-did>` command the operator runs to follow the author.
/// It is GUIDANCE ONLY: NO `<button>`/`<form>`/`hx-*` control, NO auto-subscribe.
/// The BARE DID (the slice-03 `peer add` verb's accepted form) is derived by
/// stripping any app-identity `#…` fragment, mirroring the CLI `search` follow
/// affordance. PURE total function.
fn render_follow_guidance(author_did: &str) -> Markup {
    let bare = author_did.split('#').next().unwrap_or(author_did);
    html! {
        " "
        p { (SEARCH_FOLLOW_GUIDANCE_PREFIX) " " (bare) }
    }
}

// =============================================================================
// Contributor-Score view (slice-09; ADR-039/040/041) — `GET /score`
// =============================================================================
//
// The `/score` route reads the contributor's LOCAL attributed feed over the
// read-only `StoreReadPort::query_contributor_scoring_feed`, runs the REUSED
// slice-04 PURE `scoring::score(&feed, &ScoringConfig::DEFAULT)` in the effect
// shell, maps the outcome to a [`ScoreState`], and renders it here. This crate
// holds NO scoring math — it PROJECTS the `scoring::WeightedView` (the ranked
// `WeightedPairing`s + their per-claim `Contribution` decomposition). The
// headline weight + the per-claim breakdown are rendered from the SAME
// `WeightedPairing`, so the breakdown subtotals sum to the weight BY
// CONSTRUCTION (Gate 2 / KPI-GRAPH-3 reproduce-by-hand). A score is NEVER shown
// without its breakdown (the J-002c thesis, I-CS-2).

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
pub fn render_score_results_fragment(state: &ScoreState) -> Markup {
    html! {
        div id=(SCORE_RESULTS_ID) {
            (render_score_result(state))
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
pub fn render_score_page(state: &ScoreState) -> String {
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
                (render_score_results_fragment(state))
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
fn render_score_result(state: &ScoreState) -> Markup {
    html! {
        @match state {
            ScoreState::Form => {}
            ScoreState::Scored { view } => {
                @for pairing in &view.ranked {
                    (render_score_pairing(pairing))
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
fn render_score_pairing(pairing: &scoring::WeightedPairing) -> Markup {
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
            (render_score_breakdown(pairing))
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
fn render_score_breakdown(pairing: &scoring::WeightedPairing) -> Markup {
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
                        td { (render_weight(contribution.subtotal)) }
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
fn render_weight(value: f64) -> String {
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
pub fn group_project(entity: &str, rows: &[SurveyRow]) -> TraversalView {
    group_by(entity, rows, |row| row.object.clone())
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
pub fn group_philosophy(entity: &str, rows: &[SurveyRow]) -> TraversalView {
    group_by(entity, rows, |row| row.subject.clone())
}

/// Shared grouping engine for the two surveys (PURE, anti-merging). `key_of` selects
/// the OTHER-dimension key per row (`object` for `/project`, `subject` for
/// `/philosophy`). Order-preserving: groups appear in first-seen key order; edges
/// appear in row order; `contributors` in first-seen author order (deduped). Empty
/// `rows` → [`TraversalView::NoClaims`]. Held in ONE place so the project + (slice-10
/// later) philosophy groupers share the identical anti-merging machinery.
fn group_by(
    entity: &str,
    rows: &[SurveyRow],
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
    let markup = html! {
        (DOCTYPE)
        html {
            (page_head("OpenLore — Project Survey"))
            body {
                h1 { "Project Survey" }
                nav {
                    a href=(MY_CLAIMS_URL) { "My Claims" }
                }
                (render_project_fragment(view))
            }
        }
    };
    markup.into_string()
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
    let markup = html! {
        (DOCTYPE)
        html {
            (page_head("OpenLore — Philosophy Survey"))
            body {
                h1 { "Philosophy Survey" }
                nav {
                    a href=(MY_CLAIMS_URL) { "My Claims" }
                }
                (render_philosophy_fragment(view))
            }
        }
    };
    markup.into_string()
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
fn render_edge_row(edge: &EdgeRow) -> Markup {
    html! {
        tr {
            td {
                a href=(href_score(&edge.author_did)) { (edge.author_did) }
            }
            td { (render_confidence(edge.confidence)) }
            td { (render_confidence_bucket(edge.confidence)) }
            td { (edge.cid) }
        }
    }
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

/// The display-only confidence-bucket LABEL for an edge's confidence — the REUSED
/// `claim_domain::confidence_bucket` (WD-10 thresholds, the ONE SSOT) PROJECTED to a
/// human label (data-models.md §3). The viewer recomputes NO bucket and NO threshold.
/// `0.90 → triangulated`, `0.74 → well-evidenced`, `0.25 → speculative`, `0.50 →
/// weighted`. DISTINCT from the slice-04 scoring `WeightBucket` (Strong/Moderate/
/// Sparse) — the traversal edge shows the per-claim CONFIDENCE bucket, never a weight
/// bucket (J-002c boundary). PURE total match over the ADT.
pub fn render_confidence_bucket(confidence: f64) -> &'static str {
    match claim_domain::confidence_bucket(confidence) {
        claim_domain::ConfidenceBucket::Speculative => "speculative",
        claim_domain::ConfidenceBucket::Weighted => "weighted",
        claim_domain::ConfidenceBucket::WellEvidenced => "well-evidenced",
        claim_domain::ConfidenceBucket::Triangulated => "triangulated",
    }
}

/// Build the `/philosophy?object=<encoded>` traversal href for an object key (the
/// object→philosophy edge; ADR-044). The claim-controlled `object` is percent-encoded
/// into the query component so a hostile URI cannot break out of the `href` attribute
/// or smuggle a second query param. PURE total function.
fn href_philosophy(object: &str) -> String {
    format!("{PHILOSOPHY_URL}?object={}", encode_query_component(object))
}

/// Build the `/project?subject=<encoded>` traversal href for a subject key (the
/// subject→project edge; ADR-044). The claim-controlled `subject` is percent-encoded
/// into the query component. PURE total function. (Used by the `/philosophy` survey's
/// project group keys — the subject→project traversal edge; symmetric with
/// [`href_philosophy`].)
fn href_project(subject: &str) -> String {
    format!("{PROJECT_URL}?subject={}", encode_query_component(subject))
}

/// Build the `/score?contributor=<bare-did-encoded>` traversal href for an author DID
/// (the contributor→score edge; the slice-09 terminus REUSED, ADR-044 Q1 bare-DID
/// form). The DID is reduced to its BARE form ([`bare_did`]) — the signing `#fragment`
/// locator is dropped so the link matches the slice-09 `/score?contributor=` convention
/// — then percent-encoded. PURE total function.
fn href_score(author_did: &str) -> String {
    format!(
        "{SCORE_URL}?contributor={}",
        encode_query_component(bare_did(author_did))
    )
}

/// Reduce a DID to its BARE form — everything before a `#fragment` signing locator
/// (`did:plc:x#org.openlore.application` → `did:plc:x`). PURE total function; a DID
/// without a fragment passes through unchanged. Mirrors the adapter's `bare_did` so the
/// `/score` cross-link matches the slice-09 contributor convention (one bare-DID SSOT).
fn bare_did(did: &str) -> &str {
    match did.split_once('#') {
        Some((bare, _)) => bare,
        None => did,
    }
}

/// Percent-encode a claim-controlled value into an `href` QUERY COMPONENT (ADR-044
/// §security — data-models.md §5): every byte OUTSIDE the unreserved set
/// (`A-Z a-z 0-9 - _ . ~`) becomes `%XX` (uppercase hex). So `/`, `:`, `#`, `&`, `<`,
/// `>`, `"`, `=`, and space all encode — a hostile subject/object cannot break out of
/// the attribute or smuggle a second query param, AND a `github:owner/repo` URI is
/// carried as a SINGLE query value (so the linked key resolves to the SAME survey on
/// the inbound `query_param` → `percent_decode_form` decode — an exact round-trip).
/// PURE total function — defense-in-depth OVER maud's attribute auto-escape.
pub fn encode_query_component(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for &byte in value.as_bytes() {
        let unreserved = byte.is_ascii_alphanumeric()
            || matches!(byte, b'-' | b'_' | b'.' | b'~');
        if unreserved {
            out.push(byte as char);
        } else {
            out.push('%');
            out.push_str(&format!("{byte:02X}"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    //! In-crate unit + property tests for the PURE viewer core. Port-to-port at
    //! domain scope: the pure function signature IS the driving port
    //! (nw-tdd-methodology §Port-to-Port). The confidence-verbatim rendering is
    //! the load-bearing FR-VIEW-8 contract + the prime mutation target.

    use super::*;
    use proptest::prelude::*;

    fn row(
        cid: &str,
        subject: &str,
        predicate: &str,
        object: &str,
        confidence: f64,
    ) -> ClaimRowView {
        ClaimRowView {
            cid: cid.to_string(),
            subject: subject.to_string(),
            predicate: predicate.to_string(),
            object: object.to_string(),
            confidence,
            is_countered: false,
        }
    }

    fn detail(evidence: &[&str]) -> ClaimDetailView {
        ClaimDetailView {
            cid: "bafytokio".to_string(),
            subject: "tokio-rs/tokio".to_string(),
            predicate: "has-license".to_string(),
            object: "MIT".to_string(),
            confidence: 0.95,
            author_did: "did:plc:maria".to_string(),
            composed_at: "2026-05-30T12:00:00+00:00".to_string(),
            evidence: evidence.iter().map(|s| s.to_string()).collect(),
        }
    }

    /// Behavior (AC-002.1): the detail render shows EVERY claim field — subject,
    /// predicate, object, the VERBATIM confidence `0.95`, author_did,
    /// composed_at, and the CID — plus BOTH evidence URLs. Pins the exact V-5
    /// acceptance fixture at the unit level (the prime mutation target).
    #[test]
    fn render_claim_detail_shows_all_fields_and_every_evidence_url() {
        let view = detail(&[
            "https://github.com/tokio-rs/tokio/blob/HEAD/LICENSE",
            "https://github.com/tokio-rs/tokio/blob/HEAD/Cargo.toml",
        ]);
        let html = render_claim_detail(&view, &CounterThread::None);
        for needle in [
            "tokio-rs/tokio",
            "has-license",
            "MIT",
            "0.95",
            "did:plc:maria",
            "2026-05-30T12:00:00+00:00",
            "bafytokio",
            "https://github.com/tokio-rs/tokio/blob/HEAD/LICENSE",
            "https://github.com/tokio-rs/tokio/blob/HEAD/Cargo.toml",
        ] {
            assert!(
                html.contains(needle),
                "detail page must render {needle:?}; got:\n{html}"
            );
        }
    }

    /// Behavior (FR-VIEW-3, step 02-02 boundary): a claim with NO evidence renders
    /// the explicit "no evidence attached" state, never a blank evidence section.
    /// Guards the empty/non-empty fork of the evidence section.
    #[test]
    fn render_claim_detail_with_no_evidence_shows_explicit_empty_state() {
        let html = render_claim_detail(&detail(&[]), &CounterThread::None);
        assert!(
            html.contains("no evidence attached"),
            "a claim with empty evidence must show \"no evidence attached\"; got:\n{html}"
        );
    }

    /// A `CounterClaimRow` builder for the projection unit tests. `pds` empty →
    /// an OWN counter (`is_own == true`); non-empty → a peer counter.
    fn counter_row(author_did: &str, cid: &str, reason: Option<&str>, pds: &str) -> CounterClaimRow {
        use chrono::TimeZone;
        CounterClaimRow {
            author_did: author_did.to_string(),
            cid: cid.to_string(),
            reason: reason.map(|r| r.to_string()),
            confidence: 0.40,
            composed_at: chrono::Utc.with_ymd_and_hms(2026, 5, 30, 12, 0, 0).unwrap(),
            origin: PeerOrigin::Known {
                author_did: author_did.to_string(),
                fetched_from_pds: pds.to_string(),
            },
        }
    }

    /// Behavior (slice-11 / I-CT-2): an EMPTY `query_counter_claims` result projects
    /// to `CounterThread::None` — the un-countered no-noise case. The detail render
    /// then shows the claim ALONE: NO "Counter-claims" section, NO "Countered" flag,
    /// NO "0 counters" empty-state noise.
    #[test]
    fn empty_counter_rows_project_to_none_and_render_no_noise() {
        let thread = CounterThread::from_rows(&[]);
        assert_eq!(thread, CounterThread::None);

        let frag = render_claim_detail_fragment(&detail(&["https://e.test/0"]), &thread)
            .into_string();
        for noise in [
            COUNTER_THREAD_HEADING,
            COUNTERED_PRESENCE_FLAG,
            "0 counters",
            "no disagreement",
        ] {
            assert!(
                !frag.contains(noise),
                "an un-countered claim (CounterThread::None) must render no {noise:?} \
                 noise; got:\n{frag}"
            );
        }
    }

    /// Behavior (slice-11 / I-CT-3): a non-empty result projects to
    /// `CounterThread::Countered` with ONE `CounterEntry` per row, preserving order +
    /// attribution + reason; `is_own` is derived from the ORIGIN (empty PDS → own).
    /// Two rows by distinct authors stay TWO entries (never merged).
    #[test]
    fn counter_rows_project_to_attributed_entries_preserving_order_and_is_own() {
        let rows = vec![
            counter_row("did:plc:maria", "bafy-own", Some("I disagree."), ""),
            counter_row(
                "did:plc:tobias-test",
                "bafy-peer",
                Some("Different lens."),
                "https://pds.example.com",
            ),
        ];
        let thread = CounterThread::from_rows(&rows);
        match thread {
            CounterThread::Countered { counters } => {
                assert_eq!(counters.len(), 2, "two rows → two attributed entries");
                assert_eq!(counters[0].author_did, "did:plc:maria");
                assert_eq!(counters[0].cid, "bafy-own");
                assert_eq!(counters[0].reason.as_deref(), Some("I disagree."));
                assert!(counters[0].is_own, "empty PDS → own counter");
                assert_eq!(counters[1].author_did, "did:plc:tobias-test");
                assert_eq!(counters[1].cid, "bafy-peer");
                assert!(!counters[1].is_own, "non-empty PDS → peer counter");
            }
            CounterThread::None => panic!("non-empty rows must project to Countered"),
        }
    }

    /// Behavior (slice-11 / I-CT-3 / ADR-047): the rendered thread names each
    /// counter's author DID, shows its own CID as a render-only
    /// `<a href="/claims/{cid}">` one-hop drill-link, renders the verbatim reason,
    /// carries the neutral "Countered" presence flag, and never emits a merged
    /// "disputed by N" aggregate. The countered claim's confidence renders VERBATIM
    /// + UNCHANGED (shown-never-applied, I-CT-2).
    #[test]
    fn render_thread_attributes_each_counter_with_drill_link_and_verbatim_reason() {
        let reason = "Cargo's dependency pinning is opt-in, not philosophical.";
        let rows = vec![counter_row("did:plc:maria", "bafycounter", Some(reason), "")];
        let thread = CounterThread::from_rows(&rows);
        let frag = render_claim_detail_fragment(&detail(&["https://e.test/0"]), &thread)
            .into_string();

        assert!(frag.contains(COUNTER_THREAD_HEADING), "thread heading; got:\n{frag}");
        assert!(frag.contains(COUNTERED_PRESENCE_FLAG), "presence flag; got:\n{frag}");
        assert!(frag.contains("did:plc:maria"), "counter author DID; got:\n{frag}");
        assert!(
            frag.contains("href=\"/claims/bafycounter\""),
            "counter CID render-only drill-link toward /claims/{{cid}}; got:\n{frag}"
        );
        assert!(frag.contains(reason), "verbatim reason byte-for-byte; got:\n{frag}");
        // The claim's own confidence (0.95) renders VERBATIM + unchanged by the counter.
        assert!(frag.contains("0.95"), "claim confidence verbatim + unchanged; got:\n{frag}");
        for merged in ["disputed by", "consensus", "net verdict"] {
            assert!(
                !frag.contains(merged),
                "the thread must never emit a merged {merged:?} aggregate; got:\n{frag}"
            );
        }
    }

    /// Behavior (slice-11 / CT-4 anti-merging gold; I-CT-3 / KPI-AV-2): a claim
    /// countered by TWO DISTINCT (author, cid) counters renders EXACTLY two attributed
    /// `<li>` entries — each under its OWN author DID + its OWN CID drill-link + its
    /// OWN verbatim reason — and NEVER a single merged "disputed by 2" / consensus /
    /// net-verdict aggregate row. This is the RENDER-level anti-merging oracle (the
    /// projection-level `from_rows` two-entries oracle is pinned separately above): two
    /// rows → two `<li>` items, never one collapsed row.
    #[test]
    fn render_thread_two_distinct_authors_renders_two_items_never_a_merged_row() {
        let own_reason = "Pinning is a tool, not a value.";
        let peer_reason = "Reproducibility is a different axis.";
        let rows = vec![
            counter_row("did:plc:maria", "bafy-own", Some(own_reason), ""),
            counter_row(
                "did:plc:tobias-test",
                "bafy-peer",
                Some(peer_reason),
                "https://pds.example.com",
            ),
        ];
        let thread = CounterThread::from_rows(&rows);
        let frag = render_claim_detail_fragment(&detail(&["https://e.test/0"]), &thread)
            .into_string();

        // EXACTLY two attributed counter entries — one per (author, cid), never
        // collapsed. Counted by the per-entry "Counter author" label (the evidence
        // section also uses `<li>`, so count the counter-specific marker instead).
        assert_eq!(
            frag.matches("Counter author").count(),
            2,
            "two distinct (author, cid) counters must render EXACTLY two attributed \
             entries (never one merged row); got:\n{frag}"
        );
        // Each author DID + each CID drill-link + each verbatim reason renders.
        for (did, cid, reason) in [
            ("did:plc:maria", "bafy-own", own_reason),
            ("did:plc:tobias-test", "bafy-peer", peer_reason),
        ] {
            assert!(frag.contains(did), "counter author DID {did:?}; got:\n{frag}");
            assert!(
                frag.contains(&format!("href=\"/claims/{cid}\"")),
                "counter CID {cid:?} drill-link; got:\n{frag}"
            );
            assert!(frag.contains(reason), "verbatim reason {reason:?}; got:\n{frag}");
        }
        // NEVER a merged / faceless consensus aggregate row.
        for merged in ["disputed by", "disputed by 2", "consensus", "net verdict"] {
            assert!(
                !frag.contains(merged),
                "two distinct-author counters must NEVER collapse into a merged \
                 {merged:?} aggregate; got:\n{frag}"
            );
        }
    }

    /// Behavior (slice-11 / CT-6 / ADR-047): a counter whose `reason` is `None`
    /// (the ADR-015 wire-optional empty-reason edge) STILL renders its author DID +
    /// its CID AND the explicit "no reason provided" state — never a blank line,
    /// never a crash (total at the type level via `reason: Option<String>`).
    #[test]
    fn render_thread_empty_reason_shows_explicit_no_reason_state() {
        let rows = vec![counter_row("did:plc:tobias-test", "bafynoreason", None, "https://pds.x")];
        let thread = CounterThread::from_rows(&rows);
        let frag = render_claim_detail_fragment(&detail(&[]), &thread).into_string();

        assert!(frag.contains("did:plc:tobias-test"), "author still shown; got:\n{frag}");
        assert!(frag.contains("bafynoreason"), "cid still shown; got:\n{frag}");
        assert!(
            frag.contains(COUNTER_NO_REASON_NOTICE),
            "an absent reason must render the explicit {COUNTER_NO_REASON_NOTICE:?} \
             state; got:\n{frag}"
        );
    }

    /// Behavior (slice-11 / I-CT-2 shown-never-applied): the SAME claim's confidence
    /// + fields render BYTE-IDENTICALLY whether or not a counter is present — the
    /// counter is additive context BELOW, never a re-weight ABOVE. Pins the
    /// load-bearing gold at the unit level (the claim region must not drift).
    #[test]
    fn counter_presence_never_changes_the_claim_region_above_the_thread() {
        let view = detail(&["https://e.test/0"]);
        let uncountered = render_claim_detail_fragment(&view, &CounterThread::None).into_string();
        let rows = vec![counter_row("did:plc:maria", "bafyc", Some("nope"), "")];
        let countered =
            render_claim_detail_fragment(&view, &CounterThread::from_rows(&rows)).into_string();

        // The countered render is a PREFIX-superset: the claim fields + evidence the
        // un-countered render shows all appear UNCHANGED in the countered render.
        for needle in ["tokio-rs/tokio", "has-license", "MIT", "0.95", "bafytokio"] {
            assert!(
                uncountered.contains(needle) && countered.contains(needle),
                "the claim field {needle:?} must render identically with/without a \
                 counter (shown-never-applied); uncountered:\n{uncountered}\n\
                 countered:\n{countered}"
            );
        }
        // The un-countered render carries NONE of the thread chrome.
        assert!(!uncountered.contains(COUNTER_THREAD_HEADING));
        assert!(!uncountered.contains(COUNTERED_PRESENCE_FLAG));
    }

    /// Behavior (slice-07 H-4a; ADR-032/033): the claim-detail swap-target FRAGMENT
    /// wraps the detail region in `<div id="claim-detail">`, renders EVERY claim
    /// field + the VERBATIM confidence + every evidence URL in ordinal order, and
    /// carries NO full-page chrome (no `<!DOCTYPE>`, no `<html>`/`<head>`) so an
    /// `HX-Request` response is the region ALONE (I-HX-1). Pins the fragment's
    /// load-bearing structure at the unit level (the prime mutation target).
    #[test]
    fn render_claim_detail_fragment_wraps_claim_detail_with_all_fields_and_evidence() {
        let ev0 = "https://github.com/tokio-rs/tokio/blob/HEAD/LICENSE";
        let ev1 = "https://github.com/tokio-rs/tokio/blob/HEAD/Cargo.toml";
        let view = detail(&[ev0, ev1]);
        let frag = render_claim_detail_fragment(&view, &CounterThread::None).into_string();

        // Wrapped in the swap-target id, the single source of truth (CLAIM_DETAIL_ID).
        assert!(
            frag.contains(&format!("id=\"{CLAIM_DETAIL_ID}\"")),
            "the fragment must wrap the region in id=\"{CLAIM_DETAIL_ID}\"; got:\n{frag}"
        );
        // NO full-page chrome — the fragment is the region ALONE (I-HX-1).
        assert!(
            !frag.contains("<!DOCTYPE") && !frag.contains("<html"),
            "the fragment must carry NO full-page chrome (no <!DOCTYPE>/<html>); got:\n{frag}"
        );
        // EVERY claim field + the VERBATIM confidence (0.95).
        for needle in [
            "tokio-rs/tokio",
            "has-license",
            "MIT",
            "0.95",
            "did:plc:maria",
            "2026-05-30T12:00:00+00:00",
            "bafytokio",
        ] {
            assert!(
                frag.contains(needle),
                "the fragment must render the field {needle:?}; got:\n{frag}"
            );
        }
        // EVERY evidence URL, in ORDINAL order (ev0 before ev1).
        let pos0 = frag.find(ev0).expect("fragment must contain ev0");
        let pos1 = frag.find(ev1).expect("fragment must contain ev1");
        assert!(
            pos0 < pos1,
            "the fragment must render evidence in ordinal order (ev0 before ev1); got:\n{frag}"
        );
    }

    proptest! {
        /// Property (AC-002.1 — evidence ORDER + completeness): for an arbitrary
        /// NON-EMPTY list of distinct evidence URLs, the detail render contains
        /// EVERY URL AND lays them out in the GIVEN ordinal order (each URL's
        /// position in the rendered HTML is monotonically increasing). This is the
        /// anti-mutation net for the ordered evidence iteration: a renderer that
        /// reversed, sorted, deduped, or dropped evidence fails. Distinct
        /// `idx`-prefixed URLs make "appears in order" checkable by byte offset.
        #[test]
        fn render_claim_detail_lays_out_evidence_in_ordinal_order(
            n in 1usize..6,
        ) {
            let urls: Vec<String> = (0..n)
                .map(|i| format!("https://example.test/evidence-{i}"))
                .collect();
            let url_refs: Vec<&str> = urls.iter().map(String::as_str).collect();
            let html = render_claim_detail(&detail(&url_refs), &CounterThread::None);

            let mut last_pos: Option<usize> = None;
            for url in &urls {
                let pos = html.find(url.as_str());
                prop_assert!(pos.is_some(), "detail must contain evidence url {url:?}");
                let pos = pos.unwrap();
                if let Some(prev) = last_pos {
                    prop_assert!(
                        pos > prev,
                        "evidence must render in ordinal order; {url:?} appeared out of order"
                    );
                }
                last_pos = Some(pos);
            }
            prop_assert!(
                !html.contains("no evidence attached"),
                "a claim WITH evidence must not show the empty state; got:\n{html}"
            );
        }

        /// Property (FR-VIEW-8 in the detail view): for ANY confidence in
        /// `[0.0, 1.0]`, the detail render embeds the VERBATIM two-decimal
        /// confidence (`render_confidence`) and never a `%` sign — the same
        /// verbatim rule the list view obeys, re-pinned at the detail surface.
        #[test]
        fn render_claim_detail_renders_confidence_verbatim(confidence in 0.0f64..=1.0f64) {
            let mut view = detail(&["https://example.test/e0"]);
            view.confidence = confidence;
            let html = render_claim_detail(&view, &CounterThread::None);
            prop_assert!(
                html.contains(&render_confidence(confidence)),
                "detail must embed the verbatim confidence {:?}",
                render_confidence(confidence)
            );
            prop_assert!(
                !html.contains(&format!("{:.2}%", confidence * 100.0)),
                "confidence must never render as a percentage in the detail view"
            );
        }
    }

    /// Behavior: the headline V-1 claim renders as a row carrying every field —
    /// subject, predicate, object, the VERBATIM confidence `0.90`, and the CID.
    /// Example-based because it pins the exact walking-skeleton fixture the
    /// acceptance test asserts on.
    #[test]
    fn render_claims_page_shows_every_field_of_a_seeded_claim() {
        let page = PageView::new(vec![row(
            "bafyrust",
            "rust-lang/rust",
            "is-maintained-by",
            "The Rust Project",
            0.90,
        )]);
        let html = render_claims_page(&page);
        for needle in [
            "rust-lang/rust",
            "is-maintained-by",
            "The Rust Project",
            "0.90",
            "bafyrust",
        ] {
            assert!(
                html.contains(needle),
                "rendered My Claims page must contain {needle:?}; got:\n{html}"
            );
        }
    }

    /// Behavior (FR-VIEW-8, prime mutation target): confidence `0.9` renders
    /// VERBATIM as `0.90` — never `0.9`, never `90%`. Pins the exact stored↔shown
    /// numeric the operator sees.
    #[test]
    fn confidence_zero_point_nine_renders_verbatim_as_two_decimals() {
        assert_eq!(render_confidence(0.90), "0.90");
        assert_eq!(render_confidence(0.95), "0.95");
        assert_eq!(render_confidence(0.8), "0.80");
        assert_eq!(render_confidence(1.0), "1.00");
        assert_eq!(render_confidence(0.0), "0.00");
    }

    proptest! {
        /// Property (FR-VIEW-8): for ANY confidence in `[0.0, 1.0]`, the rendered
        /// string is EXACTLY two decimal places (matches `^[01]\.\d\d$`) and never
        /// carries a `%` sign. This is the anti-mutation net: a renderer that
        /// dropped the `.2` precision (`"0.9"`), used `%`, or scaled by 100 fails.
        #[test]
        fn confidence_always_renders_as_two_decimal_places(confidence in 0.0f64..=1.0f64) {
            let rendered = render_confidence(confidence);
            prop_assert!(
                !rendered.contains('%'),
                "confidence must never render as a percentage; got {rendered:?}"
            );
            // Exactly one '.', exactly two digits after it.
            let dot = rendered.find('.').expect("two-decimal render has a dot");
            let fractional = &rendered[dot + 1..];
            prop_assert_eq!(
                fractional.len(),
                2,
                "confidence must render with exactly two decimal places; got {:?}",
                rendered
            );
            prop_assert!(
                fractional.chars().all(|c| c.is_ascii_digit()),
                "the two decimals must be digits; got {rendered:?}"
            );
        }

        /// Property: every claim's subject/predicate/object/cid + its VERBATIM
        /// confidence appears in the rendered page, for an arbitrary set of rows.
        /// Generalizes the example test across the row domain (Hebert ch.3
        /// "Generalizing example tests"): a known field embedded in the input must
        /// appear in the output.
        #[test]
        fn every_row_field_appears_in_the_rendered_page(
            confidences in proptest::collection::vec(0.0f64..=1.0f64, 1..6)
        ) {
            let rows: Vec<ClaimRowView> = confidences
                .iter()
                .enumerate()
                .map(|(i, &c)| {
                    row(
                        &format!("bafycid{i}"),
                        &format!("owner/repo{i}"),
                        "embodies",
                        &format!("philosophy-{i}"),
                        c,
                    )
                })
                .collect();
            let page = PageView::new(rows.clone());
            let html = render_claims_page(&page);
            for r in &rows {
                prop_assert!(html.contains(&r.cid), "page must contain cid {:?}", r.cid);
                prop_assert!(html.contains(&r.subject), "page must contain subject {:?}", r.subject);
                prop_assert!(html.contains(&r.object), "page must contain object {:?}", r.object);
                prop_assert!(
                    html.contains(&render_confidence(r.confidence)),
                    "page must contain the verbatim confidence for {:?}",
                    r.confidence
                );
            }
        }
    }

    // -------------------------------------------------------------------------
    // Pagination arithmetic (FR-VIEW-6 / US-VIEW-004) — the load-bearing pure
    // mutation surface. The (total, page, page_size) -> (start, end, prev, next)
    // math is PURE + TOTAL; these property tests are its live mutation oracles.
    // -------------------------------------------------------------------------

    /// Build a `PageView` of `n` placeholder rows (the row content is irrelevant
    /// to the bounds arithmetic — only the counts matter).
    fn paged(n: usize, page: u64, page_size: u64, total: u64) -> PageView<ClaimRowView> {
        let rows: Vec<ClaimRowView> = (0..n)
            .map(|i| {
                row(
                    &format!("c{i}"),
                    &format!("s{i}"),
                    "p",
                    &format!("o{i}"),
                    0.90,
                )
            })
            .collect();
        PageView::paged(rows, page, page_size, total)
    }

    /// Behavior (AC-004.1 — the exact V-11 fixture at the unit level): page 1 of
    /// 312 at size 50 shows the `1–50 of 312` indicator (EN DASH) with a Next but
    /// no Previous; page 2 shows `51–100 of 312` with BOTH controls. Pins the
    /// load-bearing acceptance strings the V-11 driving test asserts on.
    #[test]
    fn page_one_and_two_of_312_render_the_exact_indicators_and_controls() {
        let p1 = paged(50, 1, 50, 312);
        assert_eq!(render_position_indicator(&p1), "1\u{2013}50 of 312");
        assert!(!p1.has_prev(), "page 1 has no Previous");
        assert!(p1.has_next(), "page 1 of 7 has a Next");

        let p2 = paged(50, 2, 50, 312);
        assert_eq!(render_position_indicator(&p2), "51\u{2013}100 of 312");
        assert!(p2.has_prev(), "page 2 has a Previous");
        assert!(p2.has_next(), "page 2 of 7 has a Next");
    }

    /// Behavior (AC-004.2 — the LAST page is bounded): page 7 of 312 at size 50
    /// shows `301–312 of 312` (end clamped to total, never 350) with a Previous but
    /// NO Next. Pins the bounded-last-page V-12 fixture.
    #[test]
    fn last_page_of_312_is_bounded_to_total_with_no_next() {
        let last = paged(12, 7, 50, 312);
        assert_eq!(render_position_indicator(&last), "301\u{2013}312 of 312");
        assert!(last.has_prev(), "the last page has a Previous");
        assert!(
            !last.has_next(),
            "the last page has no Next (bounded at total)"
        );
    }

    /// Behavior (AC-004.2 / AC-004.4 — page-beyond-last CLAMP): requesting a page
    /// PAST the last (e.g. `?page=999` over 312 at size 50, last_page = 7) CLAMPS
    /// to the last page rather than erroring or showing a broken `49901–312 of 312`
    /// indicator over an empty page. The clamped view reads the bounded last-page
    /// indicator `301–312 of 312`, a Previous, and NO Next — exactly as page 7.
    /// Pins the clamp ceiling `ceil(total/page_size)` (a mutation dropping the
    /// clamp, or off-by-one in the ceiling, fails this oracle).
    #[test]
    fn a_page_beyond_the_last_is_clamped_to_the_last_page() {
        // page 999 is far past the last page (7) of 312 at size 50.
        let clamped = paged(0, 999, 50, 312);
        assert_eq!(
            clamped.page, 7,
            "a page beyond the last must clamp to the last page (ceil(312/50) = 7)"
        );
        assert_eq!(
            render_position_indicator(&clamped),
            "301\u{2013}312 of 312",
            "the clamped page shows the bounded last-page indicator, not 49901–312"
        );
        assert!(clamped.has_prev(), "the clamped last page has a Previous");
        assert!(
            !clamped.has_next(),
            "the clamped last page has no Next (bounded at total)"
        );

        // An EXACT-multiple total (300 at size 50 -> last_page 6) clamps to 6, not
        // 7 — pins the ceiling at the boundary where div and div_ceil agree.
        let exact = paged(0, 999, 50, 300);
        assert_eq!(exact.page, 6, "ceil(300/50) = 6, not 7");
        assert_eq!(render_position_indicator(&exact), "251\u{2013}300 of 300");

        // An empty result set has no last page: the clamp resolves to page 1 (the
        // single guided page) — never page 0 (which would underflow `start`).
        let empty = paged(0, 999, 50, 0);
        assert_eq!(empty.page, 1, "an empty set clamps to page 1, never 0");
    }

    /// Behavior (AC-004.3 — a store smaller than one page): 12 of 12 at size 50
    /// shows `1–12 of 12` with NEITHER control (the whole set fits one page). Pins
    /// the V-13 single-page fixture.
    #[test]
    fn a_store_smaller_than_one_page_shows_the_indicator_and_no_controls() {
        let only = paged(12, 1, 50, 12);
        assert_eq!(render_position_indicator(&only), "1\u{2013}12 of 12");
        assert!(!only.has_prev(), "a single page has no Previous");
        assert!(!only.has_next(), "a single page has no Next");
        let html = render_claims_page(&only);
        assert!(
            !html.contains("?page="),
            "a single-page store must render no ?page= controls; got:\n{html}"
        );
        assert!(
            html.contains("1\u{2013}12 of 12"),
            "a single-page store must still show the indicator; got:\n{html}"
        );
    }

    /// Behavior: a rendered MIDDLE page links Prev to `?page={n-1}` and Next to
    /// `?page={n+1}` (the controls ARE anchor links to the adjacent pages,
    /// FR-VIEW-6). Pins the exact href arithmetic (a mutation to `+1`/`-1` fails).
    #[test]
    fn a_middle_page_links_prev_and_next_to_adjacent_pages() {
        let html = render_claims_page(&paged(50, 4, 50, 312));
        assert!(
            html.contains("?page=3"),
            "page 4 must link Prev to ?page=3; got:\n{html}"
        );
        assert!(
            html.contains("?page=5"),
            "page 4 must link Next to ?page=5; got:\n{html}"
        );
    }

    proptest! {
        /// Property (AC-004.4 — start/end/total invariants): for ANY non-empty
        /// total, any page within bounds, and any positive page size, the
        /// indicator's arithmetic holds — `start = (page-1)*size + 1`,
        /// `end = min(page*size, total)`, `start <= end <= total`, and the rendered
        /// indicator is EXACTLY `start–end of total` (EN DASH). The anti-mutation
        /// net for the bounds math: dropping the `min`, the `+1`, or the `-1` fails.
        #[test]
        fn pagination_bounds_arithmetic_holds(
            (total, page_size, page) in (1u64..=1000)
                .prop_flat_map(|total| (Just(total), 1u64..=100))
                .prop_flat_map(|(total, page_size)| {
                    let last_page = total.div_ceil(page_size);
                    (Just(total), Just(page_size), 1u64..=last_page)
                }),
        ) {
            let view: PageView<ClaimRowView> = PageView::paged(Vec::new(), page, page_size, total);
            let start = view.start();
            let end = view.end();

            prop_assert_eq!(start, (page - 1) * page_size + 1, "start = (page-1)*size+1");
            prop_assert_eq!(end, (page * page_size).min(total), "end = min(page*size, total)");
            prop_assert!(start <= end, "start ({}) must be <= end ({})", start, end);
            prop_assert!(end <= total, "end ({}) must be <= total ({})", end, total);
            prop_assert_eq!(
                render_position_indicator(&view),
                format!("{start}\u{2013}{end} of {total}"),
                "indicator must read start–end of total"
            );
        }

        /// Property (FR-VIEW-6 — prev/next presence boundaries): Previous is present
        /// IFF `page > 1`; Next is present IFF this page does not reach `total`
        /// (`end < total`). In particular: NO Prev on page 1, NO Next on the last
        /// page. The anti-mutation net for the control-presence predicates.
        #[test]
        fn prev_and_next_presence_match_the_page_boundaries(
            (total, page_size, page) in (1u64..=1000)
                .prop_flat_map(|total| (Just(total), 1u64..=100))
                .prop_flat_map(|(total, page_size)| {
                    let last_page = total.div_ceil(page_size);
                    (Just(total), Just(page_size), 1u64..=last_page)
                }),
        ) {
            let view: PageView<ClaimRowView> = PageView::paged(Vec::new(), page, page_size, total);
            let last_page = total.div_ceil(page_size);

            prop_assert_eq!(view.has_prev(), page > 1, "Prev present iff page > 1");
            prop_assert_eq!(view.has_next(), page < last_page, "Next present iff before the last page");
            if page == 1 {
                prop_assert!(!view.has_prev(), "page 1 must have no Previous");
            }
            if page == last_page {
                prop_assert!(!view.has_next(), "the last page must have no Next");
            }
        }

        /// Property (deterministic, non-overlapping page ranges, AC-004.3): for a
        /// fixed total + page size, walking page 1..=last yields contiguous,
        /// non-overlapping ranges whose union is exactly `1..=total` — each page's
        /// `start` is the previous page's `end + 1`, and the final page's `end`
        /// equals `total`. Pins that paging partitions the result set with no gaps
        /// and no double-counting.
        #[test]
        fn page_ranges_partition_the_result_set(
            total in 1u64..=1000,
            page_size in 1u64..=100,
        ) {
            let last_page = total.div_ceil(page_size);
            let mut expected_start = 1u64;
            for page in 1..=last_page {
                let view: PageView<ClaimRowView> =
                    PageView::paged(Vec::new(), page, page_size, total);
                prop_assert_eq!(view.start(), expected_start, "page {} start must follow the prior end", page);
                prop_assert!(view.end() >= view.start(), "each page covers >= 1 row");
                expected_start = view.end() + 1;
            }
            // After the last page, the next start would be total + 1: the union of
            // ranges is exactly 1..=total (full cover, no overshoot).
            prop_assert_eq!(expected_start, total + 1, "the pages must cover exactly 1..=total");
        }

        /// Property (AC-001.3 — the empty fork): a `total == 0` page renders the
        /// EMPTY position indicator and NO `?page=` controls — regardless of the
        /// (clamped) page / size. Guards the `total == 0` guard in the renderer.
        #[test]
        fn an_empty_result_set_renders_no_indicator_and_no_controls(
            page in 1u64..=10,
            page_size in 1u64..=100,
        ) {
            let view: PageView<ClaimRowView> = PageView::paged(Vec::new(), page, page_size, 0);
            prop_assert_eq!(render_position_indicator(&view), String::new());
            let html = render_claims_page(&view);
            prop_assert!(!html.contains("?page="), "an empty set must render no controls");
        }
    }

    /// Behavior (FR-VIEW-7 / AC-001.3): an empty page renders the guided empty
    /// state — the operator is pointed at the CLI, not shown a blank page — AND it
    /// is JUST guidance: NO claims `<table>`, NO pagination controls, and NO
    /// error/stack-trace markers (AC-001.3 criterion 2; NFR-VIEW-6). Pins the
    /// `total == 0` empty/non-empty fork: a mutation swapping the branch would
    /// either drop the guidance or emit a table here.
    #[test]
    fn empty_page_renders_only_the_guided_empty_state() {
        let page: PageView<ClaimRowView> = PageView::new(vec![]);
        let html = render_claims_page(&page);
        // (a) the guided CLI text IS present — never a blank page.
        assert!(
            html.contains("not signed any claims yet")
                || html.contains("claims you sign with the CLI will appear here"),
            "empty My Claims page must guide the operator to the CLI; got:\n{html}"
        );
        // (b) NO claims table renders on the empty page (the non-empty fork is the
        // only place a `<table>`/row appears) — guards the `total == 0` boundary.
        assert!(
            !html.contains("<table"),
            "the empty page must render NO claims table — only guidance; got:\n{html}"
        );
        // (c) NO pagination controls (no `?page=` next/prev links) on a page with
        // zero claims (AC-001.3 criterion 2).
        assert!(
            !html.contains("?page="),
            "the empty page must render NO pagination controls; got:\n{html}"
        );
        // (d) NO error / raw stack-trace markers leak into the operator's view
        // (NFR-VIEW-6) — the empty store is a guided state, not an error.
        for stack_trace_marker in ["panicked at", "RUST_BACKTRACE", "stack backtrace", "Error:"] {
            assert!(
                !html.contains(stack_trace_marker),
                "the empty page must show no error/stack-trace marker \
                 ({stack_trace_marker:?}); got:\n{html}"
            );
        }
    }

    /// Behavior (AC-001.2): a bound loopback socket address renders as an
    /// `http://` loopback URL — the address the operator opens in a browser.
    /// Pins the `http://` scheme prefix (a mutation dropping it fails here).
    #[test]
    fn loopback_url_prefixes_the_bound_address_with_http_scheme() {
        assert_eq!(loopback_url("127.0.0.1:8080"), "http://127.0.0.1:8080");
        assert_eq!(loopback_url("127.0.0.1:0"), "http://127.0.0.1:0");
    }

    /// Behavior (AC-001.2): the launch banner states the loopback listen URL, the
    /// read-only assurance VERBATIM, and that no signing key is loaded. Pins all
    /// three load-bearing strings so a mutation to any one is caught.
    #[test]
    fn launch_banner_states_loopback_url_read_only_and_no_signing_key() {
        let banner = read_only_launch_banner("127.0.0.1:54321");
        assert!(
            banner.contains("http://127.0.0.1:54321"),
            "launch banner must state the loopback listen URL; got:\n{banner}"
        );
        assert!(
            banner.contains(READ_ONLY_NOTICE),
            "launch banner must state the read-only assurance verbatim; got:\n{banner}"
        );
        assert!(
            banner.contains("read-only"),
            "launch banner must contain the literal \"read-only\"; got:\n{banner}"
        );
        assert!(
            banner.contains("No signing key is loaded"),
            "launch banner must state no signing key is loaded; got:\n{banner}"
        );
    }

    // Property (AC-001.2): for ANY bound loopback host:port, the launch banner
    // embeds the exact `http://<addr>` loopback URL. Generalizes the example
    // across the port domain (Hebert ch.3 "Generalizing example tests").
    proptest! {
        #[test]
        fn launch_banner_always_embeds_the_loopback_url(port in 0u16..=65535) {
            let addr = format!("127.0.0.1:{port}");
            let banner = read_only_launch_banner(&addr);
            prop_assert!(
                banner.contains(&format!("http://{addr}")),
                "banner must embed http://{addr}; got:\n{banner}"
            );
            prop_assert!(
                banner.contains("read-only"),
                "banner must always state read-only; got:\n{banner}"
            );
        }
    }

    /// Behavior (AC-001.2 / NFR-VIEW-1): the landing page states the view is
    /// read-only (VERBATIM assurance) and links back to the My Claims list.
    #[test]
    fn landing_page_states_read_only_and_links_to_claims() {
        let html = render_landing();
        assert!(
            html.contains("read-only"),
            "landing page must state the view is read-only; got:\n{html}"
        );
        assert!(
            html.contains(READ_ONLY_NOTICE),
            "landing page must carry the read-only assurance verbatim; got:\n{html}"
        );
        assert!(
            html.contains("/claims"),
            "landing page must link to the My Claims list; got:\n{html}"
        );
        // Slice-07 (H-5b / I-HX-2): every page-bearing route — including the
        // landing page — loads htmx from the LOCAL `/static/htmx.min.js` route,
        // NEVER a CDN (offline-first). Pins the chrome `<script src>` line on the
        // landing page so it cannot silently drop the local asset reference.
        assert!(
            html.contains(r#"<script src="/static/htmx.min.js">"#),
            "landing page must reference the local htmx asset (offline-first; \
             I-HX-2); got:\n{html}"
        );
    }

    /// Behavior (AC-002.3 / FR-VIEW-3 / NFR-VIEW-6): the guided not-found page
    /// carries the EXACT plain-language message the operator sees for a mistyped
    /// CID AND a back link to the My Claims list — and leaks NO raw internals
    /// (no stack-trace markers, no raw DB error). Pins the message literal + the
    /// back link, the two mutation targets for the `get_claim -> None` 404 render.
    #[test]
    fn render_error_states_the_not_found_message_and_links_back_to_claims() {
        let html = render_error();
        assert!(
            html.contains("No claim with that identifier in your store"),
            "the guided 404 must carry the plain-language not-found message; got:\n{html}"
        );
        assert!(
            html.contains("/claims"),
            "the guided 404 must link back to the My Claims list; got:\n{html}"
        );
        for leaked in [
            "panicked at",
            "RUST_BACKTRACE",
            "stack backtrace",
            "IO Error",
            "StoreReadError",
            "Error:",
        ] {
            assert!(
                !html.contains(leaked),
                "the guided 404 must leak no raw internals ({leaked:?}); got:\n{html}"
            );
        }
    }

    /// Behavior (slice-07 H-4c; ADR-032/033 / AC-002.3 / NFR-VIEW-6): the guided
    /// not-found FRAGMENT carries the EXACT plain-language message + a `/claims`
    /// back link (so the operator's next step is obvious), is wrapped in the
    /// `#claim-detail` swap target (it swaps INTO the same region a found detail
    /// would), carries NO full-page chrome (no `<!DOCTYPE>`, no `<html>`/`<head>`
    /// — an `HX-Request` 404 returns ONLY this region, I-HX-1), and leaks NO raw
    /// internals. Pins the four mutation targets for the `get_claim -> None`
    /// fragment render.
    #[test]
    fn render_claim_not_found_fragment_guides_without_chrome_or_leak() {
        let html = render_claim_not_found_fragment().into_string();
        assert!(
            html.contains(CLAIM_NOT_FOUND_NOTICE),
            "the not-found fragment must carry the plain-language message; got:\n{html}"
        );
        assert!(
            html.contains("/claims"),
            "the not-found fragment must link back to the My Claims list; got:\n{html}"
        );
        assert!(
            html.contains(CLAIM_DETAIL_ID),
            "the not-found fragment must be wrapped in the #claim-detail swap target \
             (it swaps into the SAME region a found detail would); got:\n{html}"
        );
        // NO full-page chrome — an HX-Request 404 returns ONLY this region (I-HX-1).
        let lower = html.to_lowercase();
        assert!(
            !lower.contains("<!doctype") && !lower.contains("<html") && !lower.contains("<head"),
            "the not-found fragment must carry NO full-page chrome (no <!DOCTYPE>/\
             <html>/<head>); got:\n{html}"
        );
        for leaked in [
            "panicked at",
            "RUST_BACKTRACE",
            "stack backtrace",
            "IO Error",
            "StoreReadError",
            "Error:",
        ] {
            assert!(
                !html.contains(leaked),
                "the not-found fragment must leak no raw internals ({leaked:?}); \
                 got:\n{html}"
            );
        }
    }

    // -------------------------------------------------------------------------
    // Peer Claims view (`/peer-claims`, US-VIEW-003 / V-8) unit + property tests
    // -------------------------------------------------------------------------

    fn peer_row(
        cid: &str,
        subject: &str,
        predicate: &str,
        object: &str,
        confidence: f64,
        origin: PeerOrigin,
    ) -> PeerClaimRowView {
        PeerClaimRowView {
            cid: cid.to_string(),
            subject: subject.to_string(),
            predicate: predicate.to_string(),
            object: object.to_string(),
            confidence,
            origin,
            is_countered: false,
        }
    }

    fn known_origin(author_did: &str) -> PeerOrigin {
        PeerOrigin::Known {
            author_did: author_did.to_string(),
            fetched_from_pds: "https://pds.example.test".to_string(),
        }
    }

    /// Behavior (AC-003.1 / V-8 happy): the Peer Claims page renders each
    /// federated claim as a row carrying subject/predicate/object, the VERBATIM
    /// confidence, its CID, AND its peer ORIGIN — the peer's `author_did`,
    /// rendered VERBATIM (attribution discipline, FR-VIEW-4). Pins the exact V-8
    /// fixture at the unit level.
    #[test]
    fn render_peer_claims_page_shows_every_field_including_peer_origin() {
        let page = PageView::new(vec![peer_row(
            "bafypeer",
            "github:peer/axum",
            "embodiesPhilosophy",
            "org.openlore.philosophy.ergonomics",
            0.70,
            known_origin("did:plc:peer-axum"),
        )]);
        let html = render_peer_claims_page(&page);
        for needle in [
            "github:peer/axum",
            "embodiesPhilosophy",
            "org.openlore.philosophy.ergonomics",
            "0.70",
            "bafypeer",
            // The peer ORIGIN (author_did) is rendered VERBATIM — never elided.
            "did:plc:peer-axum",
        ] {
            assert!(
                html.contains(needle),
                "peer claims page must render {needle:?}; got:\n{html}"
            );
        }
    }

    /// Behavior (BR-VIEW-5 — "mine vs federated never ambiguous"): the Peer
    /// Claims page is a SEPARATE surface — its heading + intro state these are
    /// federated peer claims, NOT the operator's own. Guards against the page
    /// being confused with My Claims (a mutation reusing the own-claims heading
    /// would fail).
    #[test]
    fn render_peer_claims_page_is_a_distinct_federated_surface() {
        let page = PageView::new(vec![peer_row(
            "bafypeer",
            "github:peer/axum",
            "embodiesPhilosophy",
            "obj",
            0.70,
            known_origin("did:plc:peer-axum"),
        )]);
        let html = render_peer_claims_page(&page);
        assert!(
            html.contains("Peer Claims"),
            "the peer view must carry the Peer Claims heading; got:\n{html}"
        );
        assert!(
            html.contains("NOT your own"),
            "the peer view must state these are not the operator's own claims \
             (BR-VIEW-5); got:\n{html}"
        );
    }

    /// Behavior (slice-07 H-2a; ADR-032/033): the Peer Claims swap-target FRAGMENT
    /// wraps the peer table in the SHARED [`CLAIMS_TABLE_ID`] swap-target element
    /// (DESIGN §6 — the peer table reuses `#claims-table`, inside `#view-panel`),
    /// carries each row's peer ORIGIN (the author_did, VERBATIM) + the position
    /// indicator, and emits NO full-page chrome (no `<!DOCTYPE>`, no `<html>`) so an
    /// `HX-Request` response carries ONLY the swap region (I-HX-1). Pins the
    /// load-bearing bits: the swap-target id, the page-2 indicator, the verbatim
    /// origin, and the no-chrome fragment shape.
    #[test]
    fn render_peer_claims_table_fragment_wraps_swap_target_with_origin_and_indicator() {
        // Page 2 of a 120-row peer set at size 50 (the H-2a fixture): the indicator
        // reads "51–100 of 120"; the rows carry the peer DID verbatim.
        let page = PageView::paged(
            vec![peer_row(
                "bafypeerpage2",
                "github:peer/axum",
                "endorses",
                "an-object",
                0.80,
                known_origin("did:plc:peer-axum"),
            )],
            2,
            50,
            120,
        );
        let html = render_peer_claims_table_fragment(&page).into_string();

        assert!(
            html.contains("id=\"claims-table\""),
            "the peer fragment must wrap the table in the shared swap-target \
             id=\"claims-table\" (DESIGN §6); got:\n{html}"
        );
        assert_eq!(
            CLAIMS_TABLE_ID, "claims-table",
            "the peer fragment reuses the shared swap-target id const"
        );
        assert!(
            html.contains("51\u{2013}100 of 120"),
            "the peer fragment must render the page-2 indicator \"51\u{2013}100 of \
             120\" (EN DASH); got:\n{html}"
        );
        assert!(
            html.contains("did:plc:peer-axum"),
            "the peer fragment must keep each row's origin (author_did verbatim) so \
             My-vs-federated is never ambiguous; got:\n{html}"
        );
        // The fragment is ONLY the swap region — NO full-page chrome (I-HX-1).
        assert!(
            !html.contains("<!DOCTYPE") && !html.contains("<html"),
            "the peer fragment must carry NO full-page chrome; got:\n{html}"
        );
    }

    /// Behavior (slice-07 H-6a; ADR-034 / DESIGN §6): the Peer Claims VIEW-PANEL
    /// fragment — the swap target the tab switch lands on — wraps the active peer
    /// list region in `<div id="view-panel">` (the tab's `hx-target`) AND contains
    /// the inner `#claims-table` fragment (so peer paging, which targets
    /// `#claims-table`, still lands). It carries the peer origin and NO full-page
    /// chrome (I-HX-1). Pins the `#view-panel` ⊃ `#claims-table` composition — the
    /// load-bearing tab-swap structure — at the unit level.
    #[test]
    fn render_peer_claims_view_panel_fragment_wraps_view_panel_around_the_table() {
        let page = PageView::paged(
            vec![peer_row(
                "bafypeerpanel",
                "github:peer/axum",
                "endorses",
                "an-object",
                0.80,
                known_origin("did:plc:peer-axum"),
            )],
            1,
            50,
            120,
        );
        let html = render_peer_claims_view_panel_fragment(&page).into_string();

        // Wrapped in the tab swap-target id (VIEW_PANEL_ID), the single source of truth.
        assert!(
            html.contains(&format!("id=\"{VIEW_PANEL_ID}\"")),
            "the view-panel fragment must wrap the region in id=\"{VIEW_PANEL_ID}\" \
             (ADR-034: the tab targets #view-panel); got:\n{html}"
        );
        assert_eq!(
            VIEW_PANEL_ID, "view-panel",
            "the tab swap-target id const is \"view-panel\""
        );
        // The inner #claims-table fragment is nested inside the view panel, so the
        // peer paging swap (which targets #claims-table) still lands (DESIGN §6).
        assert!(
            html.contains(&format!("id=\"{CLAIMS_TABLE_ID}\"")),
            "the view-panel fragment must contain the inner id=\"{CLAIMS_TABLE_ID}\" \
             (peer paging targets #claims-table, inside #view-panel); got:\n{html}"
        );
        // It is the PEER list — the peer origin renders so My-vs-federated is clear.
        assert!(
            html.contains("did:plc:peer-axum"),
            "the view-panel fragment must carry the peer origin (author_did); got:\n{html}"
        );
        // The fragment is ONLY the swap region — NO full-page chrome (I-HX-1).
        assert!(
            !html.contains("<!DOCTYPE") && !html.contains("<html"),
            "the view-panel fragment must carry NO full-page chrome; got:\n{html}"
        );
    }

    /// Behavior (slice-07 H-6a; ADR-034): the page chrome's tab navigation carries
    /// BOTH tab anchors (My Claims → `/claims`, Peer Claims → `/peer-claims`), and
    /// each anchor carries a real `href` (the no-JS path) PLUS the htmx attributes
    /// `hx-get` (= the same URL), `hx-target="#view-panel"`, `hx-swap`, and
    /// `hx-push-url="true"` (so the swap pushes the real URL — bookmarkable, Back
    /// works). Pins the progressive-enhancement contract: one anchor, two modes.
    #[test]
    fn tab_nav_anchors_carry_href_plus_htmx_attributes_with_push_url() {
        let html = render_tab_nav().into_string();

        // Both tabs present, each with its real href (the no-JS fallback path).
        assert!(
            html.contains(&format!("href=\"{MY_CLAIMS_URL}\"")),
            "the tab nav must carry a real href to the My Claims URL \
             {MY_CLAIMS_URL:?} (no-JS path); got:\n{html}"
        );
        assert!(
            html.contains(&format!("href=\"{PEER_CLAIMS_URL}\"")),
            "the tab nav must carry a real href to the Peer Claims URL \
             {PEER_CLAIMS_URL:?} (no-JS path); got:\n{html}"
        );
        // The htmx enhancement on the SAME anchors: hx-get = the same URL.
        assert!(
            html.contains(&format!("hx-get=\"{PEER_CLAIMS_URL}\"")),
            "the Peer Claims tab must carry hx-get={PEER_CLAIMS_URL:?} (= its href); \
             got:\n{html}"
        );
        assert!(
            html.contains(&format!("hx-get=\"{MY_CLAIMS_URL}\"")),
            "the My Claims tab must carry hx-get={MY_CLAIMS_URL:?} (= its href); got:\n{html}"
        );
        // The tab swap targets the view panel (NOT #claims-table — that's paging).
        assert!(
            html.contains(&format!("hx-target=\"#{VIEW_PANEL_ID}\"")),
            "each tab must target hx-target=\"#{VIEW_PANEL_ID}\" (ADR-034); got:\n{html}"
        );
        // hx-push-url=true: the swap pushes the real URL into history (bookmark/Back).
        assert!(
            html.contains("hx-push-url=\"true\""),
            "each tab must carry hx-push-url=\"true\" so the active view is \
             bookmarkable and Back works (ADR-034); got:\n{html}"
        );
        // An hx-swap is declared (the panel's inner region is replaced).
        assert!(
            html.contains("hx-swap="),
            "each tab must declare an hx-swap; got:\n{html}"
        );
        // Both tab labels render.
        assert!(
            html.contains("My Claims") && html.contains("Peer Claims"),
            "both tab labels (My Claims / Peer Claims) must render; got:\n{html}"
        );
    }

    /// Behavior (FR-VIEW-4 — the prime mutation target): `render_peer_origin` for
    /// a `Known` origin embeds the peer's `author_did` VERBATIM and is NOT elided.
    /// A mutation that dropped the DID (rendered "" or a placeholder) fails here.
    #[test]
    fn render_peer_origin_known_shows_author_did_verbatim() {
        let rendered = render_peer_origin(&known_origin("did:plc:peer-axum"));
        assert!(
            rendered.contains("did:plc:peer-axum"),
            "a Known origin must render the author_did verbatim; got {rendered:?}"
        );
        // The fetched-from PDS is also surfaced (origin = author_did + pds).
        assert!(
            rendered.contains("https://pds.example.test"),
            "a Known origin must surface the fetched_from_pds; got {rendered:?}"
        );
    }

    /// Behavior (V-10 boundary, step 03-03 extension — pinned now so the ADT arm
    /// is total): an `Unknown` origin renders the literal "unknown" label, never
    /// an empty string (the row must still render, labeled — never dropped).
    #[test]
    fn render_peer_origin_unknown_shows_the_unknown_label() {
        let rendered = render_peer_origin(&PeerOrigin::Unknown);
        assert_eq!(rendered, "unknown");
        assert_eq!(PEER_ORIGIN_UNKNOWN_LABEL, "unknown");
    }

    /// Behavior (V-10 boundary / AC-003.3 — the prime anti-elision mutation
    /// target at the LIST level): a page containing an `Unknown`-origin row STILL
    /// renders that row — it is NEVER filtered out — and the row is labeled
    /// "unknown" while every OTHER field renders normally. The Known/Unknown ADT
    /// match must be TOTAL: a mutation that dropped, skipped, or elided the
    /// `Unknown` arm (rendering an empty page or omitting the row) fails here.
    #[test]
    fn render_peer_claims_page_keeps_an_unknown_origin_row() {
        let page = PageView::new(vec![peer_row(
            "bafyorphanrow",
            "github:peer/orphan-repo",
            "endorses",
            "an-unattributed-object",
            0.70,
            PeerOrigin::Unknown,
        )]);
        let html = render_peer_claims_page(&page);
        // The row is NOT dropped: a table renders (not the empty-state) and the
        // row's OTHER fields all appear (AC-003.3 #1, #3).
        assert!(
            html.contains("<table"),
            "an Unknown-origin row must still render as a table row — never be \
             dropped into the empty state; got:\n{html}"
        );
        for needle in [
            "bafyorphanrow",
            "github:peer/orphan-repo",
            "endorses",
            "an-unattributed-object",
            "0.70",
        ] {
            assert!(
                html.contains(needle),
                "an Unknown-origin row must render its field {needle:?} normally; \
                 got:\n{html}"
            );
        }
        // Its origin is labeled "unknown" rather than dropped (AC-003.3 #2).
        assert!(
            html.contains("unknown"),
            "an Unknown-origin row must be labeled \"unknown\"; got:\n{html}"
        );
        assert!(
            !html.contains("No federated claims yet"),
            "a page WITH an Unknown-origin row must NOT show the empty state; \
             got:\n{html}"
        );
    }

    /// Behavior (FR-VIEW-7 / AC-003.2 / V-9): an empty Peer Claims page renders
    /// the guided "No federated claims yet" empty state — NOT a blank page, NO
    /// table. Pins the `total == 0` empty/non-empty fork.
    #[test]
    fn empty_peer_claims_page_renders_the_guided_no_peers_state() {
        let page: PageView<PeerClaimRowView> = PageView::new(vec![]);
        let html = render_peer_claims_page(&page);
        assert!(
            html.contains("No federated claims yet"),
            "the empty peer view must guide the operator (FR-VIEW-7); got:\n{html}"
        );
        assert!(
            !html.contains("<table"),
            "the empty peer view must render NO table — only guidance; got:\n{html}"
        );
    }

    proptest! {
        /// Property (FR-VIEW-4 — attribution discipline, anti-elision net): for an
        /// arbitrary set of peer rows each with a DISTINCT `Known` author_did, the
        /// rendered Peer Claims page contains EVERY peer's `author_did` verbatim
        /// (plus every subject/object/cid/verbatim-confidence). A renderer that
        /// dropped, deduped, or elided any origin fails. Generalizes the example
        /// across the row domain.
        #[test]
        fn every_peer_row_renders_its_origin_did_verbatim(
            n in 1usize..6,
        ) {
            let rows: Vec<PeerClaimRowView> = (0..n)
                .map(|i| {
                    peer_row(
                        &format!("bafypeercid{i}"),
                        &format!("github:peer/repo{i}"),
                        "embodiesPhilosophy",
                        &format!("philosophy-{i}"),
                        0.70,
                        known_origin(&format!("did:plc:peer-{i}")),
                    )
                })
                .collect();
            let page = PageView::new(rows.clone());
            let html = render_peer_claims_page(&page);
            for r in &rows {
                prop_assert!(html.contains(&r.cid), "page must contain cid {:?}", r.cid);
                prop_assert!(
                    html.contains(&r.subject),
                    "page must contain subject {:?}",
                    r.subject
                );
                if let PeerOrigin::Known { author_did, .. } = &r.origin {
                    prop_assert!(
                        html.contains(author_did),
                        "page must render the peer origin DID {author_did:?} VERBATIM \
                         (never elided)"
                    );
                }
            }
        }
    }

    // =========================================================================
    // Live Scrape view (`render_scrape_page`, US-VIEW-005) — unit + property.
    // =========================================================================

    /// Build a `CandidateClaim` from its display fields + a single source signal
    /// (whose `value` becomes the candidate's derived-from). Routes through the
    /// smart constructor so the non-empty-source invariant (I-SCR-4) holds.
    fn candidate(
        subject: &str,
        predicate: &str,
        object: &str,
        confidence: f64,
        signal_value: &str,
    ) -> CandidateClaim {
        let signal = ports::Signal {
            kind: ports::SignalKind::DependencyManifestPinned,
            value: signal_value.to_string(),
            source_url: "https://github.com/rust-lang/cargo/blob/HEAD/Cargo.lock".to_string(),
        };
        CandidateClaim::try_new(
            subject.to_string(),
            predicate.to_string(),
            object.to_string(),
            vec![signal.source_url.clone()],
            confidence,
            vec![signal],
        )
        .expect("a candidate with one source signal must construct")
    }

    /// Behavior (AC-005.1): `GET /scrape` renders the labeled target form and NO
    /// candidate rows. The form is how the operator submits a target.
    #[test]
    fn render_scrape_page_form_shows_labeled_target_input_and_no_candidates() {
        let html = render_scrape_page(&ScrapeState::Form);
        assert!(
            html.contains("name=\"target\""),
            "the GET form must carry a labeled target input; got:\n{html}"
        );
        assert!(
            html.contains("GitHub target"),
            "the target input must be labeled in domain language; got:\n{html}"
        );
        assert!(
            !html.contains("<tr>"),
            "the empty form must render NO candidate rows; got:\n{html}"
        );
    }

    /// Behavior (slice-07 H-3a / H-5b / I-HX-2): the `/scrape` FULL page emits the
    /// SAME single local `<script src="/static/htmx.min.js">` chrome line as every
    /// other enhanced page — its `hx-post` form swap needs htmx loaded in-browser,
    /// or the form falls back to a full POST. Pins EXACTLY ONE local script src and
    /// NO off-host CDN (offline-first), so the chrome can neither drop the asset nor
    /// reach a CDN.
    #[test]
    fn render_scrape_page_loads_local_htmx_and_no_cdn() {
        let html = render_scrape_page(&ScrapeState::Form);
        assert_eq!(
            html.matches(r#"<script src="/static/htmx.min.js">"#)
                .count(),
            1,
            "the /scrape full page must emit EXACTLY ONE local htmx script src \
             (offline-first; H-3a/I-HX-2); got:\n{html}"
        );
        for cdn in [
            "unpkg.com",
            "jsdelivr.net",
            "cdnjs.cloudflare.com",
            "//cdn.",
        ] {
            assert!(
                !html.contains(cdn),
                "the /scrape full page must reference NO external CDN ({cdn:?}); got:\n{html}"
            );
        }
    }

    /// Behavior (AC-005.2 — the prime row-rendering mutation target): each
    /// proposed candidate renders subject, predicate, object, the VERBATIM
    /// confidence, AND its display-only derived-from provenance.
    #[test]
    fn render_scrape_page_proposals_show_every_field_plus_derived_from() {
        let rows = vec![CandidateRowView::from_candidate(&candidate(
            "github:rust-lang/cargo",
            "embodiesPhilosophy",
            "org.openlore.philosophy.dependency-pinning",
            0.25,
            "Cargo.lock committed (exact pins)",
        ))];
        let html = render_scrape_page(&ScrapeState::Proposals(rows));
        for needle in [
            "github:rust-lang/cargo",
            "embodiesPhilosophy",
            "org.openlore.philosophy.dependency-pinning",
            "0.25",
            "derived-from",
            "Cargo.lock committed (exact pins)",
        ] {
            assert!(
                html.contains(needle),
                "live-scrape proposal row must render {needle:?}; got:\n{html}"
            );
        }
    }

    /// Behavior (slice-07 H-3a; ADR-032/033): the scrape-results swap-target
    /// FRAGMENT wraps the proposal rows in the `#scrape-results` swap-target
    /// element, renders each candidate's subject/predicate/object + the VERBATIM
    /// confidence + the display-only derived-from provenance, emits NO full-page
    /// chrome (no `<!DOCTYPE>`, no `<html>`) so an `HX-Request` response carries
    /// ONLY the swap region (I-HX-1), and renders NO sign affordance (BR-VIEW-1 /
    /// I-SCR-1 — signing stays in the CLI). Pins the load-bearing bits: the
    /// swap-target id, the verbatim confidence, the derived-from, the no-chrome
    /// fragment shape, and the no-sign-control guarantee.
    #[test]
    fn render_scrape_results_fragment_wraps_swap_target_with_candidates_and_no_sign() {
        let rows = vec![CandidateRowView::from_candidate(&candidate(
            "github:rust-lang/cargo",
            "embodiesPhilosophy",
            "org.openlore.philosophy.dependency-pinning",
            0.25,
            "Cargo.lock committed (exact pins)",
        ))];
        let html = render_scrape_results_fragment(&ScrapeState::Proposals(rows)).into_string();

        assert!(
            html.contains("id=\"scrape-results\""),
            "the scrape-results fragment must wrap its rows in the swap-target \
             id=\"scrape-results\" (DESIGN swap map); got:\n{html}"
        );
        assert_eq!(
            SCRAPE_RESULTS_ID, "scrape-results",
            "the fragment reuses the shared swap-target id const"
        );
        for needle in [
            "github:rust-lang/cargo",
            "embodiesPhilosophy",
            "org.openlore.philosophy.dependency-pinning",
            "0.25",
            "derived-from",
            "Cargo.lock committed (exact pins)",
        ] {
            assert!(
                html.contains(needle),
                "the scrape-results fragment row must render {needle:?}; got:\n{html}"
            );
        }
        assert!(
            !html.to_lowercase().contains("<!doctype") && !html.to_lowercase().contains("<html"),
            "the fragment must carry NO full-page chrome (no <!DOCTYPE>/<html>) so an \
             HX-Request response is ONLY the swap region (I-HX-1); got:\n{html}"
        );
        for sign_control_marker in [
            "name=\"sign\"",
            "Sign claim",
            "type=\"submit\" value=\"sign",
        ] {
            assert!(
                !html.contains(sign_control_marker),
                "the scrape-results fragment must render NO sign control \
                 ({sign_control_marker:?}) — signing stays in the CLI \
                 (BR-VIEW-1 / I-SCR-1); got:\n{html}"
            );
        }
    }

    /// Behavior (AC-005.2 — the derived-from PRESENCE branch): EVERY rendered
    /// candidate carries a derived-from provenance value (not just the first).
    #[test]
    fn render_scrape_page_renders_derived_from_on_every_candidate() {
        let rows = vec![
            CandidateRowView::from_candidate(&candidate(
                "github:rust-lang/cargo",
                "embodiesPhilosophy",
                "org.openlore.philosophy.dependency-pinning",
                0.25,
                "signal-one-value",
            )),
            CandidateRowView::from_candidate(&candidate(
                "github:rust-lang/cargo",
                "embodiesPhilosophy",
                "org.openlore.philosophy.test-driven",
                0.25,
                "signal-two-value",
            )),
        ];
        let html = render_scrape_page(&ScrapeState::Proposals(rows));
        // The derived-from label appears once per row (here: twice).
        assert_eq!(
            html.matches("derived-from").count(),
            2,
            "each candidate row must carry its own derived-from; got:\n{html}"
        );
        for value in ["signal-one-value", "signal-two-value"] {
            assert!(
                html.contains(value),
                "each candidate's source signal value {value:?} must render; got:\n{html}"
            );
        }
    }

    /// Behavior (BR-VIEW-2 / I-SCR-1): the proposals page states nothing is
    /// signed or saved AND directs the operator to the CLI to sign.
    #[test]
    fn render_scrape_page_proposals_state_nothing_saved_and_direct_to_cli() {
        let rows = vec![CandidateRowView::from_candidate(&candidate(
            "github:rust-lang/cargo",
            "embodiesPhilosophy",
            "org.openlore.philosophy.dependency-pinning",
            0.25,
            "Cargo.lock committed",
        ))];
        let html = render_scrape_page(&ScrapeState::Proposals(rows));
        assert!(
            html.contains("nothing") && (html.contains("signed") || html.contains("saved")),
            "the proposals page must state nothing is signed or saved; got:\n{html}"
        );
        assert!(
            html.contains("sign") && html.contains("CLI"),
            "the proposals page must direct the operator to the CLI to sign; got:\n{html}"
        );
    }

    /// Behavior (BR-VIEW-1 / I-SCR-1 — the HARD human-gate guardrail): NO sign /
    /// save control is rendered ANYWHERE on the live-scrape page (form, proposals,
    /// or guidance). The live view may describe signing-via-CLI but never offers a
    /// sign affordance. Pins the no-sign-control guarantee across every state.
    #[test]
    fn render_scrape_page_renders_no_sign_control_in_any_state() {
        let proposals = ScrapeState::Proposals(vec![CandidateRowView::from_candidate(&candidate(
            "github:rust-lang/cargo",
            "embodiesPhilosophy",
            "org.openlore.philosophy.dependency-pinning",
            0.25,
            "Cargo.lock committed",
        ))]);
        for state in [
            ScrapeState::Form,
            proposals,
            ScrapeState::Guidance("nothing to show".to_string()),
        ] {
            let html = render_scrape_page(&state);
            for sign_control_marker in [
                "name=\"sign\"",
                "Sign claim",
                "type=\"submit\" value=\"sign",
            ] {
                assert!(
                    !html.contains(sign_control_marker),
                    "the live-scrape page must render NO sign control ({sign_control_marker:?}) \
                     in state {state:?}; got:\n{html}"
                );
            }
        }
    }

    /// Behavior: the guidance state renders the supplied message (the guided
    /// zero-candidates / network-down branch, NFR-VIEW-6) and still shows the
    /// form so the operator can re-submit — never a blank result.
    #[test]
    fn render_scrape_page_guidance_shows_the_message_and_the_form() {
        let html = render_scrape_page(&ScrapeState::Guidance(
            SCRAPE_NO_CANDIDATES_NOTICE.to_string(),
        ));
        assert!(
            html.contains(SCRAPE_NO_CANDIDATES_NOTICE),
            "the guidance state must render the supplied message; got:\n{html}"
        );
        assert!(
            html.contains("name=\"target\""),
            "the guidance state must still render the target form; got:\n{html}"
        );
    }

    /// Behavior (AC-005.3 / V-S3 — the zero-candidates fork): a target that
    /// harvests successfully but derives NO candidates renders the EXACT guided
    /// [`SCRAPE_NO_CANDIDATES_NOTICE`] ("No candidate claims could be derived..."
    /// + a suggested alternative) — NOT a blank result, NOT the network-down copy
    /// (V-S4 — a DISTINCT ADT arm). It renders NO candidate rows and (the form
    /// aside) the result region carries no `<table>`. The typed `ZeroCandidates`
    /// arm keeps this failure mode distinct from `NetworkDown`/`Guidance` so the
    /// specific copy is a single, pinned mutation site.
    #[test]
    fn render_scrape_page_zero_candidates_shows_the_guided_no_candidates_message() {
        let html = render_scrape_page(&ScrapeState::ZeroCandidates);
        // (a) the EXACT zero-candidates copy + suggested alternative renders.
        assert!(
            html.contains(SCRAPE_NO_CANDIDATES_NOTICE),
            "the zero-candidates state must render the guided no-candidates \
             message + suggested alternative; got:\n{html}"
        );
        assert!(
            html.contains("No candidate claims could be derived"),
            "the zero-candidates message must state no candidates could be \
             derived; got:\n{html}"
        );
        assert!(
            html.contains("Try a different"),
            "the zero-candidates message must offer a suggested alternative; \
             got:\n{html}"
        );
        // (b) NO candidate rows / NO candidate table render in the zero-candidates
        // state — only the form + the guided message (never a blank or partial
        // table).
        assert!(
            !html.contains("<table"),
            "the zero-candidates state must render NO candidate table; got:\n{html}"
        );
        assert!(
            !html.contains("<tr>"),
            "the zero-candidates state must render NO candidate rows; got:\n{html}"
        );
        // (c) the form still renders so the operator can try another target, and
        // NO sign control is offered (BR-VIEW-1 / I-SCR-1).
        assert!(
            html.contains("name=\"target\""),
            "the zero-candidates state must still render the target form so the \
             operator can re-submit; got:\n{html}"
        );
        for sign_control_marker in [
            "name=\"sign\"",
            "Sign claim",
            "type=\"submit\" value=\"sign",
        ] {
            assert!(
                !html.contains(sign_control_marker),
                "the zero-candidates state must render NO sign control \
                 ({sign_control_marker:?}); got:\n{html}"
            );
        }
    }

    /// Behavior (AC-005.4 / V-S4 — the network-down fork; the DISTILL low-nit
    /// resolution): the typed [`ScrapeState::NetworkDown`] arm renders the EXACT
    /// guided [`SCRAPE_NETWORK_DOWN_NOTICE`] — (a) it NAMES the cause in domain
    /// language ("GitHub could not be reached"), (b) it REASSURES that the offline
    /// store view "still works offline" (NFR-VIEW-7), and (c) it LEAKS NO transport
    /// internals: no HTTP status code, no "connection refused"/"timed out"/"DNS",
    /// no raw URL (`http`), no stack-trace marker (NFR-VIEW-6). This (the cause +
    /// the leak-ABSENCE) is the prime mutation target — the arm is a unit variant
    /// so the raw error can never be interpolated. NO candidate table, form still
    /// renders, NO sign control (BR-VIEW-1 / I-SCR-1).
    #[test]
    fn render_scrape_page_network_down_names_cause_and_leaks_no_internals() {
        let html = render_scrape_page(&ScrapeState::NetworkDown);
        // (a) the EXACT network-down copy renders, naming the cause in plain
        // domain language.
        assert!(
            html.contains(SCRAPE_NETWORK_DOWN_NOTICE),
            "the network-down state must render the guided network-down message; \
             got:\n{html}"
        );
        assert!(
            html.contains("GitHub could not be reached"),
            "the network-down message must name the cause in domain language; \
             got:\n{html}"
        );
        // (b) it reassures that the store view still works offline (NFR-VIEW-7).
        assert!(
            html.contains("store view still works offline"),
            "the network-down message must reassure that the store view still \
             works offline (NFR-VIEW-7); got:\n{html}"
        );
        // (c) it leaks NO transport internals (NFR-VIEW-6) — the absence assertions
        // are the load-bearing sanitization pins (the mutation target). Lowercase
        // the body so casing variants cannot slip a leak through.
        let lower = html.to_lowercase();
        for leaked_internal in [
            "connection refused",
            "connecterror",
            "timed out",
            "timeout",
            "dns",
            "503",
            "502",
            "500",
            "401",
            "403",
            "404",
            "http",
            "refused",
            "panicked at",
            "stack backtrace",
        ] {
            assert!(
                !lower.contains(leaked_internal),
                "the network-down render must leak NO transport internals \
                 ({leaked_internal:?}); got:\n{html}"
            );
        }
        // (d) NO candidate table / rows, the form still renders, NO sign control.
        assert!(
            !html.contains("<table"),
            "the network-down state must render NO candidate table; got:\n{html}"
        );
        assert!(
            html.contains("name=\"target\""),
            "the network-down state must still render the target form so the \
             operator can re-submit; got:\n{html}"
        );
        for sign_control_marker in [
            "name=\"sign\"",
            "Sign claim",
            "type=\"submit\" value=\"sign",
        ] {
            assert!(
                !html.contains(sign_control_marker),
                "the network-down state must render NO sign control \
                 ({sign_control_marker:?}); got:\n{html}"
            );
        }
    }

    /// Behavior (I-VIEW-5 / WD-62): `CandidateRowView` is the ONLY view-model
    /// carrying derived-from. Projecting a candidate joins its source signal
    /// values into the display-only provenance string.
    #[test]
    fn candidate_row_view_carries_derived_from_from_source_signals() {
        let view = CandidateRowView::from_candidate(&candidate(
            "github:rust-lang/cargo",
            "embodiesPhilosophy",
            "org.openlore.philosophy.dependency-pinning",
            0.25,
            "Cargo.lock committed (exact pins)",
        ));
        assert_eq!(view.derived_from, "Cargo.lock committed (exact pins)");
        assert_eq!(view.confidence, 0.25);
    }

    proptest! {
        /// Property (FR-VIEW-8 in the live-scrape view): for ANY confidence in
        /// `[0.0, 1.0]`, a proposal row embeds the VERBATIM two-decimal confidence
        /// and never a `%` sign — the same verbatim rule re-pinned at this surface.
        #[test]
        fn render_scrape_page_renders_candidate_confidence_verbatim(
            confidence in 0.0f64..=1.0f64,
        ) {
            let rows = vec![CandidateRowView::from_candidate(&candidate(
                "github:rust-lang/cargo",
                "embodiesPhilosophy",
                "org.openlore.philosophy.dependency-pinning",
                confidence,
                "Cargo.lock committed",
            ))];
            let html = render_scrape_page(&ScrapeState::Proposals(rows));
            prop_assert!(
                html.contains(&render_confidence(confidence)),
                "proposal row must embed the verbatim confidence {:?}",
                render_confidence(confidence)
            );
            prop_assert!(
                !html.contains('%'),
                "confidence must never render as a percentage in the live-scrape view"
            );
        }
    }

    // -------------------------------------------------------------------------
    // htmx swap-target fragment (slice-07; ADR-032/033 / US-HX-001 / I-HX-1/5).
    // The fragment fn is the swap-target region returned alone under HX-Request;
    // the full page EMBEDS the same fn, so parity is structural (not duplicated).
    // -------------------------------------------------------------------------

    /// Behavior (H-1a / I-HX-1): the swap-target FRAGMENT wraps the table + the
    /// position indicator + Prev/Next inside ONE `<div id="claims-table">`,
    /// carries every row field + the VERBATIM confidence, and carries NO
    /// full-page chrome (no `<!DOCTYPE>`, no `<html>`/`<head>`). This is what an
    /// `HX-Request` response returns alone. Pins the exact page-2-of-312 fixture
    /// (`51–100 of 312`, EN DASH) the H-1a acceptance test asserts on.
    #[test]
    fn claims_table_fragment_wraps_the_swap_target_with_no_chrome() {
        let view = paged(50, 2, 50, 312);
        let html = render_claims_table_fragment(&view).into_string();

        // Wrapped in exactly the swap-target id.
        assert!(
            html.contains(&format!("id=\"{CLAIMS_TABLE_ID}\"")),
            "fragment must be wrapped in <div id=\"{CLAIMS_TABLE_ID}\">; got:\n{html}"
        );
        // The table region + indicator (EN DASH) + controls are present.
        assert!(
            html.contains("<table"),
            "fragment carries the claims table; got:\n{html}"
        );
        assert!(
            html.contains("51\u{2013}100 of 312"),
            "fragment shows the page-2 indicator \"51\u{2013}100 of 312\"; got:\n{html}"
        );
        assert!(
            html.contains("?page=1"),
            "fragment links Prev to ?page=1; got:\n{html}"
        );
        assert!(
            html.contains("?page=3"),
            "fragment links Next to ?page=3; got:\n{html}"
        );
        // The verbatim confidence rule holds in the fragment (FR-VIEW-8).
        assert!(
            html.contains("0.90"),
            "fragment renders confidence verbatim; got:\n{html}"
        );
        // NO full-page chrome: the fragment is ONLY the swap-target region.
        let lower = html.to_lowercase();
        assert!(
            !lower.contains("<!doctype"),
            "fragment must carry no DOCTYPE; got:\n{html}"
        );
        assert!(
            !lower.contains("<html"),
            "fragment must carry no <html> chrome; got:\n{html}"
        );
        assert!(
            !lower.contains("<head"),
            "fragment must carry no <head> chrome; got:\n{html}"
        );
    }

    /// Behavior (ADR-032 / I-HX-5 — parity by construction): the full page is
    /// chrome wrapped AROUND the SAME `render_claims_table_fragment` fn. The
    /// fragment's exact bytes therefore appear verbatim inside the full page (the
    /// table region is not re-rendered by a divergent path), the page carries the
    /// full-page chrome the fragment lacks, and the `<head>` emits EXACTLY ONE
    /// local `<script src="/static/htmx.min.js">` (offline-first, never a CDN;
    /// I-HX-2). Guards against the table logic being duplicated/diverging.
    #[test]
    fn claims_page_embeds_the_fragment_and_emits_one_local_htmx_script() {
        let view = paged(50, 2, 50, 312);
        let fragment = render_claims_table_fragment(&view).into_string();
        let page = render_claims_page(&view);

        // The full page EMBEDS the fragment verbatim (parity by construction).
        assert!(
            page.contains(&fragment),
            "the full page must embed the SAME fragment bytes; fragment:\n{fragment}\n\npage:\n{page}"
        );
        // The page carries full-page chrome the fragment does not.
        let lower = page.to_lowercase();
        assert!(
            lower.contains("<!doctype html>"),
            "the full page carries a DOCTYPE; got:\n{page}"
        );
        assert!(
            lower.contains("<html"),
            "the full page carries <html> chrome; got:\n{page}"
        );
        // EXACTLY ONE local htmx script, never a CDN.
        assert_eq!(
            page.matches("<script src=\"/static/htmx.min.js\">").count(),
            1,
            "the <head> must emit exactly one local <script src=\"/static/htmx.min.js\">; got:\n{page}"
        );
        for cdn in ["unpkg.com", "jsdelivr", "cdnjs", "//cdn."] {
            assert!(
                !lower.contains(cdn),
                "the htmx asset must be local, never a CDN ({cdn}); got:\n{page}"
            );
        }
    }

    proptest! {
        /// Property (I-HX-5 — parity across the page domain): for ANY non-empty
        /// page within bounds, the fragment's bytes are contained verbatim in the
        /// full page, AND the fragment carries no full-page chrome while the page
        /// does. Generalizes the example: page = chrome + the SAME fragment fn for
        /// every (total, size, page), so the two shapes can never diverge.
        #[test]
        fn fragment_is_always_embedded_verbatim_in_the_full_page(
            (total, page_size, page) in (1u64..=1000)
                .prop_flat_map(|total| (Just(total), 1u64..=100))
                .prop_flat_map(|(total, page_size)| {
                    let last_page = total.div_ceil(page_size);
                    (Just(total), Just(page_size), 1u64..=last_page)
                }),
        ) {
            let view = PageView::paged(Vec::new(), page, page_size, total);
            let fragment = render_claims_table_fragment(&view).into_string();
            let full = render_claims_page(&view);
            prop_assert!(
                full.contains(&fragment),
                "the full page must embed the fragment verbatim for page {page}/{page_size}/{total}"
            );
            let frag_lower = fragment.to_lowercase();
            prop_assert!(!frag_lower.contains("<html"), "the fragment carries no chrome");
            prop_assert!(full.to_lowercase().contains("<html"), "the page carries chrome");
        }
    }

    // -------------------------------------------------------------------------
    // Network Search view (slice-08; ADR-037) — `render_search_results_fragment`
    // -------------------------------------------------------------------------

    use appview_domain::{NetworkResultRow, NetworkSearchResult};
    use ports::AuthorRelationship;

    /// Build a verified network result row for the search-fragment tests. The CID
    /// is caller-supplied so distinct rows stay distinct; `verified_against` is
    /// non-empty (verified-before-index drives the `[verified]` marker).
    fn search_row(author: &str, cid: &str, object: &str, confidence: f64) -> NetworkResultRow {
        NetworkResultRow {
            author_did: ports::claim_domain::Did(author.to_string()),
            cid: ports::claim_domain::Cid(cid.to_string()),
            subject: "github:bazelbuild/bazel".to_string(),
            predicate: "embodiesPhilosophy".to_string(),
            object: object.to_string(),
            confidence,
            verified_against: ports::claim_domain::KeyId(format!(
                "{author}#org.openlore.application"
            )),
            relationship: AuthorRelationship::NetworkUnfollowed,
            counter_annotation: None,
        }
    }

    /// Build a per-author `NetworkSearchResult` from `(author, cid, object, conf)`
    /// rows — mirrors the `compose_results` per-author shape (each author its own
    /// group). Used to drive the render-fragment tests at the view-model boundary.
    fn search_result(rows: &[(&str, &str, &str, f64)]) -> NetworkSearchResult {
        use std::collections::BTreeMap;
        let mut by: BTreeMap<String, (ports::claim_domain::Did, Vec<NetworkResultRow>)> =
            BTreeMap::new();
        for (author, cid, object, conf) in rows {
            let row = search_row(author, cid, object, *conf);
            by.entry(author.to_string())
                .or_insert_with(|| (ports::claim_domain::Did(author.to_string()), Vec::new()))
                .1
                .push(row);
        }
        let by_author: Vec<_> = by.into_values().collect();
        let distinct = by_author.len() as u32;
        let total = rows.len() as u32;
        NetworkSearchResult {
            by_author,
            distinct_author_count: distinct,
            total_claims: total,
            suggestion: None,
        }
    }

    /// Behavior (N-1 / AC-001.2): the results fragment renders per-author groups —
    /// every row carries the `[verified]` marker, the author DID (attribution), and
    /// the VERBATIM confidence (`0.85`, never `0.9`/`90%`). The prime mutation
    /// target: the marker, the DID, and the verbatim confidence are each pinned.
    #[test]
    fn search_fragment_renders_verified_attributed_rows_with_verbatim_confidence() {
        let result = search_result(&[(
            "did:plc:priya-test#org.openlore.application",
            "bafypriya",
            "org.openlore.philosophy.reproducible-builds",
            0.85,
        )]);

        let html = render_search_results_fragment(&SearchState::Results {
            result,
            dimension: appview_domain::SearchDimension::Object,
        })
        .into_string();

        assert!(
            html.contains("[verified]"),
            "every rendered row carries the [verified] marker; got:\n{html}"
        );
        assert!(
            html.contains("did:plc:priya-test#org.openlore.application"),
            "the row is attributed to its author DID (verbatim); got:\n{html}"
        );
        assert!(
            html.contains("0.85"),
            "the confidence renders VERBATIM as 0.85; got:\n{html}"
        );
        assert!(
            !html.contains("0.9") && !html.contains("90%"),
            "the confidence must NOT be rounded to 0.9/90%; got:\n{html}"
        );
    }

    /// Behavior (anti-merging, I-NS-3): two DIFFERENT authors claiming the SAME
    /// object render as TWO attributed rows under two author groups — never one
    /// merged "network consensus" row. The fragment projects the REUSED per-author
    /// `compose_results` shape; there is no second grouping path in the viewer.
    #[test]
    fn search_fragment_renders_two_author_groups_never_a_merged_row() {
        let result = search_result(&[
            (
                "did:plc:priya-test#org.openlore.application",
                "bafypriya",
                "phil.deppin",
                0.70,
            ),
            (
                "did:plc:sven-test#org.openlore.application",
                "bafysven",
                "phil.deppin",
                0.65,
            ),
        ]);

        let html = render_search_results_fragment(&SearchState::Results {
            result,
            dimension: appview_domain::SearchDimension::Object,
        })
        .into_string();

        assert!(html.contains("did:plc:priya-test#org.openlore.application"));
        assert!(html.contains("did:plc:sven-test#org.openlore.application"));
        let lowered = html.to_ascii_lowercase();
        for banned in ["network consensus", "the network thinks", "authors agree"] {
            assert!(
                !lowered.contains(banned),
                "the fragment must show NO merged consensus row; found {banned:?} in:\n{html}"
            );
        }
        assert_eq!(html.matches("[verified]").count(), 2, "two verified rows");
    }

    /// Behavior (I-NS-6 parity by construction): the full `/search` page EMBEDS the
    /// results fragment VERBATIM, and the fragment carries NO full-page chrome while
    /// the page does. So the two shapes can never diverge for a given state.
    #[test]
    fn search_full_page_embeds_the_results_fragment_verbatim() {
        let result = search_result(&[(
            "did:plc:priya-test#org.openlore.application",
            "bafypriya",
            "org.openlore.philosophy.reproducible-builds",
            0.85,
        )]);
        let state = SearchState::Results {
            result,
            dimension: appview_domain::SearchDimension::Object,
        };

        let fragment = render_search_results_fragment(&state).into_string();
        let page = render_search_page(&state);

        assert!(
            page.contains(&fragment),
            "the full page must embed the results fragment verbatim;\nfragment:\n{fragment}\npage:\n{page}"
        );
        assert!(
            !fragment.to_lowercase().contains("<html"),
            "the fragment carries no full-page chrome; got:\n{fragment}"
        );
        assert!(
            page.to_lowercase().contains("<html"),
            "the full page carries chrome; got:\n{page}"
        );
    }

    /// Behavior (US-NS-003 / AC-003.2 — the CONTRIBUTOR render path): a CONTRIBUTOR
    /// search renders ONE developer's verified trail under a SINGLE author DID, every
    /// row carrying `[verified]` + the author DID + the VERBATIM confidence, AND the
    /// honest-framing footer "not a community consensus" beneath the trail — never a
    /// merged consensus row. Pins the dimension-specific footer (the prime mutation
    /// target: a `Contributor`→`_` mutant drops the footer; the per-author projection
    /// + verbatim confidence carry through unchanged).
    #[test]
    fn contributor_results_render_one_author_trail_with_the_honesty_footer() {
        // One developer's trail: TWO verified claims under the SINGLE Priya
        // app-identity DID (the slice-05 handle→DID resolved form).
        let priya = "did:plc:priya-test#org.openlore.application";
        let result = search_result(&[
            (priya, "bafyone", "org.openlore.philosophy.reproducible-builds", 0.82),
            (priya, "bafytwo", "org.openlore.philosophy.hermetic-builds", 0.79),
        ]);

        let html = render_search_results_fragment(&SearchState::Results {
            result,
            dimension: appview_domain::SearchDimension::Contributor,
        })
        .into_string();

        // The honest-framing footer is present (the contributor-specific promise).
        assert!(
            html.contains(SEARCH_CONTRIBUTOR_FOOTER),
            "the contributor render must carry the honesty footer; got:\n{html}"
        );
        assert!(
            html.to_ascii_lowercase().contains("not a community consensus"),
            "the footer states the trail is not a community consensus; got:\n{html}"
        );
        // ONE author group — every row attributed to the SINGLE Priya DID, [verified],
        // with the VERBATIM confidence carried through.
        assert!(
            html.contains(priya),
            "the trail is attributed to the single author DID; got:\n{html}"
        );
        assert_eq!(
            html.matches("[verified]").count(),
            2,
            "both verified rows render under the one author; got:\n{html}"
        );
        for verbatim in ["0.82", "0.79"] {
            assert!(
                html.contains(verbatim),
                "confidence renders VERBATIM ({verbatim}); got:\n{html}"
            );
        }
        // …and NO merged "network consensus" row (the footer is a PROMISE, never an
        // aggregate verdict — the per-author shape is the only output).
        let lowered = html.to_ascii_lowercase();
        for banned in ["network consensus", "the network thinks", "authors agree"] {
            assert!(
                !lowered.contains(banned),
                "the contributor render must show NO merged consensus row; \
                 found {banned:?} in:\n{html}"
            );
        }
    }

    /// Behavior (US-NS-002 — the OBJECT/SUBJECT render path carries NO contributor
    /// footer): the honest-framing footer is CONTRIBUTOR-specific, so an OBJECT
    /// search renders the per-author survey WITHOUT the "not a community consensus"
    /// line. Pins the dimension fork the other direction (a `_`→always mutant would
    /// wrongly stamp the footer on every dimension).
    #[test]
    fn object_results_render_without_the_contributor_footer() {
        let result = search_result(&[(
            "did:plc:priya-test#org.openlore.application",
            "bafypriya",
            "org.openlore.philosophy.reproducible-builds",
            0.82,
        )]);

        let html = render_search_results_fragment(&SearchState::Results {
            result,
            dimension: appview_domain::SearchDimension::Object,
        })
        .into_string();

        assert!(
            !html.contains(SEARCH_CONTRIBUTOR_FOOTER),
            "the OBJECT dimension must NOT render the contributor footer; got:\n{html}"
        );
    }

    /// Behavior (US-NS-003 / AC-003.3 — the SUBJECT render path): a SUBJECT search
    /// surveys ONE project's claims grouped BY AUTHOR — N distinct author rows, each
    /// `[verified]` — with NO merged "the network thinks X about it" consensus row
    /// AND NO contributor footer (the honesty footer is contributor-specific; a
    /// subject survey speaks for itself). Pins the SUBJECT arm of the dimension fork
    /// (a `Contributor`→`_` mutant would wrongly stamp the footer on a subject
    /// survey; a merge mutant would collapse the N author rows into one).
    #[test]
    fn subject_results_render_n_author_groups_without_a_footer_or_merge() {
        let result = search_result(&[
            (
                "did:plc:priya-test#org.openlore.application",
                "bafypriya",
                "org.openlore.philosophy.reproducible-builds",
                0.82,
            ),
            (
                "did:plc:sven-test#org.openlore.application",
                "bafysven",
                "org.openlore.philosophy.hermetic-builds",
                0.71,
            ),
            (
                "did:plc:tobias-test#org.openlore.application",
                "bafytobias",
                "org.openlore.philosophy.dependency-pinning",
                0.66,
            ),
        ]);

        let html = render_search_results_fragment(&SearchState::Results {
            result,
            dimension: appview_domain::SearchDimension::Subject,
        })
        .into_string();

        // N distinct author groups, each attributed + verified (no merge).
        assert!(html.contains("did:plc:priya-test#org.openlore.application"));
        assert!(html.contains("did:plc:sven-test#org.openlore.application"));
        assert!(html.contains("did:plc:tobias-test#org.openlore.application"));
        assert_eq!(
            html.matches("[verified]").count(),
            3,
            "three verified author rows (one per distinct author); got:\n{html}"
        );
        let lowered = html.to_ascii_lowercase();
        for banned in ["network consensus", "the network thinks", "authors agree"] {
            assert!(
                !lowered.contains(banned),
                "the SUBJECT survey must show NO merged consensus row; found {banned:?} in:\n{html}"
            );
        }
        // …and NO contributor footer (the honesty footer is contributor-specific).
        assert!(
            !html.contains(SEARCH_CONTRIBUTOR_FOOTER),
            "the SUBJECT dimension must NOT render the contributor footer; got:\n{html}"
        );
    }

    /// Behavior (I-NS-1 / WD-NS-3): the results fragment renders NO sign/follow/
    /// subscribe control — following stays a CLI action; the viewer is read-only.
    #[test]
    fn search_fragment_renders_no_sign_or_follow_control() {
        let result = search_result(&[(
            "did:plc:priya-test#org.openlore.application",
            "bafypriya",
            "org.openlore.philosophy.reproducible-builds",
            0.85,
        )]);

        let html = render_search_results_fragment(&SearchState::Results {
            result,
            dimension: appview_domain::SearchDimension::Object,
        })
        .into_string();
        let lowered = html.to_ascii_lowercase();

        for banned in [
            "name=\"sign\"",
            "name=\"follow\"",
            "subscribe",
            "<button",
            "<form",
        ] {
            assert!(
                !lowered.contains(banned),
                "the results fragment must carry NO sign/follow control; found {banned:?} in:\n{html}"
            );
        }
    }

    /// Behavior (N-17 / AC-004.5 / WD-NS-3 / I-NS-1): a row by an UNFOLLOWED network
    /// author renders the render-only `openlore peer add <bare-did>` CLI follow
    /// GUIDANCE as TEXT (so the operator can follow from the CLI) — and renders NO
    /// executable follow/subscribe control. Following stays a deliberate CLI action;
    /// the viewer is read-only and holds no key. The guidance names the BARE DID (the
    /// slice-03 `peer add` verb accepts the bare form), stripping any app-identity
    /// `#…` fragment.
    #[test]
    fn search_fragment_unfollowed_row_shows_cli_follow_guidance_text_only() {
        // The composed row carries the app-qualified author DID; the guidance must
        // name the BARE DID the slice-03 `peer add` verb accepts.
        let result = search_result(&[(
            "did:plc:priya-test#org.openlore.application",
            "bafypriya",
            "org.openlore.philosophy.reproducible-builds",
            0.82,
        )]);

        let html = render_search_results_fragment(&SearchState::Results {
            result,
            dimension: appview_domain::SearchDimension::Object,
        })
        .into_string();

        // The render-only follow guidance TEXT names the BARE DID.
        assert!(
            html.contains("openlore peer add did:plc:priya-test"),
            "an unfollowed-author row must show the render-only CLI follow guidance \
             `openlore peer add <bare-did>` as TEXT; got:\n{html}"
        );
        // …and the guidance is TEXT ONLY — no executable follow/subscribe control.
        let lowered = html.to_ascii_lowercase();
        for banned in ["name=\"follow\"", "subscribe", ">follow<", "<button", "<form", "hx-post"] {
            assert!(
                !lowered.contains(banned),
                "the follow guidance must be TEXT ONLY — NO control element; found \
                 {banned:?} in:\n{html}"
            );
        }
    }

    /// Behavior (US-NS-002 Ex 4 / NoResults): a reachable index that returned zero
    /// rows renders a guided plain-language empty state NAMING the queried value —
    /// never a blank region.
    #[test]
    fn search_fragment_no_results_names_the_queried_value() {
        let html = render_search_results_fragment(&SearchState::NoResults {
            queried_value: "org.openlore.philosophy.reprducible".to_string(),
        })
        .into_string();

        assert!(
            html.contains("No claims found"),
            "the NoResults arm renders a guided empty state; got:\n{html}"
        );
        assert!(
            html.contains("org.openlore.philosophy.reprducible"),
            "the empty state NAMES the queried value; got:\n{html}"
        );
    }

    /// Behavior (I-NS-2 / WD-NS-4): the `Unavailable` arm renders the FIXED notice
    /// and leaks NO transport internals — the unit variant cannot interpolate a
    /// transport string, so no HTTP status / "connection refused" / raw URL leaks.
    #[test]
    fn search_fragment_unavailable_is_fixed_and_leaks_no_internals() {
        let html = render_search_results_fragment(&SearchState::Unavailable).into_string();
        let lowered = html.to_ascii_lowercase();

        assert!(
            html.contains(SEARCH_UNAVAILABLE_NOTICE),
            "the Unavailable arm renders the fixed notice; got:\n{html}"
        );
        for leaked in [
            "connection refused",
            "timed out",
            "http://127.0.0.1",
            "503",
            "500",
            "panicked at",
        ] {
            assert!(
                !lowered.contains(&leaked.to_lowercase()),
                "the Unavailable render must leak no transport internals; found {leaked:?} in:\n{html}"
            );
        }
    }

    /// Behavior (US-NS-003 / AC-003.4 — the dimension-selector form offers ALL THREE
    /// dimensions): the `/search` form GETs back to `/search` and exposes an input
    /// for EACH dimension the handler parses (object / contributor / subject), so the
    /// operator can submit / re-submit along any dimension. Pins the form against a
    /// regression that drops the contributor or subject input (the handler parses all
    /// three — the form must offer all three).
    #[test]
    fn search_form_offers_all_three_dimension_inputs() {
        let html = render_search_page(&SearchState::Form);

        assert!(
            html.contains(&format!("action=\"{SEARCH_URL}\"")),
            "the form GETs back to /search; got:\n{html}"
        );
        for dimension_field in ["name=\"object\"", "name=\"contributor\"", "name=\"subject\""] {
            assert!(
                html.contains(dimension_field),
                "the dimension form must offer the {dimension_field} input \
                 (object / contributor / subject); got:\n{html}"
            );
        }
    }

    /// Behavior (N-12 / OD-AV-7 / I-NS-3 — counter SHOWN, not applied): a row whose
    /// `counter_annotation` is `Some` renders an INLINE annotation naming the
    /// countering author (`countered by <K.author>`) AND still renders the claim
    /// VERBATIM (its triple + author DID + `[verified]` marker stay present). The
    /// counter is an ANNOTATION, never a filter/merge/override — the load-bearing
    /// shown-not-applied render gate the browser surface inherits from slice-05.
    #[test]
    fn search_fragment_shows_the_counter_annotation_inline_never_applied() {
        // C — the countered row (Priya); annotated as countered by K (Sven).
        let mut countered = search_row(
            "did:plc:priya-test#org.openlore.application",
            "bafycountered",
            "org.openlore.philosophy.reproducible-builds",
            0.82,
        );
        countered.counter_annotation = Some(appview_domain::CounterRef {
            referencing_cid: ports::claim_domain::Cid("bafycounter".to_string()),
            counter_author: ports::claim_domain::Did(
                "did:plc:sven-test#org.openlore.application".to_string(),
            ),
            ref_type: ports::claim_domain::ReferenceType::Counters,
        });

        let result = NetworkSearchResult {
            by_author: vec![(
                ports::claim_domain::Did("did:plc:priya-test#org.openlore.application".to_string()),
                vec![countered],
            )],
            distinct_author_count: 1,
            total_claims: 1,
            suggestion: None,
        };

        let html = render_search_results_fragment(&SearchState::Results {
            result,
            dimension: appview_domain::SearchDimension::Object,
        })
        .into_string();

        // The counter is SHOWN inline — the annotation names the countering author.
        assert!(
            html.contains("countered by did:plc:sven-test#org.openlore.application"),
            "OD-AV-7: the row must carry an INLINE counter-annotation naming the \
             countering author (countered by <K.author>); got:\n{html}"
        );
        // …and the countered claim is STILL shown verbatim (NOT filtered/merged):
        // its author DID, its triple, and the [verified] marker remain present.
        assert!(
            html.contains("[verified]"),
            "the countered row must STILL carry the [verified] marker (shown, not \
             applied); got:\n{html}"
        );
        assert!(
            html.contains("did:plc:priya-test#org.openlore.application")
                && html.contains("org.openlore.philosophy.reproducible-builds"),
            "the countered claim must STILL render verbatim (its author DID + object \
             stay present — the counter is an annotation, not a filter); got:\n{html}"
        );
    }

    /// Behavior (N-12 negative — no counter): a row whose `counter_annotation` is
    /// `None` renders NO counter-annotation line (the annotation is conditional on
    /// the `Some` — an uncountered claim carries no `countered by` text). Pins the
    /// mutation that would unconditionally emit the annotation.
    #[test]
    fn search_fragment_omits_the_counter_annotation_when_uncountered() {
        let result = search_result(&[(
            "did:plc:priya-test#org.openlore.application",
            "bafyplain",
            "org.openlore.philosophy.reproducible-builds",
            0.82,
        )]);

        let html = render_search_results_fragment(&SearchState::Results {
            result,
            dimension: appview_domain::SearchDimension::Object,
        })
        .into_string();

        assert!(
            !html.contains("countered by"),
            "an UNCOUNTERED row (counter_annotation == None) must render NO \
             'countered by' annotation; got:\n{html}"
        );
    }

    // -------------------------------------------------------------------------
    // Contributor-Score view (slice-09; ADR-039/040/041) —
    // `render_score_results_fragment` projection. The render core REUSES
    // `scoring::score` to obtain a REAL `WeightedView` (never a hand-rolled
    // pairing), so these tests pin the PROJECTION (per-pairing breakdown rows,
    // verbatim confidence, headline weight, and that the rendered subtotals sum to
    // the rendered weight) WITHOUT reimplementing the scoring math.
    // -------------------------------------------------------------------------

    use chrono::{TimeZone, Utc};
    use claim_domain::{Cid, Did};
    use scoring::{score, AttributedClaim, ScoringConfig};

    /// Build one `AttributedClaim` for the score-render fixtures. The author DID +
    /// cid are rendered verbatim in the breakdown; the confidence is the scored
    /// base value (Gate 6).
    fn attributed(author: &str, cid: &str, subject: &str, object: &str, confidence: f64) -> AttributedClaim {
        AttributedClaim {
            author_did: Did(author.to_string()),
            cid: Cid(cid.to_string()),
            subject: subject.to_string(),
            predicate: "embodiesPhilosophy".to_string(),
            object: object.to_string(),
            confidence,
            composed_at: Utc.with_ymd_and_hms(2026, 5, 30, 12, 0, 0).unwrap(),
            relationship: AuthorRelationship::SubscribedPeer,
        }
    }

    /// A RICH feed: one contributor asserting the SAME object across THREE distinct
    /// subjects (cross-project span ≥ 2 → NOT sparse) at varied confidences, so the
    /// pure scorer yields a real weight + a multi-row breakdown that decomposes.
    fn rich_scored_state() -> ScoreState {
        let repro = "org.openlore.philosophy.reproducible-builds";
        let feed = vec![
            attributed("did:plc:priya-test", "bafyone", "github:bazelbuild/bazel", repro, 0.86),
            attributed("did:plc:priya-test", "bafytwo", "github:NixOS/nixpkgs", repro, 0.90),
            attributed("did:plc:priya-test", "bafythree", "github:GNOME/meson", repro, 0.74),
        ];
        let view = score(&feed, &ScoringConfig::DEFAULT);
        ScoreState::Scored { view }
    }

    /// Behavior (C-1/C-4; I-CS-2/I-CS-10): the score fragment renders, for the
    /// contributor's scored feed, EVERY contribution's author DID + cid + the
    /// VERBATIM base confidence (`0.86`, never `0.9`/`86%`) inside a per-claim
    /// breakdown — never an opaque number. Pins the per-row attribution + verbatim
    /// projection at the unit level (the cardinal anti-opaque-number contract).
    #[test]
    fn score_fragment_renders_per_claim_breakdown_attributed_and_verbatim() {
        let html = render_score_results_fragment(&rich_scored_state()).into_string();

        assert!(
            html.contains(SCORE_RESULTS_ID),
            "the score fragment must carry the `#score-results` swap-target id; got:\n{html}"
        );
        // Per-row attribution: the contributor's author DID appears (every
        // Contribution carries its non-Option author_did, I-CS-10).
        assert!(
            html.contains("did:plc:priya-test"),
            "the breakdown must attribute rows to the author DID; got:\n{html}"
        );
        // Every claim's cid is rendered (Gate 5 analog).
        for cid in ["bafyone", "bafytwo", "bafythree"] {
            assert!(html.contains(cid), "the breakdown must name the claim cid {cid:?}; got:\n{html}");
        }
        // Each base confidence is rendered VERBATIM (two decimals — I-CS-6).
        for conf in ["0.86", "0.90", "0.74"] {
            assert!(
                html.contains(conf),
                "the breakdown must render the confidence {conf:?} verbatim (never 0.9/86%); got:\n{html}"
            );
        }
        // The score is never a faceless merged consensus number (anti-merging, I-CS-2).
        let lowered = html.to_ascii_lowercase();
        for banned in ["authors agree", "community consensus", "consensus score"] {
            assert!(
                !lowered.contains(banned),
                "the breakdown must show NO merged consensus row; found {banned:?} in:\n{html}"
            );
        }
    }

    /// Behavior / CARDINAL (C-5; KPI-GRAPH-3 reproduce-by-hand): the per-claim
    /// subtotals the fragment renders for a pairing SUM to the headline weight it
    /// renders for that SAME pairing — because both are projected from the SAME
    /// `WeightedPairing`. This pins the transparency-by-construction contract at the
    /// unit level: the operator can reproduce the number from what she SEES.
    #[test]
    fn score_fragment_rendered_subtotals_sum_to_the_displayed_weight() {
        // A single-pairing feed (TWO distinct authors on the SAME subject+object) so
        // the rendered weight + subtotals are unambiguous and the pairing decomposes
        // into two attributed rows (anti-merging).
        let repro = "org.openlore.philosophy.reproducible-builds";
        let feed = vec![
            attributed("did:plc:priya-test", "bafyone", "github:bazelbuild/bazel", repro, 0.86),
            attributed("did:plc:rachel-test", "bafytwo", "github:bazelbuild/bazel", repro, 0.90),
        ];
        let view = score(&feed, &ScoringConfig::DEFAULT);
        assert_eq!(view.ranked.len(), 1, "fixture must produce exactly one pairing");
        let pairing = &view.ranked[0];

        let html = render_score_results_fragment(&ScoreState::Scored { view: view.clone() }).into_string();

        // The headline weight renders VERBATIM (two decimals).
        let weight_str = format!("{:.2}", pairing.weight);
        assert!(
            html.contains(&format!("Weight: {weight_str}")),
            "the pairing's headline weight {weight_str:?} must render; got:\n{html}"
        );
        // Each contribution's subtotal renders VERBATIM, and their running sum
        // equals the displayed weight (reproduce-by-hand; the subtotals + the
        // weight are projected from the SAME pairing, so they agree by construction).
        let mut running = 0.0_f64;
        for c in pairing.contributions() {
            let subtotal_str = format!("{:.2}", c.subtotal);
            assert!(
                html.contains(&subtotal_str),
                "the breakdown must render the subtotal {subtotal_str:?}; got:\n{html}"
            );
            running += c.subtotal;
        }
        assert!(
            (running - pairing.weight).abs() < 1e-9,
            "Σ subtotal ({running}) must equal the displayed weight ({}) — \
             reproduce-by-hand (KPI-GRAPH-3)",
            pairing.weight
        );
    }

    /// Behavior / CARDINAL anti-opaque (C-5/C-4; I-CS-2 / J-002c): the renderer
    /// NEVER projects a `Weight:` headline without an accompanying per-claim
    /// breakdown `<table>` — across the RICH (multi-pairing/multi-row), SPARSE
    /// (single-row), and CONFLICTING-authors (one pairing, two rows) feeds, in BOTH
    /// the fragment AND the full page. This pins the STRUCTURAL half of the cardinal
    /// transparency gate at the unit level: an opaque-number regression (emitting a
    /// weight while dropping the breakdown table) silently re-creates the J-002
    /// aggregator failure. The arithmetic sibling
    /// (`score_fragment_rendered_subtotals_sum_to_the_displayed_weight`) pins
    /// Σ-subtotal == weight; THIS test pins that the weight and its table are
    /// STRUCTURALLY inseparable — every rendered weight carries a table.
    #[test]
    fn score_render_never_shows_a_weight_without_a_breakdown_table() {
        // Build the three CARDINAL postures as REAL scored views (never hand-rolled
        // pairings — the render core reuses `scoring::score`).
        let repro = "org.openlore.philosophy.reproducible-builds";
        // RICH: one contributor across distinct subjects → multi-row, NOT sparse.
        let rich = rich_scored_state();
        // SPARSE: one claim/one author/one subject → `[SPARSE]`, single-row.
        let sparse = ScoreState::Scored {
            view: score(
                &[attributed("did:plc:bjorn-test", "bafysparse", "github:torvalds/linux", repro, 0.95)],
                &ScoringConfig::DEFAULT,
            ),
        };
        // CONFLICTING: two distinct authors on the SAME (subject, object) → ONE
        // pairing, TWO attributed rows.
        let conflicting = ScoreState::Scored {
            view: score(
                &[
                    attributed("did:plc:test-jeff", "bafyown", "github:denoland/deno", repro, 0.40),
                    attributed("did:plc:test-jeff-collaborator", "bafypeer", "github:denoland/deno", repro, 0.55),
                ],
                &ScoringConfig::DEFAULT,
            ),
        };

        for (posture, state) in [("rich", &rich), ("sparse", &sparse), ("conflicting", &conflicting)] {
            let fragment = render_score_results_fragment(state).into_string();
            let page = render_score_page(state);
            for (shape, html) in [("fragment", &fragment), ("page", &page)] {
                // The surface must actually show a weight (else the structural guard
                // would pass vacuously).
                assert!(
                    html.contains("Weight:"),
                    "anti-opaque ({posture}/{shape}): a Scored render must show a \
                     `Weight:` headline; got:\n{html}"
                );
                // EVERY pairing <section> that shows a weight MUST carry a breakdown
                // <table> — no weight is ever an opaque number detached from its
                // per-claim decomposition (I-CS-2 / J-002c).
                for section in html.split("<section").skip(1) {
                    if section.contains("Weight:") {
                        assert!(
                            section.contains("<table"),
                            "anti-opaque ({posture}/{shape}): a pairing section \
                             renders a `Weight:` headline with NO breakdown `<table>` \
                             — a weight must never be shown without its per-claim \
                             breakdown; offending section:\n<section{section}"
                        );
                    }
                }
                // No weight may render OUTSIDE a pairing section (in the chrome): every
                // `Weight:` occurrence must fall inside a breakdown-bearing section, so
                // the in-section count equals the total count.
                let total = html.matches("Weight:").count();
                let in_sections: usize = html
                    .split("<section")
                    .skip(1)
                    .map(|s| s.matches("Weight:").count())
                    .sum();
                assert_eq!(
                    in_sections, total,
                    "anti-opaque ({posture}/{shape}): every displayed weight must fall \
                     inside a breakdown-bearing pairing <section>; {total} weight(s) \
                     total but {in_sections} inside sections; got:\n{html}"
                );
            }
        }
    }

    /// Behavior (C-8; AC-003.2 / I-CS-6 / KPI-4 verbatim): a contributing claim
    /// stored at 0.90 renders byte-for-byte "0.90" (never "0.9", never "90%"), the
    /// displayed pairing weight is the EXACT consumed `WeightedPairing.weight`
    /// (`{:.2}` of the value — no bucket-midpoint rounding), and BOTH guarantees
    /// hold identically in the fragment AND the full page (no divergence — the page
    /// EMBEDS the fragment fn, so there is exactly ONE confidence formatter
    /// (`render_confidence`) and ONE weight formatter (`render_weight`)). This pins
    /// the single-site verbatim contract at the unit level: a stray `{:.1}` / `%`
    /// path on either shape would fail here.
    #[test]
    fn score_render_keeps_confidence_and_weight_verbatim_in_fragment_and_page() {
        let state = rich_scored_state();
        let ScoreState::Scored { view } = &state else {
            panic!("rich_scored_state must be a Scored view");
        };
        // The consumed weight rendered EXACTLY as `render_weight` would (two
        // decimals of the consumed value — no midpoint rounding).
        let pairing = &view.ranked[0];
        let weight_verbatim = format!("Weight: {}", render_weight(pairing.weight));

        let fragment = render_score_results_fragment(&state).into_string();
        let page = render_score_page(&state);

        for (shape, html) in [("fragment", &fragment), ("page", &page)] {
            // The 0.90 claim renders "0.90" verbatim — never truncated, never a percent.
            assert!(
                html.contains("0.90"),
                "C-8 ({shape}): a claim at 0.90 must render \"0.90\" verbatim (I-CS-6); got:\n{html}"
            );
            assert!(
                !html.contains("90%") && !html.contains('%'),
                "C-8 ({shape}): confidence/weight must render as verbatim decimals, \
                 never a percent (no \"90%\"/\"%\" — single-site render_confidence / \
                 render_weight, no second percent path); got:\n{html}"
            );
            // The displayed weight is the EXACT consumed value (no bucket-midpoint
            // rounding) — the verbatim `Weight: <{:.2} of the consumed weight>`.
            assert!(
                html.contains(&weight_verbatim),
                "C-8 ({shape}): the displayed weight must be the exact consumed \
                 WeightedPairing.weight ({weight_verbatim:?}), with no bucket-midpoint \
                 rounding; got:\n{html}"
            );
        }
        // No divergence: the verbatim region the page shows is the EXACT fragment.
        assert!(
            page.contains(&fragment),
            "C-8: the full page must embed the EXACT fragment, so the verbatim \
             confidence/weight cannot diverge between shapes; page:\n{page}"
        );
    }

    /// Behavior / CARDINAL (C-6; I-CS-2 / I-CS-10 anti-merging): TWO DISTINCT authors
    /// asserting the SAME (subject, object) at DIFFERENT confidences render as TWO
    /// SEPARATE breakdown rows under their OWN author DIDs — within ONE pairing —
    /// never averaged or collapsed into a single faceless consensus row. Pins the
    /// per-author-row decomposition at the unit level: the pure scorer groups by
    /// (subject, object) and the renderer emits one row per `Contribution`, so a
    /// merge/de-dup of same-pairing different-author claims is structurally
    /// impossible. The sum-to-weight sibling pins the arithmetic; THIS test pins the
    /// row CARDINALITY + per-row attribution + verbatim distinct confidences.
    #[test]
    fn score_fragment_renders_two_distinct_authors_on_one_pairing_as_two_rows_no_merge() {
        let repro = "org.openlore.philosophy.reproducible-builds";
        let subject = "github:denoland/deno";
        // Two DISTINCT authors, SAME (subject, object), DIFFERENT confidences.
        let feed = vec![
            attributed("did:plc:test-jeff", "bafyown", subject, repro, 0.40),
            attributed("did:plc:test-jeff-collaborator", "bafypeer", subject, repro, 0.55),
        ];
        let view = score(&feed, &ScoringConfig::DEFAULT);
        // ONE pairing (same subject+object) decomposing into TWO contributions.
        assert_eq!(view.ranked.len(), 1, "two same-(subject,object) claims must form ONE pairing");
        assert_eq!(
            view.ranked[0].contributions().len(),
            2,
            "the one pairing must decompose into TWO contributions (one per author), never merged"
        );

        let html = render_score_results_fragment(&ScoreState::Scored { view }).into_string();

        // BOTH distinct author DIDs render (per-row attribution; non-Option author_did).
        for did in ["did:plc:test-jeff", "did:plc:test-jeff-collaborator"] {
            assert!(
                html.contains(did),
                "the breakdown must attribute a SEPARATE row to {did:?}; got:\n{html}"
            );
        }
        // Each author's distinct base confidence renders VERBATIM — neither averaged
        // nor collapsed (an averaged 0.475 would surface NEITHER 0.40 NOR 0.55).
        for conf in ["0.40", "0.55"] {
            assert!(
                html.contains(conf),
                "the breakdown must render the verbatim confidence {conf:?} (never averaged); got:\n{html}"
            );
        }
        // No averaged/merged consensus midpoint leaks.
        assert!(
            !html.contains("0.48") && !html.contains("0.47"),
            "the breakdown must NOT render an averaged consensus confidence (anti-merging); got:\n{html}"
        );
        // No faceless merged-consensus phrasing.
        let lowered = html.to_ascii_lowercase();
        for banned in ["authors agree", "community consensus", "consensus score", "the network says"] {
            assert!(
                !lowered.contains(banned),
                "the breakdown must show NO merged consensus row; found {banned:?} in:\n{html}"
            );
        }
    }

    /// Behavior (C-7/C-10; I-CS-3): a thin single-claim/single-author/single-subject
    /// feed renders `[SPARSE]` + the "treat as a lead" honesty line REGARDLESS of how
    /// HIGH the confidence is — the breadth guard (inherited from the pure core),
    /// not the magnitude, decides the bucket. The viewer PROJECTS the pure core's
    /// `WeightBucket::Sparse`; it recomputes no bucket (WD-CS-6).
    #[test]
    fn score_fragment_projects_sparse_bucket_and_honesty_line_at_any_confidence() {
        let repro = "org.openlore.philosophy.reproducible-builds";
        // One claim, one author, one subject, HIGH confidence.
        let feed = vec![attributed("did:plc:bjorn-test", "bafysparse", "github:torvalds/linux", repro, 0.95)];
        let view = score(&feed, &ScoringConfig::DEFAULT);
        let html = render_score_results_fragment(&ScoreState::Scored { view }).into_string();

        assert!(
            html.contains("[SPARSE]"),
            "a thin pairing must render the `[SPARSE]` marker; got:\n{html}"
        );
        let lowered = html.to_ascii_lowercase();
        assert!(
            lowered.contains("treat as a lead"),
            "a `[SPARSE]` pairing must carry the 'treat as a lead' honesty line; got:\n{html}"
        );
        // The honesty line names the PROJECTED counts (claim_count=1,
        // distinct_author_count=1) — "based on 1 claim(s) by 1 author(s)" — read off
        // the pure-core pairing, NOT recomputed by the viewer (WD-CS-6).
        assert!(
            lowered.contains("based on 1 claim") && lowered.contains("by 1 author"),
            "a `[SPARSE]` pairing's honesty line must project the counts (based on 1 \
             claim(s) by 1 author(s)); got:\n{html}"
        );
        assert!(
            !html.contains("Strong"),
            "a thin pairing must NOT be labelled Strong regardless of confidence; got:\n{html}"
        );
    }

    /// Behavior (C-9; OD-CS-6 / I-CS-5): the `NoClaims` state renders the guided
    /// "No local claims for that contributor." notice NAMING the queried DID — never
    /// a fabricated zero score, never a `[SPARSE]`/weight leak.
    #[test]
    fn score_fragment_renders_guided_no_claims_state_naming_the_did() {
        let html = render_score_results_fragment(&ScoreState::NoClaims {
            contributor: "did:plc:nobody-local".to_string(),
        })
        .into_string();

        assert!(
            html.to_ascii_lowercase().contains("no local claims"),
            "the NoClaims state must render the guided notice; got:\n{html}"
        );
        assert!(
            html.contains("did:plc:nobody-local"),
            "the NoClaims state must name the queried DID; got:\n{html}"
        );
        for banned in ["[SPARSE]", "Weight:"] {
            assert!(
                !html.contains(banned),
                "the empty state must show NO fabricated score; found {banned:?} in:\n{html}"
            );
        }
    }

    /// Behavior (C-2/C-3; I-CS-7 parity): the full `/score` page EMBEDS the EXACT
    /// `render_score_results_fragment` output — the page's score region is the
    /// fragment string verbatim — so fragment/full-page parity is structural, and
    /// the full page additionally carries chrome (`<!DOCTYPE>`) + the contributor
    /// form.
    #[test]
    fn score_page_embeds_the_fragment_and_adds_chrome_and_form() {
        let state = rich_scored_state();
        let fragment = render_score_results_fragment(&state).into_string();
        let page = render_score_page(&state);

        assert!(
            page.contains(&fragment),
            "the full page must EMBED the exact score-results fragment (parity by \
             construction, I-CS-7); page:\n{page}"
        );
        assert!(
            page.to_lowercase().contains("<!doctype html>"),
            "the full page must carry full-page chrome; page:\n{page}"
        );
        assert!(
            page.contains("name=\"contributor\""),
            "the full page must carry the contributor form; page:\n{page}"
        );
        // The fragment alone carries NO full-page chrome (I-CS-7 / I-HX-1).
        assert!(
            !fragment.contains("<!DOCTYPE") && !fragment.contains("<html"),
            "the fragment must carry NO full-page chrome; fragment:\n{fragment}"
        );
    }

    // -------------------------------------------------------------------------
    // Graph-Traversal view (slice-10; ADR-042/043/044/045) — group_project +
    // render_project_fragment / render_project_page. The pure group + render core
    // (port-to-port at domain scope: the pure fn IS the driving port).
    // -------------------------------------------------------------------------

    /// Build one [`SurveyRow`] for the traversal fixtures (a peer-origin edge).
    fn survey_row(author: &str, cid: &str, subject: &str, object: &str, confidence: f64) -> SurveyRow {
        SurveyRow {
            author_did: author.to_string(),
            cid: cid.to_string(),
            subject: subject.to_string(),
            predicate: "embodiesPhilosophy".to_string(),
            object: object.to_string(),
            confidence,
            origin: PeerOrigin::Known {
                author_did: author.to_string(),
                fetched_from_pds: "https://pds.example".to_string(),
            },
            composed_at: chrono::DateTime::parse_from_rfc3339("2026-05-30T12:00:00+00:00")
                .unwrap()
                .with_timezone(&chrono::Utc),
        }
    }

    /// Behavior (data-models.md §2 / I-GT-3): `group_project` groups a project's
    /// survey rows by `object` (the philosophy embodied), one group per distinct
    /// object, with the distinct contributors deduped + order-preserved.
    #[test]
    fn group_project_groups_by_object_with_deduped_contributors() {
        let rows = [
            survey_row("did:plc:rachel-test", "bafy1", "github:rust-lang/cargo", "phil-a", 0.90),
            survey_row("did:plc:rachel-test", "bafy2", "github:rust-lang/cargo", "phil-b", 0.74),
        ];
        let view = group_project("github:rust-lang/cargo", &rows);
        let TraversalView::Found { entity, groups, contributors } = view else {
            panic!("a non-empty survey must group to Found; got {view:?}");
        };
        assert_eq!(entity, "github:rust-lang/cargo");
        assert_eq!(groups.len(), 2, "two distinct objects → two groups");
        assert_eq!(groups[0].key, "phil-a");
        assert_eq!(groups[1].key, "phil-b");
        // The spanning contributor appears ONCE in the contributor list (deduped).
        assert_eq!(contributors, vec!["did:plc:rachel-test".to_string()]);
    }

    /// Behavior (I-GT-3 anti-merging): two DISTINCT authors on the SAME object render
    /// as TWO `EdgeRow`s under ONE group key — never averaged into a consensus row.
    #[test]
    fn group_project_keeps_two_authors_on_one_object_as_two_rows() {
        let rows = [
            survey_row("did:plc:maria", "bafy1", "github:rust-lang/cargo", "phil-a", 0.92),
            survey_row("did:plc:tobias-test", "bafy2", "github:rust-lang/cargo", "phil-a", 0.70),
        ];
        let view = group_project("github:rust-lang/cargo", &rows);
        let TraversalView::Found { groups, contributors, .. } = view else {
            panic!("expected Found");
        };
        assert_eq!(groups.len(), 1, "one shared object → one group");
        assert_eq!(groups[0].edges.len(), 2, "two authors → two edges (no merge)");
        assert_eq!(contributors.len(), 2, "two distinct contributors");
    }

    /// Behavior (I-GT-4): an EMPTY survey yields `NoClaims` naming the entity — never
    /// a fabricated edge.
    #[test]
    fn group_project_empty_rows_yields_no_claims_naming_the_entity() {
        let view = group_project("github:nonexistent/repo", &[]);
        assert_eq!(
            view,
            TraversalView::NoClaims {
                entity: "github:nonexistent/repo".to_string()
            }
        );
    }

    /// Behavior (I-GT-3 / I-GT-5): `render_project_fragment` carries the
    /// `#traversal-results` id, the group key as a `/philosophy?object=` traversal
    /// href, each edge's author DID (a `/score?contributor=` link), the VERBATIM
    /// confidence (`0.90`) + the REUSED display-only bucket (`triangulated`) + the cid.
    #[test]
    fn render_project_fragment_attributes_each_edge_verbatim_with_bucket_and_cid() {
        let rows = [survey_row(
            "did:plc:rachel-test",
            "bafyedge1",
            "github:rust-lang/cargo",
            "org.openlore.philosophy.dependency-pinning",
            0.90,
        )];
        let view = group_project("github:rust-lang/cargo", &rows);
        let html = render_project_fragment(&view).into_string();
        assert!(html.contains(TRAVERSAL_RESULTS_ID), "fragment must carry the region id; {html}");
        assert!(
            html.contains("/philosophy?object="),
            "the group key must be a /philosophy traversal href; {html}"
        );
        assert!(
            html.contains("/score?contributor="),
            "the author must be a /score traversal link; {html}"
        );
        assert!(html.contains("did:plc:rachel-test"), "edge must attribute its author; {html}");
        assert!(html.contains("0.90"), "confidence must render VERBATIM (0.90, not 0.9); {html}");
        assert!(html.contains("triangulated"), "the REUSED display-only bucket must show; {html}");
        assert!(html.contains("bafyedge1"), "the edge must name its cid; {html}");
        // NO full-page chrome (I-GT-6 / I-HX-1).
        assert!(!html.contains("<!DOCTYPE") && !html.contains("<html"), "fragment has no chrome; {html}");
    }

    /// Behavior (I-GT-4): `render_project_fragment` for a `NoClaims` view names the
    /// queried entity + the guided notice, and fabricates NO edge (no `/philosophy`
    /// href, no `/score` link).
    #[test]
    fn render_project_fragment_no_claims_names_entity_and_fabricates_no_edge() {
        let view = TraversalView::NoClaims {
            entity: "github:nonexistent/repo".to_string(),
        };
        let html = render_project_fragment(&view).into_string();
        assert!(html.contains(TRAVERSAL_RESULTS_ID));
        assert!(html.contains("github:nonexistent/repo"), "must name the queried entity; {html}");
        assert!(html.contains(TRAVERSAL_NO_CLAIMS_NOTICE), "must show the guided notice; {html}");
        assert!(
            !html.contains("/philosophy?object=") && !html.contains("/score?contributor="),
            "a NoClaims render must fabricate NO traversal edge; {html}"
        );
    }

    /// Behavior (I-GT-6 parity by construction): `render_project_page` EMBEDS the
    /// EXACT `render_project_fragment` region verbatim, plus full-page chrome.
    #[test]
    fn render_project_page_embeds_the_fragment_region_with_chrome() {
        let rows = [survey_row(
            "did:plc:rachel-test",
            "bafyedge1",
            "github:rust-lang/cargo",
            "phil-a",
            0.90,
        )];
        let view = group_project("github:rust-lang/cargo", &rows);
        let fragment = render_project_fragment(&view).into_string();
        let page = render_project_page(&view);
        assert!(
            page.contains(&fragment),
            "the full page must EMBED the exact traversal-results fragment (parity by \
             construction, I-GT-6); page:\n{page}"
        );
        assert!(
            page.to_lowercase().contains("<!doctype html>"),
            "the full page must carry full-page chrome; page:\n{page}"
        );
    }

    /// Behavior (ADR-044 §security): `encode_query_component` percent-encodes every
    /// byte outside the unreserved set, so a hostile claim-controlled URI cannot break
    /// out of the href attribute. Pins the canonical encoded forms.
    #[test]
    fn encode_query_component_percent_encodes_reserved_and_hostile_bytes() {
        assert_eq!(
            encode_query_component("github:rust-lang/cargo"),
            "github%3Arust-lang%2Fcargo"
        );
        assert_eq!(
            encode_query_component("github:evil/x\"><script>&q= space"),
            "github%3Aevil%2Fx%22%3E%3Cscript%3E%26q%3D%20space"
        );
        // Unreserved bytes pass through unchanged.
        assert_eq!(encode_query_component("aZ0-_.~"), "aZ0-_.~");
    }

    /// The inbound decoder's behavior, mirrored as a TEST ORACLE so the round-trip
    /// property can prove `encode_query_component` is its exact inverse WITHOUT a
    /// cross-crate dependency on the adapter (`adapter-http-viewer::percent_decode_
    /// form`). Decodes a `%XX` triplet back to its byte and passes unreserved bytes
    /// through verbatim — the same total decode the inbound `query_param` applies to
    /// a followed traversal link (ADR-044 §security round-trip). NOTE: unlike a raw
    /// HTML-form decoder it does NOT treat `+` as space, because the ENCODER never
    /// emits a bare `+` (space → `%20`), so over the encoder's output the two agree.
    #[cfg(test)]
    fn percent_decode_query_component(value: &str) -> String {
        let bytes = value.as_bytes();
        let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
        let mut i = 0;
        while i < bytes.len() {
            if bytes[i] == b'%' && i + 2 < bytes.len() {
                let hi = (bytes[i + 1] as char).to_digit(16);
                let lo = (bytes[i + 2] as char).to_digit(16);
                match (hi, lo) {
                    (Some(hi), Some(lo)) => {
                        out.push((hi * 16 + lo) as u8);
                        i += 3;
                    }
                    _ => {
                        out.push(bytes[i]);
                        i += 1;
                    }
                }
            } else {
                out.push(bytes[i]);
                i += 1;
            }
        }
        String::from_utf8_lossy(&out).into_owned()
    }

    /// Property (ADR-044 §security — the injection-boundary INVARIANT): for ANY
    /// claim-controlled string, `encode_query_component` emits ONLY bytes that are
    /// safe inside an `href` query component — every byte is either RFC3986
    /// unreserved (`A-Z a-z 0-9 - _ . ~`) or part of a `%XX` uppercase-hex triplet.
    /// So NONE of `"`, `<`, `>`, `&`, `=`, space, `?`, `#`, `%` (the attribute /
    /// markup / param-smuggling breakout bytes) can ever leak unencoded — the
    /// generalization of the hostile EXAMPLE over arbitrary attacker input.
    fn assert_only_unreserved_or_percent_triplets(encoded: &str) -> Result<(), TestCaseError> {
        let bytes = encoded.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            let b = bytes[i];
            if b == b'%' {
                prop_assert!(
                    i + 2 < bytes.len(),
                    "encoded output {encoded:?} has a truncated percent-triplet at {i}"
                );
                for j in [i + 1, i + 2] {
                    prop_assert!(
                        bytes[j].is_ascii_digit()
                            || (b'A'..=b'F').contains(&bytes[j]),
                        "encoded output {encoded:?} must use UPPERCASE hex digits; \
                         byte {:?} at {j} is not 0-9/A-F",
                        bytes[j] as char
                    );
                }
                i += 3;
            } else {
                prop_assert!(
                    b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~'),
                    "encoded output {encoded:?} leaked a non-unreserved byte {:?} at \
                     {i} OUTSIDE a percent-triplet — it could break out of the href \
                     attribute or smuggle a query param (ADR-044 §security)",
                    b as char
                );
                i += 1;
            }
        }
        Ok(())
    }

    proptest! {
        /// Property (ADR-044 §security — round-trip exactness + injection boundary):
        /// for ANY string (including the hostile `"<>&%?=# ` bytes), the encoder
        /// (1) emits ONLY unreserved bytes or `%XX` triplets (nothing can break out
        /// of the `href`), AND (2) is the EXACT inverse of the inbound decode — a
        /// followed traversal link decodes back to the byte-for-byte original subject,
        /// so the linked key resolves to the SAME survey. Generalizes the hostile
        /// EXAMPLE oracle over arbitrary attacker-controlled input.
        #[test]
        fn encode_query_component_is_injection_safe_and_round_trips(value in ".*") {
            let encoded = encode_query_component(&value);
            assert_only_unreserved_or_percent_triplets(&encoded)?;
            prop_assert_eq!(
                percent_decode_query_component(&encoded),
                value.clone(),
                "decode(encode(s)) must equal s exactly (round-trip) for {:?}",
                value
            );
        }

        /// Property: the hostile breakout bytes are ALWAYS encoded — for any string,
        /// none of `"`, `<`, `>`, `&`, space, `?`, `#`, `%`, `=` survives unencoded
        /// in the output (each becomes its `%XX` form), so no second attribute, no
        /// `<script>`, and no smuggled `&param=`/`#fragment` can appear in the href.
        #[test]
        fn encode_query_component_never_leaks_a_hostile_byte(value in ".*") {
            let encoded = encode_query_component(&value);
            for hostile in ['"', '<', '>', '&', ' ', '?', '#', '%', '='] {
                // The only `%` in the output begins a triplet; a hostile char that was
                // present in the input must have been replaced by `%XX`, so it cannot
                // appear as a RAW char. (`%` itself encodes to `%25`, so a raw `%` only
                // ever heads a valid triplet — checked by the round-trip property.)
                if hostile != '%' {
                    prop_assert!(
                        !encoded.contains(hostile),
                        "hostile byte {hostile:?} leaked unencoded into {encoded:?}"
                    );
                }
            }
        }
    }

    /// Behavior (ADR-044 Q1 bare-DID): the `/score` cross-link reduces a fragmented
    /// signing DID to its BARE form before encoding (matches the slice-09 convention).
    #[test]
    fn href_score_uses_the_bare_did_without_the_signing_fragment() {
        let href = href_score("did:plc:rachel-test#org.openlore.application");
        assert_eq!(href, "/score?contributor=did%3Aplc%3Arachel-test");
    }

    /// Behavior (US-GT-002 Example 1 / AC-002.3 — GT-4 oracle): the "Contributors who
    /// claimed" section renders EACH distinct contributor DID as a render-only `<a href>`
    /// link to `/score?contributor=<bare-did>` (the slice-09 terminus REUSED; bare-DID
    /// form, ADR-044 Q1), in first-seen ORDER, with a spanning contributor appearing
    /// ONCE (deduped). Two distinct authors are NEVER merged into one aggregate link.
    #[test]
    fn render_project_fragment_lists_contributors_as_deduped_ordered_score_links() {
        // Two distinct authors on the SAME edge — both must appear as their OWN /score
        // link (no merge); the first author's fragmented signing DID reduces to bare.
        let rows = [
            survey_row(
                "did:plc:maria#org.openlore.application",
                "bafy1",
                "github:rust-lang/cargo",
                "phil-a",
                0.92,
            ),
            survey_row(
                "did:plc:tobias-test",
                "bafy2",
                "github:rust-lang/cargo",
                "phil-a",
                0.70,
            ),
        ];
        let view = group_project("github:rust-lang/cargo", &rows);
        let html = render_project_fragment(&view).into_string();
        // The labeled section is present.
        assert!(
            html.contains("Contributors who claimed"),
            "the contributors section must be labeled; {html}"
        );
        // BOTH distinct contributors render as their OWN bare-DID /score anchor — never
        // merged into one aggregate, the signing #fragment dropped (bare-DID form).
        assert!(
            html.contains(r#"<a href="/score?contributor=did%3Aplc%3Amaria">"#),
            "Maria must render as a bare-DID /score link; {html}"
        );
        assert!(
            html.contains(r#"<a href="/score?contributor=did%3Aplc%3Atobias-test">"#),
            "Tobias must render as a bare-DID /score link; {html}"
        );
        // Scope the dedup/order assertions to the "Contributors who claimed" LIST
        // section (the edge-row author links reuse the SAME href, so the whole-document
        // count would double — the contract is on the distinct contributor LIST).
        let list = html
            .split_once("Contributors who claimed")
            .expect("contributors section present")
            .1;
        // Deduped + order-preserved within the list: Maria (first-seen) precedes Tobias.
        let maria_at = list
            .find("contributor=did%3Aplc%3Amaria")
            .expect("Maria link present in list");
        let tobias_at = list
            .find("contributor=did%3Aplc%3Atobias-test")
            .expect("Tobias link present in list");
        assert!(maria_at < tobias_at, "first-seen order preserved; {html}");
        // Each distinct contributor appears EXACTLY ONCE in the list (deduped — never
        // merged, never duplicated).
        assert_eq!(
            list.matches("contributor=did%3Aplc%3Amaria").count(),
            1,
            "Maria appears once in the contributors list (deduped); {html}"
        );
        assert_eq!(
            list.matches("contributor=did%3Aplc%3Atobias-test").count(),
            1,
            "Tobias appears once in the contributors list (deduped); {html}"
        );
    }

    // -------------------------------------------------------------------------
    // Graph-Traversal view — the SYMMETRIC philosophy survey (slice-10 / step
    // 02-01): group_philosophy + render_philosophy_fragment / render_philosophy_page.
    // The object→philosophy mirror of the project oracles, swapping subject↔object:
    // `group_philosophy` groups BY subject (the project that embodies the philosophy),
    // and the group key links to `/project?subject=` (vs `/philosophy?object=`).
    // -------------------------------------------------------------------------

    /// Behavior (data-models.md §2 / I-GT-3, symmetric to the project oracle):
    /// `group_philosophy` groups a philosophy's survey rows by `subject` (the project
    /// that embodies it), one group per distinct subject, contributors deduped +
    /// order-preserved.
    #[test]
    fn group_philosophy_groups_by_subject_with_deduped_contributors() {
        let rows = [
            survey_row("did:plc:rachel-test", "bafy1", "github:NixOS/nixpkgs", "phil-x", 0.92),
            survey_row("did:plc:rachel-test", "bafy2", "github:bazelbuild/bazel", "phil-x", 0.85),
        ];
        let view = group_philosophy("phil-x", &rows);
        let TraversalView::Found { entity, groups, contributors } = view else {
            panic!("a non-empty survey must group to Found; got {view:?}");
        };
        assert_eq!(entity, "phil-x");
        assert_eq!(groups.len(), 2, "two distinct subjects → two groups");
        assert_eq!(groups[0].key, "github:NixOS/nixpkgs");
        assert_eq!(groups[1].key, "github:bazelbuild/bazel");
        // The spanning contributor appears ONCE in the contributor list (deduped).
        assert_eq!(contributors, vec!["did:plc:rachel-test".to_string()]);
    }

    /// Behavior (I-GT-3 anti-merging, symmetric): two DISTINCT authors on the SAME
    /// subject (project) render as TWO `EdgeRow`s under ONE group key — never averaged.
    #[test]
    fn group_philosophy_keeps_two_authors_on_one_subject_as_two_rows() {
        let rows = [
            survey_row("did:plc:maria", "bafy1", "github:NixOS/nixpkgs", "phil-x", 0.92),
            survey_row("did:plc:tobias-test", "bafy2", "github:NixOS/nixpkgs", "phil-x", 0.70),
        ];
        let view = group_philosophy("phil-x", &rows);
        let TraversalView::Found { groups, contributors, .. } = view else {
            panic!("expected Found");
        };
        assert_eq!(groups.len(), 1, "one shared subject → one group");
        assert_eq!(groups[0].edges.len(), 2, "two authors → two edges (no merge)");
        assert_eq!(contributors.len(), 2, "two distinct contributors");
    }

    /// Behavior (I-GT-3 / I-GT-5, symmetric): `render_philosophy_fragment` carries the
    /// `#traversal-results` id, the group key (a project) as a `/project?subject=`
    /// traversal href, each edge's author DID (a `/score?contributor=` link), the
    /// VERBATIM confidence (`0.92`) + the REUSED display-only bucket + the cid.
    #[test]
    fn render_philosophy_fragment_attributes_each_edge_verbatim_with_bucket_and_cid() {
        let rows = [survey_row(
            "did:plc:rachel-test",
            "bafyedge1",
            "github:NixOS/nixpkgs",
            "org.openlore.philosophy.reproducible-builds",
            0.92,
        )];
        let view = group_philosophy("org.openlore.philosophy.reproducible-builds", &rows);
        let html = render_philosophy_fragment(&view).into_string();
        assert!(html.contains(TRAVERSAL_RESULTS_ID), "fragment must carry the region id; {html}");
        assert!(
            html.contains("/project?subject="),
            "the group key (a project) must be a /project traversal href; {html}"
        );
        assert!(
            html.contains("/score?contributor="),
            "the author must be a /score traversal link; {html}"
        );
        assert!(html.contains("did:plc:rachel-test"), "edge must attribute its author; {html}");
        assert!(html.contains("0.92"), "confidence must render VERBATIM (0.92, not 0.9); {html}");
        assert!(html.contains("triangulated"), "the REUSED display-only bucket must show; {html}");
        assert!(html.contains("bafyedge1"), "the edge must name its cid; {html}");
        // NO full-page chrome (I-GT-6 / I-HX-1).
        assert!(!html.contains("<!DOCTYPE") && !html.contains("<html"), "fragment has no chrome; {html}");
        // The philosophy fragment groups BY subject → it must NOT link to /philosophy.
        assert!(!html.contains("/philosophy?object="), "philosophy survey keys link to /project, not /philosophy; {html}");
    }

    /// Behavior (I-GT-4, symmetric): `render_philosophy_fragment` for a `NoClaims` view
    /// names the queried entity + the guided notice, and fabricates NO edge.
    #[test]
    fn render_philosophy_fragment_no_claims_names_entity_and_fabricates_no_edge() {
        let view = TraversalView::NoClaims {
            entity: "org.openlore.philosophy.actor-model".to_string(),
        };
        let html = render_philosophy_fragment(&view).into_string();
        assert!(html.contains(TRAVERSAL_RESULTS_ID));
        assert!(html.contains("org.openlore.philosophy.actor-model"), "must name the queried entity; {html}");
        assert!(html.contains(TRAVERSAL_NO_CLAIMS_NOTICE), "must show the guided notice; {html}");
        assert!(
            !html.contains("/project?subject=") && !html.contains("/score?contributor="),
            "a NoClaims render must fabricate NO traversal edge; {html}"
        );
    }

    /// Behavior (I-GT-6 parity by construction, symmetric): `render_philosophy_page`
    /// EMBEDS the EXACT `render_philosophy_fragment` region verbatim, plus full-page
    /// chrome.
    #[test]
    fn render_philosophy_page_embeds_the_fragment_region_with_chrome() {
        let rows = [survey_row(
            "did:plc:rachel-test",
            "bafyedge1",
            "github:NixOS/nixpkgs",
            "phil-x",
            0.92,
        )];
        let view = group_philosophy("phil-x", &rows);
        let fragment = render_philosophy_fragment(&view).into_string();
        let page = render_philosophy_page(&view);
        assert!(
            page.contains(&fragment),
            "the full page must EMBED the exact traversal-results fragment (parity by \
             construction, I-GT-6); page:\n{page}"
        );
        assert!(
            page.to_lowercase().contains("<!doctype html>"),
            "the full page must carry full-page chrome; page:\n{page}"
        );
    }

    // =========================================================================
    // slice-12 — the per-row "Countered" PRESENCE FLAG on the /claims LIST
    // (US-LF-002/003; ADR-048). The flag is a render-only one-hop link, set in the
    // EFFECT shell via `from_row_with_presence`, so the pure render stays a TOTAL
    // function of (page, presence). These oracles pin: flag IFF in the presence set,
    // un-countered → no marker, presence-only single neutral flag, and that the flag
    // is ADDITIVE (it never changes row order / count / confidence).
    // =========================================================================

    /// Build a boundary `ports::ClaimRow` (the shell's input to
    /// `from_row_with_presence`) at a fixed timestamp.
    fn claim_row(cid: &str, subject: &str, confidence: f64) -> ClaimRow {
        ClaimRow {
            cid: cid.to_string(),
            subject: subject.to_string(),
            predicate: "embodiesPhilosophy".to_string(),
            object: "org.openlore.philosophy.x".to_string(),
            confidence,
            author_did: "did:plc:maria#org.openlore.application".to_string(),
            composed_at: chrono::Utc::now(),
        }
    }

    /// The exact render-only one-hop flag anchor for a countered row.
    fn flag_anchor(cid: &str) -> String {
        format!("<a href=\"/claims/{cid}\">{COUNTERED_PRESENCE_FLAG}</a>")
    }

    /// Oracle: `from_row_with_presence` sets `is_countered = true` IFF the row's CID
    /// is a member of the presence set, and FALSE otherwise (presence membership, the
    /// adapter's DISTINCT subset). A total projection — never fails.
    #[test]
    fn from_row_with_presence_flags_iff_cid_in_presence_set() {
        let countered = claim_row("bafyCountered", "github:rust-lang/cargo", 0.90);
        let plain = claim_row("bafyPlain", "github:rust-lang/rust", 0.90);
        let presence: std::collections::HashSet<String> =
            ["bafyCountered".to_string()].into_iter().collect();

        let countered_view = ClaimRowView::from_row_with_presence(&countered, &presence);
        let plain_view = ClaimRowView::from_row_with_presence(&plain, &presence);

        assert!(
            countered_view.is_countered,
            "a row whose CID is in the presence set must be flagged countered"
        );
        assert!(
            !plain_view.is_countered,
            "a row whose CID is NOT in the presence set must NOT be flagged"
        );
    }

    /// Oracle: `from_row_with_presence` carries every display field through UNCHANGED
    /// from `from_row` — the flag is ADDITIVE only (it adds `is_countered`, it does
    /// not alter subject/predicate/object/confidence/cid).
    #[test]
    fn from_row_with_presence_preserves_every_display_field() {
        let boundary = claim_row("bafyX", "github:rust-lang/cargo", 0.73);
        let empty: std::collections::HashSet<String> = std::collections::HashSet::new();

        let plain = ClaimRowView::from_row(&boundary);
        let with_presence = ClaimRowView::from_row_with_presence(&boundary, &empty);

        assert_eq!(with_presence.cid, plain.cid);
        assert_eq!(with_presence.subject, plain.subject);
        assert_eq!(with_presence.predicate, plain.predicate);
        assert_eq!(with_presence.object, plain.object);
        assert_eq!(with_presence.confidence, plain.confidence);
        assert!(
            !with_presence.is_countered,
            "an empty presence set flags NOTHING"
        );
    }

    /// Oracle (US-LF-002 / I-LF-6): a COUNTERED row renders the neutral "Countered"
    /// marker as a render-only `<a href="/claims/{cid}">Countered</a>` one-hop link;
    /// an UN-countered row renders NO such marker (no-noise, I-LF-2).
    #[test]
    fn countered_row_renders_one_hop_link_uncountered_renders_none() {
        let countered = ClaimRowView {
            cid: "bafyCountered".to_string(),
            subject: "github:rust-lang/cargo".to_string(),
            predicate: "embodiesPhilosophy".to_string(),
            object: "org.openlore.philosophy.x".to_string(),
            confidence: 0.90,
            is_countered: true,
        };
        let plain = ClaimRowView {
            is_countered: false,
            cid: "bafyPlain".to_string(),
            ..countered.clone()
        };
        let page = PageView::new(vec![countered.clone(), plain.clone()]);
        let html = render_claims_table_fragment(&page).into_string();

        assert!(
            html.contains(&flag_anchor("bafyCountered")),
            "the countered row must render the one-hop flag link; html:\n{html}"
        );
        assert!(
            !html.contains(&flag_anchor("bafyPlain")),
            "the un-countered row must render NO flag link; html:\n{html}"
        );
        // No-noise: no "0 counters" / count / verdict text anywhere.
        for noise in ["0 counters", "disputed by", "no disagreement"] {
            assert!(!html.contains(noise), "no-noise: {noise:?} must be absent; {html}");
        }
    }

    /// Oracle (I-LF-2 / I-LF-4 — additive only): the presence flag NEVER changes row
    /// ORDER, COUNT, or any row's verbatim CONFIDENCE. Rendering the SAME page with
    /// and without flags differs ONLY by the additive marker — the CID order, the row
    /// count, and the confidence cells are byte-identical once the markers are elided.
    #[test]
    fn the_flag_is_additive_order_count_confidence_unchanged() {
        let rows_flagged = vec![
            ClaimRowView {
                cid: "bafyA".to_string(),
                subject: "s-a".to_string(),
                predicate: "p".to_string(),
                object: "o".to_string(),
                confidence: 0.91,
                is_countered: true,
            },
            ClaimRowView {
                cid: "bafyB".to_string(),
                subject: "s-b".to_string(),
                predicate: "p".to_string(),
                object: "o".to_string(),
                confidence: 0.42,
                is_countered: false,
            },
        ];
        let rows_plain: Vec<ClaimRowView> = rows_flagged
            .iter()
            .cloned()
            .map(|mut r| {
                r.is_countered = false;
                r
            })
            .collect();

        let flagged = render_claims_table_fragment(&PageView::new(rows_flagged)).into_string();
        let plain = render_claims_table_fragment(&PageView::new(rows_plain)).into_string();

        // Eliding the additive markers from the flagged render yields the plain render
        // BYTE-for-byte: order, count, and confidence cells are unchanged.
        let elided = flagged.replace(&flag_anchor("bafyA"), "");
        assert_eq!(
            elided, plain,
            "the flag must be ADDITIVE only — eliding the marker must reproduce the \
             un-flagged render byte-for-byte (order/count/confidence unchanged)"
        );
        // The verbatim confidence cells are present in BOTH renders.
        assert!(plain.contains("0.91") && plain.contains("0.42"));
        assert!(flagged.contains("0.91") && flagged.contains("0.42"));
    }

    proptest! {
        /// Property: the list render is a TOTAL function of (page, presence) — for ANY
        /// vec of rows with ANY per-row `is_countered`, rendering never panics, and a
        /// row carries the flag link IFF `is_countered` is true.
        #[test]
        fn render_is_total_over_page_and_presence(
            flags in proptest::collection::vec(any::<bool>(), 0..8usize)
        ) {
            let rows: Vec<ClaimRowView> = flags
                .iter()
                .enumerate()
                .map(|(i, &countered)| ClaimRowView {
                    cid: format!("bafy{i:03}"),
                    subject: format!("s-{i}"),
                    predicate: "p".to_string(),
                    object: "o".to_string(),
                    confidence: 0.5,
                    is_countered: countered,
                })
                .collect();
            let html = render_claims_table_fragment(&PageView::new(rows)).into_string();
            for (i, &countered) in flags.iter().enumerate() {
                let anchor = flag_anchor(&format!("bafy{i:03}"));
                prop_assert_eq!(
                    html.contains(&anchor),
                    countered,
                    "row {} flag presence must equal is_countered={}",
                    i,
                    countered
                );
            }
        }
    }

    // =========================================================================
    // slice-13 — the per-row "Countered" PRESENCE FLAG on the FEDERATED /peer-claims
    // LIST (US-CF-002; ADR-049). MIRRORS the slice-12 ClaimRowView oracles EXACTLY on
    // the PeerClaimRowView: flag set in the EFFECT shell via `from_row_with_presence`
    // (the pure render stays a TOTAL function of (page, presence)), flag IFF the row's
    // cid is in the presence set, un-countered → no marker, and the flag is ADDITIVE
    // (it never changes the peer ORIGIN, confidence, row order, or count).
    // =========================================================================

    /// Build a boundary `ports::PeerClaimRow` (the shell's input to the federated
    /// `from_row_with_presence`) at a fixed timestamp.
    fn peer_claim_row(cid: &str, subject: &str, confidence: f64) -> PeerClaimRow {
        PeerClaimRow {
            cid: cid.to_string(),
            subject: subject.to_string(),
            predicate: "embodiesPhilosophy".to_string(),
            object: "org.openlore.philosophy.x".to_string(),
            confidence,
            origin: PeerOrigin::Known {
                author_did: "did:plc:peer-axum".to_string(),
                fetched_from_pds: "https://pds.example.test".to_string(),
            },
            composed_at: chrono::Utc::now(),
        }
    }

    /// Oracle (US-CF-002): the FEDERATED `PeerClaimRowView::from_row_with_presence` sets
    /// `is_countered = true` IFF the row's CID is a member of the presence set, and FALSE
    /// otherwise (presence membership, the adapter's DISTINCT subset). A total projection.
    #[test]
    fn peer_from_row_with_presence_flags_iff_cid_in_presence_set() {
        let countered = peer_claim_row("bafyPeerCountered", "github:peer/axum", 0.70);
        let plain = peer_claim_row("bafyPeerPlain", "github:peer/tokio", 0.70);
        let presence: std::collections::HashSet<String> =
            ["bafyPeerCountered".to_string()].into_iter().collect();

        let countered_view = PeerClaimRowView::from_row_with_presence(&countered, &presence);
        let plain_view = PeerClaimRowView::from_row_with_presence(&plain, &presence);

        assert!(
            countered_view.is_countered,
            "a peer row whose CID is in the presence set must be flagged countered"
        );
        assert!(
            !plain_view.is_countered,
            "a peer row whose CID is NOT in the presence set must NOT be flagged"
        );
    }

    /// Oracle (US-CF-002 — additive only): the FEDERATED `from_row_with_presence` carries
    /// every display field (including the peer ORIGIN) through UNCHANGED from `from_row` —
    /// the flag is ADDITIVE only; an empty presence set flags NOTHING.
    #[test]
    fn peer_from_row_with_presence_preserves_every_display_field() {
        let boundary = peer_claim_row("bafyPeerX", "github:peer/serde", 0.73);
        let empty: std::collections::HashSet<String> = std::collections::HashSet::new();

        let plain = PeerClaimRowView::from_row(&boundary);
        let with_presence = PeerClaimRowView::from_row_with_presence(&boundary, &empty);

        assert_eq!(with_presence.cid, plain.cid);
        assert_eq!(with_presence.subject, plain.subject);
        assert_eq!(with_presence.predicate, plain.predicate);
        assert_eq!(with_presence.object, plain.object);
        assert_eq!(with_presence.confidence, plain.confidence);
        assert_eq!(
            with_presence.origin, plain.origin,
            "the peer ORIGIN must carry through unchanged beside the flag (I-CF-4)"
        );
        assert!(
            !with_presence.is_countered,
            "an empty presence set flags NOTHING"
        );
    }

    /// The exact render-only one-hop flag anchor for a countered peer row (the SAME
    /// `<a href="/claims/{cid}">Countered</a>` the slice-12 own-list flag emits).
    fn peer_flag_anchor(cid: &str) -> String {
        format!("<a href=\"/claims/{cid}\">{COUNTERED_PRESENCE_FLAG}</a>")
    }

    /// Oracle (US-CF-002 / I-CF-6): a COUNTERED peer row renders the neutral "Countered"
    /// marker as a render-only `<a href="/claims/{cid}">Countered</a>` one-hop link; an
    /// UN-countered peer row renders NO such marker (no-noise, I-CF-2).
    #[test]
    fn countered_peer_row_renders_one_hop_link_uncountered_renders_none() {
        let countered = PeerClaimRowView {
            cid: "bafyPeerCountered".to_string(),
            subject: "github:peer/axum".to_string(),
            predicate: "embodiesPhilosophy".to_string(),
            object: "org.openlore.philosophy.x".to_string(),
            confidence: 0.70,
            origin: PeerOrigin::Known {
                author_did: "did:plc:peer-axum".to_string(),
                fetched_from_pds: "https://pds.example.test".to_string(),
            },
            is_countered: true,
        };
        let plain = PeerClaimRowView {
            is_countered: false,
            cid: "bafyPeerPlain".to_string(),
            ..countered.clone()
        };
        let page = PageView::new(vec![countered.clone(), plain.clone()]);
        let html = render_peer_claims_table_fragment(&page).into_string();

        assert!(
            html.contains(&peer_flag_anchor("bafyPeerCountered")),
            "the countered peer row must render the one-hop flag link; html:\n{html}"
        );
        assert!(
            !html.contains(&peer_flag_anchor("bafyPeerPlain")),
            "the un-countered peer row must render NO flag link; html:\n{html}"
        );
        // No-noise: no count / verdict text anywhere.
        for noise in ["0 counters", "disputed by", "no disagreement"] {
            assert!(!html.contains(noise), "no-noise: {noise:?} must be absent; {html}");
        }
    }

    /// Oracle (I-CF-2 / I-CF-4 — additive only): the peer-claims presence flag NEVER
    /// changes row ORDER, COUNT, the peer ORIGIN cell, or any row's verbatim CONFIDENCE.
    /// Rendering the SAME page with and without flags differs ONLY by the additive marker.
    #[test]
    fn the_peer_flag_is_additive_order_count_origin_confidence_unchanged() {
        let rows_flagged = vec![
            PeerClaimRowView {
                cid: "bafyPeerA".to_string(),
                subject: "s-a".to_string(),
                predicate: "p".to_string(),
                object: "o".to_string(),
                confidence: 0.91,
                origin: PeerOrigin::Known {
                    author_did: "did:plc:peer-a".to_string(),
                    fetched_from_pds: "https://pds.a.test".to_string(),
                },
                is_countered: true,
            },
            PeerClaimRowView {
                cid: "bafyPeerB".to_string(),
                subject: "s-b".to_string(),
                predicate: "p".to_string(),
                object: "o".to_string(),
                confidence: 0.42,
                origin: PeerOrigin::Known {
                    author_did: "did:plc:peer-b".to_string(),
                    fetched_from_pds: "https://pds.b.test".to_string(),
                },
                is_countered: false,
            },
        ];
        let rows_plain: Vec<PeerClaimRowView> = rows_flagged
            .iter()
            .cloned()
            .map(|mut r| {
                r.is_countered = false;
                r
            })
            .collect();

        let flagged =
            render_peer_claims_table_fragment(&PageView::new(rows_flagged)).into_string();
        let plain = render_peer_claims_table_fragment(&PageView::new(rows_plain)).into_string();

        // Eliding the additive marker from the flagged render yields the plain render
        // BYTE-for-byte: order, count, origin, and confidence cells are unchanged.
        let elided = flagged.replace(&peer_flag_anchor("bafyPeerA"), "");
        assert_eq!(
            elided, plain,
            "the peer flag must be ADDITIVE only — eliding the marker must reproduce the \
             un-flagged render byte-for-byte (order/count/origin/confidence unchanged)"
        );
        // The verbatim confidence cells + the peer-origin DIDs are present in BOTH renders.
        assert!(plain.contains("0.91") && plain.contains("0.42"));
        assert!(flagged.contains("0.91") && flagged.contains("0.42"));
        assert!(plain.contains("did:plc:peer-a") && flagged.contains("did:plc:peer-a"));
    }

    proptest! {
        /// Property: the peer-claims list render is a TOTAL function of (page, presence) —
        /// for ANY vec of rows with ANY per-row `is_countered`, rendering never panics, and
        /// a row carries the flag link IFF `is_countered` is true.
        #[test]
        fn peer_render_is_total_over_page_and_presence(
            flags in proptest::collection::vec(any::<bool>(), 0..8usize)
        ) {
            let rows: Vec<PeerClaimRowView> = flags
                .iter()
                .enumerate()
                .map(|(i, &countered)| PeerClaimRowView {
                    cid: format!("bafyPeer{i:03}"),
                    subject: format!("s-{i}"),
                    predicate: "p".to_string(),
                    object: "o".to_string(),
                    confidence: 0.5,
                    origin: PeerOrigin::Known {
                        author_did: format!("did:plc:peer-{i}"),
                        fetched_from_pds: "https://pds.example.test".to_string(),
                    },
                    is_countered: countered,
                })
                .collect();
            let html = render_peer_claims_table_fragment(&PageView::new(rows)).into_string();
            for (i, &countered) in flags.iter().enumerate() {
                let anchor = peer_flag_anchor(&format!("bafyPeer{i:03}"));
                prop_assert_eq!(
                    html.contains(&anchor),
                    countered,
                    "peer row {} flag presence must equal is_countered={}",
                    i,
                    countered
                );
            }
        }
    }
}
