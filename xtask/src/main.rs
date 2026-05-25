//! `cargo xtask` — architecture-verification tasks.
//!
//! Two subcommands today (both stubbed for step 01-01):
//!
//! - `cargo xtask check-arch` — parses `cargo metadata` to enforce the
//!   dependency rules from `component-boundaries.md` §Cross-component
//!   invariants (claim-domain/lexicon MUST NOT touch I/O crates; no
//!   adapter-* depends on another adapter-*; only cli depends on
//!   adapter-*).
//! - `cargo xtask check-probes` — AST-walks every `impl <Port> for
//!   <Adapter>` block and asserts a non-stub `probe()` body. Companion
//!   to the pre-commit hook `scripts/check-probes.sh`.
//!
//! Both are step 06-XX deliverables; this scaffold only ensures the
//! `xtask` binary exists and routes args.
//
// SCAFFOLD: true

#![forbid(unsafe_code)]

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let subcommand = args.get(1).map(String::as_str).unwrap_or("");

    match subcommand {
        "check-arch" => {
            panic!("Not yet implemented -- RED scaffold");
        }
        "check-probes" => {
            panic!("Not yet implemented -- RED scaffold");
        }
        other => {
            eprintln!(
                "xtask: unknown subcommand `{other}`; expected one of: check-arch, check-probes"
            );
            std::process::exit(2);
        }
    }
}
