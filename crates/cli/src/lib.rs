//! `cli` — composition root + verb dispatch.
//!
//! Wires concrete adapters into the pure core. Hosts the two-prompt
//! interactive flow (ADR-003) and the `init | claim add | claim publish
//! <cid> | claim retract <cid> | graph query --subject <uri>` verbs.
//!
//! The `openlore` binary at `src/main.rs` delegates here.
//!
//! RED-baseline scaffold (step 01-01): every verb panics.
//
// SCAFFOLD: true

#![allow(dead_code)]
#![forbid(unsafe_code)]

use clap::{Parser, Subcommand};

/// Top-level CLI surface (ADR-003 locked verb contract).
#[derive(Debug, Parser)]
#[command(name = "openlore", version, about = "OpenLore — sign and publish philosophical claims")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Resolve identity, create DuckDB + identity config (idempotent).
    Init {
        #[arg(long)]
        handle: String,
        #[arg(long = "app-password")]
        app_password: String,
    },
    /// Claim operations (add / publish / retract).
    #[command(subcommand)]
    Claim(ClaimCommand),
    /// Graph operations (query).
    #[command(subcommand)]
    Graph(GraphCommand),
}

#[derive(Debug, Subcommand)]
pub enum ClaimCommand {
    /// Compose, preview, sign, optionally publish a new claim.
    Add {
        #[arg(long)]
        subject: String,
        #[arg(long)]
        predicate: String,
        #[arg(long)]
        object: String,
        #[arg(long)]
        evidence: Vec<String>,
        #[arg(long)]
        confidence: f64,
    },
    /// Publish a previously-signed claim by its CID.
    Publish { cid: String },
    /// Retract a published claim by counter-claim referencing original CID.
    Retract { cid: String },
}

#[derive(Debug, Subcommand)]
pub enum GraphCommand {
    /// Query the local store (and slice-03+ federated peers).
    Query {
        #[arg(long)]
        subject: String,
    },
}

/// Run the dispatched command. Every arm panics in the RED scaffold;
/// DELIVER fills them in one scenario at a time.
pub fn dispatch(cli: Cli) -> i32 {
    match cli.command {
        Command::Init { .. } => {
            panic!("Not yet implemented -- RED scaffold");
        }
        Command::Claim(ClaimCommand::Add { .. }) => {
            panic!("Not yet implemented -- RED scaffold");
        }
        Command::Claim(ClaimCommand::Publish { .. }) => {
            panic!("Not yet implemented -- RED scaffold");
        }
        Command::Claim(ClaimCommand::Retract { .. }) => {
            panic!("Not yet implemented -- RED scaffold");
        }
        Command::Graph(GraphCommand::Query { .. }) => {
            panic!("Not yet implemented -- RED scaffold");
        }
    }
}
