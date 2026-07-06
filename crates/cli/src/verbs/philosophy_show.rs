//! `philosophy show` — inspect ONE philosophy in full (slice-23; ADR-059 §5).
//!
//! `openlore philosophy show <name-or-object>` resolves EITHER a bare name
//! (`memory-safety`) OR the full derived object id
//! (`org.openlore.philosophy.memory-safety`) to a single embedded seed and
//! prints its name, FULL description (verbatim), aliases, and seeAlso link — so
//! the user confirms this is the right philosophy (and which alias strings
//! resolve) before copying its exact object into a claim (J-002; US-PV-002).
//!
//! OFFLINE by construction (ADR-059 D3): the verb reads the compile-time
//! `lexicon::philosophy::find` resolver over the embedded seeds — NO store
//! handle, NO signer, NO network. Like `philosophy list` it is dispatched as
//! its OWN read-only entry point BEFORE `Wiring::production` is built, so it
//! runs even before `init` and with the network disabled (AC-002.1 / I-9).
//!
//! Returns `(exit_code, stdout)`; the dispatcher prints the captured text.

use lexicon::philosophy::find;

use crate::render::render_record;

/// Argument struct for the `philosophy show` verb (mirrors the clap subcommand).
#[derive(Debug, Clone)]
pub struct PhilosophyShowArgs {
    /// The name-OR-object key to resolve: a bare philosophy name or its full
    /// derived object id. Both resolve to the same record (ADR-059 §5).
    pub key: String,
}

/// Run the `philosophy show` verb. Returns `(exit_code, stdout)`; the dispatcher
/// prints the captured text. Offline: resolves the key against the embedded
/// seeds via the pure `find` resolver.
///
/// On a hit: render the full record (exit 0). On a miss: a minimal non-zero
/// return for now — the full unknown-name guidance lands in the sibling step
/// (01-02). No panic / unwrap / expect on the miss path.
pub fn run(args: &PhilosophyShowArgs) -> (i32, String) {
    match find(&args.key) {
        Some(record) => (0, render_record(&record)),
        None => (1, String::new()),
    }
}
