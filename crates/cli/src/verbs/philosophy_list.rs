//! `philosophy list` — discover the shared philosophy vocabulary (slice-22).
//!
//! `openlore philosophy list` prints the embedded well-known philosophy seeds
//! (ADR-059) so the user can copy an EXACT shared object id into a claim
//! `--object` instead of inventing a private string (J-002; US-PV-001).
//!
//! OFFLINE by construction (ADR-059 D3): the verb reads the compile-time
//! `lexicon::philosophy::seeds()` constants — NO store handle, NO signer, NO
//! network. It is dispatched as its OWN read-only entry point BEFORE the
//! read-write `Wiring::production` is built (mirroring the `ui` verb), so it
//! runs even before `init` and with the network disabled (AC-001.4 / I-9).
//!
//! Like every other verb it returns `(exit_code, stdout)` and performs NO
//! stdout writes of its own — the dispatcher prints the captured text — so the
//! verb logic is unit-testable without spawning a subprocess.

use lexicon::philosophy::seeds;

use crate::render::render_philosophy_list;

/// Argument struct for the `philosophy list` verb (mirrors the clap subcommand).
#[derive(Debug, Clone)]
pub struct PhilosophyListArgs {
    /// `--json`: opt-in machine-readable emission. Text is the DEFAULT view
    /// (AC-001.3 / P-001). The JSON-array rendering lands in a sibling step
    /// (02-02); at this step the flag parses and still yields the text view.
    pub json: bool,
}

/// Run the `philosophy list` verb. Returns `(exit_code, stdout)`; the dispatcher
/// prints the captured text. Offline: reads the embedded seeds only, renders the
/// human-readable text view via the pure `render_philosophy_list`.
pub fn run(args: &PhilosophyListArgs) -> (i32, String) {
    // The JSON-array emission is a sibling step (02-02); until then `--json`
    // parses but the text view is the only rendering. Text is the default per
    // AC-001.3, so falling back to text here keeps `--json` a harmless no-op
    // rather than a parse error.
    let _ = args.json;
    let stdout = render_philosophy_list(&seeds());
    (0, stdout)
}
