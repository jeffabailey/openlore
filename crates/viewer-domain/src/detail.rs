//! `/claims/{cid}` — the claim-detail surface + counter-claim threads.

use super::*;

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
    // slice-21 (ADR-058 D6): the detail body is composed through `page_shell`
    // (persistent left nav + `<main id="viewer-main">`). The detail route is the deep
    // `/claims/{cid}` drill from the My Claims list, so `active = MY_CLAIMS_URL` (the
    // base-path const) marks the My Claims nav item current. The `render_*_fragment`
    // fn is UNCHANGED (it rides `Shape::Fragment` for the #claim-detail swap).
    let body = html! {
        h1 { "Claim Detail" }
        p { (READ_ONLY_NOTICE) }
        (render_claim_detail_fragment(claim, thread))
        p {
            a href="/claims" { "Back to My Claims" }
        }
    };
    page_shell("OpenLore — Claim Detail", MY_CLAIMS_URL, body)
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
