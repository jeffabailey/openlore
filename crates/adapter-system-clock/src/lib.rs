//! `adapter-system-clock` — `ClockPort` over `chrono::Utc::now()`.
//!
//! Degenerate adapter (probe always Ok). Exists so the contract symmetry
//! holds — every port has at least one named adapter.
//!
//! RED-baseline scaffold (step 01-01).
//
// SCAFFOLD: true

#![allow(dead_code)]
#![forbid(unsafe_code)]

use chrono::{DateTime, Utc};
use ports::{ClockPort, ProbeOutcome};

pub struct SystemClockAdapter;

impl SystemClockAdapter {
    pub fn new() -> Self {
        Self
    }
}

impl Default for SystemClockAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl ClockPort for SystemClockAdapter {
    fn probe(&self) -> ProbeOutcome {
        panic!("Not yet implemented -- RED scaffold");
    }

    fn now_utc(&self) -> DateTime<Utc> {
        panic!("Not yet implemented -- RED scaffold");
    }
}
