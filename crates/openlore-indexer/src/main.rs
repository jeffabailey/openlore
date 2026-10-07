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
//! Parses args with clap, then delegates to `run::run`, which does the
//! wire → PROBE → use gate (refuse to start on any probe failure: emit
//! `health.startup.refused` + exit 2) before dispatching the subcommand.
//!
//! `serve` and `ingest` are live; `stats` is still a `todo!()` scaffold.

#![forbid(unsafe_code)]

use clap::{Parser, Subcommand};

mod config;
#[cfg(unix)]
mod control;
mod health;
mod pass_runner;
mod probe_gauntlet;
mod run;
mod search_handler;

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

/// The indexer subcommands. `serve` is the long-running search server; `ingest`
/// is a one-shot bounded PULL; `stats` reports coverage.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Serve the search query surface over the index (ADR-027, ADR-080).
    Serve,
    /// Run a one-shot bounded PULL pass (ADR-024).
    Ingest,
    /// Report index coverage (claims indexed, distinct authors, ingest lag).
    Stats,
    /// Ask the running `serve` for one pass; exit with that pass's code
    /// (0 also when it joined a running pass; 4 when `serve` is unreachable).
    Trigger,
}

fn main() -> std::process::ExitCode {
    let parsed = IndexerCli::parse();
    let code = match parsed.command {
        Command::Trigger => trigger(),
        command => run::run(command),
    };
    std::process::ExitCode::from(u8::try_from(code & 0xFF).unwrap_or(1))
}

/// `trigger` reads ONLY the control-socket variable — no config, no probes, no
/// store (M4: the client must never take the index lock from `serve`).
#[cfg(unix)]
fn trigger() -> i32 {
    control::trigger(std::env::var(control::CONTROL_SOCKET_VAR).ok())
}

/// The control channel exists only on Unix: elsewhere `serve` is unreachable.
#[cfg(not(unix))]
fn trigger() -> i32 {
    eprintln!(
        "{}",
        serde_json::json!({
            "event": "indexer.trigger.unreachable",
            "cause": "unsupported_platform",
        })
    );
    4
}
