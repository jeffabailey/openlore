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
    /// The subject URI to filter by. Exact match (no fuzzy / prefix).
    pub subject: String,
    /// `--federated` (slice-03): widen the query to subscribed peers via
    /// `StoragePort::query_federated_by_subject`. Defaults to false
    /// (local-only — the slice-01 behavior). The live federated branch is
    /// driven by the FQ-* acceptance scenarios in a later slice-03 phase;
    /// step 01-04 only routes the flag.
    pub federated: bool,
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
    if args.federated {
        return run_federated(wiring, args);
    }

    let claims = wiring
        .storage
        .query_by_subject(&args.subject)
        .with_context(|| format!("querying claims by subject {}", args.subject))?;

    let mut stdout = String::new();
    stdout.push_str(LOCAL_ONLY_HEADER);
    stdout.push('\n');
    stdout.push('\n');

    if claims.is_empty() {
        // WS-13 / US-004 AC #3: name the subject so the message is
        // self-explanatory (an operator scanning logs sees WHICH lookup
        // came back empty). The federation footer follows so the
        // slice-03 `--federated` pointer is still visible.
        stdout.push_str(&format!("No local claims about {}.\n", args.subject));
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
fn run_federated(wiring: &Wiring, args: &GraphQueryArgs) -> Result<GraphQueryOutcome> {
    let rows = wiring
        .storage
        .query_federated_by_subject(&args.subject)
        .with_context(|| format!("federated query by subject {}", args.subject))?;

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
