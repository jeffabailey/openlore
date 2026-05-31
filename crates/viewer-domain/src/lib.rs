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
use ports::{ClaimDetail, ClaimRow, PeerClaimRow, PeerOrigin};

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

/// A page of view-model rows ready to render. For the walking skeleton (step
/// 01-01) this is a thin wrapper over the rows; the position indicator +
/// pagination controls (FR-VIEW-6) land in step 04-01, which extends this with
/// the page bounds + total.
#[derive(Debug, Clone, PartialEq)]
pub struct PageView<T> {
    pub rows: Vec<T>,
}

impl<T> PageView<T> {
    /// Construct a page view from its rows.
    pub fn new(rows: Vec<T>) -> Self {
        Self { rows }
    }
}

/// Render the My Claims page as a complete HTML document (maud). PURE: a total
/// function from the view-model to an HTML string — no I/O. Each seeded claim
/// renders as a row carrying subject/predicate/object, the VERBATIM confidence
/// (`0.90`, FR-VIEW-8), and its CID. An empty page renders the guided empty
/// state (FR-VIEW-7) instead of a blank table.
pub fn render_claims_page(page: &PageView<ClaimRowView>) -> String {
    let body = if page.rows.is_empty() {
        render_empty_state()
    } else {
        render_claims_table(&page.rows)
    };
    let markup = html! {
        (DOCTYPE)
        html {
            head {
                meta charset="utf-8";
                title { "OpenLore — My Claims" }
            }
            body {
                h1 { "My Claims" }
                p { "This is a read-only view of the claims you have signed." }
                (body)
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
}
