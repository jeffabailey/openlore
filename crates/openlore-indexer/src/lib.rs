//! The network indexer as a library (D-STZ-2): what the self-hosted binary and
//! the Lambda root compose. `main.rs` holds the clap surface; everything else is
//! here, so the split changes no behaviour of the binary.
//!
//! Signing-INCAPABLE, holding NO local store (ADR-023 / I-AV-5): the boundary is
//! the ABSENCE of those dependencies from this crate's graph, not a visibility
//! rule, so the split does not move it. `xtask check-arch` still enforces it.

#![forbid(unsafe_code)]

pub mod config;
#[cfg(unix)]
mod control;
pub mod events;
pub mod health;
pub mod ingest_pass;
pub mod pass_runner;
mod probe_gauntlet;
pub mod run;
pub mod runner_contract;
pub mod search_handler;

pub use probe_gauntlet::ProbeRefusal;
