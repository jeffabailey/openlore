//! `graph query --subject <uri>` — read back claims for a subject.
//!
//! Step 05-11 (WS-11): the first read-path verb. The contract is the
//! mirror image of `claim add` + `claim publish`:
//!
//! - Read via `StoragePort::query_by_subject(subject)` -> `Vec<SignedClaim>`.
//! - Render each claim's fields VERBATIM from the same serde model the
//!   write path canonicalizes through (`crate::render::render_graph_query_result`).
//! - Print to stdout.
//!
//! ## KPI-4 zero-normalization invariant
//!
//! The read path MUST emit every field byte-for-byte equal to what the
//! author composed:
//!
//! - `confidence` as the original `f64` (e.g. `0.86`), NEVER as a bucket
//!   label (`well-evidenced` etc.) — that bucket vocabulary is
//!   compose-time display only (WD-10 / D-12).
//! - `composedAt` keeps the exact RFC3339 string, no timezone shifting.
//! - `evidence` URLs verbatim, no scheme/case normalization.
//! - `author` is the full DID with verification-method fragment.
//!
//! This makes the round-trip identity (compose -> sign -> publish ->
//! query) provable at the CLI boundary, which is what WS-11 asserts.
//!
//! ## Slice-01 scope
//!
//! - Local-only: queries `StoragePort` (DuckDB), NOT the PDS / federated
//!   peers. The `--federated` flag is the slice-03 surface (US-004 AC #2);
//!   WS-12 (step 05-12) wraps the rendered claims with a header ("Showing
//!   local claims only") and a footer mentioning `--federated` and
//!   `slice-03` so the local-only default is announced unconditionally.
//! - Empty-result explainer (US-004 AC #3) is WS-13's contract (step
//!   05-13): when `query_by_subject` returns an empty `Vec`, the verb
//!   emits `No local claims about <subject>.` between the local-only
//!   header and the federation footer, with exit 0. Silence would be
//!   hostile — see the `run` doc for the rationale.
//!
//! ## Header + footer (US-004 AC #2 + WD-13)
//!
//! - Header: `Showing local claims only` — printed before the per-claim
//!   blocks. Content-frozen by US-004 AC #2 (the exact phrasing is part
//!   of the user-visible contract; do NOT paraphrase).
//! - Footer: announces that federated querying is a future affordance
//!   landing in slice-03 (per WD-13), and names the `--federated` flag
//!   that will activate it. Both `--federated` and `slice-03` appear
//!   literally in the footer so operators searching with `grep` find
//!   the right pointer.
//!
//! Both are unconditional in slice-01 because the `--federated` flag
//! is not yet wired (federation is slice-03 territory per WD-13). They
//! frame every query result — populated or empty — so the contract
//! ("local-only is the default; federation lands in slice-03") is
//! announced regardless of whether the lookup found anything. The
//! empty-result branch (WS-13) inserts its explainer between the header
//! and footer instead of the per-claim block.

use anyhow::{Context, Result};

use crate::orientation::{self, OrientationMilestone};
use crate::render::{
    is_retracted_by, render_annotated_graph_query_result, render_federated_query_result,
    AnnotatedClaim,
};
use crate::wiring::Wiring;

/// Header line printed before the per-claim render. US-004 AC #2 content-
/// frozen; do NOT paraphrase — the exact string is the contract.
const LOCAL_ONLY_HEADER: &str = "Showing local claims only.";

/// Footer announcing that federation lands in slice-03 (per WD-13). The
/// `--federated` flag name and the `slice-03` token both appear verbatim
/// so operators grepping for either find this pointer.
const FEDERATION_FOOTER: &str =
    "(Federated peers are not queried in slice-01; pass --federated in slice-03 to widen the search.)";

/// One-time orientation line shown on the FIRST EVER `--federated`
/// invocation per install (FQ-6 / WD-39; gherkin habit scenario 1).
/// Content-frozen by the acceptance contract; do NOT paraphrase — the exact
/// string is the user-visible contract.
const FIRST_FEDERATED_QUERY_ORIENTATION: &str = "First federated query complete. Peer claims appear under their author DIDs. No claims are merged. Use `openlore peer add <did>` to follow more peers.";

/// Argument struct for the `graph query` verb (mirrors the clap subcommand).
#[derive(Debug, Clone)]
pub struct GraphQueryArgs {
    /// The subject URI to filter by (slice-01/03 dimension). Exact match (no
    /// fuzzy / prefix). Optional in slice-04 because `--object`/`--contributor`
    /// are alternative dimensions (ADR-020).
    pub subject: Option<String>,
    /// `--federated` (slice-03): widen the query to subscribed peers via
    /// `StoragePort::query_federated_by_subject`. Defaults to false
    /// (local-only — the slice-01 behavior). The slice-04 explorer flags IMPLY
    /// federated scope (WD-87 / OD-GRAPH-4).
    pub federated: bool,
    /// `--object <philosophy>` (slice-04 / ADR-020): the object-dimension read.
    pub object: Option<String>,
    /// `--contributor <did>` (slice-04 / ADR-020): the contributor-dimension read.
    pub contributor: Option<String>,
    /// `--traverse` (slice-04 / ADR-020): bounded graph traversal. OPT-IN.
    pub traverse: bool,
    /// `--depth <N>` (slice-04 / ADR-020 / WD-76): traversal depth bound,
    /// default 2.
    pub depth: u8,
    /// `--weighted` / `--score` (slice-04 / ADR-020): the transparent
    /// display-only weight ranking. OPT-IN.
    pub weighted: bool,
    /// `--explain <subject>` (slice-04 / ADR-020): per-claim weight audit. OPT-IN.
    pub explain: Option<String>,
}

impl GraphQueryArgs {
    /// True iff ANY slice-04 explorer flag is present. The explorer flags imply
    /// federated scope (WD-87 / OD-GRAPH-4); the bare `--subject`/`--federated`
    /// surface (no explorer flag) routes to the slice-01/03 behavior unchanged.
    fn uses_explorer_surface(&self) -> bool {
        self.object.is_some()
            || self.contributor.is_some()
            || self.traverse
            || self.weighted
            || self.explain.is_some()
    }
}

/// Outcome of one `graph query` invocation. The exit code + stdout chunk
/// the dispatcher emits. Matches the shape used by other verbs so the
/// dispatcher can route uniformly.
pub struct GraphQueryOutcome {
    pub exit_code: i32,
    pub stdout: String,
}

/// Run the `graph query` verb. Looks up claims by subject in local
/// storage and renders the result block to stdout.
///
/// Returns `anyhow::Error` on `StoragePort::query_by_subject` failure;
/// the dispatcher renders this via `eprintln!`. There is no specialized
/// failure renderer for graph-query because the local DuckDB is part of
/// the bootstrap state (probed at startup) — a query failure here is a
/// deeper integrity problem, not a user-fixable retry case.
///
/// Empty-result branch (WS-13 / US-004 AC #3): when the local store has
/// no claims for the subject, emit an explainer line naming the subject
/// instead of an empty per-claim block. Silence here would be hostile —
/// operators couldn't tell "no claims" from "the verb crashed". We keep
/// the federation footer so the user also sees the slice-03 affordance
/// (a future `--federated` pass might find the subject upstream). Exit
/// code stays 0: empty is a normal not-found result, not an error.
pub fn run(wiring: &Wiring, args: &GraphQueryArgs) -> Result<GraphQueryOutcome> {
    // Slice-04 (ADR-020): the explorer surface (any of --object / --contributor
    // / --traverse / --weighted / --explain) routes to the explorer handler,
    // which implies federated scope (WD-87 / OD-GRAPH-4). The bare
    // --subject / --federated surface below is the slice-01/03 behavior,
    // byte-identical (architecture-design §5.2 invariant 2).
    if args.uses_explorer_surface() {
        return run_explorer(wiring, args);
    }

    // Slice-01/03 path: requires the --subject dimension. (clap allows omitting
    // it now that --object/--contributor are alternatives; a bare `graph query`
    // with no dimension is a usage error.)
    let subject = args.subject.as_deref().ok_or_else(|| {
        anyhow::anyhow!(
            "graph query requires a dimension: pass --subject <uri>, --object <philosophy>, \
             or --contributor <did>"
        )
    })?;

    if args.federated {
        return run_federated(wiring, subject);
    }

    let claims = wiring
        .storage
        .query_by_subject(subject)
        .with_context(|| format!("querying claims by subject {subject}"))?;

    let mut stdout = String::new();
    stdout.push_str(LOCAL_ONLY_HEADER);
    stdout.push('\n');
    stdout.push('\n');

    if claims.is_empty() {
        // WS-13 / US-004 AC #3: name the subject so the message is
        // self-explanatory (an operator scanning logs sees WHICH lookup
        // came back empty). The federation footer follows so the
        // slice-03 `--federated` pointer is still visible.
        stdout.push_str(&format!("No local claims about {subject}.\n"));
    } else {
        // WS-15 / ADR-008 Behavioral rule 3 + WD-11: for each claim,
        // probe `query_referencing` to discover any back-pointers from
        // other local claims. A `ReferenceType::Retracts` back-pointer
        // means the original was soft-retracted; the renderer annotates
        // it `retracted by author` (content-frozen UX per WD-11). The
        // original artefact is NEVER mutated — annotation is a pure
        // render-time projection over immutable history.
        let annotated = annotate_claims(wiring, &claims)?;
        let rendered = render_annotated_graph_query_result(&annotated);
        stdout.push_str(&rendered);
        if !rendered.ends_with('\n') {
            stdout.push('\n');
        }
    }

    stdout.push('\n');
    stdout.push_str(FEDERATION_FOOTER);
    stdout.push('\n');

    Ok(GraphQueryOutcome {
        exit_code: 0,
        stdout,
    })
}

/// Run the `--federated` branch (FQ-1): widen the query across `claims` +
/// `peer_claims` via `StoragePort::query_federated_by_subject` (UNION ALL
/// with explicit `author_did` projection — never a JOIN; I-FED-1), then
/// render the rows GROUPED BY author DID with the content-frozen no-merge
/// footer (ADR-013).
///
/// The federated read is a single port call; the grouping + footer are a
/// pure projection (`render_federated_query_result`). Per-row attribution
/// (you / subscribed-peer / unsubscribed-cache) is carried on each
/// `FederatedRow.author_relationship` set by the adapter.
///
/// The zero-peers degraded footer (FQ-4 / US-FED-003 AC #7) is handled
/// purely inside `render_federated_query_result`: when no peer contributed a
/// row, it swaps the no-merge footer for the content-frozen `peer add` hint.
/// The verb stays a thin port-call + render — no branching here.
///
/// First-federated-query orientation (FQ-6 / WD-39): the FIRST EVER
/// `--federated` invocation per install emits a one-time orientation block
/// (gated by `OrientationState.first_federated_query_completed_at`), then
/// records the milestone so subsequent invocations omit it. Emitted BEFORE
/// the rendered result (data-models.md §OrientationState) so the framing
/// reads as an introduction to the per-author output that follows.
///
/// Out of FQ-1 scope (covered by a later slice-03 scenario, currently RED):
/// the inline counter template (FQ-7 / WD-42).
fn run_federated(wiring: &Wiring, subject: &str) -> Result<GraphQueryOutcome> {
    let rows = wiring
        .storage
        .query_federated_by_subject(subject)
        .with_context(|| format!("federated query by subject {subject}"))?;

    // FQ-6 / WD-39: prepend the one-time orientation (empty after the first
    // invocation) ahead of the rendered result. Gating + the non-fatal state
    // write live in the helper; the federated read itself is unaffected.
    let orientation_block = maybe_emit_first_federated_query_orientation(wiring);
    let rendered = render_federated_query_result(&rows);
    let stdout = format!("{orientation_block}{rendered}");

    Ok(GraphQueryOutcome {
        exit_code: 0,
        stdout,
    })
}

/// Slice-04 (ADR-020): the explorer dispatch entry. Routes a `graph query`
/// invocation carrying any explorer flag (`--object` / `--contributor` /
/// `--traverse` / `--weighted` / `--explain`) to the matching read path. The
/// explorer surface IMPLIES federated scope (WD-87 / OD-GRAPH-4) — own + peer
/// claims via the extended `StoragePort` scoring-feed / dimension / traversal
/// read methods, then the pure `scoring` core for `--weighted`/`--explain`.
///
/// SCAFFOLD: true (slice-04) — the body is a `todo!()`. DD-GRAPH-13 bootstrap:
/// this routes the parsed explorer flags into per-dimension handlers whose
/// bodies are also `todo!()`, so every slice-04 acceptance scenario reaches a
/// `todo!()` panic (RED) rather than a compile error (BROKEN). The per-scenario
/// GREEN implementation (Release 1: GQE-1/10/26; Release 2: GQE-6/20; Release 3:
/// GQE-16) lands the read path one acceptance scenario at a time.
fn run_explorer(wiring: &Wiring, args: &GraphQueryArgs) -> Result<GraphQueryOutcome> {
    // The dispatch tree (DELIVER materializes one branch per acceptance
    // scenario):
    //   - --weighted [--explain S]  -> score the attributed feed (pure
    //     `scoring::score`) and render WeightedView (+ Contribution list for
    //     --explain); --explain for a subject absent from the result set is a
    //     usage error (non-zero exit) per architecture-design §5.2 invariant 5.
    //   - --traverse [--depth K]    -> StoragePort::traverse_graph(start, bound)
    //     and render the bounded, cycle-safe edge tree (Gate 5).
    //   - --object O / --contributor D (no --weighted/--traverse) -> the plain
    //     attributed dimension listing grouped by subject / under the DID.
    //
    // The dimension feed is read through the extended StoragePort:
    //   query_by_object / query_by_contributor / query_attributed_for_scoring
    //   (federated UNION ALL, explicit author_did projection — anti-merging).

    // GQE-1 (US-GRAPH-001): the plain `--object` dimension listing grouped by
    // subject. Only this branch is GREEN at step 03-01; the --weighted /
    // --traverse / --explain / --contributor branches stay RED (todo!()) so
    // GQE-2..27 fail for the right reason.
    if !args.weighted && !args.traverse && args.explain.is_none() {
        if let Some(object) = args.object.as_deref() {
            return run_object_dimension(wiring, object);
        }
    }

    let _ = (wiring, args);
    todo!(
        "DELIVER (slice-04): dispatch the explorer surface — for --weighted/--explain feed the \
         extended StoragePort attributed claims into the pure scoring::score core and render the \
         WeightedView (+ Contribution breakdown for --explain); for --traverse call \
         StoragePort::traverse_graph(start, bound); else render the plain attributed dimension \
         listing. Explorer flags imply federated scope (WD-87/OD-GRAPH-4); every rendered row \
         carries its author_did (anti-merging, WD-73). (ADR-020; GQE-1..27)"
    )
}

/// GQE-1 (US-GRAPH-001 happy): the `--object <philosophy>` dimension read. Calls
/// the extended `StoragePort::query_by_object` (own + peer stores via a SAFE
/// `UNION ALL` projecting `author_did` — anti-merging) and renders the
/// attributed per-claim rows GROUPED BY SUBJECT with the distinct-subject +
/// distinct-author footer + the no-merge guarantee. Explorer flags imply
/// federated scope (WD-87) — the read already spans own + peers, so there is no
/// `--federated` branch here.
///
/// The verb stays a thin port-call + pure render: the read is one port call;
/// the grouping + footer are a pure projection (`render_object_query_grouped_by_subject`).
fn run_object_dimension(wiring: &Wiring, object: &str) -> Result<GraphQueryOutcome> {
    let claims = wiring
        .storage
        .query_by_object(object)
        .with_context(|| format!("querying claims by object {object}"))?;

    // GQE-4 (US-GRAPH-001 Example 4): when the object matched nothing, probe the
    // store for the nearest existing philosophy URI so the empty result carries
    // a "Did you mean ...?" near-match suggestion. The probe reuses ONLY the
    // existing `query_by_object` exact-match read (no new port surface): the
    // FIRST single-edit neighbour that has claims IS the closest existing object
    // string (a typo is one edit from the correct URI). Skipped entirely on the
    // happy path (claims found) so it costs nothing there.
    let suggestion = if claims.is_empty() {
        nearest_existing_object(wiring, object)?
    } else {
        None
    };

    let stdout = crate::render::render_object_query_grouped_by_subject(
        object,
        &claims,
        suggestion.as_deref(),
    );

    Ok(GraphQueryOutcome {
        exit_code: 0,
        stdout,
    })
}

/// Probe the store for the nearest EXISTING object to a `missed` (unmatched)
/// philosophy URI: the first single-edit-distance neighbour that itself has
/// claims (GQE-4 / US-GRAPH-001 Example 4). Returns `None` when no neighbour
/// matches (no near-match to suggest — the renderer then prints the bare
/// no-claims line). The candidate set is the pure
/// `render::single_edit_neighbours`; the existence check is the existing
/// `StoragePort::query_by_object` exact-match read — the suggestion is therefore
/// always a real object in the local graph, never a fabricated string.
fn nearest_existing_object(wiring: &Wiring, missed: &str) -> Result<Option<String>> {
    for candidate in crate::render::single_edit_neighbours(missed) {
        let matches = wiring
            .storage
            .query_by_object(&candidate)
            .with_context(|| format!("probing near-match object {candidate}"))?;
        if !matches.is_empty() {
            return Ok(Some(candidate));
        }
    }
    Ok(None)
}

/// Emit the first-federated-query orientation block exactly once per install
/// (FQ-6 / WD-39). Returns the rendered framing text to prepend ahead of the
/// federated result, or the empty string if it has already fired.
///
/// Mirrors `peer_pull::maybe_emit_first_pull_orientation` +
/// `claim_counter::maybe_emit_first_counter_claim_orientation`: load the
/// `[federation]` snapshot, consult the PURE `should_fire`, record the
/// milestone on first fire, and return the block. A write failure is
/// logged-and-ignored (the orientation may re-fire on the next query, but the
/// query itself succeeds) — never fatal (data-models.md §OrientationState).
fn maybe_emit_first_federated_query_orientation(wiring: &Wiring) -> String {
    let identity_path = wiring.paths.identity_toml();
    let state = orientation::load(&identity_path).unwrap_or_default();
    if !state.should_fire(OrientationMilestone::FirstFederatedQuery) {
        return String::new();
    }

    let now = wiring.clock.now_utc().to_rfc3339();
    if let Err(err) = orientation::mark_completed(
        &identity_path,
        OrientationMilestone::FirstFederatedQuery,
        now,
    ) {
        // Non-fatal: the orientation may re-fire on the next federated query,
        // but the query itself succeeds. Log to stderr, do not abort.
        eprintln!(
            "openlore graph query --federated: could not record first-federated-query orientation: {err:#}"
        );
    }

    first_federated_query_orientation_block()
}

/// PURE render of the one-time first-federated-query orientation block
/// (WD-39; gherkin habit scenario 1, content-frozen). One orientation line
/// followed by a blank line so the per-author result that follows reads as a
/// distinct section.
fn first_federated_query_orientation_block() -> String {
    format!("{FIRST_FEDERATED_QUERY_ORIENTATION}\n\n")
}

/// Compute the per-claim `is_retracted` annotation by probing the
/// storage port's back-reference index. Pure-ish: the storage I/O is at
/// the boundary; the projection rule (`is_retracted_by`) lives in the
/// renderer module as a free function so it stays unit-testable without
/// a wiring.
///
/// Errors surface as `anyhow::Error` via `with_context` so the
/// dispatcher's `eprintln!` carries the failing CID — same pattern
/// `query_by_subject` uses above.
fn annotate_claims(
    wiring: &Wiring,
    claims: &[claim_domain::SignedClaim],
) -> Result<Vec<AnnotatedClaim>> {
    let mut annotated = Vec::with_capacity(claims.len());
    for claim in claims {
        let target_cid = &claim.signature.signed_cid;
        let referencing = wiring
            .storage
            .query_referencing(target_cid)
            .with_context(|| format!("looking up back-references for cid {}", target_cid.0))?;
        let is_retracted = is_retracted_by(target_cid, &referencing);
        annotated.push(AnnotatedClaim {
            claim: claim.clone(),
            is_retracted,
        });
    }
    Ok(annotated)
}
