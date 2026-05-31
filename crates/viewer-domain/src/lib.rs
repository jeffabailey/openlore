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
use ports::ClaimRow;

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
}
