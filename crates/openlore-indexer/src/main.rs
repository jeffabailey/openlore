//! `openlore-indexer` — the network indexer binary (the SECOND composition root).
//!
//! ADR-023: a self-hostable single binary that ingests ONLY public, signed,
//! signature-verified claims into a SEPARATE re-buildable `index.duckdb`, and
//! serves dimensional search over `org.openlore.appview.searchClaims` (ADR-027).
//! It is signing-INCAPABLE and holds NO local store — it never touches the
//! user's `openlore.duckdb` and cannot author/sign/publish a claim (the
//! capability boundary, ADR-023 / I-AV-5). The structural backstop is `xtask
//! check-arch`'s `indexer_holds_no_signing_or_local_store` rule.
//!
//! Subcommands:
//!   - `serve`  — run the bounded pull-ingest loop + the query server.
//!   - `ingest` — a one-shot bounded PULL pass (ADR-024).
//!   - `stats`  — report index coverage.
//!
//! Parses args with clap, then delegates to `run::run`, which does the
//! wire → PROBE → use gate (refuse to start on any probe failure: emit
//! `health.startup.refused` + exit 2) before dispatching the subcommand.
//!
//! Bootstrap SCAFFOLD (step 01-04): the binary + the clap surface + the
//! composition-root sequence are established; the verb bodies + adapter
//! constructors are `todo!()` (the real ingest/serve lands in Phase 03/04).
//
// SCAFFOLD: true

#![forbid(unsafe_code)]

use clap::{Parser, Subcommand};

mod probe_gauntlet;
mod run;

/// The `openlore-indexer` CLI surface (ADR-023 single-binary indexer).
#[derive(Debug, Parser)]
#[command(
    name = "openlore-indexer",
    version,
    about = "OpenLore network indexer — ingest + serve public verified claims (ADR-023)"
)]
pub struct IndexerCli {
    #[command(subcommand)]
    pub command: Command,
}

/// The indexer subcommands. `serve` is the long-running mode; `ingest` is a
/// one-shot bounded PULL; `stats` reports coverage.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Run the bounded pull-ingest loop + serve the query surface (ADR-024/027).
    Serve,
    /// Run a one-shot bounded PULL pass (ADR-024).
    Ingest,
    /// Report index coverage (claims indexed, distinct authors, ingest lag).
    Stats,
}

fn main() -> std::process::ExitCode {
    let parsed = IndexerCli::parse();
    let code = run::run(parsed.command);
    std::process::ExitCode::from(u8::try_from(code & 0xFF).unwrap_or(1))
}
