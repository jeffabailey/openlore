//! `/scrape` — the Live Scrape candidate surface.

use super::*;

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
