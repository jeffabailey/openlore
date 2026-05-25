//! `openlore` — the binary entry point.
//!
//! Parses args with clap, then delegates to `cli::dispatch`. Uses
//! `tokio::runtime::Builder::new_current_thread` per ADR-004 (low
//! concurrency CLI; no need for multi-thread runtime).
//!
//! RED-baseline scaffold (step 01-01).
//
// SCAFFOLD: true

#![forbid(unsafe_code)]

use clap::Parser;
use cli::{dispatch, Cli};

fn main() -> std::process::ExitCode {
    let parsed = Cli::parse();
    // Build a single-threaded tokio runtime so PdsPort async calls have a
    // home. The runtime is unused in the RED scaffold (every dispatch
    // arm panics first); DELIVER wires it through when adapters land.
    let _runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("failed to construct tokio current-thread runtime");

    let code = dispatch(parsed);
    std::process::ExitCode::from(u8::try_from(code & 0xFF).unwrap_or(1))
}
