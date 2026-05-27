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

use crate::render::render_graph_query_result;
use crate::wiring::Wiring;

/// Header line printed before the per-claim render. US-004 AC #2 content-
/// frozen; do NOT paraphrase — the exact string is the contract.
const LOCAL_ONLY_HEADER: &str = "Showing local claims only.";

/// Footer announcing that federation lands in slice-03 (per WD-13). The
/// `--federated` flag name and the `slice-03` token both appear verbatim
/// so operators grepping for either find this pointer.
const FEDERATION_FOOTER: &str =
    "(Federated peers are not queried in slice-01; pass --federated in slice-03 to widen the search.)";

/// Argument struct for the `graph query` verb (mirrors the clap subcommand).
#[derive(Debug, Clone)]
pub struct GraphQueryArgs {
    /// The subject URI to filter by. Exact match (no fuzzy / prefix).
    pub subject: String,
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
        let rendered = render_graph_query_result(&claims);
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
