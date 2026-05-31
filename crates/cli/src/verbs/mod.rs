//! `verbs` — one module per CLI verb. Each verb is a function that takes
//! a `&Wiring` plus its own argument struct and returns an exit code +
//! a chunk of stdout. Verbs do not perform their own stdout writes;
//! the dispatcher prints the captured text. This makes verb logic
//! unit-testable without spawning a subprocess.
//!
//! Slice-01 verbs:
//! - `init`: bootstrap identity + DuckDB; idempotent.
//! - `claim_add`: compose preview + first half of the two-prompt flow.
//! - `claim_publish`, `claim_retract`, `graph_query`.
//!
//! Slice-03 verbs (federated read; step 01-04 declares the handlers as
//! `todo!()` scaffolds; live impls land per-scenario in Phases 03-05):
//! - `claim_counter`: author a counter-claim against another claim.
//! - `peer_add` / `peer_pull` / `peer_remove`: peer subscription lifecycle.
//! - `graph_query` gains the `--federated` branch.
//!
//! Slice-02 verbs (github scraper; step 01-04 declares the handler as a
//! `todo!()` scaffold; the live pipeline lands per-scenario in Phases 03-05):
//! - `scrape_github`: derive candidate claims from a public GitHub target,
//!   optionally signing selected candidates via the slice-01 pipeline.

pub mod claim_add;
pub mod claim_counter;
pub mod claim_publish;
pub mod claim_retract;
pub mod graph_query;
pub mod init;
pub mod peer_add;
pub mod peer_pull;
pub mod peer_remove;
pub mod scrape_github;
// Slice-05 (appview search; step 01-04): the `openlore search` NETWORK verb
// (WD-113). `todo!()` handler bodies; the live XRPC dispatch lands per-scenario
// in Phase 03/04 (AV-* scenarios register at 01-05).
pub mod search;
// Slice-06 (htmx viewer; ADR-028/030): the `openlore ui` read-only viewer verb.
// A long-running localhost HTTP server over a READ-ONLY `StoreReadPort`; the
// ONLY verb that links `adapter-http-viewer` (cli is its sole linker).
pub mod ui;

/// Strip a `#fragment` from a DID, returning the bare DID. A signed
/// claim's `author` carries the verification-method fragment
/// (`did:plc:rachel-test#org.openlore.application`); the bare DID is what
/// the self-/cross-attribution comparisons and the display lines use.
/// Shared by [`peer_pull`] (the pure SelfAttribution pre-check) and
/// [`claim_counter`] (the `counters: <cid> (by <peer>)` preview line) so the
/// fragment-stripping rule lives in exactly one place.
pub(crate) fn bare_did(did: &str) -> String {
    did.split('#').next().unwrap_or(did).to_string()
}
