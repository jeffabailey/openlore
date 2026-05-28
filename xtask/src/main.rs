//! `cargo xtask` — architecture-verification tasks.
//!
//! Two subcommands:
//!
//! - `cargo xtask check-arch` — parses `cargo metadata` to enforce the
//!   dependency rules from `component-boundaries.md` §Cross-component
//!   invariants (claim-domain/lexicon MUST NOT touch I/O crates; no
//!   adapter-* depends on another adapter-*; only cli depends on
//!   adapter-*). Implemented in [`check_arch`]; step 06-05.
//! - `cargo xtask check-probes` — AST-walks every `impl <Port> for
//!   <Adapter>` block and asserts a non-stub `probe()` body. Companion
//!   to the pre-commit hook `scripts/check-probes.sh`. Implemented in
//!   [`check_probes`]; step 06-06.

#![forbid(unsafe_code)]

use std::process::ExitCode;

use xtask::{check_arch, check_probes};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    let subcommand = args.get(1).map(String::as_str).unwrap_or("");

    match subcommand {
        "check-arch" => match check_arch::run() {
            Ok(code) => ExitCode::from(code as u8),
            Err(e) => {
                eprintln!("xtask check-arch: {e:#}");
                ExitCode::from(2)
            }
        },
        "check-probes" => match check_probes::run() {
            Ok(code) => ExitCode::from(code),
            Err(e) => {
                eprintln!("xtask check-probes: {e:#}");
                ExitCode::from(2)
            }
        },
        other => {
            eprintln!(
                "xtask: unknown subcommand `{other}`; expected one of: check-arch, check-probes"
            );
            ExitCode::from(2)
        }
    }
}
