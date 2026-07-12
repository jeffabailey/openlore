//! `/search` — the network discovery surface + four-arm follow-state.

use super::*;

/// The HTML `id` of the network-search swap-target element — the `<div>` the htmx
/// `#search-results` fragment IS, and the region the full `/search` page wraps
/// chrome (+ the dimension form) around (slice-08; ADR-037 / mirrors
/// [`SCRAPE_RESULTS_ID`]). Held in ONE place so the fragment fn and any future
/// `hx-target`/`hx-swap` reference the SAME id (a mutation to the id has exactly
/// one site to attack — pinned by the unit test). The no-JS full page embeds the
/// SAME `<div id="search-results">`, so the fragment and the full page's results
/// region are structurally identical (I-NS-6 parity by construction).
pub const SEARCH_RESULTS_ID: &str = "search-results";

/// The real route the network-search view is served at (`/search`) — the no-JS
/// `href`/form `action`, the htmx `hx-get`, AND the nav link all reference this
/// one path (one source of truth for "where the search lives"). Held in ONE place
/// so the chrome's nav link and the form's action can never drift apart.
pub const SEARCH_URL: &str = "/search";

/// The `[verified]` marker every rendered network-search row carries (I-NS-4 —
/// verification is an ingest precondition; there is no unverified state on the
/// viewer surface). Held in ONE place so the marker text is a single mutation
/// site. The acceptance gate (`assert_search_html_every_row_verified_and_attributed`)
/// counts these per author row.
pub const SEARCH_VERIFIED_MARKER: &str = "[verified]";

/// The inline counter-annotation prefix a countered row carries (OD-AV-7 / I-NS-3 —
/// shown, NEVER applied): `countered by <K.author> (<K.cid>)`. Held in ONE place so
/// the shown-not-applied annotation text the browser surface renders is a single
/// source of truth. The counter is an ANNOTATION on the still-rendered countered
/// row — it never filters, merges, or over-rides the claim (the viewer inherits the
/// slice-05 CLI counter-render discipline).
pub const SEARCH_COUNTERED_BY_PREFIX: &str = "countered by";

/// The public-data framing banner the `/search` page states UP FRONT (I-NS-5):
/// discovery indexes only PUBLIC signed claims, verified before indexing; nothing
/// private is read. Held in ONE place so the framing is a single source of truth.
pub const SEARCH_PUBLIC_DATA_NOTICE: &str =
    "Discovery indexes only public signed claims, verified before indexing — \
     nothing private is read.";

/// The fixed plain-language notice the `SearchState::Unavailable` arm renders when
/// the configured network index is unreachable OR unconfigured (I-NS-2 / WD-NS-4).
/// Held in ONE place AND emitted as a fixed constant (NEVER interpolated from a
/// transport error) so the degradation message is a single source of truth AND
/// structurally cannot leak internals: it states the index is unavailable and that
/// the operator's LOCAL store views still work, with NO HTTP status, "connection
/// refused", raw URL, or stack-trace marker (the `Unavailable` arm is a UNIT
/// variant precisely so no transport string can be threaded in). Pinned by the
/// leak-absence unit test + the N-13..N-16 acceptance gate.
pub const SEARCH_UNAVAILABLE_NOTICE: &str =
    "The network index is unavailable. Your local store views still work.";

/// The honest-framing FOOTER the CONTRIBUTOR dimension renders beneath a
/// developer's verified trail (US-NS-003 / AC-003.2): a contributor search surfaces
/// ONE developer's reasoning — it is NOT a community consensus, and the footer says
/// so up front so the per-author trail can never be mistaken for an aggregate
/// verdict. Held in ONE place (the SAME wording the slice-05 CLI `--contributor`
/// render emits) so the honesty promise is a single source of truth + a single
/// mutation site. It is a PROMISE, not a merged row — the anti-merging scan
/// (`assert_search_html_has_no_merged_consensus_row`) excludes it by construction.
pub const SEARCH_CONTRIBUTOR_FOOTER: &str =
    "This is one developer's reasoning trail, not a community consensus.";

/// The render-only follow GUIDANCE prefix an UNFOLLOWED network-author row carries
/// (N-17 / AC-004.5 / WD-NS-3 / I-NS-1): the viewer surfaces the slice-03
/// `openlore peer add <bare-did>` command as TEXT so the operator can follow the
/// author FROM THE CLI. It is GUIDANCE ONLY — there is NO executable follow /
/// subscribe control and NO auto-subscribe path; following stays a deliberate CLI
/// action and the read-only viewer holds no key. Held in ONE place (the SAME slice-03
/// verb the CLI `search` follow affordance emits) so the guidance is a single source
/// of truth + a single mutation site. The bare DID (the slice-03 `peer add` verb's
/// accepted form) is appended by [`render_follow_guidance`].
pub const SEARCH_FOLLOW_GUIDANCE_PREFIX: &str =
    "Follow this author from the CLI: openlore peer add";

/// The neutral render-only "Following" indicator a SubscribedPeer network-author row
/// carries (slice-16 / US-SF-002 / ADR-053 D3) — the SIBLING of
/// [`SEARCH_FOLLOW_GUIDANCE_PREFIX`]. When the operator ALREADY follows a search-result
/// author (resolved against the LOCAL active-subscription set in the effect shell), the
/// row shows this neutral LABEL instead of the `openlore peer add <did>` guidance — an
/// already-followed author is NOT re-offered a follow (R-SF-3). It is a NEUTRAL copy:
/// no command, no verb-phrase, no DID — distinct from the follow guidance. It is render-
/// only TEXT (C-1, CARDINAL): NO executable follow/unfollow/subscribe control, NO `hx-*`
/// mutation; the read-only viewer holds no key and exposes no follow route. Held in ONE
/// place (mirrors `SEARCH_FOLLOW_GUIDANCE_PREFIX`) so the "Following" copy is a single
/// source of truth + a single mutation site; emitted by [`render_following_indicator`].
pub const SEARCH_FOLLOWING_INDICATOR: &str = "Following";

/// The neutral render-only SELF indicator a `You` network-author row carries
/// (slice-20 / US-FS-002 / ADR-057 D3) — the result is the OPERATOR's OWN claim, so
/// neither "Following" nor an `openlore peer add` follow applies (you cannot follow
/// yourself). A NEUTRAL self-attribution LABEL: no command, no verb-phrase, no DID,
/// no judgement. It is render-only TEXT (C-1, CARDINAL): NO executable
/// follow/unfollow/subscribe control, NO `hx-*` mutation; the read-only viewer holds
/// no key. Held in ONE place (mirrors [`SEARCH_FOLLOWING_INDICATOR`]) so the self
/// copy is a single source of truth; emitted by [`render_self_indicator`].
pub const SEARCH_SELF_INDICATOR: &str = "Your own claim";

/// The neutral render-only RESIDUE indicator an `UnsubscribedCache` network-author
/// row carries (slice-20 / US-FS-002 / ADR-057 D3) — the result is a peer the
/// operator SOFT-REMOVED (his cached claims retained, subscription inactive). He is
/// residue, NOT a fresh network find, so the `openlore peer add` affordance is
/// suppressed (like `SubscribedPeer`). A NEUTRAL descriptive LABEL: no command, no
/// DID, never pejorative (no "ex-peer"/"stale"/"abandoned"). It is render-only TEXT
/// (C-1, CARDINAL): NO executable control, NO `hx-*` mutation. Held in ONE place
/// (mirrors [`SEARCH_FOLLOWING_INDICATOR`]); emitted by
/// [`render_cached_unsubscribed_indicator`].
pub const SEARCH_REMOVED_CACHED_INDICATOR: &str = "A peer you removed (cached)";

/// The read-only `?hide_retracted=1` GET-param CONTROL label the `/search` form
/// renders (feature `retraction-aware-search-filter`; US-RF-002 / OD-RF-2 / ADR-060).
/// A plain GET-param checkbox toggle — a public-data READ affordance, NEVER a
/// write/sign/subscribe control (I-RF-6): the read-only viewer holds no key. Held in
/// ONE place; the SUBSTRING the RF-V6 acceptance gate keys on.
pub const SEARCH_HIDE_RETRACTED_LABEL: &str = "Hide retracted claims";

/// The `<input name>` / GET-param key of the read-only hide toggle
/// (`?hide_retracted=1`, feature `retraction-aware-search-filter`). Held in ONE place
/// so the form control's `name` and the effect shell's param key cannot drift apart.
pub const SEARCH_HIDE_RETRACTED_PARAM: &str = "hide_retracted";

/// Content-frozen retraction-count noun MIRRORING the slice-01 CLI
/// `cli::render::search::RETRACTION_HIDDEN_COUNT_NOUN` (US-RF-001/002 / OD-RF-3 /
/// D-RF-D5): the honest unit is retraction EVENTS — "1 retracted claim(s) hidden",
/// NOT 2 rows. The CLI const lives in the `cli` crate (which `viewer-domain` cannot
/// import), so the SUBSTRING is mirrored VERBATIM here so both surfaces stay
/// byte-identical. Do NOT paraphrase — the exact phrasing is the disclosure contract.
pub const SEARCH_RETRACTION_HIDDEN_COUNT_NOUN: &str = "retracted claim(s) hidden";

/// The viewer re-run guidance appended to EVERY hide disclosure — the browser
/// equivalent of the CLI `re-run without --hide-retracted` (US-RF-002 / I-RF-3): the
/// filter is non-destructive + reversible, so the surface ALWAYS names how to restore
/// the hidden rows (untick the read-only GET-param control). Held in ONE place;
/// carries the "Untick" verb the RF-V1/V4 acceptance gate keys on.
pub const SEARCH_RETRACTION_UNTICK_GUIDANCE: &str =
    "Untick the Hide retracted claims control to see them again.";

/// Content-frozen empty-after-filter fragment MIRRORING the slice-01 CLI
/// `cli::render::search::RETRACTION_ALL_HIDDEN_FRAGMENT` (US-RF-002 / RF-V4 / I-RF-3):
/// when the filter hid EVERY result, the guided region states the claims
/// `were soft-retracted` — an explicit "they exist but were withdrawn" state, never a
/// bare blank region. Do NOT paraphrase.
pub const SEARCH_RETRACTION_ALL_HIDDEN_FRAGMENT: &str = "were soft-retracted";

/// Resolve the four-arm [`AuthorRelationship`] for ONE search-result author against
/// the operator's THREE LOCAL presence sets (slice-20 / US-FS-001/002 / ADR-057 D2).
/// PURE total deterministic function — the SSOT for the `/search` follow-state
/// precedence, mirroring the LOCAL-graph precedence the federated-read resolver uses.
///
/// PRECEDENCE (C-6 / WD-FS-2 / ADR-057 D2), strongest fact first:
/// `You` > `SubscribedPeer` > `UnsubscribedCache` > `NetworkUnfollowed`.
///   • the author IS the operator (∈ `own`) → [`AuthorRelationship::You`];
///   • else actively followed (∈ `active`) → [`AuthorRelationship::SubscribedPeer`];
///   • else soft-removed-but-cached (∈ `cached`) → [`AuthorRelationship::UnsubscribedCache`];
///   • else a genuinely-new network author → [`AuthorRelationship::NetworkUnfollowed`].
///
/// The three sets store BARE DIDs (the own/active/cached LOCAL reads project the bare
/// `author_did`/`peer_did`); the result `author_did` may carry the
/// `#org.openlore.application` signing fragment — so the membership test strips the
/// fragment via the [`bare_did`] SSOT on the RESULT side before `HashSet::contains`
/// (R-FS-6). A total if/else-if chain with a total `else` — every author lands in
/// exactly one arm; an empty set simply never matches (the slice-16 fall-through).
pub fn resolve_author_relationship(
    author_did: &str,
    own: &std::collections::HashSet<String>,
    active: &std::collections::HashSet<String>,
    cached: &std::collections::HashSet<String>,
) -> AuthorRelationship {
    let bare = bare_did(author_did);
    if own.contains(bare) {
        AuthorRelationship::You
    } else if active.contains(bare) {
        AuthorRelationship::SubscribedPeer
    } else if cached.contains(bare) {
        AuthorRelationship::UnsubscribedCache
    } else {
        AuthorRelationship::NetworkUnfollowed
    }
}

/// The state the network-search results region renders (the pure render input). An
/// ADT over the four outcomes of a `/search` interaction so the renderer matches
/// totally (nw-fp-domain-modeling §1): the empty GET form, a populated per-author
/// result, a guided no-results empty state, or the fixed unavailable notice. The
/// effect shell builds this from the index-query outcome (REACHABLE-with-results →
/// `Results`; reachable-zero → `NoResults`; unreachable/unconfigured →
/// `Unavailable`; the bare `GET /search` with no dimension → `Form`); the renderer
/// is a pure total function over it.
#[derive(Debug, Clone, PartialEq)]
pub enum SearchState {
    /// `GET /search` with no dimension supplied: the empty dimension form, no
    /// query run yet.
    Form,
    /// A REACHABLE index returned ≥1 verified row: render the per-author groups.
    /// Carries the REUSED `appview-domain::compose_results` output VERBATIM — the
    /// viewer holds NO second grouping/verification path (anti-merging is the pure
    /// core's job; the renderer only projects it). The `dimension` the search ran
    /// along is carried alongside so the renderer can add the dimension-specific
    /// honest-framing footer (the CONTRIBUTOR dimension surfaces ONE developer's
    /// trail + the "not a community consensus" footer, US-NS-003 / AC-003.2); the
    /// per-author projection itself is dimension-independent.
    Results {
        /// The REUSED per-author `compose_results` output (anti-merging by
        /// construction — there is no merged "network consensus" row).
        result: appview_domain::NetworkSearchResult,
        /// The dimension the search ran along — selects the dimension-specific
        /// footer (CONTRIBUTOR → the honest-framing "not a community consensus"
        /// line; OBJECT/SUBJECT → none). The grouping is unaffected.
        dimension: appview_domain::SearchDimension,
    },
    /// A REACHABLE index returned ZERO rows for the queried dimension+value
    /// (US-NS-002 Ex 4 / SearchState::NoResults): render a guided plain-language
    /// "no claims found" empty state naming the queried value — never a blank
    /// region or a crash.
    NoResults {
        /// The queried value, named in the guided empty state (so the operator
        /// sees WHAT was searched). E.g. a typo'd object or an absent contributor.
        queried_value: String,
    },
    /// The configured index is UNREACHABLE or UNCONFIGURED (I-NS-2): render the
    /// FIXED [`SEARCH_UNAVAILABLE_NOTICE`]. A UNIT variant — it carries NO
    /// transport detail, so the raw error/URL/status CANNOT be interpolated,
    /// guaranteeing no leaked internals (I-NS-2) by construction.
    Unavailable,
    /// A REACHABLE index returned rows AND `?hide_retracted=1` hid ≥1 author-self-
    /// retraction EVENT while ≥1 SURVIVOR remains (feature
    /// `retraction-aware-search-filter`; US-RF-002 / RF-V1/V3/V5 / ADR-060). Carries
    /// the SURVIVORS' `compose_results` projection (the SAME anti-merging core
    /// [`Results`] uses — the viewer holds no second grouping path), the search
    /// `dimension` (same honest-framing footer selection as [`Results`]), AND the
    /// disclosed hidden EVENT count. `hidden_count` is `>= 1` BY CONSTRUCTION — a
    /// zero-count filter yields [`Results`] (no misleading "0 hidden" line, D-4), so a
    /// notice-with-zero is UNREPRESENTABLE (nw-fp-domain-modeling §4). The renderer
    /// ALWAYS emits the disclosure notice for this variant, in BOTH htmx shapes
    /// (I-RF-3 / RF-V3 — it lives in the shared results-region fragment).
    FilteredResults {
        /// The SURVIVORS' REUSED per-author `compose_results` output (anti-merging by
        /// construction — decided on the RAW rows in the effect shell BEFORE this
        /// lossy projection, ADR-060 §subtlety-1).
        result: appview_domain::NetworkSearchResult,
        /// The dimension the search ran along — selects the dimension-specific footer
        /// (CONTRIBUTOR → the honest-framing line; OBJECT/SUBJECT → none), exactly as
        /// [`Results`].
        dimension: appview_domain::SearchDimension,
        /// The number of author-self-retraction EVENTS hidden (D-RF-D5) — `>= 1` by
        /// construction. Disclosed VERBATIM in the "N retracted claim(s) hidden" notice.
        hidden_count: u32,
    },
    /// `?hide_retracted=1` hid EVERY matching row (feature
    /// `retraction-aware-search-filter`; US-RF-002 / RF-V4 / I-RF-3): the guided
    /// empty-after-filter region names that all `hidden_count` results
    /// `were soft-retracted` + how to untick to restore them — never a blank region,
    /// never a crash. DISTINCT from [`NoResults`] (the index returned ZERO rows): here
    /// the rows EXIST but were all author-self-retracted (the withdrawn state, not the
    /// absent state).
    AllRetracted {
        /// The number of author-self-retraction EVENTS hidden — `>= 1` by construction
        /// (this variant only arises when the filter emptied a NON-empty result set).
        hidden_count: u32,
    },
}

/// Render the network-search swap-target FRAGMENT (slice-08; ADR-037): the
/// `<div id="search-results">` wrapping the per-author result groups (or the guided
/// no-results / fixed unavailable notice) for the given [`SearchState`]. PURE: a
/// total function from the view-model to a `Markup` — NO full-page chrome (no
/// `<!DOCTYPE>`, no `<html>`/`<head>`) and NO dimension form, so an `HX-Request`
/// response carries ONLY this results region (I-NS-6). Renders NO sign/follow
/// control (I-NS-1 / WD-NS-3 — following stays a CLI action). [`render_search_page`]
/// EMBEDS this SAME fn beneath the form, so the fragment and the full page's results
/// region are byte-identical by construction (I-NS-6 parity — the results-rendering
/// logic is NOT duplicated). This is the slice-08 structural contract: page =
/// chrome + form + fragment.
///
/// The result rows PROJECT `appview-domain`'s per-author [`NetworkSearchResult`] —
/// each group keyed by its author DID, every row carrying the `[verified]` marker,
/// the author DID, and the VERBATIM confidence (via [`render_confidence`]) — and
/// there is NO merged "network consensus" row (the per-author shape is the only
/// output of the REUSED `compose_results`; the viewer never re-groups).
pub fn render_search_results_fragment(state: &SearchState) -> Markup {
    html! {
        div id=(SEARCH_RESULTS_ID) {
            (render_search_result(state))
        }
    }
}

/// Render the network-search page (`GET /search`, US-NS-001..004) as a complete
/// HTML document (maud). PURE: a total function from the [`SearchState`] to an HTML
/// string — no I/O, no network. ALWAYS renders the public-data framing banner UP
/// FRONT (I-NS-5), a nav link back to the other views, and the labeled dimension
/// form (so the operator can submit / re-submit), THEN the results region. Renders
/// NO sign/follow control anywhere (I-NS-1 / WD-NS-3 — following stays a CLI
/// action; the only "follow" surface is the render-only `openlore peer add <did>`
/// guidance TEXT on an unfollowed row).
///
/// COMPOSITION (slice-08; ADR-037): the results region is chrome + framing + form
/// wrapped AROUND [`render_search_results_fragment`] — the EXACT same fragment fn
/// the htmx shape returns alone. Because the results region is the SAME fn in both
/// shapes, fragment/full-page parity is structural, not asserted by duplicating
/// render logic (I-NS-6). The `<head>` emits exactly ONE local
/// `<script src="/static/htmx.min.js">` (offline-first, never a CDN; I-NS-7) — the
/// SAME chrome line every other enhanced page carries, so the form's `hx-get` swap
/// works in-browser instead of falling back to a full GET.
pub fn render_search_page(state: &SearchState) -> String {
    // slice-21 (ADR-058 D6): composed through `page_shell` (persistent left nav +
    // `<main id="viewer-main">`); `active = SEARCH_URL` marks the Network Search nav
    // item current. The `render_*_fragment` fn is UNCHANGED (it rides `Shape::Fragment`
    // for the #search-results swap).
    // feature retraction-aware-search-filter (US-RF-002 / OD-RF-2 / RF-V6): reflect the
    // active hide state on the read-only toggle so unticking restores the hidden rows.
    // The filter-bearing states (FilteredResults / AllRetracted) are the ONLY ones the
    // toggle is active for; every other state renders it unchecked.
    let hide_active = matches!(
        state,
        SearchState::FilteredResults { .. } | SearchState::AllRetracted { .. }
    );
    let body = html! {
        h1 { "Network Search" }
        p { (SEARCH_PUBLIC_DATA_NOTICE) }
        nav {
            a href=(MY_CLAIMS_URL) { "My Claims" }
        }
        (render_search_form(hide_active))
        (render_search_results_fragment(state))
    };
    page_shell("OpenLore — Network Search", SEARCH_URL, body)
}

/// Render the labeled dimension form (`GET /search` and the top of every results
/// render). PURE. The form GETs back to `/search` with a labeled input for EACH
/// dimension the handler parses — `object` (philosophy / object URI),
/// `contributor` (a developer handle, US-NS-003), and
/// `subject` (a project target, US-NS-003) — so the operator can submit / re-submit
/// along ANY dimension. The handler checks the fields in object → contributor →
/// subject order (see `parse_search_dimension`), so an empty field is simply "not
/// this dimension". It carries NO sign/follow control. Enhanced with
/// `hx-get`/`hx-target` so an in-browser submit swaps ONLY the `#search-results`
/// region; the no-JS path is a plain `GET` to `/search`.
fn render_search_form(hide_active: bool) -> Markup {
    html! {
        form method="get" action=(SEARCH_URL)
             hx-get=(SEARCH_URL)
             hx-target=(format!("#{SEARCH_RESULTS_ID}"))
             hx-swap="innerHTML" {
            label for="object" { "Philosophy / object URI" }
            input type="text" id="object" name="object";
            label for="contributor" { "Contributor handle" }
            input type="text" id="contributor" name="contributor";
            label for="subject" { "Project / subject" }
            input type="text" id="subject" name="subject";
            // feature retraction-aware-search-filter (US-RF-002 / OD-RF-2 / I-RF-6 /
            // RF-V6): the read-only `?hide_retracted=1` toggle — a plain GET-param
            // checkbox (a public-data READ affordance), NEVER a write/sign/subscribe
            // control (the viewer holds no signing key). `checked[hide_active]`
            // reflects the active state so unticking + submitting restores the hidden
            // rows via the SAME GET path (no JS required).
            label for=(SEARCH_HIDE_RETRACTED_PARAM) { (SEARCH_HIDE_RETRACTED_LABEL) }
            input type="checkbox" id=(SEARCH_HIDE_RETRACTED_PARAM)
                  name=(SEARCH_HIDE_RETRACTED_PARAM) value="1" checked[hide_active];
            button type="submit" { "Search" }
        }
    }
}

/// Render the results region beneath the form for the given [`SearchState`]. PURE
/// total match over the ADT: the GET form shows nothing yet; results show the
/// per-author groups; no-results shows the guided empty state; unavailable shows
/// the fixed notice.
fn render_search_result(state: &SearchState) -> Markup {
    html! {
        @match state {
            SearchState::Form => {}
            SearchState::Results { result, dimension } => {
                (render_search_author_groups(result))
                (render_search_footer(*dimension))
            }
            // No-results (US-NS-002 Ex 4): the guided plain-language empty state
            // naming the queried value — never a blank region or a crash.
            SearchState::NoResults { queried_value } => {
                p {
                    "No claims found for " (queried_value) "."
                }
            }
            // Unavailable (I-NS-2): the FIXED plain-language notice ONLY — the unit
            // variant carries no transport detail, so nothing can leak.
            SearchState::Unavailable => {
                p { (SEARCH_UNAVAILABLE_NOTICE) }
            }
            // FilteredResults (US-RF-002 / RF-V1/V3/V5): the disclosure notice ABOVE
            // the SURVIVORS + the SAME per-author groups + footer `Results` renders.
            // The notice lives here (in the shared results-region fragment), so BOTH
            // htmx shapes carry it (I-RF-3 / RF-V3 parity by construction).
            SearchState::FilteredResults {
                result,
                dimension,
                hidden_count,
            } => {
                (render_retraction_notice(*hidden_count))
                (render_search_author_groups(result))
                (render_search_footer(*dimension))
            }
            // AllRetracted (US-RF-002 / RF-V4 / I-RF-3): the guided empty-after-filter
            // region — the rows EXIST but were all withdrawn, so this is NOT a blank
            // region and NOT `NoResults`.
            SearchState::AllRetracted { hidden_count } => {
                (render_all_retracted_region(*hidden_count))
            }
        }
    }
}

/// Render the honest retraction disclosure notice shown IN the results region above
/// the survivors when `?hide_retracted=1` hid ≥1 author-self-retraction EVENT
/// (US-RF-002 / RF-V1/V3/V5 / I-RF-3): "N retracted claim(s) hidden" (N = EVENTS,
/// D-RF-D5 — the honest unit, MIRRORING the slice-01 CLI footer via
/// [`SEARCH_RETRACTION_HIDDEN_COUNT_NOUN`]) + the untick-to-restore guidance. It lives
/// in the shared results-region fragment, so BOTH htmx shapes carry it (RF-V3 parity).
/// PURE total function.
fn render_retraction_notice(hidden_count: u32) -> Markup {
    html! {
        p {
            (hidden_count) " " (SEARCH_RETRACTION_HIDDEN_COUNT_NOUN) ". "
            (SEARCH_RETRACTION_UNTICK_GUIDANCE)
        }
    }
}

/// Render the guided empty-after-filter region when `?hide_retracted=1` hid EVERY
/// matching row (US-RF-002 / RF-V4 / I-RF-3): names that all `hidden_count` results
/// `were soft-retracted` by their authors (via [`SEARCH_RETRACTION_ALL_HIDDEN_FRAGMENT`],
/// MIRRORING the slice-01 CLI buffer) + the untick guidance — an explicit withdrawn
/// state, never a bare blank region. PURE total function.
fn render_all_retracted_region(hidden_count: u32) -> Markup {
    html! {
        p {
            "All " (hidden_count) " matching claim(s) "
            (SEARCH_RETRACTION_ALL_HIDDEN_FRAGMENT)
            " by their authors and are hidden from this view ("
            (hidden_count) " " (SEARCH_RETRACTION_HIDDEN_COUNT_NOUN) "). "
            (SEARCH_RETRACTION_UNTICK_GUIDANCE)
        }
    }
}

/// Render the per-author result groups (anti-merging, I-NS-3): one section per
/// author DID, each holding that author's verified rows. PROJECTS the REUSED
/// `appview-domain::compose_results` output — there is NO merged "network
/// consensus" row because the per-author shape is the only thing the pure core
/// produces. Each group is keyed by its author DID (rendered VERBATIM —
/// attribution is never elided).
fn render_search_author_groups(result: &appview_domain::NetworkSearchResult) -> Markup {
    html! {
        @for (author_did, rows) in &result.by_author {
            section {
                h2 { "Author: " (author_did.0) }
                @for row in rows {
                    (render_search_result_row(row))
                }
            }
        }
    }
}

/// Render the dimension-specific honest-framing footer beneath the per-author
/// groups. PURE total match over the dimension: the CONTRIBUTOR dimension surfaces
/// ONE developer's reasoning trail, so it emits the [`SEARCH_CONTRIBUTOR_FOOTER`]
/// "not a community consensus" line (US-NS-003 / AC-003.2) — the same honesty
/// promise the slice-05 CLI `--contributor` render emits. The OBJECT + SUBJECT
/// dimensions render NO footer (their per-author survey speaks for itself; the
/// honesty promise is contributor-specific). The footer is a PROMISE, never a
/// merged row, so it does not collide with the anti-merging guarantee.
fn render_search_footer(dimension: appview_domain::SearchDimension) -> Markup {
    html! {
        @if matches!(dimension, appview_domain::SearchDimension::Contributor) {
            p { (SEARCH_CONTRIBUTOR_FOOTER) }
        }
    }
}

/// Render one network-search result row (a verified, attributed claim). Carries the
/// `[verified]` marker (I-NS-4 — there is no unverified state on the surface), the
/// author DID (attribution, I-NS-3), the claim triple, and the VERBATIM confidence
/// (via [`render_confidence`] — `0.85`, never `0.9`/`90%`; FR-VIEW-8). Renders NO
/// sign/follow control (I-NS-1). The per-row markup is small + named so the
/// load-bearing marker + attribution + verbatim-confidence each have one site to
/// pin against mutation.
fn render_search_result_row(row: &appview_domain::NetworkResultRow) -> Markup {
    html! {
        div {
            span { (SEARCH_VERIFIED_MARKER) }
            " "
            span { (row.author_did.0) }
            " "
            span { (row.subject) " " (row.predicate) " " (row.object) }
            " "
            span { (render_confidence(row.confidence)) }
            // OD-AV-7 / I-NS-3: when this row was COUNTERED, show the counter inline
            // (`countered by <K.author> (<K.cid>)`). The claim above is still
            // rendered VERBATIM — the counter is an ANNOTATION, never applied as a
            // filter/merge/override (the viewer reuses the slice-05 shown-not-applied
            // discipline; the annotation is conditional on `Some`).
            @if let Some(counter) = &row.counter_annotation {
                " "
                span {
                    (SEARCH_COUNTERED_BY_PREFIX) " " (counter.counter_author.0)
                    " (" (counter.referencing_cid.0) ")"
                }
            }
            // slice-16 (US-SF-002 / Theme A / ADR-053 D3) — the per-row follow-state
            // affordance, resolved in the effect shell against the LOCAL active set:
            //   • SubscribedPeer → the NEUTRAL render-only "Following" indicator (the
            //     author the operator ALREADY follows is NOT re-offered a follow,
            //     R-SF-3) — NO `peer add` command.
            //   • NetworkUnfollowed → the slice-08 render-only `openlore peer add
            //     <bare-did>` CLI follow GUIDANCE as TEXT (N-17 / AC-004.5 / WD-NS-3 /
            //     I-NS-1), UNCHANGED.
            // Both arms are render-only TEXT (C-1, CARDINAL): no executable control, no
            // `hx-*` mutation; following stays a deliberate CLI action and the read-only
            // viewer holds no key.
            //
            // slice-20 (US-FS-001/002 / ADR-057 D3) COMPLETES the four-arm resolution
            // the effect shell now performs against the operator's THREE LOCAL presence
            // sets (own / active / cached, via `resolve_author_relationship`):
            //   • You → the NEUTRAL render-only SELF indicator (the operator's OWN claim
            //     — neither "Following" nor `peer add`; you cannot follow yourself) — NO
            //     `peer add` command.
            //   • UnsubscribedCache → the NEUTRAL render-only RESIDUE indicator (a peer
            //     she SOFT-REMOVED; cached residue, NOT a fresh find) — NO `peer add`
            //     command (the affordance is suppressed, like SubscribedPeer).
            // The slice-16 SubscribedPeer + NetworkUnfollowed arms are BYTE-STABLE (C-7,
            // CARDINAL) — the two new arms only ADD.
            @match row.relationship {
                AuthorRelationship::You => (render_self_indicator()),
                AuthorRelationship::SubscribedPeer => (render_following_indicator()),
                AuthorRelationship::UnsubscribedCache => (render_cached_unsubscribed_indicator()),
                AuthorRelationship::NetworkUnfollowed => (render_follow_guidance(&row.author_did.0)),
            }
        }
    }
}

/// Render the render-only CLI follow GUIDANCE for an UNFOLLOWED network author
/// (N-17 / AC-004.5 / WD-NS-3 / I-NS-1) as plain TEXT inside a `<p>` — the slice-03
/// `openlore peer add <bare-did>` command the operator runs to follow the author.
/// It is GUIDANCE ONLY: NO `<button>`/`<form>`/`hx-*` control, NO auto-subscribe.
/// The BARE DID (the slice-03 `peer add` verb's accepted form) is derived by
/// stripping any app-identity `#…` fragment (via the shared [`bare_did`] SSOT),
/// mirroring the CLI `search` follow affordance. PURE total function.
fn render_follow_guidance(author_did: &str) -> Markup {
    html! {
        " "
        p { (SEARCH_FOLLOW_GUIDANCE_PREFIX) " " (bare_did(author_did)) }
    }
}

/// Render the neutral render-only "Following" indicator for an ALREADY-FOLLOWED network
/// author (slice-16 / US-SF-002 / ADR-053 D3) as plain TEXT inside a `<p>` — the SIBLING
/// of [`render_follow_guidance`]. It surfaces the [`SEARCH_FOLLOWING_INDICATOR`] copy as
/// a NEUTRAL LABEL (no command, no verb-phrase, no DID): a developer the operator ALREADY
/// follows is shown as such rather than re-offered a follow (R-SF-3). It is render-only
/// TEXT (C-1, CARDINAL): NO `<button>`/`<form>`/mutating `<a>`/`hx-*` control, NO
/// follow/unfollow/subscribe input — the read-only viewer holds no key. The copy is
/// prefixed with a neutral "Relationship:" label so the indicator is never a bare
/// `>Following<` element (it reads as descriptive TEXT, never a control). PURE total
/// function — takes no input, returns the fixed neutral marker.
fn render_following_indicator() -> Markup {
    html! {
        " "
        p { "Relationship: " (SEARCH_FOLLOWING_INDICATOR) }
    }
}

/// Render the neutral render-only SELF indicator for the operator's OWN claim
/// (slice-20 / US-FS-002 / ADR-057 D3) as plain TEXT inside a `<p>` — the SIBLING of
/// [`render_following_indicator`]. It surfaces the [`SEARCH_SELF_INDICATOR`] copy as a
/// NEUTRAL LABEL (no command, no verb-phrase, no DID): the operator's own claim is
/// shown as such, never re-offered a follow (you cannot follow yourself). It is
/// render-only TEXT (C-1, CARDINAL): NO `<button>`/`<form>`/mutating `<a>`/`hx-*`
/// control — the read-only viewer holds no key. Prefixed with the neutral
/// "Relationship:" label (mirrors `render_following_indicator`) so it reads as
/// descriptive TEXT, never a control. PURE total function.
fn render_self_indicator() -> Markup {
    html! {
        " "
        p { "Relationship: " (SEARCH_SELF_INDICATOR) }
    }
}

/// Render the neutral render-only RESIDUE indicator for a SOFT-REMOVED-but-cached
/// peer (slice-20 / US-FS-002 / ADR-057 D3) as plain TEXT inside a `<p>` — the SIBLING
/// of [`render_following_indicator`]. It surfaces the [`SEARCH_REMOVED_CACHED_INDICATOR`]
/// copy as a NEUTRAL, non-pejorative LABEL (no command, no DID): a peer the operator
/// removed is shown as cached residue, NOT re-offered a follow (he is not a fresh
/// network find). It is render-only TEXT (C-1, CARDINAL): NO `<button>`/`<form>`/
/// mutating `<a>`/`hx-*` control — the read-only viewer holds no key. Prefixed with the
/// neutral "Relationship:" label so it reads as descriptive TEXT, never a control.
/// PURE total function.
fn render_cached_unsubscribed_indicator() -> Markup {
    html! {
        " "
        p { "Relationship: " (SEARCH_REMOVED_CACHED_INDICATOR) }
    }
}

// =============================================================================
// Contributor-Score view (slice-09; ADR-039/040/041) — `GET /score`
// =============================================================================
//
// The `/score` route reads the contributor's LOCAL attributed feed over the
// read-only `StoreReadPort::query_contributor_scoring_feed`, runs the REUSED
// slice-04 PURE `scoring::score(&feed, &ScoringConfig::DEFAULT)` in the effect
// shell, maps the outcome to a [`ScoreState`], and renders it here. This crate
// holds NO scoring math — it PROJECTS the `scoring::WeightedView` (the ranked
// `WeightedPairing`s + their per-claim `Contribution` decomposition). The
// headline weight + the per-claim breakdown are rendered from the SAME
// `WeightedPairing`, so the breakdown subtotals sum to the weight BY
// CONSTRUCTION (Gate 2 / KPI-GRAPH-3 reproduce-by-hand). A score is NEVER shown
// without its breakdown (the J-002c thesis, I-CS-2).
