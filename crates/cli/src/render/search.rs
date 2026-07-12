//! `openlore search` — network discovery results, verification, share links.

use super::*;

/// Content-frozen public-data-only banner for `search` (KPI-AV-5 / I-AV-4).
/// Printed BEFORE network results so the user is reassured discovery indexes
/// ONLY public, signed, signature-verified claims — nothing private is read or
/// aggregated. Do NOT paraphrase — the exact string is the user-visible
/// contract. (Distinct from the slice-02 `PUBLIC_DATA_BANNER`, which is about
/// GitHub scraping; this one is about the network index.)
pub const SEARCH_PUBLIC_DATA_BANNER: &str = "Discovery indexes ONLY public, signed claims, \
verified before indexing. Nothing private is read or aggregated.";

/// Content-frozen no-merge guarantee for the network search views (I-AV-2 /
/// WD-103). Reuses the slice-03/04 ADR-013 phrasing so the anti-merging promise
/// reads identically across local + network views. Do NOT paraphrase.
pub const SEARCH_NO_MERGE_FOOTER: &str =
    "Each claim is attributed to its author DID. No claims are merged.";

/// Content-frozen `[verified]` marker (I-AV-1): every network result carries it
/// by construction (verified-before-index; there is no `[unverified]` state). Do
/// NOT paraphrase.
pub const VERIFIED_MARKER: &str = "[verified]";

/// Content-frozen honest-trail footer for the `--contributor` network view
/// (US-AV-003 / J-002). One developer's RAW trail — never a community consensus.
/// Do NOT paraphrase.
pub const SEARCH_CONTRIBUTOR_TRAIL_FOOTER: &str =
    "This is one developer's reasoning trail, not a community consensus.";

/// The relationship annotation appended to a per-author header in a network
/// view. Extends the slice-03 [`relationship_annotation`] with the slice-05
/// `NetworkUnfollowed` label `(not subscribed)` (US-AV-005) — an author present
/// in the network index the user does not follow. Content-frozen per ADR-013.
pub fn search_relationship_annotation(relationship: AuthorRelationship) -> &'static str {
    match relationship {
        AuthorRelationship::You => "(you)",
        AuthorRelationship::SubscribedPeer => "(subscribed peer)",
        AuthorRelationship::UnsubscribedCache => "(unsubscribed cache)",
        AuthorRelationship::NetworkUnfollowed => "(not subscribed)",
    }
}

/// Render the render-only `peer add` FOLLOW AFFORDANCE for an unfollowed network
/// author (US-AV-005 / WD-110 / I-AV-7). REUSES the slice-03 command VERBATIM —
/// it is a render string only; there is NO executable follow path and NO
/// auto-subscribe. The user copy-pastes it to follow the discovered author. PURE.
///
/// The affordance reuses the slice-03 `openlore peer add <did>` command verbatim
/// (no parallel subscription path, I-AV-7); the bare DID is used so the printed
/// command is the exact one the slice-03 verb accepts.
pub fn render_follow_affordance(author_did: &str) -> String {
    let bare = author_did.split('#').next().unwrap_or(author_did);
    format!("    Follow this author: openlore peer add {bare}\n")
}

/// Render the network search result: the FLAT attributed transport rows
/// re-grouped per author, each row carrying its `author_did` + numeric
/// confidence + display bucket + evidence + cid + the `[verified]` marker, under a
/// per-author header annotated with its relationship label; the unfollowed
/// authors get the `peer add` follow affordance; the footer states the distinct-
/// author count + the no-merge guarantee + the `peer add` pointer. PURE function —
/// no I/O.
///
/// The public-data banner is printed FIRST (KPI-AV-5 / I-AV-4 — before the first
/// result row). The relationship label is resolved CLI-side via `relationship_for`
/// (the index is per-user-neutral; the caller closes over the user's
/// `peer_subscriptions`). Two identical-(subject,object) rows by DIFFERENT authors
/// render as TWO rows (anti-merging, I-AV-2) — there is NO merged/consensus row.
pub fn render_network_search_result(
    dimension: SearchDimension,
    result: &NetworkSearchResultRaw,
    relationship_for: &dyn Fn(&str) -> AuthorRelationship,
) -> String {
    let mut out = String::new();
    // The public-data banner ALWAYS precedes the results (KPI-AV-5 / I-AV-4).
    out.push_str(&format!("{SEARCH_PUBLIC_DATA_BANNER}\n\n"));

    if result.results.is_empty() {
        // The verb routes an empty dimension result to `render_empty_network_search`
        // (it computes the near-match suggestion via `appview_domain`, AVC-8, and
        // knows the queried value) — so this branch is a defensive fallback that
        // names the dimension without a value. Pass the wire-supplied suggestion
        // (server-side `None` today) so the message degrades gracefully.
        out.push_str(&render_empty_network_result(
            dimension,
            "<unknown>",
            result.suggestion.as_deref(),
        ));
        return out;
    }

    // OD-AV-7 (shown-not-applied; I-AV-9): the counter/retract relationships
    // visible across the FLAT result set. Computed ONCE, up front, as a pure
    // projection of the rows' typed `references` — a countering claim K's `counters`
    // reference to a countered claim C's CID becomes a `countered-by <K.cid>
    // (by <K.author>)` annotation on C's row. The counter is SHOWN, never APPLIED:
    // no row is filtered, dropped, or down-weighted (mirrors slice-04 WD-85 / the
    // federated `counter_relationships` precedent).
    let counters = network_counter_relationships(&result.results);

    // Group the FLAT attributed rows per author (first-seen author order — stable,
    // hash-randomization-free). NEVER collapses two authors onto one row.
    for (author_did, rows) in &group_network_rows_by_author(&result.results) {
        let relationship = relationship_for(author_did);
        out.push_str(&format!(
            "author: {} {}\n",
            author_did,
            search_relationship_annotation(relationship)
        ));
        for row in rows {
            out.push_str(&render_one_network_row(row, &counters));
        }
        // The discovery→federation funnel affordance — ONLY for unfollowed authors
        // (a subscribed peer already followed; I-AV-7).
        if matches!(relationship, AuthorRelationship::NetworkUnfollowed) {
            out.push_str(&render_follow_affordance(author_did));
        }
        out.push('\n');
    }

    // The footer is dimension-specific: the OBJECT/SUBJECT survey states the
    // distinct-author COUNT + the no-merge guarantee; the CONTRIBUTOR trail frames
    // ONE developer's reasoning trail (NOT a community consensus) + the slice-03
    // `peer add <did>` follow offer naming the trail's author (KPI-AV-1 honesty).
    match dimension {
        SearchDimension::Contributor => {
            out.push_str(&render_contributor_network_trail_footer(&result.results));
        }
        SearchDimension::Object | SearchDimension::Subject => {
            out.push_str(&render_network_search_footer(
                distinct_network_author_count(&result.results),
            ));
        }
    }
    out
}

/// Render the CONTRIBUTOR-dimension network footer (AV-15 / US-AV-003 / KPI-AV-1):
/// the honest framing that this is ONE developer's reasoning trail — NOT a
/// community consensus — plus the slice-03 `openlore peer add <did>` follow offer
/// naming the trail's BARE author DID. A network trail is one author's reasoning,
/// never an aggregate the network endorses (the load-bearing honesty assertion).
/// Pure helper.
///
/// The trail is single-author by construction (a contributor query is `author_did
/// = ?`), so the author DID is read from the first attributed row; the follow offer
/// strips the app-identity fragment to the bare DID the slice-03 `peer add` accepts.
pub(crate) fn render_contributor_network_trail_footer(rows: &[NetworkResultRowRaw]) -> String {
    match rows.first() {
        Some(row) => {
            let bare = row
                .author_did
                .0
                .split('#')
                .next()
                .unwrap_or(&row.author_did.0);
            format!(
                "{CONTRIBUTOR_TRAIL_FOOTER} Follow this author with \
                 `openlore peer add {bare}`.\n"
            )
        }
        // Empty trails route through `render_empty_network_result` above; this is a
        // defensive fallback that still states the honest framing.
        None => format!("{CONTRIBUTOR_TRAIL_FOOTER}\n"),
    }
}

/// Render one FLAT attributed network row: the author DID, the numeric confidence
/// + display-only bucket, evidence, the cid, and the `[verified]` marker (every
/// row carries it by construction — verified-before-index, I-AV-1). Pure helper.
///
/// OD-AV-7 (shown-not-applied; I-AV-9): when this row is COUNTERED by another
/// claim K in the result set, its `countered-by <K.cid> (by <K.author>)`
/// annotation is appended at the end of the block. The annotation is per-row
/// METADATA derived from `counters`; it NEVER removes or down-weights the row —
/// the countered claim is SHOWN with its counter, never filtered.
pub(crate) fn render_one_network_row(
    row: &NetworkResultRowRaw,
    counters: &[NetworkCounterRelationship],
) -> String {
    let mut out = String::new();
    out.push_str(&format!("  author_did: {}\n", row.author_did.0));
    out.push_str(&format!("    subject:    {}\n", row.subject));
    out.push_str(&format!("    object:     {}\n", row.object));
    out.push_str(&format!(
        "    confidence: {} ({})\n",
        render_candidate_confidence(row.confidence),
        confidence_bucket_label(row.confidence)
    ));
    out.push_str(&format!(
        "    evidence:   {}\n",
        render_evidence(&row.evidence)
    ));
    out.push_str(&format!("    cid:        {}\n", row.cid.0));
    out.push_str(&format!("    {VERIFIED_MARKER}\n"));
    // OD-AV-7: the counter annotation(s) for THIS row (it is countered by K).
    for annotation in network_counter_annotations_for(&row.cid.0, counters) {
        out.push_str(&format!("    {annotation}\n"));
    }
    out
}

/// One counter/retract relationship visible in the network result set: a
/// countering claim K (`counter_cid`, authored by `counter_author`) that
/// `counters`/`retracts` a countered claim C (`countered_cid`). The countered
/// row's author is NOT captured here — the annotation lands on C's row and names
/// K (the counterer); C's own attribution is its row's `author_did` (anti-merging
/// preserved — this projection never separates a claim from its attribution).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NetworkCounterRelationship {
    counter_cid: String,
    counter_author: String,
    countered_cid: String,
}

/// Pure projection: every counter/retract relationship visible across the FLAT
/// network rows. A row K participates when it carries a typed `Counters`/`Retracts`
/// reference; the target C's CID is the reference's `cid`. K's author is K's own
/// `author_did` (the wire carried K's `references` + `author_did` — OD-AV-7). The
/// annotation lands on C by its CID, so C need not carry any reference itself.
/// Mirrors the federated `counter_relationships` precedent at network scale.
pub(crate) fn network_counter_relationships(
    rows: &[NetworkResultRowRaw],
) -> Vec<NetworkCounterRelationship> {
    let mut relationships = Vec::new();
    for row in rows {
        for reference in &row.references {
            if !matches!(
                reference.ref_type,
                claim_domain::ReferenceType::Counters | claim_domain::ReferenceType::Retracts
            ) {
                continue;
            }
            relationships.push(NetworkCounterRelationship {
                counter_cid: row.cid.0.clone(),
                counter_author: row.author_did.0.clone(),
                countered_cid: reference.cid.0.clone(),
            });
        }
    }
    relationships
}

/// The counter annotation line(s) for the row whose CID is `cid`: for every
/// relationship that COUNTERS this row, emit `countered-by <K.cid> (by <K.author>)`
/// — the counter SHOWN on the countered row (OD-AV-7 / I-AV-9). Pure helper over
/// the precomputed relationships; sorted by countering CID for deterministic output.
pub(crate) fn network_counter_annotations_for(
    cid: &str,
    counters: &[NetworkCounterRelationship],
) -> Vec<String> {
    let mut matching: Vec<&NetworkCounterRelationship> =
        counters.iter().filter(|r| r.countered_cid == cid).collect();
    matching.sort_by(|a, b| a.counter_cid.cmp(&b.counter_cid));
    matching
        .into_iter()
        .map(|rel| {
            format!(
                "countered-by {} (by {})",
                rel.counter_cid, rel.counter_author
            )
        })
        .collect()
}

/// Group FLAT attributed network rows by author DID, preserving first-seen author
/// order (stable output). Each row lands under ITS OWN `author_did` — two
/// identical-content rows by distinct authors land in DISTINCT groups (anti-
/// merging, I-AV-2). Pure helper.
pub(crate) fn group_network_rows_by_author(
    rows: &[NetworkResultRowRaw],
) -> Vec<(String, Vec<&NetworkResultRowRaw>)> {
    let mut order: Vec<String> = Vec::new();
    let mut grouped: Vec<(String, Vec<&NetworkResultRowRaw>)> = Vec::new();
    for row in rows {
        let did = row.author_did.0.clone();
        match order.iter().position(|d| d == &did) {
            Some(pos) => grouped[pos].1.push(row),
            None => {
                order.push(did.clone());
                grouped.push((did, vec![row]));
            }
        }
    }
    grouped
}

/// The count of distinct (full) author DIDs in a FLAT attributed result set — a
/// COUNT over attributed rows, NEVER a merge. Pure helper.
pub(crate) fn distinct_network_author_count(rows: &[NetworkResultRowRaw]) -> usize {
    let mut seen: std::collections::HashSet<&str> = std::collections::HashSet::new();
    for row in rows {
        seen.insert(row.author_did.0.as_str());
    }
    seen.len()
}

/// Render the network-search footer: the distinct-author count (a COUNT over
/// attributed rows; never a merge) + the content-frozen no-merge guarantee + the
/// `openlore peer add <did>` follow pointer. Pure helper.
pub(crate) fn render_network_search_footer(author_count: usize) -> String {
    format!(
        "{author_count} distinct author(s). {SEARCH_NO_MERGE_FOOTER} \
         Follow any author with `openlore peer add <did>`.\n"
    )
}

/// The dimension noun used in the empty-result message ("object" / "contributor"
/// / "subject") so the message names WHAT was searched. Pure helper.
pub(crate) fn search_dimension_noun(dimension: SearchDimension) -> &'static str {
    match dimension {
        SearchDimension::Object => "object",
        SearchDimension::Contributor => "contributor",
        SearchDimension::Subject => "subject",
    }
}

/// Render the empty-network-result message (US-AV-002 Ex 4): NAME the queried
/// dimension + value ("No network claims found for object <value>") and, when a
/// near-match `suggestion` was resolved, surface it ("Did you mean <near>?"). Exit
/// stays 0 (a valid empty result — the verb does not error, AV-12). Pure helper.
///
/// The `suggestion` is computed by the VERB (it ranks the known network objects
/// against the query via the pure `appview_domain::near_match_suggestion`, AVC-8)
/// and passed in — the renderer stays pure (no I/O). It is `None` when no
/// known value is close enough (the bare empty-result line, still exit 0).
pub(crate) fn render_empty_network_result(
    dimension: SearchDimension,
    value: &str,
    suggestion: Option<&str>,
) -> String {
    let noun = search_dimension_noun(dimension);
    match suggestion {
        Some(near) => {
            format!("No network claims found for {noun} {value}. Did you mean {near}?\n")
        }
        None => format!("No network claims found for {noun} {value}.\n"),
    }
}

/// Render the empty network-search view (US-AV-002 Ex 4 / AV-12): the public-data
/// banner FIRST (KPI-AV-5 / I-AV-4 — shown on EVERY search session, empty or not),
/// then the empty-result message NAMING the queried dimension + value, plus the
/// near-match "Did you mean <near>?" line when the verb resolved one. PURE
/// function — no I/O. The verb computes `suggestion` by ranking the known network
/// objects against the query (`appview_domain::near_match_suggestion`, AVC-8) and
/// keeps exit 0 (a valid not-yet-found state, NOT an error).
pub fn render_empty_network_search(
    dimension: SearchDimension,
    value: &str,
    suggestion: Option<&str>,
) -> String {
    let mut out = String::new();
    // The public-data banner ALWAYS precedes the result — even an empty one
    // (KPI-AV-5 / I-AV-4: every search session sets the indexing expectation).
    out.push_str(&format!("{SEARCH_PUBLIC_DATA_BANNER}\n\n"));
    out.push_str(&render_empty_network_result(dimension, value, suggestion));
    out
}

// -----------------------------------------------------------------------------
// `--hide-retracted` disclosure (feature `retraction-aware-search-filter`; ADR-060)
// -----------------------------------------------------------------------------

/// Content-frozen retraction-count noun (US-RF-001 / OD-RF-3). The honest unit is
/// EVENTS — one author self-retraction (an original + its same-author marker) reads
/// as `1 retracted claim(s) hidden`, NOT 2 (D-RF-D5). The disclosure lines below
/// are the SINGLE SOURCE for this wording so the CLI + the slice-02 viewer stay
/// byte-identical. Do NOT paraphrase — the exact phrasing is the user-visible
/// disclosure contract.
pub const RETRACTION_HIDDEN_COUNT_NOUN: &str = "retracted claim(s) hidden";

/// Content-frozen re-run guidance appended to EVERY `--hide-retracted` disclosure
/// (US-RF-001 / I-RF-3): the filter is non-destructive + reversible, so the surface
/// always names how to see the hidden claims again. Do NOT paraphrase.
pub const RETRACTION_RERUN_GUIDANCE: &str = "re-run without --hide-retracted";

/// Content-frozen empty-after-filter fragment (US-RF-001 / RF-6 / I-RF-3): when the
/// filter hid EVERY result, the guided buffer states the claims `were soft-retracted`
/// — an explicit "they exist but were withdrawn" state, never a bare "nothing exists
/// here". Do NOT paraphrase.
pub const RETRACTION_ALL_HIDDEN_FRAGMENT: &str = "were soft-retracted";

/// Render the honest retraction disclosure footer appended AFTER the survivor
/// results (US-RF-001 / I-RF-3): "N retracted claim(s) hidden" (N = retraction
/// EVENTS, D-RF-D5) + the re-run guidance. Emitted ONLY when `hidden_count >= 1`
/// (the verb suppresses it at `0` — no misleading line, D-4). PURE function.
pub fn render_retraction_disclosure(hidden_count: u32) -> String {
    format!(
        "\n{hidden_count} {RETRACTION_HIDDEN_COUNT_NOUN}. \
         To see them, {RETRACTION_RERUN_GUIDANCE}.\n"
    )
}

/// Render the guided empty-after-filter buffer (US-RF-001 / RF-6 / I-RF-3): when
/// `--hide-retracted` hid EVERY matching claim, the surface names that all
/// `hidden_count` results `were soft-retracted` by their authors + the re-run
/// guidance — an emotional-arc buffer, NOT a bare empty result that reads as
/// "nothing exists here". The public-data banner precedes it (I-AV-4, every search
/// session). PURE function.
pub fn render_all_retracted_buffer(hidden_count: u32) -> String {
    format!(
        "{SEARCH_PUBLIC_DATA_BANNER}\n\n\
         All {hidden_count} matching claim(s) {RETRACTION_ALL_HIDDEN_FRAGMENT} by their \
         authors and are hidden from this view ({hidden_count} {RETRACTION_HIDDEN_COUNT_NOUN}).\n\
         To see them, {RETRACTION_RERUN_GUIDANCE}.\n"
    )
}

/// Content-frozen `--show` signature-verified line prefix (US-AV-004 Ex1 /
/// KPI-AV-3): the inspected record's signature was VERIFIED against the author's
/// DID. The `<did>` is filled with the bare author DID. This renders the SAME
/// verification result the indexer computed at ingest (the row's
/// `verified_against`); `--show` does NOT re-verify (no second path; US-AV-004
/// Technical Notes). Do NOT paraphrase — the exact phrasing is the user-visible
/// trust contract.
pub const SHOW_SIGNATURE_VERIFIED_PREFIX: &str = "Signature: VERIFIED against ";

/// Content-frozen `--show` CID-match line suffix (US-AV-004 Ex1 / KPI-AV-3): the
/// inspected record's CID was recomputed and matches the published record. The
/// `CID: <cid>` is filled with the row's cid. This surfaces the cid the indexer
/// ALREADY computed + verified at ingest (no second path). Do NOT paraphrase.
pub const SHOW_CID_RECOMPUTED_SUFFIX: &str = " (recomputed, matches published record)";

/// Render the `--show <cid>` trust-inspection view: the full record (subject /
/// object / confidence / evidence / author DID) PLUS "Signature: VERIFIED against
/// <did>" + "CID: <cid> (recomputed, matches published record)". PURE function —
/// no I/O.
///
/// ## Same pure-core verification result, no second path (US-AV-004 Technical Notes)
///
/// The two verification lines render the verification result the indexer ALREADY
/// computed at INGEST — the row's `verified_against` (the key the signature
/// verified against, never empty per WD-104) and the row's `cid` (the CID the
/// indexer recomputed + matched before indexing, WD-104 verified-before-index).
/// `--show` does NOT re-verify nor re-sign; it reads the stored verified record
/// and surfaces those already-computed facts. The display is READ-ONLY — it
/// creates / signs / mutates nothing (US-AV-004; AV-23).
///
/// The `<did>` in the signature line is the BARE author DID (fragment stripped) so
/// it reads as the human-recognizable identity, matching the result-list rows.
pub fn render_show_verification_line(row: &NetworkResultRowRaw) -> String {
    let mut out = String::new();
    // The full record — the SAME fields the result-list row carries, surfaced in
    // full for the trust inspection (subject / object / confidence / evidence /
    // author DID). Read verbatim from the stored verified record.
    out.push_str(&format!("subject:     {}\n", row.subject));
    out.push_str(&format!("object:      {}\n", row.object));
    out.push_str(&format!(
        "confidence:  {} ({})\n",
        render_candidate_confidence(row.confidence),
        confidence_bucket_label(row.confidence)
    ));
    out.push_str(&format!(
        "evidence:    {}\n",
        render_evidence(&row.evidence)
    ));
    out.push_str(&format!("author:      {}\n", row.author_did.0));

    // The trust lines — rendered from the verification result the indexer computed
    // at ingest (the row's `verified_against` + `cid`). No re-verification here.
    let bare_did = row
        .verified_against
        .0
        .split('#')
        .next()
        .unwrap_or(&row.verified_against.0);
    out.push_str(&format!("{SHOW_SIGNATURE_VERIFIED_PREFIX}{bare_did}\n"));
    out.push_str(&format!("CID: {}{SHOW_CID_RECOMPUTED_SUFFIX}\n", row.cid.0));
    out
}

/// Content-frozen `--share` sharing-semantics line (US-AV-006 Ex1 / I-AV-8 /
/// KPI-AV-6): the shared link encodes the QUERY (dimension+value), so opening it
/// re-runs the search against the CURRENT index — it is NOT a frozen snapshot of
/// the results. Do NOT paraphrase — the exact phrasing is the user-visible
/// honesty contract (a stale snapshot would silently lose later-ingested claims).
pub const SHARE_QUERY_NOT_SNAPSHOT_SEMANTICS: &str =
    "This link encodes the query, not a frozen snapshot — opening it re-runs the \
search against the current index.";

/// The `<dimension>` token a `--share` link encodes for each search dimension —
/// the query-string KEY (`object` / `contributor` / `subject`). PURE helper; the
/// link is `openlore://search?<key>=<value>`, forward-compatible across all three
/// dimensions (AV-26 object today, AV-29 contributor next).
pub(crate) fn share_dimension_key(dimension: SearchDimension) -> &'static str {
    match dimension {
        SearchDimension::Object => "object",
        SearchDimension::Contributor => "contributor",
        SearchDimension::Subject => "subject",
    }
}

/// Emit the `--share` query-encoding link (WD-110 / I-AV-8): encodes ONLY the
/// dimension + value, NEVER a result snapshot. Opening it re-runs the query →
/// current per-author-attributed verified results. PURE function — no I/O.
///
/// Output is two lines:
///
/// ```text
/// Shareable link: openlore://search?<dimension>=<value>
/// This link encodes the query, not a frozen snapshot — opening it re-runs the search against the current index.
/// ```
///
/// The link carries EXACTLY one `<dimension>=<value>` query parameter — no result
/// payload, no author DID, no cid, no confidence, no `&`-joined snapshot fields —
/// so it encodes the QUERY only (the query-encoding-not-snapshot contract,
/// KPI-AV-6). The caller resolves the value to encode (e.g. the contributor
/// dimension passes the RESOLVED app-identity-bare DID, AV-29).
pub fn render_share_link(dimension: SearchDimension, value: &str) -> String {
    let key = share_dimension_key(dimension);
    format!(
        "Shareable link: openlore://search?{key}={value}\n\
         {SHARE_QUERY_NOT_SNAPSHOT_SEMANTICS}\n"
    )
}

#[cfg(test)]
mod retraction_disclosure_tests {
    //! Fast in-crate unit tier for the PURE `--hide-retracted` disclosure renderers
    //! (feature `retraction-aware-search-filter`; ADR-060). The subprocess acceptance
    //! suite (`tests/acceptance/search_hide_retracted.rs`) exercises the SAME strings
    //! end-to-end; these pin the content-frozen disclosure contract at the pure
    //! render-function boundary (the driving port at domain scope) so a whole-function
    //! mutation (`String::new()` / `"xyzzy"`) is caught in <1ms without spawning the CLI.
    use super::*;

    /// The honest disclosure footer (US-RF-001 / I-RF-3 / D-RF-D5): "N retracted
    /// claim(s) hidden" (N = self-retraction EVENTS, the honest unit) + the re-run
    /// guidance. Both fragments are content-frozen — the user-visible contract.
    #[test]
    fn disclosure_footer_states_the_event_count_and_rerun_guidance() {
        let footer = render_retraction_disclosure(2);
        assert!(
            footer.contains(&format!("2 {RETRACTION_HIDDEN_COUNT_NOUN}")),
            "footer discloses the hidden EVENT count + the frozen noun; got: {footer:?}"
        );
        assert!(
            footer.contains(RETRACTION_RERUN_GUIDANCE),
            "footer names how to re-run without the flag; got: {footer:?}"
        );
    }

    /// The guided empty-after-filter buffer (US-RF-001 / RF-6 / I-RF-3): when the
    /// filter hid EVERY result, the buffer names that all N results `were
    /// soft-retracted` + the count + the re-run guidance — never a bare empty result.
    #[test]
    fn all_retracted_buffer_names_the_withdrawn_state_and_count() {
        let buffer = render_all_retracted_buffer(3);
        assert!(
            buffer.contains("All 3 matching claim(s)"),
            "buffer opens with the guided 'All N matching claim(s)' framing; got: {buffer:?}"
        );
        assert!(
            buffer.contains(RETRACTION_ALL_HIDDEN_FRAGMENT),
            "buffer names the withdrawn ('were soft-retracted') state; got: {buffer:?}"
        );
        assert!(
            buffer.contains(&format!("3 {RETRACTION_HIDDEN_COUNT_NOUN}")),
            "buffer discloses the hidden EVENT count; got: {buffer:?}"
        );
        assert!(
            buffer.contains(RETRACTION_RERUN_GUIDANCE),
            "buffer names how to re-run without the flag; got: {buffer:?}"
        );
    }
}
