//! `xtask` library target — re-export shim exposing the architecture-
//! verification modules so integration tests under `xtask/tests/*` can drive
//! the pure classifiers (`check_arch::classify_sql_literal`,
//! `check_arch::classify_autoconfirm_guard`, `check_probes::classify_probe_body`).
//!
//! The `main.rs` binary consumes these same modules via this crate, so there is
//! exactly ONE compilation of each module (no bin/lib duplication). Adding this
//! lib target is what lets Rust integration tests link against the crate —
//! integration tests cannot reach a binary-only crate's modules.

#![forbid(unsafe_code)]

pub mod check_arch;
pub mod check_probes;
