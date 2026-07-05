//! Shared viewer-render infrastructure: page chrome, generic pagination,
//! verbatim formatting, counter/countered presence rendering, nav URLs, and the
//! percent-encoding/href builders reused across every feature surface.

use super::*;

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
pub fn clamp_page(page: u64, page_size: u64, total: u64) -> u64 {
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

/// The HTML `id` of the OUTER content region every full page wraps its surface
/// body in — `<main id="viewer-main">` (slice-21 / ADR-058 D1). The THIRD reserved
/// swap target, sibling of [`VIEW_PANEL_ID`] / [`CLAIMS_TABLE_ID`]: a boosted
/// left-nav click returns the FULL page and the client `hx-select`s this region, so
/// the swapped content is byte-equivalent to the full-page content region by
/// construction (I-HX-5). The persistent left nav renders OUTSIDE this region so it
/// stays mounted across a content-only swap. Held in ONE place so the shell, the
/// nav's `hx-target`/`hx-select`, and any future reference cannot drift apart.
pub const VIEWER_MAIN_ID: &str = "viewer-main";

/// The HTML `id` of the persistent left-nav CONTAINER — `<nav id="viewer-nav">`
/// (slice-21 / ADR-058 D2). Rendered OUTSIDE [`VIEWER_MAIN_ID`] on every full page,
/// it persists across a boosted content-only swap (never torn down, no flash /
/// scroll-reset). Held in ONE place so a mutation to the container id has exactly
/// one site to attack (pinned by the unit property test).
pub const VIEWER_NAV_ID: &str = "viewer-nav";

/// The HTML `id` of the inner link-list an out-of-band active-marker swap targets —
/// `<ul id="viewer-nav-items">` (slice-21 / ADR-058 D2/D5). Nested inside
/// [`VIEWER_NAV_ID`] so the container persists while (a later step's) OOB
/// `hx-swap-oob="innerHTML"` copy replaces ONLY this list's children to update the
/// active marker in place. Held in ONE place (one mutation site).
pub const VIEWER_NAV_ITEMS_ID: &str = "viewer-nav-items";

/// The 8 shipped top-level entry-point surfaces the persistent left nav (and the
/// slice-17 landing hub) link, as `(label, url)` pairs. The `url` is the route's
/// URL CONST (NOT a hardcoded literal that could drift, R-LD-4). The SINGLE source
/// of truth for the surface set (AC-001.3): [`render_viewer_nav`] reads it in-module
/// (slice-21 / ADR-058 D2 co-locates it here so the nav needs no `common -> landing`
/// back-edge), and `landing.rs`'s inline hub reads the SAME table. Held in ONE place
/// so a dropped surface is a single, mutation-killable site.
pub(crate) const LANDING_HUB_SURFACES: &[(&str, &str)] = &[
    ("My Claims", MY_CLAIMS_URL),
    ("Peer Claims", PEER_CLAIMS_URL),
    ("Project Survey", PROJECT_URL),
    ("Philosophy Survey", PHILOSOPHY_URL),
    ("Contributor Score", SCORE_URL),
    ("Network Search", SEARCH_URL),
    ("Live Scrape", SCRAPE_URL),
    ("Peer Subscriptions", PEERS_URL),
];

/// Render the PERSISTENT LEFT NAV — `<nav id="viewer-nav">` wrapping a
/// `<ul id="viewer-nav-items">` of one plain `<a href=url>` per
/// [`LANDING_HUB_SURFACES`] surface (slice-21 / ADR-058 D2). PURE total function
/// over the `active` key — emits ordinary markup, header-unaware. The `<nav>`
/// carries `hx-boost="true"` + `hx-target="#viewer-main"` + `hx-select="#viewer-main"`
/// + `hx-swap="innerHTML"` (cascades to its `<a>` children): a boosted nav click
/// fetches the FULL page and the client `hx-select`s the [`VIEWER_MAIN_ID`] content
/// region, so the nav (outside it) stays mounted and the URL is pushed into history
/// (AC-002.2). Every item is a plain `<a href>` — with JS off the links do full-page
/// navigation (no-JS navigable, AC-001.4 / I-HX-1), NO form / button / mutating
/// control (read-only, C-1). The item whose `url` equals `active` gets a neutral,
/// semantic `aria-current="page"` marker (AC-001.2); NO other item does, and a
/// non-member `active` key (e.g. the landing `""`) marks NOTHING. Single-source: the
/// item set is derived SOLELY from [`LANDING_HUB_SURFACES`] (AC-001.3) — no second,
/// driftable literal list.
pub fn render_viewer_nav(active: &str) -> Markup {
    html! {
        nav id=(VIEWER_NAV_ID)
            hx-boost="true"
            hx-target=(format!("#{VIEWER_MAIN_ID}"))
            hx-select=(format!("#{VIEWER_MAIN_ID}"))
            hx-swap="innerHTML" {
            ul id=(VIEWER_NAV_ITEMS_ID) {
                (render_viewer_nav_links(active))
            }
        }
    }
}

/// Render the SHARED per-surface `<li><a href>` link items the persistent nav and its
/// out-of-band update copy BOTH wrap (slice-21 / ADR-058 D2/D5). PURE total function
/// over the `active` key: ONE `<li>` per [`LANDING_HUB_SURFACES`] surface, each a plain
/// `<a href=url>` (no-JS navigable — never an `hx-get`-only affordance) carrying its
/// label; the item whose `url` equals `active` gets the neutral, semantic
/// `aria-current="page"` marker (AC-001.2 / 002.3), NO other item does, and a non-member
/// `active` key marks NOTHING. This is the SINGLE link-rendering site (AC-001.3 — no
/// duplicated link literal): [`render_viewer_nav`] wraps it in the `<nav id="viewer-nav">`
/// + `<ul id="viewer-nav-items">` chrome; [`render_viewer_nav_oob`] wraps it in the bare
/// `<ul id="viewer-nav-items" hx-swap-oob="innerHTML">` OOB sibling. `pub(crate)` — an
/// internal helper, not part of the crate's public render API.
pub(crate) fn render_viewer_nav_links(active: &str) -> Markup {
    html! {
        @for (label, url) in LANDING_HUB_SURFACES {
            li {
                a href=(url) aria-current=[(*url == active).then_some("page")] {
                    (label)
                }
            }
        }
    }
}

/// Render the OUT-OF-BAND active-marker update copy — JUST the bare
/// `<ul id="viewer-nav-items" hx-swap-oob="innerHTML">` sibling wrapping the SAME
/// [`render_viewer_nav_links`] list as the in-shell [`render_viewer_nav`] (slice-21 /
/// ADR-058 D5). PURE total function over the `active` key, header-unaware. On a BOOSTED
/// response the effect shell appends this at body-end (after `</main>`): htmx processes
/// `hx-swap-oob="innerHTML"` INDEPENDENTLY of `hx-select`, replacing ONLY the inner
/// `<ul id="viewer-nav-items">`'s children — so the `<nav id="viewer-nav">` CONTAINER
/// persists (never torn down, no flash / scroll-reset) while the active marker updates
/// in place (AC-002.1 refined + AC-002.3). Emits NO `<nav>` container and NO
/// `hx-boost`/`hx-target` (those live on the persisting container, not the OOB copy).
/// The item marked `aria-current="page"` is the surface whose `url` equals `active`; a
/// non-member key (the landing / 404 `""`) marks NOTHING. Direct / no-JS loads never
/// append this copy (the effect shell emits it for boosted responses only, and htmx
/// ignores `hx-swap-oob` on an initial full-page load anyway — I-HX-1 preserved).
pub fn render_viewer_nav_oob(active: &str) -> Markup {
    html! {
        ul id=(VIEWER_NAV_ITEMS_ID) hx-swap-oob="innerHTML" {
            (render_viewer_nav_links(active))
        }
    }
}

/// Compose a complete full-page document AROUND a surface's content region
/// (slice-21 / ADR-058 D1/D6) — the ONE chrome helper every `render_*_page` (and the
/// 404 [`render_error`]) routes its body through. PURE total function: `(DOCTYPE)` +
/// `<html>` { [`page_head`]`(title)` + `<body>` { [`render_viewer_nav`]`(active)`
/// OUTSIDE `<main id="viewer-main">` `(content)` } }. Because the persistent nav
/// renders OUTSIDE [`VIEWER_MAIN_ID`], a boosted `hx-select="#viewer-main"` swap
/// replaces ONLY the surface content while the nav stays mounted (AC-002.1); and
/// because `#viewer-main`'s inner HTML IS the surface `content`, the boosted content
/// is byte-equivalent to the full-page content region by construction (AC-002.4 /
/// I-HX-5). `active` is the surface's own compile-time URL const at the call site
/// (NOT request-read); the landing / 404 pass `""` so no item is marked.
pub fn page_shell(title: &str, active: &str, content: Markup) -> String {
    let markup = html! {
        (DOCTYPE)
        html {
            (page_head(title))
            body {
                (render_viewer_nav(active))
                main id=(VIEWER_MAIN_ID) {
                    (content)
                }
            }
        }
    };
    markup.into_string()
}

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
pub fn htmx_script() -> Markup {
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
pub fn page_head(title: &str) -> Markup {
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
pub fn render_pagination<T>(page: &PageView<T>) -> Markup {
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

/// Render the at-a-glance "Countered" PRESENCE flag for ONE surface row, keyed only by
/// its claim `cid` and `is_countered` flag — the single SSOT body shared by every list /
/// federated / traversal presence flag (slice-12/13 / US-LF-002 / US-CF-002/003 /
/// ADR-048/049). A render-only `<a href="/claims/{cid}">Countered</a>` one-hop link to
/// that claim's slice-11 counter thread — navigation TEXT, never an executable
/// write/sign/counter control (I-LF-1 / I-CF-1). PRESENCE-only: a single neutral marker,
/// NEVER a count ("disputed by N") or a verdict. An UN-countered row (`is_countered ==
/// false`) renders NOTHING — no marker, no "0 counters" noise (no-noise discipline, I-LF-2
/// / I-CF-2). PURE total function over (`cid`, `is_countered`), so each caller's render
/// stays a total function of (page, presence). The flag text is the shared
/// [`COUNTERED_PRESENCE_FLAG`] constant (one source of truth across every surface).
/// (Distinct from the detail view's [`render_presence_flag`], which emits a non-link
/// `<p>` marker over the [`CounterThread`] ADT rather than this linked list/edge form.)
pub fn render_countered_link(cid: &str, is_countered: bool) -> Markup {
    html! {
        @if is_countered {
            a href=(format!("/claims/{cid}")) { (COUNTERED_PRESENCE_FLAG) }
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

/// The real route the Live Scrape surface is served at (`/scrape`) — the 8th and
/// final entry-point URL const, minted this slice (ADR-054 D4 / R-LD-4). The
/// landing nav hub links every shipped surface via its URL const; `/scrape` was the
/// one top-level surface lacking a const (the route arm spelled the literal). Held
/// here so the hub link and the route reference the SAME path — no drifting literal.
pub const SCRAPE_URL: &str = "/scrape";

/// The marker the landing summary renders for a count whose read FAILED (a
/// `LandingSummary` field is `None` → this marker, ADR-054 D2 / WD-LD-8). A
/// horizontal bar "—", visually + semantically DISTINCT from the digit "0" (a
/// SUCCESSFUL read of an empty store). Held in ONE place so the missing-number
/// contract has a single source of truth (and a string mutation has exactly one
/// site to attack — pinned by the unit tests below).
pub const MISSING_COUNT_MARKER: &str = "—";

/// Render ONE count VERBATIM for the landing summary: `Some(n)` → the number `n`
/// (including a genuine `Some(0)` → "0", a SUCCESSFUL read of an empty store),
/// `None` → [`MISSING_COUNT_MARKER`] "—" (a FAILED read, NEVER a fabricated 0).
/// PURE total function — the single site of the `0 ≠ missing` distinction, so a
/// mutation collapsing the two is killed by one unit test.
pub fn render_count(count: Option<usize>) -> String {
    match count {
        Some(n) => n.to_string(),
        None => MISSING_COUNT_MARKER.to_string(),
    }
}

/// Render the COUNTERED-own-claims parenthetical for BOTH the landing summary and the
/// `/claims` list header from the SAME `Option<usize>` (slice-18 / ADR-055 D3 — single
/// source). `Some(n)` → "(n countered)" (incl. a genuine `Some(0)` → "(0 countered)", a
/// SUCCESSFUL read of an honest zero), `None` → "(— countered)" (the
/// [`MISSING_COUNT_MARKER`] inside the parenthetical — a FAILED read, NEVER a fabricated
/// 0; reuses the [`render_count`] inner-number mapping so `0 ≠ missing` is one rule). The
/// copy is NEUTRAL disputed-claim awareness — never "refuted"/"false"/"disputed by N"/a
/// score/a deduction/a verdict (C-6 / WD-CC-10). Held in ONE place so the exact copy +
/// the missing-marker behaviour is a single mutation-killable site. PURE total function.
pub fn render_countered(countered: Option<usize>) -> String {
    format!("({} countered)", render_count(countered))
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
    // slice-21 (ADR-058 D6): the 404 full page ALSO routes through `page_shell` so the
    // persistent left nav is present for navigational recovery — consistent with every
    // other full page. `active = ""` (the 404 is not a nav surface), so NO nav item is
    // marked. Full-page-only (no `Shape` fork on the 404); the fragment 404 uses the
    // chrome-less [`render_claim_not_found_fragment`].
    let body = html! {
        h1 { "Claim Not Found" }
        p { (CLAIM_NOT_FOUND_NOTICE) }
        p {
            a href="/claims" { "Back to My Claims" }
        }
    };
    page_shell("OpenLore — Claim Not Found", "", body)
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

/// The neutral "Countered" PRESENCE flag rendered near a claim that has ≥1
/// counter (CT-8 / I-CT-3): a presence marker ONLY — never a verdict, a score, a
/// count ("disputed by N"), or a count-based re-rank. Held in ONE place so the
/// flag text is a single source of truth.
pub const COUNTERED_PRESENCE_FLAG: &str = "Countered";

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
pub fn href_philosophy(object: &str) -> String {
    format!("{PHILOSOPHY_URL}?object={}", encode_query_component(object))
}

/// Build the `/project?subject=<encoded>` traversal href for a subject key (the
/// subject→project edge; ADR-044). The claim-controlled `subject` is percent-encoded
/// into the query component. PURE total function. (Used by the `/philosophy` survey's
/// project group keys — the subject→project traversal edge; symmetric with
/// [`href_philosophy`].)
pub fn href_project(subject: &str) -> String {
    format!("{PROJECT_URL}?subject={}", encode_query_component(subject))
}

/// Build the `/score?contributor=<bare-did-encoded>` traversal href for an author DID
/// (the contributor→score edge; the slice-09 terminus REUSED, ADR-044 Q1 bare-DID
/// form). The DID is reduced to its BARE form ([`bare_did`]) — the signing `#fragment`
/// locator is dropped so the link matches the slice-09 `/score?contributor=` convention
/// — then percent-encoded. PURE total function.
pub fn href_score(author_did: &str) -> String {
    format!(
        "{SCORE_URL}?contributor={}",
        encode_query_component(bare_did(author_did))
    )
}

/// Reduce a DID to its BARE form — everything before a `#fragment` signing locator
/// (`did:plc:x#org.openlore.application` → `did:plc:x`). PURE total function; a DID
/// without a fragment passes through unchanged. Mirrors the adapter's `bare_did` so the
/// `/score` cross-link matches the slice-09 contributor convention (one bare-DID SSOT).
pub fn bare_did(did: &str) -> &str {
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
        let unreserved = byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~');
        if unreserved {
            out.push(byte as char);
        } else {
            out.push('%');
            out.push_str(&format!("{byte:02X}"));
        }
    }
    out
}

// =============================================================================
// Peer Subscriptions view (slice-15; ADR-052) — `GET /peers`
// =============================================================================
//
// The `/peers` route reads the operator's ACTIVE peer subscriptions over the
// read-only `StoreReadPort::list_active_peer_subscriptions` (ONE aggregate query
// — peer_subscriptions LEFT JOIN peer_claims, WHERE removed_at IS NULL, GROUP BY
// COUNT(pc.cid)), maps the flat `Vec<PeerSubscriptionSummary>` to a `PeersView`
// ADT (Subscriptions | NoSubscriptions) in the effect shell, and renders it here.
// This crate holds NO read/SQL — it PROJECTS the flat DTO (UNLIKE slice-10's
// viewer-domain → claim-domain edge, the render is a TOTAL function of the flat
// `PeerSubscriptionSummary`, so NO new pure-core dependency edge). Each ACTIVE
// peer is one attributed row: its DID VERBATIM (I-PS-3) + its PER-PEER local claim
// count + the RENDER-ONLY `openlore peer remove <bare-did>` revocation command
// (mirroring the slice-08 `render_follow_guidance` render-only `openlore peer add`
// precedent — TEXT, never an executable control; I-PS-1). When the active set is
// empty, a GUIDED empty state pointing at `openlore peer add <did>` (US-PS-003).
