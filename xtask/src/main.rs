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
//!   to the pre-commit hook `scripts/check-probes.sh`. Lands in a
//!   later step (06-XX).

#![forbid(unsafe_code)]

mod check_arch;

use std::process::ExitCode;

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
        "check-probes" => {
            // SCAFFOLD: still stubbed; lands in a later step.
            eprintln!("xtask check-probes: not yet implemented");
            ExitCode::from(2)
        }
        other => {
            eprintln!(
                "xtask: unknown subcommand `{other}`; expected one of: check-arch, check-probes"
            );
            ExitCode::from(2)
        }
    }
}
