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
//!   WS-12 (step 05-12) adds the "Showing local claims only" footer.
//! - Empty-result explainer (US-004 AC #3) is WS-13's contract (step
//!   05-12 in the wave; this verb returns an empty stdout for empty
//!   queries in slice-01 and the explainer text wraps around it later).

use anyhow::{Context, Result};

use crate::render::render_graph_query_result;
use crate::wiring::Wiring;

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
pub fn run(wiring: &Wiring, args: &GraphQueryArgs) -> Result<GraphQueryOutcome> {
    let claims = wiring
        .storage
        .query_by_subject(&args.subject)
        .with_context(|| format!("querying claims by subject {}", args.subject))?;

    let stdout = render_graph_query_result(&claims);

    Ok(GraphQueryOutcome {
        exit_code: 0,
        stdout,
    })
}
