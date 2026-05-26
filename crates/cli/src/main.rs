//! `openlore` — the binary entry point.
//!
//! Parses args with clap, then delegates to `cli::dispatch`. Uses
//! `tokio::runtime::Builder::new_current_thread` per ADR-004 (low
//! concurrency CLI; no need for multi-thread runtime).
//!
//! The async runtime is constructed once and entered for the duration
//! of dispatch so PdsPort async calls have a runtime context to
//! execute on. Slice-01 init verb does not touch the runtime; later
//! verbs (claim publish, claim retract) do.

#![forbid(unsafe_code)]

use clap::Parser;
use cli::{dispatch, Cli};

fn main() -> std::process::ExitCode {
    let parsed = Cli::parse();

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("failed to construct tokio current-thread runtime");

    let code = runtime.block_on(async { dispatch(parsed) });

    std::process::ExitCode::from(u8::try_from(code & 0xFF).unwrap_or(1))
}
