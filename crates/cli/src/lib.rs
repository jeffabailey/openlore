//! `cli` — composition root + verb dispatch.
//!
//! Wires concrete adapters into the pure core. Hosts the two-prompt
//! interactive flow (ADR-003) and the `init | claim add | claim publish
//! <cid> | claim retract <cid> | graph query --subject <uri>` verbs.
//!
//! The `openlore` binary at `src/main.rs` delegates here.
//!
//! Slice-01 status: `init` is implemented (step 05-01). All other verbs
//! still panic in the RED scaffold; subsequent phase-05 steps fill them
//! in one acceptance scenario at a time.

#![allow(dead_code)]
#![forbid(unsafe_code)]

use clap::{Parser, Subcommand};

pub mod errors;
pub mod io;
pub mod paths;
pub mod render;
pub mod verbs;
pub mod wiring;

use paths::OpenLorePaths;
use wiring::Wiring;

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

/// Dispatch a parsed command. Returns the exit code the caller should
/// hand back to the OS. Slice-01 wires the `init` verb; other arms
/// panic in the RED scaffold per the outside-in plan.
///
/// The wire-probe-use sequence per ADR-009 D-9:
/// 1. Resolve XDG paths.
/// 2. Construct Wiring (instantiates every adapter).
/// 3. Walk the probe gauntlet; refuse with health.startup.refused on any refusal.
/// 4. For non-`init` verbs, check the bootstrap-state arm: identity.toml
///    must exist. Missing identity.toml means the user has not run
///    `openlore init` yet; refuse with a hint pointing at that command.
/// 5. Dispatch the verb.
pub fn dispatch(cli: Cli) -> i32 {
    // Step 1: paths.
    let paths = match OpenLorePaths::from_env() {
        Ok(p) => p,
        Err(err) => {
            eprintln!("openlore: failed to resolve XDG paths: {err:#}");
            return 2;
        }
    };

    // Step 2: wiring.
    let wiring = match Wiring::production(paths) {
        Ok(w) => w,
        Err(err) => {
            eprintln!("openlore: failed to construct adapter wiring: {err:#}");
            return 2;
        }
    };

    // Step 3: probe gauntlet.
    if let Err(refusal) = wiring.probe_gauntlet() {
        emit_health_startup_refused(&refusal);
        return 2;
    }

    // Step 4: bootstrap-state arm. The `init` verb IS the bootstrap;
    // every other verb requires it to have run successfully at least
    // once (identity.toml present at the resolved config path).
    if requires_initialized_state(&cli.command) {
        if let Err(refusal) = wiring.check_initialized_state() {
            emit_health_startup_refused(&refusal);
            return 2;
        }
    }

    // Step 5: dispatch.
    match cli.command {
        Command::Init {
            handle,
            app_password,
        } => match verbs::init::run(
            &wiring,
            &verbs::init::InitArgs {
                handle,
                app_password,
            },
        ) {
            Ok(outcome) => {
                print!("{}", outcome.stdout);
                outcome.exit_code
            }
            Err(err) => {
                eprintln!("openlore init: {err:#}");
                1
            }
        },
        Command::Claim(ClaimCommand::Add {
            subject,
            predicate,
            object,
            evidence,
            confidence,
        }) => match verbs::claim_add::run(
            &wiring,
            &verbs::claim_add::ClaimAddArgs {
                subject,
                predicate,
                object,
                evidence,
                confidence,
            },
        ) {
            Ok(outcome) => {
                print!("{}", outcome.stdout);
                outcome.exit_code
            }
            Err(err) => {
                eprintln!("openlore claim add: {err:#}");
                1
            }
        },
        Command::Claim(ClaimCommand::Publish { cid }) => match verbs::claim_publish::run(
            &wiring,
            &verbs::claim_publish::ClaimPublishArgs { cid },
        ) {
            Ok(outcome) => {
                print!("{}", outcome.stdout);
                outcome.exit_code
            }
            Err(err) => {
                // Typed `PublishError` routes PDS failures through the
                // WS-10 retry-hint renderer; other failure classes fall
                // back to anyhow's chained-cause format. The renderer
                // produces a newline-terminated string so we use
                // `eprint!` (not `eprintln!`) here.
                eprint!("{}", verbs::claim_publish::render_publish_error(&err));
                1
            }
        },
        Command::Claim(ClaimCommand::Retract { .. }) => {
            panic!("Not yet implemented -- RED scaffold");
        }
        Command::Graph(GraphCommand::Query { subject }) => {
            match verbs::graph_query::run(
                &wiring,
                &verbs::graph_query::GraphQueryArgs { subject },
            ) {
                Ok(outcome) => {
                    print!("{}", outcome.stdout);
                    outcome.exit_code
                }
                Err(err) => {
                    eprintln!("openlore graph query: {err:#}");
                    1
                }
            }
        }
    }
}

/// Predicate: does this verb require `openlore init` to have run first?
///
/// `Init` itself is the bootstrap verb and MUST be permitted on a fresh
/// environment (otherwise the system is unreachable). Every other verb
/// in the ADR-003 contract — `claim add`, `claim publish`, `claim
/// retract`, `graph query` — operates on initialized identity + storage
/// state and is therefore gated on the bootstrap-state arm.
fn requires_initialized_state(cmd: &Command) -> bool {
    !matches!(cmd, Command::Init { .. })
}

/// Emit a `health.startup.refused` event to stderr in the structured
/// shape DevOps consumes. The pure data (`reason`, `detail`, `structured`)
/// comes straight from the refusing adapter's `ProbeOutcome::Refused`
/// payload — no enrichment, no rewording.
///
/// For slice-01 the event format is a single JSON-line on stderr plus a
/// human-readable line. The tracing-subscriber integration lands in
/// step 05-17 (observability). The shape of the JSON line is the
/// contract; the human-readable line is convenience.
fn emit_health_startup_refused(refusal: &wiring::ProbeRefusal) {
    let event = serde_json::json!({
        "event": "health.startup.refused",
        "adapter": refusal.adapter,
        "reason": format!("{:?}", refusal.reason),
        "detail": refusal.detail,
        "structured": refusal.structured,
    });
    eprintln!("{event}");
    eprintln!(
        "openlore: refusing to start — {} adapter: {}",
        refusal.adapter, refusal.detail
    );
}
