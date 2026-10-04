//! `openlore-review-app` — the hosted review app, the THIRD composition root
//! (ADR-072).
//!
//! Subcommands:
//!   - `serve`          — wire, probe, then serve (refuse to start on any
//!                        hard probe failure: `health.startup.refused`, exit 2);
//!   - `probe`          — wire and probe only (`--self-test`: throwaway keys,
//!                        in-memory store, no network — the image self-test);
//!   - `gen-client-jwk` — print a fresh private ES256 client JWK for the
//!                        `client-jwk` secret.
//!
//! It holds no claim-signing identity and no local claim store: users' claims
//! are written to THEIR repos through the OAuth client, create-only.

#![forbid(unsafe_code)]

use std::collections::BTreeMap;
use std::process::ExitCode;

use clap::{Parser, Subcommand};

mod config;
mod http;
mod limiter;
mod routes {
    pub(crate) mod github;
    pub(crate) mod publish;
    pub(crate) mod review;
    pub(crate) mod scan;
    pub(crate) mod signin;
}
mod executor;
mod scan;
mod wiring;

#[derive(Debug, Parser)]
#[command(
    name = "openlore-review-app",
    version,
    about = "OpenLore review — review and publish your suggested claims to your own Bluesky repo"
)]
struct ReviewAppCli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Wire, probe, then serve the app.
    Serve,
    /// Wire and probe, then exit (0 = every probe passed).
    Probe {
        /// Probe throwaway keys and an in-memory store, offline.
        #[arg(long)]
        self_test: bool,
    },
    /// Print a fresh private ES256 client JWK (the `client-jwk` secret).
    GenClientJwk,
}

fn main() -> ExitCode {
    let env: BTreeMap<String, String> = std::env::vars().collect();
    let code = match ReviewAppCli::parse().command {
        Command::Serve => wiring::block_on(wiring::serve(&env)),
        Command::Probe { self_test: true } => wiring::self_test(),
        Command::Probe { self_test: false } => wiring::block_on(wiring::probe_only(&env)),
        Command::GenClientJwk => wiring::gen_client_jwk(),
    };
    ExitCode::from(code)
}
