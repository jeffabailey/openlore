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

    /// Behavior (FR-VIEW-7): an empty page renders the guided empty state — the
    /// operator is pointed at the CLI, not shown a blank page.
    #[test]
    fn empty_page_renders_the_guided_empty_state() {
        let page: PageView<ClaimRowView> = PageView::new(vec![]);
        let html = render_claims_page(&page);
        assert!(
            html.contains("not signed any claims yet")
                || html.contains("claims you sign with the CLI will appear here"),
            "empty My Claims page must guide the operator to the CLI; got:\n{html}"
        );
    }
}
