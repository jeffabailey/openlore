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
//!   - `serve`  — answer searches over the index (the query server; no ingest).
//!   - `ingest` — a one-shot bounded PULL pass (ADR-024).
//!   - `stats`  — report index coverage.
//!   - `trigger` — ask the running `serve` for one pass over its control
//!     socket (ADR-080 §3). Dispatched BEFORE any configuration is parsed or
//!     store opened: it reads only the socket variable (M4).
//!
//! The wiring, the pass and the handlers live in the library (D-STZ-2), so the
//! Lambda root can reuse them. This binary is the clap surface and nothing else.
//!
//! Parses args with clap, then delegates to `run::run`, which does the
//! wire → PROBE → use gate (refuse to start on any probe failure: emit
//! `health.startup.refused` + exit 2) before dispatching the subcommand.
//!
//! `serve` and `ingest` are live; `stats` is still a `todo!()` scaffold.

#![forbid(unsafe_code)]

use clap::Parser;
use openlore_indexer::run::{run, trigger, Command};

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

fn main() -> std::process::ExitCode {
    let parsed = IndexerCli::parse();
    let code = match parsed.command {
        Command::Trigger => trigger(),
        command => run(command),
    };
    std::process::ExitCode::from(u8::try_from(code & 0xFF).unwrap_or(1))
}
