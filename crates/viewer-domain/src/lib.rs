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
use ports::{CandidateClaim, ClaimDetail, ClaimRow, PeerClaimRow, PeerOrigin};

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
}

impl ClaimRowView {
    /// Project a boundary [`ports::ClaimRow`] into the view-model. Total — never
    /// fails (the view-model is a strict subset of the DTO's display fields;
    /// `author_did`/`composed_at` are not shown in the list view, FR-VIEW-1).
    pub fn from_row(row: &ClaimRow) -> Self {
        Self {
            cid: row.cid.clone(),
            subject: row.subject.clone(),
            predicate: row.predicate.clone(),
            object: row.object.clone(),
            confidence: row.confidence,
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
            head {
                meta charset="utf-8";
                title { "OpenLore — My Claims" }
                script src="/static/htmx.min.js" {}
            }
            body {
                h1 { "My Claims" }
                p { "This is a read-only view of the claims you have signed." }
                (render_claims_table_fragment(page))
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
/// so the VERBATIM `0.90` rule lives in exactly one place (FR-VIEW-8).
fn render_claim_row(row: &ClaimRowView) -> Markup {
    html! {
        tr {
            td { (row.subject) }
            td { (row.predicate) }
            td { (row.object) }
            td { (render_confidence(row.confidence)) }
            td { (row.cid) }
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
pub const READ_ONLY_NOTICE: &str =
    "This viewer is read-only — nothing here can change your store.";

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
            head {
                meta charset="utf-8";
                title { "OpenLore — Viewer" }
            }
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

/// Render one claim's detail page as a complete HTML document (maud). PURE: a
/// total function from the detail view-model to an HTML string — no I/O. Shows
/// EVERY claim field (subject, predicate, object, the VERBATIM confidence,
/// author_did, composed_at, CID) PLUS the COMPLETE `evidence[]` array, one URL
/// per row in ordinal order (FR-VIEW-3 / AC-002.1). A claim with no evidence
/// shows an explicit "no evidence attached" state (FR-VIEW-3, step 02-02) rather
/// than a blank section.
pub fn render_claim_detail(claim: &ClaimDetailView) -> String {
    let markup = html! {
        (DOCTYPE)
        html {
            head {
                meta charset="utf-8";
                title { "OpenLore — Claim Detail" }
            }
            body {
                h1 { "Claim Detail" }
                p { (READ_ONLY_NOTICE) }
                (render_claim_fields(claim))
                (render_evidence_section(&claim.evidence))
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
}

impl PeerClaimRowView {
    /// Project a boundary [`ports::PeerClaimRow`] into the view-model. Total —
    /// never fails. The peer ORIGIN ADT is carried through unchanged so the
    /// rendering rule (verbatim DID for `Known`, "unknown" label for `Unknown`)
    /// lives in ONE place ([`render_peer_origin`]).
    pub fn from_row(row: &PeerClaimRow) -> Self {
        Self {
            cid: row.cid.clone(),
            subject: row.subject.clone(),
            predicate: row.predicate.clone(),
            object: row.object.clone(),
            confidence: row.confidence,
            origin: row.origin.clone(),
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
    let body = if page.rows.is_empty() {
        render_peer_empty_state()
    } else {
        render_peer_claims_table(&page.rows)
    };
    let markup = html! {
        (DOCTYPE)
        html {
            head {
                meta charset="utf-8";
                title { "OpenLore — Peer Claims" }
            }
            body {
                h1 { "Peer Claims" }
                p {
                    "This is a read-only view of claims federated from your peers \
                     — these are NOT your own claims."
                }
                (body)
            }
        }
    };
    markup.into_string()
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
            td { (row.cid) }
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
pub fn render_scrape_page(state: &ScrapeState) -> String {
    let markup = html! {
        (DOCTYPE)
        html {
            head {
                meta charset="utf-8";
                title { "OpenLore — Live Scrape" }
            }
            body {
                h1 { "Live Scrape" }
                p { (READ_ONLY_NOTICE) }
                (render_scrape_form())
                (render_scrape_result(state))
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

#[cfg(test)]
mod tests {
    //! In-crate unit + property tests for the PURE viewer core. Port-to-port at
    //! domain scope: the pure function signature IS the driving port
    //! (nw-tdd-methodology §Port-to-Port). The confidence-verbatim rendering is
    //! the load-bearing FR-VIEW-8 contract + the prime mutation target.

    use super::*;
    use proptest::prelude::*;

    fn row(cid: &str, subject: &str, predicate: &str, object: &str, confidence: f64) -> ClaimRowView {
        ClaimRowView {
            cid: cid.to_string(),
            subject: subject.to_string(),
            predicate: predicate.to_string(),
            object: object.to_string(),
            confidence,
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
        let html = render_claim_detail(&view);
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
        let html = render_claim_detail(&detail(&[]));
        assert!(
            html.contains("no evidence attached"),
            "a claim with empty evidence must show \"no evidence attached\"; got:\n{html}"
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
            let html = render_claim_detail(&detail(&url_refs));

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
            let html = render_claim_detail(&view);
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
            .map(|i| row(&format!("c{i}"), &format!("s{i}"), "p", &format!("o{i}"), 0.90))
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
        assert!(!last.has_next(), "the last page has no Next (bounded at total)");
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
        assert!(html.contains("?page=3"), "page 4 must link Prev to ?page=3; got:\n{html}");
        assert!(html.contains("?page=5"), "page 4 must link Next to ?page=5; got:\n{html}");
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
            html.contains("nothing")
                && (html.contains("signed") || html.contains("saved")),
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
        let proposals = ScrapeState::Proposals(vec![CandidateRowView::from_candidate(
            &candidate(
                "github:rust-lang/cargo",
                "embodiesPhilosophy",
                "org.openlore.philosophy.dependency-pinning",
                0.25,
                "Cargo.lock committed",
            ),
        )]);
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
        for sign_control_marker in ["name=\"sign\"", "Sign claim", "type=\"submit\" value=\"sign"] {
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
        for sign_control_marker in ["name=\"sign\"", "Sign claim", "type=\"submit\" value=\"sign"] {
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
        assert!(html.contains("<table"), "fragment carries the claims table; got:\n{html}");
        assert!(
            html.contains("51\u{2013}100 of 312"),
            "fragment shows the page-2 indicator \"51\u{2013}100 of 312\"; got:\n{html}"
        );
        assert!(html.contains("?page=1"), "fragment links Prev to ?page=1; got:\n{html}");
        assert!(html.contains("?page=3"), "fragment links Next to ?page=3; got:\n{html}");
        // The verbatim confidence rule holds in the fragment (FR-VIEW-8).
        assert!(html.contains("0.90"), "fragment renders confidence verbatim; got:\n{html}");
        // NO full-page chrome: the fragment is ONLY the swap-target region.
        let lower = html.to_lowercase();
        assert!(!lower.contains("<!doctype"), "fragment must carry no DOCTYPE; got:\n{html}");
        assert!(!lower.contains("<html"), "fragment must carry no <html> chrome; got:\n{html}");
        assert!(!lower.contains("<head"), "fragment must carry no <head> chrome; got:\n{html}");
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
        assert!(lower.contains("<!doctype html>"), "the full page carries a DOCTYPE; got:\n{page}");
        assert!(lower.contains("<html"), "the full page carries <html> chrome; got:\n{page}");
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
}
