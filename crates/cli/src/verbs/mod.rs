//! `verbs` — one module per CLI verb. Each verb is a function that takes
//! a `&Wiring` plus its own argument struct and returns an exit code +
//! a chunk of stdout. Verbs do not perform their own stdout writes;
//! the dispatcher prints the captured text. This makes verb logic
//! unit-testable without spawning a subprocess.
//!
//! Slice-01 verbs:
//! - `init`: bootstrap identity + DuckDB; idempotent.
//! - (later) `claim_add`, `claim_publish`, `claim_retract`, `graph_query`.

pub mod init;
