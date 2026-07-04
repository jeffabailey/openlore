//! `GET /` — the landing dashboard summary + nav hub.

use super::*;

/// The at-a-glance LOCAL store summary the landing dashboard renders (`GET /`,
/// slice-17 / US-LD-000/001 / ADR-054 D1). THREE INDEPENDENT `Option<usize>` counts:
/// each is `Some(n)` for a SUCCESSFUL read (rendered as the number `n`, including a
/// genuine `Some(0)`) or `None` for a FAILED read (rendered as [`MISSING_COUNT_MARKER`]
/// "—", NEVER a fabricated `0`). The three are independent so one count's read
/// failing degrades ONLY that count — the other two still render their numbers (the
/// per-count `.ok()` degrade, ADR-054 D2). A flat record (the building block for the
/// pure render); the effect shell builds it by resolving each `Result<usize,
/// StoreReadError>` via `.ok()`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LandingSummary {
    /// The operator's own-claim count (`count_claims`). `None` = the read failed.
    pub own_claims: Option<usize>,
    /// The federated peer-claim count (`count_peer_claims`). `None` = read failed.
    pub peer_claims: Option<usize>,
    /// The ACTIVE peer-subscription count (`count_active_peer_subscriptions`,
    /// `removed_at IS NULL`). `None` = the read failed.
    pub active_peers: Option<usize>,
    /// The COUNTERED-own-claims count (`count_countered_own_claims`, slice-18 /
    /// ADR-055 D2 — additive, parallel to the three above, IDENTICAL degrade
    /// semantics). `Some(n)` = a SUCCESSFUL read of `n` (incl. a genuine `Some(0)` —
    /// an honest "nothing of mine has drawn a counter"); `None` = the read FAILED →
    /// the missing marker, NEVER a fabricated 0 (`0 ≠ missing`, C-5 / WD-CC-6). The
    /// countered count is disputed-claim AWARENESS rendered BESIDE the own-claims line
    /// — it never re-weights the own-claims number (additive, C-4).
    pub countered_own_claims: Option<usize>,
    /// The COUNTERED-peer-claims count (`count_countered_peer_claims`, slice-19 /
    /// ADR-056 D2 — additive, parallel to the four above, IDENTICAL degrade semantics;
    /// the deferred PEER sibling of [`LandingSummary::countered_own_claims`]). `Some(n)`
    /// = a SUCCESSFUL read of `n` (incl. a genuine `Some(0)` — an honest "nothing of my
    /// cached peer material has drawn a counter"); `None` = the read FAILED → the missing
    /// marker, NEVER a fabricated 0 (`0 ≠ missing`, C-5 / WD-PC-6). The countered-peer
    /// count is disputed-claim AWARENESS rendered BESIDE the peer-claims line — it never
    /// re-weights the peer-claims number (additive, C-4). It fails INDEPENDENTLY of the
    /// slice-18 own count (ADR-056 D4).
    pub countered_peer_claims: Option<usize>,
}

/// The 8 shipped top-level entry-point surfaces the landing nav hub links, as
/// `(label, url)` pairs. The `url` is the route's URL CONST (NOT a hardcoded
/// literal that could drift, R-LD-4) — 7 existing consts + the slice-17
/// [`SCRAPE_URL`]. The discoverability contract (WD-LD-7 / Theme 2 / C-3): the hub
/// links ALL 8, each a plain `<a href>` (no-JS navigable). Held in ONE place so a
/// dropped surface is a single, mutation-killable site.
const LANDING_HUB_SURFACES: &[(&str, &str)] = &[
    ("My Claims", MY_CLAIMS_URL),
    ("Peer Claims", PEER_CLAIMS_URL),
    ("Project Survey", PROJECT_URL),
    ("Philosophy Survey", PHILOSOPHY_URL),
    ("Contributor Score", SCORE_URL),
    ("Network Search", SEARCH_URL),
    ("Live Scrape", SCRAPE_URL),
    ("Peer Subscriptions", PEERS_URL),
];

/// Render the viewer's landing page (`GET /`) as a complete HTML document (maud).
/// PURE: a TOTAL function of the [`LandingSummary`] — no I/O, no panic on ANY of the
/// 2³ `Option` combinations. States the view is read-only (the operator is told, up
/// front, that nothing here can change her store — NFR-VIEW-1, the [`READ_ONLY_NOTICE`]
/// shared verbatim with the launch banner), renders the THREE at-a-glance LOCAL
/// counts (each `Some(n)` → the number, `None` → [`MISSING_COUNT_MARKER`] "—", ADR-054
/// D2), and a navigation hub of plain `<a href>` links to ALL 8 shipped top-level
/// surfaces ([`LANDING_HUB_SURFACES`], via their URL consts — no drifting literal,
/// R-LD-4). Full-page-only (ADR-054 D5): returns a complete document, NO `Shape`
/// fork. Every navigation affordance is a plain link — NO form/button/mutating
/// control (the front door is read-only, C-1 CARDINAL).
pub fn render_landing(summary: &LandingSummary) -> String {
    let markup = html! {
        (DOCTYPE)
        html {
            (page_head("OpenLore — Viewer"))
            body {
                h1 { "OpenLore Viewer" }
                p { (READ_ONLY_NOTICE) }
                // The at-a-glance LOCAL store summary — three INDEPENDENT counts, each
                // labelled so the operator reads WHICH count is which (Theme 1). A
                // failed read renders the missing-number marker "—", DISTINCT from a
                // genuine 0 (ADR-054 D2 / WD-LD-8).
                section {
                    // slice-18 (ADR-055 D3): the countered count renders BESIDE the
                    // UNCHANGED own-claims line ("12 own claims (3 countered)") — the
                    // own-claims `render_count` is UNTOUCHED (additive awareness, never a
                    // re-weight, C-4). The countered count flows through the SAME shared
                    // `render_countered` helper the `/claims` header uses (single source).
                    p {
                        (render_count(summary.own_claims)) " own claims "
                        (render_countered(summary.countered_own_claims))
                    }
                    // slice-19 (ADR-056 D3): the countered-PEER count renders BESIDE the
                    // UNCHANGED peer-claims line ("4 peer claims (1 countered)") — the
                    // peer-claims `render_count` is UNTOUCHED (additive awareness, never a
                    // re-weight, C-4); the slice-18 own line above is byte-untouched
                    // (WD-PC-7). The countered-peer count flows through the SAME REUSED
                    // `render_countered` helper the `/peer-claims` header uses (single
                    // source — NO new helper, WD-PC-10).
                    p {
                        (render_count(summary.peer_claims)) " peer claims "
                        (render_countered(summary.countered_peer_claims))
                    }
                    p { (render_count(summary.active_peers)) " active peers" }
                }
                // The navigation hub — every shipped surface as a plain <a href>
                // (no-JS navigable), via its URL const (no drift, R-LD-4). The ONLY
                // affordances on the front door (read-only — no write control, C-1).
                nav {
                    ul {
                        @for (label, url) in LANDING_HUB_SURFACES {
                            li {
                                a href=(url) { (label) }
                            }
                        }
                    }
                }
            }
        }
    };
    markup.into_string()
}
