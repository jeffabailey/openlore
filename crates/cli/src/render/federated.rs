//! `openlore claim graph --federated` — attributed peer rows + counter annotations.

use super::*;

/// Render the `graph query --subject <S> --federated` result block: rows
/// from BOTH the user's own `claims` AND `peer_claims`, GROUPED BY author
/// DID. Pure function — no I/O, no storage access.
///
/// ## Anti-merging contract (I-FED-1 layer 3, behavioral — WD-30)
///
/// Each `FederatedRow` carries its `author_did` at the type level
/// (non-Option). This renderer surfaces that attribution per row and
/// NEVER collapses two authors' rows into one aggregate:
///
/// - Rows are grouped under a per-author header (first-seen author order),
///   one header per distinct DID. The header annotates the author's
///   relationship to the local user: `(you)` / `(subscribed peer)` /
///   `(unsubscribed cache)`.
/// - Every claim row prints `author_did`, `confidence`, and `cid` so an
///   operator can attribute any single row to exactly one author.
/// - The footer states the count of distinct authors AND the
///   content-frozen [`NO_MERGE_FOOTER_LITERAL`] (ADR-013). No row is ever
///   labeled "merged" / "consensus" / "aggregate".
pub fn render_federated_query_result(rows: &[FederatedRow]) -> String {
    let groups = group_by_author(rows);

    // FQ-5 (US-FED-003 AC #8): the bidirectional counter relationships over
    // the row set. Computed once, up front, as a pure projection of the
    // reference graph so each row's annotation is an O(1) lookup. The
    // annotation is per-row METADATA — it NEVER merges two rows.
    let counters = counter_relationships(rows);

    let mut out = String::new();
    for (author_did, relationship, author_rows) in &groups {
        out.push_str(&format!(
            "author: {} {}\n",
            author_did,
            relationship_annotation(*relationship)
        ));
        for (idx, row) in author_rows.iter().enumerate() {
            if idx > 0 {
                out.push('\n');
            }
            out.push_str(&render_one_federated_row(author_did, row, &counters));
        }
        out.push('\n');
    }

    // FQ-4 (US-FED-003 AC #7): when NO peer contributed a row, the federated
    // read has gracefully degraded to own-only output. Emit the content-frozen
    // zero-peers hint footer instead of the no-merge guarantee (which only
    // makes sense once two-or-more authors' rows could merge). The own rows
    // above are unchanged — degradation never swallows the local claims.
    if has_no_peer_rows(rows) {
        out.push_str(&render_no_peers_footer());
    } else {
        out.push_str(&render_federation_footer(groups.len()));
    }

    // FQ-5 summary line (US-FED-003 AC #8): state the count of counter
    // relationships explicitly so an operator sees the bidirectional links at
    // a glance. Omitted entirely when there are none (keeps the happy-path
    // FQ-1..4 output byte-stable).
    if !counters.is_empty() {
        out.push_str(&render_counter_relationship_summary(counters.len()));
    }
    out
}

// -----------------------------------------------------------------------------
// Slice-04 (ADR-020) — `graph query --object <philosophy>` dimension renderer
// -----------------------------------------------------------------------------

/// One bidirectional counter relationship discovered in the federated row
/// set: a `counter_cid` (authored by `counter_author`) that `counters` a
/// `target_cid` (authored by `target_author`). Both endpoints' authors are
/// captured so the renderer can draw BOTH arrows (forward + backward)
/// without ever separating a claim from its attribution (anti-merging).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CounterRelationship {
    counter_cid: String,
    counter_author: String,
    target_cid: String,
    target_author: String,
}

/// Pure projection: every counter relationship visible in `rows`. A row is a
/// counter when its claim carries a `ReferenceType::Counters` reference whose
/// target CID is ALSO present in the row set (so both endpoints are
/// attributable). Cross-subject / cross-store counters whose target is not in
/// this result are skipped — they cannot be annotated bidirectionally here
/// (the renderer is pure; it only knows the rows it was handed). The author
/// DIDs are taken from each endpoint row's `author_did` (already bare).
pub(crate) fn counter_relationships(rows: &[FederatedRow]) -> Vec<CounterRelationship> {
    use std::collections::HashMap;
    let author_by_cid: HashMap<&str, &str> = rows
        .iter()
        .map(|row| {
            (
                row.signed_claim.signature.signed_cid.0.as_str(),
                row.author_did.0.as_str(),
            )
        })
        .collect();

    let mut relationships = Vec::new();
    for row in rows {
        let counter_cid = row.signed_claim.signature.signed_cid.0.as_str();
        let counter_author = row.author_did.0.as_str();
        for reference in &row.signed_claim.unsigned.references {
            if !matches!(reference.ref_type, claim_domain::ReferenceType::Counters) {
                continue;
            }
            let target_cid = reference.cid.0.as_str();
            // Only annotate when the target is in the row set (both endpoints
            // attributable). Otherwise the backward arrow has no row to land on.
            if let Some(target_author) = author_by_cid.get(target_cid) {
                relationships.push(CounterRelationship {
                    counter_cid: counter_cid.to_string(),
                    counter_author: counter_author.to_string(),
                    target_cid: target_cid.to_string(),
                    target_author: (*target_author).to_string(),
                });
            }
        }
    }
    relationships
}

/// The annotation lines for one row given the full relationship set. A row
/// may be BOTH a counter (forward) AND countered (backward), so both arrow
/// kinds are emitted. Pure helper over the precomputed relationships.
pub(crate) fn counter_annotations_for(cid: &str, counters: &[CounterRelationship]) -> Vec<String> {
    let mut lines = Vec::new();
    // Forward: this row counters something.
    for rel in counters.iter().filter(|r| r.counter_cid == cid) {
        lines.push(format!(
            "counters {} by {}",
            rel.target_cid, rel.target_author
        ));
    }
    // Backward: this row is countered by something.
    for rel in counters.iter().filter(|r| r.target_cid == cid) {
        lines.push(format!(
            "countered-by {} by {}",
            rel.counter_cid, rel.counter_author
        ));
    }
    lines
}

/// Render the FQ-5 summary line stating the counter-relationship count.
/// Pluralized so a single relationship reads naturally ("1 counter
/// relationship"). Pure helper.
pub(crate) fn render_counter_relationship_summary(count: usize) -> String {
    let noun = if count == 1 {
        "counter relationship"
    } else {
        "counter relationships"
    };
    format!("{count} {noun}.\n")
}

/// Group federated rows by author DID, preserving first-seen author order
/// (so the local user's "(you)" block — typically the `Own` source — keeps
/// a stable position rather than hash-randomized). Returns one entry per
/// distinct DID carrying its `AuthorRelationship` and the rows attributed
/// to it. Pure helper.
pub(crate) fn group_by_author(
    rows: &[FederatedRow],
) -> Vec<(String, AuthorRelationship, Vec<&FederatedRow>)> {
    let mut order: Vec<String> = Vec::new();
    let mut grouped: Vec<(String, AuthorRelationship, Vec<&FederatedRow>)> = Vec::new();
    for row in rows {
        let did = row.author_did.0.clone();
        match order.iter().position(|d| d == &did) {
            Some(pos) => grouped[pos].2.push(row),
            None => {
                order.push(did.clone());
                grouped.push((did, row.author_relationship, vec![row]));
            }
        }
    }
    grouped
}

/// The human-readable relationship annotation appended to a per-author
/// header. Content-frozen per ADR-013 header convention.
///
/// `NetworkUnfollowed` is exclusively a slice-05 NETWORK-search concern (a
/// `FederatedRow` never carries it); for these LOCAL/federated views it maps to
/// the same `(not subscribed)` label as the slice-05 network renderer
/// ([`search_relationship_annotation`]) so the match stays total without a panic
/// (the variant is structurally unreachable here, but a label is safer than
/// `unreachable!`).
pub(crate) fn relationship_annotation(relationship: AuthorRelationship) -> &'static str {
    match relationship {
        AuthorRelationship::You => "(you)",
        AuthorRelationship::SubscribedPeer => "(subscribed peer)",
        AuthorRelationship::UnsubscribedCache => "(unsubscribed cache)",
        AuthorRelationship::NetworkUnfollowed => "(not subscribed)",
    }
}

/// Render one federated row. Reuses the slice-01 per-claim field block
/// (subject/predicate/object/evidence/confidence/author/composedAt/cid)
/// and additionally pins the row's `author_did` on its own line so every
/// row is independently attributable (anti-merging behavioral layer).
///
/// FQ-5 (US-FED-003 AC #8): when this row participates in a counter
/// relationship visible in the row set, its bidirectional annotation lines
/// (`counters <cid> by <did>` and/or `countered-by <cid> by <did>`) are
/// appended at the end of the block. The annotation is per-row metadata
/// derived from `counters`; it never merges rows.
///
/// FQ-7 (WD-42 — habit-bridging affordance, KPI-FED-3): every PEER row
/// (`source_table == SourceTable::Peer`) gets an inline copy-pasteable
/// counter template appended at the end of the block, shown BY DEFAULT.
/// OWN rows are excluded — you don't counter your own claim.
pub(crate) fn render_one_federated_row(
    author_did: &str,
    row: &FederatedRow,
    counters: &[CounterRelationship],
) -> String {
    let mut out = String::new();
    out.push_str(&format!("  author_did:  {author_did}\n"));
    for line in render_one_claim(&row.signed_claim).lines() {
        out.push_str(&format!("  {line}\n"));
    }
    for annotation in counter_annotations_for(&row.signed_claim.signature.signed_cid.0, counters) {
        out.push_str(&format!("  {annotation}\n"));
    }
    // FQ-7 / WD-42: peer rows carry the inline counter template (default-on).
    if matches!(row.source_table, SourceTable::Peer) {
        out.push_str(&format!("  {}\n", render_counter_template(row)));
    }
    out
}

/// Render the FQ-7 inline counter template for a peer row (WD-42). A single
/// copy-pasteable line: `openlore claim counter <peer_cid> --reason "..."`
/// pre-filled with the target claim's `--subject` / `--predicate` /
/// `--object`. The user fills in `--reason` / `--evidence` / `--confidence`
/// (the `"..."` reason placeholder and the omitted evidence/confidence flags
/// are the fill-in slots). Pure helper — the habit-bridging affordance that
/// turns "I see a peer claim I disagree with" into one keystroke-away action
/// (KPI-FED-3 friction reduction).
pub(crate) fn render_counter_template(row: &FederatedRow) -> String {
    let claim = &row.signed_claim.unsigned;
    format!(
        "openlore claim counter {} --reason \"...\" \
         --subject {} --predicate {} --object {} --evidence ... --confidence ...",
        row.signed_claim.signature.signed_cid.0, claim.subject, claim.predicate, claim.object,
    )
}

/// Render the federation footer: the distinct-author count plus the
/// content-frozen no-merge guarantee. Pure helper.
pub(crate) fn render_federation_footer(author_count: usize) -> String {
    format!("{author_count} author(s). {NO_MERGE_FOOTER_LITERAL}\n")
}

/// Render the zero-peers degraded-path footer (FQ-4 / US-FED-003 AC #7):
/// the content-frozen hint pointing the user at `peer add`. Pure helper.
pub(crate) fn render_no_peers_footer() -> String {
    format!("{NO_PEERS_FOOTER_LITERAL}\n")
}

/// `true` when NO row in a federated result came from the peer table —
/// i.e. zero peers contributed claims. Pure projection over the rows'
/// `source_table` attribution (the type-level anti-merging field). This is
/// the signal the renderer uses to switch from the no-merge footer to the
/// zero-peers degraded hint (FQ-4). An empty result counts as no-peers too.
pub(crate) fn has_no_peer_rows(rows: &[FederatedRow]) -> bool {
    !rows
        .iter()
        .any(|row| matches!(row.source_table, SourceTable::Peer))
}
