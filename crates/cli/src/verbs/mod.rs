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

pub mod claim_add;
pub mod claim_counter;
pub mod claim_publish;
pub mod claim_retract;
pub mod graph_query;
pub mod init;
pub mod peer_add;
pub mod peer_pull;
pub mod peer_remove;
