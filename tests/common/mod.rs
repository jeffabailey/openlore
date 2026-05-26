//! Shared test helpers across acceptance + integration test crates.
//!
//! Includes:
//! - [`state_delta`] — universe-bound state-delta assertion (DD-3
//!   bootstrap, step 04-05). The Rust port of the
//!   `nw-tdd-methodology` delta-first paradigm.
//!
//! ## Usage
//!
//! Future state-mutating acceptance tests (WS-7 CID stability, FR-3
//! at_uri reconstructibility per `distill/wave-decisions.md` DD-3)
//! pull this in via `#[path = "../../tests/common/state_delta.rs"]
//! mod state_delta;` at the top of the consuming test file, then
//! call `state_delta::assert_state_delta(...)`.

pub mod state_delta;
