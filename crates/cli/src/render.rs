//! `render` — pure-function renderers for verb output blocks.
//!
//! Step 05-11 introduces `render_graph_query_result`: turn a list of
//! `SignedClaim` values (as returned by `StoragePort::query_by_subject`)
//! into a human-friendly stdout block. The renderer is pure (no I/O,
//! no clock, no storage access) so it can be unit-tested without spinning
//! up a wiring.
//!
//! ## Byte-for-byte invariant (KPI-4)
//!
//! The graph-query output prints every field VERBATIM from the SignedClaim
//! serde model — the same model the write path canonicalizes through. No
//! normalization happens here:
//!
//! - `confidence` is rendered as the original `f64` (e.g. `0.86`), NEVER
//!   as a bucket label like `well-evidenced`. Bucket labels are
//!   compose-time display only and MUST NOT appear here (WD-10 / D-12).
//! - `evidence` URLs are printed verbatim, one per line, no
//!   reformatting or scheme normalization.
//! - `composedAt` keeps the exact RFC3339 string the author composed
//!   with, including timezone marker (no conversion to local time).
//! - `author` is the full DID string with verification-method fragment.
//!
//! These invariants make round-trip identity (compose → sign → publish →
//! query) verifiable byte-for-byte at the CLI boundary, which is KPI-4's
//! zero-silent-normalization promise.
//!
//! ## Field label format
//!
//! Each claim renders as a labeled block:
//!
//! ```text
//! subject:     github:rust-lang/rust
//! predicate:   embodiesPhilosophy
//! object:      org.openlore.philosophy.memory-safety
//! evidence:    https://www.rust-lang.org/
//! confidence:  0.86
//! author:      did:plc:test-jeff#org.openlore.application
//! composedAt:  2026-05-25T12:00:00Z
//! cid:         <signed_cid>
//! ```
//!
//! When multiple claims match, blocks are separated by a blank line so
//! awk/grep/cut-style downstream tooling can split on `\n\n`.
//!
//! ## WS-15: retraction annotation (ADR-008 Behavioral rule 3 + WD-11)
//!
//! Per WD-11 "no hard-delete", a retracted claim is preserved verbatim
//! in both the local store and the PDS. The retraction is published as
//! a NEW counter-claim referencing the original. To make the retract
//! VISIBLE without mutating immutable history, the render layer
//! annotates the original claim with the literal string
//! `retracted by author` on its own line at the end of the block.
//!
//! The annotation is content-frozen UX (WD-11) — do NOT paraphrase. The
//! annotation list is computed by the verb via
//! `StoragePort::query_referencing` and passed alongside each claim so
//! the renderer stays pure (no I/O, no storage access).

use adapter_github::AuthReport;
use claim_domain::{Cid, SignedClaim};
use ports::{
    AttributedClaim, AuthorRelationship, CandidateClaim, FederatedRow, GraphEdge, GraphNode,
    SourceTable, TraversalResult,
};

// -----------------------------------------------------------------------------
// Slice-02 (github scraper) — public-data banner + candidate-list renderer
// -----------------------------------------------------------------------------

/// Content-frozen public-data-only banner (WD-51 / I-SCR-2; journey
/// `scrape-propose-sign.yaml` step 1 tui_mockup). Printed BEFORE any harvest
/// so the user is reassured no private data is read before any network beat
/// begins (emotional arc: skeptical -> reassured). Names BOTH "only PUBLIC
/// GitHub data is read" AND "Nothing published". Do NOT paraphrase.
pub const PUBLIC_DATA_BANNER: &str = "Only PUBLIC GitHub data is read. The target is the SUBJECT \
of any claim you may later sign — never a controller. Nothing published.";

/// Content-frozen candidate-list footer (journey step 2 tui_mockup): nothing
/// the scraper proposes is a claim until the human signs it (the human-gate,
/// I-SCR-1). Do NOT paraphrase — the exact string is the user-visible
/// reassurance contract.
pub const NOTHING_IS_A_CLAIM_FOOTER: &str =
    "Remember: nothing is a claim until you sign it. Select one to sign: `--sign N`.";

/// Render the public-data-only banner block (WD-51). Pure function — no I/O.
/// The verb prints this BEFORE invoking the harvest so the ordering AC
/// (SG-2: banner precedes harvest) holds structurally.
pub fn render_public_data_banner() -> String {
    format!("{PUBLIC_DATA_BANNER}\n")
}

/// Render the numbered candidate list (journey step 2 tui_mockup). Pure
/// function of the derived candidates — no I/O, no clock.
///
/// Each candidate renders as:
///
/// ```text
///  [1] embodiesPhilosophy  org.openlore.philosophy.dependency-pinning
///      from signal : Cargo.lock committed (exact pins)
///      confidence  : 0.25 (speculative)
/// ```
///
/// Every candidate NAMES its source signal(s) verbatim (auditability,
/// I-SCR-4 / KPI-SCR-3) and displays the conservative default confidence
/// `0.25` with the `speculative` bucket label (compose-time display only;
/// WD-10). Multiple contributing signals each get their own `from signal :`
/// line (US-SCR-002 Ex 4 collapse). A footer reassures nothing is a claim
/// until signed.
pub fn render_candidate_list(subject: &str, candidates: &[CandidateClaim]) -> String {
    let mut out = String::new();
    out.push_str(&format!("Candidate claims for subject {subject}\n"));
    out.push_str(&format!(
        "({} derived — NOTHING is signed or published; you choose)\n",
        candidates.len()
    ));
    for (idx, candidate) in candidates.iter().enumerate() {
        let number = idx + 1;
        out.push_str(&format!(
            " [{number}] {}  {}\n",
            candidate.predicate, candidate.object
        ));
        for signal in candidate.source_signals() {
            out.push_str(&format!("     from signal : {}\n", signal.value));
        }
        out.push_str(&format!(
            "     confidence  : {} ({})\n",
            render_candidate_confidence(candidate.confidence),
            confidence_bucket_label(candidate.confidence),
        ));
    }
    out.push_str(&format!("{NOTHING_IS_A_CLAIM_FOOTER}\n"));
    out
}

/// Render the auth-mode / rate-budget report line for a harvest (ADR-019 §5;
/// US-SCR-004; journey step 1 auth output). PURE function of the observed
/// [`AuthReport`] — no I/O.
///
/// - [`AuthReport::Authenticated`] => `authenticated (N/M rate budget)` so the
///   user sees the harvest ran on the higher PAT budget and how much remains.
/// - [`AuthReport::Anonymous`] => `unauthenticated` (no budget to report).
///
/// By construction this can NEVER echo a token value: an `AuthReport` carries
/// only the budget numbers, never the PAT bytes (no-token-leak; US-SCR-004).
pub fn render_auth_report(report: &AuthReport) -> String {
    match report {
        AuthReport::Authenticated { remaining, limit } => {
            format!("authenticated ({remaining}/{limit} rate budget)\n")
        }
        AuthReport::Anonymous => "unauthenticated\n".to_string(),
    }
}

/// Render a candidate's confidence as the minimal decimal matching the
/// original `f64` (e.g. `0.25`) via serde — never `{:.2}` (that would be
/// normalization). Mirrors the read-path `render_confidence` rule.
fn render_candidate_confidence(confidence: f64) -> String {
    serde_json::to_value(confidence)
        .map(|v| v.to_string())
        .unwrap_or_else(|_| "(unrenderable)".to_string())
}

/// The compose-time display bucket label for a confidence value (WD-10).
/// Slice-02 candidates are always the conservative `0.25` default
/// (speculative); the full bucket scale is a slice-01 concern. This label is
/// DISPLAY-ONLY — it never enters a signed payload (the signed claim records
/// the numeric `f64`).
fn confidence_bucket_label(confidence: f64) -> &'static str {
    if confidence < 0.4 {
        "speculative"
    } else if confidence < 0.7 {
        "weighted"
    } else if confidence < 0.9 {
        "well-evidenced"
    } else {
        "triangulated"
    }
}

/// Inherited slice-01 framing literal (I-7 / WD-6): a claim is asserted by
/// you, NOT as truth. Content-frozen; do NOT paraphrase.
pub const NOT_AS_TRUTH_LITERAL: &str = "not as truth";

/// Slice-03 content-frozen no-merge guarantee (ADR-013 footer convention).
/// Printed in the `graph query --federated` footer. Do NOT paraphrase —
/// the exact string is the KPI-FED-2 anti-merging user-visible contract.
pub const NO_MERGE_FOOTER_LITERAL: &str =
    "Each claim is attributed to its author DID. No claims are merged.";

/// Slice-03 content-frozen zero-peers degraded-path hint (US-FED-003 AC #7;
/// user-stories.md Example 2 + UAT scenario #4). Emitted as the
/// `graph query --federated` footer when ZERO peers contributed rows — the
/// federated read gracefully degrades to own-only output and points the
/// user at `peer add` so they know how to follow a peer's claim stream. Do
/// NOT paraphrase — the exact string is the user-visible contract.
pub const NO_PEERS_FOOTER_LITERAL: &str =
    "No peers subscribed. Use `openlore peer add <did>` to follow a peer's claim stream.";

/// Slice-03 content-frozen framing literal for counter-claims: a counter
/// NEVER overwrites its target — both coexist. Pinned by US-FED-004 AC;
/// do NOT paraphrase. The compose preview MUST carry it verbatim.
pub const COUNTER_COEXIST_LITERAL: &str = "counter-claims coexist, never overwrite";

/// Pure data shape the counter-claim compose preview renders. Mirrors the
/// fields the user composed plus the countered target + its author DID, so
/// the render layer stays decoupled from the canonical `UnsignedClaim`.
#[derive(Debug, Clone)]
pub struct ComposedCounterClaim {
    /// The countered target's CID.
    pub target_cid: String,
    /// The bare DID of the target's author (the "peer" being countered).
    pub target_author_did: String,
    /// The NFC-normalized free-text reason (WD-35) — shown verbatim.
    pub reason: String,
    /// The user's own author DID (composing the counter).
    pub author_did: String,
    /// RFC3339 UTC compose timestamp (ClockPort::now_utc()).
    pub composed_at: String,
}

/// Pure function: render the counter-claim compose preview. Three
/// load-bearing contracts (US-FED-004 AC):
///
/// 1. BOTH framing literals appear: the inherited [`NOT_AS_TRUTH_LITERAL`]
///    (I-7) AND the slice-03 [`COUNTER_COEXIST_LITERAL`].
/// 2. The countered target + its author are named on one line:
///    `counters: <target_cid> (by <peer_did>)`.
/// 3. The `--reason` text appears verbatim (NFC-normalized upstream),
///    word-wrapped at 78 columns so the preview stays terminal-friendly.
pub fn render_counter_compose_preview(counter: &ComposedCounterClaim) -> String {
    let mut out = String::new();
    // Framing line 1 — inherited "not as truth" (I-7).
    out.push_str(&format!(
        "Compose preview (a counter-claim is asserted by you, {NOT_AS_TRUTH_LITERAL})\n"
    ));
    // Framing line 2 — slice-03 "counter-claims coexist, never overwrite".
    out.push_str(&format!("  ({COUNTER_COEXIST_LITERAL})\n"));
    // The countered target + its peer author.
    out.push_str(&format!(
        "  counters: {} (by {})\n",
        counter.target_cid, counter.target_author_did
    ));
    out.push_str(&format!("  author:     {}\n", counter.author_did));
    out.push_str(&format!("  composedAt: {}\n", counter.composed_at));
    // The reason, wrapped at 78 cols, shown verbatim under a labeled block.
    out.push_str("  reason:\n");
    for line in wrap_at(&counter.reason, 78) {
        out.push_str(&format!("    {line}\n"));
    }
    out
}

/// Word-wrap `text` to at most `width` columns per line, breaking on ASCII
/// spaces. A single word longer than `width` is emitted on its own line
/// uncut (we never split inside a word — that could corrupt a URL or CID).
/// Pure helper; the reason text is shown verbatim, only line-broken.
fn wrap_at(text: &str, width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current = String::new();
    for word in text.split(' ') {
        if current.is_empty() {
            current.push_str(word);
        } else if current.chars().count() + 1 + word.chars().count() <= width {
            current.push(' ');
            current.push_str(word);
        } else {
            lines.push(std::mem::take(&mut current));
            current.push_str(word);
        }
    }
    if !current.is_empty() || lines.is_empty() {
        lines.push(current);
    }
    lines
}

/// One claim plus the set of CIDs that reference it back-pointer-style.
/// Built by the verb (graph_query) from
/// `StoragePort::query_referencing(claim.signature.signed_cid)` and
/// passed to the renderer so the render layer stays pure.
///
/// The renderer only inspects the boolean `is_retracted` projection —
/// the full reference list lives in the verb in case future slices need
/// finer-grained annotations (e.g. "corrected by ...", "superseded
/// by ..."). Carrying the bool keeps the render-time decision a
/// constant-time check.
#[derive(Debug, Clone)]
pub struct AnnotatedClaim {
    pub claim: SignedClaim,
    /// `true` if any other local claim back-references this CID with
    /// `ReferenceType::Retracts`. Drives the `retracted by author`
    /// annotation per ADR-008 Behavioral rule 3.
    pub is_retracted: bool,
}

/// Render a slice of `SignedClaim` values into the graph-query stdout
/// block. Pure function — no I/O, no clock access.
///
/// Back-compat entry point for callers that don't carry annotation
/// data. Equivalent to passing all claims with `is_retracted = false`.
/// Production callers use [`render_annotated_graph_query_result`] so
/// the WS-15 annotation appears.
pub fn render_graph_query_result(claims: &[SignedClaim]) -> String {
    let annotated: Vec<AnnotatedClaim> = claims
        .iter()
        .cloned()
        .map(|claim| AnnotatedClaim {
            claim,
            is_retracted: false,
        })
        .collect();
    render_annotated_graph_query_result(&annotated)
}

/// Render a slice of `AnnotatedClaim` values into the graph-query
/// stdout block. Pure function — no I/O, no clock access. The
/// annotation decision is precomputed by the verb (see
/// [`AnnotatedClaim::is_retracted`]).
pub fn render_annotated_graph_query_result(annotated: &[AnnotatedClaim]) -> String {
    let mut out = String::new();
    for (idx, ann) in annotated.iter().enumerate() {
        if idx > 0 {
            out.push('\n');
        }
        out.push_str(&render_one_claim(&ann.claim));
        if ann.is_retracted {
            // Content-frozen per WD-11 — exact string is the contract.
            out.push_str("retracted by author\n");
        }
    }
    out
}

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

/// Slice-04 content-frozen no-merge guarantee for the `--object` dimension
/// view (US-GRAPH-001 / KPI-GRAPH-2; component-boundaries.md §"Render contract
/// (cli)"). Reuses the slice-03 ADR-013 phrasing so the anti-merging promise
/// reads identically across the federated subject view and the object view.
/// Do NOT paraphrase — the exact string is the user-visible contract.
pub const OBJECT_QUERY_NO_MERGE_FOOTER: &str =
    "Each claim is attributed to its author DID. No claims are merged.";

/// Generate the single-edit-distance neighbours of `object` confined to the
/// philosophy-URI character class (`[a-z0-9.-]`). A typo'd philosophy URI is
/// almost always one edit away from the correct one (a transposed / dropped /
/// doubled / swapped character), so the near-miss suggestion engine (GQE-4 /
/// US-GRAPH-001 Example 4) probes these candidates against the store and
/// proposes the first that has claims — "the closest existing object string"
/// (data-models.md §object near-miss suggestion). Pure function — no I/O.
///
/// Returns candidates in deterministic, dedup'd order so the verb's probe loop
/// (and any property test) is reproducible. The original `object` is never
/// emitted as its own neighbour (it already came back empty).
pub fn single_edit_neighbours(object: &str) -> Vec<String> {
    const ALPHABET: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789.-";
    let chars: Vec<char> = object.chars().collect();
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut out: Vec<String> = Vec::new();
    let mut push = |candidate: String, seen: &mut std::collections::HashSet<String>| {
        if candidate != object && seen.insert(candidate.clone()) {
            out.push(candidate);
        }
    };

    // Transpositions (adjacent swap) — the commonest typo class; probe first.
    for i in 0..chars.len().saturating_sub(1) {
        let mut next = chars.clone();
        next.swap(i, i + 1);
        push(next.into_iter().collect(), &mut seen);
    }
    // Substitutions.
    for i in 0..chars.len() {
        for &byte in ALPHABET {
            let mut next = chars.clone();
            next[i] = byte as char;
            push(next.into_iter().collect(), &mut seen);
        }
    }
    // Deletions (a doubled / extra character).
    for i in 0..chars.len() {
        let mut next = chars.clone();
        next.remove(i);
        push(next.into_iter().collect(), &mut seen);
    }
    // Insertions (a dropped character).
    for i in 0..=chars.len() {
        for &byte in ALPHABET {
            let mut next = chars.clone();
            next.insert(i, byte as char);
            push(next.into_iter().collect(), &mut seen);
        }
    }
    out
}

/// Slice-04 content-frozen empty-`--object` explainer prefix (US-GRAPH-001
/// Example 4 / UAT scenario 4). Names the queried object so the message is
/// self-explanatory. Followed by an optional near-match suggestion. Do NOT
/// paraphrase — the exact phrasing is the user-visible contract.
fn render_no_claims_for_object(object: &str, suggestion: Option<&str>) -> String {
    match suggestion {
        Some(near) => format!("No claims found for object {object}. Did you mean {near}?\n"),
        None => format!("No claims found for object {object}.\n"),
    }
}

/// Render the `graph query --object <philosophy>` dimension result: the
/// attributed per-claim rows GROUPED BY SUBJECT (project), each row carrying
/// its `author_did` + numeric confidence + display-only bucket + cid. Pure
/// function — no I/O, no storage access.
///
/// ## Anti-merging contract (I-GRAPH-2 / WD-73; US-GRAPH-001)
///
/// Each [`AttributedClaim`] carries its `author_did` at the type level
/// (non-`Option`). This renderer surfaces that attribution per row and NEVER
/// collapses two authors' claims about the same `(subject, object)` into one
/// aggregate:
///
/// - Rows are grouped under a per-subject header (first-seen subject order).
/// - Every claim row prints `author_did` (annotated with its relationship —
///   `(you)` / `(subscribed peer)` / `(unsubscribed cache)`), the numeric
///   `confidence`, its DISPLAY-ONLY bucket label, and the `cid` — so an
///   operator can attribute any single row to exactly one author.
/// - Two claims with identical `(subject, object)` by DIFFERENT authors render
///   as TWO rows (never merged).
/// - The footer states the distinct-SUBJECT count AND the distinct-AUTHOR
///   count AND the content-frozen [`OBJECT_QUERY_NO_MERGE_FOOTER`].
///
/// The `suggestion` argument carries the near-match the verb resolved by
/// probing the store (GQE-4 / US-GRAPH-001 Example 4) when the dimension read
/// came back empty. It is `None` when claims were found (the happy path) or when
/// no near-match exists.
pub fn render_object_query_grouped_by_subject(
    object: &str,
    claims: &[AttributedClaim],
    suggestion: Option<&str>,
) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "Claims embodying {object} (grouped by subject):\n\n"
    ));

    if claims.is_empty() {
        // Empty is HONEST (US-GRAPH-001 Example 4): name the queried object and,
        // if the store holds a near-match, suggest it. No per-claim row is
        // manufactured; exit code stays 0 (a valid not-yet-found state).
        out.push_str(&render_no_claims_for_object(object, suggestion));
        return out;
    }

    for (subject, subject_claims) in &group_by_subject(claims) {
        out.push_str(&format!("subject: {subject}\n"));
        for claim in subject_claims {
            out.push_str(&render_one_attributed_claim(claim));
        }
        out.push('\n');
    }

    out.push_str(&render_object_query_footer(
        distinct_subject_count(claims),
        distinct_author_count(claims),
    ));
    out
}

/// Group attributed claims by subject, preserving first-seen subject order
/// (stable, hash-randomization-free output). Returns one entry per distinct
/// subject carrying its claims. Pure helper.
fn group_by_subject<'a>(claims: &'a [AttributedClaim]) -> Vec<(String, Vec<&'a AttributedClaim>)> {
    let mut order: Vec<String> = Vec::new();
    let mut grouped: Vec<(String, Vec<&'a AttributedClaim>)> = Vec::new();
    for claim in claims {
        match order.iter().position(|s| s == &claim.subject) {
            Some(pos) => grouped[pos].1.push(claim),
            None => {
                order.push(claim.subject.clone());
                grouped.push((claim.subject.clone(), vec![claim]));
            }
        }
    }
    grouped
}

/// Render one attributed claim row under its subject group: the author DID
/// (with its relationship annotation), the numeric confidence + display-only
/// bucket, and the cid. Every value is independently attributable (anti-merging
/// behavioral layer). Pure helper.
fn render_one_attributed_claim(claim: &AttributedClaim) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "  author_did: {} {}\n",
        claim.author_did.0,
        relationship_annotation(claim.relationship)
    ));
    out.push_str(&format!(
        "    confidence: {} ({})\n",
        render_candidate_confidence(claim.confidence),
        confidence_bucket_label(claim.confidence)
    ));
    out.push_str(&format!("    cid:        {}\n", claim.cid.0));
    out
}

/// The count of distinct subjects in an attributed result set. Pure helper.
fn distinct_subject_count(claims: &[AttributedClaim]) -> usize {
    let mut seen: std::collections::HashSet<&str> = std::collections::HashSet::new();
    for claim in claims {
        seen.insert(claim.subject.as_str());
    }
    seen.len()
}

/// The count of distinct (bare) author DIDs in an attributed result set. Pure
/// helper.
fn distinct_author_count(claims: &[AttributedClaim]) -> usize {
    let mut seen: std::collections::HashSet<&str> = std::collections::HashSet::new();
    for claim in claims {
        seen.insert(claim.author_did.0.as_str());
    }
    seen.len()
}

/// Render the `--object` dimension footer: the distinct-subject count AND the
/// distinct-author count AND the content-frozen no-merge guarantee
/// (US-GRAPH-001). Pure helper.
fn render_object_query_footer(subject_count: usize, author_count: usize) -> String {
    format!(
        "{subject_count} subject(s), {author_count} author(s). {OBJECT_QUERY_NO_MERGE_FOOTER}\n"
    )
}

// -----------------------------------------------------------------------------
// Slice-04 (ADR-020) — `graph query --contributor <did>` dimension renderer
// -----------------------------------------------------------------------------

/// Slice-04 content-frozen honest-framing footer for the `--contributor`
/// dimension view (US-GRAPH-002 / J-002 "published reasoning trail, not
/// surveillance"). The contributor view shows ONE developer's RAW trail — each
/// claim's compose-time confidence verbatim, NEVER an aggregate/community
/// score. Do NOT paraphrase — the exact string is the user-visible contract.
pub const CONTRIBUTOR_TRAIL_FOOTER: &str =
    "This is one developer's reasoning trail, not a community consensus.";

/// Slice-04 content-frozen graceful-degrade hint for the `--contributor`
/// dimension view when the queried DID has NO local claims (not subscribed /
/// not pulled). US-GRAPH-002 Example 3 / UAT scenario 3: an absent contributor
/// is a valid empty result (exit 0), NOT an error — the view degrades to a
/// no-local-claims message plus a subscribe/pull next step pointing at
/// `openlore peer add` + `openlore peer pull` so the user can populate that
/// contributor's trail (J-002 anxiety mitigation; slice-03 `peer add`/`pull`
/// hint precedent). `{contributor}` is filled with the queried DID. Do NOT
/// paraphrase — the exact phrasing is the user-visible contract.
const CONTRIBUTOR_ABSENT_HINT_TEMPLATE: &str =
    "No local claims authored by {contributor}. Subscribe and pull with `openlore peer add` + `openlore peer pull`.";

/// Render the `graph query --contributor <did>` dimension result: every claim
/// that DID authored, across all subjects, listed under the contributor's DID
/// with subject/object/confidence/cid. Pure function — no I/O, no storage
/// access.
///
/// ## Honest-trail contract (US-GRAPH-002 / J-002; anti-merging WD-73)
///
/// Each [`AttributedClaim`] carries its `author_did` at the type level
/// (non-`Option`). For a contributor query every row is by the SAME author, so
/// the listing reads as that one developer's published reasoning trail:
///
/// - A header names the contributor DID, annotated with its relationship to the
///   local user (`(you)` for a self-review / `(subscribed peer)` /
///   `(unsubscribed cache)`).
/// - Every claim row prints `subject`, `object`, the numeric `confidence` shown
///   HONESTLY (the raw compose-time `f64` + its display-only bucket — NOT a
///   manufactured aggregate weight), and the `cid` — so every claim in the
///   trail is independently attributable to exactly one signed claim.
/// - The footer states the claim count AND the content-frozen
///   [`CONTRIBUTOR_TRAIL_FOOTER`] ("one developer's reasoning trail, not a
///   community consensus") so the view never reads as community endorsement.
///
/// ## Empty branch (GQE-8 / US-GRAPH-002 Example 3 — absent contributor)
///
/// When `claims` is empty the queried DID has NO local claims (the user has
/// not subscribed to / pulled that contributor). This is a VALID empty result,
/// not an error — the renderer degrades GRACEFULLY to a no-local-claims message
/// naming the queried DID plus the content-frozen subscribe/pull hint
/// ([`CONTRIBUTOR_ABSENT_HINT_TEMPLATE`]) pointing at `openlore peer add` +
/// `openlore peer pull`. The honest-trail footer is omitted — there is no trail
/// to frame (J-002 anxiety mitigation: sparse renders sparse, with a next step).
/// The verb keeps exit 0.
pub fn render_contributor_query_trail(contributor: &str, claims: &[AttributedClaim]) -> String {
    // GQE-8: an absent contributor (no local claims) degrades to the
    // no-local-claims message + subscribe/pull hint. Branch on the data shape
    // (empty vs found) so the found-trail path below stays uncluttered.
    if claims.is_empty() {
        return render_contributor_absent_hint(contributor);
    }

    let mut out = String::new();
    out.push_str(&format!(
        "Reasoning trail for contributor {contributor}:\n\n"
    ));

    // The contributor's relationship to the local user is uniform across the
    // trail (every row is by the same author). Read it from the first row so the
    // header annotation (you / subscribed peer / unsubscribed cache) is honest.
    if let Some(first) = claims.first() {
        out.push_str(&format!(
            "author_did: {} {}\n",
            first.author_did.0,
            relationship_annotation(first.relationship)
        ));
    }

    for claim in claims {
        out.push_str(&render_one_contributor_claim(claim));
    }
    out.push('\n');

    out.push_str(&render_contributor_trail_footer(claims.len()));
    out
}

/// Render one claim row of a contributor's trail: its subject, object, the
/// numeric confidence + display-only bucket (shown honestly — the raw
/// compose-time value, never an aggregate), and the cid. Every row is
/// independently attributable to one signed claim (anti-merging behavioral
/// layer). Pure helper.
fn render_one_contributor_claim(claim: &AttributedClaim) -> String {
    let mut out = String::new();
    out.push_str(&format!("  subject:    {}\n", claim.subject));
    out.push_str(&format!("  object:     {}\n", claim.object));
    out.push_str(&format!(
        "  confidence: {} ({})\n",
        render_candidate_confidence(claim.confidence),
        confidence_bucket_label(claim.confidence)
    ));
    out.push_str(&format!("  cid:        {}\n", claim.cid.0));
    out
}

/// Render the `--contributor` dimension footer: the claim count plus the
/// content-frozen honest-trail framing (US-GRAPH-002). Pure helper.
fn render_contributor_trail_footer(claim_count: usize) -> String {
    format!("{claim_count} claim(s). {CONTRIBUTOR_TRAIL_FOOTER}\n")
}

/// Render the absent-contributor graceful-degrade hint (GQE-8 / US-GRAPH-002
/// Example 3): the queried DID has no local claims, so emit the content-frozen
/// [`CONTRIBUTOR_ABSENT_HINT_TEMPLATE`] with `{contributor}` filled in. No
/// per-claim row, no honest-trail footer — there is no trail to frame, only the
/// subscribe/pull next step. Pure helper.
fn render_contributor_absent_hint(contributor: &str) -> String {
    format!(
        "{}\n",
        CONTRIBUTOR_ABSENT_HINT_TEMPLATE.replace("{contributor}", contributor)
    )
}

// -----------------------------------------------------------------------------
// Slice-04 (ADR-020 / US-GRAPH-004) — `graph query --traverse` tree renderer
// -----------------------------------------------------------------------------

/// Content-frozen Gate-5 honesty notice for the `--traverse` view: every
/// displayed edge maps to exactly one signed claim; the recursive CTE selects
/// FROM existing rows only and never fabricates or interpolates an edge
/// (I-GRAPH-5). Do NOT paraphrase — the exact string is the user-visible
/// contract (US-GRAPH-004 Example 1 / Gate 5).
pub const TRAVERSAL_INVENTS_NO_EDGES_NOTICE: &str = "Traversal does not invent edges.";

/// Content-frozen line the renderer emits when a traversal surfaces NO
/// connecting (cross-project) edges within the bound (GQE-21 / US-GRAPH-004
/// Example 2). Emitted in two honest cases: (a) the seed reaches zero edges at
/// all, and (b) the seed reaches edges but NO contributor's claims span more
/// than one project (a lone author on a lone project triangulates with
/// nothing — there is no non-obvious connection to surface). The `{depth}`
/// placeholder is filled with the bound so the honest "nothing found, nothing
/// fabricated" message names the depth searched. Do NOT paraphrase.
const TRAVERSAL_NO_EDGES_TEMPLATE: &str = "No connecting edges found at depth {depth}.";

/// Render the `graph query --object <philosophy> --traverse` result: a bounded,
/// cycle-safe tree from the queried philosophy to the projects that embody it
/// and the contributors who authored those claims, plus a "Connections found"
/// callout naming any contributor whose claims span MORE THAN ONE project (the
/// non-obvious cross-project connection — KPI-GRAPH-1). Pure function — no I/O,
/// no storage access.
///
/// ## Gate 5 / I-GRAPH-5 (traversal invents no edges)
///
/// Every [`GraphEdge`] in `result.edges` carries its backing `claim_cid`
/// (non-`Option`) and `author_did` (non-`Option`). The renderer surfaces BOTH
/// on each edge row, so an operator can trace any displayed edge to exactly one
/// signed claim, and emits the content-frozen
/// [`TRAVERSAL_INVENTS_NO_EDGES_NOTICE`]. Two claims about the same project by
/// different authors render as TWO edge rows (never merged; anti-merging WD-73).
///
/// `max_depth` is the bound the walk used (WD-76); it labels the
/// no-edges-found message when the seed is isolated and frames the
/// omitted-edge report.
pub fn render_traversal_tree(object: &str, result: &TraversalResult, max_depth: u8) -> String {
    render_traversal_from_seed(&format!("philosophy: {object}"), result, max_depth)
}

/// Render the `graph query --contributor <did> --traverse` result (GQE-22 /
/// US-GRAPH-004 Example 3): the SAME bounded, cycle-safe tree as
/// [`render_traversal_tree`], but seeded at a contributor rather than a
/// philosophy. The walk anchors on the contributor's own claims (depth 1) and
/// fans across the projects those claims share with other contributors; the
/// header names the seed contributor. The WD-76 omitted-edge report
/// ("Showing depth N; M edge(s) omitted. Use `--depth N+1` to go deeper.") and
/// the content-frozen Gate-5 notice render identically. Pure function.
pub fn render_traversal_contributor_tree(
    contributor: &str,
    result: &TraversalResult,
    max_depth: u8,
) -> String {
    render_traversal_from_seed(&format!("contributor: {contributor}"), result, max_depth)
}

/// Shared traversal-tree renderer core, parameterized by the seed `header` line
/// (a philosophy seed renders `philosophy: <object>`, a contributor seed
/// `contributor: <did>`). The tree body, the WD-76 omitted-edge report, the
/// KPI-GRAPH-1 "Connections found" callout, and the content-frozen Gate-5
/// honesty notice are identical regardless of seed. Pure helper.
fn render_traversal_from_seed(header: &str, result: &TraversalResult, max_depth: u8) -> String {
    let mut out = String::new();
    out.push_str(&format!("{header}\n\n"));

    if result.edges.is_empty() {
        // Honest empty (GQE-21): name the depth searched; fabricate no
        // connection. The Gate-5 notice still frames the (empty) result.
        out.push_str(&format!(
            "{}\n\n",
            TRAVERSAL_NO_EDGES_TEMPLATE.replace("{depth}", &max_depth.to_string())
        ));
        out.push_str(&format!("{TRAVERSAL_INVENTS_NO_EDGES_NOTICE}\n"));
        return out;
    }

    // The tree, grouped by project (first-seen order — stable output). Each
    // project heads a group; under it, one edge row per backing signed claim
    // carrying its author DID + claim_cid (every edge independently auditable).
    for (subject, project_edges) in &group_edges_by_project(&result.edges) {
        out.push_str(&format!("  project: {subject}\n"));
        for edge in project_edges {
            out.push_str(&render_one_traversal_edge(edge));
        }
        out.push('\n');
    }

    // WD-76: if the bound omitted deeper edges, report how many + how to widen.
    if result.omitted_edge_count > 0 {
        out.push_str(&format!(
            "Showing depth {max_depth}; {} edge(s) omitted. Use `--depth {}` to go deeper.\n\n",
            result.omitted_edge_count,
            max_depth.saturating_add(1)
        ));
    }

    // KPI-GRAPH-1: the non-obvious connection — any contributor whose claims
    // span MORE THAN ONE project. The callout names each such contributor and
    // the exact projects they triangulate across. When NO contributor spans
    // multiple projects (e.g. a lone author on a lone project — GQE-21), there
    // is no non-obvious connection to surface: state that HONESTLY rather than
    // silently omitting the callout, and fabricate nothing (Gate 5 / I-GRAPH-5).
    let callout = render_connections_callout(&result.edges);
    if callout.is_empty() {
        out.push_str(&format!(
            "{}\n\n",
            TRAVERSAL_NO_EDGES_TEMPLATE.replace("{depth}", &max_depth.to_string())
        ));
    } else {
        out.push_str(&callout);
    }

    // Gate 5: the content-frozen honesty notice.
    out.push_str(&format!("{TRAVERSAL_INVENTS_NO_EDGES_NOTICE}\n"));
    out
}

/// Group traversal edges by their project (`to` node subject), preserving
/// first-seen order. Pure helper.
fn group_edges_by_project(edges: &[GraphEdge]) -> Vec<(String, Vec<&GraphEdge>)> {
    let mut order: Vec<String> = Vec::new();
    let mut grouped: Vec<(String, Vec<&GraphEdge>)> = Vec::new();
    for edge in edges {
        let subject = project_subject(edge);
        match order.iter().position(|s| s == &subject) {
            Some(pos) => grouped[pos].1.push(edge),
            None => {
                order.push(subject.clone());
                grouped.push((subject, vec![edge]));
            }
        }
    }
    grouped
}

/// Render one traversal edge under its project group: the contributor (author
/// DID) who authored the backing claim, and the backing `claim_cid` (Gate 5 —
/// every edge maps to exactly one signed claim). Pure helper.
fn render_one_traversal_edge(edge: &GraphEdge) -> String {
    let mut out = String::new();
    out.push_str(&format!("    author_did: {}\n", edge.author_did.0));
    out.push_str(&format!("    claim_cid:  {}\n", edge.claim_cid.0));
    out
}

/// Render the "Connections found" callout (KPI-GRAPH-1 north star): for every
/// contributor whose edges span MORE THAN ONE distinct project, emit a line
/// naming the contributor and the projects they triangulate across — the
/// non-obvious cross-project connection the traversal surfaces. When no
/// contributor spans multiple projects, the callout is omitted entirely (there
/// is no non-obvious connection to surface). Pure helper.
fn render_connections_callout(edges: &[GraphEdge]) -> String {
    let spanning = contributors_spanning_multiple_projects(edges);
    if spanning.is_empty() {
        return String::new();
    }
    let mut out = String::new();
    out.push_str("Connections found:\n");
    for (author_did, projects) in spanning {
        out.push_str(&format!(
            "  {author_did} spans {} of these projects ({}) -> a contributor whose \
             dependency-pinning claims triangulate across projects.\n",
            projects.len(),
            projects.join(", ")
        ));
    }
    out.push('\n');
    out
}

/// The contributors whose traversal edges span MORE THAN ONE distinct project,
/// each paired with the projects (first-seen order) they span. The
/// cross-project connection KPI-GRAPH-1 surfaces. Pure helper.
fn contributors_spanning_multiple_projects(edges: &[GraphEdge]) -> Vec<(String, Vec<String>)> {
    let mut order: Vec<String> = Vec::new();
    let mut by_author: Vec<(String, Vec<String>)> = Vec::new();
    for edge in edges {
        let author = edge.author_did.0.clone();
        let subject = project_subject(edge);
        let pos = match order.iter().position(|a| a == &author) {
            Some(pos) => pos,
            None => {
                order.push(author.clone());
                by_author.push((author, Vec::new()));
                by_author.len() - 1
            }
        };
        if !by_author[pos].1.contains(&subject) {
            by_author[pos].1.push(subject);
        }
    }
    by_author
        .into_iter()
        .filter(|(_, projects)| projects.len() > 1)
        .collect()
}

/// The project subject an edge points at (its `to` node). Defensive fallback to
/// the empty string for a non-project `to` node (the traversal only emits
/// philosophy→project edges, but the match stays total). Pure helper.
fn project_subject(edge: &GraphEdge) -> String {
    match &edge.to {
        GraphNode::Project { subject } => subject.clone(),
        GraphNode::Philosophy { object } => object.clone(),
        GraphNode::Contributor { author_did } => author_did.0.clone(),
    }
}

/// One bidirectional counter relationship discovered in the federated row
/// set: a `counter_cid` (authored by `counter_author`) that `counters` a
/// `target_cid` (authored by `target_author`). Both endpoints' authors are
/// captured so the renderer can draw BOTH arrows (forward + backward)
/// without ever separating a claim from its attribution (anti-merging).
#[derive(Debug, Clone, PartialEq, Eq)]
struct CounterRelationship {
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
fn counter_relationships(rows: &[FederatedRow]) -> Vec<CounterRelationship> {
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
fn counter_annotations_for(cid: &str, counters: &[CounterRelationship]) -> Vec<String> {
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
fn render_counter_relationship_summary(count: usize) -> String {
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
fn group_by_author(rows: &[FederatedRow]) -> Vec<(String, AuthorRelationship, Vec<&FederatedRow>)> {
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
fn relationship_annotation(relationship: AuthorRelationship) -> &'static str {
    match relationship {
        AuthorRelationship::You => "(you)",
        AuthorRelationship::SubscribedPeer => "(subscribed peer)",
        AuthorRelationship::UnsubscribedCache => "(unsubscribed cache)",
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
fn render_one_federated_row(
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
fn render_counter_template(row: &FederatedRow) -> String {
    let claim = &row.signed_claim.unsigned;
    format!(
        "openlore claim counter {} --reason \"...\" \
         --subject {} --predicate {} --object {} --evidence ... --confidence ...",
        row.signed_claim.signature.signed_cid.0, claim.subject, claim.predicate, claim.object,
    )
}

/// Render the federation footer: the distinct-author count plus the
/// content-frozen no-merge guarantee. Pure helper.
fn render_federation_footer(author_count: usize) -> String {
    format!("{author_count} author(s). {NO_MERGE_FOOTER_LITERAL}\n")
}

/// Render the zero-peers degraded-path footer (FQ-4 / US-FED-003 AC #7):
/// the content-frozen hint pointing the user at `peer add`. Pure helper.
fn render_no_peers_footer() -> String {
    format!("{NO_PEERS_FOOTER_LITERAL}\n")
}

/// `true` when NO row in a federated result came from the peer table —
/// i.e. zero peers contributed claims. Pure projection over the rows'
/// `source_table` attribution (the type-level anti-merging field). This is
/// the signal the renderer uses to switch from the no-merge footer to the
/// zero-peers degraded hint (FQ-4). An empty result counts as no-peers too.
fn has_no_peer_rows(rows: &[FederatedRow]) -> bool {
    !rows
        .iter()
        .any(|row| matches!(row.source_table, SourceTable::Peer))
}

/// Compute the `is_retracted` flag for one CID given the back-reference
/// list `StoragePort::query_referencing` returns. Pure helper kept here
/// (next to the renderer that consumes it) so the projection rule lives
/// in one place; the verb wires storage I/O around it.
pub fn is_retracted_by(_target: &Cid, referencing: &[(Cid, claim_domain::ReferenceType)]) -> bool {
    referencing
        .iter()
        .any(|(_, ref_type)| matches!(ref_type, claim_domain::ReferenceType::Retracts))
}

/// Render one `SignedClaim` as a labeled block. The label widths are
/// padded so the values line up visually; the load-bearing contract is
/// "value text matches compose-time byte-for-byte", not the column
/// alignment.
fn render_one_claim(claim: &SignedClaim) -> String {
    let mut out = String::new();
    out.push_str(&format!("subject:     {}\n", claim.unsigned.subject));
    out.push_str(&format!("predicate:   {}\n", claim.unsigned.predicate));
    out.push_str(&format!("object:      {}\n", claim.unsigned.object));
    out.push_str(&format!(
        "evidence:    {}\n",
        render_evidence(&claim.unsigned.evidence)
    ));
    out.push_str(&format!(
        "confidence:  {}\n",
        render_confidence(&claim.unsigned.confidence)
    ));
    out.push_str(&format!("author:      {}\n", claim.unsigned.author_did.0));
    out.push_str(&format!("composedAt:  {}\n", claim.unsigned.composed_at));
    out.push_str(&format!("cid:         {}\n", claim.signature.signed_cid.0));
    out
}

/// Render the evidence list. Empty -> "(none)" so the line is never
/// orphaned; single entry -> the URL verbatim; multiple entries -> a
/// comma-joined list. URLs are NEVER normalized (no scheme lowering,
/// no trailing-slash stripping, no percent-encode round-trip) — that's
/// the KPI-4 zero-normalization invariant.
fn render_evidence(evidence: &[String]) -> String {
    if evidence.is_empty() {
        "(none)".to_string()
    } else {
        evidence.join(", ")
    }
}

/// Render the confidence field. Goes through serde so we read the
/// original `f64` (the `Confidence` newtype's inner is crate-private to
/// `claim_domain`; its `value()` accessor is a RED-scaffold panic at
/// this slice). `serde_json::to_value` returns a JSON number, which
/// `Display` renders as the minimal decimal representation matching the
/// original `f64` (e.g. `0.86`, not `0.860000`).
///
/// We deliberately do NOT use `{:.2}` formatting here — that would be
/// normalization (forcing 2 decimal places) and would break KPI-4 for
/// values like `0.123456` that the user might legitimately compose with.
fn render_confidence(confidence: &claim_domain::Confidence) -> String {
    serde_json::to_value(confidence)
        .map(|v| v.to_string())
        .unwrap_or_else(|_| "(unrenderable)".to_string())
}

// -----------------------------------------------------------------------------
// Slice-04 (ADR-020) — `graph query --object <philosophy> --weighted` renderer
// -----------------------------------------------------------------------------

/// Content-frozen never-stored notice for the `--weighted` view (WD-72; journey
/// `journey-explore-the-graph-visual.md` step 4 tui_mockup). Weights are a
/// DERIVED, DISPLAY-ONLY aggregate VIEW computed at query time — they are NOT
/// stored, NOT signed, and NOT published; a re-run after a `peer pull` may
/// change them; and (anti-merging, I-GRAPH-2) each weight decomposes to the
/// exact claims that produced it. Do NOT paraphrase — the exact phrasing is the
/// user-visible contract.
const WEIGHTED_NEVER_STORED_FOOTER: &str =
    "Note: weights are a DISPLAY-ONLY aggregate VIEW computed at query time from the claims \
above. They are NOT stored, NOT signed, and NOT published. Re-running after a `peer pull` may \
change them. Each weight decomposes to the exact claims that produced it; nothing is merged or \
invented.";

/// Render the `graph query --object <philosophy> --weighted` result: the
/// projects ranked by adherence weight (descending), each weight shown WITH its
/// inputs, the transparent NO-ML formula, and the never-stored display-only
/// footer. Pure function — no I/O, no storage access.
///
/// ## Transparency contract (WD-71 / KPI-GRAPH-3; US-GRAPH-003)
///
/// `view.ranked` is already sorted weight-descending by the pure `scoring::score`
/// core. A weight is NEVER shown as a bare number: each carries its claim count,
/// distinct-author count, and max confidence (the formula inputs), so a user can
/// reproduce it by hand against the printed formula. The formula block states
/// "no ML" verbatim (WD-71). Cross-project triangulation by the SAME author and
/// multi-author support are surfaced as the breadth that lifts a weight.
///
/// ## Anti-merging (I-GRAPH-2 / WD-73)
///
/// A weight is an AGGREGATE VIEW, never a merge that loses attribution. The
/// breakdown surfaces the contributing authors by DID (from the pairing's
/// non-empty `contributions`), so the aggregate always decomposes to its claims.
///
/// ## Sparse honesty (WD-74; Gate 3) — RED for GQE-11
///
/// The `[SPARSE]` honesty line for a thin (1-claim 1-author) pairing lands with
/// GQE-11 (step 05-02). This renderer prints the bucket label for every pairing;
/// the sparse-specific "(!) based on N claims by M authors" advice is added then.
pub fn render_weighted_view(object: &str, view: &scoring::WeightedView) -> String {
    let mut out = String::new();
    let heading = format!("Weighted view: {object}");
    out.push_str(&heading);
    out.push('\n');
    out.push_str(&"=".repeat(heading.len()));
    out.push_str("\n\n");
    out.push_str("Projects ranked by adherence weight (transparent formula below):\n\n");

    for (index, pairing) in view.ranked.iter().enumerate() {
        out.push_str(&render_weighted_pairing(index + 1, pairing));
    }

    out.push_str(&render_weight_formula_block());
    out.push('\n');
    out.push_str(WEIGHTED_NEVER_STORED_FOOTER);
    out.push('\n');
    out
}

/// Render one ranked `(subject, object)` pairing: its rank, subject, weight, and
/// display-only bucket, then the weight inputs (claims / authors / max
/// confidence) and the breadth line (cross-project span and/or multi-author
/// support) naming the contributing authors. Pure helper.
fn render_weighted_pairing(rank: usize, pairing: &scoring::WeightedPairing) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "  {rank}. {subject}   weight {weight:.2}   [{bucket}]\n",
        subject = pairing.subject,
        weight = pairing.weight,
        bucket = weight_bucket_label(pairing.bucket),
    ));
    out.push_str(&format!(
        "       claims  : {claims}   authors: {authors}   max-confidence {max_conf}\n",
        claims = pairing.claim_count,
        authors = pairing.distinct_author_count,
        max_conf = render_candidate_confidence(pairing.max_confidence),
    ));
    out.push_str(&render_pairing_breadth_line(pairing));
    // WD-74 (Gate 3 sparse_renders_sparse): a thin (single-claim single-author
    // no-span) pairing carries an epistemic-honesty block naming its ACTUAL
    // evidence base + lead-not-conclusion advice, so a single high-confidence
    // opinion is NEVER presented as a settled verdict (mitigates J-002).
    out.push_str(&render_sparse_honesty_block(pairing));
    out.push('\n');
    out
}

/// Content-frozen sparse-honesty line template (WD-74; GQE-11 docstring). Names
/// the actual evidence base verbatim — "based on N claim(s) by M author(s)" —
/// with `{claims}` / `{authors}` filled by [`render_sparse_honesty_block`]. Do
/// NOT paraphrase; the exact wording is the user-visible epistemic-honesty
/// contract (Gate 3 sparse_renders_sparse).
const SPARSE_HONESTY_LINE_TEMPLATE: &str = "(!) based on {claims} by {authors}";

/// Content-frozen lead-not-conclusion advice (WD-74; GQE-11 docstring). Thin
/// evidence is a LEAD to investigate, never a defensible conclusion. Do NOT
/// paraphrase — the exact phrasing is the user-visible contract.
const SPARSE_LEAD_NOT_CONCLUSION_ADVICE: &str =
    "treat as a lead, not a defensible conclusion — investigate before relying on it";

/// Render the WD-74 epistemic-honesty block for a [`WeightBucket::Sparse`]
/// pairing: a line naming the real evidence base ("(!) based on N claim(s) by
/// M author(s)") plus the lead-not-conclusion advice. Returns the empty string
/// for a non-sparse pairing (Strong/Moderate already cleared the breadth guard,
/// so they need no honesty caveat). Pure helper.
///
/// The counts come straight off the pairing's `claim_count` /
/// `distinct_author_count` — the SAME inputs that drove the
/// [`scoring::weight_bucket`] breadth guard (WD-74/WD-90) — so the sentence
/// can never disagree with the `[SPARSE]` label it accompanies.
fn render_sparse_honesty_block(pairing: &scoring::WeightedPairing) -> String {
    if !matches!(pairing.bucket, scoring::WeightBucket::Sparse) {
        return String::new();
    }
    let honesty_line = SPARSE_HONESTY_LINE_TEMPLATE
        .replace("{claims}", &pluralize(pairing.claim_count, "claim"))
        .replace(
            "{authors}",
            &pluralize(pairing.distinct_author_count, "author"),
        );
    format!("       {honesty_line}\n       {SPARSE_LEAD_NOT_CONCLUSION_ADVICE}\n")
}

/// Pluralize a count + singular noun for the honesty line: `1 claim`, `2
/// claims`, `0 authors`. English `-s` plural suffices for the domain nouns
/// (claim/author). Pure helper.
fn pluralize(count: u32, singular: &str) -> String {
    if count == 1 {
        format!("{count} {singular}")
    } else {
        format!("{count} {singular}s")
    }
}

/// The breadth line surfaced under a pairing's inputs: the cross-project span
/// (the SAME author asserting this philosophy on >= 2 distinct subjects, which
/// the formula rewards with the triangulation bonus) and/or multi-author support
/// (distinct authors raising triangulation). Both name the contributing authors
/// so the aggregate stays attributed (anti-merging). Returns the empty string
/// for a thin single-author single-project pairing. Pure helper.
fn render_pairing_breadth_line(pairing: &scoring::WeightedPairing) -> String {
    let mut out = String::new();
    if pairing.cross_project_span > 1 {
        // Name the author whose cross-project span earned the triangulation
        // bonus (the contribution carrying a non-zero triangulation bonus).
        if let Some(spanning) = pairing
            .contributions()
            .iter()
            .find(|c| c.cross_project_triangulation_bonus > 0.0)
        {
            out.push_str(&format!(
                "       also-claimed-by: {} spans {} projects\n",
                spanning.author_did.0, pairing.cross_project_span,
            ));
        }
    }
    if pairing.distinct_author_count > 1 {
        out.push_str(&format!(
            "       multi-author: {} distinct authors raise triangulation\n",
            pairing.distinct_author_count,
        ));
        // List the contributing authors by DID *with each claim's own
        // confidence* so the multi-author aggregate decomposes to its attributed
        // claims (anti-merging, WD-73). Surfacing every contribution's confidence
        // is what keeps a CONFLICTING pair (e.g. 0.85 vs 0.20) honest: both
        // claims stay visible per their OWN confidence, never averaged away
        // (GQE-13; ADR-022 anti-merging-in-aggregates).
        for contribution in pairing.contributions() {
            out.push_str(&format!(
                "         - {} (confidence {})\n",
                contribution.author_did.0,
                render_candidate_confidence(contribution.base),
            ));
        }
    }
    out
}

/// The display-only bucket label for a `WeightBucket` (WD-72; never persisted).
/// Pure helper.
fn weight_bucket_label(bucket: scoring::WeightBucket) -> &'static str {
    match bucket {
        scoring::WeightBucket::Strong => "STRONG",
        scoring::WeightBucket::Moderate => "MODERATE",
        scoring::WeightBucket::Sparse => "SPARSE",
    }
}

/// The auditable NO-ML formula block (WD-71 / WD-77 SSOT; journey step 4
/// tui_mockup). Names every formula input and the constants, and states
/// "no ML" verbatim so the weight is reproducible by hand (Gate 2). The
/// bucket labels are flagged DISPLAY-ONLY (WD-72). Pure helper.
fn render_weight_formula_block() -> String {
    let mut out = String::new();
    out.push_str("How weight is computed (auditable, no ML):\n");
    out.push_str(
        "  weight = sum over claims of [ confidence\n\
\x20                               x author_distinct_bonus\n\
\x20                               x cross_project_triangulation_bonus ]\n",
    );
    out.push_str(
        "  - author_distinct_bonus        : 1.0 for the first author, \
+0.25 per add'l distinct author on the SAME (subject,object)\n",
    );
    out.push_str(
        "  - cross_project_triangulation  : +0.5 if the SAME author asserts \
this philosophy on >=2 distinct subjects\n",
    );
    out.push_str(
        "  - bucket labels [STRONG]/[MODERATE]/[SPARSE] are DISPLAY-ONLY; never persisted.\n",
    );
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use claim_domain::{Cid, ClaimReference, Confidence, Did, SignatureBlock, UnsignedClaim};
    use proptest::prelude::*;

    fn confidence(value: f64) -> Confidence {
        serde_json::from_value(serde_json::json!(value))
            .expect("test confidence value is well-formed")
    }

    /// Build a `FederatedRow` for a given author DID + cid + relationship +
    /// source table. The claim body fields are deterministic stand-ins; the
    /// federated renderer's contract is about attribution + grouping, not
    /// the (already-tested) per-claim field rendering.
    fn federated_row(
        author_did: &str,
        cid: &str,
        relationship: AuthorRelationship,
        source_table: SourceTable,
    ) -> FederatedRow {
        FederatedRow {
            author_did: Did(author_did.to_string()),
            author_relationship: relationship,
            signed_claim: SignedClaim {
                unsigned: UnsignedClaim {
                    subject: "github:rust-lang/cargo".to_string(),
                    predicate: "embodiesPhilosophy".to_string(),
                    object: "org.openlore.philosophy.memory-safety".to_string(),
                    evidence: vec!["https://github.com/rust-lang/cargo".to_string()],
                    confidence: confidence(0.5),
                    author_did: Did(format!("{author_did}#org.openlore.application")),
                    composed_at: "2026-05-22T09:18:44Z".to_string(),
                    references: Vec::<ClaimReference>::new(),
                    reason: None,
                },
                signature: SignatureBlock {
                    signed_cid: Cid(cid.to_string()),
                    signature_bytes: vec![0u8; 64],
                    verification_method: format!("{author_did}#org.openlore.application"),
                },
            },
            source_table,
        }
    }

    /// Build a `FederatedRow` whose claim carries a single `Counters`
    /// reference to `counters_target`. Used to exercise the bidirectional
    /// counter annotation (FQ-5): the row is a counter-claim pointing at
    /// another row's CID.
    fn federated_counter_row(
        author_did: &str,
        cid: &str,
        relationship: AuthorRelationship,
        source_table: SourceTable,
        counters_target: &str,
    ) -> FederatedRow {
        let mut row = federated_row(author_did, cid, relationship, source_table);
        row.signed_claim.unsigned.references = vec![ClaimReference {
            ref_type: claim_domain::ReferenceType::Counters,
            cid: Cid(counters_target.to_string()),
        }];
        row
    }

    /// FQ-5 (US-FED-003 AC #8): when one federated row counters another, the
    /// renderer annotates BOTH rows bidirectionally — the counter-claim row
    /// shows `counters <target_cid> by <peer_did>` and the countered row shows
    /// `countered-by <counter_cid> by <author_did>` — and the summary line
    /// states the counter-relationship count. The annotation is per-row
    /// METADATA computed from the reference graph over the row set; it never
    /// merges the two rows (both authors keep their own headers).
    #[test]
    fn render_federated_query_result_annotates_counter_relationships_bidirectionally() {
        // Rachel's target claim + the local user's counter pointing at it.
        let rows = vec![
            federated_counter_row(
                "did:plc:test-jeff",
                "bafycounter1",
                AuthorRelationship::You,
                SourceTable::Own,
                "bafytarget1",
            ),
            federated_row(
                "did:plc:rachel-test",
                "bafytarget1",
                AuthorRelationship::SubscribedPeer,
                SourceTable::Peer,
            ),
        ];

        let rendered = render_federated_query_result(&rows);

        // Forward: the counter-claim row names what it counters + the target's
        // author DID.
        assert!(
            rendered.contains("counters bafytarget1 by did:plc:rachel-test"),
            "expected the counter-claim row annotated \
             'counters bafytarget1 by did:plc:rachel-test' (forward); got:\n{rendered}"
        );

        // Backward: the countered row names what counters it + that counter's
        // author DID.
        assert!(
            rendered.contains("countered-by bafycounter1 by did:plc:test-jeff"),
            "expected the countered row annotated \
             'countered-by bafycounter1 by did:plc:test-jeff' (backward); got:\n{rendered}"
        );

        // The summary line states the counter-relationship count (exactly 1).
        assert!(
            rendered.contains("1 counter relationship"),
            "expected the summary line to state the counter-relationship count \
             (1 counter relationship); got:\n{rendered}"
        );

        // Anti-merging: both authors keep their own per-author header — the
        // annotation is metadata, never a merge.
        assert!(
            rendered.contains("author: did:plc:test-jeff (you)")
                && rendered.contains("author: did:plc:rachel-test (subscribed peer)"),
            "expected BOTH authors to keep their own headers (no merge); got:\n{rendered}"
        );
    }

    fn fixture_signed() -> SignedClaim {
        SignedClaim {
            unsigned: UnsignedClaim {
                subject: "github:rust-lang/rust".to_string(),
                predicate: "embodiesPhilosophy".to_string(),
                object: "org.openlore.philosophy.memory-safety".to_string(),
                evidence: vec!["https://www.rust-lang.org/".to_string()],
                confidence: confidence(0.86),
                author_did: Did("did:plc:test-jeff#org.openlore.application".to_string()),
                composed_at: "2026-05-25T12:00:00Z".to_string(),
                references: Vec::<ClaimReference>::new(),
                reason: None,
            },
            signature: SignatureBlock {
                signed_cid: Cid("bafytestcid".to_string()),
                signature_bytes: vec![0u8; 64],
                verification_method: "did:plc:test-jeff#org.openlore.application".to_string(),
            },
        }
    }

    /// KPI-4: confidence renders as the original f64, not as a bucket
    /// label. None of "speculative" / "weighted" / "well-evidenced" /
    /// "triangulated" appear in the rendered output.
    #[test]
    fn render_graph_query_result_never_emits_bucket_label() {
        let rendered = render_graph_query_result(&[fixture_signed()]);
        for label in &["speculative", "weighted", "well-evidenced", "triangulated"] {
            assert!(
                !rendered.contains(label),
                "rendered output contained bucket label '{label}' (WD-10 forbids); got:\n{rendered}"
            );
        }
        assert!(
            rendered.contains("confidence:  0.86"),
            "expected confidence rendered as 0.86; got:\n{rendered}"
        );
    }

    /// US-FED-004 AC: the counter-claim compose preview carries BOTH
    /// framing literals, names the countered target + its peer author, and
    /// shows the reason verbatim. Pins the load-bearing compose UX copy
    /// without spawning a subprocess.
    #[test]
    fn render_counter_compose_preview_contains_both_framing_literals_and_target() {
        let counter = ComposedCounterClaim {
            target_cid: "bafytargetcid001".to_string(),
            target_author_did: "did:plc:rachel-test".to_string(),
            reason: "The cited benchmark was retracted by upstream.".to_string(),
            author_did: "did:plc:test-jeff#org.openlore.application".to_string(),
            composed_at: "2026-05-28T09:42:11+00:00".to_string(),
        };
        let preview = render_counter_compose_preview(&counter);
        assert!(
            preview.contains(NOT_AS_TRUTH_LITERAL),
            "preview must contain the inherited 'not as truth' literal (I-7); got:\n{preview}"
        );
        assert!(
            preview.contains(COUNTER_COEXIST_LITERAL),
            "preview must contain the slice-03 'counter-claims coexist, never overwrite' \
             literal; got:\n{preview}"
        );
        assert!(
            preview.contains("counters: bafytargetcid001 (by did:plc:rachel-test)"),
            "preview must name the countered target + its peer author; got:\n{preview}"
        );
        assert!(
            preview.contains("The cited benchmark was retracted by upstream."),
            "preview must show the reason verbatim; got:\n{preview}"
        );
    }

    /// The reason is word-wrapped at 78 columns: no rendered line of the
    /// reason block exceeds 78 chars (plus the 4-space indent), and the
    /// full reason survives concatenation (verbatim, only line-broken).
    #[test]
    fn render_counter_compose_preview_wraps_reason_at_78_cols() {
        let long_reason = "word ".repeat(40);
        let long_reason = long_reason.trim().to_string();
        let counter = ComposedCounterClaim {
            target_cid: "bafytargetcid".to_string(),
            target_author_did: "did:plc:rachel-test".to_string(),
            reason: long_reason.clone(),
            author_did: "did:plc:test-jeff".to_string(),
            composed_at: "2026-05-28T09:42:11+00:00".to_string(),
        };
        let preview = render_counter_compose_preview(&counter);
        // Each reason line (the 4-space-indented ones) <= 78 cols of content.
        for line in preview.lines() {
            if let Some(content) = line.strip_prefix("    ") {
                assert!(
                    content.chars().count() <= 78,
                    "reason line exceeds 78 cols: {content:?}"
                );
            }
        }
        // The reason words survive verbatim (rejoined across wrap breaks).
        let rejoined: String = preview
            .lines()
            .filter_map(|l| l.strip_prefix("    "))
            .collect::<Vec<_>>()
            .join(" ");
        assert_eq!(rejoined, long_reason, "reason must survive wrap verbatim");
    }

    /// FQ-1 (behavioral anti-merging, I-FED-1 layer 3): the federated
    /// renderer groups rows under ONE header per distinct author DID and
    /// emits a footer that states the distinct-author count AND the
    /// content-frozen no-merge guarantee, with NO merged/consensus row.
    #[test]
    fn render_federated_query_result_groups_by_author_with_no_merge_footer() {
        let rows = vec![
            federated_row(
                "did:plc:test-jeff",
                "bafyown1",
                AuthorRelationship::You,
                SourceTable::Own,
            ),
            federated_row(
                "did:plc:rachel-test",
                "bafypeer1",
                AuthorRelationship::SubscribedPeer,
                SourceTable::Peer,
            ),
            federated_row(
                "did:plc:rachel-test",
                "bafypeer2",
                AuthorRelationship::SubscribedPeer,
                SourceTable::Peer,
            ),
        ];

        let rendered = render_federated_query_result(&rows);

        // Two distinct author headers, each annotated with its relationship.
        assert!(
            rendered.contains("author: did:plc:test-jeff (you)"),
            "expected the local user's per-author header annotated '(you)'; got:\n{rendered}"
        );
        assert!(
            rendered.contains("author: did:plc:rachel-test (subscribed peer)"),
            "expected the peer's per-author header annotated '(subscribed peer)'; got:\n{rendered}"
        );

        // Each row carries author_did + confidence + cid (independently
        // attributable — anti-merging behavioral layer).
        for cid in ["bafyown1", "bafypeer1", "bafypeer2"] {
            assert!(
                rendered.contains(cid),
                "expected each row cid {cid} to appear; got:\n{rendered}"
            );
        }
        assert!(
            rendered.contains("author_did:"),
            "expected each row to pin author_did on its own line; got:\n{rendered}"
        );

        // Footer: distinct-author count (2) + content-frozen no-merge text.
        assert!(
            rendered.contains("2 author(s)."),
            "expected the footer to state the distinct-author count (2); got:\n{rendered}"
        );
        assert!(
            rendered.contains(NO_MERGE_FOOTER_LITERAL),
            "expected the content-frozen no-merge footer; got:\n{rendered}"
        );

        // KPI-FED-2 zero-merge gate: no merged/consensus/aggregate label.
        let lower = rendered.to_lowercase();
        for banned in ["merged", "consensus", "aggregate"] {
            // The no-merge footer contains "merged" inside "are merged" —
            // exclude that one legitimate occurrence by checking it does not
            // appear OUTSIDE the footer literal.
            let without_footer = lower.replace(&NO_MERGE_FOOTER_LITERAL.to_lowercase(), "");
            assert!(
                !without_footer.contains(banned),
                "federated render must not label any row {banned:?}; got:\n{rendered}"
            );
        }
    }

    /// FQ-4 (US-FED-003 AC #7): when ONLY own rows are present (zero peers
    /// contributed), the federated renderer degrades gracefully — the own
    /// rows still render under the "(you)" header, but the footer is the
    /// content-frozen zero-peers hint, NOT the no-merge guarantee. The hint
    /// is an exact user-visible string (content-frozen), so an example-based
    /// test pins it (golden-string contract — property-framing would not add
    /// coverage over a single literal).
    #[test]
    fn render_federated_query_result_emits_zero_peers_hint_when_no_peer_rows() {
        let rows = vec![federated_row(
            "did:plc:test-jeff",
            "bafyown1",
            AuthorRelationship::You,
            SourceTable::Own,
        )];

        let rendered = render_federated_query_result(&rows);

        // The own claim still renders under its "(you)" header — degradation
        // never swallows the local rows.
        assert!(
            rendered.contains("author: did:plc:test-jeff (you)"),
            "expected the own claim to render under its '(you)' header; got:\n{rendered}"
        );
        assert!(
            rendered.contains("bafyown1"),
            "expected the own claim cid to render; got:\n{rendered}"
        );

        // The footer is the content-frozen zero-peers hint VERBATIM.
        assert!(
            rendered.contains(NO_PEERS_FOOTER_LITERAL),
            "expected the content-frozen zero-peers hint footer; got:\n{rendered}"
        );

        // And the no-merge guarantee footer is NOT emitted on the degraded
        // path — the two footers are mutually exclusive.
        assert!(
            !rendered.contains(NO_MERGE_FOOTER_LITERAL),
            "expected the no-merge footer to be ABSENT when zero peers contributed; got:\n{rendered}"
        );
        assert!(
            !rendered.contains("author(s)."),
            "expected NO distinct-author-count footer on the zero-peers degraded path; got:\n{rendered}"
        );
    }

    /// FQ-7 (WD-42 — habit-bridging affordance, KPI-FED-3): every PEER row
    /// in the federated render carries an inline copy-pasteable counter
    /// template pre-filled with the target claim's CID, subject, predicate,
    /// and object, shown BY DEFAULT (no `--verbose` gate at the render layer
    /// — the renderer always emits it). OWN rows do NOT get a template (you
    /// don't counter your own claim). The template count equals the peer-row
    /// count. The exact template prefix is content-frozen UX copy, so an
    /// example-based test pins the literal — property-framing would not add
    /// coverage over a fixed string.
    #[test]
    fn render_federated_query_result_emits_inline_counter_template_per_peer_row_only() {
        let rows = vec![
            federated_row(
                "did:plc:test-jeff",
                "bafyown1",
                AuthorRelationship::You,
                SourceTable::Own,
            ),
            federated_row(
                "did:plc:rachel-test",
                "bafypeer1",
                AuthorRelationship::SubscribedPeer,
                SourceTable::Peer,
            ),
            federated_row(
                "did:plc:rachel-test",
                "bafypeer2",
                AuthorRelationship::SubscribedPeer,
                SourceTable::Peer,
            ),
        ];

        let rendered = render_federated_query_result(&rows);

        // Each PEER row carries an inline template naming its CID + pre-filled
        // subject/predicate/object from the target claim (the `federated_row`
        // fixture uses subject github:rust-lang/cargo, predicate
        // embodiesPhilosophy, object org.openlore.philosophy.memory-safety).
        for cid in ["bafypeer1", "bafypeer2"] {
            let expected = format!(
                "openlore claim counter {cid} --reason \"...\" \
                 --subject github:rust-lang/cargo --predicate embodiesPhilosophy \
                 --object org.openlore.philosophy.memory-safety"
            );
            assert!(
                rendered.contains(&expected),
                "expected an inline counter template for peer row {cid}; got:\n{rendered}"
            );
        }

        // The OWN row gets NO template — its CID never follows `counter `.
        assert!(
            !rendered.contains("openlore claim counter bafyown1"),
            "own row must NOT get a counter template (WD-42 own-rows-excluded); got:\n{rendered}"
        );

        // Exactly one template per peer row (2 peers → 2 templates).
        assert_eq!(
            rendered.matches("openlore claim counter ").count(),
            2,
            "expected exactly one template per peer row (2 peer rows); got:\n{rendered}"
        );
    }

    proptest! {
        /// Property (Modeling / Generalizing, Hebert ch.3): for ANY set of
        /// federated rows over an arbitrary author-DID alphabet, the number
        /// of per-author headers the renderer emits equals the number of
        /// DISTINCT author DIDs in the input, and the footer count agrees.
        /// This is the anti-merging invariant generalized: rows never
        /// collapse across authors, and authors never split into phantom
        /// extra headers.
        #[test]
        fn render_federated_groups_exactly_one_header_per_distinct_author(
            author_indices in prop::collection::vec(0usize..4, 1..12),
        ) {
            // Map the generated indices onto a small DID alphabet so the
            // distinct-author count is controllable + verifiable.
            let alphabet = [
                "did:plc:author-a",
                "did:plc:author-b",
                "did:plc:author-c",
                "did:plc:author-d",
            ];
            let rows: Vec<FederatedRow> = author_indices
                .iter()
                .enumerate()
                .map(|(i, &idx)| {
                    federated_row(
                        alphabet[idx],
                        &format!("bafycid{i:03}"),
                        AuthorRelationship::SubscribedPeer,
                        SourceTable::Peer,
                    )
                })
                .collect();

            let distinct: std::collections::HashSet<usize> =
                author_indices.iter().copied().collect();
            let expected_authors = distinct.len();

            let rendered = render_federated_query_result(&rows);

            // One header line per distinct author.
            let header_count = rendered
                .lines()
                .filter(|l| l.starts_with("author: "))
                .count();
            prop_assert_eq!(
                header_count,
                expected_authors,
                "expected exactly {} author headers; got {}\n{}",
                expected_authors,
                header_count,
                rendered
            );

            // Footer count agrees with the distinct-author cardinality.
            prop_assert!(
                rendered.contains(&format!("{expected_authors} author(s).")),
                "footer must state distinct-author count {}; got:\n{}",
                expected_authors,
                rendered
            );

            // Every row's cid appears as exactly ONE `cid:` field line (no
            // row dropped, no row duplicated by the grouping). We count the
            // canonical `cid:` field line — NOT raw substring — because each
            // PEER row now also names its cid inside the FQ-7 inline counter
            // template (WD-42), so a raw substring count is 2 per peer row by
            // design. The row-identity invariant is "one cid: field per row".
            for i in 0..author_indices.len() {
                let cid = format!("bafycid{i:03}");
                let cid_field_occurrences = rendered
                    .lines()
                    .filter(|l| {
                        l.trim_start().starts_with("cid:") && l.trim_end().ends_with(&cid)
                    })
                    .count();
                prop_assert_eq!(
                    cid_field_occurrences,
                    1,
                    "cid {} must appear as exactly one `cid:` field line (no merge / no drop); got {}",
                    cid,
                    cid_field_occurrences
                );
            }
        }
    }

    /// Every compose-time field appears in the output byte-for-byte.
    #[test]
    fn render_graph_query_result_contains_all_fields_verbatim() {
        let claim = fixture_signed();
        let rendered = render_graph_query_result(&[claim]);
        for expected in &[
            "github:rust-lang/rust",
            "embodiesPhilosophy",
            "org.openlore.philosophy.memory-safety",
            "https://www.rust-lang.org/",
            "did:plc:test-jeff#org.openlore.application",
            "2026-05-25T12:00:00Z",
            "bafytestcid",
        ] {
            assert!(
                rendered.contains(expected),
                "expected rendered output to contain {expected:?}; got:\n{rendered}"
            );
        }
    }

    /// The empty-`--object` renderer (GQE-4 / US-GRAPH-001 Example 4) names the
    /// queried object and, when a near-match is supplied, appends the
    /// content-frozen "Did you mean ...?" suggestion — and NEVER manufactures a
    /// per-claim row. Pins the user-visible empty-result copy without a
    /// subprocess. Example-based: the message is an exact golden string.
    #[test]
    fn render_object_query_empty_with_suggestion_names_object_and_near_match() {
        let missed = "org.openlore.philosophy.dependancy-pinning";
        let near = "org.openlore.philosophy.dependency-pinning";

        let rendered = render_object_query_grouped_by_subject(missed, &[], Some(near));
        assert!(
            rendered.contains(&format!("No claims found for object {missed}. Did you mean {near}?")),
            "expected the no-claims line to name the queried object + the near-match; got:\n{rendered}"
        );

        // Without a near-match, the bare no-claims line is emitted (no dangling
        // "Did you mean").
        let bare = render_object_query_grouped_by_subject(missed, &[], None);
        assert!(
            bare.contains(&format!("No claims found for object {missed}.")),
            "expected the bare no-claims line to name the queried object; got:\n{bare}"
        );
        assert!(
            !bare.contains("Did you mean"),
            "expected NO suggestion clause when no near-match exists; got:\n{bare}"
        );

        // Empty is honest: neither rendering manufactures a per-claim cid row.
        for out in [&rendered, &bare] {
            assert!(
                !out.lines().any(|l| l.trim_start().starts_with("cid:")),
                "empty --object render must NOT manufacture a cid row; got:\n{out}"
            );
        }
    }

    proptest! {
        /// Property (Modeling / Generalizing, Hebert ch.3) — the suggestion
        /// ranker's correctness contract: for ANY existing philosophy URI and
        /// ANY single-edit typo of it (transposition / substitution / deletion /
        /// insertion over the philosophy-URI alphabet), the correct URI is among
        /// the candidate neighbours `single_edit_neighbours(typo)` enumerates.
        /// That is the invariant the verb's probe loop relies on: the closest
        /// EXISTING object is always reachable as a single-edit neighbour, so a
        /// one-character typo always recovers its near-match. The original typo
        /// is NEVER its own neighbour (it already came back empty).
        #[test]
        fn single_edit_neighbours_recovers_the_correct_object_from_any_one_char_typo(
            // A realistic philosophy suffix over the URI alphabet, length 4..24.
            suffix in "[a-z][a-z0-9-]{3,23}",
            edit_pos in 0usize..24,
        ) {
            let correct = format!("org.openlore.philosophy.{suffix}");
            let correct_chars: Vec<char> = correct.chars().collect();
            // Build a single-substitution typo at a position inside the suffix
            // (guaranteed in-range + a guaranteed-different replacement char).
            let prefix_len = "org.openlore.philosophy.".chars().count();
            let pos = prefix_len + (edit_pos % suffix.chars().count());
            let original = correct_chars[pos];
            let replacement = if original == 'x' { 'y' } else { 'x' };
            let mut typo_chars = correct_chars.clone();
            typo_chars[pos] = replacement;
            let typo: String = typo_chars.into_iter().collect();

            prop_assume!(typo != correct);

            let neighbours = single_edit_neighbours(&typo);

            // The correct URI is recoverable as a single-edit neighbour of the typo.
            prop_assert!(
                neighbours.iter().any(|n| n == &correct),
                "expected single_edit_neighbours({typo:?}) to contain the correct URI {correct:?}"
            );
            // The typo itself is never emitted as its own neighbour.
            prop_assert!(
                !neighbours.iter().any(|n| n == &typo),
                "single_edit_neighbours must never emit the original string as a neighbour"
            );
        }
    }
}
