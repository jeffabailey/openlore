
//! In-crate unit + property tests for the PURE viewer core. Port-to-port at
//! domain scope: the pure function signature IS the driving port
//! (nw-tdd-methodology §Port-to-Port). The confidence-verbatim rendering is
//! the load-bearing FR-VIEW-8 contract + the prime mutation target.

use super::*;
use proptest::prelude::*;
// Feature-module-internal render helpers CALLED directly by these unit tests.
// They are `pub(crate)` (not part of the crate's public render API), so
// `use super::*` (which only sees the crate-root `pub use` re-exports) does
// not surface them — import them by their now-modularized paths.
use crate::peers::render_remove_guidance;
use crate::score::render_weight;

fn row(cid: &str, subject: &str, predicate: &str, object: &str, confidence: f64) -> ClaimRowView {
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

    let frag = render_claim_detail_fragment(&detail(&["https://e.test/0"]), &thread).into_string();
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
    let rows = vec![counter_row(
        "did:plc:maria",
        "bafycounter",
        Some(reason),
        "",
    )];
    let thread = CounterThread::from_rows(&rows);
    let frag = render_claim_detail_fragment(&detail(&["https://e.test/0"]), &thread).into_string();

    assert!(
        frag.contains(COUNTER_THREAD_HEADING),
        "thread heading; got:\n{frag}"
    );
    assert!(
        frag.contains(COUNTERED_PRESENCE_FLAG),
        "presence flag; got:\n{frag}"
    );
    assert!(
        frag.contains("did:plc:maria"),
        "counter author DID; got:\n{frag}"
    );
    assert!(
        frag.contains("href=\"/claims/bafycounter\""),
        "counter CID render-only drill-link toward /claims/{{cid}}; got:\n{frag}"
    );
    assert!(
        frag.contains(reason),
        "verbatim reason byte-for-byte; got:\n{frag}"
    );
    // The claim's own confidence (0.95) renders VERBATIM + unchanged by the counter.
    assert!(
        frag.contains("0.95"),
        "claim confidence verbatim + unchanged; got:\n{frag}"
    );
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
    let frag = render_claim_detail_fragment(&detail(&["https://e.test/0"]), &thread).into_string();

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
        assert!(
            frag.contains(did),
            "counter author DID {did:?}; got:\n{frag}"
        );
        assert!(
            frag.contains(&format!("href=\"/claims/{cid}\"")),
            "counter CID {cid:?} drill-link; got:\n{frag}"
        );
        assert!(
            frag.contains(reason),
            "verbatim reason {reason:?}; got:\n{frag}"
        );
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
    let rows = vec![counter_row(
        "did:plc:tobias-test",
        "bafynoreason",
        None,
        "https://pds.x",
    )];
    let thread = CounterThread::from_rows(&rows);
    let frag = render_claim_detail_fragment(&detail(&[]), &thread).into_string();

    assert!(
        frag.contains("did:plc:tobias-test"),
        "author still shown; got:\n{frag}"
    );
    assert!(
        frag.contains("bafynoreason"),
        "cid still shown; got:\n{frag}"
    );
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
    let html = render_claims_page(&page, None);
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

/// Behavior (slice-18 / US-CC-002 / ADR-055 D3 — single source, prime mutation
/// target): `render_claims_page` renders the countered count in the list HEADER
/// through the SAME `render_countered` helper the landing uses, so the two
/// surfaces cannot diverge. `Some(n)` → "(n countered)" beside "My Claims",
/// `Some(0)` → "(0 countered)" (honest zero), `None` → "(— countered)" (missing
/// marker, NEVER a fabricated 0). Pins that the header render is exactly the
/// `render_countered` output — a mutant that drops the header count, fabricates
/// 0 for a missing read, or diverges the copy from the helper is killed.
#[test]
fn render_claims_page_header_shows_the_countered_count_via_render_countered() {
    let one_claim = || {
        PageView::new(vec![row(
            "bafyrust",
            "rust-lang/rust",
            "is-maintained-by",
            "The Rust Project",
            0.90,
        )])
    };
    // Some(n) → the header carries EXACTLY the render_countered(Some(n)) output,
    // beside the "My Claims" heading (single source — the helper, not a literal).
    let html_some = render_claims_page(&one_claim(), Some(3));
    assert!(
        html_some.contains(&render_countered(Some(3))),
        "the /claims header must render the countered count via render_countered \
             (Some(3) → \"(3 countered)\", single source — ADR-055 D3); got:\n{html_some}"
    );
    // Some(0) → the honest zero "(0 countered)" (a successful read of zero),
    // DISTINCT from the missing marker.
    let html_zero = render_claims_page(&one_claim(), Some(0));
    assert!(
        html_zero.contains("(0 countered)") && !html_zero.contains("(— countered)"),
        "Some(0) must render the honest zero \"(0 countered)\", NOT the missing \
             marker (0 ≠ missing, C-5); got:\n{html_zero}"
    );
    // None → the missing marker "(— countered)", NEVER a fabricated "(0 countered)"
    // (a failed read degrades to None → the marker, ADR-055 D4 / C-5).
    let html_missing = render_claims_page(&one_claim(), None);
    assert!(
        html_missing.contains(&render_countered(None)) && !html_missing.contains("(0 countered)"),
        "None must render the missing marker \"(— countered)\" via render_countered, \
             NEVER a fabricated \"(0 countered)\" (C-5); got:\n{html_missing}"
    );
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
        let html = render_claims_page(&page, None);
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
    let html = render_claims_page(&only, None);
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
    let html = render_claims_page(&paged(50, 4, 50, 312), None);
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
        let html = render_claims_page(&view, None);
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
    let html = render_claims_page(&page, None);
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

/// A `LandingSummary` with all three reads SUCCESSFUL — the walking-skeleton
/// shape (12 own, 7 peer, 2 active). One source of truth for the GREEN-path
/// landing unit tests below.
fn seeded_summary() -> LandingSummary {
    LandingSummary {
        own_claims: Some(12),
        peer_claims: Some(7),
        active_peers: Some(2),
        countered_own_claims: Some(3),
        countered_peer_claims: Some(1),
    }
}

/// Behavior (AC-001.2 / NFR-VIEW-1): the landing page states the view is
/// read-only (VERBATIM assurance) and links back to the My Claims list — now
/// over the slice-17 `LandingSummary` signature.
#[test]
fn landing_page_states_read_only_and_links_to_claims() {
    let html = render_landing(&seeded_summary());
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

/// Behavior (slice-17 / US-LD-001 Theme 2 / C-3 / R-LD-4 / ADR-054 D4): the
/// landing nav hub links ALL 8 shipped top-level surfaces, each a plain
/// `<a href>` via its URL CONST — including the minted `SCRAPE_URL = "/scrape"`.
/// A dropped surface (or a const swapped for a drifting literal) is killed here.
#[test]
fn landing_hub_links_all_eight_surfaces_via_url_consts() {
    let html = render_landing(&seeded_summary());
    for url in [
        MY_CLAIMS_URL,
        PEER_CLAIMS_URL,
        PROJECT_URL,
        PHILOSOPHY_URL,
        SCORE_URL,
        SEARCH_URL,
        SCRAPE_URL,
        PEERS_URL,
    ] {
        assert!(
            html.contains(&format!("href=\"{url}\"")),
            "the landing hub must link {url:?} as a plain <a href> (discoverability \
                 C-3 / no drift R-LD-4); got:\n{html}"
        );
    }
    // The newly-minted /scrape const must hold its canonical value.
    assert_eq!(
        SCRAPE_URL, "/scrape",
        "SCRAPE_URL is the canonical /scrape route"
    );
}

/// Behavior (slice-17 / US-LD-001 Theme 3 / C-1 CARDINAL): the front door
/// renders NO write/compose/sign/subscribe/follow control — every navigation
/// affordance is a plain `<a href>` link. A mutation wrapping a hub link in a
/// `<form>`/`<button>` is killed here.
#[test]
fn landing_renders_no_write_control() {
    let html = render_landing(&seeded_summary()).to_ascii_lowercase();
    for banned in [
        "<form",
        "<button",
        "hx-post",
        "hx-put",
        "hx-delete",
        ">compose<",
        ">sign<",
        ">subscribe<",
        ">follow<",
    ] {
        assert!(
            !html.contains(banned),
            "the front door must render NO mutating control (C-1 CARDINAL); found \
                 {banned:?} in:\n{html}"
        );
    }
}

// PROPERTY (slice-21 / US-NAV-001 / ADR-058 D2): `render_viewer_nav(active)` is a
// TOTAL function over the `active` key. Across the whole 8-surface × active-key space
// it holds four invariants: (1) a MEMBER `active` key marks EXACTLY ONE item
// `aria-current="page"` and (2) a NON-member key marks NONE; (3) every surface renders
// as a plain `<a href>` (no-JS navigable — never an `hx-get`-only affordance); and (4)
// the item set is EXACTLY `LANDING_HUB_SURFACES` (single source, AC-001.3 — no second
// list). Also pins the `<nav id="viewer-nav">` container + inner `<ul
// id="viewer-nav-items">` ids the persistent-nav swaps key on (ADR-058 D2/D5). One
// property replaces the 8 member + N non-member example variations.
proptest! {
    /// (1)+(3)+(4): for ANY member surface as the `active` key, the nav marks
    /// EXACTLY that item active (exactly one `aria-current="page"`), lists ALL 8
    /// `LANDING_HUB_SURFACES` surfaces as plain `<a href>` links, and carries the
    /// container + items ids.
    #[test]
    fn render_viewer_nav_marks_exactly_the_active_member_and_lists_all_surfaces(
        idx in 0usize..crate::common::LANDING_HUB_SURFACES.len(),
    ) {
        let (_, active_url) = crate::common::LANDING_HUB_SURFACES[idx];
        let html = render_viewer_nav(active_url).into_string();

        // Exactly ONE active marker (the current surface, AC-001.2), no other.
        prop_assert_eq!(
            html.matches(r#"aria-current="page""#).count(),
            1,
            "a member active key must mark EXACTLY ONE item active; got:\n{}",
            html
        );
        // Every surface a plain <a href="url"> (no-JS navigable, AC-001.4) with its label.
        for (label, url) in crate::common::LANDING_HUB_SURFACES {
            prop_assert!(
                html.contains(&format!(r#"href="{url}""#)),
                "the nav must link {url:?} as a plain <a href>; got:\n{html}"
            );
            prop_assert!(
                html.contains(label),
                "the nav must render the {label:?} label; got:\n{html}"
            );
        }
        // The container + inner item-list ids (ADR-058 D2/D5).
        prop_assert!(html.contains(r#"id="viewer-nav""#), "nav container id; got:\n{html}");
        prop_assert!(
            html.contains(r#"id="viewer-nav-items""#),
            "inner items-list id; got:\n{html}"
        );
        // The item set is EXACTLY the SSOT — no extra <a href> beyond the 8 surfaces
        // (single source, AC-001.3): the count of anchor opens equals the surface count.
        prop_assert_eq!(
            html.matches("<a ").count(),
            crate::common::LANDING_HUB_SURFACES.len(),
            "the nav item set must be EXACTLY LANDING_HUB_SURFACES (no second list); got:\n{}",
            html
        );
    }

    /// (2): for ANY NON-member `active` key (including the landing / 404 `""`), the nav
    /// marks NOTHING active — yet still lists all 8 surfaces as plain links.
    #[test]
    fn render_viewer_nav_marks_nothing_for_a_non_member_active_key(
        active in "[a-z/?=-]{0,14}",
    ) {
        prop_assume!(
            !crate::common::LANDING_HUB_SURFACES
                .iter()
                .any(|(_, url)| *url == active)
        );
        let html = render_viewer_nav(&active).into_string();
        prop_assert_eq!(
            html.matches(r#"aria-current="page""#).count(),
            0,
            "a non-member active key must mark NOTHING active; active={:?} got:\n{}",
            active,
            html
        );
        for (_, url) in crate::common::LANDING_HUB_SURFACES {
            prop_assert!(
                html.contains(&format!(r#"href="{url}""#)),
                "the nav must still link {url:?}; got:\n{html}"
            );
        }
    }
}

// PROPERTY (slice-17 / US-LD-001 Theme 1+4 / ADR-054 D1+D2): `render_landing`
// is a TOTAL function of the `LandingSummary` over ALL 2³ `Option` combinations
// — it never panics, ALWAYS produces a full HTML page (DOCTYPE + chrome), keeps
// the read-only notice, and renders each count per the `0 ≠ missing` rule:
// `Some(n)` → the number `n` (with its surface label), `None` → the
// `MISSING_COUNT_MARKER` "—" (NEVER a fabricated 0). Each count is INDEPENDENT.
// This carries the in-crate mutation gate for the per-count render branch.
proptest! {
    #[test]
    fn render_landing_is_total_and_renders_each_count_independently(
        own in proptest::option::of(0usize..10_000),
        peer in proptest::option::of(0usize..10_000),
        active in proptest::option::of(0usize..10_000),
    ) {
        let summary = LandingSummary {
            own_claims: own,
            peer_claims: peer,
            active_peers: active,
            // slice-18/19: the 4th + 5th fields are additive — this slice-17 property
            // pins the THREE counts; a fixed Some(0) here keeps it a valid
            // total-function input (the countered renders are gated by the slice-18 +
            // slice-19 properties below).
            countered_own_claims: Some(0),
            countered_peer_claims: Some(0),
        };
        let html = render_landing(&summary);

        // Always a complete full page (ADR-054 D5 full-page-only) with the
        // read-only notice and the full 8-surface hub.
        prop_assert!(html.contains("<!DOCTYPE html>"), "must be a full page; got:\n{html}");
        prop_assert!(html.contains(READ_ONLY_NOTICE), "must keep the read-only notice; got:\n{html}");
        prop_assert!(html.contains(&format!("href=\"{SCRAPE_URL}\"")), "must link /scrape; got:\n{html}");

        // Each count renders per the 0 ≠ missing rule, attributed to its surface
        // label. None → the marker; Some(n) → the number n (incl. Some(0) → "0").
        for (count, label) in [
            (own, "own claims"),
            (peer, "peer claims"),
            (active, "active peers"),
        ] {
            prop_assert!(html.contains(label), "must label {label:?}; got:\n{html}");
            match count {
                Some(n) => prop_assert!(
                    html.contains(&format!("{n} {label}")),
                    "Some({n}) must render the number {n} for {label:?}; got:\n{html}"
                ),
                None => prop_assert!(
                    html.contains(&format!("{MISSING_COUNT_MARKER} {label}")),
                    "None must render the missing marker for {label:?} (NOT a 0); got:\n{html}"
                ),
            }
        }
    }
}

// ====================================================================
// slice-18 (US-CC-001 / ADR-055 D3): the shared `render_countered` helper
// + the 4th `LandingSummary` field rendered BESIDE the unchanged own-claims
// line. These carry the in-crate mutation gate for the countered-count
// render core.
// ====================================================================

/// Behavior (ADR-055 D3 — the three render branches + neutral copy): the shared
/// `render_countered` helper maps `Some(n) → "(n countered)"`, `Some(0) → "(0
/// countered)"` (an HONEST zero, a SUCCESSFUL read), and `None → "(— countered)"`
/// (the [`MISSING_COUNT_MARKER`] inside the parenthetical — a FAILED read, NEVER a
/// fabricated 0). The copy is NEUTRAL disputed-claim awareness — never a
/// verdict/penalty/"disputed by N" total (C-6 / WD-CC-10). These four pinned cases
/// are the mutation targets: a mutant that renders `Some(0)` as the marker, blanks
/// the number, drops the "countered" word, or emits verdict copy is killed.
#[test]
fn render_countered_renders_number_zero_and_missing_marker_with_neutral_copy() {
    assert_eq!(
        render_countered(Some(3)),
        "(3 countered)",
        "Some(n) must render \"(n countered)\" (ADR-055 D3)"
    );
    assert_eq!(
        render_countered(Some(0)),
        "(0 countered)",
        "Some(0) is an HONEST zero — a SUCCESSFUL read, DISTINCT from the missing \
             marker (C-5 / WD-CC-6)"
    );
    assert_eq!(
        render_countered(None),
        format!("({MISSING_COUNT_MARKER} countered)"),
        "None renders the missing marker inside the parenthetical — a FAILED read, \
             NEVER a fabricated 0 (ADR-055 D3 / C-5)"
    );
    // The copy is NEUTRAL — a countered claim is contested, not wrong; the count is
    // awareness, never a verdict/penalty/"by N" total (C-6 / WD-CC-10).
    for countered in [Some(3usize), Some(0), None] {
        let rendered = render_countered(countered).to_ascii_lowercase();
        for banned in [
            "disputed by",
            "refuted",
            "false",
            "penalty",
            "deduction",
            "deducted",
            "invalid",
            "wrong",
            "discredited",
        ] {
            assert!(
                !rendered.contains(banned),
                "render_countered must be NEUTRAL — found {banned:?} in {rendered:?} \
                     (C-6 / WD-CC-10)"
            );
        }
    }
}

// PROPERTY (slice-18 / US-CC-001 Theme 1+2 / ADR-055 D2+D3): with the FOURTH
// additive `countered_own_claims` field, `render_landing` stays a TOTAL function of
// the now-2⁴ `Option` combinations — it never panics, ALWAYS produces a full HTML
// page, renders the countered count via `render_countered` BESIDE the UNCHANGED
// own-claims line ("12 own claims (3 countered)"), and the own-claims number is
// NEVER re-weighted/deducted by the countered count (additive — C-4). This carries
// the in-crate mutation gate for the additive render beside the own-claims line.
proptest! {
    #[test]
    fn render_landing_renders_the_countered_count_beside_the_unchanged_own_claims(
        own in proptest::option::of(0usize..10_000),
        peer in proptest::option::of(0usize..10_000),
        active in proptest::option::of(0usize..10_000),
        countered in proptest::option::of(0usize..10_000),
    ) {
        let summary = LandingSummary {
            own_claims: own,
            peer_claims: peer,
            active_peers: active,
            countered_own_claims: countered,
            // slice-19: the 5th field is additive — this slice-18 property pins the
            // OWN countered render; a fixed Some(0) here keeps it a valid
            // total-function input (the peer countered render is gated by the slice-19
            // property below).
            countered_peer_claims: Some(0),
        };
        let html = render_landing(&summary);

        // Total function: still a complete full page over EVERY 2⁴ combination.
        prop_assert!(html.contains("<!DOCTYPE html>"), "must be a full page; got:\n{html}");

        // The own-claims line renders UNCHANGED (additive — the countered count
        // never re-weights it, C-4): the EXACT "{own} own claims" still appears.
        if let Some(n) = own {
            prop_assert!(
                html.contains(&format!("{n} own claims")),
                "the own-claims count must render UNCHANGED (additive, C-4); got:\n{html}"
            );
        }

        // The countered count renders via the SAME helper output, BESIDE the
        // own-claims line — Some(n) → "(n countered)", None → the missing marker.
        prop_assert!(
            html.contains(&render_countered(countered)),
            "the countered count {countered:?} must render via render_countered \
             beside the own-claims line (ADR-055 D3); got:\n{html}"
        );
    }
}

// ====================================================================
// slice-19 (US-PC-001 / ADR-056 D2+D3): the FIFTH additive `countered_peer_claims`
// field rendered via the REUSED `render_countered` helper BESIDE the UNCHANGED
// peer-claims line ("4 peer claims (1 countered)"). The PEER sibling of the slice-18
// own-line property; carries the in-crate mutation gate for the additive peer-line
// render (the slice-18 own line byte-untouched).
// ====================================================================

// PROPERTY (slice-19 / US-PC-001 Theme 1+2 / ADR-056 D2+D3): with the FIFTH additive
// `countered_peer_claims` field, `render_landing` stays a TOTAL function of the
// now-2⁵ `Option` combinations — it never panics, ALWAYS produces a full HTML page,
// renders the countered-PEER count via the REUSED `render_countered` BESIDE the
// UNCHANGED peer-claims line ("4 peer claims (1 countered)"), the peer-claims number
// is NEVER re-weighted/deducted by the countered count (additive — C-4), AND the
// slice-18 own line renders UNCHANGED beside it (the own-countered parenthetical still
// appears — WD-PC-7, the peer count touches only the peer line).
proptest! {
    #[test]
    fn render_landing_renders_the_peer_countered_count_beside_the_unchanged_peer_claims(
        own in proptest::option::of(0usize..10_000),
        peer in proptest::option::of(0usize..10_000),
        active in proptest::option::of(0usize..10_000),
        countered_own in proptest::option::of(0usize..10_000),
        countered_peer in proptest::option::of(0usize..10_000),
    ) {
        let summary = LandingSummary {
            own_claims: own,
            peer_claims: peer,
            active_peers: active,
            countered_own_claims: countered_own,
            countered_peer_claims: countered_peer,
        };
        let html = render_landing(&summary);

        // Total function: still a complete full page over EVERY 2⁵ combination.
        prop_assert!(html.contains("<!DOCTYPE html>"), "must be a full page; got:\n{html}");

        // The peer-claims line renders UNCHANGED (additive — the countered-peer count
        // never re-weights it, C-4): the EXACT "{peer} peer claims" still appears.
        if let Some(n) = peer {
            prop_assert!(
                html.contains(&format!("{n} peer claims")),
                "the peer-claims count must render UNCHANGED (additive, C-4); got:\n{html}"
            );
        }

        // The countered-PEER count renders via the REUSED helper output, BESIDE the
        // peer-claims line — Some(n) → "(n countered)", None → the missing marker.
        // We pin the "peer claims (… countered)" adjacency so a mutant that drops the
        // peer parenthetical (or renders it on the wrong line) is killed.
        prop_assert!(
            html.contains(&format!(
                "{} peer claims {}",
                render_count(peer),
                render_countered(countered_peer)
            )),
            "the countered-peer count {countered_peer:?} must render via render_countered \
             BESIDE the peer-claims line (ADR-056 D3); got:\n{html}"
        );

        // The slice-18 OWN line renders UNCHANGED beside it (WD-PC-7): the
        // own-countered parenthetical still appears via the SAME helper — the peer
        // count touches ONLY the peer line.
        prop_assert!(
            html.contains(&format!(
                "{} own claims {}",
                render_count(own),
                render_countered(countered_own)
            )),
            "the slice-18 own line must render UNTOUCHED beside the peer line \
             (WD-PC-7); got:\n{html}"
        );
    }
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
    let html = render_peer_claims_page(&page, None);
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
    let html = render_peer_claims_page(&page, None);
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

/// Behavior (slice-19 / US-PC-002 / ADR-056 D3 — single source, prime mutation
/// target): `render_peer_claims_page` renders the countered-PEER count in the list
/// HEADER through the SAME `render_countered` helper the landing uses, so the two
/// surfaces cannot diverge. `Some(n)` → "(n countered)" beside "Peer Claims",
/// `Some(0)` → "(0 countered)" (honest zero), `None` → "(— countered)" (missing
/// marker, NEVER a fabricated 0). Pins that the header render is exactly the
/// `render_countered` output — a mutant that drops the header count, fabricates 0
/// for a missing read, or diverges the copy from the helper is killed.
#[test]
fn render_peer_claims_page_header_shows_the_countered_count_via_render_countered() {
    let one_peer_claim = || {
        PageView::new(vec![peer_row(
            "bafypeer",
            "github:peer/axum",
            "embodiesPhilosophy",
            "org.openlore.philosophy.ergonomics",
            0.70,
            known_origin("did:plc:peer-axum"),
        )])
    };
    // Some(n) → the header carries EXACTLY the render_countered(Some(n)) output,
    // beside the "Peer Claims" heading (single source — the helper, not a literal).
    let html_some = render_peer_claims_page(&one_peer_claim(), Some(1));
    assert!(
        html_some.contains(&render_countered(Some(1))),
        "the /peer-claims header must render the countered count via render_countered \
             (Some(1) → \"(1 countered)\", single source — ADR-056 D3); got:\n{html_some}"
    );
    // Some(0) → the honest zero "(0 countered)" (a successful read of zero),
    // DISTINCT from the missing marker.
    let html_zero = render_peer_claims_page(&one_peer_claim(), Some(0));
    assert!(
        html_zero.contains("(0 countered)") && !html_zero.contains("(— countered)"),
        "Some(0) must render the honest zero \"(0 countered)\", NOT the missing \
             marker (0 ≠ missing, C-5); got:\n{html_zero}"
    );
    // None → the missing marker "(— countered)", NEVER a fabricated "(0 countered)"
    // (a failed read degrades to None → the marker, ADR-056 D4 / C-5).
    let html_missing = render_peer_claims_page(&one_peer_claim(), None);
    assert!(
        html_missing.contains(&render_countered(None)) && !html_missing.contains("(0 countered)"),
        "None must render the missing marker \"(— countered)\" via render_countered, \
             NEVER a fabricated \"(0 countered)\" (C-5); got:\n{html_missing}"
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
    let html = render_peer_claims_page(&page, None);
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
    let html = render_peer_claims_page(&page, None);
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
        let html = render_peer_claims_page(&page, None);
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
    let page = render_claims_page(&view, None);

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
        let full = render_claims_page(&view, None);
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
        verified_against: ports::claim_domain::KeyId(format!("{author}#org.openlore.application")),
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
        (
            priya,
            "bafyone",
            "org.openlore.philosophy.reproducible-builds",
            0.82,
        ),
        (
            priya,
            "bafytwo",
            "org.openlore.philosophy.hermetic-builds",
            0.79,
        ),
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
        html.to_ascii_lowercase()
            .contains("not a community consensus"),
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
    for banned in [
        "name=\"follow\"",
        "subscribe",
        ">follow<",
        "<button",
        "<form",
        "hx-post",
    ] {
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
    for dimension_field in [
        "name=\"object\"",
        "name=\"contributor\"",
        "name=\"subject\"",
    ] {
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
// Follow-state render arm (slice-16; US-SF-002 / ADR-053 D3) — the NEW
// `SubscribedPeer` arm of `render_search_result_row`. These in-crate unit
// tests carry the mutation gate for the pure-core render surface this slice
// adds (the "Following" indicator vs the `peer add` guidance, per resolved
// relationship). Port-to-port at the render-fn scope: input is the resolved
// `SearchState::Results`; output is the rendered HTML.
// -------------------------------------------------------------------------

/// Build a one-author `SearchState::Results` whose single row carries the given
/// resolved `relationship` — the discriminating input the follow-state render arm
/// branches on (everything else held byte-equal so the arm is the ONLY variable).
fn search_state_one_row(author: &str, relationship: AuthorRelationship) -> SearchState {
    let mut row = search_row(
        author,
        "bafyrelrow",
        "org.openlore.philosophy.reproducible-builds",
        0.88,
    );
    row.relationship = relationship;
    SearchState::Results {
        result: NetworkSearchResult {
            by_author: vec![(ports::claim_domain::Did(author.to_string()), vec![row])],
            distinct_author_count: 1,
            total_claims: 1,
            suggestion: None,
        },
        dimension: appview_domain::SearchDimension::Object,
    }
}

/// Behavior (US-SF-002 / Theme A / R-SF-3 — the load-bearing accuracy fix): a row
/// resolved to `SubscribedPeer` renders the neutral render-only "Following"
/// indicator and NO `openlore peer add` command — an already-followed author is NOT
/// re-offered a follow. Pins the NEW arm (mutation: the arm must emit
/// `SEARCH_FOLLOWING_INDICATOR`, never the guidance prefix).
#[test]
fn search_subscribed_peer_row_shows_following_and_no_peer_add() {
    let html = render_search_results_fragment(&search_state_one_row(
        "did:plc:rachel-test#org.openlore.application",
        AuthorRelationship::SubscribedPeer,
    ))
    .into_string();

    assert!(
        html.contains(SEARCH_FOLLOWING_INDICATOR),
        "a SubscribedPeer row must show the neutral {SEARCH_FOLLOWING_INDICATOR:?} \
             indicator (the NEW arm); got:\n{html}"
    );
    assert!(
        !html.contains("openlore peer add"),
        "a SubscribedPeer row must NOT re-offer a follow — NO `openlore peer add` \
             command (R-SF-3); got:\n{html}"
    );
}

/// Behavior (US-SF-002 / Theme A / R-SF-4 — no over-correction): a row resolved to
/// `NetworkUnfollowed` keeps the slice-08 render-only `openlore peer add <bare-did>`
/// guidance and shows NO "Following" indicator — UNCHANGED from slice-08. Pins the
/// complementary arm (mutation: the NetworkUnfollowed arm must emit the guidance,
/// never the "Following" indicator).
#[test]
fn search_network_unfollowed_row_keeps_peer_add_and_no_following() {
    let html = render_search_results_fragment(&search_state_one_row(
        "did:plc:priya-test#org.openlore.application",
        AuthorRelationship::NetworkUnfollowed,
    ))
    .into_string();

    // The bare-DID `openlore peer add did:plc:priya-test` guidance is present.
    assert!(
        html.contains("openlore peer add did:plc:priya-test"),
        "a NetworkUnfollowed row must KEEP the render-only `openlore peer add \
             <bare-did>` guidance (R-SF-4, slice-08 unchanged); got:\n{html}"
    );
    assert!(
        !html.contains(SEARCH_FOLLOWING_INDICATOR),
        "a NetworkUnfollowed row must NOT show the {SEARCH_FOLLOWING_INDICATOR:?} \
             indicator (binary resolution, C-6); got:\n{html}"
    );
}

/// Behavior (slice-20 / US-FS-002 / ADR-057 D3 — the NEW `You` arm): a row resolved
/// to `You` renders the neutral render-only SELF indicator
/// ([`SEARCH_SELF_INDICATOR`]) and NO `openlore peer add` command — the operator's
/// OWN claim is never re-offered a follow (you cannot follow yourself). Pins the
/// `You` arm of `render_search_result_row` / `render_self_indicator` at the in-crate
/// unit level (mutation gate: the arm must emit the self indicator, never an empty
/// `Markup::default()` and never a peer-add affordance).
#[test]
fn search_you_row_shows_self_indicator_and_no_peer_add() {
    let html = render_search_results_fragment(&search_state_one_row(
        "did:plc:me-test#org.openlore.application",
        AuthorRelationship::You,
    ))
    .into_string();

    assert!(
        html.contains(SEARCH_SELF_INDICATOR),
        "a `You` row must show the neutral {SEARCH_SELF_INDICATOR:?} self indicator \
             (the NEW arm); got:\n{html}"
    );
    assert!(
        !html.contains("openlore peer add"),
        "a `You` row must NOT re-offer a follow — NO `openlore peer add` command \
             (you cannot follow yourself); got:\n{html}"
    );
    assert!(
        !html.contains(SEARCH_FOLLOWING_INDICATOR),
        "a `You` row must NOT show the {SEARCH_FOLLOWING_INDICATOR:?} indicator \
             (the `You` arm is distinct from SubscribedPeer); got:\n{html}"
    );
}

/// Behavior (slice-20 / US-FS-002 / ADR-057 D3 — the NEW `UnsubscribedCache` arm): a
/// row resolved to `UnsubscribedCache` renders the neutral render-only RESIDUE
/// indicator ([`SEARCH_REMOVED_CACHED_INDICATOR`]) and NO `openlore peer add` command
/// — a soft-removed-but-cached peer is residue, NOT a fresh network find, so the
/// follow affordance is suppressed (like SubscribedPeer). Pins the
/// `UnsubscribedCache` arm of `render_search_result_row` /
/// `render_cached_unsubscribed_indicator` at the in-crate unit level (mutation gate:
/// the arm must emit the residue indicator, never an empty `Markup::default()` and
/// never a peer-add affordance).
#[test]
fn search_unsubscribed_cache_row_shows_residue_indicator_and_no_peer_add() {
    let html = render_search_results_fragment(&search_state_one_row(
        "did:plc:tobias-test#org.openlore.application",
        AuthorRelationship::UnsubscribedCache,
    ))
    .into_string();

    assert!(
        html.contains(SEARCH_REMOVED_CACHED_INDICATOR),
        "an `UnsubscribedCache` row must show the neutral \
             {SEARCH_REMOVED_CACHED_INDICATOR:?} residue indicator (the NEW arm); \
             got:\n{html}"
    );
    assert!(
        !html.contains("openlore peer add"),
        "an `UnsubscribedCache` row must NOT re-offer a follow — a soft-removed \
             cached peer is residue, NO `openlore peer add` command; got:\n{html}"
    );
    assert!(
        !html.contains(SEARCH_FOLLOWING_INDICATOR),
        "an `UnsubscribedCache` row must NOT show the {SEARCH_FOLLOWING_INDICATOR:?} \
             indicator (the residue arm is distinct from SubscribedPeer); got:\n{html}"
    );
}

/// Behavior (US-SF-002 / Theme B / C-1, CARDINAL — render-only): NEITHER follow-state
/// affordance is an executable control. Over a render carrying BOTH a SubscribedPeer
/// row ("Following") AND a NetworkUnfollowed row (`peer add`), the markup contains no
/// `<button>`/`<form>`/mutating `<a>`/`hx-*` follow control and no bare `>Following<`
/// control element. Pins the render-only-ness of the NEW arm at the unit level (the
/// mutation gate for the CARDINAL no-control contract).
#[test]
fn search_follow_state_affordances_are_render_only_text() {
    // A two-author MIX: Rachel (SubscribedPeer) + Priya (NetworkUnfollowed).
    let mut rachel = search_row(
        "did:plc:rachel-test#org.openlore.application",
        "bafyrachel",
        "org.openlore.philosophy.reproducible-builds",
        0.88,
    );
    rachel.relationship = AuthorRelationship::SubscribedPeer;
    let priya = search_row(
        "did:plc:priya-test#org.openlore.application",
        "bafypriya",
        "org.openlore.philosophy.reproducible-builds",
        0.82,
    );
    let state = SearchState::Results {
        result: NetworkSearchResult {
            by_author: vec![
                (
                    ports::claim_domain::Did(
                        "did:plc:rachel-test#org.openlore.application".to_string(),
                    ),
                    vec![rachel],
                ),
                (
                    ports::claim_domain::Did(
                        "did:plc:priya-test#org.openlore.application".to_string(),
                    ),
                    vec![priya],
                ),
            ],
            distinct_author_count: 2,
            total_claims: 2,
            suggestion: None,
        },
        dimension: appview_domain::SearchDimension::Object,
    };

    let html = render_search_results_fragment(&state).into_string();
    let lowered = html.to_ascii_lowercase();

    // Both affordances ARE present (the mix is genuine).
    assert!(
        html.contains(SEARCH_FOLLOWING_INDICATOR)
            && html.contains("openlore peer add did:plc:priya-test"),
        "the mix render must carry BOTH the Following indicator AND the peer-add \
             guidance; got:\n{html}"
    );
    // …and NEITHER is an executable control (C-1, CARDINAL).
    for banned in [
        "name=\"follow\"",
        "name=\"unfollow\"",
        "name=\"subscribe\"",
        ">follow<",
        ">unfollow<",
        ">subscribe<",
        ">following<",
        "hx-post",
        "hx-delete",
        "hx-put",
    ] {
        assert!(
            !lowered.contains(banned),
            "C-1 (CARDINAL): the follow-state affordances must be render-only TEXT — \
                 found executable-control marker {banned:?} in:\n{html}"
        );
    }
}

// -------------------------------------------------------------------------
// Four-arm follow-state precedence resolver (slice-20; US-FS-001/002 /
// ADR-057 D2) — the PURE `resolve_author_relationship` SSOT. Property-based:
// the precedence chain `You > SubscribedPeer > UnsubscribedCache >
// NetworkUnfollowed` must hold over EVERY combination of the three LOCAL set
// memberships, and the `#org.openlore.application` signing fragment must be
// stripped before membership (R-FS-6). Port-to-port at the pure-fn scope: the
// resolver's public signature IS the driving port; the returned arm is the
// observable. ONE property covers the whole 2³ membership lattice × fragment
// presence — the model is a precedence lookup, NOT a state machine, so a single
// exhaustive property is exact (Hebert ch.3 "Modeling": a reference oracle that
// applies the precedence in order).
// -------------------------------------------------------------------------

/// The reference oracle for the four-arm precedence — independently re-states the
/// spec (`You > SubscribedPeer > UnsubscribedCache > NetworkUnfollowed`) over BARE
/// membership so the property compares the SUT against an obviously-correct model
/// (Hebert ch.3 "Modeling"), never against itself.
fn expected_relationship(
    bare: &str,
    is_own: bool,
    is_active: bool,
    is_cached: bool,
) -> AuthorRelationship {
    let _ = bare;
    if is_own {
        AuthorRelationship::You
    } else if is_active {
        AuthorRelationship::SubscribedPeer
    } else if is_cached {
        AuthorRelationship::UnsubscribedCache
    } else {
        AuthorRelationship::NetworkUnfollowed
    }
}

proptest! {
    /// Property (US-FS-001/002 / C-6 / ADR-057 D2): for an arbitrary bare author DID,
    /// any chosen membership in each of the three LOCAL presence sets, and the author
    /// DID rendered with OR without the `#org.openlore.application` signing fragment,
    /// `resolve_author_relationship` returns the precedence-correct arm — `You` when in
    /// `own` (regardless of the other two), else `SubscribedPeer` when in `active`, else
    /// `UnsubscribedCache` when in `cached`, else `NetworkUnfollowed`. The membership
    /// always tests the BARE DID, so the fragmented and bare forms resolve IDENTICALLY
    /// (R-FS-6 fragment-strip). This single property exhausts the 2³ membership lattice ×
    /// {bare, fragmented} for arbitrary DIDs — the whole resolver contract.
    #[test]
    fn resolve_author_relationship_obeys_precedence_and_strips_the_fragment(
        stem in "[a-z0-9]{1,12}",
        is_own in any::<bool>(),
        is_active in any::<bool>(),
        is_cached in any::<bool>(),
        fragmented in any::<bool>(),
    ) {
        use std::collections::HashSet;
        let bare = format!("did:plc:{stem}-test");
        // The sets store the BARE DID (the LOCAL reads project bare author_did/peer_did).
        let set_with = |member: bool| -> HashSet<String> {
            let mut s = HashSet::new();
            if member {
                s.insert(bare.clone());
            }
            s
        };
        let own = set_with(is_own);
        let active = set_with(is_active);
        let cached = set_with(is_cached);
        // The RESULT author_did may carry the signing fragment — the membership test
        // must strip it before lookup (R-FS-6).
        let author_did = if fragmented {
            format!("{bare}#org.openlore.application")
        } else {
            bare.clone()
        };

        let resolved = resolve_author_relationship(&author_did, &own, &active, &cached);

        let expected = expected_relationship(&bare, is_own, is_active, is_cached);
        prop_assert_eq!(
            resolved,
            expected,
            "resolve_author_relationship must obey You > SubscribedPeer > \
             UnsubscribedCache > NetworkUnfollowed with the fragment stripped before \
             membership: author_did={:?} own={} active={} cached={}",
            author_did, is_own, is_active, is_cached
        );
    }
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
fn attributed(
    author: &str,
    cid: &str,
    subject: &str,
    object: &str,
    confidence: f64,
) -> AttributedClaim {
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
        attributed(
            "did:plc:priya-test",
            "bafyone",
            "github:bazelbuild/bazel",
            repro,
            0.86,
        ),
        attributed(
            "did:plc:priya-test",
            "bafytwo",
            "github:NixOS/nixpkgs",
            repro,
            0.90,
        ),
        attributed(
            "did:plc:priya-test",
            "bafythree",
            "github:GNOME/meson",
            repro,
            0.74,
        ),
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
    let html =
        render_score_results_fragment(&rich_scored_state(), &std::collections::HashSet::new())
            .into_string();

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
        assert!(
            html.contains(cid),
            "the breakdown must name the claim cid {cid:?}; got:\n{html}"
        );
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
        attributed(
            "did:plc:priya-test",
            "bafyone",
            "github:bazelbuild/bazel",
            repro,
            0.86,
        ),
        attributed(
            "did:plc:rachel-test",
            "bafytwo",
            "github:bazelbuild/bazel",
            repro,
            0.90,
        ),
    ];
    let view = score(&feed, &ScoringConfig::DEFAULT);
    assert_eq!(
        view.ranked.len(),
        1,
        "fixture must produce exactly one pairing"
    );
    let pairing = &view.ranked[0];

    let html = render_score_results_fragment(
        &ScoreState::Scored { view: view.clone() },
        &std::collections::HashSet::new(),
    )
    .into_string();

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
            &[attributed(
                "did:plc:bjorn-test",
                "bafysparse",
                "github:torvalds/linux",
                repro,
                0.95,
            )],
            &ScoringConfig::DEFAULT,
        ),
    };
    // CONFLICTING: two distinct authors on the SAME (subject, object) → ONE
    // pairing, TWO attributed rows.
    let conflicting = ScoreState::Scored {
        view: score(
            &[
                attributed(
                    "did:plc:test-jeff",
                    "bafyown",
                    "github:denoland/deno",
                    repro,
                    0.40,
                ),
                attributed(
                    "did:plc:test-jeff-collaborator",
                    "bafypeer",
                    "github:denoland/deno",
                    repro,
                    0.55,
                ),
            ],
            &ScoringConfig::DEFAULT,
        ),
    };

    for (posture, state) in [
        ("rich", &rich),
        ("sparse", &sparse),
        ("conflicting", &conflicting),
    ] {
        let empty_presence = std::collections::HashSet::new();
        let fragment = render_score_results_fragment(state, &empty_presence).into_string();
        let page = render_score_page(state, &empty_presence);
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

    let empty_presence = std::collections::HashSet::new();
    let fragment = render_score_results_fragment(&state, &empty_presence).into_string();
    let page = render_score_page(&state, &empty_presence);

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
        attributed(
            "did:plc:test-jeff-collaborator",
            "bafypeer",
            subject,
            repro,
            0.55,
        ),
    ];
    let view = score(&feed, &ScoringConfig::DEFAULT);
    // ONE pairing (same subject+object) decomposing into TWO contributions.
    assert_eq!(
        view.ranked.len(),
        1,
        "two same-(subject,object) claims must form ONE pairing"
    );
    assert_eq!(
        view.ranked[0].contributions().len(),
        2,
        "the one pairing must decompose into TWO contributions (one per author), never merged"
    );

    let html = render_score_results_fragment(
        &ScoreState::Scored { view },
        &std::collections::HashSet::new(),
    )
    .into_string();

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
    for banned in [
        "authors agree",
        "community consensus",
        "consensus score",
        "the network says",
    ] {
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
    let feed = vec![attributed(
        "did:plc:bjorn-test",
        "bafysparse",
        "github:torvalds/linux",
        repro,
        0.95,
    )];
    let view = score(&feed, &ScoringConfig::DEFAULT);
    let html = render_score_results_fragment(
        &ScoreState::Scored { view },
        &std::collections::HashSet::new(),
    )
    .into_string();

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
    let html = render_score_results_fragment(
        &ScoreState::NoClaims {
            contributor: "did:plc:nobody-local".to_string(),
        },
        &std::collections::HashSet::new(),
    )
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
    let empty_presence = std::collections::HashSet::new();
    let fragment = render_score_results_fragment(&state, &empty_presence).into_string();
    let page = render_score_page(&state, &empty_presence);

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
// Contributor-Score COUNTER-PRESENCE FLAG (slice-14; US-CF-002 / ADR-051) —
// the threaded `&presence` render chain over the REUSED slice-13
// `render_countered_link` + the SCORE_COUNTER_LEGEND SSOT. These pin the NEW
// pure-core behavior for the mutation gate (cargo-mutants -p viewer-domain):
// the legend renders ONCE in the Scored arm + is blocklist-clean; the marker
// is emitted IFF the presence set contains the contribution CID; an empty
// presence set renders byte-identically to slice-09; and the presence bool
// NEVER reaches a subtotal/weight (the sum-to-weight orthogonality). The render
// fn IS its own driving port — calling it directly is port-to-port at domain
// scope (nw-tdd-methodology Hexagonal Domain Layer).
// -------------------------------------------------------------------------

/// The render-only marker the REUSED `render_countered_link` emits for a
/// countered contribution CID (maud emits no whitespace inside the element).
fn score_marker(cid: &str) -> String {
    format!("<a href=\"/claims/{cid}\">{COUNTERED_PRESENCE_FLAG}</a>")
}

/// Build a presence set from string CIDs (the threaded `&presence` argument).
fn presence_of(cids: &[&str]) -> std::collections::HashSet<String> {
    cids.iter().map(|c| (*c).to_string()).collect()
}

/// Behavior / CARDINAL anti-misread (US-CF-002 / ADR-051 §6.3 / AC-SCORE-
/// ANTIMISREAD): a Scored breakdown renders the `SCORE_COUNTER_LEGEND` EXACTLY
/// ONCE (one legend per scored breakdown, never per row/pairing), in BOTH the
/// fragment and the full page, and the WHOLE rendered body is blocklist-clean —
/// it contains NONE of the verdict/penalty/subtraction words. The `Form` and
/// `NoClaims` arms render NO legend (it governs markers that do not appear).
/// Pins the legend SSOT render site + the blocklist-clean copy for the mutation
/// gate (a mutant deleting the legend, rendering it twice, or emitting a verdict
/// word is killed).
#[test]
fn score_legend_renders_once_in_scored_arm_blocklist_clean_and_absent_otherwise() {
    let scored = rich_scored_state();
    let empty = std::collections::HashSet::new();
    let blocklist = [
        "disputed",
        "refuted",
        "false",
        "penalty",
        "deduction",
        "lowered",
        "disputed score",
    ];

    // The Scored arm carries the legend EXACTLY ONCE in BOTH shapes ...
    for (shape, html) in [
        (
            "fragment",
            render_score_results_fragment(&scored, &empty).into_string(),
        ),
        ("page", render_score_page(&scored, &empty)),
    ] {
        assert_eq!(
            html.matches(SCORE_COUNTER_LEGEND).count(),
            1,
            "the Scored {shape} must render SCORE_COUNTER_LEGEND EXACTLY once (one \
                 legend per scored breakdown, never per row/pairing); got:\n{html}"
        );
        // ... and the WHOLE body is blocklist-clean (lowercased compare).
        let lowered = html.to_ascii_lowercase();
        for banned in blocklist {
            assert!(
                !lowered.contains(banned),
                "the Scored {shape} body must be blocklist-clean — never the \
                     verdict/penalty word {banned:?} (AC-SCORE-ANTIMISREAD); got:\n{html}"
            );
        }
    }

    // The Form + NoClaims arms render NO legend (it governs markers that never
    // appear; ADR-051 §6.3 / DD-14-3 placement: Scored arm only).
    for state in [
        ScoreState::Form,
        ScoreState::NoClaims {
            contributor: "did:plc:nobody".to_string(),
        },
    ] {
        let html = render_score_results_fragment(&state, &empty).into_string();
        assert!(
            !html.contains(SCORE_COUNTER_LEGEND),
            "the {state:?} arm must render NO SCORE_COUNTER_LEGEND (Scored arm only); \
                 got:\n{html}"
        );
    }
}

/// Behavior / CARDINAL flag-gating (US-CF-002 / AC-002-MARKER / AC-002-NO-NOISE):
/// `render_score_breakdown` (via the threaded chain) emits the REUSED
/// `render_countered_link` one-hop marker for a contribution row IFF the threaded
/// presence set CONTAINS that contribution's CID — and emits NOTHING for an
/// un-countered row. Property over the rich state's three CIDs: for an arbitrary
/// SUBSET chosen as the presence set, every CID in the set is flagged, every CID
/// NOT in the set is not. Pins the `presence.contains(&cid)` branch (a mutant
/// inverting the condition, dropping the marker, or flagging an un-countered row
/// is killed) AND the no-noise discipline.
#[test]
fn score_breakdown_emits_the_marker_iff_presence_contains_the_cid() {
    let all = ["bafyone", "bafytwo", "bafythree"];
    // Enumerate every subset of the three CIDs (2^3 = 8) as the presence set —
    // a property over the full membership lattice, exhaustively, at unit cost.
    for mask in 0u8..8 {
        let chosen: Vec<&str> = all
            .iter()
            .enumerate()
            .filter(|(i, _)| mask & (1 << i) != 0)
            .map(|(_, c)| *c)
            .collect();
        let presence = presence_of(&chosen);
        let html = render_score_results_fragment(&rich_scored_state(), &presence).into_string();

        for cid in all {
            let marker = score_marker(cid);
            if chosen.contains(&cid) {
                assert!(
                    html.contains(&marker),
                    "presence {chosen:?}: the countered contribution row for {cid:?} \
                         must carry the marker {marker:?}; got:\n{html}"
                );
            } else {
                assert!(
                    !html.contains(&marker),
                    "presence {chosen:?}: the un-countered contribution row for \
                         {cid:?} must carry NO marker {marker:?} (no-noise); got:\n{html}"
                );
            }
        }
        // No-noise discipline: no empty-state "0 counters" / "no disagreement"
        // text ever leaks regardless of the presence set.
        let lowered = html.to_ascii_lowercase();
        for noise in ["0 counters", "no disagreement", "no counters"] {
            assert!(
                !lowered.contains(noise),
                "presence {chosen:?}: the breakdown must carry no {noise:?} \
                     empty-state noise; got:\n{html}"
            );
        }
    }
}

/// Behavior / CARDINAL byte-identity + sum-to-weight orthogonality (US-CF-002 /
/// ADR-051 §7 / AC-SCORE-BYTEID + AC-SCORE-SUMWEIGHT): with the additive markers
/// AND the additive legend elided, the FLAGGED render is byte-identical to the
/// empty-presence (slice-09) render — the presence set is PURELY additive markup
/// and NEVER perturbs a weight/subtotal/confidence/bucket/rank/row-order. Pins
/// the shown-never-applied contract at the unit level: a mutant that lets the
/// presence bool reach a number, re-order a row, or that renders the marker
/// anywhere but beside the subtotal is killed (the elided bytes would diverge).
#[test]
fn flagged_score_render_is_byte_identical_to_slice09_with_markers_and_legend_elided() {
    let state = rich_scored_state();
    let empty = std::collections::HashSet::new();
    // The slice-09 baseline: the SAME state rendered with an EMPTY presence set
    // (no markers) and the legend stripped.
    let baseline = render_score_results_fragment(&state, &empty)
        .into_string()
        .replace(SCORE_COUNTER_LEGEND, "");

    // Flag ALL three contributions, then elide the additive markers AND the legend.
    let presence = presence_of(&["bafyone", "bafytwo", "bafythree"]);
    let mut flagged_elided = render_score_results_fragment(&state, &presence).into_string();
    for cid in ["bafyone", "bafytwo", "bafythree"] {
        flagged_elided = flagged_elided.replace(&score_marker(cid), "");
    }
    flagged_elided = flagged_elided.replace(SCORE_COUNTER_LEGEND, "");

    assert_eq!(
        flagged_elided, baseline,
        "with the additive markers AND the legend elided, the FLAGGED /score render \
             must be byte-identical to the empty-presence (slice-09) render — the \
             presence set is additive markup ONLY and must never perturb a number, a \
             rank, or a row order (AC-SCORE-BYTEID / shown-never-applied)"
    );
}

/// Behavior (US-CF-002 / AC-SCORE-SUMWEIGHT): the per-contribution subtotals the
/// FLAGGED render emits STILL sum to the headline weight it renders for the SAME
/// pairing — the counter subtracts nothing. Property: for ANY presence subset, each
/// flagged pairing's running Σ-subtotal equals its displayed weight (the marker
/// never reaches the number). Reuses the SAME WeightedPairing the renderer projects,
/// proving the subtotal cells are byte-identical to the un-flagged render.
#[test]
fn flagged_score_subtotals_still_sum_to_the_displayed_weight() {
    let ScoreState::Scored { view } = rich_scored_state() else {
        panic!("rich_scored_state must be Scored");
    };
    let all = ["bafyone", "bafytwo", "bafythree"];
    for mask in 0u8..8 {
        let chosen: Vec<&str> = all
            .iter()
            .enumerate()
            .filter(|(i, _)| mask & (1 << i) != 0)
            .map(|(_, c)| *c)
            .collect();
        let presence = presence_of(&chosen);
        let html =
            render_score_results_fragment(&ScoreState::Scored { view: view.clone() }, &presence)
                .into_string();
        // The flagged render's subtotal cells are the SAME projected values, so each
        // pairing's running Σ-subtotal equals its displayed weight (reproduce-by-hand
        // over the SAME WeightedPairing the renderer consumed).
        for pairing in &view.ranked {
            let mut running = 0.0_f64;
            for c in pairing.contributions() {
                let subtotal_str = format!("{:.2}", c.subtotal);
                assert!(
                    html.contains(&subtotal_str),
                    "presence {chosen:?}: the flagged breakdown must render the \
                         subtotal {subtotal_str:?} verbatim (the counter subtracts \
                         nothing); got:\n{html}"
                );
                running += c.subtotal;
            }
            assert!(
                (running - pairing.weight).abs() < 1e-9,
                "presence {chosen:?}: Σ subtotal ({running}) must equal the displayed \
                     weight ({}) on a FLAGGED render — the presence bool never reaches a \
                     number (AC-SCORE-SUMWEIGHT)",
                pairing.weight
            );
        }
    }
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
        survey_row(
            "did:plc:rachel-test",
            "bafy1",
            "github:rust-lang/cargo",
            "phil-a",
            0.90,
        ),
        survey_row(
            "did:plc:rachel-test",
            "bafy2",
            "github:rust-lang/cargo",
            "phil-b",
            0.74,
        ),
    ];
    let view = group_project(
        "github:rust-lang/cargo",
        &rows,
        &std::collections::HashSet::new(),
    );
    let TraversalView::Found {
        entity,
        groups,
        contributors,
    } = view
    else {
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
        survey_row(
            "did:plc:maria",
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
    let view = group_project(
        "github:rust-lang/cargo",
        &rows,
        &std::collections::HashSet::new(),
    );
    let TraversalView::Found {
        groups,
        contributors,
        ..
    } = view
    else {
        panic!("expected Found");
    };
    assert_eq!(groups.len(), 1, "one shared object → one group");
    assert_eq!(
        groups[0].edges.len(),
        2,
        "two authors → two edges (no merge)"
    );
    assert_eq!(contributors.len(), 2, "two distinct contributors");
}

/// Behavior (I-GT-4): an EMPTY survey yields `NoClaims` naming the entity — never
/// a fabricated edge.
#[test]
fn group_project_empty_rows_yields_no_claims_naming_the_entity() {
    let view = group_project(
        "github:nonexistent/repo",
        &[],
        &std::collections::HashSet::new(),
    );
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
    let view = group_project(
        "github:rust-lang/cargo",
        &rows,
        &std::collections::HashSet::new(),
    );
    let html = render_project_fragment(&view).into_string();
    assert!(
        html.contains(TRAVERSAL_RESULTS_ID),
        "fragment must carry the region id; {html}"
    );
    assert!(
        html.contains("/philosophy?object="),
        "the group key must be a /philosophy traversal href; {html}"
    );
    assert!(
        html.contains("/score?contributor="),
        "the author must be a /score traversal link; {html}"
    );
    assert!(
        html.contains("did:plc:rachel-test"),
        "edge must attribute its author; {html}"
    );
    assert!(
        html.contains("0.90"),
        "confidence must render VERBATIM (0.90, not 0.9); {html}"
    );
    assert!(
        html.contains("triangulated"),
        "the REUSED display-only bucket must show; {html}"
    );
    assert!(
        html.contains("bafyedge1"),
        "the edge must name its cid; {html}"
    );
    // NO full-page chrome (I-GT-6 / I-HX-1).
    assert!(
        !html.contains("<!DOCTYPE") && !html.contains("<html"),
        "fragment has no chrome; {html}"
    );
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
    assert!(
        html.contains("github:nonexistent/repo"),
        "must name the queried entity; {html}"
    );
    assert!(
        html.contains(TRAVERSAL_NO_CLAIMS_NOTICE),
        "must show the guided notice; {html}"
    );
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
    let view = group_project(
        "github:rust-lang/cargo",
        &rows,
        &std::collections::HashSet::new(),
    );
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
                    bytes[j].is_ascii_digit() || (b'A'..=b'F').contains(&bytes[j]),
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
    let view = group_project(
        "github:rust-lang/cargo",
        &rows,
        &std::collections::HashSet::new(),
    );
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
// Slice-13 (step 02-01 / US-CF-003) — the EDGE "Countered" presence flag: the
// grouper projects `EdgeRow.is_countered` from the ONE flattened
// `counter_presence_for` set, the SHARED `render_edge_row` arm emits the flag, and
// the flag is ADDITIVE (grouping/order/contributor dedup unchanged; I-CF-9).
// -------------------------------------------------------------------------

/// Behavior (US-CF-003 / I-CF-9): `group_by` (via `group_project`) sets
/// `EdgeRow.is_countered` to TRUE iff that edge's CID is a member of the presence
/// set, and FALSE otherwise — the flag is a TOTAL function of (rows, presence).
#[test]
fn group_project_sets_is_countered_iff_cid_in_presence_set() {
    let rows = [
        survey_row(
            "did:plc:rachel-test",
            "bafy-countered",
            "github:rust-lang/cargo",
            "phil-a",
            0.90,
        ),
        survey_row(
            "did:plc:rachel-test",
            "bafy-plain",
            "github:rust-lang/cargo",
            "phil-b",
            0.74,
        ),
    ];
    let presence: std::collections::HashSet<String> =
        std::iter::once("bafy-countered".to_string()).collect();
    let view = group_project("github:rust-lang/cargo", &rows, &presence);
    let TraversalView::Found { groups, .. } = view else {
        panic!("expected Found");
    };
    let countered = &groups[0].edges[0];
    let plain = &groups[1].edges[0];
    assert_eq!(countered.cid, "bafy-countered");
    assert!(
        countered.is_countered,
        "the edge whose cid is in presence must be flagged"
    );
    assert_eq!(plain.cid, "bafy-plain");
    assert!(
        !plain.is_countered,
        "the edge whose cid is NOT in presence must NOT be flagged"
    );
}

/// Behavior (US-CF-003 / I-CF-6 / I-CF-2): the SHARED `render_edge_row` arm emits the
/// render-only `<a href="/claims/{cid}">Countered</a>` one-hop marker ONLY for a
/// countered edge; an un-countered edge renders NO marker (no-noise).
#[test]
fn render_edge_row_emits_the_flag_iff_is_countered_with_claims_anchor() {
    let rows = [
        survey_row(
            "did:plc:rachel-test",
            "bafy-countered",
            "github:rust-lang/cargo",
            "phil-a",
            0.90,
        ),
        survey_row(
            "did:plc:rachel-test",
            "bafy-plain",
            "github:rust-lang/cargo",
            "phil-b",
            0.74,
        ),
    ];
    let presence: std::collections::HashSet<String> =
        std::iter::once("bafy-countered".to_string()).collect();
    let view = group_project("github:rust-lang/cargo", &rows, &presence);
    let html = render_project_fragment(&view).into_string();
    assert!(
        html.contains(r#"<a href="/claims/bafy-countered">Countered</a>"#),
        "the countered edge must carry the render-only one-hop /claims marker; {html}"
    );
    assert!(
        !html.contains(r#"<a href="/claims/bafy-plain">Countered</a>"#),
        "the un-countered edge must carry NO marker; {html}"
    );
    // PRESENCE-only: exactly ONE flag text appears (the single countered edge).
    assert_eq!(
        html.matches(COUNTERED_PRESENCE_FLAG).count(),
        1,
        "exactly one neutral presence marker, never a count; {html}"
    );
}

/// Behavior (US-CF-003 CARDINAL no-regroup / I-CF-9): the flag is ADDITIVE — with the
/// presence set EMPTY vs NON-EMPTY, the grouping (key_order), per-group edge order,
/// and the deduped contributor list are byte-identical once the additive `Countered`
/// anchors are elided. The flag re-grouped / re-ordered / re-deduped NOTHING.
#[test]
fn edge_flag_is_additive_grouping_order_and_contributors_byte_identical() {
    let rows = [
        survey_row(
            "did:plc:maria",
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
        survey_row(
            "did:plc:maria",
            "bafy3",
            "github:rust-lang/cargo",
            "phil-b",
            0.74,
        ),
    ];
    let empty = std::collections::HashSet::new();
    let baseline = render_project_fragment(&group_project("github:rust-lang/cargo", &rows, &empty))
        .into_string();
    let presence: std::collections::HashSet<String> = ["bafy1".to_string(), "bafy3".to_string()]
        .into_iter()
        .collect();
    let flagged =
        render_project_fragment(&group_project("github:rust-lang/cargo", &rows, &presence))
            .into_string();
    // Elide every additive anchor from the flagged render; what remains must equal the
    // no-flag baseline byte-for-byte.
    let mut elided = flagged.clone();
    for cid in ["bafy1", "bafy2", "bafy3"] {
        elided = elided.replace(&format!(r#"<a href="/claims/{cid}">Countered</a>"#), "");
    }
    assert_eq!(
        elided, baseline,
        "eliding the additive Countered anchors must recover the slice-10 byte-stream \
             (grouping/order/contributor dedup unchanged); flagged:\n{flagged}"
    );
}

/// Behavior (US-CF-003 — ONE shared arm serves BOTH routes): the SAME `render_edge_row`
/// arm emits the flag on the `/philosophy` survey too (consumed via
/// `render_philosophy_fragment`), proving the flag is not project-only.
#[test]
fn the_shared_edge_arm_flags_a_countered_edge_on_the_philosophy_survey_too() {
    let rows = [survey_row(
        "did:plc:rachel-test",
        "bafy-phil-countered",
        "github:rust-lang/cargo",
        "org.openlore.philosophy.reproducible-builds",
        0.90,
    )];
    let presence: std::collections::HashSet<String> =
        std::iter::once("bafy-phil-countered".to_string()).collect();
    let view = group_philosophy(
        "org.openlore.philosophy.reproducible-builds",
        &rows,
        &presence,
    );
    let html = render_philosophy_fragment(&view).into_string();
    assert!(
        html.contains(r#"<a href="/claims/bafy-phil-countered">Countered</a>"#),
        "the SHARED render_edge_row arm must flag the countered edge on /philosophy too; {html}"
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
        survey_row(
            "did:plc:rachel-test",
            "bafy1",
            "github:NixOS/nixpkgs",
            "phil-x",
            0.92,
        ),
        survey_row(
            "did:plc:rachel-test",
            "bafy2",
            "github:bazelbuild/bazel",
            "phil-x",
            0.85,
        ),
    ];
    let view = group_philosophy("phil-x", &rows, &std::collections::HashSet::new());
    let TraversalView::Found {
        entity,
        groups,
        contributors,
    } = view
    else {
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
        survey_row(
            "did:plc:maria",
            "bafy1",
            "github:NixOS/nixpkgs",
            "phil-x",
            0.92,
        ),
        survey_row(
            "did:plc:tobias-test",
            "bafy2",
            "github:NixOS/nixpkgs",
            "phil-x",
            0.70,
        ),
    ];
    let view = group_philosophy("phil-x", &rows, &std::collections::HashSet::new());
    let TraversalView::Found {
        groups,
        contributors,
        ..
    } = view
    else {
        panic!("expected Found");
    };
    assert_eq!(groups.len(), 1, "one shared subject → one group");
    assert_eq!(
        groups[0].edges.len(),
        2,
        "two authors → two edges (no merge)"
    );
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
    let view = group_philosophy(
        "org.openlore.philosophy.reproducible-builds",
        &rows,
        &std::collections::HashSet::new(),
    );
    let html = render_philosophy_fragment(&view).into_string();
    assert!(
        html.contains(TRAVERSAL_RESULTS_ID),
        "fragment must carry the region id; {html}"
    );
    assert!(
        html.contains("/project?subject="),
        "the group key (a project) must be a /project traversal href; {html}"
    );
    assert!(
        html.contains("/score?contributor="),
        "the author must be a /score traversal link; {html}"
    );
    assert!(
        html.contains("did:plc:rachel-test"),
        "edge must attribute its author; {html}"
    );
    assert!(
        html.contains("0.92"),
        "confidence must render VERBATIM (0.92, not 0.9); {html}"
    );
    assert!(
        html.contains("triangulated"),
        "the REUSED display-only bucket must show; {html}"
    );
    assert!(
        html.contains("bafyedge1"),
        "the edge must name its cid; {html}"
    );
    // NO full-page chrome (I-GT-6 / I-HX-1).
    assert!(
        !html.contains("<!DOCTYPE") && !html.contains("<html"),
        "fragment has no chrome; {html}"
    );
    // The philosophy fragment groups BY subject → it must NOT link to /philosophy.
    assert!(
        !html.contains("/philosophy?object="),
        "philosophy survey keys link to /project, not /philosophy; {html}"
    );
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
    assert!(
        html.contains("org.openlore.philosophy.actor-model"),
        "must name the queried entity; {html}"
    );
    assert!(
        html.contains(TRAVERSAL_NO_CLAIMS_NOTICE),
        "must show the guided notice; {html}"
    );
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
    let view = group_philosophy("phil-x", &rows, &std::collections::HashSet::new());
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
        assert!(
            !html.contains(noise),
            "no-noise: {noise:?} must be absent; {html}"
        );
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
        assert!(
            !html.contains(noise),
            "no-noise: {noise:?} must be absent; {html}"
        );
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

    let flagged = render_peer_claims_table_fragment(&PageView::new(rows_flagged)).into_string();
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

// =========================================================================
// Peer Subscriptions view (slice-15; ADR-052) — PeersView + render_peers_*
// + render_remove_guidance. The load-bearing pure-core mutation gate for the
// /peers thread: per-peer attribution + count fidelity, the render-only
// revocation command (bare-DID strip), the NoSubscriptions arm, page↔fragment
// parity by construction.
// =========================================================================

fn summary(peer_did: &str, count: u64) -> PeerSubscriptionSummary {
    PeerSubscriptionSummary {
        peer_did: peer_did.to_string(),
        peer_handle: "handle.test".to_string(),
        subscribed_at: chrono::DateTime::<chrono::Utc>::from_timestamp(0, 0).unwrap(),
        local_claim_count: count,
    }
}

/// Behavior (DD-PS-5): an EMPTY active set maps to `NoSubscriptions` (the guided
/// empty state), a non-empty set to `Subscriptions` — the pure `peers_view` map.
#[test]
fn peers_view_maps_empty_to_no_subscriptions_and_nonempty_to_subscriptions() {
    assert_eq!(peers_view(Vec::new()), PeersView::NoSubscriptions);
    let view = peers_view(vec![summary("did:plc:rachel-test", 5)]);
    assert!(
        matches!(view, PeersView::Subscriptions { ref peers } if peers.len() == 1),
        "a non-empty active set must map to Subscriptions; got {view:?}"
    );
}

/// Behavior (I-PS-3 / J-003a): `render_peers_fragment` carries the `#peers` region
/// id and renders ONE attributed row per peer — each peer's DID VERBATIM + its OWN
/// per-peer count (5 and 3), NEVER a merged total of 8, NO "all peers" row.
#[test]
fn render_peers_fragment_renders_one_attributed_row_per_peer_with_its_own_count() {
    let view = peers_view(vec![
        summary("did:plc:rachel-test", 5),
        summary("did:plc:tobias-test", 3),
    ]);
    let html = render_peers_fragment(&view).into_string();

    assert!(
        html.contains(PEERS_REGION_ID),
        "fragment must carry the #peers region id; {html}"
    );
    // Each peer DID is rendered verbatim with its OWN per-peer count.
    assert!(
        html.contains("did:plc:rachel-test"),
        "Rachel's DID must render verbatim; {html}"
    );
    assert!(
        html.contains("did:plc:tobias-test"),
        "Tobias's DID must render verbatim; {html}"
    );
    assert!(
        html.contains("5 cached claims"),
        "Rachel's per-peer count 5 must render; {html}"
    );
    assert!(
        html.contains("3 cached claims"),
        "Tobias's per-peer count 3 must render; {html}"
    );
    // The merged total (5+3=8) must NEVER appear, and there is NO merged aggregate row.
    assert!(
        !html.contains("8 cached claims"),
        "the per-peer counts must NEVER be summed into a merged total (8); {html}"
    );
    let lowered = html.to_ascii_lowercase();
    for banned in ["all peers", "consensus", "combined total"] {
        assert!(
            !lowered.contains(banned),
            "no merged {banned:?} aggregate row; {html}"
        );
    }
}

/// Behavior (DD-PS-2): a subscribed-but-never-pulled peer renders with count 0
/// (the LEFT JOIN + COUNT(pc.cid) zero is projected verbatim, never dropped).
#[test]
fn render_peers_fragment_renders_a_zero_claims_peer_at_count_zero() {
    let view = peers_view(vec![summary("did:plc:newpeer-test", 0)]);
    let html = render_peers_fragment(&view).into_string();
    assert!(
        html.contains("did:plc:newpeer-test"),
        "the zero-claims peer must render; {html}"
    );
    assert!(
        html.contains("0 cached claims"),
        "the zero-claims peer must render count 0; {html}"
    );
}

/// Behavior (DD-PS-6 / I-PS-1): `render_remove_guidance` emits the render-only
/// `openlore peer remove <bare-did>` command as TEXT (the prefix + the BARE DID,
/// `#fragment` stripped) — never an executable control. The mutation gate for the
/// revocation-command text + the bare-DID strip.
#[test]
fn render_remove_guidance_emits_the_bare_did_revocation_command_as_text() {
    let html = render_remove_guidance("did:plc:rachel-test#org.openlore.app").into_string();
    // Prefix + BARE did (the `#…` app-identity fragment is stripped — slice-03 verb form).
    assert!(
        html.contains("openlore peer remove did:plc:rachel-test"),
        "the bare-DID revocation command must render as text; {html}"
    );
    assert!(
        !html.contains("#org.openlore.app"),
        "the app-identity fragment must be stripped (bare-DID form); {html}"
    );
    // Render-only TEXT — NO executable control.
    let lowered = html.to_ascii_lowercase();
    for banned in [
        "<button",
        "<form",
        "hx-post",
        "hx-delete",
        "hx-put",
        "name=\"remove\"",
    ] {
        assert!(
            !lowered.contains(banned),
            "the command must be render-only TEXT, no {banned:?}; {html}"
        );
    }
}

/// Behavior (US-PS-003 / I-PS-2): the `NoSubscriptions` arm renders the guided
/// empty-state notice + the render-only `openlore peer add` starting command —
/// never blank, never an executable control.
#[test]
fn render_peers_fragment_no_subscriptions_renders_guided_empty_state() {
    let html = render_peers_fragment(&PeersView::NoSubscriptions).into_string();
    assert!(
        html.contains(PEERS_REGION_ID),
        "the empty-state fragment carries the #peers id; {html}"
    );
    assert!(
        html.contains(PEERS_NO_SUBSCRIPTIONS_NOTICE),
        "the guided empty state must name the no-peers notice; {html}"
    );
    assert!(
        html.contains("openlore peer add"),
        "the guided empty state must show the render-only `openlore peer add` command; {html}"
    );
}

/// Behavior (I-PS-5 parity by construction): `render_peers_page` EMBEDS
/// `render_peers_fragment` verbatim — the full page is chrome + the SAME fragment,
/// so the fragment body appears inside the full page; the page also carries the
/// `<head>` chrome + the single local offline-first htmx `<script src>` (no CDN).
#[test]
fn render_peers_page_embeds_the_fragment_and_carries_offline_chrome() {
    let view = peers_view(vec![summary("did:plc:rachel-test", 5)]);
    let fragment = render_peers_fragment(&view).into_string();
    let page = render_peers_page(&view);

    assert!(
        page.contains(&fragment),
        "the full page must EMBED the fragment verbatim (parity); page:\n{page}"
    );
    assert!(
        page.to_ascii_lowercase().contains("<!doctype html>"),
        "the page must be a full document; {page}"
    );
    assert!(
        page.contains(HTMX_ASSET_URL),
        "the page must reference the LOCAL htmx asset (offline-first); {page}"
    );
    assert!(
        !page.contains("//cdn") && !page.contains("https://unpkg"),
        "the page must reference NO off-host CDN; {page}"
    );
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    /// Property (I-PS-3 / J-003a): for ANY active set of peers with arbitrary
    /// distinct DIDs + per-peer counts, `render_peers_fragment` renders EVERY
    /// peer's DID verbatim AND its OWN per-peer count, and the count rendered for
    /// each peer equals its `local_claim_count` (never summed/merged across peers).
    #[test]
    fn render_peers_fragment_renders_every_peer_with_its_own_count(
        counts in prop::collection::vec(0u64..50, 1..8)
    ) {
        let peers: Vec<PeerSubscriptionSummary> = counts
            .iter()
            .enumerate()
            .map(|(i, &c)| summary(&format!("did:plc:peer-{i}-test"), c))
            .collect();
        let html = render_peers_fragment(&peers_view(peers.clone())).into_string();
        for peer in &peers {
            prop_assert!(html.contains(&peer.peer_did), "DID {:?} must render", peer.peer_did);
            prop_assert!(
                html.contains(&format!("{} cached claims", peer.local_claim_count)),
                "peer {:?} must render its OWN count {}", peer.peer_did, peer.local_claim_count
            );
            // The render-only revocation command for this peer (bare-DID).
            prop_assert!(
                html.contains(&format!("openlore peer remove {}", peer.peer_did)),
                "peer {:?} must carry its render-only remove command", peer.peer_did
            );
        }
    }
}
