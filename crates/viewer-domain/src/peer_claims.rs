//! `/peer-claims` — the federated peer-claim list surface.

use super::*;

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
pub fn render_peer_claims_page(
    page: &PageView<PeerClaimRowView>,
    countered_peer_claims: Option<usize>,
) -> String {
    // slice-21 (ADR-058 D6): the surface body is composed through `page_shell`
    // (persistent left nav + `<main id="viewer-main">`); `active = PEER_CLAIMS_URL`
    // marks the Peer Claims nav item current. The `render_*_fragment` fns are UNCHANGED
    // (they ride `Shape::Fragment` for the tab #view-panel + paging #claims-table swaps).
    let body = html! {
        // slice-19 (ADR-056 D3): the countered-PEER count renders in the list
        // HEADER, beside the "Peer Claims" heading, through the SAME shared
        // `render_countered` helper the landing summary uses (single source — the
        // two surfaces resolve from the SAME `count_countered_peer_claims` read +
        // render through the SAME helper, so they cannot diverge, WD-PC-8). Additive
        // header text ONLY — the slice-06/07 list order/paging/count + the slice-13
        // per-row flags are UNTOUCHED (C-4 / WD-PC-9). The slice-18 OWN surfaces are
        // not on this route (peer-only, BR-PC-4 / WD-PC-7).
        h1 { "Peer Claims " (render_countered(countered_peer_claims)) }
        p {
            "This is a read-only view of claims federated from your peers \
             — these are NOT your own claims."
        }
        (render_tab_nav())
        (render_peer_claims_view_panel_fragment(page))
    };
    page_shell("OpenLore — Peer Claims", PEER_CLAIMS_URL, body)
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
/// LIST row (slice-13 / US-CF-002 / ADR-049). The FEDERATED-surface sibling of
/// [`render_list_presence_flag`]: a thin surface-typed wrapper over the shared
/// [`render_countered_link`] SSOT body, emitting the SAME shared [`COUNTERED_PRESENCE_FLAG`]
/// constant (one source of truth across surfaces).
fn render_peer_list_presence_flag(row: &PeerClaimRowView) -> Markup {
    render_countered_link(&row.cid, row.is_countered)
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
